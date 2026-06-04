fn main() {
    slint_build::compile("ui/settings.slint").expect("slint compile failed");

    // Slint(winit/렌더러) 초기화가 Windows 기본 1MB 메인 스레드 스택을 초과해
    // "main has overflowed its stack"으로 죽는 알려진 이슈가 있다(특히 디버그 빌드).
    // MSVC 링커에 16MB 스택을 요청한다.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:16777216");
    }
}
