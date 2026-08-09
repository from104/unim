# UNIM 문서 색인

이 저장소의 문서는 **독자**를 기준으로 나뉜다. 찾는 것이 어느 칸에 있는지부터 고르면 된다.

| 대상 | 위치 |
|---|---|
| UNIM 을 쓰는 사람 | [`user/`](user/) |
| UNIM 을 고치는 사람 | [`dev/`](dev/) |
| 근거 자료를 찾는 사람 | [`references/`](references/) |
| 지난 결정의 맥락을 찾는 사람 | [`archive/`](archive/) |

---

## `user/` — 사용자 문서

도움말 4종은 모두 `<주제>/README.md`(영어) + `<주제>/README-ko.md`(한국어) 한 패턴을 따른다.

| 주제 | 영어 | 한국어 |
|---|---|---|
| 사용자 매뉴얼 | [user-guide/README.md](user/user-guide/README.md) | [README-ko.md](user/user-guide/README-ko.md) |
| FAQ | [faq/README.md](user/faq/README.md) | [README-ko.md](user/faq/README-ko.md) |
| 문제 해결 | [troubleshooting/README.md](user/troubleshooting/README.md) | [README-ko.md](user/troubleshooting/README-ko.md) |
| 키보드 단축키 | [keyboard-shortcuts/README.md](user/keyboard-shortcuts/README.md) | [README-ko.md](user/keyboard-shortcuts/README-ko.md) |

이 4종은 향후 앱 내장 도움말 HTML 의 입력이 된다. **경로와 파일명을 바꾸면 생성기가 깨진다.**

그 밖에:

- [`user/release-notes/`](user/release-notes/) — 버전별 릴리스 노트 (0.2.0, 0.3.0, 0.4.0)
- [`user/keymaps/`](user/keymaps/) — 자판별 사용 안내 (안마태)
- [`user/UNIM-Windows-사용안내.md`](user/UNIM-Windows-사용안내.md) — Windows 판 안내 (한국어)

## `man/` — man page

`unim`, `unim-cli`, `unim-settings` 등 8개. **이동 금지** — `Makefile`·`PKGBUILD`·`debian/` 이 이 경로를 그대로 설치 대상으로 참조한다.

## `dev/` — 개발자 문서

| 하위 | 내용 |
|---|---|
| [`dev/architecture/`](dev/architecture/) | [AGENTS.md](dev/architecture/AGENTS.md) (작업 규칙) · [IME_BEHAVIOR.md](dev/architecture/IME_BEHAVIOR.md) · [LAYOUT_PROFILE_V3.md](dev/architecture/LAYOUT_PROFILE_V3.md) (현행 자판 스키마) · [dbus-popup-migration-plan.md](dev/architecture/dbus-popup-migration-plan.md) (popup DBus 책임 이관 — **미착수**, 10항목) |
| [`dev/specs/`](dev/specs/) | [POPUP_SPEC.md](dev/specs/POPUP_SPEC.md) — 팝업 동작 명세. **변경 시 사용자 승인 필수** |
| [`dev/windows/`](dev/windows/) | Windows TSF/IMM32 포팅. 활성 31 + [`_archive/`](dev/windows/_archive/) 31. 현재 지식 상태는 [_KNOWLEDGE_STATE.md](dev/windows/_KNOWLEDGE_STATE.md) 부터 |
| [`dev/linux/`](dev/linux/) | Linux 프런트엔드 개별 이슈 |
| [`dev/release/`](dev/release/) | [RELEASE.md](dev/release/RELEASE.md) — 릴리스 절차 |

## `references/` — 참고 자료

- [`references/keymaps/`](references/keymaps/) — 자판 정의 JSON 11종 + [USER_GUIDE.md](references/keymaps/USER_GUIDE.md). **이동 금지** (`Makefile`·`PKGBUILD` 참조)
- [`references/java/`](references/java/) — 한글 조합 규칙의 Java 원본 구현. Rust 포팅의 대조군
- [`references/research/`](references/research/) — 자판·입력기 조사 (안마태, 복벌식·갈마들이, 순아래받침, Wayland IM, Windows TSF, 샌드박스) + **메이저 IME 갭 분석** 2건 ([Windows](references/research/2026-07-03-windows-major-ime-analysis.md) · [Linux](references/research/2026-07-06-linux-major-ime-analysis.md)) — 제품화 관점 미비점과 백로그의 근거

## `archive/` — 보존

현행이 아니지만 결정의 근거로 남겨둔 문서.

- [`archive/plans/`](archive/plans/) — 완결된 계획.
  - LAYOUT_PROFILE V1 / V1_IMPL / V2 — 현행 스키마는 [dev/architecture/LAYOUT_PROFILE_V3.md](dev/architecture/LAYOUT_PROFILE_V3.md)
  - 팝업 재설계 3종 (사전 리서치 → Phase 계획 → 프로세스 아키텍처) — **Phase 7(팝업 중앙화)로 완결**. 현행 명세는 [dev/specs/POPUP_SPEC.md](dev/specs/POPUP_SPEC.md)
- [`archive/reports/`](archive/reports/) — 특정 시점의 세션 작업 보고. Windows 구현 검증(2026-07-03) · 나머지 개선점 구현(2026-07-04)

---

## 정리 대기

| 경로 | 겹치는 곳 |
|---|---|
| [`branding/`](branding/) | 로고 인상 기록 1건 |
