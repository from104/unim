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
