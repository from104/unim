# 조사 B — Windows 11 third-party TSF IME 입력 표시기 깨짐: 알려진 이슈/최신 정보

조사일: 2026-05-30. 근거 없는 추측은 "확인 필요"로 명시. 인덱싱 source 라벨: `research-b-*`.

---

## 핵심 판정 (요약)

**third-party langbar 입력 표시기(가/A 모드 아이콘 + 브랜딩 아이콘)가 Win11 작업표시줄 Input Indicator에 뜨는 것은 → (b) 제약됨 + 부분적으로 (c) 알려진 버그.**

- **설계상 제약(b)**: Microsoft 공식 문서가 명시적으로, Input Indicator는 **"compatible IME"에 대해서만** 브랜딩/모드 아이콘을 시스템 트레이에 표시하고, **비호환 IME는 아이콘 대신 언어 약어(ENG/KOR 등)만** 표시한다고 규정한다. 이건 OS 버그가 아니라 Win8부터 이어진 **의도된 동작 + 요구사항**이다. third-party IME가 트레이에 모드 아이콘을 띄우려면 TSF의 정해진 방식(langbar item을 Input Indicator에 연동)으로 구현하고 IME 플래그를 올바로 set 해야 한다.
- **회귀/버그(c)**: 별개로, Win11 24H2/25H2에 Input Indicator의 위치 계산/systray 재등록 관련 UI 회귀가 보고됨(아래 질문 1·4). third-party langbar 아이콘이 업데이트 후 트레이에서 사라지는 사례도 실제 보고됨(Weasel #1682).

> 결론: UNIM의 langbar item이 Win11 트레이에 안 뜨는 것은 "OS가 third-party를 막아서"가 아니라, **(1) Input Indicator가 요구하는 호환 방식으로 langbar/mode 아이콘을 연동하지 못했을 가능성이 높고**, (2) 거기에 더해 Win11 신규 작업표시줄/Input Indicator의 알려진 표시 회귀가 겹칠 수 있음. UNIM 코드의 langbar 구현이 MS 요구사항을 충족하는지 확인 필요.

---

## 질문 1 — Win11에서 third-party TSF IME의 langbar/입력모드 인디케이터가 작업표시줄에 안 뜨는 알려진 이슈?

**결론: 있다 (설계 제약 + 알려진 회귀 둘 다).**

근거:
- MS 공식: "The Input Indicator shows the IME branding icon and mode icon **only for compatible IMEs**. IMEs that aren't compatible don't have the branding icon and mode icon displayed in the system tray. Instead, the Input Indicator shows the **language abbreviation** instead of the IME branding icon."
  - 출처: Microsoft Learn, *Custom Input Method Editor (IME) requirements* — https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements (source: `research-b-ms-ime-requirements`)
  - 동일 문장이 Win8 시절 문서에도 존재: *Third-party input method editors* (w8cookbook) — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/w8cookbook/third-party-input-method-editors.md (source: `research-b-w8cookbook-thirdparty-ime`)
- 알려진 UI 회귀: "There is a coordinate calculation bug in the Windows Shell/Taskbar logic where the Input Indicator fails to anchor correctly to the systray. This UI regression exists on 24H2 and is now persistent on every single boot in 25H2."
  - 출처: Microsoft Q&A, *Input Indicator UI bug: Language switcher incorrectly positioned… (Build 25H2)* — https://learn.microsoft.com/en-au/answers/questions/5779777/input-indicator-ui-bug-language-switcher-incorrect

## 질문 2 — Win11 새 작업표시줄이 legacy language bar item(SHOWNINTRAY)을 더 이상 트레이에 안 그리는가?

**결론: 부분적으로 맞다 / 정밀 확인 필요.** Win11은 입력 표시를 OS 통합 **Input Indicator**로 일원화했고, legacy floating/docked Language Bar는 기본 비활성. legacy langbar는 "Use the desktop language bar when it's available"(고급 키보드 설정) 옵션을 켜야 부활하며, `HKCU\Software\Microsoft\CTF\LangBar`의 `ShowStatus`(4=taskbar dock) 레지스트리로 제어된다.

- legacy langbar 부활 옵션·레지스트리 근거: WebSearch 결과(elevenforum / thewindowsclub / top-password 등). `Set-WinLanguageBarOption -UseLegacyLanguageBar`, `ShowStatus` DWORD 설명 확인.
- 다만 "Win11이 SHOWNINTRAY 플래그를 가진 third-party `ITfLangBarItem`을 트레이에 그리지 않도록 명시적으로 막는다"는 **MS 공식 문구는 못 찾음 → 확인 필요.** 확실한 것은: MS는 third-party가 트레이에 모드 아이콘을 띄우려면 **Input Indicator 연동 방식으로 바꿔야** 한다고 요구한다("If an IME relies on the language bar to show its mode icons in Windows 7, the IME must be changed in order to show its branding icon and mode icon in the input indicator"). 즉 Win7식 langbar-only 구현은 Win8+에서 트레이 노출이 보장되지 않는다.

## 질문 3 — MS 자사 IME만 트레이에 입력 모드를 표시하고 third-party는 제외하는 동작이 보고됐는가?

**결론: "MS 자사 전용"이라는 명시 정책은 근거 없음. 단, 결과적으로 그렇게 보일 수 있음.** MS 자사 IME(일/중/한)는 당연히 "compatible IME"라 브랜딩/모드 아이콘이 뜨고, 호환 요구를 못 맞춘 third-party는 언어 약어만 떠서 **체감상 "자사만 됨"으로 보인다.** 차별 기준은 "MS 제작 여부"가 아니라 "Input Indicator 호환 구현 여부"(질문 1 근거). MS 자사 IME 전용 차단 정책 문서는 **확인 필요(미발견)**.

## 질문 4 — TSF DLL이 Windows 업데이트 후 로드 실패/등록 무효화되는 케이스?

**결론: 보고됨 (TSF/IME 회귀 다수).**
- Win11 22H2: 키보드 단축키로 IME 모드 전환 시 TSF 컴포넌트를 로드하는 앱이 멈추는 버그 → KB5020044(2022-11-29)로 수정. 출처: Microsoft Support *Excel may stop or close when using new IME in Windows 11* / Neowin 보도 / winaero.
- 22H2 + VCL/Delphi 앱 IME 문제: Embarcadero docwiki *Windows 11 22H2 and VCL Applications IME Trouble*.
- 일반: 업데이트가 systray 아이콘 캐시 위치를 리셋하거나 third-party 핸들러 재등록에 실패 → 아이콘이 overflow로 가거나 초기화 실패(WebSearch: tech-champion systray icons missing). ctfmon.exe 재실행으로 임시 복구되는 사례(한국어 블로그 다수).
- system file 손상 시 sfc/scannow·DISM, Touch Keyboard and Handwriting Panel Service 재시작이 표준 복구책.
- **UNIM 관련 주의**: 메모리 기록상 UNIM v0.3.0은 DllRegisterServer가 Categories·LanguageProfile 값을 누락해 입력기 목록에 안 뜨는 별도 결함이 있음 → 이건 OS 회귀가 아니라 자체 등록 결함. 트레이 미표시 원인을 OS 탓으로 돌리기 전에 자체 등록/langbar 구현부터 점검 필요.

## 질문 5 — 다른 third-party IME(RIME/Weasel, 날개셋, 구름 등)의 Win11 동작·우회책?

- **RIME/Weasel #1682** (Win11 22H2, Bug 라벨, 2025-08 open): "weasel 상주 언어바의 우클릭 배포 메뉴를 제공하던 '中' 언어 지시기가 완전·영구적으로 사라짐." 구버전으로 내려도 동일 → third-party langbar 트레이 아이콘이 Win11에서 사라지는 실제 사례.
  - 출처: https://github.com/rime/weasel/issues/1682 (source: `research-b-weasel-1682`)
  - **핵심 정황(중요)**: 보고자가 명시하길 "OS 입력법 지시기는 표시됨, weasel 커서 위치 floating 지시기도 표시됨, weasel 자체 아이콘도 표시됨 — 오직 weasel 자체 작업표시줄 'A/中' 지시기만 영구 소멸." 즉 **OS Input Indicator·자체 floating UI는 살아있고 third-party langbar 트레이 아이템만 사라짐** → 트레이 langbar item 경로가 Win11에서 가장 취약함을 실증. 구버전으로 내려도 동일, 레지스트리/시스템 문제로 추정한다고 보고자 결론.
  - 참고: weasel은 `weasel.yaml`의 `hide_ime_mode_icon`(TSF language bar icon 숨김) 파라미터 보유(#811) — third-party가 langbar 트레이 아이콘을 직접 제어하려 시도하는 정황.
  - Weasel은 candidate UI를 자체 렌더링하므로 모드 표시 의존도가 낮지만, 트레이 우클릭 메뉴(배포/재배포) 접근이 막히는 UX 손상.
- **날개셋/한글 IME**: 한/영 전환 후 트레이 아이콘 미표시 시 ctfmon.exe 재실행으로 복구되는 사례 보고(한국어 블로그). Win11 22H2 한영 전환 불가 시 "이전 버전 Microsoft IME" 토글이 흔한 우회책(단 이건 MS IME 한정).
- **공통 우회책 후보**:
  1. IME가 candidate/모드 UI를 **자체 렌더링**(MS 요구: TSF IME는 자체 candidate window를 그려야 함) → 트레이 아이콘 의존 최소화.
  2. 트레이 대신 별도 floating indicator(자체 윈도우) 제공 — RIME식.
  3. legacy langbar 강제(고급 키보드 설정 + `ShowStatus`=4) 안내 — 사용자 측 우회.
  4. Input Indicator 호환을 위해 langbar item을 MS 요구 방식으로 구현 + IME 플래그 정확히 set.

---

## UNIM 권고 (조사 결론 기반, 코드 수정 아님)

1. **트레이 미표시를 OS 버그로 단정하지 말 것.** 1차 원인 후보는 (a) langbar/Input Indicator 비호환 구현, (b) v0.3.0 등록 결함(Categories/LanguageProfile 누락). 둘 다 자체 수정 가능.
2. MS는 "Win7식 langbar-only 모드 아이콘"을 Win8+ 트레이에서 보장하지 않음 → **자체 모드 표시 UI(floating indicator/팝업)** 가 가장 견고. (UNIM은 이미 팝업 상태머신 보유 — 활용 검토.)
3. Win11 24H2/25H2 Input Indicator 위치/표시 회귀는 OS측 미해결 → UNIM이 트레이에 전적으로 의존하면 OS 회귀에 그대로 노출됨. 의존도 낮추는 설계가 안전.

## 불확실/확인 필요 항목

- "Win11이 SHOWNINTRAY third-party langbar item을 트레이에 명시적으로 안 그린다"는 직접 공식 문구: 미발견(질문 2).
- "MS 자사 IME 전용 표시" 명시 정책: 미발견(질문 3) — 실제는 호환 여부 기준.
- Win11 특정 빌드(23H2 vs 24H2)별 third-party langbar 노출 차이의 정밀 매트릭스: 추가 조사 필요.

## 인덱싱된 source 라벨
- `research-b-ms-ime-requirements` — MS Learn Custom IME requirements (핵심 근거)
- `research-b-w8cookbook-thirdparty-ime` — Win8 third-party IME 문서(동일 정책 원문)
- `research-b-weasel-1682` — RIME/Weasel 트레이 지시기 소멸 이슈
- `research-b-rime-weasel-issues` / `research-b-ms-qa` — 보조 검색 인덱스
