# UNIM Assemblies 활성 슬롯 주입 진단 (Win+Space 전환 원인 격리).
# 가설: CTF\Assemblies\0x00000412\{TIP_KEYBOARD} 에 UNIM 이 아닌 MS IME 만 등록되어
#       Win+Space 링에서 UNIM 이 빠진다.
#
# 이 스크립트는 그 슬롯을 UNIM 으로 덮어쓰고(원본 백업), ctfmon 재시작 후
# Win+Space 로 UNIM 전환이 되는지 본다.
#
# 관리자 권한 권장. PowerShell 에서:
#   Set-ExecutionPolicy -Scope Process Bypass -Force
#   .\scripts\unim-probe-assemblies.ps1            # 주입
#   .\scripts\unim-probe-assemblies.ps1 -Restore   # 복원

param([switch]$Restore)

$ErrorActionPreference = 'Stop'
$UNIM = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$PROF = '{B2C3D4E5-F6A7-8901-BCDE-F12345678901}'
$KBD  = '{34745C63-B2F0-4784-8B67-5E12C8701A31}'   # GUID_TFCAT_TIP_KEYBOARD slot
$Asm  = "HKCU:\Software\Microsoft\CTF\Assemblies\0x00000412\$KBD"
$Backup = "$env:TEMP\unim_assemblies_backup.json"

function Restart-Ctfmon {
    Write-Host '==> ctfmon restart...' -ForegroundColor Cyan
    Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    Start-Process "$env:SystemRoot\System32\ctfmon.exe"
}

if ($Restore) {
    if (-not (Test-Path $Backup)) {
        Write-Host "ERROR: no backup at $Backup" -ForegroundColor Red; exit 1
    }
    Write-Host '==> restoring original Assemblies slot...' -ForegroundColor Cyan
    $b = Get-Content $Backup -Raw | ConvertFrom-Json
    if (-not (Test-Path $Asm)) { New-Item -Path $Asm -Force | Out-Null }
    if ($b.Default) { Set-ItemProperty $Asm -Name 'Default' -Value $b.Default }
    if ($b.Profile) { Set-ItemProperty $Asm -Name 'Profile' -Value $b.Profile }
    if ($null -ne $b.KeyboardLayout) {
        Set-ItemProperty $Asm -Name 'KeyboardLayout' -Value ([int]$b.KeyboardLayout) -Type DWord
    }
    Write-Host '   restored.' -ForegroundColor Green
    Restart-Ctfmon
    exit 0
}

Write-Host '==> [1] current Assemblies slot:' -ForegroundColor Cyan
if (Test-Path $Asm) {
    $cur = Get-ItemProperty $Asm
    Write-Host ("   Default={0} Profile={1} KeyboardLayout={2}" -f $cur.Default, $cur.Profile, $cur.KeyboardLayout) -ForegroundColor Gray
    # backup
    @{
        Default        = $cur.Default
        Profile        = $cur.Profile
        KeyboardLayout = $cur.KeyboardLayout
    } | ConvertTo-Json | Set-Content $Backup
    Write-Host "   backed up -> $Backup" -ForegroundColor Gray
} else {
    Write-Host '   (slot does not exist - will create)' -ForegroundColor Yellow
    New-Item -Path $Asm -Force | Out-Null
    '{}' | Set-Content $Backup
}

Write-Host ''
Write-Host '==> [2] injecting UNIM into the active TIP_KEYBOARD slot...' -ForegroundColor Cyan
# MS IME used KeyboardLayout=0x04120412 ; mirror that.
Set-ItemProperty $Asm -Name 'Default' -Value $UNIM
Set-ItemProperty $Asm -Name 'Profile' -Value $PROF
New-ItemProperty $Asm -Name 'KeyboardLayout' -Value 0x04120412 -PropertyType DWord -Force | Out-Null

Write-Host '   injected. now:' -ForegroundColor Green
Get-ItemProperty $Asm | Format-List Default, Profile, KeyboardLayout

Restart-Ctfmon

Write-Host ''
Write-Host '====================================================' -ForegroundColor Green
Write-Host 'TEST:' -ForegroundColor Green
Write-Host '  1) press Win+Space  -> does it switch to UNIM now?' -ForegroundColor White
Write-Host '  2) type in Notepad after switching -> hangul composes?' -ForegroundColor White
Write-Host ''
Write-Host '  - switches to UNIM -> CONFIRMED: Assemblies slot was the cause' -ForegroundColor Yellow
Write-Host '    (permanent fix = wxs writes Assemblies, or re-run profiles.Register safely)' -ForegroundColor Yellow
Write-Host '  - still no switch   -> cause is elsewhere (report back)' -ForegroundColor Yellow
Write-Host ''
Write-Host 'NOTE: this overwrote MS IME in that slot. Restore with:' -ForegroundColor Cyan
Write-Host '  .\scripts\unim-probe-assemblies.ps1 -Restore' -ForegroundColor Cyan
Write-Host '====================================================' -ForegroundColor Green
