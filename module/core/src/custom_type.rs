//! The new types that the user made: the list `type/types.csv` and the icons `type/icon/<id>.bclim` of the SD card.
//!
//! The Type page writes the files. The other pages (the species and the moves) only need the number, the English name and
//! the icon of each new type, to show it in their menus: this module reads just that.

use crate::{Clim, SdCard};
use crate::csv_text::records;

/// The first number of a new type: the game has the types 0 to 17, and uses 18 for "no type".
pub const FIRST_ID: usize = 19;
/// The last number of a new type.
pub const LAST_ID: usize = 31;
/// The size of an icon, in pixels.
pub const ICON_WIDTH: usize = 50;
pub const ICON_HEIGHT: usize = 20;

/// A new type, as the menus need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomType {
    pub id: usize,
    /// The English name.
    pub name: String,
    /// The icon as RGBA pixels (50 × 20), when the SD card has it.
    pub icon: Option<Vec<u8>>,
}

/// Reads the list of the new types and their icons from the SD card, in the order of their numbers.
pub fn load(sd: &SdCard) -> Vec<CustomType> {
    let Some(bytes) = sd.read("type/types.csv") else { return Vec::new() };
    let text = String::from_utf8_lossy(&bytes).to_string();
    let Ok(rows) = records(text.strip_prefix('\u{feff}').unwrap_or(&text)) else { return Vec::new() };
    let mut found: Vec<CustomType> = Vec::new();
    for (_, fields) in rows.iter().skip(1) {
        if fields.len() < 2 {
            continue;
        }
        let Ok(id) = fields[0].trim().parse::<usize>() else { continue };
        if !(FIRST_ID..=LAST_ID).contains(&id) || fields[1].is_empty() || found.iter().any(|t| t.id == id) {
            continue;
        }
        let icon = sd.read(&format!("type/icon/{id}.bclim")).and_then(|bytes| Clim::parse(&bytes).ok()).filter(|clim| (clim.width(), clim.height()) == (ICON_WIDTH, ICON_HEIGHT)).and_then(|clim| clim.to_rgba().ok());
        found.push(CustomType { id, name: fields[1].clone(), icon });
    }
    found.sort_by_key(|t| t.id);
    found
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_new_types_are_read_with_their_icons() {
        let folder = std::env::temp_dir().join(format!("sango_custom_type_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        assert!(load(&sd).is_empty());
        sd.write("type/types.csv", b"id,name,colour_type,ja,ja_kanji,fr,it,de,es,ko\n20,Sound,0,,,Son,,,,\n19,Cosmic,0,,,,,,,\n5,Bad,0,,,,,,,\n").unwrap();
        let rgba: Vec<u8> = (0..ICON_WIDTH * ICON_HEIGHT).flat_map(|_| [10u8, 200, 30, 255]).collect();
        sd.write("type/icon/19.bclim", &Clim::from_rgba4(ICON_WIDTH, ICON_HEIGHT, &rgba).unwrap().to_bytes()).unwrap();
        let list = load(&sd);
        assert_eq!(list.iter().map(|t| (t.id, t.name.as_str())).collect::<Vec<_>>(), vec![(19, "Cosmic"), (20, "Sound")]);
        assert!(list[0].icon.is_some() && list[1].icon.is_none());
        // The old file with three columns is read too.
        sd.write("type/types.csv", b"id,name,colour_type\n19,Old,3\n").unwrap();
        assert_eq!(load(&sd)[0].name, "Old");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
