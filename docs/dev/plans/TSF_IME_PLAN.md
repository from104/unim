# UNIM Windows TSF IME (시스템 전역) 구현 계획

## Context

UNIM Windows 프론트엔드 Phase 0~2가 완료되었습니다:
- 코어 엔진 크로스 플랫폼 빌드 (`build.rs`, `Cargo.toml` 조건부 의존성)
- Win32 VK 키코드 매핑 (`from_win32_vk()`, `from_win32_modifiers()`)
- egui 독립 GUI 앱 (`unim-windows/`)
- `cargo check --target x86_64-pc-windows-gnu` 통과 확인

이제 **Phase 4: TSF IME DLL**을 구현하여 Windows 시스템 전역에서 모든 앱에서 한글 입력을 가능하게 합니다.

---

## TSF (Text Services Framework) 아키텍처

### 키 이벤트 흐름
```
[사용자 키 입력] → [WM_KEYDOWN] → [TSF Manager (msctf.dll)]
    ↓
[ITfKeyEventSink::OnTestKeyDown] → 소비 여부 판단 (TRUE/FALSE)
    ↓ (TRUE인 경우)
[ITfKeyEventSink::OnKeyDown] → VK → KeyCode 변환 → engine.press_key()
    ↓
[ITfContext::RequestEditSession] → TSF가 잠금 허가
    ↓
[ITfEditSession::DoEditSession]
    ├── StartComposition / SetText (preedit 표시)
    ├── EndComposition (commit 확정)
    └── Display Attributes 적용 (밑줄)
    ↓
[앱에 조합 문자열/확정 문자열 표시]
```

### 조합(Composition) 생명주기
```
'ㅎ' 입력 → StartComposition("ㅎ") + 밑줄 표시
'ㅏ' 입력 → SetText("하") 조합 갱신
'ㄴ' 입력 → SetText("한") 또는 EndComposition("하") + StartComposition("ㄴ")
Space    → EndComposition("한") 확정 + 키 통과
```

---

## 필수 COM 인터페이스 (10개)

| 인터페이스 | 역할 | 파일 |
|-----------|------|------|
| `ITfTextInputProcessorEx` | IME 활성화/비활성화 생명주기 | `text_service.rs` |
| `ITfKeyEventSink` | 키 이벤트 가로채기 (OnTestKeyDown/OnKeyDown) | `key_handler.rs` |
| `ITfCompositionSink` | 조합 외부 종료 콜백 | `composition.rs` |
| `ITfThreadMgrEventSink` | 포커스 추적 (문서/컨텍스트 변경) | `text_service.rs` |
| `ITfTextEditSink` | 텍스트 편집 알림 | `text_service.rs` |
| `ITfDisplayAttributeProvider` | preedit 밑줄/색상 스타일링 | `display_attr.rs` |
| `ITfActiveLanguageProfileNotifySink` | 언어 프로필 변경 알림 | `text_service.rs` |
| `ITfThreadFocusSink` | 스레드 포커스 변경 | `text_service.rs` |
| `ITfLangBarItemButton` | 언어 바 한/영 토글 버튼 | `lang_bar.rs` |
| `IClassFactory` | COM 객체 생성 팩토리 | `class_factory.rs` |

---

## DLL 진입점 (5개 필수 export)

```rust
// lib.rs
#[no_mangle] extern "system" fn DllMain(hinst, reason, _) -> BOOL
#[no_mangle] extern "system" fn DllGetClassObject(rclsid, riid, ppv) -> HRESULT
#[no_mangle] extern "system" fn DllCanUnloadNow() -> HRESULT
#[no_mangle] extern "system" fn DllRegisterServer() -> HRESULT
#[no_mangle] extern "system" fn DllUnregisterServer() -> HRESULT
```

---

## 크레이트 구조: `unim-tsf/`

```
unim-tsf/
├── Cargo.toml              # crate-type = ["cdylib"], windows 크레이트 의존
├── src/
│   ├── lib.rs              # DllMain, DllGetClassObject, DllCanUnloadNow
│   ├── register.rs         # DllRegisterServer/Unregister — COM + TSF 프로필 등록
│   ├── class_factory.rs    # IClassFactory 구현
│   ├── text_service.rs     # #[implement(...)] 메인 구조체 — 10개 인터페이스
│   ├── key_handler.rs      # OnTestKeyDown/OnKeyDown → VK → KeyCode → engine
│   ├── composition.rs      # EditSession, StartComposition/SetText/EndComposition
│   ├── display_attr.rs     # ITfDisplayAttributeProvider — preedit 밑줄
│   ├── lang_bar.rs         # ITfLangBarItemButton — 한/영 모드 표시
│   ├── candidate_ui.rs     # ITfCandidateListUIElement — 한자/특수문자 팝업
│   └── globals.rs          # GUID 정의, 전역 상수 (CLSID, 프로필 GUID)
```

### Cargo.toml
```toml
[package]
name = "unim-tsf"
version.workspace = true
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
unim = { path = ".." }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "implement",
    "Win32_Foundation",
    "Win32_System_Com",
    "Win32_System_Ole",
    "Win32_System_LibraryLoader",
    "Win32_System_Registry",
    "Win32_UI_TextServices",
    "Win32_Graphics_Gdi",
] }
```

---

## 구현 단계

### Phase 4-1: DLL 스켈레톤 + COM 등록

**파일:** `lib.rs`, `globals.rs`, `class_factory.rs`, `register.rs`

1. GUID 정의 (CLSID, 프로필 GUID, Display Attribute GUID)
2. `DllMain` — HMODULE 저장
3. `DllGetClassObject` — `IClassFactory` 반환
4. `DllCanUnloadNow` — 참조 카운트 확인
5. `DllRegisterServer`:
   - COM InProcServer32 레지스트리 등록 (`HKLM\SOFTWARE\Classes\CLSID\{guid}`)
   - `ITfInputProcessorProfiles::Register(CLSID)`
   - `ITfInputProcessorProfiles::AddLanguageProfile(CLSID, 0x0412, ...)`  (Korean)
   - `ITfCategoryMgr::RegisterCategory` 7개 카테고리:
     - `GUID_TFCAT_TIP_KEYBOARD` (키보드 입력기)
     - `GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`
     - `GUID_TFCAT_TIPCAP_UIELEMENTENABLED` (UILess 모드)
     - `GUID_TFCAT_TIPCAP_COMLESS`
     - `GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT`
     - `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT` (UWP 앱)
     - `GUID_TFCAT_TIPCAP_SYSTRAKSUPPORT` (시스템 트레이)
6. `DllUnregisterServer` — 역순 제거

### Phase 4-2: 핵심 텍스트 서비스 구현

**파일:** `text_service.rs`

```rust
#[windows::core::implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfTextEditSink,
    ITfDisplayAttributeProvider,
    ITfActiveLanguageProfileNotifySink,
    ITfThreadFocusSink,
)]
struct UnimTextService {
    thread_mgr: Mutex<Option<ITfThreadMgr>>,
    client_id: AtomicU32,
    engine: Mutex<InputEngine>,
    config: Mutex<Config>,
    composition: Mutex<Option<ITfComposition>>,
    // lang_bar, candidate_ui 등
}
```

- `ActivateEx`: ITfThreadMgr 저장, 이벤트 싱크 등록, 언어 바 추가
- `Deactivate`: 모든 싱크/언어 바 해제
- `OnSetFocus`: 문서 포커스 변경 처리

**스레드 안전성:** InputEngine은 Send/Sync가 아니므로 `Mutex<InputEngine>` 사용.
TSF는 Apartment threading model (STA)이므로 실제로는 단일 스레드에서 호출됨.

### Phase 4-3: 키 이벤트 처리

**파일:** `key_handler.rs`

```rust
impl ITfKeyEventSink_Impl for UnimTextService_Impl {
    fn OnTestKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, ...) {
        let vk = wparam.0 as u16;
        let keycode = KeyCode::from_win32_vk(vk);  // Phase 1에서 구현 완료
        // 한글 조합 키 → TRUE, 통과 키 (Ctrl+C 등) → FALSE
    }
    fn OnKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, ...) {
        let vk = wparam.0 as u16;
        let keycode = KeyCode::from_win32_vk(vk);
        let modifiers = get_win32_modifier_state();  // GetKeyState() 호출
        let result = engine.press_key(keycode, modifiers, &config);
        // result에 따라 EditSession 요청
    }
}
```

키 분류 로직:
- 알파벳/숫자/기호 (Korean 모드) → 소비 (TRUE)
- Space, Enter, Backspace (조합 중) → 소비
- 한/영 전환 키 (RightAlt, VK_HANGUL) → 소비
- F9 / VK_HANJA → 소비 (한자 팝업)
- Ctrl/Alt 조합 → 통과 (FALSE)
- 영문 모드 일반 키 → 통과

### Phase 4-4: 조합 관리 (EditSession)

**파일:** `composition.rs`

```rust
#[implement(ITfEditSession)]
struct UnimEditSession {
    context: ITfContext,
    action: EditAction,
    service: /* back-reference to UnimTextService */,
}

enum EditAction {
    StartComposition { text: String },
    UpdateComposition { text: String },
    EndComposition { commit: String },
    CommitAndStartNew { commit: String, new_preedit: String },
}
```

`DoEditSession` 구현:
1. `StartComposition`: `InsertTextAtSelection` → `StartComposition` → `SetCompositionDisplayAttributes`
2. `UpdateComposition`: `GetRange` → `SetText` (기존 조합 텍스트 교체)
3. `EndComposition`: `EndComposition` (텍스트 확정)
4. `CommitAndStartNew`: End → Insert committed → Start new

### Phase 4-5: Display Attributes (밑줄)

**파일:** `display_attr.rs`

```rust
// preedit 스타일: 파란색 밑줄
TF_DISPLAYATTRIBUTE {
    crText: TF_DA_COLOR { type_: TF_CT_NONE },     // 기본 텍스트 색상
    crBk: TF_DA_COLOR { type_: TF_CT_NONE },       // 기본 배경
    crLine: TF_DA_COLOR { type_: TF_CT_COLORREF, cr: RGB(0, 100, 200) },
    lsStyle: TF_LS_SOLID,                           // 실선 밑줄
    fBoldLine: FALSE,
    bAttr: TF_ATTR_INPUT,
}
```

### Phase 4-6: 언어 바 (한/영 표시)

**파일:** `lang_bar.rs`

- `ITfLangBarItemButton` 구현
- 한글 모드: "가" 아이콘 + "한국어 입력" 툴팁
- 영문 모드: "A" 아이콘 + "English Input" 툴팁
- 클릭 시 모드 토글
- `TF_LBI_STYLE_BTN_TOGGLE` 스타일

### Phase 4-7: 한자/특수문자 후보 창

**파일:** `candidate_ui.rs`

**하이브리드 방식** (Microsoft SampleIME, Rime Weasel 참고):
1. `ITfCandidateListUIElement` 구현 → UILess 모드 앱 지원
2. `ITfUIElementMgr::BeginUIElement` → `bShow`가 TRUE면 자체 HWND 팝업 생성
3. `bShow`가 FALSE면 앱이 직접 렌더링 (UWP/검색창 통합)

팝업 데이터: `engine.take_popup_action()` → `PopupAction::ShowHanja` / `ShowSpecial`

### Phase 4-8: 자동/수동 한영타 오타 교정

- **자동 교정**: `engine.press_key()` 후 commit/preedit에 교정 결과 포함
- **수동 변환**: `engine.typefix_convert(direction)` 호출
  - Surrounding text API: `ITfContext` → `GetSelection` → `GetText` → `engine.set_surrounding_text()`
  - 단축키 (Ctrl+Shift+Space) → PreservedKey로 등록

---

## 참고 프로젝트

| 프로젝트 | 언어 | 설명 |
|---------|------|------|
| [akaza-ime](https://github.com/akaza-im/akaza-ime) | **Rust** | 일본어 TSF IME — 가장 유사한 구조 |
| [ime-rs](https://github.com/saschanaz/ime-rs) | Rust/C++ | MS SampleIME의 Rust 포팅 |
| [MS SampleIME](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME) | C++ | 공식 TSF IME 예제 (10개 인터페이스 구현) |
| [saenaru](https://github.com/wkpark/saenaru) | C | 오픈소스 한국어 IME |
| [Google Mozc](https://github.com/google/mozc) | C++ | TSF 등록/카테고리 참고 |

---

## 수정/생성 대상 파일

| 파일 | 작업 | Phase |
|------|------|-------|
| `Cargo.toml` (root) | workspace에 `unim-tsf` 추가 | 4-1 |
| `unim-tsf/Cargo.toml` | **신규** — cdylib, windows 크레이트 | 4-1 |
| `unim-tsf/src/globals.rs` | **신규** — GUID, 상수 정의 | 4-1 |
| `unim-tsf/src/lib.rs` | **신규** — DLL 진입점 | 4-1 |
| `unim-tsf/src/class_factory.rs` | **신규** — IClassFactory | 4-1 |
| `unim-tsf/src/register.rs` | **신규** — COM/TSF 등록 | 4-1 |
| `unim-tsf/src/text_service.rs` | **신규** — 메인 구조체 + 10개 인터페이스 | 4-2 |
| `unim-tsf/src/key_handler.rs` | **신규** — 키 이벤트 → 엔진 | 4-3 |
| `unim-tsf/src/composition.rs` | **신규** — EditSession, 조합 관리 | 4-4 |
| `unim-tsf/src/display_attr.rs` | **신규** — preedit 밑줄 스타일 | 4-5 |
| `unim-tsf/src/lang_bar.rs` | **신규** — 한/영 모드 표시 | 4-6 |
| `unim-tsf/src/candidate_ui.rs` | **신규** — 한자 팝업 (하이브리드) | 4-7 |

---

## 엔진 API 연동 요약

TSF DLL은 unim 크레이트를 직접 링크합니다 (C FFI 불필요).

### 핵심 API
```rust
use unim::config::Config;
use unim::input_engine::{InputEngine, InputResult, PopupAction};
use unim::keycode::{KeyCode, ModifierState};

// 엔진 생성
let config = Config::load_from_default_path();
let mut engine = InputEngine::new(&config);

// 키 처리
let keycode = KeyCode::from_win32_vk(vk_code);
let modifiers = ModifierState::from_win32_modifiers(mask);
let result: InputResult = engine.press_key(keycode, modifiers, &config);

// 결과 확인
if result.commit_changed {
    let commit = engine.commit_str().to_string();
    engine.clear_commit();
    // → EndComposition(commit)
}
if result.preedit_changed {
    let preedit = engine.preedit_str().to_string();
    // → UpdateComposition(preedit) 또는 StartComposition(preedit)
}

// 팝업 (한자/특수문자)
if let Some(action) = engine.take_popup_action() {
    match action {
        PopupAction::ShowHanja { target, candidates } => { /* 후보 창 표시 */ }
        PopupAction::ShowSpecial { target, characters, top_row } => { /* 특수문자 창 */ }
        PopupAction::HidePopup => { /* 팝업 숨김 */ }
        PopupAction::PopupNavigate { .. } => { /* 선택 업데이트 */ }
    }
}

// 모드 전환
let is_korean = engine.input_category() == InputCategory::Korean;

// 수동 한영타 변환
engine.set_surrounding_text(text, cursor, anchor);
if let Some((delete_count, replacement)) = engine.typefix_convert(0) {
    // 선택 영역 교체
}
```

### 스레드 안전성
- `InputEngine`은 `Send`/`Sync`가 아님
- TSF는 STA (Apartment Threading) → 단일 스레드에서 호출
- 안전을 위해 `Mutex<InputEngine>` 래핑 권장
- `Config`는 읽기 전용 후 `Arc<Config>` 공유 가능

### UnimStr 수명 주의
- `engine.commit_str()`, `engine.preedit_str()`는 엔진 내부 버퍼 참조
- 다음 `press_key()` 호출 전에 반드시 `.to_string()`으로 복사

---

## 빌드 및 설치

```bash
# Windows에서 빌드
cargo build --release -p unim-tsf

# 등록 (관리자 권한)
regsvr32 target\release\unim_tsf.dll

# 해제
regsvr32 /u target\release\unim_tsf.dll
```

등록 후 Windows 설정 > 시간 및 언어 > 언어 > 키보드에 "UNIM Korean IME" 표시됨.

---

## 검증 방법

1. **DLL 등록**: `regsvr32` 성공 + Windows 설정에 표시
2. **메모장 테스트**: 한글 입력 → preedit 밑줄 → Space로 확정
3. **한/영 전환**: RightAlt → 언어 바 아이콘 변경
4. **한자 변환**: F9 → 후보 창 표시 → 선택 → 삽입
5. **수동 한영타 변환**: 텍스트 선택 → Ctrl+Shift+Space
6. **UWP 앱**: Edge, 설정 등에서 동작 확인
7. **안정성**: Chrome, VS Code, Office 등에서 크래시 없이 동작

---

## 핵심 기술 결정

| 결정 | 선택 | 이유 |
|------|------|------|
| 엔진 연동 | Rust 직접 링크 (unim crate) | C FFI보다 PopupAction 등 전체 API 접근 가능 |
| 스레딩 | `Mutex<InputEngine>` | STA 모델이지만 안전성 보장 |
| 후보 창 | 하이브리드 (UIElement + HWND) | UILess + 데스크탑 모두 지원 |
| 등록 | DllRegisterServer 내장 | 별도 인스톨러 없이 regsvr32로 가능 |
