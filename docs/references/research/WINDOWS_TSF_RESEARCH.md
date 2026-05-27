# Windows TSF (Text Services Framework) 조사

> 조사일: 2026-05-28 · 대상: UNIM `unim-tsf` 크레이트 + 최신 Microsoft 공식 정보
> 분류: 기술 레퍼런스 (Windows 입력기 아키텍처)

## 1. TSF란 무엇인가

**TSF**는 Windows XP에서 도입된 **COM 기반 텍스트 입력 프레임워크**다. 키보드 IME,
필기 인식, 음성 인식 같은 "고급 텍스트 입력 서비스"를 애플리케이션에 전달하는 표준
통로다. 과거의 **IMM32(Input Method Manager)** 를 대체하는 현대 계층이며, Windows
10/11에서 1차 표준이다.

핵심은 **애플리케이션과 입력기 사이에 풍부한(rich) 텍스트 메타데이터를 양방향으로
주고받는다**는 것이다. IMM32가 "확정 문자열"만 넘겼다면, TSF는 조합 중
텍스트(composition), 커서 주변 텍스트(surrounding text), 선택 영역, 표시 속성(밑줄·색상)
까지 다룬다.

```
┌─────────────┐   ITfThreadMgr    ┌──────────────┐
│ Application │◄─────────────────►│ TSF Manager  │
│ (Text Store)│   ITfContext      │  (msctf.dll) │
└─────────────┘                   └──────┬───────┘
                                         │ 로드
                                  ┌──────▼───────┐
                                  │ TIP (입력기)  │  ← unim-tsf.dll
                                  │ ITfTextInput  │
                                  │   Processor   │
                                  └──────────────┘
```

- **Text Store**: 애플리케이션이 구현하는 텍스트 보관소(`ITextStoreACP`). 메모장·브라우저·Office 등.
- **TSF Manager (`msctf.dll`)**: 중재자. 어느 입력기가 활성인지, 어느 문서가 포커스인지 관리.
- **TIP (Text Input Processor)**: 입력기 본체. UNIM의 `unim-tsf.dll`이 해당. 인프로세스 COM DLL로 로드.

## 2. 핵심 COM 인터페이스 (TIP가 구현)

| 인터페이스 | 역할 |
|---|---|
| **ITfTextInputProcessor** / **...Ex** | TIP 진입점. `Activate`/`Deactivate` 라이프사이클. Ex는 활성화 플래그 추가 |
| **ITfKeyEventSink** | 키 이벤트. `OnTestKeyDown`(소비 여부 판단) → `OnKeyDown`(실제 처리) |
| **ITfCompositionSink** | 조합(composition) 종료 알림 (`OnCompositionTerminated`) |
| **ITfThreadMgrEventSink** | 스레드 매니저 이벤트. `OnSetFocus`로 문서 포커스 전환 감지 |
| **ITfTextEditSink** | 텍스트 변경 감지 (`OnTextChange`) |
| **ITfDisplayAttributeProvider** | 조합 텍스트의 밑줄/배경색 등 표시 속성 제공 |
| **ITfEditSession** | **모든 텍스트 변경의 관문.** `RequestEditSession`으로 읽기/쓰기 락을 얻어야만 텍스트 조작 가능 |

> **TSF의 가장 중요한 규칙**: 텍스트를 직접 못 건드린다. 반드시
> `ITfContext::RequestEditSession`으로 edit session을 요청하고, 콜백 안에서 `ITfRange`를
> 통해서만 읽고 쓴다. IME는 보통 `TF_ES_SYNC | TF_ES_READWRITE` 플래그로 동기 락을 잡는다.

## 3. 한글 입력 흐름 (UNIM 구현 기준)

```
ㄱ 키 입력
  → ITfKeyEventSink::OnTestKeyDown   (이 키 먹을까? → Korean 모드면 true)
  → ITfKeyEventSink::OnKeyDown       (엔진에 전달)
      → InputEngine::press_key()     (Rust 코어, OS 무관)
      → InputResult { action, preedit }
          ├ StartComposition  → composition.Start(range)
          ├ UpdateComposition → RequestEditSession → SetText(preedit)
          ├ CommitComposition → SetText(final) + EndComposition()
          └ CancelComposition → EndComposition() (텍스트 없음)
```

UNIM 설계의 핵심은 **입력 로직(코어)을 OS와 분리**한 점이다. Linux(XIM/GTK/Qt/Wayland)와
Windows(TSF)가 **같은 `InputEngine` 코어**를 공유하고, TSF 계층은 "코어 결과를 TSF edit
session으로 번역"하는 어댑터일 뿐이다.

### UNIM unim-tsf 소스 구성 (조사 시점)

| 파일 | 역할 |
|------|------|
| `lib.rs` | DLL 진입점 (DllMain, DllGetClassObject, DllCanUnloadNow) |
| `text_service.rs` | UnimTextService 본체. Activate/Deactivate, OnTestKeyDown/OnKeyDown 등 |
| `key_handler.rs` | 키 로직: modifier 상태, 소비 판단, VK→KeyCode→InputEngine |
| `composition.rs` | CompositionManager: start/update/end. ITfEditSession으로 텍스트 적용 |
| `auto_typefix.rs` | AutoTypeFix 상태 관리 (KeystrokeBuffer, UndoState, Blacklist, UserDictionary) |
| `popup_window.rs` | 한자/특수문자/이모지 9×9 격자 (GDI, WS_EX_NOACTIVATE) |
| `register.rs` | COM 레지스트리 등록/해제, profile 등록 (regsvr32 대상) |

## 4. 등록(Registration) 흐름

TIP는 단순 DLL이 아니라 **시스템 전역에 등록되는 COM 서버**다.

1. **COM 서버 등록**: `HKCR\CLSID\{CLSID}\InProcServer32` → DLL 경로, `ThreadingModel = Apartment` (STA)
2. **프로파일 등록**: `ITfInputProcessorProfiles::Register(CLSID)` + `AddLanguageProfile(CLSID, 0x0412=한국어, PROFILE_GUID, "UNIM Korean IME")`
3. **카테고리 등록**: `ITfCategoryMgr::RegisterCategory`로 `GUID_TFCAT_TIPCAP_*` 능력 선언
4. `regsvr32 unim-tsf.dll` 한 줄로 `DllRegisterServer` 트리거

> **TSF3 카테고리**: `GUID_TFCAT_TIPCAP_TSF3`는 "이 IME가 Windows 8+ 스토어
> 앱(현 UWP/WinUI 환경)에서도 동작한다"고 선언하는 능력 플래그다. 없으면 모던 앱에서
> IME가 안 뜰 수 있어 **반드시 등록**해야 한다. → register.rs 점검 필요 항목.

## 5. 최신 정보 / 주목할 점 (2025–2026)

### ① windows-rs 버전 — 업그레이드 여지
UNIM은 현재 **windows-rs 0.58**을 쓰는데, 최신은 **`windows` 0.62.2 / `windows-sys`
0.61.2**다. windows-rs는 Win32 메타데이터에서 바인딩을 자동 생성하므로 TSF 인터페이스
커버리지는 완전하다. 0.58→0.62 사이 COM `#[implement]` 매크로와 `windows-core` 분리
구조가 바뀌었으니, 업그레이드 시 인터페이스 구현 보일러플레이트 조정 필요. Rust IME
레퍼런스로 Microsoft의 [`ime-rs`](https://github.com/saschanaz/ime-rs)(공식 C++ IME 샘플의
Rust 포팅)가 좋은 참고점.

### ② TSF가 보안 연구 표적이 됨 (2025)
2025년 Praetorian 등에서 **TSF를 통한 코드 인젝션/지속성(persistence) 기법**이 공개됐다.
TIP DLL이 "GUI나 입력을 받는 거의 모든 프로세스"에 로드되는 TSF 특성을 악용하는 방식.
정상 IME에는 직접 위협은 아니지만, **서명(signing)·등록 권한·DLL 경로 검증**을 엄격히
해두는 게 배포 시 중요하다. MSI 서명 체인이 의미 있는 이유.

### ③ TSF가 사실상 유일한 미래 경로
IMM32는 레거시 호환 계층으로만 남았고, **모던 앱(WinUI3, 브라우저, 최신 Electron)은
TSF만 제대로 지원**한다. UNIM이 IMM32 대신 TSF를 택한 것은 정방향 선택이다.

## 요약

- TSF는 **COM 기반·양방향·rich 텍스트** 입력 프레임워크로 IMM32의 현대적 후속.
- TIP는 **인프로세스 COM DLL**이며, 텍스트 변경은 반드시 **edit session + ITfRange**를 거친다.
- UNIM `unim-tsf`는 6개 핵심 sink 인터페이스를 구현하고 **OS 무관 InputEngine 코어를 공유**하는 어댑터 구조.
- 실무 권장: **windows-rs 0.58→0.62 업그레이드 검토**, **TSF3 카테고리 등록 확인**, **DLL 서명·등록 보안 강화**.

## 출처

- [Text Services Framework — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework)
- [Using Text Services Framework — Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/tsf/using-text-services-framework)
- [windows::Win32::UI::TextServices — windows-rs docs](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/TextServices/)
- [windows 0.62.2 — docs.rs](https://docs.rs/crate/windows/latest)
- [ime-rs (Microsoft IME 샘플 Rust 포팅)](https://github.com/saschanaz/ime-rs)
- [Leveraging Microsoft TSF for Red Team Operations — Praetorian (2025)](https://www.praetorian.com/blog/leveraging-microsoft-text-services-framework-tsf-for-red-team-operations/)
