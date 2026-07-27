@echo off
REM Manually re-register UNIM TSF profile (requires Administrator)
REM Use this if MSI install did not register, or after manual DLL replacement.
REM
REM Scope: COM/TSF registration only (regsvr32). This does NOT reinstall files,
REM Run-key entries, or any other MSI-owned state. Full install/repair/removal
REM is via the MSI itself ("msiexec /i unim-x.y.z-x64.msi" or the Windows
REM "Apps" settings page).

setlocal
set "DLL=%~dp0unim_tsf.dll"
if not exist "%DLL%" (
    echo [UNIM] unim_tsf.dll not found at "%DLL%"
    exit /b 1
)

echo [UNIM] Registering TSF profile (64-bit)...
regsvr32 /s "%DLL%"
if errorlevel 1 (
    echo [UNIM] Registration FAILED. Run as Administrator.
    exit /b 1
)

REM VER-WIN-02: the 64-bit regsvr32 call above only populates the 64-bit
REM (native) CLSID view. 32-bit host processes (e.g. KakaoTalk, Hancom
REM Office) resolve TSF's CLSID through the WOW6432Node view instead, which
REM the MSI normally covers with static registry rows (UnimTsfDll32
REM component) rather than a regsvr32 call. If this script is being used to
REM repair a corrupted/incomplete registration, also (re)register the 32-bit
REM DLL through the SysWOW64 regsvr32 so both CLSID views agree.
set "DLL32=%~dp0unim_tsf32.dll"
if exist "%DLL32%" (
    if exist "%SystemRoot%\SysWOW64\regsvr32.exe" (
        echo [UNIM] Registering TSF profile (32-bit / WOW6432Node)...
        "%SystemRoot%\SysWOW64\regsvr32.exe" /s "%DLL32%"
        if errorlevel 1 (
            echo [UNIM] 32-bit registration FAILED. 32-bit apps (KakaoTalk, Hancom, etc.) may not see UNIM.
        )
    )
) else (
    echo [UNIM] unim_tsf32.dll not found — skipping 32-bit (WOW6432Node) registration.
)

echo [UNIM] Done. Add UNIM under Settings ^> Time ^& Language ^> Language ^> Korean ^> Options.
exit /b 0
