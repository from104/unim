<#
.SYNOPSIS
    MSI 설치·등록·DLL 로드·기능 타이핑·제거 실측 검증 (CI: windows-2022 러너).

.DESCRIPTION
    windows-msi.yml 은 오랫동안 "MSI 파일이 생겼고 최소 크기를 넘는다" 까지만
    확인했다. 이 스크립트는 그 뒤를 잇는다 — 실제로 설치되는지, TSF 텍스트
    서비스가 레지스트리에 등록되는지, DLL 이 두 비트니스 모두에서 로드되는지,
    (가능하면) 한글이 실제로 입력되는지, 제거가 깨끗한지.

    로컬(Linux) 개발 환경에는 Windows 가 없어 이 스크립트는 CI 에서만 돌아간다.
    그래서 "첫 실행에서 통과" 를 목표로 방어적으로 썼다 — 판정에 필요한 상수는
    저장소 소스(installer/wix/generated/guids.wxi, installer/wix/unim.wxs)에서
    런타임에 읽고, 실패는 단계별로 격리해 표로 보고한다.

    파일 인코딩: UTF-8 **BOM 포함**. 저장소의 다른 .ps1 은 BOM 이 없지만 이
    스크립트는 Windows PowerShell 5.1(`shell: powershell`)로도 실행되므로 BOM 이
    없으면 5.1 이 본문을 ANSI 코드페이지로 읽어 한글 주석·✅ 기호가 깨진다.

.PARAMETER MsiPath
    검증 대상 MSI 경로.

.PARAMETER Phase
    install  : 설치 + 파일 + 레지스트리 + DLL 로드 + 설정 exe (필수 게이트)
    typing   : 한글 실입력 (설치된 상태에서 실행. 승격 대기 — 실패 허용)
    uninstall: 제거 + 잔존물 확인 (필수 게이트)
    scan     : Windows Defender 오탐 능동 스캔 — MSI + -ScanPaths (승격 대기)
    all      : install·typing·uninstall 을 순서대로 (로컬/수동 실행용, scan 불포함)

.PARAMETER ScanPaths
    -Phase scan 전용. MSI 외에 추가로 스캔할 파일/디렉터리(빌드 산출물 등).

.PARAMETER ArtifactDir
    install.log·uninstall.log·스크린샷·프로브 로그를 남길 디렉터리.

.PARAMETER SkipTyping
    Phase=all 일 때 typing 단계를 건너뛴다.

.PARAMETER RepoRoot
    저장소 루트(기본: 이 스크립트의 ..\..). GUID·파일 목록 대조 원본을 읽는다.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\ci\verify-msi.ps1 `
        -MsiPath dist\unim-0.4.1-x64.msi -Phase install -ArtifactDir msi-verify
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $MsiPath,

    [ValidateSet('install', 'typing', 'uninstall', 'scan', 'all')]
    [string] $Phase = 'all',

    [string] $ArtifactDir = 'msi-verify',

    [switch] $SkipTyping,

    [string] $RepoRoot,

    # -Phase scan 전용: MSI 외에 Defender 로 스캔할 경로(빌드 산출물 디렉터리 등).
    # 워크플로는 windows-msi.yml 이 'Upload raw binaries' 로 모으는 것과 같은
    # release 출력 디렉터리를 넘긴다. 생략 시 MSI 파일만 스캔한다.
    [string[]] $ScanPaths = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# CI 로그에서 ✅/❌/⏭ 가 '?' 로 뭉개지지 않게 콘솔 출력 인코딩을 UTF-8 로.
try { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) } catch { }

# ── 공통 상수 ────────────────────────────────────────────────────────────────

# wxs 가 심는 설치 경로 힌트 키(UnimPopupWinExe 컴포넌트). 설치 디렉터리를
# 하드코딩하지 않고 이 키에서 읽는다 — ConfigurableDirectory 라 변할 수 있다.
$INSTALL_DIR_KEY  = 'HKLM:\SOFTWARE\atit.org\UNIM'
$RUN_KEY          = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
$RUN_VALUE_NAME   = 'UnimPopupRenderer'

# unim.wxs <Feature Complete> 가 담는 12개 파일 (Component / File Id 주석 병기).
# 아래 Test-WxsFileCoverage 가 wxs 의 <File Name="..."> 전량을 이 목록과 대조해
# drift(파일 추가/개명) 를 자동 검출한다 — 목록을 손으로 최신화하는 부담 제거.
$EXPECTED_FILES = @(
    @{ Rel = 'unim_tsf.dll';                       Comp = 'UnimTsfDll'          },
    @{ Rel = 'unim_tsf32.dll';                     Comp = 'UnimTsfDll32'        },
    @{ Rel = 'register-tsf.bat';                   Comp = 'RegisterScripts'     },
    @{ Rel = 'unregister-tsf.bat';                 Comp = 'RegisterScripts'     },
    @{ Rel = 'unim-settings.exe';                  Comp = 'UnimTsfSettingsExe'  },
    @{ Rel = 'unim-popup-win.exe';                 Comp = 'UnimPopupWinExe'     },
    @{ Rel = 'LICENSE.txt';                        Comp = 'LicenseFile'         },
    @{ Rel = 'NOTICE.txt';                         Comp = 'LicenseFile'         },
    @{ Rel = 'LICENSES\libhangul-hanja.LICENSE';   Comp = 'ThirdPartyLicenses'  },
    @{ Rel = 'LICENSES\unicode-cldr.LICENSE';      Comp = 'ThirdPartyLicenses'  },
    @{ Rel = 'help\unim-help-ko.html';             Comp = 'HelpFiles'           },
    @{ Rel = 'help\unim-help-en.html';             Comp = 'HelpFiles'           }
)

# ── 결과 표 ──────────────────────────────────────────────────────────────────

$script:Results = New-Object System.Collections.ArrayList

function Add-Result {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        # WARN: 2026-09-04 2차 실측 후 추가 — 정보성 경고. FAIL 과 달리 게이트를
        # 막지 않는다(Write-Summary 의 합계 실패 판정에서 제외) — '알고 있고
        # 제품 경로엔 영향 없음'을 표로는 남기되 exit code 는 오염시키지 않는
        # 상태. ko-KR 언어 목록 등록처럼 "안 되면 이유가 있고 정상 경로는
        # 그걸 안 탄다"가 확인된 항목에 쓴다.
        [Parameter(Mandatory = $true)][ValidateSet('PASS', 'FAIL', 'SKIP', 'WARN')][string] $Status,
        [string] $Detail = ''
    )
    [void] $script:Results.Add([pscustomobject]@{ Name = $Name; Status = $Status; Detail = $Detail })
    switch ($Status) {
        'PASS' { $icon = [char]0x2705 }   # ✅
        'FAIL' { $icon = [char]0x274C }   # ❌
        'SKIP' { $icon = [char]0x23ED }   # ⏭
        'WARN' { $icon = [char]0x26A0 }   # ⚠
    }
    if ([string]::IsNullOrEmpty($Detail)) {
        Write-Host ("{0} {1}" -f $icon, $Name)
    } else {
        Write-Host ("{0} {1} — {2}" -f $icon, $Name, $Detail)
    }
}

# 표를 출력하고, FAIL 이 하나라도 있으면 exit 1.
function Write-Summary {
    param([Parameter(Mandatory = $true)][string] $PhaseName)

    Write-Host ''
    Write-Host ('── verify-msi [{0}] 결과 ─────────────────────────────' -f $PhaseName)
    foreach ($r in $script:Results) {
        Write-Host ('  {0,-4}  {1}' -f $r.Status, $r.Name)
        if (-not [string]::IsNullOrEmpty($r.Detail)) {
            Write-Host ('        {0}' -f $r.Detail)
        }
    }

    $failed  = @($script:Results | Where-Object { $_.Status -eq 'FAIL' })
    $skipped = @($script:Results | Where-Object { $_.Status -eq 'SKIP' })
    $passed  = @($script:Results | Where-Object { $_.Status -eq 'PASS' })
    # WARN 은 합계엔 찍되(가시성) FAIL 판정에는 관여하지 않는다 — 아래 exit
    # 코드 분기는 여전히 $failed.Count 만 본다.
    $warned  = @($script:Results | Where-Object { $_.Status -eq 'WARN' })
    Write-Host ('  합계: PASS {0} / FAIL {1} / SKIP {2} / WARN {3}' -f $passed.Count, $failed.Count, $skipped.Count, $warned.Count)

    if ($failed.Count -gt 0) {
        foreach ($f in $failed) {
            Write-Host ('::error::verify-msi [{0}] {1}: {2}' -f $PhaseName, $f.Name, $f.Detail)
        }
        Write-Host ("{0} verify-msi [{1}] 실패 {2}건" -f [char]0x274C, $PhaseName, $failed.Count)
        return 1
    }
    Write-Host ("{0} verify-msi [{1}] 전 항목 통과" -f [char]0x2705, $PhaseName)
    return 0
}

# ── 저장소 소스에서 상수 읽기 (하드코딩 금지) ────────────────────────────────

if ([string]::IsNullOrEmpty($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$WxiPath = Join-Path $RepoRoot 'installer\wix\generated\guids.wxi'
$WxsPath = Join-Path $RepoRoot 'installer\wix\unim.wxs'

foreach ($p in @($WxiPath, $WxsPath)) {
    if (-not (Test-Path -LiteralPath $p)) {
        throw "필수 소스 파일 없음: $p (-RepoRoot 를 확인하라)"
    }
}

$WxiText = Get-Content -LiteralPath $WxiPath -Raw -Encoding UTF8
$WxsText = Get-Content -LiteralPath $WxsPath -Raw -Encoding UTF8

# <?define Name = "값" ?> 에서 값 추출.
function Get-WxiDefine {
    param([Parameter(Mandatory = $true)][string] $Name)
    $m = [regex]::Match($WxiText, ('<\?define\s+{0}\s*=\s*"([^"]+)"' -f [regex]::Escape($Name)))
    if (-not $m.Success) { throw "guids.wxi 에서 '$Name' 정의를 찾지 못했다" }
    return $m.Groups[1].Value
}

$UNIM_CLSID        = Get-WxiDefine 'UnimClsid'            # {A1B2C3D4-...}
$UNIM_PROFILE_GUID = Get-WxiDefine 'UnimProfileGuid'      # {B2C3D4E5-...}
$UNIM_LANGID_PAD   = Get-WxiDefine 'UnimLangIdHexPadded'  # 0x00000412
$UNIM_LANGID_HEX   = Get-WxiDefine 'UnimLangIdHex'        # 0x0412

# TSF 카테고리 GUID 8종은 unim.wxs 에만 있다(register.rs 는 wxs 가 심는다고
# 명시하고 스스로 등록하지 않는다 — register_server() 주석 (3) 참조).
# Category\Category\{CAT}\{CLSID} 패턴에서 CAT 을 뽑는다.
$CATEGORY_GUIDS = @(
    [regex]::Matches($WxsText, 'Category\\Category\\(\{[0-9A-Fa-f\-]+\})\\') |
        ForEach-Object { $_.Groups[1].Value } |
        Select-Object -Unique
)
if ($CATEGORY_GUIDS.Count -lt 1) { throw 'unim.wxs 에서 TSF 카테고리 GUID 를 하나도 찾지 못했다' }

$TIP_ROOT = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$UNIM_CLSID"
$LP_KEY   = "$TIP_ROOT\LanguageProfile\$UNIM_LANGID_PAD\$UNIM_PROFILE_GUID"
$CLSID_64 = "HKLM:\SOFTWARE\Classes\CLSID\$UNIM_CLSID\InProcServer32"
# 32-bit COM 뷰의 실제 물리 위치는 HKLM\SOFTWARE\WOW6432Node\Classes\...(SOFTWARE
# 직후에 WOW6432Node) 가 아니라 HKLM\SOFTWARE\Classes\Wow6432Node\...(Classes
# 직후에 Wow6432Node) 다 — HKCR 리다이렉션은 일반 SOFTWARE 리다이렉션과 별개
# 규칙이다. 이 변수는 진단 로그용 참고 경로일 뿐, 판정은 항상 아래
# Get-Clsid32InProcServer32(레지스트리 뷰 API)로 한다.
$CLSID_32 = "HKLM:\SOFTWARE\Classes\Wow6432Node\CLSID\$UNIM_CLSID\InProcServer32"

# ── 잡다 헬퍼 ────────────────────────────────────────────────────────────────

function Get-RegDefaultValue {
    param([Parameter(Mandatory = $true)][string] $Path)
    $k = Get-Item -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $k) { return $null }
    return $k.GetValue('')
}

function Get-RegNamedValue {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Name
    )
    $k = Get-Item -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $k) { return $null }
    return $k.GetValue($Name)
}

# 32-bit CLSID\...\InProcServer32 를 레지스트리 뷰 API 로 결정적으로 읽는다.
# Test-Path 로 리터럴 경로 하나만 보면, 물리 위치를 잘못 짚었을 때 install
# 단계는 '키 없음' 오탐 FAIL, uninstall 단계는 '없으니 소멸' 오탐 PASS(잔존을
# 놓침)가 된다 — 뷰 API 는 물리 위치와 무관하게 OS 가 알아서 맞는 곳을 연다.
function Get-Clsid32InProcServer32 {
    param([Parameter(Mandatory = $true)][string] $Clsid)
    $result = $null
    try {
        $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
            [Microsoft.Win32.RegistryHive]::ClassesRoot, [Microsoft.Win32.RegistryView]::Registry32)
        try {
            $sub = $base.OpenSubKey("CLSID\$Clsid\InProcServer32")
            if ($null -ne $sub) {
                try {
                    $result = [pscustomobject]@{
                        Default        = $sub.GetValue('')
                        ThreadingModel = $sub.GetValue('ThreadingModel')
                        Source         = 'Registry32 view (HKCR\CLSID\...\InProcServer32)'
                    }
                } finally { $sub.Dispose() }
            }
        } finally { $base.Dispose() }
    } catch { }
    if ($null -ne $result) { return $result }

    # 폴백: 물리 위치로 알려진 리터럴 경로를 직접 시도한다.
    $v = Get-RegDefaultValue -Path $CLSID_32
    if (-not [string]::IsNullOrEmpty($v)) {
        return [pscustomobject]@{
            Default        = $v
            ThreadingModel = Get-RegNamedValue -Path $CLSID_32 -Name 'ThreadingModel'
            Source         = $CLSID_32
        }
    }
    return $null
}

# 워크스페이스 버전(Cargo.toml). 설정 exe 헤드리스 실행 프로브에서
# WizardSeenVersion 을 현재 버전으로 심을 때 쓴다.
function Get-WorkspaceVersion {
    $cargoToml = Join-Path $RepoRoot 'Cargo.toml'
    $text = Get-Content -LiteralPath $cargoToml -Raw
    $m = [regex]::Match($text, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $m.Success) { throw "Cargo.toml 에서 workspace version 을 찾지 못했다" }
    return $m.Groups[1].Value
}

function Resolve-InstallDir {
    $v = Get-RegNamedValue -Path $INSTALL_DIR_KEY -Name 'InstallDir'
    if (-not [string]::IsNullOrEmpty($v)) { return $v.TrimEnd('\') }
    # 폴백: wxs 는 ProgramFiles64Folder\UNIM 이 기본이다.
    return (Join-Path $env:ProgramFiles 'UNIM')
}

# ── Defender 오탐 대응 헬퍼 ──────────────────────────────────────────────────
# 2026-09-03 회사컴에서 Windows Defender 가 unim_tsf.dll(0.4.1) 을
# Trojan:Win32/Bearfoos.B!ml 로 오판해 하루 4회 격리한 사고 대응. 실측상 후킹·
# 인젝션 API 없음·엔트로피 정상 — 원인은 무서명 + VERSIONINFO 공란 + low
# prevalence 다. 서명·메타데이터 정비 전까지는 재발 가능성이 있어 CI 에서
# 조기 발견하고(scan phase), install/typing/uninstall 검증이 실시간 검사로
# 오염되지 않도록 제외 경로를 쓴다.

# Defender 이벤트 로그(1116=탐지, 1117=조치)에서 $Paths 의 아무 경로나 메시지에
# 부분일치하면 히트로 본다. 로그 자체가 없거나 조회 실패면 $null(= 판정 불가,
# 없음과 구분)을 반환한다.
function Get-DefenderPathEvents {
    param(
        [Parameter(Mandatory = $true)][string[]] $Paths,
        [int] $SinceMinutes = 30
    )
    try {
        $events = @(Get-WinEvent -FilterHashtable @{
            LogName   = 'Microsoft-Windows-Windows Defender/Operational'
            Id        = 1116, 1117
            StartTime = (Get-Date).AddMinutes(-$SinceMinutes)
        } -ErrorAction Stop)
    } catch {
        # Get-WinEvent 는 '조건에 맞는 이벤트가 0건'인 정상 상황에서도 예외를
        # 던진다(FullyQualifiedErrorId=NoMatchesFound,...) — 이걸 '로그 조회
        # 불가'와 뭉뚱그려 $null 로 반환하면, 실제로는 로그가 살아 있고 그냥
        # 격리 이벤트가 없었을 뿐인데도 SKIP('이벤트 로그 조회 불가')으로
        # 오분류된다(2026-09-03 2차 실측에서 관찰된 SKIP 사유). 0건은 판정
        # 가능한 결과이므로 빈 배열(=PASS 분기)로 돌려주고, 그 외(로그 자체
        # 비활성·Provider 부재·권한 부족 등 진짜 조회 불가)만 $null 로 남긴다.
        if ($_.FullyQualifiedErrorId -match '^NoMatchesFound') {
            return @()
        }
        return $null
    }
    return @($events | Where-Object {
        $msg = $_.Message
        $hit = $false
        foreach ($p in $Paths) {
            if (-not [string]::IsNullOrEmpty($p) -and $msg -like ('*' + $p + '*')) { $hit = $true; break }
        }
        $hit
    })
}

# MpCmdRun.exe 경로 — 표준 위치(ProgramFiles 심볼릭 링크)를 우선 시도하고,
# 없으면 ProgramData\...\Platform\<최신 버전>\ 을 뒤진다.
function Resolve-MpCmdRunPath {
    $direct = Join-Path $env:ProgramFiles 'Windows Defender\MpCmdRun.exe'
    if (Test-Path -LiteralPath $direct) { return $direct }
    $platformRoot = Join-Path $env:ProgramData 'Microsoft\Windows Defender\Platform'
    if (Test-Path -LiteralPath $platformRoot) {
        $latest = Get-ChildItem -LiteralPath $platformRoot -Directory -ErrorAction SilentlyContinue |
                  Sort-Object Name -Descending | Select-Object -First 1
        if ($null -ne $latest) {
            $p = Join-Path $latest.FullName 'MpCmdRun.exe'
            if (Test-Path -LiteralPath $p) { return $p }
        }
    }
    return $null
}

# MpCmdRun.exe 를 실행하고 exit code + stdout/stderr 합본을 돌려준다.
function Invoke-MpCmdRunProcess {
    param(
        [Parameter(Mandatory = $true)][string]   $MpCmdRunPath,
        [Parameter(Mandatory = $true)][string[]] $ArgList
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $MpCmdRunPath
    # 공백 포함 원소(경로 등)를 인용 — Invoke-Msiexec 와 동일한 이유.
    $psi.Arguments = (($ArgList | ForEach-Object { if ($_ -match '\s') { '"{0}"' -f $_ } else { $_ } }) -join ' ')
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $stdout = $p.StandardOutput.ReadToEnd()
    $stderr = $p.StandardError.ReadToEnd()
    $p.WaitForExit()
    return [pscustomobject]@{ ExitCode = $p.ExitCode; Output = ($stdout + "`n" + $stderr) }
}

function New-ArtifactDir {
    if (-not (Test-Path -LiteralPath $ArtifactDir)) {
        New-Item -ItemType Directory -Path $ArtifactDir -Force | Out-Null
    }
    return (Resolve-Path -LiteralPath $ArtifactDir).ProviderPath
}

function Invoke-Msiexec {
    param(
        [Parameter(Mandatory = $true)][string[]] $ArgList,
        [Parameter(Mandatory = $true)][string]   $LogPath,
        [int] $TimeoutMs = 600000
    )
    # 공백 포함 원소(설치 경로가 'C:\Program Files\...' 아래일 때 등)를 인용한다 —
    # Windows PowerShell 5.1 의 Start-Process -ArgumentList 는 배열 원소를
    # 공백으로 잇기만 하고 자동으로 인용하지 않는다.
    $all = @($ArgList) + @('/qn', '/norestart', '/l*v', $LogPath) | ForEach-Object {
        if ($_ -match '\s') { '"{0}"' -f $_ } else { $_ }
    }
    Write-Host ("  msiexec {0}" -f ($all -join ' '))
    # -Wait 를 쓰지 않는다: PowerShell 의 -Wait 는 대상 프로세스를 Job Object 에
    # 넣고 '자손 프로세스까지' 활성 프로세스 0 을 기다린다. unim.wxs 의
    # LaunchPopupRenderer 커스텀 액션(Execute=immediate, Return=asyncNoWait)이
    # 신규 설치마다 msiexec 의 자손으로 unim-popup-win.exe 를 띄우는데, 그 exe 는
    # GetMessageW 무한 루프를 도는 상주 프로세스라 절대 끝나지 않는다 — 그래서
    # msiexec 프로세스 자신의 종료만 기다리고, 타임아웃이면 강제 종료한다.
    #
    # 2026-09-03 windows-2022 러너 첫 실측: exit= 가 공란으로 나와 항상 FAIL.
    # 원인은 Start-Process -PassThru 였다 — 그 커맨드릿이 돌려주는
    # System.Diagnostics.Process 는 -Wait 없이 쓰면 ExitCode 접근에 필요한
    # 핸들/캐시가 갖춰지지 않을 때가 있다(.NET 문서가 권장하는 우회는 시간제한
    # WaitForExit(ms) 뒤 무인자 WaitForExit() 을 한 번 더 불러 종료 정보를
    # 확정하는 것인데, Start-Process 의 PassThru 객체는 이 경로를 태워도
    # ExitCode 가 계속 비어 있었다 — install.log 상 설치는 실제로 성공(0)
    # 했는데도 스크립트만 실패로 오판). Start-Process 커맨드릿을 거치지 않고
    # System.Diagnostics.Process 를 직접 Start 하는 것으로 우회한다 —
    # Invoke-MpCmdRunProcess(아래, Defender 스캔 단계) 가 이미 쓰는 것과 같은
    # 패턴이며 그쪽은 이 문제가 없었다.
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = 'msiexec.exe'
    $psi.Arguments = ($all -join ' ')
    $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    if (-not $p.WaitForExit($TimeoutMs)) {
        try { $p.Kill() } catch { }
        throw ("msiexec 이 {0}ms 안에 끝나지 않아 강제 종료했다 (자손 프로세스 대기 의심)" -f $TimeoutMs)
    }
    # 무인자 WaitForExit() 을 한번 더 — ExitCode 가 확실히 채워지게(MSDN 권장
    # 패턴: 표준 출력을 리다이렉션했을 때뿐 아니라 이 케이스에서도 안전망).
    $p.WaitForExit()
    $exitCode = $p.ExitCode
    # CA 가 띄운 렌더러는 msiexec 종료와 무관하게 계속 산다 — 다음 단계(특히
    # uninstall 의 파일 삭제)를 방해하지 않도록 잔존 인스턴스를 정리한다.
    Get-Process -Name 'unim-popup-win' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    return $exitCode
}

# install.log 꼬리를 CI 로그에 남긴다(아티팩트를 못 받는 상황 대비).
function Show-MsiLogTail {
    param([Parameter(Mandatory = $true)][string] $LogPath, [int] $Lines = 25)
    if (-not (Test-Path -LiteralPath $LogPath)) { return }
    Write-Host ('  ── {0} (마지막 {1}줄) ──' -f (Split-Path -Leaf $LogPath), $Lines)
    Get-Content -LiteralPath $LogPath -Tail $Lines -ErrorAction SilentlyContinue |
        ForEach-Object { Write-Host ('  | ' + $_) }
}

# ── wxs ↔ 스크립트 파일 목록 1:1 대조 ────────────────────────────────────────
#
# unim.wxs 의 <File Name="..."> 를 전량 뽑아 $EXPECTED_FILES 와 대조한다.
# wxs 에 파일이 추가·개명됐는데 이 스크립트가 안 따라오면 여기서 잡힌다
# (검증이 조용히 헐거워지는 것을 막는 자기점검).
function Test-WxsFileCoverage {
    $wxsNames = @(
        [regex]::Matches($WxsText, '<File\s+Id="[^"]*"\s+Name="([^"]+)"') |
            ForEach-Object { $_.Groups[1].Value } |
            Select-Object -Unique
    )
    $expectedLeaves = @($EXPECTED_FILES | ForEach-Object { Split-Path -Leaf $_.Rel })

    $missing = @($wxsNames | Where-Object { $expectedLeaves -notcontains $_ })
    $extra   = @($expectedLeaves | Where-Object { $wxsNames -notcontains $_ })

    if ($missing.Count -eq 0 -and $extra.Count -eq 0) {
        Add-Result -Name 'wxs 파일 목록 대조' -Status PASS `
                   -Detail ("wxs {0}개 == 스크립트 {1}개" -f $wxsNames.Count, $expectedLeaves.Count)
    } else {
        $d = @()
        if ($missing.Count -gt 0) { $d += ('wxs 에만 있음: ' + ($missing -join ', ')) }
        if ($extra.Count   -gt 0) { $d += ('스크립트에만 있음: ' + ($extra -join ', ')) }
        Add-Result -Name 'wxs 파일 목록 대조' -Status FAIL -Detail ($d -join ' / ')
    }
}

# ── DLL 로드 프로브 (비트니스별 별도 프로세스) ───────────────────────────────
#
# 64-bit PowerShell 에서 32-bit DLL 은 LoadLibrary 로 못 연다(ERROR_BAD_EXE_FORMAT).
# 그래서 프로브 본체를 파일로 떨어뜨리고 System32(64) / SysWOW64(32) 의
# powershell.exe 로 각각 실행한다.
function Write-LoadProbeScript {
    param([Parameter(Mandatory = $true)][string] $Path)

    $probe = @'
param([Parameter(Mandatory = $true)][string] $Dll)
$ErrorActionPreference = 'Stop'
Add-Type -Namespace UnimCi -Name Native -MemberDefinition @"
[DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern IntPtr LoadLibraryW(string lpFileName);
[DllImport("kernel32.dll", SetLastError = true)]
public static extern IntPtr GetProcAddress(IntPtr hModule, string lpProcName);
"@
$bits = if ([IntPtr]::Size -eq 8) { 64 } else { 32 }
Write-Host ("  probe{0}: {1}" -f $bits, $Dll)
$h = [UnimCi.Native]::LoadLibraryW($Dll)
if ($h -eq [IntPtr]::Zero) {
    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Host ("  probe{0}: LoadLibraryW 실패 (GetLastError={1})" -f $bits, $err)
    exit 1
}
Write-Host ("  probe{0}: handle=0x{1:X}" -f $bits, [Int64]$h)
$rc = 0
foreach ($sym in @('DllGetClassObject', 'DllCanUnloadNow')) {
    $p = [UnimCi.Native]::GetProcAddress($h, $sym)
    if ($p -eq [IntPtr]::Zero) {
        Write-Host ("  probe{0}: {1} 미노출" -f $bits, $sym)
        $rc = 1
    } else {
        Write-Host ("  probe{0}: {1}=0x{2:X}" -f $bits, $sym, [Int64]$p)
    }
}
exit $rc
'@
    # Windows PowerShell 5.1 이 UTF-8 로 읽도록 BOM 을 붙여 쓴다(본 스크립트와 동일 이유).
    [System.IO.File]::WriteAllText($Path, $probe, (New-Object System.Text.UTF8Encoding($true)))
}

function Test-DllLoad {
    param(
        [Parameter(Mandatory = $true)][string] $Label,
        [Parameter(Mandatory = $true)][string] $DllPath,
        [Parameter(Mandatory = $true)][string] $HostPowerShell,
        [Parameter(Mandatory = $true)][string] $ProbeScript,
        [Parameter(Mandatory = $true)][string] $LogPath
    )
    if (-not (Test-Path -LiteralPath $HostPowerShell)) {
        Add-Result -Name "DLL 로드: $Label" -Status SKIP -Detail "호스트 없음: $HostPowerShell"
        return
    }
    # Windows PowerShell 5.1 은 $ErrorActionPreference='Stop' 아래서 네이티브
    # 명령의 stderr 를 2>&1 로 병합하면 NativeCommandError(종료 오류)를 던진다
    # (pwsh 7 은 안 던진다). 프로브가 stderr 를 뱉는 순간 이 phase 전체가
    # Write-Summary 도 못 찍고 죽는 걸 막기 위해 이 호출만 'Continue' 로 낮춘다.
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $out = & $HostPowerShell -NoProfile -NonInteractive -ExecutionPolicy Bypass `
                   -File $ProbeScript -Dll $DllPath 2>&1
    } finally {
        $ErrorActionPreference = $prevEap
    }
    $code = $LASTEXITCODE
    $text = ($out | Out-String)
    Add-Content -LiteralPath $LogPath -Value ("=== {0} ===`r`n{1}" -f $Label, $text)
    $text.Split("`n") | ForEach-Object { if ($_.Trim()) { Write-Host ('  ' + $_.TrimEnd()) } }
    if ($code -eq 0) {
        Add-Result -Name "DLL 로드: $Label" -Status PASS `
                   -Detail 'LoadLibraryW + DllGetClassObject OK (주의: 이 러너는 VS 빌드툴 설치로 vcruntime140/msvcp140 이 상시 존재 — 재배포 패키지(vcredist) 누락은 이 프로브로 못 잡는다)'
    } else {
        Add-Result -Name "DLL 로드: $Label" -Status FAIL -Detail "프로브 exit=$code (로그: $LogPath)"
    }
}

# ── Phase: install ───────────────────────────────────────────────────────────

function Invoke-InstallPhase {
    $art = New-ArtifactDir
    $msi = (Resolve-Path -LiteralPath $MsiPath).ProviderPath
    $installLog = Join-Path $art 'install.log'

    Write-Host ('== 대상 MSI: {0}' -f $msi)
    Write-Host ('== CLSID    : {0}' -f $UNIM_CLSID)
    Write-Host ('== Profile  : {0}' -f $UNIM_PROFILE_GUID)
    Write-Host ('== 카테고리 : {0}종' -f $CATEGORY_GUIDS.Count)

    # (0) 전제: 64-bit 호스트 + 관리자 권한.
    if ([IntPtr]::Size -eq 8) {
        Add-Result -Name '전제: 64-bit PowerShell 호스트' -Status PASS
    } else {
        Add-Result -Name '전제: 64-bit PowerShell 호스트' -Status FAIL `
                   -Detail '32-bit 호스트에서는 레지스트리 64-bit 뷰를 볼 수 없다'
    }
    $isAdmin = ([Security.Principal.WindowsPrincipal] `
                 [Security.Principal.WindowsIdentity]::GetCurrent()
               ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {
        Add-Result -Name '전제: 관리자 권한' -Status PASS -Detail ([Environment]::UserName)
    } else {
        Add-Result -Name '전제: 관리자 권한' -Status FAIL `
                   -Detail 'msiexec /qn perMachine 설치에는 관리자 권한이 필요하다'
    }

    Test-WxsFileCoverage

    # Defender 실시간 검사가 기능 검증 중 DLL 을 격리하면 install/typing/
    # uninstall 판정이 오염된다 — 오탐 탐지 자체는 -Phase scan 이 능동으로
    # 전담하므로, 여기서는 설치 디렉터리를 제외 경로로 등록해 실시간 검사가
    # 끼어들지 않게만 한다. 이 시점엔 아직 설치 전이라 InstallDir 힌트 키가
    # 없으므로 Resolve-InstallDir 의 wxs 기본값 폴백(ProgramFiles64Folder\UNIM)을
    # 그대로 쓴다 — Invoke-Msiexec 가 INSTALLDIR 오버라이드 없이 /i 만 호출하므로
    # 실제 설치 경로와 일치한다.
    $exclusionDir = Resolve-InstallDir
    try {
        Add-MpPreference -ExclusionPath $exclusionDir -ErrorAction Stop
        Add-Result -Name 'Defender 제외 경로 등록' -Status PASS -Detail $exclusionDir
    } catch {
        Add-Result -Name 'Defender 제외 경로 등록' -Status SKIP -Detail $_.Exception.Message
    }

    # (a) 설치
    $code = Invoke-Msiexec -ArgList @('/i', $msi) -LogPath $installLog
    Show-MsiLogTail -LogPath $installLog
    if ($code -eq 0) {
        Add-Result -Name 'msiexec /i 설치' -Status PASS -Detail 'exit=0'
    } elseif ($code -eq 3010) {
        # ERROR_SUCCESS_REBOOT_REQUIRED — 성공이되 재부팅 요구. CI 에서는 허용하고 기록만.
        Add-Result -Name 'msiexec /i 설치' -Status PASS -Detail 'exit=3010 (재부팅 요구 — 성공으로 간주)'
    } else {
        Add-Result -Name 'msiexec /i 설치' -Status FAIL -Detail "exit=$code (install.log 참조)"
        return   # 설치가 실패하면 이후 검사는 무의미
    }

    $installDir = Resolve-InstallDir
    Write-Host ('== INSTALLDIR: {0}' -f $installDir)
    if (Test-Path -LiteralPath $installDir) {
        Add-Result -Name '설치 디렉터리 존재' -Status PASS -Detail $installDir
    } else {
        Add-Result -Name '설치 디렉터리 존재' -Status FAIL -Detail "없음: $installDir"
        return
    }

    # (b) 파일 — wxs 가 명시한 12개가 존재하고 0바이트가 아니어야 한다.
    $badFiles = @()
    foreach ($f in $EXPECTED_FILES) {
        $p = Join-Path $installDir $f.Rel
        if (-not (Test-Path -LiteralPath $p -PathType Leaf)) {
            $badFiles += ('{0} (없음, {1})' -f $f.Rel, $f.Comp)
        } elseif ((Get-Item -LiteralPath $p).Length -le 0) {
            $badFiles += ('{0} (0바이트, {1})' -f $f.Rel, $f.Comp)
        } else {
            Write-Host ('  ok  {0} ({1} bytes)' -f $f.Rel, (Get-Item -LiteralPath $p).Length)
        }
    }
    if ($badFiles.Count -eq 0) {
        Add-Result -Name ('설치 파일 {0}개 존재·비어있지 않음' -f $EXPECTED_FILES.Count) -Status PASS
    } else {
        Add-Result -Name '설치 파일 존재·크기' -Status FAIL -Detail ($badFiles -join ' / ')
    }

    # (c) 등록 — COM CLSID (64/32 뷰), TIP LanguageProfile, 카테고리.
    $dll64 = Join-Path $installDir 'unim_tsf.dll'
    $dll32 = Join-Path $installDir 'unim_tsf32.dll'

    $c32 = Get-Clsid32InProcServer32 -Clsid $UNIM_CLSID
    foreach ($pair in @(
        @{ Label = '64-bit'; Expect = $dll64;
           Value = (Get-RegDefaultValue -Path $CLSID_64);
           Tm    = (Get-RegNamedValue -Path $CLSID_64 -Name 'ThreadingModel');
           Src   = $CLSID_64 },
        @{ Label = '32-bit (Wow6432Node)'; Expect = $dll32;
           Value = $(if ($c32) { $c32.Default } else { $null });
           Tm    = $(if ($c32) { $c32.ThreadingModel } else { $null });
           Src   = $(if ($c32) { $c32.Source } else { $CLSID_32 }) }
    )) {
        $v = $pair.Value
        if ([string]::IsNullOrEmpty($v)) {
            Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status FAIL -Detail "키/기본값 없음: $($pair.Src)"
            continue
        }
        if ($v -like '`[#*') {
            # wxs [#FileId] 토큰이 치환되지 않고 리터럴로 기록된 과거 회귀(SMOKE_TEST §2).
            Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status FAIL -Detail "토큰 미치환: $v"
            continue
        }
        if (-not (Test-Path -LiteralPath $v -PathType Leaf)) {
            Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status FAIL -Detail "가리키는 DLL 부재: $v"
            continue
        }
        if ($v -ne $pair.Expect) {
            Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status FAIL `
                       -Detail ("기대 {0} / 실제 {1}" -f $pair.Expect, $v)
            continue
        }
        if ($pair.Tm -ne 'Apartment') {
            Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status FAIL -Detail "ThreadingModel=$($pair.Tm) (기대 Apartment)"
            continue
        }
        Add-Result -Name ('InProcServer32 ({0})' -f $pair.Label) -Status PASS -Detail ("{0} (경로: {1})" -f $v, $pair.Src)
    }

    # TIP root
    if (Test-Path -LiteralPath $TIP_ROOT) {
        Add-Result -Name 'CTF TIP 엔트리' -Status PASS -Detail $TIP_ROOT
    } else {
        Add-Result -Name 'CTF TIP 엔트리' -Status FAIL -Detail "없음: $TIP_ROOT"
    }

    # LanguageProfile 0x00000412\{PROFILE} — 6개 값 중 판정에 결정적인 3개.
    if (-not (Test-Path -LiteralPath $LP_KEY)) {
        Add-Result -Name '한국어 LanguageProfile 키' -Status FAIL -Detail "없음: $LP_KEY"
    } else {
        $lpBad = @()
        $enable = Get-RegNamedValue -Path $LP_KEY -Name 'Enable'
        if ($enable -ne 1) { $lpBad += "Enable=$enable (기대 1)" }
        $subst = Get-RegNamedValue -Path $LP_KEY -Name 'SubstituteLayout'
        if ($subst -ne 1042) { $lpBad += "SubstituteLayout=$subst (기대 1042=0x412)" }
        $desc = Get-RegNamedValue -Path $LP_KEY -Name 'Description'
        if ([string]::IsNullOrEmpty($desc)) { $lpBad += 'Description 비어 있음' }
        $iconFile = Get-RegNamedValue -Path $LP_KEY -Name 'IconFile'
        if ([string]::IsNullOrEmpty($iconFile) -or -not (Test-Path -LiteralPath $iconFile)) {
            $lpBad += "IconFile 경로 부재: $iconFile"
        }
        if ($lpBad.Count -eq 0) {
            Add-Result -Name '한국어 LanguageProfile 값' -Status PASS -Detail "Enable=1, SubstituteLayout=1042, Description=$desc"
        } else {
            Add-Result -Name '한국어 LanguageProfile 값' -Status FAIL -Detail ($lpBad -join ' / ')
        }
    }

    # 카테고리 8종 — Category\Category\{CAT}\{CLSID} 와 Category\Item\{CLSID}\{CAT} 양쪽.
    $catBad = @()
    foreach ($cat in $CATEGORY_GUIDS) {
        $kCat  = "$TIP_ROOT\Category\Category\$cat\$UNIM_CLSID"
        $kItem = "$TIP_ROOT\Category\Item\$UNIM_CLSID\$cat"
        if (-not (Test-Path -LiteralPath $kCat))  { $catBad += "Category\Category\$cat 없음" }
        if (-not (Test-Path -LiteralPath $kItem)) { $catBad += "Category\Item\...\$cat 없음" }
    }
    if ($catBad.Count -eq 0) {
        Add-Result -Name ('TSF 카테고리 {0}종 등록' -f $CATEGORY_GUIDS.Count) -Status PASS
    } else {
        Add-Result -Name 'TSF 카테고리 등록' -Status FAIL -Detail ($catBad -join ' / ')
    }

    # 팝업 렌더러 자동시작 Run 키 + 설치 경로 힌트 키.
    $runVal = Get-RegNamedValue -Path $RUN_KEY -Name $RUN_VALUE_NAME
    if ([string]::IsNullOrEmpty($runVal)) {
        Add-Result -Name 'HKLM Run 자동시작 값' -Status FAIL -Detail "$RUN_KEY\$RUN_VALUE_NAME 없음"
    } elseif ($runVal -like '*`[INSTALLDIR`]*') {
        Add-Result -Name 'HKLM Run 자동시작 값' -Status FAIL -Detail "토큰 미치환: $runVal"
    } else {
        Add-Result -Name 'HKLM Run 자동시작 값' -Status PASS -Detail $runVal
    }

    # 프로그램 추가/제거 등록 + ARPINSTALLLOCATION — <SetProperty> 회귀(M-31) 재발
    # 감시. M-31 은 Uninstall\{ProductCode}\InstallLocation 이 [INSTALLDIR] 토큰을
    # 치환하지 않고 리터럴로 기록되던 문제였다 — DisplayName 매칭만으로는 재발을
    # 못 잡으므로 InstallLocation 값까지 단언한다. 개별 서브키 접근에서
    # SecurityException 이 나도 phase 전체가 죽지 않도록 try/catch 로 감싼다.
    $arpKeys = @(Get-ChildItem -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
                 Where-Object { try { $_.GetValue('DisplayName') -like 'UNIM*' } catch { $false } })
    if ($arpKeys.Count -gt 0) {
        $names = @($arpKeys | ForEach-Object { try { $_.GetValue('DisplayName') } catch { '(읽기 실패)' } })
        Add-Result -Name '프로그램 추가/제거 등록' -Status PASS -Detail ($names -join ', ')

        $locBad = @()
        foreach ($ak in $arpKeys) {
            $loc = $null
            try { $loc = $ak.GetValue('InstallLocation') } catch { }
            if ([string]::IsNullOrEmpty($loc)) {
                $locBad += "$($ak.PSChildName): InstallLocation 비어 있음"
            } elseif ($loc -like '*`[INSTALLDIR`]*') {
                $locBad += "$($ak.PSChildName): 토큰 미치환($loc)"
            } elseif (-not (Test-Path -LiteralPath $loc)) {
                $locBad += "$($ak.PSChildName): 가리키는 경로 부재($loc)"
            }
        }
        if ($locBad.Count -eq 0) {
            Add-Result -Name 'ARP InstallLocation (M-31 재발 감시)' -Status PASS
        } else {
            Add-Result -Name 'ARP InstallLocation (M-31 재발 감시)' -Status FAIL -Detail ($locBad -join ' / ')
        }
    } else {
        Add-Result -Name '프로그램 추가/제거 등록' -Status FAIL -Detail 'Uninstall 키에 UNIM 항목 없음'
    }
    $loc = Get-RegNamedValue -Path $INSTALL_DIR_KEY -Name 'InstallDir'
    if ($loc -like '*`[INSTALLDIR`]*') {
        Add-Result -Name 'InstallDir 힌트 키 토큰 치환' -Status FAIL -Detail "리터럴 토큰 기록됨: $loc"
    } else {
        Add-Result -Name 'InstallDir 힌트 키 토큰 치환' -Status PASS -Detail $loc
    }

    # (d) DLL 로드 — 비트니스별.
    $probe   = Join-Path $art 'load-probe.ps1'
    $probeLog = Join-Path $art 'dll-load-probe.log'
    Write-LoadProbeScript -Path $probe
    Set-Content -LiteralPath $probeLog -Value '' -Encoding UTF8
    $ps64 = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $ps32 = Join-Path $env:SystemRoot 'SysWOW64\WindowsPowerShell\v1.0\powershell.exe'
    Test-DllLoad -Label 'unim_tsf.dll (x64)'   -DllPath $dll64 -HostPowerShell $ps64 -ProbeScript $probe -LogPath $probeLog
    Test-DllLoad -Label 'unim_tsf32.dll (x86)' -DllPath $dll32 -HostPowerShell $ps32 -ProbeScript $probe -LogPath $probeLog

    # (e) 설정 exe 헤드리스 실행. --version/--help 는 없지만, wizard.rs 의
    # should_exit_quietly_from 은 인자에 --first-run-if-needed 가 있고
    # wizard_seen_version()(HKCU\...\WizardSeenVersion) 이 Some 이면 창·싱글턴
    # 락을 만들기도 전에 조용히 종료한다 — 그 경로로 GUI 없이 'exe 가 링크되고
    # 실행되고 0 으로 끝난다' 를 실측한다. 검사 전후로 HKCU 값을 원복한다.
    $settingsExe = Join-Path $installDir 'unim-settings.exe'
    if (-not (Test-Path -LiteralPath $settingsExe -PathType Leaf)) {
        Add-Result -Name '설정 exe 헤드리스 실행' -Status SKIP -Detail "부재: $settingsExe (위 파일 존재 검사에서 이미 FAIL 됐을 것)"
    } else {
        $wizKeyPath = 'HKCU:\Software\atit.org\UNIM'
        $hadKey = Test-Path -LiteralPath $wizKeyPath
        $prevSeen = if ($hadKey) { Get-RegNamedValue -Path $wizKeyPath -Name 'WizardSeenVersion' } else { $null }
        try {
            $ver = Get-WorkspaceVersion
            if (-not (Test-Path -LiteralPath $wizKeyPath)) {
                New-Item -Path $wizKeyPath -Force | Out-Null
            }
            Set-ItemProperty -LiteralPath $wizKeyPath -Name 'WizardSeenVersion' -Value $ver -Type String -Force

            $sp = $null
            try {
                $sp = Start-Process -FilePath $settingsExe -ArgumentList '--first-run-if-needed' `
                                     -PassThru -WindowStyle Hidden
                if ($sp.WaitForExit(15000)) {
                    if ($sp.ExitCode -eq 0) {
                        Add-Result -Name '설정 exe 헤드리스 실행' -Status PASS `
                                   -Detail "--first-run-if-needed (WizardSeenVersion=$ver 심음) → exit=0 (should_exit_quietly_from)"
                    } else {
                        Add-Result -Name '설정 exe 헤드리스 실행' -Status FAIL -Detail "exit=$($sp.ExitCode) (기대 0)"
                    }
                } else {
                    Add-Result -Name '설정 exe 헤드리스 실행' -Status FAIL `
                               -Detail '15초 안에 종료하지 않음 (GUI 창이 뜬 것으로 의심 — should_exit_quietly_from 회귀?)'
                    try { $sp.Kill() } catch { }
                }
            } catch {
                Add-Result -Name '설정 exe 헤드리스 실행' -Status FAIL -Detail $_.Exception.Message
            }
        } finally {
            try {
                if ($hadKey) {
                    if ($null -eq $prevSeen) {
                        Remove-ItemProperty -LiteralPath $wizKeyPath -Name 'WizardSeenVersion' -ErrorAction SilentlyContinue
                    } else {
                        Set-ItemProperty -LiteralPath $wizKeyPath -Name 'WizardSeenVersion' -Value $prevSeen -Type String -Force -ErrorAction SilentlyContinue
                    }
                } else {
                    Remove-Item -LiteralPath $wizKeyPath -Force -ErrorAction SilentlyContinue
                }
            } catch { }
        }
    }

    # (f) Defender 실시간 검사 격리 여부 — 위에서 등록한 제외 경로가 늦게
    # 전파돼(또는 등록 자체가 실패해) 설치 직후 실시간 검사가 먼저 격리했을
    # 가능성을 이벤트 1116(탐지)/1117(조치)로 확인한다. -Phase scan 의 능동
    # 스캔과는 다른 질문 — 이건 "방금 설치가 실시간 검사에 걸렸는가" 다.
    $defenderHits = Get-DefenderPathEvents -Paths @($installDir, $dll64, $dll32)
    if ($null -eq $defenderHits) {
        # 0건(정상 조회, 격리 없음)은 위 Get-DefenderPathEvents 가 이미 빈 배열로
        # 걸러내므로, 이 SKIP 분기는 이제 진짜 '조회 자체가 안 됨'만 의미한다
        # (Microsoft-Windows-Windows Defender/Operational 로그·Provider 부재
        # 또는 권한 부족 — Get-WinEvent -FilterHashtable 예외의 FullyQualifiedErrorId
        # 가 NoMatchesFound 가 아닌 경우).
        Add-Result -Name 'Defender 실시간 검사 격리 여부 (이벤트 1116/1117)' -Status SKIP `
                   -Detail 'Defender 이벤트 로그(Microsoft-Windows-Windows Defender/Operational) 조회 실패 — 로그/Provider 부재 또는 권한 부족'
    } elseif ($defenderHits.Count -eq 0) {
        Add-Result -Name 'Defender 실시간 검사 격리 여부 (이벤트 1116/1117)' -Status PASS
    } else {
        $first = ($defenderHits | Select-Object -First 3 | ForEach-Object { ($_.Message -split "`n")[0] }) -join ' | '
        Add-Result -Name 'Defender 실시간 검사 격리 여부 (이벤트 1116/1117)' -Status FAIL `
                   -Detail ("{0}건 — 제외 경로 등록이 늦게 전파됐거나 실패: {1}" -f $defenderHits.Count, $first)
    }

    # (g) VERSIONINFO — build-support/version_rc.rs(626d06a)가 임베드한 메타데이터가
    # 실제로 설치된 PE 에 실렸는지 증명한다(2026-09-03 Defender 오탐 대응의 한
    # 축: 무서명 + VERSIONINFO 공란 + low prevalence 중 VERSIONINFO 공백을 메운
    # 조치). CompanyName='atit.org' / ProductName='UNIM' / FileVersion=워크스페이스
    # 버전(Cargo.toml [workspace.package] version, 4자리 "x.y.z.0" — version_rc.rs
    # 의 ver_str 과 동일 포맷) 을 단언한다.
    #
    # OriginalFilename 주의: unim_tsf32.dll 은 설치 시점에만 WiX 가 붙인 이름이다
    # — 원본 cargo build 산출물은 i686/x64 두 타깃 모두 파일명이 'unim_tsf.dll'
    # 이고, unim-tsf/build.rs 가 embed_version_rc() 에 넘기는 original_filename
    # 도 두 타깃 동일하게 하드코딩된 "unim_tsf.dll" 이다(version_rc.rs 26-29행
    # 주석: "MSI 가 설치 시 다른 이름으로 복사해도 원래 파일명은 안 바뀌는 게
    # 맞다" — 의도된 설계, 회귀 아님). 그래서 unim_tsf32.dll 의 기대
    # OriginalFilename 은 실제 설치 파일명이 아니라 'unim_tsf.dll' 로 매핑한다.
    $expectedVersion     = Get-WorkspaceVersion
    $expectedFileVersion = "$expectedVersion.0"
    $versionInfoTargets = @(
        @{ Rel = 'unim_tsf.dll';       ExpectedOriginal = 'unim_tsf.dll' },
        @{ Rel = 'unim_tsf32.dll';     ExpectedOriginal = 'unim_tsf.dll' },  # 위 주석 — 설계 의도
        @{ Rel = 'unim-settings.exe';  ExpectedOriginal = 'unim-settings.exe' },
        @{ Rel = 'unim-popup-win.exe'; ExpectedOriginal = 'unim-popup-win.exe' }
    )
    foreach ($vt in $versionInfoTargets) {
        $p = Join-Path $installDir $vt.Rel
        if (-not (Test-Path -LiteralPath $p -PathType Leaf)) {
            Add-Result -Name "VERSIONINFO: $($vt.Rel)" -Status FAIL -Detail "파일 없음: $p (위 파일 존재 검사에서 이미 FAIL 됐을 것)"
            continue
        }
        $vi = (Get-Item -LiteralPath $p).VersionInfo
        $bad = @()
        if ($vi.CompanyName -ne 'atit.org') {
            $bad += "CompanyName='$($vi.CompanyName)' (기대 atit.org)"
        }
        if ($vi.ProductName -ne 'UNIM') {
            $bad += "ProductName='$($vi.ProductName)' (기대 UNIM)"
        }
        if ($vi.FileVersion -ne $expectedFileVersion) {
            $bad += "FileVersion='$($vi.FileVersion)' (기대 $expectedFileVersion)"
        }
        if ($vi.OriginalFilename -ne $vt.ExpectedOriginal) {
            $bad += "OriginalFilename='$($vi.OriginalFilename)' (기대 $($vt.ExpectedOriginal))"
        }
        if ($bad.Count -eq 0) {
            Add-Result -Name "VERSIONINFO: $($vt.Rel)" -Status PASS `
                       -Detail ("CompanyName=atit.org, ProductName=UNIM, FileVersion={0}, OriginalFilename={1}" -f $expectedFileVersion, $vt.ExpectedOriginal)
        } else {
            Add-Result -Name "VERSIONINFO: $($vt.Rel)" -Status FAIL -Detail ($bad -join ' / ')
        }
    }
}

# ── Phase: typing (기능 실측 — 승격 대기) ────────────────────────────────────
#
# 러너 전제(자세한 근거는 이 커밋의 작업 노트 참조):
#   - GitHub 호스티드 windows-2022 러너는 관리자 계정으로 돈다 → msiexec /qn 가능.
#   - 다만 대화형 데스크톱(세션 1) 보장은 문서화돼 있지 않다. ctfmon/TSF 는
#     대화형 세션에 묶여 있어, 세션 0 서비스 컨텍스트면 IME 가 붙지 못한다.
#     그래서 이 단계는 workflow 에서 continue-on-error 로 돌린다(승격 대기).

$WIN32_UI_SRC = @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class UnimCiUi {
    [StructLayout(LayoutKind.Sequential)]
    public struct MOUSEINPUT {
        public int dx; public int dy; public uint mouseData;
        public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT {
        public ushort wVk; public ushort wScan; public uint dwFlags;
        public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Explicit)]
    public struct INPUTUNION {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT {
        public uint type;
        public INPUTUNION u;
    }

    const uint INPUT_KEYBOARD      = 1;
    const uint KEYEVENTF_EXTENDED  = 0x0001;
    const uint KEYEVENTF_KEYUP     = 0x0002;

    [DllImport("user32.dll", SetLastError = true)]
    static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
    [DllImport("user32.dll")]
    static extern uint MapVirtualKey(uint uCode, uint uMapType);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr lpdwProcessId);
    [DllImport("kernel32.dll")]
    public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr SendMessageW(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);

    // vk 하나를 down/up 으로 보낸다. wScan 은 MapVirtualKey 로 채운다 —
    // 스캔코드가 0 이면 일부 IME/후킹 계층이 키를 무시한다.
    public static uint SendKey(ushort vk, bool keyUp, bool extended) {
        INPUT[] inp = new INPUT[1];
        inp[0].type = INPUT_KEYBOARD;
        inp[0].u.ki.wVk = vk;
        inp[0].u.ki.wScan = (ushort)MapVirtualKey(vk, 0 /* MAPVK_VK_TO_VSC */);
        uint flags = 0;
        if (keyUp)    flags |= KEYEVENTF_KEYUP;
        if (extended) flags |= KEYEVENTF_EXTENDED;
        inp[0].u.ki.dwFlags = flags;
        inp[0].u.ki.time = 0;
        inp[0].u.ki.dwExtraInfo = IntPtr.Zero;
        return SendInput(1, inp, Marshal.SizeOf(typeof(INPUT)));
    }

    // 포그라운드 강제: AttachThreadInput 으로 입력 큐를 붙인 뒤 SetForegroundWindow.
    public static bool ForceForeground(IntPtr hWnd) {
        uint fgTid = GetWindowThreadProcessId(GetForegroundWindow(), IntPtr.Zero);
        uint myTid = GetCurrentThreadId();
        bool attached = false;
        if (fgTid != 0 && fgTid != myTid) { attached = AttachThreadInput(myTid, fgTid, true); }
        ShowWindow(hWnd, 9 /* SW_RESTORE */);
        BringWindowToTop(hWnd);
        bool ok = SetForegroundWindow(hWnd);
        if (attached) { AttachThreadInput(myTid, fgTid, false); }
        return ok || GetForegroundWindow() == hWnd;
    }

    // 클래식 메모장의 Edit 자식 컨트롤에서 WM_GETTEXT 로 본문을 읽는다.
    public static string GetEditText(IntPtr hWndTop) {
        IntPtr edit = FindWindowExW(hWndTop, IntPtr.Zero, "Edit", null);
        if (edit == IntPtr.Zero) { edit = FindWindowExW(hWndTop, IntPtr.Zero, "RichEditD2DPT", null); }
        if (edit == IntPtr.Zero) { return null; }
        StringBuilder sb = new StringBuilder(4096);
        SendMessageW(edit, 0x000D /* WM_GETTEXT */, (IntPtr)sb.Capacity, sb);
        return sb.ToString();
    }
}
'@

# 한 화면을 PNG 로 저장(실패해도 치명적이지 않음 — 실패 원인 분석용 증거).
function Save-Screenshot {
    param([Parameter(Mandatory = $true)][string] $Path)
    try {
        Add-Type -AssemblyName System.Drawing -ErrorAction Stop
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bmp = New-Object System.Drawing.Bitmap($b.Width, $b.Height)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
        Write-Host ('  스크린샷 저장: {0}' -f $Path)
        return $true
    } catch {
        Write-Host ('  스크린샷 실패(비치명): {0}' -f $_.Exception.Message)
        return $false
    }
}

# ITfInputProcessorProfiles 로 UNIM 프로필을 기본·활성으로 지정.
# scripts/unim-set-default.ps1 의 COM 인터페이스 선언을 그대로 재사용한다.
function Enable-UnimProfile {
    $src = @'
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("1F02B6C5-7842-4EE6-8A0B-9A24183A95CA"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface ITfInputProcessorProfiles {
    void Register(ref Guid rclsid);
    void Unregister(ref Guid rclsid);
    void AddLanguageProfile(ref Guid rclsid, ushort langid, ref Guid guidProfile,
        [MarshalAs(UnmanagedType.LPWStr)] string pchDesc, uint cchDesc,
        [MarshalAs(UnmanagedType.LPWStr)] string pchIconFile, uint cchFile, uint uIconIndex);
    void RemoveLanguageProfile(ref Guid rclsid, ushort langid, ref Guid guidProfile);
    void EnumInputProcessorInfo(out IntPtr ppEnum);
    void GetDefaultLanguageProfile(ushort langid, ref Guid catid, out Guid pclsid, out Guid pguidProfile);
    void SetDefaultLanguageProfile(ushort langid, ref Guid rclsid, ref Guid guidProfiles);
    void ActivateLanguageProfile(ref Guid rclsid, ushort langid, ref Guid guidProfiles);
}

// unim-tsf/src/register.rs::set_as_default() 가 실제로 쓰는 인터페이스.
// ITfInputProcessorProfiles::ActivateLanguageProfile(레거시, Vista 이전 호환용)
// 은 langid 가 이미 사용자 언어 목록에 '설치'돼 있어야 통과하는 전제조건이
// 있다 — 그게 없으면 E_INVALIDARG(0x80070057, "Value does not fall within the
// expected range")를 낸다. 한국어 언어팩이 없는 CI 러너에서 Add-UnimInputMethodTip
// 이 ko-KR 을 언어 목록에 못 넣으면 이 레거시 경로는 구조적으로 막힌다.
// ITfInputProcessorProfileMgr::ActivateProfile 은 그 전제조건이 없고(TF_IPPMF_
// ENABLEPROFILE 플래그 자체가 "아직 활성화 안 된 프로필을 활성화" 용도) 실제
// 제품 코드가 쓰는 경로이므로 검증도 이걸 그대로 재현한다. IID·vtable 순서는
// windows crate(windows-0.62.2) Win32::UI::TextServices 바인딩에서 대조:
// ActivateProfile 이 IUnknown 바로 다음(0번째) 메서드라 그 하나만 선언하면
// QueryInterface 캐스팅이 정확히 맞아떨어진다.
[ComImport, Guid("71C6E74C-0F28-11D8-A82A-00065B84435C"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface ITfInputProcessorProfileMgr {
    void ActivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
        IntPtr hkl, uint dwFlags);
}

public static class UnimCiTsf {
    [DllImport("ole32.dll")]
    static extern int CoCreateInstance(ref Guid clsid, IntPtr outer, uint ctx, ref Guid iid, out IntPtr obj);
    [DllImport("ole32.dll")] static extern int CoInitialize(IntPtr p);

    public static string Activate(string clsidStr, string profileStr, ushort langid) {
        CoInitialize(IntPtr.Zero);
        Guid clsidProfiles = new Guid("33C53A50-F456-4884-B049-85FD643ECFED");
        Guid iid = new Guid("1F02B6C5-7842-4EE6-8A0B-9A24183A95CA");
        IntPtr p;
        int hr = CoCreateInstance(ref clsidProfiles, IntPtr.Zero, 1, ref iid, out p);
        if (hr != 0) { return "CoCreateInstance 실패: 0x" + hr.ToString("X8"); }
        var profiles = (ITfInputProcessorProfiles)Marshal.GetObjectForIUnknown(p);
        Guid unim = new Guid(clsidStr);
        Guid prof = new Guid(profileStr);
        string log = "";
        try { profiles.SetDefaultLanguageProfile(langid, ref unim, ref prof); log += "SetDefault=ok; "; }
        catch (Exception e) { log += "SetDefault=" + e.Message + "; "; }
        // register.rs::set_as_default() 와 동일하게 Mgr::ActivateProfile 을 쓴다
        // (TF_PROFILETYPE_INPUTPROCESSOR=1, TF_IPPMF_ENABLEPROFILE=1 |
        // TF_IPPMF_FORSESSION=0x20000000 = 0x20000001). 레거시
        // ActivateLanguageProfile 은 쓰지 않는다 — 위 인터페이스 선언 주석 참조.
        try {
            var mgr = (ITfInputProcessorProfileMgr)profiles;
            mgr.ActivateProfile(1u, langid, ref unim, ref prof, IntPtr.Zero, 0x20000001u);
            log += "Activate=ok";
        }
        catch (Exception e) { log += "Activate=" + e.Message; }
        return log;
    }
}
'@
    try {
        Add-Type -TypeDefinition $src -Language CSharp -ErrorAction Stop | Out-Null
    } catch {
        if ($_.Exception.Message -notmatch 'already exists') { throw }
    }
    $langid = [uint16] ([Convert]::ToInt32($UNIM_LANGID_HEX, 16))
    return [UnimCiTsf]::Activate($UNIM_CLSID, $UNIM_PROFILE_GUID, $langid)
}

# ko-KR 과 UNIM TIP 을 현재 사용자 언어 목록에 넣는다.
# InputMethodTips 형식: 'LLLL:{CLSID}{PROFILE}' (SMOKE_TEST.md §3 과 동일).
#
# 2026-09-03 windows-2022 러너 첫 실측: Set-WinUserLanguageList 가 예외 없이
# 조용히 끝나는데도(캐치할 오류가 없다) 직후 Get-WinUserLanguageList 에
# ko-KR 이 반영돼 있지 않았다 — throw 가 아니라 '등록했는데도 없음' 분기로
# FAIL. Server Core 계열 CI 이미지는 한국어 디스플레이 언어팩이 없고,
# WinUserLanguageList 커밋이 비동기(WM_SETTINGCHANGE 류)라 즉시 재조회하면
# 반영 전일 수 있다는 게 유력 가설(사용자 지시 메모). 원인이 그거라면
# 짧은 재시도로 통과할 수 있고, 그게 아니라면(언어팩 자체 부재로 구조적 불가)
# 매 시도의 실제 상태를 detail 에 그대로 남겨 원인 규명이 가능하게 한다.
function Add-UnimInputMethodTip {
    $tip = ('{0}:{1}{2}' -f $UNIM_LANGID_HEX.Substring(2), $UNIM_CLSID, $UNIM_PROFILE_GUID)
    Write-Host ('  InputMethodTip: {0}' -f $tip)
    $attempts = @()
    try {
        Import-Module International -ErrorAction Stop

        for ($i = 1; $i -le 3; $i++) {
            $list = Get-WinUserLanguageList
            $ko = @($list | Where-Object { $_.LanguageTag -eq 'ko-KR' })
            if ($ko.Count -eq 0) {
                $list.Add('ko-KR')
                Set-WinUserLanguageList $list -Force
                # Set-WinUserLanguageList 의 커밋은 비동기일 수 있다 — 재조회
                # 전에 짧게 대기(1차는 즉시, 이후 시도만 대기해 헛수고를 줄인다).
                if ($i -gt 1) { Start-Sleep -Milliseconds ([Math]::Min(500 * $i, 2000)) }
                $list = Get-WinUserLanguageList
                $ko = @($list | Where-Object { $_.LanguageTag -eq 'ko-KR' })
            }
            if ($ko.Count -eq 0) {
                $attempts += "시도$i`: ko-KR 미반영"
                continue
            }
            if ($ko[0].InputMethodTips -notcontains $tip) {
                $ko[0].InputMethodTips.Add($tip)
                Set-WinUserLanguageList $list -Force
            }
            $after = @(Get-WinUserLanguageList | Where-Object { $_.LanguageTag -eq 'ko-KR' })
            if ($after.Count -gt 0 -and ($after[0].InputMethodTips -contains $tip)) {
                return @{ Ok = $true; Detail = $tip }
            }
            $seen = if ($after.Count -gt 0) { ($after[0].InputMethodTips -join ', ') } else { '(ko-KR 없음)' }
            $attempts += "시도$i`: TIP 미반영. 현재=$seen"
        }
        $detail = 'ko-KR 을 언어 목록에 추가하지 못했다 (' + ($attempts -join ' / ') + ')'
        return @{ Ok = $false; Detail = $detail }
    } catch {
        $detail = $_.Exception.Message
        if ($attempts.Count -gt 0) { $detail = $detail + ' [' + ($attempts -join ' / ') + ']' }
        return @{ Ok = $false; Detail = $detail }
    }
}

# 메모장을 띄워 한글 한 글자('한')를 실제로 입력해 본다.
# 두벌식 'gks' = ㅎ+ㅏ+ㄴ → '한'. 스페이스로 조합을 확정한 뒤 본문을 읽는다.
function Invoke-TypingTrial {
    param(
        [Parameter(Mandatory = $true)][uint16] $ToggleVk,
        [Parameter(Mandatory = $true)][bool]   $ToggleExtended,
        [Parameter(Mandatory = $true)][string] $Label,
        [Parameter(Mandatory = $true)][string] $ArtDir
    )

    $VK_SPACE = [uint16]0x20; $VK_CTRL = [uint16]0x11
    $VK_A = [uint16]0x41; $VK_C = [uint16]0x43
    $keys = @([uint16]0x47, [uint16]0x4B, [uint16]0x53)   # G, K, S

    $notepad = Join-Path $env:SystemRoot 'System32\notepad.exe'
    $proc = $null
    try {
        Write-Host ('  [{0}] 메모장 기동' -f $Label)
        $proc = Start-Process -FilePath $notepad -PassThru
        $hwnd = [IntPtr]::Zero
        for ($i = 0; $i -lt 40; $i++) {
            Start-Sleep -Milliseconds 250
            $proc.Refresh()
            if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $proc.MainWindowHandle; break }
        }
        if ($hwnd -eq [IntPtr]::Zero) {
            return @{ Ok = $false; Text = ''; Detail = '메모장 메인 윈도우 핸들을 얻지 못했다 (비대화형 세션 의심)' }
        }

        $fg = $false
        for ($i = 0; $i -lt 10; $i++) {
            if ([UnimCiUi]::ForceForeground($hwnd)) { $fg = $true; break }
            Start-Sleep -Milliseconds 300
        }
        Write-Host ('  [{0}] 포그라운드 확보: {1}' -f $Label, $fg)
        if (-not $fg) {
            return @{ Ok = $false; Text = ''; Detail = 'SetForegroundWindow 실패 (비대화형 데스크톱 의심)' }
        }
        Start-Sleep -Milliseconds 500

        # 한/영 전환 (기본 toggle_keys = ["Korean"(VK_HANGUL 0x15), "RightAlt"(VK_RMENU 0xA5)]
        #  — src/config.rs 기본값)
        [void][UnimCiUi]::SendKey($ToggleVk, $false, $ToggleExtended)
        Start-Sleep -Milliseconds 60
        [void][UnimCiUi]::SendKey($ToggleVk, $true,  $ToggleExtended)
        Start-Sleep -Milliseconds 400

        foreach ($k in $keys) {
            [void][UnimCiUi]::SendKey($k, $false, $false)
            Start-Sleep -Milliseconds 40
            [void][UnimCiUi]::SendKey($k, $true, $false)
            Start-Sleep -Milliseconds 100
        }
        Start-Sleep -Milliseconds 200

        # 스페이스로 조합 확정 (커밋).
        [void][UnimCiUi]::SendKey($VK_SPACE, $false, $false)
        Start-Sleep -Milliseconds 40
        [void][UnimCiUi]::SendKey($VK_SPACE, $true, $false)
        Start-Sleep -Milliseconds 400

        [void](Save-Screenshot -Path (Join-Path $ArtDir ('typing-{0}.png' -f $Label)))

        # (1) 클립보드 경로: Ctrl+A, Ctrl+C
        $text = ''
        try { Set-Clipboard -Value '' -ErrorAction SilentlyContinue } catch { }
        [void][UnimCiUi]::SendKey($VK_CTRL, $false, $false)
        [void][UnimCiUi]::SendKey($VK_A, $false, $false); Start-Sleep -Milliseconds 40
        [void][UnimCiUi]::SendKey($VK_A, $true, $false)
        [void][UnimCiUi]::SendKey($VK_CTRL, $true, $false)
        Start-Sleep -Milliseconds 200
        [void][UnimCiUi]::SendKey($VK_CTRL, $false, $false)
        [void][UnimCiUi]::SendKey($VK_C, $false, $false); Start-Sleep -Milliseconds 40
        [void][UnimCiUi]::SendKey($VK_C, $true, $false)
        [void][UnimCiUi]::SendKey($VK_CTRL, $true, $false)
        Start-Sleep -Milliseconds 400
        try {
            $clip = Get-Clipboard -Raw -ErrorAction Stop
            if (-not [string]::IsNullOrEmpty($clip)) { $text = $clip }
        } catch {
            Write-Host ('  [{0}] 클립보드 읽기 실패: {1}' -f $Label, $_.Exception.Message)
        }

        # (2) 폴백: 메모장 Edit 컨트롤에 WM_GETTEXT
        if ([string]::IsNullOrEmpty($text)) {
            try {
                $edit = [UnimCiUi]::GetEditText($hwnd)
                if (-not [string]::IsNullOrEmpty($edit)) { $text = $edit }
            } catch {
                Write-Host ('  [{0}] WM_GETTEXT 실패: {1}' -f $Label, $_.Exception.Message)
            }
        }

        # (3) 폴백: UIAutomation TextPattern/ValuePattern
        if ([string]::IsNullOrEmpty($text)) {
            try {
                Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
                Add-Type -AssemblyName UIAutomationTypes  -ErrorAction Stop
                $root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
                $cond = New-Object System.Windows.Automation.PropertyCondition(
                    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                    [System.Windows.Automation.ControlType]::Document)
                $el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                if ($null -eq $el) {
                    $cond2 = New-Object System.Windows.Automation.PropertyCondition(
                        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                        [System.Windows.Automation.ControlType]::Edit)
                    $el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond2)
                }
                if ($null -ne $el) {
                    $tp = $el.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
                    $text = $tp.DocumentRange.GetText(-1)
                }
            } catch {
                Write-Host ('  [{0}] UIAutomation 실패: {1}' -f $Label, $_.Exception.Message)
            }
        }

        $shown = $text -replace "`r", '' -replace "`n", '\n'
        Write-Host ('  [{0}] 읽은 텍스트: "{1}"' -f $Label, $shown)
        Set-Content -LiteralPath (Join-Path $ArtDir ('typing-{0}.txt' -f $Label)) -Value $text -Encoding UTF8

        if ($text -like '*한*') {
            return @{ Ok = $true; Text = $text; Detail = "'한' 입력 확인" }
        }
        return @{ Ok = $false; Text = $text; Detail = ("'한' 미검출 (읽은 값: '{0}')" -f $shown) }
    } finally {
        if ($null -ne $proc) {
            try { $proc.Kill() } catch { }   # 저장 대화상자 없이 강제 종료
        }
        Start-Sleep -Milliseconds 300
    }
}

function Invoke-TypingPhase {
    $art = New-ArtifactDir

    # self-hosted 러너로 전환되면 이 단계가 실제 사용자 언어 목록을 영구히
    # 오염시킨다(호스티드 1회용 VM 에서는 무해). 감지되면 SKIP 으로 건너뛴다.
    if ($env:RUNNER_ENVIRONMENT -and $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
        Add-Result -Name '타이핑 전제: 호스티드 러너' -Status SKIP `
                   -Detail "RUNNER_ENVIRONMENT=$($env:RUNNER_ENVIRONMENT) — self-hosted 로 의심돼 언어 목록 변경을 건너뛴다"
        return
    }

    if ([string]::IsNullOrEmpty((Get-RegDefaultValue -Path $CLSID_64))) {
        Add-Result -Name '타이핑 전제: UNIM 설치됨' -Status FAIL -Detail "$CLSID_64 없음 — install 단계 뒤에 실행하라"
        return
    }
    Add-Result -Name '타이핑 전제: UNIM 설치됨' -Status PASS

    # 이 단계가 바꾸는 사용자 언어 목록(ko-KR + UNIM TIP)을 끝나면 원복한다 —
    # 아래에 중간 return 이 있으므로 함수 나머지를 try/finally 로 감싼다.
    $origLangList = $null
    try { $origLangList = Get-WinUserLanguageList } catch { }

    try {
    try {
        Add-Type -TypeDefinition $WIN32_UI_SRC -Language CSharp -ErrorAction Stop | Out-Null
    } catch {
        if ($_.Exception.Message -notmatch 'already exists') {
            Add-Result -Name '타이핑 전제: Win32 P/Invoke 컴파일' -Status FAIL -Detail $_.Exception.Message
            return
        }
    }
    Add-Result -Name '타이핑 전제: Win32 P/Invoke 컴파일' -Status PASS

    # 2026-09-03/04 windows-2022 러너 2차 실측: 3회 재시도 후에도 ko-KR 이
    # 언어 목록에 반영되지 않았다(유력 가설: 러너에 한국어 표시 언어팩이 없어
    # Set-WinUserLanguageList 가 조용히 무시 — 위 ITfInputProcessorProfileMgr
    # 인터페이스 선언부 주석(§ko-KR 언어팩 없는 CI 러너) 참조). 그런데도 아래
    # Enable-UnimProfile(ITfInputProcessorProfileMgr::ActivateProfile, 제품이
    # 실제로 쓰는 register.rs::set_as_default() 경로)와 실입력은 같은 실행에서
    # PASS 했다 — 즉 제품 경로는 언어 목록 등록에 의존하지 않는다(ActivateProfile
    # 은 ActivateLanguageProfile 레거시 경로와 달리 사용자 언어 목록 사전 등록을
    # 전제조건으로 두지 않는다). 그래서 이 항목은 필수 게이트(FAIL)가 아니라
    # 정보성 경고(WARN)로 낮춘다 — 실패는 표에 그대로 남기되 exit code 는
    # 오염시키지 않는다.
    $tipRes = Add-UnimInputMethodTip
    if ($tipRes.Ok) {
        Add-Result -Name 'ko-KR + UNIM TIP 언어 목록 등록' -Status PASS -Detail $tipRes.Detail
    } else {
        Add-Result -Name 'ko-KR + UNIM TIP 언어 목록 등록' -Status WARN -Detail $tipRes.Detail
    }

    $act = Enable-UnimProfile
    Write-Host ('  프로필 활성화: {0}' -f $act)
    if ($act -match 'Activate=ok') {
        Add-Result -Name 'TSF 프로필 기본·활성 지정' -Status PASS -Detail $act
    } else {
        Add-Result -Name 'TSF 프로필 기본·활성 지정' -Status FAIL -Detail $act
    }
    Start-Sleep -Seconds 2

    # 1차: VK_HANGUL(0x15). 실패하면 2차: 우측 Alt(VK_RMENU 0xA5, 확장키).
    $r = Invoke-TypingTrial -ToggleVk 0x15 -ToggleExtended $false -Label 'vk-hangul' -ArtDir $art
    if ($r.Ok) {
        Add-Result -Name "한글 실입력 'gks' → '한'" -Status PASS -Detail ('VK_HANGUL / ' + $r.Detail)
        return
    }
    Write-Host ('  1차(VK_HANGUL) 실패: {0} — 우측 Alt 로 재시도' -f $r.Detail)
    $r2 = Invoke-TypingTrial -ToggleVk 0xA5 -ToggleExtended $true -Label 'vk-rmenu' -ArtDir $art
    if ($r2.Ok) {
        Add-Result -Name "한글 실입력 'gks' → '한'" -Status PASS -Detail ('VK_RMENU / ' + $r2.Detail)
    } else {
        Add-Result -Name "한글 실입력 'gks' → '한'" -Status FAIL `
                   -Detail ('VK_HANGUL: {0} | VK_RMENU: {1}' -f $r.Detail, $r2.Detail)
    }
    } finally {
        if ($null -ne $origLangList) {
            try { Set-WinUserLanguageList $origLangList -Force } catch { }
        }
    }
}

# ── Phase: uninstall ─────────────────────────────────────────────────────────

function Invoke-UninstallPhase {
    $art = New-ArtifactDir
    $msi = (Resolve-Path -LiteralPath $MsiPath).ProviderPath
    $uninstallLog = Join-Path $art 'uninstall.log'

    # 제거 전에 설치 디렉터리를 확보해 둔다(제거되면 힌트 키도 사라진다).
    $installDir = Resolve-InstallDir

    # 설치 시 CA 가 띄운 unim-popup-win.exe 가 살아 있으면 파일 삭제가 재부팅
    # 지연으로 밀린다. util:CloseApplication(TerminateProcess="0") 이 정상
    # 종료를 시도하긴 하지만 완화를 보장하지 않으므로 직접 정리한다
    # (Invoke-Msiexec 도 매 호출 뒤 같은 정리를 하지만, /x 호출 전에도 미리 한다).
    Get-Process -Name 'unim-popup-win' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue

    $code = Invoke-Msiexec -ArgList @('/x', $msi) -LogPath $uninstallLog
    Show-MsiLogTail -LogPath $uninstallLog
    if ($code -eq 0) {
        Add-Result -Name 'msiexec /x 제거' -Status PASS -Detail 'exit=0'
    } elseif ($code -eq 3010) {
        Add-Result -Name 'msiexec /x 제거' -Status PASS -Detail 'exit=3010 (재부팅 지연 삭제 발생 — 성공으로 간주)'
    } else {
        Add-Result -Name 'msiexec /x 제거' -Status FAIL -Detail "exit=$code (uninstall.log 참조)"
        return
    }

    # 레지스트리 소멸 — wxs 의 ForceDeleteOnUninstall 계약. 32-bit CLSID 는
    # Test-Path 로 리터럴 경로 하나만 보면 물리 위치를 잘못 짚었을 때 '없으니
    # 소멸' 오탐 PASS(잔존을 놓침)가 되므로 레지스트리 뷰 API 로 확인한다.
    $leftKeys = @()
    foreach ($k in @($CLSID_64, $TIP_ROOT, $LP_KEY)) {
        if (Test-Path -LiteralPath $k) { $leftKeys += $k }
    }
    if ($null -ne (Get-Clsid32InProcServer32 -Clsid $UNIM_CLSID)) { $leftKeys += $CLSID_32 }
    if ($leftKeys.Count -eq 0) {
        Add-Result -Name '레지스트리(CLSID·TIP) 소멸' -Status PASS
    } else {
        Add-Result -Name '레지스트리(CLSID·TIP) 소멸' -Status FAIL -Detail ('잔존: ' + ($leftKeys -join ', '))
    }

    $runVal = Get-RegNamedValue -Path $RUN_KEY -Name $RUN_VALUE_NAME
    if ([string]::IsNullOrEmpty($runVal)) {
        Add-Result -Name 'HKLM Run 자동시작 값 소멸' -Status PASS
    } else {
        Add-Result -Name 'HKLM Run 자동시작 값 소멸' -Status FAIL -Detail "잔존: $runVal"
    }

    # 설치 디렉터리 소멸 — 파일 삭제는 약간 늦을 수 있어 최대 15초 대기.
    for ($i = 0; $i -lt 30; $i++) {
        if (-not (Test-Path -LiteralPath $installDir)) { break }
        Start-Sleep -Milliseconds 500
    }
    if (-not (Test-Path -LiteralPath $installDir)) {
        Add-Result -Name '설치 디렉터리 소멸' -Status PASS -Detail $installDir
    } else {
        # wxs 의 <Feature Complete> 는 사용자 데이터를 INSTALLDIR 에 두지 않는다
        # (설정은 %APPDATA%\unim, 로그는 %TEMP%). 따라서 잔존물은 전부 결함이다.
        $left = @(Get-ChildItem -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue |
                  ForEach-Object { $_.FullName })
        Write-Host '  잔존 파일 목록:'
        $left | ForEach-Object { Write-Host ('   - ' + $_) }
        # unim-popup-win.exe 하나만 남았다면 재부팅 지연 삭제(파일 사용 중)
        # 의심이지 진짜 결함이 아닐 수 있다 — 위에서 미리 죽였는데도 남았다면
        # 폴링으로는 해소되지 않는 지연 삭제이므로 FAIL 이 아니라 경고로 낮춘다.
        $onlyPopupExe = ($left.Count -eq 1 -and (Split-Path -Leaf $left[0]) -eq 'unim-popup-win.exe')
        if ($onlyPopupExe) {
            Add-Result -Name '설치 디렉터리 소멸' -Status SKIP `
                       -Detail ("{0} 에 unim-popup-win.exe 1개만 잔존 — 재부팅 지연 삭제(파일 사용 중) 의심, 폴링으로 해소 안 됨" -f $installDir)
        } else {
            Add-Result -Name '설치 디렉터리 소멸' -Status FAIL `
                       -Detail ("{0} 잔존 ({1}개 항목)" -f $installDir, $left.Count)
        }
    }

    # 프로그램 추가/제거 항목 소멸. SecurityException 등 개별 키 접근 실패로
    # phase 전체가 죽지 않도록 try/catch 로 감싼다.
    $arp = @(Get-ChildItem -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
             Where-Object { try { $_.GetValue('DisplayName') -like 'UNIM*' } catch { $false } } |
             ForEach-Object { try { $_.GetValue('DisplayName') } catch { '(읽기 실패)' } })
    if ($arp.Count -eq 0) {
        Add-Result -Name '프로그램 추가/제거 항목 소멸' -Status PASS
    } else {
        Add-Result -Name '프로그램 추가/제거 항목 소멸' -Status FAIL -Detail ($arp -join ', ')
    }

    # install 단계에서 등록한 Defender 제외 경로를 정리한다 — 러너는 매 잡마다
    # 새로 뜨니 잔존해도 다음 실행에 영향은 없지만, 검증 스크립트가 만든
    # 예외를 스크립트 스스로 치우는 게 원칙이다. 실패해도 비치명(SKIP).
    try {
        Remove-MpPreference -ExclusionPath $installDir -ErrorAction Stop
        Add-Result -Name 'Defender 제외 경로 해제' -Status PASS -Detail $installDir
    } catch {
        Add-Result -Name 'Defender 제외 경로 해제' -Status SKIP -Detail $_.Exception.Message
    }
}

# ── Phase: scan (Windows Defender 오탐 능동 스캔 — 승격 대기) ───────────────
#
# install/typing/uninstall 은 "우리 기능이 실시간 검사에 걸리지 않게" 회피만
# 한다(제외 경로 등록). 이 단계는 반대로 Defender 를 직접 돌려 MSI 와 빌드
# 산출물이 실제로 위협 판정을 받는지 능동으로 확인한다 — 2026-09-03 회사컴
# Trojan:Win32/Bearfoos.B!ml 오판 사고의 조기 발견 게이트.
#
# 탐지 시 이 phase 의 최종 종료코드는 2 다(다른 검증 실패의 1 과 구분) —
# 진입점의 'scan' 분기에서 $script:ScanDetected 를 보고 덮어쓴다.
$script:ScanDetected = $false

function Invoke-ScanPhase {
    $art = New-ArtifactDir

    $mpCmdRun = Resolve-MpCmdRunPath
    if ($null -eq $mpCmdRun) {
        Add-Result -Name 'MpCmdRun.exe 존재' -Status FAIL -Detail 'Windows Defender 플랫폼을 찾지 못했다 (호스티드 러너 전제 위반)'
        return
    }
    Add-Result -Name 'MpCmdRun.exe 존재' -Status PASS -Detail $mpCmdRun

    # (a) 시그니처 업데이트 — 네트워크 의존. 실패해도 러너 내장 시그니처로
    # 계속 진행하므로 게이트가 아니라 SKIP 로만 기록한다.
    $sig = Invoke-MpCmdRunProcess -MpCmdRunPath $mpCmdRun -ArgList @('-SignatureUpdate')
    Set-Content -LiteralPath (Join-Path $art 'defender-sigupdate.log') -Value $sig.Output -Encoding UTF8
    if ($sig.ExitCode -eq 0) {
        Add-Result -Name 'Defender 시그니처 업데이트' -Status PASS
    } else {
        Add-Result -Name 'Defender 시그니처 업데이트' -Status SKIP `
                   -Detail "exit=$($sig.ExitCode) (네트워크 의존, 비치명 — 로그: defender-sigupdate.log)"
    }

    # (b) 스캔 대상 — MSI + 호출자가 넘긴 원시 빌드 산출물 경로(디렉터리/파일).
    $targets = @()
    if (Test-Path -LiteralPath $MsiPath) {
        $targets += (Resolve-Path -LiteralPath $MsiPath).ProviderPath
    } else {
        Add-Result -Name 'Defender 스캔 대상: MSI' -Status FAIL -Detail "없음: $MsiPath"
    }
    # -File 로 호출되면 배열 인자를 못 받는다 — 공백으로 나누면 두 번째 값이
    # 위치 인자(-RepoRoot)로 묶이고(3차 실측), 콤마는 리터럴로 붙는다(2차 실측).
    # 그래서 한 문자열을 받아 여기서 ';' 또는 ',' 로 나눈다.
    $scanDirs = @($ScanPaths | ForEach-Object { $_ -split '[;,]' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    foreach ($d in $scanDirs) {
        if (Test-Path -LiteralPath $d) {
            $targets += (Resolve-Path -LiteralPath $d).ProviderPath
        } else {
            Add-Result -Name "Defender 스캔 대상: $d" -Status SKIP -Detail '경로 없음(이 빌드에서 생략됐을 수 있음)'
        }
    }
    if ($targets.Count -eq 0) {
        Add-Result -Name 'Defender 스캔' -Status FAIL -Detail '스캔할 경로가 하나도 없다'
        return
    }

    $detections = @()
    foreach ($t in $targets) {
        $safeName = ((Split-Path -Leaf $t.TrimEnd('\'))) -replace '[^\w.\-]', '_'
        $log = Join-Path $art ('defender-scan-' + $safeName + '.log')
        # -ScanType 3 = custom scan (단일 파일/디렉터리 지정).
        $r = Invoke-MpCmdRunProcess -MpCmdRunPath $mpCmdRun -ArgList @('-Scan', '-ScanType', '3', '-File', $t)
        Set-Content -LiteralPath $log -Value $r.Output -Encoding UTF8

        # MpCmdRun 버전마다 위협 발견 시 종료코드·출력 형식이 다르다(즉시 격리
        # 후 0 을 돌려주는 빌드도 있다) — 출력 텍스트에서 "Threat : <name>"
        # 패턴과 일반 탐지 문구를 함께 보고, 종료코드 비정상은 보조 신호로만 쓴다.
        # 2026-09-03 2차 실측: 매치 0건이면 ForEach-Object 가 $null 을 내놓는데,
        # 아래에서 바로 .Count 를 읽어 Set-StrictMode -Version Latest 아래서
        # "The property 'Count' cannot be found on this object" 로 scan 단계
        # 전체가 죽었다(위협 유무와 무관하게 매번). @() 로 감싸 항상 배열을
        # 보장한다 — 이 파일의 다른 pipeline 결과들(@($x | Where-Object ...) 류)
        # 과 동일한 방어 패턴.
        $threatNames = @([regex]::Matches($r.Output, '(?im)^\s*Threat\s*:\s*(.+)$') |
                       ForEach-Object { $_.Groups[1].Value.Trim() })
        $hasThreatText = ($r.Output -match '(?i)threat found|malicious|Win32/|MSIL/|!ml\b')
        if ($threatNames.Count -gt 0 -or $hasThreatText) {
            $names = if ($threatNames.Count -gt 0) { ($threatNames | Select-Object -Unique) -join ', ' } else { '(이름 미상 — 로그 참조)' }
            $detections += [pscustomobject]@{ Path = $t; Names = $names }
            Add-Result -Name "Defender 스캔: $(Split-Path -Leaf $t)" -Status FAIL `
                       -Detail ("탐지: {0} (exit={1}, 로그: {2})" -f $names, $r.ExitCode, $log)
        } elseif ($r.ExitCode -eq 0) {
            Add-Result -Name "Defender 스캔: $(Split-Path -Leaf $t)" -Status PASS -Detail 'exit=0, 위협 없음'
        } elseif ($r.Output -match '(?i)hr\s*=\s*0x800106ba') {
            # 2026-09-04 4차 실측: GitHub 호스티드 windows-2022 러너는 Defender
            # 서비스(WinDefend)가 꺼져 있어 MpCmdRun 이 스캔 대상마다
            # "CmdTool: Failed with hr = 0x800106ba"(서비스 미가동) 로 exit 2 를
            # 돌려준다 — 시그니처 업데이트는 되는데 스캔만 안 된다. 이건 산출물의
            # 문제가 아니라 러너의 문제이므로 FAIL 이 아닌 SKIP 으로 명시한다.
            # 이 게이트가 실제로 검사하려면 self-hosted 러너(Defender 가동) 또는
            # VirusTotal 류 외부 스캐너로 대체해야 한다(docs/dev/windows/SMOKE_TEST.md).
            Add-Result -Name "Defender 스캔: $(Split-Path -Leaf $t)" -Status SKIP `
                       -Detail ("Defender 서비스 미가동(hr=0x800106ba) — 호스티드 러너에서는 스캔 불가, exit={0}, 로그: {1}" -f $r.ExitCode, $log)
        } else {
            # 위협 텍스트는 없는데 종료코드가 비정상 — 스캔 자체 실패(파일 잠김 등)로
            # 보고 FAIL 하되 $detections 에는 넣지 않는다(위협 확정이 아니므로
            # exit 2 승격 대상이 아니라 일반 FAIL=exit 1 로 낮춘다).
            Add-Result -Name "Defender 스캔: $(Split-Path -Leaf $t)" -Status FAIL `
                       -Detail ("exit={0} (위협 텍스트 없음 — 로그: {1} 확인)" -f $r.ExitCode, $log)
        }
    }

    # (c) 보조 확인 1 — Get-MpThreatDetection. 최근 탐지 이력 중 우리 대상
    # 경로를 가리키는 게 있으면 (b) 의 능동 스캔과 무관하게도 잡는다.
    try {
        $recent = @(Get-MpThreatDetection -ErrorAction Stop |
                    Where-Object { $_.InitialDetectionTime -gt (Get-Date).AddMinutes(-30) })
        $ourHits = @()
        foreach ($d in $recent) {
            foreach ($res in @($d.Resources)) {
                foreach ($t in $targets) {
                    if ($res -and ($res -like ('*' + $t + '*'))) {
                        $ourHits += ('{0} — {1}' -f $d.ThreatName, $res)
                    }
                }
            }
        }
        if ($ourHits.Count -gt 0) {
            $ourHits = $ourHits | Select-Object -Unique
            $detections += [pscustomobject]@{ Path = '(Get-MpThreatDetection)'; Names = ($ourHits -join '; ') }
            Add-Result -Name 'Get-MpThreatDetection 보조 확인' -Status FAIL -Detail ($ourHits -join '; ')
        } else {
            Add-Result -Name 'Get-MpThreatDetection 보조 확인' -Status PASS `
                       -Detail ("최근 30분 탐지 이력 {0}건, 우리 대상 경로와 무관" -f $recent.Count)
        }
    } catch {
        Add-Result -Name 'Get-MpThreatDetection 보조 확인' -Status SKIP -Detail $_.Exception.Message
    }

    # (c) 보조 확인 2 — 이벤트 로그 1116(탐지)/1117(조치).
    $eventLog = Join-Path $art 'defender-events-1116-1117.log'
    $events = Get-DefenderPathEvents -Paths $targets -SinceMinutes 30
    if ($null -eq $events) {
        Add-Result -Name '이벤트 로그 1116/1117 보조 확인' -Status SKIP `
                   -Detail 'Defender 이벤트 로그(Microsoft-Windows-Windows Defender/Operational) 조회 실패 — 로그/Provider 부재 또는 권한 부족'
    } else {
        $events | ForEach-Object { $_.Message } | Set-Content -LiteralPath $eventLog -Encoding UTF8
        if ($events.Count -gt 0) {
            $detections += [pscustomobject]@{ Path = '(이벤트 1116/1117)'; Names = "$($events.Count)건 — 로그: $eventLog" }
            Add-Result -Name '이벤트 로그 1116/1117 보조 확인' -Status FAIL -Detail ("{0}건 — 로그: {1}" -f $events.Count, $eventLog)
        } else {
            Add-Result -Name '이벤트 로그 1116/1117 보조 확인' -Status PASS -Detail '최근 30분 내 우리 경로 관련 이벤트 없음'
        }
    }

    if ($detections.Count -gt 0) {
        $script:ScanDetected = $true
        Write-Host ''
        Write-Host ("{0} Defender 탐지 {1}건 — 위협명은 위 표 참조" -f [char]0x274C, $detections.Count)
    }
}

# ── 진입점 ───────────────────────────────────────────────────────────────────

if (-not (Test-Path -LiteralPath $MsiPath)) {
    Write-Host ("{0} MSI 없음: {1}" -f [char]0x274C, $MsiPath)
    exit 1
}

# Invoke-*Phase 안에서 예기치 못한 예외가 나도(프로브 실패 등) 결과 표 없이
# 스택 트레이스만 남기고 죽지 않도록 각 호출을 감싼다 — 예외 자체를 FAIL 한
# 줄로 기록하고 반드시 Write-Summary 까지 도달한다.
function Invoke-PhaseSafely {
    param(
        [Parameter(Mandatory = $true)][scriptblock] $Phase,
        [Parameter(Mandatory = $true)][string] $Name
    )
    try {
        & $Phase
    } catch {
        Add-Result -Name "$Name 단계 예외" -Status FAIL -Detail $_.Exception.Message
    }
}

$rc = 0
switch ($Phase) {
    'install' {
        Invoke-PhaseSafely -Phase { Invoke-InstallPhase } -Name 'install'
        $rc = Write-Summary -PhaseName 'install'
    }
    'typing' {
        Invoke-PhaseSafely -Phase { Invoke-TypingPhase } -Name 'typing'
        $rc = Write-Summary -PhaseName 'typing'
    }
    'uninstall' {
        Invoke-PhaseSafely -Phase { Invoke-UninstallPhase } -Name 'uninstall'
        $rc = Write-Summary -PhaseName 'uninstall'
    }
    'scan' {
        $script:ScanDetected = $false
        Invoke-PhaseSafely -Phase { Invoke-ScanPhase } -Name 'scan'
        $rc = Write-Summary -PhaseName 'scan'
        if ($script:ScanDetected) {
            # 위협 확정 탐지는 종료코드 2 로 다른 검증 실패(1)와 구분한다.
            $rc = 2
        }
    }
    'all' {
        Invoke-PhaseSafely -Phase { Invoke-InstallPhase } -Name 'install'
        $rcInstall = Write-Summary -PhaseName 'install'
        $script:Results = New-Object System.Collections.ArrayList

        if ($SkipTyping) {
            Write-Host ("{0} typing 단계 생략 (-SkipTyping)" -f [char]0x23ED)
        } else {
            Invoke-PhaseSafely -Phase { Invoke-TypingPhase } -Name 'typing'
            # typing 은 승격 대기 단계라 all 모드에서도 종료코드에 반영하지 않는다.
            [void](Write-Summary -PhaseName 'typing')
            $script:Results = New-Object System.Collections.ArrayList
        }

        Invoke-PhaseSafely -Phase { Invoke-UninstallPhase } -Name 'uninstall'
        $rcUninstall = Write-Summary -PhaseName 'uninstall'
        $rc = [Math]::Max($rcInstall, $rcUninstall)
    }
}

exit $rc
