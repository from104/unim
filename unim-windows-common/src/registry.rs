//! Registry read/write helpers for Win32 COM registration (REG_SZ, REG_DWORD) and module path resolution.

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;

/// REG_SZ 값을 `hkey` 아래에 기록한다. `name`이 `None`이면 기본값(unnamed).
/// (tsf register.rs:68-86 / imm32 register.rs:47-65 — 바이트 동일, as-is 이동)
pub fn set_reg_value(hkey: HKEY, name: Option<&HSTRING>, value: &str) -> Result<()> {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let err = RegSetValueExW(
            hkey,
            name.map(|h| PCWSTR(h.as_ptr())).unwrap_or(PCWSTR::null()),
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * 2,
            )),
        );
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

/// REG_DWORD 값을 `hkey` 아래에 기록한다.
/// (tsf register.rs:89-104 — as-is 이동. imm32는 향후 등록 확장 시 사용)
pub fn set_reg_dword(hkey: HKEY, name: &HSTRING, value: u32) -> Result<()> {
    let bytes = value.to_ne_bytes();
    unsafe {
        let err = RegSetValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            Some(0),
            REG_DWORD,
            Some(&bytes),
        );
        if err.is_err() {
            return Err(E_FAIL.into());
        }
    }
    Ok(())
}

/// 주어진 모듈 핸들의 파일 시스템 경로를 반환한다 (GetModuleFileNameW, 260 buf).
/// HMODULE을 인자로 받아 호출자가 자기 hinst(tsf dll_instance / imm32 ime_state::hinst)를
/// 넘긴다. (tsf register.rs:21-32 / imm32 register.rs:30-43 — 핸들 소스만 차이)
pub fn get_module_path(hmodule: HMODULE) -> Result<String> {
    let mut buf = [0u16; 260];
    let len = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(hmodule), &mut buf)
    };
    if len == 0 {
        return Err(E_FAIL.into());
    }
    Ok(String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(test)]
mod tests {
    // Registry functions require a live Win32 environment; structural/compile tests only.

    /// Verify that set_reg_value and set_reg_dword are accessible as pub symbols.
    #[test]
    fn registry_fns_are_pub() {
        // This test just confirms the items compile; no runtime registry access.
        let _: fn(
            windows::Win32::System::Registry::HKEY,
            Option<&windows::core::HSTRING>,
            &str,
        ) -> windows::core::Result<()> = super::set_reg_value;

        let _: fn(
            windows::Win32::System::Registry::HKEY,
            &windows::core::HSTRING,
            u32,
        ) -> windows::core::Result<()> = super::set_reg_dword;

        let _: fn(
            windows::Win32::Foundation::HMODULE,
        ) -> windows::core::Result<String> = super::get_module_path;
    }
}
