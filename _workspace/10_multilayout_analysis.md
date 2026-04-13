# AutoTypeFix 다중 키맵 정밀 분석

## 1. KeyCode 물리키 위치 테이블

`src/keycode.rs`의 `to_char()`가 `Some`을 반환하는 KeyCode는 총 **47개** (Space 포함).
AutoTypeFix 버퍼에서 Space는 제외되므로 실질 대상은 **46개**.

### 행별 KeyCode (QWERTY 물리 위치 기준)

| 행 | Idx | KeyCode | QWERTY lower | QWERTY upper |
|----|-----|---------|-------------|-------------|
| 1st | 0 | Backquote | ` | ~ |
| 1st | 1 | Num1 | 1 | ! |
| 1st | 2 | Num2 | 2 | @ |
| 1st | 3 | Num3 | 3 | # |
| 1st | 4 | Num4 | 4 | $ |
| 1st | 5 | Num5 | 5 | % |
| 1st | 6 | Num6 | 6 | ^ |
| 1st | 7 | Num7 | 7 | & |
| 1st | 8 | Num8 | 8 | * |
| 1st | 9 | Num9 | 9 | ( |
| 1st | 10 | Num0 | 0 | ) |
| 1st | 11 | Minus | - | _ |
| 1st | 12 | Equal | = | + |
| 1st | 13 | Backslash | \ | \| |
| 2nd | 0 | Q | q | Q |
| 2nd | 1 | W | w | W |
| 2nd | 2 | E | e | E |
| 2nd | 3 | R | r | R |
| 2nd | 4 | T | t | T |
| 2nd | 5 | Y | y | Y |
| 2nd | 6 | U | u | U |
| 2nd | 7 | I | i | I |
| 2nd | 8 | O | o | O |
| 2nd | 9 | P | p | P |
| 2nd | 10 | BracketLeft | [ | { |
| 2nd | 11 | BracketRight | ] | } |
| 3nd | 0 | A | a | A |
| 3nd | 1 | S | s | S |
| 3nd | 2 | D | d | D |
| 3nd | 3 | F | f | F |
| 3nd | 4 | G | g | G |
| 3nd | 5 | H | h | H |
| 3nd | 6 | J | j | J |
| 3nd | 7 | K | k | K |
| 3nd | 8 | L | l | L |
| 3nd | 9 | Semicolon | ; | : |
| 3nd | 10 | Quote | ' | " |
| 4th | 0 | Z | z | Z |
| 4th | 1 | X | x | X |
| 4th | 2 | C | c | C |
| 4th | 3 | V | v | V |
| 4th | 4 | B | b | B |
| 4th | 5 | N | n | N |
| 4th | 6 | M | m | M |
| 4th | 7 | Comma | , | < |
| 4th | 8 | Period | . | > |
| 4th | 9 | Slash | / | ? |

## 2. 5개 레이아웃 교차 비교표

### 2.1 Lower (비Shift)

5개 레이아웃 중 **13개** KeyCode는 모든 레이아웃에서 동일, **34개**가 하나 이상의 레이아웃에서 다름.

| KeyCode | Qwerty | Dvorak | Colemak | ColemakDh | Workman |
|---------|--------|--------|---------|-----------|---------|
| Backquote | ` | ` | ` | ` | ` |
| Num1 | 1 | 1 | 1 | 1 | 1 |
| Num2 | 2 | 2 | 2 | 2 | 2 |
| Num3 | 3 | 3 | 3 | 3 | 3 |
| Num4 | 4 | 4 | 4 | 4 | 4 |
| Num5 | 5 | 5 | 5 | 5 | 5 |
| Num6 | 6 | 6 | 6 | 6 | 6 |
| Num7 | 7 | 7 | 7 | 7 | 7 |
| Num8 | 8 | 8 | 8 | 8 | 8 |
| Num9 | 9 | 9 | 9 | 9 | 9 |
| Num0 | 0 | 0 | 0 | 0 | 0 |
| **Minus** | **-** | **[** | - | - | - |
| **Equal** | **=** | **]** | = | = | = |
| Backslash | \ | \ | \ | \ | \ |
| **Q** | **q** | **'** | q | q | q |
| **W** | **w** | **,** | w | w | **d** |
| **E** | **e** | **.** | **f** | **f** | **r** |
| **R** | **r** | **p** | **p** | **p** | **w** |
| **T** | **t** | **y** | **g** | **b** | **b** |
| **Y** | **y** | **f** | **j** | **j** | **j** |
| **U** | **u** | **g** | **l** | **l** | **f** |
| **I** | **i** | **c** | **u** | **u** | **u** |
| **O** | **o** | **r** | **y** | **y** | **p** |
| **P** | **p** | **l** | **;** | **;** | **;** |
| **BracketLeft** | **[** | **/** | [ | [ | [ |
| **BracketRight** | **]** | **=** | ] | ] | ] |
| A | a | a | a | a | a |
| **S** | **s** | **o** | **r** | **r** | s |
| **D** | **d** | **e** | **s** | **s** | **h** |
| **F** | **f** | **u** | **t** | **t** | **t** |
| **G** | **g** | **i** | **d** | g | g |
| **H** | **h** | **d** | h | **m** | **y** |
| **J** | **j** | **h** | **n** | **n** | **n** |
| **K** | **k** | **t** | **e** | **e** | **e** |
| **L** | **l** | **n** | **i** | **i** | **o** |
| **Semicolon** | **;** | **s** | **o** | **o** | **i** |
| **Quote** | **'** | **-** | ' | ' | ' |
| **Z** | **z** | **;** | z | z | z |
| **X** | **x** | **q** | x | x | x |
| **C** | **c** | **j** | c | c | **m** |
| **V** | **v** | **k** | v | **d** | **c** |
| **B** | **b** | **x** | b | **v** | **v** |
| **N** | **n** | **b** | **k** | **k** | **k** |
| **M** | **m** | **m** | m | **h** | **l** |
| Comma | , | , | , | , | , |
| Period | . | . | . | . | . |
| Slash | / | / | / | / | / |

### 2.2 Upper (Shift)

| KeyCode | Qwerty | Dvorak | Colemak | ColemakDh | Workman |
|---------|--------|--------|---------|-----------|---------|
| Backquote | ~ | ~ | ~ | ~ | ~ |
| Num1 | ! | ! | ! | ! | ! |
| Num2 | @ | @ | @ | @ | @ |
| Num3 | # | # | # | # | # |
| Num4 | $ | $ | $ | $ | $ |
| Num5 | % | % | % | % | % |
| Num6 | ^ | ^ | ^ | ^ | ^ |
| Num7 | & | & | & | & | & |
| Num8 | * | * | * | * | * |
| Num9 | ( | ( | ( | ( | ( |
| Num0 | ) | ) | ) | ) | ) |
| **Minus** | **_** | **{** | _ | _ | _ |
| **Equal** | **+** | **}** | + | + | + |
| Backslash | \| | \| | \| | \| | \| |
| **Q** | **Q** | **"** | Q | Q | Q |
| **W** | **W** | **<** | W | W | **D** |
| **E** | **E** | **>** | **F** | **F** | **R** |
| **R** | **R** | **P** | **P** | **P** | **W** |
| **T** | **T** | **Y** | **G** | **B** | **B** |
| **Y** | **Y** | **F** | **J** | **J** | **J** |
| **U** | **U** | **G** | **L** | **L** | **F** |
| **I** | **I** | **C** | **U** | **U** | **U** |
| **O** | **O** | **R** | **Y** | **Y** | **P** |
| **P** | **P** | **L** | **:** | **:** | **:** |
| **BracketLeft** | **{** | **?** | { | { | { |
| **BracketRight** | **}** | **+** | } | } | } |
| A | A | A | A | A | A |
| **S** | **S** | **O** | **R** | **R** | S |
| **D** | **D** | **E** | **S** | **S** | **H** |
| **F** | **F** | **U** | **T** | **T** | **T** |
| **G** | **G** | **I** | **D** | G | G |
| **H** | **H** | **D** | H | **M** | **Y** |
| **J** | **J** | **H** | **N** | **N** | **N** |
| **K** | **K** | **T** | **E** | **E** | **E** |
| **L** | **L** | **N** | **I** | **I** | **O** |
| **Semicolon** | **:** | **S** | **O** | **O** | **I** |
| **Quote** | **"** | **_** | " | " | " |
| **Z** | **Z** | **:** | Z | Z | Z |
| **X** | **X** | **Q** | X | X | X |
| **C** | **C** | **J** | C | C | **M** |
| **V** | **V** | **K** | V | **D** | **C** |
| **B** | **B** | **X** | B | **V** | **V** |
| **N** | **N** | **B** | **K** | **K** | **K** |
| **M** | **M** | **M** | M | **H** | **L** |
| Comma | < | < | < | < | < |
| Period | > | > | > | > | > |
| Slash | ? | ? | ? | ? | ? |

## 3. to_char_for_layout() 구현용 매핑 데이터

### 3.1 구현 전략

현재 `to_char()`/`to_shifted_char()`는 QWERTY 고정 match문. 새 메서드 `to_char_for_layout(layout, shifted)`를 추가.

**핵심 관찰**: 
- 13개 KeyCode는 모든 레이아웃에서 동일 (숫자행 대부분 + Backquote/Backslash/A/Comma/Period/Slash)
- 34개만 레이아웃별 분기 필요
- Qwerty인 경우 기존 `to_char()`와 동일 -> 최적화 가능

**권장 구현**: `const` 배열 기반 lookup table

```rust
/// 물리키 -> (행, 열) 인덱스. JSON 키맵과 동일한 순서.
/// 레이아웃별 문자를 const 배열에서 O(1) lookup.
impl KeyCode {
    /// 물리키 위치를 (행 0-3, 열) 인덱스로 반환
    fn physical_position(&self) -> Option<(usize, usize)> {
        match self {
            KeyCode::Backquote => Some((0, 0)),
            KeyCode::Num1 => Some((0, 1)),
            // ... (47개 전부)
            _ => None,
        }
    }

    pub fn to_char_for_layout(&self, layout: EnglishLayout, shifted: bool) -> Option<char> {
        // QWERTY 최적화: 기존 메서드 재사용 (가장 흔한 경우)
        if layout == EnglishLayout::Qwerty {
            return if shifted { self.to_shifted_char() } else { self.to_char() };
        }
        
        let (row, col) = self.physical_position()?;
        let table = match layout {
            EnglishLayout::Dvorak => &DVORAK_TABLE,
            EnglishLayout::Colemak => &COLEMAK_TABLE,
            EnglishLayout::ColemakDh => &COLEMAK_DH_TABLE,
            EnglishLayout::Workman => &WORKMAN_TABLE,
            EnglishLayout::Qwerty => unreachable!(),
        };
        let row_data = &table[row];
        if col >= row_data.len() { return None; }
        let (lower, upper) = row_data[col];
        Some(if shifted { upper } else { lower })
    }
}
```

### 3.2 레이아웃별 const 테이블 데이터

각 테이블은 `[[(char, char); N]; 4]` 형태 (행 4개, 각 행의 열 수 다름).

#### Dvorak

```
Row 0 (14): ('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('[','{'),(']','}'),('\\','|')
Row 1 (12): ('\'','"'),(',','<'),('.','>'),('p','P'),('y','Y'),('f','F'),('g','G'),('c','C'),('r','R'),('l','L'),('/','?'),('=','+')
Row 2 (11): ('a','A'),('o','O'),('e','E'),('u','U'),('i','I'),('d','D'),('h','H'),('t','T'),('n','N'),('s','S'),('-','_')
Row 3 (10): (';',':'),('q','Q'),('j','J'),('k','K'),('x','X'),('b','B'),('m','M'),('w','W'),('v','V'),('z','Z')
```

#### Colemak

```
Row 0 (14): ('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')
Row 1 (12): ('q','Q'),('w','W'),('f','F'),('p','P'),('g','G'),('j','J'),('l','L'),('u','U'),('y','Y'),(';',':'),('[','{'),( ']','}')
Row 2 (11): ('a','A'),('r','R'),('s','S'),('t','T'),('d','D'),('h','H'),('n','N'),('e','E'),('i','I'),('o','O'),('\'','"')
Row 3 (10): ('z','Z'),('x','X'),('c','C'),('v','V'),('b','B'),('k','K'),('m','M'),(',','<'),('.','>'),('/','?')
```

#### ColemakDh

```
Row 0 (14): ('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')
Row 1 (12): ('q','Q'),('w','W'),('f','F'),('p','P'),('b','B'),('j','J'),('l','L'),('u','U'),('y','Y'),(';',':'),('[','{'),( ']','}')
Row 2 (11): ('a','A'),('r','R'),('s','S'),('t','T'),('g','G'),('m','M'),('n','N'),('e','E'),('i','I'),('o','O'),('\'','"')
Row 3 (10): ('z','Z'),('x','X'),('c','C'),('d','D'),('v','V'),('k','K'),('h','H'),(',','<'),('.','>'),('/','?')
```

#### Workman

```
Row 0 (14): ('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+'),('\\','|')
Row 1 (12): ('q','Q'),('d','D'),('r','R'),('w','W'),('b','B'),('j','J'),('f','F'),('u','U'),('p','P'),(';',':'),('[','{'),( ']','}')
Row 2 (11): ('a','A'),('s','S'),('h','H'),('t','T'),('g','G'),('y','Y'),('n','N'),('e','E'),('o','O'),('i','I'),('\'','"')
Row 3 (10): ('z','Z'),('x','X'),('m','M'),('c','C'),('v','V'),('k','K'),('l','L'),(',','<'),('.','>'),('/','?')
```

## 4. check_reverse() 수정 가능성 확인

### 4.1 현재 상태

```rust
// auto_typefix.rs:271
pub fn check_reverse(
    buffer: &KeystrokeBuffer,
    config: &AutoTypeFixConfig,
) -> Option<AutoTypeFixResult>
```

- `english_layout` 파라미터 **없음**
- 내부에서 `buffer.to_ascii_string()` 호출 (QWERTY 고정)
- 이 ASCII 문자열로 영어 사전 매칭 수행

### 4.2 engine_worker.rs 호출부

```rust
// unim-dbus/src/engine_worker.rs:267
auto_typefix::check_reverse(buf, atf_config)
```

같은 스코프에서 `config.engine.english.layout`에 이미 접근 가능:
- 262행: `config.engine.korean.layout` 사용
- 263행: `config.engine.english.layout` 사용 (check_forward 호출에서)

### 4.3 수정 계획

**완전히 가능.** 필요한 변경:

1. `check_reverse()` 시그니처에 `english_layout: EnglishLayout` 추가
2. 내부의 `buffer.to_ascii_string()` -> `buffer.to_ascii_string(english_layout)` 변경
3. `engine_worker.rs:267`에서 `config.engine.english.layout` 전달
4. 테스트 코드의 `check_reverse()` 호출부도 동일하게 수정

### 4.4 영향 범위

| 파일 | 변경 | 난이도 |
|------|------|--------|
| `src/keycode.rs` | `to_char_for_layout()` 추가 (~50행) | 중 |
| `src/auto_typefix.rs` | `to_ascii_string()` 시그니처 변경, `check_reverse()` 시그니처 변경 | 저 |
| `unim-dbus/src/engine_worker.rs` | `check_reverse()` 호출에 layout 추가 (1행) | 매우 저 |
| 테스트 코드 | `to_ascii_string()`, `check_reverse()` 호출부 수정 | 저 |

## 5. 핵심 발견 요약

1. **47개 KeyCode** 중 **34개**가 레이아웃별로 다른 문자를 생성 (72%)
2. 숫자행은 Dvorak의 Minus/Equal만 다름; 알파벳 영역은 대거 재배치
3. `check_forward()`는 이미 `english_layout` 파라미터 보유 -- `check_reverse()`만 누락
4. `engine_worker.rs`에서 `config.engine.english.layout` 접근 가능 확인 완료
5. **구현 복잡도: 낮음** -- const lookup table + 시그니처 변경 2곳이 핵심
