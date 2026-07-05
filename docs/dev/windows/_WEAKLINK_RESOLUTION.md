> ## ✅ ⑤(카톡/IMM32) 판정 정정 (2026-06-22 SOLVED)
> 이 문서는 ⑤ 카톡/한컴을 "likely — 별도 `.ime` B안이 키를 전달한다"로 판정했으나 **틀렸다**.
> 카톡은 TSF를 정상 로드하며, 미동작의 진짜 원인은 **`unim_tsf.dll` x64 단독(32-bit COM 등록 부재)**.
> i686 `unim_tsf.dll`을 32-bit TSF로 등록하니 카톡 한글 입력이 됐다(실증). **`.ime` B안은 불필요 — 폐기.**
> 최종 진실: **[imm32-win11-SOLUTION.md](imm32-win11-SOLUTION.md)**. (③ 콘솔 판정은 별개로 유효.)

# UNIM Windows — 약한 고리 ⑤(순수 IMM32)·③(콘솔) 최종 판정

> 작성일 2026-06-19 · 입력: 한국어 IME 생태계 적대적 프로브(날개셋·새나루·나빌·weasel·mozc) +
> conhost/Windows Terminal 1차 이슈/PR + UNIM 라이브 코드 감사(feat/windows-msi-redesign, v0.3.18).
> 목적: `_IMPLEMENTATION_PATHS.md` 의 ⑤③ 칸을 외부 실증 + 코드감사로 갱신/확정.

---

## 1. ⑤ 카톡/한컴 (순수 IMM32 네이티브) — **판정: likely (별도 .ime B안이 키를 전달한다)**

### 핵심 질문
"TSF-only+CUAS 로 키가 안 닿는 앱에서, **별도 IMM32 `.ime`(B안)가 키 라우팅 차단을 우회 가능한가**?"

### 한국어 IME 생태계 실증 (B안이 표준임을 입증)
- **새나루(saenaru)**: `src/saenaru.def` 가 IMM32 IME DDI 전체(`ImeInquire`/`ImeProcessKey`/`ImeToAsciiEx`/
  `ImeSelect`/`ImeSetActiveContext`/`NotifyIME`/`ImeSetCompositionString` …)를 export하고, `imm.c`·`ui*.c`·
  `regime.c` 와 `tsf.cpp` 를 **같은 프로젝트에서 동시 빌드**. → "같은 한글 IME가 IMM32(.ime)와 TSF를 둘 다
  구현"하는 정석 듀얼 구조. 소스: https://github.com/wkpark/saenaru/blob/master/src/saenaru.def
- **weasel(小狼毫)**: 커밋 **#257 "feat(install): combine IME and TSF"** 로 레거시 `.ime`(IMM32)와 `.dll`(TSF)을
  **아키텍처별 쌍**(weasel.dll+weasel.ime / weaselx64.* / weaselARM64.*)으로 함께 설치. 듀얼 등록은
  `WeaselSetup/imesetup.cpp` 가 HKLM Keyboard Layouts(E02X0804)에 `Ime File`/`Layout File`+HKCU Preload 기록.
  소스: https://github.com/rime/weasel/commit/91cbd2c , https://deepwiki.com/rime/weasel/7.2-setup-and-registration
- **mozc**: IMM32+TSF 양쪽 등록 가능(단 Win8+ IMM32 비권장=UWP 차단, 데스크톱은 로드됨). TIP은
  forwarder DLL(mozc_tip64x, build_tip_forwarder_dll.py)로 WoW64 32비트 클라이언트 로드 해결.
  소스: https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements
- **날개셋**: '외부 모듈'이 IMM+TSF 겸용 단일 언어바 항목. 개발자 스스로 "MS 한글 IME와 100% 동등 호환은
  보장 못 한다"고 명시 → **마법적 우회 없음, 표준 IMM32/TSF 채널 위에서 동작**.
  소스: http://moogi.new21.org/ngs_imple.htm

### 판정 근거
- 데스크톱 Win32 앱(카톡/한컴은 UWP 아님)에서 IMM32 `.ime` 는 `LoadLibrary` 되어 IMM32 경로로 키를 받는다.
  UWP/Store 앱만 IMM32 차단(MS Learn 명시). → **B안이 키를 전달할 구조적 근거는 충분 = likely**.
- **confirmed 로 못 올리는 이유**: 어떤 1차 자료도 "카톡(CEF/크로미움 계열)·구형 한컴에서 `.ime`가 실제로
  inline 조합을 그렸다"는 **스크린샷/이슈 실증을 제시하지 못함**. 날개셋 매뉴얼은 CEF/Qt 자체 엔진 앱을 TSF 승격
  혜택 대상에서 **명시적으로 제외**. 카톡은 그 제외 범주. → "키는 IMM32로 닿지만 inline 표시가 되는지"는 미확정.

### UNIM 코드감사 — "단순 원인(미설치)" 배제 여부
**단순 원인(32비트 .ime 미설치)은 코드 레벨에서 배제됨.** B안 인프라가 이미 정상 구현:
- `unim-imm32` 는 x64(`x86_64-pc-windows-msvc`) + x86(`i686-pc-windows-msvc`) **양쪽 빌드** —
  windows-msi.yml:79-87 이 `vcvarsamd64_x86.bat` 호출 후 i686 빌드, 둘 다 `unim_imm32.ime` 로 rename.
- WiX 가 x64 사본을 **System32**(System64Folder), x86 사본을 **SysWOW64**(SystemFolder)에 배치 —
  unim.wxs:252-298. 동일 leaf명 충돌 없음. 양쪽 컴포넌트가 각자 KLID `E0200412` 레지스트리 5값 기록.
- `.ime` 는 **스텁 아님**: `ImeProcessKey`→`input::should_consume`, `ImeToAsciiEx`→`engine.press_key`/
  `feed_key`/`composition::build_and_emit`, `NotifyIME` CPS_CANCEL/CPS_COMPLETE 실제 처리(lib.rs:197-311).
  `.def` 가 IMM32 DDI 17개 전부 undecorated export.
- **결론**: VM에서 카톡 `OnKeyDown 0회`가 나와도 그것은 "32비트 미설치"가 아니라 **진짜 키 라우팅 차단(앱이
  IMM32 후킹)** 이며, 그 경우 B안 `.ime`(SysWOW64)만이 키를 받을 수 있다 — 이 분기는 VM 실측이 유일 결정자.

### 잔여 BLOCKER (즉시 적용 가능, VM 이전)
- **`Layout File`=`KBDKOR.DLL` 검증 필요**: 프로브 라운드들이 과거 `KBDA1.DLL` 오류를 지적. 현재 wxs/globals
  는 `KBDKOR.DLL`로 일치(unim.wxs:268/292, globals.rs). KBDKOR.DLL 이 모든 타깃 Windows에 존재하는지 VM 1회 확인.

---

## 2. ③ 진짜 콘솔 (conhost vs Windows Terminal) — **판정: 분기 확정, inline은 conhost에서 unknown / Terminal은 ✅(VM)**

### conhost(cmd/PowerShell 레거시 콘솔창)
- 콘솔 IME 는 conhost 의 **Win32 메시지 루프**(`src/interactivity/win32/windowproc.cpp` `ConsoleWindowProc`)
  + IMM/ConIME + `COOKED_READ_DATA`(라인입력)에 묶임. TSF inline 경로와 **완전히 다른 계층**.
  소스: https://deepwiki.com/microsoft/terminal/2.5-console-host-architecture
- 외부 TSF text service(UNIM 같은)가 conhost에서 inline preedit을 그릴 수 있는지 **1차 실증 0건** = unknown.
- **성숙 IME의 실측 정책 = inline 포기**: weasel 은 출고 `weasel.yaml` 이 `app_options/cmd.exe`·`conhost.exe`
  에 `ascii_mode: true` 강제(커밋 28cdd09, conhost.exe가 cmd/PowerShell/WSL 포괄). mozc 는 별도
  `mozc_renderer.exe` overlay 후보창. → **콘솔 한글 inline은 성숙 IME조차 시도하지 않는다.**
  소스: https://github.com/rime/weasel/commit/28cdd09692f77e471784bf85ff7a19bc48e113f4

### Windows Terminal / VS Code 통합터미널 (ConPTY+TSF)
- Terminal 은 conhost ConIME 를 안 쓰고 자체 `TSFInputControl`(CoreTextEditContext)로 inline **오버레이**.
  → UNIM `composition.rs` TSF 경로가 그대로 적용 = ✅(VM).
  소스: https://github.com/microsoft/terminal/pull/4796
- **한글 회귀 핵심 규칙(PR #4796 확정)**: `CompositionCompleted` 즉시 버퍼 클리어는 중·일은 OK지만 한국어는
  '마지막 자모가 다음 글자 첫 자모로 넘어가는' 특성 때문에 **트리거 글자 자체를 지운다** → Enter/Esc(commit
  확정)까지 preedit 버퍼 보존이 정답. UNIM의 commit 타이밍을 이 규칙으로 점검 필요.
  소스: https://github.com/microsoft/terminal/issues/4226 , https://github.com/microsoft/terminal/pull/4796
- 한자 변환은 Terminal 1.18(2024)까지도 미동작(reconversion 경로 약함).
  소스: https://github.com/microsoft/terminal/issues/16537
- 2026 현재 VS Code 가 `terminal.integrated.windowsUseConptyDll` 전제로 조합 오버플로 수정 중 — CLI inline은
  여전히 활발한 미완성 영역. 소스: https://github.com/microsoft/vscode/issues/301552

### 현실성 결론
- conhost: **오버레이 폴백(preedit_window) + 즉시-terminate** 이 합리적 타협. inline 집착 비권장 — 성숙 IME 일치.
- Terminal/VSCode: TSF inline 경로 채택 + PR #4796 버퍼 보존 규칙 적용. ConPTY 켜진 최신 터미널을 1순위 QA.

---

## 3. 매트릭스 ⑤③ 칸 갱신 (vs `_IMPLEMENTATION_PATHS.md`)

| 칸 | 기존 _IMPLEMENTATION_PATHS.md | 이번 라운드 갱신 | 충돌? |
|---|---|---|---|
| **⑤ 기본입력** | ❌→⚠ (G2: 키 라우팅, B안 "미검증") | ⚠ 유지 — B안의 **검증 수준 상향**: 새나루/weasel#257/mozc/날개셋이 ".ime+TSF 듀얼"을 **검증된 표준 패턴**으로 확립. UNIM 인프라(듀얼 빌드·System32/SysWOW64 배치·실제 DDI)는 **이미 완비**. "미설치=단순원인" 코드 레벨 배제. 잔여는 "카톡 inline 표시 여부" 실측뿐. | **충돌 없음, 강화.** 기존 "(B) 비권장·미검증"의 *미검증*을 *생태계 검증됨·UNIM 인프라 완비, 표시 실측만 남음*으로 정밀화. |
| **③ 기본입력** | ⚠(VM): CUAS 콘솔 제외, 즉시-terminate→오버레이 폴백 | **conhost = unknown(오버레이 폴백 권장)**, **Terminal/VSCode = ✅(VM) TSF inline + PR#4796 규칙**. 콘솔을 두 군으로 **명시 분리**. | **충돌 없음, 세분화.** 기존 "③ 진짜 콘솔" 한 칸을 conhost(IMM/오버레이) vs ConPTY터미널(TSF)로 쪼갬. |

**충돌 시 어느 쪽이 맞나**: 이번 라운드가 더 정확. _IMPLEMENTATION_PATHS.md 가 ⑤를 "❌→⚠ 미검증",
③을 단일 칸으로 둔 것을 → ⑤ "likely·인프라 완비", ③ "conhost vs ConPTY 분리"로 갱신. 단 **최종 판정은
여전히 VM 실측이 유일 분기 결정자**(아래 §5).

---

## 4. 지금 당장(VM 없이) 적용 가능 액션

1. **⑤ 단순원인 배제 = DONE, 문서화만 남음** — 32/64 `.ime` 듀얼 빌드·배치·KLID 일관성은 코드 감사로 합격.
   `_IMPLEMENTATION_PATHS.md` P0-2 "단순원인 배제" 항목을 *코드상 완료, VM은 키-라우팅 차단 확정만*으로 갱신.
   근거: windows-msi.yml:79-87, unim.wxs:252-298, lib.rs:197-311.
2. **`KBDKOR.DLL` 존재 사전 확인 메모** — wxs 주석에 "KBDKOR.DLL 은 ko-KR 기본 키보드, 전 타깃 Windows에
   존재(과거 KBDA1.DLL 오류 회귀 금지)" 한 줄 고정. 근거: unim.wxs:268/292.
3. **③ 콘솔 분기 주석/QA 매트릭스 박기** — composition.rs/text_service.rs 에 "conhost=IMM/오버레이 폴백,
   ConPTY터미널=TSF inline" 분기 전제 주석. QA 매트릭스를 conhost / Windows Terminal / VSCode 3행으로 분리.
   근거: terminal#4226, deepwiki console-host-architecture.
4. **PR #4796 한글 버퍼 보존 회귀 테스트** — TSF commit 타이밍이 "CompositionCompleted 즉시 클리어"가 아니라
   "Enter/Esc/commit 확정까지 preedit 보존"인지 코드 점검 + 단위테스트 고정. 근거: terminal/pull/4796.
5. **per-app 호환 토글 도입 검토(weasel app_options 패턴)** — exe명별 `ascii_mode`(콘솔 영문강제) /
   commit-중복방지 override 맵. 콘솔(conhost.exe/cmd.exe/powershell.exe)은 기본 `ascii_mode:true` 출고.
   날개셋도 동일 우회를 사용자 토글로 제공(코드 분기 대신 설정화). 근거: weasel commit 28cdd09.
6. **(선택) CTF\\Assemblies Default 슬롯 영구화 검토** — install 경로에 Assemblies 쓰기 부재 확인됨(rs/wxs
   ZERO, 진단 ps1만 존재). 단일 언어바 항목 통합/Win+Space TIP 슬롯 소유권이 VM에서 문제되면 ActiveSetup으로
   per-user 기록 추가. **B안 .ime 동작 자체엔 불필요**(Preload만으로 IMM32 폴백 완결). 근거: scripts/unim-probe-assemblies.ps1.

---

## 5. VM 실측으로만 풀리는 잔여 (stillVMonly)

- **O4 ⑤** — 카톡(CEF)·구형 한컴에서 B안 `.ime`(SysWOW64)가 실제로 키를 받고 **inline 조합을 그리는가**.
  (키 라우팅 차단 확정 → .ime가 키 받는가 → inline 표시되는가, 3단계 procmon/Spy++ 실측.)
- **O3 ③ conhost** — 외부 TSF text service가 conhost 레거시 콘솔창에서 inline preedit을 그릴 수 있는지의 1차
  실증(현존 0건). 안 되면 오버레이 폴백 실동작 검증.
- **TSF x64-only WOW64 활성화** — x64 단독 `unim_tsf.dll` 이 WOW64 32비트 TSF 호스트에서 활성화되는지 런타임 확인.
- **한자 reconversion** — Terminal/conhost 가 TSF reconversion 콜백을 노출하는지(Terminal 1.18까지 미동작).
- **PR#4796 한글 특성 회귀** — fInterimChar=TRUE 가 GUI(메모장/Word) 선택음영·역입력 회귀를 일으키는지 GUI+콘솔 양쪽.
