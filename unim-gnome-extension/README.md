# unim-autocorrect GNOME Shell Extension

GNOME 셸 환경에서 한/영 자판 입력 오타를 **수동으로 변환**하는 확장 프로그램입니다.

## 📋 기능

### 1. 수동 변환 (Manual Conversion)
- 사용자가 텍스트를 복사(Ctrl+C)한 후 단축키(`<Super>i`)를 누르면 클립보드 내용을 자동 감지하여 한↔영 변환
- 한글이 포함된 텍스트는 영문으로, 영문으로만 구성된 텍스트는 한글로 변환
- 변환 결과는 클립보드에 저장되며, 사용자가 붙여넣기(Ctrl+V)로 사용

### 2. 다양한 키보드 레이아웃 지원
**한글:**
- 2벌식 표준
- 3벌식 390
- 3벌식 391

**영문:**
- QWERTY
- Dvorak

### 3. 설정 UI (Preferences)
- **일반 설정**: 확장 활성화/비활성화, 패널 인디케이터 표시, 알림 표시 여부
- **키보드 레이아웃 설정**: 사용자 맞춤 한글/영문 레이아웃 선택
- **수동 변환 설정**: 단축키 활성화/비활성화 및 단축키 설정

### 4. 알림 기능
- 텍스트 변환 시 변환 전/후 내용을 알림으로 표시
- 설정에서 알림 표시 여부 조절 가능

## 🛠️ 사용 방법

1.  변환하고 싶은 텍스트를 **선택**합니다.
2.  **Ctrl+C**로 복사합니다.
3.  **`<Super>i`** (또는 설정된 단축키)를 누릅니다.
4.  알림에서 변환 결과를 확인합니다.
5.  **Ctrl+V**로 붙여넣습니다.

## 🛠️ 설치

### 빌드
```bash
make build
```

Rust CLI(`unim-cli`)를 빌드하고 GNOME Shell 확장에 필요한 파일을 준비합니다.

### 설치
```bash
make install
```

사용자의 `.local/share/gnome-shell/extensions/` 디렉토리에 설치합니다.

### 활성화
```bash
make enable
```

또는 GNOME Settings에서 "Extensions"로 들어가 수동으로 활성화할 수 있습니다.

> **참고**: 확장이 업데이트된 경우 GNOME Shell을 재시작해야 합니다.
> - X11: `Alt+F2` -> `r` -> `Enter`
> - Wayland: 로그아웃 후 다시 로그인

## 📝 파일 구조

```
unim-gnome-extension/
├── extension.js              # 메인 확장 로직
├── prefs.js                 # 설정 UI 정의
├── unimlib.js               # Rust CLI 호출 래퍼
├── metadata.json            # 확장 메타데이터
├── org.gnome.shell.extensions.unim-autocorrect.gschema.xml
│                           # GSettings 스키마 정의
├── schemas/                 # 컴파일된 스키마 저장소
│   └── gschema.compiled
├── bin/                     # Rust CLI 바이너리
│   └── unim-cli
└── README.md               # 이 파일
```

## 🔧 설정 (Settings)

### GSettings 스키마
확장의 모든 설정은 다음 스키마를 통해 관리됩니다:
- Schema ID: `org.gnome.shell.extensions.unim-autocorrect`

### 사용 가능한 설정

| 설정 ID | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `enable-extension` | boolean | true | 확장 활성화 여부 |
| `show-indicator` | boolean | true | 패널 인디케이터 표시 여부 |
| `show-notification` | boolean | true | 알림 표시 여부 |
| `korean-layout` | string | '2bul' | 한글 레이아웃 ('2bul', '390', '391') |
| `english-layout` | string | 'qwerty' | 영문 레이아웃 ('qwerty', 'dvorak') |
| `enable-manual-conversion` | boolean | true | 수동 변환 활성화 여부 |
| `manual-conversion-shortcut` | string | '' | 수동 변환 단축키 (예: `<Super>i`) |

## 🐛 알려진 제한사항

1.  **Wayland 보안 모델**: 다른 앱의 텍스트를 직접 제어할 수 없어 클립보드 기반 워크플로우를 사용합니다.
2.  **CLI 바이너리 필요**: `bin/unim-cli`가 없으면 확장이 동작하지 않습니다.

## 🔍 디버깅

### 로그 확인
```bash
make log
```

### 설정 확인
```bash
GSETTINGS_SCHEMA_DIR=~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/schemas \
gsettings get org.gnome.shell.extensions.unim-autocorrect manual-conversion-shortcut
```

## 📋 라이선스

이 확장은 unim 프로젝트의 일부입니다.
