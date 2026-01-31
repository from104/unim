#!/bin/bash
# UNIM Sandbox Environment Script
# Xephyr 기반 격리된 X11 세션에서 UNIM을 테스트합니다.
# 시스템 IM 설정에 영향을 주지 않습니다.

set -e

# === 설정 ===
DISPLAY_NUM="${UNIM_SANDBOX_DISPLAY:-:99}"
SCREEN_SIZE="${UNIM_SANDBOX_SCREEN:-1280x720}"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Rust 바이너리 경로
RUST_BIN="$PROJECT_ROOT/target/release"

# 테스트 앱 경로 (각각의 build 디렉토리)
TEST_GTK3="$PROJECT_ROOT/unim-test-gtk3/build/unim-test-gtk3"
TEST_GTK4="$PROJECT_ROOT/unim-test-gtk4/build/unim-test-gtk4"
TEST_QT5="$PROJECT_ROOT/unim-test-qt5/build/unim-test-qt5"
TEST_QT6="$PROJECT_ROOT/unim-test-qt6/build/unim-test-qt6"
TEST_XIM="$PROJECT_ROOT/unim-test-xim/build/unim-test-xim"

# 색상 출력
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[UNIM-SANDBOX]${NC} $1"; }
log_success() { echo -e "${GREEN}[UNIM-SANDBOX]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[UNIM-SANDBOX]${NC} $1"; }
log_error() { echo -e "${RED}[UNIM-SANDBOX]${NC} $1"; }

# === 사전 조건 확인 ===
check_dependencies() {
    log_info "의존성 확인 중..."
    
    local missing=()
    
    command -v Xephyr >/dev/null 2>&1 || missing+=("xserver-xephyr")
    command -v dbus-run-session >/dev/null 2>&1 || missing+=("dbus-x11")
    
    # stalonetray는 선택적 (인디케이터 테스트용)
    if ! command -v stalonetray >/dev/null 2>&1; then
        log_warn "stalonetray 미설치 - 인디케이터 테스트가 제한됩니다."
        log_warn "설치: sudo apt install stalonetray"
    fi
    
    if [ ${#missing[@]} -gt 0 ]; then
        log_error "필수 패키지가 설치되지 않았습니다:"
        for pkg in "${missing[@]}"; do
            echo "  - $pkg"
        done
        echo ""
        echo "설치 명령어: sudo apt install ${missing[*]}"
        exit 1
    fi
    
    log_success "모든 의존성이 확인되었습니다."
}

# === 빌드 확인 ===
check_build() {
    log_info "빌드 상태 확인 중..."
    
    local required_binaries=(
        "$RUST_BIN/unim-daemon"
        "$RUST_BIN/unim-xim"
    )
    
    for bin in "${required_binaries[@]}"; do
        if [ ! -x "$bin" ]; then
            log_warn "빌드가 필요합니다: $bin"
            log_info "빌드를 시작합니다..."
            make -C "$PROJECT_ROOT" build
            break
        fi
    done
    
    log_success "빌드가 준비되었습니다."
}

# === Xephyr 시작 ===
start_xephyr() {
    log_info "Xephyr X 서버를 시작합니다 (디스플레이: $DISPLAY_NUM, 해상도: $SCREEN_SIZE)..."
    
    # 이미 실행 중인지 확인
    if xdpyinfo -display "$DISPLAY_NUM" >/dev/null 2>&1; then
        log_warn "Xephyr가 이미 $DISPLAY_NUM에서 실행 중입니다."
        return 0
    fi
    
    Xephyr "$DISPLAY_NUM" -screen "$SCREEN_SIZE" -ac -host-cursor &
    XEPHYR_PID=$!
    
    # Xephyr가 준비될 때까지 대기
    for i in {1..30}; do
        if xdpyinfo -display "$DISPLAY_NUM" >/dev/null 2>&1; then
            log_success "Xephyr가 시작되었습니다. (PID: $XEPHYR_PID)"
            return 0
        fi
        sleep 0.1
    done
    
    log_error "Xephyr 시작 시간 초과"
    exit 1
}

# === 정리 ===
cleanup() {
    log_info "샌드박스 환경을 정리합니다..."
    
    # Xephyr 종료
    [ -n "${XEPHYR_PID:-}" ] && kill "$XEPHYR_PID" 2>/dev/null || true
    
    log_success "정리 완료."
}

# === 도움말 ===
show_help() {
    cat << EOF
UNIM 샌드박스 환경 스크립트

사용법: $0 [옵션] [테스트앱]

옵션:
  -h, --help          이 도움말을 표시합니다
  -d, --display NUM   Xephyr 디스플레이 번호 (기본값: :99)
  -s, --size WxH      화면 크기 (기본값: 1280x720)
  --no-build          빌드 확인 건너뛰기
  --no-tray           stalonetray 시작 안함
  --indicator         unim-indicator도 시작

테스트앱:
  gtk3                GTK3 테스트 앱 (unim-test-gtk3)
  gtk4                GTK4 테스트 앱 (unim-test-gtk4)
  qt5                 Qt5 테스트 앱 (unim-test-qt5)
  qt6                 Qt6 테스트 앱 (unim-test-qt6)
  xim                 XIM 테스트 앱 (unim-test-xim)
  gedit               gedit 에디터
  terminal            GNOME Terminal
  (없음)              기본 WM + gnome-terminal

환경변수:
  UNIM_SANDBOX_DISPLAY    Xephyr 디스플레이 번호
  UNIM_SANDBOX_SCREEN     화면 크기 (예: 1920x1080)

예제:
  $0                  기본 설정으로 샌드박스 시작
  $0 gtk4             GTK4 테스트 앱으로 시작
  $0 --indicator      인디케이터와 함께 시작
  $0 -s 1920x1080     큰 화면으로 시작
  $0 --no-build xim   빌드 건너뛰고 XIM 테스트

EOF
}

# === 테스트 앱 경로 반환 ===
get_test_app_path() {
    local app="$1"
    case "$app" in
        gtk3) echo "$TEST_GTK3" ;;
        gtk4) echo "$TEST_GTK4" ;;
        qt5)  echo "$TEST_QT5" ;;
        qt6)  echo "$TEST_QT6" ;;
        xim)  echo "$TEST_XIM" ;;
        *)    echo "" ;;
    esac
}

# === 메인 ===
main() {
    local skip_build=false
    local no_tray=false
    local with_indicator=false
    local test_app=""
    
    # 인자 파싱
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                show_help
                exit 0
                ;;
            -d|--display)
                DISPLAY_NUM="$2"
                shift 2
                ;;
            -s|--size)
                SCREEN_SIZE="$2"
                shift 2
                ;;
            --no-build)
                skip_build=true
                shift
                ;;
            --no-tray)
                no_tray=true
                shift
                ;;
            --indicator)
                with_indicator=true
                shift
                ;;
            *)
                test_app="$1"
                shift
                ;;
        esac
    done
    
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║              UNIM 샌드박스 환경 (Xephyr 기반)                ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    
    # 종료 시 정리
    trap cleanup EXIT
    
    # 단계별 실행
    check_dependencies
    
    if [ "$skip_build" = false ]; then
        check_build
    fi
    
    start_xephyr
    
    # 테스트 앱 경로 결정
    local test_app_path=""
    if [ -n "$test_app" ]; then
        test_app_path=$(get_test_app_path "$test_app")
    fi
    
    log_info "UNIM 세션을 시작합니다..."
    
    # stalonetray 및 indicator 플래그 전달
    local start_tray="$([[ "$no_tray" == "false" ]] && echo "true" || echo "false")"
    local start_indicator="$([[ "$with_indicator" == "true" ]] && echo "true" || echo "false")"
    
    # dbus-run-session 내에서 UNIM 실행
    dbus-run-session -- /bin/bash -c "
        export PATH='/usr/local/bin:/usr/bin:/bin:$RUST_BIN'
        export DISPLAY='$DISPLAY_NUM'
        export GTK_IM_MODULE=unim
        export QT_IM_MODULE=unim
        export XMODIFIERS='@im=unim'
        export CLUTTER_IM_MODULE=xim
        export UNIM_DEVELOP=1
        export LD_LIBRARY_PATH='$RUST_BIN:\${LD_LIBRARY_PATH:-}'
        
        echo ''
        echo '=== UNIM 샌드박스 활성화됨 ==='
        echo \"  DISPLAY=\$DISPLAY\"
        echo \"  GTK_IM_MODULE=\$GTK_IM_MODULE\"
        echo \"  QT_IM_MODULE=\$QT_IM_MODULE\"
        echo \"  XMODIFIERS=\$XMODIFIERS\"
        echo ''
        
        # 간단한 윈도우 매니저 시작 (있으면)
        if command -v openbox >/dev/null 2>&1; then
            echo '[UNIM-SANDBOX] openbox 윈도우 매니저 시작'
            /usr/bin/openbox &
            /bin/sleep 0.3
        elif command -v icewm >/dev/null 2>&1; then
            echo '[UNIM-SANDBOX] icewm 윈도우 매니저 시작'
            /usr/bin/icewm &
            /bin/sleep 0.3
        fi
        
        # stalonetray 시작 (시스템 트레이)
        if [[ '$start_tray' == 'true' ]] && command -v stalonetray >/dev/null 2>&1; then
            echo '[UNIM-SANDBOX] stalonetray 시작 (시스템 트레이)'
            /usr/bin/stalonetray --geometry 1x1+0+0 --icon-size 24 --kludges=force_icons_size &
            TRAY_PID=\$!
            /bin/sleep 0.5
        fi
        
        # UNIM 데몬 시작
        '$RUST_BIN/unim-daemon' &
        DAEMON_PID=\$!
        /bin/sleep 1
        
        # XIM 서버 시작
        '$RUST_BIN/unim-xim' &
        XIM_PID=\$!
        /bin/sleep 0.5
        
        echo '[UNIM-SANDBOX] 데몬 시작됨 (daemon: '\$DAEMON_PID', xim: '\$XIM_PID')'
        
        # 인디케이터 시작 (옵션)
        if [[ '$start_indicator' == 'true' ]]; then
            if [ -x '$RUST_BIN/unim-indicator' ]; then
                echo '[UNIM-SANDBOX] unim-indicator 시작'
                '$RUST_BIN/unim-indicator' &
                INDICATOR_PID=\$!
                /bin/sleep 0.5
                echo '[UNIM-SANDBOX] 인디케이터 시작됨 (PID: '\$INDICATOR_PID')'
            else
                echo '[UNIM-SANDBOX] 경고: unim-indicator를 찾을 수 없습니다'
            fi
        fi
        
        echo ''
        
        # 테스트 앱 또는 터미널 실행
        if [ -n '$test_app_path' ] && [ -x '$test_app_path' ]; then
            echo '[UNIM-SANDBOX] 테스트 앱 실행: $test_app'
            '$test_app_path'
        elif [ '$test_app' = 'gedit' ]; then
            /usr/bin/gedit
        elif [ '$test_app' = 'terminal' ]; then
            /usr/bin/gnome-terminal --wait
        else
            # 기본: gnome-terminal 사용
            if command -v gnome-terminal >/dev/null 2>&1; then
                /usr/bin/gnome-terminal --wait
            elif command -v xterm >/dev/null 2>&1; then
                /usr/bin/xterm
            else
                echo '[UNIM-SANDBOX] 터미널을 찾을 수 없습니다. 수동으로 앱을 실행하세요.'
                echo '샌드박스를 종료하려면 Ctrl+C를 누르세요.'
                /bin/sleep infinity
            fi
        fi
        
        # 정리
        kill \$DAEMON_PID 2>/dev/null || true
        kill \$XIM_PID 2>/dev/null || true
        [ -n \"\${INDICATOR_PID:-}\" ] && kill \$INDICATOR_PID 2>/dev/null || true
        [ -n \"\${TRAY_PID:-}\" ] && kill \$TRAY_PID 2>/dev/null || true
    "
    
    log_success "샌드박스 세션이 종료되었습니다."
}

main "$@"
