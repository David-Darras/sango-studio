//! The new types: a number from 19 to 31, a name, and the type of the game whose colour the buttons of the moves use.
//!
//! The list goes on the SD card as a CSV file with the header `id,name,colour_type`, one line for each new type. The name
//! has 1 to 16 characters, without comma, quote or line break. The colour type is a number from 0 to 17.

use crate::chart::{FIRST_CUSTOM, GAME_TYPES, MAX_CUSTOM, MAX_TYPES};
use sango_core::csv_text::{quote, records};

/// Where the list is on the SD card.
pub const SD_PATH: &str = "type/types.csv";

/// The header line of the file.
pub const HEADER: &str = "id,name,colour_type";

/// The longest name of a new type, in characters.
pub const MAX_NAME: usize = 16;

/// A new type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    /// The number of the type, from 19 to 31.
    pub id: usize,
    pub name: String,
    /// The type of the game (0 to 17) whose colour the buttons of the moves use.
    pub colour: u8,
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

    /// Adds a new type with the next free number. Gives its number, or nothing when the list is full.
    pub fn add(&mut self, name: &str, colour: u8) -> Option<usize> {
        if self.custom.len() >= MAX_CUSTOM {
            return None;
        }
        let id = self.free_id()?;
        self.custom.push(Custom { id, name: name.to_string(), colour });
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
            out.push_str(&format!("{},{},{}\n", custom.id, quote(&custom.name), custom.colour));
        }
        out
    }

    /// Reads the file. Every line is checked: nothing is kept when one is wrong.
    pub fn from_csv(text: &str) -> Result<TypeList, String> {
        let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
        match records.first() {
            Some((_, first)) if first.join(",") == HEADER => {}
            _ => return Err(format!("The first line is not the header of a types file ({HEADER}).")),
        }
        let mut list = TypeList::default();
        for (line, fields) in records.iter().skip(1) {
            if fields.len() == 1 && fields[0].is_empty() {
                continue;
            }
            if fields.len() != 3 {
                return Err(format!("Line {line}: {} columns instead of 3.", fields.len()));
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
            list.custom.push(Custom { id, name: fields[1].clone(), colour });
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
        ] {
            assert!(TypeList::from_csv(text).unwrap_err().starts_with(start), "{text}");
        }
    }
}
