/**
 * UNIM Input Method — Clutter.InputMethod 서브클래스
 *
 * GNOME Shell의 내장 InputMethod 인프라를 활용하여
 * 네이티브 텍스트 커밋과 preedit 표시를 구현합니다.
 *
 * IBus를 대체하여 unim-daemon과 직접 통신합니다.
 *
 * @module unim_input_method
 */

import GObject from 'gi://GObject';
import Clutter from 'gi://Clutter';
import { unimLog, unimError } from './logging.js';

/**
 * UnimInputMethod
 *
 * Clutter.InputMethod를 상속하여 GNOME Shell의 IM 인프라에 통합합니다.
 * - commitText(text): 최종 텍스트를 포커스된 앱에 전달
 * - updatePreedit(text): 조합 중 텍스트를 앱에 표시 (밑줄 preedit)
 * - clearPreedit(): preedit 상태 초기화
 *
 * GNOME Shell은 Mutter의 text-input-v3 프로토콜을 통해
 * 이 메서드들의 결과를 Wayland 클라이언트에 전달합니다.
 */
export const UnimInputMethod = GObject.registerClass({
    GTypeName: 'UnimInputMethod',
}, class UnimInputMethod extends Clutter.InputMethod {

    _init() {
        super._init();

        /** @type {string} 현재 preedit 텍스트 */
        this._preeditText = '';
        /** @type {boolean} IME 활성 상태 */
        this._active = false;
        /** @type {boolean} 포커스 보유 여부 */
        this._hasFocus = false;

        unimLog('IME', 'UnimInputMethod 인스턴스 생성');
    }

    // ===========================================
    // 공개 API — KeyHandler에서 호출
    // ===========================================

    /**
     * 최종 텍스트를 포커스된 앱에 커밋
     *
     * Mutter가 text-input-v3 commit_string으로 전달합니다.
     * Wayland에서 keymap에 없는 유니코드(한글 등)도 정상 전달됩니다.
     *
     * @param {string} text - 커밋할 텍스트
     */
    commitText(text) {
        if (!text || text.length === 0) return;
        if (!this._hasFocus) {
            unimLog('IME', `commitText 호출되었으나 포커스 없음: "${text}"`);
            return;
        }

        // preedit이 남아있으면 먼저 클리어
        if (this._preeditText.length > 0) {
            this._preeditText = '';
            this.set_preedit_text(null, 0, 0);
        }

        this.commit(text);
        unimLog('IME', `commitText: "${text}"`);
    }

    /**
     * 조합 중 텍스트를 앱에 표시 (preedit)
     *
     * 앱의 입력 필드에 밑줄과 함께 표시됩니다.
     * text-input-v3 프로토콜을 지원하는 앱에서 동작합니다.
     *
     * @param {string} text - preedit 텍스트 (빈 문자열이면 클리어)
     */
    updatePreedit(text) {
        if (!this._hasFocus) return;

        this._preeditText = text || '';

        if (this._preeditText.length > 0) {
            // Clutter.PreeditResetMode.CLEAR: 포커스 상실 시 preedit 자동 삭제
            this.set_preedit_text(
                this._preeditText,
                this._preeditText.length,  // 커서를 끝에 배치
                Clutter.PreeditResetMode.CLEAR
            );
        } else {
            this.set_preedit_text(null, 0, 0);
        }
    }

    /**
     * Preedit 상태 초기화
     */
    clearPreedit() {
        this._preeditText = '';
        if (this._hasFocus) {
            this.set_preedit_text(null, 0, 0);
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
     * 현재 preedit 텍스트
     * @returns {string}
     */
    get preeditText() {
        return this._preeditText;
    }

    // ===========================================
    // Clutter.InputMethod vfunc 오버라이드
    // ===========================================

    /**
     * 포커스 획득 시 Mutter가 호출
     *
     * 입력 가능한 위젯(text-input 지원)에 커서가 들어갈 때 호출됩니다.
     *
     * @param {Clutter.InputFocus} focus - 포커스 객체
     */
    vfunc_focus_in(focus) {
        this._hasFocus = true;
        unimLog('IME', 'vfunc_focus_in');
        super.vfunc_focus_in(focus);
    }

    /**
     * 포커스 상실 시 Mutter가 호출
     *
     * 남아있는 preedit을 정리합니다.
     */
    vfunc_focus_out() {
        this._hasFocus = false;
        this._preeditText = '';
        unimLog('IME', 'vfunc_focus_out');
        super.vfunc_focus_out();
    }

    /**
     * 키 이벤트 필터링
     *
     * 이 메서드에서 키를 처리하면 앱에 전달되지 않습니다.
     * 현재 구현에서는 KeyHandler가 키 이벤트를 별도로 처리하므로,
     * 여기서는 기본 동작(통과)을 유지합니다.
     *
     * @param {Clutter.Event} event - 키 이벤트
     * @returns {boolean} true면 소비(앱에 전달 안됨)
     */
    vfunc_filter_key_event(event) {
        // KeyHandler가 global.display key-press-event에서 처리하므로
        // 여기서는 consume하지 않음
        return false;
    }

    /**
     * 입력 상태 리셋 시 Mutter가 호출
     */
    vfunc_reset() {
        this._preeditText = '';
        unimLog('IME', 'vfunc_reset');
        super.vfunc_reset();
    }
});
