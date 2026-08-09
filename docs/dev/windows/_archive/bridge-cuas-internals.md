# Bridge investigation: CUAS internals (msctf/ctfime ↔ IMM32 변환 메커니즘)

조사 각도: **cuas-internals**. 질문 — CUAS(Cicero Unaware App Support)가 TIP의 TSF
composition을 IMM32 앱의 `WM_IME_COMPOSITION`(GCS_COMPSTR)으로 변환하는 정확한
메커니즘과 *조건*. 왜 wezterm에서 빈 composition도 즉시 종료되는가. composition을
살아있게 유지하는 필수 호출 시퀀스/인터페이스가 문서/역공학으로 알려졌는가.

방법: MS Learn 1차 소스 + 실제 TIP 소스(Google Mozc) + Chromium TSF text store +
Wine msctf/imm32. raw 본문은 컨텍스트 미반입. ctx_index source 라벨
`bridge-cuas-internals`.

---

## 0. 결론 요약 (적대적 재검증 결과)

- **선행 결론 "순수 TSF TIP는 레거시(IMM32) inline 불가"는 거짓.** Google Mozc는
  순수 TSF TIP(IMM32 .ime 아님)이며, CUAS 경유로 IMM32-only/콘솔 앱에서 inline
  preedit을 정상 표시한다. 우리(UNIM)도 가능하다. (출처: mozc `win32/tip/*` 전체가
  TSF TIP 구현이고 IMM32 앱 지원을 명시적으로 다룸)
- **"MS 한국어 IME만 IMM32 .ime 하이브리드 특권"도 근거 없음.** Win10/11 MS IME는
  TSF TIP다. 특권이 아니라 *CUAS-호환 호출 시퀀스*를 지킬 뿐. Mozc가 같은 시퀀스로
  서드파티로서 동작하는 것이 반례.
- **빈 composition이 즉시 종료되는 것은 "빈 composition 자체" 때문이 아니다.**
  Mozc는 오히려 **빈/collapsed range로 StartComposition을 먼저 한 뒤** 텍스트를
  채운다(아래 §2). 따라서 UNIM의 선행 가설 #2("빈 composition으로 시작하지 말고
  SetText 먼저")는 Mozc 동작과 **정반대**이며 진짜 원인이 아닐 가능성이 높다.
- **즉시 종료의 진짜 후보**: (a) display attribute(`GUID_PROP_ATTRIBUTE`) +
  reading(`GUID_PROP_READING`) property를 composition range에 제대로 못 박음,
  (b) composition을 만든 edit session과 텍스트/attribute를 채우는 edit session
  사이의 *비어있는 순간*에 CUAS가 스냅샷, (c) HWND/포커스 컨텍스트 미스매치
  (transitory extension vs base context 혼동).

---

## 1. CUAS의 정체 — "default text store / context owner" 역할 대행

IMM32-only 앱(wezterm)은 `ITextStoreACP`도 `ITfContextOwnerCompositionSink`도
구현하지 않는다. 이때 **CUAS(msctf.dll 내부)가 그 앱을 대신해 context owner /
text store 역할을 수행**한다. TSF의 composition 생명주기는 전적으로 이
CUAS-제공 내부 default store가 관장한다.

- `ITfContextComposition::StartComposition`은 context owner의
  `ITfContextOwnerCompositionSink::OnStartComposition(pComposition, BOOL* pfOk)`을
  호출한다. **`*pfOk = FALSE`면 composition이 거부**되고 StartComposition은 S_OK를
  반환하되 `ppComposition == NULL`이 된다. (출처: MS Learn
  nf-msctf-itfcontextcomposition-startcomposition; nf-...-onstartcomposition —
  "Receives a nonzero value to allow the composition or zero to deny")
  → **CUAS-unaware 앱에서는 이 sink를 CUAS가 구현**한다. 즉 composition 허용/거부
  여부와 즉시 종료(`OnEndComposition` → 우리 쪽 `ITfCompositionSink::OnCompositionTerminated`)는
  **CUAS가 내부 판단으로 결정**한다.
- `OnStartComposition`/`OnEndComposition`의 구현 DLL은 MS Learn 기준
  **`Msimtf.dll`** (IMM↔TSF 브리지 DLL). composition 변환 로직이 여기·msctf·
  ctfime(MSCTFIME IME)에 분산. (출처: MS Learn 두 함수 Requirements 표 "DLL: Msimtf.dll")
- Chromium `TSFTextStore`(앱 측 store 구현 예)는 `RequestLock`/`OnStartComposition`에서
  edit lock과 `*ok=TRUE`로 composition을 허용한다. CUAS는 이 store를 *대신* 구현하는
  것. (출처: chromium ui/base/ime/win/tsf_text_store.cc RequestLock L624, OnStartComposition L1009)

**CUAS default store는 result만 지원하는 게 아니라 composition(GCS_COMPSTR)도
지원한다.** 그렇지 않으면 MS IME/Mozc가 wezterm에서 inline 조합을 못 띄웠을 것.
즉 "CUAS는 result만 브리지한다"는 가설은 틀렸다.

---

## 2. composition을 살아있게 유지하는 정확한 호출 시퀀스 (Mozc 1차 소스)

Mozc `win32/tip/tip_edit_session_impl.cc`의 `CreateComposition` (L113~135) — IMM32
앱에서 검증된 시퀀스:

```
auto composition_context = ComQuery<ITfContextComposition>(context);
auto insert_selection   = ComQuery<ITfInsertAtSelection>(context);
ITfRange* insertion_pos;
insert_selection->InsertTextAtSelection(write_cookie, TF_IAS_QUERYONLY,
                                        nullptr, 0, &insertion_pos);  // 빈 collapsed range
composition_context->StartComposition(write_cookie, insertion_pos,
                                      CreateCompositionSink(...), &composition);
```

1. `InsertTextAtSelection(TF_IAS_QUERYONLY, text=null, len=0)` — **텍스트를 넣지 않고**
   현재 selection 위치의 collapsed range만 획득.
2. 그 **빈 range로 StartComposition** → composition 시작.
3. 이후(같은/후속 edit session) `composition_range->SetText(write_cookie, 0,
   preedit, len)` 로 미확정 문자열을 채우고, `GUID_PROP_ATTRIBUTE` display
   attribute property를 segment별로 SetValue. (출처: mozc tip_edit_session.cc
   `SetText(...preedit...)`; tip_edit_session_impl.cc "Set each segment's display
   attribute" / input_attribute / converted_attribute)

→ **즉 Mozc도 "빈 composition 먼저"가 정석.** wezterm에서 그 빈 composition이
즉시 종료되지 않는다. 따라서 UNIM의 즉시 종료는 "빈 composition" 탓이 아니다.

### 종료(commit)의 CUAS 내부 메커니즘 (Mozc 주석 = 역공학 지식)
tip_edit_session_impl.cc L137~150 주석:
1. composition 생성(없으면).
2. composition range 텍스트를 commit할 텍스트로 교체. **CUAS는
   `GUID_PROP_READING` property의 segment 구조로 GCS_RESULTCLAUSE /
   GCS_RESULTREADCLAUSE를 만든다.**
3. `ITfComposition::ShiftStart`로 composition range를 축소 — **range 밖으로
   밀려난 텍스트가 "확정 텍스트(GCS_RESULTSTR)"로 해석**된다.
4. caret 위치 명시적 갱신(WPF TextBox 등은 자동 갱신 안 함).
   (출처: mozc tip_edit_session_impl.cc CommitText 주석, b/8406545, b/9747361)

**핵심 판별 규칙(확정):** CUAS는 *composition range 안에 남아있는 텍스트*를
GCS_COMPSTR(미확정/밑줄), *range 밖으로 밀려나거나 EndComposition으로 떨어진
텍스트*를 GCS_RESULTSTR(확정)로 분류한다. display attribute(`GUID_PROP_ATTRIBUTE`)는
GCS_COMPSTR의 attribute 바이트(밑줄 스타일)로 변환된다.

### 빈 preedit → 종료 (Mozc)
tip_edit_session.cc: `if (!output.has_preedit())` 일 때만 composition range를
`SetText(..., L"", 0)`로 비우고 `EndComposition`. preedit이 있으면 절대 비우지
않는다. → composition이 활성인 동안에는 항상 텍스트가 들어있어야 한다.

---

## 3. Transitory Extension & CUAS 식별 (undocumented GUID — 1차 소스)

Mozc `win32/tip/tip_transitory_extension.cc`:

- **Undocumented GUID `{A94C5FD2-C471-4031-9546-709C17300CB9}`** —
  `ITfCompartmentMgr::EnumGuid()`로 발견. 이 compartment 값이 **VT_I4이고 0x01
  비트가 set이면 해당 `ITfDocumentMgr`는 CUAS가 구현한 것**(레거시 IMM32 앱)이며
  그 `ITfContext`는 실제 surrounding text를 반환하지 않는다.
  (출처: mozc tip_transitory_extension.cc `kTsfEmulatedDocumentMgrGuid`)
- 구분해야 할 3가지: (1) Transitory Extension 통한 완전 TSF-aware, (2) **CUAS가
  TSF surrounding text API를 완전히 제공하지 않는 레거시 IMM32 앱**, (3)
  TF_SS_TRANSITORY 명시 TSF 앱. (출처: 동 파일 주석)
- 레거시 IMM32 앱에서는 surrounding text를 `IMR_DOCUMENTFEED`(IMM32 reconvert)로
  폴백해서 얻는다. (출처: mozc tip_surrounding_text.cc `GetSurroundingTextImm32`)
- `GUID_COMPARTMENT_TRANSITORYEXTENSION` 값: NONE / FLOATING / ATSELECTION.
  (출처: MS Learn "Values for GUID_COMPARTMENT_TRANSITORYEXTENSION")
- 콘솔(conhost)·windowless 컨트롤은 `ITfContextView::GetWnd`가 NULL HWND를 줄 수
  있음 — composition 표시 좌표/HWND 연동 시 NULL 가정 필요. (출처: MS Learn
  nf-msctf-itfcontextview-getwnd Remarks)

**UNIM 적용 함의:** wezterm 컨텍스트가 위 GUID로 "CUAS-emulated"로 식별되는지
런타임 확인하면, MS IME가 보는 컨텍스트와 우리가 보는 컨텍스트가 같은지(=base
context vs transitory)를 가릴 수 있다. composition을 잘못된(비-CUAS) 컨텍스트에
걸면 즉시 종료될 수 있다.

---

## 4. UNIM 현 상태 대비 (composition.rs 감사)

UNIM은 이미 Mozc 정석 시퀀스를 따르고 있음:
- `acquire_insert_range`: `ITfInsertAtSelection::InsertTextAtSelection(TF_IAS_QUERYONLY, &[])`
  1순위, 실패 시 GetSelection 폴백. (composition.rs L116~135, L498)
- StartComposition 후 **2단계 별도 세션**에서 초기 preedit 텍스트 + display
  attribute를 채움. (L241 주석)
- `set_composition_attribute`: `GUID_PROP_ATTRIBUTE` SetValue + 직후 GetValue
  재확인 진단 로깅. (L26~63) — attribute는 박히고 있음(P1 진단 코드 존재).

→ 구조적으로 Mozc와 거의 동일한데도 즉시 종료된다면, 차이는 **세부 타이밍/
컨텍스트/property**에 있다. 다음을 우선 검증:

1. **`GUID_PROP_READING` 미설정.** Mozc는 commit/조합 양쪽에서 reading property를
   쓴다. CUAS가 GCS_RESULTCLAUSE/READCLAUSE를 reading segment로 만든다는 점에서,
   reading property 부재가 CUAS의 composition 분류를 깨뜨릴 수 있음(미검증, 1순위 실험).
2. **StartComposition과 SetText의 edit session 분리.** Mozc는 동일 DoEditSession
   흐름 안에서 create→fill을 잇는다. UNIM의 "2단계 별도 세션"이 *비어있는 활성
   composition* 상태를 한 틱 노출 → CUAS가 그 순간 빈 range를 GCS_RESULTSTR(빈
   확정)로 스냅샷·종료할 수 있음. **create와 첫 SetText를 단일 edit session으로
   합치는 실험** 권장(2순위, Mozc와 정합).
3. **컨텍스트 선택(base vs transitory)**: §3의 undocumented GUID로 우리가 잡은
   context가 CUAS-emulated인지 확인. MS IME가 거는 컨텍스트와 동일한지 비교.
4. **CreateCompositionSink 반환 처리**: StartComposition에 넘기는 sink가 즉시
   `OnCompositionTerminated`를 받는다면, OnStartComposition 단계에서 CUAS가
   `*pfOk=FALSE`(거부)했을 가능성 — `ppComposition`이 NULL인지 로그 확인.

---

## 5. 서드파티 선례 (이 메커니즘 실사용)

- **Google Mozc** — 순수 TSF TIP. `win32/tip/`이 CUAS/IMM32 앱 대응을 1급으로
  다룸(transitory extension 식별, IMR_DOCUMENTFEED 폴백, ShiftStart commit).
  콘솔/IMM32 앱 inline 동작. (출처: github.com/google/mozc/tree/master/src/win32/tip)
- **Chromium** — 앱 *측* `TSFTextStore` 구현으로 동일 인터페이스 계약을 보여줌
  (CUAS가 대행하는 것의 레퍼런스). (출처: chromium ui/base/ime/win/tsf_text_store.cc)
- AIMM(IActiveIMMApp) 경로는 사용하는 활성 IME 없음(자매 조사 `bridge-active-imm-aimm`).

---

## 6. 1차 소스 목록 (URL)

- MS Learn StartComposition: learn.microsoft.com/.../nf-msctf-itfcontextcomposition-startcomposition
- MS Learn OnStartComposition (pfOk 거부): .../nf-msctf-itfcontextownercompositionsink-onstartcomposition
- MS Learn OnEndComposition (DLL=Msimtf.dll): .../nf-msctf-itfcontextownercompositionsink-onendcomposition
- MS Learn Compositions 개요: learn.microsoft.com/.../tsf/compositions
- MS Learn GUID_COMPARTMENT_TRANSITORYEXTENSION values
- MS Learn ITfContextView::GetWnd
- Mozc tip_edit_session_impl.cc (CreateComposition / CommitText / ShiftStart 주석)
- Mozc tip_edit_session.cc (SetText preedit / EndComposition on empty)
- Mozc tip_transitory_extension.cc (undocumented GUID A94C5FD2, CUAS 식별)
- Mozc tip_surrounding_text.cc (IMR_DOCUMENTFEED 폴백)
- Chromium ui/base/ime/win/tsf_text_store.cc (RequestLock / OnStartComposition 계약)
- Wine dlls/msctf/context.c, dlls/imm32/imm.c (GCS_COMPSTR/GCS_RESULTSTR 변환 참고; Wine msctf는 다수 STUB)
