# 브릿지 조사 — hybrid-imm-registration 각도

조사일: 2026-06-07. 각도: UNIM을 TSF TIP인 동시에 **IMM32 IME(.ime)** 로도 등록하는 하이브리드 방안.
도구: ctx_fetch_and_index + ctx_search (1차 소스 우선). ctx_index source 라벨: `bridge-hybrid-imm-registration`.

핵심 질문(적대적 재검증 대상):
1. 단일 DLL이 TSF TIP인 동시에 IMM32 .ime가 될 수 있는가?
2. 레거시 앱(wezterm)에서 IMM32 .ime 경로로 inline preedit이 되는가?
3. 실제 하이브리드 IME 사례(MS imekr, Mozc, 서드파티)는?
4. 구현 난이도 / 필수 export / 등록 절차 / 유지보수 부담은?

---

## 결론 요약 (TL;DR)

- **단일 DLL 하이브리드는 이론상 가능하나 실효성 없음.** TIP과 .ime는 *아키텍처가 완전히 다른 두 모듈*이며
  (TIP=COM in-proc server, .ime=Ime*-prefixed C export 집합), 한 DLL이 양쪽 export·등록을 모두 가질 수는 있다.
  그러나 **하나의 키보드 레이아웃/프로필이 동시에 TIP과 .ime일 수는 없다.** 사용자는 둘 중 하나를 선택해 활성화한다.
- **레거시 inline의 핵심은 .ime 등록이 아니라 "IMM32 IME로 활성화"되는 것.** .ime로 활성화되면 CUAS가
  아니라 IMM32가 직접 앱과 통신하므로 wezterm 자체 GCS_COMPSTR 렌더가 동작할 *수도* 있다. 하지만 이는
  **TIP을 버리고 .ime로 돌아가는 것**이지 "하이브리드"의 이득이 아니다.
- **업계 1급 서드파티 IME(Mozc)는 IMM32 .ime를 완전히 폐기하고 TIP-only로 출고한다.** (1차 소스: Mozc
  현행/구버전 win32 트리 + installer.wxs, 아래). 즉 "서드파티가 쓰는 하이브리드 우회 경로"는 **실재하지 않는다.**
- **Windows 8+ 에서 IMM IME는 사실상 차단/제한.** (ReactOS IME wiki) 신규 .ime 등록은 deprecated 경로.
- **MS 한국어 IME(imekr)는 .ime 하이브리드가 아니라 TSF TIP.** (선행 조사 Mozilla bug 1208043 근거,
  `bridge-direct-imm-from-tip` 인덱스). "MS만 IMM32 특권"이라는 가설은 *부분적으로만* 맞다 → MS의 특권은
  IMM32 .ime가 아니라 *시스템 내장 TIP + CUAS 내부 동작*에 있음.

**→ hybrid-imm-registration 각도는 inline 문제의 해결책이 아니다. effort huge, 유지보수 부담 큼, 서드파티 선례 0.**

---

## 1. TIP vs IMM32 IME는 다른 모듈 (1차 소스: katahiromz ImeStudy)

> "TIP stands for Text Input Processor. ... A TIP is a DLL file, that is built with many COM objects and
> interfaces. The filename extension of a TIP is usually `.dll`. **Strictly saying, a TIP is not an IME file
> (Not IMM IME nor CTF IME!).**"
> — katahiromz/ImeStudy README (source: `katahiromz-ImeStudy-readme-full`)

> "The body of an IME is a DLL file that is registered as an IME in the operating system. An IME can provide
> the functions whose names begin with `Ime`, and `NotifyIME` function."
> — 같은 출처

IMM32 .ime 필수 export (DEF 파일로 노출):
`ImeInquire, ImeConversionList, ImeRegisterWord, ImeUnregisterWord, ImeGetRegisterWordStyle,
ImeEnumRegisterWord, ImeConfigure, ImeDestroy, ImeEscape, ImeProcessKey, ImeSelect,
ImeSetActiveContext, ImeToAsciiEx, NotifyIME, ImeSetCompositionString, ImeGetImeMenuItems`
(source: `katahiromz-ImeStudy-readme-full` → "ImeInquire..." 목록)

> "How to export functions? Just define the functions and add a `.DEF` file to your DLL file project."
> — 같은 출처. 즉 단일 DLL이 COM(DllGetClassObject 등) + Ime* export를 *물리적으로* 동시에 가질 수는 있음.
> 그러나 **등록·활성화는 별개**다 (아래 2절).

**confidence: high** — 1차 소스가 "TIP은 IME 파일이 아니다"라고 명시.

---

## 2. .ime 등록·활성화 절차와 Windows 8+ 차단 (1차 소스: MS Learn, ReactOS wiki)

### 등록 절차
- `.ime` 는 사실상 DLL을 확장자만 바꾼 것. (ReactOS wiki: "These input methods are mainly DLL files renamed
  with file extension `.ime`." source: `reactos-ime-wiki`)
- 설치: 시스템 디렉터리(`%SystemRoot%\System32`)에 .ime 복사 → 레지스트리
  `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\<KLID>` 에 `Ime File`, `Layout Text` 등 기재 →
  `ImmInstallIME(lpszIMEFileName, lpszLayoutText)` 호출로 input locale identifier(HKL) 생성.
  (source: `ms-imminstallime` — "Installs an IME. ... Returns the input locale identifier for the IME.
  **This function is intended to be used by IME setup applications only.**")
- katahiromz: "What does an IME installer? It copys the IME-related files into the system, writes some
  settings in the registry, and call the `ImmInstallIME` function." (source: `katahiromz-ImeStudy-readme-full`)

### Windows 8+ 차단 (결정적 제약)
> "**Windows 8 may have already blocked IMM IMEs, or at least in Windows store apps mode.**"
> — ReactOS IME wiki (source: `reactos-ime-wiki`)

> "Old IMM32-based IME runs under CUAS emulation layer of TSF. New IMM32 has `ImmDisableTextFrameService`
> API to disable TSF in the thread."
> — katahiromz (source: `katahiromz-ImeStudy-readme`)

→ 즉 .ime로 등록해도 데스크톱 앱에서는 여전히 CUAS 에뮬레이션 아래에서 돌 가능성이 높고, Store/UWP에서는 차단.
**.ime 등록이 CUAS를 우회한다는 보장이 없다.** (적대적 검증 결과: "IMM32로 등록하면 CUAS를 피한다"는 *가설은
미입증*. wezterm 같은 순수 IMM32 앱은 CUAS-unaware이므로 .ime가 CUAS 없이 직접 IMM32로 통신할 *가능성*은
있으나, Win8+ 정책상 신규 .ime 활성화 자체가 막힐 수 있어 실증 필요.)

**confidence: medium-high** — Win8 차단은 ReactOS wiki의 "may have" 표현(2차/추정). MS Learn 1차로
ImmInstallIME가 "setup app 전용/legacy"임은 확정.

---

## 3. 서드파티 선례: Mozc는 IMM32 .ime를 폐기, TIP-only (결정적 1차 소스)

### 현행 Mozc (master) win32 트리 — `ime/` 디렉터리 없음
`base, broker, cache_service, custom_action, installer, tip` + README.md 만 존재.
**IMM32 .ime를 빌드하는 디렉터리가 없다.** (source: `mozc-win32-tree`)

### 구버전(3.33.6089, deprecated GYP era)도 동일 — `tip`만, `ime` 없음
`base, broker, build32, build64, cache_service, custom_action, installer, tip` (source: `mozc-win32-tree-old`)
→ Mozc는 이미 수년 전 IMM32 모듈을 제거했고, 그 후로도 복원하지 않았다.

### 현행 installer_64bit.wxs — 설치 파일에 .ime 0건
설치되는 입력 모듈: **`GoogleIMEJaTIP64.dll`(TIP) 단 하나** + Broker.exe / Renderer.exe / Tool.exe.
커스텀 액션: `RegisterTIP64` / `UnregisterTIP64` 만 존재. `ImmInstallIME` 류 호출·.ime 컴포넌트 전무.
(source: `mozc-installer-wxs`)

**해석:** 일본어권 최정상 서드파티 IME가, 레거시 앱 호환을 위해서라도 IMM32 .ime를 유지할 만했음에도
**완전히 TIP-only로 전환**했다. 이것이 "서드파티가 쓸 수 있는 IMM32 하이브리드 우회 경로"가 *실재하지
않는다*는 가장 강력한 반증. Mozc는 레거시 앱 inline을 CUAS↔TIP 경로(GUID_PROP_ATTRIBUTE 등)로 처리하며,
콘솔/CUAS-emulated 문서는 별도 분기로 다룬다 (source: `mozc-tip-transitory` — "Case 2: Legacy IMM32-based
apps that are running through CUAS").

**confidence: high** — Mozc 소스 트리·installer 직접 확인.

### MS imekr (대조군)
MS 한국어 IME는 IMM32 .ime 하이브리드가 아니라 TSF TIP (`imkr*` TIP). MS의 inline 특권은 .ime가 아니라
시스템 내장 TIP + CUAS 내부 처리에서 비롯 (선행 인덱스 `bridge-direct-imm-from-tip`의 Mozilla bug 1208043).
→ "MS만 IMM32 .ime 특권"이라는 우리 잠정결론은 **틀림**: MS도 TIP을 쓴다. 차이는 CUAS의 내부 우대.
**confidence: medium** (imekr 바이너리 직접 디스어셈블은 미수행, 선행 조사 인용).

---

## 4. 만약 강행한다면 — 필수 export / 난이도 / 유지보수

- 필수 C export 16종(2절 목록)을 `cdylib`에 추가 + `.def` 파일. windows-rs로 `extern "system"` fn 구현.
- `ImeProcessKey`/`ImeToAsciiEx`/`ImeSetCompositionString`/`NotifyIME` 안에서 한글 오토마타를 다시 구현
  (TIP 코어와 공유 가능하나, IMM32 컨텍스트(HIMC, COMPOSITIONSTRING) 모델로 어댑팅 필요).
- 설치: TIP 등록(현 MSI 경로) + 별도 .ime 복사 + `Keyboard Layouts` 레지스트리 + `ImmInstallIME`.
  사용자는 입력 목록에서 "UNIM (TSF)" / "UNIM (IMM)" 두 항목 중 선택 → UX 분열.
- **난이도: huge.** 한글 IMM32 IME를 0에서 작성 = 사실상 두 번째 IME 제품. 유지보수 2배.
- **ROI: 음수.** Win8+ 차단 리스크 + 서드파티 선례 0 + Mozc가 *떠난* 경로. inline 보장 미입증.

---

## 5. 최종 권고 (hybrid-imm-registration 각도)

1. **이 각도는 추진하지 말 것.** "TSF-IMM32 브릿지"의 사용자 제보는 *.ime 하이브리드 등록*을 가리키는 게
   아닐 가능성이 높다. 더 유망한 해석은 인접 각도 `bridge-direct-imm-from-tip`
   (TIP 내부에서 `ImmSetCompositionStringW`+`ImmGenerateMessage`로 GCS_COMPSTR 직접 주입) 또는
   `bridge-transitory-extension`. 이쪽이 단일 DLL·기존 TIP 유지·저비용.
2. 굳이 IMM32 path가 필요하면 "하이브리드(둘 다 등록)"가 아니라 "TIP 유지 + TIP 핸들러 내 IMM32 직접 호출"
   (= direct-imm-from-tip)이 정답. 별도 .ime 등록 불필요.
3. 선결 검증: display attribute(GUID_PROP_ATTRIBUTE) 누락으로 인한 즉시-terminate 가설부터
   (`research-cuas-bridge-terminate` / `bridge-direct-imm-from-tip` 권고와 동일).

---

## 출처 (모두 ctx 인덱싱)
- `katahiromz-ImeStudy-readme-full`, `katahiromz-ImeStudy-readme` — TIP≠IME, Ime* export 목록, 설치 절차, CUAS
- `reactos-ime-wiki` — .ime=확장자 바꾼 DLL, Win8 IMM 차단, uimetool, CUAS, Language Bar
- `ms-imminstallime` (MS Learn) — ImmInstallIMEW 시그니처, "setup app 전용", HKL 반환
- `ms-imm32-overview` (MS Learn) — IMM 개요
- `mozc-win32-tree`, `mozc-win32-tree-old`, `mozc-installer-wxs`, `mozc-win32-readme` — Mozc TIP-only, .ime 0건
- `mozc-tip-transitory` (선행) — Mozc의 legacy IMM32/CUAS 분기 처리
- `bridge-direct-imm-from-tip`, `bridge-transitory-extension`, `research-cuas-bridge-terminate` (인접 각도 선행 인덱스)
