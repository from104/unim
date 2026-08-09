//! 앱 능력 티어 캐시 영속화.
//!
//! `text_service` 의 `cuas_windows`(즉시-terminate 브리지 학습)는
//! `Mutex<HashSet<isize>>`(HWND 키)인 인메모리 집합이라 세션 종료·창 재생성마다
//! 소실된다. 그래서 매 세션 최초 단어가 매번 "실험 대상"(composition 시도 →
//! 즉시-terminate → 폴백 학습)이 된다.
//!
//! 이 모듈은 능력 티어를 **프로세스 실행파일명 기준**으로
//! `%APPDATA%\unim\app_tiers.json` 에 영속화한다. HWND 는 세션마다 바뀌지만
//! 프로세스명(예 `wezterm.exe`)은 안정적이므로, 재기동 후 같은 앱에 포커스가
//! 가면 `text_service` 가 첫 키 이전에 `cuas_windows` 를 선제 주입해 첫 단어를
//! 실험 대상에서 제외한다.
//!
//! ## 비파괴성
//! 캐시는 **힌트**일 뿐이다. 오학습(정상 앱을 CUAS 로 잘못 기록)해도 확정 텍스트는
//! 정상 삽입되고(오버레이 폴백 경로는 파괴적이지 않음), `OnKeyDown` 의 단어 경계
//! 리셋(`composition_unsupported=false`)이 매 단어 composition 을 재시도하므로
//! 창당 자기수정이 유지된다.
//!
//! ## 저장 스로틀
//! 신규 티어 학습은 드물지만, 학습 폭주(원격 데스크톱 등)에서도 디스크 부담이
//! 없도록 저장을 **2초에 1회**로 조인다. 최초 학습은 즉시 저장(leading-edge)하고,
//! 스로틀에 걸린 변경은 dirty 로 남겨 다음 `maybe_flush`(포커스 전환 등)에서 반영한다.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// 저장 스로틀 최소 간격.
const SAVE_THROTTLE: Duration = Duration::from_secs(2);

/// 디스크 스키마(버전드). 향후 티어(예 word-only, shift-start 부족 등)를
/// 필드 추가로 확장할 수 있도록 구조체로 둔다.
#[derive(Serialize, Deserialize, Default)]
struct AppTiersFile {
    /// 스키마 버전(현재 1). 알 수 없는 버전은 로드 시 무시(빈 캐시).
    #[serde(default)]
    version: u32,
    /// composition 미지원(CUAS/즉시-terminate 브리지)으로 학습된 프로세스명
    /// (소문자 basename, 예 `wezterm.exe`).
    #[serde(default)]
    cuas: Vec<String>,
}

const SCHEMA_VERSION: u32 = 1;

/// 앱 능력 티어 인메모리 캐시 + 영속화 상태.
pub struct AppTierCache {
    /// CUAS(composition 미지원)로 학습된 프로세스명 집합.
    cuas: HashSet<String>,
    /// 미저장 변경 유무.
    dirty: bool,
    /// 마지막 디스크 저장 시각(스로틀 기준).
    last_save: Option<Instant>,
}

impl AppTierCache {
    /// 디스크에서 캐시를 로드한다. 파일 부재·파싱 실패·버전 불일치는 모두
    /// 빈 캐시로 폴백(비치명적) — 학습이 다시 시작될 뿐 파괴적이지 않다.
    pub fn load() -> Self {
        let mut cuas = HashSet::new();
        if let Some(path) = tiers_path() {
            match std::fs::read_to_string(&path) {
                Ok(text) => match serde_json::from_str::<AppTiersFile>(&text) {
                    Ok(file) if file.version == SCHEMA_VERSION => {
                        for name in file.cuas {
                            let name = name.to_ascii_lowercase();
                            if !name.is_empty() {
                                cuas.insert(name);
                            }
                        }
                        crate::register::dbg_log(&format!(
                            "app_tiers: 로드 완료 cuas={} (path={:?})",
                            cuas.len(),
                            path
                        ));
                    }
                    Ok(file) => {
                        crate::register::dbg_log(&format!(
                            "app_tiers: 스키마 버전 불일치(got={} want={}) → 빈 캐시",
                            file.version, SCHEMA_VERSION
                        ));
                    }
                    Err(e) => {
                        crate::register::dbg_log(&format!(
                            "app_tiers: JSON 파싱 실패({e}) → 빈 캐시"
                        ));
                    }
                },
                Err(_) => {
                    // 파일 부재(초기 실행)는 정상 — 조용히 빈 캐시.
                }
            }
        }
        Self {
            cuas,
            dirty: false,
            last_save: None,
        }
    }

    /// 프로세스명이 CUAS 티어로 학습돼 있는지.
    pub fn is_cuas(&self, proc_name: &str) -> bool {
        self.cuas.contains(&proc_name.to_ascii_lowercase())
    }

    /// 프로세스를 CUAS 티어로 기록한다. 신규 항목이면 dirty 로 표시하고
    /// 스로틀 경과 시 즉시 저장(leading-edge)한다.
    pub fn record_cuas(&mut self, proc_name: &str) {
        let name = proc_name.to_ascii_lowercase();
        if name.is_empty() {
            return;
        }
        if self.cuas.insert(name.clone()) {
            self.dirty = true;
            crate::register::dbg_log(&format!(
                "app_tiers: CUAS 학습 기록 proc='{}' (총 {})",
                name,
                self.cuas.len()
            ));
            self.maybe_flush();
        }
    }

    /// 미저장 변경이 있고 스로틀(2초)이 경과했으면 디스크에 저장한다.
    /// 포커스 전환 등 저비용 지점에서 주기적으로 호출해 지연된 저장을 반영한다.
    pub fn maybe_flush(&mut self) {
        if !self.dirty {
            return;
        }
        let now = Instant::now();
        let due = match self.last_save {
            None => true,
            Some(t) => now.duration_since(t) >= SAVE_THROTTLE,
        };
        if due {
            self.save_to_disk();
            self.last_save = Some(now);
            self.dirty = false;
        }
    }

    /// 현재 캐시를 JSON 으로 직렬화해 저장한다. 실패는 비치명적(로그만).
    fn save_to_disk(&self) {
        let Some(path) = tiers_path() else {
            crate::register::dbg_log("app_tiers: 저장 경로 결정 실패 → skip");
            return;
        };
        // 결정론적 출력(diff/디버깅 용이)을 위해 정렬.
        let mut cuas: Vec<String> = self.cuas.iter().cloned().collect();
        cuas.sort();
        let file = AppTiersFile {
            version: SCHEMA_VERSION,
            cuas,
        };
        let json = match serde_json::to_string_pretty(&file) {
            Ok(s) => s,
            Err(e) => {
                crate::register::dbg_log(&format!("app_tiers: 직렬화 실패({e}) → skip"));
                return;
            }
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&path, json) {
            Ok(()) => crate::register::dbg_log(&format!(
                "app_tiers: 저장 완료 cuas={} (path={:?})",
                self.cuas.len(),
                path
            )),
            Err(e) => crate::register::dbg_log(&format!(
                "app_tiers: 저장 실패({e}) path={path:?}"
            )),
        }
    }
}

/// `%APPDATA%\unim\app_tiers.json` 경로. config.yaml 과 동일한 베이스
/// (`unim::paths::config_dir()` → `unim/`)를 쓴다(UWP 리다이렉트 우회 env 포함).
fn tiers_path() -> Option<PathBuf> {
    unim::paths::config_dir().map(|p| p.join("unim").join("app_tiers.json"))
}
