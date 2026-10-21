use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::image_loader::{self, LoadedImage};

struct DisplayedImage {
    texture: egui::TextureHandle,
    path: PathBuf,
    dimensions: [u32; 2],
    reduced: bool,
}

#[derive(Default)]
pub struct LumenApp {
    image: Option<DisplayedImage>,
    pending: Option<Receiver<Result<Option<LoadedImage>, String>>>,
    error: Option<String>,
}

impl LumenApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(22, 23, 26);
        visuals.window_fill = visuals.panel_fill;
        cc.egui_ctx.set_visuals(visuals);
        Self::default()
    }

    fn open(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let max_side = ctx.input(|input| input.max_texture_side).min(8192) as u32;
        let directory = self
            .image
            .as_ref()
            .and_then(|image| image.path.parent().map(PathBuf::from));
        match std::thread::Builder::new()
            .name("image-loader".to_owned())
            .spawn(move || {
                let path = path.or_else(|| {
                    let mut dialog = rfd::FileDialog::new()
                        .set_title("Open image")
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp"]);
                    if let Some(directory) = directory {
                        dialog = dialog.set_directory(directory);
                    }
                    dialog.pick_file()
                });
                let result = path
                    .map(|path| image_loader::load(&path, max_side))
                    .transpose();
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.pending = Some(receiver);
                self.error = None;
            }
            Err(_) => {
                self.error =
                    Some("The image loader could not be started. Please try again.".to_owned())
            }
        }
    }

    fn finish_loading(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.pending else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("The image loader stopped unexpectedly. Please try again.".to_owned())
            }
        };
        self.pending = None;
        match result {
            Ok(Some(loaded)) => {
                let title = format!(
                    "{} — Lumen",
                    loaded
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
                self.image = Some(DisplayedImage {
                    texture: ctx.load_texture("photo", loaded.pixels, egui::TextureOptions::LINEAR),
                    path: loaded.path,
                    dimensions: loaded.dimensions,
                    reduced: loaded.reduced,
                });
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }
}

impl eframe::App for LumenApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.finish_loading(&ctx);
        let shortcut =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::O));
        let dropped = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_path_buf())
        });
        if let Some(path) = dropped {
            self.open(&ctx, Some(path));
        } else if shortcut {
            self.open(&ctx, None);
        }

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Lumen").strong());
                ui.separator();
                if ui
                    .add_enabled(self.pending.is_none(), egui::Button::new("Open image"))
                    .on_hover_text("Ctrl+O")
                    .clicked()
                {
                    self.open(&ctx, None);
                }
                if self.pending.is_some() {
                    ui.spinner();
                    ui.weak("Opening image…");
                }
            });
            ui.add_space(6.0);
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            if let Some(error) = self.error.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(error).color(egui::Color32::from_rgb(245, 160, 150)),
                    );
                    if ui.small_button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            }
            if let Some(image) = &self.image {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            image.path.file_name().unwrap_or_default().to_string_lossy(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(image.path.to_string_lossy());
                    ui.separator();
                    ui.weak(format!("{} × {}", image.dimensions[0], image.dimensions[1]));
                    if image.reduced {
                        ui.weak("Reduced preview");
                    }
                });
            } else {
                ui.weak("JPEG · PNG · WebP");
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(image) = &self.image {
                let available = ui.available_size();
                let native_size = image.texture.size_vec2();
                let scale = (available.x / native_size.x)
                    .min(available.y / native_size.y)
                    .min(1.0);
                let rect =
                    egui::Rect::from_center_size(ui.max_rect().center(), native_size * scale);
                ui.painter().image(
                    image.texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                return;
            }
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
                "Open an image or drop one here.",
                egui::FontId::proportional(16.0),
                muted,
            );
        });
    }
}
