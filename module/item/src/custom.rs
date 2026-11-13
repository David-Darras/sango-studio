//! The new items: the Item page lists them with the items of the game and edits their data and their texts with the same menus. Only
//! the reactions have their own tab: what the item does when a Pokémon holds it in a battle (rules, see the crate `sango-rules`) and
//! what it does when the player uses it in the Bag (actions). The page writes one file for each item on the SD card.
//!
//! # The file `item/custom/<id>.bin`
//!
//! All the numbers are little-endian.
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | The text `SITM`. |
//! | 4 | 1 | The version of the layout: 1. |
//! | 5 | 2 | The id of the item (900 to 931). |
//! | 7 | 1 | The number of rules. |
//! | 8 | … | For each of the eight languages: the name, the description, the message of the Bag (a 16-bit length, then the UTF-16 units). |
//! | … | 36 | The data of the item (see `data.rs`). |
//! | … | 1 | Bit 0: the player can use the item on a Pokémon in the Bag. |
//! | … | 2 | The item of the game whose picture is shown when the file of the picture is missing (0 for none). |
//! | … | 1 | The number of actions of the Bag (6 at most). |
//! | … | 5 each | An action of the Bag: its kind, then two 16-bit signed numbers `a` and `b`. |
//! | … | … | The rules. |
//!
//! The picture is the file `item/custom/<id>.bclim` (30 × 30 pixels).

use crate::data::{ItemData, SIZE};
use sango_core::text::LANGUAGES;
use sango_core::SdCard;
use sango_rules::{read_rules, write_rules, write_text, Context, Reader, Rule};
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The first id and the last id of a new item.
pub const FIRST_ID: u16 = 900;
pub const LAST_ID: u16 = 931;
/// The most new items: the plugin keeps this number of items of the studio at the same time.
pub const MAX_ITEMS: usize = 32;
/// The most actions of the Bag in one item.
pub const MAX_BAG_ACTIONS: usize = 6;
const MAGIC: &[u8; 4] = b"SITM";
const VERSION: u8 = 1;
/// The folder of the files on the SD card.
const FOLDER: &str = "item/custom";
/// The English language: its place in [`LANGUAGES`].
pub const ENGLISH: usize = 2;

/// The kinds of the actions of the Bag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BagKind {
    /// Changes the HP (a: the change, 32767 for all the HP, a negative number removes HP).
    ChangeHp = 1,
    /// Makes the Pokémon shiny (a: 0 switches, 1 makes it shiny, 2 makes it normal).
    SetShiny = 2,
    /// The Pokémon goes to the next ability of its species.
    NextAbility = 3,
    /// Sets the level (a).
    SetLevel = 4,
    /// Adds EVs (a: the stat, 0 HP, 1 Attack, 2 Defense, 3 Speed, 4 Sp. Atk, 5 Sp. Def; b: the number).
    AddEv = 5,
}

impl BagKind {
    pub const ALL: [BagKind; 5] = [BagKind::ChangeHp, BagKind::SetShiny, BagKind::NextAbility, BagKind::SetLevel, BagKind::AddEv];

    pub fn from_byte(byte: u8) -> Option<BagKind> {
        BagKind::ALL.into_iter().find(|kind| *kind as u8 == byte)
    }

    pub fn label(self) -> &'static str {
        match self {
            BagKind::ChangeHp => "Change the HP",
            BagKind::SetShiny => "Change the shiny state",
            BagKind::NextAbility => "Go to the next ability",
            BagKind::SetLevel => "Set the level",
            BagKind::AddEv => "Add EVs",
        }
    }
}

/// The stats of the action "Add EVs": the number is the one that the file keeps.
const EV_STATS: [(i16, &str); 6] = [(0, "HP"), (1, "Attack"), (2, "Defense"), (3, "Speed"), (4, "Sp. Atk"), (5, "Sp. Def")];

/// A thing that the item does when the player uses it on a Pokémon in the Bag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BagAction {
    pub kind: BagKind,
    pub a: i16,
    pub b: i16,
}

impl BagAction {
    pub fn new(kind: BagKind) -> BagAction {
        let a = match kind {
            BagKind::ChangeHp => 50,
            BagKind::SetShiny => 0,
            BagKind::NextAbility => 0,
            BagKind::SetLevel => 100,
            BagKind::AddEv => 0,
        };
        BagAction { kind, a, b: if kind == BagKind::AddEv { 10 } else { 0 } }
    }

    /// The action as an English phrase.
    pub fn sentence(&self) -> String {
        match self.kind {
            BagKind::ChangeHp if self.a == 32767 => "give all the HP back".into(),
            BagKind::ChangeHp if self.a >= 0 => format!("give {} HP back", self.a),
            BagKind::ChangeHp => format!("remove {} HP", -(self.a as i32)),
            BagKind::SetShiny => match self.a {
                1 => "make the Pokémon shiny".into(),
                2 => "make the Pokémon normal".into(),
                _ => "switch the shiny state of the Pokémon".into(),
            },
            BagKind::NextAbility => "give the Pokémon the next ability of its species".into(),
            BagKind::SetLevel => format!("set the level to {}", self.a),
            BagKind::AddEv => format!("add {} {} EVs", self.b, EV_STATS.iter().find(|(v, _)| *v == self.a).map(|(_, n)| *n).unwrap_or("?")),
        }
    }
}

/// A new item: the id, the texts, the data, the use in the Bag and the rules of a held item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomItem {
    pub id: u16,
    pub names: [String; 8],
    pub descriptions: [String; 8],
    /// The message that the Bag shows when the player uses the item. Empty: the message of the plugin.
    pub messages: [String; 8],
    pub data: ItemData,
    pub can_use_in_bag: bool,
    /// The item of the game whose picture is shown when the file of the picture is missing.
    pub icon_item: u16,
    pub bag_actions: Vec<BagAction>,
    pub rules: Vec<Rule>,
}

impl CustomItem {
    /// A new item: the English text goes to every language, the item is in the pocket Items and costs 1000.
    pub fn new(id: u16, name: &str, description: &str) -> CustomItem {
        let mut data = ItemData { bytes: [0; SIZE] };
        data.set("price", 100);
        data.set("pocket", 0);
        CustomItem {
            id,
            names: std::array::from_fn(|_| name.to_string()),
            descriptions: std::array::from_fn(|_| description.to_string()),
            messages: Default::default(),
            data,
            can_use_in_bag: false,
            icon_item: 0,
            bag_actions: Vec::new(),
            rules: Vec::new(),
        }
    }

    /// The place of the file on the SD card.
    pub fn path(id: u16) -> String {
        format!("{FOLDER}/{id}.bin")
    }

    /// The place of the picture on the SD card.
    pub fn picture_path(id: u16) -> String {
        format!("{FOLDER}/{id}.bclim")
    }

    /// The first id from 900 that is not in `used`.
    pub fn free_id(used: &[u16]) -> Option<u16> {
        (FIRST_ID..=LAST_ID).find(|id| !used.contains(id))
    }

    /// What is wrong with the item, in words that the user understands. An empty list means the file can be written.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(FIRST_ID..=LAST_ID).contains(&self.id) {
            problems.push(format!("The id must be from {FIRST_ID} to {LAST_ID}."));
        }
        if self.names[ENGLISH].trim().is_empty() {
            problems.push("The English name is empty.".into());
        }
        if self.bag_actions.len() > MAX_BAG_ACTIONS {
            problems.push(format!("The use in the Bag has {MAX_BAG_ACTIONS} actions at most."));
        }
        if self.can_use_in_bag && self.bag_actions.is_empty() {
            problems.push("The item can be used in the Bag but has no action there: add one, or switch the use off.".into());
        }
        problems.extend(sango_rules::problems(&self.rules, Context::Item));
        problems
    }

    /// The file. The item must have no problem.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&self.id.to_le_bytes());
        out.push(self.rules.len() as u8);
        for language in 0..8 {
            write_text(&mut out, &self.names[language]);
            write_text(&mut out, &self.descriptions[language]);
            write_text(&mut out, &self.messages[language]);
        }
        out.extend_from_slice(&self.data.bytes);
        out.push(self.can_use_in_bag as u8);
        out.extend_from_slice(&self.icon_item.to_le_bytes());
        out.push(self.bag_actions.len() as u8);
        for action in &self.bag_actions {
            out.push(action.kind as u8);
            out.extend_from_slice(&action.a.to_le_bytes());
            out.extend_from_slice(&action.b.to_le_bytes());
        }
        write_rules(&mut out, &self.rules);
        out
    }

    /// Reads a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<CustomItem, String> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != MAGIC {
            return Err("This is not a file of a new item: the text SITM is missing.".into());
        }
        let version = reader.byte()?;
        if version != VERSION {
            return Err(format!("The version {version} of the file is not known."));
        }
        let id = reader.word()?;
        let count = reader.byte()? as usize;
        let mut names: [String; 8] = Default::default();
        let mut descriptions: [String; 8] = Default::default();
        let mut messages: [String; 8] = Default::default();
        for language in 0..8 {
            names[language] = reader.text()?;
            descriptions[language] = reader.text()?;
            messages[language] = reader.text()?;
        }
        let data = ItemData::from_bytes(reader.take(SIZE)?)?;
        let flags = reader.byte()?;
        let icon_item = reader.word()?;
        let action_count = reader.byte()? as usize;
        let mut bag_actions = Vec::new();
        for _ in 0..action_count {
            let kind = reader.byte()?;
            let kind = BagKind::from_byte(kind).ok_or_else(|| format!("The action {kind} of the Bag is not known."))?;
            let (a, b) = (reader.word()? as i16, reader.word()? as i16);
            bag_actions.push(BagAction { kind, a, b });
        }
        let rules = read_rules(&mut reader, count)?;
        Ok(CustomItem { id, names, descriptions, messages, data, can_use_in_bag: flags & 1 != 0, icon_item, bag_actions, rules })
    }
}

/// The new items of the user. The Item page lists them with the items of the game, and edits them with the same menus.
#[derive(Default)]
pub struct Store {
    /// The items, as the user has them now.
    list: Vec<CustomItem>,
    /// The items as they are on the SD card: to know what changed.
    saved: Vec<CustomItem>,
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
                if let Some(item) = sd.read(&path).and_then(|bytes| CustomItem::from_bytes(&bytes).ok()) {
                    self.list.push(item);
                }
            }
        }
        self.list.sort_by_key(|i| i.id);
        self.saved = self.list.clone();
        self.loaded = true;
    }

    /// The numbers of the new items.
    pub fn ids(&self) -> Vec<usize> {
        self.list.iter().map(|i| i.id as usize).collect()
    }

    pub fn find(&self, id: usize) -> Option<&CustomItem> {
        self.list.iter().find(|i| i.id as usize == id)
    }

    pub fn find_mut(&mut self, id: usize) -> Option<&mut CustomItem> {
        self.list.iter_mut().find(|i| i.id as usize == id)
    }

    /// True when a new item changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.list != self.saved
    }

    /// Makes a new item with the next free number. It starts with the data of `template` (the item that is open), or with the
    /// default data. Gives its number.
    pub fn add(&mut self, template: Option<(&ItemData, u16)>) -> Result<usize, String> {
        if self.list.len() >= MAX_ITEMS {
            return Err(format!("The plugin keeps {MAX_ITEMS} new items at most: delete one first."));
        }
        let used: Vec<u16> = self.list.iter().map(|i| i.id).collect();
        let id = CustomItem::free_id(&used).ok_or("All the ids from 900 to 931 are used.")?;
        let mut item = CustomItem::new(id, "New item", "");
        if let Some((data, icon_item)) = template {
            item.data = data.clone();
            item.icon_item = icon_item;
        }
        self.list.push(item);
        self.list.sort_by_key(|i| i.id);
        Ok(id as usize)
    }

    /// Writes the new items that changed, and tells the plugin. Gives the number of files that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<usize, String> {
        let mut written = 0;
        for item in &self.list {
            if self.saved.iter().any(|saved| saved == item) {
                continue;
            }
            if let Some(first) = item.problems().first() {
                return Err(format!("#{} {}: {first}", item.id, item.names[ENGLISH]));
            }
            let path = CustomItem::path(item.id);
            sd.write(&path, &item.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
            written += 1;
        }
        self.saved = self.list.clone();
        Ok(written)
    }

    /// Deletes a new item: its files too.
    pub fn delete(&mut self, sd: &SdCard, id: usize) -> Result<(), String> {
        let id16 = id as u16;
        let path = CustomItem::path(id16);
        sd.remove(&path).and_then(|_| sd.mark_changed(&path))?;
        sd.remove(&CustomItem::picture_path(id16))?;
        self.list.retain(|i| i.id != id16);
        self.saved.retain(|i| i.id != id16);
        Ok(())
    }
}

/// The names and the description of a new item in a language, and the message of the Bag: the Text tab of a new item.
pub fn texts_ui(ui: &mut egui::Ui, item: &mut CustomItem, language: &mut usize) {
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
        ui.label(widget::hint("A new item starts with the same text in every language: write the text of this language. The message is what the Bag says when the player uses the item; {pokemon} is the name of the Pokémon. Empty: the message of the plugin."));
        ui.add_space(6.0);
        let language = (*language).min(7);
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut item.names[language]).desired_width(300.0));
        });
        ui.label("Description");
        ui.add(egui::TextEdit::multiline(&mut item.descriptions[language]).desired_rows(3).desired_width(520.0));
        ui.label("Message of the Bag");
        ui.add(egui::TextEdit::singleline(&mut item.messages[language]).desired_width(520.0).hint_text("{pokemon} was changed!"));
    });
}

/// What a new item does: in the Bag, and when a Pokémon holds it in a battle. The Reactions tab of a new item.
pub fn reactions_ui(ui: &mut egui::Ui, item: &mut CustomItem) {
    let salt = format!("item_{}", item.id);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Use in the Bag"));
        ui.label(widget::hint("The player chooses \"Use\" and a Pokémon in the Bag. The actions run in this order, then the game removes one item."));
        ui.checkbox(&mut item.can_use_in_bag, "The player can use this item on a Pokémon in the Bag");
        if item.can_use_in_bag {
            let mut remove = None;
            for (i, action) in item.bag_actions.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    let mut kind = action.kind;
                    egui::ComboBox::from_id_salt((&salt, "bag_kind", i)).width(220.0).selected_text(kind.label()).show_ui(ui, |ui| {
                        for option in BagKind::ALL {
                            ui.selectable_value(&mut kind, option, option.label());
                        }
                    });
                    if kind != action.kind {
                        *action = BagAction::new(kind);
                    }
                    match action.kind {
                        BagKind::ChangeHp => {
                            let mut all = action.a == 32767;
                            ui.checkbox(&mut all, "all the HP");
                            if all {
                                action.a = 32767;
                            } else {
                                if action.a == 32767 {
                                    action.a = 50;
                                }
                                let mut value = action.a as i32;
                                ui.add(egui::DragValue::new(&mut value).range(-999..=999).suffix(" HP"));
                                action.a = value as i16;
                            }
                        }
                        BagKind::SetShiny => {
                            egui::ComboBox::from_id_salt((&salt, "bag_shiny", i)).selected_text(["Switch", "Make shiny", "Make normal"][action.a.clamp(0, 2) as usize]).show_ui(ui, |ui| {
                                for (value, name) in [(0i16, "Switch"), (1, "Make shiny"), (2, "Make normal")] {
                                    ui.selectable_value(&mut action.a, value, name);
                                }
                            });
                        }
                        BagKind::NextAbility => {}
                        BagKind::SetLevel => {
                            let mut value = action.a as i32;
                            ui.add(egui::DragValue::new(&mut value).range(1..=100).prefix("level "));
                            action.a = value as i16;
                        }
                        BagKind::AddEv => {
                            egui::ComboBox::from_id_salt((&salt, "bag_ev", i)).selected_text(EV_STATS.iter().find(|(v, _)| *v == action.a).map(|(_, n)| *n).unwrap_or("?")).show_ui(ui, |ui| {
                                for (value, name) in EV_STATS {
                                    ui.selectable_value(&mut action.a, value, name);
                                }
                            });
                            let mut value = action.b as i32;
                            ui.add(egui::DragValue::new(&mut value).range(1..=252).suffix(" EVs"));
                            action.b = value as i16;
                        }
                    }
                    if widget::button(ui, "Remove") {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                item.bag_actions.remove(i);
            }
            ui.add_enabled_ui(item.bag_actions.len() < MAX_BAG_ACTIONS, |ui| {
                if widget::button(ui, "Add an action") {
                    item.bag_actions.push(BagAction::new(BagKind::ChangeHp));
                }
            });
        }
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("When a Pokémon holds the item in a battle"));
        ui.label(widget::hint("Each reaction says: when this happens, if all these are true, do these things. \"This Pokémon\" is the one that holds the item. The item is not used up."));
        sango_rules::editor::rules_editor(ui, &salt, &mut item.rules, Context::Item);
    });
    ui.add_space(10.0);
    widget::card(ui, |ui| {
        ui.set_min_width(700.0);
        ui.label(widget::title("Summary"));
        if item.can_use_in_bag && !item.bag_actions.is_empty() {
            let list: Vec<String> = item.bag_actions.iter().map(|a| a.sentence()).collect();
            ui.label(format!("In the Bag, on a Pokémon: {}.", list.join(", then ")));
        }
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
        let path = std::env::temp_dir().join(format!("sango_item_custom_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn an_item_goes_out_and_comes_back_with_the_documented_layout() {
        let mut item = CustomItem::new(900, "Plasma Core", "Raises the Sp. Atk.");
        item.messages[2] = "{pokemon} glows!".into();
        item.can_use_in_bag = true;
        item.bag_actions = vec![BagAction { kind: BagKind::ChangeHp, a: 32767, b: 0 }, BagAction { kind: BagKind::AddEv, a: 4, b: 10 }];
        item.rules.push(Rule { moment: 94, conditions: vec![Condition { kind: ConditionKind::PokemonIsOwner, arg: 0 }, Condition { kind: ConditionKind::HpBelow, arg: 50 }], actions: vec![Action::new(ActionKind::ChangeStat)] });
        assert!(item.problems().is_empty(), "{:?}", item.problems());
        let bytes = item.to_bytes();
        assert_eq!(&bytes[..8], &[b'S', b'I', b'T', b'M', 1, 132, 3, 1]);
        assert_eq!(CustomItem::from_bytes(&bytes).unwrap(), item);
        assert!(CustomItem::from_bytes(&bytes[..bytes.len() - 3]).is_err());
        // After the data: the flags, the item of the picture, the two actions of the Bag, then the rule.
        let rule_size = 4 + 2 * 3 + 7;
        let tail = bytes.len() - rule_size - (1 + 2 + 1 + 2 * 5);
        assert_eq!(&bytes[tail..tail + 4], &[1, 0, 0, 2]);
        assert_eq!(&bytes[tail + 4..tail + 9], &[1, 0xFF, 0x7F, 0, 0]);
    }

    #[test]
    fn the_problems_are_found() {
        let mut item = CustomItem::new(100, " ", "");
        item.can_use_in_bag = true;
        let problems = item.problems().join("\n");
        assert!(problems.contains("900 to 931") && problems.contains("English name") && problems.contains("no action"));
    }

    #[test]
    fn adding_an_item_copies_the_data_of_the_open_item_and_the_files_come_back() {
        let path = folder("store");
        let sd = SdCard::open(path.to_str().unwrap()).unwrap();
        let mut store = Store::default();
        store.ensure(Some(&sd));
        let mut template = ItemData { bytes: [0; SIZE] };
        template.set("price", 300);
        template.set("fling_power", 30);
        let id = store.add(Some((&template, 17))).unwrap();
        assert_eq!(id, 900);
        assert_eq!(store.find(900).unwrap().data, template);
        assert_eq!(store.find(900).unwrap().icon_item, 17);
        assert!(store.changed());
        store.save(&sd).unwrap();
        assert!(!store.changed() && sd.read("item/custom/900.bin").is_some());
        let mut other = Store::default();
        other.ensure(Some(&sd));
        assert_eq!(other.ids(), vec![900]);
        assert_eq!(other.add(None).unwrap(), 901);
        other.delete(&sd, 900).unwrap();
        assert_eq!(other.ids(), vec![901]);
        assert!(sd.read("item/custom/900.bin").is_none());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn the_cards_draw() {
        let mut item = CustomItem::new(900, "Test", "");
        item.can_use_in_bag = true;
        item.bag_actions = BagKind::ALL.iter().map(|&kind| BagAction::new(kind)).collect();
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
