#Requires -RunAsAdministrator
<#
.SYNOPSIS
    UNIM v0.3.0 MSI 설치 후 카테고리가 Wow6432Node(32-bit)에만 박혀 있는 문제 임시 해결.
    8종 표준 TSF 카테고리를 64-bit view로 복제한다.

.DESCRIPTION
    영구 해결은 installer/wix/unim.wxs의 RegistryKey에 Win64="yes" 명시 후 MSI 재빌드.
    본 스크립트는 그 전까지 사용자 PC를 동작시키기 위한 reg copy.

.NOTES
    실행:  PowerShell (관리자) -> cd C:\Users\USER\Desktop\work\unim -> .\scripts\unim-fix-x64-categories.ps1
#>

$ErrorActionPreference = 'Stop'
$TIP = '{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}'
$CATS = @(
    '{046B8C80-1647-40F7-9B21-B93B81AABC1B}',  # DISPLAYATTRIBUTEPROVIDER
    '{13A016DF-560B-46CD-947A-4C3AF1E0E35D}',  # TIPCAP_IMMERSIVESUPPORT
    '{25504FB4-7BAB-4BC1-9C69-CF81890F0EF5}',  # TIPCAP_SYSTRAYSUPPORT
    '{34745C63-B2F0-4784-8B67-5E12C8701A31}',  # TIP_KEYBOARD (필수)
    '{364215D9-75BC-11D7-A6EF-00065B84435C}',  # TIPCAP_COMLESS
    '{49D2F9CE-1F5E-11D7-A6D3-00065B84435C}',  # TIPCAP_SECUREMODE
    '{49D2F9CF-1F5E-11D7-A6D3-00065B84435C}',  # TIPCAP_UIELEMENTENABLED
    '{CCF05DD7-4A87-11D7-A6E2-00065B84435C}'   # INPUTMODECOMPARTMENT
)

$BasePath = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP\Category"

Write-Host '==> 64-bit view에 카테고리 키 생성/덮어쓰기...' -ForegroundColor Cyan
foreach ($cat in $CATS) {
    foreach ($side in @('Category', 'Item')) {
        $key = "$BasePath\$side\$cat\$TIP"
        if (-not (Test-Path $key)) {
            New-Item -Path $key -Force | Out-Null
        }
        Write-Host "  + $side\$cat" -ForegroundColor Gray
    }
}

Write-Host ''
Write-Host '==> 검증 ====' -ForegroundColor Cyan
$found = (Get-ChildItem "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$TIP\Category\Category" -ErrorAction SilentlyContinue).Count
Write-Host "  64-bit view Category\Category: $found / 8" -ForegroundColor $(if($found -eq 8){'Green'}else{'Red'})

Write-Host ''
Write-Host '==> ctfmon 재시작 (TSF 캐시 무효화)...' -ForegroundColor Cyan
Get-Process ctfmon -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process "$env:SystemRoot\System32\ctfmon.exe"

Write-Host ''
Write-Host '완료. 설정 -> 언어 -> 한국어 -> 키보드 "추가"에서 UNIM 확인.' -ForegroundColor Green
Write-Host '안 보이면 로그아웃 -> 로그인 또는 재부팅 권장.' -ForegroundColor Yellow
