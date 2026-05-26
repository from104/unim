//! Build script: link soname + C-API drift guard.
//!
//! The shipped C header is the curated `include/unim.h`. To stop it silently
//! drifting from the Rust `#[no_mangle]` surface, we regenerate a header with
//! cbindgen into `OUT_DIR` and compare the *exported function set* against the
//! committed header. A mismatch only emits `cargo:warning` (never fails the
//! build) so packaging in read-only trees stays unaffected.

use std::collections::BTreeSet;
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libunim_capi.so.0");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=include/unim.h");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let committed = PathBuf::from(&crate_dir).join("include/unim.h");
    let committed_src = match fs::read_to_string(&committed) {
        Ok(s) => s,
        Err(e) => {
            println!("cargo:warning=C-API drift guard: cannot read {}: {e}", committed.display());
            return;
        }
    };

    let generated = match cbindgen::generate(&crate_dir) {
        Ok(b) => {
            let mut buf: Vec<u8> = Vec::new();
            b.write(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        }
        Err(e) => {
            // Generation failing should not break a normal build.
            println!("cargo:warning=C-API drift guard: cbindgen generate failed: {e}");
            return;
        }
    };

    // Optional: keep a copy for inspection.
    if let Ok(out) = env::var("OUT_DIR") {
        let _ = fs::write(PathBuf::from(out).join("unim_generated.h"), &generated);
    }

    let gen_fns = exported_fns(&generated);
    let hdr_fns = exported_fns(&committed_src);

    let missing: Vec<_> = gen_fns.difference(&hdr_fns).cloned().collect();
    let extra: Vec<_> = hdr_fns.difference(&gen_fns).cloned().collect();

    if !missing.is_empty() {
        println!(
            "cargo:warning=C-API drift: include/unim.h is MISSING {} exported fn(s): {}",
            missing.len(),
            missing.join(", ")
        );
    }
    if !extra.is_empty() {
        println!(
            "cargo:warning=C-API drift: include/unim.h declares {} fn(s) not exported by lib.rs: {}",
            extra.len(),
            extra.join(", ")
        );
    }
    if missing.is_empty() && extra.is_empty() {
        println!(
            "cargo:warning=C-API drift guard OK: {} exported functions in sync",
            gen_fns.len()
        );
    }
}

/// Collect `unim_*` identifiers that are immediately followed by `(` — i.e.
/// function declarations, not prose mentions in doc comments.
fn exported_fns(src: &str) -> BTreeSet<String> {
    let bytes = src.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while let Some(rel) = src[i..].find("unim_") {
        let start = i + rel;
        let mut end = start;
        while end < bytes.len() {
            let c = bytes[end];
            if c.is_ascii_alphanumeric() || c == b'_' {
                end += 1;
            } else {
                break;
            }
        }
        if end < bytes.len() && bytes[end] == b'(' {
            out.insert(src[start..end].to_string());
        }
        i = end.max(start + 1);
    }
    out
}
