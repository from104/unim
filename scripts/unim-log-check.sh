#!/bin/bash
# unim-log-check.sh — UNIM 로그 분석 도구
# Usage: unim-log-check.sh [OPTIONS]
#   --clear          로그 파일 초기화
#   --watch          실시간 모니터링 (tail -f + 컬러)
#   --module MODULE  특정 모듈만 필터 (ENGINE, DBUS, XIM 등)
#   --errors         에러/경고만 표시
#   --since "TIME"   특정 시간 이후만 (형식: "YYYY/MM/DD HH:MM:SS")
#   --tail N         마지막 N줄만 (기본: 30)
#   --summary        요약만 표시 (기본 동작)

set -euo pipefail

LOG_FILE="${HOME}/.unim-errors.log"
MODE="summary"
MODULE_FILTER=""
ERRORS_ONLY=false
SINCE=""
TAIL_N=30

# 색상 코드
RED='\033[0;31m'
YELLOW='\033[0;33m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

usage() {
    echo "Usage: $(basename "$0") [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --clear          로그 파일 초기화"
    echo "  --watch          실시간 모니터링"
    echo "  --module MODULE  특정 모듈 필터 (ENGINE, DBUS, XIM 등)"
    echo "  --errors         에러/경고만 표시"
    echo "  --since TIME     특정 시간 이후 (\"YYYY/MM/DD HH:MM:SS\")"
    echo "  --tail N         마지막 N줄 (기본: 30)"
    echo "  --summary        요약만 (기본)"
    echo "  -h, --help       도움말"
}

# 컬러 하이라이팅 함수
colorize_line() {
    local line="$1"
    # error/panic/failed 는 빨간색
    if echo "$line" | grep -qiE '(error|panic|failed|실패)'; then
        echo -e "${RED}${line}${RESET}"
    # warning 은 노란색
    elif echo "$line" | grep -qiE '(warn|warning|경고)'; then
        echo -e "${YELLOW}${line}${RESET}"
    else
        echo "$line"
    fi
}

# 요약 출력
show_summary() {
    if [[ ! -f "$LOG_FILE" ]]; then
        echo -e "${YELLOW}로그 파일 없음: ${LOG_FILE}${RESET}"
        return
    fi

    local total
    total=$(wc -l < "$LOG_FILE")

    if [[ "$total" -eq 0 ]]; then
        echo -e "${GREEN}✅ 로그 비어있음 — 깨끗!${RESET}"
        return
    fi

    echo -e "${BOLD}═══ UNIM 로그 분석 ═══${RESET}"
    echo -e "${DIM}파일: ${LOG_FILE}${RESET}"
    echo -e "총 라인: ${BOLD}${total}${RESET}"
    echo ""

    # 모듈별 카운트
    echo -e "${BOLD}📊 모듈별 로그:${RESET}"
    grep -oP '\] - \[\K[A-Z_]+' "$LOG_FILE" 2>/dev/null | sort | uniq -c | sort -rn | while read -r count module; do
        printf "  %-16s %s\n" "$module" "$count"
    done
    echo ""

    # 에러/경고 카운트
    local errors warnings
    errors=$(grep -ciE '(error|panic|failed|실패)' "$LOG_FILE" 2>/dev/null || true)
    errors=${errors:-0}
    errors=$(echo "$errors" | tr -d '[:space:]')
    warnings=$(grep -ciE '(warn|warning|경고)' "$LOG_FILE" 2>/dev/null || true)
    warnings=${warnings:-0}
    warnings=$(echo "$warnings" | tr -d '[:space:]')

    if [[ "$errors" -gt 0 ]]; then
        echo -e "${RED}❌ 에러: ${errors}건${RESET}"
    else
        echo -e "${GREEN}✅ 에러: 0건${RESET}"
    fi

    if [[ "$warnings" -gt 0 ]]; then
        echo -e "${YELLOW}⚠️  경고: ${warnings}건${RESET}"
    fi
    echo ""

    # 에러 라인 표시
    if [[ "$errors" -gt 0 ]]; then
        echo -e "${BOLD}${RED}── 에러 내용 ──${RESET}"
        grep -iE '(error|panic|failed|실패)' "$LOG_FILE" | tail -10 | while IFS= read -r line; do
            echo -e "  ${RED}${line}${RESET}"
        done
        echo ""
    fi

    # 마지막 N줄
    echo -e "${BOLD}── 최근 ${TAIL_N}줄 ──${RESET}"
    tail -n "$TAIL_N" "$LOG_FILE" | while IFS= read -r line; do
        colorize_line "  $line"
    done
}

# 실시간 모니터링
watch_log() {
    echo -e "${BOLD}🔍 UNIM 로그 실시간 모니터링${RESET} (Ctrl+C로 종료)"
    echo -e "${DIM}파일: ${LOG_FILE}${RESET}"
    echo "──────────────────────────────────────"

    local tail_args="-f"
    if [[ ! -f "$LOG_FILE" ]]; then
        touch "$LOG_FILE"
    fi

    tail $tail_args "$LOG_FILE" | while IFS= read -r line; do
        # 모듈 필터
        if [[ -n "$MODULE_FILTER" ]]; then
            echo "$line" | grep -q "\[$MODULE_FILTER\]" || continue
        fi
        # 에러 필터
        if [[ "$ERRORS_ONLY" == true ]]; then
            echo "$line" | grep -qiE '(error|panic|failed|warn|실패|경고)' || continue
        fi
        colorize_line "$line"
    done
}

# 필터링된 출력
show_filtered() {
    if [[ ! -f "$LOG_FILE" ]]; then
        echo -e "${YELLOW}로그 파일 없음: ${LOG_FILE}${RESET}"
        return
    fi

    local content
    content=$(cat "$LOG_FILE")

    # --since 필터
    if [[ -n "$SINCE" ]]; then
        local filtered=""
        while IFS= read -r line; do
            # 타임스탬프 추출: [YYYY/MM/DD HH:MM:SS]
            local ts
            ts=$(echo "$line" | grep -oP '^\[\K[0-9/]+ [0-9:]+' || echo "")
            if [[ -n "$ts" && "$ts" > "$SINCE" ]] || [[ -z "$ts" ]]; then
                filtered+="${line}"$'\n'
            fi
        done <<< "$content"
        content="$filtered"
    fi

    # --module 필터
    if [[ -n "$MODULE_FILTER" ]]; then
        content=$(echo "$content" | grep "\[$MODULE_FILTER\]" || true)
    fi

    # --errors 필터
    if [[ "$ERRORS_ONLY" == true ]]; then
        content=$(echo "$content" | grep -iE '(error|panic|failed|warn|실패|경고)' || true)
    fi

    if [[ -z "$content" ]]; then
        echo -e "${GREEN}✅ 필터 조건에 맞는 로그 없음${RESET}"
        return
    fi

    echo "$content" | tail -n "$TAIL_N" | while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        colorize_line "$line"
    done
}

# 인자 파싱
while [[ $# -gt 0 ]]; do
    case "$1" in
        --clear)
            > "$LOG_FILE"
            echo -e "${GREEN}✅ 로그 초기화 완료${RESET}"
            exit 0
            ;;
        --watch)
            MODE="watch"
            shift
            ;;
        --module)
            MODULE_FILTER="$2"
            shift 2
            ;;
        --errors)
            ERRORS_ONLY=true
            shift
            ;;
        --since)
            SINCE="$2"
            shift 2
            ;;
        --tail)
            TAIL_N="$2"
            shift 2
            ;;
        --summary)
            MODE="summary"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}알 수 없는 옵션: $1${RESET}"
            usage
            exit 1
            ;;
    esac
done

# 모드별 실행
case "$MODE" in
    watch)
        watch_log
        ;;
    summary)
        if [[ -n "$MODULE_FILTER" || "$ERRORS_ONLY" == true || -n "$SINCE" ]]; then
            show_filtered
        else
            show_summary
        fi
        ;;
esac
