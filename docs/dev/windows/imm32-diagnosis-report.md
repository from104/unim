# UNIM IMM32 미연결 종합 진단 보고서

- 대상: UNIM v0.3.24 (`feat/windows-msi-redesign`)
- 작성일: 2026-06-17
- 증거 원본: `docs/dev/windows/imm32-diagnosis-evidence.md`
- 검증: 본 보고서의 코드/레지스트리 주장은 `installer/wix/unim.wxs`, `unim-imm32/src/register.rs`, `scripts/build-msi.bat`를 직접 확인하여 교차검증함.

---

## 1. 한 줄 결론

KakaoTalk·한컴에서 UNIM이 전혀 작동하지 않는 **결정적 원인은 IME가 어떤 사용자 세션에도 활성 HKL로 로드된 적이 없다는 것**(`HKCU\Keyboard Layout\Preload`에 `E0200412` 부재)이며, 설치 코드(WiX·.ime SelfRegCost) 어디에도 사용자 활성화 단계가 없어 발생한다. 그 뒤를 잘못된 `Layout File=KBDA1.DLL`(아랍 자판)·미서명·TSF↔IMM32 미연동이 따른다.

---

## 2. 근본원인 랭킹 표

| 순위 | 심각도 | 원인 | 증거 | 수정 |
|---|---|---|---|---|
| **A** | **BLOCKER** | **사용자 활성 입력기 미등록.** KLID 행은 HKLM에 박히지만 `HKCU\Keyboard Layout\Preload`에 `E0200412`가 없어 HKL로 로드되지 않음 → 32비트 KakaoTalk가 `.ime`를 LoadLibrary할 계기 자체가 없음. | 실측 `Preload = {"1":"00000412"}`. `unim.wxs`에 HKCU Preload/Substitutes RegistryValue 0건. `.ime`에 `SelfRegCost` 미부여(TSF DLL만 line 69 `SelfRegCost="1"`) → 설치 경로에서 `ImmInstallIMEW` 한 번도 호출 안 됨. `register.rs:124`의 `ImmInstallIMEW`는 dev `regsvr32` 헬퍼에서만 호출. `%TEMP%\unim-imm32.log` 부재 = `DLL_PROCESS_ATTACH` 0회. | per-user 활성화 단계 추가: ① WiX `Impersonate="yes"` CustomAction으로 `HKCU\Keyboard Layout\Preload`에 다음 빈 인덱스 = `e0200412` 기록 + ② `LoadKeyboardLayoutW("E0200412", KLF_ACTIVATE\|KLF_SUBSTITUTE_OK)`로 즉시 로드. (4·5절 참조) |
| **B** | **HIGH** | **Keyboard Layouts 값 오류 — `Layout File=KBDA1.DLL`(Arabic 101).** 한국어는 `KBDKOR.DLL`이어야 함. Preload를 고쳐 로드돼도 baseline 스캔코드→문자 매핑이 아랍 자판으로 처리됨. | `unim.wxs:267, :290` `Value="KBDA1.DLL"`. `register.rs:80` `set_reg_value(..., "Layout File", "KBDA1.DLL")` (주석은 "Korean base layout"으로 오기). 웹 리서치: MS Korean `00000412`의 Layout File = `KBDKOR.DLL`(kbdlayout.info/KBDKOR). | `unim.wxs:267·290`, `register.rs:80` 모두 `KBDA1.DLL`→`KBDKOR.DLL`. drift 방지로 `globals.rs`에 `UNIM_IMM32_LAYOUT_FILE` 상수 추출. |
| **C** | **MEDIUM** | **WiX↔register.rs KLID 행 불일치 + 가짜 리소스 참조.** 설치본(WiX)은 `Layout Id=0412`, Display `...,-1`, Text="Korean Input Method (UNIM)". register.rs는 `Layout Id=00d2`, `...,-1000`, Text="UNIM Korean (IMM32)". `.ime`에 문자열 리소스 `-1`/`-1000`이 실제 빌드되는지 불명 → 빈 표시 이름 위험. 또한 `Layout Id=0412`는 langid와 충돌 소지. MS Korean은 Display Name을 `REG_EXPAND_SZ`로 쓰는데 UNIM은 `REG_SZ`. | `unim.wxs:268-270` vs `register.rs:84,93-94,100`. | (a) Text/Display/Id를 `globals.rs` 단일 상수로 통일. (b) `,-N` 리소스가 `.ime`에 없으면 평문 문자열 사용 또는 STRINGTABLE 추가. (c) `Layout Id`는 `0412` 대신 `00d2` 등 임의 free 4-hex로. (d) Display Name Type을 expandable로. |
| **D** | **MEDIUM** | **TSF↔IMM32 미연동 (CTF\Assemblies / Substitute HKL 부재).** "단일 언어바 항목으로 모던앱=TSF·레거시앱=IMM32 동시구동"하는 Mozc식 브리지 코드 0건. | 실측 `HKLM\SOFTWARE\Microsoft\CTF\Assemblies` 키 자체 부재, `Substitutes` 비어 있음. install-path에 Assemblies/Substitutes 기록 코드 없음(진단 .ps1에만 존재). | **KakaoTalk 단독 동작에는 불필요** — IMM32 폴백은 Preload만으로 완결. 완전한 듀얼모드를 원할 때만 6절 경로로 추가. |
| **E** | **LOW** | **`.ime`/`.dll`/`.msi` 미서명.** Win10/11 데스크톱 일반 Win32 앱(KakaoTalk 포함)은 미서명 `.ime`도 통상 로드되므로 1차 차단요인 아님. UWP/보호프로세스·일부 AV/기업정책에서만 거부 위험. | `build-msi.bat`에 `signtool`/`sign`/`/fd` 0건. `.ime` PE Security Directory RVA=0 (`certutil -verify` → `CRYPT_E_ASN1_BADTAG`). | `build-msi.bat` candle 단계 앞에 `signtool sign /fd sha256 /tr <ts> /td sha256` 블록 추가(x64·x86 `.ime`, `unim_tsf.dll`, 최종 `.msi`). A 해결 후에도 로드 실패가 남으면 그때 우선 의심. |

> 합의 사항: 4개 조사 모두 **A(Preload 미등록)를 1차 blocker**, **B(KBDA1.DLL)를 2차**로 일치 지목. 서명은 4개 모두 "부차적 리스크"로 일치. Assemblies는 IMM32 단독 경로에는 불필요라는 데 일치.

---

## 3. 권장 수정 순서

### 최소 경로 (가장 적은 변경으로 KakaoTalk에 한글 입력)
1. **B 수정 (값 2곳):** `unim.wxs:267·290`, `register.rs:80` → `KBDKOR.DLL`.
2. **C 일부:** `Layout Id`를 `00d2`로 통일(`unim.wxs:270·293`), Display Name 평문화 또는 expandable화로 빈 이름 방지.
3. **A 수정 (핵심):** WiX에 per-user CustomAction 추가 — `Preload`에 `e0200412` 기록 + `LoadKeyboardLayoutW` 활성화. (방법은 5절)
4. 재로그인 또는 활성화 API 효과로 KakaoTalk 재실행 → 한영 토글 후 입력 확인.

이 3단계만으로 KakaoTalk/한컴(IMM32 폴백)에서 동작해야 한다. **D(Assemblies)·E(서명)는 이 경로에 불필요.**

### 완전 경로 (Mozc식 듀얼모드: 모던앱 TSF + 레거시앱 IMM32 단일 항목)
5. **D 추가:** 첫 실행 per-user 단계에서
   - `HKCU\Keyboard Layout\Substitutes`: `e0200412` → `00000412` 매핑(또는 역) — Preload는 substitute 키를 가리킴.
   - `HKCU\Software\Microsoft\CTF\Assemblies\0x00000412\{TIP_KEYBOARD GUID}\Default`에 UNIM TIP CLSID/Profile 기록 → OS가 TSF TIP과 IMM HKL을 "동일 입력 항목"으로 인식.
   - 현 `scripts/unim-set-default.ps1` 로직을 install-path 코드로 승격.
6. **E 추가:** 서명 파이프라인 + GitHub Actions secret(`SIGN_CERT`/`SIGN_PASS`).

---

## 4. 정확한 레지스트리 값 예시

### 4.1 HKLM Keyboard Layouts (교정 후)
```
[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412]
"Ime File"             = "unim_imm32.ime"                 ; (현행 유지) System32/SysWOW64 상대명
"Layout File"          = "KBDKOR.DLL"                     ; ★ KBDA1.DLL → KBDKOR.DLL
"Layout Text"          = "Korean Input Method (UNIM)"     ; WiX/register.rs 통일값
"Layout Display Name"  = "@%SystemRoot%\system32\unim_imm32.ime,-1"   ; REG_EXPAND_SZ + 리소스 -1 실재 보장
                          ; (리소스 미빌드 시) "Korean Input Method (UNIM)" REG_SZ 평문
"Layout Id"            = "00d2"                           ; ★ 0412(langid 충돌) → 임의 free 4-hex
```

### 4.2 HKCU Preload (A의 핵심 — 활성화)
```
[HKEY_CURRENT_USER\Keyboard Layout\Preload]
"1" = "00000412"     ; 기존 MS Korean
"2" = "e0200412"     ; ★ 추가 — KLID 소문자 8hex
```

### 4.3 HKCU Substitutes (D — 듀얼모드 시에만)
```
[HKEY_CURRENT_USER\Keyboard Layout\Substitutes]
"00000412" = "e0200412"
; 이때 Preload "1"이 "00000412"를 가리키면 OS가 substitute를 적용
```

### 4.4 CTF Assemblies (D — 듀얼모드 시에만)
```
[HKEY_CURRENT_USER\Software\Microsoft\CTF\Assemblies\0x00000412\{34745C63-B2F0-4784-8B67-5E12C8701A31}]
; {3474...} = TIP_KEYBOARD category GUID (표준 키보드 TIP 카테고리)
"Default"     = "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}"   ; UNIM TSF TIP CLSID
"Profile"     = "{B2C3D4E5-...-8901}"                       ; UNIM Profile GUID
"KeyboardLayout" = dword:E0200412                           ; HKL ↔ TIP 브리지
```

---

## 5. 활성화 구현 (A) — WiX CustomAction

기존 `LaunchPopupRenderer`(`unim.wxs:359`, `Impersonate="yes"`)와 동일 패턴 사용. perMachine MSI에서 HKCU 기록은 **`Impersonate="yes"` 필수**.

```xml
<CustomAction Id="ActivateImm32HKL" Script="vbscript"
              Execute="immediate" Impersonate="yes" Return="ignore">
  <![CDATA[
    Dim sh : Set sh = CreateObject("WScript.Shell")
    Dim i : i = 1
    On Error Resume Next
    Do While sh.RegRead("HKCU\Keyboard Layout\Preload\" & i) <> ""
      If LCase(sh.RegRead("HKCU\Keyboard Layout\Preload\" & i)) = "e0200412" Then WScript.Quit
      i = i + 1
    Loop
    sh.RegWrite "HKCU\Keyboard Layout\Preload\" & i, "E0200412", "REG_SZ"
  ]]>
</CustomAction>
<InstallExecuteSequence>
  <Custom Action="ActivateImm32HKL" After="InstallFinalize">
    <![CDATA[NOT Installed OR REINSTALL]]>
  </Custom>
</InstallExecuteSequence>
```

권장 보강: 재로그인 없이 즉시 활성화하려면 로그인 사용자 프로세스(`unim-windows.exe`) 기동 시 `LoadKeyboardLayoutW("E0200412", KLF_ACTIVATE | KLF_SUBSTITUTE_OK)` 호출(메시지 브로드캐스트 포함). `ImmInstallIMEW`는 고정 KLID를 보장하지 못하므로 **활성화 수단으로는 `LoadKeyboardLayoutW` 사용**, `ImmInstallIMEW`는 캐시 갱신 보조로만.

---

## 6. 검증 절차

설치 후 (재로그인 1회) 다음을 확인:
```cmd
:: 1. Preload에 E0200412 들어갔는지
reg query "HKCU\Keyboard Layout\Preload"
   ; 기대: 추가 인덱스 = E0200412

:: 2. Layout File 교정 확인
reg query "HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412" /v "Layout File"
   ; 기대: KBDKOR.DLL

:: 3. (듀얼모드 시) Assemblies 매핑
reg query "HKCU\Software\Microsoft\CTF\Assemblies\0x00000412" /s
```
- **로드 검증:** KakaoTalk 재실행 → `%TEMP%\unim-imm32.log` 생성 여부 확인(생성 = `DLL_PROCESS_ATTACH` 성공). 현재는 파일 부재 = 0회 로드.
- **타이핑 검증:** KakaoTalk 입력란에서 한영 토글 후 `gksrmf` → "한글" 조합 확인. 한컴 한글에서도 동일.
- **언어바:** 작업표시줄 입력 표시기에 "Korean Input Method (UNIM)" 항목 노출 확인.
- **회귀:** i686 `.ime`에 `dumpbin /exports`로 `ImeProcessKey`·`ImeToAsciiEx`·`ImeInquire`가 `@N` 데코 없이 노출되는지 CI 검증(IMM-5: 현재 `.def`가 undecorated 강제하므로 정상이나 회귀 방지).

---

## 7. 미해결 / 추가 확인 필요

1. **`.ime` 문자열 리소스 실재 여부 (C 핵심 미확정):** `unim-imm32` 빌드에 `.rc`/리소스 컴파일 흔적이 없어 `Layout Display Name`의 `-1`/`-1000` 리소스가 실제 PE에 박히는지 미확인. 박히지 않으면 표시 이름이 빈 문자열이 됨 → 평문 문자열 권장.
2. **`LoadKeyboardLayoutW` 즉시 활성화의 영속성:** 세션/스레드 한정이라 Preload 기록(영속)과 병행 필요. 둘의 상호작용을 실측 검증 필요.
3. **Assemblies TIP_KEYBOARD GUID 정확값:** 4.4의 `{3474...}`는 표준 카테고리 추정값 — 듀얼모드 구현 시 `unim-set-default.ps1`의 실제 사용 GUID로 대조 필요.
4. **한컴(한글) IMM32/TSF 사용 모드 미확정:** KakaoTalk는 TSF opt-out 확정이나, 한컴은 버전별로 TSF/IMM 사용이 다를 수 있어 별도 실측 필요.
5. **서명 인증서 조달:** EV/OV 인증서 미보유 상태 — E 적용 전 조달 필요.

---

## 8. 적대적 검증 (Verify) 결과 — **CONFIRMED, confidence: high**

opus 검증 에이전트가 reg query를 재실행해 최상위 근본원인을 반증 시도한 결과:

- **A(Preload 미등록) = 결정적 확정.** 재실측 `Preload = {1:00000412}`만 존재, `E0200412` 부재. 리포지토리 어디에도 HKCU Preload 기록/`KLF_ACTIVATE` 코드 없음(`unim-probe-switch.ps1`은 *읽기*만). `Impersonate="yes"` CustomAction은 `LaunchPopupRenderer`(팝업 exe 실행)뿐, HKL 활성화 아님. `register.rs`의 `ImmInstallIMEW`는 dev register 경로 한정. → `.ime`가 활성 HKL이 될 경로 자체가 없음 → `%TEMP%\unim-imm32.log` 부재(재확인) = `DLL_PROCESS_ATTACH` 0회.
- **B(KBDA1.DLL) = 확정.** MS Korean `00000412`의 Layout File = `KBDKOR.DLL`(reg query 재확인). UNIM은 `unim.wxs:267·290`·`register.rs:80`에서 `KBDA1.DLL`(Arabic 101) — 명백한 오류값.
- **C는 LOW로 강등(보정).** Layout Id/Display Name 불일치는 **IME 로드를 막지 않는다**(레이아웃/표시 메타데이터일 뿐). 또한 `0412/00d2` drift는 "둘 다 통일 필요"가 아니라 **WiX(0412) vs register.rs(00d2 이미 기록)** 의 불일치 — 코드는 이미 `00d2`. 표 2의 MEDIUM → **LOW 취급**.
- **D(Assemblies)·E(서명) = 확정·올바르게 후순위.** Assemblies 키 부재·Substitutes 공백 재확인. 미서명은 데스크톱 IMM32에서 **통상 로드를 막지 않음** → 1차 차단요인 아님(LOW 유지).

### 검증이 지적한 잔여 리스크 (최소 경로를 "보장"으로 제시하지 말 것)
1. **B를 A보다 먼저/함께 고쳐야 함** — Layout File이 아랍 자판인 HKL을 활성화하면 base 레이아웃이 오매핑됨. 활성화만으로는 한글이 안 들어갈 수 있음.
2. **Display Name 리소스(`,-1`/`,-1000`)가 `.ime`에 실재하는지 미확정** → 없으면 빈 언어바 항목. 평문/`REG_EXPAND_SZ` 권장(미해결 #1과 동일).
3. **활성화 수정 후 최우선 의심 순위 = (1) KBDKOR.DLL (2) 표시 리소스**, 서명 아님.

### 하드 수용 게이트 (수정 적용 후 반드시 통과)
```
1) reg query "HKCU\Keyboard Layout\Preload"          → E0200412 존재
2) 32비트 KakaoTalk 실행 → %TEMP%\unim-imm32.log 에 DLL_PROCESS_ATTACH 라인 생성
3) KakaoTalk 입력란: 한영 토글 후 gksrmf → "한글" 조합 확정
```
세 줄 모두 통과해야 "해결"로 판정.
