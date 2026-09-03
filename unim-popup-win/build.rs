//! unim-popup-win 빌드 스크립트 — exe 에 VERSIONINFO 를 임베드한다.
//!
//! 이 크레이트는 기존에 리소스 파일이 전혀 없었다(아이콘 없음). Windows
//! Defender 오탐(Bearfoos.B!ml, 2026-09-03) 대응으로 MSI 에 실리는 다른 PE
//! (unim-tsf, unim-settings)와 동일하게 VERSIONINFO 만 추가한다 — 새 아이콘을
//! 붙이는 건 이 변경의 범위 밖이다(창 아이콘 없이도 out-of-process 렌더러로
//! 정상 동작해 왔다).

// 공유 헬퍼: build-support/version_rc.rs 의 embed_version_rc() (VERSIONINFO
// 임베드, 버전은 CARGO_PKG_VERSION 자동 추종 — 하드코딩 금지). unim-tsf/
// unim-settings 의 build.rs 와 동일 패턴.
#[cfg(windows)]
include!("../build-support/version_rc.rs");

fn main() {
    #[cfg(windows)]
    {
        embed_version_rc(
            None,
            "UNIM Korean IME — Popup Renderer",
            "unim-popup-win.exe",
            "VFT_APP",
        );
    }
}
