/**
 * UNIM Preedit 오버레이 (폴백)
 *
 * Clutter.InputMethod.set_preedit_text()가 앱에서 지원되지 않을 때
 * 화면에 조합 중 텍스트를 플로팅 오버레이로 표시합니다.
 *
 * text-input-v3 프로토콜을 지원하는 앱에서는 네이티브 preedit이
 * 우선 동작하므로, 이 오버레이는 터미널·게임 등의 폴백용입니다.
 *
 * @module preedit_overlay
 */

import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

/**
 * PreeditOverlay
 *
 * 커서 근처에 조합 중 글자를 표시하는 플로팅 라벨
 */
export class PreeditOverlay {
    constructor() {
        this._label = null;
        this._visible = false;
    }

    /**
     * 오버레이 위젯 초기화
     */
    enable() {
        this._label = new St.Label({
            style_class: 'unim-preedit-overlay',
            text: '',
            visible: false,
        });

        // Chrome으로 등록하여 모든 창 위에 표시
        Main.layoutManager.addChrome(this._label, {
            affectsStruts: false,
            trackFullscreen: false,
        });

        unimLog('PREEDIT', '오버레이 초기화');
    }

    /**
     * Preedit 텍스트 업데이트
     *
     * @param {string} text - 조합 중 텍스트 (빈 문자열이면 숨김)
     * @param {number} [x] - 화면 X 좌표 (생략 시 현재 위치 유지)
     * @param {number} [y] - 화면 Y 좌표 (생략 시 현재 위치 유지)
     */
    update(text, x, y) {
        if (!this._label) return;

        if (!text || text.length === 0) {
            this.hide();
            return;
        }

        this._label.set_text(text);

        // 위치 지정
        if (x !== undefined && y !== undefined) {
            this._label.set_position(x, y);
        }

        if (!this._visible) {
            this._label.show();
            this._visible = true;
        }
    }

    /**
     * 오버레이 숨김
     */
    hide() {
        if (this._label && this._visible) {
            this._label.hide();
            this._label.set_text('');
            this._visible = false;
        }
    }

    /**
     * 자원 정리
     */
    disable() {
        this.hide();
        if (this._label) {
            Main.layoutManager.removeChrome(this._label);
            this._label.destroy();
            this._label = null;
        }
        unimLog('PREEDIT', '오버레이 정리');
    }
}
