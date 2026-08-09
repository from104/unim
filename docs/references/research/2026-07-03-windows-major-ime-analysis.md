# UNIM Windows 메이저 한글 입력기 종합 분석 보고서

- 작성일: 2026-07-03
- 대상: UNIM (Rust 한글 IME) — Windows 프런트엔드 `unim-tsf`(TSF TIP DLL) + 공유 코어 `src/`
- 범위: 소스·아키텍처 / 입력 품질·앱 호환성 / 경쟁 IME 기능 격차 / UI·UX 미려함 / 접근성(장애인) / 안정성·배포·신뢰
- 방법: 11개 축의 파일:라인 대조 검증 분석을 종합. 본 보고서 작성 시 핵심 근거(`register.rs:147 UNIM_DEBUG_LOG=true`, `Cargo.toml:66-70 panic=abort`, `key_handler.rs:491 .expect()`, `composition.rs:157 fInterimChar=BOOL(1)`, UIA/UIElement grep=0, `hanja.txt` 6,756,043B, 5개 핵심 파일 라인수)를 실코드로 재확인함.

---

## 1. Executive Summary

### 1.1 현 위치 한 문단 진단

UNIM은 "코어 입력 엔진과 TSF 통합 노하우는 이미 경쟁력에 도달했으나, 제품으로서의 신뢰·배포·접근성 파이프라인이 통째로 비어 있어 '메이저 IME 인식'의 문턱을 넘지 못한" 상태다. 조합/자판 코어는 preedit·word_buffer 이중 display API, 키스트로크-리플레이 backspace, password 상태머신, toggle-key 소비 노출 등으로 메이저급 견고성을 갖췄고, TSF 프런트엔드는 7개 sink/provider를 완비하고 CUAS 브리지 즉시-terminate 회피(`fInterimChar=BOOL(1)`), 동적 CUAS 감지(앱 이름이 아닌 ShiftStart 이동량), out-of-process 팝업 자가복구, 32/64비트 이중 배포까지 실현했다. 경쟁 IME 어디에도 없는 실시간 양방향 한/영 오타 자동교정(AutoTypeFix)이라는 고유 무기도 보유한다. 그러나 (1) 코드 서명이 전무해 SmartScreen 관문에서 설치가 막히고, (2) 설치 후 사용자가 스스로 입력기를 활성화해야 하며(닭-달걀 온보딩), (3) 릴리스 빌드에 진단 로그가 켜진 채 배포돼 매 키 입력이 평문으로 임시파일에 누적되고, (4) 스크린리더·돋보기 사용자가 한자·이모지 후보 기능에서 완전히 배제되며, (5) 확정 후 한자 재변환·단어 단위 변환·범용 사용자 사전이 없어 MS IME·날개셋 대비 결정적 격차가 남는다. 요약: **"코드는 견고하나 제품 배포·신뢰·접근성이 미완"**.

### 1.2 메이저화까지 핵심 갭 Top 5

| # | 갭 | 성격 | 근거 | 우선순위 |
|---|-----|------|------|----------|
| 1 | **코드 서명 전무** → SmartScreen '알 수 없는 게시자' 경고·기업배포 차단 | 채택 관문(blocker) | CI·스크립트·installer 전역 signtool 0건; `light -sval`; `installer/README.md:120` 자인 | **P0** |
| 2 | **릴리스 진단 로그 상시 ON** → 매 키 vk 평문 누적(키로거급) + 키당 동기 디스크 IO(입력 지연) | 배포 위생/프라이버시/성능 | `register.rs:147 UNIM_DEBUG_LOG=true`(주석 :143은 'false 권장'), 호출 201건, `debug.rs:26-29` 매호출 open/append | **P0** |
| 3 | **최초 활성화 100% 수동 + 닭-달걀** → 설치 후 바로 한글 입력 불가, 완료 화면·자동 등록 부재 | 온보딩 관문(blocker) | `unim.wxs` UIRef 없음; `set_as_default`는 `register.rs:166`에 있으나 호출부가 랭바/설정(이미 활성 전제)뿐 | **P0~P1** |
| 4 | **후보/조합 접근성 능동 노출 전무** → 시각장애·저시력 사용자 한자·이모지 완전 배제 | 접근성(blocker) | `unim-tsf/src`+`unim-popup-win/src` 전역 ITfUIElementSink/ITfCandidateListUIElement/WM_GETOBJECT/UIA **grep=0(재확인)** | **P0~P1** |
| 5 | **기확정/선택 한자 재변환·단어 변환·범용 사용자 사전 부재** | 기능 격차 | `candidates.rs:22-29`(마지막 1음절만), ITfFnReconversion grep=0, `typefix_userdict.rs:199` 한글 등록 차단 | **P0~P1** |

교차절단 리스크(아키텍처): `panic=abort`(`Cargo.toml:66-70`) 하에서 `catch_unwind`는 무효(no-op)인데, 입력 hot-path의 무가드 `.expect()`(`key_handler.rs:491`)가 오버레이 창 생성 실패 시 호스트 앱(Word/Chrome)을 통째로 abort시킨다 — 저확률이나 '호스트를 죽이지 않음'이라는 메이저 기준의 예외 지점.

### 1.3 로드맵 한눈에

- **단기(0~3M, P0~P1):** 코드 서명 파이프라인 · 릴리스 로그 기본 OFF · 최초설정 원클릭 활성화/완료 화면 · 후보/조합 TSF UIElement 접근성 노출 · 무가드 `.expect()` 제거 · CI 테스트 게이트 · 한자 재변환(ITfFnReconversion) · 키 처분(disposition) 단일 순수함수 통합.
- **중기(3~9M, P1~P2):** 자동 업데이트 채널(서명 검증) · 인프로세스 DLL 업그레이드 안전화(RestartManager/side-by-side) · 팝업 캐럿 앵커링 · 팝업/오버레이 테마·고대비 대응 · 범용 사용자 사전 · 단어 단위 한자 변환 · per-app 앱호환 config 외부화 · ATF `replace_composition` 코어 배선 · 접근성 섹션/프리셋 GUI · Slint i18n(ko/en).
- **장기(9M+, P2~P3):** 옛한글(고어) 입력 · 스위치 스캐닝 · 앱 능력 positive probing + 티어 캐시 영속화 · 매크로/약어 · ARM64 · 성능/메모리 예산·계측.

---

## 2. 성숙도 스코어카드

성숙도 4단계: **초기**(핵심 기능만, 제품 미달) · **사용가능**(동작하나 마감·신뢰 부족) · **경쟁력**(오픈소스 상위, 일부 상용 대등) · **메이저급**(상용 IME 대등/우위).

> 용어 주의: 본 보고서에서 **'축'은 아래 스코어카드의 평가 단위(11개)**이고, **'섹션'은 문서 장(§1~§10)**이다 — 둘은 1:1 대응이 아니다. 특히 접근성은 **스코어카드 3개 축(8 시각·9 지체·10 인지)** ↔ **문서 §7 단일 섹션(§7.1/7.2/7.3)**으로 매핑된다. 방법론·§10.3의 "11개 축"은 이 스코어카드 축 수를 가리킨다.

| # | 축 | 성숙도 | 핵심 병목 (한 줄) |
|---|-----|--------|-------------------|
| 1 | TSF 프런트엔드 아키텍처(`unim-tsf`) | 사용가능~부분경쟁력 | 호스트 크래시 안전성(무가드 expect)·상태머신 검증가능성(테스트 0)·구조 복잡도(1926줄 26필드) |
| 2 | 공유 코어 엔진(`src/`) | 사용가능(경쟁력 진입) | ATF 계약이 surrounding-text식(delete_chars)이라 비협조앱 회귀 근원, ATF 핫패스 엔진 재생성 |
| 3 | 경쟁 IME 대비 기능 격차 | 경쟁력 진입 | 한자(단어단위·기확정 재변환)·옛한글·범용 사용자사전 미달 |
| 4 | 입력 정확성·앱 호환성 | 사용가능~경쟁력 경계 | 폴백 앱 ATF 무고지 OFF, per-app 앱호환 config 부재(코드릴리스 결속) |
| 5 | 설정 GUI 미려함·사용성 | 경쟁력(메이저 미달) | i18n 전무, 모아치기 등 1차 GUI 기능 패리티 결손, 단일 인스턴스/리셋 미비 |
| 6 | 팝업/인디케이터 시각 완성도 | 사용가능~경쟁력 초입 | 캐럿 앵커링 부재(정중앙 고정), 팝업 테마 미대응, D2D 이모지 폰트 DPI 동결 |
| 7 | 설치/최초설정/온보딩 | 초기(사용가능 미만) | 코드서명·온보딩·자동업데이트·배포 위생 전무 |
| 8 | 시각장애·저시력 접근성 | 초기 | 후보 UIA 0·조합 능동통지 0·고대비 미추종·돋보기 뷰포트 밖 팝업 |
| 9 | 지체·운동장애 접근성 | 사용가능(미설계) | 자동반복 IME 세밀억제 부재, 고정키+RightAlt 토글 충돌, 접근성 섹션/프리셋 부재 |
| 10 | 인지·범용 접근성(UD) | 사용가능 | 파괴적 작업 확인/undo 부재, 프리셋/계층화 부재, 팝업 AT 무노출 |
| 11 | 안정성·배포·신뢰 | 사용가능(alpha) | 코드서명·크래시 텔레메트리·자동업데이트·로그 OFF·테스트 게이트·DLL 업그레이드 안전화 부재 |

관찰: **입력 코어·TSF 통합(1~4)은 경쟁력 근접**이나, **제품화 축(7·11)과 접근성 축(8·10)이 초기 단계**로 전체 성숙도를 끌어내린다. 메이저화는 기능 추가가 아니라 이 세 축의 격상이 관건이다.

---

## 3. 소스·아키텍처 분석

### 3.1 구조 개요

- **공유 코어 `src/`** (Linux·Windows 공유): 한글 조합(`hangul/`), 자판(`keystroke/keymap/`), 입력 엔진(`input_engine/`), 한자(`hanja/`), 오타교정(`auto_typefix/`), 설정(`config.rs`).
- **Windows 프런트엔드 `unim-tsf`** (TSF TIP `cdylib`): 텍스트 서비스(`text_service.rs` 1926줄), 키 처리(`key_handler.rs` 1053줄), 조합(`composition.rs` 1611줄), synth 주입(`synth_input.rs` 431줄), 등록(`register.rs` 253줄), 설정 폴백 다이얼로그(`settings_dialog.rs`), 랭바(`lang_bar.rs`).
- **부속 프로세스:** 팝업 렌더러 `unim-popup-win`(무상태 out-of-proc), 설정 GUI `unim-tsf-settings`(Slint 별도 exe), 레거시 `unim-imm32`(.ime).

### 3.2 강점

- **Full-surface TSF TIP:** ITfTextInputProcessorEx·KeyEventSink·CompositionSink·ThreadMgrEventSink·TextEditSink·DisplayAttributeProvider·FunctionProvider를 한 구조체에 `#[implement]` 집약, OnEndEdit read-back까지 완비(`text_service.rs:23-31, 604-702, 1408-1476`).
- **CUAS/IMM32 브리지 즉시-terminate 회피:** 조합 range에 `GUID_PROP_ATTRIBUTE`+`GUID_PROP_READING`, selection을 `fInterimChar=BOOL(1)`로 둬 CUAS가 미확정(GCS_COMPSTR)으로 브리지(`composition.rs:26-114, 146-164`). NavilIME·saenaru·kolemak 선례를 주석에 명시.
- **동적 CUAS 감지:** 앱 이름 휴리스틱이 아니라 `abs_shifted_total < delete_chars`(역확장 거부) 행동으로만 synth 폴백 결정(`composition.rs:1292`) → 정식 TSF 앱 회귀 0, 미지 앱 즉시 분기.
- **COM 자원 수명주기 정확성:** TextEditSink advise를 (cookie,context) 쌍 추적, context 변경 시 정확히 Unadvise 후 재advise(`text_service.rs:43-47, 552-599`).
- **msctf.dll 크래시 우회 등록:** `AddLanguageProfile`/`RegisterCategory` 재호출이 msctf 0x97e5a에서 0xC0000005를 유발함을 파악, LanguageProfile 6값·CLSID만 직접 기록하고 Category/Register는 wxs 위임(`register.rs:98-140`).
- **out-of-proc 팝업 역채널 마샬링:** 팝업 클릭을 message-only 창(WM_UNIM_REV)으로 받아 STA 스레드 펌프에서 edit session 수행, stale(owner/seq) 이벤트 차단(`text_service.rs:1531-1568`, `key_handler.rs:918-1034`).

### 3.3 약점·기술부채

| 약점 | 심각도 | 근거 |
|------|--------|------|
| COM 콜백 내 무가드 `.expect()`가 호스트 abort — `catch_unwind`는 `panic=abort`에서 무효(안전 착시) | major | `Cargo.toml:66-70`; `key_handler.rs:491`; `text_service.rs:631,1417-1420,1760` |
| OnTestKeyDown 정박 게이트 이중 포크 — SoT 부재(회귀위험), 비변형 predicate에서 문서 변형 | major | `text_service.rs:781-935,209-234`; `synth_input.rs:315-351`; `key_handler.rs:377-405` |
| synth 식별이 provenance 아닌 PENDING 카운터 기반 — 타이밍 의존·사용자 Backspace 오삼킴 | major | `synth_input.rs:49,52-97,108,332-351`; STALE_MS=2000(:97) |
| 손수 유지 락 순서·재진입 회피 — 컴파일 강제 없음, 데드락 시 호스트 프리즈 | major | `text_service.rs:998-1016,358-416,444-502` |
| 상태머신 복잡도 과다 — 1926줄 단일 구조체·26필드·다단 페이즈, ReplaceOutcome 4-variant | major | `text_service.rs:32-123,937-1107`; `composition.rs:270-288,630-727` |
| 핵심 상태머신 테스트 커버리지 사실상 0(`#[test]` 8건 전부 `popup_ipc.rs`) | major | grep: text_service/key_handler/composition/synth_input=0 |
| 키 hot-path 무조건 파일 로깅(preedit/commit/vk 평문) — 성능·프라이버시 | major | `register.rs:143-157`; `key_handler.rs:476`; `text_service.rs:940` |
| synth 상태가 thread_local 아닌 프로세스-전역 9 static — per-thread TIP 모델과 불일치 | minor | `synth_input.rs:52-97`, thread_local 사용 0건 |
| word 모드 앱 화이트리스트 하드코딩·비확장(`winword.exe`/`wmux.exe` 정확일치) | minor | `text_service.rs:462,1335`; config 키 부재 |
| 롤백 보험용 dead code 잔존 | minor | `composition.rs:1113-1191`; `synth_input.rs:168,243` |

**정정(중요):** 원 분석의 "lock().unwrap() ×96 = 크래시 벡터"는 오도다. `panic=abort` 하에서 뮤텍스 poison은 언와인딩-Drop 미실행으로 발생 자체가 불가하므로 96개 unwrap은 poison으로 패닉하지 못한다. 진짜 벡터는 `.expect()`·인덱스 초과·산술 오버플로·Option unwrap이다.

### 3.4 OnTestKeyDown 정박 게이트의 구조적 취약성 (심층)

메이저화를 가로막는 대표적 구조부채. TSF에서 `OnTestKeyDown`은 **비변형 predicate**("이 키를 먹을 것인가")로 규정되나, UNIM은 여기에 (a) synth echo 회계, (b) navkey/numpad/modifier-combo commit-passthrough, (c) english-hold, (d) tail 게이트를 **정박**시켰다(`text_service.rs:781-935`). 문제는 두 층위다.

1. **비표준 변형-in-predicate:** `commit_for_passthrough`(`text_service.rs:209-234`)가 OnTestKeyDown 내부에서 `end_composition`/`insert_text`로 실제 문서를 변형한다 — TSF가 비변형으로 규정한 지점에서의 mutation이며, 이것이 CUAS 특례의 근원이 된다.
2. **이중 포크와 SoT 부재:** wmux/xterm.js(Blink 터미널)는 OnTestKeyDown을 발화하지 않아(실측=0) 이 게이트가 죽는다. 그래서 각 게이트가 `handle_key_down`(`key_handler.rs:392-405`)·`observe_key_down` Case B(`synth_input.rs:332-350`)로 **이중 포크**됐다. "이 키가 조합 경계인가"의 단일 판정함수가 없어 세 곳(test_key_down·OnTestKeyDown 인라인·handle_key_down)에서 미묘히 다른 조건이 재유도되며, 두 포크가 동기이탈하면 회귀다.

**권고:** `classify_key(keycode, modifiers, engine_state, popup_active, comp_active, english_hold) -> KeyDisposition{Consume, CommitThenPassthrough, Passthrough}` 순수 함수로 통합하고 commit은 OnKeyDown에서 수행해 OnTestKeyDown을 비변형 predicate로 복원. (무엇: SoT 통합 / 왜: 앱별 특례 폭증·동기이탈 회귀 차단 / 공수: L / 영향: high / **P1**)

### 3.5 공유 코어 ↔ Windows 계약의 임피던스

코어 조합/자판 엔진 자체는 견고하나, TSF 특수제약(확정문 편집불가·앱별 동작)에 대한 **코어 1급 대응이 미비**해 프런트가 임시 배선으로 메운다.

- **ATF 결과 계약이 surrounding-text식:** `AutoTypeFixResult`가 delete_chars(화면 N자 삭제)를 지시(`mod.rs:42-61`). Linux는 delete_surrounding_text로 매핑되나 TSF 비협조앱(Chrome/wmux)에선 synth Backspace로 강등돼 회귀 saga의 근원. TSF-네이티브 무손실 경로 `replace_composition` 필드가 있으나 "Phase 2: 항상 false"로 코어 미배선(`forward.rs:130`·`reverse.rs:89`), 실제 조합치환은 프런트가 엔진 재생성으로 흉내(`auto_typefix.rs:417-437`).
- **핫패스 엔진 재생성:** ATF 적용 경로가 값싼 `engine.reset()`(`engine.rs:374`) 대신 `*engine = InputEngine::new(config)`를 순·역방향 양쪽에서 호출(`auto_typefix.rs:419,475`)해 매 교정마다 6.76MB 한자사전 재파싱+북마크 디스크 I/O를 반복.
- **CommitUnit::Smart의 '지능'이 코어에 없음:** 코어는 passthrough, 실제 앱 선택은 프런트 2-앱 하드코딩. config에 앱목록 필드 부재 → 재컴파일 없이 확장 불가(계층 역전).

---

## 4. 입력 품질 & 앱 호환성

### 4.1 앱별 신뢰성 매트릭스

앱 티어 분류: **FullTSF**(정식 조합·범위편집) / **CUAS-Bridge**(ShiftStart 거부, composition 유지) / **CUAS-즉시terminate**(오버레이 폴백) / **Blink-CE**(Chrome/Electron/wmux) / **IMM32-only**.

| 앱(대표) | 티어 | 조합 렌더 | 자동교정(ATF) | 후보(한자/이모지) | word 모드 | 잔존 결함 |
|----------|------|-----------|----------------|--------------------|-----------|-----------|
| 메모장/WordPad | FullTSF | inline 정상 | replace_surrounding 정상 | 정상 | Smart→기본 | — |
| MS Word (`winword.exe`) | FullTSF(특수) | inline 정상 | 정상조합 OK, **확정문 3중 차단**(D1 미구현) | 정상 | **word 하드코딩** | 확정문 자동교정 불가 |
| Chrome/Electron | Blink-CE | inline(fInterim) | synth pump-split **실측 OK** | 정상 | Smart | synth 경로 desync 위험 |
| wmux/xterm.js | Blink-CE(터미널) | inline | synth Case B 우회 | 정상 | **wmux 하드코딩** | **OnTestKeyDown=0**, 사용자 BS 오분류(LOW) |
| wezterm/Telegram | CUAS-Bridge | inline(fInterim) | 정상경로+synth 살아있음 | 정상 | Smart | — |
| conhost/즉시terminate CUAS | CUAS-즉시term | **오버레이 폴백** | **무고지 OFF** | 정상 | Smart | ATF 조용히 사라짐 |
| 카톡/한컴(IMM32 일부) | IMM32-only | 별도 `unim-imm32` | — | — | — | 활성화 결함 추적중(TSF경로는 32비트 DLL 커버) |
| 게임(RawInput/DirectInput) | 우회 | 미도달 | 미도달 | 미도달 | — | **플랫폼 내재 한계(모든 IME 공통)** |

### 4.2 화이트리스트 방식의 한계

- word 모드/MS Word 자동교정 판정이 `name==winword.exe || name==wmux.exe` 정확일치로 **세 곳에 복제**(`text_service.rs:460-463·1333-1336·1353-1359`)되고 `config.rs`에 앱목록 필드가 없다(grep=0).
- 결과: (a) 기본값 Smart의 2-앱 화이트리스트, (b) 일부 앱만 word로 켜는 per-app 세분화 수단 부재, (c) wmux 우회가 이름에 결속돼 동종 Blink 터미널 신규앱 자동적용 불가 → **앱호환이 코드릴리스에 묶임**.
- 학습 상태(`cuas_windows`)는 `Mutex<HashSet<isize>>`(HWND) 인메모리라 세션·창 재생성마다 소실(영속화 grep=0), 매 세션 최초 단어가 실험대상(비파괴적 열화).

### 4.3 자동적응 전략 (현재 vs 권고)

- **현재:** '이 앱은 조합을 깬다'만 **하향 단방향** 학습(`text_service.rs:1188`), 능력 상승(positive probing) 경로 없음. UIA 사용 0. by_time(200ms) 매직넘버로 즉시-terminate 판정(느린 머신/RDP 취약, 단 오학습은 비파괴적·창당 1회 자기수정).
- **권고(무엇·왜·공수·영향):**
  - 폴백 앱 ATF 무고지 해소: 오버레이 진입 시 랭바 툴팁·1회 로그로 '자동교정 제한' 고지 → 중기적으로 순방향(영→한)만이라도 synth 부활. (S→L / high / **P1**)
  - per-app 화이트리스트 config 외부화(`word_mode_apps`/`commit_unit_overrides`), 기본값에 현행 2개 seed. `maybe_reload_config` 핫리로드 경로 재사용. (M / high / **P1**)
  - 앱 능력 positive probing + 프로세스명 티어 캐시 영속화(`%APPDATA%\unim\app_tiers.json`). (L / medium / **P2**)

---

## 5. 기능 격차 (경쟁 IME 대비)

대상: Microsoft 한글 IME · 날개셋 · 새나루.

### 5.1 기능 대조표

| 기능 | MS 한글 IME | 날개셋 | 새나루 | **UNIM** | 격차 |
|------|:----:|:----:|:----:|:----:|------|
| 실시간 양방향 한/영 오타 자동교정 | ✗ | ✗ | ✗ | **✓(고유)** | UNIM 우위 |
| 대용량 한자사전 + 즐겨찾기 고정 | 부분 | ✓ | ✓ | **✓** | 대등/우위 |
| 이모지·특수문자 통합 | 부분 | 부분 | ✗ | **✓** | 우위 |
| 다양한 내장 자판 + JSON 커스텀 | 제한 | ✓(DIY) | 부분 | **✓** | 대등 |
| 옛한글(고어) 입력 | 부분 | **✓(대표)** | 부분 | **✗** | **결정적 결손** |
| 한자 단어 단위 변환 | ✓ | ✓ | 부분 | **✗(마지막 1음절)** | **결정적 결손** |
| 기확정/선택 텍스트 재변환(ITfFnReconversion) | ✓ | ✓ | 부분 | **✗(grep=0)** | **결정적 결손** |
| 범용 사용자 낱말/상용구 등록 | ✓ | ✓ | ✓ | **✗(영문 whitelist만)** | **결정적 결손** |
| 한자 후보 사용빈도 자동 학습 | ✓ | 부분 | 부분 | **✗(수동 북마크만)** | 격차 |
| 매크로/약어(스니펫) | 부분 | **✓** | ✗ | **✗** | 격차 |
| 표준 TSF 후보 UI(ITfCandidateListUIElement) | ✓ | ✓ | 부분 | **✗(자체 GDI 팝업)** | 접근성/원격 리스크 |

### 5.2 결정적 격차 상세 (근거)

- **옛한글:** 코어가 현대 초성 19·중성 21·종성 28만, 결과를 U+AC00 완성형으로만 생성(`char.rs:60-67`, `cho.rs:60-91`). 여린히읗·반시옷·아래아·첫가끝(U+1100) 불가. 학계/고문헌층 진입 조건.
- **한자 변환 마지막 1음절 제한:** 대상이 `preedit.chars().last()`(비면 None→변환불가, `candidates.rs:22-29`). 사전에 다음절 키가 **275,021줄** 존재(예 `경제:經濟`)함에도 단어 검색 경로 없음. (주의: `search_last_syllable`(`dict.rs:119-128`)는 tests/SPEC 전용 미사용 경로.)
- **재변환 미구현:** `ITfFnConfigure`만 등록, `ITfFnReconversion`은 unim-tsf 전역 grep 0건. 확정 한글 우클릭/재변환 표준 UX 부재.
- **범용 사용자 사전 부재:** `typefix_userdict.add()`가 `is_ascii_alphabetic()`으로 비영문 거부(`:199`) → 한글/한자/구절 등록 원천 차단.

### 5.3 권고

| 권고 | 무엇/왜 | 공수 | 영향 | P |
|------|---------|------|------|---|
| 기확정/선택 한자 재변환(ITfFnReconversion + target 확장) | 확정 후 수정 불가가 최대 체감 격차. `read_selection_text`/`replace_surrounding` 인프라 재사용. 단 이는 배포·온보딩·접근성 **차단요소(blocker)가 아닌 기능-패리티 격차**이므로, P0(채택 blocker) 정의와 일관되게 **P1**로 정렬(§9.1 Impact×Effort 매트릭스의 high 행 배치와도 일치) | L | high | **P1** |
| 단어 단위 한자 변환 | 데이터(275,021줄) 이미 존재, `search_word` 경로만 추가 | M | high | **P1** |
| 범용 사용자 낱말/상용구 사전 신설(YAML) | MS/날개셋 공통 기본기, 매크로 부재 상쇄. 신규 스키마 필요 | M | high | **P1** |
| 한자 후보 사용빈도 자동 학습 | 북마크 인프라 위 카운트 스토어, 저비용 | S | medium | **P2** |
| 옛한글 입력(별도 조합기 변형 격리) | 날개셋 핵심 차별점, 회귀 위험 커 후순위 | XL | high | **P2** |
| 매크로/약어 확장(사용자 사전 위) | 파워유저 유입 | M | medium | **P3** |
| 표준 TSF 후보 UI 병행(ITfCandidateListUIElement) | 접근성/원격 보험 | L | low | **P3** |

---

## 6. UI/UX 미려함

### 6.1 설정 GUI (`unim-tsf-settings` Slint 1차 + `settings_dialog.rs` DLL 폴백)

**강점(1차 Slint):** 4px 그리드 디자인 토큰·브랜드 그라디언트(`settings.slint:20-46`), 마스터디테일 사이드바(TabWidget 폐기), 다크/라이트 OS 자동추종(`:39-45`), 행별 상시 라이브 설명, 즉시저장+저장폭주 가드+**실제 라이브 적용**(엔진 `maybe_reload_config` hot-reload `text_service.rs:358`), 의존행 자동 dimming, 접근성 라벨·키보드 사이드바 이동. DLL 폴백은 MSI가 exe를 정식 동봉(`unim.wxs:241`)하므로 정상 설치에선 사실상 미노출.

**약점:**

| 약점 | 심각도 | 근거 |
|------|--------|------|
| i18n 전무 — Windows 설정 GUI 한국어 전용(`@tr` 0건, Linux GTK는 t!() 150건) | major | `settings.slint` 전반, `main.rs:224-237` |
| 기능 패리티 결손 — 모아치기(chord)가 1차 Slint에 통째로 없음(config 직접편집만) | major | settings.slint/main.rs 0건 vs DLL `settings_dialog.rs:707-719` |
| 설정 창 단일 인스턴스 가드 없음 — 창 중복·마지막저장 덮어쓰기 | minor | `register.rs:49` 무조건 spawn, main.rs Mutex/FindWindow 0건 |
| 리스트 시작 스냅샷 — 실행 중 학습분 미갱신 | minor | `main.rs:167` 1회 로드, 파일 감시 0건 |
| 기본값 복원(Reset) 부재, 검색 부재 | minor | reset/검색 위젯 3곳 0건 |
| 수치 위젯 SpinBox(하우스 '슬라이더 우선' 위배) | minor | `settings.slint:507-514,535-544` |
| DLL 폴백 WM_SETFONT 미호출(비트맵 폰트)·DPI 무대응·룰셋 겹침 | minor(노출↓) | `settings_dialog.rs` WM_SETFONT/CreateFont 0건, DPI 0건, :1394-1395 TODO |

**재설계 제안:** ① [P0] 모아치기 카드 신설(양방향 조합 Switch + 조합창 슬라이더) — 정규 GUI 유일 경로. ② [P1] i18n(@tr + ko/en, `GetUserDefaultUILanguage` 초기 로케일). ③ [P2] 단일 인스턴스화(Named Mutex/FindWindow→전면화) + 검색 + '기본값으로' 복원. ④ SpinBox→값 라벨 병기 Slider. ⑤ Window.icon 지정(제품 아이콘 embed).

### 6.2 팝업/인디케이터 (`unim-popup-win` render.rs, langbar)

**강점:** D2D 컬러 이모지(COLR/CPAL) + GDI 흑백 폴백 2단(`render.rs:253-273`), per-monitor DPI 스케일 + 프로세스 Per-Monitor-V2 선언(`main.rs:117`) + WM_DPICHANGED 재배치, 더블버퍼 무깜빡임, 테마 적응형 트레이 한/영 인디케이터(`lang_bar.rs:64-86`), 멀티해상도 투명 시그니처 ICO(9종 16~256px 32bpp 실측), Catppuccin 일관 팔레트 + 픽셀 정합 hit_test, 인라인 preedit 밑줄 디스플레이 속성(`display_attr.rs:29-54`).

**약점:**

| 약점 | 심각도 | 근거 |
|------|--------|------|
| 한자/이모지 팝업이 캐럿 아닌 **모니터 정중앙** 고정(§5.5 동결) | major | `window.rs:236-238`; 앵커 근거 `key_handler.rs:240-261` 실재 |
| 팝업 다크 전용 — 트레이는 테마 적응, 팝업만 다크 고정(정책 불일치·고대비 미대응) | major | `render.rs:15-30` vs `lang_bar.rs:64-86` |
| D2D 이모지 글리프 크기 최초 렌더 DPI에 동결(혼합 DPI 멀티모니터, 프로세스 수명 지속) | major | `d2d.rs:71,110-126,165-174` + `render.rs:259` |
| 폴백 preedit 오버레이 HiDPI 미스케일·라이트 흰박스(터미널 폴백 전용) | minor | `preedit_window.rs:26-33,36-39`; 폴백 게이트 `key_handler.rs:451` |
| Win11 라운드코너/그림자 없이 WS_BORDER 하드 1px | minor | `window.rs:80,90-91` |
| 선택 표시가 배경색 교체 단일 신호(형태 단서 없음) | minor | `render.rs:430-455` |

**재설계 제안:** ① [P1] 캐럿 앵커링(`RenderState`에 caret rect 옵셔널 필드 + `get_composition_screen_pos` IPC 전달). ② [P1] 팝업 라이트/다크/고대비 팔레트 2벌 + 트레이와 테마 정책 일원화. ③ [P1] D2D 이모지 TextFormat을 font_px 변화 시 재생성(저비용). ④ [P2] Win11 DWMWA 라운드코너+드롭섀도. ⑤ [P2] 선택 셀 2px 대비 링(WCAG 1.4.11).

### 6.3 설치/온보딩 (MSI · 최초 활성화)

**강점:** TSF 등록 이중 트랙(정적 RegistryKey + DllRegisterServer), 32비트(WOW64) 커버리지, 업그레이드/제거 위생(CloseApplication+재기동), GUID 단일 진실원 + CI/로컬 드리프트 게이트, ARP 메타데이터 완비.

**약점:** 아래 8장(안정성·배포)과 공유. 핵심은 (1) 코드서명 전무, (2) 최초 활성화 100% 수동+닭-달걀, (3) 진단 로그 릴리스 ON, (4) 자동 업데이트 부재, (5) IMM32 .ime CI 빌드/MSI 미패키징 불일치, (6) 라이선스/ICE 검증 스킵(`light -sval`).

**재설계 제안(온보딩):** 설치 완료 화면(WixUI_Minimal ExitDialog) + '기본 한국어 키보드로 설정' 체크박스 + `--firstrun` 헬퍼(InstallLayoutOrTip/HKCU CTF\Assemblies로 사용자 활성 입력목록 자동 추가 + `set_as_default` 배선 + 한/영·설정·한자키 3단계 미니 환영). (L / critical / **P1**)

---

## 7. 접근성 (장애인 기능) — 최우선 강조 섹션

> UNIM의 접근성은 "기반 카테고리는 선언됐으나 런타임 능동 계층이 통째로 비어 있는" 상태다. `unim-tsf/src`+`unim-popup-win/src` 전역에서 ITfUIElementSink · ITfCandidateListUIElement · WM_GETOBJECT · IRawElementProvider · UIAutomation **grep=0(본 보고서 재확인)**. 접근성은 메이저 IME의 법적·윤리적 필수요건이며, 현재 UNIM은 한자·이모지 후보 기능에서 스크린리더·저시력 사용자를 **완전히 배제**한다. 이 섹션을 가장 두껍게 다룬다.

### 7.1 시각장애·저시력 (Screen reader / Low vision)

#### 7.1.1 현 상태

**강점(전제 충족):**
- 조합을 정식 TSF 텍스트스토어 편집으로 처리(StartComposition→SetText→`GUID_PROP_ATTRIBUTE`+`GUID_PROP_READING`, `composition.rs:26-114,898-953`). 진짜 `ITfComposition`을 만들어 메모장/WordPad 등에서 NVDA composition-range 낭독 경로가 성립할 개연성이 큼(단 **라이브 미검증**). 다수 국산 IME가 legacy IMM32라 이 전제조차 못 맞추는 것과 대비.
- 입력 조합 display attribute 본문색이 `TF_CT_NONE`(`display_attr.rs:33-40`)라 조합 본문은 이미 앱 테마 추종. 고대비 미대응은 밑줄 선색(고정 녹색)·팝업 배경에 한정.
- UILess/DisplayAttr/Immersive 접근성 카테고리가 MSI에 **이미 등록**(`unim.wxs:126-171`) → 런타임 구현만 얹으면 됨(설치관리자 재작업 불필요).
- 후보 데이터가 TIP 프로세스 안에 이미 존재(`WireCell` t/m/f + RenderState sel/page, `popup_ipc.rs:49-68`) → UIA 제공자를 신규 IPC 없이 채울 수 있음.

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| 후보 팝업이 접근성 트리에 전혀 노출 안 됨(WM_GETOBJECT 미처리 GDI 창) → 후보 무낭독 | **blocker** | `window.rs:76-89,247-337`; UIA/IRawElementProvider grep 0건 |
| TIP가 조합/후보를 스크린리더에 능동 통지 안 함(ITfUIElementSink/CandidateList/ReadingInfo 미구현) | major | `text_service.rs:23-31,604-701`; grep 0건 |
| 고대비/다크·라이트 미추종(SPI_GETHIGHCONTRAST grep 0건) | major | `render.rs:15-30`, `preedit_window.rs:36-39`, `display_attr.rs:41-46` |
| 저시력 폰트 확대 설정 부재(config 배율 키 0건) + 폴백 오버레이 DPI 미스케일 | major | `config.rs` 0건; `preedit_window.rs:26,86-103` |
| 후보 팝업 모니터 정중앙 고정 + 계산된 캐럿 rect를 out-of-proc 팝업에 미전달 → **Windows 돋보기 확대 뷰포트 밖** | major | `window.rs:236-238`; `get_composition_screen_pos`(`key_handler.rs:240-261`)가 `ITfContextView::GetTextExt`(`:256`)로 캐럿 rect를 **이미 계산**하나 폴백 preedit 오버레이(`:489`)에만 쓰이고 팝업 IPC엔 미전달(GetTextExt는 grep=0이 아님 — §6.2/§10.1과 일치) |
| 한/영 모드 전환 시 능동 통지 전무 — 랭바 `szDescription`/툴팁은 **수동**(AT가 항목에 도달해야 읽힘), 토글 순간 `EVENT_OBJECT_STATECHANGE`/`NotifyWinEvent`·옵션 비프 없음 → 시각장애 사용자가 RightAlt/한영 토글 후 현재 모드 인지 불가 | major | `lang_bar.rs:433-485`(is_korean 반영 szDescription/툴팁, 수동); `NotifyWinEvent` grep 0건 |
| 라이트디스미스 접근성 이벤트(EVENT_OBJECT_IME_SHOW/HIDE/CHANGE) 미발생 | minor | NotifyWinEvent grep 0건 |

#### 7.1.2 신규 기능 제안 (구체)

1. **[P0] 조합·후보를 TSF UILess UI element로 능동 노출** — `ITfUIElementSink` 구현 + ActivateEx에서 `ITfUIElementMgr` advise. 조합 시 `ITfReadingInformationUIElement`(GetString=현재 조합), 후보 팝업 시 `ITfCandidateListUIElement`(GetCount/GetString(i)/GetSelection/GetCurrentPage를 RenderState로 채움). 데이터가 이미 프로세스 안·카테고리 기등록이라 착수 장벽 낮음. 정식 TSF·CUAS·폴백 오버레이 앱 **모두**에서 NVDA 조합 낭독 커버. (L / critical / **P0**) 표준: TSF UILess Mode, NVDA nvdaHelper tsf.cpp.
2. **[P1] 후보 팝업 창 UIA 제공자** — `unim-popup-win` wnd_proc에서 WM_GETOBJECT→IRawElementProviderSimple. 루트 ControlType=Menu/List, AutomationId='IME_Candidate_Window', 셀=ListItem(Name=t, HelpText=m, 선택셀 IsSelected=TRUE), MenuOpened/Closed·SelectionItem_ElementSelected 이벤트. Narrator 공식 규약 준수. (L / high / **P1**)
3. **[P2] 후보 팝업 캐럿 근처 배치 + 돋보기 추종** — 캐럿 rect 취득은 **신규 작업이 아님**: 이미 존재하는 `get_composition_screen_pos`(`key_handler.rs:240-261`, 내부에서 `ITfContextView::GetTextExt`(`:256`) 호출)를 **재사용**한다. 실제 잔여 작업은 이 rect를 `RenderState` IPC 필드로 out-of-proc 팝업에 전달하고 `compute_placement`가 캐럿 하단 배치(실패 시 중앙 폴백)하도록 배선하는 것뿐이라, GetTextExt를 새로 도입한다고 가정할 때보다 공수가 낮다. 돋보기 뷰포트 문제 동시 해결. (**S~M** / high / **P2**)
4. **[P2] 고대비·시스템 테마 추종** — display attribute 밑줄을 `TF_CT_SYSCOLOR`로, 팝업/오버레이는 SPI_GETHIGHCONTRAST 감지 시 GetSysColor 팔레트, WM_SETTINGCHANGE 재도색. (M / high / **P2**) 표준: WCAG 1.4.3/1.4.11.
5. **[P3] 저시력 폰트 배율 설정 + 폴백 오버레이 DPI 스케일** — config `popup_font_scale`(6지점 동기화) + `preedit_window` FONT_H에 모니터 DPI 스케일. (M / medium / **P3**)
6. **[P2] NVDA·Narrator·돋보기 실측 QA 시나리오** — 현 강점(정식 조합 낭독)조차 코드 추론일 뿐 라이브 미검증. baseline 확립 후 회귀 게이트화. (S / medium / **P2**)
7. **[P3] EVENT_OBJECT_IME_SHOW/HIDE/CHANGE 발생** — show_render/hide/재배치에서 NotifyWinEvent. (S / low / **P3**)
8. **[P1] 한/영 모드 전환 능동 통지(오디오 + 스크린리더 알림)** — 토글 성사 시 랭바 인디케이터 항목에 `NotifyWinEvent(EVENT_OBJECT_STATECHANGE)`(필요 시 `EVENT_OBJECT_NAMECHANGE`/UIA LiveRegion 병행)를 발생시켜 스크린리더가 '한국어↔English' 전환을 **즉시 낭독**하게 하고, 추가로 config 게이트 옵션 비프음으로 무시각 확인 경로를 준다. 현 랭바 `szDescription`/툴팁(`lang_bar.rs:433-485`)은 AT가 항목에 도달해야 읽히는 **수동** 통지뿐이라, 시각장애 사용자는 '가'/'A' 글리프를 볼 수 없어 RightAlt/한영 토글 후 지금이 한글인지 영문인지 알 수 없다(후보/조합 노출(7.1-1)만으로는 이 문제를 못 닫음). MS IME의 모드 전환 안내에 대응하는 메이저 IME 접근성 표준 기능이며, `NotifyWinEvent` 1회로 저비용·고레버리지. (S / high / **P1**) 표준: UIA LiveRegion, MSAA EVENT_OBJECT_STATECHANGE.

### 7.2 지체·운동장애 (Motor / Physical disability)

#### 7.2.1 현 상태

**강점:**
- 전환키(한/영·한자) 완전 재지정 + RightAlt 토글 데드코드 수정 확인(is_toggle_key를 is_modifier 가드 앞에서 판정, `config.rs:837-840,856-857`; `press_key.rs:56-112`; `key_handler.rs:81-89`) — WCAG 2.1.4 부합.
- 수정자 라이브 프로브로 **고정키(Sticky Keys) 래치 Shift가 쌍자음에 반영**(`modifier.rs:27-33`; `press_key.rs:282-284`) — OS와 자연 호환.
- 기본 한글 입력 타이밍 의존 0(모아치기 opt-in·기본 OFF, `config.rs:566-575`; `chord_buffer.rs:98-101`), 쌍자음도 무타이밍 결합 — WCAG 2.2 부합.
- 후보 선택 부담 낮음(Num1~9 직접선택 + 마우스/터치 역채널, `key_handler.rs:185-236,951-1001`) → 화상키보드·시선추적 dwell·스위치 포인터 가능.
- **'세벌식 순아래'는 확정 접근성 자판**(keymap JSON에 `accessibility`·`noshift` 태그 + "한 손 입력/손목 부담 적합" 설명, `ko_3bul_noshift.json:8,10,20-23`), 전 자모 무-Shift + 쌍자음 무타이밍 결합. 단 GUI에 프리셋/추천으로 미노출.

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| 조합키 자동반복 IME 레벨 억제 수단 없음(lParam KF_REPEAT 폐기) → 홀드 시 연타·토글 모드 진동 | major | `text_service.rs:781-786,937,1109-1120`; `press_key.rs:102-112` |
| 고정키 + 기본 RightAlt 토글 충돌 위험(전환 직후 첫 글자 유실, 코드-plausible·실기 미검증) | major | `config.rs:856`; `press_key.rs:84-122` |
| 좌우 이동 감소용 한손 미러/전용 프로필 부재(무-Shift 부담은 순아래가 해결) | minor | `config.rs:115-120` |
| 설정에 접근성 섹션·프리셋·필터키 연계 없음(기능은 존재, 발견성만 부족) | minor | `settings_dialog.rs:703-761` |
| 토글/한자키 자유텍스트 목록 + 입력 검증 없음 → 오타 시 조용히 토글 무력화 | minor | `settings_dialog.rs:479-484,752-760` |
| 모아치기 슬라이더 무효구간(1~9) 허용 후 조용히 OFF 강등 | minor | `settings_dialog.rs:718,1221-1225` |
| 스위치/스캐닝 입력 미지원 | minor | `key_handler.rs:918-1001` |

주: Windows 필터키(RepeatKeys)가 OS 레벨에서 반복을 전 앱 투명 억제하므로 자동반복 결함의 실질 심각도는 완화되나, 조합 맥락 세밀제어·토글 진동 방지는 IME 레벨 옵션이 필요.

#### 7.2.2 신규 기능 제안

1. **[P1] 조합키 자동반복 억제 옵션(IME 레벨 필터키)** — OnKeyDown/OnTestKeyDown에서 lParam bit30 읽어 반복 식별, `engine.ignore_key_repeat` 또는 `min_key_interval_ms`. 시그니처에 lParam 이미 유입, `_lparam→lparam` 수신만. 토글키 홀드 모드 진동 즉시 해소. (M / high / **P1**)
2. **[P1] 고정키 + 수정자 토글 충돌 해소** — 토글 성사 직후 다음 문자키 1건에서 해당 수정자 비트 마스킹 + '고정키 사용자는 비수정자 토글 권장' 안내. RightAlt 토글 자체는 유지. (M / high / **P1**, 실기 확인 시 P0 승격)
3. **[P1] 순아래 GUI 승격 + (중장기) 미러/전용 프로필** — 이미 접근성 태그 있으므로 GUI/문서 '한 손/고정키 추천 자판' 표면화(거의 무비용) + half-QWERTY 미러(프로필 v3 rule_set). (S→L / medium / **P1**)
4. **[P2] 설정 '접근성' 섹션 + 원클릭 프리셋** — '한 손 사용'(순아래+비수정자 토글+모아치기 OFF+반복 억제), '떨림 보정', '넉넉한 타이밍'. (M / medium / **P2**)
5. **[P2] 키 캡처 위젯 + 입력 검증** — 자유텍스트 대체, `parse_key_list` KeyCode 검증(오타→자기잠금 차단). (M / medium / **P2**)
6. **[P2] 슬라이더 유효 스텝 스냅 + OS 접근성 호환 실측·문서화** — 1~9 조용한 강등 제거, 필터키/고정키/OSK/시선추적 실측. (S / medium / **P2**)
7. **[P3] 후보 팝업 단일 스위치 스캐닝** — 역채널·view_model SoT 재사용. (L / low / **P3**)

### 7.3 인지·범용 (Cognitive / Universal Design)

#### 7.3.1 현 상태

**강점:** 설정 각 행 평문 description(`settings.slint:117-152`), Slint GUI 스크린리더·키보드 배선(accessible-label/role/포커스링/화살표 FocusScope), 인디케이터 색상 비의존('가'/'A' 글리프+테마+툴팁+MSAA szDescription, `lang_bar.rs:97-203,433-485`), 변경 즉시 저장+상태바 피드백, 무관 옵션 비노출(자판별 규칙세트 동적 구성).

**결함:**

| 결함 | 심각도 | 근거 |
|------|--------|------|
| 후보 팝업 접근성 트리 무노출(UIA/MSAA/TSF UIElement 0) → 시각장애 후보기능 완전 배제 | major | grep 0건(재확인) |
| 팝업 하드코딩 다크 → 라이트/고대비 데스크톱 붕괴(인디케이터와 자기모순) | major | `render.rs:16-30` vs `lang_bar.rs:64-86` |
| 파괴적 작업(모두삭제/개별삭제/사전삭제) 확인·undo 없이 즉시 실행 | major | `main.rs:407-429,466-475`(confirm/undo 0건) |
| 프리셋/기본·고급 계층 부재 — ms/시간/초 슬라이더 6종 평면 노출 | major | `settings.slint:501-589`; simple/preset/advanced grep 0건 |
| 기본값 복원/되돌리기 경로 부재 | major | reset/restore 콜백 0건 |
| i18n 미완(트레이 툴팁만 이중언어, 메뉴/About/Slint는 한국어 하드코딩, OS locale 미추종) | major | `lang_bar.rs:222-226,529-566`; settings.slint 전부 한국어 |
| DLL 폴백 다이얼로그 전문용어·한영 혼용 | minor | `settings_dialog.rs:811-821` |
| GUI 내 도움말·매뉴얼 경로 부재(AAA 수준) | minor | `lang_bar.rs:220-236` |
| 동일 개념 용어 불일치('정방향' vs '순방향') + 이중 설정 UI | minor | `main.rs:116-119` vs `settings_dialog.rs:805-808` |
| 팝업 열 헤더 활성/비활성 색상 단독 구분 | minor | `render.rs:24-25,400-401` |

#### 7.3.2 신규 기능 제안

1. **[P1] 후보 팝업 접근성 API 노출**(7.1과 동일 항목, TSF UIElement 또는 MSAA/UIA) — WCAG 4.1.2. (L / high / **P1**)
2. **[P0] 파괴적 작업 확인 + undo 토스트** — '모두 삭제' 확인 다이얼로그 + 삭제 후 '되돌리기' 5초 토스트(삭제 직전 스냅샷 1개). 학습 데이터 비가역 손실 방지. (S / high / **P0**) WCAG 3.3.4.
3. **[P0] 팝업 렌더러 OS 테마·고대비 대응** — `system_uses_dark_theme` 공유 + SPI_GETHIGHCONTRAST. 인디케이터-팝업 자기모순 해소. (M / high / **P0**) WCAG 1.4.3/1.4.11.
4. **[P1] 설정 '기본/고급' 2계층 + AutoTypeFix 3프리셋** — 상단 On/Off+'보수적/표준/적극적' 3버튼, 6개 슬라이더는 '고급' 접이식. (M / high / **P1**) COGA progressive disclosure.
5. **[P1] '기본값으로 복원' 버튼 + undo** — `Config::default()` diff 복원. (S / medium / **P1**)
6. **[P2] TSF 프런트엔드 i18n(ko/en) OS locale 추종** — lang_bar/Slint 리터럴 추출, `GetUserDefaultUILanguage`. (L / medium / **P2**)
7. **[P2] GUI 내 도움말 통합 + About URL 클릭 가능화**. (S / medium / **P2**)
8. **[P2] 용어 통일 + 폴백 라벨 평문화**. (M / medium / **P2**)
9. **[P3] 팝업 상태 색상+형태 이중부호화**. (S / low / **P3**)

### 7.4 접근성 종합

접근성은 UNIM 메이저화의 **가장 저평가된 리스크이자 가장 높은 레버리지 영역**이다. 결정적 사실은 (a) 후보/조합 접근성 API가 **전무**(grep=0)해 시각장애·저시력 사용자가 한자·이모지에서 완전 배제되고, (b) 그럼에도 **기반 카테고리는 이미 등록**(`unim.wxs:126-171`)돼 있고 **후보 데이터도 프로세스 안**(`WireCell`)에 있어 **착수 장벽이 낮다**는 점이다. P0 항목(7.1-1 UILess 노출, 7.3-2 undo, 7.3-3 테마)만으로도 "쓸 수 없음→쓸 수 있음"의 문턱을 넘고, 여기에 저비용 P1인 **한/영 모드 전환 능동 통지**(7.1-8, `NotifyWinEvent` 1회)를 더하면 무시각 사용자의 "지금 어느 모드인가" 문제까지 함께 닫힌다 — 후보 낭독만으로는 남는 사각지대다.

---

## 8. 안정성·배포·신뢰

### 8.1 강점 (장애 격리는 의외로 성숙)

- **panic=abort로 COM 경계 UB 원천 차단** — Rust panic 언와인딩이 msctf↔unim_tsf C ABI/COM vtable 경계를 넘으면 UB(실제 0xC0000005 유발), cdylib COM 노출의 유일 안전 선택(`Cargo.toml:61-70`). 근거 기반·실효.
- **팝업 렌더러 out-of-proc·무상태·자가복구** — 5초 rate-limit 재spawn, 죽어도 타이핑 지속·다음 render가 상태 복원(`popup_ipc.rs:974-985`).
- **32/64비트 이중 배포 + WOW64 대응**(`unim.wxs:193-252`), **TSF 등록 이중화**(정적 키 + DllRegisterServer), **업그레이드/다운그레이드/GUID drift 방어**(MajorUpgrade + CI 게이트).
- **렌더러 로거는 모범 사례**(5MiB 회전·ms 타임스탬프·OnceLock 핸들캐시, `logging.rs`) — 공용화 토대 존재.

### 8.2 약점 (제품 배포·신뢰 파이프라인 미완)

| 약점 | 심각도 | 근거 |
|------|--------|------|
| TIP 디버그 로그 릴리스 상시 ON + 키당 동기 파일 IO + 콘텐츠 평문 무회전 누적 | **blocker** | `register.rs:147`(주석 :143 'false 권장'); `key_handler.rs:47`; `synth_input.rs:358-360`; `debug.rs:26-29` |
| 코드서명 전무 → SmartScreen·기업배포 차단 | **blocker** | CI/스크립트/installer signtool 0건; `light -sval`; `README.md:120` |
| 크래시 텔레메트리 부재 + panic=abort로 장애가 호스트 앱에 귀속·불가시, catch_unwind 무효 | major | `text_service.rs:1420,1698,1712,1732,1746,1761`(no-op); unwrap/expect 99건 |
| 자동 업데이트 부재 | major | update/updater/latest.json 실참조 0건 |
| 릴리스 CI에 cargo test 게이트 없음 | major | `windows-msi.yml` cargo test 0건 |
| 인프로세스 TIP DLL 업그레이드 파일점유·리부트 처리 전무 → 리부트까지 stale, /qn 사일런트 무력화 | major | `unim.wxs:325-329`(렌더러만); RM/REINSTALLMODE/ForceReboot 0건 |
| MSI ICE 검증 비활성화(`-sval`), 코멘트는 정반대 서술 | minor | `windows-msi.yml:171,181`; `build-msi.bat:68` |
| 성능·메모리 예산·계측 부재 | minor | 벤치/예산 파일 부재 |
| 로깅 인프라 이원화(핵심 TIP만 열등) | minor | `debug.rs:14-30` vs `logging.rs` |

### 8.3 권고

| 권고 | 무엇/왜 | 공수 | 영향 | P |
|------|---------|------|------|---|
| TIP 로깅 기본 OFF + 레벨/회전/타임스탬프/핸들캐시, 콘텐츠 opt-in | IME는 키로깅 공격표면 그 자체. vk 상시로깅=사실상 키로거, 키당 동기 IO=전 앱 입력 지연 | M | critical | **P0** |
| Authenticode 코드서명 파이프라인(MSI+전 바이너리+.ime) | 미서명=SmartScreen 경고·기업 차단으로 유입 구조적 차단. Azure Trusted Signing/SignPath 무료 트랙 | M | critical | **P0** |
| 인프로세스 DLL 업그레이드 안전화(RestartManager or 버전드 side-by-side) | in-proc TIP는 항상 로드, 순진한 교체=리부트강제/실패→사일런트 패치 무력화. 자동업데이트 선결 | L | high | **P1** |
| 크래시 리포팅 + catch_unwind 착시 정리 | 호스트에 귀속된 장애가 개발자에 불가시→개선루프 부재. set_hook 마커/WER/미니덤프 | L | high | **P1** |
| 서명 검증형 자동 업데이트 채널(appcast) | 0.3.x 잦은 수정 전파 수단 전무. HKLM Run 상주 토대 있음 | L | high | **P1** |
| CI 테스트 게이트(cargo test) | 릴리스가 동작 검증 없이 빌드만으로 출하 | S | high | **P1** |
| ICE 검증 활성화 + 설치/제거 왕복 smoke | 32/64 이중 등록 컴포넌트 규칙 위험 큼 | S | medium | **P2** |
| 입력 지연·메모리 예산 수립·계측 | 전 앱 상주 컴포넌트라 시스템 체감 좌우 | M | medium | **P2** |

---

## 9. 우선순위 로드맵

### 9.1 Impact × Effort 매트릭스

```
 영향
critical │ [P0] 로그 OFF(M)        [P0] 코드서명(M)
         │ [P0] undo/확인(S)        [P1] 최초설정 온보딩(L)
         │ [P0] 팝업 테마·고대비(M)
─────────┼──────────────────────────────────────────────
  high   │ [P0] 무가드 expect제거(S) [P1] UILess 접근성노출(L)
         │ [P1] CI 테스트게이트(S)   [P1] 자동업데이트(L)
         │ [P1] engine.reset()(S)    [P1] DLL 업그레이드 안전화(L)
         │ [P1] 키 disposition 통합(L) [P1] 한자 재변환(L)
         │ [P1] 반복억제(M)          [P1] per-app config(M)
─────────┼──────────────────────────────────────────────
 medium  │ [P2] 팝업 캐럿앵커링(M)   [P2] 앱 티어 캐시(L)
         │ [P2] 후보 UIA(L)          [P2] Slint i18n(L)
         │ [P2] 접근성 프리셋(M)     [P2] 옛한글(XL)
─────────┼──────────────────────────────────────────────
  low    │ [P3] IME 이벤트(S)        [P3] 매크로/약어(M)
         │ [P3] 라운드코너(S)        [P3] ARM64(L)
         └──────────────────────────────────────────────
            S            M            L           XL   공수
```

### 9.2 시간 구획

> **우선순위 정의(일관화):** **P0 = 채택·온보딩·접근성·배포위생 차단요소(blocker)** 또는 사용자 데이터 비가역 손실 방지 — 이것이 막히면 '메이저 IME 인식' 자체가 불가한 항목. **P1 = blocker는 아니나 메이저 패리티에 필수인 기능-격차·구조 경화.** 이 정의에 따라 **한자 재변환(경쟁 기능-패리티 격차, 배포 차단요소 아님)은 P0가 아닌 P1**로 분류한다(§5.3·§9.1과 일관).

#### 단기 (0~3M) — 신뢰·안전·차단요소 제거

- **P0(blocker):** 릴리스 로그 기본 OFF + 콘텐츠 마스킹(배포 위생/프라이버시) · Authenticode 코드서명 파이프라인(채택 관문) · 무가드 `.expect()` 제거(호스트 abort 차단) · **접근성: 파괴적 작업 확인/undo, 팝업 OS 테마·고대비 대응**(7.3-2/3).
- **P0~P1:** 최초설정 원클릭 활성화 + 완료 화면(닭-달걀 해소) · **접근성: 조합·후보 TSF UILess UI element 능동 노출**(7.1-1, 시각장애 후보기능 개방) · **접근성: 한/영 모드 전환 능동 통지**(7.1-8, NotifyWinEvent 1회로 무시각 모드 인지).
- **P1:** 한자 재변환(ITfFnReconversion — 기능-패리티 격차라 위 blocker 항목과 분리) · CI 테스트 게이트 · `engine.reset()`로 ATF 핫패스 재생성 제거 · 키 disposition 단일 순수함수 통합 · 조합키 자동반복 억제(7.2-1) · per-app 앱호환 config 외부화.

#### 중기 (3~9M) — 기능 패리티·구조 경화·접근성 확장

- **P1:** 서명 검증 자동 업데이트 채널 · 인프로세스 DLL 업그레이드 안전화 · 크래시 리포팅 · ATF `replace_composition` 코어 배선 · 단어 단위 한자 변환 · 범용 사용자 사전 · 고정키+토글 충돌 해소(7.2-2) · 순아래 GUI 승격(7.2-3).
- **P2:** **접근성: 후보 팝업 UIA 제공자(7.1-2), 캐럿 앵커링+돋보기 추종(7.1-3), 고대비 추종(7.1-4), 설정 접근성 섹션·프리셋(7.2-4)** · 팝업 캐럿 앵커링(시각 완성도) · D2D 이모지 폰트 재생성 · Slint i18n(ko/en) · 앱 능력 positive probing + 티어 캐시 · 락 획득 중앙 가드.

#### 장기 (9M+) — 확장·니치·플랫폼

- **P2:** 옛한글(고어) 입력(별도 조합기 격리) · 한자사전 lazy-load · ATF surrounding-text 앵커.
- **P3:** 스위치 스캐닝(7.2-7) · 매크로/약어 · IME 접근성 이벤트(7.1-7) · Win11 라운드코너 · ARM64 · 성능/메모리 예산 계측 · 표준 TSF 후보 UI 병행.

### 9.3 접근성 로드맵 명시 배치

| 시기 | 접근성 항목 | P |
|------|-------------|---|
| 단기 | 파괴적 작업 확인/undo(7.3-2), 팝업 OS 테마·고대비(7.3-3/7.1-4), 조합·후보 UILess 노출(7.1-1), 한/영 모드 전환 능동 통지(7.1-8) | P0~P1 |
| 중기 | 후보 팝업 UIA(7.1-2), 캐럿앵커링+돋보기(7.1-3), 접근성 섹션·프리셋(7.2-4), 자동반복 억제(7.2-1), 고정키 충돌(7.2-2), 순아래 GUI 승격(7.2-3) | P1~P2 |
| 중기 | NVDA·Narrator·돋보기 실측 QA baseline(7.1-6) | P2 |
| 장기 | 스위치 스캐닝(7.2-7), IME 접근성 이벤트(7.1-7), 저시력 폰트 배율(7.1-5) | P3 |

---

## 10. 부록

### 10.1 근거 파일:라인 인덱스 (핵심)

**TSF 프런트엔드:**
- `Cargo.toml:61-70` — panic=abort(release/dev)
- `unim-tsf/src/register.rs:143-157` — UNIM_DEBUG_LOG=true, dbg_log
- `unim-tsf/src/register.rs:98-140` — LanguageProfile 6값 직접 기록(msctf 크래시 우회)
- `unim-tsf/src/text_service.rs:23-31` — #[implement] 7 sink/provider
- `unim-tsf/src/text_service.rs:209-234` — commit_for_passthrough(OnTestKeyDown 내 문서 변형)
- `unim-tsf/src/text_service.rs:358` — maybe_reload_config(hot-reload)
- `unim-tsf/src/text_service.rs:460-463,487-493,1333-1336,1353-1359` — winword/wmux word-gate 하드코딩 3중 복제
- `unim-tsf/src/text_service.rs:781-935` — OnTestKeyDown 정박 게이트
- `unim-tsf/src/text_service.rs:1166-1190` — 200ms by_time + 1-hit CUAS 학습
- `unim-tsf/src/text_service.rs:1408-1476` — OnEndEdit read-back / ITfFnConfigure
- `unim-tsf/src/text_service.rs:1417-1420,1698,1712,1732,1746,1761` — catch_unwind(panic=abort에서 no-op)
- `unim-tsf/src/key_handler.rs:240-261` — get_composition_screen_pos(캐럿 rect)
- `unim-tsf/src/key_handler.rs:451-496,491` — 오버레이 폴백 게이트 + 무가드 `.expect()`
- `unim-tsf/src/key_handler.rs:470-472` — 폴백 경로 ATF None(무고지 OFF)
- `unim-tsf/src/composition.rs:146-164,157` — fInterimChar=BOOL(1)
- `unim-tsf/src/composition.rs:270-288` — ReplaceOutcome 4-variant
- `unim-tsf/src/composition.rs:1292` — 동적 CUAS 감지(abs_shifted_total<delete_chars)
- `unim-tsf/src/synth_input.rs:49,52-97,108,332-351` — PENDING 카운터, STALE_MS=2000, Case B BS 오분류
- `unim-tsf/src/display_attr.rs:29-54,33-40,41-46` — 밑줄 속성, crText/crBk=TF_CT_NONE, crLine 고정 녹색
- `unim-tsf/src/lang_bar.rs:64-86,97-203,433-485` — 트레이 테마 적응·szDescription·툴팁(이중언어)
- `unim-tsf/src/preedit_window.rs:26-33,36-39,86-103` — 폴백 오버레이 고정 px·라이트 색·DPI 미스케일

**공유 코어:**
- `src/hangul/input_context.rs:328-343,220-238` — preedit 이중 API, word backspace 키재생
- `src/hangul/char.rs:60-67` — 19/21/28·0xAC00 완성형
- `src/input_engine/engine.rs:132,147,294-296,324-327,374` — 사전 즉시로드, is_toggle_key, set_word_mode flush, reset
- `src/input_engine/candidates.rs:22-29,74-104,186-263` — 한자 마지막1음절, 특수문자 폴백, 북마크 재정렬
- `src/auto_typefix/mod.rs:42-61,33` — replace_composition(항상 false), DICTIONARY Lazy
- `src/auto_typefix/forward.rs:130`, `reverse.rs:89` — replace_composition false 고정
- `src/typefix_userdict.rs:199` — add() is_ascii_alphabetic(한글 등록 차단)
- `src/config.rs:64-73,115-120,362-411,566-575,817-823,856` — CommitUnit::Smart, 자판 4종, AutoTypeFixConfig, chord, AppRule, RightAlt 토글
- `src/data/hanja.txt` — 6,756,043B(다음절 키 275,021줄)
- `src/keystroke/keymap/ko_3bul_noshift.json:8,10,20-23` — 순아래 accessibility/noshift 태그

**팝업/GUI/설치:**
- `unim-popup-win/src/render.rs:15-30,57-60,253-273,400-401,588-595` — 팔레트, DPI 스케일, D2D 컬러, 열 헤더 색상, 선택 구분
- `unim-popup-win/src/window.rs:236-238,76-89` — 정중앙 배치, GDI 창(WM_GETOBJECT 없음)
- `unim-popup-win/src/d2d.rs:71,110-126,165-174` — 이모지 폰트 동결
- `unim-popup-win/src/main.rs:117` — SetProcessDpiAwarenessContext(Per-Monitor-V2)
- `unim-popup-win/src/popup_ipc.rs:49-68` — WireCell/RenderState
- `unim-tsf-settings/ui/settings.slint:20-46,117-152,401-412,501-589` — 토큰, SettingRow, 규칙세트, 임계값 슬라이더
- `unim-tsf-settings/src/main.rs:167,266-347,407-429` — 1회 로드, auto-save, blacklist 삭제
- `unim-tsf/src/settings_dialog.rs:479-484,707-719,1394-1395` — parse_key_list 무검증, 모아치기, 겹침 TODO
- `installer/wix/unim.wxs:41-42,126-171,182-219,241,260-273,325-329` — MajorUpgrade, 접근성 카테고리, 32비트, exe 동봉, HKLM Run, CloseApplication
- `.github/workflows/windows-msi.yml:79-96,171` — imm32 빌드, light -sval
- `scripts/build-msi.bat:41-51,68` — 32/64 빌드, -sval

### 10.2 참고 표준·경쟁제품

**표준/API:**
- TSF: ITfComposition/ITfRange::SetText · ITfFnReconversion · ITfContextView::GetTextExt · ITfCandidateListUIElement/ITfReadingInformationUIElement(UILess Mode) · DisplayAttributeProvider
- 접근성: UI Automation / MSAA(IAccessible, WM_GETOBJECT) · WCAG 2.2(1.4.1/1.4.3/1.4.4/1.4.10/1.4.11/2.1.4/3.2.4/3.3.1/3.3.2/3.3.4/3.3.5/4.1.2) · Narrator/NVDA · Windows 고대비(SPI_GETHIGHCONTRAST) · Sticky/Filter Keys · EVENT_OBJECT_IME_SHOW/HIDE/CHANGE · Windows 돋보기(Magnifier)
- 배포: Authenticode/SmartScreen · signtool RFC3161 · Windows Installer(RestartManager/MsiRMFilesInUse, ICE, REINSTALLMODE) · IMM32 HKL Preload/Layout File · Per-Monitor DPI Awareness V2 · Win11 Fluent(DWMWA_WINDOW_CORNER_PREFERENCE) · WER
- 국제화: Slint @tr() · rust-i18n · GetUserDefaultUILanguage
- 업데이트: Sparkle/WinSparkle appcast · GitHub Releases API

**경쟁제품:**
- Microsoft 한글 IME(TSF 낱말 변환·후보 학습·사용자 단어 등록·후보 UI AT 노출)
- 날개셋(옛한글 입력·자판 DIY/.ist·매크로/준말·단어 변환)
- 새나루(오픈소스 TSF 한글 IME 선례)
- 참조: Mozc/Google 일본어입력(앱 프로파일 캐시·사일런트 업데이트), ATOK(앱별 설정)

### 10.3 검증 메모

본 보고서는 11개 축의 파일:라인 대조 검증 분석을 종합했으며, 작성 시 다음 핵심 근거를 실코드로 재확인함: `register.rs:147 const UNIM_DEBUG_LOG: bool = true`(주석 :143 'false 권장'), `Cargo.toml:66-70 panic="abort"`(release/dev), `key_handler.rs:491 PreeditWindow::create().expect(...)`, `composition.rs:157 fInterimChar: BOOL(1)`, ITfUIElementSink/ITfCandidateListUIElement/WM_GETOBJECT grep=0, `hanja.txt` 6,756,043B, 핵심 5파일 라인수(text_service 1926 / key_handler 1053 / composition 1611 / synth_input 431 / register 253). 원 분석 대비 주요 정정: (1) 'lock().unwrap()×96 크래시 벡터'는 panic=abort 하 poison 불가로 red herring(진짜 벡터는 .expect()·인덱스·산술), (2) catch_unwind는 panic=abort에서 no-op(안전 착시), (3) 폴백 오버레이 결함들은 composition_unsupported 폴백 전용이라 주 경로(inline fInterimChar) 대비 노출빈도 낮음, (4) '카톡 지원 증발'은 과장(TSF 경로는 32비트 DLL 커버, IMM32는 별개 레거시), (5) 영어 ATF 사전은 이미 Lazy 로드.
