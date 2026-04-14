# DBus 디버깅 레퍼런스 (UNIM)

## 기본 정보

- 버스: **세션 버스** (system 아님)
- 서비스: `org.atit.unim.InputMethod`
- 객체: `/org/atit/unim/InputMethod`
- 인터페이스: `org.atit.unim.InputMethod`

## 서비스 존재 확인

```bash
busctl --user list | grep atit
```

## 인터페이스 인트로스펙션

```bash
busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod
```

새로 추가한 `GetConfig`, `SetConfig`, `ConfigChanged`가 출력에 나타나야 한다.

## 메서드 호출

```bash
# GetConfig - 인자 없음, String 반환
busctl --user call org.atit.unim.InputMethod \
    /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod GetConfig

# SetConfig - String 인자
busctl --user call org.atit.unim.InputMethod \
    /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod SetConfig s "$(cat ~/.config/unim/config.yaml)"
```

## Signal 관찰

```bash
busctl --user monitor org.atit.unim.InputMethod
```

다른 터미널에서 설정을 변경하면 `ConfigChanged` 또는 `GlobalModeChanged` 이벤트가 출력되어야 한다.

## 데몬 로그

```bash
# 포그라운드 실행 (로그 즉시 확인)
UNIM_DEVELOP=1 target/debug/unim-daemon -n

# 파일 로그 (설치본)
UNIM_DEVELOP=1
tail -f ~/.unim-errors.log
```

로그 포맷: `[YYYY/MM/DD HH:MM:SS] - [MODULE] - message`. DBus 관련 로그의 MODULE은 `DBUS` 또는 `DAEMON`.

## 전형적 실패 진단

| 증상 | 원인 | 해결 |
|------|------|------|
| `busctl list`에 서비스 없음 | 데몬 미실행 or 서비스 등록 실패 | 데몬 포그라운드 실행해 로그 확인 |
| `GetConfig` 호출 시 UnknownMethod | 메서드 미구현 or 인터페이스 이름 오타 | `introspect` 출력과 비교 |
| `ConfigChanged` 신호 수신 안 됨 | set_config에서 signal context 누락 | `#[signal]` 구현부 점검 |
| Lock 해제 전 signal 방출 | 순환 대기 가능 | Lock 스코프를 블록으로 좁히고 signal은 scope 밖에서 |

## GNOME extension에서 구독 확인

```javascript
// GNOME Shell의 Looking Glass (Alt+F2 → lg)에서
const { dbusIme } = imports.ui.extensionSystem.extensions['unim@atit.or.kr'].stateObj;
log(dbusIme.currentConfig);  // 캐시된 config 확인
```

## JSON payload 결정사항 (본 개편)

`ConfigChanged` signal은 **JSON 문자열** 페이로드 사용. 이유: JS 측에서 YAML 파서 의존 없이 `JSON.parse`로 즉시 처리. daemon은 `serde_json`으로 Config 직렬화.

`GetConfig`는 YAML 유지 (파일 저장 포맷과 동일). 클라이언트가 필요 시 선택.
