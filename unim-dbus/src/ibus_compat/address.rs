//! IBus 주소 파일 관리
//!
//! IBus 클라이언트(`im-ibus.so`)는 `~/.config/ibus/bus/` 디렉토리의 주소 파일을 읽어
//! ibus-daemon에 연결한다. UNIM은 session bus 주소를 이 파일에 기록하여
//! `im-ibus.so`가 session bus를 통해 UNIM의 IBus 호환 서비스에 연결하게 한다.
//!
//! Fcitx5도 동일한 방식을 사용한다.

use std::fs;
use std::io;
use std::path::PathBuf;
use unim::unim_log;

/// IBus 주소 파일 경로 생성
///
/// 형식: `~/.config/ibus/bus/<machine-id>-<hostname>-<display>`
pub fn ibus_address_file_path() -> io::Result<PathBuf> {
    let config_dir = dirs_config_dir()?;
    let dir = config_dir.join("ibus").join("bus");

    let machine_id = read_machine_id()?;
    let hostname = get_hostname();
    let display = get_display_id();

    let filename = format!("{}-{}-{}", machine_id, hostname, display);
    Ok(dir.join(filename))
}

/// IBus 주소 파일 작성
///
/// Session bus 주소를 IBus 형식으로 기록한다.
/// `im-ibus.so`가 이 파일을 읽어 session bus에 연결하게 된다.
pub fn write_address_file() -> io::Result<PathBuf> {
    let path = ibus_address_file_path()?;

    // 디렉토리 생성
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bus_address = get_session_bus_address()?;
    let pid = std::process::id();

    let content = format!(
        "# This file is created by unim-daemon (IBus compat), please do not modify it.\n\
         # Created by UNIM IBus compatibility layer\n\
         IBUS_ADDRESS={}\n\
         IBUS_DAEMON_PID={}\n",
        bus_address, pid
    );

    fs::write(&path, &content)?;

    unim_log!(
        "DAEMON",
        "[IBus Compat] 주소 파일 생성: {}",
        path.display()
    );

    Ok(path)
}

/// IBus 주소 파일 삭제
pub fn remove_address_file() {
    match ibus_address_file_path() {
        Ok(path) => {
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    unim_log!(
                        "DAEMON",
                        "[IBus Compat] 주소 파일 삭제 실패: {} - {}",
                        path.display(),
                        e
                    );
                } else {
                    unim_log!(
                        "DAEMON",
                        "[IBus Compat] 주소 파일 삭제: {}",
                        path.display()
                    );
                }
            }
        }
        Err(e) => {
            unim_log!(
                "DAEMON",
                "[IBus Compat] 주소 파일 경로 확인 실패: {}",
                e
            );
        }
    }
}

/// `/etc/machine-id` 읽기
fn read_machine_id() -> io::Result<String> {
    let content = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))?;
    Ok(content.trim().to_string())
}

/// 호스트명 가져오기
fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "localhost".to_string())
}

/// DISPLAY 환경변수에서 디스플레이 ID 추출
///
/// `:0` → `0`, `:1.0` → `1`, `wayland-0` → `0`
fn get_display_id() -> String {
    // Wayland: WAYLAND_DISPLAY=wayland-0 → "0"
    if let Ok(wayland) = std::env::var("WAYLAND_DISPLAY") {
        if let Some(num) = wayland.strip_prefix("wayland-") {
            return num.to_string();
        }
    }
    // X11: DISPLAY=:0 or :0.0 → "0"
    if let Ok(display) = std::env::var("DISPLAY") {
        let stripped = display.trim_start_matches(':');
        if let Some(dot) = stripped.find('.') {
            return stripped[..dot].to_string();
        }
        return stripped.to_string();
    }
    "0".to_string()
}

/// Session bus 주소 가져오기
fn get_session_bus_address() -> io::Result<String> {
    std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "DBUS_SESSION_BUS_ADDRESS 환경변수 없음",
        )
    })
}

/// `~/.config` 디렉토리 경로
fn dirs_config_dir() -> io::Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".config"));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "HOME 환경변수 없음",
    ))
}
