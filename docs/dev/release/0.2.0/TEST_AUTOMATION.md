# UNIM 0.2.0 — 자동 커버리지 매핑 (Test Automation)

> 수동 체크리스트([`TEST_CHECKLIST.md`](TEST_CHECKLIST.md))의 어느 항목이 자동 테스트로 커버되는지, 그리고 어떤 영역이 수동 보완을 필요로 하는지 정리한다.
>
> 빌드 환경 — `cargo 1.95.0` 필수:
> ```bash
> export PATH=$HOME/.cargo/bin:$PATH
> # /usr/bin/cargo 1.75는 Cargo.lock v4 미지원 → 사용 금지
> ```

---

## 1. 한 줄 요약

| 카테고리 | 자동화 비율 | 주 도구 |
|----------|-------------|---------|
| 골든패스 (Korean 입력) | **30 %** | `cargo test --workspace`, `make test-dbus` |
| AutoTypeFix | **60 %** | `src/auto_typefix.rs` 단위 + `typefix_blacklist` 테스트 |
| 팝업 (한자/특수문자) | **20 %** | `popup_state` 단위 (67 cases) + DBus signal smoke |
| 설정 GUI (GTK/Qt) | **5 %** | i18n 키 lint, `unim-cli config` 통합 |
| 회귀 (0.2.0 Fixed 9건) | **70 %** | 기존 unit/integration test |
| 환경 매트릭스 (X11/Wayland × GNOME/KDE) | **0 %** | 전부 수동 |
| **합계 (가중)** | **약 35 %** | — |

---

## 2. `make` 자동 타겟 사용법

### 2.1 cargo 단일 진입점
```bash
cargo test --workspace --release
```
- 모든 Rust 크레이트 (`src/`, `unim-daemon`, `unim-dbus`, `unim-cli`, `unim-capi`, `unim-gui-common`, `unim-gui-gtk`, `unim-gui-qt`, `unim-frontends/xim`, `unim-frontends/wayland`) 단위·문서 테스트 일괄 실행.
- 권장: 머지 전 1회 + 릴리즈 직전 1회.

### 2.2 DBus smoke
```bash
make test-dbus
```
- `unim-daemon -n` 단발 실행 → `busctl --user list/introspect` → 깔끔하게 종료.
- 검증: 서비스 등록, 메서드/시그널 introspection.

### 2.3 IM 모듈 GUI smoke
```bash
make test-gtk3   # tests/unim-test-gtk3/build/unim-test-gtk3
make test-gtk4
make test-qt5
make test-qt6
make test-xim
make test-gnome
make test-wayland   # target/release/unim-test-wayland
```
- 각 toolkit의 데모 앱이 빌드 + 실행되는지 smoke 확인.
- 키 입력 자동화는 포함되지 않음 → §3에서 수동 보완.

### 2.4 Xephyr 샌드박스 (격리 X11)
```bash
make sandbox-gtk3
make sandbox-gtk4
make sandbox-qt5
make sandbox-qt6
make sandbox-xim
make sandbox-indicator
```
- `scripts/sandbox.sh`가 Xephyr 띄우고 toolkit 데모를 실행. 호스트 데스크톱과 격리되어 한자 popup 위치 디버깅에 유용.

### 2.5 Quick Dev (개발 루프)
```bash
make dev-daemon       # 데몬 빌드 → kill → 재배포 → UNIM_DEVELOP=1 재시작
make dev-gtk3         # GTK3 모듈만
make dev-gtk4
make dev-qt5
make dev-qt6
make dev-xim
make dev-wayland
make dev-gui-gtk
make dev-gui-qt
make dev-extension    # GNOME Shell extension → ~/.local/share/
make dev-restart      # 모든 unim-* 일괄 재시작
make dev-test         # cargo test --workspace
```

### 2.6 Windows 크로스컴파일 검증
```bash
WIN_TARGET=x86_64-pc-windows-gnu make check-windows
WIN_TARGET=x86_64-pc-windows-gnu make build-windows
```

---

## 3. 컴포넌트별 커버리지 매트릭스

| 컴포넌트 | 자동 테스트 진입점 | 수동 보완 필요 | 비고 |
|----------|--------------------|----------------|------|
| unim-daemon | `cargo test -p unim-daemon` | systemd start/stop, RSS 추적 | jemalloc 회귀 측정은 `proc/smaps` 수동 |
| unim-cli | `cargo test -p unim-cli` + locale 통합 (`tests/locale_*`) | `--help` 한글/영문 시각 확인 | gettext 출력 자동 검증은 부분만 |
| unim-dbus | `make test-dbus` + `cargo test -p unim-dbus` | 재진입 시나리오 | introspection 자동화됨 |
| unim-gui-gtk | i18n 키 unit test, `cargo test -p unim-gui-gtk` | 모든 위젯 클릭 | UI 상호작용은 수동 |
| unim-gui-qt | `cargo test -p unim-gui-qt` (cxx-qt 빌드 검증) | QML 페이지 클릭 | UI는 수동 |
| unim-frontends/xim | `cargo test -p unim-xim`, `make test-xim` | xterm/Emacs 입력 | 키보드 시뮬은 수동 |
| unim-frontends/wayland | `cargo test -p unim-wayland`, `make test-wayland` | weston-text-input-demo | 컴포지터 의존 |
| GTK3 IM | `make test-gtk3` (smoke) | gedit/ghostty 입력 | 텍스트 입력 자동화 미지원 |
| GTK4 IM | `make test-gtk4` (smoke) | gedit/gnome-text-editor | 동일 |
| Qt5 IM | `make test-qt5` (smoke) | qt5 test app | 동일 |
| Qt6 IM | `make test-qt6` (smoke) | qt6 test app | 동일 |
| GNOME Extension | 없음 (JS/GJS) | 트레이/popup/prefs.js 전부 | ESLint만 가능 |
| Windows TSF | `make check-windows` | 메모장 입력 | Windows VM 필요 |

---

## 4. 시나리오 ↔ 자동화 매핑 (수동 체크리스트 기준)

| TEST_CHECKLIST 위치 | 자동 커버 | 미커버 (수동 필수) |
|----------------------|-----------|----------------------|
| §1 daemon 시작/재시작 | systemd unit smoke (수동 명령) | RSS 측정 |
| §2 unim-cli convert / config | `cargo test -p unim-cli` | locale 시각 확인 |
| §2 config layout list/validate | unit test | describe 출력 가독성 |
| §3 DBus introspect | `make test-dbus` | 재진입 시나리오 |
| §4 GTK GUI 위젯 탐방 | i18n lint, config persistence (cli round-trip) | 위젯 클릭 |
| §5 Qt GUI QML | cxx-qt 빌드 검증 | 클릭 |
| §6 XIM 골든패스 | `cargo test -p unim-xim` | xterm 키 입력 |
| §6 XIM N+1 BS 회귀 | `cargo test -p unim-xim --test autotypefix*` | xterm 시각 확인 |
| §7 Wayland | `cargo test -p unim-wayland` | weston-text-input-demo |
| §8 GTK3 골든패스 | `make test-gtk3` smoke | gtk3-demo / gedit |
| §8 GTK3 preedit-end | `cargo test -p unim` (조합 단위) | ghostty 시각 확인 |
| §9 GTK4 focus-out 중복 | `cargo test -p unim-dbus focus_out` | gedit `늘늘` 부재 확인 |
| §9 GTK4 surrounding | unit test | gedit 역방향 |
| §10/§11 Qt5/6 | `make test-qt5/6` smoke | 데모 앱 입력 |
| §12 GNOME ext | 없음 | 전부 수동 |
| §12 Emoji popup | 없음 | 전부 수동 |
| §13 Windows | `make check-windows` | 메모장 |
| §14 회귀 (R1~R9) | R1·R2·R3·R6·R7·R9 → unit test 보유 / R4·R5·R8 → smoke + 수동 | — |
| §15 환경 매트릭스 | 0 % | 전부 수동 |

---

## 5. 자동 테스트 재실행 권장 순서

```bash
# 1. 빌드+포맷+lint 확인 (warning 0 보장)
make build
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# 2. 단위/통합 테스트
cargo test --workspace --release

# 3. DBus + IM 모듈 smoke (UI 표시 후 수동으로 닫음)
make test-dbus
make test-gtk3 test-gtk4 test-qt5 test-qt6 test-xim test-wayland test-gnome

# 4. (옵션) Windows 크로스컴파일
WIN_TARGET=x86_64-pc-windows-gnu make check-windows
```

---

## 6. 자동 커버리지 부족 영역 — 수동 보완 가이드

| 영역 | 한계 | 권장 수동 절차 |
|------|------|---------------|
| 키보드 입력 시뮬 | xdotool/wtype은 IM 우회 불가 | 사용자가 직접 타이핑 |
| 한자 popup 시각 | DOM 검사 불가 (Wayland) | GNOME Looking Glass + 사진 캡처 |
| 환경 매트릭스 | CI 머신에 4종 환경 없음 | 사용자 노트북에서 4세션 부팅 |
| GNOME Shell extension | gjs는 단위 프레임워크 부재 | ESLint + 수동 |

---

## 7. 향후 자동화 후보 (0.3.x 이후)

- **dogtail/Accerciser**: GTK GUI 시나리오 일부 자동화 가능
- **virt-install + Win11 VM**: Windows TSF E2E 자동
- **input-event 로깅 헤어니스**: XIM/Wayland 키 입력 검증 (eudev evdev wrapper)
- **GNOME Shell test harness**: gjs 기반 단위 테스트 (`gnome-shell --replace --headless`)
