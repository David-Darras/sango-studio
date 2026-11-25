//! The texts of a species or a form, in each of the eight languages of the game. The General tab shows them in a compact card (see
//! [`TextTab::compact_ui`]); the CSV tab imports and exports all of them.
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

/// The place of English in `LANGUAGES`: the language that the card shows first.
const ENGLISH: usize = 2;

/// Which line of a file a field uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Line {
    /// The number of the species.
    Species,
    /// The place of the form in the Pokédex tables.
    Place,
}

/// A field of the tab.
pub(crate) struct Field {
    pub(crate) label: &'static str,
    /// The name of the field in the CSV file.
    pub(crate) key: &'static str,
    /// The number of the file of texts.
    pub(crate) file: usize,
    pub(crate) line: Line,
    /// The number of lines of the text: 1 for a line, more for a text box.
    rows: usize,
    hint: &'static str,
}

pub(crate) const FIELDS: [Field; 6] = [
    Field { label: "Name", key: "name", file: 98, line: Line::Species, rows: 1, hint: "" },
    Field { label: "Category", key: "category", file: 2, line: Line::Species, rows: 1, hint: "The Pokédex shows it with the word Pokémon, as in Seed Pokémon." },
    Field { label: "Description", key: "entry", file: 6, line: Line::Species, rows: 5, hint: "The text that Alpha Sapphire shows. A line break is a line break of the text." },
    Field { label: "Form name", key: "form_name", file: 5, line: Line::Place, rows: 1, hint: "The name that the Pokédex gives to this form. The first form has none." },
    Field { label: "Height", key: "height", file: 4, line: Line::Place, rows: 1, hint: "The height as the Pokédex writes it. It does not change with the height of the data." },
    Field { label: "Weight", key: "weight", file: 1, line: Line::Place, rows: 1, hint: "The weight as the Pokédex writes it. It does not change with the weight of the data." },
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
        self.language = ENGLISH;
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
    pub(crate) fn romfs(&self, language: usize, file: usize) -> Option<TextFile> {
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
    pub(crate) fn line_of(field: &Field, species: usize, place: usize) -> usize {
        match field.line {
            Line::Species => species,
            Line::Place => place,
        }
    }

    /// Forgets the texts that were read and the changes that were not saved: the next display reads the files again.
    pub fn reload(&mut self) {
        self.files.clear();
        self.saved.clear();
        self.from_card.clear();
        self.buffers.clear();
        self.errors.clear();
    }

    /// The compact card of the General tab: a menu of the language, then the name of the species, the category and the Pokédex
    /// entry. `place` is the place of the form in the Pokédex tables.
    pub fn compact_ui(&mut self, ui: &mut egui::Ui, session: &Session, species: usize, form: usize, place: usize) {
        if self.current != Some((species, form)) {
            self.current = Some((species, form));
            self.buffers.clear();
            self.errors.clear();
        }
        let sd = session.sd.as_ref();
        // The name of the species, for every form: the form name stays in the CSV files.
        let fields = [&FIELDS[0], &FIELDS[1], &FIELDS[2]];
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title("Texts"));
                if self.changed() {
                    widget::badge(ui, "Not saved", color::AMBER);
                }
            });
            egui::Grid::new("species_texts").num_columns(2).spacing(egui::vec2(8.0, 6.0)).show(ui, |ui| {
                ui.label("Language");
                let chosen = LANGUAGES.get(self.language).map_or("?", |l| l.name);
                egui::ComboBox::from_id_salt("species_text_language").selected_text(chosen).show_ui(ui, |ui| {
                    // English first, then the other languages in the order of the game.
                    let order = std::iter::once(ENGLISH).chain((0..LANGUAGES.len()).filter(|i| *i != ENGLISH));
                    for (index, language) in order.map(|i| (i, &LANGUAGES[i])) {
                        ui.selectable_value(&mut self.language, index, language.name);
                    }
                });
                ui.end_row();
                for field in fields {
                    ui.label(field.label).on_hover_text(field.hint);
                    ui.vertical(|ui| self.edit(ui, sd, field, species, place));
                    ui.end_row();
                }
            });
        });
    }

    /// The text box of a field in the chosen language, and what is wrong with the text.
    fn edit(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, field: &Field, species: usize, place: usize) {
        let language = self.language;
        let line = Self::line_of(field, species, place);
        self.load(sd, language, field.file);
        let key = (language, field.file, line);
        let Some(file) = self.files.get_mut(&(language, field.file)) else {
            ui.label(widget::hint("This text is not in the files that were read."));
            return;
        };
        let Some(original) = file.text(line) else {
            ui.label(widget::hint("This form has no line in this file."));
            return;
        };
        let buffer = self.buffers.entry(key).or_insert(original);
        let response = sango_ui::game_text::edit(ui, key, buffer, if field.rows == 1 { 1 } else { 3 }, 300.0);
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
                egui::CentralPanel::default().show(ctx, |ui| tab.compact_ui(ui, &session, species, form, place));
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

        // A form shows the name of its species: Hoopa Unbound shows "Hoopa", and its form name (place 912, file 5) stays for the CSV.
        tab.language = 2;
        frame(&mut tab, 720, 1, 912);
        assert_eq!(tab.files[&(2, 98)].text(720).as_deref(), Some("Hoopa"));
        assert!(!tab.files.contains_key(&(2, 5)), "the card does not read the form names");

        // A text is changed through its box: the buffer is typed in, the file follows.
        frame(&mut tab, 1, 0, 1);
        tab.files.get_mut(&(3, 98)).unwrap().set_text(1, "Bulbizarre Nouveau").unwrap();
        assert!(tab.changed());
        // A wrong tag is not kept: the box keeps its text, the file keeps the last right text.
        assert!(tab.files.get_mut(&(3, 98)).unwrap().set_text(1, "[tag").is_err());
        assert_eq!(tab.files[&(3, 98)].text(1).as_deref(), Some("Bulbizarre Nouveau"));

        // Only the file that changed is written, and it comes back.
        let written = tab.save(&sd).unwrap();
        assert_eq!(written, ["game_text/fr/98.bin"]);
        assert!(!tab.changed() && tab.from_card());
        assert_eq!(tab.save(&sd).unwrap(), Vec::<String>::new());
        let mut again = TextTab::default();
        again.open(&dump);
        again.load(Some(&sd), 3, 98);
        again.load(Some(&sd), 2, 98);
        assert_eq!(again.files[&(3, 98)].text(1).as_deref(), Some("Bulbizarre Nouveau"));
        assert_eq!(again.files[&(2, 98)].text(1).as_deref(), Some("Bulbasaur"));
        assert!(again.from_card());

        // Reload forgets the files that were read.
        again.reload();
        assert!(again.files.is_empty() && !again.changed() && !again.from_card());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
