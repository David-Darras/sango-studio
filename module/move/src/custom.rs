//! The new moves: the Move page lists them with the moves of the game and edits their data and their texts with the same menus. Only
//! the reactions have their own tab (see the crate `sango-rules`). The page writes one file for each move on the SD card.
//!
//! # The file `move/custom/<id>.bin`
//!
//! All the numbers are little-endian.
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | The text `SMVR`. |
//! | 4 | 1 | The version of the layout: 1. |
//! | 5 | 2 | The id of the move (900 to 931). |
//! | 7 | 1 | The number of rules. |
//! | 8 | … | For each of the eight languages, the name then the description (a 16-bit length, then the UTF-16 units). |
//! | … | 36 | The data of the move (see `data.rs`). |
//! | … | … | The rules. |

use crate::data::{self, MoveData};
use sango_core::text::LANGUAGES;
use sango_core::SdCard;
use sango_rules::{read_rules, write_rules, write_text, Context, Reader, Rule};
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The first id and the last id of a new move.
pub const FIRST_ID: u16 = 900;
pub const LAST_ID: u16 = 931;
/// The most new moves: the plugin keeps this number of moves of the studio at the same time.
pub const MAX_MOVES: usize = 32;
const MAGIC: &[u8; 4] = b"SMVR";
const VERSION: u8 = 1;
/// The folder of the files on the SD card.
const FOLDER: &str = "move/custom";
/// The English language: its place in [`LANGUAGES`].
pub const ENGLISH: usize = 2;

/// The data of Pound: a new move starts like it.
pub const POUND: [u8; data::SIZE] = [0, 0, 1, 40, 100, 35, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 83, 83, 73, 0, 0, 0];

/// A new move: the id, the texts in the eight languages, the data and the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomMove {
    pub id: u16,
    pub names: [String; 8],
    pub descriptions: [String; 8],
    pub data: MoveData,
    pub rules: Vec<Rule>,
}

impl CustomMove {
    /// A new move: the English text goes to every language, and the data is the one of Pound.
    pub fn new(id: u16, name: &str, description: &str) -> CustomMove {
        let mut rule = Rule::new();
        rule.moment = 36;
        rule.conditions.clear();
        rule.actions[0].target = 3;
        CustomMove { id, names: std::array::from_fn(|_| name.to_string()), descriptions: std::array::from_fn(|_| description.to_string()), data: MoveData { bytes: POUND }, rules: vec![rule] }
    }

    /// The place of the file on the SD card.
    pub fn path(id: u16) -> String {
        format!("{FOLDER}/{id}.bin")
    }

    /// The first id from 900 that is not in `used`.
    pub fn free_id(used: &[u16]) -> Option<u16> {
        (FIRST_ID..=LAST_ID).find(|id| !used.contains(id))
    }

    /// What is wrong with the move, in words that the user understands. An empty list means the file can be written.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(FIRST_ID..=LAST_ID).contains(&self.id) {
            problems.push(format!("The id must be from {FIRST_ID} to {LAST_ID}."));
        }
        if self.names[ENGLISH].trim().is_empty() {
            problems.push("The English name is empty.".into());
        }
        // A move with no reaction is a move of the game that only has its data: this is allowed.
        problems.extend(sango_rules::problems(&self.rules, Context::Move));
        problems
    }

    /// The file. The move must have no problem.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.push(self.rules.len() as u8);
        for language in 0..8 {
            write_text(&mut out, &self.names[language]);
            write_text(&mut out, &self.descriptions[language]);
        }
        out.extend_from_slice(&self.data.bytes);
        write_rules(&mut out, &self.rules);
        out
    }

    /// Reads a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<CustomMove, String> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != MAGIC {
            return Err("This is not a file of a new move: the text SMVR is missing.".into());
        }
        let version = reader.byte()?;
        if version != VERSION {
            return Err(format!("The version {version} of the file is not known."));
        }
        let id = reader.word()?;
        let count = reader.byte()? as usize;
        let mut names: [String; 8] = Default::default();
        let mut descriptions: [String; 8] = Default::default();
        for language in 0..8 {
            names[language] = reader.text()?;
            descriptions[language] = reader.text()?;
        }
        let data = MoveData::from_bytes(reader.take(data::SIZE)?)?;
        let rules = read_rules(&mut reader, count)?;
        Ok(CustomMove { id, names, descriptions, data, rules })
    }
}

/// The new moves of the user. The Move page lists them with the moves of the game, and edits them with the same menus.
#[derive(Default)]
pub struct Store {
    /// The moves, as the user has them now.
    list: Vec<CustomMove>,
    /// The moves as they are on the SD card: to know what changed.
    saved: Vec<CustomMove>,
    loaded: bool,
    root: Option<std::path::PathBuf>,
}

impl Store {
    /// Reads the files of the SD card the first time, and when the folder of the card changes.
    pub fn ensure(&mut self, sd: Option<&SdCard>) {
        let root = sd.map(|sd| sd.root().to_path_buf());
        if self.loaded && self.root == root {
            return;
        }
        self.root = root;
        self.reload(sd);
    }

    /// Reads the files of the SD card again.
    pub fn reload(&mut self, sd: Option<&SdCard>) {
        self.list.clear();
        if let Some(sd) = sd {
            for path in sd.files_in(FOLDER) {
                if !path.ends_with(".bin") {
                    continue;
                }
                if let Some(item) = sd.read(&path).and_then(|bytes| CustomMove::from_bytes(&bytes).ok()) {
                    self.list.push(item);
                }
            }
        }
        self.list.sort_by_key(|m| m.id);
        self.saved = self.list.clone();
        self.loaded = true;
    }

    /// The numbers of the new moves.
    pub fn ids(&self) -> Vec<usize> {
        self.list.iter().map(|m| m.id as usize).collect()
    }

    pub fn find(&self, id: usize) -> Option<&CustomMove> {
        self.list.iter().find(|m| m.id as usize == id)
    }

    pub fn find_mut(&mut self, id: usize) -> Option<&mut CustomMove> {
        self.list.iter_mut().find(|m| m.id as usize == id)
    }

    /// True when a new move changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.list != self.saved
    }

    /// Makes a new move with the next free number. It starts with the data of `template` (the move that is open), or with the
    /// data of Pound. Gives its number.
    pub fn add(&mut self, template: Option<&MoveData>) -> Result<usize, String> {
        if self.list.len() >= MAX_MOVES {
            return Err(format!("The plugin keeps {MAX_MOVES} new moves at most: delete one first."));
        }
        let used: Vec<u16> = self.list.iter().map(|m| m.id).collect();
        let id = CustomMove::free_id(&used).ok_or("All the ids from 900 to 931 are used.")?;
        let mut item = CustomMove::new(id, "New move", "");
        if let Some(data) = template {
            item.data = data.clone();
        }
        self.list.push(item);
        self.list.sort_by_key(|m| m.id);
        Ok(id as usize)
    }

    /// Writes the new moves that changed, and tells the plugin. Gives the number of files that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<usize, String> {
        let mut written = 0;
        for item in &self.list {
            if self.saved.iter().any(|saved| saved == item) {
                continue;
            }
            if let Some(first) = item.problems().first() {
                return Err(format!("#{} {}: {first}", item.id, item.names[ENGLISH]));
            }
            let path = CustomMove::path(item.id);
            sd.write(&path, &item.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            written += 1;
        }
        self.saved = self.list.clone();
        Ok(written)
    }

    /// Deletes a new move: its file too.
    pub fn delete(&mut self, sd: &SdCard, id: usize) -> Result<(), String> {
        let id16 = id as u16;
        let path = CustomMove::path(id16);
        sd.remove(&path).and_then(|_| sd.mark_changed(&path))?;
        self.list.retain(|m| m.id != id16);
        self.saved.retain(|m| m.id != id16);
        Ok(())
    }
}

/// The name and the description of a new move in a language: the Text tab of a new move.
pub fn texts_ui(ui: &mut egui::Ui, item: &mut CustomMove, language: &mut usize) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (i, entry) in LANGUAGES.iter().enumerate() {
            if widget::tab_button(ui, entry.name, *language == i, false).clicked() {
                *language = i;
            }
        }
    });
    ui.add_space(8.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::hint("A new move starts with the same text in every language: write the text of this language."));
        ui.add_space(6.0);
        let language = (*language).min(7);
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut item.names[language]).desired_width(300.0));
        });
        ui.label("Description");
        ui.add(egui::TextEdit::multiline(&mut item.descriptions[language]).desired_rows(3).desired_width(520.0));
    });
}

/// What a new move does: its reactions. The Reactions tab of a new move.
pub fn reactions_ui(ui: &mut egui::Ui, item: &mut CustomMove) {
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("What the move does"));
        ui.label(widget::hint("Each reaction says: when this happens, if all these are true, do these things. The damage and the status of the move come from its data (General tab); reactions add effects (a stat change, a weather...). \"This Pokémon\" is the one that used the move."));
        sango_rules::editor::rules_editor(ui, &format!("move_{}", item.id), &mut item.rules, Context::Move);
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Summary"));
        for rule in &item.rules {
            ui.label(sango_rules::rule_sentence(rule));
        }
        let problems = item.problems();
        if !problems.is_empty() {
            ui.add_space(6.0);
            for problem in problems {
                ui.label(egui::RichText::new(problem).color(color::ROSE).size(13.5));
            }
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_rules::{Action, ActionKind, Condition, ConditionKind};

    fn folder(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sango_move_custom_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_move_goes_out_and_comes_back_with_the_documented_layout() {
        let mut item = CustomMove::new(900, "Thunderclap", "A fast electric attack.");
        item.names[3] = "Coup de tonnerre".into();
        item.data.set("power", 80);
        item.data.set("type", 12);
        item.rules.push(Rule { moment: 140, conditions: vec![Condition { kind: ConditionKind::TargetFainted, arg: 0 }], actions: vec![Action::new(ActionKind::SetWeather)] });
        assert!(item.problems().is_empty(), "{:?}", item.problems());
        let bytes = item.to_bytes();
        assert_eq!(&bytes[..8], &[b'S', b'M', b'V', b'R', 1, 132, 3, 2]);
        assert_eq!(CustomMove::from_bytes(&bytes).unwrap(), item);
        assert!(CustomMove::from_bytes(&bytes[..bytes.len() - 2]).is_err());
        assert!(CustomMove::from_bytes(b"SABR0000").is_err());
        // The data is right after the eight pairs of texts: the type and the power are at the offsets 0 and 3.
        let rules_size = 4 + 3 + 7 + 4 + 7;
        let data_at = bytes.len() - rules_size - data::SIZE;
        assert_eq!((bytes[data_at], bytes[data_at + 3]), (12, 80));
    }

    #[test]
    fn adding_a_move_copies_the_data_of_the_open_move_and_the_files_come_back() {
        let path = folder("store");
        let sd = SdCard::open(path.to_str().unwrap()).unwrap();
        let mut store = Store::default();
        store.ensure(Some(&sd));
        let mut template = MoveData { bytes: POUND };
        template.set("power", 95);
        let id = store.add(Some(&template)).unwrap();
        assert_eq!(id, 900);
        assert_eq!(store.find(900).unwrap().data, template);
        assert!(store.changed());
        store.save(&sd).unwrap();
        assert!(!store.changed() && sd.read("move/custom/900.bin").is_some());
        let mut other = Store::default();
        other.ensure(Some(&sd));
        assert_eq!(other.ids(), vec![900]);
        assert_eq!(other.add(None).unwrap(), 901);
        other.delete(&sd, 900).unwrap();
        assert_eq!(other.ids(), vec![901]);
        assert!(sd.read("move/custom/900.bin").is_none());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn the_cards_draw() {
        let mut item = CustomMove::new(900, "Test", "");
        item.rules.push(Rule::new());
        let mut language = 2;
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    texts_ui(ui, &mut item, &mut language);
                    reactions_ui(ui, &mut item);
                });
            });
        }
    }
}
