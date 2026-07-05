# UNIM 등록/로드 API 심층 재조사 (A4) — KakaoTalk 한글 입력 정공 경로

> 작성: 2026-06-22 / 브랜치 feat/windows-msi-redesign
> 목적: 인가된 상호운용 RE. "ImmInstallIME 막힘 → IMM32 .ime 우회" 가설을 제로베이스
> 재검증하고, UNIM TSF TIP을 KakaoTalk 등 레거시 앱에 닿게 하는 실제로 작동하는 경로를 규명.

---

## 0. 결론 요약 (TL;DR)

1. **이전 결론 "KakaoTalk은 msctf 미로드 = TSF 우회"는 거짓이었다.** 그것은 WOW64
   false-negative였다. KakaoTalk.exe는 **x86(32비트, PE machine 0x014C)** 프로세스이고,
   64비트 도구(tasklist, .NET Process.Modules, ProcExp64)는 32비트 프로세스의 실제
   모듈을 못 보고 wow64*.dll 6개만 본다. **32비트 Toolhelp32 프로브로 재측정하니
   KakaoTalk은 msctf.dll·imm32.dll·TextInputFramework.dll·CoreMessaging.dll·user32.dll을
   정상 로드(총 118개 모듈)한다.** → KakaoTalk은 평범한 TSF/CUAS 클라이언트다.

2. **Microsoft 한국어 IME는 IMM32 .ime가 아니라 TSF TIP다** (CLSID
   `{a028ae76-01b1-46c2-99c4-acd9858ae02f}`, MS Learn InstallLayoutOrTip 예제 문자열의
   바로 그 CLSID). MS 한국어가 KakaoTalk에 닿는 길 = **TSF TIP → CUAS 브리지 → IMM32 앱**.
   UNIM TIP도 동일 인프라(msctf/CUAS)를 그대로 탈 수 있다. **IMM32 .ime 별도 제작은 불필요.**

3. **IMM32 .ime 우회 전체가 막다른 길이었다.** ImmInstallIME 신경차단(neutered),
   E0200412 grayed, LoadKeyboardLayout 1419 — 전부 "Win11에서 서드파티 IMM32 IME는
   더 이상 1급 시민이 아니다"의 증상. 우리가 풀어야 할 문제는 IMM32가 아니라
   **TSF TIP을 사용자 입력목록에 제대로 등록·활성화**하는 것이었다.

4. **현 UNIM의 실제 결함:** TIP은 HKLM CTF\TIP에 등록됨(✓). 그러나 사용자 입력목록
   (HKCU CTF\Assemblies) 항목이 **불완전**하다 — 측정값:
   `CLSID=[비어있음]`, `Enable=[비어있음]` (Profile/Default만 채워짐). 이는
   `SetDefaultLanguageProfile`/`ActivateProfile`만 호출하고 **InstallLayoutOrTip(입력목록
   추가) 단계를 빠뜨려서** 생긴 반쪽 등록이다.

5. **정공 경로:** `input.dll!InstallLayoutOrTip("0x0412:{CLSID}{Profile}", 0)` →
   사용자 입력목록에 정식 추가 → `SetDefaultLayoutOrTip(..., SDLOT_APPLYTOCURRENTTHREAD)`
   로 기본 지정. 그러면 CUAS가 KakaoTalk(IMM32) 쪽으로 이 TIP을 브리지한다.

---

## 1. ★재검증: KakaoTalk은 msctf/imm32를 로드하는가 (false-negative 규명)

### 1.1 무엇이 잘못됐었나
- `tasklist /m`, .NET `Process.Modules`, ProcExp64 = 모두 64비트 enumerator.
- 64비트 도구가 **32비트(WOW64) 프로세스**의 모듈을 열거하면 32비트 로더 데이터를
  못 읽고 wow64 thunk만 본다. 실측: .NET Process.Modules로 KakaoTalk = **7개**
  (KakaoTalk.exe + ntdll + wow64/wow64base/wow64win/wow64con/wow64cpu). **user32.dll조차
  안 보임** → 정상 GUI 앱이 user32 없이 돌 수 없으므로 이건 명백한 측정 오류.

### 1.2 올바른 측정 — 32비트 Toolhelp32 프로브
- 신규 `unim-windows-common/examples/proc_modules.rs` (i686 빌드, `CreateToolhelp32Snapshot`
  + `TH32CS_SNAPMODULE|SNAPMODULE32`).
- 빌드: `cargo build -p unim-windows-common --example proc_modules --target i686-pc-windows-msvc --release`
- 실행: `proc_modules.exe <KakaoTalk PID>`

```
[proc_modules] self-arch = 32-bit
[proc_modules] target PID = 20892
[proc_modules] total modules = 118
---- target module presence ----
  msctf.dll                    => LOADED
  imm32.dll                    => LOADED
  TextInputFramework.dll       => LOADED
  CoreMessaging.dll            => LOADED
  user32.dll                   => LOADED
  unim_imm32.ime               => absent
  unim_tsf.dll                 => absent
```

### 1.3 KakaoTalk 아키텍처 사실
- `C:\Program Files (x86)\Kakao\KakaoTalk\KakaoTalk.exe`, PE machine = **0x014C (x86)**.
- 118개 모듈 로드, **msctf.dll·imm32.dll·TextInputFramework.dll·CoreMessaging.dll 모두 LOADED**.
- 결론: **KakaoTalk은 TSF를 우회하지 않는다.** TSF를 로드하는 IMM32-호환 앱이며,
  CUAS(msctf의 cicero unaware 지원층)가 IMM32 API ↔ TSF TIP 사이를 브리지한다.

### 1.4 "ImmDisableTextFrameService opt-out인가?"
- opt-out 앱이라면 msctf가 그 스레드에서 비활성화되지만, **msctf.dll 자체는 여전히
  로드된다**(다른 스레드/공통 초기화). 단, KakaoTalk이 TextInputFramework·CoreMessaging까지
  로드한 정황은 오히려 **모던 텍스트 입력 스택을 적극 사용**함을 시사 → full opt-out일
  가능성은 낮다. (확정 1차 근거는 §6 미해결로 남김 — 그러나 MS 한국어 TIP이 실제로
  KakaoTalk에 한글 입력된다는 사용자 보고가 "CUAS 브리지 동작"의 강한 실증.)

---

## 2. 모던 등록 API: InstallLayoutOrTip 계열 (input.dll)

### 2.1 실측 — input.dll 위치·export (이 머신, Win11 26200)
- `C:\Windows\System32\input.dll` (x64), `C:\Windows\SysWOW64\input.dll` (x86) 모두 존재.
- dumpbin //exports (x64) 확인된 관련 export 25개 중:

| export | ordinal | 용도 |
|---|---|---|
| `InstallLayoutOrTip` | 104 | **현재 사용자** 입력목록에 레이아웃/TIP 추가·활성 |
| `InstallLayoutOrTipUserReg` | 109 | **지정 사용자**(다른 HKU)에 추가 — 설치 프로그램용 |
| `InstallLayoutOrTipOffline` | 120 | 오프라인 이미지(WIM) 대상 |
| `SetDefaultLayoutOrTip` | 107 | 기본 입력항목 지정 |
| `EnumEnabledLayoutOrTip` | 110 | 활성 입력목록 열거(진단) |
| `QueryLayoutOrTipString` | 111 | KLID/CLSID → LayoutOrTipString 변환 |
| `GetDefaultLayout` | 113 | 현 기본 레이아웃 조회 |

> 주의: 이전에 PowerShell Add-Type P/Invoke로 GetProcAddress 했을 때 NOT FOUND가
> 나온 것은 마샬링 함정일 뿐, **dumpbin은 이름 export 존재를 확정**한다.
> Mozc도 LoadLibrary("input.dll")+GetProcAddress(name)로 정상 로드한다.

### 2.2 InstallLayoutOrTip 시그니처·문자열·플래그 (MS Learn)
출처: https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
```c
BOOL CALLBACK InstallLayoutOrTip(_In_ LPCWSTR psz, _In_ DWORD dwFlags);
```
- 레이아웃 목록 형식: `<LangID>:<KLID>;...`  예 `"0x0407:0x00000407"`
- TIP 프로필 목록 형식: `<LangID>:{CLSID}{Profile};...`
  - **UNIM용 문자열:**
    `"0x0412:{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}{B2C3D4E5-F6A7-8901-BCDE-F12345678901}"`
  - 혼합 예(MS Learn 원문, MS 한국어=a028ae76 그대로):
    `"0x0407:0x00000407;0x0412:{A028AE76-01B1-46C2-99C4-ACD9858AE02F}{B5FE1F02-...};0x040C:0x0000040C"`
- dwFlags(ILOT_*, 공개 헤더 없음 → 직접 #define):
  | 상수 | 값 | 의미 |
  |---|---|---|
  | ILOT_UNINSTALL | 0x01 | 제거(=DISABLED) |
  | ILOT_DEFPROFILE | 0x02 | 기본 항목으로 설정 |
  | ILOT_DEFUSER4 | 0x04 | .Default 변경 |
  | ILOT_NOAPPLYTOCURRENTSESSION | 0x20 | 저장만, 현 세션 미적용 |
  | ILOT_CLEANINSTALL | 0x40 | 기존 레이아웃/TIP 전부 disable |
  | ILOT_DISABLED | 0x80 | 지정 항목 disable |
- 반환: TRUE/FALSE.

### 2.3 SetDefaultLayoutOrTip 플래그 (Mozc input_dll.h)
출처: https://github.com/google/mozc/blob/master/src/win32/base/input_dll.h
```c
BOOL WINAPI SetDefaultLayoutOrTip(LPCWSTR psz, DWORD dwFlags);
```
- SDLOT_NOAPPLYTOCURRENTSESSION = 0x01
- SDLOT_APPLYTOCURRENTTHREAD    = 0x02

### 2.4 실제 shipping IME(Mozc)의 등록 시퀀스 (Win Vista+ 정공)
출처: https://github.com/google/mozc/blob/master/src/win32/base/imm_util.cc
```cpp
const std::wstring profile = StrCatW(L"0x0411:", clsid, profile_id);   // 일본어
if (!::InstallLayoutOrTip(profile.c_str(), 0)) { /* fail */ }
::SetDefaultLayoutOrTip(profile.c_str(), 0 /*or SDLOT_APPLYTOCURRENTTHREAD*/);
// + ITfInputProcessorProfileMgr::ActivateProfile(TF_IPPMF_FORPROCESS|FORSESSION)
```
→ **Mozc는 ImmInstallIME를 쓰지 않는다.** TSF TIP + InstallLayoutOrTip + SetDefaultLayoutOrTip
가 Win11에서의 정식 경로. UNIM이 따라야 할 패턴이 바로 이것.

---

## 3. TSF↔IMM32 브리지: hklSubstitute & RegisterProfile

### 3.1 ITfInputProcessorProfileMgr::RegisterProfile 시그니처
출처: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile
```cpp
HRESULT RegisterProfile(
  REFCLSID rclsid, LANGID langid, REFGUID guidProfile,
  const WCHAR *pchDesc, ULONG cchDesc,
  const WCHAR *pchIconFile, ULONG cchFile, ULONG uIconIndex,
  HKL   hklsubstitute,        // ← 실제로는 KLID (아래 3.2)
  DWORD dwPreferredLayout,    // Unused, 반드시 0
  BOOL  bEnabledByDefault,
  DWORD dwFlags);             // TF_RP_HIDDENINSETTINGUI / LOCALPROCESS / LOCALTHREAD
```

### 3.2 hklSubstitute의 진실 (Marc Durdin RE)
출처: https://marc.durdin.net/2017/07/substituted-layouts-in-text-services-framework/
- 문서는 "LoadKeyboardLayout으로 얻은 HKL"이라 하지만 **실제로는 KLID(예 0x00010409)**를
  넣어야 한다. HKL을 넣으면 GetProfile이 0을 돌려준다(검증 안 함, 등록은 성공으로 보고).
- 언어 제한: substitute는 **TIP이 등록되는 언어에 묶인 레이아웃만** 가능.
- **이 기능의 본래 목적 = "IMM32 IME의 매끄러운 폐기와 TSF IME(TIP)로의 치환"**.
  즉 CUAS가 IMM32-only 앱에 TIP을 브리지할 때, 해당 TIP의 substitute KLID로 HKL 슬롯을
  채워 레거시 앱이 "키보드 레이아웃이 바뀐 것"처럼 보게 한다.
- **UNIM 함의:** 한국어 기본 레이아웃 KLID `0x00000412`를 hklSubstitute로 등록하면,
  CUAS가 KakaoTalk(IMM32)에서 UNIM TIP을 0x0412 HKL로 표상시켜 브리지한다.

### 3.3 UNIM 현 register.rs와의 대조 (결함 식별)
파일: `unim-tsf/src/register.rs`
- `register_server()`는 RegisterProfile/AddLanguageProfile을 **호출하지 않는다**
  (msctf 0x97e5a NULL deref 회피 이유로 제거). 대신:
  - HKCR\CLSID\{CLSID}\InProcServer32 (COM 서버) — OK
  - HKLM CTF\TIP\{CLSID}\LanguageProfile\0x0412\{Profile} 6값 직접 기록:
    Enable=1, **SubstituteLayout=0x0412(=langid 값)**, IconFile, IconIndex, Display, Description
  - Category 8종/TIP root는 wxs가 박음
- `set_as_default()`는 `SetDefaultLanguageProfile` + `ActivateProfile(ENABLEPROFILE|FORSESSION)`만.

**문제점 2가지:**
1. **SubstituteLayout 값이 KLID가 아닐 소지.** 코드는 `u32::from(UNIM_LANGID_KOREAN)`
   = 0x0412(langid)를 넣는다. §3.2에 따르면 substitute는 **KLID 0x00000412**여야 한다.
   레지스트리 SubstituteLayout 키는 HKL 형식(상위워드=device, 하위=langid)을 기대 →
   `0x04120412`(또는 KLID 0x00000412)가 맞을 수 있다. **현재 0x0412 단독은 의심.**
2. **사용자 입력목록 등록 누락.** `set_as_default`는 입력목록(HKCU CTF\Assemblies)에
   TIP을 정식 추가하지 않고 default/activate만 한다. InstallLayoutOrTip이 빠졌다.

---

## 4. ★실측: UNIM이 지금 HKCU에 어떻게 박혀 있나 (반쪽 등록 증거)

### 4.1 HKLM CTF\TIP — 한국어 TIP 3종 (정상 등록됨)
```
TIP={a028ae76-01b1-46c2-99c4-acd9858ae02f} profile={B5FE1F02-...}  Desc=Microsoft IME      ← MS 한국어 (TSF TIP!)
TIP={A1B2C3D4-E5F6-7890-ABCD-EF1234567890} profile={B2C3D4E5-...}  Desc=UNIM Korean IME    ← UNIM
TIP={a1e2b86b-924a-4d43-...}               profile={b60af051-...}  Desc=Microsoft Old Hangul IME
```
→ **MS 한국어 = TSF TIP 확정.** KakaoTalk 도달 경로 = TSF→CUAS. UNIM도 같은 인프라.

### 4.2 HKCU CTF\Assemblies\0x00000412 (사용자 입력목록) — **불완전**
```
Profile-key={34745C63-B2F0-4784-8B67-5E12C8701A31}   ← TSF Assembly 카테고리 컨테이너 GUID
  CLSID=[]                                            ← ★비어있음 (결함)
  Profile=[{B2C3D4E5-F6A7-8901-BCDE-F12345678901}]    ← UNIM profile
  KeyboardLayout=[68289554] = 0x04120412
  Default=[{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}]    ← UNIM CLSID (기본 지정됨)
  Enable=[]                                           ← ★비어있음 (결함)
```
- Profile/Default/KeyboardLayout은 채워졌으나 **CLSID·Enable이 빈 값** → 입력목록 항목이
  반쪽. `SetDefaultLanguageProfile`/`ActivateProfile`만으로는 정식 입력목록 항목(CLSID/Enable
  포함)이 만들어지지 않음을 실증. **InstallLayoutOrTip이 채워야 할 자리.**

### 4.3 레거시 IMM32 잔재 (정리 대상)
```
HKCU\Keyboard Layout\Preload:     1=00000412, 2=E0200412   ← E0200412는 실패한 .ime 우회 잔재
HKCU\Keyboard Layout\Substitutes: E0200412=00000412
HKL 활성 목록(GetKeyboardLayoutList): 0x04120412 (한국어 1개)
```
→ E0200412(.ime) 우회 흔적은 제거 권장. 정공 경로와 무관.

---

## 5. 왜 ImmInstallIMEW가 Win11 26200에서 HKL=0/err=0인가

- ImmInstallIME는 "IME 설치 프로그램 전용"의 레거시 IMM32 API. Win11은 텍스트 입력을
  TSF/TextInputFramework로 전면 이전 중이라 서드파티 **IMM32 .ime는 1급 시민이 아니다.**
  (MS Learn imm.h 문서 + Win11 22H2+ 신형 IME가 WM_IME_CONTROL/IMC_GETOPENSTATUS 등
  레거시 IMM 호출에 기본값/무응답을 주는 호환성 변화와 같은 맥락.)
  출처: https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-imminstallimew
- HKL=0 & err=0(성공처럼 보이나 HKL 미발급) = "조용한 no-op". E0200412 grayed,
  LoadKeyboardLayout 1419(HOTKEY_NOT_REGISTERED)도 같은 신경차단의 증상.
- **함의: ImmInstallIME 경로를 더 파는 것은 시간낭비.** Win11의 정식 등록 API는
  §2의 InstallLayoutOrTip 계열 + TSF RegisterProfile이다. Mozc·MS 한국어 모두 IMM32가
  아니라 TSF TIP로 출하된다.

---

## 6. UNIM → KakaoTalk 정공 경로 (실행 계획)

### 6.1 핵심 통찰
UNIM TIP은 이미 KakaoTalk이 로드한 msctf.dll/CUAS 위에서 동작 가능하다. 빠진 것은
**(a) 사용자 입력목록 정식 등록**과 **(b) 올바른 substitute KLID**다. IMM32 .ime는 불필요.

### 6.2 권장 변경 (unim-tsf/src/register.rs / unim-windows.exe)
1. **InstallLayoutOrTip로 입력목록 정식 추가** (set_as_default 또는 첫 활성화 시, 사용자
   컨텍스트):
   ```rust
   // input.dll 동적 로드, GetProcAddress("InstallLayoutOrTip")
   let s = w!("0x0412:{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}{B2C3D4E5-F6A7-8901-BCDE-F12345678901}");
   InstallLayoutOrTip(s, 0);               // 입력목록 추가 + 현 세션 적용
   SetDefaultLayoutOrTip(s, 0x02 /*SDLOT_APPLYTOCURRENTTHREAD*/); // 한국어 기본 지정
   ```
   이게 HKCU CTF\Assemblies 항목의 CLSID/Enable 빈칸을 정상 채운다(§4.2 결함 해소).
2. **SubstituteLayout/hklSubstitute를 KLID로 교정.** register.rs의
   `set_reg_dword(..., SubstituteLayout, 0x0412)` → **0x04120412(HKL형) 또는 KLID
   0x00000412**로. RegisterProfile 경유 등록 시 hklSubstitute에 동일 값.
   (현 0x0412 단독은 §3.2 기준 의심 — 실측으로 GetProfile 반환 확인 필요.)
3. **레거시 IMM32 잔재 정리:** Preload의 E0200412, Substitutes의 E0200412 제거.
   Keyboard Layouts\E0200412 키도 제거(혼란·grayed 항목 원인).
4. **검증:** 변경 후 `EnumEnabledLayoutOrTip` 또는 HKCU Assemblies 재측정으로
   CLSID/Enable 채워짐 확인 → KakaoTalk에서 한영 토글·한글 조합 실측.

### 6.3 MS Learn / 소스 근거 URL
- InstallLayoutOrTip: https://learn.microsoft.com/en-us/windows/win32/tsf/installlayoutortip
- RegisterProfile: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile
- hklSubstitute RE(KLID): https://marc.durdin.net/2017/07/substituted-layouts-in-text-services-framework/
- Mozc imm_util.cc(InstallLayoutOrTip+SetDefaultLayoutOrTip 시퀀스): https://github.com/google/mozc/blob/master/src/win32/base/imm_util.cc
- Mozc input_dll.h(시그니처+ILOT_/SDLOT_ 플래그): https://github.com/google/mozc/blob/master/src/win32/base/input_dll.h
- ImmInstallIMEW(레거시): https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-imminstallimew

---

## 7. 미해결 / 후속 검증 필요
1. KakaoTalk이 ImmDisableTextFrameService로 스레드 opt-out하는지 1차 확인(현재는 "TIF/
   CoreMessaging 로드 정황 + MS 한국어 동작"의 정황 증거뿐). 확정하려면 KakaoTalk 입력
   스레드의 CUAS 상태 직접 측정 필요.
2. SubstituteLayout 정확값(0x0412 vs 0x04120412 vs KLID 0x00000412) — RegisterProfile
   후 GetProfile.hklSubstitute 반환으로 실측 교정.
3. InstallLayoutOrTip 적용 후 KakaoTalk 실입력 회귀 테스트(이 문서는 등록 경로 규명까지).
