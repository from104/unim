//! GSettings → config.yaml 1회성 마이그레이션 (v2)
//!
//! UNIM 설정 시스템 개편(Phase 4)에서 GNOME gschema에서 삭제된 13개 키의
//! 사용자 커스터마이징 값을 `~/.config/unim/config.yaml`로 이관한다.
//!
//! # 동작 원리
//!
//! 1. 가드 파일 `~/.config/unim/.migrated-v2` 존재 시 즉시 skip
//! 2. `dconf read /org/gnome/shell/extensions/unim/<key>`로 구 값 조회
//!    (schema 삭제 후에도 dconf DB에는 값이 남아있음)
//! 3. config.yaml이 **아직 기본값**인 필드에 한해 dconf 값으로 덮어씀
//!    (사용자가 GTK/CLI에서 이미 수정한 값은 보존)
//! 4. 저장 성공 시 가드 파일 생성
//!
//! # 에러 정책
//!
//! - 마이그레이션 실패는 비치명적 — daemon은 정상 기동
//! - 실패 시 가드를 **생성하지 않음** → 다음 기동에서 재시도
//! - 키 단위 타입 변환 실패는 해당 키만 skip (경고 로그)

use std::path::{Path, PathBuf};
use std::process::Command;

use unim::config::{
    Config, ConfigLoadStatus, EnglishLayout, InputCategory, KoreanLayout, ModeSharingMode,
};
use unim::unim_log;

const GUARD_FILENAME: &str = ".migrated-v2";
const DCONF_BASE: &str = "/org/gnome/shell/extensions/unim";

/// 구성 키 값을 읽어오는 추상 인터페이스 (테스트 용이성 목적)
pub trait SettingReader {
    /// 키의 원시 값(GVariant 텍스트 표현)을 반환. 값이 없으면 None.
    fn read(&self, key: &str) -> Option<String>;
}

/// dconf CLI 기반 실제 구현
pub struct DconfReader;

impl SettingReader for DconfReader {
    fn read(&self, key: &str) -> Option<String> {
        let path = format!("{}/{}", DCONF_BASE, key);
        let output = Command::new("dconf").arg("read").arg(&path).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// `dconf`가 시스템에 설치되어 있는지 확인
fn dconf_available() -> bool {
    Command::new("dconf")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("unim"))
}

fn guard_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join(GUARD_FILENAME))
}

fn touch_guard(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::File::create(path)?;
    Ok(())
}

/// 진입점 — daemon 기동 시 호출한다.
///
/// 가드 파일 존재 시 즉시 반환. 마이그레이션 실패 시에도 daemon 기동은 계속된다.
pub fn migrate_v2() {
    let Some(guard) = guard_path() else {
        unim_log!("DAEMON", "마이그레이션 v2 skip: config 디렉토리 결정 불가");
        return;
    };

    if guard.exists() {
        return;
    }

    // GNOME/dconf 없는 환경(순수 X11/KDE 등)에서는 조용히 가드만 생성
    if !dconf_available() {
        if let Err(e) = touch_guard(&guard) {
            unim_log!(
                "DAEMON",
                "마이그레이션 v2 경고: dconf 부재 환경 가드 생성 실패: {}",
                e
            );
        }
        return;
    }

    // 현재 config 로드 (없으면 default). 손상 복구가 있었다면 마이그레이션의
    // "정상 완료" 로그가 그 사실을 삼키기 전에 먼저 남긴다(GAP-config-06/M-11) —
    // 그렇지 않으면 손상→기본값 대체→dconf 값 얹어 저장→가드 생성 이후 로그엔
    // "정상 마이그레이션"만 남아 초기화 이력이 사라진다.
    let (mut config, load_status) = Config::load_from_default_path_with_status();
    if let ConfigLoadStatus::Recovered { reason } = &load_status {
        unim_log!(
            "DAEMON",
            "마이그레이션 v2: config.yaml 손상 복구 감지(사유: {}) — 기본값으로 초기화된 뒤 진행합니다",
            reason
        );
    }
    let default_config = Config::default();
    let reader = DconfReader;

    let applied = apply_migration(&mut config, &default_config, &reader);

    if applied == 0 {
        // dconf에 커스텀 값이 하나도 없어도 가드는 생성 (재시도 무의미)
        if let Err(e) = touch_guard(&guard) {
            unim_log!("DAEMON", "마이그레이션 v2 경고: 가드 생성 실패: {}", e);
        }
        unim_log!("DAEMON", "마이그레이션 v2: 이관할 GSettings 커스텀 값 없음");
        return;
    }

    // clamp 후 저장
    config.engine.auto_typefix.clamp_ranges();

    match config.save_to_default_path() {
        Ok(()) => {
            if let Err(e) = touch_guard(&guard) {
                unim_log!(
                    "DAEMON",
                    "마이그레이션 v2 경고: 저장 성공했으나 가드 생성 실패: {}",
                    e
                );
                return;
            }
            unim_log!(
                "DAEMON",
                "마이그레이션 v2 완료: GSettings → config.yaml ({}개 키 이관)",
                applied
            );
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "마이그레이션 v2 경고: config.yaml 저장 실패 (가드 미생성, 다음 기동 재시도): {}",
                e
            );
        }
    }
}

/// 순수 마이그레이션 로직 — 테스트에서 mock reader로 호출.
///
/// 반환: 실제로 이관된 키 개수.
pub fn apply_migration(config: &mut Config, default: &Config, reader: &dyn SettingReader) -> usize {
    let mut applied = 0;

    // korean-layout → engine.korean.layout
    if config.engine.korean.layout == default.engine.korean.layout {
        if let Some(raw) = reader.read("korean-layout") {
            if let Some(v) = parse_korean_layout(&raw) {
                config.engine.korean.layout = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: korean-layout 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // english-layout → engine.english.layout
    if config.engine.english.layout == default.engine.english.layout {
        if let Some(raw) = reader.read("english-layout") {
            if let Some(v) = parse_english_layout(&raw) {
                config.engine.english.layout = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: english-layout 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // initial-mode → engine.default_category
    if config.engine.default_category == default.engine.default_category {
        if let Some(raw) = reader.read("initial-mode") {
            if let Some(v) = parse_input_category(&raw) {
                config.engine.default_category = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: initial-mode 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // mode-sharing → engine.mode_sharing
    if config.engine.mode_sharing == default.engine.mode_sharing {
        if let Some(raw) = reader.read("mode-sharing") {
            if let Some(v) = parse_mode_sharing(&raw) {
                config.engine.mode_sharing = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: mode-sharing 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // toggle-keys → engine.toggle_keys
    if config.engine.toggle_keys == default.engine.toggle_keys {
        if let Some(raw) = reader.read("toggle-keys") {
            if let Some(v) = parse_string_list(&raw) {
                config.engine.toggle_keys = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: toggle-keys 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // hanja-keys → engine.hanja_keys
    if config.engine.hanja_keys == default.engine.hanja_keys {
        if let Some(raw) = reader.read("hanja-keys") {
            if let Some(v) = parse_string_list(&raw) {
                config.engine.hanja_keys = v;
                applied += 1;
            } else {
                unim_log!(
                    "DAEMON",
                    "마이그레이션: hanja-keys 값 해석 실패 (skip): {}",
                    raw
                );
            }
        }
    }

    // auto-typefix-* bools
    let atf_def = &default.engine.auto_typefix;
    let atf = &mut config.engine.auto_typefix;

    if atf.enabled == atf_def.enabled {
        if let Some(v) = reader
            .read("auto-typefix-enabled")
            .and_then(|r| parse_bool(&r))
        {
            atf.enabled = v;
            applied += 1;
        }
    }
    if atf.forward == atf_def.forward {
        if let Some(v) = reader
            .read("auto-typefix-forward")
            .and_then(|r| parse_bool(&r))
        {
            atf.forward = v;
            applied += 1;
        }
    }
    if atf.reverse == atf_def.reverse {
        if let Some(v) = reader
            .read("auto-typefix-reverse")
            .and_then(|r| parse_bool(&r))
        {
            atf.reverse = v;
            applied += 1;
        }
    }

    // auto-typefix-time-window (u32). 주의: 구 gschema default=2000이지만
    // config.rs default=5000. 사용자가 설정하지 않아도 dconf에 2000이 남아있을 수 있음.
    // 이 경우 2000을 "사용자 값"으로 보고 이관해야 과거 동작이 유지된다.
    // 구 gschema는 단일 키. forward/reverse 두 신 필드 모두 기본값일 때만 주입.
    if atf.forward_time_window_ms == atf_def.forward_time_window_ms
        && atf.reverse_time_window_ms == atf_def.reverse_time_window_ms
    {
        if let Some(v) = reader
            .read("auto-typefix-time-window")
            .and_then(|r| parse_uint(&r))
        {
            atf.forward_time_window_ms = v;
            atf.reverse_time_window_ms = v;
            applied += 1;
        }
    }
    if atf.kor_syllable_threshold == atf_def.kor_syllable_threshold {
        if let Some(v) = reader
            .read("auto-typefix-kor-threshold")
            .and_then(|r| parse_uint(&r))
        {
            atf.kor_syllable_threshold = v as u8;
            applied += 1;
        }
    }
    if atf.eng_word_min_length == atf_def.eng_word_min_length {
        if let Some(v) = reader
            .read("auto-typefix-eng-min-length")
            .and_then(|r| parse_uint(&r))
        {
            atf.eng_word_min_length = v as u8;
            applied += 1;
        }
    }

    applied
}

// ---------- 파서 유틸 ----------

/// dconf 문자열 값에서 따옴표 제거.
/// dconf read는 문자열을 `'value'` 형태로 반환한다.
fn strip_quotes(s: &str) -> &str {
    let t = s.trim();
    if (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
        || (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
    {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_uint(raw: &str) -> Option<u32> {
    let t = raw.trim();
    // GVariant uint32는 "uint32 2000" 형태로 표현됨
    let num = t.strip_prefix("uint32 ").unwrap_or(t).trim();
    num.parse::<u32>().ok()
}

fn parse_korean_layout(raw: &str) -> Option<KoreanLayout> {
    match strip_quotes(raw) {
        "2bul" => Some("ko_2bulstd".to_string()),
        "3bul390" => Some("ko_3bul390".to_string()),
        "3bul391" => Some("ko_3bul391".to_string()),
        "3bul_noshift" => Some("ko_3bul_noshift".to_string()),
        _ => None,
    }
}

fn parse_english_layout(raw: &str) -> Option<EnglishLayout> {
    match strip_quotes(raw) {
        "qwerty" => Some("qwerty".to_string()),
        "dvorak" => Some("dvorak".to_string()),
        "colemak" => Some("colemak".to_string()),
        "colemak_dh" => Some("colemak_dh".to_string()),
        "workman" => Some("workman".to_string()),
        _ => None,
    }
}

fn parse_input_category(raw: &str) -> Option<InputCategory> {
    match strip_quotes(raw) {
        "Korean" | "korean" => Some(InputCategory::Korean),
        "English" | "english" => Some(InputCategory::English),
        _ => None,
    }
}

fn parse_mode_sharing(raw: &str) -> Option<ModeSharingMode> {
    match strip_quotes(raw) {
        "global" | "Global" => Some(ModeSharingMode::Global),
        "per_app" | "PerApp" | "per-app" => Some(ModeSharingMode::PerApp),
        _ => None,
    }
}

/// GVariant as (array of string) 파서
///
/// 예: `['Korean', 'RightAlt']` 또는 `['Hanja', 'F9']`
fn parse_string_list(raw: &str) -> Option<Vec<String>> {
    let t = raw.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in inner.split(',') {
        let s = strip_quotes(part.trim()).to_string();
        out.push(s);
    }
    Some(out)
}

// ---------- 테스트 ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// HashMap 기반 mock reader
    struct MockReader(HashMap<String, String>);

    impl MockReader {
        fn new(entries: &[(&str, &str)]) -> Self {
            Self(
                entries
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
        }
    }

    impl SettingReader for MockReader {
        fn read(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }

    #[test]
    fn test_parse_helpers() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("nope"), None);

        assert_eq!(parse_uint("uint32 2000"), Some(2000));
        assert_eq!(parse_uint("3500"), Some(3500));
        assert_eq!(parse_uint("bad"), None);

        assert_eq!(
            parse_korean_layout("'3bul390'"),
            Some("ko_3bul390".to_string())
        );
        assert_eq!(parse_english_layout("'dvorak'"), Some("dvorak".to_string()));
        assert_eq!(
            parse_input_category("'Korean'"),
            Some(InputCategory::Korean)
        );
        assert_eq!(
            parse_mode_sharing("'per_app'"),
            Some(ModeSharingMode::PerApp)
        );
        assert_eq!(
            parse_string_list("['Korean', 'RightAlt']"),
            Some(vec!["Korean".to_string(), "RightAlt".to_string()])
        );
        assert_eq!(parse_string_list("[]"), Some(Vec::new()));
    }

    #[test]
    fn test_default_config_migrates_all_custom_values() {
        let mut config = Config::default();
        let default_config = Config::default();
        let reader = MockReader::new(&[
            ("korean-layout", "'3bul390'"),
            ("english-layout", "'dvorak'"),
            ("initial-mode", "'Korean'"),
            ("mode-sharing", "'per_app'"),
            ("toggle-keys", "['ShiftSpace']"),
            ("hanja-keys", "['F9']"),
            ("auto-typefix-enabled", "false"),
            ("auto-typefix-forward", "false"),
            ("auto-typefix-reverse", "false"),
            ("auto-typefix-time-window", "uint32 3000"),
            ("auto-typefix-kor-threshold", "uint32 4"),
            ("auto-typefix-eng-min-length", "uint32 6"),
        ]);

        let applied = apply_migration(&mut config, &default_config, &reader);
        assert_eq!(applied, 12);

        assert_eq!(config.engine.korean.layout, "ko_3bul390");
        assert_eq!(config.engine.english.layout, "dvorak");
        assert_eq!(config.engine.default_category, InputCategory::Korean);
        assert_eq!(config.engine.mode_sharing, ModeSharingMode::PerApp);
        assert_eq!(config.engine.toggle_keys, vec!["ShiftSpace".to_string()]);
        assert_eq!(config.engine.hanja_keys, vec!["F9".to_string()]);
        assert!(!config.engine.auto_typefix.enabled);
        assert!(!config.engine.auto_typefix.forward);
        assert!(!config.engine.auto_typefix.reverse);
        assert_eq!(config.engine.auto_typefix.forward_time_window_ms, 3000);
        assert_eq!(config.engine.auto_typefix.reverse_time_window_ms, 3000);
        assert_eq!(config.engine.auto_typefix.kor_syllable_threshold, 4);
        assert_eq!(config.engine.auto_typefix.eng_word_min_length, 6);
    }

    #[test]
    fn test_user_modified_config_is_preserved() {
        // config.yaml에 이미 사용자 값이 있다 (기본값과 다름)
        let mut config = Config::default();
        config.engine.korean.layout = "ko_3bul391".to_string();
        config.engine.auto_typefix.forward_time_window_ms = 4500;
        config.engine.auto_typefix.reverse_time_window_ms = 4500;

        let default_config = Config::default();
        let reader = MockReader::new(&[
            ("korean-layout", "'3bul390'"),              // 덮어쓰기 금지
            ("auto-typefix-time-window", "uint32 3000"), // 덮어쓰기 금지
            ("english-layout", "'colemak'"),             // 기본값이니 이관
        ]);

        let applied = apply_migration(&mut config, &default_config, &reader);
        assert_eq!(applied, 1, "영어 레이아웃만 이관되어야 함");

        assert_eq!(config.engine.korean.layout, "ko_3bul391", "사용자 값 보존");
        assert_eq!(
            config.engine.auto_typefix.forward_time_window_ms, 4500,
            "사용자 값 보존"
        );
        assert_eq!(
            config.engine.auto_typefix.reverse_time_window_ms, 4500,
            "사용자 값 보존"
        );
        assert_eq!(config.engine.english.layout, "colemak");
    }

    #[test]
    fn test_empty_reader_applies_zero_keys() {
        let mut config = Config::default();
        let default_config = Config::default();
        let reader = MockReader::new(&[]);

        let applied = apply_migration(&mut config, &default_config, &reader);
        assert_eq!(applied, 0);
    }

    #[test]
    fn test_invalid_values_are_skipped_with_no_change() {
        let mut config = Config::default();
        let default_config = Config::default();
        let reader = MockReader::new(&[
            ("korean-layout", "'invalid-layout'"),
            ("auto-typefix-enabled", "not-a-bool"),
            ("auto-typefix-time-window", "not-a-number"),
            // 유효한 값 하나
            ("english-layout", "'workman'"),
        ]);

        let applied = apply_migration(&mut config, &default_config, &reader);
        assert_eq!(applied, 1);
        assert_eq!(
            config.engine.korean.layout,
            default_config.engine.korean.layout
        );
        assert_eq!(config.engine.english.layout, "workman");
    }

    #[test]
    fn test_partial_migration_counts_correctly() {
        let mut config = Config::default();
        let default_config = Config::default();
        let reader = MockReader::new(&[
            ("toggle-keys", "['Korean','LeftAlt']"),
            ("hanja-keys", "['F9']"),
            ("auto-typefix-forward", "false"),
        ]);

        let applied = apply_migration(&mut config, &default_config, &reader);
        assert_eq!(applied, 3);
        assert_eq!(
            config.engine.toggle_keys,
            vec!["Korean".to_string(), "LeftAlt".to_string()]
        );
        assert_eq!(config.engine.hanja_keys, vec!["F9".to_string()]);
        assert!(!config.engine.auto_typefix.forward);
    }
}
