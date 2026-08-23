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

for cmd in gnome-shell dbus-run-session; do
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
trap 'rm -f "$LOG"' EXIT

# --wayland-display 를 고유하게 줘서 실제 세션의 소켓 이름과 부딪히지 않게 한다.
# GNOME 50 부터 --nested 는 없어졌고 --display-server 를 안 주면 기본이 중첩이다.
DISPLAY_NAME="unim-smoke-$$"
echo "headless GNOME Shell 기동 (제한시간 ${TIMEOUT}초)…"
timeout -s TERM "$TIMEOUT" env dbus-run-session -- \
    gnome-shell --wayland --headless --virtual-monitor 800x600 \
                --wayland-display "$DISPLAY_NAME" >"$LOG" 2>&1

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
    echo ""
    echo "전체 로그: $ROOT/gnome-smoke-fail.log"
    exit 1
fi

echo "✅ 확장이 $(gnome-shell --version) 에서 끝까지 활성화된다."
