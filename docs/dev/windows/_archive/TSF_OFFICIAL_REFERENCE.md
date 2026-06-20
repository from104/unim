# TSF 공식 문서 기반 재설계 레퍼런스

> 출처: Microsoft Learn (win32/tsf, api/msctf, api/ctfutb) + microsoft/Windows-classic-samples SampleIME (cpp).
> 모든 인용은 공식 문서/공식 샘플 코드. UNIM = Rust in-proc TSF TIP, windows-rs 0.62.2, CLSID `{A1B2C3D4-…7890}`, LangID 0x0412.
> **이 문서는 조사·문서화 전용. 구현은 별도 작업.**

---

## A. 한/영 상태를 OS 입력 표시기에 반영하는 표준 방법 (문제 1 — 최우선)

### A.1 결론 (한 줄)
UNIM은 한/영 토글 시 **`GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`** 와 **`GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION`** 두 thread-manager 컴파트먼트를 `ITfCompartment::SetValue`로 직접 set 해야 한다. 지금 이 컴파트먼트를 건드리지 않아 OS(및 MS IME 표시기)가 이전 상태를 그대로 표시하는 것이 잔상의 원인이다.

### A.2 문서 근거

**컴파트먼트 정의** (출처: `predefined-compartments`, Ctffunc.h/MSCTF.h):
| GUID | 값(VT) | 의미 | 스코프 |
|---|---|---|---|
| `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` | VT_I4(DWORD) | 0 아니면 키보드 open, 0이면 close | **thread manager** |
| `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION` | VT_I4(DWORD) | `TF_CONVERSIONMODE_*` 비트 조합 | **thread manager** |
| `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_SENTENCE` | VT_I4(DWORD) | `TF_SENTENCEMODE_*` | thread manager |

> 인용: "GUID_COMPARTMENT_KEYBOARD_OPENCLOSE … A DWORD that is nonzero if the keyboard is open or zero if the keyboard is closed. This compartment is specific to a thread manager object."
> URL: https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-compartments

**Conversion 모드 비트** (출처: `flags-for-conversion-mode`, IMM32의 `IME_CMODE`와 동등):
- `TF_CONVERSIONMODE_ALPHANUMERIC = 0x0000` (영문)
- `TF_CONVERSIONMODE_NATIVE = 0x0001` (NATIVE = 한글 모드. 한국어는 이 비트 1/0로 한/영 표시)
- (참고) KATAKANA 0x0002 / FULLSHAPE 0x0008 등은 일/중용.

> 인용: "TF_CONVERSIONMODE_NATIVE 0x0001 Set to 1 if NATIVE mode; 0 if ALPHANUMERIC mode."
> "This is equivalent with IME_CMODE values for IMM32."
> URL: https://learn.microsoft.com/en-us/windows/win32/tsf/flags-for-conversion-mode

→ **한국어(0x0412)의 한/영은 conversion 모드의 NATIVE 비트로 표현**된다. OPENCLOSE(IME on/off)와 CONVERSION(NATIVE vs ALPHANUMERIC)을 함께 동기화하는 것이 SampleIME 패턴이다.

### A.3 SampleIME의 정확한 호출 시퀀스 (출처: Windows-classic-samples / Samples/IME/cpp/SampleIME)

**(1) ActivateEx에서 sink 생성 + advise** — `CompositionProcessorEngine.cpp` (`SetupLanguageBar` 내부):
```cpp
// thread-manager 스코프 컴파트먼트 객체 생성
_pCompartmentConversion = new CCompartment(pThreadMgr, tfClientId,
                                           GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION);

_pCompartmentKeyboardOpenEventSink = new CCompartmentEventSink(CompartmentCallback, this);
_pCompartmentConversionEventSink   = new CCompartmentEventSink(CompartmentCallback, this);

// 변경 통지 sink advise
_pCompartmentKeyboardOpenEventSink->_Advise(pThreadMgr, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
_pCompartmentConversionEventSink  ->_Advise(pThreadMgr, GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION);
```

**(2) `_Advise` 내부** — `Compartment.cpp` (`CCompartmentEventSink::_Advise`):
```cpp
punk->QueryInterface(IID_ITfCompartmentMgr, &pCompartmentMgr);   // ITfThreadMgr → CompartmentMgr
pCompartmentMgr->GetCompartment(guidCompartment, &_pCompartment); // 컴파트먼트 획득(없으면 생성)
_pCompartment->QueryInterface(IID_ITfSource, &pSource);
pSource->AdviseSink(IID_ITfCompartmentEventSink, this, &_dwCookie); // 변경 통지 등록
```

**(3) 한/영 토글 시 값 set** — `CompositionProcessorEngine.cpp` (PreservedKey 처리, OPENCLOSE 토글):
```cpp
CCompartment CompartmentKeyboardOpen(pThreadMgr, tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);
CompartmentKeyboardOpen._SetCompartmentBOOL(isOpen ? FALSE : TRUE);  // 토글 → OS에 반영
```

**(4) OPENCLOSE 변경 시 CONVERSION도 동기화** — `KeyboardOpenCompartmentUpdated`:
```cpp
_pCompartmentConversion->_GetCompartmentDWORD(conversionMode);
CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);
if (isOpen && !(conversionMode & TF_CONVERSIONMODE_NATIVE))
    conversionMode |= TF_CONVERSIONMODE_NATIVE;       // open=한글 → NATIVE 켬
// (close 시 NATIVE 끄기 대칭 처리) → _pCompartmentConversion->_SetCompartmentDWORD(...)
```

**(5) `SetValue`의 실제 호출** — `Compartment.cpp` (`_SetCompartmentBOOL/_SetCompartmentDWORD`):
```cpp
VARIANT var; var.vt = VT_I4; var.lVal = dw;
pCompartment->SetValue(_tfClientId, &var);   // tfClientId 필수
```

**(6) Deactivate에서 정리** — `SampleIME.cpp::Deactivate`:
```cpp
CCompartment CompartmentKeyboardOpen(_pThreadMgr, _tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
CompartmentKeyboardOpen._ClearCompartment();
// 단, _ClearCompartment는 OPENCLOSE에 대해 S_FALSE 반환(클리어 안 함) — 시스템 공유 상태이므로 보존
```
> 주의: `Compartment.cpp::_ClearCompartment`는 `IsEqualGUID(_guidCompartment, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)`이면 `S_FALSE`로 즉시 반환한다. OPENCLOSE는 다른 TIP/시스템과 공유되는 thread 상태라 함부로 clear하지 않는 것이 정석.

### A.4 잔상 원인 진단
- OPENCLOSE/CONVERSION 컴파트먼트는 **thread manager 스코프** → 같은 스레드의 모든 TIP과 OS 입력 표시기가 공유하는 단일 상태.
- UNIM이 자체 내부 플래그로만 한/영을 토글하고 이 컴파트먼트를 set하지 않으면, OS는 마지막으로 set된 값(이전 MS IME 상태)을 계속 표시 → **MS IME 한/영 표시기 잔존**.
- 해법: 토글 시 두 컴파트먼트를 set + `_Advise`로 외부 변경(다른 TIP/Win key 등) 수신해 내부 상태 역동기화.

---

## B. ActivateEx 표준 초기화 순서 (문제 3)

### B.1 ActivateEx 시그니처/규칙
```cpp
HRESULT ActivateEx(ITfThreadMgr *ptim, TfClientId tid, DWORD dwFlags);
```
- 사용자 세션 시작 시 활성화. **`ITfTextInputProcessorEx`를 구현하면 `ActivateEx`가 불리고 `Activate`는 안 불린다.**
- 매니저는 반환값 **무시**. (실패해도 OS가 신경 안 씀 → 내부 정리는 스스로)
- dwFlags: `TF_TMAE_SECUREMODE`(보안 데스크톱·설정 다이얼로그 자제), `TF_TMAE_COMLESS`(COM 미초기화 가능), `TF_TMAE_CONSOLE`(콘솔), `TF_TMAE_WOW16`.
> URL: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itftextinputprocessorex-activateex

### B.2 SampleIME `ActivateEx` 정확한 순서 (출처: `SampleIME.cpp`)
```cpp
STDAPI CSampleIME::ActivateEx(ITfThreadMgr *pThreadMgr, TfClientId tfClientId, DWORD dwFlags) {
    _pThreadMgr = pThreadMgr; _pThreadMgr->AddRef();      // 1. ThreadMgr 보관 + AddRef
    _tfClientId = tfClientId; _dwActivateFlags = dwFlags; // 2. clientId/flags 보관

    _InitThreadMgrEventSink();                            // 3. ThreadMgr 이벤트 sink
    if (GetFocus(&pDocMgrFocus)==OK && pDocMgrFocus)
        _InitTextEditSink(pDocMgrFocus);                  // 4. 현재 포커스 문서에 edit sink
    _InitKeyEventSink();                                  // 5. 키 이벤트 sink (ITfKeystrokeMgr::AdviseKeyEventSink)
    _InitActiveLanguageProfileNotifySink();               // 6. 프로필 변경 통지 (한/영 등)
    _InitThreadFocusSink();                               // 7. thread focus sink (OnSetThreadFocus)
    _InitDisplayAttributeGuidAtom();                      // 8. 표시 속성 atom
    _InitFunctionProviderSink();                          // 9. function provider
    _AddTextProcessorEngine();                            // 10. 엔진+langbar+컴파트먼트 advise(=A.3-(1))
    return S_OK;  // 실패 시 Deactivate() 호출 후 E_FAIL
}
```
> **핵심: 컴파트먼트 sink advise(A.3)는 `_AddTextProcessorEngine` 단계(10)에서 일어난다.** UNIM은 ActivateEx 안에서 thread-manager 컴파트먼트 OPENCLOSE/CONVERSION을 advise해야 함.
> `Deactivate`는 역순 Uninit + 컴파트먼트 정리(A.3-(6)).

---

## C. 기본 입력기 / Assemblies Default 지정 (문제 2)

### C.1 등록 (DLL 등록 시 1회)
- COM in-proc 등록 + TSF 등록 2단계. `ITfInputProcessorProfiles::Register(clsid)` → `AddLanguageProfile(clsid, langid, guidProfile, desc, icon…)`.
> 인용: "A text service registers itself with TSF by calling ITfInputProcessorProfiles::Register with the class identifier" / "register itself for all of the languages that it supports … AddLanguageProfile".
> URL: https://learn.microsoft.com/en-us/windows/win32/tsf/text-service-registration

### C.2 "기본" 설정 두 API의 구분 (이게 UNIM 혼선의 핵심)
| API | 인터페이스 | 역할 | 컨텍스트 요건 |
|---|---|---|---|
| `SetDefaultLanguageProfile(langid, rclsid, guidProfiles)` | `ITfInputProcessorProfiles` | 해당 **언어의 기본 프로필**을 영구 지정(레지스트리 `CTF\Assemblies\...\Default`) | 시스템 설정 변경 — 등록/설정 단계에서 |
| `ActivateLanguageProfile(rclsid, langid, guidProfile)` | `ITfInputProcessorProfiles` | **현재 스레드**에서 즉시 활성 프로필로 전환 | 활성 스레드(ThreadMgr) 컨텍스트 필요 |
| `ActivateProfile(dwProfileType, langid, clsid, guidProfile, hkl, dwFlags)` | `ITfInputProcessorProfileMgr` | 프로필/키보드레이아웃 활성화(신 API, HKL 포함) | 활성 스레드 컨텍스트 |

> URL(SetDefault): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofiles-setdefaultlanguageprofile
> URL(ProfileMgr): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfinputprocessorprofilemgr

### C.3 UNIM 진단
- PowerShell `SetDefaultLanguageProfile`이 "됐다"는 것은 `CTF\Assemblies\0x0412\{TIP}\Default` 레지스트리 기록까지만. 이는 **OS 부팅/로그온 시 기본 선택**에 영향.
- DLL `ActivateEx`는 "현재 활성화됐을 때 불리는 콜백"일 뿐 **기본 지정 API가 아니다.** ActivateEx 안에서 기본을 set하려는 시도는 의미가 없다(이미 활성화된 상태).
- 코드로 즉시 전환하려면 활성 스레드에서 `ActivateLanguageProfile`/`ActivateProfile`를 호출해야 하며, 이는 보통 **UNIM TIP 자신이 아니라 전환을 트리거하는 컨텍스트**(설정 앱/트레이)에서 ThreadMgr를 잡아 호출한다.
- 정석: 설치/설정 단계에서 `SetDefaultLanguageProfile`(영구), 런타임 전환은 OS 입력 전환(Win+Space 등) 또는 `ActivateProfile`.

---

## D. 언어바 아이템 vs Win11 입력 표시기 (문제 4)

### D.1 langbar 아이템 구조
- `ITfSource` + `ITfLangBarItem` 파생 1종 구현. 설치 시 langbar가 `AdviseSink(IID_ITfLangBarItemSink)`로 sink를 건다. 상태 변경은 이 sink로 통지.
- 버튼 스타일은 `TF_LANGBARITEMINFO.dwStyle`로 결정: `TF_LBI_STYLE_BTN_BUTTON`(push), `_BTN_TOGGLE`(토글), `_BTN_MENU`(메뉴).
- `GetIcon`은 langbar 매니저가 아이콘 그릴 때 호출. **상태 변화 후 아이콘을 갱신하려면 sink로 변경을 통지(OnUpdate)해 GetIcon 재호출을 유도**해야 한다.
> URL(개념): https://learn.microsoft.com/en-us/windows/win32/tsf/language-bar
> URL(버튼): https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nn-ctfutb-itflangbaritembutton

### D.2 Win11 입력 표시기 — UILess 모드 주의
- Win11의 트레이 입력 표시기는 클래식 langbar와 별개. TIP가 게임/전체화면 등 **UILess 스레드**에서 활성화되려면 `ITfTextInputProcessorEx` 구현이 **필수**(미구현 시 UILess 스레드에서 TIP 자체가 비활성).
- TIP가 가시 UI를 그리기 전 `ITfUIElementMgr::BeginUIElement` 호출 → 앱이 `ITfUIElementSink`로 표시 허용 여부 결정.
> URL: https://learn.microsoft.com/en-us/windows/win32/tsf/uiless-mode-overview
- **추정 결론**: Win11에서 "한글" 표시가 안 뜨는 것은 GetIcon 문제가 아니라, A절의 **OPENCLOSE/CONVERSION 컴파트먼트 미반영**으로 OS 입력 모드 표시기가 갱신되지 않기 때문일 가능성이 높다. langbar GetIcon 수정보다 A절이 우선.

---

## E. UNIM 적용 변경 — 우선순위 (구현은 별도)

1. **[P0] 한/영 동기화** — ActivateEx의 엔진 초기화 단계에서:
   - ThreadMgr를 `QueryInterface(IID_ITfCompartmentMgr)` → `GetCompartment(OPENCLOSE)` / `GetCompartment(INPUTMODE_CONVERSION)`.
   - 한/영 토글 시 `SetValue(tfClientId, VT_I4)`로 OPENCLOSE(BOOL) + CONVERSION(NATIVE 비트) **둘 다** set.
   - 두 컴파트먼트에 `ITfCompartmentEventSink` advise → 외부(다른 TIP/Win key) 변경 시 내부 상태 역동기화.
2. **[P0] ActivateEx 순서 정렬** — B.2 순서대로 sink advise, 컴파트먼트 advise를 ActivateEx 내부로. Deactivate 역순 정리(단 OPENCLOSE는 Clear 금지).
3. **[P1] 기본 입력기** — `SetDefaultLanguageProfile`는 설치/설정 단계에서만(레지스트리 영구), ActivateEx에서 기본 지정 시도 제거. 런타임 강제 전환 필요 시 `ITfInputProcessorProfileMgr::ActivateProfile`.
4. **[P1] langbar 아이콘 갱신** — 토글 후 langbar item sink로 변경 통지해 GetIcon 재호출 유도. 단 Win11 표시기 자체는 P0(컴파트먼트)로 해결되는지 먼저 검증.
5. **[P2] UILess/콘솔** — `ITfTextInputProcessorEx` 유지 확인, `TF_TMAE_CONSOLE`/CUAS 경로 별도 조사(본 문서 범위 밖).

---

## F. 인덱싱된 지식베이스 source 라벨 (ctx_search용)

**TSF 공식 문서:**
- `tsf-docs-overview`, `tsf-docs-using-toc`, `tsf-docs-compartments`, `tsf-docs-predefined-compartments`,
  `tsf-docs-conversion-mode-flags`, `tsf-docs-registration`, `tsf-docs-langbar`, `tsf-docs-langbaritembutton`,
  `tsf-docs-activateex`, `tsf-docs-itfcompartment`, `tsf-docs-itfcompartmentmgr`,
  `tsf-docs-inputprocessorprofiles`, `tsf-docs-profilemgr`, `tsf-docs-setdefaultprofile`, `tsf-docs-uiless`

**SampleIME 소스:**
- `sampleime-activateex` (SampleIME.cpp — ActivateEx/Deactivate 순서)
- `sampleime-compartment-engine` (CompositionProcessorEngine.cpp — 컴파트먼트 advise/set, 한/영 동기화)
- `sampleime-compartment-class` (Compartment.cpp — CCompartment/_Advise/_SetCompartment* 헬퍼)
- `sampleime-keyeventsink` (KeyEventSink.cpp — 키/포커스 처리)

로컬 사본: `C:\Users\USER\Desktop\work\unim\_workspace\sampleime\` (Compartment.cpp/.h, SampleIME.cpp, CompositionProcessorEngine.cpp, KeyEventSink.cpp, TfInputProcessorProfile.cpp)

## G. 못 받은 문서 (404 — 슬러그 불일치)
- `tsf/keyboard-input`, `tsf/the-language-bar`, `tsf/registering-text-services`, `tsf/categories-of-text-services`, `tsf/text-services-framework`(개념지수는 받음), `tsf/programming-elements`, `tsf/predefined-guids` → 모두 404.
  - 실제 슬러그로 대체 확보: `language-bar`, `text-service-registration`, `predefined-category-values`(미인덱싱), `predefined-compartments`, `text-services-framework`(overview는 OK).
  - GitHub `MicrosoftDocs/win32/desktop-src/TSF` 디렉터리에서 전체 171개 슬러그 목록 확인함(필요 시 추가 fetch 가능).
