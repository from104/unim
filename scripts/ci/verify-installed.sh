#!/usr/bin/env bash
# 설치 후 런타임 로드 검증 (L1: 모듈 로드 가능성 + L2: 프로세스 기동/D-Bus 응답)
#
# build-deb.sh/build-rpm.sh 의 --smoke 는 패키지 개수·경로·(rpm 은) 제거까지만
# 본다 — 설치된 IM 모듈을 GTK/Qt 가 실제로 로드하는지, 데몬이 기동해 D-Bus 에
# 응답하는지는 어디서도 확인하지 않는다. 이 스크립트가 그 다음 층이다.
#
# 배포판 중립: deb(멀티아치 트리플렛, /usr/lib/<triplet>/…)와 rpm(/usr/lib64/…)
# 의 경로 차이는 후보 glob 을 순서대로 탐색해 흡수한다.
#
# 전제: unim 패키지가 이미 설치된 컨테이너 안에서 실행.
#
# 사용법: scripts/ci/verify-installed.sh <tag>
#   tag: 로그 라벨용 (ubuntu24.04|ubuntu26.04|debian13|fedora43|fedora44|el10)
#
# ⚠️ `set -e` 를 쓰지 않는다 — 검사 하나의 실패가 나머지 검사를 건너뛰게
#    하면 안 된다. 대신 항목별로 결과를 모아 표로 낸 뒤 종합 판정한다.
set -uo pipefail
shopt -s nullglob

TAG="${1:?사용법: verify-installed.sh <tag>}"

WORKDIR="$(mktemp -d /tmp/unim-verify.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

echo "═══ verify-installed ${TAG} ═══"

# ── 결과 집계 ────────────────────────────────────────────────────────────
declare -a RESULT_IDS=() RESULT_DESCS=() RESULT_STATUS=() RESULT_DETAILS=()

record() { # id desc status(PASS|FAIL|SKIP) detail
    RESULT_IDS+=("$1"); RESULT_DESCS+=("$2"); RESULT_STATUS+=("$3"); RESULT_DETAILS+=("$4")
    case "$3" in
        PASS) echo "✅ [$1] $2 — $4" ;;
        FAIL) echo "❌ [$1] $2 — $4" ;;
        SKIP) echo "⏭  [$1] $2 — $4" ;;
    esac
}

# 후보 glob 패턴을 순서대로 훑어 첫 매치를 표준출력에 낸다. 없으면 빈 문자열.
find_first() {
    local pat f
    for pat in "$@"; do
        for f in $pat; do
            [ -e "$f" ] && { echo "$f"; return 0; }
        done
    done
    return 1
}

# 공유 라이브러리를 RTLD_NOW 로 dlopen 하고 지정 심볼이 있는지 확인.
# RTLD_NOW 여야 사설 ABI(예: QtGuiPrivate) 불일치가 여기서 즉시 터진다 —
# 지연 바인딩(RTLD_LAZY)이면 심볼을 실제로 쓸 때까지 조용히 넘어간다.
# 반환: 0=성공 1=실패(dlopen/dlsym) 2=SKIP(python3·cc 둘 다 없음)
dlopen_check() { # path symbol
    local path="$1" sym="$2"
    if command -v python3 >/dev/null 2>&1; then
        python3 - "$path" "$sym" <<'PY'
import ctypes, os, sys
path, sym = sys.argv[1], sys.argv[2]
try:
    h = ctypes.CDLL(path, mode=os.RTLD_NOW)
except OSError as e:
    print(f"dlopen 실패: {e}")
    sys.exit(1)
try:
    getattr(h, sym)
except AttributeError as e:
    print(f"dlsym 실패({sym}): {e}")
    sys.exit(1)
print(f"dlopen+dlsym({sym}) OK")
PY
        return $?
    fi
    if command -v cc >/dev/null 2>&1; then
        local src="$WORKDIR/dlchk.c" bin="$WORKDIR/dlchk"
        if [ ! -x "$bin" ]; then
            cat >"$src" <<'CEOF'
#include <dlfcn.h>
#include <stdio.h>
int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: dlchk <path> <symbol>\n"); return 1; }
    void *h = dlopen(argv[1], RTLD_NOW);
    if (!h) { fprintf(stderr, "dlopen 실패: %s\n", dlerror()); return 1; }
    dlerror();
    void *sym = dlsym(h, argv[2]);
    char *err = dlerror();
    if (err) { fprintf(stderr, "dlsym 실패(%s): %s\n", argv[2], err); return 1; }
    printf("dlopen+dlsym(%s) OK\n", argv[2]);
    return 0;
}
CEOF
            cc -o "$bin" "$src" -ldl 2>&1 || { echo "cc 컴파일 실패"; return 1; }
        fi
        "$bin" "$path" "$sym"
        return $?
    fi
    echo "python3/cc 모두 없음"
    return 2
}

# ── a. GTK3 immodule ────────────────────────────────────────────────────
echo "── a. GTK3 immodule ──"

GTK3_CACHE=$(find_first \
    '/usr/lib/*/gtk-3.0/3.0.0/immodules.cache' \
    '/usr/lib64/gtk-3.0/3.0.0/immodules.cache')
if [ -n "$GTK3_CACHE" ] && grep -q 'im-unim\.so' "$GTK3_CACHE" 2>/dev/null; then
    record a1 "GTK3 immodules.cache 트리거 등록(postinst/filetrigger 동작 증명)" PASS "$GTK3_CACHE"
elif [ -n "$GTK3_CACHE" ]; then
    record a1 "GTK3 immodules.cache 트리거 등록(postinst/filetrigger 동작 증명)" FAIL "$GTK3_CACHE 에 unim 미등록"
else
    record a1 "GTK3 immodules.cache 트리거 등록(postinst/filetrigger 동작 증명)" FAIL "캐시 파일을 찾지 못함"
fi

GTK3_QUERY=$(find_first \
    '/usr/lib/*/libgtk-3-0*/gtk-query-immodules-3.0' \
    '/usr/bin/gtk-query-immodules-3.0-64' \
    '/usr/bin/gtk-query-immodules-3.0' \
    '/usr/libexec/gtk-3.0/gtk-query-immodules-3.0' \
    '/usr/lib64/gtk-3.0/gtk-query-immodules-3.0')
if [ -n "$GTK3_QUERY" ]; then
    if "$GTK3_QUERY" 2>/dev/null | grep -q 'im-unim\.so'; then
        record a2 "GTK3 모듈 자체 로드 가능성(gtk-query-immodules-3.0 직접 실행)" PASS "$GTK3_QUERY"
    else
        record a2 "GTK3 모듈 자체 로드 가능성(gtk-query-immodules-3.0 직접 실행)" FAIL "$GTK3_QUERY 출력에 im-unim.so 없음"
    fi
else
    record a2 "GTK3 모듈 자체 로드 가능성(gtk-query-immodules-3.0 직접 실행)" SKIP "쿼리 실행파일을 찾지 못함"
fi

# ── b. GTK4 (gio extension point — immodules 캐시가 없다) ──────────────
echo "── b. GTK4 gio module ──"

GTK4_MODULE=$(find_first \
    '/usr/lib/*/gtk-4.0/4.0.0/immodules/libim-unim.so' \
    '/usr/lib64/gtk-4.0/4.0.0/immodules/libim-unim.so')
if [ -z "$GTK4_MODULE" ]; then
    record b "GTK4 libim-unim.so dlopen+g_io_module_load" FAIL "모듈 파일을 찾지 못함"
else
    out=$(dlopen_check "$GTK4_MODULE" g_io_module_load 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        record b "GTK4 libim-unim.so dlopen+g_io_module_load" PASS "$GTK4_MODULE — $out"
    elif [ "$rc" -eq 2 ]; then
        record b "GTK4 libim-unim.so dlopen+g_io_module_load" SKIP "$out"
    else
        record b "GTK4 libim-unim.so dlopen+g_io_module_load" FAIL "$GTK4_MODULE — $out"
    fi
fi

# ── c. Qt5/Qt6 platform input context plugin ────────────────────────────
echo "── c. Qt5/Qt6 platforminputcontexts ──"

check_qt_plugin() { # id label path
    local id="$1" label="$2" path="$3"
    if [ -z "$path" ]; then
        record "$id" "$label" FAIL "플러그인 파일을 찾지 못함"
        return
    fi
    local unresolved
    unresolved=$(ldd "$path" 2>&1 | grep -c 'not found' || true)
    if [ "$unresolved" -gt 0 ]; then
        record "$id" "$label" FAIL "$path — ldd 미해결 심볼 ${unresolved}건: $(ldd "$path" 2>&1 | grep 'not found' | tr '\n' ';')"
        return
    fi
    local out rc
    out=$(dlopen_check "$path" qt_plugin_instance 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        record "$id" "$label" PASS "$path — ldd 클린 + $out"
    elif [ "$rc" -eq 2 ]; then
        record "$id" "$label" SKIP "$out"
    else
        record "$id" "$label" FAIL "$path — $out"
    fi
}

QT5_PLUGIN=$(find_first \
    '/usr/lib/*/qt5/plugins/platforminputcontexts/libunim.so' \
    '/usr/lib64/qt5/plugins/platforminputcontexts/libunim.so')
check_qt_plugin c1 "Qt5 platforminputcontexts ldd 미해결0 + RTLD_NOW dlopen + qt_plugin_instance" "$QT5_PLUGIN"

QT6_PLUGIN=$(find_first \
    '/usr/lib/*/qt6/plugins/platforminputcontexts/libunim.so' \
    '/usr/lib64/qt6/plugins/platforminputcontexts/libunim.so')
check_qt_plugin c2 "Qt6 platforminputcontexts ldd 미해결0 + RTLD_NOW dlopen + qt_plugin_instance" "$QT6_PLUGIN"

# ── d. 데몬: 직접 기동 + auto-activation, 둘 다 D-Bus 로 확인 ───────────
echo "── d. unim-daemon D-Bus ──"

DAEMON_BIN=/usr/libexec/unim-daemon
BUS_NAME=org.atit.unim.InputMethod
OBJ_PATH=/org/atit/unim/InputMethod
GDBUS_ARGS=(call --session --timeout 5 -d "$BUS_NAME" -o "$OBJ_PATH" -m "${BUS_NAME}.GetGlobalMode")

HAVE_DBUS_TOOLS=1
for tool in dbus-run-session gdbus timeout; do
    command -v "$tool" >/dev/null 2>&1 || HAVE_DBUS_TOOLS=0
done

setup_daemon_env() {
    local home="$WORKDIR/daemon-home"
    rm -rf "$home"
    mkdir -p "$home"/{home,config,data,cache,runtime}
    chmod 700 "$home/runtime"
    export HOME="$home/home"
    export XDG_RUNTIME_DIR="$home/runtime"
    export XDG_CONFIG_HOME="$home/config"
    export XDG_DATA_HOME="$home/data"
    export XDG_CACHE_HOME="$home/cache"
    # XDG_DATA_DIRS 는 건드리지 않는다 — 기본값(/usr/local/share:/usr/share)이
    # 이미 /usr/share/dbus-1/services 를 포함해야 auto-activation 이 성립한다.
    # env_logger 기본 필터(info)는 zbus 내부 로그까지 쏟아내 로그가 수백 KB로
    # 불어난다 — 진단에 필요한 신호만 남기도록 warn 으로 낮춘다.
    export RUST_LOG=warn
}

if [ ! -x "$DAEMON_BIN" ]; then
    record d1 "데몬 직접 기동 + GetGlobalMode 응답" FAIL "$DAEMON_BIN 실행 불가/부재"
    record d2 "데몬 auto-activation(서비스 파일, 미기동 상태에서 gdbus 콜만)" FAIL "$DAEMON_BIN 실행 불가/부재"
elif [ "$HAVE_DBUS_TOOLS" -eq 0 ]; then
    record d1 "데몬 직접 기동 + GetGlobalMode 응답" SKIP "dbus-run-session/gdbus/timeout 중 부재"
    record d2 "데몬 auto-activation(서비스 파일, 미기동 상태에서 gdbus 콜만)" SKIP "dbus-run-session/gdbus/timeout 중 부재"
else
    # d1: 직접 기동한 데몬이 응답하는가.
    #
    # 이름 등록 전에 실제 메소드 콜(gdbus call)을 반복 발사하면 안 된다 —
    # 소유자 없는 well-known name 에 대한 gdbus call 은 세션 버스의
    # auto-activation 을 그대로 트리거한다. 우리가 이미 백그라운드로
    # 기동해 둔 데몬과, 서비스 파일(org.atit.unim.InputMethod.service)이
    # 자동활성으로 스폰하는 "또 하나의" unim-daemon 이 --replace 로 같은
    # 이름을 놓고 경합하게 되고, 실측(fedora44/el10, 2026-09)에서 이 경합이
    # 데몬을 응답 불능으로 만드는 것으로 확인됐다(같은 레그의 [d2] — 데몬을
    # 직접 안 띄우고 gdbus 콜만으로 activation 시키는 경로 — 는 경합이 없어
    # PASS). 그래서 이름이 뜨는지는 `gdbus wait`(NameOwnerChanged 구독만
    # 하는 수동 대기, 메소드 콜이 아니므로 activation 을 유발하지 않는다)로
    # 확인한 뒤에야 실제 콜을 "정확히 한 번" 던진다.
    setup_daemon_env
    cat >"$WORKDIR/daemon-direct.sh" <<EOF
#!/usr/bin/env bash
set -uo pipefail
"$DAEMON_BIN" -n --replace >"$WORKDIR/daemon-direct.log" 2>&1 &
DAEMON_PID=\$!
rc=1
if gdbus wait --session --timeout 15 "$BUS_NAME"; then
    if gdbus ${GDBUS_ARGS[@]@Q} >"$WORKDIR/gdbus-direct.out" 2>&1; then rc=0; fi
fi
kill "\$DAEMON_PID" 2>/dev/null || true
wait "\$DAEMON_PID" 2>/dev/null || true
exit "\$rc"
EOF
    chmod +x "$WORKDIR/daemon-direct.sh"
    # 바깥 timeout 은 안쪽(wait 15초 + call 5초)보다 넉넉히 잡는다 — 데몬 기동
    # 지연·kill/wait 정리 시간까지 안쪽이 다 쓰면 바깥이 먼저 끊어 rc=124
    # (timeout 자체의 종료코드)로 오판될 수 있다.
    timeout 25 dbus-run-session -- "$WORKDIR/daemon-direct.sh"
    rc=$?
    resp=$(cat "$WORKDIR/gdbus-direct.out" 2>/dev/null || true)
    pkill -u "$(id -un)" -x unim-daemon 2>/dev/null || true
    if [ "$rc" -eq 0 ]; then
        record d1 "데몬 직접 기동 + GetGlobalMode 응답" PASS "$resp"
    else
        record d1 "데몬 직접 기동 + GetGlobalMode 응답" FAIL "rc=$rc resp=$resp log:$(tail -c 300 "$WORKDIR/daemon-direct.log" 2>/dev/null | tr '\n' ' ')"
    fi

    # d2: 데몬을 직접 띄우지 않고 gdbus 콜만으로 — 세션 버스가
    # /usr/share/dbus-1/services 의 서비스 파일을 읽고 activation 하는지.
    setup_daemon_env
    cat >"$WORKDIR/daemon-auto.sh" <<EOF
#!/usr/bin/env bash
set -uo pipefail
exec gdbus ${GDBUS_ARGS[@]@Q}
EOF
    chmod +x "$WORKDIR/daemon-auto.sh"
    timeout 20 dbus-run-session -- "$WORKDIR/daemon-auto.sh" >"$WORKDIR/gdbus-auto.out" 2>&1
    rc=$?
    resp=$(cat "$WORKDIR/gdbus-auto.out" 2>/dev/null || true)
    pkill -u "$(id -un)" -x unim-daemon 2>/dev/null || true
    if [ "$rc" -eq 0 ]; then
        record d2 "데몬 auto-activation(서비스 파일, 미기동 상태에서 gdbus 콜만)" PASS "$resp"
    else
        record d2 "데몬 auto-activation(서비스 파일, 미기동 상태에서 gdbus 콜만)" FAIL "rc=$rc out:$(echo "$resp" | tr '\n' ' ')"
    fi
fi

# ── e. 바이너리 기동 ─────────────────────────────────────────────────────
echo "── e. 바이너리 기동 ──"

if command -v unim-cli >/dev/null 2>&1; then
    if out=$(unim-cli --version 2>&1); then
        record e1 "unim-cli --version" PASS "$out"
    else
        rc=$?
        record e1 "unim-cli --version" FAIL "종료코드 $rc: $out"
    fi
else
    record e1 "unim-cli --version" FAIL "unim-cli 를 PATH 에서 찾지 못함"
fi

# unim-xim/unim-wayland 는 clap 등 인자 파서가 없다(소스 확인, 2026-09) —
# --help/--version 을 별도로 해석하지 않고 그대로 프론트엔드 기동(X11/Wayland
# 연결 시도)으로 들어가므로, 여기서 실행하면 하네스 밖 부작용/행 위험이 있다.
# 지원이 없으므로 SKIP.
record e2 "unim-xim --help/--version" SKIP "인자 파서 없음 — 실행하면 바로 X11 연결을 시도해 지원 확인 없이는 실행하지 않음"
record e3 "unim-wayland --help/--version" SKIP "인자 파서 없음 — 실행하면 바로 Wayland 연결을 시도해 지원 확인 없이는 실행하지 않음"

# ── 요약 ─────────────────────────────────────────────────────────────────
echo
echo "═══ 요약 (tag=${TAG}) ═══"
printf '%-4s  %-6s  %s\n' "ID" "결과" "검사"
fail_count=0
for i in "${!RESULT_IDS[@]}"; do
    printf '%-4s  %-6s  %s\n' "${RESULT_IDS[$i]}" "${RESULT_STATUS[$i]}" "${RESULT_DESCS[$i]}"
    [ "${RESULT_STATUS[$i]}" = FAIL ] && fail_count=$((fail_count + 1))
done

if [ "$fail_count" -gt 0 ]; then
    echo "❌ verify-installed ${TAG}: ${fail_count}건 실패"
    exit 1
fi
echo "✅ verify-installed ${TAG}: 전 항목 통과(SKIP 제외)"
