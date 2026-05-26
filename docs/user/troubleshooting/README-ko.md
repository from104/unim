# UNIM 트러블슈팅 (한국어)

> UNIM 0.3.0 — 증상 → 1차 진단 → 2차 명령 → 해결 순서로 정리.
> "한 번도 한글이 안 나간다"부터 "특정 앱에서만 깨진다"까지 자주 마주치는 14개 증상을 다룬다.

전체 진단의 출발점은 두 가지다. 하나는 **데몬이 살아 있는가**, 다른 하나는 **로그가 무엇이라고 말하는가**.

```bash
# (1) 데몬 살아있나?
systemctl --user status unim-daemon
# 또는 PID 확인
unim-daemon --check && echo "RUNNING" || echo "STOPPED"

# (2) 디버그 로그 켜고 재현
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
> ~/.unim-errors.log    # 비우고
# … 문제 재현 …
tail -f ~/.unim-errors.log
```

> `UNIM_DEVELOP=1`은 Engine·DBus·Frontend·Extension 전 컴포넌트의 로그를 한 파일(`~/.unim-errors.log`)로 모은다. 일반 사용 시에는 OFF가 기본 — 로그 파일이 무한히 커지지 않게 하기 위함.

---

## 1. "한글이 아예 안 나옴" — 새로 설치한 직후

### 1차 진단

```bash
echo $GTK_IM_MODULE      # unim 이어야 함
echo $QT_IM_MODULE       # unim 이어야 함
echo $XMODIFIERS         # @im=unim 이어야 함
unim-daemon --check && echo OK || echo MISSING
```

### 원인별 처방

| 증상 | 원인 | 해결 |
|------|------|------|
| 환경변수 비어 있음 | im-config 설정 누락 | `im-config -n unim` 후 로그아웃/로그인 |
| `unim-daemon --check` → MISSING | systemd unit 미등록 | `systemctl --user enable --now unim-daemon` |
| 셸에는 보이는데 GUI 앱엔 적용 안 됨 | DM이 환경변수를 안 가져감 | `~/.xprofile` 또는 `/etc/environment`에 export |
| GNOME+Wayland | 환경변수 경로가 막힘 | 대신 `gnome-extensions enable unim-gnome@from104.github.io` |

---

## 2. "GTK 앱(GNOME Text Editor 등)에서만 안 됨"

### 진단

```bash
# IM 모듈이 설치돼 있나?
ls /usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/im-unim.so 2>/dev/null
ls /usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so 2>/dev/null

# 모듈 캐시 갱신 필요?
sudo gtk-query-immodules-3.0 --update-cache
sudo gtk-query-immodules-4.0 --update-cache
```

### 처방

- 파일이 없으면 `unim-im-gtk` 패키지 재설치 또는 `sudo make install PREFIX=/usr` 재실행.
- 파일은 있지만 작동 안 하면 캐시 갱신 후 GTK 앱 재시작.
- GTK4 모듈 파일명이 `libim-unim.so`인지 반드시 확인 (GTK3는 `im-unim.so`, **접두 `lib` 유무가 다름**).

> 디버깅 팁: `GTK_IM_MODULE_FILE=/usr/lib/.../immodules.cache GTK_IM_MODULE=unim gnome-text-editor` 식으로 직접 띄워 보면 모듈 로드 단계 에러가 stderr에 보인다.

---

## 3. "Qt 앱(Kate, Krita)에서만 안 됨"

### 진단

```bash
ls /usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
ls /usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
QT_DEBUG_PLUGINS=1 kate 2>&1 | grep -i unim
```

### 처방

- 플러그인 부재 → `unim-im-qt` 재설치.
- `QT_DEBUG_PLUGINS=1` 출력에서 `Cannot load library` 등 메시지가 보이면 의존 라이브러리 누락 → `ldd <plugin>.so`로 확인.
- KDE Plasma 6에서는 Qt6 경로가 우선이니 `QT_IM_MODULE=unim`만 잘 잡혀 있으면 된다.

---

## 4. "GNOME 확장이 메뉴에 안 보임"

### 진단

```bash
gnome-extensions list | grep unim
gnome-extensions info unim-gnome@from104.github.io
journalctl --user -u gnome-shell -b | grep -i unim
```

### 처방

- 디렉토리 확인: `~/.local/share/gnome-shell/extensions/unim-gnome@from104.github.io/` 존재해야 함.
- 없으면 `make dev-extension`(소스 빌드) 또는 `unim-gnome` 패키지 설치.
- 활성화: `gnome-extensions enable unim-gnome@from104.github.io` → Alt+F2 → `r` (X11) 또는 로그아웃/로그인 (Wayland).
- GNOME Shell 버전 호환: `metadata.json`의 `shell-version` 배열에 현재 버전이 들어 있는지 확인.

---

## 5. "한자 팝업이 안 뜸"

### 진단

```bash
# 현재 팝업 모드 확인
unim-cli config get popup_mode

# DBus 시그널이 발행되고 있나? (별도 터미널에서 monitor)
busctl --user monitor org.atit.unim.InputMethod
# 그 상태에서 한글 입력 + 한자 키 → ShowHanjaPopup 시그널이 보여야 함
```

### 처방

| 환경 | 팝업 렌더 주체 (0.3.0+) | 비고 |
| ---- | ----------------------- | ---- |
| GNOME+Wayland | GNOME extension `popup_view.js` (St 위젯) | 확장이 `PopupRender` 시그널 받아 직접 그림 |
| GNOME X11 / KDE / Xfce / WM (X11) | `unim-popup-service` (GTK4) | D-Bus auto-activation으로 자동 기동 |
| Wayland (KDE Plasma 6 / Sway 등) | `unim-popup-service` (GTK4, wayland-backend) | `libgtk4-layer-shell` 필요, 실험적 — [팝업 명세](../../dev/specs/POPUP_SPEC.md) §12 참고 |

> 0.3.0부터 IM 모듈은 더 이상 자체 팝업을 그리지 않는다. 한자·특수문자·이모지 팝업 렌더링은
> 전부 `unim-popup-service`(또는 GNOME extension)로 중앙화됐다. 따라서 진단도 popup_mode가
> 아니라 렌더러 프로세스가 살아 있는지를 본다.

```bash
# X11/KDE/Xfce: popup-service 가 떠 있나? (없으면 §16 참고)
pgrep -a unim-popup
# GNOME Wayland: extension 이 활성인가?
gnome-extensions list --enabled | grep unim
```

> **DBus가 죽었을 때**: `busctl --user list | grep atit` → 비어 있으면 데몬이 서비스 등록을 못 한 것. `journalctl --user -u unim-daemon -n 100` 로그 확인.

---

## 6. "특수문자 팝업이 안 뜸"

원인 거의 동일 (한자 팝업과 같은 코드 경로).

```bash
# 한글 모드인지, 자음 1글자만 입력한 상태인지 확인
unim-cli config get current_mode    # Korean 이어야 함
```

ㄱ~ㅎ 입력 후 한자 키. 한국어 자판이 두벌식이면 잘 동작, 세벌식이면 자음 입력 자체가 다를 수 있음.

---

## 7. "한자 팝업이 9칸까지만 보이고 81칸 토글이 안 됨"

### 진단

```bash
# 마침표 키가 한자 팝업에 도달하는지 확인
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
# 한자 팝업 띄우고 . 누른 후
grep -i 'ToggleExpanded\|9x9\|expanded' ~/.unim-errors.log
```

### 처방

- 0.1.x 시절에 빌드된 IM 모듈이 잔류해 있을 가능성 — `make build && sudo make install` 재실행.
- 키보드 레이아웃에서 `.`이 다른 키로 매핑돼 있는지 확인.

---

## 7-1. "Wayland에서 ◀/▶ 페이지 버튼이 보이는데 마우스 클릭이 안 먹음"

### 증상

`unim-frontends/wayland`로 동작하는 popup(컴포지터: GNOME mutter, KWin, Sway 등)에서 ◀/▶ 버튼이 시각적으로 분명히 그려지는데 마우스 좌클릭에 반응이 없다.

### 원인

Wayland popup은 `zwp_input_popup_surface_v2` 위에 그려진다. 이 surface로 pointer 이벤트가 들어오려면 컴포지터가 IM popup으로 pointer 라우팅을 허용해야 하는데, 일부 컴포지터(특히 GNOME mutter 일부 버전)는 IM popup을 "pass-through" 처리해 클릭이 아래 앱으로 빠진다.

### 처방

- **즉시 우회**: 키보드 `←` / `→` (또는 `Page Up` / `Page Down`)로 페이지 이동. 마우스 ◀/▶와 100% 동일 동작.
- **GNOME 사용자**: GNOME Shell 확장 popup으로 자동 전환되므로 ◀/▶ 클릭이 정상 동작 (`unim-gnome-extension`이 mutter 외부에서 자체 그린다). 만일 GNOME에서도 안 되면 확장이 비활성 또는 미설치 상태인지 `gnome-extensions list --enabled | grep unim`으로 확인.
- **장기 대응**: 컴포지터 측 IM popup pointer 라우팅 지원 여부에 의존. 보고는 [GitHub Issues](https://github.com/from104/unim/issues)에 컴포지터 이름·버전 명시.

> 키보드 ←/→ 동작은 모든 컴포지터에서 보장된다. 마우스 ◀/▶는 컴포지터 정책상 best-effort.

---

## 7-2. "XIM 이모지 popup의 ◀/▶가 카테고리 탭과 겹치게 동작"

### 증상

XIM(`unim-frontends/xim`)에서 이모지 popup을 띄우면 카테고리 탭(스마일·동물·음식 …)과 ◀/▶ 페이지 버튼이 같은 영역 근처에 동시에 표시된다. "어느 게 누른 거지?" 헷갈린다.

### 원인 / 정상 여부

**정상 동작이다.** XIM 이모지 popup은 (1) 상단에 카테고리 탭(좌클릭으로 카테고리 전환), (2) 하단에 ◀/▶(좌클릭으로 한 페이지 이동) 두 컨트롤을 모두 가진다. 둘 다 좌클릭이지만 영역이 다르다 — 카테고리 탭은 popup 상단, ◀/▶는 footer.

### 처방

- 좌클릭 영역을 명확히 인지: 카테고리는 위, 페이지 이동은 아래.
- 헷갈리면 키보드로 처리:
  - **카테고리 전환**: `Tab` (다음) / `Shift+Tab` (이전).
  - **페이지 이동**: `←` / `→` (또는 `Page Up`/`Page Down`).

> 정상 동작이지만 시각적 분리가 부족하다는 피드백은 받고 있다 — 향후 footer 색 분리 예정.

---

## 8. "AutoTypeFix가 작동 안 함"

### 진단

```bash
unim-cli config get auto_typefix.enabled              # true 인가
unim-cli config get auto_typefix.forward.enabled      # 방향별
unim-cli config get auto_typefix.reverse.enabled
cat ~/.config/unim/typefix-blacklist.yaml | head -50  # 등록된 억제 단어
```

### 처방

- 마스터 토글 OFF → 설정 GUI에서 ON.
- 특정 단어가 자꾸 억제 사전에 들어가면 GUI 「교정 억제 단어」 페이지 → 「비활성」 또는 「삭제」.
- 단어 경계(스페이스/마침표)가 들어와야 비로소 교정이 적용된다. 한 글자 더 친 뒤에 스페이스를 넣어 보면 즉시 교정됨.
- 영문 모드에서 작동 안 함 → 「영문 모드 시 무시」가 ON이라 그렇다 (의도된 기본값).

---

## 9. "AutoTypeFix가 너무 자주 잘못 교정"

### 처방

| 케이스 | 해법 |
|--------|-----|
| 특정 단어 한 개만 문제 | BS + 한/영로 롤백 → 다음에 같은 단어 칠 때 자동으로 Tentative 등록됨 |
| 자주 후회한다 | 설정 → 「오타 교정」 → 임시 만료 시간을 늘려 학습 기회 확보 |
| 영문 모드에서도 reverse가 동작 | `auto_typefix.reverse.skip_incomplete_syllable` ON |
| 직접 단어 추가 | `~/.config/unim/typefix-blacklist.yaml`을 텍스트 에디터로 편집 → 데몬이 mtime 자동 리로드 |

---

## 10. "설정이 저장 안 됨 / 변경이 안 먹힘"

### 진단

```bash
ls -la ~/.config/unim/
test -w ~/.config/unim/config.yaml && echo writable || echo BLOCKED
unim-cli config list 2>&1 | head -5
journalctl --user -u unim-daemon -n 50
```

### 처방

- 권한 문제 → `chmod 644 ~/.config/unim/*.yaml`, `chmod 755 ~/.config/unim`.
- `~/.config/unim`을 root가 만든 흔적이 있으면 `sudo chown -R $USER:$USER ~/.config/unim`.
- GUI에서 변경했는데 데몬에 반영 안 됨 → 데몬 재시작: `systemctl --user restart unim-daemon`.

---

## 11. "키 입력 자체가 잠겼다 (ghostty/터미널)"

증상: 한 키만 친 뒤 화면이 멈춘 듯 키가 안 먹힘.

### 원인

GTK3/4 IM의 `preedit-end` 시그널 누락으로 ghostty가 IM 잠금 상태에 빠짐(0.1.x 잔존 이슈, 0.2.0에서 `unim_emit_preedit` 헬퍼로 해결).

### 처방

```bash
# 0.2.0 빌드인지 확인
dpkg -l | grep unim       # 또는 unim-cli --version
unim-cli --version        # 0.2.0 이상이면 해결됨
```

`0.1.x`이면 `make build && sudo make install` 후 IM 모듈 갱신.

---

## 12. "Flatpak 앱(Telegram, VS Code)에서 한글 안 됨"

### 진단

```bash
flatpak list --columns=application,environment | grep -E 'GTK_IM|QT_IM'
```

### 처방

GNOME+Wayland인 경우 호스트 환경변수가 Flatpak 샌드박스에 새어들면 입력이 막힌다. 자동 처리가 작동했으면 비어 있어야 한다.

```bash
# 자동 처리 확인 — daemon 시작 로그
journalctl --user -u unim-daemon | grep -i flatpak
# 다음 두 줄이 보이면 정상:
#   [Flatpak] GNOME+Wayland 감지 — Flatpak IM 환경변수 설정 시작
#   [Flatpak] IM 환경변수 override 완료

# 수동 적용
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
flatpak kill org.telegram.desktop
```

X11이거나 GNOME이 아닌 경우는 반대로 환경변수를 **유지**해야 한다 — 자동 처리는 GNOME+Wayland에서만 발동한다.

---

## 13. "Snap 앱에서 한글 안 됨"

Snap은 호스트 환경변수를 그대로 상속하지만 전역 override 메커니즘이 없다.

### 처방

`~/.profile`에 조건부 export:

```bash
if [ "$XDG_SESSION_TYPE" = "wayland" ] && echo "$XDG_CURRENT_DESKTOP" | grep -q "GNOME"; then
    export GTK_IM_MODULE=
    export QT_IM_MODULE=
else
    export GTK_IM_MODULE=unim
    export QT_IM_MODULE=unim
fi
export XMODIFIERS="@im=unim"
```

또는 한 번만 띄울 때:

```bash
QT_IM_MODULE= GTK_IM_MODULE= snap run telegram-desktop
```

---

## 14. "데몬이 메모리 너무 많이 먹음 (RSS 500MB↑)"

### 진단

```bash
grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
cat /proc/$(pidof unim-daemon)/smaps_rollup | grep -E 'Rss|Anonymous'
```

### 처방

UNIM 0.2.0은 `tikv_jemallocator` + `MALLOC_ARENA_MAX=2` + 60초 주기 `malloc_trim(0)`로 1MB 단위 안정 운영을 보장한다. 그럼에도 RSS가 500MB를 초과하면:

```bash
# 임시 회복
systemctl --user restart unim-daemon

# 진단 데이터 수집 (이슈 보고용)
ps -o pid,rss,vsz,cmd $(pidof unim-daemon)
journalctl --user -u unim-daemon -n 500 > unim-mem.log
```

[`AGENTS.md` §메모리 관리 규칙](../../dev/architecture/AGENTS.md)이 회귀 금지 항목과 진단 명령을 모두 정리해 둔다. 이슈로 보고하면 도움이 된다.

---

## 15. "모아치기(chord)가 제대로 인식 안 됨"

모아치기 관련 문제는 원인이 다양하다. 아래 항목을 순서대로 확인한다.

### 15-1. 현재 자판이 모아치기를 지원하지 않음

모아치기는 `supports_moachigi: true`인 자판에서만 동작한다. 빌트인 중에는 **안마태 자판 (ko_3bul_anmatae)** 만 해당된다. **쿼티형 세벌식 v2** 는 빌트인이 아닌 연구 자료(`docs/references/keymaps/ko_3bul_qwerty_v2.json`)로 보존되며, `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사한 사용자 프로필에서 모아치기 지원을 켤 수 있다.

```bash
# 현재 자판 확인
unim-cli config show | grep -E 'layout|keymap'
```

출력이 모아치기 지원 자판이 아니라면 GTK 설정에서 자판을 변경해야 한다. 모아치기 지원 자판으로 바꾸면 설정 다이얼로그에 **모아치기** 그룹이 자동으로 나타난다.

### 15-2. chord_window_ms가 너무 짧음

`chord_window_ms` 기본 권장값은 **60ms**다. 처음 모아치기를 시작하거나 입력 속도가 빠르지 않다면 **80~100ms** 부터 시작해 익숙해지면 줄여 나가는 것이 좋다.

```bash
# 현재 설정값 확인
unim-cli config get korean.chord_window_ms

# 80ms로 변경
unim-cli config set korean.chord_window_ms 80
```

또는 GTK 설정 다이얼로그 > 자판 > **동시 입력 시간 (ms)** 슬라이더로 조정한다.

### 15-3. bidirectional_combine이 비활성 상태

자모 역순 결합(예: ᆯ+ᆨ → ᆰ, ㅎ+ㄱ → ㅋ)이 안 된다면 **양방향 자모 결합** 옵션이 꺼져 있는 것이다.

```bash
# 현재 상태 확인
unim-cli config get korean.bidirectional_combine

# 활성화
unim-cli config set korean.bidirectional_combine true
```

또는 GTK 설정 다이얼로그 > 자판 > **양방향 자모 결합** 토글을 ON.

### 15-4. 키보드가 NKRO를 지원하지 않음 (ghosting)

일반 멤브레인 키보드는 2~3 KRO(Key Rollover) 한계로 인해 여러 키를 동시에 눌렀을 때 일부 키가 운영체제에 전달되지 않는다(ghosting). chord가 누락되거나 엉뚱한 자모로 조합된다면 키보드 자체가 동시 입력을 지원하지 못하는 것일 수 있다.

자가 진단:

```sh
# X11 환경에서 동시 키 이벤트 확인
xev -event keyboard
```

Wayland 환경에서는 `xev` 대신 `wev` 사용 (`apt install wev` 또는 동등).

나타나는 창에 포커스를 두고 chord에서 사용하는 키를 모두 동시에 누른다. 터미널에 `KeyPress event`가 키 수만큼 모두 찍혀야 한다. 또는 브라우저에서 [keyboardchecker.com](https://keyboardchecker.com) 등 온라인 키 테스터를 사용한다.

해결: 게이밍 키보드 또는 메커니컬 키보드의 NKRO 모드 사용 권장. 자세한 내용은 [안마태 자판 가이드 — 키보드 호환성](../keymaps/anmatae.md#키보드-호환성-nkro-권장) 참고.

### 15-5. USB 폴링 레이트 낮음 (125Hz = 8ms 분해능)

USB 키보드의 기본 폴링 레이트는 125Hz(= 8ms 간격)다. `chord_window_ms`를 10~30ms처럼 짧게 설정하면 폴링 주기 자체가 윈도우의 상당 부분을 차지해 일부 키를 놓칠 수 있다.

해결:

- `chord_window_ms`를 **60ms 이상**으로 올려 폴링 지연 여유분 확보.
- 1000Hz 폴링 지원 게이밍 키보드 사용, 또는 USB 포트 변경.

---

## 빌드 실패

### 일반 빌드 실패

```bash
make clean
make build 2>&1 | tee /tmp/unim-build.log
```

| 에러 메시지 | 원인 | 처방 |
|------|------|------|
| `lock file version 4 requires '-Znext-lockfile-bump'` | cargo 1.75 등 구 버전 | `rustup update stable`로 cargo 1.95+ 사용 |
| `gtk4/libadwaita not found` | 개발 헤더 누락 | `sudo apt install libgtk-4-dev libadwaita-1-dev` |
| `Qt6Core not found` | Qt6 dev 누락 | `sudo apt install qt6-base-dev` |
| `cxx-qt build error` | Qt 헤더 경로 어긋남 | `pkg-config --cflags Qt6Core` 확인 |
| 경고 발생 | UNIM은 zero-warning 정책 | 경고 메시지 그대로 이슈로 보고 |

> 빌드 명령은 `make build`가 정본. `cargo build --workspace`만으로는 C/C++ 프론트엔드가 빠진다.

---

## 진단 데이터 수집 (이슈 보고 시 필수)

```bash
{
  echo "=== version ==="
  unim-cli --version
  echo "=== env ==="
  echo "session=$XDG_SESSION_TYPE"
  echo "desktop=$XDG_CURRENT_DESKTOP"
  env | grep -E 'GTK_IM|QT_IM|XMOD' | sort
  echo "=== daemon ==="
  systemctl --user status unim-daemon --no-pager
  echo "=== config ==="
  unim-cli config list
  echo "=== logs (last 200) ==="
  tail -n 200 ~/.unim-errors.log 2>/dev/null
} > unim-report.txt
```

이 `unim-report.txt`를 이슈에 첨부하면 진단이 빨라진다. 비밀번호·토큰이 로그에 들어갔을 가능성은 낮지만, 첨부 전에 한 번 훑어보길 권장.

---

## 더 읽을 거리

- [FAQ](../faq/README-ko.md) — 다른 IME와의 차이, 동시 설치, 백업 복원
- [사용자 매뉴얼](../user-guide/README-ko.md) — 설정 GUI 페이지별 상세
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — 동작 명세 (개발자용)
- [`AGENTS.md`](../../dev/architecture/AGENTS.md) — 아키텍처와 메모리 관리 규칙

---

## 16. popup-service 디버깅 (0.3.0+)

### 증상: 한자·특수문자·이모지 팝업이 전혀 안 뜸 (GNOME X11 또는 KDE/Xfce)

0.3.0부터 팝업은 `unim-popup-service`가 단독으로 렌더한다. 데몬은 살아 있어도 popup-service가 없으면 팝업이 나타나지 않는다.

#### 진단 명령

```bash
# popup-service 프로세스 확인
pgrep -a unim-popup

# DBus 인터페이스 노출 확인
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup

# D-Bus 서비스 파일 설치 확인
ls ~/.local/share/dbus-1/services/org.atit.unim.PopupService.service \
   /usr/share/dbus-1/services/org.atit.unim.PopupService.service 2>/dev/null
```

#### 해결 방법

- 서비스 파일이 없으면 `unim-popup-service` 패키지가 설치되지 않은 것이다.

  ```bash
  # deb 설치
  sudo apt install ./unim-popup-service_0.3.0_amd64.deb
  # 또는 소스 빌드 후
  sudo make install PREFIX=/usr
  ```

- 서비스 파일이 있는데도 팝업이 안 뜨면 수동으로 기동해 로그를 확인한다.

  ```bash
  UNIM_DEVELOP=1 unim-popup-service &
  # 이후 한자 팝업 트리거 → 터미널 로그 확인
  ```

- `busctl` introspect가 실패하면 `org.atit.unim.PopupService` 자체가 응답하지 않는 것이다. `systemctl --user status unim-popup-service` 또는 `journalctl --user -t unim-popup-service -b --no-pager`로 오류 원인을 확인한다.

### 증상: 팝업이 클릭하자마자 바로 닫힘

팝업 **외부** 좌클릭 시 팝업이 닫히는 것은 **의도된 동작**이다. 클릭 이벤트는 아래 창에 그대로 전달된다. 팝업 내부 셀·버튼 영역을 클릭하면 닫히지 않는다. 팝업 내부를 클릭했는데도 닫힌다면 팝업 크기·위치가 잘못 계산된 것이다 — DBus caret 좌표(`caret_rect`) 값을 확인한다.

### 증상: GNOME Wayland에서 팝업이 두 번 뜸

`Meta.is_wayland_compositor()` 감지가 실패해 extension PopupView와 popup-service GTK4 팝업이 동시에 열리는 경우다. GNOME Shell 버전을 확인하고, 확장(`unim-gnome@from104.github.io`)을 비활성화 후 재활성화해 본다.

### 증상: XIM ON-THE-SPOT commit 직후 preedit 누락

0.3.0에서 `commit_then_preedit` best-effort 적용이 완료됐다. XTerm·WezTerm 등 OVER-THE-SPOT 환경에서는 정상 복귀됐다. 일부 ON-THE-SPOT(PREEDIT_CALLBACKS) 클라이언트에서는 회귀가 잔존한다. 이는 xim-0.5.0 crate 내부 `commit()`이 `preedit_started` 상태머신을 갱신하지 않는 근본 원인 때문이며, 현재 알려진 미해결 이슈다. OVER-THE-SPOT 환경(XTerm 등)으로 우회하거나 GTK/Qt IM 모듈을 사용한다.

---

## 0.2.0 릴리스 특이 진단

> 0.2.0 릴리스 직전 manual-test-planner가 작성한 추가 진단 시나리오. 위 §1–§14와 중복되는 항목은 사용자 README가 우선이며, 아래는 보조 진단 도구·세부 회귀 케이스 위주로만 남긴다.

### A. 진단 공통 도구 (보조)

| 명령 | 용도 |
|------|------|
| `journalctl --user -u unim -b --no-pager` | 데몬 systemd 로그 (이번 부팅) |
| `: > ~/.unim-errors.log; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &` | 로그 초기화 + 개발자 모드 재시작 |
| `pgrep -a unim-` | 모든 unim-* 프로세스 목록 |
| `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod` | DBus API 노출 확인 |

### B. 0.2.0에서 회귀 금지 케이스

- **gedit 이중 commit (`늘늘`)**: focus-out 시 CommitText 시그널 broadcasting 회귀가 0.2.0에서 fix됨. 재현 시 `~/.unim-errors.log | grep -i 'commit_text\|focus_out'`.
- **영문 모드 Space 누락 (gedit)**: 0.2.0에서 `consumed=true commit=" "` 경로로 수정. 회귀 시 `engine_worker.rs` Space 처리 분기 점검.
- **AutoTypeFix 잔존 BS (XIM)**: 0.2.0 N+1 BS 모델로 수정됨. Chrome preedit edge case는 알려진 SKIP.
- **`tentative_expiry_hours` 단위**: 0.2.0부터 days→hours로 변경 (1..=12). 기존 config는 자동 마이그레이션.
- **XIM ON-THE-SPOT(PREEDIT_CALLBACKS) 모드 commit 직후 preedit 누락 (미해결)**: 한글 음절 commit 직후 새 자모를 입력해도 preedit가 한 프레임 가시화되지 않다가 추가 자모가 들어와 음절이 형성되어야 보이기 시작. 영향 범위 — 자체 XIM client(예: `unim-test-xim`)·일부 ON-THE-SPOT XIM 앱. **무영향** — XTerm·WezTerm·기타 OVER-THE-SPOT(PreeditPosition) 클라이언트, GTK3/4·Qt5/6·Wayland·GNOME extension 모두 정상. `commit_then_preedit()` 에서 commit 직전 `clear_preedit()` 강제(xim-0.5.0의 PreeditDone 자동 발사) best-effort 적용했으나 일부 ON-THE-SPOT 클라이언트에서 회귀 잔존. xim crate 의 `commit()` 이 `preedit_started` 상태를 갱신하지 않는 점이 근본 원인 — crate 측 fix 또는 별도 protocol 시퀀스 재설계 필요. 추적: `unim-frontends/xim/src/handler.rs:378-`, `xim-0.5.0/src/server.rs:236-248`.

### C. 데몬 다중 인스턴스

- `pkill -9 -x unim-daemon; sleep 1; systemctl --user start unim`
- DBus 자동 활성화 + 수동 실행이 겹치는 경우 발생. 수동 실행 시 `--replace` 플래그 사용.

### D. 한자 popup 좌표

- caret_rect 미수신: `cursor_y = 0` fallback. POPUP_SPEC §6.3 좌표 소스 확인.
- 9칸 ↔ 81칸 토글 안 됨: Period(.) 키가 다른 곳에서 가로채짐. 키맵 확인.
- 책갈피(★) 동기화: `HanjaBookmarkChanged` 시그널 미수신. `busctl --user monitor org.atit.unim.InputMethod`로 시그널 흐름 확인.

### E. CLI 한국어 깨짐

- locale 미설치: `sudo locale-gen ko_KR.UTF-8`
- gettext mo: `ls /usr/share/locale/ko/LC_MESSAGES/unim*.mo`

### F. `unim-cli config set` 후 GUI 미반영

- 데몬이 mtime 핫리로드 못함 → `pkill -SIGHUP unim-daemon`
- 5지점 sync 깨짐 가능성 → CLI/엔진/GUI/locale/dbus 5점 모두 갱신됐는지 점검.

### G. 환경 매트릭스 (0.3.0 시점)

| 환경 | 지원 상태 | 비고 |
|------|----------|------|
| GNOME Wayland | ✅ 검증 | GNOME extension `popup_view.js` (St 위젯) 자체 렌더 |
| GNOME X11 | ✅ 검증 | popup-service GTK4 + GNOME extension 보조 |
| X11 + KDE Plasma 5.x | ✅ 검증 | popup-service GTK4 |
| X11 + XFCE / MATE / Cinnamon / LXDE | ✅ 검증 | popup-service GTK4 |
| Wayland + KDE Plasma 5.x | ❌ 미지원 | `gtk4-layer-shell` 미배포 (Ubuntu 24.04 noble 표준 저장소) → X11 세션 / GNOME 우회 |
| Wayland + KDE Plasma 6 | ⚠️ 실험적 | `wayland-backend` feature + `libgtk4-layer-shell` 필요. 0.3.0 QA 미검증 |
| Sway / Hyprland / river (단독 Wayland) | ⚠️ 실험적 | 동상. popup 위치·IME 포커스 회귀 가능 |
| Weston 등 reference Wayland | ⚠️ 실험적 | 동상 |

⚠️ 실험적 환경에서 문제 발견 시 [GitHub Issues](https://github.com/from104/unim/issues) 로 제보 부탁드립니다.

### H. 로그 분석 슬래시 명령

```bash
# Claude Code 사용 시
/unim-log
```
→ `~/.unim-errors.log` 자동 분류·요약·진단.
