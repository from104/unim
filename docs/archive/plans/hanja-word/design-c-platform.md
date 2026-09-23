# 한자 단어 입력 — 설계안 C (플랫폼 커버리지·배선 우선)

기준: `/home/from104/work/unim` develop `ba64255`. 모든 file:line 은 이 트리에서 실측(지도 파일 + 본 설계 중 재확인). 저장소 파일은 수정하지 않았다.

---

## 1. 개요·범위

### 1.1 v1 범위 (PM D1 그대로, 채널 선택만 구체화)

| 대상 | 정의 | 교체 채널 | 지원 프런트 |
|---|---|---|---|
| ① 최근 입력 결합 | 앱에 이미 확정된 한글 접두("대한민") + 라이브 preedit("국")의 **최장 사전 접미** | 접두 삭제가 필요한 경우(`delete_chars>0`)만 **`AutoTypefixApply(delete_chars, commit_text, "")`** 재사용(신규 시그널 없음). 접두가 없으면(Word 모드·단음절) **현행 CommitText/commit_buffer 채널** 그대로 | Linux 6종 전부 + TSF |
| ② 선택 영역 | 앱에서 선택된 한글 단어(앞뒤 공백 trim)가 사전에 **정확히** 있을 때 | `delete_chars=0` + 현행 CommitText/commit_buffer 채널(위젯이 선택을 치환). GTK4·Qt 는 CommitText 콜백에 선택-삭제 래퍼를 추가해 결정적으로 만든다 | GTK4·Qt5/6·GNOME·Wayland(배선 신설)·TSF. **GTK3·XIM 미지원**(선택 정보 자체가 없음 → 현행 idle 동작=이모지) |
| 서식 | `漢字` / `한자(漢字)` / `漢字(한자)` | 코어 `select_hanja` 한 곳에서 조립(키보드·마우스·TSF 자동 동기) | 전 플랫폼, 단음절에도 동일 적용(D2) |

핵심 설계 원칙 3개:

1. **회귀 0 경계 = "접두 삭제가 필요한가"**. 접두 삭제가 없는 모든 경우(현행 단음절, Word 모드, 대상②)는 기존 코드 경로(`popup_dispatch.rs:190-197` `commit_buffer.push_str`, `service.rs:2886-2920` `redirect_commit_and_hide`)를 **바이트 동일**하게 탄다. 새 out-of-band 채널(`HanjaReplacement`)은 대상①-Syllable 에서만 `Some` 이 된다.
2. **InputResult ABI 동결·wire 동결**. `PopupAction`·`PopupRenderPayload`·`unim-popup-win/protocol.rs`·DBus 시그니처 전부 무변경. 신규 정보는 엔진 drain 접근자(`take_hanja_replacement`, `take_atf_toggle` 선례 `engine.rs:604`) + `EngineRequest::SelectHanja` 응답 타입 확장(데몬 내부)로만 흐른다.
3. **최근 커밋 버퍼는 엔진 소유, 단일 훅**. `press_key()` 래퍼 1곳에서 commit_buffer 델타를 스캔한다(§2.1). PM D3 의 "5곳 push 훅"과 결과 동치이면서 12곳의 비한글 리셋 지점(`press_key.rs:291,367,407,426,592,623,645,1169,1258,1313,1331,1336`)과 팝업 5곳(`popup_dispatch.rs:194,201,214,228,233`)을 개별 편집 없이 자동 커버한다(§10 deviations 에 명시).

### 1.2 v2 로 미루는 것 (설계안에 메모만)

- target 축소/확장 키(팝업 안에서 `←/→` 대신 별도 키로 접두 글자 수 조정) — D5.
- 조사 분리("대한민국은" → "대한민국" 만) — ROADMAP.md:126-127 난점. v1 은 정확 접미만: "대한민국은" 을 치고 "은" 조합 중이면 접미 탐색이 "…국은"부터 줄어들다 "은" 단음절 폴백으로 끝난다(회귀 0, 사용자는 "은" 한자 팝업을 본다).
- Wayland `SurroundingText` 를 바이트 계산에도 쓰기(§5.3 은 프런트 자체 커밋 이력으로 확정).
- XIM 대상②(X11 PRIMARY 선택 우회) — selection-surrounding 지도 §7-4.
- IMM32 — 스텁 유지(무변경).
- `hanja_word_conversion` on/off 토글 설정 — 필요해지면 KoreanConfig 에 bool 추가(본 v1 은 토글 없음, 롤백은 §11).

---

## 2. 데이터 모델

### 2.1 최근 커밋 한글 버퍼 `RecentHangul` (신규 `src/input_engine/hanja_word.rs`)

```rust
/// 사전 최장 키 18자(hanja.txt:253444) − preedit 1자 = 17.
pub const RECENT_HANGUL_CAP: usize = 17;

#[derive(Default, Debug)]
pub(super) struct RecentHangul {
    buf: std::collections::VecDeque<char>,   // 완성형 음절(U+AC00..=U+D7A3)만
}
impl RecentHangul {
    pub fn push_syllable(&mut self, c: char)  // 앞에서 밀어내며 CAP 유지
    pub fn pop(&mut self) -> Option<char>
    pub fn clear(&mut self)
    pub fn chars(&self) -> impl Iterator<Item=&char>
    pub fn len(&self) -> usize
}
```

`InputEngine` 신규 필드(`engine.rs:96-126` 블록의 `hanja_target` 옆): `recent_hangul: RecentHangul`. `new()` 기본값 `Default`(`engine.rs:248-262` 리터럴에 추가), `reset()`(`engine.rs:764-784`)에서 `clear()`.

**단일 훅 — `press_key()` 래퍼** (`press_key.rs:58` 의 `pub fn press_key` 본문을 `fn press_key_inner` 로 개명, 새 `press_key` 가 감싼다. `press_key_code`(`:37`)는 `press_key` 를 부르므로 자동 포함):

```rust
pub fn press_key(&mut self, keycode: KeyCode, modifier: ModifierState, config: &Config) -> InputResult {
    let popup_was_active = self.popup_state.is_some();
    let before = self.commit_buffer.len();
    let r = self.press_key_inner(keycode, modifier, config);
    self.track_recent_hangul(keycode, &r, before, popup_was_active);
    r
}
```

`track_recent_hangul` 규칙표 (위에서부터 첫 매치가 아니라 **순서대로 전부 적용**):

| # | 조건 | 동작 | 근거 |
|---|---|---|---|
| 1 | `content_purpose.should_block_hangul()` | `clear()` 후 return | 비번 fail-closed(`surrounding.rs:71-76` 패턴) |
| 2 | `popup_was_active` | `clear()` 후 return | D3 "팝업 확정/취소 후 리셋". 팝업 중 nav 키도 여기 걸리지만 무해(팝업 열릴 때 target 은 이미 확정) |
| 3 | `commit_buffer.len() < before` | `clear()` 후 return | 내부에서 버퍼가 비워진 비정상 경로 방어 |
| 4 | 델타 `commit_buffer[before..]` 의 각 문자 `c` 순회 | `c.is_hangul_syllable()`(`hangul/char.rs:785` `HangulCharExt`) 이면 `push_syllable(c)`, 아니면 `clear()` (순회 계속) | 한글 완성 음절만 축적. 공백·영문·구두점·숫자·이모지·한자·호환자모(`press_key.rs:1313,1331` fallback_jamos) 전부 자동 리셋 |
| 5 | `!r.consumed`(passthrough) | `keycode == Backspace` → `pop()`; `keycode.is_modifier()`(`keycode/mod.rs:146`) → 무동작; 그 외(Enter/Tab/Escape/방향키/Home/End/Delete/PageUp/Down/Ctrl 조합/미지 키) → `clear()` | D3 BS pop·커서 이동 리셋. `committed_passthrough`(Enter 로 조합 커밋 후 통과, `press_key.rs:328-336`)는 4 에서 push 된 뒤 5 에서 clear → "Enter 리셋" 충족 |
| 6 | `self.input_category != Korean` | `clear()` | 한영 전환(토글키·auto-english `press_key.rs:189,274`) — 4 에서 flush 된 음절이 push 된 뒤 여기서 지워진다(순서 중요) |

래퍼 밖의 부수 훅 3곳 (press_key 를 거치지 않는 공개 API):

| 위치 | 동작 |
|---|---|
| `engine.rs:671-678` `set_input_category` | 카테고리가 실제로 바뀌면 `recent_hangul.clear()` (SetGlobalMode RPC `engine_worker.rs:1938` 경유 대비) |
| `surrounding.rs:38-47` `set_content_purpose` 의 `should_block_hangul()` 분기 | `recent_hangul.clear()` 추가 (mid-focus 비번 전환, Qt `input_context.cpp:317-323` 케이스) |
| `engine.rs:820` `chord_idle_flush_commit` | 반환 문자열을 규칙 4 로 스캔(idle 타이머 flush 음절 반영) |
| `engine.rs:686` `set_word_mode` | 토글 시 `clear()` (Word 모드에선 버퍼를 target 에 쓰지 않으므로 위생용) |

리셋 매트릭스(D3 요구 ↔ 구현):

| D3 리셋 조건 | 구현 지점 |
|---|---|
| 비한글 커밋(영문·공백·구두점·숫자·특수·이모지·한자 확정) | 규칙 4 |
| Enter | 규칙 4+5 |
| `engine.reset()` | `engine.rs:764` 에 clear 추가 |
| FocusOut / Reset RPC | `engine_worker.rs:757` `*engine = InputEngine::new()` → 소멸 (TSF 는 `text_service.rs:1868` reset) |
| content_purpose 차단 | 규칙 1 + `set_content_purpose` 훅 |
| 한영 전환 | 규칙 6 + `set_input_category` 훅 |
| 팝업 확정/취소 | 규칙 2 + `select_hanja`/`cancel_hanja` 내부 clear(§4) |
| BS 로 커밋 글자 삭제 | 규칙 5 pop |
| 커서 이동키·마우스 클릭(=Reset) | 규칙 5 clear / Reset RPC |

### 2.2 target 표현 — 접두/preedit/선택 분리

`InputEngine` 신규 필드(`hanja_target: String` 은 **사전 키·표시·즐겨찾기 키**로 그대로 유지):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HanjaTargetKind {
    #[default] PreeditSyllable, // 현행: preedit 마지막 음절(또는 미완성 자모 → 초성 특수문자 폴백)
    RecentWord,                 // 대상①(Syllable 모드): recent_hangul 접두 + preedit
    WordBuffer,                 // 대상①(Word 모드, D4): preedit_cache(word_buffer+preedit) 접미
    Selection,                  // 대상②
}
pub(super) hanja_kind: HanjaTargetKind,
pub(super) hanja_committed_prefix_len: u32, // RecentWord 에서 앱에 이미 나간 글자 수 (= delete_chars). 그 외 0
pub(super) hanja_preedit_part: String,      // 확정 당시 라이브 preedit 전체("국" / Word 모드 전체). Selection 은 ""
pub(super) hanja_commit_prefix: String,     // 확정 시 앞에 붙여 커밋할 문자열: Word 모드 비일치 접두("오늘"), Selection 의 선행 공백
pub(super) hanja_commit_suffix: String,     // Selection 의 후행 공백(trim 보존). 그 외 ""
pub(super) pending_hanja_replacement: Option<HanjaReplacement>,
```

```rust
/// 팝업 확정이 산출한 "앱 텍스트 교체 지시". InputResult(repr(C)) 밖 out-of-band.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HanjaReplacement {
    pub delete_chars: u32,   // 앱에 확정된 글자 중 지울 수(문자 단위) = hanja_committed_prefix_len
    pub commit_text: String, // 서식 적용 최종 문자열 (commit_prefix + 서식 + commit_suffix)
    pub preedit_chars: u32,  // 확정 당시 라이브 preedit 글자 수. Linux 는 preedit_text="" 로 프런트가 지움, TSF 는 keep_text 후 삭제폭에 합산(§5.5)
}
pub fn take_hanja_replacement(&mut self) -> Option<HanjaReplacement>  // popup_pending_action 선례
```

**`Some` 이 되는 조건은 단 하나**: `hanja_kind == RecentWord && hanja_committed_prefix_len > 0`. 나머지는 `None` → 현행 경로 바이트 동일.

취소 시 재커밋 문자열 API(3경로 + TSF 공용):

```rust
/// 취소/포커스아웃 시 앱에 되돌려 줄 텍스트. 접두는 이미 앱에 있으므로 제외.
pub fn hanja_cancel_text(&self) -> String {
    match self.hanja_kind {
        HanjaTargetKind::Selection => String::new(),
        _ => format!("{}{}", self.hanja_commit_prefix, self.hanja_preedit_part),
    }
}
```

현행 `PreeditSyllable` 에서 `hanja_commit_prefix=""`, `hanja_preedit_part=preedit_cache` 이고 preedit_cache 는 Syllable 모드에서 정확히 target 1음절(`input_context.rs:1089-1095` 테스트 근거)이므로 `hanja_cancel_text()==hanja_target` → 취소 경로 바이트 동일. Word 모드에서는 현행이 `hanja_target`(마지막 1음절)만 되돌려 접두를 잃던 결함(`candidates.rs:151-153` 전체 clear + `popup_dispatch.rs:228` target 만 복원)이 고쳐진다 — deviations 에 기재.

### 2.3 `HanjaOutputFormat` enum (`src/config.rs`, `CommitUnit` `:57-89` 패턴 복제)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum HanjaOutputFormat {
    /// 漢字 (기본, 현행 동작)
    #[default]
    Hanja,
    /// 한자(漢字)
    HangulHanja,
    /// 漢字(한자)
    HanjaHangul,
}
impl HanjaOutputFormat {
    pub fn display_name(&self) -> &'static str { "한자만" | "한글(한자)" | "한자(한글)" }  // 코어는 로케일 없음(CLI/GUI 가 i18n 재매핑)
    pub fn all() -> &'static [HanjaOutputFormat] { &[Hanja, HangulHanja, HanjaHangul] }
}
```

serde 태그 = variant 이름 그대로(`Hanja`/`HangulHanja`/`HanjaHangul`, `CommitUnit` 관례 — rename 없음). 위치: `KoreanConfig.hanja_output_format` (`config.rs:664` `commit_unit` 바로 아래, `#[serde(default)]`). 근거: 단축키류(`hanja_keys`)는 `EngineConfig` 최상위, 조합·변환 동작류(`commit_unit`, `word_mode_apps`)는 `KoreanConfig` — 출력 서식은 후자. **주의**: `KoreanConfig` 는 `#[serde(from = "KoreanConfigCompat")]`(`config.rs:605`)이므로 `KoreanConfigCompat`(`:790-817`)에도 `#[serde(default)] hanja_output_format: HanjaOutputFormat` 을 **반드시 추가**하고 `Default`(`:819-834`)와 `From`(`:836-`) 에서 그대로 넘긴다 — 빠뜨리면 YAML 값이 조용히 기본값으로 떨어진다.

### 2.4 서식 조립 규칙

```rust
pub fn format_hanja_output(&self, e: &HanjaEntry) -> String {
    match self.hanja_output_format {
        Hanja       => e.hanja.clone(),
        HangulHanja => format!("{}({})", e.hangul, e.hanja),
        HanjaHangul => format!("{}({})", e.hanja, e.hangul),
    }
}
```

- 한글 부분은 **`HanjaEntry.hangul`**(사전 키와 동일, 동음 다중 항목 國家/國歌 모두 "국가", `hanja.txt:28636-28637`). `hanja_target` 이 아니라 entry 를 쓰는 이유: Word 모드 비일치 접두가 target 에 섞이지 않게 하기 위함(target==entry.hangul 이 항상 성립하지만 의미상 entry 가 옳다).
- 뜻(`meaning`) 은 서식에 넣지 않는다(다음절 27.5만 건 대부분 빈 문자열, `hanja.txt` 실측). 팝업 뜻 컬럼은 현행대로(빈 뜻 우아 축소, popup-pipeline 지도 보충#7).
- 최종 커밋 문자열 = `hanja_commit_prefix + 서식 + hanja_commit_suffix`.
- 즐겨찾기 키 = `(hanja_target, e.hanja)` 원문(서식 무관). `HanjaBookmarkStore` 키는 임의 길이 `String`(`bookmark.rs:19-24`) — 스키마 무변경. 단어 즐겨찾기는 정확히 같은 단어 팝업에서만 재사용된다(UX 트레이드오프, 사용자 매뉴얼에 1줄).
- 후보 정렬 = 현행(즐겨찾기 stable 우선 → `hanja.txt` 등장 순, `candidates.rs:33-36`). "빈도순"은 코드 주석의 미검증 가정(popup 지도 보충#7-3) — §11 위험.

---

## 3. 알고리즘

### 3.1 target 결정 `resolve_hanja_target(&self) -> Option<ResolvedTarget>` (`hanja_word.rs`)

```rust
pub(super) struct ResolvedTarget {
    key: String,                 // 사전 조회 키 = hanja_target
    kind: HanjaTargetKind,
    committed_prefix_len: u32,   // RecentWord 만 >0
    preedit_part: String,        // hanja_preedit_part
    commit_prefix: String,       // hanja_commit_prefix
    commit_suffix: String,       // hanja_commit_suffix
}

fn resolve_hanja_target(&self) -> Option<ResolvedTarget> {
    // (0) 비밀번호 게이트 — pull 경로(GetHanjaCandidates)엔 press_key 게이트가 없다(selection 지도 보충#5)
    if self.content_purpose.should_block_hangul() { return None; }

    let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
    if idle {
        // (1) 대상② — 선택 영역만. 없으면 None(호출부가 emoji/무동작 결정)
        return self.selection_target();
    }

    let pre: Vec<char> = self.preedit_cache.chars().collect();
    let p = pre.len();
    let last = *pre.last()?;
    if !last.is_hangul_syllable() {
        // (2) 미완성 자모 → 현행 규칙(마지막 1글자; 초성이면 특수문자 폴백은 start_hanja_conversion 이 담당)
        return Some(ResolvedTarget { key: last.to_string(), kind: PreeditSyllable,
            committed_prefix_len: 0, preedit_part: self.preedit_cache.clone(),
            commit_prefix: pre[..p-1].iter().collect(), commit_suffix: String::new() });
    }

    if self.is_word_mode() {
        // (3) D4 — word_buffer+preedit 전체의 최장 사전 접미. 접두 삭제 0.
        let s = &pre;
        if let Some(l) = self.longest_dict_suffix(s, 2) {
            return Some(ResolvedTarget { key: s[p-l..].iter().collect(), kind: WordBuffer,
                committed_prefix_len: 0, preedit_part: self.preedit_cache.clone(),
                commit_prefix: s[..p-l].iter().collect(), commit_suffix: String::new() });
        }
        // 폴백: 현행 마지막 음절 — 단, 비일치 접두는 commit_prefix 로 보존(현행 결함 수정)
        return Some(ResolvedTarget { key: last.to_string(), kind: PreeditSyllable,
            committed_prefix_len: 0, preedit_part: self.preedit_cache.clone(),
            commit_prefix: pre[..p-1].iter().collect(), commit_suffix: String::new() });
    }

    // (4) D5 — Syllable 모드: recent_hangul + preedit 의 최장 사전 접미
    let s: Vec<char> = self.recent_hangul.chars().copied().chain(pre.iter().copied()).collect();
    let n = s.len();
    if let Some(l) = self.longest_dict_suffix(&s, std::cmp::max(2, p)) {
        if l > p {
            return Some(ResolvedTarget { key: s[n-l..].iter().collect(), kind: RecentWord,
                committed_prefix_len: (l - p) as u32, preedit_part: self.preedit_cache.clone(),
                commit_prefix: String::new(), commit_suffix: String::new() });
        }
        // l <= p (preedit 안에서 끝남, chord 등 다음절 preedit 희귀 케이스) → 접두 삭제 0
        return Some(ResolvedTarget { key: s[n-l..].iter().collect(), kind: WordBuffer,
            committed_prefix_len: 0, preedit_part: self.preedit_cache.clone(),
            commit_prefix: pre[..p-l].iter().collect(), commit_suffix: String::new() });
    }
    // (5) 다음절 일치 없음 → 현행(마지막 음절) 그대로 — 회귀 0
    Some(ResolvedTarget { key: last.to_string(), kind: PreeditSyllable, committed_prefix_len: 0,
        preedit_part: self.preedit_cache.clone(), commit_prefix: pre[..p-1].iter().collect(),
        commit_suffix: String::new() })
}

/// s 의 접미 중 길이 hi..=lo(내림차순) 로 사전에 있는 최장 길이. hi = min(18, s.len()).
fn longest_dict_suffix(&self, s: &[char], lo: usize) -> Option<usize> {
    let hi = s.len().min(18);
    (lo..=hi).rev().find(|&l| {
        let key: String = s[s.len()-l..].iter().collect();
        self.hanja_dict.contains(&key)   // 신규 dict.rs 메서드: entries.contains_key (search() 의 Vec clone 회피)
    })
}
```

비용: 한자키 1회당 HashMap 조회 ≤17회. `lo=max(2,p)` 인 이유: 길이 1 은 현행 규칙과 동일하므로 탐색 불필요, p>1 이면 preedit 전체를 포함해야 접두 계산이 성립.

### 3.2 대상② 판정 `selection_target()`

```rust
fn selection_target(&self) -> Option<ResolvedTarget> {
    let (text, cur, anc) = self.surrounding_text();          // surrounding.rs:83-89, 비번이면 항상 빈 값
    if text.is_empty() || cur == anc { return None; }
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = (cur.min(anc) as usize, (cur.max(anc) as usize).min(chars.len()));
    let sel = &chars[a..b];
    let lead = sel.iter().take_while(|c| c.is_whitespace()).count();
    let trail = sel.iter().rev().take_while(|c| c.is_whitespace()).count();
    let core = &sel[lead..sel.len()-trail];
    if core.is_empty() || core.len() > 18 { return None; }
    if !core.iter().all(|c| c.is_hangul_syllable()) { return None; }
    let key: String = core.iter().collect();
    if !self.hanja_dict.contains(&key) { return None; }       // 정확 일치만(D6)
    Some(ResolvedTarget { key, kind: Selection, committed_prefix_len: 0, preedit_part: String::new(),
        commit_prefix: sel[..lead].iter().collect(), commit_suffix: sel[sel.len()-trail..].iter().collect() })
}
```

공백을 commit_prefix/suffix 로 되돌려 넣는 이유: 위젯이 **선택 전체**(공백 포함)를 치환하므로 trim 한 만큼 복원해야 문서가 보존된다.

### 3.3 `start_hanja_conversion` 재작성 (`candidates.rs:16-109`)

```
if hanja_mode → consumed (현행 :18-20)
let Some(t) = resolve_hanja_target() else → log + consumed (현행 :107-108 과 동일 결과)
candidates = hanja_dict.search(&t.key)
if candidates.is_empty():
    if t.kind == Selection → consumed (특수문자 폴백 없음, 팝업 없음 — D6)
    else → 현행 초성 특수문자 폴백 (:74-104, ch = t.key 첫 글자) 그대로
else:
    현행 :33-71 그대로 (즐겨찾기 정렬·PopupState::new_hanja_with_top_row·ShowHanja) + 신규 필드 세팅:
        hanja_target = t.key; hanja_kind = t.kind; hanja_committed_prefix_len = t.committed_prefix_len;
        hanja_preedit_part = t.preedit_part; hanja_commit_prefix = t.commit_prefix; hanja_commit_suffix = t.commit_suffix;
    return InputResult::hanja_candidates()   // preedit 유지(팝업 중 "국" 표시), 현행과 동일
```

`press_key.rs:226-238` 한자키 분기:

```rust
if self.hanja_keys.contains(&keycode) {
    self.finalize_chord_buffer();
    let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
    if idle {
        if self.has_selection() {                 // surrounding_text 비어있지 않고 cursor != anchor
            return self.start_hanja_conversion(); // 대상②. 일치 없으면 consumed(무동작·이모지 없음) — D6
        }
        self.start_emoji_popup();                 // 현행 v3.2
        return InputResult::consumed();
    }
    return self.start_hanja_conversion();         // 조합 중: 대상①/현행
}
```

pull 경로(Qt `input_context.cpp:385`, XIM `handler.rs:1171` → `engine_worker.rs:1990`)는 `start_hanja_conversion` 을 직접 부르므로 자동 적용. 후보 없음 → 두 프런트는 특수문자 → `ProcessKey` 폴백(Qt `:408`, XIM `:1215`) → 위 분기에서 `has_selection()` 이면 다시 무동작(이모지 없음) ✓, 선택 없으면 이모지(현행) ✓.

### 3.4 `select_hanja` (`candidates.rs:140-160`)

```rust
pub fn select_hanja(&mut self, index: usize) -> Option<String> {
    if !self.hanja_mode || index >= self.hanja_candidates.len() { return None; }
    let formatted = self.format_hanja_output(&self.hanja_candidates[index]);
    let commit_text = format!("{}{}{}", self.hanja_commit_prefix, formatted, self.hanja_commit_suffix);
    if self.hanja_kind == HanjaTargetKind::RecentWord && self.hanja_committed_prefix_len > 0 {
        self.pending_hanja_replacement = Some(HanjaReplacement {
            delete_chars: self.hanja_committed_prefix_len,
            commit_text: commit_text.clone(),
            preedit_chars: self.hanja_preedit_part.chars().count() as u32,
        });
    }
    // 현행 :150-156 preedit 정리 그대로
    if !self.preedit_cache.is_empty() { self.korean_context.clear(); self.preedit_cache.clear(); }
    self.recent_hangul.clear();          // D3
    self.cancel_hanja();                 // 신규 필드도 여기서 Default 로
    Some(commit_text)                    // RPC 반환값(하위호환: 단음절이면 서식만 다를 뿐 동일 채널)
}
```

`cancel_hanja`(`:266-275`)는 신규 5필드를 기본값으로 되돌리고 `recent_hangul.clear()` 를 추가한다(`pending_hanja_replacement` 는 건드리지 않는다 — 호출자가 drain).

`popup_select`(`popup_dispatch.rs:190-197`):

```rust
if let Some(text) = self.select_hanja(abs_index) {
    if self.pending_hanja_replacement.is_none() {     // 현행 채널
        self.commit_buffer.push_str(&text);
    }                                                 // RecentWord: 호스트가 take_hanja_replacement 로 교체
    self.popup_pending_action = Some(PopupAction::HidePopup);
    return InputResult::committed();                  // preedit_changed=true → 프런트가 preedit 을 지운다
}
```

`popup_cancel`(`:225-230`): `self.commit_buffer.push_str(&self.hanja_target)` → `push_str(&self.hanja_cancel_text())`.

---

## 4. 상태기계·경로표

| 경로 | 진입 | 버퍼(recent) | target/kind | 커밋/교체 | 취소 시 |
|---|---|---|---|---|---|
| 한자키 push (GTK3/4·GNOME·Wayland·TSF·L1) | `press_key.rs:226` → `start_hanja_conversion` | 유지(팝업 열림 시 변경 없음) | §3.1 | — | — |
| 한자키 pull (Qt·XIM) | `GetHanjaCandidates` → `engine_worker.rs:1990` | 유지 | §3.1 (비번 게이트는 §3.1 (0)) | — | — |
| 키보드 확정 (Enter/숫자) | `press_key` → `process_popup_key` → `popup_select` | `select_hanja` 에서 clear; 래퍼 규칙 2 로도 clear | — | `pending` None → `commit_buffer`(현행). `Some` → Linux: `engine_worker` 가 `auto_typefix` 응답 필드에 실어 `AutoTypefixApply`(§5.1). TSF: `take_hanja_replacement` → `replace_surrounding`(§5.5) | — |
| 마우스 확정 (SelectHanja RPC) | `service.rs:2886` → `engine_worker.rs:2013` `select_hanja` + `take_hanja_replacement` | clear | — | 응답 `SelectHanjaOutcome::Commit(text)` → `redirect_commit_and_hide`(현행) / `Replace{delete_chars,text}` → 신규 `redirect_replace_and_hide`(§5.1) | — |
| 마우스 확정 (TSF) | `apply_reverse_event` `key_handler.rs:1171-1173` `press_key(Enter)` | clear | — | `take_hanja_replacement` → `replace_surrounding`(§5.5 지점 2) | — |
| 취소 ① Escape 키 | `popup_dispatch.rs:225` `popup_cancel` | clear(규칙 2) | — | `commit_buffer.push_str(hanja_cancel_text())` = preedit 부분만 | 대상① "국"만 재커밋(접두는 앱에 있음), 대상② 없음, Word "오늘"+"대한민국" 전체 |
| 취소 ② CancelHanja RPC | `engine_worker.rs:2030-2045` | clear | — | `let t = engine.hanja_cancel_text(); engine.cancel_hanja(); Some(t)` → `service.rs:3161-3168` redirect | 동일 |
| 취소 ③ FocusOut/Reset RPC | `engine_worker.rs:723-769` `reset_engine_and_capture_commit` `:737-742` | 소멸(엔진 재생성) | — | `if is_hanja_mode() { let t = engine.hanja_cancel_text(); engine.cancel_hanja(); push t }` | 동일 |
| 취소 ④ TSF OnSetFocus | `text_service.rs:1868` `engine.reset()` 직전 | reset 이 clear | — | §5.5 래퍼 | 조합 텍스트는 문서에 잔존(OnCompositionTerminated) |
| content_purpose 비번 진입 | `surrounding.rs:38` / TSF `key_handler.rs:417` / `engine_worker.rs:2246` | clear | `resolve` (0) 이 None | — | — |
| 한영 전환 | `set_input_category` / 래퍼 규칙 6 | clear | — | — | — |
| ATF 발동 | `engine_worker.rs:1562/1629` `engine.reset()` 후 replay `press_key` | reset → clear → replay 로 교정 음절 재축적 | — | — | — |

배타성: `pending_hanja_replacement` 는 `select_hanja` 확정 시점에만 `Some` 이 되고, 그 키 처리에서 ATF `process_after_key`/`check_*` 는 팝업 활성(`popup_action.is_some()` `engine_worker.rs:1343,1698`; TSF `!popup_active` `key_handler.rs:765`)으로 건너뛰므로 같은 프레임에 두 교체가 동시에 `Some` 인 경우는 없다(windows 지도 보충#8.4 확인).

---

## 5. 교체 실행

### 5.1 데몬 (`unim-dbus`)

**(a) 키보드 확정 push 경로** — `engine_worker.rs` `ProcessKeyEvent` 핸들러. `press_key`(`:1268`) 이후, 응답 조립(`:1709-1725`) 직전에:

```rust
// 한자 단어 교체(대상①): 팝업 확정 키 처리에서 코어가 남긴 교체 지시를 AutoTypefixApply 채널로 실어 보낸다.
// ATF 와 배타(팝업 활성 프레임에서 ATF 는 :1343/:1698 게이트로 미발동).
if let Some(rep) = engine.take_hanja_replacement() {
    debug_assert!(auto_typefix_result.is_none());
    auto_typefix_result = Some((rep.delete_chars, rep.commit_text, String::new()));
}
```

`:1709-1725` 의 `(final_preedit, final_commit)` 분기는 `auto_typefix_result.is_some() && !fix_has_replay` → `(Some(String::new()), None)`: preedit 클리어 응답 + commit 억제 — 우리 케이스에 정확히 필요한 조합(commit_buffer 는 비어 있음). `service.rs:2156-2166` 이 `signal_ctx`(= 키를 보낸 owner 컨텍스트) 로 `AutoTypefixApply` 를 발행한다. **수정 없음**.

**(b) 마우스 확정 pull 경로** — `EngineRequest::SelectHanja.response` 타입(`service.rs:67-71`) 을 `oneshot::Sender<SelectHanjaOutcome>` 로:

```rust
pub enum SelectHanjaOutcome { None, Commit(String), Replace { delete_chars: u32, commit_text: String } }
```

`engine_worker.rs:2013-2028`:

```rust
let result = match contexts.get_mut(&target_id).and_then(|e| e.select_hanja(index).map(|t| (t, e.take_hanja_replacement()))) {
    None => SelectHanjaOutcome::None,
    Some((t, None)) => SelectHanjaOutcome::Commit(t),
    Some((_, Some(rep))) => SelectHanjaOutcome::Replace { delete_chars: rep.delete_chars, commit_text: rep.commit_text },
};
```

`service.rs:2886-2920` `select_hanja`:

```rust
let (hanja, outcome) = ...;
match outcome {
    Commit(t) => { self.redirect_commit_and_hide(&t).await; t }
    Replace { delete_chars, commit_text } => { self.redirect_replace_and_hide(delete_chars, &commit_text).await; commit_text }
    None => String::new(),
};
Ok(hanja)   // RPC 시그니처 `s` 유지 (popup-service dbus_server.rs:208-213 forward, GNOME dbus_ime.js:701 는 반환값 무시)
```

신규 헬퍼 `redirect_replace_and_hide(&self, delete_chars: u32, commit_text: &str)` — `redirect_commit_and_hide`(`service.rs:1807-1864`) 복제, `CommitText` 대신:

```rust
self.connection.emit_signal(None::<&str>, &path, "org.atit.unim.InputContext", "AutoTypefixApply",
    &(delete_chars, commit_text.to_string(), String::new())).await
```

이어서 `HidePopup` 동일 발행. path = `last_active_input_context_path`(popup owner). 각 프런트 구독은 path-scoped(Wayland `dbus_client.rs:664`, XIM `dbus_client.rs:309-310`, GTK `gtk-common/unim_dbus_client.c:1139-1174`, Qt `qt-common/unim_dbus_client.cpp:572`, GNOME `dbus_ime.js:275-288` path 필터) 이므로 owner 에게 정확히 도달한다(atf 지도 보충#1-(2) 검증).

**(c) 취소 3경로** — §4 표. `engine_worker.rs:737-742`, `:2035-2040` 두 곳을 `hanja_cancel_text()` 로 치환.

**(d) DBus SPEC** — `unim-dbus/SPEC.md:254-256`(GetHanjaCandidates target 이 다음절 가능·SelectHanja 반환은 서식 적용 문자열·CancelHanja 반환은 preedit 부분만), `:285`(AutoTypefixApply 가 한자 단어 교체에도 쓰이며 그때 `preedit_text=""`, `delete_chars`=접두 글자 수).

### 5.2 프런트엔드 매트릭스 (Linux 6 + TSF)

| 프런트 | 대상① 교체 수신 | 대상② 선택 읽기 | 대상② 치환 | 수정 file:line | 문제·해법 |
|---|---|---|---|---|---|
| GTK4 | `immodule.c:463-533` `on_auto_typefix`: `delete_surrounding(-N,N)` → XTest BS 폴백 → 커밋 → `unim_emit_preedit("")`. **무수정** | `:1248-1275` `set_surrounding_with_selection` 매 키 전 `retrieve-surrounding`(`:865`) → 항상 최신 ✓ | 키보드 확정: `:1061-1086` 래퍼가 `result.consumed` 에서 선택 삭제 후 `result.commit` 커밋 ✓. 마우스: `:451-461` `on_commit_text` 는 래퍼 미경유 → **선택 삭제 래퍼를 공용 static 으로 추출해 `on_commit_text` 첫 줄에서 호출**(권장, 결정적) | `immodule.c:451-461`(래퍼 호출 추가), `:1061-1086`(함수 추출), `gtk4/SPEC.md` | 마우스 경로 preedit 은 `on_auto_typefix :531-532` 가 지움 ✓ |
| GTK3 | `immodule.c:396-462` 동형. **무수정** | 구조적 불가(`set_surrounding_with_selection` vtable 없음, `:1220-1222` anchor=cursor) → 데몬에 선택이 절대 안 보임 → idle=이모지 현행 | — | `gtk3/SPEC.md` 1줄(대상② 미지원 명시) | — |
| Qt5/6 | `input_context.cpp:177-222`: `setCommitString(text, -N, N)`(Konsole 은 `\b`×N 프리픽스) + 같은 이벤트로 preedit "" ✓. **무수정** | `:554-564` 포커스 1회만 → stale. **F9 분기(`:381`) 첫 줄에서 `QInputMethodQueryEvent(ImSurroundingText|ImCursorPosition|ImAnchorPosition)` 재질의 후 `m_dbus->setSurroundingText(...)` 를 빈 문자열이어도 무조건 전송**(stale 클리어) | 키보드: `:446-463` 래퍼 ✓. 마우스: `:224-231` `setCommitTextCallback` 람다 첫 줄에 `:449-462` 와 동일한 선택 삭제 블록 호출(헬퍼 `deleteSelectionIfAny()` 추출) | `qt5/src/input_context.cpp:381-385, :224-231, :446-463`, qt6 동일 라인대(`:386, :225, :450`), `qt5/SPEC.md`·`qt6/SPEC.md` | Qt 오프셋은 UTF-16 code unit(selection 지도 §7-5) — 한글 BMP 는 1:1, 이모지 섞인 선택은 `is_hangul_syllable` 검사에서 어차피 탈락 |
| XIM | `handler.rs:515-577` N+1 BS 주입 → `:1071-1109` commit → `!has_preedit` 이면 `DbusRequest::Reset`(`:1104-1107`). **무수정** | 불가(surrounding 코드 전무) → idle=이모지 현행 | — | `xim/SPEC.md` 2줄(대상② 미지원, Reset 부작용 무해 근거) | Reset 무해성 §5.4. 마우스 확정 시 on-screen preedit 잔존은 **현행 단음절 마우스 경로와 동일**(`PopupEvent::CommitText :578-612` 도 preedit 을 안 지움) — 다음 키의 ProcessKey 응답(`:462-480`)에서 자연 정리. 키보드 확정은 ProcessKey 응답 preedit "" 가 시그널보다 먼저 도착(동기 RPC 응답 후 비동기 시그널) ✓ |
| Wayland | `state.rs:248-297` `apply_auto_typefix`. **바이트 계산 수정 필수**(§5.3) | `:626-628` 드롭 중 → **배선 신설**(§5.3) | `delete 0` + `im.commit_string` → text-input v3 클라이언트(GTK/Qt 위젯)가 commit 시 선택 치환 — 표준 위젯 동작이나 저장소 밖 → L3 실측 항목 | `state.rs:248-297, :626-628, :563-619, :225-246, :307-360`, `dbus_client.rs:17-41(enum), :346-354 옆`, `wayland/SPEC.md` | — |
| GNOME | `extension.js:283-306` `onAutoTypeFix`: `expectSelfBackspaces` + `vkbd.backspaceMultiple(N)` → 50ms → `commitText`. **1줄 보강**: 백스페이스 전에 `if (this._inputMethod._preeditText.length>0) this._inputMethod.clearPreedit();`(`unim_input_method.js:728-735`) — 마우스 경로는 ProcessKey 응답이 없어 preedit "국" 이 떠 있는 채로 BS 가 가므로 먼저 지운다(키보드 경로는 `onUpdatePreedit` 이 먼저 옴) | `unim_input_method.js:562-568` `vfunc_set_surrounding` → `dbus_ime.js:646` ✓ (호출 빈도는 Mutter 의존 — 실측) | `commitText`(`:637-652`) → `this.commit(text)` → Mutter → 앱 위젯이 치환(위와 동일 실측 항목) | `extension.js:283-306`, `popup_view.js:98`(§7) | `delete_surrounding` 은 문자 단위(`:715-723`) — 바이트 문제 없음 |
| TSF | 코어 상속 + §5.5 | `composition.rs:1637-1656` `read_selection_text` pull(§5.5) | `insert_text`(`composition.rs:639`, `:1483-1496` `acquire_insert_range` → `SetText`) 가 선택 range 를 덮어쓰는지 구현자 확인(§5.5) | §5.5 | — |
| IMM32 | 무변경(스텁) | — | — | — | — |

### 5.3 Wayland — 바이트 수 (D9) + SurroundingText 배선

**결정: 프런트 자체 커밋 이력으로 바이트 실측, 실패 시 현행 휴리스틱 폴백.** 근거: (i) Wayland 프런트는 영문 모드 글자도 엔진이 소비해 `commit_string` 으로 보내므로(`press_key.rs:645` → `state.rs:229`) ATF 순·역방향의 삭제 대상은 전부 프런트가 스스로 커밋한 글자다 → 이력 꼬리의 UTF-8 길이 합 = 정답. (ii) 이력이 ASCII 만이면 `N×1`, 한글만이면 `N×3` 으로 현행 휴리스틱과 **동일값** → ATF 회귀 0. (iii) 이력이 무효(포워딩된 키·포커스 전환) 이면 폴백이 현행과 바이트 동일. `SurroundingText` 기반은 앱 갱신이 커밋 직후 비동기라 ATF 시점에 1자 stale 위험이 있어 바이트 계산에는 쓰지 않는다(v2 메모).

`State`(`state.rs:50-107`) 신규 필드:

```rust
commit_history: std::collections::VecDeque<char>,   // 최근 64자, 프런트가 commit_string 한 글자
history_valid: bool,
surrounding_pending: Option<(String, u32, u32)>,    // (text, cursor_bytes, anchor_bytes) — Done 에서 적용
```

| 지점 | 동작 |
|---|---|
| `apply_input_result :229` `commit_string(commit)` | `commit.chars()` 를 history 에 push(64 초과 시 앞에서 pop) |
| `apply_auto_typefix :282` | `delete` 처리 후 `commit_text.chars()` push |
| `apply_auto_typefix :251-278` | `before_bytes = if history_valid && history.len() >= delete_chars { history.iter().rev().take(delete_chars).map(|c| c.len_utf8()).sum() } else { 현행 휴리스틱 }`; 이어서 history 꼬리 `delete_chars` 개 pop |
| `forward_key :299`(미소비 키 포워딩) | `key`가 BackSpace(evdev 14) 이고 press 이면 `history.pop_back()`, 그 외 press 는 `history.clear(); history_valid=false` 는 과하므로 **`history_valid=false`** (다음 activate 까지 폴백) |
| `handle_deactivate :307` / Done-activate `:569` | `history.clear(); history_valid = true` |
| `handle_deactivate :320,:336,:354` 커밋 | push (직후 clear 되므로 실질 무의미, 일관성용) |

`SurroundingText` 배선(대상② 감지):

```rust
zwp_input_method_v2::Event::SurroundingText { text, cursor, anchor } => {   // :626-628
    state.surrounding_pending = Some((text, cursor, anchor));                 // 바이트 오프셋(프로토콜 xml:124-127)
}
// Done(:563) 의 `else if state.current_active` 분기(:606-619) 및 activate 분기 끝에:
if let Some((text, cb, ab)) = state.surrounding_pending.take() {
    let to_chars = |b: u32| text.get(..(b as usize).min(text.len())).map(|s| s.chars().count()).unwrap_or(0) as u32;
    let _ = state.dbus_tx.blocking_send(DbusRequest::SetSurroundingText {
        context_path: state.context_path.clone(), text, cursor: to_chars(cb), anchor: to_chars(ab) });
}
```

`dbus_client.rs:17-41` `DbusRequest` 에 `SetSurroundingText { context_path, text, cursor, anchor }` 추가, 처리는 `:346-354` `SetContentType` 과 동형(`proxy.set_surrounding_text(&text, cursor, anchor)` — 프록시 메서드는 `unim-dbus/src/client.rs:173` 에 이미 존재). 앱이 surrounding 을 미지원하면 이벤트가 안 와 대상② 가 조용히 비활성(현행 idle 동작). `wayland/SPEC.md` 에 두 사항 기록.

### 5.4 XIM — Reset 부작용 무해성 증명

`handler.rs:1103-1108`: `preedit_text==""` 이면 N+1 BS 완료 후 `DbusRequest::Reset{context_path}` 발행 → `engine_worker.rs:1923-1936` → `reset_engine_and_capture_commit`(`:723-769`). 그 시점 엔진 상태: `select_hanja` 가 이미 `korean_context.clear()`+`preedit_cache.clear()`+`cancel_hanja()`+`recent_hangul.clear()` 를 마쳤고 commit_buffer 는 비어 있으며(`popup_select` 가 push 하지 않음) chord 버퍼도 비어 있다 → `:733-753` 캡처 전부 `None` → `:757` `InputEngine::new` 재생성(모드 보존 `:758-760`, word 게이트 `:762`) → 응답 `None` → `service.rs` Reset 핸들러의 CommitText 메아리도 없음. 즉 **관측 가능한 부작용 0** (즐겨찾기 스토어는 `load_default()` 로 같은 파일을 재로드, `hanja_bookmarks` 토글은 매번 `save()`(`bookmark.rs:76`) 하므로 일관). 유일한 경합: 사용자가 (N+1)×10ms(`:569-571`) + 앱 처리 전에 다음 자모를 치면 그 preedit 이 Reset 캡처로 커밋된다 — **현행 역방향 ATF 와 동일한 창**이며 새 위험이 아니다(`xim/SPEC.md` 에 명시). 회피 불필요.

### 5.5 TSF (`unim-tsf`) — drain 접근자·2지점·ReplaceOutcome·반환형·OnSetFocus

**공용 헬퍼** `fn apply_hanja_replacement(engine, comp_mgr, context: &ITfContext, tid, comp_sink, composition_unsupported: bool, preedit_win: &mut ..., rep: HanjaReplacement) -> bool /*schedule_flush*/` (key_handler.rs 신규):

```rust
let mut delete_span = rep.delete_chars;
if comp_mgr.is_active() {
    // 역방향 ATF 와 동일 패턴(key_handler.rs:861-867): 라이브 조합("국")을 keep_text 로 materialize 한 뒤 전체 span 삭제.
    comp_mgr.end_composition_keep_text(context, tid);           // composition.rs:570
    delete_span += rep.preedit_chars;
} else if composition_unsupported {
    // 오버레이 폴백: preedit 은 문서에 없고 preedit_win 에 있다 → 오버레이만 비우고 span 은 접두만.
    (폴백 경로가 preedit 빈 문자열일 때 쓰는 동일 호출로 preedit_win 클리어)
}
let outcome = comp_mgr.replace_surrounding(context, tid, delete_span, &rep.commit_text, "", comp_sink); // composition.rs:663
let mut schedule_flush = false;
match outcome {                                                  // key_handler.rs:445-454 와 바이트 동일한 4갈래
    ReplaceOutcome::Normal => {}
    ReplaceOutcome::PhaseSplit => schedule_flush = true,        // preedit="" 라 미발생(형식상)
    ReplaceOutcome::SynthBatch => engine.remove_preedit(),
    ReplaceOutcome::SynthHeadTail => { let _ = crate::synth_input::discard_pending_tail(); engine.remove_preedit(); }
}
schedule_flush
```

**지점 1 — 키보드 확정** `key_handler.rs:587` `drain_popup_actions` 직후, `:597` "commit / preedit 처리" 진입 전:

```rust
if let Some(rep) = engine.take_hanja_replacement() {
    let schedule_flush = apply_hanja_replacement(..., rep);
    return KeyDownOutcome { eaten: true, schedule_flush, ..Default::default() };   // :456 패턴. ATF 오케스트레이션(:753-)·word 힌트(:589-595) 건너뜀
}
```

조기 반환이 안전한 이유: 이 프레임의 `result` 는 `committed()`(commit "" / preedit "") 이라 `:673-748` 일반 경로가 할 일은 `end_composition`(:739-742) 뿐인데 그것을 헬퍼가 `keep_text` 로 대체했다. `schedule_flush` 는 `text_service.rs:1451-1470` 이 기존대로 소비.

**지점 2 — 마우스 확정** `key_handler.rs:1218` `drain_popup_actions` 직후, `:1220` commit_buffer 블록 전:

```rust
let mut schedule_flush = false;
if let Some(rep) = engine.take_hanja_replacement() {
    match context {
        Some(ctx) => schedule_flush = apply_hanja_replacement(engine, comp_mgr, ctx, tid, comp_sink, /*composition_unsupported*/ ..., rep),
        None => dbg_log("popup_rev: no context — hanja replacement dropped"),   // :1242-1247 동일 degrade
    }
}
... (기존 commit_buffer 블록 그대로: RecentWord 면 비어 있으므로 no-op) ...
schedule_flush   // 반환형 () → bool
```

`apply_reverse_event` 시그니처 `-> bool`. 호출부 `text_service.rs:2436-2447` `rev_drain_and_apply(ctx)` → `rev_drain_and_apply(ctx, hwnd)` 로 확장(`rev_wnd_proc :2217-2228` 이 `hwnd` 를 갖고 있다): 반환 `true` 면 루프 내 가드(engine/config/comp/popup/ctx_guard)를 `drop` 한 뒤 `SetTimer(hwnd, FLUSH_TIMER_ID, FLUSH_DELAY_MS)`(= `:1461-1470` `arm_flush_timer` 와 동일 타이머 id/지연; `WM_UNIM_FLUSH2`/`WM_TIMER` 처리부 `:2252-` 가 그대로 받는다). `composition_unsupported`·`preedit_win` 은 `RevWndContext` 에 없으면 `apply_reverse_event` 인자로 추가(호출부 1곳).

**대상② (TSF)**: 한자키 처리 직전(`key_handler.rs:557` `press_key` 앞, `test_key_down :184` 이 이미 Hanja/F9 를 소비) 에:

```rust
if keycode == KeyCode::Hanja || keycode == KeyCode::F9 {
    if atf_active {   // :417 로컬 게이트(비번이면 COM 호출 자체 생략)
        match composition::read_selection_text(context, tid) {            // composition.rs:1637
            Some(sel) if sel.cursor != sel.anchor => engine.set_surrounding_text(sel.surrounding_text, sel.cursor, sel.anchor),
            _ => engine.set_surrounding_text(String::new(), 0, 0),          // stale 클리어
        }
    }
}
```

확정은 현행 채널(commit_buffer → `:734` `insert_text` 또는 `:1236`). **구현자 확인 항목**: `acquire_insert_range`(`composition.rs:1490`) 가 비어 있지 않은 selection range 를 그대로 돌려주면 `SetText` 가 선택을 덮어써 치환 완료. Collapse 한다면 `ITfInsertAtSelection::InsertTextAtSelection(TF_IAS_NOQUERY)` 로 삽입하는 분기 추가(TSF 계약상 선택 치환). `replace_surrounding` 은 `:1319` `Collapse(TF_ANCHOR_START)` 때문에 선택 앞 글자를 지우므로 대상② 에 **쓰지 않는다**(수동 typefix `:434-442` 가 이 경로를 타는 것은 기존 이슈 — 본 범위 밖, §11).

**OnSetFocus 캡처 래퍼** (`text_service.rs:1867-1870`):

```rust
} else {
    if engine.is_hanja_mode() {
        // 한자 팝업 중 진짜 포커스 이탈: 라이브 조합은 OnCompositionTerminated 가 문서에 남기므로 재커밋 불필요.
        // 오버레이 폴백(composition_unsupported)에서는 preedit 이 문서에 없다 → last_context 가 있으면 insert_text, 없으면 로그.
        let t = engine.hanja_cancel_text();
        engine.cancel_hanja();
        if composition_unsupported && !t.is_empty() { (last_context 로 insert_text 시도, 실패 시 dbg_log) }
    }
    engine.reset();
    engine.set_word_mode(word_mode);
}
```

`:1878` `popup_ipc.hide()` 는 이미 뒤에서 팝업을 닫는다. 이 래퍼는 D1 이 지적한 "target 유실" 을 (a) 정상 앱: 텍스트 잔존 확인·상태만 정리, (b) 오버레이 앱: 가능하면 복구 로 닫는다.

**동기화 사본**: `popup_ipc.rs` ↔ `protocol.rs` 는 필드 추가가 없으므로 **무변경**(골든 테스트 `popup_ipc.rs:1263-` 그대로 통과). 렌더러 수정은 §7(로컬).

**검증**: `make check-windows`(Makefile:520-524, WIN_CRATES `:491` 에 unim-tsf 포함) 만 가능. 런타임은 VM 대기(§11).

---

## 6. 설정 (`hanja_output_format`)

| # | 지점 | file:line | 작업 |
|---|---|---|---|
| 1 | 코어 enum·필드 | `src/config.rs:57-89` 옆 enum 신설, `:664` 아래 `#[serde(default)] pub hanja_output_format: HanjaOutputFormat`, `:685` Default, **`:790-817` Compat 필드 + `:819-834` Default + `:836-` From** | §2.3 |
| 2 | 엔진 캐시 | `engine.rs:191` 옆 `pub(super) hanja_output_format: HanjaOutputFormat`, `:245-246` `new()`, `:948-950` `rebuild_korean_context` | 두 곳 동기화(config-sync 지도 보충#12) |
| 3 | 비파괴 setter | `engine.rs:613` `set_atf_hotkeys` 옆 `pub fn set_hanja_output_format(&mut self, config: &Config)` | fingerprint(`engine_worker.rs:382-397`)에 **넣지 않음** |
| 4 | 데몬 리로드 루프 | `engine_worker.rs:988` `set_switch_keys` 직후 `engine.set_hanja_output_format(&config);` | 조합 중에도 즉시 반영, `select_hanja` 다음 호출부터 적용 |
| 5 | TSF 리로드 | `text_service.rs:462-463` `InputEngine::new` 재생성 | 자동(무수정) |
| 6 | DBus YAML/JSON | `service.rs:1161-1265` serde 자동. 레거시 `get_config/set_config`(`:719-`) 는 `commit_unit` 전례대로 **생략** | 무수정 |
| 7 | CLI | `unim-cli/src/main.rs:616-617` 옆 `#[value(name="hanja-output-format", help=h("help_ck_hanja_output_format"))] HanjaOutputFormat`; `:74-80` 옆 `hanja_output_format_display_name_localized`; `:886-891` 옆 show 라인; `:1701-1724` 옆 set 암(`"hanja"\|"한자"` / `"hangul-hanja"\|"한글한자"` / `"hanja-hangul"\|"한자한글"`, 오류 `error_invalid_hanja_output_format`) | 대화형 메뉴는 전례(commit_unit 미포함)대로 생략 가능 |
| 8 | CLI 로케일 | `unim-cli/locales/ko.yml`·`en.yml` — `:44-45` 옆 `hanja_output_format_label`/`_note`, `:88` 옆 `error_invalid_hanja_output_format`, `:106` 옆 `hanja_output_format_changed`, `:120-122` 옆 `hanja_output_format_hanja`/`_hangul_hanja`/`_hanja_hangul`, `:378` 옆 `help_ck_hanja_output_format` | 두 파일 같은 줄 수·순서 유지 |
| 9 | GTK 설정 | `unim-settings-gtk/src/settings_dialog.rs:691-726` `commit_row` 를 템플릿으로 `hanja_format_row`(`adw::ComboRow`, 3항목) 를 `:726` 뒤에 추가, `save_and_notify(&s.config, "hanja_output_format")`. `:16` import 에 enum 추가 | 슬라이더 정책 무관(열거형=ComboRow 관례) |
| 10 | GTK 로케일 | `unim-settings-gtk/locales/{ko,en}.yml` `:35-38` 옆 `row_hanja_output_format`, `hanja_output_format_{hanja,hangul_hanja,hanja_hangul}`; `:164` 옆 `row_hanja_output_format_subtitle`; `:194` 옆 `row_hanja_output_format_tooltip`("한자 변환 확정 시 넣을 형식. 漢字 / 한자(漢字) / 漢字(한자). 단어·음절 변환 모두 적용") | 라이브 도움말=툴팁 |
| 11 | GTK merge 화이트리스트 | `unim-gui-common/src/settings_helpers.rs:208` 뒤 `merge_field(&mut d.korean.hanja_output_format, bk.map(\|k\| &k.hanja_output_format), &u.korean.hanja_output_format);` | 누락 시 저장이 씹힘 |
| 12 | Slint UI | `unim-settings/ui/settings.slint:329` commit_unit 콤보 옆에 `in-out property <[string]> hanja_output_format_options; in-out property <int> hanja_output_format_index;` + ComboBox | display_name 문자열 사용(commit_unit 과 동일) |
| 13 | Slint 바인딩 | `unim-settings/src/main.rs:842-854` 옆 options/index 세팅, `:935-938` 옆 저장 역변환, `:573-576` 옆 `merge_ui_owned` 항목 | 두 화이트리스트 각각 |
| 14 | (선택) TSF 레거시 모달 | `unim-tsf/src/settings_dialog.rs:689-691`(콤보 구성)·`:1247-1252`(역변환) 패턴 복제 | Slint 폴백 전용이라 선택 — 넣지 않으면 그 UI 에서만 안 보임(값은 보존됨) |
| 15 | 문서 | `docs/user/user-guide/README-ko.md:375-446` §4.2 에 "한자 단어 변환"·"출력 형식" 소절, `README.md:375-` 영문 동일; `tools/gen-help` 는 빌드 시 HTML 자동 재생성(코드 수정 없음) | — |

핫리로드 흐름: config.yaml 저장 → `reload_if_changed`(2초 throttle, `config.rs:1349-`) → `engine_worker.rs:978-988` 루프에서 `set_hanja_output_format` 매 컨텍스트 재적용(비파괴). TSF 는 mtime 감지 시 엔진 재생성.

---

## 7. 팝업·렌더러

| 렌더러 | file:line | 수정 |
|---|---|---|
| 뷰모델 | `src/popup/view_model.rs:310-320` expanded 헤더 `「{target}」 → {hanja} {meaning}` | 무수정(문자열 연결). 8자 target 이면 20자 안팎 → 렌더러 ellipsize 로 처리 |
| GTK4 popup-service | `unim-popup-service/src/popup/hanja.rs:102-107` `target_label` | `target_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);` 추가(`:348` meaning_label 과 동일). CSS `max-width:420px` 안에서 잘림 대신 … |
| GNOME | `unim-gnome-extension/popup_view.js:98` `this._header` | 생성 직후 `this._header.clutter_text.set_ellipsize(Pango.EllipsizeMode.END);` (`:373` meaningLbl 과 동일 패턴) |
| Windows 헤더 | `unim-popup-win/src/render.rs:375` `draw_text(..., DT_VCENTER \| DT_SINGLELINE \| DT_LEFT)` | `\| DT_END_ELLIPSIS` 추가 |
| Windows compact 한자 컬럼 | `render.rs:504-522` `hanja_rect` 폭 `s(90)`, `mean_left = hanja_left + s(96)` | `let hanja_w = text_width(hdc,&main,font_main).max(s(90,scale)) + s(6,scale);` 로 rect 폭·`mean_left = hanja_left + hanja_w` 동적화. 팝업 전체 폭은 `:307-329` 가 이미 행별 실측이라 무수정 |
| Windows expanded 셀 | `render.rs:197` `CELL_W: i32 = 44` 고정 | 렌더 시 `cell_w = CELL_W.max(max_cell_text_width + s(8))` 로 동적(9열 × cell_w 가 팝업 폭). 단어 후보(2~4자) 겹침 방지 |
| GTK/GNOME expanded 셀 | CSS `.grid-cell{min-width:30px}` | 무수정(min 이라 자연 확장; 열 폭 들쭉날쭉은 허용) |
| PopupState/PopupRender/wire | `popup_state.rs:90-122`, `unim-popup-types`, `popup_ipc.rs`/`protocol.rs` | 무수정(모두 `String`) |

---

## 8. POPUP_SPEC 개정안 초안 (승인 대기 — `docs/dev/specs/HANJA_WORD_SPEC.md` 내 절로 수록, POPUP_SPEC.md 는 무수정)

> ### POPUP_SPEC 개정안 (승인 대기) — v3.4 제안
>
> **§3.7 규칙 2 (현행 `:237`)** "**대상**: preedit의 마지막 음절 (예: "대한민국" → "국")" → 다음으로 교체:
>
> 2. **대상**: 다음 우선순위로 결정한다.
>    - (a) preedit 이 없고 앱 선택 영역이 있으면: 선택 텍스트(앞뒤 공백 제외)가 전부 완성형 한글이고 18자 이하이며 사전에 정확히 있을 때 그 단어. 아니면 팝업을 띄우지 않는다(이모지 팝업으로 넘어가지 않는다).
>    - (b) preedit 이 있고 마지막 글자가 완성형 음절이면: 음절 확정 모드에서는 「최근 확정된 한글 음절(최대 17자) + preedit」, 단어 확정 모드에서는 「preedit 전체」의 접미 가운데 사전에 있는 **최장** 접미(2자 이상). 없으면 preedit 의 마지막 음절(종전 규칙).
>    - (c) preedit 의 마지막 글자가 미완성 자모이면: 종전 규칙(마지막 글자, 초성이면 규칙 7 특수문자 전환).
>    - 최근 확정 음절 버퍼는 비한글 확정·Enter·한/영 전환·커서 이동·포커스 이탈·비밀번호 필드·팝업 확정/취소 시 비워진다.
>
> **§3.7 규칙 4** 끝에 추가: "확정 문자열은 설정 `hanja_output_format`(漢字 / 한자(漢字) / 漢字(한자)) 서식을 적용한 값이며, 대상이 이미 앱에 확정된 접두를 포함하면 프론트엔드는 `AutoTypefixApply(delete_chars=접두 글자 수, commit_text=서식 문자열, preedit_text="")` 로 접두를 지우고 넣는다. 선택 영역 대상은 `delete_chars=0` 이며 위젯이 선택을 치환한다."
>
> **§3.7 규칙 5** "preedit(원래 한글) 유지" → "취소 시 앱에 아직 나가지 않은 preedit 부분만 확정 텍스트로 되돌린다(이미 확정된 접두는 그대로, 선택 영역 대상은 아무것도 넣지 않는다)". (현행 코드 `cancel_hanja`+`popup_cancel` 동작과의 불일치 정정.)
>
> **§9.2 idle Hanja 키 dispatch 정책 (v3.2, `:606-609`)** 예외 추가: "단, idle 이더라도 앱 선택 영역이 있으면 emoji popup 대신 §3.7 규칙 2(a) 선택 단어 변환을 시도하고, 사전 일치가 없으면 아무 팝업도 띄우지 않는다."
>
> **§11 변경 이력** 행 추가: `| YYYY-MM-DD | v3.4 | 한자 단어 변환 — 대상 규칙 개정(최근 확정 음절 결합·선택 영역), 출력 서식 설정, 취소 규칙 정정, 헤더 ellipsize |`

HANJA_WORD_SPEC.md 본문(W12)은 본 설계안 §2~§5 를 사양 문체로 옮기고 위 개정안 절을 말미에 둔다.

---

## 9. 테스트 계획

### L1 — `src/input_engine/tests_hanja_word.rs` (신규, `mod.rs:27-46` 에 `#[cfg(test)] mod tests_hanja_word;`)

헬퍼: `create_test_engine()`(`test_helpers.rs:9`) + 두벌식 `press_key` 시퀀스 매크로(`type_syllables(&mut engine, "대한민국")` — 기존 `tests_scenarios.rs:286-287` R/K 패턴 확장). 사전은 실 `hanja.txt` 를 쓴다(`대한민국:大韓民國` 단일 항목 `hanja.txt:57963`, `국가` 다중 `:28636-28637`).

| # | 케이스 | 단언 |
|---|---|---|
| 1 | recent_word_basic: "대한민" 확정 + "국" 조합 → `start_hanja_conversion` | `hanja_target=="대한민국"`, `hanja_kind==RecentWord`, prefix_len 3, preedit_part "국", `preedit_str()=="국"`(유지) |
| 2 | recent_word_select: 1 에 이어 `select_hanja(0)` | 반환 "大韓民國", `take_hanja_replacement()==Some{3,"大韓民國",1}`, `commit_str()==""`, recent 비어 있음, `!is_hanja_mode()` |
| 3 | recent_word_popup_select_no_commit_buffer: Enter 키로 확정(`press_key(Enter)`) | `commit_str()==""`, pending Some, `take_popup_action()==HidePopup` |
| 4 | legacy_single_syllable_unchanged: "가" → start/select | `hanja_kind==PreeditSyllable`, pending None, 기존 `test_scenario_hanja_conversion`(`tests_scenarios.rs:277-303`) 그대로 통과 |
| 5 | no_multi_match_fallback: "닭" 확정 + "국" → start | target "국"(현행), pending None |
| 6 | format_hangul_hanja / format_hanja_hangul: `set_hanja_output_format` 후 "가" select | `"{hangul}({hanja})"` / `"{hanja}({hangul})"` (후보[0] 로 기대값 생성); 단어에도 동일 |
| 7 | password_gate: `set_content_purpose(Password)` 진입 시 recent 비워짐; 비번 중 타이핑 후 `start_hanja_conversion` | 팝업 없음(`consumed`, `!is_hanja_mode()`), recent 길이 0 |
| 8 | word_mode_full / word_mode_prefix: `set_word_mode(true)`; "대한민국" / "오늘대한민국" | kind WordBuffer, prefix_len 0, select → `commit_str()=="大韓民國"` / `"오늘大韓民國"` |
| 9 | word_mode_fallback_keeps_prefix: Word 모드 "닭국" | target "국", select → commit "닭" + 후보 (현행 결함 수정 확인) |
| 10 | backspace_pop: "대한" 확정 상태(ㄱ 조합 중) → BS(preedit 비움) → BS(passthrough) | recent "대"; 이어서 "한" 재입력 시 "대한" |
| 11 | reset_conditions(파라미터화): Space·Enter·Tab·Escape·Left·Home·영문 토글·`reset()`·이모지/특수문자 확정·한자 확정·`set_input_category(English)` | recent 비어 있음 |
| 12 | cap_17: 20음절 연속 | `recent.len()==17`, 마지막 17자 |
| 13 | cancel_three_paths: (a) Escape → `commit_str()=="국"`(접두 제외); (b) `hanja_cancel_text()=="국"`; (c) Selection 대상 취소 → `""`; (d) Word 모드 "오늘대한민국" 취소 → "오늘대한민국" |
| 14 | selection_basic: `set_surrounding_text("대한민국 만세",0,4)` idle → start | kind Selection, target "대한민국"; select → `commit_str()=="大韓民國"`, pending None |
| 15 | selection_trim: `" 대한민국 "`, 0..6 | target "대한민국", commit `" 大韓民國 "` |
| 16 | selection_reject: "뷁뷁"(사전 없음) / "abc" / 19자 / cursor==anchor | `consumed`, `!is_hanja_mode()`, 이모지 팝업도 아님(`press_key(Hanja)` 경유 시) |
| 17 | selection_vs_composing: 선택 있음 + "국" 조합 중 | 대상① 경로(RecentWord/PreeditSyllable) |
| 18 | idle_no_selection_emoji: 선택 없음 idle `press_key(F9)` | 이모지 팝업(현행) |
| 19 | atf_regression: 기존 `src/auto_typefix/tests.rs`, `tests_atf_hotkey.rs`, `tests_scenarios.rs`, `tests_popup_change_page.rs`(`:23-33` 헬퍼는 `hanja_target` 만 세팅 — 신규 필드 Default 로 호환) 전량 통과 |
| 20 | bookmark_word_key: 단어 target 에서 `toggle_hanja_bookmark(0)` → `is_bookmarked("대한민국", "大韓民國")` | 재정렬·`HanjaCandidatesReordered` 현행 |

### L2 — `tests/unim-test-dbus/src/main.rs` (`test_hanja_popup :169-` 옆)

- `test_hanja_word_recent`: `set_global_mode(true)`, `focus_in`, 두벌식 evdev 키 `E(18) O(24) G(34) K(37) S(31) A(30) L(38) S(31) R(19) N(49) R(19)`(대한민국; 기존 `:187-191` 매핑과 동일 체계, 구현자가 `Layout::Dubeolsik` 분기로 검증), Hanja(123) → `get_hanja_candidates()` target=="대한민국" → `InputContextProxy` 에 `receive_auto_typefix_apply` 스트림 구독 후 `select_hanja(0)` → 시그널 `(3, "大韓民國", "")` 수신 단언(2초 타임아웃). 세벌식 분기는 스킵 표기.
- `test_hanja_word_selection`: `set_surrounding_text("대한민국 만세", 0, 4)`(`client.rs:173`) → `process_key_event(0,123,0)` → `get_hanja_candidates` target=="대한민국" → `select_hanja(0)` 반환 "大韓民國" + `CommitText` 시그널 수신.
- `test_hanja_selection_no_match_no_emoji`: `set_surrounding_text("abc def",0,3)` → Hanja 키 → `get_hanja_candidates` 빈 결과 + `ShowEmojiPopupV2` 미발행(짧은 대기).

### L3 — `tests/harness/scenarios/hanja.json` (신규)

```json
[{ "name": "hanja-word-recent", "desc": "음절 확정 모드에서 대한민+국 → F9 → 1 이 大韓民國 으로 교체되는가",
   "layout": "ko_2bulstd", "korean": true, "field": "core.plain",
   "steps": [ { "keys": ["e","o","g","k","s","a","l","s","r","n","r"], "expect": { "committed": "대한민", "preedit": "국" } },
              { "key": "F9" }, { "key": "1", "expect": { "committed": "大韓民國", "preedit": "", "rendered": "大韓民國" } } ] }]
```

`harness.py:498-514` 스텝 어휘(`key`/`keys`)와 `expect` 3키로 표현 가능(docs-rules 지도 보충#11). Linux 기본 Smart=음절이라 `commit_unit` 변경 불필요 → **harness.py 확장은 '선택'**(서식 시나리오 `hanja-word-format` 은 `set_config("hanja_output_format")` 자동 적용이 필요 — `:398-408` layout 패턴 복제, 비용 작으나 선택). Wayland 네이티브 앱은 xtest 스킵(`:388-390`).

### Windows

`make check-windows`(Makefile:520-524) 경고 0 + `cargo test -p unim`(코어 L1 은 플랫폼 중립). `unim-popup-win` 은 WIN_CRATES 밖이므로 `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu` 를 W7 검증에 추가. 런타임(VM) 검증 항목: 지점1/2 교체, CUAS 앱 synth 경로, 대상② `insert_text` 선택 치환, OnSetFocus 래퍼.

---

## 10. 작업 분해 (WBS) — 파일 소유권 비중첩

| id | 단위 | 파일(소유) | 선행 | 난이도 | 검증 |
|---|---|---|---|---|---|
| W1 | 코어: 버퍼·target·서식·drain·설정 enum | `src/config.rs`, `src/input_engine/{engine.rs, press_key.rs, candidates.rs, popup_dispatch.rs, surrounding.rs, mod.rs}`, **신규** `src/input_engine/hanja_word.rs`, `src/input_engine/tests_hanja_word.rs`, `src/hanja/dict.rs`(`contains`) | — | 어려움 | `cargo test -p unim` 전량(신규 20 + 기존), `cargo build --workspace` 경고 0 |
| W2 | 데몬 배선 | `unim-dbus/src/service.rs`, `unim-dbus/src/engine_worker.rs`, `unim-dbus/SPEC.md` | W1 | 어려움 | `cargo test -p unim-dbus`, L2(W13) |
| W3 | GTK/Qt C++ 프런트 | `unim-frontends/gtk4/src/immodule.c`, `gtk4/SPEC.md`, `gtk3/SPEC.md`, `unim-frontends/qt5/src/input_context.cpp`, `qt6/src/input_context.cpp`, `qt5/SPEC.md`, `qt6/SPEC.md` | — (W2 와 의미 의존, 컴파일 독립) | 쉬움 | `make build` 경고 0, L3 gtk3/gtk4/qt 앱 |
| W4 | Wayland | `unim-frontends/wayland/src/{state.rs, dbus_client.rs}`, `wayland/SPEC.md` | — | 보통 | `cargo test -p unim-frontend-wayland`(크레이트명 확인), ATF 순/역방향 수동 회귀, 대상② 실측 |
| W5 | XIM | `unim-frontends/xim/SPEC.md` (코드 무변경; 필요 시 `handler.rs:1103` 주석만) | — | 쉬움 | L3 xim 앱 시나리오 |
| W6 | GNOME | `unim-gnome-extension/extension.js`, `unim-gnome-extension/popup_view.js` | — | 쉬움 | `make check-compat`(정적·동적 검증, CHANGELOG-ko 0.4.2 항목), GNOME Wayland 실측 |
| W7 | 렌더러 | `unim-popup-service/src/popup/hanja.rs`, `unim-popup-win/src/render.rs` | — | 쉬움 | GTK 실행, `cargo check -p unim-popup-win --target x86_64-pc-windows-gnu` |
| W8 | TSF | `unim-tsf/src/key_handler.rs`, `unim-tsf/src/text_service.rs` | W1 | 어려움 | `make check-windows` 경고 0, VM 대기 |
| W9 | CLI | `unim-cli/src/main.rs`, `unim-cli/locales/{ko,en}.yml` | W1 | 쉬움 | `cargo test -p unim-cli`, `unim-cli config set hanja-output-format hangul-hanja` 수동 |
| W10 | GTK 설정 | `unim-settings-gtk/src/settings_dialog.rs`, `unim-settings-gtk/locales/{ko,en}.yml`, `unim-gui-common/src/settings_helpers.rs` | W1 | 쉬움 | 앱 실행 후 저장 → config.yaml 반영 → 데몬 로그 setter 적용 |
| W11 | Slint(+선택 TSF 모달) | `unim-settings/ui/settings.slint`, `unim-settings/src/main.rs`, (선택) `unim-tsf/src/settings_dialog.rs` | W1 | 쉬움 | `cargo build -p unim-settings`, `make check-windows` |
| W12 | 문서 | `CHANGELOG-ko.md`, `CHANGELOG.md`, `docs/user/user-guide/README-ko.md`, `README.md`, `ROADMAP.md`, **신규** `docs/dev/specs/HANJA_WORD_SPEC.md` | — | 쉬움 | CHANGELOG 규칙(AGENTS.md:170-197: 한 줄·명사형·한/영 항목 수 동일) 검토 |
| W13 | L2/L3 테스트 | `tests/unim-test-dbus/src/main.rs`, **신규** `tests/harness/scenarios/hanja.json`, (선택) `tests/harness/harness.py` | W2 | 보통 | `make test-dbus`(타깃명 확인), `python tests/harness/run.py` |

병렬 파: 1파 = W1 ∥ W3 ∥ W4 ∥ W5 ∥ W6 ∥ W7 ∥ W12. 2파(W1 후) = W2 ∥ W8 ∥ W9 ∥ W10 ∥ W11. 3파(W2 후) = W13. 통합 후 `cargo build --workspace`·`cargo test --workspace`·`make build`·`make check-windows` 4종 그린. 커밋 없음(D12).

CHANGELOG 초안(Unreleased, 추가됨):
- ko: "한자 변환에서 방금 입력한 여러 음절과 앱에서 선택한 단어를 한 번에 한자 단어로 바꾸는 기능 지원" / "한자 확정 형식 설정 추가 — 漢字, 한자(漢字), 漢字(한자) ([설정] › 한자 확정 형식)"
- en: "Added multi-syllable Hanja conversion for just-typed words and app text selections" / "Added Hanja output format setting — 漢字, 한자(漢字), 漢字(한자) ([Settings] › Hanja output format)"

---

## 11. 위험·롤백·미해결

**위험**

| 위험 | 완화 |
|---|---|
| 사전 후보 순서가 빈도순이라는 보장 없음(`hanja.txt` 원 저작 순서) | v1 은 현행 정렬 유지 + 즐겨찾기로 보정. 문서에 명시 |
| 조사 붙은 어절("대한민국은") 은 단음절 폴백 | v2 조사 분리/target 축소 키 |
| Qt 오프셋 UTF-16 단위 | 한글 BMP 1:1; 비한글 포함 선택은 판정에서 탈락 |
| GNOME/Wayland 대상② 치환이 위젯 동작에 의존 | 치환 안 되면 삽입만 됨(데이터 손실 없음). L3/실측으로 확인, 미치환 위젯 목록을 SPEC 에 기록 |
| TSF `acquire_insert_range` 선택 치환 여부 미확인 | §5.5 구현자 확인·분기 |
| XIM Reset 경합(N+1 BS 완료 전 타이핑) | 현행 ATF 역방향과 동일 창, SPEC 명시 |
| Wayland 이력 무효 시 폴백이 '한자(漢字)' 형식에서 1/3 삭제 | 무효는 포커스 전환·포워딩 키 직후에만 발생. 그 직후 한자 단어 교체는 recent 버퍼도 비어 있어(규칙 5·FocusOut) 대상① 자체가 발동하지 않음 → 실질 0 |
| 팝업 중 preedit 유지("국")와 대상① 확정 순서 | Linux: 응답 preedit "" 가 시그널보다 선행(동기 RPC). TSF: keep_text 로 materialize 후 span 삭제(역방향 ATF 검증 경로) |
| `press_key` 래퍼가 모든 경로를 감싸 성능 | 델타 스캔 O(len(delta)), 대부분 0~1자 |

**롤백**: wire/DBus 시그니처·설정 파일 스키마(신규 필드는 `#[serde(default)]`) 모두 하위호환. W1+W2 revert 로 기능 전체가 꺼지고 프런트(W3~W7) 변경은 잔존해도 무해(래퍼는 선택 없으면 no-op, Wayland 이력은 ATF 정확도 향상). 설정 필드는 남겨도 무시된다.

**미해결(실측 필요)**

1. Mutter `vfunc_set_surrounding` 호출 빈도(GNOME 대상② stale 가능성).
2. text-input v3 클라이언트(GTK4/Qt 위젯)·ClutterText 가 `commit` 시 선택을 치환하는지(저장소 밖 소스).
3. TSF `acquire_insert_range` 의 selection 처리, `read_selection_text` 의 CUAS 앱 정확도(`composition.rs:1601-1603` 방어 코드 존재).
4. `unim-tsf/src/settings_dialog.rs` 가 활성 UI 인지(Slint 폴백 조건) — W11 선택 항목의 필요성.
5. 수동 typefix(`key_handler.rs:434-442`)가 `replace_surrounding` 의 `Collapse(START)`(`composition.rs:1319`) 때문에 선택 앞 글자를 지우는지 — 기존 이슈 의심, 본 범위 밖 보고.
6. `tests/harness` 의 `field.render` 가 preedit 을 정확히 실어 주는 앱 목록(하네스 기존 전제).
