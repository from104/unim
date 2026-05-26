# UNIM Windows MSI Smoke Test

GitHub Actions artifact `unim-<version>-x64-msi` 를 받은 다음, Windows 11 x64 VM (또는 실기) 에서 다음 절차를 순서대로 수행하고 각 단계의 결과를 `MSI_DIAGNOSIS_<date>.md` 에 기록한다.

스냅샷 권장: 깨끗한 Windows 11 22H2 또는 24H2 한국어 옵션 사전 설치 상태.

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

`☐` 를 `OK` / `FAIL: 사유` 로 갱신.

## 5. 트레이 / 설정 UI

```powershell
# 시작 메뉴에 "UNIM Korean IME" 단축키
Get-StartApps | Where-Object Name -like '*UNIM*'

# 실행
& "$env:ProgramFiles\UNIM\unim-windows.exe"
```

기대:

- 트레이 아이콘 노출.
- 우클릭 → 설정 → 일반/오타교정/한자 페이지 렌더.

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
