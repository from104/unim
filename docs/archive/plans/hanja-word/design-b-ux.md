# 한자 단어 입력 — 설계안 B-ux (사용자 경험 우선)

작성 기준: develop `ba64255`. 모든 코드 근거는 `file:line`(이해 단계 지도 7종 + 이번 설계에서 직접 재확인한 조각). 저장소 파일은 수정하지 않았다.

설계 각도: **키 시퀀스 최소화 · 인지 용이성 · 실패 시 명확한 피드백**. 입력 한 번 한 번의 비용을 줄이는 것이 이 입력기의 핵심 가치이므로 "F9 → 숫자 1개"라는 기존 2키 시퀀스를 단어 변환에서도 그대로 유지하고, 새 키·새 모드를 만들지 않는다. 그 위에서 코어·프런트엔드·Windows·설정·문서·테스트까지 구현자가 재설계 없이 착수할 수 있도록 전부 명세한다.

---

## 1. 개요·범위

### 1.1 한 줄 요약
한자키(F9/Hanja)가 지금은 "preedit 마지막 1음절"만 바꾼다(`candidates.rs:23-29`). 이를 (①) "방금 친 어절 전체(앱에 이미 나간 접두 + 조합 중 음절)"와 (②) "앱에서 선택한 한글 단어"로 넓힌다. 후보 팝업·9개/페이지·즐겨찾기·키 바인딩은 기존 그대로(POPUP_SPEC §3 재사용), 확정 시 "이미 나간 접두를 지우고 넣기"만 AutoTypeFix 의 `AutoTypefixApply` 채널을 재사용한다. 확정 문자열 서식(漢字 / 한자(漢字) / 漢字(한자))은 설정 1개로 고른다.

### 1.2 v1 범위 (PM D1 기준)
| 항목 | v1 | 근거·비고 |
|---|---|---|
| 대상① 최근 커밋 + preedit 결합 | Linux 전 프런트(GTK3/4·Qt5/6·XIM·Wayland·GNOME) + Windows TSF | 교체 채널 `AutoTypefixApply` 는 6종 전부 구독 중(atf-replacement 보충#1 표) |
| 대상② 선택 영역 변환 | GTK4·Qt5/6·GNOME·Wayland(배선 신설)·TSF. **GTK3·XIM 미지원**(현행 idle 동작 유지) | GTK3 는 anchor vtable 부재(`gtk3/immodule.c:1220-1222`), XIM 은 surrounding 개념 부재 |
| 출력 형식 설정 | 단음절 변환 포함 전 경로 동일 적용(D2) | 조립은 코어 `select_hanja` 한 곳(windows-parity §4-6 권고) |
| Word 모드(word_buffer) | delete=0, preedit_cache 전체의 최장 접미 일치, 접두 비일치는 확정 시 접두+한자 동시 커밋(D4) | hanja-core 보충#10 |
| 팝업 | 9개/페이지·즐겨찾기·키 현행. 헤더 긴 target 축약·Windows compact 한자 열 폭 가변 | popup-pipeline 보충#7 |
| 다음절 후보 "뜻" | **글자별 뜻 합성 표시**(신규, UX) | 다음절 항목 뜻은 거의 비어 있음(`hanja.txt:28636` `국가:國家:`) |
| IMM32 | 무변경(스텁) | windows-parity §4-7 |
| Windows 검증 | `make check-windows` cross-compile 만 | 이 환경에서 런타임 불가 |

### 1.3 v2 로 미루는 것 (설계 메모만)
- **target 축소/확장 키**(D5): 팝업이 열린 상태에서 한자키 재타 → 접미를 1음절씩 줄이기(대한민국 → 한민국 → 민국 → 국), Shift+한자키 → 늘리기. v1 은 §3.1 의 "혼합 후보 리스트"(단어 후보 뒤에 음절 후보를 이어 붙임)로 막다른 길만 막는다(§11 deviation 1).
- **조사 분리**(ROADMAP.md:126-127 난점): "대한민국은" 에서 "대한민국" 잘라내기 — 조사 목록 + 최장일치. v1 은 매뉴얼에 "조사 붙이기 전에 한자키" 를 안내.
- **XIM content_purpose** 부재(selection-surrounding 보충#5): 신규 위험이 아니라 기존 구조적 공백. v1 은 문서에 명시.
- **Standalone(Qt/XIM) 대상② 의 stale 선택**: Qt 는 v1 에서 F9 시점 재질의로 해결(§5.3), XIM 은 대상 밖.
- **XIM 커서 점프 감지**: XIM 은 클릭 시 Reset 도 surrounding 도 없어 §3.1 정합성 검증이 비활성이다. v2 에서 `XNSpotLocation`(`set_ic_values`) 좌표가 직전 커밋 위치와 불연속이면 `recent_commit` 만 비우는 경량 RPC(또는 Reset)를 검토. v1 은 헤더 표시 + Esc 가 방어선.
- **ATF 순방향 교정 뒤 버퍼 시드**: 순방향 교정은 `engine.reset()`(`engine_worker.rs:1562`) 뒤 마지막 음절만 replay 하므로, 교정된 접두("대한민")는 버퍼에 없다 — 교정 직후 F9 는 마지막 음절만 잡는다. v2 에서 `engine_worker` 가 `fix.commit_text` 의 완성 음절을 `recent_commit` 에 시드하면 해결(정보는 이미 그 자리에 있음). v1 은 제외(교정 직후 한자 변환은 드묾).

---

## 2. 데이터 모델

### 2.1 최근 커밋 음절 버퍼 `RecentCommitBuffer` (신규 `src/input_engine/recent_commit.rs`)

```rust
/// 앱에 이미 확정(커밋)돼 나간 **한글 완성 음절**만을 최근순으로 보관한다.
/// 대상①의 접두("대한민") 재료. 앱 문서를 읽지 않으므로 XIM/GTK3/Wayland 에서도 동작.
pub(super) struct RecentCommitBuffer {
    syllables: String,          // U+AC00..=U+D7A3 만 허용
}
pub(super) const RECENT_COMMIT_CAP: usize = 17;   // = HANJA_MAX_KEY_LEN(18) - preedit 1
impl RecentCommitBuffer {
    pub fn push_committed(&mut self, text: &str)  // 전 문자가 완성 음절이면 append(초과분 앞에서 제거), 아니면 clear()
    pub fn pop_last(&mut self)                    // BS passthrough 1자 pop
    pub fn clear(&mut self)
    pub fn as_str(&self) -> &str
    pub fn is_empty(&self) -> bool
}
```
- 상한 17 의 근거: 실측 최장 다음절 키 18자(`hanja.txt:253444`, popup-pipeline 보충#7(4)). 메모리 ≤ 68B.
- `push_committed` 는 **한 번의 push 에 여러 음절**이 올 수 있다(chord flush·word flush) — 전부 완성 음절이면 통째 append.
- 비한글이 섞인 push 는 append 가 아니라 **clear** 다(D3 "비한글 커밋 → 리셋"). 즉 영문·공백·구두점·숫자·특수문자·이모지·한자 확정·분해 자모(`press_key.rs:1313,1331` fallback_jamos) 전부 리셋.

#### push/pop/reset 표 (hanja-core 보충#3 분류 그대로)
| 사건 | 훅 위치(file:line) | 버퍼 동작 |
|---|---|---|
| 한글 음절 확정(5곳) | `press_key.rs:510`(chord OFF 즉시), `:1102-1108`(`flush_preedit`), `:1160`, `:1252`, `:1277` — 모두 `get_committed()→push_str→clear` 3단 패턴 | `push_committed(committed)` (게이트 §2.6) |
| 비한글 commit(한글 모드 도달 가능 9곳) | `press_key.rs:407`(context_alt fallback 리터럴), `:426`(Special 자모), `:592`(chord OFF 비자모 char — 쉼표·숫자·기호가 여기로 온다), `:1169`, `:1258`(chord NonJamo), `:1313`, `:1331`(fallback_jamos 분해 자모), `:1336`(non_jamos) — 각 `commit_buffer.push*` 직후 | **`clear()` 필수.** 예: "대한민" + `,` + "국" 에서 쉼표가 버퍼를 안 끊으면 pool 이 "대한민국" 이 되고 확정 시 `delete_chars=3` 이 앱의 "한민," 를 지운다(문서 손상). `:291`(auto-english char) 는 직후 `set_input_category` 가 clear 하므로 별도 훅 불필요, `:367`/`:623`(Space)·`:645`(영문 모드 char) 는 아래 Space 행·한/영 전환 행이 이미 덮는다(영문 모드에서는 버퍼가 항상 비어 있음) |
| (안전망) F9 시점 surrounding 정합성 검증 | `candidates.rs` `resolve_hanja_target` 4단계 진입 직전 (§3.1) | `surrounding_text` 가 비어 있지 않으면 커서 앞 텍스트가 `recent_commit` 으로 끝나는지 확인, 아니면 `clear()` — 훅이 못 잡는 문맥 단절(마우스 클릭 후 이어 치기, 앱 자체 자동교정, Reset 을 안 보내는 Wayland)을 사전 조회 전에 걸러낸다 |
| Space / Enter / Tab / Escape | `press_key.rs:362-369`, `:328-336`, `:339-347`, `:350-359` | `clear()` (flush 뒤에) |
| Backspace, preedit 없음(passthrough) | `press_key.rs:315-326` `not_consumed` 분기 | `pop_last()` |
| 커서·편집키(Left/Right/Up/Down/Home/End/PageUp/PageDown/Delete/Insert) 및 그 밖의 **비소비·비수정자** 키 | `press_key()` 의 `not_consumed` 반환 직전 공통 지점(한 곳: `process_korean_key`/`process_english_key` 의 최종 `not_consumed`) | `clear()` — 단 `KeyCode::is_modifier()`(`src/keycode/mod.rs:146`) 는 제외 |
| 한/영 전환 | `engine.rs:671-678 set_input_category` | `clear()` (flush_preedit 뒤) |
| 팝업 확정/취소(한자·특수·이모지) | `candidates.rs:266 cancel_hanja`(select/cancel 공통 종단), `popup_dispatch.rs:190-222 popup_select` 특수·이모지 분기, `popup_dispatch.rs:225-241 popup_cancel` | `clear()` |
| `engine.reset()` | `engine.rs:764-784` | `clear()` (hanja_target 클리어 옆) |
| `InputEngine::new` 재생성 | `engine_worker.rs:756 *engine = InputEngine::new(config)` (FocusOut/Reset RPC), TSF `text_service.rs:461-463` maybe_reload_config | 자동 빈 버퍼 |
| content_purpose 차단 진입 | `surrounding.rs:38-44 set_content_purpose` should_block 분기 | `clear()` + 이후 push 는 게이트가 append 대신 clear (fail-closed) |
| 마우스 클릭 | GTK/Qt/GNOME → Reset RPC → 재생성. **Wayland·XIM 은 Reset 을 안 보냄**(selection-surrounding 보충#4 표) | Wayland 는 §3.1 의 surrounding 정합성 검증(F3b 로 surrounding 이 들어오므로)이 stale 버퍼를 걸러낸다. XIM 만 잔여 위험: 클릭으로 커서를 옮긴 뒤 이어 치면 비연속 문자열이 결합될 수 있음 → 팝업 헤더에 엉뚱한 단어가 뜨면 Esc(취소는 preedit 부분만 재커밋, §4) — 문서 손상 없음. v2: XIM `XNSpotLocation` 점프 감지(§1.3) |

정상 연속 타이핑 중에는 Reset/FocusOut 이 오지 않음이 확인됐으므로(selection-surrounding 보충#4 결론) 버퍼가 살아남는다.

### 2.2 변환 대상 표현 — `hanja_target` 을 쪼갠다

현 `hanja_target: String`(`engine.rs:123`)은 "사전 키 = 복원 원문 = 즐겨찾기 키" 세 역할을 겸한다. 대상①에서는 사전 키("대한민국")와 복원 원문("국")이 다르므로 분리한다.

```rust
/// 이번 변환의 출처 — 확정·취소·포커스아웃 경로가 이 값으로 분기한다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HanjaTargetSource {
    /// 현행: preedit 마지막 1글자(또는 초성). 확정=preedit 치환, 취소=target 재커밋.
    Preedit,
    /// 대상① 음절 모드: 접두 `prefix_len` 자는 앱에 이미 나감, 마지막 1자는 preedit.
    RecentAndPreedit { prefix_len: u32 },
    /// 대상① Word 모드: 전부 미커밋(preedit_cache). `nonmatch_prefix` 는 사전 비일치 접두("오늘").
    WordPreedit { nonmatch_prefix: String },
    /// 대상②: 앱 선택 영역. 앱에 나간 글자 0(위젯이 선택을 치환). 취소 시 아무것도 안 함.
    /// `ws_prefix`/`ws_suffix` 는 선택 영역에서 trim 한 앞뒤 공백 — 확정 문자열에 되붙여 공백을 보존.
    Selection { ws_prefix: String, ws_suffix: String },
}

/// 확정 시 접두 교체가 필요할 때만 채워지는 out-of-band 페이로드 (InputResult ABI 불변).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HanjaReplacement { pub delete_chars: u32, pub commit_text: String }
```
`InputEngine` 신규 필드(`engine.rs:117-123` 한자 필드 묶음 옆):
| 필드 | 타입 | 의미 |
|---|---|---|
| `recent_commit` | `RecentCommitBuffer` | §2.1 |
| `hanja_source` | `HanjaTargetSource` | 위 |
| `hanja_restore_text` | `String` | 취소/포커스아웃 시 **재커밋할 원문** — Preedit: target 그대로 / RecentAndPreedit: 마지막 음절 1자 / WordPreedit: preedit_cache 전체 / Selection: "" |
| `pending_hanja_replacement` | `Option<HanjaReplacement>` | `select_hanja` 가 RecentAndPreedit 일 때만 세팅, 호스트가 `take_pending_hanja_replacement()` 로 드레인(`take_atf_toggle` 선례 `engine.rs:604-606`) |
| `hanja_output_format` | `HanjaOutputFormat` | config 캐시(§6) |
| `pending_ui_feedback` | `Option<UiFeedback>` (선택) | 대상② 불일치 비프용 드레인 `take_ui_feedback()` (§3.2) |

`hanja_target` 의 의미는 "**사전 키 = 즐겨찾기 키 = 헤더 표시 문자열**"로 좁힌다(전체 어절). "복원 원문" 역할은 `hanja_restore_text` 로 이관 — 이관 대상 3경로: `popup_dispatch.rs:225-231`, `service.rs:3147-3168`+`engine_worker.rs:2030-2045`, `engine_worker.rs:722-750`(hanja-core 보충#2 결론).

### 2.3 `HanjaOutputFormat` (`src/config.rs`, `CommitUnit` 패턴 `config.rs:58-90` 복제)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(C)]
pub enum HanjaOutputFormat {
    /// 漢字 만 (기본 — 현 동작과 바이트 동일)
    #[default]
    Hanja,
    /// 한자(漢字)
    HangulHanja,
    /// 漢字(한자)
    HanjaHangul,
}
impl HanjaOutputFormat {
    pub fn display_name(&self) -> &'static str { /* "한자만", "한글(한자)", "한자(한글)" — 코어 fallback 라벨, UI 는 i18n 키 */ }
    pub fn all() -> &'static [HanjaOutputFormat] { &[Hanja, HangulHanja, HanjaHangul] }
    /// 서식 조립 — 유일한 조립 지점. hangul 은 HanjaEntry.hangul(=사전 키 전체).
    pub fn format(&self, hangul: &str, hanja: &str) -> String {
        match self { Hanja => hanja.into(), HangulHanja => format!("{hangul}({hanja})"), HanjaHangul => format!("{hanja}({hangul})") }
    }
}
```
- serde: 프로젝트 관례대로 rename 없음(YAML 값 `Hanja`/`HangulHanja`/`HanjaHangul`).
- 괄호는 반각 `(` `)` — 한국어 본문 관례. 상수 두 개(`HANJA_FMT_OPEN/CLOSE`)로 두어 바꾸기 쉽게.
- `#[repr(C)]` 판별자 순서 불변(Hanja=0).

### 2.4 서식 조립 규칙
| 경우 | hangul 값 | 결과 예(HangulHanja) |
|---|---|---|
| 단음절(Preedit) | `entry.hangul` = "국" | 국(國) |
| 대상① 단어 | `entry.hangul` = "대한민국" (사전 키와 동일, `config-sync 보충#12(2)`) | 대한민국(大韓民國) |
| 동음 다중 항목(국가:國家 / 국가:國歌, `hanja.txt:28636-28637`) | 둘 다 "국가" | 국가(國家) / 국가(國歌) — 서식은 동일, 구분은 §2.5 합성 뜻이 담당 |
| Word 모드 접두 비일치("오늘"+"대한민국") | 접두는 서식 밖 | 오늘대한민국(大韓民國) |
| 뜻 없음 | 서식과 무관(뜻은 표시용) | — |
| 즐겨찾기 키 | `(hanja_target, entry.hanja)` — 서식 무관, 현행 `bookmark.rs:57-78` 그대로 | `{"대한민국":["大韓民國"]}` |

### 2.5 다음절 후보의 뜻 합성 (UX 신규, `src/hanja/dict.rs`)
문제: 다음절 항목 275,020건의 뜻이 비었거나 표제어를 그대로 되풀이한다(`hanja.txt:57963` `대한민국:大韓民國:대한민국`). 9개/페이지 목록에서 國家/國歌 를 구분할 단서가 없다.

해결: 단음절 항목 28,474건(`rg -c '^[가-힣]:'`)의 뜻(`가:家:집 가`, `국:國:나라 국` `hanja.txt:55-56,28578`)으로 역색인을 만들어 글자별 뜻을 이어 붙인다.
```rust
pub struct HanjaDictionary {
    entries: HashMap<String, Vec<HanjaEntry>>,
    /// (한자 1자, 그 한자의 한글 음) → 단음절 뜻. 파싱 중 hangul.chars().count()==1 인 줄에서 채움.
    char_meaning: HashMap<(char, char), String>,
    max_key_len: usize,                              // 실측 18 — RECENT_COMMIT_CAP 검증용
}
impl HanjaDictionary {
    pub fn contains_key(&self, hangul: &str) -> bool           // search() 의 Vec clone 회피
    pub fn max_key_len(&self) -> usize
    /// 표시용 뜻: 원본 뜻이 있고 표제어와 다르면 그대로, 아니면 글자별 뜻을 "·" 로 결합.
    pub fn display_meaning(&self, e: &HanjaEntry) -> String {
        if !e.meaning.is_empty() && e.meaning != e.hangul { return e.meaning.clone(); }
        let parts: Vec<&str> = e.hanja.chars().zip(e.hangul.chars())
            .filter_map(|(hj, hg)| self.char_meaning.get(&(hj, hg)).map(String::as_str)).collect();
        parts.join(" · ")           // 예: "나라 국 · 집 가" / "나라 국 · 노래 가"
    }
}
```
- 단음절 뜻에 쉼표 다중 뜻이 있으면(`국:局:부분 국, 판 국`) 첫 쉼표 앞만 쓴다("부분 국") — 행 폭 절약.
- 비용: 시작 시 28k 삽입(기존 30만 줄 파싱 대비 미미). `HanjaEntry` 구조체는 불변.
- 적용 지점: 후보 조립(`candidates.rs` `open_hanja_popup`, `toggle_hanja_bookmark` 재조회) 에서 `(e.hanja, dict.display_meaning(e))` 쌍을 만든다 — daemon 이 산출하는 SoT(`AGENTS.md:64-73` 팝업 원칙)라 렌더러 3종 무수정.

---

## 3. 알고리즘

### 3.1 대상① — 최장 접미 일치 (`candidates.rs` 신규 `resolve_hanja_target`)

```text
resolve_hanja_target() -> Option<HanjaPlan { target, source, restore_text }>
  0. if content_purpose.should_block_hangul(): return None            // pull 경로(GetHanjaCandidates)도 여기서 차단
  1. if preedit_cache.is_empty():
        return resolve_selection_target()                              // §3.2 (없으면 None)
  2. pre  = preedit_cache;  last = pre.chars().last()
     if !is_syllable(last):                                           // 초성·미완성 자모(ㄱ, ㅏ…)
        return Some(Plan{ target: last, source: Preedit, restore: pre_last_char })   // 현행 → 특수문자 폴백 유지
  3. if is_word_mode():                                               // engine.rs:695 korean_context 누적 상태
        pool = pre                                                    // 전부 미커밋
        for k in (min(len(pool),18) ..= 2).rev():
            suf = pool[len-k..]
            if all_syllables(suf) && dict.contains_key(suf):
                return Some(Plan{ target: suf, source: WordPreedit{ nonmatch_prefix: pool[..len-k] }, restore: pool })
        return Some(Plan{ target: last, source: WordPreedit{ nonmatch_prefix: pool[..len-1] }, restore: pool })
        //  ↑ 현행 Word 모드 폴백은 select 시 korean_context.clear() 로 word_buffer 접두를 통째 잃는다
        //    (candidates.rs:150-156 + input_context.rs:391-397). 접두를 nonmatch_prefix 로 살리는 것은 버그 수정.
  4. // 음절 모드: pre 는 항상 1음절(input_context.rs:335-341 syllable 분기)
     // 4-0. surrounding 정합성 검증(안전망) — 앱이 보고한 커서 앞 텍스트와 버퍼가 어긋나면 버퍼를 버린다.
     //      GTK3/4 는 매 키 직전 retrieve-surrounding(gtk4/immodule.c:865) 으로, GNOME 은 vfunc_set_surrounding
     //      (unim_input_method.js:562-566) 으로, Wayland 는 F3b 로, Qt 는 F2a 재질의로, TSF 는 W1-a 로 갱신된다.
     //      surrounding 은 preedit 을 포함하지 않는다(GTK retrieve-surrounding / text-input-v3 / Qt ImSurroundingText 규약).
     (stext, scur, _) = surrounding_text()
     if !stext.is_empty():
         before = stext.chars().take(scur).collect()
         rc = recent_commit.as_str()
         consistent = if chars(before) >= chars(rc) { before.ends_with(rc) } else { rc.ends_with(before) }   // 앱이 앞부분을 잘라 보고해도 허용
         if !consistent: recent_commit.clear()           // → 아래 루프는 자연히 현행(마지막 음절)으로 폴백
     // surrounding 이 비어 있으면(XIM·미지원 앱) 검증 생략 — 버퍼를 그대로 신뢰
     pool = recent_commit.as_str() + last
     for k in (min(len(pool),18) ..= 2).rev():
         suf = pool[len-k..]
         if dict.contains_key(suf):                                    // pool 은 구성상 전부 완성 음절
             return Some(Plan{ target: suf, source: RecentAndPreedit{ prefix_len: k-1 }, restore: last })
     return Some(Plan{ target: last, source: Preedit, restore: last })  // 현행과 바이트 동일 → 회귀 0
```
- 비용: HashMap 조회 ≤ 17회.
- `is_syllable(c) = ('\u{AC00}'..='\u{D7A3}').contains(&c)` (`auto_typefix/dictionary.rs:15` 와 동일 범위; `src/hangul/char.rs` 에 공용 헬퍼 추가).
- chord(모아치기) 대기 자모는 `press_key.rs:229 finalize_chord_buffer()` 가 먼저 확정하므로 pool 에 반영된다.

#### 혼합 후보 리스트 (deviation 1 — 근거는 §11)
단어 일치 시 후보 = **단어 후보(정렬: 즐겨찾기 우선 → 사전 순)** 뒤에 **마지막 음절 후보(현행 목록, 즐겨찾기 우선)** 를 이어 붙인다. 각 후보는 `HanjaCandidate { entry: HanjaEntry, scope: Word | Syllable }` 로 scope 를 가진다.
- 이유: 사용자가 "국" 만 國 으로 바꾸고 싶은데 "대한민국" 이 사전에 있으면 v1 에 축소 키가 없어 막다른 길이 된다. 이어 붙이면 추가 키 0 으로 두 의도를 모두 만족(기현님 키 최소화).
- 표시: Syllable scope 행의 뜻 앞에 `국 ·` 처럼 음을 붙여 구분한다(예: `國  국 · 나라 국`). Word 행은 합성 뜻.
- 확정: Syllable scope 를 고르면 `source` 를 그 자리에서 `Preedit` 으로 강등(접두 교체 없음, delete 0). 즐겨찾기 키는 scope 의 hangul(단어 또는 음절).
- 정렬 키: `(scope_rank, !bookmarked)` — 즐겨찾기 된 음절 후보가 단어 구간 위로 올라오지 않는다.
- `toggle_hanja_bookmark`(`candidates.rs:186-263`) 의 "fresh 재조회 후 재정렬" 은 같은 조립 함수 `assemble_candidates(plan)` 를 재호출한다.
- 끄고 싶으면 `const HANJA_WORD_APPEND_SYLLABLE_TAIL: bool = true` 한 줄.

### 3.2 대상② — 선택 영역 판정 (`resolve_selection_target`)
```text
  (text, cur, anc) = surrounding_text()            // surrounding.rs:83-89, 문자 단위 오프셋
  if cur == anc: return None                        // 선택 없음 → 호출자가 현행 idle(이모지)
  raw = text[min..max]
  sel = raw.trim()                                  // 앞뒤 공백 trim(D6)
  ws_prefix = raw[..raw.len()-raw.trim_start().len()]; ws_suffix = raw[raw.trim_end().len()..]   // 위젯이 선택 전체를 치환하므로 확정 문자열에 되붙인다(§3.4)
  if sel.is_empty() || chars(sel) > 18 || !all_syllables(sel) || !dict.contains_key(sel):
        pending_ui_feedback = Some(UiFeedback::HanjaNoMatch); return NoMatch   // 팝업 없음, 이모지도 없음
  return Some(Plan{ target: sel, source: Selection{ ws_prefix, ws_suffix }, restore: "" })
```
- 반환을 `enum TargetResolve { Plan(..), NoMatchSelection, None }` 로 두어 호출자가 "선택은 있는데 불일치"(무동작)와 "선택 없음"(이모지)을 구분한다.
- **선택이 있으면 어떤 경우에도 이모지 팝업을 띄우지 않는 이유**(D6): 이모지 확정은 `commit_buffer` 로 나가는 일반 커밋이라(`popup_dispatch.rs:214`) 위젯이 선택 영역을 그 이모지로 **치환**한다 — 영문 단어를 선택한 채 무심코 F9→숫자를 누르면 선택 텍스트가 사라진다. 불일치 무동작은 이 파괴를 막는 안전장치이지 기능 제한이 아니다(매뉴얼에 한 줄 명시).
- 조합 중(preedit 있음)이면 선택은 무시한다 — 조합이 우선(현행 유지, 모호성 제거).
- 비밀번호 필드: `set_surrounding_text` 가 fail-closed 로 비워 두므로(`surrounding.rs:70-80`) 자동 무력화 + 0단계 게이트 이중.

### 3.3 한자키 dispatch (`press_key.rs:226-238` 치환 → `candidates.rs` 신규 `on_hanja_key`)
```rust
// press_key.rs:226-238 은 아래 3줄로 바뀐다 (C3 소유). 비밀번호 강제 영문 전환(:212-217)은 그 앞에 그대로.
if self.hanja_keys.contains(&keycode) {
    self.finalize_chord_buffer();
    return self.on_hanja_key();
}
// candidates.rs (C4 소유)
pub(super) fn on_hanja_key(&mut self) -> InputResult {
    let idle = self.preedit_cache.is_empty() && !self.korean_context.is_composing();
    if idle {
        return match self.resolve_selection_target() {
            TargetResolve::Plan(p) => self.open_hanja_popup(p),
            TargetResolve::NoMatchSelection => InputResult::consumed(),   // D6: 무동작(+선택 비프)
            TargetResolve::None => { self.start_emoji_popup(); InputResult::consumed() }  // v3.2 현행
        };
    }
    self.start_hanja_conversion()
}
/// pull 경로(GetHanjaCandidates, engine_worker.rs:1990)와 push 경로 공용 진입점 — 시그니처 불변.
pub fn start_hanja_conversion(&mut self) -> InputResult {
    if self.hanja_mode { return InputResult::consumed(); }
    match self.resolve_hanja_target() {
        TargetResolve::Plan(p) => self.open_hanja_popup(p),
        _ => { /* 특수문자 초성 폴백은 open_hanja_popup 내부(현행 candidates.rs:74-104 이동) */ InputResult::consumed() }
    }
}
```
`open_hanja_popup(plan)`: `assemble_candidates` → 없으면 초성 특수문자 폴백(현행 코드 그대로) → `hanja_target=plan.target`, `hanja_source`, `hanja_restore_text`, `hanja_candidates`, `hanja_mode=true`, `PopupState::new_hanja_with_top_row(&target, pairs, top_row)` + bookmark flags, `PopupAction::ShowHanja{target,candidates,top_row}`, `InputResult::hanja_candidates()`.

### 3.4 확정 (`select_hanja`, `candidates.rs:140-160` 개정)
```rust
pub fn select_hanja(&mut self, index: usize) -> Option<String> {
    if !self.hanja_mode || index >= self.hanja_candidates.len() { return None; }
    let c = &self.hanja_candidates[index];
    let text = self.hanja_output_format.format(&c.entry.hangul, &c.entry.hanja);
    let out = match (&self.hanja_source, c.scope) {
        (_, Scope::Syllable) | (HanjaTargetSource::Preedit, _) => { self.remove_preedit(); text }
        (HanjaTargetSource::WordPreedit{nonmatch_prefix}, _)   => { let p = nonmatch_prefix.clone(); self.remove_preedit(); p + &text }
        (HanjaTargetSource::Selection{ws_prefix, ws_suffix}, _) => format!("{ws_prefix}{text}{ws_suffix}"),   // delete 0, 위젯이 선택 전체를 치환 → trim 한 공백을 되붙인다
        (HanjaTargetSource::RecentAndPreedit{prefix_len}, _)   => {
            self.remove_preedit();
            self.pending_hanja_replacement = Some(HanjaReplacement{ delete_chars: *prefix_len, commit_text: text.clone() });
            text
        }
    };
    self.cancel_hanja();            // recent_commit.clear() 포함
    Some(out)
}
```
- `remove_preedit()`(`engine.rs:741-744`) = 현행 `korean_context.clear()+preedit_cache.clear()`.
- **RPC 호출자 규약**: `Some(text)` 를 받았을 때 `take_pending_hanja_replacement()` 가 `Some` 이면 text 를 **직접 커밋하지 말고** 교체 채널로 보낸다(§5.1). `None` 이면 현행대로 `CommitText`.
- `popup_select`(`popup_dispatch.rs:190-197`) 개정:
  ```rust
  if let Some(text) = self.select_hanja(abs_index) {
      let replaced = self.pending_hanja_replacement.is_some();
      if !replaced { self.commit_buffer.push_str(&text); }
      self.popup_pending_action = Some(PopupAction::HidePopup);
      return if replaced { InputResult::preedit_updated() } else { InputResult::committed() };
  }
  ```
  `preedit_updated()` = consumed·preedit_changed·commit 없음 → 프런트가 preedit "국" 을 먼저 지우고, 이어 도착하는 `AutoTypefixApply` 가 접두 삭제+커밋.

### 3.5 취소·포커스아웃 (D8)
| 경로 | 현행 | 개정 |
|---|---|---|
| `popup_dispatch.rs:225-231 popup_cancel` (키보드 Esc·TSF 마우스 OutsideCancel `key_handler.rs:1209`) | `commit_buffer.push_str(&hanja_target)` | `push_str(&hanja_restore_text)` (Selection 이면 빈 문자열 → 아무것도 안 함) |
| `engine_worker.rs:2030-2045 CancelHanja` → `service.rs:3147-3168 redirect_commit_and_hide` | `get_hanja_target()` | `get_hanja_restore_text()` |
| `engine_worker.rs:737-742 reset_engine_and_capture_commit` | `get_hanja_target()` | `get_hanja_restore_text()` |
| TSF `text_service.rs:1868` bare `engine.reset()` | 캡처 없음(기존 버그) | §5.6 래퍼 |
| 표시/조회 전용(헤더 `view_model.rs:305-320`, `is_bookmarked` 키 `candidates.rs:171,193,225`, `HanjaCandidatesReordered.target`) | `hanja_target` | 그대로(전체 어절) |

---

## 4. 상태기계 · 경로표

상태: `Idle` → (한자키) → `HanjaPopup{source}` → (확정/취소/FocusOut/Reset) → `Idle`.

| 경로 | 진입 | 버퍼 `recent_commit` | target/source | 앱 커밋·교체 | 비고 |
|---|---|---|---|---|---|
| **push 한자키**(GTK·Wayland·GNOME·TSF: `press_key.rs:226` → `on_hanja_key`) 조합 중, 음절 모드, 단어 일치 | `open_hanja_popup` | 유지(참조만) | `RecentAndPreedit{prefix_len}` | 없음(팝업만). preedit "국" 유지 | `InputResult::hanja_candidates()` |
| 동일, 단어 불일치 | 〃 | 유지 | `Preedit` | 없음 | 현행과 동일 |
| 동일, Word 모드 | 〃 | 무관 | `WordPreedit{nonmatch_prefix}` | 없음 | preedit_cache 전체 표시 유지 |
| push 한자키, idle, 선택 있음·일치 | 〃 | 무관 | `Selection` | 없음 | v3.2 idle 게이트 예외(§8) |
| push 한자키, idle, 선택 있음·불일치 | 무동작 | 무관 | — | 없음 | `consumed` + (선택) 비프. 이모지 안 뜸 |
| push 한자키, idle, 선택 없음 | 이모지 팝업 | 무관 | — | 없음 | 현행 v3.2 |
| **pull 한자키**(Qt/XIM `GetHanjaCandidates` → `engine_worker.rs:1990 start_hanja_conversion`) | `resolve_hanja_target` (idle 이면 selection 만) | 유지 | 위와 동일 | 없음 | 비밀번호: 0단계 게이트가 차단(신규) — 종전엔 없었음(selection-surrounding 보충#5) |
| **키보드 확정**(숫자/Enter → `popup_select`) RecentAndPreedit | — | `clear()` | — | 응답 preedit="" + `AutoTypefixApply(prefix_len, 서식문자열, "")` | commit_buffer 비움 |
| 키보드 확정 Preedit / WordPreedit / Selection | — | `clear()` | — | 응답 commit = 서식문자열(Word: 접두+서식) | 현행 채널 |
| **마우스 확정**(`SelectHanja` RPC → `engine_worker.rs:2013-2028` → `service.rs:2886-2920`) RecentAndPreedit | — | `clear()` | — | `redirect_auto_typefix_apply_and_hide(prefix_len, text)` (신규 헬퍼) | owner path 로 발행 |
| 마우스 확정 그 외 | — | `clear()` | — | 현행 `redirect_commit_and_hide(text)` | 위젯이 선택 치환(Selection) |
| **취소 3경로**(§3.5) | — | `clear()` | — | `hanja_restore_text` 재커밋(Selection 은 없음) | HidePopup |
| **FocusOut/Reset RPC**(`engine_worker.rs:1895-1936`) | — | 엔진 재생성 → 빈 버퍼 | — | 위와 동일 캡처 | Wayland 는 FocusOut 전에 CancelHanja 를 직접 호출(`wayland/state.rs:309-320`) |
| **content_purpose 차단**(`surrounding.rs:38-44`) | — | `clear()` | 팝업 중이었다면 `cancel_hanja()` 도 호출(비번 필드에 원문 재커밋 금지 → restore 생략) | — | fail-closed |
| **즐겨찾기 토글**(Space/우클릭) | — | 유지 | 유지 | 없음 | `assemble_candidates` 재조립 + `HanjaCandidatesReordered` |
| **BS passthrough** | — | `pop_last()` | — | 앱이 1자 삭제 | 커밋 후 preedit 없는 상태의 BS |
| **Enter/Tab/Esc/Space/커서키/비소비키** | — | `clear()` | — | 현행 | 수정자 단독 키 제외 |

---

## 5. 교체 실행

### 5.1 Linux 데몬 (`unim-dbus`)

**키보드 경로** `engine_worker.rs` ProcessKeyEvent 핸들러:
1. `:1331 let commit = drain_commit(engine); :1334 let popup_action = engine.take_popup_action();` 직후에
   `let hanja_replacement = engine.take_pending_hanja_replacement();` 추가.
2. ATF 블록(`:1338-1343`)은 `popup_action.is_none()` 조건이 있어 HidePopup 이 실린 이 프레임엔 진입하지 않는다 → 상호배제 구조적 보장. 방어적으로 `&& hanja_replacement.is_none()` 도 추가.
3. `(final_preedit, final_commit)` 계산(`:1700-1725`) 뒤:
   ```rust
   let (final_preedit, final_commit, auto_typefix_result) = if let Some(r) = hanja_replacement {
       (Some(String::new()), None, Some((r.delete_chars, r.commit_text, String::new())))
   } else { (final_preedit, final_commit, auto_typefix_result) };
   ```
   → `EngineResponse.auto_typefix`(`service.rs:267`) 에 실려 `service.rs:2155-2166` 이 `AutoTypefixApply(delete, commit, "")` 를 자기 path 로 발행. **신규 시그널·신규 PopupAction 없음.**
4. `keystroke_buffers.remove(&context_id)` 는 popup_action 이 Some 이라 이미 실행됨(`:1699`).

**마우스 경로**:
- `EngineRequest::SelectHanja` 응답 타입 `oneshot::Sender<Option<String>>`(`service.rs:67-71`) → `Option<(String, u32)>` (text, delete_chars). `engine_worker.rs:2013-2028`: `engine.select_hanja(index).map(|t| (t, engine.take_pending_hanja_replacement().map_or(0, |r| r.delete_chars)))`.
- `service.rs:2886-2920 select_hanja`: `if delete_chars > 0 { self.redirect_auto_typefix_apply_and_hide(delete_chars, &text).await } else { self.redirect_commit_and_hide(&text).await }`. RPC 반환 `s` 는 그대로 text.
- 신규 헬퍼 `redirect_auto_typefix_apply_and_hide(&self, delete_chars, text)` — `redirect_commit_and_hide`(`service.rs:1807-1862`) 를 복제해 `emit_signal(None, &owner_path, "org.atit.unim.InputContext", "AutoTypefixApply", &(delete_chars, text, ""))` 후 `HidePopup`. owner path 구독 검증은 atf-replacement 보충#1(2) — GNOME `isOwn` 필터도 통과.
- `CancelHanja` 응답은 `get_hanja_restore_text()` (§3.5).

**설정 핫리로드**: 리로드 루프 `engine_worker.rs:982-988` 의 `set_atf_hotkeys`/`set_switch_keys` 옆에 `engine.set_hanja_output_format(&config);` (비파괴, fingerprint `:361-397` 에 넣지 않음 — config-sync 보충#12(1)).

**비프(선택)**: `engine.take_ui_feedback()` 를 ProcessKeyEvent 응답 직후 드레인, `config.engine.toggle_announce_beep` 가 true 일 때만 `crate::beep::announce_hanja_nomatch()`(`unim-dbus/src/beep.rs:175-181` 옆 신설, 짧은 단음). 접근성 옵션 게이트라 기본 무음.

### 5.2 프런트엔드 매트릭스

| 프런트 | 대상① | 대상② | 필요한 수정(file:line) | 근거 |
|---|---|---|---|---|
| **GTK4** `unim-frontends/gtk4/src/immodule.c` | ✅ `on_auto_typefix`(:463-533) 그대로: `delete_surrounding` → XTest BS → `\b` 3단 폴백, 끝에 `unim_emit_preedit(unim,"")`(:529-533) 로 preedit 잔상 제거 | ✅ 선택 = `set_surrounding_with_selection`(:1248-1275) 매 키 직전 전송 | **(F1) 선택 삭제 래퍼 게이트** `:1058-1084`: `if (result.consumed)` → `if (result.consumed && ((result.commit && result.commit[0]) \|\| (result.preedit && result.preedit[0])))`. 이유: F9(팝업 열림)·팝업 내 화살표 등 "아무것도 넣지 않는 소비 키"에서 선택 영역이 지워져 취소 시 원문이 사라진다. 게이트 후 확정 키(commit 있음)에서만 삭제→커밋(래퍼가 치환 수행). 마우스 확정은 `on_commit_text`(:451-459) → `commit` 시그널 → **GtkText 가 자체 치환**: gtktext.c `gtk_text_enter_text` "if (priv->selection_bound != priv->current_pos) gtk_text_delete_selection (self);" (GTK main, 이번 설계에서 원문 확인) | 회귀 검토: 게이트가 막는 경우는 commit·preedit 둘 다 빈 소비 키뿐(한/영 토글·팝업 내비·BS 로 마지막 자모 삭제) — 어느 것도 "선택 치환"이 의도된 동작이 아님 |
| **GTK3** `gtk3/src/immodule.c` | ✅ `on_auto_typefix`(:398) GTK4 동형 | ❌ anchor 없음(:1220-1222) → 엔진은 cur==anc → 현행 idle | 없음 | 선택 영역 삭제 래퍼는 死코드(selection-surrounding §GTK3) |
| **Qt5/6** `qt5/src/input_context.cpp`(qt6 동일 라인대 `:386,450,556`) | ✅ ATF 콜백(:177-220) — Konsole `\b` 우회 포함, `QInputMethodEvent` 에 빈 preedit 실려 preedit 잔상도 제거 | ✅ 단 stale(:554-564 포커스 1회) | **(F2a)** F9 분기(:380-386) 진입 직후 `QInputMethodQueryEvent(ImSurroundingText\|ImCursorPosition\|ImAnchorPosition)` 재질의 → `m_dbus->setSurroundingText(text, cur, anc)` **빈 텍스트여도 전송**(stale 제거) → 그 다음 `getHanjaCandidates`. 오프셋은 UTF-16 code unit 이므로 `text.left(pos).toUcs4().size()` 로 문자 단위 변환. **(F2b)** 선택 삭제 래퍼(:446-463) 게이트를 GTK4 와 동일하게(`result.consumed && (!commit.isEmpty() \|\| !preedit.isEmpty())`). 마우스 확정 `setCommitTextCallback`(:222-229) → `QLineEdit` 계열은 `QWidgetLineControl::processInputMethodEvent` "if (isGettingInput) … removeSelectedText();" (qtbase 6.8, 원문 확인) | 불일치 시 흐름: getHanjaCandidates 빈 응답 → special 없음 → `processKey(Hanja)`(:399-412) → 엔진 `on_hanja_key` idle+선택 불일치 → consumed, 이모지 없음(D6 충족) |
| **XIM** `unim-frontends/xim/src/handler.rs` | ✅ N+1 자가 BS(:540-576, :1052-1107) | ❌ | **(F4)** AutoTypefixApply 도착 처리(:520-540) 첫머리에서 **현재 preedit 표시 중이면 `self.preedit(server, user_ic, "")` 로 먼저 비움**(마우스 확정 경로는 ProcessKey 응답이 없어 "국" 잔상 방지; 키보드/ATF 경로는 이미 비어 no-op). **Reset 부작용(:1104-1107) 은 무해 — 증명**: `DbusRequest::Reset`(`xim/dbus_client.rs:388-400`) → `EngineRequest::Reset` → `reset_engine_and_capture_commit`(`engine_worker.rs:722-767`): 이 시점 hanja_mode=false·preedit 없음이므로 캡처 커밋 None, 엔진 재생성(빈 recent_commit = D3 "팝업 확정 후 리셋" 과 동일 결과), `preserve_mode=true` 로 한/영 유지, `apply_word_gate` 재적용. 유일한 비용은 `InputEngine::new` 의 사전 재파싱(기존 XIM 역방향 ATF 도 동일하게 치르는 비용). 회피하려면 commit_text 에 CJK 통합한자(U+4E00–U+9FFF)가 있으면 Reset 을 생략 — v1 은 무해하므로 **생략하지 않는다** | 비밀번호 감지 부재는 기존 구조적 공백(문서 명시) |
| **Wayland** `unim-frontends/wayland/src/state.rs` | ✅ `apply_auto_typefix`(:249-297) + **(F3a) 바이트 수 수정** | ✅ **(F3b) SurroundingText 배선 신설** | **(F3a)** `:255-268` `is_forward` 판정 **앞에** `let is_hanja = commit_text.chars().any(\|c\| ('\u{4E00}'..='\u{9FFF}').contains(&c) \|\| ('\u{3400}'..='\u{4DBF}').contains(&c) \|\| ('\u{F900}'..='\u{FAFF}').contains(&c)); let before_bytes = if is_hanja { delete_chars * 3 } else { /* 기존 휴리스틱 그대로 */ };` — 대상①의 삭제 대상은 구성상 **항상 완성 음절(U+AC00–D7A3 = UTF-8 3B)** 이므로 추정이 아니라 확정값. ATF 의 commit_text 는 한글/ASCII 뿐이라 한자 판별자와 서로소 → **ATF 바이트 동일**. '한자(漢字)' 형식도 한자를 포함하므로 오분류 없음(atf-replacement 보충#6 의 1/3 삭제 문제 해소). **(F3b)** `Event::SurroundingText{text,cursor,anchor}`(:626-628 드롭) → `state.pending_surrounding = Some((text,cursor,anchor))`; `Event::Done`(:563) 에서 pending 을 꺼내 바이트→문자 오프셋 변환(`text[..floor_char_boundary(cursor)].chars().count()`) 후 신규 `DbusRequest::SetSurroundingText{context_path,text,cursor,anchor}`(`wayland/src/dbus_client.rs` enum :11-49 옆 + 핸들러 `proxy.set_surrounding_text`) 전송. deactivate(:307) 시 `SetSurroundingText("",0,0)` 로 stale 제거. 마우스 확정 `PopupEvent::CommitText`(`main.rs`) → `commit_string` → 앱 위젯이 선택 치환(툴킷 표준) | 프로토콜: `zwp_input_method_v2.surrounding_text` 는 바이트 오프셋(atf-replacement 보충#6(2) xml :124-127). 앱이 미지원이면 이벤트가 안 와 cur==anc → 현행 이모지 |
| **GNOME 확장** `unim-gnome-extension/*.js` | ✅ `_onAutoTypeFix`(`extension.js:283-300`) vkbd BS ×N → 50ms → commitText | ✅ `vfunc_set_surrounding`(`unim_input_method.js:562-566`) anchor 전달 | **(F5a)** `extension.js:283` 진입 시 현재 preedit 이 비어 있지 않으면 `this._inputMethod.updatePreedit('')` 를 BS 주입 전에 호출(마우스 확정 잔상 방지, ATF 경로는 no-op). **(F5b)** `popup_view.js:98 this._header` 에 `this._header.clutter_text.set_ellipsize(Pango.EllipsizeMode.END)` (뜻 라벨 :373 과 동일). 마우스 확정은 `commitText` → Mutter → 앱(text-input-v3) → 위젯 치환 | Mutter 의 `vfunc_set_surrounding` 호출 빈도는 리포 밖(실측 항목) |
| **TSF** | ✅ §5.3 | ✅ §5.3 | §5.3 | — |

### 5.3 Windows TSF (`unim-tsf`)

코어 로직은 직접 링크로 자동 상속(windows-parity §6). Windows 전용 작업은 4가지.

**(W1-a) 대상② 선택 읽기** — `key_handler.rs:389 handle_key_down` 에서 `:557 engine.press_key` **직전**:
```rust
if engine.is_hanja_key(keycode) && !engine.is_composing() && !popup_active && atf_active /* 비번 아님, :417 관례 */ {
    match composition::read_selection_text(context, tid) {            // composition.rs:1637-1656
        Some(sel) if sel.cursor != sel.anchor => engine.set_surrounding_text(sel.surrounding_text, sel.cursor, sel.anchor),
        _ => engine.set_surrounding_text(String::new(), 0, 0),        // stale 제거
    }
}
```
`is_hanja_key` 는 엔진 신규 접근자(`engine.rs`, C3) — `test_key_down:184` 의 하드코딩(Hanja||F9)과 달리 config 기반.
조합 중(`engine.is_composing()`)에도 같은 읽기를 수행해 `set_surrounding_text` 를 채우면 §3.1 4-0 정합성 검증이 TSF 에서도 동작한다 — 단 `read_selection_text`(`composition.rs:1567-1631`)는 `sel_range.IsEmpty`(`:1584`)면 `None` 을 돌려주므로, **선택이 비어도 `Some{cursor==anchor, surrounding_text}` 를 돌려주도록 `:1584` 분기를 완화**한다(기존 호출부 `:426-431` 은 `cursor != anchor` 가드가 있어 바이트 동일). 선택(ReadOnly EditSession) 비용은 한자키 1회에 한정.

**(W1-b) 키보드 확정 교체** — `:587 drain_popup_actions` 와 `:596 commit/preedit 처리` 사이에 조기 블록:
```rust
if let Some(rep) = engine.take_pending_hanja_replacement() {
    let mut span = rep.delete_chars;
    if comp_mgr.is_active() {                       // 조합 중 음절("국")을 텍스트로 materialize 후 span 에 포함 —
        comp_mgr.end_composition_keep_text(context, tid);   // key_handler.rs:861-866 역방향 ATF 와 동일 레시피(CUAS clear 무효 회피)
        span += 1;
    }
    if composition_unsupported { /* 오버레이 preedit_win 을 비우는 기존 호출 재사용(폴백 경로 :615 이하와 동일) */ }
    let outcome = comp_mgr.replace_surrounding(context, tid, span, &rep.commit_text, "", comp_sink);   // composition.rs:663
    match outcome {                                                                                     // :446-454 와 바이트 동일 4갈래
        ReplaceOutcome::Normal => {}
        ReplaceOutcome::PhaseSplit => schedule_flush = true,
        ReplaceOutcome::SynthBatch => engine.remove_preedit(),
        ReplaceOutcome::SynthHeadTail => { let _ = crate::synth_input::discard_pending_tail(); engine.remove_preedit(); schedule_flush = true; }
    }
    return KeyDownOutcome { eaten: true, schedule_flush, ..Default::default() };
}
```
대상②·Word·단음절은 현행 경로(`:673-750`)로 흐른다: idle 확정은 `comp_mgr.insert_text`(:734) = `InsertTextAtSelection` 계열이라 선택을 치환한다(TSF 표준; VM 검증 항목, 실패 시 대안: typefix 템플릿 `:424-457` 처럼 `replace_surrounding(delete=선택 길이)`).

**(W1-c) 마우스 확정 교체** — `apply_reverse_event`(`:1130-1249`): `:1218 drain_popup_actions` 직후, `:1221 commit_str` 처리 전에 W1-b 와 동일 블록(context 가 `Some(ctx)` 일 때만; `None` 이면 `:1242-1248` 과 같은 "dropped" 로그). **반환형 `()` → `bool`(schedule_flush)** 로 넓힌다.
**(W2-a)** `text_service.rs:2436` 호출부: 반환값을 모으고, 루프가 끝나 락이 풀린 뒤 `:1451-1495` 키 경로와 같은 SetTimer(`WM_UNIM_FLUSH2`) 예약을 수행 — 그 블록을 `fn schedule_phase2_flush(ctx)` 로 추출해 양쪽에서 호출.

**(W2-b) OnSetFocus 캡처 래퍼** — `text_service.rs:1868 engine.reset()` 직전:
```rust
if engine.popup_state().is_some() {
    // 1) 팝업 창 정리: reset() 이 popup_pending_action 을 None 으로 만들어 HidePopup 이 영영 안 나가므로 여기서 직접 hide.
    popup.send_hide(...);
    // 2) 원문 보존: 한자/특수 모드였다면 hanja_restore_text 를 옛 컨텍스트에 materialize.
    //    TSF 는 포커스 이탈 시 조합을 terminate 하며 텍스트를 문서에 남기는 것이 기본이지만(OnCompositionTerminated),
    //    앱별 편차에 대비해 옛 context(ctx.last_context) 가 살아 있으면 end_composition_keep_text 를 먼저 호출.
    if let Some(old) = last_context.as_ref() { if comp_mgr.is_active() { comp_mgr.end_composition_keep_text(old, tid); } }
    engine.cancel_hanja(); engine.cancel_special_char();
}
engine.reset();
```
(`:1843-1849` 스퓨리어스 포커스 스킵 분기는 그대로 — 그 분기는 reset 자체를 안 한다.)

**(S5) 레거시 Win32 설정 모달** `unim-tsf/src/settings_dialog.rs` — 아직 `fn_configure.rs:45`·`lang_bar.rs:799` 가 호출하므로 콤보 1개 추가: `:755-762` ModeSharing 콤보 패턴 + `:1280-1286` 저장 역변환 패턴, 한자 키 편집란(`:778`) 바로 아래. 라벨은 하드코딩 한국어(이 파일은 rust-i18n 미사용, `rg "t!\("` 무매치).

**(R1-b) 렌더러** `unim-popup-win/src/render.rs:504-522`: `let hanja_w = max(s(90,scale), text_width(hdc,&main,font_main) + s(8,scale)); hanja_rect.right = hanja_left + hanja_w; mean_left = hanja_left + hanja_w + s(6,scale);` — 4자 이상 한자에서 뜻 열과 겹치지 않게. 헤더 `:375` 플래그에 `DT_END_ELLIPSIS` 추가.

wire 프로토콜(`popup_ipc.rs`/`protocol.rs`)은 **무변경** — 셀 텍스트가 `String` 이라 다글자가 그대로 통과(popup-pipeline §2.7). 신규 필드 없음이므로 골든 라인 테스트 불변.

---

## 6. 설정

### 6.1 필드
`src/config.rs` `KoreanConfig`(`:604-670`) 의 `commit_unit`(`:664`) 바로 뒤:
```rust
/// 한자 팝업 확정 시 넣는 문자열 서식 — 漢字 / 한자(漢字) / 漢字(한자). 단음절·단어 변환 공통.
#[serde(default)]
pub hanja_output_format: HanjaOutputFormat,
```
`Default`(`:678-688`) 에 `hanja_output_format: HanjaOutputFormat::default()`. `KoreanConfigCompat`(`:790-869`) 은 레거시 이름 충돌이 없으므로 **무변경**(struct-level `#[serde(default)]` 가 구 YAML 흡수). 위치 근거: 한자 변환은 한글 조합의 산물이고 `commit_unit`(조합 동작류) 이 가장 가까운 전례; `hanja_keys` 는 "키 바인딩류"라 `EngineConfig` 최상위(config-sync §7-Q2 의 구분 기준 채택).

### 6.2 동기화 지점 (전수, config-sync §1·보충#12)
| # | 지점 | 파일:line | 작업 |
|---|---|---|---|
| 1 | 코어 | `src/config.rs:58-90` 옆 enum, `:664` 필드, `:685` Default | §2.3 |
| 2 | 엔진 캐시 | `engine.rs:245-246`(new), `:948-950`(rebuild_korean_context), 신규 `set_hanja_output_format(&Config)` (`set_atf_hotkeys :613` 옆) | 비파괴 setter |
| 3 | 데몬 리로드 | `engine_worker.rs:982-988` | setter 호출 추가; fingerprint(`:361-397`) 미포함 |
| 4 | DBus YAML/JSON | `service.rs:1161-1265` | **자동**(serde). 레거시 `get_config/set_config`(`:719-`) 는 `commit_unit` 전례대로 생략 |
| 5 | CLI | `unim-cli/src/main.rs:616-617` ConfigKey `#[value(name="hanja-output-format", help=h("help_ck_hanja_output_format"))] HanjaOutputFormat`; `:74-79` 옆 `hanja_output_format_display_name_localized`; `:1701-1723` 옆 `config set` 암(값: `hanja\|한자`, `hangul-hanja\|한글한자\|한자(漢字)`, `hanja-hangul\|한자한글\|漢字(한자)`); `:886-890` 옆 `config show` 한 줄. 대화형 메뉴(`:1806-1814`)는 `commit_unit` 전례대로 생략 |
| 6 | CLI 로케일 | `unim-cli/locales/{ko,en}.yml` — `commit_unit_*` 가 있는 줄(44-45, 88, 106, 120-122, 378) 바로 뒤에 같은 순서로: `hanja_output_format_label`, `hanja_output_format_note`, `error_invalid_hanja_output_format`, `hanja_output_format_changed`, `hanja_output_format_hanja`, `hanja_output_format_hangul_hanja`, `hanja_output_format_hanja_hangul`, `help_ck_hanja_output_format` | 두 파일 같은 줄 수 유지 |
| 7 | GTK 설정 | `unim-settings-gtk/src/settings_dialog.rs` — 한자 키 row(`:604-609`) 바로 뒤에 `adw::ComboRow`(`:691-723` commit_row 복제), `save_and_notify(&s.config, "hanja_output_format")` | 로케일 `unim-settings-gtk/locales/{ko,en}.yml`: `row_hanja_output_format`, `_subtitle`, `_tooltip`, `hanja_output_format_hanja/_hangul_hanja/_hanja_hangul` |
| 8 | GTK 병합 화이트리스트 | `unim-gui-common/src/settings_helpers.rs:170-207`, `commit_unit` 줄(:204 부근) 뒤 `merge_field(&mut d.korean.hanja_output_format, bk.map(\|k\| &k.hanja_output_format), &u.korean.hanja_output_format);` | 누락 시 GTK 저장이 disk 값에 덮임 |
| 9 | Slint 설정 | `unim-settings/ui/settings.slint:330-331` 옆 `hanja-output-format-options`/`-index` 프로퍼티, `:691-692` 콤보 마크업 복제(한자 키 필드 `:813` 아래); `unim-settings/src/main.rs:842-853` 옵션/인덱스 세팅, `:935-938` 저장 역변환, `merge_ui_owned :573` 옆 merge_field | GNOME 사용자는 prefs.js 가 이 앱으로 리다이렉트하므로 필수 |
| 10 | Windows 레거시 모달 | `unim-tsf/src/settings_dialog.rs:755-762`, `:1280-1286` | §5.3 (S5) |
| 11 | 문서 | 사용자 매뉴얼 §4.2(§9.5) | gen-help 가 HTML 자동 병합(config-sync §3.2-6) |
| — | `unim-capi` | 필드 전용 setter 없음(`commit_unit` 도 없음) | 무변경 |
| — | GNOME gschema | 일반 설정이므로 추가 금지 | 무변경 |

### 6.3 UI 문구 (ko / en)
| 키 | ko | en |
|---|---|---|
| `row_hanja_output_format` | 한자 출력 형식 | Hanja Output Format |
| `row_hanja_output_format_subtitle` | 漢字 · 한자(漢字) · 漢字(한자) — 한자 팝업에서 확정할 때 넣는 형식 | 漢字 · 한자(漢字) · 漢字(한자) — text inserted when you confirm a hanja candidate |
| `row_hanja_output_format_tooltip` | 「한자만」은 漢字만 넣습니다. 「한글(한자)」는 대한민국(大韓民國)처럼 한글 뒤에 괄호로 한자를, 「한자(한글)」는 大韓民國(대한민국)처럼 한자 뒤에 괄호로 한글을 붙입니다. 한 글자 변환과 단어 변환 모두 같은 형식을 씁니다. | "Hanja only" inserts 漢字 alone. "Hangul(Hanja)" inserts 대한민국(大韓民國); "Hanja(Hangul)" inserts 大韓民國(대한민국). Applies to both single-syllable and word conversion. |
| `hanja_output_format_hanja` | 한자만 (漢字) | Hanja only (漢字) |
| `hanja_output_format_hangul_hanja` | 한글(한자) — 한자(漢字) | Hangul(Hanja) — 한자(漢字) |
| `hanja_output_format_hanja_hangul` | 한자(한글) — 漢字(한자) | Hanja(Hangul) — 漢字(한자) |
| `row_hanja_keys_subtitle` (개정) | 쉼표로 구분 (예: Hanja, F9) — 조합 중이면 어절/음절 한자 변환, 한글 단어를 선택한 뒤 누르면 그 단어 변환, 그 외에는 이모지 팝업 | Comma separated (e.g. Hanja, F9) — converts the word/syllable being typed, converts a selected Hangul word, otherwise opens the emoji popup |
| `row_hanja_keys_tooltip` (개정) | 한자/특수문자 팝업을 띄울 키. 예: Hanja, F9. 한글 조합 중: 방금 친 어절(예: 대한민국)이 한자 사전에 있으면 단어 후보, 없으면 마지막 음절 후보. 앱에서 한글 단어를 선택한 채 누르면 그 단어의 한자 후보(GTK4·Qt·GNOME·Wayland·Windows). 조합·선택이 모두 없으면 이모지 팝업. | Keys that open the Hanja/special-char popup, e.g. Hanja, F9. While composing: word candidates for the word you just typed (e.g. 대한민국) if it is in the dictionary, otherwise last-syllable candidates. With a Hangul word selected in the app: candidates for that word (GTK4, Qt, GNOME, Wayland, Windows). Idle with no selection: emoji popup. |
| CLI `help_ck_hanja_output_format` | 한자 출력 형식 (hanja, hangul-hanja, hanja-hangul). 漢字 / 한자(漢字) / 漢字(한자) | Hanja output format (hanja, hangul-hanja, hanja-hangul): 漢字 / 한자(漢字) / 漢字(한자) |
| CLI `hanja_output_format_note` | (단음절·단어 변환 공통) | (applies to syllable and word conversion) |
| CLI `error_invalid_hanja_output_format` | 잘못된 한자 출력 형식: %{value}. 가능한 값: %{allowed} | Invalid hanja output format: %{value}. Allowed: %{allowed} |
| CLI `hanja_output_format_changed` | 한자 출력 형식을 '%{fmt}'(으)로 변경했습니다. | Hanja output format set to '%{fmt}'. |

### 6.4 핫리로드 동작
- Linux: 리로드 루프가 매 회 `set_hanja_output_format` 호출 → 다음 확정부터 새 서식, 조합 끊김 없음.
- Windows: `maybe_reload_config`(`text_service.rs:424-463`) 가 엔진을 재생성 → `new()` 캐시로 반영.

---

## 7. 팝업·렌더러

### 7.1 헤더 축약 (daemon SoT, `src/popup/view_model.rs`)
- compact 헤더(`:346-`) `「{target}」 → 한자`, expanded 헤더(`:305-320`) `「{target}」 → {hanja}  {meaning}` 의 `self.target()` 자리에 `display_target(target, max)` 적용: `max` = compact 12 / expanded 8, 초과 시 `앞4 + "…" + 뒤4`(예: 「청룡기쟁…권대회」). 상수 `HEADER_TARGET_MAX_COMPACT/EXPANDED`.
- 이유: 8~18자 target 2,124건(0.77%)에서 420px 팝업 폭(`popup_styles.generated.css:14`)을 넘고, 세 렌더러 어디에도 헤더 ellipsize 가 없다(popup-pipeline 보충#7(2)). daemon 축약이 3플랫폼 공통·결정적이므로 1차, 렌더러 ellipsize 는 2차 안전망.

### 7.2 렌더러 (2차 안전망 + Windows 폭)
| 렌더러 | file:line | 수정 |
|---|---|---|
| GTK4 popup-service | `unim-popup-service/src/popup/hanja.rs:102-107 target_label` | `target_label.set_ellipsize(gtk4::pango::EllipsizeMode::End)` (뜻 라벨 `:348` 과 동일) |
| GNOME | `unim-gnome-extension/popup_view.js:98` | `set_ellipsize(Pango.EllipsizeMode.END)` |
| Windows | `unim-popup-win/src/render.rs:375` | `DT_END_ELLIPSIS` 추가 |
| Windows compact 열 | `render.rs:504-522` | §5.3 (R1-b) 가변 폭 |
| compact 뜻 열 3종 | 빈 뜻 처리는 이미 우아함(보충#7(1)) | 합성 뜻(§2.5)이 들어가면 그대로 표시 |
| expanded(9×9) | 셀 `min-width:30px`/`CELL_W=44` | **무변경** — 단어 후보에서도 사용자가 `.` 로 확장할 수 있으나 셀이 들쭉날쭉해질 뿐 기능 손상 없음. v2 에서 단어 후보는 compact 강제 검토 |

### 7.3 UX 세부
- **키 시퀀스**: 단어 변환 = `F9` → `1~9`(2키). 선택 변환 = 마우스 드래그 → `F9` → `1~9`. 신규 키 없음.
- **인지**: 헤더 「대한민국」 → 한자 가 "무엇이 바뀌는지"를 보여준다. 대상①에서 "대한민" 은 이미 앱에 있으므로 팝업 열림 시 앱 화면은 그대로(preedit "국" 밑줄 유지) → 확정 순간 `대한민국` 이 `大韓民國` 으로 한 번에 바뀐다. 중간 상태 노출 없음(GTK4 `delete_surrounding`+commit 은 동일 메인루프 틱 안에서 처리).
- **실패 피드백**: 선택은 했는데 사전에 없으면 팝업이 안 뜬다 + (접근성 비프 옵션 ON 시) 짧은 비프. 로그 `ENGINE "선택 단어 한자 불일치: '...'"`.
- **취소**: Esc → 대상①은 "국" 이 preedit 대신 확정 텍스트로 남고(현행 동일), 대상②는 선택이 그대로 남는다(GTK4/Qt 래퍼 게이트 덕분).
- **즐겨찾기**: 단어도 Space/우클릭 동일. 단어 키 재사용 빈도가 낮다는 트레이드오프(hanja-core §4.3)는 문서에 명시.

---

## 8. POPUP_SPEC 개정안 초안 (승인 대기 — `docs/dev/specs/HANJA_WORD_SPEC.md` 의 "POPUP_SPEC 개정안(승인 대기)" 절에 그대로 실을 문구. POPUP_SPEC.md 자체는 수정하지 않는다)

**§3.7 규칙 2 (`POPUP_SPEC.md:237`) 개정문**
> 2. **대상**: 다음 순서로 결정한다.
>    (a) 조합 중이고 마지막 글자가 완성 음절이면, **최근 확정 음절(최대 17자) + 조합 중 음절**의 접미 가운데 한자 사전에 있는 **가장 긴** 문자열(예: "대한민"+"국" → "대한민국"). 단어 모드(`commit_unit=Word`)에서는 preedit 전체의 최장 접미. 없으면 (b).
>    (b) 조합 중이면 preedit 의 마지막 음절(예: "국") — 초성·미완성 자모 포함(종전 규칙).
>    (c) 조합이 없고 앱 선택 영역이 한글 완성 음절 ≤18자로 이루어져 사전에 정확히 있으면 그 선택 문자열.
>    (a) 일치 시 후보 목록은 단어 후보 뒤에 (b) 의 음절 후보를 이어 붙인다(음절 후보 행은 뜻 앞에 음을 표기).

**§3.7 규칙 4 개정문**
> 4. **선택 시**: `SelectHanja(globalIndex)` → 엔진이 **출력 형식 설정이 적용된 문자열**(漢字 / 한자(漢字) / 漢字(한자)) 반환. 대상이 (a) 이고 이미 확정된 접두가 있으면 엔진은 `CommitText` 대신 `AutoTypefixApply(delete_chars=접두 글자 수, commit_text=문자열, preedit_text="")` 를 popup-owner path 로 발행하고 프런트엔드는 AutoTypeFix 와 동일한 삭제→커밋을 수행한다. (c) 는 `delete_chars=0` 커밋이며 위젯이 선택 영역을 치환한다.

**§3.7 규칙 5 개정문**
> 5. **취소 시**: `CancelHanja()` → 엔진이 **복원 원문**(대상 (a): 조합 중이던 마지막 음절, (b): 원래 한글/초성, (c): 없음)을 반환 → 프런트엔드가 그대로 커밋(없으면 커밋 없음) → 팝업 닫기. 이미 확정돼 앱에 있는 접두는 건드리지 않는다.

**§3.7 신설 규칙 10**
> 10. **선택 영역 변환 지원 범위**: 선택 영역(anchor≠cursor)을 `SetSurroundingText` 로 전달하는 프런트엔드(GTK4·Qt5/6·GNOME Shell 확장·Wayland·Windows TSF)에서만 동작한다. GTK3·XIM 은 선택 정보를 전달하지 못하므로 종전 idle 동작(이모지 팝업)을 유지한다. 선택이 있으나 사전에 없으면 어떤 팝업도 띄우지 않는다.

**§9.2 v3.2 idle 정책 (`POPUP_SPEC.md:606-609`) 개정문**
> **idle Hanja 키 dispatch 정책 (v3.4)**: Hanja 키는 `input_category` 와 무관하게 `press_key()` 의 언어 분기 직전에 처리. 조합 중이면 한자 변환(§3.7 규칙 2(a)(b)). preedit/조합 idle 이면 (i) 앱 선택 영역이 §3.7 규칙 2(c) 를 만족할 때 한자 변환, (ii) 선택은 있으나 불일치면 아무 팝업도 띄우지 않음, (iii) 선택이 없으면 emoji popup 트리거(종전 v3.2 동작).

**§11 변경 이력 신설 행**
> | (승인일) | **v3.4** | **한자 단어 변환 — §3.7 규칙 2 대상 확장(최근 확정 음절+조합 최장 접미, 선택 영역), 규칙 4/5 확정·취소 페이로드 개정, 규칙 10 선택 변환 지원 범위, §9.2 idle 정책 예외, `AutoTypefixApply` 를 접두 교체 채널로 재사용, `SelectHanja` 반환 문자열에 출력 형식 적용, 다음절 후보 글자별 뜻 합성, 헤더 target 축약** |

**`unim-dbus/SPEC.md` §6.1 (`:243-249`) 보강문(승인 불필요, 신규 기능 문서화)**
> `GetHanjaCandidates` 의 `target` 은 다음절 어절일 수 있다. `SelectHanja` 는 출력 형식이 적용된 문자열을 반환하며, 접두 교체가 필요한 경우 `CommitText` 대신 `AutoTypefixApply(u,s,s)`(preedit_text="") 시그널이 popup-owner path 로 발행된다. `SetSurroundingText` 는 이제 한자키(선택 영역 변환)의 선행 조건이기도 하다.

---

## 9. 테스트 계획

### 9.1 L1 단위 (`src/input_engine/tests_hanja_word.rs` 신규, `mod.rs:25-45` 에 `#[cfg(test)] mod tests_hanja_word;`) — `create_test_engine()`(`test_helpers.rs:9`) + `press_key` + `start_hanja_conversion`/`on_hanja_key` 패턴(`tests_scenarios.rs:277-303`)
두벌식 키: 대=e,o / 한=g,k,s / 민=a,l,s / 국=r,n,r. 기본 config 는 `commit_unit=Smart`, Linux `word_mode_apps` 빈 목록 → 음절 모드.

| # | 케이스 | 기대 |
|---|---|---|
| 1 | 정상 대상①: "대한민" 커밋 + "국" preedit → `press_key(F9)` | `hanja_mode`, `get_hanja_target()=="대한민국"`, `hanja_source==RecentAndPreedit{3}`, 후보[0].hanja=="大韓民國", `recent_commit=="대한민"` 유지 |
| 2 | 확정 (숫자 1 → `popup_select`) | `take_pending_hanja_replacement()==Some{delete 3, "大韓民國"}`, `commit_str()==""`, preedit "", `recent_commit` 빈, `InputResult.preedit_changed && !commit_changed` |
| 3 | 회귀: 단어 불일치("가나"+"다") | target=="다", source Preedit, `pending==None`, 확정 시 `commit_str()=="多"`류(현행과 동일 시퀀스 결과) |
| 4 | 회귀: 기존 `test_scenario_hanja_conversion` 전량 + `tests_popup_change_page.rs` 통과 | 변경 없음 |
| 5 | 미완성 자모("ㄱ" preedit) | 특수문자 폴백(현행) |
| 6 | 혼합 리스트: 케이스1 후보 목록 뒤에 "국" 음절 후보 존재, 음절 후보 선택 시 `pending==None` 이고 commit=="國" | scope 강등 |
| 7 | 서식 3종: `set_hanja_output_format` 각각 후 확정 | "大韓民國" / "대한민국(大韓民國)" / "大韓民國(대한민국)"; 단음절도 동일 규칙 |
| 8 | Word 모드(`set_word_mode(true)`): "오늘대한민국" preedit → F9 | target "대한민국", source WordPreedit{"오늘"}, 확정 commit=="오늘大韓民國", 취소 restore=="오늘대한민국" |
| 9 | Word 모드 불일치 폴백 | target 마지막 음절, 확정 시 접두 유지(기존 접두 유실 버그 수정 검증) |
| 10 | 대상②: `set_surrounding_text("대한민국 만세",4,0)` idle → `on_hanja_key` | target "대한민국", source Selection, 확정 commit=="大韓民國", `pending==None`; 취소 시 commit_buffer 빈 |
| 11 | 대상② 불일치("만세"+"!" 선택) | 팝업 없음, 이모지 없음(`is_emoji_popup_active()==false`), `take_ui_feedback()==Some(HanjaNoMatch)` |
| 12 | 대상② 선택 없음 idle | 이모지 팝업(현행) |
| 13 | 대상② 조합 중이면 선택 무시 | 조합 경로 |
| 14 | 비밀번호: `set_content_purpose(Password)` 진입 시 버퍼 clear; 진입 상태에서 push 는 clear; `start_hanja_conversion()`(pull) 은 consumed·팝업 없음; `set_surrounding_text` 비어 있음 | fail-closed |
| 15 | BS pop: "대한민" 커밋 후 BS(passthrough) → 버퍼 "대한" | pop_last |
| 16 | 리셋 조건 각 1케이스: Space·Enter·Tab·Escape·Left·Home·Delete·한/영 전환·`reset()`·특수문자 팝업 확정·이모지 확정 | 버퍼 빈 |
| 16b | 비한글 commit 9곳(§2.1): 한글 모드에서 `,`·숫자·기호(`:592`), Special 자모(`:426`), context_alt fallback(`:407`), chord NonJamo(`:1169`/`:1258`), chord fallback_jamos/non_jamos(`:1313`/`:1331`/`:1336`) 각각 "대한민" 뒤에 입력 | 버퍼 빈; 이어 "국"+F9 → target "국"(단어 결합 없음) |
| 16c | surrounding 정합성: "대한민" 커밋 후 `set_surrounding_text("나라 대한민", 6, 6)` → F9 | 일치 → target "대한민국". `set_surrounding_text("엉뚱한 텍스트", 7, 7)` → F9 → 버퍼 clear, target "국". `set_surrounding_text("", 0, 0)` → 검증 생략, target "대한민국". 앱이 잘라 보고(`"한민"`) → 허용 |
| 16d | 대상② 공백 보존: `set_surrounding_text(" 대한민국 ", 0, 6)` → 확정 | commit == " 大韓民國 " |
| 17 | 수정자 단독 키(LeftShift)는 리셋하지 않음 | 버퍼 유지 |
| 18 | 상한: 18음절 연속 입력 → 버퍼 17자, 19번째에서 앞 1자 탈락 | cap |
| 19 | 취소 3경로: `popup_cancel` → commit_buffer=="국"; `cancel_hanja()` 후 `get_hanja_restore_text()=="국"`; Selection 은 "" | D8 |
| 20 | 즐겨찾기: 단어 (대한민국,大韓民國) 토글 → 재정렬·`HanjaCandidatesReordered.target=="대한민국"`; 음절 후보 즐겨찾기가 단어 구간 위로 오지 않음 | 정렬 키 |
| 21 | 합성 뜻: `display_meaning(국가:國家)=="나라 국 · 집 가"`, 다중 뜻은 첫 항만 | dict |
| 22 | `dict.max_key_len()==18`, `RECENT_COMMIT_CAP==17` | 상수 정합 |
| 23 | ATF 회귀: `src/auto_typefix/tests.rs`(78 케이스)·`tests_atf_hotkey.rs`(27) 전량 | 무변경 통과 |
| 24 | 헤더 축약: view_model `display_target` 12자 초과 축약 | 문자열 |
| 25 | TSF cross 관련 코어: `is_hanja_key(F9)==true` | 접근자 |

### 9.2 L2 DBus (`tests/unim-test-dbus/src/main.rs` `test_hanja_popup:166-260` 옆에 `test_hanja_word_popup`)
1. 음절 모드 확인(`SetConfigYaml` 로 `commit_unit: Syllable` 임시 설정 후 복원, 또는 기본 Smart 가 Linux 에서 음절임을 전제).
2. `ProcessKeyEvent` 로 대한민국 11키 입력 → Hanja(evdev 123) → `GetHanjaCandidates` → target=="대한민국" 단정.
3. `receive_auto_typefix_apply()`(`unim-dbus/src/client.rs:259`) 스트림 구독 후 `SelectHanja(0)` → 시그널 `(3, "大韓民國", "")` 수신 + 반환값 "大韓民國".
4. 대상②: `Reset` → `SetSurroundingText("대한민국 만세",4,0)` → `ProcessKeyEvent(Hanja)` → `GetHanjaCandidates` target=="대한민국" → `SelectHanja(0)` → `receive_commit_text()` 로 CommitText "大韓民國"(AutoTypefixApply 미발행).
5. 불일치: `SetSurroundingText("만세!",3,0)` → Hanja → `GetHanjaCandidates` 빈 + `ShowEmojiPopupV2` 미발행.
6. 서식: `SetConfigYaml(hanja_output_format: HangulHanja)` → 3 반복 → "대한민국(大韓民國)"; 끝나면 원복.

### 9.3 L3 Xvfb (`tests/harness/scenarios/hanja-word.json` 신규, docs-rules-tests 보충#11)
```json
{ "name": "hanja-word-2bul", "desc": "대한민국 입력 후 F9 → 1 로 단어 한자 확정", "layout": "ko_2bulstd", "korean": true, "field": "core.plain",
  "steps": [ { "keys": ["e","o","g","k","s","a","l","s","r","n","r"], "expect": { "preedit": "국", "committed": "대한민" } },
             { "key": "F9" }, { "key": "1", "expect": { "preedit": "", "committed": "大韓民國", "rendered": "大韓民國" } } ] }
```
- 전제: 즐겨찾기 파일 부재(순서 고정), 기본 형식 Hanja, 음절 모드(Linux 기본).
- `harness.py:398-408` layout 패턴을 본떠 시나리오 키 `config: {"commit_unit":"syllable","hanja_output_format":"hanja"}` 를 `unim-cli config set` 으로 적용·복원하는 확장은 **선택**(레거시 `SetConfig` 에 두 키가 없으므로 CLI 경유; 기본값만으로 시나리오가 성립해 v1 필수 아님).
- 기대값 "大韓民國" 은 `hanja.txt` 에서 `대한민국:` 표제어가 단일 항목인지 L1 #1 로 먼저 고정한다(다중이면 첫 후보가 사전 순서에 의존). 즐겨찾기 파일이 있으면 순서가 바뀌므로 하네스는 `~/.local/share/unim/hanja-bookmarks.json` 부재를 전제(있으면 스킵 처리).
- 추가 시나리오(회귀, F1 게이트 검증): `{"keys":["a","b","c"]}` → `Shift+Left`×3 로 선택 → 한/영 전환키 → `expect committed "abc"`(선택이 지워지지 않음) → F9 → `expect committed "abc"`(영문 선택은 불일치 → 팝업·이모지 없음, 텍스트 무손상).
- Wayland 네이티브 앱은 xtest 미도달로 스킵(harness.py:388-390).

### 9.4 Windows
- `make check-windows`(`Makefile:520-524`, WIN_CRATES 에 unim·unim-tsf·unim-capi 포함) 경고 0.
- `unim-tsf` 단위 테스트(`popup_ipc.rs` 골든 라인) 무변경 통과 — wire 미수정.
- VM 검증 항목(이 환경 불가, §11 미해결): W1-b/W1-c 교체, InsertTextAtSelection 선택 치환, OnSetFocus 래퍼, CUAS(카톡) synth 경로.

### 9.5 문서(D11)
- `CHANGELOG-ko.md`/`CHANGELOG.md` `## [Unreleased]` — 추가됨: "한자 팝업의 단어 변환 지원 — 방금 입력한 어절(예: 대한민국)과 앱에서 선택한 한글 단어를 한 번에 한자로 변환" / "[설정] 한자 출력 형식(漢字 · 한자(漢字) · 漢字(한자)) 추가" / "한자 팝업 다음절 후보에 글자별 뜻 표시"; 고쳐짐: "단어 조합 모드에서 마지막 음절만 한자로 바꾸면 앞 글자가 사라지던 문제 수정" / "Windows: 한자 팝업이 열린 채 다른 창으로 포커스가 옮겨가면 팝업이 남던 문제 수정". 영문은 Added/Fixed 명사구, 항목 수 동일.
- 사용자 매뉴얼 `docs/user/user-guide/README-ko.md:375-383`(§4.2) + `README.md` 대응 절: (1) 단어 변환 절차와 예, (2) "조사를 붙이기 전에 한자키", (3) 선택 변환 절차 + 지원 환경 표(GTK4·Qt·GNOME·Wayland·Windows / GTK3·XIM 제외), (4) 출력 형식 설정, (5) 단어 즐겨찾기, (6) 마우스 클릭으로 커서를 옮긴 뒤 이어 치면 엉뚱한 단어가 뜰 수 있음 → Esc.
- `ROADMAP.md:118-127` ① 항목: "v1 완료(최장 접미·선택 변환·서식) / 남은 것: 조사 분리, 축소·확장 키" 로 갱신.
- `docs/dev/specs/HANJA_WORD_SPEC.md` 신규: 본 설계 §2-§5 요약 + §8 개정안 절.
- `unim-dbus/SPEC.md:243-249` 보강(§8 말미).
- 툴팁/라이브 도움말: §6.3.

---

## 10. 작업 분해 (WBS)

파일 소유권은 겹치지 않는다(같은 파일을 두 단위가 동시에 만지지 않음). 난이도: 쉬움=sonnet, 어려움=opus 구현 기준(검증은 한 단계 위).

| id | 내용 | 파일(소유) | 선행 | 난이도 | 검증 |
|---|---|---|---|---|---|
| **C1** 사전 | `contains_key`, `max_key_len`, `char_meaning` 역색인, `display_meaning`(§2.5) | `src/hanja/dict.rs`, `src/hanja/mod.rs` | — | 쉬움 | L1 #21-22 |
| **C2** 설정 코어 | `HanjaOutputFormat` + `format` + `KoreanConfig` 필드/Default(§2.3, §6.1) | `src/config.rs` | — | 쉬움 | `cargo test -p unim` config 직렬화 왕복 |
| **C3** 엔진 상태·훅 | `recent_commit.rs` 신규; `engine.rs` 필드·new·reset·`set_hanja_output_format`·`is_hanja_key`·`take_pending_hanja_replacement`·`take_ui_feedback`·`get_hanja_restore_text`; `press_key.rs` 5 push 훅·BS pop·리셋 지점·`:226-238` → `on_hanja_key` 위임; `surrounding.rs` content_purpose clear; `types.rs` `HanjaReplacement`/`HanjaTargetSource`/`UiFeedback`; `mod.rs` `mod recent_commit;` | `src/input_engine/{recent_commit.rs,engine.rs,press_key.rs,surrounding.rs,types.rs,mod.rs}`, `src/hangul/char.rs`(is_syllable 헬퍼) | C1, C2 | 어려움 | L1 #14-18, #23 |
| **C4** 변환 상태기계 | `resolve_hanja_target`/`resolve_selection_target`/`on_hanja_key`/`open_hanja_popup`/`assemble_candidates`(혼합 리스트)/`select_hanja`/`cancel_hanja`/`toggle_hanja_bookmark` 재조립; `popup_dispatch.rs` `popup_select`/`popup_cancel` | `src/input_engine/{candidates.rs,popup_dispatch.rs}` | C3 (C3 가 `on_hanja_key` 를 호출하므로 C3·C4 는 동일 브랜치에서 순차 또는 C4 가 스텁을 먼저 제공) | 어려움 | L1 #1-13, #19-20 |
| **C5** 뷰모델 | 헤더 target 축약(§7.1) | `src/popup/view_model.rs` | — | 쉬움 | L1 #24 (view_model 테스트) |
| **T1** L1 테스트 | §9.1 전량 | `src/input_engine/tests_hanja_word.rs` (+`mod.rs` test 선언 한 줄 — C3 완료 후 추가) | C3, C4, C5 | 쉬움 | `cargo test -p unim` |
| **D1** 데몬 | §5.1: ProcessKey 드레인·응답 조립, `SelectHanja`/`CancelHanja` 응답 타입, `redirect_auto_typefix_apply_and_hide`, 리로드 setter, 비프(선택) | `unim-dbus/src/{engine_worker.rs,service.rs,beep.rs}` | C3, C4 | 어려움 | L2 §9.2 |
| **T2** L2 테스트 | §9.2 | `tests/unim-test-dbus/src/main.rs` | D1 | 쉬움 | `make test-dbus`(기존 타깃) |
| **F1** GTK4 | 선택 삭제 래퍼 게이트 | `unim-frontends/gtk4/src/immodule.c:1058-1084` | — (D1 과 병렬) | 쉬움 | L3 + 수동: gedit(X11) 선택→F9→Esc 선택 유지 |
| **F2** Qt | F9 재질의 + 래퍼 게이트, qt6 미러 | `unim-frontends/qt5/src/input_context.cpp`, `unim-frontends/qt6/src/input_context.cpp` | — | 쉬움 | 수동: kate 선택→F9 |
| **F3** Wayland | 바이트 판별자 + SurroundingText 배선 + `DbusRequest::SetSurroundingText` | `unim-frontends/wayland/src/{state.rs,dbus_client.rs,main.rs}` | — | 어려움 | 수동(sway/labwc + foot·GTK4 wl 앱): '한자(漢字)' 형식 확정 후 문서 무결 |
| **F4** XIM | ATF 도착 시 preedit 선제 클리어 | `unim-frontends/xim/src/handler.rs:520-540` | — | 쉬움 | 수동: xterm 마우스 확정 잔상 없음 |
| **F5** GNOME | preedit 선제 클리어 + 헤더 ellipsize | `unim-gnome-extension/{extension.js,popup_view.js}` | — | 쉬움 | `make check-compat` + 수동 |
| **R1** 렌더러 | GTK 헤더 ellipsize; Windows 폭·ellipsis | `unim-popup-service/src/popup/hanja.rs`, `unim-popup-win/src/render.rs` | — | 쉬움 | 빌드 + 수동/스크린샷, check-windows |
| **W1** TSF 키/마우스 | §5.3 W1-a/b/c (반환형 변경 포함) + `read_selection_text` 빈 선택 완화(`composition.rs:1584`) | `unim-tsf/src/key_handler.rs`, `unim-tsf/src/composition.rs` | C3, C4 | 어려움 | `make check-windows`; VM 대기 |
| **W2** TSF 서비스 | §5.3 W2-a/b (`schedule_phase2_flush` 추출, OnSetFocus 래퍼) | `unim-tsf/src/text_service.rs` | W1 (시그니처) | 어려움 | `make check-windows` |
| **S1** GTK 설정 | ComboRow + 로케일 + 한자키 문구 개정 | `unim-settings-gtk/src/settings_dialog.rs`, `unim-settings-gtk/locales/{ko,en}.yml` | C2 | 쉬움 | 실행·저장·`config.yaml` 확인 |
| **S2** Slint 설정 | 프로퍼티·콤보·바인딩·merge_ui_owned | `unim-settings/ui/settings.slint`, `unim-settings/src/main.rs` | C2 | 쉬움 | 실행·저장 |
| **S3** GTK 병합 | merge_gtk_ui_owned 한 줄 | `unim-gui-common/src/settings_helpers.rs` | C2 | 쉬움 | S1 저장 시 disk 반영 |
| **S4** CLI | ConfigKey·set/show·로케일 | `unim-cli/src/main.rs`, `unim-cli/locales/{ko,en}.yml` | C2 | 쉬움 | `unim-cli config set/show` |
| **S5** Win 레거시 모달 | 콤보 1개 | `unim-tsf/src/settings_dialog.rs` | C2 | 쉬움 | check-windows |
| **T3** L3 | 시나리오 json (+ harness config 확장 '선택') | `tests/harness/scenarios/hanja-word.json`, (`tests/harness/harness.py`) | D1, F1 | 쉬움 | `make test-l3`(기존 타깃명 확인) |
| **X1** 문서 | CHANGELOG ko/en, 매뉴얼 ko/en §4.2, ROADMAP, `HANJA_WORD_SPEC.md`(개정안 절 포함), `unim-dbus/SPEC.md` | 해당 md 파일들 | 전부(내용 확정 후) | 쉬움 | 리뷰; `make help`(gen-help) |

병렬 가능 묶음: {C1, C2, C5} → {C3→C4} ∥ {F1, F2, F3, F4, F5, R1} → {D1, W1, S1-S5, T1} → {W2, T2, T3} → X1. 커밋은 하지 않는다(D12).

---

## 11. 위험 · 롤백 · 미해결

### 11.1 PM 결정에서 벗어난 항목(deviations)
1. **혼합 후보 리스트**(§3.1): D5 는 "최장 접미 1개를 target" 이며 축소 키는 v2. 최장 일치가 있을 때 음절 변환이 v1 에서 불가능해지는 막다른 길을 막기 위해 단어 후보 뒤에 음절 후보를 이어 붙였다(추가 키 0, 상수 1개로 끌 수 있음). 대안: v2 축소 키까지 막다른 길을 감수.
2. **다음절 후보 뜻 합성**(§2.5): 결정 사항에 없던 UX 추가. 사전 파싱 1회 역색인, 렌더러 무수정. 대안: 뜻 빈 채로 출시(國家/國歌 구분 불가).
3. **GTK4·Qt 선택 삭제 래퍼 게이트**(§5.2): 프런트 무수정 전제(D1 "GTK4 지원")에서 벗어난 소규모 수정. 없으면 팝업 열림 순간 선택이 삭제돼 취소 시 원문이 유실된다(D8 "선택 유지" 불충족). 대안: 대상② 취소 시 원문 재커밋(GNOME/Wayland/TSF 와 동작이 갈려 채택하지 않음).
4. **Word 모드 폴백 접두 보존**(§3.1 3단계): 현행은 마지막 음절만 바꾸면 word_buffer 접두가 사라진다 — 버그 수정으로 포함.
5. **Wayland 바이트 수**(D9): "프런트 커밋 이력/SurroundingText 저장/시그널 확장" 3택 대신 **한자 포함 판별자 + 3B 확정값**을 택했다 — 대상① 삭제 대상은 구성상 항상 완성 음절이라 추정이 필요 없고 ATF 경로는 바이트 동일. SurroundingText 배선은 대상② 용도로만 신설.
6. **XIM Reset**(D9): 회피가 아니라 무해 증명(§5.2) — 코드 무수정.
7. **대상② 공백 보존**(§3.2/§3.4): D6 의 "앞뒤 공백 trim" 은 판정 기준으로만 쓰고, 확정 문자열에는 trim 한 공백을 되붙인다 — 위젯이 선택 전체를 치환하므로 그렇지 않으면 공백이 사라진다. D6 의 의도(공백 낀 선택도 인식)를 지키면서 문서를 보존하는 보완.
8. **surrounding 정합성 검증**(§3.1 4-0): D3/D5 에 없는 안전망. 훅 기반 리셋이 못 잡는 문맥 단절(클릭·앱 자동교정·Reset 없는 Wayland)에 대비해 사전 조회 직전에 앱 보고 텍스트와 버퍼를 대조한다. 실패 시 현행(마지막 음절)으로 조용히 폴백하므로 회귀 0. 대안(검증 없음)은 XIM 수준의 잔여 위험을 전 프런트로 확대.
9. **TSF `read_selection_text` 완화**(§5.3 W1-a): 선택이 비어도 surrounding 을 돌려주게 `:1584` 를 바꾼다 — 8 의 검증을 TSF 에서 살리기 위함. 기존 호출부는 가드가 있어 바이트 동일.

### 11.2 위험과 완화
| 위험 | 완화 |
|---|---|
| 잘못된 결합(클릭으로 커서 이동 뒤 이어 침, Wayland/XIM 은 Reset 없음; 앱 자동교정으로 문서가 바뀜) | 1차: §3.1 4-0 surrounding 정합성 검증이 GTK3/4·Qt·GNOME·Wayland·TSF 에서 stale 버퍼를 버린다(surrounding 은 이미 매 키/포커스마다 들어온다). 2차: 팝업 헤더로 즉시 인지 → Esc 로 취소해도 문서 무손상(접두는 손대지 않음). XIM 은 2차만(매뉴얼 명시) |
| 한글 모드에서 쉼표·숫자 등 비한글 문자를 사이에 두고 이어 친 뒤 F9 | §2.1 비한글 commit 9곳의 `clear()` 훅(L1 #16b). 누락 시 `delete_chars` 가 문장부호를 지우므로 **필수 항목** |
| GTK4/Qt 선택 삭제 래퍼 게이트(F1/F2b)가 한/영 전환 등 "빈 소비 키" 의 기존 선택 삭제 동작을 바꿈 | 기존 동작이 의도된 것이라는 근거가 없고(선택 위에서 한/영 전환이 텍스트를 지우는 것은 버그에 가깝다) 게이트 조건은 한 줄이라 즉시 되돌릴 수 있음. L3 로 "선택 → 한/영 전환 → 선택 유지" 회귀 케이스 추가(§9.3) |
| GTK4 `delete_surrounding` 미지원 앱(Electron)의 XTest BS 폴백이 한글 3자를 지우는 도중 다른 키 입력 | ATF 와 동일 기존 위험(`immodule.c:488-519`), 신규 아님 |
| Wayland 앱이 `delete_surrounding_text` 미지원 | 기존 ATF 와 동일 한계 |
| TSF CUAS 앱(카톡)에서 synth BS 경로 | `replace_surrounding` 의 SynthBatch 가 처리(ATF 검증 경로 재사용) — VM 검증 필요 |
| 사전 순서=빈도순 미검증(보충#7(3)) | 즐겨찾기로 보완; 문서에 명시 |
| `AutoTypefixApply` 재사용으로 프런트가 한자 교체를 ATF 로 오인해 되돌리기(undo) 관찰 등 부수 처리 | 데몬 측 `undo_states`/`recent_corrections` 는 ATF 블록 안에서만 갱신되고 이 프레임은 블록 미진입 → 영향 없음. GNOME `expectSelfBackspaces` 는 BS 개수만 등록 |
| 혼합 리스트가 9개/페이지를 넘겨 페이지가 늘어남 | 단어 후보는 대개 1~3개, 음절 후보는 2페이지부터 — 첫 페이지 UX 불변 |
| 설정 지점 11곳 중 누락 | §6.2 표를 체크리스트로, S1~S5 별도 단위 |

### 11.3 롤백
- 코어: `HANJA_WORD_APPEND_SYLLABLE_TAIL=false`(혼합 리스트 끄기); `resolve_hanja_target` 의 4단계 루프 상한을 1 로 두면 완전 현행 동작(회귀 스위치). 정식 on/off 설정은 두지 않는다(D5 "다음절 일치 없으면 현행"이 곧 안전망) — 필요해지면 `KoreanConfig.hanja_word_conversion: bool` 추가는 §6.2 표를 그대로 따른다.
- 프런트: F1~F5 는 각각 독립 되돌림 가능(게이트 조건·판별자 한 줄).
- 커밋 전이므로 작업 트리 되돌림으로 충분(D12).

### 11.4 미해결(실측·승인 필요)
1. POPUP_SPEC §3.7/§9.2 개정(§8) — 기현님 승인.
2. GTK 코어가 `gtk_im_context_reset()` 을 부르는 전체 조건(WebKitGTK 등) — 리포 밖, 실측(연속 타이핑 중 로그).
3. Mutter `vfunc_set_surrounding`/`vfunc_reset` 호출 빈도 — 실측(gnome-text-editor 선택→F9).
4. GtkTextView·St.Entry·Chromium(text-input-v3)·Electron 의 commit 시 선택 치환 — GtkText·QLineEdit 는 원문 확인, 나머지는 실측.
5. TSF `InsertTextAtSelection` 선택 치환·`end_composition_keep_text`+`replace_surrounding` 조합의 실제 문서 결과·OnSetFocus 래퍼 — VM.
6. GTK4 마우스 확정 후 preedit 잔상이 현재도 남는지(단음절 경로) — 실측; 신규 경로는 `on_auto_typefix` 가 명시 클리어.
7. Qt 오프셋 UTF-16→문자 변환이 서로게이트 포함 텍스트에서 정확한지 — 실측.
8. XIM 마우스 확정 preedit 잔상(F4 로 방어) — 실측.
9. L3 harness config 확장 채택 여부(선택).
10. 비프 피드백 채택 여부(선택, 접근성 옵션 게이트).
11. 앱별 surrounding 보고 지연 — 커밋 직후 앱이 surrounding 을 늦게 갱신하면 4-0 검증이 버퍼를 불필요하게 버려 단어 변환이 조용히 음절 변환으로 떨어진다(안전 방향 실패). GTK4(:865 동기 요청)·TSF(동기 EditSession)는 구조상 즉시, GNOME/Wayland 는 사람 속도에서 충분 — 실측으로 확인.
12. 사용자 매뉴얼에 "혼합 후보 리스트"(단어 뒤 음절 후보)를 어떻게 설명할지 — PM 이 deviation 1 을 기각하면 이 문단도 삭제.
