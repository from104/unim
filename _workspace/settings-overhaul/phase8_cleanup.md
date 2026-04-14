# Phase 8 — Dead Feature 완전 제거 (auto_switch, manual_shortcuts)

사용자 결정: **옵션 1** — C-API까지 breaking change로 전면 삭제.

## 1. 제거 대상

| 대상 | 상태 | 비고 |
|------|------|------|
| `auto_switch` (자동 한/영 전환 감지) | 완전 제거 | `src/input_engine.rs`에서 한 번도 호출되지 않던 dead feature |
| `manual_shortcuts` (수동 교정 단축키) | 완전 제거 | Phase 1에서 추가했으나 GNOME extension은 gschema만 읽음 — dead field |

## 2. 수정 / 삭제 파일

### 삭제
- `src/auto_switch.rs` — 파일 제거 (`rm`)

### 수정
- `src/lib.rs` — `pub mod auto_switch;` 제거
- `src/config.rs` — `AutoSwitchConfig` struct / `EngineConfig.auto_switch` 필드 / `ManualShortcutConfig` struct / `default_manual_shortcut_*` 함수 / `EngineConfig.manual_shortcuts` 필드 / 관련 `Default` 구현 제거. 기존 테스트 (`test_engine_config_defaults`, `test_config_custom_values`, `test_legacy_yaml_backcompat_autotypefix_and_manual_shortcuts`, `test_empty_yaml_full_defaults`, `test_manual_shortcut_defaults`) 수정·제거. **신규 테스트 추가**: `test_legacy_yaml_removed_fields_ignored`.
- `src/SPEC.md` — yaml 예시 및 `EngineConfig` Rust 시그니처에서 `auto_switch` 라인 제거
- `unim-dbus/src/service.rs` — 레거시 key-value 엔드포인트에서 `auto_switch_enabled` / `auto_switch_threshold` get/set 암 제거
- `unim-dbus/SPEC.md` — 설정 키 표에서 `auto_switch_enabled`, `auto_switch_threshold` 행 제거
- `unim-config/src/main.rs` — `ConfigKey::{AutoSwitch, AutoSwitchThreshold, ManualShortcutForward, ManualShortcutReverse}` variant + 매치 암 + interactive 루프 옵션 2개 제거(및 인덱스 재번호 4→6, 5→7, 6→8 등)
- `unim-config/locales/ko.yml`, `unim-config/locales/en.yml` — `auto_switch_label`, `auto_switch_threshold_label`, `manual_shortcut_forward_label`, `manual_shortcut_reverse_label`, `auto_switch_changed`, `threshold_changed`, `error_invalid_threshold`, `enable_auto_switch`, `enter_threshold` 등 dead 키 제거
- `unim-cli/locales/ko.yml`, `unim-cli/locales/en.yml` — 동일한 dead 로캘 키 제거 (`auto_switch_label`, `auto_switch_threshold_label`, `auto_switch_changed`, `threshold_changed`, `error_invalid_threshold`)
- `unim-capi/src/lib.rs` — C-API 함수 6개 제거: `unim_config_get_auto_switch_enabled`, `unim_config_set_auto_switch_enabled`, `unim_config_get_auto_switch_threshold`, `unim_config_set_auto_switch_threshold`, `unim_config_get_auto_switch_notification`, `unim_config_set_auto_switch_notification`
- `unim-capi/include/unim.h` — 동일 6개 함수 선언 및 `Auto Switch Configuration` 섹션 주석 제거
- `unim-capi/SPEC.md` — C-API 함수 표에서 3행 제거
- `unim-gui-gtk/src/settings_dialog.rs` — Page 1에서 `build_auto_switch_group` 호출 및 함수 정의 전체 제거, `build_keymap_group` 내 "수동 순방향 단축키"/"수동 역방향 단축키" `build_string_list_row` 2개 제거
- `.claude/agents/config-editor.md` — 6지점 서술을 5지점으로 수정, `ManualShortcut` 언급 제거, `manual_shortcuts` 필드 이름 언급 제거
- `.claude/skills/unim-settings-overhaul/skill.md` — Phase 1 설명에서 `EngineConfig.manual_shortcuts` 신설 항목을 "Phase 8에서 제거됨"으로 주석

## 3. C-API Breaking Change

제거된 export 함수 (헤더 + Rust):

```
unim_config_get_auto_switch_enabled
unim_config_set_auto_switch_enabled
unim_config_get_auto_switch_threshold
unim_config_set_auto_switch_threshold
unim_config_get_auto_switch_notification
unim_config_set_auto_switch_notification
```

영향:
- 외부 C/C++ 바인더가 이 심볼을 참조하면 링크 실패.
- `unim-frontends` (GTK/Qt 프론트엔드)는 이 함수들을 사용하지 않음 → 본 repo 내부 영향 **없음** (make build zero-warning 확인).
- 버전 범프(semver breaking) 권장 — unim-capi `Cargo.toml` 버전 관리 정책에 따라 별도 처리 필요.

## 4. 역호환성 (YAML 파싱)

`Config` 구조체는 `#[serde(deny_unknown_fields)]`를 사용하지 **않음** (grep 확인) → 기본 serde_yaml 동작이 unknown 필드를 조용히 ignore. 기존 `~/.config/unim/config.yaml`에 `auto_switch:`, `manual_shortcuts:` 가 있어도 파싱 실패 없음.

신규 테스트:

```rust
#[test]
fn test_legacy_yaml_removed_fields_ignored() {
    let yaml = "engine:\n  auto_switch:\n    enabled: true\n    threshold: 0.7\n  manual_shortcuts:\n    forward: ['<Super>k']\n    reverse: ['<Shift><Super>k']\n";
    let _: crate::config::Config =
        serde_yaml::from_str(yaml).expect("legacy yaml must still parse");
}
```

결과: **pass**.

## 5. 검증 결과

| 항목 | 결과 |
|------|------|
| `cargo build --workspace --release` | **성공** · zero warning (25.07s) |
| `cargo test --workspace` | **성공** · 250 passed / 0 failed (unim 크레이트 core + 모든 크레이트 합계) |
| `make build` (C/C++ 프론트엔드 포함) | **성공** · GTK3/4 + Qt5/6 전부 zero warning |
| Grep `auto_switch\|AutoSwitch\|manual_shortcut\|ManualShortcut` on code dirs | 코드 참조 0건. `src/config.rs` 내에는 역호환 테스트의 YAML 문자열 + 주석 2건만 잔존 (의도됨) |

Grep 최종 결과 (코드 파일 한정, 보고서/.claude 제외):

```
src/config.rs:957:    /// 제거된 필드(auto_switch, manual_shortcuts)가 포함된 구 yaml도
src/config.rs:961:        let yaml = "engine:\n  auto_switch:\n    enabled: ...";
```

두 건 모두 `test_legacy_yaml_removed_fields_ignored` 테스트 내부의 의도된 문자열/주석.

## 6. 사용자 확인 사항

1. **C-API semver**: `unim-capi` 공용 헤더에서 함수 6개 제거됨. 외부 소비자가 없으므로 본 프로젝트에는 영향 없지만, 공식 릴리즈 시 major 버전 범프 권장.
2. **설정 파일 자동 정리**: 기존 `~/.config/unim/config.yaml`의 `auto_switch:` / `manual_shortcuts:` 필드는 파싱만 무시될 뿐 파일에서 자동 제거되지는 않음. 다음 번 `config.save_to_default_path()` 호출 시 덮어써지므로 자연 소멸.
3. **GNOME extension**: 건드리지 않음 (제약 준수). Extension이 이미 gschema만 사용하므로 정상 동작. 단축키는 gschema 쪽에만 존재.
4. **CLI interactive 메뉴 인덱스 재번호**: `auto_switch` 항목 2개 제거로 메뉴가 앞당겨졌으나 외부 스크립트가 인덱스를 하드코딩하지 않는 한 영향 없음.
5. **과거 기록 수정 금지 준수**: `_workspace/04_implementation_plan.md` 및 `_workspace/settings-overhaul/phase*.md` (phase1~7) **미수정**.
