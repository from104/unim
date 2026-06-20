# wezterm TSF composition 즉시 종료 버그 — 기술 조사

조사일: 2026-06-01 / 대상: UNIM (Rust TSF IME, windows-rs 0.62) / 브랜치: feat/windows-msi-redesign
1차 자료: MS 공식 문서, Microsoft TSF SampleIME 소스, wezterm 소스/이슈.

> 표기: **[확증]** = 1차 자료로 뒷받침. **[추측]** = 정황상 유력, 실측 로그로 검증 요망.

---

## 0. 실제 코드 현황 (과제 설명과의 차이 — 먼저 정정)

`unim-tsf/src/composition.rs`(543줄)는 과제 설명보다 진화돼 있다:
- `acquire_insert_range()` — `InsertTextAtSelection(QUERYONLY)` 시도 → 실패 시 `GetSelection` 폴백 (wezterm/conhost 대응 이미 있음).
- `StartCompositionEditSession.DoEditSession` 순서: ① range 획득 → ② `SetText` → ③ `StartComposition` → ④ `move_caret_to_end`.
- `display_attr.rs`에 `ITfDisplayAttributeInfo` 2종(Input/Converted) + `IEnumTfDisplayAttributeInfo` 구현, `text_service.rs`에 `ITfDisplayAttributeProvider` 구현, `globals.rs`에 표시속성 GUID 2개 정의.

**핵심 발견 (코드 grep으로 확증):**
- crate 전체에서 `GUID_PROP_ATTRIBUTE`에 대한 `GetProperty`/`SetValue` 호출이 **단 한 곳도 없다**. `ITfCategoryMgr::RegisterGUID`(GUID→atom 변환) 호출도 없다.
  → 즉 **표시속성을 OS에 "제공"할 준비(Provider)는 돼 있으나, 실제 composition range에 "적용"(SetValue)하는 코드가 빠져 있다.** 이것이 SampleIME와의 결정적 차이.
- `move_caret_to_end()`(L16-30)는 `SetSelection`에서 `style.ase = TF_AE_END`를 쓴다. SampleIME는 동일 상황에서 `TF_AE_NONE`을 쓴다(아래 §5 확증).

---

## 1. StartComposition 직후 OnCompositionTerminated 즉시 호출의 원인

**[확증]** `OnCompositionTerminated`(`ITfCompositionSink`)는 *이 서비스 외의 누군가가* composition을 끝낼 때 호출된다. SampleIME 주석 원문:
> "The system calls this method whenever someone **other than this service** ends a composition."
출처: SampleIME `Composition.cpp` (인덱스 `sampleime-composition-cpp`).

**[확증]** `StartComposition`이 즉시 거부될 때는 `ppComposition = NULL` + `S_OK`다(터미네이트 콜백이 아님). 출처 — StartComposition Remarks:
> "If the advise sink rejects the new composition, this method returns S_OK but ppComposition is set to NULL." / "Any text covered by pCompositionRange receives the **GUID_PROP_COMPOSING** property."
출처: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition

→ UNIM 증상(StartComposition은 성공해 comp 객체를 얻고, **직후** OnCompositionTerminated)은 "거부"가 아니라 **시작 직후 앱이 강제 종료**하는 케이스. 앱이 종료시키는 대표 트리거: **selection(caret)이 composition range 밖으로 이동**(MS Compositions 문서의 표준 업데이트 절차 1~3단계 및 일반 동작).
출처: https://learn.microsoft.com/en-us/windows/win32/tsf/compositions

**터미널 특이성 [확증]:** wezterm Windows 입력은 **IMM32 기반**이다. `window/src/os/windows/window.rs`가 `WM_IME_SETCONTEXT / WM_IME_COMPOSITION / WM_IME_ENDCOMPOSITION`를 처리하고, `GCS_COMPSTR(0x8)`·`GCS_RESULTSTR(0x800)`를 `ImmGetCompositionStringW`로 읽는다. TSF 텍스트스토어를 직접 호스팅하지 않는다. 출처: 인덱스 `wezterm-window`.
→ UNIM의 TSF 조합은 OS의 **cicero TSF→IMM32 브리지**를 경유해 wezterm에 전달된다. 메모장(표준 edit 컨트롤=관대한 텍스트스토어)은 정상, wezterm(브리지 경유)은 실패하는 패턴과 부합. **[확증·정황]** wezterm은 TSF 텍스트스토어가 없다는 이슈가 실재한다: #7791 "Windows: Win+H voice typing produces no input (no TSF text store)". 출처: https://github.com/wezterm/wezterm/issues/7791

---

## 2. SampleIME vs UNIM — display attribute property가 필수인가

**[확증]** MS Compositions 문서:
> "If a text service is going to create compositions, it should also **support display attributes**…"
출처: https://learn.microsoft.com/en-us/windows/win32/tsf/compositions

**[확증]** SampleIME는 조합 텍스트 삽입 후 곧바로 표시속성을 range에 **SetValue**한다:
- `_SetCompositionDisplayAttributes(ec, pContext, _gaDisplayAttributeInput);` 호출.
- 그 내부는 `GetProperty(GUID_PROP_ATTRIBUTE)` → `pProperty->SetValue(ec, pRangeComposition, &var)` 패턴(같은 함수에서 `pLanguageProperty->SetValue(ec, pRangeComposition, &var)` 형태 확인). `_gaDisplayAttributeInput`은 `ITfCategoryMgr::RegisterGUID`로 미리 얻은 **TfGuidAtom**.
출처: 인덱스 `sampleime-composition-cpp`.

**[확증]** `GUID_PROP_ATTRIBUTE`는 표시속성 GUID의 atom을 담는 property, `GUID_PROP_COMPOSING`은 "조합 중" 불리언 property. 출처: https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-properties

**UNIM 차이 [확증]:** UNIM은 `_SetCompositionDisplayAttributes` 상당 코드가 **전무**(§0). `GUID_PROP_COMPOSING`은 StartComposition이 자동 부여하지만, `GUID_PROP_ATTRIBUTE`는 IME가 직접 SetValue해야 하며 UNIM은 하지 않는다.

**필수성 판정:**
- **[확증·부분]** 표시속성 미설정 → preedit 밑줄이 안 그려지는 것의 직접 원인(증상 "밑줄 안 뜸"과 정확히 일치).
- **[추측·유력]** TSF→IMM32 브리지에서, 표시속성(=GCS_ATTR/clause 정보의 원천)이 없는 composition은 브리지가 GCS_COMPSTR로 매핑할 attribute가 없어 조합을 무의미/즉시확정으로 처리할 개연성. GUI(자체 텍스트스토어)는 관대해서 통과.

---

## 3. wezterm의 TSF/IME 처리 및 관련 이슈

**[확증]** IMM32 메시지 핸들러 존재(§1). preedit 렌더링 옵션 `ImePreeditRendering`(config) 보유. 출처: `wezterm-window`.
**[확증]** 관련 공개 이슈:
- #7791 voice typing no input — **"no TSF text store"** 명시. https://github.com/wezterm/wezterm/issues/7791
- #7738 panic in window.rs:1581 — TSF/IME가 `ImmTranslateMessage` 경유 입력 시 패닉. https://github.com/wezterm/wezterm/issues/7738
- #7780 IME candidate window 위치 문제. https://github.com/wezterm/wezterm/issues/7780

→ **[확증]** wezterm은 네이티브 TSF 텍스트스토어를 제공하지 않으며 IMM32 브리지에 의존한다는 점이 1차 확인됨. UNIM이 "OnCompositionTerminated 즉시"를 겪는 환경적 토대.

---

## 4. InsertTextAtSelection(QUERYONLY) range로 StartComposition

**[확증]** SampleIME도 **동일 패턴**: `pias->InsertTextAtSelection(ec, TF_IAS_QUERYONLY, …, &rangeInsert)` 로 얻은 range로 조합. 출처: 인덱스 `sampleime-composition-cpp`.
→ **이 방식 자체는 정상이며 근본 원인이 아니다.** UNIM의 `acquire_insert_range` 폴백도 합리적. 단 wezterm에서 QUERYONLY가 실패해 `GetSelection` 폴백을 타는지 여부는 **실측 로그(dbg_log "InsertAtSelection FAILED … GetSelection fallback")로 확인 필요** — 폴백 range는 0폭이라 이후 SetText/selection 동기화가 더 취약.

---

## 5. StartComposition 직후 같은 edit session 내 SetSelection(caret 이동)

**[확증·중요]** SampleIME가 조합 중 selection을 세팅하는 두 패턴:
1. `tfSelection.range->Collapse(ec, TF_ANCHOR_END); pContext->SetSelection(ec, 1, &tfSelection);` — 단, 이 `tfSelection`은 `GetSelection`으로 얻은 **원본 selection 구조체**(style 보존).
2. 명시적으로 `sel.style.ase = TF_AE_NONE; sel.style.fInterimChar = FALSE; pContext->SetSelection(ec, 1, &sel);`
출처: 인덱스 `sampleime-composition-cpp`.

**UNIM 차이 [확증]:** `move_caret_to_end`(composition.rs L19-24)는 `style.ase = TF_AE_END`. SampleIME는 조합 caret에 **`TF_AE_NONE`**을 쓴다.
- `TF_AE_END`는 "active end가 range의 끝"이라는 *방향성*을 명시 → 앱/브리지가 selection을 composition 경계 **밖**(exclusive end 너머)으로 해석할 여지. `TF_AE_NONE`은 방향성 미지정(단순 캐럿)이라 더 안전.
- **[추측·매우 유력]** 표시속성 없는 composition + `TF_AE_END`로 caret을 range 끝에 두는 조합이, 브리지에서 "selection이 조합 밖" → 즉시 OnCompositionTerminated를 유발.

---

## 결론 — 가장 유력한 근본 원인 (1·2순위)

### 1순위 [확증→추측] composition range에 **GUID_PROP_ATTRIBUTE(SetValue) 미적용**
- **확증 부분:** UNIM은 표시속성 SetValue 코드가 전혀 없음(grep 0건). SampleIME는 반드시 함. → preedit 밑줄 미표시는 이것의 *확정적* 원인.
- **추측 부분:** wezterm(IMM32 브리지)에서 속성 없는 조합을 즉시 종료/확정시키는지는 실측 검증 필요. 메모장 정상/wezterm 실패 패턴과 부합.

### 2순위 [확증→추측] `SetSelection`에서 **`TF_AE_END` 사용 (SampleIME는 `TF_AE_NONE`)**
- 확증: 코드 차이 사실. 추측: 이 차이가 브리지에서 "caret 조합 밖" 판정을 유발.

> 두 원인은 배타적이지 않다. **둘 다 수정**하고 dbg_log로 효과를 격리 측정할 것.

---

## UNIM 코드 레벨 수정 방향 (composition.rs)

**A. 표시속성 적용 (신규, 1순위)** — `StartCompositionEditSession.DoEditSession`에서 `StartComposition` 성공 직후, `move_caret_to_end` **이전**에:
1. 프로세스 1회 초기화: `let cat: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;` → `let atom = cat.RegisterGUID(&globals::UNIM_DISPLAY_ATTR_INPUT)?;` (atom 캐시).
2. range에 적용:
   ```rust
   let prop = context.GetProperty(&GUID_PROP_ATTRIBUTE)?;
   let var = VARIANT::from(atom as i32); // VT_I4 = TfGuidAtom
   prop.SetValue(ec, &range, &var)?;
   ```
   (windows-rs 0.62: `ITfContext::GetProperty`, `ITfProperty::SetValue`, `VARIANT` I4.)
- `update_composition`/`replace_surrounding`의 SetText 직후에도 동일 SetValue 재적용 (range가 매번 재생성/재설정되므로).

**B. caret style 교정 (1줄, 2순위)** — `move_caret_to_end`의 `ase: TF_AE_END` → `ase: TF_AE_NONE`로 변경. (또는 SampleIME처럼 `GetSelection`으로 얻은 원본 selection을 Collapse(END) 후 그대로 SetSelection.)

**C. 표시속성 Provider 카테고리 등록 확인 [확증·점검 필요]** — `RegisterCategory(clsid, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, clsid)`가 DllRegisterServer/WiX에 들어가 있어야 `GetGUID(atom)`이 OS에서 유효. register.rs L9 주석상 "wxs가 동일 키를 박는다"고 돼 있으나 **DISPLAYATTRIBUTEPROVIDER 카테고리 키가 실제 존재하는지 wxs/regsvr 양쪽 확인 요망**. 누락 시 A가 무력화.

### 검증 절차
1. 먼저 **B만** 적용(저비용) → wezterm 로그에서 OnCompositionTerminated 즉시 호출 사라지는지.
2. 안 되면 **A** 적용 → preedit 밑줄 + 조합 유지 확인.
3. dbg_log에 `acquire_insert_range` 폴백 경로 여부, `StartComposition` 반환 comp 유무, OnCompositionTerminated 타임스탬프를 남겨 A/B 효과 격리.

---

## 1차 출처
- MS Compositions(TSF): https://learn.microsoft.com/en-us/windows/win32/tsf/compositions
- MS StartComposition: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontextcomposition-startcomposition
- MS Predefined Properties: https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-properties
- SampleIME Composition.cpp: https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME/cpp/SampleIME
- wezterm window.rs(IMM32 IME): https://github.com/wezterm/wezterm/blob/main/window/src/os/windows/window.rs
- wezterm issues: #7791 https://github.com/wezterm/wezterm/issues/7791 · #7738 https://github.com/wezterm/wezterm/issues/7738 · #7780 https://github.com/wezterm/wezterm/issues/7780

지식베이스 source 라벨: `msdn-tsf-compositions`, `msdn-startcomposition`, `msdn-tsf-properties`, `sampleime-composition-cpp`, `sampleime-tree`, `wezterm-window`, `wezterm-issues-ime`, `wezterm-tsf-issues`

---

## 2차 조사 (caret 기각 이후)

조사일: 2026-06-01 (후속). 실측: `select_composition_range`(TF_AE_NONE, 전체 range)로 SetSelection을 바꿔도 StartComposition 직후 OnCompositionTerminated 즉시 호출이 **100% 동일 재현**.

### 0. 새 실측으로 기각된 가설
- **caret/SetSelection (기존 1·2순위) → 기각.** caret을 0폭 END에서 composition range 전체(TF_AE_NONE)로 바꿔도 증상 동일. caret 위치·정렬은 무관.
- **SYNC edit session 거부 → 기각.** 로그가 `RequestEditSession hr=Ok(0)` → SYNC 세션이 **승인됨**. SYNC 거부(TF_E_SYNCHRONOUS)는 일어나지 않는다. 따라서 SYNC 자체가 즉시 종료의 직접 원인이 아니다.
  - 출처: MS Learn ITfContext::RequestEditSession — https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession (RW 동기 세션은 거부될 수 있으니 async 권장; 단 wezterm은 거부 안 함)

### 1. ★ 새 1순위 근본 원인 — wezterm은 TSF text store(ITextStoreACP)를 구현하지 않음 (IMM32 전용 + CUAS 브리지)
**확신도: 매우 높음 (1차 maintainer 코드 + wezterm 이슈 제목 + CUAS 메커니즘 일치).**

- **wezterm의 Windows IME는 IMM32 전용이다.** wnd_proc 메시지 루프에서 `WM_IME_STARTCOMPOSITION` / `WM_IME_COMPOSITION` / `WM_IME_ENDCOMPOSITION`을 처리하고, `ImmGetCompositionString`로 `GCS_COMPSTR`(preedit)·`GCS_RESULTSTR`(commit)을 읽어 **인라인 preedit를 직접 렌더**한다. wezterm은 자체 `ITextStoreACP`/`ITfContext` 문서를 **제공하지 않는다.**
  - 출처: wezterm window.rs — https://github.com/wezterm/wezterm/blob/main/window/src/os/windows/window.rs
  - 출처: DeepWiki Windows Integration — https://deepwiki.com/wezterm/wezterm/4.3-windows-integration ("uses the IMM32 API ... WM_IME_STARTCOMPOSITION, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION")
- **결정적 1차 증거(이슈 제목):** wezterm #7791 — *"Windows: Win+H voice typing produces no input **(no TSF text store)**"*. 메인테이너/리포터가 wezterm에 TSF text store가 없음을 명시. (voice typing은 순수 TSF text-input을 요구 → 동작 안 함.)
  - 출처: https://github.com/wezterm/wezterm/issues/7791
- **CUAS 경로:** 순수 TSF IME(UNIM)가 IMM32 전용 앱(wezterm)에 붙으면 TSF 매니저는 **CUAS(Cicero Unaware Application Support)** 를 통해 TSF↔IMM32를 변환한다. IMM32 전용 앱에는 실제 문서 컨텍스트가 없고 **default/dummy context**만 주어진다. 이 컨텍스트에서 IME가 ITfContextComposition::StartComposition으로 만든 composition은 진짜 문서에 앵커되지 못하고, CUAS가 edit session 종료 직후 이를 즉시 commit/terminate → `OnCompositionTerminated` 즉시 호출. 정확히 관측 로그와 일치.
  - 출처: CUAS 개요 — https://bugzilla.mozilla.org/show_bug.cgi?id=866736 / TSF↔IMM32 독립성 — https://learn.microsoft.com/en-us/windows/win32/tsf/compositions
- **질문 3의 답(핵심):** 이것은 **wezterm의 구조적 한계**다. 순수 TSF IME(UNIM, rime/weasel, SampleIME)는 wezterm의 native TSF 문서가 없으므로 TSF 경로로 안정적 composition을 유지할 수 없다. **MS IME가 되는 이유는 MS IME가 CUAS를 통해 IMM32 메시지(WM_IME_COMPOSITION)로 변환되는 네이티브 폴백 경로를 갖기 때문**이다 — wezterm은 이 IMM32 메시지만 인라인 렌더한다. → **TSF SetSelection/display attribute/edit session을 아무리 고쳐도 코드로 해결 불가.** 폴백 전략이 필요하다.

### 2. display attribute(GUID_PROP_ATTRIBUTE) 미설정의 영향 (질문 4)
**확신도: 높음.** display attribute property는 composition의 **밑줄/배경(시각 표시)** 에만 영향. composition의 **수명(life cycle)** 과는 인과관계가 **없다**. GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER 미등록 시 정상 TSF 호스트에서 밑줄이 안 그려질 뿐, 즉시 종료의 원인은 아니다. → 이번 증상과 무관(기존 1순위 가설도 사실상 약화).
  - 출처: MS Predefined Properties — https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-properties

### 3. SYNC vs ASYNC 보충 (질문 1)
- SampleIME / 권장 관행: composition 시작 시 `TF_ES_ASYNCDONTCARE | TF_ES_READWRITE`. UNIM은 `TF_ES_READWRITE | TF_ES_SYNC` 사용(`composition.rs:145,171,186,202,215,246`).
- MS 공식: "두 번째 규칙 — 동기 edit session을 피하라. Word 등 다수 text store는 동기 세션을 절대 승인하지 않는다." Mozc도 동기 의존을 async로 재설계함(mozc #821).
  - 출처: https://learn.microsoft.com/en-us/windows/win32/tsf/tf-es--constants / https://github.com/google/mozc/issues/821
- 단, wezterm은 SYNC를 승인하므로 이번 증상의 원인은 아니다. **다른 앱(Word 등) 호환을 위해 async 폴백으로 바꾸는 것은 별개의 안전성 개선으로 가치 있음.**

### 4. 폴백 전략 — composition 없이 직접 삽입 (질문 5, 실재함)
TSF↔IMM32는 서로 이벤트가 전달되지 않으므로(MS Compositions), composition을 유지 못 하는 앱에는 IME가 다음 폴백을 쓴다:
- **"backspace + reinsert" 패턴:** composition을 만들지 않고, 매 자모 입력마다 직전에 넣은 임시 음절을 지운 뒤 현재 조합 결과 음절을 그 자리에 직접 삽입(즉시 commit). 한글은 음절이 자모마다 변하므로(ㄱ→가→각) "이전 임시 글자 삭제 → 새 임시 글자 삽입" 반복, 음절 확정 시 그대로 둠.
- 구현 후보:
  - (a) **TSF 직접 삽입:** 매 키마다 동일 range를 SetText로 덮어쓰되 ITfContextComposition으로 감싸지 않음(즉시 commit). default context에서도 대개 동작.
  - (b) **IMM32 폴백:** 앱이 IMM32-only면 IME가 IMM32 경로로 GCS_COMPSTR/RESULTSTR을 흘려보냄 — wezterm이 가장 잘 렌더하는 경로. (단 순수 TSF TIP이 IMM32 메시지를 직접 합성하는 것은 비표준; CUAS에 의존.)

### 5. UNIM 코드 수정 방향 (확신도 순)
1. **(권장 1순위) 앱별 폴백 — composition 미유지 앱 감지 후 "임시 글자 직접 삽입" 모드.**
   - 감지: 같은 키 처리 cycle 내에서 StartComposition 직후 OnCompositionTerminated가 들어오면 "composition 비지원 앱" 플래그 세팅(앱/HWND 단위 캐시).
   - 동작: 해당 앱에서 composition을 만들지 않고, 매 자모 입력마다 `이전 임시 음절 길이만큼 삭제 + 현재 조합 음절 SetText 직접 삽입`(즉시 commit). 음절 경계에서 임시 글자를 확정으로 유지.
   - 위치: `composition.rs`(StartComposition 경로 옆 폴백 분기 신설), `key_handler.rs`(was_composing/comp_active 상태머신에 `fallback_direct_insert` 모드 추가), `text_service.rs`(앱별 플래그 보관).
2. **(2순위, 호환성 보강) RequestEditSession을 `TF_ES_ASYNCDONTCARE | TF_ES_READWRITE`로 변경 + SYNC 거부 시 async 폴백.** 이번 증상은 못 고치지만 Word 등에서의 거부/멈춤(mozc #819 유형) 예방. 위치: `composition.rs:145,171,186,202,215,246`.
3. **(3순위, 무관하나 정합성) DISPLAYATTRIBUTEPROVIDER 카테고리 등록 + composition range에 GUID_PROP_ATTRIBUTE SetValue.** 정상 TSF 호스트에서 preedit 밑줄용. 이번 증상과 무관.

### 6. 핵심 결론 (6줄)
1. caret/SetSelection도, SYNC 거부도 원인이 아니다(둘 다 새 실측으로 기각; SYNC는 `Ok(0)`로 승인됨).
2. 진짜 원인: **wezterm은 TSF text store(ITextStoreACP)를 구현하지 않는 IMM32 전용 앱**이며, 순수 TSF IME는 CUAS의 dummy default context만 받아 composition이 즉시 종료된다(이슈 #7791 "no TSF text store").
3. 이는 wezterm의 **구조적 한계** — TSF SetSelection/display attribute/edit session을 고쳐도 코드로 해결 불가.
4. MS IME가 되는 건 CUAS를 통한 IMM32(WM_IME_COMPOSITION) 네이티브 폴백 경로를 함께 갖기 때문이다.
5. display attribute 미설정은 밑줄(시각)에만 영향, composition 유지와 무관 → 기존 1순위 가설도 약화.
6. 해법은 코드 수정이 아니라 **폴백 전략**: composition 미지원 앱 감지 후 "임시 글자 직접 삽입(backspace+reinsert)" 모드.

### 2차 1차 출처
- wezterm #7791 (no TSF text store): https://github.com/wezterm/wezterm/issues/7791
- wezterm #7738 (TSF/IMM ImmTranslateMessage panic): https://github.com/wezterm/wezterm/issues/7738
- wezterm #2569 (IMM32 preedit 렌더): https://github.com/wezterm/wezterm/issues/2569
- DeepWiki Windows Integration(IMM32 메시지): https://deepwiki.com/wezterm/wezterm/4.3-windows-integration
- MS RequestEditSession: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcontext-requesteditsession
- MS TF_ES_ Constants(동기 회피 규칙): https://learn.microsoft.com/en-us/windows/win32/tsf/tf-es--constants
- MS Compositions(TSF↔IMM32 독립): https://learn.microsoft.com/en-us/windows/win32/tsf/compositions
- mozc #821 (동기→비동기 재설계): https://github.com/google/mozc/issues/821
- CUAS(Cicero Unaware App Support): https://bugzilla.mozilla.org/show_bug.cgi?id=866736
