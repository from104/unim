/**
 * UNIM GNOME Shell Extension
 *
 * TypeFIX(오타 보정) + 실시간 IME(한글 입력기) 통합 확장
 * IBus를 대체하여 Clutter.InputMethod 기반 네이티브 입력을 제공합니다.
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension, gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';

import { VirtualKeyboard } from './vkbd.js';
import { UnimIndicator } from './indicator.js';
import { UnimDbusIME } from './dbus_ime.js';
import { PopupView, unpackPopupRender } from './popup_view.js';
import { UnimInputMethod } from './unim_input_method.js';
import { KeyHandler } from './key_handler.js';
import { PreeditOverlay } from './preedit_overlay.js';
// 한자/특수문자/이모지 popup 은 unim-popup-service (GTK4) 가 전담.
// GNOME extension 은 popup show signal 시 emoji ZWSP preedit 만 관리하고
// commit/cancel 결과는 daemon CommitText 시그널로 InputMethod 에 반영된다.
import { unimLog, unimError } from './logging.js';

export default class UnimExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._settings = null;
        this._shortcutIds = [];
        this._vkbd = null;
        this._indicator = null;

        // IME 모듈
        this._dbusIME = null;
        this._inputMethod = null;
        this._keyHandler = null;
        this._preeditOverlay = null;
        // emoji popup 활성 여부 — ShowEmojiPopupV2 / HidePopup 시그널로 추적.
        // popup 자체는 unim-popup-service 가 표시. extension 은 ZWSP preedit 으로
        // wayland text-input v3 IM 세션을 engage 시켜 commitText 가 휘발하지 않도록 한다.
        this._emojiPopupActive = false;

        this._focusWindowId = 0;


        // 설정 리스너
        this._settingsChangedIds = [];

        // TypeFIX 상태
        this._conversionInProgress = false;
    }

    enable() {
        unimLog('EXTENSION', 'Extension 활성화 시작...');
        try {
            this._settings = this.getSettings();
            this._vkbd = new VirtualKeyboard();

            // 패널 인디케이터 — 항상 표시 (show-panel-indicator 설정 키 deprecated)
            this._addIndicator();

            // TypeFIX 단축키
            this._bindAllShortcuts();

            // DBus 연결 (IME 모드 무관 — 인디케이터 모드 동기화 공용)
            // PR #3 부터 emoji 트리거 키바인딩은 제거됨 (engine 이 직접 키 받음).
            this._dbusIME = new UnimDbusIME();
            const windowId = this._getActiveWindowId();
            const connected = this._dbusIME.connect(windowId, (isKorean) => {
                if (this._indicator) this._indicator._onModeChanged(isKorean);
            });
            if (!connected) {
                unimError('EXTENSION', 'unim-daemon DBus 연결 실패 — 모드 동기화 비활성');
                this._dbusIME = null;
            } else {
                // 데몬에 프런트엔드 등록
                try {
                    this._dbusIME.registerFrontend('gnome-shell');
                } catch (e) {
                    console.warn(`[unim] RegisterFrontend 실패: ${e.message}`);
                }
            }

            // IME 활성화
            if (this._settings.get_boolean('enable-ime')) {
                this._enableIME();
            }

            this._connectSettingChanged('enable-ime', () => {
                const enabled = this._settings.get_boolean('enable-ime');
                if (enabled) this._enableIME();
                else this._disableIME();
            });

            unimLog('EXTENSION', 'Extension 활성화 완료');
        } catch (e) {
            unimError('EXTENSION', `Enable 실패: ${e.message}`);
        }
    }

    disable() {
        // IME 비활성화
        this._disableIME();

        // DBus 연결 종료 (enable() 본체에서 만든 instance — IME 모드 라이프사이클 외부)
        if (this._dbusIME) {
            // 데몬에 프런트엔드 해제 (실패 무시 — disable 중이라 데몬이 없을 수 있음)
            try {
                this._dbusIME.unregisterFrontend('gnome-shell');
            } catch (_e) { /* no-op */ }
            this._dbusIME.destroy();
            this._dbusIME = null;
        }

        // TypeFIX 단축키 해제
        this._unbindAllShortcuts();

        // 설정 리스너 정리
        for (const id of this._settingsChangedIds) {
            this._settings.disconnect(id);
        }
        this._settingsChangedIds = [];

        // 인디케이터 정리
        this._removeIndicator();

        this._settings = null;
        this._vkbd = null;
        unimLog('EXTENSION', 'Extension 비활성화 완료');
    }

    // ===========================================
    // IME 관리
    // ===========================================

    /**
     * IME 활성화
     *
     * 1. UnimInputMethod 생성 및 Mutter에 등록
     * 2. DBus InputContext 생성
     * 3. KeyHandler 시작
     * 4. 포커스 감시 시작
     */
    _enableIME() {
        if (this._keyHandler) return; // 이미 활성화됨

        try {
            // 1. UnimInputMethod 생성 (Clutter.InputMethod 서브클래스, seat 등록은 KeyHandler가 담당)
            this._inputMethod = new UnimInputMethod();

            // 2. DBus 연결 (enable() 본체에서 이미 생성된 instance 사용)
            if (!this._dbusIME) {
                unimError('EXTENSION', 'unim-daemon DBus 미연결 — IME 활성화 불가');
                this._cleanupIME();
                return;
            }
            this._inputMethod.setDbusIME(this._dbusIME);

            // DBus 연결 성공 시 포커스가 있으면 인디케이터 활성화
            if (this._indicator && global.display.focus_window) {
                this._indicator.setInputActive(true);
            }

            // 3. 한자키 설정 읽기
            const hanjaKeysyms = this._loadHanjaKeysyms();

            // 4. KeyHandler 시작 (한자키는 엔진에 위임, 팝업은 인디케이터가 표시)
            this._keyHandler = new KeyHandler(this._dbusIME, this._inputMethod, {
                hanjaKeysyms,
            });
            this._keyHandler.enable();

            // 5. Preedit 오버레이 초기화
            this._preeditOverlay = new PreeditOverlay();
            this._preeditOverlay.enable();

            // 6. Popup view — **GNOME Wayland 에서만 활성화**.
            //    Mutter 가 wlr-layer-shell·input_popup_v2 모두 지원 안 해서 popup-service
            //    GTK4 popup 이 표시되지 않으므로 extension 이 PopupRenderPayload 평면 payload
            //    를 St 위젯으로 렌더한다. X11 환경에서는 popup-service GTK4 popup 이 정상
            //    표시되므로 PopupView 활성 시 popup 이 두 번 그려진다 — 따라서 X11 에서는
            //    PopupView 를 띄우지 않고 popup-service 에 위임.
            //    popup-service 는 양 환경 모두에서 origin (RPC 수신 + signal 발행).
            const isWayland = Meta.is_wayland_compositor();
            if (isWayland) {
                this._popupView = new PopupView({
                    selectHanja:         (i) => this._dbusIME.popupSelectHanja(i),
                    selectSpecial:       (i) => this._dbusIME.popupSelectSpecial(i),
                    commitEmoji:         (s) => this._dbusIME.popupCommitEmoji(s),
                    toggleHanjaBookmark: (i) => this._dbusIME.popupToggleHanjaBookmark(i),
                    popupChangePage:     (d) => this._dbusIME.popupChangePage(d),
                    togglePopupExpand:   ()  => this._dbusIME.popupToggleExpand(),
                    setEmojiCategory:    (i) => this._dbusIME.popupSetEmojiCategory(i),
                    // P3/P6: PopupView.hide() 안에서 호출하는 cancel RPC. 누락 시
                    // outside-click·정상 dismiss 양쪽 모두 daemon engine state 정리 실패.
                    cancelHanja:         ()  => this._dbusIME.popupCancelHanja(),
                    cancelSpecial:       ()  => this._dbusIME.popupCancelSpecial(),
                });
                unimLog('EXTENSION', 'PopupView 활성 (GNOME Wayland)');
            } else {
                this._popupView = null;
                unimLog('EXTENSION', 'PopupView 비활성 (X11) — popup-service GTK4 popup 이 전담');
            }

            // popup-service signal → PopupView. emoji popup 의 ZWSP preedit 트릭은
            // wayland text-input v3 IM 세션을 engage 시켜 commitText 가 휘발하지 않게
            // 보장 (별도 책임). 한자/특수는 preedit 활성 상태에서 트리거되므로 IM 이 이미 engage.
            this._dbusIME.setPopupCallbacks({
                onShowHanja: (cursorRect) => {
                    if (this._popupView) this._popupView.show(cursorRect);
                },
                onShowSpecial: (cursorRect) => {
                    if (this._popupView) this._popupView.show(cursorRect);
                },
                onShowEmoji: (cursorRect) => {
                    this._emojiPopupActive = true;
                    if (this._inputMethod) {
                        this._inputMethod.updatePreedit('​'); // ZWSP — invisible
                    }
                    if (this._popupView) this._popupView.show(cursorRect);
                },
                onHidePopup: () => {
                    if (this._emojiPopupActive && this._inputMethod) {
                        this._inputMethod.updatePreedit('');
                    }
                    this._emojiPopupActive = false;
                    if (this._popupView) this._popupView.hide();
                },
                onPopupRender: (parameters) => {
                    const payload = unpackPopupRender(parameters);
                    if (payload && this._popupView) {
                        this._popupView.update(payload);
                    }
                },
                onCommitText: (text) => {
                    // chord idle flush 등 비동기 commit 경로. daemon 이 InputContext path 로
                    // emit 한 CommitText 시그널을 받아 _inputMethod 로 직접 commit.
                    if (text && this._inputMethod) {
                        this._inputMethod.commitText(text);
                    }
                },
                onUpdatePreedit: (text, _cursorPos, visible) => {
                    // chord idle flush preedit 유지 모드 — daemon 이 InputContext path 로
                    // emit 한 UpdatePreeditText 시그널을 받아 _inputMethod 로 preedit 갱신.
                    if (this._inputMethod) {
                        this._inputMethod.updatePreedit(visible ? text : '');
                    }
                },
                onAutoTypeFix: (deleteChars, commitText, preeditText) => {
                    if (this._vkbd && this._inputMethod && this._inputMethod._hasFocus) {
                        unimLog('EXT', `AutoTypeFix 적용: bs=${deleteChars}, commit='${commitText}', preedit='${preeditText}'`);
                        // self-feedback 차단: vkbd가 보낼 backspace가 IM filter로 재진입할 때
                        // unim 엔진을 거치지 않고 곧바로 앱에 전달되도록 사전 등록
                        this._inputMethod.expectSelfBackspaces(deleteChars);
                        // 1) 백스페이스로 기존 텍스트 삭제
                        this._vkbd.backspaceMultiple(deleteChars);
                        // 2) 딜레이 후 commit + preedit 설정
                        GLib.timeout_add(GLib.PRIORITY_HIGH, 50, () => {
                            if (commitText && commitText.length > 0) {
                                this._inputMethod.commitText(commitText);
                            }
                            // 3) preedit 설정 (마지막 음절을 조합 상태로)
                            if (preeditText && preeditText.length > 0) {
                                GLib.timeout_add(GLib.PRIORITY_HIGH, 10, () => {
                                    this._inputMethod.updatePreedit(preeditText);
                                    return GLib.SOURCE_REMOVE;
                                });
                            }
                            return GLib.SOURCE_REMOVE;
                        });
                    }
                },
            });

            // 7. 포커스 감시 시작
            this._focusWindowId = global.display.connect(
                'notify::focus-window',
                this._onFocusWindowChanged.bind(this)
            );
            // 7. 포커스 상실 핸들러 (조합 중 텍스트 커밋)
            this._inputMethod.setFocusOutHandler(() => {
                // PR #3 부터 emoji popup 은 모달 grab 을 가지지 않으므로 자체 focus
                // 가로채기가 없다 — 한자/특수 팝업과 동일하게 일반 팝업 cleanup 경로
                // 를 따른다.
                this._cleanupPopups();
                // DBus FocusOut → 조합 중 텍스트 커밋
                const commit = this._dbusIME.focusOut();
                if (commit && commit.length > 0) {
                    this._inputMethod.commitText(commit);
                    return true;
                }
                return false;
            });

            // content purpose 변경 핸들러 (password/pin 등 필드 진입 시 데몬에 통지 →
            // ATF·한영전환 억제, unim-dbus/src/service.rs:2625 SetContentType 계약).
            this._inputMethod.setContentTypeHandler(p => this._dbusIME.setContentType(p));

            // 8. 리셋 핸들러 (입력 필드 내 리셋 시 팝업 정리)
            this._inputMethod.setResetHandler(() => {
                this._cleanupPopups();
            });

            // 같은 윈도우 내 클릭 시 reset 트리거는 unim_input_method.js 의
            // vfunc_set_cursor_location 안 cursor-jump 감지 로직이 담당.
            // (이전엔 global.stage button-press hook 시도했으나 mutter compositor 가
            // user app window click 을 stage 에 안 전달해 fire 되지 않아 폐기.)

            this._inputMethod.setActive(true);
            unimLog('EXTENSION', 'IME 활성화 완료');
        } catch (e) {
            unimError('EXTENSION', `IME 활성화 실패: ${e.message}`);
            this._cleanupIME();
        }
    }

    /**
     * IME 비활성화
     */
    _disableIME() {
        this._cleanupIME();
        unimLog('EXTENSION', 'IME 비활성화');
    }

    /**
     * IME 자원 정리
     * @private
     */
    _cleanupIME() {
        // 인디케이터 비활성
        if (this._indicator) this._indicator.setInputActive(false);

        // (이모지 accelerator는 disable()이 정리 — IME 토글 시 유지)

        // 포커스 감시 해제
        if (this._focusWindowId > 0) {
            global.display.disconnect(this._focusWindowId);
            this._focusWindowId = 0;
        }

        // KeyHandler 정리
        if (this._keyHandler) {
            this._keyHandler.destroy();
            this._keyHandler = null;
        }

        // Preedit 오버레이 정리
        if (this._preeditOverlay) {
            this._preeditOverlay.disable();
            this._preeditOverlay = null;
        }

        // PopupView 정리 — GNOME Wayland 환경에서 extension 이 직접 렌더한 popup.
        // popup-service 측 dismiss 는 CancelHanja/CancelSpecialChar RPC 로 별도 처리됨.
        if (this._popupView) {
            this._popupView.destroy();
            this._popupView = null;
        }

        // ZWSP preedit 잔재 정리.
        if (this._emojiPopupActive && this._inputMethod) {
            this._inputMethod.updatePreedit('');
        }
        this._emojiPopupActive = false;

        // (DBus 연결 자체는 disable()이 정리 — IME 토글 시 유지하여 emoji 단축키 보존)

        // InputMethod Wrapper 정리
        if (this._inputMethod) {
            this._inputMethod.setActive(false);
            this._inputMethod = null;
        }
    }

    // ===========================================
    // 포커스 관리
    // ===========================================

    /**
     * 창 포커스 변경 시 호출
     * @private
     */
    _onFocusWindowChanged() {
        const focusWindow = global.display.focus_window;

        // popup-service 가 자체 outside-click dismiss 처리. focus null 케이스도 popup-service
        // 가 GTK4 grab 으로 직접 캡처하므로 GNOME extension 은 별도 분기 불필요.
        // 실제 창 전환 시 ZWSP preedit 잔재만 정리.
        this._cleanupPopups();

        if (!focusWindow) {
            // 포커스 없음 (바탕화면 등) → 인디케이터 비활성
            if (this._indicator) this._indicator.setInputActive(false);
            if (this._dbusIME?.isConnected) {
                const commit = this._dbusIME.focusOut();
                if (commit && this._inputMethod) {
                    this._inputMethod.commitText(commit);
                }
            }
            this._preeditOverlay?.hide();
            return;
        }

        const windowId = this._getWindowId(focusWindow);

        // 포커스 전환: focusOut → focusIn → 인디케이터 활성
        if (this._dbusIME?.isConnected) {
            const commit = this._dbusIME.focusOut();
            if (commit && this._inputMethod) {
                this._inputMethod.commitText(commit);
            }
            this._dbusIME.focusIn(windowId);
            if (this._indicator) this._indicator.setInputActive(true);
        }

        this._preeditOverlay?.hide();
    }

    /**
     * 포커스 손실·리셋 시 popup 정리. PopupView hide + popup-service 측 한자/특수 상태
     * cleanup RPC + emoji ZWSP preedit 잔재 제거.
     * @private
     */
    _cleanupPopups() {
        if (this._popupView) {
            this._popupView.hide();
        }
        if (this._dbusIME) {
            // popup-service forward 가 popup_state 비활성 시 no-op 처리하므로 idempotent.
            this._dbusIME.popupCancelHanja();
            this._dbusIME.popupCancelSpecial();
        }
        if (this._emojiPopupActive && this._inputMethod) {
            this._inputMethod.updatePreedit('');
        }
        this._emojiPopupActive = false;
    }

    /**
     * 현재 활성 창의 ID 생성
     * @returns {string}
     * @private
     */
    _getActiveWindowId() {
        const focusWindow = global.display.focus_window;
        return focusWindow ? this._getWindowId(focusWindow) : '';
    }

    /**
     * Meta.Window에서 고유 ID 생성
     * @param {Meta.Window} metaWindow
     * @returns {string}
     * @private
     */
    _getWindowId(metaWindow) {
        try {
            const wmClass = metaWindow.get_wm_class() || '';
            const stableSeq = metaWindow.get_stable_sequence?.() || 0;
            return `${wmClass}:${stableSeq}`;
        } catch (e) {
            return `unknown:${Date.now()}`;
        }
    }


    // ===========================================
    // 유틸리티
    // ===========================================

    /**
     * 한자키 keysym 목록을 설정에서 로드
     * @returns {number[]}
     * @private
     */
    _loadHanjaKeysyms() {
        // 기본값: Hangul_Hanja + F9
        const defaults = [Clutter.KEY_Hangul_Hanja, Clutter.KEY_F9];

        try {
            // daemon에서 설정 읽기
            if (this._dbusIME?.isConnected) {
                const hanjaKeysStr = this._dbusIME.getConfig('hanja_keys');
                if (hanjaKeysStr) {
                    // "Hanja,F9" 형태의 문자열 → keysym 배열로 변환
                    const keyNames = hanjaKeysStr.split(',').map(s => s.trim());
                    const keysyms = keyNames.map(name => {
                        const sym = Clutter[`KEY_${name}`] || Clutter[`KEY_Hangul_${name}`];
                        return sym;
                    }).filter(s => s !== undefined);

                    if (keysyms.length > 0) return keysyms;
                }
            }
        } catch (e) {
            unimLog('EXTENSION', `한자키 설정 로드 실패, 기본값 사용: ${e.message}`);
        }

        return defaults;
    }

    /**
     * 설정 변경 리스너 등록 (정리 자동화)
     * @private
     */
    _connectSettingChanged(key, callback) {
        const id = this._settings.connect(`changed::${key}`, callback);
        this._settingsChangedIds.push(id);
    }

    // ===========================================
    // 패널 인디케이터
    // ===========================================

    _addIndicator() {
        if (!this._indicator) {
            this._indicator = new UnimIndicator(this);
            Main.panel.addToStatusArea('unim', this._indicator);
        }
    }

    _removeIndicator() {
        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
    }

    // ===========================================
    // TypeFIX 기능 (기존 유지)
    // ===========================================

    // ===========================================
    // 이모지 팝업 (extension 은 단순 receiver)
    //
    // 트리거: 한자 키가 idle (preedit/조합 비어있음) 상태에서 눌릴 때.
    // 엔진의 press_key.rs Hanja 분기가 start_emoji_popup() 을 호출한다.
    // 별도 단축키 트리거는 제공하지 않는다.
    // ===========================================

    _bindAllShortcuts() {
        this._unbindAllShortcuts();
        this._bindShortcut('shortcut-normal', false);
        this._bindShortcut('shortcut-normal-reverse', true);
        this._bindRegisterUserDictShortcut();
    }

    _bindShortcut(settingKey, isReverse) {
        const shortcut = this._settings.get_strv(settingKey);
        if (!shortcut || shortcut.length === 0) return;

        Main.wm.addKeybinding(
            settingKey,
            this._settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.ALL,
            () => this._onShortcutTriggered(isReverse)
        );

        this._shortcutIds.push(settingKey);
    }

    _bindRegisterUserDictShortcut() {
        const settingKey = 'shortcut-register-userdict';
        const shortcut = this._settings.get_strv(settingKey);
        if (!shortcut || shortcut.length === 0) return;

        Main.wm.addKeybinding(
            settingKey,
            this._settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.ALL,
            () => this._onRegisterUserDictTriggered()
        );

        this._shortcutIds.push(settingKey);
    }

    _unbindAllShortcuts() {
        for (const id of this._shortcutIds) {
            Main.wm.removeKeybinding(id);
        }
        this._shortcutIds = [];
    }

    _onShortcutTriggered(isReverse) {
        if (!this._settings.get_boolean('enable-extension')) return;
        if (this._conversionInProgress) {
            unimLog('EXTENSION', 'TypeFIX: 이미 변환이 진행 중입니다. 무시합니다.');
            return;
        }

        // DBus TypeFix API 사용 (클립보드 미사용)
        // direction: 0=자동, 1=영→한, 2=한→영
        const direction = isReverse ? 2 : 0;

        // gedit/gnome-text-editor 호환: request_surrounding() 먼저 호출
        // 앱이 현재 선택 정보를 포함한 latest surrounding text를 보낼 때까지 대기
        if (this._inputMethod) {
            this._inputMethod.request_surrounding();
            // 앱 응답 대기 후 TypeFix 수행 (vfunc_set_surrounding 응답 시간)
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 50, () => {
                this._doTypeFix(direction);
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._doTypeFix(direction);
        }
    }

    _doTypeFix(direction) {
        this._conversionInProgress = true;
        try {
            const proxy = this._dbusIME?.getImProxy();
            if (!proxy) {
                unimLog('EXTENSION', 'TypeFIX: DBus 연결 없음');
                return;
            }

            // 글로벌 TypeFix 호출 — 선택된 텍스트만 변환
            const result = proxy.call_sync(
                'TypeFix',
                new GLib.Variant('(u)', [direction]),
                Gio.DBusCallFlags.NONE,
                500,
                null
            );

            if (!result) {
                unimLog('EXTENSION', 'TypeFIX: 변환할 텍스트 없음');
                return;
            }

            const [deleteOffset, deleteCount, replacement] = result.deep_unpack();

            if (deleteCount === 0 || !replacement) {
                unimLog('EXTENSION', 'TypeFIX: 변환할 텍스트 없음');
                return;
            }

            unimLog('EXTENSION', `TypeFIX 완료: offset=${deleteOffset}, delete=${deleteCount}, replacement='${replacement}'`);

            // 텍스트 치환: 정확한 위치에서 선택 텍스트 삭제 후 대체 텍스트 커밋
            if (this._inputMethod) {
                this._inputMethod.delete_surrounding(deleteOffset, deleteCount);
                this._inputMethod.commitText(replacement);
            }

            if (this._settings.get_boolean('show-notification')) {
                Main.notify(_('UNIM TypeFIX'), _('Conversion complete: %s').format(replacement));
            }
        } catch (e) {
            unimError('EXTENSION', `TypeFIX DBus 오류: ${e.message}`);
        } finally {
            this._conversionInProgress = false;
        }
    }

    // ===========================================
    // 사용자 사전 등록 단축키 (역방향 AutoTypeFix whitelist)
    // ===========================================

    _onRegisterUserDictTriggered() {
        if (!this._settings.get_boolean('enable-extension')) return;

        // TypeFix와 동일 패턴: request_surrounding() → 50ms 대기 → DBus 호출.
        if (this._inputMethod) {
            this._inputMethod.request_surrounding();
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 50, () => {
                this._doRegisterUserDict();
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._doRegisterUserDict();
        }
    }

    _doRegisterUserDict() {
        try {
            const proxy = this._dbusIME?.getImProxy();
            if (!proxy) {
                unimLog('EXTENSION', 'UserDict: DBus 연결 없음');
                return;
            }

            // 마지막 포커스 컨텍스트의 선택 영역을 daemon에서 읽어 등록
            const result = proxy.call_sync(
                'RegisterUserDictFromSelection',
                null,
                Gio.DBusCallFlags.NONE,
                500,
                null
            );

            if (!result) {
                unimLog('EXTENSION', 'UserDict: 등록 실패(응답 없음)');
                return;
            }

            const [word] = result.deep_unpack();

            if (!word) {
                if (this._settings.get_boolean('show-notification')) {
                    Main.notify(
                        _('UNIM Dictionary'),
                        _('Selection is empty, invalid, or already registered.')
                    );
                }
                return;
            }

            unimLog('EXTENSION', `UserDict 등록: '${word}'`);

            if (this._settings.get_boolean('show-notification')) {
                Main.notify(
                    _('UNIM Dictionary'),
                    _("Registered '%s' to the reverse user dictionary.").format(word)
                );
            }
        } catch (e) {
            unimError('EXTENSION', `UserDict DBus 오류: ${e.message}`);
        }
    }
}
