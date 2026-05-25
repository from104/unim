//! UNIM 데몬 가용성 빠른 체크.
//!
//! `org.freedesktop.DBus::NameHasOwner("org.atit.unim.InputMethod")` 한 번 호출 후 종료.
//! zbus blocking을 짧게 사용 — 별도 runtime/스레드 생성 없음.

use unim_dbus::BUS_NAME;

/// UNIM 데몬이 현재 세션 버스에서 활성인지 확인.
///
/// 반환:
/// - `true` — 데몬 활성. 정상 UI 진입.
/// - `false` — 세션 버스 연결 실패 또는 NameOwner 없음. 사용자에게 dialog 표시 후 종료.
pub fn is_daemon_running() -> bool {
    let conn = match zbus::blocking::Connection::session() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let proxy = match zbus::blocking::fdo::DBusProxy::new(&conn) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let name = match BUS_NAME.try_into() {
        Ok(n) => n,
        Err(_) => return false,
    };
    proxy.name_has_owner(name).unwrap_or(false)
}
