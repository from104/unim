# TSF ↔ 앱 연결 "모드" — 앱의 IME 인지 수준 분류 (유지보수 레퍼런스)

> 질문: TSF가 앱과 처음 연결될 때 몇 가지 상태(모드)로 연결되나? (메모장=UI필드, wezterm=터미널, 게임=raw)
> 답: 정확한 분류 축은 "UI 종류"가 아니라 **앱이 텍스트 입력을 어떻게 받는가(IME-awareness)** 다.
> TIP(우리) 입장에선 항상 `ITfContext` 하나를 받지만, 그 컨텍스트의 **실체와 능력**이 다르다.

## 전제: TIP은 "모드 플래그"를 직접 못 받는다
포커스가 문서로 이동하면 `ITfThreadMgr`가 `ITfDocumentMgr`→`ITfContext`를 만들어 TIP에 준다.
**모든 앱에서 동일한 인터페이스(ITfContext)를 받는다.** 차이는 그 컨텍스트의 *뒷단(backing)*과
*지원 능력*에 있다. "이 앱은 무슨 모드"라는 깔끔한 질의 API는 없고, IME는 **능력을 시도해보고
경험적으로 적응**한다(우리가 겪은 여정의 근본 이유).

## 4가지 실질 범주

### ① TSF-aware (앱이 `ITextStoreACP` 직접 구현) — "진짜 TSF"
- 앱이 자기 문서를 text store로 노출. TIP이 그 store를 직접 읽고 쓴다(range 편집·GetTextExt·
  InsertAtSelection 다 동작). composition이 **앱 문서 안에서 네이티브 inline**.
- 예: 브라우저(Chromium/Firefox는 자체 TSF text store 구현), WPF/UWP 텍스트 컨트롤, RichEdit(TSF 모드),
  최신 MS Office, Win11 새 메모장 등 리치 컨트롤.
- UNIM: 정상 composition 경로 그대로 동작. inline 공짜. (우리가 처음부터 잘 되던 환경.)

### ② IMM32-only (TSF 미인지) — **CUAS가 다리를 놓음**
- 앱은 IMM32(WM_IME_* 메시지)만 안다. `ITextStoreACP` 미구현. msctf 내장 **CUAS(Cicero
  Unaware App Support)** 가 default/emulated text store와 컨텍스트 소유자를 대행하고, TIP의 TSF
  composition을 `WM_IME_COMPOSITION`(GCS_COMPSTR=미확정 / GCS_RESULTSTR=확정)으로 역브리지.
- 예: 고전 Win32 `EDIT` 컨트롤, **wezterm**(GUI 창 + 자체 IMM32 처리), Telegram류.
- inline 여부 = (앱이 GCS_COMPSTR을 받아 자기 폰트로 그리는가) **AND** (TIP이 CUAS 호환으로
  composing 신호를 주는가). 후자가 우리가 막혔던 지점.
- UNIM: 이 범주가 핵심 난제였다. 동작 조건(레퍼런스 한국어 TSF에서 발굴):
  - `TF_SELECTIONSTYLE.fInterimChar=TRUE` (조합 중 selection) — **CUAS가 미확정으로 인식하는 결정 신호**
  - non-empty range 위 StartComposition (빈 range는 즉시 OnCompositionTerminated)
  - 단일 edit session, 음절 전환 end+start 병합(`commit_and_restart`)
  - Enter/화살표 등 nav 키는 OnTestKeyDown에서 확정 후 pIsEaten=FALSE로 통과
  - 그래도 종료되면 client-side preedit 오버레이로 폴백(non-sticky: 다음 단어 inline 복구)

### ③ 콘솔 서브시스템 (classic conhost) — **CUAS 적용 제외, 별도 경로**
- MS Learn(CUAS) 명문: "all non-TSF apps **except 16-bit and console window applications**".
- 진짜 콘솔(cmd/PowerShell의 고전 conhost 화면)은 conhost 자체 콘솔 IME 경로를 탄다 — CUAS 아님.
- 주의: **"터미널"이라고 다 콘솔이 아니다.** wezterm은 GUI 창이라 ②번(IMM32/CUAS)에 해당.
  Windows Terminal은 자체 TSF text store(TSFInputControl)를 구현해 ①번에 가깝다.
  → "터미널 기반"이라는 단일 범주는 틀린 모델. **그 터미널의 구현이 뭐냐**가 전부.
- UNIM: classic conhost는 우리가 미검증. 거동이 ②와 다를 수 있어 향후 별도 확인 대상.

### ④ IMM32 조합 수락 + 자체 렌더 없음 — IME가 기본 조합창을 그림 (게임 채팅 등)
- 앱이 IMM32 조합(`WM_IME_COMPOSITION`)은 **수락하지만, 직접 그리지도·위치를 알려주지도 않음.**
  DX12/Vulkan GPU 렌더라 자체 텍스트 위젯이 없는 게임 인게임 채팅이 전형.
- 이때 preedit는 **IME 자신이 그리는 기본 조합창**으로 뜬다(앱이 아니라 IME/OS가 그림).
  앱이 `ImmSetCompositionWindow`로 캐럿을 알려주면 그 위치, **안 알려주면 화면 좌상단(0,0) 기본.**
- 예: Path of Exile 등 게임 채팅의 좌상단 preedit. (게임 내부 코드가 아닌 *관찰된 좌상단 동작*에서
  추론 — "조합 수락 + 미위치"의 전형적 징후라 거의 확실하나 단정은 아님.)
- UNIM: MS IME의 내장 좌상단 조합창에 대응하는 것이 우리의 `preedit_window` 오버레이다(같은 역할).
  위치는 `GetTextExt`로 캐럿 시도 → 게임에선 실패 → 현재 **마우스 근처** 폴백. MS IME는 **좌상단 고정.**
  → 위치 불가 시 폴백을 **좌상단(또는 화면 모서리) 고정**으로 두는 게 업계 관행에 가깝다(마우스
  추종은 거슬림). 현재 마우스 폴백은 개선 여지(미착수).

### ⑤ IME 완전 비활성 / 순수 raw 입력
- RawInput(WM_INPUT)·DirectInput만 쓰거나 `ImmDisableIME`/`TF_DisableThreadIme`로 IME를 끔.
  composition 자체가 성립 안 함 → **한글 입력 불가(영문/스캔코드만).**
- 예: 전체화면 exclusive 게임, 입력 직접 처리 앱.
- UNIM: 비대상. **주의: 같은 게임도 화면/모드별로 다르다** — 채팅창이 ④면 한글 가능, 진짜 raw면 불가.

## 한눈 요약

| 범주 | 뒷단 | composition | 대표 | UNIM 처리 |
|---|---|---|---|---|
| ① TSF-aware | 앱 ITextStoreACP | 네이티브 inline | 브라우저·리치컨트롤·Win11 메모장 | 정상 경로(공짜) |
| ② IMM32/CUAS | CUAS emulated store | 조건부 inline(GCS_COMPSTR) | EDIT·wezterm·Telegram | fInterimChar+단일세션+nav통과, 폴백 오버레이 |
| ③ 콘솔(conhost) | conhost 콘솔 IME | 콘솔 전용 | cmd/PowerShell 고전창 | 미검증(별도) |
| ④ 게임 IMM32(미렌더) | IME 기본 조합창 | IME가 그림(미위치=좌상단) | PoE 등 게임 채팅 | preedit_window 오버레이(좌상단 폴백 권장) |
| ⑤ raw/IME-off | 없음 | 없음 | 전체화면 게임 | 비대상(한글 불가) |

> 핵심 통찰: ②~④는 모두 "TSF-native 아님"이지만 **누가 preedit를 그리느냐**로 갈린다 — ② 앱이 직접
> inline(wezterm) / ③ conhost 콘솔 IME / ④ IME가 기본 조합창(게임). "게임=raw"는 부정확하다:
> 게임 채팅은 보통 ④(IME가 좌상단에 그림)이고, 진짜 raw(⑤)는 한글 자체가 안 된다.

## 유지보수 포인트
- "터미널"로 묶어 분기하지 말 것. 같은 터미널군이라도 wezterm(②) vs Windows Terminal(①) vs
  conhost(③)가 전부 다른 경로다.
- 모드를 미리 알 수 없으므로 UNIM은 **낙관적으로 composition 시도 → 종료되면 폴백** 구조다
  (`composition_unsupported` 비고착 재시도). 새 앱에서 깨지면 먼저 ①/②/③ 중 어디인지부터 가린다.
- 능력 탐지 단서: `ITfInsertAtSelection`/`GetTextExt` 동작 여부, OnCompositionTerminated 즉시 발생
  여부(②의 징후), 시스템 caret 유무. 결정적 단일 플래그는 없음 — 경험적 적응이 정석.
- 근거: MS Learn `TF_SELECTIONSTYLE`(fInterimChar — 한국어 조합 명시), CUAS 페이지(콘솔 제외),
  레퍼런스 구현 NavilIME/saenaru/kolemak. 상세 회고: `RETROSPECTIVE-tsf-terminal-inline.md`.

## 알려진 한계 — 네비게이션 키(Enter/화살표/Home/End 등) 확정↔통과 순서 (0.3.10)

증상(실측):
- **conhost**: "한글이" 입력 후 Home → **"이한글"**. (확정 "이"가 커서 이동 뒤 맨앞에 박힘)
- **wezterm 오버레이 모드**(드문 간헐 폴백): Home/End → 조합 글자 **확정 없이 키만 전달**.

확정된 원인(로그):
- UNIM의 nav-키 패스스루는 **OnTestKeyDown에서 조합 확정 후 pIsEaten=FALSE**(NavilIME 패턴)로 구현.
  wezterm(②)은 "OnTestKeyDown=FALSE면 키 통과"라 이 방식이 통한다.
- **conhost(③)는 OnTestKeyDown으로 게이팅하지 않고 OnKeyDown을 직접 호출** → UNIM의 패스스루
  로직이 conhost에선 아예 안 돈다. 엔진이 OnKeyDown에서 commit+passthrough를 내지만,
  conhost가 `fInterimChar` 인터림 문자를 **커서와 함께** 확정해 Home 이동 후 맨앞에 박힌다.
- 공통 핵심: **확정이 "이미 화면에 떠 있는 inline 조합을 제자리 확정"하는 경우(wezterm inline)는
  정상**이지만, **확정이 "텍스트를 새로 materialize"해야 하는 경우(conhost 인터림 / wezterm 오버레이)
  는 commit edit과 키 통과의 실행 순서가 호스트별로 역전**된다. TSF 규약상 OnTestKeyDown은 문서
  수정 금지라 호스트가 그 edit을 지연/재배치할 수 있는 것이 근본.

영향 범위: 주 사용 환경인 **wezterm inline은 정상**(Enter·화살표 포함). 한계는 ③ conhost(사용 빈도
낮음)와 ②의 드문 오버레이 폴백에 국한.

향후 수정 시 접근(미착수):
- 단일 코드로 깔끔히 안 됨 — **호스트별 반복 실측 필요**. 먼저 OnTestKeyDown/OnKeyDown 진입 여부와
  commit edit session의 HRESULT·순서를 로깅(진단 빌드)한 뒤 분기.
- 후보: (a) conhost 등 OnTestKeyDown 미게이팅 호스트 감지 시 OnKeyDown에서 **인터림 해제 후
  비-interim 확정 → 그다음 키**가 보장되도록; (b) 오버레이 nav는 확정을 키 통과보다 먼저 flush
  보장. 단 호스트 타이밍 의존이라 실측 없이는 또 헛다리 위험(회고 참조).
