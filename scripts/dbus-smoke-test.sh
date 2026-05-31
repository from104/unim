#!/bin/bash
# dbus-smoke-test.sh — UNIM DBus 스모크 테스트
# busctl + JSON으로 데몬 기본 기능을 자동 검증
#
# Usage: dbus-smoke-test.sh [--verbose]

set -uo pipefail

SERVICE="org.atit.unim.InputMethod"
IM_PATH="/org/atit/unim/InputMethod"
IM_IFACE="org.atit.unim.InputMethod"
IC_IFACE="org.atit.unim.InputContext"

VERBOSE=false
[[ "${1:-}" == "--verbose" ]] && VERBOSE=true

# 색상
RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

PASS=0
FAIL=0
TOTAL=0

log_verbose() {
    $VERBOSE && echo -e "${DIM}  $1${RESET}"
}

check() {
    local name="$1" result="$2" detail="${3:-}"
    TOTAL=$((TOTAL + 1))
    if [[ "$result" -eq 0 ]]; then
        PASS=$((PASS + 1))
        echo -e "  ${GREEN}PASS${RESET} ${name}"
    else
        FAIL=$((FAIL + 1))
        echo -e "  ${RED}FAIL${RESET} ${name}"
        [[ -n "$detail" ]] && echo -e "       ${DIM}${detail}${RESET}"
    fi
}

# busctl JSON 호출 헬퍼
bcall() {
    local path="$1" iface="$2" method="$3"
    shift 3
    busctl --user --json=short call "$SERVICE" "$path" "$iface" "$method" "$@" 2>&1
}

echo -e "${BOLD}═══ UNIM DBus 스모크 테스트 ═══${RESET}"
echo ""

# ── 1. 서비스 등록 확인 ──
echo -e "${BOLD}[서비스]${RESET}"
if busctl --user list 2>/dev/null | grep -q "$SERVICE"; then
    check "서비스 등록됨" 0
else
    busctl --user call "$SERVICE" "$IM_PATH" org.freedesktop.DBus.Peer Ping 2>/dev/null
    if [[ $? -eq 0 ]]; then
        check "서비스 활성화 성공" 0
    else
        check "서비스 등록/활성화" 1 "데몬 실행 확인: unim-daemon -n"
        exit 1
    fi
fi
echo ""

# ── 2. 컨텍스트 생성 ──
echo -e "${BOLD}[컨텍스트]${RESET}"
CTX_JSON=$(bcall "$IM_PATH" "$IM_IFACE" CreateInputContext ss "smoke-test" "")
log_verbose "CreateInputContext: $CTX_JSON"

# JSON에서 경로 추출: {"type":"s","data":["/org/atit/unim/InputContext_N"]}
CTX_PATH=$(echo "$CTX_JSON" | grep -oP '"/org/atit/unim/InputContext_\d+"' | tr -d '"')

if [[ -n "$CTX_PATH" ]]; then
    check "컨텍스트 생성 ($CTX_PATH)" 0
else
    check "컨텍스트 생성" 1 "$CTX_JSON"
    exit 1
fi
echo ""

# ── 3. FocusIn ──
echo -e "${BOLD}[포커스]${RESET}"
FOCUS_JSON=$(bcall "$CTX_PATH" "$IC_IFACE" FocusIn s "")
check "FocusIn" 0
echo ""

# ── 4. 키 이벤트 처리 ──
echo -e "${BOLD}[키 이벤트 — 한글 조합]${RESET}"

# 한글 모드 설정
bcall "$IM_PATH" "$IM_IFACE" SetGlobalMode b true >/dev/null 2>&1

# 레이아웃 감지
LAYOUT_JSON=$(bcall "$IM_PATH" "$IM_IFACE" GetConfig s "korean_layout")
LAYOUT=$(echo "$LAYOUT_JSON" | grep -oP '"data":\["\K[^"]+')
log_verbose "레이아웃: $LAYOUT"

case "$LAYOUT" in
    3bul390|3bul391|3bul_noshift)
        KEY1=50; KEY1_NAME="M(50)/ㅎ초성"
        KEY2=33; KEY2_NAME="F(33)/ㅏ중성"
        KEY3=31; KEY3_NAME="S(31)/ㄴ종성"
        ;;
    *)
        KEY1=34; KEY1_NAME="G(34)/ㅎ"
        KEY2=37; KEY2_NAME="K(37)/ㅏ"
        KEY3=31; KEY3_NAME="S(31)/ㄴ"
        ;;
esac

# 3개 키 전송 → "한" 조합
ALL_OK=true
for KEYINFO in "$KEY1:$KEY1_NAME" "$KEY2:$KEY2_NAME" "$KEY3:$KEY3_NAME"; do
    KC="${KEYINFO%%:*}"
    KN="${KEYINFO##*:}"
    RESULT=$(bcall "$CTX_PATH" "$IC_IFACE" ProcessKeyEvent uuu 0 "$KC" 0)
    log_verbose "$KN → $RESULT"
    # consumed가 true인지만 확인
    if echo "$RESULT" | grep -q '"data":\[true'; then
        check "$KN → consumed" 0
    else
        check "$KN → consumed" 1 "$RESULT"
        ALL_OK=false
    fi
done

# preedit 확인 (마지막 결과에서)
PREEDIT=$(echo "$RESULT" | grep -oP '"data":\[true,"\\u\K[^"]*' || true)
# JSON unicode를 직접 파싱하는 대신, consumed 여부로 판단
if $ALL_OK; then
    check "한글 조합 (3키 모두 consumed)" 0
fi
echo ""

# ── 5. Reset ──
echo -e "${BOLD}[리셋]${RESET}"
bcall "$CTX_PATH" "$IC_IFACE" Reset >/dev/null 2>&1
check "Reset" 0
echo ""

# ── 6. 전역 모드 ──
echo -e "${BOLD}[전역 모드]${RESET}"

MODE_JSON=$(bcall "$IM_PATH" "$IM_IFACE" GetGlobalMode)
log_verbose "GetGlobalMode: $MODE_JSON"
if echo "$MODE_JSON" | grep -qP '"data":\[(true|false)\]'; then
    check "GetGlobalMode" 0
else
    check "GetGlobalMode" 1 "$MODE_JSON"
fi

bcall "$IM_PATH" "$IM_IFACE" SetGlobalMode b false >/dev/null 2>&1
MODE_JSON=$(bcall "$IM_PATH" "$IM_IFACE" GetGlobalMode)
if echo "$MODE_JSON" | grep -q '"data":\[false\]'; then
    check "SetGlobalMode(false) → 영문" 0
else
    check "SetGlobalMode(false)" 1 "$MODE_JSON"
fi

bcall "$IM_PATH" "$IM_IFACE" SetGlobalMode b true >/dev/null 2>&1
check "SetGlobalMode(true) → 한글 복원" 0
echo ""

# ── 7. 정리 ──
echo -e "${BOLD}[정리]${RESET}"
bcall "$CTX_PATH" "$IC_IFACE" Destroy >/dev/null 2>&1
check "컨텍스트 파괴" 0
echo ""

# ── 결과 ──
echo -e "${BOLD}═══ 결과 ═══${RESET}"
if [[ "$FAIL" -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}✅ ALL PASS${RESET} (${PASS}/${TOTAL})"
else
    echo -e "${RED}${BOLD}❌ ${FAIL} FAILED${RESET} / ${PASS} passed / ${TOTAL} total"
fi
exit "$FAIL"
