/**
 * UNIM 이모지 팝업 (GNOME Shell Extension)
 *
 * Super+. 단축키 트리거 시 DBus `ShowEmojiPopup` 시그널을 받아 카테고리 탭 +
 * 검색 + 즐겨찾기(MRU)를 가진 이모지 선택 UI를 compositor 위에 표시한다.
 *
 * - 카테고리 데이터: `UnimDbusIME.listEmojiCategories()`  (서비스 측 단일 진실 공급원)
 * - 즐겨찾기 MRU  : `UnimDbusIME.getEmojiFavorites()`     (엔진이 commit 시 자동 갱신)
 * - 검색          : `UnimDbusIME.searchEmoji(keyword)`
 * - 선택          : `UnimDbusIME.commitEmoji(emoji)`      (HidePopup + MRU 갱신 포함)
 *
 * @module emoji_popup
 */

import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog } from './logging.js';

const COLS = 8;
const FAVORITES_TAB = '즐겨찾기';
const SEARCH_TAB = '🔍 검색';
const FLASH_DURATION_MS = 100;

/**
 * EmojiPopup
 *
 * GNOME Shell St 위젯 기반 이모지 선택 팝업.
 * 한자/특수문자 팝업과 달리 엔진에서 상태를 관리하지 않고 GUI가 자체적으로
 * 카테고리 선택·검색·즐겨찾기를 제어한다.
 */
export class EmojiPopup {
    /**
     * @param {import('./dbus_ime.js').UnimDbusIME} dbusIME
     */
    constructor(dbusIME) {
        this._dbusIME = dbusIME;
        /** @type {St.BoxLayout|null} */
        this._container = null;
        /** @type {St.Entry|null} */
        this._searchEntry = null;
        /** @type {St.BoxLayout|null} 탭 헤더 */
        this._tabBar = null;
        /** @type {St.BoxLayout|null} 현재 탭 이모지 그리드 */
        this._grid = null;
        /** @type {St.Label|null} */
        this._footer = null;

        /** @type {Array<{name: string, emojis: string[]}>} 카테고리 (첫 항목 = 즐겨찾기) */
        this._categories = [];
        /** @type {string} 현재 선택된 탭 이름 */
        this._currentTab = FAVORITES_TAB;
        /** @type {string} 검색 키워드 */
        this._searchText = '';
        /** @type {number} 현재 탭 내 선택 인덱스 */
        this._selIndex = 0;
        /** @type {string[]} 현재 탭에 표시 중인 이모지 (네비게이션 대상) */
        this._visibleEmojis = [];

        /** @type {boolean} 플래시 대기 중 */
        this._pendingCommit = false;
        /** @type {number|null} 검색 debounce 타임아웃 ID */
        this._searchDebounceId = null;
        /** @type {Clutter.Grab|null} 활성 modal grab */
        this._modalGrab = null;
    }

    /**
     * 위젯 초기화
     */
    enable() {
        this._container = new St.BoxLayout({
            style_class: 'unim-emoji-popup',
            vertical: true,
            visible: false,
            reactive: true,
            can_focus: true,
            track_hover: true,
        });

        // 팝업 자체에도 key handler를 걸어 search entry 외부에서도 ESC 동작
        this._container.connect('key-press-event', (_actor, event) => {
            if (event.get_key_symbol() === Clutter.KEY_Escape) {
                this.hide();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        // 검색 엔트리
        this._searchEntry = new St.Entry({
            style_class: 'emoji-search',
            hint_text: '이모지 검색 (예: 사랑, 동물, 음식)',
            can_focus: true,
        });
        this._searchEntry.clutter_text.connect('text-changed', () => {
            this._onSearchChanged();
        });
        this._searchEntry.clutter_text.connect('key-press-event', (actor, event) => {
            return this._onSearchKey(event);
        });
        this._container.add_child(this._searchEntry);

        // 탭 바
        this._tabBar = new St.BoxLayout({
            style_class: 'emoji-tabs',
            vertical: false,
            x_align: Clutter.ActorAlign.CENTER,
        });
        this._container.add_child(this._tabBar);

        // 그리드
        this._grid = new St.BoxLayout({
            style_class: 'emoji-grid',
            vertical: true,
        });
        this._container.add_child(this._grid);

        // 풋터
        this._footer = new St.Label({
            style_class: 'popup-footer',
            x_align: Clutter.ActorAlign.CENTER,
        });
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
        this._searchEntry = null;
        this._tabBar = null;
        this._grid = null;
        this._footer = null;
    }

    /**
     * 팝업 표시 여부
     * @returns {boolean}
     */
    get isVisible() {
        return this._container?.visible ?? false;
    }

    /**
     * 이모지 팝업 표시
     * @param {{x:number,y:number,width:number,height:number}} [cursorRect]
     */
    show(cursorRect) {
        if (!this._container) return;

        // 카테고리/즐겨찾기 최신화
        this._categories = this._dbusIME?.listEmojiCategories() ?? [];
        if (this._categories.length === 0) {
            unimLog('EMOJI', '카테고리 로드 실패');
            return;
        }

        const favorites = this._dbusIME?.getEmojiFavorites() ?? [];
        // 첫 항목이 "즐겨찾기" (서비스에서 기본 인기 이모지 반환). 사용자 MRU가 있으면 그것으로 대체.
        if (favorites.length > 0 && this._categories.length > 0) {
            this._categories[0] = { name: FAVORITES_TAB, emojis: favorites };
        } else {
            this._categories[0] = { name: FAVORITES_TAB, emojis: this._categories[0].emojis };
        }

        this._rebuildTabBar();

        this._currentTab = FAVORITES_TAB;
        this._searchText = '';
        if (this._searchEntry) this._searchEntry.set_text('');
        this._selIndex = 0;
        this._showTab(FAVORITES_TAB);

        this._container.show();
        this._positionPopup(cursorRect);

        // modal grab — 팝업이 열린 동안 KeyHandler vfunc을 우회하여 키 입력을
        // 팝업의 search entry로 직접 라우팅 (엔진에 가지 않음)
        this._acquireModalGrab();

        if (this._searchEntry) {
            this._searchEntry.grab_key_focus();
        }
        unimLog('EMOJI', `팝업 표시 (카테고리 ${this._categories.length}개)`);
    }

    /** @private */
    _acquireModalGrab() {
        if (this._modalGrab || !this._container) return;
        try {
            this._modalGrab = Main.pushModal(this._container, {
                actionMode: Shell.ActionMode.NORMAL,
            });
            // pushModal 반환이 객체(Grab)면 성공, 0이면 실패 (구형 API)
            if (typeof this._modalGrab === 'number' && this._modalGrab === 0) {
                this._modalGrab = null;
                unimLog('EMOJI', 'pushModal 실패 (fallback: key focus only)');
            }
        } catch (e) {
            this._modalGrab = null;
            unimLog('EMOJI', `pushModal 예외: ${e.message}`);
        }
    }

    /** @private */
    _releaseModalGrab() {
        if (!this._modalGrab) return;
        try {
            Main.popModal(this._modalGrab);
        } catch (e) {
            unimLog('EMOJI', `popModal 예외: ${e.message}`);
        }
        this._modalGrab = null;
    }

    /**
     * 팝업 숨김
     */
    hide() {
        if (this._searchDebounceId) {
            GLib.source_remove(this._searchDebounceId);
            this._searchDebounceId = null;
        }
        this._releaseModalGrab();
        if (this._container) {
            this._container.hide();
        }
        this._pendingCommit = false;
    }

    // ===========================================
    // 탭 관리
    // ===========================================

    _rebuildTabBar() {
        this._tabBar.destroy_all_children();
        for (const cat of this._categories) {
            const btn = new St.Button({
                style_class: 'emoji-tab',
                label: cat.name,
                can_focus: false,
                reactive: true,
            });
            const tabName = cat.name;
            btn.connect('clicked', () => {
                this._currentTab = tabName;
                this._showTab(tabName);
            });
            this._tabBar.add_child(btn);
        }
    }

    /** @private */
    _showTab(name) {
        const cat = this._categories.find(c => c.name === name);
        if (!cat) return;
        this._visibleEmojis = cat.emojis.slice();
        this._selIndex = 0;
        this._renderGrid(`${name} (${cat.emojis.length})`);
        this._highlightActiveTab(name);
    }

    /** @private */
    _highlightActiveTab(name) {
        if (!this._tabBar) return;
        let idx = 0;
        for (const btn of this._tabBar.get_children()) {
            if (this._categories[idx]?.name === name) {
                btn.add_style_class_name('active');
            } else {
                btn.remove_style_class_name('active');
            }
            idx++;
        }
    }

    // ===========================================
    // 검색
    // ===========================================

    _onSearchChanged() {
        const keyword = this._searchEntry?.get_text() ?? '';
        this._searchText = keyword;

        // 100ms debounce — 빠른 타이핑 시 매 타자 DBus 호출 방지
        if (this._searchDebounceId) {
            GLib.source_remove(this._searchDebounceId);
        }
        this._searchDebounceId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100, () => {
            this._searchDebounceId = null;
            if (!this.isVisible) return GLib.SOURCE_REMOVE;

            if (keyword === '') {
                // 빈 검색 → 즐겨찾기 탭으로 복귀
                this._currentTab = FAVORITES_TAB;
                this._showTab(FAVORITES_TAB);
                return GLib.SOURCE_REMOVE;
            }

            const results = this._dbusIME?.searchEmoji(keyword) ?? [];
            this._currentTab = SEARCH_TAB;
            this._visibleEmojis = results;
            this._selIndex = 0;
            this._renderGrid(`${SEARCH_TAB} "${keyword}" (${results.length})`);
            this._highlightActiveTab(null); // 검색 중엔 카테고리 탭 강조 해제
            return GLib.SOURCE_REMOVE;
        });
    }

    _onSearchKey(event) {
        const sym = event.get_key_symbol();
        if (sym === Clutter.KEY_Escape) {
            this.hide();
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Return || sym === Clutter.KEY_KP_Enter) {
            if (this._selIndex < this._visibleEmojis.length) {
                this._commitAt(this._selIndex);
            }
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Tab) {
            this._cycleTab(1);
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_ISO_Left_Tab) {
            this._cycleTab(-1);
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Left) {
            this._moveSelection(-1);
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Right) {
            this._moveSelection(1);
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Up) {
            this._moveSelection(-COLS);
            return Clutter.EVENT_STOP;
        }
        if (sym === Clutter.KEY_Down) {
            this._moveSelection(COLS);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _cycleTab(direction) {
        if (this._categories.length === 0) return;
        let idx = this._categories.findIndex(c => c.name === this._currentTab);
        if (idx < 0) idx = 0;
        idx = (idx + direction + this._categories.length) % this._categories.length;
        this._currentTab = this._categories[idx].name;
        if (this._searchEntry) this._searchEntry.set_text('');
        this._showTab(this._currentTab);
    }

    _moveSelection(delta) {
        if (this._visibleEmojis.length === 0) return;
        let next = this._selIndex + delta;
        if (next < 0) next = 0;
        if (next >= this._visibleEmojis.length) next = this._visibleEmojis.length - 1;
        this._selIndex = next;
        this._updateSelectionHighlight();
    }

    // ===========================================
    // 그리드 렌더링
    // ===========================================

    /**
     * 그리드 재구성 + 풋터 갱신
     * @param {string} caption 풋터 텍스트
     * @private
     */
    _renderGrid(caption) {
        this._grid.destroy_all_children();

        if (this._visibleEmojis.length === 0) {
            const empty = new St.Label({
                style_class: 'emoji-empty',
                text: '(항목 없음)',
                x_align: Clutter.ActorAlign.CENTER,
            });
            this._grid.add_child(empty);
        } else {
            for (let row = 0; row * COLS < this._visibleEmojis.length; row++) {
                const rowBox = new St.BoxLayout({ style_class: 'emoji-row' });
                for (let col = 0; col < COLS; col++) {
                    const idx = row * COLS + col;
                    if (idx >= this._visibleEmojis.length) break;
                    const emoji = this._visibleEmojis[idx];
                    const cell = new St.Button({
                        style_class: 'emoji-cell',
                        label: emoji,
                        reactive: true,
                        can_focus: false,
                    });
                    cell.connect('clicked', () => {
                        this._selIndex = idx;
                        this._commitAt(idx);
                    });
                    cell.connect('enter-event', () => {
                        this._selIndex = idx;
                        this._updateSelectionHighlight();
                        return Clutter.EVENT_PROPAGATE;
                    });
                    rowBox.add_child(cell);
                }
                this._grid.add_child(rowBox);
            }
        }

        if (this._footer) {
            this._footer.set_text(caption || '');
        }

        this._updateSelectionHighlight();
    }

    _updateSelectionHighlight() {
        if (!this._grid) return;
        let idx = 0;
        for (const row of this._grid.get_children()) {
            if (!row.get_children) continue;
            for (const cell of row.get_children()) {
                if (idx === this._selIndex) {
                    cell.add_style_class_name('selected');
                } else {
                    cell.remove_style_class_name('selected');
                }
                idx++;
            }
        }
    }

    /**
     * 지정 인덱스의 이모지를 커밋.
     * 플래시 후 DBus CommitEmoji 호출. 서비스가 HidePopup 시그널을 보내면 숨김.
     * @param {number} idx
     * @private
     */
    _commitAt(idx) {
        const emoji = this._visibleEmojis[idx];
        if (!emoji || this._pendingCommit) return;
        this._pendingCommit = true;
        this._selIndex = idx;
        this._updateSelectionHighlight();

        GLib.timeout_add(GLib.PRIORITY_DEFAULT, FLASH_DURATION_MS, () => {
            this._dbusIME?.commitEmoji(emoji);
            // CommitEmoji가 HidePopup 시그널을 보내 extension.js가 hide()를 호출.
            // 방어적으로 여기서도 숨김 (시그널 누락 시).
            this.hide();
            return GLib.SOURCE_REMOVE;
        });
    }

    // ===========================================
    // 포지셔닝
    // ===========================================

    /**
     * 커서 위치 기반 팝업 배치 (SpecialPopup과 동일 규칙)
     * @private
     */
    _positionPopup(cursorRect) {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natW] = this._container.get_preferred_width(-1);
        const [, natH] = this._container.get_preferred_height(-1);
        const popupWidth = natW > 0 ? natW : 400;
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
}
