//! X11 DPI 스케일 팩터 유틸리티
//!
//! Xft.dpi X 리소스를 읽어 96 DPI 기준 스케일 팩터를 계산합니다.
//! GTK 등 데스크탑 환경이 설정한 DPI를 따르므로, 1.25x 배율이면 120 DPI → scale 1.25.

use std::ffi::CStr;
use std::os::raw::c_int;

/// X11 Xft.dpi 리소스에서 스케일 팩터를 계산합니다 (기준: 96 DPI).
///
/// 실패 시 1.0을 반환합니다.
pub fn get_scale_factor(display: *mut x11::xlib::Display, screen: c_int) -> f64 {
    if display.is_null() {
        return 1.0;
    }

    // 방법 1: XRDB에서 Xft.dpi 읽기 (데스크탑 환경이 설정)
    let rms_ptr = unsafe { x11::xlib::XResourceManagerString(display) };
    if !rms_ptr.is_null() {
        let rms_str = unsafe { CStr::from_ptr(rms_ptr) }.to_string_lossy();
        for line in rms_str.lines() {
            let trimmed = line.trim();
            let rest = trimmed
                .strip_prefix("Xft.dpi:")
                .or_else(|| trimmed.strip_prefix("Xft.dpi\t"));
            if let Some(val) = rest {
                if let Ok(dpi) = val.trim().parse::<f64>() {
                    if dpi > 0.0 {
                        return dpi / 96.0;
                    }
                }
            }
        }
    }

    // 방법 2: 물리 해상도에서 DPI 계산
    let width_px = unsafe { x11::xlib::XDisplayWidth(display, screen) } as f64;
    let width_mm = unsafe { x11::xlib::XDisplayWidthMM(display, screen) } as f64;
    if width_mm > 0.0 {
        let dpi = width_px / (width_mm / 25.4);
        if dpi > 72.0 {
            return dpi / 96.0;
        }
    }

    1.0
}

/// 픽셀 값에 스케일 팩터를 적용합니다.
#[inline]
pub fn scale(value: c_int, factor: f64) -> c_int {
    (value as f64 * factor).round() as c_int
}

/// u16 픽셀 값에 스케일 팩터를 적용합니다.
#[inline]
pub fn scale_u16(value: u16, factor: f64) -> u16 {
    (value as f64 * factor).round() as u16
}
