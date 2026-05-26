# UNIM `unim-popup-service` 프로세스 아키텍처

**참조 리서치**: [`docs/research/popup-redesign.md`](../research/popup-redesign.md)
**브랜치**: `arch/popup-unify`
**범위**: popup 전용 standalone 프로세스, GTK4-rs 단일 toolkit, X11/Wayland 둘 다 1차 지원

> **구현 현황 (0.3.0, 2026-05)**: 이 문서는 설계 시점의 계획서다. 실제 구현은 본 계획을
> 대부분 따랐으나 아래 항목이 달라졌으니 코드를 기준으로 읽을 것.
>
> - **팝업 중앙화 완료**: 한자·특수문자·이모지 팝업은 전부 `unim-popup-service`(독립 GTK4
>   프로세스)가 렌더한다. GTK/Qt IM 모듈은 자체 팝업 위젯을 더 이상 갖지 않고 DBus 위임만
>   하며, 과거의 임베디드/로컬 팝업 위젯 코드는 제거됐다.
> - **DBus 인터페이스**: 실제 bus name·object path·시그널 이름은 §4가 아니라
>   [`docs/dev/specs/POPUP_SPEC.md`](../dev/specs/POPUP_SPEC.md)(`org.atit.unim.Popup`
>   인터페이스, `PopupRender` 통합 view-model SoT)와 코드가 정본이다.
> - **트레이 분리**: 트레이는 `unim-gui-gtk`가 아니라 별도 `unim-indicator` 프로세스가
>   책임진다(§7·§8의 "popup-service가 트레이도 담당"·"unim-gui-gtk 트레이 이관" 서술은 폐기).
> - **GNOME Wayland 분기**: GNOME Shell + Wayland에서는 popup-service GTK4 대신 GNOME
>   extension의 `popup_view.js`(St 위젯)가 직접 렌더한다(§1 분리도의 "gnome-extension 자체
>   popup"이 이 경로). popup-service는 D-Bus auto-activation으로 기동된다(autostart .desktop 폐기).
> - **4개 GUI 앱 고유 아이콘**: indicator/settings/keymap-studio/typing-practice 4개 GUI 앱이
>   각자 고유 아이콘을 가지며 `io.github.from104.unim.{Indicator,Settings,KeymapStudio,TypingPractice}.svg`
>   로 hicolor에 설치된다. 트레이 한/영 상태 아이콘(`unim-korean`/`unim-english`)은 별개로 유지된다.

---

## 1. 프로세스 분리도

```
┌────────────────────────────────────────────────────────────────┐
│ Desktop Session (X11 또는 KDE Plasma 6 Wayland 또는 wlroots)    │
│                                                                 │
│   ┌──────────────┐    DBus     ┌────────────────────────────┐  │
│   │ unim-daemon  │◄──signal───►│  unim-popup-service        │  │
│   │ (engine)     │             │  • popup window (GTK4)     │  │
│   └──────┬───────┘             │  • tray (ksni)             │  │
│          │                      │  • X11/Wayland backend     │  │
│          │ DBus method          └────────────────────────────┘  │
│          │                                                       │
│   ┌──────┴───────────────┐                                       │
│   │ unim-frontends/{xim,│                                        │
│   │ wayland,gtk,qt,gnome│                                        │
│   │ -extension}         │                                        │
│   └─────────────────────┘                                        │
│                                                                  │
│   ┌──────────────────────┐                                       │
│   │ unim-gui-gtk         │  (settings GUI 전용, popup 책임 없음) │
│   │ • SettingsWindow     │                                       │
│   └──────────────────────┘                                       │
│                                                                  │
│   ┌──────────────────────┐                                       │
│   │ unim-gnome-extension │  (GNOME Shell 전용, 자체 popup)      │
│   │ • indicator + popup  │                                       │
│   └──────────────────────┘                                       │
└────────────────────────────────────────────────────────────────┘
```

**유지**: daemon, IM frontends, gnome-extension, settings (`unim-gui-gtk` settings 부분만)
**신규**: `unim-popup-service` 단일 popup + tray 프로세스
**폐기**: `unim-gui-qt` 전체

---

## 2. 책임 (Responsibilities)

### unim-popup-service
- **단일 책임**: daemon의 popup ViewModel을 받아 GTK4 window로 렌더링 + 트레이 인디케이터
- 환경 자동 검출 (X11/Wayland) 후 적절한 backend 사용
- 사용자 클릭 → daemon에 select/cancel DBus method 발사
- 외부 클릭 dismiss는 backend별 메커니즘 사용

### unim-gui-gtk (변경 후)
- popup 모듈 전부 제거
- `SettingsWindow` 다이얼로그만 유지
- 트레이 코드도 제거 (popup-service로 이관)
- 별도 `unim-gui-gtk-settings` 바이너리 또는 기존 이름 유지

### unim-gui-qt
- **전체 폐기** (debian packaging에서도 제거)

---

## 3. 신규 crate 구조: `unim-popup-service`

```
unim-popup-service/
├── Cargo.toml
├── locales/                       # 기존 unim-gui-gtk locales 공유 또는 복사
│   ├── ko.yml
│   └── en.yml
├── src/
│   ├── main.rs                    # 진입점 + 단일 인스턴스 lock + 환경 detect
│   ├── lib.rs                     # 모듈 노출
│   ├── app.rs                     # GTK4 Application, dbus subscriber, state holder
│   │
│   ├── popup/                     # popup window 본체
│   │   ├── mod.rs                 # PopupManager (kind별 dispatch)
│   │   ├── hanja.rs               # ← unim-gui-gtk/src/hanja_popup.rs 이관
│   │   ├── special.rs             # ← unim-gui-gtk/src/special_popup.rs 이관
│   │   ├── emoji.rs               # ← unim-gui-gtk/src/emoji_popup.rs 이관
│   │   └── styles.rs              # popup_styles.generated.css 적용
│   │
│   ├── backend/                   # 컴포지터별 분기
│   │   ├── mod.rs                 # trait Backend { fn show, fn position, fn install_outside_click_handler }
│   │   ├── x11.rs                 # ← popup_positioning.rs x11_* 이관 + 트레이/override-redirect
│   │   ├── wayland_standalone.rs  # 1차: xdg_toplevel + 자체 폴링
│   │   └── wayland_input_popup.rs # 2차: zwp_input_popup_surface_v2 직접 호출
│   │
│   ├── tray.rs                    # ← unim-gui-common::tray + unim-gui-gtk::main 의 트레이 부분 이관
│   ├── dbus_listen.rs             # daemon signal subscriber (현 unim-gui-common::dbus_client 재사용)
│   └── single_instance.rs         # flock 단일 인스턴스 (unim-gui-qt::main 패턴 이식)
│
└── debian/                        # 패키징 메타 (debian/ 별도 폴더)
```

### 의존성 (Cargo.toml)
```toml
[dependencies]
unim = { path = ".." }                    # engine 공통 타입
unim-dbus = { path = "../unim-dbus" }     # DBus 인터페이스
unim-gui-common = { path = "../unim-gui-common" }  # PopupModel, popup_dbus, popup_position, tray
gtk4 = { version = "0.7", package = "gtk4", features = ["v4_10"] }
gdk4 = "0.7"
gdk4-wayland = { version = "0.7", optional = true }
gdk4-x11   = { version = "0.7", optional = true }
ksni = "0.3"                              # 트레이
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
zbus = { version = "4", default-features = false, features = ["tokio"] }
xcb = { version = "1.7", optional = true } # X11 outside-click
wayland-client = { version = "0.31", optional = true }
wayland-protocols = { version = "0.31", features = ["unstable"], optional = true }
rust-i18n = "3"
libc = "0.2"                              # flock
serde_json = "1"
```

### feature flags
- `default = ["x11", "wayland"]` — 양쪽 다 컴파일
- `x11 = ["xcb", "gdk4-x11"]`
- `wayland = ["wayland-client", "wayland-protocols", "gdk4-wayland"]`

---

## 4. DBus 인터페이스 (변경 없음)

기존 `unim-gui-common::popup_dbus`가 정의한 daemon ↔ client RPC 그대로 사용:
- daemon → client signal: `ShowHanjaPopup`, `ShowSpecialPopup`, `ShowEmojiPopupV2`, `HidePopup`, `PopupRender`, `PopupNavigate`, `HanjaBookmarkChanged`, `HanjaCandidatesReordered`
- client → daemon method: `SelectHanja`, `CancelHanja`, `SelectSpecial`, `CancelSpecial`, `SetEmojiCategory`, `CommitEmoji`, `ToggleHanjaExpanded`, `ChangePopupPage`, `ToggleHanjaBookmark`

**bus name**: `org.atit.unim.PopupService` (신규)
**object path**: `/org/atit/unim/PopupService`
**`RegisterFrontend("popup-service")`** 호출로 daemon에 자기 등록

---

## 5. Backend trait

```rust
pub trait PopupBackend: Send + Sync {
    /// 환경 진단 — 사용 가능 여부.
    fn available() -> bool;

    /// popup 표시 (cursor anchor 좌표 + 사이즈).
    fn show(&mut self, window: &gtk4::Window, anchor: AnchorRect, size: Size);

    /// popup 숨김.
    fn hide(&mut self, window: &gtk4::Window);

    /// outside click dismiss 핸들러 설치.
    fn install_outside_click_dismiss<F: Fn() + 'static>(&mut self, window: &gtk4::Window, on_dismiss: F);
}

pub enum BackendKind {
    X11,
    WaylandInputPopup,        // KWin / wlroots — input_popup_surface_v2
    WaylandStandalone,        // fallback xdg_toplevel
    Unsupported,
}
```

### 환경 검출 로직 (`main.rs` 시작 시)
```rust
fn detect_backend() -> BackendKind {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // Wayland 환경
        if x11::is_available() && std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11") {
            // X11 세션이지만 WAYLAND_DISPLAY가 잘못 set된 경우 — drop
        }
        // input_popup_surface_v2 지원 확인 (compositor capability query)
        if wayland_input_popup::is_supported() {
            return BackendKind::WaylandInputPopup;
        }
        return BackendKind::WaylandStandalone;
    }
    if x11::is_available() {
        return BackendKind::X11;
    }
    BackendKind::Unsupported
}
```

---

## 6. 라이프사이클

### 시작
```
1. systemd --user service 또는 autostart .desktop
2. flock으로 단일 인스턴스 검증
3. GTK4 Application 초기화
4. detect_backend()
5. daemon DBus signal 구독 시작
6. RegisterFrontend("popup-service") 호출
7. ksni 트레이 시작
8. GTK 메인 루프
```

### popup 표시 흐름
```
daemon → ShowHanjaPopup signal
  → popup-service::dbus_listen 수신
  → PopupManager::show(kind=Hanja, payload)
  → 적절한 backend::show() 호출
  → hanja_popup.rs::render
  → window.present()
  → backend::install_outside_click_dismiss(window, || cancel_hanja())
```

### 종료
```
daemon이 사라지거나 사용자가 트레이 → 종료
  → UnregisterFrontend 호출
  → GTK Application::quit
  → flock 해제
```

---

## 7. 인디케이터(트레이) 정책

- `unim-gui-common::tray::TrayController` 그대로 재사용 (ksni 기반)
- KStatusNotifierItem 프로토콜 호환 → KDE Plasma 6에서 자동 표시
- GTK 환경/Cinnamon/Mate 등에서도 동작 (StatusNotifier-Watcher 있는 경우)
- AyatanaIndicator/SNI 양쪽 지원 (ksni 0.3 기본 동작)

---

## 8. settings GUI 분리 정책

- `unim-gui-gtk` crate는 **settings GUI 전용**으로 축소
- 바이너리 이름 유지 (`unim-gui-gtk`) 또는 `unim-gui-settings`로 rename — 결정: 유지 (debian/ 안정성)
- popup·tray 코드는 popup-service로 이관 후 제거
- DBus method `unim-gui-gtk --settings` 호출 시 settings 다이얼로그만 띄움

---

## 9. unim-gui-qt 제거 계획

| 항목 | 처리 |
|---|---|
| Cargo workspace 멤버 | `unim-gui-qt` 제거 |
| 소스 트리 | `unim-gui-qt/` 전체 디렉토리 삭제 |
| QML 파일 | 동반 삭제 |
| `unim-frontends/qt6` | **유지** (IM 모듈은 별개. popup 기능 제거만 검토) |
| `unim-tsf` (Windows) | **유지** |
| `debian/control` | `unim-gui-qt` 패키지 정의 삭제 또는 `unim-popup-service`로 대체 |
| autostart `.desktop` | unim-gui-qt 대신 unim-popup-service |
| systemd service unit | 신규 `unim-popup-service.service` (선택) |

---

## 10. 환경 변수 / 자동 검출 우선순위

```
WAYLAND_DISPLAY 있으면 → Wayland 시도 (input_popup_v2 → standalone)
DISPLAY 있고 WAYLAND_DISPLAY 없으면 → X11
둘 다 없으면 → 오류 종료
```

XDG_SESSION_TYPE 보조 검증.

---

## 11. 다음 문서

→ [`popup-redesign-plan.md`](popup-redesign-plan.md) — Phase별 작업 분해
