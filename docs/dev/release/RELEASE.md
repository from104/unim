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
- [ ] `CHANGELOG` 해당 버전 절에 `### 알려진 문제` / `### Known issues` 가 있다
      (릴리스 본문 맨 위로 올라가는 절이다 — 남은 제약이 없으면 절 자체를 뺀다)
- [ ] `scripts/release-body.sh vX.Y.Z` 가 오류 없이 본문을 뽑는다
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

# rpm/unim.spec — Version: X.Y.Z + %changelog 새 항목(날짜 요일 일치 확인)
# PKGBUILD — pkgver=X.Y.Z (source=()/sha256sums=() 도 버전에 맞춰 확인)
# unim-imm32/unim_imm32.rc — FILEVERSION/PRODUCTVERSION(콤마 표기) + 문자열 버전 4곳
# unim-gnome-extension/metadata.json — version(정수 아님, 문자열)

# WiX GUID/버전 재생성 — 빠뜨리면 windows-msi 의 check-wxi-guids 가 실패한다
make wxi-guids

# 도움말 HTML 재생성 — 상단 배지에 버전이 박히므로 범프 즉시 stale 이 된다.
# 빠뜨리면 linux-ci 의 check-help-html 이 실패한다
make help-html

# man 8종 .TH 버전 — Makefile/PKGBUILD/debian/ 이 경로를 그대로 참조하므로 이동 금지
grep -l "^\.TH" docs/man/*.1

# 문서 안의 버전 예시(install.sh·README·유저 가이드)도 함께 확인
grep -rn "X\.Y\.Z" README.md install.sh install.ps1 docs/user/ .github/workflows/
```

> 위 산출물들(`installer/wix/generated/guids.wxi`, `help/unim-help-*.html`)은
> **생성물이지만 저장소에 커밋한다.** 재생성 후 커밋에 포함시킬 것.

---

## 2. 패키지 빌드 검증

### 2.1 Debian 패키지

```bash
make deb
# 결과: build/deb/*.deb
# dpkg --contents 로 설치 경로 확인
```

현재 11개 deb 패키지(`debian/control` 기준):
- `unim-common` — 데몬 + 공통 데이터(자판 프로필, 한자 데이터)
- `unim-im-gtk` — GTK3/4 IM 모듈
- `unim-im-qt` — Qt5/6 IM 플러그인
- `unim-xim` — XIM 프런트엔드
- `unim-wayland` — 순수 Wayland 프런트엔드
- `unim-desktop` — 트레이 인디케이터 + popup-service + 레거시 GTK 설정 창(`unim-settings-gtk`)
- `unim-settings` — Slint 기반 정식 설정 앱
- `unim-keymap-studio` — 자판 스튜디오
- `unim-typing-practice` — 타자 연습
- `unim-gnome` — GNOME Shell 확장(`unim-gnome-extension`)
- `unim` — 메타패키지(전체 의존)

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

### 2.4 Windows MSI (VM QA — 실기/VM 필요, 에이전트 수행 불가)

전체 절차는 [`docs/dev/windows/SMOKE_TEST.md`](../windows/SMOKE_TEST.md).

- [ ] 4.9 NVDA/내레이터로 한/영 전환 통지가 실제로 읽히는지 1회 확인 (A11Y-03)

---

## 3. 문서 최종 확인

- [ ] `README.md` — 버전 배지(`현재 X.Y.Z`), 주요 신기능 표 갱신
      (최상위 README 는 한국어 1벌이다. 영문판은 `help/unim-help-en.html` 이 담당한다)
- [ ] `docs/user/user-guide/` — 신기능 반영 완료
- [ ] `docs/user/troubleshooting/` — 신규 Known Issue 반영
- [ ] `docs/user/faq/` — 버전별 신규 Q&A 추가
- [ ] `docs/man/` man 8종 전부 `.TH` **버전과 날짜(월)** 업데이트 — `unim.1`, `unim-cli.1`, `unim-indicator.1`, `unim-settings.1`, `unim-settings-gtk.1`, `unim-keymap-studio.1`, `unim-typing-practice.1`, `unim-popup-service.1`
- [ ] **날짜 정합**: CHANGELOG 두 벌 · `debian/changelog` 의 `--` 줄 ·
      `rpm/unim.spec` 의 `%changelog` 날짜(요일 일치) · man `.TH` 가 모두 같은 릴리스 날짜
- [ ] 깨진 링크 0 — 아래 한 줄로 전수 검사(`%20` 인코딩과 `debian/` 빌드 산출물은 오탐)
      ```bash
      python3 - <<'PY'
      import os,re,urllib.parse
      pat=re.compile(r'\[[^\]]*\]\(([^)#][^)]*?)\)')
      bad=[]
      for root,_,fs in os.walk('.'):
          if any(x in root for x in ('/target','/.git','/node_modules','/build','/debian/tmp','/debian/unim-')): continue
          for f in fs:
              if not f.endswith('.md'): continue
              p=os.path.join(root,f)
              for m in pat.finditer(open(p,encoding='utf-8',errors='ignore').read()):
                  l=urllib.parse.unquote(m.group(1).split('#')[0].strip())
                  if not l or l.startswith(('http','mailto:','tel:')): continue
                  if not os.path.exists(os.path.normpath(os.path.join(root,l))): bad.append((p,l))
      print(len(bad)); [print(*b) for b in bad]
      PY
      ```
- [ ] 문서 한/영 짝 누락 0 (`docs/` 아래 `*.en.md`·`*-ko.md` 는 짝이 되는 `*.md` 가 있어야 한다)

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

**GitHub Release 자체는 태그 push 시 `linux-deb.yml`(`Create GitHub Release` 스텝)이 자동 생성한다.** deb 11종 + `SHA256SUMS`를 첨부하므로, **이 단계에서 수동으로 릴리스를 만들거나 본문을 붙여넣지 않는다.**

본문은 `scripts/release-body.sh <태그>` 가 **CHANGELOG 에서 직접 뽑는다.** 별도의 릴리스 노트 문서는 두지 않는다 — 원본이 하나여야 릴리스 페이지와 저장소의 이력이 어긋나지 않는다. 본문 구성은 다음과 같고, `알려진 문제` 절은 읽는 사람이 먼저 봐야 하므로 **맨 위로 끌어올린다.**

```
## [X.Y.Z] YYYY-MM-DD
### 알려진 문제 → ### 수정됨 / 추가됨 / 변경됨
<details>English (CHANGELOG.md 의 같은 절)</details>
---
설치 안내 (Linux · Windows)
**전체 변경 이력 / Full Changelog**: compare/<이전태그>...<태그>
```

compare 링크가 이전 태그를 찾아야 하므로 `linux-deb.yml` 의 checkout 은 `fetch-depth: 0` 이다. 얕게 받으면 태그가 없어 **오류 없이 그 줄만 빠진다** — 릴리스 후 본문에 그 줄이 있는지 눈으로 볼 것.

- [ ] 태그 push 후 Actions에서 `linux-deb.yml`의 릴리스 생성 성공 확인
- [ ] 릴리스 본문에 CHANGELOG 절과 `Full Changelog` compare 링크가 다 들어갔는지 확인
- [ ] (Windows MSI가 별도 워크플로에서 첨부되는 동안 시간차가 있을 수 있음 — 본문의 ⏳ 안내가 이를 고지한다)

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
