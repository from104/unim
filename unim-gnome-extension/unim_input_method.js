/**
 * UNIM Input Method — Clutter.InputMethod 서브클래스
 *
 * Clutter Backend에 등록되어 Wayland 텍스트 입력 키를
 * vfunc_filter_key_event를 통해 직접 가로챗니다.
 *
 * @module unim_input_method
 */

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { unimLog, unimError, unimProbe, isUnimDevelop } from './logging.js';
import { hintKeyLogName } from './pointer_hints.js';
import {
    HINT_FIELD_FOCUS,
    createSurroundingFacts, factsOnFocusIn, factsOnFocusOut, factsOnSurrounding,
    factsOnHintSent, factsOnDocumentChanged, factsOnEngineKey, factsOnSensitive, decideResend,
    isLibreOfficeWmClass, isFirefoxWmClass, cacheStateName, decidePoke, decideHanjaInvalidate,
    pokeCancelledByPreedit, POKE_TIMEOUT_MS, shouldReportAppPress, shouldForwardSurrounding,
    FIREFOX_HOLD_MS,
} from './surrounding_facts.js';

/** 구버전 이모지 팝업이 IM engage 용으로 세우던 U+200B — 커밋·COMMIT 모드 대상이 아니다. */
const ZWSP = '\u200B';

/** 수정자 키 — IM이 가로채지 않고 Mutter에 직접 전달 (고정키 등 접근성 호환) */
const MODIFIER_KEYSYMS = new Set([
    Clutter.KEY_Shift_L, Clutter.KEY_Shift_R,
    Clutter.KEY_Control_L, Clutter.KEY_Control_R,
    Clutter.KEY_Alt_L, Clutter.KEY_Alt_R,
    Clutter.KEY_Super_L, Clutter.KEY_Super_R,
    Clutter.KEY_Meta_L, Clutter.KEY_Meta_R,
    Clutter.KEY_Hyper_L, Clutter.KEY_Hyper_R,
    Clutter.KEY_Caps_Lock, Clutter.KEY_Num_Lock,
    Clutter.KEY_Scroll_Lock,
]);

/**
 * Clutter.InputContentPurpose → UNIM ContentPurpose 매핑.
 *
 * 로컬 /usr/lib/x86_64-linux-gnu/mutter-14/Clutter-14.gir 실측 값:
 *   normal0, alpha1, digits2, number3, phone4, url5, email6, name7,
 *   password8, date9, time10, datetime11, terminal12.
 * UNIM ContentPurpose(src/config.rs:267-298): Normal0, Password1, Pin2,
 * Email3, Number4, Url5, Terminal6.
 *
 * GTK(GtkInputPurpose)·IBus(IBusInputPurpose)와 9·10 의 의미가 다르다
 * (GTK 9=PIN,10=TERMINAL / Clutter 9=date,10=time) — 두 표를 절대 재사용하지
 * 말고 이 명시 switch(Map)만 사용한다. Clutter enum 에는 PIN 이 없어
 * password(8) 만 UNIM Password(1)로 매핑되고, PIN 커버리지는
 * CLUTTER_HINT_HIDDEN_TEXT 폴백(_effectivePurpose)이 담당한다.
 */
const CLUTTER_PURPOSE_TO_UNIM = new Map([
    [0, 0],  // normal      → Normal
    [1, 0],  // alpha       → Normal (GTK 표와 의도적 일치 — 오차단 방지)
    [2, 0],  // digits      → Normal
    [3, 4],  // number      → Number
    [4, 0],  // phone       → Normal
    [5, 5],  // url         → Url
    [6, 3],  // email       → Email
    [7, 0],  // name        → Normal
    [8, 1],  // password    → Password
    [9, 0],  // date        → Normal (GTK/IBus 의 9=PIN 과 무관 — passthrough 금지)
    [10, 0], // time        → Normal (GTK/IBus 의 10=TERMINAL 과 무관)
    [11, 0], // datetime    → Normal
    [12, 6], // terminal    → Terminal
]);

/**
 * Clutter.InputContentHintFlags 중 HIDDEN_TEXT 비트.
 *
 * SENSITIVE_DATA(128)는 의도적으로 미사용 — Qt 선례(unim-frontends/qt5/
 * src/input_context.cpp:499-509)와 동일하게 "카드번호 등 화면에 보이는
 * 민감 필드"까지 차단하면 오차단이 되므로 hidden_text 만 폴백 판정에 쓴다.
 */
const CLUTTER_HINT_HIDDEN_TEXT = 64;

// 포커스 없는 동안 버퍼된 purpose 의 유효 시간(µs). 정상 시나리오(purpose 도착 직후
// focus_in)는 ms 단위이므로 넉넉한 2초로 잡는다 — 이보다 오래 남은 pending 은 대상
// 필드가 사라진 stale 값으로 보고 폐기한다(과차단 방지, SPEC §2.7).
const PENDING_PURPOSE_TTL_US = 2_000_000;

// 데몬 Reset 이 되쏘는 CommitText 메아리를 무시하는 창(ms). 같은 main loop 의
// 다음 몇 dispatch 안에 도착하므로 실측 0.2ms 수준이지만, 데몬이 바쁠 때를 감안해
// 넉넉히 잡는다. 사람이 같은 글자를 다시 확정하기까지는 이보다 훨씬 오래 걸리고,
// 가드는 1회 소비되면 즉시 해제되므로 과차단 위험은 없다.
const COMMIT_ECHO_GUARD_MS = 300;

/** 마우스 버튼 눌림 비트 — 탐침이 `global.get_pointer()` 마스크에서 버튼만 뽑는 데 쓴다. */
const POINTER_BUTTON_MASK = Clutter.ModifierType.BUTTON1_MASK |
                            Clutter.ModifierType.BUTTON2_MASK |
                            Clutter.ModifierType.BUTTON3_MASK;

/**
 * 탐침용: 지금 눌려 있는 마우스 버튼 비트(`0x100` = 주 버튼). 좌표는 읽어도 남기지 않는다.
 * 일반 모드에서도 부른다(탐침 줄은 호출 앞에서 `isUnimDevelop()` 로 거른다).
 * @returns {string} 예: '0x0', '0x100'
 */
function probePointerButtons() {
    try {
        const mods = global.get_pointer()[2];
        return '0x' + ((mods & POINTER_BUTTON_MASK) >>> 0).toString(16);
    } catch (_e) {
        return 'err';
    }
}

/**
 * 탐침용: Shift 가 눌려 있는지(0|1). 탐침 줄에서만 부른다.
 * @returns {number}
 */
function probeShiftDown() {
    try {
        return (global.get_pointer()[2] & Clutter.ModifierType.SHIFT_MASK) ? 1 : 0;
    } catch (_e) {
        return 0;
    }
}

/**
 * 탐침용: 포인터가 포커스 창 frame rect 안이고, 그 자리의 반응형 액터 조상에 창 액터
 * (Meta.WindowActor)가 있으면 1 — 앱 창 "안" 을 가리키는지(패널·오버레이 위가 아닌지) 본다.
 * 좌표는 읽어도 남기지 않는다. 앱 눌림 판정(`_noteAppPress`)에서도 일반 모드로 부른다.
 * @returns {number}
 */
function probePointerOverWindow() {
    try {
        const win = global.display.focus_window;
        if (!win) return 0;
        const [x, y] = global.get_pointer();
        const r = win.get_frame_rect();
        if (x < r.x || y < r.y || x >= r.x + r.width || y >= r.y + r.height) return 0;
        let a = global.stage.get_actor_at_pos(Clutter.PickMode.REACTIVE, x, y);
        while (a) {
            if (a instanceof Meta.WindowActor) return 1;
            a = a.get_parent();
        }
        return 0;
    } catch (_e) {
        return 0;
    }
}

/**
 * UnimInputMethod
 *
 * Clutter.InputMethod를 상속하여 vfunc_filter_key_event를 구현합니다.
 * Backend에 등록되면 모든 Wayland 텍스트 입력 키가 이 함수를 통과합니다.
 */
export const UnimInputMethod = GObject.registerClass(
class UnimInputMethod extends Clutter.InputMethod {
    _init() {
        super._init();

        /** @type {string} 현재 preedit 텍스트 */
        this._preeditText = '';
        /** @type {boolean} IME 활성 상태 */
        this._active = false;
        /** @type {boolean} 포커스 상태 */
        this._hasFocus = false;
        /** @type {Function|null} 키 핸들러 콜백 (KeyHandler에서 설정) */
        this._keyHandler = null;
        /** @type {Function|null} 포커스 상실 콜백 */
        this._focusOutHandler = null;
        /** @type {Function|null} 리셋 콜백 (팝업 정리 등) */
        this._resetHandler = null;
        /** @type {Function|null} 팝업이 보이는지 묻는 콜백 (빈 preedit 왕복 판정용, extension.js 가 설정) */
        this._popupVisibleQuery = null;
        /**
         * 빈 preedit 왕복(S0b) 진행 상태. 없으면 null.
         * @type {{atMs: number, reported: boolean, timeoutId: number, idleId: number}|null}
         */
        this._poke = null;
        /** @type {Function|null} 앱 창 눌림 콜백 (KeyHandler 가 설정, S4) */
        this._appPressHandler = null;
        /** 직전에 보낸 앱 눌림의 버튼 마스크·시각 — 같은 눌림의 중복 reset 거르기 */
        this._lastAppPress = { mask: 0, atMs: -Infinity };
        /**
         * 탐침 전용(develop 모드에서만 기록): 이번 키가 `factsOnEngineKey` 로 세어지기 **전**의 캐시 상태 이름.
         * 한자키 자신이 keySent 를 세우므로, 키 핸들러 쪽에서 라이브 상태를 읽으면 늘 `keysent` 가 된다.
         * @type {('none'|'dirty'|'keysent'|'fresh')|null}
         */
        this._probeCacheBeforeKey = null;
        /** @type {Function|null} bare 토글 후보 수정자(Alt_R) 비소비 통지 콜백 */
        this._toggleKeyNotifier = null;
        /** @type {{x: number, y: number, width: number, height: number}} 커서 위치 */
        this._cursorRect = { x: 0, y: 0, width: 0, height: 0 };
        /**
         * 마지막 키 이벤트 timestamp (ms). vfunc_set_cursor_location 에서 cursor 점프가
         * 키 입력으로 인한 것인지(POST_KEY_GRACE_MS 이내) 외부 클릭으로 인한 것인지
         * 구분하는 기준. 0 이면 아직 키 입력 없음.
         */
        this._lastKeyEventTimeMs = 0;
        /**
         * 직전 키 이벤트 시각(ms) — `vfunc_filter_key_event` 가 `_lastKeyEventTimeMs` 를 갱신하기
         * 직전 값. 한자키 처리 시점에 "한자키 앞 키로부터 지난 시간"(Firefox 왕복 정착)을 잰다.
         */
        this._prevKeyEventTimeMs = 0;
        /** 마지막 문서 변경(확정·삭제) 시각(ms, `Date.now()`) — Firefox 왕복 정착을 키와 함께 잰다. */
        this._lastDocChangeMs = 0;
        /**
         * Firefox 낡음 표시 뒤 왕복 응답이 아닌 보고를 데몬에 넘기지 않는 기한(단조 ms). 0 이면 없음.
         * 문서 변경·포커스 변화에서 푼다(HANJA_WORD_SPEC §2.4b.6 Firefox).
         */
        this._ffHoldUntilMs = 0;
        /** @type {Object|null} DBus IME 클라이언트 연동 */
        this._dbusIME = null;
        /** @type {number} 최근 vfunc_update_content_purpose 로 받은 Clutter 원시값 */
        this._contentPurposeRaw = 0;
        /** @type {number} 최근 vfunc_update_content_hints 로 받은 Clutter 힌트 비트필드 */
        this._contentHints = 0;
        /**
         * 포커스 없는 동안 도착한 purpose — focus_in 시 flush.
         * null 이면 대기 중인 값 없음(포커스 중 도착분은 즉시 송신하므로 버퍼 불필요).
         * @type {number|null}
         */
        this._pendingPurpose = null;
        /** @type {number} pending 기록 시각(µs, monotonic) — TTL 판정용 */
        this._pendingPurposeTime = 0;
        /** @type {number} 마지막으로 데몬에 실제 송신한 purpose (dedupe 기준) */
        this._sentPurpose = 0;
        /** @type {Function|null} content purpose 변경 콜백 (dbus_ime.setContentType 배선) */
        this._contentTypeHandler = null;
        /**
         * AutoTypeFix가 vkbd로 보낸 BackSpace의 self-feedback 방지 카운터.
         * vkbd 이벤트가 mutter→IM filter로 재진입할 때, 한글 엔진이 새 preedit을
         * 깎아먹지 않도록 IM 처리를 우회시킨다. PRESS+RELEASE 한 쌍이 1개의 backspace.
         * @type {number}
         */
        this._selfBackspaceCount = 0;
        /**
         * 마지막으로 받은 surrounding text와 cursor/anchor.
         * gedit/gnome-text-editor처럼 선택 정보를 보내지 않는 앱에서도
         * TypeFix를 동작하게 하려면, request_surrounding() 호출 시 최신 값을 캐시해야 함.
         * @type {{text: string, cursor: number, anchor: number}}
         */
        this._lastSurrounding = { text: '', cursor: 0, anchor: 0 };
        /**
         * 필드 세대 + 마지막 스냅샷 캐시 (surrounding_facts.js 의 순수 상태).
         * FocusIn 계열을 보낸 직후의 재송신 판정과 ReplaceSurrounding 대조에 쓴다.
         * @type {import('./surrounding_facts.js').SurroundingFacts}
         */
        this._facts = createSurroundingFacts();

        unimLog('IME', 'UnimInputMethod 인스턴스 생성');
    }

    /**
     * DBus IME 클라이언트 주입
     * @param {Object} dbusIME
     */
    setDbusIME(dbusIME) {
        this._dbusIME = dbusIME;
    }

    /**
     * KeyHandler의 키 처리 콜백 등록
     *
     * 콜백이 **false 를 돌려주면 이 이벤트를 가로채지 않는다**(vfunc_filter_key_event
     * 가 그대로 false 를 반환). 그 밖의 값은 "가로챘음"으로 보고, 키 전달 여부는
     * 콜백이 부른 notify_key_event 가 정한다.
     *
     * @param {Function|null} handler - (keyval, evdevKeycode, state, event) => boolean|undefined
     */
    setKeyHandler(handler) {
        this._keyHandler = handler;
    }

    /**
     * 포커스 상실 콜백 등록
     * @param {Function|null} handler - () => void
     */
    setFocusOutHandler(handler) {
        this._focusOutHandler = handler;
    }

    /**
     * 현재 preedit 텍스트가 비어있지 않은지 — 외부 click 시 reset 필요 여부 판단용.
     * @returns {boolean}
     */
    hasPreedit() {
        return typeof this._preeditText === 'string' && this._preeditText.length > 0;
    }

    /**
     * 리셋 콜백 등록 (팝업 정리 등)
     * @param {Function|null} handler - () => void
     */
    setResetHandler(handler) {
        this._resetHandler = handler;
    }

    /**
     * bare 토글 후보 수정자(Alt_R) 비소비 통지 콜백 등록
     * @param {Function|null} cb - (keyval, evdevKeycode, state) => void
     */
    setToggleKeyNotifier(cb) {
        this._toggleKeyNotifier = cb;
    }

    /**
     * content purpose 변경 콜백 등록 (dbus_ime.js setContentType 배선)
     * @param {Function|null} handler - (unimPurpose: number) => void
     */
    setContentTypeHandler(handler) {
        this._contentTypeHandler = handler;
    }

    /**
     * 현재 Clutter 포커스 보유 여부 — extension.js 창 전환 fail-safe 게이트용
     * (Clutter 포커스가 살아있는 동안에는 별도 경로로 Normal 을 강제하지 않기 위함).
     * @returns {boolean}
     */
    hasFocus() {
        return this._hasFocus;
    }

    // ===========================================
    // GObject vfunc 오버라이드 — Mutter C vtable에 등록됨
    // ===========================================

    /**
     * Mutter가 텍스트 입력 키를 전달할 때 호출 (C vtable 경유)
     *
     * IBus와 동일한 패턴: 항상 true를 반환하여 키를 소비하고,
     * 처리 후 notify_key_event(event, consumed)로 키 전달 여부를 사후 통보.
     *
     * @param {Clutter.Event} event
     * @returns {boolean} 항상 true (IBus 패턴)
     */
    vfunc_filter_key_event(event) {
        if (!event) return false;
        if (!this._active) return false;

        // 키 이벤트 timestamp 갱신 — vfunc_set_cursor_location 의 cursor 점프
        // 감지에서 "최근 키 입력에 의한 cursor 이동인지" 구분에 사용.
        this._prevKeyEventTimeMs = this._lastKeyEventTimeMs;
        this._lastKeyEventTimeMs = Date.now();

        const eventType = event.type();
        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();
        const flags = event.get_flags ? event.get_flags() : 0;
        const typeName = eventType === Clutter.EventType.KEY_PRESS ? 'PRESS' :
                         eventType === Clutter.EventType.KEY_RELEASE ? 'RELEASE' : `OTHER(${eventType})`;

        // 평문 금지(HANJA_WORD_SPEC §2.8) — 탐색·편집 키만 이름을 남기고 문자 키는 `char` 로 가린다
        // (keyval·keycode 를 그대로 찍으면 개발 로그가 입력 내용을 복원할 수 있는 키 로거가 된다).
        unimLog('IME', `vfunc_filter: type=${typeName}, key=${hintKeyLogName(keyval)}, state=0x${state.toString(16)}, flags=0x${flags.toString(16)}`);

        // AutoTypeFix가 vkbd로 보낸 BackSpace는 IM 처리 없이 mutter에 직접 통과
        // (vkbd → mutter → IM filter 재진입 시 한글 엔진이 새 preedit을 깎는 self-feedback 차단)
        // PRESS와 RELEASE를 각각 1씩 차감 (한 backspace = 2 이벤트)
        if (keyval === Clutter.KEY_BackSpace && this._selfBackspaceCount > 0) {
            this._selfBackspaceCount--;
            unimLog('IME', `self-sent BackSpace 우회 (${typeName}, 남은=${this._selfBackspaceCount})`);
            return false;
        }

        // 수정자 키 단독 입력은 IM이 가로채지 않음 (return false)
        // → Mutter가 직접 처리하여 고정키(Sticky Keys) 등 접근성 기능 정상 작동
        // (return true + notify_key_event(false) 패턴은 FLAG_INPUT_METHOD 플래그를
        //  부착하여 고정키 핸들러가 이벤트를 무시하는 원인이 됨)
        if (MODIFIER_KEYSYMS.has(keyval)) {
            // bare Alt_R PRESS 는 데몬에 비소비 통지(fire-and-forget) — 토글 여부는 데몬
            // toggle_keys 가 판정(§3.4). return false 는 유지해 Mutter 네이티브 처리
            // (위 고정키/Sticky Keys 보호)를 보존한다. 앱에도 Alt press+release 가
            // 전달되는 부작용은 계획서 R1 (QA A-4b, 옵트아웃: toggle_keys 에서 RightAlt 제거).
            if (keyval === Clutter.KEY_Alt_R &&
                eventType === Clutter.EventType.KEY_PRESS &&
                this._toggleKeyNotifier) {
                this._toggleKeyNotifier(keyval, keycode > 8 ? keycode - 8 : 0, state);
            }
            return false;
        }

        // KEY_RELEASE: 처리하지 않지만 notify_key_event는 반드시 호출
        // (Mutter의 키 상태 추적 유지 — 누락 시 키 반복이 멈추지 않음)
        if (eventType !== Clutter.EventType.KEY_PRESS) {
            this.notify_key_event(event, false);
            return true;
        }

        // 탐침(develop 전용): 이 키가 세어지기 전의 캐시 상태를 먼저 찍어 둔다 — 한자키 판정용.
        // 공유 상태(factsOnEngineKey)는 건드리지 않는다.
        if (isUnimDevelop()) this._probeCacheBeforeKey = cacheStateName(this._facts);

        // 엔진으로 가는 키(통과 키는 앱 문서를 바꾼다) — 캐시 재송신 금지 판정에 알린다.
        this._facts = factsOnEngineKey(this._facts);

        // 키 핸들러가 직접 notify_key_event를 호출
        // (키 큐 패턴: call_sync 중 재진입 키를 큐에 저장 후 순차 처리)
        //
        // 반환값 규약: 핸들러가 **명시적으로 false** 를 돌려주면 이 이벤트를
        // 가로채지 않는다(아래 "왜 false 가 중요한가" 참조). 그 밖의 값
        // (undefined 포함)은 종전대로 "가로챘음"(true)으로 본다 — 이때 키 전달
        // 여부는 핸들러가 이미 호출한 notify_key_event 가 정한다.
        //
        // ── 왜 false 가 중요한가 (mutter 46 소스 확인) ──────────────────────
        // src/core/events.c:meta_display_handle_event 의 순서가 이렇다:
        //
        //   if (meta_wayland_text_input_update (...))   // ← 여기서 이 vfunc 이 불린다
        //     return CLUTTER_EVENT_STOP;               // ← true 면 즉시 반환
        //   meta_wayland_compositor_update (...);      // ← 그래서 이 줄을 건너뛴다
        //
        // `meta_wayland_compositor_update` 는 wayland 키보드의 xkb 상태를 갱신하고
        // **고정키 래치 마스크를 다시 입히는**(kbd_a11y_apply_mask) 지점이다.
        // true 를 돌려주면 원본 이벤트가 이 지점을 못 지나가고, notify_key_event 가
        // 새로 큐에 넣은 사본이 **다음 메인 루프 회차에** 대신 지나간다.
        //
        // 그런데 고정키 래치 해제는 mutter 의 **입력 스레드**가 물리적 키 뗌 시점에
        // 곧바로 통지한다(meta-input-device-native.c:handle_stickykeys_release →
        // notify_stickykeys_mask). 즉 "래치를 지우는 콜백"과 "미뤄진 키 사본"이
        // 메인 루프에서 경합하고, 전자가 이기면 사본은 수정자가 빠진 채 앱에 닿는다.
        // 이것이 Ctrl/Alt 고정키가 '가끔' 안 먹던 원인이다.
        //
        // 수정자 키 단독을 위에서 return false 로 빼둔 것도 같은 이유다(위 주석).
        // 소비하지 않을 키라면 가로채지 말고 mutter 에게 그대로 넘기는 편이
        // 언제나 옳다 — 사본도, 경합도, FLAG_INPUT_METHOD 도 생기지 않는다.
        if (this._keyHandler) {
            const evdevKeycode = keycode > 8 ? keycode - 8 : 0;
            try {
                if (this._keyHandler(keyval, evdevKeycode, state, event) === false)
                    return false;
            } catch (e) {
                unimError('IME', `vfunc_filter_key_event 오류: ${e.message}`);
                this.notify_key_event(event, false);
            }
        } else {
            this.notify_key_event(event, false);
        }

        return true;
    }

    vfunc_focus_in(_focus) {
        this._hasFocus = true;
        this._ffHoldUntilMs = 0;
        this._facts = factsOnFocusIn(this._facts, GLib.get_monotonic_time() / 1000);
        unimLog('IME', `vfunc_focus_in: _hasFocus=true`);
        if (isUnimDevelop()) {
            const c = this._facts.cache;
            unimProbe('focus_in',
                `gen=${this._facts.fieldGen} cache=${c ? (c.gen === this._facts.fieldGen ? 'same_gen' : 'old_gen') : 'none'}` +
                `${c ? ` cache_age_ms=${Math.round(GLib.get_monotonic_time() / 1000 - c.atMs)}` : ''} btn=${probePointerButtons()}`);
        }

        this._flushPendingPurpose();

        // 필드(위젯) 단위 포커스를 엔진에 알린다 — 창 FocusIn(extension.js)은 필드가 바뀌어도 오지 않는다.
        // 구버전 데몬은 알 수 없는 kind 를 무시한다. 비동기라 키 경로를 막지 않는다.
        // purpose flush 뒤에 보내므로 비밀번호 칸이면 엔진이 Password 를 먼저 안 상태로 받는다.
        if (this._dbusIME?.isConnected) {
            this._dbusIME.surroundingHintAsync(HINT_FIELD_FOCUS, 0, 0);
            unimLog('IME', `SurroundingHint: kind=${HINT_FIELD_FOCUS} (FIELD_FOCUS)`);
            unimProbe('field_focus_sent', `gen=${this._facts.fieldGen}`);
            this.resendSurroundingFacts('field_focus');
        }
    }

    /**
     * 포커스 없는 동안 도착해 대기 중이던 purpose 만 flush.
     *
     * 무조건 Normal 송신은 하지 않는다 — Mutter 가 vfunc_update_content_purpose 를
     * vfunc_focus_in 보다 먼저 호출하는 순서라면(호출 순서 미확정, gnome_reset_design
     * 근거 1) 방금 받은 Password 를 여기서 즉시 Normal 로 덮어써 비번 차단을
     * 영구 무력화하게 된다. Normal 복귀는 vfunc_focus_out 에서만 담당.
     * @private
     */
    _flushPendingPurpose() {
        if (this._pendingPurpose === null) return;
        const p = this._pendingPurpose;
        const age = GLib.get_monotonic_time() - (this._pendingPurposeTime ?? 0);
        this._pendingPurpose = null;
        // TTL 2초 — 대상 필드가 포커스를 끝내 못 받고 사라진 뒤(창 닫힘 등)
        // 남은 stale pending 이 무관한 다음 필드에 재적용되는 over-block 을
        // 막는다(SPEC §2.7 알려진 단방향 한계). 정상 순서(purpose 직후 focus_in)
        // 는 ms 단위라 TTL 에 걸리지 않는다.
        if (age > PENDING_PURPOSE_TTL_US) {
            unimLog('IME', `pending purpose ${p} 폐기 (age=${Math.round(age / 1000)}ms > TTL)`);
            return;
        }
        if (p !== this._sentPurpose) {
            this._contentTypeHandler?.(p);
            this._sentPurpose = p;
        }
    }

    vfunc_focus_out() {
        unimLog('IME', `vfunc_focus_out 호출됨 (stack: popup commit 디버깅)`);
        unimProbe('focus_out', `gen=${this._facts.fieldGen}`);
        this._ffHoldUntilMs = 0;
        this._cancelPoke('focus_out');
        // 1. 커밋 최우선: 로컬 preedit 백업
        // ZWSP 잔재(구버전 이모지 우회책)는 커밋 대상이 아니다.
        const localPreedit = this._preeditText === ZWSP ? '' : this._preeditText;

        this._hasFocus = false;

        // 2. 포커스 상실 콜백 호출 (DBus FocusOut → 커밋 텍스트 수신)
        let committed = false;
        if (this._focusOutHandler) {
            try {
                committed = this._focusOutHandler();
            } catch (e) {
                unimError('IME', `focusOut 핸들러 오류: ${e.message}`);
            }
        }

        // 3. DBus 커밋 실패 시 로컬 preedit 폴백
        //
        // 보통은 여기 도달하기 전에 localPreedit 이 이미 비어 있다 — mutter 의
        // focus 이동 경로(meta_wayland_text_input_set_focus)가
        // clutter_input_focus_reset() → (COMMIT 모드 커밋) → vfunc_reset →
        // flush_done → focus_out 순서라, vfunc_reset 이 먼저 preedit 을 비우기 때문.
        // 그 순서가 깨지는 경로에 대비한 폴백으로만 남긴다.
        if (!committed && localPreedit && localPreedit.length > 0) {
            this._preeditText = '';
            try {
                // commit 이 먼저 (_flushPreedit 과 같은 이유 — 조합 자리 앵커 유지)
                this._lastDocChangeMs = Date.now();
                this.commit(localPreedit);
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
            } catch (e) {
                unimError('IME', `focusOut 로컬 커밋 실패: ${e.message}`);
            }
        } else if (this._preeditText.length > 0) {
            this.clearPreedit();
        }

        // 4. content purpose Normal 복귀 — 단일 영속 InputContext(dbus_ime.js) 구조라
        // 이 리셋이 빠지면 비번 필드에서 세운 차단이 세션 전체에 잔존(크로스앱 누출)한다.
        // focus_out 은 실측(wl-wayland-0 로그, IN/OUT 엄격 교대 221/221)상 빠짐없이
        // 호출되므로 리셋 지점으로 채택(gnome_reset_design 근거 2). 이미 Normal 이면
        // 재전송 생략(dedupe).
        if (this._sentPurpose !== 0) {
            this._contentTypeHandler?.(0);
            this._sentPurpose = 0;
        }
        this._contentPurposeRaw = 0;
        this._contentHints = 0;
        // 5. surrounding 캐시 초기화 — 이전 입력란의 본문이 다음 입력란(surrounding 을
        // 안 보내는 VTE 등)에서 "지금 필드의 surrounding" 으로 쓰이면 안 된다.
        // **새 객체**로 갈아 끼워 이전 필드의 참조를 끊는다(SPEC.md §2.8).
        // focus_in 에서는 지우지 않는다 — Mutter 가 set_surrounding 을
        // 먼저 보낸 순서라면 유효한 값을 지우게 된다.
        this._lastSurrounding = { text: '', cursor: 0, anchor: 0 };
        this._facts = factsOnFocusOut(this._facts);
        // _pendingPurpose 는 지우지 않는다 — 포커스 없는 동안 이미 도착한 다음 필드의
        // purpose 일 수 있으며, 그 몫은 다음 vfunc_focus_in 이 flush 한다.
    }

    vfunc_reset() {
        this._cancelPoke('reset');
        this._probeReset(false);
        this._noteAppPress();
        // content purpose 는 건드리지 않는다 — reset 은 필드 "내부" 조합 취소(예:
        // Escape) 이벤트라, 여기서 Normal 로 되돌리면 비밀번호 필드 안에서 Escape
        // 한 번에 차단이 풀리는 회귀가 된다(GTK3 감사관의 "reset 에서도 Normal" 권고는
        // 이 이유로 불채택 — gnome_reset_design 부수 규칙).
        // 1. 리셋 핸들러 호출 (팝업 열려있으면 커밋+닫기)
        if (this._resetHandler) {
            try {
                this._resetHandler();
            } catch (e) {
                unimError('IME', `reset 핸들러 오류: ${e.message}`);
            }
        }

        // 2. preedit 커밋은 **여기서 하지 않는다** — mutter 가 이미 했다.
        // clutter_input_focus_reset() 은 priv->mode == COMMIT 이면
        // clutter_input_focus_commit() 을 호출한 **뒤에야**
        // clutter_input_method_reset() → 이 vfunc 을 부른다(updatePreedit 주석 참조).
        // 여기서 또 commit() 하면 같은 글자가 두 번 들어간다.
        //
        // 주의: mutter 를 거치지 않고 이 vfunc 을 직접 호출하는 경로
        // (vfunc_set_cursor_location 의 cursor-jump 감지)는 스스로 flushPreedit() 로
        // 커밋한 뒤 들어와야 한다 — 그 경로엔 mutter 의 commit 이 없다.
        if (this._preeditText.length > 0) {
            const flushed = this._preeditText;
            this._preeditText = '';
            // mutter 가 방금 조합을 확정했다 — Firefox 왕복 정착(msSinceLastActivity)에 센다.
            this._lastDocChangeMs = Date.now();
            // 엔진 상태 초기화. 단 데몬의 Reset 은 비운 조합을 **CommitText 시그널로
            // 되쏜다**(unim-dbus/src/service.rs 의 reset(): `Self::commit_text(...)`).
            // 그 메아리는 비동기라 **캐럿이 이미 이동한 뒤에** 도착하고, 그대로 커밋하면
            // 클릭한 자리에 같은 글자가 한 번 더 들어간다. 이미 mutter 가 제자리에
            // 커밋했으므로 짧은 창 동안 같은 문자열의 메아리를 무시한다.
            this._armCommitEchoGuard(flushed);
            if (this._dbusIME) {
                this._dbusIME.reset();
            }
        }
    }

    /**
     * 앱 눌림 핸들러를 설정한다 (KeyHandler 가 포인터 분류기와 연결). 앱 창 안 클릭은 stage 가 못 보므로
     * mutter 가 앱 표면 눌림에서 부르는 `vfunc_reset` 이 유일한 신호다.
     * @param {Function|null} handler - `() => void`
     */
    setAppPressHandler(handler) {
        this._appPressHandler = handler;
    }

    /**
     * `vfunc_reset` 시점의 버튼 상태로 앱 창 안 눌림을 알린다. 판정은 `shouldReportAppPress`(순수 함수).
     * cursor-jump 직접 경로는 `vfunc_reset` 을 거치지 않으므로 여기 오지 않는다(direct=false 고정).
     * @private
     */
    _noteAppPress() {
        if (!this._appPressHandler) return;
        try {
            const mods = global.get_pointer()[2];
            const btnMask = (mods & POINTER_BUTTON_MASK) >>> 0;
            const nowMs = GLib.get_monotonic_time() / 1000;
            const last = this._lastAppPress;
            const d = shouldReportAppPress({
                direct: false,
                btnMask,
                overWin: btnMask !== 0 && Main.modalCount === 0 && probePointerOverWindow() === 1,
                popup: this._isPopupVisible(),
                nowMs, lastMs: last.atMs, lastMask: last.mask,
            });
            if (!d.send) return;
            this._lastAppPress = { mask: btnMask, atMs: nowMs };
            this._appPressHandler(btnMask);
        } catch (e) {
            unimError('IME', `앱 눌림 보고 오류(무시): ${e.message}`);
        }
    }

    /**
     * 탐침 `reset` 줄 — mutter 의 클릭 reset 시점에 버튼·Shift 가 보이는지, 포인터가 앱 창
     * 안인지 잰다(대상③ S0a). develop 모드가 아니면 아무 일도 하지 않는다. 평문·좌표는 남기지 않는다.
     * @param {boolean} direct - vfunc_reset 이 아니라 cursor-jump 경로에서 부른 경우
     * @private
     */
    _probeReset(direct) {
        if (!isUnimDevelop()) return;
        unimProbe('reset',
            `btn=${probePointerButtons()} shift=${probeShiftDown()} ` +
            `preedit_len=${Array.from(this._preeditText ?? '').length} ` +
            `popup=${this._isPopupVisible() ? 1 : 0} over_win=${probePointerOverWindow()} ` +
            `direct=${direct ? 1 : 0}`);
    }

    /**
     * 팝업이 보이는지 묻는 콜백을 설정한다 (extension.js 가 PopupView 를 만든 뒤 설정).
     * @param {Function|null} query
     */
    setPopupVisibleQuery(query) {
        this._popupVisibleQuery = query;
    }

    /** @returns {boolean} 팝업이 보이는지 (KeyHandler 의 PRIMARY_EMPTY 판정용) */
    isPopupShown() {
        return this._isPopupVisible();
    }

    /** @returns {boolean} @private */
    _isPopupVisible() {
        try {
            return !!this._popupVisibleQuery?.();
        } catch (_e) {
            return false;
        }
    }

    /**
     * 포커스 창이 LibreOffice 인지. wm_class 이름은 밖으로 내보내지 않는다.
     * @returns {boolean}
     */
    isFocusWindowLibreOffice() {
        try {
            // 셸 모달(오버뷰 검색칸·실행 대화상자 등)이 IM 포커스를 잡아도 focus_window 는 직전 앱 창으로
            // 남는다 — 그때는 LO 가 아니다(왕복이 셸 ClutterText 를 상대로 발동하지 않게).
            if (Main.modalCount > 0) return false;
            return isLibreOfficeWmClass(global.display.focus_window?.get_wm_class());
        } catch (_e) {
            return false;
        }
    }

    /**
     * 포커스 창이 Firefox 인지. wm_class 이름은 밖으로 내보내지 않는다. 셸 모달 처리는 LO 와 같다.
     * @returns {boolean}
     */
    isFocusWindowFirefox() {
        try {
            if (Main.modalCount > 0) return false;
            return isFirefoxWmClass(global.display.focus_window?.get_wm_class());
        } catch (_e) {
            return false;
        }
    }

    /**
     * 한자키 직전 키 이벤트·마지막 문서 변경(확정·삭제) 중 늦은 것으로부터 지난 시간(ms). 둘 다 기록이
     * 없으면 매우 큰 값. 키가 큐에서 늦게 처리되거나(확정이 방금) 마우스로 후보를 고른 직후에도
     * 확정 바로 뒤의 왕복을 막는다.
     * @returns {number}
     */
    msSinceLastActivity() {
        const last = Math.max(this._prevKeyEventTimeMs, this._lastDocChangeMs);
        return last > 0 ? Date.now() - last : Number.MAX_SAFE_INTEGER;
    }

    /**
     * Firefox 낡음 표시 직후 부른다 — `FIREFOX_HOLD_MS` 동안 왕복 응답이 아닌 보고를 데몬에 넘기지 않는다.
     */
    beginFirefoxHold() {
        this._ffHoldUntilMs = GLib.get_monotonic_time() / 1000 + FIREFOX_HOLD_MS;
    }

    /**
     * 한자키를 엔진에 보내기 직전 마지막 보고를 낡은 것으로 표시해야 하는지(`decideHanjaInvalidate`).
     * KeyHandler 가 참이면 KEY 힌트를 보낸 뒤 왕복을 건다.
     * @returns {boolean}
     */
    shouldInvalidateBeforeHanja() {
        return decideHanjaInvalidate({
            hasFocus: this._hasFocus,
            preeditEmpty: this._preeditText === '',
            sensitive: this.isSensitivePurpose(),
            popupVisible: this._isPopupVisible(),
            isFirefox: this.isFocusWindowFirefox(),
        });
    }

    /**
     * vfunc_filter_key_event 가 방금 처리한 키가 세어지기 **전**의 캐시 상태 이름(develop 모드에서만 채워짐).
     * 호출 직후 동기 경로에서 읽어야 정확하다. 큐에 들어가는 키는 호출자가 이 값을 항목에 실어 둔다.
     * @returns {('none'|'dirty'|'keysent'|'fresh')|null}
     */
    cacheStateBeforeKey() {
        return this._probeCacheBeforeKey;
    }

    /**
     * 탐침·왕복 판정에 쓸 캐시 상태 이름. 키 직전 스냅샷이 있으면 그것을, 없으면 라이브 상태를 쓴다.
     * @param {{cache: string|null, approx?: boolean}|undefined} before
     * @returns {string}
     * @private
     */
    _cacheStateForHanjaKey(before) {
        return before?.cache ?? cacheStateName(this._facts);
    }

    /**
     * 탐침 `hanja_key` 용 스냅샷 요약 — 문서 내용 없이 상태 이름·나이·길이만.
     * @param {{cache: string|null, approx?: boolean}} [before] - 키 직전 캐시 상태 스냅샷
     *   (없으면 라이브 상태). `approx` 면 큐에서 늦게 처리된 키라 근사값이다.
     * @returns {{lo: number, ff: number, cache: string, cacheAgeMs: number, preeditLen: number, snap: string}}
     */
    probeHanjaKeyState(before) {
        const c = this._facts.cache;
        return {
            lo: this.isFocusWindowLibreOffice() ? 1 : 0,
            ff: this.isFocusWindowFirefox() ? 1 : 0,
            cache: this._cacheStateForHanjaKey(before),
            snap: before?.cache ? (before.approx ? 'queued' : 'pre') : 'live',
            cacheAgeMs: c ? Math.round(GLib.get_monotonic_time() / 1000 - c.atMs) : -1,
            preeditLen: Array.from(this._preeditText ?? '').length,
        };
    }

    /**
     * 빈 preedit 왕복(대상③ S0b 실험) — LibreOffice 는 키 이동·삭제 뒤 surrounding 을 보고하지
     * 않아 엔진 R7 기한이 만료된다. 한자키를 엔진에 보내기 직전에 `""` preedit 을 세우면
     * (GTK3 `done` → `update_im` → LO retrieve) 보고가 도착하는지 본다. 보고가 오면 **다음 주기**
     * (GLib.idle_add)에 `null` 로 되돌린다 — 같은 idle 안의 done 은 mutter 가 병합하기 때문이다.
     * 250ms 안에 보고가 없으면 타임아웃으로 되돌린다.
     *
     * LibreOffice·Firefox 창에서만 발동한다(`decidePoke`, 순수 함수 — Firefox 는 캐시가 신선해 보여도
     * 왕복하되 직전 키 뒤 정착 시간을 지킨다). 일반 로그는 `poke start/end` 뿐이고
     * 세부는 develop 탐침에만 남는다.
     * 호출 위치: KeyHandler 가 한자키를 processKey 로 넘기기 직전. 로그에 평문은 없다.
     *
     * @param {{cache: string|null, approx?: boolean}} [before] - 한자키 자신이 세어지기 전의 캐시 상태
     *   (`cacheStateBeforeKey`). 없으면 라이브 상태 — 라이브는 한자키가 이미 keySent 를 세운 뒤라 늘
     *   `keysent` 로 보이므로 `cache_fresh` 건너뛰기가 영영 발동하지 못한다.
     */
    pokeSurroundingReport(before) {
        const cacheState = this._cacheStateForHanjaKey(before);
        const d = decidePoke({
            hasFocus: this._hasFocus,
            preeditEmpty: this._preeditText === '',
            sensitive: this.isSensitivePurpose(),
            popupVisible: this._isPopupVisible(),
            isLibreOffice: this.isFocusWindowLibreOffice(),
            isFirefox: this.isFocusWindowFirefox(),
            inFlight: this._poke !== null,
            cacheState,
            sinceKeyMs: this.msSinceLastActivity(),
            selMaybe: before?.selMaybe,
        });
        if (!d.poke) {
            unimProbe('poke_skip', `why=${d.reason}`);
            return;
        }
        try {
            this.set_preedit_text('', 0, 0, Clutter.PreeditResetMode.CLEAR);
        } catch (e) {
            unimError('IME', `빈 preedit 왕복 시작 실패: ${e.message}`);
            return;
        }
        const poke = { atMs: GLib.get_monotonic_time() / 1000, reported: false, timeoutId: 0, idleId: 0 };
        poke.timeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, POKE_TIMEOUT_MS, () => {
            poke.timeoutId = 0;
            if (this._poke === poke) this._endPoke('timeout');
            return GLib.SOURCE_REMOVE;
        });
        this._poke = poke;
        unimLog('IME', 'poke start');
        unimProbe('poke_start', `cache=${cacheState}`);
    }

    /**
     * vfunc_set_surrounding 이 호출한다 — 진행 중인 왕복의 첫 보고면 타임아웃을 끄고,
     * 다음 주기에 null 로 되돌리도록 idle 을 건다.
     * @private
     */
    _onPokeReport() {
        const poke = this._poke;
        if (!poke || poke.reported) return;
        poke.reported = true;
        unimProbe('poke_report', `ms=${Math.round(GLib.get_monotonic_time() / 1000 - poke.atMs)}`);
        if (poke.timeoutId) {
            GLib.source_remove(poke.timeoutId);
            poke.timeoutId = 0;
        }
        poke.idleId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            poke.idleId = 0;
            if (this._poke === poke) this._endPoke('report');
            return GLib.SOURCE_REMOVE;
        });
    }

    /**
     * 왕복을 마친다: 소스를 정리하고 preedit 을 `null` 로 되돌린다.
     * @param {'report'|'timeout'} why
     * @private
     */
    _endPoke(why) {
        const poke = this._poke;
        if (!poke) return;
        this._clearPokeSources(poke);
        this._poke = null;
        if (this._hasFocus && this._preeditText === '') {
            try {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
            } catch (e) {
                unimError('IME', `빈 preedit 왕복 종료 실패: ${e.message}`);
            }
        }
        unimLog('IME', `poke end why=${why}`);
        unimProbe('poke_end', `why=${why}`);
    }

    /**
     * 왕복을 취소한다 — null 을 보내지 않는다. 실제 preedit·커밋·reset·focus_out 이 `""` 를 대체한다.
     * @param {string} why
     * @private
     */
    _cancelPoke(why) {
        const poke = this._poke;
        if (!poke) return;
        this._clearPokeSources(poke);
        this._poke = null;
        unimLog('IME', `poke end why=${why}`);
        unimProbe('poke_end', `why=${why}`);
    }

    /** @private */
    _clearPokeSources(poke) {
        if (poke.timeoutId) {
            GLib.source_remove(poke.timeoutId);
            poke.timeoutId = 0;
        }
        if (poke.idleId) {
            GLib.source_remove(poke.idleId);
            poke.idleId = 0;
        }
    }

    /**
     * 확장 disable/IME 끄기 때 호출 — 타이머·idle 소스를 남기지 않는다(GNOME 확장 심사 규칙).
     * 진행 중이던 `""` preedit 은 null 로 되돌린다.
     */
    destroyPoke() {
        const poke = this._poke;
        if (!poke) return;
        this._clearPokeSources(poke);
        this._poke = null;
        if (this._preeditText === '') {
            try {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
            } catch (_e) { /* 종료 중 — 무시 */ }
        }
    }

    /**
     * 방금 커밋된 문자열과 같은 CommitText 메아리를 짧은 창 동안 무시하도록 무장한다.
     * @param {string} text
     * @private
     */
    _armCommitEchoGuard(text) {
        if (!text) return;
        this._commitEchoGuard = { text, until: Date.now() + COMMIT_ECHO_GUARD_MS };
    }

    /**
     * CommitText 시그널이 방금 처리한 커밋의 메아리인지 판정한다.
     * 한 번 소비되면 해제되어, 사용자가 같은 글자를 곧바로 다시 쳐도 막히지 않는다.
     * @param {string} text
     * @returns {boolean} true 면 무시해야 한다
     */
    shouldSuppressCommitEcho(text) {
        const g = this._commitEchoGuard;
        if (!g) return false;
        if (Date.now() > g.until) {
            this._commitEchoGuard = null;
            return false;
        }
        if (g.text !== text) return false;
        this._commitEchoGuard = null;   // 1회용
        return true;
    }

    /**
     * mutter 의 reset 경로를 거치지 않고 조합을 확정해야 할 때 쓰는 로컬 flush.
     *
     * commit 을 **먼저**, preedit 클리어를 **나중에** 하는 순서가 핵심이다
     * (IME_BEHAVIOR.md §"commit 처리(먼저) → preedit 처리(commit 후)").
     * commit() 은 wayland 상에서 preedit_string(NULL)+commit_string 을 한 done
     * 배치로 보내므로 조합 중이던 자리에 앵커된다. 그 뒤 set_preedit_text(null) 로
     * mutter 쪽 preedit 사본(priv->preedit)을 비워, 뒤늦게 진짜 reset 이 와도
     * COMMIT 모드가 같은 글자를 한 번 더 커밋하지 않게 한다.
     * @private
     */
    _flushPreedit() {
        const preedit = this._preeditText;
        if (!preedit || preedit.length === 0) return;
        this._preeditText = '';
        if (preedit === ZWSP) {
            // 구버전 ZWSP 잔재는 문서에 커밋하지 않고 버린다(v3.5).
            this.clearPreedit();
            return;
        }
        // 호출자가 곧이어 데몬 reset() 을 부르면 같은 글자가 CommitText 로 되돌아온다.
        // 미리 가드를 무장해 그 메아리를 흘린다(vfunc_reset 주석 참조).
        this._armCommitEchoGuard(preedit);
        this._noteDocumentChanged();
        try {
            this.commit(preedit);
            this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
        } catch (e) {
            unimError('IME', `_flushPreedit 커밋 실패: ${e.message}`);
        }
    }

    vfunc_set_cursor_location(rect) {
        if (!rect) return;
        const newRect = {
            x: rect.get_x(),
            y: rect.get_y(),
            width: rect.get_width(),
            height: rect.get_height(),
        };

        // 외부 cursor 점프 감지 — toolkit 이 reset 을 안 보내는 GNOME Shell IBus 한계 보완.
        // 같은 윈도우 안 다른 위치 마우스 클릭이나 외부 cursor 이동 시 set_cursor_location
        // 만 호출됨. preedit 활성 + 최근 키 입력 없음 + 좌표가 큰 폭으로 점프하면
        // 외부 cursor 변경으로 판정하고 vfunc_reset 직접 호출 → preedit commit + popup
        // cancel (IME 표준 동작).
        //
        // 위양성 방어:
        //   - preedit 없으면 skip (한글 조합 중이 아니면 어차피 reset 불요)
        //   - 최근 100ms 이내 키 입력 있으면 skip (글자 입력으로 인한 자연 cursor 이동)
        //   - 작은 이동(< 50px 직선 거리) skip (글자 단위 이동은 클릭이 아니라 typing)
        //   - 초기 cursor 보고(이전 좌표 0,0) skip (focus_in 직후 위양성 차단)
        const POST_KEY_GRACE_MS = 100;
        const JUMP_THRESHOLD_PX2 = 50 * 50;
        const prev = this._cursorRect;
        const initialReport = prev.x === 0 && prev.y === 0;
        const elapsedMs = this._lastKeyEventTimeMs > 0
            ? Date.now() - this._lastKeyEventTimeMs : Number.MAX_SAFE_INTEGER;
        if (this.hasPreedit() && !initialReport && elapsedMs > POST_KEY_GRACE_MS) {
            const dx = newRect.x - prev.x;
            const dy = newRect.y - prev.y;
            if (dx * dx + dy * dy > JUMP_THRESHOLD_PX2) {
                unimLog('IME',
                    `외부 cursor 점프 감지: prev=(${prev.x},${prev.y}) → new=(${newRect.x},${newRect.y}) ` +
                    `elapsed=${elapsedMs}ms — 로컬 flush 트리거`);
                this._cursorRect = newRect;   // flush 전 갱신 (재귀 방지)
                this._cancelPoke('reset');
                this._probeReset(true);
                try {
                    // 이 경로는 mutter 의 clutter_input_focus_reset() 을 거치지 않는다
                    // — 즉 COMMIT 모드가 발동하지 않으므로 vfunc_reset 에 맡길 수 없다.
                    // vfunc_reset 을 호출하지 않고 같은 일을 명시적으로 수행한다.
                    this._resetHandler?.();      // 팝업 정리
                    this._flushPreedit();        // 커밋 + preedit 클리어 (가드 무장 포함)
                    this._dbusIME?.reset();      // 엔진 상태 초기화
                } catch (e) {
                    unimError('IME', `cursor-jump flush 실패: ${e.message}`);
                }
                // reset 후 cursor 좌표는 새 위치로. daemon 통보도 새 좌표로.
                if (this._dbusIME) {
                    this._dbusIME.reportCursorRect(
                        Math.round(newRect.x), Math.round(newRect.y),
                        Math.round(newRect.width), Math.round(newRect.height)
                    );
                }
                return;
            }
        }

        this._cursorRect = newRect;
        if (this._dbusIME) {
            this._dbusIME.reportCursorRect(
                Math.round(newRect.x), Math.round(newRect.y),
                Math.round(newRect.width), Math.round(newRect.height)
            );
        }
    }

    vfunc_set_surrounding(text, cursor, anchor) {
        if (isUnimDevelop()) {
            const prev = this._lastSurrounding;
            unimProbe('set_surrounding',
                `gen=${this._facts.fieldGen} im_focus=${this._hasFocus ? 1 : 0} len=${Array.from(text || '').length} ` +
                `cursor_moved=${(cursor || 0) !== prev.cursor ? 1 : 0} sel=${(cursor || 0) !== (anchor || 0) ? 1 : 0} ` +
                `btn=${probePointerButtons()}`);
        }
        // 최신 surrounding text + cursor/anchor를 캐시 (gedit/gnome-text-editor 호환)
        this._lastSurrounding = { text: text || '', cursor: cursor || 0, anchor: anchor || 0 };
        // 재송신·확정 대조용 스냅샷(필드 세대 태그 포함). 비밀번호 칸이면 보관하지 않는다.
        this._facts = factsOnSurrounding(this._facts, this._lastSurrounding, {
            hasFocus: this._hasFocus,
            sensitive: this.isSensitivePurpose(),
            nowMs: GLib.get_monotonic_time() / 1000,
        });
        // Firefox 낡음 표시 뒤에는 왕복 응답만 넘긴다 — 한 박자 늦은 보고로 엔진이 재판정하지 않게.
        const pokeAwaitingReply = this._poke !== null && !this._poke.reported;
        const forward = shouldForwardSurrounding({
            holdUntilMs: this._ffHoldUntilMs,
            nowMs: GLib.get_monotonic_time() / 1000,
            pokeAwaitingReply,
        });
        if (forward && this._dbusIME?.isConnected) {
            this._dbusIME.setSurroundingText(text || '', cursor || 0, anchor || 0);
        }
        if (!forward) unimProbe('ff_hold_drop', `len=${Array.from(text || '').length}`);
        if (pokeAwaitingReply) this._ffHoldUntilMs = 0;
        // 빈 preedit 왕복 중이면 — 보고는 위에서 이미 데몬에 전달했다. 다음 주기에 null 로 되돌린다.
        this._onPokeReport();
    }

    /**
     * 마지막으로 받은 surrounding text + cursor/anchor 반환
     * TypeFix가 request_surrounding() 응답 후 호출
     * @returns {{text: string, cursor: number, anchor: number}}
     */
    getLastSurrounding() {
        return this._lastSurrounding;
    }

    /**
     * 확정 대조용 마지막 스냅샷(필드 세대 태그 포함). 없으면 null — 비밀번호 칸·포커스 아웃 직후.
     * @returns {import('./surrounding_facts.js').SurroundingCache|null}
     */
    getSurroundingCache() {
        return this._facts.cache;
    }

    /**
     * 엔진에 힌트를 보냈음을 기록한다 — KEY·포인터 힌트 뒤에는 스냅샷 재송신을 하지 않는다.
     * (KeyHandler._sendHint 가 부른다.)
     * @param {number} kind
     */
    noteHintSent(kind) {
        this._facts = factsOnHintSent(this._facts, kind);
    }

    /**
     * FIELD_FOCUS·창 FocusIn 을 보낸 직후 호출한다. 같은 필드 세대의 마지막 스냅샷이 있고 그 뒤
     * KEY·포인터 힌트·커밋이 없었으면 `SetSurroundingText` 로 한 번 더 보내 엔진의 낡음을 푼다.
     * 판정은 surrounding_facts.js `decideResend` (포커스 없음·비밀번호 칸·캐시 없음·세대 불일치·
     * 낡음·묵음이면 보내지 않는다). 비동기라 키 경로를 막지 않는다. 로그는 길이만.
     *
     * @param {string} trigger - 'field_focus' | 'win_focus_in'
     * @returns {boolean} 보냈는지
     */
    resendSurroundingFacts(trigger) {
        const d = decideResend(this._facts, {
            hasFocus: this._hasFocus,
            sensitive: this.isSensitivePurpose(),
            nowMs: GLib.get_monotonic_time() / 1000,
        });
        if (d.resend && !this._dbusIME?.isConnected) {
            d.resend = false;
            d.reason = 'disconnected';
        }
        unimProbe('resend', `trigger=${trigger} result=${d.resend ? 'sent' : 'skip'} reason=${d.reason}`);
        if (!d.resend) return false;
        const c = this._facts.cache;
        this._dbusIME.setSurroundingTextAsync(c.text, c.cursor, c.anchor);
        unimLog('IME', `surrounding 재송신: trigger=${trigger}, len=${Array.from(c.text).length}`);
        return true;
    }

    /**
     * 엔진이 소비하지 않은 키가 앱으로 간다(`notify_key_event(.., false)`·`return false`) — 그 키가
     * 문서·커서를 바꿀 수 있으니 마지막 스냅샷을 낡음(dirty)으로 한다. 소비된 키(한자키·팝업 키·선택 키)는
     * 부르지 않는다. 다음 `set_surrounding` 이 오면 해제된다.
     */
    noteKeyPassedToApp() {
        this._noteDocumentChanged();
    }

    /** 확장이 문서를 직접 바꿨다 — 마지막 스냅샷은 더는 현재 문서가 아니다. @private */
    _noteDocumentChanged() {
        this._facts = factsOnDocumentChanged(this._facts);
        this._lastDocChangeMs = Date.now();
        this._ffHoldUntilMs = 0;
    }

    vfunc_update_content_hints(hints) {
        this._contentHints = hints;
        unimLog('IME', `update_content_hints: clutter_hints=0x${(hints >>> 0).toString(16)}`);
        this._applyContentPurpose();
    }

    vfunc_update_content_purpose(purpose) {
        this._contentPurposeRaw = purpose;
        unimLog('IME', `update_content_purpose: clutter=${purpose}`);
        this._applyContentPurpose();
    }

    /**
     * CLUTTER_PURPOSE_TO_UNIM 매핑 + hidden_text 힌트 폴백을 적용한 최종
     * UNIM ContentPurpose 원시값을 계산한다.
     *
     * Clutter enum 에는 PIN 값이 없어(_effectivePurpose 매핑표 주석 참조)
     * password 로 접히지 않는 PIN 필드는 text-input-v3 의 hidden_text 힌트로만
     * 구분 가능한 경우가 있다 — purpose 가 Normal 로 떨어졌더라도 hidden_text 면
     * Password(1)로 승격한다(PIN 전용 값이 없으므로 Password 로 흡수 — 미차단보다
     * 과차단이 안전).
     * @returns {number} UNIM ContentPurpose 원시값
     * @private
     */
    _effectivePurpose() {
        const p = CLUTTER_PURPOSE_TO_UNIM.get(this._contentPurposeRaw) ?? 0;
        return (p === 0 && (this._contentHints & CLUTTER_HINT_HIDDEN_TEXT)) ? 1 : p;
    }

    /**
     * 비밀번호·PIN 성격의 입력란인지(fail-closed).
     *
     * PRIMARY 선택 보강(key_handler `_probePrimaryThenProcess`)처럼 **필드 내용과
     * 무관한 외부 데이터를 읽는 동작** 전에 반드시 확인한다. 계산값(effective)뿐 아니라
     * 데몬에 이미 보낸 값과 포커스 전 대기값까지 하나라도 Password(1)/PIN(2) 이면 true —
     * 순서 경합(purpose 가 포커스보다 늦게 오는 등)에서 놓치느니 과차단이 안전하다.
     * @returns {boolean}
     */
    isSensitivePurpose() {
        const sensitive = (p) => p === 1 || p === 2;
        return sensitive(this._effectivePurpose()) ||
               sensitive(this._sentPurpose) ||
               (this._pendingPurpose !== null && sensitive(this._pendingPurpose));
    }

    /**
     * 계산된 effective purpose 를 포커스 상태에 따라 즉시 송신하거나 대기시킨다.
     *
     * 포커스 없음: 다음 vfunc_focus_in 이 flush 할 수 있도록 _pendingPurpose 에 버퍼.
     * 포커스 있음: 직전 송신값과 다를 때만 handler 호출(dedupe) — mutter 가 커밋마다
     * update_content_purpose 를 반복 호출할 수 있어 call_sync 폭주를 막는다.
     * @private
     */
    _applyContentPurpose() {
        // focus_in 뒤 늦게 도착한 purpose 가 비밀번호·PIN 이면 이미 보관한 본문을 비운다.
        if (this.isSensitivePurpose()) this._facts = factsOnSensitive(this._facts);
        const eff = this._effectivePurpose();
        if (!this._hasFocus) {
            this._pendingPurpose = eff;
            this._pendingPurposeTime = GLib.get_monotonic_time();
            return;
        }
        if (eff !== this._sentPurpose) {
            this._contentTypeHandler?.(eff);
            this._sentPurpose = eff;
        }
    }

    // ===========================================
    // 공개 API — KeyHandler 등에서 호출
    // ===========================================

    /**
     * 최종 텍스트를 포커스된 앱에 커밋
     * @param {string} text - 커밋할 텍스트
     */
    commitText(text) {
        if (!text || text.length === 0) return;

        this._cancelPoke('commit');
        unimLog('IME', `commitText: len=${Array.from(String(text ?? '')).length}, _hasFocus=${this._hasFocus}, preeditLen=${Array.from(this._preeditText ?? '').length}`); // 평문 금지(HANJA_WORD_SPEC §2.8)

        // preedit이 남아있으면 먼저 클리어
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }

        this._noteDocumentChanged();
        try {
            this.commit(text);
            unimLog('IME', `commitText: commit() 호출 완료 (에러 없음)`);
        } catch (e) {
            unimError('IME', `텍스트 커밋 실패: ${e.message}`);
        }
    }

    /**
     * 조합 중 텍스트를 앱에 표시 (preedit)
     * @param {string} text - preedit 텍스트 (빈 문자열이면 클리어)
     */
    updatePreedit(text) {
        const t = text || '';
        if (this._poke) {
            if (!pokeCancelledByPreedit(t)) {
                // 빈 갱신(한자키 직후 _commitAndPreedit 의 updatePreedit(''))은 왕복을 건드리지 않는다.
                // 여기서 null 을 보내면 "" 와 null 이 같은 주기에 들어가 mutter 가 done 을 병합한다.
                // null 은 _endPoke 가 보고 도착(또는 타임아웃) 뒤 별도 주기에 보낸다.
                this._preeditText = '';
                return;
            }
            this._cancelPoke('preedit');   // 실제 preedit 이 "" 를 덮어쓴다 — null 을 따로 보내지 않는다
        }
        this._preeditText = t;

        try {
            if (this._preeditText === ZWSP) {
                // ZWSP 는 COMMIT 모드로 세우지 않는다 — mutter reset(클릭 등)이 ZWSP 를
                // 문서에 커밋하는 것을 막는다(v3.5). 확장은 더 이상 ZWSP 를 보내지 않으며
                // 이 분기는 방어용이다.
                this.set_preedit_text(
                    this._preeditText, 1, 1, Clutter.PreeditResetMode.CLEAR);
            } else if (this._preeditText.length > 0) {
                // reset 모드는 COMMIT — "리셋이 오면 이 preedit 을 커밋하라"고 mutter 에
                // 미리 선언해 두는 것이다(mutter clutter-input-focus.c):
                //
                //   clutter_input_focus_reset():
                //     if (priv->mode == COMMIT) clutter_input_focus_commit(focus, priv->preedit);
                //     clutter_input_focus_set_preedit_text(focus, NULL, 0, 0);
                //     clutter_input_method_reset(priv->im);   // ← 그제서야 vfunc_reset
                //
                // CLEAR 로 두면 mutter 가 preedit 을 먼저 파기하고, 그 뒤 vfunc_reset 에서
                // 우리가 commit 해봐야 **앵커가 사라진 뒤**라 앱의 현재 캐럿에 들어간다.
                // 웹뷰(Chrome 등)는 클릭 시 조합을 스스로 확정하지 않고 버린 채 reset 만
                // 보내므로, 조합 중 같은 필드 다른 위치를 클릭하면 글자가 클릭한 자리로
                // 옮겨가 버렸다. COMMIT 이면 mutter 가 clutter_input_focus_commit() 을
                // 먼저 호출하고, 이는 wayland 상에서 preedit_string(NULL)+commit_string 을
                // **한 done 배치**로 보낸다. text-input-v3 의 done 처리 순서가
                // "기존 preedit 을 커서로 치환 → commit 문자열 삽입" 이라 조합 중이던
                // 자리에 정확히 앵커된다.
                //
                // priv->mode 는 reset 때마다 CLEAR 로 되돌아가므로 preedit 을 세울 때마다
                // 매번 다시 선언해야 한다 — 이 호출이 그 역할이다.
                this.set_preedit_text(
                    this._preeditText,
                    this._preeditText.length,
                    this._preeditText.length,
                    Clutter.PreeditResetMode.COMMIT
                );
            } else {
                this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
            }
        } catch (e) {
            unimError('IME', `preedit 업데이트 실패: ${e.message}`);
        }
    }

    /**
     * AutoTypeFix가 vkbd로 BackSpace를 N번 보내기 직전에 호출.
     * 그 키 이벤트들이 mutter→IM filter로 재진입할 때 unim 엔진이 처리하지 않고
     * 그대로 포커스된 앱으로 통과시키도록 카운터를 설정한다.
     * @param {number} count - 보낼 backspace 개수
     */
    expectSelfBackspaces(count) {
        if (count <= 0) return;
        this._noteDocumentChanged();   // vkbd 백스페이스가 문서를 지운다
        // 한 backspace = PRESS + RELEASE 두 이벤트
        this._selfBackspaceCount += count * 2;
        unimLog('IME', `expectSelfBackspaces: +${count} (대기 이벤트=${this._selfBackspaceCount})`);
    }

    /**
     * 커서 앞의 텍스트를 삭제 (TypeFIX용)
     * @param {number} charCount - 삭제할 글자 수
     */
    deleteSurrounding(charCount) {
        if (charCount <= 0) return;
        this._noteDocumentChanged();
        try {
            this.delete_surrounding(-(charCount), charCount);
            unimLog('IME', `deleteSurrounding: ${charCount}자 삭제`);
        } catch (e) {
            unimError('IME', `deleteSurrounding 실패: ${e.message}`);
        }
    }

    /**
     * 커서 기준 앞 a자·뒤 b자를 지우고 text 를 확정한다 (대상③ 확정 —
     * ReplaceSurroundingText 시그널, HANJA_WORD_SPEC §2.4b.5).
     *
     * a·b 는 코드포인트(문자) 단위 — 구간이 전부 한글 완성 음절(BMP)이라 Mutter 의
     * delete_surrounding 단위와 같다. idle 이라 preedit 선클리어가 필요 없다.
     * Clutter 쪽은 삭제 실패를 알 수 없다(엔진 `c==0` 방어·거부 플래그가 막는다).
     * 로그는 a·b·길이만 남긴다(평문 금지).
     *
     * @param {number} deleteBefore - 커서 앞에서 지울 문자 수
     * @param {number} deleteAfter - 커서 뒤에서 지울 문자 수
     * @param {string} text - 확정할 문자열
     */
    replaceSurrounding(deleteBefore, deleteAfter, text) {
        if (!text || text.length === 0) return;
        this._noteDocumentChanged();
        try {
            this.delete_surrounding(-deleteBefore, deleteBefore + deleteAfter);
            unimLog('IME', `replaceSurrounding: a=${deleteBefore}, b=${deleteAfter}, len=${Array.from(text).length}`);
        } catch (e) {
            unimError('IME', `replaceSurrounding 삭제 실패, 커밋 생략: ${e.message}`);
            return;
        }
        this.commitText(text);
    }

    /**
     * Preedit 상태 초기화
     */
    clearPreedit() {
        this._preeditText = '';
        try {
            this.set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR);
        } catch (e) {
            unimError('IME', `preedit 초기화 실패: ${e.message}`);
        }
    }


    /**
     * IME 활성/비활성 토글
     * @param {boolean} active
     */
    setActive(active) {
        this._active = active;
        unimLog('IME', `IME ${active ? '활성화' : '비활성화'}`);
    }

    /**
     * IME 활성 상태 확인
     * @returns {boolean}
     */
    get isActive() {
        return this._active;
    }

    /**
     * 현재 커서 위치 (rect)
     * @returns {{x: number, y: number, width: number, height: number}}
     */
    get cursorRect() {
        return this._cursorRect;
    }
});
