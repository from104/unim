# 조사 C — MS 한국어 IME(SampleIME 표준) vs UNIM langbar 인디케이터 1:1 대조

> 1차 자료: SampleIME `LanguageBar.cpp` / `CompositionProcessorEngine.cpp`
> (microsoft/Windows-classic-samples, raw GitHub), Weasel `LanguageBar.cpp`.
> UNIM: `unim-tsf/src/lang_bar.rs`(395줄), `compartment.rs`(114줄), `text_service.rs`(450줄),
> `globals.rs`(24줄) — 전부 전체 판독 완료.
> KB source: `research-c-sampleime-langbar`, `sampleime-langbar-cpp`,
> `sampleime-compengine-cpp`, `weasel-langbar-cpp`, `research-c-findings`.

---

## 1. 메서드별 1:1 대조표 (SampleIME CLangBarItemButton vs UNIM UnimLangBarButton)

| 메서드 | SampleIME (1차) | UNIM lang_bar.rs (1차) | 판정 |
|---|---|---|---|
| guidItem | `GUID_LBI_INPUTMODE` (TSF 표준) | `GUID_LBI_INPUTMODE` (windows 크레이트 표준값) | **동일 ✅** |
| dwStyle | `TF_LBI_STYLE_BTN_BUTTON \| SHOWNINTRAY` | `BTN_BUTTON \| SHOWNINTRAY` | **동일 ✅** |
| GetInfo | itemInfo 반환, GetInfo서 SHOWNINTRAY OR | 동일 구조, 표준 GUID | 동일 |
| GetStatus | `*pdwStatus = _status` (포커스 따라 DISABLED 토글) | 항상 `Ok(0)` | 차이(경미, 표시 무해) |
| GetIcon | `return (*phIcon!=NULL)? S_OK : E_FAIL` | NULL HICON이어도 **`Ok(hicon.unwrap_or_default())`** | ⚠ **차이 (#4-1)** |
| GetText | "한"/"A" | "가"/"A" | 동일 |
| AdviseSink | riid 검사→QI→`_pLangBarItemSink` 저장→`*pdwCookie=_cookie(=0)` | riid 검사→cast→sink 저장→`cookie=1` | 동일 패턴 ✅ |
| UnadviseSink | cookie 검사→Release→null | cookie 검사→`None` | 동일 ✅ |
| OnClick/Menu | 모드 토글 | 엔진 직접 토글 후 `state.update()` | 동일 의도 |
| **_RegisterCompartment** | **있음**: `CCompartment`+`CCompartmentEventSink::_Advise(OPENCLOSE)` | **버튼에 없음** (별도 모듈서 SetValue만) | ❌ **차이 (#2/#3)** |
| **_CompartmentCallback** | OPENCLOSE 변경 시 `_pLangBarItemSink->OnUpdate(TF_LBI_STATUS\|TF_LBI_ICON)` | **없음** | ❌ **누락 (#3)** |

---

## 2. 질문별 확정 답변 (1차 근거)

**#1 UI-less / 추가 인터페이스 필요?** — 불필요. SampleIME는 입력모드 표시에
`ITfFnSearchCandidateProvider`나 UI-less mode를 쓰지 않는다. 표시는 전적으로
**langbar item(GUID_LBI_INPUTMODE) + GUID_COMPARTMENT_KEYBOARD_OPENCLOSE compartment**로 이뤄진다.
UNIM에 없는 인터페이스는 없다 — 차이는 인터페이스가 아니라 **compartment 양방향 배선**.

**#2 langbar item 개수 & 입력모드 item이 advise하는 compartment** (확정):
SampleIME `SetupLanguageBar`는 IMEMode + DoubleSingleByte + Punctuation 3개 item을 만든다.
입력모드 item 생성·등록:
```
CreateLanguageBarButton(dwEnable, GUID_LBI_INPUTMODE, ..., ImeModeOnIcoIndex, ImeModeOffIcoIndex, &_pLanguageBar_IMEMode, ...);
InitLanguageBar(_pLanguageBar_IMEMode, pThreadMgr, tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
```
`InitLanguageBar`는 `_AddItem()` 성공 후 `_RegisterCompartment(..., GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)`를 호출한다.
→ **입력모드 item은 반드시 `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE`를 advise**한다. (DoubleSingleByte/Punctuation은
한/영 표시와 무관한 별도 compartment.) 한/영 표시에 **필수인 item은 IMEMode 1개**.

**#3 sink advise 보장 & OnUpdate 호출 주체** (핵심 차이):
- AdviseSink 자체: UNIM 정상(sink를 `LangBarState.sink`에 저장, cookie=1). SampleIME는 cookie=0.
  cookie 값은 자유이나 Unadvise와 일관되면 OK → UNIM 정상.
- **OnUpdate 호출 주체가 다르다.**
  - SampleIME: OS가 OPENCLOSE compartment를 바꾸면 → `_CompartmentEventSink` 콜백 →
    `_pLangBarItemSink->OnUpdate(TF_LBI_STATUS|TF_LBI_ICON)`. **OS 주도, 버튼이 직접 compartment를 구독.**
  - UNIM: `LangBarState::update(is_korean)`가 **앱 주도**로 (a) `sink.OnUpdate(TF_LBI_STATUS|ICON|TEXT)`를 쏘고
    (b) `compartment::sync_keyboard_mode()`로 OPENCLOSE에 **SetValue(쓰기 전용)**.
    `compartment.rs`는 `Advise`/`ITfCompartmentEventSink`가 **0건**(쓰기 전용 모듈, 헤더 주석에 "쓰기 전용" 명시).
  → UNIM은 **자기 토글 시점엔 OnUpdate를 직접 쏘므로 갱신은 동작**한다. 다만 OS/타 IME가
    compartment를 바꾸는 역방향 통지 경로가 없다(설계상 의도). 트레이 미표시가 sink 미발사 때문은 **아닐 가능성**이 높다.

**#4 GetStatus(DISABLED)**: SampleIME도 기본 `_status=0`(enabled). 포커스 없을 때만 `SetLanguageBarStatus(TF_LBI_STATUS_DISABLED, TRUE)`.
UNIM의 상시 0은 표시 차단 원인 아님. (다만 UNIM엔 OnSetFocus 기반 DISABLED 토글 로직 미확인.)

**#4-1 GetIcon HICON 유효성** (확정 차이):
SampleIME/Weasel 모두 `return (*phIcon == NULL) ? E_FAIL : S_OK;`.
UNIM `GetIcon`은 `create_status_icon()`이 NULL을 주면 `unwrap_or_default()`로 **NULL HICON + Ok(S_OK)** 반환.
TSF가 "S_OK인데 NULL HICON"을 받으면 트레이에 빈/깨진 아이콘 → 표시 실패·깜빡임 가능.
→ **수정 후보 1순위(코드 레벨 명확한 규약 위반).** NULL이면 `Err(E_FAIL)` 반환해야 SampleIME 규약 부합.

**#5 레지스트리**: langbar 표시는 런타임 item+compartment로 결정. 1차 소스에서 별도 langbar 레지스트리 키 의존 미확인.

---

## 3. UNIM이 빠뜨린/어긋난 것 — 우선순위 (1차 근거)

1. **[1순위·확정] GetIcon이 NULL HICON에도 S_OK 반환** (lang_bar.rs:321-329).
   SampleIME/Weasel 규약은 NULL→E_FAIL. GDI 아이콘 생성 실패 시 OS가 빈 아이콘을 받아 트레이 표시 실패.
   → `create_status_icon()` 실패 시 `Err(E_FAIL.into())` 반환으로 수정.
2. **[2순위·구조] 입력모드 버튼이 compartment를 _Advise하지 않음 (단방향)**.
   SampleIME는 버튼이 OPENCLOSE를 구독→OS 주도 OnUpdate. UNIM은 SetValue 쓰기 전용
   (compartment.rs Advise=0). 자기 토글 표시엔 무해하나, OS/시스템 한영키·타 경로 변경이
   트레이에 반영 안 됨. 양방향 구독(ITfCompartmentEventSink) 추가 검토.
3. **[확인됨·정상] ActivateEx 배선** — text_service.rs:112-153 에서
   `thread_mgr.cast::<ITfLangBarItemMgr>()` → `set_tsf(thread_mgr, tid)` →
   `UnimLangBarButton::new(...)` → `lbmgr.AddItem(&btn_item)` → `sync_keyboard_mode(...)` 호출.
   Deactivate(:191-192)에서 `RemoveItem`. **AddItem은 정상 호출됨 → 버튼 등록 자체는 문제 아님.**
   단 `AddItem` 결과를 `let _ =`로 버려 실패 시 침묵(트레이 미표시 시 진단 어려움).
4. **[경미] GetStatus 상시 0** — DISABLED 토글(OnSetFocus 연동) 없음. text_service.rs:209 OnSetFocus는
   config reload만 하고 SetLanguageBarStatus 미호출. 표시 차단 원인 아님(SampleIME도 기본 enabled).

---

## 4. 미확보 자료
- SampleIME `Compartment.cpp`의 `CCompartmentEventSink::_Advise` 본문(콜백 등록 세부) — 미인덱싱.
- MS 한국어 IME({A028AE76}) 실제 런타임 레지스트리/compartment 덤프 — 정적 분석으론 불가.
- 트레이 실제 미표시 재현 로그(dbg_log의 OPENCLOSE/CONVERSION HRESULT, AddItem 결과) — 런타임 필요.
