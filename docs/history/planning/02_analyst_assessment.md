# 자동 실시간 한영 오타 수정 — 기술적 실현 가능성 평가

> 분석 일자: 2026-04-05
> 대상: UNIM 코드베이스 (develop 브랜치, 43fbb43)

---

## 1. 키 처리 파이프라인 분석

### 현재 흐름

```
Frontend (GTK/Qt/GNOME/XIM)
  → DBus ProcessKey(hardware_code, modifier)
    → engine_worker: EngineRequest::ProcessKey
      → InputEngine::press_key_code(hardware_code, modifier, config)
        → KeyCode::from_evdev_keycode(hardware_code)
        → press_key(keycode, modifier, config)
          ├─ 수정자 키만 → not_consumed
          ├─ 팝업 활성 → process_popup_key()
          ├─ Ctrl/Alt/Super → flush & not_consumed
          ├─ 한/영 전환키 → toggle_input_category()
          └─ 입력 카테고리 분기:
              ├─ Korean → process_korean_key(keycode, modifier)
              └─ English → process_english_key(keycode, modifier)
```

### 삽입 지점 후보

**지점 A: `press_key()` 반환 직후 (engine_worker 내)**
- 위치: `unim-dbus/src/engine_worker.rs` L99~ `EngineRequest::ProcessKey` 핸들러
- engine_worker에서 `engine.press_key_code()` 호출 후, commit이 발생한 시점에서 "방금 커밋된 단어"를 검사할 수 있음
- 장점: Core 코드 변경 없이 데몬 레벨에서 후처리 가능
- surrounding_text가 이미 engine에 저장되어 있으므로 접근 가능

**지점 B: `process_korean_key()` / `process_english_key()` 내부**
- 한글 모드에서 키를 눌렀는데 자모 매핑에 실패하는 패턴 감지
- 영어 모드에서 키를 눌렀는데 한글 자모 패턴이 형성되는 것 감지
- 장점: 가장 빠른 감지
- 단점: Core 복잡도 증가, 매 키스트로크마다 판단 비용

**지점 C: commit 발생 시점 (공백/구두점 입력 시)**
- `process_korean_key()`에서 `KeyCode::Space` 처리 시 flush_preedit() 직전/직후
- 이 시점에서 "방금 완성된 단어"를 검사 → 가장 자연스러운 단어 경계
- `process_english_key()`에서도 Space 입력 시 동일하게 검사 가능

### 한글 조합기 내부 상태 활용

`HangulInputContext`는 `process_jamo()`로 자모를 받아 조합하며, 내부에 `composing` 상태와 `committed` 버퍼를 유지한다. 핵심 관찰:

- 한글 모드에서 영문을 타이핑하면 → `keyboard_map`에서 자모 매핑을 시도하는데, QWERTY 기준으로 모든 알파벳 키가 자모에 매핑되므로 **한글 모드에서 영문 의도를 자모 수준에서 감지하는 것은 원리적으로 불가능** (모든 키가 valid한 자모를 생성)
- 영어 모드에서 한글을 타이핑하면 → 영문 문자가 그대로 커밋됨 → surrounding text에서 `is_english_keystrokes()` 패턴 감지 가능

**결론: 자모 수준 실시간 감지는 한글→영문 오타만 가능하고, 영문→한글 오타는 커밋된 텍스트(단어) 수준에서만 감지 가능.**

### 키스트로크 역방향 매핑

`src/keystroke/korean_to_keystrokes.rs`의 `korean_to_keystrokes()` 함수와 `KeyboardMap`은 이미 양방향 변환을 지원:
- 영문 키 → 한글 자모: `keyboard_map.get(&c)` (HashMap<char, JamoEnum>)
- 한글 → 영문 키: `korean_to_keystrokes(text, &keyboard_map, is_three_bul)`

이 인프라는 `typefix.rs`의 `eng_to_kor()`, `kor_to_eng()`에서 이미 사용 중이며, 실시간 버전에서도 그대로 재활용 가능.

---

## 2. Surrounding Text 인프라 현황

### 호출 지점과 빈도

| 프론트엔드 | 호출 시점 | 선택 영역 지원 |
|-----------|----------|--------------|
| GTK3 | `set_surrounding()` 콜백 (GTK가 호출) | cursor만 (anchor=cursor) |
| GTK4 | `set_surrounding_with_selection()` 콜백 | cursor + selection_index |
| Qt5/6 | `QInputMethodQueryEvent` → `ImSurroundingText` | cursor + anchor |
| GNOME Extension | `vfunc_set_surrounding(text, cursor, anchor)` | cursor + anchor |
| Wayland | `zwp_input_method_v2::Event::SurroundingText` | 미구현 (주석만) |
| XIM | 미지원 | - |

모든 프론트엔드에서 DBus `SetSurroundingText(text, cursor, anchor)` → engine_worker → `engine.set_surrounding_text()` 경로로 전달됨.

### 데이터 신뢰성

- **GTK3/4, Qt5/6**: GTK/Qt 프레임워크가 주기적으로 `set_surrounding` 콜백을 호출. 일반적으로 **키 입력마다** 업데이트됨 (앱에 따라 다를 수 있음)
- **GNOME Extension**: Mutter가 `vfunc_set_surrounding`을 호출. 업데이트 주기는 포커스된 앱에 의존
- **제한**: surrounding text는 항상 최신이 아닐 수 있음. 특히 커밋 직후 프론트엔드가 새 surrounding text를 보내기 전까지 **이전 상태**가 남아있을 수 있음

### 텍스트 교체 메커니즘 (이미 구현됨!)

`delete_surrounding_text` + `commit_text` 패턴이 이미 두 곳에서 사용 중:

1. **SmartBackspace** (`service.rs` L1010):
   ```rust
   Self::delete_surrounding_text(&signal_ctx, -(delete_chars as i32), delete_chars).await.ok();
   Self::commit_text(&signal_ctx, &replacement).await.ok();
   ```

2. **GNOME Extension TypeFix** (`extension.js` L566):
   ```javascript
   this._inputMethod.deleteSurrounding(deleteCount);
   this._inputMethod.commitText(replacement);
   ```

**이 패턴을 자동 교정에 그대로 재사용할 수 있다.** 핵심 인프라가 이미 완성되어 있음.

---

## 3. 기존 TypeFix 코드 재활용 분석

### 현재 함수들

| 함수 | 용도 | 실시간 재사용 |
|------|------|-------------|
| `eng_to_kor(text, ko_layout, en_layout)` | 영문→한글 변환 | **그대로 사용 가능** |
| `kor_to_eng(text, ko_layout, en_layout)` | 한글→영문 변환 | **그대로 사용 가능** |
| `is_korean_text(text)` | 한글 여부 판별 | **그대로 사용 가능** |
| `is_english_keystrokes(text, keyboard_map)` | 영문 키스트로크 판별 | **그대로 사용 가능** |

### typefix_convert() 패턴과 실시간 버전 차이

현재 `typefix_convert()`는:
- **전제조건**: `cursor != anchor` (선택 영역 필수)
- **동작**: 선택된 텍스트를 변환하고 입력 모드를 자동 전환
- **트리거**: 사용자가 수동으로 단축키를 눌러야 함

실시간 버전에서 필요한 변경:
- **전제조건 변경**: 선택 영역 대신 "마지막 단어" (커서 앞의 공백/구두점까지)를 자동 추출
- **감지 로직 추가**: 단어가 오타인지 판단하는 휴리스틱
- **트리거 변경**: 공백/구두점 입력 시 자동 실행

### 실시간 버전 함수 설계안

```rust
/// 자동 오타 감지 및 교정
/// 커서 앞의 마지막 단어를 분석하여 한영 오타를 감지합니다.
///
/// Returns: Some((delete_chars, replacement)) 또는 None
pub fn auto_typefix(&self) -> Option<(u32, String)> {
    // 1. surrounding_text에서 커서 앞 마지막 단어 추출
    // 2. is_korean_text() / is_english_keystrokes()로 언어 판별
    // 3. 현재 input_category와 불일치하면 오타로 판단
    // 4. eng_to_kor() 또는 kor_to_eng()로 변환
    // 5. (delete_chars, replacement) 반환
}
```

---

## 4. 아키텍처 설계 후보 평가

### 방안 A: Core 엔진 내부 감지

**구현 위치**: `src/input_engine.rs` — `press_key()` 또는 `process_korean_key()`/`process_english_key()` 내부

**구현 방법**:
1. `process_english_key()`에서 Space/구두점 입력 시:
   - surrounding_text에서 마지막 단어 추출
   - `is_english_keystrokes()`로 한글 자모 패턴 확인
   - 매칭되면 `eng_to_kor()`로 변환
   - `InputResult`에 새 플래그 `auto_correction: Option<(u32, String)>` 추가
2. `process_korean_key()`에서 flush 후:
   - 방금 커밋된 한글을 `kor_to_eng()`로 역변환
   - 결과가 유효한 영단어인지 사전으로 검증 (추가 인프라 필요)

**평가**:
| 항목 | 점수 |
|------|------|
| 레이턴시 | ★★★★★ (0ms 추가, 키 처리 내에서 동기 실행) |
| 프론트엔드 호환성 | ★★★★★ (모든 프론트엔드에서 동작) |
| 구현 복잡도 | ★★★☆☆ (InputResult 확장, Core 변경 필요) |
| 유지보수 | ★★★☆☆ (Core에 교정 로직이 섞임) |
| 정확도 | ★★☆☆☆ (사전 없이는 영문→한글 오타만 감지 가능) |

**치명적 제한**: 한글 모드에서 영문 의도를 감지하려면 **영어 사전**이 필요. 현재 UNIM에는 영어 사전이 없음. 반면 영어 모드에서 한글 의도 감지는 `is_english_keystrokes()` 패턴만으로 높은 정확도 가능.

### 방안 B: DBus 데몬 감지

**구현 위치**: `unim-dbus/src/engine_worker.rs` — `EngineRequest::ProcessKey` 처리 후

**구현 방법**:
1. engine_worker에서 `press_key_code()` 호출 후 결과 검사
2. commit이 발생했고 + 트리거 조건(Space/구두점)이면:
   - engine의 surrounding_text + 방금 커밋한 텍스트로 마지막 단어 추출
   - auto_typefix 로직 실행
   - 필요시 `delete_surrounding_text` + `commit_text` 시그널 발행
3. 기존 `EngineResponse`에 `auto_correction` 필드 추가

**평가**:
| 항목 | 점수 |
|------|------|
| 레이턴시 | ★★★★☆ (~1ms 추가, 같은 요청 내에서 처리) |
| 프론트엔드 호환성 | ★★★★★ (모든 프론트엔드에서 동작) |
| 구현 복잡도 | ★★★★☆ (Core 분리 유지, engine_worker만 수정) |
| 유지보수 | ★★★★☆ (교정 로직이 데몬에 격리됨) |
| 정확도 | ★★☆☆☆ (방안 A와 동일한 한계) |

**장점**: Core의 `typefix.rs` 함수들을 호출만 하면 되고, `delete_surrounding_text` + `commit_text` 시그널 발행 패턴이 이미 SmartBackspace에 구현되어 있어 복붙 수준으로 재활용 가능.

### 방안 C: GNOME Extension 감지

**구현 위치**: `unim-gnome-extension/extension.js` 또는 `unim_input_method.js`

**구현 방법**:
1. `commitText` 시그널 수신 후 타이머 설정 (debounce)
2. surrounding text에서 마지막 단어 추출
3. JS에서 한영 변환 로직 구현 (또는 DBus로 데몬에 감지 요청)
4. `deleteSurrounding()` + `commitText()`로 교체

**평가**:
| 항목 | 점수 |
|------|------|
| 레이턴시 | ★★☆☆☆ (JS 실행 + DBus 왕복 가능) |
| 프론트엔드 호환성 | ★☆☆☆☆ (GNOME Wayland 전용) |
| 구현 복잡도 | ★★★★★ (Core/DBus 변경 없음) |
| 유지보수 | ★★★☆☆ (JS에 교정 로직 중복) |
| 정확도 | ★★☆☆☆ (동일한 한계) |

**결정적 단점**: GNOME 전용. GTK3/4 IM 모듈이나 Qt 환경에서는 동작하지 않음.

### 종합 추천: **방안 B (DBus 데몬 감지)를 주력으로 추천**

이유:
1. Core 순수성 유지 (아키텍처 원칙 준수)
2. SmartBackspace의 `delete_surrounding_text` + `commit_text` 패턴을 그대로 복제
3. 모든 프론트엔드에서 동작
4. `typefix.rs` 함수들을 import만 하면 됨 (engine_worker는 이미 `unim` 크레이트 의존)
5. 레이턴시가 사실상 방안 A와 동일 (같은 ProcessKey 요청 내에서 처리)

---

## 5. MVP 정의

### 최소 동작 범위 (Phase 1)

**"영어 모드에서 한글을 타이핑한 경우"만 자동 교정**

이유:
- 영어 모드에서 "gksrmf" 같은 한글 자모 패턴은 **사전 없이도** `is_english_keystrokes()`로 높은 정확도로 감지 가능
- 한글 모드에서 영문을 타이핑한 경우는 모든 키가 유효한 자모를 생성하므로 **사전 없이는 감지 불가능** → Phase 2로 미룸

### 감지 시점

**공백 입력 시 이전 단어 검사** (Word-boundary trigger)

이유:
- 매 키스트로크 감지는 불필요한 연산과 잦은 텍스트 치환으로 UX 저하
- 공백은 자연스러운 단어 경계이며, 사용자가 "단어 완성"을 의도한 시점
- 구두점(`.`, `,`, `!`, `?`)도 트리거에 포함

### MVP 구현 계획

```
[Phase 1 — 영어모드 한글오타 자동교정]

1. src/input_engine.rs:
   - auto_typefix_on_commit() 메서드 추가
   - surrounding_text에서 커서 앞 마지막 단어 추출
   - is_english_keystrokes()로 한글 자모 패턴 감지
   - eng_to_kor()로 변환, (delete_chars, replacement) 반환

2. unim-dbus/src/engine_worker.rs:
   - ProcessKey 처리 후, Space/구두점 커밋 시:
     - auto_typefix_on_commit() 호출
     - 결과가 있으면 EngineResponse에 auto_correction 필드 포함

3. unim-dbus/src/service.rs:
   - ProcessKeyEvent 응답에 auto_correction 정보 포함
   - 또는 별도 시그널 AutoCorrection(delete_chars, replacement) 발행
   - delete_surrounding_text + commit_text로 텍스트 교체

4. src/config.rs:
   - auto_typefix: AutoTypefixConfig { enabled: bool, trigger: WordBoundary/EveryKey }
   - 설정 파일에서 on/off 가능

5. 각 프론트엔드:
   - GTK3/4: delete-surrounding 시그널 + commit 시그널 수신 (이미 처리됨)
   - Qt5/6: deleteSurroundingText + commitString (이미 처리됨)
   - GNOME: deleteSurrounding + commitText (이미 처리됨)
```

### MVP 구현 공수 추정

| 작업 | 예상 공수 |
|------|----------|
| `auto_typefix_on_commit()` 구현 | 2시간 |
| engine_worker 후처리 로직 | 2시간 |
| DBus 시그널/응답 확장 | 2시간 |
| config.rs 설정 추가 | 1시간 |
| 단위 테스트 | 2시간 |
| 통합 테스트 (GTK4 + GNOME) | 3시간 |
| **합계** | **~12시간** |

### 핵심 리스크

1. **Surrounding text 타이밍**: 커밋 직후 surrounding text가 아직 업데이트되지 않았을 수 있음 → 해결: 커밋 전 surrounding text + 방금 입력한 키를 조합하여 판단
2. **오탐지 (False Positive)**: "abc" 같은 영문 약어가 한글 자모 패턴과 일치할 수 있음 → 해결: 최소 길이 임계값 (3글자 이상) + 자음/모음 교대 패턴 검증
3. **되돌리기(Undo)**: 자동 교정이 틀렸을 때 Ctrl+Z로 복원 → IME 레벨에서 Undo 지원은 앱에 의존 → 해결: 자동 교정 직후 단축키로 원래 텍스트 복원 기능 (Phase 2)

---

## 부록: 핵심 코드 참조

| 파일 | 핵심 함수/구조체 | 역할 |
|------|-----------------|------|
| `src/input_engine.rs` | `InputEngine::press_key()` | 키 처리 진입점 |
| `src/input_engine.rs` | `InputEngine::typefix_convert()` | 수동 TypeFix (재활용 대상) |
| `src/input_engine.rs` | `InputEngine::smart_backspace()` | delete+commit 패턴 원본 |
| `src/typefix.rs` | `eng_to_kor()`, `kor_to_eng()` | 변환 함수 (그대로 재사용) |
| `src/typefix.rs` | `is_english_keystrokes()`, `is_korean_text()` | 감지 함수 (그대로 재사용) |
| `unim-dbus/src/engine_worker.rs` | `EngineRequest::ProcessKey` 핸들러 | 삽입 지점 (방안 B) |
| `unim-dbus/src/service.rs` | `delete_surrounding_text()` 시그널 | 텍스트 삭제 (이미 구현) |
| `unim-dbus/src/service.rs` | `commit_text()` 시그널 | 텍스트 커밋 (이미 구현) |
