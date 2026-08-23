#!/usr/bin/env bash
# check-gnome-api.js 실행 껍데기 — GNOME introspection 경로를 찾아 넘긴다.
#
# Meta·Clutter 는 mutter 의 사설 typelib 에, St·Shell 은 gnome-shell 쪽에 있다.
# 둘 다 **디렉터리 이름에 ABI 버전이 박혀 있어**(mutter-18, mutter-19 …) 셸이
# 올라갈 때마다 바뀐다. 그래서 하드코딩하지 않고 매번 찾는다 — 경로를 박아
# 두는 순간 이 검사기 자체가 하위호환을 깨는 물건이 된다.
#
# 사용법: scripts/check-gnome-api.sh [확장디렉터리]
# 종료 코드는 check-gnome-api.js 의 것을 그대로 넘긴다.
#   0 = 이상 없음 · 1 = 없거나 금지된 심볼 · 2 = 검사 불가(환경 부족)

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXT_DIR="${1:-$ROOT/unim-gnome-extension}"

if ! command -v gjs >/dev/null; then
    echo "⏭️  gjs 가 없어 GNOME API 대조를 건너뛴다 (gjs 패키지 필요)."
    exit 0
fi

# mutter-* 와 gnome-shell 디렉터리를 라이브러리 경로 후보에서 찾는다.
# 여러 판이 깔려 있으면 이름순 마지막(= 가장 높은 ABI)을 쓴다.
libdirs=()
for base in /usr/lib/*/mutter-* /usr/lib/mutter-* /usr/lib64/mutter-* \
            /usr/lib/*/gnome-shell /usr/lib/gnome-shell /usr/lib64/gnome-shell; do
    [ -d "$base" ] && libdirs+=("$base")
done

if [ "${#libdirs[@]}" -eq 0 ]; then
    echo "⏭️  mutter·gnome-shell typelib 을 못 찾아 대조를 건너뛴다."
    echo "    (GNOME 이 없는 기계에서는 정상이다 — 이 검사는 셸이 있는 곳에서만 뜻이 있다)"
    exit 0
fi

# 중복 제거 후 두 경로 변수에 같이 넣는다. typelib 은 GI_TYPELIB_PATH 로,
# 그 typelib 이 참조하는 libmutter-clutter-*.so 는 LD_LIBRARY_PATH 로 찾는다 —
# 후자를 빠뜨리면 typelib 은 열리는데 심볼 해석이 통째로 실패한다.
joined=$(printf '%s\n' "${libdirs[@]}" | sort -u | paste -sd:)

GI_TYPELIB_PATH="$joined${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}" \
LD_LIBRARY_PATH="$joined${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    gjs "$ROOT/scripts/check-gnome-api.js" "$EXT_DIR"
