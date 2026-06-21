# RE 패키지 분해 + 카톡 한글입력 근본원인 (제로베이스 재조사)

작성: 2026-06-22 / 머신: Win11 26200 / 인가된 상호운용(interoperability) RE.

## 결론 한 줄

**카톡은 표준 TSF/CUAS 경로를 그대로 쓴다. 우리 IMM32 .ime 갈래는 헛다리였다.**
카톡은 **32비트(WoW64) 프로세스**라 **32비트 TSF TIP**(WOW6432Node COM 등록 + x86 DLL)이
필요한데, UNIM은 **TSF DLL을 x64만 빌드·등록**한다. 그래서 64비트 앱(Edge/Chrome/메모장)에선
되고 32비트 카톡에선 TIP이 인스턴스화조차 못 한다. 해법 = **x86 unim_tsf.dll 추가 빌드 +
WOW6432Node에 32비트 COM/TSF 등록**(weasel과 동일 패턴). IMM32 .ime는 불필요.

---

## 1. [재검증 — 이전 결론 뒤집힘] 카톡은 msctf/imm32를 로드한다

이전 "카톡은 msctf 미로드 = TSF 완전 우회"는 **tasklist의 false-negative**였다.
Sysinternals **ListDlls64**(권위 도구)로 실행 중 카톡(PID 20892) 모듈을 직접 읽음:

```
C:\WINDOWS\SysWOW64\USER32.dll
C:\WINDOWS\SysWOW64\win32u.dll
C:\WINDOWS\SysWOW64\IMM32.dll      ← 로드됨
C:\WINDOWS\SysWOW64\MSCTF.dll      ← 로드됨
```

- 모듈 135개 중 111개가 **SysWOW64** 경로 → **카톡은 32비트 앱**.
- MSCTF.dll + IMM32.dll 둘 다 로드 → 카톡은 OS 표준 TSF/CUAS 입력 스택을 쓴다.
- 즉 "MS 한국어가 카톡에 도달하는 경로" = 일반 TSF/CUAS bridge. UNIM도 **같은 길**로 가면 됨.
- 증거: `_re_work/kakao_listdlls.txt`(전체 135개), `_re_work/kakao_tsf_evidence.txt`.

## 2. [근본원인] UNIM TSF는 x64 단독 — 32비트 뷰/DLL 부재

현재 머신 레지스트리(설치된 UNIM v0.3.x):

| 키 | 64비트 뷰 | 32비트 뷰(WOW6432Node) |
|----|-----------|------------------------|
| `Classes\CLSID\{A1B2…7890}\InprocServer32` | **있음** → `C:\Program Files\UNIM\unim_tsf.dll` | **없음** |
| `Microsoft\CTF\TIP\{A1B2…7890}` | 있음(빈 부모 키) | 있음(빈 부모 키) |
| `…\CTF\TIP\{CLSID}\LanguageProfile\0x412\{B2C3…}` | **없음** (!) | 없음 |

- 설치된 `C:\Program Files\UNIM\unim_tsf.dll` = `dumpbin -headers` → **8664 machine (x64)**. 32비트 DLL 없음.
- WiX(`installer/wix/unim.wxs`) 확인: `unim_tsf.dll` 컴포넌트는 **`Win64="yes"` 단독**(line 65-68).
  `WIN_OUT_DIR32`(i686)는 **IMM32 .ime에만** 쓰이고 **TSF에는 안 쓰임**. → 32비트 TSF 컴포넌트 자체가 없음.
- 결과:
  - 64비트 앱: 64비트 msctf가 64비트 뷰의 CLSID→x64 DLL을 로드 → **동작**.
  - 32비트 카톡: 32비트 msctf가 **WOW6432Node**의 CLSID를 찾음 → **없음** + x86 DLL도 없음 → **TIP 미로드**.

> 부가 관찰: 64비트 뷰조차 `LanguageProfile\0x412\{Profile}` 키가 없다(reg query 비어 있음).
> register.rs는 이 키를 쓰게 돼 있으나 현 설치 상태엔 안 보임 — MSI static 블록/등록 타이밍 점검 필요.
> (단, 64비트 앱에서 동작한다는 기존 사실과 정합하려면 어딘가엔 프로필이 있어야 함. 활성 langbar
> Preload엔 `00000412`+`E0200412`만 있음 → UNIM TSF는 langbar Preload가 아니라 TIP 경로로 뜨는 구조.)

## 3. [A3 역공학] weasel 설치본 — "작동하는 IME"가 실제로 하는 일

- 받은 것: `weasel-0.17.4.0-installer.exe`(12MB, NSIS). 최신 7-Zip(26.01/16.04)은 NSIS 플러그인을
  제거해 추출 불가(`Cannot open as archive`). → **권위 소스인 GitHub 설치 로직**(`WeaselSetup/imesetup.cpp`)
  을 받아 분석(NSIS는 단순 런처, 실제 등록은 이 코드가 수행). 파일: `_re_work/imesetup.cpp`.

### weasel가 쓰는 등록 메커니즘 (IMM32 전혀 안 씀)

소스 주석: `// register_ime (IMM/.ime) support removed — TSF-only build`. **순수 TSF TIP**이다.

1. **DLL 양아키 설치 + regsvr32 양쪽**(`install_ime_file`, line 121-226):
   - 32비트 `weasel.dll` → System32(WoW64 리다이렉트로 SysWOW64) 복사 후 `regsvr32 /s`.
   - `is_wow64()`(=64비트 OS)면 **Wow64DisableWow64FsRedirection** 후 `weaselx64.dll` → 진짜
     System32 복사 후 **64비트 regsvr32 /s**. (ARM64면 ARM64X 리다이렉터까지 3종.)
   - 즉 **x86 + x64 두 DLL을 각자 아키텍처 regsvr32로 등록** → 양쪽 레지스트리 뷰에 COM/TSF 박힘.
2. **regsvr32 = DllRegisterServer**(line 302-359). 환경변수 `TEXTSERVICE_PROFILE`로 hans/hant 분기.
3. **input.dll!InstallLayoutOrTip**(line 393-409) — OS 공식 TIP 등록 API. 인자 문자열 포맷이 핵심:
   ```
   "0804:{A3F4CDED-...-CB0A}{3D02CAB6-...-9467}"
    └LANGID┘└─── CLSID ───┘└── Profile GUID ──┘   (CLSID·GUID 사이 구분자 없음)
   ```
   - `#define PSZTITLE_HANS L"0804:{CLSID}{PROFILE}"`. 제거 시 `ILOT_UNINSTALL(0x1)` 플래그.
4. **ITfInputProcessorProfiles::EnableLanguageProfile + EnableLanguageProfileByDefault**
   (`enable_profile`, line 277-299) — 프로필 활성화 + 기본 지정. (HKCU/세션 컨텍스트.)
5. 부수: `HKLM\SOFTWARE\Rime\Weasel` 에 WeaselRoot/ServerExecutable, WER LocalDumps 키.

> weasel GUID(참고): CLSID `{A3F4CDED-B1E9-41EE-9CA6-7B4D0DE6CB0A}`, Profile
> `{3D02CAB6-2B8E-4781-BA20-1C9267529467}`, LangID 0804(简)/0404(繁).

## 4. UNIM에 적용할 처방 (우선순위)

1. **x86 `unim_tsf.dll` 빌드**: `cargo build -p unim-tsf --target i686-pc-windows-msvc --release`.
   (i686 toolchain 이미 설치됨.)
2. **WiX에 32비트 TSF 컴포넌트 추가**(`Win64="no"`):
   - x86 DLL을 SysWOW64(=`SystemFolder`)에 배치.
   - `HKCR\WOW6432Node\CLSID\{CLSID}\InProcServer32` = x86 DLL 경로, ThreadingModel=Apartment.
   - `WOW6432Node\Microsoft\CTF\TIP\{CLSID}\…` Category/LanguageProfile 8종(64비트 블록을 32비트 뷰로 미러).
   - (Win64="no" 컴포넌트는 MSI가 자동으로 WOW6432Node로 리다이렉트하므로, 키 경로는 64비트와 동일하게 쓰되 컴포넌트 플래그로 제어.)
3. **`InstallLayoutOrTip` 호출 추가**(설치 후, 사용자 컨텍스트): weasel처럼
   `"0412:{A1B2…7890}{B2C3…8901}"` 포맷으로 `input.dll!InstallLayoutOrTip(psz, 0)`.
   현재 UNIM register.rs는 이 API를 안 쓰고 레지스트리 직기록만 함 → 32비트 등록 누락의 한 원인.
4. **IMM32 .ime 갈래 폐기**: 작동 IME(weasel)가 명시적으로 제거했고, 카톡은 TSF로 도달하므로 불필요.
   `ImmInstallIME` 死(HKL=0)·`LoadKeyboardLayoutW`=1419 추적은 더 안 해도 됨(잘못된 갈래).

## 5. 산출물

- `_re_work/weasel-0.17.4.0-installer.exe` — 원본 인스톨러.
- `_re_work/imesetup.cpp` — weasel 설치 로직(권위 소스, 분석 대상).
- `_re_work/kakao_listdlls.txt` — 카톡 전체 로드 모듈(ListDlls64).
- `_re_work/kakao_tsf_evidence.txt` — msctf/imm32 로드 증거 발췌.
- `_re_work/7zx`,`7z16`,`listdlls` — 분석 도구.
