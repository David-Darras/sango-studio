//! The tools that work on many files at once: the CSV files of the types, the reset to the romfs, and the undo.
//!
//! The files that the studio changes are on the SD card. A reset removes the files of the SD card (the game then reads the
//! romfs and the plugin uses the chart of the game). Before a reset or an import, the tools keep a copy of the files of the
//! parts that the operation touches: Undo puts the files back.
//!
//! | Part | Files of the SD card |
//! | --- | --- |
//! | Chart | `type/chart.bin` |
//! | Types | `type/types.csv`, the folder `type/icon` |
//! | Names | the file of the names of the types, `text/<language>/18.bin` |

use crate::{chart, icons, texts, types, Type};
use sango_core::{Backup, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::path::Path;

/// How many undo steps are kept.
const HISTORY: usize = 20;

/// Where the chart is on the SD card.
pub const CHART_PATH: &str = "type/chart.bin";

/// A kind of data that has its own files and its own tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Chart,
    Types,
    Names,
}

impl Part {
    const ALL: [Part; 3] = [Part::Chart, Part::Types, Part::Names];

    fn label(self) -> &'static str {
        match self {
            Part::Chart => "Chart",
            Part::Types => "Types",
            Part::Names => "Names",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Part::Chart => "General tab: the effect of each type against each type.",
            Part::Types => "Text tab: the new types and the icons of all the types, in every language.",
            Part::Names => "Text tab: the names of the types of the game, in every language.",
        }
    }

    /// The files of the SD card that the part uses: the folders, and the single files.
    fn files(self) -> (Vec<&'static str>, Vec<String>) {
        match self {
            Part::Chart => (Vec::new(), vec![CHART_PATH.to_string()]),
            Part::Types => (vec![icons::SD_FOLDER], vec![types::SD_PATH.to_string()]),
            Part::Names => (Vec::new(), (0..sango_core::text::LANGUAGES.len()).map(|l| sango_core::text::sd_path(l, texts::FILE)).collect()),
        }
    }
}

/// The parts of the page that a tab shows: the chart in the General tab, the new types, their icons and the names in the Text
/// tab. The CSV tab shows no part.
pub fn tab_parts(tab: usize) -> Vec<Part> {
    match tab {
        0 => vec![Part::Chart],
        1 => vec![Part::Types, Part::Names],
        _ => Vec::new(),
    }
}

/// The state of the tools.
pub struct Tools {
    /// The copies for Undo, the last one at the end.
    pub history: Vec<Backup>,
    pub reset_open: bool,
    /// The parts of the reset window: Chart, Types, Names.
    pub parts: [bool; 3],
    /// The parts of the CSV files: Types, Chart, Names.
    pub csv_parts: [bool; 3],
}

impl Default for Tools {
    fn default() -> Self {
        Tools { history: Vec::new(), reset_open: false, parts: [false; 3], csv_parts: [true; 3] }
    }
}

/// The CSV parts: the part, the name of the file, and the label. The types come first: the chart needs them.
const CSV_PARTS: [(Part, &str, &str); 3] = [(Part::Types, "types.csv", "Types"), (Part::Chart, "type_chart.csv", "Chart"), (Part::Names, "type_texts.csv", "Names")];

/// Copies the files of some parts.
fn snapshot(sd: &SdCard, parts: &[Part], label: String) -> Backup {
    let mut folders = Vec::new();
    let mut loose = Vec::new();
    for part in parts {
        let (f, l) = part.files();
        folders.extend(f);
        loose.extend(l);
    }
    let loose: Vec<&str> = loose.iter().map(|s| s.as_str()).collect();
    Backup::snapshot(sd, &folders, &loose, label)
}

impl Type {
    /// Reads again everything that the files of the SD card can change.
    fn files_changed(&mut self) {
        self.loaded = false;
        self.texts.reload();
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

    /// Puts the files of some parts back to the romfs.
    pub fn run_reset(&mut self, session: &Session, parts: &[Part]) {
        let Some(sd) = session.sd.as_ref() else {
            self.status = Some((false, "Choose the folder of the SD card first.".into()));
            return;
        };
        if parts.is_empty() {
            return;
        }
        let names: Vec<&str> = parts.iter().map(|part| part.label()).collect();
        let backup = snapshot(sd, parts, format!("Reset {} of the types", names.join(", ")));
        let result = (|| -> Result<(), String> {
            for part in parts {
                let (folders, loose) = part.files();
                for folder in folders {
                    for path in sd.files_in(folder) {
                        sd.remove(&path)?;
                    }
                }
                for path in loose {
                    sd.remove(&path)?;
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                let done = format!("Reset of {}.", names.join(", "));
                self.finish(backup, session, &done);
            }
            Err(message) => {
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

    /// The window of the reset: the parts.
    pub fn reset_window(&mut self, ctx: &egui::Context, session: &Session) {
        if !self.tools.reset_open {
            return;
        }
        let mut open = true;
        let mut run = false;
        let mut close = false;
        egui::Window::new("Reset to the romfs").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new("What").strong());
            for (place, part) in Part::ALL.iter().enumerate() {
                widget::chip(ui, part.label(), &mut self.tools.parts[place]).on_hover_text(part.hint());
                ui.label(widget::hint(part.hint()));
            }
            ui.add_space(8.0);
            ui.label(widget::hint("The files of the SD card go back to the data of the romfs and to the chart of the game. The changes that are not saved are lost. Undo gives the files back."));
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
            self.run_reset(session, &parts);
            self.tools.reset_open = false;
        }
        if close || !open {
            self.tools.reset_open = false;
        }
    }

    /// The card of the CSV files in the General tab.
    pub fn csv_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        widget::card(ui, |ui| {
            ui.label(widget::title("CSV files"));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (place, (_, file, label)) in CSV_PARTS.iter().enumerate() {
                    widget::chip(ui, label, &mut self.tools.csv_parts[place]).on_hover_text(*file);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let any = self.tools.csv_parts.iter().any(|on| *on);
                ui.add_enabled_ui(any, |ui| {
                    if widget::button(ui, "Export CSV…") {
                        self.export_csv_files(session);
                    }
                    if session.sd.is_some() && widget::button(ui, "Import CSV…") {
                        self.import_csv_files(session);
                    }
                });
            });
            ui.label(widget::hint("Export writes one file for each ticked part in a folder that you choose: types.csv (the new types), type_chart.csv (the chart: one row for each attacking type, with 0, 0.5, 1 or 2) and type_texts.csv (the names of the types of the game, in the eight languages). Import reads the files with these names from a folder, checks all of them, and writes the changes on the SD card. Undo gives the files back."));
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
                Part::Types => self.list.to_csv(),
                Part::Chart => self.chart.to_csv(&self.list.ids()),
                Part::Names => {
                    let text = &self.texts;
                    let romfs = |language: usize, file: usize| text.romfs(language, file);
                    texts::export(&texts::Library { sd, romfs: &romfs })
                }
            };
            let path = folder.join(file);
            if let Err(error) = std::fs::write(&path, &text) {
                result = Err(format!("{}: {error}", path.display()));
                break;
            }
            written.push(*file);
        }
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported {} to {}.", written.join(", "), folder.display())),
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
            Part::Types => {
                let list = types::TypeList::from_csv(text)?;
                sd.write(types::SD_PATH, list.to_csv().as_bytes()).and_then(|_| sd.mark_changed(types::SD_PATH))?;
                // The chart of a type that is not in the list any more goes back to ×1.
                let mut chart = sd.read(CHART_PATH).and_then(|b| chart::Chart::from_bytes(&b).ok()).unwrap_or_else(chart::Chart::original);
                let before = chart.clone();
                for removed in self.list.custom.iter().filter(|c| list.find(c.id).is_none()) {
                    chart.clear_type(removed.id);
                }
                if chart != before {
                    sd.write(CHART_PATH, &chart.to_bytes()).and_then(|_| sd.mark_changed(CHART_PATH))?;
                }
                Ok(1)
            }
            Part::Chart => {
                // The types of the SD card (the file that was just imported, when it is ticked) tell which types exist.
                let list = sd.read(types::SD_PATH).and_then(|b| String::from_utf8(b).ok()).and_then(|t| types::TypeList::from_csv(&t).ok()).unwrap_or_default();
                let base = sd.read(CHART_PATH).and_then(|b| chart::Chart::from_bytes(&b).ok()).unwrap_or_else(chart::Chart::original);
                let next = base.with_csv(text, &list.ids())?;
                if next == base {
                    return Ok(0);
                }
                sd.write(CHART_PATH, &next.to_bytes()).and_then(|_| sd.mark_changed(CHART_PATH))?;
                Ok(1)
            }
            Part::Names => {
                let rows = texts::parse(text)?;
                let romfs = |language: usize, file: usize| self.texts.romfs(language, file);
                texts::apply(&rows, &texts::Library { sd: Some(sd), romfs: &romfs }, sd)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::Dump;

    /// With the dump of the game: a reset reaches the parts that are asked, and an undo gives the files back.
    #[test]
    fn the_resets_reach_the_parts_and_can_be_undone() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let folder = std::env::temp_dir().join(format!("sango_type_tools_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Type::default();
        page.open(&dump);
        let mut changed = chart::Chart::original();
        changed.set(9, 11, chart::HALF);
        sd.write(CHART_PATH, &changed.to_bytes()).unwrap();
        sd.write(types::SD_PATH, b"id,name,colour_type\n19,Cosmic,15\n").unwrap();
        sd.write("type/icon/19.bclim", b"x").unwrap();

        page.run_reset(&session, &[Part::Chart]);
        assert!(sd.read(CHART_PATH).is_none());
        assert!(sd.read(types::SD_PATH).is_some(), "the types are another part");
        page.run_reset(&session, &[Part::Types]);
        assert!(sd.read(types::SD_PATH).is_none() && sd.read("type/icon/19.bclim").is_none());
        page.undo(&session);
        assert!(sd.read(types::SD_PATH).is_some() && sd.read("type/icon/19.bclim").is_some());
        page.undo(&session);
        assert_eq!(chart::Chart::from_bytes(&sd.read(CHART_PATH).unwrap()).unwrap(), changed);
        assert!(page.tools.history.is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
