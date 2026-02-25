//! xkbcommon 키맵 핸들러
//!
//! Wayland 컴포지터에서 수신한 키맵 fd를 사용하여
//! evdev keycode → XKB keysym 변환을 수행합니다.

use std::os::fd::OwnedFd;
use xkbcommon::xkb::{
    self, Context, Keycode, Keymap, State, CONTEXT_NO_FLAGS, KEYMAP_COMPILE_NO_FLAGS,
    KEYMAP_FORMAT_TEXT_V1,
};

/// xkbcommon 기반 키맵 핸들러
pub struct KeymapHandler {
    context: Context,
    state: Option<State>,
    /// 현재 수정자 상태 (GDK 호환 비트마스크)
    pub mod_state: u32,
}

// GDK modifier masks (unim-daemon DBus API와 호환)
const GDK_SHIFT_MASK: u32 = 1 << 0;
const GDK_CONTROL_MASK: u32 = 1 << 2;
const GDK_MOD1_MASK: u32 = 1 << 3; // Alt
const GDK_SUPER_MASK: u32 = 1 << 26;

impl KeymapHandler {
    pub fn new() -> Self {
        Self {
            context: Context::new(CONTEXT_NO_FLAGS),
            state: None,
            mod_state: 0,
        }
    }

    /// 컴포지터에서 수신한 키맵 fd로 xkb state 생성
    ///
    /// keyboard_grab의 keymap 이벤트에서 호출됩니다.
    /// fd: 키맵 파일 디스크립터, size: 키맵 크기
    pub fn update_keymap(&mut self, fd: OwnedFd, size: u32) -> bool {
        let keymap = unsafe {
            Keymap::new_from_fd(
                &self.context,
                fd,
                size as usize,
                KEYMAP_FORMAT_TEXT_V1,
                KEYMAP_COMPILE_NO_FLAGS,
            )
        };

        match keymap {
            Ok(Some(keymap)) => {
                self.state = Some(State::new(&keymap));
                true
            }
            _ => {
                self.state = None;
                false
            }
        }
    }

    /// evdev keycode → XKB keysym 변환
    ///
    /// Wayland은 evdev keycode를 사용하므로 +8 오프셋을 적용하여
    /// XKB keycode로 변환한 후 keysym을 조회합니다.
    pub fn get_keysym(&self, evdev_keycode: u32) -> u32 {
        if let Some(ref state) = self.state {
            let xkb_keycode = Keycode::new(evdev_keycode + 8);
            state.key_get_one_sym(xkb_keycode).raw()
        } else {
            // keymap 없으면 0 (XKB_KEY_NoSymbol)
            0
        }
    }

    /// 수정자 상태 업데이트
    ///
    /// keyboard_grab의 modifiers 이벤트에서 호출됩니다.
    /// Wayland modifier 비트를 GDK 호환 비트마스크로 변환합니다.
    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        // xkb state 업데이트
        if let Some(ref mut state) = self.state {
            state.update_mask(
                xkb::ModMask::from(mods_depressed),
                xkb::ModMask::from(mods_latched),
                xkb::ModMask::from(mods_locked),
                0,
                0,
                xkb::LayoutIndex::from(group),
            );
        }

        // XKB modifier bitmask → GDK modifier bitmask 변환
        // XKB: Shift=0x1, Lock=0x2, Control=0x4, Mod1(Alt)=0x8, Mod4(Super)=0x40
        self.mod_state = 0;
        if mods_depressed & 0x1 != 0 {
            self.mod_state |= GDK_SHIFT_MASK;
        }
        if mods_depressed & 0x4 != 0 {
            self.mod_state |= GDK_CONTROL_MASK;
        }
        if mods_depressed & 0x8 != 0 {
            self.mod_state |= GDK_MOD1_MASK;
        }
        if mods_depressed & 0x40 != 0 {
            self.mod_state |= GDK_SUPER_MASK;
        }
    }
}
