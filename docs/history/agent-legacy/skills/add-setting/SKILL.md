---
name: add-setting
description: UNIM에 새 설정 항목을 추가할 때 모든 연동 컴포넌트를 순서대로 수정하는 가이드
---

# 설정 항목 추가 스킬

UNIM에 새 설정 항목을 추가할 때는 아래 단계를 **반드시 순서대로** 수행해야 합니다.
`src/config.rs`가 Source of Truth이며, 나머지 컴포넌트는 이를 반영합니다.

## 수정 순서

### 1단계: Core 설정 정의

**파일**: `src/config.rs`

- 설정 구조체(`UnimConfig`)에 새 필드를 추가합니다.
- 필요한 경우 enum 타입을 정의합니다 (예: `ModeSharingMode`).
- `Default` trait 구현에 기본값을 추가합니다.
- serde 직렬화/역직렬화가 올바르게 작동하는지 확인합니다.

```rust
// 예시: 새 enum 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NewOption {
    OptionA,
    OptionB,
}

// UnimConfig 구조체에 추가
pub struct UnimConfig {
    // ... 기존 필드 ...
    pub new_option: NewOption,
}
```

### 2단계: CLI 설정 도구

**파일**: `unim-config/src/main.rs`

- `ConfigKey` enum에 새 키를 추가합니다.
- `get` / `set` / `list` 명령 처리 로직을 업데이트합니다.

**파일**: `unim-config/locales/ko.yml`, `unim-config/locales/en.yml`

- 새 설정 키에 대한 번역 문자열을 추가합니다.

### 3단계: DBus 서비스

**파일**: `unim-dbus/src/service.rs`

- `get_config` 메서드에 새 키에 대한 매칭을 추가합니다.
- `set_config` 메서드에 새 키에 대한 매칭을 추가합니다.

```rust
// get_config 예시
"new_option" => serde_json::to_string(&config.new_option).unwrap_or_default(),

// set_config 예시
"new_option" => {
    config.new_option = serde_json::from_str(&value)?;
}
```

### 4단계: GTK 설정 도구

**파일**: `unim-gtk-settings/src/settings_dialog.c`

- 새 설정을 위한 UI 위젯(ComboBox, CheckButton 등)을 추가합니다.
- DBus를 통한 `get_config`/`set_config` 연동 코드를 추가합니다.

### 5단계: Qt 설정 도구

**파일**: `unim-qt-settings/src/SettingsDialog.cpp`

- 새 설정을 위한 UI 위젯(QComboBox, QCheckBox 등)을 추가합니다.
- DBus를 통한 `get_config`/`set_config` 연동 코드를 추가합니다.

### 6단계: GNOME Extension 설정

**파일**: `unim-gnome-extension/schemas/org.gnome.shell.extensions.unim-indicator.gschema.xml`

- 새 GSettings 키를 정의합니다.

**파일**: `unim-gnome-extension/prefs.js`

- 설정 UI에 새 위젯을 추가합니다.
- GSettings 변경 시 DBus로 `set_config` 호출하는 연동 코드를 추가합니다.

### 7단계: C-API (필요 시)

**파일**: `unim-capi/src/lib.rs`

- 설정값이 외부에서 필요한 경우 FFI 함수를 추가합니다.

## 검증 체크리스트

- [ ] `cargo test --workspace` 통과
- [ ] `unim-config list` 에서 새 설정이 표시됨
- [ ] `unim-config get <key>` / `unim-config set <key> <value>` 동작 확인
- [ ] DBus `get_config`/`set_config` 정상 동작
- [ ] GTK 설정 도구에서 새 설정 표시 및 변경 가능
- [ ] Qt 설정 도구에서 새 설정 표시 및 변경 가능
- [ ] GNOME Extension 설정에서 새 설정 표시 및 변경 가능
