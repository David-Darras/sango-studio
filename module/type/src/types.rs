//! The new types: a number from 19 to 31, a name in each language, and the type of the game whose colour the buttons of the
//! moves use.
//!
//! The list goes on the SD card as a CSV file with the header
//! `id,name,colour_type,ja,ja_kanji,fr,it,de,es,ko`, one line for each new type. `name` is the English name. A language
//! column that is empty means "the English name". The older header `id,name,colour_type` (one name for all the languages)
//! is read too.
//!
//! A name has 1 to 16 characters, without comma, quote or line break. The colour type is a number from 0 to 17.

use crate::chart::{FIRST_CUSTOM, GAME_TYPES, MAX_CUSTOM, MAX_TYPES};
use sango_core::csv_text::{quote, records};
use sango_core::text::LANGUAGES;

/// Where the list is on the SD card.
pub const SD_PATH: &str = "type/types.csv";

/// The header line of the file.
pub const HEADER: &str = "id,name,colour_type,ja,ja_kanji,fr,it,de,es,ko";

/// The header of the first version of the file: one name for all the languages.
const OLD_HEADER: &str = "id,name,colour_type";

/// The longest name of a new type, in characters.
pub const MAX_NAME: usize = 16;

/// The place of English in [`LANGUAGES`].
pub const ENGLISH: usize = 2;

/// A new type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    /// The number of the type, from 19 to 31.
    pub id: usize,
    /// The English name.
    pub name: String,
    /// The type of the game (0 to 17) whose colour the buttons of the moves use.
    pub colour: u8,
    /// The name in each language, in the order of [`LANGUAGES`]. The place of English repeats `name`.
    /// An empty text means "the English name".
    pub translations: [String; 8],
}

impl Custom {
    /// The name in a language (the place in [`LANGUAGES`]): the English name when the language has no name of its own.
    pub fn name_in(&self, language: usize) -> &str {
        match self.translations.get(language) {
            Some(text) if language != ENGLISH && !text.is_empty() => text,
            _ => &self.name,
        }
    }

    /// Gives the English name. A language that had the old English name takes the new one, so a type that was never
    /// translated by hand keeps following its English name.
    pub fn rename(&mut self, new_name: &str) {
        let old = std::mem::replace(&mut self.name, new_name.to_string());
        for text in self.translations.iter_mut() {
            if *text == old {
                *text = new_name.to_string();
            }
        }
    }
}

/// The new types, in the order of their numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeList {
    pub custom: Vec<Custom>,
}

/// Why a name is refused, or nothing when it is good.
pub fn name_problem(name: &str) -> Option<String> {
    let count = name.chars().count();
    if count == 0 {
        return Some("The name is empty.".into());
    }
    if count > MAX_NAME {
        return Some(format!("The name has {count} characters: {MAX_NAME} at most."));
    }
    if name.chars().any(|c| c.is_control() || c == ',' || c == '"') {
        return Some("The name cannot have a comma, a quote or a line break.".into());
    }
    if name != name.trim() {
        return Some("The name cannot start or end with a space.".into());
    }
    None
}

impl TypeList {
    /// The numbers of all the types: the types of the game, then the new types.
    pub fn ids(&self) -> Vec<usize> {
        (0..GAME_TYPES).chain(self.custom.iter().map(|c| c.id)).collect()
    }

    /// A new type by its number.
    pub fn find(&self, id: usize) -> Option<&Custom> {
        self.custom.iter().find(|c| c.id == id)
    }

    /// The smallest number that no new type uses, when the list has room.
    pub fn free_id(&self) -> Option<usize> {
        (FIRST_CUSTOM..MAX_TYPES).find(|id| self.find(*id).is_none())
    }

    /// Adds a new type with the next free number. The English name is the name in every language: the user changes the
    /// other languages by hand. Gives its number, or nothing when the list is full.
    pub fn add(&mut self, name: &str, colour: u8) -> Option<usize> {
        if self.custom.len() >= MAX_CUSTOM {
            return None;
        }
        let id = self.free_id()?;
        self.custom.push(Custom { id, name: name.to_string(), colour, translations: std::array::from_fn(|_| name.to_string()) });
        self.custom.sort_by_key(|c| c.id);
        Some(id)
    }

    /// Removes a new type. False when the number is not in the list.
    pub fn remove(&mut self, id: usize) -> bool {
        let before = self.custom.len();
        self.custom.retain(|c| c.id != id);
        self.custom.len() != before
    }

    /// The file of the SD card.
    pub fn to_csv(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for custom in &self.custom {
            out.push_str(&format!("{},{},{}", custom.id, quote(&custom.name), custom.colour));
            for language in 0..LANGUAGES.len() {
                if language != ENGLISH {
                    out.push(',');
                    out.push_str(&quote(&custom.translations[language]));
                }
            }
            out.push('\n');
        }
        out
    }

    /// Reads the file. Every line is checked: nothing is kept when one is wrong.
    pub fn from_csv(text: &str) -> Result<TypeList, String> {
        let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
        let columns = match records.first() {
            Some((_, first)) if first.join(",") == HEADER => 10,
            Some((_, first)) if first.join(",") == OLD_HEADER => 3,
            _ => return Err(format!("The first line is not the header of a types file ({HEADER}).")),
        };
        let mut list = TypeList::default();
        for (line, fields) in records.iter().skip(1) {
            if fields.len() == 1 && fields[0].is_empty() {
                continue;
            }
            if fields.len() != columns {
                return Err(format!("Line {line}: {} columns instead of {columns}.", fields.len()));
            }
            let id = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}, column id: \"{}\" is not a number.", fields[0]))?;
            if !(FIRST_CUSTOM..MAX_TYPES).contains(&id) {
                return Err(format!("Line {line}, column id: {id} is not between {FIRST_CUSTOM} and {}.", MAX_TYPES - 1));
            }
            if list.find(id).is_some() {
                return Err(format!("Line {line}, column id: the number {id} is used twice."));
            }
            if let Some(problem) = name_problem(&fields[1]) {
                return Err(format!("Line {line}, column name: {problem}"));
            }
            let colour = fields[2].trim().parse::<u8>().ok().filter(|c| (*c as usize) < GAME_TYPES).ok_or_else(|| format!("Line {line}, column colour_type: \"{}\" is not between 0 and {}.", fields[2], GAME_TYPES - 1))?;
            let mut translations: [String; 8] = std::array::from_fn(|_| fields[1].clone());
            if columns == 10 {
                let mut column = 3;
                for language in 0..LANGUAGES.len() {
                    if language == ENGLISH {
                        continue;
                    }
                    let name = &fields[column];
                    if !name.is_empty() {
                        if let Some(problem) = name_problem(name) {
                            return Err(format!("Line {line}, column {}: {problem}", LANGUAGES[language].folder));
                        }
                        translations[language] = name.clone();
                    }
                    column += 1;
                }
            }
            list.custom.push(Custom { id, name: fields[1].clone(), colour, translations });
        }
        list.custom.sort_by_key(|c| c.id);
        Ok(list)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn types_are_added_removed_and_saved() {
        let mut list = TypeList::default();
        assert_eq!(list.add("Cosmic", 15), Some(19));
        assert_eq!(list.add("Sound, loud", 12), Some(20));
        assert_eq!(list.add("Light", 9), Some(21));
        assert!(list.remove(20));
        assert!(!list.remove(20));
        assert_eq!(list.add("Time", 4), Some(20), "a free number is used again");
        assert_eq!(list.ids().len(), 18 + 3);
        let back = TypeList::from_csv(&list.to_csv()).unwrap();
        assert_eq!(back, list);
        let mut full = TypeList::default();
        for n in 0..MAX_CUSTOM {
            assert!(full.add(&format!("Type {n}"), 0).is_some());
        }
        assert_eq!(full.add("One more", 0), None);
        assert_eq!(full.free_id(), None);
    }

    #[test]
    fn the_english_name_goes_to_every_language_and_the_others_are_changed_by_hand() {
        let mut list = TypeList::default();
        let id = list.add("Cosmic", 15).unwrap();
        let custom = list.custom.iter_mut().find(|c| c.id == id).unwrap();
        assert!((0..8).all(|language| custom.name_in(language) == "Cosmic"));
        // The French name is changed by hand, then the English name changes: French keeps its own name.
        custom.translations[3] = "Cosmique".into();
        custom.rename("Space");
        assert_eq!(custom.name_in(ENGLISH), "Space");
        assert_eq!(custom.name_in(3), "Cosmique");
        assert_eq!(custom.name_in(5), "Space", "a language that was never changed follows English");
        // The file keeps the names, and an empty column is the English name.
        let back = TypeList::from_csv(&list.to_csv()).unwrap();
        assert_eq!(back, list);
        let text = "id,name,colour_type,ja,ja_kanji,fr,it,de,es,ko\n19,Space,15,,,Espace,,,,\n";
        let read = TypeList::from_csv(text).unwrap();
        assert_eq!(read.custom[0].name_in(3), "Espace");
        assert_eq!(read.custom[0].name_in(0), "Space");
    }

    #[test]
    fn a_file_of_the_first_version_is_still_read() {
        let list = TypeList::from_csv("id,name,colour_type\n19,Cosmic,15\n").unwrap();
        assert_eq!(list.custom[0].name_in(3), "Cosmic");
    }

    #[test]
    fn a_wrong_name_or_line_is_refused_with_its_place() {
        assert!(name_problem("Cosmic").is_none());
        assert!(name_problem("").is_some());
        assert!(name_problem("A very long name indeed").is_some());
        assert!(name_problem("a,b").is_some());
        assert!(name_problem(" Cosmic").is_some());
        for (text, start) in [
            ("id,name\n19,A,0\n", "The first line"),
            ("id,name,colour_type\n5,A,0\n", "Line 2, column id"),
            ("id,name,colour_type\n19,A,0\n19,B,0\n", "Line 3, column id: the number 19"),
            ("id,name,colour_type\n19,,0\n", "Line 2, column name"),
            ("id,name,colour_type\n19,A,18\n", "Line 2, column colour_type"),
            ("id,name,colour_type\n19,A\n", "Line 2: 2 columns"),
            ("id,name,colour_type,ja,ja_kanji,fr,it,de,es,ko\n19,A,0,,,A very long name indeed,,,,\n", "Line 2, column fr"),
        ] {
            assert!(TypeList::from_csv(text).unwrap_err().starts_with(start), "{text}");
        }
    }
}
