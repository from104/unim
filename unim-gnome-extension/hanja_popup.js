/**
 * UNIM 한자 후보 팝업
 *
 * 엔진 위임 모드 전용 순수 UI 컴포넌트.
 * Compact 모드(1×9 리스트)와 확장 모드(최대 9×9 그리드)를 모두 지원하며,
 * 엔진이 `cols`/`rows` 값을 통해 모드를 전달한다.
 *
 * 모든 키 처리는 엔진(ProcessKeyEvent)이 담당하고,
 * 팝업은 렌더링과 마우스 이벤트 콜백만 담당한다.
 *
 * 상태 업데이트는 updateFromNavigate() 시그널이 유일한 경로.
 *
 * @see POPUP_SPEC.md
 * @module hanja_popup
 */

import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

/** 그리드 상수 (확장 모드) */
const MAX_ROWS = 9;
const MAX_COLS = 9;
const EXPANDED_PAGE_SIZE = MAX_ROWS * MAX_COLS; // 81
const COMPACT_PAGE_SIZE = 9;

/** 확장 아이콘 텍스트 (⊞ expand / ⊟ compact) */
const ICON_EXPAND = '⊞';
const ICON_COMPACT = '⊟';

/**
 * HanjaPopup
 *
 * 한자 후보를 compact 리스트 또는 확장 그리드로 표시.
 * 키 처리 로직 없음 — 엔진 시그널로만 상태 갱신.
 */
export class HanjaPopup {
    constructor() {
        /** @type {St.BoxLayout|null} 루트 위젯 */
        this._container = null;
        /** @type {St.Label} 헤더 (대상 문자 표시) */
        this._header = null;
        /** @type {St.BoxLayout} 후보 리스트/그리드 컨테이너 */
        this._body = null;
        /** @type {St.Label} 뜻풀이 표시 라벨 (확장 모드 전용 툴팁) */
        this._meaningStrip = null;
        /** @type {St.BoxLayout} 풋터 컨테이너 (확장 아이콘 + 페이지) */
        this._footer = null;
        /** @type {St.Label} 확장/축소 토글 아이콘 */
        this._expandIcon = null;
        /** @type {St.Label} 페이지 표시 라벨 */
        this._pageLabel = null;

        /** @type {object[]} 렌더된 행/셀 위젯들 (compact: rows, expanded: cells) */
        this._widgets = [];
        /** @type {St.Label[]} 확장 모드 가로 레이블 헤더 (sel_col 하이라이트용) */
        this._colHeaders = [];
        /** @type {string} 확장 모드 가로 레이블 키 시퀀스 (special과 동일) */
        this._topRow = 'QWERTYUIO';

        /** @type {Array<{hanja: string, meaning: string}>} 전체 후보 */
        this._candidates = [];
        /** @type {boolean[]} 후보별 즐겨찾기 상태 */
        this._bookmarks = [];
        /** @type {string} 대상 문자 */
        this._target = '';

        /** @type {number} 현재 페이지 (엔진이 관리) */
        this._currentPage = 0;
        /** @type {number} 전체 페이지 수 */
        this._totalPages = 1;
        /** @type {number} 페이지 행 수 (엔진 PopupNavigate 전달) */
        this._rows = COMPACT_PAGE_SIZE;
        /** @type {number} 페이지 열 수 (1=compact, >1=expanded) */
        this._cols = 1;
        /** @type {number} 엔진 선택 행 */
        this._selRow = 0;
        /** @type {number} 엔진 선택 열 (compact에서는 항상 0) */
        this._selCol = 0;
        /** @type {number} 마우스 호버 행 (-1=비활성) */
        this._mouseHoverRow = -1;
        /** @type {number} 마우스 호버 열 (-1=비활성) */
        this._mouseHoverCol = -1;

        /** @type {Function|null} 선택 콜백 (globalIndex) */
        this._onSelect = null;
        /** @type {Function|null} 취소 콜백 */
        this._onCancel = null;
        /** @type {Function|null} 즐겨찾기 토글 콜백 (globalIndex) — 우클릭용 */
        this._onToggleBookmark = null;
        /** @type {Function|null} 확장/축소 토글 콜백 — 확장 아이콘 클릭용 */
        this._onToggleExpand = null;
    }

    /**
     * 위젯 초기화
     */
    enable() {
        this._container = new St.BoxLayout({
            style_class: 'unim-hanja-popup',
            vertical: true,
            visible: false,
            reactive: true,
        });

        this._header = new St.Label({ style_class: 'popup-header' });
        this._container.add_child(this._header);

        this._body = new St.BoxLayout({ vertical: true });
        this._container.add_child(this._body);

        // 확장 모드에서 hover 중인 셀의 뜻풀이를 표시하는 스트립
        this._meaningStrip = new St.Label({
            style_class: 'popup-meaning-strip',
            text: '',
            visible: false,
        });
        this._container.add_child(this._meaningStrip);

        // 풋터: [⊞] ←→ [page/total]
        this._footer = new St.BoxLayout({
            style_class: 'popup-footer-box',
            vertical: false,
        });
        this._expandIcon = new St.Label({
            style_class: 'popup-expand-icon',
            text: ICON_EXPAND,
            reactive: true,
            track_hover: true,
        });
        this._expandIcon.connect('button-press-event', () => {
            if (this._onToggleExpand) {
                this._onToggleExpand();
            }
            return Clutter.EVENT_STOP;
        });
        this._pageLabel = new St.Label({
            style_class: 'popup-footer',
            x_expand: true,
            x_align: Clutter.ActorAlign.END,
        });
        this._footer.add_child(this._expandIcon);
        this._footer.add_child(this._pageLabel);
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
     * @param {Function} onSelect - 선택 콜백 (globalIndex)
     * @param {Function} onCancel - 취소 콜백
     * @param {{x: number, y: number, width: number, height: number}} [cursorRect]
     * @param {boolean[]} [bookmarks] - 후보별 즐겨찾기 플래그
     * @param {Function} [onToggleBookmark] - 우클릭 시 호출 (globalIndex)
     * @param {Function} [onToggleExpand] - 확장 아이콘 클릭 시 호출
     */
    show(target, candidates, onSelect, onCancel, cursorRect, bookmarks,
         onToggleBookmark, onToggleExpand) {
        if (!this._container || candidates.length === 0) return;

        this._target = target;
        this._candidates = candidates;
        this._bookmarks = Array.isArray(bookmarks)
            ? bookmarks.slice(0, candidates.length)
            : [];
        while (this._bookmarks.length < candidates.length) {
            this._bookmarks.push(false);
        }
        this._onSelect = onSelect;
        this._onCancel = onCancel;
        this._onToggleBookmark = onToggleBookmark || null;
        this._onToggleExpand = onToggleExpand || null;

        // 초기 상태 (엔진의 첫 PopupNavigate가 즉시 덮어씀)
        this._currentPage = 0;
        this._totalPages = Math.ceil(candidates.length / COMPACT_PAGE_SIZE);
        this._rows = Math.min(COMPACT_PAGE_SIZE, candidates.length);
        this._cols = 1;
        this._selRow = 0;
        this._selCol = 0;
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;

        this._header.set_text(`「${target}」 → 한자`);
        this._renderBody();

        this._container.show();
        this._positionPopup(cursorRect);

        unimLog('HANJA', `팝업 표시: target="${target}", ${candidates.length}개 후보`);
    }

    /**
     * 팝업 숨김
     */
    hide() {
        if (this._container) {
            this._container.hide();
        }
        if (this._meaningStrip) {
            this._meaningStrip.hide();
        }
        this._candidates = [];
        this._bookmarks = [];
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;
        this._onSelect = null;
        this._onCancel = null;
        this._onToggleBookmark = null;
        this._onToggleExpand = null;
    }

    /**
     * 특정 후보의 즐겨찾기 상태를 갱신합니다.
     *
     * 엔진의 HanjaBookmarkChanged 시그널로 호출됨.
     *
     * @param {number} globalIndex - 전체 후보 인덱스 (0-based)
     * @param {boolean} bookmarked - 새 상태
     */
    setBookmark(globalIndex, bookmarked) {
        if (globalIndex < 0 || globalIndex >= this._candidates.length) return;
        this._bookmarks[globalIndex] = !!bookmarked;
        this._refreshBookmarkAt(globalIndex);
    }

    /**
     * 한자 후보를 즐겨찾기 정렬 결과로 통째 교체하고 커서를 점프시킨다.
     *
     * 엔진의 HanjaCandidatesReordered 시그널로 호출됨. SelectHanja 인덱스
     * 미스매치를 피하기 위해 호출자는 이 함수가 끝난 다음에야 selection을 보낼 수 있다.
     *
     * @param {Array<{hanja: string, meaning: string}>} candidates
     * @param {Array<boolean>} bookmarks
     * @param {number} page
     * @param {number} selRow
     * @param {number} selCol
     */
    setCandidates(candidates, bookmarks, page, selRow, selCol) {
        if (!this.isVisible) return;
        this._candidates = Array.isArray(candidates) ? candidates : [];
        this._bookmarks = Array.isArray(bookmarks)
            ? bookmarks.slice(0, this._candidates.length)
            : [];
        while (this._bookmarks.length < this._candidates.length) {
            this._bookmarks.push(false);
        }
        this._currentPage = Math.max(0, page | 0);
        this._mouseHoverRow = -1;
        this._mouseHoverCol = -1;
        this._selRow = Math.max(0, selRow | 0);
        this._selCol = Math.max(0, selCol | 0);
        this._renderBody();
    }

    /**
     * 팝업 표시 여부
     * @returns {boolean}
     */
    get isVisible() {
        return this._container?.visible ?? false;
    }

    /**
     * 엔진 PopupNavigate 시그널로 상태 업데이트
     *
     * @param {number} page
     * @param {number} totalPages
     * @param {number} _selected - 하위 호환용 (sel_row와 동일)
     * @param {number} rows
     * @param {number} cols
     * @param {number} selRow
     * @param {number} selCol
     */
    updateFromNavigate(page, totalPages, _selected, rows, cols, selRow, selCol) {
        if (!this.isVisible) return;

        const layoutChanged =
            this._currentPage !== page ||
            this._cols !== cols ||
            this._rows !== rows;

        this._currentPage = page;
        this._totalPages = totalPages;
        this._rows = rows > 0 ? rows : 1;
        this._cols = cols > 0 ? cols : 1;
        this._selRow = selRow;
        this._selCol = selCol;

        if (layoutChanged) {
            this._renderBody();
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
     * @private
     */
    _positionPopup(cursorRect) {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natW] = this._container.get_preferred_width(-1);
        const [, natH] = this._container.get_preferred_height(-1);
        const popupWidth = natW > 0 ? natW : 280;
        const popupHeight = natH > 0 ? natH : 300;
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
     * 현재 페이지의 페이지 크기 (compact=9, expanded=81)
     * @private
     */
    _pageSize() {
        return this._cols > 1 ? EXPANDED_PAGE_SIZE : COMPACT_PAGE_SIZE;
    }

    /**
     * (row, col) → 전체 인덱스
     * @private
     */
    _globalIndex(row, col) {
        const pageStart = this._currentPage * this._pageSize();
        const pageOffset = this._cols > 1 ? col * this._rows + row : row;
        const g = pageStart + pageOffset;
        return g < this._candidates.length ? g : -1;
    }

    /**
     * 본문(리스트 or 그리드) 재렌더
     * @private
     */
    _renderBody() {
        this._body.destroy_all_children();
        this._widgets = [];
        this._colHeaders = [];

        if (this._cols > 1) {
            this._renderGrid();
        } else {
            this._renderList();
        }

        // 확장 아이콘 상태
        this._expandIcon.set_text(this._cols > 1 ? ICON_COMPACT : ICON_EXPAND);

        // compact 모드로 전환되면 뜻풀이 스트립 숨김
        if (this._cols <= 1 && this._meaningStrip) {
            this._meaningStrip.hide();
        }

        // 페이지 표시
        if (this._totalPages > 1) {
            this._pageLabel.set_text(`${this._currentPage + 1}/${this._totalPages}`);
        } else {
            this._pageLabel.set_text('');
        }

        this._updateSelection();
    }

    /**
     * Compact 리스트 렌더 (1 × rows)
     * @private
     */
    _renderList() {
        const start = this._currentPage * COMPACT_PAGE_SIZE;
        const count = Math.min(this._rows, this._candidates.length - start);

        for (let i = 0; i < count; i++) {
            const globalIdx = start + i;
            const candidate = this._candidates[globalIdx];
            const row = new St.BoxLayout({
                style_class: 'popup-item',
                reactive: true,
                track_hover: true,
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
            // 첫 렌더에서 St 멀티 클래스 토큰화 이슈를 피해 단일 클래스로 시작.
            // 'bookmarked' 토큰은 아래 _refreshAllBookmarks()에서 일괄 add한다.
            const star = new St.Label({
                style_class: 'item-bookmark',
                text: '☆',
            });

            row.add_child(num);
            row.add_child(hanja);
            row.add_child(meaning);
            row.add_child(star);

            const r = i;
            row.connect('enter-event', () => {
                this._mouseHoverRow = r;
                this._mouseHoverCol = 0;
                this._updateSelection();
                return Clutter.EVENT_STOP;
            });
            row.connect('leave-event', () => {
                this._mouseHoverRow = -1;
                this._mouseHoverCol = -1;
                this._updateSelection();
                return Clutter.EVENT_STOP;
            });
            row.connect('button-press-event', (_, event) => {
                const button = event.get_button();
                if (button === Clutter.BUTTON_SECONDARY) {
                    this._toggleBookmarkAt(globalIdx);
                } else {
                    this._selectCandidate(globalIdx);
                }
                return Clutter.EVENT_STOP;
            });

            this._body.add_child(row);
            this._widgets.push({ row, star, globalIdx, r: i, c: 0 });
        }

        // 즐겨찾기 반영 (compact는 row 자체에 bookmarked 클래스도 부여 → 액센트 배경)
        this._refreshAllBookmarks();
    }

    /**
     * 확장 그리드 렌더 (rows × cols, 열 우선 인덱싱)
     * @private
     */
    _renderGrid() {
        // 가로 레이블 헤더 행 (special과 동일: 빈 row-number 셀 + topRow 문자들)
        const headerRow = new St.BoxLayout({ style_class: 'grid-row' });
        headerRow.add_child(new St.Label({
            style_class: 'grid-row-number',
            text: '',
        }));
        for (let col = 0; col < this._cols; col++) {
            const headerChar = col < this._topRow.length ? this._topRow[col] : '';
            const headerLabel = new St.Label({
                style_class: 'grid-header',
                text: headerChar,
                x_align: Clutter.ActorAlign.CENTER,
            });
            headerRow.add_child(headerLabel);
            this._colHeaders.push(headerLabel);
        }
        this._body.add_child(headerRow);

        for (let row = 0; row < this._rows; row++) {
            const rowWidget = new St.BoxLayout({ style_class: 'grid-row' });
            const rowNumber = new St.Label({
                style_class: 'grid-row-number',
                text: `${row + 1}`,
            });
            rowWidget.add_child(rowNumber);

            for (let col = 0; col < this._cols; col++) {
                const idx = this._globalIndex(row, col);
                const candidate = idx >= 0 ? this._candidates[idx] : null;
                const cell = new St.Label({
                    style_class: 'grid-cell',
                    text: candidate ? candidate.hanja : '',
                    x_align: Clutter.ActorAlign.CENTER,
                    reactive: candidate !== null,
                    track_hover: candidate !== null,
                });

                if (candidate) {
                    const r = row;
                    const c = col;
                    const globalIdx = idx;

                    cell.connect('enter-event', () => {
                        this._mouseHoverRow = r;
                        this._mouseHoverCol = c;
                        this._updateSelection();
                        this._showMeaningFor(globalIdx);
                        return Clutter.EVENT_STOP;
                    });
                    cell.connect('leave-event', () => {
                        this._mouseHoverRow = -1;
                        this._mouseHoverCol = -1;
                        this._updateSelection();
                        this._showMeaningFor(-1);
                        return Clutter.EVENT_STOP;
                    });
                    cell.connect('button-press-event', (_, event) => {
                        const button = event.get_button();
                        if (button === Clutter.BUTTON_SECONDARY) {
                            this._toggleBookmarkAt(globalIdx);
                        } else {
                            this._selectCandidate(globalIdx);
                        }
                        return Clutter.EVENT_STOP;
                    });

                    this._widgets.push({ row: rowWidget, cell, globalIdx, r, c });
                }

                rowWidget.add_child(cell);
            }

            this._body.add_child(rowWidget);
        }

        this._refreshAllBookmarks();
    }

    /**
     * 선택/호버 하이라이트 갱신
     * @private
     */
    _updateSelection() {
        let selectedGlobal = -1;
        for (const w of this._widgets) {
            const target = w.cell || w.row;
            if (!target) continue;
            if (w.r === this._selRow && w.c === this._selCol) {
                target.add_style_class_name('selected');
                selectedGlobal = w.globalIdx;
            } else {
                target.remove_style_class_name('selected');
            }
            if (w.r === this._mouseHoverRow && w.c === this._mouseHoverCol) {
                target.add_style_class_name('hovered');
            } else {
                target.remove_style_class_name('hovered');
            }
        }
        // 활성 열 헤더 하이라이트 (sel_col 기준, special과 동일)
        for (let col = 0; col < this._colHeaders.length; col++) {
            if (col === this._selCol) {
                this._colHeaders[col].add_style_class_name('active');
            } else {
                this._colHeaders[col].remove_style_class_name('active');
            }
        }
        // 확장 모드: 마우스 호버가 없으면 엔진 선택 항목의 뜻풀이를 표시
        if (this._cols > 1 && this._mouseHoverRow < 0) {
            this._showMeaningFor(selectedGlobal);
        }
    }

    /**
     * 주어진 전체 인덱스의 뜻풀이를 스트립에 표시.
     * index < 0 또는 meaning이 없으면 스트립 숨김.
     * compact 모드에서는 항상 숨긴다 (뜻풀이가 행에 인라인 표시됨).
     * @private
     */
    _showMeaningFor(globalIndex) {
        if (!this._meaningStrip) return;
        if (this._cols <= 1) {
            this._meaningStrip.hide();
            return;
        }
        const candidate = globalIndex >= 0 ? this._candidates[globalIndex] : null;
        if (candidate && candidate.meaning) {
            this._meaningStrip.set_text(`${candidate.hanja}  ${candidate.meaning}`);
            this._meaningStrip.show();
        } else {
            this._meaningStrip.set_text('');
            this._meaningStrip.hide();
        }
    }

    /**
     * 전체 즐겨찾기 표시 갱신
     * @private
     */
    _refreshAllBookmarks() {
        for (const w of this._widgets) {
            this._applyBookmarkStyle(w, !!this._bookmarks[w.globalIdx]);
        }
    }

    /**
     * 특정 전체 인덱스의 즐겨찾기 표시 갱신
     * @param {number} globalIndex
     * @private
     */
    _refreshBookmarkAt(globalIndex) {
        for (const w of this._widgets) {
            if (w.globalIdx === globalIndex) {
                this._applyBookmarkStyle(w, !!this._bookmarks[globalIndex]);
                break;
            }
        }
    }

    /**
     * 한 위젯에 즐겨찾기 스타일 적용
     * @private
     */
    _applyBookmarkStyle(w, bookmarked) {
        // compact: row에 bookmarked 클래스 + 별 아이콘
        if (w.star) {
            w.star.set_text(bookmarked ? '★' : '☆');
            if (bookmarked) {
                w.star.add_style_class_name('bookmarked');
                w.row.add_style_class_name('bookmarked');
            } else {
                w.star.remove_style_class_name('bookmarked');
                w.row.remove_style_class_name('bookmarked');
            }
        }
        // expanded: 셀에 bookmarked 클래스만 (강조색 배경)
        if (w.cell) {
            if (bookmarked) {
                w.cell.add_style_class_name('bookmarked');
            } else {
                w.cell.remove_style_class_name('bookmarked');
            }
        }
    }

    /**
     * 후보 선택 → 콜백 호출
     * @private
     */
    _selectCandidate(globalIndex) {
        if (globalIndex < this._candidates.length && this._onSelect) {
            this._onSelect(globalIndex);
        }
    }

    /**
     * 즐겨찾기 토글 콜백 호출 (우클릭)
     * @private
     */
    _toggleBookmarkAt(globalIndex) {
        if (globalIndex < this._candidates.length && this._onToggleBookmark) {
            this._onToggleBookmark(globalIndex);
        }
    }
}
