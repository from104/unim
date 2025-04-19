# Unin - 한글 입력 시스템

Unin은 Rust로 작성된 한글 입력 시스템입니다. 이 프로젝트는 다양한 한글 입력 방식을 지원하며, 특히 2벌식과 3벌식 입력 방식을 구현하고 있습니다.

## 주요 기능

- **다양한 한글 입력 방식 지원**
  - 2벌식 입력 방식
  - 3벌식 입력 방식
  - 확장 가능한 입력 방식 구조

- **한글 자모 처리**
  - 초성, 중성, 종성 분리 및 조합
  - 특수 문자 처리
  - 완성형/조합형 한글 지원

- **키보드 입력 처리**
  - 영문 키보드 입력을 한글로 변환
  - 다양한 키보드 매핑 지원

## 기술 스택

- **Rust**: 안전하고 효율적인 시스템 프로그래밍 언어
- **X11**: 리눅스 시스템에서의 그래픽 인터페이스 지원
- **libc**: 시스템 수준의 기능 접근
- **serde_json**: JSON 데이터 처리

## 프로젝트 구조

```
src/
├── hangul/           # 한글 처리 관련 모듈
│   ├── char.rs       # 한글 문자 처리
│   ├── composer.rs   # 기본 한글 조합기
│   ├── composer_with_2bul.rs  # 2벌식 입력 지원
│   ├── composer_with_3bul.rs  # 3벌식 입력 지원
│   ├── jamo.rs       # 한글 자모 처리
│   └── mod.rs        # 모듈 정의
├── keystroke/        # 키보드 입력 처리
├── java/             # Java 연동 관련 코드
└── main.rs           # 메인 애플리케이션 진입점
```

## 설치 및 실행

1. Rust 개발 환경 설정

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. 프로젝트 빌드

```bash
cargo build --release
```

3. 실행

```bash
cargo run --release
```

## 기여 방법

1. 이 저장소를 포크합니다.
2. 새로운 기능 브랜치를 생성합니다 (`git checkout -b feature/amazing-feature`).
3. 변경사항을 커밋합니다 (`git commit -m 'Add some amazing feature'`).
4. 브랜치에 푸시합니다 (`git push origin feature/amazing-feature`).
5. Pull Request를 생성합니다.

## 라이선스

이 프로젝트는 MIT 라이선스 하에 배포됩니다. 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.

## 연락처

프로젝트 관리자 - [@your-username](https://github.com/your-username)

프로젝트 링크: [https://github.com/your-username/unin](https://github.com/your-username/unin)
