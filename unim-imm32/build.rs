fn main() {
    // Pass the module-definition file so x86 (i686) emits UNDECORATED stdcall exports
    // (ImeInquire, not _ImeInquire@12). Harmless on x64.
    let def_path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("unim_imm32.def");
    println!("cargo:rustc-link-arg=/DEF:{}", def_path.display());
    println!("cargo:rerun-if-changed=unim_imm32.def");
    // imm32.dll is linked via `#[link(name = "imm32")]` in globals.rs; emitting
    // `cargo:rustc-link-lib=dylib=imm32` here too is redundant (it requested imm32
    // twice). The link attribute is authoritative on both bitnesses.
}
