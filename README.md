# UNIM: 차세대 범용 입력기 (Universal Next-generation Input Method)

**UNIM**은 Rust로 작성된 오픈 소스 한국어 입력기 엔진(IME)입니다. 모든 주요 플랫폼에서 한국어와 영어 사용자에게 원활하고 고성능이며 확장이 가능한 타이핑 경험을 제공하는 것을 목표로 합니다.

## 🚀 최종 비전

UNIM의 최종 목표는 다음과 같은 기능을 갖춘 한국어/영어 텍스트 처리 및 입력을 위한 **완벽한 크로스 플랫폼 솔루션**이 되는 것입니다.

1. **자동 상태 전환**: 문맥에 따라 한국어와 영어 모드를 지능적으로 감지하고 전환합니다.
2. **범용 변환**: 잘못 입력된 텍스트(영타를 한글로, 또는 그 반대)를 단축키를 통해 손쉽게 변환합니다.

## 🛠️ 현재 상태

현재 프로젝트는 다음과 같이 구성되어 있습니다.

### 1. [UNIM Core](src/): UNIM의 심장

순수 **Rust**로 작성된 핵심 라이브러리인 코어는 모든 한국어 조합 및 분해 로직(2벌식, 3벌식 390, 391 표준)을 처리합니다. 현재 의존성이 없고 자산이 내장된 구조로 설계되었습니다.

### 2. [unim-cli](unim-cli/): 독립형 엔진

코어 로직에 대한 이식 가능한 명령줄 인터페이스(CLI)입니다. 독립형 변환기로 사용하거나 다른 통합을 위한 백엔드로 사용할 수 있습니다.

### 3. [GNOME Shell 확장](unim-gnome-extension/): 리눅스 네이티브 통합

단축키를 사용하여 잘못된 키보드 레이아웃으로 입력된 텍스트(예: 'gksrmf' ↔ '한국어')를 수정하는 GNOME용 확장 기능입니다. 터미널 인식 붙여넣기 및 복사 전용 모드를 지원합니다.

## 🏗️ 시스템 아키텍처 및 동작 원리

UNIM은 고성능과 확장성을 위해 **3계층 구조(3-Layered Architecture)**를 채택하고 있습니다. 특히 DBus를 통해 모든 입력 프론트엔드와 코어 엔진이 유기적으로 통신합니다.

### 1. 전체 구조도

- **Core Engine (Rust)**: 한글 조합/분해 로직이 담긴 순수 Rust 라이브러리 (`src/`).
- **DBus Layer (unim-daemon)**: 시스템 전반의 입력 상태를 관리하고 프론트엔드의 요청을 처리하는 중앙 서버.
- **Frontend / IM Modules**: 각 애플리케이션(GTK, Qt, XIM, Wayland)에서 동작하는 클라이언트 모듈.

### 2. DBus 통신 매커니즘

DBus는 UNIM 시스템의 **중추신경계** 역할을 하며 다음과 같이 동작합니다:

1. **중앙 집중식 관리 (`unim-daemon`)**:
    - `unim-daemon`이 실행되면 시스템 세션 버스에 `org.atit.unim.InputMethod` 서비스를 등록합니다.
    - 엔진 코어는 스레드 안전성(`Send+Sync`) 문제로 인해 별도의 **Worker Thread**에서 고립되어 동작하며, DBus 요청은 비동기 채널을 통해 이 스레드로 전달됩니다.
2. **가상 입력 컨텍스트 (Input Context)**:
    - 각 애플리케이션(창)이 포커스를 받으면 DBus를 통해 자신만의 `입력 컨텍스트`를 할당받습니다.
    - 이를 통해 여러 창에서 서로 간섭 없이 독립적인 한글 조합 상태(preedit)를 유지할 수 있습니다.
3. **이벤트 흐름 (Event Flow)**:
    - **입력**: `사용자 키 입력` → `IM 모듈 (클라이언트)` → `DBus` → `unim-daemon (서버)` → `코어 엔진`.
    - **응답**: `결과 생성 (Commit/Preedit)` → `DBus 시그널` → `IM 모듈` → `애플리케이션 화면 출력`.
4. **전역 상태 동기화 (Global Sync)**:
    - 한 창에서 한/영 모드를 바꾸면 `unim-daemon`이 `GlobalModeChanged` 시그널을 방송합니다.
    - `unim-indicator`(트레이 아이콘)와 다른 모든 입력 모듈들이 이 시그널을 수신하여 즉시 UI와 내부 상태를 동기화합니다.

### 3. C-API 및 라이브러리 연동

- **`unim-capi`**: Rust 코어를 C 언어에서 사용할 수 있도록 래핑한 계층입니다.
- 설정 도구(`unim-config`)나 일부 성능이 중요한 툴킷 모듈은 DBus 대신 이 C-API를 통해 엔진 데이터에 직접 접근하거나 설정을 관리합니다.

### 4. 데몬 관리 및 Systemd 통합

`unim-daemon`은 PID 파일 기반 싱글톤 관리와 systemd 사용자 서비스 통합을 지원합니다.

#### 명령줄 옵션

```bash
unim-daemon [OPTIONS]
  -n, --no-daemon  포그라운드 실행 (데몬화 없이)
  -r, --replace    기존 데몬 강제 종료 후 교체
      --check      실행 여부 확인 (exit 0=실행중, 1=미실행)
```

#### Systemd 사용자 서비스

```bash
# 서비스 파일 설치
sudo make install-systemd PREFIX=/usr

# 서비스 활성화 및 시작
systemctl --user daemon-reload
systemctl --user enable --now unim-daemon.service

# 상태 확인
systemctl --user status unim-daemon
```

---

## 🗺️ 장기 로드맵

1. **1단계 (현재)**: Rust 코어 안정화 및 GNOME Shell 확장 기능 구현.
2. **2단계 (지능화)**: 문맥 인식 기반의 **자동 한/영 전환 알고리즘** 구현.

## 📚 예제

`examples/` 디렉토리에는 UNIM 라이브러리를 시작하는 데 도움이 되는 몇 가지 예제가 포함되어 있습니다.

- **[입력 시뮬레이션 (2벌식)](examples/input_simulation_2bul.rs)**: 2벌식 표준이 실시간 조합 및 "도깨비불" 현상을 어떻게 처리하는지 확인하세요.
- **[입력 시뮬레이션 (3벌식)](examples/input_simulation_3bul.rs)**: 3벌식 레이아웃 처리 이면의 로직을 탐구합니다.
- **[자모 패턴 검색](examples/jamo_pattern_search.rs)**: 텍스트를 자모 단위로 분해하여 퍼지 검색을 수행하는 고급 예제입니다.
- **[문자열 처리](examples/string_processing.rs)**: 한글 음절을 초성, 중성, 종성으로 분해하는 기본 기능을 보여줍니다.
- **[음절 매트릭스](examples/mk_korean.rs)**: 한글 음절 전체 범위를 프로그래밍 방식으로 생성합니다.

예제 실행 방법:

```bash
cargo run --example string_processing
```

---

GNOME 확장의 자세한 설치 및 사용 방법은 [unim-gnome-extension/README.md](unim-gnome-extension/README.md)를 참조하세요.

장기 개발 계획은 [ROADMAP.md](ROADMAP.md)를 참조하세요.
