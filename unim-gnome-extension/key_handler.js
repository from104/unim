/**
 * UNIM 전역 키 이벤트 핸들러
 *
 * global.display의 key-press-event를 가로채어
 * DBus → UnimInputMethod으로 라우팅합니다.
 *
 * @module key_handler
 */

import Clutter from 'gi://Clutter';
import { unimLog, unimError } from './logging.js';

/** 바이패스할 수정자 키들 */
const MODIFIER_KEYSYMS = new Set([
    Clutter.KEY_Shift_L, Clutter.KEY_Shift_R,
    Clutter.KEY_Control_L, Clutter.KEY_Control_R,
    Clutter.KEY_Alt_L, Clutter.KEY_Alt_R,
    Clutter.KEY_Super_L, Clutter.KEY_Super_R,
    Clutter.KEY_Meta_L, Clutter.KEY_Meta_R,
    Clutter.KEY_Hyper_L, Clutter.KEY_Hyper_R,
    Clutter.KEY_Caps_Lock, Clutter.KEY_Num_Lock,
    Clutter.KEY_Scroll_Lock,
]);

/** 바이패스할 기능키 범위 (Ctrl/Alt 조합포함) */
const BYPASS_MODIFIER_MASK =
    Clutter.ModifierType.CONTROL_MASK |
    Clutter.ModifierType.MOD1_MASK |  // Alt
    Clutter.ModifierType.SUPER_MASK;

/**
 * KeyHandler
 *
 * 전역 키 이벤트를 인터셉트하여 UNIM 엔진으로 전달합니다.
 */
export class KeyHandler {
    /**
     * @param {import('./dbus_ime.js').UnimDbusIME} dbusIME - DBus 클라이언트
     * @param {import('./unim_input_method.js').UnimInputMethod} inputMethod - IM 서브클래스
     * @param {object} options
     * @param {Function} [options.onHanjaRequest] - 한자키 콜백
     * @param {number[]} [options.hanjaKeysyms] - 한자키 keysym 목록
     */
    constructor(dbusIME, inputMethod, options = {}) {
        /** @type {import('./dbus_ime.js').UnimDbusIME} */
        this._dbusIME = dbusIME;
        /** @type {import('./unim_input_method.js').UnimInputMethod} */
        this._inputMethod = inputMethod;

        /** @type {Function|null} 한자키 콜백 */
        this._onHanjaRequest = options.onHanjaRequest || null;

        /** @type {Set<number>} 한자키 keysym 집합 */
        this._hanjaKeysyms = new Set(options.hanjaKeysyms || [
            Clutter.KEY_Hangul_Hanja,  // 한자 전용 키
            Clutter.KEY_F9,            // F9 (기본)
        ]);

        /** @type {number} key-press-event 시그널 ID */
        this._keyPressId = 0;
        /** @type {number} key-release-event 시그널 ID */
        this._keyReleaseId = 0;
        /** @type {boolean} 팝업 활성 상태 (한자/특수문자 팝업이 키를 소비할 때) */
        this._popupActive = false;
        /** @type {Function|null} 팝업 키 핸들러 (팝업에서 설정) */
        this._popupKeyHandler = null;
        /** @type {boolean} IME 활성 상태 */
        this._enabled = false;
    }

    /**
     * 전역 키 인터셉션 시작
     */
    enable() {
        if (this._keyPressId > 0) return;

        this._keyPressId = global.display.connect(
            'key-press-event',
            this._onKeyPress.bind(this)
        );

        this._enabled = true;
        unimLog('KEY', '전역 키 인터셉션 시작');
    }

    /**
     * 전역 키 인터셉션 중지
     */
    disable() {
        if (this._keyPressId > 0) {
            global.display.disconnect(this._keyPressId);
            this._keyPressId = 0;
        }

        this._enabled = false;
        this._popupActive = false;
        this._popupKeyHandler = null;
        unimLog('KEY', '전역 키 인터셉션 중지');
    }

    /**
     * 팝업 키 핸들러 설정
     *
     * 팝업(한자/특수문자)이 활성화되면 모든 키가 팝업으로 전달됩니다.
     *
     * @param {Function|null} handler - 키 핸들러 (keyval) => boolean
     */
    setPopupKeyHandler(handler) {
        this._popupActive = handler !== null;
        this._popupKeyHandler = handler;
    }

    /**
     * 한자키 keysym 목록 갱신
     * @param {number[]} keysyms
     */
    updateHanjaKeysyms(keysyms) {
        this._hanjaKeysyms = new Set(keysyms);
    }

    // ===========================================
    // 내부 — 키 이벤트 처리
    // ===========================================

    /**
     * key-press-event 핸들러
     *
     * @param {Meta.Display} display
     * @param {Clutter.Event} event
     * @returns {number} Clutter.EVENT_STOP 또는 Clutter.EVENT_PROPAGATE
     * @private
     */
    _onKeyPress(display, event) {
        try {
            return this._handleKeyPress(event);
        } catch (e) {
            // 안전장치: 에러 시 키 이벤트를 앱에 전달
            unimError('KEY', `키 처리 오류: ${e.message}`);
            return Clutter.EVENT_PROPAGATE;
        }
    }

    /**
     * 실제 키 처리 로직 (에러 핸들링 분리)
     *
     * @param {Clutter.Event} event
     * @returns {number}
     * @private
     */
    _handleKeyPress(event) {
        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();

        // 1. 수정자 키 단독 입력 → 바이패스
        if (MODIFIER_KEYSYMS.has(keyval)) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 2. 팝업 활성 → 팝업에 위임
        if (this._popupActive && this._popupKeyHandler) {
            const consumed = this._popupKeyHandler(keyval, state);
            return consumed ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
        }

        // 3. Ctrl/Alt/Super 조합 → 바이패스 (단축키 등)
        if (state & BYPASS_MODIFIER_MASK) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 4. 한자키 감지
        if (this._hanjaKeysyms.has(keyval)) {
            if (this._onHanjaRequest) {
                this._onHanjaRequest();
            }
            return Clutter.EVENT_STOP;
        }

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 6. ProcessKeyEvent 호출
        const result = this._dbusIME.processKey(keyval, keycode, state);

        if (!result) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 7. 결과 처리
        const { consumed, preedit, commit } = result;

        // commit 텍스트가 있으면 앱에 커밋
        if (commit && commit.length > 0) {
            this._inputMethod.commitText(commit);
        }

        // preedit 업데이트
        this._inputMethod.updatePreedit(preedit || '');

        // consumed=true면 키를 소비, false면 앱에 전달
        return consumed ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
    }

    /**
     * 자원 정리
     */
    destroy() {
        this.disable();
        this._dbusIME = null;
        this._inputMethod = null;
        this._onHanjaRequest = null;
    }
}
