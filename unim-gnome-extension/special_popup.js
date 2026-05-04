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
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';
import { unimLog } from './logging.js';

/** 그리드 상수 */
const MAX_ROWS = 9;
const MAX_COLS = 9;
const PAGE_SIZE = MAX_ROWS * MAX_COLS; // 81

/** 선택 플래시 지속 시간 (ms) */
const FLASH_DURATION_MS = 120;

/** 페이지 이동 버튼 글리프 */
const ICON_PREV_PAGE = '◀';
const ICON_NEXT_PAGE = '▶';

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
        /** @type {St.BoxLayout} 풋터 컨테이너 (◀ + 라벨 + ▶) */
        this._footer = null;
        /** @type {St.Button} 이전 페이지 버튼 */
        this._prevPageBtn = null;
        /** @type {St.Button} 다음 페이지 버튼 */
        this._nextPageBtn = null;
        /** @type {St.Label} 페이지 표시 라벨 */
        this._pageLabel = null;
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
        /** @type {Function|null} 페이지 이동 콜백 (direction: 0=Prev, 1=Next) */
        this._onChangePage = null;
        /** @type {Function|null} popup 키 콜백 (stage 핸들러 → processKey RPC).
         *  VTE/ghostty 등 IM 부분 참여 클라이언트가 nav 키를 IM filter 로 안 보내는 것 우회. */
        this._onPopupKey = null;
        /** @type {Function|null} popup 영역 밖 클릭 시 호출. */
        this._onOutsideClick = null;

        /** @type {number} stage captured-event 핸들러 ID — popup 가시 중에만 활성. */
        this._stageHandler = 0;
        /** @type {object|null} Main.pushModal grab 토큰 — popup 가시 중에만 유지. */
        this._modalGrab = null;
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

        // 풋터: [◀] [target n/N] [▶]
        this._footer = new St.BoxLayout({
            style_class: 'popup-footer-box',
            vertical: false,
        });
        // accessible_name — 스크린리더가 ◀/▶ 글리프 대신 의미있는 라벨 읽음 (a11y).
        this._prevPageBtn = new St.Button({
            style_class: 'popup-page-btn',
            label: ICON_PREV_PAGE,
            can_focus: false,
            reactive: true,
            track_hover: true,
            accessible_name: _('Previous page'),
        });
        this._prevPageBtn.connect('clicked', () => {
            if (this._onChangePage) this._onChangePage(0);
        });
        this._pageLabel = new St.Label({
            style_class: 'popup-footer',
            x_expand: true,
            x_align: Clutter.ActorAlign.CENTER,
        });
        this._nextPageBtn = new St.Button({
            style_class: 'popup-page-btn',
            label: ICON_NEXT_PAGE,
            can_focus: false,
            reactive: true,
            track_hover: true,
            accessible_name: _('Next page'),
        });
        this._nextPageBtn.connect('clicked', () => {
            if (this._onChangePage) this._onChangePage(1);
        });
        this._footer.add_child(this._prevPageBtn);
        this._footer.add_child(this._pageLabel);
        this._footer.add_child(this._nextPageBtn);
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
     * @param {Function} [onChangePage] - 페이지 이동 ◀/▶ 클릭 시 호출 (direction: 0=Prev, 1=Next)
     */
    show(target, characters, topRow, onSelect, onCancel, cursorRect, onChangePage, onPopupKey, onOutsideClick) {
        if (!this._container || characters.length === 0) return;

        this._target = target;
        this._characters = characters;
        this._topRow = topRow || '';
        this._onSelect = onSelect;
        this._onCancel = onCancel;
        this._onChangePage = onChangePage || null;
        this._onPopupKey = typeof onPopupKey === 'function' ? onPopupKey : null;
        this._onOutsideClick = typeof onOutsideClick === 'function' ? onOutsideClick : null;
        this._pendingHide = false;

        // 초기 상태 (엔진의 첫 PopupNavigate 시그널이 곧 덮어씀)
        this._currentPage = 0;
        this._totalPages = Math.ceil(characters.length / PAGE_SIZE);
        this._engineSelRow = 0;
        this._engineSelCol = 0;
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;

        this._header.set_text(`「${target}」 → 특수문자`);
        this._updateGrid();

        this._container.show();
        this._positionPopup(cursorRect);
        this._installStageHandler();
        this._pushModalGrab();

        unimLog('SPECIAL', `팝업 표시: target="${target}", ${characters.length}개 문자`);
    }

    /**
     * Stage captured-event 리스너 — KEY_PRESS / BUTTON_PRESS 가로채기.
     * 자세한 설명은 emoji_popup.js 참고 (동일 패턴).
     * @private
     */
    _installStageHandler() {
        if (this._stageHandler) return;
        this._stageHandler = global.stage.connect('captured-event', (_actor, event) => {
            if (!this._container?.visible) return Clutter.EVENT_PROPAGATE;
            const type = event.type();

            if (type === Clutter.EventType.KEY_PRESS) {
                const sym = event.get_key_symbol();
                const state = event.get_state();
                if (
                    sym === Clutter.KEY_Shift_L || sym === Clutter.KEY_Shift_R ||
                    sym === Clutter.KEY_Control_L || sym === Clutter.KEY_Control_R ||
                    sym === Clutter.KEY_Alt_L || sym === Clutter.KEY_Alt_R ||
                    sym === Clutter.KEY_Super_L || sym === Clutter.KEY_Super_R
                ) {
                    return Clutter.EVENT_PROPAGATE;
                }
                if (typeof this._onPopupKey !== 'function') return Clutter.EVENT_PROPAGATE;

                const keycode = event.get_key_code();
                const evdevKeycode = keycode > 8 ? keycode - 8 : 0;
                this._onPopupKey(sym, evdevKeycode, state);
                return Clutter.EVENT_STOP;
            }

            if (type === Clutter.EventType.BUTTON_PRESS) {
                const source = event.get_source();
                if (source && this._container && this._container.contains(source)) {
                    return Clutter.EVENT_PROPAGATE;
                }
                if (typeof this._onOutsideClick === 'function') {
                    try {
                        this._onOutsideClick();
                    } catch (e) {
                        unimLog('SPECIAL', `outside click handler error: ${e?.message ?? e}`);
                    }
                }
                return Clutter.EVENT_STOP;
            }

            return Clutter.EVENT_PROPAGATE;
        });
    }

    /** @private */
    _uninstallStageHandler() {
        if (!this._stageHandler) return;
        global.stage.disconnect(this._stageHandler);
        this._stageHandler = 0;
    }

    /**
     * Modal grab push — Wayland 텍스트 클라이언트(특히 VTE/ghostty 등 IM 부분
     * 참여 터미널)가 키 이벤트를 surface 로 직행시키는 것을 우회. captured-event
     * 가 모든 클라이언트에서 균일하게 발화하도록 보장한다.
     * @private
     */
    _pushModalGrab() {
        if (this._modalGrab || !this._container) return;
        try {
            const grab = Main.pushModal(this._container, {
                actionMode: Shell.ActionMode.POPUP,
            });
            if (grab && (grab.get_seat_state?.() & Clutter.GrabState.KEYBOARD)) {
                this._modalGrab = grab;
            } else if (grab) {
                Main.popModal(grab);
                unimLog('SPECIAL', 'pushModal: keyboard grab 미획득 — stage 캡처 폴백');
            }
        } catch (e) {
            unimLog('SPECIAL', `pushModal 실패 (stage 캡처 폴백): ${e?.message ?? e}`);
        }
    }

    /** @private */
    _popModalGrab() {
        if (!this._modalGrab) return;
        try {
            Main.popModal(this._modalGrab);
        } catch (e) {
            unimLog('SPECIAL', `popModal 실패: ${e?.message ?? e}`);
        }
        this._modalGrab = null;
    }

    /**
     * 팝업 숨김
     */
    hide() {
        this._popModalGrab();
        this._uninstallStageHandler();
        if (this._container) {
            this._container.hide();
        }
        this._characters = [];
        this._pendingHide = false;
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;
        this._onSelect = null;
        this._onCancel = null;
        this._onChangePage = null;
        this._onPopupKey = null;
        this._onOutsideClick = null;
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
     * 9×9 그리드 시각 일관성 정책 (한자 expanded 와 동일):
     * 엔진이 보내는 rows/cols 는 카운트만 참고하고, 그리드 차원은 항상
     * MAX_ROWS×MAX_COLS 로 고정. 페이지 전환·정렬 시 그리드가 출렁이지 않아
     * 사용자가 셀 위치를 학습하기 쉽다. 빈 셀은 _cellHasChar 가 false 를
     * 반환해 자동 비활성.
     *
     * @param {number} page - 현재 페이지 (0-based)
     * @param {number} totalPages - 전체 페이지 수
     * @param {number} _rows - 엔진의 페이지 행 수 (시각 차원에 미반영)
     * @param {number} _cols - 엔진의 페이지 열 수 (시각 차원에 미반영)
     * @param {number} selRow - 선택 행
     * @param {number} selCol - 선택 열
     */
    updateFromNavigate(page, totalPages, _rows, _cols, selRow, selCol) {
        if (!this.isVisible) return;

        const pageChanged = this._currentPage !== page;
        this._currentPage = page;
        this._totalPages = totalPages;
        this._rows = MAX_ROWS;
        this._cols = MAX_COLS;
        this._engineSelRow = selRow;
        this._engineSelCol = selCol;

        if (pageChanged) {
            this._updateGrid();
        }
        this._updateSelection();
    }

    /**
     * 통합 PopupRender 시그널 핸들러 (Phase B 통합 SoT).
     *
     * daemon 산출 view_model 의 미리 포맷된 문자열을 적용:
     * - header_text: "「ㄱ」 → 특수문자"
     * - footer_text: "[ㄱ]  1 / 3"
     * - show_footer: 단일 페이지면 false → footer hide.
     * @param {object} state
     */
    updateFromRender(state) {
        if (!this.isVisible) return;
        if (this._header) {
            this._header.set_text(state.headerText || '');
        }
        if (this._pageLabel) {
            this._pageLabel.set_text(state.footerText || '');
        }
        if (this._footer) {
            if (state.showFooter) {
                this._footer.show();
                this._prevPageBtn?.show();
                this._nextPageBtn?.show();
            } else {
                this._footer.hide();
            }
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
     * 그리드 재구성 — 항상 9×9 그리드 고정.
     *
     * 한자 popup expanded 와 동일 정책: 페이지 항목 수가 81 미만이어도 시각
     * 차원은 MAX_ROWS × MAX_COLS 로 일정. 빈 셀은 reactive=false / 클릭 핸들러
     * 미연결로 자동 비활성. 그리드가 출렁이지 않아 사용자가 셀 위치를 학습하기
     * 쉽다.
     * @private
     */
    _updateGrid() {
        this._grid.destroy_all_children();
        this._cells = [];
        this._colHeaders = [];
        this._rowNumbers = [];

        this._rows = MAX_ROWS;
        this._cols = MAX_COLS;

        // 열 헤더 행 — 항상 9 컬럼 (top_row 짧으면 빈 라벨)
        const headerRow = new St.BoxLayout({ style_class: 'grid-row' });
        headerRow.add_child(new St.Label({
            style_class: 'grid-row-number',
            text: '',
        }));

        for (let col = 0; col < MAX_COLS; col++) {
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

        // 데이터 행 — 항상 9 행, 빈 셀은 reactive=false 로 비활성
        for (let row = 0; row < MAX_ROWS; row++) {
            const rowWidget = new St.BoxLayout({ style_class: 'grid-row' });

            const rowNum = new St.Label({
                style_class: 'grid-row-number',
                text: `${row + 1}`,
            });
            rowWidget.add_child(rowNum);
            this._rowNumbers.push(rowNum);

            const rowCells = [];
            for (let col = 0; col < MAX_COLS; col++) {
                const idx = this._getCharIndex(row, col);
                const ch = idx >= 0 ? this._characters[idx] : '';
                const cell = new St.Label({
                    style_class: 'grid-cell',
                    text: ch,
                    x_align: Clutter.ActorAlign.CENTER,
                    reactive: idx >= 0,
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

        // 풋터: 단일 페이지면 컨테이너 전체 hide, 여러 페이지면 표시 + ◀/▶ 노출
        if (this._totalPages > 1) {
            this._pageLabel.set_text(`[${this._target}]  ${this._currentPage + 1}/${this._totalPages}`);
            this._prevPageBtn.show();
            this._nextPageBtn.show();
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
