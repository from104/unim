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
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Once;

use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL};
use windows::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_W,
};
use windows::Win32::Security::{
    CreateWellKnownSid, WinBuiltinAnyPackageSid, ACL, DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows::Win32::Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_LocalAppData, FOLDERID_RoamingAppData,
    KF_FLAG_NO_PACKAGE_REDIRECTION,
};

static INIT: Once = Once::new();

/// 환경변수로부터 베이스 디렉터리를 읽어 비어있지 않으면 반환한다.
fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

/// roaming(Roaming) 베이스 경로를 결정한다.
///
/// 순서: `SHGetKnownFolderPath`(NO_PACKAGE_REDIRECTION) → `%APPDATA%` →
/// `%USERPROFILE%\AppData\Roaming`. AppContainer(스티커 메모 등)에서는 첫 호출이
/// 거부될 수 있으므로 env 폴백으로 실제 로밍을 잡는다.
///
/// 반환값은 `dirs::config_dir()` 와 동일한 **베이스 로밍**(예
/// `C:\Users\from1\AppData\Roaming`) — config.rs 가 뒤에 `unim/config.yaml` 을 붙인다.
/// 두 번째 반환값은 소스 라벨(진단 로그용).
fn resolve_roaming_base() -> Option<(PathBuf, &'static str)> {
    if let Some(p) = real_known_folder(&FOLDERID_RoamingAppData) {
        return Some((p, "known_folder"));
    }
    if let Some(p) = env_path("APPDATA") {
        return Some((p, "APPDATA env"));
    }
    if let Some(profile) = env_path("USERPROFILE") {
        return Some((profile.join("AppData").join("Roaming"), "USERPROFILE"));
    }
    None
}

/// local(Local) 베이스 경로를 결정한다.
///
/// 순서: `SHGetKnownFolderPath`(NO_PACKAGE_REDIRECTION) → `%LOCALAPPDATA%` →
/// `%USERPROFILE%\AppData\Local`. 반환값은 `dirs::cache_dir()` 와 동일한
/// **베이스 로컬**(예 `C:\Users\from1\AppData\Local`).
/// 두 번째 반환값은 소스 라벨(진단 로그용).
fn resolve_local_base() -> Option<(PathBuf, &'static str)> {
    if let Some(p) = real_known_folder(&FOLDERID_LocalAppData) {
        return Some((p, "known_folder"));
    }
    if let Some(p) = env_path("LOCALAPPDATA") {
        return Some((p, "LOCALAPPDATA env"));
    }
    if let Some(profile) = env_path("USERPROFILE") {
        return Some((profile.join("AppData").join("Local"), "USERPROFILE"));
    }
    None
}

/// 프로세스 1회: 리다이렉트되지 않은 실제 AppData 경로를 알아내 환경변수로
/// 공개한다. config 로드보다 먼저 호출해야 한다 (idempotent).
///
/// AppContainer(스티커 메모 등 UWP) 토큰에서는 `SHGetKnownFolderPath` 호출이
/// 거부돼 None 이 나올 수 있다. 그 경우 `%APPDATA%` / `%USERPROFILE%` env 폴백으로
/// 실제 로밍 베이스를 결정해 **항상** UNIM_CONFIG_DIR/UNIM_DATA_DIR/UNIM_CACHE_DIR
/// 을 set 한다(미설정 시 dirs::* 가 패키지 리다이렉트 경로로 폴백 → config 미독 →
/// 2벌식 폴백 버그).
pub fn init() {
    INIT.call_once(|| {
        // --- roaming(config/data) 베이스 결정 + set ---
        match resolve_roaming_base() {
            Some((roaming, src)) => {
                crate::register::dbg_log(&format!(
                    "sandbox_paths::init: roaming source={src} base={roaming:?}"
                ));
                // 세만틱: dirs::config_dir() 계약과 동일한 **베이스 로밍**으로 set.
                // (config.rs 가 뒤에 unim/config.yaml 을 join — 절대 roaming\\unim 으로 set 금지)
                std::env::set_var("UNIM_CONFIG_DIR", &roaming);
                std::env::set_var("UNIM_DATA_DIR", &roaming);
                crate::register::dbg_log(&format!(
                    "sandbox_paths::init: UNIM_CONFIG_DIR={roaming:?} UNIM_DATA_DIR={roaming:?}"
                ));

                // AppContainer(스티커 메모 등 UWP) 토큰이 실제 config.yaml 을 읽으려면
                // "ALL APPLICATION PACKAGES" 읽기 ACE 가 필요하다. 설치관리자(WiX)는
                // per-user %APPDATA% 를 다루지 못하므로 런타임에 보강한다(B5 근본 수정).
                // 디렉터리가 없으면 먼저 만든다(없으면 ACE 부여 대상 자체가 없음).
                // AppContainer 안에서 grant 자체는 권한부족으로 실패할 수 있으나
                // 비치명적 — 데스크톱/설정앱 실행이 ACL 을 부여해 둠(신규환경 데드락 완화).
                let unim_dir = roaming.join("unim");
                let _ = std::fs::create_dir_all(&unim_dir);
                grant_appcontainer_read(&unim_dir);
            }
            None => {
                crate::register::dbg_log(
                    "sandbox_paths::init: roaming 베이스 결정 실패(known_folder/APPDATA/USERPROFILE 모두 None) → dirs::* 폴백",
                );
            }
        }

        // --- local(cache) 베이스 결정 + set ---
        match resolve_local_base() {
            Some((local, src)) => {
                crate::register::dbg_log(&format!(
                    "sandbox_paths::init: local source={src} base={local:?}"
                ));
                std::env::set_var("UNIM_CACHE_DIR", &local);
                crate::register::dbg_log(&format!(
                    "sandbox_paths::init: UNIM_CACHE_DIR={local:?}"
                ));
            }
            None => {
                crate::register::dbg_log(
                    "sandbox_paths::init: local 베이스 결정 실패(known_folder/LOCALAPPDATA/USERPROFILE 모두 None) → dirs::* 폴백",
                );
            }
        }
    });
}

/// 디렉터리(및 하위 상속)에 "ALL APPLICATION PACKAGES"(S-1-15-2-1) 읽기 ACE 를
/// 추가한다. AppContainer 토큰으로 도는 UWP/패키지 앱(스티커 메모 등)이
/// config.yaml 을 읽을 수 있게 한다(B5). 실패는 비치명적(로그만, default 폴백).
///
/// 기존 DACL 을 보존하고(GetNamedSecurityInfo) GENERIC_READ + EXECUTE(디렉터리
/// traverse)만 grant 한다(쓰기 미부여 → 보안 노출 최소). 상속 플래그로 하위
/// 파일/폴더에도 적용된다.
fn grant_appcontainer_read(dir: &Path) {
    unsafe {
        // 1) ALL APPLICATION PACKAGES well-known SID 생성.
        let mut sid_buf = [0u8; 64]; // SECURITY_MAX_SID_SIZE 보다 충분
        let mut sid_len = sid_buf.len() as u32;
        let sid = PSID(sid_buf.as_mut_ptr() as *mut c_void);
        if CreateWellKnownSid(WinBuiltinAnyPackageSid, None, Some(sid), &mut sid_len).is_err() {
            crate::register::dbg_log("grant_appcontainer_read: CreateWellKnownSid 실패");
            return;
        }

        // 2) 경로를 wide null-terminated 로.
        let mut wpath: Vec<u16> = dir.as_os_str().encode_wide().collect();
        wpath.push(0);

        // 3) 기존 DACL 획득(보존).
        let mut old_dacl: *mut ACL = std::ptr::null_mut();
        let mut psd = PSECURITY_DESCRIPTOR::default();
        let rc = GetNamedSecurityInfoW(
            PCWSTR(wpath.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl as *mut _),
            None,
            &mut psd,
        );
        if rc != ERROR_SUCCESS {
            crate::register::dbg_log(&format!("grant_appcontainer_read: GetNamedSecurityInfo rc={rc:?}"));
            return;
        }

        // 4) 새 EXPLICIT_ACCESS 항목(읽기+traverse, 상속) 구성.
        let ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: (FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0),
            grfAccessMode: SET_ACCESS,
            grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: Default::default(),
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_GROUP,
                ptstrName: PWSTR(sid.0 as *mut u16),
            },
        };

        // 5) 기존 DACL 에 항목 병합.
        let mut new_dacl: *mut ACL = std::ptr::null_mut();
        let old = if old_dacl.is_null() { None } else { Some(old_dacl as *const ACL) };
        let rc = SetEntriesInAclW(Some(&[ea]), old, &mut new_dacl);
        if rc != ERROR_SUCCESS {
            crate::register::dbg_log(&format!("grant_appcontainer_read: SetEntriesInAcl rc={rc:?}"));
            free_sd(&psd);
            return;
        }

        // 6) 새 DACL 적용.
        let rc = SetNamedSecurityInfoW(
            PCWSTR(wpath.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_dacl as *const ACL),
            None,
        );
        crate::register::dbg_log(&format!(
            "grant_appcontainer_read: SetNamedSecurityInfo rc={rc:?} dir={dir:?}"
        ));

        if !new_dacl.is_null() {
            let _ = LocalFree(Some(HLOCAL(new_dacl as *mut c_void)));
        }
        free_sd(&psd);
    }
}

unsafe fn free_sd(psd: &PSECURITY_DESCRIPTOR) {
    if !psd.0.is_null() {
        let _ = LocalFree(Some(HLOCAL(psd.0)));
    }
}

/// `SHGetKnownFolderPath(rfid, KF_FLAG_NO_PACKAGE_REDIRECTION)` 로 실제 경로 해석.
///
/// AppContainer 토큰에서는 이 호출이 거부될 수 있다(`None` 반환). 과거에는
/// `.ok()?` 로 silent 였던 실패 원인(HRESULT)을 dbg_log 로 캡처해 폴백 추적을 돕는다.
fn real_known_folder(rfid: *const GUID) -> Option<PathBuf> {
    unsafe {
        let pwstr = match SHGetKnownFolderPath(rfid, KF_FLAG_NO_PACKAGE_REDIRECTION, None) {
            Ok(p) => p,
            Err(e) => {
                crate::register::dbg_log(&format!(
                    "real_known_folder: SHGetKnownFolderPath 실패 hr={:#010x} ({e})",
                    e.code().0
                ));
                return None;
            }
        };
        if pwstr.is_null() {
            crate::register::dbg_log("real_known_folder: SHGetKnownFolderPath 가 null 반환");
            return None;
        }
        let s = pwstr.to_string().ok();
        // SHGetKnownFolderPath 가 할당한 버퍼는 호출자가 CoTaskMemFree 로 해제.
        CoTaskMemFree(Some(pwstr.0 as *const c_void));
        s.map(PathBuf::from)
    }
}
