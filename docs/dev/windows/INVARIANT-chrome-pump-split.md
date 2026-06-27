# 불변식 — 크롬(Blink) 순방향 자동교정 "펌프-분할" (회귀 금지)

> ⚠️ **이 문서가 기술하는 구조를 "단순화"하려고 두 단계를 한 edit session 으로 합치지 말 것.**
> 그렇게 하면 크롬에서 앞 확정문이 사라지는 회귀가 재발한다. 실측으로 3회(방식 D→
> 펌프-분할→네이티브 통합) 반복 확인된 근본 제약이다. v0.3.38 에서 크롬 동작 확정.

## 증상 (회귀 시 재현)
영문 모드로 한글 자판 키시퀀스를 치면(예 `jbkfsuf`→"우간다", `ntkdmes`→"서기현")
AutoTypeFix 가 N음절을 교정한다. 기대 = **앞 음절들 확정 + 마지막 음절 미확정 조합**
("우간" 확정 + "다" 조합). 회귀 시 = **마지막 음절만 표시**("다"만, 앞 "우간" 유실).

## 근본 원인
크롬(Blink)의 TSF text store 는 **하나의 edit session(ec/lock) 안에서** "commit(확정) →
이어서 새 composition SetText" 를 하면, commit 을 문서에 정착시키기 **전에** 다음 SetText 가
그 자리를 덮어써 앞 확정문이 사라진다. 네이티브 한국어 타이핑이 멀쩡한 이유는 음절 전환
commit 이 **키와 키 사이(메시지 펌프가 도는 별도 edit session)** 에 일어나기 때문이다.
즉 **commit 과 그 뒤 composition 사이에 실제 메시지 펌프가 1회 이상 필요**하다.

- 같은 버그가 두 곳에 있었다:
  1. synth 폴백 `insert_pending`(ShiftStart 가 확정문 뒤로 역확장 거부 → SendInput BS 경로).
  2. **네이티브 `ReplaceSurroundingEditSession`**(ShiftStart `full=true` 성공 경로) — step3
     commit + step4 compose 를 한 ec 에 수행. ← 실측에서 이 경로가 주로 걸렸다.
- `sessionB alive=true 인데 화면엔 마지막 음절만` = lock 해제 경계만으로는 부족(펌프 필요)의
  결정적 증거였다.

## 불변식 (반드시 유지)
순방향 교정(commit_text 비어있지 않음 + 마지막 음절 preedit 있음)은 **두 단계로 분할**한다:

1. **Phase2a (세션1)** — 삭제(ShiftStart 또는 SendInput BS) 후, 교정문 **전체(commit+preedit)**
   를 **하나의 미확정 composition** 으로 만든다. **commit/EndComposition 하지 않는다.**
   `composition.rs`:
   - 네이티브: `ReplaceSurroundingEditSession::DoEditSession` 순방향 분기 (A) — full 조합 생성,
     `composition_slot` 적재. → `replace_surrounding` 이 `store_pending_restart(commit, preedit)`
     + `ReplaceOutcome::PhaseSplit` 반환.
   - synth: `insert_pending` case3 — `start_composition(full)` + `store_pending_restart`.
2. **[메시지 펌프]** — `WM_TIMER`(60ms) 또는 D3 `WM_UNIM_FLUSH` 가 펌프를 돈다. 이 사이에
   Blink 가 full 조합을 문서에 등록한다.
3. **Phase2b (세션2)** — `PostMessage(WM_UNIM_FLUSH2)` → `commit_and_restart(commit, preedit)`.
   검증된 네이티브 음절전환 경로(`CommitRestartEditSession`, 로그상 1032회 성공)를 그대로
   재사용해 앞부분 확정 + 마지막 음절 재조합. `text_service.rs`: `WM_UNIM_FLUSH2` 핸들러 →
   `timer_flush_restart_phase_b` → `key_handler::flush_restart_phase_b` → `insert_restart_phase_b`.

두 경로(synth/native)는 **같은 `PENDING_RESTART` → `WM_UNIM_FLUSH2` → `commit_and_restart`
체인** 으로 수렴한다.

## 깨지기 쉬운 지점 (수정 시 점검)
- **commit_and_restart 를 Phase2b 로 분리한 PostMessage 펌프를 제거하지 말 것.** "같은 세션에서
  바로 commit_and_restart 하면 되지 않나" → 안 된다(펌프 없음 = 원래 버그).
- **역방향/commit-only(preedit 없음)** 는 후속 조합이 없어 같은-ec 단일 commit 으로도 정착하므로
  분할하지 않는다(분기 B). 분할로 바꾸지 말 것(불필요한 지연).
- **M1 가드**: `OnCompositionTerminated` 는 `composition_mgr.clear()` 와 함께
  `discard_pending_restart()` 를 호출해야 한다. 안 하면 full 조합이 펌프 중 terminate 될 때
  Phase2b 가 이미 문서에 있는 텍스트를 또 삽입해 **이중삽입**("우간다우간다")이 된다.
- **race-flush 가드**(`text_service` OnKeyDown): `has_pending_insert() || has_pending_restart()`
  를 모두 검사해, 펌프 대기 중 다음 키가 오면 Phase2b 를 선행한다(stale 조합 오염·슬롯
  덮어쓰기 방지).
- **PENDING_RESTART 생애주기**: store 1곳(순방향), take 1곳(`flush_restart_phase_b`), discard 는
  `OnCompositionTerminated`·`OnSetFocus`·`RevWindow::Drop`·no-context·null-ptr 핸들러.
- **부작용(회귀 아님)**: ShiftStart `full=true` 인 정식 TSF 앱(메모장 등)도 이제 순방향 교정 시
  60ms 지연 + 순간 full-조합 밑줄이 생긴다(의도된 trade-off). 정식 TSF 는 60ms 내 terminate
  하지 않아 정상 동작한다.

## 관련 코드
- `unim-tsf/src/composition.rs`: `ReplaceSurroundingEditSession::DoEditSession`(순방향 A / 역방향 B),
  `replace_surrounding`(PhaseSplit + store_pending_restart), `insert_pending`(synth case3),
  `insert_restart_phase_b`, `commit_and_restart`/`CommitRestartEditSession`(+commit_applied).
- `unim-tsf/src/text_service.rs`: `WM_UNIM_FLUSH2`, `rev_wnd_proc`(FLUSH/TIMER→FLUSH2 포스팅),
  `timer_flush_restart_phase_b`, `flush_restart_phase_b` 래퍼, race-flush 가드,
  `OnCompositionTerminated` M1 가드.
- `unim-tsf/src/synth_input.rs`: `PENDING_RESTART` 슬롯 + store/take/has/discard.
- `unim-tsf/src/key_handler.rs`: `flush_restart_phase_b`.

## 검증
설치(`dist/unim-0.3.38-x64.msi` 이상) → 크롬 완전종료/재부팅 → `jbkfsuf`/`ntkdmes` 입력 →
확정 앞부분 + 마지막 음절 조합. 로그: `native full-composition alive → Phase2b 예약` →
`WM_UNIM_FLUSH2 → Phase2b` → `Phase2b commit_and_restart done → last kept (alive)`.
