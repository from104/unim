/**
 * UNIM 이모지 팝업 (GNOME Shell Extension) — engine-driven receiver.
 *
 * PR #3 (이모지 전면 개선) 부터 본 팝업은 한자/특수문자 팝업과 동일한 구조의
 * 단순 receiver 다. 키 입력·페이지·선택 인덱스는 모두 엔진(`popup_state`) 이
 * 단일 진실 소스로 관리하고, 본 모듈은 DBus 시그널로 받은 상태를 9×9 그리드 +
 * 좌측 9 탭(세로) + 하단 페이지 인디케이터로 렌더링만 한다.
 *
 * - `show(target_cat_id, items, top_row, recent, categories, cursorRect)`
 *      : `ShowEmojiPopupV2` 시그널 수신 후 호출. 카테고리/MRU/페이지 메타 일체를
 *        payload 로 받아 즉시 그린다.
 * - `updateFromShow(...)` : 카테고리 전환 등 데몬이 ShowEmojiPopupV2 를 재발행하면
 *        같은 인스턴스를 재구성한다 (한자 patten 동일).
 * - `updateFromNavigate(page, totalPages, rows, cols, selRow, selCol)`
 *      : `PopupNavigate` 시그널로 페이지/선택 위치 갱신.
 * - 마우스 클릭 → `commitEmoji(emoji)` RPC 직접 호출.
 *
 * 키 입력 처리 / `Main.pushModal` grab / 검색 입력 등은 모두 제거됨.
 *
 * @module emoji_popup
 * @see POPUP_SPEC.md
 */

import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

/** 그리드 상수 (특수문자 팝업과 동일 — 엔진 popup_state 와 합의) */
const MAX_ROWS = 9;
const MAX_COLS = 9;
const PAGE_SIZE = MAX_ROWS * MAX_COLS; // 81

/** 페이지 이동 버튼 글리프 */
const ICON_PREV_PAGE = '◀';
const ICON_NEXT_PAGE = '▶';

/** 카테고리 id → locale 라벨 (GTK 측 `tab_label_for` 와 합의 유지) */
const TAB_LABEL_KO = {
    Recent: '최근 사용',
    SmileysPeople: '표정·인물',
    Animals: '동물·자연',
    Food: '음식·음료',
    Activities: '활동',
    Travel: '여행·장소',
    Objects: '사물',
    Symbols: '기호',
    Flags: '국기',
};

/**
 * EmojiPopup
 *
 * 9×9 그리드 + 좌측 세로 9 탭 + 하단 페이지 인디케이터.
 * 키 처리·검색·모달 grab 없음 — 엔진/데몬 시그널만 수신.
 */
export class EmojiPopup {
    constructor() {
        /** @type {St.BoxLayout|null} 외곽 컨테이너 */
        this._container = null;
        /** @type {St.Label|null} 헤더 (`「Recent」 → 이모지` 등) */
        this._header = null;
        /** @type {St.BoxLayout|null} body (좌측 탭 + 우측 그리드) */
        this._body = null;
        /** @type {St.BoxLayout|null} 좌측 세로 탭 컨테이너 */
        this._tabBar = null;
        /** @type {St.BoxLayout|null} 그리드 컨테이너 */
        this._grid = null;
        /** @type {St.BoxLayout|null} 풋터 컨테이너 (◀ + 라벨 + ▶) */
        this._footer = null;
        /** @type {St.Button|null} 이전 페이지 버튼 */
        this._prevPageBtn = null;
        /** @type {St.Button|null} 다음 페이지 버튼 */
        this._nextPageBtn = null;
        /** @type {St.Label|null} 페이지 인디케이터 라벨 */
        this._pageLabel = null;

        /** @type {St.Label[][]} 셀 라벨 [row][col] */
        this._cells = [];
        /** @type {St.Label[]} 열 헤더 라벨 */
        this._colHeaders = [];
        /** @type {St.Label[]} 행 번호 라벨 */
        this._rowNumbers = [];
        /** @type {Map<string, St.Button>} 카테고리 id → 탭 버튼 */
        this._tabButtons = new Map();

        /** @type {string[]} 현재 카테고리의 emoji 풀 (엔진 send) */
        this._items = [];
        /** @type {string} 현재 카테고리 id */
        this._currentCatId = 'Recent';
        /** @type {string} top_row 키 (열 헤더 표시용) */
        this._topRow = '';
        /** @type {Array<{id:string, ko:string, en:string, count:number}>} */
        this._categories = [];

        /** @type {number} 엔진 소관 — 현재 페이지 (0-based) */
        this._currentPage = 0;
        /** @type {number} 엔진 소관 — 전체 페이지 수 */
        this._totalPages = 0;
        /** @type {number} 현재 페이지의 행 수 */
        this._rows = 0;
        /** @type {number} 현재 페이지의 열 수 */
        this._cols = 0;
        /** @type {number} 엔진 소관 — 선택 행 */
        this._engineSelRow = 0;
        /** @type {number} 엔진 소관 — 선택 열 */
        this._engineSelCol = 0;

        /** @type {number} 마우스 호버 행 (-1=비활성) */
        this._mouseHoverRow = -1;
        /** @type {number} 마우스 호버 열 (-1=비활성) */
        this._mouseHoverCol = -1;

        /** @type {Function|null} 셀 클릭 시 emoji 문자 commit 콜백 */
        this._onCommit = null;
        /** @type {Function|null} 좌측 탭 클릭 시 카테고리 인덱스 콜백 (idx → daemon RPC) */
        this._onTabClick = null;
        /** @type {Function|null} 페이지 이동 콜백 (direction: 0=Prev, 1=Next) */
        this._onChangePage = null;

        /** @type {number} stage captured-event 핸들러 ID — popup 떠있는 동안만 활성.
         *  GNOME Shell 이 Tab 을 focus chain 으로 흡수해 IM 모듈에 도달하지 않으므로,
         *  popup 활성 중 Tab/Shift+Tab 을 stage 레벨에서 캡처해 setEmojiCategory RPC 로
         *  직접 라우팅한다 (마우스 클릭과 동일한 우회 패턴). */
        this._stageKeyHandler = 0;
    }

    /**
     * 위젯 초기화 (Main.layoutManager 에 chrome 으로 추가)
     */
    enable() {
        this._container = new St.BoxLayout({
            style_class: 'unim-emoji-popup',
            vertical: true,
            visible: false,
            reactive: true,
        });

        this._header = new St.Label({ style_class: 'popup-header' });
        this._container.add_child(this._header);

        this._body = new St.BoxLayout({
            style_class: 'emoji-body',
            vertical: false,
        });
        this._container.add_child(this._body);

        // 좌측 세로 탭 컨테이너
        this._tabBar = new St.BoxLayout({
            style_class: 'emoji-tabs-vertical',
            vertical: true,
        });
        this._body.add_child(this._tabBar);

        // 우측 그리드 컨테이너
        this._grid = new St.BoxLayout({ vertical: true });
        this._body.add_child(this._grid);

        // 풋터: [◀] [카테고리 n/N] [▶]
        this._footer = new St.BoxLayout({
            style_class: 'popup-footer-box',
            vertical: false,
        });
        this._prevPageBtn = new St.Button({
            style_class: 'popup-page-btn',
            label: ICON_PREV_PAGE,
            can_focus: false,
            reactive: true,
            track_hover: true,
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
     * 자원 정리
     */
    disable() {
        this.hide();
        if (this._container) {
            Main.layoutManager.removeChrome(this._container);
            this._container.destroy();
            this._container = null;
        }
        this._header = null;
        this._body = null;
        this._tabBar = null;
        this._grid = null;
        this._footer = null;
        this._prevPageBtn = null;
        this._nextPageBtn = null;
        this._pageLabel = null;
        this._tabButtons.clear();
    }

    /**
     * 팝업 표시 여부
     * @returns {boolean}
     */
    get isVisible() {
        return this._container?.visible ?? false;
    }

    /**
     * `ShowEmojiPopupV2` 시그널 페이로드로 팝업 표시 (또는 카테고리 전환 시 재구성).
     *
     * @param {string} targetCatId - 시작 카테고리 id ("Recent" / "SmileysPeople" / ...).
     * @param {string[]} items - 시작 카테고리 emoji 풀.
     * @param {string} topRow - 활성 영문 키맵 상단 9 문자.
     * @param {string[]} _recent - Recent 캐시 (현재 표시 직접 사용 X — 엔진이 cat_index=0 일 때
     *   `items` 로 보냄. 동기화 디버깅용으로만 보관).
     * @param {Array<[string,string,string,number]>} categoriesRaw -
     *   (id, ko, en, count) 튜플 9개 (Recent + 8 통합 카테고리).
     * @param {Function} onCommit - 셀 클릭 시 emoji 문자 commit 콜백 (extension 의 commitEmoji 등).
     * @param {{x:number,y:number,width:number,height:number}} [cursorRect] - 커서 위치.
     * @param {Function} [onTabClick] - 좌측 탭 마우스 클릭 시 카테고리 인덱스 콜백 (idx).
     *   extension 이 `_dbusIME.setEmojiCategory(idx)` RPC 를 호출하도록 한다.
     *   생략하면 탭 클릭 비활성 (키보드 Tab/ShiftTab 만 동작).
     * @param {Function} [onChangePage] - 풋터 ◀/▶ 클릭 시 호출 (direction: 0=Prev, 1=Next).
     */
    show(targetCatId, items, topRow, _recent, categoriesRaw, onCommit, cursorRect, onTabClick, onChangePage) {
        if (!this._container) return;

        this._items = Array.isArray(items) ? items.slice() : [];
        this._currentCatId = targetCatId || 'Recent';
        this._topRow = topRow || '';
        this._categories = (categoriesRaw || []).map(([id, ko, en, count]) => ({
            id, ko, en, count: Number(count) || 0,
        }));
        this._onCommit = typeof onCommit === 'function' ? onCommit : null;
        this._onTabClick = typeof onTabClick === 'function' ? onTabClick : null;
        this._onChangePage = typeof onChangePage === 'function' ? onChangePage : null;

        // 페이지/선택 초기값 (엔진의 첫 PopupNavigate 시그널이 곧 덮어씀)
        this._currentPage = 0;
        this._totalPages = Math.max(1, Math.ceil(this._items.length / PAGE_SIZE));
        this._engineSelRow = 0;
        this._engineSelCol = 0;
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;

        this._updateHeader();
        this._rebuildTabBar();
        this._updateGrid();

        this._container.show();
        this._positionPopup(cursorRect);
        this._installStageTabHandler();

        unimLog(
            'EMOJI',
            `팝업 표시: cat='${this._currentCatId}', items=${this._items.length}, ` +
            `cats=${this._categories.length}, totalPages=${this._totalPages}`
        );
    }

    /**
     * Stage 레벨 captured-event 리스너로 Tab/Shift+Tab 을 가로채 카테고리 전환.
     * GNOME Shell focus chain 이 Tab 을 흡수하기 때문에 IM 모듈로 도달하지 않으므로
     * extension 측에서 직접 RPC 호출한다 (마우스 클릭과 동일한 우회).
     */
    _installStageTabHandler() {
        if (this._stageKeyHandler) return;
        this._stageKeyHandler = global.stage.connect('captured-event', (_actor, event) => {
            if (!this._container?.visible) return Clutter.EVENT_PROPAGATE;
            if (event.type() !== Clutter.EventType.KEY_PRESS) return Clutter.EVENT_PROPAGATE;
            const sym = event.get_key_symbol();
            const isShift = (event.get_state() & Clutter.ModifierType.SHIFT_MASK) !== 0;
            const isTab = sym === Clutter.KEY_Tab;
            const isShiftTab = sym === Clutter.KEY_ISO_Left_Tab || (isTab && isShift);
            if (!isTab && !isShiftTab) return Clutter.EVENT_PROPAGATE;
            if (typeof this._onTabClick !== 'function') return Clutter.EVENT_PROPAGATE;
            const total = this._categories.length;
            if (total === 0) return Clutter.EVENT_PROPAGATE;
            const cur = this._categories.findIndex(c => c.id === this._currentCatId);
            const curIdx = cur < 0 ? 0 : cur;
            const next = isShiftTab
                ? (curIdx - 1 + total) % total
                : (curIdx + 1) % total;
            this._onTabClick(next);
            return Clutter.EVENT_STOP;
        });
    }

    /**
     * Stage Tab 핸들러 해제 (popup 숨김 시 호출).
     */
    _uninstallStageTabHandler() {
        if (!this._stageKeyHandler) return;
        global.stage.disconnect(this._stageKeyHandler);
        this._stageKeyHandler = 0;
    }

    /**
     * 팝업 숨김
     */
    hide() {
        this._uninstallStageTabHandler();
        if (this._container) {
            this._container.hide();
        }
        this._items = [];
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;
        this._onCommit = null;
        this._onTabClick = null;
        this._onChangePage = null;
    }

    /**
     * 데몬 PopupNavigate 시그널로 페이지/선택 위치 갱신 (엔진이 단일 진실 소스).
     *
     * @param {number} page
     * @param {number} totalPages
     * @param {number} rows
     * @param {number} cols
     * @param {number} selRow
     * @param {number} selCol
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

    // ===========================================
    // 내부 메서드
    // ===========================================

    /** @private */
    _updateHeader() {
        if (!this._header) return;
        const label = TAB_LABEL_KO[this._currentCatId] || this._currentCatId;
        this._header.set_text(`「${label}」 → 이모지`);
    }

    /**
     * 좌측 세로 9 탭 재구성. 활성 카테고리에 .active 부여.
     * @private
     */
    _rebuildTabBar() {
        if (!this._tabBar) return;
        this._tabBar.destroy_all_children();
        this._tabButtons.clear();

        const hasClickHandler = typeof this._onTabClick === 'function';
        for (let i = 0; i < this._categories.length; i++) {
            const cat = this._categories[i];
            const label = TAB_LABEL_KO[cat.id] || cat.ko || cat.en || cat.id;
            const btn = new St.Button({
                style_class: 'emoji-tab-vertical',
                label,
                can_focus: false,
                // 마우스 클릭으로 카테고리 전환 — 콜백이 있을 때만 활성화.
                // 키보드 Tab/ShiftTab 은 별도 경로(엔진)로 항상 동작.
                reactive: hasClickHandler,
                track_hover: hasClickHandler,
            });
            if (cat.id === this._currentCatId) {
                btn.add_style_class_name('active');
            }
            if (hasClickHandler) {
                const idx = i;
                btn.connect('clicked', () => {
                    try {
                        this._onTabClick(idx);
                    } catch (e) {
                        unimLog('EMOJI', `tab click handler error: ${e?.message ?? e}`);
                    }
                });
            }
            this._tabBar.add_child(btn);
            this._tabButtons.set(cat.id, btn);
        }
    }

    /**
     * 열 우선 채움 레이아웃으로 전체 인덱스 계산.
     * @private
     */
    _getItemIndex(row, col) {
        const pageStart = this._currentPage * PAGE_SIZE;
        const pageOffset = col * MAX_ROWS + row;
        const globalIdx = pageStart + pageOffset;
        return globalIdx < this._items.length ? globalIdx : -1;
    }

    /** @private */
    _cellHasItem(row, col) {
        return this._getItemIndex(row, col) >= 0;
    }

    /**
     * 9×9 그리드 재구성 + 푸터 갱신.
     * @private
     */
    _updateGrid() {
        if (!this._grid) return;
        this._grid.destroy_all_children();
        this._cells = [];
        this._colHeaders = [];
        this._rowNumbers = [];

        // 현재 페이지 항목 수 및 열 수 계산
        const pageStart = this._currentPage * PAGE_SIZE;
        const pageItemCount = Math.max(0,
            Math.min(PAGE_SIZE, this._items.length - pageStart));
        // 빈 페이지(예: Recent 비어있음)는 9 열을 그대로 그리되 셀이 빈 라벨로 남는다 —
        // 사용자가 9×9 레이아웃을 인지하도록.
        this._cols = pageItemCount === 0
            ? MAX_COLS
            : Math.min(MAX_COLS, Math.ceil(pageItemCount / MAX_ROWS));

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

        // 데이터 행 — 빈 페이지에서도 9 행 모두 렌더 (사용자 요구: 9×9 표 유지)
        const minRows = pageItemCount === 0 ? MAX_ROWS : 0;
        this._rows = 0;
        for (let row = 0; row < MAX_ROWS; row++) {
            let hasAny = false;
            for (let col = 0; col < this._cols; col++) {
                if (this._cellHasItem(row, col)) {
                    hasAny = true;
                    break;
                }
            }
            if (!hasAny && row >= minRows) break;

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
                const idx = this._getItemIndex(row, col);
                const ch = idx >= 0 ? this._items[idx] : '';
                const cell = new St.Label({
                    style_class: 'grid-cell',
                    text: ch,
                    x_align: Clutter.ActorAlign.CENTER,
                    reactive: idx >= 0,
                });

                if (idx >= 0) {
                    const r = row;
                    const c = col;
                    const emoji = ch;

                    // 마우스 호버 (선택과 독립)
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

                    // 마우스 클릭 → emoji 직접 commit (한자/특수와 다른 패턴 —
                    // emoji 는 SelectAtIndex RPC 가 없고 데몬이 commitEmoji(s) 로
                    // last-active path 로 redirect 한다)
                    cell.connect('button-press-event', () => {
                        if (this._onCommit) {
                            this._onCommit(emoji);
                        }
                        return Clutter.EVENT_STOP;
                    });
                }

                rowWidget.add_child(cell);
                rowCells.push(cell);
            }

            this._grid.add_child(rowWidget);
            this._cells.push(rowCells);
        }

        // 푸터 — 카테고리 라벨 + 페이지 인디케이터 (◀/▶ 는 단일 페이지일 때 hide)
        if (this._footer && this._pageLabel) {
            const catLabel = TAB_LABEL_KO[this._currentCatId] || this._currentCatId;
            if (this._totalPages > 1) {
                this._pageLabel.set_text(
                    `[${catLabel}]  ${this._currentPage + 1}/${this._totalPages}`);
                this._prevPageBtn?.show();
                this._nextPageBtn?.show();
            } else {
                this._pageLabel.set_text(`[${catLabel}]`);
                this._prevPageBtn?.hide();
                this._nextPageBtn?.hide();
            }
            this._footer.show();
        }

        this._updateSelection();
    }

    /**
     * 선택/호버 하이라이트 갱신.
     *
     * .selected = 엔진이 관리하는 선택 위치 (PopupNavigate 시그널 기반).
     * .hovered  = 마우스 커서가 가리키는 위치 (선택과 독립, 표시만).
     *
     * @private
     */
    _updateSelection() {
        // 셀 하이라이트
        for (let row = 0; row < this._cells.length; row++) {
            for (let col = 0; col < this._cells[row].length; col++) {
                const cell = this._cells[row][col];
                if (row === this._engineSelRow && col === this._engineSelCol) {
                    cell.add_style_class_name('selected');
                } else {
                    cell.remove_style_class_name('selected');
                }
                if (row === this._mouseHoverRow && col === this._mouseHoverCol) {
                    cell.add_style_class_name('hovered');
                } else {
                    cell.remove_style_class_name('hovered');
                }
            }
        }
        // 활성 열 헤더 (엔진 선택 기준)
        for (let col = 0; col < this._colHeaders.length; col++) {
            if (col === this._engineSelCol) {
                this._colHeaders[col].add_style_class_name('active');
            } else {
                this._colHeaders[col].remove_style_class_name('active');
            }
        }
        // 활성 행 번호
        for (let row = 0; row < this._rowNumbers.length; row++) {
            if (row === this._engineSelRow) {
                this._rowNumbers[row].add_style_class_name('active');
            } else {
                this._rowNumbers[row].remove_style_class_name('active');
            }
        }
    }

    /**
     * 커서 위치 기반 팝업 포지셔닝 (특수문자 팝업과 동일).
     * @private
     */
    _positionPopup(cursorRect) {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natW] = this._container.get_preferred_width(-1);
        const [, natH] = this._container.get_preferred_height(-1);
        const popupWidth = natW > 0 ? natW : 480;
        const popupHeight = natH > 0 ? natH : 380;
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
}

