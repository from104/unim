# 설정 파일 템플릿

UNIM의 실제 설정 파일은 `~/.config/unim/config.yaml`에 자동 생성됩니다.
이 디렉토리는 "어떤 키가 있고 무엇을 의미하는지" 한눈에 보이는 주석 포함 참고본입니다.

| 파일 | 설명 |
|------|------|
| [`example.yaml`](example.yaml) | 전체 필드 + 기본값 + 범위 주석 |

## 사용법

이 파일을 복사해 써도 되고, 원하는 필드만 참조해 자신의 `config.yaml`에 추가해도 됩니다:

```bash
cp examples/config/example.yaml ~/.config/unim/config.yaml
# 또는 필요한 부분만 발췌
```

데몬 실행 중이면 자동으로 감지해 리로드합니다 (파일 mtime 감시).

## CLI로 안전하게 편집

YAML을 직접 편집하지 않고 `unim-cli config`로 변경하면 범위 검증이 자동으로 적용됩니다:

```bash
unim-cli config set korean-layout 3bul390
unim-cli config set auto-typefix-tentative-expiry-hours 6
unim-cli config show
```

전체 CLI 키 목록은 [`../../unim-cli/SPEC.md`](../../unim-cli/SPEC.md) 참조.

## 관련 문서

- 구조체·범위 원본: [`../../src/SPEC.md §3`](../../src/SPEC.md)
- AutoTypeFix 동작: [`../../IME_BEHAVIOR.md §9`](../../IME_BEHAVIOR.md)
- 설정 동기화(개발자용): [`../../GEMINI.md`](../../GEMINI.md)
