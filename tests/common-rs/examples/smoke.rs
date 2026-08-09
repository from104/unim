//! FFI 껍데기가 C 구현과 정말 같은 것을 보는지 확인한다.
fn main() {
    unim_test_common::log_init("rs-smoke");
    unim_test_common::log_env("rust-ffi");

    let m = unim_test_common::metrics();
    println!("창 {}x{}  필드높이 {}  배경 0x{:06x}",
             m.win_width, m.win_height, m.field_h, m.col_bg);
    println!("상태 라벨 {:?}", unim_test_common::status_labels());

    let n = unim_test_common::n_core_fields();
    let mut fields: Vec<Box<unim_test_common::Field>> =
        (0..n).map(unim_test_common::Field::new).collect();
    unim_test_common::layout(&mut fields, 200, m.win_width, 1.0);
    println!("필드 {}개, 첫 필드 id={} 라벨={} y={}",
             n, fields[0].id(), fields[0].label(), fields[0].y);

    // ㄹ 연타 — C 판 스모크와 같은 시나리오
    let f = &mut fields[0];
    f.preedit_start();
    for _ in 0..3 {
        f.set_preedit("ㄹ", -1);
        f.commit("ㄹ");
    }
    f.preedit_end();
    println!(">>> rendered = {:?}", f.rendered());
    assert_eq!(f.rendered(), "ㄹㄹㄹ");

    let pw = &mut fields[3];
    pw.commit("비밀");
    println!(">>> password display = {:?}  rendered = {:?}",
             pw.display(), pw.rendered());
    assert_eq!(pw.display(), "••");

    unim_test_common::log_shutdown();
    println!("✅ FFI 껍데기가 C 구현과 일치");
}
