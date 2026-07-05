# UNIM × KakaoTalk 한글 입력 — 최종 규명 및 해결 (SOLVED · CANONICAL)

상태: **SOLVED (2026-06-22)** · 브랜치 feat/windows-msi-redesign
종합 출처: A1 로컬 실측 / A2 오픈소스 / A3 패키지RE / A4 API 4갈래 정찰 + 실증 빌드/등록
정합성: 이 문서가 docs/dev/windows의 **단일 캐논(진실의 원천)**이다. 다른 IMM32 관련 문서는
모두 이 결론으로 정정되었거나 _archive로 이동되었다.

---

## 0. 한 줄 결론 (확정)

> **카톡은 TSF를 우회하지 않는다. UNIM이 안 뜬 진짜 이유는 카톡이 32-bit 프로세스인데
> UNIM TSF TIP(`unim_tsf.dll`)이 x64 단독으로만 빌드·등록돼 있어 32-bit COM 등록이
> 통째로 없었기 때문이다.**
> IMM32 `.ime` 갈래는 처음부터 틀린 길(Win11에서 MS 한국어도 안 쓰고, 작동하는 오픈소스
> IME들도 전부 제거함) → **폐기 확정.**
> **해결책(실증됨) = `unim_tsf.dll`을 i686로도 빌드해 32-bit COM/TSF로 양면 등록.**

### ✅ 실증 결과 (SOLVED)

i686 `unim_tsf.dll`을 빌드해 `SysWOW64\regsvr32.exe`로 32-bit COM 등록
(`WOW6432Node\Classes\CLSID\{CLSID}\InProcServer32`)하니 **카톡에서 한글 입력이 됐다**
(수동 등록으로 실측 확인). 4개 독립 정찰의 만장일치 진단·처방이 실측으로 확정됐다 (confidence HIGH).

**불변식:** 64-bit TSF 경로(Edge/Chrome/메모장/wezterm)는 정상 동작 — 깨지 말 것.
`fInterimChar` inline(`composition.rs:157`) 보존.

---

## 1. 카톡 입력경로 확정 (재검증 결과)

### 1-1. "카톡 msctf 미로드 = TSF 우회"는 FALSE NEGATIVE였다

이전 결론은 **측정 오류**였다. 원인 두 가지가 동시 확인됨:

- **카톡은 32-bit(WoW64) 프로세스다.** 실행파일이 `C:\Program Files (x86)\Kakao\KakaoTalk\KakaoTalk.exe`,
  PE machine = `0x014C`(x86). (A1·A2·A3·A4 모두 일치)
- 64-bit `tasklist` / .NET `Process.Modules`는 **WoW64 프로세스의 32-bit 모듈 목록을 구조적으로
  못 읽는다.** 그래서 ntdll/wow64 thunk 6~7개만 보였던 것(user32조차 안 보임) = 안티탬퍼가 아니라
  측정 도구의 한계.

**올바른 도구로 재측정한 결과 (PID 20892):**

| 도구 | 결과 |
|---|---|
| Sysinternals ListDlls64 (A1·A3) | 총 135개 모듈, 111개가 SysWOW64. `SysWOW64\MSCTF.dll` + `IMM32.dll` + `textinputframework.dll` **모두 로드** |
| 32-bit Toolhelp32 프로브 (A4) | 총 118개 모듈, `msctf.dll`/`imm32.dll`/`TextInputFramework.dll`/`CoreMessaging.dll`/`user32.dll` 모두 LOADED |

→ **카톡은 msctf.dll을 로드한다. TSF 인프라 위에서 동작한다.** "msctf 미로드" 결론 폐기.

### 1-2. MS 한국어가 카톡에 도달하는 경로 = 순수 TSF TIP → CUAS 브리지

- MS 한국어 IME는 **IMM32 `.ime`가 아니라 100% TSF TIP**다.
  - CLSID `{A028AE76-01B1-46C2-99C4-ACD9858AE02F}`, Profile `{B5FE1F02-D5F2-4445-9C03-C568F23C99A1}`,
    백킹 DLL = `imkrtip.dll`. (A1·A4 레지스트리 실측)
  - `IME\IMEKR` 폴더에 `.ime` 파일이 **없다**(imkrtip/imkrapi/imkrotip/imkrudt 전부 TSF DLL).
  - 시스템 전체 `.ime`는 `msctfime.ime`(OS CUAS 브리지) + `unim_imm32.ime`(우리 잔재) 둘뿐.
  - 한국어 Keyboard Layouts에 MS용 IMM32 엔트리(E0xx0412)는 **없다** → MS는 IMM32를 안 쓴다.
- **도달 메커니즘:** MS는 imkrtip.dll을 **양 아키텍처 모두 등록**한다.
  - 64-bit: `HKLM\...\Classes\CLSID` + `System32\IME\IMEKR\imkrtip.dll` (861688 B)
  - 32-bit: `HKLM\...\WOW6432Node\Classes\CLSID` + `SysWOW64\IME\IMEKR\imkrtip.dll` (774648 B)
  - 카톡(32-bit)의 `SysWOW64\MSCTF`가 WOW6432Node의 CLSID를 찾아 **32-bit imkrtip.dll을 in-proc
    로드** → inline 조합. CUAS(`msctfime.ime`)는 IMM32-only 앱용 안전망이며, MS 한국어 자체는
    직접 TIP로 동작한다.

**핵심 함의: UNIM TSF TIP도 정확히 같은 인프라를 쓸 수 있다. 별도 `.ime`는 필요 없다.**

---

## 2. 작동하는 등록·로드 메커니즘 (오픈소스/패키지RE 실증) vs 현 구현 격차

### 2-1. 작동하는 IME가 실제로 쓰는 것

| IME | 방식 | 1차 증거 |
|---|---|---|
| **weasel** (master) | 순수 TSF TIP. **IMM32 `.ime` 지원 명시적 제거**("register_ime (IMM/.ime) support removed — TSF-only build") | A2·A3 imesetup.cpp 정독 |
| **mozc** (master) | `ImmInstallIME` 안 씀. `InstallLayoutOrTip('0xLLLL:{clsid}{profile}',0)` + `SetDefaultLayoutOrTip` + `ITfInputProcessorProfileMgr::ActivateProfile`. broker32/broker64 분리 등록 | A2·A4 imm_util.cc / input_dll.h |
| **MS 한국어** | 순수 TSF TIP, 32/64 양면 COM 등록 | A1·A4 레지스트리 실측 |

**weasel 인스톨 메커니즘 (정설):**
1. x86 `weasel.dll`을 **SysWOW64**에 + x64 `weaselx64.dll`을 (FS 리다이렉션 해제 후) **진짜 System32**에 복사
2. 각각 **해당 아키텍처 regsvr32 `/s`** 로 등록 → 64-bit 뷰 + WOW6432Node 양쪽에 COM/TSF 키가 박힘
3. `input.dll!InstallLayoutOrTip(psz, 0)` 호출, `psz = "LANGID:{CLSID}{ProfileGUID}"` (구분자 없음, 예 `0804:{...}{...}`)
4. `ITfInputProcessorProfiles::EnableLanguageProfile` + `EnableLanguageProfileByDefault`로 활성화

### 2-2. UNIM 현 구현과의 격차 (★)

| 항목 | weasel/mozc/MS | UNIM 현재 | 격차 |
|---|---|---|---|
| **32-bit TSF DLL** | x86 + x64 양쪽 빌드 | **x64 단독** (`unim_tsf.dll` machine=8664). i686 빌드 디렉터리 부재 | ★ **치명** |
| **32-bit COM 등록** | WOW6432Node\Classes\CLSID\InProcServer32 | **없음**(64-bit 뷰만) | ★ **치명** — 카톡 32-bit msctf가 CLSID 못 찾음 |
| **TSF 프로필 활성** | InstallLayoutOrTip + SetDefaultLayoutOrTip | `SetDefaultLanguageProfile`+`ActivateProfile`만 (register.rs:180-190). InstallLayoutOrTip **누락** → HKCU CTF\Assemblies CLSID/Enable 빈칸 (A4 실측) | 중 |
| **IMM32 `.ime`** | 전부 제거 | unim_imm32.ime + KLID E0200412 + Substitutes/Assemblies 수작업 (activation.rs) | 헛다리 — 폐기 권장 |
| **SubstituteLayout** | RegisterProfile hklSubstitute=KLID | register.rs:126 `u32::from(langid)=0x0412` (langid 단독), wxs는 `1042` | 의심값(영향 경미) |

**결론:** UNIM이 64-bit 앱(Edge/Chrome/메모장)에서만 되고 카톡에서 안 되는 것은
**32-bit 등록이 통째로 없기 때문**이며, 그 외(카테고리 8종, LanguageProfile, COM 64-bit)는 정상이다.

---

## 3. 권장 구현 경로 (파일 단위)

### 방침: **TSF 32-bit 양면 등록으로 간다. IMM32 `.ime` 갈래는 폐기.**

근거: 카톡은 TSF를 로드한다(§1). MS·weasel·mozc 모두 TSF-only로 카톡류 32-bit 앱에 도달한다.
Win11에서 서드파티 IMM32 `.ime`는 1급 시민이 아니다(`ImmInstallIME` HKL=0 no-op,
`LoadKeyboardLayout` 1419, 항목 grayed — A1·A4). 더 단순하고 검증된 길이 있는데 막다른 길을
유지할 이유가 없다.

### 3-1. 빌드 — unim_tsf.dll을 i686로도 산출 (★ 최우선) — ✅ 완료

- **타깃 확인 완료:** `i686-pc-windows-msvc` + `x86_64-pc-windows-msvc` 둘 다 설치돼 있음.
- `unim-tsf`는 `crate-type=["cdylib"]`, deps = `unim`(엔진) + `unim-windows-common` + windows-rs 0.62.
  windows-rs는 i686 완전 지원. 엔진(`unim`)에 x64 전용 의존 없음(검색상 `target_arch=x86_64` 가드 0건).
- 빌드 커맨드:
  ```
  cargo build --release -p unim-tsf --target i686-pc-windows-msvc
  cargo build --release -p unim-tsf --target x86_64-pc-windows-msvc
  ```
- **✅ 실증:** i686 빌드 성공. `text_service.rs`/`settings_dialog.rs`/`preedit_window.rs`의
  `SetWindowLongPtrW` 반환값을 `as isize`→`as _`로 고친 **3곳** 수정으로 i686에서도 컴파일됨.
  COM export(`DllGetClassObject`/`DllRegisterServer` 등)가 undecorated로 정상 노출되는 것 확인.
  `target/i686-pc-windows-msvc/release/unim_tsf.dll` 산출.

### 3-2. `installer/wix/unim.wxs` — 32-bit TSF 컴포넌트 추가

현재 `UnimTsfDll` 컴포넌트(line 63-180)는 `Win64="yes"` 단독. 32-bit 미러를 추가한다.

1. **파일 배치:** x86 `unim_tsf.dll`을 32-bit 시스템 경로(`SystemFolder` = SysWOW64) 또는
   별도 `INSTALLDIR\x86\` 서브폴더에 설치.
   - weasel 방식(SysWOW64 직접)도 가능하나, UNIM은 perMachine `Program Files\UNIM`을 쓰므로
     `INSTALLDIR\unim_tsf32.dll` 형태로 두고 InProcServer32가 그 절대경로를 가리키게 하는 편이
     FS 리다이렉션 함정을 피한다.
2. **새 컴포넌트 `UnimTsfDll32`** (`Win64="no"`):
   - `<File>` Source = `$(var.WIN_OUT_DIR32)\unim_tsf.dll` (WIN_OUT_DIR32는 이미 정의됨, 현재 .ime에만 쓰임).
   - **32-bit 레지스트리 뷰**(`Win64="no"` 컴포넌트의 HKCR/HKLM = WOW6432Node)에 §63-180과 동일한
     키 전부 미러: `CLSID\{CLSID}\InProcServer32` + `CTF\TIP\{CLSID}` + LanguageProfile 6값 +
     Category 8종. WiX가 `Win64="no"`이면 자동으로 WOW6432Node에 박는다.
   - `SelfRegCost`는 32-bit DLL에 붙이되, **MSI self-reg는 32-bit DllRegisterServer를 64-bit
     msiexec가 호출할 때 아키텍처 문제 소지** → 안전하게는 정적 RegistryKey만 쓰고 SelfReg 생략하거나,
     `SysWOW64\regsvr32.exe`를 CustomAction으로 명시 호출.
3. `<Feature>`에 `<ComponentRef Id="UnimTsfDll32" />` 추가.
4. **WIN_OUT_DIR32 빌드 산출 보장:** CI/로컬 빌드 스크립트가 i686 unim_tsf.dll을 먼저 빌드하도록.

### 3-3. `unim-tsf/src/register.rs` — InstallLayoutOrTip 추가 + 양면 self-reg 대응

`register_server()`는 DLL이 로드된 아키텍처의 HKCR(=해당 뷰)에 쓰므로 32-bit DLL의
`DllRegisterServer`가 호출되면 자동으로 WOW6432Node에 박힌다 — **코드는 아키텍처 중립이라 그대로 OK.**

`set_as_default()` 보강 (A4 격차 #3):
- 현재 `SetDefaultLanguageProfile` + `ActivateProfile`만 호출 → HKCU `CTF\Assemblies`의
  CLSID/Enable이 빈칸으로 남아 정식 입력목록 추가가 미완.
- **`input.dll!InstallLayoutOrTip("0x0412:{CLSID}{Profile}", 0)` 추가 호출**로 OS 정식 등록을
  완성한다. input.dll은 import lib 없음 → `LoadLibrary("input.dll")` + `GetProcAddress`로 동적 호출.
  - psz 포맷 주의: CLSID와 Profile 사이 **구분자 없음**.
  - 이어서 `SetDefaultLayoutOrTip(psz, SDLOT_APPLYTOCURRENTTHREAD)` 권장.
- 이 경로는 mozc/weasel 정설이며 과거 `AddLanguageProfile` 직접호출이 낸 msctf 0x97e5a NULL
  deref를 회피할 가능성이 높다(unknown #4 — 실측 확인).
- `SubstituteLayout` 값: register.rs:126 `0x0412` vs wxs `1042(=0x412)`는 사실 동일값.
  RegisterProfile 경로로 가면 hklSubstitute=KLID `0x00000412`로 OS가 자동 생성(Durdin RE).
  **수작업 Substitutes/Assemblies(activation.rs) 박기는 InstallLayoutOrTip이 대체하므로 제거 가능.**

### 3-4. IMM32 `.ime` 갈래 정리 (폐기)

- **빼는 것:** wxs `UnimImm32Ime64`/`UnimImm32Ime32` 컴포넌트(line 252-304) + Feature ref(345-346),
  `unim-windows-common/src/activation.rs`의 Preload/Substitutes/Assemblies 수작업,
  `unim-popup-win` `ensure_imm32_active()` 호출(main.rs:106) + `--deactivate-imm32` CA(wxs 381-394).
- **단, 단계적으로:** 32-bit TSF 경로가 카톡에서 실측 검증되기 전까지 IMM32 잔재를 즉시 삭제하지 말 것.
  먼저 32-bit TSF를 추가→검증→그다음 IMM32 컴포넌트 제거(롤백 안전망 유지).
- `unim-imm32` 크레이트 자체는 남겨도 빌드만 안 하면 무해.

### 3-5. popup-win IPC 단일 인스턴스 (검토)

32-bit + 64-bit unim_tsf.dll이 동일 CLSID를 공유하면 양쪽 모두 `unim-popup-win.exe`로 IPC.
렌더러는 싱글턴 뮤텍스가 있어 중복 기동은 무해하나, **32-bit 카톡 프로세스의 TSF STA와 64-bit
앱의 TSF STA가 같은 named pipe에 동시 접속**할 때 경합 여부 점검 필요(unknown #4). 현 wire는
양쪽 동일 사본이므로 프로토콜 호환은 보장됨.

---

## 4. 확신도 · 잔여 작업

**확신도: HIGH (진단·처방 모두)** — 4개 독립 정찰 만장일치 + **실증 완료**.
1차 증거(ListDlls64 + 32-bit Toolhelp32 + 레지스트리 실측 + weasel 소스)로 진단 교차검증했고,
**i686 빌드 + `SysWOW64\regsvr32` 32-bit 등록 → 카톡 한글 입력 성공**으로 처방까지 확정됐다.

### ✅ 해소된 unknown
1. ~~i686 unim_tsf.dll 빌드가 deps 포함 깨끗이 되는지~~ → **해소: 빌드 성공**(SetWindowLongPtrW 3곳 수정).
2. ~~32-bit TSF 등록 후 카톡에서 한글 inline 조합~~ → **해소: 수동 등록으로 카톡 한글 입력 실증**.

### 남은 잔여 작업 (실증→영구 배선)
1. **wxs 영구 배선:** `unim.wxs`에 32-bit TSF 컴포넌트(`Win64="no"`, WOW6432Node\Classes\CLSID
   \InProcServer32 등)를 추가해 MSI 설치만으로 32-bit 등록이 되게 한다. 빌드 스크립트가 i686
   `unim_tsf.dll`을 먼저 산출하도록 보장. (현재는 수동 regsvr32로만 실증된 상태.)
2. **IMM32 잔재 제거:** 32-bit TSF가 MSI 경로로도 검증되면 `.ime` 컴포넌트/activation.rs
   수작업/`ensure_imm32_active()`/`--deactivate-imm32` CA를 제거(§3-4). 롤백 안전망으로 단계적 진행.
3. (선택) **InstallLayoutOrTip 추가**(§3-3): HKCU `CTF\Assemblies` 정식 등록 완성. 정적 레지스트리와
   충돌/과거 msctf 0x97e5a NULL deref 회피 여부 실측 권장.

### 잔여 미검증 (영향 경미)
- 32/64 동일 CLSID 공유 시 popup-win named-pipe IPC 경합 여부(현 wire 동일 사본이라 프로토콜 호환은 보장).
- KakaoTalk의 스레드 단위 `ImmDisableTextFrameService` opt-out 여부(TIP 실동작 증거로 정황상 안 함).

> **코드/wxs/bat은 이 문서 개정 범위 밖이다.** 위 잔여 작업은 별도 구현 태스크로 처리한다.

---

## 부록: 산출물 경로

- A1: `docs/dev/windows/re-local-mskorean.md`, `C:/tmp/kakao_dlls.txt`, `C:/tmp/kakao_summary.txt`
- A2: `docs/dev/windows/re-opensource-imes.md`
- A3: `docs/dev/windows/re-package-teardown.md`, `_re_work/imesetup.cpp`, `_re_work/kakao_listdlls.txt`, `_re_work/kakao_tsf_evidence.txt`
- A4: `docs/dev/windows/re-registration-apis.md`, `unim-windows-common/examples/proc_modules.rs`
