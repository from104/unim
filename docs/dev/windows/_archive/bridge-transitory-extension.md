# Bridge 조사: TSF Transitory Extension — wezterm inline preedit 가능성

조사일: 2026-06-07. 각도: **transitory-extension**.
대상 결론(적대적 재검증): "순수 TSF TIP는 text store 없는 IMM32-only 앱(wezterm)에서
inline composition 불가". 사용자 제보 "TSF-IMM32 브릿지가 있다"의 후보로
Transitory Extension(GUID_COMPARTMENT_TRANSITORYEXTENSION, ITfTransitoryExtensionSink,
transitory document manager)을 1차 소스로 검증.

ctx_index source 라벨: `bridge-transitory-extension`.

---

## 결론 (한 줄)

**Transitory Extension은 우리가 찾는 브릿지가 아니다.** 이것은 *애플리케이션(문서 소유자)*
측 메커니즘이며, TIP이 켜거나 활용할 수 있는 경로가 아니다. CUAS가 IMM32 앱에
이 확장을 자동 적용한다는 1차 근거는 없다. wezterm은 이 sink를 구현하지 않으므로
설령 CUAS가 transitory document를 만들어도 wezterm에 inline으로 전달되지 않는다.
**inline_in_wezterm = no.**

---

## 1. Transitory Extension이 실제로 무엇인가 (1차 소스)

### 메커니즘 (방향성이 핵심)
- `GUID_COMPARTMENT_TRANSITORYEXTENSION` 컴파트먼트는 **document manager object에
  한정**된다(VT_I4: NONE / FLOATING / ATSELECTION). 즉 이 값을 세팅하는 주체는
  **문서를 소유한 쪽(앱) 또는 그 문서를 만든 코드**다. context도 thread mgr도 아니다.
  (출처: MS Learn "Predefined Compartments", Ctffunc.h)
- 값 의미: FLOATING = floating UI로 transitory 확장 시작, ATSELECTION = 부모 document
  manager의 selection/IP 위치에 popup UI로 시작, NONE = 중지.
  (출처: MS Learn "Values for GUID_COMPARTMENT_TRANSITORYEXTENSION", Ctffunc.h)
- 확장을 켜면 TSF 매니저가 **자식 "transitory document manager"**를 생성한다. 부모/자식은
  `GUID_COMPARTMENT_TRANSITORYEXTENSION_DOCUMENTMANAGER`(부모→자식 transitory doc),
  `GUID_COMPARTMENT_TRANSITORYEXTENSION_PARENT`(자식→부모)로 서로 가리킨다.
  (출처: MS Learn "Predefined Compartments")

### 누가 sink를 받는가 (= 브릿지 불가의 결정적 근거)
- `ITfTransitoryExtensionSink`는 **"the application that uses Transitory Extension"이
  구현**한다고 명시. "The application can track the changes ... by using this sink."
  → sink의 수신자는 **앱**이지 TIP이 아니다.
  (출처: MS Learn `nn-msctf-itftransitoryextensionsink`)
- `OnTransitoryExtensionUpdated(pic, ecReadOnly, pResultRange, pCompositionRange,
  pfDeleteResultRange)`: **앱**이 result range/composition range를 읽고, result range
  삭제 여부를 돌려준다. 즉 이 콜백은 **앱이 transitory document의 조합/확정 결과를
  자기 본문에 반영**하기 위한 통로다. TIP이 inline을 "얻는" 통로가 아니라, 앱이
  text store 없이도 조합 결과를 받는 *앱측 편의*다.
  (출처: MS Learn `nf-...-ontransitoryextensionupdated`)

### 용도(설계 의도)
- floating/popup UI를 가진 "transitory" 입력 시나리오(예: 검색 상자류, 임시 입력
  필드)에서, 풀 ITextStoreACP를 구현하지 않고도 조합을 처리하기 위한 *앱측* 단축경로.
  핵심은 여전히 **앱이 sink를 advise**해야 동작한다는 점.

---

## 2. wezterm/CUAS에 적용했을 때 (적대적 검증)

1. **TIP은 transitory extension을 켤 위치에 없다.** 컴파트먼트는 document manager
   소유. wezterm의 document manager는 CUAS가 만든 것이고, 그 컴파트먼트를 FLOATING/
   ATSELECTION으로 세팅하는 것은 앱/매니저 측 행위다. TIP(UNIM)이 이걸 강제로 켜서
   inline을 끌어낼 수 있다는 1차 근거 없음.
2. **wezterm은 `ITfTransitoryExtensionSink`를 구현/advise 하지 않는다.** wezterm은
   순수 IMM32 경로(`ImmGetCompositionStringW(GCS_COMPSTR)`)만 처리한다(선행 조사
   `research-wezterm-ime` 확정). transitory document가 생겨도 그 업데이트를 받을
   앱측 sink가 없으므로 wezterm 화면에 아무것도 안 그려진다.
3. **CUAS가 IMM32 앱에 transitory extension을 자동 적용한다는 근거 없음.** MS Learn
   어디에도 "CUAS default store가 transitory extension을 켠다"는 서술이 없다. CUAS의
   문서화된 동작은 "TSF 요청을 IMM API로 역변환"(Edge TSF1 explainer: *"Windows
   provides a component to convert all TSF requests into IMM APIs"*)이며, 이는
   GCS_COMPSTR/GCS_RESULTSTR 브릿지(선행 조사 `research-cuas-bridge`)이지 transitory
   document 경로가 아니다.
4. **실제 IME/브라우저 어디서도 TIP측 transitory 사용례를 못 찾음.**
   - Firefox `TSFTextStore.cpp`(168KB): 풀 ITextStoreACP 구현. Transitory 미사용.
   - Chromium: `tsf_text_store.cc`(풀 store) + `imm32_manager.cc`/`tsf_bridge.cc`로
     IMM32↔TSF를 다루지만 transitory extension 경로 아님(Edge explainer는 Chromium에
     **풀 TSF 1.0 text store 채택**을 제안 — transitory가 아니라 정공법).
   - Wine `dlls/msctf/context.c`: transitory extension 미구현(해당 sink/compartment
     처리 부재) — 생태계에서 거의 안 쓰이는 obscure 기능임을 방증.
   - GitHub code search: 로그인 요구로 직접 확인 불가(부정 증거 아님).

---

## 3. 왜 이게 "브릿지처럼 보였나" (오해 해소)

Transitory Extension은 이름과 "text store 없이 composition"이라는 표면 때문에
브릿지로 오인되기 쉽다. 그러나 실제로는:
- **앱이 opt-in**(sink advise + 컴파트먼트 인지)해야 하는 *앱측* 기능,
- 방향이 **TSF→앱**(앱이 결과를 받음)이지 **TIP→레거시앱**(TIP이 inline을 그림)이 아님,
- CUAS의 IMM32 역변환 경로와 **무관**.

따라서 "TSF-IMM32 브릿지"의 정답 후보는 이것이 아니라, 별도 각도(CUAS GCS_COMPSTR
브릿지 정상화, 또는 IMM32 .ime/TIP 하이브리드)에서 찾아야 한다.

---

## 4. UNIM 적용 권고

- **transitory extension 경로는 추진하지 말 것.** TIP이 켤 수 없고, wezterm이 sink를
  구현하지 않아 이중으로 막힘.
- inline의 현실적 경로는 (a) CUAS GCS_COMPSTR 브릿지 정상화(선행 조사
  `research-cuas-bridge`의 P1 `ITfContextOwnerCompositionSink` 등) 또는
  (b) client-side overlay(`preedit_window.rs`) 폴백. transitory는 둘 다에 기여 못 함.

---

## 출처 (1차)
- MS Learn: ITfTransitoryExtensionSink interface (msctf.h) — "implemented by the
  application that uses Transitory Extension"
  (`/windows/win32/api/msctf/nn-msctf-itftransitoryextensionsink`)
- MS Learn: ITfTransitoryExtensionSink::OnTransitoryExtensionUpdated — 시그니처/방향
  (`/windows/win32/api/msctf/nf-msctf-itftransitoryextensionsink-ontransitoryextensionupdated`)
- MS Learn: Predefined Compartments (Ctffunc.h) — GUID_COMPARTMENT_TRANSITORYEXTENSION*
  는 document manager object 한정
  (`/windows/win32/tsf/predefined-compartments`)
- MS Learn: Values for GUID_COMPARTMENT_TRANSITORYEXTENSION — NONE/FLOATING/ATSELECTION
  (`/windows/win32/tsf/values-for-guid-compartment-transitoryextension`)
- MicrosoftEdge/MSEdgeExplainers TSF1/explainer.md — CUAS = "convert all TSF requests
  into IMM APIs"; Chromium은 풀 TSF text store 채택 제안(transitory 아님)
- mozilla gecko-dev widget/windows/TSFTextStore.cpp — 풀 ITextStoreACP, transitory 미사용
- wine dlls/msctf/context.c — transitory extension 미구현
- 선행 사내 조사: `research-wezterm-ime`, `research-cuas-bridge`
