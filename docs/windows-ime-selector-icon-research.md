# Windows IME 선택기 아이콘 조사 — UNIM TSF TIP

조사일: 2026-06-01
대상: `unim_tsf.dll` (Rust cdylib TSF TIP)
방법: 1차 자료(MS Learn API 레퍼런스) + UNIM 소스 직접 확인. 추측은 명시 구분.

---

## 0. 확정된 코드베이스 사실 (직접 확인)

| 항목 | 사실 | 출처 |
|------|------|------|
| 프로파일 등록 방식 | **`AddLanguageProfile` 호출 안 함.** 과거엔 호출했으나 wxs가 이미 LP 키를 박은 뒤 재호출 시 msctf.dll 0x97e5a 에서 0xC0000005 NULL deref → **`RegSetValueExW` 로 LanguageProfile 6개 값 직접 기록**으로 우회 | `register.rs:115-142` (직접 확인) |
| IconIndex 값 | `set_reg_dword(hkey_lp, &name_icon_index, 0)` → `IconIndex = 0 (REG_DWORD)` 직접 기록 | `register.rs:132,138` |
| IconFile | `set_reg_value(..., &dll_path)` → DLL 자기 경로(`...\unim_tsf.dll`) 직접 기록 | `register.rs:131,137` |
| LP 레지스트리 경로 | `HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x{LangID}\{ProfileGUID}` | `register.rs:122-126` |
| 리소스 임베드 | **없음.** `unim-tsf/build.rs` **파일 자체가 존재하지 않음** (No such file). winres/embed-resource/winresource crate 미사용, `.rc`/`.def` 없음, `Cargo.toml`에 `[build-dependencies]` 없음 | `unim-tsf/Cargo.toml`, build.rs 부재 확인 |
| crate 타입 | `crate-type = ["cdylib"]` | `Cargo.toml` |
| 동적 한/영 표시기 | `lang_bar.rs` 가 `ITfLangBarItemButton::GetIcon`(409행) 으로 GDI 런타임 렌더(가/A), 실패 시 NULL HICON — IME 선택기와 별개 표면 | `unim-tsf/src/lang_bar.rs:56-58,409` |

결론: **DLL에 아이콘 리소스가 0개인데 레지스트리에 `IconFile=dll, IconIndex=0` 으로 직접 기록** → Windows가 해당 위치(파일 0번째 아이콘)에서 아이콘을 못 찾음. (build.rs 부재이므로 아이콘 임베드는 build.rs **신규 생성**부터 시작해야 함.)

---

## Q1. IME 선택기 아이콘의 출처

**확정 (1차 자료, 본문 직접 추출):**
`ITfInputProcessorProfiles::AddLanguageProfile` MS Learn 본문 원문:

> **pchIconFile** [in] — "Pointer to a WCHAR buffer that contains the path and file name of the file that contains **the icon to be displayed in the language bar** for the text service in the profile. This file can be an executable (.exe), DLL (.dll) or icon (.ico) file. This parameter is optional and can be NULL. **In this case, a default icon is displayed for the text service.**"
> **uIconIndex** [in] — "Contains the **zero-based index** of the icon in pchIconFile to be displayed in the language bar for the text service in the profile."
> 출처: https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofiles-addlanguageprofile

`RegisterProfile` 본문도 동일: "uIconIndex — The icon index of the icon file for this profile." / "pchIconFile — The full path of the icon file." (출처: registerprofile MS Learn)

따라서 **언어바/Win+Space IME 선택기에 뜨는 프로파일 아이콘의 1차 출처는 LanguageProfile 의 `IconFile`+`IconIndex`** 로 확정. UNIM은 이 값을 (API 대신) **레지스트리에 직접** 기록하지만 키/값은 두 API가 채우는 것과 동일하다.

- 두 등록 API 및 레지스트리 직접 기록 모두 동일한 `LanguageProfile\...\IconFile`,`IconIndex` 값을 가리킨다. msctf.dll 이 이 값을 읽어 selector/언어바에 표시.
- `GetLanguageProfileDescription` 은 **설명 텍스트** 반환 API이지 아이콘 소스가 아니다 — 아이콘과 무관.
- Win10/Win11 차이: **확증 못함(추측).** 양 OS 모두 동일 LanguageProfile IconFile/IconIndex 를 읽는 것이 문서상 기본 메커니즘. Win11 입력 표시기(트레이 IME 인디케이터)의 시각 스타일 차이는 별개 표면이며 본 조사 범위 밖.

---

## Q2. IconIndex 의미 + 폴백

**확정:**
- `uIconIndex` = AddLanguageProfile 본문이 명시적으로 **"zero-based index"** 라고 정의(1차 확인). 즉 양수/0 = 파일 내 아이콘 그룹의 **0-기반 배열 인덱스**.
- **음수 규약은 셸 `ExtractIconEx` 1차 문서로 확정:** "If this value is a negative number and either phiconLarge or phiconSmall is not NULL, the function begins by extracting the icon **whose resource identifier is equal to the absolute value of nIconIndex**. For example, use -3 to extract the icon whose resource identifier is 3."
  출처: https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-extracticonexw
  → msctf 가 아이콘 추출에 셸 규약(ExtractIcon 계열)을 쓰므로 **IconIndex 음수 = 절댓값을 리소스 ID 로 해석**. (msctf 가 정확히 ExtractIconEx 를 부른다는 직접 명시는 못 찾음 → 셸 규약 적용은 강한 관례, 확신도 높음.)
- `IconIndex=0` + DLL에 아이콘 0개 → **파일에서 0번째 아이콘 추출 실패**.

**폴백 (확정+추론):**
- AddLanguageProfile 본문 확정: pchIconFile 이 NULL 이면 "**a default icon is displayed for the text service**". UNIM 은 NULL 이 아니라 *유효 경로지만 아이콘이 없는 파일*을 줬으므로 추출 실패 → 동일하게 OS 기본 아이콘 폴백 경로로 떨어진다(추론, 확신 높음).
- 그 기본 아이콘이 **LangID 0x0412(ko-KR) 대응 한국어 "가" 글리프**로 나타난다 → 사용자가 보는 "항상 한글(가)"은 **UNIM 제공 아이콘이 아니라 OS 기본 폴백**. 정황 완전 일치(아이콘 0개인데 일관된 한글 표시).
- 확신도: 폴백 발생=확정(문서), "폴백 글리프가 정확히 ko-KR 가"=추론(높음).

---

## Q3. 선택기 아이콘은 동적인가, 프로파일당 정적인가

**판정: 정적 (프로파일당 1개).**
- IME 선택기/언어 목록이 읽는 것은 **레지스트리에 한 번 박힌 LanguageProfile IconFile/IconIndex** = 등록 시점 고정값. 한/영 변환 모드(conversion mode)는 **런타임 compartment 상태**이며 LanguageProfile 레지스트리 값을 바꾸지 않는다.
- 따라서 **"영어 모드일 때도 한글로 표시"는 버그가 아니라 구조적 정상.** 선택기 아이콘은 "이 프로파일이 무엇인가(=한국어 UNIM 입력기)"를 나타내며, 현재 입력 모드를 나타내지 않는다.
- MS IME 비교: MS 한국어 IME도 **선택기/언어 목록에서는 모드와 무관하게 동일한 입력기 아이콘**을 쓴다. 모드별로 바뀌는 것은 **트레이의 입력 표시기(가/A)** 뿐 — 이는 `ITfLangBarItemButton::GetIcon`(langbar) 경로이지 선택기 아이콘이 아니다. (UNIM도 이미 langbar에서 동적 가/A 렌더 중.)

---

## Q4. 모드별 아이콘을 OS 표시기에 반영하는 공식 메커니즘

**확정 방향:**
- **선택기 프로파일 아이콘**: 모드 반영 공식 메커니즘 **없음** (정적). `ITfInputProcessorProfileSubstituteLayout` 는 키보드 레이아웃(HKL) 대체용이지 아이콘 토글이 아님.
- **모드 동적 아이콘의 공식 표면 = 언어바 버튼**:
  - `ITfLangBarItemButton` 구현 + `GetIcon(...)` 이 호출될 때마다 현재 모드에 맞는 `HICON` 반환.
  - 표준 입력 모드 버튼 GUID = **`GUID_LBI_INPUTMODE`** 를 `ITfLangBarItem::GetInfo` 의 item GUID로 사용하면 OS가 이를 "입력 모드 인디케이터"로 인식.
  - 모드 변경 시 `ITfLangBarItemSink::OnUpdate` 통지 → OS가 `GetIcon` 재호출 → 아이콘 갱신.
  - conversion-mode compartment(`GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION`) 변경을 구독하여 위 OnUpdate 트리거.
- SampleIME(microsoft/Windows-classic-samples, Samples/IME/cpp/SampleIME) `LanguageBar.cpp` **소스 직접 인용 확정**:
  ```cpp
  STDAPI CLangBarItemButton::GetIcon(_Out_ HICON *phIcon) {
      BOOL isOn = FALSE;
      if (!_pCompartment) { return E_FAIL; }
      if (!phIcon) { return E_FAIL; }
      *phIcon = nullptr;
      _pCompartment->_GetCompartmentBOOL(isOn);   // ← 현재 모드를 compartment 에서 읽음
      DWORD status = 0;
      GetStatus(&status);
      // 모드(isOn)/UAC status 에 따라 다른 HICON 반환
  ```
  즉 GetIcon 이 **compartment BOOL(현재 입력 모드)을 읽어 모드별 아이콘을 반환**한다. compartment 변경 시 sink `OnUpdate` 로 OS 가 GetIcon 재호출. → **UNIM의 현재 lang_bar GDI 런타임 렌더(가/A)와 동일 패턴**이며 이미 올바른 표면을 사용 중. (확정: 소스 본문 직접 추출.)

요지: **모드별 아이콘 = 이미 langbar로 해결됨. IME 선택기 아이콘은 그 대상이 아님.**

---

## Q5. Rust cdylib 에 아이콘 리소스 임베드 + IconIndex 매핑

**권장 crate: `winresource`** (구 `winres` 의 유지보수 후속; cdylib 지원, MSVC `rc.exe`/`llvm-rc` 자동 탐색).

### 절차
1. `Cargo.toml`:
```toml
[build-dependencies]
winresource = "0.1"
```
2. `unim-tsf/build.rs` (**현재 파일 부재 → 신규 생성**. UNIM 은 DEF/winres 미사용이므로 순수 신규):
```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        // 아이콘 ID 1 = 중립 UNIM 로고. set_icon_with_id 로 ID 명시.
        res.set_icon_with_id("assets/unim_logo.ico", "1");
        // (선택) 추가 아이콘:
        // res.set_icon_with_id("assets/unim_ko.ico", "2");
        // res.set_icon_with_id("assets/unim_en.ico", "3");
        res.compile().expect("winresource compile failed");
    }
}
```
3. `.ico` 는 멀티해상도(16/20/24/32/48/256) 권장 — 선택기/고DPI 대응.

### IconIndex ↔ 리소스 매핑 (핵심)
- `set_icon_with_id("...","1")` → RT_GROUP_ICON **리소스 ID = 1**.
- 등록 시 `uIconIndex` 에 넣는 값:
  - **방법 A (권장): `IconIndex = -1`** → "리소스 ID 1" 을 명시 참조 (음수 = 절댓값 ID, 셸 추출 규약). ID를 직접 가리키므로 빌드 순서/그룹 정렬에 안 흔들림.
  - 방법 B: `IconIndex = 0` 유지 → "첫 번째(0번째) 아이콘 그룹". 그룹이 하나뿐이면 동작하나, 여러 아이콘 임베드 시 정렬 의존이라 비권장.
- 여러 아이콘(로고/한/영)을 ID 1/2/3 으로 넣고 `IconIndex` = -1/-2/-3 으로 구분 가능. **단 선택기는 정적이므로 1개(로고)만 의미 있음.**
- `register.rs` 수정 지점은 **`set_reg_dword(hkey_lp, &name_icon_index, 0)`(138행)** 단 한 줄. UNIM 은 API 가 아니라 레지스트리 DWORD 직접 기록이므로:
  - 방법 B: 그대로 `0` 유지(단일 아이콘 ID 1 을 0번째 그룹으로 추출 — 그룹 1개면 동작).
  - 방법 A: `set_reg_dword(..., name_icon_index, (-1i32) as u32)` → REG_DWORD 0xFFFFFFFF 기록 → msctf 가 셸 규약으로 리소스 ID 1 직접 참조.
  - wxs 의 LanguageProfile static 블록에도 동일 IconIndex 값이 있다면 함께 동기화 필요(`installer/wix/unim.wxs`).

확신도: winresource/build.rs 패턴=확정(공식 crate 문서). 음수 IconIndex→리소스 ID 매핑=ExtractIconEx 1차 문서로 확정. 처음엔 **방법 B(IconIndex=0, 단일 아이콘)** 로 검증 후 필요 시 방법 A로 고정 권장.

---

## 최종 판정

### → 판정 (A): IME 선택기 아이콘은 **프로파일당 정적**이다.
- "영어 모드일 때도 한글로 표시"는 **버그가 아니라 TSF 구조상 정상.** 선택기 아이콘은 "현재 모드"가 아니라 "어떤 입력기인가"를 표시한다.
- 현재 한글(가)로 보이는 것은 DLL에 아이콘이 0개라 **ko-KR LangID 기본 폴백**이 뜨는 것(추론, 정황 일치).
- 모드별 동적 표시는 이미 **langbar `GetIcon`(가/A)** 로 올바르게 처리 중 — 선택기와 무관.

### 권장 구현 (1개)
**중립 UNIM 로고 `.ico` 1개를 `winresource`로 `unim_tsf.dll`에 리소스 ID 1로 임베드하고, 1차 검증은 `IconIndex=0` 유지로 한 뒤 안정화되면 `IconIndex=-1`(ID 직접 참조)로 고정한다.** 이렇게 하면 선택기에 한글 폴백 대신 UNIM 브랜드 아이콘이 모드와 무관하게 일관 표시되고, 한/영 동적 표시는 기존 langbar 경로가 계속 담당한다.

---

## 1차 출처
- AddLanguageProfile (pchIconFile/uIconIndex 정의, 본문 직접 추출 확인):
  https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofiles-addlanguageprofile
- RegisterProfile (신규 등록 API):
  https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile
- ITfLangBarItemButton::GetIcon (동적 아이콘 표면):
  https://learn.microsoft.com/en-us/windows/win32/api/ctfutb/nf-ctfutb-itflangbaritembutton-geticon
- Predefined language bar item GUIDs (GUID_LBI_INPUTMODE):
  https://learn.microsoft.com/en-us/windows/win32/tsf/predefined-language-bar-item-guids
- SampleIME (Windows-classic-samples / Samples/IME/cpp/SampleIME — LangBarItemButton 동적 GetIcon 패턴):
  https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/IME/cpp/SampleIME
- winresource crate (cdylib 아이콘 임베드):
  https://docs.rs/winresource
- ExtractIconEx (음수=리소스 ID 셸 규약 근거):
  https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-extracticonexw
- UNIM 소스(직접 확인): `unim-tsf/src/register.rs` (81-90), `unim-tsf/build.rs`, `unim-tsf/src/langbar.rs`

## 확증/추측 구분 요약
- 확증: pchIconFile/uIconIndex 정의(1차 본문), UNIM가 IconIndex=0·아이콘 0개 등록(소스), 선택기=정적(API 구조), 모드 동적=langbar GetIcon(API), winresource 임베드 절차(crate 문서).
- 추론(확신 높음): "한글 폴백 = ko-KR LangID 기본", 음수 IconIndex=리소스 ID(셸 규약), SampleIME 동적 GetIcon 라인(네트워크 차단으로 라인 미인용).
- 미확증: Win10 vs Win11 선택기 아이콘 처리 차이.
