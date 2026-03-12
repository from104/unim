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

        // TypeFIX 상태
        this._conversionInProgress = false;
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
                            this._keyHandler.setPopupKeyHandler(null);
                        },
                        () => {
                            // 취소 콜백 (ESC/미지원키): 원본 한글 커밋 → cancel
                            // GTK3/4/Qt5/6와 동일 패턴: focusOut → 원본 커밋 → cancelHanja
                            const commit = this._dbusIME.focusOut();
                            if (commit && this._inputMethod) {
                                this._inputMethod.commitText(commit);
                            }
                            this._dbusIME.cancelHanja();
                            if (this._inputMethod) {
                                this._inputMethod.updatePreedit('');
                            }
                            // FocusIn 복원 (FocusOut 후 필요)
                            const windowId = this._getActiveWindowId();
                            this._dbusIME.focusIn(windowId);
                            this._keyHandler.setPopupKeyHandler(null);
                        },
                        cursorRect
                    );
                    // 키 이벤트를 팝업으로 위임
                    this._keyHandler.setPopupKeyHandler((keyval) => {
                        return this._hanjaPopup.handleKey(keyval);
                    });
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
                            this._keyHandler.setPopupKeyHandler(null);
                        },
                        () => {
                            // 취소 콜백: 원본 초성 커밋 → cancel
                            const commit = this._dbusIME.focusOut();
                            if (commit && this._inputMethod) {
                                this._inputMethod.commitText(commit);
                            }
                            this._dbusIME.cancelSpecialChar();
                            if (this._inputMethod) {
                                this._inputMethod.updatePreedit('');
                            }
                            const windowId = this._getActiveWindowId();
                            this._dbusIME.focusIn(windowId);
                            this._keyHandler.setPopupKeyHandler(null);
                        },
                        cursorRect
                    );
                    this._keyHandler.setPopupKeyHandler((keyval) => {
                        return this._specialPopup.handleKey(keyval);
                    });
                },
                onHidePopup: () => {
                    this._hanjaPopup.hide();
                    this._specialPopup.hide();
                    this._keyHandler.setPopupKeyHandler(null);
                },
            });

            // 7. 포커스 감시 시작
            this._focusWindowId = global.display.connect(
                'notify::focus-window',
                this._onFocusWindowChanged.bind(this)
            );
            // 7. 포커스 상실 핸들러 (조합 중 텍스트 커밋)
            this._inputMethod.setFocusOutHandler(() => {
                // DBus FocusOut → 조합 중 텍스트 커밋
                const commit = this._dbusIME.focusOut();
                if (commit && commit.length > 0) {
                    this._inputMethod.commitText(commit);
                    return true;
                }
                return false;
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
        if (this._conversionInProgress) {
            unimLog('EXTENSION', 'TypeFIX: 이미 변환이 진행 중입니다. 무시합니다.');
            return;
        }

        const koreanLayout = this._settings.get_string('korean-layout');
        const englishLayout = this._settings.get_string('english-layout');
        this._doConversion(koreanLayout, englishLayout, pasteMode, isReverse);
    }

    async _doConversion(koreanLayout, englishLayout, pasteMode, isReverse) {
        this._conversionInProgress = true;
        try {
            // PRIMARY (Selection) 우선 시도, 없으면 CLIPBOARD 시도
            this._clipboard.get_text(St.ClipboardType.PRIMARY, (clipboard, text) => {
                if (!text || text.trim() === '') {
                    this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (cb, cbText) => {
                        if (cbText && cbText.trim() !== '') {
                            this._processConvertedText(cbText, koreanLayout, englishLayout, pasteMode, isReverse);
                        } else {
                            this._conversionInProgress = false;
                        }
                    });
                } else {
                    this._processConvertedText(text, koreanLayout, englishLayout, pasteMode, isReverse);
                }
            });
        } catch (e) {
            unimError('EXTENSION', `Conversion trigger error: ${e.message}`);
            this._conversionInProgress = false;
        }
    }

    async _processConvertedText(text, koreanLayout, englishLayout, pasteMode, isReverse) {
        try {
            const converted = await this._convertText(text, koreanLayout, englishLayout, isReverse);
            if (!converted || converted === text) {
                unimLog('EXTENSION', 'TypeFIX: 변환 결과가 없거나 원본과 동일함');
                this._conversionInProgress = false;
                return;
            }

            if (pasteMode === PasteMode.COPY_ONLY) {
                this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
                this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);
                unimLog('EXTENSION', 'Copy-only mode: 붙여넣기 생략');
                this._conversionInProgress = false;
            } else {
                // 클립보드 백업 후 변환 텍스트 설정 → 붙여넣기 → 복원
                this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_cb, savedClipboard) => {
                    this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
                    this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);

                    // 붙여넣기 전에 약간의 지연 (클립보드 동기화 대기)
                    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, () => {
                        if (pasteMode === PasteMode.TERMINAL) {
                            this._vkbd.backspaceMultiple(text.length);
                        }
                        this._vkbd.paste();

                        // 붙여넣기 완료 후 클립보드 복원
                        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
                            if (savedClipboard && savedClipboard.trim() !== '') {
                                this._clipboard.set_text(St.ClipboardType.CLIPBOARD, savedClipboard);
                                unimLog('EXTENSION', 'TypeFIX: 클립보드 복원 완료');
                            }
                            this._conversionInProgress = false;
                            return GLib.SOURCE_REMOVE;
                        });
                        return GLib.SOURCE_REMOVE;
                    });
                });
            }

            if (this._settings.get_boolean('show-notification')) {
                Main.notify(_('UNIM TypeFIX'), _('Conversion complete: %s').format(converted));
            }
        } catch (e) {
            unimError('EXTENSION', `Transform error: ${e.message}`);
            this._conversionInProgress = false;
        }
    }

    _convertText(text, koreanLayout, englishLayout, isReverse) {
        return new Promise((resolve, reject) => {
            const binPath = '/usr/bin/unim-cli';
            
            // 레이아웃 결정: 우선순위 1.인자, 2.설정, 3.기본값
            const kLayout = koreanLayout || this._settings.get_string('korean-layout') || '2bul';
            const eLayout = englishLayout || this._settings.get_string('english-layout') || 'qwerty';

            const koreanLayoutMap = {
                '2bul': '2bul', 
                '3bul390': '390', 
                '3bul391': '391',
                '3bul_noshift': 'noshift', 
                '390': '390', 
                '391': '391',
                'noshift': 'noshift',
            };
            const englishLayoutMap = {
                'qwerty': 'qwerty', 
                'dvorak': 'dvorak', 
                'colemak': 'colemak',
                'colemak_dh': 'colemak_dh', 
                'workman': 'workman',
            };

            const koKey = koreanLayoutMap[kLayout] || '2bul';
            const enKey = englishLayoutMap[eLayout] || 'qwerty';

            const argv = [
                binPath,
                isReverse ? '--decompose' : '--compose',
                '--korean-keyboard', koKey,
                '--english-keyboard', enKey,
            ];

            unimLog('EXTENSION', `TypeFIX 실행: ${argv.join(' ')} (input: ${text.substring(0, 10)}${text.length > 10 ? '...' : ''})`);

            try {
                const proc = new Gio.Subprocess({
                    argv,
                    flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE,
                });
                proc.init(null);
                proc.communicate_utf8_async(text, null, (p, res) => {
                    try {
                        const [ok, stdout, stderr] = p.communicate_utf8_finish(res);
                        if (!ok) {
                            unimError('EXTENSION', `unim-cli 실패: ${stderr}`);
                            resolve('');
                            return;
                        }
                        resolve(stdout ? stdout.trim() : '');
                    } catch (e) { 
                        unimError('EXTENSION', `unim-cli 결과 처리 오류: ${e.message}`);
                        reject(e); 
                    }
                });
            } catch (e) { 
                unimError('EXTENSION', `unim-cli 프로세스 시작 실패: ${e.message}`);
                reject(e); 
            }
        });
    }
}
