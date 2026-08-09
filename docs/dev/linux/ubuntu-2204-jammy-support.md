# Ubuntu 22.04 (jammy) 지원 가능성 (조사 완료·미착수)

**조사일**: 2026-07-27 · **결론**: 기술적으로 가능하나 착수 보류. 현행 정책(noble 이상)은 유지.

공식 지원은 **Ubuntu 24.04 (noble) 이상**이다(`README.md`, `docs/user/user-guide/README-ko.md`).
이 문서는 jammy 지원을 검토할 때 같은 조사를 반복하지 않기 위한 기록이다.

## 1. 흔한 오판 — libadwaita 1.4 는 하드 블로커가 **아니다**

`libadwaita = { version = "0.7", features = ["v1_4"] }` 선언 때문에 "jammy(1.1.0)에서는
crate 자체가 불가"로 판단하기 쉬우나 **틀렸다**. `libadwaita-sys 0.7.2` 의
`[package.metadata.system-deps.libadwaita_1]` 은 케스케이드다 — base `"1"`(임의 1.x),
`v1_1`→`"1.1"`, `v1_2`→`"1.2"` … `v1_7`→`"1.7"`.

따라서 **crate 다운그레이드(0.5/0.4 등)는 불필요**하고, 같은 0.7 라인에서 feature 값만
낮추면 pkg-config 하한이 내려간다. 남는 문제는 "그 feature 에서 코드가 쓰는 심볼이
실재하느냐" 뿐이다. `gtk4-sys` 도 동일(base `4.0.0`).

### 크레이트별 실제 요구

| 크레이트 | 실사용 adw API | jammy 대응 비용 |
|---|---|---|
| `unim-indicator` | Application, StyleManager, ColorScheme (1.0급) | **Cargo.toml 1줄** (코드 0) |
| `unim-popup-service` | 위 + Window::builder (1.0급) | **Cargo.toml 1줄** (코드 0) |
| `unim-keymap-common` | **0건** (사문 의존성) | **Cargo.toml 1줄** (코드 0) |
| `unim-typing-practice` | MessageDialog 4곳 (1.2) | 소규모 + GTK4 블로커 |
| `unim-settings-gtk` | SwitchRow 15(1.4)·EntryRow 4(1.2)·ToolbarView 1(1.4) | 중규모 (1~2인일) |
| `unim-keymap-studio` | EntryRow 24(1.2)·SwitchRow 4(1.4)·MessageDialog 9(1.2)·Banner 1(1.3) | 대규모 (수일~1주+) |

### GTK4 `v4_10` 게이트

실사용 4.10 API 는 **`gtk::FileDialog` 하나**뿐 — `unim-keymap-studio/src/dialogs/import_export.rs`
2곳, `unim-typing-practice/src/practice_page.rs` 1곳. FFI 심볼(`gtk_file_dialog_new`) 자체가
4.10 산이라 링크 하드 요구지만 legacy `GtkFileChooserNative` 로 대체 가능.
GTK4 **IM 모듈(C)** 은 무관 — `gtk_widget_get_native`/`gtk_native_get_surface(_transform)`/
`gtk_widget_compute_point`/`gtk_im_context_delete_surrounding` 전부 4.0 API 라 4.6.9 로 충분.

## 2. 진짜 제약 — GNOME Wayland 에서 팝업이 없다

`docs/dev/specs/POPUP_SPEC.md:452,786` 대로 **GNOME Wayland 에서 한자/특수문자/이모지 팝업의
유일한 렌더러는 GNOME 확장**이다(Mutter 가 wlr-layer-shell·input_popup_v2 미지원 →
popup-service GTK4 창 표시 불가). 그런데 확장은 GNOME 45+ 전용이다 —
`metadata.json` shell-version `["45"…"48"]` + ESM 문법이라 GNOME 42 에서 **로드 자체가 거부**된다.

환경별로 결과가 갈린다:

| 환경 | 결과 |
|---|---|
| X11 / 비-GNOME jammy 파생 (Mint 21 Cinnamon, elementary 7, Pop!_OS 22.04 Xorg) | 입력·트레이·**팝업까지 사실상 완전 기능** |
| **Ubuntu 22.04 정품 (GNOME Wayland = 기본 세션)** | 한글 조합은 되나 **한자 변환 불가**. Xorg 세션 안내 필수 |
| KWin / wlroots Wayland | `gtk4-layer-shell` 이 jammy 아카이브에 없어 경로 자체가 닫힘 |

GNOME 확장 포크가 필요하다면 대상은 **42~44 레거시 세대**다(Pop!_OS 22.04 = 순정 GNOME 42,
Zorin 17 = GNOME 43). `clutter_input_method_set_preedit_text` 인자 수 차이
(42: 3-인자 / 45+: 4-인자, `unim-gnome-extension/unim_input_method.js:552`) 등 API 대조가 필요.

## 3. 미검증 게이트 (착수 시 최우선)

1. **Slint `unim-settings` 의 skia 링크** — `skia-bindings` prebuilt 가 glibc 2.35 에서 붙는가.
   이것이 유일한 설정 GUI 다(트레이 "설정" 메뉴가 spawn: `unim-indicator/src/gtk_ui.rs:59`).
   실패 시 남는 설정 수단은 `unim-cli config` 뿐. `renderer-femtovg` 폴백은 14px 한글 힌팅
   열화라 무해하지 않다(선택 사유가 주석에 있음).
2. **Qt6 6.2.4 private QPA 헤더** — `qpa/qplatforminputcontextplugin_p.h`(`qt6/src/plugin.cpp:8`),
   `Qt6::GuiPrivate`(CMakeLists.txt:38). Qt 는 private 헤더의 마이너 간 소스 호환을 보장하지 않는다.
3. **런타임 로드** — 컨테이너 빌드는 컴파일만 증명한다. immodule 실제 로드, libadwaita 1.1
   스타일 차이로 인한 조용한 시각 회귀, GNOME 42 의 SNI 트레이(순정 미지원 — Ubuntu
   appindicator 확장 의존), 팝업 좌표는 실기 세션에서만 확인 가능.

## 4. 옵션과 비용

| 옵션 | 범위 | 비용 |
|---|---|---|
| **A. 코어 서브셋** | 입력 100%(엔진·데몬·CLI·XIM·Wayland·GTK3/4·Qt5/6) + 트레이 + (X11 한정)팝업 + (게이트1 통과 시)설정앱. 제외: settings-gtk, keymap-studio, typing-practice, GNOME 확장 | 스파이크 0.5~1일 → 구현 1~2주(debian 트리 5~6파일 포크·CI 컨테이너 잡·install.sh 코드명 매핑·문서 3종·실기 QA) + 릴리스마다 이중 빌드 |
| B. adw 1.1 이중 경로 (`cfg`) | A + GUI 3종 | 1~2주. 67+ 콜사이트 영구 이중화 — **비권장** |
| B'. 단일 하향 통일 | 전 배포판 공통 소스 | 4~6일. noble UX 후퇴 → UX 원칙 충돌, 비권장 |
| C. GNOME 42~44 확장 포크 | jammy GNOME Wayland 팝업·패널 | 1~2주 + 상시 VM QA |
| D. Slint 이관 | 장기적으로 adw 의존 축 소멸 | 수 주. B 대신 택하면 이중 경로 부채 0 |

**착수 시 첫 단계는 옵션과 무관하게 동일**: `ubuntu:22.04` 컨테이너 스파이크(0.5~1일) —
`v1_4` 3줄 제거 후 코어+immodule+indicator+popup-service 컴파일, `-p unim-settings` skia 링크,
`qt6-base-private-dev`(6.2.4) cmake. 셋 중 하나라도 실패하면 옵션 A 의 범위가 바뀐다.

## 5. 착수 시 지켜야 할 불변식

- **cargo feature 는 빌드 대상 전체에 통합된다.** jammy 프로파일에서 빌드되는 어떤 워크스페이스
  멤버도 `libadwaita` `v1_2`+ / `gtk4` `v4_10` 을 선언해서는 안 된다. 하나라도 남으면 pkg-config
  하한이 다시 올라가 빌드 전체가 깨지고, 에러 메시지가 원인을 가리키지 않는다.
  → CI jammy 잡에 대상 크레이트 `Cargo.toml` 을 grep 해 즉시 실패시키는 값싼 가드를 둘 것.
- **jammy 표준 지원 종료(2027-04)를 문서에 못 박고 시작할 것.** 시한이 없으면 이중 산출물이
  무기한 유지된다.

## 부록 — jammy 제공 버전 (조사 시점)

libadwaita 1.1.0 · GTK4 4.6.9 · GTK3 3.24.33 · glib 2.72 · Qt5 5.15.3 · Qt6 6.2.4 ·
GNOME Shell 42 · gcc 11 · clang 14 · cmake 3.22 · glibc 2.35 · distro rustc 1.58(→ rustup 필요, MSRV 1.78)

상세 조사 원본(트랙별 근거·적대 검증 8건): `생각 모음/2 Projects/ATIT/unim/archive/2026-07-27-jammy-build-feasibility.md`
