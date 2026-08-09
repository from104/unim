#Requires -RunAsAdministrator
<#
.SYNOPSIS
    UNIM Windows TSF 빌드용 도구체인 일괄 설치 (관리자 권한 PowerShell에서 실행).

.DESCRIPTION
    설치 항목:
    1. WiX Toolset 3.14.1.8722 (MSI 빌드용 candle.exe / light.exe). NetFx3 자동 활성화.
    2. Microsoft Visual Studio 2022 Build Tools (C++ workload + Win11 SDK) - unim_tsf.dll MSVC 링크용.
    (Rust toolchain은 사용자 권한으로 별도 설치됨 - 이 스크립트 대상 아님.)

.NOTES
    실행:
      Set-ExecutionPolicy -Scope Process Bypass -Force
      .\scripts\install-windows-toolchain.ps1

    설치 후 새 PowerShell 열고:
      rustup --version
      candle.exe -?
      where.exe link.exe   # MSVC link 확인 (vcvars64.bat 이후)
#>

$ErrorActionPreference = 'Stop'

function Test-Admin {
    $current = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($current)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-Admin)) {
    Write-Host "ERROR: 관리자 권한이 필요합니다. 'PowerShell (관리자)' 로 다시 여세요." -ForegroundColor Red
    exit 1
}

Write-Host '==> [1/3] .NET Framework 3.5 (NetFx3) 활성화...' -ForegroundColor Cyan
$netfx = Get-WindowsOptionalFeature -Online -FeatureName NetFx3 -ErrorAction SilentlyContinue
if ($netfx -and $netfx.State -eq 'Enabled') {
    Write-Host '   NetFx3 이미 활성화됨, 스킵.' -ForegroundColor Green
} else {
    Enable-WindowsOptionalFeature -Online -FeatureName NetFx3 -All -NoRestart | Out-Null
    Write-Host '   NetFx3 활성화 완료.' -ForegroundColor Green
}

Write-Host ''
Write-Host '==> [2/3] WiX Toolset 3.14.1.8722 설치...' -ForegroundColor Cyan
$wixCheck = & winget list --id WiXToolset.WiXToolset -e 2>&1 | Select-String 'WiXToolset.WiXToolset'
if ($wixCheck) {
    Write-Host '   WiX 이미 설치됨, 스킵.' -ForegroundColor Green
} else {
    & winget install --id WiXToolset.WiXToolset -e `
        --accept-source-agreements --accept-package-agreements --silent
    if ($LASTEXITCODE -ne 0) {
        Write-Host "   WiX 설치 실패 (exit $LASTEXITCODE)." -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host '   WiX 설치 완료.' -ForegroundColor Green
}

Write-Host ''
Write-Host '==> [3/3] Visual Studio 2022 Build Tools 설치 (C++ + Win11 SDK, 6-8GB)...' -ForegroundColor Cyan
$vsCheck = & winget list --id Microsoft.VisualStudio.2022.BuildTools -e 2>&1 | Select-String 'BuildTools'
if ($vsCheck) {
    Write-Host '   VS Build Tools 이미 설치됨 - 워크로드 갱신만 시도.' -ForegroundColor Yellow
}

$vsArgs = '--passive --wait --norestart ' +
          '--add Microsoft.VisualStudio.Workload.VCTools ' +
          '--add Microsoft.VisualStudio.Component.Windows11SDK.22621 ' +
          '--includeRecommended'

& winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
    --accept-source-agreements --accept-package-agreements `
    --override $vsArgs

if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -2147024891) {
    Write-Host "   VS Build Tools 설치 실패 (exit $LASTEXITCODE)." -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host '   VS Build Tools 설치 완료 (시간이 더 걸릴 수 있음 - 백그라운드 진행).' -ForegroundColor Green

Write-Host ''
Write-Host '====================================================' -ForegroundColor Green
Write-Host '모든 도구 설치 트리거 완료. 새 PowerShell 창에서:' -ForegroundColor Green
Write-Host '  1) rustup --version' -ForegroundColor White
Write-Host '  2) candle.exe -? ' -ForegroundColor White
Write-Host '  3) "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" 후 cl.exe -? ' -ForegroundColor White
Write-Host '  4) cargo install cargo-wix' -ForegroundColor White
Write-Host '====================================================' -ForegroundColor Green
