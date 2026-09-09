#!/usr/bin/env bash
# .deb 빌드 + 검증 게이트 + 배포판별 매니페스트 — CI 레그와 로컬 매트릭스 공용.
#
# 배포판별로 다시 빌드하는 이유: glibc 는 하위호환만 보장하고, Qt IM 플러그인은
# Qt GuiPrivate 에 링크해 Qt 마이너 간 ABI 보장이 없다. 한 빌드로 여러 배포판을
# 덮을 수 없어서 배포판마다 그 배포판의 라이브러리로 빌드한다.
#
# 사용법: scripts/ci/build-deb.sh <tag> [--smoke]
#   tag    : ubuntu24.04 | ubuntu26.04 | debian13  (자산·매니페스트 접미사)
#   --smoke: 빌드 후 이 컨테이너에 실설치(apt-get install ./debs/*.deb)까지 검증
#
# 동작:
#   1. 버전 게이트 — Cargo.toml ↔ debian/changelog 일치 (릴리스 태그 검증은
#      워크플로 몫; RELEASE_TAG 환경변수가 있으면 여기서도 대조)
#   2. debian/changelog 첫 항목 버전에 '~<tag>' 접미사 주입 (PPA 관용 —
#      dpkg 정렬상 '~' 는 무접미사보다 낮아, 훗날 저장소 배포로 전환해도
#      업그레이드 경로가 보존된다. 파일명 충돌 해소 + 'dpkg -l' 로 식별 가능)
#   3. make deb
#   4. 게이트 — 정확히 11개(9 amd64 + 2 all), 0바이트 금지, 파일명에 버전 포함
#   5. debs/SHA256SUMS-<tag> 생성 (파일명만 — install.sh 의 flat tmpdir 계약)
#
# ⚠️ debian/changelog 를 제자리 수정한다 — CI 체크아웃/로컬 매트릭스의 소스
#    사본에서 돌릴 것. 개발 트리에서 직접 돌리면 git 에 변경이 남는다.

set -euo pipefail

TAG="${1:?사용법: build-deb.sh <tag> [--smoke]}"
SMOKE="${2:-}"
SUFFIX="~${TAG}"

case "$TAG" in
    ubuntu*|debian*) ;;
    *) echo "❌ 알 수 없는 tag: $TAG (ubuntu24.04|ubuntu26.04|debian13)"; exit 1 ;;
esac

# ── 1. 버전 게이트 ──────────────────────────────────────────────────────────
CARGO_VER=$(grep -E '^version *= *"' Cargo.toml | head -1 | sed -E 's/.*"([0-9.]+)".*/\1/')
DEB_FULL=$(head -1 debian/changelog | sed -E 's/^unim \(([^)]+)\).*/\1/')
DEB_VER=${DEB_FULL%-*}
DEB_REV=${DEB_FULL##*-}

echo "Cargo.toml : $CARGO_VER"
echo "changelog  : $DEB_VER-$DEB_REV"
if [ "$CARGO_VER" != "$DEB_VER" ]; then
    echo "❌ Cargo.toml($CARGO_VER) ↔ debian/changelog($DEB_VER) 버전 불일치"
    exit 1
fi
if [ -n "${RELEASE_TAG:-}" ] && [ "${RELEASE_TAG#v}" != "$CARGO_VER" ]; then
    echo "❌ 태그($RELEASE_TAG) ↔ Cargo.toml($CARGO_VER) 불일치"
    exit 1
fi

# ── 2. 접미사 주입 (멱등 — 이미 붙어 있으면 통과) ──────────────────────────
if [[ "$DEB_FULL" == *"$SUFFIX" ]]; then
    echo "접미사 이미 주입됨: $DEB_FULL"
else
    sed -i "1s/^unim (${DEB_FULL})/unim (${DEB_FULL}${SUFFIX})/" debian/changelog
    head -1 debian/changelog
fi
FULL="${DEB_VER}-${DEB_REV}${SUFFIX}"

# ── 3. 빌드 ─────────────────────────────────────────────────────────────────
make deb

# ── 4. 게이트 ───────────────────────────────────────────────────────────────
shopt -s nullglob
debs=(debs/*.deb)
shopt -u nullglob
count=${#debs[@]}
echo "발견된 .deb: $count 개"
for f in "${debs[@]}"; do
    size=$(stat -c%s "$f")
    if [ "$size" -eq 0 ]; then echo "❌ 0바이트 패키지: $f"; exit 1; fi
    base=$(basename "$f")
    if [[ "$base" != *"${FULL}"* ]]; then
        echo "❌ 파일명에 버전(${FULL})이 없다: $base"; exit 1
    fi
    echo "  ok  $base ($size bytes)"
done
# 9 amd64 + 2 all = 11. dbgsym 누출 회귀도 이 카운트가 잡는다.
if [ "$count" -ne 11 ]; then
    echo "❌ .deb 개수가 11이 아니다 (실제 $count) — 구성 변경 또는 dbgsym 누출 의심"
    exit 1
fi

# ── 5. 매니페스트 ───────────────────────────────────────────────────────────
( cd debs && sha256sum *.deb > "SHA256SUMS-${TAG}" )
echo "── SHA256SUMS-${TAG} ──"
cat "debs/SHA256SUMS-${TAG}"

# ── 6. 스모크 (선택) ────────────────────────────────────────────────────────
if [ "$SMOKE" = "--smoke" ]; then
    SUDO=""; [ "$(id -u)" -ne 0 ] && SUDO="sudo"
    export DEBIAN_FRONTEND=noninteractive
    # './'+절대경로 glob → apt 가 로컬 파일로 인식, 외부 런타임 의존까지 해석.
    $SUDO apt-get install -y -qq "$PWD"/debs/*.deb >/dev/null
    dpkg -l 'unim*' | awk '/^ii/{print "   설치:", $2, $3}'
    installed=$(dpkg -l 'unim*' 2>/dev/null | grep -c '^ii' || true)
    if [ "$installed" -ne 11 ]; then
        echo "❌ 스모크 설치 후 unim 패키지가 11개가 아니다 (실제 $installed)"
        exit 1
    fi
    echo "✅ 스모크 설치 11/11"

    # 설치 후 런타임 로드 검증 (L1+L2) — GTK/Qt 가 모듈을 실제로 로드하는지,
    # 데몬이 기동해 D-Bus 에 응답하는지. 패키지 개수·경로만으론 못 잡는다.
    "$(dirname "${BASH_SOURCE[0]}")/verify-installed.sh" "$TAG"

    # 기능 타이핑(L3) — 설치된 IM 모듈·데몬으로 실제 GTK/Qt/XIM 경로를 XTEST 로
    # 통과시켜 tests/harness/ 의 회귀 시나리오를 검증한다. verify-installed.sh
    # 는 "로드되는가"만 보고, 이건 "실제로 한글이 조합·확정되는가"를 본다.
    # purge 전에 돈다 — 지운 뒤엔 검증할 모듈이 없다.
    "$(dirname "${BASH_SOURCE[0]}")/functional-test.sh" "$TAG"

    # ── purge 검증 — rpm 쪽 %preun 검증과 대칭. 신규 install 만으론 postrm/
    # 트리거 제거 경로가 안 지나가므로 실제로 지워 봐야 한다.
    $SUDO apt-get purge -y -qq 'unim*' >/dev/null
    remaining=$(dpkg -l 'unim*' 2>/dev/null | grep -c '^ii' || true)
    if [ "$remaining" -ne 0 ]; then
        echo "❌ purge 후 unim 패키지 잔존(설치 상태 'ii'):"; dpkg -l 'unim*'; exit 1
    fi
    for f in /usr/libexec/unim-daemon \
             /usr/lib/*/gtk-3.0/3.0.0/immodules/im-unim.so; do
        [ -e "$f" ] && { echo "❌ purge 후에도 남은 파일: $f"; exit 1; }
    done
    # 트리거 대칭 — 설치 때 캐시에 등록됐다면, 제거 때도 캐시에서 빠져야 한다.
    for cache in /usr/lib/*/gtk-3.0/3.0.0/immodules.cache \
                 /usr/lib64/gtk-3.0/3.0.0/immodules.cache; do
        if [ -f "$cache" ] && grep -q 'im-unim\.so' "$cache" 2>/dev/null; then
            echo "❌ purge 후에도 immodules.cache 에 unim 잔존: $cache"; exit 1
        fi
    done
    echo "✅ 스모크 purge — 패키지·대표 파일·immodules.cache 등록 모두 정리됨"
fi

echo "✅ build-deb ${TAG}: ${FULL} — 11개 .deb + SHA256SUMS-${TAG}"
