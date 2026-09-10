#!/usr/bin/env bash
#
# 릴리스 자산 이름 대조 — 매니페스트가 가리키는 파일이 실제로 릴리스에 있는가.
#
# 왜 필요한가 (2026-09-09 v0.4.2 실측):
#   deb 파일명에 배포판 접미사를 `~` 로 붙였는데, GitHub 은 릴리스 자산 이름의
#   `~` 를 `.` 로 바꿔 저장한다. 매니페스트(SHA256SUMS-*)에는 `~` 인 채 적혀
#   있어서, install.sh 가 매니페스트의 이름으로 내려받다가 404 를 맞았다.
#   빌드·업로드는 전부 그린이었다 — 아무도 "올라간 이름"을 본 적이 없었기 때문.
#   이 스크립트가 그 구멍을 막는다. 릴리스 생성 **직후** 돌린다.
#
# 사용법: scripts/ci/verify-release-assets.sh <태그> <매니페스트> [매니페스트...]
#   예:   scripts/ci/verify-release-assets.sh v0.4.3 debs/SHA256SUMS-ubuntu24.04 ...
#
# 필요: gh (GITHUB_TOKEN 인증), jq
# 종료: 0=전부 일치, 1=하나라도 불일치(에러 주석 출력)

set -euo pipefail

TAG="${1:?사용법: verify-release-assets.sh <태그> <매니페스트>...}"
shift
[ "$#" -gt 0 ] || { echo "❌ 매니페스트를 하나 이상 달라"; exit 1; }

echo "릴리스 자산 조회: ${TAG}"
assets=$(gh release view "$TAG" --json assets --jq '.assets[].name' | sort)
if [ -z "$assets" ]; then
    echo "::error::릴리스 ${TAG} 에 자산이 없다 — 업로드 실패 의심"
    exit 1
fi
echo "자산 $(printf '%s\n' "$assets" | wc -l) 개 확인"

missing=0
for manifest in "$@"; do
    if [ ! -f "$manifest" ]; then
        echo "::error::매니페스트 없음: $manifest"
        missing=$((missing + 1))
        continue
    fi
    # 매니페스트 자신도 릴리스에 올라가 있어야 한다.
    mname=$(basename "$manifest")
    if ! printf '%s\n' "$assets" | grep -Fxq "$mname"; then
        echo "::error::매니페스트가 릴리스에 없다: $mname"
        missing=$((missing + 1))
    fi
    # 각 줄의 파일명이 자산으로 존재하는가. (sha256sum 형식: "<해시>  <이름>")
    n=0
    while read -r _hash name; do
        [ -n "${name:-}" ] || continue
        n=$((n + 1))
        if ! printf '%s\n' "$assets" | grep -Fxq "$name"; then
            echo "::error::${mname} 이 가리키는 자산이 없다: ${name}"
            missing=$((missing + 1))
        fi
    done < "$manifest"
    echo "  ok  ${mname}: ${n}줄 대조"
done

if [ "$missing" -ne 0 ]; then
    echo "::error::릴리스 자산 대조 실패 — 불일치 ${missing}건. install.sh 가 404 를 맞는다."
    exit 1
fi

echo "✅ 매니페스트가 가리키는 자산이 릴리스에 전부 존재한다."
