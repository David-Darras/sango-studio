//! The Evolution tab: what a species or a form evolves into, up to 8 evolutions.
//!
//! Each row has the species that it evolves into, the method (level up, an item, a trade...) and what the
//! method needs (a level, an item, a move...). The user can change every part, add an evolution and delete one.
//! The file of the evolutions goes on the SD card (see `sango_core::evolution` for its path and layout).
//!
//! The game keeps the form of the Pokémon when it evolves. The target of an evolution can be any form of the species ("Rotom-Heat"):
//! the form goes in `species/evolution_form/<species>_<form>.bin`, one byte for each place of the evolution table (0xFF keeps the form
//! of the game). The plugin gives this form to the Pokémon after the evolution.

use crate::evolution_csv;
use crate::learnset::{choose, trash_button, Choice};
use crate::names;
use sango_core::evolution::{self, argument_of, method_name, Argument, Evolution, Evolutions, METHODS, SLOTS};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::HashMap;

/// The places of the columns of a row: x and width, in points. The species, the method, the need, the delete button.
const COLUMNS: [(f32, f32); 4] = [(0.0, 250.0), (260.0, 300.0), (570.0, 190.0), (770.0, 28.0)];

/// The width of the table, in points.
const TABLE_WIDTH: f32 = 860.0;

/// The height of a row, in points.
const ROW_HEIGHT: f32 = 48.0;

/// The rectangle of a column inside a row.
fn column(row: egui::Rect, place: usize) -> egui::Rect {
    let (x, width) = COLUMNS[place];
    egui::Rect::from_min_size(egui::pos2(row.left() + x, row.top()), egui::vec2(width, row.height()))
}

/// What a new argument is when the method changes to one with another kind of argument.
fn default_argument(kind: Argument) -> u16 {
    match kind {
        Argument::None => 0,
        Argument::Level => 30,
        Argument::Item => 82, // Fire Stone
        Argument::Move => 1,
        Argument::Species => 1,
        Argument::Type => 0,
        Argument::Beauty => 170,
    }
}

/// Draws a menu in a cell of a row, centred in its height. Gives what the menu gives.
fn menu_in(ui: &mut egui::Ui, cell: egui::Rect, add: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    let area = egui::Rect::from_center_size(cell.center(), egui::vec2(cell.width() - 4.0, 36.0));
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(area), add).inner
}

/// The value of a place of the file of the target forms that keeps the form of the game.
const KEEP_FORM: u8 = 0xFF;

/// The path on the SD card of the target forms of the evolutions of a species and a form.
pub fn form_path(species: usize, form: usize) -> String {
    format!("species/evolution_form/{species}_{form}.bin")
}

/// The state of the Evolution tab.
#[derive(Default)]
pub struct EvolutionTab {
    /// The form that each place of the table gives (KEEP_FORM: the form stays), one for each place.
    forms: Vec<u8>,
    saved_forms: Vec<u8>,
    /// The species and the form that are open.
    species: usize,
    form: usize,
    sd: Option<SdCard>,
    form_icons: HashMap<(usize, usize), Option<egui::TextureHandle>>,
    archive: Option<Garc>,
    species_archive: Option<Garc>,
    evolutions: Evolutions,
    /// The evolutions as they were read or saved: to know what changed.
    saved: Evolutions,
    /// True when the file comes from the SD card.
    from_card: bool,
    /// The place of the form in the archive: the number of the file that the plugin replaces.
    file: usize,
    icon_garc: Option<Garc>,
}

impl EvolutionTab {
    /// Opens the archives of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archive = Garc::open(&dump.archive(evolution::ARCHIVE)).ok();
        self.species_archive = Garc::open(&dump.archive(crate::ARCHIVE)).ok();
        self.icon_garc = Garc::open(&dump.archive(crate::icons::ICON_ARCHIVE)).ok();
        self.form_icons.clear();
    }

    /// The number of evolutions that the tab shows (for the tests).
    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.evolutions.list.len()
    }

    /// Takes the evolutions that a pack gives to a species of the pack (method, argument and species).
    pub fn load_pack(&mut self, evolutions: &[(u8, u16, u16)]) {
        self.evolutions = Evolutions { list: evolutions.iter().map(|&(method, argument, species)| Evolution { method: method as u16, argument, species }).collect() };
        self.saved = self.evolutions.clone();
    }

    /// Reads the target forms of the evolutions of the open species and form.
    fn load_forms(&mut self, sd: Option<&SdCard>) {
        let mut forms = sd.and_then(|sd| sd.read(&form_path(self.species, self.form))).unwrap_or_default();
        forms.resize(SLOTS, KEEP_FORM);
        self.forms = forms;
        self.saved_forms = self.forms.clone();
    }

    fn read_romfs(&self, file: usize) -> Option<Evolutions> {
        self.archive.as_ref()?.first_file(file).and_then(|bytes| Evolutions::from_bytes(bytes).ok())
    }

    /// Reads the evolutions of a species and a form: the SD card first, then the romfs.
    /// `file` is the place of the form in the archive.
    pub fn load(&mut self, sd: Option<&SdCard>, species: usize, form: usize, file: usize) {
        self.file = file;
        (self.species, self.form) = (species, form);
        self.sd = sd.cloned();
        self.load_forms(sd);
        let on_card = sd.and_then(|sd| sd.read(&evolution::sd_path(file))).and_then(|bytes| Evolutions::from_bytes(&bytes).ok());
        self.from_card = on_card.is_some();
        self.evolutions = on_card.or_else(|| self.read_romfs(file)).unwrap_or_default();
        self.saved = self.evolutions.clone();
    }

    /// True when the evolutions changed since they were read or saved.
    pub fn changed(&self) -> bool {
        self.evolutions != self.saved || self.forms != self.saved_forms
    }

    /// True when the file comes from the SD card.
    pub fn from_card(&self) -> bool {
        self.from_card
    }

    /// Writes the file when it changed, and tells the game. Returns the path that was written.
    pub fn save(&mut self, sd: &SdCard, _species: usize, _form: usize) -> Result<Option<String>, String> {
        if self.forms != self.saved_forms {
            let path = form_path(self.species, self.form);
            if self.forms.iter().all(|&form| form == KEEP_FORM) {
                sd.remove(&path).and_then(|_| sd.mark_changed(&path))?;
            } else {
                sd.write(&path, &self.forms).and_then(|_| sd.mark_changed(&path))?;
            }
            self.saved_forms = self.forms.clone();
        }
        if self.evolutions == self.saved {
            return Ok(None);
        }
        let path = evolution::sd_path(self.file);
        sd.write(&path, &self.evolutions.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        self.saved = self.evolutions.clone();
        self.from_card = true;
        Ok(Some(path))
    }

    /// The choices of a list of names that start at the number `first`.
    fn names_choices(list: &[&str], first: usize) -> Vec<Choice> {
        list.iter().enumerate().map(|(place, name)| Choice { label: name.to_string(), kind: None, value: place + first }).collect()
    }



    /// Draws the tab.
    /// The card of the evolutions. The Evolution tab shows it above the card of the Mega Evolutions.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        {
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(TABLE_WIDTH);
                ui.label(widget::title("Evolutions"));
                ui.horizontal(|ui| {
                    // The same menu as everywhere: the Pokémon that it evolves into, with its form.
                    ui.add_enabled_ui(self.evolutions.has_room(), |ui| {
                        let mut chosen = (0, 0);
                        if crate::menus::pokemon_menu(ui, "evolution_add", &mut chosen, 280.0, None, Some("+ Add an evolution")) && chosen.0 != 0 {
                            let place = self.evolutions.list.len();
                            self.evolutions.list.push(Evolution { method: 4, argument: default_argument(Argument::Level), species: chosen.0 as u16 });
                            if let Some(form) = self.forms.get_mut(place) {
                                *form = if chosen.1 == 0 { KEEP_FORM } else { chosen.1 as u8 };
                            }
                        }
                    });
                    if !self.evolutions.list.is_empty() && widget::confirm_button(ui, "Delete all evolutions", "Click again to confirm") {
                        self.evolutions.list.clear();
                    }
                    ui.label(widget::hint(&format!("{} of {SLOTS} evolutions", self.evolutions.list.len())));
                });
                ui.add_space(8.0);
                self.header(ui);
                self.rows(ui);
            });
        }
    }

    /// The archives and the SD card that the CSV functions read.
    fn library<'a>(&'a self, sd: Option<&'a SdCard>) -> Option<evolution_csv::Library<'a>> {
        Some(evolution_csv::Library { species: self.species_archive.as_ref()?, evolutions: self.archive.as_ref()?, sd })
    }

    /// The evolutions of all the species and forms as CSV text (the SD card first, then the romfs).
    pub(crate) fn csv_export(&self, sd: Option<&SdCard>) -> Option<String> {
        Some(evolution_csv::export(&self.library(sd)?))
    }

    /// Reads CSV text, checks it, and writes the changes on the SD card. Gives the number of files that were written.
    pub(crate) fn csv_import(&self, sd: &SdCard, text: &str) -> Result<usize, String> {
        let library = self.library(Some(sd)).ok_or("The archives of the evolutions are not open.")?;
        let rows = evolution_csv::parse(text)?;
        evolution_csv::apply(&rows, &library, sd)
    }

    /// The names of the columns.
    fn header(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, 24.0), egui::Sense::hover());
        for (place, name) in ["Evolves into", "Method", "Needs"].iter().enumerate() {
            let cell = column(rect, place);
            ui.painter().text(egui::pos2(cell.left() + 4.0, cell.center().y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.0), color::MUTED);
        }
        ui.separator();
    }

    /// One row for each evolution.
    fn rows(&mut self, ui: &mut egui::Ui) {
        if self.evolutions.list.is_empty() {
            ui.add_space(6.0);
            ui.label(widget::hint("This Pokémon does not evolve."));
        }
        let popup_open = ui.memory(|m| m.any_popup_open());
        let mut remove = None;
        for index in 0..self.evolutions.list.len() {
            let mut entry = self.evolutions.list[index];
            let (row, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, ROW_HEIGHT), egui::Sense::hover());
            if ui.rect_contains_pointer(row) {
                ui.painter().rect_filled(row, egui::Rounding::same(8.0), color::SURFACE_STRONG.linear_multiply(0.6));
            }
            self.species_cell(ui, row, index, &mut entry);
            self.method_cell(ui, row, index, &mut entry, popup_open);
            self.need_cell(ui, row, index, &mut entry, popup_open);
            if trash_button(ui, column(row, 3)) {
                remove = Some(index);
            }
            self.evolutions.list[index] = entry;
        }
        if let Some(index) = remove {
            self.evolutions.list.remove(index);
            // The forms follow the places of the evolutions.
            if index < self.forms.len() {
                self.forms.remove(index);
                self.forms.push(KEEP_FORM);
            }
        }
    }

    /// A clickable text in a cell. Gives true when it is clicked, and draws the border that shows it can be.
    fn click_area(ui: &mut egui::Ui, cell: egui::Rect, id: egui::Id) -> egui::Response {
        let response = ui.interact(cell.shrink2(egui::vec2(0.0, 7.0)), id, egui::Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(response.rect, egui::Rounding::same(6.0), egui::Stroke::new(1.0_f32, color::BORDER));
        }
        response
    }

    /// The species that it evolves into: its icon, number and name. A click changes it.
    fn species_cell(&mut self, ui: &mut egui::Ui, row: egui::Rect, index: usize, entry: &mut Evolution) {
        let cell = column(row, 0);
        let target = self.forms.get(index).copied().unwrap_or(KEEP_FORM);
        let mut value = (entry.species as usize, if target == KEEP_FORM { 0 } else { target as usize });
        let changed = menu_in(ui, cell, |ui| crate::menus::pokemon_menu(ui, &format!("evolution_target_{index}"), &mut value, cell.width() - 6.0, None, None));
        if changed {
            entry.species = value.0 as u16;
            // The first form is the rule of the game: the Pokémon keeps its form.
            if let Some(place) = self.forms.get_mut(index) {
                *place = if value.1 == 0 { KEEP_FORM } else { value.1 as u8 };
            }
        }
    }

    /// The method. A click changes it, and the argument becomes one that fits.
    fn method_cell(&mut self, ui: &mut egui::Ui, row: egui::Rect, index: usize, entry: &mut Evolution, popup_open: bool) {
        let cell = column(row, 1);
        let id = egui::Id::new(("evolution_method", index));
        let response = Self::click_area(ui, cell, id);
        ui.painter().text(egui::pos2(cell.left() + 8.0, cell.center().y), egui::Align2::LEFT_CENTER, method_name(entry.method), egui::FontId::proportional(14.0), color::TEXT);
        let choices: Vec<Choice> = if popup_open { METHODS.iter().map(|(number, name, _)| Choice { label: name.to_string(), kind: None, value: *number as usize }).collect() } else { Vec::new() };
        if let Some(value) = choose(ui, id, &response, &choices, Some(entry.method as usize)) {
            let method = value as u16;
            if argument_of(method) != argument_of(entry.method) {
                entry.argument = default_argument(argument_of(method));
            }
            entry.method = method;
        }
    }

    /// What the method needs: a level, an item, a move, a species, a type or a Beauty. A click changes it.
    fn need_cell(&mut self, ui: &mut egui::Ui, row: egui::Rect, index: usize, entry: &mut Evolution, popup_open: bool) {
        let cell = column(row, 2);
        let id = egui::Id::new(("evolution_need", index));
        let kind = argument_of(entry.method);
        let text_at = |ui: &egui::Ui, text: &str, ink: egui::Color32| {
            ui.painter().text(egui::pos2(cell.left() + 8.0, cell.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(14.0), ink);
        };
        // An item, a move or a species: the menus that every page uses, with the pictures and the details.
        match kind {
            Argument::Item => {
                let mut value = entry.argument;
                menu_in(ui, cell, |ui| crate::menus::item_menu(ui, &format!("evolution_item_{index}"), &mut value, cell.width() - 6.0, false));
                entry.argument = value;
                return;
            }
            Argument::Move => {
                let mut value = entry.argument;
                menu_in(ui, cell, |ui| crate::menus::move_menu(ui, &format!("evolution_move_{index}"), &mut value, cell.width() - 6.0));
                entry.argument = value;
                return;
            }
            Argument::Species => {
                // The method needs a species (Mantyke with a Remoraid in the party): the first form stands for it.
                let mut value = (entry.argument as usize, 0);
                menu_in(ui, cell, |ui| crate::menus::pokemon_menu(ui, &format!("evolution_need_{index}"), &mut value, cell.width() - 6.0, Some(&|_, form| form == 0), None));
                entry.argument = value.0 as u16;
                return;
            }
            _ => {}
        }
        let list: Option<(Vec<Choice>, String)> = match kind {
            Argument::Type => Some((if popup_open { Self::names_choices(&names::TYPE, 0) } else { Vec::new() }, crate::name_from(&names::TYPE, entry.argument as usize, 0))),
            _ => None,
        };
        match (kind, list) {
            (_, Some((choices, name))) => {
                let response = Self::click_area(ui, cell, id);
                text_at(ui, &name, color::TEXT);
                if let Some(value) = choose(ui, id, &response, &choices, Some(entry.argument as usize)) {
                    entry.argument = value as u16;
                }
            }
            (Argument::Level | Argument::Beauty, None) => {
                let (label, max) = if kind == Argument::Level { ("Level", 100) } else { ("Beauty", 255) };
                ui.painter().text(egui::pos2(cell.left() + 8.0, cell.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), color::MUTED);
                let pill = egui::Rect::from_center_size(egui::pos2(cell.left() + 100.0, cell.center().y), egui::vec2(60.0, 26.0));
                ui.painter().rect_filled(pill, egui::Rounding::same(13.0), color::SURFACE_STRONG);
                let mut value = entry.argument as u32;
                ui.scope(|ui| {
                    let widgets = &mut ui.visuals_mut().widgets;
                    for style in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
                        style.weak_bg_fill = egui::Color32::TRANSPARENT;
                        style.bg_fill = egui::Color32::TRANSPARENT;
                        style.bg_stroke = egui::Stroke::NONE;
                    }
                    ui.put(pill, egui::DragValue::new(&mut value).range(0..=max).speed(0.15));
                });
                entry.argument = value as u16;
            }
            _ => text_at(ui, "—", color::MUTED),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The target form of an evolution goes in its own file, which goes away when every place keeps the form again.
    #[test]
    fn the_target_form_of_an_evolution_is_saved() {
        let folder = std::env::temp_dir().join(format!("sango_evolution_forms_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut tab = EvolutionTab::default();
        tab.load(Some(&sd), 412, 1, 412);
        assert_eq!(tab.forms, vec![KEEP_FORM; SLOTS]);
        tab.evolutions.list = vec![Evolution { method: 4, argument: 20, species: 413 }];
        tab.forms[0] = 2;
        assert!(tab.changed());
        tab.save(&sd, 412, 1).unwrap();
        assert_eq!(sd.read(&form_path(412, 1)).unwrap(), [2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let mut again = EvolutionTab::default();
        again.load(Some(&sd), 412, 1, 412);
        assert_eq!(again.forms[0], 2);
        again.forms[0] = KEEP_FORM;
        again.save(&sd, 412, 1).unwrap();
        assert!(sd.read(&form_path(412, 1)).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_new_argument_fits_its_method() {
        assert_eq!(default_argument(Argument::None), 0);
        assert_eq!(default_argument(Argument::Level), 30);
        // The default item is the Fire Stone, as the item list of the game says.
        assert_eq!(names::ITEM[default_argument(Argument::Item) as usize - 1], "Fire Stone");
        assert_eq!(names::TYPE[17], "Fairy");
        for (_, _, kind) in METHODS {
            let _ = default_argument(kind);
        }
    }

    /// With the dump of the game: Eevee is read and drawn, changed, saved on the SD card and read again.
    #[test]
    fn the_evolutions_of_eevee_are_read_drawn_changed_and_saved() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_evolution_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut tab = EvolutionTab::default();
        tab.open(&Dump::open(&root).unwrap());
        tab.load(Some(&sd), 133, 0, 133);
        assert_eq!(tab.evolutions.list.len(), 8);
        assert!(!tab.evolutions.has_room() && !tab.changed() && !tab.from_card());

        let ctx = egui::Context::default();
        let frame = |tab: &mut EvolutionTab| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0)));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| tab.ui(ui));
            });
        };
        frame(&mut tab);

        // One evolution is deleted, one is changed to another method: the argument fits the new method.
        tab.evolutions.list.remove(0);
        let sylveon = &mut tab.evolutions.list[4];
        assert_eq!((sylveon.method, sylveon.argument), (29, 17));
        sylveon.method = 4;
        sylveon.argument = default_argument(Argument::Level);
        assert!(tab.changed() && tab.evolutions.has_room());
        frame(&mut tab);

        let path = tab.save(&sd, 133, 0).unwrap();
        assert_eq!(path.as_deref(), Some("species/evolution/133.bin"));
        assert!(!tab.changed() && tab.from_card());
        assert_eq!(tab.save(&sd, 133, 0).unwrap(), None);
        let mut again = EvolutionTab::default();
        again.open(&Dump::open(&root).unwrap());
        again.load(Some(&sd), 133, 0, 133);
        assert_eq!(again.evolutions, tab.evolutions);
        assert_eq!(again.evolutions.list.len(), 7);

        // Without the SD card the evolutions are the ones of the romfs; a species that does not evolve draws too.
        again.load(None, 133, 0, 133);
        assert_eq!(again.evolutions.list.len(), 8);
        assert!(!again.from_card());
        again.load(None, 3, 0, 3);
        assert!(again.evolutions.list.is_empty());
        frame(&mut again);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
