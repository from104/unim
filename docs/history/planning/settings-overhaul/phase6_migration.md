# Phase 6 — Migration (GSettings → config.yaml) 산출물

`unim-daemon` 기동 시 1회성 마이그레이션 루틴 추가. 가드 파일로 재실행 방지.

---

## 1. 수정/신규 파일 목록

| 파일 | 변경 |
|------|------|
| `unim-daemon/src/migration.rs` | **신규** — 마이그레이션 로직, 파서, mock-기반 단위 테스트 6개 |
| `unim-daemon/src/main.rs:21` | `mod migration;` 추가 |
| `unim-daemon/src/main.rs:447–451` | `migrate_v2()` 호출 (설정 로드 직전, DBus/엔진 초기화 전) |

기존 크레이트(`src/config.rs`, `unim-dbus/`, `unim/`, etc.) 무수정 — Phase 1에서 이미 노출된 `Config::default`, `save_to_default_path`, `AutoTypeFixConfig::clamp_ranges()` 재사용.

Cargo dependency 추가 없음 (기존 `dirs`·`tokio`로 충분, `dconf` CLI subprocess 사용).

---

## 2. 핵심 로직 요약

```rust
pub fn migrate_v2() {
    let guard = ~/.config/unim/.migrated-v2;
    if guard.exists() { return; }           // 가드 존재 → skip
    if !dconf_available() {                 // GNOME/dconf 부재 환경
        touch_guard(); return;              // 가드만 생성하고 종료
    }

    let mut config = Config::load_from_default_path();
    let default_config = Config::default();
    let reader = DconfReader;

    let applied = apply_migration(&mut config, &default_config, &reader);
    if applied == 0 { touch_guard(); return; }   // 이관 대상 없음도 정상

    config.engine.auto_typefix.clamp_ranges();   // 범위 방어
    match config.save_to_default_path() {
        Ok(()) => touch_guard(),                 // 성공 시에만 가드 생성
        Err(_) => { /* 가드 미생성 → 다음 기동 재시도 */ }
    }
}
```

**핵심 설계 결정**

- **dconf CLI subprocess** 사용 (`gio::Settings` 미채택). 이유:
  - gschema에서 키가 이미 삭제된 상태(Phase 4 완료) → `Settings` 객체로 구 키 접근 불가
  - `glib/gio` 크레이트 신규 추가 불필요 (daemon 부팅 성능 유지)
  - dconf DB에는 스키마 삭제 후에도 값이 그대로 남아있음 (테스트로 확인)
- **SettingReader trait** 도입 — `DconfReader`(실제)와 `MockReader`(테스트) 분리
- **"기본값인 경우에만 덮어씀"** 규칙 — 사용자가 GTK/CLI로 이미 수정한 값은 보존
- **가드 생성 시점** — 저장 성공 시에만. 실패 시 다음 기동에서 재시도
- **비치명적 실패** — 마이그레이션이 실패해도 daemon은 정상 기동

---

## 3. 13개 키 매핑 표 (Phase 4 §8과 일치)

| GSettings 키 | dconf 타입 | config.yaml 필드 | 파서 |
|--------------|-----------|------------------|------|
| `korean-layout` | `s` | `engine.korean.layout` | `'2bul'`·`'3bul390'`·`'3bul391'`·`'3bul_noshift'` → KoreanLayout |
| `english-layout` | `s` | `engine.english.layout` | `'qwerty'`·`'dvorak'`·`'colemak'`·`'colemak_dh'`·`'workman'` → EnglishLayout |
| `initial-mode` | `s` | `engine.default_category` | `'Korean'`·`'English'` → InputCategory |
| `mode-sharing` | `s` | `engine.mode_sharing` | `'global'`·`'per_app'` → ModeSharingMode |
| `popup-mode` | `s` | `engine.popup_mode` | `'Standalone'`·`'Embedded'` → PopupMode |
| `toggle-keys` | `as` | `engine.toggle_keys: Vec<String>` | `['Korean','RightAlt']` |
| `hanja-keys` | `as` | `engine.hanja_keys: Vec<String>` | `['Hanja','F9']` |
| `auto-typefix-enabled` | `b` | `engine.auto_typefix.enabled` | true/false |
| `auto-typefix-forward` | `b` | `engine.auto_typefix.forward` | true/false |
| `auto-typefix-reverse` | `b` | `engine.auto_typefix.reverse` | true/false |
| `auto-typefix-time-window` | `u` | `engine.auto_typefix.time_window_ms` | `uint32 NNNN` |
| `auto-typefix-kor-threshold` | `u` | `engine.auto_typefix.kor_syllable_threshold` | u32 → u8 (clamp) |
| `auto-typefix-eng-min-length` | `u` | `engine.auto_typefix.eng_word_min_length` | u32 → u8 (clamp) |

⚠️ **주의 — 기본값 불일치 케이스**: `auto-typefix-time-window`는 구 gschema 기본 2000ms, config.rs 기본 5000ms. 사용자가 변경하지 않아도 dconf에 2000이 남아있을 수 있음. 정책상 **config.yaml이 기본값(5000)이고 dconf에 값이 있으면** 이관(2000) — 구 동작 유지. `clamp_ranges()`가 500~5000 범위로 방어.

---

## 4. 단위 테스트 목록 & 결과

| 테스트 | 검증 내용 | 결과 |
|-------|----------|------|
| `test_parse_helpers` | bool/uint/enum/list 파서 전반 | PASS |
| `test_default_config_migrates_all_custom_values` | 13개 키 전부 이관 | PASS |
| `test_user_modified_config_is_preserved` | 사용자 수정값은 보존, 기본값 필드만 이관 | PASS |
| `test_empty_reader_applies_zero_keys` | dconf 빈 환경 (applied=0) | PASS |
| `test_invalid_values_are_skipped_with_no_change` | 타입 불일치 per-key skip | PASS |
| `test_partial_migration_counts_correctly` | 부분 이관 카운트 정확성 | PASS |

```
running 6 tests
test migration::tests::test_empty_reader_applies_zero_keys ... ok
test migration::tests::test_default_config_migrates_all_custom_values ... ok
test migration::tests::test_invalid_values_are_skipped_with_no_change ... ok
test migration::tests::test_parse_helpers ... ok
test migration::tests::test_partial_migration_counts_correctly ... ok
test migration::tests::test_user_modified_config_is_preserved ... ok

test result: ok. 6 passed; 0 failed
```

---

## 5. 검증 판정 표

| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L1 | `cargo build -p unim-daemon --release` | ✓ zero warning (9.51s) |
| L2 | `cargo build --workspace --release` | ✓ zero warning |
| L2 | `cargo test --workspace` | ✓ 모든 크레이트 0 failed (unim 254, unim-dbus 4, **unim-daemon 6 신규**, unim-gui-common 6, doc-tests 19, 2 ignored) |
| L3 | `make build` | ✓ zero warning (Rust + GTK3/4 + Qt5/6 + XIM + Wayland) |
| dconf 환경 확인 | `dconf read /org/gnome/shell/extensions/unim/korean-layout` | `'3bul390'` (사용자 실제 커스텀 값 존재) |
| dconf 환경 확인 | `dconf read /org/gnome/shell/extensions/unim/auto-typefix-time-window` | `uint32 3000` |

---

## 6. 수동 검증 시나리오

### 헤드리스에서 확인 완료

- [x] `~/.config/unim/.migrated-v2` 가드 파일 현재 없음 → 다음 daemon 기동에서 migration 트리거 예정
- [x] 사용자 dconf에 실제 커스텀 값 존재 (`korean-layout='3bul390'`, `auto-typefix-time-window=uint32 3000`, `toggle-keys=['Korean','RightAlt']`) — 마이그레이션 대상
- [x] `dconf` CLI `/usr/bin/dconf` 사용 가능 확인

### 사용자 승인 후 실행할 항목

(daemon 재설치·재기동이 필요하므로 Phase 7 수동 검증에서 처리)

- [ ] `rm -f ~/.config/unim/.migrated-v2`
- [ ] 사용자 config.yaml 백업 (`cp ~/.config/unim/config.yaml ~/.config/unim/config.yaml.bak`)
- [ ] `sudo make install PREFIX=/usr` 후 daemon 재기동 (`systemctl --user restart unim` 또는 수동 kill+재실행)
- [ ] `cat ~/.config/unim/config.yaml | grep -A1 layout` → `layout: Sebeolsik390` 확인 (기본값 `Dubeolsik`이었다면)
- [ ] `cat ~/.config/unim/config.yaml | grep time_window_ms` → `time_window_ms: 3000` 확인
- [ ] `ls -la ~/.config/unim/.migrated-v2` → 파일 생성 확인
- [ ] daemon 재기동 → `~/.unim-errors.log`에 "마이그레이션 v2 완료" 로그 **재출력 안 됨** (가드 동작)
- [ ] `gsettings set org.gnome.shell.extensions.unim korean-layout 'foo'` — **schema 삭제 상태에서는 실패 예상** (gsettings는 schema 필요)이지만 dconf 값은 Phase 4 이후에도 유효

⚠️ **주의**: Phase 4의 gschema 삭제 이후 `gsettings set`은 실패한다(schema-less). 사용자가 마이그레이션 후 dconf 값을 정리하려면 `dconf reset /org/gnome/shell/extensions/unim/korean-layout` 등으로 제거 가능하나, 이는 선택적 cleanup으로 마이그레이션 필수 요구사항 아님.

---

## 7. 성능 특성

- 가드 파일 존재 시: stat(2) 1회 + 즉시 반환 (< 1ms)
- 첫 기동 시: `dconf --version` 1회 + `dconf read` 13회 subprocess
  - 각 subprocess ~5–10ms → 총 ~70–130ms
  - 100ms 목표는 환경에 따라 초과 가능하나, 전체 수명 중 단 1회 → 허용
- 마이그레이션 순서: **엔진 워커 생성·DBus 서비스 등록 이전**에 완료 → daemon 기동 후 첫 DBus 호출 시점에 이관 값이 엔진 config에 반영됨

---

## 8. Phase 7 인수인계

### 잔존 이슈 — `rust-i18n %{error}` 치환 실패

Phase 5 CLI에서 locales/*.yml의 `%{error}` 인터폴레이션이 rust-i18n 매크로와 상호작용하면서 일부 메시지가 raw 토큰으로 남을 수 있음 (Phase 5 보고서 참조). Phase 6에서는 해당 locale 파일을 건드리지 않았으므로 이슈 불변. Phase 7 reviewer가 `cargo run -p unim-config -- get-value bad-key 2>&1`로 실제 출력 확인 필요.

### 수동 검증 체크리스트 (Phase 7 최종 QA에 편입)

위 §6의 "사용자 승인 후 실행할 항목" 전체. 특히:

1. **마이그레이션 누락 확인**: 13개 키 모두 이관되는지 개별 검증 (사용자의 실제 dconf 값 기준)
2. **사용자 값 보존 시나리오**: 가드 삭제 → config.yaml에서 수동으로 한 필드만 바꿔놓고 daemon 재기동 → 그 필드는 dconf 값으로 덮어써지지 **않아야** 함
3. **재실행 방지**: 두 번째 기동에서 로그에 "마이그레이션 v2 완료" 문구가 **재출현 안 함** 확인
4. **GNOME 없는 환경** (pure Xephyr/Wayland test): dconf 부재 시 가드만 생성하고 조용히 완료 확인 → 마이그레이션이 GNOME 의존성 만들지 않음 보장

### 후속 정리 제안

- Phase 7 완료 후 충분한 시간(예: 1~2 마이너 버전)이 지나면 `DconfReader` 및 dconf cleanup(`dconf reset -f /org/gnome/shell/extensions/unim/`)을 별도 스크립트로 제공하는 것 고려
- 가드 파일명 `.migrated-v2`는 향후 v3 마이그레이션이 생기면 `.migrated-v3`로 교체하여 새 routine 트리거

---

## 9. 결정 요약

1. **dconf CLI subprocess 채택** — gschema 키가 이미 삭제되어 `gio::Settings` 사용 불가. glib 크레이트 신규 의존 회피.
2. **SettingReader trait** — 테스트 가능성 확보. 프로덕션은 `DconfReader`, 테스트는 `MockReader(HashMap)`.
3. **"default 필드만 덮어씀" 보수적 정책** — 기존 GTK/CLI 수정값 절대 손실 금지.
4. **비치명적 에러 처리** — 마이그레이션 실패가 daemon 기동을 막지 않음. 저장 실패 시 가드 미생성 → 자동 재시도.
5. **clamp_ranges() 적용** — Phase 1에서 정의한 범위 방어를 저장 전에 호출해 구 gschema 값이 범위를 벗어나도 안전.
