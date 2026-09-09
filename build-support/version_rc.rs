// 공유 build.rs 헬퍼 — PE(DLL/EXE) 에 VERSIONINFO(+선택적 아이콘)를 임베드한다.
//
// `include!("../build-support/version_rc.rs")` 로 각 크레이트의 build.rs 에서
// 그대로 끌어다 쓴다(별도 workspace 크레이트로 뽑지 않은 이유: build.rs 는
// 크레이트마다 독립 컴파일되고, 이 정도 분량(함수 하나)에 크레이트 의존 간선을
// 추가할 값어치가 없다 — 기존 unim-tsf/build.rs 의 civil_from_days 이식 관례와
// 동일하게 "작은 build.rs 헬퍼는 std 만으로 인라인" 원칙을 따른다).
//
// 배경: Windows Defender 오탐(Bearfoos.B!ml, 2026-09-03, 서기현 회사컴) — 무서명
// + VERSIONINFO 전무 + low prevalence 가 배포 위생 문제로 지목됐다. 이 헬퍼는
// 그중 VERSIONINFO 공백을 메운다(서명은 과제 3, CODE_SIGNING.md 대상).
//
// 버전은 하드코딩하지 않는다 — Cargo 가 build.rs 에 항상 주입하는
// CARGO_PKG_VERSION_{MAJOR,MINOR,PATCH} 로 FILEVERSION/PRODUCTVERSION 을 굽는다.
// 이 파일을 include! 하는 크레이트는 `#[cfg(windows)]` 블록 안에서만 호출해야
// 한다 — embed_resource::compile 은 그 크레이트의 `[target.'cfg(windows)'.
// build-dependencies]` 의 embed-resource 를 요구한다.

/// 아이콘(선택) + VS_VERSION_INFO 를 합친 .rc 를 OUT_DIR 에 생성해 embed-resource
/// 로 컴파일한다.
///
/// - `icon_rel_path`: 크레이트 디렉터리(CARGO_MANIFEST_DIR) 기준 상대경로.
///   `None` 이면 ICON 리소스 없이 VERSIONINFO 만 담는다(unim-popup-win 처럼
///   기존에 아이콘 리소스가 없던 크레이트용 — 새 아이콘을 추가하는 건 이 헬퍼의
///   범위 밖이다).
/// - `file_description` / `original_filename`: VERSIONINFO 문자열 필드.
///   `original_filename` 은 실제 빌드 산출 파일명(예: "unim_tsf.dll") — MSI 가
///   설치 시 다른 이름(unim_tsf32.dll 등)으로 복사해도 "원래 파일명"은 바뀌지
///   않는 게 맞다.
/// - `file_type`: `"VFT_DLL"` 또는 `"VFT_APP"` (winver.h 매크로, .rc 안에서 그대로
///   토큰으로 쓰인다 — `#include <winver.h>` 로 정의를 끌어온다).
///
/// OUT_DIR 은 크레이트 밖(target/.../build/<crate>-<hash>/out/)에 있으므로 아이콘
/// 경로는 절대경로로 박아야 rc.exe/windres 가 찾는다. 백슬래시는 전부 `/` 로
/// 정규화한다 — MSVC 네이티브 빌드(Windows 러너, 백슬래시 경로)에서 RC 문자열
/// 리터럴에 raw backslash 를 그대로 넣으면 이스케이프 시퀀스로 오독될 위험이
/// 있고, forward slash 는 Win32 경로 API 가 그대로 받아들인다.
#[cfg(windows)]
fn embed_version_rc(
    icon_rel_path: Option<&str>,
    file_description: &str,
    original_filename: &str,
    file_type: &str,
) {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 미설정 — build.rs 환경 이상");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR 미설정 — build.rs 환경 이상");

    let major = std::env::var("CARGO_PKG_VERSION_MAJOR").expect("CARGO_PKG_VERSION_MAJOR 미설정");
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR").expect("CARGO_PKG_VERSION_MINOR 미설정");
    let patch = std::env::var("CARGO_PKG_VERSION_PATCH").expect("CARGO_PKG_VERSION_PATCH 미설정");
    let ver_csv = format!("{major},{minor},{patch},0");
    let ver_str = format!("{major}.{minor}.{patch}.0");

    let icon_line = match icon_rel_path {
        Some(rel) => {
            println!("cargo:rerun-if-changed={rel}");
            let abs = std::path::Path::new(&manifest_dir).join(rel);
            let abs_fwd = abs.to_string_lossy().replace('\\', "/");
            format!("1 ICON \"{abs_fwd}\"\n\n")
        }
        None => String::new(),
    };

    let rc = format!(
        r#"#include <winver.h>

{icon_line}VS_VERSION_INFO VERSIONINFO
 FILEVERSION {ver_csv}
 PRODUCTVERSION {ver_csv}
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS VOS_NT_WINDOWS32
 FILETYPE {file_type}
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName",      "atit.org"
            VALUE "FileDescription",  "{file_description}"
            VALUE "FileVersion",      "{ver_str}"
            VALUE "InternalName",     "{original_filename}"
            VALUE "LegalCopyright",   "Copyright (C) atit.org"
            VALUE "OriginalFilename", "{original_filename}"
            VALUE "ProductName",      "UNIM"
            VALUE "ProductVersion",   "{ver_str}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );

    let rc_path = std::path::Path::new(&out_dir).join("unim_version_info.rc");
    std::fs::write(&rc_path, rc).expect("생성된 VERSIONINFO .rc 쓰기 실패");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    // include!() 로 끌어온 이 파일은 각 크레이트 디렉터리(package root) 밖에
    // 있어 Cargo 의 기본 "패키지 내 전체 변경 감시"에 잡히지 않는다. icon
    // rerun-if-changed 를 이미 찍은 시점부터 그 기본 감시는 꺼지므로, 이 헬퍼
    // 자체가 바뀌어도 재빌드가 트리거되도록 명시적으로 watch 를 등록한다.
    // 세 크레이트(unim-tsf/unim-settings/unim-popup-win) 모두 워크스페이스
    // 루트 바로 아래에 있어 상대경로가 동일하다.
    println!("cargo:rerun-if-changed=../build-support/version_rc.rs");

    embed_resource::compile(&rc_path, embed_resource::NONE);
}
