# TSF 입력기를 wezterm / PuTTY 등 비-TSF(IMM32) 앱에서 동작시키기 — 기술 조사

> 상태 표기: **확정** = 공식 문서/소스/UNIM 코드로 검증됨, **추정** = 강한 추론(라이브 재확인 권장), **미상** = 추가 조사 필요.
> 조사 출처: SampleIME 소스(github.com/microsoft/Windows-classic-samples, Samples/IME/cpp/SampleIME — SampleIME.cpp / KeyEventSink.cpp / Composition.cpp / KeyHandlerEditSession.cpp), MS Learn TSF 개요, CUAS 설명(blog/alibaba TSF 자료), UNIM `unim-tsf/src/*` 직접 확인.

---

## 0. UNIM 현재 구현 — 사실 정정 (직접 확인)

요청서의 일부 가정이 **코드와 다름**. 확정 사실:

- **edit-session 이미 사용 중.** `composition.rs:102` `context.RequestEditSession(tid, &session_intf, TF_ES_READWRITE | TF_ES_SYNC)`. start/update/end 모두 `ITfEditSession::DoEditSession(ec)` 안에서 range/SetText/SetSelection 수행. → "편집을 edit-session 밖에서 한다"는 **사실 아님**.
- **comp_sink(ITfCompositionSink) 이미 전달.** `composition.rs:226` `ctx_comp.StartComposition(ec, &range, &self.comp_sink)`. → composition ownership sink 자체는 **존재**.
- **InsertAtSelection→GetSelection 폴백 이미 있음.** `composition.rs:40-61` `acquire_insert_range`.
- **caret 이동 처리 있음.** `move_caret_to_end` (SetText 후 SSelection Collapse END).
- **ActivateEx 는 `text_service.rs:91`** (lib.rs 아님). `_dwflags` **무시 확정**. AdviseKeyEventSink·ThreadMgrEventSink·LangBar 등록은 함. PreserveKey 미등록(의도적).
- **OnTestKeyDown(`text_service.rs:242`) 에 focus/empty-context/keyboard-disabled 가드 없음.** `pic` context 만 보고 engine 상태로 consumed 판정. SampleIME 는 `_IsKeyEaten` 에서 `GetFocus`→`GetTop(context)`→`GUID_COMPARTMENT_KEYBOARD_DISABLED`/`OPENCLOSE` 검사. → **차이점.**
- **`OnCompositionTerminated` 이미 구현됨** (`text_service.rs:322`) — comp_mgr.clear + engine.reset + 팝업 hide + ATF reset. → P4 사실상 완료.
- **UNIM 의 range 획득 방식이 SampleIME 와 다름(핵심).** UNIM `acquire_insert_range` 는 `InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])`(빈 텍스트)로 range 만 얻고, 이후 `SetText` 로 채운 뒤 StartComposition(`composition.rs:227-231`). **SampleIME 는 `pias->InsertTextAtSelection(ec, TF_IAS_QUERYONLY, pstrAddString->Get(), len, &range)` 로 실제 텍스트를 함께 넘긴다**(StartComposition.cpp 확인). → CUAS/콘솔에서 빈 QUERYONLY 거동이 다를 수 있는 **핵심 차이.**

즉 UNIM 의 composition 파이프라인은 이미 SampleIME 구조와 유사. 결함은 **composition 흐름 자체보다 다른 곳**에 있을 가능성이 높다.

---

## TL;DR (핵심 결론)

1. **MS IME 가 wezterm·PuTTY 에서 되는 핵심 메커니즘:**
   - (a) **CUAS (Cicero Unaware Application Support)** — `msctf.dll` 의 IMM32↔TSF 에뮬레이션 레이어. PuTTY 같은 순수 IMM32 앱은 CUAS 가 만든 가짜 TSF document/context 위에서 TIP 가 동작. **확정(아키텍처)**.
   - (b) CUAS 경로에서 composition 의 **preedit 가 IMM32 `GCS_COMPSTR` 로 번역**된다. 따라서 TIP 가 composition 을 표준대로 시작/유지하면 보여야 함. **확정/추정**.

2. **가장 유력한 UNIM 실패 원인(좁힘):**
   - **(I) 빈 QUERYONLY range 에 StartComposition.** UNIM 은 `InsertTextAtSelection(QUERYONLY, 빈텍스트)` 또는 GetSelection 으로 얻은 **0길이 range** 에 SetText 후 `StartComposition`. **SampleIME 는 `InsertTextAtSelection` 에 실제 텍스트를 함께 넘겨** 비어있지 않은 range 를 받는다(StartComposition.cpp 확인). CUAS/IMM32/콘솔 컨텍스트에서 빈 QUERYONLY 거동이 GUI 앱과 달라 조합 시작이 무시될 수 있다. **추정(강) — 1순위 의심.**
   - **(II) display attribute 미적용.** composition range 에 display attribute(GUID_PROP_ATTRIBUTE/TF_DISPLAYATTRIBUTE)를 안 걸면 CUAS 가 IMM32 GCS_COMPSTR 변환 시 preedit 를 못 그려 "조합이 안 보이고 입력 실패"처럼 보일 수 있다. **추정.**
   - **(III) OnTestKeyDown 가드 부재로 인한 라우팅 차이.** wezterm/putty 의 CUAS 컨텍스트에서 context/focus 상태가 GUI 앱과 달라, 가드 없는 판정이 오작동할 가능성. **추정(중).**

3. **wezterm vs PuTTY: 원인이 다를 수 있음.**
   - PuTTY = 순수 IMM32 → **CUAS 경로**.
   - wezterm(`use_ime=true`) = Windows 에서 자체 IME 핸들링. **IMM32 기반인지 TSF-aware 인지 라이브 확인 필요**(Q4 분기점). IMM32 면 PuTTY 와 동일 원인.

---

## Q1. CUAS / IMM32 호환

**확정:** Windows XP SP2 이후 CUAS 가 기본 활성. CUAS 는 IMM32 앱의 `ImmGetContext`/`WM_IME_*` 와 TSF TIP 사이를 양방향 브리지하는 에뮬레이션 레이어다. TIP 입장에서는 정상 TSF document/context 로 보이며, **원칙적으로 표준 TSF 흐름만 따르면 자동 동작**이 설계 의도다(출처: CUAS 설명 자료, mozilla bug 866736 InputScope+CUAS).

**추정 — CUAS 경로의 함정:**
1. composition 의 preedit 는 CUAS 가 `GCS_COMPSTR` 로 번역. **display attribute 가 없으면** IMM32 앱이 조합 문자열을 못 받거나 안 그릴 수 있음.
2. CUAS 가짜 컨텍스트에서 **빈(0길이) range 에 StartComposition** 이 GUI 앱보다 까다롭게 동작할 수 있음 → 더미 컴포지션 필요.
3. `ITfInsertAtSelection` 가 CUAS 컨텍스트에서 실패 → UNIM 은 GetSelection 폴백으로 range 는 얻지만, 그 range 가 0길이라 (I) 문제로 이어짐.

**결론(Q1):** PuTTY 실패는 **앱 한계 아님**(MS IME 는 됨 = CUAS 경로 정상). UNIM 이 CUAS 가 기대하는 composition 디테일(더미 컴포지션 / display attribute)을 못 맞춘 것이 유력. → **코드로 해결 가능.**

---

## Q2. ActivateEx dwflags (TF_TMAE_* / TF_TMF_*)

**확정(UNIM):** `text_service.rs:91` 에서 `_dwflags` 무시. `Activate` 는 `ActivateEx(...,0)` 위임(`:149`).

**확정(SampleIME 대조):** SampleIME `ActivateEx` 도 본질적으로 dwFlags 를 받아 `_pThreadMgr` 저장 후 sink 초기화만 하며, **dwFlags 분기는 secure/comless 같은 특수 상황에 한정**(`_IsSecureMode`/`_IsComLess` 헬퍼). 일반 데스크톱·콘솔에서는 dwFlags 무시해도 무방.

**판정:** dwflags 무시는 **wezterm/putty 실패의 직접 원인이 아닐 가능성이 큼**. 단 진단을 위해 각 앱에서 들어오는 dwflags 를 **로깅**할 가치는 있음. 우선순위는 낮춤(P4 → 진단용 P0-log).

---

## Q3. SampleIME 의 콘솔/IMM32 처리 — UNIM 과의 핵심 차이

**확정(소스):**
- `_IsKeyEaten` (KeyEventSink.cpp): 키 소비 판정 전에 `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`(IME on/off), `_IsKeyboardDisabled()` 검사. `_IsKeyboardDisabled` 은 `GetFocus`→`GetTop(context)` 가 없으면 disabled 처리.
- OnKeyDown 은 `_IsKeyEaten` 으로 eaten 판정 후 **`CKeyHandlerEditSession` 을 RequestEditSession 으로 invoke** → 그 안에서 composition 처리. UNIM 도 동등(edit-session 사용).
- **`_StartComposition` (Composition.cpp):** InsertTextAtSelection(QUERYONLY)로 range → StartComposition → **더미 컴포지션** 흔적(`_RemoveDummyCompositionForComposing`). 이것이 UNIM 과의 **가장 큰 composition 차이**.
- composition 텍스트 갱신 시 `SetText` 사용("we've already started a composition, we don't want the app to adjust the insertion point") — UNIM 과 동일 철학.

**UNIM 과의 핵심 차이(우선순위):**
1. **더미 컴포지션 패턴 부재** (UNIM 은 빈 range 에 바로 StartComposition). ← 1순위 의심.
2. **display attribute 미적용** (UNIM `display_attr.rs` 존재하나 composition range 에 거는지 확인 필요 — **미상, 확인 필요**).
3. **OnTestKeyDown focus/keyboard-disabled 가드 부재.**

---

## Q4. wezterm vs console — 같은 원인인가?

**확정:** wezterm 은 conhost 가 아니라 자체 GUI 윈도우. conhost 가 UNIM 폴백으로 통과한 것은 conhost 의 TSF 컨텍스트가 InsertAtSelection 실패 후 GetSelection range 로도 composition 을 받아줬기 때문.

**추정(라이브 확인 필요):** wezterm Windows IME 백엔드가
- **IMM32 기반**이면 → PuTTY 와 **같은 CUAS 경로**, 같은 원인(더미 컴포지션/display attr).
- **TSF-aware**이면 → conhost 와 유사하나 InsertAtSelection 거동이 달라 폴백 range 에 StartComposition 이 거부될 수 있음(I).

**판정:** **wezterm·PuTTY 는 "같은 계열(빈 range 에 composition 시작 실패)" 일 가능성이 높다**고 잠정 결론. 단 wezterm 의 IMM32/TSF 여부가 분기점 → `github.com/wez/wezterm` 에서 `use_ime` / `ImmGetContext` / `ITfThreadMgr` 확인 권장.

---

## Q5. 누락 가능성 높은 인터페이스/동작

**확정/추정(우선순위):**
1. **더미 컴포지션 패턴** (인터페이스 아닌 동작) — 1순위.
2. **display attribute 적용** (`ITfProperty` GUID_PROP_ATTRIBUTE on composition range) — CUAS preedit 표시용. UNIM `display_attr.rs` 가 composition range 에 실제로 걸리는지 **확인 필요**.
3. `ITfContextOwnerCompositionSink` — UNIM 은 이미 `ITfCompositionSink` 전달. `OnCompositionTerminated` 핸들러 구현 여부 **확인 필요**(외부가 조합 끊을 때 정리). SampleIME 는 구현함.
4. OnTestKeyDown 가드 (focus/keyboard-disabled/openclose) — 라우팅 안정성.
5. `ITfMouseTrackerACP`, `ITfTextLayoutSink` — 입력 실패와 **무관**, 후순위.

---

## MS IME 동작 메커니즘 — 1~2개로 좁힘

1. **CUAS 브리지** — PuTTY·(IMM32 모드) wezterm 의 동작 인프라. **확정.**
2. **CUAS 친화적 composition 디테일** — 더미 컴포지션으로 비어있지 않은 range 확보 + display attribute 로 preedit 를 GCS_COMPSTR 에 실어보냄. MS IME 는 이를 충실히 함. **추정(강).**

→ UNIM 결함은 (2)를 못 맞춰 (1) 경로에서 composition 이 시작/표시 안 되는 것으로 좁혀진다.

---

## UNIM 적용 코드 변경 (우선순위 / 난이도 / 위험)

### [P0-log] dwflags + composition HRESULT 로깅 (즉시, 진단)
- **대상:** `text_service.rs:91` ActivateEx (`_dwflags` 로그), `composition.rs:226` StartComposition / `:42` InsertTextAtSelection 의 HRESULT 로그.
- **목적:** wezterm/putty/conhost/notepad 각각에서 (a)들어오는 dwflags, (b)InsertAtSelection 성공/실패, (c)StartComposition HRESULT 를 측정. **모든 후속 수정의 근거.**
- **난이도:** 매우 낮음. **위험:** 없음. **효과:** 진단 필수.

### [P1] InsertTextAtSelection 에 실제 텍스트 전달 — 가장 유력
- **대상:** `composition.rs` `acquire_insert_range`(`:40`) + `StartCompositionEditSession::DoEditSession`(`:223-231`).
- **확정 차이:** UNIM 은 `InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])` (빈 텍스트)로 range 만 받고 별도 `SetText`. SampleIME(StartComposition.cpp)는 **`InsertTextAtSelection(ec, TF_IAS_QUERYONLY, 텍스트, len, &range)` 로 텍스트를 함께 넘김**. CUAS/콘솔 가짜 컨텍스트에서 빈 QUERYONLY+분리 SetText 조합이 거부/무시될 수 있음.
- **변경:** start 경로에서 `InsertTextAtSelection` 에 첫 조합 텍스트를 직접 넘겨 비어있지 않은 range 를 받은 뒤 StartComposition. (또는 SampleIME 의 더미 컴포지션 `_RemoveDummyCompositionForComposing` 패턴 이식.) windows-rs 0.62.2: `ITfInsertAtSelection::InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &wide)` 의 3번째 인자에 실제 텍스트 슬라이스.
- **난이도:** 중. **위험:** 중(정상 GUI 앱 회귀 주의 — P0-log 로 확인). **효과:** 큼.

### [P2] composition range 에 display attribute 적용
- **대상:** `display_attr.rs` + `composition.rs` SetText 직후.
- **변경:** composition range 에 `ITfProperty(GUID_PROP_ATTRIBUTE)` 로 display attribute atom 설정 → CUAS 가 GCS_COMPSTR 로 변환 가능. 이미 `display_attr.rs` 가 atom 등록만 하고 range 적용을 안 한다면 그 연결을 추가.
- **난이도:** 중. **위험:** 낮~중. **효과:** preedit 표시·CUAS 호환.

### [P3] OnTestKeyDown 포커스/disabled 가드
- **대상:** `text_service.rs:242` OnTestKeyDown, `key_handler::test_key_down`.
- **변경:** `GetFocus`→`GetTop(context)` null 이면 미소비; `GUID_COMPARTMENT_KEYBOARD_DISABLED` 검사. SampleIME `_IsKeyEaten` 패턴.
- **난이도:** 낮~중. **위험:** 낮음. **효과:** 라우팅 안정(부차적).

### [P4] OnCompositionTerminated — 이미 구현됨 (확인 완료)
- `text_service.rs:322` 에서 comp_mgr.clear + engine.reset + 팝업 hide + ATF reset 처리됨. **추가 작업 불필요**(P1/P2 적용 후 회귀만 확인).

### 권장 착수 순서
**P0-log(측정) → 측정 결과로 P1(더미 컴포지션) vs P2(display attr) 우선순위 확정 → 재측정 → P3/P4.**

---

## "앱 한계라 불가능" vs "코드로 해결 가능"

- **코드로 해결 가능(거의 확실):** PuTTY. MS IME 가 정상 동작 = IMM32/CUAS 경로 멀쩡. UNIM composition 디테일(P1/P2) 수정으로 해결 기대.
- **코드로 해결 가능(추정):** wezterm. 단 사용자 `use_ime=true` 전제. `use_ime=false` 면 앱 설정 문제(코드로 못 고침).
- **앱 한계(불가능):** PuTTY·wezterm 은 MS IME 로 동작 확인됨 → 이 범주 아님. (만약 IMM32·TSF 둘 다 미지원 raw-key 앱이라면 불가능하나 본 두 앱은 해당 없음.)

---

## 재확인 체크리스트 (라이브)

1. UNIM `display_attr.rs` 가 composition range 에 실제로 display attribute 를 거는지. (코드 확인)
2. UNIM `ITfCompositionSink::OnCompositionTerminated` 구현 여부. (코드 확인)
3. SampleIME `Composition.cpp` `_StartComposition` / `_RemoveDummyCompositionForComposing` 전체 본문 — 더미 컴포지션 정확한 구현.
4. wezterm Windows IME 백엔드 IMM32 vs TSF (`github.com/wez/wezterm`, `use_ime`).
5. MS Learn `ITfTextInputProcessorEx::ActivateEx` dwflags 비트 의미(`TF_TMAE_*`/`TF_TMF_*`) — 현재 학습 링크 404, 정식 URL 재확인 필요.
