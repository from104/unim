@echo off
REM Manually re-register UNIM TSF profile (requires Administrator)
REM Use this if MSI install did not register, or after manual DLL replacement.

setlocal
set "DLL=%~dp0unim_tsf.dll"
if not exist "%DLL%" (
    echo [UNIM] unim_tsf.dll not found at "%DLL%"
    exit /b 1
)

echo [UNIM] Registering TSF profile...
regsvr32 /s "%DLL%"
if errorlevel 1 (
    echo [UNIM] Registration FAILED. Run as Administrator.
    exit /b 1
)
echo [UNIM] Done. Add UNIM under Settings ^> Time ^& Language ^> Language ^> Korean ^> Options.
exit /b 0
