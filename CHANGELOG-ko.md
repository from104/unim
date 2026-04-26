# Changelog

UNIM(Universal Next-generation Input Method) 프로젝트에 대한 모든 주목할만한 변경 사항은 이 파일에 기록됩니다.

형식은 [Keep a Changelog (korean)]를 기반으로 하며 이 프로젝트는 [Semantic Versioning (korean)]을 따릅니다.

## [Unreleased]

## [0.2.0] 2026-04-26

### 추가됨

- **자판 프로필 v1 (사양 + 엔진 + 설정 + CLI + GUI)**: 빌트인 자판이 자체완결형 v1 JSON 프로필(`src/keystroke/keymap/*.json`)로 통일되어, 기존 Rust 상수 + 부분 JSON 혼합 경로를 대체.
  - **사용자 프로필**: `~/.config/unim/layouts/*.json`에 v1 JSON을 두면 데몬이 시작 시 스캔하고 mtime 기반으로 핫리로드.
  - **inherits 체인 해석**: 자식 프로필이 `"inherits": "base_name"`을 선언하면 `ProfileRegistry`가 사이클 감지 + 메타데이터/레이아웃/룰셋 레이어 머지로 체인 해석.
  - **룰셋(rule sets)**: 프로필별로 명명된 옵션 서브룰(`rule_sets.<name>`) 선언 가능 — 예: `ko_3bul390`의 `sun_arae_batchim` — GUI SwitchRow 또는 CLI `set korean-active-rule-sets`로 토글.
  - **설정 필드 추가** (가산적, 미설정 시 영향 없음): `korean.custom_layout: Option<String>`, `korean.active_rule_sets: Vec<String>`. 5지점 동기화(config.rs ↔ `unim-cli config` ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog) 적용.
  - **`unim-cli config layout` 서브커맨드**: `list` / `describe <name>` / `validate <file.json>` (종료 코드 0=통과, 1=경고, 2=오류).
  - **GUI — Adw.ComboRow + 동적 SwitchRow**: 설정 다이얼로그가 모든 한글 프로필(빌트인 10개 + 사용자)을 표시하고, 선택한 프로필의 룰셋을 즉시 토글 가능한 SwitchRow로 노출.
  - **빌트인 프로필 추가 — `ko_3bul_qwerty`** (쿼티형 세벌식): Shift 없는 26자리 알파벳 포화 레이아웃 (14 초성 / 15 중성 / 19 종성). 빌트인 9개 → 10개.
  - 사양: [`docs/dev/plans/LAYOUT_PROFILE_V1.md`](docs/dev/plans/LAYOUT_PROFILE_V1.md).
- **AutoTypeFix 롤백 학습 억제 사전**(`src/typefix_blacklist.rs`, `~/.config/unim/typefix-blacklist.yaml`): 마지막 교정 위에서 일어나는 자연스러운 롤백 패턴(백스페이스 + 입력 모드 전환)을 관찰. 동일 ASCII로 두 번째 AutoTypeFix 시도(retrigger)가 발생하면 한 번에 tentative 억제 항목 등록 + 해당 시도 억제. GUI "확정" 버튼으로 Tentative → Confirmed 수동 승격, `tentative_expiry_hours`(기본 1시간, 1..=12) 후 Inactive로 자동 만료. 데몬이 mtime 변경을 감지해 자동 리로드.
- **AutoTypeFix 신규 설정 3종**: `auto_typefix.*` 하위에 `rollback_detection`(bool, 기본 true), `tentative_expiry_hours`(u16, 기본 1, 1..=12), `observation_timeout_secs`(u8, 기본 10, 5..=15). 3지점 동기화 적용.
- **설정 GUI "억제 단어" 페이지**(`unim-gui-gtk`): 신규 `Adw.PreferencesPage`, 세 그룹(Tentative / Confirmed / Inactive) 구성, 각 행에 확정 / 비활성화 / 제거 / 재활성화 액션.
- **한자 팝업 9×9 확장 격자 모드**: Period 키로 compact(9) ↔ expanded(81) 모드 전환. GTK Standalone, GTK IM, Qt IM, XIM 프런트엔드 모두에 GNOME 익스텐션과 동일하게 적용. ⊞/⊟ 아이콘으로 현재 모드 표시.
- **한자 즐겨찾기 UI** (☆/★): 포커스된 후보에 Space로 즐겨찾기 토글. `HanjaBookmarkChanged` DBus 시그널로 모든 열린 팝업(GTK / Qt / XIM / Wayland / GNOME) 실시간 갱신.
- **역방향 AutoTypeFix 사용자 사전**: 단축키로 선택 영역을 영문 측 사전 항목으로 등록(`RegisterUserDictFromSelection` DBus 메서드). 추가 / 제거 / 갱신 GUI 페이지 제공.
- **트리거 키 자동 영어 모드 전환**: 트리거 키 목록 설정(예: `:`, `/`)으로 한글 → 영어 모드를 경계 문자에서 자동 전환. 기본값은 빈 목록(역호환).
- **이모지 팝업 (Super+.)**: 카테고리 탭, 검색, MRU 즐겨찾기 지원. GTK Standalone(`unim-gui-gtk/src/emoji_popup.rs`) + GNOME Shell 익스텐션(`unim-gnome-extension/emoji_popup.js`) 구현.

### 변경됨

- **`KoreanLayout` enum 제거 (Phase 8)**: 한글 자판 필드가 평문 프로필 이름 문자열로 변경 (`KoreanLayout`은 공개 `String` 타입 별칭). `korean.layout`은 빌트인(`ko_2bulstd`, `ko_3bul390`, `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_qwerty`) 또는 사용자 프로필 이름 모두 허용. 기존 `custom_layout: Option<String>` 필드는 `layout`으로 통합. 기존 `config.yaml`의 `layout: Dubeolsik`과 `typefix-blacklist.yaml` 항목은 serde compat 레이어로 자동 정규화. C API setter/getter는 C 문자열을 받고/반환.
- **`EnglishLayout` enum 제거 (Phase 9)**: 한글 변경과 대칭. `english.layout`은 String이 되고 빌트인은 `qwerty` / `dvorak` / `colemak` / `colemak_dh` / `workman`. 기존 YAML 값은 serde `from = "EnglishConfigCompat"`로 자동 정규화. C API: `UnimEnglishLayout` enum 삭제, setter/getter는 C 문자열.
- **AutoTypeFix 역방향 롤백 게이트 BS-AND-switch → BS-OR-switch 완화**: 역방향 교정은 `clear_preedit=true`로 동작해 IM 모듈이 롤백 BS를 로컬 소비하므로 `engine_worker`로 절대 전달되지 않음 → AND 게이트는 구조적으로 도달 불가. 역방향은 모드 전환 관찰만으로 롤백 증거로 충분. 순방향은 BS-AND-switch 유지.
- **AutoTypeFix 역방향 억제 키 버그 수정**: `RecentCorrection.ascii`가 역방향에서는 `fix.corrected`(커밋된 영문 단어), 순방향에서는 `fix.original`(ASCII 런)을 저장. 이전엔 모든 역방향 항목이 `""`로 블랙리스팅되어 어떤 후속 쿼리와도 매칭되지 않았음.
- **AutoTypeFix 블랙리스트 등록 시점 이동 (rollback-moment → retrigger-moment)**: 기존 "롤백 시점 등록" 모델은 단발 모드 전환에서 false positive 다수 발생, 순방향 직관과도 일치하지 않음. 이제 BS / 모드 전환 관찰은 보류 교정 표시만 하고, retrigger 시점에 한 번에 tentative 등록 + 중복 시도 억제.
- **`unim-config` 고립 크레이트 제거**: 레거시 CLI 서브크레이트를 `unim-cli config` 서브커맨드로 통합 (설정 CLI Single Source of Truth).
- `unim-daemon`의 `GlobalModeChanged` 시그널 수신 시 `unim-gui` 트레이 아이콘과 팝업이 즉시 동기화되도록 리팩터링.

### 고쳐짐

- **IME — 영어 모드 Space가 직접 commit 경로**(`consumed=true`, `commit=" "`)로 커밋되도록 수정, 한글 모드와 동일. 이전엔 영어 모드 Space가 `not_consumed`를 반환해 GTK IM 모듈이 간헐적으로 공백을 누락(gedit에서 관찰).
- **IME — Focus-out 시 RPC 반환값 위에 추가로 발사되던 중복 `CommitText` DBus 시그널 제거**. 시그널은 컨텍스트 스코프가 아니라 같이 브로드캐스트하면 `늘` 같은 글자가 gedit에서 두 번 커밋되는 문제 발생. `FocusOut()` RPC 반환값이 focus-out 단일 commit 채널.
- **AutoTypeFix — `tentative_expiry_days`(1..=90) → `tentative_expiry_hours`(1..=12)로 변경**. 일 단위는 실용적 블랙리스트 큐레이션엔 너무 거칢. 기존 YAML의 옛 키는 제거 권장, 신규 기본값(1시간)이 자동 적용.
- **gedit / gnome-text-editor용 TypeFix surrounding-text 지원**: GTK IM 모듈이 `request_surrounding()`로 컨텍스트를 가져와, 기존에 커밋 텍스트를 노출하지 않던 앱에서도 역방향 교정 가능.
- **GTK preedit-end 키 잠금 버그**: GTK3/4 IM 모듈이 `unim_emit_preedit` 헬퍼로 `preedit-end`를 발사. preedit이 명시적 시그널 없이 끝날 때 발생하던 ghostty/터미널 키 잠금 해소.
- **XIM AutoTypeFix 재구현**: N+1 BS 프로토콜 모델로 전환, XIM 프런트엔드에서 다문자 교정이 정상 동작 (Chrome preedit edge case는 잔존).

## [0.1.0] 2026-04-21 — 첫 정식 릴리스

UNIM(Universal Next-generation Input Method)의 첫 번째 정식 릴리스. 한국어 입력기 엔진을 처음부터 Rust로 재설계한 결과물로, 다음 컴포넌트로 구성된다.

### 추가됨 — 엔진 코어

- **순수 Rust 한글 엔진 (`src/`)**: 2-bul / 3-bul 390 / 3-bul 391 한글 조합·분해 로직. UI/플랫폼 의존성 0.
- **DBus 데몬 아키텍처 (`unim-daemon` + `unim-dbus`)**: D-Bus 세션 활성화 기반 시스템 와이드 입력 상태 관리. 서비스명 `org.atit.unim.InputMethod`.
- **C-API 래퍼 (`unim-capi` / `libunim_capi`)**: Rust 코어를 C/C++ 프런트엔드에서 사용 가능하도록 노출.
- **통합 CLI (`unim-cli`)**: 한↔영 변환기 + `config` 서브커맨드 (show / set / path / reset / interactive).

### 추가됨 — 프런트엔드

- **GTK 입력기 모듈**: GTK3 (`unim-frontends/gtk3/`)와 GTK4 (`unim-frontends/gtk4/`) 모듈, 공용 컴포넌트 `unim-frontends/gtk-common/`.
- **Qt 플랫폼 입력 컨텍스트 플러그인**: Qt5 (`unim-frontends/qt5/`)와 Qt6 (`unim-frontends/qt6/`) `QPlatformInputContext` 구현, 공용 `unim-frontends/qt-common/`.
- **XIM 프런트엔드 (`unim-frontends/xim/`)**: 네이티브 Rust 기반 X11 XIM 프로토콜 구현, Over-The-Spot Preedit 지원, X11R7.6 XIM 명세 11개 적합성 항목 검증.
- **Wayland 프런트엔드 (`unim-frontends/wayland/`)**: `input-method-v2` + `virtual-keyboard-v1` 프로토콜 지원, KDE Plasma 환경 기초 지원, `zwp_input_popup_surface_v2` 한자/특수문자 팝업 통합.
- **GNOME Shell 익스텐션 (`unim-gnome-extension/`)**: 네이티브 통합 JS 익스텐션. 자판 변환 단축키(`gksrmf` ↔ `한국어`), 터미널 인식 paste 모드 등.

### 추가됨 — GUI

- **GTK4 / libadwaita 설정 다이얼로그 (`unim-gui-gtk`)**: 트레이 아이콘, 한자/특수문자 팝업, 설정 다이얼로그.
- **Qt6 / cxx-qt 대체 GUI (`unim-gui-qt`)**: GTK 대체 옵션. `unim-gui-gtk`와 충돌 없이 공존.
- **im-config 통합**: 시스템 IM 선택 도구와 자동 연동.

### 추가됨 — 기능

- **한글 자판**: 2-bul(두벌식 표준) + 3-bul(세벌식 390 / 391 / no-shift) 빌트인.
- **AutoTypeFix (TypeFix)**: 자동 한↔영 오타 교정 (순방향: 영문 입력 → 한글, 역방향: 한글 입력 → 영문). XIM / GTK / Qt / GNOME 지원.
- **한자 변환**: 한자 변환 팝업, 검색·페이지네이션·인덱스 키 네비게이션.
- **특수문자 / 이모지 검색**: 특수문자/이모지 검색 팝업.
- **앱별 입력 모드 규칙**: 앱별 입력 모드 자동 전환 규칙.

### 추가됨 — 패키징 및 문서

- **Debian 패키징 — 9개 바이너리 패키지**(`debian/control`):
  - `unim-common` (코어 + 데몬 + CLI + libunim_capi)
  - `unim-im-gtk` (GTK3/4 IM 모듈)
  - `unim-im-qt` (Qt5/6 플러그인)
  - `unim-xim` (X11 XIM 프런트엔드)
  - `unim-wayland` (Wayland 입력기 프런트엔드)
  - `unim-gui-gtk` (GTK4 / libadwaita 설정 GUI + 트레이)
  - `unim-gui-qt` (Qt6 / cxx-qt 설정 GUI + 트레이, 대체)
  - `unim-gnome` (GNOME Shell 익스텐션, `unim-gui-gtk` 의존)
  - `unim` (메타패키지 — 전체 스택)
- **종합 문서화**: 컴포넌트별 12개 `SPEC.md`, `IME_BEHAVIOR.md`(프런트엔드 동작 일관성), `POPUP_SPEC.md`(통합 팝업 디자인).

[Keep a Changelog (korean)]: https://keepachangelog.com/ko/1.0.0/
[Semantic Versioning (korean)]: https://semver.org/lang/ko/
