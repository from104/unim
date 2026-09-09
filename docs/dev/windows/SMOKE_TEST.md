# UNIM Windows MSI Smoke Test

GitHub Actions artifact `unim-<version>-x64-msi` 를 받은 다음, Windows 11 x64 VM (또는 실기) 에서 다음 절차를 순서대로 수행하고 각 단계의 결과를 `MSI_DIAGNOSIS_<date>.md` 에 기록한다.

스냅샷 권장: 깨끗한 Windows 11 22H2 또는 24H2 한국어 옵션 사전 설치 상태.

## CI 자동 검증 (2026-09 도입)

아래 절차 중 **기계가 판정할 수 있는 부분은 CI 가 매 빌드마다 이미 돌린다.**
`windows-msi.yml` 의 `Install + verify` / `Functional typing check` /
`Uninstall + verify` 세 단계가 `scripts/ci/verify-msi.ps1` 을 phase 별로 호출하고,
`install.log`·`uninstall.log`·스크린샷·타이핑 로그를 `unim-<version>-msi-verification`
아티팩트로 항상 올린다. 그러니 VM 스모크는 **자동화가 못 보는 것만** 하면 된다.

| 절 | 항목 | CI 자동 | 비고 |
|----|------|---------|------|
| 1 | `msiexec /i /qn` 설치 성공 (0 또는 3010) | ✅ | 로그 아티팩트 첨부 |
| 2 | 설치 파일 12개 존재·0바이트 아님 | ✅ | 목록은 `unim.wxs` 의 `<File>` 전량과 자동 대조 — wxs 가 바뀌면 CI 가 먼저 깨진다 |
| 2 (1)(2)(6) | `InProcServer32` 64/32 뷰 경로·`ThreadingModel`, `[#…]` 토큰 미치환 회귀 | ✅ | GUID 는 `installer/wix/generated/guids.wxi` 에서 런타임에 읽는다(하드코딩 없음) |
| 2 (3)(4)(5) | CTF TIP 엔트리, `LanguageProfile\0x00000412\{PROFILE}` 의 `Enable`/`SubstituteLayout`/`Description`/`IconFile`, 카테고리 8종 | ✅ | 카테고리 GUID 도 `unim.wxs` 에서 파싱 |
| — | `unim_tsf.dll`(x64)·`unim_tsf32.dll`(x86) `LoadLibraryW` + `DllGetClassObject` 노출 | ✅ | 비트니스별 별도 powershell.exe 로 프로브 |
| — | `HKLM…\Run\UnimPopupRenderer`, `HKLM\SOFTWARE\atit.org\UNIM\InstallDir` 토큰 치환(M-31 회귀) | ✅ | |
| 3 | `Get-WinUserLanguageList` 에 UNIM TIP 등록 + `ActivateLanguageProfile` | ⚠️ | typing 단계(승격 대기) |
| 4.1 | 메모장에 `gks` → `한` 실입력 | ⚠️ | typing 단계(승격 대기). 스크린샷·읽은 텍스트가 아티팩트로 남는다 |
| — | Windows Defender 능동 스캔 (MSI + DLL/EXE 오탐 조기 발견) | ⏭ | `-Phase scan`. **GitHub 호스티드 러너에서는 동작하지 않는다** — 2026-09-04 4차 실측: WinDefend 서비스가 꺼져 있어 `MpCmdRun -Scan` 이 `hr=0x800106ba` 로 실패(시그니처 갱신만 됨). 스크립트는 이 경우를 SKIP 으로 명시한다. 실제 게이트로 쓰려면 Defender 가 도는 self-hosted 러너나 외부 스캐너(VirusTotal API)가 필요하다. `MpCmdRun -Scan` + `Get-MpThreatDetection`/이벤트 1116·1117 로 판정, 탐지 시 exit=2. install 단계보다 먼저 돌아 exclusion 등록 이전 상태로 스캔한다 |
| 6 | `msiexec /x` 제거, 레지스트리·설치 디렉터리·ARP 항목 소멸 | ✅ | |

**⚠️ = "승격 대기"**: 호스티드 러너가 대화형 데스크톱 세션(ctfmon/TSF 활성)을
보장한다는 문서가 없어, 타이핑 단계는 `continue-on-error: true` 로 도입했다.
2회 연속 통과가 확인되면 `continue-on-error` 를 떼어 필수 게이트로 승격한다.

**아직 사람만 할 수 있는 것** (아래 절이 여전히 유효한 범위):
4.3 한자 변환 팝업, 4.5~4.8 32-bit/UWP/KakaoTalk 실앱, 4.9 낭독기 통지,
4b 팝업 전 항목, 4c AutoTypeFix, 5 랭귀지바·설정 다이얼로그·config reload,
그리고 설정 GUI 자체(`unim-settings.exe` 에 `--version` 같은 헤드리스 플래그가
없어 CI 는 존재·크기까지만 본다).

로컬에서 한 번에 돌리려면 (Windows VM 관리자 PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ci\verify-msi.ps1 `
    -MsiPath dist\unim-0.4.1-x64.msi -Phase all -ArtifactDir msi-verify
```

`-Phase all` 은 install/typing/uninstall 만 돈다 — Defender 스캔은 별도다
(스캔 자체가 실시간 검사를 강제로 트리거해 install 단계의 exclusion 등록과
순서가 섞이면 판정이 흔들린다). 따로 돌리려면:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ci\verify-msi.ps1 `
    -MsiPath dist\unim-0.4.1-x64.msi -Phase scan -ArtifactDir msi-verify `
    -ScanPaths "target\x86_64-pc-windows-msvc\release;target\i686-pc-windows-msvc\release"
```

> 2026-09-03 회사컴에서 Defender 가 `unim_tsf.dll`(0.4.1) 을
> `Trojan:Win32/Bearfoos.B!ml` 로 오판해 하루 4회 격리한 사고가 있었다(무서명 +
> VERSIONINFO 공란 + low prevalence 조합). 사용자 대응은
> [troubleshooting §4-W](../../user/troubleshooting/README-ko.md#4-w-windows-defender가-unim_tsfdll을-트로이목마로-격리한다)
> 참조.

## 0. 사전 준비

- VM / 실기: Windows 11 x64, 관리자 권한 사용자 로그인.
- VM 이면 설치 직전 스냅샷 생성 (실패 시 빠른 롤백).
- 다른 한글 IME (날개셋, 새나루 등) 가 활성화돼 있으면 비활성화. UNIM 단독 검증.

## 1. 설치

관리자 PowerShell:

```powershell
msiexec /i .\unim-0.3.0-x64.msi /l*vx install.log /qn
Get-Content install.log | Select-Object -Last 50
```

기대:

- ExitCode 0.
- `install.log` 끝부분에 `Installation success or error status: 0` 노출.
- `C:\Program Files\UNIM\unim_tsf.dll` 와 `unim-windows.exe` 생성.

## 2. 레지스트리 무결성

```powershell
# (1) COM CLSID
reg query "HKCR\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}" /s | Out-File clsid.reg -Encoding utf8

# (2) InProcServer32 — DLL 경로가 raw 토큰이 아닌 정상 경로여야 함
reg query "HKCR\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\InProcServer32" /ve
# 기대: REG_SZ = "C:\Program Files\UNIM\unim_tsf.dll"
# 실패 시: 값이 "[#unim_tsf.dll]" 같은 raw 토큰이면 wxs 의 토큰 치환이 실패한 것.

# (3) TIP 엔트리
reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}" /s | Out-File tip.reg -Encoding utf8

# (4) 한국어 프로필 활성 플래그
reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\LanguageProfile\0x00000412\{B2C3D4E5-F6A7-8901-BCDE-F12345678901}" /v Enable
# 기대: Enable    REG_DWORD    0x1

# (5) Category 키 — Item 마지막 sub-key 가 CLSID 인지 확인 (이전 P1 버그)
reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\Category\Item\{34745C63-B2F0-4784-8B67-5E12C8701A31}" /s
# 기대: …\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890} 가 sub-key 로 보임.

# (6) 32-bit COM 등록 — 32-bit 앱(KakaoTalk 등) 지원의 필수 키 (SOLVED 2026-06-22)
reg query "HKLM\SOFTWARE\WOW6432Node\Classes\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\InProcServer32" /ve
# 기대: REG_SZ = 32-bit unim_tsf.dll 경로 (i686 빌드). ThreadingModel=Apartment.
# 이 키가 없으면 32-bit 앱(카톡)에서 UNIM 이 안 뜬다 — 근본원인. 참조: imm32-win11-SOLUTION.md.
```

## 3. TIP 발견 / 활성화

```powershell
# 현 사용자의 입력 메서드 목록에 UNIM TIP 가 들어왔는지
Get-WinUserLanguageList | ForEach-Object {
    "{0}: {1}" -f $_.LanguageTag, ($_.InputMethodTips -join ', ')
}
# 기대 (한국어가 설치돼 있으면):
#   ko-KR: 0412:{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}{B2C3D4E5-F6A7-8901-BCDE-F12345678901}

# 안 보이면 수동 등록:
$list = Get-WinUserLanguageList
$ko = $list | Where-Object LanguageTag -eq 'ko-KR'
$ko.InputMethodTips.Add('0412:{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}{B2C3D4E5-F6A7-8901-BCDE-F12345678901}')
Set-WinUserLanguageList $list -Force
```

GUI 경로: `설정 → 시간 및 언어 → 한국어 → 키보드 → 키보드 추가 → "UNIM Korean IME"`.

## 4. 입력 동작 (사람이 직접)

| # | 동작 | 입력 | 기대 결과 | 결과 |
|---|------|------|-----------|------|
| 4.1 | 메모장 한글 입력 (두벌식) | `dkssudgktpdy` | `안녕하세요` | ☐ |
| 4.2 | 백스페이스 마지막 자모 제거 | `dks` → BS | `안` | ☐ |
| 4.3 | 한자 변환 (한자키 또는 우측 Ctrl) | `한자` → 한자키 | 한자 후보 popup | ☐ |
| 4.4 | 한영 토글 (한/영 키) | `dkssud` → 한/영 → `hello` | `안녕hello` | ☐ |
| 4.5 | 32 비트 앱 동작 | x86 메모장 (`%windir%\SysWOW64\notepad.exe`) | 4.1 과 동일 | ☐ |
| 4.6 | UWP 앱 동작 (Microsoft Edge 주소창) | `dkssud` | `안녕` | ☐ |
| 4.7 | **KakaoTalk (32-bit 실앱) 한글 입력** | 카톡 채팅 입력란에서 UNIM 전환 → `dkssudgktpdy` | `안녕하세요` 정상 inline 조합 (SOLVED — 32-bit TSF 등록 필수) | ☐ |
| 4.8 | KakaoTalk 한영 토글 | 카톡 입력란 → `dkssud` → 한/영 → `hi` | `안녕hi` | ☐ |
| 4.9 | **접근성: 낭독기 모드 전환 통지 (A11Y-03)** | NVDA 또는 내레이터 실행 중, 메모장에서 한/영 키로 전환 | 낭독기가 "한글"/"영어" 등 모드 전환을 음성으로 통지 (`NotifyWinEvent` 대상 객체 실효 확인) | ☐ |

`☐` 를 `OK` / `FAIL: 사유` 로 갱신.

> **4.7/4.8 실패 시 1순위 의심:** 2절 (6) 32-bit COM 키 부재 = i686 `unim_tsf.dll` 미빌드/미등록.
> 카톡 미동작의 근본원인이며, 64-bit 앱(4.1~4.6)이 정상이어도 32-bit 등록이 없으면 카톡만 안 된다.
> 진단·해결: **[imm32-win11-SOLUTION.md](imm32-win11-SOLUTION.md)**.

## 4b. 팝업 동작 (Phase 3 — 한자/특수문자/이모지)

자체 layered popup 윈도우(WS_EX_NOACTIVATE)로 렌더된다. 팝업이 떠도 타깃 앱 포커스가 유지돼야 한다.

| # | 동작 | 입력 | 기대 결과 | 결과 |
|---|------|------|-----------|------|
| 4b.1 | 한자 후보 popup | `한자` → 한자키 | 후보 격자 표시, 행(숫자)+열(Q~O) 레이블 강조 | ☐ |
| 4b.2 | 후보 페이지 이동 | popup 활성 → PageDown/PageUp | 페이지 전환, 선택 위치 보존 | ☐ |
| 4b.3 | 9x9 확장 격자 토글 | popup 활성 → Period(.) | compact(9) ↔ expanded(81) 전환 | ☐ |
| 4b.4 | 한자 북마크 토글 | popup 활성 → Space | ★ flash(노랑) 후 즐겨찾기 반영 | ☐ |
| 4b.5 | 후보 선택/취소 | 숫자/방향+Enter / Esc | commit / 팝업 닫힘·composition 유지 | ☐ |
| 4b.6 | 특수문자 popup | 특수문자 트리거 | 특수문자 격자 표시 | ☐ |
| 4b.7 | 이모지 popup | `Super`+`.` | 이모지 카테고리 격자 최초 표시 | ☐ |

## 4c. AutoTypeFix (Phase 4 — 자동 한영 오타 교정)

설정에서 AutoTypeFix `enabled`/`forward`/`reverse` 가 켜져 있어야 한다(5절 설정 또는 config.yaml).

| # | 동작 | 입력 | 기대 결과 | 결과 |
|---|------|------|-----------|------|
| 4c.1 | 순방향 자동 교정 (영문오타→한글) | 한글모드 인식 실패 영문 입력 | 자동으로 한글 재조합 | ☐ |
| 4c.2 | 역방향 자동 교정 (한글오타→영문) | 영문모드인데 한글식 입력 | 자동으로 영문 교정 | ☐ |
| 4c.3 | 수동 변환 (선택 없음) | `dkssud` → `Ctrl+Shift+Space` | `안녕`으로 변환 | ☐ |
| 4c.4 | 수동 변환 (선택 영역) | 텍스트 선택 → `Ctrl+Shift+Space` | 선택 영역만 변환 | ☐ |
| 4c.5 | undo | 자동 교정 직후 `Ctrl+Z` | 교정 전으로 복원 | ☐ |
| 4c.6 | blacklist 재트리거 억제 | 동일 단어 재입력 | 재교정 안 됨 | ☐ |

## 5. 랭귀지바 / 설정 다이얼로그 / config reload (TSF 네이티브)

별도 트레이 앱(unim-windows)은 제거됐다. 모든 UI 는 `unim_tsf.dll` 내부 네이티브 Win32.
설정 저장소는 `%APPDATA%\unim\config.yaml`. UNIM TIP 가 활성(입력 가능)인 상태에서 검증.

> 언어바에 UNIM 이 아예 노출되지 않는 경우(설치/갱신 직후 D-2)는 5.1 이전에
> **[D2-tray-after-install-checklist.md](D2-tray-after-install-checklist.md)** 의
> 재현 매트릭스·D-3 스테일 DLL 확정 절차부터 수행한다.

| # | 동작 | 기대 결과 | 결과 |
|---|------|-----------|------|
| 5.1 | 랭귀지바 버튼 노출 | 시스템 언어바에 UNIM 한/영 상태 버튼(아이콘/텍스트) | ☐ |
| 5.2 | 한/영 전환 시 랭귀지바 동기 | 한/영 키로 모드 전환 시 버튼 아이콘·텍스트 즉시 갱신 | ☐ |
| 5.3 | 랭귀지바 버튼 클릭 → 엔진 토글 | 버튼 클릭(또는 메뉴 `한/영 전환`)으로 입력 모드 전환 | ☐ |
| 5.4 | 랭귀지바 메뉴 | `한/영 전환` / `기본 입력기로 설정` / `설정 열기` 항목 표시 | ☐ |
| 5.5 | Windows 옵션 버튼 → 설정 다이얼로그 | `설정 → 시간 및 언어 → 한국어 → 키보드 → UNIM → 옵션/속성` 클릭 시 네이티브 설정창(`ITfFnConfigure::Show`) | ☐ |
| 5.6 | 설정 다이얼로그 4탭 렌더 | 일반 / 오타 교정 / 억제 단어 / 사용자 사전 탭 전환 | ☐ |
| 5.6a | 일반 탭 | 자판 콤보, 룰셋 체크(자판 변경 시 동적 재구성), 모아치기(bidirectional + chord_window 슬라이더), 시작모드·모드공유 콤보, 전환/한자키·트리거키 EDIT | ☐ |
| 5.6b | 오타 교정 탭 | AutoTypeFix 체크박스 7 + 슬라이더 6 | ☐ |
| 5.6c | 억제 단어 탭 | 블랙리스트 ListBox + 삭제/비활성화/확정 → typefix-blacklist.yaml 즉시 저장 | ☐ |
| 5.6d | 사용자 사전 탭 | 단어/메모 추가·수정·삭제 → typefix-userdict.yaml 즉시 저장 | ☐ |
| 5.7 | 설정 저장 | 확인/적용 시 config.yaml 갱신, 취소 시 미저장(블랙/사전은 즉시 저장) | ☐ |
| 5.8 | "기본 입력기로 설정" 버튼/메뉴 | UNIM 이 한국어 기본 프로필로 지정(`SetDefaultLanguageProfile`) | ☐ |
| 5.9 | config reload | 설정 다이얼로그에서 변경·저장 후 타깃 앱 포커스 이동 시 TSF 가 새 설정 반영(OnSetFocus mtime) | ☐ |

## 6. 제거

```powershell
$product = Get-WmiObject Win32_Product | Where-Object Name -like 'UNIM*'
"$($product.IdentifyingNumber)"
msiexec /x "$($product.IdentifyingNumber)" /l*vx uninstall.log /qn
```

기대:

- `C:\Program Files\UNIM\` 삭제.
- 레지스트리 키 전부 삭제 (2 단계의 reg query 가 `오류: ...` 응답).
- 시작 메뉴 단축키 사라짐.

## 7. 결과 보고

`docs/dev/windows/MSI_DIAGNOSIS_<YYYY-MM-DD>.md` 에 다음을 첨부해 PR 코멘트로 공유:

- `install.log` / `uninstall.log` 마지막 50줄.
- `clsid.reg` / `tip.reg` 전문.
- 4 절 매트릭스 결과.
- 실패 단계 스크린샷.
