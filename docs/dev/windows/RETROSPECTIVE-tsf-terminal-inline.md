# 회고: CUAS-unaware 터미널(wezterm) 한글 inline 조합

작성일 2026-06-07 · 브랜치 feat/windows-msi-redesign · 정직한 사후 검토. 자화자찬·변명 금지.

---

## 1) 한 줄 결론

정답은 **`TF_SELECTIONSTYLE.fInterimChar = TRUE`(조합 중 selection style)** 였고, 그 출처는 MS 공식 문서 분석이 **아니라** 작동하는 오픈소스 한국어 TSF IME 3종(NavilIME·saenaru·kolemak) 소스 직독 + 실측 로그(`%TEMP%\unim-tsf.log`) + 사용자의 거듭된 교정이었다.

---

## 2) 무엇이 정답이었나 — 동작 메커니즘 5요소

`fInterimChar`를 중심으로:

1. **`fInterimChar = TRUE`** — 조합 중 selection을 interim character로 표기. **이것이 핵심 트리거.** CUAS가 이 신호로 한국어 조합을 inline으로 브리지한다. (`composition.rs:157` `fInterimChar: BOOL(1)`)
2. **non-empty range** — 빈 range의 composition은 즉시 terminate된다. range가 비어있지 않아야 생존.
3. **단일 edit session** StartComposition → SetText (세션을 쪼개지 않음).
4. **음절 전환 end+start의 단일 세션 병합** — `commit_and_restart`(`composition.rs:361`)로 연속 조합 보장.
5. **Enter/화살표 nav 통과** — `OnTestKeyDown`에서 확정 후 `pIsEaten=FALSE`로 앱에 통과(NavilIME 패턴). + non-sticky 폴백(종료돼도 다음 단어 inline 복구).

---

## 3) 무엇을 왜 틀렸나 (변명 없이)

**① "터미널은 TSF 조합 불가 → 오버레이가 유일답"(반복).**
근본 원인: impossible 과대일반화. "빈 composition이 즉시 terminate된다"는 실측을 "조합 자체가 불가"로 비약. 실제로 죽은 건 *빈 range*였고 non-empty면 살았다. 실측 신호를 좁게 읽지 않고 결론으로 점프했다. 틀렸다.

**② "ITfContextOwnerCompositionSink 미구현이 P1 근본원인"(analyst 최유력).**
반증: 그 인터페이스는 **앱(CUAS)측 sink**라 TIP과 무관. SampleIME·Mozc·UNIM 셋 다 `ITfCompositionSink`만 구현 — 미구현은 MS IME와의 차이가 아니다(`bridge-tsf-missing-interfaces.md`). 서브에이전트의 인터페이스 diff 추론을 작동 소스 대조라는 1차검증보다 먼저 신뢰한 오진. 틀렸다.

**③ "MS 한국어 IME = .ime IMM32 하이브리드 특권 경로".**
반증: 이 PC 레지스트리에 `.ime` 0개, MS imekr도 순수 TSF TIP이었다(`bridge-hybrid-imm-registration.md`). "MS만 특권"이라는 편한 서사를 레지스트리 확인 전에 신봉. 틀렸다.

**④ "순수 TSF로 레거시 inline은 원천 불가 → Mozc/Weasel도 콘솔 overlay = 업계표준이니 수용".**
가장 해로운 과대결론. Mozc/Weasel이 콘솔에서 overlay/비활성인 것은 "그들이 안 한 것"이지 "할 수 없는 것"이 아니다 — **부재를 불가능으로 등치**. `feasibility.md`는 "추진하지 말 것"까지 권고했고, 사용자가 밀어붙여 실제로 가능했다. 틀렸다.

**⑤ "GUID_PROP_READING이 CUAS 브리지 핵심".**
반증: UNIM만 set, 어떤 한국어 TSF도 안 씀(`research-korean-tsf-imes.md` L50). 그럴듯한 메커니즘 가설을 A/B 없이 코드에 박았고, 적용해도 안 됐으며 오히려 제거 대상이 됐다. 틀렸다.

---

## 4) "TSF 문서를 잘 분석한 게 맞나?" — 1차 근거에 기반한 정직한 답

**아니다. 우리 TSF 1차 문서 분석은 실패했다.**

- `fInterimChar`의 정답은 **MS Learn `TF_SELECTIONSTYLE`(msctf.h) Remarks(2021-08-03 갱신)에 평문으로, 한국어를 지목해 명시돼 있었다**: *"interim character ... solid rectangle ... a standard UI element of Korean and some Chinese character compositions ... fInterimChar is an indication that a specific character is composed."* 즉 **fInterimChar는 MS 문서에 있었다.**
- 그런데 우리는 그 구조체를 자칭 "TSF 공식 레퍼런스"(`TSF_OFFICIAL_REFERENCE.md`, MS URL 9개 인용)에서조차 **통째로 누락**했고(`selectionstyle/interim` 0건), 초기 계획에선 정답 값을 `fInterimChar: BOOL(0)`으로 하드코딩해 **봉인**했다.
- 더 나쁜 건, 정작 "순수 TSF 불가"라는 과대결론은 **1차 문서 근거 없이** 2차자료(Weasel conhost 커밋)·일본어 overlay 모델·CUAS 추론으로 단언했다는 점이다. 문서 분석은 정답에 기여하지 못했고 오히려 ②④⑤ 오진의 토양이었다.
- URL 수집량(imm32-tsf 17개, reference 9개)은 "문서를 봤다"는 알리바이는 됐지만 **폭은 넓고 정확히 빗나갔다**. 정답 문서는 같은 디렉터리·같은 시기에 공존했는데 종합에 실패했다.

---

## 5) 정답을 실제로 찾게 한 것 + 더 빨리 찾는 길

**찾게 한 것 (정직히):**
- **작동 오픈소스 3종 소스 직독** — NavilIME `EditSession.cpp` L133 / saenaru `compose.cpp:227` / kolemak `SetInterimSelection`. 3개 독립 저장소 공통 패턴으로 `fInterimChar=TRUE`가 즉시 떠올랐다.
- **실측 로그** — `%TEMP%\unim-tsf.log`의 `OnCompositionTerminated:IMMEDIATE`가 lifecycle 문제를 좁혔다.
- **사용자 교정 4건** — "MS IME는 wezterm에서 inline 된다"(④ 반증 트리거), "TSF-IMM32 브리지 있다더라", "github에 오픈소스 한국어 TSF 2개"(korean-tsf 조사 착수의 직접 계기 → fInterimChar 발견), "연속 조합 되게"(commit_and_restart 요구). **방향 전환 4건이 전부 사용자발이다. 이 건의 공은 모델보다 사용자에게 있다.**

**더 빨리 찾는 길:**
- (a) 1일차에 작동 오픈소스 한국어 TSF 소스부터 읽었어야 했다. `korean-tsf-imes.md`는 6/7에야 작성됐고 작성되자마자 정답이 나왔다 — 같은 조사를 5/30에 했다면 ①④를 처음부터 회피.
- (b) 사용자의 "MS IME는 된다" 단언 즉시 레지스트리/동작을 1차 확인 → ③④ 동시 붕괴.
- (c) "빈 조합도 죽는다"를 "불가"가 아니라 "빈 range가 문제"로 좁게 신뢰 → ①에서 바로 non-empty range.
- 추정: **약 6~9회의 빌드-테스트 사이클이 틀린 가설에 낭비**됐다. 정답 자체(fInterimChar 한 줄 + 단일세션 병합)는 마지막 1~2 사이클에 끝났다.

---

## 6) 교훈 (향후 TSF류 난제 접근 원칙)

1. **"불가능"은 금지어** — "X가 안 됨"을 "X 부류가 불가"로 승격하지 말 것. 부재 ≠ 불가능.
2. **비대칭 우선순위** — 작동하는 reference 구현 1개 > 공식 문서 10페이지 > 인터페이스 diff 추론. 난제는 spec이 아니라 reference로 푼다.
3. **사용자가 "된다더라"고 반례를 들면 그건 가설이 아니라 검증 명령** — 즉시 1차 데이터(레지스트리/실측/소스)로 확인.
4. **서브에이전트의 "최유력" 결론은 1차검증 게이트를 통과해야 코드에 반영** (②의 교훈).
5. **메커니즘 가설은 A/B 제거 테스트 전엔 코드에 박지 말 것** (⑤의 교훈).
