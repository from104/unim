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

for cmd in gnome-shell dbus-run-session dconf; do
    command -v "$cmd" >/dev/null || {
        echo "⏭️  $cmd 가 없어 런타임 스모크를 건너뛴다."
        exit 0
    }
done

# 확장이 어디에도 안 깔려 있으면 셸이 로드할 것 자체가 없다.
if [ ! -d "$HOME/.local/share/gnome-shell/extensions/$UUID" ] &&
   [ ! -d "/usr/share/gnome-shell/extensions/$UUID" ]; then
    echo "⏭️  확장이 설치돼 있지 않아 건너뛴다 —"
    echo "    make install-gnome-extension GNOME_EXTENSION_DIR=\"\$HOME/.local/share/gnome-shell/extensions/$UUID\""
    exit 0
fi

LOG="$(mktemp -t unim-smoke-XXXXXX.log)"
DAEMON_BIN="$ROOT/target/release/unim-daemon"
INNER="$(mktemp -t unim-smoke-inner-XXXXXX.sh)"
trap 'rm -f "$LOG" "$LOG.daemon" "$INNER"' EXIT

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
    UNIM_SMOKE_UUID="$UUID" UNIM_SMOKE_WL_DISPLAY="$DISPLAY_NAME" \
    UNIM_SMOKE_DAEMON_BIN="$DAEMON_BIN" UNIM_SMOKE_DAEMON_LOG="$LOG.daemon" \
    dbus-run-session -- "$INNER" >"$LOG" 2>&1

# 셸은 제한시간에 SIGTERM 으로 끝난다(=124). 그건 정상 종료로 본다 —
# 우리가 보는 것은 종료 코드가 아니라 활성화 로그다.
fail=0

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
