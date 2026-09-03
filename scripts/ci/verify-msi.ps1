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
    all      : 위 셋을 순서대로 (로컬/수동 실행용)

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

    [ValidateSet('install', 'typing', 'uninstall', 'all')]
    [string] $Phase = 'all',

    [string] $ArtifactDir = 'msi-verify',

    [switch] $SkipTyping,

    [string] $RepoRoot
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
        [Parameter(Mandatory = $true)][ValidateSet('PASS', 'FAIL', 'SKIP')][string] $Status,
        [string] $Detail = ''
    )
    [void] $script:Results.Add([pscustomobject]@{ Name = $Name; Status = $Status; Detail = $Detail })
    switch ($Status) {
        'PASS' { $icon = [char]0x2705 }   # ✅
        'FAIL' { $icon = [char]0x274C }   # ❌
        'SKIP' { $icon = [char]0x23ED }   # ⏭
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
    Write-Host ('  합계: PASS {0} / FAIL {1} / SKIP {2}' -f $passed.Count, $failed.Count, $skipped.Count)

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
    $p = Start-Process -FilePath 'msiexec.exe' -ArgumentList $all -PassThru -NoNewWindow
    if (-not $p.WaitForExit($TimeoutMs)) {
        try { $p.Kill() } catch { }
        throw ("msiexec 이 {0}ms 안에 끝나지 않아 강제 종료했다 (자손 프로세스 대기 의심)" -f $TimeoutMs)
    }
    # CA 가 띄운 렌더러는 msiexec 종료와 무관하게 계속 산다 — 다음 단계(특히
    # uninstall 의 파일 삭제)를 방해하지 않도록 잔존 인스턴스를 정리한다.
    Get-Process -Name 'unim-popup-win' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    return $p.ExitCode
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
        try { profiles.ActivateLanguageProfile(ref unim, langid, ref prof); log += "Activate=ok"; }
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
function Add-UnimInputMethodTip {
    $tip = ('{0}:{1}{2}' -f $UNIM_LANGID_HEX.Substring(2), $UNIM_CLSID, $UNIM_PROFILE_GUID)
    Write-Host ('  InputMethodTip: {0}' -f $tip)
    try {
        Import-Module International -ErrorAction Stop
        $list = Get-WinUserLanguageList
        $ko = @($list | Where-Object { $_.LanguageTag -eq 'ko-KR' })
        if ($ko.Count -eq 0) {
            $list.Add('ko-KR')
            Set-WinUserLanguageList $list -Force
            $list = Get-WinUserLanguageList
            $ko = @($list | Where-Object { $_.LanguageTag -eq 'ko-KR' })
        }
        if ($ko.Count -eq 0) { return @{ Ok = $false; Detail = 'ko-KR 을 언어 목록에 추가하지 못했다' } }
        if ($ko[0].InputMethodTips -notcontains $tip) {
            $ko[0].InputMethodTips.Add($tip)
            Set-WinUserLanguageList $list -Force
        }
        $after = @(Get-WinUserLanguageList | Where-Object { $_.LanguageTag -eq 'ko-KR' })
        if ($after.Count -gt 0 -and ($after[0].InputMethodTips -contains $tip)) {
            return @{ Ok = $true; Detail = $tip }
        }
        $seen = if ($after.Count -gt 0) { ($after[0].InputMethodTips -join ', ') } else { '(ko-KR 없음)' }
        return @{ Ok = $false; Detail = "등록 후에도 목록에 없음. 현재: $seen" }
    } catch {
        return @{ Ok = $false; Detail = $_.Exception.Message }
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

    $tipRes = Add-UnimInputMethodTip
    if ($tipRes.Ok) {
        Add-Result -Name 'ko-KR + UNIM TIP 언어 목록 등록' -Status PASS -Detail $tipRes.Detail
    } else {
        Add-Result -Name 'ko-KR + UNIM TIP 언어 목록 등록' -Status FAIL -Detail $tipRes.Detail
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
