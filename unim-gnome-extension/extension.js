/**
 * UNIM Indicator GNOME Shell Extension
 * Hybrid Version: unim-cli + Native Shell APIs
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension, gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';

import { VirtualKeyboard } from './vkbd.js';
import { UnimIndicator } from './indicator.js';
import { unimLog, unimError } from './logging.js';

// Paste mode
const PasteMode = {
    NORMAL: 'normal',       // Just paste
    TERMINAL: 'terminal',   // Backspace then paste
    COPY_ONLY: 'copy_only', // No paste, copy to clipboard only
};

export default class UnimTypefixExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._settings = null;
        this._shortcutIds = [];
        this._vkbd = null;
        this._clipboard = null;
        this._indicator = null;
    }

    enable() {
        unimLog('EXTENSION', ' Enabling hybrid extension...');
        try {
            this._settings = this.getSettings();
            this._clipboard = St.Clipboard.get_default();
            this._vkbd = new VirtualKeyboard();
            
            // 패널 인디케이터 추가
            if (this._settings.get_boolean('show-panel-indicator')) {
                this._addIndicator();
            }
            
            // 설정 변경 리스너 (즉시 반영)
            this._settingsChangedId = this._settings.connect('changed::show-panel-indicator', () => {
                const showIndicator = this._settings.get_boolean('show-panel-indicator');
                if (showIndicator && !this._indicator) {
                    this._addIndicator();
                    unimLog('EXTENSION', ' Panel indicator added');
                } else if (!showIndicator && this._indicator) {
                    this._removeIndicator();
                    unimLog('EXTENSION', ' Panel indicator removed');
                }
            });
            
            this._bindAllShortcuts();
            
            unimLog('EXTENSION', ' Hybrid extension enabled');
        } catch (e) {
            unimError('EXTENSION', `Enable failed: ${e.message}`);
        }
    }
    
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

    disable() {
        this._unbindAllShortcuts();
        
        // 설정 변경 리스너 정리
        if (this._settings && this._settingsChangedId) {
            this._settings.disconnect(this._settingsChangedId);
            this._settingsChangedId = 0;
        }
        
        this._removeIndicator();
        
        this._settings = null;
        this._vkbd = null;
        this._clipboard = null;
        unimLog('EXTENSION', ' Hybrid extension disabled');
    }

    _bindAllShortcuts() {
        this._unbindAllShortcuts();

        // 6 shortcut combinations:
        // isReverse: true = Korean to English (decompose), false = English to Korean (compose)
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
        unimLog('EXTENSION', `Shortcut bound: ${settingKey} -> ${shortcut[0]} (paste: ${pasteMode}, reverse: ${isReverse})`);
    }

    _unbindAllShortcuts() {
        for (const id of this._shortcutIds) {
            Main.wm.removeKeybinding(id);
        }
        this._shortcutIds = [];
    }

    _onShortcutTriggered(pasteMode, isReverse) {
        if (!this._settings.get_boolean('enable-extension')) return;

        unimLog('EXTENSION', `Shortcut triggered: paste=${pasteMode}, reverse=${isReverse}`);

        const koreanLayout = this._settings.get_string('korean-layout');
        const englishLayout = this._settings.get_string('english-layout');
        
        this._doConversion(koreanLayout, englishLayout, pasteMode, isReverse);
    }

    async _doConversion(koreanLayout, englishLayout, pasteMode, isReverse) {
        try {
            // Primary Selection (Highlight)
            this._clipboard.get_text(St.ClipboardType.PRIMARY, (clipboard, text) => {
                if (!text || text.trim() === '') {
                    // Regular Clipboard fallback
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
        unimLog('EXTENSION', `Transforming: "${text}" (paste: ${pasteMode}, reverse: ${isReverse})`);
        try {
            const converted = await this._convertText(text, koreanLayout, englishLayout, isReverse);
            if (!converted) return;
            
            unimLog('EXTENSION', `Result: "${converted}"`);
            
            // Set both selections for maximum compatibility
            this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
            this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);
            unimLog('EXTENSION', ' Clipboard updated');
            
            // Handle paste mode
            if (pasteMode === PasteMode.COPY_ONLY) {
                unimLog('EXTENSION', ' Copy-only mode: skipping paste');
            } else {
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
                    unimLog('EXTENSION', ' Triggering paste action...');
                    
                    if (pasteMode === PasteMode.TERMINAL) {
                        const deleteCount = text.length;
                        unimLog('EXTENSION', `Terminal mode: deleting ${deleteCount} chars before paste`);
                        this._vkbd.backspaceMultiple(deleteCount);
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
            const extensionPath = this.path;
            const binPath = '/usr/bin/unim-cli';
            
            // GSettings에서 레이아웃 읽기
            const kLayout = koreanLayout || this._settings.get_string('korean-layout') || '2bul';
            const eLayout = englishLayout || this._settings.get_string('english-layout') || 'qwerty';

            // 레이아웃 값을 unim-cli 옵션으로 매핑
            const koreanLayoutMap = {
                '2bul': '2bul',
                '3bul390': '390',
                '3bul391': '391',
                '3bul_noshift': 'noshift',
                // 호환성: 기존 값도 지원
                '390': '390',
                '391': '391'
            };
            
            const englishLayoutMap = {
                'qwerty': 'qwerty',
                'dvorak': 'dvorak',
                'colemak': 'colemak',
                'colemak_dh': 'colemak-dh',
                'workman': 'workman'
            };

            const argv = [
                binPath,
                isReverse ? '--decompose' : '--compose',
                '--korean-keyboard', koreanLayoutMap[kLayout] || '2bul',
                '--english-keyboard', englishLayoutMap[eLayout] || 'qwerty'
            ];

            unimLog('EXTENSION', `Executing: ${argv.join(' ')} with input: "${text}"`);

            try {
                const proc = new Gio.Subprocess({
                    argv: argv,
                    flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
                });
                proc.init(null);
                proc.communicate_utf8_async(text, null, (proc, res) => {
                    try {
                        const [ok, stdout, stderr] = proc.communicate_utf8_finish(res);
                        if (stderr) unimError('EXTENSION', `CLI Stderr: ${stderr}`);
                        
                        const result = stdout ? stdout.trim() : '';
                        unimLog('EXTENSION', `CLI Stdout: "${result}"`);
                        resolve(result);
                    } catch (e) { 
                        unimError('EXTENSION', `communicate_utf8_finish error: ${e.message}`);
                        reject(e); 
                    }
                });
            } catch (e) { 
                unimError('EXTENSION', `Subprocess spawn error: ${e.message}`);
                reject(e); 
            }
        });
    }
}
