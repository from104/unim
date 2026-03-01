/**
 * UNIM 한자 후보 팝업
 *
 * 한자 변환 시 후보 목록을 세로 리스트로 표시합니다.
 * GTK3 unim_hanja_popup.c의 키 처리 로직을 St 위젯으로 이식.
 *
 * @module hanja_popup
 */

import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

/** 페이지당 최대 후보 수 */
const PAGE_SIZE = 9;

/**
 * HanjaPopup
 *
 * 한자 후보 리스트를 플로팅 패널로 표시합니다.
 * 숫자(1-9) 직접 선택, 화살표 네비게이션, 페이지 전환을 지원합니다.
 */
export class HanjaPopup {
    constructor() {
        /** @type {St.BoxLayout|null} 루트 위젯 */
        this._container = null;
        /** @type {St.Label} 헤더 (대상 문자 표시) */
        this._header = null;
        /** @type {St.BoxLayout} 후보 리스트 컨테이너 */
        this._list = null;
        /** @type {St.Label} 페이지 표시 라벨 */
        this._footer = null;
        /** @type {St.Label[]} 후보 행 라벨들 */
        this._rows = [];

        /** @type {Array<{hanja: string, meaning: string}>} 전체 후보 */
        this._candidates = [];
        /** @type {string} 대상 문자 */
        this._target = '';
        /** @type {number} 현재 페이지 (0부터) */
        this._currentPage = 0;
        /** @type {number} 선택 인덱스 (페이지 내, 0부터) */
        this._selectedIndex = 0;

        /** @type {Function|null} 선택 콜백 (index: number) => void */
        this._onSelect = null;
        /** @type {Function|null} 취소 콜백 () => void */
        this._onCancel = null;
    }

    /**
     * 위젯 초기화
     */
    enable() {
        this._container = new St.BoxLayout({
            style_class: 'unim-hanja-popup',
            vertical: true,
            visible: false,
        });

        this._header = new St.Label({ style_class: 'popup-header' });
        this._container.add_child(this._header);

        this._list = new St.BoxLayout({ vertical: true });
        this._container.add_child(this._list);

        this._footer = new St.Label({ style_class: 'popup-footer' });
        this._container.add_child(this._footer);

        Main.layoutManager.addChrome(this._container, {
            affectsStruts: false,
            trackFullscreen: false,
        });
    }

    /**
     * 팝업 표시
     *
     * @param {string} target - 변환 대상 문자
     * @param {Array<{hanja: string, meaning: string}>} candidates - 후보 목록
     * @param {Function} onSelect - 선택 콜백 (globalIndex: number)
     * @param {Function} onCancel - 취소 콜백
     * @param {{x: number, y: number, width: number, height: number}} [cursorRect] - 커서 위치
     */
    show(target, candidates, onSelect, onCancel, cursorRect) {
        if (!this._container || candidates.length === 0) return;

        this._target = target;
        this._candidates = candidates;
        this._currentPage = 0;
        this._selectedIndex = 0;
        this._onSelect = onSelect;
        this._onCancel = onCancel;

        this._header.set_text(`한자: ${target}`);
        this._updateList();

        // 커서 아래에 배치 + 화면 경계 조정
        const monitor = Main.layoutManager.primaryMonitor;
        if (monitor) {
            const popupWidth = 250;
            const popupHeight = 300;
            let x, y;

            if (cursorRect && (cursorRect.x > 0 || cursorRect.y > 0)) {
                // 커서 아래에 배치
                x = cursorRect.x;
                y = cursorRect.y + cursorRect.height + 4;
            } else {
                // 폴백: 화면 중앙 상단
                x = Math.floor(monitor.x + (monitor.width - popupWidth) / 2);
                y = Math.floor(monitor.y + 100);
            }

            // 오른쪽 경계
            if (x + popupWidth > monitor.x + monitor.width) {
                x = monitor.x + monitor.width - popupWidth;
            }
            // 아래 경계 → 커서 위로
            if (y + popupHeight > monitor.y + monitor.height) {
                y = (cursorRect?.y ?? y) - popupHeight - 4;
            }
            // 왼쪽/위쪽 최소
            x = Math.max(monitor.x, x);
            y = Math.max(monitor.y, y);

            this._container.set_position(x, y);
        }

        this._container.show();
        unimLog('HANJA', `팝업 표시: target="${target}", ${candidates.length}개 후보`);
    }

    /**
     * 팝업 숨김
     */
    hide() {
        if (this._container) {
            this._container.hide();
        }
        this._candidates = [];
        this._onSelect = null;
        this._onCancel = null;
    }

    /**
     * 팝업 표시 여부
     * @returns {boolean}
     */
    get isVisible() {
        return this._container?.visible ?? false;
    }

    /**
     * 키 이벤트 처리
     *
     * GTK3 unim_hanja_popup_handle_key 로직 이식.
     *
     * @param {number} keyval - Clutter keyval
     * @returns {boolean} true면 키 소비
     */
    handleKey(keyval) {
        if (!this.isVisible) return false;

        const pageCount = this._pageItemCount();

        // 숫자키 1-9: 직접 선택
        if (keyval >= Clutter.KEY_1 && keyval <= Clutter.KEY_9) {
            const idx = keyval - Clutter.KEY_1;
            if (idx < pageCount) {
                const globalIdx = this._currentPage * PAGE_SIZE + idx;
                this._selectCandidate(globalIdx);
            }
            return true;
        }

        // 위 화살표
        if (keyval === Clutter.KEY_Up) {
            if (this._selectedIndex > 0) {
                this._selectedIndex--;
                this._updateSelection();
            }
            return true;
        }

        // 아래 화살표
        if (keyval === Clutter.KEY_Down) {
            if (this._selectedIndex < pageCount - 1) {
                this._selectedIndex++;
                this._updateSelection();
            }
            return true;
        }

        // 이전 페이지: Left, PgUp, Backspace
        if (keyval === Clutter.KEY_Left || keyval === Clutter.KEY_Page_Up ||
            keyval === Clutter.KEY_BackSpace) {
            return this._prevPage();
        }

        // 다음 페이지: Right, PgDn, Space
        if (keyval === Clutter.KEY_Right || keyval === Clutter.KEY_Page_Down ||
            keyval === Clutter.KEY_space) {
            return this._nextPage();
        }

        // Enter: 현재 선택 확정
        if (keyval === Clutter.KEY_Return || keyval === Clutter.KEY_KP_Enter) {
            if (this._selectedIndex >= 0 && this._selectedIndex < pageCount) {
                const globalIdx = this._currentPage * PAGE_SIZE + this._selectedIndex;
                this._selectCandidate(globalIdx);
            }
            return true;
        }

        // Escape: 취소
        if (keyval === Clutter.KEY_Escape) {
            if (this._onCancel) this._onCancel();
            this.hide();
            return true;
        }

        // 수정자 키: 소비 (팝업 유지)
        if (this._isModifierKey(keyval)) {
            return true;
        }

        // 미처리 키 → 팝업 닫고 바이패스
        if (this._onCancel) this._onCancel();
        this.hide();
        return false;
    }

    /**
     * 자원 정리
     */
    disable() {
        this.hide();
        if (this._container) {
            Main.layoutManager.removeChrome(this._container);
            this._container.destroy();
            this._container = null;
        }
    }

    // ===========================================
    // 내부 메서드
    // ===========================================

    /** @private */
    _pageItemCount() {
        const start = this._currentPage * PAGE_SIZE;
        return Math.min(PAGE_SIZE, this._candidates.length - start);
    }

    /** @private */
    _totalPages() {
        return Math.ceil(this._candidates.length / PAGE_SIZE);
    }

    /** @private */
    _prevPage() {
        if (this._totalPages() <= 1) return true;
        this._currentPage = this._currentPage > 0
            ? this._currentPage - 1
            : this._totalPages() - 1;
        this._selectedIndex = 0;
        this._updateList();
        return true;
    }

    /** @private */
    _nextPage() {
        if (this._totalPages() <= 1) return true;
        this._currentPage = this._currentPage < this._totalPages() - 1
            ? this._currentPage + 1 : 0;
        this._selectedIndex = 0;
        this._updateList();
        return true;
    }

    /** @private */
    _selectCandidate(globalIndex) {
        if (globalIndex < this._candidates.length && this._onSelect) {
            this._onSelect(globalIndex);
        }
        this.hide();
    }

    /** @private */
    _updateList() {
        // 기존 행 제거
        this._list.destroy_all_children();
        this._rows = [];

        const start = this._currentPage * PAGE_SIZE;
        const count = this._pageItemCount();

        for (let i = 0; i < count; i++) {
            const candidate = this._candidates[start + i];
            const row = new St.BoxLayout({
                style_class: 'popup-item',
                reactive: true,
            });

            const num = new St.Label({
                style_class: 'item-number',
                text: `${i + 1}.`,
            });
            const hanja = new St.Label({
                style_class: 'item-hanja',
                text: ` ${candidate.hanja}`,
            });
            const meaning = new St.Label({
                style_class: 'item-meaning',
                text: candidate.meaning ? `  ${candidate.meaning}` : '',
            });

            row.add_child(num);
            row.add_child(hanja);
            row.add_child(meaning);

            this._list.add_child(row);
            this._rows.push(row);
        }

        // 페이지 표시
        this._footer.set_text(`${this._currentPage + 1} / ${this._totalPages()}`);

        this._updateSelection();
    }

    /** @private */
    _updateSelection() {
        for (let i = 0; i < this._rows.length; i++) {
            if (i === this._selectedIndex) {
                this._rows[i].add_style_class_name('selected');
            } else {
                this._rows[i].remove_style_class_name('selected');
            }
        }
    }

    /** @private */
    _isModifierKey(keyval) {
        return (keyval >= Clutter.KEY_Shift_L && keyval <= Clutter.KEY_Hyper_R) ||
            keyval === Clutter.KEY_Num_Lock ||
            keyval === Clutter.KEY_Scroll_Lock ||
            keyval === Clutter.KEY_Caps_Lock;
    }
}
