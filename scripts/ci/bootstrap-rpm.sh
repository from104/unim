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

# spec 의 BuildRequires 를 그대로 해석 — 이름이 그 배포판에 실재하는지의
# 검증을 겸한다 (해석 실패 = 즉시 이 스텝에서 죽는다).
dnf builddep -y rpm/unim.spec

echo "✅ bootstrap-rpm: $(. /etc/os-release && echo "$PRETTY_NAME") / rust $(rustc --version 2>/dev/null | awk '{print $2}' || echo '?')"
