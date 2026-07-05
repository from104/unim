# Bridge investigation: Active IMM (AIMM / IActiveIMMApp)

조사 각도: `active-imm-aimm`. 사용자 제보 "TSF-IMM32 브릿지가 있다"를 적대적으로 검증.
결론 요약: **AIMM은 UNIM이 wezterm에 inline preedit을 넣는 데 쓸 수 없다.** 방향이 반대이고 Win2000+에서 비활성.

---

## 1. AIMM이 실제로 무엇인가 (1차 소스)

- **정의 (MS Learn, "Active Input Method Manager" aa752003):**
  "Active IMM is a Microsoft ActiveX object that provides *limited IMM service on non-Asian language
  versions of Windows 95, Windows 98, and Windows NT 4.0*." 비-아시아 Windows에서 아시아 IME를
  쓸 수 있게 해주는 IE 시절 ActiveX 컴포넌트.
  - 출처: https://learn.microsoft.com/en-us/previous-versions/windows/internet-explorer/ie-developer/platform-apis/aa752003(v=vs.85)
- **결정적 비활성 문장 (같은 페이지):**
  "Because Windows 2000 contains cross-language version IMM support, it does not require Active IMM.
  **Therefore, Active IMM is disabled for Windows 2000 and later.**"
  → Windows 11에서 AIMM 서비스 자체가 비활성. UNIM 타깃 환경에서 죽은 기술.
- 헤더/IDL: `Dimm.h` / `Dimm.idl` (MS Learn 각 메서드 페이지 Requirements). IE 5.0 / Windows XP 표기.

## 2. 방향(directionality) — 핵심

AIMM은 두 인터페이스로 구성. 누가 호출하는가가 결정적:

- **IActiveIMMApp** = "Provides methods for an Active Input Method Manager (IMM) **client application**."
  즉 *텍스트를 소비하는 앱*(우리 케이스로 치면 wezterm)이 호출하는 쪽.
  - 출처: https://learn.microsoft.com/en-us/previous-versions/windows/internet-explorer/ie-developer/platform-apis/aa768120(v=vs.85)
- **IActiveIMMIME** = "Handles the interaction between the Active IMM and an Active **IME**."
  IMM이 IME와 통신하는 쪽.
  - 출처: https://learn.microsoft.com/en-us/previous-versions/windows/internet-explorer/ie-developer/platform-apis/aa768030(v=vs.85)

**"Supporting Active IMM" 절차 (aa752003) — 호출자는 전부 "client application":**
1. `CoCreateInstance`로 Active IMM 인스턴스 생성
2. `IActiveIMMApp` 포인터 획득
3. (메시지 펌프 소유 시) `IActiveIMMMessagePumpOwner` 획득
4. `IActiveIMMApp::Activate` 호출 — 스레드별
5. `IActiveIMMMessagePumpOwner::Start`
6. `DefWindowProc` 대신 `IActiveIMMApp::OnDefWindowProc`
7. `TranslateMessage` 대신 `IActiveIMMMessagePumpOwner::OnTranslateMessage`
8. (옵션) `IActiveIMMApp::FilterClientWindows`로 AIME 허용 윈도우 클래스 제한

→ 이 모든 단계는 **앱이 자기 메시지 루프/DefWindowProc/포커스를 AIMM에 위임**하는 모델.
즉 AIMM을 쓰려면 *wezterm 자신이 AIMM-aware하게 작성*되어 AIMM을 CoCreate하고 자기 메시지를
넘겨야 한다. TIP(UNIM)이 외부에서 AIMM을 CoCreate해서 다른 프로세스(wezterm)의 IMC에
조합을 밀어넣는 경로는 **존재하지 않음**. AIMM의 `AssociateContext`/`GetCompositionStringA` 등은
자기 프로세스/스레드의 IMC 대상이다(HIMC는 thread-local).

또한 명시: "Applications that are not Active IMM-aware remain unaware of the Active IME keyboard layouts."
→ wezterm은 AIMM-aware가 아니므로 AIMM 경로로는 애초에 닿지 않음.

## 3. 사용자가 들은 "TSF-IMM32 브릿지"의 정체 = AIMM 아님, **CUAS**

- **결정적 1차 소스 (MS Learn `ImmDisableTextFrameService`, imm.h):**
  "TSF functionality is provided to applications that are not specifically written to use TSF,
  Input Method Manager (IMM32), **or Active Input Method Manager (AIMM 1.2)**. ...
  This TSF feature is available beginning with Windows XP when ... Msctf.dll and Msimtf.dll are loaded."
  - 출처: https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immdisabletextframeservice
  - 해석: 여기서 AIMM 1.2는 **TSF/IMM32와 나란히 나열된 "앱이 텍스트를 받는 레거시 방식 중 하나"**.
    CUAS(Cicero Unaware Application Support, msctf.dll/msimtf.dll)가 이 세 종류 앱 모두에게
    TSF 호환성을 제공한다. 즉 진짜 브릿지는 **CUAS**이고, AIMM은 브릿지가 아니라 브릿지가
    감싸주는 *대상*(legacy 소비자 API) 중 하나.
- 커뮤니티/연구 설명도 일치: "CUAS is an emulation layer that connects the old IMM32-based
  application and a TSF TIP." (katahiromz/ImeStudy 외 다수)
  - https://github.com/katahiromz/ImeStudy
- 따라서 UNIM이 봐야 할 진짜 브릿지 후보는 CUAS 동작/`ImmDisableTextFrameService`·`ImmDisableIME`
  계열이며, AIMM 각도는 여기서 종결.

## 4. Rust 바인딩 가용성 (참고용 — 그래도 쓸모는 없음)

- `windows-rs`에 `IActiveIMMApp`가 `windows::Win32::UI::Input::Ime`로 노출됨
  (`Activate`, `AssociateContext`, `CreateContext`, `GetCompositionStringA`, `OnDefWindowProc` 등 메서드 존재).
  - 출처: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Input/Ime/struct.IActiveIMMApp.html
- 즉 "호출 가능 여부"는 yes지만, 위 2·3절 때문에 *호출해도 목적 달성 불가*.

## 5. 서드파티 IME 선례

- AIMM(IActiveIMMApp)을 쓰는 **활성 IME 프로젝트 발견되지 않음**. 문서가 전부
  IE-developer/platform-apis 아카이브 경로(레거시)이고, Win2000+ 비활성이라 신규 사용 사례 없음.
  Mozc/Weasel 등 TIP 진영도 AIMM 미사용(CUAS/overlay 경로).

## 6. UNIM 적용 가능성 판정

- **inline in wezterm via AIMM: NO.**
  근거: (a) Win2000+에서 AIMM 서비스 비활성, (b) 방향이 app-as-host이고 TIP→타앱 주입 경로 부재,
  (c) wezterm이 AIMM-aware가 아님, (d) HIMC는 thread-local이라 cross-process 주입 의미 없음.
- 사용자가 들은 "브릿지"는 AIMM이 아니라 **CUAS**. 다음 조사는 CUAS / IMM32 직접(IMM IME `.ime`
  하이브리드) / overlay 각도로 이전.
