# UNIM Windows 입력기 — 지식 상태 맵 (통합)

> 작성일 2026-06-19 · 6개 도메인 분석(기본입력 / 3팝업 렌더러 / AutoTypeFix / IMM32 .ime / CUAS 터미널 inline / 입력표시기·언어바) 통합.
> SoT(Single source of truth)는 라이브 코드. 본 문서는 분석 산출물의 합의·모순·잔여 미해결을 한 곳에 모은 횡단 지도다.
> **라이브 코드 재검증(2026-06-19)으로 일부 도메인 스냅샷이 stale 함을 확인** — 본문 §5에 명시.

---

## 1. 핵심 합의 (established, 도메인 횡단 통합)

### 1.1 "TSF-IMM32 브리지"의 정체 = CUAS (전 도메인 합의)
- 사용자가 들은 "브리지"는 **CUAS**(Cicero Unaware Application Support). msctf.dll 내장 default text store + Msimtf.dll context owner. **OS 자동 에뮬레이션 레이어이며 서드파티가 켤 수 있는 API는 없다.**
- CUAS는 TIP의 표준 TSF composition을 IMM32 메시지(WM_IME_STARTCOMPOSITION / WM_IME_COMPOSITION GCS_COMPSTR=미확정·GCS_RESULTSTR=확정 / WM_IME_ENDCOMPOSITION)로 역변환한다.
- **AIMM(IActiveIMMApp)은 브리지가 아니다** — Win2000+ 비활성, 방향 역전(앱 주도), HIMC 스레드-로컬. (기각, §3)

### 1.2 CUAS 미확정/확정 판별 규칙 (Mozc 1차 소스 + SampleIME 교차검증)
- composition range '안'에 남고 display/interim attribute 보유 = **GCS_COMPSTR**(미확정·밑줄).
- range '밖'으로 밀리거나 attribute 없거나 EndComposition = **GCS_RESULTSTR**(확정).
- **빈(zero-length) range는 CUAS가 즉시 terminate** 한다. composition은 활성 중 항상 non-empty 텍스트 유지 필수.
- CUAS는 GUID_PROP_READING 세그먼트 구조로 GCS_RESULTCLAUSE/READCLAUSE를 만든다(절 구조 계산용이지 composition 생존 요인은 아님).

### 1.3 즉시-종료(immediate OnCompositionTerminated) 근본원인과 결정적 수정
- **근본원인 = CUAS owner-side 종료**. MS 공식상 OnCompositionTerminated는 'TIP 외 주체(=owner=CUAS)가 composition을 끝낼 때만' 호출.
- **결정적 수정 = TF_SELECTIONSTYLE.fInterimChar = TRUE**. 조합 중 selection에 set(commit 시 FALSE). 라이브 코드 확정: `composition.rs:157 fInterimChar: BOOL(1)`, `:128 BOOL(0)`.
  - MS Learn TF_SELECTIONSTYLE Remarks가 한국어 조합을 명시("interim character ... solid rectangle ... Korean").
  - 3개 독립 오픈소스 한국어 TSF IME(NavilIME / saenaru / kolemak) 공통 패턴.
- 완성 메커니즘 5요소(전부 구현됨): ① fInterimChar=TRUE ② non-empty range ③ 단일 edit session(StartComposition→SetText 비분할) ④ commit_and_restart로 음절 전환 병합(`composition.rs:361`) ⑤ Enter/방향키 OnTestKeyDown 확정→pIsEaten=FALSE 통과(NavilIME 패턴).

### 1.4 한/영 상태 OS 동기화 (입력표시기 도메인 + 기본입력 도메인 합의)
- 두 thread-manager compartment를 `ITfCompartment::SetValue(VT_I4)`로 set:
  - `GUID_COMPARTMENT_KEYBOARD_OPENCLOSE` (0 아니면 keyboard open)
  - `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION` (`TF_CONVERSIONMODE_NATIVE=0x0001`=한글 / ALPHANUMERIC=0x0000=영문)
- thread manager 스코프 = 같은 스레드 전 TIP·OS 표시기 공유 단일 상태. Deactivate 시 OPENCLOSE는 clear 금지(_ClearCompartment가 S_FALSE).
- UNIM은 `compartment.rs`에 시퀀스 구현.

### 1.5 팝업 렌더러 = out-of-process (구현 완료, 코드 검증)
- 최종 채택 = 별도 `unim-popup-win.exe` 싱글턴 상주 프로세스(Mozc/Weasel 노선). in-proc `popup_window.rs`(649줄)는 **git rm 완료**(코드 검증: 파일 부재).
- 팝업 위치 = 무조건 화면 중앙(rcWork) — 캐럿 추적 전면 폐기. Linux v3.3 정책 일치. H1·H11 정의상 소멸.
- 격자 = column-major `idx=col*rows+row`(엔진 SoT). 와이어 셀은 column-major 확정 배열 → 전치(H3) 불가. 셀 플래그 Linux와 비트 동일.
- IPC = named pipe `\\.\pipe\unim-popup-win.<session_id>`, JSON 라인, WIRE_VERSION=1, DUPLEX(정방향 render + 역방향 마우스 evt). 와이어 타입은 popup_ipc.rs ↔ protocol.rs **의도적 동일 사본**(JSON이 계약).
- 엔진 view model(`engine.popup_state()` + `PopupState::view_model()`) 그대로 직렬화 — TSF는 raw action 재조립 안 함.
- Phase 2(마우스+컬러 이모지) 코드 랜딩(`d2d.rs` 존재).

### 1.6 AutoTypeFix (ATF) — key_handler 연결 완료
- Linux `engine_worker.rs:679-900`를 TSF 단일 인스턴스로 포팅. `Mutex<AutoTypeFixState>`. 진입점: process_after_key / try_undo / observe_backspace / observe_mode_switch.
- **key_handler 연결 완료** (PARITY_PLAN의 '미연결'은 stale). 호출부 3곳: 수동(L273)·Ctrl+Z(L287)·순/역방향(L454). popup 비활성 && composition_unsupported 아님일 때만.
- surrounding 교체 = `ReplaceSurroundingEditSession` ShiftStart(-N) → composition-wrap(StartComposition+SetText+EndComposition, reconversion 패턴). raw SetText 폴백.
- 순방향(영→한) 첫글자 소실 방지 핵심 = step3 SetText 직후 `range.Collapse(TF_ANCHOR_END)`(composition.rs L707/L717 복원됨). delete_chars=입력 영문 전체 N(과거 -1 제거).
- 역방향(한→영): committed만 delete + 조합음절은 end_composition으로 별도 제거(과거 -1 재계산 폐기).
- 매 SetText 후 READING/ATTRIBUTE 재부여 필수(SetText가 property discard).

### 1.7 IMM32 .ime 경로 — 크레이트·등록 구현 완료
- `unim-imm32/` 크레이트 존재, `unim_imm32.def`가 IMM IME DDI 전체 export(ImeInquire/ImeProcessKey/ImeToAsciiEx/ImeSetCompositionString 등).
- BLOCKER A 해소: per-user 활성화 = HKCU Preload(KLID E0200412) + `LoadKeyboardLayoutW(KLF_ACTIVATE|KLF_SUBSTITUTE_OK)`. `unim-windows-common/src/activation.rs`.
- BLOCKER B 해소: Layout File = **KBDKOR.DLL**(과거 KBDA1.DLL=아랍어 버그 수정). `unim.wxs:268,292`.
- KakaoTalk/한컴(32-bit IMM32) 폴백은 Preload 활성화만으로 충분. CTF\Assemblies·코드서명 불요(그 경로엔).

### 1.8 입력표시기·언어바·아이콘
- 트레이 '가/A' 1차 소스 = langbar item(`ITfLangBarItemButton`) GetText/GetIcon, dwStyle에 `TF_LBI_STYLE_BTN_BUTTON | SHOWNINTRAY` 필요(SampleIME·Weasel 교차검증).
- GetIcon 규약: NULL HICON이면 반드시 **E_FAIL** 반환해야 트레이가 그린다.
- IME 선택기/Win+Space 아이콘 = LanguageProfile의 IconFile+IconIndex(레지스트리 정적). 모드 무관 정적 — '영어 모드에도 한글 아이콘'은 **버그 아님**(TSF 구조상 정상).
- IconIndex 음수 = 절댓값을 리소스 ID 해석(ExtractIconEx). winresource로 RT_GROUP_ICON id 1 임베드.
- ITfTextInputProcessorEx 구현 시 ActivateEx 호출(Activate 미호출). UILess 스레드(게임/전체화면) 활성화에 필수.
- Win8+ Input Indicator는 'compatible IME'만 모드 아이콘 표시, 비호환은 언어 약어(ENG/KOR)만 — **의도된 동작이지 MS 자사 전용 차별 아님**(§3).

---

## 2. 앱 유형 × 기능 커버리지 매트릭스

범례: ✅ 코드+의미론상 동작 · ⚠ 부분/조건부/추론(VM 미검증) · ❌ 미해결/구조적 미브리지 · — 해당없음

| 앱 유형 | 기본 입력(조합) | 한자/특수문자/이모지 3팝업 | 자동교정(ATF) |
|---|---|---|---|
| **① TSF-aware** (Chrome/Edge·리치컨트롤·Win11 메모장·Windows Terminal v1.24+) | ✅ 네이티브 inline, 처음부터 정상 | ⚠ 렌더러 분리로 표시됨. UILess 강제 호스트는 ITfCandidateListUIElement 미구현(H2) → ⚠ | ✅ composition-wrap 경로(Chrome/Blink raw SetText 무시 대응 완료). VM 미검증 |
| **② IMM32-only/CUAS** (EDIT·wezterm·alacritty·Telegram) | ⚠ fInterimChar+non-empty range+단일세션으로 inline **추론상 가능**, VM 미검증(P0). 실패 시 오버레이 폴백 | ⚠ 렌더러 중앙 표시(앱 무관). VM 미검증 | ⚠ composition_unsupported 폴백(replace_surrounding 직접삽입). fallback_pending 위치 어긋남 리스크 |
| **③ 진짜 콘솔** (classic conhost cmd/PowerShell) | ❌ CUAS 제외 대상. 별도 콘솔 IME 경로 미검증. 방향키 순서역전('한글이'+Home→'이한글') | ⚠ 렌더러 표시 추정. GetWnd NULL HWND 가능 | ❌ 폴백 경로 미검증 |
| **④ IMM32 자체렌더 없음** (게임 채팅) | ⚠ IME 기본 조합창이 좌상단 미위치로 그려짐 | ⚠ 렌더러 중앙(게임 오버레이/Z-order 미검증) | ⚠ 미검증 |
| **⑤ 순수 IMM32 네이티브** (KakaoTalk·한컴) | ⚠ TSF TIP은 OnKeyDown 0회 호출 → .ime 경로 필요. .ime 크레이트 구현·등록 완료, **활성화/표시명/실앱 로드 VM 미검증** | ⚠ .ime ui_window 경로(미검증) | ❌ **순방향 ATF 무반응** — TIP에 키 안 옴. .ime 경로 + send_replacement 재배선 필요(현재 dead code) |

**요약 한 줄**: TSF-aware 기본입력만 확정 ✅. 나머지 전부는 "코드 결함은 확정·수정 완료, 실앱 동작은 Windows VM 미검증(⚠)"이며, **순수 IMM32 네이티브 앱의 순방향 ATF만 구조적 ❌**(revert 불가, .ime 브리지 신규 작업 필요).

---

## 3. 기각된 가설 (refuted, 중복 제거 후 통합)

> 여러 도메인이 동일 가설을 독립 기각 — 가장 강한 근거로 1회만 기록.

| 기각 가설 | 기각 근거(요지) |
|---|---|
| **ITfContextOwnerCompositionSink 미구현이 즉시종료 P1 원인** (최유력 의심이었음) | 그 인터페이스는 앱/context-owner(레거시는 CUAS 자신)가 구현하는 owner측 sink로 composition을 '거부'하는 쪽. SampleIME·Mozc·UNIM 셋 다 ITfCompositionSink만 구현 → UNIM은 누락 없음. TIP이 구현해봤자 자기 composition 자기 승인(무의미). **수정 착수 금지.** |
| **0폭 caret collapse(SetSelection)가 즉시종료 유발** | 실측 기각. TF_AE_NONE+range 전체로 바꿔도 즉시종료 100% 동일. SampleIME도 Collapse(END) 쓰며 정상. (단 fInterimChar 위해 TF_AE_NONE 채택은 유지) |
| **display attribute(GUID_PROP_ATTRIBUTE) 누락이 즉시종료 원인** | SetValue FAILED/GetValue MISMATCH 0건. GUID_PROP_COMPOSING은 TSF 자동 적용이라 생존과 독립. 단 '밑줄 미표시' 증상엔 정당한 보강. |
| **GUID_PROP_READING이 composition 생존 핵심** | UNIM만 set. NavilIME·kolemak은 GUID_PROP_* 0건으로도 정상. 생존 요인 아님 → A/B 제거 후보. (단 Mozc commit 시 RESULTCLAUSE 세그먼트 보강용으로 잔존 가능) |
| **MS 한국어 IME inline은 .ime 하이브리드/OS 특권** | 테스트 PC 레지스트리 .ime 0개. MS imekr은 순수 TSF TIP. Mozc도 .ime 0개로 CUAS inline 달성. |
| **순수 TSF TIP은 레거시/콘솔 inline 원천 불가(터미널은 조합 불가)** | over-generalization. '빈 range 죽음'을 '조합 불가'로 비약. non-empty range는 생존. 사용자 밀어붙여 실동작 확인. feasibility 문서 stale 처리. |
| **AIMM(IActiveIMMApp)으로 wezterm inline 주입** | Win2000+ 비활성, 방향 역전, HIMC 스레드-로컬. TIP이 타 프로세스 IMC에 push 불가. |
| **TIP이 ImmSetCompositionStringW/WM_IME_COMPOSITION 직접 주입** | ImmSetCompositionStringW는 현재 HKL의 IME DLL에 위임. TSF 프로파일 활성 시 HKL=TSF프로파일 → 이미 terminate 중인 CUAS 브리지로 재진입. 생산 선례 0. |
| **TSF Transitory Extension으로 wezterm inline** | compartment는 document-manager(앱/CUAS) 소유, TIP이 켤 위치 아님. 싱크는 앱이 구현(wezterm 미구현). Chromium도 full text store 채택. |
| **단일 DLL 하이브리드(TIP+.ime)가 레거시 inline에 순이득** | 한 프로파일이 TIP·.ime 동시 활성 불가. inline 이득은 'TIP 포기하고 IMM32-only로 폴백'한 효과. Win8+ 신규 .ime 차단. Mozc 폐기. |
| **Word ATF 깨짐('서기현 woody'→'ntkd기 ㄹㅊdy')은 영문 forward 게이트 raw키 누출** | 한국어 모드 부적용. OnTestKeyDown=FALSE면 OnKeyDown 미호출. 실제 원인=미커밋 CUAS-오분류 기계장치(request_sync TS_E_SYNCHRONOUS 일시거부를 영구 LAST_EDIT_REFUSED 오인 → Word를 CUAS 오분류 → SendInput 비동기 폴백이 동기 문서모델과 오프셋 경쟁). **revert 완료.** |
| **ATF CUAS 폴백으로 synth_input::send_replacement(SendInput) 사용** | 위 R3 word-corruption 버그의 일부. 현재 호출부 0개 dead code(#[allow(dead_code)]). IMM32/HKL 후속용 보존만. |
| **순방향 ATF 첫글자 소실은 Blink가 0폭 composition을 빈 확정으로 해석 → step3 Collapse 제거해야** | 오진. 실제는 순수 ITfRange/SetText 의미론(앱무관). Collapse(END) 제거 시 step4 SetText가 commit_text를 OVERWRITE. 메모장/Blink/CUAS 동일. |
| **역방향 삭제수 = kor_sim.chars().count()-1 재계산** | 옛 composition 모델 가정. commit_and_restart 모델에서 깨져 'hello'→'녀o' 잔류. 정답=committed만 delete + 조합음절 end_composition. |
| **was_composing=true면 ATF 차단** | 역방향 원천 봉쇄 버그(한글은 둘째 키부터 조합 중). Linux engine_worker에도 게이트 없음. 실제 차단=popup 활성·모드 변경. |
| **in-proc HWND 강화(A노선/DIME)로 팝업 종결** | B(프로세스 분리) 최종 채택, popup_window.rs git rm. A노선은 AppContainer IL/UIPI/게임 Z-order 한계 잔존. |
| **PopupRenderPayload를 와이어타입 직접 재사용/serde 추가** | serde derive 없고 zbus 튜플 지향. Linux 크레이트 변경=코어 무수정 위반·의존 전파. 양 크레이트 동일 사본 채택. |
| **첫 팝업 3×3 1페이지는 렌더러 Default 격자 버그** | 엔진 단일 슬롯이 Show*만 발행해 PopupState::default() 노출. 신설계는 첫 render부터 update_page_layout() 결과 포함 → 'Default 격자' 개념 소멸. |
| **구 렌더러 PostQuitMessage가 호스트 메시지루프 오염(H10)** | 프로세스 분리로 원천 차단. §8.2에 금지 게이트 명문화. |
| **Win11이 third-party TSF IME를 트레이서 명시 차단(MS 자사 전용)** | 기준은 '제작사' 아닌 'Input Indicator 호환 구현 여부'. compatible이면 third-party도 표시. 차별 정책 1차 문서 미발견. |
| **트레이 미표시 1차 원인=OnUpdate 누락/compartment 미advise** | 라이브 소스 정정: lang_bar.rs L58이 OnUpdate(STATUS\|ICON\|TEXT) 이미 호출. (※ §5: GetIcon도 이미 수정됨) |
| **모드 아이콘 동적표시에 ITfTextInputProcessorEx 외 별도 UI-less 인터페이스 필요** | SampleIME은 langbar item만 사용. 차이는 인터페이스 아닌 compartment 배선/GetIcon 규약. |
| **'영어 모드에도 선택기 아이콘 한글'이 버그** | 선택기 아이콘은 프로파일 정적값. MS 한국어 IME도 동일. 모드별 변화는 트레이 GetIcon만. |

---

## 4. 미해결 (open, 횡단 통합 — 중복 병합)

> **거의 전부의 공통분모 = Windows VM 런타임 미검증.** Linux 크로스컴파일은 sanity 전용(실행·테스트 불가).

### O1. [P0·VM] 터미널/메신저 한글 inline 실동작 — 도메인 1·5·6 공통 최상위 미해결
fInterimChar+non-empty range+단일세션+READING이 wezterm/Telegram에서 OnCompositionTerminated:IMMEDIATE를 실제로 멈추고 inline preedit를 내는지 미검증. P1(우리가 놓친 것) 적용 후에도 안 되면 P2(MS 호스트측 IMM32↔CUAS opt-in 특권) 잔존 가능성. **무빌드 최고정보 실험 = Spy++로 MS IME vs UNIM의 WM_IME_* diff(미수행).**

### O2. [P0·VM] 팝업 3종 실앱 렌더 검증
전치/하이라이트/위치점프/3×3 코드결함은 확정·수정. 메모장/Chrome/UWP/게임/터미널에서 위치·격자·선택·커밋이 Linux 동등인지 미검증. 검증 매트릭스(가짜 클라이언트 파이프 JSON write 골든) 설계됨, 수동 실행 대기.

### O3. [P1·코드] UILess 강제 호스트 대응 ITfCandidateListUIElement 미구현 (H2)
unim-tsf 전체에 UIElement/CandidateList refs **0건**(라이브 재확인). 렌더러 분리로 표시 자체는 대개 되나, unim.wxs가 UIELEMENTENABLED/IMMERSIVESUPPORT를 '지원' 등록만 한 **선언-이행 불일치**. UWP/immersive 검색창에서 규약위반 차단 가능.

### O4. [P1·코드/VM] 순수 IMM32 네이티브(KakaoTalk/한컴) 순방향 ATF 무반응 — 유일한 구조적 ❌
TIP의 OnTestKeyDown/OnKeyDown 0회 호출. 표준 TSF로 해결 불가, .ime/HKL 경로 필요. dead code send_replacement가 재사용 대상. `register.rs:170 SubstituteLayout`이 실제 KLID 아닌 LANGID만 등록하는 결함 관련.

### O5. [P1·VM] composition_unsupported 폴백 fallback_pending 위치 어긋남
백스페이스/방향키/마우스로 커서 이동 시 pending 카운트와 실제 위치 어긋남(커서이동 감지 리셋 필요). 200ms 임계값 휴리스틱이 느린/원격 시스템 오판 가능(per-HWND 학습이 완화하나 제거 못 함). ATF가 이 경로 공유 → 직접 영향.

### O6. [P1·VM] 네비게이션 키 확정↔통과 순서 호스트별 미검증
conhost '한글이'+Home→'이한글'(interim char 커서와 함께 확정). conhost는 OnTestKeyDown 게이팅 없이 OnKeyDown 직접 호출 → NavilIME 패턴 미작동. 호스트별 반복 실측 필요(단일 코드경로 아님).

### O7. [P1·코드] TF_ES_SYNC → TF_ES_ASYNCDONTCARE 전환(Phase 2) 보류
6곳 모두 TF_ES_READWRITE|TF_ES_SYNC(composition.rs L288/316/331/347/390/402/435). MS 규칙상 Word 등은 동기 세션 grant 안 함. '6곳 일괄 치환'은 오처방 판정 — composition_slot 동기 take(L239) 재설계 필수, 별도 PR. Word 회귀 잠재 위험.

### O8. [P2·VM] .ime 표시명/활성화 영속성
.ime PE에 string resource -1/-1000 미확인(빈 표시명 우려). LoadKeyboardLayoutW(세션/스레드 스코프) vs Preload(영속) 상호작용 — 신규 로그인 시 Preload KLID 인식 / 세션 중 broadcast 전파 미검증.

### O9. [P2·코드] CTF\Assemblies 듀얼모드 배선 채택 여부
현재 install-path 코드에 Assemblies/Substitutes write 없음(진단 .ps1만). IMM32-only 폴백은 이것 없이 완전. 단일 언어바 항목 듀얼모드(modern=TSF/legacy=IMM32)에만 필요. 출하 여부 보류.

### O10. [P2·코드] unim-popup-wire(serde-only) 크레이트 미생성
와이어 타입 ~200줄이 popup_ipc.rs ↔ protocol.rs 의도적 중복 사본(라이브 확인: 부재). 후속 PR 단일화. 스키마 변경 시 양쪽 동시수정 부담.

### O11. [P2·VM] Phase 2 마우스 역채널·LL훅 수명
owner_hwnd+seq 불일치 역이벤트 무시, 표시 중에만 LL훅 설치·hide시 해제 규칙 미실측. 훅 누수 시 전역 마우스 지연 위험.

### O12. [P2·VM/코드] H6/H7(엔진 reset 시 팝업 미정리·팝업 중 비팝업 키 desync), 코드서명 부재
프로세스 분리와 직교, 엔진·key_handler 과제. 코드서명: signtool 부재(.ime Security Directory RVA=0). LOW 등급(데스크톱 대개 로드, UWP/protected/AV/기업정책만 거부).

---

## 5. 라이브 코드 재검증 — 도메인 스냅샷 정정 (2026-06-19)

도메인 분석 일부가 stale 스냅샷 기반임을 확인. **코드가 분석보다 앞서 있음:**

| 도메인 주장 | 라이브 코드 현실 | 영향 |
|---|---|---|
| 도메인6: "lang_bar.rs GetIcon이 null HICON 스텁(Ok(HICON::default()) L213-215) 반환 = 트레이 미표시 근본원인" | **이미 수정됨.** GetIcon은 L562-574에서 유효 HICON이면 S_OK, NULL이면 `Err(E_FAIL)` 반환(SampleIME 규약 준수). L328/333의 HICON::default는 WNDCLASS이지 GetIcon 아님. | 트레이 미표시 **코드 원인 해소됨**. 잔여는 아이콘 리소스 임베드+VM 표시 검증. |
| 도메인6: "unim-tsf에 build.rs 부재 → DLL 아이콘 0개 → ko-KR 폴백" | **build.rs 존재.** `embed_resource::compile("unim.rc")`로 RT_GROUP_ICON id 1 임베드(IconIndex=0). | 선택기 아이콘 **코드상 해소**. unim.rc/unim.ico 실재·rc.exe PATH·VM 표시만 검증 필요. |

→ **입력표시기 도메인의 '확정 근본원인'(GetIcon null·build.rs 부재)은 이미 코드 수정 완료.** 해당 미해결은 O1/O2급 VM 표시 검증으로 강등. _RESEARCH_GAPS.md에 반영.

---

## 6. docs/dev/windows/ 46개 정리 계획

분류 기준: **keep**=현행 SoT/계획/체크리스트로 유효 · **merge**=내용 유효하나 산재, 통합처로 흡수 · **archive**=기각가설 전용/세대중복/stale.

### KEEP (12) — 현행 유지
| 파일 | 근거 |
|---|---|
| `RETROSPECTIVE-tsf-terminal-inline.md` | inline 최종 verdict·오진 회고. 의사결정 SoT. |
| `popup-renderer-design.md` | 팝업 out-of-proc 최종 설계 SoT(구현 반영). |
| `PARITY_PLAN.md` | Linux 동등화 마스터 플랜(단 §41-42 ATF '미연결' stale 주석 정정 필요). |
| `SMOKE_TEST.md` | VM 스모크 체크리스트. O1~O12 검증 입력. |
| `impl-reading-singlesession-plan.md` | 단일세션+READING 구현 계획(적용완료, 검증대기 항목 유효). |
| `client-side-preedit-plan.md` | 오버레이 폴백 설계(공존 안전망). |
| `windows-console-composition-bug.md` | 폴백/conhost 잔여 리스크 TODO. |
| `tsf-bugs-r3-diagnosis.md` | R3 회귀(firstchar/word) 진단+revert 근거. |
| `TSF_INPUT_FIX_PLAN.md` | 입력표시기 수정 계획(GetIcon/아이콘/compartment, 단 §5 정정 반영). |
| `windows-ime-selector-icon-research.md` | 선택기 아이콘 등록 규약 1차 추출. |
| `repro-matrix-p1.md` | P1 실측 절차(Spy++ diff 입력). 짧지만 O1 실행 절차. |
| `MSI_DIAGNOSIS_TEMPLATE.md` | 진단 양식 템플릿(재사용). |

### MERGE (18) — 통합 후 원본 archive
- **통합처 A: `research-tsf-imm32-bridge-SYNTHESIS.md`**(브리지/CUAS 최종 종합 — KEEP 승격)으로 흡수:
  `bridge-cuas-internals.md`, `bridge-active-imm-aimm.md`, `bridge-direct-imm-from-tip.md`, `bridge-transitory-extension.md`, `bridge-tsf-missing-interfaces.md`, `bridge-hybrid-imm-registration.md`, `bridge-terminal-side.md`, `bridge-korean-imes-console.md`, `research-cuas-bridge-terminate.md`, `research-SYNTHESIS-cuas-inline.md` (10개 — bridge-* 각도별 조사는 SYNTHESIS가 결론 흡수).
- **통합처 B: `imm32-diagnosis-report.md`**(IMM32 진단 결론 — KEEP 승격)으로 흡수:
  `imm32-diagnosis-evidence.md`, `imm32-tsf-integration.md`, `unim-tsf-fix.md` (3개).
- **통합처 C: `research-korean-tsf-imes.md`**(한국어 TSF IME 교차감사 — KEEP 승격)으로 흡수:
  `research-wezterm-ime.md`, `research-unim-composition-audit.md` (2개).
- **통합처 D: `popup-renderer-design.md`(KEEP)** 로 흡수:
  `popup-tsf-fix-plan.md`, `windows-common-assessment.md`, `windows-common-design.md` (3개 — 팝업 H-진단·common 평가는 최종 설계서에 결론 반영됨).

### ARCHIVE (16) — `docs/dev/windows/_archive/`로 이동
| 파일 | 근거 |
|---|---|
| `research-tsf-imm32-inline-feasibility.md` | **stale 명시**('IMM32 inline 금지·overlay만'이 SYNTHESIS로 번복). |
| `wezterm-composition-research.md` (1차) | 3rd가 대체. 세대 중복. caret 가설(기각) 중심. |
| `wezterm-composition-research-3rd.md` | 결론이 RETROSPECTIVE로 승계. (또는 KEEP 1택 — RETROSPECTIVE 유지 시 archive) |
| `wezterm-cuas-composition-plan.md` | 계획 단계, 구현·결론이 후속에 반영. |
| `research-cuas-bridge-terminate.md`* | (MERGE A 흡수 후 archive) |
| `TSF_CONSOLE_COMPAT_RESEARCH.md` | ActivateEx dwflags=무죄 결론만, P4 완료. conhost 미해결은 KEEP 문서가 보유. |
| `RESEARCH_A_WIN11_INDICATOR_SETTINGS.md` | ShowStatus 커뮤니티 출처, 결론은 FIX_PLAN/GAPS로. |
| `RESEARCH_B_WIN11_TSF_REGRESSIONS.md` | OS 회귀 배경, 결론 흡수됨. |
| `RESEARCH_C_MSIME_VS_UNIM.md` | langbar 정적추정(라이브 소스로 정정됨). |
| `TSF_INPUT_INDICATOR_RESEARCH.md` | 정적 추정 1순위 의심이 라이브로 정정. FIX_PLAN이 대체. |
| `TSF_RESEARCH_REDESIGN.md` | 카테고리 8종 등 결론이 진단/FIX_PLAN에 반영. |
| `tsf-app-connection-modes.md`* | (5범주 분류는 본 _KNOWLEDGE_STATE §2 매트릭스로 승계) |
| `windows-settings-exe-plan.md` | 설정 exe 분리 — 본 6도메인과 직교, 별도 트랙. 현 작업 무관(보존만). |
| `TSF_OFFICIAL_REFERENCE.md` | 레퍼런스 인용집 — 유효 인용은 KEEP 문서에 산재 인라인. (보존 가치 있으면 KEEP 가능) |
| `bridge-*` 잔여 | (MERGE A 흡수 후 archive) |

> *주: MERGE 대상 원본은 통합처에 결론 흡수 후 _archive로 이동(이력 보존). **세대 중복(1차 vs 3차 wezterm-composition-research)과 기각가설 전용 문서(bridge-active-imm/transitory/direct-imm)가 archive 1순위.**
> 정리 후 활성 문서: KEEP 12 + 통합처 승격 4(SYNTHESIS/imm32-diagnosis-report/research-korean-tsf-imes/popup-renderer-design) = **약 16개**, 나머지 ~30개는 _archive.
