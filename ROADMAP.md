# UNIM 프로젝트 로드맵

이 문서는 **UNIM** 프로젝트의 장기 목표와 개발 단계별 계획을 설명합니다.

## 🎯 핵심 목표

언어 상태 자동 감지 및 수동 텍스트 변환 기능을 갖춘, 하나로 통합된 크로스 플랫폼(Windows, macOS, Linux) 한국어 입력기 엔진(IME)을 구축하는 것입니다.

## 🛣️ 개발 단계

### 1단계: 기반 구축 및 리눅스 네이티브 (완료)

- [x] 한글 조합 로직을 갖춘 견고한 Rust 코어 라이브러리 개발.
- [x] 데이터 자산이 내장된 이식 가능한 `unim-cli` 구현.
- [x] `St.Clipboard`와 `Clutter`를 사용한 네이티브 GNOME Shell 확장 프로그램 개발.
- [x] 안정성을 위한 하이브리드 아키텍처(CLI + 네이티브 API) 적용.

### 2단계: 3계층 아키텍처 및 전체 프론트엔드 (완료)

- [x] **DBus 데몬 아키텍처**: `unim-daemon` + `unim-dbus` 기반 중앙 엔진 서비스 구축.
- [x] **GTK3/GTK4 IM 모듈**: C 언어 기반 IM Module 구현 (공통 코드 `gtk-common` 분리).
- [x] **Qt5/Qt6 플러그인**: C++ 기반 QPlatformInputContext 플러그인 구현 (공통 코드 `qt-common` 분리).
- [x] **XIM 프론트엔드**: Rust `xim` crate 기반 X11 XIM 서버 구현 (Over-The-Spot Preedit, 프로토콜 적합성 검증 완료).
- [x] **Wayland 프론트엔드**: `input-method-v2` + `virtual-keyboard-v1` 프로토콜 기반 구현 (KDE Plasma 지원).
- [x] **한자/특수문자 입력**: X11 Xft 팝업 (XIM), GTK/Qt 팝업 (IM 모듈) 기반 한자 및 특수문자 후보 선택.
- [x] **설정 도구**: GTK/Qt GUI 설정 도구 (`unim-gtk-settings`, `unim-qt-settings`) + CLI (`unim-config`).
- [x] **시스템 트레이**: `unim-gui` 트레이 아이콘 및 팝업 통합.

### 3단계: 문서화 및 안정화 (진행 중)

- [x] **컴포넌트별 SPEC.md 작성**: 12개 컴포넌트 기능 명세 문서화.
  - `src/`, `unim-capi/`, `unim-cli/`, `unim-config/`, `unim-daemon/`, `unim-dbus/`
  - `unim-frontends/gtk3/`, `gtk4/`, `qt5/`, `qt6/`, `xim/`, `wayland/`
- [x] **XIM 프로토콜 적합성 검증**: [XIM 사양](https://www.x.org/releases/X11R7.6/doc/libX11/specs/XIM/xim.html) 대비 3회 교차 검증 (11개 항목 적합).
- [x] **Wayland 프로토콜 참조 문서화**: `input-method-v2`, `virtual-keyboard-v1` 프로토콜 사양 참조 및 아키텍처 문서화.
- [ ] **Wayland 키 반복(Key Repeat)**: `mio` + `timerfd` 기반 구현 (Phase 2).
- [ ] **Wayland 한자/특수문자 팝업**: Layer-Shell 또는 팝업 서피스 기반 (Phase 3).
- [ ] **Surrounding Text / Content Type**: Wayland 프로토콜 이벤트 활용 (Phase 4).
- [ ] **Debian 패키지 안정화**: 패키지 빌드/설치 프로세스 검증 및 개선.

### 4단계: 자동 상태 전환 (지능화)

- [ ] **문맥 감지**: 현재 입력 필드 상태나 언어 문맥을 감지하는 방법 연구.
- [ ] **자동 교정 엔진**: 실시간 "오타" 감지 구현 (예: `gksrmf` 입력 시 타이핑 중에 자동으로 `한글`로 변환).
- [ ] **사용자 학습**: 사용자별 타이핑 패턴을 학습하는 선택적 로컬 사전 기능.

### 5단계: 크로스 플랫폼 확장

- [ ] **입력 컨텍스트 통합**: 단순 "변환 도구"에서 완전한 입력기(IME) 서비스로 진화 (리눅스용 `ibus`, `fcitx5` 연동).
- [ ] **크로스 플랫폼 지원**: Windows(TSF) 및 macOS용 네이티브 백그라운드 서비스 및 연동 방안 조사.
