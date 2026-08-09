# IMM32 ↔ TSF 연계 지식베이스 (Windows 한글 입력기 개발)

> 대상: UNIM Windows TSF TIP(Text Input Processor) 개발자
> 목적: IMM32 레거시 앱(wezterm, Telegram/Qt 등)에서 TSF TIP의 한글 조합(composition)이
> 끊기는 현상의 근본 원인을 규명하고, 레퍼런스 IME(mozc/weasel/Mozilla/Chromium)의
> 검증된 패턴으로 수정 방향을 제시한다.
> 작성 기준: 코드는 현재 HEAD(branch `feat/windows-msi-redesign`, commit `8b67db9` 이후), 문서는 7차원 조사 + 적대적 검증 + 완전성 비평 종합.
> 표기 규칙: 각 주장에 `[n]` 출처 번호, confidence는 **확인됨**(1차 소스 2개+) / **정황**(2차 자료·정황만) / **추측**(소스 없음) 3단계.

---

## 1. 개요 — 왜 IMM32 레거시 앱에서 TSF TIP 연계가 어려운가

**핵심 결론 먼저:**

1. **근본 원인은 UNIM 코드 결함이 아니라 환경(앱)의 구조적 한계다.** wezterm·Telegram(Qt)은 네이티브 TSF text store(`ITextStoreACP`)를 구현하지 않고 IMM32 API에만 의존한다. 이런 "Cicero-unaware" 앱에서는 Windows의 **CUAS**(Cicero Unaware Application Support) 에뮬레이션 레이어가 더미 text store를 끼워넣고 **context owner를 CUAS 자신이 소유**한다. [1][2][16]

2. **즉시-종료(immediate `OnCompositionTerminated`)의 직접 메커니즘은 "0폭 caret collapse"가 아니라 CUAS owner-side 종료다.** UNIM 자체 실측에서 caret style을 `TF_AE_NONE` + range 전체로 바꿔도 즉시-종료가 **100% 동일 재현**되어 caret 가설은 기각되었다. [21] 한편 SampleIME(정상 IME)도 `Collapse(TF_ANCHOR_END)` 0폭 caret을 쓰면서 정상 동작한다. [17] MS 공식 문서상 `ITfCompositionSink::OnCompositionTerminated`는 "이 서비스(TIP) 외의 주체(=owner=CUAS)가 composition을 끝낼 때만" 호출된다. [3]

3. **과거 UNIM의 실제 코드 결함은 종료 콜백의 `engine.reset()` 자폭이었고, 이미 제거되었다.** `OnCompositionTerminated`가 매번 엔진을 리셋해 preedit가 영원히 1글자에 머무는 버그가 commit `8b67db9`에서 해소됨(정상 종료 정리는 `OnSetFocus`로 이전). [29][30]

4. **현재 남은 최대 갭은 6곳의 동기 edit session(`TF_ES_SYNC`)이다.** MS 공식 "Rules of Text Services"는 *"두 번째 규칙: 동기 edit session을 피하라. Microsoft Word 같은 다수 text store는 동기 세션을 절대 grant하지 않는다"*고 명시한다. [24] mozc도 issue #821에서 동기 세션 의존을 폐기하는 재설계를 진행했다. [23]

**왜 본질적으로 어려운가:** Windows 입력은 IMM32(함수형 + `WM_IME_*` 메시지)와 TSF(COM 기반, 비동기 락 모델)라는 **두 세대 프레임워크**가 공존하며, CUAS가 그 사이를 닫힌-소스(undocumented) 브리지로 변환한다. TIP은 자신이 TSF 계약만 지켜도, 그 출력이 CUAS의 블랙박스 변환을 거쳐 IMM32 메시지로 합성되는 경로의 타이밍·종료 의미가 **공식 문서에 거의 없어** 리버스엔지니어링(ReactOS)과 실측에 의존해야 한다. [1][2]

---

## 2. CUAS 아키텍처 — msctf / msctfime / msutb 역할, TIP→IMM32 변환 경로

### 2.1 모듈 역할 분담 (**확인됨**)

| 모듈 | 역할 | confidence |
|---|---|---|
| **msctf.dll** | TSF 코어 런타임 + **CUAS 호스트**. Language Bar 초기화. `imm32.dll`에 정적 링크. CUAS 변환 로직이 이 안에 통합됨. | 확인됨 [1][4] |
| **msctfime.ime** | 레거시 IMM32 IME를 CTF/TSF 세계로 노출하는 **어댑터/브리지**(ReactOS 재구현 클래스명 `CicBridge`). `ImeProcessKey`/`ImeToAsciiEx` 진입점을 `ITfThreadMgr`로 중계. **로더가 아님**. | 정황 [5][14] |
| **ctfmon.exe** | Language Bar **프론트엔드** + TIP/AUI 활성화 프로세스("CTF Loader"). XP에서 `internat.exe`를 대체. | 확인됨 [6][1] |
| **msutb.dll** | TIP Bar(Language Bar) **백엔드**. | 정황 [1][4] |

> **전제 교정 (적대적 검증 결과):** `ctfime.dll`이라는 **정확한 파일은 Windows에 존재하지 않는다**(4개 독립 DLL 인벤토리에서 0건). 지시 대상은 `msctfime.ime`(또는 함수 차원에서 `msctf.dll`)로 보아야 한다. `CtfIme*` 접두 함수군은 두 모듈에 분산된다: `CtfImeCreateThreadMgr`/`CtfImeAssociateFocus`는 `msctf.dll` export, `CtfImeEscapeEx`/`CtfImeIsIME`는 `msctfime.ime` export. "CTF IME 로더 vs IME↔CTF 어댑터" 이분법은 잘못된 프레이밍이다 — 로더는 `ctfmon.exe`, 어댑터는 `msctfime.ime`, 코어/CUAS 호스트는 `msctf.dll`. [5][14]

### 2.2 CUAS 정의와 활성화 (**확인됨** + **정황**)

- **정의:** CUAS는 구형 IMM32 기반 애플리케이션과 신형 TSF TIP을 잇는 에뮬레이션 레이어다. 구형 IMM32 IME는 TSF의 CUAS 레이어 *아래에서* 동작한다. [1]
- **활성화 조건(공식):** XP부터, 시스템 모듈 `User32.dll`/`Imm32.dll`/`Win32k.sys` + TSF 모듈 `Msctf.dll`/`Msimtf.dll`이 모두 로드될 때 활성. 단일 레지스트리 키가 아니라 런타임 모듈 로드 여부로 결정. [7]

| 시점 | CUAS 상태 | confidence |
|---|---|---|
| XP (SP1+) | disabled-by-default, 수동 활성 가능 | 정황 [4][14] |
| **Vista / Win7+** | **enabled-by-default, 모든 앱 TSF-first** | 정황 [4][14] |

> **레지스트리 키 (`CTF SFM` / `EnableCicero`)의 실재 여부는 1차 MS 문서로 확정하지 못했다(추측 영역).** Vista 이후 "always on"이라는 정황은 2차 블로그(alibabacloud/actorsfit)에만 있고 공식 1차 표현은 미발견.

### 2.3 TIP → IMM32 역변환 경로 (**확인됨 골격** / **추측 세부**)

```
[UNIM TSF TIP]
    │ ITfComposition / ITfRange::SetText / GUID_PROP_ATTRIBUTE SetValue (TSF 계약)
    ▼
[msctf.dll 내부 CUAS 브리지]  ← 닫힌 소스, ReactOS CicInputContext가 부분 재현
    │ ITfContextOwnerCompositionSink(OnStart/Update/EndComposition) 수신
    │ + IMM32 COMPOSITIONSTRING 구조체 채움
    ▼
[IMM32-only 앱: wezterm / Telegram]
    WM_IME_STARTCOMPOSITION → WM_IME_COMPOSITION(GCS_COMPSTR…→GCS_RESULTSTR) → WM_IME_ENDCOMPOSITION
    앱이 ImmGetCompositionStringW(GCS_*)로 조회
```

- **확인됨:** CUAS가 TSF↔IMM32를 변환하며 IMM32 앱이 `WM_IME_COMPOSITION` + `GCS_RESULTSTR`/`GCS_COMPSTR`(via `ImmGetCompositionString`)로 조합·결과를 받는다는 것. [1][8][10]
- **MS 1차 한계:** "CUAS가 `WM_IME_COMPOSITION`/`GCS_*`를 **합성(synthesize)**한다"고 한 문장으로 명시한 단일 MS 공식 문서는 없다. MS는 (a)"CUAS가 변환한다"(개념)와 (b)"IMM32가 이 메시지를 쓴다"(메시지 사양)를 별도 문서화하며, 둘을 잇는 합성 서술은 리버스엔지니어링(katahiromz, Wine, ReactOS)으로 교차확인됨. confidence: 메커니즘 존재=**확인됨**, "정확히 GCS_*를 합성"=**정황**. [1][9][13]
- **한글 매 키 `GCS_COMPSTR` 발행 타이밍의 바이트단위 시퀀스는 ReactOS만으로 확정 불가**(`msctfime.ime`는 레거시 cicero 재구현이며 다수 메서드가 `@unimplemented`). 실제 `msctf.dll`에 대한 Spy++/IDA 실측이 추가로 필요(**추측** 영역). [13]
- **CUAS 불완전성:** 조합/결과 문자열 변환은 견고하나 candidate list 등 풍부한 TSF 기능은 완전 노출되지 않음(MS Pinyin candidate 버그 사례). 이 때문에 `ImmDisableTextFrameService`/`ImmDisableIME` API가 존재. [11]

---

## 3. IMM32 조합 API — WM_IME_* / ImmGetCompositionString / 미확정 vs 확정

### 3.1 조합 3단계 메시지 시퀀스 (**확인됨**)

```
WM_IME_STARTCOMPOSITION   (파라미터 없음, 조합 시작 직전)
   → WM_IME_COMPOSITION   (조합 상태 변경마다 반복; lParam = GCS_*/CS_* 비트)
   → WM_IME_ENDCOMPOSITION (파라미터 없음, 조합 종료)
```
세 메시지 모두 앱이 직접 그리면 처리하고, 아니면 `DefWindowProc`로 넘기라고 명시. [10]

### 3.2 `WM_IME_COMPOSITION` lParam 플래그 — 미확정 vs 확정 (**확인됨**)

| 플래그 | 의미 | 구분 |
|---|---|---|
| `GCS_COMPSTR` | 현재 조합(미확정) 문자열 | **미확정** |
| `GCS_COMPATTR` | 조합 속성 배열(8비트/문자) | 미확정 세분화 |
| `GCS_COMPCLAUSE` | 절(clause) 경계(32비트 오프셋 배열) | 미확정 세분화 |
| `GCS_CURSORPOS` | 조합 문자열 내 커서 위치 | 미확정 |
| `GCS_DELTASTART` | 변경 시작 위치 | 미확정 |
| **`GCS_RESULTSTR`** | **조합 결과(확정) 문자열** | **확정** |
| `CS_INSERTCHAR` | 현재 삽입점에 wParam 문자 표시 | 임시 |
| `CS_NOMOVECARET` | 캐럿 이동 금지 | 임시 |

> **(검증 완료 — 이전 라운드 truncate 해소) lParam의 모든 `GCS_` 비트가 0이면 조합이 취소된 것이며, 조합 문자열을 직접 그리는 앱은 그 문자열을 삭제해야 한다.** 원문 verbatim: *"If none of the GCS_ values are set, the message indicates that the current composition has been canceled and applications that draw the composition string should delete the string."* [10][12] 구현 시 주의: 조건은 `lParam==0`이 아니라 정확히 `(lParam & GCS_마스크)==0`이다(`CS_INSERTCHAR`/`CS_NOMOVECARET`는 비-GCS 비트라 동시에 켜질 수 있음). confidence: **확인됨**.

### 3.3 `ImmGetCompositionStringW` — 2-pass 패턴 (**확인됨**)

```c
LONG ImmGetCompositionStringW(HIMC, DWORD dwIndex, LPVOID lpBuf, DWORD dwBufLen);
```
- `dwBufLen=0`으로 먼저 호출 → 필요 버퍼 **바이트** 크기 반환(유니코드여도 바이트 단위). 그 후 실제 버퍼로 재호출. [8]
- `dwIndex=GCS_COMPSTR` → 미확정 조합, `GCS_RESULTSTR` → 확정 결과.
- `ImmGetContext`로 HIMC 획득 후 `ImmReleaseContext` 필수(캐시는 release 시 제거). [8][25]

### 3.4 미확정 vs 확정의 이중 표현 + `GCS_COMPATTR` 속성 (**확인됨**)

`GCS_COMPATTR`는 조합 문자열 각 문자(DBCS는 각 바이트)마다 8비트 속성 배열. TSF `TF_DA_ATTR_INFO`와 값이 비트-동일(0..5):

| 값 | IMM32 `ATTR_*` | TSF `TF_ATTR_*` | 의미 |
|---|---|---|---|
| 0 | `ATTR_INPUT` | `TF_ATTR_INPUT` | 미변환 입력(한국어: 아직 변환 안 된 한글) |
| 1 | `ATTR_TARGET_CONVERTED` | `TF_ATTR_TARGET_CONVERTED` | 선택+변환된 현재 타깃(굵은 밑줄) |
| 2 | `ATTR_CONVERTED` | `TF_ATTR_CONVERTED` | 이미 변환됨 |
| 3 | `ATTR_TARGET_NOTCONVERTED` | `TF_ATTR_TARGET_NOTCONVERTED` | 선택됐으나 미변환 |
| 4 | `ATTR_INPUT_ERROR` | `TF_ATTR_INPUT_ERROR` | 변환 불가(예: 자음 결합 실패 — 한글 명시) |
| 5 | `ATTR_FIXEDCONVERTED` | `TF_ATTR_FIXEDCONVERTED` | 더 이상 변환 안 함 |

[26][27] — MSDN "Composition String" 속성 표는 **언어 무관**이며, 언어별 단서는 `ATTR_INPUT`의 의미만 부연한다(한국어=미변환 한글). 앱은 `ATTR_TARGET_*` 구간을 굵은 밑줄, 나머지를 가는 밑줄로 그린다(Chromium `IsTargetAttribute`/`GetCompositionUnderlines`). [26][33]

> **주의 (검증 결과 uncertain):** "한국어 한자 변환 후보 선택 단계에서 반드시 `ATTR_TARGET_CONVERTED`가 된다"는 보장되지 않는다. MS 한국어 IME는 한자 변환을 별도 candidate window로 즉시 띄우며 composition을 비울 수 있어(Scintilla #2392: *"There is no composition string or target"*), 일본어식 "composition 내부 타깃 clause 하이라이트" 모델을 항상 따르지 않는다. **API 계약상 가능 vs 실측상 미발생** 충돌 → 실기기 `GCS_COMPATTR` 덤프 필요. [26][34]

### 3.5 후보 목록 + WM_IME_CHAR + WM_IME_REQUEST (**확인됨**)

- **후보:** `WM_IME_NOTIFY(IMN_OPENCANDIDATE/CHANGECANDIDATE/CLOSECANDIDATE)` → `ImmGetCandidateListW` + `CANDIDATELIST` 구조체(`dwCount`/`dwSelection`/`dwPageStart`/`dwOffset[]`), 역시 2-pass. [28]
- **WM_IME_CHAR:** 변환 결과 문자 1개 전달. 유니코드 창에서는 `WM_CHAR`와 사실상 동일. 비유니코드 창이 `DefWindowProc`로 넘기면 lead/trail 1바이트 `WM_CHAR` 둘로 변환. [10]
- **WM_IME_REQUEST(IMR_*):** IME→앱 역질의 양방향 채널. `IMR_DOCUMENTFEED`(앱의 확정 주변 문맥 제공)/`IMR_RECONVERTSTRING`(재변환 문자열 요구)은 Wayland `text-input-v3`의 surrounding-text에 대응하는 IMM32 등가물. AutoTypeFix(한영 오타 수정) Windows 이식 시 surrounding-text 통로. [35]

### 3.6 `RECONVERTSTRING` 필드 레이아웃 (**확인됨** — 이전 "미해결" 해소)

```c
typedef struct tagRECONVERTSTRING {
  DWORD dwSize;            // 구조체 + 메모리 블록 전체 크기
  DWORD dwVersion;         // 0 고정
  DWORD dwStrLen;          // 전체 문자열 길이
  DWORD dwStrOffset;       // 이 구조체 시작 기준 오프셋
  DWORD dwCompStrLen;      // composition이 될 문자열 길이
  DWORD dwCompStrOffset;   // (dwStrOffset 기준 상대)
  DWORD dwTargetStrLen;    // target clause 길이
  DWORD dwTargetStrOffset; // (dwStrOffset 기준 상대)
} RECONVERTSTRING;
```
**유니코드 IME에서 `*StrLen`은 문자 수(TCHAR), `*StrOffset`은 바이트 수.** `dwCompStrOffset`/`dwTargetStrOffset`은 `dwStrOffset` 기준 상대. [36] mozc `src/win32/base/imm_reconvert_string.h`의 `ComposeReconvertString`/안전 파싱 헬퍼가 AutoTypeFix surrounding-text 구현 참조. [37]

> IME-unaware 앱이 IME 메시지를 무시(`DefWindowProc`)하면 OS가 기본 IME 창을 띄우고 확정 결과를 `WM_CHAR`로 번역 — 역설적으로 "아무것도 안 하는" 메모장이 가장 호환성 높음. [25]

---

## 4. TSF TIP 계약 — display attribute, composition sink, sync/async, "즉시 종료" 근본 원인

### 4.1 미확정 composition 정석 5단계 (**확인됨**)

1. read/write edit session 안에서 `ITfRange` 확보
2. `ITfContext::GetProperty(GUID_PROP_ATTRIBUTE)` → `ITfProperty` 획득
3. `ITfCategoryMgr::RegisterGUID(input GUID)` → `TfGuidAtom` (1회 등록·캐시)
4. `VARIANT{vt=VT_I4, lVal=atom}` 구성
5. `ITfProperty::SetValue(ec, range, &var)` — **read/write edit session 안에서만 가능**

[18][19] — 속성을 attribute property로 다는 행위 자체가 CUAS에게 "이 range는 미확정 조합"이라는 신호다.

### 4.2 4종 sink/인터페이스 매핑 표 — "누가 OnCompositionTerminated를 호출하는가" (**확인됨**)

| 인터페이스 | 구현 주체 | 핵심 메서드 | 역할 |
|---|---|---|---|
| **`ITfCompositionSink`** | **TIP (UNIM `comp_sink`)** | `OnCompositionTerminated` | TIP이 "내가 아닌 누군가가 composition을 끝냈다"는 통지를 **수신** |
| **`ITfContextOwnerCompositionSink`** | **앱/문서 owner (CUAS 더미 스토어)** | `OnStartComposition`/`OnUpdateComposition`/`OnEndComposition` | owner가 composition을 관찰하고 **거부/종료시키는 주체** |
| `ITfContextOwnerCompositionServices` | 매니저/owner | `TerminateComposition` | composition 강제 종료 API |
| `ITfEditSession` | TIP | `DoEditSession` | 락 안에서 문서 조작 |

[3][38] — **결정적 계약:** `ITfContextComposition::StartComposition`은 owner가 advise sink를 설치했으면 `OnStartComposition`을 호출하고, **owner가 거부하면 `S_OK`를 반환하되 `ppComposition`을 `NULL`로 설정**한다. [38] 즉 CUAS 더미 owner가 StartComposition을 즉시 거부하거나 직후 종료시키는 것이 즉시-terminate의 가장 구조적인 메커니즘이다. `OnCompositionTerminated`는 "TIP 외의 주체가 종료"할 때만 오므로, StartComposition 직후 즉시 오면 그것은 **정상 흐름이 아니라 CUAS owner의 거부 신호**다. [3]

### 4.3 sync vs async — TF_ES_ 상수 (**확인됨**)

| 상수 | 의미 |
|---|---|
| `TF_ES_SYNC` (0x1) | 키처리 등 "성공이 보장되는 문서화된 상황"에서만. 실패 시 `TF_E_SYNCHRONOUS` 반환. |
| `TF_ES_ASYNCDONTCARE` (0) | 매니저 재량(sync 시도 후 거부되면 async 폴백). **더 안전한 정석 기본값** |

> **MS "Rules of Text Services" verbatim:** *"두 번째 규칙: 동기 edit session을 피하라. Microsoft Word 같은 다수 text store는 동기 세션을 절대 grant하지 않는다... 이 가정으로 설계를 시작하면 Word 테스트에서 아무것도 안 되는 고통스러운 재설계를 피할 수 있다."* *"세 번째 규칙: 핸들러는 edit session들로 구성되며, edit session은 보통 콜백을 발화하며 끝난다."* [24][20] mozc는 issue #821에서 이 이유로 동기 세션 의존을 폐기하는 재설계 진행. [23]

- **실증(mozc #819):** 정상 TIP도 Word가 "First Line Indent" 핸들 조작 후 내부 상태가 영구 변경되면 키처리 컨텍스트 안에서도 `TF_ES_READWRITE|TF_ES_SYNC`를 `TF_E_SYNCHRONOUS`로 거부 → 키 이벤트 처리 영구 중단. [22] sync 의존은 환경에 따라 취약.

### 4.4 락 모델 정석 (Chromium + Mozilla) (**확인됨**)

- **RequestLock(Chromium):** 락 미보유 시 동기 grant(`OnLockGranted`). 락 보유 중이면 동기 요청(`TS_LF_SYNC`)은 `TS_E_SYNCHRONOUS` 거부, 비동기는 큐에 push 후 `TS_S_ASYNC` 반환, unlock 직후 큐 flush. [39]
- **락 안에서 즉시 편집 금지:** pending action으로 모았다가 unlock 시 `FlushPendingActions`로 flush. CUAS/TIP가 락 중 commit을 시도해도 캐시를 유지(`mDeferClearingContentForTSF`). 재진입 가드(`is_notification_in_progress_`)로 무한 알림 루프 차단. [40][41]
- **`GetTextExt` → `TS_E_NOLAYOUT` 함정:** 다수 TIP가 NOLAYOUT 수신 시 작업 중단/후보창을 화면 좌상단으로 보냄. Mozilla는 활성 TIP별로 NOLAYOUT을 절대 반환하지 않는 per-TIP 해킹(MS-IME JP, ATOK, WeChat, Wubi; `intl.tsf.hack.*`). 터미널/커스텀 호스트는 레이아웃 rect 계산이 늦어 NOLAYOUT 반환하기 쉬움 → composition 끊김 1차 원인 후보. [40]

### 4.5 "즉시 종료" 근본 원인 종합 (**확인됨**)

```
wezterm/Telegram: ITextStoreACP 미구현 (Cicero-unaware)
   → CUAS가 더미 text store 제공 + context owner를 CUAS가 소유
   → StartComposition 직후 owner(CUAS)가 composition 거부/종료
   → ITfCompositionSink::OnCompositionTerminated 즉시 호출
   → (과거 UNIM) 핸들러가 engine.reset() 자폭 [수정됨]
```
caret style·display attribute 가설은 모두 실측으로 기각됨(§8). 근본은 CUAS owner-side 종료. [3][16][21]

---

## 5. 레퍼런스 IME 교훈 — mozc / weasel / Mozilla / Chromium

### 5.1 mozc — 프레임워크-중립 공유 브리지 (**확인됨**)
- `src/win32/base/keyevent_handler.cc`의 `KeyEventHandler::ImeProcessKey`/`ImeToAsciiEx`가 `client::ClientInterface`로 엔진과 통신. TSF front-end(`tip/`)와 과거 IMM32 IME가 같은 코어 호출. [42]
- **현행 master는 IMM32 IME(`src/win32/ime`)를 완전 제거**(commit `bc72121c8`, 2023-03-11, 37파일 -11579줄 삭제, TSF-only). 빌드 옵션 잔존 아님 — 빌드 그래프에서 참조 제거. 커밋 메시지: *"XP/Vista/7에서만 IMM32 모듈로 설치, 이후 모든 OS는 TSF 모듈 → 사용자 영향 없이 삭제 가능."* IMM32 앱 호환은 CUAS에 위임. [43][44]
  - **함정:** `chromium.googlesource.com/external/mozc` 미러는 2015년 스냅샷(stale)이라 `src/win32/ime`가 보이나 현재와 무관. `base/`에 `imm_util.cc`/`imm_reconvert_string.cc`가 남은 것은 TSF 경로의 reconversion 보조 코드이지 IMM32 IME가 아님. [44][37]
- `src/win32/base/deleter.h`의 `VKBackBasedDeleter` 상태머신으로 CUAS/legacy 앱 backspace 조합 복원 안전 처리. [45]

### 5.2 weasel(rime) — CUAS 워크어라운드 (**확인됨**, 단 목적 주의)
- `DisplayAttribute.cpp`: `CategoryMgr->RegisterGUID(c_guidDisplayAttributeInput, &_gaDisplayAttributeInput)` → `GUID_PROP_ATTRIBUTE`에 `VT_I4=atom` SetValue. `DisplayAttributeInfo` = `{TF_LS_DOT, FALSE, TF_ATTR_INPUT}`. [46]
- **ZWSP/공백 SetText 워크어라운드(`Composition.cpp`)** — *"CUAS does not provide a correct GetTextExt() position unless the composition is filled with characters"*. **목적은 후보창 좌표(candidate window position)이지 조합 끊김(termination) 복구가 아니다.** PR #883의 4커밋(do not select text / dotted underline / candidate list position)도 모두 후보창·밑줄 관련. [47][48]
- **조합 끊김 실제 대응은 별개 commit `124fc94`** "fix(tsf): do not reset composition on document focus set" — `OnSetFocus`에서 `_AbortComposition()` 한 줄 제거. **UNIM commit `8b67db9`가 포팅한 것이 바로 이 OnSetFocus 패턴이며 ZWSP가 아니다.** [49]

### 5.3 Mozilla(Gecko) — display attribute + 한국어 commit 순서 (**확인됨**)
- `TSFTextStore.cpp`: *"Getting display attributes is **really** complicated!"* — `GetProperty(GUID_PROP_ATTRIBUTE)` → `EnumRanges`로 "서로 다른 값의 구간" 분할 → 각 구간을 `TF_DISPLAYATTRIBUTE`로 변환. `bAttr`→clause type 매핑: `TF_ATTR_TARGET_CONVERTED`→eSelectedClause 등. [31][32]
- **한국어 IME commit 순서 함정(`IMMHandler.cpp`):** *"Korean IME posts WM_IME_ENDCOMPOSITION first when we hit space during composition. Then, we should ignore the message and commit the composition string at following WM_IME_COMPOSITION."* `PeekMessage(PM_NOREMOVE)`로 다음 메시지가 `WM_IME_COMPOSITION` + `IS_COMMITTING_LPARAM`이면 ENDCOMPOSITION 무시. ChangJie 등은 빈 문자열 시 COMPOSITION 없이 ENDCOMPOSITION만 보냄. [50]
- **CUAS 탐지 실전 패턴(Bug 866736):** TSF 모드가 아니면 `GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, msctf.dll)`로 msctf.dll 로드(=CUAS 활성) 확인 후 full TextStore 없이 `SetInputScopes`만 지원. [2]
- **듀얼 스택:** TSF 모드에서도 활성 IME가 IMM이면 `IMMHandler` 병행 사용(*"Even if we are in the TSF mode, the active IME may be IMM"*). commit 시 `TSFTextStore::CommitComposition` + `IMMHandler::CommitComposition` 둘 다 시도. [51]

### 5.4 Chromium — TSF/IMM32 모범 (**확인됨**)
- `ui/base/ime/win/tsf_text_store.cc`: `ITextStoreACP` 직접 구현 → 한국어 조합 안정적(Electron 동일). [52]
- `imm32_manager.cc`: **CUAS on/off에 따라 후보창 배치가 정반대** — CUAS off 시 중/일 IME가 `ImmSetCandidateWindow`를 무시하고 `GetCaretPos` 사용(임시 시스템 caret 생성), CUAS on 시 반대. `IsTargetAttribute()` = target clause에 thick underline. [33]

---

## 6. 앱별 IME 스택 분류 표

| 앱 | 입력 스택 | 경로 | 한글 조합 안정성 | confidence |
|---|---|---|---|---|
| **wezterm** | IMM32 직접(`ImmGetCompositionStringW`), TSF 미구현 | CUAS 브리지 | 불안정(즉시 종료/사라짐) | **확인됨** [15][16] |
| **Telegram Desktop** | Qt `QWindowsInputContext`(IMM32) | CUAS 브리지 | 불안정(스페이스 시 직전 글자 삭제 #29743) | **확인됨** [53][54] |
| 메모장/클래식 Win32 edit | IME-unaware, `DefWindowProc` 위임 | OS 기본 IME 창 | **가장 안정적** | 확인됨 [25][10] |
| Chromium/Electron/Edge | `ITextStoreACP` 직접(`tsf_text_store.cc`) | TSF 네이티브 | 안정적 | 확인됨 [52] |
| Firefox | `IMMHandler` + `TSFTextStore` 듀얼 | 둘 다 | 안정적(이원 처리) | 확인됨 [51][50] |
| 터미널(conhost/WT/wezterm) | 셀 격자 + 커서 좌표 모델 | (앱별) | 구조적 취약(좌표 변환) | 확인됨 [55][56] |

- **wezterm 1차 근거:** `window/src/os/windows/window.rs` L673-679에서 `WM_IME_STARTCOMPOSITION/COMPOSITION/ENDCOMPOSITION`을 wnd_proc에서 직접 처리, `ImmContext`가 HIMC를 RAII로 감쌈. Windows에서 IME는 "Always enabled, cannot be disabled". [15]
- **Telegram/Qt 1차 근거:** `qtbase/src/plugins/platforms/windows/qwindowsinputcontext.cpp`가 `ImmGetCompositionString`/`ImmReleaseContext` 직접 사용 + 한국어 전용 분기(QTBUG-58300: *"Ignore WM_IME_ENDCOMPOSITION when CTRL is pressed if language == Korean"*). [53]
- **터미널 구조적 난점:** IME는 화면 좌표로 candidate 위치를 요구하나 터미널은 셀→클라이언트→화면 좌표 + DPI 스케일 변환 필요(WT PR #1919 "IME UI does not follow the cursor"). 셀 격자 모델이라 인플레이스 부분 조합 표시가 모델과 충돌. [56][55]

> **주의:** "WM_IME_* 발생 = IMM32-only"는 위험한 추론이다. CUAS가 TSF TIP 동작을 IMM32 메시지로 합성하므로 IMM32 메시지가 보여도 실제 IME는 TSF TIP일 수 있다. 진짜 TSF-aware 판별은 `ImmDisableTextFrameService` 호출/`ITfThreadMgr` 활성/`ITextStoreACP` 구현 여부를 함께 확인해야 함. [2][16]

---

## 7. Mozilla·Chromium 소스에서 추출한 실전 함정/우회책

1. **GetTextExt NOLAYOUT 함정** — 터미널/커스텀 호스트는 NOLAYOUT 반환하기 쉽고, 다수 TIP가 이때 composition 포기. 우회: caret/첫 글자 rect라도 항상 유효 rect 반환. (Mozilla `MaybeHackNoErrorLayoutBugs`, per-TIP `intl.tsf.hack.*`) [40]
2. **락 안 즉시 편집 금지** — pending action으로 모아 unlock 시 flush(재진입/무한 루프 회피). [40][41][39]
3. **한국어 스페이스 commit 순서** — ENDCOMPOSITION 선행, 뒤따르는 COMPOSITION(commit)까지 대기. ENDCOMPOSITION에서 즉시 commit 금지. [50]
4. **단일 밑줄만 그리면 target clause 강조 누락** — `bAttr`→clause type 매핑(target은 thick). [32][33]
5. **CUAS on/off 후보창 배치 정반대** — `GetCaretPos` vs `ImmSetCandidateWindow`. [33]
6. **색 type이 `TF_CT_NONE`이면 색을 건드리지 말 것**(앱 기본값 위임). [31]
7. **GPU 렌더 터미널에서 display attribute 색을 앱 위임하면 underline 미표시 가능**(렌더러가 TSF display attribute 무시). (**추측** — 미검증)

---

## 8. UNIM 적용 — 현재 구현(파일:라인) 대비 근본 원인 + 수정 방향

### 8.1 현재 HEAD 기준 구현 현황 — 단일 진실 표 (**확인됨**, 직접 코드 확인)

> **plan doc의 "Phase 3 = SetValue 0건" 주장은 작성 시점 스냅샷이며 현재 코드와 불일치한다.** commit `8b67db9` 이후 SetValue가 반영됨. 아래가 현재 HEAD 진실.

| 항목 | 현황 | 위치 |
|---|---|---|
| `GUID_PROP_ATTRIBUTE` SetValue | **구현됨**(start/update/replace 3곳) | `composition.rs:25-39`, 호출 381/410/517 |
| `RegisterGUID` → atom 캐시(1회) | **구현됨** | `composition.rs:179` |
| display attribute provider | **구현됨**(`ITfDisplayAttributeProvider`) | `text_service.rs:519-535` |
| Input display attr | `TF_LS_SOLID`(crLine 0x00C86400) / `TF_ATTR_INPUT(0)` | `display_attr.rs:41-49` |
| Converted display attr | `TF_LS_NONE` / `TF_ATTR_TARGET_CONVERTED(2)` | `display_attr.rs:98-104` |
| `RegisterCategory(DISPLAYATTRIBUTEPROVIDER)` | wxs + register.rs | `register.rs:145` |
| composition range selection | `select_composition_range` = `TF_AE_NONE` 전체 range(SampleIME식) | `composition.rs:72-77`, 호출 377/407 |
| `move_caret_to_end` (Collapse END 0폭) | commit/replace 경로에만(조합 수명 무관) | `composition.rs:47-53`, 호출 436/444/504/543 |
| **RW EditSession `TF_ES_SYNC` (미전환)** | **6곳 잔존** | `composition.rs:205/233/248/264/277/310` (+ RO 649) |
| `OnCompositionTerminated` | **`engine.reset()` 제거됨**; `<200ms` immediate만 폴백 진입 | `text_service.rs:395-431`(임계 411=200ms) |
| 정상 종료 정리(`engine.reset()`) | `OnSetFocus`로 이전 + sticky 해제 | `text_service.rs:444-456` |
| 폴백(delete+reinsert, backspace 미합성) | `replace_surrounding`(ShiftStart+SetText) | `key_handler.rs:289-324`, `composition.rs:474-524` |
| ZWSP/더미 텍스트 | **없음**(grep 0건) | — |
| `acquire_insert_range` 폴백 | InsertTextAtSelection(QUERYONLY) → GetSelection | `composition.rs:95-135` |

### 8.2 근본 원인 후보 (검증 종합)

- **확정:** 환경(wezterm/Telegram의 `ITextStoreACP` 미구현 + CUAS owner-side 종료)이 즉시-terminate의 직접 트리거. 순수 TSF inline composition은 이 앱들에서 구조적으로 유지 불가. [3][16][21][38]
- **기각된 가설:** caret 0폭 collapse(실측 100% 동일 재현 [21]), display attribute 미등록(이미 구현 [29]), SYNC 거부 로그(실측 전부 `hr=Ok(0)`, 거부 기록 없음 [검증]).
- **남은 갭:** 6곳 `TF_ES_SYNC`(정석 위반 [24]), 200ms 매직넘버(HWND/스레드 단위 캐시 아님 — 오탐 여지), 폴백 모드 inline 미확정 밑줄 부재.

### 8.3 수정 방향 (우선순위)

**P0 — 이미 반영됨 (검증):** `OnCompositionTerminated` reset 자폭 제거 + `OnSetFocus` 이전(`8b67db9`). [29][30][49]

**P1 (저위험, 코어 무변경, Linux 회귀 0):**
- 200ms 매직넘버를 **HWND/스레드 단위 캐시**로 승격 — 느린 머신/원격 데스크톱에서 정상 앱(메모장)의 지연된 정당한 terminate를 CUAS 즉시-terminate로 오탐 방지.
- `set_composition_attribute`의 `let _ =` SetValue 실패를 **HRESULT 로깅** + 직후 `GetValue`로 atom 재확인(attribute가 실제 range에 박혔는지 런타임 검증). CUAS에서 attribute property가 비면 range를 result string으로 오인할 수 있음(A/B 위장 분리). [검증]

**P2 (고위험, 별도 PR — 정석이나 재설계 필요):**
- **`TF_ES_SYNC` → `TF_ES_ASYNCDONTCARE` 전환.** 정석(SampleIME/weasel/mozc) + MS Rules of Text Services [24] + mozc #821 [23]. **단 `start_composition`(L212)·`replace_surrounding`(L315)의 `composition_slot.lock().take()` 동기 회수 패턴이 차단 요인.**
  - 재설계안: `ITfComposition`을 `DoEditSession` 내부에서 `Arc<Mutex>` 공유 슬롯에 저장, 호출부는 즉시 `take()` 금지. 핸들러는 콜백 발화로 끝남(MS 세 번째 규칙 [24]).
  - **잔존 리스크(plan 미해결 5번):** ASYNC deferred `DoEditSession`이 다음 `OnKeyDown` 전 완료된다는 보장 미확인 → 빈 슬롯 race. mozc가 #821에서 이를 푼 구체 deferred-콜백 패턴은 소스 레벨로 추가 확보 필요. [23]

**비권장:** 폴백을 IMM32 직접 주입(`ImmSetCompositionString`)으로 — plan이 "최후수단"으로 기각(TIP 설계 외 + CUAS 이중 충돌).

### 8.4 레이턴시 측정 계획 (**미측정** — 데이터 부재 명시)
UNIM 핵심 제약 `<10ms`에 대한 정량 데이터가 7차원 어디에도 없음. VM에서 다음을 실측해 표로:
- 키당 EditSession round-trip 시간
- `SetText` + `select_composition_range` + `set_composition_attribute` 누적 시간
- SYNC vs (장차)ASYNC 비용 비교

plan doc VM 체크리스트에 타이밍 로깅 항목 추가 권고.

### 8.5 재현 매트릭스 (검증 절차)
| 앱 \ 키시퀀스 | 한글1자 | 한글2자+스페이스 | 한글+백스페이스 |
|---|---|---|---|
| 메모장(IME-unaware) | OK 기대 | OK 기대 | OK 기대 |
| wezterm | 끊김 의심 | 직전삭제/끊김 의심 | ? |
| Telegram | 직전삭제(#29743류) 의심 | 직전삭제 의심 | ? |

가설: 메모장 OK / wezterm·Telegram NG의 분기점은 "앱이 IMM32 메시지를 직접 가로채는가". 검증 시 분리 로그(SetValue HRESULT / GetValue atom 재확인 / OnCompositionTerminated 호출 시점 타임스탬프) 필수.

---

## 9. 검증 결과 요약 표

| # | 핵심 주장 | 판정 | confidence |
|---|---|---|---|
| 1 | CUAS의 정확한 한글 `GCS_COMPSTR` 발행 시퀀스를 ReactOS만으로 확정 | **uncertain** | 개념=확인됨, 바이트 시퀀스=실측 필요 |
| 2 | wezterm/Telegram이 IMM32-only(CUAS 경로) | **confirmed** | 높음(소스 확인) |
| 3 | `ctfime.dll`의 정확한 역할이 미확정 | **uncertain** | 파일명 오기(=`msctfime.ime`), 어댑터로 수렴 |
| 4 | lParam 모든 GCS_ 비트 0 = 조합 취소, 삭제 의무 | **confirmed** | 높음(원문 verbatim) |
| 5 | CUAS가 TSF composition을 `WM_IME_COMPOSITION`/`GCS_*`로 번역 | **confirmed** | 메커니즘 높음, MS 1차 합성 서술=정황 |
| 6 | 한국어 한자 변환 = `ATTR_TARGET_CONVERTED` | **uncertain** | 속성 정의=확인됨, 실제 set=충돌 |
| 7 | CUAS `bAttr`→`GCS_COMPATTR` 1:1 변환을 1차/소스 주석에서 직접 확인 | **refuted** | 높음(값 동일은 정황, 변환 계약은 닫힌 소스) |
| 8 | wezterm 즉시 종료 트리거 = 0폭 caret collapse | **refuted** | 높음(실측 100% 동일 재현 + SampleIME 반증) |
| 9 | UNIM 6곳 SYNC가 컨텍스트 밖 호출로 거부됨 | **refuted** | 높음(코드 구조 + 로그 전부 Ok(0)) |
| 10 | weasel ZWSP가 UNIM 조합 끊김을 해결 / UNIM이 더미 텍스트 사용 | **refuted** | 높음(ZWSP=후보창용, UNIM ZWSP 0건) |
| 11 | mozc master가 IMM32 제거된 TSF-only | **confirmed** | 0.95(트리 부재+삭제 커밋+BUILD 부재 3중) |
| 12 | UNIM 끊김이 display attr 미등록 vs lifecycle 중 어느쪽 | **uncertain** | 메모장=lifecycle, CUAS=A 무음실패 위장 가능 |

---

## 10. 참고문헌

1. katahiromz/ImeStudy — Windows IME/IMM Study: https://github.com/katahiromz/ImeStudy
2. Mozilla Bug 866736 — InputScope support for IMM32 with CUAS: https://bugzilla.mozilla.org/show_bug.cgi?id=866736
3. ITfContextOwnerCompositionSink interface (msctf.h): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontextownercompositionsink
4. msctf.dll process info: https://www.file.net/process/msctf.dll.html
5. ReactOS msctfime (CicBridge): https://doxygen.reactos.org/d5/d28/msctfime_8cpp.html
6. Text Services Framework — Wikipedia: https://en.wikipedia.org/wiki/Text_Services_Framework
7. ImmDisableTextFrameService function (imm.h): https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immdisabletextframeservice
8. ImmGetCompositionStringW function (imm.h): https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immgetcompositionstringw
9. Wine imm32 reimplementation: https://github.com/wine-mirror/wine/blob/master/dlls/imm32/imm.c
10. WM_IME_COMPOSITION message: https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition
11. ImmDisableIME function: https://learn.microsoft.com/en-us/windows/desktop/api/imm/nf-imm-immdisableime
12. WM_IME_COMPOSITION raw source (verbatim "none of GCS_"): https://raw.githubusercontent.com/MicrosoftDocs/win32/docs/desktop-src/Intl/wm-ime-composition.md
13. ReactOS inputcontext (CicInputContext / SetCompositionString): https://doxygen.reactos.org/d1/d32/inputcontext_8cpp_source.html
14. katahiromz ImeStudy README: https://raw.githubusercontent.com/katahiromz/ImeStudy/main/README.md
15. wezterm Windows IMM32 (window.rs L673-679): https://github.com/wezterm/wezterm/blob/577474d8/window/src/os/windows/window.rs#L673-L679
16. wezterm Windows integration (DeepWiki): https://deepwiki.com/wezterm/wezterm/4.3-windows-integration
17. Microsoft Windows-classic-samples SampleIME Composition.cpp: https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/Composition.cpp
18. Providing Display Attributes (TSF): https://learn.microsoft.com/en-us/windows/win32/tsf/providing-display-attributes
19. Compositions (TSF): https://learn.microsoft.com/en-us/windows/win32/tsf/compositions
20. TF_ES_ Constants: https://learn.microsoft.com/en-us/windows/win32/tsf/tf-es--constants
21. UNIM wezterm-composition-research.md (caret 가설 기각 L146-149): C:\Users\USER\Desktop\work\unim\docs\wezterm-composition-research.md
22. mozc issue #819 (Word sync 거부): https://github.com/google/mozc/issues/819
23. mozc issue #821 (SYNC→ASYNC 재설계): https://github.com/google/mozc/issues/821
24. Rules of Text Services - TSF Aware (Avoid Synchronous edit sessions): https://learn.microsoft.com/en-us/archive/blogs/tsfaware/rules-of-text-services
25. Status, Composition, and Candidates Windows: https://learn.microsoft.com/en-us/windows/win32/intl/status--composition--and-candidates-windows
26. Composition String (GCS_COMPATTR 속성): https://learn.microsoft.com/en-us/windows/win32/intl/composition-string
27. TF_DA_ATTR_INFO enum: https://learn.microsoft.com/en-us/windows/win32/api/msctf/ne-msctf-tf_da_attr_info
28. CANDIDATELIST structure: https://learn.microsoft.com/en-us/windows/win32/api/imm/ns-imm-candidatelist
29. UNIM composition.rs (SetValue/SYNC/slot.take 현황): C:\Users\USER\Desktop\work\unim\unim-tsf\src\composition.rs
30. UNIM text_service.rs (OnCompositionTerminated/OnSetFocus): C:\Users\USER\Desktop\work\unim\unim-tsf\src\text_service.rs
31. Mozilla TSFTextStore.cpp (display attr "really complicated"): https://searchfox.org/mozilla-central/source/widget/windows/TSFTextStore.cpp
32. Mozilla TSFTextStore.cpp raw (GetGeckoSelectionValue bAttr→clause): https://hg.mozilla.org/mozilla-central/raw-file/tip/widget/windows/TSFTextStore.cpp
33. Chromium imm32_manager.cc (CUAS on/off 후보창, IsTargetAttribute): https://chromium.googlesource.com/chromium/src/+/5e16d46c92747ae76914b9a5db114a19cdb00bde/ui/base/ime/win/imm32_manager.cc
34. Scintilla bug #2392 (한국어 한자 변환 "no composition string or target"): https://sourceforge.net/p/scintilla/bugs/2392/
35. IMR_DOCUMENTFEED notification: https://learn.microsoft.com/en-us/windows/win32/intl/imr-documentfeed
36. RECONVERTSTRING structure (immdev.h): https://learn.microsoft.com/en-us/windows/win32/api/immdev/ns-immdev-reconvertstring
37. mozc imm_reconvert_string.h: https://github.com/google/mozc/blob/master/src/win32/base/imm_reconvert_string.h
38. ITfContextComposition::StartComposition (owner 거부 시 S_OK + ppComposition NULL): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition
39. Chromium tsf_text_store.cc (RequestLock 동기 grant + lock_queue): https://chromium.googlesource.com/chromium/src/+/HEAD/ui/base/ime/win/tsf_text_store.cc
40. Mozilla TSFTextStore.cpp (MaybeHackNoErrorLayoutBugs / FlushPendingActions): https://hg.mozilla.org/mozilla-central/raw-file/tip/widget/windows/TSFTextStore.cpp
41. ITextStoreACP::RequestLock: https://learn.microsoft.com/en-us/windows/win32/api/textstor/nf-textstor-itextstoreacp-requestlock
42. mozc keyevent_handler.cc (공유 브리지): https://github.com/google/mozc/blob/master/src/win32/base/keyevent_handler.cc
43. mozc commit bc72121c8 (Delete win32/ime): https://github.com/google/mozc/commit/bc72121c8
44. mozc git-trees API (master TSF-only 확인): https://api.github.com/repos/google/mozc/git/trees/master?recursive=1
45. mozc deleter.h (VKBackBasedDeleter): https://github.com/google/mozc/blob/master/src/win32/base/deleter.h
46. weasel DisplayAttribute.cpp: https://github.com/rime/weasel/blob/master/WeaselTSF/DisplayAttribute.cpp
47. weasel Composition.cpp (ZWSP=후보창 좌표용): https://raw.githubusercontent.com/rime/weasel/master/WeaselTSF/Composition.cpp
48. weasel PR #883: https://github.com/rime/weasel/pull/883
49. weasel commit 124fc94 (do not reset composition on focus set): https://github.com/rime/weasel/commit/124fc9475c30963a9bbbf9a097b452b52e8ab658
50. Mozilla IMMHandler.cpp (Korean IME ENDCOMPOSITION 선행): https://searchfox.org/mozilla-central/source/widget/windows/IMMHandler.cpp
51. Firefox IME handling guide (Gecko): https://firefox-source-docs.mozilla.org/editor/IMEHandlingGuide.html
52. Chromium tsf_text_store.h (ITextStoreACP): https://chromium.googlesource.com/chromium/src/+/lkgr/ui/base/ime/win/tsf_text_store.h
53. Qt qwindowsinputcontext.cpp (IMM32 + 한국어 분기): https://github.com/qt/qtbase/blob/dev/src/plugins/platforms/windows/qwindowsinputcontext.cpp
54. Telegram #29743 (한국어 스페이스 직전 글자 삭제): https://github.com/telegramdesktop/tdesktop/issues/29743
55. Windows Terminal #2213 (All IMEs do not work / CJK): https://github.com/microsoft/terminal/issues/2213
56. Windows Terminal PR #1919 (IME UI does not follow the cursor): https://github.com/microsoft/terminal/pull/1919
57. UNIM wezterm-cuas-composition-plan.md (Phase 0~4): C:\Users\USER\Desktop\work\unim\docs\wezterm-cuas-composition-plan.md
58. UNIM key_handler.rs (폴백 분기): C:\Users\USER\Desktop\work\unim\unim-tsf\src\key_handler.rs
59. UNIM display_attr.rs (Input/Converted display attr): C:\Users\USER\Desktop\work\unim\unim-tsf\src\display_attr.rs
60. UNIM register.rs (RegisterCategory): C:\Users\USER\Desktop\work\unim\unim-tsf\src\register.rs

---

*문서 종합 기준일: 2026-06. 추측 표기 항목은 Spy++/IDA 실측 또는 Windows VM 런타임 검증으로 후속 확정 필요.*
