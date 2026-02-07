---
description: 변경사항 확인 및 Git 커밋
---

# 변경사항 커밋

// turbo-all

1. 현재 브랜치 확인
```bash
git branch --show-current
```

2. 변경된 파일 확인
```bash
git status
```

3. 변경 내용 diff 확인
```bash
git diff --stat
```

4. 스테이징 (변경된 파일 전체 추가)
```bash
git add -A
```

5. 커밋 (메시지는 상황에 맞게 작성)
```bash
git commit -m "<적절한 커밋 메시지>"
```

## 커밋 메시지 컨벤션

- `feat:` 새로운 기능
- `fix:` 버그 수정
- `refactor:` 리팩토링
- `docs:` 문서 변경
- `chore:` 빌드/설정 변경
- `test:` 테스트 추가/수정
