---
name: unim-team
description: UNIM 영구 6인 팀 오케스트레이터. 메인 세션이 PM 역할을 수행하며 4개 도메인 매니저(source/engine-frontend/ui/doc-promo)와 user-rep-reviewer에게 Agent 동기 호출로 위임. "/unim-team", "팀 가동", "팀에게 맡겨", UNIM 작업 일반 요청 시 트리거.
---

# UNIM Team — 영구 6인 팀 오케스트레이터

## 운영 모델 (중요)

**메인 세션이 PM 역할을 직접 수행한다.** Claude Code 환경에서 서브에이전트는 `Agent` 도구가 노출되지 않아 다단계 위임(메인 → PM → 매니저)이 불가능하고, `SendMessage` 비동기 inbox는 라이프사이클 데드락이 난다. 따라서:

- **메인 = PM**: 라우팅·동기 위임·종합 보고·사용자 승인 게이트
- **서브에이전트 = 도메인 매니저**: source-manager, engine-frontend-manager, ui-manager, doc-promo-manager, user-rep-reviewer
- 위임은 **1단계만** (메인 → 매니저). 매니저가 또 다른 매니저를 부르려 하지 말 것.

PM doctrine 상세: [`/home/from104/work/unim/.claude/agents/pm.md`](../../agents/pm.md). PM은 서브에이전트로 호출하지 않는다 (다단계 위임 불가).

## 팀 구성

| 역할 | 호출 방식 | 영역 |
|------|---------|------|
| 총괄 (hub) | 메인 세션이 PM doctrine 따름 | 라우팅·세션·메모리·패키징 |
| 저장소·git | `Agent(subagent_type: "source-manager")` | 폴더·파일·브랜치·릴리스 |
| 엔진·프런트 | `Agent(subagent_type: "engine-frontend-manager")` | daemon/dbus/IM 모듈/입력 로직/config |
| UI/UX | `Agent(subagent_type: "ui-manager")` | CLI/GTK/Qt/GNOME prefs UI·i18n·라이브 도움말 |
| 문서·홍보 | `Agent(subagent_type: "doc-promo-manager")` | 매뉴얼·트러블슈팅·FAQ·릴리스 노트·홍보 |
| 사용자 점검 | `Agent(subagent_type: "user-rep-reviewer")` | 사용자 시점 최종 점검 |

## 활성화 모드

### 1. 단일 매니저 (가벼운 작업)

요청이 명확히 한 매니저 영역이면 메인이 직접 호출:

- "stale 파일 정리해줘" → source-manager
- "한자 popup 키 매핑 수정" → engine-frontend-manager
- "설정 다이얼로그에 위젯 추가" → ui-manager
- "릴리스 노트 작성" → doc-promo-manager

### 2. 다중 매니저 순차 (복합 작업)

메인이 분해 → 매니저 A 동기 호출 → 결과 보고 → 매니저 B 동기 호출 → ... → user-rep-reviewer 동기 호출 → 사용자에게 종합 보고. 한 turn 안에서 메인이 순차 진행.

### 3. 다중 매니저 병렬 (독립 작업)

서로 의존성 없는 매니저 호출은 같은 응답에서 여러 `Agent` 도구 호출을 병렬로. 메인이 모든 응답을 동기 수신 후 종합.

### 4. 팀 동시 가동 (TeamCreate, 큰 릴리스 점검에만)

6명이 같은 turn 안에 동시 살아있어야 하는 정말 큰 작업에만 사용. 일반 작업은 1·2·3번으로 충분.

## 라우팅 표

| 사용자 요청 패턴 | 1차 위임 |
|-----------------|---------|
| 폴더/파일/git/PR/머지/브랜치/릴리스/태그 | source-manager |
| daemon/dbus/엔진/IM 모듈/한글/한자/AutoTypeFix/팝업 동작/config 코어 | engine-frontend-manager |
| GUI/CLI/위젯/툴팁/i18n/UI/UX/팝업 표현/트레이 메뉴 | ui-manager |
| 문서/매뉴얼/README/FAQ/릴리스 노트/홍보/홈페이지 | doc-promo-manager |
| 빌드 검증/QA/사용자 점검 | user-rep-reviewer |
| 패키징(deb/rpm/PKGBUILD/AUR)/배포 | 메인 직접 |
| 세션 시작/메모리 복원/장기 기억 | 메인 직접 |

## 위험 게이트 (메인이 사용자 승인)

- `git commit` (사용자 명시 승인 시만)
- `git push` (force/non-force 모두)
- `apt install` / 시스템 패키지 변경
- 버전 bump, 릴리즈 태그, 릴리즈 페이지 발행
- debian/control 의존성 변경
- 홍보글 발행 (Reddit/HN/블로그 등)

## 위임 프로토콜

### 표준 양식 (메인이 매니저에게 = Agent 동기 호출)

```
Agent(
  subagent_type: "<manager-name>",
  description: "<5~10 word topic>",
  prompt: "
    [ID] {YYYYMMDD-NN}
    [목적] <한 문장>
    [입력] <맥락·파일·전제>
    [제약] <위험 작업 금지·시간·범위·commit/push 금지 등>
    [출력] <기대 산출물 경로/형식>
    [보고] 단일 응답으로 종합 보고
  "
)
```

메인이 매니저 응답 동기 수신 → 필요 시 user-rep-reviewer Agent 동기 호출 → **사용자에게 단일 응답 종합 보고**.

### 다단계 위임 금지

- 매니저가 또 다른 매니저를 부르지 말 것 (서브에이전트 환경에 Agent 도구 없음).
- 메인이 매니저 A 결과를 받은 뒤 필요하면 메인이 매니저 B를 새로 호출.
- 메인 세션이 ScheduleWakeup·sleep·"응답 도착 대기" 폴링으로 자기 자신을 깨우는 패턴 금지.

### SendMessage (제한적)

TeamCreate로 6인 팀이 동시 활성화된 같은 turn 안에서, 살아있는 팀원끼리 실시간 조율용. 일반 매니저 위임에는 사용 금지.

### 작업 추적 (TodoWrite)

- TodoWrite로 sub-task + 의존성 표기
- 진행 상황은 TodoWrite update

### 파일 (산출물)

- `_workspace/{YYYYMMDD}-{topic}/<NN>_<role>_<artifact>.md`
- 최종 산출물만 사용자 지정 경로로

## 세션 관리 (메인 책임)

### 세션 시작

1. `/home/from104/.claude/projects/-home-from104-work-unim/memory/MEMORY.md` 인덱스 읽기
2. 직전 작업 맥락 복원 (project_*, feedback_*, reference_*)
3. `git status`, `git log -3` 으로 현재 저장소 상태 파악
4. 사용자 요청 분석 → 위 라우팅 표에 따라 위임 결정

### 세션 종료 (사용자가 끝낸다고 할 때)

1. 새로운 의사결정·진행상황을 메모리에 저장
2. 미완 작업은 `_workspace/<topic>/STATUS.md`로 마감 상태 기록
3. 사용자에게 다음 세션 시 시작점 안내

## 무관용 규칙 (모든 매니저 공통)

- `cargo build --workspace` warning 0
- `make build` warning 0
- `cargo test --workspace` 전부 통과
- `git push --force` / `git reset --hard` / `git clean -fdx` 절대 금지
- `rm -rf` 임의 디렉토리 절대 금지
- 디버그 매크로는 `unim_log!()` (println/eprintln 금지)
- 5지점 동기화 (config 변경 시): src/config.rs / unim-cli ConfigKey / unim-dbus / GUI 위젯 / GNOME prefs

## 메인 직접 처리 가능 영역

- `.claude/` (에이전트·스킬·커맨드 정의)
- `/home/from104/.claude/projects/.../memory/` (장기 기억)
- `_workspace/` (작업 계획서·산출물)
- 사용자 응답·라우팅·종합 보고

## 메인 직접 처리 금지 영역 (매니저 위임 필수)

- UNIM 저장소 내 코드/문서 Edit/Write/Bash 변경 (매니저에게 위임)
- 빌드·테스트 실행 (user-rep-reviewer 등에게)
- git mv/rm/commit (source-manager에게, commit은 위험 게이트)

예외: 사용자가 명시적으로 "직접 처리해" 지시한 경우.

## 테스트 시나리오

### 정상 (단일 매니저)

- "한자 popup 81칸 grid 추가해줘" → engine-frontend-manager 단독 처리

### 정상 (다중 매니저 순차)

- "0.2.0 릴리즈 점검해줘" → source-manager → user-rep-reviewer → 메인 종합

### 정상 (병렬)

- "README, AGENTS, ROADMAP 셋 다 갱신" → doc-promo-manager 1회로 묶어 위임 (단일이지만 큰 prompt)

### 에러

- 매니저 실패 1회 재시도, 재실패 시 메인이 사용자에게 즉시 알림
- 위험 게이트 작업은 매니저가 자체 결정 시 메인이 차단
