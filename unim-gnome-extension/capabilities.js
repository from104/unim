/**
 * 프런트 능력 비트 계산 — gi 의존 없는 순수 모듈 (node 단위 시험 대상)
 *
 * `SetClientCapabilities(caps)` 로 데몬에 선언하는 능력 비트를 정한다. 대상③(커서가 걸친
 * 단어) 확정 경로(`REPLACE_SURROUNDING`)는 mutter `delete_surrounding` 의 삭제 단위
 * 실측(HANJA_WORD_SPEC §6.5 (10)) 전까지 꺼 둔다. 실측용으로만 GSettings 키
 * `surrounding-word-replace`(기본 false)로 켤 수 있다 — prefs UI 에는 노출하지 않는다.
 *
 * @module capabilities
 */

/** SetClientCapabilities 비트 — unim-dbus/SPEC.md §6.1 */
export const CAP_REPLACE_SURROUNDING = 0x1;
export const CAP_POINTER_HINTS = 0x4;

/** 실측 스위치 GSettings 키 (org.gnome.shell.extensions.unim). */
export const SURROUNDING_WORD_REPLACE_KEY = 'surrounding-word-replace';

/**
 * 지금 선언할 프런트 능력 비트.
 * 켜짐(정확히 `true`)이면 0x5(REPLACE_SURROUNDING | POINTER_HINTS), 그 외는 0x4.
 * `SELECTION_RELIABLE`(0x2)은 선언하지 않는다(App-push 경로).
 *
 * @param {boolean} replaceEnabled - `surrounding-word-replace` 값
 * @returns {number}
 */
export function computeClientCapabilities(replaceEnabled) {
    return replaceEnabled === true
        ? (CAP_REPLACE_SURROUNDING | CAP_POINTER_HINTS)
        : CAP_POINTER_HINTS;
}
