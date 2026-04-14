//! UNIM Windows 한글 입력기 GUI
//!
//! egui/eframe 기반 독립 실행형 한국어 입력 앱.
//! 코어 엔진을 in-process로 직접 사용하여 DBus/데몬 없이 동작합니다.

mod app;
mod input_handler;
mod tray;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 450.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("UNIM 한글 입력기"),
        ..Default::default()
    };

    eframe::run_native(
        "UNIM Korean IME",
        options,
        Box::new(|cc| Ok(Box::new(app::UnimApp::new(cc)))),
    )
}
