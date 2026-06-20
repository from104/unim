# 조사: 서드파티 한국어 IME의 콘솔/레거시 inline preedit 메커니즘

조사 각도: **korean-imes-console** (날개셋/새나루/구름/NavilIME 등이 wezterm·cmd·conhost 같은
순수 IMM32 / 커스텀 렌더러 앱에서 inline preedit을 어떻게 달성하는가, 역공학)

작성일: 2026-06-07 · 1차 소스 우선(MS Learn, IME 소스코드, 제작자 본인 문서/블로그)

---

## 0. 결론 요약 (TL;DR)

- **콘솔/레거시에서 inline 되는 서드파티 한국어 IME는 실재한다**: 날개셋(외부 모듈), 새나루(saenaru),
  MS 기본 한국어 IME. 그러나 **그 inline은 전부 IMM32 `.ime` IME 경로로 달성**된다. 순수 TSF TIP로
  콘솔 inline을 하는 사례는 발견되지 않았다.
- **이것은 "MS만의 특권"이 아니다.** IMM32 IME(`.ime` DLL, `ImeProcessKey`/`ImeToAsciiEx`/
  `ImeSetCompositionString` 등 IME DDI export)는 **공개된 서드파티 작성 가능 API**다. 새나루는
  오픈소스로 `src/`에 완전한 IMM32 IME를 구현해 cmd.exe에서 동작함이 확인됐다.
- 따라서 **"순수 TSF TIP의 레거시 inline 불가" 선행결론은 유지**되지만, **"서드파티가 못 한다"는 함의는
  반증**된다. 우회 경로 = UNIM이 IMM32 `.ime` IME(또는 IMM32 hybrid)를 **추가로** 제공하는 것.
- 사용자 제보한 "TSF-IMM32 브릿지"의 정체: (a) OS의 **CUAS**(TSF TIP → IMM32 앱)는 **콘솔/16비트
  앱을 명시적으로 제외**한다(MS Learn). (b) 날개셋이 쓰는 "에디트 컨트롤에 TSF 임시 도입" 옵션은
  방향이 반대(앱에 TSF 부여)이며 **OS edit/richedit/IE 입력란에만** 적용 — wezterm 같은 커스텀
  렌더러엔 무력. 즉 wezterm을 구제하는 브릿지는 둘 다 아니다. **정답 브릿지는 IMM32 IME 자체.**

---

## 1. 날개셋 한글 입력기 (김용묵 / 한글문화원)

### 1.1 아키텍처 (제작자 본인 문서, moogi.new21.org/ngs_imple.htm)
- 외부 모듈 = "메모장이나 웹 브라우저, Office 등 여타 소프트웨어에서 한글을 입력할 수 있는
  **Windows용 정식 한글 IME**".
- **IMM32 + TSF 양쪽 모두 지원**(WebSearch 인용: "구형의 IMM32 방식도 지원되고 있습니다").
  즉 MS 기본 IME와 동일한 **하이브리드**. 콘솔/레거시 inline은 IMM32 경로에서 나온다(귀결).
- TSF 고급기능(낱자 단위 수정·단어 한자변환)은 "TSF 인터페이스를 지원하는 **일부 프로그램**"에서만.

### 1.2 사용자가 들은 "브릿지"의 실체 — 방향이 반대다
제작자 문서(ngs_imple.htm) + WebSearch가 인용한 cosmic 문서 원문:
> "날개셋 외부 모듈은 TSF를 지원하는 IME일 뿐만 아니라 **운영체제의 에디트 컨트롤에 TSF
> 인터페이스를 임시로 도입하는 옵션이 있습니다.**"
> "운영체제가 제공하는 **비공식 확장 기능을 활용하여**, 운영체제의 **에디트 컨트롤과 리치 에디트
> 컨트롤, 웹 브라우저(IE) 내부의 입력란**에서 날개셋이 **TSF A급으로 동작**하게 합니다."

해석:
- 이것은 "TSF TIP → 레거시 앱" 브릿지가 **아니다**. 반대로 **앱(타깃 edit control)에 TSF document
  manager를 후킹으로 주입**해 그 앱을 TSF 인식 앱으로 승격시키는 기법.
- 적용 대상이 **OS 표준 EDIT / RICHEDIT / IE 입력란으로 한정**된다. wezterm·conhost는 자체
  텍스트 버퍼·렌더러라 EDIT 컨트롤이 아니므로 **이 후킹 대상이 아님** → 이 옵션으로 wezterm inline 불가.
- 따라서 메모장에서 inline 자체는 IMM32 경로로 되는 것이고, 이 옵션은 "단어 한자변환" 같은 *고급*
  편집 기능을 메모장에 얹는 부가물.

### 1.3 제작자 블로그(moogi.new21.org/tc/1153, "날개셋 8.2") — 후킹 철학 확인
- "예전(IMM32)에는 뭔가 **static하고 write-only**이기만 하던 고정 프로토콜에다가 문자열을 넣어서
  메시지만 쏴 주면 됐지만, 지금(TSF)은 … COM 객체 관리 … **레거시 프로그램에 대한 호환 유지,
  스펙대로 동작하지 않는 프로그램에 대한 보정**…"
- 한자 후보창 위치: "cursor 근처 아래에다가 표시해 주는 게 원칙인데, 이걸 **hook 프로시저를 통해
  알아 와야 한다.**" → 주변정보 획득을 **앱 후킹**으로 해결.
- "**훅킹으로 응용 프로그램에서 TSF edit session까지 요청**해 보는 것은 처음" → 1.2의 "TSF 임시
  도입"이 곧 in-process 후킹임을 자인.

➡ 날개셋의 모든 "레거시 우회"는 **(1) IMM32 IME 경로 + (2) 앱 측 후킹**의 조합이지, OS가 주는
  TSF→레거시 마법 브릿지가 아니다.

---

## 2. 새나루 / saenaru (wkpark, Hye-Shik Chang) — 오픈소스 1차 소스 ★결정적

GitHub: https://github.com/wkpark/saenaru (Star 12, 441 commits, BSD)

### 2.1 두 개의 분리된 구현체 (레포 트리 직접 확인)
- `src/` = **IMM32 `.ime` IME**. `src/saenaru.def` EXPORTS:
  ```
  ImeInquire ImeConfigure ImeConversionList ImeProcessKey ImeSelect
  ImeSetActiveContext ImeToAsciiEx NotifyIME ImeSetCompositionString
  ImeRegisterWord ... CompStrWndProc CandWndProc StatusWndProc GuideWndProc
  ```
  → 교과서적 IMM32 IME DDI 전부 export. `imm.c`, `immsec.c`, `uicomp.c`(조합창 UI),
  `toascii.c`, `hangul.c`. `uicomp.c` 헤더 주석: `Copyright (c) 1990-1998 Microsoft
  Corporation` → **MS IMM32 IME SDK 샘플에서 파생**(공개 SDK라서 서드파티가 합법 사용).
- `tip/` = **별도 TSF TIP DLL**. `compose.cpp`, `editsess.h`, `compart.cpp`, `tmgrsink.cpp` 등.
- 즉 새나루도 날개셋·MS IME와 동일한 **IMM32 + TSF 이중 제공** 구조.

### 2.2 cmd.exe inline 동작 확인
- WebSearch(릴리스/문서): "saenaru 1.3.0 … Testing confirmed that Korean input works …
  Notepad, WordPad, Chrome, Firefox, **cmd.exe**, and Store apps."
- cmd.exe에서 되는 이유 = **IMM32 `.ime` 경로**. 콘솔은 TSF가 닿지 않으므로(§4) 이 동작은
  반드시 IMM32에서 나온 것. → **서드파티 IMM32 IME가 콘솔 inline을 한다는 직접 반례 확보.**

### 2.3 UNIM이 베낄 수 있는 부분
- `src/imm.c`의 `GnMsg.lParam = GCS_COMPSTR | GCS_COMPATTR` (WM_IME_COMPOSITION 생성),
  `uicomp.c`의 `CFS_*`/`COMPOSITIONFORM` 처리 = wezterm이 읽는 바로 그 GCS_COMPSTR을 만드는 코드.
- wezterm은 `ImmGetCompositionStringW(GCS_COMPSTR)`을 직접 읽어 자체 렌더하므로, UNIM이 IMM32
  IME로서 GCS_COMPSTR을 올바로 채우면 **MS 기본 IME와 동일하게** wezterm inline이 된다.

---

## 3. 기타 서드파티 (구름은 macOS, NavilIME는 순수 TSF)

- **구름(Gureum)**: macOS 전용(InputMethodKit). Windows·콘솔 무관 → 본 조사 대상 외.
- **NavilIME** (https://github.com/navilera/NavilIME): "Korean IME **based on TSF**" (libhangul).
  **순수 TSF TIP** → 콘솔 inline 사례로 보고된 것 없음. 우리(UNIM)와 같은 한계에 있을 것으로 추정
  (TSF 전용은 콘솔 미지원이라는 §4 원칙과 일치). = 반례가 아니라 **선행결론의 동조 사례**.

---

## 4. 왜 콘솔은 TSF가 못 닿나 — MS 1차 소스

- MS Learn "Input Method Editor and Text Services Framework Accessibility in Windows XP":
  > "all non-TSF-enabled applications, **except 16-bit and console window applications**,
  > are empowered by the TSF-based text services through a compatibility layer."
  → **CUAS(호환 계층)는 콘솔 창을 명시적으로 제외.** 콘솔 = IMM32 전속 영역.
- 즉 우리 실측("빈 composition조차 CUAS가 즉시 terminate")과 정확히 부합. 콘솔에서 TSF TIP의
  composition을 살릴 OS 경로는 설계상 없음.
- conhost는 IMM32(WM_IME_*, ImmGetCompositionString)만 처리. (microsoft/terminal 콘솔 호스트
  아키텍처에 TSF IDataProvider가 일부 있으나 wezterm은 conhost가 아닌 자체 IMM32 처리.)

표준 TSF가 앱에 요구하는 것(realerror IME 구현문서 정리): 앱이 `ITfDocumentMgr`/`ITfContext`/
`ITextStoreACP` 노출 + `StartComposition` 수용. **wezterm은 이걸 구현 안 함** → TSF inline 원천 불가.

---

## 5. UNIM 적용 권고

### 권장 우회 경로: IMM32 `.ime` IME를 추가 제공 (하이브리드化)
- 현 UNIM = 순수 TSF TIP. 콘솔/IMM32-only 앱(wezterm)에서 inline 불가는 **설계 한계이지 버그 아님.**
- 콘솔 inline을 원하면 **MS·날개셋·새나루와 동일하게 IMM32 IME DLL을 별도로 빌드**해야 함.
  - export: IME DDI 전체(`ImeInquire`,`ImeProcessKey`,`ImeToAsciiEx`,`ImeSelect`,`NotifyIME`,
    `ImeSetCompositionString`, UI WndProc들).
  - 핵심: `ImeToAsciiEx`에서 한글 오토마타 돌리고 `WM_IME_COMPOSITION`에 `GCS_COMPSTR`(+
    `GCS_COMPATTR`/`GCS_CURSORPOS`) 세팅, 확정 시 `GCS_RESULTSTR`.
  - **새나루 `src/` (BSD)**가 거의 그대로 참고 가능한 레퍼런스 구현.
- UNIM의 한글 엔진(Rust)은 재사용하고 IMM32 표면만 얇게 입히면 됨(엔진 ↔ `.ime` FFI 경계).

### 비권장 (날개셋식 후킹)
- "앱 edit control에 TSF 주입" 후킹은 OS EDIT/RICHEDIT/IE 한정이라 wezterm엔 무용.
- 임의 앱 글로벌 후킹으로 GCS를 흉내 내는 것은 IMM32 IME를 만드는 것보다 더 취약/위험.

### 효용 한계
- IMM32 IME는 cmd/conhost/wezterm/구형 Win32에서 inline 회복. 단 TSF 전용 신형 앱(UWP 일부)·
  보안 격리 앱엔 별개 이슈. → IMM32(레거시) + TSF(현대) **둘 다 제공이 정석**(=MS·날개셋·새나루).

---

## 6. 핵심 출처

1. moogi.new21.org/ngs_imple.htm (김용묵, 날개셋 구현체 소개; EUC-KR 디코드) — 외부모듈=정식 IME,
   "에디트 컨트롤에 TSF 임시 도입 옵션", "메모장은 TSF 미지원" 등.
2. moogi.new21.org/tc/1153 (김용묵 블로그, 날개셋 8.2) — IMM32 "static·write-only", 후킹으로
   TSF edit session 요청, 한자창 위치 hook 획득.
3. cosmic.mearie.org/f/ngsdoc/{tsf_overview,tsfindex,ngsm_sysopt}.htm — (한국 IP 방화벽으로
   직접 fetch 불가) WebSearch 스니펫 인용으로 확보: "IMM32 방식도 지원", "비공식 확장으로 edit
   control에 TSF A급", A/B/C급 등급 체계.
4. github.com/wkpark/saenaru — `src/saenaru.def`(IMM32 IME DDI export), `src/{imm,uicomp,
   toascii,hangul}.c`, `tip/`(별도 TSF), 릴리스 노트(cmd.exe 동작 확인). BSD, MS IMM32 샘플 파생.
5. MS Learn "Input Method Editor and Text Services Framework Accessibility in Windows XP" —
   CUAS가 16비트·콘솔 창 제외.
6. ime.realerror.com/docs/reference/ime-implementation-details/ — TSF가 앱에 요구하는
   ITfDocumentMgr/ITfContext/ITextStoreACP, StartComposition 구조.
7. github.com/navilera/NavilIME — 순수 TSF 한국어 IME(콘솔 inline 반례 아님, 동조 사례).
8. namu.wiki/위키백과 날개셋 항목 — 일반 개요·문제점(보조).
