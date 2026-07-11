/**
 * UNIM 전역 키 이벤트 핸들러
 *
 * Clutter Backend에 커스텀 InputMethod를 등록하여
 * Wayland 텍스트 입력 키를 가로채고,
 * DBus를 통해 UNIM 엔진으로 라우팅합니다.
 *
 * @module key_handler
 */

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import { unimLog, unimError } from './logging.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
/** 바이패스할 수정자 키들 */
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

/** 바이패스할 기능키 범위 (Ctrl/Alt 조합포함) */
const BYPASS_MODIFIER_MASK =
    Clutter.ModifierType.CONTROL_MASK |
    Clutter.ModifierType.MOD1_MASK |  // Alt
    Clutter.ModifierType.SUPER_MASK;

// ── 자동 영문(auto_english) 조합 트리거 selective divert ─────────────────────
// 기본적으로 Ctrl/Alt/Super 조합은 위 blanket 경로로 앱에 통과시킨다. 다만
// 사용자가 설정한 조합 트리거(예: `key:Ctrl+B` — tmux prefix)만은 engine 으로
// divert 해서 한→영 모드 전환을 일으켜야 한다(키 자체는 여전히 앱으로 통과).
//
// **핵심 제약**: divert 여부 판정은 engine(src/keycode/modifiers.rs::
// from_x11_mask, src/keycode/conversion.rs::from_evdev_keycode,
// src/input_engine/press_key.rs::match_auto_english_trigger)과 비트 단위로
// 동일해야 한다. 어긋나면 비트리거 조합(Ctrl+C 등)이 engine 으로 새고, 조합 중
// engine 이 committed()(consumed=true)로 키를 소비해 Ctrl+C 가 앱에 안 가는
// 회귀가 난다. 그래서 false-negative(트리거를 놓쳐 blanket 폴백=기능만 불발)는
// 허용하되 false-positive(비트리거를 divert)는 절대 만들지 않도록 보수적으로 짠다.

/** engine from_x11_mask 와 동일한 수정자 비트 (Clutter state 는 XKB real-mod 비트를 실음). */
const X11_SHIFT_MASK = 1 << 0;
const X11_CONTROL_MASK = 1 << 2;
const X11_MOD1_MASK = 1 << 3;   // Alt
const X11_MOD4_MASK = 1 << 6;   // Super

/** evdev keycode → KeyCode 이름 (KeyCode::from_evdev_keycode 의 비-수정자 항목 미러). */
const EVDEV_TO_KEYNAME = new Map([
    [30, 'A'], [48, 'B'], [46, 'C'], [32, 'D'], [18, 'E'], [33, 'F'], [34, 'G'],
    [35, 'H'], [23, 'I'], [36, 'J'], [37, 'K'], [38, 'L'], [50, 'M'], [49, 'N'],
    [24, 'O'], [25, 'P'], [16, 'Q'], [19, 'R'], [31, 'S'], [20, 'T'], [22, 'U'],
    [47, 'V'], [17, 'W'], [45, 'X'], [21, 'Y'], [44, 'Z'],
    [2, 'Num1'], [3, 'Num2'], [4, 'Num3'], [5, 'Num4'], [6, 'Num5'],
    [7, 'Num6'], [8, 'Num7'], [9, 'Num8'], [10, 'Num9'], [11, 'Num0'],
    [28, 'Enter'], [1, 'Escape'], [14, 'Backspace'], [15, 'Tab'], [57, 'Space'],
    [12, 'Minus'], [13, 'Equal'], [26, 'BracketLeft'], [27, 'BracketRight'],
    [43, 'Backslash'], [39, 'Semicolon'], [40, 'Quote'], [41, 'Backquote'],
    [51, 'Comma'], [52, 'Period'], [53, 'Slash'],
    [58, 'CapsLock'],
    [59, 'F1'], [60, 'F2'], [61, 'F3'], [62, 'F4'], [63, 'F5'], [64, 'F6'],
    [65, 'F7'], [66, 'F8'], [67, 'F9'], [68, 'F10'], [87, 'F11'], [88, 'F12'],
    [110, 'Insert'], [102, 'Home'], [104, 'PageUp'], [111, 'Delete'],
    [107, 'End'], [109, 'PageDown'],
    [106, 'Right'], [105, 'Left'], [108, 'Down'], [103, 'Up'],
    [122, 'Korean'], [123, 'Hanja'],
]);

/** runtime 도달 가능한 KeyCode 이름 집합 (base 유효성 검증용). */
const VALID_KEY_NAMES = new Set(EVDEV_TO_KEYNAME.values());

/** parse_functional_name 의 shift_sensitive 목록 미러 (등장 시 shift 없어야 매칭). */
const SHIFT_SENSITIVE_NAMES = new Set([
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    'Num0', 'Num1', 'Num2', 'Num3', 'Num4', 'Num5', 'Num6', 'Num7', 'Num8', 'Num9',
    'Minus', 'Equal', 'BracketLeft', 'BracketRight', 'Backslash', 'Semicolon',
    'Quote', 'Backquote', 'Comma', 'Period', 'Slash', 'Space',
]);

/**
 * auto_english 조합 트리거 표기 1건을 파싱한다 (press_key.rs::parse_keyspec 미러).
 *
 * Ctrl/Alt/Super 중 최소 1개를 요구하는 **조합** 트리거만 반환한다. `char:`·legacy·
 * 단일 `key:` 표기는 조합이 아니므로(수정자 없이 입력돼 GNOME 이 이미 정상 경로로
 * 처리) null 을 돌려준다 — divert 대상이 아니다.
 *
 * @param {string} spec - 예: "key:Ctrl+B", "key:Ctrl+Shift+B", "key:Alt+F1"
 * @returns {{name:string, ctrl:boolean, alt:boolean, superKey:boolean, shift:(boolean|null)}|null}
 */
function parseComboTrigger(spec) {
    if (typeof spec !== 'string') return null;
    if (!spec.startsWith('key:')) return null;   // char:/legacy 는 조합 문법 아님
    const body = spec.slice(4);
    if (!body.includes('+')) return null;        // 단일 base — 비조합 (engine 정상 경로)
    const tokens = body.split('+').map((t) => t.trim());
    let base = tokens.pop();                      // 마지막 토큰 = base
    let ctrl = false;
    let alt = false;
    let superKey = false;
    let shiftForced = false;
    for (const tok of tokens) {
        const t = tok.toLowerCase();
        if (t === 'ctrl' || t === 'control') ctrl = true;
        else if (t === 'alt') alt = true;
        else if (t === 'super' || t === 'win' || t === 'meta') superKey = true;
        else if (t === 'shift') shiftForced = true;
        else return null;                        // 미지 modifier 토큰 → 침묵 무시
    }
    if (!ctrl && !alt && !superKey) return null; // 순수 shift 조합은 bypass 대상 아님
    // base 는 대소문자 구분(KeyCode::from_name). "Shift<Name>" 융합형 지원.
    let baseShiftForced = false;
    if (base.startsWith('Shift')) {
        base = base.slice(5);
        baseShiftForced = true;
    }
    if (!VALID_KEY_NAMES.has(base)) return null; // 빈/미지 base → 침묵 무시
    let shift;
    if (shiftForced || baseShiftForced) shift = true;               // shift 필수
    else shift = SHIFT_SENSITIVE_NAMES.has(base) ? false : null;    // 문자키=없어야, 제어키=무관
    return { name: base, ctrl, alt, superKey, shift };
}




/**
 * KeyHandler
 *
 * 전역 키 이벤트를 인터셉트하여 UNIM 엔진으로 전달합니다.
 */
export class KeyHandler {
    /**
     * @param {import('./dbus_ime.js').UnimDbusIME} dbusIME - DBus 클라이언트
     * @param {import('./unim_input_method.js').UnimInputMethod} inputMethod - IM 서브클래스
     * @param {object} options
     * @param {Function} [options.onHanjaRequest] - 한자키 콜백
     * @param {number[]} [options.hanjaKeysyms] - 한자키 keysym 목록
     */
    constructor(dbusIME, inputMethod, options = {}) {
        /** @type {import('./dbus_ime.js').UnimDbusIME} */
        this._dbusIME = dbusIME;
        /** @type {import('./unim_input_method.js').UnimInputMethod} */
        this._inputMethod = inputMethod;

        /** @type {Function|null} 한자키 콜백 */
        this._onHanjaRequest = options.onHanjaRequest || null;

        /** @type {Set<number>} 한자키 keysym 집합 */
        this._hanjaKeysyms = new Set(options.hanjaKeysyms || [
            Clutter.KEY_Hangul_Hanja,  // 한자 전용 키
            Clutter.KEY_F9,            // F9 (기본)
        ]);

        /** @type {number} captured-event 시그널 ID */
        this._keyPressId = 0;
        /** @type {boolean} IME 활성 상태 */
        this._enabled = false;

        /** @type {Clutter.InputMethod|null} 원래 IM (복원용) */
        this._savedInputMethod = null;
        /** @type {boolean} Backend 등록 여부 */
        this._backendRegistered = false;
        /** @type {boolean} 키 처리 중 재진입 방지 플래그 */
        this._processingKey = false;
        /** @type {Array<{keyval: number, keycode: number, state: number, event: Clutter.Event}>} 재진입 키 큐 */
        this._keyQueue = [];
    }

    /**
     * 전역 키 인터셉션 시작
     *
     * 전략:
     * A) UnimInputMethod (Clutter.InputMethod 서브클래스)를 Backend에 등록
     *    → vfunc_filter_key_event가 C vtable에 올바르게 바인딩됨
     *    → Mutter가 모든 Wayland 텍스트 입력 키를 우리 vfunc으로 전달
     * B) global.stage captured-event (폴백: 셸 UI 등)
     */
    enable() {
        if (this._enabled) return;

        // 1. Backend에 커스텀 IM 등록 (Wayland 텍스트 입력 가로채기)
        this._registerWithBackend();

        // 2. 폴백 (Backend 미등록 시 또는 Shell UI용)
        this._keyPressId = global.stage.connect(
            'captured-event',
            (actor, event) => {
                if (event.type() !== Clutter.EventType.KEY_PRESS) {
                    return Clutter.EVENT_PROPAGATE;
                }
                // Backend 등록 완료 시 vfunc이 이미 처리하므로 스킵
                // (이중 입력 방지)
                if (this._backendRegistered) {
                    return Clutter.EVENT_PROPAGATE;
                }
                return this._onKeyPress(actor, event);
            }
        );

        this._enabled = true;
        unimLog('KEY', `전역 키 인터셉션 시작 완료 (backend-im=${this._backendRegistered ? '등록됨' : '미등록'}, captured-event=활성)`);
    }

    /**
     * 커스텀 UnimInputMethod를 Clutter Backend에 등록
     * API: clutter_backend_set_input_method(backend, method)
     *
     * GObject.registerClass()로 등록된 서브클래스의 vfunc은
     * C vtable에 올바르게 등록되어, Mutter C 코드에서 호출됩니다.
     * @private
     */
    _registerWithBackend() {
        try {
            const backend = Clutter.get_default_backend();

            if (typeof backend.set_input_method !== 'function') {
                unimLog('KEY', '⚠️ backend.set_input_method가 함수가 아닙니다.');
                return;
            }

            // 키 핸들러 콜백 등록 (vfunc_filter_key_event에서 호출됨)
            // event를 전달받아 직접 notify_key_event를 호출 (비동기 키 큐 지원)
            this._inputMethod.setKeyHandler((keyval, keycode, state, event) => {
                this._handleVfuncKey(keyval, keycode, state, event);
            });

            // 현재 IM 저장 (disable 시 복원용)
            this._savedInputMethod = Main.inputMethod;

            // Backend에 우리 IM 등록
            backend.set_input_method(this._inputMethod);
            this._backendRegistered = true;

            unimLog('KEY', 'UnimInputMethod를 Clutter Backend에 등록 완료');
        } catch (e) {
            unimError('KEY', `Backend 등록 실패: ${e.message}`);
        }
    }

    /**
     * vfunc_filter_key_event에서 호출되는 키 처리 콜백
     *
     * 키 핸들러가 직접 notify_key_event를 호출합니다.
     * call_sync() 중 GLib 재진입으로 도착한 키는 큐에 저장 후 순차 처리합니다.
     *
     * @param {number} keyval - X11 keysym
     * @param {number} keycode - evdev keycode
     * @param {number} state - modifier state
     * @param {Clutter.Event} event - 원본 키 이벤트 (notify_key_event용)
     * @private
     */
    _handleVfuncKey(keyval, keycode, state, event) {
        // 0. 재진입 방지: call_sync() 중 GLib 메인 루프가 이벤트를 처리하여
        //    동일 키가 재진입될 수 있음 → 큐에 저장하여 순차 처리 (키 누락 방지)
        if (this._processingKey) {
            try {
                this._keyQueue.push({keyval, keycode, state, event: event.copy()});
            } catch (e) {
                // event.copy() 실패 시 기존 동작 유지 (키 소비)
                unimError('KEY', `이벤트 복사 실패, 키 소비: ${e.message}`);
                this._inputMethod.notify_key_event(event, true);
            }
            return;
        }

        // 1. 수정자 키 단독 → 미소비
        if (MODIFIER_KEYSYMS.has(keyval)) {
            this._inputMethod.notify_key_event(event, false);
            return;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through
        //    engine이 팝업 키를 처리하고 시그널(PopupNavigate, HidePopup)로 UI 갱신

        // 3. Ctrl/Alt/Super 조합 → 키를 먼저 전달한 후 조합 flush
        //    (고정키 사용 시 _flushCompose의 call_sync 중 modifier가 해제되는 것을 방지)
        if (state & BYPASS_MODIFIER_MASK) {
            // 3a. auto_english 조합 트리거(예: `key:Ctrl+B`)면 키를 먼저 앱으로 전달
            //     (고정키 순서 보존)한 뒤 engine 으로 divert 해 한→영 전환.
            if (this._matchesAutoEnglishCombo(keycode, state)) {
                this._inputMethod.notify_key_event(event, false);
                this._divertComboToEngine(keyval, keycode, state);
                this._drainKeyQueue();
                return;
            }
            // 3b. 비트리거 조합 → 기존 동작(전달 후 조합 flush).
            this._inputMethod.notify_key_event(event, false);
            this._flushCompose();
            return;
        }

        // 4. 한자키는 엔진에 위임 (ProcessKeyEvent를 통해 처리)

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            this._inputMethod.notify_key_event(event, false);
            return;
        }

        // 6. ProcessKeyEvent 호출 (재진입 가드)
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, keycode, state);
        } finally {
            this._processingKey = false;
        }

        if (!result) {
            this._inputMethod.notify_key_event(event, false);
            this._drainKeyQueue();
            return;
        }

        // 7. 결과 처리
        const { consumed, preedit, commit } = result;

        // commit + preedit 동시 송출 시 mutter Clutter race 회피:
        //   commit() 직후 같은 frame에서 set_preedit_text()를 호출하면
        //   특히 commit과 preedit이 같은 글자(예: ㄹ→ㄹ 분리)일 때
        //   mutter가 갱신을 dedup/유실해 사용자에게 보이지 않는다.
        //   commit이 있을 때만 preedit을 다음 main loop iteration으로 분리.
        this._commitAndPreedit(commit, preedit);
        this._inputMethod.notify_key_event(event, consumed);

        // 8. 재진입으로 큐에 쌓인 키 순차 처리
        this._drainKeyQueue();
    }

    /**
     * commit + preedit 송출 (race 회피).
     *
     * commit이 비어 있으면 기존처럼 즉시 preedit을 갱신해 latency를 보존.
     * commit이 존재하면 commitText만 즉시 호출하고, preedit은 다음
     * main loop iteration으로 분리해 mutter의 set_preedit_text dedup race를
     *회피한다.
     *
     * NOTE: AutoTypeFix(extension.js:onAutoTypeFix)는 PRIORITY_HIGH 50/10ms
     * 타임아웃을 사용한다. 본 헬퍼는 PRIORITY_DEFAULT 0ms를 사용해 자연스러운
     * 큐 순서(AutoTypeFix가 동시에 발화해도 먼저 처리)를 유지한다.
     *
     * @param {string} commit - commit 텍스트 (없으면 빈 문자열/null)
     * @param {string} preedit - preedit 텍스트 (없으면 빈 문자열/null)
     * @private
     */
    _commitAndPreedit(commit, preedit) {
        const hasCommit = commit && commit.length > 0;
        if (hasCommit) {
            this._inputMethod.commitText(commit);
            const im = this._inputMethod;
            const text = preedit || '';
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 0, () => {
                if (im && typeof im.updatePreedit === 'function') {
                    im.updatePreedit(text);
                }
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._inputMethod.updatePreedit(preedit || '');
        }
    }

    /**
     * 재진입으로 큐에 쌓인 키를 순차 처리
     *
     * call_sync() 중 GLib 메인 루프 재진입으로 도착한 키를
     * FIFO 순서로 처리하여 키 누락을 방지합니다.
     * @private
     */
    _drainKeyQueue() {
        while (this._keyQueue.length > 0) {
            const entry = this._keyQueue.shift();

            if (MODIFIER_KEYSYMS.has(entry.keyval)) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            if (entry.state & BYPASS_MODIFIER_MASK) {
                // auto_english 조합 트리거면 divert (키는 앱으로 통과).
                if (this._matchesAutoEnglishCombo(entry.keycode, entry.state)) {
                    this._inputMethod.notify_key_event(entry.event, false);
                    this._divertComboToEngine(entry.keyval, entry.keycode, entry.state);
                    continue;
                }
                this._flushCompose();
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            if (!this._dbusIME.isConnected) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            this._processingKey = true;
            let result;
            try {
                result = this._dbusIME.processKey(entry.keyval, entry.keycode, entry.state);
            } finally {
                this._processingKey = false;
            }

            if (!result) {
                this._inputMethod.notify_key_event(entry.event, false);
                continue;
            }

            const { consumed, preedit, commit } = result;
            // _handleVfuncKey와 동일한 race 회피 (commit→preedit 분리)
            this._commitAndPreedit(commit, preedit);
            this._inputMethod.notify_key_event(entry.event, consumed);
        }
    }

    /**
     * 조합 중인 텍스트 flush (Ctrl/Alt 조합 시)
     *
     * 로컬 preedit을 커밋하고 엔진 상태를 초기화합니다.
     * notify_key_event 패턴에서는 키 전달을 Mutter가 처리하므로
     * forward_key 호출이 불필요합니다.
     * @private
     */
    _flushCompose() {
        // 로컬 preedit에서 커밋 텍스트 획득
        const preedit = this._inputMethod._preeditText || '';
        if (preedit.length > 0) {
            this._inputMethod.clearPreedit();
            this._inputMethod.commitText(preedit);
        }
        // 엔진 상태 초기화 (포커스 변경 없이)
        if (this._dbusIME?.isConnected) {
            this._dbusIME.reset();
        }
    }

    /**
     * 현재 (evdevKeycode, state) 가 설정된 auto_english **조합** 트리거와 정확히
     * 일치하는지 판정한다 — engine match_auto_english_trigger 의 조합 분기 미러.
     *
     * true → 이 조합 키를 blanket flush 대신 engine 으로 divert(모드 전환) 해야 함.
     * false → 기존 blanket 동작 유지(Ctrl+C 등 회귀 없음).
     *
     * 캐시된 Config(dbus_ime GetConfigJson/ConfigChangedJson 스냅샷)를 매 조합 키마다
     * 읽어 파싱한다. trigger_keys 는 소수 항목이고 조합 키는 드물어 비용은 무시 가능,
     * 대신 캐시 무효화 버그가 원천 차단된다. 파싱/접근 오류는 false 로 흡수해 키 hot
     * path 안정성을 지킨다(예외 시 호출부가 blanket 폴백 실행).
     *
     * @param {number} evdevKeycode
     * @param {number} state - Clutter modifier 비트필드
     * @returns {boolean}
     * @private
     */
    _matchesAutoEnglishCombo(evdevKeycode, state) {
        try {
            const cfg = this._dbusIME ? this._dbusIME.getCachedConfig() : null;
            const ae = cfg && cfg.engine ? cfg.engine.auto_english : null;
            if (!ae || ae.enabled !== true || !Array.isArray(ae.trigger_keys)) {
                return false;
            }
            const name = EVDEV_TO_KEYNAME.get(evdevKeycode);
            if (!name) return false;
            const ctrl = (state & X11_CONTROL_MASK) !== 0;
            const alt = (state & X11_MOD1_MASK) !== 0;
            const superKey = (state & X11_MOD4_MASK) !== 0;
            const shift = (state & X11_SHIFT_MASK) !== 0;
            // 엔진 관점(from_x11_mask)의 ctrl/alt/super 가 하나도 없으면 조합 트리거
            // 매칭 자체가 불가(engine 가드 `control||alt||super` 미진입) → divert 불요.
            if (!ctrl && !alt && !superKey) return false;
            for (const spec of ae.trigger_keys) {
                const t = parseComboTrigger(spec);
                if (!t) continue;
                if (t.name === name
                    && t.ctrl === ctrl
                    && t.alt === alt
                    && t.superKey === superKey
                    && (t.shift === null || t.shift === shift)) {
                    return true;
                }
            }
            return false;
        } catch (e) {
            unimError('KEY', `auto_english 조합 매칭 오류(무시): ${e.message}`);
            return false;
        }
    }

    /**
     * 조합 트리거를 engine 으로 divert 해 한→영 모드 전환을 수행하고 결과(commit/
     * preedit)를 반영한다. 키는 **소비하지 않는다** — 호출부가 이미 notify_key_event
     * (false)/EVENT_PROPAGATE 로 앱에 전달했다(tmux prefix Ctrl+B 등이 앱행이어야 함).
     * ProcessKeyEvent 는 조합 트리거에 대해 consumed=false(idle=not_consumed,
     * 조합중=committed_passthrough)만 돌려주므로 consumed 는 무시한다. D-Bus 실패
     * 시 기존 로컬 flush 로 폴백.
     *
     * @param {number} keyval
     * @param {number} evdevKeycode
     * @param {number} state
     * @private
     */
    _divertComboToEngine(keyval, evdevKeycode, state) {
        if (!this._dbusIME.isConnected) {
            this._flushCompose();
            return;
        }
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, evdevKeycode, state);
        } finally {
            this._processingKey = false;
        }
        if (result) {
            // commit(직전 조합 flush)/preedit(빈 문자열) 만 반영 — 일반 키 경로와 동일.
            this._commitAndPreedit(result.commit, result.preedit);
        } else {
            // D-Bus 실패 → 로컬 preedit flush + engine reset (기존 blanket 폴백).
            this._flushCompose();
        }
    }

    /**
     * Backend에서 원래 IM 복원
     * @private
     */
    _unregisterFromBackend() {
        if (this._backendRegistered) {
            try {
                const backend = Clutter.get_default_backend();
                if (this._savedInputMethod) {
                    backend.set_input_method(this._savedInputMethod);
                    unimLog('KEY', '원본 InputMethod 복원 완료');
                } else {
                    backend.set_input_method(null);
                }
                this._inputMethod.setKeyHandler(null);
            } catch (e) {
                unimError('KEY', `Backend 해제 중 오류: ${e.message}`);
            }
        }
        this._savedInputMethod = null;
        this._backendRegistered = false;
    }

    /**
     * 전역 키 인터셉션 중지
     */
    disable() {
        // captured-event 해제
        if (this._keyPressId > 0) {
            global.stage.disconnect(this._keyPressId);
            this._keyPressId = 0;
        }

        // Backend에서 원래 IM 복원
        this._unregisterFromBackend();

        this._enabled = false;
        unimLog('KEY', '전역 키 인터셉션 중지');
    }

    /**
     * 한자키 keysym 목록 갱신
     * @param {number[]} keysyms
     */
    updateHanjaKeysyms(keysyms) {
        this._hanjaKeysyms = new Set(keysyms);
    }

    // ===========================================
    // 내부 — 키 이벤트 처리
    // ===========================================

    /**
     * key-press-event 핸들러
     *
     * @param {Clutter.Actor} actor
     * @param {Clutter.Event} event
     * @returns {number} Clutter.EVENT_STOP 또는 Clutter.EVENT_PROPAGATE
     * @private
     */
    _onKeyPress(actor, event) {
        try {
            return this._handleKeyPress(event);
        } catch (e) {
            // 안전장치: 에러 시 키 이벤트를 앱에 전달
            unimError('KEY', `키 처리 오류: ${e.message}`);
            return Clutter.EVENT_PROPAGATE;
        }
    }

    /**
     * 실제 키 처리 로직 (에러 핸들링 분리)
     *
     * @param {Clutter.Event} event
     * @returns {number}
     * @private
     */
    _handleKeyPress(event) {
        // 0. 재진입 방지 (call_sync 중 GLib 메인 루프 재진입 차단)
        if (this._processingKey) {
            return Clutter.EVENT_STOP;
        }

        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();

        const evdevKeycode = keycode > 8 ? keycode - 8 : 0;

        // 1. 수정자 키 단독 입력 → 바이패스
        if (MODIFIER_KEYSYMS.has(keyval)) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through (이중 처리 방지)

        // 3. Ctrl/Alt/Super 조합 → 조합 중이면 커밋 후 바이패스
        if (state & BYPASS_MODIFIER_MASK) {
            // 3a. auto_english 조합 트리거면 engine 으로 divert(모드 전환). 키는
            //     PROPAGATE 로 앱에 전달되므로 별도 forward 불필요.
            if (this._matchesAutoEnglishCombo(evdevKeycode, state)) {
                this._divertComboToEngine(keyval, evdevKeycode, state);
                return Clutter.EVENT_PROPAGATE;
            }
            // 3b. captured-event 폴백이므로 forward_key 불필요, PROPAGATE로 키 전달
            const preedit = this._inputMethod._preeditText || '';
            if (preedit.length > 0) {
                this._inputMethod.clearPreedit();
                this._inputMethod.commitText(preedit);
                if (this._dbusIME?.isConnected) this._dbusIME.reset();
            }
            return Clutter.EVENT_PROPAGATE;
        }

        // 4. 한자키는 엔진에 위임 (ProcessKeyEvent를 통해 처리)
        //    엔진이 ShowHanjaPopup 시그널을 발행하면 인디케이터가 팝업 표시

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 6. ProcessKeyEvent 호출 (재진입 가드)
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, evdevKeycode, state);
        } finally {
            this._processingKey = false;
        }

        if (!result) {
            return Clutter.EVENT_PROPAGATE;
        }

        // 7. 결과 처리
        const { consumed, preedit, commit } = result;

        // commit + preedit 동시 송출 시 mutter race 회피 (vfunc 경로와 동일)
        this._commitAndPreedit(commit, preedit);

        // consumed=true면 키를 소비, false면 앱에 전달
        return consumed ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
    }

    /**
     * 자원 정리
     */
    destroy() {
        this.disable();
        this._keyQueue = [];
        this._dbusIME = null;
        this._inputMethod = null;
        this._onHanjaRequest = null;
    }
}
