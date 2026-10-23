use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::chrome::{self, Icon};
use crate::image_loader::{self, Animation, LoadedImage};
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
    playback: Option<Playback>,
}

fn update_texture(texture: &mut egui::TextureHandle, pixels: Arc<egui::ColorImage>) {
    if texture.size() == pixels.size {
        // Replace every pixel without reallocating same-sized GPU storage.
        texture.set_partial([0, 0], pixels, egui::TextureOptions::LINEAR);
    } else {
        texture.set(pixels, egui::TextureOptions::LINEAR);
    }
}

struct Playback {
    animation: Animation,
    ends: Vec<Duration>,
    frame: usize,
    started: Instant,
    elapsed: Duration,
    playing: bool,
    finished: bool,
}

impl Playback {
    fn new(animation: Animation, now: Instant) -> Self {
        let mut total = Duration::ZERO;
        let ends = animation
            .frames
            .iter()
            .map(|frame| {
                total += frame.delay;
                total
            })
            .collect();
        Self {
            animation,
            ends,
            frame: 0,
            started: now,
            elapsed: Duration::ZERO,
            playing: true,
            finished: false,
        }
    }

    fn toggle(&mut self, now: Instant) {
        if self.playing {
            self.elapsed += now.saturating_duration_since(self.started);
            self.playing = false;
        } else {
            if self.finished {
                self.elapsed = Duration::ZERO;
                self.finished = false;
            }
            self.started = now;
            self.playing = true;
        }
    }

    fn advance(&mut self, now: Instant) -> Option<Duration> {
        let elapsed = self.elapsed
            + if self.playing {
                now.saturating_duration_since(self.started)
            } else {
                Duration::ZERO
            };
        let cycle = *self.ends.last().expect("An animation contains frames");
        if let Some(plays) = self.animation.plays
            && elapsed >= cycle * plays
        {
            self.elapsed = cycle * plays;
            self.frame = self.animation.frames.len() - 1;
            self.playing = false;
            self.finished = true;
            return None;
        }
        let phase = Duration::from_nanos((elapsed.as_nanos() % cycle.as_nanos()) as u64);
        self.frame = self.ends.partition_point(|end| *end <= phase);
        self.playing.then_some(self.ends[self.frame] - phase)
    }
}

#[derive(Default)]
pub struct LumenApp {
    image: Option<DisplayedImage>,
    folder: Option<Folder>,
    pending: Option<Receiver<LoadResult>>,
    error: Option<String>,
    view: View,
    startup_path: Option<PathBuf>,
    brand: Option<egui::TextureHandle>,
}

impl LumenApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup_path: Option<PathBuf>) -> Self {
        chrome::configure(&cc.egui_ctx);
        let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
            .expect("The embedded brand asset must be valid");
        let brand = cc.egui_ctx.load_texture(
            "lumen-brand",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        Self {
            startup_path,
            brand: Some(brand),
            ..Self::default()
        }
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

    fn navigation_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if chrome::open_button(ui, self.pending.is_none()).clicked() {
            self.open(ctx, None);
        }
        ui.add_space(4.0);
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
        if chrome::icon_button(ui, Icon::Previous, "Previous image · Left arrow", previous)
            .clicked()
        {
            self.navigate(ctx, Direction::Previous);
        }
        if chrome::icon_button(ui, Icon::Next, "Next image · Right arrow", next).clicked() {
            self.navigate(ctx, Direction::Next);
        }
        if let Some(folder) = &self.folder {
            let (position, count) = folder.position();
            ui.weak(format!("{position} / {count}"));
        }
    }

    fn view_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> Option<f32> {
        let mut zoom_request = None;
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
            if chrome::icon_button(ui, Icon::Minus, "Zoom out", true).clicked() {
                zoom_request = Some(1.0 / 1.25);
            }
            if chrome::icon_button(ui, Icon::Plus, "Zoom in", true).clicked() {
                zoom_request = Some(1.25);
            }
            ui.separator();
            if chrome::icon_button(ui, Icon::RotateLeft, "Rotate left · Shift+R", true).clicked() {
                self.view.rotate(false);
            }
            if chrome::icon_button(ui, Icon::RotateRight, "Rotate right · R", true).clicked() {
                self.view.rotate(true);
            }
        });
        ui.separator();
        if chrome::icon_button(
            ui,
            Icon::FullScreen,
            "Full screen · F11 · Esc to exit",
            true,
        )
        .clicked()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        zoom_request
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
                        let mut dialog = rfd::FileDialog::new().set_title("Open image").add_filter(
                            "Images",
                            &[
                                "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "heic",
                                "heif",
                            ],
                        );
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
                self.view = View::default();
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
                let texture = if let Some(previous) = self.image.take() {
                    let mut texture = previous.texture;
                    update_texture(&mut texture, loaded.pixels);
                    texture
                } else {
                    ctx.load_texture("photo", loaded.pixels, egui::TextureOptions::LINEAR)
                };
                self.image = Some(DisplayedImage {
                    texture,
                    path: loaded.path,
                    dimensions: loaded.dimensions,
                    reduced: loaded.reduced,
                    playback: loaded
                        .animation
                        .map(|animation| Playback::new(animation, Instant::now())),
                });
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }
}

impl LumenApp {
    fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.finish_loading(&ctx);
        if let Some(image) = &mut self.image
            && let Some(playback) = &mut image.playback
        {
            let toggle = ctx.input_mut(|input| {
                let pressed = input.events.iter().any(|event| matches!(event,
                    egui::Event::Key { key: egui::Key::Space, pressed: true, repeat: false, modifiers, .. }
                    if modifiers.is_none()
                ));
                input.consume_key(egui::Modifiers::NONE, egui::Key::Space);
                pressed
            });
            let now = Instant::now();
            let previous = playback.frame;
            // Advance before toggling so a key press at the end restarts the GIF.
            playback.advance(now);
            if toggle {
                playback.toggle(now);
            }
            if let Some(delay) = playback.advance(now) {
                ctx.request_repaint_after(delay);
            }
            if playback.frame != previous {
                update_texture(
                    &mut image.texture,
                    playback.animation.frames[playback.frame].pixels.clone(),
                );
            }
        }
        if let Some(path) = self.startup_path.take() {
            self.open(&ctx, Some(path));
        }
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
        if self.image.is_some() {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::R)) {
                self.view.rotate(false);
            } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::R))
            {
                self.view.rotate(true);
            }
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
            let narrow = ui.available_width() < 760.0;
            egui::Panel::top("toolbar")
                .resizable(false)
                .exact_size(if narrow { 96.0 } else { 56.0 })
                .frame(
                    egui::Frame::new()
                        .fill(chrome::SURFACE)
                        .inner_margin(egui::Margin::symmetric(12, 8)),
                )
                .show(ui, |ui| {
                    let content = ui.max_rect();
                    let mut controls = content;
                    controls.max.x -= 32.0;
                    ui.scope_builder(egui::UiBuilder::new().max_rect(controls), |ui| {
                        ui.horizontal(|ui| {
                            self.navigation_controls(ui, &ctx);
                            if !narrow {
                                ui.separator();
                                zoom_request = self.view_controls(ui, &ctx);
                            }
                        });
                        if narrow {
                            ui.horizontal(|ui| {
                                zoom_request = self.view_controls(ui, &ctx);
                            });
                        }
                    });
                    if self.pending.is_some() {
                        let indicator = egui::Rect::from_center_size(
                            egui::pos2(content.right() - 12.0, content.top() + 17.0),
                            egui::vec2(16.0, 16.0),
                        );
                        ui.put(indicator, egui::Spinner::new().size(16.0))
                            .on_hover_text("Opening image…");
                    }
                });
        }

        if !fullscreen || self.error.is_some() {
            egui::Panel::bottom("status")
                .resizable(false)
                .exact_size(if self.error.is_some() { 80.0 } else { 36.0 })
                .frame(
                    egui::Frame::new()
                        .fill(chrome::SURFACE)
                        .inner_margin(egui::Margin::symmetric(12, 6)),
                )
                .show(ui, |ui| {
                    ui.spacing_mut().interact_size.y = 20.0;
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
                            let mut details =
                                format!("{} × {}", image.dimensions[0], image.dimensions[1]);
                            if image.reduced {
                                details.push_str("  ·  Reduced preview");
                            }
                            if let Some(playback) = &image.playback
                                && !playback.playing
                            {
                                details.push_str(if playback.finished {
                                    "  ·  Finished"
                                } else {
                                    "  ·  Paused"
                                });
                            }
                            let width = ui
                                .painter()
                                .layout_no_wrap(
                                    details.clone(),
                                    egui::FontId::proportional(12.0),
                                    chrome::MUTED,
                                )
                                .size()
                                .x;
                            let name_width = (ui.available_width() - width - 16.0).max(0.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(name_width, 20.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            image
                                                .path
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy(),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(image.path.to_string_lossy());
                                },
                            );
                            ui.label(egui::RichText::new(details).size(12.0).color(chrome::MUTED));
                        });
                    } else {
                        ui.label(
                            egui::RichText::new("JPEG · PNG · WebP · GIF · BMP · TIFF · HEIC")
                                .size(12.0)
                                .color(chrome::MUTED),
                        );
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(chrome::CANVAS)
                    .inner_margin(if fullscreen { 0 } else { 12 }),
            )
            .show(ui, |ui| {
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
                        self.view.zoom_at(
                            factor,
                            bounds.center(),
                            bounds,
                            source,
                            pixels_per_point,
                        );
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
                    painter.add(self.view.mesh(image.texture.id(), rect));
                    if !fullscreen {
                        let zoom = self.view.scale(bounds, source, pixels_per_point) * 100.0;
                        let label = painter.layout_no_wrap(
                            format!("{zoom:.0}%"),
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_gray(220),
                        );
                        let size = label.size() + egui::vec2(16.0, 8.0);
                        let badge = egui::Rect::from_min_size(
                            bounds.right_bottom() - size - egui::vec2(8.0, 8.0),
                            size,
                        );
                        painter.rect_filled(badge, 6.0, egui::Color32::from_black_alpha(190));
                        painter.galley(
                            badge.min + egui::vec2(8.0, 4.0),
                            label,
                            egui::Color32::WHITE,
                        );
                    }
                    return;
                }
                let bounds = ui.max_rect();
                let height = bounds.height().min(270.0);
                let area = egui::Rect::from_center_size(
                    bounds.center(),
                    egui::vec2(bounds.width().min(360.0), height),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(area)
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                    |ui| {
                        let icon_size = (height * 0.32).clamp(48.0, 88.0);
                        if let Some(brand) = &self.brand {
                            ui.add(egui::Image::new((
                                brand.id(),
                                egui::vec2(icon_size, icon_size),
                            )));
                        }
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("Lumen")
                                .size(28.0)
                                .color(egui::Color32::from_rgb(240, 241, 237)),
                        );
                        ui.label(
                            egui::RichText::new("Open a photo or drop it here.")
                                .size(14.0)
                                .color(chrome::MUTED),
                        );
                        ui.add_space(12.0);
                        if chrome::open_button(ui, self.pending.is_none()).clicked() {
                            self.open(&ctx, None);
                        }
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Ctrl+O  ·  JPEG, PNG, WebP, GIF, BMP, TIFF, HEIC")
                                .size(11.0)
                                .color(chrome::MUTED),
                        );
                    },
                );
            });
    }
}

impl eframe::App for LumenApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(plays: Option<u32>) -> Animation {
        Animation {
            frames: [100, 200, 300]
                .into_iter()
                .map(|millis| image_loader::AnimationFrame {
                    pixels: std::sync::Arc::new(egui::ColorImage::filled(
                        [1, 1],
                        egui::Color32::WHITE,
                    )),
                    delay: Duration::from_millis(millis),
                })
                .collect(),
            plays,
        }
    }

    #[test]
    fn playback_obeys_delays_and_catches_up_without_iterating_missed_cycles() {
        let now = Instant::now();
        let mut playback = Playback::new(animation(None), now);
        assert_eq!(playback.advance(now), Some(Duration::from_millis(100)));
        assert_eq!(
            playback.advance(now + Duration::from_millis(100)),
            Some(Duration::from_millis(200))
        );
        assert_eq!(playback.frame, 1);
        playback.advance(now + Duration::from_millis(300));
        assert_eq!(playback.frame, 2);
        playback.advance(now + Duration::from_millis(600));
        assert_eq!(playback.frame, 0);
        assert_eq!(
            playback.advance(now + Duration::from_secs(3600) + Duration::from_millis(150)),
            Some(Duration::from_millis(150))
        );
        assert_eq!(playback.frame, 1);
    }

    #[test]
    fn pause_preserves_remaining_frame_time_and_finite_playback_can_restart() {
        let now = Instant::now();
        let mut playback = Playback::new(animation(Some(2)), now);
        playback.toggle(now + Duration::from_millis(50));
        assert_eq!(playback.advance(now + Duration::from_secs(30)), None);
        assert_eq!(playback.frame, 0);
        playback.toggle(now + Duration::from_secs(30));
        assert_eq!(
            playback.advance(now + Duration::from_secs(30)),
            Some(Duration::from_millis(50))
        );
        playback.advance(now + Duration::from_secs(31));
        assert_eq!(playback.frame, 2);
        assert!(!playback.finished);
        assert_eq!(playback.advance(now + Duration::from_secs(32)), None);
        assert!(playback.finished);
        assert_eq!(playback.frame, 2);
        assert_eq!(playback.advance(now + Duration::from_secs(40)), None);
        assert_eq!(playback.frame, 2);
        playback.toggle(now + Duration::from_secs(40));
        assert_eq!(
            playback.advance(now + Duration::from_secs(40)),
            Some(Duration::from_millis(100))
        );
        assert_eq!(playback.frame, 0);
        assert!(!playback.finished);
    }

    #[test]
    fn space_toggles_animation_once_and_ignores_key_repeat_and_modifiers() {
        let ctx = egui::Context::default();
        chrome::configure(&ctx);
        let mut app = LumenApp {
            image: Some(DisplayedImage {
                texture: ctx.load_texture(
                    "animation",
                    egui::ColorImage::filled([1, 1], egui::Color32::WHITE),
                    egui::TextureOptions::LINEAR,
                ),
                path: "motion.gif".into(),
                dimensions: [1, 1],
                reduced: false,
                playback: Some(Playback::new(animation(None), Instant::now())),
            }),
            ..Default::default()
        };
        for (repeat, modifiers, playing, release) in [
            (false, egui::Modifiers::NONE, false, false),
            (true, egui::Modifiers::NONE, false, true),
            (false, egui::Modifiers::CTRL, false, true),
            (false, egui::Modifiers::NONE, true, true),
        ] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480.0, 360.0),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::Space,
                    physical_key: None,
                    pressed: true,
                    repeat,
                    modifiers,
                }],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| app.show(ui));
            output.textures_delta.clear();
            for clipped in &output.shapes {
                assert_visible_shapes(
                    &clipped.shape,
                    clipped.clip_rect,
                    &mut false,
                    &mut Vec::new(),
                );
            }
            assert_eq!(
                app.image
                    .as_ref()
                    .unwrap()
                    .playback
                    .as_ref()
                    .unwrap()
                    .playing,
                playing,
                "repeat={repeat}, modifiers={modifiers:?}"
            );
            // Release between physical presses so egui does not infer a repeat.
            if release {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key: egui::Key::Space,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers,
                        }],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
        }
    }

    fn assert_visible_shapes(
        shape: &egui::epaint::Shape,
        clip: egui::Rect,
        metadata_seen: &mut bool,
        positions: &mut Vec<egui::Rect>,
    ) {
        match shape {
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    assert_visible_shapes(shape, clip, metadata_seen, positions);
                }
            }
            egui::epaint::Shape::Text(text) => {
                positions.push(shape.visual_bounding_rect());
                let label = text.galley.text();
                if label.starts_with("3840 × 2160") {
                    *metadata_seen = true;
                }
                assert!(
                    clip.expand(2.0).contains_rect(shape.visual_bounding_rect()),
                    "Clipped text {label:?}: {:?} outside {clip:?}",
                    shape.visual_bounding_rect()
                );
            }
            egui::epaint::Shape::LineSegment { .. } => {
                positions.push(shape.visual_bounding_rect());
                assert!(
                    clip.expand(2.0).contains_rect(shape.visual_bounding_rect()),
                    "Clipped toolbar stroke: {:?} outside {clip:?}",
                    shape.visual_bounding_rect()
                );
            }
            _ => {}
        }
    }

    #[test]
    fn loaded_image_controls_and_metadata_remain_visible_when_resizing() {
        let ctx = egui::Context::default();
        chrome::configure(&ctx);
        let directory = tempfile::tempdir().unwrap();
        let name = "a-long-photo-name-that-must-truncate-without-moving-the-metadata-2.jpg";
        let path = directory.path().join(name);
        for index in 1..=7 {
            std::fs::write(
                directory
                    .path()
                    .join(name.replace("-2.jpg", &format!("-{index}.jpg"))),
                [],
            )
            .unwrap();
        }
        let mut app = LumenApp {
            image: Some(DisplayedImage {
                texture: ctx.load_texture(
                    "layout-fixture",
                    egui::ColorImage::filled([2, 2], egui::Color32::WHITE),
                    egui::TextureOptions::LINEAR,
                ),
                path: path.clone(),
                dimensions: [3840, 2160],
                reduced: true,
                playback: None,
            }),
            folder: Some(Folder::scan(&path).unwrap()),
            ..Default::default()
        };
        for size in [
            egui::vec2(480.0, 360.0),
            egui::vec2(620.0, 560.0),
            egui::vec2(759.0, 560.0),
            egui::vec2(760.0, 560.0),
            egui::vec2(800.0, 560.0),
            egui::vec2(1100.0, 740.0),
            egui::vec2(480.0, 360.0),
        ] {
            let mut idle_positions = None;
            for loading in [false, true, false] {
                let (_sender, receiver) = mpsc::channel();
                app.pending = loading.then_some(receiver);
                for pass in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ui| app.show(ui),
                    );
                    output.textures_delta.clear();
                    let mut metadata_seen = false;
                    let mut positions = Vec::new();
                    for clipped in &output.shapes {
                        assert_visible_shapes(
                            &clipped.shape,
                            clipped.clip_rect,
                            &mut metadata_seen,
                            &mut positions,
                        );
                    }
                    assert!(metadata_seen, "Image metadata was not painted at {size:?}");
                    if pass == 2 {
                        if let Some(idle) = &idle_positions {
                            assert_eq!(
                                &positions, idle,
                                "Controls moved when loading={loading} at {size:?}"
                            );
                        } else {
                            idle_positions = Some(positions);
                        }
                    }
                }
            }
            app.view.rotate(true);
        }
    }

    fn save_image(path: &std::path::Path) {
        image::RgbaImage::from_pixel(3, 2, image::Rgba([100, 150, 200, 255]))
            .save(path)
            .unwrap();
    }

    #[test]
    fn reuses_photo_texture_across_sizes_and_animation_transitions() {
        let ctx = egui::Context::default();
        let mut app = LumenApp::default();
        let mut texture_id = None;
        let mut previous_size = None;
        for (index, (size, animated)) in [
            ([4, 3], false),
            ([4, 3], false),
            ([2, 5], true),
            ([2, 5], true),
            ([2, 5], false),
            ([4, 3], false),
        ]
        .into_iter()
        .enumerate()
        {
            let pixels = Arc::new(egui::ColorImage::filled(
                size,
                egui::Color32::from_rgba_unmultiplied(20 * index as u8, 90, 170, 128),
            ));
            let path = PathBuf::from(format!(
                "photo{index}.{}",
                if animated { "gif" } else { "png" }
            ));
            let animation = animated.then(|| Animation {
                frames: vec![
                    image_loader::AnimationFrame {
                        pixels: pixels.clone(),
                        delay: Duration::from_millis(100),
                    },
                    image_loader::AnimationFrame {
                        pixels: Arc::new(egui::ColorImage::filled(size, egui::Color32::RED)),
                        delay: Duration::from_millis(100),
                    },
                ],
                plays: None,
            });
            let (sender, receiver) = mpsc::channel();
            sender
                .send(Ok(Some(OpenedImage {
                    image: LoadedImage {
                        path: path.clone(),
                        dimensions: [size[0] as u32, size[1] as u32],
                        pixels: pixels.clone(),
                        reduced: false,
                        animation,
                    },
                    folder: Folder::single(path.clone()),
                    notice: None,
                })))
                .unwrap();
            app.pending = Some(receiver);
            app.finish_loading(&ctx);
            let displayed = app.image.as_ref().unwrap();
            let id = displayed.texture.id();
            assert_eq!(*texture_id.get_or_insert(id), id);
            assert_eq!(displayed.path, path);
            assert_eq!(displayed.texture.size(), size);
            assert_eq!(displayed.playback.is_some(), animated);
            let mut delta = ctx.tex_manager().write().take_delta();
            assert!(
                delta.free.is_empty(),
                "Photo transitions must not delete textures"
            );
            let updates = delta.set.get(&id).unwrap();
            assert_eq!(updates.len(), 1);
            let update = &updates[0];
            assert_eq!(update.pos, (previous_size == Some(size)).then_some([0, 0]));
            let egui::ImageData::Color(uploaded) = &update.image;
            assert!(
                Arc::ptr_eq(uploaded, &pixels),
                "Upload the decoder's exact pixels without resizing or conversion"
            );
            previous_size = Some(size);
            delta.clear();
        }
    }

    #[test]
    fn successful_open_resets_rotation_but_failed_open_preserves_it() {
        let ctx = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo.png");
        save_image(&path);
        let mut app = LumenApp::default();
        let deliver = |app: &mut LumenApp, result: LoadResult| {
            let (sender, receiver) = mpsc::channel();
            sender.send(result).unwrap();
            app.pending = Some(receiver);
            app.finish_loading(&ctx);
        };
        deliver(&mut app, load_selection(path.clone(), 1024).map(Some));
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(3.0, 2.0));
        let texture = egui::TextureId::Managed(1);
        let original = app.view.mesh(texture, rect);
        app.view.rotate(true);
        let rotated = app.view.mesh(texture, rect);
        assert_ne!(rotated, original);
        deliver(&mut app, Err("Unreadable file".to_owned()));
        assert_eq!(app.view.mesh(texture, rect), rotated);
        deliver(&mut app, load_selection(path, 1024).map(Some));
        assert_eq!(app.view.mesh(texture, rect), original);
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
