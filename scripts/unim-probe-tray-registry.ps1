#Requires -RunAsAdministrator
<#
.SYNOPSIS
    [B] 트레이 아이콘 미표시 원인 격리 진단 (레지스트리 vs DLL 아이콘 리소스).

.DESCRIPTION
    UNIM TIP 의 LanguageProfile 에 MS IME 와 동일한 표시 관련 값을 임시로 주입한다:
      - IconFile  -> 아이콘 리소스가 실제로 있는 시스템 DLL(imkrtip.dll) 로 임시 교체
      - IconIndex -> 0
      - ProfileFlags -> 0x2 (MS IME 와 동일)
      - Display Description -> MS IME 와 동일 형식의 리소스 참조 문자열
    그 후 ctfmon 을 재시작해 트레이/입력표시기에 아이콘이 뜨는지 본다.

    결과 해석:
      - 아이콘이 뜨면  -> 원인은 "unim_tsf.dll 에 아이콘 리소스 없음" (DLL 임베드 필요)
      - 안 뜨면        -> 원인은 레지스트리/플래그가 아니라 다른 것 (langbar AddItem/Win11 정책 등)

    되돌리기: 이 스크립트는 원래 값을 백업하고, -Restore 스위치로 복원한다.

.NOTES
    PowerShell (관리자) 에서:
      Set-ExecutionPolicy -Scope Process Bypass -Force
      .\scripts\unim-probe-tray-registry.ps1            # 주입
      .\scripts\unim-probe-tray-registry.ps1 -Restore   # 복원
#>

param([switch]$Restore)

$ErrorActionPreference = 'Stop'
$TIP  = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$PROF = '{B2C3D4E5-F6A7-8901-BCDE-F12345678901}'
$LP = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP\LanguageProfile\0x00000412\$PROF"
$BackupPath = "$env:TEMP\unim_tray_probe_backup.json"

$MsIconFile = '%SystemRoot%\system32\ime\imekr\imkrtip.dll'

function Restart-Ctfmon {
    Write-Host '==> ctfmon 재시작 (TSF 캐시 무효화)...' -ForegroundColor Cyan
    Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    Start-Process "$env:SystemRoot\System32\ctfmon.exe"
}

if (-not (Test-Path $LP)) {
    Write-Host "ERROR: UNIM LanguageProfile 키 없음 ($LP). 먼저 UNIM 을 설치하세요." -ForegroundColor Red
    exit 1
}

if ($Restore) {
    if (-not (Test-Path $BackupPath)) {
        Write-Host "ERROR: 백업 파일 없음 ($BackupPath). 복원할 것이 없습니다." -ForegroundColor Red
        exit 1
    }
    Write-Host '==> 원래 값으로 복원...' -ForegroundColor Cyan
    $bak = Get-Content $BackupPath -Raw | ConvertFrom-Json
    # 임시 추가했던 값 제거
    foreach ($n in @('ProfileFlags', 'Display Description')) {
        if (-not ($bak.PSObject.Properties.Name -contains $n)) {
            Remove-ItemProperty -Path $LP -Name $n -ErrorAction SilentlyContinue
        }
    }
    # IconFile / IconIndex 원복
    if ($bak.IconFile) {
        Set-ItemProperty -Path $LP -Name 'IconFile' -Value $bak.IconFile
    }
    if ($null -ne $bak.IconIndex) {
        Set-ItemProperty -Path $LP -Name 'IconIndex' -Value ([int]$bak.IconIndex) -Type DWord
    }
    Write-Host '   복원 완료.' -ForegroundColor Green
    Restart-Ctfmon
    exit 0
}

# -- 현재 값 백업 --
Write-Host '==> 현재 LanguageProfile 값 백업...' -ForegroundColor Cyan
$cur = Get-ItemProperty -Path $LP
$backup = @{
    IconFile  = $cur.IconFile
    IconIndex = $cur.IconIndex
}
if ($cur.PSObject.Properties.Name -contains 'ProfileFlags') {
    $backup['ProfileFlags'] = $cur.ProfileFlags
}
if ($cur.PSObject.Properties.Name -contains 'Display Description') {
    $backup['Display Description'] = $cur.'Display Description'
}
$backup | ConvertTo-Json | Set-Content $BackupPath
Write-Host "   백업 -> $BackupPath" -ForegroundColor Gray

# -- MS IME 와 동일한 표시 값 주입 --
Write-Host '==> 진단용 값 주입 (아이콘 있는 시스템 DLL + ProfileFlags=0x2)...' -ForegroundColor Cyan
# IconFile 을 REG_EXPAND_SZ 로 (imkrtip.dll - 아이콘 리소스 보유)
New-ItemProperty -Path $LP -Name 'IconFile' -Value $MsIconFile -PropertyType ExpandString -Force | Out-Null
Set-ItemProperty   -Path $LP -Name 'IconIndex' -Value 0 -Type DWord
New-ItemProperty -Path $LP -Name 'ProfileFlags' -Value 2 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $LP -Name 'Display Description' `
    -Value '@%SystemRoot%\system32\input.dll,-5183' -PropertyType ExpandString -Force | Out-Null

Write-Host '   주입 완료. 현재 값:' -ForegroundColor Green
Get-ItemProperty -Path $LP | Format-List IconFile, IconIndex, ProfileFlags, 'Display Description'

Restart-Ctfmon

Write-Host ''
Write-Host '====================================================' -ForegroundColor Green
Write-Host '확인:' -ForegroundColor Green
Write-Host '  1) UNIM 으로 입력기 전환' -ForegroundColor White
Write-Host '  2) 작업표시줄 시계 옆 입력 표시기에 아이콘이 보이는가?' -ForegroundColor White
Write-Host ''
Write-Host '  - 아이콘이 보이면 -> 원인 = unim_tsf.dll 에 아이콘 리소스 없음 (DLL 임베드 필요)' -ForegroundColor Yellow
Write-Host '  - 안 보이면       -> 원인 = 레지스트리가 아닌 다른 것 (langbar/Win11 정책)' -ForegroundColor Yellow
Write-Host ''
Write-Host '복원:  .\scripts\unim-probe-tray-registry.ps1 -Restore' -ForegroundColor Cyan
Write-Host '====================================================' -ForegroundColor Green
