---
description: 한글 요약 기반 영문 커밋 및 깃헙 동기화 (git-sync 스킬 사용)
---

# Git Sync & Push

명령 시 변경 사항을 영문으로 요약하여 커밋하고 GitHub에 푸시합니다.

// turbo-all

1. 변경 사항 분석 및 영문 메시지 생성
   - `git-sync` 스킬의 가이드라인에 따라 변경 사항을 분석하고 영문 커밋 메시지를 생성합니다.

2. 현재 브랜치 확인
```bash
git branch --show-current
```

3. 스테이징
```bash
git add -A
```

4. 커밋 (자동 생성된 영문 메시지 사용)
```bash
git commit -m "<AI가 생성한 영문 커밋 메시지>"
```

5. 원격 저장소 동기화
```bash
git push origin $(git branch --show-current)
```
