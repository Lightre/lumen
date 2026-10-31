#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chrome;
mod image_loader;
mod navigation;
#[cfg(feature = "perf")]
mod perf;
mod view;

fn main() -> eframe::Result {
    let startup_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(feature = "perf")]
    let measurement =
        perf::Config::from_args().map_err(|error| eframe::Error::AppCreation(Box::new(error)))?;
    #[cfg(feature = "perf")]
    let startup_path = measurement
        .as_ref()
        .map(|config| config.first_image.clone())
        .or(startup_path);
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("The embedded application icon must be valid");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 740.0])
            .with_min_inner_size([480.0, 360.0])
            .with_icon(icon),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Lumen",
        options,
        Box::new(move |cc| {
            let app = app::LumenApp::new(cc, startup_path);
            #[cfg(feature = "perf")]
            let app = app.with_measurement(measurement)?;
            Ok(Box::new(app))
        }),
    )
}
