// 공유 헬퍼: build-support/version_rc.rs 의 embed_version_rc() (VERSIONINFO
// 임베드, 버전은 CARGO_PKG_VERSION 자동 추종 — 하드코딩 금지). unim-tsf/build.rs
// 와 동일 패턴.
#[cfg(windows)]
include!("../build-support/version_rc.rs");

fn main() {
    // ko/en 번들 번역: translations/<lang>/LC_MESSAGES/unim-settings.po 를
    // 컴파일 타임에 바이너리에 포함한다. 소스 msgid 는 한국어이므로 index 0(원문)
    // = 한국어이고, en 폴더의 .po 가 영어 번역을 제공한다. 런타임 선택은
    // main.rs 의 select_bundled_translation 이 OS UI 언어에 따라 수행한다.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations");
    slint_build::compile_with_config("ui/settings.slint", config)
        .expect("slint compile failed");

    // 도움말 HTML 설치 경로(Linux). Makefile 의 PREFIX 가 가변(deb/rpm=/usr, 소스
    // 빌드=/usr/local)이라 컴파일 타임에 실제 $(DATADIR) 를 주입받는다. 미설정이면
    // option_env! 가 None 이 되고 런타임 후보 순회(/usr → /usr/local → 개발 폴백)가
    // 대신 처리한다 — 즉 이 주입은 "정답을 앞에 세우는" 최적화지 필수 조건이 아니다.
    println!("cargo:rerun-if-env-changed=UNIM_DATADIR");
    if let Ok(datadir) = std::env::var("UNIM_DATADIR") {
        println!("cargo:rustc-env=UNIM_DATADIR={datadir}");
    }

    // Slint(winit/렌더러) 초기화가 Windows 기본 1MB 메인 스레드 스택을 초과해
    // "main has overflowed its stack"으로 죽는 알려진 이슈가 있다(특히 디버그 빌드).
    // MSVC 링커에 16MB 스택을 요청한다.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:16777216");
    }

    // Windows 셸 아이콘(작업표시줄·탐색기·바로가기)은 exe 의 아이콘 리소스에서 온다.
    // Slint 의 `Window { icon: ... }` 은 창 좌상단만 바꾸므로 이것 없이는 기본 아이콘이
    // 뜬다. unim-tsf 가 언어바 아이콘에 쓰는 것과 같은 방식이다.
    //
    // VERSIONINFO 도 같은 자리에서 함께 임베드한다(Windows Defender 오탐,
    // 2026-09-03 — 배포 위생 대응. build-support/version_rc.rs 참고).
    #[cfg(windows)]
    {
        embed_version_rc(
            Some("../installer/assets/unim.ico"),
            "UNIM Korean IME — Settings",
            "unim-settings.exe",
            "VFT_APP",
        );
    }
}
