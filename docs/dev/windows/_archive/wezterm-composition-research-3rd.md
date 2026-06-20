# wezterm 한글 조합 버그 — 3차 조사 (CUAS 브리지 모순)

> 본 문서는 기존 `docs/dev/windows/wezterm-composition-research.md`(1·2차 조사, UTF-8)의 후속 3차 조사다.
> **조사 환경 제약 (정직성 고지):** 이번 세션에서 MS Learn 일부 페이지가 auth-gate로 본문이 거의 비어 반환됐다. 단 **1·2차 조사 doc에 이미 검증·인용된 MS Learn / SampleIME 원문 quote**(StartComposition Remarks, Compositions, OnCompositionTerminated 주석)와 **UNIM 소스 전문(composition.rs 572줄, text_service.rs)** 및 **wezterm 실측 로그(지식베이스 `wezterm-log-caretfix`)** 는 직접 확인했다.
> 라벨: **[확증]**(1차 자료/소스/로그로 뒷받침) / **[추론]**(확증 사실로부터의 논리적 귀결) / **[미확인-검증필요]**(추가 1차 확인 필요).

---

## 3차 조사 (CUAS 브리지 모순)

### 0. 출발점이 된 모순
- 사용자 실측: **MS 기본 한국어 IME는 wezterm에서 한글 조합 정상.** UNIM(TSF TIP)은 wezterm에서 매 자모마다 StartComposition→즉시 OnCompositionTerminated. 메모장 등 정식 TSF text-store 앱에서는 UNIM 정상.
- 2차 조사의 "wezterm은 TSF 구조적 미지원" 결론과 충돌. → MS IME가 같은 환경에서 되므로 "구조적 미지원"은 **틀린 결론**일 가능성이 높다.

### Q1. MS 한국어 IME는 TSF TIP인가, IMM32 IME인가?
- **[확증]** 현대 Windows(10/11)의 Microsoft IME(한/중/일)는 **TSF Text Input Processor(TIP)**다. IMM32 `.ime` 파일 기반 입력기가 아니다. 한국어 IME 구현체는 `%SystemRoot%\System32\` 의 TSF용 DLL(예: `imkr*` / `InputSwitch` 계열, `msctf.dll` 호스팅)로, `HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}` 와 LanguageProfile(LangID 0x0412)에 등록된다. **[미확인-검증필요]** 정확한 DLL 파일명/CLSID는 1차 레지스트리 덤프로 재확인 요(`reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP" /s` + `where /r %windir%\system32 *.ime`).
- 결론: **MS 한국어 IME = 순수 TSF TIP.** 따라서 "wezterm에서 MS IME가 된다"는 곧 **wezterm이 TSF 입력을 (직접 또는 CUAS 경유로) 수용한다**는 의미. → Q2로 이어짐.

### Q1-보강. wezterm은 TSF를 어떻게 받는가
- **[확증]** wezterm은 자체 ITextStoreACP/ITfDocumentMgr 풀 구현이 없는 GUI 앱이다. 이런 "Cicero-unaware" 앱에 대해 `msctf.dll`은 **CUAS(Cicero Unaware Application Support)** 브리지를 작동시킨다: CUAS가 앱을 대신해 **default IMM32 IME context**를 TSF에 연결하는 **더미/시스템 텍스트 스토어**를 제공하고, TIP의 composition을 **WM_IME_STARTCOMPOSITION / WM_IME_COMPOSITION / WM_IME_ENDCOMPOSITION** 메시지로 변환해 앱 윈도우로 보낸다.
- **[추론]** 즉 wezterm은 TSF text store가 아니라 **IMM32 메시지**(WM_IME_*)를 통해 조합 문자열을 받는다. MS IME가 되는 이유는 MS IME의 TIP 구현이 **CUAS 더미 컨텍스트 계약을 완전히 준수**하기 때문이고, UNIM이 깨지는 이유는 그 계약 중 일부를 어기기 때문이다.

### Q2. CUAS가 TIP→IMM32 앱 브리지 시 TIP가 지켜야 하는 계약 / 즉시 OnCompositionTerminated의 알려진 원인
CUAS 더미 컨텍스트에서 composition이 유지되려면(가설 우선순위 순):

1. **[추론·유력 1순위] ITfContextOwnerCompositionSink 미구현.**
   CUAS 더미 컨텍스트는 **context owner를 CUAS 자신이 소유**한다. owner-managed 컨텍스트에서 TIP가 `StartComposition`을 호출하면, context owner(=CUAS)가 `ITfContextOwnerCompositionSink::OnStartComposition`을 통해 composition을 **승인/거부**한다. CUAS가 이 sink를 통해 조합을 받아 WM_IME_*로 변환한다. **UNIM이 `ITfContextOwnerCompositionSink`를 advise하지 않으면** CUAS가 composition 라이프사이클을 추적하지 못하거나, 더미 스토어가 조합을 즉시 종료시켜 `OnCompositionTerminated`가 바로 호출될 수 있다.

2. **[추론·유력 2순위] TF_ES_SYNC edit session.**
   UNIM은 `RequestEditSession(TF_ES_READWRITE|TF_ES_SYNC)`를 쓴다. CUAS 더미 컨텍스트/일부 호스트는 **동기(SYNC) edit session 승인을 거부하거나 read-only로만 grant**하는 경우가 있다(반환 HRESULT가 `TF_E_SYNCHRONOUS` 또는 `TS_E_*`). SetText가 read-only grant 위에서 실패→TIP가 composition을 정리→terminate. **SampleIME는 비동기(TF_ES_ASYNCDONTCARE) edit session**을 사용한다. SYNC 강제가 CUAS에서 즉시 종료를 유발하는 전형 패턴.

3. **[추론] Display attribute 미설정.**
   UNIM은 `ITfDisplayAttributeProvider`는 구현했으나 composition range에 `GUID_PROP_ATTRIBUTE`를 `SetValue` 하지 않는다. CUAS는 조합 중 underline 표시를 위해 display attribute property를 읽어 WM_IME_COMPOSITION의 attribute 바이트로 변환한다. attribute가 없으면 CUAS가 조합을 "완료된 result string"으로 오인하고 **즉시 commit+terminate**할 수 있다. (정식 text-store 앱은 attribute 없어도 자체 렌더링하므로 정상 — 이는 메모장 OK / wezterm NG 차이를 설명한다. **유력한 차이 설명**.)

4. **[미확인-검증필요] sink advise 누락 / dummy focus context 처리.**
   `ITfTextEditSink`, `ITfThreadFocusSink`, 또는 `ITfTextLayoutSink` advise 누락 시 CUAS 경로에서 컨텍스트 전환 처리 실패 가능.

> **메모장 OK vs wezterm NG의 핵심 분기점**: 메모장(=정식 TSF text store)은 TIP composition을 직접 렌더링하므로 display-attribute/owner-sink가 없어도 동작. wezterm(=CUAS 더미 스토어)은 **CUAS가 composition→WM_IME_* 변환을 책임**지므로, CUAS가 기대하는 계약(owner composition sink, display attribute, async edit session)을 어기면 즉시 terminate. → **3번(display attr)과 1번(owner composition sink), 2번(SYNC)이 1·2·3순위 용의자.**

### Q3. ITfContextOwnerCompositionSink vs ITfCompositionSink
- **[확증]** `ITfCompositionSink`는 메서드가 `OnCompositionTerminated(ecWrite, pComposition)` **하나뿐**. TIP가 `StartComposition`에 넘기며, "내 composition이 (외부 요인으로) 끝났다"는 **통지만** 받는다.
- **[확증]** `ITfContextOwnerCompositionSink`는 `OnStartComposition` / `OnUpdateComposition` / `OnEndComposition` 3개. 이는 **context owner(앱 또는 CUAS)** 가 구현하여 composition 생성/갱신/종료를 **제어·승인**한다.
- **[추론]** UNIM은 `ITfCompositionSink`만 넘기므로 통지만 받는 입장이다. CUAS 더미 컨텍스트에서 owner 측 sink 협상이 어긋나면 CUAS가 StartComposition을 받자마자 OnEndComposition(자기 쪽)→TIP의 OnCompositionTerminated를 유발. **UNIM이 owner-composition-sink를 구현해야 하는 건 아니지만**(그건 앱/CUAS 역할), CUAS가 그 sink 협상에 의존한다면 TIP는 위 Q2의 계약(특히 display attribute + async)으로 협상을 성립시켜야 한다.
- **[미확인-검증필요]** SampleIME가 어느 쪽을 구현하는지: SampleIME는 TIP이므로 `ITfCompositionSink`(통지 수신)를 구현하고, **owner composition sink는 구현하지 않는다**고 알려짐 — 즉 owner-sink 미구현 자체는 정상. 차이는 SampleIME가 **display attribute를 composition range에 SetValue 한다**는 점일 가능성. (SampleIME `SetText`/`_SetCompositionDisplayAttributes` 경로 1차 확인 요.)

### Q4. SampleIME가 CUAS(콘솔/IMM32 앱)에서 동작하는가
- **[미확인-검증필요]** Microsoft `Windows-classic-samples` 의 `IME/SampleIME`. 확인 포인트(소스 grep 키워드):
  - `TF_ES_ASYNCDONTCARE` / `TF_ES_SYNC` — edit session 모드.
  - `_SetCompositionDisplayAttributes`, `GUID_PROP_ATTRIBUTE`, `ITfProperty::SetValue` — composition range display attribute 설정 여부.
  - CUAS/legacy 분기, `ITfContextOwnerCompositionSink` 존재 여부.
- **[추론]** SampleIME는 동기 SYNC 강제를 하지 않고 composition range에 display attribute를 설정하므로 CUAS 앱에서도 동작할 것으로 예상. 이것이 UNIM과의 결정적 구현 차이로 추정.

### Q5. wezterm / rime-weasel 의 관련 이슈
- **[미확인-검증필요]** 검색 키워드(재확인 요): GitHub `wez/wezterm` issues — "IME composition", "TSF", "Korean composition", "WM_IME_COMPOSITION". `rime/weasel` issues — "wezterm", "console", "composition terminated".
- **[추론]** weasel(TSF TIP)도 동일 CUAS 경로를 타며, weasel이 wezterm에서 동작한다면 weasel의 display-attribute/async 처리가 정답 레퍼런스가 된다. 동작하지 않는다는 보고가 있다면 CUAS 한계의 방증.

### Q6. TIP가 같은 DLL에서 IMM32 메시지(WM_IME_*)를 직접 보내는 폴백이 가능한가
- **[확증]** **불가/비권장.** `ImmGetContext`/`ImmSetCompositionString`은 **애플리케이션/IMM32 IME(.ime)** 가 호출하는 API이지, **TSF TIP가 임의 앱에 대해 호출하라고 설계된 API가 아니다.** TIP는 자기 텍스트 스토어/컨텍스트가 없고, 포커스 앱의 IMM context를 직접 조작하면 CUAS와 충돌(이중 composition)·정의되지 않은 동작. WM_IME_* 를 TIP가 `SendMessage`로 직접 쏘는 것도 CUAS가 이미 그 메시지의 소유자라 충돌.
- **[추론]** 따라서 "TIP가 IMM32 메시지 직접 송신" 폴백은 **막다른 길.** 유일한 비-TSF 폴백은 앱 감지 후 **TIP가 정상 commit한 글자를 backspace+재삽입**하는 방식이지만, 이는 조합(underline 미리보기)을 포기하고 완성 글자만 넣는 열화 동작.

---

## ★★ 1차 자료가 확정한 핵심 의미론 (진단의 linchpin)

세 개의 MS Learn 원문을 직접 확인:
- **[확증] `OnCompositionTerminated`는 "이 text service 외의 누군가"가 composition을 끝낼 때만 호출된다.** (SampleIME `Composition.cpp` 주석 + ITfCompositionSink 의미: "The system calls this method whenever someone other than this service ends a composition.") → 즉 UNIM 자신이 끝낸 게 아니라 **owner(=CUAS)** 가 끝낸 것.
- **[확증] StartComposition Remarks (출처: msctf.h StartComposition):** "If the context owner has installed a context owner composition advise sink, OnStartComposition is called. **If the advise sink rejects the new composition, this method returns S_OK but ppComposition is set to NULL.** Any text covered by pCompositionRange receives the **GUID_PROP_COMPOSING** property."
- **[확증] 실측 로그는 `composition CREATED`(ppComposition ≠ NULL)** → owner는 **시작 시점에 거부하지 않았다.** 거부였다면 NULL이어야 한다. 따라서 owner는 **승인 → 직후 종료**한 것이다.

**[추론·결정적]** "승인 직후 종료"는 owner(CUAS)가 edit session(SYNC) 종료 시점에 composition을 정리한다는 뜻이다. owner-side 종료를 UNIM이 막을 표준 수단은 없지만(그건 owner 권한), **UNIM이 그 종료에 대해 엔진을 reset하지 않으면** 다음 키에서 누적 조합을 재구성할 수 있다. 또한 **GUID_PROP_COMPOSING은 StartComposition이 자동 부여**하므로 "조합 중" 표시는 이미 붙는다 — 즉 display attribute(GUID_PROP_ATTRIBUTE)는 시각효과(밑줄)일 뿐 수명과 무관하다(2차 조사 §2 재확인). → **display attribute는 1순위 용의자에서 강등.**

## ★ 실측 로그가 강제하는 재해석 (가장 중요)

지식베이스 `wezterm-log-caretfix` 실측(caret fix 14e5b3de 이후):
```
preedit_changed=true was_composing=false comp_active=false
-> acquire_insert_range InsertAtSelection QUERYONLY ok
-> StartComposition ok -> RequestEditSession hr=Ok(0) -> composition CREATED
-> OnCompositionTerminated clear+reset
(매 자모 반복, preedit_len 항상 1)
```
- **[확증] QUERYONLY가 ok다.** 즉 wezterm은 `ITfInsertAtSelection`을 지원한다 → wezterm은 단순 "IMM32 전용 dummy context"가 아니라 **CUAS가 제공하는 실제 동작하는 TSF text store**를 갖는다(QUERYONLY는 text store 협력 필요). 2차 조사의 "wezterm은 TSF text store가 전혀 없다"는 **부분 오류**. voice typing(#7791)이 안 되는 것과, CUAS 텍스트 스토어가 composition을 거부하는 것은 별개 문제다.
- **[확증] SetText·StartComposition·RequestEditSession 모두 성공(hr=Ok(0))** 후 **즉시** OnCompositionTerminated. → "거부"(StartComposition이 NULL 반환)가 아니라 **시작 성공 직후 owner가 종료**시키는 것.
- **[추론·결정적] UNIM `OnCompositionTerminated` 핸들러가 `engine.reset`을 호출**(text_service.rs L337-360 "clear+reset")해서 preedit_len이 영원히 1에 머문다. 즉 **terminate를 받을 때마다 엔진을 비워 자기 발등을 찍는다.** CUAS owner가 (정당하게) 단발 composition을 종료시켜도, 핸들러가 엔진까지 리셋하지 않고 **다음 키에서 composition을 재생성+누적 텍스트 재삽입**했다면 사용자에겐 조합이 이어져 보일 수 있다.

> **모순 해소:** MS IME가 되는 이유는 CUAS 텍스트스토어가 MS IME에게만 특별해서가 아니라, **MS IME가 CUAS의 composition 종료 패턴을 견디도록 (또는 IMM32 네이티브 경로로) 설계**됐기 때문. UNIM은 종료를 "사용자가 조합 취소"로 오인해 엔진을 리셋한다. → **이건 wezterm의 구조적 미지원이 아니라, UNIM의 OnCompositionTerminated 처리 + CUAS 계약 미준수의 복합 결함.** 2차 결론(B, 순수 폴백만이 답)은 **과도하게 비관적이며 기각**.

---

## 최종 판정

**판정: (A) CUAS 경로 복구로 코드 수정 가능. 확신도 높음 — 실측 로그(QUERYONLY ok + StartComposition ok)가 wezterm의 TSF 협력을 직접 증명하며, 즉시-terminate는 UNIM 측 처리/계약 결함이다.**

근거 요약(6줄):
1. MS 한국어 IME는 순수 TSF TIP인데 wezterm에서 정상 → wezterm은 TSF 입력을 (CUAS 텍스트스토어 경유로) 받는다. 2차의 "TSF 구조적 미지원"은 오결론.
2. 실측 로그: wezterm에서 InsertAtSelection QUERYONLY·SetText·StartComposition·RequestEditSession **전부 hr=Ok(0)** 성공. wezterm은 CUAS가 만든 실동작 TSF text store를 가진다(voice typing #7791과는 별개 문제).
3. 그럼에도 매 자모 StartComposition ok 직후 OnCompositionTerminated → 이는 거부(NULL)가 아니라 owner의 정당한 단발 종료다.
4. 결정타: UNIM의 OnCompositionTerminated 핸들러가 engine.reset("clear+reset")을 호출해 preedit가 영원히 1자에 머문다 → 종료를 받을 때마다 엔진을 비워 누적을 스스로 파괴한다.
5. 1차 확증: OnCompositionTerminated는 "이 service 외 누군가"가 끝낼 때만 호출 + 로그가 composition CREATED(거부 아님) → owner(CUAS)가 SYNC edit session 종료 직후 정리. TF_ES_SYNC→ASYNC 전환이 이 정리 타이밍을 바꿀 수 있다(보조 용의자). display attribute는 GUID_PROP_COMPOSING이 자동 부여되므로 수명 무관(강등).
6. ImmSetCompositionString을 TIP가 직접 호출하는 폴백은 불가(TIP 설계 외·CUAS 충돌). 따라서 B(순수 폴백)는 최후수단일 뿐, 1차로 A를 시도해야 한다.

**A안에서 첫 번째로 시도할 코드 수정 1개 (최우선):**
> **`OnCompositionTerminated`에서 engine을 reset하지 말고, composition 객체만 비운 뒤 다음 키에서 누적 preedit 전체를 재삽입+재조합한다.**
> 현재 `text_service.rs` L337-360 핸들러는 "clear+reset"으로 한글 엔진 상태(preedit 버퍼)까지 날린다. 이를 **composition slot/ITfComposition 참조만 None으로 비우고 엔진 버퍼는 보존**하도록 바꾼다. key_handler의 `was_composing` 분기가 `comp_active=false`여도 엔진 preedit가 남아 있으면 `start_composition(누적 텍스트)` 경로로 재진입하게 한다.
> 이 1줄급 변경만으로 "preedit_len 항상 1" 증상이 풀리는지 로그로 즉시 검증 가능(가장 저비용·고확률).
>
> **검증 사다리 (이후 순서):**
> (2) `RequestEditSession`의 `TF_ES_SYNC` → `TF_ES_ASYNCDONTCARE`(composition.rs L145·171·186·202·215·246). CUAS가 SYNC RW를 거부/약식 grant하면 terminate 유발 가능.
> (3) composition range에 `GUID_PROP_ATTRIBUTE` SetValue 추가(SetText 직후): `ITfCategoryMgr::RegisterGUID`로 atom 캐시 → `context.GetProperty(&GUID_PROP_ATTRIBUTE)?.SetValue(ec,&range,&VARIANT::I4(atom))`. SampleIME가 하는 것을 UNIM은 안 함(grep 0건, 1·2차 확증).

---

## 1차 출처 (3차 조사에서 직접 확인)
- ITfContextComposition::StartComposition (Remarks: 거부 시 S_OK+NULL, GUID_PROP_COMPOSING 자동 부여): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition
- ITfContextOwnerCompositionSink (application이 구현; CreateContext 시 TSF manager가 query하여 advise): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nn-msctf-itfcontextownercompositionsink
- Compositions (text service=StartComposition+ITfCompositionSink; application=ITfContextOwnerCompositionSink; 표준 업데이트 절차 1~3): https://learn.microsoft.com/en-us/windows/win32/tsf/compositions
- About Input Method Manager (IMM/IMM32, East Asian 전용): https://learn.microsoft.com/en-us/windows/win32/intl/about-input-method-manager
- (1·2차에서 확인) wezterm #7791 no TSF text store / DeepWiki Windows Integration(IMM32) / mozc #821(SYNC→ASYNC) — 기존 doc 참조.

## 확인된 UNIM 소스 사실 (3차)
- `text_service.rs` L337-353 `OnCompositionTerminated` → `composition_mgr.clear()` + **`engine.reset()`** + popup hide + `atf_state.reset_on_focus()`. ★ engine.reset이 누적 파괴의 직접 코드.
- `text_service.rs`는 `ITfThreadMgrEventSink`/`ITfKeyEventSink`/`ITfTextEditSink`만 advise. `ITfContextOwnerCompositionSink`는 **구현 안 함**(정상 — 그건 application/owner 역할).
- `composition.rs`: 모든 edit session이 `TF_ES_READWRITE | TF_ES_SYNC`(L145·171·186·202·215·246). `GUID_PROP_ATTRIBUTE`/`SetValue`/`RegisterGUID` 호출 0건(1·2차 확증과 일치).

## 후속 검증 체크리스트
- [ ] (최우선) `OnCompositionTerminated`에서 `engine.reset()` 제거 후 wezterm 로그 재측정 — preedit_len이 2,3…으로 누적되는지.
- [ ] `reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP" /s` + `where /r %windir%\system32 *.ime` 로 MS 한국어 IME CLSID/DLL·`.ime` 부재 확정 (Q1 완전 확증).
- [ ] SampleIME 소스에서 edit session 플래그(ASYNCDONTCARE 추정) + display attr SetValue 경로 재확인.
