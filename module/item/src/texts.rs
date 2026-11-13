//! The texts of the items, in each of the eight languages of the game.
//!
//! | Field | File of texts | Line |
//! | --- | --- | --- |
//! | Name | 114 | the number of the item |
//! | Name in messages | 113 | the number of the item |
//! | Description | 117 | the number of the item |
//!
//! The name is the one of the lists (the Bag, the shops). The name in messages is the one that the game puts into a
//! sentence: it has tags for the plural. The description is the text of the Bag. The files 115 and 116 of the archive
//! also have 776 lines of item names, but no code of the game names them, so the page does not show them.
//!
//! A file that the user changed goes on the SD card with the layout of the romfs, in the folder `text`.

use sango_core::csv_text::{quote, records};
use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::{HashMap, HashSet};

/// A text of an item.
pub struct Field {
    pub label: &'static str,
    /// The name of the field in the CSV file.
    pub key: &'static str,
    /// The number of the file of texts.
    pub file: usize,
    /// The number of lines that the text box shows.
    pub rows: usize,
    pub hint: &'static str,
}

pub const FIELDS: [Field; 3] = [
    Field { label: "Name", key: "name", file: 114, rows: 1, hint: "The name in the lists: the Bag, the shops, the summary." },
    Field { label: "Name in messages", key: "name_in_messages", file: 113, rows: 1, hint: "The name that the game writes inside a sentence. It has tags for the plural: keep them when you change the name." },
    Field { label: "Description", key: "description", file: 117, rows: 4, hint: "The text of the Bag. A line break is a line break of the text." },
];

/// The language of the names in the lists of the page: English.
pub const ENGLISH: usize = 2;

/// The state of the Text tab.
#[derive(Default)]
pub struct ItemTexts {
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
    /// The item that the boxes are for.
    current: Option<usize>,
    /// The chosen language (0 to 7).
    language: usize,
}

impl ItemTexts {
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
        self.current = None;
    }

    /// A file of texts as the romfs has it.
    pub fn romfs(&self, language: usize, file: usize) -> Option<TextFile> {
        self.archives.get(language)?.as_ref()?.first_file(file).and_then(|bytes| TextFile::from_bytes(bytes).ok())
    }

    /// Reads a file of texts if it is not read yet: the SD card first, then the romfs.
    fn load(&mut self, sd: Option<&SdCard>, language: usize, file: usize) {
        if self.files.contains_key(&(language, file)) {
            return;
        }
        let on_card = sd.and_then(|sd| sd.read(&text::sd_path(language, file))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        let from_card = on_card.is_some();
        if let Some(found) = on_card.or_else(|| self.romfs(language, file)) {
            if from_card {
                self.from_card.insert((language, file));
            }
            self.saved.insert((language, file), found.clone());
            self.files.insert((language, file), found);
        }
    }

    /// The English name of an item, as the user has it now.
    pub fn name(&mut self, sd: Option<&SdCard>, item: usize) -> String {
        self.load(sd, ENGLISH, FIELDS[0].file);
        self.files.get(&(ENGLISH, FIELDS[0].file)).and_then(|file| file.text(item)).unwrap_or_default()
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

    /// Draws the tab for an item.
    pub fn ui(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, item: usize) {
        if self.current != Some(item) {
            self.current = Some(item);
            self.buffers.clear();
            self.errors.clear();
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for (index, language) in LANGUAGES.iter().enumerate() {
                self.language = language_tab(ui, index, language.name, self.language);
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("item_text_tab").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_width(760.0);
                for field in &FIELDS {
                    self.field(ui, sd, field, item);
                    ui.add_space(10.0);
                }
            });
        });
    }

    /// One field: its name, its text box, and what is wrong with the text.
    fn field(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, field: &Field, item: usize) {
        let language = self.language;
        self.load(sd, language, field.file);
        let key = (language, field.file, item);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(field.label).strong());
            let edited = match (self.files.get(&(language, field.file)), self.saved.get(&(language, field.file))) {
                (Some(now), Some(before)) => now.text(item) != before.text(item),
                _ => false,
            };
            if edited {
                widget::badge(ui, "Not saved", color::AMBER);
            }
        });
        ui.label(widget::hint(field.hint));
        let Some(file) = self.files.get_mut(&(language, field.file)) else {
            ui.label(widget::hint("This text is not in the files that were read."));
            return;
        };
        let Some(original) = file.text(item) else {
            ui.label(widget::hint("This item has no line in this file."));
            return;
        };
        let buffer = self.buffers.entry(key).or_insert(original);
        let edit = if field.rows == 1 { egui::TextEdit::singleline(buffer) } else { egui::TextEdit::multiline(buffer).desired_rows(field.rows) };
        let response = ui.add(edit.id_salt(key).desired_width(f32::INFINITY));
        if response.changed() {
            match file.set_text(item, buffer) {
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
fn language_tab(ui: &mut egui::Ui, place: usize, text: &str, chosen: usize) -> usize {
    let id = egui::Id::new(("item_text_language", place));
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
    /// A file of texts as the user has it: the SD card first, then the romfs.
    fn file(&self, language: usize, file: usize) -> Option<TextFile> {
        let on_card = self.sd.and_then(|sd| sd.read(&text::sd_path(language, file))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        on_card.or_else(|| (self.romfs)(language, file))
    }
}

/// The header line of the CSV file of the texts.
pub const HEADER: &str = "item,field,language,text";

/// One line of the CSV file of the texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub item: usize,
    /// The place in `FIELDS`.
    pub field: usize,
    /// The place in `LANGUAGES`.
    pub language: usize,
    pub text: String,
}

/// Writes all the texts of all the items, as the user has them.
pub fn export(library: &Library) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    let mut files: HashMap<(usize, usize), Option<TextFile>> = HashMap::new();
    for item in 1..=crate::data::COUNT {
        for definition in FIELDS.iter() {
            for language in 0..LANGUAGES.len() {
                let file = files.entry((language, definition.file)).or_insert_with(|| library.file(language, definition.file));
                let Some(text) = file.as_ref().and_then(|file| file.text(item)) else { continue };
                out.push_str(&format!("{item},{},{},{}\n", definition.key, LANGUAGES[language].folder, quote(&text)));
            }
        }
    }
    out
}

/// Reads and checks the CSV text. Nothing is applied yet.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    match records.first() {
        Some((_, first)) if first.join(",") == HEADER => {}
        _ => return Err("The first line is not the header of an item texts CSV file.".into()),
    }
    let mut rows = Vec::new();
    for (line, fields) in records.iter().skip(1) {
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        if fields.len() != 4 {
            return Err(format!("Line {line}: {} columns instead of 4.", fields.len()));
        }
        let item = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}, column item: \"{}\" is not a number.", fields[0]))?;
        if !(1..=crate::data::COUNT).contains(&item) {
            return Err(format!("Line {line}, column item: {item} is not between 1 and {}.", crate::data::COUNT));
        }
        let field = FIELDS.iter().position(|f| f.key == fields[1].trim()).ok_or_else(|| format!("Line {line}, column field: \"{}\" is not a field.", fields[1]))?;
        let language = LANGUAGES.iter().position(|l| l.folder == fields[2].trim()).ok_or_else(|| format!("Line {line}, column language: \"{}\" is not a language.", fields[2]))?;
        rows.push(Row { item, field, language, text: fields[3].clone() });
    }
    Ok(rows)
}

/// Writes the texts of the rows in the files of texts of the SD card. A file is written when a text changed. Gives the
/// number of files that were written. When a text is not right, nothing is written.
pub fn apply(rows: &[Row], library: &Library, sd: &SdCard) -> Result<usize, String> {
    use std::collections::BTreeMap;
    let mut files: BTreeMap<(usize, usize), TextFile> = BTreeMap::new();
    let mut original: BTreeMap<(usize, usize), TextFile> = BTreeMap::new();
    for row in rows {
        let key = (row.language, FIELDS[row.field].file);
        if !files.contains_key(&key) {
            let file = library.file(key.0, key.1).ok_or_else(|| format!("The file of texts {} of the language {} is missing.", key.1, LANGUAGES[key.0].folder))?;
            original.insert(key, file.clone());
            files.insert(key, file);
        }
        let file = files.get_mut(&key).expect("the file was just read");
        if file.text(row.item).as_deref() != Some(row.text.as_str()) {
            file.set_text(row.item, &row.text).map_err(|message| format!("Item {}, {} ({}): {message}", row.item, FIELDS[row.field].key, LANGUAGES[row.language].folder))?;
        }
    }
    let mut written = 0;
    for (key, file) in &files {
        if original.get(key) == Some(file) {
            continue;
        }
        let path = text::sd_path(key.0, key.1);
        sd.write(&path, &file.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        written += 1;
    }
    Ok(written)
}

/// Puts the texts of an item back to the romfs text in the files of the SD card, in every language. A file that is
/// the romfs file again is removed.
pub fn reset_item(sd: &SdCard, item: usize, romfs: &dyn Fn(usize, usize) -> Option<TextFile>) -> Result<(), String> {
    for language in 0..LANGUAGES.len() {
        for field in &FIELDS {
            let path = text::sd_path(language, field.file);
            let Some(mut on_card) = sd.read(&path).and_then(|bytes| TextFile::from_bytes(&bytes).ok()) else { continue };
            let Some(rom) = romfs(language, field.file) else { continue };
            if let Some(original) = rom.text(item) {
                on_card.set_text(item, &original)?;
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

/// Removes the files of texts of the items from the SD card: the game reads the romfs again.
pub fn reset_all(sd: &SdCard) -> Result<(), String> {
    for language in 0..LANGUAGES.len() {
        for field in &FIELDS {
            sd.remove(&text::sd_path(language, field.file))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the texts go out and come back, a changed text is written, and a reset gives the
    /// romfs text back.
    #[test]
    fn the_texts_of_the_items_go_out_come_back_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let mut texts = ItemTexts::default();
        texts.open(&dump);
        let folder = std::env::temp_dir().join(format!("sango_item_texts_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let romfs = |language: usize, file: usize| texts.romfs(language, file);
        let library = Library { sd: Some(&sd), romfs: &romfs };

        let csv = export(&library);
        assert!(csv.starts_with(HEADER));
        assert!(csv.contains("\n1,name,en,Master Ball\n"));
        assert!(csv.contains("\n1,description,en,\"The best Poké Ball with the ultimate\nlevel of performance."));
        let rows = parse(&csv).unwrap();
        assert!(rows.len() > 775 * 3 * 7);
        assert_eq!(apply(&rows, &library, &sd), Ok(0), "the texts as they are change nothing");

        let changed = csv.replace("\n1,name,fr,Master Ball\n", "\n1,name,fr,\"Master Ball, test\"\n");
        assert_ne!(changed, csv, "the French name of the Master Ball is in the file");
        let rows = parse(&changed).unwrap();
        assert_eq!(apply(&rows, &library, &sd), Ok(1));
        assert!(export(&library).contains("\n1,name,fr,\"Master Ball, test\"\n"));

        // The name of the Bag goes back with the reset of the item, and the file is the romfs file again.
        reset_item(&sd, 1, &romfs).unwrap();
        assert!(export(&library).contains("\n1,name,fr,Master Ball\n"));
        assert!(sd.files_in("text").is_empty());

        assert!(parse("species,form\n").is_err());
        assert!(parse(&format!("{HEADER}\n1,name,xx,Master Ball\n")).unwrap_err().starts_with("Line 2, column language"));
        assert!(parse(&format!("{HEADER}\n9999,name,en,Master Ball\n")).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
