# TSF–IMM32 브릿지 종합 판정 — wezterm inline preedit (서드파티 TSF TIP)

> 8개 각도 findings + 10개 적대적 verdict 종합. 1차 소스(MS Learn, 실제 IME 소스, ReactOS/Wine, 앱 소스) 우선.
> 추측과 확증을 명확히 구분한다. 작성일 2026-06-07.

---

## 0. TL;DR (확정 판정)

1. **가능한가? → 조건부 가능 (conditional).** 순수 TSF TIP의 wezterm inline은 *원리상* 가능하다 — "터미널은 조합 불가"는 **반증됨**. 단 UNIM이 *현재* 되지 않는 것도 사실이며, 차이는 **CUAS-호환 composition 셋업**에 있다.
2. **"TSF-IMM32 브릿지"의 정체 → CUAS** (Cicero Unaware Application Support; `msctf.dll` 내장 + 컨텍스트 소유자 `Msimtf.dll`). **OS 자동, 호출 API 아님.** 서드파티 TIP도 동일하게 이용 가능하나 "켜는 함수"는 없다. 사용자 제보는 정확하다.
3. **우선순위 경로**: (1순위·저위험) `GUID_PROP_READING` 추가 + 단일 세션 start→SetText → (2순위) IMM32 `.ime` 별도 DLL → (백업) overlay.
4. **지금 당장 빌드 없이**: `%TEMP%\unim-tsf.log` SetValue/terminate 교차분석, Spy++로 MS IME vs UNIM의 `WM_IME_*` diff. (코드 1줄도 안 고치고 원인 좁히기.)

---

## 1. wezterm inline은 서드파티 TSF TIP에게 가능한가?

**판정: 조건부 가능 (CONDITIONAL).** 두 개의 독립 verdict가 "원천 불가" 단언을 **refuted**로 깸:

- **Mozc = 순수 TSF TIP**(`win32/ime/` 디렉터리 없음, `.ime` 0개, Imm write 호출 0건)이 표준 TSF composition만으로 레거시 IMM32 GUI 앱에 CUAS 경유 inline을 만든다. `tip_edit_session_impl.cc` 주석이 메커니즘을 명시: **"CUAS updates GCS_RESULTCLAUSE and GCS_RESULTREADCLAUSE by using the segment structure of GUID_PROP_READING property"** (L137-177). 즉 CUAS가 TSF→GCS_* 로 브리지하는 입력이 `GUID_PROP_READING` 세그먼트 구조다.
- **wezterm 실측**: issue #2569은 Windows MS IME(TSF TIP)의 라이브 inline preedit이 wezterm 패널에 뜨는 동작을 전제로 한 버그. → CUAS는 wezterm(자체 HWND GUI 창, `ImmGetContext` 유효)에 GCS_COMPSTR 라이브 브리지를 제공한다.

**그러나 결정적 공백 (uncertain로 남는 부분):**
- "서드파티 TIP가 *실제로* sustained wezterm inline을 얻은" 직접 동작 보고는 in-hand 미확보. UNIM 자신이 *production 경로 위에 있는데도* CUAS가 즉시 terminate(GCS_RESULTSTR로 스냅샷) → MS IME는 같은 경로에서 정상. **"무엇이 composition을 live GCS_COMPSTR로 만드는가 vs terminate되는가"를 명시한 1차 문서는 없다** (msctf 내부 블랙박스; MS Learn CUAS 전용 페이지 404; Wine msctf composition은 STUB).
- **경계 교정**: MS Learn 1차 문구 "all non-TSF-enabled applications, **except 16-bit and console window applications**"의 제외는 진짜 콘솔 서브시스템(conhost classic)에만 직접 적용된다. **wezterm은 GUI 창이라 이 제외에 안 걸린다.** 따라서 wezterm의 즉시-terminate를 "MS가 콘솔 제외해서"로 설명하는 것은 부정확 — 원인은 composition 셋업 비호환이다.

---

## 2. "TSF-IMM32 브릿지"의 정체와 쓸모

**정체 = CUAS (Cicero Unaware Application Support).** `msctf.dll`에 내장된 OS 자동 에뮬레이션 레이어로, TSF-비인지(IMM32-only) 앱을 위해 컨텍스트 소유자(`Msimtf.dll`)와 default text store를 대행하고, TIP의 TSF composition을 IMM32 `WM_IME_COMPOSITION`+`GCS_COMPSTR`로 역브리지한다.
- 1차 근거: katahiromz/ImeStudy "CUAS is an emulation layer that connects between the old IMM32-based application and a TSF TIP"; MS Learn `ImmDisableTextFrameService`(TSF/IMM32/AIMM 1.2를 나란히 호환 대상으로 나열).
- **쓸모 있는가? → 그렇다, 그러나 "호출"하는 게 아니다.** CUAS는 OS 자동이며 서드파티가 부를 브리지 API는 존재하지 않는다. TIP은 그냥 *CUAS가 미확정으로 인식할* 표준 TSF composition을 만들면 된다.

**브릿지로 오인된 막다른 길 (모두 no-go, 적대 검증 완료):**
- **AIMM (IActiveIMMApp)** — refuted. 비-아시아 Win9x/NT4용 ActiveX 래퍼, "disabled for Windows 2000 and later". 방향이 앱(소비자)→IMM. cross-process 주입 불가.
- **Transitory Extension** — refuted. `ITfTransitoryExtensionSink`는 **앱**이 구현(TSF→앱 방향). TIP은 켤 위치에 없고 wezterm은 sink 미구현. 이중 차단.
- **direct ImmSetCompositionString from TIP** (`CtfImmGenerateMessage` 등) — **REFUTED (강).** `ImmSetCompositionStringW`는 hMsgBuf에 직접 적재하지 않고 *현재 HKL의 IME DLL*에 위임(Wine `imc_select_ime`→`ime_acquire(GetKeyboardLayout(0))`). TSF TIP 활성 시 HKL은 IME HKL이 아니라 TSF 프로파일 → 경로가 **이미 UNIM을 죽이는 그 CUAS 브릿지로 재진입**. 지목된 `CtfImmDispatchDefImeMessage`는 `IsMsImeMessage`(MS IME UI)만 forward. write-side production 선례 0건.
- **하이브리드 .ime 등록** — refuted (인과로서). 단일 DLL이 COM+Ime* export를 물리적으로 동시 보유는 가능하나, 동시 *활성화* 불가·CUAS 우회 미보장·Win8+ 차단. Mozc는 `.ime` 완전 폐기 TIP-only 출고. inline 효과가 나도 그것은 "IMM32 단독 회귀"의 효과지 하이브리드 덕이 아니다.

---

## 3. 구현 경로 — 난이도·위험도 우선순위

### 1순위 — CUAS-호환 TSF composition 정상화 (저위험·저비용, 빌드 1회)
근거 강도 **상** (Mozc 1차 소스 + verdict 다수 정합). 두 가지를 **동시 적용**:
- **(A) `GUID_PROP_READING` SetValue 추가.** UNIM은 `GUID_PROP_ATTRIBUTE`만 설정, **READING 미설정**(composition.rs grep 확인). Mozc 메커니즘상 CUAS가 GCS_COMPSTR/RESULTREADCLAUSE 세그먼트를 만드는 입력이 바로 READING이다. **즉시-terminate의 유력한 1차근거 차이.** (가설이나 가장 강함.)
- **(B) 2-phase → 단일 세션 start→SetText.** 현재 `start_composition`은 phase1(빈 StartComposition+세션종료)·phase2(별도 SetText) **2개 독립 top-level 세션**(composition.rs:223-306, 주석 367-374). Mozc/SampleIME는 **단일 세션**에서 `InsertTextAtSelection(QUERYONLY)→StartComposition→SetText`. 코드 주석의 "SampleIME도 2-phase"는 소스 확인 결과 **거짓**. (단, 선행 실측이 "단일세션=즉시terminate"였다면 그땐 SetText→Start 잘못된 순서였을 수 있음 → SampleIME 정확 순서 Start→SetText 동일 ec로 재시도 가치 있음. 동일 단일세션 코드가 이미 composition.rs:528-541 `ReplaceSurroundingEditSession`에 존재 → 그 패턴을 일반 경로로 승격.)
- **착수 금지**: 선행 SYNTHESIS의 **P1 "`ITfContextOwnerCompositionSink` 구현/등록"** — **다중 verdict로 refuted.** 그 sink는 *앱(컨텍스트 소유자)*이 구현(MS Learn nn-msctf-itfcontextownercompositionsink); 레거시 앱은 CUAS가 owner. Mozc·SampleIME·UNIM 셋 다 `ITfCompositionSink`만 구현 = **UNIM이 빠뜨린 인터페이스 없음.** 시간 낭비.

### 2순위 — IMM32 `.ime` 별도 DLL (고비용, 가장 확실)
근거 강도 **상** (saenaru 1차 소스로 **confirmed**, enables_inline=true). 유일하게 1차 소스로 *검증된 확실* 경로:
- saenaru(`src/saenaru.def` IME DDI 전체 export, `src/imm.c` L259-266이 `WM_IME_COMPOSITION`에 `GCS_COMPSTR|GCS_COMPATTR` 세팅, `ImeInquire`가 `IME_PROP_AT_CARET`)가 IMM32 소비자에 inline data를 직접 밀어넣음.
- 현 Rust 엔진 유지 + 얇은 IMM32 `.ime` cdylib 별도 빌드(ImeInquire/ImeProcessKey/ImeToAsciiEx/NotifyIME/ImeSetCompositionString + CompStr/Cand WndProc). IMM32+TSF 둘 다 제공하는 하이브리드 등록(MS/날개셋/새나루 정석).
- 리스크: 작업량 huge; Win8+ IMM 차단 가능성(ReactOS wiki "may have" — medium); 새나루 코드 MS 샘플 파생 라이선스 확인 필요. **단 1순위가 실패할 때만 착수.**

### 백업 — client-side overlay (`preedit_window.rs`)
1·2순위 모두 막히면 유일답. wezterm에 미확정을 안 보내고 UNIM 자체 창에 그림, 확정만 앱 삽입. (MEMORY 기록상 wezterm/Telegram은 del+재삽입 폴백이 깨지므로 overlay가 안전.)

---

## 4. 지금 당장 시도 — 빌드 없이 / 최소 노력

1. **로그 교차분석 (코드 0줄):** `%TEMP%\unim-tsf.log`에서 `SetValue FAILED`/`GetValue MISMATCH` 0건 재확인(attribute 무죄 검증) + `StartComposition→OnCompositionTerminated: IMMEDIATE` 타임스탬프로 종료 시점 특정.
2. **Spy++ diff (코드 0줄):** 동일 wezterm에서 MS 한국어 IME vs UNIM의 `WM_IME_STARTCOMPOSITION`/`WM_IME_COMPOSITION(GCS_COMPSTR vs GCS_RESULTSTR)`/`WM_IME_ENDCOMPOSITION` 시퀀스 캡처·비교 → CUAS 분류 전환 지점을 실증으로 특정. **가장 정보량 큰 무빌드 실험.**
3. **undocumented 컨텍스트 진단 (소량 코드):** GUID `{A94C5FD2-C471-4031-9546-709C17300CB9}` compartment를 GetValue(VT_I4 & 0x01)로 읽어 wezterm 컨텍스트가 CUAS-emulated인지, MS IME가 거는 base context와 같은지 확인.
4. **StartComposition 직후 `ppComposition` NULL 로깅:** =`OnStartComposition pfOk=FALSE`(CUAS 거부) 여부 판별.

---

## 5. 충돌하는 주장의 판정

| 주장 | verdict | enables_inline | 근거 강도 |
|---|---|---|---|
| CUAS가 TSF→GCS_COMPSTR 브리지 (메커니즘 존재) | confirmed | — | 상 (1차) |
| 서드파티 TIP가 *실제로* wezterm inline 획득 | **uncertain** | false (미입증) | 메커니즘 상 / 충분성 미입증 |
| saenaru IMM32 `.ime` → IMM32 앱 inline | **confirmed** | **true** | 상 (1차) |
| direct ImmSetCompositionString from TIP | **refuted** | false | 상 (반증) |
| AIMM / Transitory Ext. / 하이브리드 등록 | **refuted** | false | 상 (반증) |
| 선행 P1 `ITfContextOwnerCompositionSink` 필요 | **refuted** | — | 상 (Mozc+MS Learn) |

**핵심 메타판정:** 메커니즘 존재는 confirmed, 서드파티 *충분성*은 uncertain, IMM32 `.ime`만 확실 confirmed. → **1순위(저위험 TSF 정상화)부터 실측하되, 실패 시 2순위(IMM32 .ime)가 1차 소스로 보장된 fallback.**

## 6. 확증 vs 추측 구분
- **확증(1차):** CUAS 존재·OS자동, Mozc TIP-only, READING이 CUAS 세그먼트 입력, saenaru IMM32 inline, wezterm 순수 IMM32, ContextOwnerCompositionSink는 앱 측, direct-imm/AIMM/transitory no-go.
- **강한 가설(미실측):** READING 추가 또는 단일세션 전환이 UNIM 즉시-terminate를 실제로 없앤다. → 1순위 실험으로 검증 필요.
- **블랙박스(1차 부재):** CUAS의 정확한 live-vs-terminate 분류 규칙. msctf 내부 비공개.
