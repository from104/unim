fn main() {
    // ko/en 번들 번역: translations/<lang>/LC_MESSAGES/unim-settings.po 를
    // 컴파일 타임에 바이너리에 포함한다. 소스 msgid 는 한국어이므로 index 0(원문)
    // = 한국어이고, en 폴더의 .po 가 영어 번역을 제공한다. 런타임 선택은
    // main.rs 의 select_bundled_translation 이 OS UI 언어에 따라 수행한다.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations");
    slint_build::compile_with_config("ui/settings.slint", config)
        .expect("slint compile failed");

    // Slint(winit/렌더러) 초기화가 Windows 기본 1MB 메인 스레드 스택을 초과해
    // "main has overflowed its stack"으로 죽는 알려진 이슈가 있다(특히 디버그 빌드).
    // MSVC 링커에 16MB 스택을 요청한다.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:16777216");
    }
}
