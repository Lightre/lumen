use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::image_loader::{self, LoadedImage};
use crate::navigation::{Direction, Folder};
use crate::view::View;

struct OpenedImage {
    image: LoadedImage,
    folder: Folder,
    notice: Option<String>,
}

type LoadResult = Result<Option<OpenedImage>, String>;

enum OpenRequest {
    Pick,
    File(PathBuf),
    Navigate(Folder, Direction),
}

fn load_selection(path: PathBuf, max_side: u32) -> Result<OpenedImage, String> {
    let image = image_loader::load(&path, max_side)?;
    let (folder, notice) = match Folder::scan(&path) {
        Ok(folder) => (folder, None),
        Err(error) => (Folder::single(path), Some(error)),
    };
    Ok(OpenedImage {
        image,
        folder,
        notice,
    })
}

fn load_neighbor(
    mut folder: Folder,
    direction: Direction,
    max_side: u32,
) -> Result<OpenedImage, String> {
    let mut skipped = false;
    while let Some(next) = folder.neighbor(direction) {
        folder = next;
        if let Ok(image) = image_loader::load(folder.path(), max_side) {
            return Ok(OpenedImage {
                image,
                folder,
                notice: skipped.then(|| "Some unreadable images were skipped.".to_owned()),
            });
        }
        skipped = true;
    }
    Err("No other readable images in this direction.".to_owned())
}

struct DisplayedImage {
    texture: egui::TextureHandle,
    path: PathBuf,
    dimensions: [u32; 2],
    reduced: bool,
}

#[derive(Default)]
pub struct LumenApp {
    image: Option<DisplayedImage>,
    folder: Option<Folder>,
    pending: Option<Receiver<LoadResult>>,
    error: Option<String>,
    view: View,
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
        self.start_loading(ctx, path.map_or(OpenRequest::Pick, OpenRequest::File));
    }

    fn navigate(&mut self, ctx: &egui::Context, direction: Direction) {
        if let Some(folder) = &self.folder
            && folder.neighbor(direction).is_some()
        {
            self.start_loading(ctx, OpenRequest::Navigate(folder.clone(), direction));
        }
    }

    fn start_loading(&mut self, ctx: &egui::Context, request: OpenRequest) {
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
                let result = match request {
                    OpenRequest::Pick => {
                        let mut dialog = rfd::FileDialog::new()
                            .set_title("Open image")
                            .add_filter("Images", &["jpg", "jpeg", "png", "webp"]);
                        if let Some(directory) = directory {
                            dialog = dialog.set_directory(directory);
                        }
                        dialog
                            .pick_file()
                            .map(|path| load_selection(path, max_side))
                            .transpose()
                    }
                    OpenRequest::File(path) => load_selection(path, max_side).map(Some),
                    OpenRequest::Navigate(folder, direction) => {
                        load_neighbor(folder, direction, max_side).map(Some)
                    }
                };
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
            Ok(Some(opened)) => {
                self.view.fit();
                let loaded = opened.image;
                self.folder = Some(opened.folder);
                self.error = opened.notice;
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
        let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if fullscreen
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Home)) {
            self.view.fit();
        }
        let mut zoom_request = None;
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
        } else {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
            {
                self.navigate(&ctx, Direction::Previous);
            } else if ctx
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
            {
                self.navigate(&ctx, Direction::Next);
            }
        }

        if !fullscreen {
            egui::Panel::top("toolbar").show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Lumen").strong());
                    ui.separator();
                    if ui
                        .add_enabled(self.pending.is_none(), egui::Button::new("Open image"))
                        .on_hover_text("Ctrl+O")
                        .clicked()
                    {
                        self.open(&ctx, None);
                    }
                    let ready = self.pending.is_none();
                    let previous = ready
                        && self
                            .folder
                            .as_ref()
                            .is_some_and(|folder| folder.neighbor(Direction::Previous).is_some());
                    let next = ready
                        && self
                            .folder
                            .as_ref()
                            .is_some_and(|folder| folder.neighbor(Direction::Next).is_some());
                    if ui
                        .add_enabled(previous, egui::Button::new("Previous"))
                        .on_hover_text("Left arrow")
                        .clicked()
                    {
                        self.navigate(&ctx, Direction::Previous);
                    }
                    if ui
                        .add_enabled(next, egui::Button::new("Next"))
                        .on_hover_text("Right arrow")
                        .clicked()
                    {
                        self.navigate(&ctx, Direction::Next);
                    }
                    if let Some(folder) = &self.folder {
                        let (position, count) = folder.position();
                        ui.weak(format!("{position} / {count}"));
                    }
                    ui.separator();
                    ui.add_enabled_ui(self.image.is_some(), |ui| {
                        if ui
                            .selectable_label(self.view.is_fit(), "Fit")
                            .on_hover_text("Fit to window · Home")
                            .clicked()
                        {
                            self.view.fit();
                        }
                        if ui
                            .button("100%")
                            .on_hover_text("One image pixel per screen pixel")
                            .clicked()
                        {
                            self.view.actual_size();
                        }
                        if ui.button("−").on_hover_text("Zoom out").clicked() {
                            zoom_request = Some(1.0 / 1.25);
                        }
                        if ui.button("+").on_hover_text("Zoom in").clicked() {
                            zoom_request = Some(1.25);
                        }
                    });
                    if ui
                        .button("Full screen")
                        .on_hover_text("F11 · Esc to exit")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                    }
                    if self.pending.is_some() {
                        ui.spinner();
                        ui.weak("Opening image…");
                    }
                });
                ui.add_space(6.0);
            });
        }

        if !fullscreen || self.error.is_some() {
            egui::Panel::bottom("status").show(ui, |ui| {
                if let Some(error) = self.error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(error)
                                .color(egui::Color32::from_rgb(245, 160, 150)),
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
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(image) = &self.image {
                let bounds = ui.max_rect();
                let source = egui::vec2(image.dimensions[0] as f32, image.dimensions[1] as f32);
                let pixels_per_point = ctx.pixels_per_point();
                let response = ui.allocate_rect(bounds, egui::Sense::click_and_drag());
                if response.double_clicked() {
                    if self.view.is_fit() {
                        self.view.actual_size();
                    } else {
                        self.view.fit();
                    }
                }
                if let Some(factor) = zoom_request {
                    self.view
                        .zoom_at(factor, bounds.center(), bounds, source, pixels_per_point);
                }
                if response.hovered() {
                    let scroll = ctx.input(|input| input.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        let anchor = ctx
                            .input(|input| input.pointer.hover_pos())
                            .unwrap_or(bounds.center());
                        self.view.zoom_at(
                            (scroll * 0.003).clamp(-2.0, 2.0).exp(),
                            anchor,
                            bounds,
                            source,
                            pixels_per_point,
                        );
                    }
                }
                let rect = self.view.rect(bounds, source, pixels_per_point);
                let can_pan = rect.width() > bounds.width() || rect.height() > bounds.height();
                if can_pan {
                    response.clone().on_hover_cursor(egui::CursorIcon::Grab);
                    if response.dragged_by(egui::PointerButton::Primary) {
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                        self.view.pan(
                            ctx.input(|input| input.pointer.delta()),
                            bounds,
                            source,
                            pixels_per_point,
                        );
                    }
                }
                let rect = self.view.rect(bounds, source, pixels_per_point);
                let painter = ui.painter().with_clip_rect(bounds);
                painter.image(
                    image.texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                if !fullscreen {
                    let zoom = self.view.scale(bounds, source, pixels_per_point) * 100.0;
                    painter.text(
                        bounds.right_bottom() - egui::vec2(12.0, 12.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("{zoom:.0}%"),
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(170),
                    );
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn save_image(path: &std::path::Path) {
        image::RgbaImage::from_pixel(3, 2, image::Rgba([100, 150, 200, 255]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn skips_broken_and_deleted_neighbors_in_both_directions() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("photo1.png");
        let last = directory.path().join("photo3.png");
        save_image(&first);
        std::fs::write(directory.path().join("photo2.png"), b"broken").unwrap();
        save_image(&last);
        let opened = load_selection(first.clone(), 1024).unwrap();
        let next = load_neighbor(opened.folder, Direction::Next, 1024).unwrap();
        assert_eq!(next.image.path.file_name().unwrap(), "photo3.png");
        assert_eq!(next.folder.position(), (3, 3));
        assert!(next.notice.is_some());
        let previous = load_neighbor(next.folder.clone(), Direction::Previous, 1024).unwrap();
        assert_eq!(previous.image.path.file_name().unwrap(), "photo1.png");
        std::fs::remove_file(first).unwrap();
        assert!(load_neighbor(next.folder, Direction::Previous, 1024).is_err());
    }

    #[test]
    fn opening_another_folder_replaces_the_navigation_list() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        save_image(&first.path().join("one.png"));
        save_image(&first.path().join("two.png"));
        save_image(&second.path().join("other.png"));
        let opened = load_selection(first.path().join("one.png"), 1024).unwrap();
        assert_eq!(opened.folder.position(), (1, 2));
        let opened = load_selection(second.path().join("other.png"), 1024).unwrap();
        assert_eq!(opened.folder.position(), (1, 1));
        assert!(opened.folder.neighbor(Direction::Next).is_none());
    }
}
