//! The names of the 18 types of the game, in each of the eight languages.
//!
//! They are the lines 0 to 17 of the file of texts 18: the line is the number of the type. A file that the user changed goes
//! on the SD card with the layout of the romfs, in the folder `text`. The names of the new types, in every language, are in the
//! list of the types (see `types.rs`): the Text tab edits them too.

use sango_core::csv_text::{quote, records};
use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::{HashMap, HashSet};

/// The file of texts with the names of the types.
pub const FILE: usize = 18;

/// The number of lines that the page edits: the 18 types of the game.
pub const COUNT: usize = crate::chart::GAME_TYPES;

/// The language of the names in the lists of the page: English.
pub const ENGLISH: usize = 2;

/// The header line of the CSV file of the texts.
pub const HEADER: &str = "type,language,text";

/// The state of the Text tab.
#[derive(Default)]
pub struct TypeTexts {
    archives: Vec<Option<Garc>>,
    /// The files that were read, as the user changed them, for each language.
    files: HashMap<usize, TextFile>,
    /// The files as they were read or saved: to know what changed.
    saved: HashMap<usize, TextFile>,
    /// The files that came from the SD card.
    from_card: HashSet<usize>,
    /// The text of each box that was touched: the language and the line.
    buffers: HashMap<(usize, usize), String>,
    /// What is wrong in a box: the same key.
    errors: HashMap<(usize, usize), String>,
    /// The chosen language (0 to 7).
    language: usize,
}

impl TypeTexts {
    /// Opens the archives of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archives = LANGUAGES.iter().map(|language| Garc::open(&dump.archive(language.archive)).ok()).collect();
        self.reload();
    }

    /// Forgets the texts that were read and the changes that were not saved: the next display reads the files again.
    pub fn reload(&mut self) {
        self.files.clear();
        self.saved.clear();
        self.from_card.clear();
        self.buffers.clear();
        self.errors.clear();
    }

    /// The file of texts as the romfs has it.
    pub fn romfs(&self, language: usize, file: usize) -> Option<TextFile> {
        self.archives.get(language)?.as_ref()?.first_file(file).and_then(|bytes| TextFile::from_bytes(bytes).ok())
    }

    /// Reads the file of a language if it is not read yet: the SD card first, then the romfs.
    fn load(&mut self, sd: Option<&SdCard>, language: usize) {
        if self.files.contains_key(&language) {
            return;
        }
        let on_card = sd.and_then(|sd| sd.read(&text::sd_path(language, FILE))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        let from_card = on_card.is_some();
        if let Some(found) = on_card.or_else(|| self.romfs(language, FILE)) {
            if from_card {
                self.from_card.insert(language);
            }
            self.saved.insert(language, found.clone());
            self.files.insert(language, found);
        }
    }

    /// The English name of a type of the game, as the user has it now.
    pub fn name(&mut self, sd: Option<&SdCard>, kind: usize) -> String {
        self.load(sd, ENGLISH);
        self.files.get(&ENGLISH).and_then(|file| file.text(kind)).unwrap_or_default()
    }

    /// True when a file changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.files.iter().any(|(key, file)| self.saved.get(key) != Some(file))
    }

    /// True when a file that was read comes from the SD card.
    pub fn from_card(&self) -> bool {
        !self.from_card.is_empty()
    }

    /// Writes the files that changed on the SD card, and tells the game.
    pub fn save(&mut self, sd: &SdCard) -> Result<Vec<String>, String> {
        let mut changed: Vec<usize> = self.files.iter().filter(|(key, file)| self.saved.get(*key) != Some(*file)).map(|(key, _)| *key).collect();
        changed.sort_unstable();
        let mut written = Vec::new();
        for language in changed {
            let path = text::sd_path(language, FILE);
            sd.write(&path, &self.files[&language].to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            self.saved.insert(language, self.files[&language].clone());
            self.from_card.insert(language);
            written.push(path);
        }
        Ok(written)
    }

    /// Draws the tab: the languages, and for each type its icon in this language and a box for its name. `row` draws what belongs
    /// to a type in this language: it gets the type, the language and `false` to draw the icon before the box, `true` to draw the
    /// buttons of the icon after it. Gives the number of a new type that the user removed.
    pub fn ui(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, custom: &mut crate::types::TypeList, row: &mut dyn FnMut(&mut egui::Ui, usize, usize, bool)) -> Option<usize> {
        let mut removed = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (index, language) in LANGUAGES.iter().enumerate() {
                self.language = language_tab(ui, index, language.name, self.language);
            }
        });
        ui.add_space(8.0);
        let language = self.language;
        self.load(sd, language);
        egui::ScrollArea::vertical().id_salt("type_text_tab").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_min_width(820.0);
                ui.label(widget::hint("The name of each type and its icon (50 × 20 pixels, with the name written on it), in this language. The new types come after the types of the game; a new type without an icon in this language shows its English icon."));
                ui.add_space(6.0);
                for kind in 0..COUNT {
                    let key = (language, kind);
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(60.0, 22.0), |ui| row(ui, kind, language, false));
                        let Some(file) = self.files.get_mut(&language) else {
                            ui.label(widget::hint("This language is not in the files that were read."));
                            return;
                        };
                        let Some(original) = file.text(kind) else {
                            ui.label(widget::hint("No line for this type."));
                            return;
                        };
                        let buffer = self.buffers.entry(key).or_insert(original);
                        let response = ui.add(egui::TextEdit::singleline(buffer).id_salt(key).desired_width(260.0));
                        if response.changed() {
                            match file.set_text(kind, buffer) {
                                Ok(()) => {
                                    self.errors.remove(&key);
                                }
                                Err(message) => {
                                    self.errors.insert(key, message);
                                }
                            }
                        }
                        if let Some(message) = self.errors.get(&key) {
                            ui.label(egui::RichText::new(message).color(color::ROSE).size(12.5));
                        }
                        row(ui, kind, language, true);
                    });
                    ui.add_space(2.0);
                }
                if !custom.custom.is_empty() {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("New types").strong());
                    if language != ENGLISH {
                        ui.label(widget::hint("A new type starts with its English name in every language: type the name in this language."));
                    }
                    for entry in custom.custom.iter_mut() {
                        let id = entry.id;
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(60.0, 22.0), |ui| row(ui, id, language, false));
                            let mut text = entry.name_in(language).to_string();
                            let response = ui.add(egui::TextEdit::singleline(&mut text).id_salt(("type_new_text", language, id)).desired_width(260.0).char_limit(crate::types::MAX_NAME));
                            if response.changed() {
                                if language == ENGLISH {
                                    entry.rename(&text);
                                } else {
                                    entry.translations[language] = text.clone();
                                }
                            }
                            if let Some(problem) = crate::types::name_problem(entry.name_in(language)) {
                                ui.label(egui::RichText::new(problem).color(color::ROSE).size(12.5));
                            }
                            row(ui, id, language, true);
                            ui.label(widget::hint("Colour of the buttons in battle:"));
                            let mut colour = entry.colour;
                            sango_species::game_type_menu(ui, &format!("type_colour_{id}"), &mut colour);
                            entry.colour = colour;
                            if widget::button(ui, "Remove") {
                                removed = Some(id);
                            }
                        });
                        ui.add_space(2.0);
                    }
                }
            });
        });
        removed
    }
}

/// A tab button of the languages. Returns the chosen place.
fn language_tab(ui: &mut egui::Ui, place: usize, text: &str, chosen: usize) -> usize {
    let id = egui::Id::new(("type_text_language", place));
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

/// What the CSV functions need to read the texts.
pub struct Library<'a> {
    pub sd: Option<&'a SdCard>,
    /// A file of texts as the romfs has it: the language and the file.
    pub romfs: &'a dyn Fn(usize, usize) -> Option<TextFile>,
}

impl Library<'_> {
    /// The file of a language as the user has it: the SD card first, then the romfs.
    fn file(&self, language: usize) -> Option<TextFile> {
        let on_card = self.sd.and_then(|sd| sd.read(&text::sd_path(language, FILE))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        on_card.or_else(|| (self.romfs)(language, FILE))
    }
}

/// One line of the CSV file of the texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: usize,
    /// The place in `LANGUAGES`.
    pub language: usize,
    pub text: String,
}

/// Writes the names of all the types of the game, in all the languages, as the user has them.
pub fn export(library: &Library) -> String {
    let mut out = format!("{HEADER}\n");
    for kind in 0..COUNT {
        for language in 0..LANGUAGES.len() {
            let Some(text) = library.file(language).and_then(|file| file.text(kind)) else { continue };
            out.push_str(&format!("{kind},{},{}\n", LANGUAGES[language].folder, quote(&text)));
        }
    }
    out
}

/// Reads and checks the CSV text. Nothing is applied yet.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    match records.first() {
        Some((_, first)) if first.join(",") == HEADER => {}
        _ => return Err("The first line is not the header of a type texts CSV file.".into()),
    }
    let mut rows = Vec::new();
    for (line, fields) in records.iter().skip(1) {
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        if fields.len() != 3 {
            return Err(format!("Line {line}: {} columns instead of 3.", fields.len()));
        }
        let kind = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}, column type: \"{}\" is not a number.", fields[0]))?;
        if kind >= COUNT {
            return Err(format!("Line {line}, column type: {kind} is not between 0 and {}.", COUNT - 1));
        }
        let language = LANGUAGES.iter().position(|l| l.folder == fields[1].trim()).ok_or_else(|| format!("Line {line}, column language: \"{}\" is not a language.", fields[1]))?;
        rows.push(Row { kind, language, text: fields[2].clone() });
    }
    Ok(rows)
}

/// Writes the texts of the rows in the files of texts of the SD card. A file is written when a text changed. Gives the
/// number of files that were written. When a text is not right, nothing is written.
pub fn apply(rows: &[Row], library: &Library, sd: &SdCard) -> Result<usize, String> {
    use std::collections::BTreeMap;
    let mut files: BTreeMap<usize, TextFile> = BTreeMap::new();
    let mut original: BTreeMap<usize, TextFile> = BTreeMap::new();
    for row in rows {
        if !files.contains_key(&row.language) {
            let file = library.file(row.language).ok_or_else(|| format!("The file of the names of the types of the language {} is missing.", LANGUAGES[row.language].folder))?;
            original.insert(row.language, file.clone());
            files.insert(row.language, file);
        }
        let file = files.get_mut(&row.language).expect("the file was just read");
        if file.text(row.kind).as_deref() != Some(row.text.as_str()) {
            file.set_text(row.kind, &row.text).map_err(|message| format!("Type {} ({}): {message}", row.kind, LANGUAGES[row.language].folder))?;
        }
    }
    let mut written = 0;
    for (language, file) in &files {
        if original.get(language) == Some(file) {
            continue;
        }
        let path = text::sd_path(*language, FILE);
        sd.write(&path, &file.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the names go out and come back, a changed name is written, and a reset gives the romfs back.
    #[test]
    fn the_names_of_the_types_go_out_come_back_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let mut texts = TypeTexts::default();
        texts.open(&dump);
        let folder = std::env::temp_dir().join(format!("sango_type_texts_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let romfs = |language: usize, file: usize| texts.romfs(language, file);
        let library = Library { sd: Some(&sd), romfs: &romfs };

        let csv = export(&library);
        assert!(csv.starts_with(HEADER));
        assert!(csv.contains("\n0,en,Normal\n") && csv.contains("\n17,en,Fairy\n"));
        assert_eq!(csv.lines().count(), 1 + 18 * 8);
        let rows = parse(&csv).unwrap();
        assert_eq!(apply(&rows, &library, &sd), Ok(0), "the names as they are change nothing");

        let changed = csv.replace("\n0,en,Normal\n", "\n0,en,Plain\n");
        assert_eq!(apply(&parse(&changed).unwrap(), &library, &sd), Ok(1));
        assert!(export(&library).contains("\n0,en,Plain\n"));
        for language in 0..LANGUAGES.len() {
            sd.remove(&text::sd_path(language, FILE)).unwrap();
        }
        assert!(export(&library).contains("\n0,en,Normal\n"));

        assert!(parse(&format!("{HEADER}\n18,en,X\n")).unwrap_err().starts_with("Line 2, column type"));
        assert!(parse(&format!("{HEADER}\n1,xx,X\n")).unwrap_err().starts_with("Line 2, column language"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
