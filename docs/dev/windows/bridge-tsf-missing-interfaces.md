# Bridge angle: tsf-missing-interfaces — adversarial re-verification

조사 각도: 우리 TSF TIP 코드와 MS SampleIME / Mozc 의 *남은 차이*를 1차 소스로 정밀 비교해,
"wezterm inline 불가"가 구조적 한계가 아니라 *우리가 빠뜨린 것* 일 가능성을 검증한다.

선행 결론(이번에 적대적으로 재검증한 대상):
- "P1 — `ITfContextOwnerCompositionSink` 미구현이 MS IME 와의 결정적 차이, 1순위 수정"
  (research-unim-composition-audit.md:34-38, research-SYNTHESIS-cuas-inline.md:14)

## 핵심 반증 결과 (가장 중요)

### 후보 (3) `ITfContextOwnerCompositionSink` 미구현 = **기각 (선행 결론이 틀림)**

`ITfContextOwnerCompositionSink` 는 *TIP* 가 아니라 *애플리케이션(문서/컨텍스트 소유자)* 가 구현하는 인터페이스다.

- MS Learn(mslearn-ContextOwnerCompositionSink): "The interface is implemented by an **application**
  to receive composition-related notifications. When the **application** calls
  `ITfDocumentMgr::CreateContext`, the TSF manager queries the object for this interface."
  → 즉 컨텍스트를 만든 주체(레거시 앱의 경우 **CUAS 자신**)가 구현·소유한다. TIP 이 advise 하는 sink 가 아니다.
- MS Learn(mslearn-StartComposition Remarks): "If the **context owner** has installed a context owner
  composition advise sink, `OnStartComposition` is called. If the advise sink **rejects** the new
  composition, this method returns S_OK but ppComposition is set to **NULL**."
  → 이 sink 는 composition 을 *거부* 하는 쪽(소유자/CUAS)이다. TIP 이 이걸 구현해봤자 자기 composition 을 자기가 승인하는 무의미.

3대 1차 소스 교차검증 — **TIP 측 구현 목록에 ContextOwnerCompositionSink 는 어디에도 없다**:
- SampleIME `CSampleIME` 상속 목록(sampleime-SampleIME-h): `ITfCompositionSink` 만 있음, ContextOwnerCompositionSink 없음.
- Mozc `TipTextServiceImpl` 의 `TipComImplements<...>` 목록(mozc-tip-text-service): ContextOwnerCompositionSink 없음.
  Mozc 의 composition sink 는 별도 `CompositionSinkImpl : TipComImplements<ITfCompositionSink>` 로 `OnCompositionTerminated` 만 구현.
- 우리 `UnimTextService` `#[implement(...)]`(text_service.rs:23-31): `ITfCompositionSink` 구현, ContextOwnerCompositionSink 없음.

→ **세 IME 가 전부 동일하게 `ITfCompositionSink` 만 구현**한다. ContextOwnerCompositionSink 미구현은 MS IME 와의 차이가 *아니다*.
   선행 P1 "1순위 수정 A" 는 근거 없음. 구현해도 wezterm inline 이 켜질 가능성 낮음(위험만 증가).

## 진짜로 발견된 차이 (후보 1) — composition 생성/충전이 **2개의 독립 top-level 세션**

이게 SampleIME 와의 *실제* 구조적 차이이며, 우리 코드 주석이 SampleIME 를 *잘못* 인용하고 있다.

우리 코드(composition.rs `start_composition`):
- phase1: `StartCompositionEditSession` 으로 `RequestEditSession(TF_ES_READWRITE|TF_ES_SYNC)` → **빈** composition 만 생성(텍스트 없음). 세션 종료.
  (composition.rs:213-235, DoEditSession 본문 364-)
- phase2: 세션이 끝난 뒤 별도로 `update_composition` 호출 → **또 다른** `RequestEditSession(TF_ES_SYNC)` 로 SetText.
  (composition.rs:237-247, update_composition 250-)
- 주석(composition.rs:241-243, 367-376)은 "SampleIME 가 빈 조합 먼저 만들고 키핸들러 세션에서 _AddComposingAndChar 로 채우는 2-phase 구조" 라고 적었지만 **사실과 다름**.

SampleIME 실제(sampleime-StartComposition-cpp `CStartCompositionEditSession::DoEditSession`):
- **단일 세션** 안에서 `InsertTextAtSelection(TF_IAS_QUERYONLY)` → `StartComposition(ec, range, _pTextService, &comp)` → `SetSelection`.
  텍스트 SetText 는 같은 키 핸들러 흐름의 `_SetInputString`/`_AddComposingAndChar` 가 처리하지만 모두 **동일 ec(edit cookie)** 내.
- `_StartComposition` 자체도 `RequestEditSession(TF_ES_SYNC|TF_ES_READWRITE)` 를 쓰되, 한 세션 안에서 start+selection 을 끝낸다.
- 즉 SampleIME 는 "빈 조합을 만들고 세션을 끝낸 뒤, *다른* 세션에서 텍스트를 채우는" 짓을 하지 않는다.

함의: 우리의 phase1(빈 composition + 세션 종료)이 CUAS-unaware 경로에서 즉시-terminate 의 *원인일 수 있다*.
빈 composition 으로 세션을 닫는 순간 CUAS 가 "빈 GCS_COMPSTR → 종료" 로 해석할 여지. SampleIME 처럼 한 세션에서 start+SetText
를 끝내면 첫 동기화 시점에 이미 비어있지-않은 미확정 문자열이 존재.

주의(반대 가설): 선행 실측은 "한 세션에서 start+SetText 하면 즉시-terminate 라서 일부러 2-phase 로 쪼갰다"(composition.rs:369-375)고 기록.
그렇다면 단일-세션이 *이미* 시도되어 실패했을 수 있음. 단 그 실측이 SetText→Start 순서(잘못된 순서, composition.rs:530-531 가 경고)
였는지, 올바른 Start→SetText 단일 세션이었는지 로그로 불명확. **SampleIME 정확 순서(Start→SetSelection, 같은 ec 내 SetText)** 재시도 가치 있음.

## 후보 (2) display attribute — **무죄 확정**

우리 set_composition_attribute(composition.rs:26-) 는 SampleIME/Mozc 와 동일 메커니즘:
- `ITfCategoryMgr::RegisterGUID(&UNIM_DISPLAY_ATTR_INPUT)` → TfGuidAtom (composition.rs:206) — Mozc `InitDisplayAttributes`
  `category_->RegisterGUID(...)` 와 동일.
- `GetProperty(GUID_PROP_ATTRIBUTE)` → `SetValue(ec, range, VARIANT{VT_I4, atom})` (composition.rs:27,40) — SampleIME
  `_SetCompositionDisplayAttributes` 의 `pLanguageProperty->SetValue(ec, pRangeComposition, &var)` 와 동일.
- `InputDisplayAttribute` provider: `lsStyle=TF_LS_SOLID(1)`, `bAttr=TF_ATTR_INPUT(0)` (display_attr.rs:47-49) — 정상.
- 카테고리 등록 GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER 는 wxs 가 박음(register.rs:178-179).
- 런타임 진단: SetValue FAILED/GetValue MISMATCH 0건(P1 로그) → attribute 는 실제로 박힌다.

→ display attribute 는 원인이 아님. (단 wezterm 은 IMM32 라 GCS_COMPATTR 를 자체 렌더에 거의 안 씀 — attribute 유무 자체가 inline 여부를 좌우하지 않음.)

## 후보 (4) 포커스 document manager / context 선택 — **정상**

- OnKeyDown 이 받는 `pic`(ITfContext)를 그대로 composition 에 사용(text_service.rs:344-371). 별도 GetFocus/GetTop 으로
  엉뚱한 context 를 집지 않음 — SampleIME 의 `_pContext`(키 핸들러가 받은 그 context) 사용과 동일.
- ActivateEx 에서 AdviseKeyEventSink + ThreadMgrEventSink advise 정상(text_service.rs:197-214).
- 즉 context/focus 선택 오류 가능성 낮음.

## 빌드 없이 시도할 수정 후보 (위험도 순)

### 수정 1 (저위험, 최우선) — **단일 세션 start+SetText (SampleIME 정확 복제)**
- composition.rs `start_composition` 의 phase1/phase2 분리를 폐기.
- 한 `RequestEditSession(TF_ES_SYNC|TF_ES_READWRITE)` 안에서:
  `InsertTextAtSelection(QUERYONLY)` → `ctx_comp.StartComposition(ec, range, comp_sink, &comp)` →
  `range.SetText(ec, 0, wide)` → `set_composition_attribute` → `SetSelection`(caret 끝).
  (이미 ReplaceSurroundingEditSession composition.rs:528-541 에 거의 동일 단일-세션 코드가 존재 — 그 패턴을 일반 경로로 승격.)
- 위험: 선행 실측이 "단일 세션 = 즉시 terminate" 라 기록 → 효과 없을 수 있음. 단 그 실측의 정확한 순서가 불명.
  되돌리기 쉬움(주석된 2-phase 보존). **MVP 1순위.**

### 수정 2 (저위험) — 선행 P1 "수정 A(ContextOwnerCompositionSink advise)" **착수 금지**
- 본 조사로 근거 없음 확정. 시간/위험 낭비. drop.

### 수정 3 (중위험, 별도 각도 필요) — wezterm 은 IMM32-only 이므로 TSF SetText 가 IMM32 GCS_COMPSTR 로 브리지되는지가 본질.
- 이건 tsf-missing-interfaces 각도의 범위를 넘음(CUAS↔IMM32 브리지 동작). 본 각도 결론: TIP 인터페이스 누락은 원인 아님.
  다른 각도(cuas-bridge / immersive vs legacy app input mode)에서 추적해야 함.

## 한 줄 결론
`ITfContextOwnerCompositionSink` 미구현은 *원인이 아니다*(SampleIME·Mozc 도 미구현). 우리만의 진짜 차이는
**composition 을 빈 채로 만들고 세션을 닫은 뒤 별도 세션에서 채우는 2-phase 구조**다. SampleIME 처럼
단일 세션 start+SetText 로 되돌리는 것이 빌드-전 최우선 시도다. 단 이것이 wezterm inline 을 켤지는
CUAS↔IMM32 브리지 동작에 달려 있어 본 각도만으로 단정 불가.

## 1차 소스
- MS Learn ITfContextOwnerCompositionSink: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontextownercompositionsink
- MS Learn StartComposition (Remarks: context owner rejects → ppComposition NULL): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition
- MS Learn OnStartComposition (pfOk allow/deny): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextownercompositionsink-onstartcomposition
- SampleIME StartComposition.cpp (단일 세션): https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/StartComposition.cpp
- SampleIME SampleIME.h (CSampleIME 상속 목록): https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/SampleIME.h
- SampleIME Composition.cpp (_AddComposingAndChar / _SetCompositionDisplayAttributes): https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/Composition.cpp
- Mozc tip_text_service.cc (TipTextServiceImpl 인터페이스 목록 / CompositionSinkImpl): https://github.com/google/mozc/blob/master/src/win32/tip/tip_text_service.cc
- 우리 코드: unim-tsf/src/text_service.rs:23-31,197-214,344-371 / composition.rs:26-69,206,213-247,250-,364-,528-541 / display_attr.rs:20-53
