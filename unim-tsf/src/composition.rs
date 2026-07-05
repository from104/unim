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

/// 조합 selection 을 환경(word 모드)에 맞게 설정한다.
///
/// - **word 모드(MS Word 단어 단위 조합)**: SetText 후 selection 을 range 끝으로 접힌
///   0폭 caret(`move_caret_to_end`)으로 둔다. 다음절 word 조합에서
///   `select_composition_range`(전체 range 선택)는 Word 가 caret/네모를 "선택 시작=첫
///   글자"에 두어 caret 이 최신 입력을 따라가지 못한다(캐럿이 단어 끝으로 안 따라감).
///   끝-caret 으로 두면 caret 이 마지막 글자 뒤를 따라간다. 미확정(밑줄) 표시는 호출부가
///   `set_composition_attribute`/`set_composition_reading` 을 **range 전체**에 별도로
///   부여하므로, caret 만 끝으로 접어도 underline 은 단어 전체에 남는다
///   ("전체 underline + 끝 caret"). fInterimChar 는 `move_caret_to_end` 의 BOOL(0)
///   (확정 caret 의미론)을 그대로 쓴다 — Word 는 정식 TSF text store 라 0폭 collapsed
///   selection 으로도 조합을 종료하지 않으며, 미확정 표시는 attribute 가 담당하므로
///   caret 의 interim 여부와 별개다.
/// - **syllable 모드·비-Word(wezterm/CUAS 터미널 등)**: 기존 `select_composition_range`
///   (전체 range, fInterimChar=BOOL(1)) 그대로 — CUAS 즉시-terminate 회피 불변식
///   (`select_composition_range` 주석) 유지. 1음절 조합 caret 동작 불변.
unsafe fn select_composition(
    context: &ITfContext,
    ec: u32,
    range: &ITfRange,
    word_mode: bool,
) -> Result<()> {
    if word_mode {
        move_caret_to_end(context, ec, range)
    } else {
        select_composition_range(context, ec, range)
    }
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
    /// **native(full=true) 전용** 펌프-분할 — 순방향에서 세션1 이 full 조합(commit+
    /// preedit)을 만들어 미확정 유지하고 (commit, preedit)을 PENDING_RESTART 에 적재했다.
    /// 마지막 음절을 TSF 조합으로 유지하므로 **엔진의 잔여 preedit 을 보존**해야 한다
    /// (호출부는 remove_preedit 를 호출하면 안 됨). text_service 가 SetTimer →
    /// WM_UNIM_FLUSH2 → Phase2b(commit_and_restart)로 commit 을 정착시킨다.
    PhaseSplit,
    /// synth 단일배치 폴백 — 삭제+교정문 전체(commit+마지막음절)를 한 SendInput
    /// (BS+UNICODE)으로 즉시 확정했다. 후속 스케줄링 없음. 호출부는 엔진
    /// commit+preedit 을 모두 클리어(remove_preedit)하고 schedule_flush 를 걸지 않는다.
    SynthBatch,
    /// **synth(full=false) 순방향 전용** (R6b) — 머리(commit)만 단일배치로 즉시 확정하고,
    /// 꼬리(preedit)는 `store_pending_tail` 에 적재했다. PENDING==0 게이트 후 라이브 조합으로
    /// 띄운다. 호출부는 **엔진 preedit("다")을 보존**(remove_preedit 금지)하고
    /// `schedule_flush=true`(안전망 타이머)로 둔다. 머리는 v0.3.39 와 동일하게 항상 확정(유실 0).
    SynthHeadTail,
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
    /// 확인해 synth 단일배치 폴백(send_replacement_batch = BS×N + UNICODE 교정문 전체를
    /// 한 SendInput 으로)으로 처리한다.
    ///
    /// 4번째 요소 `before_len`(=삭제 전 커서 앞 글자 수, edit session 내부 read-back)은
    /// 단일배치 전환 후 미사용(과거 D3 readback 게이트 기준선)이나, edit session 쪽
    /// 기록 코드 회귀 면적을 줄이기 위해 슬롯 형태는 유지하고 replace_surrounding 이
    /// `_before_len` 으로 무시한다.
    synth_slot: Arc<Mutex<Option<(u32, String, String, Option<u32>)>>>,
    /// 단어별 preedit caret 정책 힌트(`set_word_mode_hint` 로 키 처리 직전 갱신).
    ///
    /// `engine.is_word_mode()`(코어 — Windows TSF 에서는 winword.exe 포커스 게이트)를
    /// 프론트(key_handler)가 읽어 전달한다. true(MS Word 단어 모드)면 start/update/
    /// commit_and_restart 조합 edit session 이 조합 selection 을 끝-caret 으로 둔다
    /// (caret 이 마지막 글자를 따라감). false(syllable/비-Word)면 기존 전체 range
    /// selection 을 유지한다(CUAS 즉시-terminate 회피 불변식). word 모드는 winword.exe
    /// 에서만 켜지므로(text_service word-gate), false 경로는 종전과 바이트 동일하다.
    word_mode_hint: bool,
    /// 단어별 preedit Phase 4 — Word 협조앱 영문 라이브 보유 버퍼.
    ///
    /// 영문 모드 + word 게이트(winword) + 협조앱(`!known_cuas`)에서, 코어가 즉시 확정한
    /// 영문 알파벳을 문서에 바로 삽입하지 않고 committed=0 라이브 조합으로 누적한다. 그래야
    /// ATF 순방향(영→한)이 이 조합을 교정 한글로 SetText 치환할 수 있다(Word 가 확정문 삭제는
    /// 차단하지만 미확정 조합 SetText 는 허용 — D1). 누적 문자열 자체가 보유 영문의 source of
    /// truth 이며, 비어 있으면 "보유 중 아님"을 뜻한다. CUAS/터미널/Blink/IMM32 경로에서는
    /// **절대** 채우지 않는다(호출부 게이트가 보장 — 과거 B2 즉시-terminate 학습 재발 차단).
    english_hold: String,
}

impl CompositionManager {
    pub fn new() -> Self {
        Self {
            composition: None,
            composition_slot: Arc::new(Mutex::new(None)),
            attr_atom: None,
            synth_slot: Arc::new(Mutex::new(None)),
            word_mode_hint: false,
            english_hold: String::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.composition.is_some()
    }
    pub fn clear(&mut self) {
        self.composition = None;
        // Phase 4 — 조합 추적이 끊기면(외부 terminate/포커스 등) 보유 영문도 함께 폐기한다.
        // 보유 영문 조합이 외부에서 terminate 되면 Word 가 interim 텍스트를 문서에 보존하므로
        // (재삽입 불필요) 누적만 비워 "문서=확정, 보유=없음" 동기를 맞춘다.
        self.english_hold.clear();
    }

    // ── Phase 4: Word 협조앱 영문 라이브 보유 ──

    /// 보유 영문 조합이 현재 떠 있는지(누적 문자열이 비어있지 않은지) 반환한다.
    pub fn english_hold_active(&self) -> bool {
        !self.english_hold.is_empty()
    }

    /// 현재 보유 중인 영문 문자열 사본(길이 비교·진단용).
    pub fn english_hold_text(&self) -> String {
        self.english_hold.clone()
    }

    /// 새 영문 문자를 라이브 조합으로 누적한다(start 또는 update).
    ///
    /// 활성 조합이 없으면 `start_composition`, 있으면(=보유 영문 연장) `update_composition`
    /// 으로 조합을 연장한다. 누적 문자열(`english_hold`)이 보유 영문의 source of truth 다.
    /// `word_mode_hint`(끝-caret) 를 그대로 사용한다. 호출부(key_handler)는 Word + 협조앱 +
    /// 영문 모드 + 단일 알파벳일 때만 호출한다(게이트). 코어 영문 commit 시맨틱은 불변.
    pub fn hold_english(
        &mut self,
        context: &ITfContext,
        tid: u32,
        ch: &str,
        comp_sink: &ITfCompositionSink,
    ) {
        self.english_hold.push_str(ch);
        let text = self.english_hold.clone();
        if self.composition.is_some() {
            self.update_composition(context, tid, &text);
        } else {
            self.start_composition(context, tid, &text, comp_sink);
        }
    }

    /// 보유 영문을 영문 그대로 확정하고 보유를 종료한다(경계/네비게이션 패스쓰루).
    ///
    /// 활성 조합이 있으면 `end_composition_with_text`(보유 영문), 없으면(드묾) `insert_text`
    /// 로 확정한다. 어느 쪽이든 누적은 비운다. ATF 미발동(정상 영문) 단어가 space/구두점/
    /// Enter/방향키 등에서 매끄럽게 확정되는 경로다.
    pub fn confirm_english_hold(&mut self, context: &ITfContext, tid: u32) {
        if self.english_hold.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.english_hold);
        if self.composition.is_some() {
            self.end_composition_with_text(context, tid, &text);
        } else {
            self.insert_text(context, tid, &text);
        }
    }

    /// 보유 영문 라이브 조합에서 마지막 문자 1개를 지운다(Phase 4 S1 Backspace).
    ///
    /// `english_hold` 의 마지막 char 를 pop 하고, 남으면 `update_composition` 으로 재렌더
    /// (캐럿 끝), 비면 활성 조합을 `end_composition` 으로 종료하고 보유를 해제한다. 보유가
    /// 없었으면 아무 동작도 하지 않는다. 반환값은 pop 이후에도 보유가 남아있는지
    /// (true=조합 유지, false=종료/해제됨). **영문 모드 보유 영문 전용** — 한글 word 모드
    /// Backspace(코어 키스트로크 재생)와는 호출부 게이트로 분리한다. ATF 키스트로크 버퍼
    /// 동기 축소는 호출부(key_handler)가 별도로 담당한다(관심사 분리).
    pub fn backspace_english_hold(&mut self, context: &ITfContext, tid: u32) -> bool {
        if self.english_hold.is_empty() {
            return false;
        }
        self.english_hold.pop();
        if self.english_hold.is_empty() {
            if self.composition.is_some() {
                self.end_composition(context, tid);
            }
            false
        } else {
            let text = self.english_hold.clone();
            self.update_composition(context, tid, &text);
            true
        }
    }

    /// 보유 영문 누적만 비운다(조합 객체는 보존 — 호출부가 한글 word 조합으로 전용).
    ///
    /// ATF 순방향 치환에서, 보유 영문 라이브 조합을 교정 한글로 `update_composition` 한 직후
    /// 호출한다. 같은 `ITfComposition` 이 이제 한글 word 라이브 조합으로 계속 떠 있으므로
    /// (committed=0, 이어치기 가능) 누적 영문만 폐기한다.
    pub fn clear_english_hold(&mut self) {
        self.english_hold.clear();
    }

    /// 단어별 preedit caret 정책 힌트를 설정한다(start/update/commit_and_restart 가 읽음).
    ///
    /// key_handler 가 조합 dispatch 직전 `engine.is_word_mode()` 값을 넘긴다. 자세한
    /// 의미는 `word_mode_hint` 필드 주석 참조. 조합 표시(selection caret)만 바꾸며 코어
    /// 한글 조합 로직·ATF·synth 와 무관하다.
    pub fn set_word_mode_hint(&mut self, on: bool) {
        self.word_mode_hint = on;
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
            word_mode: self.word_mode_hint,
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
                word_mode: self.word_mode_hint,
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

    /// 활성 조합을 **range 텍스트를 지우지 않고**(SetText(empty) 미호출) EndComposition
    /// 만으로 종료한다. TSF 규약상 EndComposition 은 조합만 끝낼 뿐 range 텍스트를 지우지
    /// 않으므로(참고: EndCompositionEditSession text=None 주석), 조합 중이던 자모가 확정
    /// 텍스트로 그대로 남는다(materialize).
    ///
    /// 용도 — 역방향(한→영) 교정: 조합 음절을 clear(=range.SetText(empty))로 제거하는 대신
    /// 확정 텍스트로 materialize 한 뒤, 호출부가 replace_surrounding 의 delete_chars 를 1
    /// 늘려 committed+조합 전체 span 을 통째 삭제한다. end_composition(clear)의 SetText(empty)
    /// 가 xterm.js(wmux)/일부 CUAS 에서 시각적으로 무효라 조합 자모가 잔류 → 첫 자모가
    /// 생존하던 버그("ㄹㅊㅊ"→"ㄹoo")를 앱-독립적으로 회피한다(정상앱은 결과 동일).
    pub fn end_composition_keep_text(&mut self, context: &ITfContext, tid: u32) {
        if let Some(ref composition) = self.composition {
            let session = EndCompositionKeepEditSession {
                context: context.clone(),
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
            word_mode: self.word_mode_hint,
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
    /// 반환(ReplaceOutcome):
    /// - `Normal`: 정상 TSF 경로(ShiftStart 충분, 역방향/commit-only) 또는 synth 미진입.
    /// - `PhaseSplit`: **native(full=true)** 순방향. 세션1 이 full 조합을 만들고
    ///   PENDING_RESTART 에 적재했다. 호출부는 **엔진 preedit 보존** + SetTimer →
    ///   Phase2b(commit_and_restart) 펌프-분할(text_service).
    /// - `SynthBatch`: synth 단일배치 폴백(full=false). 삭제+교정문 전체를 한 SendInput
    ///   (BS+UNICODE)으로 즉시 확정했다. 호출부는 엔진 preedit 을 클리어하고 후속
    ///   스케줄링을 걸지 않는다.
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
        if let Some((d, c, p, _before_len)) = synth_req {
            // synth 폴백: 삭제+삽입을 SendInput(BS+UNICODE)으로 처리. edit session 이 만든
            // composition 은 없다(슬롯 기록만) → 추적 비움. SendInput 은 반드시 edit
            // session(COM lock) 밖인 이 시점에서 호출한다.
            self.composition = None;
            if p.is_empty() {
                // 역방향/commit-only(preedit 없음): 종전 단일배치(전체=commit) 유지.
                crate::register::dbg_log(&format!(
                    "ReplaceSurrounding: synth 단일배치(역/commit-only) del={d} commit_len={}",
                    c.chars().count()
                ));
                crate::synth_input::send_replacement_batch(d, &c);
                return ReplaceOutcome::SynthBatch;
            }
            // 순방향(p="다"): R6b — 머리(commit)만 단일배치 확정 + 꼬리 라이브 예약.
            // store_pending_tail 을 send_replacement_batch 보다 먼저 호출(동기 동일 틱이라
            // 순서는 안전하나 has_pending_tail 선행 보장). 머리만 보내므로
            // PENDING = del + commit_units(꼬리 제외).
            crate::synth_input::store_pending_tail(&p);
            crate::register::dbg_log_ev!(
                &format!(
                    "ReplaceSurrounding: synth head-tail del={d} head_len={} tail_len={}",
                    c.chars().count(),
                    p.chars().count()
                ),
                "ReplaceSurrounding: synth head-tail del={d} head_len={} tail='{p}'",
                c.chars().count()
            );
            crate::synth_input::send_replacement_batch(d, &c);
            // 머리 UNICODE 유닛 수 기록 — conhost/Blink 은 이 유닛의 keydown echo 가 없어
            // BS echo 드레인 후 PENDING 이 여기서 멈춘다. flush_pending_tail 이 이 값으로
            // "삭제 완료"(PENDING<=residual)를 판정해 라이브/degrade 를 가른다.
            crate::synth_input::set_head_residual(c.encode_utf16().count() as i32);
            return ReplaceOutcome::SynthHeadTail;
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
    /// word 모드(MS Word 단어 단위 조합)면 끝-caret, 아니면 전체 range selection.
    word_mode: bool,
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

            // selection: word 모드면 끝-caret(caret 이 마지막 글자 따라감), 그 외엔
            // composition range 전체(TF_AE_NONE, wezterm terminate 회피).
            let _ = select_composition(&self.context, ec, &range, self.word_mode);

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
    /// word 모드(MS Word 단어 단위 조합)면 끝-caret, 아니면 전체 range selection.
    word_mode: bool,
}

impl ITfEditSession_Impl for UpdateCompositionEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let range = self.composition.GetRange()?;
            let wide: Vec<u16> = self.text.encode_utf16().collect();
            range.SetText(ec, 0, &wide)?;
            // selection: word 모드면 끝-caret(caret 이 마지막 글자 따라감), 그 외엔
            // composition range 전체(SampleIME 방식, wezterm terminate 회피).
            let _ = select_composition(&self.context, ec, &range, self.word_mode);
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

// ── EditSession: 조합 종료(텍스트 보존) — 자모 materialize ──

#[implement(ITfEditSession)]
struct EndCompositionKeepEditSession {
    context: ITfContext,
    composition: ITfComposition,
}

impl ITfEditSession_Impl for EndCompositionKeepEditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            // range 텍스트를 건드리지 않는다 — 조합 자모가 확정 텍스트로 남는다.
            // 이어지는 replace_surrounding 의 ShiftStart 삭제가 이 자모까지 균일하게
            // 지운다(앱-독립).
            //
            // ⚠️ caret 을 조합 range 끝(자모 뒤)으로 **명시 collapse** 한다. keep 은 구
            // end_composition 과 달리 SetText(empty)를 하지 않으므로, 그냥 EndComposition
            // 만 하면 selection 이 조합 range 전체(자모 P 를 감싼 범위)로 남을 수 있다.
            // 그러면 후속 ReplaceSurrounding 이 GetSelection→Collapse(ANCHOR_START)로 P
            // **앞**에 앵커를 잡아 ShiftStart(-(committed+1))이 P 앞 글자를 침범(앞 확정문
            // 삭제)하거나 P 를 남긴다. move_caret_to_end 로 caret 을 P 뒤로 정규화해
            // 삭제 span 의 기준점을 앱 재량과 무관하게 결정론화한다(구 end_composition 의
            // move_caret_to_end 승계). SetText 는 하지 않으므로 P 는 그대로 확정 텍스트.
            let range = self.composition.GetRange()?;
            let _ = move_caret_to_end(&self.context, ec, &range);
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
    /// word 모드(MS Word 단어 단위 조합)면 새 조합 selection 을 끝-caret 으로 둔다.
    word_mode: bool,
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
            // selection: word 모드면 끝-caret, 그 외엔 전체 range(wezterm terminate 회피).
            let _ = select_composition(&self.context, ec, &new_range, self.word_mode);
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
                // sink 비대칭 앱(wmux/Blink 터미널 — OnTestKeyDown 미발화)에서는 committed
                // 텍스트의 ShiftStart 가 phantom-성공(shifted=-N)하지만 SetText 가 pty 화면에
                // 렌더링되지 않는다(역방향 "ㄹㅊㅊ"→"ㄹㅊo" 깨짐). ShiftStart 를 아예 건너뛰고
                // 곧장 synth 폴백(BS×N + UNICODE)으로 보낸다 — 순방향이 이미 쓰는, 실제 키
                // 입력을 존중하는 경로. 정상 TSF 앱은 sink_asymmetric()=false 라 미진입(무변경).
                // 순방향(preedit 有)은 어차피 ShiftStart 실패로 동일 synth 에 도달하므로 결과 동일.
                if crate::synth_input::sink_asymmetric() {
                    let before_len = read_text_before_cursor_len(&self.context, ec);
                    let (t, k) = crate::synth_input::sink_counters();
                    crate::register::dbg_log(&format!(
                        "ReplaceSurrounding: sink-asymmetric(test={t} kd={k}) → synth 강제 라우팅 (delete_chars={} before_len={:?})",
                        self.delete_chars, before_len
                    ));
                    *self.synth_slot.lock().unwrap() = Some((
                        self.delete_chars as u32,
                        self.commit_text.clone(),
                        self.preedit_text.clone(),
                        before_len,
                    ));
                    return Ok(());
                }
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
            // fetched 는 GetText 계약상 버퍼 길이 이하지만, CUAS/IMM32 브리지 등
            // 오작동 앱이 초과값을 돌려주면 슬라이스 인덱스 초과로 패닉→host abort 가
            // 난다. 버퍼 길이로 clamp 해 방어(정상 경로는 min 이 no-op 라 동작 불변).
            let anchor_n = (anchor_fetched as usize).min(anchor_buf.len());
            let anchor_text = String::from_utf16_lossy(&anchor_buf[..anchor_n]);
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
            // (위 anchor 와 동일) fetched 초과값 방어 — 인덱스 초과 패닉→host abort 차단.
            let cursor_n = (cursor_fetched as usize).min(cursor_buf.len());
            let cursor_text_before = String::from_utf16_lossy(&cursor_buf[..cursor_n]);
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
        // fetched(got) 초과값 방어 — 슬라이스 인덱스 초과 패닉→host abort 차단
        // (정상 경로는 got<=buf.len() 라 min 이 no-op, 동작 불변).
        let got_n = (got as usize).min(buf.len());
        let text = String::from_utf16_lossy(&buf[..got_n]);
        Some(text.chars().count() as u32)
    }
}
