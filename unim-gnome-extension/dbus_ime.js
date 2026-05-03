/**
 * UNIM DBus IME Client for GNOME Shell Extension
 *
 * unim-daemon의 InputMethod / InputContext DBus 서비스와 통신하여
 * 입력 컨텍스트 생성/파괴, 키 이벤트 처리, 한자/특수문자 후보 조회를 담당합니다.
 *
 * @module dbus_ime
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import { unimLog, unimError } from './logging.js';

/** DBus 서비스 상수 */
const UNIM_BUS_NAME = 'org.atit.unim.InputMethod';
const UNIM_OBJECT_PATH = '/org/atit/unim/InputMethod';
const UNIM_IM_INTERFACE = 'org.atit.unim.InputMethod';
const UNIM_IC_INTERFACE = 'org.atit.unim.InputContext';

/** DBus 호출 타임아웃 (밀리초). 입력 지연을 최소화하기 위해 짧게 설정. */
const DBUS_TIMEOUT_MS = 500;

/**
 * UNIM DBus IME 클라이언트
 *
 * InputMethod 팩토리로 InputContext를 생성하고,
 * 생성된 컨텍스트를 통해 키 이벤트, 포커스, 한자/특수문자를 처리합니다.
 */
export class UnimDbusIME {
    constructor() {
        /** @type {Gio.DBusProxy|null} InputMethod 팩토리 프록시 */
        this._imProxy = null;
        /** @type {Gio.DBusProxy|null} InputContext 프록시 */
        this._icProxy = null;
        /** @type {string|null} 생성된 InputContext의 DBus 객체 경로 */
        this._contextPath = null;
        /** @type {number} g-signal 핸들러 ID (GlobalModeChanged) */
        this._imSignalId = 0;
        /** @type {number} g-signal 핸들러 ID (InputContext 시그널) */
        this._icSignalId = 0;
        /** @type {number} 글로벌 팝업 시그널 구독 ID */
        this._popupSignalId = 0;
        /** @type {boolean} 데몬 연결 상태 */
        this._connected = false;
        /** @type {Function|null} 모드 변경 콜백 */
        this._onModeChanged = null;
        /** @type {Function|null} 한자 팝업 표시 콜백 */
        this._onShowHanja = null;
        /** @type {Function|null} 특수문자 팝업 표시 콜백 */
        this._onShowSpecial = null;
        /** @type {Function|null} 이모지 팝업 표시 콜백 (Super+. 트리거) */
        this._onShowEmoji = null;
        /** @type {Function|null} 팝업 숨김 콜백 */
        this._onHidePopup = null;
        /** @type {Function|null} 팝업 네비게이션 콜백 */
        this._onPopupNavigate = null;
        /** @type {Function|null} 한자 즐겨찾기 변경 콜백 */
        this._onHanjaBookmarkChanged = null;
        /** @type {Function|null} 한자 후보 재정렬 콜백 (즐겨찾기 토글 후) */
        this._onHanjaCandidatesReordered = null;
        /** @type {Function|null} AutoTypeFix 교정 콜백 */
        this._onAutoTypeFix = null;
        /** @type {Function|null} Config 갱신 콜백 (parsed JSON object) */
        this._onConfigChanged = null;
        /** @type {object|null} 캐시된 Config (GetConfigJson / ConfigChangedJson payload) */
        this._configCache = null;
    }

    /**
     * 현재 캐시된 Config 객체 반환 (없으면 null)
     *
     * 단일 진실 공급원(config.yaml)의 스냅샷. 시작 시 GetConfigJson으로
     * 로드되고 ConfigChangedJson signal로 갱신된다.
     * 매 키스트로크 DBus 호출 방지 목적.
     *
     * @returns {object|null}
     */
    getCachedConfig() {
        return this._configCache;
    }

    /**
     * Config 변경 콜백 등록
     * @param {Function} cb - (cfg: object) => void
     */
    setOnConfigChanged(cb) {
        this._onConfigChanged = cb || null;
    }

    /**
     * DBus 연결 초기화
     *
     * InputMethod 프록시 생성 후 InputContext를 생성합니다.
     *
     * @param {string} windowId - 초기 창 식별자
     * @param {Function} [onModeChanged] - 모드 변경 콜백 (isKorean: boolean)
     * @returns {boolean} 연결 성공 여부
     */
    connect(windowId, onModeChanged) {
        this._onModeChanged = onModeChanged || null;

        try {
            // 1. InputMethod 팩토리 프록시 생성
            this._imProxy = Gio.DBusProxy.new_for_bus_sync(
                Gio.BusType.SESSION,
                Gio.DBusProxyFlags.NONE,
                null,
                UNIM_BUS_NAME,
                UNIM_OBJECT_PATH,
                UNIM_IM_INTERFACE,
                null
            );

            if (!this._imProxy) {
                unimError('DBUS_IME', 'InputMethod 프록시 생성 실패');
                return false;
            }

            // GlobalModeChanged / ConfigChangedJson 시그널 수신
            this._imSignalId = this._imProxy.connect('g-signal',
                (_proxy, _senderName, signalName, parameters) => {
                    if (signalName === 'GlobalModeChanged' && this._onModeChanged) {
                        const [isKorean] = parameters.deep_unpack();
                        this._onModeChanged(isKorean);
                    } else if (signalName === 'ConfigChangedJson') {
                        const [jsonStr] = parameters.deep_unpack();
                        try {
                            const cfg = JSON.parse(jsonStr);
                            this._configCache = cfg;
                            if (this._onConfigChanged) {
                                this._onConfigChanged(cfg);
                            }
                        } catch (e) {
                            unimError('DBUS_IME',
                                `ConfigChangedJson 파싱 실패: ${e.message}`);
                        }
                    }
                }
            );

            // 초기 Config 로드 (GetConfigJson)
            this._loadInitialConfig();

            // 2. InputContext 생성
            this._createContext(windowId);

            this._connected = true;
            unimLog('DBUS_IME', `연결 완료 (context: ${this._contextPath})`);
            return true;
        } catch (e) {
            unimError('DBUS_IME', `연결 실패: ${e.message}`);
            this._connected = false;
            return false;
        }
    }

    /**
     * 팝업 콜백 등록
     * @param {object} callbacks
     * @param {Function} [callbacks.onShowHanja] - (target, candidates, topRow, cursorRect)
     * @param {Function} [callbacks.onShowSpecial] - (target, characters, topRow, cursorRect)
     * @param {Function} [callbacks.onShowEmoji]
     *   - (targetCatId, items, topRow, recent, categoriesRaw, cursorRect).
     *     PR #3 부터 V2 시그널만 사용 — payload 에 카테고리·MRU 메타가 포함되어
     *     5단계 sync RPC (`listEmojiCategories` / `getEmojiFavorites` / `searchEmoji`)
     *     의존을 모두 제거.
     * @param {Function} [callbacks.onHidePopup] - ()
     * @param {Function} [callbacks.onPopupNavigate] - (page, totalPages, selected, rows, cols, selRow, selCol)
     */
    setPopupCallbacks(callbacks) {
        this._onShowHanja = callbacks.onShowHanja || null;
        this._onShowSpecial = callbacks.onShowSpecial || null;
        this._onShowEmoji = callbacks.onShowEmoji || null;
        this._onHidePopup = callbacks.onHidePopup || null;
        this._onPopupNavigate = callbacks.onPopupNavigate || null;
        this._onAutoTypeFix = callbacks.onAutoTypeFix || null;
        this._onHanjaBookmarkChanged = callbacks.onHanjaBookmarkChanged || null;
        this._onHanjaCandidatesReordered = callbacks.onHanjaCandidatesReordered || null;
    }

    /**
     * 초기 Config 로드 — GetConfigJson 1회 호출하여 캐시 초기화
     *
     * 실패해도 확장 기동은 계속한다 (데몬 부팅 타이밍 이슈 등).
     * 실패 시 ConfigChangedJson signal을 통해 지연 갱신될 수 있다.
     *
     * @private
     */
    _loadInitialConfig() {
        try {
            const result = this._imProxy.call_sync(
                'GetConfigJson',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (!result) return;
            const [jsonStr] = result.deep_unpack();
            this._configCache = JSON.parse(jsonStr);
            if (this._onConfigChanged) {
                this._onConfigChanged(this._configCache);
            }
            unimLog('DBUS_IME',
                `GetConfigJson 초기 로드 완료 (${jsonStr.length} bytes)`);
        } catch (e) {
            unimError('DBUS_IME',
                `GetConfigJson 실패 (무시하고 계속): ${e.message}`);
        }
    }

    /**
     * InputContext 생성
     * @param {string} windowId - 창 식별자
     * @private
     */
    _createContext(windowId) {
        const result = this._imProxy.call_sync(
            'CreateInputContext',
            new GLib.Variant('(ss)', ['gnome-extension', windowId || '']),
            Gio.DBusCallFlags.NONE,
            DBUS_TIMEOUT_MS,
            null
        );

        if (!result) {
            throw new Error('CreateInputContext 응답 없음');
        }

        const [path] = result.deep_unpack();
        this._contextPath = path;

        // InputContext 프록시 생성
        this._icProxy = Gio.DBusProxy.new_for_bus_sync(
            Gio.BusType.SESSION,
            Gio.DBusProxyFlags.NONE,
            null,
            UNIM_BUS_NAME,
            this._contextPath,
            UNIM_IC_INTERFACE,
            null
        );

        if (!this._icProxy) {
            throw new Error(`InputContext 프록시 생성 실패: ${this._contextPath}`);
        }

        // InputContext 시그널 구독 (자기 context: 팝업 등)
        // AutoTypefixApply는 글로벌 구독에서만 처리 (중복 방지)
        this._icSignalId = this._icProxy.connect('g-signal',
            (_proxy, _senderName, signalName, parameters) => {
                if (signalName === 'AutoTypefixApply') return;
                this._handleContextSignal(signalName, parameters, true);
            }
        );

        // 모든 InputContext의 팝업 시그널을 글로벌 구독
        // Wayland에서만 활성: 다른 프론트엔드(XIM/GTK/Qt)의 팝업을 GNOME extension이 표시
        // X11에서는 gui-gtk가 팝업을 처리하므로 글로벌 구독 불필요 (중복 팝업 방지)
        const bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
        this._popupSignalId = bus.signal_subscribe(
            UNIM_BUS_NAME,
            UNIM_IC_INTERFACE,
            null,   // 모든 시그널
            null,   // 모든 object path
            null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, path, _iface, signalName, parameters) => {
                const isOwn = (path === this._contextPath);
                // AutoTypefixApply는 자기 context도 글로벌 구독에서 처리
                // (g-signal proxy가 introspection 미등록 시그널을 전달하지 않을 수 있음)
                if (signalName === 'AutoTypefixApply' && isOwn) {
                    this._handleContextSignal(signalName, parameters, true);
                    return;
                }
                // 자기 context의 다른 시그널은 _icProxy g-signal에서 이미 처리
                if (isOwn) return;
                // X11에서는 gui-gtk가 팝업을 담당하므로 스킵 (중복 팝업 방지)
                if (!Meta.is_wayland_compositor()) return;
                this._handleContextSignal(signalName, parameters, false);
            }
        );
    }

    /**
     * InputContext 시그널 처리
     * @param {string} signalName
     * @param {GLib.Variant} parameters
     * @private
     */
    _handleContextSignal(signalName, parameters, isOwnContext = false) {
        try {
            if (signalName === 'ShowHanjaPopup' && this._onShowHanja) {
                // 7-tuple: 활성 영문 키맵의 top_row를 5번째 인자로 받아 9x9 컬럼 헤더 동기화
                const [target, candidatesRaw, topRow, cx, cy, cw, ch] = parameters.deep_unpack();
                const candidates = candidatesRaw.map(([hanja, meaning]) => ({
                    hanja, meaning,
                }));
                const cursorRect = isOwnContext
                    ? { x: cx, y: cy, width: cw, height: ch }
                    : this._adjustCursorRect(cx, cy, cw, ch);
                this._onShowHanja(target, candidates, topRow, cursorRect);
            } else if (signalName === 'ShowSpecialPopup' && this._onShowSpecial) {
                const [target, characters, topRow, cx, cy, cw, ch] = parameters.deep_unpack();
                const cursorRect = isOwnContext
                    ? { x: cx, y: cy, width: cw, height: ch }
                    : this._adjustCursorRect(cx, cy, cw, ch);
                this._onShowSpecial(target, characters, topRow, cursorRect);
            } else if (signalName === 'ShowEmojiPopupV2' && this._onShowEmoji) {
                // payload: (target_cat_id, items, top_row, recent, categories, x, y, w, h)
                // categories 는 (id, ko, en, count) 튜플 9개.
                const [
                    targetCatId,
                    items,
                    topRow,
                    recent,
                    categoriesRaw,
                    cx, cy, cw, ch,
                ] = parameters.deep_unpack();
                const cursorRect = isOwnContext
                    ? { x: cx, y: cy, width: cw, height: ch }
                    : this._adjustCursorRect(cx, cy, cw, ch);
                this._onShowEmoji(
                    targetCatId,
                    items,
                    topRow,
                    recent,
                    categoriesRaw,
                    cursorRect
                );
            } else if (signalName === 'HidePopup' && this._onHidePopup) {
                this._onHidePopup();
            } else if (signalName === 'PopupNavigate' && this._onPopupNavigate) {
                const [page, totalPages, selected, rows, cols, selRow, selCol] = parameters.deep_unpack();
                this._onPopupNavigate(page, totalPages, selected, rows, cols, selRow, selCol);
            } else if (signalName === 'HanjaBookmarkChanged' && this._onHanjaBookmarkChanged) {
                const [index, bookmarked] = parameters.deep_unpack();
                this._onHanjaBookmarkChanged(index, bookmarked);
            } else if (signalName === 'HanjaCandidatesReordered' && this._onHanjaCandidatesReordered) {
                // Phase 1 (mouse-paginate UX): wasBookmarked (직전 상태) 추가됨.
                // 콜백이 9-arity 이면 무시되고, 10-arity 면 활용된다 (Phase 7 visual flash).
                const [target, hanjas, meanings, bookmarks, newCursor, page, selRow, selCol, bookmarked, wasBookmarked] = parameters.deep_unpack();
                this._onHanjaCandidatesReordered(target, hanjas, meanings, bookmarks, newCursor, page, selRow, selCol, bookmarked, wasBookmarked);
            } else if (signalName === 'AutoTypefixApply' && isOwnContext && this._onAutoTypeFix) {
                const [deleteChars, commitText, preeditText] = parameters.deep_unpack();
                unimLog('DBUS_IME', `AutoTypefixApply: delete=${deleteChars}, commit='${commitText}', preedit='${preeditText}'`);
                this._onAutoTypeFix(deleteChars, commitText, preeditText);
            }
        } catch (e) {
            unimError('DBUS_IME', `시그널 처리 오류 (${signalName}): ${e.message}`);
        }
    }

    /**
     * 연결 상태 확인
     * @returns {boolean}
     */
    get isConnected() {
        return this._connected && this._icProxy !== null;
    }

    /**
     * InputContext 프록시 반환 (직접 DBus 호출용)
     * @returns {Gio.DBusProxy|null}
     */
    getContextProxy() {
        return this._icProxy;
    }

    /**
     * InputMethod(글로벌) 프록시 반환 (글로벌 TypeFix 등)
     * @returns {Gio.DBusProxy|null}
     */
    getImProxy() {
        return this._imProxy;
    }

    // ===========================================
    // 키 이벤트 처리
    // ===========================================

    /**
     * 키 이벤트를 엔진으로 전달
     *
     * @param {number} keyval - 키 심볼 (GDK keyval)
     * @param {number} keycode - evdev 키코드
     * @param {number} state - 수정자 비트필드
     * @returns {{consumed: boolean, preedit: string, commit: string}|null} 처리 결과
     */
    processKey(keyval, keycode, state) {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'ProcessKeyEvent',
                new GLib.Variant('(uuu)', [keyval, keycode, state]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [consumed, preedit, commit] = result.deep_unpack();
            return { consumed, preedit, commit };
        } catch (e) {
            unimError('DBUS_IME', `ProcessKey 실패: ${e.message}`);
            return null;
        }
    }

    // ===========================================
    // 포커스 관리
    // ===========================================

    /**
     * 포커스 획득 알림
     * @param {string} windowId - 창 식별자
     */
    focusIn(windowId) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'FocusIn',
                new GLib.Variant('(s)', [windowId || '']),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `FocusIn 실패: ${e.message}`);
        }
    }

    /**
     * 포커스 상실 알림
     * @returns {string} 커밋된 텍스트 (조합 중이던 문자열)
     */
    focusOut() {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'FocusOut',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [commit] = result.deep_unpack();
            return commit || '';
        } catch (e) {
            unimError('DBUS_IME', `FocusOut 실패: ${e.message}`);
            return '';
        }
    }

    /**
     * 입력 상태 초기화
     */
    reset() {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'Reset',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `Reset 실패: ${e.message}`);
        }
    }

    /**
     * 커서 위치 보고
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     */
    reportCursorRect(x, y, width, height) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'ReportCursorRect',
                new GLib.Variant('(iiii)', [x, y, width, height]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `ReportCursorRect 실패: ${e.message}`);
        }
    }

    /**
     * Surrounding text 전달 (TypeFIX 더블탭용)
     * @param {string} text - 주변 텍스트
     * @param {number} cursor - 커서 위치 (문자 단위)
     * @param {number} anchor - 앵커 위치 (문자 단위)
     */
    setSurroundingText(text, cursor, anchor) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'SetSurroundingText',
                new GLib.Variant('(suu)', [text, cursor, anchor]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            // surrounding text 실패는 조용히 무시 (성능 영향 최소화)
        }
    }

    /**
     * 외부 프론트엔드(GTK/Qt/XIM)의 커서 좌표를 compositor 절대좌표로 변환
     *
     * - GNOME extension 자체 context: compositor 좌표이므로 변환 불필요
     * - Native Wayland 앱 (GTK3/GTK4): 윈도우 상대좌표 → focused window 위치 더함
     * - XWayland 앱 (XIM/Qt): X11 절대좌표 → compositor 좌표와 동일 (scale=1)
     *
     * @param {number} cx - 커서 X
     * @param {number} cy - 커서 Y
     * @param {number} cw - 커서 너비
     * @param {number} ch - 커서 높이
     * @returns {{x: number, y: number, width: number, height: number}}
     * @private
     */
    _adjustCursorRect(cx, cy, cw, ch) {
        const focusWindow = global.display?.focus_window;
        if (!focusWindow) {
            return { x: cx, y: cy, width: cw, height: ch };
        }

        const isX11 = focusWindow.get_client_type() === Meta.WindowClientType.X11;

        if (isX11) {
            // XWayland 앱: X11 절대좌표 → compositor 좌표와 동일 (scale=1 기준)
            return { x: cx, y: cy, width: cw, height: ch };
        }

        // Native Wayland 앱 (GTK3/GTK4 등): 윈도우 상대좌표
        // focused window의 buffer_rect (surface 원점 포함)을 더해 절대좌표로 변환
        const bufferRect = focusWindow.get_buffer_rect();
        const adjusted = {
            x: bufferRect.x + cx,
            y: bufferRect.y + cy,
            width: cw,
            height: ch,
        };

        unimLog('DBUS_IME',
            `좌표 변환 (Wayland): raw=(${cx},${cy}) + window=(${bufferRect.x},${bufferRect.y}) → (${adjusted.x},${adjusted.y})`);

        return adjusted;
    }

    // ===========================================
    // 한자 변환
    // ===========================================

    /**
     * 한자 후보 목록 조회
     *
     * @returns {{target: string, candidates: Array<{hanja: string, meaning: string}>}|null}
     */
    getHanjaCandidates() {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'GetHanjaCandidates',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [target, candidatesRaw] = result.deep_unpack();
            const candidates = candidatesRaw.map(([hanja, meaning]) => ({
                hanja, meaning,
            }));

            return { target, candidates };
        } catch (e) {
            unimError('DBUS_IME', `GetHanjaCandidates 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * 한자 선택
     * @param {number} index - 후보 인덱스
     * @returns {string} 선택된 한자 (실패 시 빈 문자열)
     */
    selectHanja(index) {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'SelectHanja',
                new GLib.Variant('(u)', [index]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [hanja] = result.deep_unpack();
            return hanja || '';
        } catch (e) {
            unimError('DBUS_IME', `SelectHanja 실패: ${e.message}`);
            return '';
        }
    }

    /**
     * 현재 한자 후보들의 즐겨찾기 상태 조회
     *
     * @returns {boolean[]} candidates와 동일한 순서의 즐겨찾기 플래그 (실패 시 빈 배열)
     */
    getHanjaBookmarkStates() {
        if (!this._icProxy) return [];

        try {
            const result = this._icProxy.call_sync(
                'GetHanjaBookmarkStates',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (!result) return [];
            const [flags] = result.deep_unpack();
            return Array.isArray(flags) ? flags : [];
        } catch (e) {
            unimError('DBUS_IME', `GetHanjaBookmarkStates 실패: ${e.message}`);
            return [];
        }
    }

    /**
     * 한자 즐겨찾기 토글
     *
     * @param {number} index - 후보 인덱스
     * @returns {{index: number, bookmarked: boolean}|null}
     */
    toggleHanjaBookmark(index) {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'ToggleHanjaBookmark',
                new GLib.Variant('(u)', [index]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (!result) return null;
            const [idx, bookmarked] = result.deep_unpack();
            return { index: idx, bookmarked };
        } catch (e) {
            unimError('DBUS_IME', `ToggleHanjaBookmark 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * 한자 모드 취소
     */
    cancelHanja() {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'CancelHanja',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (result) {
                const [text] = result.deep_unpack();
                return text || '';
            }
        } catch (e) {
            unimError('DBUS_IME', `CancelHanja 실패: ${e.message}`);
        }
        return '';
    }

    // ===========================================
    // 특수문자 변환
    // ===========================================

    /**
     * 특수문자 후보 목록 조회
     *
     * @returns {{target: string, characters: string[], topRow: string}|null}
     */
    getSpecialCharCandidates() {
        if (!this._icProxy) return null;

        try {
            const result = this._icProxy.call_sync(
                'GetSpecialCharCandidates',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return null;

            const [target, characters, topRow] = result.deep_unpack();
            return { target, characters, topRow };
        } catch (e) {
            unimError('DBUS_IME', `GetSpecialCharCandidates 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * 특수문자 선택
     * @param {number} index - 후보 인덱스
     * @returns {string} 선택된 특수문자 (실패 시 빈 문자열)
     */
    selectSpecialChar(index) {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'SelectSpecialChar',
                new GLib.Variant('(u)', [index]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [ch] = result.deep_unpack();
            return ch || '';
        } catch (e) {
            unimError('DBUS_IME', `SelectSpecialChar 실패: ${e.message}`);
            return '';
        }
    }

    // ===========================================
    // 이모지 팝업 (Super+. 트리거)
    // ===========================================

    /**
     * 선택한 이모지를 **마지막으로 포커스를 받은 실제 입력 컨텍스트**에 커밋.
     *
     * GTK4_IM_MODULE=unim 환경에서는 extension 자체의 `_icProxy`로 commit하면
     * GTK4_IM 모듈을 우회해 사용자 앱에 도달하지 못한다. 따라서 InputMethod-level
     * `CommitEmoji(s)` RPC(`_imProxy`)를 호출해, 데몬이 캐시한 last-active
     * InputContext path로 `CommitText` + `HidePopup` 시그널을 redirect하게 한다.
     * 즐겨찾기 MRU 갱신도 데몬 측에서 함께 수행.
     *
     * @param {string} emoji
     */
    commitEmoji(emoji) {
        if (!this._imProxy || !emoji) return;
        try {
            this._imProxy.call_sync(
                'CommitEmoji',
                new GLib.Variant('(s)', [emoji]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `CommitEmoji 실패: ${e.message}`);
        }
    }

    /**
     * 이모지 팝업 카테고리 변경 — 마우스 클릭으로 좌측 탭을 직접 전환할 때 호출.
     *
     * `_imProxy.SetEmojiCategory(idx)` RPC 를 호출하면 데몬이 마지막 포커스
     * 컨텍스트의 popup_state 를 갱신하고 `ShowEmojiPopupV2` 시그널을 재발행한다.
     * 본 extension 의 `_onShowEmoji` 핸들러가 그 시그널을 받아 동일 인스턴스를
     * 재구성하므로, 키보드 Tab/ShiftTab 과 동일한 화면 효과가 된다.
     *
     * @param {number} idx - 0..=8 카테고리 인덱스 (0=Recent, 1..=8=Smileys..Flags)
     */
    setEmojiCategory(idx) {
        if (!this._imProxy) return;
        if (typeof idx !== 'number' || idx < 0 || !Number.isInteger(idx)) {
            unimError('DBUS_IME', `setEmojiCategory: 잘못된 idx=${idx}`);
            return;
        }
        try {
            this._imProxy.call_sync(
                'SetEmojiCategory',
                new GLib.Variant('(u)', [idx]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            unimLog('DBUS_IME', `SetEmojiCategory(${idx}) 호출 완료`);
        } catch (e) {
            unimError('DBUS_IME', `SetEmojiCategory(${idx}) 실패: ${e.message}`);
        }
    }

    /**
     * 특수문자 모드 취소
     */
    cancelSpecialChar() {
        if (!this._icProxy) return '';

        try {
            const result = this._icProxy.call_sync(
                'CancelSpecialChar',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (result) {
                const [text] = result.deep_unpack();
                return text || '';
            }
        } catch (e) {
            unimError('DBUS_IME', `CancelSpecialChar 실패: ${e.message}`);
        }
        return '';
    }

    // ===========================================
    // 설정 조회
    // ===========================================

    // ===========================================
    // 글로벌 액션 트리거 (compositor-only 단축키 우회)
    // ===========================================

    /**
     * 글로벌 액션 트리거 (InputMethod-level TriggerAction RPC)
     *
     * GNOME Shell extension이 `global.display.grab_accelerator()`로
     * 캡처한 단축키를 데몬에 전달하기 위한 비차단 best-effort 호출.
     * 응답 reply type이 없는 RPC이므로 reply 파싱 생략, 성공/실패 무시.
     *
     * **`_imProxy`(InputMethod) 사용**: extension 자체의 `_icProxy`는 사용자 앱이
     * 아닌 extension 자신의 컨텍스트라, GTK4_IM 모듈이 기다리는 InputContext path와
     * 일치하지 않는다. InputMethod-level RPC는 데몬이 캐시한 last-active 실제 입력
     * 컨텍스트 path로 `ShowEmojiPopupV2` 시그널을 redirect하므로 단축키 캡처 주체와
     * 입력 주체가 분리된 환경(extension+GTK4_IM)에서도 정확한 컨텍스트로 popup이 발생한다.
     *
     * 사용 예: `triggerAction('emoji_popup')` →
     * 데몬이 popup_mode/last-active path 게이트를 검사 후 ShowEmojiPopupV2 signal 발행.
     *
     * @param {string} name - 액션 이름 (예: 'emoji_popup')
     */
    triggerAction(name) {
        if (!this._imProxy) {
            unimError('DBUS_IME', `triggerAction(${name}): InputMethod 미연결 — skip`);
            return;
        }
        if (typeof name !== 'string' || name.length === 0) {
            unimError('DBUS_IME', 'triggerAction: name 비어있음');
            return;
        }

        try {
            this._imProxy.call_sync(
                'TriggerAction',
                new GLib.Variant('(s)', [name]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            unimLog('DBUS_IME', `TriggerAction(${name}) 호출 완료`);
        } catch (e) {
            unimError('DBUS_IME', `TriggerAction(${name}) 실패: ${e.message}`);
        }
    }

    /**
     * 설정 값 조회
     * @param {string} key - 설정 키
     * @returns {string} 설정 값 (실패 시 빈 문자열)
     */
    getConfig(key) {
        if (!this._imProxy) return '';

        try {
            const result = this._imProxy.call_sync(
                'GetConfig',
                new GLib.Variant('(s)', [key]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );

            if (!result) return '';
            const [value] = result.deep_unpack();
            return value || '';
        } catch (e) {
            unimError('DBUS_IME', `GetConfig(${key}) 실패: ${e.message}`);
            return '';
        }
    }

    // ===========================================
    // 정리
    // ===========================================

    /**
     * 모든 자원 정리
     *
     * InputContext 파괴 → 프록시 해제 → 시그널 해제
     */
    destroy() {
        // 글로벌 팝업 시그널 구독 해제
        if (this._popupSignalId > 0) {
            try {
                const bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
                bus.signal_unsubscribe(this._popupSignalId);
            } catch (_e) {
                // 버스 접근 실패 무시
            }
            this._popupSignalId = 0;
        }

        // InputContext 시그널 해제 + 파괴
        if (this._icProxy) {
            if (this._icSignalId > 0) {
                this._icProxy.disconnect(this._icSignalId);
                this._icSignalId = 0;
            }
            try {
                this._icProxy.call_sync(
                    'Destroy',
                    null,
                    Gio.DBusCallFlags.NONE,
                    DBUS_TIMEOUT_MS,
                    null
                );
            } catch (_e) {
                // 데몬이 이미 종료된 경우 무시
            }
            this._icProxy = null;
        }
        this._contextPath = null;

        // InputMethod 프록시 정리
        if (this._imProxy) {
            if (this._imSignalId > 0) {
                this._imProxy.disconnect(this._imSignalId);
                this._imSignalId = 0;
            }
            this._imProxy = null;
        }

        this._connected = false;
        this._onModeChanged = null;
        this._onShowHanja = null;
        this._onShowSpecial = null;
        this._onShowEmoji = null;
        this._onHidePopup = null;
        this._onPopupNavigate = null;
        this._onHanjaBookmarkChanged = null;
        this._onHanjaCandidatesReordered = null;

        unimLog('DBUS_IME', '자원 정리 완료');
    }
}
