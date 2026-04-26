---
name: add-frontend
description: 새 프론트엔드(GTK/Qt/XIM 등) IM 모듈 추가 시 파일 구조, 빌드 설정, DBus 연동 패턴 가이드
---

# 프론트엔드 추가 스킬

새로운 툴킷(GTK/Qt) 또는 프로토콜(XIM/Wayland) 프론트엔드를 추가할 때 사용합니다.

## 프론트엔드 유형

UNIM은 두 가지 유형의 프론트엔드를 지원합니다:

### 유형 A: 툴킷 IM 모듈 (C/C++)

GTK, Qt 등 GUI 툴킷의 입력 모듈로 동작합니다.

**기존 참조 구현**:
- GTK3: `unim-frontends/gtk3/`
- GTK4: `unim-frontends/gtk4/`
- Qt5: `unim-frontends/qt5/`
- Qt6: `unim-frontends/qt6/`

**공통 코드**:
- GTK 공통: `unim-frontends/gtk-common/` (한자 팝업 등)
- Qt 공통: `unim-frontends/qt-common/`

### 유형 B: 헤드리스 프론트엔드 (Rust)

XIM, Wayland 등 프로토콜 수준의 프론트엔드입니다.

**기존 참조 구현**:
- XIM: `unim-frontends/xim/`
- Wayland: `unim-frontends/wayland/`

## 유형 A 추가 절차 (C/C++ 툴킷 모듈)

### 1. 디렉토리 구조 생성

```
unim-frontends/<toolkit>/
├── CMakeLists.txt
└── src/
    ├── immodule.c (또는 .cpp)
    └── unim_dbus_client.c (또는 .cpp)
```

### 2. CMakeLists.txt 작성

```cmake
cmake_minimum_required(VERSION 3.10)
project(<toolkit>-unim)

# 툴킷별 패키지 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(<TOOLKIT> REQUIRED <toolkit-pkg-name>)

# 공유 라이브러리 빌드
add_library(im-unim SHARED
    src/immodule.c
    src/unim_dbus_client.c
)

target_include_directories(im-unim PRIVATE
    ${<TOOLKIT>_INCLUDE_DIRS}
    ${CMAKE_SOURCE_DIR}/../gtk-common/src  # 공통 코드 참조
)

target_link_libraries(im-unim ${<TOOLKIT>_LIBRARIES})
```

### 3. DBus 클라이언트 구현

`unim_dbus_client.c`에 다음 핵심 함수를 구현합니다:

- `unim_dbus_create_context()` - 입력 컨텍스트 생성
- `unim_dbus_destroy_context()` - 입력 컨텍스트 해제
- `unim_dbus_focus_in()` - 포커스 진입
- `unim_dbus_focus_out()` - 포커스 이탈
- `unim_dbus_process_key()` - 키 처리
- `unim_dbus_reset()` - 상태 리셋

DBus 서비스: `org.atit.unim.InputMethod`

### 4. IM 모듈 구현

GTK의 경우 `GtkIMContext`, Qt의 경우 `QPlatformInputContext`를 상속합니다.

핵심 콜백:
- `filter_keypress` - 키 이벤트 필터링 및 DBus 전달
- `focus_in` / `focus_out` - 포커스 관리
- `set_cursor_location` - 커서 위치 전달 (후보창 위치 결정)
- `reset` - preedit 초기화

### 5. Makefile 업데이트

`Makefile`에 빌드 및 설치 타겟을 추가합니다:

- `build-frontends` 타겟에 새 모듈 빌드 추가
- `install-frontends` 타겟에 설치 경로 추가
- `uninstall-frontends` 타겟에 제거 경로 추가
- `clean` 타겟에 빌드 디렉토리 정리 추가

### 6. 로깅

`docs/dev/architecture/GEMINI.md`의 로깅 시스템을 따라 `unim_log_message()` 함수를 구현합니다.

모듈명 규칙: `<TOOLKIT>_IM` (예: `GTK3_IM`, `QT6_IM`)

## 유형 B 추가 절차 (Rust 헤드리스)

### 1. Cargo 프로젝트 생성

```
unim-frontends/<protocol>/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── dbus_client.rs
    └── handler.rs
```

### 2. Cargo.toml 설정

```toml
[package]
name = "unim-<protocol>"
version = "0.0.1"
edition = "2021"

[dependencies]
unim = { path = "../../" }
unim-dbus = { path = "../../unim-dbus" }
```

### 3. Workspace 등록

루트 `Cargo.toml`의 `[workspace] members`에 추가합니다.

### 4. Makefile 업데이트

`install-core` 타겟에 바이너리 설치를 추가합니다.

## 검증

- [ ] `make build-frontends` (또는 `cargo build --workspace`) 성공
- [ ] 새 모듈이 올바른 경로에 설치됨
- [ ] DBus 연결 및 키 처리 동작 확인
- [ ] 로그 출력 정상 (`UNIM_DEVELOP=1` 환경에서)
