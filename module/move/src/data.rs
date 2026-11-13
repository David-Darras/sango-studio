//! The data of one move: a resource of 36 bytes in the file 0 of the archive 189.
//!
//! The file has the same layout for all the moves. It is kept as bytes, and each field is a place in these bytes
//! ([`Def`]): the table [`DEFS`] drives the page, the CSV files and the notes, so that all of them agree.
//!
//! | Bytes | Content |
//! | --- | --- |
//! | 0-6 | Type, category, damage kind, power, accuracy, PP, priority. |
//! | 7 | The number of hits (the largest in the high 4 bits; the smallest in the low 4 bits is not read). |
//! | 8-15 | The status condition (2 bytes), its chance, how long it stays, its turns, the critical hit stage, the flinch chance. |
//! | 16-17 | The number of the script of the trainer AI. |
//! | 18-19 | Drain or recoil of the damage, and HP given or lost (signed percentages). |
//! | 20 | The target. |
//! | 21-29 | Up to three stat changes: the stats, the stages, the chances. |
//! | 30-31 | Not read. |
//! | 32-33 | The 16 properties of the move (contact, sound, punch...), one bit each. |
//! | 34-35 | Not read. |
//!
//! Each note says how the game reads the field. The notes come from reading the code of the game: a field that no code
//! reads is listed in [`UNUSED`] and shows only the word "unused" in the page.

/// The size of the data of one move, in bytes.
pub const SIZE: usize = 36;

/// The number of moves (the move 0 is no move).
pub const COUNT: usize = 621;

/// The archive of the data of the moves.
pub const ARCHIVE: usize = 189;

/// The folder of the files of the moves on the SD card.
pub const SD_FOLDER: &str = "move/data";

/// Where the file of a move is on the SD card (36 bytes, as the resource in the archive).
pub fn sd_path(id: usize) -> String {
    format!("{SD_FOLDER}/{id}.bin")
}

/// A group of fields in the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Basics,
    Damage,
    Status,
    Stats,
    Target,
    Properties,
}

impl Group {
    pub const ALL: [Group; 6] = [Group::Basics, Group::Damage, Group::Status, Group::Stats, Group::Target, Group::Properties];

    pub fn title(self) -> &'static str {
        match self {
            Group::Basics => "Basics",
            Group::Damage => "Damage and recovery",
            Group::Status => "Status condition",
            Group::Stats => "Stat changes",
            Group::Target => "Target and AI",
            Group::Properties => "Properties",
        }
    }
}

/// How a field is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A number.
    Number,
    /// A box to tick.
    Flag,
    /// A choice in a list of names: the value is the place in the list.
    Choice(&'static [&'static str]),
    /// A number that is a code of the game: the page shows examples of the moves that have it.
    Code,
    /// A number with a name for each value of a list (the values are not 0, 1, 2...).
    Named(&'static [(i32, &'static str)]),
    /// A type of Pokémon (0 to 17), with its icon.
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

    /// Reads the field in the bytes of a move.
    pub fn get(&self, bytes: &[u8; SIZE]) -> i32 {
        let raw = if self.width == 2 { u16::from_le_bytes([bytes[self.offset], bytes[self.offset + 1]]) as u32 } else { bytes[self.offset] as u32 };
        let value = ((raw >> self.shift) & ((1u32 << self.bits) - 1)) as i32;
        if self.signed && value >= 1 << (self.bits - 1) {
            value - (1 << self.bits)
        } else {
            value
        }
    }

    /// Writes the field in the bytes of a move. A value out of the limits is brought to the nearest limit.
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

/// What a move does. It selects the part of the battle engine that runs the move.
pub const CATEGORIES: [&str; 14] = [
    "Damage",
    "Status condition",
    "Stat change",
    "Recovery",
    "Damage + status condition",
    "Stat change + status condition",
    "Damage + stat change of the target",
    "Damage + stat change of the user",
    "Drain",
    "One-hit KO",
    "Field effect",
    "Side effect",
    "Force out",
    "Other",
];

/// How the damage is calculated.
pub const DAMAGE_KINDS: [&str; 3] = ["Status", "Physical", "Special"];

/// How long a status condition stays.
pub const DURATIONS: [&str; 5] = ["None", "Permanent", "Number of turns", "While the user stays", "Turns or until the user leaves"];

/// The status conditions that a move can give, with their number in the file. 65535 means that the code of the move
/// decides. The first 8 are the usual conditions; the others are effects of the battle (the ones that the files of the moves
/// use are 8 to 9, 12 to 15 and 17 to 21; the other numbers are set by the code of some moves).
pub const STATUS_CONDITIONS: [(i32, &str); 39] = [
    (0, "None"),
    (1, "Paralysis"),
    (2, "Sleep"),
    (3, "Freeze"),
    (4, "Burn"),
    (5, "Poison"),
    (6, "Confusion"),
    (7, "Infatuation"),
    (8, "Trapped (Bind, Wrap...)"),
    (9, "Nightmare"),
    (10, "Curse"),
    (11, "Taunt"),
    (12, "Torment"),
    (13, "Disable"),
    (14, "Drowsiness (Yawn)"),
    (15, "Heal Block"),
    (16, "Ability suppressed (Gastro Acid)"),
    (17, "Identified (Foresight...)"),
    (18, "Leech Seed"),
    (19, "Embargo"),
    (20, "Perish Song"),
    (21, "Ingrain"),
    (22, "Cannot flee or switch"),
    (23, "Encore"),
    (24, "Roost"),
    (25, "Locked into a move"),
    (26, "Locked into a charging move"),
    (27, "Choice lock"),
    (28, "Next move cannot miss"),
    (29, "Cannot miss one target"),
    (30, "Magnet Rise"),
    (31, "Grounded (Smack Down)"),
    (32, "Telekinesis"),
    (33, "Sky Drop"),
    (34, "Accuracy up"),
    (35, "Aqua Ring"),
    (36, "Type changed (Ion Deluge)"),
    (37, "Powder"),
    (65535, "Decided by the move"),
];

/// The stats that a move changes.
pub const STATS: [&str; 9] = ["None", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed", "Accuracy", "Evasion", "All stats"];

/// The Pokémon that a move hits.
pub const TARGETS: [&str; 14] = [
    "One other Pokémon (chosen)",
    "The user or an ally (chosen)",
    "One ally (chosen)",
    "One opponent (chosen)",
    "All the other Pokémon",
    "All the opponents",
    "All the allies",
    "The user",
    "All the Pokémon",
    "One random opponent",
    "The field",
    "The opponents' side",
    "The user's side",
    "Decided by the move",
];

const FLAG: Kind = Kind::Flag;
const NUMBER: Kind = Kind::Number;

const fn def(key: &'static str, label: &'static str, offset: usize, width: u8, shift: u8, bits: u8, signed: bool, kind: Kind, group: Group, summary: &'static str, code: &'static str, url: &'static str) -> Def {
    Def { key, label, offset, width, shift, bits, signed, kind, group, summary, code, url }
}

const MOVE_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Move";
const STAT_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Stat_modifier";
const STATUS_URL: &str = "https://bulbapedia.bulbagarden.net/wiki/Status_condition";

/// All the fields that the game reads, in the order of the page.
pub const DEFS: [Def; 44] = [
    // Basics
    def("type", "Type", 0, 1, 0, 8, false, Kind::Type, Group::Basics, "The type of the move. It gives the same-type attack bonus and the effectiveness against the target.", "Read by the battle code for the damage, and by the lists of moves.", "https://bulbapedia.bulbagarden.net/wiki/Type"),
    def("category", "Effect kind", 1, 1, 0, 8, false, Kind::Choice(&CATEGORIES), Group::Basics, "What the move does. It chooses the part of the battle code that runs the move: a move that must hurt needs a kind with damage, or it deals none whatever its power is.", "Read by the battle code to run the move.", MOVE_URL),
    def("damage_kind", "Damage kind", 2, 1, 0, 8, false, Kind::Choice(&DAMAGE_KINDS), Group::Basics, "How the damage is calculated: Physical uses Attack and Defense, Special uses Sp. Atk and Sp. Def, Status deals no damage.", "Read by the battle code for the damage, and by the lists of moves.", "https://bulbapedia.bulbagarden.net/wiki/Move_category"),
    def("power", "Power", 3, 1, 0, 8, false, NUMBER, Group::Basics, "The base power of the move. 0 for a move that deals no damage; 1 for a move whose power depends on something else.", "Read by the battle code and by the trainer AI.", "https://bulbapedia.bulbagarden.net/wiki/Power"),
    def("accuracy", "Accuracy", 4, 1, 0, 8, false, NUMBER, Group::Basics, "The chance to hit, in percent. 101 means that the move never misses.", "Read by the battle code and by the lists of moves.", "https://bulbapedia.bulbagarden.net/wiki/Accuracy"),
    def("pp", "PP", 5, 1, 0, 8, false, NUMBER, Group::Basics, "The number of times that the move can be used (before PP Up).", "Read when a Pokémon learns the move and by the lists of moves.", "https://bulbapedia.bulbagarden.net/wiki/Power_Points"),
    def("priority", "Priority", 6, 1, 0, 8, true, NUMBER, Group::Basics, "The move goes first when its priority is higher, whatever the Speed. Most moves have 0.", "Read by the battle code to order the moves.", "https://bulbapedia.bulbagarden.net/wiki/Priority"),
    // Damage and recovery
    def("max_hits", "Number of hits", 7, 1, 4, 4, false, NUMBER, Group::Damage, "The largest number of hits of the move. 0 is one hit. A move with several hits picks a number up to this one.", "Read by the battle code, which picks the number of hits, and by the screen of the battle.", "https://bulbapedia.bulbagarden.net/wiki/Multistrike_move"),
    def("crit_stage", "Critical-hit stage", 14, 1, 0, 8, false, NUMBER, Group::Damage, "The critical-hit stage that the move adds. 6 means that the move always lands a critical hit.", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Critical_hit"),
    def("flinch", "Flinch chance", 15, 1, 0, 8, false, NUMBER, Group::Damage, "The chance, in percent, that the target flinches.", "Read by the battle code and by some abilities.", "https://bulbapedia.bulbagarden.net/wiki/Flinch"),
    def("drain", "Drain or recoil", 18, 1, 0, 8, true, NUMBER, Group::Damage, "A percent of the damage that the move deals: positive, the user gets that many HP back; negative, the user is hurt by that much (recoil).", "Read by the battle code, which gives or takes HP after the damage, and by some abilities.", "https://bulbapedia.bulbagarden.net/wiki/Recoil"),
    def("hp_change", "HP given or lost", 19, 1, 0, 8, true, NUMBER, Group::Damage, "A percent of the maximum HP of the user: positive, the user recovers that many HP (Recover); negative, the user loses that many.", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Recover_(move)"),
    // Status condition
    def("status", "Status condition", 8, 2, 0, 16, false, Kind::Named(&STATUS_CONDITIONS), Group::Status, "The status condition that the move gives (sleep, burn, confusion, Leech Seed...). The last choice means that the code of the move decides.", "Read by the battle code, which gives the condition to the target.", STATUS_URL),
    def("status_chance", "Status chance", 10, 1, 0, 8, false, NUMBER, Group::Status, "The chance, in percent, to give the status condition. 0 with a status move means that it always works.", "Read by the battle code.", STATUS_URL),
    def("status_duration", "Duration", 11, 1, 0, 8, false, Kind::Choice(&DURATIONS), Group::Status, "How the length of the status condition is counted.", "Read by the battle code.", STATUS_URL),
    def("status_turns_min", "Fewest turns", 12, 1, 0, 8, false, NUMBER, Group::Status, "The fewest turns that the status condition stays (when it is counted in turns).", "Read by the battle code.", STATUS_URL),
    def("status_turns_max", "Most turns", 13, 1, 0, 8, false, NUMBER, Group::Status, "The most turns that the status condition stays (when it is counted in turns).", "Read by the battle code.", STATUS_URL),
    // Stat changes
    def("stat_1", "Stat 1", 21, 1, 0, 8, false, Kind::Choice(&STATS), Group::Stats, "The first stat that the move changes.", "Read by the battle code.", STAT_URL),
    def("stat_1_stages", "Stages 1", 24, 1, 0, 8, true, NUMBER, Group::Stats, "The stages that the first stat gets: positive raises it, negative lowers it.", "Read by the battle code.", STAT_URL),
    def("stat_1_chance", "Chance 1", 27, 1, 0, 8, false, NUMBER, Group::Stats, "The chance, in percent, that the first stat changes. 0 or 100 mean always.", "Read by the battle code.", STAT_URL),
    def("stat_2", "Stat 2", 22, 1, 0, 8, false, Kind::Choice(&STATS), Group::Stats, "The second stat that the move changes.", "Read by the battle code.", STAT_URL),
    def("stat_2_stages", "Stages 2", 25, 1, 0, 8, true, NUMBER, Group::Stats, "The stages that the second stat gets.", "Read by the battle code.", STAT_URL),
    def("stat_2_chance", "Chance 2", 28, 1, 0, 8, false, NUMBER, Group::Stats, "The chance, in percent, that the second stat changes.", "Read by the battle code.", STAT_URL),
    def("stat_3", "Stat 3", 23, 1, 0, 8, false, Kind::Choice(&STATS), Group::Stats, "The third stat that the move changes.", "Read by the battle code.", STAT_URL),
    def("stat_3_stages", "Stages 3", 26, 1, 0, 8, true, NUMBER, Group::Stats, "The stages that the third stat gets.", "Read by the battle code.", STAT_URL),
    def("stat_3_chance", "Chance 3", 29, 1, 0, 8, false, NUMBER, Group::Stats, "The chance, in percent, that the third stat changes.", "Read by the battle code.", STAT_URL),
    // Target and AI
    def("target", "Target", 20, 1, 0, 8, false, Kind::Choice(&TARGETS), Group::Target, "The Pokémon that the move hits.", "Read by the battle code, the screen where the player chooses the target, and the trainer AI.", MOVE_URL),
    def("ai_script", "AI script", 16, 2, 0, 16, false, Kind::Code, Group::Target, "The number of the script that the trainer AI uses to decide when to use the move.", "Read by the trainer AI.", ""),
    // Properties: one bit each
    def("flag_contact", "Contact", 32, 2, 0, 1, false, FLAG, Group::Properties, "The move makes contact with the target: abilities and items that react to contact work.", "Read by the battle code, the abilities and the items.", "https://bulbapedia.bulbagarden.net/wiki/Contact"),
    def("flag_charge", "Charge turn", 32, 2, 1, 1, false, FLAG, Group::Properties, "The move needs a turn to charge before it hits (Solar Beam, Fly).", "Read by the battle code and by some abilities.", ""),
    def("flag_recharge", "Recharge turn", 32, 2, 2, 1, false, FLAG, Group::Properties, "The user must rest on the next turn (Hyper Beam).", "Read by the battle code.", ""),
    def("flag_protect", "Blocked by Protect", 32, 2, 3, 1, false, FLAG, Group::Properties, "Protect and the moves like it stop this move.", "Read by the battle code and by the moves that guard a side.", "https://bulbapedia.bulbagarden.net/wiki/Protect_(move)"),
    def("flag_magic_coat", "Reflected by Magic Coat", 32, 2, 4, 1, false, FLAG, Group::Properties, "Magic Coat sends the move back to its user.", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Magic_Coat_(move)"),
    def("flag_snatch", "Stolen by Snatch", 32, 2, 5, 1, false, FLAG, Group::Properties, "Snatch lets another Pokémon use the move first.", "Read by the move Snatch.", "https://bulbapedia.bulbagarden.net/wiki/Snatch_(move)"),
    def("flag_mirror_move", "Copied by Mirror Move", 32, 2, 6, 1, false, FLAG, Group::Properties, "Mirror Move can copy the move.", "Read by the move Mirror Move.", "https://bulbapedia.bulbagarden.net/wiki/Mirror_Move_(move)"),
    def("flag_punch", "Punching move", 32, 2, 7, 1, false, FLAG, Group::Properties, "The move is boosted by the ability Iron Fist.", "Read by the ability Iron Fist.", "https://bulbapedia.bulbagarden.net/wiki/Iron_Fist_(Ability)"),
    def("flag_sound", "Sound move", 32, 2, 8, 1, false, FLAG, Group::Properties, "The ability Soundproof blocks the move.", "Read by the ability Soundproof.", "https://bulbapedia.bulbagarden.net/wiki/Soundproof_(Ability)"),
    def("flag_gravity", "Fails in Gravity", 32, 2, 9, 1, false, FLAG, Group::Properties, "The move cannot be used while Gravity is on (moves that fly or jump).", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Gravity_(move)"),
    def("flag_thaw", "Thaws the user", 32, 2, 10, 1, false, FLAG, Group::Properties, "A frozen user thaws and uses the move (hot moves).", "Read by the battle code.", ""),
    def("flag_far", "Reaches far", 32, 2, 11, 1, false, FLAG, Group::Properties, "In a Triple Battle the move can target an opponent that is far.", "Read by the battle code, the screen where the player chooses the target, and the trainer AI.", "https://bulbapedia.bulbagarden.net/wiki/Triple_Battle"),
    def("flag_heal_block", "Blocked by Heal Block", 32, 2, 12, 1, false, FLAG, Group::Properties, "Heal Block stops the move (moves that restore HP).", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Heal_Block_(move)"),
    def("flag_substitute", "Passes a Substitute", 32, 2, 13, 1, false, FLAG, Group::Properties, "The move works even when the target has a Substitute.", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Substitute_(move)"),
    def("flag_sky_battle", "Fails in Sky Battles", 32, 2, 14, 1, false, FLAG, Group::Properties, "The move cannot be used in a Sky Battle.", "Read by the battle code.", "https://bulbapedia.bulbagarden.net/wiki/Sky_Battle"),
    def("flag_allies", "Effect aimed at allies", 32, 2, 15, 1, false, FLAG, Group::Properties, "The animation of the move aims at an ally when the move has a target that is one other Pokémon.", "Read by the battle screen.", ""),
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
pub const UNUSED: [Unused; 3] = [
    Unused { label: "Fewest hits", offset: 7, shift: 0, bits: 4, width: 1, show: Some(Group::Damage) },
    Unused { label: "Bytes 30 and 31", offset: 30, shift: 0, bits: 16, width: 2, show: None },
    Unused { label: "Bytes 34 and 35", offset: 34, shift: 0, bits: 16, width: 2, show: None },
];

/// The data of one move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveData {
    pub bytes: [u8; SIZE],
}

impl MoveData {
    /// Reads the data of a move.
    pub fn from_bytes(bytes: &[u8]) -> Result<MoveData, String> {
        let bytes: [u8; SIZE] = bytes.try_into().map_err(|_| format!("The data of a move has {SIZE} bytes, not {}.", bytes.len()))?;
        Ok(MoveData { bytes })
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
        let missing: Vec<usize> = used.iter().enumerate().filter(|(_, u)| !**u).map(|(i, _)| i).collect();
        assert!(missing.is_empty(), "bits of the file with no field: {missing:?}");
        let mut keys: Vec<&str> = DEFS.iter().map(|d| d.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), DEFS.len(), "two fields have the same key");
    }

    /// With the dump of the game: every status condition that a move gives has a name in the list.
    #[test]
    fn every_status_of_the_moves_has_a_name() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(ARCHIVE)).unwrap();
        let bundle = sango_core::Bundle::parse(garc.first_file(0).unwrap().to_vec()).unwrap();
        let status = DEFS.iter().find(|d| d.key == "status").unwrap();
        for id in 1..=COUNT {
            let value = status.get(&MoveData::from_bytes(bundle.resource(id).unwrap()).unwrap().bytes);
            assert!(STATUS_CONDITIONS.iter().any(|(v, _)| *v == value), "the move {id} has the status {value}");
        }
        let mut numbers: Vec<i32> = STATUS_CONDITIONS.iter().map(|(v, _)| *v).collect();
        numbers.dedup();
        assert_eq!(numbers.len(), STATUS_CONDITIONS.len());
    }

    #[test]
    fn a_field_is_read_and_written_in_its_bits() {
        let mut data = MoveData::from_bytes(&[0; SIZE]).unwrap();
        assert!(data.set("power", 90));
        assert!(data.set("priority", -6));
        assert!(data.set("max_hits", 5));
        assert!(data.set("status", 0xFFFF));
        assert!(data.set("drain", -33));
        assert!(data.set("flag_contact", 1));
        assert!(data.set("flag_sky_battle", 1));
        assert_eq!(data.get("power"), Some(90));
        assert_eq!(data.get("priority"), Some(-6));
        assert_eq!(data.bytes[6], 250);
        assert_eq!(data.get("max_hits"), Some(5));
        assert_eq!(data.bytes[7], 0x50, "the fewest hits stay 0");
        assert_eq!(data.get("status"), Some(0xFFFF));
        assert_eq!(data.get("drain"), Some(-33));
        assert_eq!((data.bytes[32], data.bytes[33]), (1, 64));
        data.set("max_hits", 20);
        assert_eq!(data.get("max_hits"), Some(15));
        assert!(!data.set("nothing", 1));
        assert!(MoveData::from_bytes(&[0; 10]).is_err());
    }
}
