# 한자 단어 입력 — 설계안 A-engine (엔진 상태기계·최소 위험 우선)

- 저장소: `/home/from104/work/unim` (develop, HEAD ba64255) — 파일 미수정, 산출은 이 문서뿐.
- 근거 표기: `file:line` 은 전부 이 세션에서 직접 읽은 코드/문서다. 지도 파일(같은 폴더의 `survey-*.md`)의 결론은 그대로 인용하고, 설계에 필요한 지점은 재확인했다.
- 각도: 코어(`src/`) 변경을 **한 곳(신규 impl 파일)에 모으고**, 기존 단음절·ATF 경로는 "다음절 일치가 없으면 바이트 동일" 을 불변식으로 잡는다. 모든 경로(push/pull/마우스/취소 3종/FocusOut/Reset/content_purpose)를 §4 의 표로 닫는다.

---

## 1. 개요·범위

### 1.1 한 줄 요약
한자키를 눌렀을 때 변환 대상을 "preedit 마지막 음절" 에서 **(엔진이 직접 기억한 최근 커밋 음절 + 현재 preedit) 의 사전 최장 접미** (대상①) 또는 **앱 선택 영역** (대상②) 으로 넓힌다. 확정 시 이미 앱에 나간 접두는 기존 `AutoTypefixApply(delete_chars, commit_text, preedit_text="")` 채널로 지우고 서식 적용 문자열을 커밋한다. 팝업(9개/페이지·즐겨찾기·재정렬)은 그대로 재사용한다.

### 1.2 v1 범위 (PM D1 그대로)
| 항목 | v1 | 근거 |
|---|---|---|
| 대상① (최근 커밋+preedit) | Linux 6 프런트 전부 + Windows TSF(cross-compile 검증만) | 교체 채널 `AutoTypefixApply` 를 GTK3/4·Qt5/6·XIM·Wayland·GNOME 이 이미 구독(atf-replacement.md 보충#1). TSF 는 `replace_surrounding` (unim-tsf/src/composition.rs:663) |
| 대상② (선택 영역) | GTK4·Qt5/6·GNOME·Wayland(배선 신설)·TSF. **GTK3·XIM 미지원**(기존 idle 동작 유지) | GTK3 는 anchor 를 못 받는다(unim-frontends/gtk3/src/immodule.c:1220-1222 `selection_index = cursor_index`), XIM 은 surrounding 인프라 자체가 없다(selection-surrounding.md §2) |
| 출력 형식 설정 | 단음절 변환에도 동일 적용(D2) | `select_hanja` 한 곳에서 서식 조립 → 키보드/마우스/pull 전 경로 자동 동기 |
| IMM32 | 무변경 | 후보창 스텁(unim-imm32/src/ui_window.rs:106-145) |
| 커밋 | 하지 않음(D12) | — |

### 1.3 v2 로 미루는 것 (설계 메모만)
- target 축소/확장 키(예: `←/→` 로 "대한민국"→"민국")(D5).
- preedit 이 비어 있고 버퍼만 있는 상태(공백 없이 비자모로 끝난 직후 등)에서의 한자키 → 현행대로 이모지. §3.1 분기표 참고.
- 조사 분리("대한민국은" → "대한민국"): ROADMAP.md:126-127 의 난점 — 최장 접미 일치는 접미가 조사면 실패한다. v1 은 사용자가 조사 입력 전에 한자키를 누르는 흐름만 지원.
- 비프(D6 "비프 가능"): 코어에 비프 설비가 없고(`crate::beep` 은 unim-dbus 전용, unim-dbus/src/engine_worker.rs:1739) 무동작으로 둔다.
- 기능 on/off 스위치(`hanja_word_conversion`): §11.3 참고 — v1 은 두지 않는다.

---

## 2. 데이터 모델

### 2.1 최근 커밋 음절 버퍼 `recent_syllables`

**정의(불변식)**: `recent_syllables` 는 "마지막 경계 사건 이후 엔진이 이 컨텍스트의 앱에 **커밋한 텍스트** 중, 끝에서 이어지는 한글 완성 음절(U+AC00..U+D7A3) 열의 최대 17자". 경계 사건 = §2.1.3 리셋 표의 모든 행. preedit 은 포함하지 않는다(preedit 은 아직 앱에 "커밋" 되지 않았다).

| 항목 | 값 |
|---|---|
| 타입 | `String` (엔진 필드, `pub(super) recent_syllables: String`, `src/input_engine/engine.rs` 필드 블록 :119-123 옆) |
| 상한 | `RECENT_SYLLABLE_CAP = HANJA_MAX_KEY_CHARS - 1 = 17` (`HANJA_MAX_KEY_CHARS = 18`, 사전 최장 키 실측 18자 — popup-pipeline.md 보충#7(4)). 초과 시 앞에서 pop |
| 소유 | 엔진(`InputEngine`) — TSF 는 코어를 직접 링크하므로 자동 상속(windows-parity.md §6) |
| 초기값 | `InputEngine::new` 에서 빈 문자열(engine.rs:248-287 리터럴에 추가) |

#### 2.1.1 push/pop/reset 연산
```rust
// src/input_engine/hanja_word.rs (신규 impl InputEngine 블록)
const RECENT_SYLLABLE_CAP: usize = 17;
pub(super) fn is_hangul_syllable(c: char) -> bool { ('\u{AC00}'..='\u{D7A3}').contains(&c) }

pub(super) fn recent_push_char(&mut self, c: char) {
    if self.content_purpose.should_block_hangul() { self.recent_syllables.clear(); return; } // fail-closed (surrounding.rs:71-76 복제)
    if !is_hangul_syllable(c) { self.recent_syllables.clear(); return; }                     // 비한글 커밋 = 경계
    if self.recent_syllables.chars().count() >= RECENT_SYLLABLE_CAP {
        let first_len = self.recent_syllables.chars().next().map(char::len_utf8).unwrap_or(0);
        self.recent_syllables.drain(..first_len);
    }
    self.recent_syllables.push(c);
}
pub(super) fn recent_pop(&mut self) { self.recent_syllables.pop(); }
pub(super) fn recent_clear(&mut self) { self.recent_syllables.clear(); }
```

#### 2.1.2 훅 위치 — 단일 초크포인트(래퍼) 방식
지도(hanja-core.md 보충#3)는 push 5곳(press_key.rs:510, 1102-1108, 1160, 1252, 1277)에 훅을 권했지만, **리셋 지점이 12곳+5곳으로 흩어져** 있어 놓칠 위험이 크다. 대신 `press_key` 를 얇은 래퍼로 감싸 "이번 키 처리로 `commit_buffer` 에 **새로 붙은 문자열**" 을 한 곳에서 분류한다. 이 방식이 가능한 근거: 5개 Rust 호스트가 `commit_str()` 직후 같은 함수 안에서 `clear_commit()` 을 짝지어 호출하지만(hanja-core.md 보충#3 §3), 래퍼는 drain 여부와 무관하게 **호출 전 길이 스냅샷**(`before_len`) 이후 구간만 본다.

```rust
// src/input_engine/press_key.rs — 기존 `pub fn press_key`(:59-63) 본문을 `press_key_inner` 로 개명(pub(super)), 새 press_key:
pub fn press_key(&mut self, keycode: KeyCode, modifier: ModifierState, config: &Config) -> InputResult {
    let before_len = self.commit_buffer.len();
    let was_popup = self.hanja_mode || self.special_char_mode || self.is_emoji_popup_active();
    let cat_before = self.input_category;
    let result = self.press_key_inner(keycode, modifier, config);
    self.recent_track_after_key(keycode, modifier, &result, before_len, was_popup, cat_before);
    result
}
```
```rust
// src/input_engine/hanja_word.rs
pub(super) fn recent_track_after_key(&mut self, keycode: KeyCode, modifier: ModifierState,
                                     result: &InputResult, before_len: usize, was_popup: bool, cat_before: InputCategory) {
    // (0) reset() 등이 버퍼를 비웠으면 스냅샷이 무효 → 경계
    if self.commit_buffer.len() < before_len { self.recent_clear(); return; }
    // (1) 팝업 확정/취소/재처리(NotHandled) 로 나온 텍스트는 추적하지 않는다 — D3 "팝업 확정/취소 후 리셋"
    if was_popup { self.recent_clear(); return; }
    // (2) 한/영 전환(토글키·auto-english·ATF 자동전환) = 경계
    if self.input_category != cat_before { self.recent_clear(); return; }
    // (3) 새로 커밋된 문자열 분류: 음절 → push, 그 외 → clear (push 내부에서 처리)
    let appended: Vec<char> = self.commit_buffer[before_len..].chars().collect();
    for c in appended.iter().copied() { self.recent_push_char(c); }
    // (4) 앱으로 통과한 키(not_consumed / committed_passthrough) = 커서 이동·Enter·Tab·Escape·Delete·단축키 → 경계.
    //     예외 1: 수정자 단독 키(Shift 등, press_key.rs:80-82 not_consumed) 는 무시.
    //     예외 2: Backspace 통과(조합 없음, press_key.rs:315-325) 는 앱이 커서 앞 1자를 지운 것 → pop.
    //             단 선택 영역이 있으면(surrounding_cursor != surrounding_anchor) 앱은 선택을 지우므로 clear.
    if !result.consumed && !keycode.is_modifier() {
        if keycode == KeyCode::Backspace && appended.is_empty() && !self.korean_context.is_composing() {
            if self.surrounding_cursor != self.surrounding_anchor { self.recent_clear(); } else { self.recent_pop(); }
        } else {
            self.recent_clear();
        }
    }
    let _ = modifier;
}
```
- `InputResult` 의 `consumed` 의미: `not_consumed()`/`committed_passthrough()` 만 `consumed=false` (src/input_engine/types.rs:241-249, 286-294). Enter(press_key.rs:328-336)·Tab(:339-347)·Escape(:350-358)·비문자키(:597-605)·Ctrl/Alt 조합(:172-208) 이 전부 이 두 결과로 떨어진다 → 자동으로 리셋. Enter 는 조합 중이면 `flush_preedit` 로 음절이 먼저 붙고(3에서 push) 곧바로 (4)에서 clear — 순서상 옳다.
- 팝업 NotHandled 재처리(src/input_engine/popup_dispatch.rs:180-185)의 `self.press_key(...)` 는 **`press_key_inner` 로 바꿔** 래퍼 중첩을 막는다(외곽 래퍼 1회만 분류; `was_popup=true` 라 결국 clear).
- 래퍼 밖에서 `commit_buffer` 에 음절이 붙는 유일한 경로 `chord_idle_flush_pending`(engine.rs:844-861, `mem::take`) 에 `for c in taken.chars() { self.recent_push_char(c) }` 한 줄 추가. `chord_idle_flush_commit`(engine.rs:820-830)은 FocusOut/Reset 전용(engine_worker.rs:733) — 직후 엔진이 재생성되므로 훅 불필요.

#### 2.1.3 리셋 조건표 (D3 전량 + 구현 지점)
| 경계 사건 | 구현 지점 | 비고 |
|---|---|---|
| 비한글 커밋(영문·공백·구두점·숫자·특수문자·이모지·한자 확정·분해 자모) | 래퍼 (3) `recent_push_char` 의 비음절 분기 | press_key.rs 의 12개 비음절 push 지점(hanja-core.md 보충#3 §1)을 개별 수정하지 않는다 |
| Enter·Tab·Escape·방향키·Home/End/PageUp/PageDown·Delete·Insert·F키·Ctrl/Alt/Super 단축키 | 래퍼 (4) 통과 키 | 마우스 클릭은 키가 아니라 아래 Reset 행 |
| Backspace(조합 없음) | 래퍼 (4) → pop (선택 있으면 clear) | 조합 중 BS 는 preedit 만 바뀜(press_key.rs:318-322) → 버퍼 불변 |
| 한/영 전환(토글키·auto-english·`set_input_category`) | 래퍼 (2) + `set_input_category`(engine.rs:671-678) 안에서 카테고리가 실제로 바뀔 때 `recent_clear()` | 후자는 engine_worker 의 Global 전파(:1845-1849)·FocusIn 경로처럼 press_key 밖 호출 대비 |
| 팝업 확정/취소/NotHandled 재처리 | 래퍼 (1) + `select_hanja`/`cancel_hanja` 내부 직접 `recent_clear()` | 후자는 RPC(pull/마우스) 경로(래퍼 없음) 대비 |
| `engine.reset()` | engine.rs:764-784 에 `recent_syllables.clear(); pending_hanja_replacement=None; hanja_* 신규 필드 clear` 추가 | ATF 교정 후 reset(engine_worker.rs:600,1629; unim-tsf/src/auto_typefix.rs:455,520)·TSF OnSetFocus(text_service.rs:1868) 포함 |
| FocusOut·Reset(마우스 클릭·XResetIC·GNOME cursor-jump) | Linux: `reset_engine_and_capture_commit`(engine_worker.rs:723-760)가 `InputEngine::new` 로 재생성 → 자동 | 정상 연속 타이핑 중엔 Reset 이 오지 않음(selection-surrounding.md 보충#4) |
| content_purpose 차단(비밀번호/PIN) | `set_content_purpose`(surrounding.rs:38 분기) 에 `recent_clear()`; `recent_push_char` 첫 줄 fail-closed | pull 경로 노출 차단은 §4.6 |
| 설정 리로드(파괴적 재구성) | `rebuild_korean_context`(engine.rs:933-) 에 `recent_clear()` | 자판이 바뀌면 문맥 무의미. 비파괴 setter(§6)는 건드리지 않음 |
| 단어 모드 토글 `set_word_mode` | 건드리지 않음 | 호출부가 전부 리셋 직후(FocusIn/Reset/ATF/OnSetFocus)라 버퍼가 이미 비어 있다(engine_worker.rs:692 주석) |

### 2.2 변환 대상 표현 — `hanja_target` 를 셋으로 분리
| 필드(신규/기존) | 의미 | 대상①(음절모드) | 대상①(Word모드) | 대상② | 현행 단음절 |
|---|---|---|---|---|---|
| `hanja_target: String` (기존, engine.rs:123) | **사전 키 = 표시·즐겨찾기 키** | "대한민국" | "대한민국" | "대한민국" | "국" |
| `hanja_committed_chars: u32` (신규) | 확정 시 지울 **이미 앱에 나간** 글자 수 | 3 | 0 | 0 | 0 |
| `hanja_recommit: String` (신규) | 취소/FocusOut 시 다시 커밋할 텍스트(= 팝업 진입 때 지운 preedit 전체) | "국" | "오늘대한민국" | "" | "국" |
| `hanja_uncommitted_prefix: String` (신규) | 확정 시 한자 앞에 붙일, 사전 불일치 preedit 접두(Word 모드) | "" | "오늘" | "" | "" |
| `pending_hanja_replacement: Option<HanjaReplacement>` (신규) | 확정 결과 out-of-band 드레인(§4.3) | Some | None | None | None |

```rust
// src/input_engine/types.rs (PopupAction 옆, ABI 무관 — InputResult 불변)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HanjaReplacement {
    /// 커서 앞에서 지울 **확정** 글자 수(preedit 제외). Linux 는 이 값을 AutoTypefixApply.delete_chars 로 그대로 싣는다.
    pub delete_chars: u32,
    /// 팝업 진입 당시 preedit 글자 수. Linux 는 응답 preedit="" 로 지우므로 무시. TSF 는 composition 을 keep_text 로 materialize 한 뒤 삭제 span 에 더한다(unim-tsf/src/key_handler.rs:861-867 역방향 ATF 패턴).
    pub preedit_chars: u32,
    /// 서식 적용이 끝난 최종 커밋 문자열(Word 모드 접두 포함).
    pub text: String,
}
```
- `get_hanja_target()`(candidates.rs:127-129) 은 그대로 키를 반환(팝업 헤더·`HanjaCandidatesReordered.target`·`is_bookmarked` 호환). 신규 `pub fn get_hanja_recommit(&self) -> &str` 를 추가하고 **재커밋 3경로**(popup_dispatch.rs:227-228, engine_worker.rs:737-742, engine_worker.rs:2036-2040)를 이 접근자로 바꾼다. 현행 단음절에서는 `recommit == target` 이라 바이트 동일.
- `pub fn take_hanja_replacement(&mut self) -> Option<HanjaReplacement>` — `take_atf_toggle`(engine.rs:604-606)·`take_popup_action`(popup_dispatch.rs:20) 과 같은 drain 채널.

### 2.3 `HanjaOutputFormat` enum (D2)
```rust
// src/config.rs — CommitUnit(:57-90) 바로 아래
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum HanjaOutputFormat {
    /// 漢字 만 (기본, 현 동작 유지)
    #[default]
    Hanja,
    /// 한자(漢字)
    HangulHanja,
    /// 漢字(한자)
    HanjaHangul,
}
impl HanjaOutputFormat {
    pub fn display_name(&self) -> &'static str { match self { Hanja => "漢字", HangulHanja => "한자(漢字)", HanjaHangul => "漢字(한자)" } }
    pub fn all() -> &'static [Self] { &[Self::Hanja, Self::HangulHanja, Self::HanjaHangul] }
    /// 서식 조립 — 유일한 구현 지점. 괄호는 ASCII `(`/`)`.
    pub fn render(&self, hangul: &str, hanja: &str) -> String {
        match self { Hanja => hanja.to_string(), HangulHanja => format!("{hangul}({hanja})"), HanjaHangul => format!("{hanja}({hangul})") }
    }
}
```
- serde 태그는 variant 이름 그대로(`Hanja`/`HangulHanja`/`HanjaHangul`) — `CommitUnit` 관례(config-sync.md §2.1: rename 없음, `#[repr(C)]`, `#[default]` 위치만 지정, 재배치 금지).
- `#[repr(C)]` 판별자 순서 `Hanja=0/HangulHanja=1/HanjaHangul=2` 고정.

### 2.4 서식 조립 규칙
| 상황 | 규칙 |
|---|---|
| 한글 부분의 출처 | `hanja_target`(사전 키). `HanjaEntry.hangul` 과 동일하지만(config-sync.md 보충#12(2)) 키를 쓰면 동음 다중 항목(國家/國歌)에서도 한 값이다 |
| 뜻(`meaning`) | 서식에 포함하지 않는다. 다음절 항목은 뜻이 비어 있거나 한글을 그대로 되풀이한다(`src/data/hanja.txt:57963` `대한민국:大韓民國:대한민국`, `:282334` `한국:韓國:대한민국`). 팝업 compact 는 이미 빈 뜻을 우아하게 생략(popup-pipeline.md 보충#7(1)) |
| 동음 다중 항목 | 각각 별개 후보(현행). 서식은 선택된 항목의 `hanja` 만 |
| Word 모드 접두 | `text = hanja_uncommitted_prefix + render(target, hanja)` — 접두는 서식 대상 아님 |
| 즐겨찾기 키 | `(hanja_target, entry.hanja)` 문자열 쌍 그대로(D7). 서식과 무관하게 한자 원문으로 저장 → 서식 설정을 바꿔도 즐겨찾기 유지 |
| 단음절 | 동일 규칙(D2) — `"가"` 선택 `家` + HangulHanja → `"가(家)"` |

---

## 3. 알고리즘

### 3.1 한자키 분기 (press_key.rs:226-238 개정)
```rust
if self.hanja_keys.contains(&keycode) {
    self.finalize_chord_buffer();
    let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
    if idle {
        // [신규] 선택 영역이 있으면 이모지가 아니라 대상② 시도. 사전 불일치면 팝업 없이 소비(D6).
        if self.selection_for_hanja().is_some() {          // §3.3 판정
            return self.start_hanja_conversion();          // 내부에서 대상② 처리, 실패 시 consumed()
        }
        self.start_emoji_popup();                          // 현행
        return InputResult::consumed();
    }
    return self.start_hanja_conversion();                  // 현행 호출, 내부가 대상①로 확장
}
```
- 비밀번호 필드는 :212-217 에서 이미 영문 강제 → preedit 없음 → idle. `selection_for_hanja()` 는 `should_block_hangul()` 이면 `None`(surrounding_text 는 이미 비어 있음, surrounding.rs:71-76) → 이모지(현행).
- pull 경로(Qt/XIM `GetHanjaCandidates`)는 press_key 를 안 거치고 `start_hanja_conversion` 을 직접 부른다(engine_worker.rs:1990). 후보가 없으면 프런트가 `ProcessKey` 로 폴백해(qt5 input_context.cpp:396-408, xim handler.rs:1211-1217) 위 분기에 도달 → 선택이 있으면 이모지 없이 소비. 두 경로의 결론이 일치한다.

### 3.2 `start_hanja_conversion` 의 target 결정 (candidates.rs:22-29 교체)
```rust
// candidates.rs:16-
if self.hanja_mode { return InputResult::consumed(); }
if self.content_purpose.should_block_hangul() {              // [신규] pull 경로 fail-closed (selection-surrounding.md 보충#5)
    unim_log!("ENGINE", "한자 변환 차단: content_purpose"); return InputResult::consumed();
}
let Some(spec) = self.resolve_hanja_target() else {          // §3.2.1
    unim_log!("ENGINE", "한자/특수문자 후보 없음"); return InputResult::consumed();
};
let mut candidates = self.hanja_dict.search(&spec.key);
if !candidates.is_empty() {
    // 즐겨찾기 stable sort(:34-36) 그대로 — 키만 spec.key
    self.hanja_target = spec.key.clone();
    self.hanja_committed_chars = spec.committed_chars;
    self.hanja_recommit = spec.recommit;
    self.hanja_uncommitted_prefix = spec.uncommitted_prefix;
    ... (:43-71 동일: candidates/hanja_mode/PopupState::new_hanja_with_top_row/ShowHanja)
    return InputResult::hanja_candidates();
}
// 후보 없음: 대상② 는 특수문자 폴백 없이 소비(D6). 단음절/초성은 현행 특수문자 폴백(:74-104).
if spec.source == HanjaSource::Selection { return InputResult::consumed(); }
... (:74-108 동일)
```

#### 3.2.1 `resolve_hanja_target` (신규, `src/input_engine/hanja_word.rs`)
```rust
pub(super) enum HanjaSource { Syllable, Word, Selection }
pub(super) struct HanjaTargetSpec { key: String, committed_chars: u32, recommit: String, uncommitted_prefix: String, source: HanjaSource }

pub(super) fn resolve_hanja_target(&self) -> Option<HanjaTargetSpec> {
    let pre = self.preedit_cache.as_str();
    if !pre.is_empty() {
        let last = pre.chars().last()?;
        // (a) 미완성 자모(초성 "ㄱ" 등) → 현행 규칙(마지막 1글자, 특수문자 폴백 유지)
        if !is_hangul_syllable(last) {
            return Some(spec(last.to_string(), 0, pre.to_string(), pre_without_last(pre), HanjaSource::Syllable));
        }
        if self.is_word_mode() {
            // (b) Word 모드: 앱에 나간 글자 0 → delete 0, preedit 전체 안에서 최장 접미
            let (k, prefix) = longest_dict_suffix(pre, &self.hanja_dict, 1 /*min*/);
            return Some(spec(k, 0, pre.to_string(), prefix, HanjaSource::Word));
        }
        // (c) 음절 모드: (버퍼 + preedit) 의 최장 접미. 음절 모드 preedit 은 최대 1음절(input_context.rs:337-342, syllable 은 preedit 만)
        let joined = format!("{}{}", self.recent_syllables, pre);
        let (k, _) = longest_dict_suffix(&joined, &self.hanja_dict, 2 /*min: 버퍼 1자 이상 포함*/);
        let pre_n = pre.chars().count() as u32;
        let k_n = k.chars().count() as u32;
        if k_n > pre_n {
            let committed = k_n - pre_n;
            if self.recent_prefix_verified(&k, committed, pre) {            // §3.2.2 soft 검증
                return Some(spec(k, committed, pre.to_string(), String::new(), HanjaSource::Syllable));
            }
        }
        // 다음절 일치 없음/검증 실패 → 현행(마지막 음절) 바이트 동일
        return Some(spec(last.to_string(), 0, pre.to_string(), pre_without_last(pre), HanjaSource::Syllable));
    }
    // (d) preedit 없음: 선택 영역(대상②)
    let word = self.selection_for_hanja()?;
    Some(spec(word, 0, String::new(), String::new(), HanjaSource::Selection))
}

/// `text` 의 접미 중 사전에 있는 가장 긴 것. 길이 상한 18. 접미는 전부 완성 음절이어야 한다.
/// 반환 (접미, 접두). 없으면 (마지막 1글자, 나머지).
fn longest_dict_suffix(text: &str, dict: &HanjaDictionary, min_len: usize) -> (String, String) {
    let chars: Vec<char> = text.chars().collect();
    let max = chars.len().min(HANJA_MAX_KEY_CHARS);
    for k in (min_len.max(2)..=max).rev() {
        let suffix = &chars[chars.len()-k..];
        if !suffix.iter().all(|c| is_hangul_syllable(*c)) { continue; }   // 사이에 비음절이 끼면 그 길이는 불가
        let s: String = suffix.iter().collect();
        if dict.contains(&s) { return (s, chars[..chars.len()-k].iter().collect()); }
    }
    let last = chars.last().map(|c| c.to_string()).unwrap_or_default();
    (last, chars[..chars.len().saturating_sub(1)].iter().collect())
}
```
- `HanjaDictionary::contains(&str) -> bool`(src/hanja/dict.rs:104 옆, `entries.contains_key`) 를 추가해 루프에서 `search()` 의 Vec clone 을 피한다(최대 17회 조회).
- Word 모드 (b)의 min 1: 현행 Word 모드에서도 마지막 1음절은 항상 후보가 될 수 있어야 한다. 접두는 `uncommitted_prefix` 로 남겨 확정 시 함께 커밋(D4) — 부분 커밋 API 불필요(hanja-core.md 보충#10).
- 음절 모드 (c)의 min 2: 버퍼 글자를 최소 1자 포함해야 "단어" 다. 1글자 일치는 현행 폴백이 담당.
- `is_word_mode()` 는 런타임 누적 상태(engine.rs:695-697). Linux 기본 Smart==Syllable(빈 `word_mode_apps`) 이라 (c) 가 기본 경로.

#### 3.2.2 soft 정합성 검증 `recent_prefix_verified`
버퍼는 키 입력만 보므로 마우스 편집·앱 자동교정·Undo(단축키는 리셋되지만 메뉴 클릭은 아님) 뒤 어긋날 수 있다. 잘못 지우는 것이 최악의 결과이므로, **앱이 준 surrounding text 가 있으면** 접두를 대조한다.
```rust
fn recent_prefix_verified(&self, key: &str, committed: u32, pre: &str) -> bool {
    if self.surrounding_text.is_empty() { return true; }                       // XIM·Wayland(배선 전)·빈 필드 → 검증 불가, 통과
    let cursor = (self.surrounding_cursor as usize).min(self.surrounding_text.chars().count());
    let before: String = self.surrounding_text.chars().take(cursor).collect();
    let prefix: String = key.chars().take(committed as usize).collect();
    // GTK 계열 surrounding 은 preedit 을 포함하지 않는다(접두로 끝남). TSF 는 composition 텍스트가 문서에 있어 key 전체로 끝난다.
    before.ends_with(&prefix) || before.ends_with(key) || before.ends_with(&format!("{prefix}{pre}"))
}
```
- 실패 시 현행(마지막 음절) — 안전한 방향으로만 퇴화. Qt 는 focus-in 스냅샷이라 stale(qt5 input_context.cpp:554-564) → §5.2 에서 F9 시 재질의로 신선하게 만든다. GNOME 의 `vfunc_set_surrounding` 호출 빈도는 미확정(§11).

### 3.3 대상② 판정 `selection_for_hanja` (D6)
```rust
pub(super) fn selection_for_hanja(&self) -> Option<String> {
    if self.content_purpose.should_block_hangul() { return None; }
    let (text, cur, anc) = (self.surrounding_text.as_str(), self.surrounding_cursor, self.surrounding_anchor);
    if text.is_empty() || cur == anc { return None; }                                     // typefix_convert(surrounding.rs:193) 와 동일 판정
    let chars: Vec<char> = text.chars().collect();
    let (s, e) = (cur.min(anc) as usize, cur.max(anc) as usize);
    if e > chars.len() { return None; }
    let word: String = chars[s..e].iter().collect::<String>().trim().to_string();       // 앞뒤 공백 trim
    let n = word.chars().count();
    if n == 0 || n > HANJA_MAX_KEY_CHARS || !word.chars().all(is_hangul_syllable) { return None; }
    if !self.hanja_dict.contains(&word) { return None; }                                  // 정확 일치만
    Some(word)
}
```
- `press_key` 분기(§3.1)와 `resolve_hanja_target` (d) 가 같은 함수를 쓰므로 "선택 있음 ∧ 사전 불일치 → 이모지 없음·팝업 없음" 이 두 경로에서 일치한다. 단 press_key 분기는 "선택 존재" 만으로 이모지를 막아야 하므로 §3.1 의 호출을 `self.has_selection()`(cur != anc && !blocked) 로 두고, 사전 판정은 `start_hanja_conversion` 이 한다 — 즉 `if self.has_selection() { return self.start_hanja_conversion(); }`.
- trim 된 공백은 위젯이 선택 전체를 치환하므로 함께 사라진다(v1 수용, §11).

### 3.4 후보 조립·정렬 (D7, 무변경)
- `hanja_dict.search(key)` 순서(사전 등장순, dict.rs:104-106) → 즐겨찾기 stable sort(candidates.rs:34-36) → `PopupState::new_hanja_with_top_row(key, pairs, top_row)`(:58-64) → `HANJA_PAGE_SIZE=9`(popup_keys.rs:85).
- `toggle_hanja_bookmark`(candidates.rs:186-263)의 재조회 `search(&self.hanja_target)` 는 키가 단어여도 그대로 동작.

---

## 4. 상태기계·경로표

### 4.1 상태
| 상태 | 정의 |
|---|---|
| `Idle` | `hanja_mode=false`, preedit 없음 |
| `Composing` | preedit 있음(음절/Word 누적) |
| `HanjaPopup{src}` | `hanja_mode=true`, `src ∈ {Syllable(committed_chars≥0), Word, Selection}`, `preedit_cache` 는 진입 당시 값 유지(현행) |
| `Replacing` | (순간 상태) `pending_hanja_replacement=Some`, `hanja_mode=false`, preedit 비움 — 호스트가 drain 하면 `Idle` |

### 4.2 전이표
| # | 시작 상태 | 사건 | 버퍼 `recent_syllables` | target 필드 | 커밋/교체 출력 | 종료 |
|---|---|---|---|---|---|---|
| 1 | Composing(음절, 버퍼 "대한민", pre "국") | 한자키(push: press_key) | 불변 | key=대한민국, committed=3, recommit=국 | ShowHanja | HanjaPopup{Syllable} |
| 2 | 동일 | 한자키(pull: GetHanjaCandidates) | 불변 | 동일 | 응답+ShowHanjaPopup(engine_worker.rs:1990-2001) | 동일 |
| 3 | Composing(음절, 다음절 불일치) | 한자키 | 불변 | key=국, committed=0, recommit=국 | ShowHanja(현행 바이트 동일) | HanjaPopup{Syllable} |
| 4 | Composing(Word, pre "오늘대한민국") | 한자키 | 불변(Word 모드에선 항상 빈 값 — committed 가 안 나감) | key=대한민국, committed=0, recommit=오늘대한민국, prefix=오늘 | ShowHanja | HanjaPopup{Word} |
| 5 | Idle, 선택 "대한민국" | 한자키 | 불변 | key=대한민국, committed=0, recommit="" | ShowHanja | HanjaPopup{Selection} |
| 6 | Idle, 선택 있으나 불일치/비한글/>18 | 한자키 | 불변 | — | 없음(`consumed`) | Idle |
| 7 | Idle, 선택 없음 | 한자키 | 불변 | — | 이모지(현행) | Emoji |
| 8 | HanjaPopup{Syllable, committed=3} | 숫자/Enter(push) → `popup_select` | **clear** | 전부 clear | `pending=Some{3,1,text}`; `commit_buffer` 에 push 안 함; `HidePopup`; 결과 `preedit_updated()`(preedit "" 전달) | Replacing → 호스트 drain → Idle |
| 9 | HanjaPopup{Syllable, committed=0} / 현행 | 숫자/Enter | clear | clear | `commit_buffer.push(text)`; `committed()` (현행과 동일, text 만 서식 적용) | Idle |
| 10 | HanjaPopup{Word} | 숫자/Enter | clear | clear | `commit_buffer.push(prefix+text)`; `committed()` | Idle |
| 11 | HanjaPopup{Selection} | 숫자/Enter | clear | clear | `commit_buffer.push(text)`; `committed()` — 위젯이 선택을 치환(§5.2) | Idle |
| 12 | HanjaPopup{*} | 마우스 SelectHanja RPC | clear(`select_hanja` 내부) | clear | committed>0: `redirect_auto_typefix_apply` + HidePopup / 그 외: `redirect_commit_and_hide(text)` (service.rs:2886-2920 확장) | Idle |
| 13 | HanjaPopup{*} | Escape / NotHandled 키 → `popup_cancel` | clear(래퍼 (1)) | clear | `commit_buffer.push(recommit)`(빈 문자열이면 없음) + HidePopup; NotHandled 는 이어서 키 재처리 | Idle/Composing |
| 14 | HanjaPopup{*} | CancelHanja RPC(팝업 외부 클릭·GTK popup hide 훅·Wayland deactivate) | clear(`cancel_hanja` 내부) | clear | engine_worker.rs:2035-2040 이 `get_hanja_recommit()` 반환 → service.rs:3161-3168 `redirect_commit_and_hide` | Idle |
| 15 | HanjaPopup{*} | FocusOut / Reset RPC | 엔진 재생성 | 재생성 | `reset_engine_and_capture_commit`(engine_worker.rs:737-742) 가 `get_hanja_recommit()` 을 커밋 텍스트로 반환 | 새 엔진 Idle |
| 16 | HanjaPopup{*} | `engine.reset()` 직접(ATF 내부·TSF OnSetFocus) | clear | clear + pending=None | 없음(현행과 동일: TSF 는 문서에 composition 텍스트가 남는다 §5.4) | Idle |
| 17 | HanjaPopup{*} | 즐겨찾기 토글/페이지/expand | 불변 | 불변 | 현행 | 동일 |
| 18 | 임의 | SetContentType(Password/PIN) | clear | (팝업 중이면 현행대로 유지 — `set_content_purpose` 는 팝업을 닫지 않음) | — | — |
| 19 | 임의 | 한/영 전환·통과 키·비한글 커밋 | clear | — | 현행 | — |
| 20 | Composing(음절) | Backspace | 조합 중: 불변 / 조합 없음: pop | — | 현행 | — |

- 표 8 의 `preedit_updated()` 반환: engine_worker 는 `auto_typefix` 가 Some 이면 어차피 `final_preedit = Some("")`(engine_worker.rs:1709-1725) 로 preedit 을 지우므로 어느 결과든 무방하나, TSF 가 `result.consumed` 를 보고 키를 먹어야 하므로(unim-tsf/src/key_handler.rs:575-578) `consumed=true` 인 결과여야 한다.
- 표 13/14 에서 `recommit` 이 빈 문자열(Selection)이면 커밋 없음 — 선택은 앱에 그대로 남는다(D8).

### 4.3 out-of-band 채널 — 생산자/소비자 상호배제
| 생산 | 소비(Linux) | 소비(TSF) |
|---|---|---|
| `select_hanja` 가 `committed_chars>0` 일 때만 `pending_hanja_replacement=Some` | engine_worker ProcessKeyEvent: `take_popup_action()`(:1334) 직후 `let hanja_repl = engine.take_hanja_replacement();` → `auto_typefix_result = Some((d, text, ""))` 로 **기존 필드 재사용**(:1749); SelectHanja 핸들러(:2013-2028): `select_hanja` 직후 drain | `handle_key_down`: `drain_popup_actions`(:587) 직후 drain → §5.4; `apply_reverse_event`: `drain_popup_actions`(:1218) 직후 drain |
| ATF `check_forward/check_reverse` 는 `popup_action.is_some()` 이면 스킵(engine_worker.rs:1696-1699; TSF `!popup_active` 게이트 key_handler.rs:753-754) | 같은 키에서 두 값이 동시에 Some 이 되는 경로 없음 — 확정 키는 항상 `HidePopup` 액션을 동반 | 동일 |
| engine_worker.rs:1841 `resp.auto_typefix.is_some() && Global` 모드 전파 블록 | `hanja_repl.is_none()` 조건 추가(카테고리 전파는 ATF 자동 전환 전용 — 한자 교체는 모드 불변) | — |

---

## 5. 교체 실행

### 5.1 Linux 데몬 (unim-dbus)
| 지점 | 변경 |
|---|---|
| `EngineRequest::SelectHanja.response`(service.rs:67-71) | `oneshot::Sender<Option<String>>` → `oneshot::Sender<Option<(u32, String)>>` (delete_chars, text) |
| engine_worker.rs:2013-2028 | `let text = engine.select_hanja(index); let repl = engine.take_hanja_replacement(); response.send(text.map(|t| (repl.map(|r| r.delete_chars).unwrap_or(0), t)))` |
| service.rs:2886-2920 `select_hanja` RPC | `let (d, hanja) = resp.unwrap_or((0, String::new())); if d > 0 { self.redirect_auto_typefix_apply(d, &hanja, "").await; self.redirect_commit_and_hide("").await /* HidePopup 만 */ } else { self.redirect_commit_and_hide(&hanja).await }; Ok(hanja)` — **RPC 시그니처 `SelectHanja(u)->s` 불변**(popup-service dbus_server.rs:208-214 forward·GNOME popupSelectHanja 무수정) |
| service.rs:1807 옆 신규 `redirect_auto_typefix_apply(&self, delete_chars: u32, commit_text: &str, preedit_text: &str)` | `redirect_commit_and_hide` 와 동일하게 `last_active_input_context_path` 로 `self.connection.emit_signal(None, &path, "org.atit.unim.InputContext", "AutoTypefixApply", &(delete_chars, commit_text.to_string(), preedit_text.to_string()))`. 멤버명은 zbus 변환 결과 `AutoTypefixApply`(unim-gnome-extension/dbus_ime.js:267,278 실사용 명) |
| engine_worker.rs ProcessKeyEvent | :1334 직후 drain; ATF 블록 뒤 `if let Some(r) = hanja_repl { auto_typefix_result = Some((r.delete_chars, r.text, String::new())); fix_has_replay = false; }`; :1841 게이트 |
| engine_worker.rs:737-742, :2036-2040 | `get_hanja_target()` → `get_hanja_recommit()` |
| engine_worker.rs:983-988 리로드 루프 | `engine.set_hanja_output_format(&config);` 추가(비파괴, §6) |
| 시그널 순서 | ProcessKey 응답(preedit "" → 프런트가 preedit 제거)이 먼저 반환되고 `AutoTypefixApply` 는 :2155-2166 에서 발행 — 프런트는 동기 RPC 응답을 처리한 뒤 메인루프에서 시그널을 받는다(ATF 역방향과 동일 순서) |

### 5.2 프런트엔드 매트릭스
| 프런트 | 대상① | 대상② | 교체 채널 처리(기존) | 수정 file:line | 문제/해법 |
|---|---|---|---|---|---|
| GTK4 | ✔ | ✔ | `on_auto_typefix`(unim-frontends/gtk4/src/immodule.c:463-534): `gtk_im_context_delete_surrounding` → XTest BS → `\b` 3단 폴백, commit, `preedit ""` 클리어(:531-533) | 없음 | 선택 치환: 마우스 확정은 `on_commit_text`(:451-461)→`commit` 시그널만 발행하고 IM 모듈의 선택-삭제 래퍼(:1060-1086)는 안 탄다. **GtkText/GtkTextView 의 commit 핸들러가 선택을 먼저 지우는 것이 GTK 표준 동작**(GtkText `gtk_text_commit_cb`→선택 삭제 후 삽입; GtkTextView `gtk_text_view_commit_handler`→`gtk_text_buffer_delete_selection`) — 저장소 밖 근거라 L3 로 실측(§9.3). 키보드 확정은 :1061-1085 래퍼가 선택을 먼저 지운다 |
| GTK3 | ✔ | ✘ | gtk3/src/immodule.c:396-462 동형 | 없음 | anchor 부재(:1220-1222) → 엔진에 선택이 절대 안 보임 → idle 이모지(현행) |
| Qt5/6 | ✔ | ✔ | `setAutoTypeFixCallback`(qt5 input_context.cpp:177-222): `setCommitString(commit, -d, d)` 원자 교체, Konsole 은 `\b` 접두 | qt5 :380-413 / qt6 :381-414 한자키 분기 **첫 줄**에 `QInputMethodQueryEvent(ImSurroundingText\|ImCursorPosition\|ImAnchorPosition)` 재질의 후 `m_dbus->setSurroundingText(text, cur, anc)` **무조건 전송**(빈 텍스트도 — stale 제거) | stale(:554-564 focus-in 1회) 해소. 선택 치환: 키보드 확정은 :446-463 래퍼, 마우스 확정은 `setCommitTextCallback`(:224-230) → `QInputMethodEvent::setCommitString` — QLineEdit/QTextEdit 은 commit 이 있으면 선택을 제거하고 삽입(Qt 표준, `QWidgetLineControl::processInputMethodEvent`/`QWidgetTextControlPrivate::inputMethodEvent` 의 `removeSelectedText`) → L3 실측 |
| XIM | ✔ | ✘ | N+1 self-BS(unim-frontends/xim/src/handler.rs:544-573, :1052-1112) | handler.rs `PopupEvent::AutoTypeFix` 처리(:522-549) 첫머리에 **로컬 preedit 이 남아 있으면 `self.preedit(server, user_ic, "")` 로 먼저 지운다** | 마우스 확정(RPC 경로)은 ProcessKey 응답이 없어 XIM 이 그린 "국" 이 남을 수 있다 → 위 수정. **Reset 부작용(:1103-1108) 무해 증명**: 교체 시점에 엔진은 이미 `cancel_hanja` 를 거쳐 Idle(hanja_mode=false, preedit/commit_buffer 비움)이고 `pending` 은 drain 됨. Reset RPC → `reset_engine_and_capture_commit`(engine_worker.rs:1932-1935, preserve_mode=true) → 캡처할 preedit/target 없음 → `None` → 커밋 에코 없음; 재생성으로 `recent_syllables` 가 비는 것은 D3(확정 후 리셋) 와 일치. 카테고리·word 게이트는 보존/재적용(:758-760, desired_word) |
| Wayland (zwp_input_method_v2) | ✔ | ✔(배선 신설) | `apply_auto_typefix`(unim-frontends/wayland/src/state.rs:249-297) | §5.3 | 바이트 수 오판(D9) 해결 + `SurroundingText` 를 엔진에 전달. 선택 치환은 `commit_string` 을 받은 앱 툴킷(GTK/Qt wayland IM 컨텍스트 → 위 표준 동작) 책임 |
| GNOME 확장 | ✔ | ✔(실측 필요) | `onAutoTypeFix`(unim-gnome-extension/extension.js:283-300): `expectSelfBackspaces` + vkbd BS + 50ms 후 `commitText`(preedit 있으면 먼저 clear, unim_input_method.js:643-645) | 없음 | anchor 는 `vfunc_set_surrounding`(unim_input_method.js:562-566) 으로 이미 전달. 확정 `commit()` 이 선택을 치환하는지는 Mutter/앱 툴킷 소관(GTK4 앱은 text-input-v3 → GTK IM → 위 표준 동작). `vfunc_set_surrounding` 호출 빈도 미확정 → §3.2.2 soft 검증이 stale 이면 단음절로 퇴화(안전) |
| TSF | ✔ | ✔ | §5.4 | §5.4 | — |
| IMM32 | 무변경 | 무변경 | — | — | 스텁 |

### 5.3 Wayland D9 — 실측 바이트 (택일: SurroundingText 저장)
세 안 중 **"SurroundingText 이벤트 저장 + 실측 바이트, 없으면 현행 휴리스틱"** 을 택한다. 시그널 확장(바이트 수를 실어 보내기)은 6 프런트 전부의 시그니처를 건드리고, 프런트 자체 커밋 이력은 앱이 바꾼 텍스트를 못 본다. 컴포지터는 `surrounding_text(text, cursor: 바이트, anchor: 바이트)` 를 매 변경마다 보내고(wayland-protocols input-method-unstable-v2 §surrounding_text; atf-replacement.md 보충#6(2)) 미지원 앱은 첫 `done` 전까지 이 이벤트가 없다 → 휴리스틱 폴백.

| 지점 | 변경 |
|---|---|
| `State` 필드 | `surrounding: Option<(String /*text*/, u32 /*cursor_bytes*/, u32 /*anchor_bytes*/)>`, `pending_surrounding: Option<(String,u32,u32)>` |
| state.rs:626-628 `Event::SurroundingText { text, cursor, anchor }` | `state.pending_surrounding = Some((text, cursor, anchor))` (더블버퍼 — `done` 에서 적용) |
| state.rs:563 `Event::Done` 분기 | `if let Some(s) = state.pending_surrounding.take() { state.surrounding = Some(s.clone()); if state.current_active { let cur_c = s.0[..s.1.min(len)].chars().count(); let anc_c = ...; dbus_tx.blocking_send(DbusRequest::SetSurroundingText { context_path, text: s.0, cursor: cur_c as u32, anchor: anc_c as u32 }) } }` — 바이트→문자 오프셋 변환(UTF-8 경계 클램프) |
| dbus_client.rs `DbusRequest` enum(:41 `SetContentType` 옆) | `SetSurroundingText { context_path: String, text: String, cursor: u32, anchor: u32 }` variant + 핸들러(:346 패턴)에서 `InputContextProxy::set_surrounding_text(&text, cursor, anchor)` 호출(데몬 RPC 는 service.rs:2725 에 이미 있음) |
| `apply_auto_typefix`(:251-268) | `let before_bytes = match &self.surrounding { Some((t, cur, _)) if *cur as usize <= t.len() => { let before = &t[..*cur as usize]; let tail_bytes: usize = before.chars().rev().take(delete_chars as usize).map(char::len_utf8).sum(); tail_bytes as u32 } _ => 기존 is_forward 휴리스틱 }` |
| `handle_deactivate`(:307) | `surrounding = None; pending_surrounding = None` |
| ATF 회귀 0 | ATF 순방향은 ASCII 만, 역방향은 한글 완성형만 지운다 → 실측 바이트 == 휴리스틱 값. 실측 경로는 "정확히 같은 수" 를 내고, 폴백 경로는 코드 그대로 |
| 한자 교체 | 삭제 대상은 한글 3바이트 × N 이지만 `한자(漢字)` 형식은 첫 글자가 한글이라 휴리스틱이 1B 로 오판(보충#6(1)) → 실측 경로가 정답. 실측 불가 앱(폴백)에서는 이 형식만 여전히 위험 → §11 위험표 |

### 5.4 Windows TSF (cross-compile 검증만)
| 지점 | 변경 |
|---|---|
| `handle_key_down` 한자키 선택 읽기(신규, key_handler.rs:555 `engine.press_key` 직전) | `if (keycode == KeyCode::Hanja \|\| keycode == KeyCode::F9) && atf_active && !engine.is_composing() { match composition::read_selection_text(context, tid) { Some(sel) if sel.cursor != sel.anchor => engine.set_surrounding_text(sel.surrounding_text, sel.cursor, sel.anchor), _ => engine.set_surrounding_text(String::new(), 0, 0) } }` — :424-431 수동 typefix 패턴 복제. `read_selection_text` 는 선택이 없으면 `None`(composition.rs:1584) 이므로 stale 제거를 위해 빈 값을 명시 저장. `atf_active`(:417) 이중 게이트 관례 유지 |
| `handle_key_down` 교체 지점(정상 경로 :672-750) | `drain_popup_actions`(:587) 직후 `let hanja_repl = engine.take_hanja_replacement();`. 정상 경로 블록(:672) 안에서 `if let Some(r) = hanja_repl { <교체> } else { <기존 :673-750 그대로> }`. `<교체>` = `let mut span = r.delete_chars; if comp_mgr.is_active() { comp_mgr.end_composition_keep_text(context, tid); span += r.preedit_chars; } let outcome = comp_mgr.replace_surrounding(context, tid, span, &r.text, "", comp_sink); match outcome { Normal => {}, PhaseSplit => schedule_flush = true, SynthBatch => engine.remove_preedit(), SynthHeadTail => { let _ = crate::synth_input::discard_pending_tail(); engine.remove_preedit(); } }` — :445-454(수동 typefix) 4갈래 바이트 동일 복제. `keep_text` + span 가산은 :861-867 역방향 ATF 의 CUAS 대응 패턴(clear SetText 가 무효인 앱에서도 정확) |
| 오버레이 폴백 경로(:600-670, `composition_unsupported`) | preedit 은 오버레이 창에만 있고 문서엔 없다 → `span = r.delete_chars` 만, 오버레이 hide 후 `replace_surrounding` 동일 4갈래. VM 검증 항목 |
| `apply_reverse_event`(:1130-1251) | `drain_popup_actions`(:1218) 직후 `if let Some(r) = engine.take_hanja_replacement() { match context { Some(ctx) => <교체, schedule_flush 대신 로그>, None => dbg_log("no context — hanja replacement dropped") } }` 그리고 이어지는 `commit_str()` 삽입(:1221-1249)은 비어 있어 no-op |
| `apply_reverse_event` 반환형 | **`()` 유지**(PM 지적 "확장 필요" 에서 벗어남, §deviations). 근거: 교체는 항상 `preedit_text=""` 라 `PhaseSplit`(native full 조합 전용, composition.rs:273-278)·`SynthHeadTail`(synth 순방향 꼬리 전용, :283-287) 이 발생하지 않는다 — :443-444 주석("수동 typefix 는 preedit="" 라 Normal 또는 SynthBatch 만 발생") 과 같은 조건. 두 arm 은 방어적으로 `discard_pending_tail`+`remove_preedit` 만 하고 타이머를 걸지 않는다. 시그니처를 넓히면 `rev_drain_and_apply`(text_service.rs:2415-2450) 와 `WM_UNIM_FLUSH2` 배선(:1451)까지 손대야 해 회귀면이 커진다 |
| OnSetFocus 캡처 래퍼(text_service.rs:1868) | `if engine.is_hanja_mode() { dbg_log("hanja popup dropped on focus change: recommit='{}'", engine.get_hanja_recommit()); }` 후 기존 `engine.reset()`. **재커밋은 하지 않는다**(§deviations): TSF 에서 preedit 은 문서 안의 composition 텍스트이고, 포커스 이탈 시 앱이 composition 을 terminate 하면 텍스트는 문서에 남는다(:1874-1876 "조합 객체는 보통 OnCompositionTerminated 가 이미 정리"). Linux 처럼 클라이언트 preedit 이 증발하는 구조가 아니므로 재커밋하면 **중복 삽입** 이 된다. 지도가 "유실" 이라 본 것은 엔진 측 target 이 사라진다는 뜻이고 문서 텍스트 유실 여부는 VM 실측 항목(§11). `reset()` 이 신규 필드까지 비우는 것으로 충분 |
| 핫리로드 | `maybe_reload_config`(text_service.rs:462-470) 가 `InputEngine::new` 로 재생성 → `new()` 한 줄이면 서식 반영. 별도 setter 호출 불필요 |
| 렌더러 | §7 |
| 검증 | `make check-windows`(Makefile:520-524, WIN_CRATES :491 — unim-capi 포함이라 `InputResult` 불변이 컴파일로 강제) |

---

## 6. 설정 (`hanja_output_format`)

| # | 지점 | file:line | 내용 |
|---|---|---|---|
| 1 | 코어 enum | src/config.rs:57-90 옆 | §2.3 `HanjaOutputFormat` |
| 2 | 코어 필드 | src/config.rs:673-675 `word_mode_apps` 다음 | `#[serde(default)] pub hanja_output_format: HanjaOutputFormat,` — 위치 근거: `commit_unit` 과 같은 "조합/변환 동작류 = KoreanConfig"(config-sync.md §7-Q2 결론) |
| 3 | `Default` | src/config.rs:677-689 | `hanja_output_format: HanjaOutputFormat::default()` |
| 4 | **Compat 브리지** | src/config.rs:790-817 `KoreanConfigCompat` 에 `#[serde(default)] hanja_output_format: HanjaOutputFormat`, `Default`(:819-834), `From`(:863-871) 에 복사 | `KoreanConfig` 는 `#[serde(from = "KoreanConfigCompat")]`(:605) 로만 역직렬화되므로 **여기 빠지면 YAML 값이 조용히 버려진다** (지도가 놓친 지점) |
| 5 | 엔진 캐시 | src/input_engine/engine.rs 필드 :187-198 옆 `pub(super) hanja_output_format: HanjaOutputFormat`; `new()`(:245-246 옆) ; `rebuild_korean_context()`(:947-950 옆) ; 신규 `pub fn set_hanja_output_format(&mut self, config: &Config)` 비파괴 setter | fingerprint(engine_worker.rs:382-397)에 **넣지 않는다** — ATF 핫키·전환키 선례(:371-373 주석). 리로드 루프 :983-988 에서 매번 호출 |
| 6 | 적용 | src/input_engine/candidates.rs `select_hanja`(:140-160) | `self.hanja_output_format.render(&self.hanja_target, &entry.hanja)` |
| 7 | CLI | unim-cli/src/main.rs:616-617 옆 `#[value(name = "hanja-output-format", help = h("help_ck_hanja_output_format"))] HanjaOutputFormat`; :74-80 옆 `hanja_output_format_display_name_localized`; `config set` arm :1701-1724 패턴(`"hanja" \| "한자"`, `"hangul-hanja" \| "한글(한자)"`, `"hanja-hangul" \| "한자(한글)"`); `config show` :886-891 옆 한 줄; 대화형 메뉴(:1806-1816)는 선례(commit_unit 미포함)대로 생략 | 레거시 `get_config/set_config` 도 선례대로 생략(config-sync.md §3.3) |
| 8 | CLI 로케일 | unim-cli/locales/ko.yml:44-45,88,106,120-122,378 대응 위치에 ko/en 같은 줄로: `hanja_output_format_label`, `error_invalid_hanja_output_format`, `hanja_output_format_changed`, `hanja_output_format_hanja`/`_hangul_hanja`/`_hanja_hangul`, `help_ck_hanja_output_format` | en.yml 동일 줄 번호 관례 |
| 9 | GTK 설정 | unim-settings-gtk/src/settings_dialog.rs:691-726 `commit_row` 복제 → `hanja_fmt_row`(ComboRow, 3항목) 바로 아래; save 라벨 `"hanja_output_format"`; 로케일 unim-settings-gtk/locales/{ko,en}.yml:35-36,164,194 옆 `row_hanja_output_format`/`_subtitle`/`_tooltip` + 3 값 라벨 | 3지 선택 = ComboRow 관례(:691 주석) |
| 10 | GTK merge 화이트리스트 | unim-gui-common/src/settings_helpers.rs:208 다음 `merge_field(&mut d.korean.hanja_output_format, bk.map(\|k\| &k.hanja_output_format), &u.korean.hanja_output_format);` | 빠지면 GTK 저장이 disk 값에 덮임 |
| 11 | Slint UI | unim-settings/ui/settings.slint:330-331 옆 `in-out property <[string]> hanja-output-format-options; in-out property <int> hanja-output-format-index;` + commit-unit ComboBox 옆에 동일 ComboBox(위치는 `rg 'commit-unit-index' unim-settings/ui/settings.slint`) | 표시값은 `display_name()`(코어, 한국어 리터럴 — commit_unit 과 동일 관례 main.rs:846) |
| 12 | Slint 바인딩 | unim-settings/src/main.rs:842-854 패턴(`set_hanja_output_format_options/_index`), :935-938 역변환, `merge_ui_owned` :578 다음 `merge_field(... hanja_output_format ...)` | GTK 와 별도 사본 — 양쪽 다 |
| 13 | TSF 레거시 모달 | unim-tsf/src/settings_dialog.rs:687-694 콤보 + :1247-1254 역변환 패턴, 새 `ID_CMB_HANJA_OUTPUT_FORMAT` | `fn_configure.rs:45`·`lang_bar.rs:799` 가 여전히 호출 → 활성 UI 로 취급 |
| 14 | DBus | 없음 — `GetConfigYaml/SetConfigYaml/GetConfigJson` serde 자동(service.rs:1161-1265) | `ConfigChangedJson` 도 자동 |
| 15 | GNOME prefs/gschema | 없음(일반 설정) | GEMINI.md 원칙 |
| 16 | 문서 | docs/user/user-guide/README-ko.md:375-383 §4.2 에 "출력 형식" 항목, README.md 대응 절(:227 부근 Hanja 설명) | §10 W10 |

YAML 예: `engine: { korean: { hanja_output_format: HangulHanja } }`. 구 config.yaml 은 필드 없음 → 기본 `Hanja` (`#[serde(default)]` + Compat default).

---

## 7. 팝업·렌더러

| 항목 | 파일:라인 | 수정 |
|---|---|---|
| 헤더 문자열 | src/popup/view_model.rs:308-320(expanded), compact 고정 헤더 | 무변경(문자열 연결) |
| GTK4 헤더 ellipsize | unim-popup-service/src/popup/hanja.rs:102-107 `target_label` | `target_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);` 추가(meaning_label :348 과 동일). CSS `max-width:420px` 는 유지 |
| GNOME 헤더 ellipsize | unim-gnome-extension/popup_view.js:98 `this._header` | `this._header.clutter_text.set_ellipsize(Pango.EllipsizeMode.END);` (:373 meaningLbl 과 동일 API) |
| Windows 헤더 | unim-popup-win/src/render.rs:375 | `DT_VCENTER \| DT_SINGLELINE \| DT_LEFT \| DT_END_ELLIPSIS` |
| Windows compact 한자 컬럼 | unim-popup-win/src/render.rs:504-505,514,520 (90px/96px 고정) | 루프 전에 `let hanja_col = rows.iter().map(\|c\| text_width(hdc, &format!("{} ★", c.t), font_main)).max().unwrap_or(0).max(s(90, scale));` → `hanja_rect.right = hanja_left + hanja_col`, `mean_left = hanja_left + hanja_col + s(6, scale)`. 팝업 폭 `max_w` 는 이미 실측 동적(:307-329) |
| Windows expanded 셀 | unim-popup-win/src/render.rs:197 `CELL_W = 44` | 그리드 그리기 진입 시 `cell_w = max(44, max text_width + 8)` 로 지역 변수화(다글자 후보 클리핑 방지). wire 프로토콜(popup_ipc.rs ↔ protocol.rs) **무변경** |
| GTK/GNOME expanded | CSS `.grid-cell{min-width:30px}` | 무변경(자동 확장). 열 정렬 불균일은 시각 이슈로 수용, L3/수동 확인 |
| 뜻이 한글 되풀이("대한민국") | src/popup/view_model.rs compact `meaning`(:358) | 선택 폴리시: `meaning == target` 이면 `None` 으로 — 선택 항목(WBS W2 옵션) |
| 즐겨찾기·재정렬·페이지 | 무변경 | 키가 단어 문자열이어도 `BTreeMap<String,…>`(src/hanja/bookmark.rs:57-66) 그대로 |

---

## 8. POPUP_SPEC 개정안 초안 (파일 미수정 — `docs/dev/specs/HANJA_WORD_SPEC.md` 의 "POPUP_SPEC 개정안(승인 대기)" 절에 그대로 실을 문구)

> **§3.7 동작 규칙 — 2번 교체**
> 2. **대상**: 다음 우선순위로 결정한다.
>    (a) 조합 중이고 확정 단위가 음절이면, 엔진이 기억하는 최근 커밋 한글 음절(최대 17자, 어절 경계에서 리셋)과 현재 preedit 을 이어 붙인 문자열의 접미 중 사전에 있는 가장 긴 것(최소 2자). 없으면 preedit 의 마지막 음절(예: "대한민국" → "국").
>    (b) 조합 중이고 확정 단위가 단어(누적 preedit)이면, preedit 전체의 접미 중 사전에 있는 가장 긴 것. 접미 앞의 나머지 preedit 은 확정 시 그대로 함께 커밋한다.
>    (c) 조합 중이 아니고 앱 선택 영역이 있으면, 선택 텍스트(앞뒤 공백 제외)가 한글 완성 음절만으로 이루어진 18자 이하이고 사전에 정확히 있을 때 그 문자열. 조건을 만족하지 않으면 팝업 없이 키를 소비한다(이모지 팝업으로 넘어가지 않는다).
>    (a)에서 접미가 이미 커밋된 글자를 포함하면, 선택 확정 시 `AutoTypefixApply(delete_chars=이미 커밋된 글자 수, commit_text=서식 적용 문자열, preedit_text="")` 로 교체한다(CommitText 대신). 단음절 대상은 종전과 같이 CommitText 를 쓴다.

> **§3.7 동작 규칙 — 4·5번 보강**
> 4. **선택 시**: `SelectHanja(globalIndex)` → 엔진이 서식 적용 문자열(설정 `hanja_output_format`: 漢字 / 한자(漢字) / 漢字(한자))을 반환 → 프론트엔드가 커밋. 대상 (a)에 커밋된 글자가 포함되면 `AutoTypefixApply` 시그널로 교체(위 2번).
> 5. **취소 시**: `CancelHanja()` → 팝업 진입 때 지운 preedit(음절 모드: 마지막 음절, 단어 모드: 누적 preedit 전체)을 커밋 → 팝업 닫기. 이미 앱에 나간 글자와 선택 영역은 건드리지 않는다.

> **§9.2 idle Hanja 키 dispatch 정책 (v3.2) — 예외 추가**
> > (v3.4) 조합 idle 이더라도 앱 선택 영역이 존재하면(`SetSurroundingText` 의 `cursor_pos != anchor_pos`) emoji popup 을 열지 않고 §3.7-2(c) 선택 영역 한자 변환을 시도한다. 조건 불충족 시 키만 소비한다. 선택 영역을 전달하지 못하는 프론트엔드(GTK3·XIM)는 종전과 같이 emoji popup 이다.

> **§6.1 DBus (unim-dbus/SPEC.md:255 `SelectHanja` 행 주석)**
> `SelectHanja` 의 반환 `s` 는 서식 적용 문자열. 데몬은 대상에 커밋된 글자가 포함되면 `CommitText` 대신 `AutoTypefixApply(u,s,s)` 를 popup-owner path 로 발행한다. 시그니처 변경 없음.

> **§11 변경 이력 행 추가**
> | 2026-09-XX | **v3.4** | **한자 단어 변환 — 대상 결정을 최근 커밋 음절+preedit 최장 접미 / 단어 모드 preedit 접미 / 앱 선택 영역으로 확장(§3.7-2). 확정 시 `AutoTypefixApply` 교체 채널 재사용. 출력 형식 설정 `hanja_output_format`. idle Hanja 정책에 선택 영역 예외(§9.2). 헤더 ellipsize·Windows compact 컬럼 동적 폭.** |

---

## 9. 테스트 계획

### 9.1 L1 — `src/input_engine/tests_hanja_word.rs` (mod.rs:26-45 에 `#[cfg(test)] mod tests_hanja_word;` 등록, `create_test_engine`(test_helpers.rs:9-12) 사용)
두벌식 키(press_key.rs 실사용 시나리오 tests_scenarios.rs:283-288 관례): 대한민국 = `E O G K S A L R N R`(ㄷ ㅐ ㅎ ㅏ ㄴ ㅁ ㅣ ㄱ ㅜ ㄱ). 기대 사전 상태: `대한민국:大韓民國` 단일 항목(hanja.txt:57963), `뷁` 무항목.

| # | 케이스 | 검증 |
|---|---|---|
| 1 | 정상: 대한민 커밋+국 조합 → 한자키 | `recent_syllables=="대한민"`, `preedit_str()=="국"`, `start_hanja_conversion().hanja_candidates_available`, `get_hanja_target()=="대한민국"`, committed_chars 3 |
| 2 | 다음절 불일치 폴백: 뷁 + 국 | target "국", committed 0 (현행 바이트 동일) |
| 3 | 확정(교체): 1 에서 `select_hanja(0)` | 반환 `"大韓民國"`, `take_hanja_replacement()==Some{3,1,"大韓民國"}`, `commit_str()==""`, preedit 비움, `!is_hanja_mode()`, recent 비움 |
| 4 | 확정(push 경로): `press_key(Num1)` | `pending` Some, `commit_str()==""`, 결과 `consumed && preedit_changed`, `take_popup_action()==HidePopup` |
| 5 | 서식 3종(D2): `set_hanja_output_format` 각각 후 단음절 "가"→후보0 선택 | `"{h}"`, `"가({h})"`, `"{h}(가)"` (h=`get_hanja_candidates()[0].0`) |
| 6 | 서식+단어 | `"대한민국(大韓民國)"`, `"大韓民國(대한민국)"` |
| 7 | 취소 3경로: (a) `press_key(Escape)` → `commit_str()=="국"`; (b) `cancel_hanja()` 직접 → 필드 clear, `get_hanja_recommit()` 사전 값 "국"; (c) NotHandled(Num0 이 아닌 예: `KeyCode::Q` 자모) → "국" 커밋 후 새 preedit | recent 전부 비움 |
| 8 | Word 모드: `set_word_mode(true)`, 오늘대한민국 | target "대한민국", prefix "오늘", `select_hanja` → `"오늘大韓民國"`, `pending==None`, `commit_str()` 에 push(`press_key(Num1)` 후 `"오늘大韓民國"`) |
| 9 | Word 모드 취소 | `commit_str()=="오늘대한민국"` (현행은 "국" 만 — 의도된 수정, §11) |
| 10 | Backspace pop | 대한민+국 → BS×3(조합 소진) → BS(not_consumed) → recent "대한" ; 선택 있을 때(`set_surrounding_text("x",0,1)`) BS → clear |
| 11 | 리셋 조건 각 1케이스: Space·Enter·Tab·Escape·Left·Home·Delete·Ctrl+A·토글키·영문자(영문모드)·특수문자 자모·이모지/한자 확정 후·`reset()`·`set_input_category` | recent 빔 |
| 12 | 한자키 자체는 리셋 아님 | 1 후 recent 유지(팝업 중) |
| 13 | 비밀번호: 대한민 입력 후 `set_content_purpose(Password)` | recent 빔; 이후 `start_hanja_conversion()` → `consumed`, `!is_hanja_mode()`(pull 경로 fail-closed); `set_content_purpose(Normal)` 후 새 입력부터 정상 |
| 14 | 대상②: `set_surrounding_text("나는 대한민국 사람", 3, 7)` idle → `press_key(F9)` | hanja_mode, target "대한민국", committed 0, recommit ""; `select_hanja(0)`→`"大韓民國"`, pending None; push 경로면 `commit_str()=="大韓民國"` |
| 15 | 대상② 거부: 불일치("abc 뷁뷁" 선택)·비한글 포함·19자·공백만 | `press_key(F9)` → `consumed`, `!is_hanja_mode()`, `!is_emoji_popup_active()` |
| 16 | 선택 없음 idle → 이모지(회귀) | `is_emoji_popup_active()` |
| 17 | 상한 17: 20음절 입력 | `recent_syllables.chars().count()==17` |
| 18 | soft 검증 실패: recent "대한민", `set_surrounding_text("xyz",3,3)` | target "국" |
| 19 | soft 검증 통과(GTK 형: "…대한민"), TSF 형("…대한민국") | target "대한민국" |
| 20 | 초성 특수문자 폴백 회귀: "ㄱ" → 특수문자 | 현행 |
| 21 | `HanjaCandidatesReordered.target == "대한민국"` (즐겨찾기 토글, 임시 경로 bookmark store) | 키 일치 |
| 22 | chord idle flush pending 이 음절을 push | `chord_idle_flush_pending` 후 recent |
| 23 | config serde: 필드 없는 YAML → `Hanja`; `hanja_output_format: HanjaHangul` → 파싱(Compat 경유) | `Config::load_from_str` 상당 |
| 24 | 회귀: `cargo test --workspace` (ATF tests.rs 1266줄·tests_scenarios·tests_popup_change_page 포함) 전량 통과, 경고 0 | Zero Tolerance |

### 9.2 L2 — `tests/unim-test-dbus/src/main.rs` (`test_hanja_popup` :166-262 확장)
- `test_hanja_word_popup`: `focus_in`, 두벌식 evdev `E(18) O(24) G(34) K(37) S(31) A(30) L(38) R(19) N(49) R(19)` → Hanja(123) → `get_hanja_candidates()` target=="대한민국" → `ic.receive_auto_typefix_apply()` 스트림(unim-dbus/src/client.rs:259-264) 구독 후 `select_hanja(0)` → `(3, "大韓民國", "")` 수신(타임아웃 2s). 세벌식 레이아웃 분기는 기존 `Layout` 매치 관례.
- `test_hanja_selection_popup`: `set_surrounding_text("대한민국", 0, 4)`(client.rs:173) → `process_key_event(0,123,0)` → `get_hanja_candidates()` target "대한민국" → `receive_commit_text()` 구독 후 `select_hanja(0)` → CommitText "大韓民國".
- `test_hanja_selection_no_emoji`: `set_surrounding_text("뷁뷁",0,2)` → Hanja → `get_hanja_candidates()` 빈 응답, ShowEmojiPopup 미발행(짧은 타임아웃으로 부재 확인).
- 서식: `SetConfigYaml` 로 `HangulHanja` 후 위 첫 케이스 재실행 → `"대한민국(大韓民國)"` (선택).

### 9.3 L3 — `tests/harness/scenarios/hanja_word.json` (2bulstd 전용)
```json
[{ "name": "hanja-word-syllable-mode", "korean": true, "field": "core.plain", "layout": "ko_2bulstd",
   "steps": [
     { "keys": ["e","o","g","k","s","a","l","r","n","r"], "expect": { "preedit": "국", "committed": "대한민" } },
     { "key": "F9" },
     { "key": "1", "expect": { "preedit": "", "committed": "大韓民國", "rendered": "大韓民國" } } ] },
 { "name": "hanja-word-selection", "korean": true, "field": "core.plain",
   "steps": [
     { "keys": ["e","o","g","k","s","a","l","r","n","r"] }, { "key": "space" }, { "key": "Left" },
     { "key": "shift+Home" },
     { "key": "F9" }, { "key": "1", "expect": { "committed": "大韓民國 ", "rendered": "大韓民國 " } } ] }]
```
- 판정은 `field.render` 의 `preedit/committed/rendered` 3키(harness.py:353-356)만으로 가능(docs-rules-tests.md 보충#11(1)). `F9`·`1` 은 xdotool 키명 그대로.
- 전제: 테스트 환경 즐겨찾기 파일이 비어 첫 후보가 `大韓民國`(단일 항목이라 무관).
- `hanja-word-selection` 은 `core.plain` 필드가 **GTK4 IM 경로**일 때만 의미가 있다(GTK3 는 anchor 를 못 보내 idle 이모지로 떨어짐). harness 의 앱 스펙(harness.py:35-86 `xtest`/툴킷 표)에서 GTK4 앱을 골라 `field` 를 지정하고, 없으면 이 시나리오는 `known_fail` 이 아니라 생략한다.
- `commit_unit` 시나리오 자동 적용 확장(harness.py:398-409 `layout` 패턴 복제 → `config: {"commit_unit": ...}`): Linux 기본이 Smart==Syllable 이라 위 시나리오엔 불필요 → **선택**(D11 "비용이 크면 선택").
- Wayland 네이티브 앱은 harness 가 스킵(harness.py:388-390) → GNOME 선택 치환은 수동 실측 항목.

### 9.4 Windows
- `make check-windows` 그린(Makefile:520-524). 런타임(VM) 검증 항목: TSF 키보드/마우스 교체, CUAS(synth) 경로 다글자 삭제, 오버레이 폴백, OnSetFocus 시 문서 텍스트 보존, 렌더러 컬럼 폭.

---

## 10. 작업 분해 (WBS)

파일 소유권은 겹치지 않는다. 병렬 가능: {W1} → {W2} → {W3, W7, W11}; {W4, W5, W6, W8, W9, W10} 은 W1/W2 API 명세만 보고 즉시 병렬 착수 가능(W9 는 W1 의 enum 이름·`all()`·`display_name()` 계약, W3/W7 은 W2 의 `take_hanja_replacement`/`get_hanja_recommit`/`set_hanja_output_format` 시그니처).

| id | 단위 | 파일(소유) | 선행 | 난이도 | 검증 |
|---|---|---|---|---|---|
| W1 | 설정 enum·필드·Compat | `src/config.rs` | — | 쉬움 | `cargo test -p unim config::` + 케이스 23 |
| W2 | 코어 상태기계·알고리즘·서식·테스트 | `src/input_engine/hanja_word.rs`(신규), `engine.rs`, `press_key.rs`(래퍼·inner 개명), `popup_dispatch.rs`(:184 inner 호출, popup_select/cancel), `candidates.rs`(target 결정·select/cancel·pull 게이트), `surrounding.rs`(:38 clear), `types.rs`(HanjaReplacement), `mod.rs`(mod 등록), `tests_hanja_word.rs`(신규), `src/hanja/dict.rs`(`contains`), (옵션) `src/popup/view_model.rs` | W1 | 어려움 | 케이스 1-24, `cargo test --workspace` 경고 0 |
| W3 | Linux 데몬 배선 | `unim-dbus/src/service.rs`(SelectHanja 응답형·RPC·redirect 헬퍼), `unim-dbus/src/engine_worker.rs`(drain·auto_typefix 재사용·:1841 게이트·recommit 접근자 2곳·리로드 setter) | W2 | 쉬움 | `cargo build -p unim-dbus`, L2 |
| W4 | Wayland surrounding 저장·전달·실측 바이트 | `unim-frontends/wayland/src/state.rs`, `dbus_client.rs` | — | 어려움(프로토콜 더블버퍼) | 빌드 + 수동(sway/KDE) ATF 회귀 + 한자(漢字) 형식 교체 |
| W5 | Qt F9 재질의 | `unim-frontends/qt5/src/input_context.cpp`, `qt6/src/input_context.cpp` | — | 쉬움 | 빌드 + 수동 |
| W6 | XIM preedit 선클리어 | `unim-frontends/xim/src/handler.rs` | — | 쉬움 | 빌드 + 수동(마우스 확정) |
| W7 | TSF 선택 읽기·교체 2지점·OnSetFocus 로그 | `unim-tsf/src/key_handler.rs`, `unim-tsf/src/text_service.rs` | W2 | 어려움 | `make check-windows`(VM 런타임은 별도) |
| W8 | 렌더러 3종 | `unim-popup-service/src/popup/hanja.rs`, `unim-gnome-extension/popup_view.js`, `unim-popup-win/src/render.rs` | — | 쉬움 | 빌드, 긴 target 수동 확인, `make check-compat`(GNOME) |
| W9 | 설정 동기화 8지점 | `unim-cli/src/main.rs`, `unim-cli/locales/{ko,en}.yml`, `unim-settings-gtk/src/settings_dialog.rs`, `unim-settings-gtk/locales/{ko,en}.yml`, `unim-gui-common/src/settings_helpers.rs`, `unim-settings/src/main.rs`, `unim-settings/ui/settings.slint`, `unim-tsf/src/settings_dialog.rs` | W1 | 쉬움(기계적) | 빌드 경고 0, `unim-cli config set/show` 왕복, GTK/Slint 저장 후 YAML 확인 |
| W10 | 문서 | `docs/dev/specs/HANJA_WORD_SPEC.md`(신규, §8 개정안 포함), `CHANGELOG-ko.md`/`CHANGELOG.md`(한 줄·명사형: "한자 단어 변환 — 방금 입력한 어절·선택 영역을 한자 단어로 변환, 출력 형식(漢字/한자(漢字)/漢字(한자)) 설정 추가" / "Hanja word conversion for the just-typed word or the app selection, with output format setting (漢字 / 한자(漢字) / 漢字(한자))"), `docs/user/user-guide/README-ko.md` §4.2, `README.md` 대응 절, `ROADMAP.md:118-127` ① 상태, `unim-dbus/SPEC.md:255` 주석 | — | 쉬움 | 리뷰 |
| W11 | L2·L3 | `tests/unim-test-dbus/src/main.rs`, `tests/harness/scenarios/hanja_word.json`, (선택) `tests/harness/harness.py` config 확장 | W2, W3 | 쉬움 | `make test-dbus`류 기존 타깃, L3 러너 |
| W12 | 통합 검증 | — | 전부 | 쉬움 검증(opus) | `cargo build --workspace` 경고 0, `cargo test --workspace`, `make build`, `make check-windows`, L2/L3 그린 |

---

## 11. 위험·롤백·미해결

### 11.1 위험표
| 위험 | 영향 | 완화 |
|---|---|---|
| 버퍼 드리프트(마우스 편집·메뉴 Undo·앱 자동교정) → 잘못된 글자 삭제 | 문서 손상 | §3.2.2 soft 검증(surrounding 있는 GTK3/4·Qt(재질의)·GNOME·Wayland(배선 후)·TSF). XIM 은 검증 불가 — 통과 키 전량 리셋으로 창을 좁힘. 잔여 위험 수용 |
| Wayland 폴백(surrounding 미지원 앱) + `한자(漢字)` 형식 | 1/3 만 삭제 | 실측 경로 우선. 폴백에서 형식이 `HangulHanja` 면 `delete_chars*3` 강제(삭제 대상은 항상 한글) — W4 에 포함 |
| GNOME `commit()` 이 선택을 치환하지 않는 앱 | 선택 옆 삽입 | 실측 후 필요 시 `_lastSurrounding` anchor 로 `delete_surrounding` 선호출(v1.1) |
| 사전 순서 ≠ 빈도(popup-pipeline.md 보충#7(3)) | 후보 품질 | 즐겨찾기로 보완, v2 랭킹 |
| 단어 즐겨찾기 재사용률 낮음 | UX | 수용(D7) |
| Word 모드 취소/확정 동작 변화(현행은 preedit 접두를 잃음) | 개선이나 동작 변화 | 케이스 8-9 명시, CHANGELOG 언급 |
| TSF OnSetFocus 재커밋 미실시 | 만약 앱이 composition 텍스트를 버리는 경우 유실 | VM 실측 후 결정(현재 근거상 텍스트 보존) |
| TSF CUAS synth 로 다글자 삭제 | 앱별 편차 | 기존 ATF 와 동일 경로 |
| ATF 와의 간섭 | 이중 교체 | 팝업 활성 중 ATF 게이트(engine_worker.rs:1696-1699, key_handler.rs:753-754) + 생산자 상호배제(§4.3) |
| pull 경로 비밀번호 노출 | 정보 노출 | `start_hanja_conversion` 첫머리 게이트 + setter 측 clear; XIM 은 목적 통지 자체가 없어 구조적 잔여(현행 preedit 노출과 동급) |
| 헤더 20자 오버플로 | 시각 | ellipsize 3종 |

### 11.2 롤백
- 코어: 다음절 일치가 없으면 `resolve_hanja_target` 이 현행 결과와 바이트 동일한 spec 을 내므로, 긴급 시 `longest_dict_suffix` 호출 두 곳을 `min_len` 초과값으로 막으면 기능이 꺼진다(한 줄). 설정 필드는 `#[serde(default)]` 라 남겨도 무해.
- 프런트: W4/W5/W6/W8 은 각각 독립 revert 가능(다른 단위와 API 의존 없음).
- 데몬: W3 의 응답형 변경은 컴파일로 강제되므로 W2 와 함께 revert.

### 11.3 기능 스위치를 두지 않는 이유
`hanja_word_conversion: bool` 을 두면 설정 동기 지점이 9곳 늘고 L1/L2 매트릭스가 두 배가 된다. 기능은 "다음절 일치 있음 ∧ 검증 통과" 에서만 발동하고 그 외는 현행이라, 스위치 없이도 회귀면이 좁다. 사용자 요구가 생기면 v1.1 에서 W1+W9 만으로 추가 가능.

### 11.4 미해결(설계로 확정 못 한 것)
1. GNOME `vfunc_set_surrounding` 호출 빈도(Mutter 소스 밖) — soft 검증의 오탐률. 실측 필요.
2. GNOME/Wayland 앱에서 commit 이 선택을 치환하는지(툴킷별) — L3 는 Wayland 네이티브를 스킵하므로 수동.
3. TSF: 포커스 이탈 시 composition 텍스트 보존 여부(앱별) — VM.
4. Qt 의 `ImCursorPosition/ImAnchorPosition` 이 UTF-16 코드유닛 단위일 가능성(selection-surrounding.md §7-5) — 서로게이트 문자가 선택 앞에 있으면 오프셋 어긋남. 한글 완성 음절만 허용하는 대상② 판정이 대부분을 거르지만, 앞 문맥에 이모지가 있으면 선택 슬라이스가 어긋나 "불일치 → 무동작" 으로 퇴화(안전).
5. `한자(漢字)` 서식의 괄호를 ASCII 로 확정했다 — 전각 `（）` 선호 여부는 기현님 확인.
6. 선택 영역 앞뒤 공백을 trim 해 조회하되 치환은 위젯이 선택 전체를 바꾼다(공백 소실) — 수용 여부.
