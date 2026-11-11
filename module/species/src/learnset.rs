//! The Learnset tab: the moves of a species or a form, in four groups.
//!
//! | Group | Where it comes from | What the user can do |
//! | --- | --- | --- |
//! | Level up | the archive 191 (see `sango_core::learnset`) | change the level and the move, add, delete |
//! | Egg moves | the archive 190 | change the move, add, delete |
//! | Tutor | the bits of the tutors in the species data | add, delete |
//! | TM/HM | the bits of the TMs and HMs in the species data | add, delete |
//!
//! Every row has the columns of the move tables of Bulbapedia: Level, Move, Type, Category, Power, Accuracy
//! and PP. The level-up moves and the egg moves have their own files on the SD card (see `sango_core::learnset`
//! for the paths and the layout). The TMs and the tutors are bits of the species data file.

use crate::data::SpeciesData;
use crate::learnset_csv;
use crate::machines::{machine_name, BP_TUTORS, MACHINES};
use crate::move_names::MOVE;
use crate::names;
use sango_core::learnset::{self, EggMoves, LevelUp, LevelUpMove, MAX_LEVEL_UP_MOVES};
use sango_core::moves::{Category, MoveTable};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};

/// The groups of moves, with the name of their button.
const GROUPS: [&str; 4] = ["Level up", "Egg moves", "Tutor", "TM/HM"];

const LEVEL_UP: usize = 0;
const EGG: usize = 1;
const TUTOR: usize = 2;

/// The moves of the special tutors, in the order of the bits of `special_tutor_moves`.
const SPECIAL_TUTORS: [&str; 7] = ["Grass Pledge", "Fire Pledge", "Water Pledge", "Frenzy Plant", "Blast Burn", "Hydro Cannon", "Draco Meteor"];

/// The places of the columns of a row: x and width, in points. Level (or TM, or tutor), Move, Type,
/// Category, Power, Accuracy, PP, delete button.
const COLUMNS: [(f32, f32); 8] = [(0.0, 76.0), (86.0, 210.0), (304.0, 92.0), (404.0, 96.0), (508.0, 52.0), (568.0, 70.0), (646.0, 44.0), (700.0, 28.0)];

/// The width of a table, in points.
const TABLE_WIDTH: f32 = 736.0;

/// The height of a row, in points.
const ROW_HEIGHT: f32 = 40.0;

/// The height of a row of the list of choices, in points.
const CHOICE_HEIGHT: f32 = 28.0;

/// The number of a move from its name. `None` when the name is not in the list.
fn move_id(name: &str) -> Option<u16> {
    MOVE.iter().position(|n| *n == name).map(|place| place as u16 + 1)
}

/// The name of a move.
pub(crate) fn move_name(id: u16) -> &'static str {
    (id as usize).checked_sub(1).and_then(|place| MOVE.get(place)).copied().unwrap_or("-")
}

/// Where a tutor move is in the species data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// A bit of `special_tutor_moves`.
    Special(usize),
    /// A bit of one of the four words of `bp_tutor_moves`: (tutor, bit).
    Bp(usize, usize),
}

impl Slot {
    /// Every tutor move of the game, with its move number.
    pub(crate) fn all() -> Vec<(Slot, u16)> {
        let special = (0..SPECIAL_TUTORS.len()).map(|bit| (Slot::Special(bit), move_id(SPECIAL_TUTORS[bit]).unwrap_or(0)));
        let bp = BP_TUTORS.iter().enumerate().flat_map(|(tutor, moves)| moves.iter().enumerate().map(move |(bit, &id)| (Slot::Bp(tutor, bit), id)));
        special.chain(bp).collect()
    }

    /// The name of the tutor, as the first column shows it.
    fn label(self) -> String {
        match self {
            Slot::Special(_) => "Special".to_string(),
            Slot::Bp(tutor, _) => format!("BP {}", tutor + 1),
        }
    }

    /// True when the species can learn this move.
    pub(crate) fn is_set(self, data: &SpeciesData) -> bool {
        match self {
            Slot::Special(bit) => data.special_tutor_moves >> bit & 1 == 1,
            Slot::Bp(tutor, bit) => data.bp_tutor_moves[tutor] >> bit & 1 == 1,
        }
    }

    /// Lets the species learn this move, or not.
    pub(crate) fn set(self, data: &mut SpeciesData, on: bool) {
        let (word, bit) = match self {
            Slot::Special(bit) => (&mut data.special_tutor_moves, bit),
            Slot::Bp(tutor, bit) => (&mut data.bp_tutor_moves[tutor], bit),
        };
        if on {
            *word |= 1 << bit;
        } else {
            *word &= !(1 << bit);
        }
    }
}

/// True when the species can learn the TM or HM of a bit.
pub(crate) fn machine_is_set(data: &SpeciesData, bit: usize) -> bool {
    data.tm_hm_moves[bit / 32] >> (bit % 32) & 1 == 1
}

/// Lets the species learn the TM or HM of a bit, or not.
pub(crate) fn set_machine(data: &mut SpeciesData, bit: usize, on: bool) {
    if on {
        data.tm_hm_moves[bit / 32] |= 1 << (bit % 32);
    } else {
        data.tm_hm_moves[bit / 32] &= !(1 << (bit % 32));
    }
}

/// The colour of a type, as on Bulbapedia.
fn type_color(kind: u8) -> egui::Color32 {
    const COLORS: [(u8, u8, u8); 18] = [
        (168, 168, 120), // Normal
        (192, 48, 40),   // Fighting
        (168, 144, 240), // Flying
        (160, 64, 160),  // Poison
        (224, 192, 104), // Ground
        (184, 160, 56),  // Rock
        (168, 184, 32),  // Bug
        (112, 88, 152),  // Ghost
        (184, 184, 208), // Steel
        (240, 128, 48),  // Fire
        (104, 144, 240), // Water
        (120, 200, 80),  // Grass
        (248, 208, 48),  // Electric
        (248, 88, 136),  // Psychic
        (152, 216, 216), // Ice
        (112, 56, 248),  // Dragon
        (112, 88, 72),   // Dark
        (238, 153, 172), // Fairy
    ];
    let (r, g, b) = COLORS.get(kind as usize).copied().unwrap_or((120, 120, 120));
    egui::Color32::from_rgb(r, g, b)
}

/// The colour of a category.
fn category_color(category: Category) -> egui::Color32 {
    match category {
        Category::Physical => egui::Color32::from_rgb(214, 90, 40),
        Category::Special => egui::Color32::from_rgb(70, 120, 220),
        Category::Status => egui::Color32::from_rgb(130, 134, 150),
    }
}

/// The rectangle of a column inside a row.
fn column(row: egui::Rect, place: usize) -> egui::Rect {
    let (x, width) = COLUMNS[place];
    egui::Rect::from_min_size(egui::pos2(row.left() + x, row.top()), egui::vec2(width, row.height()))
}

/// A rounded label with a fill. Returns its rectangle.
pub(crate) fn pill(painter: &egui::Painter, left_center: egui::Pos2, text: &str, fill: egui::Color32, ink: egui::Color32, font_size: f32) -> egui::Rect {
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(font_size), ink);
    let rect = egui::Rect::from_min_size(egui::pos2(left_center.x, left_center.y - 11.0), egui::vec2(galley.size().x + 18.0, 22.0));
    painter.rect_filled(rect, egui::Rounding::same(11.0), fill);
    painter.galley(egui::pos2(rect.left() + 9.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    rect
}

/// A type as a coloured label. The text is dark on a light colour.
pub(crate) fn type_pill(painter: &egui::Painter, left_center: egui::Pos2, kind: u8, font_size: f32) -> egui::Rect {
    let fill = type_color(kind);
    let light = fill.r() as u32 * 299 + fill.g() as u32 * 587 + fill.b() as u32 * 114 > 170_000;
    let ink = if light { egui::Color32::from_rgb(40, 40, 50) } else { color::WHITE };
    pill(painter, left_center, names::TYPE.get(kind as usize).copied().unwrap_or("-"), fill, ink, font_size)
}

/// An entry of a list of choices.
pub(crate) struct Choice {
    pub(crate) label: String,
    /// The type of the move, drawn at the right of the label.
    pub(crate) kind: Option<u8>,
    /// What the list gives back when the entry is chosen.
    pub(crate) value: usize,
}

/// A list of choices that opens under `anchor` when it is clicked: a search box, then the entries that match,
/// each with the type of its move. Gives back the value of the chosen entry.
pub(crate) fn choose(ui: &mut egui::Ui, id: egui::Id, anchor: &egui::Response, options: &[Choice], current: Option<usize>) -> Option<usize> {
    if anchor.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    let mut picked = None;
    let search_id = id.with("search");
    let focus_id = id.with("focused");
    let mut search: String = ui.data_mut(|d| d.get_temp(search_id).unwrap_or_default());
    let shown = egui::popup::popup_below_widget(ui, id, anchor, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        const WIDTH: f32 = 320.0;
        ui.set_width(WIDTH);
        let edit = ui.add(egui::TextEdit::singleline(&mut search).hint_text("Search...").desired_width(WIDTH - 12.0));
        if !ui.data_mut(|d| d.get_temp::<bool>(focus_id).unwrap_or(false)) {
            edit.request_focus();
            ui.data_mut(|d| d.insert_temp(focus_id, true));
        }
        ui.add_space(4.0);
        let query = search.trim().to_lowercase();
        let found: Vec<&Choice> = options.iter().filter(|c| query.is_empty() || c.label.to_lowercase().contains(&query)).collect();
        if found.is_empty() {
            ui.label(widget::hint("No match."));
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            picked = Some(found[0].value);
        }
        // The list has its own height: the room that the popup leaves is not enough to show many rows.
        let height = (found.len() as f32 * CHOICE_HEIGHT).min(340.0);
        ui.allocate_ui(egui::vec2(WIDTH, height), |ui| {
            egui::ScrollArea::vertical().id_salt(id.with("list")).auto_shrink([false, false]).show_rows(ui, CHOICE_HEIGHT, found.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for choice in &found[range] {
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(WIDTH - 14.0, CHOICE_HEIGHT), egui::Sense::click());
                    if Some(choice.value) == current {
                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::ACCENT_SOFT);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    ui.painter().text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, &choice.label, egui::FontId::proportional(13.5), color::TEXT);
                    if let Some(kind) = choice.kind {
                        let width = ui.painter().layout_no_wrap(names::TYPE.get(kind as usize).copied().unwrap_or("-").to_string(), egui::FontId::proportional(11.5), color::WHITE).size().x + 18.0;
                        type_pill(ui.painter(), egui::pos2(rect.right() - width - 6.0, rect.center().y), kind, 11.5);
                    }
                    if response.clicked() {
                        picked = Some(choice.value);
                    }
                }
            });
        });
    });
    if picked.is_some() {
        ui.memory_mut(|m| m.close_popup());
    }
    if shown.is_some() && picked.is_none() {
        ui.data_mut(|d| d.insert_temp(search_id, search));
    } else {
        ui.data_mut(|d| {
            d.remove::<String>(search_id);
            d.remove::<bool>(focus_id);
        });
    }
    picked
}

/// The state of the Learnset tab.
#[derive(Default)]
pub struct Learnset {
    /// The data of the moves.
    moves: Option<MoveTable>,
    level_up_archive: Option<Garc>,
    egg_archive: Option<Garc>,
    species_archive: Option<Garc>,
    level_up: LevelUp,
    /// The level-up moves as they were read or saved: to know what changed.
    level_up_saved: LevelUp,
    eggs: EggMoves,
    eggs_saved: EggMoves,
    /// True when the files come from the SD card.
    from_card: bool,
    /// The chosen group.
    group: usize,
    /// True when a level changed: the moves go back in the order of the levels when the user lets go.
    needs_sort: bool,
    /// What happened at the last click on a CSV button: true for a success.
    status: Option<(bool, String)>,
    /// True when an import wrote files: the species page reads its data again.
    imported: bool,
}

impl Learnset {
    /// Opens the archives of a dump. The moves of the species are read when the species is loaded.
    pub fn open(&mut self, dump: &Dump) {
        self.moves = Garc::open(&dump.archive(sango_core::moves::ARCHIVE)).and_then(|garc| MoveTable::from_garc(&garc)).ok();
        self.level_up_archive = Garc::open(&dump.archive(learnset::LEVEL_UP_ARCHIVE)).ok();
        self.egg_archive = Garc::open(&dump.archive(learnset::EGG_ARCHIVE)).ok();
        self.species_archive = Garc::open(&dump.archive(crate::ARCHIVE)).ok();
    }

    /// Reads the moves of a species and a form: the SD card first, then the romfs.
    /// `file` is the place of the form in the archive of the level-up moves.
    pub fn load(&mut self, sd: Option<&SdCard>, species: usize, form: usize, file: usize) {
        let on_card = |path: String| sd.and_then(|sd| sd.read(&path));
        let level_card = on_card(learnset::level_up_sd_path(species, form)).and_then(|b| LevelUp::from_bytes(&b).ok());
        let egg_card = on_card(learnset::egg_sd_path(species)).and_then(|b| EggMoves::from_bytes(&b).ok());
        self.from_card = level_card.is_some() || egg_card.is_some();
        let (level_up, eggs) = (self.read_romfs_level_up(file), self.read_romfs_eggs(species));
        self.level_up = level_card.or(level_up).unwrap_or_default();
        self.eggs = egg_card.or(eggs).unwrap_or_default();
        self.level_up_saved = self.level_up.clone();
        self.eggs_saved = self.eggs.clone();
        self.needs_sort = false;
    }

    fn read_romfs_level_up(&self, file: usize) -> Option<LevelUp> {
        self.level_up_archive.as_ref()?.first_file(file).and_then(|b| LevelUp::from_bytes(b).ok())
    }

    fn read_romfs_eggs(&self, species: usize) -> Option<EggMoves> {
        self.egg_archive.as_ref()?.first_file(species).and_then(|b| EggMoves::from_bytes(b).ok())
    }

    /// Takes the moves of the romfs again. Save writes them.
    pub fn reset(&mut self, species: usize, file: usize) {
        if let Some(level_up) = self.read_romfs_level_up(file) {
            self.level_up = level_up;
        }
        if let Some(eggs) = self.read_romfs_eggs(species) {
            self.eggs = eggs;
        }
    }

    /// True when the level-up moves or the egg moves changed since they were read or saved.
    pub fn changed(&self) -> bool {
        self.level_up != self.level_up_saved || self.eggs != self.eggs_saved
    }

    /// True when the files of the moves come from the SD card.
    pub fn from_card(&self) -> bool {
        self.from_card
    }

    /// Writes the files that changed on the SD card, and tells the game. Returns the paths that were written.
    pub fn save(&mut self, sd: &SdCard, species: usize, form: usize) -> Result<Vec<String>, String> {
        let mut written = Vec::new();
        if self.level_up != self.level_up_saved {
            let path = learnset::level_up_sd_path(species, form);
            sd.write(&path, &self.level_up.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            self.level_up_saved = self.level_up.clone();
            written.push(path);
        }
        if self.eggs != self.eggs_saved {
            let path = learnset::egg_sd_path(species);
            sd.write(&path, &self.eggs.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            self.eggs_saved = self.eggs.clone();
            written.push(path);
        }
        if !written.is_empty() {
            self.from_card = true;
        }
        Ok(written)
    }

    /// Draws the tab: the buttons of the groups, then the table of the chosen group.
    pub fn ui(&mut self, ui: &mut egui::Ui, data: &mut SpeciesData, session: &Session) {
        egui::ScrollArea::vertical().id_salt("learnset_tab").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (place, name) in GROUPS.iter().enumerate() {
                    self.group = tab(ui, place, name, self.group);
                }
                ui.add_space(12.0);
                if widget::button(ui, "Export CSV") {
                    self.export_csv(session);
                }
                if session.sd.is_some() && widget::button(ui, "Import CSV") {
                    self.import_csv(session);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(TABLE_WIDTH);
                match self.group {
                    LEVEL_UP => self.level_up_table(ui),
                    EGG => self.egg_table(ui),
                    TUTOR => self.tutor_table(ui, data),
                    _ => self.machine_table(ui, data),
                }
            });
        });
        // The moves go back in the order of the levels once the pointer is up and no field is being typed in.
        if self.needs_sort && !ui.input(|i| i.pointer.any_down()) && ui.memory(|m| m.focused().is_none()) {
            self.level_up.sort();
            self.needs_sort = false;
        }
    }

    /// The archives and the SD card that the CSV functions read.
    fn library<'a>(&'a self, sd: Option<&'a SdCard>) -> Option<learnset_csv::Library<'a>> {
        Some(learnset_csv::Library { species: self.species_archive.as_ref()?, level_up: self.level_up_archive.as_ref()?, eggs: self.egg_archive.as_ref()?, sd })
    }

    /// Writes the moves of all the species and forms to a CSV file that the user chooses.
    fn export_csv(&mut self, session: &Session) {
        let Some(library) = self.library(session.sd.as_ref()) else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("learnsets.csv").save_file() else { return };
        let text = learnset_csv::export(&library);
        let rows = text.lines().count() - 1;
        self.status = Some(match std::fs::write(&path, &text) {
            Ok(()) => (true, format!("Exported {rows} moves to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    /// Reads a CSV file that the user chooses, checks it, and writes the changes on the SD card.
    fn import_csv(&mut self, session: &Session) {
        let (Some(library), Some(sd)) = (self.library(session.sd.as_ref()), session.sd.as_ref()) else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() else { return };
        let result = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display())).and_then(|text| learnset_csv::parse(&text)).and_then(|rows| learnset_csv::apply(&rows, &library, sd));
        self.status = Some(match result {
            Ok(report) if report.total() == 0 => (true, "The file matches the current moves: nothing to write.".to_string()),
            Ok(report) => {
                self.imported = true;
                (true, format!("Imported: {} files written on the SD card. The game loads them again by itself.", report.total()))
            }
            Err(message) => (false, message),
        });
    }

    /// True once after an import wrote files.
    pub fn take_imported(&mut self) -> bool {
        std::mem::take(&mut self.imported)
    }

    /// The choices of a list of moves, each with its type.
    fn move_choices(&self) -> Vec<Choice> {
        (1..=MOVE.len()).map(|id| Choice { label: move_name(id as u16).to_string(), kind: self.moves.as_ref().and_then(|t| t.get(id)).map(|m| m.kind), value: id }).collect()
    }

    /// The header of a table: the names of the columns.
    fn header(&self, ui: &mut egui::Ui, first: &str) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, 24.0), egui::Sense::hover());
        let names = [first, "Move", "Type", "Category", "Power", "Accuracy", "PP"];
        for (place, name) in names.iter().enumerate() {
            let cell = column(rect, place);
            let (anchor, position) = if place >= 4 { (egui::Align2::RIGHT_CENTER, egui::pos2(cell.right(), cell.center().y)) } else { (egui::Align2::LEFT_CENTER, egui::pos2(cell.left() + 4.0, cell.center().y)) };
            ui.painter().text(position, anchor, name, egui::FontId::proportional(12.0), color::MUTED);
        }
        ui.separator();
    }

    /// A row: the background that shows under the pointer, and the rectangle to draw in.
    fn row(ui: &mut egui::Ui) -> egui::Rect {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, ROW_HEIGHT), egui::Sense::hover());
        if ui.rect_contains_pointer(rect) {
            ui.painter().rect_filled(rect, egui::Rounding::same(8.0), color::SURFACE_STRONG.linear_multiply(0.6));
        }
        rect
    }

    /// The columns of a move: type, category, power, accuracy and PP.
    fn move_columns(&self, ui: &egui::Ui, row: egui::Rect, id: u16) {
        let Some(info) = self.moves.as_ref().and_then(|table| table.get(id as usize)) else { return };
        let painter = ui.painter();
        let y = row.center().y;
        type_pill(painter, egui::pos2(column(row, 2).left(), y), info.kind, 12.0);
        let cell = column(row, 3);
        painter.circle_filled(egui::pos2(cell.left() + 6.0, y), 4.5, category_color(info.category));
        painter.text(egui::pos2(cell.left() + 18.0, y), egui::Align2::LEFT_CENTER, info.category.name(), egui::FontId::proportional(13.5), color::TEXT);
        let number = |place: usize, text: String| {
            let ink = if text == "—" { color::MUTED } else { color::TEXT };
            painter.text(egui::pos2(column(row, place).right(), y), egui::Align2::RIGHT_CENTER, text, egui::FontId::proportional(13.5), ink);
        };
        number(4, info.power_text());
        number(5, info.accuracy_text());
        number(6, info.pp.to_string());
    }

    /// The name of a move in its column. With `choices`, a click opens the list and gives the chosen move.
    fn move_cell(&self, ui: &mut egui::Ui, row: egui::Rect, id: u16, popup: Option<(egui::Id, &[Choice])>) -> Option<u16> {
        let cell = column(row, 1);
        let response = ui.interact(cell, ui.make_persistent_id(("learnset_move_cell", cell.min.x as i32, cell.min.y as i32)), egui::Sense::click());
        let editable = popup.is_some();
        if editable && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(cell.shrink2(egui::vec2(0.0, 6.0)), egui::Rounding::same(6.0), egui::Stroke::new(1.0_f32, color::BORDER));
        }
        ui.painter().text(egui::pos2(cell.left() + 8.0, cell.center().y), egui::Align2::LEFT_CENTER, move_name(id), egui::FontId::proportional(15.0), color::TEXT);
        popup.and_then(|(popup_id, choices)| choose(ui, popup_id, &response, choices, Some(id as usize))).map(|value| value as u16)
    }

    /// The level-up moves: one row for each, with its level and its move that the user can change.
    fn level_up_table(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Level");
        let choices = if ui.memory(|m| m.any_popup_open()) { self.move_choices() } else { Vec::new() };
        let mut remove = None;
        for index in 0..self.level_up.moves.len() {
            let mut entry = self.level_up.moves[index];
            let row = Self::row(ui);
            // The level: a number that is dragged or typed.
            let level_rect = egui::Rect::from_center_size(column(row, 0).left_center() + egui::vec2(28.0, 0.0), egui::vec2(56.0, 26.0));
            ui.painter().rect_filled(level_rect, egui::Rounding::same(13.0), color::SURFACE_STRONG);
            let mut level = entry.level as u32;
            ui.scope(|ui| {
                let widgets = &mut ui.visuals_mut().widgets;
                for style in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
                    style.weak_bg_fill = egui::Color32::TRANSPARENT;
                    style.bg_fill = egui::Color32::TRANSPARENT;
                    style.bg_stroke = egui::Stroke::NONE;
                }
                ui.put(level_rect, egui::DragValue::new(&mut level).range(1..=100).speed(0.15));
            });
            entry.level = level as u16;
            if let Some(id) = self.move_cell(ui, row, entry.id, Some((egui::Id::new(("learnset_level_up_move", index)), &choices))) {
                entry.id = id;
            }
            self.move_columns(ui, row, entry.id);
            if trash_button(ui, column(row, 7)) {
                remove = Some(index);
            }
            if entry.level != self.level_up.moves[index].level {
                self.needs_sort = true;
            }
            self.level_up.moves[index] = entry;
        }
        if let Some(index) = remove {
            self.level_up.moves.remove(index);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let room = self.level_up.has_room();
            let add = ui.add_enabled(room, egui::Button::new(egui::RichText::new("+ Add a move").color(color::WHITE)).fill(color::ACCENT));
            let add_id = egui::Id::new("learnset_level_up_add");
            let choices = if ui.memory(|m| m.any_popup_open()) { self.move_choices() } else { Vec::new() };
            if let Some(id) = choose(ui, add_id, &add, &choices, None) {
                let level = self.level_up.moves.last().map_or(1, |m| m.level);
                self.level_up.moves.push(LevelUpMove { id: id as u16, level });
            }
            ui.label(widget::hint(&format!("{} of {MAX_LEVEL_UP_MOVES} moves. The game keeps no more.", self.level_up.moves.len())));
        });
    }

    /// The egg moves.
    fn egg_table(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "");
        let choices = if ui.memory(|m| m.any_popup_open()) { self.move_choices() } else { Vec::new() };
        let mut remove = None;
        for index in 0..self.eggs.moves.len() {
            let row = Self::row(ui);
            let id = self.eggs.moves[index];
            let new = self.move_cell(ui, row, id, Some((egui::Id::new(("learnset_egg_move", index)), &choices)));
            self.move_columns(ui, row, new.unwrap_or(id));
            if trash_button(ui, column(row, 7)) {
                remove = Some(index);
            }
            if let Some(new) = new {
                self.eggs.moves[index] = new;
            }
        }
        if let Some(index) = remove {
            self.eggs.moves.remove(index);
        }
        ui.add_space(8.0);
        let add = ui.add(egui::Button::new(egui::RichText::new("+ Add a move").color(color::WHITE)).fill(color::ACCENT));
        if let Some(id) = choose(ui, egui::Id::new("learnset_egg_add"), &add, &choices, None) {
            self.eggs.push(id as u16);
        }
    }

    /// The moves of the tutors that the species can learn.
    fn tutor_table(&mut self, ui: &mut egui::Ui, data: &mut SpeciesData) {
        self.header(ui, "Tutor");
        let slots = Slot::all();
        let mut remove = None;
        for &(slot, id) in slots.iter().filter(|(slot, _)| slot.is_set(data)) {
            let row = Self::row(ui);
            ui.painter().rect_filled(egui::Rect::from_center_size(column(row, 0).left_center() + egui::vec2(31.0, 0.0), egui::vec2(62.0, 24.0)), egui::Rounding::same(12.0), color::SURFACE_STRONG);
            ui.painter().text(column(row, 0).left_center() + egui::vec2(31.0, 0.0), egui::Align2::CENTER_CENTER, slot.label(), egui::FontId::proportional(12.5), color::TEXT);
            self.move_cell(ui, row, id, None);
            self.move_columns(ui, row, id);
            if trash_button(ui, column(row, 7)) {
                remove = Some(slot);
            }
        }
        if let Some(slot) = remove {
            slot.set(data, false);
        }
        ui.add_space(8.0);
        let add = ui.add(egui::Button::new(egui::RichText::new("+ Add a move").color(color::WHITE)).fill(color::ACCENT));
        let choices: Vec<Choice> = if ui.memory(|m| m.any_popup_open()) {
            slots
                .iter()
                .enumerate()
                .filter(|(_, (slot, _))| !slot.is_set(data))
                .map(|(place, (slot, id))| Choice { label: format!("{} ({})", move_name(*id), slot.label()), kind: self.moves.as_ref().and_then(|t| t.get(*id as usize)).map(|m| m.kind), value: place })
                .collect()
        } else {
            Vec::new()
        };
        if let Some(place) = choose(ui, egui::Id::new("learnset_tutor_add"), &add, &choices, None) {
            slots[place].0.set(data, true);
        }
    }

    /// The TMs and HMs that the species can learn.
    fn machine_table(&mut self, ui: &mut egui::Ui, data: &mut SpeciesData) {
        self.header(ui, "TM/HM");
        let mut remove = None;
        for bit in (0..MACHINES.len()).filter(|&bit| machine_is_set(data, bit)) {
            let row = Self::row(ui);
            ui.painter().rect_filled(egui::Rect::from_center_size(column(row, 0).left_center() + egui::vec2(31.0, 0.0), egui::vec2(62.0, 24.0)), egui::Rounding::same(12.0), color::SURFACE_STRONG);
            ui.painter().text(column(row, 0).left_center() + egui::vec2(31.0, 0.0), egui::Align2::CENTER_CENTER, machine_name(bit), egui::FontId::proportional(12.5), color::TEXT);
            self.move_cell(ui, row, MACHINES[bit], None);
            self.move_columns(ui, row, MACHINES[bit]);
            if trash_button(ui, column(row, 7)) {
                remove = Some(bit);
            }
        }
        if let Some(bit) = remove {
            set_machine(data, bit, false);
        }
        ui.add_space(8.0);
        let add = ui.add(egui::Button::new(egui::RichText::new("+ Add a TM or HM").color(color::WHITE)).fill(color::ACCENT));
        let choices: Vec<Choice> = if ui.memory(|m| m.any_popup_open()) {
            (0..MACHINES.len())
                .filter(|&bit| !machine_is_set(data, bit))
                .map(|bit| Choice { label: format!("{} {}", machine_name(bit), move_name(MACHINES[bit])), kind: self.moves.as_ref().and_then(|t| t.get(MACHINES[bit] as usize)).map(|m| m.kind), value: bit })
                .collect()
        } else {
            Vec::new()
        };
        if let Some(bit) = choose(ui, egui::Id::new("learnset_machine_add"), &add, &choices, None) {
            set_machine(data, bit, true);
        }
    }
}

/// A tab button of the groups. Returns the chosen place.
fn tab(ui: &mut egui::Ui, place: usize, text: &str, chosen: usize) -> usize {
    let id = egui::Id::new(("learnset_group", place));
    let mut chosen = chosen;
    let active = widget::is_active(ui, id);
    if active && chosen != place {
        chosen = place;
    }
    let response = widget::tab_button(ui, text, chosen == place, active && widget::page_has_focus(ui));
    widget::register_focusable(ui, id, response.rect);
    if response.clicked() {
        chosen = place;
        widget::set_active_id(ui, id);
    }
    chosen
}

/// A trash can that deletes a row, drawn by hand (the font has no such glyph), in a cell. True when clicked.
pub(crate) fn trash_button(ui: &mut egui::Ui, cell: egui::Rect) -> bool {
    let rect = egui::Rect::from_center_size(cell.center(), egui::vec2(26.0, 26.0));
    let response = ui.interact(rect, ui.make_persistent_id(("learnset_trash", rect.min.x as i32, rect.min.y as i32)), egui::Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(rect, egui::Rounding::same(7.0), color::ROSE.linear_multiply(0.15));
    }
    let ink = if response.hovered() { color::ROSE } else { color::MUTED };
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let c = rect.center();
    let painter = ui.painter();
    // The lid and its handle, then the body with two lines.
    painter.line_segment([egui::pos2(c.x - 6.0, c.y - 4.5), egui::pos2(c.x + 6.0, c.y - 4.5)], stroke);
    painter.line_segment([egui::pos2(c.x - 2.0, c.y - 6.5), egui::pos2(c.x + 2.0, c.y - 6.5)], stroke);
    painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(c.x - 4.5, c.y - 4.5), egui::pos2(c.x + 4.5, c.y + 6.5)), egui::Rounding::same(1.5), stroke);
    painter.line_segment([egui::pos2(c.x - 1.5, c.y - 1.5), egui::pos2(c.x - 1.5, c.y + 3.5)], stroke);
    painter.line_segment([egui::pos2(c.x + 1.5, c.y - 1.5), egui::pos2(c.x + 1.5, c.y + 3.5)], stroke);
    response.on_hover_text("Delete this move").clicked()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_names_of_the_special_tutor_moves_are_in_the_list() {
        for name in SPECIAL_TUTORS {
            assert!(move_id(name).is_some(), "{name}");
        }
        assert_eq!(MOVE.len(), 621);
        assert_eq!(move_id("Tackle"), Some(33));
        assert_eq!(move_id("Double-Edge"), Some(38));
        assert_eq!(move_id("Nothing"), None);
        assert_eq!(move_name(33), "Tackle");
        assert_eq!(move_name(0), "-");
    }

    #[test]
    fn tutor_slots_are_read_and_written_as_bits() {
        let mut data = SpeciesData::from_bytes(&[0; 80]).unwrap();
        data.special_tutor_moves = 0b1000101;
        data.bp_tutor_moves = [0x40, 0, 0x40f, 0x2002];
        let set: Vec<Slot> = Slot::all().into_iter().map(|(slot, _)| slot).filter(|slot| slot.is_set(&data)).collect();
        assert_eq!(set.len(), 3 + 1 + 5 + 2);
        assert!(set.contains(&Slot::Special(6)) && set.contains(&Slot::Bp(0, 6)) && set.contains(&Slot::Bp(3, 13)));
        // The total number of tutor moves is 7 special ones and 15 + 17 + 16 + 15 charged ones.
        assert_eq!(Slot::all().len(), 7 + 63);
        Slot::Bp(0, 6).set(&mut data, false);
        Slot::Special(1).set(&mut data, true);
        assert_eq!((data.bp_tutor_moves[0], data.special_tutor_moves), (0, 0b1000111));
        assert_eq!(Slot::Special(0).label(), "Special");
        assert_eq!(Slot::Bp(2, 0).label(), "BP 3");
    }

    #[test]
    fn machines_are_read_and_written_as_bits() {
        let mut data = SpeciesData::from_bytes(&[0; 80]).unwrap();
        set_machine(&mut data, 5, true);
        set_machine(&mut data, 106, true);
        assert!(machine_is_set(&data, 5) && machine_is_set(&data, 106) && !machine_is_set(&data, 6));
        assert_eq!((data.tm_hm_moves[0], data.tm_hm_moves[3]), (1 << 5, 1 << 10));
        set_machine(&mut data, 5, false);
        assert_eq!(data.tm_hm_moves[0], 0);
    }

    #[test]
    fn types_and_categories_have_a_colour() {
        let distinct: std::collections::HashSet<_> = (0..18).map(type_color).collect();
        assert_eq!(distinct.len(), 18);
        assert_ne!(category_color(Category::Physical), category_color(Category::Special));
    }

    /// With the dump of the game: the moves of Bulbasaur are read, drawn in every group, changed, saved on
    /// the SD card and read again.
    #[test]
    fn the_moves_of_bulbasaur_are_read_drawn_changed_and_saved() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_learnset_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut learnset = Learnset::default();
        learnset.open(&Dump::open(&root).unwrap());
        learnset.load(Some(&sd), 1, 0, 1);
        assert_eq!(learnset.level_up.moves.len(), 14);
        assert_eq!(learnset.eggs.moves.len(), 15);
        assert!(!learnset.changed() && !learnset.from_card());

        // Every group draws, with the move data of the game.
        let garc = Garc::open(&std::path::Path::new(&root).join("a/1/9/5")).unwrap();
        let mut data = SpeciesData::from_bytes(garc.first_file(1).unwrap()).unwrap();
        let ctx = egui::Context::default();
        let session = Session::default();
        let frame = |learnset: &mut Learnset, data: &mut SpeciesData| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0)));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| learnset.ui(ui, data, &session));
            });
        };
        for group in 0..GROUPS.len() {
            learnset.group = group;
            frame(&mut learnset, &mut data);
        }
        let tackle = learnset.moves.as_ref().unwrap().get(33).unwrap();
        assert_eq!((tackle.power_text().as_str(), tackle.pp), ("50", 35));

        // A level-up move is added, in the order of the levels, and one is deleted. The room is counted.
        learnset.level_up.moves.push(LevelUpMove { id: 74, level: 2 });
        learnset.level_up.sort();
        learnset.level_up.moves.remove(0);
        assert_eq!(learnset.level_up.moves.len(), 14);
        assert_eq!(learnset.level_up.moves[0], LevelUpMove { id: 74, level: 2 });
        learnset.eggs.moves.pop();
        assert!(learnset.changed());

        // A level that changes puts the move back in the order, once nothing is held.
        learnset.level_up.moves[0].level = 99;
        learnset.needs_sort = true;
        learnset.group = LEVEL_UP;
        frame(&mut learnset, &mut data);
        assert_eq!(learnset.level_up.moves.last().unwrap().level, 99);
        assert!(!learnset.needs_sort);

        // Only the two files that changed are written, and they come back.
        let written = learnset.save(&sd, 1, 0).unwrap();
        assert_eq!(written, ["pokemon/learnset/1_0.bin", "pokemon/egg_moves/1.bin"]);
        assert!(!learnset.changed() && learnset.from_card());
        assert_eq!(learnset.save(&sd, 1, 0).unwrap(), Vec::<String>::new());
        let mut again = Learnset::default();
        again.open(&Dump::open(&root).unwrap());
        again.load(Some(&sd), 1, 0, 1);
        assert_eq!(again.level_up, learnset.level_up);
        assert_eq!(again.eggs, learnset.eggs);
        assert!(again.from_card());

        // Reset gives back the romfs.
        again.reset(1, 1);
        assert_eq!(again.level_up.moves.len(), 14);
        assert_eq!(again.level_up.moves[0], LevelUpMove { id: 33, level: 1 });
        assert_eq!(again.eggs.moves.len(), 15);
        assert!(again.changed());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
