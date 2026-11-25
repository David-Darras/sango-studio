//! The names of the 18 types of the game, in each of the eight languages.
//!
//! They are the lines 0 to 17 of the file of texts 18: the line is the number of the type. A file that the user changed goes
//! on the SD card with the layout of the romfs, in the folder `text`. The names of the new types, in every language, are in the
//! list of the types (see `types.rs`).

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

/// The names of the types of the game: the files that were read and the changes.
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
    /// The files of the romfs that were read, for each language.
    originals: HashMap<usize, TextFile>,
}

impl TypeTexts {
    /// Opens the archives of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.archives = LANGUAGES.iter().map(|language| Garc::open(&dump.archive(language.archive)).ok()).collect();
        self.originals.clear();
        self.language = ENGLISH;
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

    /// The name of a type of the game in the chosen language, as the user has it now.
    pub fn text(&mut self, sd: Option<&SdCard>, kind: usize) -> String {
        let language = self.language;
        self.load(sd, language);
        self.files.get(&language).and_then(|file| file.text(kind)).unwrap_or_default()
    }

    /// The chosen language (the place in `LANGUAGES`).
    pub fn language(&self) -> usize {
        self.language
    }

    /// The menu of the eight languages, English first.
    pub fn language_menu(&mut self, ui: &mut egui::Ui) {
        let chosen = LANGUAGES.get(self.language).map_or("?", |l| l.name);
        egui::ComboBox::from_id_salt("type_text_language").selected_text(chosen).show_ui(ui, |ui| {
            let order = std::iter::once(ENGLISH).chain((0..LANGUAGES.len()).filter(|i| *i != ENGLISH));
            for place in order {
                ui.selectable_value(&mut self.language, place, LANGUAGES[place].name);
            }
        });
    }

    /// The name of a type of the game in the chosen language as the romfs has it.
    fn romfs_name(&mut self, kind: usize) -> Option<String> {
        let language = self.language;
        if !self.originals.contains_key(&language) {
            let file = self.romfs(language, FILE)?;
            self.originals.insert(language, file);
        }
        self.originals.get(&language).and_then(|file| file.text(kind))
    }

    /// True when the name of a type of the game in the chosen language is not the name of the romfs.
    pub fn is_changed(&mut self, sd: Option<&SdCard>, kind: usize) -> bool {
        let language = self.language;
        self.load(sd, language);
        let now = self.files.get(&language).and_then(|file| file.text(kind));
        now.is_some() && now != self.romfs_name(kind)
    }

    /// Puts the name of the romfs back for a type of the game, in the chosen language. Save writes it.
    pub fn reset_name(&mut self, sd: Option<&SdCard>, kind: usize) {
        let language = self.language;
        self.load(sd, language);
        let Some(original) = self.romfs_name(kind) else { return };
        if let Some(file) = self.files.get_mut(&language) {
            if file.set_text(kind, &original).is_ok() {
                self.buffers.insert((language, kind), original);
                self.errors.remove(&(language, kind));
            }
        }
    }

    /// The box of the name of a type of the game in the chosen language. A name that the game cannot keep has a red frame and says
    /// why in a bubble.
    pub fn name_field(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, kind: usize, width: f32) {
        let language = self.language;
        self.load(sd, language);
        let key = (language, kind);
        let Some(file) = self.files.get_mut(&language) else {
            ui.label(widget::hint("No file"));
            return;
        };
        let Some(original) = file.text(kind) else {
            ui.label(widget::hint("No line"));
            return;
        };
        let buffer = self.buffers.entry(key).or_insert(original);
        let response = sango_ui::game_text::edit_line(ui, ("type_name", key), buffer, width);
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
            ui.painter().rect_stroke(response.rect, 3.0, egui::Stroke::new(1.5_f32, color::ROSE));
            response.on_hover_text(egui::RichText::new(message).color(color::ROSE));
        }
    }
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
