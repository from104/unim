# DBus 클라이언트 예제 (계획 단계)

`unim-daemon`이 세션 버스에 등록하는 `org.atit.unim.InputMethod` 서비스와
대화하는 최소 클라이언트 예제를 모으는 디렉토리입니다.

현재 **자리만 잡아둔 상태**이며 실제 예제는 아래 계획대로 추가 예정.

## 계획된 예제

| 파일 | 언어 | 목적 |
|------|------|------|
| `get_config_yaml.py` | Python (dbus-python 또는 jeepney) | `GetConfigYaml` 호출 결과 출력 |
| `set_config_yaml.py` | Python | `SetConfigYaml` 으로 `auto_typefix.enabled` 토글 |
| `watch_preedit_signal.rs` | Rust (zbus) | `UpdatePreeditText` 시그널 subscribe + 출력 |
| `watch_config_changed.py` | Python | `ConfigChangedJson` 시그널 관찰 |
| `send_keypress.rs` | Rust (zbus) | 컨텍스트 생성 → `ProcessKeyEvent` 호출 → 결과 확인 |

## 수동으로 시도해보기 (지금)

예제 코드가 생기기 전까지는 `busctl`/`dbus-send`로 직접 호출 가능:

```bash
# YAML 전체 설정 조회
busctl --user call org.atit.unim.InputMethod \
  /org/atit/unim/InputMethod \
  org.atit.unim.InputMethod GetConfigYaml

# 전역 입력 모드 조회
busctl --user call org.atit.unim.InputMethod \
  /org/atit/unim/InputMethod \
  org.atit.unim.InputMethod GetGlobalMode

# 한국어 모드로 전환
busctl --user call org.atit.unim.InputMethod \
  /org/atit/unim/InputMethod \
  org.atit.unim.InputMethod SetGlobalMode b true
```

## 참조

- DBus 인터페이스 전체 명세: [`../../unim-dbus/SPEC.md`](../../unim-dbus/SPEC.md)
- 설정 키 목록: [`../../unim-config/SPEC.md`](../../unim-config/SPEC.md)
- 데몬 구조: [`../../unim-daemon/SPEC.md`](../../unim-daemon/SPEC.md)

## 기여

예제 추가 시:
1. 의존성은 **최소**로. 단일 파일이면 최상단 주석에 `pip install ...` 또는 `cargo add ...` 기술
2. 예제 상단에 **목적 / 실행 방법 / 기대 출력** doc-comment
3. `examples/README.md` 및 본 README의 표에 한 줄 등록
