# unim-autocorrect 구현 명세서

## 개요

이 문서는 PRD 요구사항에 따라 `unim-autocorrect` GNOME Shell 확장을 구현한 내용을 설명합니다.

## PRD 요구사항 대응표

| 요구사항 | 상태 | 구현 내용 | 파일 |
|---------|------|---------|------|
| 실시간 오타 감지 | ✅ | `onKeyPress()` 메서드로 키 입력 감시 | extension.js |
| 자동 텍스트 변환 | ✅ | 스페이스/엔터 키에서 변환 실행 | extension.js |
| 입력기 상태 동기화 | ✅ | `switchInputSource()` 메서드로 입력기 전환 | extension.js |
| 다양한 키보드 레이아웃 지원 | ✅ | 2벌식, 3벌식 390/391, QWERTY, Dvorak 지원 | extension.js |
| 설정 UI 제공 | ✅ | Adwaita 기반 설정 패널 제공 | prefs.js |
| 알림 메시지 표시 | ✅ | `showNotification()` 메서드로 알림 구현 | extension.js |
| GSettings 기반 설정 | ✅ | GSettings 스키마 정의 및 바인딩 | gschema.xml |

## 주요 구현 내용

### 1. extension.js (메인 로직)

#### 라이브러리 초기화 (UnimLib)
```javascript
const UnimLib = {
    lib: null,
    transform_string: null,
    free_string: null,
    
    init(path)           // 라이브러리 로드
    transformString()    // 문자열 변환
    close()             // 라이브러리 해제
}
```

**특징:**
- ctypes를 통한 FFI 함수 선언
- 안전한 C 문자열 변환 (ctypes.char.array())
- 메모리 누수 방지 (resultPtr.isNull() 확인)
- 예외 처리 강화

#### 확장 클래스 구조
```javascript
export default class UnimAutocorrectExtension extends Extension {
    constructor()       // 초기화
    enable()           // 확장 활성화
    disable()          // 확장 비활성화
    onKeyPress()       // 키 이벤트 처리
    performConversion() // 문자열 변환 수행
}
```

#### 핵심 메서드

**`enable()`**
- GSettings 스키마 로드
- Rust 라이브러리 초기화
- 키보드 이벤트 신호 연결
- 포커스 변경 신호 연결

**`onKeyPress(actor, event)`**
- 확장 활성화 여부 확인
- 텍스트 입력 위젯 검증
- 키 코드 분석
- 스페이스/엔터 키에서 변환 트리거

**`performConversion(focusedWidget, isManual)`**
- 현재 텍스트 추출
- 마지막 단어 분리
- 현재 레이아웃 감지
- Rust FFI를 통한 변환
- 텍스트 대체 및 입력기 전환
- 알림 표시

#### 레이아웃 관리 메서드

**`getLayoutFromSource(source)`**
- 입력 소스 ID에서 레이아웃 결정
- 한글/영문 구분
- 설정된 특정 레이아웃 적용

**`getSelectedKoreanLayout()`**
- GSettings에서 한글 레이아웃 조회
- 2벌식, 3벌식 390, 3벌식 391 지원

**`getSelectedEnglishLayout()`**
- GSettings에서 영문 레이아웃 조회
- QWERTY, Dvorak 지원

**`getOppositeLayout(layout)`**
- 현재 레이아웃의 반대 레이아웃 반환
- 한글 ↔ 영문 전환

#### 입력기 전환 메서드

**`switchInputSource(text, inputSourceManager)`**
- 변환된 텍스트 분석 (한글/영문 구분)
- 적절한 입력 소스 검색
- 입력 소스 활성화

**`isTextInputWidget(widget)`**
- St.Entry 인스턴스 확인
- clutter_text 속성 확인
- 위젯 이름 분석

#### 설정 확인 메서드

**`isExtensionEnabled()`** - 확장 전체 활성화 여부
**`isAutomaticConversionEnabled()`** - 자동 변환 활성화 여부
**`isNotificationEnabled()`** - 알림 표시 여부

모두 GSettings에서 설정값을 조회합니다.

#### 알림 메서드

**`showNotification(message)`**
- Main.notify()를 통한 GNOME 알림
- "변환 전 → 변환 후" 형식의 메시지

### 2. prefs.js (설정 UI)

#### 구조
```javascript
export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window)  // 설정 패널 구성
    openShortcutDialog()          // 단축키 설정 다이얼로그
    getKeyName(keyval)            // 키 코드 → 키 이름 변환
}
```

#### 설정 그룹

**일반 설정 (General Settings)**
- 확장 전체 활성화/비활성화 토글

**자동 변환 (Automatic Conversion)**
- 자동 변환 활성화/비활성화
- 알림 표시 여부

**키보드 레이아웃 (Keyboard Layouts)**
- 한글 레이아웃 선택 (콤보박스)
  - 2-Bul Standard
  - 3-Bul 390
  - 3-Bul 391
- 영문 레이아웃 선택 (콤보박스)
  - QWERTY
  - Dvorak

**수동 변환 (Manual Conversion)** - 개발 예정
- 수동 변환 활성화/비활성화
- 단축키 설정 버튼

#### 설정 바인딩

GSettings와 UI 위젯 자동 동기화:
```javascript
settings.bind(
    'setting-key',
    widget,
    'active/selected',
    Gio.SettingsBindFlags.DEFAULT
);
```

#### 단축키 설정 다이얼로그 (향후 구현)

- 키보드 이벤트 감시
- 수정자 키 조합 감지 (Ctrl, Alt, Shift, Super)
- 키 이름 매핑 (keyval → 'A', 'F1' 등)
- 설정 저장 및 버튼 레이블 업데이트

### 3. GSettings 스키마 (org.gnome.shell.extensions.unim-autocorrect.gschema.xml)

#### 정의된 설정

| 키 | 타입 | 기본값 | 설명 |
|-----|------|--------|------|
| `enable-extension` | boolean | true | 확장 활성화 |
| `enable-automatic-conversion` | boolean | true | 자동 변환 |
| `show-notification` | boolean | true | 알림 표시 |
| `korean-layout` | string | '2bul' | 한글 레이아웃 |
| `english-layout` | string | 'qwerty' | 영문 레이아웃 |
| `enable-manual-conversion` | boolean | true | 수동 변환 (예정) |
| `manual-conversion-shortcut` | string | '' | 단축키 (예정) |

#### 스키마 컴파일

Makefile에서 `glib-compile-schemas` 명령으로 컴파일:
```bash
glib-compile-schemas unim-gnome-extension/schemas
```

### 4. Makefile 개선사항

```makefile
build:
    # Rust 라이브러리 빌드
    # 라이브러리 복사
    # GSettings 스키마 컴파일

install:
    # 파일 복사
    # 스키마 컴파일
    # 확장 활성화 준비

clean:
    # 빌드 산출물 정리
    # 스키마 디렉토리 정리
```

### 5. metadata.json 업데이트

```json
{
    "settings-schema": "org.gnome.shell.extensions.unim-autocorrect"
}
```

GNOME이 설정 UI를 찾을 수 있도록 스키마 ID 지정

## 기술적 특징

### 안정성
- ✅ 신호 해제 (disconnect) 구현
- ✅ Null 포인터 확인
- ✅ 예외 처리 (try-catch)
- ✅ 리소스 정리 (disable 메서드)

### 성능
- ✅ 조건부 이벤트 처리 (활성화 여부 확인)
- ✅ 효율적인 텍스트 추출 (마지막 단어만 처리)
- ✅ 메모리 누수 방지 (FFI 메모리 해제)

### 사용자 경험
- ✅ 자동 입력기 전환 (투명한 경험)
- ✅ 시각적 피드백 (알림)
- ✅ 직관적인 설정 UI
- ✅ 다양한 레이아웃 지원

### 호환성
- ✅ GNOME 46 이상 지원
- ✅ GSettings 기반 설정 (D-Bus 미사용)
- ✅ Adwaita GTK4 기반 UI

## 향후 개선 계획

### 1단계: 수동 변환 기능
- 단축키 기반 선택 텍스트 변환
- 단축키 동적 바인딩

### 2단계: 고급 기능
- 선택 영역 자동 감지
- 이전 변환 히스토리 기억
- 예외 사전 관리

### 3단계: 환경 최적화
- Wayland 환경 지원 개선
- 다양한 애플리케이션 호환성
- 성능 최적화

## 테스트 검증 항목

### 기능 테스트
- [ ] 자동 변환 작동 확인
- [ ] 입력기 전환 확인
- [ ] 알림 표시 확인
- [ ] 설정 변경 적용 확인

### 호환성 테스트
- [ ] 다양한 텍스트 입력 위젯
- [ ] 다양한 애플리케이션
- [ ] GNOME 버전 호환성

### 안정성 테스트
- [ ] 반복 입력 테스트
- [ ] 메모리 누수 검사
- [ ] 예외 상황 처리

## 참고 자료

- GNOME Shell Extension Development: https://wiki.gnome.org/Devel/Papers/ShellExtensions
- GSettings Documentation: https://developer.gnome.org/gio/stable/GSettings.html
- Rust FFI via ctypes: https://gitlab.gnome.org/GNOME/gjs/-/wikis/FFI
