/**
 * 포인터 사건 분류 — 대상③ 거부 신호용 (gi 의존 없는 순수 모듈)
 *
 * 마우스·터치 사건을 `SurroundingHint` kind(2 POINTER_PLAIN · 3 POINTER_SELECT)로
 * 분류한다. 판정은 엔진(HANJA_WORD_SPEC §2.4b.6·§2.4b.7)이 하고, 여기서는 사건의
 * 성격만 가른다. 선택 내용·길이는 추정하지 않는다.
 *
 * 규칙 (unim-gnome-extension/SPEC.md §2.8):
 *  - 단일 클릭(Shift 없음, 직전 눌림에서 500ms 초과) → POINTER_PLAIN
 *  - Shift+클릭 · 500ms 안의 재눌림(더블/세 번 클릭) → POINTER_SELECT
 *  - 눌린 채 4px 넘게 이동(드래그) → 눌림당 1회 POINTER_SELECT
 *  - 보조 버튼(3) → 무시, 가운데 버튼(2) → POINTER_PLAIN(붙여넣기는 앱이 커밋)
 *  - 터치 → POINTER_SELECT (길게 눌러 선택을 구분할 수 없다)
 *
 * @module pointer_hints
 */

/** 이 시간(ms) 안에 이어진 두 번째 버튼 눌림은 더블클릭으로 본다. */
export const MULTI_CLICK_MS = 500;
/** 눌린 채 이 거리(px) 넘게 움직이면 드래그(마우스 선택)로 본다. */
export const DRAG_THRESHOLD_PX = 4;

/** SurroundingHint kind — 데몬 계약(unim-dbus/SPEC.md §6.1)과 같은 값. */
export const HINT_KEY = 1;
export const HINT_POINTER_PLAIN = 2;
export const HINT_POINTER_SELECT = 3;
export const HINT_PRIMARY_CHANGED = 4;
export const HINT_SELECTION_UNKNOWN = 5;

/**
 * @typedef {object} PointerState
 * @property {boolean} down - 주 버튼(1)이 눌린 채인지
 * @property {number|null} pressX - 주 버튼 눌림 x
 * @property {number|null} pressY - 주 버튼 눌림 y
 * @property {number|null} lastPressMs - 직전 주 버튼 눌림 시각(ms, 단조 시계)
 * @property {boolean} dragReported - 이번 눌림에서 드래그를 이미 보고했는지
 */

/** @returns {PointerState} */
export function createPointerState() {
    return { down: false, pressX: null, pressY: null, lastPressMs: null, dragReported: false };
}

/**
 * 포인터 사건 1건을 분류한다. 상태는 불변 갱신 — 새 상태를 돌려준다.
 *
 * @param {PointerState} state
 * @param {{type: 'press'|'motion'|'release'|'touch', button?: number, shift?: boolean,
 *          nowMs?: number, x?: number, y?: number}} ev
 * @returns {{state: PointerState, hint: 0|2|3}} hint 0 이면 보고할 것 없음
 */
export function classifyPointer(state, ev) {
    switch (ev.type) {
    case 'press': {
        const button = ev.button ?? 1;
        if (button === 3) return { state, hint: 0 };                 // 보조 버튼(문맥 메뉴)
        if (button !== 1) return { state, hint: HINT_POINTER_PLAIN };  // 가운데 버튼 등
        const now = ev.nowMs ?? 0;
        const multi = state.lastPressMs !== null && now - state.lastPressMs < MULTI_CLICK_MS;
        const next = {
            down: true,
            pressX: ev.x ?? null,
            pressY: ev.y ?? null,
            lastPressMs: now,
            dragReported: false,
        };
        return { state: next, hint: (multi || ev.shift) ? HINT_POINTER_SELECT : HINT_POINTER_PLAIN };
    }
    case 'motion': {
        if (!state.down || state.dragReported) return { state, hint: 0 };
        const px = state.pressX ?? ev.x ?? 0;
        const py = state.pressY ?? ev.y ?? 0;
        if (Math.hypot((ev.x ?? px) - px, (ev.y ?? py) - py) <= DRAG_THRESHOLD_PX)
            return { state, hint: 0 };
        return { state: { ...state, dragReported: true }, hint: HINT_POINTER_SELECT };
    }
    case 'release':
        if ((ev.button ?? 1) !== 1 || !state.down) return { state, hint: 0 };
        return { state: { ...state, down: false }, hint: 0 };
    case 'touch':
        return { state, hint: HINT_POINTER_SELECT };
    default:
        return { state, hint: 0 };
    }
}

// ── 키 로그 이름 ────────────────────────────────────────────────────────────
// 로그에는 탐색·편집 키만 이름을 남기고 문자 키는 `char` 로 가린다(HANJA_WORD_SPEC §2.8 —
// 사용자 입력 평문 금지). X11 keysym 값 그대로(Clutter.KEY_* 와 같다) — gi 비의존.
const KEY_LOG_NAMES = new Map([
    [0xff51, 'left'], [0xff52, 'up'], [0xff53, 'right'], [0xff54, 'down'],
    [0xff50, 'home'], [0xff57, 'end'], [0xff55, 'pgup'], [0xff56, 'pgdn'],
    [0xff96, 'left'], [0xff97, 'up'], [0xff98, 'right'], [0xff99, 'down'],
    [0xff95, 'home'], [0xff9c, 'end'], [0xff9a, 'pgup'], [0xff9b, 'pgdn'],
    [0xff0d, 'enter'], [0xff8d, 'enter'],
    [0xff09, 'tab'], [0xfe20, 'tab'],
    [0xff08, 'backspace'], [0xffff, 'delete'], [0xff9f, 'delete'],
    [0xff1b, 'escape'],
]);

/**
 * 힌트 로그용 키 이름 — 탐색·편집 키만 이름, 나머지는 'char'.
 * @param {number} keyval - X11 keysym
 * @returns {string}
 */
export function hintKeyLogName(keyval) {
    return KEY_LOG_NAMES.get(keyval) ?? 'char';
}
