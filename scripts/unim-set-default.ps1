# Set UNIM as the default Korean IME (point Assemblies Default slot to UNIM).
# Calls ITfInputProcessorProfiles::SetDefaultLanguageProfile directly from
# PowerShell so we can verify "make UNIM the default" works WITHOUT rebuilding.
#
# No admin needed (HKCU). PowerShell:
#   Set-ExecutionPolicy -Scope Process Bypass -Force
#   .\scripts\unim-set-default.ps1

$ErrorActionPreference = 'Continue'
$UNIM = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$PROF = '{B2C3D4E5-F6A7-8901-BCDE-F12345678901}'
$KBD  = '{34745C63-B2F0-4784-8B67-5E12C8701A31}'
$Asm  = "HKCU:\Software\Microsoft\CTF\Assemblies\0x00000412\$KBD"

Write-Host '==> [before] Assemblies Default slot:' -ForegroundColor Cyan
if (Test-Path $Asm) {
    $b = Get-ItemProperty $Asm
    Write-Host ("   Default={0} Profile={1}" -f $b.Default, $b.Profile) -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> calling ITfInputProcessorProfiles::SetDefaultLanguageProfile via COM...' -ForegroundColor Cyan

$cs = @"
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

public static class Tsf {
    [DllImport("ole32.dll")]
    static extern int CoCreateInstance(ref Guid clsid, IntPtr outer, uint ctx, ref Guid iid, out IntPtr obj);
    [DllImport("ole32.dll")] static extern int CoInitialize(IntPtr p);

    public static string SetDefault() {
        CoInitialize(IntPtr.Zero);
        Guid clsidProfiles = new Guid("33C53A50-F456-4884-B049-85FD643ECFED");
        Guid iid = new Guid("1F02B6C5-7842-4EE6-8A0B-9A24183A95CA");
        IntPtr p;
        int hr = CoCreateInstance(ref clsidProfiles, IntPtr.Zero, 1, ref iid, out p);
        if (hr != 0) return "CoCreateInstance failed: 0x" + hr.ToString("X8");
        var profiles = (ITfInputProcessorProfiles)Marshal.GetObjectForIUnknown(p);
        Guid unim = new Guid("A1B2C3D4-E5F6-7890-ABCD-EF1234567890");
        Guid prof = new Guid("B2C3D4E5-F6A7-8901-BCDE-F12345678901");
        try { profiles.Register(ref unim); } catch { }
        try {
            profiles.SetDefaultLanguageProfile(0x0412, ref unim, ref prof);
            return "SetDefaultLanguageProfile OK";
        } catch (Exception ex) {
            return "SetDefaultLanguageProfile FAILED: " + ex.Message;
        }
    }
}
"@

try {
    Add-Type -TypeDefinition $cs -Language CSharp
    $r = [Tsf]::SetDefault()
    Write-Host ("   $r") -ForegroundColor $(if($r -like '*OK*'){'Green'}else{'Red'})
} catch {
    Write-Host ("   ERROR compiling/calling: $_") -ForegroundColor Red
}

Start-Sleep -Milliseconds 300
Write-Host ''
Write-Host '==> [after] Assemblies Default slot:' -ForegroundColor Cyan
if (Test-Path $Asm) {
    $a = Get-ItemProperty $Asm
    $mark = if ($a.Default -match 'A1B2C3D4') { '  <== UNIM (success!)' } else { '  (still not UNIM)' }
    Write-Host ("   Default={0}{1}" -f $a.Default, $mark) -ForegroundColor $(if($a.Default -match 'A1B2C3D4'){'Green'}else{'Yellow'})
    Write-Host ("   Profile={0}" -f $a.Profile) -ForegroundColor Gray
}

Write-Host ''
Write-Host '==> ctfmon restart...' -ForegroundColor Cyan
Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Start-Process "$env:SystemRoot\System32\ctfmon.exe"

Write-Host ''
Write-Host 'TEST: in Notepad, does Korean input go to UNIM directly?' -ForegroundColor Yellow
Write-Host '  - if Default=UNIM and hangul composes via UNIM -> this is the fix' -ForegroundColor Yellow
Write-Host '    (permanent: call set_as_default from ActivateEx or langbar menu)' -ForegroundColor Yellow
