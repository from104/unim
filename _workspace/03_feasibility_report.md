# 자동 실시간 한영 오타 수정 — 종합 가능성 리포트

> 작성일: 2026-04-05
> 상태: **사용자 승인 완료**

## 승인된 범위

- **양방향 동시 구현** (영어모드→한글 + 한글모드→영문)
- **즉시 자동 교정** (단어 경계 시 자동 교체, Ctrl+Z 되돌리기)
- **방안 B: DBus 데몬 감지** (Core 순수성 유지)
- **감지 시점: 단어 경계** (공백/구두점/Enter)

## 필요 컴포넌트

### 1. 영어 사전 (한글모드 영문오타 감지용)
- 고빈도 영어 단어 ~50,000개 임베드 (include_str!)
- `src/data/english_words.txt` 또는 `src/auto_typefix.rs` 내 HashSet

### 2. auto_typefix 모듈 (`src/auto_typefix.rs`)
- `auto_detect_and_correct()` — 양방향 자동 감지+교정
- 방향 A: 영어모드에서 한글 자모 패턴 → eng_to_kor() (사전 불필요)
- 방향 B: 한글모드에서 영문 → kor_to_eng() → 영어 사전 매칭

### 3. engine_worker 후처리
- Space/구두점 커밋 후 auto_detect_and_correct() 호출
- 결과 있으면 delete_surrounding_text + commit_text 시그널

### 4. config 설정
- `auto_typefix_enabled: bool` (기본: true)
