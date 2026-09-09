#!/usr/bin/env bash
# rpm 빌드 컨테이너 부트스트랩 — CI 레그와 로컬 매트릭스 공용.
#
# 사용법: scripts/ci/bootstrap-rpm.sh <tag>
#   tag: fedora43 | fedora44 | el10
#
# fedora:* 는 dnf5(builddep = dnf5-plugins), almalinux:10 은 dnf4 계열.
# el10 은 세 저장소가 더 필요하다:
#   - CRB   : libadwaita-devel, qt6-qtbase-private-devel (2026-08 실측)
#   - EPEL  : qt5-qtbase-devel/-private-devel(5.15.18), rpmlint
#     → RHEL 10 본체는 Qt5 를 제거했지만 EPEL 이 패키징하므로 spec 조건 분기
#       없이 Fedora 와 같은 11개 구성을 유지한다. 런타임에도 EPEL 필요
#       (unim-im-qt 의 Qt5 절반) — install.sh 가 안내한다.

set -euo pipefail

TAG="${1:?사용법: bootstrap-rpm.sh <tag>}"

# fedora 미러 타임아웃 완화(2026-09 실측: 기본 dnf 타임아웃으로 미러가
# 자주 Curl error 28 로 죽어 이후 dnf 호출 전부가 조기 실패했다) — 이
# 스크립트의 모든 dnf 호출에 공통 적용되도록 전역 dnf.conf 에 설정한다.
#
# dnf.conf 는 INI 다. 파일 끝에 append 하면 `[main]` 뒤에 다른 섹션이
# 붙어 있는 배포판(또는 나중에 붙는 경우)에서 키가 엉뚱한 섹션으로 들어가
# 조용히 무시된다. 그래서 반드시 `[main]` 바로 뒤에 삽입한다.
# (기존 값이 있으면 먼저 지워 중복 키를 만들지 않는다.)
if [ -f /etc/dnf/dnf.conf ] && grep -q '^\[main\]' /etc/dnf/dnf.conf; then
    sed -i -e '/^[[:space:]]*\(timeout\|retries\)[[:space:]]*=/d' \
           -e '/^\[main\]/a timeout=60\nretries=5' /etc/dnf/dnf.conf
else
    # `[main]` 이 없는(또는 파일 자체가 없는) 배포판 — 섹션째 새로 만든다.
    mkdir -p /etc/dnf
    printf '[main]\ntimeout=60\nretries=5\n' >> /etc/dnf/dnf.conf
fi

case "$TAG" in
    el*)
        dnf -y -q install dnf-plugins-core
        dnf config-manager --set-enabled crb
        dnf -y -q install epel-release
        ;;
    fedora*) ;;
    *) echo "❌ 알 수 없는 tag: $TAG (fedora43|fedora44|el10)"; exit 1 ;;
esac

dnf -y install git-core rpm-build rpmdevtools rpmlint \
               gawk findutils util-linux
# F41+ 는 dnf5(builddep 은 dnf5-plugins), dnf4 는 dnf-command(builddep).
dnf -y install dnf5-plugins || dnf -y install 'dnf-command(builddep)'

# scripts/ci/verify-installed.sh (--smoke 가 부르는 설치 후 런타임 검증)가
# 요구하는 런타임 도구. python3 는 spec BuildRequires 로 이미 들어오지만
# 명시해 순서 의존을 없앤다. glibc-common(ldd) 은 배포판 자체가 항상 갖춘다.
dnf -y install python3 dbus-daemon glib2 dbus-x11

# scripts/ci/functional-test.sh (--smoke 가 verify-installed.sh 다음에 부르는
# Xvfb 기반 기능 타이핑 검증)가 요구하는 도구 + tests/unim-test-xim 컴파일에
# 필요한 dev 헤더(spec BuildRequires 는 GTK/Qt/glib/X11 만 있고 Xft/fontconfig
# 는 없다 — XIM 테스트 앱 전용). 패키지명은 fedora43/el10 양쪽에서 dnf 로
# 실측(2026-09) — Fedora 는 예전 메타패키지 xorg-x11-utils 가 개별 패키지로
# 쪼개져 xwininfo 로 설치해야 하고(el10 도 동일), ImageMagick 은 두 배포판
# 모두 대문자 그대로다.
dnf -y install xorg-x11-server-Xvfb xwininfo xdotool ImageMagick libXft-devel fontconfig-devel || \
    echo "⚠️  Xvfb/xwininfo/xdotool 중 일부가 이 배포판에 없다 — functional-test.sh 가 감지해 스킵한다 " \
         "(el10 은 2026-09 기준 EPEL10 이 아직 Xvfb 를 패키징하지 않음, 코드 문제 아님)"

# spec 의 BuildRequires 를 그대로 해석 — 이름이 그 배포판에 실재하는지의
# 검증을 겸한다 (해석 실패 = 즉시 이 스텝에서 죽는다).
dnf builddep -y rpm/unim.spec

echo "✅ bootstrap-rpm: $(. /etc/os-release && echo "$PRETTY_NAME") / rust $(rustc --version 2>/dev/null | awk '{print $2}' || echo '?')"
