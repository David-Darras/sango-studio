//! The new moves of the user (the files `move/custom/<id>.bin` of the SD card), for the menus that list moves: the level-up moves of a
//! species can be a new move.
//!
//! The Move page writes the files. This module reads them again at most one time each second, so a new move shows in the menus
//! soon after it is saved.

use sango_core::moves::{Category, Move};
use sango_core::SdCard;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

struct Registry {
    list: Vec<sango_core::custom_item::Custom>,
    read_at: Option<Instant>,
    /// The names as texts that live for the whole program: the lists of the pages keep `&'static str`.
    names: HashMap<(u16, String), &'static str>,
}

static REGISTRY: RwLock<Option<Registry>> = RwLock::new(None);

/// Reads the new moves of the SD card again, at most one time each second. Call it at the start of a page that has a menu of moves.
pub fn refresh(sd: Option<&SdCard>) {
    let due = REGISTRY.read().map_or(false, |r| r.as_ref().and_then(|r| r.read_at).map_or(true, |at| at.elapsed() >= Duration::from_secs(1)));
    if !due {
        return;
    }
    let list = sd.map(sango_core::custom_item::moves).unwrap_or_default();
    if let Ok(mut guard) = REGISTRY.write() {
        let registry = guard.get_or_insert_with(|| Registry { list: Vec::new(), read_at: None, names: HashMap::new() });
        registry.read_at = Some(Instant::now());
        registry.list = list;
    }
}

/// The data of a move as the lists show it.
fn info(data: &[u8; 36]) -> Move {
    let category = match data[2] {
        1 => Category::Physical,
        2 => Category::Special,
        _ => Category::Status,
    };
    Move { kind: data[0], category, power: data[3], accuracy: data[4], pp: data[5] }
}

/// The new moves, in the order of their numbers: the number, the English name and the data that the lists show.
pub fn all() -> Vec<(u16, String, Move)> {
    REGISTRY.read().ok().and_then(|r| r.as_ref().map(|r| r.list.iter().map(|c| (c.id as u16, c.name.clone(), info(&c.data))).collect())).unwrap_or_default()
}

/// The data of a new move, or nothing for another number.
pub fn info_of(id: u16) -> Option<Move> {
    REGISTRY.read().ok()?.as_ref()?.list.iter().find(|c| c.id == id as usize).map(|c| info(&c.data))
}

/// The English name of a new move, or nothing for another number.
pub fn name_of(id: u16) -> Option<&'static str> {
    if (id as usize) < sango_core::custom_item::FIRST_ID {
        return None;
    }
    let name = REGISTRY.read().ok()?.as_ref()?.list.iter().find(|c| c.id == id as usize)?.name.clone();
    let mut guard = REGISTRY.write().ok()?;
    let registry = guard.as_mut()?;
    // A name is kept once for the whole program: there are 32 new moves at most, and a few names for each.
    let key = (id, name);
    if let Some(found) = registry.names.get(&key) {
        return Some(found);
    }
    let leaked: &'static str = Box::leak(key.1.clone().into_boxed_str());
    registry.names.insert(key, leaked);
    Some(leaked)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_new_move_has_a_name_and_data_in_the_lists() {
        // The registry is shared: this test is the only one that fills it.
        let folder = std::env::temp_dir().join(format!("sango_species_custom_moves_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut bytes = b"SMVR".to_vec();
        bytes.extend_from_slice(&[1, 132, 3, 0]);
        for language in 0..8 {
            for text in 0..2 {
                let content = if language == 2 && text == 0 { "Thunderclap" } else { "x" };
                let units: Vec<u16> = content.encode_utf16().collect();
                bytes.extend_from_slice(&(units.len() as u16).to_le_bytes());
                for unit in units {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
        }
        let mut data = [0u8; 36];
        data[0] = 12;
        data[2] = 2;
        data[3] = 90;
        data[4] = 100;
        data[5] = 15;
        bytes.extend_from_slice(&data);
        sd.write("move/custom/900.bin", &bytes).unwrap();
        refresh(Some(&sd));
        assert_eq!(name_of(900), Some("Thunderclap"));
        assert_eq!(name_of(901), None);
        let found = info_of(900).unwrap();
        assert_eq!((found.kind, found.category, found.power, found.accuracy, found.pp), (12, Category::Special, 90, 100, 15));
        assert_eq!(all().len(), 1);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
