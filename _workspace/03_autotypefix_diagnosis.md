# AutoTypeFix 미작동 원인 진단 보고서

## 결론 요약

AutoTypeFix가 작동하지 않는 원인은 **최소 3개의 독립적인 문제**가 겹쳐 있다.

---

## 문제 1: 설치되지 않은 바이너리 (치명적)

- 현재 실행 중인 데몬: `/usr/libexec/unim-daemon` (시스템 설치 버전)
- `target/release/unim-daemon`: **존재하지 않음** (빌드 안 됨)
- `src/auto_typefix.rs`, `src/lib.rs` 변경, `unim-dbus/src/engine_worker.rs`, `unim-dbus/src/service.rs` 변경이 모두 **unstaged** 상태 (git status 기준 ` M`)
- **결론**: AutoTypeFix 코드가 빌드도, 설치도 되지 않았다. 실행 중인 데몬은 이 기능이 없는 구버전이다.

**해결**: `make build && sudo make install PREFIX=/usr` 후 데몬 재시작.

---

## 문제 2: GNOME Extension이 delete_surrounding_text 시그널을 수신하지 않음 (치명적)

### 서버 측 (service.rs)
AutoTypeFix 교정 결과가 있으면 **DBus 시그널 2개를 발행**:
1. `delete_surrounding_text(offset, n_chars)` -- 원문 삭제
2. `commit_text(replacement)` -- 교정문 커밋

이 시그널은 `ProcessKeyEvent` 메서드의 **반환값 이후에** 비동기로 발행된다 (line 729-744).

### 클라이언트 측 (dbus_ime.js)
`_handleContextSignal()`에서 처리하는 시그널 목록:
- `ShowHanjaPopup` -- O
- `ShowSpecialPopup` -- O
- `HidePopup` -- O
- `PopupNavigate` -- O
- **`DeleteSurroundingText` -- X (처리 없음)**
- **`CommitText` -- X (처리 없음)**

GNOME extension의 `_handleContextSignal()`에 `DeleteSurroundingText`와 `CommitText` 시그널 핸들러가 **구현되어 있지 않다**.

### 타이밍 문제
`processKey()`는 `call_sync()`로 호출된다. 반환값 `(consumed, preedit, commit)`을 받은 후 key_handler.js가:
1. `commitText(commit)` -- 원래 커밋 텍스트 (예: "gksrmf ")를 먼저 커밋
2. `updatePreedit(preedit)` -- preedit 업데이트
3. `notify_key_event(event, consumed)` -- 키 이벤트 전달

그 **후에** 서버 측에서 `delete_surrounding_text` + `commit_text` 시그널이 도착하는데, 수신 핸들러가 없으므로 무시된다.

더 나아가, **원래 커밋과 교정 커밋이 모두 발생**하는 구조적 문제가 있다:
- `ProcessKeyEvent` 반환값의 `commit`에 원래 텍스트가 이미 포함되어 있고
- AutoTypeFix가 이를 다시 삭제하고 교정문을 커밋하려 함
- 즉, "gksrmf " 커밋 -> delete 6+1자 -> "한글 " 커밋의 3단계인데, 1단계에서 이미 원문이 입력됨

---

## 문제 3: 한글 모드에서의 word_buffer 축적 문제 (잠재적)

### Space 키 처리 흐름 (한글 모드)
`input_engine.rs` line 438-443:
```
Space -> flush_preedit() -> commit_buffer에 조합문자 추가
      -> commit_buffer.push(' ')
      -> InputResult::committed()
```

한 번의 `press_key()` 호출로 `commit_str()`에 **"가 "** (조합문자+공백)이 한꺼번에 들어온다.

### Enter 키 처리 흐름 (한글 모드)
`input_engine.rs` line 408-413:
```
Enter -> flush_preedit() -> commit_buffer에 조합문자 추가
      -> InputResult::committed_passthrough()  (consumed=false)
```

Enter는 **조합문자만 커밋하고 Enter 자체는 통과** (not consumed). Enter 문자('\n')는 commit에 포함되지 않으므로 **word_buffer에 단어 경계 문자가 들어오지 않는다**.

### word_buffer 분석

engine_worker.rs line 243-317의 로직:
1. `commit_str`이 있으면 `word_buffer`에 push
2. 마지막 문자가 `is_word_boundary()`이면 단어 추출 -> `detect_and_correct()` 호출

**Space의 경우**: "가 " -> word_buffer = "가 " -> 마지막 문자 ' '이 경계 -> word_part = "가" -> detect_and_correct("가", Korean, ...) 호출. 그러나 1글자이므로 `word.chars().count() < 2` 체크에서 **None 반환**.

이것은 **한글은 1음절이 의미있는 단어일 수 있음에도** 2자 미만은 무시하는 로직 문제다. 다만 역방향 (한글->영어)에서 1자 한글이 영어 사전에 매칭될 확률은 거의 없으므로 실질적 영향은 낮다.

**Enter의 경우**: 조합문자만 커밋되고 '\n'이 없으므로 단어 경계를 감지할 수 없다. word_buffer에 축적만 되고 교정 트리거가 발생하지 않는다.

### 영어 모드에서의 흐름
영어 모드에서는 `preferred_direct: true` 설정이므로 대부분의 키가 **consumed=false로 통과**된다. 즉 `commit_str`이 비어 있고, word_buffer에 아무것도 축적되지 않는다.

이것이 **순방향 (영어모드에서 한글 오타)가 절대로 작동할 수 없는 이유**다. 영어 모드에서 `preferred_direct: true`이면 키가 엔진을 거치지 않고 직접 앱으로 전달되므로 commit이 발생하지 않는다.

---

## 문제 4: current_mode 판별 오류 가능성 (잠재적)

engine_worker.rs line 268-269:
```rust
auto_typefix::detect_and_correct(
    &word_part,
    current_mode,  // <-- engine.input_category()
    ...
)
```

`current_mode`는 키 처리 **후의** 모드다. 만약 사용자가 한/영 전환 키를 누른 후 첫 단어 경계에서 교정을 시도하면, word_buffer에는 이전 모드의 텍스트가 있지만 `current_mode`는 새 모드를 가리킨다. 모드 전환 시 word_buffer를 clear하는 로직(line 198)이 있어 이 경우는 보호되지만, edge case 주의 필요.

---

## 문제 5: 로그에 AutoTypeFix 흔적 없음

`~/.unim-errors.log`에 AutoTypeFix 관련 로그가 전혀 없다. 이는 문제 1(코드가 설치 안 됨)과 일치한다.

---

## 우선순위 정리

| 순위 | 문제 | 심각도 | 해결 난이도 |
|------|------|--------|-------------|
| 1 | 빌드/설치 안 됨 | 치명적 | 낮음 (make build && sudo make install) |
| 2 | GNOME ext가 DeleteSurroundingText/CommitText 시그널 미처리 | 치명적 | 중간 (dbus_ime.js 수정) |
| 3 | 영어 모드 preferred_direct=true로 commit 미발생 | 치명적 (순방향 불가) | 높음 (아키텍처 재설계) |
| 4 | Enter 키가 경계 문자를 커밋하지 않음 | 중간 | 낮음 |
| 5 | 시그널 vs 반환값 이중 커밋 문제 | 중간 | 중간 (반환값에 포함 vs 시그널 선택) |

---

## 아키텍처 제안

### 현재 문제의 근본 원인
AutoTypeFix는 **commit 텍스트를 word_buffer에 축적**하는 방식인데, IME의 commit 흐름이 이에 적합하지 않다:
- 영어 모드: `preferred_direct=true` -> 엔진을 거치지 않음 -> commit 없음
- 한글 모드: 단어 단위가 아닌 음절+경계 단위로 commit -> 축적은 되지만 교정 시 이미 원문이 앱에 입력된 상태

### 대안
1. **ProcessKeyEvent 반환값에 auto_typefix를 포함** (시그널 대신): `(consumed, preedit, commit, delete_count, replacement)` 형태로 확장. 프론트엔드가 commit 전에 교정을 적용.
2. **Surrounding text 기반**: 단어 경계 시 surrounding text에서 직전 단어를 읽어 교정. 영어 모드에서도 작동 가능.
3. **preferred_direct=false 강제**: 영어 모드에서도 엔진이 키를 처리하도록 변경. 성능/호환성 영향 있음.
