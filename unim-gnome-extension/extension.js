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
import St from 'gi://St';
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

// TypeFIX paste modes
const PasteMode = {
    NORMAL: 'normal',
    TERMINAL: 'terminal',
    COPY_ONLY: 'copy_only',
};

export default class UnimExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._settings = null;
        this._shortcutIds = [];
        this._vkbd = null;
        this._clipboard = null;
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
    }

    enable() {
        unimLog('EXTENSION', 'Extension 활성화 시작...');
        try {
            this._settings = this.getSettings();
            this._clipboard = St.Clipboard.get_default();
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
        this._clipboard = null;
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

            // 3. 한자키 설정 읽기
            const hanjaKeysyms = this._loadHanjaKeysyms();

            // 4. KeyHandler 시작
            this._keyHandler = new KeyHandler(this._dbusIME, this._inputMethod, {
                hanjaKeysyms,
                onHanjaRequest: () => this._onHanjaRequest(),
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

            // 7. 포커스 감시 시작
            this._focusWindowId = global.display.connect(
                'notify::focus-window',
                this._onFocusWindowChanged.bind(this)
            );
            // 8. 포커스 상실 핸들러 (조합 중 텍스트 커밋 + 팝업 닫기)
            this._inputMethod.setFocusOutHandler(() => {
                // 팝업이 열려있으면 닫기
                if (this._hanjaPopup?.isVisible) {
                    this._dbusIME.cancelHanja();
                    this._hanjaPopup.hide();
                    this._keyHandler?.setPopupKeyHandler(null);
                }
                if (this._specialPopup?.isVisible) {
                    this._dbusIME.cancelSpecialChar();
                    this._specialPopup.hide();
                    this._keyHandler?.setPopupKeyHandler(null);
                }

                // DBus FocusOut → 조합 중 텍스트 커밋
                const commit = this._dbusIME.focusOut();
                if (commit && commit.length > 0) {
                    this._inputMethod.commitText(commit);
                }
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
            this._hanjaPopup.disable();
            this._hanjaPopup = null;
        }
        if (this._specialPopup) {
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

        if (!focusWindow) {
            // 포커스 없음 (바탕화면 등)
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

        // 포커스 전환: focusOut → focusIn
        if (this._dbusIME?.isConnected) {
            const commit = this._dbusIME.focusOut();
            if (commit && this._inputMethod) {
                this._inputMethod.commitText(commit);
            }
            this._dbusIME.focusIn(windowId);
        }

        this._preeditOverlay?.hide();
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
    // 한자/특수문자 처리
    // ===========================================

    /**
     * 한자키 요청 처리
     *
     * GTK3 immodule.c의 한자키 플로우를 따름:
     * 1. GetHanjaCandidates 시도
     * 2. 후보 없으면 GetSpecialCharCandidates 폴백
     * @private
     */
    _onHanjaRequest() {
        if (!this._dbusIME?.isConnected) return;

        // 팝업이 이미 열려있으면 닫기
        if (this._hanjaPopup?.isVisible) {
            this._hanjaPopup.hide();
            this._dbusIME.cancelHanja();
            this._keyHandler.setPopupKeyHandler(null);
            return;
        }
        if (this._specialPopup?.isVisible) {
            this._specialPopup.hide();
            this._dbusIME.cancelSpecialChar();
            this._keyHandler.setPopupKeyHandler(null);
            return;
        }

        // 한자 후보 조회
        const hanjaResult = this._dbusIME.getHanjaCandidates();

        if (hanjaResult && hanjaResult.candidates.length > 0) {
            this._hanjaPopup.show(
                hanjaResult.target,
                hanjaResult.candidates,
                (globalIndex) => {
                    // 선택 콜백
                    const selected = this._dbusIME.selectHanja(globalIndex);
                    if (selected && this._inputMethod) {
                        this._inputMethod.commitText(selected);
                    }
                    this._keyHandler.setPopupKeyHandler(null);
                },
                () => {
                    // 취소 콜백
                    this._dbusIME.cancelHanja();
                    this._keyHandler.setPopupKeyHandler(null);
                },
                this._inputMethod?.cursorRect
            );

            // KeyHandler에 팝업 키 핸들러 등록
            this._keyHandler.setPopupKeyHandler(
                (keyval) => this._hanjaPopup.handleKey(keyval)
            );
            return;
        }

        // 특수문자 후보 폴백
        const specialResult = this._dbusIME.getSpecialCharCandidates();

        if (specialResult && specialResult.characters.length > 0) {
            this._specialPopup.show(
                specialResult.target,
                specialResult.characters,
                specialResult.topRow,
                (globalIndex) => {
                    // 선택 콜백
                    const selected = this._dbusIME.selectSpecialChar(globalIndex);
                    if (selected && this._inputMethod) {
                        this._inputMethod.commitText(selected);
                    }
                    this._keyHandler.setPopupKeyHandler(null);
                },
                () => {
                    // 취소 콜백
                    this._dbusIME.cancelSpecialChar();
                    this._keyHandler.setPopupKeyHandler(null);
                },
                this._inputMethod?.cursorRect
            );

            this._keyHandler.setPopupKeyHandler(
                (keyval) => this._specialPopup.handleKey(keyval)
            );
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
            Main.panel.addToStatusArea('unim-indicator', this._indicator);
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
        this._bindShortcut('shortcut-normal', PasteMode.NORMAL, false);
        this._bindShortcut('shortcut-normal-reverse', PasteMode.NORMAL, true);
        this._bindShortcut('shortcut-terminal', PasteMode.TERMINAL, false);
        this._bindShortcut('shortcut-terminal-reverse', PasteMode.TERMINAL, true);
        this._bindShortcut('shortcut-copy-only', PasteMode.COPY_ONLY, false);
        this._bindShortcut('shortcut-copy-only-reverse', PasteMode.COPY_ONLY, true);
    }

    _bindShortcut(settingKey, pasteMode, isReverse) {
        const shortcut = this._settings.get_strv(settingKey);
        if (!shortcut || shortcut.length === 0) return;

        Main.wm.addKeybinding(
            settingKey,
            this._settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.ALL,
            () => this._onShortcutTriggered(pasteMode, isReverse)
        );

        this._shortcutIds.push(settingKey);
    }

    _unbindAllShortcuts() {
        for (const id of this._shortcutIds) {
            Main.wm.removeKeybinding(id);
        }
        this._shortcutIds = [];
    }

    _onShortcutTriggered(pasteMode, isReverse) {
        if (!this._settings.get_boolean('enable-extension')) return;

        const koreanLayout = this._settings.get_string('korean-layout');
        const englishLayout = this._settings.get_string('english-layout');
        this._doConversion(koreanLayout, englishLayout, pasteMode, isReverse);
    }

    async _doConversion(koreanLayout, englishLayout, pasteMode, isReverse) {
        try {
            this._clipboard.get_text(St.ClipboardType.PRIMARY, (clipboard, text) => {
                if (!text || text.trim() === '') {
                    this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (cb, cbText) => {
                        if (cbText) this._processConvertedText(cbText, koreanLayout, englishLayout, pasteMode, isReverse);
                    });
                } else {
                    this._processConvertedText(text, koreanLayout, englishLayout, pasteMode, isReverse);
                }
            });
        } catch (e) {
            unimError('EXTENSION', `Conversion trigger error: ${e.message}`);
        }
    }

    async _processConvertedText(text, koreanLayout, englishLayout, pasteMode, isReverse) {
        try {
            const converted = await this._convertText(text, koreanLayout, englishLayout, isReverse);
            if (!converted) return;

            this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
            this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);

            if (pasteMode === PasteMode.COPY_ONLY) {
                unimLog('EXTENSION', 'Copy-only mode: 붙여넣기 생략');
            } else {
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
                    if (pasteMode === PasteMode.TERMINAL) {
                        this._vkbd.backspaceMultiple(text.length);
                    }
                    this._vkbd.paste();
                    return GLib.SOURCE_REMOVE;
                });
            }

            if (this._settings.get_boolean('show-notification')) {
                Main.notify(_('UNIM TypeFIX'), _('Conversion complete: %s').format(converted));
            }
        } catch (e) {
            unimError('EXTENSION', `Transform error: ${e.message}`);
        }
    }

    _convertText(text, koreanLayout, englishLayout, isReverse) {
        return new Promise((resolve, reject) => {
            const binPath = '/usr/bin/unim-cli';
            const kLayout = koreanLayout || this._settings.get_string('korean-layout') || '2bul';
            const eLayout = englishLayout || this._settings.get_string('english-layout') || 'qwerty';

            const koreanLayoutMap = {
                '2bul': '2bul', '3bul390': '390', '3bul391': '391',
                '3bul_noshift': 'noshift', '390': '390', '391': '391',
            };
            const englishLayoutMap = {
                'qwerty': 'qwerty', 'dvorak': 'dvorak', 'colemak': 'colemak',
                'colemak_dh': 'colemak-dh', 'workman': 'workman',
            };

            const argv = [
                binPath,
                isReverse ? '--decompose' : '--compose',
                '--korean-keyboard', koreanLayoutMap[kLayout] || '2bul',
                '--english-keyboard', englishLayoutMap[eLayout] || 'qwerty',
            ];

            try {
                const proc = new Gio.Subprocess({
                    argv,
                    flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE,
                });
                proc.init(null);
                proc.communicate_utf8_async(text, null, (proc, res) => {
                    try {
                        const [ok, stdout, stderr] = proc.communicate_utf8_finish(res);
                        resolve(stdout ? stdout.trim() : '');
                    } catch (e) { reject(e); }
                });
            } catch (e) { reject(e); }
        });
    }
}
