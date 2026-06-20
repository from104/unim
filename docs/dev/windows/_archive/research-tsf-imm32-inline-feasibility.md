# TSF TIP → 레거시/콘솔 앱 inline preedit 가능성 조사

조사일: 2026-06-07 · ctx_index source 라벨: `research-tsf-imm32-inline`
대상: wezterm(순수 IMM32, ITextStoreACP 미구현, CUAS-unaware)에서 UNIM(in-proc TSF TIP) 한글 inline preedit 가능 여부 판정.

---

## 요약 판정

| 질문 | 결론 |
|---|---|
| (a) Mozc/Weasel이 wezterm·콘솔에서 inline인가? | **아니다. overlay(별도 후보/조합 창)가 업계 표준이다.** Weasel은 콘솔(conhost.exe)에서 아예 입력 자체를 비활성(ascii_mode)으로 출고한다. |
| (b) 순수 TSF TIP으로 레거시 IMM32 앱 inline 가능? | **조건부 불가.** 앱이 ITextStoreACP(TSF 문서)를 구현하지 않으면 CUAS는 inline composition을 지원하지 못하고 즉시 종료시킨다. committed text만 전달된다. |
| (c) 가능하다면 방법/난이도 | TSF 단독으로는 불가. IMM32 우회(.ime 또는 ImmSetCompositionString)만이 유일 경로이며, 정상 경로(in-proc TIP)에선 허용/안정성 모두 불확실 → 고난도·고위험. |
| (d) 권고 | **IMM32 inline 구현 추진 금지. UNIM 오버레이 창 방식을 정식 채택.** 업계 표준과 일치. |

---

## 1. 왜 CUAS가 빈 composition조차 즉시 종료하는가 (질문 2 확증)

핵심 원리: **CUAS(Cicero Unaware Application Support)는 TSF 요청을 IMM32로 변환하는 호환 레이어이며, 변환의 출발점은 앱의 TSF 문서(ITextStoreACP / Document Manager)다.**

- TSF에서 composition 문자열·결과는 `ITextStoreACP::SetText`로 앱의 텍스트 스토어에 기록되어야 한다 (MS Learn: WM_IME_COMPOSITION 처리 문서, `research-tsf-imm32-inline`).
- wezterm은 ITextStoreACP를 구현하지 않는다(순수 IMM32, CUAS-unaware). 따라서 CUAS가 TIP의 composition을 받아 적을 "문서"가 없다. 빈 composition을 시작하는 순간 CUAS는 유지할 수 있는 컨텍스트가 없으므로 `OnCompositionTerminated`로 즉시 끊는다.
- 결과적으로 CUAS는 이런 앱에 대해 **committed text(GCS_RESULTSTR)만** WM_IME_COMPOSITION으로 브리지하고, 진행 중 inline 조합(GCS_COMPSTR)은 전달하지 못한다. → MS SampleIME의 2-phase 빈-조합-먼저 패턴을 정확히 복제해도 종료되는 실측과 정확히 일치.

증거: MS Learn "Processing the WM_IME_COMPOSITION Message", "Japanese IME / TSF" (모두 `research-tsf-imm32-inline`에 인덱싱). WM_IME_SETCONTEXT/COMPOSITION은 imm32.dll API이지 msctf.dll(TSF) API가 아니며, TSF↔IMM32 이벤트는 자동 복제되지 않는다.

## 2. 성숙한 서드파티 TSF IME의 실제 동작 (질문 1 확증 — 가장 중요)

### Weasel / Rime (librime, Windows)
- **Weasel은 콘솔에서 inline을 시도조차 하지 않는다.** 기본 출고 `weasel.yaml`이 `cmd.exe`, **`conhost.exe`에 대해 `ascii_mode: true`** 를 박아 영어 입력으로 강제한다.
  증거: rime/weasel 커밋 `28cdd09` "feat(weasel.yaml): enable ascii_mode in console applications by default" (`research-tsf-imm32-inline`).
- Weasel은 `inline_preedit` 옵션을 노출하지만, 이는 **GUI 앱(ITextStoreACP 구현)에서만** 의미가 있고, 끄면 자체 후보 창(overlay)으로 폴백한다. Weasel 문서(Customization)는 `inline_preedit: false`일 때 TSF 커서 위치 보정으로 깜빡임을 줄인다고 명시 — 즉 inline 불가 환경에서는 overlay가 기본.
- → **Rime/Weasel = 콘솔에서 overlay(또는 비활성). inline 아님.**

### Mozc / Google 일본어 입력
- Windows 콘솔에서의 inline 사례 증거는 발견되지 않음. wezterm Mozc 관련 이슈(#7301 등)는 Linux(fcitx5) 환경 입력 실패 문제이며, Windows 콘솔 inline 성공 사례 아님.

### MS 기본 IME (대조군)
- 사용자 실측처럼 wezterm에서 inline이 되는 이유는 **MS IME가 TSF TIP인 동시에 IMM32 네이티브 .ime 경로(imekr*.ime / imjp*.ime)를 가진 하이브리드**이기 때문(질문 3). 레거시/콘솔 앱에서는 CUAS를 거치지 않고 IMM32 직접 경로로 WM_IME_COMPOSITION(GCS_COMPSTR)을 보내 inline을 그린다. 이는 OS 동봉 IME만 가진 특권적 경로이며 서드파티 TIP에는 같은 보장이 없다.

## 3. 순수 TSF TIP으로 레거시 inline 가능 여부 (질문 4)

- **TSF API 단독으로는 불가.** CUAS 브리지는 ITextStoreACP 없는 앱에 inline을 전달하지 못한다(§1).
- **IMM32 우회 시도**: in-proc TIP이 포커스 창의 HIMC에 `ImmGetContext` → `ImmSetCompositionStringW(GCS_COMPSTR)`로 직접 조합을 주입하는 것은 이론상 가능하나:
  - TIP 활성 시 IMC는 CUAS가 점유/관리하므로 충돌·재진입·강제 종료 위험.
  - 앱이 WM_IME_COMPOSITION 핸들러를 가져야 그려진다(wezterm은 `ime_preedit_rendering = "System"`/builtin 경로가 있어 일부 가능성은 있으나 GCS_COMPSTR 처리 여부 불확실).
  - MS가 권장/문서화한 경로가 아님 → 버전 간 깨질 위험.
- 진정한 양쪽 지원은 **별도 IMM32 IME(.ime) 등록 = TSF+IMM32 하이브리드** 구현이 정공법이며, 이는 사실상 두 번째 입력기를 만드는 수준의 작업.

## 4. 권고

1. **IMM32 inline 구현(.ime 하이브리드 또는 ImmSetCompositionString 주입) 추진하지 말 것.** 고난도·고위험·미지원 경로이고, 업계 1급 IME(Weasel/Rime)조차 콘솔에서 포기하고 overlay/비활성으로 출고한다.
2. **UNIM 오버레이 창 방식을 정식 아키텍처로 채택.** 조합은 UNIM 자체 오버레이로 그리고, 확정 문자열만 앱에 삽입(메모리노트 `unim-tsf-terminal-preedit-architecture` 방침과 일치). 이것이 업계 표준 답.
3. 콘솔/CUAS-unaware 앱 감지 시(ITextStoreACP 미구현 신호 = composition 즉시 OnCompositionTerminated) 자동으로 오버레이 폴백으로 전환하는 로직을 두면, Weasel의 app_options(conhost ascii_mode)와 동등한 사용자 경험 확보.
4. (선택) MS IME만 inline 되는 것은 OS 특권 .ime 경로 때문임을 사용자 문서/FAQ에 명시해 "왜 UNIM은 안 되냐"는 혼란 차단.

---

## 출처 (모두 ctx 인덱싱: `research-tsf-imm32-inline`)
- rime/weasel commit 28cdd09 — conhost ascii_mode 기본 출고: https://github.com/rime/weasel/commit/28cdd09692f77e471784bf85ff7a19bc48e113f4
- Weasel 정제화 문서(inline_preedit / app_options): https://hantang.github.io/rime-docs/en/windows/weasel/Customization/
- MS Learn — Processing the WM_IME_COMPOSITION Message (ITextStoreACP::SetText, GCS_COMPSTR/RESULTSTR): https://learn.microsoft.com/en-us/windows/win32/intl/processing-the-wm-ime-composition-message
- MS Learn — Japanese IME (후보창 overlay 모델, MS IME 동작): https://learn.microsoft.com/en-us/globalization/input/japanese-ime
- wezterm #3411 on-the-spot IME, discussion #2929 (preedit 미지원/취약): https://github.com/wezterm/wezterm/issues/3411
- 배경: CUAS = TSF↔IMM32 변환 레이어, 이벤트 비복제 (TSF 소개 문서들, ImeStudy)
