#!/usr/bin/env bash
# .rpm 빌드 + 검증 게이트 + 배포판별 매니페스트 — CI 레그와 로컬 매트릭스 공용.
# (배포판별 리빌드의 이유는 build-deb.sh 머리말과 같다 — glibc 단방향 호환 +
#  Qt GuiPrivate ABI. rpm 은 %{?dist} 가 파일명 구분을 공짜로 해 준다.)
#
# 사용법: scripts/ci/build-rpm.sh <tag> [--smoke]
#   tag    : fedora43 | fedora44 | el10  (매니페스트 접미사)
#   --smoke: dnf 실설치 + %preun 실행(제거) 검증까지
#
# 게이트는 linux-rpm.yml 의 것을 그대로 옮겼다: 정확히 11개(10 x86_64 +
# 1 noarch), %{_libdir} 경로, GNOME 확장 .mo, (스모크 시) Recommends 실재.

set -euo pipefail

TAG="${1:?사용법: build-rpm.sh <tag> [--smoke]}"
SMOKE="${2:-}"

case "$TAG" in
    fedora*|el*) ;;
    *) echo "❌ 알 수 없는 tag: $TAG (fedora43|fedora44|el10)"; exit 1 ;;
esac

git config --global --add safe.directory "$PWD" 2>/dev/null || true

# ── 1. 버전 게이트 ──────────────────────────────────────────────────────────
CARGO_VER=$(grep -E '^version *= *"' Cargo.toml | head -1 | sed -E 's/.*"([0-9.]+)".*/\1/')
SPEC_VER=$(grep -E '^Version:' rpm/unim.spec | head -1 | awk '{print $2}')
SPEC_REL=$(grep -E '^Release:' rpm/unim.spec | head -1 | awk '{print $2}' | sed 's/%{?dist}//')

echo "Cargo.toml : $CARGO_VER"
echo "unim.spec  : $SPEC_VER-$SPEC_REL(%{?dist})"
if [ "$CARGO_VER" != "$SPEC_VER" ]; then
    echo "❌ Cargo.toml($CARGO_VER) ↔ rpm/unim.spec($SPEC_VER) 버전 불일치"
    exit 1
fi
if [ -n "${RELEASE_TAG:-}" ] && [ "${RELEASE_TAG#v}" != "$CARGO_VER" ]; then
    echo "❌ 태그($RELEASE_TAG) ↔ Cargo.toml($CARGO_VER) 불일치"
    exit 1
fi

# ── 2. 빌드 ─────────────────────────────────────────────────────────────────
TOPDIR="$PWD/rpm/build"
mkdir -p "$TOPDIR"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
git archive --format=tar.gz --prefix="unim-${CARGO_VER}/" HEAD \
    -o "$TOPDIR/SOURCES/unim-${CARGO_VER}.tar.gz"
cp rpm/unim.spec "$TOPDIR/SPECS/unim.spec"
# -bb: 바이너리만 (SRPM 은 릴리스에 게시하지 않는다).
rpmbuild --define "_topdir $TOPDIR" -bb "$TOPDIR/SPECS/unim.spec"
mkdir -p rpms
find "$TOPDIR/RPMS" -name '*.rpm' -exec cp -f {} rpms/ \;

# ── 3. 게이트 ───────────────────────────────────────────────────────────────
shopt -s nullglob
rpms=(rpms/*.rpm)
shopt -u nullglob
count=${#rpms[@]}
echo "발견된 .rpm: $count 개"
for f in "${rpms[@]}"; do
    size=$(stat -c%s "$f")
    if [ "$size" -eq 0 ]; then echo "❌ 0바이트 패키지: $f"; exit 1; fi
    base=$(basename "$f")
    if [[ "$base" != *"${CARGO_VER}-${SPEC_REL}"* ]]; then
        echo "❌ 파일명에 버전(${CARGO_VER}-${SPEC_REL})이 없다: $base"; exit 1
    fi
    echo "  ok  $base ($size bytes)"
done
if [ "$count" -ne 11 ]; then
    echo "❌ .rpm 개수가 11이 아니다 (실제 $count) — 구성 변경 또는 debuginfo 누출 의심"
    exit 1
fi

# %{_libdir} 게이트 — REAL_LIBDIR 오판(멀티아치 경로 오염) 방어.
rpm -qlp rpms/unim-im-gtk-*.rpm | grep -qx '/usr/lib64/gtk-3.0/3.0.0/immodules/im-unim.so'
rpm -qlp rpms/unim-im-gtk-*.rpm | grep -qx '/usr/lib64/gtk-4.0/4.0.0/immodules/libim-unim.so'
rpm -qlp rpms/unim-im-qt-*.rpm  | grep -qx '/usr/lib64/qt5/plugins/platforminputcontexts/libunim.so'
rpm -qlp rpms/unim-im-qt-*.rpm  | grep -qx '/usr/lib64/qt6/plugins/platforminputcontexts/libunim.so'
echo "  ok  %{_libdir}=/usr/lib64 immodule/plugin 경로"

# GNOME 확장 번역(.mo) 게이트 — msgfmt 무음 스킵 방어.
for l in ko en; do
    if ! rpm -qlp rpms/unim-gnome-*.rpm | grep -q "locale/${l}/LC_MESSAGES/unim-gnome@from104.github.io.mo"; then
        echo "❌ unim-gnome rpm 에 ${l} 번역(.mo)이 없다 — gettext/msgfmt 확인"
        exit 1
    fi
done
echo "  ok  GNOME 확장 번역 ko/en .mo 포함"

# ── 4. 매니페스트 ───────────────────────────────────────────────────────────
( cd rpms && sha256sum *.rpm > "SHA256SUMS-${TAG}" )
echo "── SHA256SUMS-${TAG} ──"
cat "rpms/SHA256SUMS-${TAG}"

# ── 5. 스모크 (선택) ────────────────────────────────────────────────────────
if [ "$SMOKE" = "--smoke" ]; then
    dnf -y install ./rpms/*.rpm
    rpm -q unim unim-common unim-desktop unim-settings unim-gnome
    if ! rpm -q --filetriggers gtk3 | grep -qi immodules; then
        echo "⚠️  gtk3 immodules 파일트리거 미확인 — unim-im-gtk %post/%postun 검토 신호"
    fi
    # Recommends(약한 의존)는 설치 실패를 안 내므로 이름 실재를 별도 게이트.
    for p in google-noto-sans-cjk-fonts pulseaudio-utils alsa-utils fontconfig gnome-shell; do
        if ! dnf repoquery --whatprovides "$p" | grep -q .; then
            echo "❌ 의존 대상 '$p' 를 저장소에서 찾지 못했다"
            exit 1
        fi
    done

    # 설치 후 런타임 로드 검증 (L1+L2) — GTK/Qt 가 모듈을 실제로 로드하는지,
    # 데몬이 기동해 D-Bus 에 응답하는지. rpm -q 만으론 못 잡는다. dnf remove
    # 전에 돈다 — 그 다음이 %preun 검증이라 모듈이 아직 설치돼 있어야 한다.
    "$(dirname "${BASH_SOURCE[0]}")/verify-installed.sh" "$TAG"

    # 기능 타이핑(L3) — 설치된 IM 모듈·데몬으로 실제 GTK/Qt/XIM 경로를 XTEST 로
    # 통과시켜 tests/harness/ 의 회귀 시나리오를 검증한다(build-deb.sh 와 동일
    # 사유). dnf remove(%preun 검증) 전에 돈다.
    "$(dirname "${BASH_SOURCE[0]}")/functional-test.sh" "$TAG"

    # erase 트랜잭션으로 %preun 스크립틀릿을 실제로 실행시킨다 —
    # 신규 install 은 %preun 을 돌리지 않아 설치 스모크만으론 미검증.
    dnf -y remove 'unim*'
    if rpm -qa 'unim*' | grep -q .; then
        echo "❌ 제거 후 unim 패키지 잔존:"; rpm -qa 'unim*'; exit 1
    fi
    # 트리거 대칭 — 설치 때 캐시에 등록됐다면, 제거 때도 캐시에서 빠져야 한다.
    for cache in /usr/lib64/gtk-3.0/3.0.0/immodules.cache \
                 /usr/lib/*/gtk-3.0/3.0.0/immodules.cache; do
        if [ -f "$cache" ] && grep -q 'im-unim\.so' "$cache" 2>/dev/null; then
            echo "❌ 제거 후에도 immodules.cache 에 unim 잔존: $cache"; exit 1
        fi
    done
    echo "✅ 스모크 설치·제거(%preun)·immodules.cache 정리 통과"
fi

rpmlint rpms/*.rpm || true
echo "✅ build-rpm ${TAG}: ${CARGO_VER}-${SPEC_REL} — 11개 .rpm + SHA256SUMS-${TAG}"
