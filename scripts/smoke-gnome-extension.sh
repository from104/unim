#!/usr/bin/env bash
# GNOME 확장 런타임 스모크 테스트
#
# 확장을 headless GNOME Shell 에 실제로 올려 보고, 활성화가 **끝까지** 갔는지
# 로그로 확인한다. 정적 대조(check-gnome-api.sh)가 못 잡는 것을 잡는다:
# enable() 이 중간에 예외로 끝나도 GNOME 은 확장을 "사용 중" 으로 계속 표시한다.
# 2026-08-23 셸 50 사고가 그랬다 — 목록상 멀쩡한데 한 글자도 안 들어갔다.
#
# 로그아웃 없이 돌기 때문에 개발 중에도 부담이 없다. 실제 세션의 셸·데몬은
# 건드리지 않는다 — dbus-run-session 이 세션 버스를 통째로 갈라 놓는다.
#
# 사용법: scripts/smoke-gnome-extension.sh [제한시간초]
# 종료 코드: 0 = 통과, 1 = 활성화 실패, 2 = 검사 불가(환경 부족)

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMEOUT="${1:-25}"
UUID="unim-gnome@from104.github.io"

for cmd in gnome-shell dbus-run-session dconf glib-compile-schemas; do
    command -v "$cmd" >/dev/null || {
        echo "⏭️  $cmd 가 없어 런타임 스모크를 건너뛴다."
        exit 0
    }
done

# 검사 대상은 설치본이 아니라 이 워크트리다.
# 예전에는 $HOME/.local/share 의 설치본을 로드했는데, 그건 마지막으로
# `make install-gnome-extension` 한 시점의 사본이라 방금 고친 코드를 검사하지
# 않는다(2026-09-20 실측: 한 달 가까이 묵은 사본을 통과시키고 있었다).
if [ ! -d "$ROOT/unim-gnome-extension" ]; then
    echo "⏭️  unim-gnome-extension 디렉터리가 없어 건너뛴다."
    exit 0
fi

LOG="$(mktemp -t unim-smoke-XXXXXX.log)"
DAEMON_BIN="$ROOT/target/release/unim-daemon"
INNER="$(mktemp -t unim-smoke-inner-XXXXXX.sh)"
# dconf 는 세션 버스와 무관하게 $XDG_CONFIG_HOME/dconf/user 하나를 공유한다.
# 아래 INNER 가 쓰는 gsettings/dconf 값(enabled-extensions·enable-ime)은 스모크
# 전용인데, 격리하지 않으면 그 값이 로그인해 있는 실제 데스크톱에 그대로 반영된다
# (2026-09-20 실측: enabled-extensions 가 확장 하나만 남기고 덮여 실행 중 셸이
#  나머지 확장을 즉시 비활성화했다). 임시 XDG_CONFIG_HOME 으로 dconf DB 를 갈라
# 스모크의 쓰기가 실세션에 닿지 않게 한다.
SMOKE_CFG="$(mktemp -d -t unim-smoke-cfg-XXXXXX)"
# 워크트리 확장을 임시 XDG_DATA_HOME 에 올린다 — 실사용 설치본을 건드리지 않고
# 지금 소스를 검사하기 위해서다. 구성은 install-gnome-extension 과 같다.
SMOKE_DATA="$(mktemp -d -t unim-smoke-data-XXXXXX)"
# XDG_RUNTIME_DIR 도 가른다. 데몬의 PID 파일(unim-daemon.pid)이 여기에 있어서,
# 공유하면 스모크 데몬이 실사용 데몬을 보고 "이미 실행 중"이라며 물러난다.
# 그러면 확장이 격리 버스에서 이름을 부를 때 D-Bus 자동 활성화가 설치본
# `unim-daemon -n --replace` 를 띄우고, 그 --replace 가 공유 PID 파일을 읽어
# **실사용 데몬을 SIGTERM 으로 죽인다**(2026-09-23 실측 — 화면 잠금 중 입력기가
# 3시간 죽어 있었다). 아래 서비스 파일 가림막과 함께 이중으로 막는다.
SMOKE_RUN="$(mktemp -d -t unim-smoke-run-XXXXXX)"
chmod 700 "$SMOKE_RUN"
trap 'rm -f "$LOG" "$LOG.daemon" "$INNER"; rm -rf "$SMOKE_CFG" "$SMOKE_DATA" "$SMOKE_RUN"' EXIT

# 격리 버스에서 org.atit.unim.InputMethod 자동 활성화를 막는다. 세션 버스는
# $XDG_DATA_HOME/dbus-1/services 를 시스템 경로보다 먼저 보고 먼저 찾은 것을
# 쓰므로, 여기 둔 가림막이 /usr/share 의 `--replace` 서비스 파일을 이긴다.
# 데몬은 INNER 가 직접 띄운다 — 활성화로 뜨는 데몬은 없어야 한다.
mkdir -p "$SMOKE_DATA/dbus-1/services"
cat >"$SMOKE_DATA/dbus-1/services/org.atit.unim.InputMethod.service" <<'SVC_EOF'
[D-BUS Service]
Name=org.atit.unim.InputMethod
Exec=/bin/false
SVC_EOF

# 실사용 데몬이 스모크 전후로 그대로인지 확인한다(마지막에 대조).
LIVE_DAEMON_BEFORE="$(pgrep -x unim-daemon | sort | paste -sd' ')"

EXT_STAGE="$SMOKE_DATA/gnome-shell/extensions/$UUID"
mkdir -p "$EXT_STAGE"
cp -rf "$ROOT/unim-gnome-extension/." "$EXT_STAGE/"
rm -rf "$EXT_STAGE/bin" "$EXT_STAGE/po" "$EXT_STAGE/SPEC.md"
glib-compile-schemas "$EXT_STAGE/schemas"
echo "워크트리 확장 스테이징: $EXT_STAGE"

# --wayland-display 를 고유하게 줘서 실제 세션의 소켓 이름과 부딪히지 않게 한다.
# GNOME 50 부터 --nested 는 없어졌고 --display-server 를 안 주면 기본이 중첩이다.
DISPLAY_NAME="unim-smoke-$$"
echo "headless GNOME Shell 기동 (제한시간 ${TIMEOUT}초)…"

# 아래가 안 갖춰지면 뒤의 마커 검사가 항상 실패한다 — 개발 편의가 아니라
# 검사 자체의 전제조건이다:
#  1) UNIM_DEVELOP=1 — unimLog/unimError(logging.js) 는 이게 없으면 아예
#     console.log 를 안 낸다. 안 켜면 확장이 완벽히 동작해도 로그가 빈다.
#  2) LANG=C.UTF-8 — 러너 로케일이 POSIX/C 면 gjs console.log 의 한글이
#     전부 '?' 로 깨져서(글자 수는 맞는데 내용이 안 맞아) 마커 grep 이 실패한다.
#     개발자 기계는 보통 이미 UTF-8 이라 안 드러나던 문제.
#  3) enabled-extensions — install-gnome-extension 은 파일만 깔지, GNOME 은
#     org.gnome.shell.enabled-extensions 에 UUID 가 없으면 enable() 을 아예
#     안 부른다. 러너엔 기존 dconf 상태가 없으니 매번 명시적으로 켜야 한다.
#  4) enable-ime — 확장 자체의 GSettings 키(기본값 false). enable() 안에서
#     이게 true 여야 _enableIME() 이 불려서 [unim-ime] 마커까지 간다.
#     확장 스키마는 relocatable 이 아니라 gsettings 대신 dconf write 로
#     직접 쓴다 — gsettings 는 스키마가 표준 검색경로에 없으면 못 찾는다.
#  5) unim-daemon — _enableIME() 은 DBus(org.atit.unim.InputMethod) 연결에
#     성공해야 setActive(true) 까지 간다. 데몬이 없으면 "DBus 미연결" 로
#     조용히 되돌아간다 — 이 자체가 활성화 "실패"는 아니지만 이 스모크가
#     확인하려는 "끝까지 갔는가"는 데몬 없이는 증명할 수 없다. target/release
#     에 없으면(빌드 전) 데몬 없이 진행하고, 그 경우 [unim-ime] 마커는
#     정직하게 실패로 남는다 — 숨기지 않는다.
#  6) --no-x11 — headless 러너의 /tmp/.X11-unix 는 systemd-tmpfiles 가 안
#     돌아 1777 로 준비돼 있지 않을 수 있다. 이 확장은 Wayland 만 쓰므로
#     Xwayland 자체를 끄면 그 구멍을 피해간다(정적 검사는 GI 심볼만 보므로
#     영향 없음).
# 전부 같은 dbus-run-session 세션 버스 안에서, 셸보다 먼저 실행해야 한다.
cat >"$INNER" <<'INNER_EOF'
#!/usr/bin/env bash
set -uo pipefail
if [ -n "${UNIM_SMOKE_DAEMON_BIN:-}" ] && [ -x "$UNIM_SMOKE_DAEMON_BIN" ]; then
    "$UNIM_SMOKE_DAEMON_BIN" -n >"$UNIM_SMOKE_DAEMON_LOG" 2>&1 &
    for _ in $(seq 1 50); do
        dbus-send --session --print-reply --dest=org.freedesktop.DBus \
            /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner \
            string:org.atit.unim.InputMethod 2>/dev/null | grep -q 'boolean true' && break
        sleep 0.1
    done
fi
gsettings set org.gnome.shell enabled-extensions "[\"$UNIM_SMOKE_UUID\"]"
dconf write /org/gnome/shell/extensions/unim/enable-ime true
exec gnome-shell --wayland --headless --no-x11 --virtual-monitor 800x600 \
                  --wayland-display "$UNIM_SMOKE_WL_DISPLAY"
INNER_EOF
chmod +x "$INNER"

timeout -s TERM "$TIMEOUT" env UNIM_DEVELOP=1 LANG=C.UTF-8 \
    XDG_CONFIG_HOME="$SMOKE_CFG" XDG_DATA_HOME="$SMOKE_DATA" XDG_RUNTIME_DIR="$SMOKE_RUN" \
    UNIM_SMOKE_UUID="$UUID" UNIM_SMOKE_WL_DISPLAY="$DISPLAY_NAME" \
    UNIM_SMOKE_DAEMON_BIN="$DAEMON_BIN" UNIM_SMOKE_DAEMON_LOG="$LOG.daemon" \
    dbus-run-session -- "$INNER" >"$LOG" 2>&1

# 셸은 제한시간에 SIGTERM 으로 끝난다(=124). 그건 정상 종료로 본다 —
# 우리가 보는 것은 종료 코드가 아니라 활성화 로그다.
fail=0

# 스모크 데몬 정리 — 세션 버스가 닫혀도 잠깐 남는다. 격리된 PID 파일로만 찾는다
# (pgrep 이름 매칭은 실사용 데몬까지 잡으므로 쓰지 않는다).
SMOKE_DAEMON_PID="$(cat "$SMOKE_RUN/unim-daemon.pid" 2>/dev/null)"
if [ -n "$SMOKE_DAEMON_PID" ] && kill -0 "$SMOKE_DAEMON_PID" 2>/dev/null; then
    kill "$SMOKE_DAEMON_PID" 2>/dev/null
    for _ in $(seq 1 20); do kill -0 "$SMOKE_DAEMON_PID" 2>/dev/null || break; sleep 0.1; done
fi

# 실사용 데몬 보호 확인 — 격리가 새면 가장 먼저 여기서 드러난다. 시작 전에 있던
# 데몬이 전부 살아 있어야 한다(스모크 쪽 프로세스가 더 보이는 건 문제가 아니다).
LIVE_DAEMON_AFTER="$(pgrep -x unim-daemon | sort | paste -sd' ')"
live_lost=""
for pid in $LIVE_DAEMON_BEFORE; do
    kill -0 "$pid" 2>/dev/null || live_lost="$live_lost $pid"
done
if [ -n "$live_lost" ]; then
    echo "❌ 실사용 unim-daemon 이 죽었다:$live_lost (지금: [${LIVE_DAEMON_AFTER:-없음}])"
    echo "   스모크 격리가 샜다. 입력기가 죽었으면 로그아웃 없이 이렇게 되살린다:"
    echo "   gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \\"
    echo "     --method org.freedesktop.DBus.StartServiceByName org.atit.unim.InputMethod 0"
    fail=1
fi

if grep -q 'IME 활성화 실패' "$LOG"; then
    echo "❌ IME 활성화가 예외로 끝났다:"
    grep -m3 'IME 활성화 실패' "$LOG" | sed 's/^/   /'
    fail=1
fi

for marker in '\[unim-extension\] Extension 활성화 시작' \
              '\[unim-ime\] IME 활성화' \
              '\[unim-extension\] Extension 활성화 완료'; do
    if ! grep -qE "$marker" "$LOG"; then
        echo "❌ 로그에 없어야 할 공백: ${marker//\\/}"
        fail=1
    fi
done

# 이 확장이 뿜은 CRITICAL 만 본다. 다른 확장·셸 자체의 것은 우리 소관이 아니다.
# 이름이 'unim' 으로 시작하는 **옛 확장의 잔재**(unim-indicator@ 등)도 여기서
# 걸러야 한다 — 남의 기계에 뭐가 널려 있든 우리 빌드가 빨개질 이유는 없다.
if grep -E 'CRITICAL' "$LOG" | grep -E "$UUID|\[unim-[a-z_]+\]" | grep -qv 'Could not load extension'; then
    echo "❌ 확장이 CRITICAL 을 냈다:"
    grep -E 'CRITICAL' "$LOG" | grep -E "$UUID|\[unim-[a-z_]+\]" |
        grep -v 'Could not load extension' | head -5 | cut -c1-160 | sed 's/^/   /'
    fail=1
fi

# 옛 UNIM 확장 잔재는 실패가 아니라 안내다 — 지우면 로그가 조용해진다.
if grep -q 'Could not load extension unim-' "$LOG"; then
    stale=$(grep -oE 'Could not load extension unim-[^:]+' "$LOG" |
            sed 's/Could not load extension //' | sort -u | paste -sd' ')
    echo "ℹ️  옛 UNIM 확장 잔재가 이 기계에 남아 있다: $stale"
    echo "   로드에 실패할 뿐 동작에는 영향이 없지만, 지우면 로그가 깨끗해진다."
fi

if [ "$fail" -ne 0 ]; then
    cp "$LOG" "$ROOT/gnome-smoke-fail.log"
    [ -f "$LOG.daemon" ] && cp "$LOG.daemon" "$ROOT/gnome-smoke-fail-daemon.log"
    echo ""
    echo "전체 로그: $ROOT/gnome-smoke-fail.log"
    [ -f "$LOG.daemon" ] && echo "데몬 로그: $ROOT/gnome-smoke-fail-daemon.log"
    exit 1
fi

echo "✅ 확장이 $(gnome-shell --version) 에서 끝까지 활성화된다."
