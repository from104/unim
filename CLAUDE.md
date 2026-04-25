# CLAUDE.md

> **This file is a redirect stub.**
> UNIM의 에이전트 컨텍스트는 [`AGENTS.md`](AGENTS.md)로 통합되었습니다.
> 과거 Claude Code 및 기타 에이전트가 본 파일을 참조하던 내용은 모두 아래 문서로 이관:

## Where the content moved

| 과거 `CLAUDE.md` 섹션 | 현재 위치 |
|------|------|
| Project Overview / Architecture / Key Source Locations | [`AGENTS.md` — 프로젝트 개요 · 컴포넌트 맵 · 아키텍처 흐름](AGENTS.md) |
| Build Commands | [`AGENTS.md` — 빌드 시스템](AGENTS.md) |
| Strict Quality Rules | [`AGENTS.md` — 품질 규칙](AGENTS.md) |
| Development Conventions | [`AGENTS.md` — 개발 규약](AGENTS.md) |
| Logging (매크로·포맷·모듈명) | [`GEMINI.md` — Logging System](GEMINI.md) |
| Settings Synchronization (5지점) | [`GEMINI.md` — 설정 항목 연동 가이드라인](GEMINI.md) |
| Debugging (`UNIM_DEVELOP=1`) | [`AGENTS.md` — 디버깅](AGENTS.md) |
| Reference Documents | [`AGENTS.md` — 참조 문서](AGENTS.md) |

## Quick entry points

- **신규 기여자**: [`AGENTS.md`](AGENTS.md) → [`GEMINI.md`](GEMINI.md) → [`IME_BEHAVIOR.md`](IME_BEHAVIOR.md) 순서 권장
- **IME 동작 스펙**: [`IME_BEHAVIOR.md`](IME_BEHAVIOR.md)
- **팝업 명세**: [`docs/specs/POPUP_SPEC.md`](docs/specs/POPUP_SPEC.md)
- **컴포넌트별 상세**: 각 크레이트의 `SPEC.md` — [`README.md`의 SPEC 인덱스](README.md) 참조

## Why the stub stays

`.claude/skills/`·`.claude/agents/` 다수가 `CLAUDE.md` 경로를 하드코딩으로 참조합니다.
링크 구조를 깨지 않기 위해 본 파일은 리디렉트 스텁으로 유지되며,
에이전트/스킬이 `AGENTS.md`로 단계적으로 옮겨갈 때 함께 제거됩니다.
