import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import UnimLib from './unimlib.js';


const Indicator = GObject.registerClass(
class Indicator extends PanelMenu.Button {
    _init(extension) {
        super._init(0.0, extension.metadata.name, false);
        
        this._extension = extension;
        this._settings = extension.getSettings();

        // UI Setup
        this._icon = new St.Icon({
            icon_name: 'input-keyboard-symbolic',
            style_class: 'system-status-icon',
        });
        this.add_child(this._icon);

        this._buildMenu();


        this._settings.connect('changed::show-indicator', () => {
            this.visible = this._settings.get_boolean('show-indicator');
        });
        this.visible = this._settings.get_boolean('show-indicator');

        this._updateStatus();
    }

    _buildMenu() {
        const titleItem = new PopupMenu.PopupMenuItem('Unim IME Control');
        titleItem.sensitive = false;
        this.menu.addMenuItem(titleItem);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        const settingsItem = new PopupMenu.PopupMenuItem('Settings');
        settingsItem.connect('activate', () => this._extension.openPreferences());
        this.menu.addMenuItem(settingsItem);
    }

    _updateStatus() {
        this._updateIcon();
    }

    _updateIcon() {
        // Since we are not tracking engine, we just use a static icon or simple style
        this._icon.style_class = 'system-status-icon';
    }
});

export default class UnimExtension extends Extension {
    enable() {
        console.log('[unim] Enabling extension...');
        this._settings = this.getSettings();
        
        // Setup Virtual Keyboard for automation
        try {
            const seat = Clutter.get_default_backend().get_default_seat();
            this._virtualKeyboard = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        } catch (e) {
            console.error(`[unim] Failed to create virtual keyboard: ${e.message}`);
        }

        // Initialize Rust Core Library (CLI binary)
        const libPath = this.dir.get_child('lib').get_child('libunim_core.so').get_path();
        this._unimLib = new UnimLib(libPath);
        if (this._unimLib.init()) {
            console.log('[unim] Rust core library initialized');
        } else {
            console.error('[unim] Failed to initialize Rust core library');
        }

        this._indicator = new Indicator(this);
        Main.panel.addToStatusArea(this.uuid, this._indicator);

        this._initKeybinding();
        
        // Settings listener for manual conversion shortcut changes
        this._settingsId = this._settings.connect('changed::enable-manual-conversion', () => {
            this._removeKeybinding();
            this._initKeybinding();
        });
    }

    disable() {
        console.log('[unim] Disabling extension...');
        
        if (this._settingsId) {
            this._settings.disconnect(this._settingsId);
            this._settingsId = null;
        }

        this._removeKeybinding();

        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }

        if (this._unimLib) {
            this._unimLib.close();
            this._unimLib = null;
        }

        this._virtualKeyboard = null;
        this._settings = null;
    }


    _initKeybinding() {
        if (!this._settings.get_boolean('enable-manual-conversion')) return;

        console.log(`[unim] Registering keybinding, Meta: ${typeof Meta}`);
        try {
            Main.wm.addKeybinding(
                'manual-conversion-shortcut',
                this._settings,
                0,
                Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW,
                () => this._onManualConversion()
            );
        } catch (e) {
            console.error(`[unim] Failed to register keybinding: ${e.message}`);
        }
    }

    _removeKeybinding() {
        try {
            Main.wm.removeKeybinding('manual-conversion-shortcut');
        } catch (e) {
            console.debug(`[unim] Keybinding removal: ${e.message}`);
        }
    }

    _sendKeyCombo(keySymbol, modifiers = 0) {
        if (!this._virtualKeyboard) return;

        const time = GLib.get_monotonic_time();
        
        if (modifiers & Clutter.ModifierType.CONTROL_MASK)
            this._virtualKeyboard.notify_key(time, Clutter.KEY_Control_L, Clutter.KeyState.PRESSED);
        
        this._virtualKeyboard.notify_key(time, keySymbol, Clutter.KeyState.PRESSED);
        this._virtualKeyboard.notify_key(time, keySymbol, Clutter.KeyState.RELEASED);
        
        if (modifiers & Clutter.ModifierType.CONTROL_MASK)
            this._virtualKeyboard.notify_key(time, Clutter.KEY_Control_L, Clutter.KeyState.RELEASED);
    }

    async _sleep(ms) {
        return new Promise(resolve => {
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
                resolve();
                return GLib.SOURCE_REMOVE;
            });
        });
    }

    async _onManualConversion() {
        console.log('[unim] Manual conversion triggered (Copy -> Convert -> Paste)');
        
        // 1. Simulate Copy (Ctrl+C)
        this._sendKeyCombo(Clutter.KEY_c, Clutter.ModifierType.CONTROL_MASK);
        
        // Wait for clipboard to update
        await this._sleep(150);
        
        const clipboard = St.Clipboard.get_default();
        clipboard.get_text(St.ClipboardType.CLIPBOARD, async (clipboard, text) => {
            if (!text) {
                console.log('[unim] Clipboard empty or not updated');
                return;
            }

            const koLayout = `ko_${this._settings.get_string('korean-layout')}std`;
            const enLayout = `en_${this._settings.get_string('english-layout')}`;
            
            const hasKorean = /[ㄱ-ㅎㅏ-ㅣ가-힣]/.test(text);
            let fromLayout, toLayout;

            if (hasKorean) {
                fromLayout = koLayout;
                toLayout = enLayout;
            } else {
                fromLayout = enLayout;
                toLayout = koLayout;
            }

            console.log(`[unim] Auto-converting from ${fromLayout} to ${toLayout}`);
            const transformed = await this._unimLib.transformString(text, fromLayout, toLayout);
            
            if (transformed && transformed !== text) {
                // 2. Set transformed text to clipboard
                clipboard.set_text(St.ClipboardType.CLIPBOARD, transformed);
                
                // Wait for clipboard to settle
                await this._sleep(100);
                
                // 3. Simulate Paste (Ctrl+V)
                this._sendKeyCombo(Clutter.KEY_v, Clutter.ModifierType.CONTROL_MASK);
                
                if (this._settings.get_boolean('show-notification')) {
                    Main.notify('Unim', `Converted: ${text.substring(0, 10)}... → ${transformed.substring(0, 10)}...`);
                }
            }
        });
    }
}
