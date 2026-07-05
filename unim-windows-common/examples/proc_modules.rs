//! 프로세스 모듈 열거 (WOW64 false-negative 회피 진단).
//!
//! 64비트 도구(Process Explorer 64, .NET Process.Modules, tasklist)는 32비트
//! 프로세스(WOW64)의 실제 32비트 모듈을 못 보고 wow64*.dll 6개만 본다.
//! → "KakaoTalk이 msctf/imm32 미로드" 결론은 그 false-negative일 수 있다.
//!
//! 이 프로브는 **32비트로 빌드**하여 Toolhelp32(TH32CS_SNAPMODULE)로
//! 대상 32비트 프로세스의 전체 모듈 목록을 읽는다.
//!
//! 빌드(32비트 필수):
//!   cargo build -p unim-windows-common --example proc_modules --target i686-pc-windows-msvc --release
//! 실행: proc_modules.exe <PID>
//!   특정 모듈 존재 여부(msctf/imm32/textinput 등)를 요약 출력.

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[repr(C)]
    struct ModuleEntry32W {
        dw_size: u32,
        th32_module_id: u32,
        th32_process_id: u32,
        glblcnt_usage: u32,
        proccnt_usage: u32,
        mod_base_addr: *mut u8,
        mod_base_size: u32,
        h_module: isize,
        sz_module: [u16; 256],
        sz_exe_path: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(dwflags: u32, th32processid: u32) -> isize;
        fn Module32FirstW(hsnapshot: isize, lpme: *mut ModuleEntry32W) -> i32;
        fn Module32NextW(hsnapshot: isize, lpme: *mut ModuleEntry32W) -> i32;
        fn CloseHandle(h: isize) -> i32;
        fn GetLastError() -> u32;
    }

    const TH32CS_SNAPMODULE: u32 = 0x00000008;
    const TH32CS_SNAPMODULE32: u32 = 0x00000010;
    const INVALID_HANDLE_VALUE: isize = -1;

    let pid: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("usage: proc_modules.exe <PID>");

    println!("[proc_modules] self-arch = {}-bit", std::mem::size_of::<usize>() * 8);
    println!("[proc_modules] target PID = {pid}");

    let snap = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
    };
    if snap == INVALID_HANDLE_VALUE {
        let e = unsafe { GetLastError() };
        eprintln!("CreateToolhelp32Snapshot failed err={e} (5=ACCESS_DENIED, 299=PARTIAL_COPY)");
        eprintln!("ACCESS_DENIED → run elevated; PARTIAL_COPY → arch mismatch (build i686!)");
        std::process::exit(1);
    }

    let mut me = ModuleEntry32W {
        dw_size: std::mem::size_of::<ModuleEntry32W>() as u32,
        th32_module_id: 0,
        th32_process_id: 0,
        glblcnt_usage: 0,
        proccnt_usage: 0,
        mod_base_addr: std::ptr::null_mut(),
        mod_base_size: 0,
        h_module: 0,
        sz_module: [0u16; 256],
        sz_exe_path: [0u16; 260],
    };

    let targets = [
        "msctf.dll",
        "imm32.dll",
        "TextInputFramework.dll",
        "CoreMessaging.dll",
        "InputHost.dll",
        "user32.dll",
        "unim_imm32.ime",
        "unim_tsf.dll",
    ];
    let mut found: Vec<String> = Vec::new();
    let mut count = 0u32;

    let mut ok = unsafe { Module32FirstW(snap, &mut me) };
    while ok != 0 {
        count += 1;
        let len = me.sz_module.iter().position(|&c| c == 0).unwrap_or(256);
        let name = OsString::from_wide(&me.sz_module[..len])
            .to_string_lossy()
            .to_string();
        found.push(name);
        ok = unsafe { Module32NextW(snap, &mut me) };
    }
    unsafe { CloseHandle(snap) };

    println!("[proc_modules] total modules = {count}");
    println!("---- target module presence ----");
    for t in targets {
        let hit = found.iter().any(|m| m.eq_ignore_ascii_case(t));
        println!("  {t:<28} => {}", if hit { "LOADED" } else { "absent" });
    }
    println!("---- input/ime/tsf related ----");
    for m in &found {
        let lm = m.to_ascii_lowercase();
        if lm.contains("imm")
            || lm.contains("msctf")
            || lm.contains("textinput")
            || lm.contains("input")
            || lm.contains("ime")
            || lm.contains("tsf")
            || lm.contains("coremessag")
        {
            println!("  {m}");
        }
    }
}
