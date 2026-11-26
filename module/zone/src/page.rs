//! The Map Data page: a map is chosen at the top, and tabs show its values.

use crate::data::{self, Camera, Edit, Field, Settings, Store, CAMERA_COUNT, CAMERA_VALUES};
use crate::lists::{self, BATTLE_BACKGROUNDS, MAP_CHANGES, NAME_WINDOWS, WEATHERS};
use sango_core::text::TextFile;
use sango_core::{Dump, SdCard};
use sango_overworld::names::MAP_NAMES;
use sango_ui::history::History;
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module, Session};
use std::path::PathBuf;

const TABS: [&str; 6] = ["Place", "Rules", "Look", "Positions", "Camera", "Advanced"];
/// The English language, for the lists.
const ENGLISH: usize = 2;
const FIRST_ZONE: usize = 6;

pub struct ZonePage {
    opened_for: Option<(PathBuf, PathBuf)>,
    store: Option<Store>,
    error: Option<String>,
    zone: usize,
    /// The map as the page has it, as it is on the card, and as it is in the game.
    edit: Option<Edit>,
    saved: Option<Edit>,
    game: Option<Edit>,
    changed: Vec<usize>,
    /// The files of the names of the places, as they are on the card.
    name_files: Vec<Option<TextFile>>,
    /// The cameras of the game for the area of the Camera tab: the area, the camera that it uses, the cameras.
    game_cameras: Option<(usize, u16, Vec<Camera>)>,
    /// The names of the places in English, by their line, for the menu.
    places: Vec<(u16, String)>,
    musics: Vec<(i64, String)>,
    tab: usize,
    language: usize,
    /// The camera that the Camera tab shows.
    camera_shown: usize,
    status: Option<(bool, String)>,
    reset_open: bool,
    /// What the button Reset puts back: the settings, the water, the camera, the name.
    reset_parts: [bool; 4],
    /// The map that the user chose while the open one had changes: the window asks first.
    pending: Option<usize>,
    history: History<Option<Edit>>,
}

impl Default for ZonePage {
    fn default() -> Self {
        ZonePage { opened_for: None, store: None, error: None, zone: FIRST_ZONE, edit: None, saved: None, game: None, changed: Vec::new(), name_files: Vec::new(), game_cameras: None, places: Vec::new(), musics: lists::musics(), tab: 0, language: ENGLISH, camera_shown: 0, status: None, reset_open: false, reset_parts: [true; 4], pending: None, history: History::default() }
    }
}

/// The name of a map in the menus.
fn map_label(zone: usize) -> String {
    format!("{} [{zone}]", MAP_NAMES.get(zone).copied().unwrap_or("Map"))
}

/// The name of a value with its ↺ when it is not the one of the game.
fn name_cell(ui: &mut egui::Ui, label: &str, note: &str, settings: &mut Settings, game: &Settings, field: Field) {
    let changed = settings.get(field) != game.get(field);
    let back = ui
        .horizontal(|ui| {
            let (name, back) = widget::field_name(ui, label, changed);
            if !note.is_empty() {
                name.on_hover_text(note);
            }
            back
        })
        .inner;
    if back {
        settings.set(field, game.get(field));
    }
}

/// A line of a grid: a menu of the names of a list. `hints` says what each entry does, in the same order (empty: nothing to say).
fn list_row(ui: &mut egui::Ui, label: &str, note: &str, settings: &mut Settings, game: &Settings, field: Field, entries: &[(i64, String)], hints: &[&str]) {
    name_cell(ui, label, note, settings, game, field);
    let mut value = settings.get(field);
    let shown = entries.iter().find(|(v, _)| *v == value).map_or_else(|| format!("Value {value}"), |(_, name)| name.clone());
    let combo = egui::ComboBox::from_id_salt(("zone_list", field.key)).selected_text(shown).width(260.0).height(360.0).show_ui(ui, |ui| {
        for (place, (v, name)) in entries.iter().enumerate() {
            let mut row = ui.selectable_label(*v == value, name);
            if let Some(hint) = hints.get(place) {
                row = row.on_hover_text(*hint);
            }
            if row.clicked() {
                value = *v;
            }
        }
    });
    if let Some(hint) = entries.iter().position(|(v, _)| *v == value).and_then(|place| hints.get(place)) {
        combo.response.clone().on_hover_text(*hint);
    }
    let values: Vec<i64> = entries.iter().map(|(v, _)| *v).collect();
    focus::combo_arrows(ui, &combo.response, &mut value, &values);
    settings.set(field, value);
    ui.end_row();
}

fn numbered(list: &[&str]) -> Vec<(i64, String)> {
    list.iter().enumerate().map(|(n, name)| (n as i64, name.to_string())).collect()
}

/// A line of a grid: a check box.
fn flag_row(ui: &mut egui::Ui, label: &str, note: &str, settings: &mut Settings, game: &Settings, field: Field) {
    name_cell(ui, label, note, settings, game, field);
    let mut on = settings.get(field) != 0;
    ui.checkbox(&mut on, "").on_hover_text(note);
    settings.set(field, i64::from(on));
    ui.end_row();
}

/// A line of a grid: a number.
fn number_row(ui: &mut egui::Ui, label: &str, note: &str, settings: &mut Settings, game: &Settings, field: Field) {
    name_cell(ui, label, note, settings, game, field);
    let mut value = settings.get(field);
    let (low, high) = field.range();
    widget::spin(ui, &mut value, low as f64, high as f64);
    settings.set(field, value);
    ui.end_row();
}

/// A grid of two columns.
fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing(egui::vec2(18.0, 8.0)).min_col_width(170.0).show(ui, rows);
}

impl ZonePage {
    fn open(&mut self, dump: &Dump, sd: Option<&SdCard>) {
        self.edit = None;
        self.status = None;
        self.pending = None;
        self.game_cameras = None;
        match Store::open(dump) {
            Ok(store) => {
                self.store = Some(store);
                self.error = None;
                self.refresh_names(sd);
                self.refresh_changed(sd);
                self.load(sd, self.zone);
            }
            Err(message) => (self.store, self.error) = (None, Some(message)),
        }
    }

    /// Reads the names of the places again, and makes the list of the places for the menu.
    fn refresh_names(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        self.name_files = store.name_files(sd);
        // A name, then its second text, for each place.
        self.places = self.name_files.get(ENGLISH).and_then(Option::as_ref).map_or_else(Vec::new, |file| {
            let lines = (0..file.len().min(1024) as u16).step_by(2);
            lines.filter_map(|line| file.text(line as usize).filter(|name| !name.is_empty()).map(|name| (line, format!("{name} ({line})")))).collect()
        });
    }

    fn refresh_changed(&mut self, sd: Option<&SdCard>) {
        if let Some(store) = &self.store {
            self.changed = store.changed_zones(sd);
        }
    }

    fn load(&mut self, sd: Option<&SdCard>, zone: usize) {
        let Some(store) = &self.store else { return };
        self.zone = zone;
        self.history.clear();
        match store.edit(sd, zone).and_then(|edit| Ok((edit, store.edit(None, zone)?))) {
            Ok((edit, game)) => {
                self.camera_shown = edit.camera as usize;
                (self.saved, self.edit, self.game) = (Some(edit.clone()), Some(edit), Some(game));
            }
            Err(message) => {
                (self.saved, self.edit, self.game) = (None, None, None);
                self.status = Some((false, message));
            }
        }
    }

    /// True when the open map is not as it is on the card. A name counts only when its text is not the one of the card.
    fn dirty(&self) -> bool {
        let (Some(edit), Some(saved)) = (&self.edit, &self.saved) else { return false };
        (edit.settings, edit.water, edit.area, edit.camera) != (saved.settings, saved.water, saved.area, saved.camera)
            || edit.cameras != saved.cameras
            || edit.names.iter().any(|(&(language, line), name)| data::name_of(&self.name_files, language, line).as_ref() != Some(name))
    }

    fn save(&mut self, sd: &SdCard) {
        let (Some(store), Some(edit)) = (&self.store, &self.edit) else { return };
        match store.save(sd, edit) {
            Ok(()) => {
                self.saved = self.edit.clone();
                self.status = Some((true, "Saved. Restart the game to see the change.".to_string()));
                self.refresh_names(Some(sd));
                self.refresh_changed(Some(sd));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Puts the parts that are checked back to the game, and writes the map.
    fn reset(&mut self, sd: &SdCard) {
        let (Some(store), Some(edit), Some(game)) = (&self.store, &mut self.edit, &self.game) else { return };
        let [settings, water, camera, name] = self.reset_parts;
        if settings {
            edit.settings = game.settings;
        }
        if water {
            edit.water = game.water;
        }
        // The cameras are the ones of the area of the settings.
        let area = edit.settings.get(data::AREA) as usize;
        if camera || area != edit.area {
            match store.cameras(if camera { None } else { Some(sd) }, area) {
                Ok((used, cameras)) => (edit.area, edit.camera, edit.cameras) = (area, used, cameras),
                Err(message) => {
                    self.status = Some((false, message));
                    return;
                }
            }
        }
        if name {
            for (&(language, line), text) in edit.names.iter_mut() {
                if let Some(original) = store.game_name_file(language).and_then(|file| file.text(line)) {
                    *text = original;
                }
            }
        }
        data::add_names(&self.name_files, edit);
        self.save(sd);
    }

    fn export(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        let Some(path) = rfd::FileDialog::new().set_title("Export the settings of all the maps").set_file_name("maps.csv").add_filter("CSV", &["csv"]).save_file() else { return };
        let result = store.to_csv(sd).and_then(|csv| std::fs::write(&path, csv).map_err(|e| e.to_string()));
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(message) => (false, format!("{}: {message}", path.display())),
        });
    }

    fn import(&mut self, sd: &SdCard) {
        let Some(path) = rfd::FileDialog::new().set_title("Import settings of maps").add_filter("CSV", &["csv"]).pick_file() else { return };
        let Some(store) = &self.store else { return };
        let result = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|csv| store.import_csv(sd, &csv));
        match result {
            Ok(count) => {
                self.refresh_changed(Some(sd));
                self.load(Some(sd), self.zone);
                self.status = Some((true, format!("Imported: {count} map(s) changed. Restart the game.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let dirty = self.dirty();
        let mut chosen = self.zone;
        let mut clicks = widget::ActionClicks::default();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Map"));
                let count = self.store.as_ref().map_or(0, Store::zone_count);
                let changed = self.changed.clone();
                let mut value = self.zone as u16;
                picker(ui, "zone_map", &map_label(self.zone), 360.0, &mut value, None, || {
                    let entry = |zone: usize| (zone as u16, map_label(zone));
                    let mut groups = Vec::new();
                    if !changed.is_empty() {
                        groups.push(Group { title: "Changed by you".to_string(), entries: changed.iter().map(|&z| entry(z)).collect() });
                    }
                    groups.push(Group { title: "Maps of the game".to_string(), entries: (0..count).map(entry).collect() });
                    groups
                });
                chosen = value as usize;
                let card = sd.is_some() && self.edit.is_some();
                clicks = widget::action_bar(
                    ui,
                    widget::ActionBar {
                        export: Some(self.store.is_some()),
                        import: Some(card),
                        reset: Some(card),
                        save: Some(card && dirty),
                        notes: widget::ActionNotes { export: "Writes the settings of all the maps in a CSV file.", import: "Reads a CSV file of settings of maps and writes the maps that change.", reset: "Puts this map back to the game: choose the parts in the window.", ..Default::default() },
                        ..Default::default()
                    },
                );
                self.history.buttons(ui, self.edit.is_some());
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.changed.contains(&self.zone) {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can look and export, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        if chosen != self.zone {
            if dirty {
                self.pending = Some(chosen);
            } else {
                self.status = None;
                self.load(sd, chosen);
            }
        }
        if clicks.export {
            self.export(sd);
        }
        if let (true, Some(sd)) = (clicks.import, sd) {
            self.import(sd);
        }
        self.reset_open |= clicks.reset;
        if let (true, Some(sd)) = (clicks.save, sd) {
            self.save(sd);
        }
    }

    fn windows(&mut self, ctx: &egui::Context, sd: Option<&SdCard>) {
        if let Some(zone) = self.pending {
            let what = format!("the map {}", map_label(self.zone));
            match widget::unsaved_window(ctx, &what) {
                Some(widget::Unsaved::Save) => {
                    if let Some(sd) = sd {
                        self.save(sd);
                    }
                    if !self.dirty() {
                        self.load(sd, zone);
                    }
                    self.pending = None;
                }
                Some(widget::Unsaved::Discard) => {
                    self.load(sd, zone);
                    self.pending = None;
                }
                Some(widget::Unsaved::Stay) => self.pending = None,
                None => {}
            }
        }
        let what = format!("The map {}", map_label(self.zone));
        let [settings, water, camera, name] = &mut self.reset_parts;
        if widget::reset_window(ctx, &mut self.reset_open, &what, &mut [("Settings", settings), ("Water", water), ("Camera of its area", camera), ("Name of the place", name)]) {
            if let Some(sd) = sd {
                self.reset(sd);
            }
        }
    }

    fn place_tab(&mut self, ui: &mut egui::Ui, edit: &mut Edit, game: &Edit) {
        let places = self.places.clone();
        let musics = self.musics.clone();
        grid(ui, "zone_place", |ui| {
            name_cell(ui, "Place", "The name that the game shows for this map. Many maps can use the same place.", &mut edit.settings, &game.settings, data::PLACE_NAME);
            let mut line = edit.settings.get(data::PLACE_NAME) as u16;
            let shown = places.iter().find(|(l, _)| *l == line).map_or_else(|| format!("Place {line}"), |(_, name)| name.clone());
            picker(ui, "zone_place_name", &shown, 300.0, &mut line, None, || vec![Group { title: String::new(), entries: places.clone() }]);
            edit.settings.set(data::PLACE_NAME, i64::from(line));
            ui.end_row();

            data::add_names(&self.name_files, edit);
            let line = edit.settings.get(data::PLACE_NAME) as usize;
            let language = self.language;
            let original = self.store.as_ref().and_then(|s| s.game_name_file(language)).and_then(|file| file.text(line));
            let changed = original.is_some() && edit.names.get(&(language, line)) != original.as_ref();
            let back = ui
                .horizontal(|ui| {
                    let (name, back) = widget::field_name(ui, "Name of the place", changed);
                    name.on_hover_text("A change of this text shows on every map that uses this place.");
                    back
                })
                .inner;
            ui.horizontal(|ui| {
                sango_ui::language::menu(ui, "zone_language", &mut self.language, None);
                let key = (self.language, line);
                if let Some(text) = edit.names.get_mut(&key) {
                    sango_ui::game_text::edit_line(ui, ("zone_name", key), text, 280.0);
                }
            });
            if back {
                if let Some(original) = original {
                    edit.names.insert((language, line), original);
                }
            }
            ui.end_row();

            list_row(ui, "Name window", "How the name shows when the player comes in.", &mut edit.settings, &game.settings, data::NAME_WINDOW, &numbered(&NAME_WINDOWS), &[]);
            list_row(ui, "Music", "The music of the map.", &mut edit.settings, &game.settings, data::MUSIC, &musics, &[]);
            list_row(ui, "Weather", "The weather when the player comes in. Scripts can change it.", &mut edit.settings, &game.settings, data::WEATHER, &numbered(&WEATHERS), &[]);
            list_row(ui, "Battle background", "The place that shows behind the Pokémon in a battle on this map.", &mut edit.settings, &game.settings, data::BATTLE_BACKGROUND, &numbered(&BATTLE_BACKGROUNDS), &[]);
            let changes: Vec<(i64, String)> = MAP_CHANGES.iter().map(|&(value, name, _)| (value, name.to_string())).collect();
            let hints = MAP_CHANGES.map(|(_, _, hint)| hint);
            let note = "The fade of the screen when the player goes through a door or an exit. The game looks at this value of the two maps. Maps that touch have no fade.";
            list_row(ui, "Map change", note, &mut edit.settings, &game.settings, data::MAP_CHANGE, &changes, &hints);
        });
    }

    fn rules_tab(ui: &mut egui::Ui, edit: &mut Edit, game: &Edit) {
        let (settings, game) = (&mut edit.settings, &game.settings);
        grid(ui, "zone_rules", |ui| {
            flag_row(ui, "Fly", "The player can use Fly from this map.", settings, game, data::FLY);
            flag_row(ui, "Bicycle", "The player can ride a Bicycle.", settings, game, data::BICYCLE);
            flag_row(ui, "Bicycle music", "The music changes while the player rides the Bicycle.", settings, game, data::BICYCLE_MUSIC);
            flag_row(ui, "Running", "The player can run.", settings, game, data::RUNNING);
            flag_row(ui, "Roller Skates", "The player can use Roller Skates (X and Y).", settings, game, data::ROLLER_SKATES);
            flag_row(ui, "Escape Rope", "The player can leave with an Escape Rope or Dig.", settings, game, data::ESCAPE_ROPE);
            flag_row(ui, "Dowsing Machine", "The player can use the Dowsing Machine.", settings, game, data::DOWSING);
            flag_row(ui, "Communication", "The functions that talk to other players work.", settings, game, data::COMMUNICATION);
        });
    }

    fn look_tab(ui: &mut egui::Ui, edit: &mut Edit, game: &Edit) {
        let (settings, game_settings) = (&mut edit.settings, &game.settings);
        grid(ui, "zone_look", |ui| {
            flag_row(ui, "Sky", "The sky is drawn around the map.", settings, game_settings, data::SKY);
            flag_row(ui, "Glow", "Bright things glow (bloom).", settings, game_settings, data::BLOOM);
            flag_row(ui, "Dark cave", "The map is dark until the player uses Flash.", settings, game_settings, data::FLASH);
            flag_row(ui, "Echo", "The sounds have an echo.", settings, game_settings, data::REVERB);
            flag_row(ui, "3D", "The map shows in 3D with the slider of the console.", settings, game_settings, data::STEREO);
            flag_row(ui, "Idle motions", "The player moves a little while standing.", settings, game_settings, data::IDLE_MOTION);
            flag_row(ui, "Visible breath", "The breath of the player shows in the cold.", settings, game_settings, data::BREATH);
            number_row(ui, "Lights", "The most lights on the map. The game has 4 everywhere.", settings, game_settings, data::LIGHTS);
        });
        ui.add_space(12.0);
        ui.label(widget::title("Water"));
        ui.label(widget::hint("Four surfaces of water at most, each one with its height."));
        grid(ui, "zone_water", |ui| {
            for surface in 0..4 {
                let changed = edit.water.on(surface) != game.water.on(surface) || edit.water.height(surface) != game.water.height(surface);
                let back = ui.horizontal(|ui| widget::field_name(ui, &format!("Surface {}", surface + 1), changed).1).inner;
                ui.horizontal(|ui| {
                    let mut on = edit.water.on(surface);
                    ui.checkbox(&mut on, "On");
                    edit.water.set_on(surface, on);
                    let mut height = edit.water.height(surface);
                    ui.label(widget::hint("Height"));
                    widget::spin(ui, &mut height, f64::from(i16::MIN), f64::from(i16::MAX));
                    edit.water.set_height(surface, height);
                });
                if back {
                    edit.water.set_on(surface, game.water.on(surface));
                    edit.water.set_height(surface, game.water.height(surface));
                }
                ui.end_row();
            }
        });
    }

    fn positions_tab(ui: &mut egui::Ui, edit: &mut Edit, game: &Edit, count: usize) {
        let (settings, game) = (&mut edit.settings, &game.settings);
        ui.label(widget::title("Fly"));
        ui.label(widget::hint("Where the player arrives with Fly, when this map is chosen on the Town Map."));
        ui.add_space(6.0);
        grid(ui, "zone_fly", |ui| {
            name_cell(ui, "Arrival map", "The map where the player arrives.", settings, game, data::FLY_MAP);
            let mut map = settings.get(data::FLY_MAP) as u16;
            picker(ui, "zone_fly_map", &map_label(map as usize), 360.0, &mut map, None, || vec![Group { title: String::new(), entries: (0..count).map(|z| (z as u16, map_label(z))).collect() }]);
            settings.set(data::FLY_MAP, i64::from(map));
            ui.end_row();
            number_row(ui, "X", "From left to right. A tile is 18 long.", settings, game, data::FLY_X);
            number_row(ui, "Height", "", settings, game, data::FLY_Y);
            number_row(ui, "Z", "From top to bottom. A tile is 18 long.", settings, game, data::FLY_Z);
        });
        ui.add_space(12.0);
        ui.label(widget::title("Start position"));
        ui.label(widget::hint("The position of the map when the game has no other one."));
        ui.add_space(6.0);
        grid(ui, "zone_start", |ui| {
            number_row(ui, "X", "From left to right. A tile is 18 long.", settings, game, data::START_X);
            number_row(ui, "Height", "", settings, game, data::START_Y);
            number_row(ui, "Z", "From top to bottom. A tile is 18 long.", settings, game, data::START_Z);
        });
    }

    fn camera_tab(&mut self, ui: &mut egui::Ui, edit: &mut Edit, game: &Edit) {
        let maps = self.store.as_ref().map_or(0, |store| store.maps_of_area(edit.area));
        ui.label(widget::hint(&format!("The cameras belong to the area {} of the map. {maps} map(s) use this area: a change shows on all of them.", edit.area)));
        ui.add_space(6.0);
        // The game values are the ones of the same area (the area of the map can be changed).
        if self.game_cameras.as_ref().map(|c| c.0) != Some(edit.area) {
            self.game_cameras = self.store.as_ref().and_then(|store| store.cameras(None, edit.area).ok()).map(|(used, cameras)| (edit.area, used, cameras));
        }
        let (original_used, original) = self.game_cameras.as_ref().map_or((game.camera, &game.cameras), |c| (c.1, &c.2));
        let mut shown_camera = self.camera_shown;
        let entries: Vec<(usize, String)> = (0..CAMERA_COUNT).map(|n| (n, format!("Camera {n}"))).collect();
        let menu = |ui: &mut egui::Ui, id: &str, value: &mut usize| {
            let combo = egui::ComboBox::from_id_salt(id).selected_text(format!("Camera {value}")).width(200.0).height(420.0).show_ui(ui, |ui| {
                for (n, name) in &entries {
                    ui.selectable_value(value, *n, name);
                }
            });
            let values: Vec<usize> = entries.iter().map(|(n, _)| *n).collect();
            focus::combo_arrows(ui, &combo.response, value, &values);
        };
        grid(ui, "zone_camera_choice", |ui| {
            let back = ui
                .horizontal(|ui| {
                    let (name, back) = widget::field_name(ui, "Camera of the area", edit.camera != original_used);
                    name.on_hover_text("The camera that the maps of this area use.");
                    back
                })
                .inner;
            let mut used = usize::from(edit.camera);
            menu(ui, "zone_camera_used", &mut used);
            if back {
                used = usize::from(original_used);
            }
            if used != usize::from(edit.camera) {
                shown_camera = used;
            }
            edit.camera = used as u16;
            ui.end_row();
            ui.label(widget::hint("Values of")).on_hover_text("An area has 16 cameras. Scripts and places of a map can ask for another one.");
            menu(ui, "zone_camera_shown", &mut shown_camera);
            ui.end_row();
        });
        ui.add_space(8.0);
        let shown = shown_camera.min(CAMERA_COUNT - 1);
        let (Some(camera), Some(game_camera)) = (edit.cameras.get_mut(shown), original.get(shown)) else { return };
        grid(ui, "zone_camera_values", |ui| {
            for value in &CAMERA_VALUES {
                let changed = camera.value(value.at) != game_camera.value(value.at);
                let back = ui
                    .horizontal(|ui| {
                        let (name, back) = widget::field_name(ui, value.name, changed);
                        name.on_hover_text(value.note);
                        back
                    })
                    .inner;
                let mut number = camera.value(value.at);
                if ui.add(egui::DragValue::new(&mut number).speed(value.speed).max_decimals(2)).on_hover_text(value.note).changed() {
                    camera.set_value(value.at, number);
                }
                if back {
                    camera.set_value(value.at, game_camera.value(value.at));
                }
                ui.end_row();
            }
            let changed = camera.blend_frames() != game_camera.blend_frames();
            let back = ui
                .horizontal(|ui| {
                    let (name, back) = widget::field_name(ui, "Move time", changed);
                    name.on_hover_text("The number of frames that the game takes to move to this camera.");
                    back
                })
                .inner;
            let mut frames = camera.blend_frames();
            widget::spin(ui, &mut frames, 0.0, 600.0);
            camera.set_blend_frames(if back { game_camera.blend_frames() } else { frames });
            ui.end_row();
        });
        self.camera_shown = shown_camera;
    }

    fn advanced_tab(&self, ui: &mut egui::Ui, edit: &mut Edit, game: &Edit) {
        ui.label(egui::RichText::new("These values choose the files of the map. A wrong value can stop the game.").color(color::AMBER));
        let areas = self.store.as_ref().map_or(0, Store::area_count) as i64;
        let (settings, game) = (&mut edit.settings, &game.settings);
        grid(ui, "zone_advanced", |ui| {
            number_row(ui, "Area", "The textures, the objects, the lights and the cameras.", settings, game, data::AREA);
            number_row(ui, "Matrix", "The blocks of ground of the map.", settings, game, data::MATRIX);
            number_row(ui, "Text file of the scripts", "", settings, game, data::SCRIPT_TEXT);
            number_row(ui, "Event file", "", settings, game, data::EVENT_FILE);
            number_row(ui, "Town Map group", "The first map of the group of maps that share a point on the Town Map. A group with a higher number than the map stops the game.", settings, game, data::GROUP);
            number_row(ui, "Special sequence", "A sequence of the field for some maps. 65535: none.", settings, game, data::UNIQUE_SEQUENCE);
            number_row(ui, "Pokémon memory", "The place that a Pokémon remembers when it is met or does something here.", settings, game, data::MEMORY);
            number_row(ui, "Resource kind", "", settings, game, data::RESOURCE_KIND);
            number_row(ui, "Character model set", "", settings, game, data::MODEL_SET);
        });
        if areas > 0 && settings.get(data::AREA) >= areas {
            settings.set(data::AREA, areas - 1);
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let (Some(mut edit), Some(game)) = (self.edit.take(), self.game.clone()) else { return };
        ui.horizontal(|ui| {
            for (place, name) in TABS.iter().enumerate() {
                if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                    self.tab = place;
                }
            }
        });
        ui.add_space(6.0);
        let count = self.store.as_ref().map_or(0, Store::zone_count);
        egui::ScrollArea::vertical().id_salt("zone_tab").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| match self.tab {
                0 => self.place_tab(ui, &mut edit, &game),
                1 => ZonePage::rules_tab(ui, &mut edit, &game),
                2 => ZonePage::look_tab(ui, &mut edit, &game),
                3 => ZonePage::positions_tab(ui, &mut edit, &game, count),
                4 => self.camera_tab(ui, &mut edit, &game),
                _ => self.advanced_tab(ui, &mut edit, &game),
            });
        });
        // Another area: its cameras come with it.
        let area = edit.settings.get(data::AREA) as usize;
        if area != edit.area {
            if let Some(Ok((used, cameras))) = self.store.as_ref().map(|store| store.cameras(sd, area)) {
                (edit.area, edit.camera, edit.cameras) = (area, used, cameras);
            }
        }
        data::add_names(&self.name_files, &mut edit);
        self.edit = Some(edit);
    }
}

impl Module for ZonePage {
    fn title(&self) -> &str {
        "Map Data"
    }

    fn icon(&self) -> &str {
        "🗺"
    }

    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn discard_changes(&mut self) -> bool {
        self.edit = self.saved.clone();
        self.history.clear();
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Map Data");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let sd = session.sd.clone();
        let now = (dump.root().to_path_buf(), sd.as_ref().map(|sd| sd.root().to_path_buf()).unwrap_or_default());
        if self.opened_for.as_ref() != Some(&now) {
            self.opened_for = Some(now);
            self.open(dump, sd.as_ref());
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        self.top_bar(ui, sd.as_ref());
        self.windows(ui.ctx(), sd.as_ref());
        ui.add_space(8.0);
        self.tabs(ui, sd.as_ref());
        // Ctrl+Z, Ctrl+Y and the buttons on the open map. A drag is one step.
        if !ui.input(|i| i.pointer.any_down()) {
            self.history.track(ui, self.zone as u64, &mut self.edit);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: every tab of some maps draws without a change of the map, and a change is undone with the keys.
    #[test]
    fn the_page_draws_every_tab_and_undoes_a_change() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_zone_page_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let card = sd.clone();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = ZonePage::default();
        let ctx = egui::Context::default();
        let mut frame = |page: &mut ZonePage, input: egui::RawInput| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..input };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        };
        frame(&mut page, Default::default());
        for zone in [0, 6, 100, 300, 535] {
            let sd = None;
            page.load(sd, zone);
            for tab in 0..TABS.len() {
                page.tab = tab;
                frame(&mut page, Default::default());
                frame(&mut page, Default::default());
                assert!(page.edit == page.saved, "map {zone}, tab {tab}: the page changed the map by itself");
            }
        }
        page.load(None, 6);
        page.tab = 1;
        frame(&mut page, Default::default());
        page.edit.as_mut().unwrap().settings.set(data::FLY, 0);
        frame(&mut page, Default::default());
        assert!(page.unsaved());
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert!(!page.unsaved(), "undone");
        // A new name is a change; after the save it is not, and it can be undone.
        page.tab = 0;
        frame(&mut page, Default::default());
        let line = page.edit.as_ref().unwrap().settings.get(data::PLACE_NAME) as usize;
        let old = page.edit.as_ref().unwrap().names[&(ENGLISH, line)].clone();
        page.edit.as_mut().unwrap().names.insert((ENGLISH, line), "Test Town".to_string());
        frame(&mut page, Default::default());
        assert!(page.unsaved());
        page.save(&card);
        frame(&mut page, Default::default());
        assert!(!page.unsaved(), "saved");
        assert!(page.places.iter().any(|(at, name)| usize::from(*at) == line && name.starts_with("Test Town")));
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert_eq!(page.edit.as_ref().unwrap().names[&(ENGLISH, line)], old);
        assert!(page.unsaved(), "the card has the new name");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
