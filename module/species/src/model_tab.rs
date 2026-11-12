//! The Model tab: the 3D model of a species and its pictures (textures).
//!
//! The model files are in the archive 8 (see `model_files`). The tab draws the model in the pose of its file, with
//! a point of view that the user turns with the mouse and zooms with the wheel, and lists the pictures that paint it.
//! The drawing is made on the processor (see `sango_core::render`): the window shows its result as one picture.

use crate::model_files::MODELS;
use sango_core::model::{parse_model, parse_textures, Model, Texture};
use sango_core::render::{render, Camera};
use sango_core::{lz, Bundle, Dump, Garc};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::HashMap;

/// The size of the picture of the model, in pixels.
const VIEW_SIZE: usize = 440;

/// The files of a model, from its first file: the model, then the pictures and the pictures of the shiny colours.
const TEXTURE_OFFSET: usize = 2;
const SHINY_TEXTURE_OFFSET: usize = 3;

/// The forms of a species that have a model: the form numbers of the model files, in order.
pub fn variants(species: usize) -> Vec<(u8, usize)> {
    MODELS.iter().filter(|(number, _, _)| *number as usize == species).map(|&(_, form, file)| (form, file as usize)).collect()
}

/// The name of a model form, as the tab shows it.
pub fn variant_name(form: u8) -> String {
    match form {
        0 => "Normal".to_string(),
        51 => "Mega Evolution or Primal Reversion".to_string(),
        52 => "Mega Evolution Y".to_string(),
        other => format!("Form {other}"),
    }
}

/// What is loaded: the model of a species and a form, and its pictures.
struct Loaded {
    species: usize,
    form: u8,
    shiny: bool,
    model: Model,
    /// The pictures in the order of the file.
    textures: Vec<Texture>,
    /// The pictures by their name, for the drawing.
    by_name: HashMap<String, Texture>,
}

/// The state of the Model tab.
pub struct ModelTab {
    archive: Option<Garc>,
    /// The form of the model that the user chose.
    form: u8,
    shiny: bool,
    camera: Camera,
    loaded: Option<Loaded>,
    /// What went wrong when the model was read.
    error: Option<String>,
    /// The drawing of the model, and whether it must be made again.
    picture: Option<egui::TextureHandle>,
    dirty: bool,
    /// The small pictures of the list, by name.
    thumbnails: HashMap<String, egui::TextureHandle>,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
}

impl Default for ModelTab {
    fn default() -> Self {
        ModelTab { archive: None, form: 0, shiny: false, camera: Camera::default(), loaded: None, error: None, picture: None, dirty: true, thumbnails: HashMap::new(), status: None }
    }
}

/// The parts of a file of the archive: it is compressed, and it is a bundle.
fn parts(archive: &Garc, file: usize) -> Result<Vec<Vec<u8>>, String> {
    let bytes = archive.first_file(file).ok_or_else(|| format!("The file {file} is missing."))?;
    let bytes = if lz::is_compressed(bytes) { lz::decompress(bytes)? } else { bytes.to_vec() };
    let bundle = Bundle::parse(bytes)?;
    Ok((0..bundle.resource_count()).filter_map(|i| bundle.resource(i).map(|part| part.to_vec())).collect())
}

/// Reads the model of a file and its pictures.
fn read_model(archive: &Garc, first: usize, shiny: bool) -> Result<(Model, Vec<Texture>), String> {
    let model = parse_model(parts(archive, first)?.first().ok_or("The file of the model has no part.")?)?;
    let offset = if shiny { SHINY_TEXTURE_OFFSET } else { TEXTURE_OFFSET };
    let textures = parts(archive, first + offset)?.first().map(|bytes| parse_textures(bytes)).transpose()?.unwrap_or_default();
    Ok((model, textures))
}

impl ModelTab {
    /// Opens the archive of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archive = Garc::open(&dump.archive(8)).ok();
        self.loaded = None;
        self.picture = None;
        self.thumbnails.clear();
    }

    /// Reads the model of the species if it is not the one that is loaded.
    fn load(&mut self, species: usize, form: u8) {
        let same = self.loaded.as_ref().is_some_and(|l| l.species == species && l.form == form && l.shiny == self.shiny);
        if same {
            return;
        }
        self.thumbnails.clear();
        self.dirty = true;
        self.loaded = None;
        self.error = None;
        let Some(&(_, file)) = variants(species).iter().find(|(number, _)| *number == form) else { return };
        let Some(archive) = &self.archive else { return };
        match read_model(archive, file, self.shiny) {
            Ok((model, textures)) => {
                let by_name = textures.iter().map(|texture| (texture.name.clone(), texture.clone())).collect();
                self.loaded = Some(Loaded { species, form, shiny: self.shiny, model, textures, by_name });
            }
            Err(message) => self.error = Some(message),
        }
    }

    /// Draws the tab. `form` is the form of the species that the page shows: the model of that form is shown first.
    pub fn ui(&mut self, ui: &mut egui::Ui, species: usize, form: usize) {
        let available = variants(species);
        if available.is_empty() {
            ui.label(widget::hint("This species has no model in the archive."));
            return;
        }
        if !available.iter().any(|(number, _)| *number == self.form) {
            self.form = available.iter().find(|(number, _)| *number as usize == form).or(available.first()).map_or(0, |(number, _)| *number);
        }
        self.load(species, self.form);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| self.view(ui));
            ui.add_space(12.0);
            ui.vertical(|ui| self.side(ui, species, &available));
        });
    }

    /// The picture of the model: the mouse turns it, the wheel zooms.
    fn view(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(VIEW_SIZE as f32, VIEW_SIZE as f32), egui::Sense::click_and_drag());
        if response.dragged() {
            let delta = response.drag_delta();
            self.camera.yaw -= delta.x * 0.012;
            self.camera.pitch = (self.camera.pitch + delta.y * 0.012).clamp(-1.4, 1.4);
            self.dirty = true;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom = (self.camera.zoom * (1.0 + scroll * 0.002)).clamp(0.3, 6.0);
                self.dirty = true;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if self.dirty || self.picture.is_none() {
            if let Some(loaded) = &self.loaded {
                let pixels = render(&loaded.model, &loaded.by_name, &self.camera, VIEW_SIZE, VIEW_SIZE);
                let image = egui::ColorImage::from_rgba_unmultiplied([VIEW_SIZE, VIEW_SIZE], &pixels);
                self.picture = Some(ui.ctx().load_texture("species_model_view", image, egui::TextureOptions::LINEAR));
            }
            self.dirty = false;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
        painter.rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0_f32, color::BORDER));
        if let (Some(texture), Some(_)) = (&self.picture, &self.loaded) {
            let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture.id(), rect, full, egui::Color32::WHITE);
        }
        if let Some(message) = &self.error {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, message, egui::FontId::proportional(14.0), color::ROSE);
        }
        ui.add_space(4.0);
        ui.label(widget::hint("Drag to turn the model. Scroll to zoom."));
    }

    /// The form, the colours, the numbers, and the list of pictures.
    fn side(&mut self, ui: &mut egui::Ui, species: usize, available: &[(u8, usize)]) {
        ui.set_width(380.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Model"));
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Form"));
                egui::ComboBox::from_id_salt("model_form").selected_text(variant_name(self.form)).width(260.0).show_ui(ui, |ui| {
                    for &(number, _) in available {
                        if ui.selectable_label(self.form == number, variant_name(number)).clicked() {
                            self.form = number;
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.selectable_label(!self.shiny, "Normal colours").clicked() {
                    self.shiny = false;
                }
                if ui.selectable_label(self.shiny, "Shiny colours").clicked() {
                    self.shiny = true;
                }
                if widget::button(ui, "Reset the view") {
                    self.camera = Camera::default();
                    self.dirty = true;
                }
            });
            if let Some(loaded) = &self.loaded {
                let triangles: usize = loaded.model.meshes.iter().map(|mesh| mesh.triangles.len()).sum();
                ui.label(widget::hint(&format!("{} bones · {} meshes · {triangles} triangles · {} pictures", loaded.model.bones.len(), loaded.model.meshes.len(), loaded.textures.len())));
                if let Some(file) = available.iter().find(|(number, _)| *number == self.form).map(|(_, file)| *file) {
                    ui.label(widget::hint(&format!("Files {file} to {} of the archive 8", file + 7)));
                }
            }
            let _ = species;
        });
        ui.add_space(8.0);
        self.pictures(ui);
    }

    /// The list of the pictures of the model, with a small view of each.
    fn pictures(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        widget::card(ui, |ui| {
            ui.label(widget::title("Pictures"));
            egui::ScrollArea::vertical().id_salt("model_pictures").max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                for texture in &loaded.textures {
                    ui.horizontal(|ui| {
                        let handle = self.thumbnails.entry(texture.name.clone()).or_insert_with(|| {
                            let image = egui::ColorImage::from_rgba_unmultiplied([texture.width, texture.height], &texture.rgba);
                            ui.ctx().load_texture(format!("model_picture_{}", texture.name), image, egui::TextureOptions::LINEAR)
                        });
                        let side = 56.0;
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                        let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter().image(handle.id(), rect.shrink(2.0), full, egui::Color32::WHITE);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&texture.name).strong());
                            ui.label(widget::hint(&format!("{} × {} pixels", texture.width, texture.height)));
                        });
                    });
                }
            });
        });
        if let Some((ok, message)) = &self.status {
            ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_forms_of_a_species_are_listed() {
        let venusaur: Vec<u8> = variants(3).iter().map(|(form, _)| *form).collect();
        assert_eq!(venusaur, [0, 1, 51]);
        assert_eq!(variants(1), [(0, 3)]);
        assert!(variants(9999).is_empty());
        assert_eq!(variant_name(0), "Normal");
        assert_eq!(variant_name(52), "Mega Evolution Y");
        assert_eq!(variant_name(2), "Form 2");
        // The files are 8 apart and in order.
        assert!(MODELS.windows(2).all(|pair| pair[1].2 == pair[0].2 + 8));
    }

    /// With the dump of the game: the file of every model has the name that the table says (the first bone is
    /// named like the model), every model is read, and the tab draws one.
    #[test]
    fn the_models_of_the_dump_are_the_ones_of_the_table() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let (mut read, mut without_pictures, mut failed, mut misnamed) = (0, 0, Vec::new(), Vec::new());
        for &(species, form, file) in &MODELS {
            match read_model(&archive, file as usize, false) {
                Ok((model, textures)) => {
                    // The first bone is named after the species (some models add a romaji suffix, and the female
                    // form of a species keeps the name of the normal form).
                    if !model.bones.first().is_some_and(|b| b.name.starts_with(&format!("pm{species:04}_"))) {
                        misnamed.push((species, form, model.bones.first().map(|b| b.name.clone())));
                    }
                    assert!(!model.meshes.is_empty(), "the file {file} has no mesh");
                    without_pictures += textures.is_empty() as usize;
                    read += 1;
                }
                Err(message) => failed.push((species, form, message)),
            }
        }
        eprintln!("models read: {read}, without pictures: {without_pictures}, failed: {} {:?}", failed.len(), failed.iter().take(5).collect::<Vec<_>>());
        // Only four files, of the species 735 and 736 (not Pokémon of the game: after the last species), are not models.
        assert!(failed.iter().all(|(species, _, _)| *species > 721), "{failed:?}");
        assert!(misnamed.is_empty(), "{misnamed:?}");
        assert!(read >= 1004);

        let mut tab = ModelTab::default();
        tab.open(&dump);
        let ctx = egui::Context::default();
        for (species, form) in [(1usize, 0usize), (3, 1), (6, 1), (150, 0)] {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0)));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| tab.ui(ui, species, form));
            });
            assert!(tab.loaded.is_some(), "species {species}: {:?}", tab.error);
            assert!(tab.picture.is_some());
        }
        // With the variable SANGO_RENDER_OUT, some models are written as PNG files, to look at them.
        if let Ok(out) = std::env::var("SANGO_RENDER_OUT") {
            for (name, species, form) in [("pikachu", 25usize, 0u8), ("mewtwo", 150, 0), ("charizard", 6, 0), ("mega_charizard_x", 6, 51), ("gyarados", 130, 0), ("lugia", 249, 0), ("bulbasaur", 1, 0), ("charmander", 4, 0), ("squirtle", 7, 0), ("eevee", 133, 0), ("jigglypuff", 39, 0), ("gengar", 94, 0), ("snorlax", 143, 0), ("lucario", 448, 0), ("sylveon", 700, 0), ("greninja", 658, 0), ("gardevoir", 282, 0), ("nidorina", 30, 0), ("machop", 66, 0), ("tangela", 114, 0), ("turtwig", 387, 0), ("glameow", 431, 0)] {
                let Some(&(_, file)) = variants(species).iter().find(|(number, _)| *number == form) else {
                    eprintln!("{name}: no model for the form {form}");
                    continue;
                };
                let (model, textures) = read_model(&archive, file, false).unwrap();
                let by_name: HashMap<String, Texture> = textures.into_iter().map(|t| (t.name.clone(), t)).collect();
                let image = render(&model, &by_name, &Camera::default(), 400, 400);
                image::save_buffer(format!("{out}/{name}.png"), &image, 400, 400, image::ColorType::Rgba8).unwrap();
                let front = render(&model, &by_name, &Camera { yaw: std::f32::consts::PI, pitch: 0.0, zoom: 1.0 }, 800, 800);
                image::save_buffer(format!("{out}/front_{name}.png"), &front, 800, 800, image::ColorType::Rgba8).unwrap();
                for texture in by_name.values() {
                    image::save_buffer(format!("{out}/tex_{name}_{}.png", texture.name), &texture.rgba, texture.width as u32, texture.height as u32, image::ColorType::Rgba8).unwrap();
                }
                eprintln!("{name}: {:?}", model.materials);
            }
        }
        // The shiny pictures are read too.
        tab.shiny = true;
        tab.load(150, tab.form);
        assert!(tab.loaded.as_ref().unwrap().shiny);
    }
}
