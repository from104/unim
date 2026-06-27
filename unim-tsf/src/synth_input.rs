//! AutoTypeFix CUAS 폴백 — 합성 키 주입 (SendInput).
//!
//! CUAS(IMM32 브리지)·Blink 앱의 가상 문서는 이미 확정(commit)된 텍스트를 ITfRange
//! 로 노출하지 않아 ShiftStart 가 요청량만큼 이동하지 못한다(shifted 부족).
//! 그 경우 범위 편집 대신 이 모듈이 VK_BACK × delete_chars 를 입력 큐에 주입한다
//! (삭제만 SendInput). 삽입은 SendInput UNICODE 가 아니라 **TSF 조합/edit session**
//! 으로 별도 단계에서 수행한다(b1 2-phase 설계).
//!
//! ## b1 (삭제↔삽입 분리)
//! v0.3.31 까지는 `send_replacement` 가 [BS×N]+[UNICODE 교정문자] 를 **한 SendInput
//! 배치**로 보냈다. Blink(Chrome/WebView2)는 같은 배치의 BS 와 UNICODE 를 비동기로
//! 재정렬/병합해(실측: 주입 [BS BS BS BS 서 기]가 sink 로 [서 기 BS BS]로 뒤바뀜)
//! 첫 한글 글자를 유실했다. b1 은 삭제(SendInput BS×N)와 삽입(TSF edit session)을
//! **시간적으로 분리**해 같은 배치에 섞이지 않게 한다 → Blink 의 재정렬 대상 자체가
//! 사라진다.  Phase1: send_replacement_delete(BS×N) + store_pending_insert(commit,last).
//! Phase2(타이머): text_service 가 SetTimer 로 BS 문서 적용을 기다린 뒤 TSF 로 삽입.
//!
//! 재진입 차단: 주입한 BS 는 앱 메시지 루프를 거쳐 우리 자신의
//! ITfKeyEventSink(OnTestKeyDown/OnKeyDown)로 되돌아올 수 있다. dwExtraInfo 는 TSF
//! 콜백까지 전파가 보장되지 않으므로, 대신 "남은 합성 keydown 수"(BS 만) 카운터로
//! 식별해 엔진을 거치지 않고 통과시킨다. UNICODE/센티널을 더는 보내지 않으므로
//! "BS↔PACKET 재정렬" 문제가 구조적으로 소멸한다(VK_PACKET 분기는 방어적 유지).

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_BACK, VK_PACKET,
};

/// 디버깅용 마커 (식별은 카운터로 하므로 필수 아님).
const EXTRA_INFO_MARK: usize = 0x554E_494D; // "UNIM"

/// 아직 sink 에 도착하지 않은 합성 keydown 수(BS 만).
static PENDING: AtomicI32 = AtomicI32::new(0);
/// 직전 OnTestKeyDown 이 합성 키였음 (OnKeyDown 까지 호출하는 앱 대비).
static LAST_WAS_SYNTH: AtomicBool = AtomicBool::new(false);
/// 주입 시각 — 일부 이벤트가 유실돼 카운터가 남는 경우의 부패(stale) 방지.
static SEND_INSTANT: Mutex<Option<Instant>> = Mutex::new(None);

/// b1 Phase2 보류 삽입 슬롯.
///
/// Phase1(send_replacement_delete)이 BS×N 만 주입한 뒤, 삭제가 Blink 문서에
/// 적용될 시간을 벌기 위해 (마지막 음절 제외 확정문 + 마지막 음절 조합문)을 여기
/// 보관한다. text_service 의 SetTimer(WM_TIMER)가 발화하거나, 그 전에 사용자가 다음
/// 키를 누르면(race-flush) take_pending_insert 로 꺼내 TSF edit session 으로 삽입한다.
pub struct PendingInsert {
    /// 마지막 음절 제외분 — 확정(commit) 삽입 (예 "서").
    pub commit_text: String,
    /// 마지막 음절 — 미확정 조합(preedit) 유지 (예 "기"). 빈 문자열이면 조합 생략(역방향/undo).
    pub last_syllable: String,
}

/// b1 Phase2 보류 삽입 슬롯 (Phase1 store, Phase2/타이머/race-flush take).
static PENDING_INSERT: Mutex<Option<PendingInsert>> = Mutex::new(None);

/// D3 read-back 게이트 — OnEndEdit 가 BS 삭제 적용을 검증할지 여부.
///
/// Phase1 직후(schedule_flush 무장 시) `arm_readback_gate` 로 set, flush(또는 폐기)
/// 시 clear. OnEndEdit 는 `readback_gate_armed() && has_pending_insert()` 이중 가드
/// 통과 시에만 read-back 검증을 수행한다(무관 편집 무시 — atomic load 2회).
static READBACK_ARMED: AtomicBool = AtomicBool::new(false);

/// D3 read-back 게이트 기준선 — 삭제 적용 후 기대 커서 앞 글자 수.
///
/// `before_len - delete_chars`. read-back 으로 측정한 `cur_len <= expected` 이면 BS×N
/// 이 문서에 적용 완료된 것으로 보고 즉시 flush 한다. `-1` 이면 게이트 비활성
/// (before_len read-back 실패 → 타이머 단독 폴백 = 현행 b1 동작).
static EXPECTED_AFTER_DELETE: AtomicI32 = AtomicI32::new(-1);

/// 합성 이벤트가 이 시간 안에 모두 돌아오지 않으면 잔여 카운터를 버린다.
const STALE_MS: u128 = 2000;

fn key_event(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: EXTRA_INFO_MARK,
            },
        },
    }
}

/// b1 Phase1 — delete_chars 글자 삭제(VK_BACK)만 입력 큐에 주입한다.
///
/// **삽입(UNICODE)·센티널을 절대 함께 보내지 않는다.** 삽입은 Phase2 에서 TSF
/// edit session 으로 별도 수행한다. 같은 SendInput 배치에 BS 와 문자삽입을 섞지
/// 않으므로 Blink 의 비동기 재정렬(BS↔PACKET) 대상이 원천 소멸한다.
///
/// 주의: BS 1회 = 화면 글자 1개 삭제 가정 — ATF 대상(한글 음절·영문자)은
/// 모두 BMP 1코드유닛이라 성립한다. 서로게이트 쌍에는 쓰지 말 것.
///
/// 호출부: composition.rs replace_surrounding 이 ReplaceSurroundingEditSession
/// 의 ShiftStart 누적 이동량이 delete_chars 에 미달(CUAS/Blink — 확정 텍스트 뒤로
/// 역확장 거부)할 때 동적으로 폴백 호출한다. 앱 이름 휴리스틱이 아니라 실제
/// 이동량 부족으로만 분기하므로 정식 TSF 앱(메모장/Word)은 절대 이 경로를 타지
/// 않는다(회귀 0). SendInput 은 반드시 edit session 밖에서 호출해야 한다.
pub fn send_replacement_delete(delete_chars: u32) {
    if delete_chars == 0 {
        // 삭제가 없으면 BS 주입도 PENDING 도 불필요. (commit-only 0-delete 케이스)
        PENDING.store(0, Ordering::SeqCst);
        crate::register::dbg_log("synth_input: b1 BS phase del=0 (skip)");
        return;
    }
    let mut inputs: Vec<INPUT> = Vec::new();
    for _ in 0..delete_chars {
        inputs.push(key_event(VK_BACK.0, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_event(VK_BACK.0, 0, KEYEVENTF_KEYUP));
    }

    // ── PENDING = sink 로 되돌아오는 합성 keydown 수(BS 만) ──
    // UNICODE/센티널을 안 보내므로 PACKET(0xE7) 복귀 자체가 없고, "BS↔PACKET 재정렬"
    // 문제가 구조적으로 소멸한다. 일부 필드는 BS 도 sink 로 echo 하지 않는데(직행),
    // 그 경우 PENDING 은 STALE_MS 후 폐기되므로 사용자 BS 오인 창이 제한된다.
    let pending_downs = delete_chars as i32;
    PENDING.store(pending_downs, Ordering::SeqCst);
    *SEND_INSTANT.lock().unwrap() = Some(Instant::now());

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    crate::register::dbg_log(&format!(
        "synth_input: b1 BS phase bs={} pending={} sent={}/{}",
        delete_chars,
        pending_downs,
        sent,
        inputs.len()
    ));
    if (sent as usize) < inputs.len() {
        // 부분 실패(UIPI 등): 주입이 앞에서부터 잘리므로 도착하지 않는 BS keydown 수를
        // 보정한다. BS 는 입력열 맨 앞(2개/글자)이라 sent 로 도착한 BS down 수만 센다.
        let bs_inputs = (delete_chars as usize) * 2;
        let lost_bs_downs = if sent as usize >= bs_inputs {
            0
        } else {
            ((bs_inputs - sent as usize).div_ceil(2)) as i32
        };
        PENDING.fetch_sub(lost_bs_downs, Ordering::SeqCst);
    }
}

/// b1 Phase2 보류 삽입 슬롯에 (확정문, 마지막 음절)을 적재한다 (Phase1 호출).
pub fn store_pending_insert(commit_text: &str, last_syllable: &str) {
    *PENDING_INSERT.lock().unwrap() = Some(PendingInsert {
        commit_text: commit_text.to_string(),
        last_syllable: last_syllable.to_string(),
    });
    crate::register::dbg_log(&format!(
        "synth_input: b1 store_pending_insert commit_len={} last_len={}",
        commit_text.chars().count(),
        last_syllable.chars().count()
    ));
}

/// b1 Phase2 — 보류 삽입 슬롯을 꺼낸다 (1회성, take). 타이머·race-flush 양쪽에서 호출.
pub fn take_pending_insert() -> Option<PendingInsert> {
    PENDING_INSERT.lock().unwrap().take()
}

/// b1 — 보류 삽입이 남아있는지(가볍게) 확인. race-flush 가드 진입 판정용.
pub fn has_pending_insert() -> bool {
    PENDING_INSERT
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

/// b1 — 포커스 전환/Deactivate 시 보류 삽입을 폐기한다(stale 삽입 방지).
/// 반환: 폐기된 삽입이 있었으면 true.
pub fn discard_pending_insert() -> bool {
    let had = PENDING_INSERT.lock().unwrap().take().is_some();
    PENDING.store(0, Ordering::SeqCst);
    disarm_readback_gate();
    had
}

/// D3 — read-back 게이트를 무장한다(Phase1 직후, expected 기준선과 함께).
///
/// `expected`: `before_len - delete_chars`(읽기 실패면 `-1` → 게이트 비활성). 이후
/// OnEndEdit 가 `cur_len <= expected` 를 검증해 통과하면 즉시 flush 를 트리거한다.
pub fn arm_readback_gate(expected: i32) {
    EXPECTED_AFTER_DELETE.store(expected, Ordering::SeqCst);
    // expected<0 이면 게이트 자체를 켜지 않는다(타이머 단독 폴백).
    READBACK_ARMED.store(expected >= 0, Ordering::SeqCst);
}

/// D3 — read-back 게이트를 해제한다(flush·폐기·포커스 전환 시).
pub fn disarm_readback_gate() {
    READBACK_ARMED.store(false, Ordering::SeqCst);
    EXPECTED_AFTER_DELETE.store(-1, Ordering::SeqCst);
}

/// D3 — read-back 게이트가 무장됐는지(가볍게) 확인. OnEndEdit 가드 진입 판정용.
pub fn readback_gate_armed() -> bool {
    READBACK_ARMED.load(Ordering::SeqCst)
}

/// D3 — OnEndEdit read-back 검증: 측정한 커서 앞 글자 수가 기대값 이하인가.
///
/// `cur_len <= expected_after_delete` 면 BS×N 이 문서에 적용된 것으로 보고 true.
/// 게이트 비활성(expected<0)이면 항상 false(타이머에 위임). 부분 삭제(조기 OnEndEdit)
/// 면 false → 다음 OnEndEdit/타이머 대기(설계 약점 1 방어).
pub fn readback_delete_applied(cur_len: u32) -> bool {
    let expected = EXPECTED_AFTER_DELETE.load(Ordering::SeqCst);
    expected >= 0 && (cur_len as i64) <= (expected as i64)
}

fn pending_active() -> bool {
    if PENDING.load(Ordering::SeqCst) <= 0 {
        return false;
    }
    // 유실로 남은 잔여 카운터는 일정 시간 후 폐기 (사용자 BS 오인 방지).
    let stale = SEND_INSTANT
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis() > STALE_MS)
        .unwrap_or(true);
    if stale {
        PENDING.store(0, Ordering::SeqCst);
        return false;
    }
    true
}

/// OnTestKeyDown 최상단에서 호출. 합성 키면 Some(eaten) 반환 —
/// 호출자는 그 값을 그대로 반환하고 엔진 처리를 생략해야 한다.
pub fn observe_test_key_down(vk: u16) -> Option<bool> {
    // 새 keydown 이벤트마다 직전 플래그는 무효 (사용자 키 오인 방지).
    LAST_WAS_SYNTH.store(false, Ordering::SeqCst);
    if !pending_active() {
        return None;
    }
    // b1: 합성 BS 만 카운트한다. UNICODE/센티널을 더는 보내지 않으므로 PACKET 복귀는
    // 발생하지 않지만, 일부 환경이 UNICODE 를 sink 로 돌려보내도 음수로 떨어지지
    // 않도록(PENDING>0 게이트 위) VK_PACKET 분기는 방어적으로 유지한다.
    if vk == VK_BACK.0 || vk == VK_PACKET.0 {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        LAST_WAS_SYNTH.store(true, Ordering::SeqCst);
        // 통과 → 앱이 직접 삭제 처리.
        return Some(false);
    }
    // 합성 시퀀스 사이에 낀 사용자 키 — 정상 처리 위임.
    None
}

/// OnKeyDown 최상단에서 호출했을 때의 처리 지시.
pub enum SynthKeyAction {
    /// 합성 BS 키가 OnKeyDown 까지 온 경우 (일부 앱) — 엔진 생략, eaten=false.
    PassThrough,
}

/// OnKeyDown 최상단에서 호출. None 이면 일반 키 처리 계속.
pub fn observe_key_down(vk: u16) -> Option<SynthKeyAction> {
    if LAST_WAS_SYNTH.swap(false, Ordering::SeqCst) && (vk == VK_BACK.0 || vk == VK_PACKET.0) {
        return Some(SynthKeyAction::PassThrough);
    }
    None
}
