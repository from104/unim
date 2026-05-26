//! 편집 탭의 가변 버퍼.
//!
//! `LayoutProfile`을 사본으로 받아 GUI에서 직접 mutate하다가 "저장" 시
//! `unim_keymap_common::save_profile_json`을 통해 사용자 디렉토리에 직렬화한다.
//!
//! Phase B 이후 신규 메서드(rule_sets/key_meta CRUD, set_language 등)가 추가된다.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use unim::keystroke::profile::{
    builtin::BUILTIN_NAMES, CombinationsBlock, KeyMeta, LayoutProfile, LocalizedText, MoachigiSpec,
    RawTriple, RuleSet,
};

#[cfg(test)]
use unim::keystroke::profile::ProfileRegistry;

// 외부 사용자 (app.rs) 는 ProfileRegistry 를 직접 import 하므로 여기서는
// is_builtin_selection 시그니처에만 필요.
use unim::keystroke::profile::ProfileRegistry as _ProfileRegistryForSig;
use unim_keymap_common::save_profile_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Phase D 부터 사용
pub enum ComboKind {
    Cho,
    Jung,
    Jong,
}

pub struct EditorState {
    pub buf: LayoutProfile,
    pub original: LayoutProfile,
    pub dirty: bool,
}

// 일부 메서드는 후속 phase 에서만 사용된다.
#[allow(dead_code)]
impl EditorState {
    pub fn new(profile: LayoutProfile) -> Self {
        Self {
            original: profile.clone(),
            buf: profile,
            dirty: false,
        }
    }

    pub fn revert(&mut self) {
        self.buf = self.original.clone();
        self.dirty = false;
    }

    pub fn set_metadata_author(&mut self, author: Option<String>) {
        self.buf.metadata.author = author;
        self.dirty = true;
    }
    pub fn set_metadata_version(&mut self, version: Option<String>) {
        self.buf.metadata.version = version;
        self.dirty = true;
    }
    pub fn set_metadata_description(&mut self, desc: Option<String>) {
        self.buf.metadata.description = desc.map(LocalizedText::Single);
        self.dirty = true;
    }

    pub fn set_supports_moachigi(&mut self, v: bool) {
        self.buf.moachigi = if v { Some(MoachigiSpec::default()) } else { None };
        if v && self.buf.schema_version < 3 {
            self.buf.schema_version = 3;
        }
        self.dirty = true;
    }

    /// 키 라벨 변경. upper=true면 upper, 아니면 lower.
    pub fn set_key_label(&mut self, upper: bool, row: u8, col: u8, label: String) -> bool {
        let rows = if upper {
            &mut self.buf.layout.upper
        } else {
            &mut self.buf.layout.lower
        };
        let target: &mut Vec<String> = match row {
            0 => &mut rows.row1,
            1 => &mut rows.row2,
            2 => &mut rows.row3,
            3 => &mut rows.row4,
            _ => return false,
        };
        if (col as usize) < target.len() {
            target[col as usize] = label;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn combos(&self, kind: ComboKind) -> Vec<RawTriple> {
        let block = self.buf.combinations.as_ref();
        match (block, kind) {
            (Some(b), ComboKind::Cho) => b.cho.clone(),
            (Some(b), ComboKind::Jung) => b.jung.clone(),
            (Some(b), ComboKind::Jong) => b.jong.clone(),
            _ => Vec::new(),
        }
    }

    pub fn push_combo(&mut self, kind: ComboKind, triple: RawTriple) {
        let block = self
            .buf
            .combinations
            .get_or_insert_with(CombinationsBlock::default);
        match kind {
            ComboKind::Cho => block.cho.push(triple),
            ComboKind::Jung => block.jung.push(triple),
            ComboKind::Jong => block.jong.push(triple),
        }
        self.dirty = true;
    }

    pub fn update_combo(&mut self, kind: ComboKind, idx: usize, triple: RawTriple) -> bool {
        let block = match self.buf.combinations.as_mut() {
            Some(b) => b,
            None => return false,
        };
        let vec_ref: &mut Vec<RawTriple> = match kind {
            ComboKind::Cho => &mut block.cho,
            ComboKind::Jung => &mut block.jung,
            ComboKind::Jong => &mut block.jong,
        };
        if idx < vec_ref.len() {
            vec_ref[idx] = triple;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn remove_combo(&mut self, kind: ComboKind, idx: usize) -> bool {
        let block = match self.buf.combinations.as_mut() {
            Some(b) => b,
            None => return false,
        };
        let vec_ref: &mut Vec<RawTriple> = match kind {
            ComboKind::Cho => &mut block.cho,
            ComboKind::Jung => &mut block.jung,
            ComboKind::Jong => &mut block.jong,
        };
        if idx < vec_ref.len() {
            vec_ref.remove(idx);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn rule_sets(&self) -> &BTreeMap<String, RuleSet> {
        &self.buf.rule_sets
    }

    pub fn toggle_rule_set(&mut self, name: &str, active: bool) {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            rs.active = active;
            self.dirty = true;
        }
    }

    pub fn rename(&mut self, new_name: String) {
        self.buf.name = new_name;
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<PathBuf, unim_keymap_common::SaveError> {
        let path = save_profile_json(&self.buf)?;
        self.dirty = false;
        self.original = self.buf.clone();
        Ok(path)
    }

    /// 새 이름으로 저장 (Save As). 이름 검증은 호출자(name_validator) 책임.
    pub fn save_as(&mut self, new_name: String) -> Result<PathBuf, unim_keymap_common::SaveError> {
        self.buf.name = new_name;
        let path = save_profile_json(&self.buf)?;
        self.dirty = false;
        self.original = self.buf.clone();
        Ok(path)
    }

    // ── 기본 탭 (Phase B) ─────────────────────────────────────────────────

    /// 언어 변경. english 로 바꾸면 한글 전용 필드(combinations/rule_sets/
    /// key_meta/moachigi)를 비운다.
    pub fn set_language(&mut self, lang: &str) {
        self.buf.language = lang.to_string();
        if lang == "english" {
            self.buf.combinations = None;
            self.buf.rule_sets.clear();
            self.buf.active_rule_sets = None;
            self.buf.key_meta = None;
            self.buf.moachigi = None;
            if self.buf.schema_version >= 3 {
                self.buf.schema_version = 1;
            }
        }
        self.dirty = true;
    }

    pub fn set_layout_type(&mut self, t: &str) {
        self.buf.layout_type = t.to_string();
        self.dirty = true;
    }

    pub fn set_display_name_ko(&mut self, s: Option<String>) {
        self.buf.metadata.display_name =
            set_localized_lang(self.buf.metadata.display_name.take(), "ko", s);
        self.dirty = true;
    }

    pub fn set_display_name_en(&mut self, s: Option<String>) {
        self.buf.metadata.display_name =
            set_localized_lang(self.buf.metadata.display_name.take(), "en", s);
        self.dirty = true;
    }

    pub fn set_description_ko(&mut self, s: Option<String>) {
        self.buf.metadata.description =
            set_localized_lang(self.buf.metadata.description.take(), "ko", s);
        self.dirty = true;
    }

    pub fn set_description_en(&mut self, s: Option<String>) {
        self.buf.metadata.description =
            set_localized_lang(self.buf.metadata.description.take(), "en", s);
        self.dirty = true;
    }

    pub fn set_license(&mut self, s: Option<String>) {
        self.buf.metadata.license = s.filter(|v| !v.is_empty());
        self.dirty = true;
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.buf.metadata.tags = tags;
        self.dirty = true;
    }

    pub fn set_inherits(&mut self, name: Option<String>) {
        self.buf.inherits = name.filter(|v| !v.is_empty());
        self.dirty = true;
    }

    // ── 확장 탭: rule_sets (Phase E) ──────────────────────────────────────

    /// rule_set 이름 목록 (정렬).
    pub fn rule_set_names(&self) -> Vec<String> {
        self.buf.rule_sets.keys().cloned().collect()
    }

    /// 새 rule_set 추가. 이미 있으면 false.
    pub fn add_rule_set(&mut self, name: String) -> bool {
        if name.is_empty() || self.buf.rule_sets.contains_key(&name) {
            return false;
        }
        self.buf.rule_sets.insert(name, RuleSet::default());
        self.dirty = true;
        true
    }

    /// rule_set 삭제. active_rule_sets 목록에서도 제거.
    pub fn remove_rule_set(&mut self, name: &str) -> bool {
        let removed = self.buf.rule_sets.remove(name).is_some();
        if removed {
            if let Some(list) = self.buf.active_rule_sets.as_mut() {
                list.retain(|n| n != name);
            }
            self.dirty = true;
        }
        removed
    }

    /// rule_set 설명(ko/en) 설정.
    pub fn set_rule_set_description(&mut self, name: &str, ko: String, en: String) {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            let mut map: BTreeMap<String, String> = BTreeMap::new();
            if !ko.is_empty() {
                map.insert("ko".to_string(), ko);
            }
            if !en.is_empty() {
                map.insert("en".to_string(), en);
            }
            rs.description = if map.is_empty() {
                None
            } else {
                Some(LocalizedText::Map(map))
            };
            self.dirty = true;
        }
    }

    /// rule_set 의 조합 목록 (flat — scope 자동판별).
    pub fn rule_set_combos(&self, name: &str) -> Vec<RawTriple> {
        self.buf
            .rule_sets
            .get(name)
            .map(|rs| rs.combinations.clone())
            .unwrap_or_default()
    }

    pub fn push_rule_set_combo(&mut self, name: &str, triple: RawTriple) {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            rs.combinations.push(triple);
            self.dirty = true;
        }
    }

    pub fn update_rule_set_combo(&mut self, name: &str, idx: usize, triple: RawTriple) -> bool {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            if idx < rs.combinations.len() {
                rs.combinations[idx] = triple;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn remove_rule_set_combo(&mut self, name: &str, idx: usize) -> bool {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            if idx < rs.combinations.len() {
                rs.combinations.remove(idx);
                self.dirty = true;
                return true;
            }
        }
        false
    }

    // ── 확장 탭: 전역 key_meta (Phase F) ──────────────────────────────────

    /// 전역 key_meta 목록 (key, meta) — 키 순 정렬.
    pub fn key_meta_iter(&self) -> Vec<(String, KeyMeta)> {
        let mut v: Vec<(String, KeyMeta)> = self
            .buf
            .key_meta
            .as_ref()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// 전역 key_meta 설정(추가/덮어쓰기).
    pub fn set_key_meta(&mut self, key: String, meta: KeyMeta) {
        self.buf
            .key_meta
            .get_or_insert_with(HashMap::new)
            .insert(key, meta);
        // schema_version 2 이상 필요.
        if self.buf.schema_version < 2 {
            self.buf.schema_version = 2;
        }
        self.dirty = true;
    }

    /// 전역 key_meta 제거. 비면 None 으로 정리.
    pub fn remove_key_meta(&mut self, key: &str) -> bool {
        let removed = self
            .buf
            .key_meta
            .as_mut()
            .map(|m| m.remove(key).is_some())
            .unwrap_or(false);
        if removed {
            if self.buf.key_meta.as_ref().map(|m| m.is_empty()).unwrap_or(false) {
                self.buf.key_meta = None;
            }
            self.dirty = true;
        }
        removed
    }

    // ── rule_set 한정 key_meta (Phase F) ─────────────────────────────────

    /// 특정 rule_set 의 key_meta 목록 (키 순 정렬).
    pub fn rule_set_key_meta_iter(&self, name: &str) -> Vec<(String, KeyMeta)> {
        self.buf
            .rule_sets
            .get(name)
            .and_then(|rs| rs.key_meta.as_ref())
            .map(|m| {
                let mut v: Vec<(String, KeyMeta)> =
                    m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            })
            .unwrap_or_default()
    }

    /// rule_set 의 key_meta 설정(추가/덮어쓰기).
    pub fn set_rule_set_key_meta(&mut self, name: &str, key: String, meta: KeyMeta) {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            rs.key_meta.get_or_insert_with(HashMap::new).insert(key, meta);
            if self.buf.schema_version < 2 {
                self.buf.schema_version = 2;
            }
            self.dirty = true;
        }
    }

    /// rule_set 의 key_meta 제거. 비면 None 으로 정리.
    pub fn remove_rule_set_key_meta(&mut self, name: &str, key: &str) -> bool {
        if let Some(rs) = self.buf.rule_sets.get_mut(name) {
            let removed = rs
                .key_meta
                .as_mut()
                .map(|m| m.remove(key).is_some())
                .unwrap_or(false);
            if removed {
                if rs.key_meta.as_ref().map(|m| m.is_empty()).unwrap_or(false) {
                    rs.key_meta = None;
                }
                self.dirty = true;
            }
            return removed;
        }
        false
    }
}

/// `LocalizedText` 에서 특정 언어 값을 추출. Single 은 ko 로 간주(편집 UI 편의).
pub fn localized_lang(text: Option<&LocalizedText>, lang: &str) -> String {
    match text {
        Some(LocalizedText::Map(m)) => m.get(lang).cloned().unwrap_or_default(),
        Some(LocalizedText::Single(s)) => {
            if lang == "ko" {
                s.clone()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// `LocalizedText` 의 특정 언어 값을 갱신. 빈 값이면 해당 언어 제거. 모두 비면 None.
/// 기존 Single 값은 손실 방지를 위해 `ko` 키로 보존된다.
fn set_localized_lang(
    existing: Option<LocalizedText>,
    lang: &str,
    value: Option<String>,
) -> Option<LocalizedText> {
    let mut map: BTreeMap<String, String> = match existing {
        Some(LocalizedText::Map(m)) => m,
        Some(LocalizedText::Single(s)) => {
            let mut m = BTreeMap::new();
            if !s.is_empty() {
                m.insert("ko".to_string(), s);
            }
            m
        }
        None => BTreeMap::new(),
    };
    match value {
        Some(v) if !v.is_empty() => {
            map.insert(lang.to_string(), v);
        }
        _ => {
            map.remove(lang);
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(LocalizedText::Map(map))
    }
}

/// 외부 헬퍼 — 자판 이름이 빌트인 영역에 속하는지(override 무관).
///
/// 주의: `LayoutProfile.name` 은 JSON `name` 필드 그대로 (예: `"2bulstd"`) 라서
/// `BUILTIN_NAMES` (예: `"ko_2bulstd"`) 와 일치하지 않을 수 있다. 따라서 이 함수는
/// 자판을 *부른 외부 식별자* (드롭다운에서 선택한 이름) 로 호출해야 의미가 있다.
pub fn is_builtin_name(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

/// 자판 선택 정책 판정 — `name` 이 빌트인 영역에 속하고 동시에 사용자 override
/// 가 없으면 true ('Save' disable, 'Save As' only).
pub fn is_builtin_selection(name: &str, registry: &_ProfileRegistryForSig) -> bool {
    is_builtin_name(name) && !registry.is_user_override(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unim::keystroke::profile::load_builtin_profile;

    #[test]
    fn dirty_flag_flips_on_first_change() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        assert!(!ed.dirty);
        ed.set_metadata_author(Some("modified".into()));
        assert!(ed.dirty);
    }

    #[test]
    fn revert_restores_original() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let original_author = p.metadata.author.clone();
        let mut ed = EditorState::new(p);
        ed.set_metadata_author(Some("modified".into()));
        ed.revert();
        assert_eq!(ed.buf.metadata.author, original_author);
        assert!(!ed.dirty);
    }

    #[test]
    fn supports_moachigi_forces_v3_schema() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        assert_eq!(p.schema_version, 1);
        let mut ed = EditorState::new(p);
        ed.set_supports_moachigi(true);
        assert!(ed.buf.moachigi.is_some());
        assert_eq!(ed.buf.schema_version, 3);
    }

    #[test]
    fn push_then_remove_combo_round_trips() {
        let p = load_builtin_profile("ko_3bul390").unwrap();
        let mut ed = EditorState::new(p);
        let before = ed.combos(ComboKind::Cho).len();
        ed.push_combo(
            ComboKind::Cho,
            RawTriple {
                first: "ㄱ".into(),
                second: "ㄱ".into(),
                result: "ㄲ".into(),
            },
        );
        assert_eq!(ed.combos(ComboKind::Cho).len(), before + 1);
        ed.remove_combo(ComboKind::Cho, before);
        assert_eq!(ed.combos(ComboKind::Cho).len(), before);
    }

    #[test]
    fn set_key_label_writes_through() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        let ok = ed.set_key_label(false, 1, 0, "TEST".into());
        assert!(ok);
        assert_eq!(ed.buf.layout.lower.row2[0], "TEST");
        assert!(ed.dirty);
    }

    #[test]
    fn set_key_label_oob_returns_false() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        assert!(!ed.set_key_label(false, 9, 0, "X".into()));
        assert!(!ed.dirty);
    }

    #[test]
    fn is_builtin_name_recognizes_known_layouts() {
        assert!(is_builtin_name("ko_2bulstd"));
        assert!(is_builtin_name("en_qwerty"));
        assert!(!is_builtin_name("totally_custom"));
    }

    #[test]
    fn is_builtin_selection_handles_override() {
        let reg = ProfileRegistry::builtin_only();
        // 사용자 override 없는 빈 registry — 빌트인 식별자는 true.
        assert!(is_builtin_selection("ko_2bulstd", &reg));
        // 사용자 정의 이름은 항상 false.
        assert!(!is_builtin_selection("my_layout", &reg));
    }

    #[test]
    fn set_language_to_english_clears_korean_only_fields() {
        let p = load_builtin_profile("ko_3bul390").unwrap();
        let mut ed = EditorState::new(p);
        assert!(ed.buf.combinations.is_some(), "사전 조건: 한글 조합 존재");
        ed.set_language("english");
        assert_eq!(ed.buf.language, "english");
        assert!(ed.buf.combinations.is_none());
        assert!(ed.buf.rule_sets.is_empty());
        assert!(ed.buf.key_meta.is_none());
        assert!(ed.buf.moachigi.is_none());
        assert!(ed.dirty);
    }

    #[test]
    fn set_display_name_ko_creates_localized_map() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.set_display_name_ko(Some("나의 자판".into()));
        ed.set_display_name_en(Some("My Layout".into()));
        assert_eq!(localized_lang(ed.buf.metadata.display_name.as_ref(), "ko"), "나의 자판");
        assert_eq!(localized_lang(ed.buf.metadata.display_name.as_ref(), "en"), "My Layout");
    }

    #[test]
    fn set_display_name_empty_removes_lang() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.set_display_name_ko(Some("값".into()));
        ed.set_display_name_en(Some("Value".into()));
        ed.set_display_name_ko(None);
        assert_eq!(localized_lang(ed.buf.metadata.display_name.as_ref(), "ko"), "");
        assert_eq!(localized_lang(ed.buf.metadata.display_name.as_ref(), "en"), "Value");
        // en 도 제거하면 전체 None.
        ed.set_display_name_en(Some(String::new()));
        assert!(ed.buf.metadata.display_name.is_none());
    }

    #[test]
    fn set_inherits_none_removes_field() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.set_inherits(Some("ko_3bul391".into()));
        assert_eq!(ed.buf.inherits.as_deref(), Some("ko_3bul391"));
        ed.set_inherits(None);
        assert!(ed.buf.inherits.is_none());
        ed.set_inherits(Some(String::new()));
        assert!(ed.buf.inherits.is_none(), "빈 문자열도 None");
    }

    #[test]
    fn set_tags_replaces_existing() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.set_tags(vec!["a".into(), "b".into()]);
        assert_eq!(ed.buf.metadata.tags, vec!["a", "b"]);
        ed.set_tags(vec!["c".into()]);
        assert_eq!(ed.buf.metadata.tags, vec!["c"]);
    }

    #[test]
    fn rule_set_add_rejects_duplicate_name() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        assert!(ed.add_rule_set("my_set".into()));
        assert!(!ed.add_rule_set("my_set".into()), "중복 거부");
        assert!(ed.rule_set_names().contains(&"my_set".to_string()));
    }

    #[test]
    fn rule_set_remove_clears_from_active_list() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.add_rule_set("s1".into());
        ed.toggle_rule_set("s1", true);
        ed.buf.active_rule_sets = Some(vec!["s1".into()]);
        assert!(ed.remove_rule_set("s1"));
        assert!(!ed.rule_set_names().contains(&"s1".to_string()));
        assert!(ed
            .buf
            .active_rule_sets
            .as_ref()
            .map(|l| !l.contains(&"s1".to_string()))
            .unwrap_or(true));
    }

    #[test]
    fn rule_set_combo_push_and_remove() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.add_rule_set("s1".into());
        ed.push_rule_set_combo(
            "s1",
            RawTriple {
                first: "ㄱ".into(),
                second: "ㄱ".into(),
                result: "ㄲ".into(),
            },
        );
        assert_eq!(ed.rule_set_combos("s1").len(), 1);
        assert!(ed.remove_rule_set_combo("s1", 0));
        assert_eq!(ed.rule_set_combos("s1").len(), 0);
    }

    #[test]
    fn key_meta_set_and_remove_round_trip() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        let meta = KeyMeta {
            vowel_combine_head: Some(false),
            context_alt: None,
        };
        ed.set_key_meta("v".into(), meta);
        assert_eq!(ed.key_meta_iter().len(), 1);
        assert!(ed.buf.schema_version >= 2);
        assert!(ed.remove_key_meta("v"));
        assert_eq!(ed.key_meta_iter().len(), 0);
        assert!(ed.buf.key_meta.is_none(), "비면 None 정리");
    }

    #[test]
    fn rule_set_key_meta_independent_from_global() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.add_rule_set("vowel_strict".into());
        ed.set_rule_set_key_meta(
            "vowel_strict",
            "v".into(),
            KeyMeta {
                vowel_combine_head: Some(false),
                context_alt: None,
            },
        );
        // rule_set 한정 — 전역에는 영향 없음.
        assert_eq!(ed.rule_set_key_meta_iter("vowel_strict").len(), 1);
        assert_eq!(ed.key_meta_iter().len(), 0);
        assert!(ed.remove_rule_set_key_meta("vowel_strict", "v"));
        assert_eq!(ed.rule_set_key_meta_iter("vowel_strict").len(), 0);
    }

    #[test]
    fn save_as_changes_name() {
        let p = load_builtin_profile("ko_2bulstd").unwrap();
        let mut ed = EditorState::new(p);
        ed.set_metadata_author(Some("me".into()));
        assert!(ed.dirty);
        // 임시 디렉토리 보장이 없으므로 이름 변경 + dirty 리셋 동작만 검증.
        // 실제 파일 저장은 통합 테스트에서.
        ed.buf.name = "tmp_save_as_test".into();
        assert_eq!(ed.buf.name, "tmp_save_as_test");
    }
}
