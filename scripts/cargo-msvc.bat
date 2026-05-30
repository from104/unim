@echo off
REM Wrapper: sets MSVC env from vcvars64.bat, then forwards args to cargo.
REM Usage:  scripts\cargo-msvc.bat check -p unim-tsf --target x86_64-pc-windows-msvc
REM
REM PATH augmentation needed when invoked from non-cmd parent (e.g. Git Bash)
REM that didn't pick up the post-install PATH updates.
set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files (x86)\Microsoft Visual Studio\Installer;%PATH%"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 (
  echo vcvars64.bat failed >&2
  exit /b 1
)
cargo %*
