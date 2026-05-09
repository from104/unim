/**
 * UNIM GNOME Extension Logging Module
 *
 * UNIM_DEVELOP=1 환경변수가 설정되면 콘솔과 파일에 로그를 동시 출력합니다.
 * 로그 파일: ~/.unim-log/{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log
 * session-tag 우선순위: XDG_SESSION_ID > WAYLAND_DISPLAY > DISPLAY.
 * progname/pid 는 호스트 프로세스 (extension은 보통 gnome-shell).
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

const LOG_PREFIX = 'GNOME_EXT';

/**
 * 개발 모드 확인 (캐싱)
 */
let _developMode = null;

function isDevelopMode() {
    if (_developMode === null) {
        const env = GLib.getenv('UNIM_DEVELOP');
        _developMode = (env === '1');
    }
    return _developMode;
}

function _sanitizeTag(s) {
    return s.replace(/[^A-Za-z0-9_-]/g, '-');
}

function _sessionTag() {
    const xdg = GLib.getenv('XDG_SESSION_ID');
    if (xdg) return `xdg-${_sanitizeTag(xdg)}`;
    const wl = GLib.getenv('WAYLAND_DISPLAY');
    if (wl) return `wl-${_sanitizeTag(wl)}`;
    const x11 = GLib.getenv('DISPLAY');
    if (x11) return `x11-${_sanitizeTag(x11)}`;
    return 'unknown';
}

let _processName = null;

function _processNameCached() {
    if (_processName === null) {
        let name = '';
        try {
            const [ok, contents] = GLib.file_get_contents('/proc/self/comm');
            if (ok) {
                const dec = new TextDecoder();
                name = dec.decode(contents).trim();
            }
        } catch (_) {
            // ignore
        }
        if (!name) {
            name = GLib.get_prgname() || 'unknown';
        }
        _processName = _sanitizeTag(name) || 'unknown';
    }
    return _processName;
}

let _pidCached = null;

function _processPid() {
    if (_pidCached === null) {
        let pid = 0;
        try {
            const [ok, contents] = GLib.file_get_contents('/proc/self/stat');
            if (ok) {
                const text = new TextDecoder().decode(contents);
                const first = text.split(' ', 1)[0];
                const n = parseInt(first, 10);
                if (Number.isFinite(n) && n > 0) pid = n;
            }
        } catch (_) {
            // ignore
        }
        _pidCached = pid;
    }
    return _pidCached;
}

/**
 * 현재 세션·날짜·프로세스에 해당하는 로그 파일 경로를 계산하고 디렉토리를 보장합니다.
 * 매 호출 시 재계산하여 자정을 넘기면 자동으로 다음 날짜 파일로 회전됩니다.
 */
function getLogFilePath() {
    const homeDir = GLib.get_home_dir();
    const logDir = GLib.build_filenamev([homeDir, '.unim-log']);
    GLib.mkdir_with_parents(logDir, 0o700);
    const date = GLib.DateTime.new_now_local().format('%Y-%m-%d');
    return GLib.build_filenamev([
        logDir,
        `${_sessionTag()}_${date}_${_processNameCached()}-${_processPid()}.log`,
    ]);
}

/**
 * 타임스탬프 생성
 */
function getTimestamp() {
    const now = GLib.DateTime.new_now_local();
    return now.format('%Y/%m/%d %H:%M:%S');
}

/**
 * 파일에 로그 추가
 */
function appendToLogFile(logLine) {
    try {
        const file = Gio.File.new_for_path(getLogFilePath());
        const outputStream = file.append_to(Gio.FileCreateFlags.NONE, null);
        outputStream.write_all(logLine + '\n', null);
        outputStream.close(null);
    } catch (e) {
        // 파일 쓰기 실패 시 무시 (콘솔 로그는 계속 출력)
    }
}

/**
 * 중앙 로깅 함수
 * 
 * @param {string} module - 모듈 이름 (예: 'INDICATOR', 'EXTENSION')
 * @param {string} message - 로그 메시지
 */
export function unimLog(module, message) {
    if (!isDevelopMode()) return;
    
    const timestamp = getTimestamp();
    const fullModule = `${LOG_PREFIX}/${module}`;
    const logLine = `[${timestamp}] - [${fullModule}] - ${message}`;
    
    // 콘솔 출력
    console.log(`[unim-${module.toLowerCase()}] ${message}`);
    
    // 파일 출력
    appendToLogFile(logLine);
}

/**
 * 에러 로깅 함수
 */
export function unimError(module, message) {
    if (!isDevelopMode()) return;
    
    const timestamp = getTimestamp();
    const fullModule = `${LOG_PREFIX}/${module}`;
    const logLine = `[${timestamp}] - [${fullModule}] - ERROR: ${message}`;
    
    // 콘솔 출력
    console.error(`[unim-${module.toLowerCase()}] ${message}`);
    
    // 파일 출력
    appendToLogFile(logLine);
}
