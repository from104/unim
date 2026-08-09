//! Wayland `zwp_input_popup_surface_v2` backend (Phase 5).
//!
//! KWin 6 / wlroots 등 `zwp_input_method_v2`를 광고하는 컴포지터에서 IME 전용
//! popup surface를 사용한다. 컴포지터가 cursor 위치 기반으로 자동 anchor +
//! outside click 자동 dismiss(popup_done) 처리.
//!
//! 본 모듈은 골격(TODO) 단계로, 다음 구현 시 wayland-client 크레이트로
//! 다음 절차를 수행한다:
//!   1. wl_registry로부터 `zwp_input_method_manager_v2` 광고 확인
//!   2. zwp_input_method_manager_v2.get_input_method(seat) → zwp_input_method_v2
//!   3. zwp_input_method_v2.activate() (이미 daemon이 처리할 수도)
//!   4. zwp_input_method_v2.get_input_popup_surface(wl_surface) → zwp_input_popup_surface_v2
//!   5. zwp_input_popup_surface_v2.text_input_rectangle(x,y,w,h) — cursor 위치
//!   6. wl_surface.commit() → 컴포지터가 적절 위치에 popup 표시
//!   7. dismiss는 컴포지터가 자체 처리 (popup_done event)
//!
//! 현재는 detection만 noop으로 구현, 미지원 반환.

#![cfg(feature = "wayland-backend")]

/// 현재 Wayland session에서 zwp_input_method_v2/zwp_input_popup_surface_v2를 사용할 수 있는지.
/// Phase 5 후속 구현에서 실제 registry binding 확인 로직 추가.
pub fn is_supported() -> bool {
    // TODO: wl_registry를 열어 "zwp_input_method_manager_v2" 인터페이스 광고 여부 확인.
    // 현재는 항상 false 반환 → standalone backend로 폴백.
    false
}

/// TODO: input_popup_surface 생성 + cursor rectangle 전달.
pub fn show(_cursor_x: i32, _cursor_y: i32, _cursor_w: i32, _cursor_h: i32) {
    // Phase 5 후속 구현
}

/// TODO: input_popup_surface destroy.
pub fn hide() {
    // Phase 5 후속 구현
}
