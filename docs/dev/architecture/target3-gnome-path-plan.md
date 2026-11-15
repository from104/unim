# 대상③ GNOME push 경로 판정 다듬기 — 수정 계획

- 기준: develop `ec9a682`, 2026-11-03 로그(데몬 PID 10099, gnome-shell 4992). 입력 자료는 진단 일지(2ndBrain `2026-11-09 대상③ … 원인 진단`)와 조사 A(외부 소스)·B(사례 14건)
- 표기: **[확]** = 소스 줄이나 로그 줄로 근거가 있음. **[추]** = 추정이라 실측이 필요함
- 역할: 구현은 sonnet, 검증은 opus. 커밋은 단계마다 하나씩 하되 **기현님 승인 뒤에만** 한다(메모리 규칙)

## 0. 원칙과 범위

**안전 불변식 (모든 단계 공통)**
- 실제 선택이 남아 있을 수 있으면 대상③ 을 쓰지 않는다. 의심스러우면 이모지로 간다(§2.4b.7 fail-closed)
- 새 규칙은 플래그를 **푸는** 쪽에만 추가한다. 이때도 다음 조건을 지킨다
  - 그 플래그를 세운 신호가 철회됐다는 직접 증거가 있어야 한다
  - 낡음(stale) 게이트와 LO `c==0` 방어(§2.4b.8)는 그대로 남긴다(이중 방어)
- 엔진 변경은 모두 `sw_push()`(caps 에 S 없음 + P 있음) 안에서만 효력이 있다. S 컨텍스트(Qt·GTK 모듈·TSF)는 시각과 무관하게 종전대로 판정한다. 이 성질은 `contract_s_contexts_are_time_independent`·`contract_s_context_never_defers` 가 고정한다

**이번 범위에서 하지 않을 것**
- 환경변수(`GTK_IM_MODULE` 등) 변경과 그 제안 — 기현님 결정 사항
- Qt·GTK3/4 모듈·XIM·Windows TSF 경로 수정, POPUP_SPEC 변경
- R7·R8 상수(150·300·450ms) 변경. 단, M1 실측에서 보고 지연이 100ms 를 넘으면 별도 승인 안건으로 올린다
- `commit('')` 사용: LO Writer 선택을 지운다(edtwin.cxx L5922)
- `delete_surrounding(0,0)`·`request_surrounding` 사용: 효과가 없다(A Q1·Q2)
- PRIMARY **내용** 읽기: §2.10.1 퇴역. mimetype **개수**만 본다(S5)
- Firefox·GTK4 앱에 빈 preedit 왕복 적용: Firefox 에서는 compositionstart 부작용이 있고, 둘 다 키마다 보고하므로 필요도 없다
  - **2026-11-15 정정(v1.9)**: 실세션에서 Firefox 는 키마다 보고하지 **않고** 확정 직후 보고가 한 박자 늦었다. 기현님 승인으로 Firefox 도 왕복(정착 200ms)과 한자키 직전 낡음 표시를 한다. compositionstart·선택 삭제는 실측 뒤 판단(HANJA_WORD_SPEC §2.4b.6 Firefox)
- FIELD_FOCUS 에 버튼 상태를 싣는 안(B 후보 5): 사례 11 의 결과가 cursor0 으로 같아 효과가 0 이다. select-on-focus 전제가 [추]라서 보류한다
- 보고와 키 순번 매기기(B E-1 위험): 계측 필드만 넣고 동작 수정은 다음 범위로 넘긴다
- GNOME 경로 L3 하네스 신설(§5 참조)

## 1. 원인 유형 → 해결 수단

| 유형 | 사례(B 표 #) | 해결 수단 | 근거 | 단계 |
|---|---|---|---|---|
| A. LO 는 키 이동·삭제 뒤 보고하지 않음 → R7 기한 만료(stale) | 5·6·7·8 | 확장이 한자키를 엔진에 보내기 **직전**에 LO 창에서만 **빈 preedit 왕복**(`""` → 보고 도착 → `null`)을 해서 보고를 끌어낸다. 그 보고는 기존 R7 Report 경로(`poll(Report)`)가 받는다 | 빈 preedit → GTK3 `done`·`update_im`(imwayland L299-308) → `notify_im_change` → LO retrieve(L6072) [확]. LO 부작용 없음(L6029 조기 반환) [확]. 실기기 반응은 [추] | S0b(계측) → S2 |
| B. 우회 콤보 Reset 이 surrounding 을 버림 → nosur 가 보류보다 먼저 걸림 | 9·10 | push 이고 `allow_defer` 이며 이 필드에서 보고를 받은 적이 있으면 nosur 대신 Defer(reason `nosur`) 로 판정한다. 평문은 이월하지 않고 "보고 받은 적 있음" 1비트만 이월한다 | engine_worker.rs:1163-1165, surrounding_word.rs:643-645 [확] | S3 |
| C. PRIMARY 플래그는 새 보고로도, 시간이 지나도 풀리지 않음. 클릭·평이동 뒤 해제 신호(owner NULL)도 플래그를 세움 | 2·3·4·10 (그리고 13·14 는 지금처럼 막아야 함) | 한자키 시점에 PRIMARY mimetype 이 0개이고 마지막 owner-changed 뒤 100ms 가 지났으면 새 힌트 kind 7 `PRIMARY_EMPTY` 를 보낸다. 엔진은 **플래그를 세운 원인이 PrimaryChanged 하나뿐일 때만** 플래그를 푼다 | owner NULL 만으로는 구별 불가(GTK3 재claim·GTK4 claim 중간 상태) [확]. 판정 시점 `get_mimetypes(PRIMARY)` 비어 있음 = 선택 없음은 [추, 소스상 성립] | S0a(계측) → S5 |
| D. 앱 창 안 클릭은 확장에 보이지 않음 → 키 뒤 클릭이 R5 이동 후보가 되지 못함 | 1 | mutter 가 앱 표면의 BUTTON_PRESS 에서 부르는 `vfunc_reset` 시점의 버튼·Shift 상태로 `classifyPointer` 를 돌린다. 단일 클릭이면 POINTER_PLAIN, 다중 클릭이나 Shift 면 POINTER_SELECT 를 보낸다 | mutter handle_event L897~ [확]. 리셋 시점에 btn=0x100 이 보이는지는 [추] (focus_in·set_surrounding 에서 btn=0x100 실측) | S0a(계측) → S4 |
| E. field_focus 첫 키 (Firefox) | 11 | 동작 수정 없음. R6 가 풀려도 결과는 cursor0 이모지로 같다 | surrounding_word.rs:405-410·689-690 [확] | — |
| 맞게 막은 사례 | 9(선택분)·12·13·14 | 회귀 방지용 고정 테스트만 추가한다 | — | S3·S5 테스트 |

## 2. 단계 (커밋 단위)

### S0a — 계측 PROBE 보강 (확장만, develop 모드 전용, 동작 변경 0)
`unimProbe` 는 `isDevelopMode()` 일 때만 출력한다(logging.js:179). 평문은 찍지 않는다.

**변경 지점**
1. `unim-gnome-extension/unim_input_method.js:478` `vfunc_reset` 첫 줄에 `unimProbe('reset', …)` 를 추가한다
   - 필드: `btn=${probePointerButtons()} shift=0|1 preedit_len popup=0|1 over_win=0|1 direct=0|1`
   - `over_win`: 포인터가 포커스 창 frame rect 안이고, `global.stage.get_actor_at_pos(Clutter.PickMode.REACTIVE,x,y)` 의 조상에 `Meta.WindowActor` 가 있으면 1
   - `direct`: :576 `vfunc_set_cursor_location` 의 cursor-jump 경로(:588-613)에서 직접 호출한 경우 1. 인자나 플래그로 구별한다
2. `unim-gnome-extension/key_handler.js:550-557` `_watchPrimaryOwner` 콜백의 두 번째 인자 `owner` 를 받아 PROBE `primary_owner` 를 찍는다
   - 필드: `null=0|1 mimes=${(sel.get_mimetypes(Meta.SelectionType.SELECTION_PRIMARY)||[]).length}`
   - mimetype **개수만** 찍는다. 내용·mime 이름은 찍지 않는다
3. 한자키 PROBE `hanja_key` (key_handler.js 의 `_handleKeyPress` :932, `_handleVfuncKey` :335, `_processQueuedEntry` :626 에서 processKey 직전, `this._hanjaKeysyms.has(keyval)` 일 때)
   - 필드: `lo=0|1`(wm_class 가 `/^(libreoffice|soffice)/i` 에 맞는지. 이름은 찍지 않는다), `cache=none|fresh|dirty|keysent`, `cache_age_ms`, `preedit_len`, `primary_mimes`, `since_owner_ms`
4. `key_handler.js:457` `_sendHint` 의 PROBE `hint_sent` 에 `btn=` 를 추가한다
5. `logging.js` 에 순번 필드를 추가하지 않는다. 기존 `seq` 와 `t` 로 충분하다

**안전·비영향**: 로그만 바뀐다. `get_mimetypes` 는 동기 메모리 조회라 키 경로 비용이 무시할 만하다 [추]. develop 모드가 아니면 호출하지 않도록 PROBE 블록 안에 둔다.

**테스트**: `make check-gnome-api`(Makefile:577), `make test-gnome-logic`(Makefile:583) 통과

### S0b — 빈 preedit 왕복 실험 (develop 모드 전용 + LO 창 한정)
**변경 지점** — `unim-gnome-extension/unim_input_method.js` 에 `pokeSurroundingReport()` 를 신설한다

- **발동 조건(모두 충족)**
  - `this._hasFocus`
  - `this._preeditText === ''`
  - 민감 목적 아님(`isSensitivePurpose()`)
  - 팝업이 보이지 않음
  - 포커스 창 wm_class 가 LO
  - 진행 중인 왕복이 없음
  - 캐시가 `null`·`dirty`·`keySent` 중 하나(surrounding_facts.js:63-73)
- **1단계**: `set_preedit_text('', 0, 0, Clutter.PreeditResetMode.CLEAR)` 를 보내고 `this._poke = {atMs}` 를 기록한다. PROBE `poke_start`
- **2단계**: 왕복 중 `vfunc_set_surrounding`(:640) 이 오면 그 보고는 그대로 데몬에 전달한다. 이어서 `GLib.idle_add` 로 다음 주기에 `_preeditText === ''` 이고 왕복이 아직 이 것이면 `set_preedit_text(null, 0, 0, CLEAR)` 를 보낸다. PROBE `poke_report ms=` 와 `poke_end why=report`
  - mutter 는 같은 idle 안의 done 을 병합한다(defer_done L196) [확]. 그래서 반드시 보고 도착 **뒤의** 별도 주기에서 보낸다
- **타임아웃**: 250ms 안에 보고가 없으면 `null` 로 되돌린다. PROBE `poke_end why=timeout`
- **취소**: `updatePreedit`·`commitText`·`vfunc_focus_out`·`vfunc_reset` 중 하나라도 일어나면 `_poke` 를 지운다. 실제 preedit 이 `""` 를 덮어쓰므로 `null` 을 따로 보내지 않는다
- **호출 위치**: S0a-3 의 세 processKey 직전. 한자키이고 `isUnimDevelop()` 일 때만 부른다. processKey 가 동기라 mutter done idle 은 그 반환 뒤에 돌고, 엔진은 R7 Defer 로 기다리다가 보고가 오면 `poll(Report)` 로 재판정한다(engine_worker.rs:3013) [확]
- **위험 점검 (M1 에서 확인)**
  - 두 번째(동일) 보고가 열린 한자 팝업을 건드리지 않는지. `set_surrounding_text` 는 텍스트·커서만 갱신하고 `exhausted` 를 지운다(surrounding.rs:112-130) [확]
  - GTK3 serial 불일치로 done 이 무시되는지(imwayland L307) [추]

### M1 — 기현님 실측 1회차 (S0a+S0b 설치 뒤)
단계를 최소화했다. 각 항목은 한자키 뒤 **Esc 로 닫으면 된다**. 결과 판정은 Claude 가 로그로 한다.

| # | 앱 | 동작 | 보는 것 |
|---|---|---|---|
| 1 | LO Writer | `학교` 입력 → Space → ← 1회 → 한자키 | `poke_report` 도착 여부·지연 ms, 판정 ok 여부 |
| 2 | LO Writer | 단어 가운데 한 번 클릭 → 한자키 | `reset btn=0x100 over_win=1`, `primary_owner null/mimes`, `hanja_key primary_mimes` |
| 3 | LO Writer | 단어 더블클릭 → 한자키 (이모지가 맞음) | 더블클릭 뒤 `primary_mimes>0` 인지, reset 이 2회 오는지 |
| 4 | gedit | 단어 끝 클릭 → 한자키 → 드래그로 몇 글자 선택 → 한자키 (이모지가 맞음) | 클릭 뒤 `mimes=0`, 드래그 뒤 `mimes>0` |
| 5 | TextEditor | 4번과 같음 | GTK4 의 clear = 0-mime source 인지 |
| 6 | Firefox (textarea) | 칸 클릭 → `학교` 입력 → Space → ← → 한자키 | poke 가 **발동하지 않음**(`lo=0`), 키마다 보고가 오는지 |

**결과에 따른 분기**
- **1번 `poke_report` 가 왔고 판정 ok** → S2 진행
- **1번 보고가 왔지만 150ms 를 넘김** → 상수 조정을 승인 안건으로 올린다(TOTAL_MAX 450 안에서). S2 는 그대로 진행
- **1번 보고가 안 옴(timeout)** → S2 폐기. 명세 §2.4b.8 에 "LO 는 키 이동 뒤 이모지" 를 한계로 기록한다(승인 필요). S0b 코드는 되돌린다
- **2번에서 `reset btn` 에 버튼1이 보이고 `over_win=1`·`direct=0`** → S4 진행. 아니면 S4 폐기
- **2·4·5번 클릭 뒤 `primary_mimes=0`, 3·4·5번 선택 뒤 `>0`** → S5 진행
  - LO 클릭 뒤에도 `>0` 이면(Writer 가 clear 하지 않음) LO 사례 2·3·4 는 이모지로 남는다. S5 는 gedit·TextEditor 효과만 보고 진행한다
- 로그 추출: `rg 'PROBE seq=.*ev=(reset|primary_owner|hanja_key|poke_)' ~/.unim-log/*gnome-shell*.log` 와 데몬 `대상③: reason=` 줄을 시각으로 맞춘다(평문 없음)

### S1 — 명세 개정 (승인 필요, 문서만)
S2~S5 중 M1 에서 살아남은 항목의 명세 문안만 반영한다(§4 초안). 구현 커밋보다 **먼저** 한다(`e41cd24` 관례).
- 대상 파일: `docs/dev/specs/HANJA_WORD_SPEC.md` §2.4b.6(L257)·§2.4b.7(L294)·§2.4b.8(L329)·§2.10.1(L421)·§4.6, `unim-gnome-extension/SPEC.md` §2.8, `unim-dbus/SPEC.md` §6.1 (kind 표)
- 버전을 v1.8 로 올리고 결정 기록을 남긴다

### S2 — 빈 preedit 왕복 정식화 (확장만)
- S0b 의 `isUnimDevelop()` 게이트를 없앤다. LO 한정 조건은 유지한다
- PROBE 는 그대로 develop 전용이고, 일반 로그에는 `unimLog('IME', 'poke start/end why=')` 만 남긴다
- **안전**
  - 문서를 바꾸지 않는다: 빈 preedit 이고, LO 는 L6029 에서 조기 반환하며, preedit-end 는 vcl 에서 no-op 이다 [확]
  - 끌어낸 보고는 stale 만 푼다. 선택 플래그·`c==0` 방어는 그대로다
  - LO 에 선택이 있으면 보고 자체가 "선택 문자열, cursor 0" 이라 cursor0 이모지로 간다(§2.4b.8) [확]
  - 즉 **보고를 더 받을수록 정확해질 뿐 판정이 느슨해지지 않는다**
- **비영향**
  - 확장 전용이라 Qt·GTK 모듈·XIM·TSF 와 무관하다
  - LO 가 아닌 GNOME 앱에는 발동하지 않는다
- **테스트**: 발동 조건 판정을 순수 함수 `decidePoke(facts, ctx)` 로 surrounding_facts.js 에 넣는다. `scripts/test-gnome-surrounding-hints.mjs` 에 다음 표를 추가한다
  - 캐시 none/fresh/dirty/keysent × preedit 유무 × lo × popup × sensitive × inflight

### S3 — nosur 도 보류 (엔진)
**변경 지점**
1. `src/input_engine/types.rs:285~` `SurroundingWordCarry` 에 `had_report: bool` 을 추가한다. push 전용, 평문이 아닌 1비트다
2. `src/input_engine/surrounding_word.rs`
   - `sw_note_report`(:397-413) 에서 push 면 `sw_had_report = true`
   - FocusIn·FIELD_FOCUS 표시와 `sw_drop_push_state` 에서 false
   - carry/restore(:508-540) 에 포함한다. 인-플레이스 `reset()` 은 지우지 않는다
3. `surrounding_word_judge` :643-645 를 바꾼다
   ```rust
   if n == 0 || cursor > n || anchor > n {
       if push && allow_defer && self.sw_had_report {
           return SwJudge::Defer { deadline_ms: self.sw_now_ms.saturating_add(SURROUNDING_DEFER_WAIT_MS), reason: "nosur" };
       }
       return reject("nosur", 0);
   }
   ```
   - 재판정(`resume`, `sw_no_defer`)은 `allow_defer=false` 라 두 번째 보류는 없다. 판정 순서(§2.4b.7 끝)는 바뀌지 않는다
4. `unim-dbus/src/engine_worker.rs` 는 수정할 필요가 없다. Defer 응답과 `arm_defer`(service.rs:2962) 가 reason 과 무관하게 동작한다. 로그 이유 코드 `defer reason=nosur` 만 추가된다
- **안전**
  - 보류는 "판정을 미루는 것" 일 뿐이다. 재판정은 새 보고로 텍스트가 채워진 뒤의 정상 판정이고, 보고가 없으면 종전처럼 nosur 이모지로 간다
  - 보고를 받은 적 없는 필드(문맥 미지원 앱)는 보류하지 않으므로 이모지 지연이 생기지 않는다
- **비영향**
  - S 컨텍스트는 `push=false` 라 종전과 같다
  - `contract_s_context_never_defers` 가 고정한다
- **테스트**: `tests/contract/surrounding_word_contract.json` 에 다음 사례를 추가한다
  - `push_nosur_after_reset_defers_then_report_ok`: caps5 → surrounding → key(Ctrl+Right, KEY 힌트) → reset → hanja ⇒ defer(nosur) → surrounding(신선) → poll(report) ⇒ hanja
  - `push_nosur_after_reset_deadline_emoji`: 위와 같은데 보고 없이 deadline ⇒ emoji, reason nosur
  - `push_nosur_never_reported_immediate`: caps5 → focus_in → hanja ⇒ 즉시 emoji(nosur, pending false)
  - `push_nosur_defer_then_selection_report_selpossible`: 사례 9 재현. Ctrl+Shift+Left(KEY 힌트 → ShiftNav) → reset → hanja ⇒ defer → 보고 ⇒ emoji(selpossible)
  - `s_nosur_never_defers`: caps3 → reset → hanja ⇒ 즉시 nosur

### S4 — 앱 창 클릭을 포인터 힌트로 (확장만)

> **구현 결과 (2026-11-13, opus 검증 반영)**: 아래의 POINTER_PLAIN/SELECT 분류는 쓰지 않는다. 눌린 곳이 글 영역인지 알 수 없어(스크롤바·헤더바도 `over_win=1`) PLAIN 이 선택 플래그를 모두 풀면, 선택 anchor 를 보고하지 않는 GTK4 에서 선택 글자를 잃을 수 있다. 그래서 앱 표면 눌림은 새 힌트 **kind 8 `POINTER_APP`** 하나로 보낸다. 엔진은 낡음 표시·보류 취소만 하고, 선택 플래그는 원인이 FocusIn·FieldFocus 뿐일 때만 푼다. 선택이 생기고 풀리는 판단은 PRIMARY(kind 4·7)에 맡긴다. S3 의 `had_report` 도 "비어 있지 않은 보고를 마지막으로 받았다" 로 좁혔다. 명세는 HANJA_WORD_SPEC v1.8 이 기준이다.
**변경 지점**
1. `unim-gnome-extension/unim_input_method.js:478` `vfunc_reset` 에서 다음 조건을 모두 만족하면 `this._onAppPointerPress?.({button, shift, x, y, nowMs})` 를 부른다
   - `direct=0`
   - 버튼 마스크에 BUTTON1/2/3 중 하나
   - `over_win=1`
   - 팝업이 보이지 않음
   - 이번 눌림에 이미 보내지 않음. 같은 버튼 마스크가 유지되는 동안 1회로 한다
2. `key_handler.js:515-540` 의 captured 경로가 쓰는 `classifyPointer`(pointer_hints.js:52) 를 같은 `_pointerState` 로 재사용해 `_sendHint` 한다
   - 다중 클릭(500ms) 과 Shift 는 SELECT, 단일 클릭은 PLAIN, 3번 버튼은 무시, 2번 버튼은 PLAIN 이다. 종전 규칙 그대로다
   - 셸 UI captured 사건과 같은 눌림을 두 번 세지 않도록 `over_win` 으로 배타 처리한다
3. 드래그는 여전히 보이지 않는다. 드래그 선택은 PRIMARY claim 이 `PrimaryChanged` 로 다시 플래그를 세운다
   - claim 은 눌림 **뒤**에 온다(LO SttSelect·GTK claim) [확: A Q3·Q4]
- **안전**
  - POINTER_PLAIN 은 stale 을 세우므로(surrounding_word.rs:282-288) 클릭 뒤 새 보고가 와야만 판정한다
  - 눌림 시점에 선택이 지워지지 않는 경우(선택 안 눌러 DnD 시작)는 놓을 때 해제되거나, 문서가 바뀌어 stale 로 남는다
  - Calc editeng 의 마우스 선택이 PRIMARY 를 claim 하는지는 [추]다. claim 하지 않아도 LO 보고가 `c==0` 이고, GTK3 는 드래그 뒤 보고하지 않아(released_cb 임계값) stale 로 남는다
- **비영향**: 확장 전용이다
- **테스트**: `scripts/test-gnome-surrounding-hints.mjs`
  - 발동 판정을 순수 함수 `decideAppPress({direct, btnMask, overWin, popup, lastMask})` 로 분리하고 표를 추가한다
  - `classifyPointer` 기존 시험에 "리셋 유래 press 연속 2회 → SELECT" 를 추가한다
  - 엔진 픽스처에 사례 1 을 재현한다(`push_key_then_click_report_plain_hint_ok`): caps5 → field_focus → surrounding(기준) → key(미소비) → hint kind2 → surrounding(c=17) → (t+400) hanja ⇒ hanja

### S5 — PRIMARY 비어 있음 힌트 kind 7 (엔진 + 확장 + 명세)
**변경 지점**
1. `src/input_engine/types.rs:269-276` 에 `SURROUNDING_HINT_PRIMARY_EMPTY: u32 = 7` 을 추가한다
2. `src/input_engine/surrounding_word.rs`
   - 원인 누적 비트 `sel_cause_bits: u8` 을 신설한다
   - `set_sel_flag`(:245-248) 에서 OR 하고, `clear_sel_flag`(:250-253) 에서 0 으로 한다
   - carry 에 포함한다(types.rs carry 구조체)
   - `surrounding_hint`(:264~) 에 분기를 추가한다
     ```rust
     SURROUNDING_HINT_PRIMARY_EMPTY => {
         if self.sw_push() && self.selection_possible
             && self.sel_cause_bits == SelFlagCause::PrimaryChanged.bit() {
             self.clear_sel_flag();
             unim_log!("ENGINE", "SurroundingHint: kind=7 (primary_empty) → flag_cleared");
         } else { unim_log!(… "→ kept cause_bits={:#x}") }
     }
     ```
   - stale 은 건드리지 않는다. 보류(`sw_cancel_defer`)도 하지 않는다. 이 힌트는 한자키 직전에 오기 때문이다
   - `sw_last_select_ms` 를 지우지 않는다. 그래서 R6 veto 는 보수적으로 남는다
3. `unim-gnome-extension/pointer_hints.js:23-28` 에 `HINT_PRIMARY_EMPTY = 7` 을 추가한다
   - `key_handler.js` 의 S0a-3 위치(한자키 processKey 직전)에서 다음 조건이면 `_sendHint(HINT_PRIMARY_EMPTY)` 를 보낸다
     - `get_mimetypes(PRIMARY)` 길이가 0
     - `_lastOwnerChangedMs` 로부터 100ms 이상 지남
   - `_watchPrimaryOwner`(:550) 는 시각만 추가로 기록한다
   - :546 주석 "mime 은 조회하지 않는다" 를 "**개수만** 조회, 내용·이름은 읽지 않는다" 로 바꾼다(승인 항목)
   - `factsOnHintSent` 의 `SUPERSEDING_HINTS` 에는 **넣지 않는다**. 문서를 바꾸는 사건이 아니기 때문이다
4. 데몬은 kind 를 그대로 엔진에 넘긴다. 구버전 엔진은 모르는 kind 를 무시한다(unim_input_method.js:379 주석) [확]
- **안전 근거**
  - 비트가 **PrimaryChanged 단독**일 때만 푼다. 그래서 다음 경우는 풀리지 않는다
    - Shift+탐색·Ctrl+A(ShiftNav 등)
    - POINTER_SELECT
    - FocusIn·FieldFocus
    - 이들과 PRIMARY 가 섞인 경우
  - Calc·Impress 키보드 선택(PRIMARY 미반영)도 키 원인 비트가 남아 막힌다 [확: §2.10.1]
  - 남은 위험: 포커스 앱에 마우스 선택이 있는데, 백그라운드 앱이 PRIMARY 를 잡았다가 놓아 owner 가 NULL 이 되는 경우 [추, 드묾]
    - 이때도 LO 는 보고가 `c==0` 이라 이모지로 간다
    - GTK3·GTK4 는 선택이 있는 동안 소유를 다시 잡지 않으므로 이 경우가 남는다. 위험 등급은 "드묾·글자 손실 가능" 이다
    - 대응: 확장이 `mimes==0` 일 뿐 아니라 마지막 owner-changed 가 **포커스 창이 포커스를 가진 동안** 일어났을 때만 보낸다. 포커스 창 밖 사건 뒤에는 보내지 않는다. owner-changed 시각에 포커스 창 id 를 함께 기록한다
- **비영향**
  - kind 7 은 GNOME 확장만 보낸다
  - 엔진 분기는 `sw_push()` 안에서만 동작한다
  - Qt·GTK 모듈·XIM·TSF 는 이 kind 를 보내지 않는다
- **테스트 (픽스처)**
  - `push_click_primary_cleared_empty_ok`: 사례 2·4 재현. caps5 → surrounding c16 → hint4 → hint7 → hanja ⇒ hanja
  - `push_shiftnav_then_primary_empty_kept`: key Shift+Left → hint4 → hint7 → hanja ⇒ emoji selpossible
  - `push_focus_in_then_primary_empty_kept`: focus_in → surrounding → hint4 → hint7 → hanja ⇒ emoji selpossible (R6 미충족)
  - `push_drag_primary_burst_kept`: 사례 13. hint4×25 → hanja(hint7 없음) ⇒ emoji
  - `s_context_primary_empty_ignored`: caps3 에서 hint7 이 무효
- **테스트 (확장 node)**: 보낼지 판정하는 순수 함수 `decidePrimaryEmpty({mimes, sinceOwnerMs, ownerFocusWin, focusWin})` 표를 추가한다

### M2 — 기현님 실기기 재시험 (S2~S5 설치 뒤, 최종)
**GNOME 경로 교체(후보 선택 → ReplaceSurroundingText → 대조)를 반드시 포함한다.** 앱마다 같은 3동작이다.

공통 준비: 각 앱에 `학교 ` 를 입력해 둔다(Space 포함).
- Firefox 는 textarea 가 있는 페이지가 필요하다. 구현 단계에서 `tests/manual/target3-textarea.html` 같은 한 장짜리 페이지를 만들어 즐겨찾기에 두면 타이핑을 줄일 수 있다

| # | 앱 | 동작 | 기대 |
|---|---|---|---|
| 1 | TextEditor | `교` 뒤를 클릭 → 한자키 → Enter(첫 후보) | `學校` 로 교체, 데몬 `REPLACE_VERIFY … ok` |
| 2 | gedit | 1번과 같음 | 같음 |
| 3 | LO Writer | ← 1회(Space 앞으로) → 한자키 → Enter | 같음 (S2 보고 유도 경로) |
| 4 | LO Writer | `학교` 를 더블클릭 → 한자키 | **이모지** (선택 보호) → Esc |
| 5 | Firefox | 1번과 같음 | 같음 |
| 6 | gedit | Shift+← 2회 → 한자키 | **이모지** → Esc |

- 실패 시 확인할 로그: 데몬 `대상③: reason=`·`REPLACE_VERIFY`, 확장 `PROBE … ev=(poke_|hanja_key|reset)`
- 합격선
  - 교체 4건(1·2·3·5) 모두 ok 이고, 글자 손실이 0 이다
  - 보호 2건(4·6)은 이모지다

## 3. 단계별 검증 (opus)

| 단계 | 기계 검증 | 검토 초점 |
|---|---|---|
| S0a | `make check-gnome-api`, `make test-gnome-logic` | PROBE 가 develop 게이트 안에 있는지, 평문·mime 이름이 출력되지 않는지 |
| S0b·S2 | 위와 같음 + node 표 시험 | done 병합 회피(별도 주기), 취소 경로 4종, LO 한정, 비밀번호 칸 제외 |
| S3 | `cargo test -p unim-core`(계약 픽스처 포함), `cargo test -p unim-dbus --test surrounding_word`, `make check-windows` | S 컨텍스트 무보류, `had_report` 이월·초기화 지점, 두 번째 보류 없음 |
| S4 | node 표 시험 + 픽스처 | 직접 호출 reset 제외, 셸 UI 사건 중복 제외, 팝업 클릭 제외 |
| S5 | S3 와 같음 + node 표 시험 | 원인 비트 단독 조건, carry 포함, SUPERSEDING 미포함, 포커스 창 조건 |
| 공통 | L3 docker 회귀 (gtk3 26·gtk4 26·qt5 25·qt6 25 그대로) | 엔진 변경이 pull 경로에 영향 없음 |

**L3 의 한계**: L3 는 Xvfb 위 GTK·Qt 모듈 시험 앱만 다룬다. mutter·text-input-v3·확장 경로는 검증하지 못한다. 대체 검증은 다음 세 겹이다.
1. 엔진 계약 픽스처로 11-03 사례 14건의 사건열을 재현해 새 기대값을 고정한다. 위에 적지 않은 사례(5~8 stale + 보고 도착, 10, 12, 14)도 S2·S3 에 함께 넣는다
2. 확장 판정 로직을 순수 함수로 빼서 node 표 시험을 한다
3. M1·M2 실기기 확인

## 4. 명세 변경 초안 (모두 **승인 필요**)

**①(S2) §2.4b.6 "판정 보류" 표 아래에 추가**
> **보고 유도 (v1.8, GNOME 확장 · LibreOffice 한정)**: LibreOffice(gtk3 VCL)는 클릭과 IM 커밋 직후에만 주변 문맥을 보고하고, 엔진이 넘긴 키(←·→·BackSpace 등) 뒤에는 보고하지 않는다. 그래서 보류가 기한까지 기다려도 새 보고가 오지 않는다. 확장은 다음 조건을 모두 만족하면 한자키를 엔진에 보내기 직전에 빈 preedit 을 설정했다가 지운다.
> - 포커스 창이 LibreOffice 다
> - 조합 중이 아니다
> - 팝업이 닫혀 있다
> - 비밀번호 칸이 아니다
> - 마지막 스냅샷 뒤 키나 힌트가 있었다
>
> 이 왕복으로 GTK3 가 앱에 주변 문맥을 다시 묻고, 그 보고가 보류 재판정(①보고 도착)으로 들어간다. 문서는 바뀌지 않는다(빈 preedit 은 VCL 이 무시한다). `commit("")` 은 Writer 선택 영역을 지우므로 쓰지 않는다. 다른 앱은 키마다 보고하므로 이 동작을 하지 않는다. 특히 Firefox 에서는 빈 조합이 compositionstart 를 일으키므로 쓰지 않는다.

**②(S3) §2.4b.6 "보류 시작" 행 끝에 추가**
> 주변 문맥이 비어 있어 거절해야 하는 경우(`nosur`)도, push 컨텍스트이고 이 입력란에서 보고를 한 번이라도 받았다면(엔진 재생성으로 텍스트만 잃은 경우) 같은 150ms 기한으로 보류한다(원인 `nosur`). 보고를 받은 적이 없는 입력란은 종전대로 즉시 이모지로 간다.

이유 코드 목록(§2.4b.7 "이유 코드")의 원인에 `nosur` 를 추가한다.

**③(S4) §2.4b.7 표 "설정(보강)" 행의 엔진이 아는 경로 칸에 추가**
> 앱 창 안의 눌림은 셸이 포인터 사건을 받지 못하므로, mutter 가 눌림 때 부르는 IM reset 시점의 버튼·Shift 상태로 분류한다(단일 = POINTER_PLAIN, 500ms 이내 재눌림·Shift = POINTER_SELECT). 드래그는 보이지 않으며 PRIMARY 변경으로 막는다.

§2.4b.7 의 "해제 | Shift 없는 단일 클릭" 행은 그대로 유효하다.

**④(S5) §2.4b.7 표에 해제 행을 추가하고 §2.10.1 에 주석을 단다**
> | 해제(v1.8) | **PRIMARY 비어 있음** — 한자키 시점에 PRIMARY 소유자가 없거나 mimetype 이 0개이고, 마지막 PRIMARY 변경이 100ms 이상 지났으며 포커스 창이 포커스를 가진 동안 일어났다. **플래그를 세운 원인이 PRIMARY 변경 하나뿐일 때만** 푼다(키·포인터 선택·포커스 원인이 섞였으면 그대로) | 확장 `SurroundingHint(PRIMARY_EMPTY, kind 7)` |
>
> §2.10.1 주석: v1.8 의 PRIMARY_EMPTY 는 PRIMARY **내용을 읽지 않는다**. 셸 메모리의 mimetype **개수**만 본다(클라이언트 전송 없음). 선택을 **추정**하는 데 쓰지 않고, PRIMARY 신호 하나로 선 거부 플래그를 같은 신호의 철회로 **되돌리는** 데만 쓴다.

§4.6·확장 SPEC §2.8·데몬 SPEC §6.1 의 kind 표에 7 을 추가한다.

**⑤(M1 분기에서 S2 폐기 시) §2.4b.8 끝에 추가**
> 실측(2026-11): GNOME 경로의 Writer 는 키 이동 뒤 보고하지 않으며 보고 유도도 반응하지 않는다. 따라서 키로 커서를 옮긴 뒤의 한자키는 이모지로 간다(클릭 뒤에는 대상③ 동작).

**⑥ §2.4b.8 "실측 대기" 줄**: M2 결과로 바꾼다.

## 5. 승인·실측 요약

**승인 필요 항목**
- S0a: mimetype 개수 조회(PROBE 전용)
- S1: 명세 v1.8 ①~④(⑤ 는 분기 시)
- 각 커밋
- 보류 상수 변경(지연이 실측될 때만)

**기현님 실측**
- M1 6항목: 계측. S2·S4·S5 진행 여부가 여기서 갈린다
- M2 6항목: 최종. 4앱 교체와 보호 2건

**구현 순서**
- S0a → S0b → M1 → S1 → S2·S3·S4·S5 → M2
- S2·S3·S4·S5 는 서로 독립이라 아무 순서로나 커밋할 수 있다
- S3 는 M1 결과와 무관하다. 다만 효과가 LO 에서 보이려면 S2 가 필요하다
