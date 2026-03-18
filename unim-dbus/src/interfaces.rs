//! DBus 인터페이스 정의
//!
//! `org.atit.unim.InputMethod` 및 `org.atit.unim.InputContext` 인터페이스의
//! 메서드/시그널 시그니처와 데이터 타입을 정의합니다.

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Preedit(조합 중) 텍스트 정보
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct PreeditText {
    /// 조합 중인 문자열
    pub text: String,
    /// 커서 위치 (바이트 단위)
    pub cursor_pos: u32,
    /// 화면 표시 여부
    pub visible: bool,
}

/// 입력 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum InputMode {
    /// 한국어 모드
    Korean,
    /// 영어(English) 모드
    English,
}

impl Default for InputMode {
    fn default() -> Self {
        Self::Korean
    }
}

impl From<unim::config::InputCategory> for InputMode {
    fn from(category: unim::config::InputCategory) -> Self {
        match category {
            unim::config::InputCategory::Korean => InputMode::Korean,
            unim::config::InputCategory::English => InputMode::English,
        }
    }
}

impl From<InputMode> for unim::config::InputCategory {
    fn from(mode: InputMode) -> Self {
        match mode {
            InputMode::Korean => unim::config::InputCategory::Korean,
            InputMode::English => unim::config::InputCategory::English,
        }
    }
}
