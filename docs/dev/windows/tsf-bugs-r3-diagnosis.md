> **[SUPERSEDED]** 이 진단서는 구 미커밋 상태(LAST_EDIT_REFUSED/request_sync/take_edit_refused 기계장치, range.Collapse 삭제 등)를 기술함.
> 현행 진실: 해당 미커밋 변경들은 정리됐으며, TSF 터미널 inline은 `composition.rs:157 fInterimChar=BOOL(1)` + non-sticky 리셋으로 확정 구현됨.
> → [`_PATCH_DIRECTION_v2.md`](./_PATCH_DIRECTION_v2.md) §0·§2 참조.
> 역사 보존 목적으로 본문은 그대로 둔다.
>
> **[추가 정정 · KakaoTalk/IMM32 · 2026-06-22 SOLVED]** 이 문서의 카톡/한컴 진단
> ("OnKeyDown 0회 = 이 앱들이 IMM32 후킹으로 키를 가로채 msctf에 안 닿음 → IMM32 .ime 경로 필요/unsupported")은
> **오진이었다**. 진짜 원인은 키 라우팅 차단이 아니라 **`unim_tsf.dll`이 x64 단독이라 32-bit 카톡에
> TIP 자체가 등록·로드되지 않은 것**(그래서 sink는 다른 64-bit 앱에서만 무장됐고 32-bit 카톡에선 TIP 미존재).
> i686 `unim_tsf.dll`을 32-bit TSF로 등록하니 카톡 OnKeyDown이 발화하고 한글 입력됨(실증). IMM32 경로 불필요.
> 최종 진실: [`imm32-win11-SOLUTION.md`](./imm32-win11-SOLUTION.md).

# UNIM TSF R3 버그 진단 (워크플로우 결과)

```
========================================================================
BUG imm32-apps-no-connection | regression=False conf=medium verifierOK=True
TITLE: IMM32 apps (KakaoTalk/Hancom): ActivateEx completes but keystrokes never reach the KeyEventSink
REG_SRC: 
FILES: unim-tsf/src/text_service.rs, unim-tsf/src/register.rs, installer/wix/unim.wxs, unim-tsf/src/key_handler.rs, unim-tsf/src/composition.rs
RELATED: 
ROOT_CAUSE:
Key events are not routed to the TIP KeyEventSink for CUAS unaware IMM32 native apps (KakaoTalk, Hancom). Log analysis of %TEMP%/unim-tsf.log (41076 lines): 48 PIDs completed ActivateEx (AdviseKeyEventSink success, OPENCLOSE and CONVERSION compartments set, reverse channel armed) yet 37 of them never produced a single OnKeyDown or OnTestKeyDown (handle_key_down log). Activation and sink registration are fine, but these apps consume keys via IMM32 and never hand them to msctf ITfKeystrokeMgr. Decisive proof: the Hangul toggle is also dead; the toggle runs in OnKeyDown via engine.press_key (text_service.rs:285-289 no PreserveKey; key_handler.rs:80), so if OnKeyDown fired input_category would flip and OPENCLOSE would update, but it does not, so OnKeyDown is never called. The 0.3.22 request_sync and LAST_EDIT_REFUSED (composition.rs) and the not-composition_unsupported english consume gate (key_handler.rs:106-113) all run inside handle_key_down, so they are inert for apps that never enter handle_key_down, which is why hardening them changed nothing.
FIX_DIRECTION:
Cannot be fixed by standard TSF alone; an IMM32 path is needed. Step 1 add unconditional entry logging at the top of OnTestKeyDown and OnKeyDown and reproduce zero calls in KakaoTalk and Hancom. Step 2 add an IMM32 IME path or CUAS cooperation, or document these apps as unsupported, since TSF TIPs are not auto bridged into CUAS unaware apps and these apps hook IMM32 to bypass system IMEs. Step 3 register a real HKL KLID substitute instead of register.rs:170 SubstituteLayout equals LANGID only. Step 4 document an unsupported app list. Revert does not help because this is not a regression.
VERIFIER_CRITIQUE:
Confirmed. Counts match: 48 armed, 74 with keys, 37 zero-key, PID12124 armed8 keys0. handle_key_down log at key_handler 312 fires unconditionally after press_key, so absence proves OnKeyDown never reached engine. Hangul not PreserveKey-registered (text_service 285); dead PIDs show only ActivateEx initial sync at 299. Not a regression: register and compartment unchanged. register 170 SubstituteLayout is bare LANGID not real KLID, Step 3 correct.
VERIFIER_CORRECTION: none, confirmed
========================================================================
BUG forward-typefix-first-char-deleted | regression=True conf=high verifierOK=True
TITLE: 순방향 자동교정 첫 한글 글자 소실 — ReplaceSurroundingEditSession 의 range.Collapse(TF_ANCHOR_END) 제거 회귀
REG_SRC: 커밋되지 않은 working-tree 변경(브랜치 feat/windows-msi-redesign): unim-tsf/src/composition.rs ReplaceSurroundingEditSession::DoEditSession step3 에서 range.Collapse(ec, TF_ANCHOR_END) 제거(diff 의 ②번 수정 hunk, composition.rs:836-847 부근). 정식 커밋 아님 — git diff 에만 존재.
FILES: C:\Users\USER\Desktop\work\unim\unim-tsf\src\composition.rs, C:\Users\USER\Desktop\work\unim\unim-tsf\src\key_handler.rs, C:\Users\USER\Desktop\work\unim\src\auto_typefix\forward.rs
RELATED: word-garble
ROOT_CAUSE:
커밋되지 않은 ②번 수정(composition.rs ReplaceSurroundingEditSession::DoEditSession)이 step3 commit 블록에서 range.Collapse(ec, TF_ANCHOR_END) 한 줄을 SetText 와 EndComposition 사이에서 제거한 것이 근본 원인. 이 range 객체는 step4 preedit replay 에서 그대로 재사용된다. 순방향 교정은 core check_forward(src/auto_typefix/forward.rs:108-130)가 commit_text=converted[..len-1](앞 음절들), replay_keys=마지막 음절(=replay_preedit)로 항상 분할하므로 preedit 가 비지 않는다. 예 tjrl→서기 에서 commit_text=서, preedit=기. step3: ShiftStart 로 확보한 range 에 StartComposition→SetText(서) 하면 range 가 서 전체를 SPAN. 과거엔 직후 Collapse(TF_ANCHOR_END)로 range 를 서 뒤 0폭 삽입점으로 이동시켜 step4 의 StartComposition(&range)+SetText(기)가 기를 서 뒤에 INSERT → 서기. ②수정이 Collapse 를 지우면서 step4 진입 시 range 가 여전히 서를 SPAN → SetText(기)가 서를 기로 OVERWRITE → 결과 기, 첫 글자 서 소실. move_caret_to_end(composition.rs:173-187)는 range.Clone() 사본만 Collapse/SetSelection 하므로 원본 range anchor 를 옮기지 않아 step4 덮어쓰기를 막지 못한다. 순수 ITfRange/SetText 의미론이라 앱 종류 무관 — 표준 TSF(메모장)·Blink(크롬)·CUAS(wezterm) 모두 동일 증상 광역 확산. 주석의 Blink 가 0폭 composition 을 빈 확정으로 해석 진단은 오진이며, 과거 첫글자 실제 결함은 별개 delete_chars -1 문제로 a2051b4 에서 이미 해결됨.
FIX_DIRECTION:
해당 hunk 만 revert: unim-tsf/src/composition.rs ReplaceSurroundingEditSession::DoEditSession 의 Ok(composition) 분기에서 SetText(ec,0,&wide) 다음, move_caret_to_end 앞에 range.Collapse(ec, TF_ANCHOR_END) 복원(HEAD 동일). 이래야 step4 preedit replay 가 commit_text 를 덮지 않고 그 뒤에 삽입된다. ②번 정당화 주석(839-843)도 오진이므로 제거 권장. request_sync/LAST_EDIT_REFUSED/is_cuas 게이트 등 ①·④ 변경은 무관하므로 유지 가능. 최소 변경은 Collapse 한 줄 복원.
VERIFIER_CRITIQUE:
진단의 근본원인은 코드 메커니즘 수준에서 정확하다. 직접 검증한 증거:

1) git diff(composition.rs)에서 ②수정 hunk 확인: step3 commit 분기(Ok(composition))에서 SetText(ec,0,&wide) 와 EndComposition 사이의 `range.Collapse(ec, TF_ANCHOR_END)` 한 줄이 정확히 제거됨. HEAD 버전(git show HEAD)에는 그 줄이 존재. Err(raw 폴백) 분기의 Collapse(:854)는 그대로 유지됨 → 정상 TSF StartComposition 성공 경로에서만 Collapse 소실.

2) step4(:860-876)가 step3와 동일한 `range` 바인딩을 재획득 없이 재사용함을 코드에서 확인. 사이에 range anchor 를 옮기는 연산은 EndComposition(앵커 불변)뿐. TSF 의미론상 ITfRange::SetText 는 삽입 텍스트를 cover 하도록 range 를 조정(auto-collapse 안 함)하므로, Collapse 제거 시 step4 진입 시 range 가 commit_text("서")를 SPAN → range.SetText(기,:867)가 "서"를 "기"로 OVERWRITE → 첫 글자 소실. 증상(서기→기, 메모장·터미널 확산)과 정확히 일치.

3) move_caret_to_end(:173-187)는 range.Clone() 사본을 Collapse/SetSelection → document selection 만 이동, 원본 range anchor 미이동. 제거된 Collapse 대체 불가 확인.

4) forward.rs:108-130: 순방향은 commit_text=converted[..len-1], replay_keys=마지막 음절 → commit·preedit 모두 비지 않음 → step3·step4 둘 다 실행 → 덮어쓰기 조건 항상 성립 확인.

5) 앱 무관 광역 확산: key_handler.rs:469-477 의 순방향 apply 가 is_cuas=composition_unsupported 로 전달. 메모장은 false → step2.5 SendInput CUAS 폴백(:803-823) 미발동 → 곧장 step3/step4 range 편집 진입 → 순수 ITfRange 의미론 버그라 정식 TSF·Blink·CUAS 동일 증상. 확인됨.

6) a2051b4 는 별개의 delete_chars -1 잔류 버그("ntkd"→"n서기", 첫 영문 미삭제)로 현재 증상(변환결과 첫 음절 소실)과 반대 방향 — 진단의 구분 정당.

오진 지적도 타당: 신규 주석(:839-843)의 "Blink 가 0폭 composition 을 빈 확정으로 해석" 정당화는 잘못. 실제 결함은 app-independent 한 ITfRange SetText overwrite 의미론이며, 그래서 메모장·터미널까지 확산한 것이 그 증거.

경미한 정정: 제시 증거 중 key_handler.rs:286-300 을 "순방향 apply 경로"로 지목했으나 그 구간은 실제로는 Ctrl+Z try_undo 경로(:290-300)와 수동 typefix(:276-284)다. 진짜 순방향 apply 는 process_after_key 결과를 받는 key_handler.rs:469-477 이다. 라인 인용만 빗나갔고 메커니즘(commit_text+replay_preedit+is_cuas=false 전달)은 동일하게 성립.

수정 방향(해당 Collapse 한 줄 복원, ②정당화 주석 제거, ①④ 변경 유지)도 적절. 최소·정확한 수정.
VERIFIER_CORRECTION: 
========================================================================
BUG word-forward-typefix-garbled | regression=True conf=high verifierOK=True
TITLE: MS Word 순방향 ATF "서기현 woody"가 "ntkd기 ㄹㅊdy"로 깨짐 — Word를 CUAS로 오분류해 raw 키 통과 + SendInput 비동기 폴백 충돌
REG_SRC: 미커밋 작업트리 변경(git diff HEAD: composition.rs request_sync/LAST_EDIT_REFUSED/is_cuas, key_handler.rs composition_unsupported 게이트, text_service.rs take_edit_refused 학습). 커밋 10743c6 'ATF 순방향 IMM32 무반응 + Chrome/CUAS commit 경로' 위에 쌓인 0.3.19~0.3.22 미커밋 변경이 원인.
FILES: C:\Users\USER\Desktop\work\unim\unim-tsf\src\composition.rs, C:\Users\USER\Desktop\work\unim\unim-tsf\src\text_service.rs, C:\Users\USER\Desktop\work\unim\unim-tsf\src\key_handler.rs, C:\Users\USER\Desktop\work\unim\unim-tsf\src\synth_input.rs, C:\Users\USER\Desktop\work\unim\unim-tsf\src\auto_typefix.rs
RELATED: firstchar
ROOT_CAUSE:
미커밋 작업트리에 새로 추가된 request_sync/LAST_EDIT_REFUSED/is_cuas 학습 기계장치가 완전 TSF 앱인 Word를 CUAS(IMM32 브리지)로 오분류한다. composition.rs:42-63 request_sync()는 context.RequestEditSession(TF_ES_SYNC)이 음수 HRESULT(예: TS_E_SYNCHRONOUS 0x80040249 — 그 순간 sync 락을 못 주는 정당한 일시적 거부)나 Err을 돌려주면 무조건 LAST_EDIT_REFUSED=true로 세운다. text_service.rs:546-556 OnKeyDown은 take_edit_refused()를 읽어 composition_unsupported=true로 만들고 GetFocus() HWND를 cuas_windows에 영구 등록한다. 그 결과 두 가지 파괴가 동시에 일어난다: (1) key_handler.rs:106-113 영문모드+forward ATF 소비 분기가 !composition_unsupported 가드에 걸려 false 반환 → raw 영문키(n,t,k,d,d,y)가 Word 문서에 직접 들어간다(OnKeyDown은 여전히 호출돼 엔진도 같은 키를 조합 처리 → raw+조합 혼재). (2) ATF 발동 시 key_handler가 replace_surrounding(is_cuas=composition_unsupported=true)를 호출 → composition.rs:805-817 ShiftStart shifted 부족 시 synth_input::send_replacement()의 SendInput(VK_BACK+KEYEVENTF_UNICODE) 비동기 큐 주입이 발화한다. 이 async 주입이 이미 Word에 동기로 들어간 raw 키와 오프셋 경쟁을 일으켜 자모가 어긋난 'ㄹㅊ'와 잔존 raw 문자를 만든다. Word는 진짜 TSF 앱이라 start_composition/commit_and_restart가 간헐 성공해 '기' 음절만 정상 조합되어 부분정상 조각이 남는다.
FIX_DIRECTION:
request_sync의 거부 판정을 협소화: TS_E_SYNCHRONOUS/일시적 락 실패를 영구 CUAS 학습 신호로 쓰지 말 것. composition_unsupported 학습은 OnCompositionTerminated 즉시종료(시간기반)로만 한정하고 take_edit_refused()→composition_unsupported=true 경로(text_service.rs:546-556)를 제거하거나 '재시도 후 반복 거부'로만 한정. 최소한 is_cuas 기반 SendInput 폴백(composition.rs:805-817)을 정식 TSF 앱(Word)에서 영구 비활성화하고, key_handler.rs:106-113의 영문 forward 소비 가드를 composition_unsupported와 분리해 raw 키 누출을 막을 것. 가장 안전한 단기 조치는 LAST_EDIT_REFUSED 학습 기계장치 일체(composition.rs request_sync 거부학습 + text_service take_edit_refused 분기)를 revert 하여 10743c6 이전 거동으로 복귀.
VERIFIER_CRITIQUE:
근본 원인(미커밋 작업트리에 새로 추가된 CUAS-학습 기계장치가 완전 TSF 앱 Word를 오분류)은 코드 증거로 확인됨. 검증 결과:

확정된 증거:
- LAST_EDIT_REFUSED/request_sync/is_cuas 는 committed HEAD(composition.rs)에 0회 — 전부 미커밋 신규(`git show HEAD:` 확인). 회귀가 미커밋 변경 소산이라는 주장 정확.
- text_service.rs:546-556 take_edit_refused()→composition_unsupported=true + cuas_windows.insert 경로도 신규(HEAD엔 OnCompositionTerminated 시간기반 insert만 존재, 라인507).
- key_handler test_key_down 의 `composition_unsupported: bool` 파라미터와 `&& !composition_unsupported` 게이트도 diff에서 신규 추가 확인.
- composition.rs:805-817 is_cuas SendInput 폴백 게이트 신규.
- composition.rs:42-63 request_sync 가 음수 HRESULT/Err 를 무조건 LAST_EDIT_REFUSED=true 로 세움 — 정당한 일시적 sync 거부를 영구 CUAS 신호로 오인하는 진원지 맞음.

따라서 "Word가 한 번의 sync 거부로 composition_unsupported=true 로 고착 → 오버레이/insert 폴백 + is_cuas SendInput 비동기 주입 경로로 진입"이라는 핵심 메커니즘은 타당.

그러나 제시된 세부 파괴 메커니즘 (1)은 부정확:
- 진단은 "key_handler.rs:106-113 영문forward 게이트가 막혀 raw 영문키(n,t,k,d,d,y)가 Word에 직접 입력되고 OnKeyDown도 호출돼 엔진이 같은 키를 동시 조합처리 → raw+조합 혼재"라 함. 두 가지 오류:
  (a) TSF에서 OnTestKeyDown=FALSE 면 그 키에 대해 OnKeyDown은 호출되지 않는다(text_service.rs:447-451 주석도 명시). 따라서 "raw 누출 + 엔진 동시처리" 동시 발생은 성립 안 함.
  (b) 더 결정적으로, key_handler.rs:106-113 게이트는 InputCategory::English 분기다. 그런데 기대출력 "서기현 woody"의 "서기현"은 한국어 모드 입력이고, 한국어 모드 문자키 소비(key_handler.rs:90-94)는 composition_unsupported 와 무관하게 항상 true 다. 즉 "ntkd기"의 raw 누출은 영문forward 게이트로 설명 불가. 실제 깨짐은 composition_unsupported=true 진입 후 폴백 경로(key_handler.rs:339-384 오버레이/insert)와 SendInput 비동기 주입(composition.rs:805-817)이 Word의 동기 문서모델과 충돌해 발생한 것으로 봐야 정확하다.

요약: 회귀 출처와 1차 메커니즘(미커밋 CUAS-오분류 기계장치가 Word에 적용)은 확정. 다만 진단이 적시한 "영문 forward 게이트 raw 누출" 경로는 본 케이스(한국어 모드 입력)에 적용되지 않으며, 실제 자모 어긋남은 폴백 경로 자체가 원인이다. 또한 request_sync 가 Word에서 실제로 음수 HRESULT를 받는다는 전제는 정적 코드로는 확인 불가(런타임 로그 필요).
VERIFIER_CORRECTION: 근본 원인 귀속은 맞음(미커밋 LAST_EDIT_REFUSED/request_sync/take_edit_refused/is_cuas 기계장치가 Word를 CUAS로 오분류). 단 정밀 메커니즘 보정: Word가 composition_unsupported=true 로 고착되면, 한국어 모드 문자키는 여전히 소비되지만(영문forward 게이트와 무관) 처리 경로가 정상 composition(commit_and_restart)에서 폴백 경로(key_handler.rs:339-384 오버레이+insert_text)와 is_cuas SendInput 비동기 주입(composition.rs:805-817)으로 전환된다. 이 폴백은 wezterm용 설계라 진짜 TSF 앱 Word의 동기 문서모델과 충돌해 자모 어긋남/잔존문자를 만든다. "key_handler.rs:106-113 영문 forward 게이트를 통한 raw 영문키 누출 + OnKeyDown 동시 엔진처리"라는 서브 메커니즘은 본 케이스에 부적용(한국어 모드 + TSF OnTestKeyDown=FALSE→OnKeyDown 미호출). 수정방향(LAST_EDIT_REFUSED 학습 기계장치 revert / take_edit_refused 경로 제거 / is_cuas SendInput 폴백을 정식 TSF에서 비활성)은 유효.
========================================================================
BUG UNIM-TSF-AUTO-ENGLISH-SLASH-CONTEXT-ALT-CONFLICT | regression=True conf=high verifierOK=True
TITLE: 세벌식 "되" 입력 시 auto_english Slash 트리거가 slash_context_alt(ㅗ 자모 경로)를 선점
REG_SRC: commit e04458d (feat(3bul): rule B — context-sensitive '/' on 3bul390/391). slash_context_alt 도입 시 기존 Functional Slash 트리거와의 우선순위 충돌이 발생했다. 또는 더 정확히는 f39ee0c에서 Functional 트리거를 도입할 때 context_alt 활성 상태를 고려한 가드를 추가하지 않은 설계 결함이 e04458d의 slash_context_alt 도입으로 실체화됐다.
FILES: src/input_engine/press_key.rs, src/keystroke/keymap/ko_3bul390.json, C:/Users/USER/AppData/Roaming/unim/config.yaml
RELATED: 동일 메커니즘으로 slash_context_alt 활성 상태에서 '/' 키 입력 시 fallback '/' 리터럴 대신 auto_english가 발동하는 모든 컨텍스트(초성 외 상태에서는 fallback '/'가 produces_char=Some('/') 반환이므로 Character 트리거와 동일하게 동작해 실제로는 정상 발동이지만, Functional Slash는 상태 무관하게 항상 발동), vowel_strict 룰셋의 v/b 키에 context_alt가 추가될 경우 동일한 Functional 트리거 선점 버그 잠재
ROOT_CAUSE:
`match_auto_english_trigger`(src/input_engine/press_key.rs:657)에서 `Functional` 트리거는 `(keycode, shift)` 직접 비교만으로 발동 결정을 내리고, `produces_char_in_korean()` 반환값이 `None`(= 해당 키가 자모 경로로 처리됨)인 경우를 차단하지 않는다.

사용자 config.yaml의 `trigger_keys: [Slash]`는 legacy 무접두사 형식으로 `parse_trigger_key`에서 `Functional { code: KeyCode::Slash, shift: Some(false) }`로 파싱된다(press_key.rs:568~571). `Character(ch)` 트리거만이 `produces_char_in_korean(...) == Some(*ch)` 비교로 자모 경로 활성 시 매칭을 건너뛰도록 설계돼 있고, `Functional` 트리거에는 해당 보호가 없다(press_key.rs:673~680).

세벌식 390에서 "되"를 입력하는 물리 키 순서는 U(`ㄷ초성`) → `/`(`slash_context_alt: choseong_only → ㅗ`) → D(`ㅣ중성, ㅗ+ㅣ=ㅚ 결합`). `/` 키 처리 시 `match_auto_english_trigger`가 `process_korean_key` 내부의 `slash_context_alt` 평가(press_key.rs:277~302)보다 먼저 호출된다(press_key.rs:160). 이때 preedit에 ㄷ초성만 있어 `is_only_cho_filled()=true`이지만, `Functional` 트리거 매칭은 이 조건을 무시하고 즉시 발동 → `flush_preedit()`(ㄷ 확정) + `set_input_category(English)` + `'/'` commit. 이후 D 키는 영문 모드에서 처리되어 'd' raw 출력. 최종 결과: "ㄷ/d".
FIX_DIRECTION:
`match_auto_english_trigger`의 `Functional` 분기(press_key.rs:673~679)에 `produces_char_in_korean()` None 체크 가드를 추가한다.

구체적으로: `Functional` 트리거 매칭 조건에 `produces_char_in_korean(keycode, modifier.shift).is_some()` 조건을 AND로 추가한다. `produces_char_in_korean`이 None을 반환하는 경우(= key_meta_map의 context_alt 조건이 충족되어 자모 경로가 활성인 경우)에는 Functional 트리거도 발동하지 않아야 한다.

변경 대상: `src/input_engine/press_key.rs` L673~679.

```rust
AutoEnglishTrigger::Functional { code, shift } => {
    *code == keycode
        && match shift {
            None => true,
            Some(required) => *required == modifier.shift,
        }
        && produced_char.is_some()  // ← 이 조건 추가
}
```

단, `produced_char`는 이미 L670에서 계산됐으므로 추가 비용 없음. 이 수정으로 slash_context_alt가 활성인 초성 컨텍스트에서 '/' 키는 Functional Slash 트리거를 무시하고 ㅗ 자모 경로를 탄다.

주의: Functional{Escape, None} 같이 non-character 제어 키는 `produces_char_in_korean`이 english_keymap.get_char 단계에서 None을 반환하므로(Escape는 문자 키가 아님) 이 수정으로 Escape 트리거가 망가질 수 있다. `english_keymap.get_char(Escape, false) = None`이면 `produced_char = None → Escape 트리거도 차단`된다. 따라서 제어 키(Functional + non-character)는 `produced_char` 체크를 우회해야 한다: keycode.is_character_key()가 false인 경우 기존 동작 유지.

최종 조건:
```rust
AutoEnglishTrigger::Functional { code, shift } => {
    *code == keycode
        && match shift {
            None => true,
            Some(required) => *required == modifier.shift,
        }
        && (!keycode.is_character_key() || produced_char.is_some())
}
```

또는 사용자 config의 `trigger_keys`에서 `"Slash"`를 `"char:/"` 형식으로 교체하는 것도 즉시 우회책이 된다(Character 트리거는 이미 produces_char 가드를 가짐).
VERIFIER_CRITIQUE:
진단의 모든 핵심 주장이 코드로 확인되었다.

1. press_key.rs:673~679 — Functional 분기는 `*code == keycode && shift` 조건만 비교하며, 같은 함수 L670에서 계산된 `produced_char`를 사용하지 않는다. 확인됨.

2. press_key.rs:698~722 — `produces_char_in_korean`은 `context_alt` 조건이 충족될 때(`cond_ok=true`, 세벌식390 초성 컨텍스트에서 `/`) 명시적으로 `None`을 반환한다(L718~719). 즉 이 정보가 이미 L670에서 계산되어 `produced_char = None`으로 존재하지만, Functional 분기가 이를 무시한다.

3. press_key.rs:160 vs 277 — `match_auto_english_trigger`(L160)가 `key_meta.context_alt` 평가 블록(L277)보다 먼저 호출된다. 순서 확인됨.

4. Escape 위험 지적도 정확하다 — `english_keymap.get_char`는 Escape에 대해 None을 반환하므로 L699 `?`에서 조기 return되어 `produced_char = None`이 되고, `produced_char.is_some()` 조건만 추가하면 Escape 트리거가 함께 차단된다. 따라서 `!keycode.is_character_key() || produced_char.is_some()` 형태의 가드가 필수다.

누락/보완 사항: git log 14개 안에 `e04458d`(slash_context_alt 도입 커밋)나 `f39ee0c`(Functional/Character 분기 도입 커밋)가 없다. 두 커밋은 현재 브랜치 이전 히스토리에 존재하며, 버그는 두 커밋의 설계 결함이 맞물린 것으로 현재 코드(feat/windows-msi-redesign)에 이미 내재되어 있다. 회귀 커밋 특정 자체는 맞으나, 현 브랜치에서 직접 검증은 불가능하다는 점을 추가로 명시해야 한다. 이것이 유일한 불확실성이며 진단 근본원인 자체와는 무관하다.
VERIFIER_CORRECTION: 
========================================================================
BUG UNIM-TSF-TRAY-MENU-001 | regression=False conf=high verifierOK=True
TITLE: 트레이 우클릭 컨텍스트 메뉴 미표시 — TrackPopupMenuEx에 NULL HWND 전달
REG_SRC: 
FILES: unim-tsf/src/lang_bar.rs
RELATED: 
ROOT_CAUSE:
lang_bar.rs `show_context_menu`(line 563)에서 `GetForegroundWindow()`를 호출하는데, 트레이 IME 인디케이터 우클릭 시점에는 전경 창이 없어 `HWND(0)`(NULL)이 반환된다. line 564의 `!hwnd.is_invalid()` 가드가 이를 통과시킨다 — windows-rs의 `HWND::is_invalid()`는 `HWND(-1)`(INVALID_HANDLE_VALUE)만 거르고 `HWND(0)`(NULL)은 유효로 판단하기 때문이다. 결과적으로 line 565의 `SetForegroundWindow(hwnd)` 분기를 건너뛰고, line 569의 `TrackPopupMenuEx(hmenu, ..., hwnd=NULL, ...)` 호출로 이어진다. Win32 문서상 `TrackPopupMenuEx`에 NULL owner HWND를 전달하면 메뉴가 즉시 숨겨지거나 아예 표시되지 않으며 반환값 0(취소)이 돌아온다 — 메뉴가 보이지 않는 직접적인 원인이다.
FIX_DIRECTION:
lang_bar.rs `show_context_menu`에서 `GetForegroundWindow()` 반환값이 `HWND(0)`(NULL)인 경우를 명시적으로 처리해야 한다. 수정 방향 두 가지:

1. **권장**: NULL HWND 조건을 `hwnd.0 == 0`으로 분리 판별. NULL이면 `SetForegroundWindow` 스킵은 유지하되, `TrackPopupMenuEx` owner를 DLL의 숨겨진 전용 helper HWND(또는 새 `CreateWindowExW` 임시 창)로 대체해 전달. Win32 규약상 owner가 반드시 필요 — NULL owner로는 메뉴가 표시되지 않는다.

2. **대안**: `GetForegroundWindow()` 대신 `FindWindow(\"Shell_TrayWnd\", NULL)` 등 실제로 존재하는 HWND를 fallback owner로 사용하거나, ActivateEx 때 생성한 `RevWindow`의 HWND(message-only 창)를 owner로 재사용. 단 message-only 창(HWND_MESSAGE 부모)은 `TrackPopupMenuEx` owner로 부적합 — 가시 창이어야 메뉴 dismiss가 올바로 동작.

핵심 수정 코드(lang_bar.rs line 563~565):
```rust
let hwnd = GetForegroundWindow();
// HWND(0)은 is_invalid()=false라 아래 분기를 별도 처리
if hwnd.0 != 0 {
    let _ = SetForegroundWindow(hwnd);
} else {
    // NULL이면 유효 owner HWND 확보 필요 (임시 창 또는 tray shell HWND)
    // 현재 코드는 NULL을 그대로 TrackPopupMenuEx에 전달 → 메뉴 미표시
}
```
장기적으로 `UnimLangBarButton` 구조체에 helper HWND 하나를 ActivateEx 시점에 생성·보관해, 트레이 우클릭 owner로 항상 유효한 HWND를 사용하는 것이 가장 안정적이다.
VERIFIER_CRITIQUE:
진단의 핵심 원인은 정확하다: `lang_bar.rs:563`에서 `GetForegroundWindow()`가 NULL(`HWND(0)`)을 반환할 수 있고, `lang_bar.rs:564`의 `!hwnd.is_invalid()` 가드가 `HWND(0)`을 유효로 판단해 통과시키며, 결과적으로 `lang_bar.rs:574`에서 NULL owner로 `TrackPopupMenuEx`가 호출되어 메뉴가 표시되지 않는다는 흐름은 코드로 직접 확인됨.

단, 진단 서술에 사실 오류가 하나 있다: "line 564의 `!hwnd.is_invalid()` 가드가 이를 통과시킨다 ... 결과적으로 line 565의 `SetForegroundWindow(hwnd)` 분기를 건너뛰고" — 실제 코드에서 `hwnd=HWND(0)`일 때 `is_invalid()`는 false이므로 `!is_invalid()`는 true이다. 즉 `SetForegroundWindow(HWND(0))`는 **건너뛰지 않고 실제로 호출된다**. 다만 `SetForegroundWindow(NULL)`은 Win32에서 자동으로 실패(반환값 무시)하므로 실질적 효과는 없고, 그 뒤 NULL이 `TrackPopupMenuEx` owner로 전달된다는 최종 결론은 동일하게 맞다.

windows-rs `HWND::is_invalid()`가 `isize(-1)`만 invalid로 판정하고 `0(NULL)`은 유효로 통과시킨다는 핵심 전제도 windows-rs 설계상 정확하다(handle 타입들은 일반적으로 INVALID_HANDLE_VALUE인 -1만 invalid로 처리). RevWindow가 2차 요인이 아니라는 분석도 타당하다.
VERIFIER_CORRECTION: 
========================================================================
BUG UNIM-WIN-SETTINGS-KEYCAPTURE-001 | regression=True conf=high verifierOK=False
TITLE: 설정 앱 단축키 입력 필드(LineEdit) 키 캡처 불가 — nav-scope FocusScope 이벤트 가로채기
REG_SRC: commit ba6212b — feat(windows-tsf): 설정 앱 사이드바+카드 최신 디자인 재설계. nav-scope := FocusScope와 NavItem.clicked => nav-scope.focus() 패턴을 도입한 커밋.
FILES: unim-tsf-settings/ui/settings.slint, unim-tsf-settings/src/main.rs
RELATED: 
ROOT_CAUSE:
unim-tsf-settings/ui/settings.slint의 사이드바 전체가 `nav-scope := FocusScope`로 감싸여 있으며, 이 FocusScope의 `key-pressed` 핸들러가 UpArrow/DownArrow를 `accept`로 소비한다. Slint 이벤트 모델에서 FocusScope는 자신이 포커스를 보유할 때 자식 위젯보다 먼저 키 이벤트를 수신하고, `accept` 반환 시 자식(LineEdit 등)으로 이벤트가 전달되지 않는다. 더 심각한 문제는 NavItem.clicked 핸들러가 `nav-scope.focus()`를 명시 호출하여 포커스를 사이드바 FocusScope로 당기는데, 이후 우측 콘텐츠의 LineEdit를 클릭해도 nav-scope가 포커스를 포기하지 않거나(forward-focus 미설정), 포커스 상태와 무관하게 nav-scope의 key-pressed가 콘텐츠 영역 키 입력을 가로채는 구조다. 결과적으로 LineEdit(한/영 토글 키, 한자 변환 키, 트리거 키 필드)는 포커스를 받지 못하거나 키 이벤트를 수신하지 못해 편집이 불가능하다.
FIX_DIRECTION:
세 가지 선택지 중 하나를 택해야 한다.

(1) [권장] nav-scope FocusScope를 제거하고, NavItem 키보드 내비게이션을 FocusScope 없이 구현한다. 사이드바 키보드 탐색이 필요하다면 각 NavItem에 개별 FocusScope를 두거나, Slint의 focus-policy를 활용해 사이드바 FocusScope가 포커스를 독점하지 못하게 한다. nav-scope.focus() 호출도 제거한다.

(2) nav-scope FocusScope를 유지하되, key-pressed 핸들러에서 현재 포커스가 LineEdit에 있는지 확인하는 로직을 추가한다. Slint에서는 FocusScope.has-focus가 true일 때만 key-pressed가 발동하므로, LineEdit가 포커스를 받으면 nav-scope.has-focus가 false가 되어야 한다. 이를 위해 NavItem.clicked 콜백에서 nav-scope.focus() 호출을 제거하거나 조건화한다.

(3) 완전 대안: 사이드바 키보드 내비게이션 기능을 제거하고 마우스 클릭 전용으로 단순화한다(UpArrow/DownArrow key-pressed 핸들러 삭제, nav-scope.focus() 삭제).

파일: unim-tsf-settings/ui/settings.slint, lines 280-321 (nav-scope FocusScope 블록) 및 NavItem.clicked 핸들러들.
VERIFIER_CRITIQUE:
진단이 증상과 원인 커밋(ba6212b)은 올바르게 짚었으나 메커니즘 설명이 틀렸다. "nav-scope가 포커스를 포기하지 않거나 key-pressed가 콘텐츠 영역 키 입력을 가로채는 구조"라는 핵심 주장이 Slint의 실제 포커스 모델과 일치하지 않는다.

실제 Slint 동작:
1. FocusScope.key-pressed는 해당 FocusScope가 has-focus == true일 때만 발동한다. LineEdit를 마우스로 클릭하면 Slint 런타임이 포커스를 LineEdit로 이전하고 nav-scope.has-focus는 false가 된다. 이후 key-pressed는 전혀 발동하지 않으므로 LineEdit 키 입력을 "가로챈다"는 설명은 틀렸다.
2. reject는 이벤트를 부모 방향으로 버블링시킬 뿐, 사이드바와 콘텐츠 ScrollView는 HorizontalLayout 내 형제(sibling) 관계이므로 reject가 LineEdit로 이벤트를 전달하지도 않는다.
3. NavItem 내부(lines 73-115)에는 FocusScope가 없고 TouchArea만 있다. NavItem 자체가 포커스를 소유할 수 없다.

따라서 "LineEdit가 포커스를 받지 못한다"는 주장은 사용자가 LineEdit를 직접 클릭하는 경우에는 성립하지 않는다.
VERIFIER_CORRECTION: 실제 원인: NavItem.clicked 핸들러(lines 300,306,312,318)가 nav-scope.focus()를 호출하여 페이지 전환 시마다 키보드 포커스를 사이드바 FocusScope로 강제 이동시킨다. 그 결과 앱을 열거나 사이드바 항목을 클릭한 직후 포커스는 nav-scope에 있고, 키보드만 사용하는 경우(Tab 이동 등) LineEdit로 포커스가 이동하지 않아 입력이 불가능하다. 단, 사용자가 LineEdit를 마우스로 직접 클릭하면 Slint가 포커스를 LineEdit로 정상 이전하므로 편집이 가능해진다.

즉 "완전 편집 불가"가 아니라 "키보드 포커스 진입 경로 차단"이 정확한 진단이다. 마우스 클릭으로는 LineEdit가 동작하지만, 키보드 전용 워크플로우(Tab, 방향키 후 입력)에서는 nav-scope가 포커스를 독점한 채 UpArrow/DownArrow만 처리하고 나머지는 reject하므로 LineEdit에 타이핑할 수 없다. Windows TSF 설정 앱에서 키보드로 단축키 필드를 편집하려는 사용자에게 이 패턴이 치명적으로 작용한다.

근본 원인 코드: unim-tsf-settings/ui/settings.slint lines 280-321 (nav-scope FocusScope 선언) 및 lines 300,306,312,318 (nav-scope.focus() 호출). 도입 커밋: ba6212b.

수정 방향은 원본 진단의 (1)~(3) 모두 유효하다. 다만 (2)의 "FocusScope.has-focus가 false가 되어야 한다" 설명은 LineEdit 클릭 시 이미 자동으로 false가 되므로 조건 로직 추가가 아니라 단순히 NavItem.clicked에서 nav-scope.focus() 호출을 제거하는 것으로 충분하다.
########################################################################
SYNTHESIS PLAN
summary:
6개 버그 중 3개(firstchar 첫글자소실, word 깨짐, imm32 무반응 일부)는 0.3.19~0.3.22의 커밋되지 않은 working-tree 변경(브랜치 feat/windows-msi-redesign, 10743c6 위에 누적)이 만든 단일 회귀 클러스터다. 핵심 원흉은 composition.rs에 새로 추가된 LAST_EDIT_REFUSED/request_sync/take_edit_refused 학습 기계장치(정상 TSF 앱을 일시적 TS_E_SYNCHRONOUS 거부만으로 영구 CUAS로 오분류)와 ReplaceSurroundingEditSession에서 range.Collapse(TF_ANCHOR_END) 한 줄 삭제(step4 preedit replay가 commit_text를 덮어씀)다. git diff로 두 변경 모두 실재 확인(composition.rs 335번 라인 Collapse 삭제, text_service.rs take_edit_refused→GetFocus→cuas_windows 등록). 나머지 3개는 독립 버그: imm32 무반응(회귀 아님, 구조적 TSF↔IMM32 미브리지), 세벌식 '되' slash 선점(커밋 e04458d/f39ee0c, Functional 트리거에 produced_char 가드 부재 — press_key.rs:674-680 확인), 트레이메뉴(lang_bar.rs NULL HWND), 설정앱 키캡처(settings.slint ba6212b nav-scope.focus). 권장 1차 조치: composition.rs+text_service.rs+key_handler.rs의 미커밋 학습/SendInput 기계장치 revert + Collapse 한 줄 복원으로 firstchar·word 동시 해결.

root_cause_clusters:
[
 {
  "cluster": "미커밋 CUAS 학습 + range.Collapse 삭제 회귀 (tsf-composition)",
  "bug_ids": [
   "forward-typefix-first-char-deleted",
   "word-forward-typefix-garbled"
  ],
  "shared_cause": "브랜치 feat/windows-msi-redesign의 커밋되지 않은 working-tree 변경(0.3.19~0.3.22, 10743c6 위 누적)이 단일 원흉. (A) composition.rs request_sync()가 TS_E_SYNCHRONOUS(0x80040249) 같은 일시적 sync 락 거부를 LAST_EDIT_REFUSED=true로 학습 → text_service.rs OnKeyDown이 take_edit_refused()를 읽어 composition_unsupported=true로 고착시키고 GetFocus() HWND를 cuas_windows에 영구 등록(정상 TSF 앱 Word/Chrome 오분류). 그 결과 key_handler.rs가 정상 composition 대신 폴백+is_cuas SendInput(VK_BACK+UNICODE) 비동기 주입 경로로 전환돼 동기 문서모델과 오프셋 경쟁→자모 어긋남(word). (B) 같은 working-tree에서 ReplaceSurroundingEditSession::DoEditSession step3의 range.Collapse(ec,TF_ANCHOR_END) 한 줄 삭제(diff 335번 라인 확인) → step4 preedit replay의 SetText가 commit_text를 INSERT 대신 OVERWRITE → 첫 음절 소실(firstchar). move_caret_to_end는 range.Clone() 사본만 옮겨 원본 anchor 미이동이라 무력. 두 결함 모두 git diff에만 존재(정식 커밋 아님)."
 },
 {
  "cluster": "IMM32 앱 미연결 (구조적, 회귀 아님)",
  "bug_ids": [
   "imm32-apps-no-connection"
  ],
  "shared_cause": "KakaoTalk/Hancom 등 CUAS-unaware IMM32 네이티브 앱은 키를 IMM32로 소비하고 msctf ITfKeystrokeMgr에 넘기지 않아 TIP의 OnTestKeyDown/OnKeyDown이 0회 호출(로그 37/48 PID 무호출). ActivateEx·AdviseKeyEventSink는 정상. 표준 TSF만으로 해결 불가 — 별도 IMM32/HKL 경로 필요. register.rs:170 SubstituteLayout이 실제 HKL KLID가 아닌 LANGID만 등록. 회귀가 아니므로 revert 무효. 미커밋 학습 기계장치는 handle_key_down 내부라 이 앱들에선 애초 실행 안 돼 무관."
 },
 {
  "cluster": "엔진 auto_english Functional 트리거 가드 부재 (engine-keymap)",
  "bug_ids": [
   "UNIM-TSF-AUTO-ENGLISH-SLASH-CONTEXT-ALT-CONFLICT"
  ],
  "shared_cause": "src/input_engine/press_key.rs:674-680 match_auto_english_trigger의 Functional 분기가 (keycode,shift)만 비교하고 produced_char.is_some() 가드가 없음(Character 분기 681번만 보유 — 확인됨). 세벌식390 '되'(U→/→D)에서 '/'키가 slash_context_alt(ㅗ 자모 경로) 평가 전에 Functional Slash 트리거를 선점해 ㄷ확정+영문전환+'/'commit→'ㄷ/d'. 커밋 e04458d(slash_context_alt 도입)가 f39ee0c(Functional 트리거 가드 미설계)의 잠재 결함을 실체화. TSF와 무관한 코어 엔진 버그."
 },
 {
  "cluster": "lang-bar 트레이 메뉴 NULL owner",
  "bug_ids": [
   "UNIM-TSF-TRAY-MENU-001"
  ],
  "shared_cause": "lang_bar.rs show_context_menu가 GetForegroundWindow()=HWND(0)을 받는데 windows-rs HWND::is_invalid()는 HWND(-1)만 거르고 NULL은 통과시켜 SetForegroundWindow 분기를 건너뛰고 TrackPopupMenuEx에 NULL owner 전달→메뉴 미표시. 독립 버그(회귀 아님)."
 },
 {
  "cluster": "settings 앱 키보드 포커스 독점 (settings)",
  "bug_ids": [
   "UNIM-WIN-SETTINGS-KEYCAPTURE-001"
  ],
  "shared_cause": "unim-tsf-settings/ui/settings.slint NavItem.clicked(300,306,312,318)가 nav-scope.focus() 호출로 페이지 전환마다 포커스를 사이드바 FocusScope에 강제 이동→키보드 전용 워크플로우에서 LineEdit가 포커스 진입 못 함(마우스 클릭은 정상). 커밋 ba6212b 도입. 독립 버그."
 }
]

fix_groups:
[
 {
  "owner": "tsf-composition",
  "files": [
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\composition.rs",
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\text_service.rs",
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\key_handler.rs",
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\synth_input.rs",
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\auto_typefix.rs"
  ],
  "bug_ids": [
   "forward-typefix-first-char-deleted",
   "word-forward-typefix-garbled",
   "imm32-apps-no-connection"
  ],
  "plan": "세 버그 모두 동일 4-5개 파일을 건드리므로 단일 그룹/단일 담당자에게 배정(병렬 충돌 방지). (1) firstchar 최소수정 — composition.rs ReplaceSurroundingEditSession::DoEditSession의 Ok(composition) 분기에서 SetText(ec,0,&wide) 직후, move_caret_to_end 호출 앞에 `let _ = range.Collapse(ec, TF_ANCHOR_END);` 한 줄 복원(HEAD와 동일). 이래야 step4 preedit replay가 commit_text 뒤에 INSERT됨. 동시에 오진 주석(②번 정당화, 'Collapse 삭제'/'Blink 0폭 composition' 설명 블록) 제거. (2) word 깨짐 — composition.rs request_sync() 거부 판정 협소화: TS_E_SYNCHRONOUS 및 일시적 락 실패를 LAST_EDIT_REFUSED로 set하지 말 것. text_service.rs OnKeyDown의 take_edit_refused()→composition_unsupported=true + GetFocus()→cuas_windows 영구등록 경로 제거(또는 '재시도 후 반복 거부'로만 한정). composition.rs ReplaceSurroundingEditSession의 is_cuas 기반 synth_input::send_replacement SendInput 비동기 폴백을 정식 TSF 앱에서 영구 비활성화. key_handler.rs의 forward ATF 소비/replace_surrounding is_cuas 인자를 composition_unsupported와 분리. 가장 안전한 단기 조치는 LAST_EDIT_REFUSED/request_sync 거부학습 + text_service take_edit_refused 분기 + is_cuas SendInput 폴백 일체를 git checkout -p로 HEAD(10743c6 이전 거동) revert. (3) imm32 — revert로 해결 안 됨(회귀 아님). OnTestKeyDown/OnKeyDown 최상단에 무조건 진입 로깅 추가→KakaoTalk/Hancom 0회 호출 재현 확인 후, IMM32 IME 경로 또는 register.rs SubstituteLayout을 실제 HKL KLID 등록으로 교체(별도 후속작업), 단기엔 미지원 앱 목록 문서화. (1)(2)를 먼저 끝내고 빌드·메모장/Word/Chrome/wezterm 회귀 검증."
 },
 {
  "owner": "engine-keymap",
  "files": [
   "C:\\Users\\USER\\Desktop\\work\\unim\\src\\input_engine\\press_key.rs"
  ],
  "bug_ids": [
   "UNIM-TSF-AUTO-ENGLISH-SLASH-CONTEXT-ALT-CONFLICT"
  ],
  "plan": "src/input_engine/press_key.rs match_auto_english_trigger(L674~680)의 Functional 분기에 produced_char 가드 추가. produced_char는 이미 L670에서 계산됨. 제어키(Escape 등) 보호 위해 최종 조건: `*code==keycode && (shift 매칭) && (!keycode.is_character_key() || produced_char.is_some())`. KeyCode에 is_character_key() 없으면 추가하거나 produced_char 판정으로 문자키 여부 대체. 이로써 slash_context_alt 활성(초성만 채워진 상태)에서 '/'키가 Functional Slash 트리거를 무시하고 ㅗ 자모 경로를 탐. 회귀 테스트: 세벌식390 '되'(U,/,D)→'되', 평문상태 '/'→'/' fallback 또는 의도된 auto_english, Escape 트리거 정상. config.yaml의 trigger_keys '\"Slash\"'를 '\"char:/\"'로 바꾸면 즉시 우회 가능(Character 트리거는 이미 가드 보유)이나 근본수정은 코드측. 독립 파일이라 다른 그룹과 병렬 가능."
 },
 {
  "owner": "lang-bar",
  "files": [
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf\\src\\lang_bar.rs"
  ],
  "bug_ids": [
   "UNIM-TSF-TRAY-MENU-001"
  ],
  "plan": "lang_bar.rs show_context_menu에서 GetForegroundWindow() 반환을 hwnd.0!=0으로 명시 분기. NULL이면 유효 owner HWND 확보 필요(TrackPopupMenuEx는 NULL owner로 메뉴 미표시). 권장: LangBarState 구조체에 ActivateEx 시점 생성하는 가시 helper HWND(CreateWindowExW, 화면밖 1x1 popup, message-only HWND_MESSAGE는 부적합) 하나를 보관해 트레이 우클릭 owner로 항상 사용. 호출 직전 SetForegroundWindow(helper) 후 TrackPopupMenuEx(hmenu, TPM_RETURNCMD|..., x, y, helper_hwnd), 호출 직후 PostMessage(helper, WM_NULL) dismiss 보정. 대안: FindWindow(\"Shell_TrayWnd\")를 fallback owner로. 다른 그룹과 파일 비중복이라 병렬 가능."
 },
 {
  "owner": "settings",
  "files": [
   "C:\\Users\\USER\\Desktop\\work\\unim\\unim-tsf-settings\\ui\\settings.slint"
  ],
  "bug_ids": [
   "UNIM-WIN-SETTINGS-KEYCAPTURE-001"
  ],
  "plan": "unim-tsf-settings/ui/settings.slint NavItem.clicked 핸들러 4곳(L300,306,312,318)에서 nav-scope.focus() 호출 제거(권장·최소수정). 이것만으로 LineEdit 클릭/Tab 진입 시 nav-scope가 포커스 독점하지 않아 단축키 필드 편집 가능. current-page 전환 로직(root.current-page=N)은 유지. 키보드 사이드바 탐색이 꼭 필요하면 nav-scope FocusScope의 key-pressed에서 LineEdit 미포커스 시에만 Up/Down accept하도록 조건화하거나 각 NavItem 개별 FocusScope로 재설계. 회귀 검증: 앱 열고 마우스/Tab으로 한영토글키·한자키·트리거키 LineEdit에 타이핑 가능 확인. 커밋 ba6212b 패턴 수정. 파일 비중복 병렬 가능."
 }
]

revert:
[
 "[핵심] composition.rs ReplaceSurroundingEditSession::DoEditSession의 range.Collapse(ec, TF_ANCHOR_END) 삭제 hunk를 revert(한 줄 복원, HEAD 동일) — firstchar 첫글자 소실 직접 해결. diff 335번 라인.",
 "[핵심] composition.rs의 LAST_EDIT_REFUSED static + request_sync() 거부학습 + take_edit_refused() 일체를 revert(또는 request_sync를 dbg_log만 남기는 무해 버전으로 축소) — word 깨짐의 1차 원인.",
 "[핵심] text_service.rs OnKeyDown의 take_edit_refused()→composition_unsupported=true + GetFocus()→cuas_windows 영구등록 분기 revert — 정상 TSF 앱 CUAS 오분류 차단.",
 "composition.rs ReplaceSurroundingEditSession의 is_cuas 필드 기반 synth_input::send_replacement SendInput 비동기 폴백을 정식 TSF 앱에서 비활성화(또는 is_cuas 경로 revert).",
 "key_handler.rs의 !composition_unsupported 게이트(forward ATF 소비/replace_surrounding is_cuas 인자)를 revert해 10743c6 이전 거동 복귀.",
 "②번 정당화 오진 주석(composition.rs 'Blink 0폭 composition 해석' 및 'Collapse 삭제' 설명 블록) 제거.",
 "주의: imm32/lang-bar/engine-keymap/settings 4개 버그는 revert로 해결되지 않음(전자는 구조적, 후 3개는 정식 커밋 e04458d/f39ee0c/ba6212b 또는 기존 코드)."
]

priority: ["P0 tsf-composition: firstchar(Collapse 한 줄 복원) — 메모장/Chrome/wezterm 전 앱 광역 회귀, 1줄 최소수정, 즉시", "P0 tsf-composition: word 깨짐(LAST_EDIT_REFUSED/take_edit_refused/is_cuas SendInput revert) — Word 등 정식 TSF 앱 입력 파괴, firstchar와 동일 파일군이라 함께 처리", "P1 engine-keymap: 세벌식 '되' Functional 트리거 가드 추가 — 세벌식 사용자 입력 오작동, 독립 파일 병렬 가능", "P1 lang-bar: 트레이 메뉴 NULL owner 수정 — UX 차단, 독립 파일 병렬", "P2 settings: nav-scope.focus 제거 — 키보드 워크플로우 한정 영향, 마우스 우회 존재, 독립 파일 병렬", "P2 tsf-composition(후속): imm32 IMM32/HKL 경로 — 구조적 대공사, 단기엔 진입로깅+미지원 문서화"]

risks:
[
 "tsf-composition 그룹이 3개 버그 + 4-5파일을 단독 점유 — 반드시 한 담당자가 순차 작업해야 함(병렬 분할 시 request_sync/composition_unsupported/is_cuas 상호참조로 충돌). revert와 신규 fix가 같은 hunk에 얽혀 git checkout -p 정밀 적용 필요.",
 "request_sync/LAST_EDIT_REFUSED 전면 revert 시 wezterm/Telegram 등 진짜 CUAS 터미널의 inline 조합 폴백(메모리 노트의 오버레이 전략)이 함께 약화될 수 있음 — CUAS 학습을 OnCompositionTerminated 시간기반으로만 남기는지 확인 후 revert.",
 "Collapse 복원이 a2051b4(첫글자 미삭제 수정)와 충돌하지 않는지 검증 필요 — 진단상 별개 delete_chars -1 문제로 이미 해결됐으나 동일 함수라 회귀 재현 테스트 필수.",
 "engine-keymap 수정의 is_character_key() 가드 누락 시 Escape/Tab 등 제어키 Functional 트리거가 차단되는 부작용 — 비문자키 우회 조건 반드시 포함.",
 "lang-bar helper HWND를 message-only(HWND_MESSAGE)로 만들면 TrackPopupMenuEx dismiss가 깨짐 — 가시 창이어야 함.",
 "imm32는 표준 TSF 한계라 어떤 수정도 부분적 — 사용자 기대 관리 위해 미지원 앱 목록 문서화 동반.",
 "미커밋 변경 revert는 working-tree만 영향(정식 커밋 아님)이라 git stash/checkout -p로 안전하나, 같은 파일의 유지해야 할 변경(popup_window.rs 삭제, lib.rs 등 MSI 재설계분)과 섞이지 않도록 hunk 단위 선별 필수."
]
```
