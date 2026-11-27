//! The 3D model of a class of trainers, in the Class tab: the mouse turns it and the wheel zooms.
//!
//! The classes of the main characters (Brendan, May, Steven...) have no 2D picture: the battle shows a 3D model of the archive 133. The
//! archive has five files for each model: the model with its textures, then four files of motions. The class has the number of the model.

use sango_core::model::{parse_model, parse_textures, Model, Texture};
use sango_core::render::{render, Camera};
use sango_ui::retired::Retired;
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::HashMap;

/// The size of the picture, in pixels.
const SIZE: usize = 360;

/// The number of the model that means "no 3D model" in the data of a class.
pub const NO_MODEL: u8 = 255;

/// The files of the archive for each model.
pub const FILES_PER_MODEL: usize = 5;

/// What the user asked for in the card of the model.
pub enum Action {
    /// Replace the model by the one in a file, with a scale.
    Import(std::path::PathBuf, f64),
    /// Give back the model of the game.
    Reset,
}

pub struct ModelView {
    /// The model that is loaded.
    number: Option<u8>,
    loaded: Option<(Model, HashMap<String, Texture>)>,
    error: Option<String>,
    camera: Option<Camera>,
    picture: Option<egui::TextureHandle>,
    dirty: bool,
    retired: Retired,
    /// The file of the model, for the export.
    source: Option<Vec<u8>>,
    status: Option<(bool, String)>,
    /// The scale of an import.
    scale: f32,
}

impl Default for ModelView {
    fn default() -> Self {
        ModelView { number: None, loaded: None, error: None, camera: None, picture: None, dirty: false, retired: Retired::default(), source: None, status: None, scale: 1.0 }
    }
}

impl ModelView {
    /// Loads the model again at the next frame (the file changed).
    pub fn reload(&mut self) {
        self.number = None;
    }

    /// Tells the result of an action.
    pub fn say(&mut self, good: bool, message: String) {
        self.status = Some((good, message));
    }

    /// Shows the model of a class. `bytes` gives the file of the model (read when the number changes). `changed` is true when the SD card
    /// replaces the model. Gives what the user asked for.
    pub fn ui(&mut self, ui: &mut egui::Ui, number: u8, changed: bool, can_write: bool, bytes: impl FnOnce() -> Option<Vec<u8>>) -> Option<Action> {
        let mut action = None;
        self.retired.tick(ui.ctx());
        if self.number != Some(number) {
            self.number = Some(number);
            self.loaded = None;
            self.error = None;
            self.dirty = true;
            self.source = None;
            let camera = self.camera.get_or_insert_with(Camera::default);
            *camera = Camera::default();
            match bytes() {
                Some(data) => {
                    match (parse_model(&data), parse_textures(&data)) {
                        (Ok(model), Ok(textures)) => self.loaded = Some((model, textures.into_iter().map(|t| (t.name.clone(), t)).collect())),
                        (Err(message), _) | (_, Err(message)) => self.error = Some(message),
                    }
                    self.source = Some(data);
                }
                None => self.error = Some("The model is not in the archive.".to_string()),
            }
        }
        let (rect, response) = ui.allocate_exact_size(egui::vec2(SIZE as f32, SIZE as f32), egui::Sense::click_and_drag());
        let camera = self.camera.get_or_insert_with(Camera::default);
        if response.dragged() {
            let delta = response.drag_delta();
            camera.yaw += delta.x * 0.012;
            camera.pitch = (camera.pitch + delta.y * 0.012).clamp(-1.4, 1.4);
            self.dirty = true;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            // The wheel zooms the model only: the page must not scroll at the same time.
            ui.input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                i.raw_scroll_delta = egui::Vec2::ZERO;
            });
            if scroll != 0.0 {
                camera.zoom = (camera.zoom * (1.0 + scroll * 0.002)).clamp(0.3, 6.0);
                self.dirty = true;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if self.dirty {
            if let Some((model, textures)) = &self.loaded {
                let pixels = render(model, textures, camera, SIZE, SIZE);
                let image = egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &pixels);
                if let Some(old) = self.picture.take() {
                    self.retired.keep(old);
                }
                self.picture = Some(sango_ui::textures::load(ui.ctx(), "trainer_model_view", image, egui::TextureOptions::LINEAR));
            }
            self.dirty = false;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
        painter.rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0_f32, color::BORDER));
        if let (Some(texture), Some(_)) = (&self.picture, &self.loaded) {
            painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        if let Some(message) = &self.error {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, message, egui::FontId::proportional(14.0), color::ROSE);
        }
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.source.is_some(), |ui| {
                if widget::button(ui, "Export") {
                    self.export(number);
                }
            });
            ui.add_enabled_ui(can_write, |ui| {
                if widget::button(ui, "Import") {
                    if let Some(file) = rfd::FileDialog::new().set_title("3D model (glTF or GLB)").add_filter("3D model", &["glb", "gltf"]).pick_file() {
                        action = Some(Action::Import(file, f64::from(self.scale)));
                    }
                }
            });
            let reset = ui.add_enabled_ui(changed, |ui| widget::confirm_button(ui, "Reset", "Click again to reset")).inner;
            if reset {
                action = Some(Action::Reset);
            }
            ui.label(widget::hint("Scale"));
            ui.add(egui::DragValue::new(&mut self.scale).speed(0.01).range(0.05..=20.0)).on_hover_text("The size of an imported model");
        });
        if let Some((good, message)) = &self.status {
            ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
        }
        action
    }

    /// Writes the model as a GLB file, in a place that the user chooses.
    fn export(&mut self, number: u8) {
        let Some(source) = &self.source else { return };
        let Some(path) = rfd::FileDialog::new().set_title("Export the model").set_file_name(format!("trainer_model_{number}.glb")).add_filter("3D model", &["glb"]).save_file() else { return };
        let result = sango_species::glb_of_bch(source).and_then(|file| std::fs::write(&path, file).map_err(|e| format!("{}: {e}", path.display())));
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported {}.", path.display())),
            Err(message) => (false, message),
        });
    }
}

#[cfg(test)]
mod test {
    /// With the dump: the model of Brendan is exported as a GLB file.
    #[test]
    fn a_trainer_model_is_exported_as_glb() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let store = crate::store::Store::open(&sango_core::Dump::open(&root).unwrap()).unwrap();
        let bytes = store.model_file(None, 5).expect("the model 5");
        let glb = sango_species::glb_of_bch(&bytes).unwrap();
        assert_eq!(&glb[..4], b"glTF");
        assert!(glb.len() > 100_000);
    }

    /// With the dump: the GLB file of a model is imported, the file is read again, and it has the bones and the meshes of the original.
    #[test]
    fn a_trainer_model_is_imported_and_read_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_trainer_model_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = sango_core::SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let store = crate::store::Store::open(&sango_core::Dump::open(&root).unwrap()).unwrap();
        let original = store.model_file(None, 5).unwrap();
        let glb = folder.join("brendan.glb");
        std::fs::write(&glb, sango_species::glb_of_bch(&original).unwrap()).unwrap();
        store.import_model(&sd, 5, &glb, 1.0).unwrap();
        assert!(crate::store::Store::model_changed(Some(&sd), 5));
        assert!(sd.read("trainer/model/index.bin").is_some());
        let again = store.model_file(Some(&sd), 5).unwrap();
        let (a, b) = (sango_core::model::parse_model(&original).unwrap(), sango_core::model::parse_model(&again).unwrap());
        let missing: Vec<&str> = a.bones.iter().map(|x| x.name.as_str()).filter(|n| !b.bones.iter().any(|y| y.name == *n)).collect();
        assert_eq!(a.bones.len(), b.bones.len(), "the skeleton; missing {missing:?}");
        assert_eq!(a.meshes.len(), b.meshes.len(), "the meshes");
        let textures = sango_core::model::parse_textures(&again).unwrap();
        assert!(!textures.is_empty(), "the textures are in the file");
        let by_name: std::collections::HashMap<_, _> = textures.into_iter().map(|t| (t.name.clone(), t)).collect();
        let image = sango_core::render::render(&b, &by_name, &sango_core::render::Camera::default(), 400, 400);
        if let Ok(out) = std::env::var("SANGO_TEST_PNG") {
            image::save_buffer(out, &image, 400, 400, image::ColorType::Rgba8).unwrap();
        }
        store.reset_model(&sd, 5).unwrap();
        assert!(!crate::store::Store::model_changed(Some(&sd), 5));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
