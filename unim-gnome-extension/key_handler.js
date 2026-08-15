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

/* repeat 태깅 — src/keycode/modifiers.rs 의 UNIM_* 상수와 값 동기.
 * JS 비트연산은 32bit signed — (1<<31) 금지, 리터럴 + >>>0 을 사용한다. */
const UNIM_KEY_REPEAT_MASK = 0x20000000;
const UNIM_REPEAT_AWARE_MASK = 0x80000000;
const CLUTTER_FLAG_REPEATED = Clutter.EventFlags?.REPEATED ?? (1 << 2);

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
 * ATF 토글 단축키 표기 1건을 파싱한다 (press_key.rs::parse_atf_hotkey 미러).
 *
 * parseComboTrigger(auto_english)와 문법이 다르다:
 * - `key:` 접두사 없음 (raw 설정 문자열)
 * - `"Shift<Name>"` 융합형 미지원 — shift 는 `Shift+` 토큰으로만
 * - 네 수정자 전부 정확 일치(등장=필수, 미등장=금지) — shift 3태 없음
 * - 구분자 `+` 우선, `+` 가 전혀 없으면 `-` 폴백 (engine 과 동일)
 *
 * Ctrl/Alt/Super 를 포함한 조합만 반환한다 — bare·Shift 단독 조합은 blanket
 * bypass 대상이 아니어서 divert 가 필요 없다(정상 경로가 이미 engine 에 전달).
 *
 * @param {string} spec - 예: "Ctrl+Left", "Ctrl-Left", "Alt+F5"
 * @returns {{name:string, ctrl:boolean, alt:boolean, superKey:boolean, shift:boolean}|null}
 */
function parseAtfComboHotkey(spec) {
    if (typeof spec !== 'string') return null;
    const sep = spec.includes('+') ? '+' : '-';
    const tokens = spec.split(sep).map((t) => t.trim());
    const base = tokens.pop();
    let ctrl = false;
    let alt = false;
    let superKey = false;
    let shift = false;
    for (const tok of tokens) {
        const t = tok.toLowerCase();
        if (t === 'ctrl' || t === 'control') ctrl = true;
        else if (t === 'alt') alt = true;
        else if (t === 'super' || t === 'win' || t === 'meta') superKey = true;
        else if (t === 'shift') shift = true;
        else return null;                        // 미지 modifier 토큰
    }
    if (!ctrl && !alt && !superKey) return null; // bypass 비대상 — divert 불요
    if (!VALID_KEY_NAMES.has(base)) return null; // 빈/미지/수정자 base
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
            // 반환값을 **그대로 흘려보내야** 한다 — false 면 vfunc 이 이벤트를
            // 가로채지 않는다(고정키 보존). 중괄호 본문으로 감싸면 undefined 가
            // 돌아가 이 경로가 통째로 무력화되므로 표현식 형태를 유지할 것.
            this._inputMethod.setKeyHandler((keyval, keycode, state, event) =>
                this._handleVfuncKey(keyval, keycode, state, event));

            // bare Alt_R(토글 후보) 비소비 통지 — vfunc 경로에서 return false 로
            // Mutter 네이티브 처리를 보존하되 데몬에 fire-and-forget 으로 통지한다.
            this._inputMethod.setToggleKeyNotifier((keyval, keycode, state) => {
                if (this._dbusIME?.isConnected)
                    this._dbusIME.processKeyAsync(keyval, keycode, state >>> 0);
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
     * call_sync() 중 GLib 재진입으로 도착한 키는 큐에 저장 후 순차 처리합니다.
     *
     * **반환값 규약** — 이 값이 vfunc_filter_key_event 의 반환값이 된다:
     *  - `false`: 이 키를 가로채지 않는다. notify_key_event 를 부르면 **안 된다**
     *    (mutter 가 직접 전달하므로 중복이 된다). 소비하지 않을 키는 전부 이쪽으로
     *    보내야 고정키 래치가 살아남는다 — 근거는 unim_input_method.js 주석.
     *  - 그 외(undefined 포함): 가로챘다. 키 전달 여부는 이 함수가 이미 호출한
     *    notify_key_event(event, consumed) 가 정한다.
     *
     * @param {number} keyval - X11 keysym
     * @param {number} keycode - evdev keycode
     * @param {number} state - modifier state
     * @param {Clutter.Event} event - 원본 키 이벤트 (notify_key_event용)
     * @returns {boolean|undefined} false 면 미가로챔
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

        // 1. 수정자 키 단독 → 가로채지 않음 (vfunc 이 이미 걸러내므로 방어적 분기)
        if (MODIFIER_KEYSYMS.has(keyval)) {
            return false;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through
        //    engine이 팝업 키를 처리하고 시그널(PopupNavigate, HidePopup)로 UI 갱신

        // 3. Ctrl/Alt/Super 조합 → **가로채지 않는다**(return false).
        //    소비할 생각이 없는 키를 notify_key_event 로 되돌려 보내면 mutter 가
        //    고정키 래치를 다시 입히는 지점을 건너뛰게 된다 — 자세한 근거는
        //    unim_input_method.js vfunc_filter_key_event 의 "왜 false 가 중요한가".
        //    예외: ATF 토글 조합(예: Ctrl+Left)은 bypass 를 건너뛰고 6번 일반
        //    ProcessKeyEvent 경로로 — engine 정확-일치(atf_hotkey_kind)가 소비를
        //    판정하고, 불일치면 consumed=false 로 종전처럼 앱에 전달된다.
        if ((state & BYPASS_MODIFIER_MASK) && !this._matchesAtfHotkey(keycode, state)) {
            const committed = this._bypassCombo(keyval, keycode, this._tagRepeatBits(state, event),
                                                this._matchesAutoEnglishCombo(keycode, state));
            if (committed) {
                // 조합을 확정했다면 키를 한 회차 미룬다 — 6번의 hasCommit 분기와
                // 같은 이유(커밋이 앱에 먼저 적용돼야 한다). 조합 중이 아닐 때는
                // 여기 안 걸리므로 Ctrl+A·Shift+Home 등 일상 조합은 그대로
                // return false 로 빠져 고정키 래치가 살아 있다.
                this._inputMethod.notify_key_event(event, false);
                this._drainKeyQueue();
                return;
            }
            return false;
        }

        // 4. 한자키는 엔진에 위임 (ProcessKeyEvent를 통해 처리)

        // 5. DBus 연결 확인
        if (!this._dbusIME.isConnected) {
            return false;
        }

        // 6. ProcessKeyEvent 호출 (재진입 가드)
        this._processingKey = true;
        let result;
        try {
            result = this._dbusIME.processKey(keyval, keycode, this._tagRepeatBits(state, event));
        } finally {
            this._processingKey = false;
        }

        if (!result) {
            // D-Bus 실패 → 소비하지 않는다. 사본을 만드는 대신 mutter 에게 그대로
            // 넘겨야 고정키 래치가 살아 있다(3번 주석과 같은 이유).
            this._drainKeyQueue();
            return false;
        }

        // 7. 결과 처리
        const { consumed, preedit, commit } = result;

        // commit + preedit 동시 송출 시 mutter Clutter race 회피:
        //   commit() 직후 같은 frame에서 set_preedit_text()를 호출하면
        //   특히 commit과 preedit이 같은 글자(예: ㄹ→ㄹ 분리)일 때
        //   mutter가 갱신을 dedup/유실해 사용자에게 보이지 않는다.
        //   commit이 있을 때만 preedit을 다음 main loop iteration으로 분리.
        const hasCommit = !!(commit && commit.length > 0);
        this._commitAndPreedit(commit, preedit);

        if (!consumed && !hasCommit) {
            // 소비도 커밋도 없는 키는 사본을 만들지 않는다 — 3번 주석과 같은 이유.
            // **Shift 고정키가 이 경로를 탄다**: Shift 는 BYPASS_MODIFIER_MASK 에
            // 없어 3번으로 안 빠지므로, Shift+Home·Shift+방향키(선택) 같은 조합이
            // 여기까지 내려온다. 사본으로 미루면 래치가 먼저 풀려 선택이 안 된다.
            this._drainKeyQueue();
            return false;
        }
        // 커밋이 딸린 미소비 키(조합 중 Enter·Home·End 등)는 **반드시 미룬다.**
        // return false 로 넘기면 커밋과 키가 같은 dispatch 에 묶여 나가는데,
        // 앱이 그 둘을 같은 순서로 적용한다는 보장이 없다 — 렌더러가 별도
        // 프로세스인 Chrome 계열은 키를 먼저 처리해 "Enter 먼저, 글자 나중"이
        // 된다(2026-08-15 실측 회귀). notify_key_event 로 한 회차 미루면 그 사이
        // 커밋의 done 아이들이 먼저 발사돼 순서가 지켜진다.
        // 대신 이 키에서는 고정키 래치를 보장하지 못한다 — 조합 중 Shift+Home
        // 같은 드문 조합이 해당된다. 글자 순서가 어긋나는 쪽이 훨씬 나쁘다.
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

            if ((entry.state & BYPASS_MODIFIER_MASK)
                && !this._matchesAtfHotkey(entry.keycode, entry.state)) {
                // 큐에 들어온 시점에 이미 vfunc 스택을 벗어났으므로 여기서는
                // return false 를 쓸 수 없다 — notify_key_event 로 되돌려 보낸다.
                // 뒷정리는 hot path 와 같은 비동기 경로를 쓴다.
                this._bypassCombo(entry.keyval, entry.keycode,
                                  this._tagRepeatBits(entry.state, entry.event),
                                  this._matchesAutoEnglishCombo(entry.keycode, entry.state));
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
                result = this._dbusIME.processKey(entry.keyval, entry.keycode, this._tagRepeatBits(entry.state, entry.event));
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
     * Ctrl/Alt/Super 조합 키를 앱에 그대로 넘기기 직전에 할 뒷정리.
     *
     * **동기 D-Bus 호출을 하지 않는다.** 이 함수가 도는 동안 셸 메인 스레드가
     * 멈추면, 그 틈에 mutter 의 고정키 래치 해제 콜백이 먼저 실행돼 뒤이어
     * 전달되는 키에서 Ctrl/Alt 가 빠진다. 래치 해제는 mutter 입력 스레드가
     * 물리적 키 뗌 시점에 곧바로 통지하므로 이쪽이 늦으면 그대로 진다.
     * (근거: unim_input_method.js vfunc_filter_key_event 주석)
     *
     * 그래서 두 가지로 나눈다:
     *  1. 조합 중이던 글자 확정 — D-Bus 를 타지 않는 **로컬** 커밋이라 즉시 끝난다.
     *     mutter 가 이어서 보낼 키보다 먼저 앱에 닿아야 하므로 여기서 동기로 한다.
     *  2. 엔진 통지 — 전부 비동기. 응답이 필요 없고, D-Bus 가 연결 단위로 메시지
     *     순서를 보존하므로 다음 키의 ProcessKeyEvent 보다 먼저 도달한다.
     *
     * 데몬은 Reset·조합 트리거 처리 중 비운 조합을 CommitText 로 되쏘는데, 위 1
     * 에서 이미 같은 글자를 커밋했으므로 메아리 가드를 무장해 흘려보낸다.
     *
     * @param {number} keyval
     * @param {number} evdevKeycode
     * @param {number} taggedState - _tagRepeatBits 를 거친 state
     * @param {boolean} isAutoEnglishTrigger - auto_english 조합 트리거 여부
     * @returns {boolean} 조합을 확정했는지 — true 면 호출부가 키를 한 회차 미뤄야 한다
     * @private
     */
    _bypassCombo(keyval, evdevKeycode, taggedState, isAutoEnglishTrigger) {
        const preedit = this._inputMethod._preeditText || '';
        const committed = preedit.length > 0;
        if (committed) {
            this._inputMethod.clearPreedit();
            this._inputMethod.commitText(preedit);
            this._inputMethod._armCommitEchoGuard(preedit);
        }

        if (!this._dbusIME?.isConnected) return committed;

        if (isAutoEnglishTrigger) {
            // 조합 트리거(예: `key:Ctrl+B`) — 키 자체는 앱으로 가고, 엔진은 이
            // 통지를 받아 한→영 모드 전환만 한다. 반환값(consumed)은 어차피
            // 무시하는 값이라 응답을 기다릴 이유가 없다.
            this._dbusIME.processKeyAsync(keyval, evdevKeycode, taggedState);
        } else {
            this._dbusIME.resetAsync();
        }
        return committed;
    }

    /**
     * DBus 로 나가는 state 에 repeat 태깅 비트를 부착한다.
     *
     * AWARE 비트는 항상 세워 데몬이 이 프런트를 "반복 정확 구분" 프런트로 인식하게 하고,
     * Clutter event flags 에 REPEATED 가 있으면 REPEAT 비트를 추가한다. 데몬은
     * ignore_key_repeat on 일 때만 이 비트로 자동반복을 억제(off 면 from_x11_mask 가 무시).
     * JS 32bit signed 함정 회피를 위해 `>>> 0` 로 unsigned 정규화 후 반환.
     *
     * @param {number} state - raw modifier 비트필드
     * @param {Clutter.Event} [event] - 원본 키 이벤트 (flags 판정용)
     * @returns {number} 태깅된 state (unsigned)
     * @private
     */
    _tagRepeatBits(state, event) {
        let s = state | UNIM_REPEAT_AWARE_MASK;
        if (event?.get_flags && (event.get_flags() & CLUTTER_FLAG_REPEATED))
            s |= UNIM_KEY_REPEAT_MASK;
        return s >>> 0;
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
     * ATF 토글 단축키(Ctrl/Alt/Super 조합)인지 판정한다 — auto_english divert 와
     * 같은 selective-bypass 예외. true 면 호출부가 blanket bypass 를 건너뛰어 키가
     * 일반 ProcessKeyEvent 경로로 가고, 최종 판정은 engine 의 정확-일치
     * (`atf_hotkey_kind`)가 내린다: engine 매칭 시 consumed=true 로 키 소비(앱
     * 미전달 — auto_english 와 달리 토글키는 앱에 가면 안 된다), 불일치 시
     * consumed=false 로 종전처럼 앱 전달. 그래서 mirror 의 드문 false-positive 도
     * 앱 단축키를 죽이지 않는다.
     *
     * `auto_typefix.enabled` 는 보지 않는다 — 꺼진 ATF 를 켜는 것이 토글키의
     * 존재 이유다. Shift 단독 조합·bare 키는 bypass 대상이 아니므로 검사하지
     * 않는다(이미 정상 경로).
     *
     * @param {number} evdevKeycode
     * @param {number} state - Clutter modifier 비트필드
     * @returns {boolean}
     * @private
     */
    _matchesAtfHotkey(evdevKeycode, state) {
        try {
            const cfg = this._dbusIME ? this._dbusIME.getCachedConfig() : null;
            const atf = cfg && cfg.engine ? cfg.engine.auto_typefix : null;
            if (!atf) return false;
            const name = EVDEV_TO_KEYNAME.get(evdevKeycode);
            if (!name) return false;
            const ctrl = (state & X11_CONTROL_MASK) !== 0;
            const alt = (state & X11_MOD1_MASK) !== 0;
            const superKey = (state & X11_MOD4_MASK) !== 0;
            const shift = (state & X11_SHIFT_MASK) !== 0;
            // Ctrl/Alt/Super 전무 → bypass 분기 자체에 안 들어오지만 방어적으로.
            if (!ctrl && !alt && !superKey) return false;
            const lists = [
                atf.toggle_enabled_keys,
                atf.toggle_forward_keys,
                atf.toggle_reverse_keys,
            ];
            for (const list of lists) {
                if (!Array.isArray(list)) continue;
                for (const spec of list) {
                    const h = parseAtfComboHotkey(spec);
                    if (h && h.name === name
                        && h.ctrl === ctrl
                        && h.alt === alt
                        && h.superKey === superKey
                        && h.shift === shift) {
                        return true;
                    }
                }
            }
            return false;
        } catch (e) {
            unimError('KEY', `ATF 조합 매칭 오류(무시): ${e.message}`);
            return false;
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
                this._inputMethod.setToggleKeyNotifier(null);
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
            // bare Alt_R 은 데몬에 비소비 통지 후 그대로 전파 (Wayland vfunc 경로와 동일 설계).
            // captured-event 는 KEY_PRESS 만 이 경로로 진입(enable() 의 type 가드)하므로
            // PRESS 전용 통지 — 이중 토글 없음.
            if (keyval === Clutter.KEY_Alt_R && this._dbusIME?.isConnected) {
                this._dbusIME.processKeyAsync(keyval, evdevKeycode, state >>> 0);
            }
            return Clutter.EVENT_PROPAGATE;
        }

        // 2. 팝업 활성 시에도 ProcessKeyEvent로 fall-through (이중 처리 방지)

        // 3. Ctrl/Alt/Super 조합 → 조합 중이면 커밋 후 바이패스
        //    예외: ATF 토글 조합은 bypass 를 건너뛰고 6번 ProcessKeyEvent 경로로
        //    (engine 정확-일치가 소비 판정 — consumed 면 EVENT_STOP).
        if ((state & BYPASS_MODIFIER_MASK) && !this._matchesAtfHotkey(evdevKeycode, state)) {
            // captured-event 폴백이므로 forward_key 불필요, PROPAGATE 로 키 전달.
            // 뒷정리는 vfunc 경로와 같은 비동기 헬퍼 — 여기서도 동기 D-Bus 로
            // 메인 스레드를 잡으면 고정키 래치 해제와 경합한다.
            this._bypassCombo(keyval, evdevKeycode, this._tagRepeatBits(state, event),
                              this._matchesAutoEnglishCombo(evdevKeycode, state));
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
            result = this._dbusIME.processKey(keyval, evdevKeycode, this._tagRepeatBits(state, event));
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
