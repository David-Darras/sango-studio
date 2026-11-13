//! The new abilities that the user made: the files `ability/<id>.bin` of the SD card.
//!
//! The Ability page writes the files (the layout is in its module `rules`). The other pages only need the id and the English
//! name of each ability, to show it in their lists: this module reads just that.

use crate::SdCard;

/// The first id of a new ability: the game has the abilities 1 to 191.
pub const FIRST_ID: usize = 192;
/// The last id of an ability: an ability id is one byte.
pub const LAST_ID: usize = 255;

/// The position of English in the list of the eight languages of a file.
const ENGLISH: usize = 2;

/// The id and the English name of an ability, from the bytes of its file. `None` for a file that is not valid.
pub fn read_name(bytes: &[u8]) -> Option<(usize, String)> {
    if bytes.get(..4)? != b"SABR" || *bytes.get(4)? != 1 {
        return None;
    }
    let id = *bytes.get(5)? as usize;
    let mut at = 8;
    for language in 0..=ENGLISH {
        // The name, then the description.
        for part in 0..2 {
            let length = u16::from_le_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]) as usize;
            let raw = bytes.get(at + 2..at + 2 + length * 2)?;
            if language == ENGLISH && part == 0 {
                let units: Vec<u16> = raw.chunks(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
                return Some((id, String::from_utf16(&units).ok()?));
            }
            at += 2 + length * 2;
        }
    }
    None
}

/// The new abilities of the SD card, by id (192 to 255).
pub fn names(sd: &SdCard) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = sd.files_in("ability").iter().filter_map(|path| read_name(&sd.read(path)?)).filter(|(id, _)| (FIRST_ID..=LAST_ID).contains(id)).collect();
    found.sort();
    found.dedup_by_key(|(id, _)| *id);
    found
}

/// The names of the abilities for a list: the abilities of the game, then the new ones. The name of the ability `n` is at the
/// place `n - 1`. An id without an ability has an empty name.
pub fn list(game: &[&str], sd: Option<&SdCard>) -> Vec<String> {
    let mut all: Vec<String> = game.iter().map(|name| name.to_string()).collect();
    let extra = sd.map(names).unwrap_or_default();
    if let Some(last) = extra.iter().map(|(id, _)| *id).max() {
        all.resize(last, String::new());
        for (id, name) in extra {
            all[id - 1] = name;
        }
    }
    all
}

#[cfg(test)]
mod test {
    use super::*;

    /// A file with the layout of the Ability page: the English name is the third text pair.
    fn file(id: u8, english: &str) -> Vec<u8> {
        let mut bytes = b"SABR".to_vec();
        bytes.extend_from_slice(&[1, id, 0, 0]);
        for language in 0..8 {
            for text in [if language == ENGLISH { english } else { "x" }, "description"] {
                let units: Vec<u16> = text.encode_utf16().collect();
                bytes.extend_from_slice(&(units.len() as u16).to_le_bytes());
                for unit in units {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
        }
        bytes
    }

    #[test]
    fn the_name_and_the_id_are_read() {
        assert_eq!(read_name(&file(230, "Quick Start")), Some((230, "Quick Start".to_string())));
        assert_eq!(read_name(b"SABR"), None);
        assert_eq!(read_name(b"NOPE0000"), None);
        let bytes = file(230, "Quick Start");
        assert_eq!(read_name(&bytes[..20]), None);
    }

    #[test]
    fn the_list_has_the_new_abilities_at_their_ids() {
        let folder = std::env::temp_dir().join(format!("sango_custom_list_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        sd.write("ability/194.bin", &file(194, "Quick Start")).unwrap();
        sd.write("ability/12.bin", &file(12, "Too small")).unwrap();
        sd.write("ability/bad.bin", b"nothing").unwrap();
        let all = list(&["Stench", "Drizzle"], Some(&sd));
        assert_eq!(all.len(), 194);
        assert_eq!(all[0], "Stench");
        assert_eq!(all[193], "Quick Start");
        assert!(all[2].is_empty() && all[191].is_empty());
        assert_eq!(list(&["Stench"], None), vec!["Stench".to_string()]);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
