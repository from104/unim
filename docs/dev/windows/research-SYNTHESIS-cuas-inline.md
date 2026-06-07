# 종합 결론 — wezterm inline preedit은 가능하다, 종료 주체는 CUAS다

3개 병렬 조사(wezterm IME 소스 / CUAS 브리지 / UNIM 코드 감사) 수렴 결과.
세부: [research-wezterm-ime.md], [research-cuas-bridge-terminate.md], [research-unim-composition-audit.md].

## 사실 확정 (가설 → 사실로 교정)
1. **wezterm = 순수 IMM32.** ITextStoreACP(TSF text store) 미구현. `ImmGetCompositionStringW(GCS_COMPSTR)`로 미확정 문자열을 직접 읽어 **자체 렌더러·터미널 폰트로 inline 그림**. 시스템 조합창은 억제.
2. **wezterm은 조합을 능동적으로 끊지 않는다.** `WM_IME_STARTCOMPOSITION` 처리 없음, `ImmNotifyIME(CPS_CANCEL/COMPLETE)` 호출 없음.
3. **즉시 종료(OnCompositionTerminated)의 주체 = CUAS 브리지.** CUAS가 우리 TSF composition을 "유지 중(GCS_COMPSTR)"이 아니라 "확정 결과(GCS_RESULTSTR)"로 스냅샷·종료.
4. **MS 기본 IME가 같은 wezterm에서 inline 되는 이유 = CUAS가 그 TSF TIP의 composition을 제대로 GCS_COMPSTR로 브리지하기 때문.**
   → **결론: inline preedit은 우리도 가능하다.** "터미널은 조합 불가"는 틀렸다. 우리 composition 셋업이 CUAS-호환이 아닐 뿐.

## 근본 원인 (수렴 순위)
- **P1 — `ITfContextOwnerCompositionSink` 미구현/미등록** (text_service.rs `#[implement]`에 `ITfCompositionSink`만 있음). CUAS 브리지가 composition 시작을 승인·유지할 owner sink가 없어 StartComposition 직후 되돌릴 가능성. MS IME와의 결정적 코드 차이. (analyst 1순위)
- **P2 — StartComposition 순서 / 세션 패턴.** 우리는 텍스트를 SetText 한 range를 감싸 composition 시작 + start/update를 단일 `TF_ES_READWRITE|TF_ES_SYNC` 세션에서 마무리 → CUAS가 "이미 확정 문자열"로 오인·스냅샷. MS SampleIME는 **빈/collapsed range에 먼저 StartComposition**(`InsertTextAtSelection(TF_IAS_QUERYONLY)`)하고, **StartComposition을 별도 세션으로 분리**, composition 객체는 세션보다 오래 유지. (CUAS·analyst 공통)
- **P3 — 매 update마다 전체 range re-select / 매 자모 start-end churn.** "선택→조합종료"로 해석될 소지. EndComposition은 확정/취소 때만.
- **반증 — display attribute 부재는 원인 아님.** provider 구현 + 카테고리 등록 + 매 세션 SetValue 배선 완료. 단 CUAS가 미확정 인식하려면 SetValue가 **실제 성공**해야 함 → 로그(SetValue FAILED/GetValue MISMATCH 0건)로 1차 확인됨(P1 진단). 즉 attribute는 무죄에 가깝다.

## 권고 (오버레이 → 정공법 전환)
**CUAS-호환 composition으로 재작성하면 wezterm/Telegram에서 그 앱 폰트로 진짜 inline preedit이 난다(=MS IME와 동급).** client-side 오버레이(preedit_window.rs)는 그때 **최후 폴백**으로 강등(또는 불필요).

수정 순서(저위험→고위험, 각 단계 후 wezterm 실측):
1. **수정 A — `ITfContextOwnerCompositionSink` 구현 + 등록.** OnStartComposition에서 `pComposition`/range 수락(S_OK), OnUpdate/OnEndComposition 처리. (단독 우선 적용)
2. **수정 B — StartComposition을 빈 range에 먼저, 별도 세션으로.** SampleIME 패턴. 텍스트는 composition 시작 후 SetText.
3. **수정 C — composition 객체 수명 분리.** 매 자모 start/end churn 제거, update만 SetText, End는 확정/취소 시.
4. **수정 D(최후) — update를 ASYNCDONTCARE로.** composition_slot 동기 take 재설계 동반 → 최후순위.

검증: 각 수정 후 `%TEMP%\unim-tsf.log`에서 `OnCompositionTerminated: IMMEDIATE`가 사라지고 wezterm에 inline 조합이 뜨는지. 이상적으로 동일 wezterm에서 MS IME가 CUAS 경유로 보내는 IMM32 메시지 시퀀스(Spy++ WM_IME_*)를 캡처해 UNIM과 diff.
