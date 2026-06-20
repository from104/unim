# wezterm Windows IME 처리 메커니즘 1차 소스 조사

조사 대상: `wez/wezterm` `main` 브랜치, `window/src/os/windows/window.rs` (1차 소스, 약 97KB).
조사 도구: ctx_fetch_and_index + ctx_search (source 라벨 `wezterm-windows-window-rs`, `research-wezterm-ime`).

---

## 1. API 레이어 확정 — IMM32 (TSF 아님)

wezterm Windows 백엔드는 **순수 IMM32** 기반이다. TSF(ITextStoreACP) 구현은 **전혀 없다**.

근거 (window.rs 임포트):
```rust
use winapi::um::imm::*;            // IMM32 전체
extern "system" {
    pub fn ImmGetCompositionStringW(...) -> LONG;
    pub fn ImmSetCandidateWindow(...) -> BOOL;
}
```
- 사용 함수: `ImmGetContext` / `ImmReleaseContext`(ImmContext Drop), `ImmGetCompositionStringW`, `ImmSetCompositionWindow`, `ImmSetCandidateWindow`.
- `ITextStoreACP`, `ITfThreadMgr`, `ITfDocumentMgr`, `msctf` 류 심볼은 소스 전체에 **존재하지 않음**.

결론: wezterm 창은 TSF에게는 **non-aware(레거시 IMM32) 윈도우**로 보인다.

---

## 2. 처리하는 IME 메시지

wezterm WndProc(`wnd_proc` 디스패치)가 다루는 IME 메시지:

| 메시지 | 핸들러 | 동작 |
|--------|--------|------|
| `WM_IME_SETCONTEXT` | `ime_set_context` | `lparam &= ~ISC_SHOWUICOMPOSITIONWINDOW` 후 DefWindowProc → 시스템 조합창 그리기 억제 |
| `WM_IME_COMPOSITION` | `ime_composition` | preedit/result 문자열 추출 |
| `WM_IME_ENDCOMPOSITION` | `ime_end_composition` | DeadKeyStatus::None 디스패치 (조합 종료) |
| `WM_DEADCHAR/WM_KEYDOWN/WM_CHAR/WM_IME_CHAR/...` | `key` | 키 처리, IME 활성 시 TranslateMessage로 위임 |

**중요한 부재:**
- `WM_IME_STARTCOMPOSITION`은 **명시적으로 처리하지 않는다** (디스패치 테이블에 없음 → DefWindowProc 경유).
- `WM_IME_NOTIFY`도 별도 핸들러 없음.

즉 wezterm은 조합 **시작 시점에 아무 거부/제어를 하지 않으며**, 조합 진행/종료만 수동(passive) 처리한다.

---

## 3. preedit를 자체 렌더러로 그리는 방식

`ime_composition` 핵심 로직:

```rust
if inner.config.ime_preedit_rendering == ImePreeditRendering::System {
    return None;        // 시스템이 그리게 둠
}
let imc = ImmContext::get(hwnd);
let lparam = lparam as DWORD;

if lparam == 0 { /* 조합 취소 */ dispatch(AdviseDeadKeyStatus(None)); return Some(1); }

if lparam & GCS_RESULTSTR == 0 {
    // 미확정(조합 중) 문자열: GCS_COMPSTR
    if let Ok(composing) = imc.get_str(GCS_COMPSTR) {
        dispatch(AdviseDeadKeyStatus(DeadKeyStatus::Composing(composing)));
    }
    return Some(1);     // 기본 조합 표시 억제 — wezterm이 직접 그림
}

match imc.get_str(GCS_RESULTSTR) {   // 확정 문자열
    Ok(s) if !s.is_empty() => {
        dispatch(KeyEvent { key: KeyCode::Composed(s), ... });
        dispatch(AdviseDeadKeyStatus(DeadKeyStatus::None));
        return Some(1);
    }
}
```

핵심 포인트:
- **`ImmGetCompositionStringW(GCS_COMPSTR)`** 로 미확정 문자열을 직접 읽어 `WindowEvent::AdviseDeadKeyStatus(Composing(str))` 로 상위(터미널 렌더러)에 올린다 → 터미널이 **자체 폰트/렌더러로 inline preedit을 그린다**. 그래서 wezterm 폰트로 보인다.
- 조합 메시지에 대해 **`Some(1)`** 을 반환해 DefWindowProc의 기본 조합창 표시를 차단한다.
- 확정(GCS_RESULTSTR)은 `KeyCode::Composed`로 변환해 일반 텍스트 입력으로 흘려보낸다.

조합창 위치(`ImePreeditRendering::Builtin` 기본값일 때):
```rust
fn set_ime_window_position(&mut self, cursor: Rect) {
    match self.config.ime_preedit_rendering {
        Builtin => imc.set_candidate_window_position(cursor),  // 후보창만 위치잡음, CFS_EXCLUDE
        System  => imc.set_composition_window_position(cursor),// CFS_POINT
    }
}
```
- Builtin 모드: `ImmSetCandidateWindow`(CFS_EXCLUDE)로 **후보창**만 cursor cell에 맞춤. 조합 텍스트 자체는 wezterm이 그림.
- System 모드: `ImmSetCompositionWindow`(CFS_POINT)로 시스템에 조합창 위치만 알리고 시스템이 그림.

---

## 4. composition 취소/완료 코드 단서

- wezterm에는 **`ImmNotifyIME(CPS_CANCEL/CPS_COMPLETE)` 호출이 없다.** 즉 wezterm이 능동적으로 조합을 끊는 코드는 없음.
- 조합 종료/취소 인식은 전부 수동: `WM_IME_ENDCOMPOSITION`(`ime_end_composition`) 또는 `WM_IME_COMPOSITION` with `lparam==0` 에서 `DeadKeyStatus::None`만 디스패치.
- 키 경로(`key`)에서 **IME 활성 시 wezterm은 절대 키를 가로채지 않고** Windows에 넘긴다:
```rust
if ime_active {
    if msg == WM_KEYDOWN { translate_message(hwnd,...); return Some(0); }
    return None;   // IME가 알아서 굴러가게 둠
}
```
→ wezterm 측이 UNIM 조합을 강제 종료시키는 근거는 **없다**. 즉 UNIM의 즉시 OnCompositionTerminated는 wezterm WndProc가 아니라 **TSF/CUAS 브리지 계층**에서 유발될 가능성이 높다 (6번 참조).

---

## 5. 설정 옵션 / 관련 버그

- 설정: `ime_preedit_rendering` (enum `ImePreeditRendering { Builtin, System }`, config 크레이트). 일반 `use_ime` 토글은 Windows 백엔드에선 IMM 비활성보다 렌더링 모드 선택이 핵심.
- 알려진 클래스의 이슈: CJK(한/중/일) preedit 폰트가 시스템 IME 박스로 뜨던 문제 → `Builtin` 렌더링(자체 그리기)으로 해결한 흐름. (조사 시 이슈 번호는 본문 캐시에서 직접 확인 권장; 소스 주석이 "application itself draws it" 로 의도를 명시.)

---

## 6. MS IME(TSF TIP)가 wezterm IMM32 핸들러까지 도달하는 경로 (가설)

wezterm은 ITextStoreACP를 구현하지 않으므로 TSF aware 윈도우가 아니다. 그럼에도 MS 한국어 IME(TSF TIP)가 동작하는 이유:

- Windows의 **CUAS (Cicero Unaware Application Support)** 가 TSF non-aware(IMM32) 윈도우를 위해 **TSF TIP ↔ IMM32 브리지**를 제공한다.
- MS IME TIP은 CUAS를 통해 IMM32 메시지(`WM_IME_STARTCOMPOSITION/COMPOSITION/ENDCOMPOSITION`, `ImmGetCompositionStringW` 데이터)로 변환되어 wezterm WndProc에 전달된다.
- 따라서 wezterm에서 MS IME inline preedit이 되는 것 = **CUAS 브리지가 정상 동작**한다는 증거.

### UNIM 즉시-terminate 문제 가설
UNIM은 TSF TIP이다. wezterm처럼 TSF non-aware인 창에서:
1. UNIM TIP이 CUAS 브리지를 거치는데, MS IME와 달리 UNIM이 **ITfContextOwnerCompositionSink/edit session 또는 SetText/GetSelection 흐름에서 wezterm(레거시 IMM32 창)이 제공하는 빈약한 text store 응답**에 의존하면, CUAS가 조합을 즉시 무효화 → OnCompositionTerminated.
2. MS IME는 CUAS-unaware 창에 대해 GetSelection/QueryInsert 실패를 관용적으로 처리하지만, UNIM이 이를 엄격하게 다뤄 조합을 스스로 끝낼 수 있음.
3. 즉, 문제 원인은 wezterm이 아니라 **UNIM TIP의 CUAS 브리지 환경(레거시 IMM32 창)에서의 composition/edit session 처리**일 가능성이 가장 높다. (이미 메모리에 기록된 "CUAS 즉시-terminate HWND 학습 캐시" 작업과 정합.)

### UNIM이 검토할 실험
- UNIM이 IMM32-스타일로 동작하도록(또는 CUAS 경로에서 GetSelection/QueryInsertEmbedded 실패를 MS IME와 동일하게 관대 처리) 수정.
- wezterm 같은 non-aware 창 감지 시 ITextStoreACP가 없을 때의 GetSelection/GetText 기본응답 보강.
- 비교 대상: MS IME가 동일 wezterm에서 CUAS 경유로 보내는 IMM32 메시지 시퀀스를 Spy++/IME 로깅으로 캡처해 UNIM 시퀀스와 diff.

---

## 핵심 코드 경로 요약
- 파일: `window/src/os/windows/window.rs` (단일 파일에 모든 IME 로직)
- 디스패치: `wnd_proc` → `WM_IME_SETCONTEXT/COMPOSITION/ENDCOMPOSITION`, `key`
- 자료구조: `struct ImmContext { hwnd, imc }` (RAII, ImmGetContext/ImmReleaseContext)
- preedit 추출: `ImmContext::get_str(GCS_COMPSTR)` → `WindowEvent::AdviseDeadKeyStatus(Composing)`
- WM_IME_STARTCOMPOSITION: **미처리** / ImmNotifyIME: **미사용**
