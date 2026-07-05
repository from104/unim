#Requires -RunAsAdministrator
<#
.SYNOPSIS
    UNIM v0.3.0을 완전히 제거하고 잔존 레지스트리 양쪽 view를 청소한 뒤 새 MSI를 설치한다.

.DESCRIPTION
    이전 manual fix .bat이 32-bit reg.exe로 박은 키와 MSI가 박은 키가 섞여
    32-bit Wow6432Node에는 8종, 64-bit view에는 0종 카테고리가 박힌 상태를 정리한다.

.NOTES
    실행: PowerShell (관리자)
        cd C:\Users\USER\Desktop\work\unim
        Set-ExecutionPolicy -Scope Process Bypass -Force
        .\scripts\unim-clean-reinstall.ps1
#>

$ErrorActionPreference = 'Stop'
$TIP = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$MsiPath = "$PSScriptRoot\..\dist\unim-0.3.0-x64.msi"

Write-Host '==> [1/5] UNIM MSI 제거...' -ForegroundColor Cyan
$pkg = Get-Package -Name 'UNIM*' -ErrorAction SilentlyContinue
if ($pkg) {
    foreach ($p in $pkg) {
        Write-Host "   - removing $($p.Name) $($p.Version)" -ForegroundColor Gray
        $code = (Get-WmiObject -Class Win32_Product -Filter "Name='$($p.Name)'" -ErrorAction SilentlyContinue).IdentifyingNumber
        if ($code) {
            Start-Process msiexec.exe -ArgumentList "/x $code /qn /norestart" -Wait -NoNewWindow
        }
    }
} else {
    Write-Host '   (UNIM 미설치)' -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> [2/5] 64-bit / 32-bit 양쪽 view에서 TSF + CLSID 키 삭제...' -ForegroundColor Cyan
$keys = @(
    "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\CTF\TIP\$TIP",
    "HKLM:\SOFTWARE\Classes\CLSID\$TIP",
    "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$TIP"
)
foreach ($k in $keys) {
    if (Test-Path $k) {
        Remove-Item -Path $k -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "   - removed $k" -ForegroundColor Gray
    } else {
        Write-Host "   - (none) $k" -ForegroundColor DarkGray
    }
}

Write-Host ''
Write-Host '==> [3/5] HKCU 사용자 TIP 잔존 제거 (Get-WinUserLanguageList)...' -ForegroundColor Cyan
$list = Get-WinUserLanguageList
$ko = $list | Where-Object LanguageTag -eq 'ko'
if ($ko) {
    $before = $ko.InputMethodTips.Count
    $ko.InputMethodTips.RemoveAll({ param($t) $t -match $TIP.Replace('{','').Replace('}','') }) | Out-Null
    Set-WinUserLanguageList $list -Force
    Write-Host "   - korean InputMethodTips: $before -> $($ko.InputMethodTips.Count)" -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> [4/5] 새 MSI 설치 - 64-bit msiexec 명시...' -ForegroundColor Cyan
if (-not (Test-Path $MsiPath)) {
    Write-Host "   ERROR: $MsiPath 없음. scripts\build-msi.bat으로 먼저 빌드." -ForegroundColor Red
    exit 1
}
$MsiPathAbs = (Resolve-Path $MsiPath).Path
$logPath = "$env:TEMP\unim-install.log"
# Use System32 msiexec (64-bit) explicitly
$msiexec = "$env:SystemRoot\System32\msiexec.exe"
Write-Host "   msiexec: $msiexec" -ForegroundColor Gray
Write-Host "   msi    : $MsiPathAbs" -ForegroundColor Gray
Write-Host "   log    : $logPath" -ForegroundColor Gray
Start-Process $msiexec -ArgumentList "/i `"$MsiPathAbs`" /qn /norestart /l*v `"$logPath`"" -Wait -NoNewWindow

Write-Host ''
Write-Host '==> [5/5] 검증 - 64-bit Category\Category 확인...' -ForegroundColor Cyan
$cat64 = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP\Category\Category"
$cat32 = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\CTF\TIP\$TIP\Category\Category"
$n64 = if (Test-Path $cat64) { (Get-ChildItem $cat64).Count } else { 0 }
$n32 = if (Test-Path $cat32) { (Get-ChildItem $cat32).Count } else { 0 }
Write-Host "   64-bit Category count: $n64" -ForegroundColor $(if($n64 -ge 8){'Green'}else{'Red'})
Write-Host "   32-bit Category count: $n32 (이상적으로 0)" -ForegroundColor $(if($n32 -eq 0){'Green'}else{'Yellow'})
Write-Host ''
if ($n64 -ge 8) {
    Write-Host '결과 A: 64-bit view에 정상 - 가설 A 확정 (이전 manual fix 잔존이 원인)' -ForegroundColor Green
    Write-Host '       설정 -> 언어 -> 한국어 -> 키보드 "추가"에서 UNIM 확인.' -ForegroundColor Green
} elseif ($n32 -ge 8) {
    Write-Host '결과 B: 32-bit view에만 박힘 - 가설 B 확정 (WiX 3.x Component.Win64 무시)' -ForegroundColor Red
    Write-Host '       wxs를 다른 방식(KeyPath 분리 or Win64 명시 패턴)으로 수정해야 함.' -ForegroundColor Red
    Write-Host '       로그 확인: $logPath' -ForegroundColor Yellow
} else {
    Write-Host '결과 C: 양쪽 다 비어있음 - MSI 설치 실패 또는 다른 원인' -ForegroundColor Red
    Write-Host '       로그 확인: $logPath' -ForegroundColor Yellow
}

Write-Host ''
Write-Host '==> ctfmon 재시작 (TSF 캐시)...' -ForegroundColor Cyan
Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process "$env:SystemRoot\System32\ctfmon.exe"
