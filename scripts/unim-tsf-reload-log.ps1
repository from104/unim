# UNIM TSF reload + log capture helper (run as Administrator)
#
# What it does:
#   1. Restart ctfmon so the freshly-swapped unim_tsf.dll is loaded
#   2. Clear the old diagnostic log
#   3. Wait for you to reproduce in wezterm (type: H A N -> "han")
#   4. Print the tail of the log
#
# Usage (Admin PowerShell):
#   powershell -ExecutionPolicy Bypass -File scripts\unim-tsf-reload-log.ps1
# or, if already in an Admin PS session at repo root:
#   .\scripts\unim-tsf-reload-log.ps1

$ErrorActionPreference = 'SilentlyContinue'

$logPath  = Join-Path $env:TEMP 'unim-tsf.log'
$dllInst  = 'C:\Program Files\UNIM\unim_tsf.dll'

Write-Host '=== UNIM TSF reload + log ===' -ForegroundColor Cyan

# 0. Show currently installed DLL hash (so you can confirm it is the new build)
if (Test-Path $dllInst) {
    $h = (Get-FileHash $dllInst -Algorithm MD5).Hash
    Write-Host ("installed DLL md5 = {0}" -f $h) -ForegroundColor DarkGray
    Write-Host '(expected newest build: 7EC913C2...)' -ForegroundColor DarkGray
} else {
    Write-Host 'WARNING: installed DLL not found at C:\Program Files\UNIM' -ForegroundColor Yellow
}

# 1. Restart ctfmon
Write-Host '[1/4] Restarting ctfmon...'
Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Start-Process (Join-Path $env:SystemRoot 'System32\ctfmon.exe')
Start-Sleep -Milliseconds 500

# 2. Clear log
Write-Host '[2/4] Clearing old log...'
Remove-Item $logPath -Force -ErrorAction SilentlyContinue

# 3. Prompt user to reproduce
Write-Host ''
Write-Host '[3/4] NOW REPRODUCE:' -ForegroundColor Green
Write-Host '   - Open wezterm, switch to UNIM (Han/Eng)'
Write-Host '   - Type slowly:  H  A  N   (to compose one syllable)'
Write-Host '   - Then come back here.'
Write-Host ''
Read-Host 'Press ENTER after you finished typing in wezterm'

# 4. Dump log tail
Write-Host ''
Write-Host '[4/4] ---- log tail (last 40 lines) ----' -ForegroundColor Cyan
if (Test-Path $logPath) {
    Get-Content $logPath -Tail 40
} else {
    Write-Host 'NO LOG FILE PRODUCED.' -ForegroundColor Yellow
    Write-Host 'That means handle_key_down was never called (ctfmon still holds old DLL,'
    Write-Host 'or wezterm did not route keys to UNIM). Try logoff/login and rerun.'
}
Write-Host '---- end ----' -ForegroundColor Cyan
