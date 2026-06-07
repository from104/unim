# UNIM TSF Composition Audit — wezterm/CUAS 즉시-종료(immediate OnCompositionTerminated) 원인 감사

대상: `unim-tsf/src/{composition.rs, text_service.rs, display_attr.rs, globals.rs}`,
`unim-tsf/src/register.rs`, `installer/wix/unim.wxs`.
질문: IMM32 앱(wezterm)에서 StartComposition 직후 OnCompositionTerminated 가 즉시 발생.
MS 기본 IME 는 같은 앱에서 정상 inline 조합 → **우리 측 composition 라이프사이클/edit-session/
SetText/display-attribute 처리** 중 CUAS 브리지를 깨는 패턴 식별.

---

## 0. 사실 확인 (코드 근거)

| 항목 | 상태 | 근거 |
|---|---|---|
| `ITfCompositionSink` 구현 | O | text_service.rs:26, impl @ 415 |
| `ITfContextOwnerCompositionSink` 구현 | **X (미구현/미등록)** | `#[implement(...)]` 목록 text_service.rs:23-31 에 없음 |
| `ITfDisplayAttributeProvider` 구현 | O | text_service.rs:29, impl @ 570 |
| DISPLAYATTRIBUTEPROVIDER 카테고리 등록 | O | unim.wxs:122-126 `{046B8C80-...}` |
| 모든 edit session | **TF_ES_SYNC** | composition.rs:232, 260, 275, 291, 304 |
| Start: SetText → StartComposition → SetSelection → SetValue(attr) | O | composition.rs:368-410 |
| Update: SetText → SetSelection → SetValue(attr) 매번 | O | composition.rs:430-438 |
| selection style | `TF_AE_NONE`, fInterimChar=0 | composition.rs:103-106 |
| OnEndEdit | no-op | text_service.rs:522-530 |

핵심: **display-attribute 경로는 이미 정상적으로 배선되어 있다**(provider 구현 + 카테고리 등록 +
매 세션 SetValue + GetValue 재확인). 즉 "attribute 가 없어서 CUAS 가 result-string 으로 오인" 이라는
기존 가설(composition.rs:16-20 주석)은 **이미 대응 완료** 상태다. 그럼에도 즉시-종료가 난다면
원인은 attribute 부재가 아니라 **다른 곳**에 있다. 아래 순위는 이 전제 위에서 매겼다.

---

## 1. 즉시-종료 유발 후보 (가능성 순위)

### 후보 #1 (최유력) — `ITfContextOwnerCompositionSink` 미구현
- 근거: text_service.rs:23-31 의 `#[implement]` 에 `ITfContextOwnerCompositionSink` 없음.
  우리는 `ITfCompositionSink`(composition 별 종료 통지) 만 구현.
- 메커니즘: **CUAS(IMM32 브리지)는 context owner 가 없는 컨텍스트**다. SampleIME 가 wezterm 류에서
  동작하는 결정적 차이가 바로 `ITfContextOwnerCompositionSink::OnStartComposition /
  OnUpdateComposition / OnEndComposition` 을 통해 CUAS 가 composition 시작을 **승인(allow)**
  하는 핸드셰이크다. 이 sink 가 없으면 CUAS 는 "이 컨텍스트에서 inline composition 을 보증할
  주체가 없다"고 판단해 StartComposition 을 즉시 되돌린다(= OnCompositionTerminated 직후 발생).
- 정합성: MS 기본 IME 는 이 sink 를 구현하므로 같은 wezterm 에서 정상 동작 → 관찰된 증상과 일치.
- 위험도: 가장 설명력 높고, 코드상 "빠진 인터페이스" 라는 명확한 결함.

### 후보 #2 — Start 시 빈 composition 으로 시작하지 않고 SetText 먼저 함
- 근거: composition.rs:368-389 — `acquire_insert_range` 로 얻은 range 에 **먼저 SetText(텍스트
  삽입)** 한 뒤(371) 그 range 로 StartComposition(389) 한다.
- 메커니즘: CUAS/IMM32 브리지는 "비어있는 selection 에서 StartComposition → 이후 range 에 SetText"
  순서를 기대한다(IMM 의 GCS_COMPSTR 점진 갱신 모델). 텍스트가 **이미 박힌 range** 를 composition
  으로 감싸면 CUAS 가 그 텍스트를 "이미 확정된 문자열" 로 간주하고 composition 을 무효화할 수 있다.
  SampleIME 는 StartComposition 을 빈/collapsed selection 에 먼저 걸고 그 다음 SetText 한다.
- 위험도: #1 과 결합해 작동하는 2차 요인일 가능성. 단독 원인 가능성도 있음.

### 후보 #3 — 매 update 마다 `SetSelection(TF_AE_NONE)` 으로 전체 range 선택
- 근거: composition.rs:404, 434 `select_composition_range` → style `TF_AE_NONE`,
  `fInterimChar = BOOL(0)` (composition.rs:103-106).
- 메커니즘: 한글 조합 중 미확정 1글자는 IMM 의 **interim character**(fInterimChar=TRUE) 로
  표현되는 것이 자연스럽다. `fInterimChar=0` + 전체 range를 active selection 으로 만드는 패턴은
  CUAS 가 "사용자가 텍스트를 선택했다 → 조합 종료" 로 해석할 소지가 있다. 특히 매 키마다 전체 range
  re-select 는 IMM 브리지에 caret/selection 변화 폭주로 비칠 수 있다.
- 위험도: 중간. 단독으로 즉시-종료를 일으키기보다 불안정성 가중 요인.

### 후보 #4 — 전부 `TF_ES_SYNC` + SetText 후 같은/직후 세션 추가조작
- 근거: composition.rs:232 등 모든 세션 `TF_ES_READWRITE | TF_ES_SYNC`.
- 메커니즘: SYNC 세션 안에서 SetText → StartComposition → SetSelection → SetValue 를 한 호흡에
  처리하고 즉시 반환한다. CUAS 는 SYNC edit session 종료 시점에 GCS 변경을 한꺼번에 commit 하는데,
  composition 시작과 텍스트/선택/속성 변경이 **동일 commit 안에 묶이면** "조합 시작과 동시에 내용
  확정" 으로 압축 해석될 수 있다. ASYNC(`TF_ES_ASYNCDONTCARE`)면 시작/갱신이 분리된 WM_IME 메시지로
  나가 종료가 사라질 여지.
- 위험도: 중간. 단, ASYNC 전환은 인라인 입력 타이밍/순서 보장이 깨질 리스크가 커서 신중.

### 후보 #5 (낮음) — display-attribute VARIANT/atom 자체 결함
- 근거: composition.rs:26-67. SetValue 후 GetValue 재확인 로깅까지 있음(P1 진단).
- 메커니즘: atom 이 CUAS default store 에서 무효이면 거부될 수 있으나, 카테고리 등록(unim.wxs:122)과
  provider 구현(text_service.rs:582)이 있어 가능성 낮음. **로그(GetValue MISMATCH/SetValue FAILED)
  로 즉시 배제/확정 가능** → 먼저 로그부터 확인.

---

## 2. 실험적 수정 후보 (우선순위·리스크)

### 수정 A (최우선, 후보 #1 대응) — `ITfContextOwnerCompositionSink` 구현·등록
- 변경: text_service.rs:23-31 `#[implement(...)]` 에 `ITfContextOwnerCompositionSink` 추가하고
  `OnStartComposition`(pfOk=TRUE 반환), `OnUpdateComposition`, `OnEndComposition` 구현.
  StartComposition 직전 컨텍스트 source 에 `AdviseSink(ITfContextOwnerCompositionSink::IID, ...)`
  로 advise (ActivateEx 또는 컨텍스트 획득 시점). 현재 thread_mgr sink 만 advise
  (text_service.rs:209-214) 하므로 컨텍스트 단위 advise 추가 필요.
- 리스크: 낮음~중. sink advise 생명주기(컨텍스트 전환 시 unadvise) 관리 필요. 잘못 OnStartComposition
  에서 FALSE 반환하면 모든 앱에서 조합 불가 → 항상 TRUE 반환부터.
- 검증: wezterm 에서 즉시-종료 로그(text_service.rs:467) 가 사라지는지.

### 수정 B (후보 #2) — StartComposition 을 SetText 이전, collapsed range 에 먼저 건다
- 변경: composition.rs:368-389 순서를 `acquire_insert_range → Collapse(START) →
  StartComposition(빈 range) → GetRange → SetText → SetSelection` 로 재배치(SampleIME 순서).
- 리스크: 중. "거꾸로 입력"(녕안) 회귀 가능 → move_caret_to_end/select_composition_range 와
  상호작용 재검증 필요.

### 수정 C (후보 #3) — interim char 신호 부여 / 과도한 re-select 제거
- 변경: 미확정 마지막 음절에 대해 `fInterimChar = BOOL(1)` 시도, 또는 update 시 caret 만
  range-end 로 두고 전체 re-select 를 생략(composition.rs:434).
- 리스크: 중. GUI 앱(메모장)에서 선택 음영/거꾸로입력 회귀 가능 → A/B 와 분리해 단독 테스트.

### 수정 D (후보 #4, 보류 권장) — 시작/갱신 세션을 `TF_ES_ASYNCDONTCARE` 로
- 변경: composition.rs 의 RequestEditSession 플래그.
- 리스크: 높음. composition_slot take 패턴(composition.rs:239) 이 동기 반환 가정 →
  ASYNC 면 slot 이 비어 composition 생성 실패. take 타이밍 재설계 필요. **A~C 실패 시에만.**

### 수정 E (선결 진단) — 로그부터 확인
- `%TEMP%\unim-tsf.log` 에서 `set_composition_attribute: SetValue FAILED` /
  `GetValue MISMATCH` 유무 확인 → 있으면 후보 #5 확정, 없으면 #5 배제하고 #1 집중.

---

## 3. 핵심 질문 직답

1. **SYNC SetText 후 직후 조작이 CUAS 에 "종료/확정" 으로 해석될 소지** — 있음(후보 #4). 단,
   현재 더 유력한 결함은 SYNC 자체가 아니라 후보 #1(sink 부재).
2. **set_composition_attribute 가 거부/종료 유발?** — 가능성 낮음. 카테고리·provider 등록 완비.
   코드상 호출은 composition.rs:407-409 / 436-438 로 **if let Some(atom) 가드 아래** 있어
   `attr_atom = None` 으로 두면 깔끔히 분리·제거 가능(실험 용이). 먼저 로그로 SetValue 성공 확인.
3. **ITfContextOwnerCompositionSink vs ITfCompositionSink** — **ITfCompositionSink 만 구현,
   ContextOwnerCompositionSink 미구현(text_service.rs:23-31).** → CUAS 브리지가 요구하는 sink
   가 빠졌을 가능성이 가장 높음(후보 #1 = 최우선 수정 A).
4. **start_composition range/selection 의 CUAS 호환성** — 의심됨. 텍스트를 먼저 박은 range 를
   감싸는 순서(후보 #2) + TF_AE_NONE 전체 선택(후보 #3) 둘 다 CUAS 기대 모델과 어긋날 수 있음.
5. **ASYNC 전환 시 종료 소멸 여지** — 구조상 가능하나 composition_slot 동기 take(composition.rs:239)
   재설계 필수 → 리스크 높아 후순위(수정 D).

---

## 4. 권장 실행 순서

1. 수정 E(로그 확인)로 후보 #5 즉시 배제/확정.
2. 수정 A(`ITfContextOwnerCompositionSink` 구현·advise) — 최유력, 단독 적용 후 wezterm 재현.
3. 효과 없으면 수정 B(StartComposition 선행) 추가.
4. 그래도면 수정 C(interim/ re-select 정리).
5. 마지막으로 수정 D(ASYNC) — slot 재설계 동반.
