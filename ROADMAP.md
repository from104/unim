# UNIM 프로젝트 로드맵

이 문서는 **UNIM** 프로젝트의 장기 목표와 개발 단계별 계획을 설명합니다.

## 🎯 핵심 목표

언어 상태 자동 감지 및 수동 텍스트 변환 기능을 갖춘, 하나로 통합된 크로스 플랫폼(Windows, macOS, Linux) 한국어 입력기 엔진(IME)을 구축하는 것입니다.

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
- [x] **한자/특수문자 입력**: XIM 자체 Xft 팝업 + GNOME Extension 자체 팝업 + GTK/Qt/Wayland는 `unim-gui` 중앙 팝업으로 통합.
- [x] **설정 도구**: GTK/Qt GUI 설정 도구 (`unim-gtk-settings`, `unim-qt-settings`) + CLI (`unim-cli config`).
- [x] **시스템 트레이**: `unim-gui` 트레이 아이콘 및 팝업 통합.

### 3단계: 문서화 및 안정화 (진행 중)

- [x] **컴포넌트별 SPEC.md 작성**: 12개 컴포넌트 기능 명세 문서화.
  - `src/`, `unim-capi/`, `unim-cli/`, `unim-daemon/`, `unim-dbus/`
  - `unim-frontends/gtk3/`, `gtk4/`, `qt5/`, `qt6/`, `xim/`, `wayland/`
- [x] **XIM 프로토콜 적합성 검증**: [XIM 사양](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html) 대비 3회 교차 검증 (11개 항목 적합).
- [x] **Wayland 프로토콜 참조 문서화**: `input-method-v2`, `virtual-keyboard-v1` 프로토콜 사양 참조 및 아키텍처 문서화.
- [x] **Wayland 키 반복(Key Repeat)**: `mio` + `timerfd` 기반 구현 완료 (`repeat.rs`).
- [x] **Wayland 한자/특수문자 팝업**: `zwp_input_popup_surface_v2` 기반 구현 완료 (`popup_surface.rs`, `popup_renderer.rs`).
- [ ] **Surrounding Text / Content Type**: Wayland 프로토콜 이벤트 활용 (Phase 4).
- [ ] **Debian 패키지 안정화**: 패키지 빌드/설치 프로세스 검증 및 개선.

### 3.5단계: UI 프런트엔드 분리 (Fcitx5 스타일)

엔진(daemon)과 UI(팝업/인디케이터/설정)를 DBus 시그널 기반으로 완전 분리, 툴킷별 네이티브 GUI 지원.

- [x] **unim-gui 모듈 분리**: DBus, 트레이, UI 모듈 분리 완료.
- [x] **unim-gui-common 크레이트**: DBus 통신 + 트레이 등 공통 로직 추출 완료.
- [x] **unim-gui-gtk 전환**: `unim-gui-common` 의존으로 전환 완료.
- [x] **unim-gui-qt 신규 구현**: cxx-qt 기반 Qt6 네이티브 GUI 구현 완료.
- [x] **Debian 패키지 재구성**: 9개 바이너리 패키지로 분할 (`unim-common` / `unim-im-gtk` / `unim-im-qt` / `unim-xim` / `unim-wayland` / `unim-gui-gtk` / `unim-gui-qt` / `unim-gnome` / `unim` 메타). GUI 두 개 공존 허용(Conflicts 불필요), `unim-gnome`은 `unim-gui-gtk`를 Depends로 강제. `apt install unim` 한 줄로 full stack 설치.

### 3.7단계: 자판 프로필 v1 (완료)

자판 정의를 하드코딩 Rust const에서 **자기 완결 v1 JSON**으로 이관. 사용자 자판(`~/.config/unim/layouts/*.json`) + 상속(`inherits`) + 선택형 규칙 세트(rule_sets) 지원. Phase 6단계 엔진 재설계(v2)의 데이터 기반을 마련.

- [x] **v1 스키마 정의** (`docs/plans/LAYOUT_PROFILE_V1.md`): schema_version, metadata(다국어), inherits, combinations(자기 완결), rule_sets, active_rule_sets. v0 하위 호환 자동 승격.
- [x] **Phase 1·2 — 로더·빌더·Composer 통합**: `src/keystroke/profile/` 하위에 schema/loader/builder/localized 신설. `HangulComposer{2,3}Bul::new_with_profile` + v0→v1 동일 결과 regression.
- [x] **Phase 3 — 레지스트리·상속·핫리로드**: `ProfileRegistry`(내장 + `~/.config/unim/layouts` 통합 네임스페이스, 사용자 우선), `inherit::resolve`(재귀 해석 + 순환 탐지 + layer-merge), 디렉토리 mtime 기반 자동 재스캔.
- [x] **Phase 4 — Config·CLI·DBus·엔진 연결**: `korean.custom_layout`(`Option<String>`)·`korean.active_rule_sets`(`Vec<String>`) 필드 5-point 싱크. `unim-config layout list/describe/validate` 서브커맨드. `InputEngine::new`가 ProfileRegistry를 거쳐 효과적 프로필을 로드, 실패 시 enum 경로 폴백.
- [x] **Phase 5 — GTK GUI**: settings_dialog의 한국어 자판 ComboRow가 모든 한국어 프로필(내장 + 사용자) 표시. 선택 시 규칙 세트 SwitchRow가 동적 재구성.
- [x] **Phase 6 — 내장 10종 v1 이관**: `docs/plans/new_keymaps/*.json` 9종 + 신규 `ko_3bul_qwerty` 1종을 `src/keystroke/keymap/`로 이관. 기존 Rust const와 동일 `CombinedJamoMap` 산출 (behavior-preserving, regression test로 고정).
- [x] **Phase 7 — 문서·마이그레이션 공지**: 본 섹션 + CHANGELOG Added 블록 + README 간단 안내.

### 4단계: 자동 상태 전환 (지능화)

- [ ] **문맥 감지**: 현재 입력 필드 상태나 언어 문맥을 감지하는 방법 연구.
- [x] **자동 교정 엔진 (AutoTypeFix)**: 실시간 오타 감지 구현. forward(영→한: `gksrmf` → `한글`), reverse(한→영: `ㅈㅐㅍㅁ` → `wave`) 양방향 지원. XIM·GTK3/4·Qt5/6·Wayland·GNOME Shell 전 프론트엔드 통합. (`src/auto_typefix.rs`)
- [x] **사용자 학습 — 억제 사전(Blacklist)**: 롤백 관측(BS + 모드 전환) + 재시도 시점 자동 등록 방식으로 "원치 않는 교정" 단어를 Tentative로 학습. GUI에서 Confirm 시 Confirmed, 시간 만료 시 Inactive. `~/.config/unim/typefix-blacklist.yaml`에 저장, 데몬 mtime 핫리로드. (`src/typefix_blacklist.rs`)
- [ ] **사용자 학습 — 양성 사전**: 사용자별 타이핑 패턴 기반의 *긍정적* 로컬 사전(오타 교정 promotion)은 미구현. 현재 Blacklist는 교정 제외만 담당.

### 5단계: 크로스 플랫폼 확장

- [ ] **입력 컨텍스트 통합**: 단순 "변환 도구"에서 완전한 입력기(IME) 서비스로 진화 (리눅스용 `ibus`, `fcitx5` 연동).
- [ ] **크로스 플랫폼 지원**: Windows(TSF) 및 macOS용 네이티브 백그라운드 서비스 및 연동 방안 조사.

### 6단계: 엔진 재설계 (고급 한글 입력 기법 지원)

현재 UNIM의 한글 엔진은 **정적 키맵 + 하드코딩 오토마타** 구조라 아래 기능들을 표현할 수 없다. 날개셋 한글 입력기 조사(`docs/research/NALGAESET_KEYBOARD_FORMAT.md`)와 순아래받침 규칙 조사(`docs/research/순아래받침_규칙.md`)에서 드러난 공통 결론: **낱자에 "어디서 왔는지" 정보가 붙어야 하고, 키 해석이 컴포저 상태에 접근할 수 있어야 한다**. 이 두 전제를 도입하는 엔진 리팩터가 아래 모든 항목의 선행 조건이다.

- [ ] **낱자 provenance 태깅**: `Jamo` 표현을 `(kind, source_key)` 튜플로 확장해 같은 ㅗ/ㅜ라도 어느 키에서 왔는지 구별. 세벌식 390의 `9`-ㅜ, `/`-ㅗ 이중모음 전용 역할, 복벌식 자동 판정의 근거가 되는 날개셋문자 64-bit 토큰 개념(연구 문서 §4.1)에 대응.
- [ ] **문맥 의존 키 해석 (글쇠 수식 최소 집합)**: 키→자모 매핑이 컴포저 상태(`has_cho`/`has_jung`/`has_jong`/`syllable_empty`)를 조회할 수 있도록 predicate 엔진 도입. 두벌식 `/`, 세벌식 390 `/` 같은 적응형 글쇠(연구 문서 §4.2) 지원. 날개셋의 Turing-complete 수식 전면 이식은 별도.
- [ ] **자판 프로필 v2**: v1(`docs/plans/LAYOUT_PROFILE_V1.md`)에서 유보한 provenance + predicate 필드를 스키마에 추가. 세벌식 390 원본 규약을 있는 그대로 재현.
- [ ] **모아치기 (stroke replay)**: 낱자 입력 순서가 바뀌어도 재배열해 한 음절로 조합. 안마태 자판 등 순서 자유 자판 지원.
- [ ] **복벌식**: 어절 첫 타자의 손 위치(좌/우)로 두벌식·세벌식 자동 전환(연구 문서 §5.2). 어절 단위 버퍼 + 첫 낱자 provenance가 전제.
- [ ] **옛한글**: U+1100 확장 블록 낱자, 방점, 합용병서 지원. Jamo enum 확장 + 고급 문자 생성기.
- [ ] **초·종성 공유 결합 규칙 (shared combination)**: 날개셋 §5.1의 "종성이 초성 결합 규칙을 중첩 적용" 동작을 엔진 레벨에서 직접 표현. v1의 `share_cho_jong` 플래그는 복제 수준에 그침.
- [ ] **날개셋 `.ist` 수입기** (별도 바이너리 `unim-import-nalgaeset`): XML로 내보낸 `.ist`만 읽어 UNIM v2 프로필로 변환. 바이너리 `.ist`는 비지원(연구 문서 §7).
