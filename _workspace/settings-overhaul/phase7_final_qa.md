# Phase 7 — 최종 QA · 잔존 버그 수정 · 문서 갱신 산출물

## 작업 A — `%{error}` 리터럴 누출 버그 수정

### 원인

- rust-i18n 3.x의 placeholder 문법 `%{var}` 자체는 **정상**.
- 문제는 `t!("error_label")` **호출부가 `error = ...` 인자를 전달하지 않은 것**.
- 호출 3곳 모두 `eprintln!("{}: {}", t!("error_label"), e)` 포맷으로 에러를 별도 결합하고 있어, yml의 `: %{error}` 템플릿 꼬리가 불필요한 중복이자 누출 원인이었다.

### 호출 위치 (변경 없음, 참고만)

- `unim-config/src/main.rs:686`
- `unim-config/src/main.rs:716`
- `unim-config/src/main.rs:723`

### 수정 (로케일 yml만 수정 — 제약: 둘 중 하나만)

| 파일 | 라인 | Before | After |
|------|------|--------|-------|
| `unim-config/locales/en.yml` | 26 | `error_label: "Execution Error: %{error}"` | `error_label: "Execution Error"` |
| `unim-config/locales/ko.yml` | 26 | `error_label: "실행 오류: %{error}"` | `error_label: "실행 오류"` |

호출부가 `{}: {}` 포맷으로 이미 에러를 붙이므로 최종 출력은 동일 형태 유지.

### 재현 테스트

```
$ cargo build --release -p unim-config
    Finished `release` profile [optimized] target(s) in 1.33s
$ ./target/release/unim-config set auto-typefix-kor-threshold 99
Execution Error: Range 2~6, got 99
```

`%{error}` 리터럴 소멸 확인 — PASS.

---

## 작업 B — End-to-End QA

### B-1. 자동 검증

| 항목 | 결과 |
|------|------|
| `cargo build --workspace --release` | **PASS** (zero warning, 증분 빌드 이후 0.21s) |
| `cargo test --workspace` | **PASS** (핵심 doc-test 포함 전 테스트 통과) |
| `make build` | **PASS** (GTK3/4, Qt5/6, XIM, Wayland 포함 무경고) |
| `make build-tests` | **PASS** (unim-test-xim/-gnome/-wayland 모두 성공) |
| `glib-compile-schemas unim-gnome-extension/schemas/` | **PASS** |

### B-2. DBus 인터페이스 일관성

busctl introspect는 현재 세션에 daemon이 기동되지 않아 직접 확인 불가(환경 제약).
대신 `unim-dbus/src/service.rs` 선언을 grep으로 검증:

| 요구 | 선언 라인 | 상태 |
|------|-----------|------|
| `GetConfig` | 389 | **PASS** (legacy 유지) |
| `SetConfig` | 421 | **PASS** (legacy 유지) |
| `GetConfigYaml` | 565 | **PASS** |
| `GetConfigJson` | 577 | **PASS** |
| `SetConfigYaml` | 593 | **PASS** |
| `GlobalModeChanged` signal | 319 | **PASS** |
| `ConfigChanged` signal | 326 | **PASS** (legacy) |
| `ConfigChangedJson` signal | 337 | **PASS** |

Phase 2 합의(신/구 병존)와 완전 일치. 사용자 수동 검증 항목(아래 종합 판정) 참조.

### B-3. 설정 6지점 동기화 최종 확인

신규 3필드 `skip_on_english_word`, `skip_on_complete_syllable`, `manual_shortcuts`:

| 지점 | 상태 | 근거 |
|------|------|------|
| `src/config.rs` | **PASS** | Phase 1 산출물 |
| `unim-config` CLI (main.rs ConfigKey) | **PASS** | Phase 5-B (`AutoTypeFixSkipEnglishWord`, `AutoTypeFixSkipCompleteSyllable`, `ManualShortcutForward`, `ManualShortcutReverse`) |
| `unim-config/locales/*.yml` | **PASS** | Phase 5-C (ko/en 4개 라벨) |
| `unim-dbus` | **PASS** | YAML/JSON 엔드포인트로 serde 자동 직렬화 — 필드 수정 불필요 |
| `unim-gui-gtk` | **PASS** | Phase 3 위젯 바인딩 |
| `gschema` | **N-A** | 의도적 제외 — 일반 설정은 config.yaml 전용(Phase 6 결정) |

### B-4. 레거시 DBus key-value API — 제거 결정

`GetConfig` / `SetConfig` / `ConfigChanged` (key/value 레거시) 호출처 grep 결과:

| 파일 | 라인 | 호출자 |
|------|------|--------|
| `unim-frontends/gtk-common/src/unim_dbus_client.c` | 860 | GTK3/4 IM 모듈 공통 |
| `unim-frontends/qt5/src/input_context.cpp` | 144 | Qt5 플러그인 |
| `unim-frontends/qt6/src/input_context.cpp` | 145 | Qt6 플러그인 |
| `unim-gnome-extension/dbus_ime.js` | 704 | GNOME Shell extension |
| `tests/common/unim_test.c` | 40 | 공용 테스트 하네스 |

**결정: 제거 보류 (Keep)**.
- 프로덕션 프론트엔드 5종 모두가 여전히 사용 중 — 일괄 제거 시 회귀 리스크 과다.
- Phase 2에서 "병존 결정"의 전제가 아직 해소되지 않음.
- 다음 메이저 릴리스에서 **deprecation 경로**로 단계적 제거 권고: ① 로그에 deprecation 경고 추가 → ② 프론트엔드 이관 완료 후 → ③ 제거.

---

## 작업 C — 문서 갱신

| 파일 | 변경 요약 |
|------|-----------|
| `CLAUDE.md` | "Settings Synchronization" 섹션 개편. 5지점 명시, gschema는 GNOME Shell 전용임을 명시. DBus 신규/레거시 API, 마이그레이션, GTK GUI 단일 창구 설명 추가 |
| `GEMINI.md` | "설정 항목 연동 가이드라인" 섹션 전면 갱신. 5지점 표, DBus API 표(GetConfigYaml/GetConfigJson/SetConfigYaml/ConfigChangedJson), Phase 6 마이그레이션 루틴(.migrated-v2 가드) 설명 추가. 예시 섹션도 신규 5지점에 맞춰 업데이트 |
| `unim-dbus/SPEC.md` | §5.1 메서드 표에 `GetConfigYaml`/`GetConfigJson`/`SetConfigYaml` 추가, 레거시 메서드에 "(legacy)" 주석. §5.2 시그널 표에 `ConfigChangedJson` 추가 |

**새 문서 생성 없음**(제약 준수). `AGENTS.md`는 config.yaml/config.rs 수준의 일반 언급만 있어 수정 불필요.

---

## 종합 판정

### Phase 1~7 완료 상태

| Phase | 내용 | 상태 |
|-------|------|------|
| 1 | Config 구조체 확장 + `clamp_ranges()` | DONE |
| 2 | DBus YAML/JSON API + ConfigChangedJson signal | DONE (병존) |
| 3 | GTK GUI 위젯 확장 | DONE |
| 4 | GNOME 마이그레이션 준비 | DONE |
| 5 | Qt 리다이렉트 + CLI 확장 + locale | DONE |
| 6 | GSettings → config.yaml 1회성 마이그레이션 (.migrated-v2) | DONE |
| 7 | 최종 QA + %{error} 버그 수정 + 문서 갱신 | **DONE** |

### 사용자 남은 수동 검증 항목

1. **daemon 재기동**: `pkill -f unim-daemon && UNIM_DEVELOP=1 target/release/unim-daemon -n` — 마이그레이션 로그 확인(`~/.config/unim/.migrated-v2` 최초 생성 시나리오)
2. **busctl introspect**: daemon 기동 상태에서 `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod`로 5개 메서드 + 3개 signal 모두 노출 확인
3. **GTK GUI 실행**: `unim-gui-gtk --settings` — 신규 3필드(skip_on_english_word, skip_on_complete_syllable, manual_shortcuts) 위젯 표시/저장/재로드 확인
4. **Qt 트레이 리다이렉트**: Qt6 GUI 트레이 "설정" → GTK GUI subprocess 기동 확인
5. **GNOME Extension 재로드**: `gnome-extensions disable/enable org.atit.unim@unim`로 prefs.js 리다이렉트 확인
6. **CLI 회귀**: `unim-config set auto-typefix-kor-threshold 99` → 에러 메시지 정상(본 Phase 재현 완료)

### Git 커밋 제안 구조

**권고: 단일 커밋** (Phase 7 한정).

이유:
- 본 Phase에서는 코드 변경이 yml 2라인뿐 (버그 픽스).
- 나머지는 문서 갱신 3개 파일.
- 논리적으로 "Phase 7 최종 QA + 버그픽스 + 문서 갱신"이 한 덩어리로 응집.
- Phase 1~6은 이미 각자 커밋됐다는 전제(승인 plan의 Phase 경계 준수).

제안 커밋 메시지:

```
fix: drop unused %{error} placeholder in unim-config error_label

The error_label locale template had a `: %{error}` placeholder, but
callers print the error via separate `eprintln!("{}: {}", t!(...), e)`
format. rust-i18n left the placeholder unrendered, leaking the literal
`%{error}` into user-facing output. Drop the placeholder; output shape
unchanged.

Also finalize Phase 7 of the settings overhaul:
- CLAUDE.md / GEMINI.md: document the 5-point settings sync model,
  config.yaml as single source, gschema reserved for GNOME-only keys.
- unim-dbus/SPEC.md: add GetConfigYaml / GetConfigJson / SetConfigYaml
  methods and ConfigChangedJson signal.
- Keep legacy GetConfig/SetConfig/ConfigChanged (5 live callers); plan
  deprecation for a future release.
```

만약 Phase 1~6도 아직 미커밋 상태라면, 각 Phase별로 분리 커밋하여 `_workspace/settings-overhaul/phaseN_*.md`와 매칭시키는 것이 bisect/revert에 유리.
