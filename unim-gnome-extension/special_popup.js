/**
 * UNIM 특수문자 후보 팝업
 *
 * 초성 기반 특수문자를 9×9 그리드로 표시합니다.
 * 열 우선 채움 레이아웃 사용. GTK3 unim_special_popup.c 로직 이식.
 *
 * @module special_popup
 */

import GLib from 'gi://GLib';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

/** 그리드 상수 */
const MAX_ROWS = 9;
const MAX_COLS = 9;
const PAGE_SIZE = MAX_ROWS * MAX_COLS; // 81

/** top_row 물리 키 위치 (QWERTY 기준) */
const PHYSICAL_KEYS = 'qwertyuio';

/** 선택 플래시 지속 시간 (ms) */
const FLASH_DURATION_MS = 120;

/**
 * SpecialPopup
 *
 * 특수문자 후보를 9×9 그리드로 표시하는 팝업.
 * top_row 키로 열 점프, 숫자로 행 선택, 화살표로 네비게이션.
 */
export class SpecialPopup {
    constructor() {
        /** @type {St.BoxLayout|null} */
        this._container = null;
        /** @type {St.Label} 헤더 */
        this._header = null;
        /** @type {St.Widget} 그리드 */
        this._grid = null;
        /** @type {St.Label} 풋터 */
        this._footer = null;
        /** @type {St.Label[][]} 셀 라벨 [row][col] */
        this._cells = [];
        /** @type {St.Label[]} 열 헤더 라벨 */
        this._colHeaders = [];
        /** @type {St.Label[]} 행 번호 라벨 */
        this._rowNumbers = [];

        /** @type {string[]} 전체 특수문자 배열 */
        this._characters = [];
        /** @type {string} 대상 문자 */
        this._target = '';
        /** @type {string} top_row 키 (표시용) */
        this._topRow = '';
        /** @type {number} 현재 페이지의 열 수 */
        this._cols = 0;
        /** @type {number} 현재 페이지 */
        this._currentPage = 0;
        /** @type {number} 전체 페이지 수 */
        this._totalPages = 0;
        /** @type {number} 선택 행 */
        this._selRow = 0;
        /** @type {number} 선택 열 */
        this._selCol = 0;
        /** @type {boolean} 플래시 후 숨김 대기 중 */
        this._pendingHide = false;

        /** @type {Function|null} 선택 콜백 (globalIndex) */
        this._onSelect = null;
        /** @type {Function|null} 취소 콜백 */
        this._onCancel = null;
    }

    /**
     * 위젯 초기화
     */
    enable() {
        this._container = new St.BoxLayout({
            style_class: 'unim-special-popup',
            vertical: true,
            visible: false,
        });

        this._header = new St.Label({ style_class: 'popup-header' });
        this._container.add_child(this._header);

        // 그리드 영역
        this._grid = new St.BoxLayout({ vertical: true });
        this._container.add_child(this._grid);

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
     * @param {string} target - 대상 문자
     * @param {string[]} characters - 특수문자 배열
     * @param {string} topRow - top_row 키 문자열 (표시용)
     * @param {Function} onSelect - 선택 콜백 (globalIndex)
     * @param {Function} onCancel - 취소 콜백
     * @param {{x: number, y: number, width: number, height: number}} [cursorRect] - 커서 위치
     */
    show(target, characters, topRow, onSelect, onCancel, cursorRect) {
        if (!this._container || characters.length === 0) return;

        this._target = target;
        this._characters = characters;
        this._topRow = topRow || '';
        this._currentPage = 0;
        this._selRow = 0;
        this._selCol = 0;
        this._pendingHide = false;
        this._onSelect = onSelect;
        this._onCancel = onCancel;

        this._totalPages = Math.ceil(characters.length / PAGE_SIZE);
        this._header.set_text(`특수문자: ${target}`);

        this._updateGrid();

        // 커서 아래에 배치 + 화면 경계 조정
        const monitor = Main.layoutManager.primaryMonitor;
        if (monitor) {
            const popupWidth = 340;
            const popupHeight = 360;
            let x, y;

            if (cursorRect && (cursorRect.x > 0 || cursorRect.y > 0)) {
                x = cursorRect.x;
                y = cursorRect.y + cursorRect.height + 4;
            } else {
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
            x = Math.max(monitor.x, x);
            y = Math.max(monitor.y, y);

            this._container.set_position(x, y);
        }

        this._container.show();
        unimLog('SPECIAL', `팝업 표시: target="${target}", ${characters.length}개 문자`);
    }

    /**
     * 팝업 숨김
     */
    hide() {
        if (this._container) {
            this._container.hide();
        }
        this._characters = [];
        this._pendingHide = false;
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
     * GTK3 unim_special_popup_handle_key 로직 이식.
     *
     * @param {number} keyval - Clutter keyval
     * @returns {boolean} true면 키 소비
     */
    handleKey(keyval) {
        if (!this.isVisible || this._pendingHide) return false;

        // top_row 키: 열 점프 (물리 키 위치 기준)
        const lower = keyval >= Clutter.KEY_A && keyval <= Clutter.KEY_Z
            ? keyval + 32 : keyval; // 대문자 → 소문자

        for (let i = 0; i < Math.min(9, this._cols); i++) {
            const expected = PHYSICAL_KEYS.charCodeAt(i);
            if (lower === expected) {
                if (this._cellHasChar(0, i)) {
                    this._selCol = i;
                    if (!this._cellHasChar(this._selRow, this._selCol)) {
                        this._selRow = 0;
                    }
                    this._updateSelection();
                }
                return true;
            }
        }

        // 숫자 1-9: 현재 열의 N번째 행 선택
        if (keyval >= Clutter.KEY_1 && keyval <= Clutter.KEY_9) {
            const row = keyval - Clutter.KEY_1;
            if (this._cellHasChar(row, this._selCol)) {
                this._selRow = row;
                this._updateSelection();
                this._selectCurrent();
            }
            return true;
        }

        switch (keyval) {
        // 화살표: 네비게이션 (무한 루프)
        case Clutter.KEY_Up:
            if (this._selRow > 0) {
                this._selRow--;
            } else {
                // 위 가장자리 → 같은 열 마지막 행
                let lastRow = MAX_ROWS - 1;
                while (lastRow > 0 && !this._cellHasChar(lastRow, this._selCol)) {
                    lastRow--;
                }
                this._selRow = lastRow;
            }
            this._updateSelection();
            return true;

        case Clutter.KEY_Down:
            if (this._selRow < MAX_ROWS - 1 &&
                this._cellHasChar(this._selRow + 1, this._selCol)) {
                this._selRow++;
            } else {
                this._selRow = 0;
            }
            this._updateSelection();
            return true;

        case Clutter.KEY_Left:
            if (this._selCol > 0) {
                this._selCol--;
            } else {
                this._selCol = this._cols - 1;
            }
            if (!this._cellHasChar(this._selRow, this._selCol)) {
                this._selRow = 0;
            }
            this._updateSelection();
            return true;

        case Clutter.KEY_Right:
            if (this._selCol < this._cols - 1) {
                this._selCol++;
            } else {
                this._selCol = 0;
            }
            if (!this._cellHasChar(this._selRow, this._selCol)) {
                this._selRow = 0;
            }
            this._updateSelection();
            return true;

        // 페이지 이동
        case Clutter.KEY_Page_Up:
            if (this._currentPage > 0) {
                this._currentPage--;
                this._selRow = 0;
                this._selCol = 0;
                this._updateGrid();
            }
            return true;

        case Clutter.KEY_Page_Down:
        case Clutter.KEY_space:
            if (this._currentPage < this._totalPages - 1) {
                this._currentPage++;
                this._selRow = 0;
                this._selCol = 0;
                this._updateGrid();
            }
            return true;

        // Tab / Shift+Tab: 페이지 (무한 루프)
        case Clutter.KEY_Tab:
            if (this._currentPage < this._totalPages - 1) {
                this._currentPage++;
            } else {
                this._currentPage = 0;
            }
            this._selRow = 0;
            this._selCol = 0;
            this._updateGrid();
            return true;

        case Clutter.KEY_ISO_Left_Tab: // Shift+Tab
            if (this._currentPage > 0) {
                this._currentPage--;
            } else {
                this._currentPage = this._totalPages - 1;
            }
            this._selRow = 0;
            this._selCol = 0;
            this._updateGrid();
            return true;

        // Enter: 현재 선택 확정
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
            this._selectCurrent();
            return true;

        // Escape: 취소
        case Clutter.KEY_Escape:
            if (this._onCancel) this._onCancel();
            this.hide();
            return true;

        // 수정자 키: 소비
        case Clutter.KEY_Shift_L: case Clutter.KEY_Shift_R:
        case Clutter.KEY_Control_L: case Clutter.KEY_Control_R:
        case Clutter.KEY_Alt_L: case Clutter.KEY_Alt_R:
        case Clutter.KEY_Super_L: case Clutter.KEY_Super_R:
            return true;

        default:
            // 미처리 키 → 팝업 닫고 바이패스
            if (this._onCancel) this._onCancel();
            this.hide();
            return false;
        }
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

    /**
     * 열 우선 채움 레이아웃으로 그리드 인덱스 계산
     * @param {number} row
     * @param {number} col
     * @returns {number} 전체 인덱스 또는 -1
     * @private
     */
    _getCharIndex(row, col) {
        const pageStart = this._currentPage * PAGE_SIZE;
        // 열 우선: col * MAX_ROWS + row
        const pageOffset = col * MAX_ROWS + row;
        const globalIdx = pageStart + pageOffset;
        return globalIdx < this._characters.length ? globalIdx : -1;
    }

    /**
     * 셀에 문자가 있는지 확인
     * @private
     */
    _cellHasChar(row, col) {
        return this._getCharIndex(row, col) >= 0;
    }

    /**
     * 현재 선택 확정 (플래시 효과 포함)
     * @private
     */
    _selectCurrent() {
        const globalIdx = this._getCharIndex(this._selRow, this._selCol);
        if (globalIdx < 0) return;

        this._pendingHide = true;

        // 선택 플래시 표시
        this._updateSelection();

        // 플래시 후 콜백 호출 + 숨김
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, FLASH_DURATION_MS, () => {
            if (this._onSelect) {
                this._onSelect(globalIdx);
            }
            this.hide();
            return GLib.SOURCE_REMOVE;
        });
    }

    /**
     * 그리드 재구성
     * @private
     */
    _updateGrid() {
        this._grid.destroy_all_children();
        this._cells = [];
        this._colHeaders = [];
        this._rowNumbers = [];

        // 현재 페이지 문자 수 및 열 수 계산
        const pageStart = this._currentPage * PAGE_SIZE;
        const pageCharCount = Math.min(PAGE_SIZE, this._characters.length - pageStart);
        this._cols = Math.min(MAX_COLS, Math.ceil(pageCharCount / MAX_ROWS));

        // 열 헤더 행
        const headerRow = new St.BoxLayout({ style_class: 'grid-row' });
        // 빈 모서리 셀
        headerRow.add_child(new St.Label({
            style_class: 'grid-row-number',
            text: '',
        }));

        for (let col = 0; col < this._cols; col++) {
            const headerChar = col < this._topRow.length ? this._topRow[col] : '';
            const label = new St.Label({
                style_class: 'grid-header',
                text: headerChar,
                x_align: Clutter.ActorAlign.CENTER,
            });
            headerRow.add_child(label);
            this._colHeaders.push(label);
        }
        this._grid.add_child(headerRow);

        // 데이터 행
        for (let row = 0; row < MAX_ROWS; row++) {
            // 이 행에 문자가 하나라도 있는지 확인
            let hasAny = false;
            for (let col = 0; col < this._cols; col++) {
                if (this._cellHasChar(row, col)) {
                    hasAny = true;
                    break;
                }
            }
            if (!hasAny) break;

            const rowWidget = new St.BoxLayout({ style_class: 'grid-row' });

            // 행 번호
            const rowNum = new St.Label({
                style_class: 'grid-row-number',
                text: `${row + 1}`,
            });
            rowWidget.add_child(rowNum);
            this._rowNumbers.push(rowNum);

            const rowCells = [];
            for (let col = 0; col < this._cols; col++) {
                const idx = this._getCharIndex(row, col);
                const ch = idx >= 0 ? this._characters[idx] : '';
                const cell = new St.Label({
                    style_class: 'grid-cell',
                    text: ch,
                    x_align: Clutter.ActorAlign.CENTER,
                    reactive: true,
                });
                rowWidget.add_child(cell);
                rowCells.push(cell);
            }

            this._grid.add_child(rowWidget);
            this._cells.push(rowCells);
        }

        // 풋터
        if (this._totalPages > 1) {
            this._footer.set_text(`${this._currentPage + 1} / ${this._totalPages}`);
            this._footer.show();
        } else {
            this._footer.hide();
        }

        this._updateSelection();
    }

    /**
     * 선택 하이라이트 갱신
     * @private
     */
    _updateSelection() {
        for (let row = 0; row < this._cells.length; row++) {
            for (let col = 0; col < this._cells[row].length; col++) {
                if (row === this._selRow && col === this._selCol) {
                    this._cells[row][col].add_style_class_name('selected');
                } else {
                    this._cells[row][col].remove_style_class_name('selected');
                }
            }
        }
    }
}
