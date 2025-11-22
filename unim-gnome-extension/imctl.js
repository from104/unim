#!/usr/bin/gjs

// Command-line IME controller for IBus/Fcitx5 via D-Bus
// Usage:
//   gjs imctl.js status
//   gjs imctl.js toggle
//   gjs imctl.js set <im_name>

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
// byte array decode helper for GLib.spawn output (GJS 1.68+)
const ByteArray = imports.byteArray;

const IMF = { UNKNOWN: 0, IBUS: 1, FCITX5: 2 };

function dbusNameHasOwner(name) {
    const replyType = GLib.VariantType.new('(b)');
    const res = Gio.DBus.session.call_sync(
        'org.freedesktop.DBus',
        '/org/freedesktop/DBus',
        'org.freedesktop.DBus',
        'NameHasOwner',
        new GLib.Variant('(s)', [name]),
        replyType,
        Gio.DBusCallFlags.NONE,
        -1,
        null
    );
    const [owned] = res.deep_unpack();
    return owned;
}

function ibusObjectExists() {
    try {
        // Try to introspect root object of IBus
        Gio.DBus.session.call_sync(
            'org.freedesktop.IBus',
            '/org/freedesktop/IBus',
            'org.freedesktop.DBus.Introspectable',
            'Introspect',
            null,
            GLib.VariantType.new('(s)'),
            Gio.DBusCallFlags.NONE,
            -1,
            null
        );
        return true;
    } catch (_e) {
        return false;
    }
}

async function detectActiveIMF() {
    try {
        // Prefer Fcitx5 if available
        if (dbusNameHasOwner('org.fcitx.Fcitx5')) return IMF.FCITX5;
        // IBus: ensure the root object exists, not only the name owned
        if (dbusNameHasOwner('org.freedesktop.IBus') && ibusObjectExists()) return IMF.IBUS;
        // Fallback: check via `ibus engine`
        if (GLib.find_program_in_path('ibus')) {
            try {
                const [ok, out] = GLib.spawn_command_line_sync('ibus engine');
                if (ok && out && ByteArray.toString(out).trim().length > 0)
                    return IMF.IBUS;
            } catch (_e) {}
        }
    } catch (e) {
        printerr(`Failed to detect IMF: ${e.message}`);
    }
    return IMF.UNKNOWN;
}

// IBus
const IBUS_INTERFACE = `
<node>
  <interface name="org.freedesktop.IBus">
    <method name="SetGlobalEngine">
      <arg type="s" name="engine_name" direction="in"/>
    </method>
    <property name="GlobalEngine" type="(ss)" access="read"/>
  </interface>
</node>`;
const IBusProxy = Gio.DBusProxy.makeProxyWrapper(IBUS_INTERFACE);

class IbusController {
    constructor() {
        this._proxy = new IBusProxy(
            Gio.DBus.session,
            'org.freedesktop.IBus',
            '/org/freedesktop/IBus'
        );
    }
    async getCurrentIM() {
        try {
            // Try property first
            const reply = Gio.DBus.session.call_sync(
                'org.freedesktop.IBus',
                '/org/freedesktop/IBus',
                'org.freedesktop.DBus.Properties',
                'Get',
                new GLib.Variant('(ss)', ['org.freedesktop.IBus', 'GlobalEngine']),
                GLib.VariantType.new('(v)'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            const [variant] = reply.deep_unpack();
            const [name /*, displayName*/] = variant.deep_unpack();
            return name;
        } catch (_e1) {
            // Fallback: try method GetGlobalEngine (if provided)
            try {
                const alt = Gio.DBus.session.call_sync(
                    'org.freedesktop.IBus',
                    '/org/freedesktop/IBus',
                    'org.freedesktop.IBus',
                    'GetGlobalEngine',
                    null,
                    GLib.VariantType.new('(ss)'),
                    Gio.DBusCallFlags.NONE,
                    -1,
                    null
                );
                const [name /*, displayName*/] = alt.deep_unpack();
                return name;
            } catch (_e2) {
                // Final fallback: `ibus engine`
                if (GLib.find_program_in_path('ibus')) {
                    try {
                        const [ok, out] = GLib.spawn_command_line_sync('ibus engine');
                        if (ok && out) {
                            const text = ByteArray.toString(out).trim();
                            if (text.length > 0) return text;
                        }
                    } catch (_e3) {}
                }
                return 'unknown';
            }
        }
    }
    async setCurrentIM(imName) {
        try {
            await this._proxy.SetGlobalEngineRemote(imName);
        } catch (_e) {
            // Fallback to command line
            if (GLib.find_program_in_path('ibus')) {
                GLib.spawn_command_line_async(`ibus engine ${imName}`);
                return;
            }
            throw _e;
        }
    }
}

// Fcitx5
const FCITX5_INTERFACE = `
<node>
  <interface name="org.fcitx.Fcitx.Controller1">
    <method name="SetCurrentIM">
      <arg type="s" name="im_name" direction="in"/>
    </method>
    <property name="CurrentInputMethod" type="s" access="read"/>
  </interface>
</node>`;
const Fcitx5Proxy = Gio.DBusProxy.makeProxyWrapper(FCITX5_INTERFACE);

class Fcitx5Controller {
    constructor() {
        this._proxy = new Fcitx5Proxy(
            Gio.DBus.session,
            'org.fcitx.Fcitx5',
            '/controller'
        );
    }
    async getCurrentIM() {
        try {
            return this._proxy.get_cached_property('CurrentInputMethod').deep_unpack();
        } catch (e) {
            throw new Error(`Failed to query Fcitx5 current IM: ${e.message}`);
        }
    }
    async setCurrentIM(imName) {
        await this._proxy.SetCurrentIMRemote(imName);
    }
}

class IMManager {
    constructor() {
        this._imfType = IMF.UNKNOWN;
        this._controller = null;
    }
    async init() {
        this._imfType = await detectActiveIMF();
        if (this._imfType === IMF.IBUS) this._controller = new IbusController();
        else if (this._imfType === IMF.FCITX5) this._controller = new Fcitx5Controller();
    }
    isAvailable() { return this._controller !== null; }
    async getCurrentIM() { return this._controller.getCurrentIM(); }
    async setCurrentIM(imName) { return this._controller.setCurrentIM(imName); }
    async toggleKoreanMode() {
        const current = await this.getCurrentIM();
        if (this._imfType === IMF.IBUS) {
            if (current && current.includes('hangul')) return this.setCurrentIM('xkb:us::eng');
            return this.setCurrentIM('hangul');
        }
        if (this._imfType === IMF.FCITX5) {
            if (current === 'hangul') return this.setCurrentIM('keyboard-us');
            return this.setCurrentIM('hangul');
        }
        throw new Error('Unsupported IMF');
    }
}

function printHelp() {
    print('Usage: gjs imctl.js <command> [args]');
    print('Commands:');
    print('  status                 # Print current input method');
    print('  toggle                 # Toggle between Korean and English');
    print('  set <im_name>          # Set input method by name');
    print('Examples:');
    print('  gjs imctl.js status');
    print("  gjs imctl.js set 'hangul'");
    print("  gjs imctl.js set 'xkb:us::eng'   # IBus");
    print("  gjs imctl.js set 'keyboard-us'   # Fcitx5");
}

(async () => {
    const argv = ARGV; // gjs global
    if (argv.length === 0) {
        printHelp();
        GLib.program_exit(0);
        return;
    }

    const cmd = argv[0];
    const manager = new IMManager();
    await manager.init();

    if (!manager.isAvailable()) {
        printerr('No active Input Method Framework detected (IBus/Fcitx5).');
        GLib.program_exit(1);
        return;
    }

    try {
        if (cmd === 'status') {
            const im = await manager.getCurrentIM();
            print(`Current IM: ${im ?? 'unknown'}`);
        } else if (cmd === 'toggle') {
            await manager.toggleKoreanMode();
            const im = await manager.getCurrentIM();
            print(`Toggled. Current IM: ${im}`);
        } else if (cmd === 'set') {
            const imName = argv[1];
            if (!imName) {
                printerr('Missing argument: im_name');
                GLib.program_exit(2);
                return;
            }
            await manager.setCurrentIM(imName);
            const im = await manager.getCurrentIM();
            print(`Set completed. Current IM: ${im}`);
        } else if (cmd === 'help' || cmd === '--help' || cmd === '-h') {
            printHelp();
        } else {
            printerr(`Unknown command: ${cmd}`);
            printHelp();
            GLib.program_exit(2);
        }
    } catch (e) {
        printerr(`Error: ${e.message}`);
        GLib.program_exit(1);
    }
})();


