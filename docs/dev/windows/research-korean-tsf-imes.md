# 한국어 TSF IME 오픈소스 조사 — composition 생존 기법 종합

조사 목적: wezterm(CUAS-unaware, 순수 IMM32 터미널)에서 UNIM의 TSF composition이 빈 조합조차 즉시 `OnCompositionTerminated`되는 문제의 원인을, 오픈소스 한국어 TSF IME 소스에서 역추적한다. MS 한국어 IME는 같은 wezterm에서 inline 정상이므로, 서드파티 TSF가 inline을 달성했는지(P1: 우리가 놓침) 아니면 모두 못 하는지(P2: MS 특권)를 판정.

---

## 1) 확인된 한국어 Windows TSF IME 오픈소스

| # | 프로젝트 | URL | 언어 | TSF 여부 | 비고 |
|---|----------|-----|------|----------|------|
| 1 | navilera/NavilIME | https://github.com/navilera/NavilIME | C++ | yes (풀 TSF TIP, libhangul) | **유일하게 다른 composition 패턴** |
| 2 | Lee0701/libime-libhangul | https://github.com/Lee0701/libime-libhangul (코어는 submodule Lee0701/libIME2) | C++ | yes (PIME/MS샘플 fork) | UNIM과 동일 패턴, properties는 더 적음 |
| 3 | rayshoo/kolemak | https://github.com/rayshoo/kolemak | C | yes (두벌식+Colemak) | UNIM과 동일 빈-range 패턴, properties 0 |
| 4 | ccy5123/kor_based_jap | https://github.com/ccy5123/kor_based_jap | C++ | partial (한글입력층→일본어) | **빈-range 먼저 StartComposition** = UNIM과 동일 |
| 5 | wkpark/saenaru | https://github.com/wkpark/saenaru | C | yes (IMM32 주력 + `tip/` TSF 경로) | TSF 경로는 NavilIME과 같은 non-empty/fInterimChar 패턴 |
| 6 | saschanaz/saenaru | https://github.com/saschanaz/saenaru | C | partial (mirror) | TSF 모듈은 composition 안 함(랭바 전용), 조합은 100% IMM32 |

확증: 6개 중 **순수/실질 한국어 TSF composition을 구현한 것은 4개**(NavilIME, libime-libhangul, kolemak, kor_based_jap). saenaru(2종)는 TSF 경로가 있으나 조합 본체는 IMM32.

---

## 2) 레거시/터미널에서 inline을 달성한 증거?

**직접적 확증(runtime/문서/이슈 증거)은 어느 저장소에도 없다.**
- NavilIME, libime, kolemak, kor_based_jap, saenaru 전부: 소스/README/이슈에 console·CUAS·IMM32·wezterm·terminal·conhost inline 동작 또는 한계에 대한 언급/테스트/폴백 코드 **0건**.
- kolemak의 `test/enter-key-test-results.md`는 Chrome 주소창·naver 웹폼·KakaoTalk·게임챗만 테스트(터미널 없음), 주제는 Enter 재주입 타이밍(조합 생존과 무관).
- saschanaz/wkpark saenaru의 콘솔 훅(`ui.c:1251 SetConsoleHookFunc` / `1610 SAENARUConKbdProc`)은 `WH_KEYBOARD_LL` 훅만 걸고 콜백이 `MyDebugPrint`만 하는 **무동작 스텁** → 동시대 한국어 IME조차 콘솔 inline을 정공법으로 못 풀었다는 방증.

추론(확증 아님): NavilIME/saenaru가 쓰는 **non-empty range + fInterimChar=TRUE** 패턴이 IMM32 interim-char 의미론에 매핑되어 CUAS 브리지의 즉시 terminate를 회피할 *개연성*이 가장 높다. 그러나 wezterm 특정 동작은 코드로 입증되지 않음(inferential).

### UNIM이 안 한 것 위주의 핵심 기법 (코드 근거)

**기법 A — fInterimChar=TRUE (조합 중 selection style)** [가장 유력]
- NavilIME: `EditSession.cpp` L133, 조합 중 `TF_SELECTION.style.fInterimChar=TRUE`, 커밋/종료시에만 FALSE.
- saenaru: `tip/compose.cpp:227`, `keys.cpp:484` 모든 조합 SetSelection에 fInterimChar=TRUE, 커밋 `compose.cpp:286`에서 FALSE.
- kolemak: `src/edit_session.c` `SetInterimSelection()` = `ase=TF_AE_NONE, fInterimChar=TRUE` over full composition range.
- **3개 독립 저장소가 동일하게 사용.** UNIM은 `TF_AE_NONE`만 설정, fInterimChar 미설정 → 단일 최유력 parity 차이.
- (대조) kor_based_jap는 fInterimChar를 조합 유지 경로에서 안 씀 → UNIM과 같은 결함 예상.

**기법 B — non-empty range 위에서 StartComposition** [유력, NavilIME/saenaru 확증]
- NavilIME (`EditSession.cpp` ~L80-140): `InsertTextAtSelection(ec, 0, &ch, 1, &pRangeInsert)`로 **첫 자모를 실제 삽입**해 non-empty range 확보 → 그 range 위에서 `StartComposition`. 즉 빈 composition이 존재하는 순간이 없음.
- saenaru (`tip/compose.cpp:191,201,213`): `InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &ch, len, &pRangeInsert)`로 char/len을 넘겨 올바른 extent의 range를 얻고 → StartComposition → **그 다음** SetText(TF_ST_CORRECTION).
- **UNIM은 반대**: 자작 빈(zero-length) range에 StartComposition 먼저 → SetText. kor_based_jap도 동일(`Composition.cpp` L106-164, 헤더 주석 'StartComposition with zero-length range then SetText')이며 동일 결함 예상.
- CUAS/IMM32 브리지가 zero-length composition을 즉시 terminate하는 것으로 보이며, NavilIME/saenaru는 애초에 빈 composition을 만들지 않음.

**기법 C — SetText에 TF_ST_CORRECTION** [부차]
- NavilIME `Composition.cpp`, saenaru `compose.cpp:213/keys.cpp:478`: 호스트가 삽입점을 조합 내에서 이동/조정하지 않도록 TF_ST_CORRECTION 사용.

### 반증된 가설 (확증)
- **GUID_PROP_READING / GUID_PROP_ATTRIBUTE / GUID_PROP_COMPOSING가 생존을 좌우한다 → 반증.** NavilIME·kolemak는 GUID_PROP_* **0건**으로도 정상 구조; libime/kor_based_jap/saenaru는 GUID_PROP_ATTRIBUTE만(밑줄용), READING/COMPOSING 없음. **UNIM만 GUID_PROP_READING(VT_BSTR)을 set** → 이는 생존 요인이 아니며, 오히려 제거 A/B 테스트 대상.
- **ITfContextOwnerCompositionSink 미구현이 원인 → 반증.** 4개 전부 미구현(앱측이라 정상). UNIM과 동일.
- **추가 인터페이스가 원인 → 반증.** NavilIME/kolemak/libime는 UNIM보다 *적은* 인터페이스만 구현. saenaru가 추가로 가진 ITfTextEditSink/ITfCleanupContext*Sink/ITfCreatePropertyStore는 생존이 아닌 정리/관찰용.

---

## 3) P1 vs P2 판정

**판정: P1(우리가 놓침)으로 우선 시도, 단 확증 아닌 inferential. P2(MS 특권) 가능성도 잔존.**

- P1 근거: 서드파티 한국어 TSF(NavilIME, saenaru-tip)가 UNIM이 안 한 두 기법(non-empty range start + fInterimChar=TRUE)을 명시적으로 사용. UNIM은 이를 시도한 적 없음 → "전부 시도했다"는 목록에 이 둘이 빠져 있음. 저비용·저위험 parity 변경으로 검증 가치 높음.
- P1 한계: 어느 저장소도 wezterm/CUAS 터미널 inline을 *실증*하지 않음. fInterimChar/non-empty-range가 wezterm 즉시-terminate를 막는다는 것은 TSF 의미론 기반 추론.
- P2 잔존 근거: kolemak/libime는 (NavilIME 기법 없이) 동일 빈-range·무속성 패턴인데도 일반 앱에서 동작 → 만약 이들이 wezterm에서도 죽는다면, MS 한국어 IME의 wezterm inline은 **호스트측 IMM32↔TSF(CUAS) 브리지 opt-in**(앱이 cicero/CUAS 인지)이 진짜 변수일 수 있음. saenaru의 정공법이 "IMM32 IME(.ime)로 등록"인 점도 P2를 지지.

**실행 방침**: 먼저 P1 기법 A→B→C 순으로 A/B 테스트. 효과 없으면 P2 확정 → 오버레이 폴백 유지 또는 IMM32 브리지(.ime)로 전환(UNIM `docs/dev/windows/bridge-*.md` 방향과 일치).

---

## 4) UNIM 적용 후보 변경 (위험도)

1. **[저위험·즉시] fInterimChar=TRUE** — 조합 중 모든 SetSelection의 `TF_SELECTION.style.fInterimChar=TRUE`, 커밋/종료시 FALSE. 3개 저장소 공통. 화면상 블록커서 표시 변화 외 부작용 작음. **1순위.**
2. **[중위험] non-empty range start** — 단일 sync RW 세션에서 `InsertTextAtSelection`(NavilIME식 실제삽입 0-flag, 또는 saenaru식 QUERYONLY+char/len)로 range 확보 → 그 위에서 StartComposition → SetText(TF_ST_CORRECTION). UNIM의 빈-range-first를 폐기하는 구조 변경이라 커밋/백스페이스/취소 경로 회귀 점검 필요. **2순위(A가 무효일 때).**
3. **[저위험] SetText에 TF_ST_CORRECTION 플래그** — 삽입점 이동 방지. B와 함께 적용.
4. **[저위험·검증용] GUID_PROP_READING 제거 A/B** — 어느 한국어 TSF도 안 set. UNIM만의 잠재 트리거 가능성, 제거 후 wezterm 재현 확인.
5. **[참고] TIPCAP 세트 비교** — kolemak `dll_main.c` L472-522와 UNIM 등록 카테고리 대조(IMMERSIVE/SYSTRAY/SECUREMODE/UIELEMENT/INPUTMODECOMPARTMENT). 생존 직결은 아니나 parity 확인.

**핵심 소스 근거 URL**
- NavilIME: `EditSession.cpp`, `Composition.cpp`, `TextService.h` — https://raw.githubusercontent.com/navilera/NavilIME/master/NavilIME/
- saenaru tip: `tip/compose.cpp`, `tip/keys.cpp`, `tip/saenarutip.h` — https://raw.githubusercontent.com/wkpark/saenaru/master/
- kolemak: `src/edit_session.c`, `src/text_service.c`, `src/dll_main.c` — https://github.com/rayshoo/kolemak
- libIME2: `src/TextService.cpp` — https://raw.githubusercontent.com/Lee0701/libIME2/HEAD/src/TextService.cpp
- kor_based_jap: `tsf/src/Composition.cpp` — https://raw.githubusercontent.com/ccy5123/kor_based_jap/HEAD/tsf/src/
