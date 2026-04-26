---
name: release-cleanup
description: UNIM 0.2.0 릴리즈 직전 프로젝트 정리. 루트 잡파일·중복 문서·빈 로그·stale worktree·미참조 산출물을 식별하고 안전 삭제·이동·gitignore 처리. "릴리즈 정리", "프로젝트 정리", "잡파일 제거", "워크트리 정리", "릴리즈 청소" 요청 시 반드시 트리거. 위험 작업(force push, hard reset)은 절대 수행 금지.
---

# Release Cleanup — 정리 작업 패턴

## 트리거 시점
0.2.0 릴리즈 직전, 또는 사용자가 "정리해줘"·"청소해줘"라고 명시한 경우. 일반 작업 중 임시 파일 정리는 트리거하지 않음.

## 분류 기준 (4단계)

| 분류 | 처리 | 예시 |
|------|------|------|
| **DELETE** | 즉시 삭제 가능 | 0바이트 파일, *.tmp, *.bak, 명백한 stale 로그 |
| **MOVE** | 적절한 위치로 이동 | legacy 문서 → `docs/dev/history/`, plan 산출물 → `_workspace/archive/` |
| **GITIGNORE** | 추적에서 제외 | 사용자 로컬 캐시, 빌드 산출물 |
| **KEEP** | 의도적 보존 | 의미 있는 빈 파일, 문서화된 placeholder |

## 안전 절차

### 1. Inventory
```bash
ls -la /home/from104/work/unim
find /home/from104/work/unim -maxdepth 2 -type f -size 0 -not -path '*/.git/*' -not -path '*/target/*'
git -C /home/from104/work/unim status --short
git -C /home/from104/work/unim worktree list
```

### 2. 분류 판단 (각 파일별)
1. `git log --oneline -- <path>` — 최근 변경 이력
2. `grep -rn '<filename>' /home/from104/work/unim` — 다른 파일에서 참조 여부
3. 파일 내용 확인 (size > 0인 경우 head)
4. 분류 결정

### 3. 처리 실행
- DELETE (tracked): `git rm <path>`
- DELETE (untracked): `rm <path>` 단, **단일 파일 단위로**
- MOVE: `git mv <src> <dst>`
- GITIGNORE: `.gitignore`에 추가
- 디렉토리 삭제는 **반드시** 내부 파일을 모두 분류·처리 후

### 4. 매 단계마다 검증
```bash
git status --short  # 의도된 변경만 staged
make build          # warning 0 유지
```

## 절대 금지
- `rm -rf` 임의 디렉토리 (구체적 파일 단위로만)
- `git reset --hard`, `git clean -fdx`
- `target/`, `node_modules/` 외 디렉토리 통째로 삭제
- 의심스러우면 KEEP, 보고서에 사용자 판단 요청

## 보고서 양식
`_workspace/release/00_cleanup_report.md`로 출력. 표 형식: `| 경로 | 분류 | 사유 | 처리 결과 |`. 검증 결과(make build, git status)도 포함.

## 사례

### .unim-errors.log (0바이트)
- 분류: DELETE + GITIGNORE
- 처리: `rm`, `.gitignore`에 `*.unim-errors.log` 추가

### .claude/worktrees/agent-a312d99906b0399cd/
- `git worktree list`로 활성 여부 확인
- inactive면: `git worktree remove --force <path>` 또는 디렉토리 삭제
- active면: 사용자 판단 요청 (KEEP)

### docs/dev/history/agent-legacy/
- 내부 SKILL.md/workflows.md 다수
- 각 파일이 현재 `.claude/skills/`로 이관됐는지 확인
- 이관 완료 시 → 디렉토리 통째 삭제 가능 (단일 디렉토리이므로 OK)
- 일부만 이관되면 → KEEP 또는 selective DELETE
