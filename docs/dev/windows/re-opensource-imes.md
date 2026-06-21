# 오픈소스 Windows IME 등록·로드 메커니즘 역공학 (A2)

> 목표: UNIM 이 KakaoTalk 같은 32-bit 데스크톱 앱에서 한글 입력되게 하는 **실제로
> 작동하는** 등록·로드 경로를 오픈소스 IME(weasel / mozc / 날개셋 / ImeStudy) 1차
> 소스로 규명. 이전 결론("카톡=TSF 완전 우회") 재검증 포함.
> 기준일 2026-06-22. 이 머신 실측(Win11 26200) + GitHub 소스/MS 1차 문서.
> 표기: **[측정]** 이 머신 실측 / **[1차]** 소스코드·공식문서 / **[정황]** 2차.

---

## TL;DR — 3개의 결정적 발견

1. **★카톡=TSF/IMM32 우회는 FALSE NEGATIVE 였다 [측정].**
   `tasklist /m` 이 KakaoTalk 모듈을 못 본 건 안티탬퍼가 아니라 **KakaoTalk 이 32-bit
   (WOW64) 프로세스이고, 64-bit `tasklist` 가 WOW64 프로세스의 32-bit 모듈 목록을
   구조적으로 못 읽기 때문**이다. 실측: KakaoTalk(PID 20892) 모듈 = `ntdll.dll,
   wow64.dll, wow64base.dll, wow64win.dll, wow64con.dll, wow64cpu.dll` **딱 이 6개
   (전부 WOW64 브리지)** 만 보임. 같은 패턴이 다른 32-bit 앱(HncUpdateTray.exe,
   spacedeskServiceTray.exe)에도 동일. KakaoTalk.exe 는 `Program Files (x86)` 설치
   = 32-bit 확정. → **카톡은 32-bit imm32.dll(+CUAS 경유 msctf)을 정상 로드한다.
   우리가 못 봤을 뿐.** "msctf 미로드=TSF 우회" 결론 폐기.

2. **현대 오픈소스 IME 의 공통 등록 메커니즘은 `ImmInstallIME` 가 아니라
   `input.dll!InstallLayoutOrTip` + TSF `RegisterProfile(hklsubstitute)` 다 [1차].**
   - weasel(master): **IMM/.ime 지원을 완전히 제거**("TSF-only build")하고
     `EnableLanguageProfile` + `InstallLayoutOrTip()` 만 사용.
   - mozc(master): `imm_util.cc` 가 `InstallLayoutOrTip(profile, 0)` +
     `SetDefaultLayoutOrTip` 로 **TSF 프로파일을 활성화**. profile 문자열 =
     `"0x0411:{CLSID}{ProfileGUID}"`. IMM32 는 *"Win8 이상 비권장"* 으로 격하.
   - 즉 **ImmInstallIME 가 Win11 에서 죽었다(HKL=0)는 게 사실이라도 우회로가 있다:
     TSF TIP 를 등록하고 input.dll 로 활성화**한다.

3. **TSF TIP 를 "HKL 처럼" 보이게 하는 정식 API = `ITfInputProcessorProfileMgr::
   RegisterProfile` 의 `hklsubstitute` 인자 [1차].** weasel 의 "combine IME and TSF"
   커밋이 바로 이것: `RegisterProfile(clsid, langid, guidProfile, …, hkl, 0, TRUE, 0)`.
   이게 TSF 프로파일에 HKL 정체성을 부여해 **TSF 비대응/레거시 경로가 그 HKL 로
   입력기를 참조**할 수 있게 한다(=전 앱 도달의 정식 비결). UNIM 이 손으로 박던
   `Substitutes`/`Assemblies` 미러는 이 API 의 부산물을 흉내 낸 것일 뿐 — **정식
   호출이 그 키들을 OS 가 생성하게 만든다.**

---

## 1. ★카톡 도달 경로 재검증 (false-negative 규명) [측정]

### 1-1. 실측 절차와 결과

```
$ tasklist                       → KakaoTalk.exe PID 20892 실행 중 확인
$ tasklist /m imm32.dll          → 132개 프로세스(전부 64-bit). KakaoTalk 없음.
$ tasklist /m msctf.dll          → 66개. KakaoTalk 없음.
$ (full) tasklist /m | grep -i kakao
   KakaoTalk.exe  20892  ntdll.dll, wow64.dll, wow64base.dll,
                          wow64win.dll, wow64con.dll, wow64cpu.dll
$ ls "C:/Program Files (x86)/Kakao/KakaoTalk/KakaoTalk.exe"   → 존재(=32-bit 설치 경로)
```

전체 모듈 덤프에서 **"WOW64 6종만" 보이는 프로세스 = KakaoTalk, HncUpdateTray,
spacedeskServiceTray** — 모두 32-bit 앱. 64-bit `tasklist`(및 64-bit Process API)는
WOW64 프로세스를 가리키면 **CPU 가 64-bit 인 wow64 에뮬레이션 스텁만** 열거하고 그
안에 로드된 진짜 32-bit DLL 들(`SysWOW64\imm32.dll`, `SysWOW64\msctf.dll` 등)을
**열거하지 못한다.**

### 1-2. 결론

- **이전 "카톡은 msctf.dll 미로드 = TSF/CUAS 완전 우회" 는 도구 한계로 인한
  false-negative.** [측정] 폐기.
- 32-bit Win32 GUI 앱은 예외 없이 `SysWOW64\imm32.dll` 을 로드한다(user32 초기화가
  강제). 따라서 **카톡은 IMM32 경로가 살아 있다.** CUAS(`msctfime.ime`)가 활성이면
  TSF TIP 도 IMM32 브리지로 카톡에 도달한다.
- **검증을 제대로 하려면**(후속): ① 32-bit 도구로 봐야 함 — Sysinternals
  **ListDLLs(32-bit 빌드)** 또는 **Process Explorer**(WOW64 모듈 표시 가능), 또는
  `wmic process where processid=20892 get ...` 대신 **32-bit 프로세스 안에서**
  `EnumProcessModules`. ② 더 확실한 직접 증거: **우리 .ime/TIP 가 로드되면 자기
  로그를 쓰게** 해서 카톡 포커스 시 로그 생성 여부로 판정(현재 tasklist 의존 금지).
  - 이 머신엔 ListDLLs/Process Explorer 미설치(확인됨). PowerShell deny 로 .NET
    경유 모듈 열거도 막힘. → **32-bit ListDLLs 설치 또는 .ime 자체 로그가 다음 실측.**

> 함의: UNIM TSF TIP 가 이미 다수 TSF 앱에 로드된다면, **CUAS 가 그 TIP 를 카톡(32-bit
> IMM32 앱)에도 브리지할 수 있다.** 별도 .ime 가 정말 필요한지부터 재검토 대상.
> (단, CUAS 브리지가 *모든* 레거시 앱에서 완전하지 않다는 보고가 있어 §5 참조.)

---

## 2. weasel (rime/weasel) — TSF-only 로 전향 [1차]

소스: `WeaselSetup/imesetup.cpp` (master).

### 2-1. 현재(master): IMM32 완전 제거

- 파일에 **`ImmInstallIME` 없음**, **`register_ime`(IMM/.ime) 주석으로만 존재**:
  `// register_ime (IMM/.ime) support removed — TSF-only build`. [1차]
- `RegisterProfile` / `SubstituteKeyboardLayout` / `hkl` / `Preload` **모두 없음**.
- 남은 등록 호출: `ITfInputProcessorProfiles::EnableLanguageProfile(c_clsidTextService,
  lang_id, c_guidProfile, fEnable)` + `EnableLanguageProfileByDefault` +
  `RemoveLanguageProfile` + **`input.dll!InstallLayoutOrTip`**. [1차]
- DLL 자체 등록은 `regsvr32 /s weasel.dll`(silent) 또는 직접 COM 등록.
- 32/64: NSIS 인스톨러가 아키텍처별 바이너리 배치(`AtLeastWin11`/`IsNativeAMD64`/
  `IsNativeARM64` 매크로). HKLM 키: `SOFTWARE\Rime\Weasel\InstallDir`,
  `...\Run\WeaselServer`(서버 자동시작). [1차/정황]

### 2-2. 과거(combine 커밋 91cbd2c): IMM32+TSF 결합의 정석 [1차]

지금은 제거됐지만 **"TSF TIP 에 HKL 정체성 부여" 기법의 교과서**라 보존 가치 큼:

1. `ImmInstallIME()` 로 IMM32 .ime 등록 → **반환 HKL 캡처**(`ImeHKL`).
2. 그 HKL 을 TSF 등록에 전달:
   - **Win8+**: `pInputProcessorProfileMgr->RegisterProfile(c_clsidTextService,
     LANGID, c_guidProfile, desc, …, icon, …, **hkl**, 0, TRUE, 0)`
   - **Win7 이하**: `pInputProcessorProfiles->SubstituteKeyboardLayout(
     c_clsidTextService, LANGID, c_guidProfile, **hkl**)`
3. 결과: TSF TIP 와 IMM32 .ime 가 **하나의 입력기로 통합**되어 사용자가 하나만 골라도
   둘 다 활성. HKL 은 하드코딩 아니라 ImmInstallIME 반환값.
4. 이 커밋은 `regsvr32` 스폰을 **인스톨러 내 직접 COM 등록**으로 대체.

> 시사점: UNIM 의 현행 수동 `Substitutes(E0200412→00000412)` + `Assemblies\
> 0x00000412\{TIP}` 미러는 **이 RegisterProfile(hklsubstitute) 호출이 자동 생성하는
> 키를 손으로 흉내 낸 것**이다. 손으로 박으면 §imm32-load-research D 의 분석대로
> .ime 로드를 오히려 방해. **정식 API 를 호출해 OS 가 키를 만들게 하는 게 옳다.**

---

## 3. mozc (google/mozc) — TSF 우선, IMM32 잔존(비권장) [1차]

소스: `src/win32/base/imm_util.cc`, `tsf_registrar.cc`; 문서: `docs/build_mozc_in_windows.md`.

### 3-1. 핵심: 등록=TSF, 활성=InstallLayoutOrTip

- `imm_util.cc` 의 활성 코드는 **TSF 전용**:
  ```cpp
  // profile = "0x0411:" + CLSID + ProfileGUID   (0x0411 = ja-JP)
  if (!::InstallLayoutOrTip(profile.c_str(), 0)) { … }
  if (!::SetDefaultLayoutOrTip(profile.c_str(), 0)) { … }
  ```
  → **ImmInstallIME 가 아니라 `input.dll!InstallLayoutOrTip` 로 프로파일을 켠다.** [1차]
- `tsf_registrar.cc`: `ITfInputProcessorProfiles::Register` +
  `AddLanguageProfile(textsvc_guid, langid, profile_guid, desc, …, resource_dll,
  …, icon_index)` + 카테고리 등록(`RegisterCategories`):
  - `GUID_TFCAT_TIP_KEYBOARD`, `GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER`,
    `GUID_TFCAT_TIPCAP_COMLESS`, `..._INPUTMODECOMPARTMENT`, `..._UIELEMENTENABLED`,
    `..._IMMERSIVESUPPORT`, `..._SYSTRAYSUPPORT`. [1차]
  - 이 버전은 `AddLanguageProfile`(구 API) 사용 — hklsubstitute 안 씀.
- **IMM32 는 여전히 빌드되지만** 문서가 *"IMM32 and TSF … IMM32 is **not recommended
  on Windows 8 and later**"* 명시. [1차]

### 3-2. 등록 실행 메커니즘

- `mozc_broker32.exe --mode=register_ime`(관리자) + `mozc_broker64.exe
  --mode=register_ime`(관리자) — **32/64 각각 별도 broker 를 관리자로 실행**. [1차]
- 또는 `regsvr32`(관리자)로 IME 모듈 등록 가능. [1차]
- **SysWOW64 버그 워크어라운드**(검색 요약 [정황], 코드 미인용): ImmInstallIME 가
  64-bit Win 에서 SysWOW64 를 시스템 폴더로 인식 못 해 **System32 경로+파일명을
  넘겨야** 한다. → IMM32 경로를 쓸 거면 경로 처리 주의(우리 §imm32-load-research B-1
  와 일치). 단 modern mozc 는 이 경로를 사실상 안 쓰고 TSF 로 감.

> 시사점: **mozc 도 "Win11 에선 TSF 로 등록·활성"이 메인라인.** IMM32 .ime 는
> 레거시 호환용 잔재. UNIM 이 이미 작동하는 TSF TIP 를 가졌다면 mozc 와 같은 길
> (`InstallLayoutOrTip` 로 프로파일 활성 + 필요시 hklsubstitute)을 가는 게 정공법.

---

## 4. 날개셋 / ImeStudy / 기타 [정황/1차]

### 4-1. 날개셋(Nalgaeset, moogi.new21.org) — 비오픈소스 [정황]

- IMM32 와 TSF **양쪽 모두 제공**(나무위키/위키백과). 사용자가 IME(.ime) 계층과
  TSF 계층을 선택. "전 앱 동작"을 표방하나 **완전하지 않다**:
  - 발로란트(안티치트 풀스크린)에서 날개셋만 켜면 **한글 안 되고 영어만** 입력되는
    사례 보고. → 안티치트/배타적 풀스크린은 IMM32·TSF 둘 다 차단 가능.
  - 디스코드 등에서 조합 강제종료 버그 → "시스템 계층 > 프로그램 호환성 >
    응용 프로그램별 세부 보정" 으로 앱별 보정 제공.
- 시사점: **"전 앱 100% 도달"은 IMM32 든 TSF 든 보장 안 됨.** 날개셋조차 앱별 보정
  스위치를 둔다. 카톡(일반 32-bit Win32 앱)은 안티치트 앱보다 훨씬 쉬운 케이스 —
  IMM32/CUAS 표준 경로로 도달 가능성 높음.

### 4-2. katahiromz/ImeStudy — IMM32 IME 교육용 [1차]

- README 가 **VERSIONINFO 필수 조건 명시**: IME 파일은 `FILETYPE=VFT_DRV`,
  `FILESUBTYPE=VFT2_DRV_INPUTMETHOD` 여야 하고 인스톨러가 **`ImmInstallIME` 호출**.
  → **UNIM 이 `unim_imm32.rc` 에 추가한 VS_VERSION_INFO 가 정확히 이 요건**(우리가
  1813 해결한 그 수정). 방향 일치 확인. [1차]
- 실제 등록 스크립트/Win11 호환 노트는 README 에 없고, IMM32 내부는 **WineHQ/ReactOS
  소스 + "IME Hackerz"** 참조하라고 안내. → IMM32 내부 동작은 ReactOS `imm32`/`win32k`
  소스가 1차 자료(후속 조사처).

### 4-3. (참고) ImeSharp, google-input-tools/client/imm/registrar.cc

- `google-input-tools` 의 `imm/registrar.cc` 는 **구형 IMM32 등록기**(ImmInstallIME
  계열)의 실코드 — IMM32 경로를 끝까지 봐야 하면 1차 참조. (modern 경로는 아님.)

---

## 5. 종합 — UNIM 이 가야 할 길 (메커니즘 요지)

### 작동하는 등록·로드 메커니즘 (오픈소스 합의)

| 단계 | 정식 메커니즘 | API/키 |
|---|---|---|
| TIP 구현 등록 | COM 서버 등록 | `regsvr32 unim_tsf.dll` 또는 직접 `DllRegisterServer` |
| 언어 프로파일 등록 | TSF 프로파일 + 카테고리 | `ITfInputProcessorProfiles::AddLanguageProfile` **또는** `...Mgr::RegisterProfile`(+카테고리 `GUID_TFCAT_TIP_KEYBOARD` 등) |
| **HKL 정체성 부여(전앱 도달 핵심)** | RegisterProfile 의 **hklsubstitute** | `RegisterProfile(clsid, 0x0412, guidProfile, …, **hkl**, 0, TRUE, 0)` — OS 가 Substitutes/Assemblies 자동 생성 |
| 사용자 활성 | input.dll 로 켜기 | `InstallLayoutOrTip("0x0412:{CLSID}{ProfileGUID}", ILOT_DEFPROFILE?)` + `SetDefaultLayoutOrTip` |
| 32/64 | 양쪽 등록 | 32-bit 등록기 + 64-bit 등록기 각각 관리자 실행(mozc broker32/64 패턴) |

### `InstallLayoutOrTip` 사양 [1차]

- DLL: `input.dll`. import lib 없음 → `LoadLibrary("input.dll")` +
  `GetProcAddress("InstallLayoutOrTip")`.
- `psz` 형식: 레이아웃 = `"<LangID>:<KLID>"`, TIP = `"<LangID>:{CLSID}{ProfileGUID}"`.
  예: `"0x0412:{A1B2C3D4-…}{B2C3D4E5-…}"`.
- 플래그: `ILOT_UNINSTALL=0x1`, `ILOT_DEFPROFILE=0x2`(기본 지정),
  `ILOT_DEFUSER4=0x4`, `ILOT_CLEANINSTALL=0x40`, `ILOT_DISABLED=0x80`.
- **per-user**, **데스크톱 앱 전용**(Vista+). ImmInstallIME 와 달리 KLID 를 OS 가
  새로 할당하는 게 아니라 **이미 등록된 프로파일/레이아웃을 "켠다".**
- 주의(MS): *"IME 가 enabled(Preload 에 등재) 안 됐으면 실패"* — 호출 전 대상 IME 가
  활성 가능 상태여야 함.

### RegisterProfile 의 hklsubstitute [1차]

- 시그니처: `RegisterProfile(rclsid, langid, guidProfile, pchDesc, cchDesc,
  pchIconFile, cchFile, uIconIndex, **HKL hklsubstitute**, DWORD dwPreferredLayout(=0),
  BOOL bEnabledByDefault, DWORD dwFlags)`. `dwPreferredLayout` 은 *"Unused, must be 0"*.
- **hklsubstitute = "이 TSF 프로파일의 대체 HKL"** — 레거시/IMM32 경로가 이 HKL 로
  TIP 를 참조하게 만든다. weasel combine 커밋이 ImmInstallIME 반환 HKL 을 여기 전달.

### UNIM 권고(이 갈래 결론)

1. **카톡 우회 가설 폐기.** 카톡=32-bit, imm32.dll(+CUAS msctf) 로드함. tasklist
   결과는 WOW64 false-negative. → **별도 .ime 없이도 TSF TIP 가 CUAS 로 도달 가능성**
   부터 32-bit ListDLLs/Process Explorer 또는 .ime/TIP 자체 로그로 **재실측**.
2. **수동 레지스트리(Substitutes/Assemblies/Keyboard Layouts) 박기 중단.** 대신
   **`RegisterProfile(…, hklsubstitute, …)` + `InstallLayoutOrTip`** 정식 호출로
   OS 가 그 키들을 만들게 한다(weasel/mozc 합의).
3. IMM32 .ime 를 굳이 유지하면: **ImmInstallIME 로 등록 → 반환 HKL → 그 HKL 을
   RegisterProfile hklsubstitute 로** TSF TIP 와 결합(weasel combine 방식). 단
   Win11 에서 ImmInstallIME 가 HKL=0 이면(이 머신 실측) **이 경로는 막힘 →
   InstallLayoutOrTip 단독(TSF) 경로가 현실적.**
4. 32/64 등록을 **분리 실행**(mozc broker 패턴). 카톡은 32-bit 이므로 **32-bit
   등록기/뷰가 반드시 필요.**

---

## 6. 신뢰도 / 미확인

| 항목 | 판정 | confidence |
|---|---|---|
| 카톡=32-bit, tasklist 가 WOW64 모듈만 봄(false-negative) | 사실 | **확인됨**[측정] |
| 카톡이 imm32.dll/msctf.dll 을 "실제로" 로드 | 강한 추론(모든 32-bit Win32 앱 + WOW64 차단으로 직접확인 불가) | **정황→높음** — 32-bit ListDLLs 로 직접확인 필요 |
| weasel master = IMM32 제거, InstallLayoutOrTip+EnableLanguageProfile | 사실 | **확인됨**[1차] |
| mozc = TSF 우선, InstallLayoutOrTip/SetDefaultLayoutOrTip, IMM32 비권장 | 사실 | **확인됨**[1차] |
| RegisterProfile hklsubstitute = TSF TIP 에 HKL 정체성 부여(전앱 도달) | 사실 | **확인됨**[1차 MS+weasel] |
| ImmInstallIME 가 Win11서 HKL=0(이 머신) → 우회=TSF InstallLayoutOrTip | 사실/방향 | **확인됨**(우회 존재) [1차] |
| InstallLayoutOrTip 단독으로 카톡(32-bit IMM32 앱)에 한글 입력됨 | 미확인 | **추측** — 실측 필요 |
| CUAS 가 UNIM TSF TIP 를 카톡에 브리지하는지 | 미확인 | **추측** — .ime/TIP 로그로 실측 |
| 날개셋 "전앱"도 안티치트 앱은 실패 | 사실 | **확인됨**[정황] |

---

## 참고문헌 (URL)

- weasel imesetup.cpp(master, TSF-only): https://github.com/rime/weasel/blob/master/WeaselSetup/imesetup.cpp
  / raw: https://raw.githubusercontent.com/rime/weasel/master/WeaselSetup/imesetup.cpp
- weasel "combine IME and TSF" 커밋 91cbd2c(RegisterProfile+SubstituteKeyboardLayout): https://github.com/rime/weasel/commit/91cbd2c
- weasel Setup/Registration 위키: https://deepwiki.com/rime/weasel/7.2-setup-and-registration , https://deepwiki.com/rime/weasel/7.1-installation-system
- weasel TSF KLID PR #272(레지스트리로 IME KLID 탐색): https://github.com/rime/weasel/pull/272
- mozc imm_util.cc(InstallLayoutOrTip/SetDefaultLayoutOrTip): https://github.com/google/mozc/blob/master/src/win32/base/imm_util.cc
- mozc tsf_registrar.cc(AddLanguageProfile+카테고리): https://github.com/google/mozc/blob/master/src/win32/base/tsf_registrar.cc
- mozc input_dll.h(input.dll 동적로드 선언): https://github.com/google/mozc/blob/master/src/win32/base/input_dll.h
- mozc build 문서(broker32/64 register_ime, IMM32 비권장): https://github.com/google/mozc/blob/master/docs/build_mozc_in_windows.md
- google-input-tools imm/registrar.cc(구형 IMM32 등록기): https://github.com/google/google-input-tools/blob/master/client/imm/registrar.cc
- MS InstallLayoutOrTip(input.dll, psz 형식/플래그): https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
- MS SetDefaultLayoutOrTip: https://learn.microsoft.com/en-us/windows/win32/tsf/setdefaultlayoutortip
- MS RegisterProfile(hklsubstitute 인자): https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile
- katahiromz/ImeStudy(VFT_DRV/VFT2_DRV_INPUTMETHOD, ImmInstallIME 안내): https://github.com/katahiromz/ImeStudy
- 날개셋 나무위키: https://namu.wiki/w/날개셋%20한글%20입력기
- 내부: docs/dev/windows/imm32-load-research.md(ImmInstallIME 절차 상세), unim-imm32/, unim-windows-common/src/activation.rs

*다음 실측: ① 32-bit ListDLLs/Process Explorer 로 카톡의 SysWOW64\imm32.dll·msctf.dll
로드 직접확인 ② UNIM TSF TIP/.ime 에 로드 로그 심어 카톡 포커스 시 생성 여부 ③ ReactOS
imm32/win32k 소스로 IMM32 내부 KLID 매핑 확인.*
