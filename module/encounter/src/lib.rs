//! The Encounter page: the wild Pokémon of each map, place by place, and the screen of the DexNav.
//!
//! The user chooses a map in a menu, then a kind of place (the grass, the water, the fishing rods, the hordes...), and gives a Pokémon, a form
//! and two levels to each place of the table. The page shows the chance that the game gives to each place. The table goes in the file of the map
//! on the SD card (`overworld/zone/<map>.bin`, with the events that the Overworld page wrote), and the game reads it instead of its own.
//! The DexNav shows the same table.

pub mod data;
pub mod dexnav;
mod navi_parts;
mod navi_view;

use data::{Encounter, RateKind, Slot, Zones, METHODS};
use sango_overworld::names::MAP_NAMES;
use sango_species_base::{form_key, from_form_key, species_form_name, SpeciesIcons};
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The tabs after the kinds of places.
const SUMMARY: usize = METHODS.len();
const DEXNAV: usize = SUMMARY + 1;

/// How many changes the undo remembers.
const UNDO_LIMIT: usize = 100;

/// The Encounter page.
pub struct EncounterPage {
    zones: Option<Zones>,
    opened_for: Option<(PathBuf, PathBuf)>,
    error: Option<String>,
    /// The name of each map, with a number when two maps have the same name.
    names: Vec<String>,
    /// The maps that have a table in the game.
    with_table: Vec<usize>,
    /// The maps whose table is not the one of the game.
    changed: Vec<usize>,
    zone: usize,
    /// The table of the map that is open, and the table that is saved (on the SD card or in the game).
    table: Option<Encounter>,
    saved: Option<Encounter>,
    from_sd: bool,
    tab: usize,
    undo: Vec<Encounter>,
    status: Option<(bool, String)>,
    icons: SpeciesIcons,
    /// The Pokémon for the button "Fill the table".
    /// The Pokémon of "Fill every place with": a species and a form (see `form_key`).
    fill_species: u16,
    fill_levels: (u8, u8),
    /// The map to open when the user agrees to lose the changes.
    pending: Option<usize>,
    navi: navi_view::NaviView,
}

impl Default for EncounterPage {
    fn default() -> Self {
        EncounterPage {
            zones: None,
            opened_for: None,
            error: None,
            names: Vec::new(),
            with_table: Vec::new(),
            changed: Vec::new(),
            zone: 0,
            table: None,
            saved: None,
            from_sd: false,
            tab: 0,
            undo: Vec::new(),
            status: None,
            icons: SpeciesIcons::default(),
            fill_species: form_key(1, 0),
            fill_levels: (5, 5),
            pending: None,
            navi: navi_view::NaviView::default(),
        }
    }
}

/// The names of the maps: two maps with the same name get a number.
fn map_names(count: usize) -> Vec<String> {
    let name_of = |zone: usize| MAP_NAMES.get(zone).copied().unwrap_or("Map").to_string();
    let mut total: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for zone in 0..count {
        *total.entry(name_of(zone)).or_default() += 1;
    }
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    (0..count)
        .map(|zone| {
            let name = name_of(zone);
            if total[&name] > 1 {
                let nth = seen.entry(name.clone()).or_default();
                *nth += 1;
                format!("{name} ({nth})")
            } else {
                name
            }
        })
        .collect()
}

/// The name of a species and a form in the menus ("#0479 Rotom-Heat").
fn pokemon_label(species: u16, form: u8) -> String {
    match species {
        0 => "No Pokémon".to_string(),
        n => format!("#{:04} {}", n, species_form_name(n as usize, form as usize)),
    }
}

impl EncounterPage {
    fn open(&mut self, session: &Session) {
        self.error = None;
        self.zones = None;
        self.table = None;
        self.saved = None;
        self.undo.clear();
        let (Some(dump), Some(sd)) = (session.dump.as_ref(), session.sd.as_ref()) else {
            self.opened_for = session.dump.as_ref().map(|d| (d.root().to_path_buf(), PathBuf::new()));
            if let Some(dump) = session.dump.as_ref() {
                self.icons.open(dump);
            }
            return;
        };
        self.opened_for = Some((dump.root().to_path_buf(), sd.root().to_path_buf()));
        self.icons.open(dump);
        match Zones::open(dump, &sd.overworld_folder()) {
            Ok(zones) => {
                self.names = map_names(zones.count());
                let (with_table, changed) = zones.with_tables();
                self.with_table = with_table;
                self.changed = changed;
                self.zones = Some(zones);
                let first = self.with_table.first().copied().unwrap_or(0);
                self.load(first);
            }
            Err(message) => self.error = Some(message),
        }
    }

    /// Opens the table of a map.
    fn load(&mut self, zone: usize) {
        let Some(zones) = self.zones.as_ref() else { return };
        self.zone = zone;
        self.undo.clear();
        match zones.pack(zone).and_then(|pack| zones.table(zone).map(|table| (table, pack.from_sd))) {
            Ok((table, from_sd)) => {
                self.table = table.clone();
                self.saved = table;
                self.from_sd = from_sd;
            }
            Err(message) => {
                self.table = None;
                self.saved = None;
                self.status = Some((false, message));
            }
        }
    }

    fn dirty(&self) -> bool {
        self.table != self.saved
    }

    fn save(&mut self, session: &Session) {
        let (Some(zones), Some(table)) = (self.zones.as_ref(), self.table.as_ref()) else { return };
        match zones.write(self.zone, table) {
            Ok(size) => {
                if let Some(sd) = session.sd.as_ref() {
                    let _ = sd.mark_changed(&format!("overworld/zone/{}.bin", self.zone));
                }
                self.saved = self.table.clone();
                self.from_sd = true;
                self.refresh_changed();
                self.status = Some((true, format!("Saved ({size} bytes). Restart the game, or leave and enter the map, to see the new Pokémon.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn reset(&mut self, session: &Session) {
        let Some(zones) = self.zones.as_ref() else { return };
        match zones.reset(self.zone) {
            Ok(()) => {
                if let Some(sd) = session.sd.as_ref() {
                    let _ = sd.mark_changed(&format!("overworld/zone/{}.bin", self.zone));
                }
                let zone = self.zone;
                self.refresh_changed();
                self.load(zone);
                self.status = Some((true, "The wild Pokémon of the game are back.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn refresh_changed(&mut self) {
        if let Some(zones) = self.zones.as_ref() {
            self.changed = zones.with_tables().1;
        }
    }

    /// Changes the table of the open map through `edit`, and remembers the old one for the undo.
    fn change(&mut self, edit: impl FnOnce(&mut Encounter)) {
        let Some(table) = self.table.as_mut() else { return };
        let before = table.clone();
        edit(table);
        if *table != before {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let dirty = self.dirty();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Map"));
                let shown = self.names.get(self.zone).cloned().unwrap_or_else(|| "(none)".to_string());
                let mut value = self.zone as u16;
                let with_table = self.with_table.clone();
                let changed = self.changed.clone();
                let names = self.names.clone();
                let picked = picker(ui, "encounter_map", &shown, 360.0, &mut value, None, || {
                    let entry = |zone: usize| (zone as u16, names[zone].clone());
                    let mut groups = Vec::new();
                    if !changed.is_empty() {
                        groups.push(Group { title: "Changed by you".to_string(), entries: changed.iter().map(|&z| entry(z)).collect() });
                    }
                    groups.push(Group { title: "With wild Pokémon".to_string(), entries: with_table.iter().map(|&z| entry(z)).collect() });
                    let others: Vec<(u16, String)> = (0..names.len()).filter(|z| !with_table.contains(z)).map(entry).collect();
                    groups.push(Group { title: "Without wild Pokémon in the game".to_string(), entries: others });
                    groups
                });
                if picked && value as usize != self.zone && (value as usize) < self.names.len() {
                    if dirty {
                        self.pending = Some(value as usize);
                    } else {
                        self.load(value as usize);
                    }
                }
                if let Some(sd) = session.sd.as_ref() {
                    ui.add_enabled_ui(dirty, |ui| {
                        if widget::save_button(ui, self.unsaved()) {
                            let _ = sd;
                            self.save(session);
                        }
                    })
                    .response
                    .on_disabled_hover_text("Nothing to save: the table is the one that was loaded or saved.");
                    ui.add_enabled_ui(!self.undo.is_empty(), |ui| {
                        if widget::button(ui, "Undo") {
                            if let Some(before) = self.undo.pop() {
                                self.table = Some(before);
                            }
                        }
                    });
                    if self.from_sd && widget::confirm_button(ui, "Reset to the game", "Click again to reset") {
                        self.reset(session);
                    }
                }
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if session.sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can read the tables, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
    }

    /// The question that comes when the user opens another map with changes that are not saved.
    fn pending_dialog(&mut self, ctx: &egui::Context, session: &Session) {
        let Some(target) = self.pending else { return };
        let mut done = false;
        egui::Window::new("Changes that are not saved").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label("The table of this map has changes that are not saved.");
            ui.horizontal(|ui| {
                if widget::button_styled(ui, egui::RichText::new("Save and go").color(color::WHITE), Some(color::ACCENT)) && session.sd.is_some() {
                    self.save(session);
                    self.load(target);
                    done = true;
                }
                if widget::button(ui, "Go without saving") {
                    self.load(target);
                    done = true;
                }
                if widget::button(ui, "Stay here") {
                    done = true;
                }
            });
        });
        if done {
            self.pending = None;
        }
    }

    fn kind_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (place, method) in METHODS.iter().enumerate() {
                let active = self.table.as_ref().is_some_and(|t| t.rates[method.rate_at] > 0);
                if widget::tab_button(ui, method.name, self.tab == place, active).clicked() {
                    self.tab = place;
                }
            }
            if widget::tab_button(ui, "Summary", self.tab == SUMMARY, false).clicked() {
                self.tab = SUMMARY;
            }
            if widget::tab_button(ui, "DexNav", self.tab == DEXNAV, false).clicked() {
                self.tab = DEXNAV;
            }
        });
    }

    /// The rate of a kind of place: the chance of a meeting, or the table that the game uses.
    fn rate_row(&mut self, ui: &mut egui::Ui, method: &data::Method) {
        let Some(rate) = self.table.as_ref().map(|t| t.rates[method.rate_at]) else { return };
        let mut value = rate;
        ui.horizontal(|ui| match method.rate_kind() {
            RateKind::Chance => {
                ui.label(widget::hint("Chance of a meeting at each step (0 for no wild Pokémon here)"));
                widget::spin(ui, &mut value, 0.0, 100.0);
                if value > 0 {
                    ui.label(widget::hint("%"));
                }
            }
            RateKind::Table(names) => {
                ui.label(widget::hint(if method.rate_at == 2 { "The game adds these places to" } else { "The rocks give" }));
                let mut choice = value as u16;
                let shown = if value == 0 { "Nothing".to_string() } else { names.get(value as usize - 1).copied().unwrap_or("?").to_string() };
                picker(ui, &format!("encounter_rate_{}", method.rate_at), &shown, 160.0, &mut choice, Some("Nothing"), || {
                    vec![Group { title: "Choice".to_string(), entries: names.iter().enumerate().map(|(i, n)| (i as u16 + 1, n.to_string())).collect() }]
                });
                value = choice.min(names.len() as u16) as u8;
            }
        });
        if value != rate {
            self.change(|t| t.rates[method.rate_at] = value);
        }
    }

    fn method_tab(&mut self, ui: &mut egui::Ui, place: usize) {
        let method = METHODS[place];
        ui.label(widget::hint(method.help));
        ui.add_space(4.0);
        self.rate_row(ui, &method);
        ui.add_space(6.0);
        let Some(table) = self.table.as_ref() else { return };
        if table.rates[method.rate_at] == 0 && table.places(&method).iter().all(|s| s.species == 0) {
            ui.label(widget::hint("The game has no wild Pokémon here. Give a rate above 0 and a Pokémon to the places to add some."));
        }
        // The places, with the chance of each one.
        let mut edits: Vec<(usize, Slot)> = Vec::new();
        let slots: Vec<Slot> = table.places(&method).to_vec();
        for (at, slot) in slots.iter().enumerate() {
            if method.group != method.count && at % method.group == 0 {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("Horde {} — chance {} %", at / method.group + 1, method.chance(at / method.group))).strong());
            }
            let mut edited = *slot;
            ui.horizontal(|ui| {
                let chance = if method.group == method.count { format!("{:>3} %", method.chance(at)) } else { "     ".to_string() };
                ui.add_sized([46.0, 22.0], egui::Label::new(egui::RichText::new(chance).monospace()));
                match self.icons.get(ui.ctx(), edited.species as usize) {
                    Some(texture) if edited.species != 0 => {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(40.0, 30.0)));
                    }
                    _ => {
                        ui.add_space(44.0);
                    }
                }
                // One menu for the species and the form: each form is a Pokémon of its own ("Rotom-Heat").
                let mut value = (edited.species as usize, edited.form as usize);
                if sango_species::menus::pokemon_menu(ui, &format!("encounter_species_{place}_{at}"), &mut value, 260.0, None, Some("No Pokémon")) {
                    let (species, form) = value;
                    if species != edited.species as usize && species != 0 && edited.min_level == 0 {
                        edited.min_level = 5;
                        edited.max_level = 5;
                    }
                    edited.species = species as u16;
                    edited.form = form as u8;
                }
                if edited.species != 0 {
                    ui.label(widget::hint("Level"));
                    widget::spin(ui, &mut edited.min_level, 1.0, 100.0);
                    ui.label(widget::hint("to"));
                    widget::spin(ui, &mut edited.max_level, 1.0, 100.0);
                    if edited.max_level < edited.min_level {
                        if edited.min_level != slot.min_level {
                            edited.max_level = edited.min_level;
                        } else {
                            edited.min_level = edited.max_level;
                        }
                    }
                }
            });
            if edited != *slot {
                edits.push((at, edited));
            }
        }
        if !edits.is_empty() {
            self.change(|t| {
                for (at, slot) in edits {
                    t.places_mut(&method)[at] = slot;
                }
            });
        }
        ui.add_space(8.0);
        self.fill_row(ui, place);
    }

    /// Gives one Pokémon to all the places of a kind, or empties them.
    fn fill_row(&mut self, ui: &mut egui::Ui, place: usize) {
        let method = METHODS[place];
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(widget::hint("Fill every place with"));
            let mut fill = from_form_key(self.fill_species);
            if sango_species::menus::pokemon_menu(ui, &format!("encounter_fill_{place}"), &mut fill, 260.0, None, None) {
                self.fill_species = form_key(fill.0, fill.1);
            }
            ui.label(widget::hint("level"));
            widget::spin(ui, &mut self.fill_levels.0, 1.0, 100.0);
            ui.label(widget::hint("to"));
            widget::spin(ui, &mut self.fill_levels.1, 1.0, 100.0);
            if self.fill_levels.1 < self.fill_levels.0 {
                self.fill_levels.1 = self.fill_levels.0;
            }
            if widget::button(ui, "Fill") {
                let ((species, form), (low, high)) = (from_form_key(self.fill_species), self.fill_levels);
                self.change(|t| {
                    for slot in t.places_mut(&method) {
                        *slot = Slot { species: species as u16, form: form as _, min_level: low, max_level: high };
                    }
                });
            }
            if widget::confirm_button(ui, "Empty the places", "Click again to empty") {
                self.change(|t| {
                    for slot in t.places_mut(&method) {
                        *slot = Slot::default();
                    }
                    t.rates[method.rate_at] = 0;
                });
            }
        });
    }

    /// All the Pokémon of the map, for each kind of place, with their total chance.
    fn summary_tab(&mut self, ui: &mut egui::Ui) {
        let Some(table) = self.table.as_ref() else { return };
        ui.label(widget::hint("The Pokémon that the player can meet on this map. The chance is the chance among the Pokémon of the same kind of place."));
        for method in &METHODS {
            let found = table.species_of(method);
            if found.is_empty() {
                continue;
            }
            ui.add_space(6.0);
            let rate = table.rates[method.rate_at];
            let active = match method.rate_kind() {
                RateKind::Chance => format!("{rate} % at each step"),
                RateKind::Table(names) => if rate == 0 { "off".to_string() } else { names.get(rate as usize - 1).copied().unwrap_or("?").to_string() },
            };
            ui.label(egui::RichText::new(format!("{} ({active})", method.name)).strong());
            for (species, form, chance) in found {
                ui.label(format!("   {}: {chance} %", pokemon_label(species, form as u8)));
            }
        }
    }
}

impl Module for EncounterPage {
    fn unsaved(&self) -> bool {
        self.dirty() || self.navi.unsaved()
    }

    fn title(&self) -> &str {
        "Encounter"
    }

    fn icon(&self) -> &str {
        "🌿"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Encounter");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let now = (dump.root().to_path_buf(), session.sd.as_ref().map(|sd| sd.root().to_path_buf()).unwrap_or_default());
        if self.opened_for.as_ref() != Some(&now) {
            self.open(session);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        if self.zones.is_none() {
            ui.label(widget::hint("Choose the SD card on the Home page to open the wild Pokémon."));
            return;
        }
        self.tool_bar(ui, session);
        self.pending_dialog(ui.ctx(), session);
        ui.add_space(8.0);
        if self.tab == DEXNAV {
            self.kind_tabs(ui);
            ui.add_space(6.0);
            let (zone, table) = (self.zone, self.table.clone());
            let (navi, icons) = (&mut self.navi, &mut self.icons);
            egui::ScrollArea::vertical().id_salt("encounter_navi_scroll").auto_shrink([false, false]).show(ui, |ui| navi.ui(ui, session, zone, table.as_ref(), icons));
            return;
        }
        if self.table.is_none() {
            widget::card(ui, |ui| {
                ui.label("The game has no wild Pokémon on this map.");
                if session.sd.is_some() && widget::button(ui, "Add a table of wild Pokémon") {
                    self.table = Some(Encounter::default());
                }
            });
            return;
        }
        self.kind_tabs(ui);
        if self.tab == DEXNAV {
            // The tab was just chosen: draw the DexNav on the next frame.
            ui.ctx().request_repaint();
            return;
        }
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt("encounter_scroll").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                if self.tab == SUMMARY {
                    self.summary_tab(ui);
                } else {
                    self.method_tab(ui, self.tab);
                }
            });
        });
    }
}
