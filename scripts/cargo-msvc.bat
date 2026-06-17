@echo off
REM Wrapper: sets MSVC env from vcvars64.bat, then forwards args to cargo.
REM Default (x64):
REM   scripts\cargo-msvc.bat check -p unim-tsf --target x86_64-pc-windows-msvc
REM
REM For x86 / i686 builds (e.g. unim-imm32 SysWOW64 copy), pass ARCH=x86:
REM   set ARCH=x86 && scripts\cargo-msvc.bat build -p unim-imm32 --target i686-pc-windows-msvc --release
REM This selects vcvarsamd64_x86.bat (cross-compile from host x64 → target x86).
REM
REM PATH augmentation needed when invoked from non-cmd parent (e.g. Git Bash)
REM that didn't pick up the post-install PATH updates.
set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files (x86)\Microsoft Visual Studio\Installer;%PATH%"

if /i "%ARCH%"=="x86" (
  call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsamd64_x86.bat" >nul
) else (
  call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
)
if errorlevel 1 (
  echo vcvars bat failed >&2
  exit /b 1
)
cargo %*
