---
name: debug-ime
description: UNIM IME 디버깅 가이드 - DBus 통신, 프론트엔드 키 이벤트, 한글 조합 문제 디버깅
---

# UNIM IME 디버깅 스킬

UNIM 입력기의 문제를 진단하고 수정할 때 사용합니다.

## 디버깅 환경 준비

### 로그 활성화

```bash
export UNIM_DEVELOP=1
```

로그는 콘솔과 `~/.unim-errors.log`에 동시 출력됩니다.

### 로그 모니터링

```bash
tail -f ~/.unim-errors.log
```

## 디버깅 시나리오별 가이드

### 1. 키 입력이 전달되지 않는 경우

**증상**: 특정 앱에서 한글 입력이 안 됨

**진단 순서**:

1. 환경변수 확인:
```bash
echo $GTK_IM_MODULE    # → unim
echo $QT_IM_MODULE     # → unim
echo $XMODIFIERS       # → @im=unim
```

2. 데몬 실행 확인:
```bash
busctl --user list | grep unim
```

3. DBus 인터페이스 확인:
```bash
busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod
```

4. 해당 앱의 툴킷 확인:
```bash
ldd $(which <app>) | grep -E "gtk|qt|libX"
```

5. 앱별 IM 모듈 로딩 확인 (UNIM_DEVELOP=1로 실행 후 로그 확인)

### 2. 한글 조합이 비정상인 경우

**증상**: 글자가 올바르게 조합되지 않거나 도깨비불 현상 발생

**확인 파일**:
- `src/input_engine.rs` - `process_hangul_key()` 함수
- `src/hangul/` - 조합 로직 (2벌식: `two_set.rs`, 3벌식: `three_set_*.rs`)

**테스트 방법**:
```bash
cargo test --workspace
```

### 3. DBus 통신 문제

**증상**: 포커스 전환 시 상태 불일치, 시그널 미수신

**진단**:
```bash
# DBus 시그널 모니터링
dbus-monitor --session "interface='org.atit.unim.InputMethod'"

# 수동 DBus 호출 테스트
busctl --user call org.atit.unim.InputMethod \
  /org/atit/unim/InputMethod \
  org.atit.unim.InputMethod \
  GetMode
```

**확인 파일**:
- `unim-dbus/src/service.rs` - 서버 측 메서드
- `unim-dbus/src/client.rs` - 클라이언트 측 호출

### 4. 프론트엔드별 문제

#### GTK3/GTK4

**확인 파일**: `unim-frontends/gtk3/src/immodule.c` 또는 `unim-frontends/gtk4/src/immodule.c`

**핵심 함수**:
- `filter_keypress()` - 키 이벤트 필터링
- `unim_im_context_focus_in()` / `focus_out()` - 포커스 관리
- `unim_im_context_set_cursor_location()` - 커서 위치

**주의사항**:
- GTK3은 `gtk_im_context_filter_keypress` 기반
- GTK4는 `GtkIMContext` 가상 함수 기반
- 한자(Hanja) 팝업 관련: `unim-frontends/gtk-common/src/unim_hanja_popup.c`

#### Qt5/Qt6

**확인 파일**: `unim-frontends/qt5/src/` 또는 `unim-frontends/qt6/src/`

**핵심 클래스**: `QPlatformInputContext` 상속

#### XIM

**확인 파일**: `unim-frontends/xim/src/handler.rs`

**알려진 이슈**:
- 일부 앱(WezTerm 등)에서 중복 키 이벤트 발생 가능
- `KeyRelease` 이벤트 필터링 확인 필요

### 5. 샌드박스 테스트

시스템 IM에 영향 없이 격리 환경에서 테스트:

```bash
make sandbox-gtk4    # GTK4 앱에서 테스트
make sandbox-xim     # XIM 프로토콜 테스트
```

## 로그 모듈명 참조

| 모듈명 | 위치 |
| ------ | ---- |
| `ENGINE` | `src/input_engine.rs` |
| `HANGUL` | `src/hangul/*.rs` |
| `DAEMON` | `unim-daemon` |
| `DBUS` | `unim-dbus` |
| `GTK3_IM` / `GTK4_IM` | GTK IM 모듈 |
| `QT5_IM` / `QT6_IM` | Qt IM 플러그인 |
| `XIM` | XIM 프론트엔드 |
| `WAYLAND` | Wayland 프론트엔드 |
