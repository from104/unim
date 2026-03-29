/**
 * UNIM 특수문자 후보 팝업
 *
 * 엔진 위임 모드 전용 순수 UI 컴포넌트.
 * 초성 기반 특수문자를 9x9 그리드(열 우선 채움)로 표시한다.
 * 모든 키 처리는 엔진(ProcessKeyEvent)이 담당하고,
 * 팝업은 렌더링과 마우스 이벤트 콜백만 담당한다.
 *
 * 상태 업데이트는 updateFromNavigate() 시그널이 유일한 경로.
 *
 * @see POPUP_SPEC.md
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

/** 선택 플래시 지속 시간 (ms) */
const FLASH_DURATION_MS = 120;

/**
 * SpecialPopup
 *
 * 특수문자 후보를 9x9 그리드로 표시하는 팝업.
 * 키 처리 로직 없음 — 엔진 시그널로만 상태 갱신.
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

        /** @type {number} 현재 페이지 (엔진이 관리) */
        this._currentPage = 0;
        /** @type {number} 전체 페이지 수 (엔진이 관리) */
        this._totalPages = 0;
        /** @type {number} 현재 페이지의 열 수 */
        this._cols = 0;
        /** @type {number} 현재 페이지의 행 수 */
        this._rows = 0;

        /** @type {number} 엔진이 관리하는 선택 행 */
        this._engineSelRow = 0;
        /** @type {number} 엔진이 관리하는 선택 열 */
        this._engineSelCol = 0;
        /** @type {number} 마우스 호버 행 (-1이면 비활성) */
        this._mouseHoverRow = -1;
        /** @type {number} 마우스 호버 열 (-1이면 비활성) */
        this._mouseHoverCol = -1;

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
            reactive: true,
        });

        this._header = new St.Label({ style_class: 'popup-header' });
        this._container.add_child(this._header);

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
        this._onSelect = onSelect;
        this._onCancel = onCancel;
        this._pendingHide = false;

        // 초기 상태 (엔진의 첫 PopupNavigate 시그널이 곧 덮어씀)
        this._currentPage = 0;
        this._totalPages = Math.ceil(characters.length / PAGE_SIZE);
        this._engineSelRow = 0;
        this._engineSelCol = 0;
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;

        this._header.set_text(`특수문자: ${target}`);
        this._updateGrid();

        this._container.show();
        this._positionPopup(cursorRect);

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
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;
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
     * 데몬 PopupNavigate 시그널로 상태 업데이트
     *
     * 이것이 팝업 상태를 갱신하는 유일한 경로.
     *
     * @param {number} page - 현재 페이지 (0-based)
     * @param {number} totalPages - 전체 페이지 수
     * @param {number} rows - 현재 페이지 행 수
     * @param {number} cols - 현재 페이지 열 수
     * @param {number} selRow - 선택 행
     * @param {number} selCol - 선택 열
     */
    updateFromNavigate(page, totalPages, rows, cols, selRow, selCol) {
        if (!this.isVisible) return;

        const pageChanged = this._currentPage !== page;
        this._currentPage = page;
        this._totalPages = totalPages;
        this._rows = rows;
        this._cols = cols;
        this._engineSelRow = selRow;
        this._engineSelCol = selCol;

        if (pageChanged) {
            this._updateGrid();
        }
        this._updateSelection();
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
     * 커서 위치 기반 팝업 포지셔닝
     * @param {{x: number, y: number, width: number, height: number}} [cursorRect]
     * @private
     */
    _positionPopup(cursorRect) {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natW] = this._container.get_preferred_width(-1);
        const [, natH] = this._container.get_preferred_height(-1);
        const popupWidth = natW > 0 ? natW : 340;
        const popupHeight = natH > 0 ? natH : 360;
        let x, y;

        if (cursorRect && (cursorRect.x > 0 || cursorRect.y > 0)) {
            x = cursorRect.x;
            y = cursorRect.y + cursorRect.height + 4;

            if (y + popupHeight > monitor.y + monitor.height) {
                y = cursorRect.y - popupHeight - 4;
            }
        } else {
            x = Math.floor(monitor.x + (monitor.width - popupWidth) / 2);
            y = Math.floor(monitor.y + 100);
        }

        if (x + popupWidth > monitor.x + monitor.width) {
            x = monitor.x + monitor.width - popupWidth;
        }
        x = Math.max(monitor.x, x);
        y = Math.max(monitor.y, y);

        this._container.set_position(x, y);
    }

    /**
     * 열 우선 채움 레이아웃으로 그리드 인덱스 계산
     * @param {number} row
     * @param {number} col
     * @returns {number} 전체 인덱스 또는 -1
     * @private
     */
    _getCharIndex(row, col) {
        const pageStart = this._currentPage * PAGE_SIZE;
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
     *
     * hide()는 여기서 호출하지 않음.
     * 엔진이 SelectSpecialChar 처리 후 HidePopup 시그널을 보내면 그때 닫힘.
     *
     * @param {number} row
     * @param {number} col
     * @private
     */
    _selectAt(row, col) {
        const globalIdx = this._getCharIndex(row, col);
        if (globalIdx < 0) return;

        this._pendingHide = true;
        this._updateSelection();

        // 플래시 후 콜백 호출
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, FLASH_DURATION_MS, () => {
            if (this._onSelect) {
                this._onSelect(globalIdx);
            }
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
        this._rows = 0;
        for (let row = 0; row < MAX_ROWS; row++) {
            let hasAny = false;
            for (let col = 0; col < this._cols; col++) {
                if (this._cellHasChar(row, col)) {
                    hasAny = true;
                    break;
                }
            }
            if (!hasAny) break;

            this._rows++;
            const rowWidget = new St.BoxLayout({ style_class: 'grid-row' });

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

                if (idx >= 0) {
                    const r = row;
                    const c = col;

                    // 마우스 호버: 표시만 (선택과 독립)
                    cell.connect('enter-event', () => {
                        this._mouseHoverRow = r;
                        this._mouseHoverCol = c;
                        this._updateSelection();
                        return Clutter.EVENT_STOP;
                    });
                    cell.connect('leave-event', () => {
                        this._mouseHoverRow = -1;
                        this._mouseHoverCol = -1;
                        this._updateSelection();
                        return Clutter.EVENT_STOP;
                    });

                    // 마우스 클릭: 선택 콜백
                    cell.connect('button-press-event', () => {
                        this._selectAt(r, c);
                        return Clutter.EVENT_STOP;
                    });
                }

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
     * 선택/호버 하이라이트 갱신
     *
     * .selected = 엔진이 관리하는 선택 위치 (항상 표시)
     * .hovered  = 마우스 커서가 가리키는 위치 (표시만, 선택과 무관)
     * @private
     */
    _updateSelection() {
        // 셀 하이라이트
        for (let row = 0; row < this._cells.length; row++) {
            for (let col = 0; col < this._cells[row].length; col++) {
                const cell = this._cells[row][col];
                // 엔진 선택
                if (row === this._engineSelRow && col === this._engineSelCol) {
                    cell.add_style_class_name('selected');
                } else {
                    cell.remove_style_class_name('selected');
                }
                // 마우스 호버 (선택과 독립)
                if (row === this._mouseHoverRow && col === this._mouseHoverCol) {
                    cell.add_style_class_name('hovered');
                } else {
                    cell.remove_style_class_name('hovered');
                }
            }
        }
        // 활성 열 헤더 하이라이트 (엔진 선택 기준)
        for (let col = 0; col < this._colHeaders.length; col++) {
            if (col === this._engineSelCol) {
                this._colHeaders[col].add_style_class_name('active');
            } else {
                this._colHeaders[col].remove_style_class_name('active');
            }
        }
        // 활성 행 번호 하이라이트 (엔진 선택 기준)
        for (let row = 0; row < this._rowNumbers.length; row++) {
            if (row === this._engineSelRow) {
                this._rowNumbers[row].add_style_class_name('active');
            } else {
                this._rowNumbers[row].remove_style_class_name('active');
            }
        }
    }
}
