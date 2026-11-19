//! 한자 재변환 — `WM_IME_REQUEST`/`IMR_RECONVERTSTRING` 으로 커서 단어 읽기
//! (HANJA_WORD_SPEC §2.4c).
//!
//! CUAS 로 TSF 를 쓰는 IMM32 앱(Telegram 등 Qt)은 TSF 문서에 조합 글자만 보여 조합이 없을
//! 때 커서 앞 텍스트를 읽을 수 없다. 이런 앱 다수는 IMM32 재변환 요청에 답한다 — 문단
//! 텍스트와 커서 단어 구간(comp)을 돌려준다. Qt 는 답하면서 **그 단어를 선택**한다(부작용,
//! 2026-11-17 VM 실측). 선택은 확정 문자열이 대체하므로 대상② 선택 변환으로 이어 쓴다.
//!
//! 키 처리 락(engine·composition)을 잡기 **전에** 호출한다 — `SendMessage` 가 앱 코드로
//! 곧장 들어가고, 앱의 선택 변경이 TSF sink 로 되돌아올 수 있다(재진입).
//! 사용자 평문은 로그에 남기지 않는다(§2.8 — 길이만).

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

const WM_IME_REQUEST: u32 = 0x0288;
const IMR_RECONVERTSTRING: usize = 0x0004;
/// 응답 크기 상한 — 비정상 응답 방어(문단 하나 기준으로 넉넉하게).
const MAX_BYTES: usize = 1 << 20;

/// `RECONVERTSTRING` (imm.h) — 8 × DWORD.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct ReconvertString {
    dw_size: u32,
    dw_version: u32,
    dw_str_len: u32,
    dw_str_offset: u32,
    dw_comp_str_len: u32,
    dw_comp_str_offset: u32,
    dw_target_str_len: u32,
    dw_target_str_offset: u32,
}

/// 재변환 결과 — `text` 는 앱이 준 문단, `[start, end)` 는 앱이 고른(그리고 선택한) 단어
/// 구간. 오프셋은 **char** 단위(엔진 surrounding 단위와 같다).
pub struct ReconvWord {
    pub text: String,
    pub start: u32,
    pub end: u32,
}

/// 포커스 창에 재변환 요청을 보내 커서 단어를 받는다. 앱이 답하지 않거나 응답이 어긋나면
/// `None` — 이때 앱은 선택을 바꾸지 않았다(크기 질의는 부작용이 없고, 본 요청 실패는 0).
pub fn query(hwnd: HWND) -> Option<ReconvWord> {
    if hwnd.0.is_null() {
        return None;
    }
    let hdr = std::mem::size_of::<ReconvertString>();
    // 1) 크기 질의(lParam=NULL) — 부작용 없음.
    let size = unsafe {
        SendMessageW(
            hwnd,
            WM_IME_REQUEST,
            Some(WPARAM(IMR_RECONVERTSTRING)),
            Some(LPARAM(0)),
        )
    }
    .0;
    if size <= 0 || (size as usize) < hdr || (size as usize) > MAX_BYTES {
        return None;
    }
    let size = size as usize;
    // 2) 본 요청 — u64 정렬 버퍼(헤더 DWORD 정렬 보장).
    let mut buf = vec![0u64; size.div_ceil(8)];
    let base = buf.as_mut_ptr() as *mut u8;
    unsafe {
        (base as *mut ReconvertString).write(ReconvertString {
            dw_size: size as u32,
            ..Default::default()
        });
    }
    let r = unsafe {
        SendMessageW(
            hwnd,
            WM_IME_REQUEST,
            Some(WPARAM(IMR_RECONVERTSTRING)),
            Some(LPARAM(base as isize)),
        )
    }
    .0;
    if r <= 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(base as *const u8, size) };
    parse(bytes)
}

/// Qt 최상위 창(클래스 `Qt5QWindowIcon`·`Qt6112QWindowIcon` 등)인지. Qt 위젯은 창 핸들이
/// 없어 포커스 창이 곧 최상위 QWindow 다.
pub fn is_qt_window(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    if hwnd.0.is_null() {
        return false;
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    if n <= 0 {
        return false;
    }
    is_qt_class(&String::from_utf16_lossy(&buf[..n as usize]))
}

fn is_qt_class(class: &str) -> bool {
    class.starts_with("Qt") && class.contains("QWindow")
}

/// 응답 버퍼를 해석한다(경계 검사 포함). 순수 함수.
fn parse(bytes: &[u8]) -> Option<ReconvWord> {
    let hdr = std::mem::size_of::<ReconvertString>();
    if bytes.len() < hdr {
        return None;
    }
    let dw = |i: usize| {
        u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]) as usize
    };
    let (str_len, str_off, comp_len, comp_off) = (dw(2), dw(3), dw(4), dw(5));
    // dwStrOffset 은 구조체 시작 기준 바이트, dwCompStrOffset 은 문자열 시작 기준 바이트.
    if str_len == 0 || str_off < hdr || str_off % 2 != 0 || comp_off % 2 != 0 {
        return None;
    }
    let str_end = str_off.checked_add(str_len.checked_mul(2)?)?;
    let comp_start = comp_off / 2;
    let comp_end = comp_start.checked_add(comp_len)?;
    if str_end > bytes.len() || comp_end > str_len || comp_len == 0 {
        return None;
    }
    let units: Vec<u16> = bytes[str_off..str_end]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16(&units).ok()?;
    // UTF-16 유닛 오프셋 → char 오프셋(서로게이트 쌍 = 1 char). 쌍 중간을 가리키면 거절.
    let to_char = |u: usize| -> Option<u32> {
        if u > 0 && u < units.len() && (0xDC00..0xE000).contains(&units[u]) {
            return None;
        }
        Some(char::decode_utf16(units[..u].iter().copied()).count() as u32)
    };
    Some(ReconvWord {
        start: to_char(comp_start)?,
        end: to_char(comp_end)?,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(text: &str, comp_start_u16: usize, comp_len_u16: usize) -> Vec<u8> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut v = Vec::new();
        let size = 32 + units.len() * 2;
        for d in [
            size,
            0,
            units.len(),
            32,
            comp_len_u16,
            comp_start_u16 * 2,
            comp_len_u16,
            comp_start_u16 * 2,
        ] {
            v.extend_from_slice(&(d as u32).to_le_bytes());
        }
        for u in units {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v
    }

    #[test]
    fn parses_word_bounds_in_chars() {
        let w = parse(&build("우리 학교", 3, 2)).unwrap();
        assert_eq!((w.text.as_str(), w.start, w.end), ("우리 학교", 3, 5));
    }

    #[test]
    fn converts_surrogate_offsets_to_chars() {
        // "😀 학교" — 이모지 2유닛, 단어는 유닛 3..5 → char 2..4.
        let w = parse(&build("😀 학교", 3, 2)).unwrap();
        assert_eq!((w.start, w.end), (2, 4));
    }

    #[test]
    fn qt_class_names() {
        assert!(is_qt_class("Qt6112QWindowIcon"));
        assert!(is_qt_class("Qt5QWindowIcon"));
        assert!(!is_qt_class("Chrome_WidgetWin_1"));
        assert!(!is_qt_class("QtFake"));
    }

    #[test]
    fn rejects_out_of_range_and_empty() {
        assert!(parse(&build("학교", 1, 5)).is_none());
        assert!(parse(&build("학교", 0, 0)).is_none());
        assert!(parse(&[0u8; 8]).is_none());
    }
}
