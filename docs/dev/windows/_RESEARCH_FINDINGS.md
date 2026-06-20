# UNIM Windows — 웹조사 갭 확정 결론 (G1~G6)

> 작성일 2026-06-19 · 입력: `_RESEARCH_GAPS.md`(G1~G6) + 6개 갭 deep-research + 적대적 검증(refutation/cross-check) 결과.
> 채택 원칙: **verdict=confirmed/partly 인 답만 확정 결론으로 채택**. partly 는 검증으로 깎인 부분을 명시하고, 코어 결론만 채택.
> refuted/inconclusive 는 없었음(6/6 모두 confirmed 또는 partly). 잔여 불확실성은 각 갭 말미 + `_IMPLEMENTATION_PATHS.md` §4 체크리스트에 모음.

---

## 채택 요약 표

| 갭 | 우선 | verdict(투표) | 한 줄 결론 |
|---|---|---|---|
| G1 | P0 | confirmed / partly / partly → **확정채택(코어)** | 조합 중 `fInterimChar=TRUE`, 확정·종료 시 `FALSE` 비대칭이 CUAS GCS_COMPSTR keep-alive 의 한국어 IME 관습. 단 "유일한 트리거" 는 과장 — 폴백 안전망 유지 필수. |
| G2 | P0 | confirmed / confirmed / partly → **확정채택(코어)** | 한 바이너리를 IMM32+TSF 자동 듀얼 등록하는 정석은 1차소스에 없음. 레퍼런스(Mozc/SampleIME)는 TSF-only+CUAS 의존. 순수 IMM32-only 앱의 OnKeyDown 0회는 키-라우팅 문제. |
| G3 | P1 | confirmed → **확정채택** | `SHOWNINTRAY`/`SHOWNINTRAYONLY` 둘 다 "not currently supported". 트레이는 OS Input Indicator 경로 전용. compatible IME 체크리스트 확정. |
| G4 | P1 | partly → **확정채택(코어)** | `ShowStatus` 0=floating/3=hidden/4=docked 확정. 단 GPO `Disable Thread Input Manager=1` 은 커뮤니티/추론(미검증). |
| G5 | P2 | partly → **확정채택(코어)** | 한국어 한자 후보 단계에 `ATTR_TARGET_CONVERTED` 보장 안 됨. 일본어식 in-composition target-clause 모델은 한국어에 부적합 — 별도 후보창 모델. |
| G6 | P2 | confirmed → **확정채택** | `@dll,-id` 음수=리소스 ID. STRINGTABLE id 1 을 PE 에 임베드하면 `,-1` 표시명 동작. 미서명은 데스크톱 하드차단 아님(경고만), UWP/PPL/WDAC/AV 만 거부. |

---

## G1 — CUAS GCS_COMPSTR keep-alive 상태머신 (P0, 확정채택)

**질문**: msctf.dll CUAS default text store 가 composition 을 GCS_COMPSTR(미확정)로 유지 vs `OnCompositionTerminated:IMMEDIATE` 발사하는 트리거? `fInterimChar=TRUE` 의 interim 의미론이 WM_IME_COMPOSITION GCS_* 시퀀스로 어떻게 매핑되나? Win8+ CUAS always-on 인가?

**확정 결론(검증 통과분만)**:
1. **`fInterimChar` 의미론(verbatim, official)**: interim character selection 은 정확히 한 글자 폭이며, 점멸하는 솔리드 사각형으로 표시되는 한국어/일부 중국어 조합의 표준 UI. nonzero 면 `ase=TF_AE_NONE` 이고 캐럿 없음(하이라이트가 캐럿 대체), 단일 selection 일 때만 nonzero 가능.
2. **GCS_* 매핑(official)**: `GCS_COMPSTR`="current composition string"(=조합 중), `GCS_RESULTSTR`="composition result"(=확정). 모든 GCS_ 비트 0 ⇒ "composition has been canceled" → 앱은 조합 문자열 삭제. 임시문자(CS_INSERTCHAR+CS_NOMOVECARET)는 "후속 GCS_RESULTSTR 메시지로 교체"된다.
3. **CUAS always-on(official 로 강화)**: `ImmDisableTextFrameService` 는 "Windows Vista 이후 더 이상 사용 불가, 대신 `ImmDisableIME` 사용". → Vista+ disable-불가 방향을 1차 권위로 확인. EnableCicero/CTF SFM 레지스트리 토글은 **MS 1차 문서에 실재 없음** → CUAS 를 레지스트리 키로 끄는 설계는 신뢰 불가.
4. **한국어 IME 관습(OSS empirical)**: NavilIME(EditSession.cpp:132-133), saenaru(compose.cpp:226-227, keys.cpp:483-484) 둘 다 조합 중 `ase=TF_AE_NONE + fInterimChar=TRUE`, 커밋/종료 시 `FALSE`. UNIM composition.rs:157(BOOL(1)) / :128(BOOL(0)) 와 정확히 일치.

**검증으로 깎인 부분(채택 안 함 / 약화)**:
- ❌ "fInterimChar=TRUE = GCS_COMPSTR 유지의 **유일한** 핵심 트리거" 는 **과장**. MS 공식 SampleIME(Composition.cpp:238-243)는 `fInterimChar=FALSE` + zero-width `Collapse(TF_ANCHOR_END)` 로도 정상 조합 → fInterimChar 는 한국어 IME **관습**이지 보편적 필수 신호 아님. UNIM 의 keep-alive 는 GUID_PROP_ATTRIBUTE + GUID_PROP_READING 속성에도 의존.
- ⚠ MS spec 은 fInterimChar 를 "정확히 한 글자"로 한정하나 UNIM/kolemak 은 다음절 composition range 전체에 BOOL(1) 적용 → **out-of-spec**. GUI 앱(메모장/Word) 선택 음영·역입력 회귀 가능성 미검증.
- ⚠ 경쟁 가설(프로젝트 KB): wezterm 즉시-terminate 의 진짜 원인은 ITextStoreACP 부재(CUAS 가 쓸 문서 없음)일 수 있음. text_service.rs:510 주석도 composition 이 "대부분 성공"이라 인정 → fInterimChar 가 종료를 **완전 제거하진 못함**.

**소스**:
- TF_SELECTIONSTYLE — https://learn.microsoft.com/en-us/windows/win32/api/msctf/ns-msctf-tf_selectionstyle (official)
- WM_IME_COMPOSITION — https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition (official)
- IME Composition String Values — https://learn.microsoft.com/en-us/windows/win32/intl/ime-composition-string-values (official, GCS_*)
- ImmDisableTextFrameService(Vista+ deprecation) — https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immdisabletextframeservice (official)
- ITfComposition::EndComposition(GUID_PROP_COMPOSING 제거) — https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcomposition-endcomposition (official)
- NavilIME EditSession.cpp:132-133 — https://github.com/navilera/NavilIME/blob/master/NavilIME/EditSession.cpp (oss)
- saenaru compose.cpp:226-227 / keys.cpp:483-484 — https://github.com/wkpark/saenaru (oss)
- 반례: MS SampleIME Composition.cpp:238-243(fInterimChar=FALSE) — https://github.com/microsoft/Windows-classic-samples (official)
- CUAS Vista 보조(reverse-engineering) — https://nyaruru.hatenablog.com/entry/20070308/p1 (community)

---

## G2 — 순수 IMM32 네이티브 앱(카톡/한컴)에 TIP 도달 (P0, 확정채택)

**질문**: TSF 프로파일만 활성일 때 IMM32-only 로 키를 소비하는 32비트 앱(OnKeyDown 0회)에 한글을 넣는 정석? (a) 같은 언어바 항목 듀얼 등록(CTF\Assemblies+Substitutes) 동작? (b) Mozc 처리/포기? (c) WM_IME_REQUEST IMR_DOCUMENTFEED/IMR_RECONVERTSTRING 동작 앱 범위?

**확정 결론(검증 통과분)**:
- **(a) 한 바이너리 자동 듀얼 등록의 정석은 1차소스에 없음.** TSF 등록(`ITfInputProcessorProfiles::AddLanguageProfile` + `ITfCategoryMgr::RegisterCategory`, HKLM\...\CTF\TIP)과 IMM32 .ime 등록(`ImmInstallIME`)은 완전 분리 경로. SampleIME Register.cpp = TSF 카테고리만(IMM32/Assemblies 0건), Mozc tsf_registrar.cc 동일.
- **(b) Mozc 는 IMM32 입력 모듈을 폐기**("Deletion of code for IMM32 — support limited to Windows 10 or later"). 순수 TSF TIP + OS CUAS 브리지 의존. 단 read-side `imm_reconvert_string.h`(RequestType{kReconvertString, kDocumentFeed})는 HEAD 잔존 → RECONVERTSTRING 자료구조를 TSF 재변환/주변텍스트에 재활용.
- **(c) WM_IME_REQUEST 는 OS/IME → 앱 WindowProc 로 가는 IMM32-side 메시지.** TSF TIP 이 직접 송수신 못 함. 구현 앱은 좁음(Emacs/WinMerge opt-in; Scintilla 미구현, Flutter 초기 미지원). 카톡/한컴 구현 보장 없음.

**검증으로 깎인/정정된 부분**:
- ⚠ **"CTF\Assemblies 는 IMM32 와 무관한 활성화 캐시일 뿐" 은 부정확**. `HKCU\...\CTF\Assemblies\<langid>\{TIP_KEYBOARD GUID}\Default` 는 TIP-CLSID+Profile+KeyboardLayout(HKL) 바인딩이며, `Keyboard Layout\Substitutes` 와 함께 **TSF TIP 과 IMM32 HKL 을 하나의 언어바 항목으로 통합**하는 실제 메커니즘(MS 한국어 IME 자체 패턴). 단 이는 UI 항목 통합일 뿐 — **별도의 동작하는 .ime 바이너리가 추가로 필요**하고 한 바이너리가 둘을 자동 제공하진 못함. (UNIM 의 듀얼모드 wxs 는 아직 계획/미검증.)
- ⚠ **"Mozc 가 IMM32 앱을 포기" 는 두 층위 혼동**. Mozc 는 "IMM32 IME 이기를" 포기했지 "IMM32 앱을 서빙하기를" 포기 안 함 — 순수 TSF TIP 이 CUAS 로 IMM32-only GUI 앱(wezterm/메모장)을 실제 서빙.
- ⚠ **(c) 의 진짜 실패 지점**: 카톡/한컴 OnKeyDown 0회는 DOCUMENTFEED/주변텍스트 문제가 **아니라 키 라우팅 문제**. 이 앱들이 IMM32 를 후킹해 시스템 IME 를 우회 → 키가 msctf `ITfKeystrokeMgr` 에 안 닿아 TIP KeyEventSink 미발화. (UNIM 자체 진단: 48 PID 중 37개가 sink 무장하고도 OnKeyDown 0회 — imm32-diagnosis-report.md.)
- ➜ **순수 미수정 결론**: 별도 IMM32 .ime 를 "데스크톱 호환 IME"로 병행 설치하는 길은 기술적 가능하나 MS 비권장·UWP 미커버. 64비트 OS 에서 32+64 둘 다 동일 파일명으로 설치 필요.

**소스**:
- TSF text-service-registration — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/text-service-registration.md (official)
- SampleIME Register.cpp — https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/IME/cpp/SampleIME/Register.cpp (official)
- Mozc tsf_registrar.cc — https://github.com/google/mozc/blob/master/src/win32/base/tsf_registrar.cc (oss)
- Mozc IMM32 삭제 changelog — https://zenn.dev/komatsuh/articles/9b88d84c0590f6?locale=en (oss)
- Mozc imm_reconvert_string.h — https://github.com/google/mozc/blob/master/src/win32/base/imm_reconvert_string.h (oss)
- WM_IME_REQUEST — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/Intl/wm-ime-request.md (official)
- IMR_DOCUMENTFEED — https://learn.microsoft.com/en-us/windows/win32/intl/imr-documentfeed (official)
- 64-bit considerations — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/64-bit-platform-considerations.md (official)
- w8cookbook third-party IME — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/w8cookbook/third-party-input-method-editors.md (official)
- Wine imm32 imm.c(ImmRequestMessage → SendMessage app) — https://github.com/wine-mirror/wine/blob/master/dlls/imm32/imm.c (oss)

---

## G3 — Win11 SHOWNINTRAY 노출 조건 + compatible IME 체크리스트 (P1, 확정채택)

**질문**: Win11 이 SHOWNINTRAY ITfLangBarItem 을 새 트레이/Input Indicator 에 그리는 추가 조건이 MS 1차 문서에 있나? 24H2/25H2 anchor 회귀 공식 인지/수정 빌드? compatible IME 체크리스트?

**확정 결론(confirmed)**:
- **SHOWNINTRAY(0x2)·SHOWNINTRAYONLY(0x8) 둘 다 "This flag is not currently supported"(verbatim).** third-party TIP 이 ITfLangBarItem 을 직접 트레이에 그리는 공식 경로 없음. 트레이는 OS Input Indicator 가 호환 IME 에 한해 브랜딩 아이콘 1 + 모드 아이콘 1 을 그림. 비호환이면 언어 약어(KO) 폴백 = 트레이 글리프 미표시 조건.
- **compatible IME 체크리스트(verbatim)**:
  1. TSF 구현 필수(IMM32 차단).
  2. `GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT` 를 `ITfCategoryMgr::RegisterCategory` 로 등록.
  3. `ITfThreadMgrEx::GetActiveFlags` 로 `TF_TMF_IMMERSIVEMODE` 분기.
  4. 아이콘은 DLL/EXE 리소스에 임베드(단독 .ico 금지), 모드 아이콘은 `GUID_LBI_INPUTMODE`, 16x16(DPI 데스크톱)+20x20(PPI UAC).
  5. app-container 인지, 사전/설정은 Program Files/Windows 하위 또는 app-container SID ACL.
  6. 터치 후보 페이지 키 Next `0xF003` / Prev `0xF004` 를 VK_PACKET 으로 처리.
- **24H2/25H2 anchor 회귀**: 언어 스위처가 좌하단으로 튀는 보고가 MS Q&A 커뮤니티 스레드에 존재(24H2 간헐, 25H2 상시)하나 **공식 인지(release-health resolved-issues)나 수정 빌드 KB 는 1차 문서에 없음**.
- **rime/weasel#1682**: third-party 작업표시줄 langbar indicator 가 영구 소멸(반면 OS Input Indicator/caret/앱 아이콘은 생존) 실증. (단 reporter OS 문자열 19045 는 Win10 빌드라 Win11-신트레이 특정 증거로는 약함.)

**UNIM 코드 현실 정정**: WiX `unim.wxs` 가 IMMERSIVESUPPORT(13A016DF), UIELEMENTENABLED(49D2F9CF), SYSTRAYSUPPORT(25504FB4), TIP_KEYBOARD, DISPLAYATTRIBUTEPROVIDER 카테고리를 **이미 등록**(unim.wxs:166/134/174/118/126). lang_bar.rs GetIcon 도 유효 HICON 이면 S_OK / NULL 이면 E_FAIL(L562-574, SampleIME 규약 준수)로 **이미 수정됨**. 잔여는 아이콘 리소스 임베드 + VM 표시 검증.

**소스**:
- TF_LBI_STYLE_ Constants(SHOWNINTRAY not supported) — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/tf-lbi-style--constants.md (official)
- IME requirements(체크리스트) — https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements (official)
- TSF language-bar(AddItem 생명주기) — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/language-bar.md (official)
- rime/weasel#1682 — https://github.com/rime/weasel/issues/1682 (oss)
- MS Q&A 25H2 anchor — https://learn.microsoft.com/en-au/answers/questions/5779777/ (community)

---

## G4 — CTF\LangBar ShowStatus 값·정책 (P1, 확정채택 코어)

**질문**: `HKCU\Software\Microsoft\CTF\LangBar\ShowStatus`(DWORD) 0/3/4 의미, 그룹정책 경로, Win11 23H2/24H2/25H2 third-party langbar 노출 차이?

**확정 결론(코어, partly 검증)**:
- **ShowStatus(DWORD): 0=Floating on desktop, 3=Hidden, 4=Docked in the taskbar.** 두 독립 소스(renenyffenegger.ch verbatim + ExplorerPatcher #390 사용자 재부팅 실측)로 확정. 커뮤니티 가이드의 3/4 뒤바꿈은 오류.
- 주의: 레거시 XP/Office 언어바 관습(0=hidden/1=floating/2=docked)과 혼동 금지 — Win10/11 CTF LangBar 는 0/3/4.
- **그룹정책**: ShowStatus 를 직접 설정하는 네이티브 ADMX 없음 = per-user preference, 기업 배포는 Group Policy Preferences(레지스트리 항목)로만.
- **Win11 버전 차이**: 23H2/24H2/25H2 third-party langbar 노출의 버전별 하드 분기를 1차소스로 확인 못 함 → 최근 빌드 전반에 걸쳐 일관(트레이 Input Indicator 가 클래식 langbar 대체, Docked 옵션 기본 회색). "버전별 분기 없음"이 방어 가능한 결론.

**검증으로 깎인 부분**:
- ⚠ "Turn off Advanced Text Services GPO 가 `HKCU\...\CTF\Disable Thread Input Manager=1` 을 쓴다" 는 **MS 1차 미확인**(인용한 policy-csp-admx-globalization 페이지엔 그 문구 없음; admx.help 522). → **커뮤니티/추론으로 다운그레이드**. RestrictUILangSelect 만 그 페이지로 확정됨.

**소스**:
- renenyffenegger CTF/LangBar — https://renenyffenegger.ch/notes/Windows/registry/tree/HKEY_CURRENT_USER/Software/Microsoft/CTF/LangBar/index (community, verbatim)
- ExplorerPatcher #390(재부팅 실측) — https://github.com/valinet/ExplorerPatcher/discussions/390 (oss)
- MS Q&A 4301337(docked greyed) — https://learn.microsoft.com/en-us/answers/questions/4301337/ (official answers)
- policy-csp-admx-globalization(RestrictUILangSelect 만 뒷받침) — https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-globalization (official)

---

## G5 — 한자 후보 GCS_COMPATTR / ATTR_TARGET_CONVERTED (P2, 확정채택 코어)

**질문**: 한국어 한자 후보 선택 중 ATTR_TARGET_CONVERTED(GCS_COMPATTR) 보장? 일본어식 in-composition target-clause 하이라이트 모델이 한국어에 성립?

**확정 결론(코어, partly 검증)**:
- **아니다 — 보장 안 됨.** 두 층위 분리: (1) API 계약상 ATTR_* 값은 언어 무관이라 만들 수 *있음*. (2) 실측상 MS 한국어 IME 한자 변환은 일본어식 'composition 내부 다중 절+타깃 절 하이라이트'가 아니라 **즉시 별도 candidate box 를 띄우는 reconversion 모델**. Scintilla #2392("There is no composition string or target") + Mozilla #1213589(확정 텍스트를 캐럿~줄바꿈 범위로 역변환)가 교차 입증.
- ATTR_* 값(per imm.h, MS Learn 은 의미만): ATTR_INPUT=0, ATTR_TARGET_CONVERTED=1, ATTR_CONVERTED=2, ATTR_TARGET_NOTCONVERTED=3, ATTR_INPUT_ERROR=4, ATTR_FIXEDCONVERTED=5.
- GCS 비트 전부 0 = 조합 취소 → preedit 삭제 처리 필수(Linux preedit-end 교훈과 동형).

**검증으로 정정된 부분(인용 오류, 결론은 유지)**:
- ATTR_* 표와 한국어 노트는 cited URL(intl/ime-composition-string-values)에 **없음**. 실제 위치는 `intl/composition-string` + `nf-imm-immsetcompositionstringw`. 한국어 노트 실제 문구는 "In Korean, this attribute represents a Hangul character that the IME has not yet converted"(≠ "undetermined string" 오인용). 숫자값은 imm.h 출처(MS Learn 아님).

**소스**:
- 정정된 ATTR_* 위치 — https://learn.microsoft.com/en-us/windows/win32/intl/composition-string (official)
- ImmSetCompositionString(target clause 규칙) — https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-immsetcompositionstringw (official)
- WM_IME_COMPOSITION(GCS 비트 0=취소) — https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition (official)
- Scintilla #2392 — https://sourceforge.net/p/scintilla/bugs/2392/ (community)
- Mozilla #1213589 — https://bugzilla.mozilla.org/show_bug.cgi?id=1213589 (community)

---

## G6 — .ime 표시명 리소스(STRINGTABLE) + 코드서명 범위 (P2, 확정채택)

**질문**: `Layout Display Name=@...,-1` 이 참조하는 string resource 를 .ime PE 에 STRINGTABLE 로 넣는 정석? Win10/11 에서 unsigned .ime/.dll/.msi 가 거부되는 정확한 환경?

**확정 결론(confirmed)**:
- **STRINGTABLE 정석**: .rc 에 `STRINGTABLE BEGIN 1 L"Korean Input Method (UNIM)" END` 를 넣고 rc.exe(MSVC)/windres(mingw)로 컴파일해 PE 에 임베드. `Layout Display Name`(REG_EXPAND_SZ) = `@%SystemRoot%\system32\unim_imm32.ime,-1`. `@파일,-id` 에서 **음수 -id = 리소스 ID**(0 이상은 인덱스). OS 가 `SHLoadIndirectString` 으로 해석. `Layout Text`(REG_SZ 평문)는 폴백.
- **TSF 평행물**: `SetLanguageProfileDisplayName(dllPath, resId)` → `CTF\TIP\<CLSID>\LanguageProfile\Display Description`. **`;v2` 미지원이라 표시명 변경 시 새 리소스 ID 발급 필수**(mozc b/2994558). 단 TSF 는 양수 인덱스를 기대(mozc 실증)하여 IMM32 음수 규약과 인자 의미가 다를 수 있음 → 리소스 재사용 시 VM 검증.
- **서명/거부 환경**:
  - 데스크톱 일반 프로세스: unsigned .ime/.dll 은 **하드 차단 아님** — 설치 시 critical warning, Defender 가 악성 판정 시 제거.
  - **거부 환경 = AppContainer(UWP/Store, IMM32 차단·TIP 만 로드) + PPL/Code-Integrity 강제 프로세스(`/INTEGRITYCHECK` 요구 signing level) + 기업 WDAC/AppLocker + 일부 AV**.
- **64비트**: 32+64 텍스트서비스 DLL 둘 다 동일 파일명으로 설치 필수. `%WINDIR%\IME`(시스템 예약, WOW64 미적용)에 절대 설치 금지.

**UNIM 코드 현실 정정**:
- unim-tsf 는 `embed-resource="2"` + `unim.rc`(ICON id 1)로 **아이콘은 임베드하나 STRINGTABLE 없음**.
- **unim-imm32 의 register.rs(L90-95)는 STRINGTABLE 부재 때문에 `Layout Display Name` 을 평문 REG_SZ 로 우회 기록 중** — 과거 `@...,-1000` 인디렉트가 빈 문자열 반환했던 결함의 워크어라운드. 이것이 G6 의 핵심 actionable(STRINGTABLE 임베드 시 `,-1` 정석 복귀 가능).

**소스**:
- STRINGTABLE resource — https://learn.microsoft.com/en-us/windows/win32/menurc/stringtable-resource (official)
- SHLoadIndirectString(음수=resource ID) — https://learn.microsoft.com/en-us/windows/win32/api/shlwapi/nf-shlwapi-shloadindirectstring (official)
- /INTEGRITYCHECK — https://learn.microsoft.com/en-us/cpp/build/reference/integritycheck-require-signature-check (official)
- IME requirements(AppContainer/IMM32 차단) — https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements (official)
- Mozc tsf_registrar.cc(;v2 미지원 b/2994558) — https://github.com/google/mozc/blob/master/src/win32/base/tsf_registrar.cc (oss)
- 64-bit considerations(%WINDIR%\IME 금지) — https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/64-bit-platform-considerations.md (official)
