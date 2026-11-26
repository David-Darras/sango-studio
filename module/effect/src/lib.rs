//! The Effect page: the particle effects of the battles (the archive 31).
//!
//! An effect has layers. A layer is an emitter (how many particles, when, in which shape) with the set of particles that it makes (their
//! colour, their transparency, their size, their life). The page edits these numbers in place; a file that the user changes goes on the SD card
//! (`effect/<number>.bin`, compressed as in the archive) and the plugin reads it instead of the file of the game. See `effect_file.rs`.

mod effect_file;
mod preview;

use effect_file::{ColorCurve, Effect, FieldKind, NumberCurve};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};

/// The archive of the particle effects.
const ARCHIVE: usize = 31;
/// The folder of the SD card.
const FOLDER: &str = "effect";

/// The page.
pub struct EffectPage {
    /// The window of the button Reset.
    reset_open: bool,
    /// The file that the user chose while the open effect had changes that were not saved: the window asks first.
    pending_file: Option<usize>,
    garc: Option<Garc>,
    root: Option<std::path::PathBuf>,
    error: Option<String>,
    /// The name of each file (the menu counts from 1: the name at the place n is the one of the file n).
    names: Vec<String>,
    file: usize,
    effect: Option<Effect>,
    baseline: Vec<u8>,
    layer: usize,
    status: Option<(bool, String)>,
    /// The pictures of the open effect, as textures.
    textures: Vec<Option<egui::TextureHandle>>,
    retired: sango_ui::retired::Retired,
    /// The preview of the open effect.
    view: PreviewState,
    /// Undo and redo of the open effect.
    history: sango_ui::history::History<Option<Effect>>,
}

impl Default for EffectPage {
    fn default() -> Self {
        EffectPage { reset_open: false, garc: None, root: None, error: None, names: Vec::new(), file: 723, effect: None, baseline: Vec::new(), layer: 0, status: None, textures: Vec::new(), retired: Default::default(), view: PreviewState::default(), pending_file: None, history: Default::default() }
    }
}

/// The state of the preview: the frame, the point of view, and the picture.
struct PreviewState {
    frame: f32,
    playing: bool,
    repeat: bool,
    camera: preview::Camera,
    last_tick: Option<std::time::Instant>,
    texture: Option<egui::TextureHandle>,
    /// What the picture shows: the file, the frame, the point of view and the bytes of the effect (a change of one of them draws it again).
    shown: Option<(usize, f32, preview::Camera, Vec<u8>)>,
    pictures: Vec<preview::Look>,
    extent: f32,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState { frame: 0.0, playing: true, repeat: true, camera: preview::Camera::default(), last_tick: None, texture: None, shown: None, pictures: Vec::new(), extent: 10.0 }
    }
}

/// The side of the picture of the preview, in pixels.
const PREVIEW_SIDE: usize = 380;
/// The frames each second.
const FRAMES_PER_SECOND: f32 = 30.0;

fn path(file: usize) -> String {
    format!("{FOLDER}/{file}.bin")
}

/// A white box with an outline, like the number boxes of the other pages: the user sees that it can be edited.
fn field<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = color::WHITE;
        widgets.inactive.bg_fill = color::WHITE;
        widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, color::FIELD_BORDER);
        widgets.hovered.weak_bg_fill = color::WHITE;
        widgets.hovered.bg_fill = color::WHITE;
        add(ui)
    })
    .inner
}

/// A number box with a tip and the limits of the value.
fn number(ui: &mut egui::Ui, value: &mut f32, speed: f64, min: f64, max: f64) -> bool {
    field(ui, |ui| ui.add(egui::DragValue::new(value).speed(speed).range(min..=max).clamp_existing_to_range(false)).on_hover_text("Click to type a number, or drag to change it.").changed())
}

fn rgb(value: [f32; 3]) -> egui::Color32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgb(byte(value[0]), byte(value[1]), byte(value[2]))
}

/// The colour of a curve at a moment of the life (0 to 1).
fn color_at(curve: &ColorCurve, at: f32) -> [f32; 3] {
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t.clamp(0.0, 1.0));
    if curve.fixed {
        curve.middle
    } else if at < curve.in_time {
        mix(curve.start, curve.middle, if curve.in_time > 0.0 { at / curve.in_time } else { 1.0 })
    } else if at > curve.out_time {
        mix(curve.middle, curve.end, if curve.out_time < 1.0 { (at - curve.out_time) / (1.0 - curve.out_time) } else { 1.0 })
    } else {
        curve.middle
    }
}

/// The transparency of a curve at a moment of the life.
fn alpha_at(curve: &NumberCurve, at: f32) -> f32 {
    if curve.fixed {
        curve.peak
    } else if at < curve.in_time {
        curve.start + (curve.peak - curve.start) * if curve.in_time > 0.0 { at / curve.in_time } else { 1.0 }
    } else if at > curve.out_time {
        curve.peak + (curve.end - curve.peak) * if curve.out_time < 1.0 { (at - curve.out_time) / (1.0 - curve.out_time) } else { 1.0 }
    } else {
        curve.peak
    }
}

/// The picture of the colour and of the transparency of the particles during their life.
fn life_strip(ui: &mut egui::Ui, color_curve: Option<&ColorCurve>, alpha_curve: Option<&NumberCurve>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 26.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_gray(60));
    let steps = 64;
    for step in 0..steps {
        let t = step as f32 / (steps - 1) as f32;
        let base = rgb(color_curve.map_or([1.0; 3], |c| color_at(c, t)));
        let alpha = alpha_curve.map_or(1.0, |a| alpha_at(a, t)).clamp(0.0, 1.0);
        let left = rect.left() + rect.width() * step as f32 / steps as f32;
        let cell = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(left + rect.width() / steps as f32 + 0.5, rect.bottom()));
        painter.rect_filled(cell, 0.0, egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), (alpha * 255.0) as u8));
    }
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, color::BORDER));
}

impl EffectPage {
    fn open(&mut self, dump: &Dump) {
        self.error = None;
        match Garc::open(&dump.archive(ARCHIVE)) {
            Ok(garc) => {
                self.names = (1..garc.file_count()).map(|id| sango_choreography::EFFECT_NAMES.iter().find(|(number, _)| *number as usize == id).map_or_else(|| format!("Effect {id}"), |(_, name)| name.to_string())).collect();
                self.garc = Some(garc);
            }
            Err(message) => self.error = Some(message),
        }
        self.effect = None;
        self.history.clear();
    }

    fn load(&mut self, sd: Option<&SdCard>) {
        let Some(garc) = self.garc.as_ref() else { return };
        let bytes = sd.and_then(|sd| sd.read(&path(self.file))).map(|b| if sango_core::lz::is_compressed(&b) { sango_core::lz::decompress(&b).unwrap_or_default() } else { b }).or_else(|| garc.first_file_decompressed(self.file));
        match bytes.map(|b| (Effect::parse(&b), b)) {
            Some((Ok(effect), bytes)) => {
                self.effect = Some(effect);
                self.baseline = bytes;
                self.layer = 0;
                self.status = None;
                self.forget_textures();
            }
            Some((Err(message), _)) => {
                self.effect = None;
                self.status = Some((false, message));
            }
            None => {
                self.effect = None;
                self.status = Some((false, "This effect is not in the archive.".to_string()));
            }
        }
    }

    /// The textures of the pictures are made again at the next frame.
    fn forget_textures(&mut self) {
        for texture in self.textures.drain(..).flatten() {
            self.retired.keep(texture);
        }
    }

    fn dirty(&self) -> bool {
        self.effect.as_ref().is_some_and(|e| e.bytes() != self.baseline.as_slice())
    }

    fn save(&mut self, sd: &SdCard) {
        let Some(effect) = self.effect.as_ref() else { return };
        let bytes = effect.bytes().to_vec();
        match sd.write(&path(self.file), &sango_core::lz::compress_lz11(&bytes)) {
            Ok(_) => {
                self.baseline = bytes;
                self.status = Some((true, "Saved. Restart the game to see the effect.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn reset(&mut self, sd: &SdCard) {
        match sd.remove(&path(self.file)) {
            Ok(_) => {
                self.load(Some(sd));
                self.status = Some((true, "The effect of the game is back. Restart the game to see it.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The preview of the open effect: the particles at a frame, with the buttons of the time.
    fn preview_card(&mut self, ui: &mut egui::Ui) {
        let Some(effect) = &self.effect else { return };
        let (file, view) = (self.file, &mut self.view);
        let length = preview::length(effect);
        widget::card(ui, |ui| {
            ui.label(widget::title("Preview"));
            ui.label(widget::hint("The particles appear where and when the layers say, with their size, colour, transparency and picture. They do not move yet: the speeds and the forces are not read."));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if widget::button(ui, if view.playing { "Pause" } else { "Play" }) {
                    view.playing = !view.playing;
                    view.last_tick = None;
                }
                if widget::button(ui, "Restart") {
                    view.frame = 0.0;
                }
                ui.checkbox(&mut view.repeat, "Repeat");
                ui.add(egui::Slider::new(&mut view.frame, 0.0..=length).text("frame").integer());
            });
            // The time goes on.
            let now = std::time::Instant::now();
            if view.playing {
                let seconds = view.last_tick.map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.25));
                view.frame += seconds * FRAMES_PER_SECOND;
                if view.frame > length {
                    view.frame = if view.repeat { 0.0 } else { length };
                    view.playing = view.repeat;
                }
                ui.ctx().request_repaint();
            }
            view.last_tick = Some(now);
            let (rect, response) = ui.allocate_exact_size(egui::vec2(PREVIEW_SIDE as f32, PREVIEW_SIDE as f32), egui::Sense::click_and_drag());
            if response.dragged() {
                let delta = response.drag_delta();
                view.camera.yaw -= delta.x * 0.01;
                view.camera.pitch = (view.camera.pitch + delta.y * 0.01).clamp(-1.4, 1.4);
            }
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                ui.input_mut(|i| {
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                    i.raw_scroll_delta = egui::Vec2::ZERO;
                });
                if scroll != 0.0 {
                    view.camera.zoom = (view.camera.zoom * (1.0 + scroll * 0.002)).clamp(0.3, 6.0);
                }
            }
            // Draw again when the file, the frame, the point of view or the effect changed.
            let bytes = effect.bytes();
            let same = matches!(&view.shown, Some((f, frame, camera, saved)) if *f == file && *frame == view.frame && *camera == view.camera && saved.as_slice() == bytes);
            if !same || view.texture.is_none() {
                if !matches!(&view.shown, Some((f, _, _, saved)) if *f == file && saved.as_slice() == bytes) {
                    view.pictures = preview::looks(effect);
                    view.extent = preview::extent(effect);
                }
                let pixels = preview::render(effect, &view.pictures, view.frame, view.camera, view.extent, PREVIEW_SIDE);
                view.texture = Some(sango_ui::textures::load(ui.ctx(), "effect_preview", egui::ColorImage::from_rgba_unmultiplied([PREVIEW_SIDE, PREVIEW_SIDE], &pixels), egui::TextureOptions::LINEAR));
                view.shown = Some((file, view.frame, view.camera, bytes.to_vec()));
            }
            if let Some(texture) = &view.texture {
                let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(texture.id(), rect, full, egui::Color32::WHITE);
            }
            ui.label(widget::hint("Drag to turn the view, scroll to zoom."));
        });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        if let Some(file) = self.pending_file {
            let what = format!("the effect {}", self.file);
            match widget::unsaved_window(ui.ctx(), &what) {
                Some(widget::Unsaved::Save) => {
                    if let Some(sd) = session.sd.as_ref() {
                        self.save(sd);
                    }
                    if !self.dirty() {
                        self.file = file;
                        self.load(session.sd.as_ref());
                    }
                    self.pending_file = None;
                }
                Some(widget::Unsaved::Discard) => {
                    self.file = file;
                    self.load(session.sd.as_ref());
                    self.pending_file = None;
                }
                Some(widget::Unsaved::Stay) => self.pending_file = None,
                None => {}
            }
        }
        let dirty = self.dirty();
        let from_card = session.sd.as_ref().is_some_and(|sd| sd.read(&path(self.file)).is_some());
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut file = self.file;
                let names: Vec<&str> = self.names.iter().map(String::as_str).collect();
                sango_species_base::searchable_picker(ui, "effect_file", &mut file, &names, 320.0);
                if file != self.file && file >= 1 && file <= names.len() {
                    if dirty {
                        // The changes of the open file would be lost: the window asks first.
                        self.pending_file = Some(file);
                    } else {
                        self.file = file;
                        self.load(session.sd.as_ref());
                    }
                }
                let has_card = session.sd.is_some();
                let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(self.effect.is_some()), import: Some(true), reset: Some(has_card && from_card), save: Some(has_card && dirty), ..Default::default() });
                self.history.buttons(ui, self.effect.is_some());
                if clicks.export {
                    if let Some(effect) = self.effect.as_ref() {
                        self.status = export_bytes("Export the effect", &format!("effect_{}.bin", self.file), ("Effect", &["bin"]), effect.bytes()).or(self.status.take());
                    }
                }
                if clicks.import {
                    match import_bytes("Import an effect", ("Effect", &["bin"])) {
                        Some(Ok(bytes)) => match Effect::parse(&bytes) {
                            Ok(effect) => {
                                self.effect = Some(effect);
                                self.layer = 0;
                                self.forget_textures();
                                self.status = Some((true, "Imported. Save to keep it.".to_string()));
                            }
                            Err(message) => self.status = Some((false, format!("This file is not an effect: {message}"))),
                        },
                        Some(Err(message)) => self.status = Some((false, message)),
                        None => {}
                    }
                }
                self.reset_open |= clicks.reset;
                if let (true, Some(sd)) = (clicks.save, session.sd.as_ref()) {
                    self.save(sd);
                }
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if from_card {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if session.sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can look at the effects, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &format!("The effect {}", self.file), &mut []) {
            if let Some(sd) = session.sd.as_ref() {
                self.reset(sd);
            }
        }
    }

    /// The layers of the effect, as buttons, and the cards of the open layer.
    fn layers(&mut self, ui: &mut egui::Ui) {
        let Some(effect) = self.effect.as_mut() else { return };
        let count = effect.layer_count();
        if count == 0 {
            ui.label(widget::hint("This effect has no layer."));
            return;
        }
        self.layer = self.layer.min(count - 1);
        // The pictures come first: they are what the user sees of the effect.
        if pictures_card(ui, effect, &mut self.textures, &mut self.status) {
            for texture in self.textures.drain(..).flatten() {
                self.retired.keep(texture);
            }
        }
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Layers"));
            ui.label(widget::hint("An effect is made of layers. Each layer makes its own particles: choose one to change it."));
            ui.add_space(4.0);
            let labels: Vec<String> = (0..count)
                .map(|layer| {
                    let name = effect.emitter_name(layer);
                    if name.is_empty() { format!("Layer {}", layer + 1) } else { format!("{}  {}", layer + 1, name) }
                })
                .collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            widget::tabs(ui, "effect_layer", &labels, &mut self.layer);
        });
        let layer = self.layer;
        ui.add_space(8.0);
        emission_card(ui, effect, layer);
        ui.add_space(8.0);
        shape_card(ui, effect, layer);
        ui.add_space(8.0);
        particles_card(ui, effect, layer);
    }
}

/// The pictures of the effect: each one is shown and can be exported as a PNG file, or replaced by a PNG file of the same size.
/// Gives true when a picture changed.
fn pictures_card(ui: &mut egui::Ui, effect: &mut Effect, textures: &mut Vec<Option<egui::TextureHandle>>, status: &mut Option<(bool, String)>) -> bool {
    let mut changed = false;
    textures.resize(effect.pictures().len(), None);
    widget::card(ui, |ui| {
        ui.label(widget::title("Pictures of the particles"));
        ui.label(widget::hint("The pictures that the particles of the effect are drawn with. Export one as a PNG file, change it, and import it back: it must keep its size."));
        ui.add_space(4.0);
        if effect.pictures().is_empty() {
            ui.label(widget::hint("This effect has no picture."));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for place in 0..effect.pictures().len() {
                let picture = effect.pictures()[place].clone();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(if picture.name.is_empty() { format!("Picture {}", place + 1) } else { picture.name.clone() }).strong());
                    ui.label(widget::hint(&format!("{} x {}", picture.width, picture.height)));
                    if textures[place].is_none() {
                        if let Some(rgba) = effect.picture_rgba(place) {
                            textures[place] = Some(sango_ui::textures::load(ui.ctx(), format!("effect_picture_{place}"), egui::ColorImage::from_rgba_unmultiplied([picture.width, picture.height], &rgba), egui::TextureOptions::NEAREST));
                        }
                    }
                    let size = egui::vec2(160.0, 120.0);
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    // A dark chequer behind the picture shows the transparent parts.
                    ui.painter().rect_filled(rect, 6.0, egui::Color32::from_gray(70));
                    if let Some(texture) = &textures[place] {
                        let scale = (size.x / picture.width as f32).min(size.y / picture.height as f32);
                        let shown = egui::Rect::from_center_size(rect.center(), egui::vec2(picture.width as f32 * scale, picture.height as f32 * scale));
                        ui.painter().image(texture.id(), shown, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }
                    ui.horizontal(|ui| {
                        if widget::button(ui, "Export PNG") {
                            *status = export_picture(effect, place, &picture.name);
                        }
                        if widget::button(ui, "Import PNG") {
                            let result = import_picture(effect, place);
                            if matches!(result, Some((true, _))) {
                                changed = true;
                            }
                            if result.is_some() {
                                *status = result;
                            }
                        }
                    });
                });
            }
        });
    });
    changed
}

/// Writes a picture as a PNG file, in a place that the user chooses.
fn export_picture(effect: &Effect, place: usize, name: &str) -> Option<(bool, String)> {
    let picture = &effect.pictures()[place];
    let file_name = if name.is_empty() { format!("picture_{}.png", place + 1) } else { format!("{name}.png") };
    let path = rfd::FileDialog::new().set_title("Export the picture").set_file_name(file_name).add_filter("PNG picture", &["png"]).save_file()?;
    let result = effect.picture_rgba(place).ok_or_else(|| "The picture cannot be read.".to_string()).and_then(|rgba| image::save_buffer(&path, &rgba, picture.width as u32, picture.height as u32, image::ColorType::Rgba8).map_err(|e| format!("{}: {e}", path.display())));
    Some(match result {
        Ok(()) => (true, format!("Exported {}.", path.display())),
        Err(message) => (false, message),
    })
}

/// Replaces a picture by a PNG file that the user chooses. The file must have the size of the picture.
fn import_picture(effect: &mut Effect, place: usize) -> Option<(bool, String)> {
    let path = rfd::FileDialog::new().set_title("Import a picture").add_filter("PNG picture", &["png"]).pick_file()?;
    let picture = effect.pictures()[place].clone();
    let result = image::open(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|image| {
        let image = image.to_rgba8();
        if (image.width() as usize, image.height() as usize) != (picture.width, picture.height) {
            return Err(format!("The picture has {} x {} pixels: it must have {} x {}.", image.width(), image.height(), picture.width, picture.height));
        }
        effect.set_picture_rgba(place, image.as_raw())
    });
    Some(match result {
        Ok(()) => (true, format!("Imported {}. Save to keep it.", path.display())),
        Err(message) => (false, message),
    })
}

fn emission_card(ui: &mut egui::Ui, effect: &mut Effect, layer: usize) {
    let mut emission = effect.emission(layer);
    let before = emission.clone();
    widget::card(ui, |ui| {
        ui.label(widget::title("Emission"));
        ui.label(widget::hint("How many particles the layer makes, and when."));
        ui.add_space(4.0);
        egui::Grid::new(("effect_emission", layer)).num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            ui.label("Particles at each emission");
            number(ui, &mut emission.ratio, 0.05, 0.0, 10_000.0);
            ui.end_row();
            ui.label("Random part of the number");
            number(ui, &mut emission.ratio_random, 0.05, 0.0, 10_000.0);
            ui.end_row();
            ui.label("Frames between two emissions");
            widget::spin(ui, &mut emission.interval, 1.0, 10_000.0);
            ui.end_row();
            ui.label("Random part of the interval");
            number(ui, &mut emission.interval_random, 0.05, 0.0, 10_000.0);
            ui.end_row();
            ui.label("First frame");
            widget::spin(ui, &mut emission.start, 0.0, 100_000.0);
            ui.end_row();
            ui.label("Never stops");
            ui.checkbox(&mut emission.infinite, "The layer makes particles as long as the effect lives");
            ui.end_row();
            ui.add_enabled_ui(!emission.infinite, |ui| ui.label("Frames of work"));
            ui.add_enabled_ui(!emission.infinite, |ui| widget::spin(ui, &mut emission.span, 0.0, 100_000.0));
            ui.end_row();
        });
    });
    if emission != before {
        effect.set_emission(layer, &emission);
    }
}

fn shape_card(ui: &mut egui::Ui, effect: &mut Effect, layer: usize) {
    let Some(kind) = effect.form_kind(layer) else { return };
    let fields = effect.form_values(layer);
    let mut changes = Vec::new();
    widget::card(ui, |ui| {
        ui.label(widget::title("Shape"));
        ui.label(widget::hint(&format!("The area where the particles appear: a {}.", kind.name().to_lowercase())));
        ui.add_space(4.0);
        if fields.is_empty() {
            ui.label(widget::hint("A point has no number to change."));
            return;
        }
        egui::Grid::new(("effect_shape", layer)).num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            for (field, values) in &fields {
                let mut values = *values;
                ui.label(field.name).on_hover_text(field.help);
                let changed = match field.kind {
                    FieldKind::Number => number(ui, &mut values[0], 0.01, 0.0, 1.0),
                    FieldKind::Angle => {
                        let mut degrees = values[0].to_degrees();
                        let changed = ui.add(egui::DragValue::new(&mut degrees).speed(1.0).range(0.0..=360.0).clamp_existing_to_range(false).suffix("°")).changed();
                        values[0] = degrees.to_radians();
                        changed
                    }
                    FieldKind::Vec2 | FieldKind::Vec3 => {
                        let count = if field.kind == FieldKind::Vec2 { 2 } else { 3 };
                        let mut changed = false;
                        ui.horizontal(|ui| {
                            for (axis, part) in ["X", "Y", "Z"].iter().zip(values.iter_mut()).take(count) {
                                ui.label(widget::hint(axis));
                                changed |= ui.add(egui::DragValue::new(part).speed(0.05)).changed();
                            }
                        });
                        changed
                    }
                    FieldKind::Switch => {
                        let mut on = values[0] != 0.0;
                        let changed = ui.checkbox(&mut on, "").changed();
                        values[0] = f32::from(u8::from(on));
                        changed
                    }
                };
                if changed {
                    changes.push((*field, values));
                }
                ui.end_row();
            }
        });
    });
    for (field, values) in changes {
        effect.set_form_value(layer, &field, values);
    }
}

fn color_row(ui: &mut egui::Ui, label: &str, value: &mut [f32; 3]) {
    ui.label(label);
    ui.horizontal(|ui| {
        // The picker rounds the colour when it draws: the value changes only when the user changes it.
        let mut shown = *value;
        if egui::color_picker::color_edit_button_rgb(ui, &mut shown).changed() {
            *value = shown;
        }
        for part in value.iter_mut() {
            ui.add(egui::DragValue::new(part).speed(0.01).range(0.0..=8.0).clamp_existing_to_range(false)).on_hover_text("Red, green and blue from 0 to 1 (more than 1 is brighter).");
        }
    });
}

fn particles_card(ui: &mut egui::Ui, effect: &mut Effect, layer: usize) {
    let mut color_curve = effect.color(layer);
    let mut alpha_curve = effect.alpha(layer);
    let mut size = effect.size(layer);
    let mut life = effect.life(layer);
    let mut capacity = effect.capacity(layer);
    let (c0, a0, s0, l0, k0) = (color_curve.clone(), alpha_curve.clone(), size.clone(), life, capacity);
    widget::card(ui, |ui| {
        ui.label(widget::title("Particles"));
        ui.label(widget::hint("What the particles of the layer look like during their life."));
        ui.add_space(4.0);
        life_strip(ui, color_curve.as_ref(), alpha_curve.as_ref());
        ui.label(widget::hint("Colour and transparency from the birth (left) to the end (right) of a particle."));
        ui.add_space(6.0);
        egui::Grid::new(("effect_particles", layer)).num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
            if let Some(curve) = color_curve.as_mut() {
                if curve.fixed {
                    color_row(ui, "Colour", &mut curve.middle);
                    curve.start = curve.middle;
                    curve.end = curve.middle;
                    ui.end_row();
                } else {
                    color_row(ui, "Colour at the birth", &mut curve.start);
                    ui.end_row();
                    color_row(ui, "Colour in the middle", &mut curve.middle);
                    ui.end_row();
                    color_row(ui, "Colour at the end", &mut curve.end);
                    ui.end_row();
                    ui.label("Middle colour from");
                    ui.horizontal(|ui| {
                        number(ui, &mut curve.in_time, 0.01, 0.0, 1.0);
                        ui.label(widget::hint("to"));
                        number(ui, &mut curve.out_time, 0.01, 0.0, 1.0);
                        ui.label(widget::hint("of the life (0 to 1)."));
                    });
                    ui.end_row();
                }
            } else {
                ui.label("Colour");
                ui.label(widget::hint("The colour of this layer comes from its picture: no number to change."));
                ui.end_row();
            }
            if let Some(curve) = alpha_curve.as_mut() {
                if curve.fixed {
                    ui.label("Opacity");
                    number(ui, &mut curve.peak, 0.01, 0.0, 1.0);
                    curve.start = curve.peak;
                    curve.end = curve.peak;
                    ui.end_row();
                } else {
                    ui.label("Opacity at the birth");
                    number(ui, &mut curve.start, 0.01, 0.0, 1.0);
                    ui.end_row();
                    ui.label("Opacity at the peak");
                    number(ui, &mut curve.peak, 0.01, 0.0, 1.0);
                    ui.end_row();
                    ui.label("Opacity at the end");
                    number(ui, &mut curve.end, 0.01, 0.0, 1.0);
                    ui.end_row();
                    ui.label("Peak from");
                    ui.horizontal(|ui| {
                        number(ui, &mut curve.in_time, 0.01, 0.0, 1.0);
                        ui.label(widget::hint("to"));
                        number(ui, &mut curve.out_time, 0.01, 0.0, 1.0);
                    });
                    ui.end_row();
                }
            }
            if let Some(curve) = size.as_mut() {
                let fixed = curve.fixed;
                let labels: &[&str] = if fixed { &["Size"] } else { &["Size at the birth", "Size in the middle", "Size at the end"] };
                for (place, label) in labels.iter().enumerate() {
                    let value = match (fixed, place) {
                        (true, _) | (false, 1) => &mut curve.middle,
                        (false, 0) => &mut curve.start,
                        _ => &mut curve.end,
                    };
                    ui.label(*label);
                    ui.horizontal(|ui| {
                        for (axis, part) in ["X", "Y", "Z"].iter().zip(value.iter_mut()) {
                            ui.label(widget::hint(axis));
                            ui.add(egui::DragValue::new(part).speed(0.05));
                        }
                    });
                    ui.end_row();
                }
                if fixed {
                    curve.start = curve.middle;
                    curve.end = curve.middle;
                }
            }
            if let Some((shortest, longest)) = life.as_mut() {
                ui.label("Life of a particle");
                ui.horizontal(|ui| {
                    number(ui, shortest, 0.5, 0.0, 100_000.0);
                    ui.label(widget::hint("to"));
                    number(ui, longest, 0.5, 0.0, 100_000.0);
                    ui.label(widget::hint("frames"));
                });
                ui.end_row();
            }
            if let Some(count) = capacity.as_mut() {
                ui.label("Most particles at once");
                widget::spin(ui, count, 1.0, 100_000.0);
                ui.end_row();
            }
        });
    });
    if color_curve != c0 {
        if let Some(curve) = &color_curve {
            effect.set_color(layer, curve);
        }
    }
    if alpha_curve != a0 {
        if let Some(curve) = &alpha_curve {
            effect.set_alpha(layer, curve);
        }
    }
    if size != s0 {
        if let Some(curve) = &size {
            effect.set_size(layer, curve);
        }
    }
    if life != l0 {
        if let Some((shortest, longest)) = life {
            effect.set_life(layer, shortest, longest);
        }
    }
    if capacity != k0 {
        if let Some(count) = capacity {
            effect.set_capacity(layer, count);
        }
    }
}

impl Module for EffectPage {
    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn title(&self) -> &str {
        "Effect"
    }

    fn icon(&self) -> &str {
        "💫"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        self.retired.tick(ui.ctx());
        ui.add_space(8.0);
        ui.heading("Effect");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let root = dump.root().to_path_buf();
        if self.root.as_deref() != Some(root.as_path()) {
            self.root = Some(root);
            self.open(dump);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        if self.effect.is_none() && self.status.is_none() {
            self.load(session.sd.as_ref());
        }
        self.tool_bar(ui, session);
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("effect_page").auto_shrink([false, false]).show(ui, |ui| {
            self.preview_card(ui);
            ui.add_space(8.0);
            self.layers(ui);
        });
        // Ctrl+Z, Ctrl+Y and the buttons on the open effect. A drag is one step: nothing while the mouse button is down.
        if !ui.input(|i| i.pointer.any_down()) && self.history.track(ui, self.file as u64, &mut self.effect) {
            let count = self.effect.as_ref().map_or(0, |effect| effect.layer_count());
            self.layer = self.layer.min(count.saturating_sub(1));
            self.forget_textures();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: the page opens an effect, a change is saved on the SD card, read back, and reset.
    #[test]
    fn an_effect_is_edited_saved_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_effect_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = EffectPage::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        assert!(page.effect.as_ref().is_some_and(|e| e.layer_count() > 0), "the effect is open");
        let moved: Vec<usize> = page.effect.as_ref().unwrap().bytes().iter().zip(&page.baseline).enumerate().filter(|(_, (a, b))| a != b).map(|(at, _)| at).collect();
        assert!(moved.is_empty(), "the page changed the file by itself at {moved:x?}");
        let mut emission = page.effect.as_ref().unwrap().emission(0);
        emission.ratio += 2.0;
        page.effect.as_mut().unwrap().set_emission(0, &emission);
        assert!(page.dirty());
        // Ctrl+Z and Ctrl+Y on the change.
        for (input, undone) in [(Default::default(), false), (sango_ui::history::test_keys(egui::Key::Z), true), (sango_ui::history::test_keys(egui::Key::Y), false)] {
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
            assert_eq!(!page.dirty(), undone);
        }
        page.save(session.sd.as_ref().unwrap());
        assert!(!page.dirty());
        assert!(session.sd.as_ref().unwrap().read("effect/index.bin").is_some(), "the list of the files is written");
        page.effect = None;
        page.status = None;
        page.load(session.sd.as_ref());
        assert_eq!(page.effect.as_ref().unwrap().emission(0), emission);
        page.reset(session.sd.as_ref().unwrap());
        assert!(!page.dirty());
        assert_ne!(page.effect.as_ref().unwrap().emission(0), emission);
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/// Writes bytes in a file that the user chooses. Gives what happened, or `None` when the user closed the window.
fn export_bytes(title: &str, name: &str, filter: (&str, &[&str]), bytes: &[u8]) -> Option<(bool, String)> {
    let path = rfd::FileDialog::new().set_title(title).set_file_name(name).add_filter(filter.0, filter.1).save_file()?;
    Some(match std::fs::write(&path, bytes) {
        Ok(()) => (true, format!("Exported to {}.", path.display())),
        Err(e) => (false, format!("{}: {e}", path.display())),
    })
}

/// Reads a file that the user chooses (decompressed when it is compressed like the files of the game).
fn import_bytes(title: &str, filter: (&str, &[&str])) -> Option<Result<Vec<u8>, String>> {
    let path = rfd::FileDialog::new().set_title(title).add_filter(filter.0, filter.1).pick_file()?;
    Some(std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display())).map(|b| if sango_core::lz::is_compressed(&b) { sango_core::lz::decompress(&b).unwrap_or(b) } else { b }))
}
