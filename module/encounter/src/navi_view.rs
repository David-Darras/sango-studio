//! The DexNav part of the Encounter page: the pictures of the screen (export and import as PNG files), and the places where the Pokémon
//! appear on it.

use crate::data::{Encounter, METHODS};
use crate::dexnav::{map_types, Kind, Navi, TypePlace, TYPE_COUNT};
use crate::navi_parts::PARTS;
use sango_core::Dump;
use sango_species_base::SpeciesIcons;
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TABS: [&str; 2] = ["Screen of the map", "Pictures"];

const GROUND: &str = "Places of the Pokémon of the ground";
const WATER: &str = "Places of the Pokémon of the water";
const FISHING: &str = "Places of the Pokémon of the fishing rods";
/// The number of the part that holds the places of the Pokémon (the name of the pane is `4y`).
const PLACES_PARENT: u32 = 178;
/// The number of the part that is the picture of the map.
const MAP_PICTURE: u32 = 0;

/// The DexNav tab.
pub struct NaviView {
    /// The window of the button Reset….
    reset_open: bool,
    navi: Option<Navi>,
    error: Option<String>,
    opened_for: Option<PathBuf>,
    tab: usize,
    /// The resource of the picture that is shown.
    chosen: usize,
    textures: HashMap<usize, egui::TextureHandle>,
    /// The backgrounds of the preview, smoothed.
    smooth: HashMap<usize, egui::TextureHandle>,
    status: Option<(bool, String)>,
    /// The part of the screen that is selected, in `PARTS`.
    part: usize,
    /// The type of screen of each map.
    map_types: Vec<(usize, usize)>,
    /// The type of screen that is shown, when it is not the one of the map.
    view_type: Option<usize>,
    /// Where a drag of a place started: the pointer and the place.
    drag: Option<(egui::Pos2, (f32, f32))>,
}

impl Default for NaviView {
    fn default() -> Self {
        NaviView { reset_open: false, navi: None, error: None, opened_for: None, tab: 0, chosen: 0, textures: HashMap::new(), smooth: HashMap::new(), status: None, part: 20, map_types: Vec::new(), view_type: None, drag: None }
    }
}

/// What a picture is, for the menu.
fn describe(name: &str) -> String {
    let stem = name.trim_end_matches(".bclim");
    let number = |prefix: &str| stem.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok());
    if let Some(n) = number("screen_") {
        return if n == 0 { "Background of the screen".to_string() } else { format!("Background of the screen, type {n}") };
    }
    if let Some(n) = number("complete") {
        return format!("Mark of a complete list {}", n + 1);
    }
    match stem {
        "ball" => "Poké Ball".to_string(),
        "dummy" => "Empty picture".to_string(),
        "kirakira" => "Sparkle (cannot be changed)".to_string(),
        "pmn_zukan_main02" => "Window picture".to_string(),
        "search_check_button" => "Search button, to look".to_string(),
        "search_get_button" => "Search button, to catch".to_string(),
        "sp_mark" => "Special mark".to_string(),
        "zukan_unknown" => "Unknown Pokémon".to_string(),
        other => other.to_string(),
    }
}

fn write_png(path: &Path, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
    image::save_buffer(path, rgba, width as u32, height as u32, image::ColorType::Rgba8).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_png(path: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let picture = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
    Ok((picture.width() as usize, picture.height() as usize, picture.into_raw()))
}

impl NaviView {
    /// Writes the DexNav when it changed (the button Save does the same).
    pub fn save_changes(&mut self, session: &sango_core::Session) {
        if !self.unsaved() {
            return;
        }
        let Some(navi) = self.navi.as_mut() else { return };
        self.status = Some(match navi.save() {
            Ok(_) => {
                if let Some(sd) = session.sd.as_ref() {
                    let _ = sd.mark_changed("dexnav/0.bin");
                }
                (true, "Saved. Restart the game to see the change.".to_string())
            }
            Err(e) => (false, e),
        });
    }

    /// True when the DexNav has changes that are not saved.
    pub fn unsaved(&self) -> bool {
        self.navi.as_ref().is_some_and(|n| n.unsaved())
    }

    fn open(&mut self, dump: &Dump, sd_root: &Path) {
        self.textures.clear();
        self.smooth.clear();
        self.error = None;
        self.map_types = map_types(dump).unwrap_or_default();
        match Navi::open(dump, sd_root) {
            Ok(navi) => {
                self.chosen = navi.entries.iter().position(|e| e.kind == Kind::Picture).unwrap_or(0);
                self.navi = Some(navi);
            }
            Err(message) => {
                self.navi = None;
                self.error = Some(message);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, session: &Session, map: usize, table: Option<&Encounter>, icons: &mut SpeciesIcons) {
        let (Some(dump), Some(sd)) = (session.dump.as_ref(), session.sd.as_ref()) else {
            ui.label(widget::hint("Choose the dump and the SD card on the Home page to open the DexNav."));
            return;
        };
        let root = sd.root().to_path_buf();
        if self.opened_for.as_deref() != Some(root.as_path()) {
            self.opened_for = Some(root.clone());
            self.open(dump, &root);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        self.tool_bar(ui, session);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (place, name) in TABS.iter().enumerate() {
                if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                    self.tab = place;
                }
            }
        });
        ui.add_space(6.0);
        if self.tab == 0 {
            self.screen_tab(ui, map, table, icons);
        } else {
            self.pictures_tab(ui);
        }
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let Some(navi) = self.navi.as_mut() else { return };
        let (unsaved, from_card, changed) = (navi.unsaved(), navi.from_sd, navi.changed());
        let mut clicks = widget::ActionClicks::default();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                clicks = widget::action_bar(ui, widget::ActionBar { reset: Some(from_card || changed), save: Some(unsaved), ..Default::default() });
                if unsaved {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if from_card {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some((good, text)) = &self.status {
                ui.label(egui::RichText::new(text).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        let mut message = None;
        if clicks.save {
            if let Some(navi) = self.navi.as_mut() {
                message = Some(match navi.save() {
                    Ok(size) => {
                        if let Some(sd) = session.sd.as_ref() {
                            let _ = sd.mark_changed("dexnav/0.bin");
                        }
                        (true, if size == 0 { "The DexNav of the game is back.".to_string() } else { format!("Saved ({size} bytes). Restart the game to see the change.") })
                    }
                    Err(e) => (false, e),
                });
            }
        }
        self.reset_open |= clicks.reset;
        if widget::reset_window(ui.ctx(), &mut self.reset_open, "The DexNav", &mut []) {
            if let Some(navi) = self.navi.as_mut() {
                message = Some(match navi.reset() {
                    Ok(()) => {
                        if let Some(sd) = session.sd.as_ref() {
                            let _ = sd.mark_changed("dexnav/0.bin");
                        }
                        (true, "The DexNav of the game is back.".to_string())
                    }
                    Err(e) => (false, e),
                });
            }
        }
        if message.is_some() {
            self.textures.clear();
            self.smooth.clear();
            self.status = message;
        }
    }

    /// The texture of a picture, made when it is first shown.
    fn texture(&mut self, ctx: &egui::Context, at: usize) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.textures.get(&at) {
            return Some(texture.clone());
        }
        let navi = self.navi.as_ref()?;
        let picture = navi.picture(&navi.entries[at]).ok()?;
        let rgba = picture.to_rgba().ok()?;
        let image = egui::ColorImage::from_rgba_unmultiplied([picture.width(), picture.height()], &rgba);
        // A small picture keeps its square pixels; a big one is smoothed, so that a size between two whole sizes stays clean.
        let options = if picture.width() <= 64 { egui::TextureOptions::NEAREST } else { egui::TextureOptions::LINEAR };
        let texture = ctx.load_texture(format!("dexnav_{at}"), image, options);
        self.textures.insert(at, texture.clone());
        Some(texture)
    }

    /// The background of the preview: the picture at the size of its pane (as the game draws it), then enlarged with a smooth filter, so
    /// that the drawing looks like the screen of the game and not like a grid of big pixels.
    fn smooth_texture(&mut self, ctx: &egui::Context, at: usize, width: u32, height: u32) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.smooth.get(&at) {
            return Some(texture.clone());
        }
        let navi = self.navi.as_ref()?;
        let picture = navi.picture(&navi.entries[at]).ok()?;
        let rgba = image::RgbaImage::from_raw(picture.width() as u32, picture.height() as u32, picture.to_rgba().ok()?)?;
        let small = image::imageops::resize(&rgba, width, height, image::imageops::FilterType::Lanczos3);
        let big = image::imageops::resize(&small, width * 4, height * 4, image::imageops::FilterType::CatmullRom);
        let image = egui::ColorImage::from_rgba_unmultiplied([big.width() as usize, big.height() as usize], big.as_raw());
        let texture = ctx.load_texture(format!("dexnav_smooth_{at}"), image, egui::TextureOptions::LINEAR);
        self.smooth.insert(at, texture.clone());
        Some(texture)
    }

    fn pictures_tab(&mut self, ui: &mut egui::Ui) {
        let Some(navi) = self.navi.as_ref() else { return };
        let pictures: Vec<(usize, String, bool)> =
            navi.entries.iter().enumerate().filter(|(_, e)| e.kind == Kind::Picture).map(|(at, e)| (at, describe(&e.name), navi.picture_changed(e))).collect();
        let Some(current) = pictures.iter().find(|p| p.0 == self.chosen).cloned() else { return };
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Picture"));
                let mut value = self.chosen as u16;
                picker(ui, "dexnav_picture", &format!("{}{}", current.1, if current.2 { " (changed)" } else { "" }), 340.0, &mut value, None, || {
                    vec![Group { title: "Pictures".to_string(), entries: pictures.iter().map(|(at, name, changed)| (*at as u16, format!("{name}{}", if *changed { " (changed)" } else { "" }))).collect() }]
                });
                self.chosen = value as usize;
            });
        });
        ui.add_space(6.0);
        let at = self.chosen;
        let Some(navi) = self.navi.as_ref() else { return };
        let entry = navi.entries[at].clone();
        let info = navi.picture(&entry).ok().map(|p| (p.width(), p.height(), format!("{:?}", p.format())));
        let texture = self.texture(ui.ctx(), at);
        widget::card(ui, |ui| {
            if let Some((w, h, format)) = &info {
                ui.label(widget::hint(&format!("{w} × {h} pixels, format {format}. A picture that you import must have the same size.")));
            }
            if let Some(texture) = &texture {
                let size = texture.size_vec2();
                let scale = (480.0 / size.x).min(300.0 / size.y).clamp(1.0, 4.0);
                let scale = if size.x <= 64.0 { scale.floor().max(1.0) } else { scale };
                let (rect, _) = ui.allocate_exact_size(size * scale + egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(70));
                ui.painter().image(texture.id(), rect.shrink(4.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            } else {
                ui.label(egui::RichText::new("This picture cannot be shown or changed.").color(color::AMBER));
            }
            ui.add_space(6.0);
            let editable = texture.is_some();
            let mut message = None;
            ui.horizontal_wrapped(|ui| {
                if widget::button(ui, "Export as PNG") {
                    if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}.png", entry.name.trim_end_matches(".bclim"))).add_filter("PNG", &["png"]).save_file() {
                        message = Some(match self.export(at, &path) {
                            Ok(()) => (true, format!("Exported to {}.", path.display())),
                            Err(e) => (false, e),
                        });
                    }
                }
                ui.add_enabled_ui(editable, |ui| {
                    if widget::button(ui, "Import a PNG") {
                        if let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).pick_file() {
                            message = Some(match self.import(at, &path) {
                                Ok(()) => (true, "The picture is changed. Save the DexNav to use it in the game.".to_string()),
                                Err(e) => (false, e),
                            });
                        }
                    }
                });
                if self.navi.as_ref().is_some_and(|n| n.picture_changed(&entry)) && widget::confirm_button(ui, "Reset this picture", "Click again to reset") {
                    if let Some(navi) = self.navi.as_mut() {
                        navi.reset_picture(&entry);
                    }
                    self.textures.remove(&at);
                    self.smooth.remove(&at);
                    message = Some((true, "The picture of the game is back.".to_string()));
                }
            });
            ui.horizontal_wrapped(|ui| {
                if widget::button(ui, "Export all the pictures") {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        message = Some(self.export_all(&folder));
                    }
                }
                if widget::button(ui, "Import the pictures of a folder") {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        message = Some(self.import_all(&folder));
                    }
                }
            });
            ui.label(widget::hint("The folder has one PNG file for each picture, named like the exported ones. The pictures that are not in the folder stay as they are."));
            if message.is_some() {
                self.status = message;
            }
        });
    }

    fn export(&self, at: usize, path: &Path) -> Result<(), String> {
        let navi = self.navi.as_ref().ok_or("The DexNav is not open.")?;
        let picture = navi.picture(&navi.entries[at])?;
        write_png(path, picture.width(), picture.height(), &picture.to_rgba()?)
    }

    fn import(&mut self, at: usize, path: &Path) -> Result<(), String> {
        let (width, height, rgba) = read_png(path)?;
        let navi = self.navi.as_mut().ok_or("The DexNav is not open.")?;
        let entry = navi.entries[at].clone();
        let picture = navi.picture(&entry)?;
        if (width, height) != (picture.width(), picture.height()) {
            return Err(format!("The picture must be {} × {} pixels: this one is {width} × {height}.", picture.width(), picture.height()));
        }
        navi.set_picture(&entry, &rgba)?;
        self.textures.remove(&at);
        self.smooth.remove(&at);
        Ok(())
    }

    fn export_all(&self, folder: &Path) -> (bool, String) {
        let Some(navi) = self.navi.as_ref() else { return (false, "The DexNav is not open.".into()) };
        let mut done = 0;
        for (at, entry) in navi.entries.iter().enumerate().filter(|(_, e)| e.kind == Kind::Picture) {
            let path = folder.join(format!("{}.png", entry.name.trim_end_matches(".bclim")));
            match self.export(at, &path) {
                Ok(()) => done += 1,
                Err(e) if entry.name == "kirakira.bclim" => {
                    let _ = e;
                }
                Err(e) => return (false, format!("{}: {e}", entry.name)),
            }
        }
        (true, format!("{done} pictures exported to {}.", folder.display()))
    }

    fn import_all(&mut self, folder: &Path) -> (bool, String) {
        let Some(navi) = self.navi.as_ref() else { return (false, "The DexNav is not open.".into()) };
        let todo: Vec<(usize, PathBuf, String)> = navi
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == Kind::Picture)
            .map(|(at, e)| (at, folder.join(format!("{}.png", e.name.trim_end_matches(".bclim"))), e.name.clone()))
            .filter(|(_, path, _)| path.exists())
            .collect();
        let mut done = 0;
        for (at, path, name) in todo {
            if let Err(e) = self.import(at, &path) {
                return (false, format!("{name}: {e}"));
            }
            done += 1;
        }
        (true, format!("{done} pictures imported. Save the DexNav to use them in the game."))
    }


    /// The Pokémon that the DexNav lists for a map, for each kind of place, as the game builds the list: a kind counts only when its rate is
    /// not 0, and a Pokémon is listed once for each form.
    fn listed(table: &Encounter) -> (Vec<u16>, Vec<u16>, Vec<u16>) {
        let unique = |methods: &[usize]| -> Vec<u16> {
            let mut seen: Vec<(u16, u8)> = Vec::new();
            for &m in methods {
                if table.rates[METHODS[m].rate_at] == 0 {
                    continue;
                }
                for slot in table.places(&METHODS[m]) {
                    if slot.species != 0 && !seen.contains(&(slot.species, slot.form)) {
                        seen.push((slot.species, slot.form));
                    }
                }
            }
            seen.into_iter().map(|s| s.0).collect()
        };
        // The extra places go with the ground or with the water, as the rate of the table says.
        let ground = if table.rates[2] == 1 { unique(&[0, 1, 8, 2]) } else { unique(&[0, 1, 8]) };
        let water = if table.rates[2] == 2 { unique(&[3, 2]) } else { unique(&[3]) };
        (ground, water, unique(&[5, 6, 7]))
    }

    /// The screen of the DexNav for a map: the background of its type, with the Pokémon of the map at the places of the type. A place can be
    /// dragged.
    fn screen_tab(&mut self, ui: &mut egui::Ui, map: usize, table: Option<&Encounter>, icons: &mut SpeciesIcons) {
        let Some(navi) = self.navi.as_ref() else { return };
        let map_type = self.map_types.iter().find(|(z, _)| *z == map).map(|p| p.1);
        let kind = self.view_type.unwrap_or(map_type.unwrap_or(0)).min(TYPE_COUNT - 1);
        let changed: Vec<bool> = (0..TYPE_COUNT).map(|k| navi.type_changed(k)).collect();
        let places = match navi.type_places(kind) {
            Ok(places) => places,
            Err(e) => {
                ui.label(egui::RichText::new(e).color(color::ROSE));
                return;
            }
        };
        let panes = navi.panes().unwrap_or_default();
        let background = navi.type_background(kind).ok().and_then(|name| navi.entries.iter().position(|e| e.name == name));
        widget::card(ui, |ui| {
            ui.label(widget::hint(&match map_type {
                Some(t) => format!("For this map, the game shows the screen of type {}.", t + 1),
                None => "This map is not in the list of the game: the game shows the screen of type 1.".to_string(),
            }));
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Screen"));
                let mut value = kind as u16;
                let shown = format!("Type {}{}", kind + 1, if changed[kind] { " (changed)" } else { "" });
                picker(ui, "dexnav_type", &shown, 220.0, &mut value, None, || {
                    vec![Group {
                        title: "Types of screen".to_string(),
                        entries: (0..TYPE_COUNT)
                            .map(|k| (k as u16, format!("Type {}{}{}", k + 1, if Some(k) == map_type { " — this map" } else { "" }, if changed[k] { " (changed)" } else { "" })))
                            .collect(),
                    }]
                });
                self.view_type = (value as usize != map_type.unwrap_or(0)).then_some(value as usize);
            });
            ui.label(widget::hint(
                "The game lists at most 20 Pokémon (15 on the ground, 9 in the water, 6 with the rods) and a form counts as one Pokémon. Drag a Pokémon (or an empty place) to move it. A type of screen is shared: all the maps with this type use the new places. The Pokémon are \
                 given to the places in the order of the game's list, so the real order can differ a little. These changes are not tested in the game.",
            ));
        });
        ui.add_space(6.0);
        // What goes where.
        let (ground, water, fishing) = table.map(Self::listed).unwrap_or_default();
        let over = (ground.len() > 15) || (water.len() > 9) || (fishing.len() > 6) || (ground.len() + water.len() + fishing.len() > 20);
        if over {
            ui.label(
                egui::RichText::new(format!(
                    "This map lists {} Pokémon on the ground (limit 15), {} in the water (limit 9), {} with the rods (limit 6), {} in all (limit 20). The game has no place for the others and can crash.",
                    ground.len(), water.len(), fishing.len(), ground.len() + water.len() + fishing.len()
                ))
                .color(color::AMBER),
            );
        }
        let numbers = |group: &str| PARTS.iter().filter(|p| p.2 == group).map(|p| p.0).collect::<Vec<u32>>();
        let mut assigned: Vec<(u32, u16)> = Vec::new();
        for (group, list) in [(GROUND, &ground), (WATER, &water), (FISHING, &fishing)] {
            for (number, species) in numbers(group).into_iter().zip(list.iter()) {
                assigned.push((number, *species));
            }
        }
        // The drawing: the bottom screen of 320 × 240, the center is (0, 0) and y goes up.
        let scale = ((ui.available_width() - 16.0) / 320.0).clamp(1.0, 2.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(320.0 * scale, 240.0 * scale), egui::Sense::click_and_drag());
        let to_screen = |x: f32, y: f32| egui::pos2(rect.center().x + x * scale, rect.center().y - y * scale);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(30));
        let pane_size = panes.iter().find(|p| p.number == MAP_PICTURE).map(|p| (p.width.round() as u32, p.height.round() as u32)).unwrap_or((238, 168));
        if let Some(texture) = background.and_then(|at| self.smooth_texture(ui.ctx(), at, pane_size.0, pane_size.1)) {
            // The picture has the size of its pane. The game draws it 19 units further to the left than the place of the pane says (measured on
            // a screenshot of the game), and the frame of the screen hides what is outside of 2 ... 242 and 22 ... 198.
            let pane = panes.iter().find(|p| p.number == MAP_PICTURE);
            let (left, top, width, height) = pane.map(|p| (p.parent.0 + p.x - 19.0, p.parent.1 + p.y + p.height / 2.0 + 2.0, p.width, p.height)).unwrap_or((-179.0, 99.0, 238.0, 168.0));
            let full = egui::Rect::from_two_pos(to_screen(left, top), to_screen(left + width, top - height));
            let frame = egui::Rect::from_two_pos(to_screen(-158.0, 98.0), to_screen(82.0, -78.0));
            let shown = full.intersect(frame);
            let uv = egui::Rect::from_min_max(
                egui::pos2((shown.min.x - full.min.x) / full.width(), (shown.min.y - full.min.y) / full.height()),
                egui::pos2((shown.max.x - full.min.x) / full.width(), (shown.max.y - full.min.y) / full.height()),
            );
            painter.image(texture.id(), shown, uv, egui::Color32::WHITE);
        }
        let origin = panes.iter().find(|p| p.number == PLACES_PARENT).map(|p| (p.parent.0 + p.x, p.parent.1 + p.y)).unwrap_or((-35.0, 15.0));
        let group_color = |group: &str| match group {
            GROUND => egui::Color32::from_rgb(34, 160, 80),
            WATER => egui::Color32::from_rgb(59, 130, 246),
            _ => egui::Color32::from_rgb(168, 85, 247),
        };
        let parts: Vec<(usize, TypePlace)> =
            PARTS.iter().enumerate().filter(|(_, p)| p.2.starts_with("Places")).filter_map(|(i, p)| places.iter().find(|t| t.number == p.0).map(|t| (i, *t))).collect();
        let box_of = |t: &TypePlace| egui::Rect::from_two_pos(to_screen(origin.0 + t.x - 20.0, origin.1 + t.y + 15.0), to_screen(origin.0 + t.x + 20.0, origin.1 + t.y - 15.0));
        for (i, t) in &parts {
            let r = box_of(t);
            if !r.intersects(rect) {
                continue;
            }
            let tint = group_color(PARTS[*i].2);
            let selected = *i == self.part;
            let species = assigned.iter().find(|(n, _)| *n == t.number).map(|(_, s)| *s);
            match species.and_then(|s| icons.get(ui.ctx(), s as usize).cloned()) {
                Some(texture) => {
                    painter.image(texture.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    painter.rect_stroke(r, 2.0, egui::Stroke::new(if selected { 2.5_f32 } else { 1.0_f32 }, tint));
                }
                None => {
                    painter.rect(r, 2.0, tint.gamma_multiply(if selected { 0.45 } else { 0.18 }), egui::Stroke::new(if selected { 2.5_f32 } else { 1.0_f32 }, tint));
                    painter.text(r.center(), egui::Align2::CENTER_CENTER, PARTS[*i].1.rsplit(' ').next().unwrap_or(""), egui::FontId::proportional(11.0), egui::Color32::WHITE);
                }
            }
        }
        let mut moves: Vec<(TypePlace, f32, f32)> = Vec::new();
        if let Some(pointer) = response.interact_pointer_pos() {
            if response.drag_started() || response.clicked() {
                if let Some((i, t)) = parts.iter().rev().find(|(_, t)| box_of(t).contains(pointer)) {
                    self.part = *i;
                    self.drag = Some((pointer, (t.x, t.y)));
                }
            }
            if response.dragged() {
                if let (Some((start, (x0, y0))), Some((_, t))) = (self.drag, parts.iter().find(|(i, _)| *i == self.part)) {
                    let delta = (pointer - start) / scale;
                    moves.push((*t, (x0 + delta.x).round(), (y0 - delta.y).round()));
                }
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }
        ui.add_space(6.0);
        // The numbers of the selected place.
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Place"));
                let mut value = self.part as u16;
                let shown = format!("{} — {}", PARTS[self.part].2.trim_start_matches("Places of the Pokémon of the "), PARTS[self.part].1);
                picker(ui, "dexnav_part", &shown, 300.0, &mut value, None, || {
                    let mut groups: Vec<Group> = Vec::new();
                    for (i, (_, name, group)) in PARTS.iter().enumerate().filter(|(_, p)| p.2.starts_with("Places")) {
                        match groups.iter_mut().find(|g| g.title == *group) {
                            Some(g) => g.entries.push((i as u16, name.to_string())),
                            None => groups.push(Group { title: group.to_string(), entries: vec![(i as u16, name.to_string())] }),
                        }
                    }
                    groups
                });
                self.part = (value as usize).min(PARTS.len() - 1);
                if let Some((_, t)) = parts.iter().find(|(i, _)| *i == self.part) {
                    let (mut x, mut y) = (t.x, t.y);
                    ui.label(widget::hint("X"));
                    widget::spin(ui, &mut x, -2000.0, 2000.0);
                    ui.label(widget::hint("Y"));
                    widget::spin(ui, &mut y, -2000.0, 2000.0);
                    if (x, y) != (t.x, t.y) {
                        moves.push((*t, x, y));
                    }
                }
                if changed[kind] && widget::confirm_button(ui, "Reset this screen", "Click again to reset") {
                    if let Some(navi) = self.navi.as_mut() {
                        navi.reset_type(kind);
                    }
                }
            });
        });
        if let Some(navi) = self.navi.as_mut() {
            for (place, x, y) in moves {
                navi.set_type_place(&place, x, y);
            }
        }
    }
}
