# UNIM Config (unim-config) 세부 기능 명세

> `unim-config`는 **UNIM 설정 관리 CLI 도구**입니다.
> 서브커맨드 기반으로 설정을 조회/변경/초기화하며,
> 인터렉티브 TUI 모드도 지원합니다.
> 코어 엔진의 `Config` 구조체를 직접 사용하여 `~/.config/unim/config.yaml`을 관리합니다.

---

## 1. 아키텍처

```
사용자 ──▶ unim-config ──▶ ~/.config/unim/config.yaml
               │
               ├── unim::config (Config 구조체)
               ├── clap (서브커맨드 파싱)
               ├── dialoguer (인터렉티브 TUI)
               └── rust-i18n (국제화)
```

### 파일 구조

```
unim-config/
├── Cargo.toml
├── src/
│   └── main.rs         # CLI + 인터렉티브 로직 (441행)
└── locales/
    ├── ko.yml          # 한국어 번역 (79개 키)
    └── en.yml          # 영어 번역
```

---

## 2. CLI 인터페이스

```
unim-config [COMMAND]
```

### 2.1 서브커맨드

| 커맨드 | 설명 |
|--------|------|
| *(없음)* | `show`와 동일 + 도움말 힌트 |
| `show` | 현재 설정 전체 표시 |
| `set <KEY> <VALUE>` | 설정 값 변경 |
| `path` | 설정 파일 경로 출력 |
| `reset` | 모든 설정을 기본값으로 초기화 |
| `interactive` | 인터렉티브 TUI 모드 |

---

## 3. 설정 항목 (`ConfigKey`)

### 3.1 `korean-layout`

```bash
unim-config set korean-layout 2bul
```

| 허용 값 | 별칭 | 매핑 |
|---------|------|------|
| `2bul` | `dubeolsik` | `KoreanLayout::Dubeolsik` |
| `3bul390` | `390` | `KoreanLayout::Sebeolsik390` |
| `3bul391` | `391` | `KoreanLayout::Sebeolsik391` |
| `3bul_noshift` | `noshift` | `KoreanLayout::SebeolsikNoShift` |

### 3.2 `english-layout`

```bash
unim-config set english-layout dvorak
```

| 허용 값 | 별칭 | 매핑 |
|---------|------|------|
| `qwerty` | — | `EnglishLayout::Qwerty` |
| `dvorak` | — | `EnglishLayout::Dvorak` |
| `colemak` | — | `EnglishLayout::Colemak` |
| `colemak_dh` | `colemak-dh` | `EnglishLayout::ColemakDh` |
| `workman` | — | `EnglishLayout::Workman` |

### 3.3 `default-category`

```bash
unim-config set default-category korean
```

| 허용 값 | 별칭 |
|---------|------|
| `korean` | `ko`, `한글`, `한국어` |
| `english` | `en`, `영어` |

### 3.4 `mode-sharing`

```bash
unim-config set mode-sharing per-app
```

| 허용 값 | 별칭 |
|---------|------|
| `global` | `전역` |
| `per-app` | `perapp`, `앱별` |
| `per-window` | `perwindow`, `창별` |

### 3.5 `auto-switch`

```bash
unim-config set auto-switch true
```

| 허용 값 (on) | 허용 값 (off) |
|-------------|-------------|
| `true`, `on`, `1`, `yes` | `false`, `off`, `0`, `no` |

### 3.6 `auto-switch-threshold`

```bash
unim-config set auto-switch-threshold 0.7
```

- 범위: `0.0` ~ `1.0` (범위 외 값은 에러)

### 3.7 `toggle-keys`

```bash
unim-config set toggle-keys "Korean,RightAlt"
```

- 쉼표 구분된 `KeyCode` 이름 목록
- 허용 값: `Korean`, `RightAlt`, `Hangul`, `F10` 등 (`KeyCode::from_name()` 참조)
- 기본값: `Korean,RightAlt`

### 3.8 `hanja-keys`

```bash
unim-config set hanja-keys "Hanja,F9"
```

- 쉼표 구분된 `KeyCode` 이름 목록
- 허용 값: `Hanja`, `F9` 등 (`KeyCode::from_name()` 참조)
- 기본값: `Hanja,F9`

---

## 4. 서브커맨드 상세

### 4.1 `show`

```
$ unim-config show

UNIM 입력기 설정
================
한국어 레이아웃: 두벌식 표준 (2bul)
영어 레이아웃: QWERTY (qwerty)
초기 입력 모드: 한국어
모드 공유 방식: Global (전역 공유)
자동 전환: 비활성화
자동 전환 임계값: 0.50

설정 파일: /home/user/.config/unim/config.yaml
```

### 4.2 `set`

```
$ unim-config set korean-layout 3bul390
한국어 레이아웃 레이아웃을 '3bul390'(으)로 변경했습니다.
설정이 저장되었습니다.
```

흐름: 설정 로드 → 값 파싱 → 필드 변경 → `save_to_default_path()` → 확인 메시지

### 4.3 `path`

```
$ unim-config path
/home/user/.config/unim/config.yaml
```

### 4.4 `reset`

기본 설정으로 초기화한 후 저장하고 `show` 실행합니다.

### 4.5 `interactive`

인터렉티브 TUI 모드 — 터미널에서 화살표 키로 설정 항목을 탐색하고 변경합니다.

```
┌─ 설정할 항목을 선택하세요 ─────────────────┐
│ ▸ 한국어 레이아웃                           │
│   영어 레이아웃                             │
│   초기 입력 모드                            │
│   모드 공유 방식                            │
│   자동 전환                                │
│   자동 전환 임계값                          │
│   설정을 기본값으로 초기화                    │
│   저장 및 종료                              │
│   저장하지 않고 종료                         │
└─────────────────────────────────────────────┘
```

| 항목 | 위젯 | 설명 |
|------|------|------|
| 한국어 레이아웃 | `Select` | 4개 선택지, 현재 값 기본 선택 |
| 영어 레이아웃 | `Select` | 5개 선택지 |
| 초기 입력 모드 | `Select` | 한국어/영어 |
| 모드 공유 방식 | `Select` | Global/PerApp/PerWindow |
| 자동 전환 | `Confirm` | Yes/No |
| 자동 전환 임계값 | `Input` | 숫자 입력 (0.0~1.0 검증) |
| 기본값 초기화 | `Confirm` | 확인 후 리셋 |
| 저장 및 종료 | — | `save_to_default_path()` 후 종료 |
| 저장하지 않고 종료 | — | 변경사항 폐기 |

> [!NOTE]
> 인터렉티브 모드는 루프 구조로, 항목 선택 후 화면을 클리어하고 현재 설정을 다시 표시합니다.
> 저장/종료를 선택할 때까지 반복됩니다.

---

## 5. 국제화 (i18n)

### 5.1 로케일 결정

```
LANG / LC_ALL → "ko_KR.UTF-8" → "ko"
```

### 5.2 번역 범위

| 분류 | 키 수 | 예시 |
|------|-------|------|
| UI 레이블 | 12 | `settings_title`, `korean_layout_label` |
| 변경 확인 | 6 | `config_saved`, `layout_changed` |
| 에러 메시지 | 6 | `error_invalid_layout`, `error_save_failed` |
| 인터렉티브 프롬프트 | 9 | `select_setting`, `confirm_reset` |
| 기타 | 5+ | `help_hint`, `korean_mode` |

한국어 입력값도 허용합니다 (`한글`, `전역`, `앱별`, `창별`).

---

## 6. 에러 처리

| 상황 | 동작 |
|------|------|
| 잘못된 레이아웃 값 | 허용 값 목록과 함께 에러 출력 |
| 잘못된 임계값 (범위 외) | `0.0 ~ 1.0` 범위 안내 |
| 설정 저장 실패 | 에러 사유 출력 |
| 설정 파일 경로 없음 | `error_path_not_found` 메시지 |
| 잘못된 서브커맨드 | clap 자동 에러 + help 출력 |

모든 에러는 `stderr`로 출력되며 종료 코드 `1`을 반환합니다.

---

## 7. 의존성

| 크레이트 | 용도 |
|----------|------|
| `unim` | `Config`, `KoreanLayout`, `EnglishLayout` 등 |
| `clap` (derive) | 서브커맨드/인자 파싱 |
| `dialoguer` | 인터렉티브 TUI (Select, Confirm, Input) |
| `rust-i18n` | 국제화 |

---

## 8. 빌드 및 설치

```bash
# 빌드
cargo build -p unim-config

# 설치 경로 (Makefile 기준)
/usr/lib/unim/unim-config
```
