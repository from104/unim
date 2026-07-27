@echo off
REM Manually unregister UNIM TSF profile (requires Administrator)
REM
REM PKG-WIN-07: this only undoes the COM registration (regsvr32 /u). It does
REM NOT remove installed files, Run-key entries, the language profile keys
REM the MSI wrote directly (HKLM ...\CTF\TIP\{CLSID}\LanguageProfile\...), or
REM any other MSI-owned state, so a leftover "ghost" language-bar profile can
REM remain even after running this script. For a full, clean removal use the
REM MSI uninstaller instead: "msiexec /x {ProductCode}" or the Windows "Apps"
REM settings page (this is what the MSI's own ForceDeleteOnUninstall keys
REM cover).

setlocal
set "DLL=%~dp0unim_tsf.dll"
if not exist "%DLL%" (
    echo [UNIM] unim_tsf.dll not found at "%DLL%"
    exit /b 1
)

echo [UNIM] Unregistering TSF profile (64-bit)...
regsvr32 /u /s "%DLL%"
if errorlevel 1 (
    echo [UNIM] Unregistration failed. Run as Administrator.
    exit /b 1
)

REM VER-WIN-02: also unregister the 32-bit (WOW6432Node) CLSID view so it
REM does not disagree with the 64-bit view after a partial/manual cleanup.
set "DLL32=%~dp0unim_tsf32.dll"
if exist "%DLL32%" (
    if exist "%SystemRoot%\SysWOW64\regsvr32.exe" (
        echo [UNIM] Unregistering TSF profile (32-bit / WOW6432Node)...
        "%SystemRoot%\SysWOW64\regsvr32.exe" /u /s "%DLL32%"
        if errorlevel 1 (
            echo [UNIM] 32-bit unregistration failed.
        )
    )
) else (
    echo [UNIM] unim_tsf32.dll not found — skipping 32-bit (WOW6432Node) unregistration.
)

echo [UNIM] Done. Note: this removes COM registration only — no files were removed.
echo [UNIM] For a complete removal, use "msiexec /x" or Windows Settings ^> Apps.
exit /b 0
