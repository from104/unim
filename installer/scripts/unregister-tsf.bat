@echo off
REM Manually unregister UNIM TSF profile (requires Administrator)

setlocal
set "DLL=%~dp0unim_tsf.dll"
if not exist "%DLL%" (
    echo [UNIM] unim_tsf.dll not found at "%DLL%"
    exit /b 1
)

echo [UNIM] Unregistering TSF profile...
regsvr32 /u /s "%DLL%"
if errorlevel 1 (
    echo [UNIM] Unregistration failed. Run as Administrator.
    exit /b 1
)
echo [UNIM] Done.
exit /b 0
