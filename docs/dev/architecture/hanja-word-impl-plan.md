# 한자 단어 입력 — 최종 구현 계획 (PLAN)

- 기준: develop `ba64255`. 골격 = **A-engine**(심사 합계 1위, press_key 단일 래퍼·설정 16지점·상호배제표). 심사가 지목한 A 의 결함 5개(키열·GTK4/Qt 래퍼·Wayland 폴백·공백·비밀번호 중 팝업)를 고치고 B/C 의 graft 를 이식했다.
- 스펙: `/home/from104/work/unim/docs/dev/specs/HANJA_WORD_SPEC.md`(저장소, 승인 대기). 이 계획은 그 스펙의 구현 지시서다.
- 모든 `file:line` 은 이 세션에서 실측했거나 세 설계안·심사가 교차 확인한 값. 구현자는 편집 전 `rg` 로 재확인한다(라인은 앞선 편집으로 밀릴 수 있다).
- 커밋 없음(D12). develop 작업 트리에서 그대로.

---

## 1. 결정 요약 — 설계안 간 차이 해소

| # | 쟁점 | A | B | C | **최종** | 근거 |
|---|---|---|---|---|---|---|
| 1 | 버퍼 훅 방식 | press_key 래퍼 단일 초크포인트 + `chord_idle_flush_pending` 훅 | push 5곳 + 비한글 9곳 + not_consumed 공통 개별 훅 | 래퍼 + `chord_idle_flush_commit`(오류) | **A** | B 는 한 곳만 빠져도 delete_chars 가 문장부호를 지운다(B 스스로 '필수'). C 의 `chord_idle_flush_commit`(engine.rs:820) 은 reset 전용(engine_worker.rs:733)이라 무의미 — 실제 idle 경로는 `chord_idle_flush_pending`(engine.rs:844, engine_worker.rs:2633) |
| 2 | KoreanConfigCompat | 필수 | '무변경'(오류) | 필수 | **A/C** | `#[serde(default, from = "KoreanConfigCompat")]`(config.rs:605) + `From` 명시 복사(:836-872) — Compat 에 없으면 YAML 값 폐기 |
| 3 | GTK4/Qt `if (result.consumed)` 선택 삭제 래퍼 | 미인지 | 게이트(F1/F2b) | 미인지(오히려 의존) | **B** | gtk4 immodule.c:1061·qt5 :446 실측 — 빈 소비 키(F9·Esc·내비)에서 선택이 지워져 D8 위반. `consumed && (commit≠"" \|\| preedit≠"")` 게이트 |
| 4 | Wayland 바이트(D9) | SurroundingText 실측 + 폴백 '형식이면 ×3'(프런트는 config 모름) | commit_text 한자 판별자 → ×3 | 프런트 커밋 이력 상태기계 | **B(판별자)** + SurroundingText 는 대상②/검증 용도로만 배선 | 삭제 대상은 구성상 항상 완성 음절(3B) → 추정 아님. ATF commit_text(한글/ASCII)와 서로소 → 회귀 0. C 는 포워딩 키 1회로 이력 무효 → 같은 포커스 안에서 재발동 시 1/3 삭제 |
| 5 | 판별자 정의 | — | CJK 3구간 범위 | — | **"ASCII 도 한글도 아닌 문자 포함"** | CJK 확장 B(U+20000+) 등 범위 밖 한자도 커버. ATF 와 서로소 유지 |
| 6 | 정합성 검증(soft) | 매치 접두를 surrounding 과 대조, 3변형(GTK/TSF/잘림) | 버퍼 전체 단일 `ends_with` | 없음 | **A** + TSF 변형은 `surrounding_includes_preedit` 플래그 게이트 | B 는 TSF(조합 포함 surrounding)에서 항상 실패. C 는 Wayland/XIM 클릭 드리프트 무방비. A 의 TSF 변형(`prefix+pre`)을 프런트 구분 없이 두면 Wayland/GNOME 클릭 드리프트가 그 절로 통과한다(§2.4) |
| 7 | 취소 텍스트 API | `get_hanja_recommit()` | `get_hanja_restore_text()` | `hanja_cancel_text()` = prefix+preedit(중복 버그) | **`hanja_cancel_text()` 이름 + A 의미(= 진입 시 preedit 전체, Selection "")** | C 의 합성은 Word 모드에서 "오늘오늘대한민국" 중복(심사 2). 접두는 recommit 에 이미 포함 |
| 8 | 데몬 SelectHanja 응답 | `Option<(u32,String)>` | `Option<(String,u32)>` | `SelectHanjaOutcome{None,Commit,Replace}` | **C** | 의도가 드러나고 delete_chars=0 오용을 컴파일로 막음 |
| 9 | TSF `apply_reverse_event` 반환형 | `()` 유지 | `bool` + 타이머 추출 | `bool` + `rev_drain_and_apply(ctx,hwnd)` | **A(반환형) + 매개변수 2개 추가** | preedit="" 교체는 PhaseSplit/SynthHeadTail 미발생(key_handler.rs:443-444). 단 현행 시그니처(key_handler.rs:1130-1140)에는 오버레이 상태(`composition_unsupported`·`preedit_window`, text_service.rs:80/:64 서비스 필드)가 없어 U12 (b) 공용 함수를 마우스 경로에서 부를 수 없다 → `composition_unsupported: bool, preedit_win: &mut Option<PreeditWindow>` 추가, 호출부 text_service.rs:2436 이 기존 락 순서 뒤에 전달. "변경 최소화" 는 반환형·타이머 구조에 한함 |
| 10 | TSF OnSetFocus | 로그만, 재커밋 안 함 | send_hide + keep_text + cancel | cancel + 오버레이 폴백 insert | **A + C 오버레이 분기** | text_service.rs:1878 `popup_ipc.hide()` 이미 존재 → send_hide 불필요. 정상 앱은 조합 텍스트 문서 잔존(재커밋=중복). 오버레이 앱만 best-effort |
| 11 | TSF 대상② 선택 읽기 | idle 일 때만, 없으면 빈 값 저장 | 조합 중에도 읽기 + `DoEditSession :1583-1585 IsEmpty` 조기 종료 완화(`read_selection_text` 자체는 :1637) | idle 일 때만 | **A/C** + 조합 중엔 빈 값 저장 | IsEmpty 완화는 CUAS 앱 정확도 미검증. 조합 중 빈 값 → 검증 생략(통과) 으로 결정적 동작. 완화는 v1.1 |
| 12 | 대상② 공백 | trim 후 소실(미해결) | 되붙임 | 되붙임 | **B/C** | 위젯이 선택 전체를 치환 |
| 13 | 혼합 후보 리스트(단어 뒤 음절) | 없음 | 있음(deviation 1) | 없음 | **기각(v2 축소 키)** | D5/D7 이탈 + `hanja_candidates` 타입 변경 파급(engine.rs:119/260/773, engine_worker.rs:1998, tests_popup_change_page.rs). 스펙 Q7 로 PM 판단 |
| 14 | 다음절 뜻 합성 | 없음 | 있음(deviation 2) | 없음 | **선택 단위(U2b)** | dict.rs 만 수정·렌더러 무수정·國家/國歌 구분. 스펙 Q5 |
| 15 | 헤더 긴 target | 렌더러 ellipsize 3종 | 데몬 축약 + ellipsize | ellipsize 3종 | **ellipsize 3종** | view_model 무수정으로 회귀면 최소 |
| 16 | 비프 | 없음(설비 부재) | `take_ui_feedback` + `toggle_announce_beep` 게이트 | 없음 | **선택(U11 옵션)** | 기본 무음. 스펙 Q6 |
| 17 | 비밀번호 목적이 팝업 중 도착 | 팝업 유지(결함) | cancel + 재커밋 생략 | 미처리 | **B** | 비번 필드에 원문 재삽입 금지 |
| 18 | XIM/GNOME preedit 선클리어 | XIM 만 | XIM+GNOME | 불필요 | **XIM+GNOME(1줄씩)** | 마우스 확정은 ProcessKey 응답이 없어 잔상. 키보드 경로엔 no-op |
| 19 | Qt F9 재질의 | 있음 | 있음(+UTF-16→문자 변환 명시) | 있음 | **B** | `text.left(pos).toUcs4().size()` |
| 20 | 한자키 dispatch 위치 | press_key.rs:226-238 유지 | candidates.rs `on_hanja_key` 로 이동 | 유지 | **유지** | POPUP_SPEC §9.2 가 규율하는 지점을 옮기지 않는다 |
| 21 | 키열 | `E O G K S A L R N R`(오류: 대한미국) | 11키 | 11키 | **`E O G K S A L S R N R`** / evdev `18 24 34 37 31 30 38 31 19 49 19` | 민 = ㅁ(A)ㅣ(L)ㄴ(S) |
| 22 | Windows 레거시 모달 | 포함 | 포함 | 선택 | **포함** | `fn_configure.rs:45`·`lang_bar.rs:799` 가 호출하는 활성 UI |
| 23 | 프런트 SPEC.md **7종**(gtk3/4·qt5/6·xim·wayland·**gnome**) | 없음 | 없음 | 있음(6종) | **C + GNOME** | 각 프런트 단위가 자기 SPEC 을 갱신(CONTRIBUTING.md:66-72 즉시 반영 규칙). `unim-gnome-extension/POPUP_SPEC.md` 는 리디렉트 stub 이라 무수정 |
| 26 | 대상② 불일치 동작 | D6 무동작 | — | — | **D6 유지, 스펙 Q8 로 PM 재판단** | 검증이 지적한 select-on-focus 위젯의 죽은 키·이모지 영구 차단. 대안 = 이모지 폴백 / 팝업 중 한자키 재타 전환 |
| 27 | XIM·surrounding 미지원 Wayland 의 클릭 드리프트 | 헤더 확인 후 Esc | — | — | **XIM 스팟 점프 Reset(v1, U5) + Wayland 는 스펙 Q9(b) PM 판단** | XIM 은 :918 스팟 보고 지점이 이미 있어 1분기 추가. **판정 기준 = "IM 이 유발하지 않은 스팟 갱신"**(핸들러가 commit/preedit_draw 직후 `expect_spot_update=true`, 기대 없이 온 보고 ∧ idle → Reset; 보조로 y 변화·x 감소 무조건) + idle 구간당 1회 디바운스. Reset 은 값싸지 않다 — `reset_engine_and_capture_commit`(engine_worker.rs:1923-1935 → :733 chord 강제 flush, :757 `InputEngine::new` → engine.rs:243 `HanjaDictionary::new()` 사전 재파싱 ≈6.45MB) — 디바운스가 상한, 실측 문제 시 경량 RPC `ClearRecentSyllables`(v1.1). Wayland 축소는 D1 을 건드리므로 PM |
| 28 | IMM32·`unim-capi` 회귀(코어 동작 변화가 무변경 호스트로 흘러듦) | '무변경' | — | — | **코어 호스트 능력 플래그 `hanja_word_replace_capable`(기본 false)** | unim-imm32/src/input.rs:110-112 은 Hanja/F9 를 consume 해 :181 `press_key` 로 넘기고 lib.rs:224 `ImeToAsciiEx` 는 commit/preedit 만 드레인(`rg popup_action\|take_hanja unim-imm32/src` = 0건, ui_window.rs:7 후보창 스텁). 종전 blind 한자 모드(F9→숫자→commit "國")가 RecentWord 확정에서 pending 페이로드+`preedit_updated()` 로 바뀌면 "국" 소실·pending 영구 미드레인. 플래그 false 면 `resolve_hanja_target` 이 `l > p` 접미를 건너뛰어 바이트 동일. 데몬 엔진 생성 헬퍼·TSF 생성 직후(text_service.rs:187/:463) true |
| 24 | `is_hangul_syllable` | 신규 헬퍼 | 신규 헬퍼 | `HangulCharExt::is_hangul_syllable`(char.rs:785) 재사용 | **C** | 이미 존재 |
| 25 | TSF 한자키 판정 | `Hanja \|\| F9` 하드코딩 | `engine.is_hanja_key()` | 하드코딩 | **B** | config 기반 1줄 접근자 |

---

## 2. 데이터 모델·알고리즘 최종

### 2.1 설정 (`src/config.rs`)

```rust
// CommitUnit(:57-90) 바로 아래
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum HanjaOutputFormat { #[default] Hanja, HangulHanja, HanjaHangul }
impl HanjaOutputFormat {
    pub fn all() -> &'static [Self] { &[Self::Hanja, Self::HangulHanja, Self::HanjaHangul] }
    pub fn display_name(&self) -> &'static str { "한자만" | "한글(한자)" | "한자(한글)" }   // 코어 폴백 라벨, UI 는 i18n
    pub const OPEN: &'static str = "("; pub const CLOSE: &'static str = ")";              // Q2 전각 전환 대비 상수
    pub fn render(&self, hangul: &str, hanja: &str) -> String {
        match self { Hanja => hanja.into(),
                     HangulHanja => format!("{hangul}{}{hanja}{}", Self::OPEN, Self::CLOSE),
                     HanjaHangul => format!("{hanja}{}{hangul}{}", Self::OPEN, Self::CLOSE) }
    }
}
// KoreanConfig(:606-) commit_unit 옆:  #[serde(default)] pub hanja_output_format: HanjaOutputFormat,
// Default(:677-689) 에 추가. KoreanConfigCompat(:790-) 에 #[serde(default)] hanja_output_format: HanjaOutputFormat,
// Compat Default(:819-834) 에 추가, From(:836-872) Self { ..., hanja_output_format: c.hanja_output_format }
```

### 2.2 엔진 필드 (`src/input_engine/engine.rs` :123 `hanja_target` 옆, `new()` :229 리터럴, `reset()` :764)

| 필드 | 타입 | 의미 / 종전 단음절에서의 값 |
|---|---|---|
| `recent_syllables` | `String` | 최근 확정 음절 버퍼(≤17자) |
| `hanja_source` | `HanjaSource` | `Syllable`(종전) / `RecentWord` / `WordBuffer` / `Selection` |
| `hanja_committed_chars` | `u32` | 확정 접두 길이. `RecentWord` 만 >0 |
| `hanja_recommit` | `String` | 취소 시 재커밋 = 진입 시 `preedit_cache` 전체. Selection "". 종전: target 과 동일(바이트 동일) |
| `hanja_commit_prefix` / `hanja_commit_suffix` | `String` | 확정 문자열 앞/뒤에 붙일 문자열(Word 비일치 접두 / Selection 앞뒤 공백). 종전 "" |
| `pending_hanja_replacement` | `Option<HanjaReplacement>` | out-of-band 페이로드. `take_hanja_replacement()` 로 drain(`take_atf_toggle` :604 선례) |
| `hanja_output_format` | `HanjaOutputFormat` | config 캐시. `new()`·`rebuild_korean_context()`(:933) 에서 세팅 + 비파괴 `set_hanja_output_format(&Config)`(:613 `set_atf_hotkeys` 옆) |
| `pending_ui_feedback` (선택) | `Option<UiFeedback>` | `take_ui_feedback()` — 대상② 불일치 비프 |
| `hanja_word_replace_capable` | `bool` | 호스트 능력 플래그, **기본 false**. setter `set_hanja_word_replace_capable(bool)`. true 인 호스트(Linux 데몬·TSF)만 `RecentWord`(버퍼 포함 접미) 채택. IMM32·capi 는 기본값으로 종전 단음절(§1 #28). `reset()`·`rebuild_korean_context` 에서 **보존**(호스트 속성) |
| `surrounding_includes_preedit` | `bool` | 기본 false. TSF 만 true(U12 (a)). `recent_prefix_verified` 의 `prefix+pre` 절 게이트(스펙 §2.2.3). `reset()` 에서 보존 |
| `recent_mark` | `usize` | 래퍼가 키 진입 시 저장한 `commit_buffer.len()`. `recent_absorb_commit_delta()` 가 `commit_buffer[mark..]` 를 push 하고 mark 를 전진 — 한자키 분기의 chord finalize 델타를 `start_hanja_conversion` 전에 흡수(§2.3·§2.5) |

```rust
// src/input_engine/types.rs (PopupAction 옆; InputResult repr(C) 불변)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HanjaReplacement { pub delete_chars: u32, pub preedit_chars: u32, pub text: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HanjaSource { #[default] Syllable, RecentWord, WordBuffer, Selection }
pub const HANJA_MAX_KEY_CHARS: usize = 18;  pub const RECENT_SYLLABLE_CAP: usize = 17;
```

`hanja_target` 은 **사전 키 = 헤더 표시 = 즐겨찾기 키** 로 의미를 좁힌다. `get_hanja_target()`(candidates.rs:127) 불변. 신규 접근자: `hanja_cancel_text() -> String`(= `hanja_recommit.clone()`), `take_hanja_replacement()`, `is_hanja_key(KeyCode) -> bool`, `has_selection() -> bool`(surrounding 비어 있지 않고 cursor≠anchor, 비번 차단 시 false).

### 2.3 버퍼 — press_key 래퍼 (`src/input_engine/press_key.rs`)

```rust
// :58 `pub fn press_key` 본문 → `pub(super) fn press_key_inner`. 새 래퍼:
pub fn press_key(&mut self, keycode: KeyCode, modifier: ModifierState, config: &Config) -> InputResult {
    let before_len = self.commit_buffer.len();
    self.recent_mark = before_len;                       // 한자키 분기의 chord finalize 델타를 중간 흡수하기 위한 마크(§2.5)
    let was_popup  = self.hanja_mode || self.special_char_mode || self.is_emoji_popup_active();
    let cat_before = self.input_category;
    let r = self.press_key_inner(keycode, modifier, config);
    self.recent_track_after_key(keycode, &r, before_len, was_popup, cat_before);
    r
}
// src/input_engine/hanja_word.rs (신규 impl InputEngine)
pub(super) fn recent_track_after_key(&mut self, keycode: KeyCode, r: &InputResult, before_len: usize, was_popup: bool, cat_before: InputCategory) {
    if self.content_purpose.should_block_hangul() { self.recent_clear(); return; }      // (0) fail-closed
    if self.commit_buffer.len() < before_len       { self.recent_clear(); return; }      // (1) 내부 reset 등 → 스냅샷 무효
    if was_popup                                    { self.recent_clear(); return; }      // (2) 팝업 확정/취소/NotHandled 재처리
    if self.input_category != cat_before            { self.recent_clear(); return; }      // (3) 한/영 전환(토글·auto-english·ATF)
    let appended_any = self.commit_buffer.len() > before_len;                             // (4) 델타 분류 — mark 이후만(이중 push 방지)
    self.recent_absorb_commit_delta();                                                    //     commit_buffer[recent_mark..] 를 push, mark 전진
    if !r.consumed && !keycode.is_modifier() {                                            // (5) 통과 키
        if keycode == KeyCode::Backspace && !appended_any && !self.korean_context.is_composing() {
            if self.surrounding_cursor != self.surrounding_anchor { self.recent_clear(); } else { self.recent_pop(); }
        } else { self.recent_clear(); }
    }
}
/// commit_buffer[recent_mark..] 의 문자를 recent_push_char 하고 mark 를 buffer 끝으로 전진.
/// 한자키 분기(§2.5)가 `finalize_chord_buffer()` 뒤·`start_hanja_conversion()` 앞에서 호출해
/// 같은 키 안에서 확정된 chord 음절("민")을 target 풀에 넣는다. 래퍼 (4) 도 같은 헬퍼를 쓴다.
pub(super) fn recent_absorb_commit_delta(&mut self) {
    let mark = self.recent_mark.min(self.commit_buffer.len());
    let appended: Vec<char> = self.commit_buffer[mark..].chars().collect();
    for c in appended { self.recent_push_char(c); }
    self.recent_mark = self.commit_buffer.len();
}
pub fn recent_push_char(&mut self, c: char) {   // pub: unim-dbus 의 ATF 순방향 시드(U11)가 외부 크레이트에서 호출 — pub(super) 로는 불가
    if self.content_purpose.should_block_hangul() { self.recent_clear(); return; }   // chord 타이머 경로는 래퍼 (0) 을 안 거친다 — 훅 자체 fail-closed
    if !c.is_hangul_syllable() { self.recent_clear(); return; }
    if self.recent_syllables.chars().count() >= RECENT_SYLLABLE_CAP { let n = self.recent_syllables.chars().next().map(char::len_utf8).unwrap_or(0); self.recent_syllables.drain(..n); }
    self.recent_syllables.push(c);
}
```
- `popup_dispatch.rs:184` NotHandled 재귀 `self.press_key(...)` → `self.press_key_inner(...)`(외곽 래퍼 1회만, was_popup=true 라 결국 clear).
- 래퍼 밖 훅: `engine.rs:844 chord_idle_flush_pending` — `commit` 을 `mem::take` 하기 직전 `for c in self.commit_buffer.chars() { self.recent_push_char(c) }` (borrow 회피: 먼저 `let taken = mem::take(..)` 후 순회). `engine.rs:671 set_input_category` — 실제로 바뀔 때 `recent_clear()`. `surrounding.rs:38 set_content_purpose` 차단 분기 — **분기 진입 첫 줄**(기존 `flush_preedit()` :45 보다 앞)에 `recent_clear()` + `self.surrounding_text.clear(); self.surrounding_cursor = 0; self.surrounding_anchor = 0;`(진입 시 잔류 제거 — 종전 :71-76 은 새 값만 거부하고 :38-47 은 잔류를 안 지워, 목적 통지보다 먼저 온 선택 스냅샷이 목적 게이트 없는 :183 `typefix_convert` 에 남는다) + 팝업 활성이면 `cancel_hanja()/cancel_special_char()/cancel_emoji_popup()`(korean_context·preedit_cache 를 비움) + `popup_pending_action = Some(HidePopup)`(재커밋 없음). 순서가 뒤집히면 한자 팝업이 유지한 preedit "국" 이 `flush_preedit` 로 commit_buffer 에 실려 다음 ProcessKeyEvent 의 drain 에서 비번 필드로 나간다. 데몬 쪽 HidePopup 발행은 U11 응답 채널. `engine.rs:764 reset()` — `recent_clear()`, `pending_hanja_replacement=None`, 신규 hanja_* 필드 Default. `engine.rs:933 rebuild_korean_context` — `recent_clear()`.
- Enter 는 조합 중이면 flush 로 음절이 (4)에서 push 된 뒤 (5)에서 clear — 순서상 옳다. Space 는 commit " " 또는 통과 → 어느 쪽이든 clear.

### 2.4 target 결정 (`hanja_word.rs`)

```rust
pub(super) enum Resolve { Target(HanjaTargetSpec), NoMatchSelection, None }
pub(super) struct HanjaTargetSpec { key: String, source: HanjaSource, committed: u32, recommit: String, prefix: String, suffix: String }

pub(super) fn resolve_hanja_target(&self) -> Resolve {
    if self.content_purpose.should_block_hangul() { return Resolve::None; }                     // pull 경로 게이트
    let pre: Vec<char> = self.preedit_cache.chars().collect(); let p = pre.len();
    if p == 0 && !self.korean_context.is_composing() { return self.selection_target(); }       // 대상②
    let Some(&last) = pre.last() else { return Resolve::None; };
    let pre_s = self.preedit_cache.clone();
    let spec_last = |src| HanjaTargetSpec { key: last.to_string(), source: src, committed: 0, recommit: pre_s.clone(), prefix: pre[..p-1].iter().collect(), suffix: String::new() };
    if !last.is_hangul_syllable() { return Resolve::Target(spec_last(HanjaSource::Syllable)); }  // 미완성 자모 → 종전(초성 특수문자 폴백은 start_hanja_conversion)
    let word_mode = self.is_word_mode();
    let pool: Vec<char> = if word_mode { pre.clone() } else { self.recent_syllables.chars().chain(pre.iter().copied()).collect() };
    let n = pool.len(); let mut recent_untrusted = false;
    for l in (2..=n.min(HANJA_MAX_KEY_CHARS)).rev() {
        if l > p && (!self.hanja_word_replace_capable || recent_untrusted) { continue; }   // 호스트 플래그 false(IMM32/capi) → 버퍼 포함 접미 불채택(§1 #28)
        let suf = &pool[n-l..];
        if !suf.iter().all(|c| c.is_hangul_syllable()) { continue; }
        let key: String = suf.iter().collect();
        if !self.hanja_dict.contains(&key) { continue; }
        if l > p {                                                                             // 대상① 음절 모드(버퍼 글자 포함)
            let prefix: String = pool[n-l..n-p].iter().collect();
            if !self.recent_prefix_verified(&prefix, &pre_s) { recent_untrusted = true; continue; }
            return Resolve::Target(HanjaTargetSpec { key, source: HanjaSource::RecentWord, committed: (l-p) as u32, recommit: pre_s, prefix: String::new(), suffix: String::new() });
        }
        return Resolve::Target(HanjaTargetSpec { key, source: HanjaSource::WordBuffer, committed: 0, recommit: pre_s, prefix: pre[..p-l].iter().collect(), suffix: String::new() });  // Word 모드 / preedit 내부 일치
    }
    Resolve::Target(spec_last(if word_mode { HanjaSource::WordBuffer } else { HanjaSource::Syllable }))   // 종전(마지막 음절), Word 모드는 접두 보존
}
fn recent_prefix_verified(&self, prefix: &str, pre: &str) -> bool {
    if self.surrounding_text.is_empty() { return true; }                      // XIM·미지원 앱: 검증 생략
    let n = self.surrounding_text.chars().count();
    let cur = self.surrounding_cursor as usize;
    if cur > n { return false; }                                               // 오프셋 초과 = 단위 불일치 → 검증 실패(클램프 금지, 스펙 §2.2.3)
    let before: String = self.surrounding_text.chars().take(cur).collect();
    if before.is_empty() { return false; }                                     // 커서 0/줄 첫머리: "" 는 모든 문자열의 접미 → 잘림 절 공허 통과 금지
    if before.ends_with(prefix) || prefix.ends_with(&before) { return true; }  // GTK/Qt/Wayland/GNOME: surrounding 은 preedit 미포함
    if !self.surrounding_includes_preedit { return false; }                    // Wayland/GNOME 클릭 드리프트("…대한민국" 뒤 클릭 + "국")에서 full 절이 뚫리는 것을 차단
    let full = format!("{prefix}{pre}");                                       // TSF 만: 조합 텍스트가 문서 안에 있다
    before.ends_with(&full) || full.ends_with(&before)
}
/// 선택 구간 (a, b). max(cursor, anchor) 가 문자 길이를 넘으면 단위 불일치로 **거부**(None, unim_log 1회) — 클램프하면
/// GNOME 바이트 오프셋 "대한민국" 의 "대한"(0,6) 이 (0,4) 로 삼켜져 target "대한민국" 팝업 → "大韓民國민국" 이 된다. a >= b 도 None.
/// has_selection() 도 이것으로 판정.
fn selection_span(&self) -> Option<(usize, usize)> {
    let n = self.surrounding_text.chars().count();
    let hi = self.surrounding_cursor.max(self.surrounding_anchor) as usize;
    if hi > n { unim_log!("ENGINE", "surrounding 오프셋 초과({} > {}자) — 선택 무시", hi, n); return None; }
    let a = self.surrounding_cursor.min(self.surrounding_anchor) as usize;
    (a < hi).then_some((a, hi))
}
fn selection_target(&self) -> Resolve {
    if self.content_purpose.should_block_hangul() { return Resolve::None; }
    let Some((a, b)) = self.selection_span() else { return Resolve::None; };  // 범위 검사 없이 &chars[a..b] 하면 a>b 패닉 → engine_worker 단일 스레드 사망
    let chars: Vec<char> = self.surrounding_text.chars().collect();
    let sel = &chars[a..b];
    let lead = sel.iter().take_while(|c| c.is_whitespace()).count(); let trail = sel.iter().rev().take_while(|c| c.is_whitespace()).count();
    if lead + trail >= sel.len() { return Resolve::NoMatchSelection; }
    let core = &sel[lead..sel.len()-trail];
    if core.len() > HANJA_MAX_KEY_CHARS || !core.iter().all(|c| c.is_hangul_syllable()) { return Resolve::NoMatchSelection; }
    let key: String = core.iter().collect();
    if !self.hanja_dict.contains(&key) { return Resolve::NoMatchSelection; }
    Resolve::Target(HanjaTargetSpec { key, source: HanjaSource::Selection, committed: 0, recommit: String::new(), prefix: sel[..lead].iter().collect(), suffix: sel[sel.len()-trail..].iter().collect() })
}
```
`HanjaDictionary::contains(&str) -> bool`(dict.rs:104 `search` 옆, `entries.contains_key`) — Vec clone 회피, 한자키 1회당 ≤17회 조회.

같은 패턴이 `surrounding.rs:197-199 typefix_convert`(`chars[start..end.min(chars.len())]` — 현행은 클램프, 수동 Ctrl+Shift+Space 전용)에 이미 있다 — U8 이 `selection_span()`(범위 초과 거부)으로 함께 고친다(오프셋은 `service.rs:2725 SetSurroundingText` 패스스루라 검증 없는 IPC 입력).

### 2.5 한자키 분기·후보·확정·취소

```rust
// press_key.rs:226-238 (위치 유지)
if self.hanja_keys.contains(&keycode) {
    self.finalize_chord_buffer();            // :1219-1229 → apply_chord_entries(:1238 "commit_buffer 갱신 포함") — 음절 모드 chord 에서 직전 음절("민")이 여기서 커밋된다
    self.recent_absorb_commit_delta();       // 그 델타를 target 풀에 흡수 — 안 하면 풀 "대한"+"국" → "한국"·committed 1 → XIM(검증 생략)에서 '민' 오삭제
    let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
    if idle && !self.has_selection() { self.start_emoji_popup(); return InputResult::consumed(); }   // 종전
    return self.start_hanja_conversion();          // 조합 중: 대상①/종전. idle+선택: 대상②(불일치면 consumed, 팝업 없음)
}
// candidates.rs:16-29 교체
if self.hanja_mode { return InputResult::consumed(); }
let spec = match self.resolve_hanja_target() {
    Resolve::Target(s) => s,
    Resolve::NoMatchSelection => { unim_log!("ENGINE", "선택 단어 한자 불일치"); /* 선택: self.pending_ui_feedback = Some(UiFeedback::HanjaNoMatch); */ return InputResult::consumed(); }
    Resolve::None => { unim_log!("ENGINE", "한자/특수문자 후보 없음"); return InputResult::consumed(); }
};
let mut candidates = self.hanja_dict.search(&spec.key);
if !candidates.is_empty() {
    // :33-71 그대로(즐겨찾기 stable sort, PopupState::new_hanja_with_top_row(&spec.key, ..), ShowHanja) + 신규 필드 세팅.
    // 단 :37-42 unim_log "한자 후보 발견: '{}' -> {} 개" 의 target 평문은 `{}자`(spec.key.chars().count()) 로 — target 이 ≤18자 타이핑 텍스트로
    // 넓어지고 UNIM_DEVELOP=1 이면 ~/.unim-log 파일에 영속(logging.rs:82-88,:115-118); XIM·터미널은 purpose 통지가 없다(스펙 §2.8).
    self.hanja_target = spec.key; self.hanja_source = spec.source; self.hanja_committed_chars = spec.committed;
    self.hanja_recommit = spec.recommit; self.hanja_commit_prefix = spec.prefix; self.hanja_commit_suffix = spec.suffix;
    return InputResult::hanja_candidates();
}
if spec.source == HanjaSource::Selection { return InputResult::consumed(); }   // D6: 특수문자 폴백 없음
// :74-108 초성 특수문자 폴백 그대로(ch = spec.key 첫 글자)

// candidates.rs:140-160 select_hanja
pub fn select_hanja(&mut self, index: usize) -> Option<String> {
    if !self.hanja_mode || index >= self.hanja_candidates.len() { return None; }
    let entry = &self.hanja_candidates[index];
    let body = self.hanja_output_format.render(&self.hanja_target, &entry.hanja);
    let text = format!("{}{}{}", self.hanja_commit_prefix, body, self.hanja_commit_suffix);
    if self.hanja_source == HanjaSource::RecentWord && self.hanja_committed_chars > 0 {
        self.pending_hanja_replacement = Some(HanjaReplacement { delete_chars: self.hanja_committed_chars, preedit_chars: self.hanja_recommit.chars().count() as u32, text: text.clone() });
    }
    self.remove_preedit();          // engine.rs:741 (종전 :150-156 과 동일)
    self.recent_clear();
    self.cancel_hanja();            // 신규 필드 Default 로. pending 은 건드리지 않는다(호스트가 drain)
    Some(text)
}
// popup_dispatch.rs:190-197 popup_select
if let Some(text) = self.select_hanja(abs_index) {
    let replaced = self.pending_hanja_replacement.is_some();
    if !replaced { self.commit_buffer.push_str(&text); }
    self.popup_pending_action = Some(PopupAction::HidePopup);
    return if replaced { InputResult::preedit_updated() } else { InputResult::committed() };   // 둘 다 consumed=true(TSF :575-578)
}
// popup_dispatch.rs:225-231 popup_cancel: push_str(&self.hanja_target) → let t = self.hanja_cancel_text(); if !t.is_empty() { push_str(&t) }
// candidates.rs:266 cancel_hanja: hanja_source/committed/recommit/prefix/suffix 를 Default 로 되돌리는 줄 추가
```

### 2.6 out-of-band 상호배제

| 생산 | 소비(Linux) | 소비(TSF) |
|---|---|---|
| `select_hanja`(RecentWord ∧ committed>0)만 `pending=Some` | engine_worker ProcessKeyEvent :1334 직후 drain / SelectHanja 핸들러 :2013-2028 | key_handler :587 직후 / :1218 직후 |
| ATF `check_*` 는 `popup_action.is_none()`(:1343)·TSF `!popup_active`(:753-754) 게이트로 확정 프레임 스킵 | `:1343` 에 `&& hanja_repl.is_none()` 방어 추가 | 동일 |
| `:1841` `resp.auto_typefix.is_some() && Global` 카테고리 전파("contexts borrow 해제 후" — 엔진 빌림 블록 **밖**) | `&& !hanja_replaced` 게이트. `hanja_replaced` 는 블록 안 :1334 에서 선언하면 스코프가 닿지 않으므로 `let mut hanja_replaced = false;` 를 블록 **앞**(:1156 `global_mode_propagate` 선례)에 두고 블록 안에서 세팅 | — |
| 소비 후 KeystrokeBuffer 폐기 | ATF 블록(:1338-1345)은 `popup_action.is_none()` 게이트로 확정 프레임을 통째로 건너뛰어 `buf.push`/`update_on_commit`/`clear` 도 안 돈다 → 문서 "大韓民國"(4자) vs 버퍼 11키·`committed_chars=3`·`has_preedit=true` stale → 역방향 `delete_chars = committed_chars + has_preedit`(src/auto_typefix/reverse.rs:13) 오계산. `hanja_replaced` 면 `keystroke_buffers.remove(&context_id)`(모드 전환 선례 :1290), `SelectHanja` RPC 경로(:2013-2028)도 동일 | `state.buf.clear()`(auto_typefix.rs:221 선례) 1줄 — 키보드·마우스 두 지점 |

---

## 3. 경로표 최종

| # | 시작 | 사건 | 버퍼 | target 필드 | 출력 | 종료 |
|---|---|---|---|---|---|---|
| 1 | Composing(음절, 버퍼 "대한민", pre "국") | 한자키 push/pull | 불변 | key 대한민국, RecentWord, committed 3, recommit "국" | ShowHanja(preedit "국" 유지) | HanjaPopup |
| 2 | 동일, 검증 실패/다음절 불일치 | 한자키 | 불변 | key "국", Syllable, committed 0 | ShowHanja(종전 바이트 동일) | HanjaPopup |
| 3 | Composing(Word, "오늘대한민국") | 한자키 | 무관(Word 는 버퍼 미사용) | key 대한민국, WordBuffer, prefix "오늘", recommit 전체 | ShowHanja | HanjaPopup |
| 4 | Composing(pre "ㄱ") | 한자키 | 불변 | key "ㄱ", Syllable | 특수문자 폴백(종전) | SpecialPopup |
| 5 | Idle, 선택 "대한민국"(공백 포함 가능) | 한자키 | 불변 | key 대한민국, Selection, prefix/suffix=공백, recommit "" | ShowHanja | HanjaPopup |
| 6 | Idle, 선택 불일치/비한글/>18 | 한자키 | 불변 | — | 없음(consumed, 선택 비프·로그) | Idle |
| 7 | Idle, 선택 없음 | 한자키 | 불변 | — | 이모지(종전) | Emoji |
| 8 | HanjaPopup{RecentWord} | 숫자/Enter | clear | clear | pending Some; commit_buffer 무추가; HidePopup; `preedit_updated()` → 데몬 `auto_typefix=(3,text,"")` → 응답 preedit "" + `AutoTypefixApply` | Idle |
| 9 | HanjaPopup{Syllable/WordBuffer/Selection} | 숫자/Enter | clear | clear | commit_buffer.push(prefix+서식+suffix); `committed()` | Idle |
| 10 | HanjaPopup{*} | SelectHanja RPC | clear | clear | `Replace` → `redirect_replace_and_hide(d,text)` / `Commit` → `redirect_commit_and_hide(text)` | Idle |
| 11 | HanjaPopup{*} | Esc / NotHandled | clear | clear | `hanja_cancel_text()` 커밋(비어 있으면 없음) + HidePopup; NotHandled 는 `press_key_inner` 재처리 | Idle/Composing |
| 12 | HanjaPopup{*} | CancelHanja RPC | clear | clear | engine_worker :2036-2040 → `hanja_cancel_text()` → `redirect_commit_and_hide` | Idle |
| 13 | HanjaPopup{*} | FocusOut/Reset RPC | 재생성 | 재생성 | :737-742 `hanja_cancel_text()` 캡처 커밋 | 새 엔진 |
| 14 | HanjaPopup{*} | `engine.reset()`(ATF 내부·TSF OnSetFocus) | clear | clear + pending None | 없음(TSF 오버레이 앱만 best-effort 삽입) | Idle |
| 15 | HanjaPopup{*} | 즐겨찾기/페이지/expand | **clear**(래퍼 (2) `was_popup` — 팝업 중 키 공통, target·committed 는 진입 시 확정이라 무해) | 불변 | 종전 | 동일 |
| 16 | 임의 | SetContentType(Password) | clear | 팝업 활성이면 cancel(재커밋 없음, 종전 `flush_preedit` 보다 먼저) + HidePopup | 데몬이 응답 bool 로 HidePopup 발행 | Idle |
| 17 | 임의 | 통과 키·비한글 커밋·한/영 전환 | clear | — | 종전 | — |
| 18 | Composing(음절) | Backspace | 조합 중 불변 / 통과 시 pop(선택 있으면 clear) | — | 종전 | — |
| 19 | Idle | chord idle 만료 flush | 음절 push | — | 종전 | — |

---

## 4. 프런트엔드 매트릭스 최종

| 프런트 | 대상① | 대상② | 수정(file:line) | 근거·비고 |
|---|---|---|---|---|
| GTK4 `unim-frontends/gtk4/src/immodule.c` | ✔ `on_auto_typefix`(:465-534): delete_surrounding → XTest BS → `\b` 폴백, commit, preedit "". **:475 `if (!unim->is_focused) return;`** 는 종전 `on_commit_text`(:451-461) 에 없던 드롭 조건 — 마우스 확정을 교체 채널로 옮기면 팝업 클릭이 포커스를 뺏는 환경에서 "국" 유실 가능. **우회는 넣지 않는다(가드 유지)**: `preedit==""&&delete>0` 는 역방향 ATF 프레임(engine_worker.rs:1713-1716)과 같은 시그니처라 우회가 역방향 가드까지 풀고, 포커스 이탈 뒤 `delete_surrounding` 실패(Electron) 시 :488-519 XTest BS 폴백이 **현재 포커스 창**에 실제 키로 들어간다. 실측(스펙 §6.5)에서 포커스 탈취가 확인되면 popup-service 쪽(override-redirect / layer-shell keyboard_interactivity=none)에서 수정; 프런트 우회가 불가피하면 한자 전용 판별자(commit_text 에 ASCII·한글 아닌 문자, §4.2 와 동일) + XTest 폴백 경로 제외 | ✔ 매 키 직전 `retrieve-surrounding`(:865) + `set_surrounding_with_selection`(:104/:381) | **:1061** `if (result.consumed)` → `if (result.consumed && ((result.commit && result.commit[0]) \|\| (result.preedit && result.preedit[0])))` | 마우스 확정 `on_commit_text`(:451-461) → `commit` 시그널 → GtkText 가 자체 치환(`gtk_text_enter_text` 선택 삭제). 키보드 확정은 게이트 통과(commit 있음) → 래퍼 치환 |
| GTK3 `gtk3/src/immodule.c` | ✔ 동형(:396-462) 무수정 | ✘ anchor=cursor(:1220-1222) → idle 이모지 | 코드 없음. `gtk3/SPEC.md` 1줄 | 래퍼는 死코드 — 손대지 않음 |
| Qt5/6 `qt5/src/input_context.cpp`(qt6 `:386/:225/:447/:556` 대응) | ✔ ATF 콜백(:177-222) `setCommitString(text,-N,N)` 무수정 | ✔ 단 surrounding 이 focus-in 1회(:554-564, `:560 if (!surroundingText.isEmpty())` 조건부) → stale | **(a) :290 `update(Qt::InputMethodQueries)`** 에서 갱신: `queries & (ImSurroundingText\|ImCursorPosition\|ImAnchorPosition)` 이면 `QInputMethodQueryEvent` 재질의 → `(text, toChars(cur), toChars(anc))` 를 캐시 삼중과 비교해 **바뀐 경우만** `m_dbus->setSurroundingText(..)`, **빈 텍스트도 전송**(:560 조건 제거); `m_contentPurpose` 가 Password/PIN 이면 `("",0,0)` 만(프런트 이중 게이트, 스펙 §2.8); `toChars(pos) = text.left(pos).toUcs4().size()`. :381 한자키 분기 안 재질의는 두어도 되지만 중복. **(b) :446** 게이트 `result.consumed && (!result.commit.isEmpty() \|\| !result.preedit.isEmpty())` | :381 은 `Key_F9 \|\| Key_Hangul_Hanja` 하드코딩 pull 경로 — 설정된 다른 `hanja_keys` 는 :408 식 `processKey` → 엔진 §2.5 분기(`idle && !has_selection()`)로 가므로 한자키 시점 재질의로는 못 막는다. QLineEdit Tab 포커스 = 전체 선택(anchor 0, cursor len) 스냅샷이 stale 로 남으면 End·타이핑 뒤 커스텀 한자키 idle → `has_selection()` true → stale 텍스트로 팝업 → 확정 CommitText → 실제 선택 없음 → 커서에 "大韓民國" 삽입(사전 미일치면 NoMatchSelection 으로 포커스 내내 이모지·한자 침묵). 불일치 폴백: `getHanjaCandidates` 빈 → 특수문자 없음 → `processKey`(:408, 결과 무시) → 엔진 consumed → 이모지 없음. 마우스 확정 `setCommitTextCallback`(:224-231) → QLineEdit `removeSelectedText` 치환 |
| XIM `xim/src/handler.rs` | ✔ N+1 self-BS(:515-577, :1052-1112) | ✘ | **(a) :515** `PopupEvent::AutoTypeFix` 처리 첫머리 preedit 선클리어 — `handle_popup_event(&mut self, event, server)`(:497) 에는 `user_ic` 가 **없다**(`fn preedit`(:319) 는 `user_ic` 필수). CommitText 분기(:578-612)처럼 `last_focused_ic_info`(:106) 로 `InputContext::new(cw, im_id, ic_id, "")` 를 재구성해 `server.preedit_draw(&mut ic, "")`; NOTHING/POSITION IC(PeWindow 자체 렌더링, :335 주석)는 `input_style` 을 `last_focused_ic_info` 옆에 함께 캐시해 PeWindow 숨김 경로로 분기. 비용이 크면 선클리어 포기 + `xim/SPEC.md` 에 "마우스 확정 시 preedit 잔상 잔여(키보드 확정은 응답 preedit \"\" 로 정상)". **(b) :918** 스팟 보고 지점(`handle_set_ic_values` :907 — 앱이 스팟을 보고할 때마다, xterm 은 커서 이동마다): 판정 기준 = **IM 이 유발하지 않은 스팟 갱신**. 핸들러가 commit/preedit_draw 직후 `expect_spot_update = true` 를 세우고 :918 에서 소비; 기대 없이 도착한 갱신이고 로컬 preedit 없음(idle)이면 `DbusRequest::Reset` 1회. 보조: 직전 커밋 스팟 대비 y 변화·x 감소는 무조건 Reset. **idle 구간당 1회 디바운스**(`reset_sent` 플래그, 다음 commit/preedit_draw 에서 해제). 거리 임계는 쓰지 않는다(앱이 커밋마다 스팟을 전진 보고하고 폰트 폭은 IM 이 모르므로 "같은 줄 1~2자 후퇴 클릭" 을 전진 노이즈와 구분 못 함 — XIM 은 surrounding 이 없어 §2.2.3 검증도 건너뛰므로 미탐 = 오삭제). `xim/SPEC.md` 3줄(대상② 미지원, 확정 후 Reset 무해 근거, 스팟 점프 Reset 기준·비용) | 확정 후 Reset(:1103-1108) 무해 증명 = 스펙 §4.3. **스팟 점프 Reset 은 값싸지 않다**: `reset_engine_and_capture_commit`(engine_worker.rs:1923-1935 → :733 chord 강제 flush·:737-742 hanja 캡처·:757 `InputEngine::new` → 사전 재파싱 ≈6.45MB, :1555-1557 주석) — 오탐(스크롤·expose) 비용 = 재파싱 1회 + 단음절 퇴화(안전 방향), 디바운스가 상한. 실측에서 문제되면 경량 RPC `ClearRecentSyllables`(엔진 `recent_clear` 만) v1.1 |
| Wayland `wayland/src/{state.rs,dbus_client.rs}` | ✔ `apply_auto_typefix`(:249-297) + **바이트 판별자** | ✔ **배선 신설** | **(a) :255** `is_forward` 판정 앞: `let is_hanja = commit_text.chars().any(\|c\| !c.is_ascii() && !c.is_hangul_syllable() && !is_hangul_jamo(c)); let before_bytes = if is_hanja { delete_chars * 3 } else { 기존 }`. **(b) :626-628** `Event::SurroundingText{text,cursor,anchor}` → `state.pending_surrounding = Some(..)`; **:563 Done** 분기(activate/active 양쪽 끝)에서 pending 을 꺼내 `to_chars(b)`: `let mut b = b.min(text.len()); while !text.is_char_boundary(b) { b -= 1; } text[..b].chars().count()` 로 변환(`str::floor_char_boundary` 는 선언 MSRV `rust-version = "1.78"`(Cargo.toml:30) 에서 불안정 — CI 는 `dtolnay/rust-toolchain@stable`(linux-ci.yml:59) 이라 stable 에서만 통과하므로 쓰지 않는다) → `DbusRequest::SetSurroundingText{context_path,text,cursor,anchor}`. **(c)** `dbus_client.rs:17-41` enum variant + `:346-354` `SetContentType` 동형 핸들러(`proxy.set_surrounding_text` 는 `unim-dbus/src/client.rs:173` 에 존재). **(d) :307** `handle_deactivate` 에서 `SetSurroundingText("",0,0)` + pending None. `wayland/SPEC.md` | 앱이 surrounding 미지원이면 이벤트 없음 → cur==anc → 이모지(종전). 마우스 확정 `commit_string` → text-input v3 클라이언트 위젯 치환(실측) |
| GNOME `unim-gnome-extension/{extension.js,popup_view.js,SPEC.md}` | ✔ `onAutoTypeFix`(:283-306) vkbd BS → 50ms → `commitText` | ✔ `vfunc_set_surrounding`(unim_input_method.js:562 — Mutter 오프셋을 단위 변환 없이 전달, 엔진이 클램프) | **extension.js:283** 진입 시 `this._inputMethod._preeditText?.length` 이면 `this._inputMethod.clearPreedit()`(**`unim_input_method.js:728`** 정의, `UnimInputMethod` 메서드 — extension.js:644 는 `_doTypeFix` 라 참조 아님) 을 BS 전에 호출. JS 는 컴파일 검사가 없으니 메서드명을 `rg 'clearPreedit\(' unim-gnome-extension/` 로 재확인. **popup_view.js:98** `this._header.clutter_text.set_ellipsize(Pango.EllipsizeMode.END)`(:373 패턴). **SPEC.md** §2.6(:116-121) 적용 로직에 "preedit 비어 있지 않으면 clearPreedit 선행(한자 단어 교체 잔상 방지)", :247 `AutoTypefixApply` 행 "한자 단어 교체에도 사용(preedit_text=\"\")", :267 `vfunc_set_surrounding` 행 "한자 선택 변환(대상②)·접두 정합성 검증 용도" | Mutter 호출 빈도·오프셋 단위 실측 항목(바이트로 확인되면 확장 쪽에서 `TextEncoder` 등으로 문자 단위 변환 — 엔진은 초과 오프셋을 거부한다, §2.4). **`onAutoTypeFix`(:284) 의 `_hasFocus` 게이트**는 GTK4 `is_focused` 와 같은 마우스 확정 드롭 조건 — St 팝업 클릭 시 `_hasFocus` 유지 여부 실측(스펙 §6.5), 유지 안 되면 렌더러 쪽 수정 우선(GTK4 행과 같은 원칙). `make check-compat` |
| TSF `unim-tsf/src/{key_handler.rs,text_service.rs}` | ✔ §5 U12 | ✔ | §5 U12 | cross-compile 만 |
| popup-service `unim-popup-service/src/popup/hanja.rs` | — | — | **:102-107** `target_label.set_ellipsize(gtk4::pango::EllipsizeMode::End)`(:348 meaning_label 동일) | |
| Windows 렌더러 `unim-popup-win/src/render.rs` | — | — | **:375** `\| DT_END_ELLIPSIS`; **:504-522** compact 한자 열 `hanja_w = max(s(90), text_width(hdc, &format!("{} ★", c.t), font_main) + s(8))` → `hanja_rect.right`, `mean_left = hanja_left + hanja_w + s(6)`; **:197** `CELL_W` **고정 유지**(설계서 :422 "popup 폭 고정 정책" — 페이지별 동적 폭은 페이지 넘길 때 요동) + expanded 셀 텍스트 `DT_END_ELLIPSIS`(전체 한자는 헤더 담당, 설계서 :425). 동결 설계서 `docs/dev/windows/popup-renderer-design.md` :416(compact 한자 열 실측 폭)·:422(격자 셀 ellipsis) 문구 갱신 | wire(`popup_ipc.rs`↔`protocol.rs`, golden :1263) 무변경 |
| IMM32 `unim-imm32` | **대상① 미지원(단음절 유지)** — 코드 무변경 | ✘ | — (코어 플래그 `hanja_word_replace_capable` 기본 false 로 보장, §1 #28) | 후보창 스텁(ui_window.rs:7)이지만 input.rs:110-112 가 Hanja/F9 를 consume → :181 `press_key` → 엔진 blind hanja_mode 진입, lib.rs:224 는 commit/preedit 만 드레인. 플래그 없이는 RecentWord 확정이 "국" 소실. `unim-capi`(examples/capi-c/minimal_session.c) 동일 구조 → setter export 1개만 |

---

## 5. WBS — 의존 순서 스테이지

난이도: 쉬움=sonnet 구현·opus 검증 / 어려움=opus 구현·fable 검증. 파일 소유권은 스테이지 안에서 겹치지 않는다. 각 단위는 **자기 파일만** 만진다.

**검증 게이트 범위**: 공용 작업 트리(D12, worktree 없음)라 같은 Stage 의 다른 단위가 편집 중이면 전체 워크스페이스 빌드는 남의 결함으로 빨개진다. 단위 게이트는 **자기 크레이트/타깃**(`cargo build -p <crate>`, 프런트는 해당 `make` 타깃)으로 한정하고, `cargo build --workspace`·`make build` 는 **각 Stage 종료 시 1회**(그리고 U15) 돌린다.

**Q1 게이트**: `selection_target` 의 idle+선택 분기(§2.5 :188-189)·규칙 10·§9.2 이탈은 스펙 §5 개정안(Q1) 승인 후 착수한다(U8·U11 선행 조건, U14b). 승인 전에는 대상① 만 구현해도 U8 의 나머지는 진행 가능. **문서·UI 문구 중 대상② 서술도 같은 게이트**: U9 의 GTK `row_hanja_keys_subtitle/tooltip` 개정·Slint `settings.slint:810` description 개정, U14 의 CHANGELOG Added 1행 후반부("…앱에서 선택한 한글 단어…")·매뉴얼 §4.2 소절 3(선택 변환)·루트 README 선택 행. 승인 전 착수 시 스펙 §3.3 "Q1 승인 전 문구(대상① 판)" 2벌 중 대상① 판을 쓰고, 승인 후 U14b 가 대상② 판으로 교체한다 — Q1 거절 시 존재하지 않는 기능을 로케일·CHANGELOG·매뉴얼이 안내하는 사고 방지.

### Stage 0 (선행 없음 — 전부 병렬)

**U1 설정 코어** — 쉬움
- 파일: `src/config.rs`
- 지침: §2.1. `CommitUnit`(:57-90) 아래 enum; `KoreanConfig`(:606-) `commit_unit` 옆 필드 `#[serde(default)]`; `Default`(:677-689); `KoreanConfigCompat`(:790-) 필드 `#[serde(default)]` + Compat `Default`(:819-834) + `From`(:836-872) `Self{ .., hanja_output_format: c.hanja_output_format }`. `clamp_ranges` 무관(열거형).
- 검증: `cargo test -p unim config` + 신규 단위 테스트 2개(필드 없는 YAML → `Hanja`; `engine: { korean: { hanja_output_format: HanjaHangul } }` 파싱 → `HanjaHangul`, `Compat` 경유 증명). `cargo build -p unim` 경고 0(워크스페이스는 Stage 종료 시).

**U2 사전** — 쉬움
- 파일: `src/hanja/dict.rs`, `src/hanja/mod.rs`(export 필요 시)
- 지침: `pub fn contains(&self, hangul: &str) -> bool { self.entries.contains_key(hangul) }`(:104 `search` 옆). **U2b(선택, 스펙 Q5)**: 파싱 시 `hangul.chars().count()==1` 줄에서 `char_meaning: HashMap<(char,char),String>`(한자, 음 → 뜻 첫 항(쉼표 앞)) 역색인 + **폴백 `HashMap<char,String>`(한자 단독 → 첫 등장 뜻)** — 두음법칙·다독음(`역사:歷史` 의 歷 은 단음절 표제어 "력" 이라 (歷,'역') 미스)에서 빈칸 방지; `pub fn display_meaning(&self, e: &HanjaEntry) -> String`(원본 뜻이 있고 표제어와 다르면 원본, 아니면 글자별 뜻 `" · "` join, (한자,음) 미스 시 단독 키). 적용 지점은 U8 이 `get_hanja_candidates`/`toggle_hanja_bookmark` 의 `(hanja, meaning)` 조립에서 호출(U8 지침에 조건부 포함).
- 검증: `cargo test -p unim hanja::` + 테스트(`contains("대한민국")`, `!contains("뷁")`; U2b: `display_meaning(국가:國家)=="나라 국 · 집 가"`, `display_meaning(역사:歷史)` 에 빈 항 없음).

**U3 GTK4·Qt 프런트** — 쉬움
- 파일: `unim-frontends/gtk4/src/immodule.c`, `unim-frontends/qt5/src/input_context.cpp`, `unim-frontends/qt6/src/input_context.cpp`, `unim-frontends/{gtk3,gtk4,qt5,qt6}/SPEC.md`
- 지침: §4 표 GTK4 :1061 게이트만(**:475 `is_focused` 우회는 넣지 않는다** — 역방향 ATF 프레임과 시그니처 동일·XTest 폴백이 무관 창에 들어감, §4 GTK4 행) / Qt (a) **:290 `update()`** 에서 surrounding 갱신(변경 시만 전송·빈 텍스트 포함·비번이면 `("",0,0)`) + (b) :446 게이트, qt6 미러(:386/:447/:556 대응). SPEC.md: gtk4·qt 는 "대상② 지원 + 래퍼 게이트 조건", qt 에 "surrounding 은 `update()` 시 갱신(커스텀 hanja_keys 포함)·비번 이중 게이트", gtk3 는 "대상② 미지원(anchor 미전달)".
- 검증: gtk4·qt5·qt6 `make` 타깃(Makefile 확인) 경고 0. 수동: gedit(X11) 선택 → F9 → Esc → 선택 유지 / kate 선택 → F9 → 팝업 / kate: QLineEdit Tab 포커스(전체 선택) → End → 11키 → 커스텀 한자키 → 헤더 "대한민국"(stale 선택 없음). L3 케이스 3(U13), (선택) 케이스 4 커스텀 한자키.

**U4 Wayland** — 어려움
- 파일: `unim-frontends/wayland/src/state.rs`, `unim-frontends/wayland/src/dbus_client.rs`, `unim-frontends/wayland/SPEC.md`
- 지침: §4 표 Wayland (a)~(d). `is_hangul_jamo` 는 U+3131–318E·U+1100–11FF 범위 로컬 헬퍼(코어 `HangulCharExt` 가 wayland 크레이트에서 접근 가능하면 그것). 더블버퍼: `SurroundingText` 는 `done` 전까지 pending. 바이트 오프셋 UTF-8 경계 클램프. `commit_string` 후 preedit 이 비워지는지 확인(ATF 역방향 경로와 동일). (스펙 Q9(b) 채택 시) 컨텍스트별 `saw_surrounding: bool` 을 두고 한 번도 안 왔으면 한자키 시점에 `SetSurroundingText` 대신 "surrounding 미지원" 표식을 보내 엔진이 `committed>0` 대상① 을 끄게 한다 — 표식 전달 방식(빈 텍스트와 구분)은 U8 과 합의, 승인 전 미착수.
- 검증: `cargo build -p <wayland 크레이트>`(이름 `Cargo.toml` 확인) 경고 0 + 기존 단위 테스트. 수동(sway/labwc + GTK4 wl 앱): ATF 순/역방향 회귀, `한자(漢字)` 형식 확정 후 문서 무결, 선택 → F9 팝업.

**U5 XIM** — 쉬움
- 파일: `unim-frontends/xim/src/handler.rs`, `unim-frontends/xim/SPEC.md`
- 지침: §4 표 XIM (a) :515 preedit 선클리어 — `user_ic` 부재이므로 `last_focused_ic_info` 로 IC 재구성 + `input_style` 캐시(비용 크면 포기하고 SPEC 비고 정정); (b) :918 스팟 점프 Reset — 판정 기준은 §4 XIM 행: `expect_spot_update`(commit/preedit_draw 직후 set, :918 에서 소비) 없이 온 갱신 ∧ idle → `DbusRequest::Reset` 1회; 보조로 직전 커밋 스팟 대비 y 변화·x 감소 무조건; `reset_sent` 로 idle 구간당 1회 디바운스. SPEC: 대상② 미지원, 확정 후 Reset(:1103-1108) 무해 근거(스펙 §4.3), 마우스 확정 잔상 방지, 스팟 점프 Reset 기준 + "Reset = 엔진 재생성(사전 재파싱)·chord 강제 flush 동반, 오탐 비용은 재파싱 1회 + 단음절 퇴화".
- 검증: `cargo build -p <xim 크레이트>` + 수동(xterm: 대한민+국 → F9 → 마우스 클릭 확정 → 잔상 없음; 대한민 입력 → 다른 줄 클릭 → 국 → F9 → 헤더 "국"; **같은 줄 1~2자 왼쪽 클릭 → 국 → F9 → 헤더 "국"**(미탐 검증); 연속 타이핑 중 Reset 이 안 나가는지 로그로 확인(오탐 = 사전 재파싱이라 로그 `Reset` 빈도 확인); 스크롤·expose 뒤 Reset 이 나가도 단음절 퇴화만).

**U6 GNOME 확장** — 쉬움
- 파일: `unim-gnome-extension/extension.js`, `unim-gnome-extension/popup_view.js`, `unim-gnome-extension/SPEC.md`
- 지침: §4 표 GNOME 3건(extension.js `clearPreedit()` 선행 — `unim_input_method.js:728` 정의 확인, popup_view ellipsize, SPEC.md §2.6·:247·:267). `POPUP_SPEC.md`(리디렉트 stub)는 무수정.
- 검증: `make check-compat` + 수동(gnome-text-editor: 단어 확정·선택 변환; **St 팝업을 마우스로 클릭해 확정 → `_hasFocus` 유지·교체 적용 여부**(extension.js:284 게이트, 드롭되면 렌더러 쪽 수정 우선 — 스펙 §4.1); `vfunc_set_surrounding` 오프셋 단위 — 바이트면 확장에서 문자 변환 후속).

**U7 렌더러(GTK·Windows)** — 쉬움
- 파일: `unim-popup-service/src/popup/hanja.rs`, `unim-popup-win/src/render.rs`, `docs/dev/windows/popup-renderer-design.md`
- 지침: §4 표 popup-service / Windows 렌더러 3건(헤더 ellipsis, compact 한자 열 실측 폭, expanded 셀 **폭 고정 + `DT_END_ELLIPSIS`** — `CELL_W` 동적화 금지, 설계서 :422 폭 고정 정책). 동결 설계서 :416·:422 문구 갱신(`protocol.rs:1-3` 규칙). wire 무변경(`popup_ipc.rs:1263` 골든 테스트 그대로).
- 검증: `cargo build -p unim-popup-service` 경고 0 + 긴 target 수동 확인; `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu`(실패 시 사유 기록 → VM 검증 항목으로 이관).

### Stage 1 (U1·U2 완료 후)

**U8 코어 엔진 상태기계** — 어려움
- 파일: `src/input_engine/hanja_word.rs`(신규), `src/input_engine/{engine.rs, press_key.rs, popup_dispatch.rs, candidates.rs, surrounding.rs, types.rs, mod.rs}`, `src/input_engine/tests_hanja_word.rs`(**스텁만** — `#[cfg(test)] mod tests_hanja_word;` 등록 + 스모크 1개; 본문은 U10), **`src/SPEC.md`**(U8 단독 소유, U1 과 겹치지 않음), **`unim-capi/src/lib.rs`**(setter 1개 export — `unim_engine_set_hanja_word_replace_capable`, `:277 unim_engine_set_korean_layout` 선례)
- 선행: U1(enum·필드), U2(`contains`). **대상② 분기(`selection_target`·`NoMatchSelection`)는 스펙 Q1 승인 후**(그 전에는 `Resolve::None` 스텁으로 두고 대상①·서식·결함 수정만).
- 지침(순서대로):
  1. `types.rs`: `HanjaReplacement`, `HanjaSource`, 상수 2개, (선택) `UiFeedback`.
  2. `engine.rs`: 필드(:123 옆 — `hanja_word_replace_capable: false`·`surrounding_includes_preedit: false`·`recent_mark: 0` 포함), `new()`(:229 리터럴), `reset()`(:764) clear(두 호스트 플래그는 **보존**), `set_input_category`(:671) 실제 변경 시 `recent_clear`, `rebuild_korean_context`(:933) clear + 서식 캐시, `set_hanja_output_format`(:613 옆), `set_hanja_word_replace_capable(bool)`·`set_surrounding_includes_preedit(bool)`(같은 자리), `take_hanja_replacement`(:604 옆), `hanja_cancel_text`, `is_hanja_key`, `has_selection`, `chord_idle_flush_pending`(:844) push 훅.
  3. `press_key.rs`: :58 `press_key` → `press_key_inner`(pub(super)), 새 래퍼(§2.3, `recent_mark` 저장); :226-238 분기(§2.5 — `finalize_chord_buffer()` 뒤 `recent_absorb_commit_delta()` 후 `start_hanja_conversion()`).
  4. `hanja_word.rs`: `recent_*`(`recent_push_char` 는 **`pub`** — unim-dbus U11 순방향 시드가 외부 크레이트에서 호출; 첫 줄 비번 게이트), `recent_absorb_commit_delta`, `recent_track_after_key`, `resolve_hanja_target`(`l > p` 는 `hanja_word_replace_capable` 일 때만), `recent_prefix_verified`(`before` 빈 문자열 → false, cursor > 길이 → false, `prefix+pre` 절은 `surrounding_includes_preedit` 일 때만), `selection_span`(**범위 초과 → None + 로그**, a≥b → None — 클램프 금지), `selection_target`(§2.3-2.4). `use crate::hangul::char::HangulCharExt`. `surrounding.rs:197-199 typefix_convert` 의 슬라이스도 `selection_span` 으로 교체(현행 `end.min(len)` 클램프 → 거부).
  5. `candidates.rs`: `start_hanja_conversion`(:16-108) 재작성(§2.5; :37-42 로그는 target 평문 대신 `{}자`), `select_hanja`(:140-160), `cancel_hanja`(:266) 필드 리셋. `toggle_hanja_bookmark`(:186-263) 은 `search(&self.hanja_target)` 그대로. (U2b 채택 시) 후보 `(hanja, meaning)` 조립 두 곳에서 `dict.display_meaning(e)`.
  6. `popup_dispatch.rs`: :184 `press_key_inner`; `popup_select`(:190-197); `popup_cancel`(:225-231).
  7. `surrounding.rs:38`: 차단 분기 **첫 줄**(기존 `flush_preedit()` :45 앞)에 `recent_clear()` + **surrounding 3필드 클리어**(`surrounding_text.clear()`, cursor/anchor 0 — 진입 시 잔류 제거, §2.3) + 팝업 활성 시 cancel 3종 + `popup_pending_action = Some(HidePopup)`. 뒤에 두면 유지 중이던 preedit 이 flush 로 commit_buffer 에 실려 비번 필드로 나간다.
  8. `mod.rs`: `mod hanja_word;` + `#[cfg(test)] mod tests_hanja_word;`(:45 옆).
  9. `unim-capi/src/lib.rs`: `unim_engine_set_hanja_word_replace_capable(engine, bool)` export 1개(기본 false 라 기존 소비자 바이트 동일).
  10. `src/SPEC.md`: §2.1 구조체(:111-114)에 `recent_syllables`·`hanja_source`·`hanja_committed_chars`·`hanja_recommit`·`hanja_commit_prefix/suffix`·`pending_hanja_replacement`·`hanja_output_format`·`hanja_word_replace_capable`·`surrounding_includes_preedit` 행, `hanja_target` 주석 "사전 키(어절 가능)"; §2.3 다이어그램(:135-176, :166/:176 Hanja 분기)에 `press_key` 래퍼(`recent_track_after_key`)와 Hanja 3갈래(대상①/②/이모지); §3.2(:305) `HanjaOutputFormat`. CONTRIBUTING.md:66-68 즉시 반영 규칙.
- 불변식: 다음절 일치 없음 ∧ 형식 `Hanja` ∧ **preedit 1자** ⇒ `InputResult`·commit_buffer·`get_hanja_target()` 종전과 바이트 동일. preedit 2자 이상(단어 모드·모아치기 preview)은 스펙 §2.3 결함 수정으로 확정 시 접두+한자, 취소 시 전체 재커밋 — 의도된 변경(종전 `select_hanja` 는 preedit 전체를 비우고 `popup_cancel` 은 target 만 재커밋해 앞 preedit 소실). `tests_popup_change_page.rs:23-33` 헬퍼는 `hanja_target` 만 세팅 — 신규 필드 Default 로 호환.
- 검증: `cargo test -p unim` 전량(기존 + 스모크) 경고 0, `cargo build -p unim` 경고 0(워크스페이스·`make check-windows` 는 Stage 1 종료 시 — `unim-capi` 가 `InputResult` 불변을 컴파일로 강제).

**U9 설정 동기화 8지점** — 쉬움(기계적)
- 파일: `unim-cli/src/main.rs`, `unim-cli/locales/{ko,en}.yml`, **`unim-cli/SPEC.md`**, `unim-settings-gtk/src/settings_dialog.rs`, `unim-settings-gtk/locales/{ko,en}.yml`, `unim-gui-common/src/settings_helpers.rs`, `unim-settings/ui/settings.slint`, `unim-settings/src/main.rs`, `unim-settings/translations/en/LC_MESSAGES/unim-settings.po`, `unim-tsf/src/settings_dialog.rs`
- 선행: U1. (U8 과 같은 Stage 이나 코어 크레이트를 U8 이 편집 중이면 의존 크레이트 빌드가 흔들린다 — 게이트는 자기 크레이트로, 불안정하면 U8 뒤로 미룬다)
- 지침: CLI — `:616-617` 옆 `#[value(name="hanja-output-format", help=h("help_ck_hanja_output_format"))] HanjaOutputFormat`; `:74-80` 옆 `hanja_output_format_display_name_localized`; `config show` `:886-891` 옆 한 줄; `config set` `:1701-1724` 패턴 arm(값 `hanja|한자`, `hangul-hanja|한글한자`, `hanja-hangul|한자한글`, 오류 `error_invalid_hanja_output_format`); 대화형 메뉴(:1806-1816)는 commit_unit 선례대로 생략; **`unim-cli/SPEC.md:76-92` §2.3 `config set` 키 표**에 `hanja-output-format | hanja, hangul-hanja, hanja-hangul` 행(표는 `commit-unit`·`word-mode-apps` 도 빠진 드리프트 — `commit-unit | syllable, word, smart` 백필은 선택). 로케일 — ko.yml `:44-45,:88,:106,:120-122,:378` 대응 위치에 8키(스펙 §3.3), en.yml 같은 줄 수. GTK — `settings_dialog.rs:691-726` `commit_row` 복제 → `hanja_fmt_row`(ComboRow 3항목) 를 한자 키 row(:604-609) 아래, `save_and_notify(.., "hanja_output_format")`, `:16` import; 로케일 `:35-38` 옆 3값+row, `:164` subtitle, `:194` tooltip, **`:25`/`:189` 한자 키 subtitle·tooltip 개정**(스펙 §3.3 — Q1 승인 전이면 "대상① 판" 문구). 병합 — `settings_helpers.rs:208` 뒤 `merge_field(&mut d.korean.hanja_output_format, bk.map(|k| &k.hanja_output_format), &u.korean.hanja_output_format);`. Slint — `settings.slint:330-331` 옆 `hanja-output-format-options/-index`, `:687-693` ComboBox 복제(`title: @tr("한자 출력 형식")`·`description`·`accessible-label`); **en `.po`** 에 msgid 3개(`한자 출력 형식`/`Hanja Output Format`, description, `:175` `한글 확정 단위` 옆 — `build.rs:13` 번들이라 없으면 영어 로케일에 한국어 노출); **한자 변환 키 row description 개정** — `settings.slint:810` `@tr("한글을 한자로 변환할 때 사용할 키입니다.")` 를 스펙 §3.3 Slint 행 문구로(Q1 승인 전이면 대상① 판) + `.po:244` msgid/msgstr 갱신(GTK 만 개정하면 GNOME 사용자가 prefs.js 리다이렉트로 보는 Slint 앱과 안내가 갈린다); `main.rs:842-854` 옵션/인덱스(`display_name()` — 한국어 리터럴 노출은 commit_unit 선례와 같은 알려진 갭, 스펙 §3.3), `:935-938` 역변환, `:578` 옆 merge_field. Windows 모달 — `ID_CMB_COMMIT_UNIT` 선례 **3지점**(const :68 / create :693 / 읽기 :1248) + 저장 역변환으로 `ID_CMB_HANJA_OUTPUT_FORMAT=4006` 콤보, 라벨 하드코딩 한국어(파일 관례).
- 검증: `cargo build -p unim-cli -p unim-settings-gtk -p unim-gui-common -p unim-settings` 경고 0; `unim-cli config set hanja-output-format hangul-hanja` → `config show` 왕복 → `~/.config/unim/config.yaml` 에 `hanja_output_format: HangulHanja`; GTK/Slint 저장 후 YAML 확인, Slint `LANG=en` 에서 제목 영어; `make check-windows`(Stage 종료 시).

### Stage 2 (U8 완료 후)

**U10 L1 테스트** — 쉬움
- 파일: `src/input_engine/tests_hanja_word.rs`(본문)
- 선행: U8
- 지침: §6.1 케이스 전량(추가된 `cursor=0` 퇴화·오프셋 초과 `("가나",5,3)` 무패닉·팝업 중 키 버퍼 clear·비번 진입 시 `commit_buffer` 빈 채 `!is_hanja_mode()`·chord 비번 게이트 포함). 헬퍼 `type_keys(&mut e, &[KeyCode])`(`create_test_engine` test_helpers.rs:9 + `tests_scenarios.rs:283-288` 패턴). 키열 11키. Selection 은 `set_surrounding_text(..)`.
- 검증: `cargo test -p unim tests_hanja_word` 전량 + `cargo test --workspace` 경고 0.

**U11 Linux 데몬 배선** — 어려움
- 파일: `unim-dbus/src/service.rs`, `unim-dbus/src/engine_worker.rs`, (선택) `unim-dbus/src/beep.rs`, `unim-dbus/SPEC.md`
- 선행: U8
- 지침: `service.rs:67-71` `SelectHanja.response: oneshot::Sender<SelectHanjaOutcome>` + `pub enum SelectHanjaOutcome { None, Commit(String), Replace { delete_chars: u32, text: String } }`; `:2886-2920` `select_hanja` — `match outcome { Commit(t) => redirect_commit_and_hide(&t), Replace{d,t} => redirect_replace_and_hide(d,&t), None => "" }`, `Ok(text)`(시그니처 `(u)->s` 불변); `:1807` 옆 `redirect_replace_and_hide(&self, delete_chars, text)` = `redirect_commit_and_hide` 복제, `emit_signal(None::<&str>, &path, "org.atit.unim.InputContext", "AutoTypefixApply", &(delete_chars, text.to_string(), String::new()))` 후 HidePopup. `engine_worker.rs` — **엔진 생성 헬퍼** `fn new_daemon_engine(config: &Config) -> InputEngine { let mut e = InputEngine::new(config); e.set_hanja_word_replace_capable(true); e }` 로 `InputEngine::new` 호출 전부를 대체(FocusIn `:1059`, `reset_engine_and_capture_commit` `:757`, 리로드 루프 `:988` 부근 — 구현자는 `rg 'InputEngine::new' unim-dbus/src/engine_worker.rs` 로 테스트 외 잔존 0 확인; 한 곳이라도 빠지면 그 경로 뒤 단어 변환이 조용히 꺼진다); `let mut hanja_replaced = false;` 를 엔진 빌림 블록 **앞**(`:1156 global_mode_propagate` 옆 — `:1841` 은 "contexts borrow 해제 후" 라 블록 안 선언은 스코프 밖)에; `:1334` 직후 `let hanja_repl = engine.take_hanja_replacement(); hanja_replaced = hanja_repl.is_some(); if hanja_replaced { keystroke_buffers.remove(&context_id); }`(§2.6 소비 후 버퍼 폐기); `:1343` `&& hanja_repl.is_none()`; ATF 블록 뒤(`:1709` 전) `if let Some(r) = hanja_repl { auto_typefix_result = Some((r.delete_chars, r.text, String::new())); fix_has_replay = false; }`(:1709-1725 는 `(Some(""), None)` 을 산출 — 수정 없음); `:1841` `&& !hanja_replaced`; `:2013-2028` `select_hanja` + `take_hanja_replacement` → outcome(Replace 면 `keystroke_buffers.remove(&context_id)`); `:2036-2040`·`:737-742` `get_hanja_target()` → `hanja_cancel_text()`; `:988` 뒤 `engine.set_hanja_output_format(&config);`; **SetContentType 응답 채널** — `service.rs:117` `SetContentType { context_id, purpose, response: oneshot::Sender<bool> }`(워커에는 `emit_signal`/`SignalContext` 가 전무하고 :2073 부근 ToggleHanjaBookmark 패턴도 RPC response 로 되돌려 service.rs 가 발행하는 구조라 그대로 적용 불가), `engine_worker.rs:2252` `set_content_purpose` 뒤 `engine.take_popup_action().is_some()` 을 response 로, `service.rs:2707-2721 set_content_type` 이 await 후 true 면 `self.redirect_commit_and_hide("").await`(:1826 빈 텍스트면 CommitText 생략·HidePopup 만). 프런트 `SetContentType(u)` 호출 시그니처 불변. **ATF 순방향 시드** — 순방향 적용 블록의 replay **뒤** `engine.clear_commit()`(`:1600`, 비-word 분기; word 분기 `:1588` 은 commit_text 가 "" 라 시드 없음) 직후 `engine.recent_clear(); for c in fix.commit_text.chars() { engine.recent_push_char(c) }`. reset 직후·replay 전에 두면 안 된다 — 데몬 스스로 "replay 에서 발생한 commit 은 무시" 라며 `clear_commit` 을 부르므로(:1592-1600) replay 중 한글 commit 델타가 래퍼 (4) 로 시드 위에 추가 push 돼 surrounding 없는 XIM/Wayland 에서 검증 없이 과다 삭제. replay 뒤 시드는 부작용과 무관하게 결정적. (`recent_push_char` 의 `pub` 가시성은 U8 소유 — U11 은 바꾸지 않는다.) 레거시 `get_config/set_config` 암은 추가하지 않는다(commit_unit 선례, U13 은 YAML 헬퍼). (선택) `beep.rs:181` 옆 `announce_hanja_nomatch()` + ProcessKeyEvent 에서 `take_ui_feedback()` drain, `config.engine.toggle_announce_beep` 게이트. `unim-dbus/SPEC.md:254-256,:264(SetSurroundingText 용도),:285` 보강(스펙 §5.8).
- 선행 추가: 대상② 관련 배선(`NoMatchSelection` 로그·비프)은 스펙 Q1 승인 후.
- 검증: `cargo build -p unim-dbus` 경고 0, `cargo test -p unim-dbus`, L2(U13 — 팝업 중 `SetContentType(Password)` → HidePopup 수신, ATF 순방향 직후 target=="대한민국").

**U12 Windows TSF** — 어려움
- 파일: `unim-tsf/src/key_handler.rs`, `unim-tsf/src/text_service.rs`
- 선행: U8
- 지침: (0) 엔진 생성 직후(`text_service.rs:187` 초기, `:463` 리로드 교체) `engine.set_hanja_word_replace_capable(true)` — 두 곳 모두, 빠지면 그 경로 뒤 단어 변환이 조용히 꺼진다(§1 #28). (a) `key_handler.rs:557` 직전 — `if engine.is_hanja_key(keycode) && atf_active(:417) { engine.set_surrounding_includes_preedit(true); if !engine.is_composing() { match composition::read_selection_text(context, tid) { Some(sel) if sel.cursor != sel.anchor => engine.set_surrounding_text(sel.surrounding_text, sel.cursor, sel.anchor), _ => engine.set_surrounding_text(String::new(),0,0) } } else { engine.set_surrounding_text(String::new(),0,0) } }`(플래그는 TSF 의 surrounding 이 조합 텍스트를 포함하므로 §2.4 `prefix+pre` 절을 여는 스위치 — 다른 프런트는 기본 false). (b) 공용 `fn apply_hanja_replacement(engine, comp_mgr, context, tid, comp_sink, composition_unsupported: bool, preedit_win: &mut Option<PreeditWindow>, rep) -> bool /*schedule_flush*/`: `span = rep.delete_chars; if comp_mgr.is_active() { comp_mgr.end_composition_keep_text(context, tid)(composition.rs:570); span += rep.preedit_chars } else if composition_unsupported { 오버레이 preedit_win 클리어(폴백 경로 :603- 의 빈 preedit 호출 재사용) }; outcome = comp_mgr.replace_surrounding(context, tid, span, &rep.text, "", comp_sink)(composition.rs:663); match outcome { Normal => {}, PhaseSplit => flush=true, SynthBatch => engine.remove_preedit(), SynthHeadTail => { discard_pending_tail(); engine.remove_preedit() } }`(key_handler.rs:445-454 4갈래 바이트 동일); 끝에 ATF `state.buf.clear()`(auto_typefix.rs:221 선례, §2.6 소비 후 버퍼 폐기 — `atf_state` 접근은 호출부 관례대로). (c) `:587` 직후 `if let Some(rep) = engine.take_hanja_replacement() { let schedule_flush = apply_hanja_replacement(..); return KeyDownOutcome { eaten: true, schedule_flush, ..Default::default() }; }`. (d) `:1218` 직후 동일 블록, `context` `Some(ctx)` 일 때만, `None` 이면 `dbg_log("popup_rev: no context — hanja replacement dropped")`(:1242-1247 동형). **`apply_reverse_event`(:1130-1140) 시그니처에 `composition_unsupported: bool, preedit_win: &mut Option<PreeditWindow>` 추가** — 현행 매개변수에는 오버레이 상태가 없어 (b) 를 부를 수 없다(서비스 필드 `text_service.rs:80 composition_unsupported: AtomicBool`·`:64 preedit_window: Mutex<Option<PreeditWindow>>`); 호출부 `text_service.rs:2436` 에서 `ctx.composition_unsupported.load(SeqCst)`·`&mut *ctx.preedit_window.lock().unwrap()` 을 기존 락 순서(engine → config → composition_mgr → popup_ipc → last_context) **뒤**에 취득해 전달(구현자: `rg 'preedit_window.lock' unim-tsf/src` 로 역순 락 경로 없음 확인). 반환형 `()` 유지(schedule_flush 는 로그만). 이어지는 commit_str 블록(:1221-1249)은 비어 있어 no-op. (e) `text_service.rs:1855-1869` — hanja 정리 블록은 `if preserve_live_compose { 스킵 } else { engine.reset() }` **앞(양 분기 공통)** 에 둔다: `if engine.is_hanja_mode() { let t = engine.hanja_cancel_text(); engine.cancel_hanja(); if !preserve_live_compose && composition_unsupported && !t.is_empty() { last_context 로 insert_text 시도, 실패 시 dbg_log } else { dbg_log("hanja popup dropped on focus change") } }`. 스킵 분기(:1855-1866, 전이 포커스 조합 보존)는 `cancel_hanja` 만 하고 재커밋·삽입 시도는 하지 않는다 — 그 분기도 `popup_ipc.hide()`(:1878) 는 실행하므로 엔진만 hanja_mode 로 남으면 다음 키가 `press_key.rs:88` 팝업 dispatch 로 샌다. hide 호출 추가 없음. `composition_unsupported` 접근 경로는 구현자가 `rg composition_unsupported unim-tsf/src/text_service.rs` 로 확인. 대상② 확정은 현행 `insert_text`(:734/:1236) 경로 — `replace_surrounding` 은 `:1319 Collapse(TF_ANCHOR_START)` 때문에 대상② 에 금지. 구현자 확인 항목: `acquire_insert_range`(composition.rs:224) 가 선택 range 를 유지하는지; Collapse 한다면 `InsertTextAtSelection(TF_IAS_NOQUERY)` 분기 추가.
- 검증: `make check-windows` 경고 0 (`cargo test -p unim-tsf` 골든 라인 무변경 통과). VM 항목은 §8 로.

### Stage 3 (U11 완료 후)

**U13 L2·L3** — 쉬움
- 파일: `tests/unim-test-dbus/src/main.rs`, `tests/harness/scenarios/hanja_word.json`(신규), (선택) `tests/harness/harness.py`
- 선행: U11, U3(L3 케이스 3)
- 지침: §6.2 L2 4케이스(`test_hanja_popup:169` 옆, evdev `18 24 34 37 31 30 38 31 19 49 19`, `Layout::Dubeolsik` 분기·세벌식은 스킵 표기, `auto_typefix_apply` 스트림 `unim-dbus/src/client.rs:259`, `set_surrounding_text` `:173`). §6.3 L3 3시나리오(`harness.py:498-514` `key/keys` 어휘, `expect` 3키 `:353-356`). GTK4 앱 필드는 `harness.py:35-86` 앱 표에서 지정, 없으면 케이스 2 생략. `commit_unit`/서식 자동 적용은 **선택** — 채택 시 `harness.py` 에 `set_config_field(path, value)` 헬퍼(`GetConfigYaml` → 키 패치 → `SetConfigYaml`, 시나리오 종료 시 원본 YAML 복원)를 **필수**로 추가한다. `:398-409` layout 패턴(`get_config`/`set_config` = 레거시 `GetConfig`/`SetConfig` 키 API)은 복제 금지 — `service.rs:719-733/812-891` 레거시 match 에 `commit_unit`·`hanja_output_format` 암이 없어 unknown key 로 조용히 실패한다. L2 에 (5) 팝업 중 `SetContentType(Password)` → HidePopup, (6) ATF 순방향 시드 추가(스펙 §6.2).
- 검증: `make test-dbus`(타깃명 Makefile 확인) 그린; L3 러너로 3케이스.

**U14 문서** — 쉬움
- 파일: `CHANGELOG-ko.md`, `CHANGELOG.md`, `docs/user/user-guide/README-ko.md`(§4.2 :375-383, §5.1 :664 GUI 투어, :690 이모지 문장, **조합 확정 단위 절 :590-593 CLI 블록 + :605-609 YAML 샘플**, §7.2 :916-920 CLI 예시), `docs/user/user-guide/README.md`(§4.2 :375, **같은 절 :588-591 CLI 블록 + :603-606 YAML 샘플** + 대응 절 — ko/en **1:1**, help HTML 4종이 여기서 생성되므로 한쪽만 고치면 `unim-help-ko.html`/`-en.html` 이 갈린다), `docs/user/UNIM-Windows-사용안내.md`(:44 idle 표), `docs/user/faq/README-ko.md`·`README.md`(Q28 :651 확인), `ROADMAP.md`(:118-127 ①), **루트 `README.md`**(:207-213 한자키 3분기 표 — "한글 단어를 선택한 채(GTK4·Qt·GNOME·Wayland·Windows) → 그 단어 한자 팝업" 행 추가·조합 중 행 예를 `대한민국 → 大韓民國` 로·idle 행에 "(선택 없음)" 단서; :217-219 "글자를 친 뒤로는 한자키 한 번 + 숫자 한 번" 은 단어에도 성립함을 한 구; :33 영문 bullet 에 word 한 구. `make help-html` 입력이 아니라 자동 반영 안 됨, tools/gen-help/src/main.rs:4-8. 선택 행은 Q1 게이트), `help/unim-help-{ko,en}.html`·`help/windows/unim-help-{ko,en}.html`(**`make help-html` 재생성 산출물, 저장소 추적**). `docs/dev/specs/HANJA_WORD_SPEC.md` 는 **U14b 단독 소유**(같은 Stage 안 소유권 불겹침 — 공용 트리에서 동시 편집 시 상태 줄·§8 이력 행이 서로 덮인다).
- 선행: 내용 확정(초안은 Stage 0 부터 가능). POPUP_SPEC.md 는 **승인 전 무수정**(U14b). 조합 확정 단위 절의 YAML 샘플에 `hanja_output_format: Hanja` 1줄 + CLI 블록에 `unim-cli config set hanja-output-format …` 1줄(ko/en 동일).
- 지침: §7 문구. 매뉴얼 §4.2 에 소절 5개: 단어 변환 절차와 예 / "조사를 붙이기 전에 한자키" / 선택 변환 절차 + 지원 환경 표(GTK4·Qt·GNOME·Wayland·Windows, GTK3·XIM 제외 — Linux 프런트 표는 `<!-- @platform:linux -->` 마커로 감싸 Windows 판 HTML 에 새지 않게, README-ko.md:11-28 관례) + "선택이 사전에 없으면 팝업이 뜨지 않음" / 출력 형식 설정 / 단어 즐겨찾기·마우스 클릭 후 이어 치면 엉뚱한 단어가 뜰 수 있음 → Esc. §5.1 GUI 투어에 「한자 출력 형식」 row 1문단; :690 이모지 문장에 "한글 단어를 선택한 상태에서는 선택 단어 한자 변환" 예외 1구; §7.2 블록에 `# 한자 출력 형식 — 4.2 참고` + `unim-cli config set hanja-output-format hangul-hanja` 1줄(en 대응); Windows 사용안내 :44 행을 "(idle, 선택 없음)" 으로 정정 + 선택 변환 행 추가. ROADMAP ①: "v1 완료(최장 접미·선택 변환·서식) / 남은 것: 조사 분리, 축소·확장 키, 후보 랭킹". 마지막에 `make help-html` 실행(생성기 `tools/gen-help` 는 빌드 의존성이 **아니다** — Makefile:227; `make help` 는 사용법 출력 타깃).
- 검증: CHANGELOG 규칙(`docs/dev/architecture/AGENTS.md:170-197` 한 줄·명사형·사용자 관점·굵게 금지·ko/en 항목 수 동일) 리뷰, `make check-help-html` 그린(CI `linux-ci.yml:74` 와 동일 게이트).

**U14b POPUP_SPEC v3.4 반영** — 쉬움
- 파일: `docs/dev/specs/POPUP_SPEC.md`, `docs/dev/specs/HANJA_WORD_SPEC.md`(**단독 소유** — 상태 "승인·반영" + §8 변경 이력 행을 한 번에)
- 선행: **스펙 Q1 승인**(기현님) **+ U14 종료**(순차: U14 → U14b — 같은 Stage 이지만 HANJA_WORD_SPEC.md 소유권 충돌을 피하고, 승인 후 대상② 판 문구로 U9/U14 산출물을 교체하는 것도 U14b 가 맡는다). CONTRIBUTING.md:87-97 팝업 변경 6지점 체크리스트에 POPUP_SPEC.md 가 포함된다.
- 지침: 스펙 §5.1~5.7 문구 그대로(규칙 2·4·5·10, §9.2 인용문, §2.4 행 주석, §11 v3.4 행). HANJA_WORD_SPEC 상태를 "승인·반영" 으로 + §8 이력 행. Q1 승인 전 "대상① 판" 문구로 나간 GTK/Slint 로케일·CHANGELOG·매뉴얼·README 를 대상② 판으로 교체(스펙 §3.3).
- 검증: 문서 diff 리뷰(POPUP_SPEC 다른 조항 무변경).

### Stage 4

**U15 통합 검증** — 쉬움 검증(opus)
- 선행: 전부
- `cargo build --workspace` 경고 0 → `cargo test --workspace` → `make build` → `make check-windows` + `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu` → `make check-compat` → L2 → L3. 불변식 확인: 형식 `Hanja` + 다음절 불일치 시 기존 `test_scenario_hanja_conversion`·`tests_popup_change_page` 바이트 동일.

병렬 요약: `{U1,U2,U3,U4,U5,U6,U7}` → `{U8,U9}` → `{U10,U11,U12}` → `{U13,U14}` → `U14b(Q1 승인 후, U14 뒤 순차)` → `U15`. 임계 경로 U1→U8→U11→U13. 각 Stage 종료 시 `cargo build --workspace` + `make build` 1회(단위 게이트는 자기 크레이트).

---

## 6. 테스트 계획

| 층 | 파일 | 케이스 | 명령 |
|---|---|---|---|
| L1 | `src/input_engine/tests_hanja_word.rs` | 스펙 §6.1 표(≈31): 대상① 7(호스트 플래그 false → "국"·commit "國" 바이트 동일, chord 한자키 확정 음절 흡수 → "대한민국"·committed 3 포함), 검증 6(`surrounding_includes_preedit` false/true 대비, cursor 초과 실패 포함), 버퍼 4, 오프셋 거부 2(`("가나",5,3)` 무패닉·`("대한민국",0,12)` 선택 없음), 리셋 조건 파라미터화 1(≈15 지점), 단어 모드 3, 대상② 7, 서식 4, 즐겨찾기 1, 비밀번호 4(진입 시 surrounding 잔류 제거 포함) + 회귀 전량 | `cargo test -p unim`, `cargo test --workspace` |
| L1 config | `src/config.rs` 테스트 | 필드 없는 YAML → `Hanja`; `HanjaHangul` 파싱(Compat 경유) | 동상 |
| L2 | `tests/unim-test-dbus/src/main.rs` | 단어 교체 시그널 `(3,"大韓民國","")`; 선택 → `CommitText`; 불일치 → 이모지 미발행; (선택) 서식 `SetConfigYaml` | `make test-dbus` |
| L3 | `tests/harness/scenarios/hanja_word.json` | `hanja-word-syllable`(2bul) / `hanja-word-selection`(GTK4 필드) / `selection-kept-on-consumed-key`(래퍼 게이트 회귀) | L3 러너(Xvfb). Wayland 네이티브 앱은 스킵(harness.py:388-390) |
| Windows | — | `make check-windows`, popup-win gnu check. VM: 확정 2지점·CUAS synth·오버레이·`insert_text` 치환·OnSetFocus·렌더러 폭 | cross-compile 만 |
| 수동 | — | GTK4 gedit(X11) 선택→F9→Esc 유지; kate; xterm 마우스 확정 잔상; sway/labwc `한자(漢字)` 형식 문서 무결; GNOME gnome-text-editor 선택 변환 | — |

전제: 테스트 환경 즐겨찾기 파일 부재(`대한민국:大韓民國` 단일 항목이라 순서 무관), 기본 형식 `Hanja`, Linux 기본 `commit_unit=Smart`(= 음절, `word_mode_apps` 빈 목록).

---

## 7. 문서·CHANGELOG 문구 초안

### 7.1 CHANGELOG (`## [Unreleased]`, ko 정본, 항목 수 동일)

두 파일 모두 첫 버전 절이 `## [0.4.3] 2026-09-10`(ko :9, en :7)이고 `[Unreleased]` 절이 없다 → 최상단에 `## [Unreleased]`(날짜 없음) 절을 신설한다. UI 이름은 대괄호 + 그 언어의 실제 UI 문자열(AGENTS.md:170-197): ko `[설정] › [한자 출력 형식]`, en `[Settings] › [Hanja Output Format]`(GTK en 로케일 `row_hanja_output_format`). U2b·Windows 항목은 게재 전 PM 결정으로 확정 문구로 치환 — 플레이스홀더 채로 게재 금지.

**CHANGELOG-ko.md**
```
### 추가됨
- 한자 변환에서 방금 입력한 어절(예: 대한민국)과 앱에서 선택한 한글 단어를 한 번에 한자 단어로 바꾸는 기능 지원 (단어 선택 변환은 GTK4·Qt·GNOME·Wayland·Windows)
- 한자 확정 문자열 형식 설정 추가 — 漢字, 한자(漢字), 漢字(한자) 중 선택 ([설정] › [한자 출력 형식], `unim-cli config set hanja-output-format`)
- 한자 팝업의 두 글자 이상 후보에 글자별 뜻 표시   ← U2b 채택 시에만, 미채택이면 ko/en 함께 삭제

### 수정됨
- 단어 확정 모드에서 한자 팝업을 취소하거나 마지막 음절만 변환하면 앞에 조합해 둔 글자가 사라지던 문제 수정
- GTK4·Qt 앱에서 텍스트를 선택한 채 한자 키나 한/영 전환키를 누르면 선택한 텍스트가 지워지던 문제 수정
- Windows: 한자 팝업이 열린 채 다른 창으로 갔다 돌아오면 다음 키 입력이 팝업 조작으로 처리되던 문제 수정
```
**CHANGELOG.md**
```
### Added
- Hanja conversion of the word just typed (e.g. 대한민국) and of a Hangul word selected in the app, in one step (selection conversion on GTK4, Qt, GNOME, Wayland and Windows)
- Hanja output format setting — choose 漢字, 한자(漢字) or 漢字(한자) ([Settings] › [Hanja Output Format], `unim-cli config set hanja-output-format`)
- Per-character meanings shown for multi-syllable hanja candidates   ← only if U2b is adopted; otherwise delete together with the ko line

### Fixed
- In word commit mode, cancelling the hanja popup or converting only the last syllable dropped the syllables composed before it
- On GTK4 and Qt apps, pressing the hanja key or the Korean/English toggle with text selected erased the selection
- Windows: after switching windows while the hanja popup was open, the next keystroke was handled as popup navigation
```
Windows 항목은 VM 검증 전 게재 여부를 PM 이 판단(cross-compile 만 검증됨).

### 7.2 사용자 매뉴얼 §4.2 (요지)
1. 단어 변환: "대한민국" 을 치고 마지막 글자가 조합 중일 때 한자키 → 「대한민국」 후보 → 숫자. 조사("은/는")를 붙이기 전에 누른다.
2. 선택 변환: 앱에서 한글 단어를 마우스로 선택 → 한자키 → 숫자. 지원 환경 표. 사전에 없는 단어·한글이 아닌 선택은 아무 팝업도 뜨지 않는다(선택이 지워지지 않도록).
3. 출력 형식: [설정] › 한자 출력 형식(漢字 / 한자(漢字) / 漢字(한자)). 한 글자 변환에도 적용.
4. 즐겨찾기: 단어도 Space/우클릭으로 동일. 같은 단어 팝업에서만 재사용.
5. 마우스로 커서를 옮긴 뒤 이어 쳤을 때 엉뚱한 단어가 헤더에 뜨면 Esc — 이미 입력된 글자는 건드리지 않는다.

### 7.3 라이브 도움말·툴팁
스펙 §3.3 표 그대로(설정 row 3값 + subtitle/tooltip, 한자 키 row subtitle/tooltip 개정, CLI 8키).

---

## 8. 위험·롤백

### 8.1 위험표

| 위험 | 영향 | 완화 |
|---|---|---|
| 버퍼 드리프트(마우스 편집·메뉴 Undo·앱 자동교정, Wayland/XIM 은 클릭 시 Reset 없음) → 잘못된 글자 삭제(XIM N+1 BS·Wayland delete_surrounding 은 비가역) | 문서 손상 | 정합성 검증(GTK3/4·Qt 재질의·GNOME·Wayland 배선·TSF idle, `before` 빈 문자열은 실패) 실패 시 단음절 퇴화. XIM 은 통과 키 전량 리셋 + **스팟 점프 Reset(U5)** + 헤더 확인 후 Esc(접두 무손상). surrounding 미지원 Wayland 앱은 스펙 Q9(b) PM 판단. 잔여 수용, 매뉴얼 명시 |
| 선택 오프셋이 텍스트 길이를 넘음(GNOME 단위 미변환·Qt UTF-16) | 슬라이스 패닉 → 워커 스레드 사망(전 컨텍스트 정지); **클램프하면** 바이트 (0,6)→(0,4) 로 "대한" 선택이 "대한민국" 팝업이 돼 확정 시 "大韓民國민국"(그럴듯한 오답) | `selection_span()` **범위 초과 거부**(None + 로그) + a≥b 조기 반환, `recent_prefix_verified` cursor 초과 실패, `typefix_convert` 동일. L1 케이스 2. GNOME 바이트 확인 시 확장에서 문자 변환 |
| 정합성 검증 `prefix+pre` 절이 비-TSF 에서 클릭 드리프트를 통과시킴(버퍼 "대한민" 잔류 → 기존 "…대한민국" 뒤 클릭 → "국") | Wayland/GNOME `delete_surrounding` 으로 '한민국' 비가역 삭제 | `surrounding_includes_preedit` 플래그(TSF 만 true) 로 절 게이트. L1 대비 케이스 |
| GTK4 `is_focused` 우회가 역방향 ATF 가드까지 풀어 포커스 이탈 뒤 XTest BS 폴백이 무관 창에 입력 | 다른 창 텍스트 삭제 | 우회 없음(현행 가드 유지). 포커스 탈취는 popup-service 쪽 수정, 실측 §6.5 |
| IMM32·capi 호스트가 교체 페이로드를 드레인하지 않음 | 확정 시 조합 음절 소실·pending 영구 잔류 | 호스트 플래그 기본 false(§1 #28). L1 바이트 동일 케이스 |
| chord 모드에서 한자키 자체가 직전 음절을 커밋해 target 풀에서 빠짐 | 접두 길이 오산 → XIM(검증 생략) 비가역 오삭제 | 한자키 분기 `recent_absorb_commit_delta()`(§2.5). L1 chord 케이스 |
| XIM 스팟 점프 Reset 오탐(스크롤·expose) / 미탐(같은 줄 짧은 후퇴) | 오탐 = 엔진 재생성(사전 재파싱) + 단음절 퇴화 / 미탐 = 오삭제 | 기준 "IM 이 유발하지 않은 갱신"(미탐 최소화) + 디바운스(오탐 상한). 실측 비용 문제 시 경량 RPC |
| 비번 필드 전환 시 팝업 잔존·preedit 유출 | 비번 노출 | 차단 분기 첫 줄 정리 + SetContentType 응답 채널로 HidePopup. L1/L2 케이스 |
| Wayland `delete_surrounding_text` 미지원 앱 | 삭제 실패 | 기존 ATF 와 동일 한계 |
| GTK4 `delete_surrounding` 미지원 앱(Electron)의 XTest BS 폴백 중 키 입력 | 기존 ATF 동일 | 신규 아님 |
| 래퍼 게이트(U3)가 "빈 소비 키" 의 선택 삭제를 바꿈 | 동작 변화 | 의도된 동작이 아님(선택 위 한/영 전환이 텍스트를 지우는 것은 버그). L3 케이스 3 으로 고정. 한 줄 되돌림 가능 |
| GNOME/Wayland 위젯이 commit 시 선택을 치환하지 않음 | 선택 옆 삽입(손실 없음) | 실측 후 `_lastSurrounding` anchor 로 `delete_surrounding` 선호출(v1.1) |
| Mutter `vfunc_set_surrounding` stale | 검증 오탐 → 단음절 퇴화(안전) | 실측 |
| TSF CUAS(synth) 다글자 삭제·오버레이 폴백·OnSetFocus 문서 보존 | 앱별 편차 | 역방향 ATF 검증 경로 재사용. VM 대기 |
| `AutoTypefixApply` 재사용을 프런트가 ATF 로 오인(undo 등) | 부수 처리 | 데몬 `undo_states/recent_corrections` 는 ATF 블록 안에서만 갱신 — 확정 프레임은 블록 미진입. GNOME `expectSelfBackspaces` 는 BS 개수만 |
| 사전 순서 ≠ 빈도 | 후보 품질 | 즐겨찾기 보완, v2 랭킹 |
| 단어 일치 시 음절만 변환 불가(Q7) | UX 막다른 길 | v2 축소 키. PM 판단 |
| 설정 지점 누락 | 저장 무효 | 스펙 §3.2 표를 체크리스트로, U9 단일 단위 |
| Qt UTF-16 오프셋 서로게이트 | 오프셋 어긋남 → 불일치 무동작(안전) | 한글 완성 음절만 허용하는 판정이 대부분 거름. 실측 |
| 헤더 20자 오버플로 | 시각 | ellipsize 3종 |

### 8.2 롤백
- 코어: `resolve_hanja_target` 루프 하한 `2` 를 `HANJA_MAX_KEY_CHARS+1` 로 두면 다음절 탐색이 꺼지고 종전 동작(한 줄). 설정 필드는 `#[serde(default)]` 라 남겨도 무해.
- 데몬(U11)은 U8 API 에 컴파일 의존 → 함께 되돌린다. 프런트(U3~U7)는 각각 독립 revert 가능(게이트·판별자 한 줄, 배선은 이벤트 드롭으로 복귀).
- U14b(POPUP_SPEC v3.4)는 문서 단독 revert + HANJA_WORD_SPEC 상태 되돌림.
- 커밋 전이므로 작업 트리 되돌림으로 충분(D12).

### 8.3 기능 스위치를 두지 않는 이유
"다음절 일치 없음 = 종전" 이 곧 안전망. 스위치는 설정 동기 9곳과 L1/L2 매트릭스를 두 배로 만든다. 필요 시 v1.1 에서 U1+U9 만으로 추가.

---

## 9. 미해결·PM 판단 항목

1. 스펙 §0 Q1~Q9(기현님 승인). Q1 은 U8 대상② 분기·U11·U14b 의 착수 게이트. Q8(D6 불일치 무동작 vs 이모지 폴백)·Q9(Wayland surrounding 미지원 앱 대상① 축소)는 PM 결정 D6·D1 을 건드리므로 별도 판단.
2. Windows CHANGELOG 항목 게재 시점(VM 검증 전/후).
3. L3 GTK4 앱 필드 존재 여부(harness 앱 표) — 없으면 케이스 2 생략.
4. `unim-popup-win` 이 gnu 타깃에서 check 되는지 — 실패 시 렌더러 검증은 VM 으로.
5. TSF `acquire_insert_range` 선택 range 처리(구현자 확인, 필요 시 `InsertTextAtSelection` 분기).
6. 실측 항목: Mutter surrounding 빈도·오프셋 단위, GtkTextView·St.Entry·Chromium·Electron 선택 치환, 앱별 surrounding 보고 지연, popup-service 클릭 시 GTK4 `is_focused` 유지 여부.
7. XIM preedit 선클리어(U5 (a))의 IC 재구성 비용 — 크면 포기하고 SPEC 비고 정정(구현자 판단, 계획 §4 XIM 행).
