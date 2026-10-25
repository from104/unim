/**
 * 문서 스냅샷(surrounding) 사실 보고 판정 — gi 의존 없는 순수 모듈 (node 단위 시험 대상)
 *
 * 대상③(커서가 걸친 단어) 공통 계약(통합 계획 §2.1 F1·F2·F7)의 GNOME 쪽 판정을 맡는다.
 * 확장은 push 형이라 엔진이 문서를 당겨 올 수 없다. 그래서 두 가지를 확장이 책임진다.
 *
 *  1. **재송신** — mutter 가 새 필드의 `set_surrounding` 을 창 FocusIn / FIELD_FOCUS 보다
 *     먼저 넘기면, 엔진은 SetSurroundingText → FocusIn 순서로 받아 낡음(stale)이 서고
 *     다음 보고가 올 때까지 풀리지 않는다. FocusIn 계열을 보낸 직후, 같은 필드의 마지막
 *     스냅샷을 한 번 더 보내 낡음을 푼다 (`decideResend`).
 *  2. **확정 대조** — `ReplaceSurroundingText` 의 `expect_text` 를 마지막 스냅샷으로 대조해
 *     맞지 않으면 교체를 드롭한다 (`verifyReplaceExpectation`).
 *
 * 시험 방식(`scripts/test-gnome-surrounding-hints.mjs`)이 data: URL 로 import 하므로 이
 * 파일은 **다른 모듈을 import 하지 않는다**. pointer_hints.js 의 kind 값은 여기에 같은
 * 값으로 다시 선언하고 시험이 서로 같은지 확인한다.
 *
 * 오프셋 단위는 코드포인트다 (SPEC.md §2.8 — mutter cursor/anchor 도 코드포인트).
 * 로그에는 길이만 남기고 문서 내용은 남기지 않는다 (HANJA_WORD_SPEC §2.8).
 *
 * @module surrounding_facts
 */

// ── 상수 ────────────────────────────────────────────────────────────────────

/** SurroundingHint kind — 데몬 계약(unim-dbus/SPEC.md §6.1). 1~3 은 pointer_hints.js 와 같은 값. */
export const FACT_HINT_KEY = 1;
export const FACT_HINT_POINTER_PLAIN = 2;
export const FACT_HINT_POINTER_SELECT = 3;
/** 신설 — 입력 필드(위젯) 단위 포커스. 창 단위 FocusIn 과 별개로 필드가 바뀔 때마다 보낸다. */
export const HINT_FIELD_FOCUS = 6;

/**
 * 재송신은 **필드 포커스(`vfunc_focus_in`)가 이 시간(ms) 안에 있었고 그 뒤 focus_out 이 없을 때만**
 * 허용한다. 창 전환에서 `notify::focus-window` 가 IM 의 focus_out(이전 창)보다 먼저 오면, 이전 창
 * 필드의 캐시가 새 창의 FocusIn 뒤에 나가는 것을 막는 방어선이다. 정상 순서(set_surrounding 과
 * FocusIn 의 간격)는 수십 ms 이내라 넉넉히 잡아도 짧다.
 */
export const FIELD_FOCUS_RECENT_MS = 500;
/** 이 시간(ms) 넘게 묵은 스냅샷은 재송신하지 않는다. 같은 창 안에서만 뜻이 있으므로 위 상수와 같게 둔다. */
export const RESEND_MAX_AGE_MS = FIELD_FOCUS_RECENT_MS;

/** 이 kind 의 힌트가 나간 뒤에는 스냅샷이 현재 문서와 같다고 볼 수 없다 (엔진 R2: stale 로 만드는 사건). */
const SUPERSEDING_HINTS = new Set([FACT_HINT_KEY, FACT_HINT_POINTER_PLAIN, FACT_HINT_POINTER_SELECT]);

/** 재송신 불가 이유 코드 (로그용). */
export const RESEND_OK = 'ok';
export const RESEND_NOFOCUS = 'nofocus';
export const RESEND_SENSITIVE = 'sensitive';
export const RESEND_NOCACHE = 'nocache';
export const RESEND_GEN_MISMATCH = 'gen_mismatch';
export const RESEND_DIRTY = 'dirty';
export const RESEND_KEY_SENT = 'key_sent';
export const RESEND_FOCUS_OLD = 'focus_old';
export const RESEND_EXPIRED = 'expired';

/** 확정 대조 결과 코드. 로그 `reason=` 값과 같다. */
export const VERIFY_OK = 'ok';
export const VERIFY_MISMATCH = 'verify_mismatch';

// ── 상태 ────────────────────────────────────────────────────────────────────

/**
 * @typedef {object} SurroundingCache
 * @property {string} text
 * @property {number} cursor - 코드포인트
 * @property {number} anchor - 코드포인트
 * @property {number} gen - 이 스냅샷이 속한 필드 세대
 * @property {number} atMs - 받은 시각(ms, 단조 시계)
 * @property {boolean} dirty - 받은 뒤 KEY·포인터 힌트 또는 확장의 문서 변경(커밋·삭제)이 있었는지.
 *   재송신과 **확정 대조** 모두 이 값이 서 있으면 쓰지 않는다.
 * @property {boolean} keySent - 받은 뒤 엔진으로 키(ProcessKeyEvent)를 보냈는지. **재송신에만** 쓴다 —
 *   한자키 자체가 이 경로라 확정 대조까지 막으면 대상③ 이 영영 실행되지 않는다.
 */

/**
 * @typedef {object} SurroundingFacts
 * @property {number} fieldGen - 필드 포커스(focus_in) 횟수. 현재 포커스 필드의 세대
 * @property {number|null} fieldFocusAtMs - 마지막 focus_in 시각(ms). focus_out 뒤에는 null
 * @property {SurroundingCache|null} cache
 */

/** @returns {SurroundingFacts} */
export function createSurroundingFacts() {
    return { fieldGen: 0, fieldFocusAtMs: null, cache: null };
}

/**
 * 필드 포커스 진입(`vfunc_focus_in`). 세대를 올린다.
 * 캐시는 지우지 않는다 — mutter 가 `set_surrounding` 을 focus_in 보다 먼저 보낸 순서라면
 * 그 값이 바로 새 필드의 것이다 (포커스 아웃 때마다 비우므로 남은 캐시는 새 필드 몫).
 *
 * @param {SurroundingFacts} f
 * @param {number} nowMs - focus_in 시각(ms, 단조 시계)
 * @returns {SurroundingFacts}
 */
export function factsOnFocusIn(f, nowMs) {
    return { fieldGen: f.fieldGen + 1, fieldFocusAtMs: nowMs, cache: f.cache };
}

/**
 * 필드 포커스 이탈(`vfunc_focus_out`). 이전 필드의 본문이 다음 필드로 새지 않게 캐시를 비운다.
 * @param {SurroundingFacts} f
 * @returns {SurroundingFacts}
 */
export function factsOnFocusOut(f) {
    return { fieldGen: f.fieldGen, fieldFocusAtMs: null, cache: null };
}

/**
 * 목적(purpose)이 비밀번호·PIN 으로 바뀌었다(focus_in 뒤에 늦게 도착한 경우 포함). 보관한 본문을 비운다.
 * @param {SurroundingFacts} f
 * @returns {SurroundingFacts}
 */
export function factsOnSensitive(f) {
    return f.cache ? { ...f, cache: null } : f;
}

/**
 * `set_surrounding` 수신. 포커스가 없을 때 온 값은 **다음** 필드의 것이다 (focus_out 이 캐시를
 * 비운 뒤라서) — 세대를 +1 로 매겨 두면 focus_in 에서 세대가 올라 맞아떨어진다.
 * 비밀번호·PIN 칸에서는 아무것도 보관하지 않는다 (캐시 재송신 금지).
 *
 * @param {SurroundingFacts} f
 * @param {{text: string, cursor: number, anchor: number}} snap
 * @param {{hasFocus: boolean, sensitive: boolean, nowMs: number}} ctx
 * @returns {SurroundingFacts}
 */
export function factsOnSurrounding(f, snap, ctx) {
    if (ctx.sensitive) return { ...f, cache: null };
    return {
        ...f,
        cache: {
            text: snap.text ?? '',
            cursor: snap.cursor ?? 0,
            anchor: snap.anchor ?? 0,
            gen: ctx.hasFocus ? f.fieldGen : f.fieldGen + 1,
            atMs: ctx.nowMs,
            dirty: false,
            keySent: false,
        },
    };
}

/**
 * 힌트를 엔진에 보냈음을 기록한다. KEY·POINTER_PLAIN·POINTER_SELECT 는 스냅샷을 낡게 만든다.
 * @param {SurroundingFacts} f
 * @param {number} kind
 * @returns {SurroundingFacts}
 */
export function factsOnHintSent(f, kind) {
    if (!f.cache || f.cache.dirty || !SUPERSEDING_HINTS.has(kind)) return f;
    return { ...f, cache: { ...f.cache, dirty: true } };
}

/**
 * 엔진으로 키를 보냈음을 기록한다 (ProcessKeyEvent 경로 — 통과 키는 앱 문서를 바꾼다).
 * 재송신만 막고 확정 대조는 막지 않는다 (`SurroundingCache.keySent` 참조).
 * @param {SurroundingFacts} f
 * @returns {SurroundingFacts}
 */
export function factsOnEngineKey(f) {
    if (!f.cache || f.cache.keySent) return f;
    return { ...f, cache: { ...f.cache, keySent: true } };
}

/**
 * 확장이 문서를 직접 바꿨음을 기록한다 (commit·delete_surrounding). 스냅샷이 낡았다.
 * @param {SurroundingFacts} f
 * @returns {SurroundingFacts}
 */
export function factsOnDocumentChanged(f) {
    if (!f.cache || f.cache.dirty) return f;
    return { ...f, cache: { ...f.cache, dirty: true } };
}

// ── 재송신 판정 ──────────────────────────────────────────────────────────────

/**
 * FIELD_FOCUS·창 FocusIn 을 보낸 직후, 마지막 스냅샷을 `SetSurroundingText` 로 한 번 더
 * 보내도 되는지 판정한다.
 *
 * 순서대로 거른다 (먼저 걸린 이유가 반환된다):
 *  1. 포커스가 없다                         → nofocus
 *  2. 비밀번호·PIN 칸                       → sensitive
 *  3. 캐시가 없다                           → nocache
 *  4. 캐시가 현재 필드 세대가 아니다        → gen_mismatch
 *  5. 필드 포커스가 없거나 500ms 보다 오래  → focus_old (창 전환에서 이전 창 캐시가 새는 것을 막는다)
 *  6. 캐시를 받은 뒤 KEY·포인터 힌트·커밋   → dirty   (엔진이 이미 낡음으로 알고 있다)
 *  7. 캐시를 받은 뒤 엔진으로 키를 보냄     → key_sent
 *  8. 너무 묵었다                           → expired
 *
 * @param {SurroundingFacts} f
 * @param {{hasFocus: boolean, sensitive: boolean, nowMs: number}} ctx
 * @returns {{resend: boolean, reason: string}}
 */
export function decideResend(f, ctx) {
    if (!ctx.hasFocus) return { resend: false, reason: RESEND_NOFOCUS };
    if (ctx.sensitive) return { resend: false, reason: RESEND_SENSITIVE };
    const c = f.cache;
    if (!c) return { resend: false, reason: RESEND_NOCACHE };
    if (c.gen !== f.fieldGen) return { resend: false, reason: RESEND_GEN_MISMATCH };
    if (f.fieldFocusAtMs === null || ctx.nowMs - f.fieldFocusAtMs > FIELD_FOCUS_RECENT_MS)
        return { resend: false, reason: RESEND_FOCUS_OLD };
    if (c.dirty) return { resend: false, reason: RESEND_DIRTY };
    if (c.keySent) return { resend: false, reason: RESEND_KEY_SENT };
    if (ctx.nowMs - c.atMs > RESEND_MAX_AGE_MS) return { resend: false, reason: RESEND_EXPIRED };
    return { resend: true, reason: RESEND_OK };
}

// ── 확정 대조 ────────────────────────────────────────────────────────────────

/**
 * `ReplaceSurroundingText(a, b, commit, expect)` 를 실행하기 전에, 마지막 스냅샷의
 * `text[cursor−a .. cursor+b]` 가 `expect` 와 같은지 대조한다. 코드포인트 단위.
 * 캐시가 없거나, 받은 뒤 문서가 바뀌었을 수 있거나(`dirty`), 범위를 벗어나거나, `expect` 가 비었으면
 * 모두 불일치(fail-closed)다.
 *
 * @param {SurroundingCache|null} cache
 * @param {number} a - 커서 앞 문자 수
 * @param {number} b - 커서 뒤 문자 수
 * @param {string} expect
 * @returns {{ok: boolean, reason: string, cacheLen: number, expectLen: number}} 길이만 담는다
 */
export function verifyReplaceExpectation(cache, a, b, expect) {
    const expectChars = typeof expect === 'string' ? Array.from(expect) : null;
    const expectLen = expectChars ? expectChars.length : 0;
    if (!cache || cache.dirty || !expectChars || expectLen === 0 || !Number.isInteger(a) || !Number.isInteger(b)
        || a < 0 || b < 0) {
        return { ok: false, reason: VERIFY_MISMATCH, cacheLen: cache ? Array.from(cache.text).length : 0, expectLen };
    }
    const chars = Array.from(cache.text);
    const start = cache.cursor - a;
    const end = cache.cursor + b;
    if (start < 0 || end > chars.length || end - start !== expectLen) {
        return { ok: false, reason: VERIFY_MISMATCH, cacheLen: chars.length, expectLen };
    }
    const ok = chars.slice(start, end).join('') === expect;
    return { ok, reason: ok ? VERIFY_OK : VERIFY_MISMATCH, cacheLen: chars.length, expectLen };
}
