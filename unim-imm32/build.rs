fn main() {
    // Pass the module-definition file so x86 (i686) emits UNDECORATED stdcall exports
    // (ImeInquire, not _ImeInquire@12). Harmless on x64.
    //
    // `/DEF:` is MSVC linker syntax. Emitting it unconditionally breaks non-MSVC
    // link steps: on a native Linux `cargo build --workspace` rust-lld reads
    // `/DEF:<path>` as a (missing) input file and aborts. The production .ime is
    // linked by MSVC on GitHub Actions, so gate the flag on the MSVC target env
    // (the mingw-gnu sanity cross-check only runs `cargo check`, which never links).
    println!("cargo:rerun-if-changed=unim_imm32.def");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let def_path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("unim_imm32.def");
        println!("cargo:rustc-link-arg=/DEF:{}", def_path.display());
    }
    // imm32.dll is linked via `#[link(name = "imm32")]` in globals.rs; emitting
    // `cargo:rustc-link-lib=dylib=imm32` here too is redundant (it requested imm32
    // twice). The link attribute is authoritative on both bitnesses.

    // Embed unim_imm32.rc (STRINGTABLE id 1 = layout display name) into the
    // cdylib (.dll -> copied to .ime). embed-resource invokes rc.exe + links the
    // compiled .res; it auto-selects the rc target arch, so the SAME call covers
    // BOTH x86_64 and i686 builds (mirrors unim-tsf/build.rs).
    //
    // O8 note: embed-resource emits a `/DEF`-compatible extra link input (the .res
    // object), so it does NOT conflict with the `/DEF:` link-arg above — both are
    // independent inputs to the MSVC linker.
    #[cfg(windows)]
    {
        embed_resource::compile("unim_imm32.rc", embed_resource::NONE);
        println!("cargo:rerun-if-changed=unim_imm32.rc");
    }
}
