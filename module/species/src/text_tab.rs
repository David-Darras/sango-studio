//! The Text tab: the texts of a species or a form, in each of the eight languages of the game.
//!
//! | Field | File of texts | Line |
//! | --- | --- | --- |
//! | Name | 98 | the number of the species |
//! | Category ("Seed Pokémon") | 2 | the number of the species |
//! | Pokédex entry of Alpha Sapphire | 6 | the number of the species |
//! | Form name | 5 | the place of the form in the Pokédex tables |
//! | Height and weight as the Pokédex shows them | 4 and 1 | the place of the form |
//!
//! The files of texts are the ones of `sango_core::text`. A file that the user changed goes on the SD card
//! with the layout of the romfs. A text is written in the plain form of that module: the letters, `[tag ...]`
//! for the names and numbers that the game fills in, and `[char XXXX]` for a character that has no usual form.

use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::{HashMap, HashSet};

/// Which line of a file a field uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Line {
    /// The number of the species.
    Species,
    /// The place of the form in the Pokédex tables.
    Place,
}

/// A field of the tab.
struct Field {
    label: &'static str,
    /// The number of the file of texts.
    file: usize,
    line: Line,
    /// The number of lines that the text box shows.
    rows: usize,
    hint: &'static str,
}

const FIELDS: [Field; 6] = [
    Field { label: "Name", file: 98, line: Line::Species, rows: 1, hint: "" },
    Field { label: "Category", file: 2, line: Line::Species, rows: 1, hint: "The Pokédex shows it with the word Pokémon, as in Seed Pokémon." },
    Field { label: "Pokédex entry", file: 6, line: Line::Species, rows: 5, hint: "The text that Alpha Sapphire shows. A line break is a line break of the text." },
    Field { label: "Form name", file: 5, line: Line::Place, rows: 1, hint: "The name that the Pokédex gives to this form. The first form has none." },
    Field { label: "Height", file: 4, line: Line::Place, rows: 1, hint: "The height as the Pokédex writes it. It does not change with the height of the data." },
    Field { label: "Weight", file: 1, line: Line::Place, rows: 1, hint: "The weight as the Pokédex writes it. It does not change with the weight of the data." },
];

/// The state of the Text tab.
#[derive(Default)]
pub struct TextTab {
    archives: Vec<Option<Garc>>,
    /// The files that were read, as the user changed them. The key is the language and the file.
    files: HashMap<(usize, usize), TextFile>,
    /// The files as they were read or saved: to know what changed.
    saved: HashMap<(usize, usize), TextFile>,
    /// The files that came from the SD card.
    from_card: HashSet<(usize, usize)>,
    /// The text of each box that was touched: the language, the file and the line.
    buffers: HashMap<(usize, usize, usize), String>,
    /// What is wrong in a box: the same key.
    errors: HashMap<(usize, usize, usize), String>,
    /// The species and the form that the boxes are for.
    current: Option<(usize, usize)>,
    /// The chosen language (0 to 7).
    language: usize,
}

impl TextTab {
    /// Opens the archives of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archives = LANGUAGES.iter().map(|language| Garc::open(&dump.archive(language.archive)).ok()).collect();
        self.files.clear();
        self.saved.clear();
        self.from_card.clear();
        self.buffers.clear();
        self.errors.clear();
        self.current = None;
    }

    /// Reads a file of texts if it is not read yet: the SD card first, then the romfs.
    fn load(&mut self, sd: Option<&SdCard>, language: usize, file: usize) {
        if self.files.contains_key(&(language, file)) {
            return;
        }
        let on_card = sd.and_then(|sd| sd.read(&text::sd_path(language, file))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        let from_card = on_card.is_some();
        let found = on_card.or_else(|| self.romfs(language, file));
        if let Some(found) = found {
            if from_card {
                self.from_card.insert((language, file));
            }
            self.saved.insert((language, file), found.clone());
            self.files.insert((language, file), found);
        }
    }

    /// A file of texts as the romfs has it.
    fn romfs(&self, language: usize, file: usize) -> Option<TextFile> {
        self.archives.get(language)?.as_ref()?.first_file(file).and_then(|bytes| TextFile::from_bytes(bytes).ok())
    }

    /// True when a file of texts changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.files.iter().any(|(key, file)| self.saved.get(key) != Some(file))
    }

    /// True when a file of texts that was read comes from the SD card.
    pub fn from_card(&self) -> bool {
        !self.from_card.is_empty()
    }

    /// Writes the files that changed on the SD card, and tells the game. Returns the paths that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<Vec<String>, String> {
        let mut changed: Vec<(usize, usize)> = self.files.iter().filter(|(key, file)| self.saved.get(*key) != Some(*file)).map(|(key, _)| *key).collect();
        changed.sort_unstable();
        let mut written = Vec::new();
        for key in changed {
            let path = text::sd_path(key.0, key.1);
            sd.write(&path, &self.files[&key].to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            self.saved.insert(key, self.files[&key].clone());
            self.from_card.insert(key);
            written.push(path);
        }
        Ok(written)
    }

    /// The line of a field.
    fn line_of(field: &Field, species: usize, place: usize) -> usize {
        match field.line {
            Line::Species => species,
            Line::Place => place,
        }
    }

    /// Takes the texts of the species and the form from the romfs again, in all the languages that were read.
    /// Save writes them.
    pub fn reset(&mut self, species: usize, place: usize) {
        for field in &FIELDS {
            let line = Self::line_of(field, species, place);
            for language in 0..LANGUAGES.len() {
                let original = self.romfs(language, field.file).and_then(|file| file.text(line));
                if let (Some(original), Some(file)) = (original, self.files.get_mut(&(language, field.file))) {
                    let _ = file.set_text(line, &original);
                }
            }
        }
        self.buffers.clear();
        self.errors.clear();
    }

    /// Draws the tab. `place` is the place of the form in the Pokédex tables.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &Session, species: usize, form: usize, place: usize) {
        if self.current != Some((species, form)) {
            self.current = Some((species, form));
            self.buffers.clear();
            self.errors.clear();
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (index, language) in LANGUAGES.iter().enumerate() {
                self.language = tab(ui, index, language.name, self.language);
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("text_tab").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_width(760.0);
                for field in &FIELDS {
                    self.field(ui, session.sd.as_ref(), field, species, place);
                    ui.add_space(10.0);
                }
            });
        });
    }

    /// One field: its name, its text box, and what is wrong with the text.
    fn field(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, field: &Field, species: usize, place: usize) {
        let language = self.language;
        let line = Self::line_of(field, species, place);
        self.load(sd, language, field.file);
        let key = (language, field.file, line);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(field.label).strong());
            let edited = match (self.files.get(&(language, field.file)), self.saved.get(&(language, field.file))) {
                (Some(now), Some(before)) => now.text(line) != before.text(line),
                _ => false,
            };
            if edited {
                widget::badge(ui, "Not saved", color::AMBER);
            }
        });
        if !field.hint.is_empty() {
            ui.label(widget::hint(field.hint));
        }
        let Some(file) = self.files.get_mut(&(language, field.file)) else {
            ui.label(widget::hint("This text is not in the files that were read."));
            return;
        };
        let Some(original) = file.text(line) else {
            ui.label(widget::hint("This form has no line in this file."));
            return;
        };
        let buffer = self.buffers.entry(key).or_insert(original);
        let edit = if field.rows == 1 { egui::TextEdit::singleline(buffer) } else { egui::TextEdit::multiline(buffer).desired_rows(field.rows) };
        let response = ui.add(edit.id_salt(key).desired_width(f32::INFINITY));
        if response.changed() {
            match file.set_text(line, buffer) {
                Ok(()) => {
                    self.errors.remove(&key);
                }
                Err(message) => {
                    self.errors.insert(key, message);
                }
            }
        }
        if let Some(message) = self.errors.get(&key) {
            ui.label(egui::RichText::new(format!("{message} The change is not kept until the text is right.")).color(color::ROSE).size(13.0));
        }
    }
}

/// A tab button of the languages. Returns the chosen place.
fn tab(ui: &mut egui::Ui, place: usize, text: &str, chosen: usize) -> usize {
    let id = egui::Id::new(("text_language", place));
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

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the texts of Bulbasaur are read in every language, one is changed, saved on
    /// the SD card and read again, and a text with a wrong tag is not kept.
    #[test]
    fn the_texts_of_bulbasaur_are_read_changed_and_saved() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_text_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let dump = Dump::open(&root).unwrap();
        let session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut tab = TextTab::default();
        tab.open(&dump);

        let ctx = egui::Context::default();
        let frame = |tab: &mut TextTab, species: usize, form: usize, place: usize| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0)));
            let _ = ctx.run(input, |ctx| {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), egui::Id::new(("text_language", tab.language))));
                egui::CentralPanel::default().show(ctx, |ui| tab.ui(ui, &session, species, form, place));
            });
        };
        // Every language draws, and the names of Bulbapedia are in the files.
        for language in 0..LANGUAGES.len() {
            tab.language = language;
            frame(&mut tab, 1, 0, 1);
        }
        let name = |tab: &TextTab, language: usize| tab.files[&(language, 98)].text(1).unwrap();
        assert_eq!((name(&tab, 2), name(&tab, 3), name(&tab, 5)), ("Bulbasaur".to_string(), "Bulbizarre".to_string(), "Bisasam".to_string()));
        assert!(tab.files[&(2, 6)].text(1).unwrap().starts_with("Bulbasaur can be seen napping"));
        assert!(!tab.changed() && !tab.from_card());

        // The form name of Hoopa Unbound (place 912) is in the file 5.
        tab.language = 2;
        frame(&mut tab, 720, 1, 912);
        assert_eq!(tab.files[&(2, 5)].text(912).as_deref(), Some("Hoopa Unbound"));

        // A text is changed through its box: the buffer is typed in, the file follows.
        frame(&mut tab, 1, 0, 1);
        tab.files.get_mut(&(3, 98)).unwrap().set_text(1, "Bulbizarre Nouveau").unwrap();
        assert!(tab.changed());
        // A wrong tag is not kept: the box keeps its text, the file keeps the last right text.
        assert!(tab.files.get_mut(&(3, 98)).unwrap().set_text(1, "[tag").is_err());
        assert_eq!(tab.files[&(3, 98)].text(1).as_deref(), Some("Bulbizarre Nouveau"));

        // Only the file that changed is written, and it comes back.
        let written = tab.save(&sd).unwrap();
        assert_eq!(written, ["text/fr/98.bin"]);
        assert!(!tab.changed() && tab.from_card());
        assert_eq!(tab.save(&sd).unwrap(), Vec::<String>::new());
        let mut again = TextTab::default();
        again.open(&dump);
        again.load(Some(&sd), 3, 98);
        again.load(Some(&sd), 2, 98);
        assert_eq!(again.files[&(3, 98)].text(1).as_deref(), Some("Bulbizarre Nouveau"));
        assert_eq!(again.files[&(2, 98)].text(1).as_deref(), Some("Bulbasaur"));
        assert!(again.from_card());

        // Reset gives back the romfs for the species.
        again.reset(1, 1);
        assert_eq!(again.files[&(3, 98)].text(1).as_deref(), Some("Bulbizarre"));
        assert!(again.changed());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
