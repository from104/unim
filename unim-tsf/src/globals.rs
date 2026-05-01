//! UNIM TSF 전역 상수 및 GUID 정의

#[cfg(windows)]
use windows::core::GUID;

#[cfg(windows)]
pub const UNIM_CLSID: GUID = GUID::from_u128(0xA1B2C3D4_E5F6_7890_ABCD_EF1234567890);

#[cfg(windows)]
pub const UNIM_PROFILE_GUID: GUID = GUID::from_u128(0xB2C3D4E5_F6A7_8901_BCDE_F12345678901);

#[cfg(windows)]
pub const UNIM_DISPLAY_ATTR_INPUT: GUID = GUID::from_u128(0xC3D4E5F6_A7B8_9012_CDEF_123456789012);

#[cfg(windows)]
pub const UNIM_DISPLAY_ATTR_CONVERTED: GUID =
    GUID::from_u128(0xD4E5F6A7_B8C9_0123_DEF0_234567890123);

#[cfg(windows)]
pub const UNIM_LANGBAR_ITEM_GUID: GUID = GUID::from_u128(0xE5F6A7B8_C9D0_1234_EF01_345678901234);

pub const UNIM_LANGID_KOREAN: u16 = 0x0412;
pub const UNIM_IME_NAME: &str = "UNIM Korean IME";
