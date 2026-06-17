//! Modifier state assembly from Windows virtual key probes (GetKeyState / IMM32 key array).

use unim::keycode::ModifierState;

/// 코어: 수정자 상태를 두 프로브로부터 조립한다.
///
/// `down(vk)` = 해당 VK가 눌려 있는가(bit7 set),
/// `toggled(vk)` = 토글 on 인가(bit0 set).
///
/// VK 집합/조립 규칙은 TSF·IMM32 공통. (assessment §5)
pub fn modifier_state_from(
    down: impl Fn(u16) -> bool,
    toggled: impl Fn(u16) -> bool,
) -> ModifierState {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    ModifierState {
        shift: down(VK_SHIFT.0),
        control: down(VK_CONTROL.0),
        alt: down(VK_MENU.0),
        super_key: down(VK_LWIN.0) || down(VK_RWIN.0),
        caps_lock: toggled(VK_CAPITAL.0),
        num_lock: toggled(VK_NUMLOCK.0),
    }
}

/// TSF wrapper — live `GetKeyState`. (tsf key_handler.rs:20-38 대체)
pub fn modifier_state_live() -> ModifierState {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
    modifier_state_from(
        |vk| unsafe { GetKeyState(vk as i32) } < 0,
        |vk| (unsafe { GetKeyState(vk as i32) } & 0x01) != 0,
    )
}

/// IMM32 wrapper — `lpbKeyState` 256바이트 배열. null → all-false.
/// (imm32 input.rs:45-61 대체)
///
/// # Safety
/// `key_state`는 읽을 수 있는 256바이트 배열을 가리켜야 한다(IMM32 콜백 계약).
pub unsafe fn modifier_state_from_key_array(key_state: *const u8) -> ModifierState {
    if key_state.is_null() {
        return ModifierState::default();
    }
    modifier_state_from(
        |vk| unsafe { (*key_state.add(vk as usize) & 0x80) != 0 },
        |vk| unsafe { (*key_state.add(vk as usize) & 0x01) != 0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// modifier_state_from: all-zeros probes → default (all false).
    #[test]
    fn all_false_probes_produce_default() {
        let state = modifier_state_from(|_| false, |_| false);
        assert_eq!(state, ModifierState::default());
    }

    /// modifier_state_from: shift bit set.
    #[test]
    fn shift_probe_sets_shift() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT;
        let state = modifier_state_from(|vk| vk == VK_SHIFT.0, |_| false);
        assert!(state.shift);
        assert!(!state.control);
        assert!(!state.alt);
        assert!(!state.super_key);
        assert!(!state.caps_lock);
        assert!(!state.num_lock);
    }

    /// modifier_state_from: caps_lock toggled via toggle probe.
    #[test]
    fn caps_lock_via_toggle_probe() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL;
        let state = modifier_state_from(|_| false, |vk| vk == VK_CAPITAL.0);
        assert!(state.caps_lock);
        assert!(!state.shift);
    }

    /// modifier_state_from: num_lock toggled.
    #[test]
    fn num_lock_via_toggle_probe() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_NUMLOCK;
        let state = modifier_state_from(|_| false, |vk| vk == VK_NUMLOCK.0);
        assert!(state.num_lock);
        assert!(!state.caps_lock);
    }

    /// modifier_state_from: super_key set by LWIN or RWIN.
    #[test]
    fn super_key_from_lwin_or_rwin() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LWIN, VK_RWIN};
        let lwin = modifier_state_from(|vk| vk == VK_LWIN.0, |_| false);
        assert!(lwin.super_key);
        let rwin = modifier_state_from(|vk| vk == VK_RWIN.0, |_| false);
        assert!(rwin.super_key);
    }

    /// modifier_state_from_key_array: null pointer → default.
    #[test]
    fn key_array_null_is_default() {
        let state = unsafe { modifier_state_from_key_array(std::ptr::null()) };
        assert_eq!(state, ModifierState::default());
    }

    /// modifier_state_from_key_array: 256-byte array with bit7 set for VK_SHIFT.
    #[test]
    fn key_array_shift_bit7() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT;
        let mut arr = [0u8; 256];
        arr[VK_SHIFT.0 as usize] = 0x80; // bit7 = down
        let state = unsafe { modifier_state_from_key_array(arr.as_ptr()) };
        assert!(state.shift);
        assert!(!state.control);
    }

    /// modifier_state_from_key_array: bit0 for VK_CAPITAL → caps_lock.
    #[test]
    fn key_array_caps_bit0() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL;
        let mut arr = [0u8; 256];
        arr[VK_CAPITAL.0 as usize] = 0x01; // bit0 = toggled
        let state = unsafe { modifier_state_from_key_array(arr.as_ptr()) };
        assert!(state.caps_lock);
        assert!(!state.shift);
    }

    /// Win32-bit-decode round-trip via from_win32_modifiers helper on ModifierState itself.
    #[test]
    fn win32_modifiers_round_trip() {
        // bit layout: bit0=Shift, bit1=Ctrl, bit2=Alt, bit3=Super, bit4=CapsLock, bit5=NumLock
        // Set shift(bit0)=1, ctrl(bit1)=0, alt(bit2)=1, super(bit3)=0, caps(bit4)=1, num(bit5)=0
        // => 0b010101 = 0x15
        let raw: u32 = 0b01_0101; // shift=1, ctrl=0, alt=1, super=0, caps=1, num=0
        let state = ModifierState::from_win32_modifiers(raw);
        assert!(state.shift,    "shift should be set (bit0)");
        assert!(!state.control, "control should be clear (bit1)");
        assert!(state.alt,      "alt should be set (bit2)");
        assert!(!state.super_key, "super_key should be clear (bit3)");
        assert!(state.caps_lock,  "caps_lock should be set (bit4)");
        assert!(!state.num_lock,  "num_lock should be clear (bit5)");
    }
}
