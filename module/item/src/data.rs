//! The data of one item: the file of 36 bytes of the item archive.
//!
//! The file has the same layout for all the items. It is kept as bytes, and each field is a place in these bytes
//! ([`Def`]): the table [`DEFS`] drives the page, the CSV files and the notes, so that all of them agree.
//!
//! | Bytes | Content |
//! | --- | --- |
//! | 0-1 | The price, in tens of Poké Dollars. |
//! | 2-7 | Held effect, its power, Pluck effect (not used), Fling effect, Fling power, Natural Gift power. |
//! | 8-9 | Natural Gift type (5 bits), key item (1), registrable (1), Bag pocket (4), a copy of the battle pocket (5, not read). |
//! | 10-15 | Field use, battle use, use on a Pokémon, category, consumption (2 × 4 bits), sort number. |
//! | 16-35 | The effects on a Pokémon: cures, stat changes, PP, HP, EVs, friendship (see the table). |
//!
//! Each note says how the game reads the field. The notes come from reading the code of the game: a field that no
//! code reads is listed in [`UNUSED`] and not in the page.

/// The size of the file of an item, in bytes.
pub const SIZE: usize = 36;

/// The number of items (the number of files of the archive without the last one).
pub const COUNT: usize = 775;

/// The archive of the item data.
pub const ARCHIVE: usize = 197;

/// Where the file of an item is on the SD card.
pub fn sd_path(item: usize) -> String {
    format!("item/data/{item}.bin")
}

/// The folder of the files of the items on the SD card.
pub const SD_FOLDER: &str = "item/data";

/// A group of fields in the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Shop,
    Held,
    Use,
    Effects,
    Evs,
    Friendship,
}

impl Group {
    pub const ALL: [Group; 6] = [Group::Shop, Group::Held, Group::Use, Group::Effects, Group::Evs, Group::Friendship];

    pub fn title(self) -> &'static str {
        match self {
            Group::Shop => "Shop and Bag",
            Group::Held => "Held item",
            Group::Use => "Use",
            Group::Effects => "Effects on a Pokémon",
            Group::Evs => "Effort values",
            Group::Friendship => "Friendship",
        }
    }
}

/// How a field is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A number. The file keeps `value`, the page shows `value × scale`.
    Number { scale: u32 },
    /// A box to tick.
    Flag,
    /// A choice in a list of names: the value is the place in the list.
    Choice(&'static [&'static str]),
    /// A number that is a code of the game: the page shows examples of the items that have it.
    Code,
    /// A type of Pokémon (0 to 17), with its icon. Another value means no type.
    Type,
}

/// A field: its place in the bytes, and what the page says about it.
#[derive(Clone, Copy, Debug)]
pub struct Def {
    /// The name of the column in the CSV file.
    pub key: &'static str,
    /// The name in the page.
    pub label: &'static str,
    /// The first byte.
    pub offset: usize,
    /// 1 or 2 bytes (the 2 bytes are read as a little-endian number).
    pub width: u8,
    /// The first bit of the field in the bytes, and its number of bits.
    pub shift: u8,
    pub bits: u8,
    /// The value has a sign (two's complement on `bits` bits).
    pub signed: bool,
    pub kind: Kind,
    pub group: Group,
    /// What the field means.
    pub summary: &'static str,
    /// How the game reads it, from the code of the game.
    pub code: &'static str,
    /// The page of Bulbapedia for it, or nothing when no page fits.
    pub url: &'static str,
}

impl Def {
    /// The largest value of the field.
    pub fn max(&self) -> i32 {
        if self.signed {
            (1 << (self.bits - 1)) - 1
        } else {
            (1 << self.bits) - 1
        }
    }

    /// The smallest value of the field.
    pub fn min(&self) -> i32 {
        if self.signed {
            -(1 << (self.bits - 1))
        } else {
            0
        }
    }

    /// Reads the field in the bytes of an item.
    pub fn get(&self, bytes: &[u8; SIZE]) -> i32 {
        let raw = if self.width == 2 { u16::from_le_bytes([bytes[self.offset], bytes[self.offset + 1]]) as u32 } else { bytes[self.offset] as u32 };
        let value = ((raw >> self.shift) & ((1u32 << self.bits) - 1)) as i32;
        if self.signed && value >= 1 << (self.bits - 1) {
            value - (1 << self.bits)
        } else {
            value
        }
    }

    /// Writes the field in the bytes of an item. A value out of the limits is brought to the nearest limit.
    pub fn set(&self, bytes: &mut [u8; SIZE], value: i32) {
        let value = value.clamp(self.min(), self.max());
        let mask = ((1u32 << self.bits) - 1) << self.shift;
        let raw = if self.width == 2 { u16::from_le_bytes([bytes[self.offset], bytes[self.offset + 1]]) as u32 } else { bytes[self.offset] as u32 };
        let raw = (raw & !mask) | (((value as u32) << self.shift) & mask);
        if self.width == 2 {
            bytes[self.offset..self.offset + 2].copy_from_slice(&(raw as u16).to_le_bytes());
        } else {
            bytes[self.offset] = raw as u8;
        }
    }
}

/// The pockets of the Bag.
pub const POCKETS: [&str; 5] = ["Items", "Medicine", "TMs & HMs", "Berries", "Key Items"];

/// The categories of the items.
pub const CATEGORIES: [&str; 6] = ["None", "Equipment", "Normal", "Battle", "Ball", "Mail"];

/// What an item does in battle when the player uses it.
pub const BATTLE_USES: [&str; 4] = ["None", "Ball", "Recovery", "Escape"];

const FLAG: Kind = Kind::Flag;

const fn def(key: &'static str, label: &'static str, offset: usize, width: u8, shift: u8, bits: u8, signed: bool, kind: Kind, group: Group, summary: &'static str, code: &'static str, url: &'static str) -> Def {
    Def { key, label, offset, width, shift, bits, signed, kind, group, summary, code, url }
}

const ITEMS_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Item";
const HELD_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Held_item";
const BAG_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Bag";
const STAT_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Stat_modifier";
const EV_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Effort_values";
const FRIENDSHIP_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Friendship";
const STATUS_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Status_condition";

/// All the fields that the game reads, in the order of the page. The first one that has `key` "price" is the first
/// field of the group "Shop and Bag".
pub const DEFS: [Def; 62] = [
    // Shop and Bag
    def("price", "Price", 0, 2, 0, 16, false, Kind::Number { scale: 10 }, Group::Shop, "The price in a Poké Mart, in Poké Dollars. The file keeps the price divided by 10. The selling price is half.", "The Bag and the shops multiply it by 10; the Bag sells for half of it.", "https://bulbapedia.bulbagarden.net/wiki/Poké_Mart"),
    def("pocket", "Pocket", 8, 2, 7, 4, false, Kind::Choice(&POCKETS), Group::Shop, "The pocket of the Bag where the item goes.", "The Bag, the shops and the save data (the list of items of each pocket) read it.", BAG_URL),
    def("key_item", "Key item", 8, 2, 5, 1, false, FLAG, Group::Shop, "A key item cannot be given to a Pokémon to hold, tossed or sold.", "The Bag shows the Give and Toss commands only when it is off, and checks it before a sale.", "https://bulbapedia.bulbagarden.net/wiki/Key_Item"),
    def("registrable", "Can be registered", 8, 2, 6, 1, false, FLAG, Group::Shop, "The item can be registered to the Y button.", "The Bag checks it when the player opens the menu of the item.", ""),
    def("category", "Category", 13, 1, 0, 8, false, Kind::Choice(&CATEGORIES), Group::Shop, "The kind of item. Balls and Mail are checked by the battle code; the other values only sort the Bag.", "The battle code looks for Ball and Mail. The Bag sorts by category, then by sort number.", ITEMS_URL),
    def("sort_number", "Sort number", 15, 1, 0, 8, false, Kind::Number { scale: 1 }, Group::Shop, "The place of the item when the Bag sorts the items by type: a small number comes first.", "The Bag sorts by category, then by this number.", BAG_URL),
    // Held item
    def("hold_effect", "Held effect", 2, 1, 0, 8, false, Kind::Code, Group::Held, "The number of the effect when a Pokémon holds the item. The battle handlers of the items look at the item itself, so the number is a label that the trainer AI reads.", "Read by one command of the trainer AI scripts (the effect of the item that a Pokémon holds). The battle handlers use the item number, not this field.", HELD_URL),
    def("hold_power", "Held power", 3, 1, 0, 8, false, Kind::Number { scale: 1 }, Group::Held, "A number that the effect of the item reads: a percent, a fraction of the HP, a number of turns. It means different things for each item.", "Read by the battle handlers of most held items (HP fraction of a berry, percent of a boost, turns of an extension); the Repel items use it as their number of steps.", HELD_URL),
    def("consumed", "Consumed in battle", 14, 1, 0, 4, false, FLAG, Group::Held, "The item is removed from the Pokémon when its effect works in battle (berries, gems), and it does not come back after the battle.", "The battle code removes the item after its effect worked; at the end of the battle the Pokémon gets its first item back only when this field is 0.", HELD_URL),
    def("fling_effect", "Fling effect", 5, 1, 0, 8, false, Kind::Code, Group::Held, "The effect that Fling adds when it throws the item (flinch, poison, burn...).", "Read by the move Fling.", "https://bulbapedia.bulbagarden.net/wiki/Fling_(move)"),
    def("fling_power", "Fling power", 6, 1, 0, 8, false, FLAG, Group::Held, "The power of Fling with the item. 0 means that Fling fails.", "Read by the move Fling, and by the trainer AI.", "https://bulbapedia.bulbagarden.net/wiki/Fling_(move)"),
    def("natural_gift_power", "Natural Gift power", 7, 1, 0, 8, false, Kind::Number { scale: 1 }, Group::Held, "The power of Natural Gift with the item (berries).", "Read by the move Natural Gift.", "https://bulbapedia.bulbagarden.net/wiki/Natural_Gift_(move)"),
    def("natural_gift_type", "Natural Gift type", 8, 2, 0, 5, false, Kind::Type, Group::Held, "The type of Natural Gift with the item (berries). The items that cannot be used for it have 31, which is no type.", "Read by the move Natural Gift.", "https://bulbapedia.bulbagarden.net/wiki/Natural_Gift_(move)"),
    // Use
    def("field_use", "Use outside battle", 10, 1, 0, 8, false, Kind::Code, Group::Use, "What the Bag does when the player uses the item outside of a battle. 0 means that the item has no use there.", "The Bag shows the Use command when it is not 0. The number chooses the action (heal, Town Map, bicycle, TM, berry, fishing rod...).", BAG_URL),
    def("battle_use", "Use in battle", 11, 1, 0, 8, false, Kind::Choice(&BATTLE_USES), Group::Use, "What the item does when the player uses it in a battle.", "The battle code treats the Escape value (an item that makes the player leave the battle).", BAG_URL),
    def("use_on_pokemon", "Used on a Pokémon", 12, 1, 0, 8, false, FLAG, Group::Use, "The player uses the item on a Pokémon: the effects below only work when it is on.", "The effects of an item on a Pokémon only work when it is 1. The evolution check reads it too.", ITEMS_URL),
    def("keep_when_used", "Kept when used", 14, 1, 4, 4, false, FLAG, Group::Use, "The item is not used up when the player uses it (a flute that stays in the Bag).", "The Bag and the battle code do not remove the item when it is not 0.", ITEMS_URL),
    // Effects
    def("cure_sleep", "Cures sleep", 16, 1, 0, 1, false, FLAG, Group::Effects, "The item wakes a sleeping Pokémon.", "Read by the use of items on a Pokémon, in the Bag and in battle.", STATUS_URL),
    def("cure_poison", "Cures poison", 16, 1, 1, 1, false, FLAG, Group::Effects, "The item cures poison.", "Read by the use of items on a Pokémon, in the Bag and in battle.", STATUS_URL),
    def("cure_burn", "Cures burn", 16, 1, 2, 1, false, FLAG, Group::Effects, "The item cures a burn.", "Read by the use of items on a Pokémon, in the Bag and in battle.", STATUS_URL),
    def("cure_freeze", "Cures freeze", 16, 1, 3, 1, false, FLAG, Group::Effects, "The item thaws a frozen Pokémon.", "Read by the use of items on a Pokémon, in the Bag and in battle.", STATUS_URL),
    def("cure_paralysis", "Cures paralysis", 16, 1, 4, 1, false, FLAG, Group::Effects, "The item cures paralysis.", "Read by the use of items on a Pokémon, in the Bag and in battle.", STATUS_URL),
    def("cure_confusion", "Cures confusion", 16, 1, 5, 1, false, FLAG, Group::Effects, "The item cures confusion (in battle).", "Read by the battle code.", STATUS_URL),
    def("cure_infatuation", "Cures infatuation", 16, 1, 6, 1, false, FLAG, Group::Effects, "The item cures infatuation (in battle).", "Read by the battle code.", STATUS_URL),
    def("guard_stat_stages", "Guards the stats", 16, 1, 7, 1, false, FLAG, Group::Effects, "The item protects the stats of the team from being lowered (X items such as Guard Spec.).", "Read by the battle code.", STAT_URL),
    def("revive", "Revives", 17, 1, 0, 1, false, FLAG, Group::Effects, "The item brings a fainted Pokémon back (with the HP below).", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/Revive"),
    def("revive_all", "Revives the team", 17, 1, 1, 1, false, FLAG, Group::Effects, "The item brings all the fainted Pokémon back.", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/Revival_Herb"),
    def("level_up", "Raises the level", 17, 1, 2, 1, false, FLAG, Group::Effects, "The item raises the level of the Pokémon by 1 (Rare Candy).", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/Rare_Candy"),
    def("evolve", "Evolves", 17, 1, 3, 1, false, FLAG, Group::Effects, "The item can make the Pokémon evolve (the evolutions that need an item).", "The Bag marks the item with the evolution icon and the evolution check starts when it is set.", "https://bulbapedia.bulbagarden.net/wiki/Evolutionary_stone"),
    def("raise_attack", "Attack stages", 17, 1, 4, 4, false, FLAG, Group::Effects, "The stages of Attack that the item gives in battle (X Attack).", "Read by the battle code.", STAT_URL),
    def("raise_defense", "Defense stages", 18, 1, 0, 4, false, FLAG, Group::Effects, "The stages of Defense that the item gives in battle.", "Read by the battle code.", STAT_URL),
    def("raise_sp_atk", "Sp. Atk stages", 18, 1, 4, 4, false, Kind::Number { scale: 1 }, Group::Effects, "The stages of Special Attack that the item gives in battle.", "Read by the battle code.", STAT_URL),
    def("raise_sp_def", "Sp. Def stages", 19, 1, 0, 4, false, Kind::Number { scale: 1 }, Group::Effects, "The stages of Special Defense that the item gives in battle.", "Read by the battle code.", STAT_URL),
    def("raise_speed", "Speed stages", 19, 1, 4, 4, false, Kind::Number { scale: 1 }, Group::Effects, "The stages of Speed that the item gives in battle.", "Read by the battle code.", STAT_URL),
    def("raise_accuracy", "Accuracy stages", 20, 1, 0, 4, false, Kind::Number { scale: 1 }, Group::Effects, "The stages of accuracy that the item gives in battle.", "Read by the battle code.", STAT_URL),
    def("raise_critical", "Critical-hit stages", 20, 1, 4, 2, false, Kind::Number { scale: 1 }, Group::Effects, "The critical-hit stages that the item gives in battle (Dire Hit).", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Critical_hit"),
    def("pp_up", "PP Up", 20, 1, 6, 1, false, FLAG, Group::Effects, "The item raises the maximum PP of a move by 1 step (PP Up).", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/PP_Up"),
    def("pp_max", "PP Max", 20, 1, 7, 1, false, FLAG, Group::Effects, "The item raises the maximum PP of a move to the limit (PP Max).", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/PP_Max"),
    def("restore_pp", "Restores PP of one move", 21, 1, 0, 1, false, FLAG, Group::Effects, "The item restores the PP of one move, by the amount below.", "Read by the use of items on a Pokémon, in the Bag and in battle.", "https://bulbapedia.bulbagarden.net/wiki/Ether"),
    def("restore_all_pp", "Restores PP of all moves", 21, 1, 1, 1, false, FLAG, Group::Effects, "The item restores the PP of all the moves, by the amount below.", "Read by the use of items on a Pokémon.", "https://bulbapedia.bulbagarden.net/wiki/Elixir"),
    def("restore_hp", "Restores HP", 21, 1, 2, 1, false, FLAG, Group::Effects, "The item restores HP, by the amount below.", "Read by the use of items on a Pokémon, in the Bag and in battle.", "https://bulbapedia.bulbagarden.net/wiki/Potion"),
    def("hp_restore", "HP restored", 29, 1, 0, 8, false, Kind::Number { scale: 1 }, Group::Effects, "The HP that the item restores. 255 restores all the HP, 254 half of the maximum HP, 253 a quarter of it.", "Read by the use of items on a Pokémon, in the Bag and in battle.", "https://bulbapedia.bulbagarden.net/wiki/Potion"),
    def("pp_restore", "PP restored", 30, 1, 0, 8, false, Kind::Number { scale: 1 }, Group::Effects, "The PP that the item restores. 127 restores all the PP.", "Read by the use of items on a Pokémon, in the Bag and in battle.", "https://bulbapedia.bulbagarden.net/wiki/Ether"),
    // Effort values
    def("ev_hp", "Gives HP EVs", 21, 1, 3, 1, false, FLAG, Group::Evs, "The item changes the HP effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_attack", "Gives Attack EVs", 21, 1, 4, 1, false, FLAG, Group::Evs, "The item changes the Attack effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_defense", "Gives Defense EVs", 21, 1, 5, 1, false, FLAG, Group::Evs, "The item changes the Defense effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_speed", "Gives Speed EVs", 21, 1, 6, 1, false, FLAG, Group::Evs, "The item changes the Speed effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_sp_atk", "Gives Sp. Atk EVs", 21, 1, 7, 1, false, FLAG, Group::Evs, "The item changes the Special Attack effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_sp_def", "Gives Sp. Def EVs", 22, 1, 0, 1, false, FLAG, Group::Evs, "The item changes the Special Defense effort values, by the amount below.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_limit_control", "Follows the EV limit", 22, 1, 1, 1, false, FLAG, Group::Evs, "The item obeys the limit of 100 effort values for one stat (Berries that lower EVs do not).", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_hp_value", "HP EVs", 23, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The HP effort values that the item gives. A negative number lowers them (a berry).", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_attack_value", "Attack EVs", 24, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The Attack effort values that the item gives.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_defense_value", "Defense EVs", 25, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The Defense effort values that the item gives.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_speed_value", "Speed EVs", 26, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The Speed effort values that the item gives.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_sp_atk_value", "Sp. Atk EVs", 27, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The Special Attack effort values that the item gives.", "Read by the use of items on a Pokémon.", EV_URL),
    def("ev_sp_def_value", "Sp. Def EVs", 28, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Evs, "The Special Defense effort values that the item gives.", "Read by the use of items on a Pokémon.", EV_URL),
    // Friendship
    def("friendship_1", "Changes friendship (low)", 22, 1, 2, 1, false, FLAG, Group::Friendship, "The item changes the friendship of a Pokémon that has little friendship, by the amount below.", "Read by the use of items on a Pokémon.", FRIENDSHIP_URL),
    def("friendship_2", "Changes friendship (medium)", 22, 1, 3, 1, false, FLAG, Group::Friendship, "The item changes the friendship of a Pokémon that has medium friendship, by the amount below.", "Read by the use of items on a Pokémon.", FRIENDSHIP_URL),
    def("friendship_3", "Changes friendship (high)", 22, 1, 4, 1, false, FLAG, Group::Friendship, "The item changes the friendship of a Pokémon that has a lot of friendship, by the amount below.", "Read by the use of items on a Pokémon.", FRIENDSHIP_URL),
    def("friendship_1_value", "Friendship (low)", 31, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Friendship, "The friendship that the item gives to a Pokémon with little friendship. A negative number lowers it.", "Read by the friendship code, with the two values that follow, according to the friendship of the Pokémon.", FRIENDSHIP_URL),
    def("friendship_2_value", "Friendship (medium)", 32, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Friendship, "The friendship that the item gives to a Pokémon with medium friendship.", "Read by the friendship code.", FRIENDSHIP_URL),
    def("friendship_3_value", "Friendship (high)", 33, 1, 0, 8, true, Kind::Number { scale: 1 }, Group::Friendship, "The friendship that the item gives to a Pokémon with a lot of friendship.", "Read by the friendship code.", FRIENDSHIP_URL),
];

/// A field that no code of the game reads: the file has it, the page shows only the word "unused" for it (when it has a
/// group), and the CSV files do not have it.
#[derive(Clone, Copy, Debug)]
pub struct Unused {
    pub label: &'static str,
    pub offset: usize,
    pub shift: u8,
    pub bits: u8,
    pub width: u8,
    /// The group of the page that shows the word, or none.
    pub show: Option<Group>,
}

/// The fields of the file that the game never reads. An import keeps them as they are.
pub const UNUSED: [Unused; 4] = [
    Unused { label: "Pluck effect", offset: 4, shift: 0, bits: 8, width: 1, show: None },
    Unused { label: "Battle pocket (copy)", offset: 8, shift: 11, bits: 5, width: 2, show: None },
    Unused { label: "Spare bits", offset: 22, shift: 5, bits: 3, width: 1, show: None },
    Unused { label: "Padding", offset: 34, shift: 0, bits: 16, width: 2, show: None },
];

/// The data of one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemData {
    pub bytes: [u8; SIZE],
}

impl ItemData {
    /// Reads the file of an item.
    pub fn from_bytes(bytes: &[u8]) -> Result<ItemData, String> {
        let bytes: [u8; SIZE] = bytes.try_into().map_err(|_| format!("The file of an item has {SIZE} bytes, not {}.", bytes.len()))?;
        Ok(ItemData { bytes })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }

    /// The value of a field.
    pub fn get(&self, key: &str) -> Option<i32> {
        DEFS.iter().find(|def| def.key == key).map(|def| def.get(&self.bytes))
    }

    /// Gives a value to a field. False when there is no such field.
    pub fn set(&mut self, key: &str, value: i32) -> bool {
        match DEFS.iter().find(|def| def.key == key) {
            Some(def) => {
                def.set(&mut self.bytes, value);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_fields_do_not_overlap_and_stay_in_the_file() {
        let mut used = [false; SIZE * 8];
        let mut mark = |offset: usize, width: u8, shift: u8, bits: u8, what: &str| {
            for bit in shift..shift + bits {
                let at = offset * 8 + bit as usize;
                assert!(at < (offset + width as usize) * 8, "{what} goes out of its bytes");
                assert!(at < used.len(), "{what} goes out of the file");
                assert!(!used[at], "{what} overlaps another field");
                used[at] = true;
            }
        };
        for def in &DEFS {
            mark(def.offset, def.width, def.shift, def.bits, def.key);
        }
        for unused in UNUSED.iter() {
            mark(unused.offset, unused.width, unused.shift, unused.bits, unused.label);
        }
        // Every bit of the file belongs to a field.
        let missing: Vec<usize> = used.iter().enumerate().filter(|(_, u)| !**u).map(|(i, _)| i).collect();
        assert!(missing.is_empty(), "bits of the file with no field: {missing:?}");
        let mut keys: Vec<&str> = DEFS.iter().map(|d| d.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), DEFS.len(), "two fields have the same key");
    }

    #[test]
    fn a_field_is_read_and_written_in_its_bits() {
        let mut item = ItemData::from_bytes(&[0; SIZE]).unwrap();
        assert!(item.set("price", 1000));
        assert!(item.set("pocket", 3));
        assert!(item.set("key_item", 1));
        assert!(item.set("natural_gift_type", 12));
        assert!(item.set("ev_attack_value", -10));
        assert_eq!(item.get("price"), Some(1000));
        assert_eq!(item.get("pocket"), Some(3));
        assert_eq!(item.get("key_item"), Some(1));
        assert_eq!(item.get("natural_gift_type"), Some(12));
        assert_eq!(item.get("ev_attack_value"), Some(-10));
        assert_eq!(item.bytes[24], 246);
        // A value out of the limits goes to the nearest limit, and the other fields stay.
        item.set("raise_critical", 9);
        assert_eq!(item.get("raise_critical"), Some(3));
        assert_eq!(item.get("raise_accuracy"), Some(0));
        item.set("ev_hp_value", -500);
        assert_eq!(item.get("ev_hp_value"), Some(-128));
        assert!(!item.set("nothing", 1));
        assert!(ItemData::from_bytes(&[0; 10]).is_err());
    }
}
