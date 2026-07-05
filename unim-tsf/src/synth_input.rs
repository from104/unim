//! AutoTypeFix CUAS 폴백 — 합성 키 주입 (SendInput).
//!
//! CUAS(IMM32 브리지)·Blink 앱의 가상 문서는 이미 확정(commit)된 텍스트를 ITfRange
//! 로 노출하지 않아 ShiftStart 가 요청량만큼 이동하지 못한다(shifted 부족 = full=false,
//! wezterm/telegram). 그 경우 범위 편집 대신 이 모듈이
//! `[BS×N + KEYEVENTF_UNICODE(교정문 전체)]` 를 **한 SendInput 배치**로 입력 큐에
//! 적재한다(`send_replacement_batch`).
//!
//! ## 단일배치 (삭제+삽입을 한 입력큐 FIFO)
//! 삭제(BS×N)와 삽입(교정문 전체 UNICODE)을 같은 SendInput 배치에 순차 적재하면
//! 입력큐 FIFO 로 [삭제 N → 삽입] 순서가 보장되어, 과거 b1(삭제=입력큐 BS /
//! 삽입=TSF edit session)의 **2채널 레이스**(readback 거짓양성·뒤늦은 BS 가 삽입
//! 글자를 먹음)가 구조적으로 소멸한다. 교정문 전체(commit+마지막음절)를 즉시 확정하므로
//! 라이브 마지막음절 조합은 포기한다(브리지앱은 어차피 조합을 terminate — 수용).
//!
//! 재진입 차단: 주입한 BS·UNICODE(VK_PACKET 0xE7) keydown 은 앱 메시지 루프를 거쳐
//! 우리 자신의 ITfKeyEventSink(OnTestKeyDown/OnKeyDown)로 echo 될 수 있다. dwExtraInfo
//! 는 TSF 콜백까지 전파가 보장되지 않으므로, 대신 "남은 합성 keydown 수"(BS·PACKET
//! 둘 다) 카운터(PENDING)로 식별해 엔진을 거치지 않고 통과시킨다.
//!
//! ⚠ 앱별 echo 비대칭: 대부분의 앱(wezterm/CUAS/메모장)은 합성 echo 를 OnTestKeyDown→
//! OnKeyDown 양쪽으로 흘려 `observe_test_key_down` 이 PENDING 을 감산하지만, **wmux/xterm.js
//! 등 Blink contenteditable 호스트는 OnTestKeyDown 을 아예 발화하지 않고 OnKeyDown 으로만**
//! echo 한다(실측 wmux OnTestKeyDown=0). 그래서 `observe_key_down` 이 OnTestKeyDown 페어링
//! (LAST_WAS_SYNTH) 없이도 합성 echo 를 직접 식별·감산하는 Case B 를 갖는다(없으면 race-flush
//! 오발로 꼬리가 깨졌다). 또한 이들 앱에선 합성 UNICODE(head)가 WM_CHAR 로만 처리돼 keydown
//! echo 가 없어 PENDING 이 head 길이(SYNTH_HEAD_RESIDUAL)에서 멈춘다(0 도달 불가). 그래서
//! "삭제 완료" 판정을 PENDING==0 이 아니라 PENDING<=head_units 로 하여, 60ms 안전망 타이머
//! 시점(머리 WM_CHAR 가 문서에 정착한 뒤)에 라이브 꼬리 조합(start_composition)으로 수렴한다.
//! start_composition 거부 시에만 degrade 확정으로 폴백한다(꼬리에 종성을 이어쳐도 결합 유지).
//!
//! TSF 삽입/readback/펌프-분할(`store_pending_insert`/`arm_readback_gate`/`insert_pending`)
//! 은 synth 경로에서 미사용 — native(full=true, chrome) 펌프-분할 전용으로 잔존한다.
//!
//! (참고) v0.3.31 의 `[BS+UNICODE]` 단일배치는 Blink(Chrome/WebView2)가 같은 배치의
//! BS 와 UNICODE 를 비동기로 재정렬/병합해 첫 한글 글자를 유실했었다. 그러나 지금
//! 크롬 주 입력면은 native(full=true)라 synth 를 타지 않으며, 주 타깃 wezterm
//! (conhost)·telegram(Qt)은 WM_KEYDOWN/WM_CHAR 큐를 동기 FIFO 처리해 재정렬이
//! 성립하지 않는다(잔여 노출면 = contenteditable 등 full=false 진입 표면 한정).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_PACKET,
};

/// 디버깅용 마커 (식별은 카운터로 하므로 필수 아님).
const EXTRA_INFO_MARK: usize = 0x554E_494D; // "UNIM"

/// 아직 sink 에 도착하지 않은 합성 keydown 수(BS + 머리 UNICODE PACKET echo).
static PENDING: AtomicI32 = AtomicI32::new(0);
/// head-tail 순방향 배치의 머리 UNICODE 유닛 수. conhost/Blink 은 머리 UNICODE 의
/// keydown(VK_PACKET) echo 를 흘리지 않아 BS echo 전량 복귀 후에도 PENDING 이 이 값에서
/// 멈춘다(0 도달 불가). 그래서 "삭제(BS) 적용 완료" 판정을 PENDING==0 이 아니라
/// PENDING<=이 값으로 해야 한다. 역방향(꼬리 없음) 배치에선 send_replacement_batch
/// 최상단이 0 으로 기저 리셋하고 flush_pending_tail 이 이 값을 아예 참조하지 않는다.
static SYNTH_HEAD_RESIDUAL: AtomicI32 = AtomicI32::new(0);
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

/// R6b — 보류 꼬리(마지막 음절) 슬롯. 순방향 synth 분기가 머리를 단일배치로 확정한 뒤
/// 꼬리를 여기 적재한다. WM_UNIM_TAIL 게이트(PENDING<=0, 머리 echo 있는 wezterm) 또는
/// 60ms 타이머/race-flush 가 take 해, BS 삭제 완료(PENDING<=head_residual) 시 라이브 조합을
/// 시도하고 미복귀(PENDING>residual) 시 degrade 확정한다.
static PENDING_TAIL: Mutex<Option<String>> = Mutex::new(None);
/// R6b — 라이브 꼬리 조합이 살아있는지. start_composition 성공 시 set, 사용자 키 관찰 시
/// (OnTestKeyDown) clear, terminate 가드/포커스 전환/Drop 에서 폐기.
static TAIL_LIVE: AtomicBool = AtomicBool::new(false);

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

/// synth 단일배치 — delete_chars 글자 삭제(VK_BACK) + 교정문 전체(KEYEVENTF_UNICODE)를
/// **한 SendInput 배치**로 입력큐에 적재한다. 입력큐 FIFO 로 [삭제 N → 삽입] 순서가
/// 보장되어, 과거 b1(삭제=입력큐 BS / 삽입=TSF edit session)의 2채널 레이스가 소멸한다.
///
/// 주의: BS 1회 = 화면 글자 1개 삭제 가정 — ATF 대상(한글 음절 AC00..D7A3·ASCII)은
/// 모두 BMP 1 코드유닛이라 글자수=UTF-16 코드유닛수가 성립한다(서로게이트 미대상).
///
/// 호출부: composition.rs replace_surrounding 이 (1) ReplaceSurroundingEditSession
/// 의 ShiftStart 누적 이동량이 delete_chars 에 미달(CUAS/Blink — 확정 텍스트 뒤로
/// 역확장 거부)하거나, (2) sink_asymmetric(OnTestKeyDown 미발화 앱 = wmux/xterm.js —
/// ShiftStart 는 phantom-성공하나 committed SetText 가 pty 화면에 렌더 안 됨) 일 때
/// 동적으로 폴백 호출한다. 둘 다 앱 이름 휴리스틱이 아니라 실측 신호(이동량 부족 /
/// TSF sink 발화 패턴)로 분기한다. 정식 TSF 앱(메모장/Word — OnTestKeyDown 발화 +
/// ShiftStart 성공)은 두 조건 모두 불충족이라 이 경로를 타지 않는다(회귀 0).
/// SendInput 은 반드시 edit session(COM lock) 밖에서 호출한다(호출부가 보장).
pub fn send_replacement_batch(delete_chars: u32, text: &str) {
    // A5: 머리 residual 기저 리셋(역방향 SynthBatch=꼬리 없음 케이스 안전). head-tail 은
    // 배치 뒤 composition.rs 가 set_head_residual 로 덮어쓴다. no-op 조기 return 보다 앞에 둔다.
    SYNTH_HEAD_RESIDUAL.store(0, Ordering::SeqCst);
    let mut inputs: Vec<INPUT> = Vec::new();
    // (1) 삭제: BS down/up × delete_chars (입력큐 선두)
    for _ in 0..delete_chars {
        inputs.push(key_event(VK_BACK.0, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_event(VK_BACK.0, 0, KEYEVENTF_KEYUP));
    }
    // (2) 삽입: 교정문 전체를 UTF-16 코드유닛 단위 UNICODE down/up (삭제 뒤에 적재)
    for cu in text.encode_utf16() {
        inputs.push(key_event(0, cu, KEYEVENTF_UNICODE));
        inputs.push(key_event(0, cu, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    if inputs.is_empty() {
        // del=0 && text 빈 → no-op
        PENDING.store(0, Ordering::SeqCst);
        crate::register::dbg_log("synth_input: batch no-op (del=0, text empty)");
        return;
    }
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    // PENDING = 실제 전송된 합성 keydown 수.
    //   모든 이벤트가 down/up 쌍 → 전송분 중 down 수 = ceil(sent/2).
    //   전송(full)이면 inputs.len()/2 = delete_chars + utf16_units.
    //   부분 전송(UIPI 등): SendInput 이 앞에서부터 채우므로 ceil(sent/2)가
    //   "도착한 down(=앞으로 echo 될 수)"과 정확히 일치 → 과집계 0.
    //   ※ STA 단일스레드: 우리가 handle_key_down 안에서 동기 실행 중이라
    //     메시지펌프가 안 돌고, 따라서 어떤 echo 도 이 store 이전에 도착 불가.
    PENDING.store((sent as usize).div_ceil(2) as i32, Ordering::SeqCst);
    *SEND_INSTANT.lock().unwrap() = Some(Instant::now());
    crate::register::dbg_log(&format!(
        "synth_input: batch del={} uni_units={} pending={} sent={}/{}",
        delete_chars,
        text.encode_utf16().count(),
        (sent as usize).div_ceil(2),
        sent,
        inputs.len()
    ));
}

/// b1 Phase2 보류 삽입 슬롯에 (확정문, 마지막 음절)을 적재한다 (Phase1 호출).
///
/// synth 단일배치 전환으로 현재 호출부가 없다(롤백 보험 + native 잔존 인프라 일관성).
#[allow(dead_code)]
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
    SYNTH_HEAD_RESIDUAL.store(0, Ordering::SeqCst); // 위생 — 다음 배치 리셋에만 의존 금지
    disarm_readback_gate();
    discard_pending_restart();
    had
}

/// b1[D] Phase2b 보류 재시작 슬롯 — (commit_text, last_syllable).
///
/// 방식 D 펌프-분할: Phase2a(세션A `start_composition(full)`)가 성공하면 이 슬롯에
/// (commit, tail)을 적재하고 PostMessage(WM_UNIM_FLUSH2)로 **메시지 펌프를 한 번 돌린
/// 뒤** Phase2b(세션B `commit_and_restart`)가 take 해 확정한다. 크롬(Blink)은 세션A↔
/// 세션B 사이에 실제 펌프가 없으면(같은 틱 연속 lock) 세션A 조합을 등록하지 못해
/// 세션B EndComposition 의 commit 이 정착하지 않는다(앞 확정문 유실) — 펌프가 그 정착
/// 경계를 만든다.
static PENDING_RESTART: Mutex<Option<(String, String)>> = Mutex::new(None);

/// b1[D] — Phase2b 재시작 슬롯 적재 (Phase2a 세션A 성공 직후).
pub fn store_pending_restart(commit_text: &str, last_syllable: &str) {
    *PENDING_RESTART.lock().unwrap() = Some((commit_text.to_string(), last_syllable.to_string()));
}

/// b1[D] — Phase2b 재시작 슬롯을 꺼낸다 (1회성, take).
pub fn take_pending_restart() -> Option<(String, String)> {
    PENDING_RESTART.lock().unwrap().take()
}

/// b1[D] — Phase2b 재시작이 보류 중인지 확인 (WM_UNIM_FLUSH2 포스팅 판정용).
pub fn has_pending_restart() -> bool {
    PENDING_RESTART
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

/// b1[D] — 보류 재시작 폐기 (stale 방지; discard_pending_insert·Drop 에서 호출).
pub fn discard_pending_restart() -> bool {
    PENDING_RESTART.lock().unwrap().take().is_some()
}

/// D3 — read-back 게이트를 무장한다(Phase1 직후, expected 기준선과 함께).
///
/// `expected`: `before_len - delete_chars`(읽기 실패면 `-1` → 게이트 비활성). 이후
/// OnEndEdit 가 `cur_len <= expected` 를 검증해 통과하면 즉시 flush 를 트리거한다.
///
/// synth 단일배치 전환으로 현재 호출부가 없다(롤백 보험 + native 잔존 인프라 일관성).
#[allow(dead_code)]
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
    // 단일배치: UNICODE 주입분이 VK_PACKET(0xE7) keydown 으로 sink 에 echo 되므로
    // BS·PACKET 둘 다 정상 카운트한다(PENDING = 한 배치의 합성 down 총수). PENDING>0
    // 게이트 위에서만 fetch_sub 하므로 음수로 떨어지지 않는다.
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
    /// 합성 BS/UNICODE 키가 OnKeyDown 으로 도착한 경우 — 엔진 생략, eaten=false(앱 직접 삭제).
    PassThrough,
}

/// OnKeyDown 최상단에서 호출. None 이면 일반 키 처리 계속.
///
/// 두 경로로 합성 echo 를 식별한다:
/// - **Case A (OnTestKeyDown 발화 앱: wezterm/CUAS/메모장 등)**: 직전 OnTestKeyDown
///   (observe_test_key_down)이 이 합성 키를 이미 식별·카운트(PENDING 감산)하며
///   LAST_WAS_SYNTH 를 set 했다 → 페어링으로 통과(여기선 재감산하지 않는다).
/// - **Case B (OnTestKeyDown 미발화 앱: wmux/xterm.js 등 Blink contenteditable)**: 해당 TSF
///   는 OnTestKeyDown 을 호출하지 않아(실측 wmux=0회) 합성 BS/UNICODE echo 가 OnKeyDown 으로만
///   도착한다. LAST_WAS_SYNTH 가 set 되지 않으므로, 진행 중 합성 배치(pending_active)+합성
///   vk(BS/PACKET)로 **직접** 식별해 PENDING 을 감산하고 통과시킨다. 이게 없으면 합성 BS echo
///   가 아래 race-flush 가드(text_service OnKeyDown)로 흘러 "사용자 다음 키"로 오분류되어,
///   머리 echo 미복귀(PENDING>0) 중 꼬리가 조기 degrade 되고 뒤이어 도착하는 나머지 합성 BS
///   가 방금 삽입한 꼬리를 다시 지워 출력이 깨졌다(wmux 한글 자동교정 결함).
///
/// Case A 가 LAST_WAS_SYNTH 를 swap 으로 소비(true→false)하므로 OnTestKeyDown 발화 앱은
/// Case B 에 진입하지 않는다 → PENDING 이중감산 없음. pending_active 는 STALE_MS 경과분을
/// 폐기하므로, 합성 배치가 없을 때의 사용자 실제 BS 는 Case B 를 타지 않는다.
pub fn observe_key_down(vk: u16) -> Option<SynthKeyAction> {
    // Case A — OnTestKeyDown 페어링(재감산 금지).
    if LAST_WAS_SYNTH.swap(false, Ordering::SeqCst) && (vk == VK_BACK.0 || vk == VK_PACKET.0) {
        return Some(SynthKeyAction::PassThrough);
    }
    // Case B — OnKeyDown 단독 echo (OnTestKeyDown 미발화 앱). pending_active 게이트 위에서만
    // 감산하므로 음수로 떨어지지 않는다.
    //
    // ⚠ 트레이드오프(LOW): 합성 배치 진행 중(PENDING>0, STALE_MS 이내)에 사용자가 누른 진짜
    // Backspace 는 여기서 식별돼 통과(소비)된다 — provenance 가 아니라 vk+카운터로 식별하기
    // 때문. 노출은 synth/full=false 브리지 앱(wmux 등)에 한정되고(정식 TSF 앱은 PENDING 이
    // 0 이라 미진입), 이들 앱에선 UNICODE head 가 keydown echo 가 없어 PENDING 이 최대 STALE_MS
    // 까지 잔존할 수 있다. BS 는 eaten=FALSE 로 앱에 그대로 전달돼 삭제 자체는 일어나며, 잔여
    // 카운터는 STALE 폴백으로 닫힌다(수용 가능 — 적대검증 #2).
    if (vk == VK_BACK.0 || vk == VK_PACKET.0) && pending_active() {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        return Some(SynthKeyAction::PassThrough);
    }
    None
}

// ════════════════════════════════════════════════════════════════════════════
// R6b — 브리지앱 synth 순방향 "마지막 음절 라이브 미확정 조합(preedit)" 슬롯/플래그
// ════════════════════════════════════════════════════════════════════════════

/// R6b — 순방향 synth 분기가 머리(commit)를 단일배치로 확정한 직후 꼬리를 적재한다.
pub fn store_pending_tail(tail: &str) {
    *PENDING_TAIL.lock().unwrap() = Some(tail.to_string());
    crate::register::dbg_log_ev!(
        &format!("synth: store_pending_tail len={}", tail.chars().count()),
        "synth: store_pending_tail '{tail}'"
    );
}

/// R6b — 보류 꼬리를 1회성으로 꺼낸다(flush 진입점). 게이트/타이머/race/동기 degrade 공용.
pub fn take_pending_tail() -> Option<String> {
    PENDING_TAIL.lock().unwrap().take()
}

/// R6b — 보류 꼬리가 남아있는지(가볍게) 확인. 게이트·가드 진입 판정용.
pub fn has_pending_tail() -> bool {
    PENDING_TAIL.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// R6b — 보류 꼬리 슬롯을 폐기하고 TAIL_LIVE 도 false 로 만든다(포커스 전환·Drop·
/// terminate 가드·no-context). 반환: 폐기된 꼬리가 있었으면 true. PENDING 카운터는
/// 건드리지 않는다(머리 echo 추적과 분리 — 포커스 전환 PENDING 리셋은 함께 호출되는
/// discard_pending_insert 가 수행).
pub fn discard_pending_tail() -> bool {
    let had = PENDING_TAIL.lock().unwrap().take().is_some();
    TAIL_LIVE.store(false, Ordering::SeqCst);
    had
}

/// R6b — 라이브 꼬리 조합이 성립했음(start_composition 성공).
pub fn set_tail_live() {
    TAIL_LIVE.store(true, Ordering::SeqCst);
}

/// R6b — 사용자 키 관찰로 꼬리 adopt/종료 시 플래그 해제(M-3 — OnTestKeyDown 실제 키).
pub fn clear_tail_live() {
    TAIL_LIVE.store(false, Ordering::SeqCst);
}

/// R6b — terminate 가드 판정용. 라이브 꼬리 조합이 살아있는가.
pub fn tail_live() -> bool {
    TAIL_LIVE.load(Ordering::SeqCst)
}

/// R6b — OnTestKeyDown 게이트: 머리 배치 echo 전량 복귀(PENDING<=0) + 보류 꼬리 존재.
/// (echo 직후 호출이라 pending_active 의 stale 보정 불필요.)
pub fn tail_gate_ready() -> bool {
    PENDING.load(Ordering::SeqCst) <= 0 && has_pending_tail()
}

/// R6b — 머리 echo 가 아직 복귀 중인가(PENDING>0, stale 보정 포함)의 pub 래퍼.
/// deletes_still_pending 으로 대체됨(head_residual 임계치 미반영이라 conhost/Blink 머리
/// no-echo 케이스에서 영영 참) — 롤백 보험으로 잔존.
#[allow(dead_code)]
pub fn pending_echo_active() -> bool {
    pending_active()
}

/// R6b — head-tail 순방향 머리 UNICODE 유닛 수를 기록한다(composition.rs 가 배치 직후 호출).
/// 이 값이 "삭제 완료" 판정 임계치가 된다(PENDING<=이 값 → BS 드레인 완료).
pub fn set_head_residual(units: i32) {
    SYNTH_HEAD_RESIDUAL.store(units, Ordering::SeqCst);
}

/// R6b — BS 삭제 echo 가 아직 복귀 중인가(라이브 불가) 판정. PENDING 이 머리 residual 보다
/// 크면 아직 삭제 BS 가 큐/비행 중이라는 뜻 → degrade. 머리 residual 만 남았으면
/// (PENDING<=residual, conhost/Blink 는 머리 no-echo 로 여기서 정체) 삭제 완료로 보고
/// 라이브 조합을 시도한다. pending_active() 의 stale 폴백을 그대로 이용해, 합성 배치가
/// 없으면 거짓 → 정식 TSF 앱(PENDING=0)은 미진입.
pub fn deletes_still_pending() -> bool {
    pending_active() && PENDING.load(Ordering::SeqCst) > SYNTH_HEAD_RESIDUAL.load(Ordering::SeqCst)
}

/// R6b — 라이브 성립 로그용 (PENDING, SYNTH_HEAD_RESIDUAL) 스냅샷. device-QA 에서
/// conhost/Blink(pending==residual>0)와 wezterm(pending<=0) 경로를 로그만으로 구분한다.
pub fn synth_echo_state() -> (i32, i32) {
    (
        PENDING.load(Ordering::SeqCst),
        SYNTH_HEAD_RESIDUAL.load(Ordering::SeqCst),
    )
}

/// R6b — 라이브 꼬리 조합 성립 시 호출 — 합성 echo 회계를 정리한다. 머리 UNICODE 는
/// conhost/Blink 에서 keydown echo 가 없어 PENDING 이 residual 만큼 STALE_MS 까지 잔존하는데,
/// 그 창의 사용자 실제 BS 오분류(observe_key_down Case B)와 뒤늦은 stray echo 를 막기 위해
/// 카운터를 즉시 0 으로 닫는다. conhost/Blink 전용 경로라 안전하다(wezterm 은 머리 echo 로
/// PENDING 이 0 에 도달해 WM_UNIM_TAIL 로 처리되므로 라이브 분기의 이 clear 에 도달하되
/// PENDING 은 이미 0 이라 무해).
pub fn clear_synth_echo() {
    PENDING.store(0, Ordering::SeqCst);
    SYNTH_HEAD_RESIDUAL.store(0, Ordering::SeqCst);
    *SEND_INSTANT.lock().unwrap() = None;
}

// ════════════════════════════════════════════════════════════════════════════
// sink 비대칭 감지 — OnTestKeyDown 미발화 앱(wmux/xterm.js 등 Blink 터미널)
// ════════════════════════════════════════════════════════════════════════════
//
// wmux(xterm.js)/일부 Blink 는 OnTestKeyDown 을 호출하지 않고 OnKeyDown 만 발화한다
// (Case B — observe_key_down 주석 참조). 이런 앱은 committed(확정) 텍스트를 TSF 로
// 편집(ReplaceSurrounding 의 ShiftStart+SetText)해도 pty 화면에 렌더링되지 않는다:
// ShiftStart 는 TSF 가상문서에서 phantom-성공(shifted=-N)하지만 실제 화면은 안 바뀐다.
// 그래서 역방향(한→영) 교정이 네이티브 경로로 빠져 화면이 비일관 깨진다
// ("ㄹㅊㅊ"→"ㄹㅊo"). 순방향은 삭제 대상이 실제 타이핑된 pty 영문이라 ShiftStart 가
// 진짜 실패→synth 로 우회돼 정상 동작한다. 이 신호로 그런 앱을 감지해 committed 삭제를
// synth SendInput(순방향이 이미 쓰는 경로)으로 강제 라우팅한다.
//
// 신호: 현재 포커스에서 OnTestKeyDown 이 한 번도 안 불렸고(test==0) 실제 사용자 키다운이
// 2회 이상(kd>=2). 정상 TSF 앱(메모장/Word/wezterm)은 매 키 OnTestKeyDown 을 발화하므로
// test>0 → 영구 false → 네이티브 경로 무변경. cold-start 없음(교정 대상 한글 타이핑
// 자체가 kd 를 채우므로 선행 순방향 교정 불필요). synth echo 는 kd 로 세지 않는다(회계
// 오염 방지 — 호출부가 observe_key_down 통과분에서만 note_user_key_down).
static SINK_TEST_KD: AtomicU32 = AtomicU32::new(0);
static SINK_USER_KD: AtomicU32 = AtomicU32::new(0);

/// OnTestKeyDown 진입 시(합성/사용자 무관) 호출 — 이 앱이 OnTestKeyDown 을 발화함을 기록.
pub fn note_test_key_down() {
    SINK_TEST_KD.fetch_add(1, Ordering::SeqCst);
}

/// OnKeyDown 에서 **실제 사용자 키**(observe_key_down 통과)일 때만 호출.
pub fn note_user_key_down() {
    SINK_USER_KD.fetch_add(1, Ordering::SeqCst);
}

/// 포커스 전환 시 카운터 리셋 — 앱마다 재감지(창 재생성·앱 전환 대응).
pub fn reset_sink_counters() {
    SINK_TEST_KD.store(0, Ordering::SeqCst);
    SINK_USER_KD.store(0, Ordering::SeqCst);
}

/// OnTestKeyDown 미발화(test==0) + 사용자 키다운 2회 이상(kd>=2) → committed 삭제를
/// synth 로 강제 라우팅해야 하는 앱(wmux/Blink 터미널). 로그용 (test, kd) 도 반환.
pub fn sink_asymmetric() -> bool {
    SINK_TEST_KD.load(Ordering::SeqCst) == 0 && SINK_USER_KD.load(Ordering::SeqCst) >= 2
}

/// 로그용 (test_kd, user_kd) 스냅샷.
pub fn sink_counters() -> (u32, u32) {
    (
        SINK_TEST_KD.load(Ordering::SeqCst),
        SINK_USER_KD.load(Ordering::SeqCst),
    )
}

/// R6b degrade 전용 — 꼬리를 UNICODE 단일배치로 입력큐에 적재하되 PENDING 을 **덮어쓰지
/// 않고 누적(fetch_add)** 한다(M-1). 머리 echo 미복귀(PENDING>0) 중 호출돼도 잔여 머리
/// echo 수를 잃지 않아 echo 오분류가 없다. 입력큐 FIFO 로 꼬리는 머리 뒤에 적재돼
/// 위치도 정확하다. SendInput 은 반드시 edit session(COM lock) 밖에서 호출(호출부 보장).
pub fn append_tail_batch(text: &str) {
    let mut inputs: Vec<INPUT> = Vec::new();
    for cu in text.encode_utf16() {
        inputs.push(key_event(0, cu, KEYEVENTF_UNICODE));
        inputs.push(key_event(0, cu, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    if inputs.is_empty() {
        return;
    }
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    PENDING.fetch_add((sent as usize).div_ceil(2) as i32, Ordering::SeqCst);
    *SEND_INSTANT.lock().unwrap() = Some(Instant::now());
    crate::register::dbg_log_ev!(
        &format!(
            "synth: append_tail_batch len={} (+{} pending now={})",
            text.chars().count(),
            (sent as usize).div_ceil(2),
            PENDING.load(Ordering::SeqCst)
        ),
        "synth: append_tail_batch '{text}' (+{} pending now={})",
        (sent as usize).div_ceil(2),
        PENDING.load(Ordering::SeqCst)
    );
}
