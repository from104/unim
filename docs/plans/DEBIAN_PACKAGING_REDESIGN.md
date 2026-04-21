# UNIM Debian Packaging — 9-Package Layout

> Status: **Implemented (0.1.0-1).** Owner: `from104`. Last updated: 2026-04-21.
>
> 본 문서는 `debian/` 디렉토리의 최종 패키지 분할 정책과 각 패키지의 책임 범위를 정의한다.
> 과거 "2 패키지 통합" 방안(r2, 2026-04-19)은 GTK/Qt IM 모듈 공존 불가 문제와 사용자 선택권 부재로 폐기되고, 본 9패키지 설계로 교체되었다.

---

## 1. 배경 · 목표

### 1.1 분할 원칙

패키지는 **사용자가 "선택"하는 축**을 따라 쪼갠다:

| 축 | 분리 여부 | 근거 |
|---|---|---|
| 엔진 (core / daemon / dbus / cli) | 공통 | 모든 설치에 필수 |
| IM 모듈 (GTK / Qt) | 툴킷별 분리 | 실사용자는 둘 다 원하지만, 최소 설치 케이스도 허용 |
| 세션 프로토콜 (XIM / Wayland) | 프로토콜별 분리 | X11/Wayland 세션 선택에 따라 다름 |
| 설정 GUI (GTK / Qt) | 툴킷별 분리 | 의존성 무게 다름, 사용자 취향 |
| GNOME Shell extension | 독립 | GNOME 한정, gnome-shell Depends |

**공존 허용**: 모든 패키지는 서로 `Conflicts` 없이 공존 가능. 메타패키지 `unim`은 전부를 끌어옴.

### 1.2 목표

1. `lintian -E -W` 출력 **0건** (정당화된 `*.lintian-overrides`만 허용).
2. `dpkg-buildpackage -b -us -uc` warning **0건**.
3. 9개 패키지 정상 설치 / 업그레이드 / 제거 (`piuparts` 통과).
4. `apt install unim` 한 줄로 full stack 설치 체감 유지.
5. 소스 트리 재현성: `make deb` 후 워크트리에 leftover 없음 (`debs/` 외).
6. Launchpad PPA / Debian mentors 업로드 가능 메타데이터.

### 1.3 비목표

- multiarch co-installable layout.
- snap / flatpak 패키징.
- 자동 systemd unit 활성화 (D-Bus activation만 사용).
- GNOME Shell extension의 EGO upload 자동화.

---

## 2. 패키지 매트릭스

| 패키지 | Arch | 파일 | Depends (핵심) |
|---|---|---|---|
| `unim-common` | any | libunim_capi, unim-daemon, unim-cli, dbus service, im-config 데이터, 아이콘, man(unim/cli) | `${shlibs}`, `dbus` |
| `unim-im-gtk` | any | gtk3 + gtk4 immodule (.so × 2) | `unim-common (= ${binary:Version})` |
| `unim-im-qt` | any | qt5 + qt6 platforminputcontexts (.so × 2) | `unim-common (= ${binary:Version})` |
| `unim-xim` | any | `usr/libexec/unim-xim` | `unim-common (= ${binary:Version})` |
| `unim-wayland` | any | `usr/libexec/unim-wayland` | `unim-common (= ${binary:Version})` |
| `unim-gui-gtk` | any | `unim-gui-gtk` + autostart + man | `unim-common (= ${binary:Version})` |
| `unim-gui-qt` | any | `unim-gui-qt` + man | `unim-common (= ${binary:Version})` |
| `unim-gnome` | all | `usr/share/gnome-shell/extensions/unim-gnome@from104.github.io/` | `unim-common`, `unim-gui-gtk`, `gnome-shell` |
| `unim` | all | — (메타) | 위 8개 전부 (`= ${source:Version}`) |

### Recommends 그래프

- `unim-common` → `im-config`
- `unim-im-gtk` → `unim-xim | unim-wayland`
- `unim-im-qt` → `unim-xim | unim-wayland`
- `unim-gui-gtk` → `unim-im-gtk`
- `unim-gui-qt` → `unim-im-qt`

### 왜 `unim-gnome`이 `unim-gui-gtk`를 Depends로 끌고 오는가

GNOME Shell extension의 "preferences" 엔트리포인트가 `unim-gui-gtk`를 launch한다. Recommends로 두면 `apt install --no-install-recommends` 환경에서 설정창 진입이 실패하므로 **Depends로 강제**.

---

## 3. 사용자 시나리오

| 환경 | 권장 패키지 집합 |
|---|---|
| GNOME Wayland / X11 | `apt install unim-common unim-gnome` (+ 필요 시 `unim-im-*`) or `apt install unim` (풀 스택) |
| KDE Plasma Wayland | `unim-common unim-im-gtk unim-im-qt unim-wayland unim-gui-qt` |
| KDE Plasma X11 | `unim-common unim-im-gtk unim-im-qt unim-xim unim-gui-qt` |
| Sway / Hyprland / COSMIC | `unim-common unim-im-gtk unim-im-qt unim-wayland unim-gui-gtk` |
| XFCE / MATE / Cinnamon (X11) | `unim-common unim-im-gtk unim-im-qt unim-xim unim-gui-gtk` |
| 최소 XIM only (i3, openbox) | `unim-common unim-im-gtk unim-xim unim-gui-gtk` |
| "다 설치" | `apt install unim` |

---

## 4. debian/ 파일 레이아웃

```text
debian/
├── changelog
├── control                       # 9 Package stanzas + Source
├── copyright
├── rules                         # dh 시퀀서 + Makefile install 위임
├── source/format
├── unim-common.install
├── unim-common.lintian-overrides
├── unim-common.prerm             # pkill unim-daemon
├── unim-im-gtk.install
├── unim-im-gtk.lintian-overrides
├── unim-im-gtk.postinst          # gtk-query-immodules-3.0 --update-cache
├── unim-im-gtk.postrm            # 동일
├── unim-im-qt.install
├── unim-im-qt.lintian-overrides
├── unim-xim.install
├── unim-xim.lintian-overrides
├── unim-xim.prerm                # pkill unim-xim
├── unim-wayland.install
├── unim-wayland.lintian-overrides
├── unim-wayland.prerm            # pkill unim-wayland
├── unim-gui-gtk.install
├── unim-gui-gtk.lintian-overrides
├── unim-gui-gtk.prerm            # pkill unim-gui-gtk
├── unim-gui-qt.install
├── unim-gui-qt.lintian-overrides
├── unim-gui-qt.prerm             # pkill unim-gui-qt
├── unim-gnome.install
├── unim-gnome.lintian-overrides
└── unim.lintian-overrides        # 메타 패키지 (파일 없음)
```

`debian/rules`는 기존 `dh $@` + `Makefile install DESTDIR=debian/tmp` 파이프라인을 그대로 사용. 각 `.install` 파일이 `debian/tmp`에서 해당 패키지로 파일을 분배한다.

---

## 5. lintian 전략

모든 `.lintian-overrides`는 패키지 단위로 분리되며, 각 항목에 정당화 주석을 둔다.

### 공통 override

- **`embedded-library libyaml`**: Rust crate `unsafe-libyaml` (pure-Rust rewrite). 시스템 libyaml.so 의존 없음. Rust 정적 아티팩트 내부. 적용 패키지: `unim-common`, `unim-xim`, `unim-wayland`, `unim-gui-gtk`, `unim-gui-qt`.
- **`initial-upload-closes-no-bugs`**: 실제 첫 업로드. 모든 패키지에 적용.

### `unim-common` 전용 override

- **`package-name-doesnt-match-sonames libunim-capi0`** + **`link-to-shared-library-in-wrong-package`**: `libunim_capi`는 외부 역의존 없는 내부 안정 C-API. `-dev` 분리 계획 없음. 미버전 `.so` 심볼릭 링크는 로컬 빌드 헬퍼 / gtk-common 테스트 앱이 기본명으로 resolve하도록 유지.

---

## 6. 검증 체크리스트

- [ ] `dpkg-buildpackage -us -uc -b -d` → 9개 `.deb` 생성
- [ ] `lintian -E -W ../unim*.deb ../unim_*.dsc` → warnings 0
- [ ] `dpkg -c` 각 `.deb` → 파일 중복 / 누락 없음
- [ ] `piuparts ../unim*.deb` → install / remove / purge 정상
- [ ] `apt install unim` → 전체 스택 설치 체감 일치
- [ ] `apt install unim-common unim-im-gtk unim-xim unim-gui-gtk` → 최소 X11 설치 동작
- [ ] `apt install unim-common unim-gnome` (+ Recommends 자동) → GNOME Wayland 동작
- [ ] `apt remove unim-gui-gtk` → `unim-gnome`도 함께 제거 (Depends 체인)

---

## 7. 히스토리

| 개정 | 날짜 | 요지 |
|---|---|---|
| r1 | 2026-04-17 | 4패키지 설계 (unim / unim-gui-gtk / unim-gui-qt / gnome-shell-extension-unim) |
| r2 | 2026-04-19 | 2패키지 최소화 (unim + gnome-shell-extension-unim) |
| **r3** | **2026-04-21** | **9패키지 분할 (현재)** — 사용자 선택권·툴킷 공존·최소 설치 케이스 확보 |
