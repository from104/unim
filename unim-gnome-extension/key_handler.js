/**
 * UNIM 전역 키 이벤트 핸들러
 *
 * Clutter Backend에 커스텀 InputMethod를 등록하여
 * Wayland 텍스트 입력 키를 가로채고,
 * DBus를 통해 UNIM 엔진으로 라우팅합니다.
 *
 * @module key_handler
 */

import Clutter from 'gi://Clutter';
import { unimLog, unimError } from './logging.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
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

        /** @type {number} captured-event 시그널 ID */
        this._keyPressId = 0;
        /** @type {boolean} IME 활성 상태 */
        this._enabled = false;

        /** @type {Clutter.InputMethod|null} 원래 IM (복원용) */
        this._savedInputMethod = null;
        /** @type {boolean} Backend 등록 여부 */
        this._backendRegistered = false;
        /** @type {boolean} 키 처리 중 재진입 방지 플래그 */
        this._processingKey = false;
        /** @type {Array<{keyval: number, keycode: number, state: number, event: Clutter.Event}>} 재진입 키 큐 */
        this._keyQueue = [];
    }

    /**
     * 전역 키 인터셉션 시작
     *
     * 전략:
     * A) UnimInputMethod (Clutter.InputMethod 서브클래스)를 Backend에 등록
     *    → vfunc_filter_key_event가 C vtable에 올바르게 바인딩됨
     *    → Mutter가 모든 Wayland 텍스트 입력 키를 우리 vfunc으로 전달
     * B) global.stage captured-event (폴백: 셸 UI 등)
     */
    enable() {
        if (this._enabled) return;

        // 1. Backend에 커스텀 IM 등록 (Wayland 텍스트 입력 가로채기)
        this._registerWithBackend();

        // 2. 폴백 (Backend 미등록 시 또는 Shell UI용)
        this._keyPressId = global.stage.connect(
            'captured-event',
            (actor, event) => {
                if (event.type() !== Clutter.EventType.KEY_PRESS) {
                    return Clutter.EVENT_PROPAGATE;
                }
                // Backend 등록 완료 시 vfunc이 이미 처리하므로 스킵
                // (이중 입력 방지)
                if (this._backendRegistered) {
                    return Clutter.EVENT_PROPAGATE;
                }
                return this._onKeyPress(actor, event);
            }
        );

        this._enabled = true;
        unimLog('KEY', `전역 키 인터셉션 시작 완료 (backend-im=${this._backendRegistered ? '등록됨' : '미등록'}, captured-event=활성)`);
    }

    /**
     * 커스텀 UnimInputMethod를 Clutter Backend에 등록
     * API: clutter_backend_set_input_method(backend, method)
     *
     * GObject.registerClass()로 등록된 서브클래스의 vfunc은
     * C vtable에 올바르게 등록되어, Mutter C 코드에서 호출됩니다.
     * @private
     */
    _registerWithBackend() {
        try {
            const backend = Clutter.get_default_backend();

            if (typeof backend.set_input_method !== 'function') {
                unimLog('KEY', '⚠️ backend.set_input_method가 함수가 아닙니다.');
                return;
            }

            // 키 핸들러 콜백 등록 (vfunc_filter_key_event에서 호출됨)
            // event를 전달받아 직접 notify_key_event를 호출 (비동기 키 큐 지원)
            this._inputMethod.setKeyHandler((keyval, keycode, state, event) => {
                this._handleVfuncKey(keyval, keycode, state, event);
            });

            // 현재 IM 저장 (disable 시 복원용)
            this._savedInputMethod = Main.inputMethod;

            // Backend에 우리 IM 등록
            backend.set_input_method(this._inputMethod);
            this._backendRegistered = true;

            unimLog('KEY', 'UnimInputMethod를 Clutter Backend에 등록 완료');
        } catch (e) {
            unimError('KEY', `Backend 등록 실패: ${e.message}`);
        }
    }

    /**
     * vfunc_filter_key_event에서 호출되는 키 처리 콜백
     *
     * 키 핸들러가 직접 notify_key_event를 호출합니다.
     * call_sync() 중 GLib 재진입으로 도착한 키는 큐에 저장 후 순차 처리합니다.
     *
     * @param {number} keyval - X11 keysym
     * @param {number} keycode - evdev keycode
     * @param {number} state - modifier state
     * @param {Clutter.Event} event - 원본 키 이벤트 (notify_key_event용)
     * @private
     */
    _handleVfuncKey(keyval, keycode, state, event) {
        // 0. 재진입 방지: call_sync() 중 GLib 메인 루프가 이벤트를 처리하여
        //    동일 키가 재진입될 수 있음 → 큐에 저장하여 순차 처리 (키 누락 방지)
        if (this._processingKey) {
            try {
                this._keyQueue.push({keyval, keycode, state, event: event.copy()});
            } catch (e) {
                // event.copy() 실패 시 기존 동작 유지 (키 소비)
                unimError('KEY', `이벤트 복사 실패, 키 소비: ${e.message}`);
                this._inputMethod.notify_key_event(event, true);
            }
            return;
        }

        // 1. 수정자 키 단독 → 미소비
        if (MODIFIER_KEYSYMS.has(keyval)) {
            this._inputMethod.notify_key_event(event, false);
            return;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through
        //    engine이 팝업 키를 처리하고 시그널(PopupNavigate, HidePopup)로 UI 갱신

        // 3. Ctrl/Alt/Super 조합 → 키를 먼저 전달한 후 조합 flush
        //    (고정키 사용 시 _flushCompose의 call_sync 중 modifier가 해제되는 것을 방지)
        if (state & BYPASS_MODIFIER_MASK) {
            this._inputMethod.notify_key_event(event, false);
            this._flushCompose();
            return;
        }

        // 4. 한자키는 엔진에 위임 (ProcessKeyEvent를 통해 처리)

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            this._inputMethod.notify_key_event(event, false);
            return;
        }

        // 6. ProcessKeyEvent 호출 (재진입 가드)
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, keycode, state);
        } finally {
            this._processingKey = false;
        }

        if (!result) {
            this._inputMethod.notify_key_event(event, false);
            this._drainKeyQueue();
            return;
        }

        // 7. 결과 처리
        const { consumed, preedit, commit } = result;

        if (commit && commit.length > 0) {
            this._inputMethod.commitText(commit);
        }

        this._inputMethod.updatePreedit(preedit || '');
        this._inputMethod.notify_key_event(event, consumed);

        // 8. 재진입으로 큐에 쌓인 키 순차 처리
        this._drainKeyQueue();
    }

    /**
     * 재진입으로 큐에 쌓인 키를 순차 처리
     *
     * call_sync() 중 GLib 메인 루프 재진입으로 도착한 키를
     * FIFO 순서로 처리하여 키 누락을 방지합니다.
     * @private
     */
    _drainKeyQueue() {
        while (this._keyQueue.length > 0) {
            const entry = this._keyQueue.shift();

            if (MODIFIER_KEYSYMS.has(entry.keyval)) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            if (entry.state & BYPASS_MODIFIER_MASK) {
                this._flushCompose();
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            if (!this._dbusIME.isConnected) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            this._processingKey = true;
            let result;
            try {
                result = this._dbusIME.processKey(entry.keyval, entry.keycode, entry.state);
            } finally {
                this._processingKey = false;
            }

            if (!result) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            const { consumed, preedit, commit } = result;
            if (commit && commit.length > 0) {
                this._inputMethod.commitText(commit);
            }
            this._inputMethod.updatePreedit(preedit || '');
            this._inputMethod.notify_key_event(entry.event, consumed);
        }
    }

    /**
     * 조합 중인 텍스트 flush (Ctrl/Alt 조합 시)
     *
     * 로컬 preedit을 커밋하고 엔진 상태를 초기화합니다.
     * notify_key_event 패턴에서는 키 전달을 Mutter가 처리하므로
     * forward_key 호출이 불필요합니다.
     * @private
     */
    _flushCompose() {
        // 로컬 preedit에서 커밋 텍스트 획득
        const preedit = this._inputMethod._preeditText || '';
        if (preedit.length > 0) {
            this._inputMethod.clearPreedit();
            this._inputMethod.commitText(preedit);
        }
        // 엔진 상태 초기화 (포커스 변경 없이)
        if (this._dbusIME?.isConnected) {
            this._dbusIME.reset();
        }
    }

    /**
     * Backend에서 원래 IM 복원
     * @private
     */
    _unregisterFromBackend() {
        if (this._backendRegistered) {
            try {
                const backend = Clutter.get_default_backend();
                if (this._savedInputMethod) {
                    backend.set_input_method(this._savedInputMethod);
                    unimLog('KEY', '원본 InputMethod 복원 완료');
                } else {
                    backend.set_input_method(null);
                }
                this._inputMethod.setKeyHandler(null);
            } catch (e) {
                unimError('KEY', `Backend 해제 중 오류: ${e.message}`);
            }
        }
        this._savedInputMethod = null;
        this._backendRegistered = false;
    }

    /**
     * 전역 키 인터셉션 중지
     */
    disable() {
        // captured-event 해제
        if (this._keyPressId > 0) {
            global.stage.disconnect(this._keyPressId);
            this._keyPressId = 0;
        }

        // Backend에서 원래 IM 복원
        this._unregisterFromBackend();

        this._enabled = false;
        unimLog('KEY', '전역 키 인터셉션 중지');
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
     * @param {Clutter.Actor} actor
     * @param {Clutter.Event} event
     * @returns {number} Clutter.EVENT_STOP 또는 Clutter.EVENT_PROPAGATE
     * @private
     */
    _onKeyPress(actor, event) {
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
        // 0. 재진입 방지 (call_sync 중 GLib 메인 루프 재진입 차단)
        if (this._processingKey) {
            return Clutter.EVENT_STOP;
        }

        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();

        const evdevKeycode = keycode > 8 ? keycode - 8 : 0;

        // 1. 수정자 키 단독 입력 → 바이패스
        if (MODIFIER_KEYSYMS.has(keyval)) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through (이중 처리 방지)

        // 3. Ctrl/Alt/Super 조합 → 조합 중이면 커밋 후 바이패스
        if (state & BYPASS_MODIFIER_MASK) {
            // captured-event 폴백이므로 forward_key 불필요, PROPAGATE로 키 전달
            const preedit = this._inputMethod._preeditText || '';
            if (preedit.length > 0) {
                this._inputMethod.clearPreedit();
                this._inputMethod.commitText(preedit);
                if (this._dbusIME?.isConnected) this._dbusIME.reset();
            }
            return Clutter.EVENT_PROPAGATE;
        }

        // 4. 한자키는 엔진에 위임 (ProcessKeyEvent를 통해 처리)
        //    엔진이 ShowHanjaPopup 시그널을 발행하면 인디케이터가 팝업 표시

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 6. ProcessKeyEvent 호출 (재진입 가드)
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, evdevKeycode, state);
        } finally {
            this._processingKey = false;
        }

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
        this._keyQueue = [];
        this._dbusIME = null;
        this._inputMethod = null;
        this._onHanjaRequest = null;
    }
}
