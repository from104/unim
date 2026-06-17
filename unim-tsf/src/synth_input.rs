//! AutoTypeFix CUAS 폴백 — 합성 키 주입 (SendInput).
//!
//! CUAS(IMM32 브리지) 앱의 가상 문서는 이미 확정(commit)된 텍스트를 ITfRange 로
//! 노출하지 않아 ShiftStart 가 요청량만큼 이동하지 못한다(shifted 부족).
//! 그 경우 범위 편집 대신 이 모듈이 VK_BACK × delete_chars 와 교정 텍스트
//! (KEYEVENTF_UNICODE)를 입력 큐에 주입한다 — espanso/AutoHotkey 가 임의 앱에서
//! 검증한 유일한 보편 경로.
//!
//! 재진입 차단: 주입한 키는 앱 메시지 루프를 거쳐 우리 자신의
//! ITfKeyEventSink(OnTestKeyDown/OnKeyDown)로 되돌아온다. dwExtraInfo 는 TSF
//! 콜백까지 전파가 보장되지 않으므로, 대신 "남은 합성 keydown 수" 카운터로
//! 식별해 엔진을 거치지 않고 통과시킨다.
//!
//! preedit 재시작 순서: replay preedit 은 composition 으로 시작해야 하는데,
//! edit session 안에서 즉시 시작하면 큐에 남은 백스페이스가 composition 활성
//! 상태에서 처리되어 동작이 불안정하다. 그래서 주입 시퀀스 맨 뒤에 센티널 키
//! (VK_F24)를 붙이고, 센티널이 sink 에 도착한 시점(= 큐 순서상 앞의 모든
//! 삭제·삽입이 앱에 전달 완료된 뒤)에 composition 을 시작한다.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_F24, VK_PACKET,
};

/// 디버깅용 마커 (식별은 카운터로 하므로 필수 아님).
const EXTRA_INFO_MARK: usize = 0x554E_494D; // "UNIM"

/// 센티널: 앱이 무시하는 키 중 충돌 가능성이 가장 낮은 F24.
const SENTINEL_VK: u16 = VK_F24.0;

/// 아직 sink 에 도착하지 않은 합성 keydown 수.
static PENDING: AtomicI32 = AtomicI32::new(0);
/// 직전 OnTestKeyDown 이 합성 키였음 (OnKeyDown 까지 호출하는 앱 대비).
static LAST_WAS_SYNTH: AtomicBool = AtomicBool::new(false);
/// 센티널 keydown 이 test 를 통과해 OnKeyDown 처리를 기다리는 중.
static SENTINEL_ARMED: AtomicBool = AtomicBool::new(false);
/// 주입 시각 — 일부 이벤트가 유실돼 카운터가 남는 경우의 부패(stale) 방지.
static SEND_INSTANT: Mutex<Option<Instant>> = Mutex::new(None);
/// 센티널 도착 시 시작할 replay preedit.
static PENDING_PREEDIT: Mutex<Option<String>> = Mutex::new(None);

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

/// delete_chars 글자 삭제(VK_BACK) + commit_text 삽입(KEYEVENTF_UNICODE)을
/// 입력 큐에 주입한다. preedit 이 비어있지 않으면 센티널 키를 덧붙이고
/// 보관해 두었다가, 센티널이 sink 에 도착할 때 호출자가 composition 을
/// 시작한다 (`take_action` 의 `StartPreedit`).
///
/// 주의: BS 1회 = 화면 글자 1개 삭제 가정 — ATF 대상(한글 음절·영문자)은
/// 모두 BMP 1코드유닛이라 성립한다. 서로게이트 쌍에는 쓰지 말 것.
///
/// 현재 호출부 없음: is_cuas SendInput 비동기 폴백(정식 TSF 앱 Word 오분류
/// 회귀)을 revert 하면서 유일한 호출자가 제거됐다. 후속 IMM32/HKL 경로
/// 작업에서 재사용 예정이라 보존한다.
#[allow(dead_code)]
pub fn send_replacement(delete_chars: u32, commit_text: &str, preedit_text: &str) {
    let mut inputs: Vec<INPUT> = Vec::new();
    for _ in 0..delete_chars {
        inputs.push(key_event(VK_BACK.0, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_event(VK_BACK.0, 0, KEYEVENTF_KEYUP));
    }
    for unit in commit_text.encode_utf16() {
        inputs.push(key_event(0, unit, KEYEVENTF_UNICODE));
        inputs.push(key_event(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    let has_preedit = !preedit_text.is_empty();
    if has_preedit {
        inputs.push(key_event(SENTINEL_VK, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_event(SENTINEL_VK, 0, KEYEVENTF_KEYUP));
    }
    if inputs.is_empty() {
        return;
    }

    // keydown 수 = 전체의 절반 (down/up 쌍).
    let downs = (inputs.len() / 2) as i32;
    PENDING.store(downs, Ordering::SeqCst);
    SENTINEL_ARMED.store(false, Ordering::SeqCst);
    *SEND_INSTANT.lock().unwrap() = Some(Instant::now());
    *PENDING_PREEDIT.lock().unwrap() = if has_preedit {
        Some(preedit_text.to_string())
    } else {
        None
    };

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    crate::register::dbg_log(&format!(
        "synth_input: bs={} commit_units={} sentinel={} sent={}/{}",
        delete_chars,
        commit_text.encode_utf16().count(),
        has_preedit,
        sent,
        inputs.len()
    ));
    if (sent as usize) < inputs.len() {
        // 부분 실패(UIPI 등) — 도착하지 않을 keydown 만큼 카운터 보정.
        let lost_downs = ((inputs.len() - sent as usize) / 2) as i32;
        PENDING.fetch_sub(lost_downs, Ordering::SeqCst);
    }
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
        SENTINEL_ARMED.store(false, Ordering::SeqCst);
        *PENDING_PREEDIT.lock().unwrap() = None;
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
    if vk == VK_BACK.0 || vk == VK_PACKET.0 {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        LAST_WAS_SYNTH.store(true, Ordering::SeqCst);
        // 통과 → 앱이 직접 삭제/삽입 처리.
        return Some(false);
    }
    if vk == SENTINEL_VK {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        SENTINEL_ARMED.store(true, Ordering::SeqCst);
        // 소비 → OnKeyDown 에서 StartPreedit 수행, 앱에는 전달 안 됨.
        return Some(true);
    }
    // 합성 시퀀스 사이에 낀 사용자 키 — 정상 처리 위임.
    None
}

/// OnKeyDown 최상단에서 호출했을 때의 처리 지시.
pub enum SynthKeyAction {
    /// 합성 BS/문자 키가 OnKeyDown 까지 온 경우 (일부 앱) — 엔진 생략, eaten=false.
    PassThrough,
    /// 센티널 도착 — 보관된 preedit 으로 composition 을 시작하고 eaten=true.
    StartPreedit(String),
}

/// OnKeyDown 최상단에서 호출. None 이면 일반 키 처리 계속.
pub fn observe_key_down(vk: u16) -> Option<SynthKeyAction> {
    if vk == SENTINEL_VK && SENTINEL_ARMED.swap(false, Ordering::SeqCst) {
        let preedit = PENDING_PREEDIT.lock().unwrap().take();
        return Some(SynthKeyAction::StartPreedit(preedit.unwrap_or_default()));
    }
    if LAST_WAS_SYNTH.swap(false, Ordering::SeqCst) && (vk == VK_BACK.0 || vk == VK_PACKET.0) {
        return Some(SynthKeyAction::PassThrough);
    }
    None
}
