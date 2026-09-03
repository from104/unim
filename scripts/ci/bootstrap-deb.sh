#!/usr/bin/env bash
# deb 빌드 컨테이너 부트스트랩 — CI 와 로컬 매트릭스(scripts/build-linux-matrix.sh)가
# 같은 스크립트를 쓴다. 워크플로 YAML 에 로직을 두면 로컬 재현과 이원화되기 때문.
#
# 전제: ubuntu:24.04 / ubuntu:26.04 / debian:13 계열 컨테이너(root) 또는
#       sudo 가능한 러너. debian/control 의 Build-Depends 를 그대로 설치하고,
#       배포판 rust 가 MSRV(1.78) 미달이면 rustup stable 을 올린다.
#
# 사용법: scripts/ci/bootstrap-deb.sh

set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"

$SUDO apt-get update -qq
$SUDO apt-get install -y -qq --no-install-recommends \
    git ca-certificates build-essential devscripts equivs >/dev/null

# scripts/ci/verify-installed.sh (--smoke 가 부르는 설치 후 런타임 검증)가
# 요구하는 런타임 도구. Build-Depends 에는 없다 — 빌드가 아니라 검증용.
$SUDO apt-get install -y -qq --no-install-recommends \
    python3 dbus dbus-daemon libglib2.0-bin >/dev/null

# scripts/ci/functional-test.sh (--smoke 가 verify-installed.sh 다음에 부르는
# Xvfb 기반 기능 타이핑 검증)가 요구하는 도구.
#   xvfb            — 헤드리스 X 서버(WM 없이 Xvfb 단독으로 충분함을 실측 확인)
#   x11-utils       — xwininfo(창 좌표)·xdpyinfo(서버 대기)·xwd(실패 진단 스샷)
#   xdotool         — XTEST 키 입력·창 포커스
#   imagemagick     — harness.py 의 실패 스크린샷(import -window, 없으면 생략)
# tests/unim-test-xim 컴파일에 필요한 dev 헤더(control 의 Build-Depends 는
# GTK/Qt/glib/X11 은 이미 있으나 Xft/fontconfig 는 없다 — XIM 앱 전용).
$SUDO apt-get install -y -qq --no-install-recommends \
    xvfb x11-utils xdotool imagemagick libxft-dev libfontconfig-dev >/dev/null || \
    echo "⚠️  xvfb/x11-utils/xdotool 중 일부가 이 배포판에 없다 — functional-test.sh 가 감지해 스킵한다"

# 'apt build-dep .' 은 소스 저장소(deb-src)를 요구한다. 24.04+/데비안13 은
# deb822(*.sources) 형식이라 옛 one-line sources.list 용 sed 로는 안 잡힌다 —
# 두 형식 모두 처리한다.
for f in /etc/apt/sources.list.d/*.sources; do
    [ -f "$f" ] && $SUDO sed -i 's/^Types: deb$/Types: deb deb-src/' "$f"
done
if [ -f /etc/apt/sources.list ] && grep -q '^deb ' /etc/apt/sources.list &&
   ! grep -q '^deb-src ' /etc/apt/sources.list; then
    # 각 deb 줄을 유지한 채 deb-src 사본을 덧붙인다.
    $SUDO sed -i -n 'p; s/^deb /deb-src /p' /etc/apt/sources.list
fi
$SUDO apt-get update -qq

# Build-Depends 를 control 에서 그대로 — 향후 항목 추가가 자동으로 따라온다.
# (build-dep 은 소스 패키지가 아니라 로컬 debian/ 을 읽으므로 deb-src 가 없어도
#  동작하는 경우가 있으나, 배포판에 따라 요구하므로 위에서 켜 둔다.)
$SUDO apt-get build-dep -y -qq . >/dev/null

# rust: 배포판 cargo 가 1.78 미만이거나 없으면 rustup. debian/rules 가
# ~/.cargo/bin 을 PATH 앞에 붙이므로 설치만 하면 잡힌다.
need_rustup=1
if command -v cargo >/dev/null; then
    v=$(cargo --version | awk '{print $2}')
    maj=${v%%.*}; min=$(echo "$v" | cut -d. -f2)
    if [ "$maj" -gt 1 ] || { [ "$maj" -eq 1 ] && [ "$min" -ge 78 ]; }; then
        need_rustup=0
    fi
fi
if [ ! -x "$HOME/.cargo/bin/cargo" ] && [ "$need_rustup" -eq 1 ]; then
    echo "배포판 cargo 미달/부재 → rustup stable 설치"
    $SUDO apt-get install -y -qq --no-install-recommends curl >/dev/null
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
fi

. "$HOME/.cargo/env" 2>/dev/null || true
echo "✅ bootstrap-deb: $(. /etc/os-release && echo "$PRETTY_NAME") / cargo $(cargo --version 2>/dev/null | awk '{print $2}' || echo '?')"
