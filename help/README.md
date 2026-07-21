# help/ — 오프라인 도움말 (생성물)

이 디렉토리의 `.html` 파일은 **자동 생성물이다. 직접 편집하지 마라.**
편집해도 다음 `make help-html` 에서 통째로 덮어써진다.

| 파일 | 내용 |
|------|------|
| `unim-help-ko.html` | 한국어 도움말 (사용자 매뉴얼 · 단축키 · FAQ · 문제 해결 병합) |
| `unim-help-en.html` | 영어 도움말 (동일 구성) |

## 내용을 고치려면

원본 마크다운을 고치고 재생성한다.

```
docs/user/user-guide/README-ko.md          docs/user/user-guide/README.md
docs/user/keyboard-shortcuts/README-ko.md  docs/user/keyboard-shortcuts/README.md
docs/user/faq/README-ko.md                 docs/user/faq/README.md
docs/user/troubleshooting/README-ko.md     docs/user/troubleshooting/README.md
```

```sh
make help-html      # 재생성
git add help/       # 산출물도 함께 커밋한다
```

`make check-help-html` 은 재생성 후 `git diff` 로 드리프트를 검출한다.
Linux CI 가 이 타깃을 돌리므로, 마크다운만 고치고 재생성을 잊으면 CI 가 막는다.

## 왜 산출물을 커밋하는가

패키징(deb / rpm / MSI)은 이 파일들을 **복사만** 한다.
그래야 배포 빌드에 마크다운 렌더러 의존성이 끼어들지 않는다.

## 생성기

`tools/gen-help/` (Rust, `unim-gen-help`). 개발 도구이며 배포 대상이 아니다.

- 4개 문서를 언어별 HTML **한 장**으로 병합하고, 문서 간 상대 링크를 내부 앵커로 바꾼다
- CSS 인라인 · 외부 CDN/폰트/이미지/스크립트 참조 0 —
  `C:\Program Files\` 나 `/usr/share` 아래에서 `file://` 로 열어도 완전히 동작한다
- 앵커를 찾지 못한 링크는 빌드를 깨뜨리지 않고 경고를 남긴 뒤 GitHub 절대 URL 로 폴백한다
