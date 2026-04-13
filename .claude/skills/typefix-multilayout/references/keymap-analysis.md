# AutoTypeFix 다중 키맵 — 코드 분석 결과

> 이 문서는 2026-04-14 조사 결과를 정리한 것이다. 구현 시 참조용.

## 1. 핵심 문제

`KeystrokeBuffer::to_ascii_string()` (auto_typefix.rs:107)이 `KeyCode::to_char()`를 호출하는데,
`to_char()`는 QWERTY 하드코딩이다. Dvorak 사용자가 물리적 S키를 누르면 `'o'`여야 하는데 `'s'`를 반환한다.

## 2. 이미 갖춰진 인프라

| 구성요소 | 상태 | 위치 |
|---------|------|------|
| EnglishLayout enum (5종) | 완비 | config.rs:110-120 |
| 영문 키맵 JSON (5개) | 완비 | src/keystroke/keymap/en_*.json |
| typefix::eng_to_kor() EnglishLayout 파라미터 | 완비 | typefix.rs:23 |
| typefix::kor_to_eng() EnglishLayout 파라미터 | 완비 | typefix.rs:61 |
| check_forward() english_layout 파라미터 | 완비 | auto_typefix.rs:154 |
| input_engine english_layout 필드 | 완비 | input_engine.rs |

## 3. 수정 대상 (4개 파일)

### A. keycode.rs — to_char_for_layout() 추가

현재 `to_char()` (263-312행)과 `to_shifted_char()` (324-390행)은 QWERTY 고정.
새 메서드 `to_char_for_layout(layout: EnglishLayout) -> Option<char>` 필요.

**구현 전략**: 영문 키맵 JSON의 행/열 인덱스와 KeyCode를 매핑하는 테이블 생성.
KeyCode는 물리 키 위치를 나타내므로, 각 레이아웃의 JSON에서 같은 위치의 문자를 반환.

물리키 위치 매핑 (QWERTY 기준 행/열):
- 1행: Grave,1,2,3,4,5,6,7,8,9,0,Minus,Equal,Backslash
- 2행: Q,W,E,R,T,Y,U,I,O,P,BracketLeft,BracketRight
- 3행: A,S,D,F,G,H,J,K,L,Semicolon,Apostrophe
- 4행: Z,X,C,V,B,N,M,Comma,Period,Slash

### B. auto_typefix.rs — to_ascii_string() 시그니처 변경

```rust
// 현재 (107행)
pub fn to_ascii_string(&self) -> String

// 변경
pub fn to_ascii_string(&self, english_layout: EnglishLayout) -> String
```

내부에서 `keycode.to_char()` 대신 `keycode.to_char_for_layout(english_layout)` 호출.

### C. auto_typefix.rs — check_reverse() 시그니처 변경

```rust
// 현재 (271행)
pub fn check_reverse(buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig) -> Option<AutoTypeFixResult>

// 변경
pub fn check_reverse(buffer: &KeystrokeBuffer, config: &AutoTypeFixConfig, english_layout: EnglishLayout) -> Option<AutoTypeFixResult>
```

### D. engine_worker.rs — check_reverse 호출 수정

```rust
// 현재 (267행)
auto_typefix::check_reverse(buf, atf_config)

// 변경
auto_typefix::check_reverse(buf, atf_config, config.engine.english.layout)
```

## 4. 영문 키맵 JSON 구조 (en_dvorak.json 기준)

```json
{
  "layout": {
    "lower": {
      "1st": ["`","1","2",...],     // 숫자행
      "2nd": ["'",",",".","p",...], // 상단 알파벳행
      "3nd": ["a","o","e","u",...], // 홈행
      "4th": [";","q","j","k",...}  // 하단 알파벳행
    },
    "upper": { ... }               // Shift 상태
  }
}
```

QWERTY에서 같은 위치의 문자와 1:1 대응:
- QWERTY 2행[0]='q' → Dvorak 2행[0]="'"
- QWERTY 3행[1]='s' → Dvorak 3행[1]='o'

## 5. 테스트 전략

### 검증 포인트
1. **QWERTY 회귀**: 기존 테스트 전부 통과 (to_char_for_layout(Qwerty) == to_char())
2. **Dvorak 순방향**: Dvorak 물리키 → 올바른 ASCII → eng_to_kor() 올바른 한글
3. **Dvorak 역방향**: 한글모드 물리키 → Dvorak ASCII → 영어 사전 매칭
4. **5개 레이아웃 교차**: 모든 레이아웃에서 동일 물리키 → 레이아웃 고유 문자
5. **Shift 조합**: 대문자/특수문자의 레이아웃별 차이

### 핵심 테스트 케이스 (Dvorak 예시)
- 물리키 G,K,S,R,M,F (QWERTY) → Dvorak에서 I,T,O,P,M,Y
- "gksrmf" → QWERTY: "한글", Dvorak: 다른 문자열
- Dvorak 물리키로 "한글" 입력: 해당 물리키 시퀀스가 다름
