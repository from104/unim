# UNIM 프로젝트 로드맵

이 문서는 **UNIM** 프로젝트의 장기 목표와 개발 단계별 계획을 설명합니다.

> 기준 버전: **0.4.0** (2026-08-10)

## 🎯 핵심 목표

언어 상태 자동 감지 및 수동 텍스트 변환 기능을 갖춘, 하나로 통합된 크로스 플랫폼(Windows, macOS, Linux) 한국어 입력기 엔진(IME)을 구축하는 것입니다.

## 📍 현재 위치 (0.4.0 기준)

| 구분 | 내용 |
|------|------|
| **완료** | Rust 코어(2벌식·3벌식 계열·안마태), 3계층 아키텍처(Core → D-Bus → Frontend), 전 프론트엔드(GTK3/4·Qt5/6·XIM·Wayland·GNOME Shell·Windows TSF/IMM32), 자판 프로필 v1, 자동 오타 교정 + 억제 사전, 비밀번호 필드 보호, 배포 채널(deb 11종·rpm·MSI·설치 스크립트), 툴킷 실화면 자동시험 하네스 |
| **진행 중** | Windows 완성도 다듬기, 단독 Wayland 컴포지터(KDE 6·Sway·Hyprland) 실측, 샌드박스 앱(Flatpak·Snap) 입력 경로 복구, 문서 정비 |
| **다음** | 4단계 문맥 감지(한/영 자동 전환) — 이 프로젝트의 원래 목표입니다 |
| **그 뒤** | 6단계 엔진 재설계 → 7단계 입력 방식·플랫폼 확장 |

## 🛣️ 개발 단계

### 1단계: 기반 구축 및 리눅스 네이티브 (완료)

- [x] 한글 조합 로직을 갖춘 견고한 Rust 코어 라이브러리 개발.
- [x] 데이터 자산이 내장된 이식 가능한 `unim-cli` 구현.
- [x] `St.Clipboard`와 `Clutter`를 사용한 네이티브 GNOME Shell 확장 프로그램 개발.
- [x] 안정성을 위한 하이브리드 아키텍처(CLI + 네이티브 API) 적용.

### 2단계: 3계층 아키텍처 및 전체 프론트엔드 (완료)

- [x] **DBus 데몬 아키텍처**: `unim-daemon` + `unim-dbus` 기반 중앙 엔진 서비스 구축.
- [x] **GTK3/GTK4 IM 모듈**: C 언어 기반 IM Module 구현 (공통 코드 `gtk-common` 분리).
- [x] **Qt5/Qt6 플러그인**: C++ 기반 QPlatformInputContext 플러그인 구현 (공통 코드 `qt-common` 분리).
- [x] **XIM 프론트엔드**: Rust `xim` crate 기반 X11 XIM 서버 구현 (Over-The-Spot Preedit, 프로토콜 적합성 검증 완료).
- [x] **Wayland 프론트엔드**: `input-method-v2` + `virtual-keyboard-v1` 프로토콜 기반 구현 (KDE Plasma 지원).
- [x] **한자/특수문자/이모지 입력**: 모든 팝업이 `unim-popup-service` 단일 서비스로 중앙화 완료. GNOME Wayland만 Shell extension `popup_view.js`(St 위젯)로 자체 렌더, 그 외 환경(X11·기타 Wayland)은 popup-service(GTK4)가 전담. GTK/Qt IM 모듈의 임베디드 팝업 위젯은 제거됨. 한자키 하나가 조합 상태에 따라 한자·특수문자·이모지로 자동 분기.
- [x] **설정 도구**: 통합 설정 창 + CLI (`unim-cli config`).
- [x] **시스템 트레이**: 트레이 인디케이터(`unim-indicator`) 별도 프로세스로 분리.

### 3단계: 문서화 및 안정화 (진행 중)

- [x] **컴포넌트별 SPEC.md 작성**: 12개 컴포넌트 기능 명세 문서화.
  - `src/`, `unim-capi/`, `unim-cli/`, `unim-daemon/`, `unim-dbus/`
  - `unim-frontends/gtk3/`, `gtk4/`, `qt5/`, `qt6/`, `xim/`, `wayland/`
- [x] **XIM 프로토콜 적합성 검증**: [XIM 사양](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html) 대비 3회 교차 검증 (11개 항목 적합).
- [x] **Wayland 프로토콜 참조 문서화**: `input-method-v2`, `virtual-keyboard-v1` 프로토콜 사양 참조 및 아키텍처 문서화.
- [x] **Wayland 키 반복(Key Repeat)**: `mio` + `timerfd` 기반 구현 완료 (`unim-frontends/wayland/src/repeat.rs`).
- [x] **Wayland 팝업 경로 정리**: 초기의 `zwp_input_popup_surface_v2` 자체 렌더 구현은 **팝업 중앙화(2단계) 과정에서 제거**했고, 현재 Wayland 환경의 팝업은 GNOME이면 Shell extension이, 그 외 컴포지터면 `unim-popup-service`(GTK4 + `wayland-backend` feature, gtk4-layer-shell)가 맡습니다. Wayland 프론트엔드 크레이트에는 팝업 렌더 코드가 남아 있지 않습니다.
- [x] **Surrounding Text / Content Type**: 코어에 `src/input_engine/surrounding.rs` 를 두고 GTK3/4·Qt·XIM·Wayland·GNOME 확장·TSF·IMM32 전 경로에 배선 완료. `content_purpose` 는 비밀번호 필드 보호(4단계)의 판정 근거로 실사용 중이며, GNOME Wayland 경로의 빈 껍데기 구현도 0.4.0에서 메웠습니다.
- [x] **패키지 안정화**: deb 11종 + rpm + MSI 빌드·설치 검증. 한 줄 설치 스크립트(리눅스 `install.sh` / Windows `install.ps1`)에 SHA256 검증과 `--update` / `--check` 경로 포함.
- [x] **툴킷 실화면 자동시험 하네스**: 6개 툴킷 테스트 앱이 코어 필드를 캔버스에 **직접 그려** preedit 을 100% 관측하고, `xdotool`(XTEST)로 실제 키를 주입해 툴킷 → IM 모듈 → 데몬 전 구간을 검사합니다. 판정 기준은 `field.render` 사건 하나로 통일했습니다. 명세는 [`docs/dev/testing/TEST_APPS.md`](docs/dev/testing/TEST_APPS.md), 실행은 `make test-apps`.
- [ ] **단독 Wayland 컴포지터 실측**: KDE 6 Wayland·Sway·Hyprland 에서 팝업 위치·IME 포커스 전환·좌표 변환 확인. 코드 경로는 있으나 실기기 검증 전입니다.
- [ ] **샌드박스 앱(Flatpak·Snap) 입력 경로**: 샌드박스 안에는 호스트의 `im-unim.so` 가 보이지 않아 GTK 가 **XIM 으로 폴백**합니다. 옵시디언(Electron)이 대표 사례입니다. 두 경로 모두 결함이 남아 있어, 현재 이런 앱에서는 한글 입력이 온전하지 않습니다.
  - [x] **IBus 호환 레이어 복구** (`fbffe56`): Flatpak 런타임에는 `im-ibus.so` 가 있고 UNIM 은 `org.freedesktop.IBus` 와 `org.freedesktop.portal.IBus` 를 이미 등록하므로 이 길이 정공법입니다. 그런데 경로 자체가 처음부터 동작하지 않았습니다 — ① GVariant 직렬화가 모든 필드를 `v` 로 내보내 클라이언트가 거부(IBusText 는 `(sa{sv}sv)` 계약), ② keycode 이중 차감(GTK `im-ibus` 가 이미 `hardware_keycode - 8` 을 보냄), ③ attribute 인덱스에 문자 대신 바이트 길이. 셋을 고쳐 **한글 커밋과 연속 입력이 정상 동작**합니다. 값이 아니라 **wire 시그니처를 단언하는 테스트**도 함께 넣었습니다(기존 테스트는 variant 껍질을 벗겨 내서 형식 오류를 통과시켰습니다).
  - [ ] **IBus `UpdatePreeditText` 미도달**: `CommitText` 는 도달하는데 preedit 만 화면에 나오지 않습니다(밑줄 속성 유무와 무관). 이 하나를 풀면 샌드박스 앱 지원이 완성됩니다.
  - [ ] **XIM `PreeditDraw` 정지**: `PreeditDraw` 를 보내면 그 IC 가 **다음 키를 받지 못합니다**(3/3 재현). 확정 직후 다음 글자가 통째로 씹히는 증상의 정체입니다. 배제된 가설 8건은 조사 기록에 있으며, 다음 후보는 Property 전송 경로(`send_req_impl`)입니다.

### 3.5단계: UI 프런트엔드 분리 (Fcitx5 스타일) — 완료

엔진(daemon)과 UI(팝업·인디케이터·설정)를 DBus 시그널 기반으로 완전 분리했습니다.

- [x] **unim-gui 모듈 분리**: DBus, 트레이, UI 모듈 분리 완료.
- [x] **unim-gui-common 크레이트**: DBus 통신 + 트레이 등 공통 로직 추출 완료.
- [x] **설정 앱 통합 (Slint)**: GTK4 판(`unim-settings-gtk`)과 cxx-qt 기반 Qt6 판을 각각 두던 구조를 접고, **Slint 기반 `unim-settings` 한 벌로 통합**했습니다. 리눅스와 Windows 가 같은 설정 화면을 씁니다. 구 GTK4 판은 `unim-settings-gtk` 로 개명해 당분간 함께 배포하며 추후 퇴역합니다.
- [x] **Debian 패키지 재구성**: **11개 바이너리 패키지**로 분할 — `unim-common` / `unim-im-gtk` / `unim-im-qt` / `unim-xim` / `unim-wayland` / `unim-desktop` / `unim-settings` / `unim-keymap-studio` / `unim-typing-practice` / `unim-gnome` / `unim`(메타). 인디케이터·팝업 서비스·레거시 GTK 다이얼로그는 `unim-desktop` 에 묶었고, `apt install unim` 한 줄로 full stack 이 설치됩니다.

### 3.7단계: 자판 프로필 v1 → v3 (완료)

자판 정의를 하드코딩 Rust const에서 **자기 완결 JSON**으로 이관했습니다. 사용자 자판(`~/.config/unim/layouts/*.json`) + 상속(`inherits`) + 선택형 규칙 세트(rule_sets)를 지원하며, 6단계 엔진 재설계의 데이터 기반이 됩니다.

스키마는 세 판을 거쳤고 로더가 **v1·v2·v3를 모두 파싱**합니다 — v2는 키별 메타데이터(`key_meta`), v3는 모아치기(`supports_moachigi` + `chord_window_ms`)와 옛한글 명시적 거부(`LoadError::ArchaicJamoNotSupported`)를 추가했습니다. 내장 키맵은 현재 v1 17종·v2 2종·v3 2종입니다. v3 명세는 [`docs/dev/architecture/LAYOUT_PROFILE_V3.md`](docs/dev/architecture/LAYOUT_PROFILE_V3.md)에 있습니다.

- [x] **v1 스키마 정의** ([`docs/archive/plans/LAYOUT_PROFILE_V1.md`](docs/archive/plans/LAYOUT_PROFILE_V1.md)): schema_version, metadata(다국어), inherits, combinations(자기 완결), rule_sets, active_rule_sets. v0 하위 호환 자동 승격.
- [x] **Phase 1·2 — 로더·빌더·Composer 통합**: `src/keystroke/profile/` 하위에 schema/loader/builder/localized 신설. `HangulComposer{2,3}Bul::new_with_profile` + v0→v1 동일 결과 regression.
- [x] **Phase 3 — 레지스트리·상속·핫리로드**: `ProfileRegistry`(내장 + `~/.config/unim/layouts` 통합 네임스페이스, 사용자 우선), `inherit::resolve`(재귀 해석 + 순환 탐지 + layer-merge), 디렉토리 mtime 기반 자동 재스캔.
- [x] **Phase 4 — Config·CLI·DBus·엔진 연결**: `korean.custom_layout`(`Option<String>`)·`korean.active_rule_sets`(`Vec<String>`) 필드 5-point 싱크. `unim-cli config layout list/describe/validate` 서브커맨드. `InputEngine::new`가 ProfileRegistry를 거쳐 효과적 프로필을 로드하고, 실패 시 enum 경로로 폴백.
- [x] **Phase 5 — 설정 GUI**: 한국어 자판 선택 목록이 모든 한국어 프로필(내장 + 사용자)을 표시하고, 선택 시 규칙 세트 스위치가 동적으로 재구성됩니다.
- [x] **Phase 6 — 내장 10종 v1 이관**: `docs/references/keymaps/*.json` 9종 + 신규 `ko_3bul_qwerty` 1종을 `src/keystroke/keymap/`로 이관. 기존 Rust const와 동일한 `CombinedJamoMap` 산출 (behavior-preserving, regression test로 고정).
- [x] **Phase 7 — 문서·마이그레이션 공지**: 본 섹션 + CHANGELOG Added 블록 + README 안내.
- [x] **키맵 도구 제공**: 자판을 눈으로 보고(보기), 편집하고(키맵 스튜디오), 익히는(타자 연습) GTK4 도구 3종. 키맵 스튜디오·타자 연습은 5행 키보드 위젯을 공유하며, 키맵 스튜디오는 헤더 3단 드롭다운(언어 › 출처 › 자판) + 4탭 구성에 빌트인 보호 / 사용자 자판 저장 정책을 둡니다.

### 4단계: 자동 상태 전환 (지능화) — 진행 중

- [ ] **문맥 감지**: 입력 필드 상태나 언어 문맥을 보고 한/영을 **자동으로 전환**하는 것. 이 프로젝트의 원래 목표이며, 다음 큰 걸음입니다. `surrounding_text`(3단계 완료)가 기반 자산이 됩니다.
- [x] **자동 교정 엔진 (AutoTypeFix)**: 실시간 오타 감지 구현. forward(영→한: `gksrmf` → `한글`), reverse(한→영: `ㅈㅐㅍㅁ` → `wave`) 양방향 지원. XIM·GTK3/4·Qt5/6·Wayland·GNOME Shell·Windows(TSF·IMM32) 전 프론트엔드 통합. 전체 / 순방향 / 역방향 토글 단축키를 각각 지정할 수 있습니다(전체 기본값 `Shift+F8`). (`src/auto_typefix/`)
- [x] **사용자 학습 — 억제 사전(Blacklist)**: 롤백 관측(BS + 모드 전환) + 재시도 시점 자동 등록 방식으로 "원치 않는 교정" 단어를 Tentative로 학습. GUI에서 Confirm 시 Confirmed, 시간 만료 시 Inactive. `~/.config/unim/typefix-blacklist.yaml`에 저장, 데몬 mtime 핫리로드. (`src/typefix_blacklist.rs`)
- [x] **비밀번호 필드 보호**: `content_purpose` 를 근거로 비밀번호 칸에서 영문 모드로 강제 전환하고, 그동안의 키를 버퍼·실행취소·학습 사전 어디에도 남기지 않습니다. 리눅스 전 경로 + Windows TSF 는 배선 완료, IMM32 는 최선노력 수준입니다.
- [ ] **사용자 학습 — 양성 사전**: 사용자별 타이핑 패턴 기반의 *긍정적* 로컬 사전(오타 교정 promotion)은 미구현입니다. 현재 Blacklist는 교정 제외만 담당합니다.

### 5단계: 크로스 플랫폼 확장 — Windows 완료, macOS 미착수

- [x] **Windows 네이티브 입력기**: TSF 텍스트 서비스(`unim-tsf`) + IMM32 폴백(`unim-imm32`) 구현. 조합·팝업·자동 오타 교정·설정·언어바가 단일 DLL(32/64비트)에 들어가며, 데몬 없이 코어를 in-process 로 링크합니다. 트레이·팝업은 `unim-popup-win`, 배포는 MSI. **리눅스 코어를 수정하지 않는다**는 제약을 지켜 포팅했습니다.
- [x] **IBus 호환 레이어**: `unim-dbus/src/ibus_compat/`(약 1,300줄, address·context·portal·service·types)로 IBus 클라이언트 경로를 수용합니다. 검증 스크립트는 `tests/test_ibus_compat.py`.
- [ ] **macOS 입력기**: 미착수. 상세는 7단계 ②.
- [ ] **fcitx5 연동**: 미착수. IBus 호환 레이어와 달리 아직 조사 단계도 아닙니다.

> **메모 — `unim-capi` 위치**: 현재 UNIM 내부 컴포넌트는 모두 DBus 또는 Rust API를 직접 사용해 unim-capi를 링크하는 in-tree 소비자가 없습니다(프런트엔드의 capi 링크 의존도 해제됨). unim-capi는 외부 프로그램이 UNIM 코어를 임베딩하기 위한 **공개 C API**로 유지하며, 공개 헤더 `unim.h`는 빌드 시 Rust 표면(현재 **`extern "C"` 함수 67개**)과의 드리프트를 자동 검사합니다. 위 크로스 플랫폼 임베딩의 토대가 됩니다.

### 6단계: 엔진 재설계 (고급 한글 입력 기법 지원)

현재 UNIM의 한글 엔진은 **정적 키맵 + 하드코딩 오토마타** 구조라 아래 기능들을 표현할 수 없습니다. 복벌식·갈마들이 조사(`docs/references/research/복벌식·갈마들이 조사.md`)와 순아래받침 규칙 조사(`docs/references/research/순아래받침_규칙.md`)에서 드러난 공통 결론은 이렇습니다 — **낱자에 "어디서 왔는지" 정보가 붙어야 하고, 키 해석이 컴포저 상태에 접근할 수 있어야 합니다.** 이 두 전제를 도입하는 엔진 리팩터가 아래 모든 항목의 선행 조건입니다.

- [ ] **낱자 provenance 태깅**: `Jamo` 표현을 `(kind, source_key)` 튜플로 확장해 같은 ㅗ/ㅜ라도 어느 키에서 왔는지 구별합니다. 세벌식 390의 `9`-ㅜ, `/`-ㅗ 이중모음 전용 역할, 복벌식 자동 판정의 근거가 되는 날개셋문자 64-bit 토큰 개념(연구 문서 §4.1)에 대응합니다.
- [ ] **문맥 의존 키 해석 (글쇠 수식 최소 집합)**: 키→자모 매핑이 컴포저 상태(`has_cho`/`has_jung`/`has_jong`/`syllable_empty`)를 조회할 수 있도록 predicate 엔진을 도입합니다. 두벌식 `/`, 세벌식 390 `/` 같은 적응형 글쇠(연구 문서 §4.2)를 지원합니다. 날개셋의 Turing-complete 수식 전면 이식은 별도 과제입니다.
- [ ] **자판 프로필 v4**: 현재 스키마는 v3까지 나와 있으나(3.7단계) provenance + predicate 필드는 세 판 모두 유보했습니다. v4에서 이를 추가해 세벌식 390 원본 규약을 있는 그대로 재현합니다. v3 명세 §11이 v4로 넘긴 항목(옛한글 입력, 두벌식 모아치기)도 여기서 함께 다룹니다.
- [ ] **모아치기 stroke replay**: 동시 입력 자체는 이미 됩니다 — 안마태 자판용 chord 윈도우 버퍼(`src/input_engine/chord_buffer.rs`, 기본 60ms)가 0.3.0부터 동작합니다. 남은 것은 **버퍼에 모인 낱자를 순서와 무관하게 재배열해 한 음절로 확정**하는 부분으로, provenance 태깅이 있어야 어느 낱자가 초성 자리인지 결정할 수 있습니다.
- [ ] **복벌식**: 어절 첫 타자의 손 위치(좌/우)로 두벌식·세벌식을 자동 전환합니다(연구 문서 §5.2). 어절 단위 버퍼 + 첫 낱자 provenance가 전제입니다.
- [ ] **옛한글**: U+1100 확장 블록 낱자, 방점, 합용병서 지원. Jamo enum 확장 + 고급 문자 생성기가 필요합니다. 현재 v3 로더는 옛한글 코드포인트를 **명시적으로 거부**하며(단순화를 위한 의도된 선택), 이 항목은 그 결정을 뒤집는 작업입니다.
- [ ] **초·종성 공유 결합 규칙 (shared combination)**: 날개셋 §5.1의 "종성이 초성 결합 규칙을 중첩 적용" 동작을 엔진 레벨에서 직접 표현합니다. v1의 `share_cho_jong` 플래그는 복제 수준에 그칩니다.
- [ ] **날개셋 `.ist` 수입기** (별도 바이너리 `unim-import-nalgaeset`): XML로 내보낸 `.ist`만 읽어 UNIM v2 프로필로 변환합니다. 바이너리 `.ist`는 비지원입니다(연구 문서 §7).

### 7단계: 입력 방식·플랫폼 확장 (구상 — 미착수)

아래 6개는 아직 계획 단계도 아닌 **아이디어 백로그**입니다. 각 항목의 "선행"은 착수 전에
반드시 끝나 있어야 하는 것이고, "난점"은 그 항목이 실패한다면 십중팔구 여기서 실패한다는 지점입니다.
규모는 전부 주 단위 이상이며, 순서는 우선순위가 아닙니다.

- [ ] **① 한자 단어 단위 변환**: 현재는 `hanja_target` 이 한 음절(또는 현재 preedit)이라 `대한민국`을
  `大韓民國`으로 한 번에 바꿀 수 없습니다. **유리한 조건 — 사전 데이터는 이미 있습니다**:
  `src/data/hanja.txt`(libhangul 유래) 30만 3천 행 중 **다음절 항목이 27만 5천 개**이고
  `HanjaDictionary` 는 한글 문자열 키의 `HashMap` 이라 `entries.get("대한민국")` 이 그대로 동작합니다.
  단어 누적 버퍼(`InputContext::word_buffer`, `commit_unit = Word`)도 이미 있습니다.
  - 필요한 일: 변환 대상 결정(어절 최장일치 → 실패 시 음절 폴백), 이미 **커밋된** 단어를 뒤에서
    변환하는 경로(`surrounding_text` + `smart_backspace` 인프라 재사용, 한컴·MS IME 방식),
    후보 랭킹(현재는 사전 등장 순), 팝업의 다중 음절 표시(`POPUP_SPEC.md` 준수).
  - 난점: "어디까지가 한 단어인가" 판정입니다. 조사가 붙은 `대한민국은` 에서 `대한민국`만 잘라내려면
    최장일치 + 조사 목록이 필요하고, 잘못 자르면 사용자가 매번 범위를 고쳐야 해 오히려 느려집니다.

- [ ] **② macOS 입력기**: InputMethodKit(`IMKServer`/`IMKInputController`) 기반 네이티브 IME.
  DBus 없이 in-process 구조라 **Windows TSF 포팅과 같은 형태**입니다 — 코어는 그대로 두고 프런트엔드만 새로 만듭니다.
  - 선행: `unim-capi`(현재 `extern "C"` 67개, `unim.h` 드리프트 가드 있음)가 Swift/ObjC 에서
    필요한 표면을 다 덮는지 점검해야 합니다. 덮지 않으면 capi 확장이 1순위 작업입니다.
  - 난점: 코드 서명·공증(Apple Developer 계정 필수)과 `~/Library/Input Methods` 번들 배포 체계입니다.
    빌드도 macOS 머신이 있어야 합니다(현재 개발 환경엔 없습니다 — CI 러너로 우회 가능한지 확인이 필요합니다).
    후보창은 `IMKCandidates` 를 쓸지 자체 `NSPanel` 을 그릴지 결정해야 합니다(POPUP_SPEC 대응).

- [ ] **③ Android · iOS 입력기**: 데스크톱 포팅과 **성격이 다릅니다** — 화면 자판 UI 자체를 우리가 그려야 하고,
  그 자판이 곧 제품의 얼굴이 됩니다. ⑥ 한손 자판과 사실상 한 몸입니다.
  - Android: `InputMethodService` (Kotlin) + JNI ↔ Rust 코어(`cargo-ndk`).
  - iOS: Keyboard Extension(`UIInputViewController`) + Rust staticlib. **앱 확장 메모리 한도**(수십 MB)가
    실질 제약이라 ⑤ 예측 모델과 동시 적재는 어려울 수 있습니다. "전체 접근 허용" 없이 동작하는 범위 설계가 필요합니다.
  - 난점: 두 플랫폼 모두 스토어 심사·서명 체계가 필요하고, 데스크톱과 공유 불가능한 UI 코드가
    대량으로 생깁니다. 유지보수 대상이 사실상 두 개 늘어납니다.

- [ ] **④ 음성 입력**: 접근성 관점에서 이 목록 중 사용자 체감이 가장 클 수 있는 항목입니다.
  - 설계 결정 3가지: (a) **온디바이스 전용**(whisper.cpp 계열)인가 클라우드 허용인가 —
    UNIM 의 프라이버시 원칙상 온디바이스 기본이 자연스럽습니다, (b) 결과를 preedit 으로 넣어 수정 가능하게
    할지 바로 커밋할지, (c) 구두점·편집 명령("지우기", "쉼표")을 받는 명령 모드를 둘지.
  - 선행: 스트리밍 부분 결과를 IME 프로토콜에 태우는 방식 정리(각 프런트엔드의 preedit 갱신 빈도 제약),
    트리거 방식(핫키 vs 토글 vs 상시 대기).
  - 난점: 한국어 모델의 정확도·지연·모델 크기 삼각 절충입니다. 그리고 상시 대기 시 마이크 점유 정책.

- [ ] **⑤ 단어·문장 예측 후보창 (경량 LLM)**: 다음 단어/문장 후보를 후보창에 제시합니다.
  - **선행(필수) — 비밀번호 필드 억제**: `content_purpose` 게이트(4단계 완료)가 예측 경로에도
    적용되어야 합니다. 예측은 입력 문맥을 모델에 넣는 기능이라, 이 게이트 없이는 켜서는 안 됩니다.
    민감 필드에서는 예측·학습 양쪽 모두 정지가 기본값입니다.
  - 기존 자산: `surrounding_text`(문맥), `typefix_userdict`·`typefix_blacklist`(로컬 학습 선례와 저장 규약),
    팝업 서비스(후보창 렌더).
  - 난점: **지연 예산**입니다. 타이핑 흐름을 끊지 않으려면 키 입력당 수십 ms 안에 후보가 나와야 하는데,
    이는 모델 크기의 상한을 정해 버립니다. n-gram/사전 기반 1차 후보 + 소형 LM 재순위 같은 하이브리드가
    현실적일 가능성이 큽니다. 메모리·배터리(모바일에서는 ③과 충돌), 학습 데이터의 로컬 보관 원칙도 함께 정해야 합니다.

- [ ] **⑥ 한손 입력 자판 (천지인 · 나랏글 등)**: 모바일(③)의 전제이자, 데스크톱에서도 한손 사용자에게 의미 있는 항목입니다.
  - **선행(필수) — 자판 프로필 v2**(6단계). 현재 v1 스키마는 정적 키→자모 매핑이라 이 자판들의 핵심 동작을
    표현할 수 없습니다: 천지인의 **같은 키 반복 타 순환**(ㄱ→ㅋ→ㄲ)과 `ㅣ·ㅡ` 조합 모음, 나랏글의
    **획추가·쌍자음 변형 키**(현재 조합 중인 낱자에 연산을 가하는 방식). 둘 다 6단계의
    provenance 태깅 + 컴포저 상태 접근 predicate 가 있어야 자연스럽게 표현됩니다.
  - 추가로 필요한 것: **multi-tap 타이머**(같은 키 재타 vs 다음 글자 시작을 시간으로 가르는 확정 규칙).
    기존 `chord_window_ms`(모아치기)와 개념이 비슷하나 의미가 반대라 별도 설계가 필요합니다.
  - 난점: 타이머 기반 확정은 프런트엔드마다 키 이벤트 타이밍 보장이 달라(특히 XIM·원격 세션)
    동작이 갈릴 수 있습니다. 타이머 없는 확정 방식(다음 키가 오면 즉시 확정)과의 선택지 비교가 선행되어야 합니다.

> **의존 관계 요약**: ⑥ ← 6단계(프로필 v2) / ③ ← ⑥ / ⑤ ← content_purpose 게이트(완료) /
> ②·③ ← `unim-capi` 표면 점검. ①은 다른 항목에 의존하지 않아 **단독 착수가 가능한 유일한 항목**이고,
> 사전 데이터가 이미 있어 이 목록에서 투입 대비 효과가 가장 좋습니다.
