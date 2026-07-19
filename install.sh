#!/usr/bin/env bash
# ============================================================================
# UNIM — 한 줄 설치 스크립트 / one-line installer
# ============================================================================
#
# 용도 / Purpose:
#   GitHub Releases 에 게시된 UNIM .deb 패키지를 내려받아 apt 로 설치한다.
#   Downloads UNIM's published .deb packages from GitHub Releases and
#   installs them via apt (dependencies resolved automatically).
#
# 사용법 / Usage:
#   curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
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
#   이 스크립트는 mktemp 임시 디렉토리와 apt 트랜잭션 외에는 시스템을 건드리지
#   않는다. 모든 .deb 는 SHA256 체크섬으로 검증되며, 검증 실패 시 아무것도
#   설치하지 않고 중단한다 (부분 설치 없음).
#   This script touches nothing outside a mktemp working directory and the apt
#   transaction. Every .deb is SHA256-verified; on mismatch it aborts without
#   installing anything (no partial installs).
#
# 대상 / Target: Ubuntu 24.04 (noble) 이상 / 동급 Debian, amd64.
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

# 릴리스 자산 파일명 검증 정규식 (경로 탈출 가드).
ASSET_RE='^unim[A-Za-z0-9._+~-]*\.deb$'

# ── 1. 환경 가드 ────────────────────────────────────────────────────────────
guard_environment() {
	# (bash 감지는 스크립트 최상단 POSIX 가드에서 이미 처리됨.)
	if [[ "$(uname -s)" != "Linux" ]]; then
		err "리눅스 전용입니다. 현재 OS: $(uname -s)" \
		    "Linux only. Current OS: $(uname -s)"
	fi

	if ! command -v apt-get >/dev/null 2>&1 || ! command -v dpkg >/dev/null 2>&1; then
		err "apt 기반 배포판(Ubuntu/Debian)에서만 동작합니다 (apt-get·dpkg 필요)." \
		    "Only works on apt-based distributions (Ubuntu/Debian); apt-get and dpkg are required."
	fi

	# ID / ID_LIKE 가 debian·ubuntu 계열이 아니면 경고만 (하드 실패는 apt 부재 시).
	if [[ -r /etc/os-release ]]; then
		# shellcheck disable=SC1091  # /etc/os-release 는 런타임 파일이라 정적 분석 불가
		. /etc/os-release
		if [[ ${ID:-}${ID_LIKE:-} != *debian* && ${ID:-}${ID_LIKE:-} != *ubuntu* ]]; then
			info "경고: Debian/Ubuntu 계열이 아닌 것 같습니다 (ID=${ID:-?}). 계속 진행합니다." \
			     "Warning: this does not look like a Debian/Ubuntu derivative (ID=${ID:-?}). Continuing anyway."
		fi
	fi

	local arch
	arch="$(dpkg --print-architecture)"
	if [[ "$arch" != "amd64" ]]; then
		err "현재 amd64 빌드만 제공합니다. 감지된 아키텍처: ${arch}" \
		    "Only amd64 builds are provided at this time. Detected architecture: ${arch}"
	fi

	if ! command -v curl >/dev/null 2>&1; then
		err "curl 이 필요합니다: sudo apt-get install -y curl" \
		    "curl is required: sudo apt-get install -y curl"
	fi

	if ! command -v sha256sum >/dev/null 2>&1; then
		err "sha256sum(coreutils) 이 필요합니다: sudo apt-get install -y coreutils" \
		    "sha256sum (coreutils) is required: sudo apt-get install -y coreutils"
	fi

	# root/sudo 결정. SUDO 는 전역으로 노출.
	if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
		SUDO=""
	else
		if ! command -v sudo >/dev/null 2>&1; then
			err "root 권한이 필요하지만 sudo 가 없습니다. root 로 다시 실행하세요." \
			    "Root privileges are required but sudo is not available. Re-run as root."
		fi
		SUDO="sudo"
		info "설치 중 sudo 암호를 물어볼 수 있습니다." \
		     "You may be prompted for your sudo password during installation."
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
		err "릴리스 조회 실패 — 네트워크/레이트리밋 확인. UNIM_VERSION=vX.Y.Z 로 버전 고정 가능." \
		    "Failed to query the latest release — check your network / rate limit. You can pin a version with UNIM_VERSION=vX.Y.Z."
	fi

	local tag
	if command -v jq >/dev/null 2>&1; then
		tag="$(printf '%s' "$json" | jq -r '.tag_name // empty')"
	else
		tag="$(printf '%s' "$json" | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4)"
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

	info "SHA256SUMS 매니페스트 다운로드 중 (${tag})..." \
	     "Downloading the SHA256SUMS manifest (${tag})..."
	if ! curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 \
	          -o "${workdir}/SHA256SUMS" "${base}/SHA256SUMS"; then
		err "SHA256SUMS 다운로드 실패 — 태그 '${tag}' 에 해당하는 릴리스 자산을 찾을 수 없습니다." \
		    "Failed to download SHA256SUMS — no matching release asset for tag '${tag}'."
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
	done < <(awk '{print $2}' "${workdir}/SHA256SUMS")

	if [[ ${#names[@]} -eq 0 ]]; then
		err "매니페스트에서 유효한 .deb 자산을 찾지 못했습니다." \
		    "No valid .deb assets found in the manifest."
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
	if ! ( cd "$workdir" && sha256sum -c SHA256SUMS ); then
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
	msg "  · GNOME 사용자는 'unim-gnome' 확장이 함께 설치됩니다 — 재로그인 후 활성화되어 상단 패널 인디케이터가 나타납니다." \
	    "  · GNOME users: the 'unim-gnome' extension is installed too — it activates after re-login and adds a top-panel indicator."
	echo
}

# ── main: 파이프 절단 가드 (마지막 줄에서만 호출) ───────────────────────────
main() {
	guard_environment

	# TAG 결정
	TAG=""
	resolve_version

	# 작업 디렉토리 + 정리 트랩
	WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/unim-install.XXXXXX")"
	trap 'rm -rf "$WORKDIR"' EXIT INT TERM

	download_assets "$TAG" "$WORKDIR"
	verify_checksums "$WORKDIR"
	install_debs "$WORKDIR"
	print_success "$TAG"
}

main "$@"
