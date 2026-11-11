//! The Evolution tab: what a species or a form evolves into, up to 8 evolutions.
//!
//! Each row has the species that it evolves into, the method (level up, an item, a trade...) and what the
//! method needs (a level, an item, a move...). The user can change every part, add an evolution and delete one.
//! The file of the evolutions goes on the SD card (see `sango_core::evolution` for its path and layout).

use crate::learnset::{choose, trash_button, Choice};
use crate::move_names::MOVE;
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

/// The state of the Evolution tab.
#[derive(Default)]
pub struct EvolutionTab {
    archive: Option<Garc>,
    evolutions: Evolutions,
    /// The evolutions as they were read or saved: to know what changed.
    saved: Evolutions,
    /// True when the file comes from the SD card.
    from_card: bool,
    icon_garc: Option<Garc>,
    icon_textures: HashMap<usize, Option<egui::TextureHandle>>,
}

impl EvolutionTab {
    /// Opens the archives of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archive = Garc::open(&dump.archive(evolution::ARCHIVE)).ok();
        self.icon_garc = Garc::open(&dump.archive(crate::icons::ICON_ARCHIVE)).ok();
        self.icon_textures.clear();
    }

    fn read_romfs(&self, file: usize) -> Option<Evolutions> {
        self.archive.as_ref()?.first_file(file).and_then(|bytes| Evolutions::from_bytes(bytes).ok())
    }

    /// Reads the evolutions of a species and a form: the SD card first, then the romfs.
    /// `file` is the place of the form in the archive.
    pub fn load(&mut self, sd: Option<&SdCard>, species: usize, form: usize, file: usize) {
        let on_card = sd.and_then(|sd| sd.read(&evolution::sd_path(species, form))).and_then(|bytes| Evolutions::from_bytes(&bytes).ok());
        self.from_card = on_card.is_some();
        self.evolutions = on_card.or_else(|| self.read_romfs(file)).unwrap_or_default();
        self.saved = self.evolutions.clone();
    }

    /// Takes the evolutions of the romfs again. Save writes them.
    pub fn reset(&mut self, file: usize) {
        if let Some(evolutions) = self.read_romfs(file) {
            self.evolutions = evolutions;
        }
    }

    /// True when the evolutions changed since they were read or saved.
    pub fn changed(&self) -> bool {
        self.evolutions != self.saved
    }

    /// True when the file comes from the SD card.
    pub fn from_card(&self) -> bool {
        self.from_card
    }

    /// Writes the file when it changed, and tells the game. Returns the path that was written.
    pub fn save(&mut self, sd: &SdCard, species: usize, form: usize) -> Result<Option<String>, String> {
        if !self.changed() {
            return Ok(None);
        }
        let path = evolution::sd_path(species, form);
        sd.write(&path, &self.evolutions.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        self.saved = self.evolutions.clone();
        self.from_card = true;
        Ok(Some(path))
    }

    /// The choices of a list of names that start at the number `first`.
    fn names_choices(list: &[&str], first: usize) -> Vec<Choice> {
        list.iter().enumerate().map(|(place, name)| Choice { label: name.to_string(), kind: None, value: place + first }).collect()
    }

    /// The choices for the species that it evolves into.
    fn species_choices() -> Vec<Choice> {
        Self::names_choices(&names::SPECIES, 1)
    }

    /// Draws the tab.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("evolution_tab").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_width(TABLE_WIDTH);
                self.header(ui);
                self.rows(ui);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let add = ui.add_enabled(self.evolutions.has_room(), egui::Button::new(egui::RichText::new("+ Add an evolution").color(color::WHITE)).fill(color::ACCENT));
                    let choices = if ui.memory(|m| m.any_popup_open()) { Self::species_choices() } else { Vec::new() };
                    if let Some(species) = choose(ui, egui::Id::new("evolution_add"), &add, &choices, None) {
                        self.evolutions.list.push(Evolution { method: 4, argument: default_argument(Argument::Level), species: species as u16 });
                    }
                    ui.label(widget::hint(&format!("{} of {SLOTS} evolutions.", self.evolutions.list.len())));
                });
            });
        });
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
            self.species_cell(ui, row, index, &mut entry, popup_open);
            self.method_cell(ui, row, index, &mut entry, popup_open);
            self.need_cell(ui, row, index, &mut entry, popup_open);
            if trash_button(ui, column(row, 3)) {
                remove = Some(index);
            }
            self.evolutions.list[index] = entry;
        }
        if let Some(index) = remove {
            self.evolutions.list.remove(index);
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
    fn species_cell(&mut self, ui: &mut egui::Ui, row: egui::Rect, index: usize, entry: &mut Evolution, popup_open: bool) {
        let cell = column(row, 0);
        let id = egui::Id::new(("evolution_species", index));
        let response = Self::click_area(ui, cell, id);
        let species = entry.species as usize;
        if let Some(texture) = crate::icon_texture(&mut self.icon_textures, self.icon_garc.as_ref(), ui.ctx(), species) {
            let icon = egui::Rect::from_min_size(egui::pos2(cell.left() + 4.0, cell.center().y - 15.0), egui::vec2(40.0, 30.0));
            let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(texture.id(), icon, full, egui::Color32::WHITE);
        }
        ui.painter().text(egui::pos2(cell.left() + 52.0, cell.center().y), egui::Align2::LEFT_CENTER, format!("#{species:04}"), egui::FontId::proportional(12.0), color::MUTED);
        ui.painter().text(egui::pos2(cell.left() + 104.0, cell.center().y), egui::Align2::LEFT_CENTER, crate::name_from(&names::SPECIES, species, 1), egui::FontId::proportional(15.0), color::TEXT);
        let choices = if popup_open { Self::species_choices() } else { Vec::new() };
        if let Some(value) = choose(ui, id, &response, &choices, Some(species)) {
            entry.species = value as u16;
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
        let list: Option<(Vec<Choice>, String)> = match kind {
            Argument::Item => Some((if popup_open { Self::names_choices(&names::ITEM, 1) } else { Vec::new() }, crate::name_from(&names::ITEM, entry.argument as usize, 1))),
            Argument::Move => Some((if popup_open { Self::names_choices(&MOVE, 1) } else { Vec::new() }, crate::name_from(&MOVE, entry.argument as usize, 1))),
            Argument::Species => Some((if popup_open { Self::species_choices() } else { Vec::new() }, crate::name_from(&names::SPECIES, entry.argument as usize, 1))),
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
        assert_eq!(path.as_deref(), Some("pokemon/evolution/133_0.bin"));
        assert!(!tab.changed() && tab.from_card());
        assert_eq!(tab.save(&sd, 133, 0).unwrap(), None);
        let mut again = EvolutionTab::default();
        again.open(&Dump::open(&root).unwrap());
        again.load(Some(&sd), 133, 0, 133);
        assert_eq!(again.evolutions, tab.evolutions);
        assert_eq!(again.evolutions.list.len(), 7);

        // Reset gives back the romfs; a species that does not evolve draws too.
        again.reset(133);
        assert_eq!(again.evolutions.list.len(), 8);
        assert!(again.changed());
        again.load(None, 3, 0, 3);
        assert!(again.evolutions.list.is_empty());
        frame(&mut again);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
