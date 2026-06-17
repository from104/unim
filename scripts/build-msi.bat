@echo off
REM Local MSI build for unim-tsf (mirrors .github/workflows/windows-msi.yml).
REM Usage (from repo root):
REM   scripts\build-msi.bat
REM
REM Requires:
REM   - Rust toolchain (cargo)
REM   - VS 2022 Build Tools (vcvars64.bat)
REM   - WiX Toolset 3.x (candle.exe, light.exe)
REM
REM Output:
REM   dist\unim-<VERSION>-x64.msi

setlocal enabledelayedexpansion

set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files (x86)\Microsoft Visual Studio\Installer;C:\Program Files (x86)\WiX Toolset v3.14\bin;%PATH%"

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 ( echo vcvars64.bat failed >&2 & exit /b 1 )

REM 1. Verify GUID drift
bash installer/wix/gen-guids.sh
git diff --exit-code installer/wix/generated/guids.wxi >nul
if errorlevel 1 (
  echo ERROR: guids.wxi drifted. Commit the regenerated file.
  exit /b 1
)

REM 2. Build release DLL
cargo build -p unim-tsf --target x86_64-pc-windows-msvc --release
if errorlevel 1 exit /b 1

REM 2b. Build release settings GUI (Slint). Same WIN_OUT_DIR as the DLL.
cargo build -p unim-tsf-settings --target x86_64-pc-windows-msvc --release
if errorlevel 1 exit /b 1

REM 2c. Build release popup renderer (out-of-process). Same WIN_OUT_DIR.
cargo build -p unim-popup-win --target x86_64-pc-windows-msvc --release
if errorlevel 1 exit /b 1

REM 2d. Build IMM32 IME DLL — x64 (for System32) and x86 (for SysWOW64).
REM     Cargo emits unim_imm32.dll; we copy/rename to unim_imm32.ime in-place
REM     so WiX can pick it up via WIN_OUT_DIR / WIN_OUT_DIR32.
cargo build -p unim-imm32 --target x86_64-pc-windows-msvc --release
if errorlevel 1 exit /b 1
copy /Y "target\x86_64-pc-windows-msvc\release\unim_imm32.dll" ^
        "target\x86_64-pc-windows-msvc\release\unim_imm32.ime" >nul
if errorlevel 1 exit /b 1

REM     32-bit build requires a 32-bit MSVC env (vcvarsamd64_x86 = cross from
REM     the host x64 machine).  We re-initialise the environment in a child cmd
REM     so that it doesn't pollute the outer x64 environment used by the candle
REM     step below.
cmd /c ^
  "call ""C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsamd64_x86.bat"" >nul && ^
   cargo build -p unim-imm32 --target i686-pc-windows-msvc --release"
if errorlevel 1 exit /b 1
copy /Y "target\i686-pc-windows-msvc\release\unim_imm32.dll" ^
        "target\i686-pc-windows-msvc\release\unim_imm32.ime" >nul
if errorlevel 1 exit /b 1

REM 3. Extract workspace version
for /f "tokens=2 delims==" %%V in ('findstr /B /C:"version = " Cargo.toml') do (
  set "VERSION=%%V"
)
set "VERSION=%VERSION: "=%"
set "VERSION=%VERSION:"=%"

REM 4. candle + light
if not exist dist mkdir dist
candle.exe -arch x64 -ext WixUtilExtension ^
    -dWIN_OUT_DIR=target\x86_64-pc-windows-msvc\release ^
    -dWIN_OUT_DIR32=target\i686-pc-windows-msvc\release ^
    -out installer\wix\unim.wixobj installer\wix\unim.wxs
if errorlevel 1 exit /b 1

light.exe -sval -ext WixUtilExtension ^
    -out dist\unim-%VERSION%-x64.msi installer\wix\unim.wixobj
if errorlevel 1 exit /b 1

echo.
echo MSI built: dist\unim-%VERSION%-x64.msi
endlocal
