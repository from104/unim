# Wayland Input Method Architecture Research

Date: 2026-03-28

## 핵심 질문: GNOME Shell extension만으로 GTK/Qt IM module을 대체할 수 있는가?

**결론: 아니오. GTK/Qt IM module은 여전히 필요하다.**

---

## 1. GNOME Shell의 내장 입력기(IBus/InputMethod vfunc)는 모든 앱을 커버하는가?

**부분적으로만 커버한다.**

GNOME Wayland의 입력 흐름:
```
키입력 → Mutter (compositor) → text-input-v3 protocol → 앱(GTK/Qt)
                ↕ (D-Bus)
         ibus-daemon ←→ GNOME Shell (panel 역할)
```

- GNOME Shell은 ibus-daemon과 **D-Bus**로 통신한다 (Wayland input-method protocol을 사용하지 않음)
- GNOME Shell은 ibus-daemon을 `--panel disable` 옵션으로 systemd를 통해 실행하고, Shell 자체가 panel(후보창) 역할을 수행
- 앱과 Mutter 사이는 **text-input-v3** 프로토콜을 사용
- 커버리지 범위:
  - **GTK3/GTK4 native Wayland 앱**: text-input-v3 지원 → 기본적으로 동작
  - **Qt6 >= 6.7 native Wayland 앱**: text-input-v3 지원 (6.8.2+ 권장)
  - **Qt5 native Wayland 앱**: text-input-v2만 지원 → Mutter는 v2 미지원 → **IM module 필요**
  - **XWayland 앱**: text-input 미사용 → **XIM/IM module 필요**
  - **Chromium/Electron**: 복잡한 상황 (text-input-v1 기본, GTK4 모드로 전환 필요)

## 2. GTK3/GTK4 앱에 별도 IM module이 필요한가? (GNOME Wayland)

**Native Wayland GTK 앱은 text-input-v3로 동작하지만, IM module이 더 나은 경험을 제공한다.**

- GTK3/GTK4는 text-input-v3를 네이티브 지원
- `GTK_IM_MODULE`을 설정하지 않으면 자동으로 text-input-v3 사용
- Ubuntu는 2020년부터 `GTK_IM_MODULE=ibus` 설정을 제거함 (im-config 0.45-1)
- **하지만** text-input-v3의 한계:
  - Preedit 스타일이 제한적 (bold 하이라이트만 가능)
  - 일부 GTK4 popover에서 입력기 전환 불가 버그 존재 (ibus#2638)
  - 커스텀 기능(예: 한자 후보창 위치 제어)을 IM module이 더 정밀하게 처리
- **Fcitx5 권장사항**: GTK_IM_MODULE을 환경변수로 설정하지 말고, `~/.config/gtk-3.0/settings.ini`와 `~/.config/gtk-4.0/settings.ini`에서 per-toolkit 설정 권장 (X11용 IM module, Wayland용 text-input 분리)

## 3. Qt5/Qt6 앱에 별도 IM plugin이 필요한가? (GNOME Wayland)

**Qt5는 반드시 필요. Qt6도 현실적으로 필요.**

- **Qt5**: Wayland에서도 기본적으로 XWayland를 사용하는 경우가 많고, text-input-v2만 지원. Mutter는 text-input-v2 미지원이므로 `QT_IM_MODULE=fcitx/ibus` 필수
- **Qt6 < 6.7**: text-input-v3 미지원 → IM module 필수
- **Qt6 6.7~6.8.1**: text-input-v3 지원하지만 버그 존재
- **Qt6 >= 6.8.2**: text-input-v3 안정적 지원. `QT_IM_MODULES="wayland;fcitx"` fallback 설정 가능
- **현실**: 대부분의 Qt 앱이 아직 Qt5/Qt6 < 6.8.2이므로 IM module이 필수

## 4. XWayland 앱은?

**반드시 별도 처리 필요.**

- XWayland 앱은 X11과 동일한 방식으로 입력 처리
- `XMODIFIERS=@im=ibus` (또는 fcitx) 설정 필수
- GTK2, SDL1, Xlib 기반 앱, 대부분의 Electron/Chromium (기본값이 XWayland)
- XIM 프로토콜 또는 toolkit별 IM module로 처리
- GNOME Shell extension은 XWayland 앱에 직접 입력을 전달할 수 없음

## 5. Non-GNOME Wayland (Sway, Hyprland, KDE)?

**완전히 다른 아키텍처. GNOME Shell extension은 무용.**

| Compositor | Input Method Protocol | 비고 |
|---|---|---|
| Sway | input-method-v2 + text-input-v3 | IBus 1.5.32+, Fcitx5 지원. Sway 1.10+ 필요 |
| Hyprland | input-method-v2 + text-input-v3 | IBus 1.5.32+ 지원 |
| KDE Plasma | input-method-v1 + text-input-v1/v2 | IBus 1.5.29+, 자체 VK 시스템 |
| COSMIC | input-method-v2 | IBus 1.5.32+ 지원 |
| XFCE 4.20 | input-method-v2 (Labwc 기반) | IBus 1.5.32+ 지원 |
| Weston | input-method-v1 | 레거시 |

- 이 환경들에서는 `ibus start --type wayland` 또는 `ibus start --type kde-wayland`로 실행
- Fcitx5는 Wayland frontend로 직접 compositor와 통신
- **GTK/Qt IM module은 여전히 필요** (특히 Qt5, text-input 미지원 앱)

## 6. IBus/Fcitx5는 GNOME extension이 있어도 GTK/Qt module을 배포하는가?

**그렇다. 모두 별도 패키지로 GTK/Qt module을 배포한다.**

### IBus 패키지 구조:
- `ibus` - 코어 데몬
- `ibus-gtk2` / `ibus-gtk3` / `ibus-gtk4` - GTK IM module
- `ibus-qt` - Qt IM module (별도 프로젝트)
- GNOME에서는 Shell이 panel 역할을 하지만, GTK/Qt IM module은 여전히 설치됨

### Fcitx5 패키지 구조:
- `fcitx5` - 코어 데몬
- `fcitx5-gtk` - GTK2/3/4 IM module (필수 권장)
- `fcitx5-qt` - Qt5/6 IM module (필수 권장)
- `fcitx5-module-kimpanel` - GNOME kimpanel extension 연동

### 이유:
1. **XWayland 앱** 지원을 위해 필수
2. **Qt5** (text-input-v2만 지원, Mutter 미지원)를 위해 필수
3. **Popup 위치 제어**: IM module은 클라이언트 프로세스 내에서 팝업을 렌더링하여 정확한 위치 가능
4. **Preedit 품질**: text-input-v3의 preedit 스타일이 제한적
5. **기능 완전성**: IM module이 모든 Fcitx/IBus 기능을 지원

---

## UNIM에 대한 시사점

### GNOME Shell extension만으로는 불가능한 이유:

1. **XWayland 앱**: extension에서 접근 불가. XIM frontend 또는 GTK/Qt IM module 필요
2. **Qt5 앱**: text-input-v3 미지원. IM module 필수
3. **Qt6 < 6.8.2**: 불안정. IM module 권장
4. **Popup 위치**: extension은 GNOME Shell UI 위에만 표시 가능, text-input-v3 경유 시 위치 제어 제한
5. **Non-GNOME 환경**: extension 무용. Wayland frontend + IM module 필요

### 권장 아키텍처:

```
[GNOME Wayland]
  GTK3/4 앱 (native) → text-input-v3 → Mutter → (D-Bus) → unim-daemon
                       또는 GTK IM module → (D-Bus) → unim-daemon
  Qt5 앱             → Qt IM module → (D-Bus) → unim-daemon  (필수)
  Qt6 >= 6.8.2 앱    → text-input-v3 → Mutter → (D-Bus) → unim-daemon
                       또는 Qt IM module → (D-Bus) → unim-daemon
  XWayland 앱        → XIM / IM module → (D-Bus) → unim-daemon  (필수)
  GNOME Shell UI     → extension → (D-Bus) → unim-daemon

[Non-GNOME Wayland (Sway, Hyprland 등)]
  모든 앱 → Wayland frontend (input-method-v2) → (D-Bus) → unim-daemon
  XWayland 앱 → XIM → (D-Bus) → unim-daemon
  Qt5 앱 → Qt IM module → (D-Bus) → unim-daemon

[X11]
  GTK 앱 → GTK IM module → (D-Bus) → unim-daemon
  Qt 앱  → Qt IM module → (D-Bus) → unim-daemon
  기타   → XIM → (D-Bus) → unim-daemon
```

### 결론: 현재 UNIM의 3-layer 아키텍처는 올바르다

- GTK3/4 IM module: **유지 필요** (XWayland, X11, preedit 품질)
- Qt5/6 IM module: **유지 필요** (Qt5 필수, Qt6 권장)
- GNOME Shell extension: **유지 필요** (GNOME Shell UI 자체의 입력 처리)
- Wayland frontend: **유지 필요** (Sway, Hyprland 등 non-GNOME 환경)
- XIM frontend: **유지 필요** (XWayland 레거시 앱)

**단, GNOME Wayland에서 GTK IM module의 역할은 축소되는 추세.** text-input-v3 경유가 기본이 되면서, GTK IM module은 "더 나은 경험"을 위한 선택적 컴포넌트로 전환 중. Qt5는 여전히 필수.

---

## Sources

- [Using Fcitx 5 on Wayland](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland) - 가장 포괄적인 Wayland IM 가이드
- [IBus Wayland Desktop Wiki](https://github.com/ibus/ibus/wiki/WaylandDesktop) - IBus의 각 compositor별 설정
- [Ubuntu: IBus No More GTK_IM_MODULE](https://discourse.ubuntu.com/t/ibus-no-more-gtk-im-module-ibus/17727) - GTK_IM_MODULE 제거 결정
- [Fcitx5 ArchWiki](https://wiki.archlinux.org/title/Fcitx5) - 실용적 설정 가이드
- [IBus Issue #2638](https://github.com/ibus/ibus/issues/2638) - GTK4 popover 입력기 전환 버그
- [GNOME GTK Text Input Protocol (Phoronix)](https://www.phoronix.com/news/GNOME-Wayland-GTK-Input-Proto) - GNOME의 text-input 구현
- [Clutter.InputMethod API](https://mutter.gnome.org/clutter/class.InputMethod.html) - Mutter 내부 IM 구현
- [openSUSE Wayland Input Methods](https://en.opensuse.org/SDB:Wayland_input_methods) - 종합 가이드
- [Wayland text-input-v3 Protocol](https://wayland.app/protocols/text-input-unstable-v3) - 프로토콜 사양
- [Wayland input-method-v2 Protocol](https://wayland.app/protocols/input-method-unstable-v2) - 프로토콜 사양
