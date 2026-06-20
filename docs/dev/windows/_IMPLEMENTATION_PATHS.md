# UNIM Windows — 구현 경로 매트릭스 + 로드맵 + VM 잔여 (G1~G6 확정 후)

> 작성일 2026-06-19 · 입력: `_RESEARCH_FINDINGS.md`(G1~G6 확정) + `_KNOWLEDGE_STATE.md` 앱×기능 매트릭스 + 라이브 코드(unim-tsf/src, unim-imm32/src, installer/wix).
> 목표: "모든 앱·경우를 커버하는 TSF+IMM32 한글 입력기(기본 + 한자/특수문자/이모지 3팝업 + 자동교정)".

---

## §2. 앱 유형 × 기능 매트릭스 — 확정 구현 경로

범례: ✅=경로 확정·코드 가능 / ✅(VM)=경로 확정, VM 실측만 남음 / ⚠=조건부·폴백 의존 / ❌=구조적 미지원(우회만).
팝업 3종(한자/특수문자/이모지)은 **별도 `unim-popup-win.exe` 싱글턴 렌더러**가 화면 중앙에 자체 표시하므로 호스트 앱의 TSF/IMM32 능력과 대체로 **직교**(아래 ★ 참조).

| 앱 유형 | 기본입력 | 3팝업(한자/특문/이모지) | 자동교정(AutoTypeFix) |
|---|---|---|---|
| **① TSF-aware**<br>(브라우저·리치컨트롤·Win11 메모장, 앱 ITextStoreACP) | ✅ 네이티브 inline(공짜 경로). ITfComposition/SetText 직행. | ✅ 별도 렌더러 중앙 표시 + 선택결과 GCS_RESULTSTR/SetText 확정. | ✅ ITfRange Clone/ShiftStart 로 surrounding-text TSF-native 읽기. |
| **② IMM32/CUAS inline**<br>(EDIT·wezterm·Telegram, CUAS emulated store) | ✅(VM) **G1 확정**: 조합 중 `fInterimChar=TRUE`(composition.rs:157) 유지가 CUAS GCS_COMPSTR keep-alive(한국어 IME 관습). 확정/종료 시 `FALSE`(:128). + 단일세션 + nav 통과. **단 유일 트리거 아님** → text_service.rs:598 즉시-terminate 폴백 안전망 **유지 필수**. | ✅ 렌더러 직교. 확정은 SetText→GCS_RESULTSTR 매핑. | ✅(VM) TSF-native 읽기 1순위. 실패 시 N+1 BS 폴백(Linux XIM 교훈 동형). |
| **③ 진짜 콘솔**<br>(conhost cmd/PowerShell 고전창) | ⚠(VM) CUAS 페이지가 콘솔 제외. conhost 콘솔 IME 별도 경로. composition 안 되면 즉시-terminate→**오버레이 폴백**(preedit_window). | ✅ 렌더러 직교(중앙 표시). | ⚠ surrounding-text 채널 부재 → best-effort. |
| **④ 게임 IMM32(미렌더)**<br>(PoE 등 게임 채팅) | ⚠ IME 가 기본 조합창을 좌상단에 그림. UNIM preedit_window 오버레이(좌상단 폴백 권장). | ✅ 렌더러 직교. | ⚠ best-effort(키 라우팅 닿는 한). |
| **⑤ 순수 IMM32 네이티브**<br>(카톡/한컴 32비트, OnKeyDown 0회) | ❌→⚠ **G2 확정**: 근본 원인은 키-라우팅(앱이 IMM32 후킹, 키가 msctf ITfKeystrokeMgr 에 안 닿음). TSF-only+CUAS 로는 불가. **우회 2갈래**: (A) 32/64 TIP DLL 양쪽 설치 확인(미설치면 OnKeyDown 0회의 단순 원인) → 여전히 0회면 키-라우팅 차단 확정. (B) 별도 IMM32 .ime 병행 설치 + Substitutes/Assemblies 로 단일 언어바 항목 통합(비권장·UWP 미커버·미검증). | ✅(VM) 렌더러 직교 — **키만 닿으면** 팝업 트리거/표시는 동작. 키 안 닿으면 팝업도 못 뜸. | ❌ 키 라우팅 차단 시 불가. |
| **⑤' raw/IME-off**(전체화면 게임) | ❌ 비대상(한글 자체 불가). | n/a | n/a |

★ **3팝업 직교성의 정확한 의미**: 팝업 *표시*는 별도 프로세스라 호스트와 무관하나, **팝업을 띄우는 트리거(한자키/특문키)는 OnKeyDown 을 거친다**. 따라서 ⑤처럼 키가 TIP 에 안 닿는 앱은 팝업도 못 뜬다(=기본입력과 같은 게이트). ②~④는 키가 닿으므로 팝업 OK. 확정 결과를 앱에 쓰는 경로만 기본입력과 동일 분기.

### ❌·⚠ 칸이 이번 조사로 어떻게 바뀌나

- **②기본입력 ⚠→✅(VM)**: G1 이 fInterimChar 비대칭을 한국어 IME 관습으로 1차+OSS 확정. UNIM 코드(composition.rs:128/157)가 이미 정답 경로. **단 G1 검증에서 "유일 트리거" 가 깎임** → 폴백(text_service.rs:598 by_time||known_cuas)이 dispensable 아님이 재확인됨. 즉 ✅(VM)이되 폴백 제거 금지.
- **⑤기본입력 ❌→⚠(우회)**: G2 가 "한 바이너리 자동 듀얼"의 정석 부재를 확정하되, 실패 지점을 **키 라우팅**으로 정밀화. 단순 원인(32비트 TIP 미설치)부터 배제 후, 진짜 후킹 차단이면 (B) 별도 .ime 만이 키를 전달 가능. **여전히 VM 실측이 유일한 분기 결정자**(O4).
- **트레이 표시(기본입력 외 UX)**: G3 이 SHOWNINTRAY 직접 그리기를 폐기 확정, OS Input Indicator 경로 전용으로. WiX 가 이미 IMMERSIVE/UIELEMENT/SYSTRAY 카테고리 등록 + GetIcon E_FAIL 수정 완료 → 잔여는 아이콘 리소스 임베드 + VM 표시 검증뿐.

---

## §3. 구현 우선순위 로드맵 (P0 → P2)

> 단위: 코드/레지스트리/WiX 변경. 각 항목 [근거 갭ID · 소스URL].

### P0
1. **②/⑤ 회귀 방지 가드 — fInterimChar 비대칭 고정 + 폴백 보존** [G1 · composition.rs:128/157, text_service.rs:598]
   - composition.rs:157 BOOL(1) 라인에 "CUAS GCS_COMPSTR keep-alive (G1, 한국어 IME 관습; 단독 트리거 아님)" 의도 주석/단위테스트 고정. 확정/종료 BOOL(0) 비대칭 절대 유지.
   - text_service.rs:598 즉시-terminate 폴백을 **제거 금지** 마킹(G1 검증: fInterimChar 단독으로 종료 완전 제거 못 함).
   - 소스: https://learn.microsoft.com/en-us/windows/win32/api/msctf/ns-msctf-tf_selectionstyle , https://github.com/microsoft/Windows-classic-samples (SampleIME 반례)
2. **⑤ 키 라우팅 단순원인 배제 — 32/64 TIP DLL 양쪽 설치 검증** [G2 · 64-bit-considerations]
   - MSI/빌드가 unim_tsf32.dll + unim_tsf64.dll 둘 다 동일 파일명으로 설치하는지 WiX/CI 점검. 한쪽만이면 32비트 카톡에서 TIP 미로드 → OnKeyDown 0회의 단순 원인. (VM 실측 O4 전제 조건.)
   - 소스: https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/64-bit-platform-considerations.md
3. **듀얼 등록(자동 1바이너리) 설계 금지 못박기** [G2 · text-service-registration, SampleIME Register.cpp]
   - 한 바이너리를 IMM32+TSF 로 자동 듀얼화하는 경로는 1차소스 0건. register.rs/wxs 는 TSF 단일 경로 유지. CTF\Assemblies 는 **별도 .ime + 단일 UI 항목 통합용**으로만(듀얼모드 wxs, 미검증) 한정 — 출하 보류.
   - 소스: https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/text-service-registration.md

### P1
4. **트레이/Input Indicator compatible 체크리스트 충족 검증** [G3 · IME requirements, WiX unim.wxs]
   - WiX 가 이미 IMMERSIVESUPPORT/UIELEMENTENABLED/SYSTRAYSUPPORT/TIP_KEYBOARD/DISPLAYATTRIBUTEPROVIDER 등록(unim.wxs:166/134/174/118/126) — 확인 완료. 잔여: 모드 아이콘 GUID_LBI_INPUTMODE 16x16+20x20 임베드, TF_TMF_IMMERSIVEMODE 분기 점검.
   - SHOWNINTRAY 직접 그리기 코드 금지(이미 없음). 트레이는 OS Input Indicator 경로 전용.
   - 소스: https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements , https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/TSF/tf-lbi-style--constants.md
5. **ShowStatus 상수 하드코딩 + 사용자 안내 문구** [G4 · renenyffenegger, ExplorerPatcher#390]
   - UNIM 이 ShowStatus 를 읽거나 쓰면 0=floating/3=hidden/4=docked 를 두 소스 인용 주석과 함께 하드코딩(3/4 재스왑 방지). 1·2 는 무효 취급.
   - 레거시 langbar 강제 켜기 안내는 "Show input indicator on taskbar" 토글 + "Use the desktop language bar when available" 체크박스 안내로.
   - GPO `Disable Thread Input Manager=1` 은 **커뮤니티/추론**이므로 안내문에 단정 금지.
   - 소스: https://github.com/valinet/ExplorerPatcher/discussions/390 , https://renenyffenegger.ch/notes/Windows/registry/tree/HKEY_CURRENT_USER/Software/Microsoft/CTF/LangBar/index

### P2
6. **unim-imm32 .ime STRINGTABLE 임베드 → Layout Display Name 정석 복귀** [G6 · STRINGTABLE, SHLoadIndirectString]
   - unim-imm32 에 `.rc`(`STRINGTABLE BEGIN 1 L"Korean Input Method (UNIM)" END`) 추가 + `embed-resource` 빌드 단계(현재 build.rs 있으나 STRINGTABLE 없음). 리소스 ID **반드시 1**(register.rs 가 `,-1` 참조하도록 전환). 평문 REG_SZ 우회(register.rs:90-95) 제거.
   - mingw=windres, GHA MSVC=rc.exe/embed-resource.
   - 소스: https://learn.microsoft.com/en-us/windows/win32/menurc/stringtable-resource , https://learn.microsoft.com/en-us/windows/win32/api/shlwapi/nf-shlwapi-shloadindirectstring
7. **TSF 표시명 SetLanguageProfileDisplayName 시 새 리소스 ID 규칙 주석** [G6 · mozc b/2994558]
   - `;v2` 미지원 → 표시명 변경 릴리스에서 ID 재사용 금지·새 ID 발급(주석 명시). IMM32 음수 ID vs TSF 양수 인덱스 인자 의미 차이 주의(VM 검증 항목 O8).
   - 소스: https://github.com/google/mozc/blob/master/src/win32/base/tsf_registrar.cc
8. **한자 후보 attribute 가정 제거** [G5 · composition-string, Scintilla#2392]
   - 한자 후보 단계에서 GCS_COMPATTR 에 ATTR_TARGET_CONVERTED 가 반드시 온다고 가정 금지. "composition 비어 있음(GCS 비트 0)=취소→preedit 삭제"를 정상 경로로 처리. 일본어식 다중-clause target 하이라이트는 일본어 전용 분기로 한정, 한국어는 별도 후보창(렌더러) 모델.
   - 소스: https://learn.microsoft.com/en-us/windows/win32/intl/composition-string , https://sourceforge.net/p/scintilla/bugs/2392/
9. **코드서명(signtool) 단계 추가** [G6 · /INTEGRITYCHECK, IME requirements]
   - 배포 MSI/exe/.ime/.dll Authenticode 서명(windows-msi.yml signtool). VM sanity 는 미서명 가능. AppContainer/PPL/WDAC/AV 환경 거부 회피. `%WINDIR%\IME` 설치 금지 재확인.
   - 소스: https://learn.microsoft.com/en-us/cpp/build/reference/integritycheck-require-signature-check

---

## §4. VM 실측으로만 풀리는 잔여 (웹조사 종결 — 외부 1차소스로 더 못 풂)

> 웹조사(G1~G6)는 여기까지. 아래는 닫힌 소스(msctf.dll)·앱별 동작·런타임 환경이라 VM Spy++/procmon/디버거 실측만이 확정 경로. `_KNOWLEDGE_STATE.md` O1~O12 와 연결.

- [ ] **O1 / G1잔여** — wezterm 등 CUAS-unaware 호스트에서 msctf.dll 이 selection 변화를 어느 임계로 `OnCompositionTerminated:IMMEDIATE` 발사하는지 바이트단위 규칙. **VM Spy++/IME 로깅으로 MS IME WM_IME_*+GCS_* 시퀀스 캡처 → UNIM 시퀀스 diff** (repro-matrix-p1.md 절차). fInterimChar=TRUE 가 다음절/GUI 앱(메모장/Word) 선택음영·역입력 회귀를 일으키는지 GUI+콘솔 양쪽 실측.
- [ ] **O4 / G2잔여** — 카톡/한컴이 TSF-only 일 때 정말 키를 TIP 에 안 보내는가: (1) 32비트 TIP DLL 미설치/미로드(단순 원인)부터 procmon/디버거로 배제 → (2) 진짜 IMM32 후킹 차단인지 확정. 확정 시 별도 IMM32 .ime(B안)만이 키 전달 가능한지 실측. 카톡/한컴 컨트롤이 IMR_DOCUMENTFEED/IMR_RECONVERTSTRING 구현하는지 WM_IME_REQUEST 후킹(추정: 미구현).
- [ ] **O2 / G5잔여** — 한자 후보를 띄우는 "바로 그 순간" GCS_COMPATTR 실제 덤프(ATTR_INPUT vs ATTR_TARGET_CONVERTED vs composition 부재). 팝업 렌더(중앙 표시)·column-major 매핑 VM 실측.
- [ ] **O5 / G3잔여** — 24H2/25H2 좌하단 anchor 회귀의 OS 인지/수정 빌드는 1차 미확정 → VM 에서 재현/회복 관찰. GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT 미등록 시 신규 트레이 폴백 렌더 + UNIM 등록 상태 실측. floating/모드 아이콘 글리프 실제 표시. weasel#1682식 트레이 영구 소멸 재현/회복.
- [ ] **O—/ G4잔여** — `ShowStatus=4`(docked) 가 ko-KR 단독 설치 시 선택 가능한지, "Turn off Advanced Text Services" GPO 가 실제로 무엇을 쓰는지(`Disable Thread Input Manager=1` 추론 확인), UNIM TIP 항목이 ShowStatus 무관하게 Input Indicator 플라이아웃에 뜨는지. VM reg add 병행.
- [ ] **O8 / G6잔여** — STRINGTABLE id 1 임베드 후 `,-1` 표시명 실제 렌더, IMM32 음수 ID vs TSF 양수 인덱스 인자 의미 차이 검증. unsigned .ime 데스크톱 등록 경고 시점·문구(Win8 cookbook 수준 → 실측), AV/기업 WDAC 미서명 .ime 차단 강도.
- [ ] **O3 — ③ 진짜 콘솔(conhost)** — conhost 콘솔 IME 경로에서 composition/오버레이 폴백 실동작(별도 트랙, CUAS 콘솔 제외).
- [ ] **O11 — Phase 2 마우스 역채널·LL훅 수명** — owner_hwnd+seq 불일치 역이벤트 무시, 표시 중에만 LL훅 설치·hide 시 해제 규칙 실측(훅 누수 시 전역 마우스 지연).
- [ ] **O12 — H6/H7** — 엔진 reset 시 팝업 미정리·팝업 중 비팝업 키 desync (엔진·key_handler 과제, 프로세스 분리와 직교).
