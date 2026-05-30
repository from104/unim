# UNIM Windows TSF 결함 5종 — SampleIME + Weasel 패턴 연구 보고서

**조사 대상**: Microsoft SampleIME (Windows-classic-samples / MIT) + RIME Weasel (rime/weasel / GPLv3)
**조사 일자**: 2026-05-28
**대상 UNIM commit**: 87f3ab6 (feat/windows-msi-redesign)

---

## 핵심 출처

| Source 라벨 | URL | 비고 |
|---|---|---|
| SampleIME-Register | `Windows-classic-samples/Samples/IME/cpp/SampleIME/Register.cpp` | 카테고리/프로파일 등록 |
| SampleIME-Main | `.../SampleIME.cpp` | ITfFunctionProvider 구현 |
| SampleIME-CandidateWindow | `.../CandidateWindow.cpp` | 팝업 윈도우 생성 |
| SampleIME-CandidateUI | `.../CandidateListUIPresenter.cpp` | ITfCandidateListUIElement |
| SampleIME-LangBar | `.../LanguageBar.cpp` | TF_LBI_STYLE_SHOWNINTRAY |
| SampleIME-KeyEventSink | `.../KeyEventSink.cpp` | 키 이벤트 흐름 |
| Weasel-Register | `weasel/WeaselTSF/Register.cpp` | 17종 카테고리 + RegisterProfile |
| Weasel-TSFMain | `WeaselTSF/WeaselTSF.cpp` | ActivateEx + 인터페이스 목록 |
| Weasel-Composition | `WeaselTSF/Composition.cpp` | GetTextExt + CUAS workaround |
| Weasel-CandidateList | `WeaselTSF/CandidateList.cpp` | ITfCandidateListUIElementBehavior |
| Weasel-LangBar | `WeaselTSF/LanguageBar.cpp` | LangBar + 설정 진입 |
| Weasel-KeyEventSink | `WeaselTSF/KeyEventSink.cpp` | OnTest/OnKey 분리 |
| Weasel-Compartment | `WeaselTSF/Compartment.cpp` | 키보드 disabled 감지 |

---

## 결함 1 — TSF 등록 키 (입력기 "추가" 목록 노출)

### Microsoft SampleIME 패턴
- **핵심 파일**: `Register.cpp` (`SupportCategories[]` 배열) + `SampleIME.cpp` `DllRegisterServer` 흐름
- **카테고리 8종** (전부 `RegisterCategory`로 등록):
  1. `GUID_TFCAT_TIP_KEYBOARD` — 키보드 IME 분류
  2. `GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER` — 조합 밑줄 색상
  3. `GUID_TFCAT_TIPCAP_UIELEMENTENABLED` — UI Element 모드
  4. `GUID_TFCAT_TIPCAP_SECUREMODE` — 보안 입력 필드 (UAC, 비밀번호)
  5. `GUID_TFCAT_TIPCAP_COMLESS` — COM 미초기화 컨텍스트 (console!)
  6. `GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT` — 한/영 compartment 노출
  7. `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT` — UWP/스토어 앱
  8. `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT` — 트레이 인디케이터
- **등록 흐름**: `ITfInputProcessorProfileMgr::RegisterProfile`(최신 API) 사용, langid + profile GUID + 아이콘 + `bEnable=TRUE`
- **호출 순서**: `DllRegisterServer` → `RegisterServer()` (CLSID 등록) → `RegisterProfiles()` (`RegisterProfile`) → `RegisterCategories()` (`RegisterCategory` 루프)

### Weasel 패턴
- **핵심 파일**: `Register.cpp` `RegisterProfiles()` + `SupportCategories0[]`
- **카테고리 17종**: SampleIME 8종에 추가:
  - `GUID_TFCAT_CATEGORY_OF_TIP` — TIP "카테고리" 자체
  - `GUID_TFCAT_TIPCAP_WOW16` — 16bit 호환
  - `GUID_TFCAT_PROP_AUDIODATA` / `GUID_TFCAT_PROP_INKDATA` — 음성/잉크 속성
  - `GUID_TFCAT_PROPSTYLE_CUSTOM` / `STATIC` / `STATICCOMPACT`
  - `GUID_TFCAT_DISPLAYATTRIBUTEPROPERTY` (DAP 외 별도)
- **UNIM 대비 차이**: Weasel은 langid 4개에 대해 `RegisterProfile` 반복 (hans/hant/Korean/Japanese), 환경변수 `TEXTSERVICE_PROFILE`로 enable 선택. HKL 충돌 회피를 위해 `FindIME(langid)` (레지스트리 `Keyboard Layouts` 스캔) 후 기존 HKL 재사용.

### UNIM 현재 구현의 결함
- `unim-tsf/src/register.rs:112-117` 에서 **카테고리 5종만** 등록: `TIP_KEYBOARD`, `DISPLAYATTRIBUTEPROVIDER`, `UIELEMENTENABLED`, `IMMERSIVESUPPORT`, `SYSTRAYSUPPORT`
- **누락 3종 (중대)**: `SECUREMODE`, `COMLESS`, `INPUTMODECOMPARTMENT`
- `INPUTMODECOMPARTMENT` 누락 → "입력 모드" 항목이 Windows 설정에서 안 보임 → 한국어 키보드 옵션 진입 경로가 끊김
- `COMLESS` 누락 → conhost/wezterm에서 카테고리 매치 실패 → IME가 활성화 안 됨 (**결함 5와 직결**)
- `AddLanguageProfile` 사용 중 — `RegisterProfile` (`InputProcessorProfileMgr`) 로 마이그레이션 권장 (Windows 8+ 표준)

### 권장 적용안
- `unim-tsf/src/register.rs:112` — `for cat in &[...]` 배열에 **8종 카테고리** 추가 (SampleIME 패턴 그대로)
- 동일 파일 `unregister_server`도 8종 동기화
- `installer/wix/unim.wxs` 의 static `RegistryKey` 도 동일 8종으로 동기화 (이중 트랙)
- **선택**: `AddLanguageProfile` → `RegisterProfile` 마이그레이션 (한 번에 langid+icon+enable 한 호출)
- **의존성**: 결함 3·5와 함께 해결해야 함 (INPUTMODECOMPARTMENT/COMLESS 가 두 결함의 근본 원인)

---

## 결함 2 — 트레이 아이콘 부재

### Microsoft SampleIME 패턴
- **standalone tray app 없음**. 오직 `TF_LBI_STYLE_SHOWNINTRAY` 플래그가 붙은 `CLangBarItemButton` 만 사용
- `LangBar.cpp` `GetInfo`: `dwStyle = TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY`
- `GetIcon(phIcon)` 에서 `LoadImage(dllInstance, MAKEINTRESOURCE(idx), IMAGE_ICON, ...)` 로 DLL 리소스 아이콘 동적 로드
- Windows 11 의 작업표시줄 알림 영역에 IME 마크 표시는 **TSF 언어바가 자동 처리** — 별도 NotifyIconData 호출 불필요

### Weasel 패턴
- **별도 `WeaselServer.exe`** 가 standalone tray app으로 동작 (`Shell_NotifyIcon` 직접)
- `WeaselTSF.dll` (TSF 컴포넌트) ↔ `WeaselServer.exe` (트레이 + 후보 렌더) → **Named Pipe / Shared Memory IPC** (`WeaselIPC`)
- LangBar 도 `TF_LBI_STYLE_SHOWNINTRAY` 사용 (이중)
- UNIM 대비 차이: Weasel은 후보 렌더링까지 별도 프로세스에서 하는 **out-of-proc 아키텍처** (메모리 격리 + 보안 모드 회피)

### UNIM 현재 구현의 결함
- `unim-tsf/src/lang_bar.rs` 에 `ITfLangBarItem` 구현 존재하나 `TF_LBI_STYLE_SHOWNINTRAY` 플래그 확인 필요
- `unim-windows` standalone exe 제거되면서 systray가 같이 사라짐 — 즉 이전 구현은 NotifyIconData 직접 호출(Weasel 패턴)이었음
- LangBar만으로는 Windows 11에서 알림 영역 표시 약함 — 사용자 가시성 ↓

### 권장 적용안
- **1순위 (단순)**: `lang_bar.rs` 의 `GetInfo` 에서 `dwStyle |= TF_LBI_STYLE_SHOWNINTRAY` 보장 + 아이콘 리소스 확인. 추가 프로세스 없음.
- **2순위 (강화)**: 별도 `unim-tray.exe` (eframe 없이 가벼운 Win32) 재도입. DBus 대신 **Named Pipe** 또는 Windows 메시지 (`HWND_MESSAGE` broadcast)로 unim-tsf.dll 과 동기.
- **권장**: 우선 1순위 만으로 충분. 사용자가 트레이 메뉴(설정 열기, 한/영 토글)를 강하게 원할 때만 2순위 추가.
- **의존성**: 결함 1(SYSTRAYSUPPORT 카테고리)가 등록되어야 동작 — 이미 UNIM에 있음.

---

## 결함 3 — Windows 설정 통합 (입력기 옵션)

### Microsoft SampleIME 패턴
- `SampleIME.cpp` 가 `ITfFunctionProvider` 직접 구현 (인터페이스 추가 + QueryInterface 분기)
- `GetType` → returns IME CLSID
- `GetFunction(rguid, riid, ppunk)` → `riid == IID_ITfFnConfigure` 시 `CTipFunctionProvider`-스타일 객체 반환 (또는 자기 자신)
- `ITfFnConfigure::Show(hwndParent, langid, guidProfile)` 가 호출되면 모달 다이얼로그 띄움
- 등록: `RegisterCategory` 의 `GUID_TFCAT_TIPCAP_UIELEMENTENABLED` + `INPUTMODECOMPARTMENT` 가 Windows 설정에서 "옵션" 버튼 노출의 트리거

### Weasel 패턴
- LangBar 컨텍스트 메뉴 (`OnMenuSelect`)에서 `ShellExecute(L"open", ...)` 로 외부 설정 도구 launch (`WeaselTSF.dll` 자체는 다이얼로그 없음)
- Windows 설정 통합 지원은 **약함** — 주로 LangBar 메뉴 의존
- `_HandleLangBarMenuSelect(wID)` 가 라우팅

### UNIM 현재 구현의 결함
- `unim-tsf/src/fn_configure.rs` (`UnimFnConfigure` + `UnimFunctionProvider`) **이미 잘 구현돼 있음**
- `text_service.rs:27` 가 `ITfFunctionProvider` 를 implement 목록에 포함
- 그러나 결함 1의 `INPUTMODECOMPARTMENT` 카테고리 누락 → Windows 설정이 "옵션" 버튼을 그리지 않음 → `ITfFnConfigure::Show`가 호출되지 않음
- 즉 **결함 3은 결함 1의 부산물**

### 권장 적용안
- 결함 1의 카테고리 8종 추가가 **즉시 해결**
- `lang_bar.rs` 에 컨텍스트 메뉴 항목 "설정 열기" 추가 (Weasel 패턴) — `OnMenuSelect` 에서 `settings_dialog::show_modal()` 호출
- (확인 필요) Windows 11 의 새 "언어 옵션" UI 가 어떤 metadata key 를 요구하는지 — `LANGBARITEMINFO::szDescription` 외 추가 필드 확인

---

## 결함 4 — 팝업 (Candidate Window) 미표시

### Microsoft SampleIME 패턴
- `CandidateWindow.cpp` `_CreateMainWindow`: `CreateWindowEx(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, WS_BORDER | WS_POPUP, ..., parentWndHandle)`
- 단**, `WS_EX_NOACTIVATE` 가 없음** (포커스 빼앗기 가능)
- `CandidateListUIPresenter.cpp` 가 `BeginUIElement` 호출 → `pUIElementMgr->BeginUIElement(this, &_isShowMode, &_uiElementId)` — 호스트가 `_isShowMode=FALSE` 면 자체 그리지 말라는 신호 (UWP/Modern UI 케이스)
- 위치 계산: `ITfContextView::GetTextExt(ec, range, &rc, &fClipped)` → 실패 시 `MapWindowPoints` 로 좌표 변환

### Weasel 패턴
- `CandidateList.cpp` 가 `ITfCandidateListUIElement` + `ITfCandidateListUIElementBehavior` + `ITfIntegratableCandidateListUIElement` 3종 구현 → 가능하면 호스트 통합 후보창 사용
- `Composition.cpp` `CGetTextExtentEditSession::DoEditSession`:
  ```
  if (GetTextExt(...) == S_OK && (rc.left != 0 || rc.top != 0)) {
      if (_enhancedPosition) { foreground window rect 비교 + 보정 }
  } else {
      // fallback: GetCaretPos / foreground window rect
  }
  ```
- **CUAS workaround**: composition 시작 직후 `pRangeComposition->SetText(ec, TF_ST_CORRECTION, L" ", 1)` (zero-width space) 삽입 — 그래야 `GetTextExt` 가 0,0 이 아닌 실제 좌표 반환 (https://github.com/rime/weasel/pull/883)
- **TextLayoutSink** 등록 → `OnLayoutChange` 에서 후보창 reposition (스크롤/리사이즈 대응)

### UNIM 현재 구현의 결함
- `popup_window.rs:526-530` 윈도우 스타일 **양호**: `WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` (SampleIME보다 우수)
- 추정 결함:
  1. **위치 계산 누락/실패**: `GetTextExt` 직접 호출 부재 또는 (0,0) fallback 미처리 → 화면 좌상단 또는 화면 밖에 그려짐
  2. **CUAS workaround 누락**: composition 직후 zero-width space 안 넣으면 `GetTextExt` 가 (0,0) 반환 → 위치 잘못
  3. **ITfTextLayoutSink 미구현** → 첫 입력 위치만 잡고 이후 스크롤/리사이즈 시 misalign
  4. **ITfCandidateListUIElement 미구현 가능성** → UWP/Modern 앱에서 누락
  5. `ShowWindow` 호출 시 `SW_SHOWNA` (no activate) 사용 여부 확인 필요 — `SW_SHOWNOACTIVATE` 또는 `SW_SHOWNA` 권장
- `popup_window.rs:600,634` 에 `SW_HIDE` 만 보이고 SHOW 경로 확인 필요

### 권장 적용안
- `composition.rs` 에 `CGetTextExtentEditSession` 패턴 도입:
  - `pContext->GetActiveView()` → `pContextView->GetTextExt(ec, range, &rc, &fClipped)`
  - `rc` 가 (0,0) 이면 `GetCaretPos` → `ClientToScreen` fallback
  - `rc` 좌표를 `popup_window` 에 전달
- `composition.rs` 의 composition 시작 직후 `pRangeComposition->SetText(ec, TF_ST_CORRECTION, " ", 1)` (Weasel CUAS workaround) 추가 — inline preedit 모드일 때는 생략
- `text_service.rs` 에 `ITfTextLayoutSink` 등록 + `OnLayoutChange` 에서 재계산 호출
- `popup_window.rs` `ShowWindow` 를 `SW_SHOWNA` 로 (focus 빼앗지 않음)
- **의존성**: 결함 5와 일부 겹침 — GetTextExt 가 콘솔에서 실패하므로 fallback이 두 결함을 동시에 푼다

---

## 결함 5 — 콘솔 (PowerShell / Windows Terminal / wezterm) 입력 이상

### Microsoft SampleIME 패턴
- `Register.cpp` 에 `GUID_TFCAT_TIPCAP_COMLESS` 카테고리 등록 — **conhost가 IME를 활성화하는 필수 조건**
- `KeyEventSink.cpp` `OnTestKeyDown` / `OnKeyDown` 분리 — 콘솔은 `OnTestKeyDown` 없이 바로 `OnKeyDown` 호출하는 경우 있음
- `VKeyFromVKPacketAndWchar` 로 `VK_PACKET` 처리 — 터치 키보드/IME 합성 입력 정규화
- 콘솔용 별도 처리 코드 **없음** — 카테고리 등록만으로 동작

### Weasel 패턴
- 17종 카테고리 모두 등록 (COMLESS + WOW16 + IMMERSIVE 포함)
- `Compartment.cpp` `_IsKeyboardDisabled`:
  - `GUID_COMPARTMENT_KEYBOARD_DISABLED` 와 `GUID_COMPARTMENT_EMPTYCONTEXT` 양쪽 체크
  - **EMPTYCONTEXT 가 TRUE 면 키 안 먹음** (conhost의 readline 모드 케이스)
- `KeyEventSink.cpp` `OnTestKeyDown` 에 `_fTestKeyDownPending` 가드 — "Some apps send multiple OnTestKeyDown for a single key (MS WORD 2010 x64), some send OnKeyDown only (QQ2012)" 주석. 콘솔 호스트도 같은 케이스.
- `TSFMain.cpp` `ActivateEx(dwFlags)` 에서 `_activateFlags = dwFlags` 저장 → 이후 `TF_TMAE_COMLESS` 등 플래그별 분기
- Caps Lock 처리: `Caps_Lock` 키 release 시 한/영 전환 후 `SendInput` 으로 가짜 Caps_Lock 두 번 보내 OS 상태 동기화 (콘솔 한/영 동작 안정화)

### UNIM 현재 구현의 결함
- **`GUID_TFCAT_TIPCAP_COMLESS` 카테고리 미등록** (결함 1) → conhost가 UNIM을 IME로 활성화 시도 자체를 안 함 → 키 입력이 IME 우회
- `key_handler.rs` 에 `OnTestKeyDown` 중복 호출 가드 없을 가능성 → 중복 입력
- `_IsKeyboardDisabled` 의 EMPTYCONTEXT 체크 누락 가능성 → readline 모드에서 조합 깨짐
- conhost 의 IME 메시지 흐름이 일반 GUI 앱과 달라 `ITfThreadMgrEx::ActivateEx` 의 `dwFlags` 처리 누락 가능

### 권장 적용안
- **즉시**: 결함 1의 카테고리 8종 등록 — 특히 `COMLESS`, `IMMERSIVE`, `WOW16`
- `key_handler.rs` 에 OnTestKeyDown / OnKeyDown 분리 및 pending 가드 추가
- `text_service.rs` `_IsKeyboardDisabled` 헬퍼 도입 (Weasel `Compartment.cpp` 패턴) — `GUID_COMPARTMENT_KEYBOARD_DISABLED` + `GUID_COMPARTMENT_EMPTYCONTEXT` 양쪽 VT_I4 체크
- `ActivateEx` 의 `dwFlags` 저장 → 이후 분기에 활용
- Caps Lock 한/영 토글 시 Weasel 의 `SendInput(Caps_Lock x2)` 패턴 검토
- (확인 필요) wezterm 의 TSF 지원 수준 — 일부 빌드는 IMM32-only 호환. 별도 IMM32 호환 처리 검토 필요

---

## 종합 권장사항 (200~300자)

**1순위 (단일 commit 으로 해결 가능 — 결함 1·3·5 동시 해소)**:
`register.rs` 의 카테고리 배열을 SampleIME 8종으로 확장 (`SECUREMODE`/`COMLESS`/`INPUTMODECOMPARTMENT` 추가). 동시에 `unim.wxs` 의 static registry 도 8종 동기화. 이것만으로 conhost 활성화 + Windows 설정 "옵션" 노출 + 보안 모드 동작이 동시 해결.

**2순위 (결함 4 — 별도 commit)**:
`composition.rs` 에 `CGetTextExtentEditSession` 패턴 도입 (`GetTextExt` + (0,0) fallback + Weasel CUAS workaround), `text_service.rs` 에 `ITfTextLayoutSink` 추가. Weasel 의 zero-width-space 트릭은 LGPL 무관한 1줄 알고리즘이라 차용 OK.

**3순위 (결함 2 — 최저 우선순위)**:
`lang_bar.rs` 의 `TF_LBI_STYLE_SHOWNINTRAY` 확인 — 별도 `unim-tray.exe` 재도입은 사용자 피드백 후 결정. Weasel 의 out-of-proc 모델은 GPLv3 + Named Pipe 구조라 UNIM에 부적합.

**라이선스 차용 범위**:
- **SampleIME (MIT)**: 카테고리 배열, 윈도우 스타일 플래그, `RegisterProfile` 호출 시그니처 — **그대로 차용 가능** (MIT 호환).
- **Weasel (GPLv3)**: 알고리즘만 (`GetTextExt` fallback 로직, CUAS zero-width-space, `_IsKeyboardDisabled` 의 EMPTYCONTEXT 체크, OnTestKeyDown pending 가드) — **재구현 필수**, 코드 직접 복사 금지. 패턴/주석 인용으로 PR 메시지에 출처 표기.

**다음 단계**:
planner 에이전트에 위임 권장 — 결함 1 (등록 키 8종 확장) 은 즉시 구현 가능하지만, 결함 4·5는 `composition.rs` / `key_handler.rs` 의 기존 흐름 분석이 선행되어야 하므로 plan 이 필요.
