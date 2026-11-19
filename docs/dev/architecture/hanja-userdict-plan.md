# 한자 사용자 사전 구현 계획

> 규격: [`docs/dev/specs/HANJA_USERDICT_SPEC.md`](../specs/HANJA_USERDICT_SPEC.md) (이하 SPEC). 기준 커밋 develop `c43c7dc`.
> 상태: **계획 — 기현님 승인 대기.** 커밋·푸시는 기현님 지시 때만.
> 담당 모델(프로젝트 정책): 구현 sonnet · 검증 opus.

> **기현님 결정(2026-11-16)**: SPEC §10 — 팝업 명세 보강 승인, 새 페이지 「한자 사용자 사전」, ATF 표시 문구 「자동 교정 사전」으로 개명(파일·CLI·설정 키 그대로), 0.4.5 에 싣는다. 이 계획대로 구현 승인.

## 0. 작업 묶음과 순서

```
WP1a 코어 파일 모델·검증·인덱스·캐시 (API 고정) ──┬─▶ WP1b 엔진 조회 통합
                                                  ├─▶ WP2 CLI
                                                  ├─▶ WP3 GTK 페이지
                                                  └─▶ WP4 Slint 페이지
WP5 문서·i18n 문안 — 언제든 병행 (WP2~4 의 키 이름은 아래 §1 고정값 사용)
```

- WP1a 는 작고(파일 1개 + mod 1줄) 아래 §1 시그니처를 그대로 만든다. **WP1a 머지 뒤** WP1b·2·3·4 를 병렬로 돌린다(서로 파일이 겹치지 않음).
- 데몬(`unim-dbus`)·TSF(`unim-tsf`)·IMM32·capi·GNOME 확장은 **수정 없음**(SPEC D2 — 엔진이 직접 감지). config 키 추가 없음 → Config 3지점 싱크 대상 아님.

## 1. 고정 API 계약 (WP1a 가 만들고 나머지가 쓴다)

`src/hanja/user_dict.rs` (신규), `src/hanja/mod.rs` 에 `pub mod user_dict;` + `pub use user_dict::{HanjaUserDict, HanjaUserEntry, HanjaUserIndex, EntryError, LoadError};`

```rust
pub const USERDICT_VERSION: u8 = 1;
pub const MEANING_MAX_CHARS: usize = 60;
// 한글 상한은 crate::input_engine::HANJA_MAX_KEY_CHARS(=18, types.rs:319, mod.rs 에서 pub use) 재사용

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HanjaUserEntry {
    pub hangul: String,
    pub hanja: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meaning: Option<String>,
    #[serde(default)]
    pub added_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HanjaUserDict {
    #[serde(default = "default_version")] pub version: u8,
    #[serde(default)] pub entries: Vec<HanjaUserEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryError {
    HangulEmpty, HangulNotSyllable { ch: char, pos: usize }, HangulTooLong { len: usize },
    HanjaLenMismatch { hangul: usize, hanja: usize }, HanjaNotHan { ch: char, pos: usize },
    MeaningTooLong { len: usize }, MeaningControl, Duplicate, NotFound,
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse { line: Option<usize>, column: Option<usize>, message: String }, // message 는 화면용, 로그 금지
    NewerVersion(u8),
}

impl HanjaUserDict {
    pub fn default_path() -> Option<PathBuf>;                     // paths::config_dir()/unim/hanja-userdict.yaml
    pub fn load(path: &Path) -> Result<Self, LoadError>;           // 없는 파일 → Ok(빈 사전)
    pub fn load_default() -> Result<Self, LoadError>;
    pub fn save(&self, path: &Path) -> std::io::Result<()>;        // atomic_io::atomic_write, version>1 이면 Err
    pub fn save_default(&self) -> std::io::Result<()>;
    pub fn validate(hangul: &str, hanja: &str, meaning: Option<&str>)
        -> Result<(String, String, Option<String>), EntryError>;  // 정규화(trim)된 값 반환, NFC 금지
    pub fn entry_error(&self, idx: usize) -> Option<EntryError>;   // 저장된 항목 유효성(목록 ⚠ 표시용)
    pub fn find(&self, hangul: &str, hanja: &str) -> Option<usize>;
    pub fn add(&mut self, hangul: &str, hanja: &str, meaning: Option<String>) -> Result<usize, EntryError>;
    pub fn update_at(&mut self, idx: usize, hangul: &str, hanja: &str, meaning: Option<String>)
        -> Result<(), EntryError>;                                 // 위치·added_at 유지, 자기 제외 중복 검사
    pub fn remove(&mut self, hangul: &str, hanja: Option<&str>) -> usize; // 지운 개수
    pub fn remove_at(&mut self, idx: usize) -> Option<HanjaUserEntry>;
    pub fn quarantine(path: &Path) -> std::io::Result<PathBuf>;    // → <path>.broken-YYYYMMDD-HHMMSS
}

pub struct HanjaUserIndex { /* HashMap<String, Vec<HanjaEntry>>, valid, invalid */ }
impl HanjaUserIndex {
    pub fn empty() -> Self;
    pub fn from_dict(d: &HanjaUserDict) -> Self;   // 유효 항목만, 파일 순서
    pub fn get(&self, hangul: &str) -> &[HanjaEntry];
    pub fn contains(&self, hangul: &str) -> bool;
    pub fn valid_count(&self) -> usize;
    pub fn invalid_count(&self) -> usize;
}

/// 프로세스 전역 캐시: stat → 서명 (mtime, len) 비교 → 바뀌었으면 재파싱. 패닉 금지.
pub fn shared_index() -> Arc<HanjaUserIndex>;
/// 시험·타 크레이트 시험용. Some 이면 파일을 보지 않고 이것만 돌려준다.
#[doc(hidden)] pub fn set_shared_override(idx: Option<Arc<HanjaUserIndex>>);

/// SPEC §3.1 병합: 사용자(순서) → 내장(사용자와 같은 한자 제외). 사용자 뜻 None 이면 내장 원본 뜻.
pub fn merge_candidates(user: &[HanjaEntry], builtin: Vec<HanjaEntry>) -> Vec<HanjaEntry>;
```

캐시 내부(`struct UserLayerCache { path, sig: Option<(SystemTime, u64)>, index: Arc<_>, warned_sig }` + `static OnceLock<Mutex<_>>`)는 경로를 인자로 받는 생성자로 단위 시험한다. `#[cfg(test)]` 에서 `shared_index()` 는 override 가 없으면 **빈 인덱스**(개발자 실제 `~/.config` 차단).

## 2. WP1a — 코어 파일 모델·검증·인덱스·캐시

| 파일 | 할 일 |
|---|---|
| `src/hanja/user_dict.rs` (신규) | §1 전부. 구조는 `src/typefix_userdict.rs` 본보기(`now_unix`·`get_mtime`·`save_to_path`→`atomic_write`). 차이: 로드 실패를 `Result` 로 올림, 서명 `(mtime,len)`·`!=` 비교, 파일 삭제 시 빈 층, 파싱 실패 로그는 `e.location()` 의 행·열만(`unim_log!("ENGINE", …)`), 무효 항목 보존 |
| `src/hanja/mod.rs` | `pub mod user_dict;` + re-export |
| `Cargo.toml`(core) | 의존성 추가 없음(serde_yaml 0.9 기존) |

시험(같은 파일 `mod tests`): SPEC §9 코어 단위 목록. 임시 경로는 `typefix_userdict.rs::tests::unique_temp_path` 방식.

검증: `cargo test -p unim --lib hanja::user_dict` · `cargo clippy -p unim --lib` · `make check-windows`(코어는 TSF·IMM32 가 링크).

## 3. WP1b — 엔진 조회 통합 (전 조회 지점 교체)

### 3.1 새 필드·헬퍼

| 파일:줄 | 변경 |
|---|---|
| `src/input_engine/engine.rs:131` 근처 | 필드 `pub(super) hanja_user: Arc<crate::hanja::HanjaUserIndex>`, `#[cfg(test)] pub(super) hanja_user_pinned: bool` |
| `engine.rs:357-376` (생성자) | `hanja_user: crate::hanja::user_dict::shared_index()` (내장 `SHARED_HANJA_DICT` 는 그대로) |
| `engine.rs:1180` 주석 | "보존" 목록에 `hanja_user` 추가(재구성 경로에서 건드리지 않음) |
| `src/input_engine/hanja_lookup.rs` (신규) + `mod.rs` 에 `mod hanja_lookup;` | `hanja_search(&self,&str)->Vec<HanjaEntry>` = `merge_candidates(self.hanja_user.get(k), self.hanja_dict.search(k))`; `hanja_contains(&self,&str)->bool` = `self.hanja_user.contains(k) \|\| self.hanja_dict.contains(k)`; `refresh_hanja_user_layer(&mut self)`(pinned 면 무시); `#[cfg(test)] pin_hanja_user_layer(&mut self, Arc<_>)` |

### 3.2 조회 지점 — 전수 (develop `c43c7dc` 기준)

| # | 위치 | 현재 | 바꿀 것 | 대상 |
|---|---|---|---|---|
| 1 | `src/input_engine/candidates.rs:36` 앞 (`start_hanja_conversion`, `if self.hanja_mode {…}` 블록 뒤) | — | `self.refresh_hanja_user_layer();` **추가** (유일한 갱신 지점, SPEC §3.3) | 전부 |
| 2 | `candidates.rs:46` | `hanja_dict.search(&first.key)` | `hanja_search` | ③ 첫 후보 |
| 3 | `candidates.rs:67` | `hanja_dict.search(&spec.key)` | `hanja_search` | 단음절·①·② |
| 4 | `candidates.rs:169` | `hanja_dict.display_meaning(e)` | **유지**(내장 글자 역색인으로 사용자 항목 뜻도 합성) | 표시 |
| 5 | `candidates.rs:285` (`toggle_hanja_bookmark`) | `hanja_dict.search(&target)` | `hanja_search` — **안 바꾸면 ★ 토글 때 사용자 항목이 사라진다** | ★ |
| 6 | `src/input_engine/hanja_word.rs:279` (`resolve_hanja_target`) | `hanja_dict.contains(&key)` | `hanja_contains` | ① 접미 |
| 7 | `hanja_word.rs:482` (`cycle_surrounding_word`) | `hanja_dict.search(&c.key)` | `hanja_search` | ③ 재타 |
| 8 | `hanja_word.rs:547` (`selection_target`) | `hanja_dict.contains(&key)` | `hanja_contains` | ② |
| 9 | `hanja_word.rs:593` (`shrink_hanja_target`) | `hanja_dict.contains(&new_key)` | `hanja_contains` | ① 축소 |
| 10 | `hanja_word.rs:596` | `hanja_dict.search(&new_key)` | `hanja_search` | ① 축소 |
| 11 | `src/input_engine/surrounding_word.rs:774` (`surrounding_word_judge`) | `hanja_dict.contains(&key)` | `hanja_contains` | ③ 판정 |

- 이 밖에 `hanja_dict` 를 쓰는 곳은 생성(`engine.rs:357-361`)과 시험(`tests_hanja_word.rs:1332` 공유 확인)뿐이다. `unim-dbus`·`unim-tsf`·`unim-imm32`·`unim-capi`·GNOME 확장에는 사전 직접 조회가 없다(모두 `InputEngine` 경유). 교체 후 `rg -n 'hanja_dict\.(search|contains)' src` 결과가 0건이어야 한다.
- 후보 로그(`candidates.rs:127-132`)에 사용자 항목 수 `사용자 {}개` 수치 추가(평문 금지).
- `poll_deferred_hanja`(`surrounding_word.rs:533`)·`resume_deferred_hanja` 는 원래 한자키에서 갱신된 스냅샷을 쓰고 이어서 `start_hanja_conversion` 을 부르므로 추가 갱신 불필요.

### 3.3 시험

- 신규 `src/input_engine/tests_hanja_userdict.rs` + `mod.rs` 에 `#[cfg(test)] mod tests_hanja_userdict;`. 엔진 생성은 `test_helpers.rs`·`tests_hanja_word.rs` 의 기존 헬퍼(`type_keys`, `setup_recent_word_pending` 등) 재사용, 사용자 층은 `pin_hanja_user_layer`.
- 사례: SPEC §9 엔진 목록. 대표 기대값 — 사용자 `서기현→徐基鉉`, 음절 모드 `서기`+`현` 한자키 → target `서기현`, `candidates[0]=徐基鉉`; ★ 토글 뒤에도 `徐基鉉` 존재; 사용자 `현→鉉`(내장 중복) → 1회만, 0번 자리.
- 무회귀: 기존 시험 전부 무수정 통과(사용자 층 기본 빈 값).

검증: `cargo test -p unim --lib input_engine` · `cargo test --workspace` · `make check-windows` · `make build`(경고 0).

## 4. WP2 — CLI (`unim-cli dict hanja`)

| 파일:줄 | 변경 |
|---|---|
| `unim-cli/src/main.rs:273-293` `enum DictCommands` | 변형 `Hanja { #[command(subcommand)] action: HanjaDictCommand }` (about/long_about = `h("help_hd_about")`/`h("help_hd_long")`) |
| 같은 파일, `UserDictCommand`(:297) 옆 | `enum HanjaDictCommand { List{hangul: Option<String>}, Add{hangul, hanja, #[arg(long)] meaning: Option<String>}, Edit{hangul, hanja, #[arg(long = "hangul")] new_hangul: Option<String>, #[arg(long = "hanja")] new_hanja: Option<String>, #[arg(long)] meaning: Option<String>}, Remove{hangul, hanja: Option<String>, #[arg(short='y', long)] yes: bool}, Clear, Path }` — 위치 인자(현재 쌍)와 옵션(새 값)은 필드 id 가 달라 clap 충돌 없음 |
| `handle_dict`(:2209) | `Some(DictCommands::Hanja{action}) => dispatch_hanja_dict(action)` |
| 신규 함수(`dispatch_user_dict`:2219 아래) | `dispatch_hanja_dict`, `hanja_dict_list/add/edit/remove/clear/path`. 쓰기 = `HanjaUserDict::load_default()` → 수정 → `save_default()`. `LoadError::Parse` 면 쓰기 거부(메시지 SPEC §5), `clear` 만 `quarantine` 후 진행. 다수 제거는 기존 `confirm_multi_removal`(:2250대) 재사용. `EntryError` → i18n 문장 매핑 함수 하나(`hanja_entry_error_msg`) |
| `unim-cli/locales/ko.yml`·`en.yml` | `dict_no_subcommand_hint`(:114)에 `hanja` 추가. 신규 키: `hanja_dict_list_title`, `hanja_dict_empty`, `hanja_dict_added`, `hanja_dict_duplicate`, `hanja_dict_edited`, `hanja_dict_edit_nothing`, `hanja_dict_removed`, `hanja_dict_not_found`, `hanja_dict_cleared`, `hanja_dict_clear_confirm`, `hanja_dict_quarantined`, `hanja_dict_corrupt`, `hanja_dict_newer_version`, `hanja_dict_err_hangul_empty`, `…_hangul_not_syllable`, `…_hangul_too_long`, `…_hanja_len`, `…_hanja_not_han`, `…_meaning_too_long`, `…_meaning_control`, `hanja_dict_invalid_mark`; 도움말 `help_hd_about/long`, `help_hd_{list,add,edit,remove,clear,path}_{about,long}`, 인자 `help_hd_arg_{hangul,hanja,meaning,new_hangul,new_hanja}_{short,long}`. ko/en 키 집합 동일 |

시험: CLI 에 시험 디렉터리가 없으면 `main.rs` 안 `#[cfg(test)]` 로 `hanja_entry_error_msg` 매핑·출력 서식 함수 단위 시험. 수동:

```bash
D=$(mktemp -d); export UNIM_CONFIG_DIR=$D
cargo run -q -p unim-cli -- dict hanja add 서기현 徐基鉉 --meaning '내 이름'; echo $?   # 0
cargo run -q -p unim-cli -- dict hanja add 서기현 徐基鉉; echo $?                       # 1 중복
cargo run -q -p unim-cli -- dict hanja add 서기현 徐基; echo $?                          # 1 글자 수
cargo run -q -p unim-cli -- dict hanja edit 서기현 徐基鉉 --meaning ''
cargo run -q -p unim-cli -- dict hanja list; cat $D/unim/hanja-userdict.yaml
echo 'entries: [' > $D/unim/hanja-userdict.yaml; cargo run -q -p unim-cli -- dict hanja add 가 家; echo $?  # 1, 파일 그대로
```

검증: `cargo build -p unim-cli` · `cargo test -p unim-cli` · 위 수동 절차.

## 5. WP3 — GTK 설정 페이지 (`unim-settings-gtk`)

| 파일:줄 | 변경 |
|---|---|
| `unim-settings-gtk/src/settings_dialog.rs:140-141` | ATF 사용자 사전 페이지 다음에 `let page_hanja = build_hanja_dict_page(&window); window.add(&page_hanja);` |
| 같은 파일 끝(본보기 :2001-2282 `build_userdict_page`·`refill_userdict_group`·`show_userdict_edit_dialog`·`userdict_file_changed`) | 신규 `HanjaDictPageRefs`, `build_hanja_dict_page`, `refill_hanja_dict_group`, `show_hanja_dict_edit_dialog(refs, Option<(String,String)>)`, `hanja_dict_file_changed`. 차이점: ① 행 조작은 `(한글,한자)` 로 `find` ② 행 activatable(클릭=편집) ③ 대화상자 실시간 검증(`HanjaUserDict::validate`)·오류 라벨·저장 버튼 sensitive 토글 ④ `EntryRow` `entry-activated`: 한글→한자 포커스, 한자/뜻→저장 ⑤ 삭제 = 즉시 저장 + `adw::Toast` "되돌리기"(5초, 지운 항목을 원래 인덱스로 재삽입 — 그동안 파일이 바뀌었으면 끝에 추가) ⑥ refill 전후 `PreferencesPage` 의 스크롤 조정값 보존·같은 자리 행에 `grab_focus` ⑦ `LoadError::Parse` 면 오류 행 + [보관하고 새로 시작](`quarantine`) |
| `show_toast`(:219) | 버튼 달린 변형 `show_toast_with_action(text, label, closure)` 추가(기존 호출 무변경) |
| `unim-settings-gtk/locales/ko.yml`·`en.yml` | `page_hanja_dict_title`, `hanja_dict_group_title`, `hanja_dict_group_desc_count`, `hanja_dict_empty(_subtitle)`, `hanja_dict_btn_add_tooltip`, `hanja_dict_btn_delete`, `hanja_dict_dialog_{add,edit}_title`, `hanja_dict_field_{hangul,hanja,meaning}`, `hanja_dict_field_hanja_hint`, `hanja_dict_meaning_auto`, `hanja_dict_invalid_prefix`, `hanja_dict_err_*`(CLI 와 같은 7종), `hanja_dict_corrupt`, `hanja_dict_btn_quarantine`, `toast_undo` |
| `unim_log!` | 개수만(`[Settings] hanja-userdict 저장 실패: {io 오류 종류}` — 내용·serde 문장 금지) |

검증: `cargo build -p unim-settings-gtk`(경고 0) · `UNIM_CONFIG_DIR=$(mktemp -d) cargo run -p unim-settings-gtk` 로 추가·편집·삭제·되돌리기·깨진 파일·CLI 동시 변경(2초 내 목록 갱신) 확인 · 키보드만으로 추가 완료되는지.

## 6. WP4 — Slint 설정 페이지 (`unim-settings`, Linux·Windows 공용)

| 파일:줄 | 변경 |
|---|---|
| `unim-settings/ui/settings.slint:404-446` | 속성 `hanja-model:[StandardListViewItem]`, `hanja-selected:int`, `hanja-hangul/hanja/meaning:string`, `hanja-error:string`, `hanja-corrupt:bool`; 콜백 `hanja-add()`, `hanja-save-edit()`, `hanja-select(int)`, `hanja-remove(int)`, `hanja-quarantine()` |
| 사이드바 `:611-640`, 키보드 탐색 `:589-592` | NavItem 6번째(페이지 5 "한자 사전"), `Math.min(4, …)` → `Math.min(5, …)`. 기존 페이지 번호 유지(알림=4) |
| 페이지 블록(ATF 사용자 사전 `:1311-1365` 본보기) | `if root.current-page == 5:` 안내 Text + 입력 Card(3 `LineEdit`, `accepted` 로 다음 칸 포커스/저장, 오류 Text, [추가] [고친 내용 저장]) + 목록 Card(`StandardListView`, `current-item` 변경 → `hanja-select`, [선택 삭제]) + 깨짐 Card |
| `export global Tr`(:227-238) | `hd-added`, `hd-saved`, `hd-removed`, `hd-err-*` 문구 |
| `unim-settings/src/main.rs` | `refresh_hanja(ui)`(디스크에서 다시 읽어 모델·깨짐 상태 갱신, 선택 인덱스는 삭제 뒤 같은 자리로 클램프), 콜백 배선(:1506-1560 userdict 본보기 — 단 **메모리 사본을 들지 말고 매번 `load_default`**, 조작은 `find`), `enum DeleteSnapshot`(:707)에 `HanjaDict(HanjaUserEntry, usize)` 추가 + `on_undo_restore`(:1564~) 분기, 2초 `slint::Timer` mtime 폴링, 모듈 머리 주석(:6-11) 목록 갱신 |
| `unim-settings/translations/en/LC_MESSAGES/unim-settings.po` | 새 `@tr` 문자열 영문 |

검증: Linux `cargo build -p unim-settings` · `UNIM_CONFIG_DIR=$(mktemp -d) cargo run -p unim-settings` 수동. **Windows 빌드는 로컬 불가**(skia → MSVC, Makefile `check-windows` 주석 :482) — develop 대상 PR 로 `windows-msi.yml`(MSVC check·build) 통과 확인.

## 7. WP5 — 문서·i18n

| 파일 | 변경 |
|---|---|
| `docs/user/user-guide/README-ko.md`·`README.md` | §4.2 "단어 즐겨찾기"(ko :655) 뒤에 `#### 한자 사용자 사전`(등록·CLI 예·후보 순서·적용 시점·파일 위치). §5 설정 페이지 목록에 "한자 사전" 페이지(en :933 "5.4 User Dictionary" 다음). 파일 표(Linux·Windows, en :1206 근처)에 `hanja-userdict.yaml` — 백업 YES |
| `docs/man/unim.1` | FEATURES 한 줄, FILES(:89-) `~/.config/unim/hanja-userdict.yaml` |
| `docs/man/unim-cli.1` | DICT SUBCOMMANDS 절 신설(`dict user-dict`·`dict blacklist`·`dict hanja`) + EXAMPLES 1줄 |
| `docs/man/unim-settings.1`·`unim-settings-gtk.1` | FILES 에 한 줄 |
| `help/` | `make help-html` 재생성 → `make check-help-html` |
| `CHANGELOG.md` `[0.4.5] ### Added` | "Hanja user dictionary — register names and terms missing from the built-in dictionary (Settings › Hanja Dictionary, `unim-cli dict hanja`)" — **SPEC §10-3 승인 후** |
| `docs/dev/specs/HANJA_WORD_SPEC.md` | v1.11 상태 줄 + §2.5.1 에 "후보 = 사용자 사전 → 내장(HANJA_USERDICT_SPEC §3)" 한 줄 — **기현님 승인(2026-11-16)** |
| ATF 표시 문구 개명(SPEC §10-2) | 사용자 문서·도움말·man 의 ATF「사용자 사전」→「자동 교정 사전」(en "User Dictionary" → "Auto-correction Dictionary"). GTK·Slint 문자열은 WP3·WP4 가 맡는다 |
| `docs/dev/specs/POPUP_SPEC.md` §3.7 규칙 3 | "사용자 한자 사전 항목이 내장 사전보다 앞(HANJA_USERDICT_SPEC §3)" 한 문장 — **기현님 승인(2026-11-16)** |
| `src/SPEC.md:111-118` | `InputEngine` 필드 목록에 `hanja_user` |

## 8. 위험과 대응

| 위험 | 대응 |
|---|---|
| ★ 토글 재정렬(`candidates.rs:285`)이 내장만 다시 받아 사용자 항목 소실 | §3.2 #5 교체 + 전용 시험 |
| 단위 시험이 개발자의 실제 `~/.config/unim/hanja-userdict.yaml` 을 읽어 결과가 바뀜 | `cfg(test)` 기본 빈 층. 타 크레이트 시험(`unim-dbus` 등)이 후보를 단언하면 `set_shared_override(Some(empty))` 또는 `UNIM_CONFIG_DIR=<임시>` |
| 기존 동작 변화 | 파일 없음 = 병합 결과가 내장과 동일 → 기존 시험 무수정 통과가 게이트 |
| TSF(앱 UI 스레드)에서 파일 I/O | 한자키당 `stat` 1회(µs), 재파싱은 변경 때만(작은 파일). `unwrap` 금지·Mutex poison `into_inner` |
| 데이터 유실(깨진 파일 덮어쓰기, 오래된 인덱스로 엉뚱한 항목 삭제) | SPEC §8: 쓰기 거부·보관 후 시작, 조작은 `(한글,한자)` 로 찾기, 무효 항목 보존 |
| 로그 평문 유출(serde_yaml 오류 문장에 항목 내용) | 로그는 행·열만. 리뷰 때 `rg -n 'unim_log!.*(hangul\|hanja\|meaning)' src unim-settings*` 로 점검 |
| Slint Windows 회귀 | 로컬 검증 불가 → `windows-msi.yml` PR 통과가 게이트 |
| POPUP_SPEC | 렌더링·키·시그니처 무변경. 순서 문구만 승인 대기(§7) |

## 9. 통합 검증(전 WP 머지 뒤)

1. `make build`(경고 0) · `cargo test --workspace` · `make check-windows` · `rg -n 'hanja_dict\.(search|contains)' src` = 0건.
2. GNOME 확장·데몬 무수정이므로 `make test-gnome-logic`·L3 재실행은 불필요(손댔다면 실행).
3. 실기(데몬 재시작 없이): `unim-cli dict hanja add 서기현 徐基鉉` → gedit 에서 음절 모드 `서기현`+한자키(①) / 단어 안 커서(③) / 선택(②) → 첫 후보 徐基鉉, Space(★) 뒤에도 유지 → 설정앱에서 뜻 수정·삭제 → 다음 한자키 반영.
4. `journalctl --user -u unim-daemon --since -10min | rg '사용자 한자'` — 개수만, 평문 없음.
5. Windows: MSI CI 산출물로 VM 에서 Slint 페이지 추가 → 메모장 한자키 반영(VM 대기 항목에 추가).

## 10. 덤으로 발견한 것 (이번 범위 밖, 기록만)

- `unim-cli/locales/ko.yml:321` `help_ud_path_long` 이 "`$XDG_DATA_HOME/unim/typefix_userdict.json`" 이라고 안내 — 실제는 `~/.config/unim/typefix-userdict.yaml`.
- `src/typefix_userdict.rs:113` 파싱 실패 시 serde 오류 문장을 `eprintln!`(항목 내용이 섞일 수 있음)하고 빈 사전을 돌려줘, 다음 CLI/GUI 저장이 원본을 덮어쓴다(유실).
- GTK ATF 페이지 삭제(`settings_dialog.rs:2137`)는 목록을 그린 뒤 다시 읽은 파일에 같은 **인덱스**로 `remove_at` — 그 사이 CLI 가 바꿨으면 다른 단어가 지워질 수 있다.
