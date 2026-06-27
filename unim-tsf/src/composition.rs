//! TSF 조합(Composition) 관리

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};

use windows::core::*;
use windows::core::BOOL;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_I4};
use windows::Win32::UI::TextServices::*;

use crate::globals::UNIM_DISPLAY_ATTR_INPUT;

/// composition range 에 `GUID_PROP_ATTRIBUTE` display-attribute 속성을 부여한다.
///
/// CUAS(Cicero Unaware Application Support — wezterm/텔레그램 등 IMM32 소비 앱)는
/// 이 attribute property 를 읽어 `WM_IME_COMPOSITION` 의 "미확정(밑줄) 조합" attribute
/// 바이트로 변환한다. 속성이 없으면 CUAS 가 range 를 "완료된 result string" 으로 오인해
/// `OnCompositionTerminated` 를 StartComposition 직후 즉시 호출(매 자모 초기화)할 수 있다.
/// 정식 TSF text store(메모장)에서는 밑줄 렌더링 신호로 동작한다.
///
/// `atom` 은 `ITfCategoryMgr::RegisterGUID(&UNIM_DISPLAY_ATTR_INPUT)` 로 얻은 `TfGuidAtom`.
/// SetValue 실패는 치명적이지 않으므로 패닉/조기 return 하지 않되, HRESULT 와 직후
/// GetValue 재확인 결과를 dbg_log 로 남긴다(지식베이스 P1 진단 — attribute 무음 실패 A vs
/// lifecycle 종료 B 분리). 잘못된 VARIANT 로 정상 앱 조합까지 깨지면 안 되기 때문(리스크3 완화).
unsafe fn set_composition_attribute(context: &ITfContext, ec: u32, range: &ITfRange, atom: u32) {
    let prop = match context.GetProperty(&GUID_PROP_ATTRIBUTE) {
        Ok(p) => p,
        Err(_) => return,
    };
    // VARIANT 는 0.62 에 From<i32> 가 없어 VT_I4 를 union 필드로 직접 구성한다.
    // 레이아웃: VARIANT.Anonymous(VARIANT_0) → .Anonymous(ManuallyDrop<VARIANT_0_0>)
    //          → .vt(VARENUM) + .Anonymous(VARIANT_0_0_0).lVal(i32). union 이라 unsafe.
    let mut var = VARIANT::default(); // zeroed (vt=VT_EMPTY) → 버릴 live 값 없음
    // ManuallyDrop union 필드에 직접 쓰면 rustc 가 구 값 destructor 실행을 막아
    // 자동 DerefMut 을 거부한다. 명시적으로 *ManuallyDrop 을 거쳐 inner 를 잡고 쓴다.
    let inner = &mut *var.Anonymous.Anonymous; // &mut VARIANT_0_0 (zeroed)
    inner.vt = VT_I4; // VARENUM(3)
    inner.Anonymous.lVal = atom as i32; // TfGuidAtom
    match prop.SetValue(ec, range, &var) {
        Ok(()) => {
            // 런타임 검증(지식베이스 P1): attribute property 가 실제로 range 에 박혔는지
            // 직후 GetValue 로 재확인한다. CUAS 가 range 를 result-string 으로 오인하는
            // 'A(attribute 무음 실패)' 케이스를 'B(lifecycle 종료)' 와 분리하기 위함.
            match prop.GetValue(ec, range) {
                Ok(got) => {
                    let g = &*got.Anonymous.Anonymous;
                    let ok = g.vt == VT_I4 && g.Anonymous.lVal == atom as i32;
                    if !ok {
                        crate::register::dbg_log(&format!(
                            "set_composition_attribute: GetValue MISMATCH vt={} lVal={} (expected VT_I4 atom={})",
                            g.vt.0, g.Anonymous.lVal, atom
                        ));
                    }
                }
                Err(e) => crate::register::dbg_log(&format!(
                    "set_composition_attribute: SetValue OK but GetValue failed hr={:?}",
                    e
                )),
            }
        }
        Err(e) => crate::register::dbg_log(&format!(
            "set_composition_attribute: SetValue FAILED hr={:?}",
            e
        )),
    }
}

/// composition range 에 `GUID_PROP_READING` reading 속성(VT_BSTR)을 부여한다.
///
/// CUAS(IMM32 브리지)는 미확정→확정 변환 시 `GCS_RESULTCLAUSE`/`GCS_RESULTREADCLAUSE`
/// 를 `GUID_PROP_READING` 속성의 세그먼트 구조로부터 생성한다(Mozc
/// `tip_edit_session_impl.cc` 주석 근거). READING 속성이 없으면 CUAS 가 read clause
/// 세그먼트를 만들지 못해 composition 을 미확정으로 유지하지 못하고 즉시
/// `OnCompositionTerminated` 할 수 있다. ATTRIBUTE(밑줄 신호)와 같은 range 에 함께
/// 부여해야 한다.
///
/// 한국어는 별도 음성 표기층이 없어 reading 값으로 조합 중 한글 문자열 자체를 넣는다
/// (NFC 음절 사용). 값 내용보다 composition range 를 덮는 non-empty READING 세그먼트의
/// 존재가 핵심이다.
///
/// `ITfProperty::SetValue` 는 VARIANT 를 `[in] const` 로 받아 값을 복사한다(소유권
/// 미이전, MS Learn 확인). windows-rs 0.62 VARIANT 는 Drop 미구현이고 union 의
/// `bstrVal` 은 `ManuallyDrop<BSTR>`(BSTR Drop=SysFreeString)이라, SetValue 직후
/// `ManuallyDrop::drop` 으로 우리 BSTR 을 직접 해제해야 누수가 없다. SetValue 실패는
/// 비치명(조합 계속, 정상 앱 무영향)이라 패닉/조기 return 없이 dbg_log 만 남긴다.
unsafe fn set_composition_reading(context: &ITfContext, ec: u32, range: &ITfRange, reading: &str) {
    if reading.is_empty() {
        return;
    }
    let prop = match context.GetProperty(&GUID_PROP_READING) {
        Ok(p) => p,
        Err(_) => return,
    };
    // VT_BSTR VARIANT 를 union 필드로 직접 구성(set_composition_attribute 와 동일 패턴).
    let mut var = VARIANT::default(); // zeroed (vt=VT_EMPTY) → 버릴 live 값 없음
    {
        // inner 의 mut borrow 를 블록으로 종료시킨 뒤 &var(immutable) 로 SetValue 한다
        // (borrow checker: inner 가 살아있으면 &var 가 E0502).
        let inner = &mut *var.Anonymous.Anonymous; // &mut VARIANT_0_0 (zeroed)
        inner.vt = VT_BSTR; // VARENUM(8)
        // BSTR::from(&str)=SysAllocString 래퍼(소유 BSTR). bstrVal 은 ManuallyDrop<BSTR>.
        inner.Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(reading));
    }
    let r = prop.SetValue(ec, range, &var);
    // SetValue 는 값을 복사하므로 우리 BSTR 을 직접 해제(누수 방지). ManuallyDrop union
    // 필드는 자동 DerefMut 이 안 되므로 set_composition_attribute 와 동일하게 *ManuallyDrop
    // 을 거쳐 inner 를 다시 잡고 drop 한다(이 시점엔 &var borrow 가 끝나 충돌 없음).
    let inner = &mut *var.Anonymous.Anonymous;
    ManuallyDrop::drop(&mut inner.Anonymous.bstrVal);
    if let Err(e) = r {
        crate::register::dbg_log(&format!("set_composition_reading: SetValue FAILED hr={:?}", e));
    }
}

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

/// composition range 전체를 selection 으로 설정한다 (SampleIME `_SetComposition` 방식).
///
/// `move_caret_to_end` 는 selection 을 `Collapse(END)` 한 0폭 커서로 만든다. 이는
/// 메모장 등 정식 TSF text store 에서는 문제없지만, **IMM32→TSF 브리지(cicero)**
/// 로 동작하는 콘솔(wezterm 등)에서는 "커서가 composition 범위 밖으로 이동"으로
/// 해석돼 `ITfCompositionSink::OnCompositionTerminated` 가 StartComposition 직후
/// 즉시 호출된다(조합이 매 자모 초기화되는 근본 원인). SampleIME 처럼 collapse
/// 하지 않고 range 전체를 `TF_AE_NONE` selection 으로 두면 브리지가 composition
/// 을 유지한다.
unsafe fn select_composition_range(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    let r = range.Clone()?;
    let mut sel = TF_SELECTION {
        range: ManuallyDrop::new(Some(r)),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_NONE,
            // 조합 중 selection 은 interim character 로 표시한다(fInterimChar=TRUE).
            // NavilIME(EditSession.cpp)·saenaru(tip/compose.cpp)·kolemak(SetInterimSelection)
            // 등 한국어 TSF IME 가 공통으로 사용하는 패턴으로, IMM32 interim-char 의미론에
            // 매핑돼 CUAS 브리지(wezterm 등)가 composition 을 미확정으로 인식하게 만든다.
            // 커밋/종료 경로(move_caret_to_end)는 FALSE 로 둔다.
            fInterimChar: BOOL(1),
        },
    };
    let res = context.SetSelection(ec, std::slice::from_ref(&sel));
    // 우리가 만든 ITfRange 참조 해제 (SetSelection 은 내부에서 AddRef)
    ManuallyDrop::drop(&mut sel.range);
    res
}

/// 교체 대상 range 전체를 selection 으로 설정한다(확정 selection, fInterimChar=FALSE).
///
/// AutoTypeFix 교체에서 SetText 직전 호출. Blink(Chrome/Electron)는 "현재
/// selection 밖" 편집을 무시해 첫 글자가 누락(B1)되거나 원문이 잔류(B2)할 수
/// 있는데, 교체 range 를 selection 으로 명시하면 그 영역이 reconversion 대상으로
/// 확정돼 result-string 까지 안전히 덮어쓴다. 조합용(interim-char) selection 과
/// 달리 fInterimChar=FALSE 로 둔다(미확정 표시 불필요, 즉시 교체 확정).
unsafe fn select_replacement_range(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    let r = range.Clone()?;
    let mut sel = TF_SELECTION {
        range: ManuallyDrop::new(Some(r)),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_NONE,
            fInterimChar: BOOL(0),
        },
    };
    let res = context.SetSelection(ec, std::slice::from_ref(&sel));
    ManuallyDrop::drop(&mut sel.range);
    res
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
    match context.cast::<ITfInsertAtSelection>() {
        Ok(insert) => match insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[]) {
            Ok(range) => {
                crate::register::dbg_log("acquire_insert_range: InsertAtSelection QUERYONLY ok");
                return Ok(range);
            }
            Err(e) => crate::register::dbg_log(&format!(
                "acquire_insert_range: InsertAtSelection FAILED hr=0x{:08X} -> GetSelection fallback",
                e.code().0
            )),
        },
        Err(e) => crate::register::dbg_log(&format!(
            "acquire_insert_range: cast ITfInsertAtSelection FAILED hr=0x{:08X} -> GetSelection fallback",
            e.code().0
        )),
    }
    // 폴백: 현재 selection range 를 직접 사용
    let mut sel = TF_SELECTION::default();
    let mut fetched: u32 = 0;
    let gs = context.GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched);
    if let Err(e) = &gs {
        crate::register::dbg_log(&format!(
            "acquire_insert_range: GetSelection FAILED hr=0x{:08X}",
            e.code().0
        ));
    }
    gs?;
    let result = if fetched != 0 {
        sel.range
            .as_ref()
            .map(|r| r.clone())
            .ok_or_else(|| Error::from(E_FAIL))
    } else {
        crate::register::dbg_log("acquire_insert_range: GetSelection fetched=0 (no selection)");
        Err(Error::from(E_FAIL))
    };
    // GetSelection 이 채운 참조 해제 (위에서 clone 으로 새 참조 확보)
    ManuallyDrop::drop(&mut sel.range);
    result
}

/// `replace_surrounding` 의 결과 신호 (b1: bool → enum).
///
/// 호출부(key_handler→text_service)가 후속 동작을 분기하는 데 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceOutcome {
    /// 정상 TSF 경로(메모장/Word 등) 또는 synth 미진입. 추가 동작 없음.
    Normal,
    /// 역방향/undo synth 폴백 — commit-only(마지막 음절 조합 없음)를 Phase2 에
    /// 위임했다. 엔진에 잔여 조합 preedit 이 없으므로 호출부는 remove_preedit 불필요.
    /// (Phase2 삽입은 SetTimer 가 트리거 — text_service 가 PhaseSplit 와 동일 경로로 처리.)
    PhaseSplitCommitOnly,
    /// 순방향 synth 폴백 — 삭제(Phase1)와 삽입(Phase2)을 분리했다. Phase2 에서
    /// 마지막 음절을 TSF 조합으로 유지하므로 **엔진의 잔여 preedit 을 보존**해야 한다
    /// (호출부는 remove_preedit 를 호출하면 안 됨). text_service 가 SetTimer 를 건다.
    PhaseSplit,
}

/// 조합 상태 관리자
pub struct CompositionManager {
    composition: Option<ITfComposition>,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
    /// `GUID_PROP_ATTRIBUTE` SetValue 용 캐시된 `TfGuidAtom`.
    /// 최초 1회 `ITfCategoryMgr::RegisterGUID` 로 획득해 재사용(매번 등록 금지).
    attr_atom: Option<u32>,
    /// AutoTypeFix synth 폴백 슬롯. ReplaceSurroundingEditSession(TF_ES_SYNC)이
    /// ShiftStart 부족(CUAS/Blink — 확정 텍스트 뒤로 역확장 거부)을 감지하면,
    /// 문서를 변형하지 않고 (delete_chars, commit_text, preedit_text)를 여기에
    /// 기록한 뒤 즉시 반환한다. replace_surrounding 이 edit session 밖에서 슬롯을
    /// 확인해 b1 2-phase 폴백(Phase1=send_replacement_delete BS×N + store_pending_insert,
    /// Phase2=타이머 후 insert_pending)으로 처리한다.
    ///
    /// D3: 4번째 요소 `before_len`(=삭제 전 커서 앞 글자 수, edit session 내부에서
    /// read-back 측정)을 함께 기록한다. replace_surrounding 이 이를 꺼내
    /// `expected_after_delete = before_len - delete_chars` 를 산출해 OnEndEdit read-back
    /// 게이트의 기준선으로 쓴다. `None` 이면 read-back 실패 → 게이트 비활성(타이머 단독).
    synth_slot: Arc<Mutex<Option<(u32, String, String, Option<u32>)>>>,
}

impl CompositionManager {
    pub fn new() -> Self {
        Self {
            composition: None,
            composition_slot: Arc::new(Mutex::new(None)),
            attr_atom: None,
            synth_slot: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.composition.is_some()
    }
    pub fn clear(&mut self) {
        self.composition = None;
    }

    /// composition display-attribute 의 `TfGuidAtom` 을 얻는다(최초 1회 등록·캐시).
    ///
    /// `ITfCategoryMgr` 는 `CoCreateInstance(CLSID_TF_CategoryMgr)` 로 획득(register.rs 가
    /// `ITfInputProcessorProfiles` 를 얻는 것과 동일 패턴). `ITfCategoryMgr` 는 IUnknown
    /// 파생일 뿐 `ITfThreadMgr` 계층이 아니라 cast 가 보장되지 않으므로 CoCreate 가 안전.
    /// 실패 시 `None` — 호출자는 attribute set 을 조용히 skip 한다(조합은 계속).
    fn attr_atom(&mut self) -> Option<u32> {
        if let Some(atom) = self.attr_atom {
            return Some(atom);
        }
        unsafe {
            let cat_mgr: ITfCategoryMgr = windows::Win32::System::Com::CoCreateInstance(
                &CLSID_TF_CategoryMgr,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
            .ok()?;
            let atom = cat_mgr.RegisterGUID(&UNIM_DISPLAY_ATTR_INPUT).ok()?;
            self.attr_atom = Some(atom);
            Some(atom)
        }
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
        // attr_atom() 은 &mut self 이므로 session(불변 캡처) 구성 전에 먼저 호출한다.
        let attr_atom = self.attr_atom();

        // 단일 세션: StartComposition(empty) → SetText(preedit) → select → ATTRIBUTE+READING
        // 을 한 번에 처리한다(이전 2-phase 통합). READING(GUID_PROP_READING) 부여로 CUAS
        // 즉시-terminate 를 회피하므로 빈-조합 선행 세션이 더 이상 필요 없다.
        let session = StartCompositionEditSession {
            context: context.clone(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
            text: text.to_string(),
            attr_atom,
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let hr = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
            crate::register::dbg_log(&format!(
                "start_composition: RequestEditSession hr={:?}",
                hr.map(|s| s.0)
            ));
        }

        let result = self.composition_slot.lock().unwrap().take();
        if let Some(comp) = result {
            self.composition = Some(comp);
            crate::register::dbg_log("start_composition: composition CREATED (single-session)");
        } else {
            crate::register::dbg_log("start_composition: composition slot EMPTY (failed)");
        }
    }

    pub fn update_composition(&mut self, context: &ITfContext, tid: u32, text: &str) {
        let attr_atom = self.attr_atom();
        if let Some(ref composition) = self.composition {
            let session = UpdateCompositionEditSession {
                context: context.clone(),
                text: text.to_string(),
                composition: composition.clone(),
                attr_atom,
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

    /// 음절 전환: 기존 composition 을 commit_text 로 확정·종료하고, **같은 edit
    /// session 안에서** 새 composition 을 preedit_text 로 시작한다.
    ///
    /// end_composition + start_composition 을 두 개의 별도 sync 세션으로 호출하면
    /// CUAS-unaware(IMM32, wezterm 등) 앱에서 "조합 종료 직후 새 조합 시작"을
    /// CUAS 가 거부해 새 composition 이 즉시 OnCompositionTerminated 된다(매 음절
    /// 전환마다 오버레이로 떨어지는 원인). 한 트랜잭션으로 합쳐 churn 을 없앤다.
    /// 활성 composition 이 없으면 insert_text + start_composition 으로 폴백.
    ///
    /// 반환: commit_text 가 **실제로 문서에 확정 적용됐으면** `true`. b1[D] 폴백이
    /// 세션B 거부 시 mode1(RequestEditSession 전체 거부=확정 안 됨)/mode2(step1 commit
    /// 성공·tail StartComposition 실패)를 구분해 tail 의 이중삽입(mode1)·유실(mode2)을
    /// 막는 데 쓴다. 네이티브 호출부(key_handler)는 반환값을 무시한다.
    pub fn commit_and_restart(
        &mut self,
        context: &ITfContext,
        tid: u32,
        commit_text: &str,
        preedit_text: &str,
        comp_sink: &ITfCompositionSink,
    ) -> bool {
        let Some(old) = self.composition.clone() else {
            if !commit_text.is_empty() {
                self.insert_text(context, tid, commit_text);
            }
            self.start_composition(context, tid, preedit_text, comp_sink);
            return !commit_text.is_empty();
        };
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;
        // step1(commit) 성공 시 CommitRestartEditSession 이 EndComposition 직후 true 로
        // 세팅한다. 세션이 통째로 거부되거나 step1 전에 실패하면 false 로 남는다.
        let applied = Arc::new(Mutex::new(false));
        let attr_atom = self.attr_atom();
        let session = CommitRestartEditSession {
            context: context.clone(),
            old_composition: old,
            commit_text: commit_text.to_string(),
            preedit_text: preedit_text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
            commit_applied: applied.clone(),
            attr_atom,
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
        self.composition = self.composition_slot.lock().unwrap().take();
        let was_applied = *applied.lock().unwrap();
        was_applied
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
    ///
    /// 반환(b1 ReplaceOutcome):
    /// - `Normal`: 정상 TSF 경로(ShiftStart 충분) 또는 synth 미진입. 추가 동작 없음.
    /// - `PhaseSplit`: 순방향 synth 폴백. Phase1(BS×N SendInput)만 수행하고
    ///   (마지막 음절 제외 확정문 + 마지막 음절 조합문)을 PendingInsert 에 적재했다.
    ///   호출부는 **엔진 preedit(마지막 음절)을 보존**하고 SetTimer 로 Phase2 삽입을
    ///   예약해야 한다(text_service).
    /// - `PhaseSplitCommitOnly`: 역방향/undo synth 폴백. Phase1(BS×N)만 수행하고
    ///   확정문만 PendingInsert 에 적재(마지막 음절="")했다. 조합 없음.
    pub fn replace_surrounding(
        &mut self,
        context: &ITfContext,
        tid: u32,
        delete_chars: u32,
        commit_text: &str,
        preedit_text: &str,
        comp_sink: &ITfCompositionSink,
    ) -> ReplaceOutcome {
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;
        // synth 폴백 슬롯도 매 호출 전 비운다.
        let synth = self.synth_slot.clone();
        *synth.lock().unwrap() = None;
        let attr_atom = self.attr_atom();

        let session = ReplaceSurroundingEditSession {
            context: context.clone(),
            delete_chars: delete_chars as i32,
            commit_text: commit_text.to_string(),
            preedit_text: preedit_text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
            synth_slot: synth.clone(),
            attr_atom,
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            // 거부 시 그대로 무시 (skip)
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }

        // synth 폴백 판정: edit session 안에서 ShiftStart 가 부족하면 문서를
        // 변형하지 않고 슬롯에 기록만 했다. Some 이면 여기서(= edit session 밖)
        // SendInput 합성 백스페이스+삽입으로 폴백한다. SendInput 은 edit session
        // 안에서 호출하면 안 되므로(동기 락 보유 중) 반드시 이 시점에 호출.
        let synth_req = self.synth_slot.lock().unwrap().take();
        if let Some((d, c, p, before_len)) = synth_req {
            // synth 경로는 b1 2-phase(삭제↔삽입 분리)로 처리하므로 edit session 이
            // 만든 composition 은 존재하지 않는다. 활성 composition 추적은 비운다.
            self.composition = None;

            // ── b1 Phase1: 삭제만 SendInput, 삽입은 Phase2(타이머)로 분리 ──
            // 같은 SendInput 배치에 BS 와 문자삽입을 섞지 않으므로(=UNICODE 미주입)
            // Blink 의 비동기 재정렬(BS↔PACKET) 대상이 원천 소멸한다.
            //   - 순방향(p="기"): commit="서"(마지막 음절 제외) 확정 + last="기" 조합 유지.
            //   - 역방향/undo(p=""): commit-only(마지막 음절 없음).
            // 삭제는 여기서(edit session 밖) 즉시 주입하고, 삽입 텍스트는 PendingInsert
            // 슬롯에 적재한 뒤 호출부가 SetTimer 로 Phase2 를 예약한다.
            crate::register::dbg_log(&format!(
                "ReplaceSurrounding: ShiftStart 부족 → b1 BS phase \
                 (del={d} commit_len={} last_syllable_len={})",
                c.chars().count(),
                p.chars().count()
            ));
            crate::synth_input::send_replacement_delete(d);
            crate::synth_input::store_pending_insert(&c, &p);
            // D3: read-back 게이트 기준선 산출. before_len(삭제 전 커서 앞 글자 수)을
            // 읽었으면 expected = before_len - d. 음수 방지(클램프). 못 읽었으면 -1
            // → 게이트 비활성(타이머 단독 폴백 = 현행 b1, 악화 없음).
            let expected = match before_len {
                Some(b) => (b as i32 - d as i32).max(0),
                None => -1,
            };
            crate::synth_input::arm_readback_gate(expected);
            crate::register::dbg_log(&format!(
                "ReplaceSurrounding: D3 readback gate expected={} (before_len={:?} del={})",
                expected, before_len, d
            ));
            return if p.is_empty() {
                ReplaceOutcome::PhaseSplitCommitOnly
            } else {
                ReplaceOutcome::PhaseSplit
            };
        }

        // 정상 TSF 경로(ShiftStart full):
        if !preedit_text.is_empty() {
            // 순방향: 세션1 이 full 조합(commit+preedit)을 만들었다(slot Some). self.composition
            // 에 보관하고 (commit, preedit)을 PENDING_RESTART 에 적재 → 호출부가 펌프(타이머→
            // WM_UNIM_FLUSH2) 후 Phase2b(commit_and_restart)로 commit 을 정착시킨다. commit+
            // compose 를 한 ec 에 하면 Blink 가 commit 유실(실측: full=true 네이티브 경로도 동일
            // 버그). slot 이 비면(StartComposition 거부) raw 로 full 텍스트가 들어갔으므로
            // 조합 없이 Normal(rare degrade).
            let result = self.composition_slot.lock().unwrap().take();
            if let Some(comp) = result {
                self.composition = Some(comp);
                crate::synth_input::store_pending_restart(commit_text, preedit_text);
                crate::register::dbg_log(
                    "ReplaceSurrounding: native full-composition alive → Phase2b 예약(pump-split)",
                );
                return ReplaceOutcome::PhaseSplit;
            }
            self.composition = None;
            crate::register::dbg_log(
                "ReplaceSurrounding: full-composition slot empty(raw 폴백) → Normal",
            );
        } else {
            // preedit 없음(역방향/commit-only) — 단일 commit 확정 완료, 조합 없음.
            self.composition = None;
        }
        ReplaceOutcome::Normal
    }

    /// b1 Phase2 — BS×N 이 Blink 문서에 적용된 뒤(SetTimer/race-flush) 호출.
    ///
    /// **방식 D(검증된 빌딩블록 재사용)**: 자작 단일세션 InsertCompose 가 같은 lock 안에서
    /// commit 을 정착시키기 전에 tail SetText 로 그 자리를 덮어써 **앞 확정문을 유실**시키던
    /// 문제(크롬에서 마지막 음절만 표시)를, 네이티브 타이핑이 1032회 성공한
    /// commit_and_restart 경로로 우회한다.
    ///   - case1 (last_syllable 빈): 역방향/undo — commit_text 만 wrap-commit(insert_commit).
    ///   - case2 (commit_text 빈): tail 만 단일 start_composition.
    ///   - case3 (둘 다 있음, 핵심): 세션A `start_composition(commit+tail 전체)` 로 라이브
    ///     조합 1개를 만들고(= CommitRestart 의 "기존 조합 존재" 전제 충족), 세션B
    ///     `commit_and_restart(commit, tail)` 로 앞부분을 확정·tail 을 재조합한다. 두 세션이
    ///     별도 ec/lock 이라 commit 조합이 **lock 해제 경계를 건넌 뒤** 확정된다(구 단일세션의
    ///     "같은 lock 내 born+commit+overwrite" 구조를 회피).
    ///
    /// 반환: 마지막 음절이 폴백 확정됐으면 `true` — 호출부가 엔진 preedit 도 비워야
    /// 동기(문서=확정, 엔진=빈조합)가 맞는다. 조합 유지 성공 시 `false`(엔진 보존).
    ///
    /// SendInput 을 호출하지 않으므로 edit session 락 안에서 호출해도 안전하다.
    /// 어떤 실패도 패닉 금지(dbg_log). 세션B 거부(드묾) 시 세션A 의 full 조합이 문서에
    /// 잔존(텍스트 보존) → tail 재삽입 금지(이중삽입 회귀 방지).
    pub fn insert_pending(
        &mut self,
        context: &ITfContext,
        tid: u32,
        commit_text: &str,
        last_syllable: &str,
        comp_sink: &ITfCompositionSink,
    ) -> bool {
        // 잔여 조합이 있으면 먼저 정리(보통 없음 — synth 경로는 composition=None).
        self.composition = None;

        // ── D: 검증된 빌딩블록 재사용(자작 단일세션 InsertCompose 폐기) ──
        // 구 InsertComposeEditSession 은 step1(새 조합 생성→SetText→EndComposition)과
        // step2(같은 range 재사용 조합)를 한 ec 에 수행했는데, Blink 가 같은 lock 안에서
        // step1 commit 을 정착시키기 전에 step2 SetText 가 그 자리를 덮어써 **앞 확정문이
        // 유실**됐다(마지막 음절만 표시). 반면 네이티브 commit_and_restart
        // (CommitRestartEditSession)는 ①기존 라이브 조합을 commit 하고 ②EndComposition
        // 직후 acquire_insert_range 로 range 를 재획득해 tail 을 쓴다 — 크롬 1032회 성공.
        // D 는 이 성공 경로를 그대로 재사용한다(세션A 로 "기존 조합" 전제를 만들고,
        // 세션B 로 검증된 commit_and_restart 호출).

        // case 1: 역방향/undo — tail 없음 → 확정문만 wrap-commit(조합 없음).
        if last_syllable.is_empty() {
            if !commit_text.is_empty() {
                self.insert_commit(context, tid, commit_text, comp_sink);
            }
            crate::register::dbg_log("b1[D]: commit-only (no tail)");
            return false;
        }

        // case 2: 앞 확정문 없음 — tail 만 단일 라이브 조합.
        if commit_text.is_empty() {
            self.start_composition(context, tid, last_syllable, comp_sink);
            return if self.is_active() {
                crate::register::dbg_log("b1[D]: tail-only kept (composition alive)");
                false
            } else {
                crate::register::dbg_log("b1[D]: tail-only REJECTED → commit fallback");
                self.insert_commit(context, tid, last_syllable, comp_sink);
                true
            };
        }

        // case 3 (핵심): 전체 교정문으로 라이브 조합(세션A) → [메시지 펌프] → 세션B(Phase2b).
        // ⚠️ 세션A↔세션B 를 같은 틱에 연속 실행(같은-lock back-to-back)하면 크롬(Blink)이
        // 세션A 조합을 문서에 등록하기 전이라 세션B EndComposition 의 commit 이 정착하지
        // 못한다(앞 확정문 유실 — 실측 확인: sessionB alive=true 인데 화면엔 tail 만). 네이티브
        // 타이핑은 키 사이에 메시지 펌프가 끼어 정착 경계가 생긴다. 그래서 세션A 만 여기서
        // 만들고 (commit, tail)을 PENDING_RESTART 에 적재 → 호출부가 PostMessage(WM_UNIM_FLUSH2)
        // 로 펌프를 한 번 돌린 뒤 insert_restart_phase_b 가 commit_and_restart 한다.
        let full = format!("{commit_text}{last_syllable}");
        self.start_composition(context, tid, &full, comp_sink); // 세션A
        if self.is_active() {
            crate::synth_input::store_pending_restart(commit_text, last_syllable);
            crate::register::dbg_log(
                "b1[D]: sessionA start_composition(full) alive → Phase2b 예약(pump-split)",
            );
            // 세션A 조합 생존 → 엔진 preedit 보존(false). 확정은 Phase2b 가 수행.
            false
        } else {
            // 세션A 실패: 구 동작 승계 — commit wrap 확정 + tail 조합 best-effort(동기, 펌프 불요).
            crate::register::dbg_log("b1[D]: sessionA FAILED → insert_commit+start fallback");
            self.insert_commit(context, tid, commit_text, comp_sink);
            self.start_composition(context, tid, last_syllable, comp_sink);
            if self.is_active() {
                false
            } else {
                self.insert_commit(context, tid, last_syllable, comp_sink);
                true
            }
        }
    }

    /// b1[D] Phase2b — 메시지 펌프(WM_UNIM_FLUSH2) 후 호출. Phase2a 세션A 가 만든 라이브
    /// 조합(예 "느느늘")을 검증된 `commit_and_restart` 로 확정한다(앞부분 확정 + tail 재조합).
    /// 펌프가 세션A 조합의 문서 등록을 보장하므로 commit 이 정착한다.
    ///
    /// self.composition 이 펌프 중 사라졌으면 commit_and_restart 의 else 폴백
    /// (insert_text+start_composition)이 처리한다. 반환: tail 조합 유지 실패로 확정
    /// 폴백했으면 true(호출부가 엔진 preedit 도 비움), 유지 성공이면 false(보존).
    pub fn insert_restart_phase_b(
        &mut self,
        context: &ITfContext,
        tid: u32,
        commit_text: &str,
        last_syllable: &str,
        comp_sink: &ITfCompositionSink,
    ) -> bool {
        let commit_applied =
            self.commit_and_restart(context, tid, commit_text, last_syllable, comp_sink);
        if self.is_active() {
            crate::register::dbg_log("b1[D]: Phase2b commit_and_restart done → last kept (alive)");
            false
        } else if commit_applied {
            // mode2(드묾): commit 은 확정됐으나 tail 조합 실패 → tail 확정 삽입(유실 방지).
            crate::register::dbg_log(
                "b1[D]: Phase2b mode2 (commit ok, tail failed) → insert_commit(tail)",
            );
            self.insert_commit(context, tid, last_syllable, comp_sink);
            true
        } else {
            // mode1(드묾): 세션B 거부·commit 미적용 → 세션A full 조합 잔존(텍스트 보존),
            // tail 재삽입하면 중복이므로 금지.
            crate::register::dbg_log(
                "b1[D]: Phase2b mode1 (rejected) → keep sessionA full text (no double-insert)",
            );
            true
        }
    }

    /// 확정 텍스트를 composition 으로 감싸 삽입한다(StartComposition→SetText→
    /// EndComposition). raw SetText 직접 편집은 Blink 가 무시할 수 있어(첫 글자
    /// 누락) reconversion 동일 경로로 감싼다. StartComposition 거부 시 insert_text
    /// (raw SetText) 로 폴백한다. self.composition 은 갱신하지 않는다(즉시 확정).
    fn insert_commit(
        &self,
        context: &ITfContext,
        tid: u32,
        text: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        let session = InsertCommitEditSession {
            context: context.clone(),
            text: text.to_string(),
            comp_sink: comp_sink.clone(),
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
    }
}

// ── EditSession: 조합 시작 ──

#[implement(ITfEditSession)]
struct StartCompositionEditSession {
    context: ITfContext,
    comp_sink: ITfCompositionSink,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
    /// 단일 세션에서 채울 초기 preedit 텍스트.
    text: String,
    /// composition range 에 부여할 display-attribute atom (None 이면 skip).
    attr_atom: Option<u32>,
}

impl ITfEditSession_Impl for StartCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // 단일 세션: 빈 range 에 StartComposition → SetText(preedit) → select →
            // ATTRIBUTE+READING 을 한 번에 처리한다(이전 2-phase 통합).
            //
            // StartComposition 을 SetText 보다 먼저 호출하는 순서는 유지한다
            // (CUAS-unaware 앱에서 SetText→Start 순서는 즉시-terminate 유발). 한 세션에서
            // 채워도 READING 세그먼트(GUID_PROP_READING)를 부여하면 CUAS 가 range 를
            // 미확정(GCS_COMPSTR)으로 브리지하므로 즉시 종료되지 않는다
            // (Mozc tip_edit_session_impl.cc 근거). 정상 TSF 앱(메모장)은 READING 을
            // 표준 속성으로 무시/정상처리하므로 회귀 없음.
            let range = acquire_insert_range(&self.context, ec)?;

            let ctx_comp: ITfContextComposition = match self.context.cast() {
                Ok(c) => c,
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "StartComp.DoEditSession: cast ITfContextComposition FAILED hr=0x{:08X}",
                        e.code().0
                    ));
                    return Err(e);
                }
            };

            let composition = match ctx_comp.StartComposition(ec, &range, &self.comp_sink) {
                Ok(c) => c,
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "StartComp.DoEditSession: StartComposition FAILED hr=0x{:08X}",
                        e.code().0
                    ));
                    return Err(e);
                }
            };

            // preedit 텍스트 채우기.
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, 0, &wide)?;

            // selection 을 composition range 전체로 (TF_AE_NONE, wezterm terminate 회피).
            let _ = select_composition_range(&self.context, ec, &range);

            // 미확정(밑줄) ATTRIBUTE + READING 부여(실패 무시).
            if let Some(atom) = self.attr_atom {
                set_composition_attribute(&self.context, ec, &range, atom);
            }
            set_composition_reading(&self.context, ec, &range, &self.text);

            crate::register::dbg_log("StartComp.DoEditSession: single-session ok (text+attr+reading)");
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
    /// composition range 에 부여할 display-attribute atom (None 이면 skip).
    attr_atom: Option<u32>,
}

impl ITfEditSession_Impl for UpdateCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, 0, &wide)?;
            // selection 을 composition range 전체로 (SampleIME 방식, wezterm terminate 회피)
            let _ = select_composition_range(&self.context, ec, &range);
            // 매 update 마다 미확정(밑줄) ATTRIBUTE + READING 재부여(실패 무시).
            // SetText 가 covered 텍스트를 바꾸면 기존 property 가 discard 되므로(MS Learn
            // SetValue Remarks) 매 갱신마다 READING(GUID_PROP_READING)을 다시 부여해야
            // CUAS 가 range 를 계속 미확정 조합(GCS_COMPSTR)으로 브리지한다.
            if let Some(atom) = self.attr_atom {
                set_composition_attribute(&self.context, ec, &range, atom);
            }
            set_composition_reading(&self.context, ec, &range, &self.text);
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

// ── EditSession: 확정 + 재시작 (음절 전환, end+start churn 회피) ──

#[implement(ITfEditSession)]
struct CommitRestartEditSession {
    context: ITfContext,
    old_composition: ITfComposition,
    commit_text: String,
    preedit_text: String,
    comp_sink: ITfCompositionSink,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
    /// step1(commit) EndComposition 성공 직후 true. 호출부(commit_and_restart)가
    /// 세션 거부(false)와 step1-성공/step2-실패(true)를 구분하는 데 쓴다.
    commit_applied: Arc<Mutex<bool>>,
    attr_atom: Option<u32>,
}

impl ITfEditSession_Impl for CommitRestartEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // 1. 기존 composition 을 commit_text 로 확정하고 종료(같은 세션).
            let old_range = self.old_composition.GetRange()?;
            let wide_c: Vec<u16> = self.commit_text.encode_utf16().collect();
            old_range.SetText(ec, 0, &wide_c)?;
            let _ = move_caret_to_end(&self.context, ec, &old_range);
            self.old_composition.EndComposition(ec)?;
            // commit 이 문서에 확정됨 — 이후 step2 가 실패해도 호출부가 mode2(tail 만
            // 재삽입)로 복구하도록 표식. (EndComposition 실패 시 ? 로 조기반환 → false 유지.)
            *self.commit_applied.lock().unwrap() = true;

            // 2. 같은 세션에서 새 composition 시작 — end+start 를 한 트랜잭션으로 합쳐
            //    CUAS 즉시-terminate(매 음절 전환) 를 회피한다.
            let new_range = acquire_insert_range(&self.context, ec)?;
            let ctx_comp: ITfContextComposition = match self.context.cast() {
                Ok(c) => c,
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "CommitRestart: cast ITfContextComposition FAILED hr=0x{:08X}",
                        e.code().0
                    ));
                    return Err(e);
                }
            };
            let composition = match ctx_comp.StartComposition(ec, &new_range, &self.comp_sink) {
                Ok(c) => c,
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "CommitRestart: StartComposition FAILED hr=0x{:08X}",
                        e.code().0
                    ));
                    return Err(e);
                }
            };
            let wide_p: Vec<u16> = self.preedit_text.encode_utf16().collect();
            new_range.SetText(ec, 0, &wide_p)?;
            let _ = select_composition_range(&self.context, ec, &new_range);
            if let Some(atom) = self.attr_atom {
                set_composition_attribute(&self.context, ec, &new_range, atom);
            }
            set_composition_reading(&self.context, ec, &new_range, &self.preedit_text);
            crate::register::dbg_log("CommitRestart: commit+restart in single session ok");
            *self.composition_slot.lock().unwrap() = Some(composition);
        }
        Ok(())
    }
}

// ── EditSession: 확정문 삽입 + 마지막 음절 미확정 조합 시작 (b1 폴백 Phase2, 단일세션) ──

/// commit_text(마지막 음절 제외 확정문)를 composition-wrap 으로 확정하고, **같은 ec·
/// 같은 range** 로 last_syllable(마지막 음절)을 미확정 조합으로 시작한다.
///
/// 네이티브 preedit 분기(ReplaceSurroundingEditSession step3+step4, composition.rs)를
/// 그대로 모방한다 — delete_chars=0 이므로 ShiftStart/synth 로직만 빠지고, step3(commit-
/// wrap)→step4(start composition)를 한 트랜잭션으로 묶는다. 과거 insert_commit +
/// start_composition 2-세션 churn 으로 마지막 음절이 즉시-terminate 되던 것을 막는다.
///
/// **range 는 step1 에서 1회만 획득하고 step2 에서 재사용**한다(재획득 금지 — Blink
/// async caret 미정착으로 오프셋이 어긋난다). step1 의 Collapse(TF_ANCHOR_END) 로 range
/// 가 확정문 끝으로 이동하므로, step2 의 StartComposition 이 정확히 그 뒤에서 시작한다.
///
/// commit_text 가 비면 step1 스킵, last_syllable 이 비면(역방향/undo) step2 스킵.
/// 어떤 실패도 패닉 금지(panic=abort) — ? 실패 경로는 RequestEditSession 거부로
/// graceful degrade 된다.
///
/// ⚠️ **방식 D(insert_pending 재작성)로 대체돼 더는 발행되지 않는 dead code다.** 같은
/// lock 내 commit→tail-overwrite 로 앞 확정문이 유실되는 결함(크롬)이 확인됐다. D 의
/// 실측 검증이 끝나면 제거한다 — 그 전까지는 롤백 보험으로 보존(allow(dead_code)).
#[implement(ITfEditSession)]
#[allow(dead_code)]
struct InsertComposeEditSession {
    context: ITfContext,
    /// 마지막 음절 제외 확정문(빈 문자열 가능 → step1 스킵).
    commit_text: String,
    /// 마지막 음절(빈 문자열이면 step2 스킵 = 역방향/undo).
    last_syllable: String,
    comp_sink: ITfCompositionSink,
    /// 마지막 음절 조합을 돌려받는 슬롯(step2 성공 시 Some).
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
    /// composition range 에 부여할 display-attribute atom (None 이면 skip).
    attr_atom: Option<u32>,
}

impl ITfEditSession_Impl for InsertComposeEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            crate::register::dbg_log(&format!(
                "InsertCompose.DoEditSession: enter commit_len={} last_len={}",
                self.commit_text.chars().count(),
                self.last_syllable.chars().count()
            ));

            // range 1회 획득 — 커서 위치. step1/step2 모두 이 range 를 재사용한다.
            let range = acquire_insert_range(&self.context, ec)?;

            // ── step1: 확정문 commit_text — composition-wrap 확정(네이티브 step3 모방) ──
            // raw SetText 직접 편집은 Blink 가 "selection 밖"으로 무시(첫 글자 누락)할 수
            // 있어 reconversion 동일 경로로 감싼다. EndComposition 으로 확정(BOOL(0)).
            if !self.commit_text.is_empty() {
                let wide_c: Vec<u16> = self.commit_text.encode_utf16().collect();
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                match ctx_comp.StartComposition(ec, &range, &self.comp_sink) {
                    Ok(composition) => {
                        let _ = select_replacement_range(&self.context, ec, &range);
                        let set_result = range.SetText(ec, 0, &wide_c);
                        // 커서(=range)를 확정문 끝으로 collapse → step2 가 그 뒤에서 시작.
                        let _ = range.Collapse(ec, TF_ANCHOR_END);
                        let _ = move_caret_to_end(&self.context, ec, &range);
                        let _ = composition.EndComposition(ec);
                        set_result?;
                    }
                    Err(e) => {
                        crate::register::dbg_log(&format!(
                            "InsertCompose: step1 StartComposition 거부({e:?}) → raw 폴백"
                        ));
                        let _ = select_replacement_range(&self.context, ec, &range);
                        range.SetText(ec, 0, &wide_c)?;
                        range.Collapse(ec, TF_ANCHOR_END)?;
                        let _ = move_caret_to_end(&self.context, ec, &range);
                    }
                }
            }

            // ── step2: 마지막 음절 last_syllable — 미확정 조합 시작(네이티브 step4 모방) ──
            // 같은 range(이미 끝으로 collapse 됨) 재사용. StartComposition→SetText 순서
            // 유지(역순은 CUAS 즉시-terminate 유발). EndComposition 호출 안 함(미확정 유지).
            if !self.last_syllable.is_empty() {
                let wide_p: Vec<u16> = self.last_syllable.encode_utf16().collect();
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                let composition = ctx_comp.StartComposition(ec, &range, &self.comp_sink)?;
                range.SetText(ec, 0, &wide_p)?;
                // 조합 selection = interim-char(BOOL(1)) inline 유지(fInterimChar 불변식).
                let _ = select_composition_range(&self.context, ec, &range);
                if let Some(atom) = self.attr_atom {
                    set_composition_attribute(&self.context, ec, &range, atom);
                }
                set_composition_reading(&self.context, ec, &range, &self.last_syllable);
                crate::register::dbg_log("InsertCompose: step2 composition slot filled (last alive)");
                *self.composition_slot.lock().unwrap() = Some(composition);
            }
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
    /// synth 폴백 슬롯. ShiftStart 가 delete_chars 만큼 못 움직이면(CUAS/Blink)
    /// 문서를 변형하지 않고 (delete_chars, commit_text, preedit_text, before_len)를
    /// 여기에 기록한 뒤 즉시 반환한다 — edit session 밖에서 SendInput 으로 폴백한다.
    /// before_len: D3 read-back 베이스라인(삭제 전 커서 앞 글자 수, None=읽기 실패).
    pub synth_slot: std::sync::Arc<std::sync::Mutex<Option<(u32, String, String, Option<u32>)>>>,
    /// composition range 에 부여할 display-attribute atom (None 이면 skip).
    pub attr_atom: Option<u32>,
}

impl ITfEditSession_Impl for ReplaceSurroundingEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> windows::core::Result<()> {
        unsafe {
            // 1. 실제 caret range 획득.
            //    과거엔 InsertTextAtSelection(QUERYONLY) 로 삽입 위치를 얻었으나,
            //    Blink/CUAS 에서는 그 range 가 확정 텍스트 뒤로 ShiftStart 역확장을
            //    거부하는 경우가 많다. GetSelection 으로 얻은 실제 selection range 가
            //    result-string 영역으로의 역확장(reconversion 의미)을 더 잘 받아들인다.
            //    GetSelection 실패 시 구(InsertTextAtSelection) 경로로 폴백한다.
            let range: ITfRange = {
                let mut sel = [TF_SELECTION::default(); 1];
                let mut fetched: u32 = 0;
                let got = self
                    .context
                    .GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched);
                let sel_range = if got.is_ok() && fetched > 0 {
                    let r = ManuallyDrop::take(&mut sel[0].range);
                    r
                } else {
                    None
                };
                match sel_range {
                    Some(r) => r,
                    None => {
                        let insert: ITfInsertAtSelection = self.context.cast()?;
                        insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?
                    }
                }
            };

            // 2. 커서 앞으로 ShiftStart (-delete_chars) — 교체 대상 범위 확보.
            //    shifted(실제 이동량)를 반드시 검증한다. 경계/확정 텍스트 거부로
            //    부족(shifted<delete_chars)하면 교체 대상이 비어 원문이 잔류하므로
            //    (B2 잔류 버그), 부족분만큼 commit 위치를 보정하기 위해 로깅하고
            //    이동된 범위만 교체한다(확보된 만큼은 정확히 덮어쓴다).
            range.Collapse(ec, TF_ANCHOR_START)?;
            if self.delete_chars > 0 {
                let mut shifted: i32 = 0;
                // ShiftStart 는 실제 이동량을 shifted 에 돌려줌. 경계 도달 시 부분 이동.
                let _ = range.ShiftStart(
                    ec,
                    -(self.delete_chars),
                    &mut shifted,
                    std::ptr::null(),
                );
                let mut abs_shifted_total = shifted.unsigned_abs() as i32;
                crate::register::dbg_log(&format!(
                    "ReplaceSurrounding: delete_chars={} shifted={} (full={})",
                    self.delete_chars,
                    shifted,
                    abs_shifted_total >= self.delete_chars
                ));
                // 부족분이 있으면(주로 Blink/CUAS) 추가 1회 재시도: Collapse 후 남은
                // 만큼 다시 역확장. 일부 store 는 두 번에 나눠야 확정 텍스트를 넘어선다.
                if abs_shifted_total < self.delete_chars {
                    let remaining = self.delete_chars - abs_shifted_total;
                    let mut shifted2: i32 = 0;
                    let _ = range.ShiftStart(ec, -remaining, &mut shifted2, std::ptr::null());
                    abs_shifted_total += shifted2.unsigned_abs() as i32;
                    crate::register::dbg_log(&format!(
                        "ReplaceSurrounding: retry shifted2={} (remaining={})",
                        shifted2, remaining
                    ));
                }

                // ── synth 폴백 판정(앱 이름 휴리스틱 금지, 동적 감지) ──
                // ShiftStart + 재시도 누적 이동량이 delete_chars 에 미달하면 CUAS/Blink
                // 처럼 확정 텍스트 뒤로 역확장이 거부된 것이다(shifted=-1, shifted2=0).
                // 이 edit session(TF_ES_SYNC)은 문서를 절대 변형하지 않고(SetText/
                // StartComposition 호출 금지 — 문서를 원본 그대로 유지해야 합성
                // 백스페이스가 실제 확정 텍스트에 작용) synth 슬롯에 요청만 기록한 뒤
                // 즉시 반환한다. 실제 SendInput 은 replace_surrounding 이 edit session
                // 밖에서 수행한다.
                if abs_shifted_total < self.delete_chars {
                    // D3: 삭제 전 커서 앞 글자 수를 같은 ec 로 read-back 측정해 게이트
                    // 베이스라인으로 슬롯에 싣는다. 문서는 변형하지 않는다(read-only 호출).
                    // 읽기 실패면 None → 호출부가 expected=-1 로 게이트 비활성(degrade).
                    let before_len = read_text_before_cursor_len(&self.context, ec);
                    crate::register::dbg_log(&format!(
                        "ReplaceSurrounding: ShiftStart 부족(got={} need={}) → synth_input 폴백 (before_len={:?})",
                        abs_shifted_total, self.delete_chars, before_len
                    ));
                    *self.synth_slot.lock().unwrap() = Some((
                        self.delete_chars as u32,
                        self.commit_text.clone(),
                        self.preedit_text.clone(),
                        before_len,
                    ));
                    return Ok(());
                }
            }

            // 3. 교체 — ⚠️ commit+compose 를 **한 ec** 에 하면 Blink 가 commit 정착 전에
            //    후속 조합 SetText 로 덮어써 앞 확정문이 유실된다(실측: "ntkdmes"→"현"만,
            //    "jbkfsuf"→"다"만 — full=true 네이티브 경로도 InsertCompose 와 동일 버그).
            //    그래서 분기한다:
            //    (A) 순방향(preedit 있음): 삭제된 range 에 **full 조합(commit+preedit)** 하나만
            //        만들어(미확정) composition_slot 에 싣는다. 확정은 호출부가 펌프 후
            //        Phase2b(commit_and_restart)로 수행 → commit 이 정착한다.
            //    (B) 역방향/commit-only(preedit 없음): 후속 조합이 없어 덮어쓰기 문제가 없으므로
            //        종전처럼 commit-wrap 으로 확정한다(단일 commit 은 같은 ec 에서도 정착).
            //    raw SetText 직접 편집은 Blink 가 무시/CUAS 미전달이므로 둘 다 composition 으로
            //    감싼다(reconversion 동일 경로). StartComposition 거부 시 raw 폴백.
            if !self.preedit_text.is_empty() {
                // (A) 순방향: full 조합 생성(삭제+commit+preedit 를 하나의 미확정 조합으로).
                let full: String = format!("{}{}", self.commit_text, self.preedit_text);
                let wide: Vec<u16> = full.encode_utf16().collect();
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                match ctx_comp.StartComposition(ec, &range, &self.comp_sink) {
                    Ok(composition) => {
                        // 교체 대상 range 를 selection 으로 명시(Blink "selection 밖 편집" 무시 방지).
                        let _ = select_replacement_range(&self.context, ec, &range);
                        // SetText 실패해도 조합을 반드시 닫아 누수 방지(B 분기와 동일 패턴).
                        let set_result = range.SetText(ec, 0, &wide);
                        if set_result.is_ok() {
                            // 미확정 조합 유지 — selection 전체 range(BOOL(1)) + READING(즉시-terminate 회피).
                            let _ = select_composition_range(&self.context, ec, &range);
                            if let Some(atom) = self.attr_atom {
                                set_composition_attribute(&self.context, ec, &range, atom);
                            }
                            set_composition_reading(&self.context, ec, &range, &full);
                            // slot 에 full 조합 보관 → 호출부가 store_pending_restart + Phase2b.
                            *self.composition_slot.lock().unwrap() = Some(composition);
                        } else {
                            // SetText 거부: 시작한 조합을 닫는다(slot 미적재 → 호출부 Normal).
                            let _ = composition.EndComposition(ec);
                        }
                        set_result?;
                    }
                    Err(e) => {
                        // 거부(드묾): raw 로 full 텍스트를 써 둔다(텍스트 보존). slot 은 비어
                        // 호출부가 Normal 처리(엔진 preedit 잔존 가능 — rare degrade).
                        crate::register::dbg_log(&format!(
                            "ReplaceSurrounding: full-composition StartComposition 거부({e:?}) → raw 폴백"
                        ));
                        let _ = select_replacement_range(&self.context, ec, &range);
                        range.SetText(ec, 0, &wide)?;
                        range.Collapse(ec, TF_ANCHOR_END)?;
                        let _ = move_caret_to_end(&self.context, ec, &range);
                    }
                }
            } else if self.delete_chars > 0 || !self.commit_text.is_empty() {
                // (B) 역방향/commit-only: 종전 commit-wrap 확정(후속 조합 없음 → 유실 무관).
                let wide: Vec<u16> = self.commit_text.encode_utf16().collect();
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                match ctx_comp.StartComposition(ec, &range, &self.comp_sink) {
                    Ok(composition) => {
                        let _ = select_replacement_range(&self.context, ec, &range);
                        let set_result = range.SetText(ec, 0, &wide);
                        let _ = range.Collapse(ec, TF_ANCHOR_END);
                        let _ = move_caret_to_end(&self.context, ec, &range);
                        let _ = composition.EndComposition(ec);
                        set_result?;
                    }
                    Err(e) => {
                        crate::register::dbg_log(&format!(
                            "ReplaceSurrounding: StartComposition 거부({e:?}) → raw 폴백"
                        ));
                        let _ = select_replacement_range(&self.context, ec, &range);
                        range.SetText(ec, 0, &wide)?;
                        range.Collapse(ec, TF_ANCHOR_END)?;
                        let _ = move_caret_to_end(&self.context, ec, &range);
                    }
                }
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

// ── EditSession: 확정 텍스트를 composition 으로 감싸 삽입 (b1 Phase2 확정문) ──

/// commit_text 를 StartComposition→SetText→EndComposition 으로 감싸 삽입한다.
///
/// b1 Phase2 의 "서"(마지막 음절 제외 확정문) 삽입 전용. raw SetText 직접 편집은
/// Blink 가 "현재 selection 밖"으로 무시할 수 있어(composition.rs:837 부정선례)
/// 첫 글자가 누락될 위험이 있으므로, reconversion 과 동일한 composition 경로로
/// 감싼다. StartComposition 거부 시 raw SetText 로 폴백한다.
#[implement(ITfEditSession)]
struct InsertCommitEditSession {
    context: ITfContext,
    text: String,
    comp_sink: ITfCompositionSink,
}

impl ITfEditSession_Impl for InsertCommitEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            let range = acquire_insert_range(&self.context, ec)?;
            let ctx_comp: ITfContextComposition = self.context.cast()?;
            match ctx_comp.StartComposition(ec, &range, &self.comp_sink) {
                Ok(composition) => {
                    // 교체 원자성: SetText 전에 삽입 range 를 selection 으로 명시해
                    // Blink 가 selection 밖 편집으로 무시하는 것을 막는다.
                    let _ = select_replacement_range(&self.context, ec, &range);
                    let set_result = range.SetText(ec, 0, &wide);
                    let _ = range.Collapse(ec, TF_ANCHOR_END);
                    let _ = move_caret_to_end(&self.context, ec, &range);
                    let _ = composition.EndComposition(ec);
                    set_result?;
                }
                Err(e) => {
                    crate::register::dbg_log(&format!(
                        "InsertCommit: StartComposition 거부({e:?}) → raw SetText 폴백"
                    ));
                    let _ = select_replacement_range(&self.context, ec, &range);
                    range.SetText(ec, 0, &wide)?;
                    let _ = range.Collapse(ec, TF_ANCHOR_END);
                    let _ = move_caret_to_end(&self.context, ec, &range);
                }
            }
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

/// b1/D3 read-back — 이미 보유한 edit cookie(`ec`)로 커서 앞 텍스트의 글자 수를 잰다.
///
/// OnEndEdit(read-only ec) 와 ReplaceSurrounding DoEditSession(read-write ec) 양쪽에서
/// 호출한다. **새 edit session 을 열지 않고** 호출자가 가진 ec 를 그대로 써서 중첩
/// 세션 금지 제약을 지킨다(설계 D3 §c). GetSelection → Collapse(START) →
/// ShiftStart(-margin) → GetText → chars().count() 순.
///
/// 반환: 커서 앞 글자 수(`Some`). selection 없음·GetText 실패 등 읽기 불가 시 `None`
/// (호출자는 게이트 비활성=현행 타이머 단독 폴백으로 degrade). margin 은 BMP 1코드
/// 유닛 가정(ATF 대상) 하에 충분한 윈도우(최대 4096)를 잡는다.
///
/// [shouldFix #2 한계 명시] 커서 앞 텍스트가 4096자를 초과하면 before_len·cur_len 둘 다
/// 4096 에 캡되어 `cur_len(4096) <= expected(=4096-d)` 가 영원히 거짓 → D3 read-back
/// 게이트가 열리지 않고 60ms 안전망 타이머 단독(=현행 b1)으로 안전 degrade 한다. 즉
/// 초장문 필드에서는 D3 의 결정성 이득만 사라질 뿐 동작은 깨지지 않는다(악화 없음).
///
/// SetText/StartComposition 을 절대 호출하지 않으므로 read-only ec 에서도 안전하다.
pub(crate) fn read_text_before_cursor_len(context: &ITfContext, ec: u32) -> Option<u32> {
    unsafe {
        let mut sel = TF_SELECTION::default();
        let mut fetched: u32 = 0;
        if context
            .GetSelection(ec, TF_DEFAULT_SELECTION, std::slice::from_mut(&mut sel), &mut fetched)
            .is_err()
            || fetched == 0
        {
            return None;
        }
        let sel_range = sel.range.as_ref()?.clone();
        // 커서(selection 끝) 앞 텍스트만 잰다.
        let cursor_range = sel_range.Clone().ok()?;
        cursor_range.Collapse(ec, TF_ANCHOR_END).ok()?;
        let mut shifted: i32 = 0;
        // 음수 ShiftStart 로 커서 앞으로 역확장(실패해도 GetText 로 읽히는 만큼 잰다).
        cursor_range
            .ShiftStart(ec, -4096, &mut shifted, std::ptr::null())
            .unwrap_or(());
        let mut buf = [0u16; 4096];
        let mut got: u32 = 0;
        if cursor_range.GetText(ec, 0, &mut buf, &mut got).is_err() {
            return None;
        }
        let text = String::from_utf16_lossy(&buf[..got as usize]);
        Some(text.chars().count() as u32)
    }
}
