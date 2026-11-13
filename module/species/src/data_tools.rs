//! The tools that work on many files at once: the CSV files of all the species, the reset of the data to the romfs,
//! and the undo of both.
//!
//! The data that the studio changes is in files of the SD card. The files of the romfs are never changed. A reset
//! removes the files of the SD card (the game then reads the romfs) or, for the texts, puts the romfs text back
//! in the files. Before a reset or an import, the tools keep a copy of the files of the parts that the operation
//! touches: Undo puts the files back as they were.
//!
//! | Part | Folder or file of the SD card |
//! | --- | --- |
//! | Data | `pokemon/data`, `pokemon/pokedex_data.bin` |
//! | Moves | `pokemon/learnset`, `pokemon/egg_moves` |
//! | Evolutions | `pokemon/evolution` |
//! | Texts | `text` |
//! | Icons | `pokemon/icon` |

use crate::pokedex_data::{self, PokedexData};
use crate::text_csv;
use crate::text_tab::FIELDS;
use crate::{csv, icon_tab, names, Species};
use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{evolution, learnset, Backup, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::BTreeSet;
use std::path::Path;

/// How many undo steps are kept.
const HISTORY: usize = 20;

/// A kind of data that has its own files and its own tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Data,
    Moves,
    Evolutions,
    Texts,
    Icons,
}

impl Part {
    pub const ALL: [Part; 5] = [Part::Data, Part::Moves, Part::Evolutions, Part::Texts, Part::Icons];

    fn label(self) -> &'static str {
        match self {
            Part::Data => "Data",
            Part::Moves => "Moves",
            Part::Evolutions => "Evolutions",
            Part::Texts => "Texts",
            Part::Icons => "Icons",
        }
    }

    /// What the part holds, for the help text.
    fn hint(self) -> &'static str {
        match self {
            Part::Data => "General tab: stats, types, abilities, items, shape and the other fields.",
            Part::Moves => "Learnset tab: level-up moves and egg moves.",
            Part::Evolutions => "Evolution tab.",
            Part::Texts => "Text tab: name, category, entry, form name, height and weight, in every language.",
            Part::Icons => "The icon card of the General tab: the pictures of the species in the lists.",
        }
    }

    /// The folders of the SD card that the part uses.
    fn folders(self) -> &'static [&'static str] {
        match self {
            Part::Data => &["pokemon/data"],
            Part::Moves => &["pokemon/learnset", "pokemon/egg_moves"],
            Part::Evolutions => &["pokemon/evolution"],
            Part::Texts => &["text"],
            Part::Icons => &["pokemon/icon"],
        }
    }

    /// The single files of the SD card that the part uses.
    fn files(self) -> &'static [&'static str] {
        match self {
            Part::Data => &[pokedex_data::SD_PATH],
            _ => &[],
        }
    }
}

/// The part that a tab of the page shows, when it has one.
pub fn tab_part(tab: usize) -> Option<Part> {
    match tab {
        0 => Some(Part::Data),
        2 => Some(Part::Moves),
        3 => Some(Part::Evolutions),
        4 => Some(Part::Texts),
        _ => None,
    }
}

/// How much a reset reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The chosen species and form.
    Form,
    /// The chosen species, all its forms.
    Species,
    /// All the species.
    All,
}

/// Copies the files of some parts.
fn snapshot(sd: &SdCard, parts: &[Part], label: String) -> Backup {
    let folders: Vec<&str> = parts.iter().flat_map(|part| part.folders().iter().copied()).collect();
    let loose: Vec<&str> = parts.iter().flat_map(|part| part.files().iter().copied()).collect();
    Backup::snapshot(sd, &folders, &loose, label)
}

/// What a reset needs to know about the romfs.
pub struct RomLibrary<'a> {
    /// The archive of the species data.
    pub species: &'a Garc,
    /// The Pokédex data of the romfs: the shapes and the places of the forms.
    pub pokedex: Option<&'a PokedexData>,
    /// A file of texts as the romfs has it: the language and the file.
    pub texts: &'a dyn Fn(usize, usize) -> Option<TextFile>,
}

/// Puts the files of the parts back to the romfs, for a scope. The files of the SD card are removed, except the
/// Pokédex data and the files of texts that hold other species too: their lines are put back.
pub fn reset(sd: &SdCard, scope: Scope, parts: &[Part], species: usize, form: usize, library: &RomLibrary) -> Result<(), String> {
    let forms: Vec<usize> = match scope {
        Scope::Form => vec![form],
        Scope::Species => (0..csv::form_count(library.species, species)).collect(),
        Scope::All => Vec::new(),
    };
    let remove = |paths: Vec<String>| -> Result<(), String> {
        for path in paths {
            sd.remove(&path)?;
        }
        Ok(())
    };
    for part in parts {
        match (part, scope) {
            (Part::Data, Scope::All) => {
                remove(sd.files_in("pokemon/data"))?;
                remove(vec![pokedex_data::SD_PATH.to_string()])?;
            }
            (Part::Data, _) => {
                remove(forms.iter().map(|&f| crate::sd_path(species, f)).collect())?;
                reset_shapes(sd, species, &forms, library)?;
            }
            (Part::Moves, Scope::All) => {
                remove(sd.files_in("pokemon/learnset"))?;
                remove(sd.files_in("pokemon/egg_moves"))?;
            }
            (Part::Moves, _) => {
                remove(forms.iter().map(|&f| learnset::level_up_sd_path(species, f)).collect())?;
                // The egg moves belong to the species: the first form takes them with it.
                if forms.contains(&0) {
                    remove(vec![learnset::egg_sd_path(species)])?;
                }
            }
            (Part::Evolutions, Scope::All) => remove(sd.files_in("pokemon/evolution"))?,
            (Part::Evolutions, _) => remove(forms.iter().map(|&f| evolution::sd_path(species, f)).collect())?,
            (Part::Icons, Scope::All) => remove(sd.files_in("pokemon/icon"))?,
            (Part::Icons, _) => {
                let files: BTreeSet<usize> = forms.iter().flat_map(|&f| [false, true].into_iter().filter_map(move |female| icon_tab::icon_file(species, f, female))).collect();
                remove(files.into_iter().map(icon_tab::sd_path).collect())?;
            }
            (Part::Texts, Scope::All) => {
                for language in 0..LANGUAGES.len() {
                    remove(FIELDS.iter().map(|field| text::sd_path(language, field.file)).collect())?;
                }
            }
            (Part::Texts, _) => reset_texts(sd, species, &forms, library)?,
        }
    }
    Ok(())
}

/// Gives the shapes of the forms the value of the romfs, in the Pokédex data of the SD card (when it is there).
fn reset_shapes(sd: &SdCard, species: usize, forms: &[usize], library: &RomLibrary) -> Result<(), String> {
    let Some(mut on_card) = sd.read(pokedex_data::SD_PATH).and_then(|bytes| PokedexData::from_bytes(bytes).ok()) else { return Ok(()) };
    let Some(rom) = library.pokedex else { return Ok(()) };
    for &form in forms {
        if let Some(shape) = rom.shape(species, form) {
            on_card.set_shape(species, form, shape);
        }
    }
    sd.write(pokedex_data::SD_PATH, &on_card.to_bytes())?;
    sd.mark_changed(pokedex_data::SD_PATH)
}

/// Puts the texts of the forms back to the romfs text in the files of the SD card. A file that is the romfs file
/// again is removed.
fn reset_texts(sd: &SdCard, species: usize, forms: &[usize], library: &RomLibrary) -> Result<(), String> {
    let mut lines: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &form in forms {
        let place = library.pokedex.map_or(species, |pokedex| pokedex.place(species, form));
        for field in &FIELDS {
            lines.insert((field.file, crate::text_tab::TextTab::line_of(field, species, place)));
        }
    }
    for language in 0..LANGUAGES.len() {
        let files: BTreeSet<usize> = lines.iter().map(|(file, _)| *file).collect();
        for file in files {
            let path = text::sd_path(language, file);
            let Some(mut on_card) = sd.read(&path).and_then(|bytes| TextFile::from_bytes(&bytes).ok()) else { continue };
            let Some(rom) = (library.texts)(language, file) else { continue };
            for &(_, line) in lines.iter().filter(|(f, _)| *f == file) {
                if let Some(original) = rom.text(line) {
                    on_card.set_text(line, &original)?;
                }
            }
            if on_card == rom {
                sd.remove(&path)?;
            } else {
                sd.write(&path, &on_card.to_bytes())?;
                sd.mark_changed(&path)?;
            }
        }
    }
    Ok(())
}

/// The state of the tools.
pub struct Tools {
    /// The copies for Undo, the last one at the end.
    pub history: Vec<Backup>,
    pub reset_open: bool,
    pub scope: Scope,
    pub parts: [bool; 5],
    /// The parts of the CSV files: Data, Moves, Evolutions, Texts.
    pub csv_parts: [bool; 4],
}

impl Default for Tools {
    fn default() -> Self {
        Tools { history: Vec::new(), reset_open: false, scope: Scope::Form, parts: [false; 5], csv_parts: [true; 4] }
    }
}

/// The CSV parts and the name of their file.
const CSV_PARTS: [(Part, &str, &str); 4] = [
    (Part::Data, "species.csv", "Species data"),
    (Part::Moves, "learnsets.csv", "Moves"),
    (Part::Evolutions, "evolutions.csv", "Evolutions"),
    (Part::Texts, "texts.csv", "Texts"),
];

impl Species {
    /// Reads again everything that the files of the SD card can change.
    fn files_changed(&mut self) {
        self.loaded = None;
        self.list_rows = None;
        self.text.reload();
        self.icon_tab.reload();
    }

    /// Keeps a backup when the operation changed files, and says what happened.
    fn finish(&mut self, backup: Backup, session: &Session, done: &str) {
        let Some(sd) = session.sd.as_ref() else { return };
        let count = backup.changes(sd);
        self.files_changed();
        if count == 0 {
            self.status = Some((true, "Nothing changed: the files are already like this.".into()));
            return;
        }
        self.tools.history.push(backup);
        if self.tools.history.len() > HISTORY {
            self.tools.history.remove(0);
        }
        self.status = Some((true, format!("{done} {count} file(s) changed. Undo gives them back.")));
    }

    /// The parts that a reset window has ticked.
    fn ticked_parts(&self) -> Vec<Part> {
        Part::ALL.iter().zip(self.tools.parts).filter(|(_, on)| *on).map(|(part, _)| *part).collect()
    }

    /// Puts the files of some parts back to the romfs, for a scope.
    pub fn run_reset(&mut self, session: &Session, scope: Scope, parts: &[Part]) {
        let (Some(sd), Some(garc)) = (session.sd.as_ref(), self.garc.as_ref()) else {
            self.status = Some((false, "Choose the folder of the SD card first.".into()));
            return;
        };
        if parts.is_empty() {
            return;
        }
        let names_of: Vec<&str> = parts.iter().map(|part| part.label()).collect();
        let place = match scope {
            Scope::Form => format!("#{:04} {} form {}", self.species, names::SPECIES[self.species - 1], self.form),
            Scope::Species => format!("#{:04} {}", self.species, names::SPECIES[self.species - 1]),
            Scope::All => "all the species".to_string(),
        };
        let label = format!("Reset {} of {place}", names_of.join(", "));
        let backup = snapshot(sd, parts, label);
        let rom_pokedex = self.read_pokedex_romfs().ok();
        let result = {
            let text = &self.text;
            let texts = |language: usize, file: usize| text.romfs(language, file);
            let library = RomLibrary { species: garc, pokedex: rom_pokedex.as_ref(), texts: &texts };
            reset(sd, scope, parts, self.species, self.form, &library)
        };
        match result {
            Ok(()) => {
                let done = format!("Reset of {} ({place}).", names_of.join(", "));
                self.finish(backup, session, &done);
            }
            Err(message) => {
                // What was done before the error can be undone.
                self.finish(backup, session, "Reset stopped.");
                self.status = Some((false, format!("{message} The files that were already changed can be undone.")));
            }
        }
    }

    /// Gives back the files of the last operation.
    pub fn undo(&mut self, session: &Session) {
        let (Some(sd), Some(backup)) = (session.sd.as_ref(), self.tools.history.pop()) else { return };
        self.status = Some(match backup.restore(sd) {
            Ok(count) => (true, format!("Undone: {}. {count} file(s) put back.", backup.label)),
            Err(message) => (false, message),
        });
        self.files_changed();
    }

    /// The text that the Undo button shows when the pointer is on it.
    pub fn undo_label(&self) -> Option<&str> {
        self.tools.history.last().map(|backup| backup.label.as_str())
    }

    /// The window of the reset: the scope and the parts.
    pub fn reset_window(&mut self, ctx: &egui::Context, session: &Session) {
        if !self.tools.reset_open {
            return;
        }
        let mut open = true;
        let mut run = false;
        let mut close = false;
        let species_name = names::SPECIES[self.species - 1];
        egui::Window::new("Reset to the romfs").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new("How much").strong());
            ui.radio_value(&mut self.tools.scope, Scope::Form, format!("This form: #{:04} {species_name}, form {}", self.species, self.form));
            ui.radio_value(&mut self.tools.scope, Scope::Species, format!("This species, all its forms: #{:04} {species_name}", self.species));
            ui.radio_value(&mut self.tools.scope, Scope::All, "All the species");
            ui.add_space(8.0);
            ui.label(egui::RichText::new("What").strong());
            for (place, part) in Part::ALL.iter().enumerate() {
                ui.checkbox(&mut self.tools.parts[place], part.label()).on_hover_text(part.hint());
                ui.label(widget::hint(part.hint()));
            }
            ui.horizontal(|ui| {
                if ui.small_button("All parts").clicked() {
                    self.tools.parts = [true; 5];
                }
                if ui.small_button("No part").clicked() {
                    self.tools.parts = [false; 5];
                }
            });
            ui.add_space(8.0);
            ui.label(widget::hint("The files of the SD card go back to the data of the romfs. The changes that are not saved are lost. Undo gives the files back."));
            if self.tools.scope == Scope::All {
                ui.label(egui::RichText::new("This reaches every species.").color(color::ROSE));
            }
            ui.label(widget::hint("The game takes the removed files again at the next start."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let any = self.tools.parts.iter().any(|on| *on) && session.sd.is_some();
                ui.add_enabled_ui(any, |ui| {
                    run = widget::button_styled(ui, egui::RichText::new("Reset").color(color::WHITE), Some(color::ROSE));
                });
                close = widget::button(ui, "Cancel");
            });
        });
        if run {
            let parts = self.ticked_parts();
            let scope = self.tools.scope;
            self.run_reset(session, scope, &parts);
            self.tools.reset_open = false;
        }
        if close || !open {
            self.tools.reset_open = false;
        }
    }

    /// The card of the CSV files in the General tab: all the species, whatever the species that is open.
    pub fn csv_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        widget::card(ui, |ui| {
            ui.label(widget::title("CSV files of all the species"));
            ui.label(widget::hint("These buttons work on all the species and all the forms, not only on the one that is open."));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (place, (_, file, label)) in CSV_PARTS.iter().enumerate() {
                    ui.checkbox(&mut self.tools.csv_parts[place], *label).on_hover_text(*file);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let any = self.tools.csv_parts.iter().any(|on| *on);
                ui.add_enabled_ui(any && self.garc.is_some(), |ui| {
                    if widget::button(ui, "Export CSV…") {
                        self.export_csv_files(session);
                    }
                    if session.sd.is_some() && widget::button(ui, "Import CSV…") {
                        self.import_csv_files(session);
                    }
                });
            });
            ui.label(widget::hint("Export writes one file for each ticked part in a folder that you choose: species.csv, learnsets.csv, evolutions.csv and texts.csv. Import reads the files with these names from a folder, checks all of them, and writes the changes on the SD card. Undo gives the files back."));
        });
    }

    /// The texts library for the CSV functions.
    fn with_text_library<R>(&self, sd: Option<&SdCard>, call: impl FnOnce(&text_csv::Library) -> R) -> Option<R> {
        let garc = self.garc.as_ref()?;
        let texts = |language: usize, file: usize| self.text.romfs(language, file);
        let library = text_csv::Library { species: garc, pokedex: self.pokedex.as_ref(), sd, romfs: &texts };
        Some(call(&library))
    }

    /// Writes the CSV files of the ticked parts in a folder that the user chooses.
    fn export_csv_files(&mut self, session: &Session) {
        let Some(garc) = &self.garc else { return };
        let Some(folder) = rfd::FileDialog::new().set_title("Folder for the CSV files").pick_folder() else { return };
        let sd = session.sd.as_ref();
        let mut written = Vec::new();
        let mut result: Result<(), String> = Ok(());
        for (place, (part, file, _)) in CSV_PARTS.iter().enumerate() {
            if !self.tools.csv_parts[place] {
                continue;
            }
            let text = match part {
                Part::Data => Some(csv::export(garc, sd, self.pokedex.as_ref())),
                Part::Moves => self.learnset.csv_export(sd),
                Part::Evolutions => self.evolution.csv_export(sd),
                _ => self.with_text_library(sd, text_csv::export),
            };
            let Some(text) = text else {
                result = Err(format!("{file}: the data is not open."));
                break;
            };
            let path = folder.join(file);
            if let Err(error) = std::fs::write(&path, &text) {
                result = Err(format!("{}: {error}", path.display()));
                break;
            }
            written.push(*file);
        }
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported {} for all the species to {}.", written.join(", "), folder.display())),
            Err(message) => (false, message),
        });
    }

    /// Reads the CSV files of the ticked parts from a folder that the user chooses, checks them, and writes the changes.
    fn import_csv_files(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let Some(folder) = rfd::FileDialog::new().set_title("Folder of the CSV files").pick_folder() else { return };
        let parts: Vec<Part> = CSV_PARTS.iter().enumerate().filter(|(place, _)| self.tools.csv_parts[*place]).map(|(_, (part, _, _))| *part).collect();
        let backup = snapshot(sd, &parts, format!("Import of CSV files from {}", folder.display()));
        let mut report = Vec::new();
        let mut failure = None;
        for (place, (part, file, label)) in CSV_PARTS.iter().enumerate() {
            let path = folder.join(file);
            if !self.tools.csv_parts[place] || !Path::new(&path).is_file() {
                continue;
            }
            let outcome = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display())).and_then(|text| self.import_one(*part, &text, sd));
            match outcome {
                Ok(count) => report.push(format!("{label} {count}")),
                Err(message) => {
                    failure = Some(format!("{file}: {message}"));
                    break;
                }
            }
        }
        if let Some(message) = failure {
            self.finish(backup, session, "Import stopped.");
            self.status = Some((false, format!("{message} The files that were already written can be undone.")));
            return;
        }
        if report.is_empty() {
            self.status = Some((false, format!("No file of the ticked parts is in {}.", folder.display())));
            return;
        }
        let done = format!("Imported ({}).", report.join(", "));
        self.finish(backup, session, &done);
    }

    /// Checks and applies the text of one CSV file. Gives the number of files that were written.
    fn import_one(&mut self, part: Part, text: &str, sd: &SdCard) -> Result<usize, String> {
        match part {
            Part::Data => {
                let garc = self.garc.as_ref().ok_or("The archive is not open.")?;
                let rows = csv::parse(text)?;
                let report = csv::apply(&rows, garc, sd, &mut self.pokedex)?;
                Ok(report.written + report.shape_written as usize)
            }
            Part::Moves => self.learnset.csv_import(sd, text),
            Part::Evolutions => self.evolution.csv_import(sd, text),
            _ => self
                .with_text_library(Some(sd), |library| text_csv::parse(text, library).and_then(|rows| text_csv::apply(&rows, library, sd)))
                .unwrap_or_else(|| Err("The archive is not open.".into())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn card(name: &str) -> SdCard {
        let path = std::env::temp_dir().join(format!("sango_tools_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        SdCard::open(path.to_str().unwrap()).unwrap()
    }

    /// With the dump of the game: the resets reach the scope that is asked, and only the parts that are asked.
    #[test]
    fn the_resets_reach_the_scope_and_the_parts() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let species = Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let archives: Vec<Option<Garc>> = LANGUAGES.iter().map(|l| Garc::open(&dump.archive(l.archive)).ok()).collect();
        let texts = |language: usize, file: usize| archives.get(language)?.as_ref()?.first_file(file).and_then(|b| TextFile::from_bytes(b).ok());
        let rom_pokedex = Garc::open(&dump.archive(pokedex_data::ARCHIVE)).ok().and_then(|g| g.first_file(pokedex_data::FILE).and_then(|b| PokedexData::from_bytes(b.to_vec()).ok()));
        let library = RomLibrary { species: &species, pokedex: rom_pokedex.as_ref(), texts: &texts };
        let sd = card("reset");
        for file in ["pokemon/data/1_0.bin", "pokemon/data/2_0.bin", "pokemon/learnset/1_0.bin", "pokemon/learnset/2_0.bin", "pokemon/evolution/1_0.bin"] {
            sd.write(file, b"x").unwrap();
        }
        // The data of one form only.
        reset(&sd, Scope::Form, &[Part::Data], 1, 0, &library).unwrap();
        assert!(sd.read("pokemon/data/1_0.bin").is_none());
        assert!(sd.read("pokemon/data/2_0.bin").is_some());
        assert!(sd.read("pokemon/learnset/1_0.bin").is_some(), "the moves are another part");
        // The moves of all the species.
        reset(&sd, Scope::All, &[Part::Moves], 1, 0, &library).unwrap();
        assert!(sd.files_in("pokemon/learnset").is_empty());
        assert!(sd.read("pokemon/data/2_0.bin").is_some());
        // A text of one species goes back, the others stay.
        let mut file = TextFile::from_bytes(texts(2, 98).unwrap().to_bytes().as_slice()).unwrap();
        file.set_text(1, "Changed").unwrap();
        file.set_text(2, "Changed too").unwrap();
        sd.write(&text::sd_path(2, 98), &file.to_bytes()).unwrap();
        reset(&sd, Scope::Species, &[Part::Texts], 1, 0, &library).unwrap();
        let after = TextFile::from_bytes(&sd.read(&text::sd_path(2, 98)).unwrap()).unwrap();
        assert_eq!(after.text(1).as_deref(), Some("Bulbasaur"));
        assert_eq!(after.text(2).as_deref(), Some("Changed too"));
        // The last change goes back: the file is the romfs file again, so it is removed.
        reset(&sd, Scope::Species, &[Part::Texts], 2, 0, &library).unwrap();
        assert!(sd.read(&text::sd_path(2, 98)).is_none());
    }
}
