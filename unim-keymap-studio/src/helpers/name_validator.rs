//! 자판 이름(식별자) 검증 — Save As / 새 자판 / 복제 시 사용.
//!
//! 규칙:
//! - 빈 이름 거부.
//! - 영문·숫자·언더스코어·하이픈만 허용 (`^[A-Za-z0-9_-]+$`).
//! - 빌트인 이름과 충돌 거부 (override 금지 — GUI 정책).
//! - 사용자 폴더에 같은 이름 존재 시 거부. 단, `current_name` 과 동일하면 허용
//!   (자기 자신 덮어쓰기).

use unim::keystroke::profile::{builtin::BUILTIN_NAMES, ProfileRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameConflict {
    /// 사용 가능.
    None,
    /// 빌트인 이름과 충돌.
    BuiltinExists,
    /// 사용자 폴더에 같은 이름 존재.
    UserExists,
    /// 허용되지 않는 문자 포함.
    InvalidChars,
    /// 빈 이름.
    Empty,
}

/// 이름이 허용 문자(`A-Za-z0-9_-`)로만 구성됐는지.
fn is_valid_chars(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 새 이름의 충돌 여부 판정.
///
/// `current_name`: 현재 편집 중 자판 이름 (자기 자신 덮어쓰기는 허용).
pub fn validate_new_name(
    name: &str,
    registry: &ProfileRegistry,
    current_name: Option<&str>,
) -> NameConflict {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return NameConflict::Empty;
    }
    if !is_valid_chars(trimmed) {
        return NameConflict::InvalidChars;
    }
    if BUILTIN_NAMES.contains(&trimmed) {
        return NameConflict::BuiltinExists;
    }
    // 자기 자신과 같은 이름이면 허용.
    if current_name == Some(trimmed) {
        return NameConflict::None;
    }
    if registry.is_user_override(trimmed) {
        return NameConflict::UserExists;
    }
    NameConflict::None
}

/// 사용자에게 보일 검증 메시지의 i18n 키.
pub fn conflict_message_key(conflict: NameConflict) -> &'static str {
    match conflict {
        NameConflict::None => "save_as_hint_ok",
        NameConflict::BuiltinExists => "save_as_hint_builtin",
        NameConflict::UserExists => "save_as_hint_user_exists",
        NameConflict::InvalidChars => "save_as_hint_invalid",
        NameConflict::Empty => "save_as_hint_empty",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> ProfileRegistry {
        ProfileRegistry::builtin_only()
    }

    #[test]
    fn empty_name_rejected() {
        assert_eq!(validate_new_name("", &reg(), None), NameConflict::Empty);
        assert_eq!(validate_new_name("   ", &reg(), None), NameConflict::Empty);
    }

    #[test]
    fn special_chars_rejected() {
        assert_eq!(
            validate_new_name("my layout", &reg(), None),
            NameConflict::InvalidChars
        );
        assert_eq!(
            validate_new_name("한글이름", &reg(), None),
            NameConflict::InvalidChars
        );
        assert_eq!(
            validate_new_name("name.json", &reg(), None),
            NameConflict::InvalidChars
        );
    }

    #[test]
    fn dash_and_underscore_allowed() {
        assert_eq!(
            validate_new_name("my_layout-2", &reg(), None),
            NameConflict::None
        );
    }

    #[test]
    fn builtin_name_conflict_detected_even_without_user_dir() {
        assert_eq!(
            validate_new_name("ko_2bulstd", &reg(), None),
            NameConflict::BuiltinExists
        );
        assert_eq!(
            validate_new_name("en_qwerty", &reg(), None),
            NameConflict::BuiltinExists
        );
    }

    #[test]
    fn same_name_as_current_allowed() {
        // 사용자 자판을 자기 이름으로 다시 저장 — 허용. (빌트인이 아닌 이름)
        assert_eq!(
            validate_new_name("my_custom", &reg(), Some("my_custom")),
            NameConflict::None
        );
    }
}
