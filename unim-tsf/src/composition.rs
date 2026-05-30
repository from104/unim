//! TSF 조합(Composition) 관리

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};

use windows::core::*;
use windows::core::BOOL;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::TextServices::*;

/// SetText 후 문서 selection(caret) 을 range 끝으로 이동한다.
///
/// TSF 는 `ITfRange::SetText` 만으로는 문서의 caret 을 옮기지 않는다. 옮기지 않으면
/// 다음 `InsertTextAtSelection` 이 직전 텍스트 "앞"에서 일어나 음절이 거꾸로 쌓인다
/// ("안녕" → "녕안"). 매 SetText 직후 호출해 방지한다.
unsafe fn move_caret_to_end(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    let end = range.Clone()?;
    end.Collapse(ec, TF_ANCHOR_END)?;
    let mut sel = TF_SELECTION {
        range: ManuallyDrop::new(Some(end)),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_END,
            fInterimChar: BOOL(0),
        },
    };
    let r = context.SetSelection(ec, std::slice::from_ref(&sel));
    // 우리가 만든 ITfRange 참조 해제 (SetSelection 은 내부에서 AddRef 한다)
    ManuallyDrop::drop(&mut sel.range);
    r
}

/// 텍스트를 삽입할 range 를 얻는다.
///
/// 1순위로 `ITfInsertAtSelection::InsertTextAtSelection(QUERYONLY)` 를 시도하고,
/// 실패하면 현재 selection range 로 폴백한다. 콘솔 호스트(conhost / Windows
/// Terminal / wezterm)는 `ITfInsertAtSelection` 을 지원하지 않아 E_FAIL /
/// TF_E_DISCONNECTED 를 돌려주는 경우가 많은데, 폴백이 없으면 조합 자체가
/// 조용히 실패한다(= wezterm 에서 한글 입력 안 됨). GUI 클라이언트는 1순위
/// 경로를 그대로 타므로 동작 변화 없음. (SampleIME `_InsertAtSelection` 패턴)
unsafe fn acquire_insert_range(context: &ITfContext, ec: u32) -> Result<ITfRange> {
    if let Ok(insert) = context.cast::<ITfInsertAtSelection>() {
        if let Ok(range) = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[]) {
            return Ok(range);
        }
    }
    // 폴백: 현재 selection range 를 직접 사용
    let mut sel = TF_SELECTION::default();
    let mut fetched: u32 = 0;
    context.GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched)?;
    let result = if fetched != 0 {
        sel.range
            .as_ref()
            .map(|r| r.clone())
            .ok_or_else(|| Error::from(E_FAIL))
    } else {
        Err(Error::from(E_FAIL))
    };
    // GetSelection 이 채운 참조 해제 (위에서 clone 으로 새 참조 확보)
    ManuallyDrop::drop(&mut sel.range);
    result
}

/// 조합 상태 관리자
pub struct CompositionManager {
    composition: Option<ITfComposition>,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
}

impl CompositionManager {
    pub fn new() -> Self {
        Self {
            composition: None,
            composition_slot: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.composition.is_some()
    }
    pub fn clear(&mut self) {
        self.composition = None;
    }

    pub fn start_composition(
        &mut self,
        context: &ITfContext,
        tid: u32,
        text: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;

        let session = StartCompositionEditSession {
            context: context.clone(),
            text: text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }

        let result = self.composition_slot.lock().unwrap().take();
        if let Some(comp) = result {
            self.composition = Some(comp);
        }
    }

    pub fn update_composition(&self, context: &ITfContext, tid: u32, text: &str) {
        if let Some(ref composition) = self.composition {
            let session = UpdateCompositionEditSession {
                context: context.clone(),
                text: text.to_string(),
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ =
                    context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
    }

    pub fn end_composition_with_text(&mut self, context: &ITfContext, tid: u32, text: &str) {
        if let Some(ref composition) = self.composition {
            let session = EndCompositionEditSession {
                context: context.clone(),
                text: Some(text.to_string()),
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ =
                    context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
        self.composition = None;
    }

    pub fn end_composition(&mut self, context: &ITfContext, tid: u32) {
        if let Some(ref composition) = self.composition {
            let session = EndCompositionEditSession {
                context: context.clone(),
                text: None,
                composition: composition.clone(),
            };
            let session_intf: ITfEditSession = session.into();
            unsafe {
                let _ =
                    context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            }
        }
        self.composition = None;
    }

    pub fn insert_text(&self, context: &ITfContext, tid: u32, text: &str) {
        let session = InsertTextEditSession {
            context: context.clone(),
            text: text.to_string(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
    }

    /// AutoTypeFix 교체: 커서 앞 delete_chars 를 삭제하고 commit_text 삽입.
    /// preedit 이 있으면 삽입 후 composition 을 시작하고 self.composition 을 갱신.
    ///
    /// RequestEditSession 거부(TF_E_NOLOCK 등) 시 조용히 skip (패닉 없음).
    pub fn replace_surrounding(
        &mut self,
        context: &ITfContext,
        tid: u32,
        delete_chars: u32,
        commit_text: &str,
        preedit_text: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;

        let session = ReplaceSurroundingEditSession {
            context: context.clone(),
            delete_chars: delete_chars as i32,
            commit_text: commit_text.to_string(),
            preedit_text: preedit_text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            // 거부 시 그대로 무시 (skip)
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }

        // preedit 있으면 composition 슬롯에서 꺼내 보관
        if !preedit_text.is_empty() {
            let result = self.composition_slot.lock().unwrap().take();
            if let Some(comp) = result {
                self.composition = Some(comp);
            }
        } else {
            // preedit 없음 — 기존 composition 이 있으면 종료됐으므로 clear
            self.composition = None;
        }
    }
}

// ── EditSession: 조합 시작 ──

#[implement(ITfEditSession)]
struct StartCompositionEditSession {
    context: ITfContext,
    text: String,
    comp_sink: ITfCompositionSink,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
}

impl ITfEditSession_Impl for StartCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            // 콘솔(wezterm 등) 폴백 포함 삽입 range 획득
            let range = acquire_insert_range(&self.context, ec)?;
            range.SetText(ec, 0, &wide)?;

            let ctx_comp: ITfContextComposition = self.context.cast()?;
            let composition = ctx_comp.StartComposition(ec, &range, &self.comp_sink)?;
            // caret 을 조합 텍스트 끝으로 이동 (거꾸로 입력 방지)
            let _ = move_caret_to_end(&self.context, ec, &range);
            *self.composition_slot.lock().unwrap() = Some(composition);
        }
        Ok(())
    }
}

// ── EditSession: 조합 갱신 ──

#[implement(ITfEditSession)]
struct UpdateCompositionEditSession {
    context: ITfContext,
    text: String,
    composition: ITfComposition,
}

impl ITfEditSession_Impl for UpdateCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, 0, &wide)?;
            // caret 을 조합 텍스트 끝으로 이동 (거꾸로 입력 방지)
            let _ = move_caret_to_end(&self.context, ec, &range);
        }
        Ok(())
    }
}

// ── EditSession: 조합 종료 ──

#[implement(ITfEditSession)]
struct EndCompositionEditSession {
    context: ITfContext,
    text: Option<String>,
    composition: ITfComposition,
}

impl ITfEditSession_Impl for EndCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            match self.text {
                Some(ref text) => {
                    // 확정: composition range 를 확정 텍스트로 교체.
                    let wide: Vec<u16> = text.encode_utf16().collect();
                    range.SetText(ec, 0, &wide)?;
                    // caret 을 확정 텍스트 끝으로 이동 (거꾸로 입력 방지).
                    let _ = move_caret_to_end(&self.context, ec, &range);
                }
                None => {
                    // 취소: composition range 에 남은 preedit 텍스트를 반드시 비운다.
                    // EndComposition 은 조합만 끝낼 뿐 range 텍스트를 지우지 않으므로,
                    // 비우지 않으면 마지막 남은 자모가 문서에 그대로 커밋된다
                    // (preedit 에서 backspace 로 마지막 자모를 지웠을 때의 버그).
                    range.SetText(ec, 0, &[])?;
                    let _ = move_caret_to_end(&self.context, ec, &range);
                }
            }
            self.composition.EndComposition(ec)?;
        }
        Ok(())
    }
}

// ── EditSession: 주변 텍스트 교체 (AutoTypeFix용) ──

/// delete_chars 글자를 커서 앞에서 삭제하고 commit_text 를 삽입.
/// preedit 문자열이 있으면 commit_text 삽입 후 composition 을 시작한다.
///
/// ITfRange ShiftStart 방식 — backspace 합성 없이 직접 범위를 조작.
/// RequestEditSession 거부(TF_E_*) 또는 shifted 부족 시 조용히 skip.
#[implement(ITfEditSession)]
pub struct ReplaceSurroundingEditSession {
    pub context: ITfContext,
    pub delete_chars: i32,
    pub commit_text: String,
    /// 순방향 replay preedit. 비어있으면 composition 시작 안 함.
    pub preedit_text: String,
    pub comp_sink: ITfCompositionSink,
    /// composition 결과를 돌려받는 슬롯 (preedit 있을 때만 사용)
    pub composition_slot: std::sync::Arc<std::sync::Mutex<Option<ITfComposition>>>,
}

impl ITfEditSession_Impl for ReplaceSurroundingEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> windows::core::Result<()> {
        unsafe {
            // 1. 커서 위치 범위 획득 (QUERYONLY 으로 삽입 위치 탐색)
            let insert: ITfInsertAtSelection = self.context.cast()?;
            let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;

            // 2. 커서 앞으로 ShiftStart (-delete_chars)
            if self.delete_chars > 0 {
                range.Collapse(ec, TF_ANCHOR_START)?;
                let mut shifted: i32 = 0;
                // ShiftStart 는 실제 이동량을 shifted 에 돌려줌. 경계 도달 시 부분 이동.
                let _ = range.ShiftStart(
                    ec,
                    -(self.delete_chars),
                    &mut shifted,
                    std::ptr::null(),
                );
                // 실제로 이동한 양이 요청보다 적으면 데이터 보호: 이동된 만큼만 삭제
                // (shifted 는 음수 또는 0으로 반환될 수 있음)
                // 삭제 — SetText(&[]) 로 범위를 비움
                range.SetText(ec, 0, &[])?;
            }

            // 3. commit_text 삽입
            if !self.commit_text.is_empty() {
                let wide: Vec<u16> = self.commit_text.encode_utf16().collect();
                range.SetText(ec, 0, &wide)?;
                // 커서를 삽입된 텍스트 끝으로 이동
                range.Collapse(ec, TF_ANCHOR_END)?;
                let _ = move_caret_to_end(&self.context, ec, &range);
            }

            // 4. preedit 이 있으면 composition 시작 (순방향 replay)
            if !self.preedit_text.is_empty() {
                let wide: Vec<u16> = self.preedit_text.encode_utf16().collect();
                range.SetText(ec, 0, &wide)?;
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                let composition = ctx_comp.StartComposition(ec, &range, &self.comp_sink)?;
                let _ = move_caret_to_end(&self.context, ec, &range);
                *self.composition_slot.lock().unwrap() = Some(composition);
            }
        }
        Ok(())
    }
}

// ── EditSession: 텍스트 삽입 ──

#[implement(ITfEditSession)]
struct InsertTextEditSession {
    context: ITfContext,
    text: String,
}

impl ITfEditSession_Impl for InsertTextEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            // 콘솔(wezterm 등) 폴백 포함 삽입 range 획득.
            // SetText 후 caret 을 끝으로 보내야 비조합 commit 이 같은 offset 에
            // 쌓여 메모장에서 음절이 거꾸로 들어가는 것을 막는다 (PATH 1, 주 경로).
            let range = acquire_insert_range(&self.context, ec)?;
            range.SetText(ec, 0, &wide)?;
            let _ = move_caret_to_end(&self.context, ec, &range);
        }
        Ok(())
    }
}

// ── EditSession: 선택 영역 텍스트 읽기 (수동 TypeFix 용) ──

/// ReadOnly EditSession: 현재 선택 영역 텍스트와 전체 문서 텍스트, 커서/앵커 위치를 읽는다.
///
/// 결과는 Mutex<Option<SelectionReadResult>> 슬롯에 저장된다.
#[implement(ITfEditSession)]
pub struct ReadSelectionEditSession {
    pub context: ITfContext,
    pub result_slot: std::sync::Arc<std::sync::Mutex<Option<SelectionReadResult>>>,
}

/// 선택 영역 읽기 결과
#[derive(Debug)]
pub struct SelectionReadResult {
    /// 문서 전체 텍스트 (커서 앞 sufficient context — 최대 4096자)
    pub surrounding_text: String,
    /// 커서 위치 (char 단위)
    pub cursor: u32,
    /// 앵커 위치 (char 단위, cursor != anchor 이면 선택 영역 있음)
    pub anchor: u32,
}

impl ITfEditSession_Impl for ReadSelectionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // 1. 현재 선택(selection) 범위 획득
            let mut sel = TF_SELECTION::default();
            let mut fetched: u32 = 0;
            self.context
                .GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched)?;
            if fetched == 0 {
                return Ok(());
            }
            let sel_range = match sel.range.as_ref() {
                Some(r) => r.clone(),
                None => return Ok(()),
            };

            // 선택 영역이 없으면(IsEmpty) 조용히 종료
            if sel_range.IsEmpty(ec).unwrap_or(BOOL(1)).as_bool() {
                return Ok(());
            }

            // 2. anchor 위치 계산: sel_range 시작(TF_ANCHOR_START) 앞 텍스트 길이
            //    Clone 후 Collapse(START), ShiftStart(-4096), GetText 순으로 읽기.
            let anchor_range = sel_range.Clone()?;
            anchor_range.Collapse(ec, TF_ANCHOR_START)?;
            let mut anchor_shifted: i32 = 0;
            anchor_range.ShiftStart(ec, -4096, &mut anchor_shifted, std::ptr::null())
                .unwrap_or(());
            let mut anchor_buf = [0u16; 4096];
            let mut anchor_fetched: u32 = 0;
            anchor_range
                .GetText(ec, 0, &mut anchor_buf, &mut anchor_fetched)
                .unwrap_or(());
            let anchor_text = String::from_utf16_lossy(&anchor_buf[..anchor_fetched as usize]);
            let anchor_pos = anchor_text.chars().count() as u32;

            // 3. cursor 위치 계산: sel_range 끝(TF_ANCHOR_END) 앞 텍스트 길이
            let cursor_range = sel_range.Clone()?;
            cursor_range.Collapse(ec, TF_ANCHOR_END)?;
            let mut cursor_shifted: i32 = 0;
            cursor_range.ShiftStart(ec, -4096, &mut cursor_shifted, std::ptr::null())
                .unwrap_or(());
            let mut cursor_buf = [0u16; 4096];
            let mut cursor_fetched: u32 = 0;
            cursor_range
                .GetText(ec, 0, &mut cursor_buf, &mut cursor_fetched)
                .unwrap_or(());
            let cursor_text_before = String::from_utf16_lossy(&cursor_buf[..cursor_fetched as usize]);
            let cursor_pos = cursor_text_before.chars().count() as u32;

            // surrounding_text 는 cursor 위치까지의 텍스트 (typefix_convert 가 사용하는 컨텍스트)
            *self.result_slot.lock().unwrap() = Some(SelectionReadResult {
                surrounding_text: cursor_text_before,
                cursor: cursor_pos,
                anchor: anchor_pos,
            });
        }
        Ok(())
    }
}

/// TSF context 에서 현재 선택 영역을 읽어 반환한다.
///
/// 선택 영역이 없거나 읽기 실패 시 `None` 반환 — 호출자는 기존 동작으로 fallback.
pub fn read_selection_text(
    context: &ITfContext,
    tid: u32,
) -> Option<SelectionReadResult> {
    let slot: std::sync::Arc<std::sync::Mutex<Option<SelectionReadResult>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    let session = ReadSelectionEditSession {
        context: context.clone(),
        result_slot: slot.clone(),
    };
    let session_intf: ITfEditSession = session.into();
    unsafe {
        // ReadOnly 세션 — TF_ES_READ | TF_ES_SYNC
        let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READ | TF_ES_SYNC);
    }

    let result = slot.lock().unwrap().take();
    result
}
