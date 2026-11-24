//! The rules that replace the code of a move or of an item of the game in battle.
//!
//! The game runs its own code for the special effects of some moves (Hex, Weather Ball...) and of some held items (Leftovers, Life
//! Orb...). The Reactions tab shows this code as rules when the rules can do the same thing (see [`crate::presets`]), and else says
//! why (see [`crate::reasons`]). Without a file, the game runs its code. When the user changes the rules and saves, the Studio writes
//! the file and the plugin runs the rules in place of the code of the game.
//!
//! # The file `<folder>/<id>.bin`
//!
//! All the numbers are little-endian.
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | The magic text: `SMRL` for a move, `SIRL` for an item. |
//! | 4 | 1 | The version of the layout: 1. |
//! | 5 | 2 | The id of the move or of the item. |
//! | 7 | 1 | The number of rules. |
//! | 8 | … | The rules (see the layout in the root of this crate). |
//!
//! # The file `<folder>/list.bin`
//!
//! The plugin cannot list a folder: this file lists the ids that have a file. The magic text (`SMLS` for the moves, `SILS` for the
//! items), the version 1, three bytes 0, then 128 bytes with one bit for each id (the bit `id % 8` of the byte `id / 8`).

use crate::{read_rules, rule_sentence, write_rules, Context, Reader, Rule};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::BTreeMap;

const VERSION: u8 = 1;
/// The ids that the list can hold.
const LIST_IDS: usize = 1024;

/// What changes between the files of the moves and the files of the items.
pub struct Kind {
    /// The folder on the SD card.
    pub folder: &'static str,
    pub magic: &'static [u8; 4],
    pub list_magic: &'static [u8; 4],
    pub context: Context,
    /// The first id of a new move or item: an id of the game is smaller.
    pub first_new_id: u16,
    /// The most files: the plugin keeps this number at the same time.
    pub max: usize,
    /// "move" or "item", for the messages.
    pub what: &'static str,
}

/// The rules of the moves of the game.
pub const MOVES: Kind = Kind { folder: "move/rules", magic: b"SMRL", list_magic: b"SMLS", context: Context::Move, first_new_id: 900, max: 16, what: "move" };

/// The rules of the held items of the game.
pub const ITEMS: Kind = Kind { folder: "item/rules", magic: b"SIRL", list_magic: b"SILS", context: Context::Item, first_new_id: 800, max: 16, what: "item" };

impl Kind {
    /// The place of the file of an id.
    pub fn path(&self, id: u16) -> String {
        format!("{}/{id}.bin", self.folder)
    }

    /// The place of the list.
    pub fn list_path(&self) -> String {
        format!("{}/list.bin", self.folder)
    }

    /// The file of the rules of an id.
    pub fn to_bytes(&self, id: u16, rules: &[Rule]) -> Vec<u8> {
        let mut out = self.magic.to_vec();
        out.push(VERSION);
        out.extend_from_slice(&id.to_le_bytes());
        out.push(rules.len() as u8);
        write_rules(&mut out, rules);
        out
    }

    /// Reads a file of rules: the id and the rules.
    pub fn from_bytes(&self, bytes: &[u8]) -> Result<(u16, Vec<Rule>), String> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != self.magic {
            return Err(format!("This is not a file of the rules of a {}.", self.what));
        }
        if reader.byte()? != VERSION {
            return Err("The version of the file is not known.".into());
        }
        let id = reader.word()?;
        let count = reader.byte()? as usize;
        Ok((id, read_rules(&mut reader, count)?))
    }

    /// The file of the list for these ids.
    pub fn list_bytes(&self, ids: &[u16]) -> Vec<u8> {
        let mut out = self.list_magic.to_vec();
        out.extend_from_slice(&[VERSION, 0, 0, 0]);
        let mut bits = [0u8; LIST_IDS / 8];
        for &id in ids.iter().filter(|id| (**id as usize) < LIST_IDS) {
            bits[id as usize / 8] |= 1 << (id % 8);
        }
        out.extend_from_slice(&bits);
        out
    }

    /// What is wrong with the rules of an id. An empty list means the file can be written.
    pub fn problems(&self, id: u16, rules: &[Rule]) -> Vec<String> {
        let mut problems = Vec::new();
        if id == 0 || id >= self.first_new_id {
            problems.push(format!("The {} {id} is not a {} of the game.", self.what, self.what));
        }
        problems.extend(crate::problems(rules, self.context));
        problems
    }
}

/// The rules of the moves or of the items of the game, as the user has them and as they are on the SD card.
pub struct Store {
    kind: &'static Kind,
    list: BTreeMap<u16, Vec<Rule>>,
    saved: BTreeMap<u16, Vec<Rule>>,
    loaded: bool,
    root: Option<std::path::PathBuf>,
}

impl Store {
    pub fn new(kind: &'static Kind) -> Store {
        Store { kind, list: BTreeMap::new(), saved: BTreeMap::new(), loaded: false, root: None }
    }

    pub fn kind(&self) -> &'static Kind {
        self.kind
    }

    /// Reads the files of the SD card the first time, and when the folder of the card changes.
    pub fn ensure(&mut self, sd: Option<&SdCard>) {
        let root = sd.map(|sd| sd.local().to_path_buf());
        if self.loaded && self.root == root {
            return;
        }
        self.root = root;
        self.list.clear();
        if let Some(sd) = sd {
            for file in sd.files_in(self.kind.folder) {
                if let Some((id, rules)) = sd.read(&file).and_then(|bytes| self.kind.from_bytes(&bytes).ok()) {
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

    /// The ids that have rules.
    pub fn ids(&self) -> Vec<u16> {
        self.list.keys().copied().collect()
    }

    /// Puts the rules of an id in the list: they are saved with the next "Save".
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
        if self.list.len() > self.kind.max {
            return Err(format!("The plugin keeps the rules of {} {}s of the game at most.", self.kind.max, self.kind.what));
        }
        let mut written = 0;
        for (id, rules) in &self.list {
            if self.saved.get(id) == Some(rules) {
                continue;
            }
            if let Some(first) = self.kind.problems(*id, rules).first() {
                return Err(format!("{} {id}: {first}", self.kind.what));
            }
            let file = self.kind.path(*id);
            sd.write(&file, &self.kind.to_bytes(*id, rules)).and_then(|_| sd.mark_changed(&file))?;
            written += 1;
        }
        self.saved = self.list.clone();
        self.write_list(sd)?;
        Ok(written)
    }

    /// Deletes the rules of an id: the game runs its own code again.
    pub fn delete(&mut self, sd: &SdCard, id: u16) -> Result<(), String> {
        let file = self.kind.path(id);
        sd.remove(&file).and_then(|_| sd.mark_changed(&file))?;
        self.list.remove(&id);
        self.saved.remove(&id);
        self.write_list(sd)
    }

    fn write_list(&self, sd: &SdCard) -> Result<(), String> {
        let ids: Vec<u16> = self.saved.keys().copied().collect();
        let path = self.kind.list_path();
        if ids.is_empty() {
            sd.remove(&path).and_then(|_| sd.mark_changed(&path))
        } else {
            sd.write(&path, &self.kind.list_bytes(&ids)).and_then(|_| sd.mark_changed(&path))
        }
    }
}

/// The editor of the rules of an id of the game, and the summary.
pub fn rules_ui(ui: &mut egui::Ui, kind: &Kind, id: u16, rules: &mut Vec<Rule>) {
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title(&format!("What the {} does", kind.what)));
        let owner = match kind.context {
            Context::Item => "the Pokémon that holds the item",
            _ => "the Pokémon that uses the move",
        };
        ui.label(widget::hint(&format!("Each reaction says: when this happens, if all these are true, do these things. \"This Pokémon\" is {owner}.")));
        crate::editor::rules_editor(ui, &format!("game_{}_{id}", kind.what), rules, kind.context);
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Summary"));
        for rule in rules.iter() {
            ui.label(rule_sentence(rule));
        }
        for problem in kind.problems(id, rules) {
            ui.label(egui::RichText::new(problem).color(color::ROSE).size(13.5));
        }
    });
}

/// The Reactions tab of a move or of an item of the game. Without a file, the game runs its own code: the tab shows `preset`, the
/// code of the game as rules, or else `reason`. The first change of the preset puts the rules in the store: "Save" writes the file,
/// and then the plugin runs the rules in place of the code of the game. Gives a message for the status line, when there is one.
pub fn reactions_tab(ui: &mut egui::Ui, store: &mut Store, draft: &mut Option<(u16, Vec<Rule>)>, id: u16, preset: Option<Vec<Rule>>, reason: &str, sd: Option<&SdCard>) -> Option<(bool, String)> {
    let kind = store.kind;
    let mut back_to_game = false;
    let mut own_rules = false;
    egui::ScrollArea::vertical().id_salt(format!("{}_game_reactions", kind.what)).auto_shrink([false, false]).show(ui, |ui| {
        if let Some(rules) = store.find_mut(id) {
            widget::card(ui, |ui| {
                ui.set_min_width(700.0);
                ui.label(widget::title("Your rules replace the code of the game"));
                ui.label(widget::hint(&format!("The file {} is on the SD card (or is written at the next \"Save\"): in battle, the plugin runs these rules and not the code of the game. The data of the {} stays. Omega Ruby and Alpha Sapphire only.", kind.path(id), kind.what)));
                ui.add_space(4.0);
                back_to_game = sd.is_some() && widget::button(ui, "Back to the code of the game");
            });
            ui.add_space(10.0);
            rules_ui(ui, kind, id, rules);
            return;
        }
        if draft.as_ref().map(|d| d.0) != Some(id) {
            *draft = preset.clone().map(|rules| (id, rules));
        }
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            if preset.is_some() {
                ui.label(widget::title("The game runs its own code"));
                ui.label(widget::hint(&format!("Below is what the code of the game does, written as rules. Change a value: at the next \"Save\", the Studio writes the file {} and the plugin runs the rules in place of the code of the game.", kind.path(id))));
            } else {
                ui.label(widget::title(&format!("No reaction as rules for this {}", kind.what)));
                ui.label(egui::RichText::new(reason).color(color::AMBER));
                ui.add_space(4.0);
                ui.label(widget::hint(&format!("You can give the {} your own rules. With code of the game, it then loses this code; its data stays.", kind.what)));
                ui.add_space(4.0);
                own_rules = sd.is_some() && widget::button(ui, "Add my own rules");
            }
        });
        if let Some((_, rules)) = draft.as_mut() {
            ui.add_space(10.0);
            rules_ui(ui, kind, id, rules);
        }
    });
    if let (Some((draft_id, rules)), Some(preset)) = (draft.as_ref(), &preset) {
        if *draft_id == id && rules != preset {
            store.insert(id, rules.clone());
            *draft = None;
        }
    }
    if own_rules {
        let mut rule = Rule::new();
        if kind.context == Context::Move {
            rule.moment = 36;
            rule.conditions.clear();
            rule.actions[0].target = 3;
        }
        store.insert(id, vec![rule]);
    }
    if back_to_game {
        *draft = None;
        if let Some(sd) = sd {
            return Some(match store.delete(sd, id) {
                Ok(()) => (true, format!("The {} {id} runs the code of the game again.", kind.what)),
                Err(message) => (false, message),
            });
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_file_reads_back_the_same() {
        let rules = crate::presets::game_move_rules(311).unwrap();
        let bytes = MOVES.to_bytes(311, &rules);
        assert_eq!(&bytes[..8], &[b'S', b'M', b'R', b'L', 1, 55, 1, 7]);
        assert_eq!(MOVES.from_bytes(&bytes).unwrap(), (311, rules));
        assert!(MOVES.from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(ITEMS.from_bytes(&bytes).is_err(), "a file of a move is not a file of an item");
        assert!(MOVES.problems(311, &[]).is_empty());
        assert!(!MOVES.problems(900, &[]).is_empty());
        assert!(!ITEMS.problems(800, &[]).is_empty());
    }

    #[test]
    fn the_list_has_one_bit_for_each_id() {
        let bytes = MOVES.list_bytes(&[1, 311, 621]);
        assert_eq!(bytes.len(), 8 + 128);
        assert_eq!(&bytes[..4], b"SMLS");
        assert_eq!(bytes[8], 0b10);
        assert_eq!(bytes[8 + 38], 1 << 7);
        assert_eq!(bytes[8 + 77], 1 << 5);
        assert_eq!(&ITEMS.list_bytes(&[234])[..4], b"SILS");
    }

    #[test]
    fn saving_and_deleting_write_the_list() {
        let folder = std::env::temp_dir().join(format!("sango_game_rules_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        for kind in [&MOVES, &ITEMS] {
            let id = if kind.context == Context::Move { 506 } else { 234 };
            let mut store = Store::new(kind);
            store.ensure(Some(&sd));
            store.insert(id, vec![Rule::new()]);
            assert!(store.changed());
            assert_eq!(store.save(&sd).unwrap(), 1);
            assert_eq!(sd.read(&kind.list_path()).unwrap(), kind.list_bytes(&[id]));
            let mut other = Store::new(kind);
            other.ensure(Some(&sd));
            assert_eq!(other.ids(), vec![id]);
            other.delete(&sd, id).unwrap();
            assert!(sd.read(&kind.path(id)).is_none() && sd.read(&kind.list_path()).is_none());
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
