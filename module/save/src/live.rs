//! The changes that the running game takes: the file `save/blocks.bin` of the folder of the studio on the SD card.
//!
//! The plugin looks for the file while the game runs. When the player walks freely in the world, it writes the blocks in the save
//! in memory, moves the player when the file asks for it, and removes the file.
//!
//! | Size | Meaning |
//! | --- | --- |
//! | 4 | The signature `SSAV`. |
//! | 4 | The version: 1. |
//! | 4 | 1 when the player moves, else 0. |
//! | 4 | The number of blocks. |
//! | 20 | The map, the side, then the point (three decimal numbers). |
//! | For each block | Its number, its size, then its bytes, to a multiple of 4. |

use crate::data::Place;
use crate::format::{self, Save};
use sango_core::SdCard;
use std::path::PathBuf;

/// The place of the file in the folder of the studio.
pub const FILE: &str = "save/blocks.bin";
const SIGNATURE: &[u8; 4] = b"SSAV";
const VERSION: u32 = 1;
const HEADER: usize = 36;

/// What a file asks: the move of the player and the blocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub place: Option<Place>,
    pub blocks: Vec<(usize, Vec<u8>)>,
}

impl Changes {
    pub fn to_bytes(&self) -> Vec<u8> {
        let place = self.place.unwrap_or(Place { map: 0, x: 0.0, y: 0.0, z: 0.0, facing: 0 });
        let mut bytes = SIGNATURE.to_vec();
        for word in [VERSION, u32::from(self.place.is_some()), self.blocks.len() as u32, u32::from(place.map), u32::from(place.facing), place.x.to_bits(), place.y.to_bits(), place.z.to_bits()] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        for (block, data) in &self.blocks {
            bytes.extend_from_slice(&(*block as u32).to_le_bytes());
            bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(data);
            bytes.resize(bytes.len().next_multiple_of(4), 0);
        }
        bytes
    }

    /// Reads a file. A file that is not complete gives nothing.
    pub fn parse(bytes: &[u8]) -> Option<Changes> {
        let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if bytes.get(..4)? != SIGNATURE || word(4)? != VERSION {
            return None;
        }
        let place = Place { map: word(16)? as u16, facing: word(20)? as u8, x: f32::from_bits(word(24)?), y: f32::from_bits(word(28)?), z: f32::from_bits(word(32)?) };
        let place = (word(8)? != 0).then_some(place);
        let mut blocks = Vec::new();
        let mut at = HEADER;
        for _ in 0..word(12)? {
            let (block, size) = (word(at)? as usize, word(at + 4)? as usize);
            blocks.push((block, bytes.get(at + 8..at + 8 + size)?.to_vec()));
            at += 8 + size.next_multiple_of(4);
        }
        Some(Changes { place, blocks })
    }
}

fn path(sd: &SdCard) -> PathBuf {
    sd.root().join(FILE)
}

/// True while the game did not take the last changes.
pub fn is_waiting(sd: &SdCard) -> bool {
    path(sd).is_file()
}

/// Removes the changes that the game did not take.
pub fn cancel(sd: &SdCard) {
    let _ = std::fs::remove_file(path(sd));
}

/// Asks the running game to take the changed blocks of the save. `now` is the file as it was just written, and `place` the new
/// place of the player when it changed. The blocks that the game did not take yet go again, as they are now. The block of the
/// position does not go: the plugin moves the player instead.
pub fn send(sd: &SdCard, now: &Save, changed: &[usize], place: Option<Place>) -> Result<(), String> {
    let path = path(sd);
    let waiting = std::fs::read(&path).ok().and_then(|bytes| Changes::parse(&bytes)).unwrap_or_default();
    let mut blocks: Vec<usize> = waiting.blocks.iter().map(|(block, _)| *block).chain(changed.iter().copied()).filter(|&b| b < format::BLOCK_COUNT && b != format::POSITION).collect();
    blocks.sort_unstable();
    blocks.dedup();
    let changes = Changes { place: place.or(waiting.place), blocks: blocks.into_iter().map(|block| (block, now.block(block).to_vec())).collect() };
    if changes.place.is_none() && changes.blocks.is_empty() {
        return Ok(());
    }
    let failed = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    // The game reads a complete file only.
    let part = path.with_extension("part");
    std::fs::write(&part, changes.to_bytes()).map_err(failed)?;
    std::fs::rename(&part, &path).map_err(failed)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_file_is_read_back() {
        let changes = Changes { place: Some(Place { map: 224, x: 153.0, y: 2.0, z: 243.0, facing: 1 }), blocks: vec![(11, vec![1, 2, 3]), (18, vec![4; 8])] };
        let bytes = changes.to_bytes();
        assert_eq!(Changes::parse(&bytes), Some(changes));
        assert_eq!(Changes::parse(&bytes[..bytes.len() - 4]), None);
        assert_eq!(Changes::parse(b"SSAV"), None);
    }

    #[test]
    fn the_blocks_that_wait_go_again() {
        let root = std::env::temp_dir().join(format!("sango_save_live_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let sd = SdCard::open(&root.to_string_lossy()).unwrap();
        let mut save = format::empty_for_test();
        let to = Place { map: 7, x: 9.0, y: 0.0, z: 9.0, facing: 0 };
        send(&sd, &save, &[format::MISC, format::POSITION], Some(to)).unwrap();
        assert!(is_waiting(&sd));
        save.block_mut(format::MISC)[8] = 3;
        send(&sd, &save, &[format::ITEMS], None).unwrap();
        let changes = Changes::parse(&std::fs::read(path(&sd)).unwrap()).unwrap();
        assert_eq!(changes.blocks.iter().map(|b| b.0).collect::<Vec<_>>(), [format::ITEMS, format::MISC]);
        assert_eq!((changes.blocks[1].1[8], changes.place), (3, Some(to)));
        cancel(&sd);
        assert!(!is_waiting(&sd));
        let _ = std::fs::remove_dir_all(root);
    }
}
