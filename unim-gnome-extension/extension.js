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

const IBUS_SERVICE = 'org.freedesktop.IBus';
const IBUS_PATH = '/org/freedesktop/IBus';
const IBUS_IFACE = 'org.freedesktop.IBus';
const DBUS_PROPS_IFACE = 'org.freedesktop.DBus.Properties';

/**
 * Helper class for raw D-Bus calls using Gio.DBus.session
 */
class DBusClient {
    constructor() {
        this._bus = Gio.DBus.session;
    }

    /**
     * Call a D-Bus method (asynchronous)
     * @param {string} service - Service name
     * @param {string} path - Object path
     * @param {string} iface - Interface name
     * @param {string} method - Method name
     * @param {GLib.Variant} params - Parameters (optional)
     * @param {GLib.VariantType} replyType - Expected reply type (optional)
     * @returns {Promise<any>} - The result of the call (unpacked)
     */
    call(service, path, iface, method, params = null, replyType = null) {
        return new Promise((resolve, reject) => {
            try {
                this._bus.call(
                    service,
                    path,
                    iface,
                    method,
                    params,
                    replyType,
                    Gio.DBusCallFlags.NONE,
                    -1,
                    null,
                    (connection, result) => {
                        try {
                            const reply = connection.call_finish(result);
                            resolve(reply);
                        } catch (e) {
                            console.warn(`[unim] D-Bus call failed (${method}): ${e.message}`);
                            reject(e);
                        }
                    }
                );
            } catch (e) {
                console.warn(`[unim] D-Bus call error (${method}): ${e.message}`);
                reject(e);
            }
        });
    }
}

/**
 * Manages IBus interaction
 */
class InputMethodManager {
    constructor() {
        this._client = new DBusClient();
    }

    /**
     * Get the current global engine name from IBus
     * @returns {Promise<string|null>} Engine name (e.g., 'hangul', 'xkb:us::eng')
     */
    async getCurrentEngine() {
        // Method 1: D-Bus Properties.Get
        try {
            const reply = await this._client.call(
                IBUS_SERVICE,
                IBUS_PATH,
                DBUS_PROPS_IFACE,
                'Get',
                new GLib.Variant('(ss)', [IBUS_IFACE, 'GlobalEngine']),
                new GLib.VariantType('(v)')
            );

            const [variant] = reply.deep_unpack();
            const [name, _displayName] = variant.deep_unpack();
            console.log(`[unim] Current engine (D-Bus): ${name}`);
            return name;
        } catch (e) {
            console.debug(`[unim] D-Bus Properties.Get failed: ${e.message}`);
        }

        // Method 2: GetGlobalEngine method (some IBus versions)
        try {
            const reply = await this._client.call(
                IBUS_SERVICE,
                IBUS_PATH,
                IBUS_IFACE,
                'GetGlobalEngine',
                null,
                new GLib.VariantType('(ss)')
            );
            const [name, _displayName] = reply.deep_unpack();
            console.log(`[unim] Current engine (GetGlobalEngine): ${name}`);
            return name;
        } catch (e) {
            console.debug(`[unim] GetGlobalEngine method failed: ${e.message}`);
        }

        // Method 3: ibus engine command (fallback)
        if (GLib.find_program_in_path('ibus')) {
            try {
                const [ok, out] = GLib.spawn_command_line_sync('ibus engine');
                if (ok && out) {
                    let text;
                    try {
                        // Try imports.byteArray (GJS standard)
                        const ByteArray = imports.byteArray;
                        text = ByteArray.toString(out).trim();
                    } catch (_) {
                        // Fallback: convert Uint8Array to string manually
                        text = String.fromCharCode.apply(null, Array.from(out)).trim();
                    }
                    if (text && text.length > 0) {
                        console.log(`[unim] Current engine (ibus command): ${text}`);
                        return text;
                    }
                }
            } catch (e) {
                console.debug(`[unim] ibus engine command failed: ${e.message}`);
            }
        }

        // Method 4: GNOME Shell internal API (final fallback)
        return this._getFallbackEngine();
    }

    /**
     * Set the global engine via IBus
     * @param {string} engineName 
     */
    async setEngine(engineName) {
        // Method 1: D-Bus SetGlobalEngine
        try {
            await this._client.call(
                IBUS_SERVICE,
                IBUS_PATH,
                IBUS_IFACE,
                'SetGlobalEngine',
                new GLib.Variant('(s)', [engineName]),
                null
            );
            console.log(`[unim] Set engine to: ${engineName} (via D-Bus)`);
            
            // Verify: wait a bit and check
            await new Promise(resolve => setTimeout(resolve, 100));
            const current = await this.getCurrentEngine();
            if (current === engineName) {
                return true;
            } else {
                console.warn(`[unim] Set to ${engineName} but current is ${current}`);
            }
        } catch (e) {
            console.debug(`[unim] D-Bus SetGlobalEngine failed: ${e.message}`);
        }

        // Method 2: ibus engine command (fallback)
        if (GLib.find_program_in_path('ibus')) {
            try {
                GLib.spawn_command_line_async(`ibus engine ${engineName}`);
                console.log(`[unim] Set engine to: ${engineName} (via ibus command)`);
                // Wait a bit for the command to take effect
                await new Promise(resolve => setTimeout(resolve, 200));
                return true;
            } catch (e) {
                console.debug(`[unim] ibus engine command failed: ${e.message}`);
            }
        }

        // Method 3: GNOME Shell internal API (final fallback)
        return this._setFallbackEngine(engineName);
    }

    /**
     * Toggle between Korean and English
     */
    async toggle() {
        const current = await this.getCurrentEngine();
        console.log(`[unim] Current engine before toggle: ${current}`);

        // If current contains 'hangul' or 'korean', switch to English
        // Otherwise (including 'xkb:us::eng'), switch to Hangul
        if (current && (current.includes('hangul') || current.includes('korean'))) {
            await this.setEngine('xkb:us::eng');
        } else {
            await this.setEngine('hangul');
        }
        
        return await this.getCurrentEngine();
    }

    // --- Fallbacks using GNOME Shell Internal API ---

    _getFallbackEngine() {
        try {
            // Attempt to use GNOME Shell's InputMethod
            if (Main.inputMethod && Main.inputMethod.currentInputSource) {
                return Main.inputMethod.currentInputSource.id;
            }
        } catch (e) {
            console.warn(`[unim] Fallback get failed: ${e.message}`);
        }
        return null;
    }

    _setFallbackEngine(engineId) {
        try {
            if (Main.panel.statusArea.keyboard) {
                const ism = Main.panel.statusArea.keyboard.getInputSourceManager();
                const sources = ism.inputSources;
                for (let i in sources) {
                    if (sources[i].id === engineId) {
                        sources[i].activate();
                        return true;
                    }
                }
            }
        } catch (e) {
            console.warn(`[unim] Fallback set failed: ${e.message}`);
        }
        return false;
    }
}

const Indicator = GObject.registerClass(
class Indicator extends PanelMenu.Button {
    _init(extension) {
        super._init(0.0, extension.metadata.name, false);
        
        // Store extension reference
        this._extension = extension;
        this._imManager = new InputMethodManager();
        this._settings = extension.getSettings();

        // UI Setup
        this._icon = new St.Icon({
            icon_name: 'input-keyboard-symbolic',
            style_class: 'system-status-icon',
        });
        this.add_child(this._icon);

        // Menu
        this._buildMenu();

        // Behavior
        this.connect('button-press-event', (actor, event) => {
            const button = event.get_button();
            if (button === 1) { // Left click
                this._handleClick();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        // Settings listener
        this._settings.connect('changed::show-indicator', () => {
            this.visible = this._settings.get_boolean('show-indicator');
        });
        this.visible = this._settings.get_boolean('show-indicator');

        // Initial status check
        this._updateStatus();
    }

    _buildMenu() {
        const titleItem = new PopupMenu.PopupMenuItem('Unim IME Control');
        titleItem.sensitive = false;
        this.menu.addMenuItem(titleItem);

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        const toggleItem = new PopupMenu.PopupMenuItem('Toggle Han/Eng');
        toggleItem.connect('activate', () => {
            this._handleClick();
        });
        this.menu.addMenuItem(toggleItem);

        const settingsItem = new PopupMenu.PopupMenuItem('Settings');
        settingsItem.connect('activate', () => {
            this._extension.openPreferences();
        });
        this.menu.addMenuItem(settingsItem);
    }

    async _handleClick() {
        const newEngine = await this._imManager.toggle();
        this._updateIcon(newEngine);
    }

    async _updateStatus() {
        const engine = await this._imManager.getCurrentEngine();
        this._updateIcon(engine);
    }

    _updateIcon(engineName) {
        if (engineName && (engineName.includes('hangul') || engineName.includes('korean'))) {
            // Korean Icon style (maybe filled or specific color if possible, but keep simple for now)
            this._icon.icon_name = 'input-keyboard-symbolic'; // Or a specific hangul icon if available
            this._icon.style_class = 'system-status-icon unim-active'; // Add a class for potential styling
        } else {
            this._icon.icon_name = 'input-keyboard-symbolic';
            this._icon.style_class = 'system-status-icon';
        }
    }
});

export default class UnimExtension extends Extension {
    enable() {
        console.log('[unim] Enabling extension...');
        this._settings = this.getSettings();
        this._indicator = new Indicator(this);
        Main.panel.addToStatusArea(this.uuid, this._indicator);

        // Listen for manual conversion setting changes
        this._keybindingSettingsId = this._settings.connect(
            'changed::enable-manual-conversion',
            () => {
                this._removeKeybinding();
                this._initKeybinding();
            }
        );

        this._initKeybinding();
    }

    disable() {
        console.log('[unim] Disabling extension...');
        
        // Disconnect settings listener
        if (this._keybindingSettingsId) {
            this._settings.disconnect(this._keybindingSettingsId);
            this._keybindingSettingsId = null;
        }
        
        this._removeKeybinding();

        if (this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
        this._settings = null;
    }

    _initKeybinding() {
        // Only register keybinding if manual conversion is enabled
        if (!this._settings.get_boolean('enable-manual-conversion')) {
            console.log('[unim] Manual conversion disabled, skipping keybinding registration');
            return;
        }

        try {
            Main.wm.addKeybinding(
                'manual-conversion-shortcut',
                this._settings,
                Meta.KeyBindingFlags.NONE,
                Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW,
                () => {
                    this._onManualConversion();
                }
            );
            console.log('[unim] Keybinding registered successfully');
        } catch (e) {
            console.error(`[unim] Failed to register keybinding: ${e.message}`);
        }
    }

    _removeKeybinding() {
        try {
            Main.wm.removeKeybinding('manual-conversion-shortcut');
        } catch (e) {
            // Keybinding might not be registered, ignore error
            console.debug(`[unim] Keybinding removal: ${e.message}`);
        }
    }

    _onManualConversion() {
        console.log('[unim] Manual conversion shortcut detected!');
        // TODO: Implement actual clipboard text conversion here
        // 1. Get clipboard content
        // 2. Convert text using libunim_core
        // 3. Set clipboard content
        // 4. Show notification
        Main.notify('Unim', 'Manual conversion shortcut pressed');
    }
}
