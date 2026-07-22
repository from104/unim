<#
.SYNOPSIS
    UNIM — Windows 한 줄 설치 스크립트 / one-line installer.

.DESCRIPTION
    GitHub Releases 에 게시된 UNIM 의 x64 MSI 를 내려받아 설치한다.
    Downloads and installs UNIM's published x64 MSI from GitHub Releases.

    흐름 / Flow (D2 하이브리드 자기승격):
      1) 비관리자로 진입 → 버전 해석 → MSI + SHA256SUMS-msi 다운로드 →
         매니페스트 대조(1차 검증) → Unblock-File(MOTW 제거).
      2) UAC 로 승격한 powershell 안에서 실행 직전 재해시(2차 검증) 후에만
         msiexec 로 설치 — 1차↔2차 검증 사이의 TOCTOU 창을 실질적으로 축소한다
         (재해시~msiexec 오픈 사이 잔여 micro-window 는 표준 한계로 수용).

    #Requires -RunAsAdministrator 는 일부러 넣지 않는다 — 진입은 비관리자로
    시작해 다운로드·검증까지 수행하고, MSI 실행만 UAC 로 승격하기 때문이다.
    (#Requires -RunAsAdministrator is intentionally omitted: entry is
     non-admin; only the MSI execution is elevated via UAC.)

.PARAMETER Update
    설치돼 있으면 최신(또는 UNIM_VERSION) 릴리스로 업데이트한다.
    Update an existing installation to the latest (or UNIM_VERSION) release.

.PARAMETER Check
    설치 버전과 최신 버전만 보고한다 (아무것도 바꾸지 않음).
    Report installed vs latest version only (changes nothing).

.PARAMETER Force
    이미 최신이거나 다운그레이드여도 강제로 진행한다.
    Proceed even when already up to date or when downgrading.

.PARAMETER Help
    이 도움말을 출력한다. / Show this help.

.EXAMPLE
    irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
    설치 (기본) / install (default).

.EXAMPLE
    & ([scriptblock]::Create((irm https://raw.githubusercontent.com/from104/unim/main/install.ps1))) -Update
    업데이트 / update. -Check, -Force 도 같은 방식.

.EXAMPLE
    $env:UNIM_VERSION='v0.4.0'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
    특정 버전 고정 / pin a specific version.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\install.ps1 -Check
    스크립트를 먼저 내려받아 읽은 뒤 실행 / inspect first, then run.

.NOTES
    안전 / Safety: MSI 는 SHA256 로 2회(다운로드 직후 + 승격 후 실행 직전)
    검증되며, 불일치 시 아무것도 설치하지 않고 중단한다. UAC 는 1회만 뜬다.
    미서명 MSI 는 SmartScreen 경고가 나타날 수 있다.

    환경변수 / Environment:
      UNIM_VERSION   설치할 태그 (예: v0.4.0 또는 0.4.0). 미설정 시 최신 릴리스.
      UNIM_BASE_URL  릴리스 자산 베이스 URL 오버라이드 (테스트/미러 전용).
#>

# 인코딩: 이 파일은 UTF-8, **BOM 없이** 저장한다. 주 설치 경로가
# `irm <url> | iex` 이고 raw.githubusercontent 는 BOM 을 그대로 서빙하므로,
# BOM(U+FEFF) 이 iex 파싱 선두로 새어드는 위험을 원천 차단하기 위함이다
# (rustup·chocolatey·scoop 등 부트스트래퍼 공통 관례). 대가로 PS 5.1 에서
# `-File` 로 직접 실행하면 한국어 출력이 콘솔 코드페이지로 오독될 수 있으나,
# 이는 2차 경로의 표시상 문제일 뿐 스크립트 로직(전부 ASCII)은 정상 동작한다.
# BOM 을 다시 넣지 말 것.

# ── 전역 설정 ────────────────────────────────────────────────────────────────
$ErrorActionPreference = 'Stop'

# TLS 1.2 를 기존 프로토콜에 OR 로 추가한다 (-bor 로 TLS1.3 등 기존 값 보존).
# GitHub 는 TLS 1.2 이상만 허용하므로 PS 5.1 기본값(SSL3/TLS1.0)으로는 실패한다.
[Net.ServicePointManager]::SecurityProtocol = `
    [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

# ── 파일명 계약 (install.sh 와 동형) ─────────────────────────────────────────
$Repo        = 'from104/unim'
$ApiLatest   = "https://api.github.com/repos/$Repo/releases/latest"
$ReleaseBase = if ($env:UNIM_BASE_URL) { $env:UNIM_BASE_URL } `
               else { "https://github.com/$Repo/releases/download" }
$Manifest    = 'SHA256SUMS-msi'
# 릴리스 자산 파일명 검증 정규식 (경로 탈출 가드) — install.sh 의 DEB/RPM_ASSET_RE 동형.
$AssetRe     = '^unim[A-Za-z0-9._+~-]*\.msi$'
# 설치 식별 (unim.wxs 실측: DisplayName / Manufacturer).
$InstallName = 'UNIM Korean IME'
$InstallPub  = 'atit.org'
# 고정 로그 경로 — GUID 작업디렉토리 밖이라 finally 정리 후에도 생존한다.
$LogPath     = Join-Path $env:TEMP 'unim-install.log'

# ── i18n: UI 문화권이 ko 면 한국어, 아니면 영어 (판별은 이 한 곳에 집약) ──────
function Msg([string]$Ko, [string]$En) {
    if ((Get-UICulture).TwoLetterISOLanguageName -eq 'ko') { $Ko } else { $En }
}

function Info([string]$Ko, [string]$En) {
    Write-Host ('[unim-install] ' + (Msg $Ko $En))
}

# 단계 헤더 (install.sh 에는 없는 Windows 관례 — Cyan 강조).
function Step([int]$N, [int]$Total, [string]$Ko, [string]$En) {
    Write-Host ("==> [$N/$Total] " + (Msg $Ko $En)) -ForegroundColor Cyan
}

# 오류 출력 후 종료 (Write-Host 는 호스트가 Unicode 를 안전히 렌더 → 한글 깨짐 방지).
function Die([string]$Ko, [string]$En) {
    Write-Host ('[unim-install] ' + (Msg $Ko $En)) -ForegroundColor Red
    exit 1
}

# $a < $b (검증된 x.y.z 만 입력 — 호출 전에 정규식으로 게이트한다).
function Test-VersionLt([string]$A, [string]$B) {
    return ([version]$A) -lt ([version]$B)
}

# ── 1. 환경 가드 ────────────────────────────────────────────────────────────
function Test-Environment {
    if ($env:OS -ne 'Windows_NT') {
        Die "이 설치 스크립트는 Windows 전용입니다." `
            "This installer is for Windows only."
    }
    # WOW64 환경에서는 PROCESSOR_ARCHITEW6432 가 실제 아키텍처를 담는다.
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') {
        Die "현재 x64(AMD64) 빌드만 제공합니다. 감지된 아키텍처: $arch" `
            "Only x64 (AMD64) builds are provided at this time. Detected architecture: $arch"
    }
}

# ── 2. 버전 결정 → 태그 반환 (install.sh resolve_version 대칭) ───────────────
function Resolve-Version {
    if ($env:UNIM_VERSION) {
        $v = $env:UNIM_VERSION
        if ($v -notlike 'v*') { $v = "v$v" }
        if ($v -notmatch '^v\d+\.\d+\.\d+$') {
            Die "UNIM_VERSION 형식이 올바르지 않습니다: '$($env:UNIM_VERSION)' (예: v0.4.0)" `
                "Invalid UNIM_VERSION format: '$($env:UNIM_VERSION)' (expected e.g. v0.4.0)"
        }
        return $v
    }

    Info "최신 릴리스 조회 중..." "Looking up the latest release..."
    try {
        $rel = Invoke-RestMethod -Uri $ApiLatest `
            -Headers @{ 'User-Agent' = 'unim-install' }
    } catch {
        Die "릴리스 조회 실패 — 네트워크/레이트리밋 확인 (아직 공개된 릴리스가 없을 수도 있습니다). UNIM_VERSION=vX.Y.Z 로 버전 고정 가능." `
            "Failed to query the latest release — check your network / rate limit (there may be no published release yet). You can pin a version with UNIM_VERSION=vX.Y.Z."
    }

    $tag = [string]$rel.tag_name
    # API 응답 태그도 UNIM_VERSION 과 동일한 형식 검증 (URL 보간 전 가드).
    if ($tag -notmatch '^v\d+\.\d+\.\d+$') {
        Die "API 가 반환한 태그 형식이 예상과 다릅니다: '$tag'" `
            "Unexpected release tag format from the GitHub API: '$tag'"
    }
    return $tag
}

# ── 3. 설치된 UNIM 조회 (Uninstall 키, DisplayName 정확 일치 + Publisher) ─────
#   반환: [pscustomobject] @{ Present = $bool; Version = 'x.y.z' 또는 $null(불명) }
#   Win32_Product(자기수리 트리거)·느슨 매칭·미검증 캐스팅을 모두 회피한다.
function Get-InstalledUnim {
    $roots = @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
        # 32비트 호스트에서 실행돼도 x64 등록을 놓치지 않도록 네이티브 뷰도 조회.
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )
    foreach ($root in $roots) {
        $entries = Get-ItemProperty -Path $root -ErrorAction SilentlyContinue
        foreach ($e in $entries) {
            if ($e.DisplayName -eq $InstallName -and $e.Publisher -eq $InstallPub) {
                $dv = [string]$e.DisplayVersion
                # DisplayVersion 은 미검증 레지스트리 값 → [version] 캐스팅 전 형식 검증.
                if ($dv -match '^\d+\.\d+\.\d+$') {
                    return [pscustomobject]@{ Present = $true; Version = $dv }
                }
                return [pscustomobject]@{ Present = $true; Version = $null }
            }
        }
    }
    return [pscustomobject]@{ Present = $false; Version = $null }
}

# ── 4. 다운로드 (매니페스트 + MSI). 반환: 자산 엔트리 배열 ────────────────────
function Invoke-Download {
    param([string]$Tag, [string]$WorkDir)

    $base = "$ReleaseBase/$Tag"
    $manifestPath = Join-Path $WorkDir $Manifest

    $prev = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'   # 다운로드 진행바 억제 (파이프 환경 노이즈 방지)
    try {
        Info "$Manifest 매니페스트 다운로드 중 ($Tag)..." `
             "Downloading the $Manifest manifest ($Tag)..."
        try {
            Invoke-WebRequest -Uri "$base/$Manifest" -OutFile $manifestPath -UseBasicParsing
        } catch {
            Die "$Manifest 다운로드 실패 — 태그 '$Tag' 에 해당하는 릴리스 자산을 찾을 수 없습니다 (잠시 후 재시도하세요)." `
                "Failed to download $Manifest — no matching release asset for tag '$Tag' (try again in a moment)."
        }

        # 매니페스트 파싱: `<64-hex> [*]<파일명>` → 파일명 재게이트 (경로 탈출 가드).
        $entries = @()
        foreach ($line in (Get-Content -LiteralPath $manifestPath)) {
            if ($line -match '^([0-9a-fA-F]{64})\s+\*?(.+)$') {
                $hash = $Matches[1]
                $name = $Matches[2].Trim()
                if ($name -match '[\\/]') {
                    Die "매니페스트에 경로 문자가 포함된 자산명이 있습니다 (거부): $name" `
                        "Manifest contains an asset name with a path separator (rejected): $name"
                }
                if ($name -notmatch $AssetRe) {
                    Die "매니페스트에 예상치 못한 자산명이 있습니다 (거부): $name" `
                        "Manifest contains an unexpected asset name (rejected): $name"
                }
                $entries += [pscustomobject]@{ Hash = $hash; Name = $name }
            }
        }
        if ($entries.Count -eq 0) {
            Die "매니페스트에서 유효한 MSI 자산을 찾지 못했습니다." `
                "No valid MSI asset found in the manifest."
        }

        Info "패키지 $($entries.Count)개 다운로드 중..." "Downloading $($entries.Count) package(s)..."
        foreach ($e in $entries) {
            Info "  → $($e.Name)" "  → $($e.Name)"
            try {
                Invoke-WebRequest -Uri "$base/$($e.Name)" `
                    -OutFile (Join-Path $WorkDir $e.Name) -UseBasicParsing
            } catch {
                Die "다운로드 실패: $($e.Name)" "Download failed: $($e.Name)"
            }
        }
        return $entries
    } finally {
        $ProgressPreference = $prev
    }
}

# ── 5. 체크섬 검증 (1차 — 필수 게이트) ──────────────────────────────────────
function Test-Checksums {
    param([string]$WorkDir, $Entries)
    Info "SHA256 체크섬 검증 중..." "Verifying SHA256 checksums..."
    foreach ($e in $Entries) {
        $path = Join-Path $WorkDir $e.Name
        # Get-FileHash 는 대문자 반환 → 양쪽 ToLowerInvariant 후 비교 (대소문자 무시 고정).
        $got = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        if ($got.ToLowerInvariant() -ne $e.Hash.ToLowerInvariant()) {
            Die "체크섬 불일치 — 다운로드 손상 또는 변조 의심. 재시도하거나 이슈로 제보하세요. (아무것도 설치되지 않았습니다.)" `
                "Checksum mismatch — download corruption or tampering suspected. Retry or report an issue. (Nothing was installed.)"
        }
    }
    Info "체크섬 검증 통과." "Checksums verified."
}

# ── 6. 승격 설치 (D2 하이브리드). 반환: msiexec 종료코드 ─────────────────────
#   승격된 powershell 안에서 실행 직전 재해시(2차 검증) 후에만 msiexec 실행.
#   스텁은 고정 템플릿 + Base64(-EncodedCommand) 로 전달 → 셸 재파싱 인젝션 원천 차단.
function Invoke-ElevatedInstall {
    param([string]$MsiPath, [string]$ExpectedHash)

    # 보간 전 기대 해시 형식 검증 (인젝션 방어의 최후 게이트).
    if ($ExpectedHash -notmatch '^[0-9A-Fa-f]{64}$') {
        Die "내부 오류: 예상 해시 형식이 올바르지 않습니다." `
            "Internal error: malformed expected hash."
    }

    Unblock-File -LiteralPath $MsiPath   # MOTW 제거 (해시 불변) — SmartScreen 재확인 억제

    # 단일 인용부호 이스케이프 (사용자명에 ' 가 있어도 안전 — 경로는 %TEMP% 유래).
    $msiEsc  = $MsiPath -replace "'", "''"
    $logEsc  = $LogPath -replace "'", "''"
    $hashEsc = $ExpectedHash.ToLowerInvariant()

    # 승격 프로세스가 실행할 고정 스텁. `$… 는 스텁 내부에서 평가되도록 이스케이프,
    # $msiEsc/$hashEsc/$logEsc 만 지금 보간된다.
    $stub = @"
`$ErrorActionPreference = 'Stop'
`$msi = '$msiEsc'
`$want = '$hashEsc'
`$log = '$logEsc'
`$got = (Get-FileHash -LiteralPath `$msi -Algorithm SHA256).Hash
if (`$got.ToLowerInvariant() -ne `$want) { exit 100 }
`$p = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', `$msi, '/qn', '/norestart', '/l*v', `$log) -Wait -PassThru
exit `$p.ExitCode
"@
    $enc = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($stub))

    Step 3 3 "설치 중 (관리자 권한 승격 — UAC 승인 필요)..." `
             "Installing (elevating — UAC approval required)..."
    try {
        # 호스트는 powershell.exe(5.1)로 고정 — 모든 Windows 에 존재, 부모가 pwsh 7 이어도 동일.
        $proc = Start-Process -FilePath 'powershell.exe' -Verb RunAs -Wait -PassThru `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', $enc)
    } catch {
        # PS 5.1: UAC 거부는 InvalidOperationException(Inner=Win32Exception,1223) 로 던져진다.
        # PS 7: Win32Exception(1223) 직접형. 두 경우 모두 매칭한다.
        $inner = $_.Exception.InnerException
        if ( ($inner -is [System.ComponentModel.Win32Exception] -and $inner.NativeErrorCode -eq 1223) -or `
             ($_.Exception -is [System.ComponentModel.Win32Exception] -and $_.Exception.NativeErrorCode -eq 1223) ) {
            Die "설치가 취소되었습니다 (UAC 권한 상승 거부). 아무것도 설치되지 않았습니다." `
                "Installation was cancelled (UAC elevation declined). Nothing was installed."
        }
        throw   # 그 외 예외는 그대로 전파 (fail-closed)
    }
    return $proc.ExitCode
}

# ── 성공/보고 안내 ──────────────────────────────────────────────────────────
function Show-Success {
    param([string]$Tag, [bool]$Reboot)
    Write-Host ''
    Info "UNIM $Tag 설치 완료!" "UNIM $Tag installed successfully!"
    Write-Host ''
    Write-Host (Msg "다음 단계:" "Next steps:")
    Write-Host (Msg "  · 입력 언어 전환기(Win+Space 등)에서 UNIM 을 선택해 사용하세요." `
                    "  · Select UNIM from the input switcher (e.g. Win+Space) to start using it.")
    if ($Reboot) {
        Write-Host (Msg "  · 설치를 완전히 적용하려면 재부팅이 필요합니다." `
                        "  · A reboot is required to fully apply the installation.")
    }
    Write-Host ''
}

function Show-UpdateSuccess {
    param([string]$From, [string]$To, [bool]$Reboot)
    Write-Host ''
    Info "UNIM 업데이트 완료: $From → $To" "UNIM updated: $From → $To"
    Write-Host ''
    Write-Host (Msg "다음 단계:" "Next steps:")
    if ($Reboot) {
        Write-Host (Msg "  · 새 버전을 완전히 적용하려면 재부팅하세요." `
                        "  · Reboot to fully apply the new version.")
    } else {
        Write-Host (Msg "  · 실행 중이던 UNIM 이 있으면 다시 시작하거나 재로그인하세요." `
                        "  · If UNIM was running, restart it or log back in.")
    }
    Write-Host (Msg "  · 설정은 그대로 유지됩니다." "  · Your settings are preserved.")
    Write-Host ''
}

function Show-CheckReport {
    param($Inst, [string]$Latest)
    if (-not $Inst.Present) {
        Info "UNIM 이 설치되어 있지 않습니다. 최신 릴리스: $Latest" `
             "UNIM is not installed. Latest release: $Latest"
    } elseif ($null -eq $Inst.Version) {
        Info "UNIM 이 설치되어 있으나 버전을 확인할 수 없습니다. 최신 릴리스: $Latest" `
             "UNIM is installed but its version could not be determined. Latest release: $Latest"
    } elseif ($Inst.Version -eq $Latest) {
        Info "최신 버전입니다 ($($Inst.Version))." "Up to date ($($Inst.Version))."
    } elseif (Test-VersionLt $Inst.Version $Latest) {
        Info "업데이트 가능: $($Inst.Version) → $Latest  (적용: -Update)" `
             "Update available: $($Inst.Version) → $Latest  (apply with: -Update)"
    } else {
        Info "설치된 버전($($Inst.Version))이 릴리스($Latest)보다 새롭습니다." `
             "The installed version ($($Inst.Version)) is newer than the release ($Latest)."
    }
}

# ── 사용법 ──────────────────────────────────────────────────────────────────
function Show-Usage {
    Write-Host (Msg "UNIM 설치 스크립트 (Windows)" "UNIM installer (Windows)")
    Write-Host ''
    Write-Host (Msg "사용법:" "Usage:")
    Write-Host '  irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex'
    Write-Host '  & ([scriptblock]::Create((irm <url>))) -Update|-Check|-Force'
    Write-Host '  powershell -ExecutionPolicy Bypass -File .\install.ps1 [-Update|-Check|-Force]'
    Write-Host ''
    Write-Host (Msg "동작:" "Actions:")
    Write-Host (Msg "  (없음)   설치 (기본)" "  (none)   install (default)")
    Write-Host (Msg "  -Update  설치돼 있으면 최신 버전으로 업데이트" `
                    "  -Update  update an existing installation to the latest release")
    Write-Host (Msg "  -Check   설치 버전과 최신 버전만 확인 (아무것도 바꾸지 않음)" `
                    "  -Check   report installed vs latest version only (changes nothing)")
    Write-Host ''
    Write-Host (Msg "옵션:" "Options:")
    Write-Host (Msg "  -Force   이미 최신이거나 다운그레이드여도 강제 진행" `
                    "  -Force   proceed even if up to date or downgrading")
    Write-Host (Msg "  -Help    이 도움말" "  -Help    show this help")
    Write-Host ''
    Write-Host (Msg "환경변수: UNIM_VERSION=vX.Y.Z (버전 고정), UNIM_BASE_URL (미러)" `
                    "Environment: UNIM_VERSION=vX.Y.Z (pin a version), UNIM_BASE_URL (mirror)")
}

# ── main: 파이프 절단 가드 (마지막 줄에서만 호출) ───────────────────────────
function Main {
    param([switch]$Update, [switch]$Check, [switch]$Force, [switch]$Help)

    if ($Help) { Show-Usage; return }

    Test-Environment
    $tag = Resolve-Version
    $latest = $tag.Substring(1)          # 태그에서 'v' 제거 → x.y.z
    $inst = Get-InstalledUnim

    # ── check: 조회만 하고 종료 (무변경 보장) ────────────────────────────
    if ($Check) {
        Show-CheckReport $inst $latest
        return
    }

    # ── 동작 결정: update 는 필요할 때만 (멱등·다운그레이드 가드, install.sh 대칭) ──
    $isUpdate = [bool]$Update
    if ($isUpdate) {
        if (-not $inst.Present) {
            Info "UNIM 이 설치되어 있지 않아 새로 설치합니다." `
                 "UNIM is not installed yet — performing a fresh install."
            $isUpdate = $false
        } elseif ($null -eq $inst.Version) {
            # 설치 버전 불명 → 비교 불가. -Force 없이는 진행하지 않는다 (fail-safe).
            if (-not $Force) {
                Die "설치된 UNIM 버전을 확인할 수 없어 비교할 수 없습니다. 강제로 재설치하려면 -Force 를 쓰세요." `
                    "Cannot determine the installed UNIM version to compare. Use -Force to reinstall anyway."
            }
            Info "설치 버전 불명 — -Force 로 재설치합니다." `
                 "Installed version unknown — reinstalling with -Force."
        } elseif ($inst.Version -eq $latest -and -not $Force) {
            Info "이미 최신 버전입니다 ($($inst.Version)). 다시 설치하려면 -Force 를 쓰세요." `
                 "Already up to date ($($inst.Version)). Use -Force to reinstall anyway."
            return
        } elseif ((Test-VersionLt $latest $inst.Version) -and -not $Force) {
            Die "설치된 버전($($inst.Version))이 대상($latest)보다 새롭습니다. 다운그레이드하려면 -Force 를 쓰세요." `
                "The installed version ($($inst.Version)) is newer than the target ($latest). Use -Force to downgrade."
        } else {
            Info "업데이트: $($inst.Version) → $latest" "Updating: $($inst.Version) → $latest"
        }
    }

    # ── 다운로드 → 검증 → 승격 설치 (GUID 작업디렉토리 + finally 정리) ────
    $workDir = Join-Path $env:TEMP ("unim-install-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $workDir -Force | Out-Null
    try {
        Step 1 3 "패키지 다운로드..." "Downloading packages..."
        $entries = @(Invoke-Download $tag $workDir)

        Step 2 3 "체크섬 검증..." "Verifying checksums..."
        Test-Checksums $workDir $entries

        $msi = $entries[0]
        $msiPath = Join-Path $workDir $msi.Name
        $code = Invoke-ElevatedInstall $msiPath $msi.Hash

        switch ($code) {
            0 {
                if ($isUpdate) { Show-UpdateSuccess $inst.Version $latest $false }
                else           { Show-Success $tag $false }
            }
            3010 {   # 성공 + 재부팅 필요 (실패 아님)
                if ($isUpdate) { Show-UpdateSuccess $inst.Version $latest $true }
                else           { Show-Success $tag $true }
            }
            100 {
                Die "무결성 재검증 실패 — 파일 변조가 의심됩니다. 아무것도 설치되지 않았습니다." `
                    "Integrity re-verification failed — tampering suspected. Nothing was installed."
            }
            default {
                Die "설치에 실패했습니다 (msiexec 코드 $code). 로그: $LogPath" `
                    "Installation failed (msiexec code $code). Log: $LogPath"
            }
        }
    } finally {
        Remove-Item -LiteralPath $workDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Main @args
