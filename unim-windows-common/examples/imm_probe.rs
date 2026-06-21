//! IMM32 `.ime` 격리 진단 (imm32-load-research.md §E + 후속).
//!
//! (1) LoadLibrary 로 .ime 로드(DllMain 실행) 확인
//! (2) 필수 IME export 존재 확인
//! (3) ImeInquire 직접 호출 → 반환 BOOL + IMEINFO + UI class 덤프
//! (4) ImmInstallIMEW 호출 → 반환 HKL + GetLastError
//!
//! 빌드(32비트, KakaoTalk x86 경로):
//!   cargo build -p unim-windows-common --example imm_probe --target i686-pc-windows-msvc --release
//! 실행: imm_probe.exe [<.ime 경로>]   (인자 없으면 System32\unim_imm32.ime)
//!   (1)(2)(3) 은 비관리자 가능. (4) ImmInstallIME 실제 등록은 관리자 필요.

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::GetLastError;

    #[link(name = "imm32")]
    extern "system" {
        fn ImmInstallIMEW(lpsz_ime_file_name: PCWSTR, lpsz_layout_text: PCWSTR) -> isize;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(p: PCWSTR) -> isize;
        fn GetProcAddress(h: isize, name: *const u8) -> *const core::ffi::c_void;
        fn FreeLibrary(h: isize) -> i32;
    }

    #[repr(C)]
    #[derive(Default, Debug)]
    #[allow(non_snake_case)]
    struct IMEINFO {
        dwPrivateDataSize: u32,
        fdwProperty: u32,
        fdwConversionCaps: u32,
        fdwSentenceCaps: u32,
        fdwUICaps: u32,
        fdwSCSCaps: u32,
        fdwSelectCaps: u32,
    }
    type ImeInquireFn = unsafe extern "system" fn(*mut IMEINFO, *mut u16, u32) -> i32;

    let path_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "C:\\Windows\\System32\\unim_imm32.ime".to_string());
    let path_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let path = PCWSTR(path_w.as_ptr());
    println!("== target: {path_str} ==");

    // (1) LoadLibrary
    let h = unsafe { LoadLibraryW(path) };
    if h == 0 {
        let e = unsafe { GetLastError() };
        println!("[1] LoadLibraryW FAILED  GetLastError={} (0x{:08X})", e.0, e.0);
    } else {
        println!("[1] LoadLibraryW OK  HMODULE=0x{:X}", h);

        // (2) required exports
        let names = [
            "ImeInquire", "ImeProcessKey", "ImeToAsciiEx", "ImeSelect",
            "ImeSetActiveContext", "NotifyIME", "ImeSetCompositionString",
            "ImeConfigure", "ImeDestroy", "ImeEscape", "UIWndProc",
        ];
        print!("[2] exports:");
        for n in names {
            let mut c: Vec<u8> = n.bytes().collect();
            c.push(0);
            let p = unsafe { GetProcAddress(h, c.as_ptr()) };
            print!(" {}={}", n, if p.is_null() { "X" } else { "ok" });
        }
        println!();

        // (3) ImeInquire
        let p = unsafe { GetProcAddress(h, b"ImeInquire\0".as_ptr()) };
        if p.is_null() {
            println!("[3] ImeInquire MISSING");
        } else {
            let f: ImeInquireFn = unsafe { core::mem::transmute(p) };
            let mut info = IMEINFO::default();
            let mut cls = [0u16; 64];
            let r = unsafe { f(&mut info, cls.as_mut_ptr(), 0) };
            let end = cls.iter().position(|&x| x == 0).unwrap_or(cls.len());
            let cls_s = String::from_utf16_lossy(&cls[..end]);
            println!("[3] ImeInquire BOOL={r}");
            println!("    dwPrivateDataSize={}", info.dwPrivateDataSize);
            println!("    fdwProperty=0x{:08X} fdwConversionCaps=0x{:08X} fdwSentenceCaps=0x{:08X}",
                info.fdwProperty, info.fdwConversionCaps, info.fdwSentenceCaps);
            println!("    fdwUICaps=0x{:08X} fdwSCSCaps=0x{:08X} fdwSelectCaps=0x{:08X}",
                info.fdwUICaps, info.fdwSCSCaps, info.fdwSelectCaps);
            println!("    UIClass={cls_s:?}");
        }
        unsafe { FreeLibrary(h); }
    }

    // (4) ImmInstallIME
    let name = w!("UNIM Korean (IMM32)");
    let hkl = unsafe { ImmInstallIMEW(path, name) };
    let err = unsafe { GetLastError() };
    println!("[4] ImmInstallIMEW -> HKL=0x{:08X}  GetLastError={} (0x{:08X})",
        hkl as usize, err.0, err.0);
    if hkl != 0 {
        let k = hkl as u32;
        println!("    => SUCCESS  KLID={:08X} (LANGID={:04X} device={:04X})",
            k, k & 0xFFFF, (k >> 16) & 0xFFFF);
    }

    // (5) LoadKeyboardLayoutW(E0200412) — 레지스트리 기반 로드 테스트.
    //     재부팅 후 win32k 가 Keyboard Layouts 를 재스캔해 E0200412 를 로드테이블에
    //     넣었다면 여기서 비0 HKL 이 나온다(재부팅 전엔 1419 예상). 비관리자 가능.
    #[link(name = "user32")]
    extern "system" {
        fn LoadKeyboardLayoutW(pwszklid: PCWSTR, flags: u32) -> isize;
    }
    let klid = w!("E0200412");
    let h2 = unsafe { LoadKeyboardLayoutW(klid, 0x0000_0001 /* KLF_ACTIVATE */) };
    let e2 = unsafe { GetLastError() };
    println!("[5] LoadKeyboardLayoutW(E0200412, KLF_ACTIVATE) -> HKL=0x{:08X}  GetLastError={} (0x{:08X})",
        h2 as usize, e2.0, e2.0);
    if h2 != 0 {
        println!("    => LOADABLE: 레지스트리 기반 등록이 라이브 테이블에 있음(재부팅 효과 확인).");
    }
}
