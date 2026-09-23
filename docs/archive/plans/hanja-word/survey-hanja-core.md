# 한자 코어·사전·즐겨찾기

## 1. 핵심 파일과 역할

| 파일 | 역할 |
|---|---|
| `src/hanja/mod.rs` | 서브모듈 재노출 (`bookmark`, `dict`) |
| `src/hanja/dict.rs` | `hanja.txt` 파싱 + `HanjaDictionary`(HashMap 기반 검색) |
| `src/hanja/bookmark.rs` | `HanjaBookmarkStore` — 즐겨찾기 JSON 영속화 |
| `src/data/hanja.txt` | libhangul 유래 한자 사전 원본 (6.45MB, 303,521줄) |
| `src/input_engine/candidates.rs` | 한자/특수문자 변환 상태머신 (start/select/toggle/cancel) — `InputEngine` 의 impl 블록 |
| `src/input_engine/types.rs` | `InputResult`(ABI 고정 `repr(C)`), `PopupAction` enum |
| `src/input_engine/press_key.rs` | 한자키 진입점 dispatch (`press_key()` 최상위) |
| `src/input_engine/engine.rs` | `InputEngine` 구조체 필드 선언 + `reset()` |
| `src/input_engine/surrounding.rs` | `surrounding_text`/`smart_backspace`/`typefix_convert` — AutoTypeFix 의 "삭제+대체" 노하우 원본 |
| `src/popup/popup_state.rs`, `popup_keys.rs`, `popup_layout.rs` | 팝업 그리드/키 처리 (한자 compact=9/페이지, expanded=9×9) — 이미 요구사항 충족 |
| `docs/dev/specs/POPUP_SPEC.md` | 한자/특수문자/이모지 팝업 통합 규격서 (§2~§3이 한자) |
| `unim-dbus/src/service.rs` | Linux DBus: `select_hanja`, `delete_surrounding_text`/`commit_text` 시그널 |
| `unim-dbus/src/engine_worker.rs` | DBus↔엔진 브리지 (`engine.select_hanja(index)` 등 호출) |
| `unim-tsf/src/key_handler.rs`, `composition.rs` | Windows TSF: 한자키 dispatch, `comp_mgr.replace_surrounding` (typefix 재사용 대상) |

## 2. 핵심 타입·함수 (file:line · 시그니처 · 역할)

### 사전 (`src/hanja/dict.rs`)
- `HanjaEntry { hangul: String, hanja: String, meaning: String }` — dict.rs:13-20. `first_hanja_char(&self) -> Option<char>` dict.rs:25-27.
- `const HANJA_DATA: &str = include_str!("../data/hanja.txt")` — dict.rs:9. **빌드 시 바이너리에 정적 임베드**(지연 로딩 아님). 파싱은 `HanjaDictionary::new()` 호출 시점(런타임, 프로세스 시작 1회)에 전체 303,521줄을 순회하며 발생 — dict.rs:49-52.
- `HanjaDictionary { entries: HashMap<String, Vec<HanjaEntry>> }` — dict.rs:34-37. **키 타입은 `String`(임의 길이)** — 단음절("가")뿐 아니라 다음절("대한민국")도 키가 될 수 있는 구조. 현재 실제로 다음절 키가 사전에 존재함(§ "데이터 포맷" 참조).
- `fn parse_dictionary(data: &str) -> HashMap<String, Vec<HanjaEntry>>` — dict.rs:55-92. 형식: `한글:한자:설명`(`splitn(3, ':')`). `#` 시작 줄/빈 줄 무시. `hangul`/`hanja` 둘 중 하나라도 비면 skip. **중복 키 처리: dedup 없음** — 동일 `hangul` 키에 여러 줄이 있으면 `entries.entry(hangul).or_default().push(entry)`(dict.rs:87)로 전부 Vec에 누적된다. 실측: 고유 hangul 키 중 25,199개가 2줄 이상(= 후보가 여럿인 정상 케이스, 사전 내 순서가 곧 "빈도순"이라는 게 코드 주석의 전제).
- `fn search(&self, hangul: &str) -> Vec<HanjaEntry>` — dict.rs:104-106. `self.entries.get(hangul).cloned().unwrap_or_default()`. **길이 제한 없음 — 이미 다음절 문자열로도 그대로 호출 가능.** 별도의 "다음절 lookup API"를 새로 만들 필요 없이 이 함수가 그대로 대상 ①②에 재사용 가능하다.
- `fn search_last_syllable(&self, text: &str) -> Option<(String, Vec<HanjaEntry>)>` — dict.rs:119-128. `text.chars().last()`로 **마지막 한 글자만** 뽑아 `search()` 호출. prefix 검색 API는 없음(확인: `rg "fn.*prefix" src/hanja/`→ 매치 없음).
- `entry_count()`/`key_count()` — dict.rs:131-138, 통계용.
- 메모리/시작시간: 6.45MB 텍스트를 `HashMap<String, Vec<HanjaEntry>>`로 완전 파싱 후 상주. `HanjaEntry`가 `hangul` 필드까지 값마다 중복 보관(키와 동일 문자열을 Value 안에도 저장) — 27.5만 항목 규모에서 문자열 힙 오버헤드 존재하나 현재도 이 상태로 운영 중(신규 리스크 아님). 시작 지연은 엔진 초기화 1회성(테스트에서도 `HanjaDictionary::new()` 매 호출마다 재파싱 — `Arc<HanjaDictionary>`로 감싸 엔진 간 공유, engine.rs:115 `pub(super) hanja_dict: std::sync::Arc<crate::hanja::HanjaDictionary>`).

### 즐겨찾기 (`src/hanja/bookmark.rs`)
- `HanjaBookmarkStore { entries: BTreeMap<String, BTreeSet<String>>, path: Option<PathBuf> }` — bookmark.rs:19-24. **키는 한글 문자열(길이 무관, 이미 단어도 담을 수 있는 구조) → 즐겨찾기된 한자 문자열 집합.** "뜻"은 저장하지 않는다 — 순수 (한글키, 한자문자열) 매핑뿐.
- 파일 경로: `crate::paths::data_dir().join("unim").join("hanja-bookmarks.json")` — bookmark.rs:53, 즉 `~/.local/share/unim/hanja-bookmarks.json` (주석 bookmark.rs:6).
- 포맷: `{ "한글키": ["한자1", "한자2", ...], ... }` (BTreeMap→직렬화, `serde_json::to_string_pretty`) — bookmark.rs:96-103.
- `load_default()`/`load_from_path(path)` — bookmark.rs:28-49. 파일 없음/파싱 실패 시 **조용히 빈 저장소로 폴백**(에러 전파 없음, `unwrap_or_default()` / `Err(_) => BTreeMap::new()`).
- `is_bookmarked(&self, hangul: &str, hanja: &str) -> bool` — bookmark.rs:57-61.
- `toggle(&mut self, hangul: &str, hanja: &str) -> bool` — bookmark.rs:64-78. 매 토글마다 **즉시 `save()`**(디스크 I/O, bookmark.rs:76) — 디바운스 없음. 빈 집합이 되면 키 자체를 제거(bookmark.rs:73-75).
- `list(&self, hangul: &str) -> Vec<String>` — bookmark.rs:81-86.
- 리로드: 명시적 리로드 API 없음 — 프로세스 시작 시 `load_default()` 1회, 이후 메모리 상태가 SoT(다른 프로세스가 파일을 바꿔도 반영 안 됨). 멀티 프로세스(popup-service 등) 간 공유 여부는 이 조사 범위 밖(엔진 프로세스마다 자체 인스턴스 보유, engine.rs:259 `hanja_bookmarks: crate::hanja::HanjaBookmarkStore::load_default()`).

### 상태머신 (`src/input_engine/candidates.rs`)
- `pub fn start_hanja_conversion(&mut self) -> InputResult` — candidates.rs:16-109.
  - **이미 hanja_mode면 즉시 `InputResult::consumed()`**(재진입 가드, candidates.rs:18-20).
  - **target 결정 로직 (candidates.rs:23-29, 현재 구현의 핵심 제약):**
    ```rust
    let target = if !self.preedit_cache.is_empty() {
        self.preedit_cache.chars().last().map(|c| c.to_string())  // 마지막 "한 글자"만
    } else {
        None   // 커밋 버퍼 음절 사용 경로 — 미구현, 주석만 존재
    };
    ```
    - `preedit_cache`가 비어있지 않으면 그 **마지막 문자 1개**만 대상이 된다. `commit_unit=Word/Smart`(단어 누적 모드)에서 `preedit_cache`는 `korean_context.get_preedit_display()`(= `word_buffer + preedit`, `input_context.rs:335-341`)라서 이미 누적된 여러 음절을 담고 있을 수 있는데도 **마지막 한 글자만 잘라 쓴다** — 즉 Word/Smart 모드에서도 다음절 결합은 현재 활용되지 않고 있다.
    - `preedit_cache`가 비어있으면(=Syllable 모드에서 방금 음절이 이미 커밋되어 앱으로 나간 직후, 또는 idle) `target = None` → **바로 "후보 없음" 경로로 빠진다.** 요구사항 대상①("대한민"이 이미 커밋되고 "국"이 조합 중")의 "이미 커밋된 이전 음절"을 기억하는 버퍼가 **엔진에 전혀 없다.**
  - 후보 있으면(candidates.rs:33-71): 즐겨찾기 우선 stable sort(candidates.rs:36) → `self.hanja_target`/`hanja_candidates`/`hanja_mode=true` 세팅 → `PopupState::new_hanja_with_top_row` 생성 → `popup_pending_action = Some(PopupAction::ShowHanja{..})` → `InputResult::hanja_candidates()` 반환.
  - 후보 없고 target이 초성 1글자면(candidates.rs:75-104): `crate::special_chars::search_by_choseong(ch)`로 특수문자 fallback.
  - 둘 다 없으면(candidates.rs:107-108): `InputResult::consumed()`(팝업 없이 키만 소비).
- `pub fn is_hanja_mode(&self) -> bool` — candidates.rs:112-114.
- `pub fn get_hanja_candidates(&self) -> Vec<(String, String)>` — candidates.rs:119-124, (한자, 뜻) 튜플 리스트.
- `pub fn get_hanja_target(&self) -> &str` — candidates.rs:127-129.
- `pub fn select_hanja(&mut self, index: usize) -> Option<String>` — candidates.rs:140-160.
  - 범위 밖 index → `None`.
  - **preedit 처리(candidates.rs:150-156): `korean_context.clear()` + `preedit_cache.clear()` — commit_buffer에는 추가하지 않는다.** 주석: "DBus 응답으로 한자를 반환하므로 commit_buffer에 추가하지 않음(추가 시 다음 키 입력에 묻어나와 이중 커밋 발생)". 즉 **커밋 경로는 `InputResult`/`commit_buffer`가 아니라 반환값 `Option<String>`(DBus RPC 응답)이다.** 호출자(`unim-dbus/src/service.rs:2886` `select_hanja` DBus 메서드, `unim-dbus/src/engine_worker.rs:2023`)가 이 문자열을 받아 **CommitText 시그널로 직접 커밋**한다 — 앱에 이미 나간 텍스트를 지우는 동작은 여기 전혀 없다(현재는 필요 없음: 대상이 항상 아직 안 나간 preedit이므로).
  - 마지막에 `self.cancel_hanja()` 호출(candidates.rs:158) → 후보/타깃/팝업 상태까지 정리.
- `pub fn hanja_bookmark_states(&self) -> Vec<bool>` — candidates.rs:166-174, `get_hanja_candidates()`와 동일 순서의 즐겨찾기 bool.
- `pub fn toggle_hanja_bookmark(&mut self, index: usize) -> Option<(usize, bool, bool)>` — candidates.rs:186-263. 반환 3-튜플 `(new_index, new_state, was_state)`.
  - 토글 후 **사전에서 다시 `search()`로 fresh 후보를 받아 재정렬**(candidates.rs:201-205) — "즐겨찾기 해제 시 원래 사전 순서로 되돌아가야 한다"는 요구를 stable-sort 재적용 방식으로 구현.
  - `popup_state.replace_hanja_items(...)` + `set_selected_global(new_index)`로 팝업 상태 일괄 갱신(candidates.rs:229-237).
  - `PopupAction::HanjaCandidatesReordered{..}` 발행(candidates.rs:240-250) — frontend가 후보+즐겨찾기+커서를 한 트랜잭션으로 교체.
- `pub fn cancel_hanja(&mut self)` — candidates.rs:266-275. `hanja_mode=false`, `hanja_candidates.clear()`, `hanja_target.clear()`, `popup_state=None`, **그리고 `korean_context.clear()` + `preedit_cache.clear()`** — "한자 선택 후 원래 한글이 남지 않도록"(주석 candidates.rs:272). **주의**: select_hanja/cancel_hanja 어느 경로든 preedit 자체는 통째로 사라진다 — 부분(예: 다음절 preedit 중 마지막 음절만) 취소는 지원 안 됨.

### 진입점 (`src/input_engine/press_key.rs`)
- 한자키 dispatch는 `press_key()` 최상위(언어 모드 무관), Korean 전용 분기(`process_korean_key`) 밖에서 처리 — press_key.rs:219-238:
  ```rust
  if self.hanja_keys.contains(&keycode) {
      self.finalize_chord_buffer();                    // chord 대기 자모 flush
      let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
      if idle {
          self.start_emoji_popup();                     // idle → 이모지 (한/영 모드 무관 항상 ON)
          return InputResult::consumed();
      }
      return self.start_hanja_conversion();             // 조합 중 → 한자 변환
  }
  ```
  - **idle 판정 기준이 그대로 "대상①이 막히는 지점"이다** — Syllable 모드에서 "대한민"이 이미 커밋되고 "국"의 조합도 방금 끝나 preedit이 비면(예: 스페이스 없이 바로 다음 문자 타이핑 전) `idle=true`가 되어 **한자가 아니라 이모지 팝업이 뜬다.** "국"이 아직 preedit에 남아있는 그 찰나에만 한자 진입이 된다.
  - `hanja_keys: Vec<String>` — config.rs:954, 기본값 `["Hanja", "F9"]`(config.rs:992). `press_key`가 자체 `self.hanja_keys: Vec<KeyCode>`(파싱된 형태, engine.rs 필드)로 비교(문자열 파싱은 config.rs 주석 press_key.rs:777-789 참조 — 별칭/공백 불허, 대소문자 구분).
- `finalize_chord_buffer()` 호출 이유: 세벌식 chord 입력 중 한자키가 눌리면 먼저 현재 chord를 음절로 확정.

### 상태 필드 선언 (`src/input_engine/engine.rs`)
- `commit_buffer: String` (engine.rs:100), `preedit_cache: String` (102), `hanja_dict: Arc<HanjaDictionary>` (115), `hanja_bookmarks: HanjaBookmarkStore` (117), `hanja_candidates: Vec<HanjaEntry>` (119), `hanja_mode: bool` (121), `hanja_target: String` (123).
- **"최근 커밋된 음절을 기억하는 버퍼"에 해당하는 필드가 없다** — `commit_buffer`는 "이번 키 처리로 새로 확정된 조각"을 담아 즉시 host로 flush되는 단명 버퍼(1회성 소비 후 `clear_commit()`으로 비워짐, engine.rs:731-733)이지 누적 이력이 아니다.
- `reset()` — engine.rs:764-784. `hanja_mode`/`hanja_candidates`/`hanja_target`을 명시적으로 초기화(772-774). 새 "최근 커밋 음절 버퍼"를 추가한다면 **이 함수가 리셋 지점 후보 1순위**(포커스 전환·언어 전환·명시적 reset 등 기존에 잘 정의된 리셋 트리거를 그대로 물려받을 수 있음).

### `InputResult`/`PopupAction` (`src/input_engine/types.rs`)
- `InputResult`는 `#[repr(C)]` + **`unim-capi` ABI 경계**(types.rs:60-61 주석: "`InputResult`(`repr(C)` + unim-capi ABI)에는 필드를 추가하지 않으며, `popup_pending_action` 선례와 동일한 out-of-band 드레인 패턴으로 ABI 를 보존한다"). 필드 5개: `consumed, preedit_changed, commit_changed, hanja_candidates_available, special_char_candidates_available` (types.rs:225-237).
- 생성자: `not_consumed()`(241), `consumed()`(252), `preedit_updated()`(263), `committed()`(274), `committed_passthrough()`(286), `hanja_candidates()`(297), `special_char_candidates()`(308).
- `PopupAction` enum(types.rs:124-218)은 ABI 제약이 없는 out-of-band 채널 — `self.popup_pending_action: Option<PopupAction>` 필드에 적재되고 호출자가 드레인. 새 payload(예: 대상①의 "삭제할 글자 수")를 실어 보내려면 `InputResult`가 아니라 **이 `PopupAction`(또는 `AtfToggleKind`와 같은 별도 out-of-band 필드)** 경로를 새로 만들어야 한다 — types.rs:57-61 주석이 이 패턴을 명시적으로 권장.
- 신규 액션 후보 위치: `PopupAction::ShowHanja{ target, candidates, top_row }`(126-131)는 이미 `target: String`을 담고 있으므로 다음절 target도 타입 변경 없이 그대로 흘려보낼 수 있음. 다만 "삭제할 이전 커밋 글자 수"는 이 enum에 없음 — 신규 필드/variant 필요.

## 3. 현재 동작 흐름 (단계별, 호출 순서)

### 3.1 기존 단음절 한자 변환 (Syllable 모드, "가" 조합 중 한자키)
1. 사용자가 자모를 눌러 "가"가 preedit으로 조합됨(`korean_context` 내부, `update_preedit_cache()`가 `preedit_cache="가"`로 반영— press_key.rs:1097-1099).
2. 한자키 입력 → `press_key()` 최상위에서 `hanja_keys` 매치(press_key.rs:226) → `finalize_chord_buffer()` → `idle=false`(preedit_cache 비어있지 않음) → `start_hanja_conversion()` 호출.
3. `start_hanja_conversion`: `target_syllable = "가"`(preedit_cache 마지막 글자, candidates.rs:25) → `hanja_dict.search("가")` → 다수 후보 → 즐겨찾기 우선 정렬 → `hanja_mode=true`, `hanja_target="가"`, `hanja_candidates=[...]` → `PopupState::new_hanja_with_top_row` 생성, `popup_state=Some(..)` → `popup_pending_action=Some(ShowHanja{target:"가", candidates, top_row})` → `InputResult::hanja_candidates()` 반환(consumed=true, preedit_changed=true, hanja_candidates_available=true).
4. Frontend가 `popup_pending_action`을 드레인해 `ShowHanja` 시그널을 DBus로 발행(`unim-dbus`) → 팝업 UI가 뜸.
5. 사용자가 팝업에서 항목 클릭/번호키 → (unim-gui-common 등) `select_hanja_via_dbus(index)` → DBus `SelectHanja(index)` → `engine_worker.rs:2023` `engine.select_hanja(index)` 호출.
6. `select_hanja`: 유효 index → `hanja = candidates[index].hanja.clone()` → **preedit이 비어있지 않으면 `korean_context.clear()`+`preedit_cache.clear()`**(이 시점 preedit="가")** → `cancel_hanja()`(팝업/상태 전부 정리) → `Some(hanja)` 반환.
7. DBus 메서드가 이 `hanja` 문자열을 **RPC 반환값**으로 클라이언트에 돌려줌(`SelectHanja(u index) -> (s hanja)`, POPUP_SPEC.md:101) — 클라이언트가 이를 받아 `CommitText`로 앱에 커밋(엔진이 직접 commit_buffer/InputResult로 알리지 않음, 이 함수 시그니처 자체가 RPC 응답 채널).
8. 앱에는 "가"가 애초에 한 번도 나간 적이 없으므로(계속 preedit 상태였음) 삭제 없이 한자 문자만 커밋하면 끝 — **삭제+대체가 필요 없는 케이스**였기 때문에 지금까지 이 인프라가 없어도 동작했다.

### 3.2 후보 없을 때 특수문자 fallback
- `target_syllable`의 첫 글자가 초성(예: "ㄱ")이면 `crate::special_chars::search_by_choseong` 시도(candidates.rs:75-104) → 있으면 특수문자 팝업(`special_char_mode=true`, `PopupAction::ShowSpecial`), 없으면 `InputResult::consumed()`만(팝업 없음, candidates.rs:107-108).

### 3.3 즐겨찾기 토글
1. 팝업에서 스페이스(또는 지정 키) → `PopupKeyResult::ToggleBookmark(index)` → 호출자가 `toggle_hanja_bookmark(index)` 호출.
2. `HanjaBookmarkStore::toggle` 즉시 디스크 저장 → 사전에서 fresh search 재정렬 → `popup_state` 일괄 갱신 → `PopupAction::HanjaCandidatesReordered` 발행 → frontend가 후보/즐겨찾기/커서를 통째로 교체.

### 3.4 취소
- 팝업 Escape/포커스 상실 → `cancel_hanja()` → `hanja_mode=false` 등 정리 + **원래 preedit("가")까지 지운다**(korean_context.clear()) — POPUP_SPEC.md:240 "취소 시: CancelHanja() → preedit(원래 한글) 유지 → 팝업 닫기"라는 스펙 문서 서술과 코드가 어긋난다(코드는 preedit을 지움, 복원 안 함) — §7 미해결 질문 참조.

## 4. 이 기능을 위한 확장 지점 (어디를 어떻게, 위험도)

### 4.1 대상① "최근 커밋 음절 + 현재 preedit 결합" — 신규 상태 필요, 위험도 중
- **문제**: `start_hanja_conversion`(candidates.rs:23-29)이 `preedit_cache`의 **마지막 한 글자만** 본다. "대한민"이 Syllable 모드로 이미 앱에 커밋되어 나간 뒤 "국"만 preedit에 남은 상태에서는, 엔진 어디에도 "대한민"을 기억하는 곳이 없다(§2 "상태 필드 선언" 참조 — `commit_buffer`는 1회성).
- **확장안**: `InputEngine`에 신규 필드(예: `recent_committed_syllables: String` 또는 `VecDeque<char>`)를 추가해, Syllable 모드에서 매 음절 커밋 시점(`flush_preedit()` 호출부, press_key.rs:1102-1110 및 유사 커밋 경로들)마다 append. `start_hanja_conversion`이 `target`을 정할 때 "recent_committed + preedit_last_syllable"을 합쳐 **가장 긴 매치부터 축소하며(대한민국 → 한민국 → 민국 → 국) `hanja_dict.search()`를 시도**하는 방식(사전이 이미 임의 길이 키를 지원하므로 `search()` 자체는 그대로 재사용 가능, dict.rs:104-106).
  - 리셋 조건을 신중히 설계해야 함: 커서 이동, 포커스 전환(`reset()`, engine.rs:764), 언어 전환, 비-한글 입력, 일정 길이 초과, Backspace 등 — 잘못 리셋하지 않으면 완전히 무관한 문맥에서 엉뚱한 단어가 결합될 위험(예: "안녕하세요국"). **가장 큰 설계 위험 지점.**
  - Word/Smart 모드에서는 이미 `preedit_cache`(=`word_buffer`+`preedit`)가 다음절을 들고 있으므로 이 신규 버퍼가 필요 없고, 오히려 신규 로직이 `preedit_cache` 전체를 target 후보로 쓰도록 `start_hanja_conversion`의 target 결정 분기를 **commit_unit별로 분리**해야 한다(현재는 분기 없음 — candidates.rs 어디에도 `commit_unit` 참조 없음, 확인: `rg "commit_unit" src/input_engine/candidates.rs` → 매치 없음).
- **커밋측 위험(더 큼)**: 매치된 단어가 이미 부분적으로 앱에 커밋되어 있으므로(Syllable 모드), 한자 선택 시 **이미 나간 "대한민"을 지우고 "大韓民國"으로 교체**해야 한다. 이는 `select_hanja`의 기존 반환 채널(`Option<String>`, RPC 응답)만으로는 표현 불가 — "몇 글자를 지워야 하는지"를 함께 실어야 한다.
  - 재사용 대상: `typefix_convert`가 이미 쓰는 `(offset_from_cursor: i32, delete_chars: u32, replacement: String)` 3-튜플 패턴(surrounding.rs:178-182) + 이를 소비하는 host측 배선 — Linux `delete_surrounding_text` 시그널 + `commit_text` 시그널(unim-dbus/src/service.rs:2781, 2794-2799), Windows `comp_mgr.replace_surrounding(context, tid, delete_count, &replacement, "", comp_sink)`(unim-tsf/src/key_handler.rs:434 부근). `select_hanja`의 반환 타입을 `Option<(u32 /*delete_chars*/, String /*replacement*/)>`처럼 확장(또는 별도 API `select_hanja_word`)하면 XIM의 N+1 BS 백엔드까지 포함해 기존 배선을 그대로 태울 수 있다.
  - `InputResult`는 ABI 고정이라 건드리면 안 됨(§2 인용) — 이 정보는 **반환값 확장(`select_hanja`의 시그니처 변경/신규 함수) 또는 `PopupAction`류 out-of-band 필드**로 보내야 한다.

### 4.2 대상② "선택(selection)된 한글 단어" — 기존 selection 인프라 재사용, 위험도 낮음~중
- 이미 `surrounding_text`/`surrounding_cursor`/`surrounding_anchor`(engine.rs:182-186)와 `typefix_convert`(surrounding.rs:183-272)가 "선택 영역 읽기 → 변환 → 삭제+대체 3-튜플 반환"의 정확히 같은 패턴을 구현해 두었다.
- **확장안**: `typefix_convert`를 본뜬 신규 메서드(예: `start_hanja_conversion_from_selection(&mut self)`)를 `candidates.rs`에 추가:
  1. `surrounding_cursor != surrounding_anchor` 확인(선택 있음, typefix_convert 패턴 그대로, surrounding.rs:193-195).
  2. 선택 구간 텍스트 추출(surrounding.rs:197-199 동일 로직) → `word`.
  3. `hanja_dict.search(&word)` 직접 호출(다음절 키 그대로 지원, dict.rs:104) → 후보 있으면 대상①과 동일하게 `hanja_mode`/`hanja_candidates`/팝업 진입.
  4. 선택 시 커밋도 대상①과 동일하게 "삭제(선택 영역 전체, `delete_chars = word.chars().count()`)+한자 대체" 3-튜플이 필요.
- 위험: 진입 트리거를 무엇으로 할지가 미정(요구사항에 "앱에서 선택된 한글 단어"라고만 명시, 키 바인딩 없음) — 한자키를 이중 의미(현재 idle/조합중 분기에 "선택 있음"을 추가 분기로 끼워야 함, press_key.rs:219-238) 또는 별도 단축키 신설 중 선택 필요. `surrounding_text`가 비밀번호 필드에서는 항상 비어있음(surrounding.rs:71-76)이라 이 경로는 자동으로 비번 필드에서 무력화됨(안전).

### 4.3 팝업 9개/페이지·즐겨찾기 — 확장 불필요, 위험도 없음
- `HANJA_PAGE_SIZE = 9`(popup_keys.rs:79)로 이미 요구사항과 일치. `PopupState::new_hanja_with_top_row`/`replace_hanja_items`/bookmark flag 배선 모두 임의 길이 `target: String`을 그대로 받으므로(§2 PopupAction::ShowHanja 참조) **다음절 단어를 넣어도 팝업 레이어 자체는 코드 변경 없이 동작할 가능성이 높다** — 단, `PopupState::new_hanja_with_top_row`/`popup_state.rs`의 라벨 렌더링(헤더 `"「{target}」 → 한자"` 등, POPUP_SPEC.md:158)이 다음절 target도 자연스럽게 표시하는지는 실제 값 대입 검증 필요(§7).
- `HanjaBookmarkStore`는 이미 임의 길이 키(`String`)를 쓰므로 **스키마 변경 없이** 단어 단위 즐겨찾기를 저장할 수 있다(bookmark.rs:19-24). 단, 지금 즐겨찾기 UX는 "동일 음절이면 여러 문맥에서 재사용"이 전제(예: "한" 즐겨찾기는 "한"이 나오는 모든 팝업에 반영)인데, **단어 키("대한민국")는 문맥이 정확히 같은 4글자 조합일 때만 재사용되므로 재사용 빈도가 훨씬 낮아질 것** — 설계 시 UX 트레이드오프로 고려.

### 4.4 출력 형식 설정(漢字 / 한자(漢字) / 漢字(한자)) — 완전 신규, 위험도 낮음
- 확인: `rg "hanja.*format|OutputFormat|HanjaFormat" src/config.rs src/hanja/*.rs` → 매치 없음(없음, 확인: 위 rg 패턴). 완전히 새로 추가해야 하는 설정.
- 자연스러운 위치: `src/config.rs`의 `KoreanConfig` 또는 별도 `HanjaConfig` 섹션에 enum 추가(`CommitUnit`과 동일 패턴— config.rs:59-88 참조, `all()`/`display_name()`/serde 브리지 관례를 그대로 답습 가능).
- 적용 지점: `select_hanja`(및 신규 대상①②의 커밋 경로)가 최종 `replacement` 문자열을 만들 때 이 설정에 따라 `"漢字"` / `"한자(漢字)"` / `"漢字(한자)"` 형태로 포맷 — `HanjaEntry`가 이미 `hangul`+`hanja` 둘 다 갖고 있으므로 포맷팅에 필요한 데이터는 이미 존재(추가 조회 불필요).
- 3지점 싱크 규칙 주의(메모리 `feedback_config_3way_sync`): 엔진(`src/config.rs`)·GUI(`unim-gui-gtk`)·CLI(`unim-cli config ConfigKey`) 동시 반영 필수.

## 5. 지켜야 할 규칙 (AGENTS.md·POPUP_SPEC·주석에서 인용, file:line)

- **InputResult ABI 동결** — `src/input_engine/types.rs:60-61`: "`InputResult`(`repr(C)` + unim-capi ABI)에는 필드를 추가하지 않으며, `popup_pending_action` 선례와 동일한 out-of-band 드레인 패턴으로 ABI 를 보존한다." → 신규 데이터는 반드시 `PopupAction`류 out-of-band 채널이나 함수 반환값 확장으로.
- **비밀번호/PIN 필드 차단** — `src/input_engine/surrounding.rs:71-76` (surrounding_text 비저장) 및 `src/input_engine/candidates.rs`가 호출되는 `press_key.rs:212-217`의 `content_purpose.should_block_hangul()` 강제 영문 전환. 대상②(selection 기반)는 이 필드를 그대로 재사용하므로 **자동으로 안전**하지만, 대상①의 신규 "최근 커밋 음절 버퍼"는 **직접 이 게이트를 다시 적용해야 함**(신규 필드이므로 기존 가드가 자동으로 커버하지 않음 — 비밀번호 필드에서 직전 입력한 글자를 기억했다가 한자 팝업에 노출하면 정보 유출).
- **CommitUnit별 분기 부재를 그대로 물려받지 말 것** — 현재 `candidates.rs`는 `commit_unit`을 전혀 참조하지 않는다(확인: `rg "commit_unit" src/input_engine/candidates.rs` 매치 없음). 신규 로직이 Syllable/Word/Smart 세 모드에서 동일한 "최근 커밋 버퍼" 가정을 쓰면 Word/Smart 모드에서 이중 처리(이미 `preedit_cache`에 잡힌 걸 또 별도 버퍼로 잡음) 위험.
- **PopupState 리스트 교체는 "재정렬 후 새 후보로 selection을 보내지 않는다"** — `src/input_engine/types.rs:182-183` (`HanjaCandidatesReordered` 주석): "SelectHanja 인덱스 미스매치를 피하려면 frontend가 이 액션을 받기 전엔 새 후보로 selection을 보내지 않아야 한다." 신규 기능이 후보 목록을 동적으로 바꿀 때(예: prefix 축소 재검색) 동일 원칙 적용 필요.
- **POPUP_SPEC.md는 절대적 규격**(메모리 `feedback_popup_spec_absolute`) — 변경 시 사용자 승인 필수. §3(한자 팝업, 141-260행)이 그리드/키/색상/취소 동작까지 규정하므로, 다음절 단어 지원이 헤더 텍스트("「{target}」 → 한자")나 셀 렌더링에 영향을 주면 **문서 갱신도 반드시 승인받고 함께 진행**.
- **Config 3지점 동시 싱크**(메모리 `feedback_config_3way_sync`) — 출력 형식 설정 추가 시 엔진·GUI·CLI 동시 반영.

## 6. Windows 동등성 메모

- 코어(`src/hanja/*`, `src/input_engine/candidates.rs`, `types.rs`)는 Linux/Windows 공용 크레이트(`src/`) — **플랫폼 분기 없음**, 즉 이 조사 범위의 로직 변경은 자동으로 양 플랫폼에 적용된다.
- Windows 쪽 소비처: `unim-tsf/src/key_handler.rs:184` (`KeyCode::Hanja || KeyCode::F9` 감지) → 공유 `InputEngine`으로 위임. `unim-tsf/src/popup_ipc.rs`가 `PopupState`를 그대로 직렬화해 팝업 프로세스에 전달(테스트: popup_ipc.rs:1216-1267 `hanja_compact_empty_col_headers_and_meaning` 등) — 팝업 레이어도 공용.
- **삭제+대체 메커니즘의 Windows 대응**: Linux는 DBus `delete_surrounding_text` 시그널 + `commit_text` 시그널 조합(unim-dbus/src/service.rs:2781-2790)이지만, Windows TSF는 **네이티브 `comp_mgr.replace_surrounding(context, tid, delete_count, &replacement, "", comp_sink)`**(unim-tsf/src/key_handler.rs:434)로 이미 typefix가 이 API를 쓰고 있다 — 대상①②의 커밋 경로가 동일한 `(delete_count, replacement)` 형태로 결과를 만들면 **Windows측 배선은 이미 존재하는 함수 호출 하나로 끝날 가능성이 높다.** XIM(레거시 X11)만 네이티브 delete-surrounding이 없어 N+1 BS 합성 방식(`unim-frontends/xim/src/handler.rs:118,452,546,638,1068-1111,1256`)을 쓰므로, 세 프론트엔드(DBus/TSF/XIM) 모두가 "delete_chars + replacement" 공통 계약을 소비할 수 있게 반환값 설계를 통일하는 게 핵심.
- Windows는 `winword.exe` 등 `word_mode_apps`(engine.rs:709-718 근방, config.rs)에서 `CommitUnit::Word`가 기본 desired인 경우가 있어(Smart 게이트) 대상①의 "Syllable 모드 전용" 신규 버퍼 로직이 Windows 워드류 앱에서는 애초에 발동하지 않을 수 있음 — 설계 시 "이 기능은 Syllable 모드 한정"으로 명확히 스코프를 좁히거나, Word/Smart 모드 경로(=이미 `preedit_cache`에 다음절이 있음)도 함께 다뤄야 함을 명시할 것.

## 7. 미해결 질문

1. **CancelHanja 시 preedit 복원 여부 — 문서 vs 코드 불일치.** POPUP_SPEC.md:240은 "취소 시: preedit(원래 한글) 유지"라 하는데, `cancel_hanja()`(candidates.rs:266-275)는 `korean_context.clear()`+`preedit_cache.clear()`로 **원래 한글까지 지운다**. 어느 쪽이 맞는 의도인지, 신규 기능(다음절 취소)에서 "부분 복원"(예: "국"만 복원, "대한민"은 이미 지워졌으므로 복원 불가)을 어떻게 다룰지 확인 필요.
2. **"최근 커밋 음절" 버퍼의 정확한 리셋 트리거 목록.** Backspace, 화살표 이동, 마우스 클릭(커서 이동), 다른 앱으로 포커스 전환, 언어 전환, 일정 시간/글자 수 경과, IME 재시작 중 정확히 무엇을 리셋 조건으로 삼을지 요구사항에 명시 없음 — 잘못 설계하면 문맥 오염(엉뚱한 결합) 또는 반대로 사용성 저하(너무 쉽게 끊김) 위험.
3. **대상①과 대상②를 같은 한자키로 트리거할지, 별도 단축키를 둘지.** 현재 한자키는 "idle→이모지 / 조합중→한자"(press_key.rs:231-237) 2분기뿐. "선택 영역 있음" 3번째 분기를 추가할지, 선택이 있어도 조합 중이면 어느 쪽 우선인지 결정 필요.
4. **`select_hanja`/신규 API의 정확한 반환 시그니처.** `Option<(u32, String)>`로 확장할지, 완전히 새 DBus 메서드(`SelectHanjaWord`)를 신설해 기존 단음절 경로(현재 동작 그대로 보존)와 분리할지 — 기존 프론트엔드(GNOME extension, GTK popup-service, XIM, TSF)가 전부 `select_hanja` 반환 타입을 `(s hanja)`로 하드코딩하고 있어(POPUP_SPEC.md:101 DBus 시그니처) **기존 시그니처를 바꾸면 모든 프론트엔드 동시 수정 필요** — 별도 메서드 신설이 더 안전할 가능성.
5. **다음절 사전 커버리지의 실사용 적중률.** 27.5만 다음절 항목이 "정확히 그 글자수·그 조합"일 때만 매치되는 완전일치 사전이라(prefix/부분 매치 API 없음, dict.rs 확인), "대한민국"처럼 사전에 있는 단어는 되지만 사전에 없는 임의 조합(예: 신조어, 고유명사)은 매치 실패 → 이때 fallback UX(예: 마지막 음절만이라도 단독 후보 표출)를 어떻게 할지 요구사항에 없음.
6. **다음절 target에 대한 팝업 헤더/렌더링 실제 검증 미실시.** §4.3에서 "코드 변경 없이 동작할 가능성이 높다"고 썼으나 `PopupState::new_hanja_with_top_row` 내부의 라벨 폭 계산·말줄임 등이 4글자 이상 target에서 깨지지 않는지는 실제 실행/테스트로 확인하지 않았다(이 조사는 정적 코드 읽기만 수행).
7. **HanjaBookmarkStore 멀티 프로세스 동기화.** 여러 InputEngine 인스턴스(예: 여러 IM 컨텍스트/프로세스)가 각자 `load_default()`로 메모리 상태를 갖는데, 한쪽에서 `toggle()`로 저장한 파일을 다른 인스턴스가 재로드하는 경로가 코드상 없음(확인: `rg "load_default|reload" src/hanja/bookmark.rs` → `load_default` 정의 1건뿐, 재로드 API 없음) — 단어 단위 즐겨찾기가 늘어나면 이 불일치가 더 체감될 수 있음(기존에도 있던 제약이나, 신규 기능 설계 시 재확인 필요).

## 보충 #10: Word 모드에서 delete_chars=0·전체 치환이 성립하는 이유, 그리고 부분 커밋 API 부재

**결론: Word 모드에서는 delete_chars=0·target=word_buffer+preedit 전체 치환이 항상 맞다 — 단, "부분 일치 시 앞부분 먼저 커밋"은 현재 코드에 그런 API가 없고, 사실상 발생하지 않는 시나리오다.** 근거를 아래에 정리한다.

### (1) Word 모드에서 앱에 실제로 나간 글자는 정말 0이다 — `commit_buffer`가 비어있음을 코드로 확인
- `compose_and_route`(`src/hangul/input_context.rs:189-203`): 음절이 완성되면 `if self.accumulate_word { self.word_buffer.push(committed_char) } else { self.committed.push(committed_char) }` — Word 모드에서는 완성 음절이 `committed`가 아니라 `word_buffer`로 간다.
- `press_key.rs`에서 앱으로 나가는 유일한 경로는 `korean_context.get_committed()` → `self.commit_buffer.push_str(committed)`(예: press_key.rs:501-514, 1158-1163, 1250-1256, 1275 부근) — 이건 `korean_context.committed`만 읽는다. Word 모드에서 `committed`는 `korean_context.commit()`이 호출되기 전까지 계속 빈 문자열이다(유일한 호출부: `flush_preedit`, press_key.rs:1102-1109, `is_composing()`일 때만 발동).
- 즉 Word 모드에서 음절을 계속 입력하는 동안 `commit_buffer`는 채워지지 않는다 — TSF/Linux 프런트 모두 "커밋된 텍스트"로 앱에 보낸 게 실제로 없다. `preedit_cache = get_preedit_display() = word_buffer + preedit`(input_context.rs:332-341, engine.rs `update_preedit_cache` 경유)가 화면에 보이는 전부이며 이건 여전히 "조합 중" 상태다.
- 따라서 대상①이 한자키를 누른 시점에 `delete_chars=0`이고 target을 `preedit_cache` 전체(또는 그 안의 사전 매치 부분)로 잡는 건 **자연스러운 귀결**이다 — 지울 "이미 커밋된" 문자가 애초에 없다.

### (2) `select_hanja`의 `korean_context.clear()`는 부분이 아니라 전부를 지운다
- `select_hanja`(candidates.rs:140-158): `if !self.preedit_cache.is_empty() { self.korean_context.clear(); self.preedit_cache.clear(); }`
- `InputContext::clear()`(input_context.rs:391-397): `chord_input_order.clear(); composer.force_compose_korean(); preedit.clear(); committed.clear(); word_buffer.clear(); word_keys.clear();` — **`committed`까지 포함해 전부 폐기**한다. 이건 대상①(음절 확정 모드)이나 지금의 단음절 Word 모드 모두에서 "이미 앱에 나간 텍스트"를 건드리지 않는다는 전제 하에서만 안전한데, (1)에서 확인했듯 Word 모드 중엔 `committed`가 항상 비어 있으므로 이 `clear()`가 폐기하는 건 실제로 `word_buffer`+`preedit`뿐이다. 코드상 "부분 커밋" API는 존재하지 않는다 — `.commit(` 호출부는 `flush_preedit` 단 한 곳(press_key.rs:1104)뿐이고, 그마저 `is_composing()`이면 word_buffer+preedit 전체를 한 번에 `committed`로 합친다(input_context.rs:353-373). 접두사만 골라 커밋하는 함수는 grep으로 확인되지 않는다.

### (3) "오늘대한민국" 부분 일치 시나리오는 현재 word_buffer 축적 규칙상 사실상 발생하지 않는다
- `word_buffer`는 오직 `compose_and_route`(완성 음절 push)로만 자라고, 유일하게 비워지는 경로는 `commit()`(word 경계에서 전체를 `committed`로 flush, input_context.rs:359-368)과 `clear()`/`clear_composing()`류(전체 폐기)뿐이다 — "이미 지나간 단어만 커밋하고 새 단어는 word_buffer에 남기는" 부분 flush 로직이 없다.
- `commit()`을 부르는 `flush_preedit()`는 `is_composing()`일 때 통째로 호출되며, 스페이스·구두점 등 비-자모 키 입력 시 발동한다(주석상 실제 발동 지점은 apply_chord_entries의 NonJamo 분기 `self.flush_preedit()` 등). 따라서 "오늘"과 "대한민국" 사이에 스페이스나 구두점이 있었다면 그 시점에 이미 `flush_preedit`가 "오늘"을 `committed`→`commit_buffer`로 내보내고 `word_buffer`를 비웠을 것이므로, 한자키를 누르는 시점의 `word_buffer`는 이미 "대한민국"만 남아 있다 — 질문이 가정한 "word_buffer에 '오늘대한민국'이 공존"하는 상태 자체가, 그 사이에 flush를 유발하는 키(공백/구두점/비-자모)가 전혀 없었던 경우에만 성립한다.
- 그런 무경계 연속타이핑 케이스가 실제로 존재한다면, 현재 코드는 이를 분할 커밋할 수단이 없다 — 대상①의 최장 사전 일치 로직이 "대한민국"만 매치했을 때 "오늘"을 남겨서 먼저 커밋하려면 `InputContext`에 신규 API(예: `commit_prefix(n: usize)` — `word_buffer`를 앞쪽 n글자는 `committed`로 push, 나머지는 `word_buffer`에 유지)가 **새로 필요**하다. 이는 설계 단계의 갭이며 기존 코드에 준하는 부분이 없다.

### (4) TSF/Linux 프런트는 "전체 라이브 조합 치환" 모델이라 (1)-(3)과 정합적
- `end_composition_with_text`(unim-tsf/src/composition.rs:521-535)는 `EndCompositionEditSession { text: Some(text), composition }` — 활성 composition 전체를 새 텍스트로 바꾸고 세션을 끝낸다. **부분 range만 바꾸는 오버로드는 없다.** 즉 대상①이 target을 `preedit_cache` 전체로 잡아 이 함수 하나로 치환하는 건 기존 API로 바로 되지만, "오늘"은 커밋 텍스트로 남기고 "대한민국"만 composition으로 교체하는 건 이 함수로 못 한다(별도로 `context.InsertTextAtSelection` 류로 "오늘"을 먼저 텍스트로 박아넣고, 남은 걸로 새 composition을 시작해야 함 — 코드상 그런 시퀀스는 없음).
- Linux Phase A2(unim-dbus/src/engine_worker.rs:1580-1600 부근, `fix.replace_composition` 분기)도 동일한 모델이다: `all_keys`(전체 키스트로크)를 재생해 `engine.preedit_str()`(=word_buffer+preedit)을 통째로 만들고, `replay_commit=""`(전체가 단일 라이브 조합)로 프런트가 "보유 중인 라이브 조합을 그 문자열로 SetText 치환"하게 시그널한다(주석: "전체가 단일 라이브 조합"). 이 경로 역시 부분 교체 개념이 없고, AutoTypeFix 재사용을 노린 설계(요구사항의 "대체 입력은 AutoTypeFix 노하우 재사용")와 그대로 들어맞는다 — 다만 이건 "영→한 오타 교정"용으로 만들어진 전체-치환 경로이지, 대상①의 부분 매치 케이스를 지원하도록 확장된 것은 아니다.

**설계 함의**: 대상①을 Word 모드에서 구현하려면 (a) 최장 사전 일치가 `word_buffer+preedit` **전체**와 매치되는 케이스만 우선 지원(=코드 변경 없이 기존 `select_hanja`/`end_composition_with_text`/Phase A2 전체-치환 경로 그대로 재사용 가능), (b) 부분 매치(접두사 비매치) 케이스는 `InputContext`에 부분 커밋 API 신설 + TSF `InsertTextAtSelection`+새 composition 시작 시퀀스 + Linux DBus에 `commit_text`(비어있지 않은 접두사)+`preedit`(나머지) 동시 발행 조합이 모두 새로 필요하다. (b)는 회귀 위험이 있는 신규 경로이므로 스코프에 넣을지 별도 결정 필요.

## 보충 #2: hanja_target 을 "복원용 원문"으로 소비하는 곳 전수 조사 — (committed_prefix, preedit_part) 분리 시 고칠 호출부 범위

### 결론 먼저
질문에서 지목한 3개 후보(known 3경로 외) 중 **실제로 target을 재커밋에 쓰는 새 경로는 없다.** 다만 조사 중 기존 3경로와 별개로 **TSF에 이미 존재하는 4번째 사각지대**(신규 아님, 기존 버그)를 하나 발견했다 — 대상① 도입 시 반드시 같이 다뤄야 함.

### 질문에 나열된 3개 읽기 지점 — 전부 "표시/조회 키"이지 "복원 텍스트"가 아님
1. **popup_state 헤더 target** — `src/popup/view_model.rs`의 `vm.target`은 `popup_ipc.rs:59,288`(TSF) / GTK `header_text` 조립에 쓰이는 **표시 문자열**뿐이다. golden line(`popup_ipc.rs:1307`)에서도 `"header_text":"「ㄱ」 → 특수문자"` 형태로만 등장 — 재커밋 코드 경로 없음.
2. **`is_bookmarked(&self.hanja_target, &e.hanja)`** — `candidates.rs:171,193(x2),225`, 전부 **북마크 스토어 조회 키**(`src/hanja/bookmark.rs:57`)로만 쓰인다. 커밋 텍스트 조립과 무관.
3. **`HanjaCandidatesReordered.target`** (`candidates.rs:241`) — TSF `key_handler.rs:1078-1084`에서 `match`는 `bookmarked`/`was_bookmarked`만 꺼내 flash 신호로 쓰고 `target`은 `..`로 **버린다**. GNOME(`dbus_ime.js:646` 로그 라인)·`unim-gui-common/dbus_client.rs:646,658`도 로그 출력 + `GuiAction::HanjaCandidatesReordered` 페이로드 전달용일 뿐, 어디서도 이 target으로 CommitText를 만들지 않는다.

→ 이 3곳은 (committed_prefix, preedit_part) 분리와 **무관**하다. 표시/키 조회는 `target` 전체(또는 접두+preedit 합친 문자열)를 그대로 써도 되고, 필드를 쪼개도 `.format!("{prefix}{preedit}")` 한 줄이면 호환된다.

### TSF의 popup cancel/외부 취소 — 새 경로 아님, 기존 3경로 중 (popup_cancel)로 합류
- `key_handler.rs:1120,1209`(키보드) + `apply_reverse_event`의 "외부 취소" 주석(`key_handler.rs:1120`) 둘 다 **`engine.press_key(KeyCode::Escape, ...)`** 를 호출한다(in-process, TSF는 코어를 직접 링크). Escape → 코어 내부 → `popup_dispatch.rs:225-231`(popup_cancel) — 이미 알려진 3경로 중 하나와 **동일 함수**를 두 호출부(키보드/마우스)가 공유한다. TSF 전용 신규 소비처가 아니라 **호출 지점이 2곳으로 늘 뿐**이므로, `popup_dispatch.rs`의 popup_cancel 한 곳만 고치면 TSF 양쪽 다 해결됨.

### GNOME/unim-popup-service의 Escape 처리 — CancelHanja 반환값 `s`를 자체 커밋하지 않음
- `unim-gnome-extension/dbus_ime.js:739-741` `popupCancelHanja()`는 `this._callPopupService('CancelHanja', null)` — **반환값을 버린다(fire-and-forget)**. `unim-popup-service/src/dbus_server.rs:245-251` `cancel_hanja()`도 단순 forward(`proxy.cancel_hanja().await`)이고 반환값을 그대로 zbus 호출자에게 리턴할 뿐, popup-service 자신이 그 문자열로 뭔가를 커밋하지 않는다. 실제 커밋은 `unim-dbus/src/service.rs:3147-3168`의 `cancel_hanja()` RPC 핸들러 내부 `redirect_commit_and_hide(&preedit)`(`service.rs:1807-1845`, CommitText를 popup-owner IC로 직접 발행)가 전담 — 이미 질문이 지목한 "CancelHanja RPC(→service.rs:3147→redirect_commit_and_hide)" 경로와 동일하다. GNOME 쪽은 별도 소비처가 아님.

### 새로 발견된 사각지대 (기존 버그, 대상①과 별개로 존재) — TSF 의 bare `engine.reset()`
- 코어 `InputEngine::reset()`(`src/input_engine/engine.rs:761-782`)은 `hanja_target`/`hanja_candidates`/`popup_state`를 **커밋 없이 그냥 clear**한다(주석에도 "팝업 상태 초기화"라고만 적혀 있고 커밋 언급 없음). Linux D-Bus 프런트는 이 bare `reset()`을 절대 직접 안 부르고 항상 래퍼 `reset_engine_and_capture_commit`(`unim-dbus/src/engine_worker.rs:722-750`)을 거친다 — 거기서 `is_hanja_mode()`→`get_hanja_target()` 캡처 후 `cancel_hanja()`→커밋 순서를 명시적으로 밟는다.
- **TSF는 이런 래퍼가 없다.** `engine.reset()`을 bare로 직접 부르는 4곳: `unim-tsf/src/text_service.rs:1868`(OnSetFocus 워드게이트 재적용, popup 활성 여부 무관하게 무조건 실행), `unim-tsf/src/key_handler.rs:844`(ATF 역방향 보유영문 불일치 처리, `!popup_active` 게이트 안쪽이라 안전), `unim-tsf/src/auto_typefix.rs:455,520`(ATF 순방향/역방향 리셋, 이것도 호출부가 `key_handler.rs:764` `if !popup_active && atf_active`로 감싸여 있어 안전).
- 즉 **`key_handler.rs:844`·`auto_typefix.rs:455/520`은 이미 `!popup_active` 게이트로 보호되어 hanja_mode 중엔 도달 불가** — 문제 없음. 하지만 `text_service.rs:1868`(OnSetFocus)은 그런 게이트가 전혀 없다(1740-1878 구간 확인, popup 상태 체크 없음). 포커스가 다른 필드/창으로 넘어가는 순간(Alt-Tab 등) hanja 팝업이 떠 있어도 이 코드가 그대로 실행돼 `hanja_target`이 **커밋 없이 증발**한다 — 대상①이 새로 만드는 이중커밋 리스크와 정반대로, 이미 존재하는 "유실" 리스크다.
- **의미**: 대상① 설계로 hanja_target을 (committed_prefix, preedit_part)로 쪼개거나 `hanja_committed_len` 필드를 추가할 때, TSF 쪽에도 Linux의 `reset_engine_and_capture_commit`에 대응하는 **캡처 래퍼를 `text_service.rs:1868` 직전에 신설**해야 한다 — 그러지 않으면 대상① 도입 후 "대한민(커밋됨)+국(preedit)" 상태에서 Alt-Tab 시 "대한민국" 전체가 조용히 사라지는 회귀가 생긴다(이중 커밋이 아니라 완전 유실이라는 점에서 원 질문이 우려한 것과 반대 방향의 버그).

### 최종 답변 — 상태기계 변경(1) 시 고쳐야 할 호출부 목록
target 전체 재커밋 로직이 있는 곳(= (committed_prefix, preedit_part) 분리 시 반드시 수정):
1. `src/input_engine/popup_dispatch.rs:225-231` (popup_cancel) — TSF 키보드/마우스 Escape 둘 다 이 한 곳을 공유
2. `unim-dbus/src/service.rs:3147-3168` (`cancel_hanja` RPC → `redirect_commit_and_hide`) — GNOME/unim-popup-service의 CancelHanja는 전부 이 경로로 귀결
3. `unim-dbus/src/engine_worker.rs:722-750` (`reset_engine_and_capture_commit`) — Linux FocusOut/Reset 전 경로가 공유

표시/조회용이라 안 고쳐도 되는 곳: popup_state 헤더 target, `is_bookmarked` 키, `HanjaCandidatesReordered.target` (위 3곳).

추가로 **신설**해야 하는 곳(기존 버그 수정, 대상① 전제조건): `unim-tsf/src/text_service.rs:1868` 직전에 Linux `reset_engine_and_capture_commit` 상당의 캡처+커밋 로직을 넣는 것 — 이건 "고쳐야 할 기존 소비처"가 아니라 "TSF에 없어서 새로 만들어야 하는 소비처"다.

## 보충 #3: commit_buffer push/drain 지점 분류 — '최근 커밋 음절 버퍼' 훅은 어디에

### 1. press_key.rs `commit_buffer.push*` 17곳 분류

**한글 음절 확정(한자 후보의 재료가 되는 것)** — 6곳:
- `510`(`get_committed()` 결과, chord OFF 즉시 처리), `1160`(`update_chord_preview` 단일키 Jamo 브랜치), `1252`(`apply_chord_entries` 1키 Jamo 브랜치), `1277`(`apply_chord_entries` 2+키 실패 시 sequential 재생 브랜치) — 넷 다 `korean_context.process_jamo_with_meta()` 직후 `get_committed()`→`push_str(committed)`→`clear_committed()` 3단 패턴이며, `flush_preedit()`(1102-1108, `korean_context.commit()`→`get_committed()`→`push_str`→`clear()`)도 동일 계열. 이 5곳(510,1102-1108의 flush_preedit,1160,1252,1277)이 "한글 음절이 방금 확정됐다"는 의미가 살아있는 지점.

**한글이 아니거나 음절이 아닌 것** — 11곳:
- `291` auto-english trigger commit(영문 char, `AutoEnglishTrigger::Functional/Character`) — 라인 264-291 참조, 영문 전환과 함께 발생.
- `367` Space(`' '`, process_korean_key 내 — Space는 한글 모드에서도 그냥 공백 커밋).
- `407` `alt.fallback`(key_meta `context_alt` 불충족 시 fallback 리터럴, 임의 문자열 — 자모 조합 결과 아님).
- `426` Special 자모(`JamoEnum::Special`, 특수문자 — 라인 421-428).
- `592` "자모가 아닌 문자(기호 등)" chord-OFF 즉시 commit(라인 587-593 주석 "자모가 아닌 문자(기호 등)").
- `623` Space(`process_english_key`, 완전히 영문 모드).
- `645` 영문 키맵 char(`process_english_key` 최종 분기).
- `1169`/`1258` `ChordEntryKind::NonJamo(c)` 단일 비자모 즉시 commit.
- `1313`/`1331` `apply_chord_result` Case B/C의 `fallback_jamos`(**호환 자모 낱개**, 예: ㄱ/ㅏ — 결합 실패 시 분해된 자모라 완성 음절이 아님. 완성 음절과 혼동 주의).
- `1336` Case C의 `non_jamos`(비자모 원본 char).

→ 결론: 17곳 중 **완성된 한글 음절**이 commit_buffer에 들어가는 지점은 정확히 5곳(510, flush_preedit 내부 1곳, 1160, 1252, 1277)이고 나머지 12곳은 영문/공백/특수문자/기호/분해자모다. 이 5곳 모두 공통 서브패턴(`korean_context.get_committed()` 읽고 즉시 `clear_committed()`/`clear()`)을 공유하므로, "한글 음절 확정" 이벤트는 이 3-스텝 패턴이 나타나는 지점으로 **정확히 특정 가능**하다.

### 2. popup_dispatch.rs 5곳 + mod.rs 1곳

- `popup_dispatch.rs:194` `select_hanja()` 결과 한자 문자열 commit(팝업에서 한자 선택 확정).
- `popup_dispatch.rs:201` `select_special_char()` 결과 특수문자 commit.
- `popup_dispatch.rs:214` 이모지 선택 commit(`emoji_at_global_index`).
- `popup_dispatch.rs:228` `popup_cancel()`의 한자 취소 시 `hanja_target`(원래 한글) 복원 commit — **여기가 §7 미해결질문①의 "취소 시 preedit 복원" 갈림길**: 이건 `hanja_target`을 그대로 다시 commit하는 것이라 원본 텍스트가 살아나긴 하지만, 이미 `cancel_hanja()`가 `korean_context.clear()`로 조합 상태를 지운 뒤라 **preedit이 아니라 commit_buffer로 되돌아간다**(즉 "미리보기 중 조합"이 아니라 "확정 텍스트"로 복귀 — 문서(POPUP_SPEC.md:240 "preedit 유지")와 다시 어긋남, §7-1 재확인).
- `popup_dispatch.rs:233` 특수문자 취소 시 `special_char_target` 복원 commit(동일 패턴).
- `mod.rs:161` 유닛테스트 내부에서 `engine.commit_buffer.push_str("test")` 직접 조작 — 프로덕션 경로 아님, 테스트 하네스 전용.

→ 5곳(194,201,214,228,233) 전부 "이미 결정된 결과 문자열을 그대로 commit"하는 종단 지점이라, 이들 다음에 "한글 음절"이라는 의미는 없음(한자/특수문자/이모지거나, 취소 시 원문 재주입) — 이 신규 '최근 커밋 음절 버퍼' 관점에서는 **리셋 트리거**로 다뤄야 할 지점들(한자/특수문자/이모지가 실제로 커밋되면 직전에 쌓아온 "최근 한글 음절" 문맥은 끊어져야 자연스러움, §7-2 리셋 목록에 추가 후보).

### 3. drain 측 — commit_str()/clear_commit() 페어링 실태

- `engine.rs:721-723` `commit_str(&self) -> &str`(읽기 전용, 비파괴) / `engine.rs:731-733` `clear_commit(&mut self)`(비움) — 별개 pub 메서드. **엔진 자체는 페어링을 강제하지 않는다** — 호출자 책임.
- `engine.rs:845-858` `chord_idle_flush_pending()` — `mem::take(&mut self.commit_buffer)`로 **읽음과 동시에 비움**(원자적) — 질문에서 말한 "drain 지점 ②"가 맞음. idle 타임아웃 시 chord flush 결과를 내보내는 전용 경로.
- **Rust 네이티브 소비자는 전부 같은 함수 내에서 즉시 페어링**: `unim-dbus/src/engine_worker.rs:333-339` `drain_commit()`(commit_str→비었으면 None, 아니면 clear_commit→Some(s)), `unim-tsf/src/key_handler.rs:615-616`,`674-675`,`1221-1223`(3곳 모두 commit_str 직후 clear_commit), `unim-imm32/src/input.rs:210-211`(commit_changed일 때만 read+clear), `unim-imm32/src/lib.rs:305-307`(CPS_COMPLETE 처리, clear_preedit→commit_str→clear_commit). **이 5개 Rust 프론트엔드 경로는 예외 없이 같은 함수 스코프 안에서 짝을 이룬다** — drain 측에 훅을 걸어도 "매 프레임 정확히 한 번, 텍스트 종류 무관하게" 실행이 보장됨.
- **예외: `unim-capi`(C ABI)** — `unim-capi/src/lib.rs:196-199`(`unim_engine_commit_str`)와 `:322-325`(`unim_engine_clear_commit`)가 **서로 독립된 `extern "C"` 함수**로 노출되어 있어, C쪽 호출자가 read 후 clear를 호출할지 여부는 Rust 코드가 보장하지 못한다. 다만 조사 결과 이 C ABI를 실제로 소비하는 프로덕션 프론트엔드는 발견되지 않음(`rg "unim_capi|libunim_capi"` → `unim-capi/Cargo.toml` 자체와 `examples/capi-c/minimal_session.c`뿐; GTK는 `unim-frontends/gtk-common/src/unim_dbus_client.c`로 DBus를 타지 C ABI를 안 쓴다, XIM도 `unim-frontends/xim/src/dbus_client.rs`로 DBus 경유). 즉 현재는 이론적 갭이고 실사용 경로는 전부 페어링됨.

→ 결론: drain 측은 사실상 "commit_str 읽고 바로 clear" 지점이 **다수(5개 프론트엔드 × 여러 호출부)** 흩어져 있고, 이들 각각이 텍스트 종류(한글 음절/영문/특수문자/한자/이모지)를 구분하지 못한 채 동일하게 소비한다 — 질문에서 예상한 "drain은 단일 지점"이라는 전제와 달리 **drain은 다중 지점**이며, 여기 훅을 걸면 텍스트 종류별 분기 로직을 프론트엔드마다 새로 심어야 한다(구분 정보가 이 시점엔 이미 사라진 뒤).

### 4. `korean_context.get_committed()` — Syllable 모드 정확히 1음절인가

**예, 확인됨.** 근거:
- `src/hangul/input_context.rs:347-348` `get_committed(&self) -> &str { &self.committed }` — 단순 필드 참조, 누적 여부는 `committed` 필드가 어떻게 채워지는지에 달림.
- `input_context.rs:201` `self.committed.push(committed_char)` — 컴포저가 완성 음절 1개를 낼 때마다 **문자 1개씩만** push(word_buffer 미사용 시).
- 테스트 `input_context.rs:1089-1095`(`syllable_vs_word_mode_committed_semantics` 유사 테스트) — "syllable 모드(기본): '가나' 입력 시 첫 음절 '가'는 committed, '나'는 preedit" → `assert_eq!(syl.get_committed(), "가")`, 즉 두 번째 음절이 진행 중이면 `committed`엔 **직전에 확정된 정확히 1음절만** 남는다(두 번째 음절은 아직 preedit).
- 단, 이는 "그 시점까지 `clear_committed()`가 호출되지 않았다"는 전제하의 값이다 — press_key.rs의 모든 실사용 경로(§1의 5곳)는 `get_committed()` 직후 즉시 `clear_committed()`(또는 `clear()`)를 호출하므로, 실제 엔진 동작에서는 "한 번의 read 시점당 최대 1음절"이 항상 보장된다(두 음절이 쌓인 채로 read되는 코드 경로 없음, §1 push 지점 전수 확인).

### 5. 설계 함의 (push vs drain 선택)

- **push 측(§1의 5개 "한글 음절 확정" 지점)에 훅을 두는 편이 명확히 유리**: (a) "이게 한글 음절이다"라는 타입 정보가 그 지점에 이미 있어 신규 버퍼 append 시 별도 판별 불필요, (b) 나머지 12개 push 지점(영문/공백/특수문자/기호/분해자모) 및 popup_dispatch.rs 5곳(한자/특수문자/이모지/취소복원)이 **자연스러운 리셋 지점 후보 목록**으로 그대로 쓰인다(어절 경계 판정 로직을 이미 존재하는 분기 구조 위에 얹으면 됨) — 코드가 push 시점마다 "이건 syllable" vs "이건 아님"을 이미 구분해 놓았으므로 새 판별 로직이 필요 없다.
- **drain 측(commit_str/clear_commit)에 훅을 두면 불리**: §3에서 확인했듯 drain 지점은 5개 프론트엔드에 흩어져 있고(단일 지점 아님), 그 시점엔 이미 여러 push가 뒤섞여 `commit_buffer`에 문자열이 합쳐진 뒤라(예: 음절+공백이 한 번의 drain에 같이 나갈 수 있음, engine.rs:367 Space도 같은 buffer) "이 문자열 중 어디까지가 한글 음절이었는지"를 사후에 재구성해야 하는 부담이 생긴다.
- **유닛테스트 함의**: 신규 버퍼를 push 측(예: 5곳 각각에 `self.recent_committed.push_str(committed)` 유사 훅 추가)에 걸면, 테스트는 `clear_commit()`을 호출할 필요가 **없다** — `commit_buffer`와 신규 버퍼는 독립 필드이므로 `commit_str()`/`clear_commit()` 호출 여부와 무관하게 신규 버퍼 상태를 직접 assert 가능(예: `engine.recent_committed_syllables` 같은 필드를 두면 됨). 다만 리셋 조건(§7-2, popup_dispatch 5곳 포함)에 대한 테스트는 `clear_commit()`이 아니라 **신규 버퍼 전용 clear 메서드** 호출/미호출을 검증해야 한다.
