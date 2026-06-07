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

/// 조합 상태 관리자
pub struct CompositionManager {
    composition: Option<ITfComposition>,
    composition_slot: Arc<Mutex<Option<ITfComposition>>>,
    /// `GUID_PROP_ATTRIBUTE` SetValue 용 캐시된 `TfGuidAtom`.
    /// 최초 1회 `ITfCategoryMgr::RegisterGUID` 로 획득해 재사용(매번 등록 금지).
    attr_atom: Option<u32>,
}

impl CompositionManager {
    pub fn new() -> Self {
        Self {
            composition: None,
            composition_slot: Arc::new(Mutex::new(None)),
            attr_atom: None,
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
    pub fn commit_and_restart(
        &mut self,
        context: &ITfContext,
        tid: u32,
        commit_text: &str,
        preedit_text: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        let Some(old) = self.composition.clone() else {
            if !commit_text.is_empty() {
                self.insert_text(context, tid, commit_text);
            }
            self.start_composition(context, tid, preedit_text, comp_sink);
            return;
        };
        let slot = self.composition_slot.clone();
        *slot.lock().unwrap() = None;
        let attr_atom = self.attr_atom();
        let session = CommitRestartEditSession {
            context: context.clone(),
            old_composition: old,
            commit_text: commit_text.to_string(),
            preedit_text: preedit_text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
            attr_atom,
        };
        let session_intf: ITfEditSession = session.into();
        unsafe {
            let _ = context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC);
        }
        self.composition = self.composition_slot.lock().unwrap().take();
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
        let attr_atom = self.attr_atom();

        let session = ReplaceSurroundingEditSession {
            context: context.clone(),
            delete_chars: delete_chars as i32,
            commit_text: commit_text.to_string(),
            preedit_text: preedit_text.to_string(),
            comp_sink: comp_sink.clone(),
            composition_slot: slot.clone(),
            attr_atom,
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
    /// composition range 에 부여할 display-attribute atom (None 이면 skip).
    pub attr_atom: Option<u32>,
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
                // SampleIME 패턴: 빈 range 에 StartComposition 먼저, 그 뒤 SetText.
                // (CUAS-unaware 앱에서 SetText→Start 순서는 즉시-terminate 유발.)
                let ctx_comp: ITfContextComposition = self.context.cast()?;
                let composition = ctx_comp.StartComposition(ec, &range, &self.comp_sink)?;
                range.SetText(ec, 0, &wide)?;
                // composition 시작 — selection 전체 range (wezterm terminate 회피)
                let _ = select_composition_range(&self.context, ec, &range);
                // 순방향 replay preedit 에도 미확정(밑줄) ATTRIBUTE + READING 부여(실패 무시).
                if let Some(atom) = self.attr_atom {
                    set_composition_attribute(&self.context, ec, &range, atom);
                }
                set_composition_reading(&self.context, ec, &range, &self.preedit_text);
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
