# TSF 입력 표시기(작업표시줄 "가/A") 메커니즘 조사

> 목적: UNIM(Rust TSF, windows-rs 0.62.2)이 Windows 11 작업표시줄 입력 표시기에
> 한/영 상태("가"/"A")를 표시하게 만드는 정확한 메커니즘을 **1차 자료**로 확정.
> 추측 금지. 근거 없는 항목은 "근거 못 찾음 — 확인 필요"로 명시.

## 인덱싱한 source 라벨 (메인이 ctx_search 가능)
- `sampleime-langbar-cpp` — Microsoft Windows-classic-samples SampleIME `LanguageBar.cpp` (전체, 113 sections / 16.4KB) — **핵심**
- `sampleime-langbar-h` — 동 `LanguageBar.h` (18 sections / 2.4KB) — `CLangBarItemButton` 클래스 선언
- `sampleime-compengine-cpp` — 동 `CompositionProcessorEngine.cpp` (108 sections / 64.6KB)
- `weasel-langbar-cpp` — rime/weasel `WeaselTSF/LanguageBar.cpp` (53 sections / 12.8KB) — 교차검증 확보
- `tsf-conversion-flags` / `tsf-compartments2` — MS Learn Predefined Compartments / Flags for Conversion Mode (이전 세션 인덱스에 존재)

> raw URL 패턴(검증됨, HTTP 200):
> `https://raw.githubusercontent.com/microsoft/Windows-classic-samples/main/Samples/IME/cpp/SampleIME/<파일>`
> MS Learn `nn-msctf-*` / `ns-msctf-*` API 페이지는 fetch 시 **HTTP 404** (슬러그 불일치). msctf 개념 문서는 `desktop-src/TSF/*` 경로로 접근해야 함.

---

## Q1. Windows 입력 표시기는 무엇을 읽어 "가/A"를 그리는가?

**확정된 1차 근거 (SampleIME `LanguageBar.cpp`):**
작업표시줄/트레이 입력 표시기는 IME가 등록한 **langbar item**(`ITfLangBarItemButton`)을 그린다.
표시 대상이 되려면 그 item의 `GetInfo()`가 채우는 `TF_LANGBARITEMINFO.dwStyle`에
`TF_LBI_STYLE_SHOWNINTRAY` 플래그가 있어야 한다.

```cpp
// LanguageBar.cpp — 생성자
_tfLangBarItemInfo.clsidService = Global::SampleIMECLSID; // 이 TextService 소유
_tfLangBarItemInfo.guidItem     = guidLangBar;            // 이 item의 GUID
_tfLangBarItemInfo.dwStyle      = (TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY);
_tfLangBarItemInfo.ulSort       = 0;
StringCchCopy(_tfLangBarItemInfo.szDescription, ..., description);
```

```cpp
// LanguageBar.cpp — GetInfo: 호출 시마다 SHOWNINTRAY를 다시 OR 한다
STDAPI CLangBarItemButton::GetInfo(_Out_ TF_LANGBARITEMINFO *pInfo)
{
    _tfLangBarItemInfo.dwStyle |= TF_LBI_STYLE_SHOWNINTRAY;
    *pInfo = _tfLangBarItemInfo;
    return S_OK;
}
```

- (a) **langbar item의 GetText/GetIcon** — 표시 내용("가/A" 텍스트/아이콘)의 직접 소스. **YES.**
- (b) compartment `INPUTMODE_CONVERSION` — 표시기를 직접 그리는 소스는 **아님**. 단,
  SampleIME은 compartment 변경을 **sink로 받아** langbar item을 다시 그리도록 트리거한다(Q3 참조).
  compartment는 "상태 저장 + 갱신 트리거"이지 "표시기가 읽는 값"이 아니다.
- (c) `ITfFnGetPreferredTouchKeyboardLayout` — 터치 키보드 레이아웃용. 트레이 표시기와 무관. (SampleIME langbar 경로에 등장 안 함.)
- (d) 별도 UI-less mode 인터페이스 — SampleIME은 langbar item 방식만 사용. **근거상 불필요.**

**Win10 vs Win11 차이 — 근거 못 찾음 (확인 필요).** MS Learn `nn-msctf-*` 페이지가 404라
"Win11 새 입력 표시기가 legacy langbar item을 그리는 조건"의 1차 문서를 확보하지 못함.
현 시점 결론: SampleIME 코드 기준 메커니즘은 `SHOWNINTRAY` langbar item이며,
Win11이 이를 거르는 추가 조건이 있는지는 **미확인**.

---

## Q2. GUID_LBI_INPUTMODE langbar item의 정확한 요구사항

**확정 (SampleIME 기준):**
- item 종류: `ITfLangBarItemButton` (NOT Balloon). `LanguageBar.h`에서
  `class CLangBarItemButton : public ITfLangBarItemButton, public ITfSource`.
- 필수 dwStyle 조합: `TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY`.
  - `BTN_BUTTON` = 버튼형 item.
  - `SHOWNINTRAY` = 트레이/입력 표시기에 노출. **GetInfo에서 매번 재-OR 한다는 점이 핵심.**
- `TF_LBI_STYLE_TEXTCOLORICON`, `TF_LBI_STYLE_BTN_TOGGLE` — SampleIME 입력모드 item에는 **사용 안 함**. 즉 필수 아님.
- GetText / GetIcon: `ITfLangBarItemButton`은 둘 다 구현해야 하는 메서드. 본문 확정:
```cpp
STDAPI CLangBarItemButton::GetText(_Out_ BSTR *pbstrText) {
    *pbstrText = SysAllocString(_tfLangBarItemInfo.szDescription);
    return (*pbstrText == nullptr) ? E_OUTOFMEMORY : S_OK;
}
STDAPI CLangBarItemButton::GetIcon(_Out_ HICON *phIcon) {
    BOOL isOn = FALSE;
    if (!_pCompartment) return E_FAIL;     // compartment 미등록이면 아이콘 실패
    *phIcon = nullptr;
    _pCompartment->_GetCompartmentBOOL(isOn);  // 현재 on/off를 compartment에서 읽음
    DWORD status = 0; GetStatus(&status);
    int desiredSize = _isSecureMode ? 24 : 16; // UAC 모드면 24x24
    if (isOn && !(status & TF_LBI_STATUS_DISABLED))
        *phIcon = LoadImage(..., MAKEINTRESOURCE(_onIconIndex),  IMAGE_ICON, desiredSize, desiredSize, 0);
    else
        *phIcon = LoadImage(..., MAKEINTRESOURCE(_offIconIndex), IMAGE_ICON, desiredSize, desiredSize, 0);
    return (*phIcon != NULL) ? S_OK : E_FAIL;  // 아이콘 NULL이면 E_FAIL
}
```
  - **핵심 1: GetIcon은 `_pCompartment`가 없으면 즉시 E_FAIL.** 즉 langbar item이 compartment를 등록(`_RegisterCompartment`)하지 않으면 아이콘을 못 그린다.
  - **핵심 2: GetIcon은 항상 유효 HICON을 돌려줘야 S_OK.** NULL이면 E_FAIL → 트레이가 그리지 못함.
  - **핵심 3: GetIcon은 on/off를 compartment에서 읽어 on/off 아이콘을 분기한다.** 아이콘이 상태를 반영하는 통로.

---

## Q3. SampleIME의 실제 구현 (확정 부분 + 미회수 부분)

**확정된 코드 시퀀스:**

1. **AddItem** — `ITfLangBarItemMgr::AddItem(this)`:
```cpp
HRESULT CLangBarItemButton::_AddItem(_In_ ITfThreadMgr *pThreadMgr)
{
    ITfLangBarItemMgr* pLangBarItemMgr = nullptr;
    hr = pThreadMgr->QueryInterface(IID_ITfLangBarItemMgr, (void **)&pLangBarItemMgr);
    if (SUCCEEDED(hr)) {
        hr = pLangBarItemMgr->AddItem(this);   // this = CLangBarItemButton
        if (SUCCEEDED(hr)) _isAddedToLanguageBar = TRUE;
        pLangBarItemMgr->Release();
    }
}
```

2. **포커스 시 활성/비활성** — `_UpdateLanguageBarOnSetFocus` → `SetLanguageBarStatus` →
   각 item의 `SetStatus(TF_LBI_STATUS_DISABLED, needDisableButtons)`.
   문서 포커스가 없거나 context 미연결이면 버튼을 DISABLED로. **즉 GetStatus(DISABLED) 상태면 표시기가 흐려지거나 안 뜸.** (UNIM 점검 포인트)

3. **여러 개의 langbar item** — `SetLanguageBarStatus`가 다루는 멤버:
   `_pLanguageBar_IMEMode`, `_pLanguageBar_DoubleSingleByte`, `_pLanguageBar_Punctuation`.
   → 입력모드(한/영 해당) item은 별도(`_pLanguageBar_IMEMode`)로 존재.
   이 item이 **어떤 guidItem으로 생성되는지**(GUID_LBI_INPUTMODE 여부)는
   `CompositionProcessorEngine.cpp`의 생성 코드 확인 필요 — **검색 응답 미회수, 확인 필요.**

4. **compartment ↔ langbar 갱신 흐름 (본문 확정):**

`_RegisterCompartment`: item이 직접 compartment 이벤트 sink를 advise 한다.
```cpp
BOOL CLangBarItemButton::_RegisterCompartment(ITfThreadMgr *pThreadMgr, TfClientId tfClientId, REFGUID guidCompartment) {
    _pCompartment = new CCompartment(pThreadMgr, tfClientId, guidCompartment);
    _pCompartmentEventSink = new CCompartmentEventSink(_CompartmentCallback, this);
    _pCompartmentEventSink->_Advise(pThreadMgr, guidCompartment); // compartment 변경 구독
    ...
}
```

`_CompartmentCallback` (static): compartment 값이 바뀌면 **langbar sink에 OnUpdate를 쏜다**.
```cpp
// static
if (IsEqualGUID(guid, guidCompartment)) {
    if (fakeThis->_pLangBarItemSink)
        fakeThis->_pLangBarItemSink->OnUpdate(TF_LBI_STATUS | TF_LBI_ICON); // ★ 트레이 재그리기 트리거
}
```

`SetStatus` (포커스 변경 시 호출됨)도 동일하게 sink에 OnUpdate:
```cpp
void CLangBarItemButton::SetStatus(DWORD dwStatus, BOOL fSet) {
    ... // _status에 dwStatus를 set/clear, 변경 시 isChange=TRUE
    if (isChange && _pLangBarItemSink)
        _pLangBarItemSink->OnUpdate(TF_LBI_STATUS | TF_LBI_ICON); // ★ 동일 트리거
}
```

`AdviseSink`: 트레이/언어바가 item을 그릴 때 `IID_ITfLangBarItemSink`로 advise → item이 `_pLangBarItemSink`에 보관. **이 sink가 곧 "다시 그려라" 채널.**

- **결론: 표시 갱신의 핵심은 compartment 변경(또는 status 변경)을 받아 `_pLangBarItemSink->OnUpdate(TF_LBI_STATUS | TF_LBI_ICON)`를 호출하는 것.** 텍스트도 바뀌면 `TF_LBI_TEXT`를 OR 해야 함(SampleIME은 아이콘 IME라 STATUS|ICON만 사용).
- compartment(OPENCLOSE)만 SetValue 하는 것으로는 **부족**하다 — item이 그 compartment를 **직접 advise**하고 콜백에서 sink로 OnUpdate를 쏴야 트레이가 다시 읽는다.

---

## Q4. Weasel / 한국어 오픈소스 TSF IME (교차검증)
**Weasel(rime/weasel `WeaselTSF/LanguageBar.cpp`)이 SampleIME과 동일 메커니즘임을 확인:**
- item GUID: **`GUID_LBI_INPUTMODE`** 사용.
  `_pLangBarButton = new CLangBarItemButton(this, GUID_LBI_INPUTMODE, _cand->style());`
- 클래스: `CLangBarItemButton : ITfLangBarItemButton, ITfSource` (QueryInterface가 `IID_ITfLangBarItem`, `IID_ITfLangBarItemButton`, `IID_ITfSource` 지원).
- dwStyle: `TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY` (SampleIME 대비 `BTN_MENU` 추가 — 메뉴 있는 버튼).
- `_pLangBarItemSink` 멤버 보유 + `LANGBARITEMSINK_COOKIE = 0x42424242` 정의 → AdviseSink/OnUpdate 동일 패턴.
- → **GUID_LBI_INPUTMODE + ITfLangBarItemButton + SHOWNINTRAY + sink OnUpdate** 조합이 두 독립 IME에서 공통. 메커니즘 신뢰도 높음.
- windows-rs 기반 Rust TSF 한글 IME 예제: 이번 세션 미검색 — 추가 조사 가능(우선순위 낮음, 위 두 소스로 충분).

---

## Q5. Win11 third-party TSF 입력 표시기 제약
- "Win11이 자사 IME만 입력모드를 트레이에 표시한다"는 **1차 근거 못 찾음 — 확인 필요.**
  (MS Learn API 페이지 404로 Win11 동작 변경 문서 미확보.)
- 레지스트리/그룹정책(입력 표시기 표시 옵션, ProfileFlags) — **근거 못 찾음 — 확인 필요.**
- 현재까지 1차 근거상으로는 **"SHOWNINTRAY langbar item + compartment sink 갱신"이면 표시되어야 한다**가
  SampleIME 기준 결론. Win11 추가 제약 여부는 미확정.

---

## ★ 작업표시줄 "가/A" 표시 필수 조건 체크리스트 (SampleIME 1차 근거)

| # | 조건 | 근거 | UNIM 현재 |
|---|------|------|-----------|
| 1 | item이 `ITfLangBarItemButton` + `ITfSource` 구현 | LanguageBar.h / Weasel QueryInterface | 확인 필요 (ITfSource 구현 여부) |
| 2 | `ITfLangBarItemMgr::AddItem(item)` 등록 | `_AddItem` | **OK** |
| 3 | `GetInfo().dwStyle`에 `TF_LBI_STYLE_BTN_BUTTON \| TF_LBI_STYLE_SHOWNINTRAY` | 생성자 + GetInfo (Weasel 동일) | OK |
| 4 | GetInfo 호출마다 SHOWNINTRAY 재-OR | GetInfo 본문 | 권장 (SampleIME가 매번 OR) |
| 5 | GetText가 유효 BSTR("가"/"A") 반환, NULL이면 E_OUTOFMEMORY | GetText 본문 | OK |
| 6 | **GetIcon가 유효 HICON 반환(NULL이면 E_FAIL)** | GetIcon 본문 | OK(GDI), 단 반환 HRESULT 점검 |
| 7 | **GetIcon이 `_pCompartment` 의존 → item이 compartment를 직접 보유/등록** | GetIcon `if(!_pCompartment) return E_FAIL` | **확인 필요 — 유력 누락** |
| 8 | **item이 OPENCLOSE/INPUTMODE compartment를 `_RegisterCompartment`로 직접 advise** | _RegisterCompartment | **확인 필요 — 가장 유력 누락 ①** |
| 9 | **compartment 변경 콜백에서 `_pLangBarItemSink->OnUpdate(TF_LBI_STATUS\|TF_LBI_ICON)` 호출** | _CompartmentCallback / SetStatus | **확인 필요 — 가장 유력 누락 ②** |
| 10 | `AdviseSink(IID_ITfLangBarItemSink)`로 받은 sink를 `_pLangBarItemSink`에 보관 | AdviseSink 본문 | **확인 필요 — ②의 전제** |
| 11 | 포커스 시 `SetStatus(TF_LBI_STATUS_DISABLED, FALSE)`로 활성화 | _UpdateLanguageBarOnSetFocus | 확인 필요 |
| 12 | 입력모드 item guidItem = `GUID_LBI_INPUTMODE` | Weasel + UNIM | OK |

---

## UNIM에 빠진 것 — 가장 유력한 후보 (근거와 함께)

1. **★ langbar item이 compartment를 직접 advise하고, 변경 콜백에서 sink에 OnUpdate를 쏘는 경로 누락 (체크 #8/#9/#10).**
   UNIM은 한/영 토글 시 compartment(OPENCLOSE/INPUTMODE_CONVERSION)만 thread-level로 `SetValue`한다.
   SampleIME·Weasel은 **langbar item 자신이** `_RegisterCompartment`로 compartment 변경을 구독하고,
   `_CompartmentCallback`에서 `_pLangBarItemSink->OnUpdate(TF_LBI_STATUS | TF_LBI_ICON)`을 호출해
   트레이가 GetText/GetIcon을 **다시 읽게** 만든다. 이 OnUpdate가 없으면 SetValue를 해도
   트레이가 item을 재그리지 않아 "가/A가 안 뜨거나 안 바뀜"으로 나타난다.
   → 1차 근거: SampleIME `_CompartmentCallback`/`SetStatus`의
     `if(...&&_pLangBarItemSink) _pLangBarItemSink->OnUpdate(TF_LBI_STATUS|TF_LBI_ICON);`,
     `_RegisterCompartment`의 `_pCompartmentEventSink->_Advise(...)`,
     `AdviseSink`의 `QueryInterface(IID_ITfLangBarItemSink, &_pLangBarItemSink)`.

2. **GetIcon이 item-소유 compartment에 의존 (체크 #7).**
   SampleIME GetIcon은 `if(!_pCompartment) return E_FAIL;` 후 `_pCompartment->_GetCompartmentBOOL(isOn)`로
   on/off를 읽어 아이콘을 분기한다. UNIM이 GetIcon에서 별도 GDI 아이콘만 생성하고 item에
   compartment를 묶지 않았다면, 위 1번의 OnUpdate 트리거 경로 자체가 성립하지 않는다(같은 뿌리).
   → 1차 근거: SampleIME GetIcon 본문.

(보조) GetInfo에서 SHOWNINTRAY 매-호출 재-OR(체크 #4), 포커스 시 SetStatus로 DISABLED 해제(체크 #11)도
SampleIME가 명시적으로 수행하므로 함께 반영 권장.

---

## 코드로 가능한가 / Win11 제약인가 — 판정

- **현 1차 근거 기준: 코드로 가능하다고 본다.** SampleIME(MS 공식 샘플)이 동일한
  `SHOWNINTRAY` langbar item + compartment sink 갱신 메커니즘으로 트레이 표시를 달성한다.
  UNIM 실패는 위 1~2번(특히 **sink OnUpdate 통지 + GetStatus 포커스 활성화**) 누락 가능성이 가장 높다.
- **단, "Win11 third-party 제약" 가능성은 배제 못 함 — 근거 못 찾음.** MS Learn API 페이지 404로
  Win11 동작 변경 문서를 확보하지 못했다. 코드 보강 후에도 안 뜨면 이 경로를 재조사해야 한다.

---

## 파일 경로
- 본 문서: `C:\Users\USER\Desktop\work\unim\docs\dev\windows\TSF_INPUT_INDICATOR_RESEARCH.md`

## 못 받은 자료 (다음 세션 우선순위)
1. SampleIME `CompositionProcessorEngine.cpp`의 `_pLanguageBar_IMEMode` **생성 시 넘기는 guidItem/onIconIndex/offIconIndex/어느 compartment를 _RegisterCompartment 하는지** 정확 라인 — `ctx_search(source:"sampleime-compengine-cpp", queries:["_pLanguageBar_IMEMode new CLangBarItemButton GUID","_RegisterCompartment GUID_COMPARTMENT_KEYBOARD"])`.
2. Win11 입력 표시기/third-party 제약 1차 문서: MS Learn `desktop-src/TSF/*` 정확 슬러그(404 회피) 또는 WebSearch. **현재 제약 근거 미확보.**
3. `ITfLangBarItemSink::OnUpdate`, `TF_LBI_STATUS_BTN_TOGGLE` 등 플래그 정의의 MS Learn 1차 페이지(슬러그 확인 필요).

## 핵심 코드 본문은 모두 확보 완료
GetText/GetIcon/OnClick/GetStatus/SetStatus/AdviseSink/_RegisterCompartment/_CompartmentCallback 본문 확정.
재확인: `ctx_search(source:"sampleime-langbar-cpp", queries:["GetIcon _pCompartment","_CompartmentCallback OnUpdate","SetStatus isChange"])`.
