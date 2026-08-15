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
import { unimLog, unimError } from './logging.js';

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
        this._lastKeyEventTimeMs = Date.now();

        const eventType = event.type();
        const keyval = event.get_key_symbol();
        const keycode = event.get_key_code();
        const state = event.get_state();
        const flags = event.get_flags ? event.get_flags() : 0;
        const typeName = eventType === Clutter.EventType.KEY_PRESS ? 'PRESS' :
                         eventType === Clutter.EventType.KEY_RELEASE ? 'RELEASE' : `OTHER(${eventType})`;

        unimLog('IME', `vfunc_filter: type=${typeName}, keyval=${keyval}, keycode=${keycode}, state=0x${state.toString(16)}, flags=0x${flags.toString(16)}`);

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
        unimLog('IME', `vfunc_focus_in: _hasFocus=true`);

        // 포커스 없는 동안 도착해 대기 중이던 purpose 만 flush.
        // 무조건 Normal 송신은 하지 않는다 — Mutter 가 vfunc_update_content_purpose
        // 를 vfunc_focus_in 보다 먼저 호출하는 순서라면(호출 순서 미확정, gnome_reset_design
        // 근거 1) 방금 받은 Password 를 여기서 즉시 Normal 로 덮어써 비번 차단을
        // 영구 무력화하게 된다. Normal 복귀는 vfunc_focus_out 에서만 담당(하단).
        if (this._pendingPurpose !== null) {
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
    }

    vfunc_focus_out() {
        unimLog('IME', `vfunc_focus_out 호출됨 (stack: popup commit 디버깅)`);
        // 1. 커밋 최우선: 로컬 preedit 백업
        const localPreedit = this._preeditText;

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
        // _pendingPurpose 는 지우지 않는다 — 포커스 없는 동안 이미 도착한 다음 필드의
        // purpose 일 수 있으며, 그 몫은 다음 vfunc_focus_in 이 flush 한다.
    }

    vfunc_reset() {
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
        // 호출자가 곧이어 데몬 reset() 을 부르면 같은 글자가 CommitText 로 되돌아온다.
        // 미리 가드를 무장해 그 메아리를 흘린다(vfunc_reset 주석 참조).
        this._armCommitEchoGuard(preedit);
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
        // 최신 surrounding text + cursor/anchor를 캐시 (gedit/gnome-text-editor 호환)
        this._lastSurrounding = { text: text || '', cursor: cursor || 0, anchor: anchor || 0 };
        if (this._dbusIME?.isConnected) {
            this._dbusIME.setSurroundingText(text || '', cursor || 0, anchor || 0);
        }
    }

    /**
     * 마지막으로 받은 surrounding text + cursor/anchor 반환
     * TypeFix가 request_surrounding() 응답 후 호출
     * @returns {{text: string, cursor: number, anchor: number}}
     */
    getLastSurrounding() {
        return this._lastSurrounding;
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
     * 계산된 effective purpose 를 포커스 상태에 따라 즉시 송신하거나 대기시킨다.
     *
     * 포커스 없음: 다음 vfunc_focus_in 이 flush 할 수 있도록 _pendingPurpose 에 버퍼.
     * 포커스 있음: 직전 송신값과 다를 때만 handler 호출(dedupe) — mutter 가 커밋마다
     * update_content_purpose 를 반복 호출할 수 있어 call_sync 폭주를 막는다.
     * @private
     */
    _applyContentPurpose() {
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

        unimLog('IME', `commitText: text='${text}', _hasFocus=${this._hasFocus}, _preeditText='${this._preeditText}'`);

        // preedit이 남아있으면 먼저 클리어
        if (this._preeditText.length > 0) {
            this.clearPreedit();
        }

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
        this._preeditText = text || '';

        try {
            if (this._preeditText.length > 0) {
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
        try {
            this.delete_surrounding(-(charCount), charCount);
            unimLog('IME', `deleteSurrounding: ${charCount}자 삭제`);
        } catch (e) {
            unimError('IME', `deleteSurrounding 실패: ${e.message}`);
        }
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
