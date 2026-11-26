//! The tools that work on many files at once: the CSV file of the texts of all the abilities, the reset to the romfs, and the
//! undo.
//!
//! The files that the studio changes are on the SD card: the files of texts, in the folder `text`. A reset puts the romfs
//! text of an ability back in the files (a file that is the romfs file again is removed), or removes the files of the texts
//! of the abilities for all the abilities. Before a reset or an import, the tools keep a copy of the folder `text`: Undo puts
//! the files back.

use crate::{texts, Ability};
use sango_core::{Backup, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};

/// How many undo steps are kept.
const HISTORY: usize = 20;

/// How much a reset reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The chosen ability.
    Ability,
    /// All the abilities.
    All,
}

/// The state of the tools.
pub struct Tools {
    /// The copies of the files before an operation, the last one at the end.
    pub history: Vec<Backup>,
    pub reset_open: bool,
    pub scope: Scope,
    /// The parts of a reset: the texts, the reactions.
    pub reset_texts: bool,
    pub reset_reactions: bool,
    /// The windows of the buttons Export and Import.
    pub export_open: bool,
    pub import_open: bool,
}

impl Default for Tools {
    fn default() -> Self {
        Tools { history: Vec::new(), reset_open: false, scope: Scope::Ability, reset_texts: true, reset_reactions: false, export_open: false, import_open: false }
    }
}

/// Copies the files of texts.
fn snapshot(sd: &SdCard, label: String) -> Backup {
    Backup::snapshot(sd, &["game_text"], &[], label)
}

impl Ability {
    /// Reads again everything that the files of the SD card can change.
    fn files_changed(&mut self) {
        self.list_names = None;
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
        self.status = Some((true, format!("{done} {count} file(s) changed.")));
    }

    /// Puts the texts of an ability, or of all the abilities, back to the romfs.
    pub fn run_reset(&mut self, session: &Session, scope: Scope) {
        let Some(sd) = session.sd.as_ref() else {
            self.status = Some((false, "Choose the folder of the SD card first.".into()));
            return;
        };
        let place = match scope {
            Scope::Ability => format!("ability #{:03} {}", self.id, self.texts.name(Some(sd), self.id)),
            Scope::All => "all the abilities".to_string(),
        };
        let backup = snapshot(sd, format!("Reset the texts of {place}"));
        let result = match scope {
            Scope::Ability => {
                let text = &self.texts;
                texts::reset_ability(sd, self.id, &|language, file| text.romfs(language, file))
            }
            Scope::All => texts::reset_all(sd),
        };
        match result {
            Ok(()) => {
                let done = format!("Reset of the texts of {place}.");
                self.finish(backup, session, &done);
            }
            Err(message) => {
                self.finish(backup, session, "Reset stopped.");
                self.status = Some((false, message));
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

    /// Puts the reactions of an ability of the game, or of all of them, back to the code of the game: their files are deleted.
    fn reset_reactions(&mut self, session: &Session, scope: Scope) {
        let Some(sd) = session.sd.as_ref() else { return };
        let ids: Vec<usize> = match scope {
            Scope::Ability => vec![self.id],
            Scope::All => self.store.game_ids(),
        };
        let mut done = 0;
        let with_rules = self.store.game_ids();
        for id in ids.into_iter().filter(|id| with_rules.contains(id)) {
            if let Err(message) = self.store.delete(sd, id) {
                self.status = Some((false, message));
                return;
            }
            done += 1;
        }
        self.draft = None;
        self.status = Some((true, format!("{done} ability(ies) run the code of the game again.")));
    }

    /// The window of the reset: how much, and what (the texts, the reactions).
    pub fn reset_window(&mut self, ctx: &egui::Context, session: &Session) {
        if !self.tools.reset_open {
            return;
        }
        let mut open = true;
        let mut run = false;
        let mut close = false;
        let name = self.texts.name(session.sd.as_ref(), self.id);
        egui::Window::new("Reset to the romfs").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new("How much").strong());
            ui.radio_value(&mut self.tools.scope, Scope::Ability, format!("This ability: #{:03} {name}", self.id));
            ui.radio_value(&mut self.tools.scope, Scope::All, "All the abilities");
            ui.add_space(8.0);
            ui.label(egui::RichText::new("What").strong());
            ui.checkbox(&mut self.tools.reset_texts, "Texts");
            ui.label(widget::hint("The name and the description, in every language, go back to the texts of the romfs."));
            ui.checkbox(&mut self.tools.reset_reactions, "Reactions");
            ui.label(widget::hint("The rules that replace the code of the game are deleted: the game runs its own code again."));
            ui.add_space(8.0);
            ui.label(widget::hint("The changes that are not saved are lost."));
            if self.tools.scope == Scope::All {
                ui.label(egui::RichText::new("This reaches every ability.").color(color::ROSE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let any = self.tools.reset_texts || self.tools.reset_reactions;
                ui.add_enabled_ui(session.sd.is_some() && any, |ui| {
                    run = widget::button_styled(ui, egui::RichText::new("Reset").color(color::WHITE), Some(color::ROSE));
                });
                close = widget::button(ui, "Cancel");
            });
        });
        if run {
            let scope = self.tools.scope;
            if self.tools.reset_texts {
                self.run_reset(session, scope);
            }
            if self.tools.reset_reactions {
                self.reset_reactions(session, scope);
            }
            self.tools.reset_open = false;
        }
        if close || !open {
            self.tools.reset_open = false;
        }
    }

    /// The windows of the buttons Export and Import: the texts of all the abilities in a CSV file.
    pub fn csv_windows(&mut self, ctx: &egui::Context, session: &Session) {
        for import in [false, true] {
            let shown = if import { self.tools.import_open } else { self.tools.export_open };
            if !shown {
                continue;
            }
            let mut open = true;
            let mut run = false;
            let mut close = false;
            let title = if import { "Import CSV file" } else { "Export CSV file" };
            egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(widget::hint("For all the abilities, not only the one that is open."));
                ui.add_space(6.0);
                let mut texts = true;
                ui.add_enabled(false, egui::Checkbox::new(&mut texts, "Texts: the name and the description in the eight languages (ability_texts.csv)"));
                ui.add_space(6.0);
                ui.label(widget::hint(if import { "Choose the folder of the file: it is checked, then the changes go on the SD card." } else { "Choose a folder for the file." }));
                ui.horizontal(|ui| {
                    run = widget::button_styled(ui, egui::RichText::new(if import { "Import" } else { "Export" }).color(color::WHITE), Some(color::ACCENT));
                    close = widget::button(ui, "Cancel");
                });
            });
            if run {
                if import {
                    self.import_csv(session);
                } else {
                    self.export_csv(session);
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

    /// Writes the CSV file in a folder that the user chooses.
    fn export_csv(&mut self, session: &Session) {
        let Some(folder) = rfd::FileDialog::new().set_title("Folder for the CSV file").pick_folder() else { return };
        let text = &self.texts;
        let romfs = |language: usize, file: usize| text.romfs(language, file);
        let csv = texts::export(&texts::Library { sd: session.sd.as_ref(), romfs: &romfs });
        let path = folder.join("ability_texts.csv");
        self.status = Some(match std::fs::write(&path, csv) {
            Ok(()) => (true, format!("Exported ability_texts.csv for all the abilities to {}.", folder.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    /// Reads the CSV file from a folder that the user chooses, checks it, and writes the changes.
    fn import_csv(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let Some(folder) = rfd::FileDialog::new().set_title("Folder of the CSV file").pick_folder() else { return };
        let path = folder.join("ability_texts.csv");
        if !path.is_file() {
            self.status = Some((false, format!("There is no ability_texts.csv in {}.", folder.display())));
            return;
        }
        let backup = snapshot(sd, format!("Import of ability_texts.csv from {}", folder.display()));
        let text = &self.texts;
        let romfs = |language: usize, file: usize| text.romfs(language, file);
        let result = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display())).and_then(|text| texts::parse(&text)).and_then(|rows| texts::apply(&rows, &texts::Library { sd: Some(sd), romfs: &romfs }, sd));
        match result {
            Ok(count) => {
                let done = format!("Imported ability_texts.csv ({count} files).");
                self.finish(backup, session, &done);
            }
            Err(message) => self.status = Some((false, format!("ability_texts.csv: {message}"))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::Dump;

    /// With the dump of the game: a reset of one ability keeps the others, and an undo gives the files back.
    #[test]
    fn the_resets_reach_the_scope_and_can_be_undone() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let folder = std::env::temp_dir().join(format!("sango_ability_tools_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Ability::default();
        page.open(&dump);
        // Two names are changed in the English file of the names.
        let mut file = page.texts.romfs(texts::ENGLISH, texts::FIELDS[0].file).unwrap();
        file.set_text(1, "Smell").unwrap();
        file.set_text(2, "Rain").unwrap();
        sd.write(&sango_core::text::sd_path(texts::ENGLISH, texts::FIELDS[0].file), &file.to_bytes()).unwrap();

        page.id = 1;
        page.run_reset(&session, Scope::Ability);
        let after = sango_core::text::TextFile::from_bytes(&sd.read(&sango_core::text::sd_path(texts::ENGLISH, texts::FIELDS[0].file)).unwrap()).unwrap();
        assert_eq!(after.text(1).as_deref(), Some("Stench"));
        assert_eq!(after.text(2).as_deref(), Some("Rain"), "the other ability keeps its text");
        page.run_reset(&session, Scope::All);
        assert!(sd.files_in("game_text").is_empty());
        page.undo(&session);
        assert!(!sd.files_in("game_text").is_empty(), "the second reset is undone");
        page.undo(&session);
        let first = sango_core::text::TextFile::from_bytes(&sd.read(&sango_core::text::sd_path(texts::ENGLISH, texts::FIELDS[0].file)).unwrap()).unwrap();
        assert_eq!(first.text(1).as_deref(), Some("Smell"), "the first reset is undone");
        assert!(page.tools.history.is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
