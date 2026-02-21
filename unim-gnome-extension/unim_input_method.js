/**
 * UNIM Input Method — Clutter.InputMethod 서브클래스
 *
 * Clutter Backend에 등록되어 Wayland 텍스트 입력 키를
 * vfunc_filter_key_event를 통해 직접 가로챗니다.
 *
 * @module unim_input_method
 */

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import { unimLog, unimError } from './logging.js';

/**
 * UnimInputMethod
 *
 * Clutter.InputMethod를 상속하여 vfunc_filter_key_event를 구현합니다.
 * Backend에 등록되면 모든 Wayland 텍스트 입력 키가 이 함수를 통과합니다.
 */
export const UnimInputMethod = GObject.registerClass(
class UnimInputMethod extends Clutter.InputMethod {
    _init() {
        super._init();

        /** @type {string} 현재 preedit 텍스트 */
        this._preeditText = '';
        /** @type {boolean} IME 활성 상태 */
        this._active = false;
        /** @type {boolean} 포커스 상태 */
        this._hasFocus = false;
        /** @type {Function|null} 키 핸들러 콜백 (KeyHandler에서 설정) */
        this._keyHandler = null;
        /** @type {Function|null} 포커스 상실 콜백 */
        this._focusOutHandler = null;
        /** @type {{x: number, y: number, width: number, height: number}} 커서 위치 */
        this._cursorRect = { x: 0, y: 0, width: 0, height: 0 };

        unimLog('IME', 'UnimInputMethod 인스턴스 생성');
    }

    /**
     * KeyHandler의 키 처리 콜백 등록
     * @param {Function|null} handler - (keyval, keycode, state) => boolean
     */
    setKeyHandler(handler) {
        this._keyHandler = handler;
    }

    /**
     * 포커스 상실 콜백 등록
     * @param {Function|null} handler - () => void
     */
    setFocusOutHandler(handler) {
        this._focusOutHandler = handler;
    }

    // ===========================================
    // GObject vfunc 오버라이드 — Mutter C vtable에 등록됨
    // ===========================================

    /**
     * Mutter가 텍스트 입력 키를 전달할 때 호출 (C vtable 경유)
     *
     * @param {Clutter.Event} event
     * @returns {boolean} true면 키 소비
     */
    vfunc_filter_key_event(event) {
        if (!event) return false;
        if (!this._active) return false;

        try {
            if (event.type() === Clutter.EventType.KEY_PRESS && this._keyHandler) {
                const keyval = event.get_key_symbol();
                const keycode = event.get_key_code();
                const state = event.get_state();
                const evdevKeycode = keycode > 8 ? keycode - 8 : 0;

                const consumed = this._keyHandler(keyval, evdevKeycode, state);
                if (consumed) return true;
            }
        } catch (e) {
            unimError('IME', `vfunc_filter_key_event 오류: ${e.message}`);
        }

        return false;
    }

    vfunc_focus_in(_focus) {
        this._hasFocus = true;
    }

    vfunc_focus_out() {
        this._hasFocus = false;
        // 포커스 상실 콜백 호출 (조합 중 텍스트 커밋 + 팝업 닫기)
        if (this._focusOutHandler) {
            try {
                this._focusOutHandler();
            } catch (e) {
                unimError('IME', `focusOut 핸들러 오류: ${e.message}`);
            }
        }
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }
    }

    vfunc_reset() {
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }
    }

    vfunc_set_cursor_location(rect) {
        if (rect) {
            this._cursorRect = {
                x: rect.get_x(),
                y: rect.get_y(),
                width: rect.get_width(),
                height: rect.get_height(),
            };
        }
    }

    vfunc_set_surrounding(text, cursor, anchor) {
        // 주변 텍스트 정보
    }

    vfunc_update_content_hints(hints) {
        // 콘텐츠 힌트
    }

    vfunc_update_content_purpose(purpose) {
        // 콘텐츠 목적
    }

    // ===========================================
    // 공개 API — KeyHandler 등에서 호출
    // ===========================================

    /**
     * 최종 텍스트를 포커스된 앱에 커밋
     * @param {string} text - 커밋할 텍스트
     */
    commitText(text) {
        if (!text || text.length === 0) return;

        // preedit이 남아있으면 먼저 클리어
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }

        try {
            this.commit(text);
        } catch (e) {
            unimError('IME', `텍스트 커밋 실패: ${e.message}`);
        }
    }

    /**
     * 조합 중 텍스트를 앱에 표시 (preedit)
     * @param {string} text - preedit 텍스트 (빈 문자열이면 클리어)
     */
    updatePreedit(text) {
        this._preeditText = text || '';

        try {
            if (this._preeditText.length > 0) {
                this.set_preedit_text(
                    this._preeditText,
                    this._preeditText.length,
                    this._preeditText.length,
                    Clutter.PreeditResetMode.CLEAR
                );
            } else {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
            }
        } catch (e) {
            unimError('IME', `preedit 업데이트 실패: ${e.message}`);
        }
    }

    /**
     * Preedit 상태 초기화
     */
    clearPreedit() {
        this._preeditText = '';
        try {
            this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
        } catch (e) {
            unimError('IME', `preedit 초기화 실패: ${e.message}`);
        }
    }

    /**
     * IME 활성/비활성 토글
     * @param {boolean} active
     */
    setActive(active) {
        this._active = active;
        unimLog('IME', `IME ${active ? '활성화' : '비활성화'}`);
    }

    /**
     * IME 활성 상태 확인
     * @returns {boolean}
     */
    get isActive() {
        return this._active;
    }

    /**
     * 현재 커서 위치 (rect)
     * @returns {{x: number, y: number, width: number, height: number}}
     */
    get cursorRect() {
        return this._cursorRect;
    }
});
