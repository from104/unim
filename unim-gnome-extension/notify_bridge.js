/**
 * UNIM 상황 알림 브리지 (NOTIFY_SPEC §3.3 P2)
 *
 * 데몬이 `gnome-shell-notify` 등록자가 있을 때 내는 InputMethod `Notify` 시그널을
 * 받아 GNOME Shell 배너로 그린다. 확장 전용 `MessageTray.Source` 1개를 재사용하고
 * 알림은 **transient** 로 올린다 — 배너가 사라지면 알림 목록(메시지 트레이)에 남지 않는다.
 * 새 알림 전에 직전 알림을 `destroy()` 해 "최신 1장" 규칙을 지킨다.
 *
 * GNOME 은 배너 표시 시간을 셸이 고정하므로 `duration_ms` 는 무시한다(P3 St 토스트부터 적용).
 *
 * 로그 규칙(§2.3): title/body 는 입력 텍스트를 담을 수 있어 **어떤 로그에도 남기지 않는다** —
 * kind 만 남긴다.
 *
 * 버전 호환(metadata.json shell-version 45~50):
 *   - 46+  Source/Notification 이 props 객체 생성자, `isTransient` 프로퍼티
 *   - 45   위치 인자 생성자 + `setTransient(true)`, 표시는 `showNotification`
 * 버전 판정은 `Config.PACKAGE_VERSION` 으로 하고, 메서드는 존재 여부로 고른다.
 */

import * as Config from 'resource:///org/gnome/shell/misc/config.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import { unimLog, unimError } from './logging.js';

/** 데몬에 등록하는 알림 브리지 이름 (unim-dbus `NOTIFY_BRIDGE_NAME` 과 같아야 한다). */
export const NOTIFY_BRIDGE_NAME = 'gnome-shell-notify';

/** `Notify` 시그널 flags (unim-dbus SPEC §5.2) */
export const FLAG_TEXT_SHOWN = 0x01;
export const FLAG_REPLACE = 0x02;

const SOURCE_TITLE = 'UNIM';
const SOURCE_ICON = 'unim-korean';

/**
 * `kind`(DBus, 세분) → `engine.notify.events` 설정명. 매핑에 없는 kind 는 설정으로 거르지 않는다.
 * (데몬이 이미 거른 뒤라 여기는 이중 안전장치다 — 설정을 못 읽으면 통과시킨다.)
 */
const KIND_TO_EVENT = {
    atf_corrected_forward: 'atf_corrected',
    atf_corrected_reverse: 'atf_corrected',
    atf_suppressed: 'atf_suppressed',
    blacklist_learned: 'blacklist_learned',
    password_enter: 'password_enter',
    password_leave: 'password_leave',
    mode_toggle_suppressed: 'mode_toggle_suppressed',
    mode_changed: 'mode_changed',
    feature_toggled: 'feature_toggled',
    feature_result: 'feature_result',
};

/** GNOME Shell 메이저 버전 (파싱 실패 시 최신으로 간주). */
function shellMajor() {
    const n = parseInt(String(Config.PACKAGE_VERSION).split('.')[0], 10);
    return Number.isFinite(n) ? n : 99;
}

/**
 * `engine.notify` 설정이 이 알림을 허용하는가.
 *
 * @param {object|null} notifyCfg - `config.engine.notify` (없으면 null → 허용)
 * @param {string} kind - DBus kind
 * @returns {boolean}
 */
export function notifyAllowed(notifyCfg, kind) {
    if (!notifyCfg) return true;
    if (notifyCfg.enabled === false) return false;
    if (kind === 'test') return true;
    const ev = KIND_TO_EVENT[kind];
    if (ev && Array.isArray(notifyCfg.events) && !notifyCfg.events.includes(ev))
        return false;
    return true;
}

export class NotifyBridge {
    /**
     * @param {object} [opts]
     * @param {function(): (object|null)} [opts.getNotifyConfig] - 현재 `engine.notify` 반환
     */
    constructor(opts = {}) {
        this._getNotifyConfig = opts.getNotifyConfig || (() => null);
        /** @type {object|null} MessageTray.Source (확장 전용, 재사용) */
        this._source = null;
        this._sourceDestroyId = 0;
        /** @type {object|null} 직전 알림 */
        this._last = null;
        this._lastDestroyId = 0;
        this._destroyed = false;
    }

    /** 이 셸에서 브리지를 쓸 수 있는가 (미지원이면 등록하지 않아 fdo 폴백이 유지된다). */
    static supported() {
        return typeof MessageTray.Source === 'function' &&
            typeof MessageTray.Notification === 'function' &&
            !!Main.messageTray;
    }

    /**
     * `Notify` 시그널 처리.
     *
     * @param {string} kind
     * @param {string} title
     * @param {string} body
     * @param {number} _durationMs - GNOME 은 무시
     * @param {number} _flags
     */
    onNotifySignal(kind, title, body, _durationMs, _flags) {
        if (this._destroyed) return;
        if (!notifyAllowed(this._getNotifyConfig(), kind)) {
            unimLog('NOTIFY', `[Notify] kind=${kind} 설정으로 폐기`);
            return;
        }
        if (this.show(title, body))
            unimLog('NOTIFY', `[Notify] kind=${kind} 경로=extension`);
    }

    /**
     * transient 알림 1건 표시 (직전 알림은 먼저 걷는다).
     *
     * @param {string} title
     * @param {string} body
     * @returns {boolean} 표시 시도 성공 여부
     */
    show(title, body) {
        if (this._destroyed) return false;
        try {
            // 직전 알림을 먼저 떼어 두고 새 알림을 올린 뒤 파괴한다 — 마지막 알림이 사라지면
            // 셸이 Source 를 같이 정리하므로, 순서를 뒤집으면 Source 재사용이 무너진다.
            const prev = this._takeLast();
            const source = this._ensureSource();
            const notification = this._createNotification(source, title, body);
            this._lastDestroyId = notification.connect('destroy', () => {
                this._last = null;
                this._lastDestroyId = 0;
            });
            this._last = notification;
            this._present(source, notification);
            if (prev) {
                try {
                    prev.destroy();
                } catch (_e) { /* 이미 파괴됨 */ }
            }
            return true;
        } catch (e) {
            unimError('NOTIFY', `알림 표시 실패: ${e.message}`);
            return false;
        }
    }

    /** 정리 — 직전 알림과 Source 를 걷는다. */
    destroy() {
        this._destroyed = true;
        this._dropLast();
        this._dropSource();
    }

    // -------------------------------------------------------------------

    /**
     * 직전 알림을 추적에서 떼어 돌려준다(파괴는 호출자 몫).
     * @returns {object|null}
     * @private
     */
    _takeLast() {
        const n = this._last;
        if (!n) return null;
        this._last = null;
        const id = this._lastDestroyId;
        this._lastDestroyId = 0;
        try {
            if (id) n.disconnect(id);
        } catch (_e) { /* 이미 파괴됨 */ }
        return n;
    }

    /** @private */
    _dropLast() {
        const n = this._takeLast();
        if (!n) return;
        try {
            n.destroy();
        } catch (_e) { /* 이미 파괴됨 */ }
    }

    /** @private */
    _dropSource() {
        const s = this._source;
        if (!s) return;
        this._source = null;
        const id = this._sourceDestroyId;
        this._sourceDestroyId = 0;
        try {
            if (id) s.disconnect(id);
            s.destroy();
        } catch (_e) { /* 이미 파괴됨 */ }
    }

    /** @private */
    _ensureSource() {
        if (this._source) return this._source;
        const source = shellMajor() >= 46
            ? new MessageTray.Source({ title: SOURCE_TITLE, iconName: SOURCE_ICON })
            : new MessageTray.Source(SOURCE_TITLE, SOURCE_ICON);
        // 마지막 알림이 사라지며 셸이 Source 를 정리할 수 있다 — 그러면 다음에 새로 만든다.
        this._sourceDestroyId = source.connect('destroy', () => {
            this._source = null;
            this._sourceDestroyId = 0;
        });
        Main.messageTray.add(source);
        this._source = source;
        return source;
    }

    /** @private */
    _createNotification(source, title, body) {
        if (shellMajor() >= 46) {
            return new MessageTray.Notification({
                source, title, body, isTransient: true,
            });
        }
        const n = new MessageTray.Notification(source, title, body);
        n.setTransient(true);
        return n;
    }

    /** @private */
    _present(source, notification) {
        if (typeof source.addNotification === 'function')
            source.addNotification(notification);
        else
            source.showNotification(notification);
    }
}
