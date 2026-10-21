use eframe::egui;

pub const CANVAS: egui::Color32 = egui::Color32::from_rgb(20, 21, 24);
pub const SURFACE: egui::Color32 = egui::Color32::from_rgb(27, 29, 33);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(145, 151, 161);

pub fn configure(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = SURFACE;
    visuals.window_fill = SURFACE;
    visuals.override_text_color = Some(egui::Color32::from_rgb(218, 221, 227));
    visuals.selection.bg_fill = egui::Color32::from_rgb(65, 71, 81);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 43, 49));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(37, 40, 46);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 57, 65);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(67, 73, 83);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(7);
    }
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    });
}

pub fn open_button(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new("Open image").color(egui::Color32::from_rgb(30, 32, 36)),
        )
        .fill(egui::Color32::from_rgb(230, 232, 235))
        .min_size(egui::vec2(112.0, 34.0)),
    )
    .on_hover_text("Open a photo · Ctrl+O")
}

#[derive(Clone, Copy)]
pub enum Icon {
    Previous,
    Next,
    Minus,
    Plus,
    FullScreen,
}

pub fn icon_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        let style = ui.style().interact(&response);
        if response.hovered() || response.has_focus() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 7.0, style.bg_fill);
        }
        if response.has_focus() {
            ui.painter()
                .rect_stroke(rect, 7.0, style.fg_stroke, egui::StrokeKind::Inside);
        }
        let color = if ui.is_enabled() {
            style.fg_stroke.color
        } else {
            egui::Color32::from_gray(75)
        };
        let stroke = egui::Stroke::new(1.7, color);
        let c = rect.center();
        let point = |x, y| c + egui::vec2(x, y);
        let line = |a, b| ui.painter().line_segment([a, b], stroke);
        match icon {
            Icon::Previous => {
                line(point(2.5, -5.0), point(-2.5, 0.0));
                line(point(-2.5, 0.0), point(2.5, 5.0));
            }
            Icon::Next => {
                line(point(-2.5, -5.0), point(2.5, 0.0));
                line(point(2.5, 0.0), point(-2.5, 5.0));
            }
            Icon::Minus | Icon::Plus => {
                line(point(-5.0, 0.0), point(5.0, 0.0));
                if matches!(icon, Icon::Plus) {
                    line(point(0.0, -5.0), point(0.0, 5.0));
                }
            }
            Icon::FullScreen => {
                for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    line(point(x * 6.0, y * 2.0), point(x * 6.0, y * 6.0));
                    line(point(x * 6.0, y * 6.0), point(x * 2.0, y * 6.0));
                }
            }
        }
        response.on_hover_text(label)
    })
    .inner
}
