> ## ⛔ 결론 정정 (SUPERSEDED · 2026-06-22)
> **전제가 틀렸다: 카톡은 "TSF opt-out"이 아니다.** 카톡(32-bit)은 `SysWOW64\MSCTF.dll`을
> 정상 로드하며 TSF 위에서 동작한다(ListDlls64 + 32-bit Toolhelp32 실측). UNIM이 안 뜬 진짜
> 원인은 IMM32 로드 실패가 아니라 **`unim_tsf.dll`이 x64 단독이라 32-bit COM 등록이 없었던 것**.
> 해결 = i686 `unim_tsf.dll` 빌드 + 32-bit TSF 등록(`WOW6432Node\Classes\CLSID\InProcServer32`).
> **IMM32 `.ime` 재구현 명세(ImmInstallIMEW 정공법 등)는 추진하지 않는다 — 갈래 자체가 헛다리.**
> 최종 진실: **[imm32-win11-SOLUTION.md](imm32-win11-SOLUTION.md)**.
> 단, 이 문서의 IMM32 API 동작 관찰(1419/HKL=0 no-op 등)은 "Win11에서 서드파티 IMM32가 1급
> 시민이 아님"의 근거로 유효하다.

# IMM32 `.ime` 로드 실패 근본 규명 + 재구현 명세

> 대상: UNIM `unim_imm32.ime`(KLID `E0200412`)가 KakaoTalk(32bit, TSF opt-out)에서
> 한 번도 로드되지 않는 문제.
> 기준: 이 머신 실측(v0.3.26) + MS 1차 문서 + mozc 등록 절차.
> 표기: **확인됨**(1차 문서/실측 2+) / **정황**(2차·정황) / **추측**(소스 없음).

---

## TL;DR (A~D 핵심 결론)

- **A. 1419 원인:** `LoadKeyboardLayoutW("E0200412")` 가 1419 로 실패하는 건
  **KLID `E0200412` 가 "OS 가 인지하는 설치된 입력 로케일"이 아니기 때문**이다.
  우리는 `Keyboard Layouts\E0200412` 레지스트리 행만 **수동으로** 박았는데, IMM32 IME 의
  KLID 는 **OS(`ImmInstallIMEW`)가 high-word(device id)를 할당**하면서 win32k 내부
  레이아웃 테이블에 등록해야 비로소 로드 가능한 HKL 이 된다. 수동 레지스트리 행은
  **데이터일 뿐 등록 행위가 아니다.** 즉 정공법(ImmInstallIME)을 거치지 않았다.
  (보조 가설: `Layout Id`/`Layout File` 부정합도 로드 거부에 기여 — 아래 §1.)
- **B. 정공법:** `.ime` 를 System32/SysWOW64 에 복사 → **관리자 권한**으로
  **`ImmInstallIMEW(전체경로, 표시명)`** 호출 → OS 가 KLID(예 `E0xx0412`)를 **할당하고
  반환**. 그 **반환 HKL 의 KLID 문자열**을 읽어 HKCU `Preload` 에 기록. KLID 는
  **우리가 고정할 수 없다**(OS 가 빈 high-word 슬롯을 잡음). per-machine 등록(HKLM,
  관리자) + per-user 활성(HKCU Preload, 유저).
- **C. 바꿔야 할 것:**
  1. `register.rs`: 수동 `RegCreateKeyW(Keyboard Layouts\E0200412)` 를 **권위(authority)
     에서 강등**하고, `ImmInstallIMEW` 를 **권위로 승격**(반환 HKL 채택). `ime_file` 인자를
     **bare 파일명이 아니라 전체경로**로.
  2. `activation.rs`: 하드코딩 `E0200412` 로 `LoadKeyboardLayoutW` 호출 금지 →
     **ImmInstallIMEW 가 반환한 실제 KLID** 로 Preload/Load. **Substitutes(E0200412→
     00000412) 제거**(IMM32 .ime 로드를 적극 방해 — §5).
  3. `unim.wxs`: 정적 `Keyboard Layouts\E0200412` 블록은 **불충분**. 설치 시
     `unim-popup-win.exe --activate-imm32` 류 CA 에서 ImmInstallIME 를 타게 하거나,
     OS 가 부여한 KLID 를 받아 처리. 고정 KLID 가정 폐기.
- **D. 단일항목 통합:** **포기 권고(IMM32 우선).** Substitutes + Assemblies 미러링은
  IMM32 .ime 로드와 **상충**한다. 먼저 .ime 가 독립 KLID 로 로드/입력되게 만든 뒤,
  통합은 별도 과제로. (§5/§D)

---

## 0. 실측 사실 정리 (입력 증거)

| # | 사실 | 출처 |
|---|---|---|
| E1 | `tasklist /m unim_imm32.ime` = 0 프로세스(KakaoTalk 실행 중) | 브리핑 실측 |
| E2 | `%TEMP%\unim-imm32.log` 부재 = DLL_PROCESS_ATTACH 0회 | 브리핑 실측 |
| E3 | `LoadKeyboardLayoutW("E0200412", ACTIVATE\|SUBSTITUTE_OK)` → 1419 (substitute 쓰기 전에도) | 브리핑 실측 |
| E4 | `Keyboard Layouts\E0200412`: Ime File=unim_imm32.ime, **Layout File=KBDA1.DLL**(아랍! 과거 덤프), Layout Id=**0412**(과거 덤프) | imm32-diagnosis-evidence.md §3 |
| E4b | 현 코드/wxs 는 Layout File=**KBDKOR.DLL**, Layout Id=**00d2** 로 수정됨 | globals.rs:45/47, unim.wxs:268/274 |
| E5 | Preload=00000412 만(과거 덤프). 현 코드는 E0200412 추가 시도 | activation.rs:260 |
| E6 | `.ime` 미서명, x64=System32 / x86=SysWOW64 배치됨 | 브리핑 실측 |

> ⚠️ E4 의 KBDA1.DLL/Layout Id=0412 는 **과거 진단 시점 덤프**다. 현재 v0.3.26 머신의
> 실제 레지스트리 값은 재확인 필요(현 코드는 KBDKOR.DLL/00d2 를 쓰지만, 1차 설치가
> MSI 인지 수동 .bat 인지에 따라 실측값이 다를 수 있음). **이 차이가 1419 의 보조
> 원인일 수 있어 §1 에서 분리 분석.**

---

## A. 1419(ERROR_HOTKEY_NOT_REGISTERED) 정확한 원인

### A-1. 1차 원인 — KLID 가 "OS 가 등록한 입력 로케일"이 아님 (**확인됨**)

MS 1차 문서:

- `ImmInstallIMEW`: *"Installs an IME. … Returns the input locale identifier for the IME.
  This function is intended to be used by IME setup applications only."* — 즉 **IME 설치는
  ImmInstallIME 가 정공법**이고, 이 함수가 **KLID(input locale identifier)를 만들어
  반환**한다. [imm-imminstallimew]
- KLID 구조: *"each KLID is 32 bits … the bottom half is a LANGID, and the top half is
  something device-specific."* *"Layout IDs … manage the situation when multiple keyboards
  use the same LANGID. Each keyboard layout using the same LANGID after the first must
  (1) have one and (2) it must not duplicate one that is already assigned."*
  [terminology] [narkive-klid]
- `LoadKeyboardLayoutW`: pwszKLID 는 **이미 시스템에 설치돼 있는** 레이아웃의 이름이어야
  하며, 매칭 실패 시 *"the return value is the default language of the system"* 또는
  NULL. [winuser-loadkeyboardlayoutw]

핵심: IMM32 IME 의 high-word(`E0xx`)는 **임의로 정해 레지스트리에 박는 값이 아니라
OS 가 `ImmInstallIME` 시점에 "빈 device-id 슬롯"을 할당**하는 값이다. win32k 는 자기
내부 레이아웃 테이블(부팅 시 `Keyboard Layouts` 를 스캔해 구축 + ImmInstallIME 가
런타임 추가)에 그 KLID 가 있어야 `LoadKeyboardLayout` 으로 인스턴스화한다. 우리는
`RegCreateKeyW` 로 행만 박고 ImmInstallIME 를 "보조(실패 무시)"로 돌렸으므로
(register.rs:112-134), **OS 내부 테이블에 E0200412 가 유효 IME 로 등록된 적이 없다.**
→ `LoadKeyboardLayoutW` 가 "그런 hotkey/layout 없음"으로 거부 = **1419**.

> 왜 하필 1419(HOTKEY_NOT_REGISTERED)? IMM32 IME 의 input-locale 슬롯은 내부적으로
> 언어 hotkey(Ctrl+Shift 등 전환) 테이블과 한 자료구조에 묶인다. 미등록 KLID 를
> 활성화하려 하면 "등록 안 된 hotkey 슬롯" 에러로 떨어진다. confidence: **정황**
> (정확한 win32k 내부 매핑은 비공개; 그러나 "미등록 KLID → 1419" 자체는 커스텀
> 레이아웃/IME 개발자 보고에서 반복 관측). [answers-kbd-never-activated]

### A-2. 보조 원인 — `Layout File`/`Layout Id` 부정합 (**정황**)

- **2012 보안 업데이트**: 레이아웃 DLL 은 `%Windir%\System32` 에 있어야 하고 상대경로/
  앱 디렉터리 참조는 거부된다. `LoadKeyboardLayoutW` 문서도 *"This can occur if the
  layout library is loaded from the application directory."* → NULL/실패. [winuser-loadkeyboardlayoutw] [answers-kbd-2012]
- 과거 `Layout File=KBDA1.DLL`(아랍 자판) 은 잘못된 스캔코드 DLL. 현재 KBDKOR.DLL 로
  교정. **KBDKOR.DLL 자체는 System32 에 존재**하므로 이 부분은 이제 정상.
- 과거 `Layout Id=0412` 는 **langid(0x0412)와 동일** → "다른 레이아웃과 충돌 금지"
  규칙 위반 가능. 현재 `00d2` 로 교정(자유 식별자). 단 **Layout Id 는 OS 가
  high-word 와 매핑·관리하는 값**이라, ImmInstallIME 없이 우리가 임의로 박으면
  high-word 매핑이 성립하지 않는다(여전히 A-1 의 부분집합).

**결론(A):** 1419 의 **주원인은 A-1(ImmInstallIME 미경유 = OS 미등록)** 이다. A-2 는
교정됐거나 부차적. **수동 레지스트리 기록만으로는 절대 불충분.** (confidence: A-1
높음/확인됨, A-2 정황)

---

## B. 정확한 등록 + 로드 절차 (정공법)

mozc 가 하는 일 = *"copying IME files into the system, writing settings in the registry,
**and calling the ImmInstallIME function**"*, 그리고 **`mozc_broker --mode=register_ime`
는 관리자 권한**으로 실행. [build-mozc-windows] [imm-imminstallimew]

### B-1. per-machine 등록 (관리자 1회 — 설치 시)

1. `.ime` 를 `%Windir%\System32`(x64) + `%Windir%\SysWOW64`(x86) 에 복사. (이미 함)
2. **관리자 권한**으로 `ImmInstallIMEW(L"C:\\Windows\\System32\\unim_imm32.ime",
   L"UNIM Korean (IMM32)")` 호출.
   - 인자 (a): **KLID 가 아니라 `.ime` 전체경로 + 표시명.** [imm-imminstallimew]
   - (b): **KLID 는 OS 가 할당**한다. `E0200412` 고정 불가. high-word 는 빈 슬롯
     (`E020`, `E021`, … 0x0412 LANGID 그룹 내)을 OS 가 잡는다. **우리가 결정 못 함.**
   - (c): ImmInstallIME 가 **`Keyboard Layouts\<할당KLID>` 행을 자동 생성**한다(Ime
     File/Layout File/Layout Text 등). → **우리 수동 행과 중복/충돌**. 수동 행은 폐기.
   - (d): 반환 HKL → `(HKL as u32)` 의 하위 16비트 = LANGID, 상위 16비트 = device id.
     KLID 문자열 = `format!("{:08X}", hkl_u32)`. **이 문자열을 Preload/Substitutes/
     Assemblies 참조에 사용.**
   - (e): **per-machine + 관리자 필수**(HKLM Keyboard Layouts 쓰기). mozc 도 broker 를
     관리자로 실행. [build-mozc-windows]
3. (선택) ImmInstallIME 가 채우지 않는 값(`Layout Display Name` 인디렉트, 아이콘 리소스)
   만 **반환 KLID 키에** 보강.

> ⚠️ 32/64 양쪽: 32비트 앱(KakaoTalk)은 **SysWOW64 의 x86 .ime + WOW64 레지스트리
> 뷰**를 쓴다. ImmInstallIME 도 **x86 프로세스에서 한 번, x64 프로세스에서 한 번**
> (또는 OS 가 양쪽 뷰에 미러) 호출돼야 32/64 앱 모두 커버. mozc 가 broker32/broker64
> 를 분리 실행하는 이유. [build-mozc-windows] **(UNIM 은 x86 등록 경로가 비어 있을
> 가능성 — 검증 필요.)**

### B-2. per-user 활성 (유저 컨텍스트 — 로그인/설치 후)

4. B-1 반환 KLID 를 HKCU `Keyboard Layout\Preload` 에 다음 빈 인덱스로 기록. (현
   activation.rs 로직 재사용, **단 KLID 를 하드코딩 말고 받아온 값으로.**)
5. `LoadKeyboardLayoutW(받은_KLID, KLF_ACTIVATE|KLF_SUBSTITUTE_OK)` → 이제 1419 안 남.

### B-3. KLID 전략 요약

| 항목 | 정공법 |
|---|---|
| KLID 고정 가능? | **불가.** OS 할당. `E0200412` 를 우리가 못 고름. |
| E020 의미 | E0xx = IME(device id high word), 0412 = ko-KR LANGID. **E020 의 20 은 우리가 아니라 OS 가 정함.** |
| 권위 | `ImmInstallIMEW` 반환 HKL → KLID. 레지스트리 행은 그 부산물. |
| 권한 | per-machine 등록 = 관리자. per-user Preload = 유저. |

---

## C. 현재 구현 대비 변경안 (파일 단위)

### C-1. `unim-imm32/src/register.rs` (개발/regsvr32 경로)

- **삭제/강등:** §1 의 `RegCreateKeyW(Keyboard Layouts\E0200412)` + 5개 수동 값
  (register.rs:57-105). 이건 "권위"가 아니다. ImmInstallIME 가 만든다.
- **승격:** `ImmInstallIMEW` 를 "보조·실패무시"(register.rs:112-134)에서 **주
  등록 경로**로. **인자를 bare `unim_imm32.ime` → 전체경로**(`get_dll_path()` 결과)로 교체.
  반환 HKL 을 로그+저장(다음 단계에서 Preload 에 쓸 KLID 도출).
- `globals.rs::UNIM_IMM32_KLID = "E0200412"` 상수: **"고정 KLID 가정" 폐기.** 런타임에
  ImmInstallIME 반환값에서 도출. (상수는 fallback 표시용으로만 남기거나 제거.)

### C-2. `unim-windows-common/src/activation.rs`

- `const UNIM_IMM32_KLID = "E0200412"` 하드코딩 →
  **ImmInstallIME 반환 KLID 를 받아 쓰는 시그니처**로 변경(예: `ensure_imm32_active(klid: &str)`).
  ImmInstallIME 가 popup-win 컨텍스트에서 호출된다면 거기서 반환값 전달.
- `LoadKeyboardLayoutW(하드코딩 KLID)` (activation.rs:161) → 받은 KLID.
- **Substitutes 제거:** `write_substitute_and_assembly()` 의 `upsert_substitute`
  (E0200412→00000412) **삭제.** §5 참조 — 이게 .ime 로드를 막는다.
- **Assemblies 미러링 보류:** `upsert_assembly`(TSF CLSID 미러)는 IMM32 .ime 로드와
  무관하거나 방해. D 권고대로 **분리/보류.**

### C-3. `installer/wix/unim.wxs`

- 정적 `Keyboard Layouts\E0200412` 블록(unim.wxs:262-275 x64, 289-302 x86)은
  **불충분 + 충돌**(ImmInstallIME 가 다른 KLID 를 만들면 고아 키가 됨). **제거 또는
  ImmInstallIME 경유로 대체.**
- **추가:** 설치 CA 에서 `unim-popup-win.exe --install-imm32`(가칭) 를 **deferred,
  no-impersonate, 관리자** 로 실행해 `ImmInstallIMEW`(x64) 호출. x86 .ime 등록은
  별도 32비트 헬퍼 또는 OS WOW64 미러에 의존(검증 필요).
- 언인스톨: `UnloadKeyboardLayout` + (ImmInstallIME 가 만든)KLID 키 정리.

### C-4. 서명 — **불필요(미확인은 아님)**

- **데스크톱 IMM32 .ime 는 미서명이어도 로드된다.** mozc 등 커스텀 IME 가 코드서명
  없이 동작해 온 전례. 1419 와 **무관**(서명 문제면 DLL_PROCESS_ATTACH 후 거부지
  LoadKeyboardLayout 단계 1419 가 아님). [build-mozc-windows]
- 단, MS 신규 정책 *"the system blocks IMEs that are implemented by using Input Method
  Manager (IMM32)"* 는 **UWP/앱컨테이너(Store/Windows 앱) 한정**이다. 데스크톱 레거시
  앱(KakaoTalk x86)은 여전히 CUAS 경유 IMM32 를 쓰며 `ImmInstallIME`/`.ime` 가 *"desktop
  apps only"* 로 현행 문서에 살아 있다. [ime-requirements] [imm-imminstallimew]
  → **KakaoTalk 타깃에는 IMM32 경로 유효.** (confidence: **확인됨** — 문서가 desktop
  apps 한정 차단임을 명시.)

---

## D. 단일항목 통합 — 권고: **포기(IMM32 우선)**

- 현 설계는 Substitutes(E0200412→00000412) + Assemblies\0x00000412\{TIP_KEYBOARD} 를
  UNIM TSF CLSID 로 미러해 "TSF TIP + IMM32 .ime 를 한 언어바 항목"으로 묶으려 함.
- 문제: **Substitutes 가 E0200412 선택을 베이스 한국어(00000412)로 치환**한다.
  KLF_SUBSTITUTE_OK 하에서 E0200412 를 로드하려 해도 00000412(MS 한국어, .ime 아님)가
  대신 로드 → **우리 .ime 는 영원히 로드 안 됨.** (실측 E3 의 1419 와 별개로, 설령
  KLID 가 유효해져도 substitute 가 .ime 를 가로챈다.) [winuser-loadkeyboardlayoutw
  KLF_SUBSTITUTE_OK 설명]
- 권고:
  1. **1단계(이 과제):** Substitutes/Assemblies 통합 전부 제거. .ime 를 **독립 KLID**
     로 ImmInstallIME 등록 → KakaoTalk 에서 .ime 가 로드/조합되는 것 먼저 확보.
  2. **2단계(후속, 선택):** 통합이 정말 필요하면 mozc/MS 한국어가 실제로 쓰는 방식
     (TSF TIP 와 IMM32 IME 를 같은 profile 로 묶는 정식 API)을 재조사. 현재의 수동
     Substitutes 미러는 .ime 로드와 양립 불가.

---

## E. 검증 방법

설치/등록 후 순서대로:

1. **레지스트리:** `reg query "HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts"`
   에서 **ImmInstallIME 가 만든 새 KLID**(예 `E0xx0412`) 존재 + Ime File=unim_imm32.ime.
   (E0200412 고정이 아닐 수 있음에 유의.)
2. **Preload:** `reg query "HKCU\Keyboard Layout\Preload"` 에 그 KLID. Substitutes 에
   E0200412 항목 **없을 것**(제거됨).
3. **로드:** Win+Space 전환기에 "UNIM Korean (IMM32)" 노출. 선택.
4. **프로세스 로드 확인(핵심):** KakaoTalk 포커스 + UNIM IMM32 선택 상태에서
   `tasklist /m unim_imm32.ime` → **KakaoTalk.exe(+다른 IMM32 앱) 가 나와야 함**(현재 0).
5. **DLL attach 로그:** debug 빌드면 `%TEMP%\unim-imm32.log` 에 DLL_PROCESS_ATTACH +
   `ImeInquire`/`ImeSelect` 기록 생성(현재 부재).
6. **실입력:** KakaoTalk 입력란에 한글 조합/커밋.

> 빠른 격리 실험(코드 변경 전, VM/실측): 관리자 cmd 에서 작은 테스트 EXE 로
> `ImmInstallIMEW("C:\Windows\System32\unim_imm32.ime","UNIM Korean (IMM32)")` 직접 호출
> → 반환 HKL(0 이 아니면 KLID 출력) 확인. **0 이면 .ime 자체의 export/IMEINFO 문제,
> 비0 이면 등록 경로(우리 register.rs)만 고치면 됨.** 이 한 번의 실험이 A-1 을 확정한다.

---

## F. 신뢰도 / 미확인 (사실 vs 추측)

| 항목 | 판정 | confidence |
|---|---|---|
| ImmInstallIME 가 KLID 를 할당·반환, 인자는 .ime 전체경로 | 사실 | **확인됨**(1차) [imm-imminstallimew] |
| 수동 Keyboard Layouts 행만으론 IMM32 IME 가 로드 안 됨 | 사실(정공법=ImmInstallIME) | **확인됨**(mozc 절차+MS) |
| 1419 의 직접 원인 = OS 미등록 KLID | 강한 가설 | **정황→확인됨**(미등록 KLID 활성화 실패 패턴) |
| 1419 가 "정확히" HOTKEY_NOT_REGISTERED 인 win32k 내부 이유 | 가설 | **추측**(비공개 내부) |
| KLID 고정(E0200412) 불가, OS 할당 | 사실 | **확인됨**(KLID high word=device id, ImmInstallIME 반환) |
| Substitutes(E0200412→00000412)가 .ime 로드 차단 | 사실 | **확인됨**(KLF_SUBSTITUTE_OK 문서) |
| 미서명 .ime 데스크톱 로드 가능 | 사실 | **정황→확인됨**(mozc 전례 + IMM32 차단=앱컨테이너 한정) |
| KakaoTalk(데스크톱 x86)에 IMM32 경로 유효 | 사실 | **확인됨**(문서 "desktop apps only" 차단) |
| **x86 ImmInstallIME 등록이 32비트 앱 커버에 별도 필요** | 미확인 | **추측** — VM 실측 필요 |
| 현 머신 실제 레지스트리값(KBDA1 vs KBDKOR, Layout Id) | 미확인 | 재덤프 필요(E4 는 과거) |
| ImmInstallIME 직접 호출 시 반환 HKL(0 여부) | **미실측 — E 의 격리실험 필수** | — |

---

## 참고문헌

- [imm-imminstallimew] ImmInstallIMEW (imm.h): https://learn.microsoft.com/en-us/windows/win32/api/imm/nf-imm-imminstallimew
- [winuser-loadkeyboardlayoutw] LoadKeyboardLayoutW (winuser.h): https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-loadkeyboardlayoutw
- [ime-requirements] Input Method Editor (IME) requirements (IMM32 차단=desktop 외): https://learn.microsoft.com/en-us/windows/apps/develop/input/input-method-editor-requirements
- [build-mozc-windows] How to build Mozc in Windows (register_ime, 관리자, ImmInstallIME): https://code.googlesource.com/mozc/+/HEAD/doc/build_mozc_in_windows.md
- [terminology] kbdlayout.info terminology (KLID=LANGID low + device high): https://kbdlayout.info/terminology
- [narkive-klid] Keyboard Layout IDs (Layout Id 충돌 규칙): https://microsoft.public.win32.programmer.international.narkive.com/9j6xMxHh/keyboard-layout-ids
- [answers-kbd-never-activated] Developed a keyboard layout DLL but never activated: https://learn.microsoft.com/en-us/answers/questions/1239908/developed-a-keyboard-layout-dll-from-source-but-is
- [answers-kbd-2012] how to programmatically install a keyboard layout (System32 강제, 2012 업데이트): https://learn.microsoft.com/en-us/answers/questions/134349/how-to-programmatically-install-a-keyboard-layout
- katahiromz/ImeStudy: https://github.com/katahiromz/ImeStudy
- 내부: unim-imm32/src/{register.rs,globals.rs,lib.rs}, unim-windows-common/src/activation.rs, installer/wix/unim.wxs, docs/dev/windows/_archive/imm32-diagnosis-evidence.md, research-korean-tsf-imes.md

*기준일 2026-06-21. E 의 격리 실험(ImmInstallIME 직접 호출 반환값)과 현 머신 레지스트리
재덤프로 A-1/x86 경로를 실측 확정할 것.*
