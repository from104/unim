# RE: 카톡 한글 입력 로컬 실측 재검증 (A1 임무)

날짜: 2026-06-22 / Windows 11 26200 / 도구: Sysinternals Listdlls64 v3.2, reg.exe(64-bit), dumpbin(VS2022 BuildTools)

## 결론 (이전 결론 전면 교정)

1. **카톡은 TSF를 우회하지 않는다.** 이전 "msctf 미로드 = TSF/CUAS 완전 우회"는 **tasklist false-negative였다.** Listdlls64로 카톡 PID 20892 모듈을 직접 읽으니 MSCTF.dll + IMM32.dll + textinputframework.dll **모두 로드 중**.
2. **카톡은 32-bit (SysWOW64) 프로세스다.** 로드된 IMM32/MSCTF/textinputframework 전부 `C:\WINDOWS\SysWOW64\` 경로. 실행파일도 `Program Files (x86)`.
3. **MS 한국어 IME는 100% TSF TIP다. IMM32 .ime를 전혀 안 쓴다.** `IME\IMEKR` 폴더에 `.ime` 파일 없음 — imkrtip.dll/imkrapi.dll/imkrotip.dll/imkrudt.dll(전부 TSF DLL)뿐. 시스템 전체 `.ime`는 `msctfime.ime`(OS CUAS 브리지)와 `unim_imm32.ime`(우리것)뿐.
4. **UNIM이 카톡에서 안 뜨는 근본원인 = unim_tsf.dll이 x64 전용 + 32-bit COM 등록 누락.**
   - `unim_tsf.dll` machine = 8664 (x64 only), 단일 파일.
   - `HKLM\SOFTWARE\WOW6432Node\Classes\CLSID\{A1B2C3D4...}` **키 자체가 없음** (InProcServer32 미등록).
   - 32-bit 프로세스(카톡)의 TSF는 32-bit COM 레지스트리(WOW6432Node\Classes\CLSID)에서 CLSID를 찾는데, UNIM 항목이 없어 인스턴스화 불가. 설령 있어도 x64 DLL이라 32-bit 프로세스에 in-proc 로드 불가.
   - 이래서 UNIM은 **64-bit TSF 앱(Edge/Chrome/WebView2/wezterm/메모장)에서만** 동작하고 카톡(32-bit)에서는 절대 안 됨.
5. **IMM32 .ime 갈래는 처음부터 잘못된 길이었다.** MS도 안 쓰는 메커니즘을 쫓았다. ImmInstallIME 실패/E0200412 1419 에러는 본질과 무관.

## 해결 방향 (다음 단계)
- `unim_tsf.dll`을 **i686(32-bit)로도 빌드**하여 `SysWOW64`(또는 Program Files 내 32-bit 폴더)에 배치.
- 32-bit DLL을 `HKLM\SOFTWARE\WOW6432Node\Classes\CLSID\{A1B2C3D4...}\InProcServer32`에 등록 (MS는 imkrtip.dll을 System32+SysWOW64 양쪽 + Classes\CLSID 양 뷰에 등록).
- WOW6432Node\CTF\TIP 프로필은 이미 존재하므로 32-bit Classes\CLSID InProcServer32 + Category만 채우면 됨. MSI가 32-bit regsvr32(SysWOW64\regsvr32.exe)로 DllRegisterServer를 한 번 더 호출하거나, WiX가 32-bit 컴포넌트로 등록.

## MS 한국어 IME 등록 footprint (레퍼런스)

CLSID `{A028AE76-01B1-46C2-99C4-ACD9858AE02F}`, Profile `{B5FE1F02-D5F2-4445-9C03-C568F23C99A1}`, LangID 0x0412.

### InProcServer32 — 양 아키텍처 모두 등록 (핵심!)
- 64-bit: `HKLM\SOFTWARE\Classes\CLSID\{a028ae76...}\InProcServer32` = `C:\Windows\System32\IME\IMEKR\imkrtip.dll` (Apartment)
- 32-bit: `HKLM\SOFTWARE\WOW6432Node\Classes\CLSID\{a028ae76...}\InProcServer32` = `C:\Windows\SysWOW64\IME\IMEKR\imkrtip.dll` (Apartment)
- 파일도 별개: System32\IME\IMEKR\imkrtip.dll (861688 B), SysWOW64\IME\IMEKR\imkrtip.dll (774648 B) — 각 아키텍처 빌드.

### LanguageProfile leaf (64-bit & WOW6432Node 동일)
- Description = "Microsoft IME"
- Display Description = `@%SystemRoot%\system32\input.dll,-5183`
- Enable = 0x0 (참고: 우리 UNIM은 0x1)
- IconFile = `C:\Windows\System32\IME\IMEKR\imkrtip.dll`

### Category (TIP가 키보드/IME로 보이게 하는 분류) — 9개
{046B8C80-...} {13A016DF-...} {25504FB4-...} {34745C63-...} {364215D9-...} {3AF314A2-...} {49D2F9CE-...} {49D2F9CF-...} {CCF05DD7-...}
(TF_CATEGORY_TIP_KEYBOARD 등 포함)

## Keyboard Layouts
- `00000412`: Layout File = KBDKOR.DLL, Text = Korean (일반 키보드 레이아웃 — IME 아님)
- `E0200412`: UNIM IMM32 수동 등록 잔재 (Ime File=unim_imm32.ime). MS 한국어용 E0xx0412 IMM32 엔트리는 없음 → MS는 IMM32 안 씀 재확인.

## UNIM 현 등록 상태 비교
- 64-bit CLSID InProcServer32: O (`C:\Program Files\UNIM\unim_tsf.dll`)
- **32-bit (WOW6432Node) CLSID InProcServer32: X (키 없음)** ← 카톡 미동작 원인
- WOW6432Node\CTF\TIP\{A1B2C3D4}\LanguageProfile: O (프로필만 있고 백킹 COM 없음 = 무용)
- LanguageProfile leaf: Enable=0x1, SubstituteLayout=0x412, IconFile=unim_tsf.dll

## 관측 가능한 단서 (인라인 조합 여부)
- 카톡이 textinputframework.dll + MSCTF를 로드 → MS 한국어가 카톡에 도달하는 경로는 **TSF (CUAS 비활성, 순수 TSF TIP)**. 32-bit 앱이라도 SysWOW64 MSCTF가 32-bit imkrtip.dll을 in-proc 로드해 inline 조합 제공.
- msctfime.ime는 System32+SysWOW64 양쪽 존재 = IMM32-only 앱을 위한 TSF 브리지지만, MS 한국어 자체는 이를 거치지 않고 TIP로 직접 동작.
