//! 다이얼로그 모음.
//!
//! - `save_as`         : Ctrl+Shift+S 다른 이름으로 저장 (Phase B)
//! - `key_edit`        : 자판 탭 키 셀 편집 (Phase C)
//! - `combo_edit`      : 조합 추가/편집 (Phase D)
//! - `rule_set_edit`   : rule_set 메타·active (Phase E)
//! - `key_meta_edit`   : key_meta 편집 (Phase F)
//! - `new_profile`     : Ctrl+N 새 자판 (Phase G)
//! - `duplicate_profile`: Ctrl+D 복제 (Phase G)
//! - `import_export`   : Ctrl+E / Ctrl+I (Phase G)
//! - `help`            : F1 (Phase G)

pub mod combo_edit;
pub mod duplicate_profile;
pub mod help;
pub mod import_export;
pub mod key_edit;
pub mod key_meta_edit;
pub mod new_profile;
pub mod rule_set_edit;
pub mod save_as;
