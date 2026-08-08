//! `tests/common` 의 C 코드를 그대로 컴파일해 링크한다.
//!
//! Rust 쪽에 스펙·필드 엔진을 **다시 구현하지 않는 것**이 요점이다. 미러를
//! 두면 언젠가 어긋나지만, 같은 오브젝트를 링크하면 어긋날 수가 없다.
//!
//! `unim_test_dbus.c` 는 gio 의존이라 `dbus` 기능으로만 붙인다 — 상태 패널
//! 문구를 Rust 로 옮겨 적으면 6개 앱 화면이 어긋나므로, 쓰는 앱은 반드시
//! 이 기능을 켜서 **같은 함수**를 부른다.

use std::path::{Path, PathBuf};

#[cfg(feature = "dbus")]
fn add_dbus(build: &mut cc::Build, common: &Path) {
    let lib = pkg_config::Config::new()
        .probe("gio-2.0")
        .expect("gio-2.0 개발 패키지가 필요하다 (libglib2.0-dev)");
    for p in &lib.include_paths {
        build.include(p);
    }
    let p = common.join("unim_test_dbus.c");
    println!("cargo:rerun-if-changed={}", p.display());
    println!(
        "cargo:rerun-if-changed={}",
        common.join("unim_test_dbus.h").display()
    );
    build.file(p);
}

#[cfg(not(feature = "dbus"))]
fn add_dbus(_build: &mut cc::Build, _common: &Path) {}

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

    add_dbus(&mut build, &common);

    build.compile("unim_test_common_c");
}
