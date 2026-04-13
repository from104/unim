# AutoTypeFix 다중 영문 키맵 구현 계획

## 목표

AutoTypeFix의 QWERTY 하드코딩을 제거하여 Dvorak/Colemak/ColemakDh/Workman 레이아웃에서도 순방향/역방향 오타 교정이 정확하게 동작하도록 한다.

## 결정 사항 (확정)

- **구현 방식**: const lookup table (매 키스트로크마다 호출되므로 성능 최우선)
- **Qwerty 최적화**: `EnglishLayout::Qwerty`인 경우 기존 `to_char()`/`to_shifted_char()` 직접 호출 (성능 영향 제로)
- 기존 `to_char()`/`to_shifted_char()` 메서드는 삭제하지 않음 (다른 곳에서 사용)

---

## 변경 파일 (4개)

### 1. `src/keycode.rs` — 레이아웃별 문자 변환 추가

**추가 위치**: `to_shifted_char()` 메서드 뒤 (374행 이후, `is_character_key()` 앞)

#### 1-A. const lookup table 정의 (모듈 레벨)

파일 하단 `#[cfg(test)]` 앞에 추가. 각 테이블은 4개 행으로 구성.

```rust
use crate::config::EnglishLayout;

/// 레이아웃별 물리키→문자 매핑 테이블.
/// 각 행은 (lower, upper) char 쌍의 배열.
/// 행 순서: 0=숫자행(14키), 1=상단알파벳(12키), 2=홈행(11키), 3=하단알파벳(10키)
type LayoutRow = &'static [(char, char)];
type LayoutTable = [LayoutRow; 4];

const DVORAK_TABLE: LayoutTable = [
    // Row 0 (14): Backquote,Num1..Num0,Minus,Equal,Backslash
    &[('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('[','{'),(']','}'),('\\','|')],
    // Row 1 (12): Q,W,E,R,T,Y,U,I,O,P,BracketLeft,BracketRight
    &[('\'','"'),(',','<'),('.','>'),('p','P'),('y','Y'),('f','F'),('g','G'),('c','C'),('r','R'),('l','L'),('/','?'),('=','+')],
    // Row 2 (11): A,S,D,F,G,H,J,K,L,Semicolon,Quote
    &[('a','A'),('o','O'),('e','E'),('u','U'),('i','I'),('d','D'),('h','H'),('t','T'),('n','N'),('s','S'),('-','_')],
    // Row 3 (10): Z,X,C,V,B,N,M,Comma,Period,Slash
    &[(';',':'),('q','Q'),('j','J'),('k','K'),('x','X'),('b','B'),('m','M'),('w','W'),('v','V'),('z','Z')],
];

const COLEMAK_TABLE: LayoutTable = [
    &[('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')],
    &[('q','Q'),('w','W'),('f','F'),('p','P'),('g','G'),('j','J'),('l','L'),('u','U'),('y','Y'),(';',':'),('[','{'),(']','}')],
    &[('a','A'),('r','R'),('s','S'),('t','T'),('d','D'),('h','H'),('n','N'),('e','E'),('i','I'),('o','O'),('\'','"')],
    &[('z','Z'),('x','X'),('c','C'),('v','V'),('b','B'),('k','K'),('m','M'),(',','<'),('.','>'),('/','?')],
];

const COLEMAK_DH_TABLE: LayoutTable = [
    &[('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')],
    &[('q','Q'),('w','W'),('f','F'),('p','P'),('b','B'),('j','J'),('l','L'),('u','U'),('y','Y'),(';',':'),('[','{'),(']','}')],
    &[('a','A'),('r','R'),('s','S'),('t','T'),('g','G'),('m','M'),('n','N'),('e','E'),('i','I'),('o','O'),('\'','"')],
    &[('z','Z'),('x','X'),('c','C'),('d','D'),('v','V'),('k','K'),('h','H'),(',','<'),('.','>'),('/','?')],
];

const WORKMAN_TABLE: LayoutTable = [
    &[('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')],
    &[('q','Q'),('d','D'),('r','R'),('w','W'),('b','B'),('j','J'),('f','F'),('u','U'),('p','P'),(';',':'),('[','{'),(']','}')],
    &[('a','A'),('s','S'),('h','H'),('t','T'),('g','G'),('y','Y'),('n','N'),('e','E'),('o','O'),('i','I'),('\'','"')],
    &[('z','Z'),('x','X'),('m','M'),('c','C'),('v','V'),('k','K'),('l','L'),(',','<'),('.','>'),('/','?')],
];
```

#### 1-B. `physical_position()` 메서드 추가

`impl KeyCode` 블록 내, `to_shifted_char()` 뒤에 추가.

```rust
/// 물리키를 (행, 열) 인덱스로 변환. 레이아웃 테이블 lookup용.
fn physical_position(&self) -> Option<(usize, usize)> {
    match self {
        // Row 0: 숫자행 (14키)
        KeyCode::Backquote => Some((0, 0)),
        KeyCode::Num1 => Some((0, 1)),
        KeyCode::Num2 => Some((0, 2)),
        KeyCode::Num3 => Some((0, 3)),
        KeyCode::Num4 => Some((0, 4)),
        KeyCode::Num5 => Some((0, 5)),
        KeyCode::Num6 => Some((0, 6)),
        KeyCode::Num7 => Some((0, 7)),
        KeyCode::Num8 => Some((0, 8)),
        KeyCode::Num9 => Some((0, 9)),
        KeyCode::Num0 => Some((0, 10)),
        KeyCode::Minus => Some((0, 11)),
        KeyCode::Equal => Some((0, 12)),
        KeyCode::Backslash => Some((0, 13)),
        // Row 1: 상단 알파벳 (12키)
        KeyCode::Q => Some((1, 0)),
        KeyCode::W => Some((1, 1)),
        KeyCode::E => Some((1, 2)),
        KeyCode::R => Some((1, 3)),
        KeyCode::T => Some((1, 4)),
        KeyCode::Y => Some((1, 5)),
        KeyCode::U => Some((1, 6)),
        KeyCode::I => Some((1, 7)),
        KeyCode::O => Some((1, 8)),
        KeyCode::P => Some((1, 9)),
        KeyCode::BracketLeft => Some((1, 10)),
        KeyCode::BracketRight => Some((1, 11)),
        // Row 2: 홈행 (11키)
        KeyCode::A => Some((2, 0)),
        KeyCode::S => Some((2, 1)),
        KeyCode::D => Some((2, 2)),
        KeyCode::F => Some((2, 3)),
        KeyCode::G => Some((2, 4)),
        KeyCode::H => Some((2, 5)),
        KeyCode::J => Some((2, 6)),
        KeyCode::K => Some((2, 7)),
        KeyCode::L => Some((2, 8)),
        KeyCode::Semicolon => Some((2, 9)),
        KeyCode::Quote => Some((2, 10)),
        // Row 3: 하단 알파벳 (10키)
        KeyCode::Z => Some((3, 0)),
        KeyCode::X => Some((3, 1)),
        KeyCode::C => Some((3, 2)),
        KeyCode::V => Some((3, 3)),
        KeyCode::B => Some((3, 4)),
        KeyCode::N => Some((3, 5)),
        KeyCode::M => Some((3, 6)),
        KeyCode::Comma => Some((3, 7)),
        KeyCode::Period => Some((3, 8)),
        KeyCode::Slash => Some((3, 9)),
        // Space는 별도 처리 (모든 레이아웃 동일)
        _ => None,
    }
}
```

#### 1-C. `to_char_for_layout()` 공개 메서드 추가

```rust
/// 지정된 영문 레이아웃에서 이 물리키가 생성하는 문자를 반환한다.
///
/// Qwerty인 경우 기존 to_char()/to_shifted_char()를 직접 호출하여 성능 영향 제로.
pub fn to_char_for_layout(&self, layout: EnglishLayout, shifted: bool) -> Option<char> {
    // Qwerty 최적화: 가장 흔한 경우, 기존 메서드 재사용
    if layout == EnglishLayout::Qwerty {
        return if shifted { self.to_shifted_char() } else { self.to_char() };
    }

    // Space는 모든 레이아웃에서 동일
    if *self == KeyCode::Space {
        return Some(' ');
    }

    let (row, col) = self.physical_position()?;
    let table: &LayoutTable = match layout {
        EnglishLayout::Dvorak => &DVORAK_TABLE,
        EnglishLayout::Colemak => &COLEMAK_TABLE,
        EnglishLayout::ColemakDh => &COLEMAK_DH_TABLE,
        EnglishLayout::Workman => &WORKMAN_TABLE,
        EnglishLayout::Qwerty => unreachable!(),
    };
    let row_data = table[row];
    if col >= row_data.len() {
        return None;
    }
    let (lower, upper) = row_data[col];
    Some(if shifted { upper } else { lower })
}
```

#### 1-D. 테스트 추가 (`#[cfg(test)]` 모듈 내, 기존 `test_keycode_to_char` 뒤)

```rust
#[test]
fn test_to_char_for_layout_qwerty_consistency() {
    // Qwerty 레이아웃은 기존 to_char()/to_shifted_char()와 동일해야 함
    let keys = [
        KeyCode::A, KeyCode::B, KeyCode::Q, KeyCode::Z,
        KeyCode::Num1, KeyCode::Semicolon, KeyCode::Slash,
        KeyCode::Backquote, KeyCode::Minus, KeyCode::Equal,
    ];
    for key in &keys {
        assert_eq!(
            key.to_char_for_layout(EnglishLayout::Qwerty, false),
            key.to_char(),
            "Qwerty lower mismatch for {:?}", key
        );
        assert_eq!(
            key.to_char_for_layout(EnglishLayout::Qwerty, true),
            key.to_shifted_char(),
            "Qwerty upper mismatch for {:?}", key
        );
    }
}

#[test]
fn test_to_char_for_layout_dvorak() {
    // Dvorak: 물리키 S → 'o', 물리키 Minus → '['
    assert_eq!(KeyCode::S.to_char_for_layout(EnglishLayout::Dvorak, false), Some('o'));
    assert_eq!(KeyCode::S.to_char_for_layout(EnglishLayout::Dvorak, true), Some('O'));
    assert_eq!(KeyCode::Minus.to_char_for_layout(EnglishLayout::Dvorak, false), Some('['));
    assert_eq!(KeyCode::Minus.to_char_for_layout(EnglishLayout::Dvorak, true), Some('{'));
    // Dvorak: Q → '
    assert_eq!(KeyCode::Q.to_char_for_layout(EnglishLayout::Dvorak, false), Some('\''));
    assert_eq!(KeyCode::Q.to_char_for_layout(EnglishLayout::Dvorak, true), Some('"'));
}

#[test]
fn test_to_char_for_layout_colemak() {
    // Colemak: 물리키 E → 'f', 물리키 K → 'e'
    assert_eq!(KeyCode::E.to_char_for_layout(EnglishLayout::Colemak, false), Some('f'));
    assert_eq!(KeyCode::K.to_char_for_layout(EnglishLayout::Colemak, false), Some('e'));
}

#[test]
fn test_to_char_for_layout_colemak_dh() {
    // ColemakDh: 물리키 T → 'b' (Colemak은 'g')
    assert_eq!(KeyCode::T.to_char_for_layout(EnglishLayout::ColemakDh, false), Some('b'));
    assert_eq!(KeyCode::T.to_char_for_layout(EnglishLayout::Colemak, false), Some('g'));
}

#[test]
fn test_to_char_for_layout_workman() {
    // Workman: 물리키 W → 'd', D → 'h'
    assert_eq!(KeyCode::W.to_char_for_layout(EnglishLayout::Workman, false), Some('d'));
    assert_eq!(KeyCode::D.to_char_for_layout(EnglishLayout::Workman, false), Some('h'));
}

#[test]
fn test_to_char_for_layout_space() {
    // Space는 모든 레이아웃에서 동일
    for layout in [EnglishLayout::Qwerty, EnglishLayout::Dvorak, EnglishLayout::Colemak, EnglishLayout::ColemakDh, EnglishLayout::Workman] {
        assert_eq!(KeyCode::Space.to_char_for_layout(layout, false), Some(' '));
    }
}
```

**import 추가**: `src/keycode.rs` 최상단에 `use crate::config::EnglishLayout;` 추가 필요.
다만 `keycode.rs`가 `config.rs`를 import하면 순환 의존이 발생할 수 있으므로 확인이 필요하다.

> **대안**: `EnglishLayout`을 직접 import하는 대신, `to_char_for_layout()`에서 `u32` 인덱스를 받거나, `EnglishLayout`을 `keycode.rs` 또는 별도 모듈로 이동. 또는 `config.rs`에서 `keycode.rs`를 import하는 방향이 아닌지 확인.

**순환 의존 확인 방법**: `config.rs`가 `keycode.rs`를 import하는지 grep 확인.

---

### 2. `src/auto_typefix.rs` — 시그니처 변경 2곳 + 내부 수정

#### 2-A. `to_ascii_string()` 시그니처 변경 (107행)

**현재**: `src/auto_typefix.rs:107`
```rust
pub fn to_ascii_string(&self) -> String {
```

**변경 후**:
```rust
pub fn to_ascii_string(&self, english_layout: EnglishLayout) -> String {
    let mut s = String::with_capacity(self.entries.len());
    for entry in &self.entries {
        let c = entry.keycode.to_char_for_layout(english_layout, entry.modifier.shift);
        if let Some(c) = c {
            s.push(c);
        }
    }
    s
}
```

기존 108-119행 전체를 교체.

#### 2-B. `check_forward()` 내 `to_ascii_string()` 호출 수정 (165행)

**현재**: `src/auto_typefix.rs:165`
```rust
let ascii = buffer.to_ascii_string();
```

**변경 후**:
```rust
let ascii = buffer.to_ascii_string(english_layout);
```

`check_forward()`는 이미 `english_layout: EnglishLayout` 파라미터를 보유 (158행).

#### 2-C. `check_forward()` 내 `partial_ascii` 루프 수정 (218-227행)

**현재**: `src/auto_typefix.rs:218-227`
```rust
let partial_ascii: String = entries[..i]
    .iter()
    .filter_map(|e| {
        if e.modifier.shift {
            e.keycode.to_shifted_char()
        } else {
            e.keycode.to_char()
        }
    })
    .collect();
```

**변경 후**:
```rust
let partial_ascii: String = entries[..i]
    .iter()
    .filter_map(|e| {
        e.keycode.to_char_for_layout(english_layout, e.modifier.shift)
    })
    .collect();
```

#### 2-D. `check_reverse()` 시그니처 변경 (271행)

**현재**: `src/auto_typefix.rs:271-274`
```rust
pub fn check_reverse(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
) -> Option<AutoTypeFixResult> {
```

**변경 후**:
```rust
pub fn check_reverse(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
    english_layout: EnglishLayout,
) -> Option<AutoTypeFixResult> {
```

#### 2-E. `check_reverse()` 내 `to_ascii_string()` 호출 수정 (280행)

**현재**: `src/auto_typefix.rs:280`
```rust
let eng = buffer.to_ascii_string();
```

**변경 후**:
```rust
let eng = buffer.to_ascii_string(english_layout);
```

#### 2-F. 테스트 수정 (기존 테스트 호출부)

| 행 | 현재 | 변경 |
|----|------|------|
| 370 | `buf.to_ascii_string()` | `buf.to_ascii_string(EnglishLayout::Qwerty)` |
| 382 | `buf.to_ascii_string()` | `buf.to_ascii_string(EnglishLayout::Qwerty)` |
| 478 | `check_reverse(&buf, &config)` | `check_reverse(&buf, &config, EnglishLayout::Qwerty)` |
| 501 | `check_reverse(&buf, &config)` | `check_reverse(&buf, &config, EnglishLayout::Qwerty)` |
| 515 | `check_reverse(&buf, &config)` | `check_reverse(&buf, &config, EnglishLayout::Qwerty)` |

#### 2-G. 새 테스트 추가 (테스트 모듈 끝에)

```rust
#[test]
fn test_to_ascii_string_dvorak() {
    // Dvorak: 물리키 S,D,F → Dvorak에서 o,e,u
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::S, KeyCode::D, KeyCode::F] {
        buf.push(key, ModifierState::default());
    }
    assert_eq!(buf.to_ascii_string(EnglishLayout::Qwerty), "sdf");
    assert_eq!(buf.to_ascii_string(EnglishLayout::Dvorak), "oeu");
}

#[test]
fn test_reverse_dvorak_hello() {
    // Dvorak에서 "hello" 물리키: D,E,N,N,S (Dvorak에서 h=J위치... 아님)
    // Dvorak 매핑: h→물리키 J, e→물리키 D, l→물리키 P, o→물리키 S
    // 즉 "hello" = 물리키 J,D,P,P,S
    let mut buf = KeystrokeBuffer::new();
    for key in [KeyCode::J, KeyCode::D, KeyCode::P, KeyCode::P, KeyCode::S] {
        buf.push(key, ModifierState::default());
    }
    buf.committed_chars = 3;
    buf.has_preedit = true;

    let config = AutoTypeFixConfig {
        eng_word_min_length: 5,
        ..AutoTypeFixConfig::default()
    };

    // Dvorak 레이아웃으로 check_reverse
    let result = check_reverse(&buf, &config, EnglishLayout::Dvorak);
    assert!(result.is_some(), "Dvorak에서 'hello' 감지 실패");
    let r = result.unwrap();
    assert_eq!(r.corrected, "hello");
}
```

---

### 3. `unim-dbus/src/engine_worker.rs` — check_reverse 호출 수정 (267행)

**현재**: `unim-dbus/src/engine_worker.rs:267`
```rust
auto_typefix::check_reverse(buf, atf_config)
```

**변경 후**:
```rust
auto_typefix::check_reverse(buf, atf_config, config.engine.english.layout)
```

변경 1행. `config.engine.english.layout`은 같은 스코프(263행)에서 이미 사용 중.

---

### 4. 순환 의존 확인 (사전 검증 필수)

`keycode.rs`에서 `use crate::config::EnglishLayout;` 추가 시 순환 의존 가능성:
- `config.rs` → `keycode.rs` import 여부 확인
- 만약 순환이면 `EnglishLayout`을 별도 파일로 분리하거나, `to_char_for_layout()`이 `u32` layout_id를 받는 방식으로 우회

**확인 명령**: `grep -n 'use crate::keycode' src/config.rs`

---

## 구현 순서

| 단계 | 작업 | 의존 | 예상 행수 |
|------|------|------|-----------|
| **1** | 순환 의존 확인 (`config.rs` ↔ `keycode.rs`) | 없음 | 0 |
| **2** | `src/keycode.rs`: const 테이블 + `physical_position()` + `to_char_for_layout()` 추가 | 1 통과 | ~120행 |
| **3** | `src/keycode.rs`: 테스트 추가 및 `cargo test -p unim -- keycode` 통과 확인 | 2 | ~60행 |
| **4** | `src/auto_typefix.rs`: `to_ascii_string()` 시그니처 변경 + 내부 수정 | 2 | ~5행 변경 |
| **5** | `src/auto_typefix.rs`: `check_forward()` 내 `partial_ascii` 루프 수정 | 4 | ~5행 변경 |
| **6** | `src/auto_typefix.rs`: `check_reverse()` 시그니처 변경 + 내부 수정 | 4 | ~3행 변경 |
| **7** | `unim-dbus/src/engine_worker.rs`: `check_reverse()` 호출에 layout 전달 | 6 | 1행 변경 |
| **8** | `src/auto_typefix.rs`: 기존 테스트 수정 (5곳에 `EnglishLayout::Qwerty` 추가) | 4,6 | 5행 변경 |
| **9** | `src/auto_typefix.rs`: Dvorak 테스트 추가 | 4,6 | ~30행 |
| **10** | `cargo build --workspace` zero warning 확인 | 전체 | - |
| **11** | `cargo test --workspace` all pass 확인 | 전체 | - |
| **12** | `make build` (C/C++ 포함) warning-free 확인 | 11 | - |

---

## 검증 방법

### 단위 테스트
1. `cargo test -p unim -- keycode::tests::test_to_char_for_layout` — 5개 레이아웃 문자 매핑 정확성
2. `cargo test -p unim -- auto_typefix::tests::test_to_ascii_string_dvorak` — Dvorak 버퍼 변환
3. `cargo test -p unim -- auto_typefix::tests::test_reverse_dvorak` — Dvorak 역방향 감지
4. 기존 모든 테스트 통과 (`cargo test --workspace`)

### 회귀 검증
- Qwerty 레이아웃 기존 동작 100% 동일 (최적화 경로 사용)
- `make build` zero warning

### 수동 검증 (선택)
- `UNIM_DEVELOP=1` + config에서 `english_layout: dvorak` 설정 후 한영 오타 교정 테스트

---

## 리스크

| 리스크 | 심각도 | 대응 |
|--------|--------|------|
| `keycode.rs` ↔ `config.rs` 순환 의존 | 중 | 단계 1에서 확인. 순환 시 `EnglishLayout`을 `keycode.rs`로 이동하거나 정수 인자 사용 |
| const 테이블 데이터 오류 (오타) | 중 | Qwerty consistency 테스트 + 각 레이아웃 spot check 테스트로 검증 |
| `to_ascii_string()` 호출부 누락 | 저 | grep 결과: 2곳(165행, 280행)만 호출. 테스트에서 2곳(370행, 382행). 총 4곳 확인 완료 |
| `check_reverse()` 호출부 누락 | 저 | grep 결과: engine_worker.rs:267 1곳 + 테스트 3곳. 총 4곳 확인 완료 |
| 다른 코드에서 `to_char()` 직접 호출 | 없음 | `to_char()`/`to_shifted_char()` 유지하므로 영향 없음. `is_character_key()`도 그대로 동작 |

---

## 변경 요약

| 파일 | 변경 행수 | 변경 유형 |
|------|-----------|-----------|
| `src/keycode.rs` | +180행 | const 테이블 + 메서드 2개 + 테스트 6개 |
| `src/auto_typefix.rs` | +35행, ~15행 수정 | 시그니처 변경 2개 + 내부 수정 3곳 + 기존 테스트 수정 5곳 + 새 테스트 2개 |
| `unim-dbus/src/engine_worker.rs` | 1행 수정 | `check_reverse()` 호출에 layout 인자 추가 |
| **합계** | **+215행, ~16행 수정** | |
