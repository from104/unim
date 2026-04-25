# UNIM CLI (unim-cli) 세부 기능 명세

> `unim-cli`는 **통합 커맨드라인 도구**입니다.
> 두 가지 모드를 제공합니다:
>
> 1. **텍스트 변환** (기본): 영문 키 입력 스트림을 한글로 조합하거나, 한글을 영문 키 스트림으로 분해합니다.
> 2. **설정 관리** (`config` 서브커맨드): UNIM 설정을 조회/변경/초기화하고 인터렉티브 편집 세션을 제공합니다.
>
> 변환 경로는 코어 엔진(`src/`)의 `hangul/` 및 `keystroke/` 모듈을 직접 사용하여 DBus/데몬 없이 독립 실행됩니다.
> 설정 경로는 `unim::config` 모듈의 로드/저장 API를 사용하며, 데몬은 파일 변경을 감지하여 자동으로 재로드합니다.

---

## 1. 아키텍처

```
stdin / 파일  ──▶  unim-cli (변환)  ──▶  stdout / 파일
                     │
                     ├── keystroke/keyboard_map   (키맵 로드)
                     ├── keystroke/keystrokes_to_korean  (영→한)
                     ├── keystroke/korean_to_keystrokes  (한→영)
                     ├── hangul/composer_with_2bul  (두벌식)
                     └── hangul/composer_with_3bul  (세벌식)

CLI args  ──▶  unim-cli config  ──▶  ~/.config/unim/config.yaml
                     │
                     └── unim::config (load / save / clamp_ranges)
```

### 파일 구조

```
unim-cli/
├── Cargo.toml
├── SPEC.md
├── src/
│   └── main.rs         # 통합 CLI 로직 (변환 + config 서브커맨드)
└── locales/
    ├── ko.yml          # 한국어 번역
    └── en.yml          # 영어 번역
```

---

## 2. CLI 인터페이스

```
unim-cli [OPTIONS] [FILE...]           # 변환 모드 (기본)
unim-cli config [SUBCOMMAND] [ARGS...] # 설정 서브커맨드
```

`clap`의 `args_conflicts_with_subcommands` 옵션으로 두 모드를 배타적으로 처리합니다.

### 2.1 변환 모드 인자/옵션

| 인자/옵션 | 단축 | 기본값 | 설명 |
|-----------|------|--------|------|
| `FILE...` | | stdin | 입력 파일 (미지정 시 표준 입력, `-`으로 명시 가능) |
| `--compose` | `-c` | ✓ (기본) | 영어 → 한글 변환 |
| `--decompose` | `-d` | | 한글 → 영어 변환 |
| `--output FILE` | `-o` | stdout | 출력 파일 |
| `--korean-keyboard MODE` | `-k` | `2bul` | 한국어 자판 (`2bul`, `390`, `391`, `noshift`) |
| `--english-keyboard MODE` | `-e` | `qwerty` | 영어 자판 (`qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`) |

### 2.2 `config` 서브커맨드

| 서브커맨드 | 설명 |
|------------|------|
| `show` | 현재 설정 요약 출력 |
| `set <KEY> <VALUE>` | 설정 항목 변경 후 저장 |
| `path` | 설정 파일 절대 경로 출력 |
| `reset` | 설정을 기본값으로 초기화 |
| `interactive` | 터미널 대화형 편집 세션 시작 |
| 없음 | `show` + 도움말 힌트 |

### 2.3 `config set` 키 목록

| 키 이름 | 값 예시 / 범위 |
|---------|----------------|
| `korean-layout` | `2bul`, `3bul390`, `3bul391`, `3bul_noshift` |
| `english-layout` | `qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman` |
| `default-category` | `korean`, `english` |
| `mode-sharing` | `global`, `per-app` |
| `toggle-keys` | 쉼표 구분 (예: `Korean,RightAlt`) |
| `hanja-keys` | 쉼표 구분 (예: `Hanja,F9`) |
| `popup-mode` | `standalone`, `embedded` |
| `auto-typefix` | `true`, `false` |
| `auto-typefix-kor-threshold` | 2 ~ 6 |
| `auto-typefix-eng-min-length` | 3 ~ 8 |
| `auto-typefix-forward-time-window-ms` | 500 ~ 5000 |
| `auto-typefix-reverse-time-window-ms` | 500 ~ 5000 |
| `auto-typefix-forward` / `auto-typefix-reverse` | `true`, `false` |
| `auto-typefix-skip-english-word` | `true`, `false` |
| `auto-typefix-skip-complete-syllable` | `true`, `false` |
| `auto-typefix-rollback-detection` | `true`, `false` |
| `auto-typefix-tentative-expiry-hours` | 1 ~ 12 |
| `auto-typefix-observation-timeout-secs` | 5 ~ 15 |
| `app-rules` | JSON 배열 (`[{"app_pattern":"code","default_category":"english"}]`) |

값 범위는 `unim::config` SSoT 상수(`AUTO_TYPEFIX_*_MIN/MAX`)를 그대로 사용하며, 저장 전에 `AutoTypeFix::clamp_ranges()`로 방어합니다.

### 2.4 `config interactive` 메뉴

1. 한국어 레이아웃 선택
2. 영어 레이아웃 선택
3. 초기 입력 모드 선택
4. 모드 공유 방식 선택
5. 한/영 전환 키 편집
6. 한자/특수문자 키 편집
7. 기본값으로 초기화 (확인 프롬프트)
8. 저장 후 종료
9. 저장하지 않고 종료

`dialoguer` 기반 `Select` / `Input` / `Confirm` 위젯을 사용합니다.

---

## 3. 변환 모드 상세

### 3.1 영어 → 한글 (`--compose`, 기본)

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
4. 줄 단위 반복: keystrokes_to_korean(line, keyboard_map, composer)
5. 출력
```

### 3.2 한글 → 영어 (`--decompose`)

```bash
$ echo "한글" | unim-cli -d
gksrmf
```

내부 흐름:

```
1. 영어/한글 JSON 키맵 로드
2. KeyboardMap 생성
3. 줄 단위 반복: korean_to_keystrokes(line, keyboard_map, is_three_bul)
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

| 분류 | 예시 키 |
|------|---------|
| CLI 설명 | `unim_cli_about`, `compose_desc` |
| 레이아웃 이름 | `twobul_std`, `qwerty` |
| 경고/에러 | `warning_multiple_stdin`, `error_label`, `error_invalid_layout` |
| 설정 라벨 | `settings_title`, `korean_layout_label`, `auto_typefix_label` |
| 설정 변경 메시지 | `config_saved`, `layout_changed`, `auto_typefix_changed` |
| 인터렉티브 | `select_setting`, `confirm_reset`, `save_and_exit` |

---

## 6. 에러 처리

| 상황 | 동작 |
|------|------|
| 변환 I/O 에러 | `stderr`로 `error_label` 메시지 후 exit 1 |
| 잘못된 설정 값 | `error_invalid_*` 메시지 후 exit 1 |
| 설정 저장 실패 | `error_save_failed` 메시지 후 exit 1 |
| 설정 경로 없음 | `error_path_not_found` 후 exit 1 |

설정 파일 접근 권한 오류는 `unim::config` 내부에서 `log_permission_error` 헬퍼가 복구 방법 (`unim-cli config`, `chmod`, `mkdir`)을 안내합니다.

---

## 7. 의존성

| 크레이트 | 용도 |
|----------|------|
| `unim` | 코어 엔진 + 설정 SSoT |
| `clap` (derive) | CLI 인자 파싱, 중첩 서브커맨드 |
| `rust-i18n` | 국제화 |
| `dialoguer` | `config interactive` 프롬프트 위젯 |
| `serde_json` | `app-rules` JSON 파싱 |

---

## 8. 사용 예시

```bash
# ── 변환 ──
echo "gksrmf" | unim-cli                    # → 한글
echo "key_stream" | unim-cli -k 390         # 세벌식 390
echo "한글" | unim-cli -d                    # → gksrmf
unim-cli input.txt -o output.txt            # 파일 변환
cat document.txt | unim-cli -d | sort | uniq

# ── 설정 ──
unim-cli config show
unim-cli config path
unim-cli config set korean-layout 3bul390
unim-cli config set auto-typefix true
unim-cli config set auto-typefix-kor-threshold 4
unim-cli config set toggle-keys "Korean,RightAlt"
unim-cli config set app-rules '[{"app_pattern":"code","default_category":"english"}]'
unim-cli config reset
unim-cli config interactive
```

---

## 9. 빌드 및 설치

```bash
cargo build -p unim-cli --release

# 설치 경로
/usr/bin/unim-cli            # 시스템 설치 (make install PREFIX=/usr)
/usr/local/bin/unim-cli      # 기본 PREFIX
```

이전 버전에서 독립 바이너리로 존재하던 `unim-config`는 `unim-cli config` 서브커맨드로 통합되었습니다.
