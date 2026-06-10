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

// 입력 모드 인디케이터 langbar item 은 표준 GUID_LBI_INPUTMODE 를 쓴다
// (lang_bar.rs 참조). 커스텀 GUID 는 OS 가 트레이 한/영 표시기를 그리지 않아
// 더 이상 사용하지 않는다.

pub const UNIM_LANGID_KOREAN: u16 = 0x0412;
pub const UNIM_IME_NAME: &str = "UNIM Korean IME";
