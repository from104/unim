---
name: release-qa
description: UNIM 0.2.0 릴리즈 최종 QA. 빌드 zero-warning, cargo test all-pass, i18n ko/en 키 정합성, 문서 링크/짝 누락, 라이브 도움말 누락, CHANGELOG/버전 동기화를 검증하고 PASS/WARN/FAIL 판정. 수정 금지(발견만). "릴리즈 QA", "최종 검증", "릴리즈 점검", "0.2.0 검수" 요청 시 반드시 트리거.
---

# Release QA — 종합 검증 패턴

## 검증 8개 항목 (각각 PASS/WARN/FAIL)

### 1. BUILD
```bash
cargo build --workspace --release 2>&1 | tee _workspace/release/04_build.log
make build 2>&1 | tee -a _workspace/release/04_build.log
grep -E '^warning' _workspace/release/04_build.log | wc -l
```
- warning 0 → PASS
- warning > 0 → FAIL (목록 보고)

### 2. TEST
```bash
cargo test --workspace 2>&1 | tee _workspace/release/04_test.log
```
- 모두 통과 → PASS
- 실패 1개 이상 → FAIL

### 3. I18N 정합성
- ko.yml ↔ en.yml 키 집합 동일성:
```bash
yq -r 'keys[]' unim-cli/locales/ko.yml | sort > /tmp/ko_keys
yq -r 'keys[]' unim-cli/locales/en.yml | sort > /tmp/en_keys
diff /tmp/ko_keys /tmp/en_keys
```
- 사용했지만 정의 안 된 키 검출:
```bash
grep -roEh 't!\("[^"]+"' unim-* --include='*.rs' | sort -u
```
- 차이 0 → PASS, 차이 있음 → WARN/FAIL

### 4. DOCS
- 깨진 링크: `grep -rEn '\]\(\.[^)]*\.md\)' docs/` → 각 경로 존재 확인
- 한/영 짝: `README.md` ↔ `README-ko.md`, 모든 가이드 짝
- 코드블록 명령 1차 실행 가능성 (적어도 `--help`)

### 5. LIVE_HELP
- GTK 위젯 인벤토리:
```bash
grep -nE 'SwitchRow|SpinRow|ComboRow|ActionRow|EntryRow' unim-gui-gtk/src/settings_dialog.rs
```
- 각 위젯에 subtitle 또는 tooltip i18n 키가 등록됐는지

### 6. VISUAL (가능하면)
- `make sandbox-gtk4` (TTY 있으면)
- DBus mock: `cargo run -p unim-test-dbus`
- TTY 없으면 SKIP 처리

### 7. VERSION 정합성
- `Cargo.toml` workspace.package.version == 0.2.0
- `unim-gnome-extension/metadata.json` version 일치
- `CHANGELOG.md` [0.2.0] 섹션 존재
- `CHANGELOG-ko.md` 동일 섹션 동기화

### 8. CLEAN
- `git status --short` clean (의도된 변경만)
- 루트에 stale `.log`/`.tmp` 없음

## 출력 양식

`_workspace/release/04_qa_report.md`:
```markdown
# Release QA Report — 0.2.0

## 종합
| 항목 | 결과 |
|------|------|
| BUILD | PASS/FAIL |
| TEST | PASS/FAIL |
| I18N | PASS/WARN/FAIL |
| DOCS | PASS/WARN/FAIL |
| LIVE_HELP | PASS/WARN/FAIL |
| VISUAL | PASS/SKIP/FAIL |
| VERSION | PASS/FAIL |
| CLEAN | PASS/FAIL |

**머지 권고: 가능 / 수정 필요**

## 항목별 상세
(검증 명령 + 결과 + 미흡 시 file:line 또는 누락 키 목록)

## 권고
(WARN 항목별 수정 제안)
```

## 작업 원칙
- **수정 금지**: 발견만 보고서로
- **객관성**: 주관적 표현 금지
- **재현 가능성**: 모든 검증은 명령어로 재현 가능
- **SKIP 명시**: 환경 부재로 못 한 검증은 SKIP, 사유 명시
- 큰 로그는 `_workspace/release/04_*.log`로 분리

## 의존성 체크 — 도구 없음 시 fallback
- `yq` 없으면 Python `python3 -c "import yaml; ..."`
- `msgfmt` 없으면 SKIP
- TTY 없으면 VISUAL SKIP
