# UNIM 트러블슈팅 (한국어)

> UNIM 0.4.0 — 증상 → 1차 진단 → 2차 명령 → 해결 순서로 정리.
> "한 번도 한글이 안 나간다"부터 "특정 앱에서만 깨진다"까지 자주 마주치는 증상들을 다룬다.

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

> Windows 지원은 v0.4.0에서 추가됐다. 아래 절차는 확인된 동작만 담았고, 여기 없는 증상은 [GitHub Issues](https://github.com/from104/unim/issues)로 제보 부탁드린다.

Windows에는 **데몬도 DBus도 없다.** UNIM은 **TSF**(Text Services Framework — Windows가 입력기를 앱에 연결하는 표준 방식) 텍스트 서비스 `unim_tsf.dll`이며, 글자를 입력하는 **앱 프로세스 안에 OS가 직접 로드**한다. 그래서 "데몬이 죽었나"에 해당하는 확인이 없고, 대신 두 가지를 본다.

1. **UNIM이 입력기로 선택돼 있는가** — 작업 표시줄 오른쪽 언어바(입력 표시기)에 UNIM이 보이는지. `Win`+`Space`로 입력기를 순환해 UNIM을 고른다.
2. **진단 로그가 무엇이라고 말하는가** — 로그는 **기본 OFF**다. 켜려면 환경 변수를 설정한다.

```bat
:: 관리자 권한 불필요 — 사용자 환경 변수로 기록된다
setx UNIM_DEBUG_LOG 1
```

- 환경 변수는 **프로세스가 시작될 때 한 번만** 읽힌다. 이미 떠 있는 앱은 이 값을 못 본다 — 진단할 앱을 완전히 종료했다가 다시 연다. 시작 메뉴·탐색기에서 여는 앱까지 확실히 적용하려면 **로그아웃 후 재로그인**한다.
- 로그 파일: `%TEMP%\unim-tsf.log`. 여러 앱이 같은 파일에 이어 쓰며 `[unim-tsf <PID>]` 태그로 구분된다. 탐색기 주소창에 `%TEMP%`를 붙여넣으면 폴더가 열린다.
- 재현 전에 로그를 비우면 읽기 쉽다: `del "%TEMP%\unim-tsf.log"`
- 다른 구성 요소도 각자 `%TEMP%`에 남긴다 — 팝업 렌더러는 `unim-popup-win.log`, 설정 앱은 `unim-settings.log`.

> ⚠️ **`UNIM_DEBUG_CONTENT`는 켜지 마라 (진단 요청을 받은 경우 제외).** 이 변수를 함께 켜면 실제로 누른 키와 조합·확정된 문자열까지 로그에 그대로 남는다. 비밀번호가 평문으로 기록될 수 있다. 진단이 끝나면 `setx UNIM_DEBUG_LOG ""` / `setx UNIM_DEBUG_CONTENT ""`로 끄고 `%TEMP%\unim-tsf.log`를 지운다.

> 진단 로그를 끈 상태(기본값)에서는 로그 관련 비용이 0이다 — 평소에 켜 둘 필요 없다.
<!-- @endplatform -->

---

## 1. "한글이 아예 안 나옴" — 새로 설치한 직후

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Windows에는 `GTK_IM_MODULE` 같은 입력기 환경 변수가 없다. UNIM이 **입력기 목록에 등록됐는지**, 그리고 **선택돼 있는지** 두 단계로 본다.

### 1차 진단

1. `설정` → `시간 및 언어` → `언어 및 지역`에 **한국어**가 있는지 확인한다. 없으면 추가한다 — UNIM은 한국어 프로필에 붙는 입력기라 한국어가 없으면 목록에 나타나지 않는다.
2. 한국어 항목의 `⋯` → `언어 옵션` → **키보드** 목록에 `UNIM Korean IME`가 있는지 확인한다.
3. 메모장을 열고 `Win`+`Space`로 UNIM을 고른 뒤 `dkssudgktpdy`를 쳐 본다. `안녕하세요`가 나오면 정상이다.

### 원인별 처방

| 증상 | 원인 | 해결 |
|------|------|------|
| 언어 목록에 한국어가 없음 | 한국어 프로필 미설치 | `설정` → `시간 및 언어` → `언어 및 지역` → **언어 추가** → 한국어 |
| 키보드 목록에 `UNIM Korean IME`가 없음 | TSF 등록이 안 됐거나 깨짐 | 설치 폴더의 `register-tsf.bat`을 **관리자 권한**으로 실행 (아래 재설치 절 참고) |
| 설치 직후부터 안 보임 | 등록 반영 전 | 로그아웃 후 재로그인 (또는 재부팅) |
| 32비트 앱(카카오톡·한컴 등)에서만 안 보임 | 32비트 COM 등록 누락 | 아래 「32비트 앱에서만 UNIM이 안 보임」 절 |
| 목록엔 있는데 글자가 안 나감 | 다른 입력기가 선택돼 있음 | `Win`+`Space`로 UNIM 선택 |

> 설치 폴더 기본값은 `C:\Program Files\UNIM\`이다. 옮겨 설치했다면 레지스트리 `HKLM\SOFTWARE\atit.org\UNIM` 의 `InstallDir` 값에 실제 경로가 적혀 있다.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 리눅스** — §2~§4는 리눅스 프런트엔드(GTK / Qt / GNOME 확장) 전용이다.

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 2-W. "32비트 앱(카카오톡·한컴 등)에서만 UNIM이 안 보임"

### 원인

Windows는 64비트 앱과 32비트 앱이 **서로 다른 레지스트리 뷰**에서 입력기(COM/TSF)를 찾는다. 64비트 앱은 `unim_tsf.dll`을, 32비트 앱은 `unim_tsf32.dll`을 각각 자기 뷰에서 찾는다. 둘 중 32비트 쪽 등록만 빠지면 **메모장·Edge에서는 잘 되는데 카카오톡에서만 UNIM이 목록에 안 뜨는** 모양이 된다.

MSI는 두 등록을 모두 수행한다. 그래도 안 보이면 등록이 깨진 것이다.

### 처방

설치 폴더(`C:\Program Files\UNIM\`)의 `register-tsf.bat`을 **관리자 권한**으로 실행한다. 이 스크립트는 64비트와 32비트 DLL을 **둘 다** 다시 등록한다.

1. 시작 메뉴에서 `cmd` 검색 → **관리자 권한으로 실행**
2. 아래를 붙여넣는다.

```bat
"C:\Program Files\UNIM\register-tsf.bat"
```

3. 로그아웃 후 재로그인한다.

> `register-tsf.bat`은 **COM/TSF 등록만** 다시 한다. 파일을 다시 깔거나 다른 설치 상태를 되돌리지는 않는다. 그쪽이 필요하면 아래 「재설치·복구·제거」 절의 MSI 복구를 쓴다.

> 되돌리려면 같은 폴더의 `unregister-tsf.bat`을 역시 관리자 권한으로 실행한다.

## 3-W. "특정 앱에서 조합 중인 글자가 끊기거나 앞 글자가 지워짐"

### 증상

일부 앱(터미널 에뮬레이터·일부 채팅 앱)에서 한글을 치면 조합 중인 글자가 중간에 확정돼 버리거나, 직전 글자가 사라진다. 메모장·Edge 같은 표준 텍스트 앱에서는 정상이다.

### 원인

Windows에는 앱이 조합 문자열을 직접 그리지 않을 때 OS가 대신 그려 주는 호환 계층(CUAS)이 있다. 이 계층은 조합을 유지하는 방식이 표준 텍스트 앱과 달라, 조합 중인 글자를 **먼저 확정된 글자로 오해**할 수 있다. UNIM은 이런 창을 감지해 폴백 모드로 넘어가지만, 앱마다 동작이 달라 **모든 조합이 완전히 보전되지는 않는다.**

### 처방

- **아직 일반 해법이 없는 알려진 제한이다.** 근본 대응은 진행 중이며, 이 릴리스에서 모든 앱에 대해 해결됐다고 말할 수 없다.
- 급하면 해당 앱에서만 다른 입력기를 쓰거나, 다른 창(메모장 등)에서 입력해 붙여넣는다.
- 제보해 주면 도움이 된다 — 아래 「진단 데이터 수집」의 로그와 함께 **앱 이름·버전**을 [GitHub Issues](https://github.com/from104/unim/issues)에 적어 주면 해당 앱을 대응 목록에 넣을 수 있다.

> 관찰된 사례: 일부 터미널 에뮬레이터와 채팅 앱. 카카오톡·한글(한컴)에서의 이 증상은 **아직 확인되지 않았다** — 겪는다면 그것도 제보 대상이다.
<!-- @endplatform -->

---

## 5. "한자 팝업이 안 뜸"

<!-- @platform:linux -->
**🐧 리눅스**

### 진단

```bash
# 팝업 렌더러가 떠 있나? (X11/KDE/Xfce, 없으면 §16 참고)
pgrep -a unim-popup
# GNOME Wayland라면 확장이 활성 상태인가?
gnome-extensions list --enabled | grep unim

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
> 전부 `unim-popup-service`(또는 GNOME extension)로 중앙화됐다. 진단은 (이제는 존재하지 않는
> `popup_mode` 설정이 아니라) 위처럼 렌더러 프로세스가 살아 있는지를 본다.

> **DBus가 죽었을 때**: `busctl --user list | grep atit` → 비어 있으면 데몬이 서비스 등록을 못 한 것. `journalctl --user -u unim-daemon -n 100` 로그 확인.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### 진단

Windows에서도 팝업은 입력기가 직접 그리지 않는다. **별도 프로세스 `unim-popup-win.exe`**가 그리고, `unim_tsf.dll`이 명명 파이프로 그리라고 시킨다. 즉 이 실행 파일을 못 찾으면 팝업이 안 뜬다.

1. **먼저 한자 키가 도달하는지 확인** — 메모장에서 `한자`를 친 뒤 `한자` 키(또는 오른쪽 `Ctrl`)를 누른다. 팝업이 뜨는지 본다.
2. **렌더러 프로세스 확인** — 팝업이 떠야 할 때 `Ctrl`+`Shift`+`Esc`로 작업 관리자를 열고 **세부 정보** 탭에서 `unim-popup-win.exe`를 찾는다.
3. **실행 파일이 제자리에 있는지 확인** — 설치 폴더(`C:\Program Files\UNIM\`)에 `unim-popup-win.exe`가 있어야 한다.

### 처방

| 증상 | 원인 | 해결 |
|------|------|------|
| 설치 폴더에 `unim-popup-win.exe`가 없음 | 설치가 부분적으로 깨짐 | 아래 「재설치·복구·제거」의 MSI 복구 |
| 파일은 있는데 프로세스가 안 뜸 | 실행 파일 탐색 실패 | 레지스트리 `HKLM\SOFTWARE\atit.org\UNIM` 의 `InstallDir`·`UnimPopupRenderer` 값이 실제 경로를 가리키는지 확인 |
| 팝업 자체가 안 뜸 (한자 키 무반응) | 한자 키가 다른 곳에서 가로채짐 | 오른쪽 `Ctrl`로 시도. 그래도 안 되면 설정 앱에서 한자 키 지정 확인 |

렌더러는 자기 로그를 따로 남긴다. 문서 맨 위 절차대로 `UNIM_DEBUG_LOG`를 켠 뒤 `%TEMP%\unim-popup-win.log`를 본다.

> 레지스트리 값은 `Win`+`R` → `regedit`으로 확인할 수 있다. **값을 고치지 말고 읽기만 한다** — 경로가 틀렸다면 MSI 복구가 올바른 해결이다.
<!-- @endplatform -->

---

## 6. "특수문자 팝업이 안 뜸"

원인 거의 동일 (한자 팝업과 같은 코드 경로).

<!-- @platform:linux -->
**🐧 리눅스** — 현재 입력 모드(한글/영문)는 CLI 로 조회하는 키가 따로 없다 — 트레이 아이콘 또는 GNOME 확장 인디케이터 표시로 확인한다.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — 현재 입력 모드(한글/영문)는 작업 표시줄 오른쪽 **언어바(입력 표시기)의 UNIM 버튼**이 보여 준다. 버튼을 클릭하면 한/영이 전환된다. 상태가 실제 입력과 어긋나 보이면 `한/영` 키를 한 번 눌러 다시 맞춘다.
<!-- @endplatform -->

한글 모드에서 자음(초성) 1글자만 입력한 상태로 한자 키를 눌러야 특수문자 팝업이 뜬다.

ㄱ~ㅎ 입력 후 한자 키. 한국어 자판이 두벌식이면 잘 동작, 세벌식이면 자음 입력 자체가 다를 수 있음.

---

## 7. "한자 팝업이 9칸까지만 보이고 81칸 토글이 안 됨"

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### 진단

한자 팝업이 떠 있는 상태에서 마침표(`.`) 키가 팝업까지 도달해야 9칸 ↔ 81칸이 전환된다.

1. 한자 팝업을 띄운다.
2. `.`을 누른다. 격자가 9칸에서 81칸으로 바뀌어야 한다.
3. 바뀌지 않으면 문서 맨 위 절차대로 `UNIM_DEBUG_LOG`를 켜고 재현한 뒤 `%TEMP%\unim-tsf.log`에서 마침표 처리 관련 줄을 찾는다.

### 처방

- 키보드 레이아웃에서 `.`이 다른 키로 매핑돼 있는지 확인한다 (`설정` → `시간 및 언어` → `언어 및 지역` → 한국어 → 키보드).
- 화면 키보드(`Win`+`Ctrl`+`O`)의 `.`로도 눌러 본다. 화면 키보드에서는 되는데 물리 키보드에서 안 되면 키보드/레이아웃 문제다.
- 팝업 페이지 이동은 `Page Down` / `Page Up`, 즐겨찾기(★) 토글은 `Space`, 취소는 `Esc`다. 이 키들은 되는데 `.`만 안 되면 마침표 키만 가로채진 것이다.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 리눅스** — §7-1·§7-2는 리눅스 전용(Wayland 컴포지터 / XIM) 증상이다.

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
<!-- @endplatform -->

---

## 8. "AutoTypeFix가 작동 안 함"

### 진단

<!-- @platform:linux -->
**🐧 리눅스**

```bash
unim-cli config show | grep -i typefix         # 전체/순방향/역방향 사용 여부 확인
cat ~/.config/unim/typefix-blacklist.yaml | head -50  # 등록된 억제 단어
```
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Windows에는 `unim-cli`가 없다. 설정 앱으로 확인한다.

1. 시작 메뉴 → **UNIM** → **UNIM Settings** (또는 언어바 UNIM 버튼 → 메뉴 → `설정`)
2. **오타 교정** 페이지에서 마스터 토글과 순방향/역방향 사용 여부를 본다.
3. **억제 단어** 페이지에서 등록된 단어 목록을 본다.

설정 파일을 직접 보고 싶으면 탐색기 주소창에 아래를 붙여넣는다.

```
%APPDATA%\unim
```

`config.yaml`(전체 설정)과 `typefix-blacklist.yaml`(억제 단어)이 여기에 있다.
<!-- @endplatform -->

### 처방

- 마스터 토글 OFF → 설정 GUI에서 ON.
- 특정 단어가 자꾸 억제 사전에 들어가면 GUI 「교정 억제 단어」 페이지 → 「비활성」 또는 「삭제」.
- 단어 경계(스페이스/마침표)가 들어와야 비로소 교정이 적용된다. 한 글자 더 친 뒤에 스페이스를 넣어 보면 즉시 교정됨.
- 영문 모드에서 작동 안 함 → 「영문 모드 시 무시」가 ON이라 그렇다 (의도된 기본값).

---

## 8-1. "비밀번호 칸인데 자동 오타 교정이 발동한다"

### 원인

비밀번호 보호([FAQ](../faq/README-ko.md) Q9)는 앱이 "이 칸은 비밀번호"라고 알려 줄 때(`content_purpose`)만 동작한다. 아래 환경은 이 신호를 UNIM 에 전달하지 못해, 비밀번호 칸이 일반 칸으로 취급된다.

| 환경 | 상태 | 이유 |
|------|------|------|
| GTK3/4, Qt5/6, GNOME 확장, Windows TSF(64비트·32비트 `unim_tsf32.dll` 모두) | 감지됨 | content_purpose / InputScope 정상 전달 |
| XIM 레거시 앱 | 미감지 | XIM 프로토콜에 해당 신호가 없음 |
| 일부 Wayland 컴포지터·웹폼 | 미감지 | content-purpose 를 보내지 않음(앱/컴포지터 재량) |
| 포커스 후 목적이 바뀌는 GTK 앱 | 미감지 | GTK IM 은 포커스 시점에만 input-purpose 를 읽고 `notify::input-purpose` 변경은 구독하지 않는다(기존 한계) — 같은 칸이 나중에 비밀번호로 바뀌면 재포커스 전까지 반영 안 됨 |

> **이 표는 아직 성긴 편이다 — 제보 바란다.** 어떤 앱이 비밀번호 칸을 제대로 알려 주고 어떤 앱이 안 알려 주는지는 실사용 사례가 쌓여야 채워진다. 리눅스·Windows 양쪽 다 사례가 충분치 않아 앱별 대응이 다 들어가 있지 못하다. 비밀번호 칸에서 교정이 발동하는 앱을 만나면 **앱 이름·버전**을 [GitHub Issues](https://github.com/from104/unim/issues) 로 알려 주기 바란다. 그게 이 표를 채우는 유일한 경로다.

### 처방

- 미감지 환경에서는 비밀번호를 치기 전에 한/영 키로 **영문 모드**를 직접 확인한다. 영문 모드에서는 순방향(영→한) 교정이 기본으로 억제되므로 사실상 안전하다.
- 특정 앱에서 자주 겪는다면 자동 오타 교정 자체를 토글 단축키([사용자 매뉴얼](../user-guide/README-ko.md) 4.4)로 잠깐 꺼 두는 것도 방법이다.
<!-- @platform:linux -->
- 토글 단축키를 지정할 때는 **한/영 키·한자 키와 같은 키를 쓰지 않는다** — 역할이 충돌해 한/영 전환이나 한자 변환이 토글에 가려질 수 있다(CLI `unim-cli config set` 은 중복 지정 시 경고를 낸다).
<!-- @endplatform -->
<!-- @platform:windows -->
- 토글 단축키를 지정할 때는 **한/영 키·한자 키와 같은 키를 쓰지 않는다** — 역할이 충돌해 한/영 전환이나 한자 변환이 토글에 가려질 수 있다.
<!-- @endplatform -->

> **preedit 노출 관련 별도 이슈**: 비밀번호 칸에서는 한글 조합 자체가 차단되므로, 조합 중 글자가 preedit(밑줄)로 화면에 잠깐 비치는 문제는 정상 감지 환경에선 거의 없다. 다만 위 미감지 Wayland 환경에서는 이론상 노출이 성립할 수 있어 **별도 이슈로 추적 중**이며, 현재 권장 우회는 위 「영문 모드 직접 확인」이다.

---

## 9. "AutoTypeFix가 너무 자주 잘못 교정"

### 처방

<!-- @platform:linux -->
**🐧 리눅스**

| 케이스 | 해법 |
|--------|-----|
| 특정 단어 한 개만 문제 | BS + 한/영로 롤백 → 다음에 같은 단어 칠 때 자동으로 Tentative 등록됨 |
| 자주 후회한다 | 설정 → 「오타 교정」 → 임시 만료 시간을 늘려 학습 기회 확보 |
| 영문 모드에서도 reverse가 동작 | `auto_typefix.reverse.skip_incomplete_syllable` ON |
| 직접 단어 추가 | `~/.config/unim/typefix-blacklist.yaml`을 텍스트 에디터로 편집 → 데몬이 mtime 자동 리로드 |
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

| 케이스 | 해법 |
|--------|-----|
| 특정 단어 한 개만 문제 | BS + 한/영로 롤백 → 다음에 같은 단어 칠 때 자동으로 Tentative 등록됨 |
| 자주 후회한다 | 설정 앱 → 「오타 교정」 → 임시 만료 시간을 늘려 학습 기회 확보 |
| 영문 모드에서도 reverse가 동작 | 설정 앱 「오타 교정」 페이지에서 역방향 관련 항목을 확인 |
| 직접 단어 추가 | `%APPDATA%\unim\typefix-blacklist.yaml`을 메모장으로 편집 |

> 설정 파일을 손으로 고쳤다면 **입력할 앱을 한 번 벗어났다 돌아온다**(다른 창 클릭 후 복귀). Windows에는 설정을 밀어 주는 데몬이 없어서, UNIM은 파일이 바뀌었는지를 **포커스가 돌아올 때** 확인해 다시 읽는다. 그래서 반영이 즉시가 아닐 수 있다.
<!-- @endplatform -->

---

## 10. "설정이 저장 안 됨 / 변경이 안 먹힘"

<!-- @platform:linux -->
**🐧 리눅스**

### 진단

```bash
ls -la ~/.config/unim/
test -w ~/.config/unim/config.yaml && echo writable || echo BLOCKED
unim-cli config show 2>&1 | head -5
journalctl --user -u unim-daemon -n 50
```

### 처방

- 권한 문제 → `chmod 644 ~/.config/unim/*.yaml`, `chmod 755 ~/.config/unim`.
- `~/.config/unim`을 root가 만든 흔적이 있으면 `sudo chown -R $USER:$USER ~/.config/unim`.
- GUI에서 변경했는데 데몬에 반영 안 됨 → 데몬 재시작: `systemctl --user restart unim-daemon`.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### 진단

설정은 `%APPDATA%\unim\config.yaml` 한 파일에 저장된다. 탐색기 주소창에 `%APPDATA%\unim`을 붙여넣어 연다.

1. `config.yaml`이 있는지, **수정한 날짜**가 방금 저장한 시각인지 본다.
2. 날짜가 안 바뀌었으면 저장 자체가 실패한 것 — 설정 앱을 닫고 다시 열어 저장해 본다.
3. 날짜는 바뀌었는데 입력에 반영이 안 되면 아래 처방으로 간다.

### 처방

- **가장 흔한 원인 — 반영 시점.** Windows에는 설정을 밀어 주는 데몬이 없다. UNIM은 `config.yaml`이 바뀌었는지를 **입력 포커스가 그 앱으로 돌아올 때** 확인해 다시 읽는다. 설정을 바꾼 뒤 **입력할 앱을 한 번 벗어났다 돌아오면** 적용된다(다른 창 클릭 → 원래 창 클릭).
- 그래도 안 되면 해당 앱을 완전히 종료했다가 다시 연다.
- 여러 앱에 걸쳐 계속 안 맞으면 로그아웃 후 재로그인한다.
- `%APPDATA%\unim` 폴더나 `config.yaml`이 **읽기 전용**이면 저장이 막힌다. 파일 우클릭 → `속성` → **읽기 전용** 체크 해제.
- 회사 PC 등에서 `%APPDATA%`가 정책으로 잠겨 있으면 저장이 안 될 수 있다 — 이 경우는 관리자 문의가 맞다.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 리눅스** — §11~§14는 리눅스 전용이다. Windows에는 GTK IM 모듈·Flatpak·Snap·상주 데몬이 모두 없다.

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

> ⚠️ **UNIM 제거 후에도 남는다**: 이 override는 사용자 전역 `~/.local/share/flatpak/overrides/global`에 영구히 기록되며, `unim` 패키지를 삭제해도 자동으로 원복되지 않는다. 다른 입력기로 옮긴 뒤 Flatpak 앱 입력이 이상하면 아래 명령으로 직접 해제해야 한다.
> ```bash
> flatpak override --user --unset-env=QT_IM_MODULE --unset-env=GTK_IM_MODULE
> ```

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 14-W. 재설치 · 복구 · 제거

Windows용 UNIM은 **MSI 설치 관리자 하나**로 배포된다. 파일이 깨졌거나 등록이 어긋났을 때 손으로 지우지 말고 아래 순서를 쓴다.

### 복구 (파일·등록을 원래대로)

받아 둔 MSI 파일을 다시 실행하면 설치 관리자가 **복구(Repair)** 옵션을 제공한다. MSI 파일이 없으면 [GitHub Releases](https://github.com/from104/unim/releases)에서 같은 버전을 다시 받는다.

### 재설치 (버전 올리기 포함)

새 MSI를 그냥 실행하면 이전 버전을 덮어쓴다. 먼저 지울 필요 없다.

### 제거

`설정` → `앱` → `설치된 앱` → **UNIM Korean IME** → `제거`. 시작 메뉴 **UNIM** 폴더의 **Uninstall UNIM** 바로 가기도 같은 일을 한다.

### 등록만 다시 하기

파일은 멀쩡한데 입력기 목록에서만 사라진 경우다. 설치 폴더의 `register-tsf.bat`을 **관리자 권한**으로 실행한다(§2-W 참고). 되돌리려면 `unregister-tsf.bat`.

### 설치 폴더에 들어 있는 것

기본 경로는 `C:\Program Files\UNIM\`이다.

| 파일 | 역할 |
|------|------|
| `unim_tsf.dll` | 64비트 앱용 입력기 본체 |
| `unim_tsf32.dll` | 32비트 앱(카카오톡·한컴 등)용 입력기 본체 |
| `unim-settings.exe` | 설정 앱 |
| `unim-popup-win.exe` | 한자·특수문자·이모지 팝업 렌더러 |
| `register-tsf.bat` / `unregister-tsf.bat` | 입력기 등록 / 등록 해제 (관리자 권한) |
| `help\unim-help-ko.html`, `help\unim-help-en.html` | 지금 보고 있는 오프라인 도움말 |
| `LICENSE.txt`, `NOTICE.txt`, `LICENSES\` | 라이선스 고지 |

> **설정 파일은 여기 없다.** 설정은 `%APPDATA%\unim\`에 사용자별로 저장되며, UNIM을 제거해도 지워지지 않는다. 완전히 지우고 싶으면 제거 후 이 폴더를 직접 삭제한다.
<!-- @endplatform -->

---

## 15. "모아치기(chord)가 제대로 인식 안 됨"

모아치기 관련 문제는 원인이 다양하다. 아래 항목을 순서대로 확인한다.

### 15-1. 현재 자판이 모아치기를 지원하지 않음

모아치기는 `supports_moachigi: true`인 자판에서만 동작한다. 빌트인 중에는 **안마태 자판 (ko_3bul_anmatae)** 만 해당된다. **쿼티형 세벌식 v2** 는 빌트인이 아닌 연구 자료(`docs/references/keymaps/ko_3bul_qwerty_v2.json`)로 보존되며, 사용자 자판 폴더로 복사한 사용자 프로필에서 모아치기 지원을 켤 수 있다.

<!-- @platform:linux -->
사용자 자판 폴더는 `~/.config/unim/layouts/` 다 — 위 파일을 `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사한다.
<!-- @endplatform -->

<!-- @platform:windows -->
사용자 자판 폴더는 `%APPDATA%\unim\layouts\` 다 — 위 파일을 `%APPDATA%\unim\layouts\ko_3bul_qwerty.json`으로 복사한다. 폴더가 없으면 직접 만든다.
<!-- @endplatform -->

<!-- @platform:linux -->
```bash
# 현재 자판 확인
unim-cli config show | grep -E 'layout|keymap'
```
<!-- @endplatform -->

<!-- @platform:windows -->
현재 자판은 설정 앱(`unim-settings.exe`) 「일반」 페이지의 자판 목록에서 확인한다.
<!-- @endplatform -->

출력이 모아치기 지원 자판이 아니라면 설정 앱(`unim-settings`)에서 자판을 변경해야 한다. 모아치기 지원 자판으로 바꾸면 **모아치기** 그룹이 자동으로 나타난다.

### 15-2. chord_window_ms가 너무 짧음

`chord_window_ms` 기본 권장값은 **60ms**다. 처음 모아치기를 시작하거나 입력 속도가 빠르지 않다면 **80~100ms** 부터 시작해 익숙해지면 줄여 나가는 것이 좋다.

<!-- @platform:linux -->
```bash
# 현재 설정값 확인
unim-cli config show | grep chord-window

# 80ms로 변경
unim-cli config set korean-chord-window-ms 80
```

또는 설정 앱(`unim-settings`) 「일반」 페이지의 자판 옵션(모아치기 지원 자판에서만 표시)에서 슬라이더로 조정한다.
<!-- @endplatform -->

<!-- @platform:windows -->
설정 앱(`unim-settings.exe`) 「일반」 페이지의 자판 옵션(모아치기 지원 자판에서만 표시)에서 슬라이더로 조정한다.
<!-- @endplatform -->

### 15-3. bidirectional_combine이 비활성 상태

자모 역순 결합(예: ᆯ+ᆨ → ᆰ, ㅎ+ㄱ → ㅋ)이 안 된다면 **양방향 자모 결합** 옵션이 꺼져 있는 것이다.

<!-- @platform:linux -->
```bash
# 현재 상태 확인
unim-cli config show | grep bidirectional-combine

# 활성화
unim-cli config set korean-bidirectional-combine true
```

또는 설정 앱(`unim-settings`) 「일반」 페이지 > 자판 옵션 > **양방향 자모 결합** 토글을 ON.
<!-- @endplatform -->

<!-- @platform:windows -->
설정 앱(`unim-settings.exe`) 「일반」 페이지 > 자판 옵션 > **양방향 자모 결합** 토글을 ON.
<!-- @endplatform -->

### 15-4. 키보드가 NKRO를 지원하지 않음 (ghosting)

일반 멤브레인 키보드는 2~3 KRO(Key Rollover) 한계로 인해 여러 키를 동시에 눌렀을 때 일부 키가 운영체제에 전달되지 않는다(ghosting). chord가 누락되거나 엉뚱한 자모로 조합된다면 키보드 자체가 동시 입력을 지원하지 못하는 것일 수 있다.

자가 진단:

<!-- @platform:linux -->
```sh
# X11 환경에서 동시 키 이벤트 확인
xev -event keyboard
```

Wayland 환경에서는 `xev` 대신 `wev` 사용 (`apt install wev` 또는 동등).

나타나는 창에 포커스를 두고 chord에서 사용하는 키를 모두 동시에 누른다. 터미널에 `KeyPress event`가 키 수만큼 모두 찍혀야 한다. 또는 브라우저에서 [keyboardchecker.com](https://keyboardchecker.com) 등 온라인 키 테스터를 사용한다.
<!-- @endplatform -->

<!-- @platform:windows -->
브라우저에서 [keyboardchecker.com](https://keyboardchecker.com) 등 온라인 키 테스터를 연 뒤, chord에서 사용하는 키를 모두 동시에 누른다. 누른 키가 **전부** 눌린 것으로 표시돼야 한다. 하나라도 빠지면 키보드 자체의 동시 입력 한계(ghosting)다 — UNIM 설정으로는 해결되지 않는다.
<!-- @endplatform -->

해결: 게이밍 키보드 또는 메커니컬 키보드의 NKRO 모드 사용 권장. 자세한 내용은 [안마태 자판 가이드 — 키보드 호환성](../keymaps/anmatae.md#키보드-호환성-nkro-권장) 참고.

### 15-5. USB 폴링 레이트 낮음 (125Hz = 8ms 분해능)

USB 키보드의 기본 폴링 레이트는 125Hz(= 8ms 간격)다. `chord_window_ms`를 10~30ms처럼 짧게 설정하면 폴링 주기 자체가 윈도우의 상당 부분을 차지해 일부 키를 놓칠 수 있다.

해결:

- `chord_window_ms`를 **60ms 이상**으로 올려 폴링 지연 여유분 확보.
- 1000Hz 폴링 지원 게이밍 키보드 사용, 또는 USB 포트 변경.

---

## 빌드 실패

<!-- @platform:windows -->
**🪟 Windows** — Windows용 UNIM은 **MSI 설치 관리자로만** 배포된다. 일반 사용자가 소스를 빌드할 일은 없다. 설치 자체가 실패했다면 위 「재설치 · 복구 · 제거」 절을 본다. 소스에서 빌드하려는 개발자는 저장소의 `docs/dev/windows/` 문서를 참고한다.
<!-- @endplatform -->

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

---

## 진단 데이터 수집 (이슈 보고 시 필수)

<!-- @platform:linux -->
**🐧 리눅스**

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
  unim-cli config show
  echo "=== logs (last 200) ==="
  tail -n 200 ~/.unim-errors.log 2>/dev/null
} > unim-report.txt
```

이 `unim-report.txt`를 이슈에 첨부하면 진단이 빨라진다. 비밀번호·토큰이 로그에 들어갔을 가능성은 낮지만, 첨부 전에 한 번 훑어보길 권장.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

이슈를 올릴 때 아래 다섯 가지를 함께 적어 주면 진단이 훨씬 빨라진다.

1. **Windows 버전** — `Win`+`R` → `winver` 입력. 나오는 창의 버전·빌드 번호.
2. **UNIM 버전** — 설정 앱(`unim-settings.exe`) 또는 언어바 UNIM 버튼 메뉴의 `정보`.
3. **증상이 나는 앱의 이름과 버전** — 앱마다 동작이 다르므로 이게 가장 중요하다. 32비트 앱인지 64비트 앱인지도 알면 좋다(작업 관리자 → 세부 정보 → 프로세스 우클릭 → `속성`).
4. **설정 파일** — `%APPDATA%\unim\config.yaml`.
5. **진단 로그** — 문서 맨 위 절차대로 `UNIM_DEBUG_LOG`를 켜고 증상을 재현한 뒤 `%TEMP%\unim-tsf.log`. 팝업 문제면 `%TEMP%\unim-popup-win.log`도 함께.

아래를 명령 프롬프트에 붙여넣으면 바탕 화면에 로그가 모인다.

```bat
copy "%TEMP%\unim-tsf.log" "%USERPROFILE%\Desktop\unim-report-tsf.log"
copy "%TEMP%\unim-popup-win.log" "%USERPROFILE%\Desktop\unim-report-popup.log"
copy "%APPDATA%\unim\config.yaml" "%USERPROFILE%\Desktop\unim-report-config.yaml"
```

> ⚠️ **첨부 전에 반드시 열어서 훑어본다.** `UNIM_DEBUG_CONTENT`를 함께 켰다면 실제로 입력한 문자열이 로그에 남아 있다. 비밀번호·개인 정보가 보이면 지우고 올린다.
<!-- @endplatform -->

---

## 더 읽을 거리

- [FAQ](../faq/README-ko.md) — 다른 IME와의 차이, 동시 설치, 백업 복원
- [사용자 매뉴얼](../user-guide/README-ko.md) — 설정 GUI 페이지별 상세
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — 동작 명세 (개발자용)
- [`AGENTS.md`](../../dev/architecture/AGENTS.md) — 아키텍처와 메모리 관리 규칙

---

<!-- @platform:linux -->
**🐧 리눅스** — 이하 §16과 0.2.0 릴리스 특이 진단은 리눅스 전용이다.

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

- 서비스 파일이 없으면 `unim-popup-service` 실행 파일을 담고 있는 **`unim-desktop`** 패키지(인디케이터·레거시 설정창과 한 묶음)가 설치되지 않은 것이다. `unim-popup-service` 는 독립 패키지가 아니다.

  ```bash
  # deb 설치 (버전은 실제 다운로드한 파일명에 맞게)
  sudo apt install ./unim-desktop_<버전>_amd64.deb
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

### 증상: XIM commit 직후 다음 글자가 안 보인다 (일부 해결, 일부 미해결)

조합 중이던 음절이 확정된 직후 새로 누른 자모가 화면에 안 나타나는 증상이다. 0.3.0부터 있었고, **경로에 따라 상태가 다르다.**

**해결된 범위 (2026-08-07)** — 자체 XIM 클라이언트와 OVER-THE-SPOT(XTerm·WezTerm 등)에서는 고쳐졌다. 원인은 그동안 이 문서가 지목하던 "xim crate의 `commit()`이 `preedit_started`를 갱신하지 않음"이 **아니었다**(그 우회책을 통째로 제거해도 증상이 그대로였다). 실제로는 **ON-THE-SPOT 클라이언트가 한 키를 처리하다 `Commit`을 만나면 그 뒤에 온 메시지를 버리기 때문**이었고, XIM만 preedit을 commit보다 **먼저** 보내도록 바꿔 해결했다. 계약은 `docs/dev/architecture/IME_BEHAVIOR.md` §8.1 예외 항목.

**아직 미해결 (2026-08-10 확인)** — **GTK가 XIM 모듈(`im-xim`)로 붙는 경로**에서는 여전히 증상이 남아 있고, 원인도 위와 다르다. 이 경우 확정 직후 입력기가 잠시 멎어 **다음 글자가 몇 초 동안 통째로 씹힌다**(libX11이 시간이 지나면 스스로 풀린다). 서버가 `PreeditDraw`를 보내는 순간 그 입력 문맥이 다음 키를 받지 못하는 것이 원인으로, 3/3 재현된다.

- **누가 겪나**: 주로 **Flatpak·Snap 앱**이다. 샌드박스 안에는 호스트의 `im-unim.so`가 보이지 않아 GTK가 XIM으로 폴백한다. 옵시디언(Electron)이 대표적이다.
- **영향 없는 환경**: 일반 설치된 GTK/Qt 앱(네이티브 IM 모듈 사용), XTerm·WezTerm 등 OVER-THE-SPOT 클라이언트.
- **현재 우회책**: 없다. 같은 앱을 deb·AppImage 등 **샌드박스 밖 배포판으로 설치**하면 네이티브 IM 모듈을 타서 증상이 사라진다. IBus 호환 경로는 0.4.0에서 크게 고쳤지만 아직 조합 중 글자가 표시되지 않아 대안이 되지 못한다.

진행 상황은 ROADMAP 3단계 "샌드박스 앱(Flatpak·Snap) 입력 경로" 항목에서 추적한다.

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
- **XIM commit 직후 다음 글자 누락 (부분 해결)**: 자체 XIM 클라이언트·OVER-THE-SPOT 은 2026-08-07 에 해결했다 — ON-THE-SPOT 클라이언트가 한 키를 처리하다 `Commit` 을 만나면 그 뒤 메시지를 버리는 것이 원인이었고(오래 적혀 있던 "xim crate 의 `commit()` 이 `preedit_started` 를 갱신하지 않음" 은 오진), XIM 만 preedit 을 commit 보다 먼저 보내도록 바꿨다(`IME_BEHAVIOR.md` §8.1). **그러나 GTK 가 `im-xim` 으로 붙는 경로는 2026-08-10 확인 결과 여전히 미해결이며 원인도 다르다** — `PreeditDraw` 를 보내는 순간 그 IC 가 다음 키를 받지 못한다(3/3 재현). 주 피해자는 Flatpak·Snap 앱이다. 회귀 감시: `tests/unim-test-xim`·`tests/unim-test-gtk3`(ON-THE-SPOT)와 `xterm`(OVER-THE-SPOT), 그리고 `GTK_IM_MODULE=xim` 으로 띄운 `tests/unim-test-gtk3`.

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

### G. 환경 매트릭스 (0.4.0 재확인 — 최초 작성은 0.3.0)

| 환경 | 지원 상태 | 비고 |
|------|----------|------|
| GNOME Wayland | ✅ 검증 | GNOME extension `popup_view.js` (St 위젯) 자체 렌더 |
| GNOME X11 | ✅ 검증 | popup-service GTK4 + GNOME extension 보조 |
| X11 + KDE Plasma 5.x | ✅ 검증 | popup-service GTK4 |
| X11 + XFCE / MATE / Cinnamon / LXDE | ✅ 검증 | popup-service GTK4 |
| Wayland + KDE Plasma 5.x | ❌ 미지원 | `gtk4-layer-shell` 미배포 (Ubuntu 24.04 noble 표준 저장소) → X11 세션 / GNOME 우회 |
| Wayland + KDE Plasma 6 | ⚠️ 실험적 | `wayland-backend` feature + `libgtk4-layer-shell` 필요. 0.4.0 QA 미검증(0.3.0 이후 변경 없음) |
| Sway / Hyprland / river (단독 Wayland) | ⚠️ 실험적 | 동상. popup 위치·IME 포커스 회귀 가능 |
| Weston 등 reference Wayland | ⚠️ 실험적 | 동상 |

> **0.4.0 재확인 결과**: 위 표는 0.4.0 릴리스 시점에도 여전히 유효하다 — **순수 Wayland(비-GNOME) 환경의 한자/특수문자 팝업은 이 릴리스에서도 미지원**이며(설계 제약, 유지), GNOME 을 거치지 않는 Wayland 컴포지터는 여전히 「실험적」 지위다. Windows(TSF, 64비트·32비트 `unim_tsf32.dll`)는 v0.4.0에서 새로 추가된 **실험적** 플랫폼이며 이 표에는 포함하지 않는다 — 별도로 [FAQ Q11](../faq/README-ko.md#q11-unim은-macoswindows에서도-되나) 참고.

⚠️ 실험적 환경에서 문제 발견 시 [GitHub Issues](https://github.com/from104/unim/issues) 로 제보 부탁드립니다.

### H. 로그 분석 슬래시 명령

```bash
# Claude Code 사용 시
/unim-log
```
→ `~/.unim-errors.log` 자동 분류·요약·진단.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 16-W. Windows 알려진 제한 (v0.4.0)

Windows 지원은 v0.4.0에서 새로 추가됐다. 아래는 릴리스 시점에 알려진 제한이며, **여기에 없는 증상은 아직 확인되지 않은 것**이다 — 겪는다면 제보 대상이다.

| 항목 | 상태 | 설명 |
|------|------|------|
| 일부 앱에서 조합 끊김·직전 글자 삭제 | ⚠️ 알려진 제한 | 앱이 조합 문자열을 직접 그리지 않을 때 쓰이는 Windows 호환 계층(CUAS)의 동작 차이. UNIM이 폴백으로 대응하지만 모든 앱에서 완전하지는 않다. §3-W 참고 |
| 콘솔·터미널 계열 앱 | ⚠️ 앱마다 다름 | 앱이 어떤 입력 방식을 쓰느냐에 따라 동작이 갈린다. 표준 텍스트 앱(메모장·Edge)이 기준 동작이다 |
| 카카오톡·한글(한컴) 등 32비트 앱 | ✅ 지원 (제한적 검증) | 32비트 입력기(`unim_tsf32.dll`)를 함께 설치해 대응한다. 안 보이면 §2-W |
| 32비트 앱에서의 개별 증상 | ❓ 미확인 | 한글 입력 자체는 확인됐으나, 앱별 세부 증상은 검증 범위 밖이다 |
| 트레이 아이콘 / 상주 인디케이터 | ❌ 없음 | Windows에서는 **언어바(입력 표시기)의 UNIM 버튼**이 그 역할을 한다 |
| `unim-cli` 명령줄 도구 | ❌ 없음 | 설정은 설정 앱 또는 `%APPDATA%\unim\config.yaml` 직접 편집으로 한다 |
| 설정 변경의 즉시 반영 | ⚠️ 포커스 기준 | 데몬이 없어 설정을 밀어 주지 못한다. 입력할 앱을 한 번 벗어났다 돌아와야 반영된다. §10 참고 |
| 리눅스 문서의 진단 명령 | ❌ 해당 없음 | `systemctl` · `journalctl` · `busctl` · `gsettings` · `im-config` 같은 명령과 `GTK_IM_MODULE` · `QT_IM_MODULE` · `XMODIFIERS` 환경 변수는 Windows에 존재하지 않는다 |

### 제보할 때

[GitHub Issues](https://github.com/from104/unim/issues)에 **앱 이름·버전**과 위 「진단 데이터 수집」의 로그를 함께 올려 주면 대응 목록에 넣을 수 있다. Windows 쪽은 앱마다 궁합이 갈려서, 실제 사용 보고가 가장 큰 도움이 된다.
<!-- @endplatform -->
