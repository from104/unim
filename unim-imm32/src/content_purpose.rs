//! content_purpose.rs — 포커스 필드의 [`ContentPurpose`] 최선노력 감지 (owner: core)
//!
//! ## IMM32 의 구조적 한계 — 문서화-한계 결정
//!
//! TSF(`unim-tsf/src/input_scope.rs::focus_is_password`)는 `ITfInputScope`(COM)로
//! 앱이 명시한 콘텐츠 힌트(`IS_PASSWORD` 등)를 직접 질의한다. IMM32(레거시 키보드
//! 레이아웃 IME API)에는 그런 질의 인터페이스가 없다 — `ImeSelect` /
//! `ImeSetActiveContext` / `ImeProcessKey` 어디에도 "이 필드는 비밀번호다" 라는
//! 정보가 실려 오지 않는다.
//!
//! 그래서 이 모듈은 **최선노력(best-effort) 휴리스틱 하나만** 구현한다: 포커스
//! 윈도우의 클래스가 표준 Win32 `Edit`(또는 `RichEdit*`) 계열이고 `ES_PASSWORD`
//! 스타일 비트가 서 있으면 [`ContentPurpose::Password`], 그 외에는 전부
//! [`ContentPurpose::Normal`].
//!
//! **Pin/Email/Number/Url/Terminal 은 IMM32 에서 감지 불가능** — 표준 Edit 계열
//! 컨트롤은 이들을 구분할 스타일 비트를 두지 않고(TSF 의 InputScope 처럼 세분화된
//! 힌트 자체가 존재하지 않음), 커스텀 컨트롤(대부분의 최신 IMM32 legacy 앱 — 카톡,
//! 한컴 등)은 애초에 어떤 스타일 힌트도 노출하지 않는다. 이는 구현 누락이 아니라
//! IMM32 API 자체의 한계이며, 코드를 늘려서 메울 수 없다 — 이 한계 위에 안전장치만
//! 얹는다(§SPEC 결정).
//!
//! 참고: 표준 `Edit` 컨트롤에 `ES_PASSWORD` 스타일을 준 경우 Windows(user32/comctl32)
//! 가 대개 그 창에서 IME 를 자체적으로 분리(disassociate)해 우리 콜백 자체가 호출되지
//! 않는다 — 그래서 이 휴리스틱이 실제 발동할 기회는 흔치 않다. 그럼에도 서브클래싱
//! 등으로 IME 가 여전히 붙어 있는 경계 케이스를 위한 안전장치로 남겨둔다(fail-closed
//! 이 아니라 fail-normal — 감지 실패 시 항상 `Normal`, 즉 한글 입력을 막지 않는 쪽으로
//! 안전하게 넘어간다. 반대로 갔다면 감지 실패가 멀쩡한 필드의 한글 입력을 막는 회귀가
//! 된다).
//!
//! 호출 지점(`ime_state.rs`): `on_select`(최초 감지 — Linux 체크리스트의 "초기
//! 감지") / `on_activate`(포커스 복귀 — 매 `ImeSetActiveContext(TRUE)` 마다
//! 재판정하므로 "mid-focus 변경 추적"을 포커스 전환 단위로 근사) / `on_deactivate`
//! (포커스 이탈 → `Normal` 명시 복귀, "sticky 차단" 방지). `NotifyIME(CPS_CANCEL)`
//! (엔진 `reset()`)는 이 모듈을 호출하지 않는다 — 필드 내부 Escape 로 한글 차단이
//! 풀리면 안 되기 때문이다(Linux 프런트엔드와 동일 불변식; 코어
//! `surrounding.rs::set_content_purpose` 의 상태머신이 이미 멱등이라 반복 호출에도
//! 안전하다).
//!
//! 위젯이 포커스를 유지한 채 런타임에 input-purpose 를 바꾸는 경우(GTK 의
//! `notify::input-purpose`)는 IMM32 에 대응하는 알림이 없어 감지하지 못한다 —
//! 별도 문서화된 한계다.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetWindowLongW, ES_PASSWORD, GWL_STYLE};

use unim::config::ContentPurpose;

/// `GetClassNameW` 대상 버퍼 크기. 표준 컨트롤 클래스명("Edit", "RichEdit20W" 등)은
/// 전부 20자 미만이라 64면 잘림 없이 충분하다(Win32 관례상 클래스명은 256자 이내지만,
/// 우리가 인식하는 이름은 전부 이 안에 든다 — 더 긴 커스텀 클래스명은 인식 대상이
/// 아니므로 잘려도 아래 비교에서 자연히 불일치 처리된다).
const CLASS_NAME_BUF_LEN: usize = 64;

/// 클래스명이 `ES_PASSWORD` 를 의미 있게 해석하는 표준 Edit 계열인지.
///
/// 정확히 "Edit" 이거나 "RichEdit" 로 시작하면(RichEdit20A/W, RICHEDIT50W 등) 인정한다.
/// 그 외 클래스는 스타일 비트 0x20 이 `ES_PASSWORD` 와 무관한 의미(예: 버튼 계열의
/// 다른 BS_* 비트)일 수 있으므로 오탐을 피하기 위해 제외한다.
fn is_password_capable_class(class_name: &str) -> bool {
    let lower = class_name.to_ascii_lowercase();
    lower == "edit" || lower.starts_with("richedit")
}

/// 순수 판정 로직 — 이미 취득한 클래스명/스타일 비트로부터 [`ContentPurpose`] 를
/// 계산한다. Win32 콜과 분리해 `#[cfg(test)]` 로 단위 테스트 가능하게 한다(실제
/// HWND 없이도 검증 가능한 부분만 순수 함수로 뺀 것).
fn purpose_from_class_and_style(class_name: &str, style: i32) -> ContentPurpose {
    if is_password_capable_class(class_name) && (style & ES_PASSWORD) != 0 {
        ContentPurpose::Password
    } else {
        ContentPurpose::Normal
    }
}

/// 현재(이 스레드) 포커스 윈도우로부터 [`ContentPurpose`] 를 최선노력으로 감지한다.
///
/// IMM32 콜백은 대상 HIMC 를 소유한 스레드에서 실행되므로(`ime_state.rs` 문서의
/// 스레딩 계약 참고), `GetFocus()` 는 항상 **이 프로세스·이 스레드**의 포커스
/// 윈도우를 정확히 반환한다 — 타 스레드/프로세스 포커스를 잘못 읽는 경합이 없다.
///
/// 포커스 윈도우가 없거나 클래스명 취득에 실패하면 `Normal` 로 안전 폴백한다(위
/// 모듈 문서의 fail-normal 원칙).
pub fn detect_focus_purpose() -> ContentPurpose {
    let hwnd: HWND = unsafe { GetFocus() };
    if hwnd.is_invalid() {
        return ContentPurpose::Normal;
    }
    detect_from_hwnd(hwnd)
}

/// 주어진 HWND 기준 감지. `detect_focus_purpose` 에서 분리해 두 단계(포커스 윈도우
/// 취득 / 그 윈도우 판정)를 각각 좁게 유지한다.
fn detect_from_hwnd(hwnd: HWND) -> ContentPurpose {
    let mut buf = [0u16; CLASS_NAME_BUF_LEN];
    // SAFETY: `buf` 는 스택에 살아있는 고정 크기 배열이고, `GetClassNameW` 는
    // `nmaxcount` 만큼만 쓴다(windows-rs 슬라이스 오버로드가 길이를 함께 넘긴다).
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return ContentPurpose::Normal;
    }
    let class_name = String::from_utf16_lossy(&buf[..len as usize]);
    // SAFETY: `hwnd` 는 방금 `GetFocus()` 가 반환한 유효 핸들(널 체크 완료).
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) };
    purpose_from_class_and_style(&class_name, style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_with_password_style_is_password() {
        assert_eq!(
            purpose_from_class_and_style("Edit", ES_PASSWORD),
            ContentPurpose::Password
        );
    }

    #[test]
    fn edit_class_name_case_insensitive() {
        assert_eq!(
            purpose_from_class_and_style("EDIT", ES_PASSWORD),
            ContentPurpose::Password
        );
    }

    #[test]
    fn richedit_variant_recognized() {
        assert_eq!(
            purpose_from_class_and_style("RichEdit20W", ES_PASSWORD),
            ContentPurpose::Password
        );
    }

    #[test]
    fn edit_without_password_style_is_normal() {
        assert_eq!(
            purpose_from_class_and_style("Edit", 0),
            ContentPurpose::Normal
        );
    }

    #[test]
    fn non_edit_class_with_coincidental_bit_is_normal() {
        // 임의 창의 스타일 비트가 0x20(ES_PASSWORD 값)과 우연히 겹쳐도(클래스마다
        // 그 비트의 의미가 다르다 — 예: 버튼 계열의 BS_* 비트) Edit 계열이 아니면
        // 오탐하지 않는다.
        assert_eq!(
            purpose_from_class_and_style("Button", ES_PASSWORD),
            ContentPurpose::Normal
        );
    }

    #[test]
    fn custom_class_never_detected_documented_limitation() {
        // 커스텀/최신 프레임워크 컨트롤(예: Electron/Chromium 계열)은 IMM32 로
        // 스타일 비트 자체를 노출하지 않는다 — 모듈 상단에 문서화된 한계이며,
        // 항상 Normal 로 폴백해야 한다(fail-normal).
        assert_eq!(
            purpose_from_class_and_style("Chrome_RenderWidgetHostHWND", ES_PASSWORD),
            ContentPurpose::Normal
        );
    }
}
