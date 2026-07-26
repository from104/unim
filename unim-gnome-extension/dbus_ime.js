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
import { unimLog, unimError } from './logging.js';

/** DBus 서비스 상수 */
const UNIM_BUS_NAME = 'org.atit.unim.InputMethod';
const UNIM_OBJECT_PATH = '/org/atit/unim/InputMethod';
const UNIM_IM_INTERFACE = 'org.atit.unim.InputMethod';
const UNIM_IC_INTERFACE = 'org.atit.unim.InputContext';

// popup-service Popup interface — ShowEmojiPopupV2 / HidePopup signal 구독 대상.
// daemon InputContext 의 popup signal 표면은 popup-service 가 mediator 로 격리.
const POPUP_SERVICE_BUS = 'org.atit.unim.PopupService';
const POPUP_OBJECT_PATH = '/org/atit/unim/popup';
const POPUP_INTERFACE = 'org.atit.unim.Popup';

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
        /** @type {Function|null} popup_render 시그널 콜백 (통합 view_model state) */
        this._onPopupRender = null;
        /** @type {Function|null} 한자 즐겨찾기 변경 콜백 */
        this._onHanjaBookmarkChanged = null;
        /** @type {Function|null} 한자 후보 재정렬 콜백 (즐겨찾기 토글 후) */
        this._onHanjaCandidatesReordered = null;
        /** @type {Function|null} AutoTypeFix 교정 콜백 */
        this._onAutoTypeFix = null;
        /** @type {Function|null} CommitText 시그널 콜백 (chord idle flush 등 비동기 commit 경로) */
        this._onCommitText = null;
        /** @type {Function|null} UpdatePreeditText 시그널 콜백 (chord idle flush preedit 갱신 등) */
        this._onUpdatePreedit = null;
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
        this._onPopupRender = callbacks.onPopupRender || null;
        this._onCommitText = callbacks.onCommitText || null;
        this._onUpdatePreedit = callbacks.onUpdatePreedit || null;
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

        // AutoTypefixApply 글로벌 구독 — 자기 context 의 g-signal proxy 가 introspection
        // 미등록 signal 을 누락할 수 있어 별도 구독.
        const bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
        this._popupSignalId = bus.signal_subscribe(
            UNIM_BUS_NAME,
            UNIM_IC_INTERFACE,
            'AutoTypefixApply',
            null,
            null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, path, _iface, signalName, parameters) => {
                const isOwn = (path === this._contextPath);
                if (isOwn) {
                    this._handleContextSignal(signalName, parameters, true);
                }
            }
        );

        // popup-service Popup interface signal 구독 — ShowEmojiPopupV2 / HidePopup 만
        // 활용 (Wayland text-input v3 IM 세션 engage 용 ZWSP preedit 트릭). 한자/특수
        // popup signal 은 popup-service 가 자체 GTK4 popup 으로 표시하므로 GNOME ext
        // 는 처리 불요.
        this._popupServiceSignalId = bus.signal_subscribe(
            POPUP_SERVICE_BUS,
            POPUP_INTERFACE,
            null,                  // ShowEmojiPopupV2 / HidePopup 모두 받음 (signal 명 분기는 핸들러)
            POPUP_OBJECT_PATH,
            null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, signalName, parameters) => {
                this._handlePopupServiceSignal(signalName, parameters);
            }
        );
    }

    /**
     * popup-service `org.atit.unim.Popup` signal 처리.
     *
     * GNOME Wayland 환경에서는 Mutter 가 wlr-layer-shell·xdg_popup·input_popup_v2 모두
     * 지원 안 해서 popup-service GTK4 popup 이 화면에 표시되지 않는다. 따라서
     * extension 이 popup-service signal 을 받아 St 위젯으로 직접 렌더한다 — 단,
     * popup-service 는 여전히 origin (signal 발행 + RPC 수신) 이며 SoT 는 daemon
     * 의 PopupRenderPayload 1개. extension 은 평면 payload 를 그대로 그리는
     * thin renderer 일 뿐 자체 상태 절대 보유 안 함.
     *
     * 처리 대상:
     *   ShowHanjaPopup / ShowSpecialPopup / ShowEmojiPopupV2 → cursor 좌표로 popup show
     *   HidePopup                                           → hide
     *   PopupRender                                         → grid/header/footer/tabs 갱신
     *   PopupNavigate                                       → 무시 (PopupRender 가 동반 발행됨)
     *   HanjaBookmarkChanged / HanjaCandidatesReordered     → 무시 (동일)
     * @private
     */
    _handlePopupServiceSignal(signalName, parameters) {
        try {
            if (signalName === 'ShowHanjaPopup' && this._onShowHanja) {
                // (target, candidates, top_row, x, y, w, h)
                this._onShowHanja(this._extractCursor(parameters, 3));
            } else if (signalName === 'ShowSpecialPopup' && this._onShowSpecial) {
                this._onShowSpecial(this._extractCursor(parameters, 3));
            } else if (signalName === 'ShowEmojiPopupV2' && this._onShowEmoji) {
                // (target, items, top_row, recent, categories, x, y, w, h, home_row)
                this._onShowEmoji(this._extractCursor(parameters, 5));
            } else if (signalName === 'HidePopup' && this._onHidePopup) {
                this._onHidePopup();
            } else if (signalName === 'PopupRender' && this._onPopupRender) {
                this._onPopupRender(parameters);
            }
            // PopupNavigate / HanjaBookmarkChanged / HanjaCandidatesReordered 는
            // PopupRender 가 동반 발행되므로 별도 처리 불요.
        } catch (e) {
            unimError('DBUS_IME', `popup-service signal 처리 오류 (${signalName}): ${e.message}`);
        }
    }

    /**
     * Show*Popup tuple 에서 cursor (x, y, w, h) 추출.
     * @param {GLib.Variant} parameters
     * @param {number} base - cursor.x 시작 인덱스 (Hanja/Special=3, Emoji=5)
     * @returns {{x:number,y:number,width:number,height:number}|null}
     * @private
     */
    _extractCursor(parameters, base) {
        try {
            const arr = parameters.deep_unpack();
            return {
                x: arr[base],
                y: arr[base + 1],
                width: arr[base + 2],
                height: arr[base + 3],
            };
        } catch (e) {
            unimError('DBUS_IME', `cursor 추출 실패: ${e.message}`);
            return null;
        }
    }

    /**
     * InputContext 시그널 처리
     * @param {string} signalName
     * @param {GLib.Variant} parameters
     * @private
     */
    _handleContextSignal(signalName, parameters, isOwnContext = false) {
        // popup 관련 signal (ShowHanjaPopup / ShowSpecialPopup / ShowEmojiPopupV2 /
        // HidePopup / PopupNavigate / PopupRender / HanjaBookmarkChanged /
        // HanjaCandidatesReordered) 은 popup-service `org.atit.unim.Popup` interface
        // 가 단일 SoT. GNOME ext 는 popup-service 측 구독 핸들러
        // _handlePopupServiceSignal 에서 ZWSP preedit 트릭만 처리.
        try {
            if (signalName === 'AutoTypefixApply' && isOwnContext && this._onAutoTypeFix) {
                const [deleteChars, commitText, preeditText] = parameters.deep_unpack();
                unimLog('DBUS_IME', `AutoTypefixApply: delete=${deleteChars}, commit='${commitText}', preedit='${preeditText}'`);
                this._onAutoTypeFix(deleteChars, commitText, preeditText);
            } else if (signalName === 'CommitText' && isOwnContext && this._onCommitText) {
                // 비동기 commit 경로 (예: chord idle flush). ProcessKeyEvent return 의 commit
                // 필드와 별개로 daemon 이 InputContext path 로 emit 한 CommitText 시그널.
                const [text] = parameters.deep_unpack();
                unimLog('DBUS_IME', `CommitText: text='${text}'`);
                this._onCommitText(text);
            } else if (signalName === 'UpdatePreeditText' && isOwnContext && this._onUpdatePreedit) {
                // 비동기 preedit 갱신 (예: chord idle flush preedit 유지 모드).
                // payload: (text, cursor_pos, visible).
                const [text, cursorPos, visible] = parameters.deep_unpack();
                unimLog('DBUS_IME', `UpdatePreeditText: text='${text}', cursor=${cursorPos}, visible=${visible}`);
                this._onUpdatePreedit(text, cursorPos, visible);
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

    /**
     * 키 이벤트를 엔진으로 fire-and-forget 통지 (결과 무시)
     *
     * bare 토글 후보 수정자(Alt_R)용. 결과 무시(모드 변경은 GlobalModeChanged
     * 시그널로 인디케이터에 반영됨). vfunc 경로 블로킹/재진입 방지 — call_sync 금지.
     *
     * @param {number} keyval - 키 심볼 (GDK keyval)
     * @param {number} keycode - evdev 키코드
     * @param {number} state - 수정자 비트필드
     */
    processKeyAsync(keyval, keycode, state) {
        if (!this._icProxy) return;
        this._icProxy.call(
            'ProcessKeyEvent',
            new GLib.Variant('(uuu)', [keyval >>> 0, keycode >>> 0, state >>> 0]),
            Gio.DBusCallFlags.NONE,
            DBUS_TIMEOUT_MS,
            null,
            (proxy, res) => {
                try {
                    proxy.call_finish(res);
                } catch (e) {
                    unimError('DBUS_IME', `ProcessKeyAsync 실패: ${e.message}`);
                }
            }
        );
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
     * 필드의 content purpose(UNIM ContentPurpose 번호 체계)를 데몬에 통지.
     *
     * 인자는 UNIM 번호 체계(0 Normal,1 Password,2 Pin,3 Email,4 Number,5 Url,
     * 6 Terminal)이며, Clutter→UNIM 변환은 unim_input_method.js
     * CLUTTER_PURPOSE_TO_UNIM 이 이미 마쳤다 — 여기서는 raw 전달만 한다.
     * 시그니처 (u)는 unim-dbus/src/service.rs:2625 set_content_type(purpose: u32)
     * 계약과 일치. call_sync 사용은 FocusIn/FocusOut/Reset 과 동일 패턴 — 핵심 이유는
     * **호출 순서 보장**이다: 비동기로 보내면 FocusIn/FocusOut 과 SetContentType 의
     * 데몬 도착 순서가 뒤집혀 purpose 가 엉뚱한 포커스 구간에 적용될 수 있다.
     * (실패 로그는 unimError 라 UNIM_DEVELOP 세션에서만 남는다 — 일반 세션에서
     * 실패를 관찰하려면 개발 모드 필요.)
     * @param {number} purpose - UNIM ContentPurpose 원시값
     */
    setContentType(purpose) {
        if (!this._icProxy) return;

        try {
            this._icProxy.call_sync(
                'SetContentType',
                new GLib.Variant('(u)', [purpose >>> 0]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
        } catch (e) {
            unimError('DBUS_IME', `SetContentType 실패: ${e.message}`);
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

    // _adjustCursorRect 제거됨 (Phase 5): popup signal 핸들러 가 GNOME ext 에서
    // 제거되면서 좌표 변환 호출자 부재. popup-service GTK4 popup 은 자체 좌표
    // 계산을 수행한다.

    // ===========================================
    // popup-service `org.atit.unim.Popup` interface RPC 호출 헬퍼.
    //
    // GNOME Wayland 환경에서는 extension 이 popup 을 직접 렌더하므로 마우스 클릭·
    // 페이지 이동·확장 토글·북마크 토글·이모지 커밋 등을 popup-service 로 보내야 한다.
    // popup-service 가 daemon InputContext 로 forward 하므로 popup-owner routing 일관성
    // 유지. extension 은 인자만 wrap 해서 fire-and-forget 비동기 호출.
    // ===========================================

    /** popup-service Popup interface 의 method 를 fire-and-forget 으로 호출. @private */
    _callPopupService(method, variant) {
        const conn = this._imProxy ? this._imProxy.get_connection() : null;
        if (!conn) {
            unimError('DBUS_IME', `${method}: DBus 연결 없음`);
            return;
        }
        conn.call(
            POPUP_SERVICE_BUS,
            POPUP_OBJECT_PATH,
            POPUP_INTERFACE,
            method,
            variant,
            null,
            Gio.DBusCallFlags.NONE,
            DBUS_TIMEOUT_MS,
            null,
            (src, res) => {
                try {
                    src.call_finish(res);
                } catch (e) {
                    unimError('DBUS_IME', `${method} 실패: ${e.message}`);
                }
            }
        );
    }

    /** PopupRender 평면 payload 의 셀 클릭 → 한자 선택 */
    popupSelectHanja(index) {
        this._callPopupService('SelectHanja', new GLib.Variant('(u)', [index]));
    }

    /** PopupRender 평면 payload 의 셀 클릭 → 특수문자 선택 */
    popupSelectSpecial(index) {
        this._callPopupService('SelectSpecialChar', new GLib.Variant('(u)', [index]));
    }

    /** PopupRender 평면 payload 의 셀 클릭 → 이모지 commit (str 그대로) */
    popupCommitEmoji(emojiStr) {
        if (!emojiStr) return;
        this._callPopupService('CommitEmoji', new GLib.Variant('(s)', [emojiStr]));
    }

    /** 한자 셀 우클릭 → 북마크 토글 */
    popupToggleHanjaBookmark(index) {
        this._callPopupService('ToggleHanjaBookmark', new GLib.Variant('(u)', [index]));
    }

    /** ◀/▶ 클릭 → 페이지 이동 (direction: 0=prev, 1=next) */
    popupChangePage(direction) {
        this._callPopupService('PopupChangePage', new GLib.Variant('(i)', [direction]));
    }

    /** ⊞/⊟ 클릭 → 한자 popup 확장 모드 토글 */
    popupToggleExpand() {
        this._callPopupService('TogglePopupExpand', null);
    }

    /** 이모지 좌측 카테고리 탭 클릭 → 카테고리 전환 */
    popupSetEmojiCategory(idx) {
        this._callPopupService('SetEmojiCategory', new GLib.Variant('(u)', [idx]));
    }

    /** popup 영역 밖 클릭 등으로 popup hide 시 한자 상태 정리 */
    popupCancelHanja() {
        this._callPopupService('CancelHanja', null);
    }

    /** popup 영역 밖 클릭 등으로 popup hide 시 특수문자 상태 정리 */
    popupCancelSpecial() {
        this._callPopupService('CancelSpecialChar', null);
    }

    /**
     * 프런트엔드 등록 (멱등; 재호출 no-op)
     * @param {string} name - 프런트엔드 식별자 (예: 'gnome-shell')
     */
    registerFrontend(name) {
        if (!this._imProxy) return;
        try {
            this._imProxy.call_sync(
                'RegisterFrontend',
                new GLib.Variant('(s)', [name]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            unimLog('DBUS_IME', `RegisterFrontend('${name}') 완료`);
        } catch (e) {
            unimError('DBUS_IME', `RegisterFrontend('${name}') 실패: ${e.message}`);
        }
    }

    /**
     * 프런트엔드 등록 해제 (없으면 no-op)
     * @param {string} name - 프런트엔드 식별자
     */
    unregisterFrontend(name) {
        if (!this._imProxy) return;
        try {
            this._imProxy.call_sync(
                'UnregisterFrontend',
                new GLib.Variant('(s)', [name]),
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            unimLog('DBUS_IME', `UnregisterFrontend('${name}') 완료`);
        } catch (e) {
            unimError('DBUS_IME', `UnregisterFrontend('${name}') 실패: ${e.message}`);
        }
    }

    /**
     * 현재 등록된 프런트엔드 목록 조회
     * @returns {string[]} 프런트엔드 이름 배열 (정렬됨), 실패 시 빈 배열
     */
    getActiveFrontends() {
        if (!this._imProxy) return [];
        try {
            const result = this._imProxy.call_sync(
                'GetActiveFrontends',
                null,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null
            );
            if (!result) return [];
            const [names] = result.deep_unpack();
            return names;
        } catch (e) {
            unimError('DBUS_IME', `GetActiveFrontends 실패: ${e.message}`);
            return [];
        }
    }

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
        // AutoTypefixApply 글로벌 시그널 구독 해제
        if (this._popupSignalId > 0) {
            try {
                const bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
                bus.signal_unsubscribe(this._popupSignalId);
            } catch (_e) {
                // 버스 접근 실패 무시
            }
            this._popupSignalId = 0;
        }

        // popup-service Popup interface 시그널 구독 해제
        if (this._popupServiceSignalId > 0) {
            try {
                const bus = Gio.bus_get_sync(Gio.BusType.SESSION, null);
                bus.signal_unsubscribe(this._popupServiceSignalId);
            } catch (_e) {
                // 버스 접근 실패 무시
            }
            this._popupServiceSignalId = 0;
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
        /** @type {Function|null} popup_render 시그널 콜백 (통합 view_model state) */
        this._onPopupRender = null;
        this._onHanjaBookmarkChanged = null;
        this._onHanjaCandidatesReordered = null;

        unimLog('DBUS_IME', '자원 정리 완료');
    }
}
