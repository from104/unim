# CUAS 브리지 메커니즘 / TSF TIP composition 즉시-종료 원인 규명

조사일: 2026-06-07. 대상: UNIM (windows-rs TSF TIP, in-proc COM cdylib)이
wezterm 등 IMM32-only(CUAS-unaware) 앱에서 StartComposition 직후(<200ms)
`OnCompositionTerminated`(UNIM 구현은 `ITfCompositionSink`)가 호출되어 조합이
끊기는 현상.

ctx_index source 라벨: `research-cuas-bridge`.

---

## 1. CUAS 브리지 메커니즘 (1차 소스 종합)

CUAS(Cicero Unaware Application Support, msctf.dll 내 default text store)는
`ITextStoreACP`를 구현하지 않는 IMM32-only 앱에 대해 msctf가 **자체 default
text store / context**를 제공하고, TSF TIP의 동작을 IMM32 메시지로 역변환한다.

- TIP는 `ITfContextComposition::StartComposition` → `ITfComposition` 획득 →
  composition range에 `ITfRange::SetText`로 미확정 문자열을 쓰고,
  `GUID_PROP_ATTRIBUTE` display attribute property를 SetValue 한다.
  (출처: MS Learn "Compositions", "Providing Display Attributes")
- CUAS는 이 default store의 상태를 폴링/관찰하여 IMM32 앱에:
  - composition 시작 → `WM_IME_STARTCOMPOSITION`
  - composition range 텍스트 + display attribute → `WM_IME_COMPOSITION`(`GCS_COMPSTR` + attribute 바이트)
  - composition 종료 + 남은 확정 텍스트 → `WM_IME_COMPOSITION`(`GCS_RESULTSTR`) + `WM_IME_ENDCOMPOSITION`
  로 변환한다.
- **핵심 판별 규칙**: CUAS는 "composition 안에 있고 display attribute가 붙은
  range"를 GCS_COMPSTR(미확정/밑줄)로, "composition 밖이거나 attribute가 없는
  확정 텍스트"를 GCS_RESULTSTR(확정)로 분류한다. attribute가 없으면 CUAS가
  range를 결과 문자열로 오인해 종료시킬 수 있다.

### wezterm 측 동작 (이미 조사됨: source `research-wezterm-ime`)

- `window/src/os/windows/window.rs` 단일 파일에 IME 로직.
- `WM_IME_STARTCOMPOSITION`을 **명시적으로 처리하지 않음**(DefWindowProc 경유).
  즉 wezterm은 조합 시작을 거부하지 않는다 → **종료의 능동 주체는 wezterm이 아님**.
- `WM_IME_COMPOSITION`에서 `GCS_COMPSTR`만 읽어 자체 렌더(`AdviseDeadKeyStatus(Composing)`),
  `GCS_RESULTSTR`은 `KeyCode::Composed`로 확정. `ImmNotifyIME` 미사용.
- lparam==0(빈 조합)을 받으면 조합 취소로 처리.
  → **즉시-종료는 CUAS 브리지 레벨에서 발생**하며, wezterm은 그 결과(빈 조합/
  종료 메시지)를 수동 반영할 뿐이다. MS 기본 한국어 IME는 같은 wezterm에서
  정상이므로 원인은 앱이 아니라 **UNIM이 CUAS default store와 상호작용하는 방식**.

---

## 2. 즉시 OnCompositionTerminated 원인 후보 (우선순위)

### [P1·최유력] 동기 edit session 안에서 StartComposition+SetText+(종료성 패턴)을 한 번에 처리

- MS Learn `ITfContext::RequestEditSession`: **`TF_ES_SYNC`는 "keystroke
  handling 등 문서화된 상황에서만" 쓰라**고 명시. 동기 세션에서 composition을
  열고 텍스트를 쓰고 selection을 collapse 하는 일련의 변경을, CUAS가 한 트랜잭션
  종료 시점에 "최종 확정"으로 스냅샷해 GCS_RESULTSTR + ENDCOMPOSITION으로
  내보낼 수 있다.
- UNIM 현황(`unim-tsf/.../composition.rs`): `start_composition`과
  `update_composition` 모두 `RequestEditSession(tid, ..., TF_ES_READWRITE | TF_ES_SYNC)`
  사용. (코드 확인됨, L232/L260)
- SampleIME(정상 동작 레퍼런스)는 StartComposition을 **별도 edit session**
  (`CStartCompositionEditSession`)으로 분리하고, key 처리(`CKeyHandlerEditSession`)와
  생애주기를 나눈다. 그리고 StartComposition 시 `InsertTextAtSelection(ec,
  TF_IAS_QUERYONLY, NULL, 0, &range)` 로 **빈 텍스트** range만 확보한 뒤
  `StartComposition(ec, range, sink, &comp)` 한다. 즉 "조합 열기"와 "문자 채우기"
  단계가 구조적으로 분리된다.

### [P2·유력] display attribute(GUID_PROP_ATTRIBUTE) 미적용/적용 실패

- attribute가 range에 안 붙으면 CUAS가 해당 range를 미확정으로 인식할 근거가
  없어 결과 문자열로 처리 → 즉시 종료. (UNIM 주석에 이미 가설로 기록됨)
- SampleIME는 composition 갱신마다 `_SetCompositionDisplayAttributes(ec, ctx,
  _gaDisplayAttributeInput)`로 매번 attribute property를 SetValue 한다.
  단순 등록(EnumDisplayAttributeInfo/RegisterCategory)만으론 부족하고 **range에
  실제 SetValue** 되어야 CUAS가 밑줄/미확정으로 본다.
- UNIM은 `set_composition_attribute`에서 VARIANT(VT_I4=TfGuidAtom)로 SetValue
  하나 HRESULT/직후 GetValue 진단 로깅 중 → 무음 실패(A) 여부 우선 검증 필요.

### [P3·가능] 빈/0-length composition range로 StartComposition

- 한글 첫 자모 입력 시 아직 화면 글자가 없는 상태에서 0-length range로
  composition을 열고 곧장 SetText 하면, CUAS가 "내용 없는 조합 → 즉시 닫힘"으로
  처리할 여지. SampleIME는 QUERYONLY로 range를 얻되 selection을 그 range로
  재설정(`SetSelection`)해 anchor를 안정화한다.

### [P4·낮음] 앱/CUAS가 ImmNotifyIME(CPS_COMPLETE/CANCEL) 발신

- wezterm은 `ImmNotifyIME` 미사용으로 확인됨. CUAS 내부가 보낼 수는 있으나,
  그 트리거 자체가 P1/P2의 결과이므로 1차 원인이 아니라 증상.

### [P5·낮음] ITfCompositionSink vs ITfContextOwnerCompositionSink 혼동

- `ITfContextOwnerCompositionSink`는 **앱(컨텍스트 소유자)** 이 구현하는 sink.
  TIP은 `ITfCompositionSink`(OnCompositionTerminated)만 구현하면 됨. UNIM이
  올바르게 `ITfCompositionSink`를 구현 중이므로 인터페이스 선택 자체는 정상.
  단 OnCompositionTerminated가 와도 거기서 곧장 EndComposition만 하지 말고
  재시작 가드를 둘지 검토(SampleIME는 종료 콜백에서 정리만 함).

---

## 3. MS IME(정상 TSF TIP)와 커스텀 TIP의 차이 (해야/하지 말아야 할 것)

해야 할 것:
- StartComposition을 키 처리와 **분리된 (또는 명확히 단계화된)** edit session으로
  열고, 조합 동안 range를 유지한 채 SetText만 갱신한다.
- composition range에 매 갱신마다 `GUID_PROP_ATTRIBUTE` SetValue로 미확정 표시.
- StartComposition 시 `InsertTextAtSelection(TF_IAS_QUERYONLY, NULL, 0)`로 빈
  range 확보 후 selection을 그 range로 SetSelection.

하지 말아야 할 것:
- 한 동기 트랜잭션 안에서 열기→채우기→selection collapse를 결과 확정처럼 마무리.
- attribute 없이 조합 텍스트만 SetText.
- 매 자모마다 composition을 새로 열고 닫기(생애주기 churn).

---

## 4. "composition을 비동기/지속 유지" 권고 근거

- `RequestEditSession` 문서: `TF_ES_SYNC`는 keystroke 등 제한 상황 전용,
  일반적으론 `TF_ES_ASYNCDONTCARE`(매니저가 동기/비동기 선택)를 권장.
- composition 자체는 edit session보다 **오래 살아야 하는 상태**다. 동기 세션이
  끝날 때마다 composition을 닫는 패턴이면 CUAS가 매번 ENDCOMPOSITION을 본다.
  → composition 객체(`ITfComposition`)는 세션 밖에서 보존하고, 세션은 range
  편집에만 쓰며 EndComposition은 사용자가 확정/취소할 때만 호출.

---

## 5. 콘솔/터미널 특이사항

- wezterm은 커스텀 HWND 기반(콘솔 호스트 아님)이며 `ISC_SHOWUICOMPOSITIONWINDOW`를
  꺼서 시스템 조합창을 억제, GCS_COMPSTR을 직접 렌더한다. 따라서 표준 IMM32
  경로를 그대로 타며 콘솔 특수 경로는 아니다.
- 진짜 콘솔(conhost) 계열은 별도지만 본 사례엔 해당 없음. CUAS 표준 브리지로 분석.

---

## 6. UNIM이 바꿔야 할 구체 지점 (가설, 우선순위 순)

대상 파일: `unim-tsf/src/composition.rs`, `unim-tsf/src/text_service.rs`,
`unim-tsf/src/display_attr.rs`.

1. **(P1) composition 생애주기와 edit session 분리**: StartComposition을
   조합 시작 시 1회만 수행하고, 이후 자모 입력은 동일 composition range의
   SetText 갱신으로 처리. 매 키마다 start/end 반복하지 않도록 `composition`
   slot 유지 로직 점검(이미 slot 보유 → 실제로 매 키 재시작 여부 로그 검증).
2. **(P2) display attribute SetValue 성공 검증**: `set_composition_attribute`의
   HRESULT + 직후 GetValue를 dbg_log로 분리 진단(무음 실패 A vs 종료 B). 실패면
   VARIANT(VT_I4 = TfGuidAtom) 구성/atom 등록 경로부터 수정.
3. **(P1보강) StartComposition을 SampleIME 패턴으로**: `InsertTextAtSelection(
   TF_IAS_QUERYONLY, NULL, 0, &range)` → `StartComposition(ec, range, sink)` →
   `SetSelection(range)`. 빈 range로 안정적으로 연 뒤 SetText.
4. **(P3) update_composition을 `TF_ES_READWRITE | TF_ES_ASYNCDONTCARE`로** 전환
   실험(키 핸들러 외 경로). start는 keystroke 컨텍스트이므로 SYNC 유지 가능.
5. **(P5) OnCompositionTerminated에서 즉시 EndComposition만 하지 말고** 원인
   로깅 + (필요시) 사용자 입력이 진행 중이면 재시작 가드.

### 검증 실험 설계
- A/B: attribute SetValue를 끈 빌드 vs 켠 빌드에서 wezterm 즉시-종료 재현 여부
  비교 → P2 확정/배제.
- 로그: start_composition / update / OnCompositionTerminated 타임스탬프로
  "매 자모 재시작" 여부와 종료 트리거 시점 확정 → P1 vs P3 분리.

---

## 출처
- MS Learn: Compositions (`/windows/win32/tsf/compositions`)
- MS Learn: Providing Display Attributes (`/windows/win32/tsf/providing-display-attributes`)
- MS Learn: ITfContext::RequestEditSession (TF_ES_SYNC/ASYNCDONTCARE 설명)
- MS Learn: ITfContextOwnerCompositionSink / OnEndComposition
- microsoft/Windows-classic-samples SampleIME: StartComposition.cpp,
  Composition.cpp(_AddComposingAndChar/_SetInputString/_SetCompositionDisplayAttributes),
  KeyHandlerEditSession.cpp, OnCompositionTerminated
- chromium ui/base/ime/win/tsf_text_store.cc (TSF↔store 동작 참조)
- 사내 선행 조사: source `research-wezterm-ime` (wezterm WndProc IME 경로)
- UNIM 현행 코드: unim-tsf/src/composition.rs (RequestEditSession TF_ES_SYNC 확인)
