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

/// A kind of data that has its own files and its own tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Data,
    Texts,
    Icons,
}

impl Part {
    const ALL: [Part; 3] = [Part::Data, Part::Texts, Part::Icons];

    fn label(self) -> &'static str {
        match self {
            Part::Data => "Data",
            Part::Texts => "Texts",
            Part::Icons => "Icon",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Part::Data => "General tab: price, pocket, held effect, effects on a Pokémon and the other fields.",
            Part::Texts => "Text tab: name, name in messages and description, in every language.",
            Part::Icons => "The picture of the item (the icon card of the General tab).",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Part::Data => data::SD_FOLDER,
            Part::Texts => "text",
            Part::Icons => crate::icons::SD_FOLDER,
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
    /// The parts of the reset window: Data, Texts, Icons.
    pub parts: [bool; 3],
    /// The parts of the CSV files: Data, Texts.
    pub csv_parts: [bool; 2],
}

impl Default for Tools {
    fn default() -> Self {
        Tools { history: Vec::new(), reset_open: false, scope: Scope::Item, parts: [false; 3], csv_parts: [true; 2] }
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
        self.status = Some((true, format!("{done} {count} file(s) changed. Undo gives them back.")));
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
        let result = (|| -> Result<(), String> {
            for part in parts {
                match (part, scope) {
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
        })();
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
            ui.label(widget::hint("The files of the SD card go back to the data of the romfs. The changes that are not saved are lost. Undo gives the files back."));
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

    /// The card of the icon of the item in the General tab: the picture, and the buttons to export or import it.
    pub fn icon_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE_STRONG);
                if let Some(texture) = self.icons.texture(ui.ctx(), self.item) {
                    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(texture.id(), rect.shrink(3.0), full, egui::Color32::WHITE);
                }
                ui.vertical(|ui| {
                    ui.label(widget::title("Icon"));
                    let own = crate::icons::ItemIcons::has_own(self.item);
                    if own {
                        ui.label(widget::hint("30 × 30 pixels, in the Bag and the shops."));
                    } else {
                        ui.label(widget::hint("This item has no icon of its own."));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(own, |ui| {
                            if widget::button(ui, "Export PNG…") {
                                self.export_icon();
                            }
                            if widget::button(ui, "Import PNG…") {
                                self.import_icon();
                            }
                        });
                        ui.add_enabled_ui(own && session.sd.is_some(), |ui| {
                            if widget::button(ui, "Reset icon") {
                                self.run_reset(session, Scope::Item, &[Part::Icons]);
                            }
                        });
                    });
                });
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

    /// The card of the CSV files in the General tab: all the items, whatever the item that is open.
    pub fn csv_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        widget::card(ui, |ui| {
            ui.label(widget::title("CSV files of all the items"));
            ui.label(widget::hint("These buttons work on all the items, not only on the one that is open."));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (place, (_, file, label)) in CSV_PARTS.iter().enumerate() {
                    widget::chip(ui, label, &mut self.tools.csv_parts[place]).on_hover_text(*file);
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
            ui.label(widget::hint("Export writes one file for each ticked part in a folder that you choose: items.csv (the data) and item_texts.csv (the name, the name in messages and the description, in the eight languages). Import reads the files with these names from a folder, checks all of them, and writes the changes on the SD card. Undo gives the files back."));
        });
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
                Part::Icons => continue,
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
                csv::apply(&rows, &data, &pockets, sd)
            }
            Part::Texts => {
                let rows = texts::parse(text)?;
                let romfs = |language: usize, file: usize| self.texts.romfs(language, file);
                texts::apply(&rows, &texts::Library { sd: Some(sd), romfs: &romfs }, sd)
            }
            Part::Icons => Ok(0),
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
        sd.write("text/en/114.bin", b"not a real file").unwrap();

        page.item = 2;
        page.run_reset(&session, Scope::Item, &[Part::Data]);
        assert!(sd.read(&data::sd_path(2)).is_none());
        assert!(sd.read(&data::sd_path(1)).is_some() && sd.read(&data::sd_path(3)).is_some());
        assert!(sd.read("text/en/114.bin").is_some(), "the texts are another part");
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
