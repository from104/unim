# UNIM 0.2.0 — 수동 테스트 체크리스트 (한국어)

> 사용자(기현)가 직접 키보드와 GUI로 따라할 수 있는 형식. 각 시나리오 옆 `[ ] PASS / FAIL` 박스에 결과를 기록한다.
> 시간 표기는 보수적(평소 3분 작업 ≈ 5분 표기). 모든 명령은 그대로 복사·실행 가능한 형태이며, 실패 시 진단법을 동봉했다.
>
> **사전 작업**: 본 체크리스트의 회귀 시나리오는 `[0.2.0] Fixed` 항목과 1:1 대응한다.
> 자동화 가능 영역은 [`TEST_AUTOMATION.md`](TEST_AUTOMATION.md), 트러블슈팅은 [사용자 트러블슈팅 가이드](../../../user/troubleshooting/README-ko.md) "0.2.0 릴리스 특이 진단" 섹션 참조.

---

## 0. 사전 준비 (10분)

```bash
export PATH=$HOME/.cargo/bin:$PATH        # cargo 1.95.0 보장
cd /home/from104/work/unim
```

- [ ] `cargo --version` → `cargo 1.95.0 (...)`
- [ ] `make build` 성공, **warning 0건**
- [ ] `cargo test --workspace` → 전부 PASS
- [ ] `sudo make install PREFIX=/usr` 정상 종료 (또는 `make deb` → `sudo dpkg -i`)
- [ ] `systemctl --user daemon-reload && systemctl --user start unim` 후 `systemctl --user is-active unim` → `active`
- [ ] `pgrep -a unim-daemon` 1개만 떠 있는지 확인 (중복 데몬 금지)
- [ ] 로그 초기화: `: > ~/.unim-errors.log`
- [ ] `UNIM_DEVELOP=1` 모드 시작: `pkill -9 unim-daemon; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &`

**실패 시 진단**: `journalctl --user -u unim -b --no-pager | tail -50`, `~/.unim-errors.log` 마지막 100줄 검토.

---

## 1. unim-daemon (핵심 엔진)

### [unim-daemon] 시작·재시작·정상 종료 — 5분
**선행조건**: 0번 사전 준비 완료
**절차**:
1. `systemctl --user restart unim`
2. `pgrep -a unim-daemon` → PID 1개
3. `kill -TERM $(pgrep unim-daemon)` 후 `journalctl --user -u unim --since '1 min ago'`
4. `systemctl --user start unim` 으로 재시작

**기대 결과**: 종료 시 panic·"thread 'main' panicked" 없음. 재시작 후 `busctl --user list | grep org.atit.unim` 표시.
**실패 시 진단**: `journalctl --user -u unim -p err`, `~/.unim-errors.log`.
- [ ] PASS / FAIL

### [unim-daemon] RSS 누수 회귀 — 10분
**선행조건**: 데몬 가동 중, jemalloc + MALLOC_ARENA_MAX=2 적용 확인 (`cat /proc/$(pidof unim-daemon)/environ | tr '\0' '\n' | grep MALLOC`)
**절차**:
1. 시작 RSS 기록: `grep VmRSS /proc/$(pidof unim-daemon)/status`
2. 5분 동안 GTK4 텍스트뷰에서 한국어 입력 + 포커스 이동 반복 (50회)
3. 다시 RSS 측정

**기대 결과**: 증가량 < 30 MB. 64MB 이상 anonymous arena ≤ 2개.
**실패 시 진단**: AGENTS.md "메모리 관리 규칙" 진단 명령 실행.
- [ ] PASS / FAIL

---

## 2. unim-cli

### [unim-cli] --help / locale — 1분
**선행조건**: `LANG=ko_KR.UTF-8` 또는 `en_US.UTF-8` 사용 가능
**절차**:
1. `LANG=ko_KR.UTF-8 unim-cli --help` → 한국어 도움말
2. `LANG=en_US.UTF-8 unim-cli --help` → 영어 도움말
3. `unim-cli --version` → `0.2.0`

**기대 결과**: locale에 따라 메시지 자동 전환, 깨진 글자 없음.
**실패 시 진단**: `locale -a | grep ko_KR`, gettext mo 파일 존재 (`ls /usr/share/locale/ko/LC_MESSAGES/unim*.mo`).
- [ ] PASS / FAIL

### [unim-cli] convert — 1분
**절차**:
1. `unim-cli convert --to-hangul "dkssudgktpdy"` → `안녕하세요`
2. `unim-cli convert --to-english "안녕"` → `dks` 형태의 ASCII

**기대 결과**: 두벌식 표준 매핑.
- [ ] PASS / FAIL

### [unim-cli] config show/set/path/reset — 5분
**절차**:
1. `unim-cli config path` → `~/.config/unim/config.yaml` 경로 출력
2. `unim-cli config show` → 현재 값 트리 출력
3. `cp ~/.config/unim/config.yaml /tmp/unim-config.bak` (백업)
4. `unim-cli config set engine.auto_typefix.enabled false`
5. `unim-cli config show | grep -A1 auto_typefix` → `enabled: false` 확인
6. `unim-cli config set engine.auto_typefix.enabled true` 로 복원
7. (옵션) `unim-cli config reset` 후 `cp /tmp/unim-config.bak ~/.config/unim/config.yaml` 로 원상 복구

**기대 결과**: 변경 즉시 GUI에서도 반영(스위치 위치 동일), 데몬 자동 재로드.
**실패 시 진단**: `cat ~/.unim-errors.log | grep -i config`, `unim-cli config path`로 잘못된 경로 의심.
- [ ] PASS / FAIL

### [unim-cli] config layout list/describe/validate — 3분
**절차**:
1. `unim-cli config layout list` → 10개 빌트인 + 사용자 프로필 표시
2. `unim-cli config layout describe ko_3bul_qwerty` → metadata + rule_sets 출력
3. `unim-cli config layout validate src/keystroke/keymap/ko_3bul390.json` → exit 0
4. (강제 실패) 임의 yaml에 대해 validate → exit 2

**기대 결과**: exit code 0/1/2 정상 (PASS/warning/error).
- [ ] PASS / FAIL

---

## 3. unim-dbus (IPC)

### [unim-dbus] busctl introspect — 2분
**절차**:
1. `busctl --user list | grep org.atit.unim` → 서비스 보임
2. `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod | head -30`

**기대 결과**: `ProcessKeyEvent`, `FocusIn/Out`, `GetHanjaCandidates`, `SelectHanja`, `HanjaBookmarkChanged` 시그널 등 노출.
- [ ] PASS / FAIL

### [unim-dbus] unim-test-dbus 자동 흐름 — 2분
**절차**:
1. `make test-dbus` 실행

**기대 결과**: introspection 출력 후 깔끔하게 종료, "⚠️ unim 서비스 없음" 미출력.
- [ ] PASS / FAIL

### [unim-dbus] DBus 재진입 회귀 — 5분
**선행조건**: GNOME Shell + Wayland (재진입 패턴 재현용)
**절차**:
1. gedit/Discord 등 텍스트 필드에서 한자 후보 팝업 띄우기 (`매`+F9)
2. 팝업 떠 있는 동안 즉시 `1` 키
3. 동일 컨텍스트에서 다시 `매`+F9 → `2`

**기대 결과**: 키 잠금/누락 없이 두 번 모두 한자 커밋. (재진입 방지 큐 패턴 정상)
**실패 시 진단**: `~/.unim-errors.log | grep -i 'reentr\|key_handler\|queue'`.
- [ ] PASS / FAIL

---

## 4. unim-gui-gtk (GTK 설정 GUI)

### [unim-gui-gtk] 시작·시스템 테마 — 2분
**절차**:
1. `unim-gui-gtk` (또는 트레이 메뉴 → 설정)
2. GNOME 다크 토글 → 다이얼로그 자동 다크/라이트 전환

**기대 결과**: `Adw.StyleManager::Default`로 시스템 추종, 깜빡임 없음.
- [ ] PASS / FAIL

### [unim-gui-gtk] 모든 페이지 위젯 탐방 — 10분
**절차**: 좌측 사이드바 페이지를 순서대로 클릭하며 모든 SwitchRow/SpinRow/ComboRow/Scale 토글
1. **General**: 한/영 키, toggle keys, 시작 모드
2. **Korean Layout**: ComboRow에서 `ko_2bulstd → ko_3bul390 → ko_3bul_qwerty` 차례로 변경, rule_sets SwitchRow가 즉시 갱신
3. **English Layout**: `qwerty / dvorak / colemak / colemak_dh / workman` 5종 전환
4. **AutoTypeFix**: 각 SwitchRow 토글, `tentative_expiry_hours` Scale 1~12 슬라이드 (SpinRow 사용 금지 — 슬라이더 권장)
5. **Suppression Words**: Tentative/Confirmed/Inactive 그룹 표시, Confirm/Deactivate/Remove/Reactivate 행 액션
6. **Hanja**: 9칸/81칸 grid 모드 기본값, 책갈피 등록 목록
7. **Reverse Dict**: 사용자 사전 항목 추가/삭제
8. **Per-app Rules**: 앱별 입력 모드 규칙
9. **About**

각 변경 후 `unim-cli config show | grep <key>` 로 즉시 저장 확인.

**기대 결과**: 모든 위젯 변경이 1초 이내 `~/.config/unim/config.yaml`에 반영되며, 데몬이 mtime 핫리로드.
**실패 시 진단**: `UNIM_DEVELOP=1 unim-gui-gtk` 콘솔 stderr, `~/.unim-errors.log`.
- [ ] PASS / FAIL

### [unim-gui-gtk] Suppression Words 행 액션 회귀 — 5분
**절차**:
1. 영문 모드에서 `the`(이미 한국어 자동 교정 트리거) 입력
2. 백스페이스로 롤백 + 한/영 토글
3. 다시 `the` 입력 → 두 번째 시도가 즉시 억제, GUI Tentative 그룹에 추가
4. `Confirm` → Confirmed 그룹으로 이동, 데몬 자동 재로드

**기대 결과**: GUI 행이 사라지고 Confirmed에 등장, 후속 `the` 입력 시 교정 안 됨.
- [ ] PASS / FAIL

---

## 5. unim-gui-qt (Qt 설정 GUI)

### [unim-gui-qt] QML 페이지 탐방 — 10분
**선행조건**: `apt install qt6-base-dev qt6-declarative-dev` 완료, `unim-gui-qt` 설치
**절차**:
1. `unim-gui-qt` 실행
2. 사이드바 모든 QML 페이지 클릭 (General/Layout/AutoTypeFix/Hanja/About 등)
3. GTK GUI와 동일한 항목을 변경 후 `unim-cli config show`로 동기화 검증

**기대 결과**: GTK GUI와 변경값 일치 (5지점 sync 보장). 트레이 메뉴 한국어/영어 토글 동작.
**실패 시 진단**: `QT_LOGGING_RULES='*.debug=true' unim-gui-qt 2>&1 | tee /tmp/qt.log`.
- [ ] PASS / FAIL

---

## 6. unim-frontends/xim (X11 XIM)

### [xim] xterm 한국어 입력 — 5분
**선행조건**: X11 세션 (Xorg or Xwayland), `XMODIFIERS=@im=unim GTK_IM_MODULE=xim QT_IM_MODULE=xim`, `unim-xim` 데몬 가동
**절차**:
1. `pgrep -a unim-xim` 확인 → 없으면 `/usr/libexec/unim-xim &`
2. `XMODIFIERS=@im=unim xterm` 실행
3. xterm에서 한/영 토글 후 `dkssudgktpdy` 입력 → `안녕하세요`
4. `매`+F9 → 한자 후보 팝업 → `1` 선택

**기대 결과**: 인라인 preedit 표시, 커밋 정상, 한자 팝업 위치 caret 아래.
**실패 시 진단**: `xprop -root | grep XIM_SERVERS`, `~/.unim-errors.log | grep -i xim`.
- [ ] PASS / FAIL

### [xim] AutoTypeFix N+1 BS 회귀 — 5분
**선행조건**: xterm에서 영문 모드로 시작
**절차**:
1. xterm 영문 모드에서 `dks` 입력 (한국어가 의도된 ASCII 오타)
2. 한/영 키 → forward AutoTypeFix 발화
3. 출력에 `안`이 표시되고 BS+commit 시퀀스가 정상 (N+1 BS 모델)

**기대 결과**: ASCII가 한글로 교정, 흩날린 BS 잔존 없음. (Chrome preedit edge case는 알려진 이슈 → SKIP)
**실패 시 진단**: `~/.unim-errors.log | grep -E 'typefix|N\+1'`.
- [ ] PASS / FAIL

### [xim] Emacs/터미널 입력 — 3분
**절차**: emacs -nw 에서 두벌식 한국어 입력 + 한자 변환
- [ ] PASS / FAIL

---

## 7. unim-frontends/wayland

### [wayland] weston-text-input-demo — 5분
**선행조건**: 순수 Wayland 컴포지터 (Weston/sway), `unim-wayland` 가동
**절차**:
1. `weston-text-input-demo &`
2. 텍스트 영역에 한국어 입력 → 인라인 preedit
3. focus 변경 → preedit 자동 commit, 다른 영역에서 새 컨텍스트 시작

**기대 결과**: Focus In/Out 시 §2.2/§8.3 시퀀스 준수. 순수 Wayland 한자 popup은 미해결 알려진 이슈 → SKIP.
**실패 시 진단**: `WAYLAND_DEBUG=1 weston-text-input-demo 2>&1 | head -100`.
- [ ] PASS / FAIL

---

## 8. GTK3 IM 모듈

### [im-gtk3] gedit / gtk3-demo 골든패스 — 5분
**선행조건**: `GTK_IM_MODULE=unim`, `/usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/im-unim.so` 존재
**절차**:
1. `GTK_IM_MODULE=unim gtk3-demo` 실행 → "Text View" 데모
2. `dkssudgktpdy` → `안녕하세요`
3. Space 1회 입력 → 공백이 정확히 1개만 들어감 (회귀: 영문 Space 누락 fix 552b5bd)
4. `매` 입력 후 F9 → 한자 후보 팝업 (caret 바로 아래)
5. Period 키로 9칸 ↔ 81칸 grid 토글, ⊞/⊟ 아이콘 변경
6. 후보에서 Space로 책갈피 토글 → ☆ ↔ ★

**기대 결과**: §3.4 키 바인딩 모두 동작. 책갈피 토글 시 다른 GTK4/Qt 팝업도 라이브 갱신 (HanjaBookmarkChanged 시그널).
**실패 시 진단**: `gtk-query-immodules-3.0 | grep unim`, `~/.unim-errors.log`.
- [ ] PASS / FAIL

### [im-gtk3] preedit-end keylock 회귀 — 3분
**선행조건**: ghostty 또는 다른 GTK3 터미널
**절차**:
1. `GTK_IM_MODULE=unim ghostty` (가능하면)
2. 한국어 조합 중 Enter → 커밋 후 즉시 다음 키 입력 (예: `a`)
3. 키 잠금 없는지 확인

**기대 결과**: preedit-end 시그널 정상 발사 → 키 정상 입력. (회귀: 0.2.0 Fixed)
- [ ] PASS / FAIL

---

## 9. GTK4 IM 모듈

### [im-gtk4] gedit/gnome-text-editor — 5분
**선행조건**: `GTK_IM_MODULE=unim`, `/usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so` 존재
**절차**:
1. `GTK_IM_MODULE=unim gnome-text-editor` 실행
2. `dkssudgktpdy` 입력 → `안녕하세요`
3. focus-out (다른 창 클릭) → `늘`이 두 번 커밋되지 않는지 확인 (회귀: 0.2.0 Fixed)
4. 한자 popup, 책갈피, 9×9 토글 (GTK3와 동일 시나리오)

**기대 결과**: focus-out 시 단 1번만 commit. 한자 popup grid 모드 토글 정상.
**실패 시 진단**: `~/.unim-errors.log | grep -i 'focus_out\|duplicate'`.
- [ ] PASS / FAIL

### [im-gtk4] surrounding-text 역방향 교정 — 3분
**절차**:
1. gedit에서 한국어로 단어 입력 후 한/영 토글 → reverse correction 시도
2. surrounding text를 받아 영단어로 교정되는지 확인

**기대 결과**: 한글 → 영문 변환 (회귀: 0.2.0 Fixed `request_surrounding`).
- [ ] PASS / FAIL

---

## 10. Qt5 IM 모듈

### [im-qt5] qt5-test-app 골든패스 — 5분
**선행조건**: `QT_IM_MODULE=unim`, `/usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunim.so` 존재
**절차**:
1. `make sandbox-qt5` 또는 `QT_IM_MODULE=unim ./tests/unim-test-qt5/build/unim-test-qt5`
2. 한국어 입력 + 한자 popup + 81칸 grid 토글

**기대 결과**: §3.5 동작 + caret 위치 정상.
- [ ] PASS / FAIL

---

## 11. Qt6 IM 모듈

### [im-qt6] qt6-test-app — 5분
**선행조건**: `QT_IM_MODULE=unim`, qt6 plugin 설치됨
**절차**:
1. `make sandbox-qt6`
2. Qt5와 동일 시나리오

**기대 결과**: Qt5와 1:1 동등. 트레이 GUI(Qt) 와의 충돌 없음.
- [ ] PASS / FAIL

---

## 12. unim-gnome-extension

### [gnome-ext] 활성화 + 인디케이터 — 3분
**선행조건**: GNOME 45+ Wayland 세션, `make dev-extension` 후 로그아웃→로그인
**절차**:
1. `gnome-extensions list --enabled | grep unim`
2. 상단 패널 트레이에 한/영 인디케이터 표시
3. 클릭 → 한국어/영어 토글, 아이콘 즉시 갱신

**기대 결과**: GlobalModeChanged 시그널 동기화.
**실패 시 진단**: `journalctl --user /usr/bin/gnome-shell --since '5 min ago' | grep -i unim`.
- [ ] PASS / FAIL

### [gnome-ext] prefs.js 옵션 — 3분
**절차**: `gnome-extensions prefs unim@from104.github.io`
- [ ] 5개 GNOME 전용 옵션 모두 표시 + 변경 즉시 저장
- [ ] 더 이상 dead feature 옵션이 보이지 않음 (Phase 8 cleanup 검증)
- [ ] PASS / FAIL

### [gnome-ext] 한자 popup Push 모드 — 5분
**선행조건**: Wayland + GNOME, popup_mode=Standalone
**절차**:
1. Firefox/Discord 등 Wayland 네이티브 앱에서 `매`+F9
2. GNOME Extension이 Push 방식 한자 popup 표시 (St 위젯)
3. `1`~`9`/`Period`/`Space` 모두 동작

**기대 결과**: Push 모드에서도 §3.4 모든 바인딩 동작, focus 이동 시 자동 닫힘.
**실패 시 진단**: GNOME Looking Glass(`Alt+F2 → lg`)에서 `Main.panel._unim` 검사.
- [ ] PASS / FAIL

### [gnome-ext] 이모지 popup (Super+.) — 3분
**선행조건**: 0.2.0 신규 기능
**절차**:
1. 텍스트 필드에서 `Super+.` → 이모지 popup
2. 카테고리 탭 이동, 검색 박스에 `smile` 입력 → 후보 좁혀짐
3. Enter로 커밋

**기대 결과**: MRU favorites 탭 갱신, popup 닫힘.
- [ ] PASS / FAIL

---

## 13. unim-windows / unim-tsf (옵션)

### [windows-tsf] cross-compile check — 3분
**선행조건**: `rustup target add x86_64-pc-windows-gnu`, `apt install mingw-w64`
**절차**:
1. `WIN_TARGET=x86_64-pc-windows-gnu make check-windows`

**기대 결과**: cargo check 0 warning / 0 error.
- [ ] PASS / FAIL

### [windows-tsf] 메모장 입력 (옵션, 별도 Windows VM) — 10분+
**절차**:
1. `make build-windows` → `target/x86_64-pc-windows-gnu/release/` 산출물 복사
2. Windows에 설치 후 메모장에서 한국어 입력 + 한자 변환

**기대 결과**: 두벌식 입력 정상.
- [ ] PASS / FAIL

---

## 14. 회귀 시나리오 통합 (0.2.0 Fixed 매핑)

| ID | 회귀 항목 | 검증 위치 | 결과 |
|----|----------|-----------|------|
| R1 | 영문 Space 누락 (gedit) | §9 GTK4 골든패스 | [ ] |
| R2 | focus-out 이중 commit `늘늘` | §9 GTK4 focus-out | [ ] |
| R3 | tentative_expiry days→hours | §4 Suppression Words | [ ] |
| R4 | gedit surrounding-text 역방향 | §9 GTK4 surrounding | [ ] |
| R5 | GTK preedit-end keylock | §8 GTK3 ghostty | [ ] |
| R6 | XIM AutoTypeFix N+1 BS | §6 XIM AutoTypeFix | [ ] |
| R7 | reverse blacklist 빈 문자열 등록 | §4 Suppression 행 액션 | [ ] |
| R8 | DBus call_sync 재진입 | §3 DBus 재진입 | [ ] |
| R9 | RSS 누수 (jemalloc + ARENA_MAX) | §1 RSS 회귀 | [ ] |

---

## 15. 환경 매트릭스 (수동 전용)

각 조합에서 §8(GTK3) + §9(GTK4) + §10/11(Qt5/6) + §12(GNOME ext) 골든패스 1회씩 수행.

| 조합 | gedit 한국어 | 한자 popup | AutoTypeFix | 결과 |
|------|--------------|------------|-------------|------|
| X11 + GNOME (Xorg session) | [ ] | [ ] | [ ] | [ ] |
| X11 + KDE Plasma | [ ] | [ ] | [ ] | [ ] |
| Wayland + GNOME | [ ] | [ ] | [ ] | [ ] |
| Wayland + KDE Plasma | [ ] | [ ] | [ ] | [ ] |

---

## 16. 마무리 정리

- [ ] `~/.unim-errors.log` 에 ERROR/PANIC 레벨 0건
- [ ] `journalctl --user -u unim -p err -b` 출력 없음
- [ ] `pgrep -a unim-` 가 의도된 데몬만 표시 (좀비/중복 없음)
- [ ] `git status` 작업 트리 변경 없음 (테스트 중 우발 수정 방지)
- [ ] 결과 요약을 `_workspace/release/01_test_plan_report.md`에 기재
