---
name: settings-sync-check
description: UNIM Config 구조체에 필드를 추가·변경하거나 설정 키를 삭제할 때마다 반드시 사용할 것. 6개 동기화 지점(src/config.rs / unim-cli config 서브커맨드 / locales / unim-dbus / GTK UI / GNOME gschema)의 누락을 원천 차단한다. "설정 추가", "config.rs 수정", "설정 필드", "ConfigKey", "gschema", "AutoTypeFix 항목" 같은 맥락에서 반드시 트리거.
---

# Settings Sync Check — UNIM 설정 6지점 체크리스트

UNIM에서 설정 한 개 추가가 실제로는 6곳 편집이다. 한 군데라도 빠지면 **CLI와 GUI가 불일치**, **GNOME에서 변경한 값이 엔진에 반영 안 됨** 같은 silent bug가 나온다. CLAUDE.md의 "Settings Synchronization" 섹션이 이 스킬의 원문이다.

## 체크리스트

필드 추가·변경·삭제 시 아래 순서로 작업하라. **각 항목 끝에서 `grep`으로 남은 참조가 없는지 확인**하라.

### 1. `src/config.rs` — 진실 공급원

- 구조체 필드 추가: 타입·이름·`#[serde(default = "fn_name")]` + `default_fn_name()` 함수
- 범위가 있다면 `Default` 구현 값과 별도로 clamp/검증 함수 제공 권장
- 필드 삭제 시: `#[serde(default)]` 덕분에 구 config.yaml은 계속 읽힘. 삭제 후 `cargo check` 필수

### 2. `unim-cli/src/main.rs` — CLI ConfigKey enum

- `ConfigKey` enum에 `#[value(name = "kebab-case-name")]` 추가
- `get_value()` / `set_value()` 매치 암 확장
- 범위 검증: `set_value`에서 clamp 또는 에러

### 3. `unim-cli/locales/*.yml` — 번역

- `ko.yml`, `en.yml` (및 다른 존재하는 로캘 전부) 키 추가
- 키 이름은 ConfigKey 이름과 일치시키는 것이 관례
- `grep -r "기존 키" unim-cli/locales/` 로 패턴 확인 후 같은 계층에 삽입

### 4. `unim-dbus/src/service.rs` — DBus 메서드

- **YAML 통짜 교환 모델이면 자동 커버** (이번 개편 후 기본)
- 개별 키 인터페이스가 있다면 매치 암 확장
- `ConfigChanged` signal은 `set_config` 내부에서 자동 방출됨 — 개별 처리 불필요

### 5. `unim-gui-gtk/src/settings_dialog.rs` — GTK GUI 위젯

- 해당 필드의 그룹·위젯 선정 (SpinRow/SwitchRow/ComboRow/EntryRow)
- `SettingsState.config` 경로 접근
- `updating` 플래그 가드 + `connect_*_notify` 콜백
- `save_and_notify()` 호출로 저장+DBus 전파

### 6. `unim-gnome-extension/schemas/*.gschema.xml` + `prefs.js`

- **이번 개편 후 원칙**: Shell API에 의존하지 않는 한 gschema에는 **추가하지 않음**
- 즉 일반 Config 필드는 config.yaml 전용 → gschema·prefs.js 건드리지 않음
- Shell API 의존(단축키, panel, notification, ime)인 경우만 gschema에 key 추가 + prefs.js 위젯 추가
- 스키마 수정 시 `glib-compile-schemas schemas/` 컴파일 확인

## 작업 원칙

- **순서 엄수**: 위 1→6 순서. 역순으로 하면 빌드가 중간에 깨져 원인 파악이 어려워짐.
- **필드 rename 시**: 모든 지점에서 동시에. `rg "old_field_name"`으로 잔존 참조 수색 필수.
- **필드 삭제 시**: gschema에서 지운 키를 prefs.js/extension.js가 여전히 읽으면 런타임 에러 — `rg "deleted-key" unim-gnome-extension/`로 확인.

## 검증

작업 완료 후 반드시 아래 실행:

```bash
cargo build --workspace       # zero warning
cargo test --workspace        # all pass
make build                    # C/C++ 프론트엔드 포함 zero warning
glib-compile-schemas unim-gnome-extension/schemas/   # gschema 변경 시
```

## 안티 패턴

- ❌ CLI만 추가하고 GUI 누락 → "CLI로 바꾼 값이 GUI에서 안 보임"
- ❌ gschema만 남기고 config.rs 없음 → "GNOME prefs의 값이 엔진에 무시됨"
- ❌ locale 하나만 번역 → "특정 언어에서 키 이름이 영어로 튀어나옴"
- ❌ serde default 없이 필드 추가 → 기존 사용자의 config.yaml 파싱 실패
