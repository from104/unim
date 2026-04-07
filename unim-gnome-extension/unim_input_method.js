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

/** 수정자 키 — IM이 가로채지 않고 Mutter에 직접 전달 (고정키 등 접근성 호환) */
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
        /** @type {Function|null} 리셋 콜백 (팝업 정리 등) */
        this._resetHandler = null;
        /** @type {{x: number, y: number, width: number, height: number}} 커서 위치 */
        this._cursorRect = { x: 0, y: 0, width: 0, height: 0 };
        /** @type {Object|null} DBus IME 클라이언트 연동 */
        this._dbusIME = null;
        /**
         * AutoTypeFix가 vkbd로 보낸 BackSpace의 self-feedback 방지 카운터.
         * vkbd 이벤트가 mutter→IM filter로 재진입할 때, 한글 엔진이 새 preedit을
         * 깎아먹지 않도록 IM 처리를 우회시킨다. PRESS+RELEASE 한 쌍이 1개의 backspace.
         * @type {number}
         */
        this._selfBackspaceCount = 0;

        unimLog('IME', 'UnimInputMethod 인스턴스 생성');
    }

    /**
     * DBus IME 클라이언트 주입
     * @param {Object} dbusIME
     */
    setDbusIME(dbusIME) {
        this._dbusIME = dbusIME;
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

    /**
     * 리셋 콜백 등록 (팝업 정리 등)
     * @param {Function|null} handler - () => void
     */
    setResetHandler(handler) {
        this._resetHandler = handler;
    }

    // ===========================================
    // GObject vfunc 오버라이드 — Mutter C vtable에 등록됨
    // ===========================================

    /**
     * Mutter가 텍스트 입력 키를 전달할 때 호출 (C vtable 경유)
     *
     * IBus와 동일한 패턴: 항상 true를 반환하여 키를 소비하고,
     * 처리 후 notify_key_event(event, consumed)로 키 전달 여부를 사후 통보.
     *
     * @param {Clutter.Event} event
     * @returns {boolean} 항상 true (IBus 패턴)
     */
    vfunc_filter_key_event(event) {
        if (!event) return false;
        if (!this._active) return false;

        const eventType = event.type();
        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();
        const flags = event.get_flags ? event.get_flags() : 0;
        const typeName = eventType === Clutter.EventType.KEY_PRESS ? 'PRESS' :
                         eventType === Clutter.EventType.KEY_RELEASE ? 'RELEASE' : `OTHER(${eventType})`;

        unimLog('IME', `vfunc_filter: type=${typeName}, keyval=${keyval}, keycode=${keycode}, state=0x${state.toString(16)}, flags=0x${flags.toString(16)}`);

        // AutoTypeFix가 vkbd로 보낸 BackSpace는 IM 처리 없이 mutter에 직접 통과
        // (vkbd → mutter → IM filter 재진입 시 한글 엔진이 새 preedit을 깎는 self-feedback 차단)
        // PRESS와 RELEASE를 각각 1씩 차감 (한 backspace = 2 이벤트)
        if (keyval === Clutter.KEY_BackSpace && this._selfBackspaceCount > 0) {
            this._selfBackspaceCount--;
            unimLog('IME', `self-sent BackSpace 우회 (${typeName}, 남은=${this._selfBackspaceCount})`);
            return false;
        }

        // 수정자 키 단독 입력은 IM이 가로채지 않음 (return false)
        // → Mutter가 직접 처리하여 고정키(Sticky Keys) 등 접근성 기능 정상 작동
        // (return true + notify_key_event(false) 패턴은 FLAG_INPUT_METHOD 플래그를
        //  부착하여 고정키 핸들러가 이벤트를 무시하는 원인이 됨)
        if (MODIFIER_KEYSYMS.has(keyval)) {
            return false;
        }

        // KEY_RELEASE: 처리하지 않지만 notify_key_event는 반드시 호출
        // (Mutter의 키 상태 추적 유지 — 누락 시 키 반복이 멈추지 않음)
        if (eventType !== Clutter.EventType.KEY_PRESS) {
            this.notify_key_event(event, false);
            return true;
        }

        // 키 핸들러가 직접 notify_key_event를 호출
        // (키 큐 패턴: call_sync 중 재진입 키를 큐에 저장 후 순차 처리)
        if (this._keyHandler) {
            const evdevKeycode = keycode > 8 ? keycode - 8 : 0;
            try {
                this._keyHandler(keyval, evdevKeycode, state, event);
            } catch (e) {
                unimError('IME', `vfunc_filter_key_event 오류: ${e.message}`);
                this.notify_key_event(event, false);
            }
        } else {
            this.notify_key_event(event, false);
        }

        return true;
    }

    vfunc_focus_in(_focus) {
        this._hasFocus = true;
        unimLog('IME', `vfunc_focus_in: _hasFocus=true`);
    }

    vfunc_focus_out() {
        unimLog('IME', `vfunc_focus_out 호출됨 (stack: popup commit 디버깅)`);
        // 1. 커밋 최우선: 로컬 preedit 백업
        const localPreedit = this._preeditText;

        this._hasFocus = false;

        // 2. 포커스 상실 콜백 호출 (DBus FocusOut → 커밋 텍스트 수신)
        let committed = false;
        if (this._focusOutHandler) {
            try {
                committed = this._focusOutHandler();
            } catch (e) {
                unimError('IME', `focusOut 핸들러 오류: ${e.message}`);
            }
        }

        // 3. DBus 커밋 실패 시 로컬 preedit 폴백
        if (!committed && localPreedit && localPreedit.length > 0) {
            this._preeditText = '';
            try {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
                this.commit(localPreedit);
            } catch (e) {
                unimError('IME', `focusOut 로컬 커밋 실패: ${e.message}`);
            }
        } else if (this._preeditText.length > 0) {
            this.clearPreedit();
        }
    }

    vfunc_reset() {
        // 1. 리셋 핸들러 호출 (팝업 열려있으면 커밋+닫기)
        if (this._resetHandler) {
            try {
                this._resetHandler();
            } catch (e) {
                unimError('IME', `reset 핸들러 오류: ${e.message}`);
            }
        }

        // 2. preedit 커밋
        const preedit = this._preeditText;
        if (preedit && preedit.length > 0) {
            this._preeditText = '';
            try {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
                this.commit(preedit);
            } catch (e) {
                unimError('IME', `vfunc_reset 커밋 실패: ${e.message}`);
            }
            // 엔진 상태 초기화
            if (this._dbusIME) {
                this._dbusIME.reset();
            }
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
            
            if (this._dbusIME) {
                this._dbusIME.reportCursorRect(
                    Math.round(this._cursorRect.x),
                    Math.round(this._cursorRect.y),
                    Math.round(this._cursorRect.width),
                    Math.round(this._cursorRect.height)
                );
            }
        }
    }

    vfunc_set_surrounding(text, cursor, anchor) {
        if (this._dbusIME?.isConnected) {
            this._dbusIME.setSurroundingText(text || '', cursor || 0, anchor || 0);
        }
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

        unimLog('IME', `commitText: text='${text}', _hasFocus=${this._hasFocus}, _preeditText='${this._preeditText}'`);

        // preedit이 남아있으면 먼저 클리어
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }

        try {
            this.commit(text);
            unimLog('IME', `commitText: commit() 호출 완료 (에러 없음)`);
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
     * AutoTypeFix가 vkbd로 BackSpace를 N번 보내기 직전에 호출.
     * 그 키 이벤트들이 mutter→IM filter로 재진입할 때 unim 엔진이 처리하지 않고
     * 그대로 포커스된 앱으로 통과시키도록 카운터를 설정한다.
     * @param {number} count - 보낼 backspace 개수
     */
    expectSelfBackspaces(count) {
        if (count <= 0) return;
        // 한 backspace = PRESS + RELEASE 두 이벤트
        this._selfBackspaceCount += count * 2;
        unimLog('IME', `expectSelfBackspaces: +${count} (대기 이벤트=${this._selfBackspaceCount})`);
    }

    /**
     * 커서 앞의 텍스트를 삭제 (TypeFIX용)
     * @param {number} charCount - 삭제할 글자 수
     */
    deleteSurrounding(charCount) {
        if (charCount <= 0) return;
        try {
            this.delete_surrounding(-(charCount), charCount);
            unimLog('IME', `deleteSurrounding: ${charCount}자 삭제`);
        } catch (e) {
            unimError('IME', `deleteSurrounding 실패: ${e.message}`);
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
