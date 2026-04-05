# Keystroke Replay 접근법 실현 가능성 분석

## 0. 진단 문서 오류 정정

`03_autotypefix_diagnosis.md`의 **문제 3 ("preferred_direct=true로 commit 미발생")은 오진이다.**

근거:
- `preferred_direct`는 `src/config.rs:263`에 필드만 정의되어 있고, **어떤 코드에서도 읽히지 않는다** (grep 확인).
- `process_english_key()` (`src/input_engine.rs:526-550`)는 무조건 `commit_buffer.push(c)` 후 `InputResult::committed()`를 반환한다.
- GNOME extension `key_handler.js:204`는 영어 모드에서도 항상 `processKey()`를 호출한다.
- 즉, **영어 모드에서도 ProcessKeyEvent는 호출되고, commit도 발생한다.**

이 발견은 사용자 제안 접근법의 실현 가능성을 크게 높인다.

---

## 1. 아키텍처 적합성 분석

### 1.1 ProcessKey에서 모든 키스트로크를 볼 수 있는가?

**YES.**

경로: GNOME extension `key_handler.js:204` → DBus `ProcessKeyEvent` → `engine_worker.rs:106-344` → `engine.press_key(key, modifier, &config)`

- `engine_worker.rs:109`: `keycode: u32` (evdev keycode) 수신
- `engine_worker.rs:142`: `KeyCode::from_evdev_keycode(keycode as u16)` 변환
- `engine_worker.rs:143`: `ModifierState::from_x11_mask(state)` 변환

**영어 모드에서도 호출됨**: `press_key()` → `process_english_key()` → commit 발생 → engine_worker가 결과를 받음.

유일한 예외: modifier 키 단독(Shift, Ctrl 등)은 GNOME extension에서 먼저 필터링 (`key_handler.js:176`). Ctrl/Alt 조합도 필터링 (`key_handler.js:186`). 이는 AutoTypeFix에 필요 없는 키이므로 문제 없음.

### 1.2 timestamp를 볼 수 있는가?

**NO -- 현재 파이프라인에 timestamp가 없다.**

- `EngineRequest::ProcessKey`에는 `keyval`, `keycode`, `state`만 있다 (`service.rs:24-30`).
- DBus `ProcessKeyEvent` 메서드 시그니처: `(keyval: u32, keycode: u32, state: u32)`.
- timestamp는 프론트엔드에서 버려진다.

**해결 방안**: engine_worker 내에서 `std::time::Instant::now()`를 사용하면 된다. DBus 경유 레이턴시가 ~1ms 이하이므로 프론트엔드 timestamp와 거의 동일하다. 시간 윈도우가 100ms~3000ms이면 1ms 오차는 무시 가능.

### 1.3 keycode로부터 실제 입력된 문자를 복원할 수 있는가?

**YES.**

- `KeyCode::to_char()` / `to_shifted_char()` (`src/keycode.rs:263-374`): QWERTY 기준 문자 반환.
- `EnglishKeymap::get_char(keycode, shifted)`: JSON 기반 레이아웃별 매핑 (Dvorak/Colemak 지원).
- `KeyboardMap` (`src/keystroke/`): 영문 키 → 한글 자모 매핑.

즉, (KeyCode, ModifierState) 쌍만 있으면 양쪽 언어의 출력 문자를 모두 복원할 수 있다.

---

## 2. 키스트로크 버퍼 구현 가능성

### 2.1 구조

```rust
struct KeystrokeEntry {
    keycode: KeyCode,
    modifier: ModifierState,
    timestamp: Instant,  // engine_worker 내에서 기록
}

struct KeystrokeBuffer {
    entries: VecDeque<KeystrokeEntry>,
    max_size: usize,  // 설정값 (3~10)
}
```

### 2.2 삽입 지점

`engine_worker.rs:140` (`if let Some(engine) = contexts.get_mut(&context_id)` 블록 시작) 직후에:

```
1. 키스트로크 버퍼에 (key, modifier, Instant::now()) 추가
2. engine.press_key() 호출 (기존 로직)
3. 결과 처리 후, 버퍼 검사 → 교정 필요 시 교정 동작
```

### 2.3 버퍼 관리

- 문자 키(is_character_key)만 버퍼에 추가. Enter/Backspace/Tab/Escape는 버퍼를 **초기화**.
- 모드 전환 시 버퍼 초기화 (engine_worker.rs:198의 기존 패턴 활용).
- FocusIn/FocusOut 시 초기화 (engine_worker.rs:356의 기존 패턴 활용).
- 시간 윈도우 초과한 오래된 엔트리는 검사 시 자동 제거.

### 2.4 성능

- VecDeque 최대 10개 요소: O(1) push/pop, O(10) 검사 = 무시할 수 있는 비용.
- 매 키 입력마다 검사해도 총 비용 < 1us.
- Instant::now()는 vDSO 호출로 ~20ns.

---

## 3. 감지 로직 분석

### 3.1 방향 A (영어 모드에서 한글 오타)

트리거: 버퍼에 N개(min_keystrokes 이상) 문자 키가 쌓이고, 시간 윈도우 내에 있을 때.

```
1. 버퍼의 최근 N개 keycode → ASCII 문자로 변환 (KeyCode::to_char + shift)
2. is_english_keystrokes() — 모든 문자가 한글 자모에 매핑되는지 (기존 auto_typefix.rs 재활용)
3. 영어 사전에 없는지 확인 (기존 DICTIONARY 재활용)
4. 조건 충족 시 교정 트리거
```

**핵심 차이점**: 현재 구현은 "단어 경계(Space)" 시 word_buffer를 검사하지만, 새 접근법은 **매 키 입력마다** 최근 N개를 검사한다. 단어 경계 개념이 없어지므로 Enter 문제(문제 4)가 자동 해결된다.

**감지 시점 문제**: 매 키마다 검사하면 "gksrmf" 입력 중 3글자("gks")에서 이미 트리거될 수 있다.
- 해결: min_keystrokes를 충분히 크게 설정 (기본 4~5) + 사전 비포함 확인.
- 또는: 버퍼가 가득 찼을 때(max_keystrokes)만 검사. 하지만 이러면 5글자 한글 단어를 10글자 버퍼에서 놓칠 수 있음.
- **권장**: 매 키마다 검사하되, min_keystrokes 이상일 때만. 가장 긴 매칭을 우선.

### 3.2 방향 B (한글 모드에서 영어 오타)

한글 모드에서는 commit이 음절 단위로 발생하고 preedit이 존재한다.

```
1. 버퍼의 최근 N개 keycode → 영문 문자로 변환 (KeyCode::to_char + shift)
2. 조합하면 무엇이 되는지: korean_to_eng_full() 호출 불필요 — keycode에서 직접 영문 복원 가능!
3. 영어 사전에 있는지 확인
4. 조건 충족 시 교정 트리거
```

**핵심 이점**: keycode 기반이므로 한글 조합/분해 과정을 거칠 필요 없이, 물리적 키 자체로 영문 문자열을 복원할 수 있다. 이는 `korean_to_eng_full()`의 독립 자모 문제를 완전히 우회한다.

### 3.3 감지 신뢰도 고려

| 시나리오 | 방향 A | 방향 B |
|----------|--------|--------|
| "gksrmf" (한글) → 영어 사전 미포함 + 자모 매핑 O | 교정 O | N/A |
| "hello" (영어) → 영어 사전 포함 | 교정 X (정상) | N/A |
| ㅗ디ㅣㅐ (영어) → keycode → "hello" → 사전 포함 | N/A | 교정 O |
| "한글" (한글) → keycode → "gksrmf" → 사전 미포함 | N/A | 교정 X (정상) |

---

## 4. 교정 동작 분석

### 4.1 "지우고 → 모드 전환 → 재입력" 흐름

사용자가 제안한 "동일 키스트로크 재입력"은 세련된 접근이지만, **실제로는 더 단순한 방법이 가능하다.**

#### 방법 1: delete_surrounding_text + commit (현재 구현 패턴)

```
1. delete_surrounding_text(N) — 이미 입력된 텍스트 삭제
2. commit_text(corrected) — 교정된 텍스트 커밋
3. (선택) 모드 전환
```

이미 `EngineResponse.auto_typefix: Option<(u32, String)>` 필드가 존재하고, service.rs:728-744에서 시그널을 발행하는 코드가 있다.

**문제**: GNOME extension이 이 시그널을 수신하지 않음 (문제 2). 하지만 이것은 프론트엔드 수정으로 해결 가능.

#### 방법 2: ProcessKeyEvent 반환값에 포함 (문제 5 해결)

`EngineResponse`에 이미 `auto_typefix` 필드가 있다. 시그널 대신 **반환값으로 교정 결과를 전달**하면:

```
ProcessKeyEvent 반환: (consumed, preedit, commit, auto_typefix_delete, auto_typefix_text)
```

프론트엔드가:
1. commit을 먼저 처리 (원래 문자)
2. auto_typefix가 있으면: delete_surrounding_text → commit_text 순서로 교정

**하지만 이러면 원래 문자가 먼저 표시되고 즉시 삭제/교체되므로 깜빡임이 발생한다.**

#### 방법 3: commit을 교정 결과로 대체 (최적)

**가장 깔끔한 접근**: 교정이 필요하다고 판단되면, **원래 commit을 발행하지 않고**, 대신:

```
1. 이미 화면에 있는 이전 N-1개 문자를 delete_surrounding_text로 삭제
2. 교정된 전체 텍스트를 commit으로 반환
3. preedit은 비움
```

이 경우 반환값이: `(consumed=true, preedit="", commit="한글", delete_before=5)` 형태.

**이것이 사용자 제안("지우고 → 한영 바꾸고 → 재입력")의 본질을 구현하되, 실제로는 재입력 대신 이미 변환된 결과를 직접 commit하는 것.**

### 4.2 삭제할 문자 수 계산

- **방향 A** (영어→한글): N개 영문 키스트로크 → N개 ASCII 문자가 화면에 있음 → delete N개 → 한글 결과 commit
- **방향 B** (한글→영어): N개 키스트로크 → 한글 음절 M개가 화면에 있음 (M <= N) → delete M개 → 영문 결과 commit. **여기서 M을 정확히 계산해야 한다.** 이미 commit된 음절 수는 engine_worker의 commit 기록에서 추적 가능.

### 4.3 preedit(조합 중) 처리

한글 모드에서 마지막 키스트로크는 preedit 상태일 수 있다.

예: "ㅗ디ㅣ|ㅐ" (| = preedit 경계). 교정 시:
1. preedit을 먼저 clear (preedit="" 반환)
2. 이미 commit된 음절들을 delete_surrounding_text로 삭제
3. 교정 결과를 commit

**이것은 engine_worker 내에서 engine.press_key() 결과를 받은 후, 교정을 판단하고, 반환값을 조작하는 것으로 구현 가능하다.** 즉 press_key의 원래 결과(commit, preedit)를 교정 결과로 대체.

---

## 5. 현재 5가지 문제 해결 여부

| 문제 | 설명 | 해결 여부 | 근거 |
|------|------|----------|------|
| 1 | 빌드/설치 안 됨 | 동일 | 코드를 빌드/설치해야 하는 것은 어떤 접근법이든 마찬가지 |
| 2 | GNOME ext가 DeleteSurroundingText 시그널 미처리 | **해결 가능** | 방법 3(반환값에 포함)을 쓰면 시그널 불필요. 또는 시그널 핸들러 추가 (dbus_ime.js 수정) |
| 3 | 영어 모드 preferred_direct=true로 commit 미발생 | **오진 확인 — 문제 아님** | preferred_direct는 코드에서 사용되지 않음. 영어 모드에서도 ProcessKeyEvent 호출되고 commit 발생함 |
| 4 | Enter 키가 경계 문자를 커밋하지 않음 | **해결** | 단어 경계 개념 자체가 없어짐. 키스트로크 버퍼 기반이므로 Enter가 버퍼를 초기화할 뿐 |
| 5 | 시그널 vs 반환값 이중 커밋 | **해결 가능** | 방법 3: 교정 시 원래 commit을 대체. 또는 반환값 확장으로 프론트엔드가 한 번에 처리 |

---

## 6. 설정 구조 제안

```yaml
auto_typefix:
  enabled: true
  min_keystrokes: 4     # 최소 키스트로크 수 (3은 false positive 위험)
  max_keystrokes: 10    # 최대 키스트로크 수
  time_window_ms: 2000  # 시간 윈도우 (ms)
  direction_a: true     # 영→한 교정
  direction_b: true     # 한→영 교정
```

### 구현 위치

`src/config.rs`에 `AutoTypeFixConfig` 구조체 추가:

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AutoTypeFixConfig {
    pub enabled: bool,
    pub min_keystrokes: u8,
    pub max_keystrokes: u8,
    pub time_window_ms: u32,
    pub direction_a: bool,  // 영→한
    pub direction_b: bool,  // 한→영
}
```

기존 `config.engine.auto_typefix: bool`을 `config.engine.auto_typefix: AutoTypeFixConfig`로 확장.

---

## 7. 리스크 분석

### 7.1 False Positive (가장 큰 리스크)

**방향 A**: "gksrmf" 같은 명백한 케이스는 문제없지만, "fn", "src", "cfg" 같은 짧은 프로그래밍 약어가 한글 자모에 매핑되면서 영어 사전에 없으면 오교정된다.

- 완화: min_keystrokes를 4 이상으로. 2~3자는 검사하지 않음.
- 완화: 프로그래밍 약어 화이트리스트 (선택적).

**방향 B**: 한글 음절이 우연히 영어 사전 단어에 매핑되는 경우. 예: "디" → "ek" (사전에 없음, 안전). "나" → "sk" (사전에 없음, 안전). 대부분 안전하지만 긴 단어에서 우연 매칭 가능성 존재.

- 완화: 짧은 단어(4자 미만) 사전 매칭 제외.

### 7.2 깜빡임 (원문 표시 → 삭제 → 교정문 표시)

**방법 3(commit 대체) 사용 시**:
- 방향 A: 이전 N-1개 영문자는 이미 화면에 있고, N번째 키 처리 시 삭제+교정 발생. **N-1개 문자가 잠깐 보인 후 교정됨** → 약간의 깜빡임 불가피.
- 방향 B: 한글 음절이 잠깐 보인 후 영문으로 교체됨 → 깜빡임.

이는 모든 자동 교정 시스템의 본질적 한계. macOS/iOS의 자동 교정도 동일한 패턴.

### 7.3 preedit 상태에서의 처리

한글 모드에서 마지막 키스트로크가 preedit(조합 중)일 때:
- 교정 시 preedit을 clear하고 전체를 commit으로 대체해야 함.
- engine_worker에서 `engine.press_key()` 결과의 preedit/commit을 조작 가능.
- **위험**: engine 내부 상태(korean_context)와 불일치 발생 가능. 교정 후 engine을 reset해야 할 수 있음.

### 7.4 교정 후 커서 위치

`delete_surrounding_text` + `commit_text` 후 커서는 commit된 텍스트 끝에 위치한다.
- 영문 N자 삭제 → 한글 M자 commit → 커서는 한글 끝. 정상.
- 한글 M자 삭제 → 영문 N자 commit → 커서는 영문 끝. 정상.

### 7.5 되돌리기 (Ctrl+Z) 지원

현재 구현에 이미 `last_autofix` HashMap이 있다 (`engine_worker.rs:53-54`).
- Ctrl+Z 시 교정을 역방향으로 되돌림 (`engine_worker.rs:146-185`).
- 새 접근법에서도 동일 패턴 재활용 가능.
- **추가 고려**: 교정 후 모드를 전환했는지 추적하여, 되돌리기 시 모드도 복원해야 할 수 있음.

### 7.6 멀티바이트 삭제 정확성

`delete_surrounding_text`의 단위가 프론트엔드마다 다를 수 있음:
- GNOME extension: 문자(char) 단위
- GTK: 바이트 또는 문자 단위 (구현에 따라 다름)
- 한글 1음절 = 3바이트 UTF-8 = 1 char

각 프론트엔드에서 delete 단위를 확인하고 통일해야 함.

---

## 8. 구현 계획 (MVP)

### Phase 1: 키스트로크 버퍼 인프라 (engine_worker.rs만 수정)

1. `KeystrokeBuffer` 구조체 추가 (VecDeque 기반)
2. ProcessKey 핸들러에서 문자 키마다 버퍼에 추가
3. 비문자 키/모드 전환/포커스 변경 시 버퍼 초기화
4. 로그 출력으로 버퍼 동작 확인

### Phase 2: 방향 A 감지 (영어→한글, 가장 쉬움)

1. 버퍼 N개 이상 + 시간 윈도우 내 → ASCII 문자 복원
2. `is_english_keystrokes()` + 사전 비포함 확인 (기존 로직 재활용)
3. `eng_to_kor()` 변환 (기존 typefix 모듈 재활용)
4. 교정 결과를 EngineResponse에 포함

### Phase 3: GNOME extension 수정

1. `ProcessKeyEvent` 반환값 확장 또는 DeleteSurroundingText/CommitText 시그널 핸들러 추가
2. 교정 동작 처리 (delete + commit)

### Phase 4: 방향 B 감지 (한글→영어, preedit 처리 필요)

1. 버퍼 keycode → 영문 문자 복원
2. 사전 매칭
3. preedit clear + commit된 음절 삭제 + 영문 commit

### Phase 5: 설정 UI

1. config.rs에 AutoTypeFixConfig 구조체
2. unim-config CLI 지원
3. GNOME prefs / GUI 설정

---

## 9. 결론

**사용자 제안 접근법은 실현 가능하며, 현재 구현보다 근본적으로 우수하다.**

핵심 이점:
1. **keycode 기반**: 문자가 아닌 물리 키 기반이므로 preferred_direct, commit 유무에 의존하지 않음
2. **단어 경계 불필요**: 매 키마다 버퍼 검사하므로 Enter/Tab 문제 해소
3. **양방향 복원 가능**: keycode에서 영문/한글 양쪽 출력을 모두 복원 가능
4. **기존 코드 재활용**: `is_english_keystrokes()`, `eng_to_kor()`, `DICTIONARY`, `KeyCode::to_char()` 모두 그대로 사용
5. **시그널 의존 제거 가능**: 반환값 확장으로 프론트엔드 수정 최소화

**가장 큰 리스크는 false positive**이며, 이는 min_keystrokes 임계값과 사전 품질로 관리한다.

**추가 발견**: `preferred_direct` 설정은 코드에서 사용되지 않는 죽은 코드이다. 진단 문서의 "문제 3"은 사실이 아니었다.
