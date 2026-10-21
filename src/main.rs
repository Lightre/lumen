#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod image_loader;
mod navigation;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 740.0])
            .with_min_inner_size([480.0, 360.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Lumen",
        options,
        Box::new(|cc| Ok(Box::new(app::LumenApp::new(cc)))),
    )
}
