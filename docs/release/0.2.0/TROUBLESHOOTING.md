# UNIM 0.2.0 — 트러블슈팅 (초안)

> 본 문서는 manual-test-planner가 작성한 **초안**으로, doc-writer가 사용자 친화 문구·스크린샷·FAQ 형식으로 확장한다.
> 증상 → 1차 진단 → 2차 진단 → 우회/수정 순으로 정리.

---

## 0. 진단 공통 도구

| 명령 | 용도 |
|------|------|
| `journalctl --user -u unim -b --no-pager` | 데몬 systemd 로그 (이번 부팅) |
| `: > ~/.unim-errors.log; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &` | 로그 초기화 + 개발자 모드 재시작 |
| `tail -f ~/.unim-errors.log` | 실시간 로그 |
| `pgrep -a unim-` | 모든 unim-* 프로세스 목록 |
| `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod` | DBus API 노출 확인 |
| `unim-cli config show` / `path` | 현재 설정/경로 |
| `gtk-query-immodules-3.0 \| grep unim` / `gtk-query-immodules-4.0 \| grep unim` | GTK IM 모듈 등록 |
| `qtpaths --plugin-dir` + `ls`(plugins/platforminputcontexts) | Qt 플러그인 등록 |

---

## 1. 빌드 / 설치 단계

### 1.1 `cargo build` 실패 — `lock file version 4 requires '-Znext-lockfile-bump'`
- 원인: 시스템 cargo (`/usr/bin/cargo` 1.75)가 Cargo.lock v4 미지원.
- 해결:
  ```bash
  rustup default stable
  rustup update
  export PATH=$HOME/.cargo/bin:$PATH
  cargo --version   # 1.95.0+ 확인
  ```

### 1.2 `make build` warning 발생
- 정책: warning 0 (Zero Tolerance). 새 warning은 즉시 제거.
- 우선 `cargo clippy --workspace --all-targets -- -D warnings`로 식별.

### 1.3 `sudo make install` 시 권한 경고
- 가능하면 `make build` 먼저(비sudo) → `sudo make install PREFIX=/usr` 순서.
- Makefile의 `_check-build`가 `target/release/unim-daemon`을 검사하여 root 빌드 방지.

---

## 2. 데몬 / 서비스

### 2.1 데몬이 자동 시작되지 않음
- 1차: `systemctl --user status unim`
- 2차: `~/.config/systemd/user/unim.service` 존재 + `[Install] WantedBy=default.target`
- `systemctl --user daemon-reload && systemctl --user enable --now unim`

### 2.2 데몬이 두 개 이상 떠 있음
- `pkill -9 -x unim-daemon; sleep 1; systemctl --user start unim`
- DBus 자동 활성화 + 수동 실행이 겹치는 경우 발생. 수동 실행 시 `--replace` 플래그 사용.

### 2.3 RSS가 시간 경과로 증가
- AGENTS.md "메모리 관리 규칙" 진단 실행:
  ```bash
  grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
  cat /proc/$(pidof unim-daemon)/smaps_rollup | grep -E 'Rss|Anonymous'
  ```
- jemalloc + `MALLOC_ARENA_MAX=2` 적용 확인. 손상된 경우 회귀.

---

## 3. CLI

### 3.1 `unim-cli --help` 한국어가 깨짐
- locale 미설치: `sudo locale-gen ko_KR.UTF-8`
- gettext mo: `ls /usr/share/locale/ko/LC_MESSAGES/unim*.mo`

### 3.2 `unim-cli config set` 후 GUI에 반영 안 됨
- 데몬이 mtime 핫리로드 못함 → `pkill -SIGHUP unim-daemon`
- 5지점 sync 깨짐 가능성 → CLI/엔진/GUI/locale/dbus 5점 모두 갱신됐는지 점검.

---

## 4. DBus / IPC

### 4.1 `busctl --user list | grep unim` 비어 있음
- 1차: `pgrep -a unim-daemon` 확인
- 2차: `~/.local/share/dbus-1/services/org.atit.unim.InputMethod.service` 존재 확인
- 우회: 수동 활성 — `dbus-send --session --print-reply --dest=org.atit.unim.InputMethod /org/atit/unim/InputMethod org.freedesktop.DBus.Peer.Ping`

### 4.2 한자 popup 중 키 잠금 (재진입)
- `~/.unim-errors.log | grep -i 'queue\|reentr'`
- GNOME Extension `key_handler.js`의 큐가 정상 동작하는지 (Wayland 한정).

---

## 5. GTK 입력 모듈

### 5.1 한국어가 입력되지 않음
- 1차: `GTK_IM_MODULE=unim` 환경 변수 export 확인
- 2차: `gtk-query-immodules-3.0 | grep unim` / `gtk-query-immodules-4.0 | grep unim`
- 3차: 모듈 파일명 검증 — GTK3는 `im-unim.so`, GTK4는 `libim-unim.so` (혼동 주의)

### 5.2 ghostty/터미널에서 키 잠금 (preedit-end 누락)
- 0.2.0에서 `unim_emit_preedit` 헬퍼로 수정됨. 재발 시 회귀 — 즉시 보고.

### 5.3 gedit에서 `늘늘` 이중 commit
- focus-out 시 CommitText 시그널 broadcasting 회귀. 0.2.0에서 fix됨.
- 재현 시 `~/.unim-errors.log | grep -i 'commit_text\|focus_out'` 확인.

### 5.4 영문 모드 Space가 누락됨 (gedit)
- 0.2.0에서 `consumed=true commit=" "` 경로로 수정됨. 회귀 시 `engine_worker.rs` Space 처리 분기 점검.

---

## 6. Qt 입력 모듈

### 6.1 Qt5/6 앱에서 입력 안 됨
- `QT_IM_MODULE=unim` export
- 플러그인 위치: `/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunim.so`, `qt6/...`
- `QT_DEBUG_PLUGINS=1 <app> 2>&1 | grep -i unim`

### 6.2 Qt6 트레이 GUI(unim-gui-qt)와 IM 모듈이 충돌
- 다른 프로세스이며 데몬을 공유. 둘 다 가동 가능. RSS 갑자기 증가하면 §2.3.

---

## 7. XIM

### 7.1 xterm에서 한국어 안 됨
- `XMODIFIERS=@im=unim`, `xprop -root | grep XIM_SERVERS`
- `pgrep -a unim-xim` (없으면 `/usr/libexec/unim-xim &`)

### 7.2 AutoTypeFix가 잔존 BS를 남김
- 0.2.0 N+1 BS 모델로 수정됨. Chrome preedit edge case는 알려진 SKIP.

---

## 8. Wayland

### 8.1 weston-text-input-demo에서 preedit이 보이지 않음
- 컴포지터가 `text-input-v3` 지원 확인 (`weston-info` / `wayland-info`).
- `WAYLAND_DEBUG=1` 후 `wl_text_input` 메시지 확인.

### 8.2 순수 Wayland에서 한자 popup이 보이지 않음
- 알려진 미해결 이슈. GNOME Wayland에서는 GNOME Extension Push 모드로 우회.

---

## 9. GNOME Shell Extension

### 9.1 트레이 인디케이터가 안 보임
- `gnome-extensions list --enabled | grep unim`
- 비활성: `gnome-extensions enable unim@from104.github.io`
- 로그아웃 → 로그인 필요 (`make dev-extension` 후).

### 9.2 prefs.js에 옵션 부재
- `glib-compile-schemas ~/.local/share/gnome-shell/extensions/<UUID>/schemas`
- Phase 8 cleanup으로 dead feature 옵션이 제거됨 — 의도된 변화.

### 9.3 한자 popup이 caret 위치에서 어긋남
- POPUP_SPEC §6 화면 경계 보정 로직. caret_rect 좌표가 NULL이면 화면 중앙 fallback.
- `~/.unim-errors.log | grep -i 'cursor_rect\|popup_x'`

### 9.4 Super+. 이모지 popup이 동작 안 함
- 단축키 충돌(GNOME 자체 이모지 picker). `gsettings reset org.gnome.shell.keybindings show-screen-recording-ui` 등 확인.

---

## 10. AutoTypeFix / Suppression

### 10.1 의도치 않은 자동 교정
- GUI Suppression Words에서 ASCII 입력 후 `Confirm` → 영구 차단.
- 또는 `~/.config/unim/typefix-blacklist.yaml` 직접 편집 후 mtime 변경.

### 10.2 의도한 교정이 학습되지 않음
- forward는 BS+모드전환(AND), reverse는 BS or 모드전환(OR) 게이트.
- `~/.unim-errors.log | grep -i typefix`로 RecentCorrection 추적.

### 10.3 `tentative_expiry_hours` 단위 혼동
- 0.2.0부터 days→hours로 변경됨 (1..=12). config.yaml의 기존 값은 자동 마이그레이션.

---

## 11. 한자 / 특수문자 popup

### 11.1 popup이 빈 화면에 뜸
- caret_rect 미수신: `cursor_y = 0` fallback. POPUP_SPEC §6.3 좌표 소스 확인.

### 11.2 9칸 ↔ 81칸 토글 안 됨
- Period(.) 키가 다른 곳에서 가로채짐. 키맵 확인.

### 11.3 책갈피(★)가 다른 popup에 즉시 반영 안 됨
- `HanjaBookmarkChanged` 시그널 미수신. `busctl --user monitor org.atit.unim.InputMethod`로 시그널 흐름 확인.

---

## 12. 환경 매트릭스 알려진 이슈

| 환경 | 알려진 이슈 |
|------|-------------|
| Wayland + GNOME | 정상 (Push 모드) |
| Wayland + KDE | 한자 popup 미표시 (Push 모드 미구현) |
| X11 + GNOME | XIM 폴백 권장 |
| X11 + KDE | 정상 |
| 순수 Wayland (Weston/sway) | 한자 popup 미해결 SKIP |

---

## 13. 로그 분석 슬래시 명령

```bash
# Claude Code 사용 시
/unim-log
```
→ `~/.unim-errors.log` 자동 분류·요약·진단.

---

## 14. 그래도 막히면

1. `~/.unim-errors.log` + `journalctl --user -u unim` 첨부하여 GitHub Issue
2. 재현 단계 + `unim-cli --version`, `unim-cli config show`, `cargo --version`, `gnome-shell --version` 동봉
3. 환경 (X11/Wayland, GNOME/KDE, 배포판/버전) 명시
