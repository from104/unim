# UNIM Popup 재설계 — 사전 리서치

**작성일**: 2026-05-14
**브랜치**: `arch/popup-unify`
**작성자**: 메인 에이전트 (사용자 지시)
**범위**: 한자/특수문자/이모지 popup 통합 + unim-gui-qt 폐기 + GTK4 단일화 + X11/Wayland 분기 대응

---

## 1. 문제 정의

### 1.1 현황 (코드 분포)

| 프런트엔드 | 파일 | 줄 수 | 기술 스택 |
|---|---|---|---|
| `unim-gui-gtk` | hanja_popup.rs / special_popup.rs / emoji_popup.rs / popup_positioning.rs | **2,196** | Rust + gtk4-rs (GTK4) |
| `unim-gui-qt` | HanjaPopup.qml / SpecialPopup.qml / EmojiPopup.qml | **871** | cxx-qt 0.8 + QML + Qt6 |
| `unim-gnome-extension` | hanja_popup.js / special_popup.js / emoji_popup.js | **2,119** | GNOME Shell St(JS) |
| `unim-windows` | ui/popup.rs | (~100) | Windows native |
| **합계 (3개 Linux 프런트)** | | **≈5,186** | |

각 프런트가 **동일한 ViewModel(`PopupModel`)에서 동일한 시각·동작 명세(`POPUP_SPEC.md`)를 따라야 한다**. 즉 같은 일을 4가지 언어/툴킷으로 4번 구현. 한 시각 사양 변경마다 4곳 동기화 — 실제 버그·드리프트가 반복 발생.

### 1.2 분기 책임

- **GNOME Shell (Wayland·X11 공통)**: extension 자체가 popup·트레이 담당
- **KDE Plasma 6 / Wayland**: `unim-gui-qt`가 popup·트레이
- **X11 / 비-GNOME**: `unim-gui-gtk`가 popup·트레이
- **Wayland(layer-shell 환경)**: 사실상 미지원 — 최근 KWin/Hyprland에서 GTK4 client-side popup 위치 신뢰도 문제 다수 보고

### 1.3 사용자 요구

> "gtk·qt·gnome ext (wayland)로 나누어진 코드부터 통합. unim-gui-qt 폐기, 인디케이터도 gtk로 통합 (KDE 확인). 별도 popup 전용 프로세스로 분리. X11/Wayland 별 분기. 별도 브랜치 끝까지."

→ **단일 popup 서비스(GTK4 기반) + DBus IPC + 컴포지터별 분기**. GNOME extension은 별도 책임 가능하나 daemon 의 ViewModel을 재사용하여 코드 양 축소.

---

## 2. 외부 사례 리서치

### 2.1 IBus (모범 사례 — daemon + ui 분리)

- **구조**: `ibus-daemon` (입력 엔진/IPC 허브) + `ibus-ui-gtk3` (Panel·Candidate Window·Property·Emoji Selector 등 UI 프로세스, GTK3) — **두 프로세스 분리, DBus IPC**.
- **Panel 책임**: 후보창(candidate window), Language Bar, Property Panel, Emoji Selector. 모두 GTK3 단일 툴킷.
- **결정 근거**: daemon의 입력 로직과 UI 분리로 인해 desktop 환경(GNOME/KDE/X11/Wayland)별로 panel만 교체하면 됨. 실제 KDE는 `kimpanel` GNOME은 자체.
- **단점**: panel이 GTK3라 KDE Wayland에서 위치 정확성·테마 매칭 이슈. 일부 Plasma 사용자는 ibus 대신 fcitx5 채택.

출처: [IBus - ArchWiki](https://wiki.archlinux.org/title/IBus), [ibus/ui/gtk3/panel.vala](https://github.com/ibus/ibus/blob/main/ui/gtk3/panel.vala)

### 2.2 Fcitx5 (in-process render + 폴백)

- **구조**: `fcitx5` daemon + IM module(GTK/Qt 임베디드 client-side render) + 폴백 standalone window
- **Wayland 동작**:
  1. **1순위**: IM 모듈이 client process 안에서 `xdg_popup`(`get_popup` + positioner)으로 sub-popup 그림 — 위치 정확
  2. **2순위 fallback**: client-side가 안 되면 fcitx5 자체 standalone window 띄움 — **이때 Wayland window는 자유 위치 못 잡아 XWayland fallback** (위치 정확성 위해)
- **KDE Plasma 6 / KWin Wayland**: KIM Panel은 Wayland 미지원 → fcitx5는 자체 candidate window로 input-method-v2 사용
- **단점**: GTK3/Qt5의 `xdg_popup` 구현이 reposition 미지원이라 show/hide 트릭 + 깜빡임 발생. Plasma 6.6.1에서 일부 개선

출처: [Using Fcitx 5 on Wayland](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland), [Fcitx5 GitHub Discussion #895](https://github.com/fcitx/fcitx5/discussions/895)

### 2.3 Seogi (Hangul IME for Wayland, mswiger/seogi)

- **구조**: 단일 daemon (libhangul 사용) + DBus 서비스로 상태 노출 (Hangul mode toggle)
- **프로토콜**: `input-method-unstable-v2` + `virtual-keyboard-unstable-v1` 사용
- **현재 한계**: Sway만 테스트 — KWin/GNOME은 미검증. Waybar 모듈로 상태 표시 (`seogi-waybar`)
- **시사점**: Wayland 전용 IME는 protocol을 직접 호출. popup·트레이는 별도 파트너 모듈(Waybar 등)에 위임 가능

출처: [GitHub mswiger/seogi](https://github.com/mswiger/seogi), [seogi-waybar](https://github.com/mswiger/seogi-waybar)

### 2.4 Kime (Korean IME in Rust)

- **구조**: Rust 단일 코드베이스. XIM, Wayland, GTK3, GTK4, Qt5, Qt6 모두 지원 (multi-protocol)
- **시사점**: 단일 Rust core에서 multi-frontend 노출 — UNIM과 동일 철학. 다만 candidate popup은 IM 모듈이 client-side에서 그림 (별도 popup 프로세스 없음)

출처: [GitHub Riey/kime](https://github.com/Riey/kime)

### 2.5 종합 비교표

| 프로젝트 | popup 위치 | 분리 방식 | KDE Wayland 지원 |
|---|---|---|---|
| **IBus** | 별도 panel 프로세스 (GTK3) | daemon ↔ panel DBus | △ (kimpanel 의존) |
| **Fcitx5** | client in-process + 자체 standalone fallback | daemon + IM module + 자체 candidate | ◯ (input-method-v2) |
| **Kime** | client in-process (IM 모듈 안) | 분리 없음 — 모든 frontend가 자체 popup | △ (toolkit별 한계) |
| **Seogi** | 미지원 (Waybar 위임) | daemon + 외부 panel | △ (Sway 검증) |
| **UNIM (현재)** | 프런트별 4중 구현 | 분리 안 됨 | △ (Qt 측 미완성) |
| **UNIM (목표)** | **별도 popup 프로세스 (GTK4)** | daemon ↔ popup-service DBus | ◎ (목표) |

→ **IBus 패턴이 가장 적합**. 단 GTK3 대신 GTK4-rs 사용, daemon은 Rust 단일.

---

## 3. Wayland IME popup 프로토콜 정리

### 3.1 text-input-v3 (`zwp_text_input_v3`)
- **역할**: 애플리케이션 ↔ IME(또는 컴포지터)간 텍스트 입력 전달
- **이벤트**: `preedit_string`, `commit_string`, `delete_surrounding_text`, `done`
- **client-side popup 위치 query**: `text_input_v3.set_cursor_rectangle()` — 컴포지터에 cursor 위치 알림

출처: [Text input protocol v3](https://wayland.app/protocols/text-input-unstable-v3)

### 3.2 input-method-v2 (`zwp_input_method_v2`)
- **역할**: IME가 직접 텍스트 입력 처리 (compositor가 라우팅)
- **input_popup_surface_v2**: IME 전용 popup surface role. wl_surface에 "input_popup" role 부여. **컴포지터가 cursor 위치 기반 자동 배치**
- **현황**:
  - KWin (Plasma 6): 지원
  - Sway/wlroots: 지원
  - GNOME Mutter: 미지원 (자체 IBus 사용)

출처: [Input method v2 protocol](https://wayland.app/protocols/input-method-unstable-v2), [SDB:Wayland input methods (openSUSE)](https://en.opensuse.org/SDB:Wayland_input_methods)

### 3.3 xdg-popup (`xdg_popup` in xdg-shell)
- **역할**: 일반 popup window (메뉴/툴팁). `get_popup` + `xdg_positioner`로 위치 지정. **transient_for parent xdg_toplevel 필수**.
- **grab**: `xdg_popup.grab(seat, serial)` — outside click 자동 dismiss (`popup_done` 이벤트 발사)
- **GTK4-rs 매핑**: `gtk4::Window`에 `transient_for(parent)` + `Qt.Popup`-like flag → GTK4가 자동 xdg-popup 매핑
- **한계**: parent toplevel이 우리 프로세스 안에 있어야. **별도 popup-service 프로세스는 외부 앱의 toplevel을 parent로 못 잡음** → xdg-popup grab 사용 불가

출처: [Popups & parent windows (Wayland Book)](https://wayland-book.com/xdg-shell-in-depth/popups.html), [XDG shell protocol](https://wayland.app/protocols/xdg-shell)

### 3.4 wlr-layer-shell-unstable-v1
- **역할**: panel/notification 같은 desktop shell 컴포넌트. anchor·z-layer·keyboard_interactivity 지정
- **keyboard_interactivity**: `none` / `on-demand` / `exclusive` 3종. on-demand면 outside click dismiss 가능
- **컴포지터 지원**: wlroots(sway/hyprland), KWin (Plasma 6 layer-shell-v1 지원). GNOME Mutter 미지원
- **시사점**: GNOME 빼고 거의 다 지원. layer-shell + on-demand로 popup 표시 가능

출처: [wlr-layer-shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1), [Wayland Explorer](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)

### 3.5 Wayland 분기 전략

| 환경 | 권장 메커니즘 | 비고 |
|---|---|---|
| **KDE Plasma 6 (KWin)** | `input_popup_surface_v2` (1순위) → `xdg_popup` external standalone (2순위) | KWin이 모든 IME popup 처리 |
| **GNOME Wayland** | extension 사용 (현재 그대로) | Mutter는 IME protocol 미지원 |
| **wlroots (Sway/Hyprland)** | `input_popup_surface_v2` 또는 layer-shell `on-demand` | seogi 사례 참조 |
| **X11/XWayland** | override-redirect + XGrabPointer | 아래 §4 참조 |

---

## 4. X11 popup 분리 메커니즘

### 4.1 override-redirect (popup 최우선)
- WM에게 "내 위치/크기 관리 마라" 알림 → 우리 좌표 그대로 표시 (KWin 보정 없음)
- 단점: tooltip 분류로 분류되어 alt-tab 등에서 사라짐 (popup 의도엔 OK)

### 4.2 _NET_WM_WINDOW_TYPE_POPUP_MENU
- EWMH hint. WM에게 "popup 메뉴"라 알림 → WM이 적절히 처리 (보통 stack on top, no taskbar)
- override-redirect와 함께 사용 권장

### 4.3 XGrabPointer + ReplayPointer (fcitx5 fallback 패턴)
- popup 표시 시 root window에 `GrabPointer { pointer_mode: SYNC, keyboard_mode: ASYNC }`
- ButtonPress 이벤트 수신 → 좌표 ↔ popup 영역 비교
  - **inside** → `AllowEvents(REPLAY_POINTER)`: popup이 정상 클릭 받음
  - **outside** → dismiss + `UngrabPointer`: 클릭은 외부 앱으로 정상 전달
- 키보드 grab 절대 금지 (IM 입력 보호)

출처: [X11::Protocol::WM manpage](https://manpages.ubuntu.com/manpages/bionic/man3/X11::Protocol::WM.3pm.html), [Mozilla bug 1575136](https://bugzilla.mozilla.org/show_bug.cgi?id=1575136)

### 4.4 GTK4-rs에서 override-redirect
- `gtk4::Window::set_decorated(false)` + `Window` type hint 적용
- 외부 클릭 dismiss는 `gtk4::EventControllerLegacy` + `gdk4_x11::X11Surface`로 XGrabPointer 호출 (현재 GTK 측에 `popup_positioning::x11_install_outside_click_handler` 이미 구현 — XCB polling 기반)
- GTK4가 자체 popup 모드 지원 (`gtk4::Popover`도 있음, 단 in-process)

---

## 5. UNIM 현 코드 자산 평가

### 5.1 재사용 가능 (그대로 유지)
- `unim-daemon` (engine, popup_dispatch, view_model 계산) — popup-service의 SoT data 제공
- `unim-gui-common::popup_state::PopupModel` — ViewModel 공통 데이터 구조 (kind/cells/headers/tab_labels 등)
- `unim-gui-common::popup_dbus` — daemon ↔ client RPC 헬퍼
- `unim-gui-common::popup_position::compute_popup_xy` — 화면 경계 보정 (Rust 단일)
- `unim-gui-gtk::popup_positioning::x11_install_outside_click_handler` — X11 outside-click polling (검증된 패턴)
- `unim-gui-gtk::{hanja,special,emoji}_popup.rs` — **GTK4 popup 코드 자산**
- `unim-gui-gtk::tray` (ksni 기반) — KDE Plasma 6 system tray protocol과 호환 (KStatusNotifierItem). GTK/KDE 모두 동작

### 5.2 폐기/통합 대상
- `unim-gui-qt/*` — **전체 폐기** (~5,500줄 bridge.rs 포함). Settings 다이얼로그는 GTK4로 재작성 또는 기존 `unim-gui-gtk` settings 재사용
- `unim-gui-qt::outside_click_watcher` — Qt 측 polling 제거 (GTK 측 이미 동일 패턴 존재)
- `unim-gui-qt::tray` (ksni 동일 사용)

### 5.3 신규 추가 필요
- 새 crate `unim-popup-service` — popup·트레이 전담 standalone 프로세스 (GTK4-rs)
- (선택) Wayland 보강: `input_popup_surface_v2` 사용 가능한 환경 검출 + 자체 protocol 호출

---

## 6. 핵심 설계 결정 (Decisions)

| ID | 결정 | 근거 |
|---|---|---|
| D-1 | **GTK4-rs 단일 toolkit** (Qt 폐기) | GTK4가 X11/Wayland 양쪽 안정. ksni로 KDE 트레이 호환. Rust 단일 코드베이스 |
| D-2 | **popup·트레이 = 별도 프로세스** | IBus 패턴. daemon과 UI 분리. 재시작·교체 가능 |
| D-3 | **DBus 통신** (기존 `unim-gui-common::popup_dbus` 재사용) | RPC 자산 그대로. 신규 protocol 추가 최소화 |
| D-4 | **X11/Wayland 분기는 popup-service 내부에서** | 외부에서 단일 API. 환경 detect 후 자동 분기 |
| D-5 | **Wayland 1차 전략: 클라이언트 standalone window (xdg_toplevel) + 최소 데코** | 현실적. KWin은 cursor anchor를 daemon이 위치로 계산 후 전달 |
| D-6 | **Wayland 2차 전략: `input_popup_surface_v2`** | KWin/wlroots에서 정확. 별도 Phase로 검증 후 마이그레이션 |
| D-7 | **GNOME extension은 그대로 유지** (직접 표시) | GNOME Shell 자체 popup이 더 정확. 단 GNOME extension의 popup 로직과 popup-service의 GTK 로직은 같은 SoT(daemon ViewModel) 공유 |
| D-8 | **outside-click dismiss**: X11=기존 XCB polling, Wayland=xdg-popup grab 또는 layer-shell on-demand | GTK 측 검증된 코드 자산 활용 |
| D-9 | **KDE Plasma 6 검증 환경**: 사용자 환경(KWin X11 + Wayland)에서 단계별 테스트 | 실제 사용 환경에서 회귀 없음 보장 |

---

## 7. 위험 분석

### 7.1 위험 매트릭스

| 위험 | 가능성 | 영향 | 완화 |
|---|---|---|---|
| Qt 폐기 시 KDE 환경에서 GTK4 popup 위치 부정확 | 중 | 높 | Phase 4에서 KWin 사용자 환경 검증 + show/hide 트릭 (fcitx5 패턴) 적용 |
| Wayland popup grab semantics 차이 (xdg-popup parent 요구) | 높 | 중 | standalone window + XCB-style polling 우회 또는 `input_popup_surface_v2` 도입 |
| 별도 프로세스화로 IPC 오버헤드 증가 | 낮 | 낮 | popup 표시는 ms 단위 — DBus latency 무시 가능 |
| 사용자 정의 시각 사양과 GNOME extension drift | 중 | 중 | popup_styles `popup_tokens.toml` SoT 유지. extension도 동일 토큰 사용 |
| Phase 진행 중 빌드 깨짐 | 중 | 중 | 각 Phase 끝에 `make build` + `cargo test --workspace` 회귀 검증 강제 |
| KDE 트레이 인디케이터 ksni 동작 차이 | 낮 | 낮 | 현재 unim-gui-gtk가 이미 ksni 사용. 추가 변경 없음 |

### 7.2 비위험 (현실 점검)
- **GNOME에서 동작 보장 불필요**: GNOME extension이 자체 popup 처리 — 이번 작업은 KDE/non-GNOME 환경 중심
- **Windows 분리 불필요**: `unim-windows`는 별도 OS. 본 작업 범위 밖

---

## 8. 다음 단계 — 아키텍처 설계 문서로 이동

다음 문서: `docs/architecture/popup-process.md` — 새 `unim-popup-service` 프로세스의 모듈 구조·DBus 인터페이스·X11/Wayland 분기 점·시작/종료 라이프사이클 명세.

이후: `docs/architecture/popup-redesign-plan.md` — Phase별 작업 분해.

---

## Sources

- [IBus - ArchWiki](https://wiki.archlinux.org/title/IBus)
- [ibus/ui/gtk3/panel.vala](https://github.com/ibus/ibus/blob/main/ui/gtk3/panel.vala)
- [Using Fcitx 5 on Wayland](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland)
- [Fcitx5 - ArchWiki](https://wiki.archlinux.org/title/Fcitx5)
- [Fcitx5 GitHub Discussion #895](https://github.com/fcitx/fcitx5/discussions/895)
- [GitHub Riey/kime](https://github.com/Riey/kime)
- [GitHub mswiger/seogi](https://github.com/mswiger/seogi)
- [seogi-waybar](https://github.com/mswiger/seogi-waybar)
- [Text input protocol v3](https://wayland.app/protocols/text-input-unstable-v3)
- [Input method v2 protocol](https://wayland.app/protocols/input-method-unstable-v2)
- [Popups & parent windows (Wayland Book)](https://wayland-book.com/xdg-shell-in-depth/popups.html)
- [XDG shell protocol](https://wayland.app/protocols/xdg-shell)
- [wlr-layer-shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
- [SDB:Wayland input methods (openSUSE)](https://en.opensuse.org/SDB:Wayland_input_methods)
- [QtCS2021 - Wayland text-input-unstable-v4 (Qt Wiki)](https://wiki.qt.io/QtCS2021_-_Wayland_text-input-unstable-v4_protocol)
- [Mozilla bug 1575136 (X11 popup type)](https://bugzilla.mozilla.org/show_bug.cgi?id=1575136)
- [Fix GTK4 application themes on KDE Plasma 6 Wayland (gist)](https://gist.github.com/DenebTM/3cad3bbaee0cdc2ad190162a969e4a87)
- [How Input Methods Work in Linux (Nerufic)](https://nerufic.com/en/posts/how-input-methods-work-in-linux/)

---

## 후속 노트 — 0.3.0 구현 결과 (2026-05-19)

### 달성한 것

`arch/popup-unify` 브랜치에서 사전 리서치에서 제안한 설계를 전면 구현했다.

|항목|결과|
|---|---|
|렌더러 통합|`unim-popup-service` (GTK4) + GNOME extension `popup_view.js` 두 렌더러로 수렴|
|`unim-gui-qt` 폐기|완료. KDE 사용자는 `unim-gui-gtk` + `unim-popup-service`로 마이그레이션|
|D-Bus forward 계층|`org.atit.unim.Popup` 인터페이스 신설. daemon은 forward만, 렌더러가 구독|
|단일 view-model SoT|`PopupRender` payload — 셀·헤더·푸터·탭·하이라이트 완전 통합|
|GNOME Wayland|Mutter `wlr-layer-shell` 미지원 확인 → extension St 위젯으로 우회|
|GNOME X11|popup-service GTK4 윈도우 (D-Bus auto-activation)|
|KDE/Xfce/X11 WM|popup-service GTK4 윈도우|
|Wayland WM (Sway/Hyprland)|popup-service `libgtk4-layer-shell` 조건부 지원|
|외부 좌클릭 dismiss|팝업 영역 밖 클릭 → 팝업 닫힘 + 클릭 이벤트 pass-through|
|D-Bus auto-activation|`org.atit.unim.PopupService.service` — autostart .desktop 제거|

### 미해결 사항

- **KDE Plasma 5.x Wayland**: Ubuntu 24.04 표준 저장소에 `gtk4-layer-shell` 미제공.
  팝업 위치 지정 불가. 회피책: X11 세션 사용 또는 GNOME으로 전환.
- **XIM ON-THE-SPOT (PREEDIT_CALLBACKS) preedit drop**: `commit_then_preedit` 경로
  `clear_preedit()` → `commit()` 순서 수정으로 best-effort 완화. 일부 클라이언트에서
  잔존. upstream xim-0.5.0 수정 대기 중.

### 설계 변경점

사전 리서치에서 `unim-daemon` 내부에 팝업 렌더링을 유지하는 안과 완전 분리하는 안을
비교했다. 최종 구현은 **완전 분리** 방향을 채택했으며, daemon은 view-model 생성과
forward만 담당한다. 이로써 렌더러 교체/추가가 daemon 코드 변경 없이 가능해졌다.

### 관련 문서

- [POPUP_SPEC.md](../dev/specs/POPUP_SPEC.md)
- [IME_BEHAVIOR.md §4](../dev/architecture/IME_BEHAVIOR.md)
- [0.3.0 릴리즈 노트](../user/release-notes/0.3.0/README.md)
