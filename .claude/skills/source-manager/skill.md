---
name: source-manager
description: UNIM 저장소·파일·git/GitHub 운영 패턴. 폴더/파일 분류·이동·삭제, .gitignore, 브랜치(develop/main/release/claude-*), 커밋 분할, PR/머지, 릴리스 태그, CHANGELOG 동기화. "파일 정리", "git 정리", "브랜치 정리", "커밋 분할", "PR 머지", "릴리스 태그" 트리거.
---

# Source Manager Operating Pattern

## 분류 4가지
| 분류 | 처리 | 안전 |
|------|------|------|
| DELETE | `git rm` (tracked) / `rm` (untracked) | 단일 파일 단위 |
| MOVE | `git mv` | 참조 grep 후 이동 |
| GITIGNORE | `.gitignore` 추가 + `git rm --cached` | 추적 제거 동반 |
| KEEP | 그대로 | 의심되면 KEEP |

## git 운영

### 브랜치
- `develop`: 통합 기본
- `main`: 릴리스
- `release/0.x.y`: 릴리스 후보
- `feature/<topic>` 또는 `claude/<topic>`: 작업

### 커밋 (사용자 승인 시만)
HEREDOC + Conventional Commits + Co-Authored-By:
```
git commit -m "$(cat <<'EOF'
<type>(<scope>): <subject>

<body — 왜·무엇·검증>

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### 큰 변경 분할
1. `git reset HEAD` 로 unstage
2. 의미 단위로 `git add <paths>` + `git commit`
3. 각 커밋 후 `git status --short` 확인

### PR
- `gh pr create --base develop --title "..." --body "$(...)"` (HEREDOC body)
- `gh pr checks <N>` CI 확인
- 머지 전 build-validator + reviewer 에이전트 사전 검증 의뢰

## 릴리스 5지점 동기화
1. `Cargo.toml` workspace.package.version
2. `unim-gnome-extension/metadata.json` version
3. `debian/changelog` 새 항목
4. `PKGBUILD` pkgver
5. `CHANGELOG.md` + `CHANGELOG-ko.md` 새 섹션

```bash
# 검증
grep -rEh '^version' Cargo.toml unim-gnome-extension/metadata.json PKGBUILD
head -3 debian/changelog CHANGELOG.md CHANGELOG-ko.md
```

## .claude/worktrees 관리
```bash
git worktree list
# inactive 확인 후 정리
git worktree remove --force <path>  # 또는 디렉토리 삭제
```

## 절대 금지
- `git reset --hard`, `git push --force`, `git clean -fdx`
- `rm -rf` 임의 디렉토리
- `--no-verify` / `--no-gpg-sign`
- 의심되면 KEEP, PM에게 사용자 판단 요청

## 출력 양식
```markdown
## Source Manager Report — {ID}
| 경로 | 작업 | 사유 | 결과 |

git: 브랜치 / staged N / unstaged M
검증: make build PASS / git status clean
사용자 판단 필요: ...
```
