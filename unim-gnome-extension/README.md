# unim-autocorrect GNOME Shell Extension

GNOME 셸 환경에서 한/영 자판 입력 오타를 자동으로 감지하고 수정하는 확장 프로그램입니다.

## 📋 기능

### 1. 자동 변환 (Automatic Conversion)
- 사용자가 영문으로 입력해야 할 때 한글로 입력하거나, 한글을 입력해야 할 때 영문으로 입력한 경우 자동으로 감지
- 스페이스바 또는 엔터 키 입력 시 마지막 단어를 자동으로 변환
- 설정에서 자동 변환 활성화/비활성화 가능

### 2. 입력기 상태 동기화
- 텍스트가 한글에서 영문으로 변환되면 GNOME의 입력 소스도 자동으로 영문으로 전환
- 영문에서 한글로 변환되면 한글 입력기로 자동 전환
- 사용자의 추가 조작 없이 원활한 입력 경험 제공

### 3. 다양한 키보드 레이아웃 지원
**한글:**
- 2벌식 표준
- 3벌식 390
- 3벌식 391

**영문:**
- QWERTY
- Dvorak

### 4. 설정 UI (Preferences)
- **일반 설정**: 확장 전체 활성화/비활성화
- **자동 변환 설정**: 자동 변환 활성화/비활성화, 알림 표시 여부
- **키보드 레이아웃 설정**: 사용자 맞춤 한글/영문 레이아웃 선택
- **수동 변환 설정**: 수동 변환 활성화/비활성화, 단축키 설정 (개발 예정)

### 5. 알림 기능
- 텍스트 변환 시 시각적 피드백 제공
- 설정에서 알림 표시 여부 조절 가능

## 🛠️ 설치

### 빌드
```bash
make build
```

Rust 라이브러리(`unim-core`)를 빌드하고 GNOME Shell 확장에 필요한 파일을 준비합니다.

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

## 📝 파일 구조

```
unim-gnome-extension/
├── extension.js              # 메인 확장 로직
├── prefs.js                 # 설정 UI 정의
├── metadata.json            # 확장 메타데이터
├── org.gnome.shell.extensions.unim-autocorrect.gschema.xml
│                           # GSettings 스키마 정의
├── schemas/                 # 컴파일된 스키마 저장소
│   └── gschema.compiled
├── lib/                     # Rust 라이브러리
│   └── libunim_core.so
└── README.md               # 이 파일
```

## 🔧 설정 (Settings)

### GSettings 스키마
확장의 모든 설정은 다음 스키마를 통해 관리됩니다:
- Schema ID: `org.gnome.shell.extensions.unim-autocorrect`
- 저장 위치: `~/.config/dconf/user`

### 사용 가능한 설정

| 설정 ID | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `enable-extension` | boolean | true | 확장 활성화 여부 |
| `enable-automatic-conversion` | boolean | true | 자동 변환 활성화 여부 |
| `show-notification` | boolean | true | 알림 표시 여부 |
| `korean-layout` | string | '2bul' | 한글 레이아웃 ('2bul', '390', '391') |
| `english-layout` | string | 'qwerty' | 영문 레이아웃 ('qwerty', 'dvorak') |
| `enable-manual-conversion` | boolean | true | 수동 변환 활성화 여부 (개발 예정) |
| `manual-conversion-shortcut` | string | '' | 수동 변환 단축키 (개발 예정) |

## 📌 PRD 요구사항 대응 현황

### ✅ 구현 완료
- [x] 실시간 오타 감지 및 변환
- [x] 입력 소스 자동 전환
- [x] 다양한 키보드 레이아웃 지원 (2벌식, 3벌식 390, 3벌식 391, QWERTY, Dvorak)
- [x] 설정 UI (일반, 자동 변환, 키보드 레이아웃 설정)
- [x] 알림 기능
- [x] GSettings 기반 설정 관리
- [x] 안정적인 신호/이벤트 처리

### 🔄 향후 구현 예정
- [ ] 수동 변환 기능 (단축키 기반)
- [ ] 더 정교한 자모 조합 로직
- [ ] 선택 텍스트 변환 기능
- [ ] Wayland 환경 최적화

## 🐛 알려진 제한사항

1. **Wayland 환경**: X11과 달리 Wayland에서는 일부 키 입력 감지 기능에 제약이 있을 수 있습니다.
2. **일부 애플리케이션**: Gtk 기반이 아닌 특수한 텍스트 입력 위젯은 동작하지 않을 수 있습니다.
3. **FFI 라이브러리 로드**: libunim_core.so 로드 실패 시 확장이 동작하지 않습니다.

## 🔍 디버깅

### 로그 확인
```bash
make log
```

또는 직접 확인:
```bash
journalctl -f -o cat /usr/bin/gnome-shell | grep unim
```

### 설정 확인
```bash
gsettings get org.gnome.shell.extensions.unim-autocorrect enable-extension
```

### 설정 초기화
```bash
dconf reset -f /org/gnome/shell/extensions/unim-autocorrect/
```

## 🔗 Rust FFI 인터페이스

확장은 다음 FFI 함수를 호출합니다:

```c
// 문자열 변환
char* transform_string(const char* input, const char* fromLayout, const char* toLayout);

// 메모리 해제
void free_string(char* ptr);
```

**레이아웃 이름 형식:**
- Korean: `ko_2bulstd`, `ko_3bul390`, `ko_3bul391`
- English: `en_qwerty`, `en_dvorak`

## 📋 라이선스

이 확장은 unim 프로젝트의 일부입니다.

## 🙋 기여

버그 리포트 및 기능 제안은 프로젝트의 이슈 트래커를 통해 보고해주세요.
