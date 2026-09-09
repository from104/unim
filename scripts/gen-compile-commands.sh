#!/usr/bin/env bash
# clangd 용 컴파일 데이터베이스 생성기
#
# UNIM 의 C/C++ 부분은 프런트엔드마다 **독립된 CMake 프로젝트**로 쪼개져 있다
# (gtk3·gtk4·qt5·qt6 + 테스트 앱 6종). 그래서 어느 한 빌드 디렉터리의
# compile_commands.json 도 저장소 전체를 못 덮는다 — 그걸 하나로 합쳐
# 저장소 루트에 놓아야 clangd 가 모든 .c/.cpp 를 제대로 연다.
#
# 겸해서 .clangd 도 만든다. clangd 는 기본적으로 컴파일러(cc/c++)에게
# 시스템 헤더 경로를 묻지 않아서(--query-driver 미지정), GCC 의 libstdc++
# 경로를 모른 채 C++ 파일마다 "'type_traits' file not found" 를 쏟는다.
# 여기서 실제 컴파일러에게 물어 -isystem 으로 박아 준다. GCC 판이 올라가면
# 이 스크립트를 다시 돌리면 된다.
#
# 산출물 둘 다 .gitignore 대상이다 — 기계마다 경로가 다르다.
#
# 사용법: scripts/gen-compile-commands.sh [빌드디렉터리]
#         (기본 빌드디렉터리: build/ccdb, 구성만 하고 컴파일은 안 한다)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_ROOT="${1:-$ROOT/build/ccdb}"
cd "$ROOT"

command -v cmake >/dev/null || { echo "cmake 가 없다"; exit 1; }
command -v python3 >/dev/null || { echo "python3 가 없다"; exit 1; }

# CMakeLists.txt 를 가진 디렉터리를 직접 찾는다 — 프런트엔드가 추가돼도
# 목록을 손보지 않아도 되게.
mapfile -t PROJECTS < <(git ls-files '*CMakeLists.txt' | xargs -r -n1 dirname | sort -u)
[ "${#PROJECTS[@]}" -gt 0 ] || { echo "CMake 프로젝트를 못 찾았다"; exit 1; }

mkdir -p "$BUILD_ROOT"
ok=0 fail=0
for d in "${PROJECTS[@]}"; do
    n="${d//\//_}"
    if cmake -S "$d" -B "$BUILD_ROOT/$n" -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
             >"$BUILD_ROOT/$n.log" 2>&1; then
        printf '  OK   %s\n' "$d"; ok=$((ok+1))
    else
        # 의존성이 없는 프로젝트는 건너뛴다 (예: Qt5 미설치 기계).
        # 그 프런트엔드만 clangd 지원이 빠질 뿐 나머지는 정상 동작한다.
        printf '  SKIP %s  — %s\n' "$d" \
            "$(grep -m1 -iE 'error|could not find|not found' "$BUILD_ROOT/$n.log" | cut -c1-70)"
        fail=$((fail+1))
    fi
done

python3 - "$ROOT" "$BUILD_ROOT" <<'PY'
import json, glob, os, sys
root, build_root = os.path.realpath(sys.argv[1]), os.path.realpath(sys.argv[2])
seen, out = set(), []
for p in sorted(glob.glob(os.path.join(build_root, "*", "compile_commands.json"))):
    for e in json.load(open(p)):
        f = os.path.realpath(e["file"])
        # 저장소 밖(CMake 의 컴파일러 검사용 소스)과 빌드 디렉터리 안의
        # 생성 소스(Qt moc 등)는 버린다 — 사람이 여는 파일이 아니다.
        if f in seen or not f.startswith(root + os.sep) or f.startswith(build_root + os.sep):
            continue
        seen.add(f); out.append(e)
if not out:
    sys.exit("합칠 항목이 없다 — CMake 구성이 전부 실패했다")
with open(os.path.join(root, "compile_commands.json"), "w") as fh:
    json.dump(out, fh, indent=1)
print(f"compile_commands.json — {len(out)} 항목")
PY

# --- .clangd : GCC 시스템 헤더 경로를 clangd 에 알려 준다 -------------------
cxx_includes=$("${CXX:-g++}" -E -x c++ - -v </dev/null 2>&1 |
    sed -n '/#include <\.\.\.> search starts here:/,/End of search list/p' |
    sed '1d;$d;s/^ //')

{
    echo "# scripts/gen-compile-commands.sh 가 생성함 — 직접 고치지 말 것."
    echo "# clangd 는 --query-driver 없이는 GCC 의 libstdc++ 경로를 모른다."
    echo "If:"
    echo "  PathMatch: [.*\\.cpp, .*\\.cc, .*\\.hpp, .*\\.hh]"
    echo "CompileFlags:"
    echo "  Add:"
    while IFS= read -r inc; do
        [ -d "$inc" ] || continue
        echo "    - -isystem"
        echo "    - $inc"
    done <<<"$cxx_includes"
} > "$ROOT/.clangd"

echo ".clangd — C++ 시스템 헤더 경로 $(grep -c -- '-isystem' "$ROOT/.clangd")개"
echo "CMake 구성: 성공 $ok · 건너뜀 $fail"
echo
echo "클로드 코드가 이미 떠 있으면 clangd 가 옛 데이터베이스를 붙들고 있다 — 세션을 다시 시작할 것."
