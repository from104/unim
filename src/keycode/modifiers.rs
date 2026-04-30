//! 수정자 키 상태 ([`ModifierState`]) 정의 및 플랫폼별 마스크 변환.

/// 수정자 키 상태
///
/// Shift, Control, Alt, Super 키의 현재 눌림 상태를 나타냅니다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ModifierState {
    /// Shift 키 눌림 상태
    pub shift: bool,
    /// Control 키 눌림 상태
    pub control: bool,
    /// Alt 키 눌림 상태
    pub alt: bool,
    /// Super (Windows/Command) 키 눌림 상태
    pub super_key: bool,
    /// Caps Lock 활성화 상태
    pub caps_lock: bool,
    /// Num Lock 활성화 상태
    pub num_lock: bool,
}

impl ModifierState {
    /// 새로운 빈 ModifierState를 생성합니다.
    pub fn new() -> Self {
        Self::default()
    }

    /// X11 수정자 마스크에서 ModifierState를 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `mask` - X11 수정자 마스크 (GDK 스타일)
    pub fn from_x11_mask(mask: u32) -> Self {
        const SHIFT_MASK: u32 = 1 << 0;
        const LOCK_MASK: u32 = 1 << 1;
        const CONTROL_MASK: u32 = 1 << 2;
        const MOD1_MASK: u32 = 1 << 3; // Alt
        const MOD4_MASK: u32 = 1 << 6; // Super

        Self {
            shift: (mask & SHIFT_MASK) != 0,
            control: (mask & CONTROL_MASK) != 0,
            alt: (mask & MOD1_MASK) != 0,
            super_key: (mask & MOD4_MASK) != 0,
            caps_lock: (mask & LOCK_MASK) != 0,
            num_lock: false, // X11에서 별도 처리 필요
        }
    }

    /// Win32 수정자 비트마스크에서 ModifierState를 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `modifiers` - 비트마스크: bit0=Shift, bit1=Control, bit2=Alt,
    ///                 bit3=Super(Win), bit4=CapsLock, bit5=NumLock
    pub fn from_win32_modifiers(modifiers: u32) -> Self {
        Self {
            shift: (modifiers & 0x01) != 0,
            control: (modifiers & 0x02) != 0,
            alt: (modifiers & 0x04) != 0,
            super_key: (modifiers & 0x08) != 0,
            caps_lock: (modifiers & 0x10) != 0,
            num_lock: (modifiers & 0x20) != 0,
        }
    }

    /// 수정자가 하나도 눌리지 않은 상태인지 확인합니다.
    pub fn is_empty(&self) -> bool {
        !self.shift && !self.control && !self.alt && !self.super_key
    }

    /// Shift만 눌린 상태인지 확인합니다.
    pub fn is_shift_only(&self) -> bool {
        self.shift && !self.control && !self.alt && !self.super_key
    }

    /// Control만 눌린 상태인지 확인합니다.
    pub fn is_control_only(&self) -> bool {
        !self.shift && self.control && !self.alt && !self.super_key
    }

    /// Alt만 눌린 상태인지 확인합니다.
    pub fn is_alt_only(&self) -> bool {
        !self.shift && !self.control && self.alt && !self.super_key
    }
}
