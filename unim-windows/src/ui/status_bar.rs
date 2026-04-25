//! 한/영 모드 상태 표시 바

use unim::config::InputCategory;

/// 상태 바 UI를 그립니다.
pub fn show(ui: &mut egui::Ui, category: InputCategory) {
    let (label, color) = match category {
        InputCategory::Korean => ("한", egui::Color32::from_rgb(0, 120, 215)),
        InputCategory::English => ("EN", egui::Color32::from_rgb(100, 100, 100)),
    };

    ui.horizontal(|ui| {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 28.0)),
            0.0,
            color,
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(label)
                .color(egui::Color32::WHITE)
                .size(16.0)
                .strong(),
        );
        ui.add_space(4.0);
        let mode_text = match category {
            InputCategory::Korean => "한국어 입력 모드",
            InputCategory::English => "English Input Mode",
        };
        ui.label(
            egui::RichText::new(mode_text)
                .color(egui::Color32::from_rgb(220, 220, 220))
                .size(13.0),
        );
    });
}
