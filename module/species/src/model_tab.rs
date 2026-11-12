//! The Model tab: the 3D model of a species and its pictures (textures).
//!
//! The model files are in the archive 8 (the file 0 lists them, see `variants`). The tab draws the model in the pose of its file, with
//! a point of view that the user turns with the mouse and zooms with the wheel, and lists the pictures that paint it.
//! The drawing is made on the processor (see `sango_core::render`): the window shows its result as one picture.

use sango_core::model::{first_bone_name, parse_model, parse_textures, Model, Texture};
use sango_core::motion::{Motion, MotionPack};
use sango_core::posing::posed;
use sango_core::render::{bounds_of, render, render_framed_at, Camera};
use sango_core::{lz, Bundle, Dump, Garc};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::HashMap;

/// The size of the picture of the model, in pixels.
const VIEW_SIZE: usize = 440;

/// The files of a model, from its first file: the model, then the pictures and the pictures of the shiny colours.
const TEXTURE_OFFSET: usize = 2;
const SHINY_TEXTURE_OFFSET: usize = 3;

/// The motion files, from the first file of a model: the battle motions and the Pokémon-Amie motions.
const BATTLE_MOTION_OFFSET: usize = 5;
const AMIE_MOTION_OFFSET: usize = 6;

/// The names of the battle motions, in the order of the file.
const BATTLE_MOTIONS: [&str; 18] = [
    "Idle A",
    "Idle B",
    "Idle C",
    "Entrance 1: jump out",
    "Entrance 1: falling loop",
    "Entrance 1: landing",
    "Entrance 2",
    "Mega Evolution pose",
    "Physical attack 1",
    "Physical attack 2",
    "Physical attack 3",
    "Physical attack 4",
    "Special attack 1",
    "Special attack 2",
    "Special attack 3",
    "Special attack 4",
    "Damage",
    "Faint",
];

/// The names of the Pokémon-Amie motions, in the order of the file.
const AMIE_MOTIONS: [&str; 26] = [
    "Idle",
    "React",
    "Turn A",
    "Turn B",
    "Doze off: start",
    "Doze off: loop",
    "Doze off: wake up",
    "Sleep: loop",
    "Sleep: wake up",
    "Dislike",
    "Confused",
    "Happy A",
    "Happy B",
    "Happy C",
    "Habit A",
    "Habit B",
    "Habit C",
    "Habit D",
    "Lonely",
    "Want to play A",
    "Want to play B",
    "Angry",
    "Bite: start",
    "Bite: loop",
    "Bite: end",
    "Touch",
];

/// The name of a motion: the files hold more motions than the list of the game (they control the loops and the blinks).
fn motion_name(battle: bool, index: usize) -> String {
    let names: &[&str] = if battle { &BATTLE_MOTIONS } else { &AMIE_MOTIONS };
    let file = if battle { "Battle" } else { "Pokémon-Amie" };
    match names.get(index) {
        Some(name) => format!("{file} · {name}"),
        None => format!("{file} · control motion {index}"),
    }
}

/// A model of a species: the normal one, the female one or a form.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// The place of the model among the models of the species: 0 is the normal model.
    pub slot: u8,
    /// The first of the 8 files of the model in the archive 8.
    pub file: usize,
    /// The name that the tab shows.
    pub label: String,
}

/// The list of the models of the game is the file 0 of the archive 8: for each species (by its number in the Pokédex),
/// 4 bytes: the place of its first model (u16, the models take 8 files each, after the 3 first files), the number of
/// models (u8), and flags (u8: 2 the species has a female model, 4 it has forms).
fn model_list(archive: &Garc, species: usize) -> Option<(usize, usize, u8)> {
    let bytes = archive.first_file(0)?;
    let at = species.checked_sub(1)? * 4;
    let entry = bytes.get(at..at + 4)?;
    let (first, count, flags) = (u16::from_le_bytes([entry[0], entry[1]]) as usize, entry[2] as usize, entry[3]);
    (count > 0).then_some((first, count, flags))
}

/// The models of a species, from the list of the game. The names come from the file of each model: the number after the
/// name of the species is the form (51 and 52 are the Mega Evolutions and the Primal Reversions).
pub fn variants(archive: &Garc, species: usize) -> Vec<Variant> {
    let Some((first, count, flags)) = model_list(archive, species) else { return Vec::new() };
    (0..count)
        .map(|slot| {
            let mut file = (first + slot) * 8 + 3;
            let name = parts(archive, file).ok().and_then(|p| first_bone_name(p.first()?));
            // A form that shares the model of the species (the sizes of Pumpkaboo and Gourgeist) has no model of its own.
            if name.is_none() {
                file = first * 8 + 3;
            }
            let form: Option<u32> = name.and_then(|name| name.split('_').nth(1)?.get(..2)?.parse().ok());
            let label = match (slot, form) {
                (0, _) => "Normal".to_string(),
                (1, _) if flags & 2 != 0 => "Female".to_string(),
                (_, Some(51)) => "Mega Evolution or Primal Reversion".to_string(),
                (_, Some(52)) => "Mega Evolution Y".to_string(),
                (_, Some(number)) if number > 0 => format!("Form {number}"),
                _ => format!("Form {slot}"),
            };
            Variant { slot: slot as u8, file, label }
        })
        .collect()
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
    /// The battle motions and the Pokémon-Amie motions, when the files have them.
    battle: Option<MotionPack>,
    amie: Option<MotionPack>,
}

/// The motion that plays: which file, which one, and the motion itself.
struct Playing {
    battle: bool,
    index: usize,
    motion: Motion,
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
    /// The motion that the model plays, and its frame (the game plays 30 each second).
    playing: Option<Playing>,
    running: bool,
    frame: f32,
    last_tick: Option<std::time::Instant>,
    /// The sphere that the picture shows, from all the poses of the motion: the view does not move while the motion plays.
    focus: Option<([f32; 3], f32)>,
    /// True when the last picture took too long: the pictures are then drawn without the smoothing of the edges.
    slow: bool,
    /// The models of the species that the tab shows, and the species.
    variants: Vec<Variant>,
    variants_of: Option<usize>,
}

impl Default for ModelTab {
    fn default() -> Self {
        ModelTab { archive: None, form: 0, shiny: false, camera: Camera::default(), loaded: None, error: None, picture: None, dirty: true, thumbnails: HashMap::new(), status: None, playing: None, running: false, frame: 0.0, last_tick: None, focus: None, slow: false, variants: Vec::new(), variants_of: None }
    }
}

/// The parts of a file of the archive: it is compressed, and it is a bundle.
fn parts(archive: &Garc, file: usize) -> Result<Vec<Vec<u8>>, String> {
    let bytes = archive.first_file(file).ok_or_else(|| format!("The file {file} is missing."))?;
    let bytes = if lz::is_compressed(bytes) { lz::decompress(bytes)? } else { bytes.to_vec() };
    let bundle = Bundle::parse(bytes)?;
    Ok((0..bundle.resource_count()).filter_map(|i| bundle.resource(i).map(|part| part.to_vec())).collect())
}

/// Reads the motions of a file of the archive, when it has some.
fn read_motions(archive: &Garc, file: usize) -> Option<MotionPack> {
    let parts = parts(archive, file).ok()?;
    MotionPack::parse(parts.first()?).ok()
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
        self.variants_of = None;
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
        let Some(archive) = &self.archive else { return };
        let Some(file) = variants(archive, species).iter().find(|v| v.slot == form).map(|v| v.file) else { return };
        match read_model(archive, file, self.shiny) {
            Ok((model, textures)) => {
                let by_name = textures.iter().map(|texture| (texture.name.clone(), texture.clone())).collect();
                let (battle, amie) = (read_motions(archive, file + BATTLE_MOTION_OFFSET), read_motions(archive, file + AMIE_MOTION_OFFSET));
                // The model starts in its idle motion (the first of the battle file, else the first of Pokémon-Amie),
                // and the motion goes on without end.
                self.slow = false;
                self.playing = [(true, &battle), (false, &amie)].into_iter().find_map(|(is_battle, pack)| {
                    let pack = pack.as_ref()?;
                    let index = (0..pack.len()).find(|&i| pack.has(i))?;
                    pack.motion(index).ok().map(|motion| Playing { battle: is_battle, index, motion })
                });
                self.running = self.playing.is_some();
                self.frame = 0.0;
                self.last_tick = None;
                self.loaded = Some(Loaded { species, form, shiny: self.shiny, model, textures, by_name, battle, amie });
                self.refocus();
            }
            Err(message) => self.error = Some(message),
        }
    }

    /// Draws the tab. `form` is the form of the species that the page shows: the model of that form is shown first.
    pub fn ui(&mut self, ui: &mut egui::Ui, species: usize, form: usize) {
        let Some(archive) = &self.archive else { return };
        if self.variants_of != Some(species) {
            self.variants = variants(archive, species);
            self.variants_of = Some(species);
        }
        let available = self.variants.clone();
        if available.is_empty() {
            ui.label(widget::hint("This species has no model in the archive."));
            return;
        }
        if !available.iter().any(|v| v.slot == self.form) {
            self.form = available.iter().find(|v| v.slot as usize == form).or(available.first()).map_or(0, |v| v.slot);
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
        // The motion goes on with the time.
        let now = std::time::Instant::now();
        if let (true, Some(playing)) = (self.running, &self.playing) {
            let seconds = self.last_tick.map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.25));
            self.frame += seconds * 30.0;
            let length = playing.motion.frames.max(1) as f32;
            if self.frame >= length {
                self.frame %= length;
            }
            self.dirty = true;
            ui.ctx().request_repaint();
        }
        self.last_tick = Some(now);
        if self.dirty || self.picture.is_none() {
            if let Some(loaded) = &self.loaded {
                let pack = if self.playing.as_ref().is_some_and(|p| p.battle) { &loaded.battle } else { &loaded.amie };
                let pixels = match (&self.playing, pack) {
                    (Some(playing), Some(pack)) => {
                        let moved = posed(&loaded.model, pack, &playing.motion.pose(pack.joints.len(), self.frame));
                        let start = std::time::Instant::now();
                        let frame = self.focus.unwrap_or_else(|| sango_core::render::bounds(&moved));
                        let pixels = render_framed_at(&moved, &loaded.by_name, &self.camera, frame, VIEW_SIZE, VIEW_SIZE, if self.slow { 1 } else { 2 });
                        // A picture that takes more than 90 ms (the gas of Gastly and Koffing) is drawn without smoothing.
                        let took = start.elapsed();
                        self.slow = self.slow || took.as_millis() > 90;
                        pixels
                    }
                    _ => render(&loaded.model, &loaded.by_name, &self.camera, VIEW_SIZE, VIEW_SIZE),
                };
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
    fn side(&mut self, ui: &mut egui::Ui, species: usize, available: &[Variant]) {
        ui.set_width(380.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Model"));
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Form"));
                let selected = available.iter().find(|v| v.slot == self.form).map_or(String::new(), |v| v.label.clone());
                egui::ComboBox::from_id_salt("model_form").selected_text(selected).width(260.0).show_ui(ui, |ui| {
                    for variant in available {
                        if ui.selectable_label(self.form == variant.slot, &variant.label).clicked() {
                            self.form = variant.slot;
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
            if ui.checkbox(&mut self.camera.outline, "Outline").changed() {
                self.dirty = true;
            }
            ui.horizontal(|ui| {
                if widget::button(ui, "Export…") {
                    self.export_model();
                }
                if widget::button(ui, "Import pictures…") {
                    self.import_pictures();
                }
            });
            if let Some(loaded) = &self.loaded {
                let triangles: usize = loaded.model.meshes.iter().map(|mesh| mesh.triangles.len()).sum();
                ui.label(widget::hint(&format!("{} bones · {} meshes · {triangles} triangles · {} pictures", loaded.model.bones.len(), loaded.model.meshes.len(), loaded.textures.len())));
                if let Some(file) = available.iter().find(|v| v.slot == self.form).map(|v| v.file) {
                    ui.label(widget::hint(&format!("Files {file} to {} of the archive 8", file + 7)));
                }
            }
            let _ = species;
        });
        ui.add_space(8.0);
        self.motions(ui);
        ui.add_space(8.0);
        self.pictures(ui);
    }

    /// Writes the model as a .glb file and its pictures as PNG files, in a folder that the user chooses.
    fn export_model(&mut self) {
        let Some(loaded) = &self.loaded else { return };
        let Some(folder) = rfd::FileDialog::new().set_title("Folder for the exported model").pick_folder() else { return };
        let base = format!("pm{:04}_{}", loaded.species, loaded.form);
        let result = (|| -> Result<usize, String> {
            let mut clips = Vec::new();
            for (battle, pack) in [(true, &loaded.battle), (false, &loaded.amie)] {
                let Some(pack) = pack else { continue };
                for index in (0..pack.len()).filter(|&i| pack.has(i)) {
                    let Ok(motion) = pack.motion(index) else { continue };
                    let frames = (0..motion.frames.max(1)).map(|f| sango_core::posing::bone_values(&loaded.model, pack, &motion.pose(pack.joints.len(), f as f32))).collect();
                    clips.push(crate::model_export::Clip { name: motion_name(battle, index), frames });
                }
            }
            let file = crate::model_export::glb(&loaded.model, &loaded.by_name, &clips)?;
            std::fs::write(folder.join(format!("{base}.glb")), file).map_err(|e| e.to_string())?;
            let pictures = folder.join("textures");
            std::fs::create_dir_all(&pictures).map_err(|e| format!("{}: {e}", pictures.display()))?;
            for texture in &loaded.textures {
                std::fs::write(pictures.join(format!("{}.png", texture.name)), crate::model_export::png(texture)?).map_err(|e| e.to_string())?;
            }
            Ok(loaded.textures.len())
        })();
        self.status = Some(match result {
            Ok(count) => (true, format!("Exported {base}.glb and {count} pictures to {}.", folder.display())),
            Err(message) => (false, message),
        });
    }

    /// Reads PNG files that the user chooses (a folder with a PNG file named like each picture to change, or its
    /// `textures` folder) and shows them on the model. Nothing is written to the game files or the SD card.
    fn import_pictures(&mut self) {
        let Some(folder) = rfd::FileDialog::new().set_title("Folder of the changed pictures").pick_folder() else { return };
        let Some(loaded) = &mut self.loaded else { return };
        let mut changed = Vec::new();
        for texture in loaded.textures.iter_mut() {
            let candidates = [folder.join(format!("{}.png", texture.name)), folder.join("textures").join(format!("{}.png", texture.name))];
            let Some(path) = candidates.iter().find(|p| p.is_file()) else { continue };
            match std::fs::read(path).map_err(|e| format!("{}: {e}", path.display())).and_then(|bytes| crate::model_export::texture_from_png(&texture.name, &bytes)) {
                Ok(picture) => {
                    *texture = picture;
                    changed.push(texture.name.clone());
                }
                Err(message) => {
                    self.status = Some((false, message));
                    return;
                }
            }
        }
        for texture in &loaded.textures {
            loaded.by_name.insert(texture.name.clone(), texture.clone());
        }
        self.thumbnails.clear();
        self.dirty = true;
        self.status = Some(if changed.is_empty() {
            (false, format!("No PNG file named like a picture of the model in {}.", folder.display()))
        } else {
            (true, format!("{} picture(s) replaced in the view (the game files are not changed): {}.", changed.len(), changed.join(", ")))
        });
    }

    /// The motion list and the controls to play it.
    fn motions(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        let mut entries: Vec<(bool, usize)> = Vec::new();
        for (battle, pack) in [(true, &loaded.battle), (false, &loaded.amie)] {
            if let Some(pack) = pack {
                entries.extend((0..pack.len()).filter(|&i| pack.has(i)).map(|i| (battle, i)));
            }
        }
        let mut chosen: Option<Option<(bool, usize)>> = None;
        widget::card(ui, |ui| {
            ui.label(widget::title("Motion"));
            if entries.is_empty() {
                ui.label(widget::hint("This model has no motion in the archive."));
                return;
            }
            let selected = self.playing.as_ref().map_or("Rest pose".to_string(), |p| motion_name(p.battle, p.index));
            egui::ComboBox::from_id_salt("model_motion").selected_text(selected).width(340.0).height(360.0).show_ui(ui, |ui| {
                if ui.selectable_label(self.playing.is_none(), "Rest pose").clicked() {
                    chosen = Some(None);
                }
                for &(battle, index) in &entries {
                    let is_selected = self.playing.as_ref().is_some_and(|p| p.battle == battle && p.index == index);
                    if ui.selectable_label(is_selected, motion_name(battle, index)).clicked() {
                        chosen = Some(Some((battle, index)));
                    }
                }
            });
            if let Some(playing) = &self.playing {
                let last = playing.motion.frames.saturating_sub(1) as f32;
                ui.horizontal(|ui| {
                    if widget::button(ui, if self.running { "Pause" } else { "Play" }) {
                        self.running = !self.running;
                        self.last_tick = None;
                    }
                    let mut frame = self.frame.min(last).floor();
                    let response = ui.add(egui::Slider::new(&mut frame, 0.0..=last.max(1.0)).integer().text("frame"));
                    // The slider rounds the frame while the motion plays: only the user stops the motion.
                    if response.changed() && (response.dragged() || response.has_focus() || response.is_pointer_button_down_on()) {
                        self.frame = frame;
                        self.running = false;
                        self.dirty = true;
                    }
                });
                ui.label(widget::hint(&format!("{} frames, {:.1} seconds", playing.motion.frames, playing.motion.frames as f32 / 30.0)));
            }
        });
        if let Some(choice) = chosen {
            self.playing = choice.and_then(|(battle, index)| {
                let pack = if battle { loaded.battle.as_ref() } else { loaded.amie.as_ref() }?;
                pack.motion(index).ok().map(|motion| Playing { battle, index, motion })
            });
            self.running = self.playing.is_some();
            self.frame = 0.0;
            self.last_tick = None;
            self.dirty = true;
            self.refocus();
        }
    }

    /// Chooses the sphere that the picture shows: the one that holds the model in 16 poses of the motion that plays.
    fn refocus(&mut self) {
        self.focus = None;
        let (Some(loaded), Some(playing)) = (&self.loaded, &self.playing) else { return };
        let Some(pack) = (if playing.battle { &loaded.battle } else { &loaded.amie }) else { return };
        let poses: Vec<Model> = (0..16).map(|k| posed(&loaded.model, pack, &playing.motion.pose(pack.joints.len(), playing.motion.frames as f32 * k as f32 / 16.0))).collect();
        self.focus = Some(bounds_of(&poses));
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

    /// With the dump of the game: the models of a species come from the list of the game, and the first bone of each model
    /// is named after the model (the number of the model is not always the number in the Pokédex: the models of the
    /// species after the 649th have other numbers).
    #[test]
    fn the_models_of_the_dump_are_the_ones_of_the_list() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let labels = |species: usize| variants(&archive, species).iter().map(|v| v.label.clone()).collect::<Vec<_>>();
        assert_eq!(variants(&archive, 1).iter().map(|v| (v.slot, v.file)).collect::<Vec<_>>(), [(0, 3)]);
        assert_eq!(labels(3), ["Normal", "Female", "Mega Evolution or Primal Reversion"]);
        assert_eq!(labels(6), ["Normal", "Mega Evolution or Primal Reversion", "Mega Evolution Y"]);
        assert!(variants(&archive, 9999).is_empty());
        // Yveltal, Hawlucha, Furfrou and Fennekin have their own models (the files are not in the order of the Pokédex).
        let file = |species: usize| variants(&archive, species)[0].file;
        assert_eq!((file(717), file(701), file(676), file(653)), (8003, 7827, 7539, 7091));
        let (mut read, mut without_pictures, mut failed) = (0, 0, Vec::new());
        for species in 1..=721usize {
            for variant in variants(&archive, species) {
                match read_model(&archive, variant.file, false) {
                    Ok((model, textures)) => {
                        assert!(!model.meshes.is_empty(), "the file {} has no mesh", variant.file);
                        without_pictures += textures.is_empty() as usize;
                        read += 1;
                    }
                    Err(message) => failed.push((species, variant.slot, message)),
                }
            }
        }
        eprintln!("models read: {read}, without pictures: {without_pictures}, failed: {} {:?}", failed.len(), failed.iter().take(5).collect::<Vec<_>>());
        assert!(failed.is_empty(), "{failed:?}");
        assert!(read >= 1000);

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
            for (name, species, form) in [("pikachu", 25usize, 0u8), ("mewtwo", 150, 0), ("charizard", 6, 0), ("mega_charizard_x", 6, 1), ("gyarados", 130, 0), ("lugia", 249, 0), ("bulbasaur", 1, 0), ("charmander", 4, 0), ("squirtle", 7, 0), ("eevee", 133, 0), ("jigglypuff", 39, 0), ("gengar", 94, 0), ("snorlax", 143, 0), ("lucario", 448, 0), ("sylveon", 700, 0), ("greninja", 658, 0), ("gardevoir", 282, 0), ("nidorina", 30, 0), ("machop", 66, 0), ("tangela", 114, 0), ("turtwig", 387, 0), ("glameow", 431, 0), ("charmander2", 4, 0), ("hawlucha", 701, 0), ("sandshrew", 27, 0), ("sandslash", 28, 0), ("solosis", 577, 0), ("cofagrigus", 563, 0), ("munna", 517, 0), ("vivillon", 666, 0), ("jumpluff", 189, 0), ("ampharos", 181, 0), ("spiritomb", 442, 0), ("torkoal", 324, 0), ("moltres", 146, 0), ("muk", 89, 0), ("ponyta", 77, 0), ("snivy", 495, 0), ("malamar", 687, 0), ("inkay", 686, 0), ("spewpa", 665, 0), ("yveltal", 717, 0), ("furfrou", 676, 0), ("fennekin", 653, 0), ("ponyta", 77, 0), ("gastly", 92, 0), ("grimer", 88, 0), ("koffing", 109, 0), ("charmeleon", 5, 0), ("caterpie", 10, 0), ("weedle", 13, 0), ("kakuna", 14, 0), ("beedrill", 15, 0), ("raichu", 26, 0), ("wigglytuff", 40, 0), ("diglett", 50, 0), ("primeape", 57, 0), ("onix", 95, 0), ("pinsir", 127, 0), ("weedle2", 13, 0), ("pikachu2", 25, 0), ("eevee2", 133, 0), ("haunter", 93, 0), ("weezing", 110, 0)] {
                let Some(file) = variants(&archive, species).iter().find(|v| v.slot == form).map(|v| v.file) else {
                    eprintln!("{name}: no model for the form {form}");
                    continue;
                };
                let (model, textures) = read_model(&archive, file, false).unwrap();
                let by_name: HashMap<String, Texture> = textures.into_iter().map(|t| (t.name.clone(), t)).collect();
                std::fs::write(format!("{out}/{name}.glb"), crate::model_export::glb(&model, &by_name, &[]).unwrap()).unwrap();
                if let Some(pack) = read_motions(&archive, file + BATTLE_MOTION_OFFSET) {
                    if let Some(index) = (0..pack.len()).find(|&i| pack.has(i)) {
                        let motion = pack.motion(index).unwrap();
                        let frames = (0..motion.frames.max(1)).map(|f| sango_core::posing::bone_values(&model, &pack, &motion.pose(pack.joints.len(), f as f32))).collect();
                        let clip = crate::model_export::Clip { name: "idle".into(), frames };
                        std::fs::write(format!("{out}/{name}_anim.glb"), crate::model_export::glb(&model, &by_name, &[clip]).unwrap()).unwrap();
                    }
                }
                let image = render(&model, &by_name, &Camera::default(), 400, 400);
                image::save_buffer(format!("{out}/{name}.png"), &image, 400, 400, image::ColorType::Rgba8).unwrap();
                let front = render(&model, &by_name, &Camera { yaw: std::f32::consts::PI, pitch: 0.0, zoom: 1.0, outline: true }, 800, 800);
                image::save_buffer(format!("{out}/front_{name}.png"), &front, 800, 800, image::ColorType::Rgba8).unwrap();
                for texture in by_name.values() {
                    image::save_buffer(format!("{out}/tex_{name}_{}.png", texture.name), &texture.rgba, texture.width as u32, texture.height as u32, image::ColorType::Rgba8).unwrap();
                }
                eprintln!("{name}: {:?}", model.materials);
                // Some poses: the idle motion, an attack and the faint motion of the battle file, and the idle of Pokémon-Amie.
                for (label, battle, index, fraction) in [("idle", true, 0usize, 0.0f32), ("idle_mid", true, 0, 0.5), ("idle_q1", true, 0, 0.25), ("idle_q3", true, 0, 0.75), ("attack", true, 8, 0.5), ("faint", true, 17, 1.0), ("amie_idle", false, 0, 0.0), ("amie_happy", false, 11, 0.5)] {
                    let Some(pack) = read_motions(&archive, file + if battle { BATTLE_MOTION_OFFSET } else { AMIE_MOTION_OFFSET }) else { continue };
                    let Ok(motion) = pack.motion(index) else { continue };
                    let moved = posed(&model, &pack, &motion.pose(pack.joints.len(), motion.frames as f32 * fraction));
                    let image = render(&moved, &by_name, &Camera { yaw: std::f32::consts::PI + 0.5, pitch: 0.1, zoom: 1.0, outline: true }, 400, 400);
                    image::save_buffer(format!("{out}/pose_{name}_{label}.png"), &image, 400, 400, image::ColorType::Rgba8).unwrap();
                }
            }
        }
        // The shiny pictures are read too.
        tab.shiny = true;
        tab.load(150, tab.form);
        assert!(tab.loaded.as_ref().unwrap().shiny);
    }
}
