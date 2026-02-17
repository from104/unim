# UNIM CLI (unim-cli) 세부 기능 명세

> `unim-cli`는 **커맨드라인 한글↔영문 변환 도구**입니다.
> 영문 키 입력 스트림을 한글로 조합하거나, 한글을 영문 키 스트림으로 분해합니다.
> 코어 엔진(`src/`)의 `hangul/` 및 `keystroke/` 모듈을 직접 사용하며,
> DBus/데몬 없이 **독립적으로 실행**됩니다.

---

## 1. 아키텍처

```
stdin / 파일  ──▶  unim-cli  ──▶  stdout / 파일
                     │
                     ├── keystroke/keyboard_map   (키맵 로드)
                     ├── keystroke/keystrokes_to_korean  (영→한)
                     ├── keystroke/korean_to_keystrokes  (한→영)
                     ├── hangul/composer_with_2bul  (두벌식)
                     └── hangul/composer_with_3bul  (세벌식)
```

### 파일 구조

```
unim-cli/
├── Cargo.toml
├── src/
│   └── main.rs         # CLI 로직 (289행)
└── locales/
    ├── ko.yml          # 한국어 번역 (57개 키)
    └── en.yml          # 영어 번역 (57개 키)
```

---

## 2. CLI 인터페이스

```
unim-cli [OPTIONS] [FILE...]
```

### 2.1 인자

| 인자 | 설명 |
|------|------|
| `FILE...` | 입력 파일 (미지정 시 표준 입력, `-`으로 명시 가능) |

### 2.2 옵션

| 옵션 | 단축 | 기본값 | 설명 |
|------|------|--------|------|
| `--compose` | `-c` | ✓ (기본) | 영어 → 한글 변환 |
| `--decompose` | `-d` | | 한글 → 영어 변환 |
| `--output FILE` | `-o` | stdout | 출력 파일 |
| `--korean-keyboard MODE` | `-k` | `2bul` | 한국어 자판 |
| `--english-keyboard MODE` | `-e` | `qwerty` | 영어 자판 |

### 2.3 한국어 자판 모드 (`-k`)

| 값 | 설명 |
|----|------|
| `2bul` (기본) | 두벌식 표준 |
| `390` | 세벌식 390 |
| `391` | 세벌식 최종 |
| `noshift` | 세벌식 순아래 |

### 2.4 영어 자판 모드 (`-e`)

| 값 | 설명 |
|----|------|
| `qwerty` (기본) | QWERTY |
| `dvorak` | Dvorak |
| `colemak` | Colemak |
| `colemak_dh` | Colemak-DH |
| `workman` | Workman |

---

## 3. 변환 모드

### 3.1 영어 → 한글 (`--compose`, 기본)

영문 키 스트림을 한글 음절로 조합합니다.

```bash
$ echo "gksrmf" | unim-cli
한글

$ echo "dkssudgktpdy" | unim-cli
안녕하세요
```

내부 흐름:

```
1. 영어/한글 JSON 키맵 로드 (include_str! 임베딩)
2. KeyboardMap::create_keyboard_map_from_str() → HashMap<char, JamoEnum>
3. 두벌식 → HangulComposer2Bul / 세벌식 → HangulComposer3Bul 생성
4. 줄 단위 반복:
   keystrokes_to_korean(line, keyboard_map, composer) → 한글 문자열
5. 출력
```

### 3.2 한글 → 영어 (`--decompose`)

한글 문자열을 원래의 영문 키 스트림으로 분해합니다.

```bash
$ echo "한글" | unim-cli -d
gksrmf

$ echo "안녕하세요" | unim-cli -d
dkssudgktpdy
```

내부 흐름:

```
1. 영어/한글 JSON 키맵 로드
2. KeyboardMap 생성
3. 줄 단위 반복:
   korean_to_keystrokes(line, keyboard_map, is_three_bul) → 영문 스트림
4. 출력
```

---

## 4. 입출력 처리

### 4.1 입력 소스

| 지정 방식 | 동작 |
|-----------|------|
| 인자 없음 | 표준 입력 (대화형/파이프) |
| `파일명` | 해당 파일 읽기 |
| `-` | 명시적 표준 입력 |
| 여러 파일 | 순서대로 연결하여 처리 |

> [!NOTE]
> `-`(표준 입력)가 여러 번 지정되면 경고를 출력하고 중복을 무시합니다.

### 4.2 출력 대상

| 지정 방식 | 동작 |
|-----------|------|
| `-o` 미지정 | 표준 출력 |
| `-o 파일명` | 파일에 쓰기 |
| `-o -` | 명시적 표준 출력 |

### 4.3 줄 단위 처리

- 빈 줄은 그대로 출력 (변환 없음)
- 각 줄은 독립적으로 변환 (조합 상태가 줄 간에 유지되지 않음)

---

## 5. 국제화 (i18n)

### 5.1 메커니즘

`rust-i18n` 크레이트를 사용하며, `locales/` 디렉토리의 YAML 파일에서 번역을 로드합니다.

```rust
rust_i18n::i18n!("locales");  // 빌드 시 임베딩
```

### 5.2 로케일 결정

```
1. LANG 환경변수 확인
2. LC_ALL 환경변수 확인 (폴백)
3. 둘 다 없으면 "en"
4. "ko_KR.UTF-8" → "ko" (첫 번째 '_' 전까지)
```

### 5.3 번역 범위

| 분류 | 키 수 | 예시 |
|------|-------|------|
| CLI 설명 | 8 | `unim_cli_about`, `compose_desc` |
| 레이아웃 이름 | 5 | `twobul_std`, `qwerty` |
| 경고/에러 | 3 | `warning_multiple_stdin`, `error_label` |
| 설정 관련 | 20+ | `config_saved`, `layout_changed` |

---

## 6. 의존성

| 크레이트 | 용도 |
|----------|------|
| `unim` | 코어 엔진 (hangul, keystroke 모듈) |
| `clap` (derive) | CLI 인자 파싱 |
| `rust-i18n` | 국제화 |
| `serde` + `serde_json` | JSON 키맵 파싱 |

---

## 7. 사용 예시

```bash
# 기본 (영→한, 두벌식, QWERTY)
echo "gksrmf" | unim-cli
# → 한글

# 세벌식 390
echo "key_stream" | unim-cli -k 390

# 세벌식 순아래
echo "key_stream" | unim-cli -k noshift

# 한→영 분해
echo "한글" | unim-cli -d
# → gksrmf

# 파일 변환
unim-cli input.txt -o output.txt

# Dvorak 키보드
unim-cli -e dvorak input.txt

# Colemak-DH 키보드
unim-cli -e colemak_dh input.txt

# 파이프 체인
cat document.txt | unim-cli -d | sort | uniq
```

---

## 8. 빌드 및 설치

```bash
# 빌드
cargo build -p unim-cli

# 설치 경로 (Makefile 기준)
/usr/lib/unim/unim-cli
```
