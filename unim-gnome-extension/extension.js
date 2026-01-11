/**
 * UNIM Autocorrect - GNOME Shell Extension
 * Hybrid Version: unim-cli + Native Shell APIs
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import Clutter from 'gi://Clutter';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension, gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';

import { VirtualKeyboard } from './vkbd.js';

export default class UnimAutocorrectExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._settings = null;
        this._shortcutId = null;
        this._vkbd = null;
        this._clipboard = null;
    }

    enable() {
        console.log('[unim-autocorrect] Enabling hybrid extension...');
        try {
            this._settings = this.getSettings();
            this._clipboard = St.Clipboard.get_default();
            this._vkbd = new VirtualKeyboard();
            
            this._bindShortcut();
            
            this._settingsChangedId = this._settings.connect(
                'changed::manual-conversion-shortcut',
                () => this._bindShortcut()
            );
            
            console.log('[unim-autocorrect] Hybrid extension enabled');
        } catch (e) {
            console.error(`[unim-autocorrect] Enable failed: ${e.message}`);
        }
    }

    disable() {
        this._unbindShortcut();
        if (this._settingsChangedId) {
            this._settings.disconnect(this._settingsChangedId);
            this._settingsChangedId = null;
        }
        this._settings = null;
        this._vkbd = null;
        this._clipboard = null;
        console.log('[unim-autocorrect] Hybrid extension disabled');
    }

    _bindShortcut() {
        this._unbindShortcut();
        const shortcut = this._settings.get_strv('manual-conversion-shortcut');
        if (!shortcut || shortcut.length === 0) return;
        
        Main.wm.addKeybinding(
            'manual-conversion-shortcut',
            this._settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.ALL,
            () => this._onConversionShortcut()
        );
        
        this._shortcutId = 'manual-conversion-shortcut';
        console.log(`[unim-autocorrect] Shortcut bound: ${shortcut[0]}`);
    }

    _unbindShortcut() {
        if (this._shortcutId) {
            Main.wm.removeKeybinding(this._shortcutId);
            this._shortcutId = null;
        }
    }

    _onConversionShortcut() {
        if (!this._settings.get_boolean('enable-extension')) return;
        
        const autoPaste = this._settings.get_boolean('auto-paste');
        const koreanLayout = this._settings.get_string('korean-layout');
        const englishLayout = this._settings.get_string('english-layout');
        
        this._doConversion(autoPaste, koreanLayout, englishLayout);
    }

    async _doConversion(autoPaste, koreanLayout, englishLayout) {
        try {
            // Primary Selection (Highlight)
            this._clipboard.get_text(St.ClipboardType.PRIMARY, (clipboard, text) => {
                if (!text || text.trim() === '') {
                    // Regular Clipboard fallback
                    this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (cb, cbText) => {
                        if (cbText) this._processConvertedText(cbText, autoPaste, koreanLayout, englishLayout);
                    });
                } else {
                    this._processConvertedText(text, autoPaste, koreanLayout, englishLayout);
                }
            });
        } catch (e) {
            console.error(`[unim-autocorrect] Conversion trigger error: ${e.message}`);
        }
    }

    async _processConvertedText(text, autoPaste, koreanLayout, englishLayout) {
        console.log(`[unim-autocorrect] Transforming: "${text}"`);
        try {
            const converted = await this._convertText(text, koreanLayout, englishLayout);
            if (!converted) return;
            
            console.log(`[unim-autocorrect] Result: "${converted}"`);
            
            // Set both selections for maximum compatibility
            this._clipboard.set_text(St.ClipboardType.CLIPBOARD, converted);
            this._clipboard.set_text(St.ClipboardType.PRIMARY, converted);
            console.log('[unim-autocorrect] Clipboard updated');
            
            if (autoPaste) {
                // Increase timeout to give clipboard time to settle
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
                    console.log('[unim-autocorrect] Triggering auto-paste...');
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

    _convertText(text, koreanLayout, englishLayout) {
        return new Promise((resolve, reject) => {
            const extensionPath = this.path;
            const binPath = GLib.build_filenamev([extensionPath, 'bin', 'unim-cli']);
            
            const kLayout = koreanLayout || '2bul';
            const eLayout = englishLayout || 'qwerty';

            const argv = [
                binPath,
                '--compose',
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
