//! The Character page: the 3D models of the characters of the overworld (the player, the Gym Leaders, the other people of the maps).
//!
//! The page shows a model, exports it as a GLB file with its skeleton, its pictures and one animation for each motion of the character
//! (walk, run, wave...), and imports a GLB or glTF file in its place. The motions of the game stay: the new model must have the joints of
//! the old one.
//!
//! A change is a whole pack of the archive on the SD card (`overworld/model/<pack>.bin`, see `store.rs`). The plugin gives it to the game
//! in place of the pack: restart the game after a change.

mod names;
pub mod store;

use sango_core::model::Texture;
use sango_core::render::{render, Camera};
use sango_core::{Dump, SdCard};
use sango_ui::history::{Step, Steps};
use sango_ui::picker::{picker, Group};
use sango_ui::retired::Retired;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;
use store::{Loaded, Store};

/// The size of the picture of the model, in pixels.
const SIZE: usize = 360;

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
    /// The window of the button Reset.
    reset_open: bool,
    /// Undo and redo of the imports and the resets: the number of the model, and its file on the SD card as it was.
    steps: Steps<(usize, Option<Vec<u8>>)>,
    /// The SD card of the steps.
    steps_card: Option<PathBuf>,
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
            reset_open: false,
            steps: Steps::default(),
            steps_card: None,
        }
    }
}

impl Character {
    fn open(&mut self, dump: &Dump) {
        self.opened_for = Some(dump.root().to_path_buf());
        self.loaded = None;
        self.steps.clear();
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

    fn reset(&mut self, sd: &SdCard) {
        let Some(store) = &self.store else { return };
        let before = Store::card_file(sd, self.chosen);
        self.status = Some(match store.reset(sd, self.chosen) {
            Ok(()) => {
                self.steps.done((self.chosen, before));
                self.loaded = None;
                (true, "The model of the game is back. Restart the game.".to_string())
            }
            Err(message) => (false, message),
        });
    }

    /// Undo or redo of an import or of a reset: the file of the SD card goes back, and its model is shown.
    fn step(&mut self, step: Step, sd: &SdCard) {
        let mut shown = None;
        let done = self.steps.apply(step, |(pack, file)| {
            let replaced = Store::card_file(sd, *pack);
            Store::put_card_file(sd, *pack, file.as_deref())?;
            shown = Some(*pack);
            Ok((*pack, replaced))
        });
        match done {
            Some(Ok(())) => {
                if let Some(pack) = shown {
                    self.chosen = pack;
                    self.loaded = None;
                    let way = if step == Step::Undo { "Undone" } else { "Done again" };
                    self.status = Some((true, format!("{way}. Restart the game.")));
                }
            }
            Some(Err(message)) => self.status = Some((false, message)),
            None => {}
        }
    }

    fn import(&mut self, sd: &SdCard) {
        let Some(store) = &self.store else { return };
        let Some(file) = rfd::FileDialog::new().set_title("3D model (glTF or GLB)").add_filter("3D model", &["glb", "gltf"]).pick_file() else { return };
        let before = Store::card_file(sd, self.chosen);
        self.status = Some(match store.import(sd, self.chosen, &file, f64::from(self.scale)) {
            Ok(()) => {
                self.steps.done((self.chosen, before));
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
        let sd = session.sd.clone();
        // The steps are for the files of one SD card.
        let card = sd.as_ref().map(|sd| sd.root().to_path_buf());
        if self.steps_card != card {
            self.steps_card = card;
            self.steps.clear();
        }
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
        let what = format!("The model {}", self.labels.get(self.chosen).map_or("", |label| label.as_str()));
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &what, &mut []) {
            if let Some(sd) = sd.as_ref() {
                self.reset(sd);
            }
        }
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
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            let changed = changed_packs.contains(&self.chosen);
                            let clicks = widget::action_bar(
                                ui,
                                widget::ActionBar {
                                    export: Some(self.loaded.is_some()),
                                    import: Some(sd.is_some()),
                                    reset: Some(changed && sd.is_some()),
                                    notes: widget::ActionNotes { export: "Writes the model and its motions in a GLB file.", import: "Replaces the model with the one of a glTF or GLB file.", reset: "Puts the model of the game back.", ..Default::default() },
                                    ..Default::default()
                                },
                            );
                            if clicks.export {
                                self.export(sd.as_ref());
                            }
                            if let (true, Some(sd)) = (clicks.import, sd.as_ref()) {
                                self.import(sd);
                            }
                            self.reset_open |= clicks.reset;
                            if let (Some(step), Some(sd)) = (self.steps.asked(ui, sd.is_some()), sd.as_ref()) {
                                self.step(step, sd);
                            }
                            ui.label(widget::hint("Scale"));
                            ui.add(egui::DragValue::new(&mut self.scale).speed(0.01).range(0.05..=20.0)).on_hover_text("The size of an imported model");
                        });
                        if let Some((good, message)) = &self.status {
                            ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
                        }
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: the reset of a model is a step, undo puts the file of the card back, redo removes it again.
    #[test]
    fn a_reset_is_undone_and_done_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_character_undo_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut page = Character::default();
        page.open(&sango_core::Dump::open(&root).unwrap());
        Store::put_card_file(&sd, page.chosen, Some(b"a model")).unwrap();
        page.reset(&sd);
        assert_eq!(Store::card_file(&sd, page.chosen), None);
        page.step(Step::Undo, &sd);
        assert_eq!(Store::card_file(&sd, page.chosen).as_deref(), Some(b"a model".as_slice()));
        page.step(Step::Redo, &sd);
        assert_eq!(Store::card_file(&sd, page.chosen), None);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
