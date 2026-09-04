#!/usr/bin/env bash
# Xvfb 기반 기능 타이핑(L3) 시험 — 설치된(또는 로컬 빌드된) UNIM 산출물이
# 실제 GTK/Qt/XIM 경로를 타고 진짜 한글 조합을 완주하는지 XTEST 로 실측한다.
#
# verify-installed.sh(L1+L2)는 "모듈이 로드되는가·데몬이 응답하는가"만 본다.
# 여기는 그 다음 층 — 툴킷 → IM 모듈 → 데몬 → 화면까지 전 구간을 실제 키
# 입력으로 통과시켜 tests/harness/ 의 회귀 시나리오로 판정한다.
#
# 전제: unim 패키지가 설치됐거나(CI 매트릭스 레그) UNIM_DAEMON_BIN 등으로
#       로컬 target/release 바이너리를 가리킨 컨테이너/러너 안에서 실행.
#       실세션 데몬·~/.config/unim 은 절대 건드리지 않는다 — HOME 을 포함한
#       전 XDG 경로를 이 스크립트가 만드는 스크래치 디렉터리로 격리한다.
#
# 사용법: scripts/ci/functional-test.sh <tag> [--apps gtk3,gtk4,qt5,qt6,xim] [--scenarios s1,s2,...]
#   tag        : 로그 라벨용 (ubuntu24.04|fedora43|el10|... — 자유 문자열)
#   --apps     : 실행할 앱 (기본: gtk3,gtk4,qt5,qt6,xim)
#   --scenarios: 실행할 시나리오 이름 (기본: 미지정 = tests/harness/scenarios/
#                전부 — 8종 필수 회귀 + english-passthrough 등 레이아웃 무관분)
#
# 환경변수:
#   UNIM_DAEMON_BIN          데몬 바이너리 경로 (기본 /usr/libexec/unim-daemon)
#   UNIM_XIM_BIN              unim-xim 바이너리 경로 (기본 /usr/libexec/unim-xim)
#   UNIM_FUNCTEST_ARTIFACT_DIR 로그·스크린샷 상위 디렉터리
#                              (기본: 저장소 루트/functional-logs)
#   UNIM_HARNESS_READY_TIMEOUT / UNIM_HARNESS_WINDOW_TIMEOUT
#                              앱 기동·창 탐색 대기(초) — 느린 CI 컨테이너용
#                              (tests/harness/harness.py 가 읽는다)
#   UNIM_HARNESS_STEP_TIMEOUT_MS / UNIM_HARNESS_SETTLE_MS / UNIM_HARNESS_SCENARIO_RETRIES
#                              스텝 판정 대기(ms)·클릭 직후 안정화 대기(ms)·
#                              시나리오 전체 재시도 횟수 — CI 전용 튜닝
#                              (아래 §CI 기본값 참고, harness.py/run.py 가 읽는다)
#
# ── CI 기본값 (2026-09 실측) ────────────────────────────────────────────────
# GitHub Actions 는 러너/컨테이너 모두에 CI=true 를 자동으로 심는다. 공유
# 러너는 CPU 경합이 있어 XIM/Qt 의 IM 컨텍스트 등록(비동기 D-Bus 왕복)이
# 로컬 docker 보다 늦게 끝날 수 있다 — 클릭 직후 첫 키를 IM 이 아직 못 받아
# 그대로 리터럴 문자가 커밋되는 실패(2026-09 실측: linux-deb.yml ubuntu24.04
# 레그의 xim basic-compose/focus-switch, linux-rpm.yml fedora43 의 qt6
# basic-compose-2bul — 매번 다른 시나리오에서 산발적으로 재현, 로컬 재현
# 안 됨)로 이어진다. 실세션(GUI 사용자가 직접 돌리는 make check-runtime-x11
# 등, CI=true 없음)의 기본 동작은 절대 바꾸지 않는다 — CI 에서만 값을 올린다.
if [ -n "${CI:-}" ]; then
    : "${UNIM_HARNESS_STEP_TIMEOUT_MS:=4000}"   # 기본 2500ms → 4000ms
    : "${UNIM_HARNESS_SETTLE_MS:=300}"          # 포커스 클릭 뒤 추가 안정화 (기본 0)
    : "${UNIM_HARNESS_SCENARIO_RETRIES:=1}"     # 실패 시나리오 1회 전체 재시도 (기본 0)
    export UNIM_HARNESS_STEP_TIMEOUT_MS UNIM_HARNESS_SETTLE_MS UNIM_HARNESS_SCENARIO_RETRIES
fi
#
# ⚠️ `set -e` 를 쓰지 않는다 — 한 앱의 빌드/실행 실패가 나머지 앱 판정을
#    가로막으면 안 된다(verify-installed.sh 와 같은 정책). 대신 앱별 종료
#    코드를 모아 표로 낸 뒤 하나라도 실패하면 이 스크립트가 exit 1 한다.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TAG="${1:?사용법: functional-test.sh <tag> [--apps gtk3,gtk4,qt5,qt6,xim] [--scenarios s1,s2,...]}"
shift

APPS_CSV="gtk3,gtk4,qt5,qt6,xim"
SCENARIOS_CSV=""

while [ $# -gt 0 ]; do
    case "$1" in
        --apps)      APPS_CSV="$2"; shift 2 ;;
        --scenarios) SCENARIOS_CSV="$2"; shift 2 ;;
        *) echo "❌ 알 수 없는 인자: $1" >&2; exit 1 ;;
    esac
done
IFS=',' read -r -a APPS <<< "$APPS_CSV"

DAEMON_BIN="${UNIM_DAEMON_BIN:-/usr/libexec/unim-daemon}"
XIM_BIN="${UNIM_XIM_BIN:-/usr/libexec/unim-xim}"
LOCAL_MODE=0
[ -n "${UNIM_DAEMON_BIN:-}" ] && LOCAL_MODE=1

ARTIFACT_ROOT="${UNIM_FUNCTEST_ARTIFACT_DIR:-$REPO/functional-logs}"
OUT_DIR="$ARTIFACT_ROOT/functional-${TAG}"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

WORK="$(mktemp -d "/tmp/unim-functest.${TAG}.XXXXXX")"

echo "═══ functional-test ${TAG} ═══"
echo "  앱          ${APPS[*]}"
echo "  데몬        $DAEMON_BIN"
echo "  unim-xim    $XIM_BIN"
echo "  로그        $OUT_DIR"
echo "  스크래치    $WORK"
echo

# ── 정리 ─────────────────────────────────────────────────────────────────
XVFB_PID="" DAEMON_PID="" XIM_PID=""
cleanup() {
    [ -n "$XIM_PID" ] && kill "$XIM_PID" 2>/dev/null || true
    [ -n "$DAEMON_PID" ] && kill "$DAEMON_PID" 2>/dev/null || true
    [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

# ── 0. 사전 조건 ─────────────────────────────────────────────────────────
# 필수: 이게 없으면 스크립트 자체가 성립하지 않는다(D-Bus 세션 격리·빌드).
for tool in dbus-run-session gdbus cmake; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "❌ 필수 도구 없음: $tool (bootstrap-{deb,rpm}.sh 가 dbus-x11/cmake 를 설치했는지 확인)"
        exit 1
    fi
done
# 선택(디스플레이·입력): 이게 없는 배포판이 있다 — el10(AlmaLinux/RHEL10) 은
# 2026-09 실측 기준 EPEL10 이 아직 xorg-x11-server-Xvfb/xdotool 을 패키징하지
# 않는다(Xwayland 만 있음, Xvfb 없음). 코드 회귀가 아니라 그 배포판 패키지
# 생태계가 아직 못 따라온 것이므로 빌드/릴리스를 막지 않고 이 시험만 건너뛴다
# — verify-installed.sh(L1+L2, 모듈 로드·데몬 D-Bus 응답)는 이미 통과한 뒤다.
MISSING_DISPLAY_TOOLS=()
for tool in Xvfb xdotool xwininfo; do
    command -v "$tool" >/dev/null 2>&1 || MISSING_DISPLAY_TOOLS+=("$tool")
done
if [ "${#MISSING_DISPLAY_TOOLS[@]}" -gt 0 ]; then
    echo "⚠️  functional-test ${TAG}: 이 플랫폼에 없는 도구 — ${MISSING_DISPLAY_TOOLS[*]}"
    echo "   기능 타이핑(L3) 시험을 건너뛴다(SKIP) — 패키지 생태계 격차, 코드 회귀 아님."
    exit 0
fi
if [ ! -x "$DAEMON_BIN" ]; then
    echo "❌ 데몬 바이너리가 없다: $DAEMON_BIN (UNIM_DAEMON_BIN 으로 오버라이드 가능)"
    exit 1
fi

# ── 1. 테스트 앱 빌드 — tests/unim-test-* 만 직접 cmake, cargo 전체 빌드는
#      끌어오지 않는다(Makefile 의 build-tests 는 build-frontends→build-rust
#      의존이라 설치본 검증에 안 맞는다. 테스트 앱은 tests/common(C) +
#      GTK/Qt/X11 헤더만으로 독립적으로 빌드된다) ──────────────────────────
NPROC="$(nproc 2>/dev/null || echo 4)"

build_cmake_dir() { # dir label
    local dir="$1" label="$2"
    echo "🔨 빌드: $label ($dir)"
    # build/ 를 매번 새로 만든다 — 이 스크립트가 컨테이너 밖(예: 반복 실행하는
    # 러너, 소스를 재사용하는 로컬 개발)에서 두 번째로 돌면 이전 실행의
    # CMakeCache.txt 가 남아 있을 수 있다. cmake 는 캐시가 기록한 절대경로와
    # 지금의 소스/빌드 디렉터리가 한 글자라도 다르면(체크아웃 경로가 매번
    # 바뀌는 CI 컨테이너에서는 거의 항상 다르다) 즉시 에러로 죽는다 — 조용히
    # 잘못된 곳에 빌드하는 것보다는 안전하지만, 매번 새로 만들면 애초에 안 만난다.
    rm -rf "$dir/build"
    mkdir -p "$dir/build"
    if ! (cd "$dir/build" && cmake .. >"$WORK/cmake-$(basename "$dir").log" 2>&1 \
            && make -j"$NPROC" >>"$WORK/cmake-$(basename "$dir").log" 2>&1); then
        echo "❌ 빌드 실패: $label — 로그 $WORK/cmake-$(basename "$dir").log"
        tail -n 40 "$WORK/cmake-$(basename "$dir").log"
        return 1
    fi
    return 0
}

declare -A APP_DIR=(
    [gtk3]="tests/unim-test-gtk3"
    [gtk4]="tests/unim-test-gtk4"
    [qt5]="tests/unim-test-qt"
    [qt6]="tests/unim-test-qt"
    [xim]="tests/unim-test-xim"
)

BUILD_FAILED=0
declare -A DIRS_DONE=()
for app in "${APPS[@]}"; do
    dir="${APP_DIR[$app]:-}"
    if [ -z "$dir" ]; then
        echo "❌ 알 수 없는 앱: $app (gtk3|gtk4|qt5|qt6|xim)"
        exit 1
    fi
    [ -n "${DIRS_DONE[$dir]:-}" ] && continue
    DIRS_DONE[$dir]=1
    if ! build_cmake_dir "$REPO/$dir" "$app"; then
        BUILD_FAILED=1
    fi
done
if [ "$BUILD_FAILED" -ne 0 ]; then
    echo "❌ 테스트 앱 빌드 실패 — 시험을 진행할 수 없다"
    exit 1
fi
echo "✅ 테스트 앱 빌드 완료"
echo

# ── 2. Xvfb 기동 ─────────────────────────────────────────────────────────
# 준비 확인은 xdotool 로 한다(xdpyinfo 가 아니라) — xdpyinfo 는 배포판에 따라
# xwininfo 와 별개 패키지라(Fedora 계열) 요구 도구를 늘리고 싶지 않다.
# xdotool 은 이미 위에서 필수로 확인했다.
XVFB_DISPLAY="${UNIM_FUNCTEST_DISPLAY:-:99}"
Xvfb "$XVFB_DISPLAY" -screen 0 1280x800x24 -ac >"$WORK/xvfb.log" 2>&1 &
XVFB_PID=$!
XVFB_UP=0
for i in $(seq 1 50); do
    DISPLAY="$XVFB_DISPLAY" xdotool getdisplaygeometry >/dev/null 2>&1 && { XVFB_UP=1; break; }
    sleep 0.1
done
if [ "$XVFB_UP" -ne 1 ]; then
    echo "❌ Xvfb 기동 실패 — 로그:"; cat "$WORK/xvfb.log"
    exit 1
fi
export DISPLAY="$XVFB_DISPLAY"
echo "✅ Xvfb $XVFB_DISPLAY 기동 (pid $XVFB_PID)"

# ── 3. 세션 격리 — HOME/XDG/UNIM_* 를 전부 스크래치로. 실세션 설정에
#      절대 닿지 않는다 ────────────────────────────────────────────────────
ISO="$WORK/iso"
mkdir -p "$ISO"/{home,config,data,cache,runtime}
chmod 700 "$ISO/runtime"
export HOME="$ISO/home"
export XDG_RUNTIME_DIR="$ISO/runtime"
export XDG_CONFIG_HOME="$ISO/config"
export XDG_DATA_HOME="$ISO/data"
export XDG_CACHE_HOME="$ISO/cache"
export UNIM_CONFIG_DIR="$ISO/config/unim"
export UNIM_DATA_DIR="$ISO/data/unim"
export UNIM_CACHE_DIR="$ISO/cache/unim"

# 로컬 개발 모드(UNIM_DAEMON_BIN 오버라이드 — make check-runtime-x11) —
# IM 모듈도 패키지 설치본이 아니라 unim-frontends/*/build/ 의 로컬 산출물을
# 봐야 하므로 scripts/sandbox.sh 의 setup_local_modules() 와 같은 요령으로
# GTK_PATH/QT_PLUGIN_PATH 에 얹는다. 설치본을 검증하는 CI 매트릭스 레그
# (UNIM_DAEMON_BIN 미지정)에서는 이 블록이 아무 일도 하지 않는다 — 시스템
# GTK_PATH/QT_PLUGIN_PATH 기본값이 이미 apt/dnf 설치 경로를 안다.
# ⚠️ GTK3 는 GTK_PATH 만으로 안 잡힌다 — gtk-query-immodules-3.0 으로 다시
#    만든 immodules.cache 가 있어야 한다(sandbox.sh 도 같은 한계). 여기서는
#    해소하지 않는다 — gtk3 로컬모드는 알려진 제약으로 남긴다.
if [ "$LOCAL_MODE" -eq 1 ]; then
    LOCAL_MOD="$WORK/local-modules"
    mkdir -p "$LOCAL_MOD/gtk-3.0/3.0.0/immodules" "$LOCAL_MOD/gtk-4.0/4.0.0/immodules" \
             "$LOCAL_MOD/qt5/plugins/platforminputcontexts" "$LOCAL_MOD/qt6/plugins/platforminputcontexts"
    [ -f "$REPO/unim-frontends/gtk3/build/im-unim.so" ] && \
        ln -sf "$REPO/unim-frontends/gtk3/build/im-unim.so" "$LOCAL_MOD/gtk-3.0/3.0.0/immodules/im-unim.so"
    [ -f "$REPO/unim-frontends/gtk4/build/libim-unim.so" ] && \
        ln -sf "$REPO/unim-frontends/gtk4/build/libim-unim.so" "$LOCAL_MOD/gtk-4.0/4.0.0/immodules/libim-unim.so"
    [ -f "$REPO/unim-frontends/qt5/build/libunim.so" ] && \
        ln -sf "$REPO/unim-frontends/qt5/build/libunim.so" "$LOCAL_MOD/qt5/plugins/platforminputcontexts/libunim.so"
    [ -f "$REPO/unim-frontends/qt6/build/libunim.so" ] && \
        ln -sf "$REPO/unim-frontends/qt6/build/libunim.so" "$LOCAL_MOD/qt6/plugins/platforminputcontexts/libunim.so"
    export GTK_PATH="$LOCAL_MOD:${GTK_PATH:-}"
    export QT_PLUGIN_PATH="$LOCAL_MOD/qt5/plugins:$LOCAL_MOD/qt6/plugins:${QT_PLUGIN_PATH:-}"
fi

# dbus-run-session 안에서 데몬을 띄우고 그 서브셸 안에서 시험까지 전부
# 수행한다 — 세션 버스 주소를 이 스크립트 프로세스로 되가져올 방법이
# dbus-run-session 의 표준 사용법에 없기 때문에, 데몬 기동부터 harness 실행,
# 결과 파일 기록까지를 통째로 하나의 dbus-run-session 안에 넣는다.
RUN_SCRIPT="$WORK/run-in-session.sh"
cat >"$RUN_SCRIPT" <<'INNER'
#!/usr/bin/env bash
set -uo pipefail
DAEMON_BIN="$1"; XIM_BIN="$2"; NEED_XIM="$3"; WORK="$4"
shift 4

"$DAEMON_BIN" -n --replace >"$WORK/daemon.log" 2>&1 &
echo $! >"$WORK/daemon.pid"
for i in $(seq 1 40); do
    gdbus call --session -d org.atit.unim.InputMethod \
        -o /org/atit/unim/InputMethod -m org.atit.unim.InputMethod.GetGlobalMode \
        >/dev/null 2>&1 && break
    sleep 0.25
done
if ! gdbus call --session -d org.atit.unim.InputMethod \
        -o /org/atit/unim/InputMethod -m org.atit.unim.InputMethod.GetGlobalMode \
        >/dev/null 2>&1; then
    echo "❌ unim-daemon 이 D-Bus 에 응답하지 않는다 — 로그:"
    cat "$WORK/daemon.log"
    exit 3
fi
echo "✅ unim-daemon 기동 확인"

if [ "$NEED_XIM" = "1" ]; then
    if [ ! -x "$XIM_BIN" ]; then
        echo "❌ unim-xim 바이너리가 없다: $XIM_BIN"
        exit 3
    fi
    XMODIFIERS=@im=unim "$XIM_BIN" >"$WORK/unim-xim.log" 2>&1 &
    echo $! >"$WORK/xim.pid"
    sleep 1
fi

"$@"
rc=$?

kill "$(cat "$WORK/daemon.pid" 2>/dev/null)" 2>/dev/null || true
[ -f "$WORK/xim.pid" ] && kill "$(cat "$WORK/xim.pid" 2>/dev/null)" 2>/dev/null || true
exit "$rc"
INNER
chmod +x "$RUN_SCRIPT"

NEED_XIM=0
for app in "${APPS[@]}"; do [ "$app" = xim ] && NEED_XIM=1; done

# ── 4. harness.py 실행 ───────────────────────────────────────────────────
SCENARIO_ARGS=()
if [ -n "$SCENARIOS_CSV" ]; then
    IFS=',' read -r -a _scs <<< "$SCENARIOS_CSV"
    for s in "${_scs[@]}"; do SCENARIO_ARGS+=(--scenario "$s"); done
fi
APP_ARGS=()
for a in "${APPS[@]}"; do APP_ARGS+=(--app "$a"); done

HARNESS_LOG="$OUT_DIR/harness.log"
# ⚠️ `python3 -u`(PYTHONUNBUFFERED) 필수 — 붙이지 않으면 파이프(tty 아님)로
#    나가는 stdout 이 완전 버퍼링돼, 전역 timeout 이 걸려 프로세스가 죽을 때
#    이미 실행된 시나리오의 PASS/FAIL 출력이 통째로 유실된다(2026-09 실측:
#    fedora43 레그가 rc=124 로 죽었는데 로그에 "unim-daemon 기동 확인" 이후
#    단 한 줄도 안 남아 진단 불가였다). 900s 는 5앱×전 시나리오 실측
#    기준(느린 공유 호스트 포함) 여유를 둔 값 — 600s 는 부하 상황에서 빠듯했다.
# ⚠️ 파이프(`| tee`)로 받지 않는다 — dbus-daemon 이 활성화한 서비스(unim-popup-service,
#    xdg-desktop-portal 류)가 stdout 파이프를 물려받은 채 dbus-run-session 종료 후에도
#    살아남으면 tee 가 EOF 를 영영 못 받아 스크립트가 행(hang)한다(2026-09 실측:
#    debian13·fedora44 레그가 결과 출력 후 수 분간 종료 안 됨). 파일로 직접 쓰고,
#    setsid 로 새 프로세스 그룹에 넣어 끝나면 그룹째 정리한 뒤 로그를 출력한다.
UNIM_HARNESS_OUT="$OUT_DIR" \
    setsid timeout 900 dbus-run-session -- "$RUN_SCRIPT" "$DAEMON_BIN" "$XIM_BIN" "$NEED_XIM" "$WORK" \
    python3 -u "$REPO/tests/harness/run.py" "${APP_ARGS[@]}" "${SCENARIO_ARGS[@]}" --allow-layout-change \
    >"$HARNESS_LOG" 2>&1 &
INNER_PID=$!
wait "$INNER_PID"; HARNESS_RC=$?
# 잔존 프로세스(D-Bus 활성화 서비스 등) 그룹째 정리 — 격리 세션 안의 것만이다
kill -- -"$INNER_PID" 2>/dev/null || true
cat "$HARNESS_LOG"

# ── 5. 진단(실패 시) — Xvfb 스크린샷 + 데몬 stderr 는 harness 가 이미
#      OUT_DIR 밑에 <app>-<scenario>-stepN.png 로 남긴다. 데몬 로그를
#      함께 복사해 CI 아티팩트에서 한 번에 보이게 한다 ─────────────────────
[ -f "$WORK/daemon.log" ] && cp "$WORK/daemon.log" "$OUT_DIR/daemon.log"
[ -f "$WORK/unim-xim.log" ] && cp "$WORK/unim-xim.log" "$OUT_DIR/unim-xim.log"
if command -v xwd >/dev/null 2>&1 && [ "$HARNESS_RC" -ne 0 ]; then
    xwd -root -display "$XVFB_DISPLAY" -out "$OUT_DIR/screen-final.xwd" 2>/dev/null || true
fi

echo
if [ "$HARNESS_RC" -eq 0 ]; then
    echo "✅ functional-test ${TAG}: 전 시나리오 통과 (로그 $OUT_DIR)"
else
    echo "❌ functional-test ${TAG}: 실패 (rc=$HARNESS_RC) — 로그 $OUT_DIR"
fi
exit "$HARNESS_RC"
