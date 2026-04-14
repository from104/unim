# Phase 4 — GNOME Migrator 산출물

GNOME Shell extension을 단일 창구화 구조(SSoT = config.yaml)에 맞춰 전면 재편.

---

## 1. 수정/삭제 파일 목록

| 파일 | 변경 요약 |
|------|----------|
| `unim-dbus/src/service.rs:573-584` | **신규** `get_config_json()` 메서드 (serde_json 직렬화, zbus introspect 시 `GetConfigJson`) |
| `unim-dbus/src/client.rs:35-36` | 프록시 trait에 `get_config_json()` 대칭 추가 |
| `unim-gnome-extension/schemas/org.gnome.shell.extensions.unim.gschema.xml` | **13개 키 삭제**, 7개 키 유지 |
| `unim-gnome-extension/dbus_ime.js:56-82` | `_configCache`, `getCachedConfig()`, `setOnConfigChanged()` 추가 |
| `unim-gnome-extension/dbus_ime.js:113-140` | `g-signal` 핸들러에 `ConfigChangedJson` 분기 추가 |
| `unim-gnome-extension/dbus_ime.js:168-197` | `_loadInitialConfig()` 신설 — `GetConfigJson` 1회 호출로 `_configCache` 시딩 |
| `unim-gnome-extension/prefs.js` | **전면 재작성** — 2 page → 1 page + 리다이렉트 카드 (461 lines → 165 lines) |

**건드리지 않은 파일** (정책 경계):
- `extension.js` — 삭제된 13개 키 중 **읽는 지점 0건** 확인 후 무수정 (조사 결과 §3 참조)
- `key_handler.js`, `indicator.js`, 기타 JS — 영향 없음

---

## 2. gschema diff

### 삭제 (13개)
```
korean-layout, english-layout, initial-mode, mode-sharing, popup-mode,
toggle-keys, hanja-keys,
auto-typefix-enabled, auto-typefix-forward, auto-typefix-reverse,
auto-typefix-time-window, auto-typefix-kor-threshold, auto-typefix-eng-min-length
```

### 유지 (7개)
| 키 | 이유 |
|-----|------|
| `enable-extension` | extension.js:544에서 비활성화 가드로 사용 중 → **유지 확정** |
| `show-panel-indicator` | `Panel.statusArea` 조작 (Shell API) |
| `show-notification` | `Main.notify` (Shell API) |
| `enable-ime` | `Clutter.InputMethod` (Wayland 전용, Shell API) |
| `shortcut-normal` | `Main.wm.addKeybinding` |
| `shortcut-normal-reverse` | `Main.wm.addKeybinding` |

### `enable-extension` 사용처 조사 결과

```
extension.js:544:        if (!this._settings.get_boolean('enable-extension')) return;
```

`_bindShortcut` 내에서 단축키 실행 시 확장 전체 비활성화 가드로 사용. 제거하면 단축키가 항상 동작하게 되므로 **유지**. 다만 향후 활성화/비활성화는 GNOME Extensions 앱의 표준 on/off 토글로 통일하는 게 맞을 수 있음 (Phase 7 reviewer 판단).

### `glib-compile-schemas` 결과

```
$ glib-compile-schemas unim-gnome-extension/schemas/
(무출력, exit=0)  ← 성공
```

`gschemas.compiled` 생성 확인.

---

## 3. extension.js 전환 포인트 (before/after 매핑)

**조사 결과: extension.js가 삭제된 13개 키를 읽는 지점이 0건.**

```
$ grep -nE "'(korean-layout|english-layout|initial-mode|mode-sharing|popup-mode|toggle-keys|hanja-keys|auto-typefix-)'" extension.js
(결과 없음)
```

기존 구조가 이미 config.yaml 중심이었음:
- `extension.js:465`: `this._dbusIME.getConfig('hanja_keys')` — 이미 DBus `GetConfig`를 통한 config.yaml 조회
- `extension.js`의 `_settings` 사용은 5개 유지 키 + `enable-extension`로만 국한

| extension.js 지점 | before | after |
|------|--------|-------|
| L59, L65 `show-panel-indicator` | `_settings.get_boolean` | **변경 없음** (유지 키) |
| L74, L79 `enable-ime` | `_settings.get_boolean` | **변경 없음** (유지 키) |
| L465 `hanja_keys` | `_dbusIME.getConfig('hanja_keys')` | **변경 없음** (이미 DBus 경로) |
| L522 `shortcut-normal{,-reverse}` | `_settings.get_strv` | **변경 없음** (유지 키) |
| L544 `enable-extension` | `_settings.get_boolean` | **변경 없음** (유지 키) |
| L594 `show-notification` | `_settings.get_boolean` | **변경 없음** (유지 키) |

결론: **extension.js 코드 수정 불필요.** Phase 4 목표인 "단일 창구화"가 사실상 이전부터 설계되어 있었고, 이 Phase에서는 gschema 정리 + prefs.js 단순화 + dbus_ime.js에 범용 ConfigChangedJson 인프라 추가만으로 충분.

---

## 4. `dbus_ime.js` ConfigChangedJson 구독 스니펫

```javascript
// Constructor (L56-61)
this._onAutoTypeFix = null;
this._onConfigChanged = null;
this._configCache = null;

// Public API (L64-82)
getCachedConfig() { return this._configCache; }
setOnConfigChanged(cb) { this._onConfigChanged = cb || null; }

// g-signal handler (L113-134) — GlobalModeChanged 병존
this._imSignalId = this._imProxy.connect('g-signal',
    (proxy, senderName, signalName, parameters) => {
        if (signalName === 'GlobalModeChanged' && this._onModeChanged) {
            const [isKorean] = parameters.deep_unpack();
            this._onModeChanged(isKorean);
        } else if (signalName === 'ConfigChangedJson') {
            const [jsonStr] = parameters.deep_unpack();
            try {
                const cfg = JSON.parse(jsonStr);
                this._configCache = cfg;
                if (this._onConfigChanged) this._onConfigChanged(cfg);
            } catch (e) {
                unimError('DBUS_IME', `ConfigChangedJson 파싱 실패: ${e.message}`);
            }
        }
    }
);

// 초기 로드 (L168-197)
_loadInitialConfig() {
    try {
        const result = this._imProxy.call_sync(
            'GetConfigJson', null, Gio.DBusCallFlags.NONE, DBUS_TIMEOUT_MS, null
        );
        if (!result) return;
        const [jsonStr] = result.deep_unpack();
        this._configCache = JSON.parse(jsonStr);
        if (this._onConfigChanged) this._onConfigChanged(this._configCache);
        unimLog('DBUS_IME', `GetConfigJson 초기 로드 완료 (${jsonStr.length} bytes)`);
    } catch (e) {
        unimError('DBUS_IME', `GetConfigJson 실패 (무시하고 계속): ${e.message}`);
    }
}
```

**소비자 확장 지침 (향후 작업)**: extension.js에서 hanja_keys·layout 등 config.yaml 필드에 실시간으로 반응해야 하는 경우 `this._dbusIME.setOnConfigChanged((cfg) => { ... })` 로 구독하고 `cfg.engine.korean.layout` 등으로 접근. 매 키스트로크 DBus 호출 금지 — `getCachedConfig()` 사용.

---

## 5. `prefs.js` 최종 구조 (ASCII)

```
Adw.PreferencesWindow
└─ Adw.PreferencesPage "UNIM" (input-keyboard-symbolic)
   │
   ├─ Group "일반 설정"
   │   └─ ActionRow "UNIM 설정 앱 열기" (activatable)
   │        suffix: Gtk.Image(go-next-symbolic)
   │        activated → Gio.Subprocess.new(['unim-gui-gtk', '--settings'])
   │        → window.close()
   │        fallback: try/catch + Adw.Toast(5s) + unimError
   │      description: "자판·입력 모드·오타 교정 등 일반 설정은
   │                    UNIM 설정 앱(unim-gui-gtk --settings)에서 관리합니다."
   │
   ├─ Group "표시"
   │   ├─ SwitchRow "상단 패널 인디케이터" → show-panel-indicator
   │   └─ SwitchRow "변환 알림 표시" → show-notification
   │
   ├─ Group "실시간 입력기"
   │   └─ SwitchRow "IME 모드 활성화" → enable-ime
   │        XDG_SESSION_TYPE != wayland → set_sensitive(false) + 안내 subtitle
   │      description: "Wayland 세션 전용. Clutter.InputMethod로 IBus를 대체합니다."
   │
   └─ Group "변환 단축키"
       ├─ ShortcutRow "영어 → 한글" → shortcut-normal
       └─ ShortcutRow "한글 → 영어" → shortcut-normal-reverse
          (Gtk.Entry + reset button, edit-undo-symbolic, flat)
```

**제거된 UI**: 한국어/영어 레이아웃 ComboRow, 초기 모드/모드 공유 ComboRow, 팝업 모드 ComboRow, AutoTypeFix 4개 SpinRow + 3개 SwitchRow, `_syncToConfigFile()`, `_syncAutoTypeFixToConfig()` 헬퍼 전체.

---

## 6. busctl introspect 검증

**제약**: 이 환경에서는 시스템에 설치된 `/usr/libexec/unim-daemon` (옛 빌드, `GetConfigJson` 미포함)이 DBus activation으로 계속 되살아나 debug 바이너리 검증이 불가. 재설치 후 재검증이 필요.

검증 가능한 수준:
- `cargo build --workspace --release` zero warning — 인터페이스 선언·구현 모두 타입 체크 통과
- `client.rs`의 `#[proxy]` trait에 `get_config_json` 메서드 선언 컴파일 성공 → zbus introspect XML 생성 시 `GetConfigJson`이 포함될 것이 보장됨
- 기존 `get_config_yaml` 패턴을 그대로 따름 (Phase 2 보고서 §5.1 introspect 로그에서 `GetConfigYaml` 확인됨)

사용자가 `sudo make install PREFIX=/usr` 후:
```bash
busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod | grep Config
# 기대값:
# .GetConfig        method s s
# .GetConfigJson    method - s    ← NEW
# .GetConfigYaml    method - s
# .SetConfig        method ss -
# .SetConfigYaml    method s -
# .ConfigChanged        signal ss
# .ConfigChangedJson    signal s

busctl --user call org.atit.unim.InputMethod /org/atit/unim/InputMethod \
    org.atit.unim.InputMethod GetConfigJson
# 기대: s "{\"engine\":{...},\"frontend\":{...},...}" (valid JSON)
```

---

## 7. 수동 테스트 체크리스트 (make dev-extension 후)

사용자가 `sudo make install PREFIX=/usr && make dev-extension` 수행 후 재로그인(Wayland) 또는 Alt+F2→r(X11) 하고 아래 확인:

- [ ] `gnome-extensions prefs unim@atit.or.kr` 실행 → **1 페이지만** 노출 (UNIM)
- [ ] "UNIM 설정 앱 열기" 클릭 → `unim-gui-gtk --settings` 실행되며 prefs 창 닫힘
- [ ] unim-gui-gtk 미설치 상태에서 클릭 → Toast "실행 실패" 표시 (에러 로그 `~/.unim-errors.log`)
- [ ] "표시" 그룹에서 상단 패널 인디케이터 토글 → 즉시 반영
- [ ] "표시" 그룹에서 변환 알림 토글 → 오타 교정 시 알림 on/off 반영
- [ ] Wayland 세션: "IME 모드 활성화" 활성, 토글 시 즉시 반영
- [ ] X11 세션: "IME 모드 활성화" 비활성 상태 + "Wayland 세션에서만 사용 가능합니다." 서브타이틀
- [ ] "변환 단축키" 그룹의 entry에 `<Super>k` 수정 → 기존 단축키 해제 후 새 바인딩
- [ ] 단축키 reset 버튼 클릭 → 기본값 복원
- [ ] GTK 설정 앱에서 한국어 자판 변경 → `~/.config/unim/config.yaml` 갱신 + daemon의 `ConfigChangedJson` signal 방출 + `~/.unim-errors.log`에 "GetConfigJson 초기 로드 완료" 로그 (UNIM_DEVELOP=1)
- [ ] `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod | grep GetConfigJson` → 메서드 존재
- [ ] `busctl --user call ... GetConfigJson` → `serde_json` 직렬화 결과 (valid JSON) 반환

---

## 8. 사용자 마이그레이션 영향 (Phase 6 연계)

**위험**: 기존 사용자가 GNOME prefs에서 다음 키를 커스터마이징했을 수 있음:
- `korean-layout`, `english-layout`, `initial-mode`, `mode-sharing`, `popup-mode`
- `toggle-keys`, `hanja-keys`
- `auto-typefix-*` (6개)

`gsettings reset` 또는 schema 삭제로 이 값들이 기본값으로 돌아가면 **사용자 설정 손실**.

**완화 (Phase 6 마이그레이션 루틴에 전달해야 할 요구사항)**:
1. `unim-daemon` 최초 기동 시 1회성 마이그레이션 루틴 실행 (`~/.config/unim/.migrated-v2` 가드)
2. 삭제 전 gschema 키 목록 (마이그레이션 대상):
   ```
   GSettings 키 ↔ config.yaml 필드 매핑
   korean-layout ('2bul' 등) → engine.korean.layout (Dubeolsik 등)
   english-layout              → engine.english.layout
   initial-mode                → engine.default_category
   mode-sharing                → engine.mode_sharing
   popup-mode                  → engine.popup_mode
   toggle-keys (as)            → engine.toggle_keys (Vec<String>)
   hanja-keys (as)             → engine.hanja_keys (Vec<String>)
   auto-typefix-enabled        → engine.auto_typefix.enabled
   auto-typefix-forward        → engine.auto_typefix.forward
   auto-typefix-reverse        → engine.auto_typefix.reverse
   auto-typefix-time-window    → engine.auto_typefix.time_window_ms
   auto-typefix-kor-threshold  → engine.auto_typefix.kor_syllable_threshold
   auto-typefix-eng-min-length → engine.auto_typefix.eng_word_min_length
   ```
3. **읽기 방법**: 스키마 삭제 후에도 `dconf read /org/gnome/shell/extensions/unim/<key>` 로 원시 값 접근 가능. daemon의 마이그레이션은 `dconf` CLI 또는 gio `Settings.new_with_path` (schema-less)로 접근해야 함.
4. 마이그레이션 조건: config.yaml이 기본값인 경우에만 dconf 값으로 덮어쓰기 (사용자가 GTK에서 이미 수정했다면 보존).
5. 마이그레이션 후 해당 dconf 키 삭제 (`dconf reset` 또는 무시).

---

## 9. 검증 판정 표

| 검증 레벨 | 명령 | 결과 |
|-----------|------|------|
| L2 | `cargo build --workspace --release` | ✓ zero warning (24.04s) |
| L2 | `cargo test --workspace` | ✓ 283 passed (unim 254, unim-dbus 4, unim-gui-common 6, doctests 19), 0 failed, 2 ignored |
| gschema | `glib-compile-schemas unim-gnome-extension/schemas/` | ✓ exit=0, `gschemas.compiled` 생성 |
| 삭제 키 잔존 검사 | `grep -rE "'(korean-layout\|...)'" *.js` | ✓ prefs.js: 0건, extension.js: 0건 |
| 유지 키 개수 | schema XML `<key>` count | ✓ 7개 (Plan D 명세 5개 + `enable-extension` 유지 + shortcut 2개) |
| JS 문법 | `node --check *.js` | ✓ 문법 에러 없음 (`gi://` import는 경고 없이 파싱됨) |
| busctl introspect GetConfigJson | 샌드박스 환경 제약으로 미실행 | — (사용자 재설치 후 검증) |

---

## 10. Phase 5, 6 인수인계

### Phase 5 · config-editor (CLI / Qt 리다이렉트)

- `unim-config` CLI는 `auto_typefix.skip_on_english_word`, `skip_on_complete_syllable`, `engine.manual_shortcuts.forward/reverse` 신규 키 반영 완료 가정 (Phase 1 산출물 §6 인수인계).
- Qt GUI (`unim-gui-qt`)의 설정 창 진입점을 `std::process::Command::new("unim-gui-gtk").arg("--settings").spawn()`로 전환.
- GNOME prefs와 Qt GUI 모두 **리다이렉트 일관성** — Phase 3 `--settings` 엔트리포인트 재사용.

### Phase 6 · migrator (unim-daemon 기동 루틴)

- §8의 13개 gschema 키 ↔ config.yaml 필드 매핑 표가 인수 자료.
- `~/.config/unim/.migrated-v2` 가드.
- `dconf` CLI 또는 `Settings.new_with_path`(schema-less)로 구 키 읽기.
- config.yaml 필드가 기본값일 때만 덮어쓰기.
- 마이그레이션 후 `dconf reset /org/gnome/shell/extensions/unim/korean-layout` 등으로 구 키 정리.

### Phase 7 · reviewer

- 레거시 `GetConfig`/`SetConfig`/`ConfigChanged` (단일 키) 제거 여부 판단.
- `enable-extension` 키의 장기 유지 여부 (GNOME 표준 on/off와 중복).
- end-to-end: GTK에서 한국어 자판 변경 → `ConfigChangedJson` signal → extension의 `_configCache` 갱신 → 인디케이터 텍스트 즉시 반영 (현재는 소비자 코드 없음, 향후 이런 워크플로 필요 시 활용).
- 수동 재설치·재로그인 후 §7 체크리스트 수행.

---

## 11. 주요 결정사항 요약

1. **GetConfigJson 증분 추가** (service.rs + client.rs): YAML 파서 없는 JS 환경을 위한 대칭 API. ConfigChangedJson signal과 payload 포맷 통일.
2. **extension.js 무수정**: 삭제된 키 13개의 extension.js 내 사용처 0건. 무의미한 치환 작업 회피.
3. **enable-extension 유지**: 실제 사용처(extension.js:544) 확인.
4. **dbus_ime.js에 인프라만 추가**: 현재 소비자 코드 없지만 향후 확장을 위한 캐시·콜백 API 선행 구축.
5. **prefs.js 461 → 165 lines**: 시각적 단순성 + 유지보수 부담 경감.
6. **busctl 검증 보류**: 설치본 DBus activation 간섭으로 debug daemon 검증 불가. 사용자 재설치 후 §7 체크리스트로 검증.
