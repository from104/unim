# Phase 2 — DBus Implementer 산출물

## 1. 수정 파일

| 파일 | 변경 요약 |
|------|-----------|
| `unim-dbus/Cargo.toml:24` | `serde_yaml = "0.9"` 의존성 추가 |
| `unim-dbus/src/service.rs:325-341` | 신규 signal `config_changed_json(json: &str)` 선언 (기존 `config_changed(key,value)` 유지) |
| `unim-dbus/src/service.rs:552-617` | 신규 메서드 `get_config_yaml()`, `set_config_yaml(yaml)` 구현 |
| `unim-dbus/src/client.rs:30-48` | 프록시 trait에 `get_config_yaml`/`set_config_yaml` 메서드, `config_changed`·`config_changed_json` signal 선언 추가 |

## 2. 설계 결정 — 왜 "추가" 했는가 (대체가 아니라)

기존 `GetConfig(key: s) -> s` / `SetConfig(key: s, value: s)` / `ConfigChanged(ss)` 인터페이스는 **gtk3/4, qt5/6, xim, wayland, unim-gnome-extension dbus_ime.js, tests/*, unim-gui-gtk** 등 최소 10곳에서 이미 호출 중이다. plan은 `unim-dbus/`, `unim-daemon/`만 수정하도록 Phase 경계를 명시하므로, 레거시 시그니처를 제거하면 타 프론트엔드가 즉시 깨진다.

**결정**: 신규 YAML/JSON 기반 메서드·시그널을 **병존 추가**. 레거시 key-기반 메서드는 Phase 3~5에서 클라이언트 이관이 완료된 뒤 Phase 7에서 제거.

- `GetConfigYaml() -> s` — 전체 Config YAML 반환 (파일 저장 포맷과 동일)
- `SetConfigYaml(yaml: s) -> ()` — YAML 수신 → 파싱 → clamp → 저장 → 공유 상태 갱신 → `ConfigChangedJson` signal 방출
- `ConfigChangedJson(json: s)` signal — 전체 Config의 JSON 직렬화 payload (GNOME extension JS 호환성 · `JSON.parse` 직접 처리)

## 3. Introspection XML 스니펫

```
org.atit.unim.InputMethod           interface -         -            -
.GetConfig                          method    s         s            -   (legacy, 유지)
.GetConfigYaml                      method    -         s            -   ← NEW
.SetConfig                          method    ss        -            -   (legacy, 유지)
.SetConfigYaml                      method    s         -            -   ← NEW
.ConfigChanged                      signal    ss        -            -   (legacy, 유지)
.ConfigChangedJson                  signal    s         -            -   ← NEW
.GlobalModeChanged                  signal    b         -            -   (기존)
```

## 4. set_config_yaml 동작 순서 (service.rs:570-617)

1. `serde_yaml::from_str::<Config>(&yaml)` — 실패 시 `zbus::fdo::Error::InvalidArgs("YAML 파싱 실패: …")`
2. `new_config.engine.auto_typefix.clamp_ranges()` — Phase 1 방어 호출
3. `new_config.save_to_default_path()` — IO 실패 시 `zbus::fdo::Error::Failed("Config 파일 저장 실패: …")`
4. `self.config.write().await` 스코프 진입 → `*cfg = new_config;` → 같은 스코프에서 `serde_json::to_string(&*cfg)` → 스코프 종료 (lock drop)
5. lock 해제 이후 `Self::config_changed_json(&signal_ctx, &json).await?` 방출
6. `unim_log!("DBUS", …)` 로 길이 기록

**Lock 스코프 최소화**: `save_to_default_path()`는 lock 밖에서 호출(Config 복사본 사용). signal 방출은 lock drop 뒤. Deadlock/재진입 위험 없음.

## 5. busctl 검증 로그 (4단계)

### 5.1 Introspect

```
$ busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod
.GetConfigYaml                      method    -         s            -
.SetConfigYaml                      method    s         -            -
.ConfigChangedJson                  signal    s         -            -
```

### 5.2 GetConfigYaml

```
$ busctl --user call org.atit.unim.InputMethod /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod GetConfigYaml
s "engine:\n  default_category: English\n  mode_sharing: Global\n  korean:\n    layout: Sebeolsik390\n  …  auto_typefix:\n    enabled: true\n    time_window_ms: 5000\n    kor_syllable_threshold: 2\n    eng_word_min_length: 5\n    forward: true\n    reverse: true\n    skip_on_english_word: true\n    skip_on_complete_syllable: true\n  manual_shortcuts:\n    forward:\n    - <Super>k\n    reverse:\n    - <Shift><Super>k\n"
```

Phase 1 신설 필드 `skip_on_english_word`, `skip_on_complete_syllable`, `manual_shortcuts` 포함 확인. `~/.config/unim/config.yaml`와 의미적으로 일치 (serde 필드 순서만 다름).

### 5.3 Monitor

```
$ busctl --user monitor org.atit.unim.InputMethod
```

### 5.4 SetConfigYaml → ConfigChangedJson 수신

```
$ busctl --user call org.atit.unim.InputMethod /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod SetConfigYaml s "$(cat ~/.config/unim/config.yaml)"
# (method_return, 빈 반환)
```

모니터 캡처:
```
Type=signal  Sender=:1.243  Path=/org/atit/unim/InputMethod
Interface=org.atit.unim.InputMethod  Member=ConfigChangedJson
MESSAGE "s" {
    STRING "{"engine":{"default_category":"English","mode_sharing":"Global","korean":{"layout":"Sebeolsik390",…},"auto_typefix":{"enabled":true,"time_window_ms":5000,"kor_syllable_threshold":2,"eng_word_min_length":5,"forward":true,"reverse":true,"skip_on_english_word":true,"skip_on_complete_syllable":true},"manual_shortcuts":{"forward":["<Super>k"],"reverse":["<Shift><Super>k"]}}}";
};
Type=method_return  ReplyCookie=2  (SetConfigYaml 응답)
```

전체 Config JSON이 single-string payload로 방출됨. 신설 필드 포함 확인.

## 6. 클라이언트 구독 샘플

### 6.1 GNOME extension (dbus_ime.js) — JS

```javascript
// dbus_ime.js 의 InputMethod 프록시 g-signal 핸들러에서:
this._imSignalId = this._imProxy.connect('g-signal',
    (proxy, senderName, signalName, params) => {
        if (signalName === 'GlobalModeChanged') {
            const [isKorean] = params.deepUnpack();
            this._onModeChanged && this._onModeChanged(isKorean);
        } else if (signalName === 'ConfigChangedJson') {        // ← NEW
            const [jsonStr] = params.deepUnpack();
            try {
                const cfg = JSON.parse(jsonStr);
                // UnimConfigCache.current = cfg; 등으로 캐시 갱신
                this._onConfigChanged && this._onConfigChanged(cfg);
            } catch (e) {
                unimError('DBUS_IME', `ConfigChangedJson 파싱 실패: ${e.message}`);
            }
        }
    });

// 초기 로드 (시작 시 1회)
async _loadInitialConfig() {
    const [yamlStr] = await this._imProxy.call_sync(
        'GetConfigYaml', null, Gio.DBusCallFlags.NONE, -1, null
    ).deepUnpack();
    // YAML 파서 필요 시 GLib.KeyFile 대체 — 그러나 payload JSON이므로
    // 실제론 시작 시에도 GetConfigYaml 대신 다음 패턴 추천:
    //   this._imProxy.call('GetConfigYaml', ...) 결과 YAML → 서버에서
    //   set_config_yaml(yaml)을 즉시 호출하면 ConfigChangedJson이 되돌아온다.
    // 또는 간단히 no-op SetConfigYaml 방식 대신 서버에 'GetConfigJson' 추가를
    // Phase 4에서 논의 (현재는 YAML→JS에서 처리).
}
```

**주의 (gnome-migrator 참고)**: GNOME extension은 YAML 파싱 라이브러리가 없다. JSON으로만 갱신받는 게 단순하다. **향후 Phase 4에서 `GetConfigJson()` 메서드 추가**를 검토할 것 — 현 시점에선 `ConfigChangedJson` signal을 첫 연결 직후에도 강제로 트리거하도록 클라이언트가 `SetConfigYaml(현재YAML)`을 한 번 호출하는 우회가 가능하나, 정식 API로 `GetConfigJson` 추가가 깔끔.

### 6.2 GTK GUI (unim-gui-gtk) — Rust

```rust
use unim_dbus::client::InputMethodProxy;
use zbus::{Connection, proxy::SignalStream};
use futures_util::StreamExt;

async fn subscribe_config_changes<F>(mut on_change: F) -> zbus::Result<()>
where
    F: FnMut(unim::config::Config),
{
    let conn = Connection::session().await?;
    let proxy = InputMethodProxy::new(&conn).await?;

    // 초기 상태 로드
    let yaml = proxy.get_config_yaml().await?;
    if let Ok(cfg) = serde_yaml::from_str::<unim::config::Config>(&yaml) {
        on_change(cfg);
    }

    // ConfigChangedJson 구독
    let mut stream: SignalStream = proxy.receive_config_changed_json().await?;
    while let Some(signal) = stream.next().await {
        let args = signal.args()?;
        let json: String = args.json;
        if let Ok(cfg) = serde_json::from_str::<unim::config::Config>(&json) {
            on_change(cfg);
        }
    }
    Ok(())
}

// 저장 호출 (GTK 다이얼로그 변경 이벤트):
pub async fn save_config(cfg: &unim::config::Config) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    let proxy = InputMethodProxy::new(&conn).await?;
    let yaml = serde_yaml::to_string(cfg)
        .map_err(|e| zbus::Error::Failure(e.to_string()))?;
    proxy.set_config_yaml(&yaml).await
}
```

## 7. 검증 결과

| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L2 | `cargo build --workspace --release` | ✓ zero warning (24.2s) |
| L2 | `cargo test --workspace` | ✓ 283 passed (unim 254, unim-dbus 4, unim-gui-common 6, doctests 19), 0 failed, 2 ignored |
| DBus introspect | `GetConfigYaml`, `SetConfigYaml`, `ConfigChangedJson` 모두 노출 | ✓ |
| DBus GetConfigYaml | YAML 문자열 반환 (파일과 일치) | ✓ |
| DBus SetConfigYaml + monitor | `ConfigChangedJson` signal 수신 (JSON payload에 신설 필드 포함) | ✓ |
| L3 `make build` | 이 Phase에서는 skip (C/C++ 프론트엔드 미수정. Phase 7 reviewer가 종합 수행) | — |

## 8. 발견 이슈 및 결정사항

1. **Breaking change 회피**: 기존 `GetConfig/SetConfig/ConfigChanged` 는 유지. 신규 `*Yaml`/`*Json` 로 병존. 제거 시점 = Phase 7 reviewer 판단.
2. **GNOME extension 초기 로드**: JS는 YAML 파서가 없다. **권고 — Phase 4에서 `GetConfigJson()` 메서드 추가** (짧음: 한 메서드, 기존 `get_config_yaml` 구현에서 `serde_json::to_string` 교체). 합의되면 dbus-implementer가 즉시 증분 작업 가능.
3. **lock 순서**: `save_to_default_path()`는 신규 Config(로컬 변수)에 대해 호출하므로 shared RwLock 없이 실행 → 순환 대기 불가. Drop 순서상 write guard가 JSON 직렬화 이후에 drop되도록 블록 묶음 (signal은 guard drop 다음 줄).
4. **config_changed_json 이름 스네이크케이스**: zbus의 interface 매크로는 method/signal 이름을 자동으로 PascalCase 변환하므로 introspect에서 `ConfigChangedJson`으로 노출됨 — DBus 컨벤션 준수.
5. **serde_yaml 의존성**: Config 구조가 `unim` 크레이트 재사용이지만 `unim-dbus`에서 직접 파싱해야 하므로 Cargo.toml에 명시 추가.

## 9. 인수인계

### Phase 3 · gtk-designer

- `unim-gui-gtk/src/settings_dialog.rs:491` 에 기존 `SetConfig` 호출이 있음. 신규 `SetConfigYaml` 호출로 점진 이관 가능. 샘플 코드는 §6.2 참고.
- 제안 플로우: 다이얼로그가 변경 이벤트 → Config 구조체 갱신 → `serde_yaml::to_string(&config)` → `set_config_yaml`.
- 초기값 로드: `get_config_yaml()` → `serde_yaml::from_str::<Config>` → 위젯 바인딩.
- ConfigChangedJson signal 구독으로 타 프론트엔드가 바꾼 값이 실시간 반영.

### Phase 4 · gnome-migrator

- `dbus_ime.js`: §6.1 코드를 `g-signal` 핸들러에 삽입. `_onConfigChanged` 콜백 신설.
- **요청**: 앞서 언급한 `GetConfigJson()` 메서드 추가 여부 결정. 필요 시 dbus-implementer에게 회신 → `get_config_yaml`와 같은 로직에서 `serde_json::to_string` 사용한 쌍둥이 메서드 즉시 추가 가능.
- 레거시 `GetConfig('popup_mode')`·`GetConfig('hanja_keys')` 호출 지점 (dbus_ime.js:633, immodule.c:85/381 등)은 Phase 4에서 `ConfigChangedJson` 캐시로 이관.

### Phase 5 · config-editor (CLI)

- CLI 동작은 로컬 YAML을 직접 수정하는 경로라 DBus 영향 없음. 그러나 CLI 수정 후 daemon에게 즉시 전파하고 싶다면 `SetConfigYaml` 호출 옵션 추가 고려 (선택).

### Phase 7 · reviewer

- 레거시 `GetConfig/SetConfig/ConfigChanged` 제거 여부 결정.
- 제거 시: `unim-dbus/src/client.rs`의 `get_config/set_config/config_changed` 삭제, service.rs의 해당 구현 삭제, SPEC.md 갱신.
- 보존 시: deprecated 주석만 추가.

## 10. 검증 판정 표

| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L2 cargo build --workspace --release | zero warning | ✓ |
| L2 cargo test --workspace | 283 passed / 0 failed | ✓ |
| DBus introspect | GetConfigYaml, SetConfigYaml, ConfigChangedJson 노출 | ✓ |
| DBus GetConfigYaml | YAML 반환 (Phase 1 신규 필드 포함) | ✓ |
| DBus SetConfigYaml + monitor | ConfigChangedJson JSON payload 수신 | ✓ |
