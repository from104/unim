//! 한자 사전 합친 조회 — 내장 사전 + 사용자 층(HANJA_USERDICT_SPEC §3).
//!
//! 엔진의 모든 한자 조회는 이 헬퍼를 지난다(`hanja_dict.search/contains` 직접 호출 금지).
//! 사용자 층은 `Arc<HanjaUserIndex>` 스냅샷이며, 새 팝업을 여는 한자키
//! (`start_hanja_conversion` 의 비-한자모드 분기)에서만 갱신한다.

use super::InputEngine;
use crate::hanja::user_dict::{merge_candidates, shared_index};
use crate::hanja::HanjaEntry;

impl InputEngine {
    /// 사용자 항목 → 내장 항목(같은 한자 제외) 순의 후보. 사용자 층이 비면 내장과 동일.
    pub(super) fn hanja_search(&self, key: &str) -> Vec<HanjaEntry> {
        merge_candidates(self.hanja_user.get(key), self.hanja_dict.search(key))
    }

    /// 사용자 층 또는 내장 사전에 키가 있는가.
    pub(super) fn hanja_contains(&self, key: &str) -> bool {
        self.hanja_user.contains(key) || self.hanja_dict.contains(key)
    }

    /// 사용자 층 스냅샷 갱신(새 팝업을 여는 한자키에서만 호출). 단위 시험에서 층을
    /// 고정(pin)했으면 무시한다.
    pub(super) fn refresh_hanja_user_layer(&mut self) {
        #[cfg(test)]
        if self.hanja_user_pinned {
            return;
        }
        self.hanja_user = shared_index();
    }

    /// 시험용: 이 엔진의 사용자 층을 고정한다(전역 override 와 무관 — 병렬 시험 안전).
    #[cfg(test)]
    pub(super) fn pin_hanja_user_layer(&mut self, idx: std::sync::Arc<crate::hanja::HanjaUserIndex>) {
        self.hanja_user = idx;
        self.hanja_user_pinned = true;
    }
}
