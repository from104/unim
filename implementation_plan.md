# 작업 계획: Latin 명칭을 English로 변경

전체 소스 코드에서 입력 모드 명칭인 `Latin`을 `English`로, `latin`을 `english`로 변경합니다. 이는 사용자 인터페이스와 코드 내부 로직에서 일관성을 유지하기 위함입니다.

## 작업 범위

### 1. 코어 라이브러리 (`src/`)
- `src/status.rs`:
    - `InputCategory::Latin` -> `InputCategory::English`
    - 문자열 변환 로직 ("latin" -> "english") 수정
- `src/config.rs`:
    - `InputCategory::Latin` -> `English`
    - `LatinLayout` -> `EnglishLayout`
    - `LatinConfig` -> `EnglishConfig`
    - `EngineConfig` 내의 `latin` 필드 -> `english`
    - 관련 테스트 코드 및 기본값 설정 수정

### 2. DBus 인터페이스 (`unim-dbus/`)
- `unim-dbus/src/interfaces.rs`:
    - `InputMode::Latin` -> `InputMode::English`
    - 코어 라이브러리와의 변환 로직 수정
- `unim-dbus/src/service.rs`: 관련 필드 및 메서드 호출 수정
- `unim-dbus/src/engine_worker.rs`: 모드 비교 로직 수정

### 3. 인디케이터 (`unim-indicator/`)
- `unim-indicator/src/main.rs`:
    - 변수 명칭 변경 (예: `latin_btn` -> `english_btn`)
    - 레이블 및 툴팁 텍스트 수정 ("Latin" -> "English", "영문 모드 (Latin)" -> "영문 모드 (English)")
    - 아이콘 이름 및 CSS 클래스 수정 ("unim-latin" -> "unim-english")

### 4. 리소스 및 빌드 시스템
- 아이콘 파일명 변경: `unim-indicator/data/icons/hicolor/scalable/apps/unim-latin.svg` -> `unim-english.svg`
- `Makefile`: 아이콘 설치 및 삭제 경로 수정

## 작업 순서

1. 코어 라이브러리 수정 및 빌드 확인
2. DBus 모듈 수정
3. 인디케이터 수정
4. 아이콘 파일 이름 변경 및 빌드 시스템 수정
5. 전체 다시 빌드 및 동작 확인
