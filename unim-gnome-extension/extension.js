/**
 * UNIM Autocorrect - GNOME Shell Extension
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

// Paste mode
const PasteMode = {
    NORMAL: 'normal',       // Just paste
    TERMINAL: 'terminal',   // Backspace then paste
    COPY_ONLY: 'copy_only', // No paste, copy to clipboard only
};

export default class UnimAutocorrectExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._settings = null;
        this._shortcutIds = [];
        this._vkbd = null;
        this._clipboard = null;
    }

    enable() {
        console.log('[unim-autocorrect] Enabling hybrid extension...');
        try {
            this._settings = this.getSettings();
            this._clipboard = St.Clipboard.get_default();
            this._vkbd = new VirtualKeyboard();
            
            this._bindAllShortcuts();
            
            console.log('[unim-autocorrect] Hybrid extension enabled');
        } catch (e) {
            console.error(`[unim-autocorrect] Enable failed: ${e.message}`);
        }
    }

    disable() {
        this._unbindAllShortcuts();
        this._settings = null;
        this._vkbd = null;
        this._clipboard = null;
        console.log('[unim-autocorrect] Hybrid extension disabled');
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
        console.log(`[unim-autocorrect] Shortcut bound: ${settingKey} -> ${shortcut[0]} (paste: ${pasteMode}, reverse: ${isReverse})`);
    }

    _unbindAllShortcuts() {
        for (const id of this._shortcutIds) {
            Main.wm.removeKeybinding(id);
        }
        this._shortcutIds = [];
    }

    _onShortcutTriggered(pasteMode, isReverse) {
        if (!this._settings.get_boolean('enable-extension')) return;

        console.log(`[unim-autocorrect] Shortcut triggered: paste=${pasteMode}, reverse=${isReverse}`);

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
            console.error(`[unim-autocorrect] Conversion trigger error: ${e.message}`);
        }
    }

    async _processConvertedText(text, koreanLayout, englishLayout, pasteMode, isReverse) {
        console.log(`[unim-autocorrect] Transforming: "${text}" (paste: ${pasteMode}, reverse: ${isReverse})`);
        try {
            const converted = await this._convertText(text, koreanLayout, englishLayout, isReverse);
            if (!converted) return;
            
            console.log(`[unim-autocorrect] Result: "${converted}"`);
            
            // Set both selections for maximum compatibility
            this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
            this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);
            console.log('[unim-autocorrect] Clipboard updated');
            
            // Handle paste mode
            if (pasteMode === PasteMode.COPY_ONLY) {
                console.log('[unim-autocorrect] Copy-only mode: skipping paste');
            } else {
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
                    console.log('[unim-autocorrect] Triggering paste action...');
                    
                    if (pasteMode === PasteMode.TERMINAL) {
                        const deleteCount = text.length;
                        console.log(`[unim-autocorrect] Terminal mode: deleting ${deleteCount} chars before paste`);
                        this._vkbd.backspaceMultiple(deleteCount);
                    }
                    
                    this._vkbd.paste();
                    return GLib.SOURCE_REMOVE;
                });
            }
            
            if (this._settings.get_boolean('show-notification')) {
                Main.notify(_('UNIM Autocorrect'), _('Conversion complete: %s').format(converted));
            }
        } catch (e) {
            console.error(`[unim-autocorrect] Transform error: ${e.message}`);
        }
    }

    _convertText(text, koreanLayout, englishLayout, isReverse) {
        return new Promise((resolve, reject) => {
            const extensionPath = this.path;
            const binPath = GLib.build_filenamev([extensionPath, 'bin', 'unim-cli']);
            
            const kLayout = koreanLayout || '2bul';
            const eLayout = englishLayout || 'qwerty';

            const argv = [
                binPath,
                isReverse ? '--decompose' : '--compose',
                '--korean-keyboard', kLayout === '391' ? '391' : (kLayout === '390' ? '390' : '2bul'),
                '--english-keyboard', eLayout === 'dvorak' ? 'dvorak' : 'qwerty'
            ];

            console.log(`[unim-autocorrect] Executing: ${argv.join(' ')} with input: "${text}"`);

            try {
                const proc = new Gio.Subprocess({
                    argv: argv,
                    flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
                });
                proc.init(null);
                proc.communicate_utf8_async(text, null, (proc, res) => {
                    try {
                        const [ok, stdout, stderr] = proc.communicate_utf8_finish(res);
                        if (stderr) console.error(`[unim-autocorrect] CLI Stderr: ${stderr}`);
                        
                        const result = stdout ? stdout.trim() : '';
                        console.log(`[unim-autocorrect] CLI Stdout: "${result}"`);
                        resolve(result);
                    } catch (e) { 
                        console.error(`[unim-autocorrect] communicate_utf8_finish error: ${e.message}`);
                        reject(e); 
                    }
                });
            } catch (e) { 
                console.error(`[unim-autocorrect] Subprocess spawn error: ${e.message}`);
                reject(e); 
            }
        });
    }
}
