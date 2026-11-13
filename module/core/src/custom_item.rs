//! The new moves and the new items that the user made: the files `move/custom/<id>.bin` and `item/custom/<id>.bin` of the SD
//! card.
//!
//! The Move page and the Item page write the files (the layouts are in their modules `custom`). The other pages (the species)
//! only need the number, the English name and some data of each new move and item, to show them in their menus: this module reads
//! just that.

use crate::SdCard;

/// The size of the data of a move or of an item.
const DATA_SIZE: usize = 36;
/// The position of English in the list of the eight languages of a file.
const ENGLISH: usize = 2;

/// The first id and the last id of a new move and of a new item.
pub const FIRST_ID: usize = 900;
pub const LAST_ID: usize = 931;

/// A new move or a new item, as the menus need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    pub id: usize,
    /// The English name.
    pub name: String,
    /// The 36 bytes of the data of the move or of the item.
    pub data: [u8; DATA_SIZE],
    /// The picture as RGBA pixels (30 × 30), for an item that has its file.
    pub picture: Option<Vec<u8>>,
}

/// Reads the id, the English name and the data of a file. `texts` is the number of texts of a language (2 for a move: the name and
/// the description; 3 for an item: the name, the description and the message of the Bag).
fn read(bytes: &[u8], magic: &[u8; 4], texts: usize) -> Option<(usize, String, [u8; DATA_SIZE])> {
    if bytes.get(..4)? != magic || *bytes.get(4)? != 1 {
        return None;
    }
    let id = u16::from_le_bytes([*bytes.get(5)?, *bytes.get(6)?]) as usize;
    let mut at = 8;
    let mut name = None;
    for language in 0..8 {
        for text in 0..texts {
            let length = u16::from_le_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]) as usize;
            let raw = bytes.get(at + 2..at + 2 + length * 2)?;
            if language == ENGLISH && text == 0 {
                let units: Vec<u16> = raw.chunks(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
                name = Some(String::from_utf16(&units).ok()?);
            }
            at += 2 + length * 2;
        }
    }
    let data: [u8; DATA_SIZE] = bytes.get(at..at + DATA_SIZE)?.try_into().ok()?;
    Some((id, name?, data))
}

/// The new moves of the SD card, by id (900 to 931).
pub fn moves(sd: &SdCard) -> Vec<Custom> {
    load(sd, "move/custom", b"SMVR", 2, false)
}

/// The new items of the SD card, by id (900 to 931), with their pictures.
pub fn items(sd: &SdCard) -> Vec<Custom> {
    load(sd, "item/custom", b"SITM", 3, true)
}

fn load(sd: &SdCard, folder: &str, magic: &[u8; 4], texts: usize, with_picture: bool) -> Vec<Custom> {
    let mut found: Vec<Custom> = Vec::new();
    for path in sd.files_in(folder) {
        if !path.ends_with(".bin") {
            continue;
        }
        let Some((id, name, data)) = sd.read(&path).and_then(|bytes| read(&bytes, magic, texts)) else { continue };
        if !(FIRST_ID..=LAST_ID).contains(&id) || found.iter().any(|c| c.id == id) {
            continue;
        }
        let picture = if with_picture { sd.read(&format!("{folder}/{id}.bclim")).and_then(|bytes| crate::Clim::parse(&bytes).ok()).filter(|clim| (clim.width(), clim.height()) == (30, 30)).and_then(|clim| clim.to_rgba().ok()) } else { None };
        found.push(Custom { id, name, data, picture });
    }
    found.sort_by_key(|c| c.id);
    found
}

#[cfg(test)]
mod test {
    use super::*;

    fn write_text(out: &mut Vec<u8>, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        out.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for unit in units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }

    /// A file with the layout of the pages: the header, the texts, the data.
    fn file(magic: &[u8; 4], id: u16, texts: usize, english: &str) -> Vec<u8> {
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&[1]);
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(0);
        for language in 0..8 {
            for text in 0..texts {
                write_text(&mut bytes, if language == ENGLISH && text == 0 { english } else { "x" });
            }
        }
        let mut data = [0u8; DATA_SIZE];
        data[3] = 80;
        bytes.extend_from_slice(&data);
        bytes
    }

    #[test]
    fn the_moves_and_the_items_are_read() {
        let folder = std::env::temp_dir().join(format!("sango_custom_item_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        assert!(moves(&sd).is_empty() && items(&sd).is_empty());
        sd.write("move/custom/901.bin", &file(b"SMVR", 901, 2, "Thunderclap")).unwrap();
        sd.write("move/custom/900.bin", &file(b"SMVR", 900, 2, "Absolute Zero")).unwrap();
        sd.write("move/custom/5.bin", &file(b"SMVR", 5, 2, "Too small")).unwrap();
        sd.write("item/custom/900.bin", &file(b"SITM", 900, 3, "Plasma Core")).unwrap();
        sd.write("item/custom/bad.bin", b"nothing").unwrap();
        let list = moves(&sd);
        assert_eq!(list.iter().map(|c| (c.id, c.name.as_str())).collect::<Vec<_>>(), vec![(900, "Absolute Zero"), (901, "Thunderclap")]);
        assert_eq!(list[0].data[3], 80);
        let list = items(&sd);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Plasma Core");
        assert!(list[0].picture.is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
