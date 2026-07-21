# UNIM 릴리스 절차

UNIM 정식 릴리스(`develop` → `main` 머지 + 태그 생성) 시 수행해야 하는 단계별 체크리스트.

> **주의**: develop → main 머지·릴리스 태그·CHANGELOG 동반 작업은
> **메인테이너(PM) + 사용자 명시 승인 없이 실행 금지**.

---

## 사전 조건

- [ ] `develop` 브랜치의 모든 PR이 머지 완료
- [ ] `cargo build --workspace` — 경고 0
- [ ] `make build` — 경고 0 (C/C++ 프론트엔드 포함)
- [ ] `cargo test --workspace` — 전체 통과
- [ ] `cargo clippy --workspace -- -D warnings` — 에러 0
- [ ] `CHANGELOG.md` / `CHANGELOG-ko.md` — `[Unreleased]` 항목이 해당 버전으로 확정
- [ ] `docs/user/release-notes/<version>/README.md` + `README.en.md` 존재
- [ ] 버전 번호 일관성 확인: `Cargo.toml` workspace version = CHANGELOG 최신 버전

---

## 1. 버전 범프

버전이 박히는 곳이 여러 군데다. **하나라도 빠지면 CI 가 막는다** — 아래를 한 번에 처리한다.

```bash
# workspace Cargo.toml
version = "X.Y.Z"

# CHANGELOG.md: [Unreleased] → [X.Y.Z] - YYYY-MM-DD
# CHANGELOG-ko.md: 동일 처리

# debian/changelog 새 엔트리 (X.Y.Z-1)

# WiX GUID/버전 재생성 — 빠뜨리면 windows-msi 의 check-wxi-guids 가 실패한다
make wxi-guids

# 도움말 HTML 재생성 — 상단 배지에 버전이 박히므로 범프 즉시 stale 이 된다.
# 빠뜨리면 linux-ci 의 check-help-html 이 실패한다
make help-html

# 문서 안의 버전 예시(install.sh·README·유저 가이드)도 함께 확인
grep -rn "X\.Y\.Z" README.md install.sh docs/user/ .github/workflows/
```

> 위 두 `make` 산출물(`installer/wix/generated/guids.wxi`, `help/unim-help-*.html`)은
> **생성물이지만 저장소에 커밋한다.** 재생성 후 커밋에 포함시킬 것.

---

## 2. 패키지 빌드 검증

### 2.1 Debian 패키지

```bash
make deb
# 결과: build/deb/*.deb
# dpkg --contents 로 설치 경로 확인
```

주요 패키지:
- `unim-daemon` — 데몬 바이너리 + DBus 서비스 파일
- `unim-popup-service` — popup-service 바이너리 + D-Bus activation 파일
- `unim-gui-gtk` — GTK4 indicator + 설정 다이얼로그
- `unim-cli` — CLI 도구
- `unim-frontend-gtk3`, `unim-frontend-gtk4` — IM 모듈
- `unim-frontend-qt5`, `unim-frontend-qt6` — Qt IM 플러그인
- `unim-common` — 공통 데이터 (자판 프로필, 한자 데이터)
- `unim-gnome-extension` — GNOME Shell 확장

### 2.2 RPM 패키지

```bash
make rpm
# 결과: build/rpm/RPMS/**/*.rpm
```

### 2.3 popup-service D-Bus 활성화 검증

```bash
# 설치 후
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/Popup
```

---

## 3. 문서 최종 확인

- [ ] `README.md` / `README-ko.md` — 버전 배지, 주요 신기능 표 갱신
- [ ] `docs/user/user-guide/` — 신기능 반영 완료
- [ ] `docs/user/troubleshooting/` — 신규 Known Issue 반영
- [ ] `docs/user/faq/` — 버전별 신규 Q&A 추가
- [ ] `docs/man/unim.1` / `unim-popup-service.1` — 버전 업데이트
- [ ] 깨진 링크 0: `grep -rEn '\]\(\.[^)]*\.md\)' docs/` 후 경로 존재 확인
- [ ] 한/영 짝 누락 0 (README.md + README-ko.md / README.en.md + README.md)

---

## 4. 6매니저 하네스 최종 게이트

6개 관심 영역 모두 sign-off:

| 매니저 | 확인 항목 |
|--------|----------|
| **engine-frontend-manager** | 기능 동작, 테스트, clippy 통과 |
| **ui-manager** | i18n 키 완결, GUI 동작, 도움말 텍스트 |
| **doc-promo-manager** | 문서 한/영 짝, 링크, 릴리즈 노트 |
| **source-manager** | CHANGELOG 형식, 패키지 파일, 빌드 스크립트 |
| **user-rep-reviewer** | UX 회귀 없음, 알려진 이슈 명시 |
| **pm** | 전체 종합 + 사용자 승인 게이트 |

---

## 5. Git 태그 + 머지

```bash
# develop → main
git checkout main
git merge --no-ff develop -m "release: vX.Y.Z"

# 태그
git tag -a vX.Y.Z -m "UNIM vX.Y.Z"
git push origin main --tags
```

---

## 6. GitHub Release 생성

- 제목: `UNIM vX.Y.Z — <한 줄 요약>`
- 본문: `docs/user/release-notes/<version>/README.en.md` 내용 붙여넣기
- 에셋: deb/rpm 패키지 파일 업로드

---

## 7. 홍보 발행 (선택, PM 승인 필수)

홍보 초안 위치: `docs/promo/<version>-*.md`

발행 채널:
- Reddit r/linux / r/linuxquestions
- Hacker News (Show HN)
- discuss.gnome.org
- 클리앙 / OKKY (한국어)

> 홍보글 발행은 PM 통과 + 사용자 명시 승인 필수.

---

## 릴리스 후 작업

- [ ] `develop` 브랜치에서 `[Unreleased]` 섹션 초기화 (CHANGELOG.md, CHANGELOG-ko.md)
- [ ] 버전 번호 다음 개발 버전으로 올리기 (예: `0.3.0` → `0.4.0-dev`)
- [ ] 작업 일지 작성: `~/obsidian/생각 모음/2 Projects/ATIT/unim/일지/`
