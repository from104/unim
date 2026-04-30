//! 키 이벤트 → InputEngine 연동 모듈
//!
//! egui 키 이벤트를 UNIM KeyCode로 변환하고 엔진에 전달합니다.

use unim::config::Config;
use unim::input_engine::{InputEngine, InputResult, PopupAction};
use unim::keycode::{KeyCode, ModifierState};

/// egui::Key → unim::KeyCode 변환
pub fn from_egui_key(key: &egui::Key) -> KeyCode {
    match key {
        egui::Key::A => KeyCode::A,
        egui::Key::B => KeyCode::B,
        egui::Key::C => KeyCode::C,
        egui::Key::D => KeyCode::D,
        egui::Key::E => KeyCode::E,
        egui::Key::F => KeyCode::F,
        egui::Key::G => KeyCode::G,
        egui::Key::H => KeyCode::H,
        egui::Key::I => KeyCode::I,
        egui::Key::J => KeyCode::J,
        egui::Key::K => KeyCode::K,
        egui::Key::L => KeyCode::L,
        egui::Key::M => KeyCode::M,
        egui::Key::N => KeyCode::N,
        egui::Key::O => KeyCode::O,
        egui::Key::P => KeyCode::P,
        egui::Key::Q => KeyCode::Q,
        egui::Key::R => KeyCode::R,
        egui::Key::S => KeyCode::S,
        egui::Key::T => KeyCode::T,
        egui::Key::U => KeyCode::U,
        egui::Key::V => KeyCode::V,
        egui::Key::W => KeyCode::W,
        egui::Key::X => KeyCode::X,
        egui::Key::Y => KeyCode::Y,
        egui::Key::Z => KeyCode::Z,
        egui::Key::Num0 => KeyCode::Num0,
        egui::Key::Num1 => KeyCode::Num1,
        egui::Key::Num2 => KeyCode::Num2,
        egui::Key::Num3 => KeyCode::Num3,
        egui::Key::Num4 => KeyCode::Num4,
        egui::Key::Num5 => KeyCode::Num5,
        egui::Key::Num6 => KeyCode::Num6,
        egui::Key::Num7 => KeyCode::Num7,
        egui::Key::Num8 => KeyCode::Num8,
        egui::Key::Num9 => KeyCode::Num9,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Escape => KeyCode::Escape,
        egui::Key::Backspace => KeyCode::Backspace,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Space => KeyCode::Space,
        egui::Key::Minus => KeyCode::Minus,
        egui::Key::Plus => KeyCode::Equal,
        egui::Key::OpenBracket => KeyCode::BracketLeft,
        egui::Key::CloseBracket => KeyCode::BracketRight,
        egui::Key::Backslash => KeyCode::Backslash,
        egui::Key::Semicolon => KeyCode::Semicolon,
        egui::Key::Quote => KeyCode::Quote,
        egui::Key::Backtick => KeyCode::Backquote,
        egui::Key::Comma => KeyCode::Comma,
        egui::Key::Period => KeyCode::Period,
        egui::Key::Slash => KeyCode::Slash,
        egui::Key::F1 => KeyCode::F1,
        egui::Key::F2 => KeyCode::F2,
        egui::Key::F3 => KeyCode::F3,
        egui::Key::F4 => KeyCode::F4,
        egui::Key::F5 => KeyCode::F5,
        egui::Key::F6 => KeyCode::F6,
        egui::Key::F7 => KeyCode::F7,
        egui::Key::F8 => KeyCode::F8,
        egui::Key::F9 => KeyCode::F9,
        egui::Key::F10 => KeyCode::F10,
        egui::Key::F11 => KeyCode::F11,
        egui::Key::F12 => KeyCode::F12,
        egui::Key::Insert => KeyCode::Insert,
        egui::Key::Home => KeyCode::Home,
        egui::Key::PageUp => KeyCode::PageUp,
        egui::Key::Delete => KeyCode::Delete,
        egui::Key::End => KeyCode::End,
        egui::Key::PageDown => KeyCode::PageDown,
        egui::Key::ArrowRight => KeyCode::Right,
        egui::Key::ArrowLeft => KeyCode::Left,
        egui::Key::ArrowDown => KeyCode::Down,
        egui::Key::ArrowUp => KeyCode::Up,
        _ => KeyCode::Unknown,
    }
}

/// egui Modifiers → unim ModifierState 변환
pub fn from_egui_modifiers(m: &egui::Modifiers) -> ModifierState {
    ModifierState {
        shift: m.shift,
        control: m.ctrl,
        alt: m.alt,
        super_key: m.mac_cmd || m.command,
        caps_lock: false,
        num_lock: false,
    }
}

/// 키 처리 결과를 담는 구조체
pub struct KeyProcessResult {
    pub consumed: bool,
    pub commit: Option<String>,
    pub preedit: Option<String>,
    pub popup_action: Option<PopupAction>,
}

/// 엔진에 키 입력을 전달하고 결과를 반환합니다.
pub fn process_key(
    engine: &mut InputEngine,
    config: &Config,
    key: &egui::Key,
    modifiers: &egui::Modifiers,
) -> KeyProcessResult {
    let keycode = from_egui_key(key);
    let modifier = from_egui_modifiers(modifiers);

    let result: InputResult = engine.press_key(keycode, modifier, config);

    let commit = if result.commit_changed {
        let s = engine.commit_str().to_string();
        engine.clear_commit();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    };

    let preedit = if result.preedit_changed {
        let s = engine.preedit_str().to_string();
        Some(s)
    } else {
        None
    };

    let popup_action = engine.take_popup_action();

    KeyProcessResult {
        consumed: result.consumed,
        commit,
        preedit,
        popup_action,
    }
}
