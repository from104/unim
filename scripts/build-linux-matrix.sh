#!/usr/bin/env bash
# 로컬 배포판 매트릭스 빌드 — CI 와 같은 scripts/ci/* 를 컨테이너에서 돌린다.
#
# 개발자가 릴리스 전에 6개 배포판 빌드를 손안에서 재현하는 도구다. 워크플로
# YAML 은 얇은 호출자일 뿐 실제 로직은 scripts/ci/ 에 있으므로, 여기서 통과한
# 것은 CI 에서도 통과한다 (러너 사양 차이는 예외).
#
# 사용법: scripts/build-linux-matrix.sh [--only <tag>[,<tag>…]] [--smoke]
#   --only  : 일부 레그만 (예: --only el10,debian13)
#   --smoke : 각 컨테이너에서 실설치 검증까지 (기본은 빌드+게이트만)
#
# 소스는 git ls-files 기준으로 컨테이너 안 /build 에 복사된다 — 호스트
# 작업 트리는 건드리지 않고(target/·debs/ 오염 없음), 미커밋 변경도 포함된다.
# 산출물은 dist/linux/<tag>/ 로 나온다. cargo 레지스트리 캐시는 명명 볼륨
# unim-cargo-registry 로 레그 간 공유(아키텍처 무관), target 은 공유하지
# 않는다(배포판별 링크 대상이 달라 오염 위험).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# tag → 이미지·채널. 새 배포판 지원은 이 표와 워크플로 매트릭스에 함께 추가.
declare -A IMAGE=(
    [ubuntu24.04]=ubuntu:24.04
    [ubuntu26.04]=ubuntu:26.04
    [debian13]=debian:13
    [fedora43]=fedora:43
    [fedora44]=fedora:44
    [el10]=almalinux:10
)
declare -A KIND=(
    [ubuntu24.04]=deb [ubuntu26.04]=deb [debian13]=deb
    [fedora43]=rpm [fedora44]=rpm [el10]=rpm
)
ORDER=(ubuntu24.04 ubuntu26.04 debian13 fedora43 fedora44 el10)

ONLY="" SMOKE=""
while [ $# -gt 0 ]; do
    case "$1" in
        --only)  ONLY="$2"; shift 2 ;;
        --smoke) SMOKE="--smoke"; shift ;;
        *) echo "알 수 없는 인자: $1"; exit 1 ;;
    esac
done

RUNTIME=""
for c in podman docker; do
    command -v "$c" >/dev/null && { RUNTIME="$c"; break; }
done
[ -n "$RUNTIME" ] || { echo "❌ podman/docker 가 없다"; exit 1; }

# 소스 스냅샷: 추적 파일 + 미추적-비무시 파일 (미커밋 변경 포함, target/ 등 제외).
# build-rpm.sh 의 git archive 가 HEAD 를 요구하므로 .git 도 같이 담는다.
SNAP="$(mktemp -d)"
trap 'rm -rf "$SNAP"' EXIT
git ls-files -co --exclude-standard -z | tar --null -T - -cf "$SNAP/src.tar"
tar -rf "$SNAP/src.tar" .git

pass=() fail=()
for tag in "${ORDER[@]}"; do
    if [ -n "$ONLY" ] && [[ ",$ONLY," != *",$tag,"* ]]; then continue; fi
    kind="${KIND[$tag]}"
    img="${IMAGE[$tag]}"
    out="$ROOT/dist/linux/$tag"
    mkdir -p "$out"
    echo ""
    echo "━━━ $tag ($img, $kind) ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if "$RUNTIME" run --rm \
        -v "$SNAP/src.tar:/src.tar:ro" \
        -v "$out:/out" \
        -v unim-cargo-registry:/root/.cargo/registry \
        -w /build "$img" bash -c "
            set -euo pipefail
            mkdir -p /build && tar -xf /src.tar -C /build
            if [ '$kind' = rpm ]; then scripts/ci/bootstrap-rpm.sh $tag
            else scripts/ci/bootstrap-deb.sh; fi
            scripts/ci/build-${kind}.sh $tag $SMOKE
            cp -f ${kind}s/*.${kind} ${kind}s/SHA256SUMS-$tag /out/
        "; then
        pass+=("$tag")
    else
        fail+=("$tag")
    fi
done

echo ""
echo "━━━ 결과 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
[ "${#pass[@]}" -gt 0 ] && echo "✅ 통과: ${pass[*]}"
[ "${#fail[@]}" -gt 0 ] && { echo "❌ 실패: ${fail[*]}"; exit 1; }
echo "산출물: dist/linux/<tag>/"
