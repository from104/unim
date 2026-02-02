/**
 * UNIM GNOME Extension Logging Module
 * 
 * UNIM_DEVELOP=1 환경변수가 설정되면 콘솔과 파일에 로그를 동시 출력합니다.
 * 로그 파일: ~/.unim-errors.log
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

/**
 * 로그 파일 경로 (캐싱)
 */
let _logFilePath = null;

function getLogFilePath() {
    if (_logFilePath === null) {
        const homeDir = GLib.get_home_dir();
        _logFilePath = GLib.build_filenamev([homeDir, '.unim-errors.log']);
    }
    return _logFilePath;
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
