//! The tools that work on many files at once: the CSV files of all the items, the reset to the romfs, and the undo.
//!
//! The files that the studio changes are on the SD card. A reset removes the files of the SD card (the game then reads
//! the romfs), or, for the texts of one item, puts the romfs text back in the files. Before a reset or an import, the
//! tools keep a copy of the files of the parts that the operation touches: Undo puts the files back.
//!
//! | Part | Folder of the SD card |
//! | --- | --- |
//! | Data | `item/data`, `item/battle_pocket.bin` |
//! | Texts | `text` |
//! | Icons | `item/icon` |

use crate::{csv, data, texts, Item};
use sango_core::{Backup, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::path::Path;

/// How many undo steps are kept.
const HISTORY: usize = 20;

/// A kind of data that has its own files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Data,
    Texts,
    Icons,
    Reactions,
}

impl Part {
    const ALL: [Part; 4] = [Part::Data, Part::Texts, Part::Icons, Part::Reactions];

    fn label(self) -> &'static str {
        match self {
            Part::Data => "Data",
            Part::Texts => "Texts",
            Part::Icons => "Icon",
            Part::Reactions => "Reactions",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Part::Data => "price, pocket, held effect, effects on a Pokémon and the other fields.",
            Part::Texts => "Name, name in messages and description, in every language.",
            Part::Icons => "The picture of the item (the card of the icon).",
            Part::Reactions => "The rules that replace the code of the game are deleted: the game runs its own code again.",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Part::Data => data::SD_FOLDER,
            Part::Texts => "game_text",
            Part::Icons => crate::icons::SD_FOLDER,
            Part::Reactions => sango_rules::game_files::ITEMS.folder,
        }
    }

    /// The single files of the SD card that the part uses.
    fn files(self) -> &'static [&'static str] {
        match self {
            Part::Data => &[crate::pockets::SD_PATH],
            _ => &[],
        }
    }
}

/// How much a reset reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The chosen item.
    Item,
    /// All the items.
    All,
}

/// The state of the tools.
pub struct Tools {
    /// The copies for Undo, the last one at the end.
    pub history: Vec<Backup>,
    pub reset_open: bool,
    pub scope: Scope,
    /// The parts of the reset window: Data, Texts, Icon, Reactions.
    pub parts: [bool; 4],
    /// The parts of the CSV files: Data, Texts.
    pub csv_parts: [bool; 2],
    /// The windows of the buttons Export… and Import….
    pub export_open: bool,
    pub import_open: bool,
}

impl Default for Tools {
    fn default() -> Self {
        Tools { history: Vec::new(), reset_open: false, scope: Scope::Item, parts: [true, true, false, false], csv_parts: [true; 2], export_open: false, import_open: false }
    }
}

/// The CSV parts: the part, the name of the file, and the label.
const CSV_PARTS: [(Part, &str, &str); 2] = [(Part::Data, "items.csv", "Item data"), (Part::Texts, "item_texts.csv", "Texts")];

/// Copies the files of some parts.
fn snapshot(sd: &SdCard, parts: &[Part], label: String) -> Backup {
    let folders: Vec<&str> = parts.iter().map(|part| part.folder()).collect();
    let loose: Vec<&str> = parts.iter().flat_map(|part| part.files().iter().copied()).collect();
    Backup::snapshot(sd, &folders, &loose, label)
}

impl Item {
    /// Reads again everything that the files of the SD card can change.
    fn files_changed(&mut self) {
        self.loaded = None;
        self.list_names = None;
        self.texts.reload();
        self.game_rules = sango_rules::game_files::Store::new(&sango_rules::game_files::ITEMS);
        self.draft = None;
        self.icons_ready = false;
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
        self.status = Some((true, format!("{done} {count} file(s) changed.")));
    }

    /// Puts the files of some parts back to the romfs, for a scope.
    pub fn run_reset(&mut self, session: &Session, scope: Scope, parts: &[Part]) {
        let Some(sd) = session.sd.as_ref() else {
            self.status = Some((false, "Choose the folder of the SD card first.".into()));
            return;
        };
        if parts.is_empty() {
            return;
        }
        let names: Vec<&str> = parts.iter().map(|part| part.label()).collect();
        let place = match scope {
            Scope::Item => format!("item #{:03} {}", self.item, self.texts.name(Some(sd), self.item)),
            Scope::All => "all the items".to_string(),
        };
        let backup = snapshot(sd, parts, format!("Reset {} of {place}", names.join(", ")));
        let mut reactions: Result<(), String> = Ok(());
        if parts.contains(&Part::Reactions) {
            let with_rules = self.game_rules.ids();
            let ids: Vec<u16> = match scope {
                Scope::Item => vec![self.item as u16],
                Scope::All => with_rules.clone(),
            };
            for id in ids.into_iter().filter(|id| with_rules.contains(id)) {
                if let Err(message) = self.game_rules.delete(sd, id) {
                    reactions = Err(message);
                    break;
                }
            }
            self.draft = None;
        }
        let result = reactions.and_then(|_| (|| -> Result<(), String> {
            for part in parts {
                match (part, scope) {
                    (Part::Reactions, _) => {}
                    (Part::Data, Scope::Item) => {
                        sd.remove(&data::sd_path(self.item))?;
                        crate::pockets::reset_item(sd, self.pocket_garc.as_ref(), self.item)?;
                    }
                    (Part::Data, Scope::All) => {
                        for path in sd.files_in(data::SD_FOLDER) {
                            sd.remove(&path)?;
                        }
                        sd.remove(crate::pockets::SD_PATH)?;
                    }
                    (Part::Texts, Scope::Item) => {
                        let text = &self.texts;
                        texts::reset_item(sd, self.item, &|language, file| text.romfs(language, file))?;
                    }
                    (Part::Texts, Scope::All) => texts::reset_all(sd)?,
                    (Part::Icons, Scope::Item) => {
                        if let Some(file) = crate::icons::ItemIcons::file_of(self.item) {
                            sd.remove(&crate::icons::sd_path(file))?;
                        }
                    }
                    (Part::Icons, Scope::All) => {
                        for path in sd.files_in(crate::icons::SD_FOLDER) {
                            sd.remove(&path)?;
                        }
                    }
                }
            }
            Ok(())
        })());
        match result {
            Ok(()) => {
                let done = format!("Reset of {} ({place}).", names.join(", "));
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

    /// The window of the reset: the scope and the parts.
    pub fn reset_window(&mut self, ctx: &egui::Context, session: &Session) {
        if !self.tools.reset_open {
            return;
        }
        let mut open = true;
        let mut run = false;
        let mut close = false;
        let name = self.texts.name(session.sd.as_ref(), self.item);
        egui::Window::new("Reset to the romfs").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new("How much").strong());
            ui.radio_value(&mut self.tools.scope, Scope::Item, format!("This item: #{:03} {name}", self.item));
            ui.radio_value(&mut self.tools.scope, Scope::All, "All the items");
            ui.add_space(8.0);
            ui.label(egui::RichText::new("What").strong());
            for (place, part) in Part::ALL.iter().enumerate() {
                widget::chip(ui, part.label(), &mut self.tools.parts[place]).on_hover_text(part.hint());
                ui.label(widget::hint(part.hint()));
            }
            ui.add_space(8.0);
            ui.label(widget::hint("The files of the SD card go back to the data of the romfs. The changes that are not saved are lost."));
            if self.tools.scope == Scope::All {
                ui.label(egui::RichText::new("This reaches every item.").color(color::ROSE));
            }
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
            let parts: Vec<Part> = Part::ALL.iter().zip(self.tools.parts).filter(|(_, on)| *on).map(|(part, _)| *part).collect();
            let scope = self.tools.scope;
            self.run_reset(session, scope, &parts);
            self.tools.reset_open = false;
        }
        if close || !open {
            self.tools.reset_open = false;
        }
    }

    /// The card of the icon of the item, like the card of the icon of a species: the buttons on the left, the picture on the right.
    pub fn icon_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let own = crate::icons::ItemIcons::has_own(self.item);
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(widget::title("Icon"));
                    ui.add_space(6.0);
                    ui.add_enabled_ui(own, |ui| {
                        if widget::button(ui, "Export") {
                            self.export_icon();
                        }
                        if widget::button(ui, "Import") {
                            self.import_icon();
                        }
                    });
                    ui.add_enabled_ui(own && session.sd.is_some(), |ui| {
                        if widget::button(ui, "Reset") {
                            self.run_reset(session, Scope::Item, &[Part::Icons]);
                        }
                    });
                })
                .response
                .on_disabled_hover_text("This item has no icon of its own.");
                ui.add_space(12.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE_STRONG);
                if let Some(texture) = self.icons.texture(ui.ctx(), self.item) {
                    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(texture.id(), rect.shrink(3.0), full, egui::Color32::WHITE);
                }
            });
        });
    }

    /// Writes the icon of the item as a PNG file that the user chooses.
    fn export_icon(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name(format!("item_{}.png", self.item)).save_file() else { return };
        self.status = Some(match self.icons.export(self.item, &path) {
            Ok(()) => (true, format!("Exported the icon to {}.", path.display())),
            Err(message) => (false, message),
        });
    }

    /// Reads a PNG file of 30 × 30 pixels that the user chooses as the icon of the item. Save writes it.
    fn import_icon(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() else { return };
        self.status = Some(match self.icons.import(self.item, &path) {
            Ok(()) => (true, "Icon imported. Save writes it on the SD card.".to_string()),
            Err(message) => (false, message),
        });
    }

    /// The windows of the buttons Export… and Import…: the CSV files of all the items, for the parts that the user ticks.
    pub fn csv_windows(&mut self, ctx: &egui::Context, session: &Session) {
        for import in [false, true] {
            let shown = if import { self.tools.import_open } else { self.tools.export_open };
            if !shown {
                continue;
            }
            let mut open = true;
            let mut run = false;
            let mut close = false;
            let title = if import { "Import CSV files" } else { "Export CSV files" };
            egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(widget::hint("For all the items, not only the one that is open."));
                ui.add_space(6.0);
                for (place, (_, file, label)) in CSV_PARTS.iter().enumerate() {
                    ui.checkbox(&mut self.tools.csv_parts[place], format!("{label} ({file})"));
                }
                ui.add_space(6.0);
                ui.label(widget::hint(if import { "Choose the folder of the files: they are checked, then the changes go on the SD card." } else { "Choose a folder for the files." }));
                ui.horizontal(|ui| {
                    let any = self.tools.csv_parts.iter().any(|on| *on) && (!import || session.sd.is_some());
                    ui.add_enabled_ui(any, |ui| {
                        run = widget::button_styled(ui, egui::RichText::new(if import { "Import" } else { "Export" }).color(color::WHITE), Some(color::ACCENT));
                    });
                    close = widget::button(ui, "Cancel");
                });
            });
            if run {
                if import {
                    self.import_csv_files(session);
                } else {
                    self.export_csv_files(session);
                }
            }
            if run || close || !open {
                if import {
                    self.tools.import_open = false;
                } else {
                    self.tools.export_open = false;
                }
            }
        }
    }

    /// Writes the CSV files of the ticked parts in a folder that the user chooses.
    fn export_csv_files(&mut self, session: &Session) {
        let Some(folder) = rfd::FileDialog::new().set_title("Folder for the CSV files").pick_folder() else { return };
        let sd = session.sd.as_ref();
        let mut written = Vec::new();
        let mut result: Result<(), String> = Ok(());
        for (place, (part, file, _)) in CSV_PARTS.iter().enumerate() {
            if !self.tools.csv_parts[place] {
                continue;
            }
            let text = match part {
                Part::Data => {
                    let data = |item: usize| self.current(sd, item);
                    let mut names = |item: usize| self.index.as_ref().and_then(|i| i.names.get(item)).cloned().unwrap_or_default();
                    let pockets = crate::pockets::current(self.pocket_garc.as_ref(), sd);
                    csv::export(&data, &pockets, &mut names)
                }
                Part::Texts => {
                    let text = &self.texts;
                    let romfs = |language: usize, file: usize| text.romfs(language, file);
                    texts::export(&texts::Library { sd, romfs: &romfs })
                }
                // The pictures are PNG files, exported one by one: they are not in the CSV list.
                Part::Icons | Part::Reactions => continue,
            };
            let path = folder.join(file);
            if let Err(error) = std::fs::write(&path, &text) {
                result = Err(format!("{}: {error}", path.display()));
                break;
            }
            written.push(*file);
        }
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported {} for all the items to {}.", written.join(", "), folder.display())),
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
    fn import_one(&self, part: Part, text: &str, sd: &SdCard) -> Result<usize, String> {
        match part {
            Part::Data => {
                let rows = csv::parse(text)?;
                let data = |item: usize| self.current(Some(sd), item);
                let pockets = crate::pockets::current(self.pocket_garc.as_ref(), Some(sd));
                let written = csv::apply(&rows, &data, &pockets, sd)?;
                // A file that is the same as the game is not a change: it goes away.
                let romfs = |name: &str| name.parse::<usize>().ok().and_then(|item| self.garc.as_ref()?.first_file(item)).map(|bytes| bytes.to_vec());
                sango_core::modified::remove_unchanged(sd, crate::data::SD_FOLDER, &romfs)?;
                Ok(written)
            }
            Part::Texts => {
                let rows = texts::parse(text)?;
                let romfs = |language: usize, file: usize| self.texts.romfs(language, file);
                texts::apply(&rows, &texts::Library { sd: Some(sd), romfs: &romfs }, sd)
            }
            Part::Icons | Part::Reactions => Ok(0),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::Dump;

    /// With the dump of the game: a reset of one item reaches that item only, a reset of the texts keeps the data, and
    /// an undo gives the files back.
    #[test]
    fn the_resets_reach_the_scope_and_the_parts_and_can_be_undone() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let folder = std::env::temp_dir().join(format!("sango_item_tools_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Item::default();
        page.open(&dump);
        for item in [1, 2, 3] {
            sd.write(&data::sd_path(item), &[0; data::SIZE]).unwrap();
        }
        sd.write("game_text/en/114.bin", b"not a real file").unwrap();

        page.item = 2;
        page.run_reset(&session, Scope::Item, &[Part::Data]);
        assert!(sd.read(&data::sd_path(2)).is_none());
        assert!(sd.read(&data::sd_path(1)).is_some() && sd.read(&data::sd_path(3)).is_some());
        assert!(sd.read("game_text/en/114.bin").is_some(), "the texts are another part");
        assert_eq!(page.tools.history.len(), 1);

        page.run_reset(&session, Scope::All, &[Part::Data]);
        assert!(sd.files_in(data::SD_FOLDER).is_empty());

        page.undo(&session);
        assert_eq!(sd.files_in(data::SD_FOLDER).len(), 2, "the second reset is undone");
        page.undo(&session);
        assert_eq!(sd.files_in(data::SD_FOLDER).len(), 3, "the first reset is undone");
        assert!(page.tools.history.is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
