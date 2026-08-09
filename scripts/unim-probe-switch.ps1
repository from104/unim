# UNIM Win+Space 입력기 전환 진단 (ASCII-only 출력).
# 관리자 권한 불필요. PowerShell 에서:
#   Set-ExecutionPolicy -Scope Process Bypass -Force
#   .\scripts\unim-probe-switch.ps1

$ErrorActionPreference = 'Continue'
$TIP  = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$PROF = '{B2C3D4E5-F6A7-8901-BCDE-F12345678901}'

Write-Host '==> [1] WinUserLanguageList (languages + their IME tips)' -ForegroundColor Cyan
$list = Get-WinUserLanguageList
foreach ($l in $list) {
    Write-Host ("  Language: {0}  (Autonym={1})" -f $l.LanguageTag, $l.Autonym) -ForegroundColor White
    Write-Host ("    Handwriting={0} InputMethodTips:" -f $l.Handwriting) -ForegroundColor Gray
    foreach ($t in $l.InputMethodTips) {
        $mark = if ($t -match 'A1B2C3D4') { '  <== UNIM' } else { '' }
        Write-Host ("      - {0}{1}" -f $t, $mark) -ForegroundColor Gray
    }
}
$totalTips = ($list | ForEach-Object { $_.InputMethodTips.Count } | Measure-Object -Sum).Sum
Write-Host ("  TOTAL input method tips across all languages: {0}" -f $totalTips) -ForegroundColor $(if($totalTips -ge 2){'Green'}else{'Red'})
Write-Host '  (Win+Space cycles through ALL of these; need >= 2 to toggle)' -ForegroundColor DarkGray

Write-Host ''
Write-Host '==> [2] Keyboard Layout\Preload (legacy preload list)' -ForegroundColor Cyan
$preload = 'HKCU:\Keyboard Layout\Preload'
if (Test-Path $preload) {
    Get-Item $preload | Select-Object -ExpandProperty Property | ForEach-Object {
        $v = (Get-ItemProperty $preload).$_
        Write-Host ("  $_ = $v") -ForegroundColor Gray
    }
} else { Write-Host '  (none)' -ForegroundColor DarkGray }

Write-Host ''
Write-Host '==> [3] Keyboard Layout\Substitutes' -ForegroundColor Cyan
$subst = 'HKCU:\Keyboard Layout\Substitutes'
if (Test-Path $subst) {
    Get-Item $subst | Select-Object -ExpandProperty Property | ForEach-Object {
        $v = (Get-ItemProperty $subst).$_
        Write-Host ("  $_ = $v") -ForegroundColor Gray
    }
} else { Write-Host '  (none)' -ForegroundColor DarkGray }

Write-Host ''
Write-Host '==> [4] CTF\Assemblies (per-language assembled TIP list - what CTF actually cycles)' -ForegroundColor Cyan
$asm = 'HKCU:\Software\Microsoft\CTF\Assemblies'
if (Test-Path $asm) {
    Get-ChildItem $asm -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host ("  Layout group: {0}" -f $_.PSChildName) -ForegroundColor White
        Get-ChildItem $_.PSPath -ErrorAction SilentlyContinue | ForEach-Object {
            $props = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
            $clsid = $props.CLSID
            $prof  = $props.Profile
            $mark = if (($clsid -match 'A1B2C3D4') -or ($prof -match 'B2C3D4E5')) { '  <== UNIM' } else { '' }
            Write-Host ("    {0}  CLSID={1} Profile={2}{3}" -f $_.PSChildName, $clsid, $prof, $mark) -ForegroundColor Gray
        }
    }
} else { Write-Host '  (none)' -ForegroundColor DarkGray }

Write-Host ''
Write-Host '==> [5] CTF\SortOrder\AssemblyItem (input switch order - the Win+Space ring)' -ForegroundColor Cyan
$sort = 'HKCU:\Software\Microsoft\CTF\SortOrder\AssemblyItem'
if (Test-Path $sort) {
    Get-ChildItem $sort -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        $props = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
        if ($props.CLSID -or $props.KeyboardLayout -or $props.Profile) {
            $mark = if (($props.CLSID -match 'A1B2C3D4') -or ($props.Profile -match 'B2C3D4E5')) { '  <== UNIM' } else { '' }
            Write-Host ("  {0}: CLSID={1} KLID={2} Profile={3}{4}" -f $_.PSChildName, $props.CLSID, $props.KeyboardLayout, $props.Profile, $mark) -ForegroundColor Gray
        }
    }
} else { Write-Host '  (none)' -ForegroundColor DarkGray }

Write-Host ''
Write-Host '==> [6] UNIM TIP LanguageProfile Enable flag (HKLM)' -ForegroundColor Cyan
$lp = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP\LanguageProfile\0x00000412\$PROF"
if (Test-Path $lp) {
    $p = Get-ItemProperty $lp
    Write-Host ("  Enable={0}  Description={1}" -f $p.Enable, $p.Description) -ForegroundColor Gray
} else { Write-Host '  (UNIM LP key missing!)' -ForegroundColor Red }

Write-Host ''
Write-Host '==> [7] HKCU enabled-state for UNIM profile' -ForegroundColor Cyan
$lpu = "HKCU:\SOFTWARE\Microsoft\CTF\TIP\$TIP\LanguageProfile\0x00000412\$PROF"
if (Test-Path $lpu) {
    $p = Get-ItemProperty $lpu
    Write-Host ("  HKCU Enable={0}" -f $p.Enable) -ForegroundColor Gray
} else { Write-Host '  (no HKCU per-user override - inherits HKLM)' -ForegroundColor DarkGray }

Write-Host ''
Write-Host 'Diagnostic complete. Copy ALL output to the main session.' -ForegroundColor Cyan
Write-Host 'Key question: does [1] show >= 2 tips, and does UNIM appear in [4]/[5] rings?' -ForegroundColor Yellow
