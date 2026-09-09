//! unim-tsf 빌드 스크립트 — DLL 에 시그니처 아이콘 + VERSIONINFO 를 임베드한다.
//!
//! 임베드된 RT_GROUP_ICON(ID 1)으로 LanguageProfile IconFile=DLL, IconIndex=0
//! 이 UNIM 'UN' 아이콘을 가리킨다(IME 선택기·설치 목록 표시). rc.exe 는 빌드
//! 래퍼(scripts/cargo-msvc.bat 의 vcvars)에서 PATH 에 잡힌다.
//!
//! VERSIONINFO: Windows Defender 오탐(Bearfoos.B!ml, 2026-09-03) 대응 — 배포
//! 위생 3종(서명·버전정보·평판) 중 버전정보 공백을 메운다. unim.rc 는 아이콘만
//! 담고 있었고 VS_VERSION_INFO 블록이 아예 없었다. 정적 unim.rc 를 고치는 대신
//! OUT_DIR 에 아이콘+VERSIONINFO 를 합친 .rc 를 매 빌드 생성해 CARGO_PKG_VERSION
//! 을 그대로 굽는다(하드코딩 금지 — Cargo.toml 버전이 바뀌면 자동 추종).

// 공유 헬퍼: build-support/version_rc.rs 의 embed_version_rc() (VERSIONINFO
// 임베드, 버전은 CARGO_PKG_VERSION 자동 추종 — 하드코딩 금지).
#[cfg(windows)]
include!("../build-support/version_rc.rs");

fn main() {
    #[cfg(windows)]
    {
        embed_version_rc(
            Some("icons/unim-signature.ico"),
            "UNIM Korean IME — TSF text service",
            "unim_tsf.dll",
            "VFT_DLL",
        );
    }

    // D-3(진단): 컴파일 타임에 빌드 타임스탬프를 env 로 굽는다 — 런타임 진단 배너
    // (unim-windows-common::debug::log_startup_banner)가 "이 DLL 이 언제 빌드됐는지"
    // 답할 수 있게 한다. 이 블록은 #[cfg(windows)] 밖에 있어야 한다 — 빌드 스크립트는
    // 항상 호스트에서 컴파일·실행되므로(Linux 에서 windows-gnu 로 크로스 빌드해도
    // build.rs 자체는 Linux 바이너리), 위 아이콘 임베드처럼 windows cfg 안에 넣으면
    // `make check-windows`(Linux → gnu cross) 에서 env 가 아예 안 잡혀 컴파일이 깨진다.
    println!("cargo:rustc-env=UNIM_BUILD_TIMESTAMP={}", build_timestamp_utc());
}

/// 현재 UTC 시각을 `YYYY-MM-DD HH:MM:SS UTC` 로 반환한다.
///
/// 외부 크레이트 의존을 추가하지 않으려고(`chrono` 는 build-dependencies 에 없음)
/// std 만으로 Howard Hinnant 의 `civil_from_days`(공개 도메인, 그레고리력 전 범위
/// 정확)를 이식했다.
fn build_timestamp_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
}

/// days-since-epoch(1970-01-01) → (year, month, day). Howard Hinnant 의
/// `civil_from_days` 이식 (http://howardhinnant.github.io/date_algorithms.html).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}
