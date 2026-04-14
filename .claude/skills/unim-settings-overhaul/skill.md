---
name: unim-settings-overhaul
description: UNIM 설정 시스템 전면 개편(단일 창구화 + GTK GUI 재설계) 7-Phase 오케스트레이션. config.yaml 단일 진실 공급원 확립, DBus GetConfig/SetConfig/ConfigChanged 완성, GTK4+libadwaita 다이얼로그 전면 재작성, GNOME extension 정리, Qt GUI 리다이렉트, 마이그레이션, QA를 순차 실행. "설정 개편", "설정 통합", "단일 창구", "Phase 1부터", "다음 Phase" 맥락에서 트리거. 승인된 plan `/home/from104/.claude/plans/golden-hatching-pnueli.md`를 입력으로 사용.
---

# UNIM Settings Overhaul — 오케스트레이터

승인된 plan(`/home/from104/.claude/plans/golden-hatching-pnueli.md`)에 따라 7개 Phase를 순차 실행한다. 각 Phase는 주 구현 에이전트 → reviewer 검증 → 사용자 승인 → 다음 Phase 순.

## 실행 모드

**서브 에이전트 모드** (파이프라인). Phase가 엄격히 순차 의존하므로 team/TaskCreate 불필요. 메인 Claude가 오케스트레이터 역할로 Agent 도구 호출.

## 산출물 저장소

모든 중간 산출물은 `/home/from104/work/unim/_workspace/settings-overhaul/`에 저장:

```
_workspace/settings-overhaul/
├── phase1_config_editor.md
├── phase2_dbus_implementer.md
├── phase3_gtk_designer.md
├── phase4_gnome_migrator.md
├── phase5_misc.md
├── phase6_migration.md
└── phase7_final_qa.md
```

각 보고서에는 수정 파일 목록(file:line), 검증 결과(build/test), 다음 Phase로의 인수인계 사항, 발견한 이슈·결정사항을 기록.

## Phase 실행 계획

### Phase 1 — Config 구조 확장

**담당**: `config-editor` (+ `test-writer` 보조)

**목표**: `AutoTypeFixConfig`에 `skip_on_english_word`, `skip_on_complete_syllable` 추가. 범위 확장(kor 2~6, eng 3~8). `src/auto_typefix.rs` 로직 업데이트. (`EngineConfig.manual_shortcuts`는 Phase 8에서 제거됨 — GNOME 전용 키는 gschema에만 존재.)

**호출**:
```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "Phase 1 config 확장",
  prompt: "@/home/from104/work/unim/.claude/agents/config-editor.md 역할로 plan의 Phase 1 작업을 수행하라. 산출물은 _workspace/settings-overhaul/phase1_config_editor.md에 기록."
)
```

이어서 `test-writer`로 serde 역호환성 + 범위 테스트 작성 요청.

**완료 조건**: `cargo test -p unim` all pass, `cargo build --workspace` zero warning.

### Phase 2 — DBus GetConfig/SetConfig/ConfigChanged

**담당**: `dbus-implementer`

**핵심 결정사항** (gnome-migrator와 사전 합의됨): `ConfigChanged` signal payload는 **JSON 문자열** (JS 파싱 호환).

**완료 조건**: `busctl --user call ... GetConfig` 성공, `busctl --user monitor`에서 SetConfig 후 signal 관찰.

### Phase 3 — GTK 다이얼로그 전면 재설계

**담당**: `gtk-designer`

**핵심**: `unim-gui-gtk/src/settings_dialog.rs` 전면 재작성. plan Phase F의 3페이지 구조. 시스템 테마 자동. ForceDark 제거.

**완료 조건**: `make build-frontends` 성공, `make sandbox-gtk4`에서 UI 수동 확인, 값 변경 → `cat ~/.config/unim/config.yaml` 반영 확인.

### Phase 4 — GNOME extension 정리·전환

**담당**: `gnome-migrator`

**핵심**: gschema 13개 키 삭제, extension.js DBus 경로 전환, prefs.js 단순화 + 리다이렉트 버튼.

**완료 조건**: `glib-compile-schemas` 성공, `gnome-extensions prefs unim@atit.or.kr`에서 Shell 5개 + 리다이렉트 확인, GTK에서 한글 자판 변경 → extension indicator 즉시 반영.

### Phase 5 — Qt GUI 리다이렉트 + CLI/번역

**담당**: `config-editor` (CLI/번역) + 필요시 서브 에이전트(Qt 리다이렉트)

**Qt 리다이렉트**: `unim-gui-qt/src/...`에서 설정 진입점이 `std::process::Command::new("unim-gui-gtk").arg("--settings").spawn()` 호출하도록 변경.

**CLI**: `unim-config/src/main.rs`의 `ConfigKey` enum에 신규 키 3개 추가:
- `auto-typefix-skip-english-word`
- `auto-typefix-skip-complete-syllable`
- `manual-shortcut-forward`, `manual-shortcut-reverse`

**번역**: `unim-config/locales/*.yml` 모두 갱신.

### Phase 6 — 마이그레이션 루틴

**담당**: `config-editor` + `gnome-migrator` 협업

**위치**: `unim-daemon` 기동 시 1회성 마이그레이션 함수.

**로직**:
1. `~/.config/unim/.migrated-v2` 존재 시 skip
2. GSettings(`org.gnome.shell.extensions.unim`)에서 삭제 예정 13개 키 읽기 (gio)
3. config.yaml 값이 기본값인 것만 GSettings 값으로 덮어씀
4. `save_to_default_path()` 저장 후 `.migrated-v2` 생성

### Phase 7 — 최종 QA + 문서

**담당**: `reviewer`

**검증**:
- `cargo test --workspace` (L2)
- `make build` zero-warning (L3)
- 수동 end-to-end 7개 시나리오 (plan Verification 섹션)
- 문서 갱신: 관련 SPEC.md, GEMINI.md의 Settings Guide

## 각 Phase 승인 게이트

각 Phase 완료 후 reviewer 검증 → PASS 시 사용자에게 보고 → **사용자 명시 승인** 전까지 다음 Phase 진행 금지. (사용자 지시: "충분한 분석과 계획 승인 후 구현")

## 에러 핸들링

| 상황 | 처리 |
|------|------|
| Phase N 빌드 실패 | 해당 Phase 보고서에 에러 전문 기록, 에이전트에게 1회 재시도 요청, 재실패 시 사용자에게 에스컬레이션 |
| reviewer FAIL 판정 | 구체적 수정 지시를 받아 동일 에이전트에 재호출 (최대 2회) |
| 에이전트 간 결정 충돌 (예: signal payload 형식) | 오케스트레이터가 중재, plan 원문 → 사용자 순 |
| plan 범위를 벗어난 요구 발견 | 즉시 보고, plan 수정 여부 사용자 결정 |

## 참고 자료

- 승인된 plan: `/home/from104/.claude/plans/golden-hatching-pnueli.md`
- DBus 디버깅: `references/dbus-debug.md`
- GTK UI QA: `references/gtk-visual-qa.md`
- 프로젝트 규칙: `/home/from104/work/unim/CLAUDE.md`, `AGENTS.md`, `GEMINI.md`

## 테스트 시나리오

**정상 흐름**: Phase 1 config 확장 → cargo test pass → reviewer PASS → 사용자 승인 → Phase 2 진행 → ... → Phase 7 전체 검증 PASS.

**에러 흐름**: Phase 3 GTK 재설계에서 libadwaita SpinRow가 adw 0.7에 없다고 판명 → gtk-designer가 대안(ActionRow + SpinButton) 제안 → 오케스트레이터가 사용자에게 결정 요청 → 승인 후 수정 진행.
