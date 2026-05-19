//! 편집 탭의 가변 버퍼.
//!
//! `LayoutProfile`을 사본으로 받아 GUI에서 직접 mutate하다가 "저장" 시
//! `unim_keymap_common::save_profile_json`을 통해 사용자 디렉토리에 직렬화한다.

use std::collections::BTreeMap;
use std::path::PathBuf;

use unim::keystroke::profile::{
    CombinationsBlock, LayoutProfile, LocalizedText, MoachigiSpec, RawTriple, RuleSet,
};
use unim_keymap_common::save_profile_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// 일부 메서드는 후속 phase(combinations 편집 GUI)에서만 사용된다.
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
}
