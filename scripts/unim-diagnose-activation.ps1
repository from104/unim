#Requires -RunAsAdministrator
# UNIM activation diagnostic (ASCII-only to avoid PS 5.1 CP949 issues).

$ErrorActionPreference = 'Continue'
$TIP  = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$PROF = '{B2C3D4E5-F6A7-8901-BCDE-F12345678901}'

Write-Host '==> [1] Korean InputMethodTips' -ForegroundColor Cyan
$ko = (Get-WinUserLanguageList) | Where-Object LanguageTag -eq 'ko'
if ($ko) {
    Write-Host '   ko InputMethodTips:' -ForegroundColor Gray
    $ko.InputMethodTips | ForEach-Object { Write-Host "     - $_" -ForegroundColor White }
    $unimTip = "0412:$TIP$PROF"
    $hasUnim = $ko.InputMethodTips -contains $unimTip
    Write-Host ("   UNIM TIP " + $unimTip + " present: " + $hasUnim) -ForegroundColor $(if($hasUnim){'Green'}else{'Red'})
}

Write-Host ''
Write-Host '==> [2] Force-add UNIM TIP' -ForegroundColor Cyan
$list = Get-WinUserLanguageList
$ko = $list | Where-Object LanguageTag -eq 'ko'
$unimTip = "0412:$TIP$PROF"
if (-not ($ko.InputMethodTips -contains $unimTip)) {
    $ko.InputMethodTips.Add($unimTip) | Out-Null
    try {
        Set-WinUserLanguageList $list -Force
        Write-Host '   Added.' -ForegroundColor Green
    } catch {
        Write-Host ("   Add FAILED: " + $_) -ForegroundColor Red
    }
} else {
    Write-Host '   Already present.' -ForegroundColor Yellow
}

Write-Host ''
Write-Host '==> [3] Re-check + ctfmon DLL load' -ForegroundColor Cyan
Start-Sleep -Seconds 2
$list2 = Get-WinUserLanguageList
$ko2 = $list2 | Where-Object LanguageTag -eq 'ko'
Write-Host '   ko InputMethodTips after add:' -ForegroundColor Gray
$ko2.InputMethodTips | ForEach-Object { Write-Host "     - $_" -ForegroundColor White }

$ctfmon = Get-Process ctfmon -ErrorAction SilentlyContinue
if ($ctfmon) {
    Write-Host ("   ctfmon PID: " + $ctfmon.Id) -ForegroundColor Gray
    $loaded = $ctfmon.Modules | Where-Object { $_.ModuleName -like '*unim*' }
    if ($loaded) {
        Write-Host ("   unim_tsf.dll LOADED: " + $loaded.FileName) -ForegroundColor Green
    } else {
        Write-Host '   unim_tsf.dll NOT loaded in ctfmon' -ForegroundColor Red
    }
} else {
    Write-Host '   ctfmon not running' -ForegroundColor Red
}

Write-Host ''
Write-Host '==> [4] DLL file + regsvr32 dry run' -ForegroundColor Cyan
$dll = 'C:\Program Files\UNIM\unim_tsf.dll'
if (Test-Path $dll) {
    $sizeMB = [math]::Round((Get-Item $dll).Length / 1MB, 2)
    Write-Host ("   File: " + $dll + " size: " + $sizeMB + " MB") -ForegroundColor Gray
    Write-Host '   regsvr32 silent attempt:' -ForegroundColor Gray
    $r = Start-Process regsvr32.exe -ArgumentList "/s `"$dll`"" -PassThru -Wait
    Write-Host ("   regsvr32 exit code: " + $r.ExitCode + " (0=OK)") -ForegroundColor $(if($r.ExitCode -eq 0){'Green'}else{'Red'})
} else {
    Write-Host '   DLL MISSING' -ForegroundColor Red
}

Write-Host ''
Write-Host '==> [5] Recent Application event log (unim/TSF/CTF)' -ForegroundColor Cyan
$evts = Get-WinEvent -LogName Application -MaxEvents 200 -ErrorAction SilentlyContinue | Where-Object {
    $_.Message -match 'unim|TSF|TextInputProcessor|CTF'
} | Select-Object -First 10
if ($evts) {
    $evts | ForEach-Object {
        Write-Host ("   [" + $_.TimeCreated + "] " + $_.LevelDisplayName + ": " + $_.ProviderName) -ForegroundColor Yellow
        $msg = $_.Message
        $excerpt = $msg.Substring(0, [math]::Min(200, $msg.Length))
        Write-Host ("     " + $excerpt) -ForegroundColor Gray
    }
} else {
    Write-Host '   (no related events)' -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> [6] Final ko TIPs' -ForegroundColor Cyan
$cur = (Get-WinUserLanguageList | Where-Object LanguageTag -eq 'ko').InputMethodTips
Write-Host ("   ko TIPs: " + ($cur -join ', ')) -ForegroundColor Gray

Write-Host ''
Write-Host 'Diagnostic complete. Copy the output above to the main session.' -ForegroundColor Cyan
