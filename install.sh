#!/usr/bin/env bash
# ============================================================================
# UNIM — 한 줄 설치 스크립트 / one-line installer
# ============================================================================
#
# 용도 / Purpose:
#   GitHub Releases 에 게시된 UNIM 패키지를 내려받아 설치한다 — apt 기반은
#   .deb 를 apt 로, dnf 기반(Fedora)은 .rpm 을 dnf 로 (감지 후 자동 분기).
#   Downloads UNIM's published packages from GitHub Releases and installs them:
#   .deb via apt on apt-based distros, .rpm via dnf on Fedora (auto-detected).
#
# 사용법 / Usage:
#   curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
#
#   # 업데이트 / update (설치돼 있으면 최신 릴리스로):
#   curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash -s -- --update
#
#   # 확인만 / check only (아무것도 바꾸지 않음 / changes nothing):
#   curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash -s -- --check
#
#   전체 옵션 / all options: bash install.sh --help
#
#   # 특정 버전 고정 / pin a specific version:
#   UNIM_VERSION=v0.4.0 curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
#
#   # 스크립트를 먼저 읽고 실행 (curl|bash 를 신뢰하지 않는 경우):
#   # inspect first, then run (if you don't trust curl|bash):
#   curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh -o install.sh
#   less install.sh && bash install.sh
#
# 환경변수 / Environment variables:
#   UNIM_VERSION   설치할 태그 (예: v0.4.0 또는 0.4.0). 미설정 시 최신 릴리스.
#                  Tag to install (e.g. v0.4.0 or 0.4.0). Defaults to latest release.
#   UNIM_BASE_URL  릴리스 자산 베이스 URL 오버라이드 (테스트/미러 전용).
#                  Override for the release-asset base URL (testing / mirrors only).
#
# 안전 / Safety:
#   이 스크립트는 mktemp 임시 디렉토리와 패키지 매니저(apt/dnf) 트랜잭션 외에는
#   시스템을 건드리지 않는다. 모든 패키지는 SHA256 체크섬으로 검증되며, 검증
#   실패 시 아무것도 설치하지 않고 중단한다 (부분 설치 없음).
#   This script touches nothing outside a mktemp working directory and the
#   package-manager (apt/dnf) transaction. Every package is SHA256-verified;
#   on mismatch it aborts without installing anything (no partial installs).
#
# 대상 / Target (amd64/x86_64):
#   .deb — Ubuntu 24.04 (noble) 이상 / Debian 13 (trixie) 이상
#   .rpm — Fedora 43 이상
#   그 외 배포판은 소스 빌드를 사용한다 (README 참고).
#   Other distributions: build from source (see README).
# ============================================================================

# bash 감지 가드 — 반드시 POSIX 문법으로, `set -o pipefail`(bash 전용) 앞에 둔다.
# `sh install.sh` / `curl ... | sh` 로 실행하면 dash 는 여기서 깔끔히 멈춘다
# (아래 bash 전용 문법을 파싱하기 전에 종료 → 부분 실행 없음).
if [ -z "${BASH_VERSION:-}" ]; then
	echo "[unim-install] bash 로 실행해야 합니다: bash install.sh" >&2
	echo "[unim-install] This installer must be run with bash: bash install.sh" >&2
	exit 1
fi

set -euo pipefail

# ── i18n: LANG 이 ko* 면 한국어, 아니면 영어 ────────────────────────────────
msg() {
	if [[ ${LANG:-} == ko* ]]; then
		printf '%s\n' "$1"
	else
		printf '%s\n' "$2"
	fi
}

# ── 오류 출력 후 종료 ────────────────────────────────────────────────────────
err() {
	printf '[unim-install] %s\n' "$(msg "$1" "$2")" >&2
	exit 1
}

info() {
	printf '[unim-install] %s\n' "$(msg "$1" "$2")"
}

# ── 설정 (환경변수 오버라이드 가능) ──────────────────────────────────────────
REPO="from104/unim"
API_LATEST="https://api.github.com/repos/${REPO}/releases/latest"
RELEASE_BASE="${UNIM_BASE_URL:-https://github.com/${REPO}/releases/download}"

# 릴리스 자산 파일명 검증 정규식 (경로 탈출 가드) — 패키지 종류별.
DEB_ASSET_RE='^unim[A-Za-z0-9._+~-]*\.deb$'
RPM_ASSET_RE='^unim[A-Za-z0-9._+~-]*\.rpm$'

# guard_environment 가 결정하는 전역:
#   PKG_KIND     deb | rpm            — 내려받을 자산 종류
#   INSTALL_MODE apt | dnf | rpm-ostree — 실제 설치 명령
#   MANIFEST / ASSET_RE                — 종류별 체크섬 매니페스트·파일명 규칙
PKG_KIND=""
INSTALL_MODE=""
MANIFEST=""
ASSET_RE=""

# 동작 모드: install(기본) | update | check. parse_args 가 결정.
ACTION="install"
FORCE=0
# guard_environment 가 설정 (root 면 빈 문자열). 기본값은 set -u 대비.
SUDO=""

# ── 배포판 최소 버전 (바이너리 호환 하한) ────────────────────────────────────
# .deb 는 Ubuntu 24.04(noble)에서 빌드된다 — glibc 2.39, time64 전환 이후의
# 패키지명(libglib2.0-0t64·libgtk-3-0t64·libqt6core6t64 …), libadwaita 1.4+,
# Qt6 6.4+ 를 요구한다. Debian 12(bookworm)는 glibc 2.36 이고 t64 이전 이름을
# 쓰므로 의존 해석 단계에서 실패한다 → Debian 은 13(trixie)부터.
# .rpm 은 fedora:43 컨테이너에서 빌드된다. Qt IM 플러그인이 Qt private API 를
# 쓰기 때문에 빌드 당시보다 낮은 Qt 마이너에서는 로드되지 않는다 → 같은
# 릴리스(43) 이상.
MIN_UBUNTU="24.04"
MIN_DEBIAN="13"
MIN_FEDORA="43"

# $1 < $2 (버전 비교, sort -V 기준)
version_lt() {
	[[ $1 != "$2" && "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -1)" == "$1" ]]
}

# 코드명 → upstream 버전. 파생 배포판(Mint·Pop!_OS·elementary·Zorin·Neon 등)은
# 자체 VERSION_ID 를 쓰지만 os-release 에 베이스 코드명을 함께 남기므로,
# 이것으로 실제 호환성을 판정한다. 미등록 코드명은 빈 문자열(→ 안내만).
ubuntu_version_of() {
	case "$1" in
		focal)     echo "20.04" ;;
		jammy)     echo "22.04" ;;
		kinetic)   echo "22.10" ;;
		lunar)     echo "23.04" ;;
		mantic)    echo "23.10" ;;
		noble)     echo "24.04" ;;
		oracular)  echo "24.10" ;;
		plucky)    echo "25.04" ;;
		questing)  echo "25.10" ;;
	esac
}

debian_version_of() {
	case "$1" in
		buster)   echo "10" ;;
		bullseye) echo "11" ;;
		bookworm) echo "12" ;;
		trixie)   echo "13" ;;
		forky)    echo "14" ;;
	esac
}

# ── 패키지 매니저·설치 모드 감지 ─────────────────────────────────────────────
# UNIM 이 게시하는 바이너리는 .deb(Ubuntu/Debian)와 .rpm(Fedora) 두 종류다.
# 그 외 배포판은 "왜 안 되는지 + 무엇을 하면 되는지"를 구체적으로 안내한다
# (막연한 실패보다 소스 빌드 경로를 알려주는 편이 실제로 도움이 된다).
detect_package_manager() {
	# 1) 불변(atomic/immutable) 시스템 — /usr 이 읽기 전용이라 apt/dnf 가 무력하다.
	#    Fedora Atomic(Silverblue·Kinoite·Bazzite·Nobara Atomic)은 rpm-ostree 로
	#    로컬 rpm 을 레이어링할 수 있으므로 정식 지원한다(재부팅 필요).
	if [[ -f /run/ostree-booted ]]; then
		if command -v rpm-ostree >/dev/null 2>&1; then
			PKG_KIND="rpm"
			INSTALL_MODE="rpm-ostree"
			MANIFEST="SHA256SUMS-rpm"
			ASSET_RE="$RPM_ASSET_RE"
			info "불변(atomic) 시스템을 감지했습니다 — rpm-ostree 로 패키지를 레이어링합니다." \
			     "Detected an atomic (image-based) system — packages will be layered via rpm-ostree."
			return
		fi
		err "불변(ostree) 시스템이지만 rpm-ostree 를 찾을 수 없어 설치할 수 없습니다." \
		    "This is an ostree-based system but rpm-ostree was not found, so installation cannot proceed."
	fi

	# 2) openSUSE MicroOS 등 트랜잭셔널 업데이트 계열.
	if command -v transactional-update >/dev/null 2>&1; then
		err "openSUSE MicroOS 등 트랜잭셔널 시스템은 아직 지원하지 않습니다 (게시된 .rpm 은 Fedora 전용). 소스 빌드를 사용하세요: https://github.com/${REPO}#설치" \
		    "Transactional systems such as openSUSE MicroOS are not supported yet (the published .rpm targets Fedora). Please build from source: https://github.com/${REPO}#installation"
	fi

	# 3) 정식 지원 경로.
	if command -v apt-get >/dev/null 2>&1 && command -v dpkg >/dev/null 2>&1; then
		PKG_KIND="deb"
		INSTALL_MODE="apt"
		MANIFEST="SHA256SUMS"
		ASSET_RE="$DEB_ASSET_RE"
		return
	fi
	if command -v dnf >/dev/null 2>&1 && command -v rpm >/dev/null 2>&1; then
		PKG_KIND="rpm"
		INSTALL_MODE="dnf"
		MANIFEST="SHA256SUMS-rpm"
		ASSET_RE="$RPM_ASSET_RE"
		return
	fi

	# 4) 미지원 — 감지된 패키지 매니저별로 이유를 특정해 안내한다.
	local why_ko="" why_en=""
	if command -v zypper >/dev/null 2>&1; then
		why_ko="openSUSE/SUSE 는 아직 지원하지 않습니다 — 게시된 .rpm 은 Fedora 패키지명에 의존합니다."
		why_en="openSUSE/SUSE is not supported yet — the published .rpm depends on Fedora package names."
	elif command -v pacman >/dev/null 2>&1; then
		why_ko="Arch 계열(Arch·Manjaro·EndeavourOS·SteamOS)용 바이너리는 아직 게시하지 않습니다."
		why_en="Binaries for Arch-based distributions (Arch, Manjaro, EndeavourOS, SteamOS) are not published yet."
	elif command -v apk >/dev/null 2>&1; then
		why_ko="Alpine 은 musl libc 기반이라 glibc 용 바이너리를 쓸 수 없습니다."
		why_en="Alpine uses musl libc, so the glibc binaries cannot run there."
	elif command -v xbps-install >/dev/null 2>&1 || command -v eopkg >/dev/null 2>&1 \
	     || command -v emerge >/dev/null 2>&1 || command -v nix-env >/dev/null 2>&1; then
		why_ko="이 배포판용 바이너리는 아직 게시하지 않습니다."
		why_en="Binaries for this distribution are not published yet."
	else
		why_ko="지원하는 패키지 매니저(apt-get+dpkg 또는 dnf+rpm)를 찾지 못했습니다."
		why_en="No supported package manager found (needs apt-get+dpkg or dnf+rpm)."
	fi
	err "${why_ko} 현재 바이너리 제공 대상: Ubuntu ${MIN_UBUNTU}+ / Debian ${MIN_DEBIAN}+ (.deb), Fedora ${MIN_FEDORA}+ (.rpm). 소스 빌드는 모든 배포판에서 가능합니다: https://github.com/${REPO}#설치" \
	    "${why_en} Binaries are currently provided for: Ubuntu ${MIN_UBUNTU}+ / Debian ${MIN_DEBIAN}+ (.deb) and Fedora ${MIN_FEDORA}+ (.rpm). Building from source works on any distribution: https://github.com/${REPO}#installation"
}

# ── 배포판 버전 게이트 ───────────────────────────────────────────────────────
# 베이스 배포판·버전을 추정한 뒤 최소 버전과 비교한다. 추정에 실패하면(미등록
# 파생·롤링) 안내만 하고 통과시키며, 실제 부적합은 apt/dnf 의 의존 해석이 잡는다.
check_distro_version() {
	[[ -r /etc/os-release ]] || return 0

	local base="" ver="" pretty="" min=""

	# EL 계열은 별도 취급 — dnf 는 있지만 게시 .rpm(Fedora 빌드)과 호환되지 않는다.
	case "${ID:-}" in
		rhel|centos|almalinux|rocky|ol)
			err "RHEL 계열(${PRETTY_NAME:-${ID}})은 아직 지원하지 않습니다 — 게시된 .rpm 은 Fedora ${MIN_FEDORA} 에서 빌드되어 더 새로운 시스템 라이브러리를 요구합니다. 소스 빌드를 사용하세요: https://github.com/${REPO}#설치" \
			    "RHEL-family (${PRETTY_NAME:-${ID}}) is not supported yet — the published .rpm is built on Fedora ${MIN_FEDORA} and needs newer system libraries. Please build from source: https://github.com/${REPO}#installation"
			;;
	esac

	# 베이스 판정: 정식 배포판 → 그대로, 파생 → 코드명으로 upstream 버전 역산.
	case "${ID:-}" in
		ubuntu) base="ubuntu"; ver="${VERSION_ID:-}" ;;
		debian) base="debian"; ver="${VERSION_ID:-}" ;;
		fedora) base="fedora"; ver="${VERSION_ID:-}" ;;
		*)
			if [[ -n ${UBUNTU_CODENAME:-} ]]; then
				base="ubuntu"; ver="$(ubuntu_version_of "$UBUNTU_CODENAME")"
			elif [[ ${ID_LIKE:-} == *ubuntu* && -n ${VERSION_CODENAME:-} ]]; then
				base="ubuntu"; ver="$(ubuntu_version_of "$VERSION_CODENAME")"
			elif [[ ${ID_LIKE:-} == *debian* && -n ${VERSION_CODENAME:-} ]]; then
				base="debian"; ver="$(debian_version_of "$VERSION_CODENAME")"
			elif [[ ${ID_LIKE:-} == *fedora* ]]; then
				# Nobara·Ultramarine 등은 Fedora 릴리스 번호를 그대로 쓴다.
				base="fedora"; ver="${VERSION_ID:-}"
			fi
			;;
	esac

	case "$base" in
		ubuntu) pretty="Ubuntu"; min="$MIN_UBUNTU" ;;
		debian) pretty="Debian"; min="$MIN_DEBIAN" ;;
		fedora) pretty="Fedora"; min="$MIN_FEDORA" ;;
		*)
			info "베이스 배포판을 특정하지 못했습니다 (ID=${ID:-?}). 계속 진행하며, 호환되지 않으면 패키지 매니저가 의존성 단계에서 알려줍니다." \
			     "Could not determine the base distribution (ID=${ID:-?}). Continuing; if it is incompatible, the package manager will report it at the dependency stage."
			return 0
			;;
	esac

	# 패키지 종류와 베이스 계열이 어긋나는 경우(예: apt 를 얹은 Fedora)는 안내만.
	if [[ ($PKG_KIND == "deb" && $base == "fedora") || ($PKG_KIND == "rpm" && $base != "fedora") ]]; then
		info "패키지 매니저(${INSTALL_MODE})와 배포판(${pretty})의 조합이 이례적입니다. 계속 진행합니다." \
		     "Unusual combination of package manager (${INSTALL_MODE}) and distribution (${pretty}). Continuing anyway."
		return 0
	fi

	if [[ -z $ver ]]; then
		info "${pretty} 롤링/테스팅 또는 미등록 릴리스로 보입니다. ${pretty} ${min} 이상 기준으로 계속 진행합니다." \
		     "This looks like a rolling/testing or unrecognized ${pretty} release. Continuing; ${pretty} ${min}+ is assumed."
		return 0
	fi

	if version_lt "$ver" "$min"; then
		local base_note_ko="" base_note_en=""
		if [[ ${ID:-} != "$base" ]]; then
			base_note_ko=" (${PRETTY_NAME:-${ID:-?}} → ${pretty} ${ver} 기반)"
			base_note_en=" (${PRETTY_NAME:-${ID:-?}} is based on ${pretty} ${ver})"
		fi
		err "${pretty} ${ver}${base_note_ko} 는 지원 범위 밖입니다 — ${pretty} ${min} 이상이 필요합니다. 배포 패키지가 더 새로운 시스템 라이브러리(glibc·GTK·Qt)를 요구해 설치가 실패합니다. 소스 빌드를 사용하세요: https://github.com/${REPO}#설치" \
		    "${pretty} ${ver}${base_note_en} is not supported — ${pretty} ${min} or newer is required. The published packages need newer system libraries (glibc/GTK/Qt) than this release provides. Please build from source instead: https://github.com/${REPO}#installation"
	fi
}

# ── 1. 환경 가드 ────────────────────────────────────────────────────────────
guard_environment() {
	# (bash 감지는 스크립트 최상단 POSIX 가드에서 이미 처리됨.)
	if [[ "$(uname -s)" != "Linux" ]]; then
		err "리눅스 전용입니다. 현재 OS: $(uname -s)" \
		    "Linux only. Current OS: $(uname -s)"
	fi

	# os-release 를 먼저 읽는다 — 불변 배포판·EL 계열 판정에 필요.
	if [[ -r /etc/os-release ]]; then
		# shellcheck disable=SC1091  # /etc/os-release 는 런타임 파일이라 정적 분석 불가
		. /etc/os-release
	fi

	detect_package_manager
	check_distro_version

	local arch
	if [[ $PKG_KIND == "deb" ]]; then
		arch="$(dpkg --print-architecture)"
		if [[ "$arch" != "amd64" ]]; then
			err "현재 amd64 빌드만 제공합니다. 감지된 아키텍처: ${arch}" \
			    "Only amd64 builds are provided at this time. Detected architecture: ${arch}"
		fi
	else
		arch="$(uname -m)"
		if [[ "$arch" != "x86_64" ]]; then
			err "현재 x86_64 빌드만 제공합니다. 감지된 아키텍처: ${arch}" \
			    "Only x86_64 builds are provided at this time. Detected architecture: ${arch}"
		fi
	fi

	local pkg_hint
	case "$INSTALL_MODE" in
		apt)        pkg_hint="sudo apt-get install -y" ;;
		dnf)        pkg_hint="sudo dnf install -y" ;;
		rpm-ostree) pkg_hint="sudo rpm-ostree install" ;;
		*)          pkg_hint="(패키지 매니저 / package manager)" ;;
	esac

	if ! command -v curl >/dev/null 2>&1; then
		err "curl 이 필요합니다: ${pkg_hint} curl" \
		    "curl is required: ${pkg_hint} curl"
	fi

	if ! command -v sha256sum >/dev/null 2>&1; then
		err "sha256sum(coreutils) 이 필요합니다: ${pkg_hint} coreutils" \
		    "sha256sum (coreutils) is required: ${pkg_hint} coreutils"
	fi

	# root/sudo 결정. SUDO 는 전역으로 노출. (check 는 시스템을 바꾸지 않으므로 생략.)
	if [[ $ACTION == "check" ]]; then
		SUDO=""
		return
	fi
	if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
		SUDO=""
	else
		if ! command -v sudo >/dev/null 2>&1; then
			err "root 권한이 필요하지만 sudo 가 없습니다. root 로 다시 실행하세요." \
			    "Root privileges are required but sudo is not available. Re-run as root."
		fi
		# 안내는 실제로 설치를 시작하기 직전에 한다 (조회만 하고 끝나는
		# --update/--check 경로에서 불필요한 암호 경고를 띄우지 않도록).
		SUDO="sudo"
	fi
}

# ── 2. 버전 결정 → TAG (전역) ───────────────────────────────────────────────
resolve_version() {
	if [[ -n ${UNIM_VERSION:-} ]]; then
		local v="$UNIM_VERSION"
		[[ $v == v* ]] || v="v$v"
		if [[ ! $v =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
			err "UNIM_VERSION 형식이 올바르지 않습니다: '${UNIM_VERSION}' (예: v0.4.0)" \
			    "Invalid UNIM_VERSION format: '${UNIM_VERSION}' (expected e.g. v0.4.0)"
		fi
		TAG="$v"
		return
	fi

	info "최신 릴리스 조회 중..." "Looking up the latest release..."
	local json
	if ! json="$(curl -fsSL --retry 3 --connect-timeout 15 "$API_LATEST")"; then
		err "릴리스 조회 실패 — 네트워크/레이트리밋 확인 (아직 공개된 릴리스가 없을 수도 있습니다). UNIM_VERSION=vX.Y.Z 로 버전 고정 가능." \
		    "Failed to query the latest release — check your network / rate limit (there may be no published release yet). You can pin a version with UNIM_VERSION=vX.Y.Z."
	fi

	local tag
	if command -v jq >/dev/null 2>&1; then
		tag="$(printf '%s' "$json" | jq -r '.tag_name // empty')"
	else
		# no-jq 경로: grep 무매칭 시 pipefail+set -e 로 무메시지 조기종료되지 않도록
		# 파이프라인 말미에 `|| true` — 빈 tag 로 아래 empty-tag 가드까지 정상 도달한다.
		tag="$(printf '%s' "$json" | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4 || true)"
	fi

	if [[ -z $tag || $tag == "null" ]]; then
		err "릴리스 태그를 찾지 못했습니다 — 아직 공개된 릴리스가 없거나 레이트리밋일 수 있습니다. UNIM_VERSION=vX.Y.Z 로 고정하세요." \
		    "Could not find a release tag — there may be no published release yet, or you hit a rate limit. Pin one with UNIM_VERSION=vX.Y.Z."
	fi
	# API 응답 태그도 UNIM_VERSION 과 동일한 형식 검증 (URL 보간 전 가드).
	if [[ ! $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
		err "API 가 반환한 태그 형식이 예상과 다릅니다: '${tag}'" \
		    "Unexpected release tag format from the GitHub API: '${tag}'"
	fi
	TAG="$tag"
}

# ── 3. 다운로드 (mktemp + trap). WORKDIR 는 전역 ────────────────────────────
download_assets() {
	local tag="$1" workdir="$2"
	local base="${RELEASE_BASE}/${tag}"

	info "${MANIFEST} 매니페스트 다운로드 중 (${tag})..." \
	     "Downloading the ${MANIFEST} manifest (${tag})..."
	if ! curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 \
	          -o "${workdir}/${MANIFEST}" "${base}/${MANIFEST}"; then
		err "${MANIFEST} 다운로드 실패 — 태그 '${tag}' 에 해당하는 릴리스 자산을 찾을 수 없습니다." \
		    "Failed to download ${MANIFEST} — no matching release asset for tag '${tag}'."
	fi

	# 매니페스트에서 자산 목록 추출 + 파일명 검증 (경로 탈출 가드).
	local names=()
	local name
	while IFS= read -r name; do
		[[ -z $name ]] && continue
		name="${name#\*}"  # sha256sum 바이너리 모드 마커 제거
		if [[ $name == */* ]]; then
			err "매니페스트에 경로 문자가 포함된 자산명이 있습니다 (거부): ${name}" \
			    "Manifest contains an asset name with a path separator (rejected): ${name}"
		fi
		if [[ ! $name =~ $ASSET_RE ]]; then
			err "매니페스트에 예상치 못한 자산명이 있습니다 (거부): ${name}" \
			    "Manifest contains an unexpected asset name (rejected): ${name}"
		fi
		names+=("$name")
	done < <(awk '{print $2}' "${workdir}/${MANIFEST}")

	if [[ ${#names[@]} -eq 0 ]]; then
		err "매니페스트에서 유효한 패키지 자산을 찾지 못했습니다." \
		    "No valid package assets found in the manifest."
	fi

	info "패키지 ${#names[@]}개 다운로드 중..." "Downloading ${#names[@]} package(s)..."
	for name in "${names[@]}"; do
		info "  → ${name}" "  → ${name}"
		if ! curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 \
		          -o "${workdir}/${name}" "${base}/${name}"; then
			err "다운로드 실패: ${name}" "Download failed: ${name}"
		fi
	done
}

# ── 4. 체크섬 검증 (필수 게이트) ────────────────────────────────────────────
verify_checksums() {
	local workdir="$1"
	info "SHA256 체크섬 검증 중..." "Verifying SHA256 checksums..."
	if ! ( cd "$workdir" && sha256sum -c "$MANIFEST" ); then
		err "체크섬 불일치 — 다운로드 손상 또는 변조 의심. 재시도하거나 이슈로 제보하세요. (아무것도 설치되지 않았습니다.)" \
		    "Checksum mismatch — download corruption or tampering suspected. Retry or report an issue. (Nothing was installed.)"
	fi
	info "체크섬 검증 통과." "Checksums verified."
}

# ── 5. 설치 (apt 로 로컬 .deb 설치, 의존성 자동 해결) ────────────────────────
install_debs() {
	local workdir="$1"

	# apt-get update 는 관대하게: 고장난 서드파티 저장소 때문에 설치가
	# 막히지 않도록 실패해도 경고만 한다 (실제 게이트는 install 단계).
	info "패키지 목록 갱신 중..." "Refreshing package lists..."
	# shellcheck disable=SC2086  # $SUDO 는 root 일 때 빈 문자열이어야 하므로 의도적으로 비인용
	if ! $SUDO apt-get update -qq; then
		info "경고: apt-get update 가 실패했습니다 (계속 진행)." \
		     "Warning: apt-get update failed (continuing anyway)."
	fi

	info "UNIM 패키지 설치 중..." "Installing UNIM packages..."
	# 절대경로 glob → apt 가 로컬 파일로 인식, 패키지 간·외부 런타임 의존성
	# 동시 해결. dpkg -i 대신 apt 사용 (의존성 자동 처리).
	# shellcheck disable=SC2086  # $SUDO 는 root 일 때 빈 문자열이어야 하므로 의도적으로 비인용
	if ! $SUDO apt-get install -y "$workdir"/*.deb; then
		err "설치 중 오류가 발생했습니다. 'sudo apt-get -f install' 로 의존성을 정리한 뒤 재시도하세요." \
		    "Installation failed. Run 'sudo apt-get -f install' to repair dependencies, then retry."
	fi
}

# ── 5b. 설치 (dnf 로 로컬 .rpm 설치, 의존성 자동 해결) ───────────────────────
install_rpms() {
	local workdir="$1"

	info "UNIM 패키지 설치 중..." "Installing UNIM packages..."
	# dnf install 은 로컬 파일 경로를 직접 받아 저장소 의존성까지 함께 해결한다.
	# (dnf localinstall 은 deprecated alias — dnf4/dnf5 공통으로 install 사용.
	#  apt-get update 대응 단계 불필요: dnf 는 메타데이터를 자체 갱신한다.)
	# shellcheck disable=SC2086  # $SUDO 는 root 일 때 빈 문자열이어야 하므로 의도적으로 비인용
	if ! $SUDO dnf install -y "$workdir"/*.rpm; then
		err "설치 중 오류가 발생했습니다. 네트워크/저장소 상태를 확인한 뒤 재시도하세요." \
		    "Installation failed. Check your network / repository state and retry."
	fi
}

# ── 5c. 설치 (rpm-ostree 레이어링, 불변 시스템) ──────────────────────────────
install_rpms_ostree() {
	local workdir="$1"

	info "rpm-ostree 로 패키지를 레이어링하는 중 (시간이 걸립니다)..." \
	     "Layering packages with rpm-ostree (this takes a while)..."
	# 재설치·업그레이드 시 같은 패키지가 이미 레이어링돼 있으면 --idempotent 로
	# 중복 오류를 피한다. 새 배포는 다음 부팅부터 적용된다.
	# shellcheck disable=SC2086  # $SUDO 는 root 일 때 빈 문자열이어야 하므로 의도적으로 비인용
	if ! $SUDO rpm-ostree install --idempotent -y "$workdir"/*.rpm; then
		err "rpm-ostree 레이어링에 실패했습니다. 'rpm-ostree status' 로 상태를 확인하고, 필요하면 'rpm-ostree rollback' 으로 되돌리세요." \
		    "rpm-ostree layering failed. Check 'rpm-ostree status', and use 'rpm-ostree rollback' to revert if needed."
	fi
}

# ── 설치된 UNIM 버전 조회 (미설치면 빈 문자열) ──────────────────────────────
# 기준 패키지는 unim-common — 모든 구성이 의존하는 코어라 항상 함께 설치된다.
installed_version() {
	local v=""
	if [[ $PKG_KIND == "deb" ]]; then
		v="$(dpkg-query -W -f='${Version}' unim-common 2>/dev/null || true)"
	else
		v="$(rpm -q --qf '%{VERSION}' unim-common 2>/dev/null || true)"
		[[ $v == *"not installed"* ]] && v=""
	fi
	# deb 는 '0.4.0-1', rpm 은 '0.4.0' → 데비안 리비전을 떼어 upstream 만 남긴다.
	printf '%s' "${v%%-*}"
}

# ── 6. 성공 안내 ────────────────────────────────────────────────────────────
print_success() {
	local tag="$1"
	echo
	info "UNIM ${tag} 설치 완료!" "UNIM ${tag} installed successfully!"
	echo
	msg "다음 단계:" "Next steps:"
	msg "  1. 로그아웃 후 다시 로그인하세요 — 환경변수가 새 세션에 적용됩니다." \
	    "  1. Log out and back in — the environment variables take effect in a new session."
	msg "  2. 재로그인한 첫 세션에서 첫 실행 마법사(unim-settings)가 자동으로 뜨며," \
	    "  2. On your first session after logging back in, the first-run wizard"
	msg "     기본 입력기 지정까지 GUI 로 안내합니다." \
	    "     (unim-settings) launches automatically and guides you through setting the default IME."
	msg "     수동 실행: unim-settings --first-run" \
	    "     Run it manually with: unim-settings --first-run"
	msg "  · GNOME 사용자는 'unim-gnome' 확장이 함께 설치됩니다. 재로그인 후 한 번만 실행하세요: gnome-extensions enable unim-gnome@from104.github.io" \
	    "  · GNOME users: the 'unim-gnome' extension is installed too. After logging back in, run once: gnome-extensions enable unim-gnome@from104.github.io"
	msg "    (지원하는 GNOME Shell 버전 범위 밖이면 위 명령이 통해도 확장이 비활성으로 남습니다 — 'gnome-extensions show unim-gnome@from104.github.io' 로 지원 범위를 확인하세요.)" \
	    "    (Outside the supported GNOME Shell version range, the extension stays disabled even after the command above — check the supported range with 'gnome-extensions show unim-gnome@from104.github.io'.)"
	if [[ $INSTALL_MODE == "rpm-ostree" ]]; then
		msg "  · 불변 시스템에서는 재부팅해야 새 배포가 적용됩니다: systemctl reboot" \
		    "  · On atomic systems the new deployment applies after a reboot: systemctl reboot"
	fi
	echo
}

# ── 6b. 업데이트 완료 안내 ──────────────────────────────────────────────────
print_update_success() {
	local from="$1" to="$2"
	echo
	info "UNIM 업데이트 완료: ${from} → ${to}" "UNIM updated: ${from} → ${to}"
	echo
	msg "다음 단계:" "Next steps:"
	if [[ $INSTALL_MODE == "rpm-ostree" ]]; then
		msg "  1. 재부팅하면 새 배포가 적용됩니다: systemctl reboot" \
		    "  1. Reboot to activate the new deployment: systemctl reboot"
	else
		msg "  1. 로그아웃 후 다시 로그인하세요 — 데몬·IM 모듈·GNOME 확장이 새 버전으로 교체됩니다." \
		    "  1. Log out and back in — the daemon, IM modules, and GNOME extension are replaced with the new build."
		msg "     (재로그인 전까지는 이전 버전 데몬이 계속 실행됩니다.)" \
		    "     (Until you log back in, the previously running daemon stays active.)"
	fi
	msg "  · 설정은 그대로 유지됩니다 (~/.config/unim/)." \
	    "  · Your settings are preserved (~/.config/unim/)."
	echo
}

# ── 사용법 ──────────────────────────────────────────────────────────────────
usage() {
	msg "UNIM 설치 스크립트" "UNIM installer"
	echo
	msg "사용법:" "Usage:"
	msg "  bash install.sh [동작] [옵션]" "  bash install.sh [action] [options]"
	msg "  curl -fsSL <url>/install.sh | bash -s -- [동작] [옵션]" \
	    "  curl -fsSL <url>/install.sh | bash -s -- [action] [options]"
	echo
	msg "동작:" "Actions:"
	msg "  (없음)      설치 (기본)" "  (none)      install (default)"
	msg "  --update    설치돼 있으면 최신 버전으로 업데이트" \
	    "  --update    update an existing installation to the latest release"
	msg "  --check     설치 버전과 최신 버전만 확인 (아무것도 바꾸지 않음)" \
	    "  --check     report installed vs latest version only (changes nothing)"
	echo
	msg "옵션:" "Options:"
	msg "  --force     이미 최신이어도 다시 설치" "  --force     reinstall even if already up to date"
	msg "  -h, --help  이 도움말" "  -h, --help  show this help"
	echo
	msg "환경변수: UNIM_VERSION=vX.Y.Z (버전 고정), UNIM_BASE_URL (미러)" \
	    "Environment: UNIM_VERSION=vX.Y.Z (pin a version), UNIM_BASE_URL (mirror)"
}

# ── 인자 파싱 ───────────────────────────────────────────────────────────────
parse_args() {
	while [[ $# -gt 0 ]]; do
		case "$1" in
			--update|update)   ACTION="update" ;;
			--check|check)     ACTION="check" ;;
			--install|install) ACTION="install" ;;
			--force|-f)        FORCE=1 ;;
			-h|--help|help)    usage; exit 0 ;;
			*)
				usage >&2
				err "알 수 없는 인자: $1" "Unknown argument: $1"
				;;
		esac
		shift
	done
}

# ── main: 파이프 절단 가드 (마지막 줄에서만 호출) ───────────────────────────
main() {
	parse_args "$@"
	guard_environment

	local installed
	installed="$(installed_version)"

	# TAG 결정 (모든 동작이 최신/지정 버전을 알아야 한다)
	TAG=""
	resolve_version
	local latest="${TAG#v}"

	# ── check: 조회만 하고 종료 ──────────────────────────────────────────
	if [[ $ACTION == "check" ]]; then
		if [[ -z $installed ]]; then
			info "UNIM 이 설치되어 있지 않습니다. 최신 릴리스: ${latest}" \
			     "UNIM is not installed. Latest release: ${latest}"
		elif [[ $installed == "$latest" ]]; then
			info "최신 버전입니다 (${installed})." "Up to date (${installed})."
		elif version_lt "$installed" "$latest"; then
			info "업데이트 가능: ${installed} → ${latest}  (적용: install.sh --update)" \
			     "Update available: ${installed} → ${latest}  (apply with: install.sh --update)"
		else
			info "설치된 버전(${installed})이 릴리스(${latest})보다 새롭습니다." \
			     "The installed version (${installed}) is newer than the release (${latest})."
		fi
		return 0
	fi

	# ── update: 필요할 때만 내려받는다 ───────────────────────────────────
	if [[ $ACTION == "update" ]]; then
		if [[ -z $installed ]]; then
			info "UNIM 이 설치되어 있지 않아 새로 설치합니다." \
			     "UNIM is not installed yet — performing a fresh install."
			ACTION="install"
		elif [[ $installed == "$latest" && $FORCE -eq 0 ]]; then
			info "이미 최신 버전입니다 (${installed}). 다시 설치하려면 --force 를 쓰세요." \
			     "Already up to date (${installed}). Use --force to reinstall anyway."
			return 0
		elif version_lt "$latest" "$installed" && [[ $FORCE -eq 0 ]]; then
			err "설치된 버전(${installed})이 대상 버전(${latest})보다 새롭습니다. 다운그레이드하려면 --force 를 쓰세요." \
			    "The installed version (${installed}) is newer than the target (${latest}). Use --force to downgrade."
		else
			info "업데이트: ${installed} → ${latest}" "Updating: ${installed} → ${latest}"
		fi
	fi

	if [[ -n $SUDO ]]; then
		info "설치 중 sudo 암호를 물어볼 수 있습니다." \
		     "You may be prompted for your sudo password during installation."
	fi

	# 작업 디렉토리 + 정리 트랩
	WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/unim-install.XXXXXX")"
	trap 'rm -rf "$WORKDIR"' EXIT INT TERM

	download_assets "$TAG" "$WORKDIR"
	verify_checksums "$WORKDIR"
	case "$INSTALL_MODE" in
		rpm-ostree) install_rpms_ostree "$WORKDIR" ;;
		dnf)        install_rpms "$WORKDIR" ;;
		*)          install_debs "$WORKDIR" ;;
	esac

	if [[ $ACTION == "update" ]]; then
		print_update_success "$installed" "$latest"
	else
		print_success "$TAG"
	fi
}

main "$@"
