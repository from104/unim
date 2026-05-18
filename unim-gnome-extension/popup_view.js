/**
 * UNIM 통합 Popup View (GNOME Shell extension)
 *
 * **단일 진실 공급원**: daemon 이 `PopupRender` 시그널로 보내는 평면 view-model 을
 * 그대로 St 위젯으로 렌더한다. 한자/특수문자/이모지 popup 분기는 daemon 이 모두
 * 결정하며, 본 view 는 다음만 책임진다:
 *
 *   1. Show*Popup signal → 위치 지정 + show (cursor 좌표 무시, 정중앙 고정)
 *   2. PopupRender signal → grid/headers/tabs/footer 갱신
 *   3. HidePopup signal → hide
 *   4. 마우스 클릭 → popup-service `org.atit.unim.Popup` RPC 호출
 *
 * 시각 디자인은 popup-service GTK4 popup 과 **100% 동일** — 클래스명·CSS 토큰을
 * `unim-popup-service/src/popup/popup_styles.generated.css` 와 일치시켜
 * stylesheet.css 에 동일 셀렉터로 정의했다. kind 별 루트 클래스를 toggle 한다.
 *
 * 자체 상태(페이지/선택/cands/북마크) 절대 보유 금지 — daemon 의 PopupRender 가
 * 항상 최신.
 *
 * @module popup_view
 */

import GLib from 'gi://GLib';
import St from 'gi://St';
import Pango from 'gi://Pango';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimError } from './logging.js';

/** ★ 해제 후 cursor 셀에 일시 적용되는 yellow flash 지속시간 (ms).
 *  popup_styles.generated.css `.bookmark-flash` 와 짝. */
const BOOKMARK_FLASH_DURATION_MS = 140;

// PopupRenderPayload cell flag 비트 — unim-popup-types/src/lib.rs::popup_render_flags 와 SoT 일치
// COL_HIGHLIGHT/ROW_HIGHLIGHT 는 row_headers/col_headers `.active` 로 표현되므로 셀 측에선 미사용.
const FLAG_HAS_DATA      = 0x01;
const FLAG_SELECTED      = 0x02;
const FLAG_BOOKMARKED    = 0x10;

// kind 코드 (unim-popup-types/src/lib.rs::PopupRenderPayload::kind)
const KIND_HANJA   = 0;
const KIND_SPECIAL = 1;
const KIND_EMOJI   = 2;

// kind → 루트 컨테이너 / 헤더 / 그리드 클래스 매핑. GTK popup_styles.generated.css 와
// 동일한 셀렉터를 사용해 stylesheet.css 한 군데서 토큰 공유.
const KIND_CLASSES = {
    [KIND_HANJA]:   { root: 'unim-hanja-popup',   header: 'hanja-target', grid: 'hanja-grid' },
    [KIND_SPECIAL]: { root: 'unim-special-popup', header: 'popup-header', grid: 'special-grid' },
    [KIND_EMOJI]:   { root: 'unim-emoji-popup',   header: 'popup-header', grid: 'emoji-grid' },
};

/**
 * @typedef {Object} PopupRpc
 * @property {(idx:number)=>void} selectHanja
 * @property {(idx:number)=>void} selectSpecial
 * @property {(emojiStr:string)=>void} commitEmoji
 * @property {(idx:number)=>void} toggleHanjaBookmark
 * @property {(direction:number)=>void} popupChangePage
 * @property {()=>void} togglePopupExpand
 * @property {(idx:number)=>void} setEmojiCategory
 */

export class PopupView {
    /**
     * @param {PopupRpc} rpc - popup-service RPC 호출 헬퍼 (extension.js 에서 주입)
     */
    constructor(rpc) {
        this._rpc = rpc;
        this._kind = -1;
        this._visible = false;
        this._cols = 1;     // PopupRender layout.cols 추적 (한자 compact/expanded 판별)
        this._outsideClickHandlerId = 0;   // stage captured-event id (외부 클릭 dismiss)

        // 루트 컨테이너 (가로 — 좌측 이모지 탭 + 본문 vbox).
        // kind 별 클래스(unim-{hanja,special,emoji}-popup) 는 update() 에서 toggle.
        this._container = new St.BoxLayout({
            vertical: false,
            reactive: true,
            visible: false,
        });

        // 본문 vbox — 헤더 + 그리드 + 푸터.
        // 이모지 카테고리 탭은 별도 BoxLayout 없이 통합 grid 의 col 0 에 직접 attach
        // (GTK popup-service 와 동일 패턴). 행 높이가 같은 GridLayout 안에 들어가야
        // 탭 9개가 우측 행 번호(1~9)와 vertical 정렬됨.
        this._body = new St.BoxLayout({
            style_class: 'unim-hanja-vbox',   // hanja vbox 토큰 재사용 (padding/margin 0)
            vertical: true,
            // container `min-width: 280px` 를 grid 까지 전파해야 한자 compact 의 col 1
            // (cell, x_expand=true) 이 자연 폭으로 늘어난다. 미설정 시 _body 가
            // grid 의 자연폭(~80px) 만큼만 차지 → compact 첫 렌더 좁음 버그 (관측 #2057).
            x_expand: true,
            x_align: Clutter.ActorAlign.FILL,
        });

        // 헤더 라벨 — 한자=hanja-target, 특수/이모지=popup-header (kind 별 toggle)
        this._header = new St.Label({ text: '' });

        // 그리드 — GridLayout 으로 셀·헤더 배치. style_class 는 kind 별 toggle.
        // _body 의 x_expand cascade 를 받아 grid 도 container 폭만큼 차지.
        // 한자 compact 의 col 1(cell, x_expand=true) 이 extra space 를 흡수해 GTK
        // ListBox 와 동일한 행 폭을 얻는다. cell x_expand=false 인 expanded/special/
        // emoji 는 col 들이 expand 안 되므로 시각 차이 없음.
        this._grid = new St.Widget({
            layout_manager: new Clutter.GridLayout(),
            x_expand: true,
            x_align: Clutter.ActorAlign.FILL,
        });

        // 푸터 wrapper — popup width 만큼 가로로 확장 (페이저가 좌·우 끝에 분산).
        this._footer = new St.BoxLayout({
            style_class: 'popup-footer-box',
            vertical: false,
            visible: false,
            x_expand: true,
        });
        this._prevBtn = new St.Button({
            style_class: 'popup-page-btn',
            label: '◀',
            x_align: Clutter.ActorAlign.START,
        });
        this._prevBtn.connect('clicked', () => this._rpc.popupChangePage(0));

        // 페이지 라벨 — hexpand + 중앙 정렬로 푸터 중앙 차지.
        this._pageLabel = new St.Label({
            style_class: 'page-label',
            text: '',
            x_expand: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });

        this._nextBtn = new St.Button({
            style_class: 'popup-page-btn',
            label: '▶',
            x_align: Clutter.ActorAlign.END,
        });
        this._nextBtn.connect('clicked', () => this._rpc.popupChangePage(1));

        // 한자 popup 확장/축소 토글 (⊞/⊟). 우측 끝, ▶ 다음에 위치.
        this._expandBtn = new St.Button({
            style_class: 'popup-expand-icon',
            label: '',
            visible: false,
            x_align: Clutter.ActorAlign.END,
        });
        this._expandBtn.connect('clicked', () => this._rpc.togglePopupExpand());

        this._footer.add_child(this._prevBtn);
        this._footer.add_child(this._pageLabel);
        this._footer.add_child(this._nextBtn);
        this._footer.add_child(this._expandBtn);

        this._body.add_child(this._header);
        this._body.add_child(this._grid);
        this._body.add_child(this._footer);

        this._container.add_child(this._body);

        Main.layoutManager.addChrome(this._container, {
            affectsStruts: false,
            trackFullscreen: false,
        });
    }

    /** Show*Popup signal 핸들러 — 위치 정중앙 고정, 셀은 후속 PopupRender 가 채운다. */
    show(_cursorRect) {
        this._visible = true;
        this._container.show();
        this._positionPopup();
        this._installOutsideClickHandler();
    }

    /** HidePopup signal 핸들러. kind 별 cancel RPC 도 함께 호출하여 engine 상태 정리.
     *  GTK popup-service `window.connect_hide(|| cancel_hanja_via_dbus())` 동등.
     *
     *  popup 영역 밖 클릭에 의한 dismiss 는 별도 stage-level handler 가 아니라
     *  IM 모듈의 vfunc_focus_out / vfunc_reset → extension._cleanupPopups 경로로
     *  처리된다 (해당 경로가 PopupView.hide + popupCancel* RPC 를 모두 호출). */
    hide() {
        if (!this._visible) return;
        this._visible = false;
        this._uninstallOutsideClickHandler();
        const kindAtHide = this._kind;
        this._container.hide();
        try {
            if (kindAtHide === KIND_HANJA) {
                this._rpc.cancelHanja();
            } else if (kindAtHide === KIND_SPECIAL) {
                this._rpc.cancelSpecial();
            }
            // emoji 는 daemon 이 CommitEmoji / 단축키 cancel 시 자체 정리.
        } catch (e) {
            unimError('POPUP_VIEW', `hide cancel RPC 실패: ${e.message}`);
        }
    }

    /**
     * PopupRender signal 핸들러 — 평면 payload 받아서 갱신.
     */
    update(payload) {
        const prevKind = this._kind;
        this._kind = payload.kind;
        this._cols = payload.layout.cols;

        const { texts, layout, flags, cells, colHeaders, rowHeaders,
                tabLabels, activeTabIndex } = payload;

        // ── kind 별 클래스 toggle ──
        const klass = KIND_CLASSES[this._kind] || KIND_CLASSES[KIND_HANJA];
        if (prevKind !== this._kind) {
            // 이전 루트 클래스 제거 후 새 클래스 적용.
            for (const k of Object.values(KIND_CLASSES)) {
                this._container.remove_style_class_name(k.root);
            }
            this._container.add_style_class_name(klass.root);
            this._header.set_style_class_name(klass.header);
            this._grid.set_style_class_name(klass.grid);
        }

        // ── 헤더 텍스트 ── (daemon header_text 우선, fallback target)
        // GTK popup-service 와 동일 — daemon 이 "「target」 → 한자" 같은 포맷팅을 미리
        // 만들어 header_text 로 보내준다. payload.texts.header 가 비어있을 때만 target 사용.
        this._header.set_text(texts.header || texts.target || '');

        // ── 그리드 ── (rows/cols/cells/headers 매번 갱신, emoji 시 col 0 탭도 함께)
        this._rebuildGrid(layout.rows, layout.cols, cells, colHeaders, rowHeaders,
                          tabLabels, activeTabIndex);

        // ── 푸터 ── 모든 kind 공통: **페이저는 항상 표시**, 페이지가 1개여도
        //   레이아웃 안정을 위해 출력하되 ◀/▶ 만 비활성화 (popup 크기 점프 방지).
        this._pageLabel.set_text(`${layout.page + 1} / ${layout.totalPages}`);
        this._footer.show();

        const singlePage = layout.totalPages <= 1;
        this._prevBtn.reactive = !singlePage;
        this._nextBtn.reactive = !singlePage;
        this._prevBtn.can_focus = !singlePage;
        this._nextBtn.can_focus = !singlePage;
        // dim — 활성 시 255, 비활성 시 약 40% 투명도.
        this._prevBtn.opacity = singlePage ? 100 : 255;
        this._nextBtn.opacity = singlePage ? 100 : 255;

        // 확장/축소 아이콘 (한자 popup 에서만 visible).
        if (flags.expandVisible) {
            this._expandBtn.set_label(texts.expand || '');
            this._expandBtn.show();
        } else {
            this._expandBtn.hide();
        }

        // ── 좌측 카테고리 탭 ── (이모지 전용) — _rebuildGrid 가 col 0 에 직접 attach.
        // 별도 BoxLayout 없이 통합 grid 안에 들어가야 우측 row(1~9) 와 vertical 정렬됨.

        // 셀 채워졌으므로 자연 크기 재산출 후 정중앙으로 재배치
        if (this._visible) {
            this._positionPopup();
        }
    }

    /** @private */
    _rebuildGrid(rows, cols, cells, colHeaders, rowHeaders, tabLabels, activeTabIndex) {
        this._grid.destroy_all_children();
        const layout = this._grid.layout_manager;

        const hasColHeaders = colHeaders.length > 0;
        const hasRowHeaders = rowHeaders.length > 0;
        // 이모지 popup 만 col 0 에 카테고리 탭을 attach (GTK popup-service emoji.rs
        // 와 동일 패턴). 같은 GridLayout 안에 있어야 탭과 row(1~9) 가 자연 정렬됨.
        const hasTabs = this._kind === KIND_EMOJI && tabLabels.length > 0;
        const tabsColOffset = hasTabs ? 1 : 0;
        const rowHeaderCol  = tabsColOffset;          // tabs 가 있으면 1, 없으면 0
        const colOffset     = tabsColOffset + (hasRowHeaders ? 1 : 0);
        const rowOffset     = hasColHeaders ? 1 : 0;

        // 한자 compact 모드 (cols==1) 는 GTK ListBox `.hanja-list` 와 시각 일치를 위해
        // row_header 클래스를 `hanja-num` 으로 (expanded 의 grid-row-number 와 토큰 다름).
        const isHanjaCompact = this._kind === KIND_HANJA && cols === 1;
        const rowHeaderClass = isHanjaCompact ? 'hanja-num' : 'grid-row-number';

        // 이모지 카테고리 탭 (col 0, row rowOffset..rowOffset+N-1).
        if (hasTabs) {
            for (let i = 0; i < tabLabels.length; i++) {
                const tabBtn = this._buildTabButton(tabLabels[i], i, i === activeTabIndex);
                layout.attach(tabBtn, 0, i + rowOffset, 1, 1);
            }
        }

        // 컬럼 헤더 (grid-header)
        if (hasColHeaders) {
            for (let c = 0; c < colHeaders.length; c++) {
                const [text, active] = colHeaders[c];
                const lbl = new St.Label({
                    style_class: active ? 'grid-header active' : 'grid-header',
                    text,
                });
                layout.attach(lbl, c + colOffset, 0, 1, 1);
            }
        }

        // 행 헤더 + 셀
        for (let r = 0; r < rows; r++) {
            if (hasRowHeaders && r < rowHeaders.length) {
                const [text, active] = rowHeaders[r];
                const lbl = new St.Label({
                    style_class: active ? `${rowHeaderClass} active` : rowHeaderClass,
                    text,
                    y_align: Clutter.ActorAlign.CENTER,
                });
                layout.attach(lbl, rowHeaderCol, r + rowOffset, 1, 1);
            }
            for (let c = 0; c < cols; c++) {
                // column-major: index = c * rows + r
                const idx = c * rows + r;
                if (idx >= cells.length) continue;
                const [text, meaning, flagBits] = cells[idx];
                if ((flagBits & FLAG_HAS_DATA) === 0) continue;

                const cell = this._buildCell(text, meaning, flagBits, r, c, isHanjaCompact);
                layout.attach(cell, c + colOffset, r + rowOffset, 1, 1);
            }
        }
    }

    /** @private */
    _buildCell(text, meaning, flagBits, row, col, isHanjaCompact) {
        const selected   = (flagBits & FLAG_SELECTED) !== 0;
        const bookmarked = (flagBits & FLAG_BOOKMARKED) !== 0;

        // GTK CSS 와 동일 선택자:
        //   한자: .grid-cell.grid-cell-selected.bookmarked
        //   특수/이모지: .grid-cell.selected
        const classes = ['grid-cell'];
        if (selected) {
            classes.push(this._kind === KIND_HANJA ? 'grid-cell-selected' : 'selected');
        }
        if (bookmarked) classes.push('bookmarked');

        const btn = new St.Button({
            style_class: classes.join(' '),
            can_focus: false,
            // GTK4 :hover pseudo 와 동등한 St hover 활성화. CSS `.grid-cell:hover`
            // 가 동작하려면 위젯이 reactive 이고 hover 변화를 추적해야 한다.
            reactive: true,
            track_hover: true,
            x_expand: isHanjaCompact,   // compact 시 행 너비 채워서 GTK ListBox row 와 동일
            x_align: isHanjaCompact ? Clutter.ActorAlign.FILL : Clutter.ActorAlign.CENTER,
        });

        if (this._kind === KIND_HANJA) {
            // 한자 셀: GTK 한자 popup-service hanja.rs:render_list/render_grid 의
            // ListBoxRow > HBox{hanja-num, hanja-char, hanja-meaning(expand+ellipsize),
            // hanja-bookmark} 와 동일 구조. expanded 모드는 num/meaning 없이 char 만.
            const inner = new St.BoxLayout({
                vertical: false,
                style_class: 'unim-hanja-vbox',
                x_expand: true,
            });

            const charLbl = new St.Label({ style_class: 'hanja-char', text });
            inner.add_child(charLbl);

            if (isHanjaCompact) {
                // 뜻 라벨 — 빈 문자열이어도 hexpand spacer 역할로 추가해서 ★ 가
                // 항상 우측 끝에 고정되게 한다 (뜻이 없으면 별이 char 옆에 붙는 버그 회피).
                const meaningLbl = new St.Label({
                    style_class: 'hanja-meaning',
                    text: meaning || '',
                    x_expand: true,
                    x_align: Clutter.ActorAlign.START,
                });
                meaningLbl.clutter_text.set_ellipsize(Pango.EllipsizeMode.END);
                inner.add_child(meaningLbl);

                inner.add_child(new St.Label({
                    style_class: bookmarked ? 'hanja-bookmark bookmarked' : 'hanja-bookmark',
                    text: bookmarked ? '★' : '☆',
                    x_align: Clutter.ActorAlign.END,
                }));
            }
            btn.set_child(inner);
        } else {
            // 특수/이모지 셀 = 글자 1개. St.Button label 직접 사용.
            btn.set_label(text);
        }

        // 마우스 클릭 → popup-service RPC
        btn.connect('clicked', () => {
            try {
                if (this._kind === KIND_HANJA) {
                    // column-major idx — daemon resolve_popup_owner 가 처리
                    const idx = col * 9 + row;
                    this._rpc.selectHanja(idx);
                } else if (this._kind === KIND_SPECIAL) {
                    const idx = col * 9 + row;
                    this._rpc.selectSpecial(idx);
                } else if (this._kind === KIND_EMOJI) {
                    this._rpc.commitEmoji(text);
                }
            } catch (e) {
                unimError('POPUP_VIEW', `cell click 실패: ${e.message}`);
            }
        });

        // 한자 우클릭 → ★/☆ 토글 + yellow bookmark-flash 140ms 즉시 시각 피드백
        // (GTK popup-service hanja.rs:bookmark_flash 동등). 다음 PopupRender 가
        // 동반 발행되면 cell 이 재생성되어 자연스럽게 flash 도 사라진다.
        if (this._kind === KIND_HANJA) {
            btn.connect('button-press-event', (target, event) => {
                if (event.get_button() === 3) {
                    const idx = col * 9 + row;
                    this._rpc.toggleHanjaBookmark(idx);
                    this._flashBookmark(target);
                    return Clutter.EVENT_STOP;
                }
                return Clutter.EVENT_PROPAGATE;
            });
        }
        return btn;
    }

    /**
     * 한자 cell 에 yellow flash 140ms 적용 후 자동 제거.
     * @private
     */
    _flashBookmark(widget) {
        if (!widget) return;
        widget.add_style_class_name('bookmark-flash');
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, BOOKMARK_FLASH_DURATION_MS, () => {
            // 위젯이 재생성됐을 수 있으므로 try-catch
            try {
                widget.remove_style_class_name('bookmark-flash');
            } catch (_) { /* destroyed — ignore */ }
            return GLib.SOURCE_REMOVE;
        });
    }

    /**
     * 이모지 카테고리 탭 button 1개 생성 — 통합 grid 의 col 0 에 attach 됨.
     * 탭 label = "최근 사용 (A)" 형식. 단축키 prefix `(A)`/`(S)`/… 가 세로축으로
     * 정렬되게 라벨 우측 정렬 (child Label 의 x_align: END).
     * @private
     */
    _buildTabButton(text, idx, checked) {
        const lblWidget = new St.Label({
            text,
            x_expand: true,
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.CENTER,
        });
        const btn = new St.Button({
            style_class: checked ? 'emoji-tab-button checked' : 'emoji-tab-button',
            child: lblWidget,
            can_focus: false,
            x_expand: true,
            x_align: Clutter.ActorAlign.FILL,
            reactive: true,
            track_hover: true,
        });
        btn.connect('clicked', () => {
            try {
                this._rpc.setEmojiCategory(idx);
            } catch (e) {
                unimError('POPUP_VIEW', `tab click 실패: ${e.message}`);
            }
        });
        return btn;
    }

    /**
     * 외부 클릭 dismiss — stage `captured-event` 를 가로채 buttonpress 좌표가
     * popup container 밖이면 hide() 호출. 이벤트는 PROPAGATE 하여 클릭이 아래
     * 창에 그대로 전달된다 (scrim 방식 대비 외부 창 1-click 활성화 가능).
     * 셀 클릭은 좌표가 container 안이라 통과 → 기존 onClick 동작.
     * @private
     */
    _installOutsideClickHandler() {
        if (this._outsideClickHandlerId) return;
        this._outsideClickHandlerId = global.stage.connect('captured-event',
            (_actor, event) => {
                if (event.type() !== Clutter.EventType.BUTTON_PRESS) {
                    return Clutter.EVENT_PROPAGATE;
                }
                const [x, y] = event.get_coords();
                const [cx, cy] = this._container.get_transformed_position();
                const [cw, ch] = this._container.get_transformed_size();
                if (x >= cx && x < cx + cw && y >= cy && y < cy + ch) {
                    return Clutter.EVENT_PROPAGATE;
                }
                this.hide();
                return Clutter.EVENT_PROPAGATE;
            });
    }

    /** @private */
    _uninstallOutsideClickHandler() {
        if (!this._outsideClickHandlerId) return;
        try {
            global.stage.disconnect(this._outsideClickHandlerId);
        } catch (_) { /* already disconnected */ }
        this._outsideClickHandlerId = 0;
    }

    /**
     * popup 위치 — **화면 정중앙 고정** (GTK4 popup-service 와 동일 정책).
     *
     * 사용자(기현) 정책: cursor 추적 시 fractional scaling·multi-monitor·CSD
     * 좌표 변환 누적 오차로 popup 이 어긋난 위치에 떠 인지 부담 ↑.
     * @private
     */
    _positionPopup() {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natW] = this._container.get_preferred_width(-1);
        const [, natH] = this._container.get_preferred_height(-1);
        const popupW = natW > 0 ? natW : 320;
        const popupH = natH > 0 ? natH : 320;

        const x = Math.floor(monitor.x + (monitor.width  - popupW) / 2);
        const y = Math.floor(monitor.y + (monitor.height - popupH) / 2);
        this._container.set_position(x, y);
    }

    destroy() {
        this._uninstallOutsideClickHandler();
        if (this._container) {
            try {
                Main.layoutManager.removeChrome(this._container);
            } catch (e) { /* already removed */ }
            this._container.destroy();
            this._container = null;
        }
        this._body = null;
        this._header = null;
        this._grid = null;
        this._footer = null;
        this._prevBtn = null;
        this._nextBtn = null;
        this._pageLabel = null;
        this._expandBtn = null;
        this._rpc = null;
    }
}

/**
 * PopupRender DBus tuple → 객체 변환 헬퍼.
 * @param {GLib.Variant} parameters
 * @returns {object|null}
 */
export function unpackPopupRender(parameters) {
    try {
        const [kind, texts, layout, flags, cells, colHeaders, rowHeaders,
               tabLabels, activeTabIndex] = parameters.deep_unpack();
        const [target, header, footer, expand] = texts;
        const [rows, cols, selRow, selCol, page, totalPages] = layout;
        const [showFooter, expandVisible] = flags;
        return {
            kind,
            texts: { target, header, footer, expand },
            layout: { rows, cols, selRow, selCol, page, totalPages },
            flags: { showFooter, expandVisible },
            cells,
            colHeaders,
            rowHeaders,
            tabLabels,
            activeTabIndex,
        };
    } catch (e) {
        unimError('POPUP_VIEW', `PopupRender unpack 실패: ${e.message}`);
        return null;
    }
}
