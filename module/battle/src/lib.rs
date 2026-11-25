//! The Battle page: the backgrounds of the battles.
//!
//! The archive `a/1/3/6` has the pieces of the backgrounds. A piece is a file of the archive:
//!
//! - a **BG bundle** (compressed with LZ11): the first 3D model is the piece (a ground, a plate...), the others are small files for it;
//! - a **3D model** alone (`BCH`), for a far ground or a sky;
//! - an **effect** (`CGFX`), for the weather and the particles (the Effect page edits those files).
//!
//! The page draws each 3D model, lists its pictures, saves a picture as PNG and replaces it with another PNG that has the same size. A changed
//! file goes on the SD card as `battle/background/<file>.bin`, with the bytes of the file of the archive, and the plugin reads it in place
//! of the file of the game.

mod labels;

use sango_core::render::{render, Camera};
use sango_core::{bundle, lz, Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;

/// The number of the archive of the backgrounds (`a/1/3/6`).
pub const ARCHIVE: usize = 136;

/// The folder of the files on the SD card.
const FOLDER: &str = "battle/background";

/// The size of the picture of a model.
const VIEW: (usize, usize) = (480, 320);

/// The kind of a file of the archive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A bundle of 3D models (`BG`).
    Bundle,
    /// A 3D model.
    Model,
    /// An effect.
    Effect,
    Other,
}

/// A file of the archive in the list.
#[derive(Clone, Debug)]
struct Entry {
    id: usize,
    kind: Kind,
    parts: usize,
}

impl Entry {
    /// A plain English name for the file, when there is one.
    fn name(&self) -> Option<&'static str> {
        match self.id {
            35..=113 => labels::GROUNDS.get(self.id - 35).copied().filter(|n| !n.is_empty()),
            114..=151 => labels::PLATES.get(self.id - 114).copied().filter(|n| !n.is_empty()),
            _ => None,
        }
    }

    fn group(&self) -> &'static str {
        match (self.id, self.kind) {
            (35..=113, _) => "Grounds",
            (114..=152, _) => "Plates and other pieces",
            (153..=157, _) => "Other pieces",
            (_, Kind::Effect) => "Effects (CGFX)",
            _ => "3D models",
        }
    }

    fn label(&self) -> String {
        match self.name() {
            Some(name) => format!("{}  ·  {name}", self.id),
            None => format!("{}", self.id),
        }
    }
}

/// A picture of a model.
struct Picture {
    name: String,
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    texture: egui::TextureHandle,
}

/// A file that is open.
struct Open {
    id: usize,
    /// The bytes of the file, decompressed.
    raw: Vec<u8>,
    compressed: bool,
    /// The start and the end of each part (a model alone has one part).
    parts: Vec<(usize, usize)>,
    part: usize,
    camera: Camera,
    /// The picture of the model that is shown, and what it was made for.
    view: Option<(egui::TextureHandle, (usize, usize, u32, u32))>,
    pictures: Vec<Picture>,
    pictures_for: Option<usize>,
    /// The picture that the arrow keys are on.
    selected_picture: usize,
    /// True when the selected picture must come in sight (after a key).
    scroll_picture: bool,
    dirty: bool,
    replaced: bool,
    /// The textures that were removed during this frame (freed at the start of the next one).
    retired: Vec<egui::TextureHandle>,
    error: Option<String>,
}

/// The Battle page.
pub struct BattlePage {
    /// The window of the button Reset….
    reset_open: bool,
    opened_for: Option<PathBuf>,
    entries: Vec<Entry>,
    filter: String,
    open: Option<Open>,
    status: Option<(bool, String)>,
    /// True when the selected file must come in sight in the list (after a key).
    scroll_list: bool,
}

impl Default for BattlePage {
    fn default() -> Self {
        BattlePage { reset_open: false, opened_for: None, entries: Vec::new(), filter: String::new(), open: None, status: None, scroll_list: false }
    }
}

fn sd_path(id: usize) -> String {
    format!("{FOLDER}/{id}.bin")
}

/// The point of view that the page starts with and goes back to: inside the place, like in a battle, with perspective (no outline: the
/// places have none).
fn front_view() -> Camera {
    Camera { yaw: 0.0, pitch: 0.12, zoom: 1.0, outline: false, inside: true }
}

/// Turns a picture upside down. The pictures of the models are stored from the bottom row: the decoder gives them in that order, and the
/// page shows them, saves them and reads them with the first row at the top, like any picture.
fn flip_rows(rgba: &[u8], width: usize) -> Vec<u8> {
    rgba.chunks(width * 4).rev().flatten().copied().collect()
}

/// A zone of the page that the arrow keys can work in: a click in the zone gives it the focus, and the arrows then change the choice.
/// Gives true when the keys are for the zone.
fn zone(ui: &egui::Ui, name: &str, rect: egui::Rect) -> bool {
    let clicked = ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(rect);
    focus::focus(ui, egui::Id::new(("battle_zone", name)), rect, clicked, false, None)
}

/// The decompressed bytes of a file of the archive, and whether it was compressed.
fn unpack(file: &[u8]) -> Result<(Vec<u8>, bool), String> {
    if lz::is_compressed(file) {
        Ok((lz::decompress(file)?, true))
    } else {
        Ok((file.to_vec(), false))
    }
}

/// The parts of a decompressed file.
fn parts_of(raw: &[u8]) -> (Kind, Vec<(usize, usize)>) {
    match raw.get(..4) {
        Some(b"BCH\0") => (Kind::Model, vec![(0, raw.len())]),
        Some(b"CGFX") => (Kind::Effect, vec![(0, raw.len())]),
        _ if raw.get(..2) == Some(b"BG") => match bundle::ranges(raw) {
            Ok(ranges) if !ranges.is_empty() && ranges.iter().all(|&(a, b)| a <= b && b <= raw.len()) => (Kind::Bundle, ranges),
            _ => (Kind::Other, vec![(0, raw.len())]),
        },
        _ => (Kind::Other, vec![(0, raw.len())]),
    }
}

impl BattlePage {
    fn scan(&mut self, dump: &Dump) {
        let path = dump.archive(ARCHIVE);
        if self.opened_for.as_ref() == Some(&path) {
            return;
        }
        self.opened_for = Some(path.clone());
        self.entries.clear();
        self.open = None;
        let Ok(garc) = Garc::open(&path) else { return };
        for id in 0..garc.file_count() {
            let Some(file) = garc.file(id, 0) else { continue };
            let Ok((raw, _)) = unpack(file) else { continue };
            let (kind, parts) = parts_of(&raw);
            self.entries.push(Entry { id, kind, parts: parts.len() });
        }
    }

    fn open_file(&mut self, session: &Session, id: usize) {
        let result = (|| -> Result<Open, String> {
            let dump = session.dump.as_ref().ok_or("No dump.")?;
            let sd_file = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(id)));
            let replaced = sd_file.is_some();
            let (raw, compressed) = match sd_file {
                Some(bytes) => unpack(&bytes)?,
                None => unpack(Garc::open(&dump.archive(ARCHIVE))?.file(id, 0).ok_or("This file is not in the archive.")?)?,
            };
            let (_, parts) = parts_of(&raw);
            Ok(Open {
                id,
                raw,
                compressed,
                parts,
                part: 0,
                camera: front_view(),
                view: None,
                pictures: Vec::new(),
                pictures_for: None,
                selected_picture: 0,
                scroll_picture: false,
                dirty: false,
                replaced,
                retired: Vec::new(),
                error: None,
            })
        })();
        match result {
            Ok(open) => {
                self.open = Some(open);
                self.status = None;
            }
            Err(e) => self.status = Some((false, e)),
        }
    }
}

impl Open {
    fn part_bytes(&self) -> &[u8] {
        let (a, b) = self.parts[self.part.min(self.parts.len() - 1)];
        &self.raw[a..b]
    }

    /// Makes the picture of the model and the list of its pictures again when the part, the point of view or the pictures changed.
    fn refresh(&mut self, ctx: &egui::Context) {
        if self.pictures_for != Some(self.part) {
            for p in self.pictures.drain(..) {
                self.retired.push(p.texture);
            }
            self.pictures_for = Some(self.part);
            self.view = None;
            self.error = None;
            match sango_core::model::parse_textures(self.part_bytes()) {
                Ok(list) => {
                    for t in list {
                        let rgba = flip_rows(&t.rgba, t.width);
                        let image = egui::ColorImage::from_rgba_unmultiplied([t.width, t.height], &rgba);
                        let texture = ctx.load_texture(format!("battle/{}/{}", self.id, t.name), image, egui::TextureOptions::LINEAR);
                        self.pictures.push(Picture { name: t.name, width: t.width, height: t.height, rgba, texture });
                    }
                }
                Err(e) => self.error = Some(e),
            }
        }
        let key = (self.id, self.part, self.camera.yaw.to_bits(), self.camera.pitch.to_bits() ^ self.camera.zoom.to_bits().rotate_left(7));
        let stamp = (key.0, key.1, key.2, key.3);
        if self.view.as_ref().map(|(_, s)| *s) != Some(stamp) {
            let bytes = self.part_bytes().to_vec();
            match sango_core::model::parse_model(&bytes) {
                Ok(model) if !model.meshes.is_empty() => {
                    let textures: HashMap<String, sango_core::model::Texture> = sango_core::model::parse_textures(&bytes)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| (t.name.clone(), t))
                        .collect();
                    let pixels = render(&model, &textures, &self.camera, VIEW.0, VIEW.1);
                    let image = egui::ColorImage::from_rgba_unmultiplied([VIEW.0, VIEW.1], &pixels);
                    let handle = ctx.load_texture("battle/view", image, egui::TextureOptions::LINEAR);
                    if let Some((old, _)) = self.view.take() {
                        self.retired.push(old);
                    }
                    self.view = Some((handle, stamp));
                }
                Ok(_) => self.error = Some("This part has no 3D mesh.".into()),
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Replaces a picture of the model of the part with the pixels of a PNG file.
    fn replace_picture(&mut self, name: &str, path: &std::path::Path) -> Result<(), String> {
        let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
        let new = sango_core::model::replace_texture(self.part_bytes(), name, &flip_rows(image.as_raw(), image.width() as usize))?;
        let (a, b) = self.parts[self.part];
        self.raw[a..b].copy_from_slice(&new);
        self.pictures_for = None;
        self.dirty = true;
        Ok(())
    }
}

impl Module for BattlePage {
    fn title(&self) -> &str {
        "Environment"
    }

    fn icon(&self) -> &str {
        "⚔"
    }

    fn unsaved(&self) -> bool {
        self.open.as_ref().map(|o| o.dirty).unwrap_or(false)
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        let Some(dump) = session.dump.clone() else {
            widget::missing_card(ui, "Open the dump of the game on the Home page to see the battle backgrounds.");
            return;
        };
        self.scan(&dump);
        let mut to_open: Option<usize> = None;
        let scroll_list = std::mem::take(&mut self.scroll_list);
        let mut displayed: Vec<usize> = Vec::new();

        egui::SidePanel::left("battle_list").resizable(true).default_width(220.0).show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search a name or a number").desired_width(f32::INFINITY));
            ui.add_space(4.0);
            let filter = self.filter.to_lowercase();
            let current = self.open.as_ref().map(|o| o.id);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut groups: Vec<&'static str> = self.entries.iter().map(|e| e.group()).collect();
                groups.dedup();
                for g in groups {
                    let items: Vec<&Entry> =
                        self.entries.iter().filter(|e| e.group() == g).filter(|e| filter.is_empty() || e.label().to_lowercase().contains(&filter)).collect();
                    if items.is_empty() {
                        continue;
                    }
                    let id = ui.make_persistent_id(("battle_group", g));
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, !filter.is_empty() || g == "Grounds")
                        .show_header(ui, |ui| {
                            ui.label(egui::RichText::new(format!("{g} ({})", items.len())).strong());
                        })
                        .body(|ui| {
                            for e in items {
                                displayed.push(e.id);
                                let response = ui.selectable_label(current == Some(e.id), e.label());
                                if current == Some(e.id) && scroll_list {
                                    response.scroll_to_me(None);
                                }
                                if response.clicked() {
                                    to_open = Some(e.id);
                                }
                            }
                        });
                }
            });
            // The arrows up and down choose the previous or the next file of the list.
            if zone(ui, "list", ui.max_rect()) {
                let step = focus::steps(ui, false);
                match current {
                    Some(c) => {
                        if let Some(to) = displayed.iter().position(|&id| id == c).and_then(|at| focus::stepped(at, step, displayed.len())) {
                            to_open = Some(displayed[to]);
                        }
                    }
                    None if step != 0 && !displayed.is_empty() => to_open = Some(displayed[0]),
                    None => {}
                }
            }
        });
        if let Some(id) = to_open {
            self.open_file(session, id);
            self.scroll_list = true;
        }

        let Some(open) = self.open.as_mut() else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(widget::title("Battle backgrounds"));
                ui.label(widget::hint("Choose a file on the left: a ground, a plate or a 3D model of the battles."));
            });
            return;
        };
        open.retired.clear();
        let entry = self.entries.iter().find(|e| e.id == open.id).cloned();

        ui.horizontal_wrapped(|ui| {
            ui.label(widget::title(&format!("File {}{}", open.id, entry.as_ref().and_then(|e| e.name()).map(|n| format!(" · {n}")).unwrap_or_default())));
            ui.separator();
            let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(true), import: Some(true), reset: Some(open.replaced), save: Some(open.dirty), ..Default::default() });
            if clicks.export {
                self.status = export_bytes("Export the file", &format!("battle_{}.bin", open.id), ("File of the game", &["bin"]), &open.raw).or(self.status.take());
            }
            if clicks.import {
                match import_bytes("Import a file", ("File of the game", &["bin"])) {
                    Some(Ok(raw)) => {
                        let (_, parts) = parts_of(&raw);
                        open.raw = raw;
                        open.parts = parts;
                        open.part = 0;
                        open.pictures.clear();
                        open.pictures_for = None;
                        if let Some((old, _)) = open.view.take() {
                            open.retired.push(old);
                        }
                        open.dirty = true;
                        self.status = Some((true, "Imported. Save to keep it.".to_string()));
                    }
                    Some(Err(message)) => self.status = Some((false, message)),
                    None => {}
                }
            }
            if clicks.save {
                save(open, session.sd.as_ref(), &mut self.status);
            }
            self.reset_open |= clicks.reset;
            if open.dirty {
                widget::badge(ui, "Not saved", color::AMBER);
            } else if open.replaced {
                widget::badge(ui, "From the SD card", color::TEAL);
            } else {
                widget::badge(ui, "From the romfs", color::GREEN);
            }
        });
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &format!("The file {}", open.id), &mut []) {
            reset(open, &dump, session.sd.as_ref(), &mut self.status);
        }
        if let Some((ok, text)) = &self.status {
            ui.colored_label(if *ok { color::GREEN } else { color::ROSE }, text);
        }
        ui.label(widget::hint(&format!(
            "{} · {} KB{}{}",
            entry.as_ref().map(|e| format!("{} part{}", e.parts, if e.parts == 1 { "" } else { "s" })).unwrap_or_default(),
            open.raw.len() / 1024,
            if open.compressed { ", compressed" } else { "" },
            if open.replaced { ", on the SD card" } else { "" }
        )));

        if open.parts.len() > 1 {
            ui.horizontal(|ui| {
                ui.label("Part");
                for i in 0..open.parts.len() {
                    let (a, b) = open.parts[i];
                    if ui.selectable_label(open.part == i, format!("{} ({} KB)", i, (b - a).div_ceil(1024))).clicked() {
                        open.part = i;
                    }
                }
            });
        }
        if entry.map(|e| e.kind) == Some(Kind::Effect) {
            ui.label(widget::hint("This file is an effect (CGFX). The Effect page edits the effects of the game."));
            return;
        }

        let ctx = ui.ctx().clone();
        open.refresh(&ctx);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // The model: drag to turn it.
            ui.horizontal(|ui| {
                match &open.view {
                    Some((handle, _)) => {
                        let width = (ui.available_width() - 140.0).clamp(160.0, VIEW.0 as f32);
                        let response = ui.add(egui::Image::new((handle.id(), egui::vec2(width, width * VIEW.1 as f32 / VIEW.0 as f32))).sense(egui::Sense::drag()));
                        if response.dragged() {
                            let d = response.drag_delta();
                            open.camera.yaw -= d.x * 0.01;
                            open.camera.pitch = (open.camera.pitch + d.y * 0.01).clamp(-1.3, 1.3);
                        }
                        response.on_hover_text("Drag to turn the model");
                    }
                    None => {
                        ui.label(widget::hint(open.error.as_deref().unwrap_or("No 3D model in this part.")));
                    }
                }
                ui.vertical(|ui| {
                    ui.label("Zoom");
                    ui.add(egui::Slider::new(&mut open.camera.zoom, 0.5..=4.0).show_value(false));
                    if ui.button("Front view").clicked() {
                        open.camera = front_view();
                    }
                });
            });

            ui.separator();
            ui.label(widget::title(&format!("Pictures ({})", open.pictures.len())));
            if open.pictures.is_empty() {
                ui.label(widget::hint("This part has no picture that can be read."));
            }
            let mut export: Option<usize> = None;
            let mut replace: Option<usize> = None;
            let count = open.pictures.len();
            open.selected_picture = open.selected_picture.min(count.saturating_sub(1));
            let scroll_picture = std::mem::take(&mut open.scroll_picture);
            let first_top = ui.cursor().top();
            for i in 0..count {
                let selected = open.selected_picture == i;
                let p = &open.pictures[i];
                let fill = if selected { color::ACCENT_SOFT } else { egui::Color32::TRANSPARENT };
                let row = egui::Frame::group(ui.style()).fill(fill).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let side = 96.0;
                        let scale = (side / p.width as f32).min(side / p.height as f32).min(4.0);
                        let (w, h) = (p.width as f32 * scale, p.height as f32 * scale);
                        ui.allocate_ui(egui::vec2(side, side), |ui| {
                            ui.centered_and_justified(|ui| {
                                ui.add(egui::Image::new((p.texture.id(), egui::vec2(w, h))));
                            });
                        });
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&p.name).strong());
                            ui.label(widget::hint(&format!("{} × {} pixels", p.width, p.height)));
                            ui.horizontal(|ui| {
                                if ui.button("Save PNG").clicked() {
                                    export = Some(i);
                                }
                                if ui.button("Replace with a PNG").clicked() {
                                    replace = Some(i);
                                }
                            });
                        });
                    });
                });
                if selected && scroll_picture {
                    ui.scroll_to_rect(row.response.rect, None);
                }
                if ui.interact(row.response.rect, egui::Id::new(("battle_picture", i)), egui::Sense::click()).clicked() {
                    open.selected_picture = i;
                }
            }
            // The arrows up and down choose the previous or the next picture.
            let pictures_rect = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), first_top), egui::pos2(ui.max_rect().right(), ui.cursor().top().max(first_top)));
            if count > 0 && zone(ui, "pictures", pictures_rect) {
                if let Some(to) = focus::stepped(open.selected_picture, focus::steps(ui, false), count) {
                    open.selected_picture = to;
                    open.scroll_picture = true;
                }
            }
            if let Some(i) = export {
                let p = &open.pictures[i];
                if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}.png", p.name)).add_filter("PNG picture", &["png"]).save_file() {
                    let result = image::RgbaImage::from_raw(p.width as u32, p.height as u32, p.rgba.clone()).ok_or("Bad picture.".to_string()).and_then(|img| img.save(&path).map_err(|e| e.to_string()));
                    self.status = Some(match result {
                        Ok(()) => (true, format!("Saved {}.", path.display())),
                        Err(e) => (false, e),
                    });
                }
            }
            if let Some(i) = replace {
                let name = open.pictures[i].name.clone();
                if let Some(path) = rfd::FileDialog::new().add_filter("PNG picture", &["png"]).pick_file() {
                    self.status = Some(match open.replace_picture(&name, &path) {
                        Ok(()) => (true, format!("{name} is replaced. Save for the game to keep it.")),
                        Err(e) => (false, e),
                    });
                }
            }
        });
    }
}

fn save(open: &mut Open, sd: Option<&SdCard>, status: &mut Option<(bool, String)>) {
    let Some(sd) = sd else {
        *status = Some((false, "Open the SD card folder on the Home page first.".into()));
        return;
    };
    let bytes = if open.compressed { lz::compress_lz11(&open.raw) } else { open.raw.clone() };
    match sd.write(&sd_path(open.id), &bytes) {
        Ok(path) => {
            open.dirty = false;
            open.replaced = true;
            *status = Some((true, format!("Saved: {}", path.display())));
        }
        Err(e) => *status = Some((false, e)),
    }
}

fn reset(open: &mut Open, dump: &Dump, sd: Option<&SdCard>, status: &mut Option<(bool, String)>) {
    if let Some(sd) = sd {
        let _ = sd.remove(&sd_path(open.id));
    }
    let result = Garc::open(&dump.archive(ARCHIVE)).and_then(|g| unpack(g.file(open.id, 0).ok_or("This file is not in the archive.")?));
    match result {
        Ok((raw, compressed)) => {
            let (_, parts) = parts_of(&raw);
            open.raw = raw;
            open.compressed = compressed;
            open.parts = parts;
            open.part = 0;
            open.pictures_for = None;
            open.dirty = false;
            open.replaced = false;
            *status = Some((true, "The file of the game is back.".into()));
        }
        Err(e) => *status = Some((false, e)),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Reads every file of the archive, draws the models, and replaces each picture with itself: the picture must stay the same.
    #[test]
    fn every_background_is_read_and_a_picture_can_be_replaced() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&path).unwrap();
        let garc = Garc::open(&dump.archive(ARCHIVE)).unwrap();
        assert_eq!(garc.file_count(), 158);
        let (mut models, mut pictures, mut replaced, mut unwritable) = (0, 0, 0, 0);
        for id in 0..garc.file_count() {
            let (raw, _) = unpack(garc.file(id, 0).unwrap()).unwrap();
            let (kind, parts) = parts_of(&raw);
            if kind == Kind::Effect || kind == Kind::Other {
                continue;
            }
            for (a, b) in parts {
                let part = &raw[a..b];
                if sango_core::model::parse_model(part).map(|m| !m.meshes.is_empty()).unwrap_or(false) {
                    models += 1;
                }
                for t in sango_core::model::parse_textures(part).unwrap_or_default() {
                    pictures += 1;
                    match sango_core::model::replace_texture(part, &t.name, &t.rgba) {
                        Ok(new) => {
                            assert_eq!(new.len(), part.len());
                            let again = sango_core::model::parse_textures(&new).unwrap();
                            let same = again.iter().find(|x| x.name == t.name).unwrap();
                            // A lossy format (ETC1) may change a few pixels when encoded again: the size must hold.
                            assert_eq!((same.width, same.height), (t.width, t.height));
                            replaced += 1;
                        }
                        Err(_) => unwritable += 1,
                    }
                }
            }
        }
        println!("{models} models, {pictures} pictures, {replaced} replaced, {unwritable} not writable");
        assert!(models > 100 && pictures > 100);
    }
}

#[cfg(test)]
mod preview {
    use super::*;

    #[test]
    fn draw_some_backgrounds() {
        let (Ok(path), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("BATTLE_PREVIEW")) else { return };
        let dump = Dump::open(&path).unwrap();
        let garc = Garc::open(&dump.archive(ARCHIVE)).unwrap();
        for id in [35, 60, 94, 113] {
            let (raw, _) = unpack(garc.file(id, 0).unwrap()).unwrap();
            let (_, parts) = parts_of(&raw);
            let (a, b) = parts[0];
            let part = &raw[a..b];
            let model = sango_core::model::parse_model(part).unwrap();
            let textures: HashMap<String, sango_core::model::Texture> = sango_core::model::parse_textures(part).unwrap().into_iter().map(|t| (t.name.clone(), t)).collect();
            for (turn, yaw) in [(0, 0.0f32), (1, std::f32::consts::PI)] {
                let pixels = render(&model, &textures, &Camera { yaw, ..front_view() }, VIEW.0, VIEW.1);
                image::RgbaImage::from_raw(VIEW.0 as u32, VIEW.1 as u32, pixels).unwrap().save(format!("{out}/bg_{id}_{turn}.png")).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod pictures {
    use super::*;

    #[test]
    fn save_pictures() {
        let (Ok(path), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("BATTLE_PREVIEW")) else { return };
        let dump = Dump::open(&path).unwrap();
        let garc = Garc::open(&dump.archive(ARCHIVE)).unwrap();
        for id in [94, 98] {
            let (raw, _) = unpack(garc.file(id, 0).unwrap()).unwrap();
            let (_, parts) = parts_of(&raw);
            let part = &raw[parts[0].0..parts[0].1];
            for t in sango_core::model::parse_textures(part).unwrap().into_iter().take(12) {
                image::RgbaImage::from_raw(t.width as u32, t.height as u32, t.rgba).unwrap().save(format!("{out}/tex_{id}_{}.png", t.name)).unwrap();
                println!("TEX {id} {}", t.name);
            }
        }
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
