# 문서·규칙·테스트 관례

조사 대상: UNIM(`/home/from104/work/unim`, branch `develop`) — "한자 단어 입력" 기능을 위한
문서/규칙/테스트 관례 사전 조사. **읽기 전용**, 모든 주장에 file:line 근거.

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `AGENTS.md` (루트) | **redirect stub** — 실제 내용은 아래로 이동 (`AGENTS.md:1-6`) |
| `docs/dev/architecture/AGENTS.md` (277줄) | 프로젝트 컨텍스트: 컴포넌트 맵, 아키텍처 흐름, 빌드 시스템, Zero-Tolerance 품질 규칙, 메모리 관리 규칙, 개발 규약, **CHANGELOG 작성 규칙(§170-197)**, 핵심 파일 목록, 에이전트/스킬 정의 |
| `docs/dev/architecture/GEMINI.md` (262줄) | 개발 컨벤션, **설정 5지점 동기화 가이드**(§31-94), 로깅 시스템(§96-191), `CLAUDE.md` 행동 규범 임베드(§193-261) |
| `docs/dev/specs/POPUP_SPEC.md` (786줄) | 한자/특수문자/이모지 팝업 **단일 원본(SoT)** — 아키텍처, DBus 프로토콜, 레이아웃/상수/상태머신/키바인딩, 엔진 `PopupAction`, `PopupViewModel`, lifecycle |
| `CONTRIBUTING.md` (131줄) | 기여자 가이드 — Zero Tolerance, 아키텍처 규칙, **6지점 sync 체크리스트(§87-97, 팝업 관련 변경 시)**, 브랜치/PR 워크플로, 문서화 규칙, 6매니저 하네스 |
| `ROADMAP.md` (175줄) | 장기 로드맵. **118-127행이 "① 한자 단어 단위 변환" 항목 원문** |
| `CHANGELOG.md` / `CHANGELOG-ko.md` | 영/한 릴리스 노트, 한국어가 정본(SoT) |
| `docs/user/user-guide/README-ko.md` | 사용자 매뉴얼(한글), §4.2(375행~)가 "한자 변환(Hanja)" 절 |
| `src/input_engine/tests_*.rs`, `test_helpers.rs` | 엔진 유닛 테스트 — 그룹별 파일 분리 |
| `src/auto_typefix/tests.rs` (1266줄) | AutoTypeFix 유닛 테스트 |
| `tests/harness/{harness.py,run.py,scenarios/}` | **L3 Xvfb 실기능 타이핑 시험** — XTEST로 진짜 키 입력, `field.render` JSONL로 판정 |
| `tests/unim-test-dbus/src/main.rs` | DBus 레벨 통합 테스트 — `test_hanja_popup`(166-221행) 존재 |
| `Makefile` | 빌드/테스트 타깃의 유일한 SoT (AGENTS.md:91, CONTRIBUTING.md 재확인) |
| `.github/workflows/{linux-ci,linux-deb,linux-rpm,windows-msi}.yml` | CI 워크플로 4종 |
| `unim-settings-gtk/src/settings_dialog.rs` | **실제** GTK 설정 UI (문서상 이름 `unim-gui-gtk`는 **없어진 크레이트명** — §5 참고) |

**존재하지 않음(확인 완료)**:
- `DESIGN.md` (루트) — 없음. `find . -iname "DESIGN.md"` 결과는 `unim-typing-practice/DESIGN.md`, `unim-imm32/DESIGN.md` 뿐이고 둘 다 한자와 무관.
- `docs/dev/specs/` 아래 POPUP_SPEC.md 외 다른 spec 문서 없음 (`ls docs/dev/specs/` → `POPUP_SPEC.md` 1개뿐).
- `plans/` 폴더 없음(`find . -maxdepth 2 -iname "*plan*"` 결과 0건, `docs/dev/architecture/dbus-popup-migration-plan.md` 1건만 예외 — 과거 마이그레이션 계획 문서가 `architecture/` 안에 있었던 선례).
- 한자 단어 기능 전용 시나리오 테스트 — `tests/harness/scenarios/`에는 `2bulstd.json`·`3bul390.json`·`common.json` 뿐, hanja 시나리오 없음(확인: `ls tests/harness/scenarios/`).
- clippy 타깃 — Makefile에 `clippy` 문자열 0건(확인: `grep -n clippy Makefile`). Zero-Tolerance는 `cargo build --workspace` 경고 0개 + `cargo test --workspace` 전부 통과 기준이며 clippy는 별도 게이트가 아님.

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### 팝업/한자 엔진 API (테스트에서 확인된 실제 시그니처)

- `src/input_engine/test_helpers.rs:9` — `pub(super) fn create_test_engine() -> InputEngine` (`Config::default()`로 생성)
- `src/input_engine/tests_scenarios.rs:277-303` (`test_scenario_hanja_conversion`) — 실사용 패턴:
  ```rust
  engine.set_input_category(InputCategory::Korean);
  engine.press_key(KeyCode::R, modifier, &config); // ㄱ
  engine.press_key(KeyCode::K, modifier, &config); // 가
  engine.preedit_str();                             // "가"
  let result = engine.start_hanja_conversion();     // -> InputResult { hanja_candidates_available: bool, .. }
  engine.is_hanja_mode();                           // bool
  let candidates = engine.get_hanja_candidates();    // Vec<(String, String)> 추정 (한자, 뜻)
  let selected = engine.select_hanja(0);             // Option<String>
  ```
- `src/input_engine/tests_popup_change_page.rs:23-35` — 테스트 전용 팝업 직접 구성:
  ```rust
  fn make_engine_with_hanja_popup(n_candidates: usize) -> super::InputEngine {
      // PopupState::new_hanja("한", candidates) 로 popup_state 직접 세팅
      // engine.hanja_mode = true; engine.hanja_target = "한".to_string();
  }
  ```
  → **`hanja_target`은 현재 `String` 단일 필드이고, 한 음절(또는 preedit 전체 문자열) 하나만 담는다.** 커밋된 여러 음절을 누적하는 구조가 아님.
- `src/input_engine/surrounding.rs:70` — `pub fn set_surrounding_text(&mut self, text: String, cursor_pos: u32, anchor_pos: u32)` — 비밀번호 필드에서는 fail-closed로 비움(§65-76 주석).
- `src/input_engine/surrounding.rs:83-89` — `pub fn surrounding_text(&self) -> (&str, u32, u32)`.
- `src/input_engine/surrounding.rs:105` — `pub fn smart_backspace(&self) -> Option<(u32, String)>` — **"미배선(실험적) — 현재 호출자는 테스트뿐. 전 배선은 v0.4.x 이후 (FUNC-LINUX-05)"**(105행 주석, 동일 문구가 `unim-dbus/src/service.rs:2754`에도 있음). 커서 앞 마지막 한글 음절을 자모 단위로 분해해 1글자 삭제 결과를 반환할 뿐, DBus 경로 전체가 실사용되지 않음.
- `src/hangul/input_context.rs:60` — `word_buffer: String` 필드. **주의**: 이건 "단어 모드(`accumulate_word`)"에서 진행 중인 음절 누적용이며 (335-366행), 앱에 **아직 커밋 신호를 보내지 않은** 내부 버퍼다. Syllable/Smart 커밋 모드(기본값 `CommitUnit::Smart`, `src/config.rs`)에서는 음절이 확정될 때마다 앱으로 개별 커밋되므로 `word_buffer`가 계속 비어 있다(주석: `input_context.rs:1096-1097` "syllable 모드에서 display 는 현재 음절만"). → **요구사항 대상①("대한민"이 이미 앱에 커밋된 상태)은 `word_buffer` 재사용으로 못 푼다** — 앱에 이미 나간 텍스트를 다시 읽어와야 하므로 `surrounding_text`/`SmartBackspace` 계열(비배선) 또는 새 "최근 커밋 이력" 버퍼가 필요.

### DBus 인터페이스 (`unim-dbus/SPEC.md:220-265`)

- `GetHanjaCandidates() -> (s target, a(ss) candidates)`
- `SelectHanja(u index) -> s hanja`
- `CancelHanja()`
- `SetSurroundingText(s text, u cursor_pos, u anchor_pos)` — "SmartBackspace/수동 TypeFix 선행 조건"(주석)
- `SmartBackspace() -> (u delete_chars, s replacement)` — **미배선**, `DeleteSurroundingText` 시그널 자동 발행까지는 구현되어 있음(`unim-dbus/src/service.rs:2781` 근방) 하지만 실사용 경로 없음
- `ToggleHanjaBookmark(u index) -> (u new_index, b bookmarked)`
- `popup_change_page(i direction)` / `TogglePopupExpand()`

### 실제 "이미 커밋된 텍스트 치환" 메커니즘 (AutoTypeFix가 이미 쓰는 것 — 재사용 대상)

- DBus 시그널 `delete_surrounding_text(offset: i32, n_chars: u32)` (`unim-dbus/src/client.rs:188`, 발행부 `unim-dbus/src/service.rs:2796`)
- 프런트엔드별 수신·적용:
  - GNOME extension: `unim-gnome-extension/unim_input_method.js:718` — `this.delete_surrounding(-(charCount), charCount)` (Clutter.InputMethod 네이티브 API)
  - GNOME extension(팝업 클릭 delete): `unim-gnome-extension/extension.js:678`
  - Wayland: `unim-frontends/wayland/src/state.rs:248-278` — `delete_surrounding_text` + `commit` + `preedit` (zwp_text_input_v3)
  - **XIM은 delete_surrounding 자체가 없어** N+1 synthetic BackSpace 주입 방식 사용(`unim-frontends/xim/src/handler.rs:118,452,546,638,1068,1111,1256`) — "자가 주입 BackSpace 카운터: N+1 (마지막 1개는 commit 트리거로 소비)"
  - GTK/Qt IM 모듈 쪽의 동등 배선은 이번 조사에서 파일 미확인(엔진-프런트엔드 서브시스템 조사 필요 — §7 참고)

## 3. 현재 동작 흐름 (단계별, 호출 순서)

**기존 단일 음절 한자 변환 흐름** (POPUP_SPEC.md §3.7, `docs/dev/specs/POPUP_SPEC.md:234-250`):

1. 한국어 모드, 한자키(F9/Hanja) 입력
2. **대상**: "preedit의 마지막 음절"(예: "대한민국" → "국") — 이미 커밋된 앞부분("대한민")은 대상에 포함되지 않음. 이것이 **로드맵 ①이 지적하는 정확한 제약**(ROADMAP.md:118-119: "현재는 `hanja_target`이 한 음절(또는 현재 preedit)이라 `대한민국`을 `大韓民國`으로 한 번에 바꿀 수 없습니다")
3. `GetHanjaCandidates()` → `(target, [(hanja, meaning)])`
4. 팝업 표시(9개/페이지) → 숫자/화살표/마우스로 선택 또는 ESC 취소
5. `SelectHanja(index)` → 엔진이 한자 반환 → 프런트엔드 커밋 + 팝업 닫기
6. 한자 후보 없음 + 초성만 있으면 특수문자 검색으로 자동 전환(§234 규칙7)
7. 즐겨찾기(Space) 토글 시 `HanjaCandidatesReordered` 시그널로 재정렬 + cursor 점프 + flash(§3.7 규칙8-9)

엔진 내부 키 처리 순서(POPUP_SPEC.md §9.2, `docs/dev/specs/POPUP_SPEC.md:596-609`):
`press_key` → 팝업 활성 확인 → `process_popup_key` 위임 → 숫자키(`popup_select`)/화살표(`PopupNavigate`)/ESC(`popup_cancel`→`HidePopup`)/기타(취소+키 재처리).
Hanja 키 자체는 `input_category`와 무관하게 언어 분기 **이전**에 처리(v3.2 정책, 605-609행) — idle이면 emoji popup, 조합 중이면 한자 변환.

**PopupRender SoT 발행 흐름** (POPUP_SPEC.md §10.3, 665-689행): ProcessKey RPC → `engine.press_key()` → `engine.popup_state().view_model(home_row)` → `build_render_state()` → `EngineResponse.render_state` → daemon이 `emit_popup_render` → 프런트엔드 `update_from_render(state)`.

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

| 확장 지점 | 방법 | 위험도 |
|---|---|---|
| **대상① 최근 커밋 음절 결합** | `hanja_target`을 단일 `String`에서 "최근 커밋 이력 + 현재 preedit" 결합으로 확장. `word_buffer`(`src/hangul/input_context.rs:60`) 재사용은 **불가**(§2 참고, 이미 앱에 나간 텍스트는 여기 없음) — 앱으로부터 다시 읽어와야 하므로 `surrounding_text` 경로(`src/input_engine/surrounding.rs:70-89`)를 진짜로 배선하거나, 엔진이 자체적으로 "최근 N 커밋 음절" 링버퍼를 별도로 유지해야 함. 후자는 POPUP_SPEC/AGENTS 어디에도 없는 **신규 상태** — 리셋 조건(포커스 아웃, 다른 앱 전환, 타임아웃, 커서 이동 등)을 새로 정의해야 함(§7 미해결 질문) | **높음** — 신규 상태 + 리셋 조건 설계, 커밋 모드(Syllable/Smart/Word) 3종 각각에서 동작 검증 필요 |
| **대상② selection 기반 변환** | `SetSurroundingText`가 이미 `cursor_pos`/`anchor_pos`를 받으므로 selection 유무는 `anchor_pos != cursor_pos`로 판별 가능(`surrounding.rs:70`). 다만 이 경로도 "미배선"과 마찬가지로 실제 프런트엔드가 selection 변경 시 `SetSurroundingText`를 호출하는지 이번 조사에서 미확인(엔진-프런트엔드 서브시스템 확인 필요) | **높음** — 프런트엔드별(GTK/Qt/XIM/Wayland/GNOME) selection 이벤트 훅 유무가 제각각일 가능성 |
| **커밋된 텍스트 치환** | AutoTypeFix가 이미 쓰는 `delete_surrounding_text` 시그널(§2) + GNOME/Wayland의 네이티브 delete, XIM의 N+1 BS 패턴을 재사용. `SmartBackspace`/`GetSurroundingText` 경로는 "미배선"이므로 **그대로 재사용 불가**, 대신 AutoTypeFix가 실제로 쓰는 경로(engine_worker.rs의 오프셋 계산, `unim-dbus/src/engine_worker.rs:547,574,794`)를 참고해 신규 배선 필요 | **중간** — 이미 검증된 패턴이 있으나 프런트엔드 5종 전부에 새로 연결해야 함 |
| **팝업 9개/즐겨찾기** | 기존 한자 팝업 인프라(PopupState, PopupViewModel, 9-페이지, ToggleHanjaBookmark) 그대로 재사용 가능 — POPUP_SPEC §2-3 규칙이 이미 단어 후보에도 형태적으로 맞음(후보가 여러 글자 문자열이어도 `cells`의 `text: String` 필드는 이미 임의 길이 문자열 허용, `docs/dev/specs/POPUP_SPEC.md:627-634`) | **낮음** — 레이아웃/키바인딩은 그대로, `target` 필드에 여러 음절이 들어가는 것만 다름 |
| **출력 형식 설정(漢字/한자(漢字)/漢字(한자))** | 신규 설정 항목 — **반드시 5/6지점 동기화**(§5) 준수. 실제 GUI 파일은 `unim-settings-gtk/src/settings_dialog.rs` (문서상 이름 `unim-gui-gtk` 아님, §5 참고) | **중간** — 지점 하나라도 누락 시 설정 불일치(GEMINI.md 규칙) |
| **PopupAction/PopupRender 확장** | 다음절 후보를 담을 필드가 이미 `String` 기반이라 타입 확장은 불필요해 보이나, `header_text`("「{target}」 → 한자") 포맷이 여러 음절 target에서도 자연스러운지, `footer_text`/`col_headers` 등에 영향 없는지 확인 필요(`docs/dev/specs/POPUP_SPEC.md:121-137`) | **낮음~중간** |

## 5. 지켜야 할 규칙 (인용, file:line)

### AGENTS.md (`docs/dev/architecture/AGENTS.md`)

- Zero Tolerance(§106-113, 110-112행): "`cargo build --workspace`는 **경고 0개**로 완료되어야 한다" / "`cargo test --workspace`는 **모든 테스트 통과**해야 한다" / "`make build`... 도 경고 없이 완료되어야 한다"
- 팝업 원칙(64-73행): "`PopupRender` payload가 단일 view-model SoT", "렌더러는 `PopupRender`만 소비하고, 자체 상태를 관리하지 않는다", "팝업 dismiss 단일 경로: `focus_out` / `reset`"
- 문서 언어(166행): "**문서·기획·walkthrough는 한국어**, **Git commit 메시지는 영어**"
- 로깅(167행): "`println!`, `log::*`, `console.log` 금지"
- 설정 동기화(168행): "설정 변경 시 **5지점 동기화** 규칙 준수"
- CHANGELOG 규칙(170-197행, 발췌):
  - "**한 항목 = 한 줄.**"
  - "**명사형 어미로 끝맺는다.** ... '지원'·'추가'·'개선'·'수정'·'변경'·'제거'·'이동' 같은 명사로 끝낸다."
  - "**철저히 사용자 관점.** ... 화면·버튼 이름은 대괄호로 표기한다(`[한글]`, `[설정]` 처럼)."
  - "**굵게(`**...**`)·강조 기호(★) 를 쓰지 않는다.**"
  - "**새 버전을 기록할 때 모든 언어 파일을 같은 커밋에서 함께 채운다.**"

### GEMINI.md (`docs/dev/architecture/GEMINI.md`)

- 5지점 동기화 대상 표(40-49행): 설정 코어(`src/config.rs`) / CLI(`unim-cli/src/main.rs`) / 로케일(`unim-cli/locales/*.yml`) / unim-dbus(`unim-dbus/src/service.rs`) / **`unim-gui-gtk/src/gtk_ui.rs`**(§5-문서-드리프트 참고 — 실제로는 `unim-settings-gtk/src/settings_dialog.rs`)
- 체크리스트(58-67행) 7항목, 1번: "`src/config.rs` — 설정 구조체에 새 필드 추가 (+ `clamp_ranges()` 방어 시 범위 확인)"
- 로깅 규칙(170-176행): "기존 `log::*` 크레이트 사용 금지", "환경변수 의존: 프로덕션 환경에서는 `UNIM_DEVELOP`이 설정되지 않으므로 로그가 출력되지 않습니다"
- 한국어 주석 관련 명문 규칙은 GEMINI.md/AGENTS.md에 **없음**(확인: `rg "주석"` 결과 없음). 다만 문서 전체가 한국어 주석/설명으로 일관 작성되어 있어 **관례적**으로 한국어 주석을 쓴다(강제 규칙 문서는 발견 못함).

### CONTRIBUTING.md

- Zero Tolerance(7-19행) — AGENTS.md와 동일 3원칙, "기존 이슈 방치 금지"(17-18행) 추가: "'내가 한 것이 아니니까' 넘기지 말고 즉시 수정"
- **6지점 sync 체크리스트(87-97행, 팝업 관련 변경 시 필수)**:
  ```
  - [ ] src/config.rs — 설정 구조체
  - [ ] unim-cli/src/main.rs — ConfigKey enum
  - [ ] unim-cli/locales/{ko,en}.yml — CLI 라벨
  - [ ] unim-dbus/src/service.rs — DBus 디스패치
  - [ ] unim-gui-gtk/src/settings_dialog.rs — GUI 위젯   ← 실제 경로는 unim-settings-gtk/src/settings_dialog.rs
  - [ ] docs/dev/specs/POPUP_SPEC.md — 팝업 명세 (팝업 변경 시)
  - [ ] unim-gnome-extension/popup_view.js — GNOME 렌더러 (GNOME 팝업 변경 시)
  - [ ] unim-popup-service/src/ — popup-service 렌더러 (팝업 변경 시)
  ```
  → **한자 단어 입력은 팝업+설정 둘 다 걸치므로 이 6지점 전부 대상.**
- 브랜치 워크플로(51-64행): `main`=릴리스 전용, `develop`=활성 통합, `feature/*`·`fix/*`·`claude/*`가 `develop` base로 분기 → `develop`로 PR
- 문서화(66-72행): "모듈의 아키텍처나 주요 작동 방식이 바뀌면, 해당 모듈 디렉토리(예: `unim-frontends/gtk4/`)의 `SPEC.md`를 즉시 반영"

### POPUP_SPEC.md — 한자 팝업 규칙 원문 (`docs/dev/specs/POPUP_SPEC.md`)

- 페이지/즐겨찾기 상수(162-174행): 페이지 크기 **9**(숫자키 1~9 대응), 초기 선택 0, 최소너비 280px/최대 420px, 패딩 12px, 행높이 28px
- 키 바인딩 표(187-202행): `1~9` 즉시선택+커밋, `Enter` 확정, `Space` 즐겨찾기 토글, `↑↓` wrap-around 이동, `←/PageUp`·`→/PageDown` wrap-around 페이지 전환, `Home`/`End`(v3.2) 첫/끝 점프, `Escape` 취소, `.`(Period) compact↔expanded 토글, 기타 키는 팝업 닫고 재처리
- 동작 규칙 9항목(234-250행), 특히 2번(237행): "**대상**: preedit의 마지막 음절 (예: '대한민국' → '국')" — **이 문장 자체가 로드맵이 지적하는 현재 제약의 명세 근거**이며, 한자 단어 입력 기능은 이 규칙을 다음절 대상으로 개정해야 함 → **이는 "명세 변경"에 해당하므로 사용자 승인 필요**(아래 참고)
- "변경 시 승인 필수" 문구는 **POPUP_SPEC.md 본문에는 없음**(확인: `rg "승인|변경 시" docs/dev/specs/POPUP_SPEC.md` 결과 0건). 다만 세션 메모리(`~/.claude/projects/.../memory/feedback_popup_spec_absolute.md`)에 "POPUP_SPEC.md 규칙은 예외 없이 준수... **명세 자체를 변경하려면 반드시 사용자 승인 필요**"라는 기현님 지시(2026-03-29)가 별도로 기록되어 있음 — **문서 자체가 아니라 운영 규칙(메모리)에 있는 제약**이므로, POPUP_SPEC.md §3.7 규칙2("대상: 마지막 음절")를 다음절로 바꾸는 설계는 문서 수정 전 반드시 사용자 승인을 받아야 함.
- 변경 이력(726-736행)에 유사 확장 사례가 이미 존재 — v3.1(마우스 페이지네이션), v3.2(PopupViewModel SoT), v3.3(GNOME Wayland 분기) 모두 "버전 bump + 변경 내용 한 줄 요약"으로 기록. 한자 단어 입력 확장 시 **v3.4 항목 추가가 관례에 맞음**.

### 테스트 의무

- AGENTS.md/GEMINI.md/CONTRIBUTING.md 공통: `cargo test --workspace` 전부 통과 + 신규 경고/실패 방치 금지(§Zero Tolerance)
- 명시적 "신기능은 반드시 새 테스트를 추가하라"는 문장은 세 문서 어디에도 **없음**(확인: rg 결과 없음). 다만 `src/input_engine/tests_scenarios.rs`에 시나리오별 테스트가 축적되는 기존 관례(`test_scenario_hanja_conversion` 등)로 보아 새 시나리오("한자 단어 변환")도 이 파일 또는 신규 `tests_hanja_word.rs`에 추가하는 것이 자연스러움.

## 6. Windows 동등성 메모

- POPUP_SPEC.md §1.2(28-39행)는 Linux 프런트엔드만 다루고, Windows(TSF)는 이 표에 없음 — 별도 `unim-tsf`/`unim-popup-win` 경로.
- 사용자 매뉴얼(`docs/user/user-guide/README-ko.md:263-269`)에 따르면 Windows는 `unim-popup-win.exe`가 팝업 전용 렌더러로 동일 SoT(입력기 본체가 "무엇을 보여줄지"만 넘김) 구조를 따름 — Linux popup-service와 설계상 대칭.
- AutoTypeFix의 커밋 텍스트 치환은 `unim-tsf/src/auto_typefix.rs:445-447`에 별도 Windows 구현이 있고, 주석에 "엔진 word_buffer 와 정합, 이어치기" 언급 — Windows도 Linux와 동일하게 word_buffer는 미커밋 누적용이며, 이미 커밋된 텍스트 재작업은 TSF의 `ITextStoreACP`(surrounding text) API에 의존할 것으로 추정(이번 조사에서 TSF 쪽 구체 함수까지는 미확인 — Windows 서브시스템 조사 필요).
- ROADMAP.md의 "① 한자 단어 단위 변환" 항목 자체는 플랫폼 구분 없이 "src/" 코어 레벨(`hanja_target`, `HanjaDictionary`) 이슈로 기술되어 있어(118-127행), 코어 로직은 Linux/Windows 공용이고 "치환 채널"(delete_surrounding vs TSF API vs N+1 BS)만 플랫폼별로 다름.

## 7. 미해결 질문

1. **"최근 커밋 음절"을 어디에 얼마나 보관할 것인가**: `word_buffer`는 재사용 불가(§2,4). 엔진에 새 필드(예: `recent_committed: String` + 리셋 조건)를 만들 것인지, 아니면 매번 `surrounding_text`를 프런트엔드에 요청해 읽어올 것인지 — 후자는 `SmartBackspace`와 마찬가지로 "미배선" 상태를 실제 배선해야 하는 큰 작업(FUNC-LINUX-05 범위와 겹침). 이 판단은 엔진-프런트엔드 서브시스템 조사·설계자가 결정할 사안.
2. **커밋 모드 3종(Syllable/Smart/Word, `src/config.rs:59-88`)별로 대상① 트리거 조건이 다른가**: Word 모드에서는 애초에 단어 전체가 preedit에 남아있어(`word_buffer`+`preedit`) 커밋된 부분이 없을 수 있음 — 이 경우 기존 단일 음절 로직으로 이미 충분할 수도. 설계 시 3개 모드 각각의 시나리오를 명시해야 함.
3. **selection 기반 변환(대상②)의 프런트엔드별 이벤트 소스**: GTK/Qt/XIM/Wayland/GNOME 중 어느 것이 실제로 "텍스트 선택 변경"을 엔진에 알리는 경로를 갖고 있는지 이번 조사에서 확인하지 못함(코드에 selection 관련 DBus 메서드가 SPEC.md §6.1에 안 보임 — `GetHanjaCandidates`는 preedit 기반뿐). 신규 DBus 메서드(예: `GetHanjaCandidatesForSelection(text)`)가 필요할 가능성.
4. **POPUP_SPEC.md §3.7 규칙2 문구 개정 승인**: "대상: preedit의 마지막 음절" 문구를 다음절로 바꾸는 것 자체가 명세 변경 → 착수 전 기현님 승인 필요(§5 참고). 승인 없이 코드만 다르게 동작시키면 "명세와 다른 구현" 상태가 되어 메모리 규칙 위반.
5. **AGENTS.md/GEMINI.md/CONTRIBUTING.md의 `unim-gui-gtk` 경로 드리프트**: 세 문서 모두 실재하지 않는 크레이트명(`unim-gui-gtk/src/gtk_ui.rs` 또는 `settings_dialog.rs`)을 5/6지점 체크리스트에 명시하고 있음. 실제는 `unim-settings-gtk/src/settings_dialog.rs`. 이번 기능 설계 문서에는 올바른 경로를 쓰되, 이 문서 드리프트 자체를 별도로 고칠지는 사용자 판단 필요(작업 범위 밖일 수 있음 — CLAUDE.md 3번 "Surgical Changes" 원칙상 무관한 정정은 별도 언급만 하고 건드리지 않는 것이 맞음).
6. **한자 단어 입력 기능의 L3 Xvfb 시나리오 신설 여부**: `tests/harness/scenarios/`에 자판 프로필별 시나리오만 있고 기능별(팝업 등) 시나리오는 없어 보임 — 이 기능이 L3 레벨 테스트 대상이 될지, DBus 레벨(`tests/unim-test-dbus`)의 `test_hanja_popup` 확장으로 충분할지는 테스트 전략 결정 필요.

## 보충 #11: L1/L2/L3 테스트 인프라의 팝업 흐름 표현력

### (1) `tests/harness/harness.py` — L3 Xvfb 하네스

- **스텝 어휘** (492-514행): 한 스텝은 `click`/`key`/`keys`/`mode`/`wait` 중 정확히 하나만 가져야 하며(512-514행 `else: raise RuntimeError`), 그 외 키는 없다. `key`/`keys`는 그대로 `inj.key(...)` → `xdotool key ...`로 넘어가므로(499·503행), **`'F9'`·`'1'` 같은 xdotool 키명은 그대로 주입 가능**하다 — xdotool은 `F9`(펑션키)와 `1`(숫자키) 둘 다 표준 키심(keysym) 이름으로 인식한다. 별도 popup 전용 스텝 타입은 없다.
- **판정**: `expect`는 `_wait_for`(359행) → `_match`(353-356행: `all(render.get(k)==v for k,v in expect.items())`)로, `render`는 `RunningApp.last_render(field)`(265-269행)가 반환하는 최신 `field.render` 이벤트 dict다. 실제 시나리오 JSON(`scenarios/2bulstd.json:9-12`, `common.json:9-22`)이 `expect: {"preedit":..., "committed":..., "rendered":...}` 3키를 이미 조합해 쓰고 있어, **"대한민"+"국" 입력 → `key F9` → `key 1` → `expect: {"committed": "대한민국", "rendered": "대한민국"}` 형태 한 시나리오로 팝업을 직접 관측하지 않고도 최종 결과만으로 판정 가능**하다. 단, 이는 앱 드라이버(각 데모 앱)가 `field.render` 이벤트에 최신 committed/rendered 텍스트를 계속 실어 보낸다는 기존 전제에 의존하며 별도 "popup" 이벤트는 필요 없다.
- **`set_config`(134-135행)**: `_gdbus("SetConfig", key, value)`를 그대로 호출하는 범용 함수라 `commit_unit`·`hanja_output_format` 등 어떤 config 키도 값 설정 자체는 된다. 다만 **`run_scenario` 안에서 시나리오 JSON을 통해 자동으로 적용되는 config 키는 `layout`(398-408행, `korean_layout` 전용) 하나뿐**이다 — `sc.get(...)` 호출 전수 조사(387·398·421·444·447행) 결과 `known_fail`/`layout`/`key_delay_ms`/`korean`/`field` 외에 범용 `config` 딕셔너리 처리 로직은 존재하지 않는다. 즉 `commit_unit`/`hanja_output_format`을 시나리오 단위로 바꾸려면 `layout` 패턴(398-408행: 저장→설정→`finally`에서 복원, 550-551행)을 그대로 본떠 harness.py에 소규모 확장을 추가해야 하며, 현재 코드만으로는 시나리오 JSON에 그런 키를 적어도 무시된다.
- **xtest 제외**: 388-390행에서 `if not spec["xtest"]: res.skipped = "XTEST 가 닿지 않는 앱 (Wayland 네이티브)"; return res` — Wayland 네이티브 앱(`xtest: False`, 35·81·86행)은 L3 시나리오 실행 자체에서 스킵되어 결과에 포함되지 않는다(실패가 아니라 스킵으로 집계).

### (2) `tests/unim-test-dbus/src/main.rs` — L2 DBus 테스트

- `test_hanja_popup`(166행~)은 **혼합 경로**다: ①`ic.process_key_event(0, 123, 0)`(evdev Hanja키, 193-196행)로 엔진에 "한자 변환 시작"을 **push**로 트리거 → ②`ic.get_hanja_candidates()`(200행)로 후보 목록을 **pull**(RPC) → ③`ic.select_hanja(0)`(243행)로 선택도 **pull**(RPC)이다. 즉 트리거는 키 이벤트, 조회·선택은 순수 GetHanjaCandidates/SelectHanja RPC 경로이며 커밋 결과 자체를 다시 ProcessKey로 검증하지는 않는다(`selected`가 빈 문자열이 아닌지만 확인, 246-258행).
- `SetSurroundingText` 관련 테스트: 파일 전체(`rg -n "SetSurroundingText" tests/unim-test-dbus/src/main.rs`) **0건**. 대상②(selection 기반 변환)를 검증할 DBus 템플릿은 **아직 없다** — SPEC.md §6.1에도 selection 관련 메서드가 없다는 기존 조사(미해결 질문 3)와 일치. 신규 기능은 새 RPC(예: `GetHanjaCandidatesForSelection`)와 그에 대응하는 새 테스트 함수를 처음부터 만들어야 한다.

### (3) `src/input_engine/tests_scenarios.rs:277-303` — L1 유닛

- `test_scenario_hanja_conversion`은 `press_key`로 "가"를 **preedit에만** 쌓고(286-288행) `start_hanja_conversion()`을 그대로 호출한다 — **`clear_commit()`을 전혀 호출하지 않는다**(이 테스트 함수 안에서 `clear_commit` 매칭 0건; 파일 내 다른 3곳(70·77·82행)은 별개 테스트). 즉 이 테스트는 커밋 버퍼/최근 커밋 음절과 무관하게 preedit 단독 경로만 돈다.
- `engine.rs:731-733`의 `clear_commit()`은 `self.commit_buffer.clear()` 하나뿐이고, `commit_buffer`는 프런트엔드가 매 키 이벤트마다 `commit_str()`로 읽어가는 **1회성 플러시 버퍼**다(실제 런타임에서는 프런트엔드가 계속 읽어가며 비워짐). `input_engine/*.rs` 전체에서 `recent_committed`/`last_committed`/`committed_syllables`/`commit_history` 류 필드는 **검색 결과 0건** — "최근 커밋 음절을 기억"하는 대상①용 버퍼는 코드베이스에 아직 전혀 존재하지 않는다. 따라서 gap 3(대상① 훅 위치)은 기존 `commit_buffer`를 재사용할 수 없고(그건 프런트엔드가 즉시 비움) **독립된 신규 상태 필드**를 엔진에 추가해야 하며, 그 결과 L1 테스트도 `test_scenario_hanja_conversion`류의 "clear_commit 없는" 패턴을 그대로 이어받되 새 필드의 초기화/리셋 조건(모드 전환·포커스 아웃·타임아웃 등)을 별도로 검증하는 신규 테스트가 필요하다 — 기존 패턴에서 그대로 파생되지 않는다.

### 결론

- L3는 팝업을 직접 들여다보지 않아도 `preedit`/`committed`/`rendered` 3키 조합만으로 대상①·②의 "최종 결과"를 판정할 수 있고 `F9`/숫자키 주입도 문제없다. 단 시나리오 단위 config 전환(`commit_unit`/`hanja_output_format`)은 harness.py에 `layout`과 같은 패턴의 소규모 확장이 선행돼야 한다(현재는 미지원).
- L2는 대상①(push 트리거 + pull 조회/선택) 템플릿은 있으나 대상②(selection, SetSurroundingText) 템플릿은 전무 — 신규 RPC+테스트 필요.
- L1은 "최근 커밋 음절" 버퍼 자체가 코드에 없어 gap 3의 설계(필드 위치·리셋 조건)가 먼저 나와야 테스트 패턴도 정해진다.
