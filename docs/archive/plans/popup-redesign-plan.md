# UNIM popup 재설계 — Phase별 실행 계획

**브랜치**: `arch/popup-unify`
**기준 문서**: [`popup-process.md`](popup-process.md), [`popup-redesign.md`](popup-redesign.md)
**진행 정책**: 단일 세션 단계별 빌드 검증, 최종 한 번에 사용자 회귀 테스트

---

## Phase 1 — 신규 crate 골격 + popup 코드 복사

**산출물**: `unim-popup-service` workspace 멤버, `cargo build --workspace` 통과.

작업:
1. `unim-popup-service/Cargo.toml` (의존성 정의, default-features x11+wayland)
2. `unim-popup-service/src/main.rs` (entry, ensure_single_instance, GTK Application 골격)
3. `unim-popup-service/src/lib.rs` (모듈 노출)
4. `unim-popup-service/src/popup/{mod,hanja,special,emoji}.rs` — `unim-gui-gtk/src/{hanja,special,emoji}_popup.rs` 그대로 복사 + path 수정
5. `unim-popup-service/src/backend/{mod,x11,wayland_standalone}.rs` 골격
6. `unim-popup-service/src/{tray,dbus_listen,single_instance}.rs`
7. `unim-popup-service/locales/{ko,en}.yml` — `unim-gui-gtk/locales` 복사
8. workspace `Cargo.toml`에 멤버 추가
9. `cargo build -p unim-popup-service`

검증: 빌드 통과, 기본 binary 실행 시 "Hello" 로그만.

---

## Phase 2 — daemon DBus 구독 + popup 표시

**산출물**: 신 popup-service가 실제로 popup 표시 가능.

작업:
1. `dbus_listen.rs`: `dbus_client::watch_dbus_signals` 패턴 이식 — daemon에서 ShowXxxPopup signal 수신 → PopupManager 호출
2. `popup/mod.rs`: PopupManager (kind별 dispatch — Hanja/Special/Emoji)
3. backend trait 호출 — Phase 3에서 backend 구체화 전, 기본 GTK4 window 표시
4. `RegisterFrontend("popup-service")` 호출
5. tray (ksni) 시작 — `unim-gui-common::tray::TrayController` 사용
6. main.rs 통합

검증: daemon 실행 + popup-service 실행 → 한자 키 → popup 표시.

---

## Phase 3 — X11 backend 정리

**산출물**: X11 환경에서 popup 정확 표시 + outside-click dismiss.

작업:
1. `backend/x11.rs`: `unim-gui-gtk/src/popup_positioning.rs`의 `x11_install_outside_click_handler` 이관
2. override-redirect 설정 + _NET_WM_WINDOW_TYPE_POPUP_MENU
3. Phase 2의 popup_manager에서 backend.install_outside_click_dismiss 호출
4. KDE Plasma X11 환경 검증

---

## Phase 4 — Wayland 1차 standalone backend

**산출물**: Wayland 환경에서 popup 정상 표시 (xdg_toplevel + XCB-style polling 우회).

작업:
1. `backend/wayland_standalone.rs`: gdk4-wayland 통한 xdg_toplevel popup
2. cursor anchor 위치를 daemon에서 받아 popup window 좌표 직접 계산 (compute_popup_xy 재사용)
3. outside-click dismiss: GTK4 Window::set_modal(false) + 자체 X11 같은 polling 불가 → KWin idle hint
4. show/hide 트릭 (fcitx5 패턴) for KWin

---

## Phase 5 — Wayland 2차 input_popup_surface_v2 backend

**산출물**: KWin/wlroots에서 컴포지터 정확 anchor.

작업:
1. `backend/wayland_input_popup.rs`: wayland-client + wayland-protocols로 zwp_input_method_v2 + zwp_input_popup_surface_v2 직접 호출
2. detection: zwp_input_method_v2 protocol 광고 있으면 사용
3. fallback chain: input_popup → standalone → 오류

---

## Phase 6 — unim-gui-gtk 정리 (popup·tray 제거, settings만)

**산출물**: `unim-gui-gtk`는 settings GUI 전용.

작업:
1. `unim-gui-gtk/src/main.rs`: popup·tray 초기화 제거
2. `hanja_popup.rs`, `special_popup.rs`, `emoji_popup.rs`, `popup_positioning.rs` 삭제 (또는 deprecated 폴더 보관)
3. tray 의존성 제거
4. main.rs는 SettingsWindow만 표시
5. `unim-gui-gtk --settings` 호출 동작 확인

---

## Phase 7 — unim-gui-qt 완전 폐기

**산출물**: workspace에서 unim-gui-qt 제거.

작업:
1. `unim-gui-qt/` 디렉토리 git rm -rf
2. workspace `Cargo.toml`에서 멤버 제거
3. `debian/control`에서 `unim-gui-qt` 패키지 정의 제거
4. autostart `.desktop` 파일 정리
5. `unim-popup-service.desktop` 신규 추가
6. CHANGELOG 업데이트

---

## Phase 8 — 최종 빌드/테스트 검증

**산출물**: `make build`(zero warning) + `cargo test --workspace`(all pass) 모두 통과.

작업:
1. `make build`
2. `cargo test --workspace`
3. 사용자 실기 테스트:
   - X11: 한자/특수문자/이모지 popup 표시, 외부 클릭 dismiss
   - Wayland (KDE Plasma 6): 동일
   - GNOME extension: 변경 없음 확인
   - 트레이 메뉴: KDE + GTK 환경
4. `obsidian-journal-writer`로 작업 일지

---

## 단일 세션 진행 우선순위

세션 토큰 한계로 인해 다음 우선순위로 진행:
1. **Phase 1, 2, 3** (필수) — 신규 crate + daemon 통신 + X11 동작
2. **Phase 6, 7** (필수) — 기존 코드 정리
3. **Phase 4, 5** (가능 시) — Wayland backend
4. **Phase 8** (필수) — 빌드 검증

토큰 모자르면 Phase 4/5는 후속 세션으로 (사용자에 보고).
