//! 유저당 단일 인스턴스 lock — $XDG_RUNTIME_DIR/unim-popup-service.lock 에 flock.
//! 반환된 File은 process 종료까지 살아 있어야 lock 유지.

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

pub fn acquire() -> Option<File> {
    let dir: PathBuf = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".cache");
                p
            })
        })?;
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    let lock_path = dir.join("unim-popup-service.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .ok()?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        return None;
    }
    Some(file)
}
