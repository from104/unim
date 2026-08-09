# UNIM Windows 나머지 개선점 — 세션 종합 구현 보고서

- 작성일: 2026-07-04
- 브랜치: `feat/windows-msi-redesign`
- 세션 범위: `aad728c..HEAD` (릴리스 커밋 `5a8a066` v0.3.55 포함)
- 최종 코어 테스트: `cargo test -p unim` → **702 passed / 0 failed / 0 ignored** (60.56s), до크테스트 19 passed / 1 ignored. **그린**

## 종합 요약

| 구분 | 수 |
|------|----|
| 랜딩(success/committed) | **13** |
| 리버트 | **0** |
| 스킵 | **0** |
| device-QA 대기 | **13** (전 아이템) |

이번 세션 13개 구현 아이템 전부 랜딩. 리버트·스킵 없음. 모든 아이템이 device-QA(실기기 육안/실측)를 후속으로 남김 — Windows 크레이트는 빌드 검증만 가능하고 실입력 동작은 사용자 몫이기 때문. 릴리스 커밋 `5a8a066`으로 버전 0.3.55 범프 완료.

## 아이템별 결과

| # | 아이템 | 우선 | 상태 | 커밋 | verify | device-QA |
|---|--------|------|------|------|--------|-----------|
| R1 | ATF `replace_composition` 코어 배선 (word모드 SetText) | P1 | success | `93e38f5` | (build) | 필요 |
| R2 | 조합키 자동반복 억제 옵션 `ignore_key_repeat` (지체장애) | P1 | committed | `c9f6b5d` | ok | 필요 |
| R3 | 고정키 + 수정자 토글(RightAlt) 충돌 해소 (지체장애) | P1 | committed | `5fd1873` | ok | 필요 |
| R4 | 폴백앱 ATF 무고지 해소 (앱당 1회 고지) | P1 | committed | `4c0c4ad` | ok | 필요 |
| G1 | 오타교정 탭 정보구조 2계층 + 강도 3프리셋 + 기본값복원/undo | P1 | success | `700c11c` | (build) | 필요 |
| G2 | 설정 접근성 섹션 + 순아래 승격 배지 + 모아치기 카드 | P0 | committed | `36dd75d` | ok | 필요 |
| G3 | Slint 설정 GUI i18n(ko/en) OS locale 추종 | P1 | success | `df2527c` | (build) | 필요 |
| G4 | 설정 마감: 단일인스턴스 + 검색 + 슬라이더 + 창아이콘 | P2 | committed | `9d3d44d` | ok | 필요 |
| P1 | 팝업 Win11 라운드코너 + DWM 드롭섀도 | P2 | committed | `9bbe43a` | concerns | 필요 |
| P2 | D2D 이모지 폰트 DPI 재생성 (혼합 DPI) | P2 | committed | `671ae9b` | ok | 필요 |
| P3 | 앱 능력 티어 캐시 영속화 `app_tiers.json` | P2 | committed | `62425c4` | concerns | 필요 |
| DOC | 후속 설계 사양서 (범용 사용자사전/상용구 + classify_key 통합) | P1 | committed | `a8e471c` | ok | 문서 |

> verify 열의 `(build)`는 별도 verify 게이트 없이 코어 배선·GUI 변경으로 빌드 그린만 확인된 항목. `concerns`(P1/P3)는 아래 followup에 별도 실측 조건 명시.

## 아이템별 landing 메모 및 followup

### R1 ATF `replace_composition` 코어 배선 — `93e38f5`
word 모드 앱(Word 등)에서 라이브 조합을 삭제 없이 SetText로 치환하는 코어 신호 준비. Windows 소비 배선 포함.
- device-QA: Word 등 word모드 앱에서 순방향(영문 라이브 조합→한글 SetText 치환)·역방향(한글 committed=0 라이브 조합→영문 SetText 치환) 실측. 확정문 삭제 없이 조합만 치환되는지, synth 강등 없이 동작하는지 육안 확인.
- device-QA: 음절 모드/메모장 등 비협조앱에서 기존 `replace_surrounding` 경로가 바이트 동일 유지(회귀 없음) 확인.
- Linux: `unim-dbus`의 `buf.word_mode` 배선은 Windows 빌드 불가로 인스펙션만. Linux 환경 `cargo build -p unim-dbus` 컴파일 그린 확인 필요(필드 미참조라 동작 무영향).
- 향후 Linux 프런트(GTK/Qt/Wayland)에서 `replace_composition` 실제 소비해 word모드 치환 구현 여부 검토(현재 코어 신호만 준비, Linux 미소비).

### R2 조합키 자동반복 억제 — `c9f6b5d`
- device-QA: 옵션 ON/OFF로 조합 중 자모키·한영 토글키 홀드 시 연타/진동 억제 확인, 백스페이스/방향키·영문 홀드는 정상 반복 유지 확인.
- Linux 프런트(GTK/Qt/XIM/Wayland)는 config 필드만 노출, 실제 억제 로직은 TSF 전용 — Linux 필요 시 별도 구현 과제.
- unim-settings GTK SwitchRow 인스펙션만 — Linux 빌드 환경 컴파일 검증 필요.

### R3 고정키 + 수정자 토글 충돌 해소 — `5fd1873`
- device-QA: 고정키(Sticky Keys) ON + RightAlt 토글 직후 첫 자모 정상 조합 실측(메모장/워드/크롬).
- unim-imm32도 `press_key` 앞 동일 단축키 게이트(`control||alt||super`)를 가지므로 카톡/한컴 등 IMM32 앱에서 살리려면 동일 `peek_sticky_masked_modifiers` 정렬 필요 — 이번 스코프(TSF) 제외, 빌드/검증 불가라 미적용.
- 현재 무설정 상시 동작(수정자 토글 직후 1키 한정, 자기제한적). 필요 시 접근성 옵션 config 플래그 게이트화 검토 — settings 6지점 동기화 동반.

### R4 폴백앱 ATF 무고지 해소 — `4c0c4ad`
- 순방향 ATF 부활: 오버레이가 append-only 삽입만 하므로 삭제 없는 SetText 치환으로 폴백 앱 순방향(영→한) 교정 되살릴 여지 — PoC 필요.
- device-QA: wmux/xterm.js 등 CUAS 폴백 앱 한글 입력 시 트레이 랭바 툴팁에 '이 앱은 자동교정 제한' 표시, 정상 앱(메모장) 복귀 시 사라짐 육안 확인.
- 스크린리더(NVDA/Narrator): 폴백 진입 시 `NotifyWinEvent(NAMECHANGE)` 낭독 여부 확인.

### G1 오타교정 탭 정보구조 2계층 + 3프리셋 + 복원/undo — `700c11c`
- 프리셋 버튼에 현재 활성 프리셋 하이라이트(config 역매칭) 추가 시 적용 강도 시각화 가능 — 현재는 status-text 피드백만.
- '기본값으로 복원'은 `auto_typefix` 범위 한정. 일반 탭 포함 전역 복원 필요 시 별도 설계.

### G2 접근성 섹션 + 순아래 승격 + 모아치기 카드 — `36dd75d` (P0)
- 모아치기 카드는 `supports_moachigi` 자판(`ko_3bul_anmatae` 등)에서만 노출. anmatae는 `KOREAN_LAYOUT_BUILTINS`에 없어 GUI에서 직접 선택 불가 → 현재 config로 이미 쓰는 사용자만 카드 노출. 신규 사용자 GUI 접근하려면 anmatae를 선택 목록에 추가하는 별도 작업 필요(anmatae-moachigi-rollout 스킬 범위).
- '넉넉한 타이밍' 프리셋이 지원 자판에서 모아치기(`chord_window_ms=Some(150)`)를 켜는 부수효과. 의도적(카드 설명 명시)이나 사용자 검증에서 놀람 요소인지 확인 권장.
- device-QA: 순아래 선택 시 배지 표시, 접근성 프리셋 2종 클릭 후 관련 컨트롤(자판 인덱스·토글키·자동반복·모아치기) 즉시 반영, 슬라이더 드래그 중 값 실시간 표시 + released 시 저장, 스크린리더 접근성 라벨 낭독.

### G3 Slint GUI i18n(ko/en) — `df2527c`
- 코어(`src/config.rs`) 표시명은 여전히 한국어 전용: `korean_layout_display_name`/`english_layout_display_name` 및 `CommitUnit::display_name`('음절/단어/스마트')이 `&'static str` 한국어 반환이라 영어 로케일에서도 한글 노출. GUI 범위 밖 — core 변경 필요한 별도 작업.
- device-QA: (1) 영어 로케일 Windows에서 GUI 전체 영어 표시, (2) 한국어 로케일 회귀 없이 한글 유지, (3) 상태바/토스트/프리셋 메시지 언어 일치 육안 확인.

### G4 설정 마감: 단일인스턴스·검색·슬라이더·창아이콘 — `9d3d44d`
- device-QA: 설정 창 두 번 실행 시 기존 창 전면화(최소화 상태 복원 포함).
- device-QA: 검색어 입력 시 페이지 내 행 필터링·빈 카드 제목 잔존 육안 확인, 크로스 페이지 검색 필요성 재검토.
- device-QA: 6개 슬라이더 드래그/키보드 조작·값 라벨 실시간 갱신·released 저장.
- device-QA: 작업표시줄/타이틀바 아이콘 표시(투명 배경 PNG 렌더).

### P1 팝업 Win11 라운드코너 + DWM 섀도 — `9bbe43a` (verify: concerns)
- device-QA: Win11 실기기에서 라운드 코너·그림자 육안 확인. 그림자 약하면 `DwmExtendFrameIntoClientArea` margins 조정 또는 sheet-of-glass(-1) 검토.
- 레이어드(alpha 255)+DWM 확장 프레임 상호작용으로 가장자리 1px 유리 노출 여부 실측(현재 불투명 도색이 덮어 미노출 예상).

### P2 D2D 이모지 폰트 DPI 재생성 — `671ae9b`
- device-QA: 혼합 DPI 멀티모니터에서 이모지 팝업을 서로 다른 배율(100%↔150%) 모니터로 옮겨 글리프 크기 즉시 갱신 육안 확인.

### P3 앱 능력 티어 캐시 영속화 — `62425c4` (verify: concerns)
- device-QA: wezterm/wmux 등 CUAS 앱에서 한 단어 학습 후 앱·세션 재기동 → 재기동 직후 첫 단어부터 오버레이 폴백 즉시 적용(첫 단어 실험대상 제거) 확인.
- `%APPDATA%\unim\app_tiers.json` 생성·정렬 저장, 2초 throttle 디스크 쓰기 조임 로그(`dbg_log app_tiers:`) 확인.
- 향후 word-only/ShiftStart-부족 등 추가 티어 필요 시 `AppTiersFile` 필드 추가로 확장(스키마 version 범프).

### DOC 후속 설계 사양서 — `a8e471c`
산출물: `docs/dev/windows/FOLLOWUP-SPECS.md` (329줄 신규). 아래 "후속 설계" 참조.

## device-QA 대기 체크리스트 (사용자 몫)

Windows 크레이트는 빌드 검증만 가능. 아래 항목은 실기기 실측/육안 확인 필요.

- [ ] **R1 — Word/word모드 ATF**: word모드 앱 순방향(영→한 SetText 치환)·역방향(한 committed=0→영 치환), 확정문 삭제 없음·synth 강등 없음. 음절모드/메모장 replace_surrounding 회귀 없음.
- [ ] **R2 — 키홀드 반복억제**: 옵션 ON/OFF로 자모키·토글키 홀드 억제, 백스페이스/방향키·영문 홀드 정상 반복 유지.
- [ ] **R3 — 고정키 + RightAlt**: Sticky Keys ON + RightAlt 토글 직후 첫 자모 정상 조합(메모장/워드/크롬).
- [ ] **R4 — 폴백앱 고지**: CUAS 폴백 앱(wmux/xterm.js) 진입 시 트레이 툴팁 '자동교정 제한' 표시, 정상 앱 복귀 시 소멸. 스크린리더 NAMECHANGE 낭독.
- [ ] **P1 — Win11 라운드코너/섀도**: 실기기 라운드 코너·드롭섀도 육안, 가장자리 1px 유리 노출 여부.
- [ ] **P2 — 혼합 DPI 이모지**: 서로 다른 배율 모니터 이동 시 이모지 글리프 크기 즉시 갱신.
- [ ] **P3 — 앱티어 재기동**: CUAS 앱 학습 후 재기동 직후 첫 단어부터 폴백 즉시 적용, `app_tiers.json` 저장·throttle 로그 확인.
- [ ] **설정 GUI 육안**: G1 프리셋 3종/기본값복원·undo, G2 접근성 섹션·순아래 배지·모아치기 카드·프리셋 2종, G3 ko/en 로케일 추종, G4 단일인스턴스·검색·6슬라이더·창아이콘.

## 후속 설계 (승인 대기)

산출: `docs/dev/windows/FOLLOWUP-SPECS.md`

- **범용 사용자사전/상용구**: 사용자 게이트 G1~G5 승인 후 (A) 스키마+코어+CLI 착수.
- **classify_key 통합**: B-P1(classify_key 순수 리팩터) 구현 후 device-QA 매트릭스 1회전. B-P2 `deferred_commit`(WM_UNIM_COMMIT) 구현 — CUAS 순서 역전 실측 필수.
- Linux GTK GUI 탭은 별도 PR(검증 환경 분리).

## 재확인 제외 항목 (이번 세션 스코프 밖, 명시적 보류)

- 옛한글(고어 자모) 입력
- 한자 단어 변환
- 서명/온보딩 플로우
- 자동 업데이트

## 검증 근거

- `git log --oneline aad728c..HEAD`: 13개 구현 커밋 + 릴리스 커밋 `5a8a066` 확인, 리버트·스킵 없음.
- `cargo test -p unim`: 702 passed / 0 failed. 코어 배선(R1/G1/G3 등)이 코어 회귀 없음 확인.
- Windows 크레이트(unim-tsf/unim-tsf-settings/unim-popup-win) 및 GTK settings는 각 아이템 커밋 시점 빌드 그린(개별 verify) 또는 인스펙션. 실입력 동작은 위 device-QA 체크리스트로 위임.
