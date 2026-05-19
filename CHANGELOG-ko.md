# Changelog

<!-- markdownlint-disable MD024 -->

UNIM(Universal Next-generation Input Method) 프로젝트에 대한 모든 주목할만한 변경 사항은 이 파일에 기록됩니다.

형식은 [Keep a Changelog (korean)]를 기반으로 하며 이 프로젝트는 [Semantic Versioning (korean)]을 따릅니다.

## [Unreleased]

_아직 변경 사항 없음._

---

## [0.3.0] 2026-05-19

### 호환성 깨짐

- **DBus 시그널 `HanjaCandidatesReordered` 페이로드가 10-tuple로 변경** (기존 9-tuple). 토글 직전 즐겨찾기 상태를 담은 `was_bookmarked: bool` 필드가 끝에 추가됐다 — 프런트엔드는 `was_bookmarked && !bookmarked`일 때만 cursor flash를 띄우는 식으로 분기. `org.atit.unim.InputContext`의 외부 구독자는 unpacking 코드를 함께 갱신해야 함 (9-tuple은 더 이상 수용하지 않음).
- **자판 프로필 v0 스키마 미지원 전환.** v1 마커(`schema_version`, `metadata`, `inherits`, `combinations`, `rule_sets`, `active_rule_sets`) 중 하나도 없는 0.1.x 시기의 v0 JSON은 이제 로더가 `LoadError::UnsupportedSchema`로 거부하고 콘솔에 경고를 출력한다. `~/.config/unim/layouts/*.json`의 사용자 v0 프로필은 `"schema_version": 1`을 추가하고 `combinations` 블록을 명시적으로 채워 v1으로 변환할 것([`docs/dev/plans/LAYOUT_PROFILE_V1.md`](docs/dev/plans/LAYOUT_PROFILE_V1.md) §3 참고). 빌트인 프로필은 0.2.0 시점에 모두 v1로 이관 완료된 상태.

### 제거됨

- **`unim-gui-qt` 패키지 제거**: Qt6/cxx-qt 대체 GUI 패키지(`unim-gui-qt`)가 완전히 제거됐다. KDE Plasma 사용자는 `unim-settings`(= `unim-gui-gtk`)와 `unim-popup-service`로 전환한다. 기존 설정 파일(`~/.config/unim/config.yaml`)과 자판 레이아웃(`~/.config/unim/layouts/`)은 그대로 보존된다.
- **Rust 상수 자모 조합 테이블** (`JUNG_COMBINATIONS`, `JONG_COMBINATIONS`, `CHO_COMBINATIONS`, Lazy static `COMBINED_JAMO_2BUL`/`_3BUL`)을 `src/hangul/composer_with_{2,3}bul.rs`에서 삭제. `HangulComposer{2,3}Bul::new()`는 이제 `new_with_profile(load_builtin_profile("ko_2bulstd"|"ko_3bul390"))`로 위임. 자모 조합 규칙의 단일 source of truth는 v1 빌트인 프로필 JSON.
- **`SchemaKind` enum + `detect()`** 삭제(`src/keystroke/profile/schema.rs`). v0/v1 판별 역할은 `RawProfile::has_v1_markers()`로 단순화. 빌더의 `fallback_for(layout_type)` v0 호환 경로도 함께 삭제.
- **`HangulComposer3BulMoachigi` 별도 컴포저 제거**: 모아치기 로직이 `InputEngine`의 `chord_buffer` 레이어를 통해 `HangulComposer3Bul` 내부로 통합됨. 사용자 체감 변화 없음.
- **`emoji_popup.enabled` 설정 필드 전체 5지점에서 제거**: 한자 키 idle 트리거가 이제 항상 켜진다 — 조합 중에는 한자 변환, idle 상태에서는 이모지 팝업이 단일 진입점.
- **쿼티형 세벌식(`ko_3bul_qwerty`) 빌트인에서 제거**: v3 모아치기 schema JSON 본문은 `docs/references/keymaps/ko_3bul_qwerty_v2.json`에 연구 자료로 보존. `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사하면 사용자 프로필로 계속 사용 가능.

### 추가됨

- **팝업 단일 SoT 아키텍처 — `unim-popup-service`**: 한자·특수문자·이모지 팝업의 렌더링 책임이 daemon에서 신규 사이드카 프로세스 `unim-popup-service`로 이관됐다. daemon의 `org.atit.unim.InputContext` 시그널 8종을 `org.atit.unim.Popup` 인터페이스로 forward하여 모든 환경에서 단일 view-model(PopupRender)을 공유한다. D-Bus auto-activation(`org.atit.unim.PopupService.service`) 방식으로 기동 — autostart .desktop 폐기.
- **GNOME Shell extension `popup_view.js` 통합**: GNOME Wayland 환경에서 Mutter가 wlr-layer-shell 미지원이므로 extension이 St 위젯(`PopupView`)으로 한자·특수문자·이모지 팝업을 직접 렌더한다. popup-service와 동일한 CSS 토큰·클래스명 공유. `Meta.is_wayland_compositor()`가 true일 때만 활성화 — X11에서는 popup-service GTK4 popup 사용(이중 렌더 방지).
- **외부 좌클릭 dismiss**: 팝업 바깥 좌클릭 시 팝업이 닫히며, 클릭 이벤트는 아래 창에 그대로 전달된다. 팝업 외부 클릭은 정상 동작이다.
- **안마태 2003(안마태) 자판 빌트인** (`ko_3bul_anmatae`): UNIM 최초의 모아치기(chord 기반) 한글 자판. 고정 초·중·종성 영역을 갖는 세벌식 자판. 초성 9개·중성 15개·종성 20개 결합 규칙 포함.
- **Qwerty형 세벌식 v2 빌트인** (`ko_3bul_qwerty`, v2): 0.2.0 빌트인 제거 후 모아치기 v3 schema로 재도입. Shift 없는 26자리 알파벳 포화(10 초성 / 6 중성 / 10 종성) 레이아웃.
- **자판 프로필 v3 schema**: `supports_moachigi: bool` 최상위 능력 마커 추가. GTK 설정 다이얼로그는 이 플래그가 true일 때만 모아치기 그룹을 노출.
- **모아치기 사용자 설정**: `~/.config/unim/config.yaml`의 `korean.*` 하위에 두 개의 opt-in 설정이 추가됨:
  - `korean.bidirectional_combine: Option<bool>` — true이면 초·중·종성 결합을 `(a,b)`·`(b,a)` 양방향으로 시도. 기본 unset → OFF.
  - `korean.chord_window_ms: Option<u16>` — 단일 chord 윈도우 지속 시간(ms). 0 또는 unset = chord 비활성. GUI 슬라이더 범위 10~200ms. 기본 unset → OFF.
- **모아치기 chord 엔진** (`src/input_engine/chord_buffer.rs`): 단일 윈도우 chord 누산기. 자모 1개 → 일반 처리; 2개 이상 → 영역 분류 + permutation 탐색.
- **모아치기 v4 — Atomic Window Principle**: 윈도우 만료 시점에 모든 분기 결정. 중간 commit 아티팩트 제거.
- **`chord_compose` 모듈** (`src/input_engine/chord_compose.rs`): 영역별 permutation 탐색 (cho ≤ 2키, jung/jong ≤ 3키, 실패 시 호환자모 fallback).
- **`chord_window_ms` 범위 및 기본값 갱신**: 범위 10~100ms → **10~200ms**, 기본값 50ms → **60ms**.
- **`KoreanConfig::validate_chord_window_ms`**: 신규 검증 함수. 0(chord 비활성) 또는 10~200ms만 허용.
- **Backspace 시 chord preedit 복원**: chord 도중·직후 Backspace가 `input_order` 역순으로 자모를 제거하고 나머지로 음절을 재합성.
- **GTK 설정 다이얼로그 — 모아치기 그룹**: "동시 입력 자모 역순 결합" 토글 + "동시 입력 시간(ms)" 슬라이더(10~200ms, 기본 60ms, tick 마크 10 / 50 / 100 / 150 / 200). `supports_moachigi=true`인 자판 선택 시만 표시.
- **모든 popup·모든 프런트엔드에 마우스 페이지 이동 ◀/▶ 버튼**: 한자·특수문자·이모지 popup 모두 footer에 ◀(이전) / ▶(다음) 버튼 추가. 키보드 `←`/`→` 및 `Page Up`/`Page Down`과 동일한 wrap-around 동작. `total_pages == 1`이면 버튼 숨김. 신규 DBus RPC: `popup_change_page(direction: i32)`.
- **한자 즐겨찾기 해제 cursor flash** (Catppuccin yellow `#f9e2af`, 140ms): ☆로 해제 시 popup 재정렬 + cursor가 사전순 원위치로 점프. 도착 셀이 깜박여 자동 페이지 이동을 인지하게 한다.
- **Wayland popup pointer 입력 인프라** (`unim-frontends/wayland`): `WlPointer` 이벤트 핸들링으로 popup ◀/▶ 클릭 수신.
- **i18n 키 추가**: `popup_previous_page`, `popup_next_page`를 ko/en (yml·po 4지점)에 동기 추가.
- **BUILTIN_NAMES × 4축 정합성 테스트**: 10종 빌트인 전수에 대해 fallback 미발생·v1 schema·combinations 보유를 단일 테스트로 검증.
- **AutoTypeFix 학습 blacklist 강화**: retrigger 시점에 tentative 억제 항목 등록 + 즉시 억제.
- **사용자 가이드 — 키보드 호환성 섹션**: `docs/user/keymaps/anmatae.md`·`anmatae.en.md`에 NKRO 권장 섹션 추가.
- **트러블슈팅 §15 — 모아치기 섹션**: 5가지 주요 원인(윈도우 너무 짧음, NKRO 미지원, USB 폴링 레이트 낮음, bidirectional_combine 비활성, 자판 미지원) 진단·해결 수록.
- **RPM 패키징** (`rpm/unim.spec`): Fedora/RHEL/openSUSE 계열 패키지 지원 추가.

### 변경됨

- **설정 다이얼로그 라이브 도움말 보강**: 26개 tooltip·15개 subtitle 재작성, 5개 i18n 키 신규 추가. four-element 템플릿(무엇/언제/왜/권장값) 적용.
- GTK 설정 다이얼로그의 `chord_window_ms` 슬라이더: 범위 10~100ms → **10~200ms**, 기본값 50ms → 60ms.
- `bidirectional_combine` tooltip: chord 윈도우와 독립 동작하며 순차 입력에도 적용됨을 명시.

### 고침 (best-effort)

- **XIM `commit_then_preedit` 가 `commit()` 직전 `clear_preedit()` 강제** (`unim-frontends/xim/src/handler.rs`): OVER-THE-SPOT 경로(XTerm·WezTerm)에서 commit 직후 preedit 가시화 정상 복귀. XIM 한정 잔존 회귀 회피책 적용 완료.

### 알려진 이슈

- **KDE Plasma 5.x Wayland popup 미지원**: Ubuntu 24.04 (noble) 표준 저장소에 `gtk4-layer-shell` 패키지가 없어 한자/특수문자/이모지 팝업이 표시되지 않습니다. X11 세션 사용 또는 GNOME으로 우회 권장.
- **KDE Plasma 6 Wayland / Sway / Hyprland / river — 실험적, 검증 미흡**: 시스템에 `libgtk4-layer-shell` 가 설치된 상태에서 `wayland-backend` cargo feature 를 켜고 빌드하면 **이론상** 동작하지만, **0.3.0 QA 사이클에서 충분히 테스트되지 않았습니다.** popup 위치 정렬, IME 포커스 전환, layer-shell 좌표 변환 등에서 미세 회귀가 있을 수 있습니다. 문제 발견 시 [GitHub Issues](https://github.com/from104/unim/issues) 로 제보 부탁드립니다.

---

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
