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
import { UnimInputMethod } from './unim_input_method.js';
import { KeyHandler } from './key_handler.js';
import { PreeditOverlay } from './preedit_overlay.js';
import { HanjaPopup } from './hanja_popup.js';
import { SpecialPopup } from './special_popup.js';
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
        this._hanjaPopup = null;
        this._specialPopup = null;

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

            // 패널 인디케이터
            if (this._settings.get_boolean('show-panel-indicator')) {
                this._addIndicator();
            }

            // 설정 변경 리스너
            this._connectSettingChanged('show-panel-indicator', () => {
                const show = this._settings.get_boolean('show-panel-indicator');
                if (show && !this._indicator) this._addIndicator();
                else if (!show && this._indicator) this._removeIndicator();
            });

            // TypeFIX 단축키
            this._bindAllShortcuts();

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

            // 2. DBus 연결
            this._dbusIME = new UnimDbusIME();
            this._inputMethod.setDbusIME(this._dbusIME);
            
            const windowId = this._getActiveWindowId();
            const connected = this._dbusIME.connect(windowId, (isKorean) => {
                // GlobalModeChanged → 인디케이터 업데이트
                if (this._indicator) {
                    this._indicator._onModeChanged(isKorean);
                }
            });

            if (!connected) {
                unimError('EXTENSION', 'unim-daemon DBus 연결 실패 — IME 비활성');
                this._cleanupIME();
                return;
            }

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

            // 6. 한자/특수문자 팝업 초기화
            this._hanjaPopup = new HanjaPopup();
            this._hanjaPopup.enable();
            this._specialPopup = new SpecialPopup();
            this._specialPopup.enable();

            // DBus 팝업 시그널 콜백 등록
            this._dbusIME.setPopupCallbacks({
                onShowHanja: (target, candidates, cursorRect) => {
                    this._hanjaPopup.show(
                        target, candidates,
                        (globalIdx) => {
                            // 선택 콜백: SelectHanja → 한자 반환 → 커밋
                            unimLog('HANJA', `선택 콜백: globalIdx=${globalIdx}, _hasFocus=${this._inputMethod?._hasFocus}`);
                            const hanja = this._dbusIME.selectHanja(globalIdx);
                            unimLog('HANJA', `selectHanja 반환: '${hanja}', _hasFocus=${this._inputMethod?._hasFocus}`);
                            if (hanja && this._inputMethod) {
                                // preedit 클리어 후 한자 커밋
                                this._inputMethod.updatePreedit('');
                                this._inputMethod.commitText(hanja);
                            }
                        },
                        () => {
                            // 취소 콜백 (마우스 클릭 취소용)
                            const commit = this._dbusIME.cancelHanja();
                            if (commit && this._inputMethod) {
                                this._inputMethod.commitText(commit);
                                this._inputMethod.updatePreedit('');
                            }
                        },
                        cursorRect
                    );
                },
                onShowSpecial: (target, characters, topRow, cursorRect) => {
                    this._specialPopup.show(
                        target, characters, topRow,
                        (globalIdx) => {
                            // 선택 콜백: SelectSpecialChar → 특수문자 반환 → 커밋
                            unimLog('SPECIAL', `선택 콜백: globalIdx=${globalIdx}, _hasFocus=${this._inputMethod?._hasFocus}`);
                            const ch = this._dbusIME.selectSpecialChar(globalIdx);
                            unimLog('SPECIAL', `selectSpecialChar 반환: '${ch}', _hasFocus=${this._inputMethod?._hasFocus}`);
                            if (ch && this._inputMethod) {
                                this._inputMethod.updatePreedit('');
                                this._inputMethod.commitText(ch);
                            }
                        },
                        () => {
                            // 취소 콜백 (마우스 클릭 취소용)
                            const commit = this._dbusIME.cancelSpecialChar();
                            if (commit && this._inputMethod) {
                                this._inputMethod.commitText(commit);
                                this._inputMethod.updatePreedit('');
                            }
                        },
                        cursorRect
                    );
                },
                onHidePopup: () => {
                    this._hanjaPopup?.hide();
                    this._specialPopup?.hide();
                },
                onPopupNavigate: (page, totalPages, selected, rows, cols, selRow, selCol) => {
                    if (this._hanjaPopup?.isVisible) {
                        this._hanjaPopup.updateFromNavigate(page, totalPages, selected);
                    }
                    if (this._specialPopup?.isVisible) {
                        this._specialPopup.updateFromNavigate(page, totalPages, rows, cols, selRow, selCol);
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
                // 팝업 열려있으면 먼저 취소 (trigger text 커밋 + 팝업 닫기)
                this._cleanupPopups();
                // DBus FocusOut → 조합 중 텍스트 커밋
                const commit = this._dbusIME.focusOut();
                if (commit && commit.length > 0) {
                    this._inputMethod.commitText(commit);
                    return true;
                }
                return false;
            });

            // 8. 리셋 핸들러 (입력 필드 내 리셋 시 팝업 정리)
            this._inputMethod.setResetHandler(() => {
                this._cleanupPopups();
            });

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

        // 팝업 정리
        if (this._hanjaPopup) {
            this._hanjaPopup.hide();
            this._hanjaPopup.disable();
            this._hanjaPopup = null;
        }
        if (this._specialPopup) {
            this._specialPopup.hide();
            this._specialPopup.disable();
            this._specialPopup = null;
        }

        // DBus 정리
        if (this._dbusIME) {
            this._dbusIME.destroy();
            this._dbusIME = null;
        }

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

        // 팝업이 열려있고 포커스가 null(Chrome 위젯 클릭 등)이면
        // 마우스 클릭 이벤트가 먼저 처리되도록 지연
        const popupVisible = this._hanjaPopup?.isVisible || this._specialPopup?.isVisible;
        if (popupVisible && !focusWindow) {
            return; // Chrome 위젯 클릭 — 팝업 유지, 클릭 핸들러에 맡김
        }

        // 실제 창 전환 시 팝업 닫기 + 엔진 모드 취소
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
     * 열려있는 한자/특수문자 팝업 정리
     * @private
     */
    _cleanupPopups() {
        if (this._hanjaPopup?.isVisible) {
            this._hanjaPopup.hide();
            const trigger = this._dbusIME?.cancelHanja();
            if (trigger && this._inputMethod) {
                this._inputMethod.commitText(trigger);
            }
        }
        if (this._specialPopup?.isVisible) {
            this._specialPopup.hide();
            const trigger = this._dbusIME?.cancelSpecialChar();
            if (trigger && this._inputMethod) {
                this._inputMethod.commitText(trigger);
            }
        }
        // 팝업 키 핸들러는 더 이상 사용하지 않음 (ProcessKeyEvent 경로로 통일)
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

    _bindAllShortcuts() {
        this._unbindAllShortcuts();
        this._bindShortcut('shortcut-normal', false);
        this._bindShortcut('shortcut-normal-reverse', true);
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
        this._doTypeFix(direction);
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
}
