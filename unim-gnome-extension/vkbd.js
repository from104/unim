/**
 * Native Virtual Keyboard for GNOME Shell
 */

import Clutter from 'gi://Clutter';

export class VirtualKeyboard {
    constructor() {
        this._device = null;
        this._init();
    }

    _init() {
        try {
            const seat = Clutter.get_default_backend().get_default_seat();
            this._device = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
            console.log('[unim] Virtual keyboard device created');
        } catch (e) {
            console.error(`[unim] Failed to create virtual keyboard: ${e}`);
        }
    }

    sendKeyCombination(modifiers, keyval) {
        if (!this._device) {
            console.error('[unim] No virtual keyboard device!');
            return;
        }

        const time = Clutter.get_current_event_time();
        console.log(`[unim] Sending key combination: modifiers=${modifiers}, keyval=${keyval}`);

        // Press modifiers
        modifiers.forEach(mod => {
            this._device.notify_keyval(time, mod, Clutter.KeyState.PRESSED);
        });

        // Press key
        this._device.notify_keyval(time, keyval, Clutter.KeyState.PRESSED);
        
        // Release key
        this._device.notify_keyval(time, keyval, Clutter.KeyState.RELEASED);

        // Release modifiers
        modifiers.reverse().forEach(mod => {
            this._device.notify_keyval(time, mod, Clutter.KeyState.RELEASED);
        });
    }

    paste() {
        console.log('[unim] Virtual Keyboard: Paste (Ctrl+v)');
        // Try both lower and upper just in case, but normally Ctrl+v (97) or Ctrl+V (86)
        this.sendKeyCombination([Clutter.KEY_Control_L], Clutter.KEY_v);
    }
    
    copy() {
        this.sendKeyCombination([Clutter.KEY_Control_L], Clutter.KEY_c);
    }

    backspace() {
        this.sendKeyCombination([], Clutter.KEY_BackSpace);
    }

    backspaceMultiple(count) {
        console.log(`[unim] Virtual Keyboard: Backspace x${count}`);
        for (let i = 0; i < count; i++) {
            this.backspace();
        }
    }
}
