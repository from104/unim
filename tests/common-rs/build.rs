//! `tests/common` 의 C 코드를 그대로 컴파일해 링크한다.
//!
//! Rust 쪽에 스펙·필드 엔진을 **다시 구현하지 않는 것**이 요점이다. 미러를
//! 두면 언젠가 어긋나지만, 같은 오브젝트를 링크하면 어긋날 수가 없다.
//!
//! `unim_test_dbus.c` 와 `unim_test.c` 는 gio 의존이라 뺀다 — Rust 앱은
//! 데몬 통신을 자기 방식으로 한다.

use std::path::PathBuf;

fn main() {
    let common = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("common");

    let sources = ["unim_test_spec.c", "unim_test_log.c", "unim_test_field.c"];

    let mut build = cc::Build::new();
    build.include(&common).flag_if_supported("-std=c11");
    for s in &sources {
        let p = common.join(s);
        println!("cargo:rerun-if-changed={}", p.display());
        build.file(p);
    }
    println!("cargo:rerun-if-changed={}", common.join("unim_test_spec.h").display());
    println!("cargo:rerun-if-changed={}", common.join("unim_test_log.h").display());
    println!("cargo:rerun-if-changed={}", common.join("unim_test_field.h").display());

    build.compile("unim_test_common_c");
}
