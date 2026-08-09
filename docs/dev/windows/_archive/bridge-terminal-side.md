# 브릿지 조사 — terminal-side 각도

조사일: 2026-06-07. 각도: 터미널 측 IMM32 처리 상세와 inline preedit 경계.
도구: ctx_fetch_and_index + ctx_search (1차 소스: wezterm/winit/Windows Terminal 소스, MS Learn, GitHub 이슈/PR).
ctx_index source 라벨: `bridge-terminal-side`.

핵심 질문(적대적 재검증 대상):
1. wezterm은 CUAS-브릿지된 TSF TIP의 GCS_COMPSTR을 실제로 렌더하는가?
2. Windows Terminal(자체 TSF text store) vs wezterm vs conhost vs alacritty의 inline 지원 차이는?
3. 같은 IME가 어느 터미널에선 inline 되고 어디선 안 되는 경계는 무엇인가?

---

## 0. 한 줄 결론

**터미널의 inline preedit 경계는 "앱이 TSF text store(ITextStoreACP)를 구현했는가 + TSF 활성화 시 `TF_TMAE_UIELEMENTENABLEDONLY`를 끄는가"로 갈린다.**
wezterm/alacritty는 ITextStoreACP가 아예 없는 **순수 IMM32** 앱이고, CUAS가 TSF TIP을 IMM32 메시지(GCS_COMPSTR)로 역브릿지해줄 때만 inline이 된다.
MS 한국어 IME가 wezterm에서 inline 되는 것은 "CUAS가 그 TIP의 composition을 GCS_COMPSTR로 정상 브릿지"하기 때문이고, **우리 TIP도 동일 경로를 타도록 CUAS-호환 composition을 만들면 inline 가능**하다(별도 브릿지 API 불필요).

---

## 1. wezterm = 순수 IMM32, GCS_COMPSTR 직접 렌더 (1차 소스 확정)

소스: `wez/wezterm` `window/src/os/windows/window.rs` (source 라벨 `wezterm-windows-window-rs`).

- 임포트는 `winapi::um::imm::*` 뿐. `ImmGetCompositionStringW`, `ImmSetCompositionWindow`,
  `ImmSetCandidateWindow`만 선언. `ITextStoreACP`/`ITfThreadMgr`/`msctf` 심볼 **전무**.
- `WM_IME_COMPOSITION` 핸들러(`ime_composition`)의 실제 로직(소스 인용):
  - `lparam == 0` → IME 취소 처리.
  - `lparam & GCS_RESULTSTR == 0` (즉 확정 결과 없음) →
    `imc.get_str(GCS_COMPSTR)`로 미확정 문자열을 읽어
    `AdviseDeadKeyStatus(DeadKeyStatus::Composing(composing))` 디스패치 →
    **"We will show the composing string ourselves. Suppress the default composition display." → `return Some(1)`** (DefWindowProc 안 탐, 시스템 조합창 억제).
  - `GCS_RESULTSTR` 있으면 `KeyCode::Composed(s)`로 확정 입력.
- `WM_IME_SETCONTEXT`(`ime_set_context`): `lparam &= ~ISC_SHOWUICOMPOSITIONWINDOW` 후 DefWindowProc → 시스템 조합창 끄기. (단, `ime_preedit_rendering == System`이면 이 전부를 건너뛰고 시스템 렌더에 위임.)
- `WM_IME_STARTCOMPOSITION`: 디스패치 테이블에 **없음** → DefWindowProc 경유(능동 처리 안 함).
- 조합을 능동으로 끊지 않음: `ImmNotifyIME(CPS_CANCEL/COMPLETE)` 호출 코드 없음.

→ **wezterm은 표준 IMM32 클라이언트.** CUAS가 우리 TSF TIP의 composition을
`WM_IME_COMPOSITION`+GCS_COMPSTR로 변환해 보내주기만 하면 wezterm은 **그대로 inline 렌더한다.**
즉 wezterm 측에 추가로 필요한 일은 없다. 공은 전적으로 CUAS↔우리 TIP 셋업에 있다.

설정 분기: wezterm `config.ime_preedit_rendering` = `Builtin`(기본, 자체 렌더) / `System`(시스템 조합창). `Builtin`이 GCS_COMPSTR 자체 렌더 경로.

## 2. alacritty(winit) = wezterm과 동일한 순수 IMM32 경계 (1차 소스 확정)

소스: `rust-windowing/winit` `winit-win32/src/ime.rs` (source 라벨 `winit-win32-ime-rs`). alacritty의 Windows IME는 winit에 위임.

- `windows_sys::...::Input::Ime` (IMM32)만 사용. TSF 없음.
- `get_composing_text_and_cursor()` = `ImmGetCompositionStringW(GCS_COMPSTR)` + `GCS_COMPATTR`(ATTR_TARGET_CONVERTED/NOTCONVERTED로 타겟 절 경계 산출) + `GCS_CURSORPOS`.
- `get_composed_text()` = `GCS_RESULTSTR`.
- `ImmSetCompositionWindow`/`ImmSetCandidateWindow`로 위치만 지정, 렌더는 자체.
- `ImmAssociateContextEx(IACE_DEFAULT/IACE_CHILDREN)`로 IME on/off.

→ **alacritty의 inline 가능/불가 경계는 wezterm과 동일.** 둘 다 CUAS GCS_COMPSTR 브릿지에 의존. 우리 TIP이 wezterm에서 inline 되면 alacritty에서도 같은 원리로 inline 된다(반대도 성립).

## 3. Windows Terminal / conhost = 자체 TSF text store. 경계가 다름 (결정적 1차 소스)

소스: microsoft/terminal 이슈 #20040, #20038, PR #19738 (source 라벨 `winterm-korean-ime-20040`, `winterm-ime-20038`, `winterm-pr-19738-tsf-inline`), MS Learn ActivateEx(`mslearn-activateex-flags`).

### 결정적 사실 — `TF_TMAE_UIELEMENTENABLEDONLY` 플래그가 경계를 만든다

- MS Learn `ITfThreadMgrEx::ActivateEx(dwFlags)`:
  `TF_TMAE_UIELEMENTENABLEDONLY` = "TSF activates **only** text services that are
  categorized in `GUID_TFCAT_TIPCAP_UIELEMENTENABLED`." (1차 소스, 원문 인용)
  → 이 플래그를 켜면 앱의 TSF text store로는 **후보창 같은 UI 요소만** 동작하고,
  TIP은 **full TSF inline 편집에서 제외되어 IMM32 경로(플로팅 조합창)로 떨어진다.**
- Windows Terminal/conhost는 v1.23까지 이 플래그를 **켜둠** → 한국어 MS IME가
  **IMM32 경로로 동작(자체 플로팅 조합창)**, 터미널 inline 아님.
- PR **#19738 "Remove TF_TMAE_UIELEMENTENABLEDONLY"** (lhecker, 2026-01, v1.24부터 cherry-pick)에서 이 플래그를 제거.
  - 원래 목적: CorvusSKK(#19722)/Sogou(#19670) 후보/조합 UI가 안 보이는 회귀 수정.
  - **부수효과: 한국어 IME가 처음으로 TSF inline 렌더(터미널 자체 `tsfPreview` 오버레이)로 전환됨.** (#20040 root cause: "This regression was introduced by #19738, which caused the Korean IME to use TSF inline rendering for the first time (previously it used IMM32 with a floating composition window).")
- 후속 회귀들이 이 전환을 증명:
  - #20040: composing 글자가 우측 글자를 시각적으로 덮음. 원인 `Renderer::_PaintBufferOutput`가 `tsfPreview` 오버레이를 `ReplaceText`(overwrite)로 그림 → conhost가 **자체 버퍼에 inline preedit를 직접 렌더**함을 증명.
  - #20038/#20039: 조합 중 화살표키가 PTY로 먼저 전달되어 확정 글자가 잘못된 위치에 삽입. `TermControl::_KeyHandler`가 `HasActiveComposition()` 미확인. 수정은 conhost `windowio.cpp` 동작(조합 중 키를 PTY로 안 보냄)을 미러. → **Windows Terminal은 `ITfContextOwnerCompositionSink`(OnStartComposition/OnEndComposition), 비동기 TSF edit session(`TF_ES_ASYNC`), `HasActiveComposition()` 등 full TSF text-store 호스트 인프라를 구현**.

### 정리 — 터미널별 inline 경로

| 터미널 | TSF text store | inline 경로 | 비고 |
|--------|---------------|-------------|------|
| wezterm | 없음(순수 IMM32) | CUAS→GCS_COMPSTR 자체 렌더 | `ime_preedit_rendering=Builtin` |
| alacritty(winit) | 없음(순수 IMM32) | CUAS→GCS_COMPSTR 자체 렌더 | wezterm과 동일 |
| Windows Terminal | 있음(ITextStoreACP+OwnerCompositionSink) | full TSF inline(tsfPreview) | v1.24+(`UIELEMENTENABLEDONLY` 제거 후). 그 전엔 IMM32 플로팅창 |
| conhost(legacy console) | 있음(자체 TSF/IMM 호스트) | inline(자체 버퍼) | windowio.cpp 키 가드 |

→ **같은 MS 한국어 IME라도**: v1.23 Windows Terminal에서는 **플로팅 조합창**(inline 아님),
v1.24+에서는 **inline**. wezterm에서는 항상 **CUAS 브릿지 inline**.
**경계의 정체 = 앱의 TSF 인프라 구현 정도 + ActivateEx 플래그.** "터미널이라서 안 된다"가 아니다.

## 4. 우리(UNIM TSF TIP) 관점의 함의 — 적대적 재검증 결과

선행 잠정결론 "순수 TSF TIP의 레거시 inline 불가"는 **부분적으로 틀렸다**:

- wezterm/alacritty에서 inline을 받는 주체는 **앱이 아니라 CUAS**다. wezterm은 GCS_COMPSTR을 받으면 무조건 inline 렌더한다(소스 확정). 따라서 **"브릿지"는 이미 OS에 존재한다 = CUAS**. 사용자가 들은 "TSF-IMM32 브릿지"의 실체가 바로 CUAS(Cicero Unaware App Support)이며, 서드파티 TIP이 명시적으로 호출할 별도 API가 아니라 **OS가 자동 수행**한다.
- 우리 문제는 "브릿지 부재"가 아니라 **"우리 composition이 CUAS에 의해 GCS_RESULTSTR(확정)로 분류·즉시 종료"**되는 것. 즉 CUAS가 우리 composition을 "유지 중 미확정(GCS_COMPSTR)"으로 보게 만드는 셋업이 핵심.
- **반례로 자기검증**: MS 한국어 IME도 같은 CUAS를 통해 wezterm에 inline 된다(실측). 동일 OS 경로이므로 **우리 TIP이 MS IME와 같은 composition 셋업을 하면 같은 GCS_COMPSTR 브릿지를 받을 수 있다.** "MS만 IMM32 .ime 하이브리드 특권" 가설은 wezterm 사례에선 불필요(MS IME도 TSF TIP 경로로 CUAS 브릿지를 받음).
- **단, conhost/Windows Terminal 경로는 별개**다. 거기선 CUAS-GCS_COMPSTR이 아니라 앱 자체 ITextStoreACP로 inline이 들어간다. wezterm은 그 경로가 없으므로 **무조건 CUAS-GCS_COMPSTR 경로만** 유효.

### CUAS-GCS_COMPSTR 브릿지를 받기 위해 우리 TIP이 만족해야 할 조건(타 조사와 수렴)
1. composition을 **지속 유지**(매 자모 open/close churn 금지). `ITfComposition`을 edit session 밖에서 보존.
2. composition range에 **매 갱신 `GUID_PROP_ATTRIBUTE`(display attribute) SetValue** → CUAS가 미확정(밑줄)로 분류해야 GCS_COMPSTR로 나감. attribute 없으면 GCS_RESULTSTR로 오인.
3. **동기 트랜잭션(TF_ES_SYNC) 한 방에 open→fill→collapse를 결과확정처럼 끝내지 말 것.** keystroke 처리 외에는 `TF_ES_ASYNCDONTCARE` 권장(MS Learn RequestEditSession).
4. (주의) `ITfContextOwnerCompositionSink`는 **앱 측 sink**다. TIP은 `ITfCompositionSink`만 구현하면 됨 — 이건 우리가 이미 맞게 함. (선행 synthesis의 P1 "TIP이 OwnerCompositionSink 미구현" 지목은 **인터페이스 소유 주체를 혼동**한 것. Windows Terminal이 OwnerCompositionSink를 구현하는 건 그게 "앱"이기 때문. wezterm은 그것도 없고 CUAS가 대행함.)

## 5. 한계/추가 검증 필요

- CUAS가 정확히 어떤 조건에서 우리 composition을 GCS_RESULTSTR로 스냅샷·종료하는지는 msctf 내부 구현이라 1차 문서가 없음(블랙박스). 위 1~3은 SampleIME(정상 레퍼런스)와 MS Learn 권고에서 역추론한 것.
- wezterm/alacritty 모두 `WM_IME_STARTCOMPOSITION`을 능동 처리 안 하므로, CUAS가 STARTCOMPOSITION→COMPOSITION(GCS_COMPSTR)→유지 시퀀스를 보내주기만 하면 됨. 실측에서 끊기는 지점(OnCompositionTerminated by_time<200ms)이 CUAS의 GCS_RESULTSTR 조기 스냅샷과 일치하는지 로그 타임스탬프로 교차검증 권장.
- conhost 경로(진짜 콘솔)에서 우리 TIP 동작은 본 사례(wezterm 커스텀 HWND)와 무관 — 별도 조사 대상.

---

## 출처

- wezterm `window/src/os/windows/window.rs` (raw, main) — `ime_composition`/`ime_set_context`/`ime_end_composition`, GCS_COMPSTR/GCS_RESULTSTR/ISC_SHOWUICOMPOSITIONWINDOW. source 라벨 `wezterm-windows-window-rs`.
- winit `winit-win32/src/ime.rs` (raw, master) — IMM32 GCS_COMPSTR/COMPATTR/RESULTSTR. source 라벨 `winit-win32-ime-rs`.
- MS Learn `ITfThreadMgrEx::ActivateEx` (msctf.h) — TF_TMAE_UIELEMENTENABLEDONLY 정의. source 라벨 `mslearn-activateex-flags`.
  https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgrex-activateex
- microsoft/terminal PR #19738 "Remove TF_TMAE_UIELEMENTENABLEDONLY" (lhecker) — 한국어 IME가 v1.24부터 TSF inline로 전환된 결정적 변경. source 라벨 `winterm-pr-19738-tsf-inline`. https://github.com/microsoft/terminal/pull/19738
- microsoft/terminal #20040 (drvoss) — root cause에 "previously it used IMM32 with a floating composition window" 명시. source 라벨 `winterm-korean-ime-20040`. https://github.com/microsoft/terminal/issues/20040
- microsoft/terminal #20038/#20039 — TSF 활성 후 HasActiveComposition 키 가드, conhost windowio.cpp 미러. source 라벨 `winterm-ime-20038`. https://github.com/microsoft/terminal/issues/20038
- (교차) 선행 조사 research-cuas-bridge-terminate.md / research-wezterm-ime.md.
