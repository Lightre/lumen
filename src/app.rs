use eframe::egui;

pub struct LumenApp;

impl LumenApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(22, 23, 26);
        visuals.window_fill = visuals.panel_fill;
        cc.egui_ctx.set_visuals(visuals);
        Self
    }
}

impl eframe::App for LumenApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            let center = ui.max_rect().center();
            let painter = ui.painter();
            let muted = egui::Color32::from_rgb(135, 139, 148);
            let icon = egui::Rect::from_center_size(
                center - egui::vec2(0.0, 76.0),
                egui::vec2(64.0, 48.0),
            );
            painter.rect_stroke(
                icon,
                8.0,
                egui::Stroke::new(1.5, muted),
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(icon.min + egui::vec2(45.0, 14.0), 4.0, muted);
            painter.line_segment(
                [
                    icon.min + egui::vec2(9.0, 37.0),
                    icon.min + egui::vec2(25.0, 21.0),
                ],
                egui::Stroke::new(1.5, muted),
            );
            painter.line_segment(
                [
                    icon.min + egui::vec2(25.0, 21.0),
                    icon.min + egui::vec2(43.0, 39.0),
                ],
                egui::Stroke::new(1.5, muted),
            );
            painter.text(
                center - egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                "Lumen",
                egui::FontId::proportional(34.0),
                egui::Color32::from_rgb(232, 234, 239),
            );
            painter.text(
                center + egui::vec2(0.0, 32.0),
                egui::Align2::CENTER_CENTER,
                "A little space for your photos.",
                egui::FontId::proportional(16.0),
                muted,
            );
        });
    }
}
