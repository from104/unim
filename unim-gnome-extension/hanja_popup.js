/**
 * UNIM 한자 후보 팝업
 *
 * 엔진 위임 모드 전용 순수 UI 컴포넌트.
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

/** 페이지당 최대 후보 수 */
const PAGE_SIZE = 9;

/**
 * HanjaPopup
 *
 * 한자 후보 리스트를 플로팅 패널로 표시한다.
 * 키 처리 로직 없음 — 엔진 시그널로만 상태 갱신.
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
        /** @type {St.Label[]} 후보 행 위젯들 */
        this._rows = [];

        /** @type {Array<{hanja: string, meaning: string}>} 전체 후보 */
        this._candidates = [];
        /** @type {string} 대상 문자 */
        this._target = '';

        /** @type {number} 현재 페이지 (엔진이 관리, updateFromNavigate로 수신) */
        this._currentPage = 0;
        /** @type {number} 전체 페이지 수 (엔진이 관리) */
        this._totalPages = 1;
        /** @type {number} 엔진이 관리하는 선택 인덱스 (페이지 내) */
        this._engineSelectedIndex = 0;
        /** @type {number} 마우스 호버 인덱스 (-1이면 비활성) */
        this._mouseHoverIndex = -1;

        /** @type {Function|null} 선택 콜백 (globalIndex: number) => void */
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
            reactive: true,
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
        this._onSelect = onSelect;
        this._onCancel = onCancel;

        // 초기 상태 (엔진의 첫 PopupNavigate 시그널이 곧 덮어씀)
        this._currentPage = 0;
        this._totalPages = Math.ceil(candidates.length / PAGE_SIZE);
        this._engineSelectedIndex = 0;
        this._mouseHoverIndex = -1;

        this._header.set_text(`「${target}」 → 한자`);
        this._updateList();

        // 먼저 표시하여 실제 크기 측정 가능하게 함
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
        this._candidates = [];
        this._mouseHoverIndex = -1;
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
     * @param {number} selected - 선택된 인덱스 (페이지 내)
     */
    updateFromNavigate(page, totalPages, selected) {
        if (!this.isVisible) return;

        const pageChanged = this._currentPage !== page;
        this._currentPage = page;
        this._totalPages = totalPages;
        this._engineSelectedIndex = selected;

        if (pageChanged) {
            this._updateList();
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
     *
     * 글자를 가리지 않도록 커서 위에 표시.
     * 공간이 부족하면 아래로.
     *
     * @param {{x: number, y: number, width: number, height: number}} [cursorRect]
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
            // 기본: 커서 아래에 표시
            y = cursorRect.y + cursorRect.height + 4;

            // 아래 공간 부족 시 커서 위로
            if (y + popupHeight > monitor.y + monitor.height) {
                y = cursorRect.y - popupHeight - 4;
            }
        } else {
            x = Math.floor(monitor.x + (monitor.width - popupWidth) / 2);
            y = Math.floor(monitor.y + 100);
        }

        // 오른쪽 경계
        if (x + popupWidth > monitor.x + monitor.width) {
            x = monitor.x + monitor.width - popupWidth;
        }
        // 왼쪽/위쪽 최소
        x = Math.max(monitor.x, x);
        y = Math.max(monitor.y, y);

        this._container.set_position(x, y);
    }

    /**
     * 현재 페이지의 후보 수
     * @returns {number}
     * @private
     */
    _pageItemCount() {
        const start = this._currentPage * PAGE_SIZE;
        return Math.min(PAGE_SIZE, this._candidates.length - start);
    }

    /**
     * 후보 리스트 렌더링
     * @private
     */
    _updateList() {
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

            // 마우스 호버: 로컬 시각 효과만 (엔진 상태 변경 없음)
            const rowIndex = i;
            row.connect('enter-event', () => {
                this._mouseHoverIndex = rowIndex;
                this._updateSelection();
                return Clutter.EVENT_STOP;
            });
            row.connect('leave-event', () => {
                this._mouseHoverIndex = -1;
                this._updateSelection();
                return Clutter.EVENT_STOP;
            });

            // 마우스 클릭: 선택 콜백
            const globalIdx = start + i;
            row.connect('button-press-event', () => {
                this._selectCandidate(globalIdx);
                return Clutter.EVENT_STOP;
            });

            this._list.add_child(row);
            this._rows.push(row);
        }

        // 페이지 표시
        if (this._totalPages > 1) {
            this._footer.set_text(`${this._currentPage + 1}/${this._totalPages}`);
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
        for (let i = 0; i < this._rows.length; i++) {
            // 엔진 선택 하이라이트
            if (i === this._engineSelectedIndex) {
                this._rows[i].add_style_class_name('selected');
            } else {
                this._rows[i].remove_style_class_name('selected');
            }
            // 마우스 호버 표시 (선택과 독립)
            if (i === this._mouseHoverIndex) {
                this._rows[i].add_style_class_name('hovered');
            } else {
                this._rows[i].remove_style_class_name('hovered');
            }
        }
    }

    /**
     * 후보 선택 → 콜백 호출
     *
     * hide()는 여기서 호출하지 않음.
     * 엔진이 SelectHanja 처리 후 HidePopup 시그널을 보내면 그때 닫힘.
     * 여기서 hide()를 호출하면 콜백 내 DBus 호출과 경쟁 조건이 발생할 수 있음.
     *
     * @param {number} globalIndex
     * @private
     */
    _selectCandidate(globalIndex) {
        if (globalIndex < this._candidates.length && this._onSelect) {
            this._onSelect(globalIndex);
        }
    }
}
