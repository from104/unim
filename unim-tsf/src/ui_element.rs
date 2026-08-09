//! I8 — UILess Mode UIElement 접근성 노출 (조합 reading + 후보 candidate)
//!
//! NVDA·Narrator 등 스크린리더는 TSF `ITfUIElementMgr` 에 advise 한
//! `ITfUIElementSink` 로 UI element 의 show/update/hide 를 통지받고, 각 element 를
//! `ITfReadingInformationUIElement` / `ITfCandidateListUIElement` 로 QueryInterface
//! 해 **조합 문자열**·**후보 목록**을 읽어 음성으로 낭독한다. 본 모듈은 그 두
//! UI element 를 UILess Mode 로 구현·구동해 시각장애 사용자가 한자/이모지/특수문자
//! 후보와 조합 중 음절을 들을 수 있게 한다.
//!
//! ## 설계 원칙 (COM 경계 안전 최우선)
//! - 두 COM 객체(`ReadingInfoElement`·`CandidateListElement`)는 각자 `Arc<Mutex<State>>`
//!   스냅샷을 보유한다. TSF STA 스레드가 상태를 갱신(`sync_*`)한 뒤
//!   `BeginUIElement`/`UpdateUIElement`/`EndUIElement` 를 호출하면, 스크린리더의
//!   sink 콜백이 같은 스냅샷을 읽는다. 엔진 락과 독립이라 재진입 데드락이 없다.
//! - **모든 COM-facing 메서드는 절대 패닉하지 않는다** — `Mutex` poison 시에도
//!   안전한 기본값(빈 문자열/0)을 반환한다. `panic=abort` 하에서 호스트 앱을
//!   조용히 죽이지 않기 위함이다.
//! - 팝업 out-of-process 렌더링은 **무변경** — 본 모듈은 접근성 통지만 추가하며
//!   `BeginUIElement` 의 `pbShow` 출력(자체 UI 그리기 여부)은 무시한다.
//!
//! ## 알려진 한계 (followup)
//! - 후보 목록은 **현재 페이지**만 노출한다(와이어 프로토콜이 한 페이지만 전송).
//!   전체 다중 페이지 후보 flat 목록은 프로토콜 확장 후 followup. 페이지 전환은
//!   매번 `UpdateUIElement` 로 재통지되어 스크린리더가 새 페이지를 다시 읽는다.
//! - `GetDocumentMgr` 는 `E_FAIL`(NVDA/Narrator 미사용). 필요 시 followup.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use windows::core::*;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::TextServices::*;

use unim::popup::{PopupKind, PopupViewModel};

use crate::register::dbg_log;

// ── UI element 식별 GUID (TIP 사설·안정). 스크린리더는 QI 로 타입을 판별하므로
//    값 자체는 유일·불변이기만 하면 된다. UNIM_CLSID/Profile 과 비충돌. ─────────
const UNIM_UIE_READING_GUID: GUID = GUID::from_u128(0xE5C4A3B2_1D0F_4A6B_9C8D_7E6F5A4B3C21);
const UNIM_UIE_CANDIDATE_GUID: GUID = GUID::from_u128(0xF6D5B4C3_2E1A_4B7C_8D9E_6F5A4B3C2D10);

/// UIElement id 미할당 표식.
const NO_ID: u32 = u32::MAX;

// ════════════════════════════════════════════════════════════════════════════
// 후보 스냅샷 — 엔진 view model → 접근성 flat 목록
// ════════════════════════════════════════════════════════════════════════════

/// 현재 페이지 후보의 접근성 스냅샷 (COM 객체가 소비).
#[derive(Debug, Clone, Default)]
pub struct CandidateSnapshot {
    /// 낭독용 후보 문자열(한자는 "韓 나라 한" 처럼 뜻 포함). row-major 순서.
    pub items: Vec<String>,
    /// 현재 선택 인덱스(`items` 기준).
    pub selection: u32,
    /// 현재 페이지(0-based, 설명용).
    pub current_page: u32,
    /// 전체 페이지 수(설명용).
    pub total_pages: u32,
    /// 0=한자,1=특수문자,2=이모지 (설명 라벨용).
    pub kind: u32,
}

/// 엔진 `PopupViewModel` 을 접근성 후보 스냅샷으로 평탄화한다.
///
/// 셀을 **row-major**(행 우선)로 순회해 비어있지 않은 셀만 목록에 담고, 선택 셀의
/// flat 인덱스를 함께 산출한다. 한자는 뜻(`meaning`)을 붙여 동음이의 한자를
/// 음성으로 구분할 수 있게 한다(접근성 핵심 가치).
pub fn candidate_snapshot_from_view_model(vm: &PopupViewModel) -> CandidateSnapshot {
    let mut items: Vec<String> = Vec::new();
    let mut selection: u32 = 0;
    for row in &vm.cells {
        for cell in row {
            if let Some(cd) = cell.as_ref() {
                if cd.is_selected {
                    selection = items.len() as u32;
                }
                let label = match cd.meaning.as_deref() {
                    Some(m) if !m.is_empty() => format!("{} {}", cd.text, m),
                    _ => cd.text.clone(),
                };
                items.push(label);
            }
        }
    }
    let kind = match vm.kind {
        PopupKind::Hanja => 0,
        PopupKind::SpecialChar => 1,
        PopupKind::Emoji => 2,
    };
    CandidateSnapshot {
        items,
        selection,
        current_page: vm.current_page as u32,
        total_pages: vm.total_pages as u32,
        kind,
    }
}

fn kind_label(kind: u32) -> &'static str {
    match kind {
        0 => "한자",
        1 => "특수문자",
        2 => "이모지",
        _ => "후보",
    }
}

// ════════════════════════════════════════════════════════════════════════════
// (a) ITfReadingInformationUIElement — 조합(reading) 문자열 노출
// ════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct ReadingState {
    /// 현재 조합 문자열(엔진 preedit).
    text: String,
    /// 표시 중 여부.
    shown: bool,
    /// 마지막 갱신 플래그(TF_RIUIE_*).
    updated_flags: u32,
    /// 연관 컨텍스트(GetContext 반환용). 없으면 None → E_FAIL.
    context: Option<ITfContext>,
}

#[implement(ITfReadingInformationUIElement)]
struct ReadingInfoElement {
    state: Arc<Mutex<ReadingState>>,
}

impl ReadingInfoElement {
    fn new(state: Arc<Mutex<ReadingState>>) -> Self {
        Self { state }
    }
}

impl ITfUIElement_Impl for ReadingInfoElement_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        let text = self
            .state
            .lock()
            .map(|s| s.text.clone())
            .unwrap_or_default();
        Ok(BSTR::from(format!("UNIM 조합: {text}").as_str()))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(UNIM_UIE_READING_GUID)
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        // UILess: 자체 창을 그리지 않으므로 상태 미러만 갱신(접근성 통지 목적).
        if let Ok(mut s) = self.state.lock() {
            s.shown = bshow.as_bool();
        }
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        let shown = self.state.lock().map(|s| s.shown).unwrap_or(false);
        Ok(BOOL::from(shown))
    }
}

impl ITfReadingInformationUIElement_Impl for ReadingInfoElement_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(self.state.lock().map(|s| s.updated_flags).unwrap_or(0))
    }

    fn GetContext(&self) -> Result<ITfContext> {
        match self.state.lock() {
            Ok(s) => s.context.clone().ok_or_else(|| E_FAIL.into()),
            Err(_) => Err(E_FAIL.into()),
        }
    }

    fn GetString(&self) -> Result<BSTR> {
        let text = self
            .state
            .lock()
            .map(|s| s.text.clone())
            .unwrap_or_default();
        Ok(BSTR::from(text.as_str()))
    }

    fn GetMaxReadingStringLength(&self) -> Result<u32> {
        // 조합 음절 상한(넉넉히). 스크린리더 버퍼 힌트일 뿐.
        Ok(64)
    }

    fn GetErrorIndex(&self) -> Result<u32> {
        // 조합 오류 없음 → 문자열 길이(오류 하이라이트 없음 의미).
        let len = self
            .state
            .lock()
            .map(|s| s.text.chars().count() as u32)
            .unwrap_or(0);
        Ok(len)
    }

    fn IsVerticalOrderPreferred(&self) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// (b) ITfCandidateListUIElement — 후보 목록 노출
// ════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct CandidateElemState {
    snap: CandidateSnapshot,
    shown: bool,
    updated_flags: u32,
}

#[implement(ITfCandidateListUIElement)]
struct CandidateListElement {
    state: Arc<Mutex<CandidateElemState>>,
}

impl CandidateListElement {
    fn new(state: Arc<Mutex<CandidateElemState>>) -> Self {
        Self { state }
    }
}

impl ITfUIElement_Impl for CandidateListElement_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        let desc = match self.state.lock() {
            Ok(s) => format!(
                "UNIM {} 후보 {}/{}",
                kind_label(s.snap.kind),
                s.snap.current_page.saturating_add(1),
                s.snap.total_pages.max(1)
            ),
            Err(_) => "UNIM 후보".to_string(),
        };
        Ok(BSTR::from(desc.as_str()))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(UNIM_UIE_CANDIDATE_GUID)
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        if let Ok(mut s) = self.state.lock() {
            s.shown = bshow.as_bool();
        }
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        let shown = self.state.lock().map(|s| s.shown).unwrap_or(false);
        Ok(BOOL::from(shown))
    }
}

impl ITfCandidateListUIElement_Impl for CandidateListElement_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(self.state.lock().map(|s| s.updated_flags).unwrap_or(0))
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        // NVDA/Narrator 미사용. 연관 문서 매니저 노출은 followup.
        Err(E_FAIL.into())
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self
            .state
            .lock()
            .map(|s| s.snap.items.len() as u32)
            .unwrap_or(0))
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(self.state.lock().map(|s| s.snap.selection).unwrap_or(0))
    }

    fn GetString(&self, uindex: u32) -> Result<BSTR> {
        let s = match self.state.lock() {
            Ok(g) => g,
            Err(_) => return Ok(BSTR::new()),
        };
        // 범위 밖은 빈 문자열(에러 전파로 클라이언트가 막히지 않게).
        match s.snap.items.get(uindex as usize) {
            Some(item) => Ok(BSTR::from(item.as_str())),
            None => Ok(BSTR::new()),
        }
    }

    fn GetPageIndex(&self, pindex: *mut u32, usize_: u32, pupagecnt: *mut u32) -> Result<()> {
        // 단일 논리 페이지 모델(현재 페이지만 노출). 시작 인덱스 = [0], 페이지 수 = 1.
        unsafe {
            if !pindex.is_null() && usize_ >= 1 {
                *pindex = 0;
            }
            if !pupagecnt.is_null() {
                *pupagecnt = 1;
            }
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> Result<()> {
        // 외부 페이지 지정은 무시(엔진이 SoT). 성공 반환.
        Ok(())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        // 단일 논리 페이지 모델 → 항상 0(GetPageIndex 와 정합).
        Ok(0)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// UiElements — TextService 가 보유하는 UILess 구동 매니저
// ════════════════════════════════════════════════════════════════════════════

/// 조합·후보 UI element 2종을 소유하고 `ITfUIElementMgr` 로 show/update/hide 를
/// 구동한다. TSF STA 스레드 전용(TextService 가 `Mutex` 로 감쌈).
pub struct UiElements {
    reading_state: Arc<Mutex<ReadingState>>,
    /// lazily 생성되는 reading element(ITfUIElement 로 보관, QI 로 파생 인터페이스 접근).
    reading_elem: Option<ITfUIElement>,
    /// 현재 할당된 UIElement id(NO_ID = 미표시).
    reading_id: AtomicU32,

    cand_state: Arc<Mutex<CandidateElemState>>,
    cand_elem: Option<ITfUIElement>,
    cand_id: AtomicU32,
}

impl Default for UiElements {
    fn default() -> Self {
        Self::new()
    }
}

impl UiElements {
    pub fn new() -> Self {
        Self {
            reading_state: Arc::new(Mutex::new(ReadingState::default())),
            reading_elem: None,
            reading_id: AtomicU32::new(NO_ID),
            cand_state: Arc::new(Mutex::new(CandidateElemState::default())),
            cand_elem: None,
            cand_id: AtomicU32::new(NO_ID),
        }
    }

    /// 조합(reading) 동기화. `text`=Some(비어있지 않음) → show/update, 그 외 → hide.
    pub fn sync_reading(
        &mut self,
        mgr: &ITfUIElementMgr,
        text: Option<&str>,
        ctx: Option<&ITfContext>,
    ) {
        let showing = self.reading_id.load(Ordering::SeqCst) != NO_ID;
        match text {
            Some(s) if !s.is_empty() => {
                // 이미 같은 문자열로 표시 중이면 재통지 생략(중복 낭독 방지).
                if showing {
                    let same = self
                        .reading_state
                        .lock()
                        .map(|st| st.text == s)
                        .unwrap_or(false);
                    if same {
                        return;
                    }
                }
                if let Ok(mut st) = self.reading_state.lock() {
                    st.text = s.to_string();
                    st.context = ctx.cloned();
                    st.updated_flags = TF_RIUIE_STRING;
                    st.shown = true;
                }
                self.ensure_reading_elem();
                if showing {
                    self.update_reading(mgr);
                } else {
                    self.begin_reading(mgr);
                }
            }
            _ => {
                if let Ok(mut st) = self.reading_state.lock() {
                    st.shown = false;
                    st.text.clear();
                }
                if showing {
                    self.end_reading(mgr);
                }
            }
        }
    }

    /// 후보(candidate) 동기화. `snap`=Some → show/update, None → hide.
    pub fn sync_candidate(&mut self, mgr: &ITfUIElementMgr, snap: Option<&CandidateSnapshot>) {
        let showing = self.cand_id.load(Ordering::SeqCst) != NO_ID;
        match snap {
            Some(sn) if !sn.items.is_empty() => {
                if let Ok(mut st) = self.cand_state.lock() {
                    st.snap = sn.clone();
                    st.shown = true;
                    st.updated_flags = TF_CLUIE_STRING
                        | TF_CLUIE_SELECTION
                        | TF_CLUIE_COUNT
                        | TF_CLUIE_CURRENTPAGE
                        | TF_CLUIE_PAGEINDEX;
                }
                self.ensure_cand_elem();
                if showing {
                    self.update_candidate(mgr);
                } else {
                    self.begin_candidate(mgr);
                }
            }
            _ => {
                if let Ok(mut st) = self.cand_state.lock() {
                    st.shown = false;
                    st.snap = CandidateSnapshot::default();
                }
                if showing {
                    self.end_candidate(mgr);
                }
            }
        }
    }

    /// 모든 UI element 종료(비활성화·포커스 이탈·설정 리로드 시). 멱등.
    pub fn end_all(&mut self, mgr: &ITfUIElementMgr) {
        if self.reading_id.load(Ordering::SeqCst) != NO_ID {
            if let Ok(mut st) = self.reading_state.lock() {
                st.shown = false;
                st.text.clear();
                st.context = None;
            }
            self.end_reading(mgr);
        }
        if self.cand_id.load(Ordering::SeqCst) != NO_ID {
            if let Ok(mut st) = self.cand_state.lock() {
                st.shown = false;
                st.snap = CandidateSnapshot::default();
            }
            self.end_candidate(mgr);
        }
        // COM 참조 해제(다음 활성화에서 재생성). 매니저가 이미 End 로 자기 참조를
        // 풀었으므로 여기서 우리 참조를 놓으면 객체가 정리된다.
        self.reading_elem = None;
        self.cand_elem = None;
    }

    // ── 내부 헬퍼 ──────────────────────────────────────────────────────────

    fn ensure_reading_elem(&mut self) {
        if self.reading_elem.is_none() {
            let ri: ITfReadingInformationUIElement =
                ReadingInfoElement::new(Arc::clone(&self.reading_state)).into();
            match ri.cast::<ITfUIElement>() {
                Ok(ui) => self.reading_elem = Some(ui),
                Err(e) => dbg_log(&format!("ui_element: reading cast 실패 {e:?}")),
            }
        }
    }

    fn ensure_cand_elem(&mut self) {
        if self.cand_elem.is_none() {
            let cl: ITfCandidateListUIElement =
                CandidateListElement::new(Arc::clone(&self.cand_state)).into();
            match cl.cast::<ITfUIElement>() {
                Ok(ui) => self.cand_elem = Some(ui),
                Err(e) => dbg_log(&format!("ui_element: candidate cast 실패 {e:?}")),
            }
        }
    }

    fn begin_reading(&self, mgr: &ITfUIElementMgr) {
        let elem = match self.reading_elem.as_ref() {
            Some(e) => e,
            None => return,
        };
        let mut show = BOOL::from(true);
        let mut id: u32 = 0;
        // pbShow 출력은 무시(팝업/조합은 UNIM 이 별도로 표시). 접근성 통지만 목적.
        let r = unsafe { mgr.BeginUIElement(elem, &mut show, &mut id) };
        match r {
            Ok(()) => {
                self.reading_id.store(id, Ordering::SeqCst);
                dbg_log(&format!("ui_element: reading BeginUIElement id={id}"));
            }
            Err(e) => dbg_log(&format!("ui_element: reading Begin 실패 {e:?}")),
        }
    }

    fn update_reading(&self, mgr: &ITfUIElementMgr) {
        let id = self.reading_id.load(Ordering::SeqCst);
        if id == NO_ID {
            return;
        }
        if let Err(e) = unsafe { mgr.UpdateUIElement(id) } {
            dbg_log(&format!("ui_element: reading Update 실패 {e:?}"));
        }
    }

    fn end_reading(&self, mgr: &ITfUIElementMgr) {
        let id = self.reading_id.swap(NO_ID, Ordering::SeqCst);
        if id == NO_ID {
            return;
        }
        if let Err(e) = unsafe { mgr.EndUIElement(id) } {
            dbg_log(&format!("ui_element: reading End 실패 {e:?}"));
        }
    }

    fn begin_candidate(&self, mgr: &ITfUIElementMgr) {
        let elem = match self.cand_elem.as_ref() {
            Some(e) => e,
            None => return,
        };
        let mut show = BOOL::from(true);
        let mut id: u32 = 0;
        let r = unsafe { mgr.BeginUIElement(elem, &mut show, &mut id) };
        match r {
            Ok(()) => {
                self.cand_id.store(id, Ordering::SeqCst);
                dbg_log(&format!("ui_element: candidate BeginUIElement id={id}"));
            }
            Err(e) => dbg_log(&format!("ui_element: candidate Begin 실패 {e:?}")),
        }
    }

    fn update_candidate(&self, mgr: &ITfUIElementMgr) {
        let id = self.cand_id.load(Ordering::SeqCst);
        if id == NO_ID {
            return;
        }
        if let Err(e) = unsafe { mgr.UpdateUIElement(id) } {
            dbg_log(&format!("ui_element: candidate Update 실패 {e:?}"));
        }
    }

    fn end_candidate(&self, mgr: &ITfUIElementMgr) {
        let id = self.cand_id.swap(NO_ID, Ordering::SeqCst);
        if id == NO_ID {
            return;
        }
        if let Err(e) = unsafe { mgr.EndUIElement(id) } {
            dbg_log(&format!("ui_element: candidate End 실패 {e:?}"));
        }
    }
}
