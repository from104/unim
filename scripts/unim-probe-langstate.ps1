# Diagnose how UNIM's Korean/English state is (not) shown in the taskbar indicator.
# ASCII-only output. No admin needed.
#   Set-ExecutionPolicy -Scope Process Bypass -Force
#   .\scripts\unim-probe-langstate.ps1
#
# Run this WHILE UNIM is the active IME (switch to UNIM first, then run).

$ErrorActionPreference = 'Continue'
$UNIM = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'

Write-Host '==> [1] Current active keyboard layout / IME (foreground)' -ForegroundColor Cyan
Add-Type -Namespace W -Name U -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern System.IntPtr GetKeyboardLayout(uint tid);
"@
$hwnd = [W.U]::GetForegroundWindow()
$pid2 = 0
$tid = [W.U]::GetWindowThreadProcessId($hwnd, [ref]$pid2)
$hkl = [W.U]::GetKeyboardLayout($tid)
Write-Host ("   foreground tid=$tid  HKL=0x{0:X}" -f $hkl.ToInt64()) -ForegroundColor Gray

Write-Host ''
Write-Host '==> [2] CTF compartment values (OPENCLOSE / CONVERSION) — HKCU live state' -ForegroundColor Cyan
# These live in volatile per-thread CTF state, not registry. Show what we can from registry caches:
$ime = 'HKCU:\Software\Microsoft\CTF\MSUTB\ManualFix'
Write-Host '   (compartment values are volatile/in-process; verify via DebugView [UNIM-TSF] logs instead)' -ForegroundColor DarkGray

Write-Host ''
Write-Host '==> [3] Assemblies Default (who owns the Korean TIP_KEYBOARD slot now)' -ForegroundColor Cyan
$asm = "HKCU:\Software\Microsoft\CTF\Assemblies\0x00000412\{34745C63-B2F0-4784-8B67-5E12C8701A31}"
if (Test-Path $asm) {
    $a = Get-ItemProperty $asm
    $mark = if ($a.Default -match 'A1B2C3D4') { '  <== UNIM is default' } else { '  (not UNIM)' }
    Write-Host ("   Default={0}{1}" -f $a.Default, $mark) -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> [4] Taskbar input indicator setting (does Windows show the indicator at all?)' -ForegroundColor Cyan
$adv = 'HKCU:\Control Panel\International\User Profile'
$tipband = 'HKCU:\Software\Microsoft\CTF\LangBar'
if (Test-Path $tipband) {
    $lb = Get-ItemProperty $tipband
    Write-Host ("   LangBar ShowStatus={0} Transparency={1} Label={2}" -f $lb.ShowStatus, $lb.Transparency, $lb.Label) -ForegroundColor Gray
} else {
    Write-Host '   (no CTF\LangBar key)' -ForegroundColor DarkGray
}
# Win11 input indicator visibility
$expl = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
Write-Host '   (Win11: Settings > Personalization > Taskbar > System tray > Input indicator)' -ForegroundColor DarkGray

Write-Host ''
Write-Host '==> [5] unim_tsf.dll installed + has langbar/compartment code?' -ForegroundColor Cyan
$dll = 'C:\Program Files\UNIM\unim_tsf.dll'
if (Test-Path $dll) {
    $bytes = [System.IO.File]::ReadAllBytes($dll)
    function HasMarker($name) {
        $pat = [System.Text.Encoding]::ASCII.GetBytes($name)
        for ($i=0; $i -lt $bytes.Length - $pat.Length; $i++) {
            $ok=$true
            for ($j=0; $j -lt $pat.Length; $j++) { if ($bytes[$i+$j] -ne $pat[$j]) { $ok=$false; break } }
            if ($ok) { return $true }
        }
        return $false
    }
    Write-Host ("   installed DLL: {0:N0} bytes" -f $bytes.Length) -ForegroundColor Gray
    Write-Host ("   has create_status_icon: {0}" -f (HasMarker 'create_status_icon')) -ForegroundColor Gray
    Write-Host ("   has sync_keyboard_mode: {0}" -f (HasMarker 'sync_keyboard_mode')) -ForegroundColor Gray
} else {
    Write-Host '   DLL not installed!' -ForegroundColor Red
}

Write-Host ''
Write-Host 'NEXT: switch to UNIM, toggle Han/Eng, watch the taskbar clock-area indicator.' -ForegroundColor Yellow
Write-Host '  Tell main session: (a) is there ANY indicator? (b) does it show 가/A or stays fixed?' -ForegroundColor Yellow
