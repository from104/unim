---
name: unim-team
description: UNIM 영구 6인 팀 오케스트레이터. PM이 사용자 요청을 받아 4개 도메인 매니저(source/engine-frontend/ui/doc-promo)에게 위임하고 user-rep-reviewer가 사용자 시점 점검. "/unim-team", "팀 가동", "팀에게 맡겨", UNIM 작업 일반 요청 시 트리거. 단일 매니저 작업이 명확하면 직접 호출하고 팀 활성화는 생략.
---

# UNIM Team — 영구 6인 팀 오케스트레이터

## 팀 구성

| 역할 | 에이전트 | 영역 |
|------|---------|------|
| 총괄 (hub) | pm | 라우팅·세션·메모리·패키징 |
| 저장소·git | source-manager | 폴더·파일·브랜치·릴리스 |
| 엔진·프런트 | engine-frontend-manager | daemon/dbus/IM 모듈/입력 로직/config |
| UI/UX | ui-manager | CLI/GTK/Qt/GNOME prefs UI·i18n·라이브 도움말 |
| 문서·홍보 | doc-promo-manager | 매뉴얼·트러블슈팅·FAQ·릴리스 노트·홍보 |
| 사용자 점검 | user-rep-reviewer | 사용자 시점 최종 점검 |

## 활성화 모드

### 1. 단일 매니저 (가벼운 작업)
요청이 명확히 한 매니저 영역이면 PM이 직접 호출:
- "stale 파일 정리해줘" → source-manager
- "한자 popup 키 매핑 수정" → engine-frontend-manager
- "설정 다이얼로그에 위젯 추가" → ui-manager
- "릴리스 노트 작성" → doc-promo-manager

### 2. 팀 전체 가동 (큰 작업·릴리스 점검)
복합 요청 또는 사용자가 "팀에게 맡겨"·"전반 점검" 요구 시:
- TeamCreate로 6명 팀 활성화
- PM이 TaskCreate로 작업 분해
- 매니저들 SendMessage로 자체 조율
- user-rep-reviewer가 사용자 시점 종합 점검

## 라우팅 표

| 사용자 요청 패턴 | 1차 위임 |
|-----------------|---------|
| 폴더/파일/git/PR/머지/브랜치/릴리스/태그 | source-manager |
| daemon/dbus/엔진/IM 모듈/한글/한자/AutoTypeFix/팝업 동작/config 코어 | engine-frontend-manager |
| GUI/CLI/위젯/툴팁/i18n/UI/UX/팝업 표현/트레이 메뉴 | ui-manager |
| 문서/매뉴얼/README/FAQ/릴리스 노트/홍보/홈페이지 | doc-promo-manager |
| 빌드 검증/QA/사용자 점검 | user-rep-reviewer |
| 패키징(deb/rpm/PKGBUILD/AUR)/배포 | pm 직접 |
| 세션 시작/메모리 복원/장기 기억 | pm |

## 위험 게이트 (PM 직접 사용자 승인)
- `git commit` (사용자 명시 승인 시만)
- `git push` (force/non-force 모두)
- `apt install` / 시스템 패키지 변경
- 버전 bump, 릴리즈 태그, 릴리즈 페이지 발행
- debian/control 의존성 변경
- 홍보글 발행 (Reddit/HN/블로그 등)

## 데이터 전달 프로토콜

### 메시지 (실시간 조율)
```
SendMessage(to: "<manager>", message: "
[ID] {YYYYMMDD-NN}
[목적] ...
[입력] <맥락·파일·전제>
[제약] <위험 작업 금지·시간·범위>
[출력] <기대 산출물 경로/형식>
[보고] 완료 시 / 단계별
")
```

### 작업 (의존성 추적)
- TaskCreate로 sub-task + 의존성
- 진행 상황 TaskUpdate

### 파일 (산출물)
- `_workspace/{YYYYMMDD}-{topic}/<NN>_<role>_<artifact>.md`
- 최종 산출물만 사용자 지정 경로로

## 세션 관리 (PM 책임)

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
- 디버그 매크로는 `unim_log!()` (printlnln/eprintln 금지)
- 5지점 동기화 (config 변경 시): src/config.rs / unim-cli ConfigKey / unim-dbus / GUI 위젯 / GNOME prefs

## 단일 호출 양식 (메인이 PM 호출)
```
Agent(
  subagent_type: "general-purpose",
  model: "opus",
  description: "<topic>",
  prompt: "/home/from104/work/unim/.claude/agents/pm.md를 따라
           다음 사용자 요청을 처리하라:
           <REQUEST>
           
           라우팅 표에 따라 적절한 매니저에게 위임하고
           user-rep-reviewer 점검 후 종합 보고."
)
```

## 팀 호출 양식 (큰 작업)
```
TeamCreate("unim-team-{YYYYMMDD-NN}",
  members: [pm, source-manager, engine-frontend-manager,
            ui-manager, doc-promo-manager, user-rep-reviewer])
```
PM이 leader. 매니저들 SendMessage로 자체 조율.

## 테스트 시나리오

### 정상 (단일 매니저)
- "한자 popup 81칸 grid 추가해줘" → engine-frontend-manager 단독 처리
- 결과: ui-manager에 표현 작업 위임, doc-promo-manager에 명세 갱신 위임

### 정상 (팀 전체)
- "0.3.0 릴리즈 점검해줘" → 팀 활성화 → 각 매니저 병렬/순차 작업 → user-rep-reviewer 종합

### 에러
- 매니저 실패 1회 재시도, 재실패 시 PM이 사용자에게 즉시 알림
- 위험 게이트 작업은 매니저가 자체 결정 시 PM이 차단
