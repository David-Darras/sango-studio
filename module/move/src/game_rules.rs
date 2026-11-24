//! The rules that replace the code of a move of the game in battle.
//!
//! The game runs its own code for the special effects of some moves (Hex, Weather Ball, Sonic Boom...). The Reactions tab of a move of
//! the game shows this code as rules when the rules can do the same thing (see `sango_rules::presets`). Without a file, the game runs
//! its code. When the user changes the rules and saves, the Studio writes the file and the plugin runs the rules in place of the code
//! of the game. A move of the game without code of the game can get rules too.
//!
//! # The file `move/rules/<id>.bin`
//!
//! All the numbers are little-endian.
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | The text `SMRL`. |
//! | 4 | 1 | The version of the layout: 1. |
//! | 5 | 2 | The id of the move (1 to 899). |
//! | 7 | 1 | The number of rules. |
//! | 8 | … | The rules (see the crate `sango-rules`). |
//!
//! # The file `move/rules/list.bin`
//!
//! The plugin cannot list a folder: this file lists the moves that have a file. The text `SMLS`, the version 1, three bytes 0, then
//! 128 bytes with one bit for each move (the bit `id % 8` of the byte `id / 8`).

use sango_core::SdCard;
use sango_rules::{read_rules, write_rules, Context, Reader, Rule};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::BTreeMap;

const MAGIC: &[u8; 4] = b"SMRL";
const LIST_MAGIC: &[u8; 4] = b"SMLS";
const VERSION: u8 = 1;
/// The folder of the files on the SD card.
const FOLDER: &str = "move/rules";
/// The list of the moves that have a file.
pub const LIST_PATH: &str = "move/rules/list.bin";
/// The ids that the list can hold.
const LIST_IDS: usize = 1024;
/// The first id of a new move: a move of the game has a smaller id.
const FIRST_NEW_ID: u16 = 900;
/// The most moves of the game with rules: the plugin keeps this number at the same time.
pub const MAX_MOVES: usize = 16;

/// The place of the file of a move.
pub fn path(id: u16) -> String {
    format!("{FOLDER}/{id}.bin")
}

/// The file of the rules of a move of the game.
pub fn to_bytes(id: u16, rules: &[Rule]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(VERSION);
    out.extend_from_slice(&id.to_le_bytes());
    out.push(rules.len() as u8);
    write_rules(&mut out, rules);
    out
}

/// Reads a file of rules: the id and the rules.
pub fn from_bytes(bytes: &[u8]) -> Result<(u16, Vec<Rule>), String> {
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != MAGIC {
        return Err("This is not a file of the rules of a move: the text SMRL is missing.".into());
    }
    if reader.byte()? != VERSION {
        return Err("The version of the file is not known.".into());
    }
    let id = reader.word()?;
    let count = reader.byte()? as usize;
    Ok((id, read_rules(&mut reader, count)?))
}

/// The file `move/rules/list.bin` for these moves.
pub fn list_bytes(ids: &[u16]) -> Vec<u8> {
    let mut out = LIST_MAGIC.to_vec();
    out.extend_from_slice(&[VERSION, 0, 0, 0]);
    let mut bits = [0u8; LIST_IDS / 8];
    for &id in ids.iter().filter(|id| (**id as usize) < LIST_IDS) {
        bits[id as usize / 8] |= 1 << (id % 8);
    }
    out.extend_from_slice(&bits);
    out
}

/// What is wrong with the rules of a move of the game. An empty list means the file can be written.
pub fn problems(id: u16, rules: &[Rule]) -> Vec<String> {
    let mut problems = Vec::new();
    if id == 0 || id >= FIRST_NEW_ID {
        problems.push(format!("The move {id} is not a move of the game."));
    }
    problems.extend(sango_rules::problems(rules, Context::Move));
    problems
}

/// The rules of the moves of the game, as the user has them and as they are on the SD card.
#[derive(Default)]
pub struct Store {
    list: BTreeMap<u16, Vec<Rule>>,
    saved: BTreeMap<u16, Vec<Rule>>,
    loaded: bool,
    root: Option<std::path::PathBuf>,
}

impl Store {
    /// Reads the files of the SD card the first time, and when the folder of the card changes.
    pub fn ensure(&mut self, sd: Option<&SdCard>) {
        let root = sd.map(|sd| sd.local().to_path_buf());
        if self.loaded && self.root == root {
            return;
        }
        self.root = root;
        self.list.clear();
        if let Some(sd) = sd {
            for file in sd.files_in(FOLDER) {
                if let Some((id, rules)) = sd.read(&file).and_then(|bytes| from_bytes(&bytes).ok()) {
                    self.list.insert(id, rules);
                }
            }
        }
        self.saved = self.list.clone();
        self.loaded = true;
    }

    pub fn find_mut(&mut self, id: u16) -> Option<&mut Vec<Rule>> {
        self.list.get_mut(&id)
    }

    /// The moves that have rules.
    pub fn ids(&self) -> Vec<u16> {
        self.list.keys().copied().collect()
    }

    /// Puts the rules of a move in the list: they are saved with the next "Save".
    pub fn insert(&mut self, id: u16, rules: Vec<Rule>) {
        self.list.insert(id, rules);
    }

    /// True when the rules changed since they were read or saved.
    pub fn changed(&self) -> bool {
        self.list != self.saved
    }

    /// Writes the rules that changed and the list, and tells the plugin. Gives the number of files that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<usize, String> {
        if !self.changed() {
            return Ok(0);
        }
        if self.list.len() > MAX_MOVES {
            return Err(format!("The plugin keeps the rules of {MAX_MOVES} moves of the game at most."));
        }
        let mut written = 0;
        for (id, rules) in &self.list {
            if self.saved.get(id) == Some(rules) {
                continue;
            }
            if let Some(first) = problems(*id, rules).first() {
                return Err(format!("Move {id}: {first}"));
            }
            let file = path(*id);
            sd.write(&file, &to_bytes(*id, rules)).and_then(|_| sd.mark_changed(&file))?;
            written += 1;
        }
        self.saved = self.list.clone();
        self.write_list(sd)?;
        Ok(written)
    }

    /// Deletes the rules of a move: the game runs its own code again.
    pub fn delete(&mut self, sd: &SdCard, id: u16) -> Result<(), String> {
        let file = path(id);
        sd.remove(&file).and_then(|_| sd.mark_changed(&file))?;
        self.list.remove(&id);
        self.saved.remove(&id);
        self.write_list(sd)
    }

    fn write_list(&self, sd: &SdCard) -> Result<(), String> {
        let ids: Vec<u16> = self.saved.keys().copied().collect();
        if ids.is_empty() {
            sd.remove(LIST_PATH).and_then(|_| sd.mark_changed(LIST_PATH))
        } else {
            sd.write(LIST_PATH, &list_bytes(&ids)).and_then(|_| sd.mark_changed(LIST_PATH))
        }
    }
}

/// The editor of the rules of a move of the game, and the summary. `id` tells the editor from the others.
pub fn rules_ui(ui: &mut egui::Ui, id: u16, rules: &mut Vec<Rule>) {
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("What the move does"));
        ui.label(widget::hint("Each reaction says: when this happens, if all these are true, do these things. \"This Pokémon\" is the Pokémon that uses the move."));
        sango_rules::editor::rules_editor(ui, &format!("game_move_{id}"), rules, Context::Move);
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Summary"));
        for rule in rules.iter() {
            ui.label(sango_rules::rule_sentence(rule));
        }
        for problem in problems(id, rules) {
            ui.label(egui::RichText::new(problem).color(color::ROSE).size(13.5));
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_file_reads_back_the_same() {
        let rules = sango_rules::presets::game_move_rules(311).unwrap();
        let bytes = to_bytes(311, &rules);
        assert_eq!(&bytes[..8], &[b'S', b'M', b'R', b'L', 1, 55, 1, 7]);
        assert_eq!(from_bytes(&bytes).unwrap(), (311, rules));
        assert!(from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(problems(311, &[]).is_empty());
        assert!(!problems(900, &[]).is_empty());
    }

    #[test]
    fn the_list_has_one_bit_for_each_move() {
        let bytes = list_bytes(&[1, 311, 621]);
        assert_eq!(bytes.len(), 8 + 128);
        assert_eq!(bytes[8], 0b10);
        assert_eq!(bytes[8 + 38], 1 << 7);
        assert_eq!(bytes[8 + 77], 1 << 5);
    }

    #[test]
    fn saving_and_deleting_write_the_list() {
        let folder = std::env::temp_dir().join(format!("sango_move_rules_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut store = Store::default();
        store.ensure(Some(&sd));
        store.insert(506, sango_rules::presets::game_move_rules(506).unwrap());
        assert!(store.changed());
        assert_eq!(store.save(&sd).unwrap(), 1);
        assert_eq!(sd.read(LIST_PATH).unwrap(), list_bytes(&[506]));
        let mut other = Store::default();
        other.ensure(Some(&sd));
        assert_eq!(other.ids(), vec![506]);
        other.delete(&sd, 506).unwrap();
        assert!(sd.read(&path(506)).is_none() && sd.read(LIST_PATH).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
