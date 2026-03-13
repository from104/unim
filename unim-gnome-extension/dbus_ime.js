/**
 * UNIM DBus IME Client for GNOME Shell Extension
 *
 * unim-daemon의 InputMethod / InputContext DBus 서비스와 통신하여
 * 입력 컨텍스트 생성/파괴, 키 이벤트 처리, 한자/특수문자 후보 조회를 담당합니다.
 *
 * @module dbus_ime
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import { unimLog, unimError } from './logging.js';

/** DBus 서비스 상수 */
const UNIM_BUS_NAME = 'org.atit.unim.InputMethod';
const UNIM_OBJECT_PATH = '/org/atit/unim/InputMethod';
const UNIM_IM_INTERFACE = 'org.atit.unim.InputMethod';
const UNIM_IC_INTERFACE = 'org.atit.unim.InputContext';

/** DBus 호출 타임아웃 (밀리초). 입력 지연을 최소화하기 위해 짧게 설정. */
const DBUS_TIMEOUT_MS = 500;

/**
 * UNIM DBus IME 클라이언트
 *
 * InputMethod 팩토리로 InputContext를 생성하고,
 * 생성된 컨텍스트를 통해 키 이벤트, 포커스, 한자/특수문자를 처리합니다.
 */
export class UnimDbusIME {
    constructor() {
        /** @type {Gio.DBusProxy|null} InputMethod 팩토리 프록시 */
        this._imProxy = null;
        /** @type {Gio.DBusProxy|null} InputContext 프록시 */
        this._icProxy = null;
        /** @type {string|null} 생성된 InputContext의 DBus 객체 경로 */
        this._contextPath = null;
        /** @type {number} g-signal 핸들러 ID (GlobalModeChanged) */
        this._imSignalId = 0;
        /** @type {number} g-signal 핸들러 ID (InputContext 시그널) */
        this._icSignalId = 0;
        /** @type {boolean} 데몬 연결 상태 */
        this._connected = false;
        /** @type {Function|null} 모드 변경 콜백 */
        this._onModeChanged = null;
        /** @type {Function|null} 한자 팝업 표시 콜백 */
        this._onShowHanja = null;
        /** @type {Function|null} 특수문자 팝업 표시 콜백 */
        this._onShowSpecial = null;
        /** @type {Function|null} 팝업 숨김 콜백 */
        this._onHidePopup = null;
        /** @type {Function|null} 팝업 네비게이션 콜백 */
        this._onPopupNavigate = null;
    }

    /**
     * DBus 연결 초기화
     *
     * InputMethod 프록시 생성 후 InputContext를 생성합니다.
     *
     * @param {string} windowId - 초기 창 식별자
     * @param {Function} [onModeChanged] - 모드 변경 콜백 (isKorean: boolean)
     * @returns {boolean} 연결 성공 여부
     */
    connect(windowId, onModeChanged) {
        this._onModeChanged = onModeChanged || null;

        try {
            // 1. InputMethod 팩토리 프록시 생성
            this._imProxy = Gio.DBusProxy.new_for_bus_sync(
                Gio.BusType.SESSION,
                Gio.DBusProxyFlags.NONE,
                null,
                UNIM_BUS_NAME,
                UNIM_OBJECT_PATH,
                UNIM_IM_INTERFACE,
                null
            );

            if (!this._imProxy) {
                unimError('DBUS_IME', 'InputMethod 프록시 생성 실패');
                return false;
            }

            // GlobalModeChanged 시그널 수신
            this._imSignalId = this._imProxy.connect('g-signal',
                (proxy, senderName, signalName, parameters) => {
                    if (signalName === 'GlobalModeChanged' && this._onModeChanged) {
                        const [isKorean] = parameters.deep_unpack();
                        this._onModeChanged(isKorean);
                    }
                }
            );

            // 2. InputContext 생성
            this._createContext(windowId);

            this._connected = true;
            unimLog('DBUS_IME', `연결 완료 (context: ${this._contextPath})`);
            return true;
        } catch (e) {
            unimError('DBUS_IME', `연결 실패: ${e.message}`);
            this._connected = false;
            return false;
        }
    }

    /**
     * 팝업 콜백 등록
     * @param {object} callbacks
     * @param {Function} [callbacks.onShowHanja] - (target, candidates, cursorRect)
     * @param {Function} [callbacks.onShowSpecial] - (target, characters, topRow, cursorRect)
     * @param {Function} [callbacks.onHidePopup] - ()
     * @param {Function} [callbacks.onPopupNavigate] - (page, totalPages, selected, rows, cols, selRow, selCol)
     */
    setPopupCallbacks(callbacks) {
        this._onShowHanja = callbacks.onShowHanja || null;
        this._onShowSpecial = callbacks.onShowSpecial || null;
        this._onHidePopup = callbacks.onHidePopup || null;
        this._onPopupNavigate = callbacks.onPopupNavigate || null;
    }

    /**
     * InputContext 생성
     * @param {string} windowId - 창 식별자
     * @private
     */
    _createContext(windowId) {
        const result = this._imProxy.call_sync(
            'CreateInputContext',
            new GLib.Variant('(ss)', ['gnome-extension', windowId || '']),
            Gio.DBusCallFlags.NONE,
            DBUS_TIMEOUT_MS,
            null
        );

        if (!result) {
            throw new Error('CreateInputContext 응답 없음');
        }

        const [path] = result.deep_unpack();
        this._contextPath = path;

        // InputContext 프록시 생성
        this._icProxy = Gio.DBusProxy.new_for_bus_sync(
            Gio.BusType.SESSION,
            Gio.DBusProxyFlags.NONE,
            null,
            UNIM_BUS_NAME,
            this._contextPath,
            UNIM_IC_INTERFACE,
            null
        );

        if (!this._icProxy) {
            throw new Error(`InputContext 프록시 생성 실패: ${this._contextPath}`);
        }

        // InputContext 시그널 구독 (ShowHanjaPopup, ShowSpecialPopup, HidePopup)
        this._icSignalId = this._icProxy.connect('g-signal',
            (proxy, senderName, signalName, parameters) => {
                this._handleContextSignal(signalName, parameters);
            }
        );
    }

    /**
     * InputContext 시그널 처리
     * @param {string} signalName
     * @param {GLib.Variant} parameters
     * @private
     */
    _handleContextSignal(signalName, parameters) {
        try {
            if (signalName === 'ShowHanjaPopup' && this._onShowHanja) {
                const [target, candidatesRaw, cx, cy, cw, ch] = parameters.deep_unpack();
                const candidates = candidatesRaw.map(([hanja, meaning]) => ({
                    hanja, meaning,
                }));
                this._onShowHanja(target, candidates, { x: cx, y: cy, width: cw, height: ch });
            } else if (signalName === 'ShowSpecialPopup' && this._onShowSpecial) {
                const [target, characters, topRow, cx, cy, cw, ch] = parameters.deep_unpack();
                this._onShowSpecial(target, characters, topRow, { x: cx, y: cy, width: cw, height: ch });
            } else if (signalName === 'HidePopup' && this._onHidePopup) {
                this._onHidePopup();
            } else if (signalName === 'PopupNavigate' && this._onPopupNavigate) {
                const [page, totalPages, selected, rows, cols, selRow, selCol] = parameters.deep_unpack();
                this._onPopupNavigate(page, totalPages, selected, rows, cols, selRow, selCol);
            }
        } catch (e) {
            unimError('DBUS_IME', `시그널 처리 오류 (${signalName}): ${e.message}`);
        }
    }

    /**
     * 연결 상태 확인
     * @returns {boolean}
     */
    get isConnected() {
        return this._connected && this._icProxy !== null;
    }

    // ===========================================
    // 키 이벤트 처리
    // ===========================================

    /**
     * 키 이벤트를 엔진으로 전달
     *
     * @param {number} keyval - 키 심볼 (GDK keyval)
     * @param {number} keycode - evdev 키코드
     * @param {number} state - 수정자 비트필드
     * @returns {{consumed: boolean, preedit: string, commit: string}|null} 처리 결과
     */
    processKey(keyval, keycode, state) {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'ProcessKeyEvent',
                new GLib.Variant('(uuu)', [keyval, keycode, state]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [consumed, preedit, commit] = result.deep_unpack();
            return { consumed, preedit, commit };
        } catch (e) {
            unimError('DBUS_IME', `ProcessKey 실패: ${e.message}`);
            return null;
        }
    }

    // ===========================================
    // 포커스 관리
    // ===========================================

    /**
     * 포커스 획득 알림
     * @param {string} windowId - 창 식별자
     */
    focusIn(windowId) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'FocusIn',
                new GLib.Variant('(s)', [windowId || '']),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `FocusIn 실패: ${e.message}`);
        }
    }

    /**
     * 포커스 상실 알림
     * @returns {string} 커밋된 텍스트 (조합 중이던 문자열)
     */
    focusOut() {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'FocusOut',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [commit] = result.deep_unpack();
            return commit || '';
        } catch (e) {
            unimError('DBUS_IME', `FocusOut 실패: ${e.message}`);
            return '';
        }
    }

    /**
     * 입력 상태 초기화
     */
    reset() {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'Reset',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `Reset 실패: ${e.message}`);
        }
    }

    /**
     * 커서 위치 보고
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     */
    reportCursorRect(x, y, width, height) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'ReportCursorRect',
                new GLib.Variant('(iiii)', [x, y, width, height]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `ReportCursorRect 실패: ${e.message}`);
        }
    }


    // ===========================================
    // 한자 변환
    // ===========================================

    /**
     * 한자 후보 목록 조회
     *
     * @returns {{target: string, candidates: Array<{hanja: string, meaning: string}>}|null}
     */
    getHanjaCandidates() {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'GetHanjaCandidates',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [target, candidatesRaw] = result.deep_unpack();
            const candidates = candidatesRaw.map(([hanja, meaning]) => ({
                hanja, meaning,
            }));

            return { target, candidates };
        } catch (e) {
            unimError('DBUS_IME', `GetHanjaCandidates 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * 한자 선택
     * @param {number} index - 후보 인덱스
     * @returns {string} 선택된 한자 (실패 시 빈 문자열)
     */
    selectHanja(index) {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'SelectHanja',
                new GLib.Variant('(u)', [index]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [hanja] = result.deep_unpack();
            return hanja || '';
        } catch (e) {
            unimError('DBUS_IME', `SelectHanja 실패: ${e.message}`);
            return '';
        }
    }

    /**
     * 한자 모드 취소
     */
    cancelHanja() {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'CancelHanja',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `CancelHanja 실패: ${e.message}`);
        }
    }

    // ===========================================
    // 특수문자 변환
    // ===========================================

    /**
     * 특수문자 후보 목록 조회
     *
     * @returns {{target: string, characters: string[], topRow: string}|null}
     */
    getSpecialCharCandidates() {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'GetSpecialCharCandidates',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [target, characters, topRow] = result.deep_unpack();
            return { target, characters, topRow };
        } catch (e) {
            unimError('DBUS_IME', `GetSpecialCharCandidates 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * 특수문자 선택
     * @param {number} index - 후보 인덱스
     * @returns {string} 선택된 특수문자 (실패 시 빈 문자열)
     */
    selectSpecialChar(index) {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'SelectSpecialChar',
                new GLib.Variant('(u)', [index]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [ch] = result.deep_unpack();
            return ch || '';
        } catch (e) {
            unimError('DBUS_IME', `SelectSpecialChar 실패: ${e.message}`);
            return '';
        }
    }

    /**
     * 특수문자 모드 취소
     */
    cancelSpecialChar() {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'CancelSpecialChar',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `CancelSpecialChar 실패: ${e.message}`);
        }
    }

    // ===========================================
    // 설정 조회
    // ===========================================

    /**
     * 설정 값 조회
     * @param {string} key - 설정 키
     * @returns {string} 설정 값 (실패 시 빈 문자열)
     */
    getConfig(key) {
        if (!this._imProxy) return '';

        try {
            const result = this._imProxy.call_sync(
                'GetConfig',
                new GLib.Variant('(s)', [key]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [value] = result.deep_unpack();
            return value || '';
        } catch (e) {
            unimError('DBUS_IME', `GetConfig(${key}) 실패: ${e.message}`);
            return '';
        }
    }

    // ===========================================
    // 정리
    // ===========================================

    /**
     * 모든 자원 정리
     *
     * InputContext 파괴 → 프록시 해제 → 시그널 해제
     */
    destroy() {
        // InputContext 파괴
        if (this._icProxy) {
            try {
                this._icProxy.call_sync(
                    'Destroy',
                    null,
                    Gio.DBusCallFlags.NONE,
                    DBUS_TIMEOUT_MS,
                    null
                );
            } catch (e) {
                // 데몬이 이미 종료된 경우 무시
            }
            this._icProxy = null;
        }
        this._contextPath = null;

        // InputMethod 프록시 정리
        if (this._imProxy) {
            if (this._imSignalId > 0) {
                this._imProxy.disconnect(this._imSignalId);
                this._imSignalId = 0;
            }
            this._imProxy = null;
        }

        this._connected = false;
        this._onModeChanged = null;
        this._onShowHanja = null;
        this._onShowSpecial = null;
        this._onHidePopup = null;

        unimLog('DBUS_IME', '자원 정리 완료');
    }
}
