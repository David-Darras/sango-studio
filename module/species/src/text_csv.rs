//! The CSV export and import of the texts of the species.
//!
//! One line of the file is one text: a species (and a form), a field of the Text tab, a language and the text.
//! The texts have commas, quotes and line breaks, so the file follows the usual rules of CSV: a text with one of
//! these characters is in quotes, and a quote inside the quotes is written twice.
//!
//! | Column | Content |
//! | --- | --- |
//! | `species` | The number of the species, 1 to 721. |
//! | `form` | The number of the form. The texts that belong to the species (name, category, entry) are for the form 0 only. |
//! | `field` | `name`, `category`, `entry`, `form_name`, `height` or `weight`. |
//! | `language` | The folder of the language: `ja`, `ja_kanji`, `en`, `fr`, `it`, `de`, `es` or `ko`. |
//! | `text` | The text in its plain form (see the module of the texts of the core). |
//!
//! The import checks all the lines first, then writes the files of texts that changed, or nothing.

use crate::pokedex_data::PokedexData;
use crate::text_tab::{Line, FIELDS};
use crate::{csv, SPECIES_COUNT};
use sango_core::csv_text::{quote, records};
use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::{Garc, SdCard};
use std::collections::{BTreeMap, HashMap};

/// The header line of the file.
pub const HEADER: &str = "species,form,field,language,text";

/// What the functions need to read the texts.
pub struct Library<'a> {
    /// The archive of the species data: it tells how many forms a species has.
    pub species: &'a Garc,
    /// The Pokédex data: it tells the place of a form in the tables of texts.
    pub pokedex: Option<&'a PokedexData>,
    pub sd: Option<&'a SdCard>,
    /// A file of texts as the romfs has it: the language and the file.
    pub romfs: &'a dyn Fn(usize, usize) -> Option<TextFile>,
}

/// One line of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub species: usize,
    pub form: usize,
    /// The place in `FIELDS`.
    pub field: usize,
    /// The place in `LANGUAGES`.
    pub language: usize,
    pub text: String,
}

impl Library<'_> {
    /// A file of texts as the user has it: the SD card first, then the romfs.
    fn file(&self, language: usize, file: usize) -> Option<TextFile> {
        let on_card = self.sd.and_then(|sd| sd.read(&text::sd_path(language, file))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
        on_card.or_else(|| (self.romfs)(language, file))
    }

    fn place(&self, species: usize, form: usize) -> usize {
        self.pokedex.map_or(species, |pokedex| pokedex.place(species, form))
    }

    /// The line of a field for a species and a form.
    fn line(&self, field: usize, species: usize, form: usize) -> usize {
        crate::text_tab::TextTab::line_of(&FIELDS[field], species, self.place(species, form))
    }
}

/// Writes all the texts of all the species and forms, as the user has them.
pub fn export(library: &Library) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    let mut files: HashMap<(usize, usize), Option<TextFile>> = HashMap::new();
    for species in 1..=SPECIES_COUNT {
        for form in 0..csv::form_count(library.species, species) {
            for (field, definition) in FIELDS.iter().enumerate() {
                // The texts of the species are written once, with the form 0.
                if definition.line == Line::Species && form != 0 {
                    continue;
                }
                let line = library.line(field, species, form);
                for language in 0..LANGUAGES.len() {
                    let file = files.entry((language, definition.file)).or_insert_with(|| library.file(language, definition.file));
                    let Some(text) = file.as_ref().and_then(|file| file.text(line)) else { continue };
                    out.push_str(&format!("{species},{form},{},{},{}\n", definition.key, LANGUAGES[language].folder, quote(&text)));
                }
            }
        }
    }
    out
}

/// Reads and checks the CSV text. Nothing is applied yet.
pub fn parse(text: &str, library: &Library) -> Result<Vec<Row>, String> {
    let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    match records.first() {
        Some((_, first)) if first.join(",") == HEADER => {}
        _ => return Err("The first line is not the header of a texts CSV file.".into()),
    }
    let mut rows = Vec::new();
    for (line, fields) in records.iter().skip(1) {
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        if fields.len() != 5 {
            return Err(format!("Line {line}: {} columns instead of 5.", fields.len()));
        }
        let number = |place: usize, name: &str| fields[place].trim().parse::<usize>().map_err(|_| format!("Line {line}, column {name}: \"{}\" is not a number.", fields[place]));
        let species = number(0, "species")?;
        let form = number(1, "form")?;
        if !(1..=SPECIES_COUNT).contains(&species) {
            return Err(format!("Line {line}, column species: {species} is not between 1 and {SPECIES_COUNT}."));
        }
        if form >= csv::form_count(library.species, species) {
            return Err(format!("Line {line}, column form: the species {species} has no form {form}."));
        }
        let field = FIELDS.iter().position(|f| f.key == fields[2].trim()).ok_or_else(|| format!("Line {line}, column field: \"{}\" is not a field.", fields[2]))?;
        let language = LANGUAGES.iter().position(|l| l.folder == fields[3].trim()).ok_or_else(|| format!("Line {line}, column language: \"{}\" is not a language.", fields[3]))?;
        if FIELDS[field].line == Line::Species && form != 0 {
            return Err(format!("Line {line}: the field {} belongs to the species, so its form is 0.", FIELDS[field].key));
        }
        rows.push(Row { species, form, field, language, text: fields[4].clone() });
    }
    Ok(rows)
}

/// Writes the texts of the rows in the files of texts of the SD card. A file is written when a text changed. Gives the
/// number of files that were written. When a text is not right, nothing is written.
pub fn apply(rows: &[Row], library: &Library, sd: &SdCard) -> Result<usize, String> {
    let mut files: BTreeMap<(usize, usize), TextFile> = BTreeMap::new();
    let mut original: BTreeMap<(usize, usize), TextFile> = BTreeMap::new();
    for row in rows {
        let key = (row.language, FIELDS[row.field].file);
        if !files.contains_key(&key) {
            let file = library.file(key.0, key.1).ok_or_else(|| format!("The file of texts {} of the language {} is missing.", key.1, LANGUAGES[key.0].folder))?;
            original.insert(key, file.clone());
            files.insert(key, file);
        }
        let line = library.line(row.field, row.species, row.form);
        let file = files.get_mut(&key).expect("the file was just read");
        let now = file.text(line);
        if now.as_deref() != Some(row.text.as_str()) {
            file.set_text(line, &row.text).map_err(|message| format!("Species {}, form {}, {} ({}): {message}", row.species, row.form, FIELDS[row.field].key, LANGUAGES[row.language].folder))?;
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

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::Dump;

    /// With the dump of the game: the texts go out, one is changed in the file, and the import writes it.
    #[test]
    fn the_texts_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let species = Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let archives: Vec<Option<Garc>> = LANGUAGES.iter().map(|l| Garc::open(&dump.archive(l.archive)).ok()).collect();
        let romfs = |language: usize, file: usize| archives.get(language)?.as_ref()?.first_file(file).and_then(|b| TextFile::from_bytes(b).ok());
        let folder = std::env::temp_dir().join(format!("sango_text_csv_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let library = Library { species: &species, pokedex: None, sd: Some(&sd), romfs: &romfs };

        let text = export(&library);
        assert!(text.starts_with(HEADER));
        assert!(text.contains("\n1,0,name,en,Bulbasaur\n"));
        // The entries have line breaks: they are in quotes, and they come back the same.
        let rows = parse(&text, &library).unwrap();
        assert!(rows.len() > 10_000);
        assert_eq!(apply(&rows, &library, &sd), Ok(0), "the texts as they are change nothing");

        // One text is changed in the CSV text: one file is written.
        let changed = text.replace("\n1,0,name,fr,Bulbizarre\n", "\n1,0,name,fr,\"Bulbizarre, le test\"\n");
        let rows = parse(&changed, &library).unwrap();
        assert_eq!(apply(&rows, &library, &sd), Ok(1));
        let again = Library { species: &species, pokedex: None, sd: Some(&sd), romfs: &romfs };
        assert!(export(&again).contains("\n1,0,name,fr,\"Bulbizarre, le test\"\n"));

        // A wrong line is refused with its place.
        let broken = format!("{HEADER}\n1,0,name,xx,Bulbasaur\n");
        assert!(parse(&broken, &library).unwrap_err().starts_with("Line 2, column language"));
        let broken = format!("{HEADER}\n1,1,name,en,Bulbasaur\n");
        assert!(parse(&broken, &library).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
