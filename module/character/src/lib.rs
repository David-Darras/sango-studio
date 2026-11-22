//! The Character and Swap pages: the 3D models of the characters of the overworld (the player, the Gym Leaders, the other people of the
//! maps).
//!
//! - **Character**: shows a model, exports it as a GLB file with its skeleton, its pictures and one animation for each motion of the
//!   character (walk, run, wave...), and imports a GLB or glTF file in its place. The motions of the game stay: the new model must have the
//!   joints of the old one.
//! - **Swap** (a tab of the page): makes a character look like another one. The Gym Leader, the player or any person of the maps can get the model (and the
//!   motions) of another character.
//!
//! A change is a whole pack of the archive on the SD card (`overworld/model/<pack>.bin`, see `store.rs`). The plugin gives it to the game
//! in place of the pack: restart the game after a change.

mod names;
mod store;

use sango_core::model::Texture;
use sango_core::render::{render, Camera};
use sango_core::{Dump, SdCard};
use sango_ui::picker::{picker, Group};
use sango_ui::retired::Retired;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;
use store::{Loaded, Store};

/// The size of the picture of the model, in pixels.
const SIZE: usize = 360;

/// The tabs of the Character page.
const TABS: [&str; 2] = ["Model", "Swap"];

/// The lists of the models: the models that the user changed come first.
fn groups(labels: &[String], changed: &[usize]) -> Vec<Group> {
    let mut result = Vec::new();
    if !changed.is_empty() {
        result.push(Group { title: "Changed by you".to_string(), entries: changed.iter().filter_map(|&n| labels.get(n).map(|l| (n as u16, l.clone()))).collect() });
    }
    result.push(Group { title: "Characters of the game".to_string(), entries: labels.iter().enumerate().map(|(n, l)| (n as u16, l.clone())).collect() });
    result
}

/// The Character page.
pub struct Character {
    opened_for: Option<PathBuf>,
    store: Option<Store>,
    error: Option<String>,
    labels: Vec<String>,
    chosen: usize,
    /// The model that is shown, and the pack that it was read for.
    loaded: Option<(usize, Loaded)>,
    load_error: Option<String>,
    camera: Camera,
    picture: Option<egui::TextureHandle>,
    dirty: bool,
    retired: Retired,
    status: Option<(bool, String)>,
    /// The scale of an import.
    scale: f32,
    tab: usize,
    swap: Swap,
}

impl Default for Character {
    fn default() -> Self {
        Character {
            opened_for: None,
            store: None,
            error: None,
            labels: Vec::new(),
            chosen: 172,
            loaded: None,
            load_error: None,
            camera: Camera::default(),
            picture: None,
            dirty: false,
            retired: Retired::default(),
            status: None,
            scale: 1.0,
            tab: 0,
            swap: Swap::default(),
        }
    }
}

impl Character {
    fn open(&mut self, dump: &Dump) {
        self.opened_for = Some(dump.root().to_path_buf());
        self.loaded = None;
        match Store::open(dump) {
            Ok(store) => {
                self.labels = store.labels();
                self.store = Some(store);
                self.error = None;
                self.chosen = self.chosen.min(self.labels.len().saturating_sub(1));
            }
            Err(message) => {
                self.store = None;
                self.error = Some(message);
            }
        }
    }

    /// Reads the model that is chosen, when it is not read yet.
    fn load(&mut self, sd: Option<&SdCard>) {
        if self.loaded.as_ref().is_some_and(|(pack, _)| *pack == self.chosen) {
            return;
        }
        self.loaded = None;
        self.load_error = None;
        self.camera = Camera::default();
        self.dirty = true;
        let Some(store) = &self.store else { return };
        match store.pack(sd, self.chosen).ok_or_else(|| "This model is not in the archive.".to_string()).and_then(|bytes| Store::read(&bytes)) {
            Ok(loaded) => self.loaded = Some((self.chosen, loaded)),
            Err(message) => self.load_error = Some(message),
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        self.retired.tick(ui.ctx());
        let (rect, response) = ui.allocate_exact_size(egui::vec2(SIZE as f32, SIZE as f32), egui::Sense::click_and_drag());
        if response.dragged() {
            let delta = response.drag_delta();
            self.camera.yaw -= delta.x * 0.012;
            self.camera.pitch = (self.camera.pitch + delta.y * 0.012).clamp(-1.4, 1.4);
            self.dirty = true;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            ui.input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                i.raw_scroll_delta = egui::Vec2::ZERO;
            });
            if scroll != 0.0 {
                self.camera.zoom = (self.camera.zoom * (1.0 + scroll * 0.002)).clamp(0.3, 6.0);
                self.dirty = true;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if self.dirty {
            if let Some((_, loaded)) = &self.loaded {
                let pixels = render(&loaded.model, &loaded.textures as &HashMap<String, Texture>, &self.camera, SIZE, SIZE);
                let image = egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &pixels);
                if let Some(old) = self.picture.take() {
                    self.retired.keep(old);
                }
                self.picture = Some(sango_ui::textures::load(ui.ctx(), "character_view", image, egui::TextureOptions::LINEAR));
            }
            self.dirty = false;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
        painter.rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0_f32, color::BORDER));
        if let (Some(texture), Some(_)) = (&self.picture, &self.loaded) {
            painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        if let Some(message) = &self.load_error {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, message, egui::FontId::proportional(14.0), color::ROSE);
        }
    }

    fn export(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        let name = self.labels.get(self.chosen).cloned().unwrap_or_default().replace(' ', "_").to_lowercase();
        let Some(path) = rfd::FileDialog::new().set_title("Export the model").set_file_name(format!("{name}.glb")).add_filter("3D model", &["glb"]).save_file() else { return };
        let result = store.export(sd, self.chosen).and_then(|file| std::fs::write(&path, file).map_err(|e| format!("{}: {e}", path.display())));
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported {}. The file has the animations of the character: one for each motion.", path.display())),
            Err(message) => (false, message),
        });
    }

    fn import(&mut self, sd: &SdCard) {
        let Some(store) = &self.store else { return };
        let Some(file) = rfd::FileDialog::new().set_title("3D model (glTF or GLB)").add_filter("3D model", &["glb", "gltf"]).pick_file() else { return };
        self.status = Some(match store.import(sd, self.chosen, &file, f64::from(self.scale)) {
            Ok(()) => {
                self.loaded = None;
                (true, "The model is changed. Restart the game to see it.".to_string())
            }
            Err(message) => (false, message),
        });
    }
}

impl Module for Character {
    fn title(&self) -> &str {
        "Character"
    }

    fn icon(&self) -> &str {
        "🧍"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Character");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        if self.opened_for.as_deref() != Some(dump.root()) {
            self.open(dump);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        ui.horizontal(|ui| {
            for (place, name) in TABS.iter().enumerate() {
                if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                    self.tab = place;
                }
            }
        });
        ui.add_space(6.0);
        if self.tab == 1 {
            self.swap.body(ui, session);
            return;
        }
        let sd = session.sd.clone();
        self.load(sd.as_ref());
        let changed_packs = Store::changed_packs(sd.as_ref());
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Character"));
                let shown = format!("{}{}", self.labels.get(self.chosen).cloned().unwrap_or_default(), if changed_packs.contains(&self.chosen) { " (changed)" } else { "" });
                let mut value = self.chosen as u16;
                picker(ui, "character_model", &shown, 320.0, &mut value, None, || groups(&self.labels, &changed_packs));
                self.chosen = value as usize;
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt("character_scroll").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.horizontal_top(|ui| {
                    self.preview(ui);
                    ui.vertical(|ui| {
                        ui.set_max_width(520.0);
                        if let Some((_, loaded)) = &self.loaded {
                            ui.label(widget::title(self.labels.get(self.chosen).map_or("", |s| s.as_str())));
                            ui.label(widget::hint(&format!("{} joints, {} parts, {} pictures, {} motions.", loaded.model.bones.len(), loaded.model.meshes.len(), loaded.textures.len(), loaded.motion_count())));
                        }
                        ui.label(widget::hint("Drag to turn the model, scroll to zoom."));
                        ui.add_space(6.0);
                        ui.label(widget::hint(
                            "The export gives a GLB file with the skeleton, the meshes, the pictures and one animation for each motion of the character \
                             (the walk, the run, the jumps, the waves... named motion 12, motion 13...). The import changes the model: the motions of the game \
                             stay and move the joints by their names, so keep the skeleton of the exported file (move the points, change the pictures, but \
                             keep the names of the joints).",
                        ));
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if self.loaded.is_some() && widget::button(ui, "Export the model (GLB)…") {
                                self.export(sd.as_ref());
                            }
                            if let Some(sd) = sd.as_ref() {
                                if widget::button(ui, "Import a model (GLB)…") {
                                    self.import(sd);
                                }
                            }
                            if changed_packs.contains(&self.chosen) && widget::confirm_button(ui, "Back to the model of the game", "Click again to go back") {
                                if let (Some(store), Some(sd)) = (&self.store, sd.as_ref()) {
                                    self.status = Some(match store.reset(sd, self.chosen) {
                                        Ok(()) => {
                                            self.loaded = None;
                                            (true, "The model of the game is back. Restart the game.".to_string())
                                        }
                                        Err(message) => (false, message),
                                    });
                                }
                            }
                            ui.label(widget::hint("Scale"));
                            ui.add(egui::DragValue::new(&mut self.scale).speed(0.01).range(0.05..=20.0));
                        });
                        if sd.is_none() {
                            ui.label(widget::hint("Choose the SD card on the Home page to import a model."));
                        }
                        if let Some((good, message)) = &self.status {
                            ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
                        }
                    });
                });
            });
        });
    }
}

/// The Swap tab of the Character page.
struct Swap {
    opened_for: Option<PathBuf>,
    store: Option<Store>,
    error: Option<String>,
    labels: Vec<String>,
    /// The character that gets another model, and the character whose model it gets.
    target: usize,
    source: usize,
    status: Option<(bool, String)>,
}

impl Default for Swap {
    fn default() -> Self {
        Swap { opened_for: None, store: None, error: None, labels: Vec::new(), target: 187, source: 172, status: None }
    }
}

impl Swap {
    fn body(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        if self.opened_for.as_deref() != Some(dump.root()) {
            self.opened_for = Some(dump.root().to_path_buf());
            match Store::open(dump) {
                Ok(store) => {
                    self.labels = store.labels();
                    self.store = Some(store);
                    self.error = None;
                }
                Err(message) => {
                    self.store = None;
                    self.error = Some(message);
                }
            }
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        let Some(sd) = session.sd.clone() else {
            widget::missing_card(ui, "Choose the SD card on the Home page to swap the models.");
            return;
        };
        let changed = Store::changed_packs(Some(&sd));
        widget::card(ui, |ui| {
            ui.label(widget::hint(
                "Make a character look like another one. Everywhere that the game shows the first character (in the maps, in the cutscenes), it shows the \
                 model of the second one, with its motions. It works for May and Brendan, the Gym Leaders and every other person of the maps. Restart the \
                 game after a change.",
            ));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Show"));
                let mut target = self.target as u16;
                let shown = format!("{}{}", self.labels.get(self.target).cloned().unwrap_or_default(), if changed.contains(&self.target) { " (changed)" } else { "" });
                picker(ui, "swap_target", &shown, 260.0, &mut target, None, || groups(&self.labels, &changed));
                self.target = target as usize;
                ui.label(widget::hint("as"));
                let mut source = self.source as u16;
                let shown = self.labels.get(self.source).cloned().unwrap_or_default();
                picker(ui, "swap_source", &shown, 260.0, &mut source, None, || groups(&self.labels, &[]));
                self.source = source as usize;
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if widget::button(ui, "Swap") {
                    if let Some(store) = &self.store {
                        self.status = Some(match store.swap(&sd, self.target, self.source) {
                            Ok(()) => (true, format!("{} now looks like {}. Restart the game.", self.labels[self.target], self.labels[self.source])),
                            Err(message) => (false, message),
                        });
                    }
                }
                if widget::button(ui, "Swap both") {
                    if let Some(store) = &self.store {
                        // The two characters trade their models: the first model is read before it is replaced.
                        let result = store.pack(Some(&sd), self.target).ok_or_else(|| "A model is not in the archive.".to_string()).and_then(|first| {
                            store.swap(&sd, self.target, self.source)?;
                            sd.write(&store::path(self.source), &first)?;
                            sd.mark_changed(&store::path(self.source))
                        });
                        self.status = Some(match result {
                            Ok(()) => (true, format!("{} and {} traded their models. Restart the game.", self.labels[self.target], self.labels[self.source])),
                            Err(message) => (false, message),
                        });
                    }
                }
            });
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        ui.add_space(6.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Changed characters"));
            if changed.is_empty() {
                ui.label(widget::hint("No character is changed: the game has all its models."));
            }
            let mut give_back = None;
            for &pack in &changed {
                ui.horizontal(|ui| {
                    ui.label(self.labels.get(pack).map_or("?", |s| s.as_str()));
                    if widget::confirm_button(ui, "Back to the model of the game", "Click again to go back") {
                        give_back = Some(pack);
                    }
                });
            }
            if let (Some(pack), Some(store)) = (give_back, &self.store) {
                self.status = Some(match store.reset(&sd, pack) {
                    Ok(()) => (true, "The model of the game is back. Restart the game.".to_string()),
                    Err(message) => (false, message),
                });
            }
        });
    }
}
