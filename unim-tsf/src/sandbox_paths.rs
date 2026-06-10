//! UWP(AppContainer) 경로 리다이렉트 우회.
//!
//! 스티커 메모·계산기 등 packaged(UWP/MSIX) 앱 안에서는 `SHGetKnownFolderPath`
//! (= `dirs::config_dir()` 내부 구현)가 RoamingAppData/LocalAppData 를 패키지
//! 전용 경로로 **리다이렉트**한다. 그 결과 UNIM DLL 이 사용자의 실제
//! `%APPDATA%\unim\config.yaml` 을 못 읽어 기본값(2벌식)으로 폴백한다.
//!
//! `KF_FLAG_NO_PACKAGE_REDIRECTION` 플래그를 주면 packaged 프로세스 안에서도
//! *리다이렉트되지 않은* 실제 경로를 돌려준다. 이를 `UNIM_CONFIG_DIR` /
//! `UNIM_DATA_DIR` / `UNIM_CACHE_DIR` 환경변수로 내보내면 코어 크레이트의
//! `unim::paths::*` 헬퍼가 전부 실제 경로를 쓰게 된다.
//!
//! 비-패키지 프로세스(메모장·wezterm)에서는 플래그가 무의미해 평소와 같은
//! 실제 경로가 나오므로 항상 호출해도 안전하다. (단, 실제 경로를 읽으려면
//! 해당 디렉터리에 "ALL APPLICATION PACKAGES" 읽기 ACE 가 필요 — MSI/설정앱이
//! 부여.)

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Once;

use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_LocalAppData, FOLDERID_RoamingAppData,
    KF_FLAG_NO_PACKAGE_REDIRECTION,
};

static INIT: Once = Once::new();

/// 프로세스 1회: 리다이렉트되지 않은 실제 AppData 경로를 알아내 환경변수로
/// 공개한다. config 로드보다 먼저 호출해야 한다 (idempotent).
pub fn init() {
    INIT.call_once(|| {
        if let Some(roaming) = real_known_folder(&FOLDERID_RoamingAppData) {
            std::env::set_var("UNIM_CONFIG_DIR", &roaming);
            std::env::set_var("UNIM_DATA_DIR", &roaming);
        }
        if let Some(local) = real_known_folder(&FOLDERID_LocalAppData) {
            std::env::set_var("UNIM_CACHE_DIR", &local);
        }
    });
}

/// `SHGetKnownFolderPath(rfid, KF_FLAG_NO_PACKAGE_REDIRECTION)` 로 실제 경로 해석.
fn real_known_folder(rfid: *const GUID) -> Option<PathBuf> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(rfid, KF_FLAG_NO_PACKAGE_REDIRECTION, None).ok()?;
        if pwstr.is_null() {
            return None;
        }
        let s = pwstr.to_string().ok();
        // SHGetKnownFolderPath 가 할당한 버퍼는 호출자가 CoTaskMemFree 로 해제.
        CoTaskMemFree(Some(pwstr.0 as *const c_void));
        s.map(PathBuf::from)
    }
}
