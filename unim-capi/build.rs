fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libunim_capi.so.0");
}
