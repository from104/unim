# Bridge research: direct-imm-from-tip

조사 각도: **TSF TIP에서 IMM32를 직접 구동**하여, 포커스 창의 IMC에
미확정 문자열(GCS_COMPSTR)을 주입하고 `WM_IME_COMPOSITION`을 발생시켜
wezterm 같은 순수 IMM32 터미널에 inline preedit을 그릴 수 있는가.

선행 잠정결론("순수 TSF TIP의 레거시 inline 불가")을 적대적으로 재검증.

---

## 핵심 결론 (요약)

1. **CUAS 환경에서 IMC 소유자는 "포커스 창의 스레드"이고, TIP은 그 스레드 안에서
   in-proc로 실행된다.** TIP DLL은 CLSCTX_INPROC_SERVER COM 서버로 포커스 앱의
   스레드에 로드된다(`CoCreateInstance(CLSID_TF_ThreadMgr, ..., CLSCTX_INPROC_SERVER)`,
   TipTextServiceFactory `OnDllProcessAttach`). 따라서 TIP이 `GetFocus()` →
   `ImmGetContext(hwnd)`로 얻는 HIMC는 **자기 스레드(=앱 스레드)의 IMC**다. 이는
   cross-thread/cross-process `ImmSetCompositionString` 제약을 우회한다.
   (출처: r32-win32-ime CoCreateInstance INPROC; Mozc tip_text_service OnDllProcessAttach;
   MS Learn ImmGetContext "default input context the system created and associated
   with all the windows of a particular thread")

2. **`ImmSetCompositionStringW(hIMC, SCS_SETSTR, comp, len, NULL, 0)`는 IMC의
   메시지 버퍼(`INPUTCONTEXT::hMsgBuf`, `dwNumMsgBuf`)에 `WM_IME_COMPOSITION`
   TRANSMSG를 적재한다.** 그리고 `ImmGenerateMessage(hIMC)` 또는
   `CtfImmDispatchDefImeMessage(hWnd, ...)`가 그 버퍼를 비우며 실제로
   `SendMessageW(hWnd, WM_IME_COMPOSITION, wParam, lParam)`로 포커스 창에
   전달한다. = 우리가 IMM32 IME가 보내는 것과 동일한 메시지를 합성해서 보낼 수 있다.
   (출처: ReactOS win32ss/user/imm32/ctf.c `CtfImmDispatchDefImeMessage`; Wine
   dlls/imm32/imm.c `ImmGenerateMessage`)

3. **`CtfImmDispatchDefImeMessage`는 실재하는 msctf↔imm32 브릿지 export다**
   (사용자 제보의 "TSF-IMM32 브릿지"의 정체). msctf.dll ordinal 8 export. imm32.dll
   에도 동명 export(`IMM32.@`). ReactOS가 그 시그니처와 구현을 재현:
   `LRESULT WINAPI CtfImmDispatchDefImeMessage(HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam)`.
   구현 핵심: `ImmLockIMC` → `pIC->hMsgBuf`의 TRANSMSG 배열을 순회하며
   `SendMessageW/PostMessageW(pIC->hWnd, uMsg, wParam, lParam)`, 끝에 `dwNumMsgBuf=0`.
   (출처: ReactOS ctf.c; STRONTIC msctf.dll export 목록 ordinal 8;
   Wine imm32.spec `CtfImmDispatchDefImeMessage`(stub) / `CtfImmGenerateMessage`(stub))

4. **MS 한국어 IME는 IMM32 .ime가 아니라 TSF TIP이다.** 그런데도 wezterm에
   GCS_COMPSTR이 도달한다 → 따라서 "TIP→IMM32 inline"은 **MS만의 특권이 아니라
   CUAS의 일반 경로**다. (Mozilla bug 1208043: "MS-IME for Korean ... calls
   InsertTextAtSelection() ... OnStartComposition()" = TSF 인터페이스 사용 = TIP)

5. **그러면 왜 UNIM의 빈/실제 composition은 CUAS에 즉시 terminate 되는가?**
   가장 유력한 원인: TIP이 composition range에 **display attribute
   (`GUID_PROP_ATTRIBUTE`)를 SetValue 하지 않으면**, CUAS는 그 range를 "미확정"이
   아니라 "확정 결과 텍스트"로 오인하여 GCS_RESULTSTR로 처리하고 composition을
   종료시킨다. Mozc는 항상 composition range에 display attribute property를 채운다.
   (출처: 기존 색인 cuas-bridge 분석 + Mozc tip_composition_util.cc
   `GetProperty(GUID_PROP_ATTRIBUTE, ...)`; MS Learn "Providing Display Attributes")
   → 이는 **direct-imm 경로를 쓰기 전에 먼저 시도할 가치가 있는 저비용 수정**.

---

## 정확한 API 시퀀스 (direct-imm-from-tip, 가설 A)

TIP의 키 처리 핸들러(in-proc, 앱 스레드) 안에서:

```c
HWND   hWnd = GetFocus();              // = 앱 스레드의 포커스 창 (wezterm)
HIMC   hIMC = ImmGetContext(hWnd);     // 앱 스레드 IMC (cross-thread 아님)
if (hIMC) {
    // 1) 미확정 문자열 적재 → IMC hMsgBuf에 WM_IME_COMPOSITION TRANSMSG 생성
    ImmSetCompositionStringW(hIMC, SCS_SETSTR,
                             (LPVOID)pComp, cbComp,   // 조합 중 "ㅎ"
                             NULL, 0);
    // 2) 버퍼 flush → 실제 SendMessage(hWnd, WM_IME_COMPOSITION, GCS_COMPSTR)
    //    (ImmSetCompositionString이 내부에서 generate 안 하는 경우)
    ImmGenerateMessage(hIMC);
    // 또는 CtfImmDispatchDefImeMessage(hWnd, WM_IME_COMPOSITION, 0, GCS_COMPSTR);
    ImmReleaseContext(hWnd, hIMC);
}
```

확정 시: `ImmSetCompositionStringW(hIMC, SCS_SETSTR, result, ..., NULL,0)` 후
`ImmNotifyIME(hIMC, NI_COMPOSITIONSTR, CPS_COMPLETE, 0)` 또는 GCS_RESULTSTR로
flush → `WM_IME_COMPOSITION(GCS_RESULTSTR)` + `WM_IME_ENDCOMPOSITION`.

wezterm은 `WM_IME_STARTCOMPOSITION` 미처리이므로, GCS_COMPSTR을 담은
`WM_IME_COMPOSITION` 자체가 도달하면 자체 렌더러가 inline 그림(실측: MS 한국어
IME에서 이미 동작). STARTCOMPOSITION은 굳이 보낼 필요 없음.

---

## 위험 / 미확인 (적대적 평가)

- **`hIMC` 유효성**: wezterm은 `ImmAssociateContextEx`로 IMC를 비활성화/교체할 수
  있음. 그 경우 `ImmGetContext`가 NULL이거나 빈 IMC. 단 MS 한국어 IME가
  동작한다는 사실은 wezterm이 IMC를 살려둔다는 강한 방증.
- **CI_TSFDISABLED / Cicero 모드 충돌**: TIP이 활성인 상태에서 같은 IMC에
  ImmSetCompositionString을 직접 쓰면, CUAS의 default text store 상태와 IMM
  버퍼가 **이중 기록**되어 충돌·재진입 가능. MS 권고: "Avoid calling Imm*
  functions while the IME is processing another window message"(reentrancy).
  → TIP의 TSF composition을 **아예 시작하지 않고**(StartComposition 호출 안 함)
  IMM 경로만 단독 사용하는 것이 더 안전할 수 있음(=TIP을 키-필터로만 쓰고
  출력은 IMM으로). 이 조합이 실제로 허용되는지는 실측 필요.
- **`ImmSetCompositionString`이 곧장 SendMessage 하는지, 버퍼만 채우는지**는
  Windows 실제 imm32 구현에 따라 다름(문서 미보장). ReactOS/Wine는 generate 단계
  분리. 실측으로 `ImmGenerateMessage`/`CtfImmDispatchDefImeMessage` 필요 여부 확인.
- **서드파티 직접 write-side 선례 부재**: Mozc는 IMM32 **read-side**
  (`GetSurroundingTextImm32`, IMR_DOCUMENTFEED)만 TIP에서 쓴다. write-side
  (ImmSetCompositionString from TIP) 공개 선례는 이번 조사에서 확인 못함 → 미개척.
- **보안**: in-proc 동일 스레드이므로 UIPI/cross-process 메시지 필터 비해당.

---

## 1차 권고

1. (저비용 선행) display attribute(GUID_PROP_ATTRIBUTE) 누락 가설부터 검증 —
   direct-imm 전에 정공법 TSF composition이 살아남는지 재확인.
2. (본 각도) display attribute로도 안 되면 direct-imm-from-tip 시퀀스를
   `ImmSetCompositionStringW + ImmGenerateMessage`로 PoC. STARTCOMPOSITION 생략,
   GCS_COMPSTR만 주입해 wezterm 자체 렌더 확인.
3. TSF StartComposition과 IMM 직접 주입의 동시 사용 충돌 여부를 실측 후 택일.

## 1차 소스
- ReactOS win32ss/user/imm32/ctf.c (CtfImmDispatchDefImeMessage 구현/시그니처)
- ReactOS win32ss/user/imm32/imm.c, win32ss/user/ntuser/ime.c
- Wine dlls/imm32/imm.c (ImmGenerateMessage), dlls/imm32/imm32.spec (CtfImm* exports)
- MS Learn: ImmSetCompositionStringW, ImmGetContext, WM_IME_COMPOSITION, TSF Compositions/Display Attributes
- Mozc win32/tip/*.cc (in-proc TIP, IMM32 read-side fallback, GUID_PROP_ATTRIBUTE)
- Mozilla bug 1208043 (MS 한국어 IME = TSF TIP 증거)
- STRONTIC msctf.dll export (CtfImmDispatchDefImeMessage ordinal 8)
- MS Learn troubleshoot: IME crash cross-thread sent message (reentrancy 경고)
