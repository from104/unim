# Phase 3: 자동 실시간 한영 오타 수정 — 구현 계획서

> 작성일: 2026-04-05
> 기반: 03_feasibility_report.md (승인 완료)

## 목표

단어 경계(Space/구두점/Enter) 시 자동으로 한영 오타를 감지하여 교정하는 AutoTypeFix 기능 구현

---

## 구현 순서

### Step 1: 영어 사전 파일 준비

**파일**: `src/data/english_words.txt` (신규)

- 고빈도 영어 단어 ~50,000개 (줄바꿈 구분, 소문자)
- Google 10,000 English + 추가 기술/일상 어휘로 구성
- 형식: 한 줄에 하나의 영단어

**로딩 방식**: `include_str!("data/english_words.txt")` — hanja.txt와 동일한 패턴 (src/hangul/hanja.rs:9 참조)

---

### Step 2: auto_typefix 모듈 (Core 레이어)

**파일**: `src/auto_typefix.rs` (신규)

```
src/auto_typefix.rs
├── ENGLISH_WORDS: &str = include_str!("data/english_words.txt")
├── EnglishDictionary (lazy_static 또는 LazyLock)
│   ├── words: HashSet<String>
│   ├── fn contains(&self, word: &str) -> bool
│   └── fn new() -> Self  // ENGLISH_WORDS 파싱
├── AutoTypeFixResult { original: String, corrected: String, delete_chars: u32 }
├── fn detect_and_correct(word, input_category, korean_layout, english_layout) -> Option<AutoTypeFixResult>
│   ├── 순방향 (영어모드): is_english_keystrokes() → eng_to_kor() → 한글이면 교정
│   └── 역방향 (한글모드): is_korean_text() → kor_to_eng() → 사전 매칭이면 교정
└── tests
```

**핵심 로직**:

```rust
pub fn detect_and_correct(
    word: &str,
    input_category: InputCategory,
    korean_layout: KoreanLayout,
    english_layout: EnglishLayout,
) -> Option<AutoTypeFixResult>
```

- **순방향** (현재 영어모드, 입력이 한글 자모 패턴):
  - `typefix::is_english_keystrokes(word, &keyboard_map)` 로 한글 자모 매핑 확인
  - `typefix::eng_to_kor(word, korean_layout, english_layout)` 로 변환
  - 변환 결과가 한글 텍스트이면 교정 결과 반환
  - **사전 불필요** — 영어 자판 위치가 한글 자모에 매핑되면 충분

- **역방향** (현재 한글모드, 입력이 한글):
  - `typefix::is_korean_text(word)` 로 한글 확인
  - `typefix::kor_to_eng(word, korean_layout, english_layout)` 로 영문 변환
  - `EnglishDictionary::contains(영문)` 로 사전 매칭
  - 사전에 있으면 교정 결과 반환

**의존 관계**: typefix.rs의 기존 함수 재활용 (eng_to_kor:23, kor_to_eng:61, is_english_keystrokes:75, is_korean_text:83)

**src/lib.rs:10 이후에 추가**:
```rust
pub mod auto_typefix;
```

---

### Step 3: Config 설정 추가 (6곳 동기화)

#### 3-1. `src/config.rs` — 소스 오브 트루스

**EngineConfig 구조체** (config.rs:309 부근):
```rust
pub struct EngineConfig {
    // ... 기존 필드들 ...
    /// 자동 오타 교정 (AutoTypeFix) 활성화 여부
    pub auto_typefix: bool,
}
```

**Default 구현** (config.rs:332 부근):
```rust
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            // ... 기존 ...
            auto_typefix: true,  // 기본 활성화
        }
    }
}
```

#### 3-2. `unim-config/src/main.rs` — CLI ConfigKey enum

**ConfigKey enum** (main.rs:41 부근):
```rust
enum ConfigKey {
    // ... 기존 ...
    /// 자동 오타 교정 활성화 (true, false)
    #[value(name = "auto-typefix")]
    AutoTypeFix,
}
```

**config_set 함수** (main.rs:149 부근) + **config_show 함수** (main.rs:74 부근)에 분기 추가

#### 3-3. `unim-config/locales/ko.yml` + `en.yml`

```yaml
# ko.yml
auto_typefix: "자동 오타 교정"
auto_typefix_desc: "단어 입력 후 한/영 오타 자동 감지 및 교정"

# en.yml
auto_typefix: "Auto TypeFix"
auto_typefix_desc: "Automatically detect and correct Korean/English typing errors"
```

#### 3-4. `unim-dbus/src/service.rs` — get_config/set_config

**get_config** (service.rs:377 부근):
```rust
"auto_typefix" => config.engine.auto_typefix.to_string(),
```

**set_config** (service.rs:408 부근):
```rust
"auto_typefix" => {
    config.engine.auto_typefix = value.parse().unwrap_or(true);
}
```

#### 3-5. `unim-gui-gtk/src/gtk_ui.rs` — GTK UI

auto_switch와 유사한 토글 스위치 위젯 추가 (auto_switch가 이 파일에 없으므로 패턴 확인 후 적절한 위치에 추가)

#### 3-6. GNOME Extension

**`unim-gnome-extension/schemas/org.gnome.shell.extensions.unim.gschema.xml`** (gschema.xml:73 부근):
```xml
<key type="b" name="auto-typefix">
  <default>true</default>
  <summary>Auto TypeFix</summary>
  <description>자동 한/영 오타 교정</description>
</key>
```

**`unim-gnome-extension/prefs.js`** (prefs.js:140 부근):
- 'auto-typefix' 키 바인딩 추가
- YAML 동기화 로직 추가 (popup-mode 패턴 참조)

---

### Step 4: Engine Worker 후처리 (핵심 통합 지점)

**파일**: `unim-dbus/src/engine_worker.rs`

**변경 위치**: `EngineRequest::ProcessKey` 핸들러 (engine_worker.rs:100~206)

ProcessKey 응답 생성 후, commit이 있고 그 commit이 단어 경계 문자(Space, 구두점, Enter)인 경우 AutoTypeFix를 트리거한다.

**수정 방식** — ProcessKey 핸들러 내부, 응답 생성 직전 (engine_worker.rs:188 부근):

```rust
// === AutoTypeFix 후처리 ===
// commit이 단어 경계 문자(공백/구두점/Enter)인 경우 직전 단어를 검사
let auto_typefix_result = if config.engine.auto_typefix {
    if let Some(ref commit_str) = commit {
        let last_char = commit_str.chars().last().unwrap_or('\0');
        if is_word_boundary(last_char) {
            // surrounding_text에서 직전 단어 추출
            let (text, cursor, _) = engine.surrounding_text();
            extract_last_word(text, cursor as usize)
                .and_then(|word| {
                    unim::auto_typefix::detect_and_correct(
                        &word,
                        engine.input_category(),
                        config.engine.korean.layout,
                        config.engine.english.layout,
                    )
                })
        } else {
            None
        }
    } else {
        None
    }
} else {
    None
};
```

**헬퍼 함수** (engine_worker.rs 상단):
```rust
/// 단어 경계 문자인지 판별
fn is_word_boundary(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\t'
        || c == '.' || c == ',' || c == '!' || c == '?'
        || c == ';' || c == ':' || c == '/' || c == '\\'
}

/// surrounding text에서 커서 직전 단어를 추출
fn extract_last_word(text: &str, cursor: usize) -> Option<String> {
    if text.is_empty() || cursor == 0 {
        return None;
    }
    let chars: Vec<char> = text.chars().take(cursor).collect();
    // 뒤에서부터 단어 경계를 찾음
    let end = chars.len();
    let start = chars.iter().rposition(|c| is_word_boundary(*c))
        .map(|pos| pos + 1)
        .unwrap_or(0);
    if start >= end {
        return None;
    }
    let word: String = chars[start..end].iter().collect();
    if word.is_empty() { None } else { Some(word) }
}
```

**문제**: surrounding_text는 commit 이전 시점의 텍스트다. Space를 누르면 한글 조합이 commit되고 Space도 commit되지만, surrounding_text는 아직 업데이트 전이다.

**해결**: surrounding_text의 커서 앞 텍스트 + preedit(이미 commit됨) + commit에서 단어 경계 문자를 제외한 부분을 합쳐서 직전 단어를 구성한다. 실제로는 한글모드에서 Space 입력 시:
1. 조합 중인 한글이 먼저 commit됨
2. Space가 commit됨  
3. surrounding_text에는 이미 이전 커밋까지 반영되어 있음 (프론트엔드가 SetSurroundingText를 호출하는 시점에 따라 다름)

**실용적 접근**: surrounding_text가 최신이 아닐 수 있으므로, engine_worker에서 **별도의 단어 버퍼**를 유지한다.

---

### Step 4-B: 단어 버퍼 방식 (권장)

engine_worker에 context별 단어 버퍼를 추가한다:

```rust
// engine_worker.rs:42 부근
let mut contexts: HashMap<u32, InputEngine> = HashMap::new();
let mut word_buffers: HashMap<u32, String> = HashMap::new();  // 추가
```

**ProcessKey 핸들러 수정**:

1. commit이 발생하면 commit 문자를 word_buffer에 축적
2. commit의 마지막 문자가 단어 경계이면:
   - word_buffer에서 경계 문자를 제외한 부분이 "직전 단어"
   - `detect_and_correct()` 호출
   - 결과가 있으면 EngineResponse에 `auto_typefix` 필드 추가
3. 모드 전환, FocusIn/Out 시 word_buffer 초기화

**EngineResponse 확장** (service.rs:151 부근):
```rust
pub struct EngineResponse {
    // ... 기존 ...
    /// AutoTypeFix 교정 결과 (delete_chars, replacement)
    pub auto_typefix: Option<(u32, String)>,
}
```

---

### Step 5: DBus 시그널 발행 (프론트엔드 연동)

**파일**: `unim-dbus/src/service.rs`

ProcessKeyEvent 메서드 (service.rs:597) 내부에서, EngineResponse에 auto_typefix 결과가 있으면 SmartBackspace와 동일한 패턴으로 처리:

```rust
// service.rs — process_key_event 내부, 팝업 시그널 이후
if let Some((delete_chars, replacement)) = response.auto_typefix {
    unim_log!("DBUS", "[DBus] AutoTypeFix: delete={}, replacement='{}'", delete_chars, replacement);
    // 단어 경계 문자(Space 등)가 이미 commit된 상태이므로
    // delete_chars + 1 (경계 문자 포함) 삭제 후 replacement + 경계 문자 재커밋
    Self::delete_surrounding_text(&signal_ctx, -((delete_chars + 1) as i32), delete_chars + 1)
        .await.ok();
    // 교정된 텍스트 + 원래 경계 문자 커밋
    let boundary_char = commit.chars().last().unwrap_or(' ');
    let corrected = format!("{}{}", replacement, boundary_char);
    Self::commit_text(&signal_ctx, &corrected).await.ok();
}
```

**주의**: commit_text 시그널은 이미 정의되어 있음 (SmartBackspace에서 사용 중). delete_surrounding_text도 기존 시그널 재활용.

---

### Step 6: 되돌리기 (Ctrl+Z) 지원

**파일**: `unim-dbus/src/engine_worker.rs`

AutoTypeFix가 실행된 직후 Ctrl+Z가 입력되면 원래 텍스트로 복원:

```rust
// context별 마지막 AutoTypeFix 결과 저장
let mut last_autofix: HashMap<u32, (String, String)> = HashMap::new();  // (original, corrected)
```

ProcessKey에서 Ctrl+Z 감지 시:
- last_autofix에 해당 context의 기록이 있으면
- corrected를 삭제하고 original을 재커밋
- last_autofix에서 제거 (1회만 되돌리기)

---

## 변경 파일 요약

| # | 파일 | 변경 내용 | 신규/수정 |
|---|------|-----------|-----------|
| 1 | `src/data/english_words.txt` | 영어 사전 ~50,000 단어 | 신규 |
| 2 | `src/auto_typefix.rs` | AutoTypeFix 감지/교정 모듈 | 신규 |
| 3 | `src/lib.rs:10` | `pub mod auto_typefix;` 추가 | 수정 |
| 4 | `src/config.rs:309,332` | `auto_typefix: bool` 필드 + Default | 수정 |
| 5 | `unim-config/src/main.rs:41,74,149` | ConfigKey::AutoTypeFix + show/set | 수정 |
| 6 | `unim-config/locales/ko.yml` | 한국어 번역 추가 | 수정 |
| 7 | `unim-config/locales/en.yml` | 영어 번역 추가 | 수정 |
| 8 | `unim-dbus/src/service.rs:151,377,408,597` | EngineResponse 확장 + get/set_config + 시그널 | 수정 |
| 9 | `unim-dbus/src/engine_worker.rs:42,100` | word_buffer + ProcessKey 후처리 + Ctrl+Z | 수정 |
| 10 | `unim-gui-gtk/src/gtk_ui.rs` | AutoTypeFix 토글 스위치 | 수정 |
| 11 | `unim-gnome-extension/schemas/...gschema.xml` | auto-typefix 키 정의 | 수정 |
| 12 | `unim-gnome-extension/prefs.js` | 설정 UI + YAML 동기화 | 수정 |

---

## 검증 방법

1. **Unit Test** — `src/auto_typefix.rs`
   - `detect_and_correct("gksrmf", English, Dubeolsik, Qwerty)` → `Some("한글")`
   - `detect_and_correct("ㅗ디ㅣㅐ", Korean, Dubeolsik, Qwerty)` → `Some("hello")`
   - 사전에 없는 단어 → `None`
   - 빈 문자열 → `None`

2. **빌드 검증** — `cargo build --workspace` + `cargo test --workspace` 제로 워닝

3. **통합 테스트** (수동)
   - 영어모드에서 "gksrmf " 입력 → "한글 "로 자동 교정
   - 한글모드에서 "ㅗ디ㅣㅐ " 입력 → "hello "로 자동 교정
   - Ctrl+Z로 되돌리기
   - 설정에서 비활성화 후 교정 안 됨 확인

4. **설정 동기화** — `unim-config set auto-typefix false` → config.yaml 반영 확인

---

## 리스크 및 주의사항

1. **Core 순수성**: auto_typefix.rs는 `src/`에 위치하지만 UI/플랫폼 의존성 없음. typefix.rs와 동일한 레벨. HashSet + include_str! 만 사용.

2. **영어 사전 크기**: ~50,000 단어 x 평균 7바이트 = ~350KB. include_str!로 바이너리에 임베드. hanja.txt (약 200KB)보다 약간 크지만 허용 범위.

3. **surrounding_text 타이밍**: 프론트엔드마다 SetSurroundingText 호출 시점이 다를 수 있음. word_buffer 방식으로 이 문제를 우회.

4. **false positive**: 순방향(영어→한글)에서 "dks" 같은 짧은 입력이 "안"으로 교정될 수 있음. **최소 길이 제한** (3자 이상) 필요.

5. **역방향 false positive**: 한글 단어가 우연히 영어 단어 키스트로크와 일치할 수 있음. 짧은 영단어(2~3자)는 사전에서 제외하거나 빈도 기반 필터링 필요.

6. **Ctrl+Z 충돌**: 앱 자체의 Undo와 충돌 가능. AutoTypeFix의 Ctrl+Z는 교정 직후(다음 키 입력 전)에만 유효하게 제한.

7. **세벌식 지원**: typefix.rs의 eng_to_kor/kor_to_eng가 이미 세벌식을 지원하므로 추가 작업 불필요.

8. **빌드 순서**: Step 1(사전) → Step 2(모듈) → Step 3(config) → Step 4(worker) → Step 5(DBus) → Step 6(Undo) 순서 엄수. 각 단계마다 빌드+테스트.

---

## 구현 우선순위 (MVP → Full)

**MVP (Phase 3a)**:
- Step 1 + Step 2 + Step 3 (config만) + Step 4-B (word_buffer)
- 순방향만 먼저 (영어모드→한글, 사전 불필요)
- Ctrl+Z 미구현

**Full (Phase 3b)**:
- Step 5 (DBus 시그널)
- 역방향 (한글모드→영문, 사전 필요)
- Step 6 (Ctrl+Z 되돌리기)
- Step 3 나머지 (GUI/GNOME 설정)
