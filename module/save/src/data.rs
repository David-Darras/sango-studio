//! The values of the save that the page edits, read from their blocks and written in them.

use crate::format::{self, Save};
use crate::pokemon::{self, Pokemon};

pub const NAME_LENGTH: usize = pokemon::NAME_LENGTH;
pub const MAX_MONEY: u32 = 9_999_999;
pub const MAX_POINTS: u32 = 9_999;
pub const MAX_HOURS: u32 = 999;
pub const PARTY_SLOTS: usize = 6;
pub const BOX_COUNT: usize = 31;
pub const BOX_SLOTS: usize = 30;
pub const BOX_NAME_LENGTH: usize = 16;
pub const MAX_ITEM_COUNT: u16 = 999;

/// The pockets of the Bag, in the order of the save: the name, the number of the pocket in the data of the items, the place of the
/// first entry and the number of entries.
pub const POCKETS: [(&str, u8, usize, usize); 5] =
    [("Items", 0, 0, 400), ("Key Items", 4, 400, 96), ("TMs & HMs", 2, 496, 108), ("Medicine", 1, 604, 64), ("Berries", 3, 668, 72)];

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn put_u16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// The player: the name, the numbers, the money and the play time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trainer {
    pub name: String,
    pub trainer_id: u16,
    pub secret_id: u16,
    /// 0 male, 1 female.
    pub gender: u8,
    pub rival: String,
    pub money: u32,
    pub battle_points: u32,
    /// One bit for each of the eight badges.
    pub badges: u32,
    pub hours: u16,
    pub minutes: u8,
    pub seconds: u8,
    /// The bits of the Mega Ring and of the Mega Evolution of Rayquaza.
    pub mega: u16,
    /// The options of the game, in one number.
    pub options: u32,
}

const TRAINER_NAME_AT: usize = 0x48;
const MEGA_AT: usize = 0x14A;
const MONEY_AT: usize = 0x08;
const BADGES_AT: usize = 0x0C;
const RIVAL_AT: usize = 0x10;
const POINTS_AT: usize = 0x30;

impl Trainer {
    pub fn read(save: &Save) -> Trainer {
        let (who, misc, time) = (save.block(format::TRAINER), save.block(format::MISC), save.block(format::PLAY_TIME));
        Trainer {
            name: pokemon::read_text(&who[TRAINER_NAME_AT..TRAINER_NAME_AT + 26]),
            trainer_id: u16_at(who, 0),
            secret_id: u16_at(who, 2),
            gender: who[5],
            rival: pokemon::read_text(&misc[RIVAL_AT..RIVAL_AT + 26]),
            money: u32_at(misc, MONEY_AT),
            battle_points: u32_at(misc, POINTS_AT),
            badges: u32_at(misc, BADGES_AT),
            hours: u16_at(time, 0),
            minutes: time[2],
            seconds: time[3],
            mega: u16_at(who, MEGA_AT),
            options: u32_at(save.block(format::OPTIONS), 0),
        }
    }

    /// Writes the values that are not the ones of the save, so a value that the user did not change keeps all its bytes.
    pub fn write(&self, save: &mut Save) {
        let old = Trainer::read(save);
        let who = save.block_mut(format::TRAINER);
        if self.name != old.name {
            pokemon::write_text(&mut who[TRAINER_NAME_AT..TRAINER_NAME_AT + 26], &self.name);
        }
        put_u16(who, 0, self.trainer_id);
        put_u16(who, 2, self.secret_id);
        who[5] = self.gender;
        put_u16(who, MEGA_AT, self.mega);
        let misc = save.block_mut(format::MISC);
        if self.rival != old.rival {
            pokemon::write_text(&mut misc[RIVAL_AT..RIVAL_AT + 26], &self.rival);
        }
        put_u32(misc, MONEY_AT, self.money);
        put_u32(misc, POINTS_AT, self.battle_points);
        put_u32(misc, BADGES_AT, self.badges);
        let time = save.block_mut(format::PLAY_TIME);
        put_u16(time, 0, self.hours);
        time[2] = self.minutes;
        time[3] = self.seconds;
        put_u32(save.block_mut(format::OPTIONS), 0, self.options);
    }

    /// The language of the game of the player (1 Japanese, 2 English, 3 French, 4 Italian, 5 German, 7 Spanish, 8 Korean).
    pub fn language(save: &Save) -> u8 {
        save.block(format::TRAINER)[0x2D]
    }
}

/// A part of the options: its first bit and its number of bits.
#[derive(Clone, Copy)]
pub struct Option_ {
    pub name: &'static str,
    pub shift: u32,
    pub bits: u32,
    pub values: &'static [&'static str],
}

/// The options of the menu of the game that the page shows.
pub const OPTIONS: [Option_; 5] = [
    Option_ { name: "Text speed", shift: 0, bits: 2, values: &["Slow", "Normal", "Fast"] },
    Option_ { name: "Battle effects", shift: 2, bits: 1, values: &["Off", "On"] },
    Option_ { name: "Battle style", shift: 3, bits: 1, values: &["Switch", "Set"] },
    Option_ { name: "Buttons", shift: 13, bits: 2, values: &["Normal", "L = A", "No L and R"] },
    Option_ { name: "Ask to save before a connection", shift: 15, bits: 1, values: &["No", "Yes"] },
];

impl Option_ {
    pub fn get(&self, options: u32) -> u32 {
        (options >> self.shift) & ((1 << self.bits) - 1)
    }

    pub fn set(&self, options: &mut u32, value: u32) {
        let mask = ((1 << self.bits) - 1) << self.shift;
        *options = (*options & !mask) | ((value << self.shift) & mask);
    }
}

/// The number of Pokémon of the party.
pub fn party_count(save: &Save) -> usize {
    (save.block(format::PARTY)[PARTY_SLOTS * pokemon::PARTY] as usize).min(PARTY_SLOTS)
}

/// The Pokémon of the party, in their order.
pub fn party(save: &Save) -> Vec<Pokemon> {
    let block = save.block(format::PARTY);
    (0..party_count(save)).map(|slot| Pokemon::read(&block[slot * pokemon::PARTY..(slot + 1) * pokemon::PARTY])).collect()
}

/// Writes the party. A place whose Pokémon does not change keeps all its bytes, and the places after the last Pokémon are empty.
pub fn set_party(save: &mut Save, party: &[Pokemon]) {
    let count = party.len().min(PARTY_SLOTS);
    let old_count = party_count(save);
    for slot in 0..PARTY_SLOTS {
        let range = slot * pokemon::PARTY..(slot + 1) * pokemon::PARTY;
        let new = party.get(slot).filter(|_| slot < count);
        if new.is_none() && slot >= old_count {
            continue;
        }
        let old = Pokemon::read(&save.block(format::PARTY)[range.clone()]);
        let new = new.copied().unwrap_or_else(Pokemon::empty);
        if old.data != new.data || old.stats != new.stats {
            let bytes = new.to_party();
            save.block_mut(format::PARTY)[range].copy_from_slice(&bytes);
        }
    }
    save.block_mut(format::PARTY)[PARTY_SLOTS * pokemon::PARTY] = count as u8;
}

fn box_range(number: usize, slot: usize) -> std::ops::Range<usize> {
    let at = (number * BOX_SLOTS + slot) * pokemon::STORED;
    at..at + pokemon::STORED
}

/// The Pokémon of a place of a box.
pub fn boxed(save: &Save, number: usize, slot: usize) -> Pokemon {
    Pokemon::read(&save.block(format::BOXES)[box_range(number, slot)])
}

/// Writes a place of a box, when its Pokémon changes.
pub fn set_boxed(save: &mut Save, number: usize, slot: usize, pokemon: &Pokemon) {
    if boxed(save, number, slot).data == pokemon.data {
        return;
    }
    let bytes = pokemon.to_stored();
    save.block_mut(format::BOXES)[box_range(number, slot)].copy_from_slice(&bytes);
}

pub fn box_name(save: &Save, number: usize) -> String {
    pokemon::read_text(&save.block(format::BOX_NAMES)[34 * number..34 * (number + 1)])
}

pub fn set_box_name(save: &mut Save, number: usize, name: &str) {
    if box_name(save, number) != name {
        pokemon::write_text(&mut save.block_mut(format::BOX_NAMES)[34 * number..34 * (number + 1)], name);
    }
}

/// The number of boxes that the player can use.
pub fn unlocked_boxes(save: &Save) -> u8 {
    save.block(format::BOX_NAMES)[0x43E]
}

pub fn set_unlocked_boxes(save: &mut Save, count: u8) {
    save.block_mut(format::BOX_NAMES)[0x43E] = count;
}

/// The entries of a pocket that have an item: the item and its quantity.
pub fn pocket(save: &Save, pocket: usize) -> Vec<(u16, u16)> {
    let (_, _, first, count) = POCKETS[pocket];
    let block = save.block(format::ITEMS);
    (first..first + count).map(|at| (u16_at(block, 4 * at), u16_at(block, 4 * at + 2))).take_while(|&(item, _)| item != 0).collect()
}

/// Writes a pocket: the entries, then empty entries.
pub fn set_pocket(save: &mut Save, pocket_number: usize, entries: &[(u16, u16)]) {
    if pocket(save, pocket_number) == entries {
        return;
    }
    let (_, _, first, count) = POCKETS[pocket_number];
    let block = save.block_mut(format::ITEMS);
    for slot in 0..count {
        let (item, quantity) = entries.get(slot).copied().unwrap_or((0, 0));
        put_u16(block, 4 * (first + slot), item);
        put_u16(block, 4 * (first + slot) + 2, quantity);
    }
}

/// The number of units of the world in the side of one tile.
pub const TILE: f32 = 18.0;

/// A place of the world: a map, a point and the side that the player looks at (0 up, 1 down, 2 left, 3 right).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub map: u16,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub facing: u8,
}

/// Where the game keeps, in the block of the position: the place of the next "Continue" when the game moves the player, the player
/// as it was at the save, and the switch of that move.
const MOVE_AT: usize = 0x70;
const PLAYER_AT: usize = 0xF4;
const MOVE_ON_AT: usize = 0x14E;
/// The kind of a place that has a point.
const DIRECT: u8 = 2;

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(bytes, at))
}

/// True when the next "Continue" moves the player.
pub fn moves_at_continue(save: &Save) -> bool {
    save.block(format::POSITION)[MOVE_ON_AT] != 0
}

/// Where the player is at the next "Continue".
pub fn place(save: &Save) -> Place {
    let block = save.block(format::POSITION);
    if moves_at_continue(save) {
        Place { map: u16_at(block, MOVE_AT + 2), x: f32_at(block, MOVE_AT + 0x10), y: f32_at(block, MOVE_AT + 0x14), z: f32_at(block, MOVE_AT + 0x18), facing: block[MOVE_AT + 6] }
    } else {
        Place { map: u16_at(block, PLAYER_AT), x: f32_at(block, PLAYER_AT + 0x10), y: f32_at(block, PLAYER_AT + 0x14), z: f32_at(block, PLAYER_AT + 0x18), facing: block[PLAYER_AT + 2] }
    }
}

/// Asks the game to move the player at the next "Continue", in the way that the game does it after the Hall of Fame: the map loads
/// from the start, with its characters. The player is then on foot.
pub fn move_player(save: &mut Save, to: Place) {
    let block = save.block_mut(format::POSITION);
    let at = MOVE_AT;
    block[at..at + 0x38].fill(0);
    block[at] = DIRECT;
    put_u16(block, at + 2, to.map);
    block[at + 6] = to.facing;
    for (offset, value) in [(0x10, to.x), (0x14, to.y), (0x18, to.z), (0x28, 1.0), (0x30, 1.0)] {
        put_u32(block, at + offset, value.to_bits());
    }
    put_u16(block, PLAYER_AT + 4, 0);
    block[MOVE_ON_AT] = 1;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_trainer_comes_back() {
        let mut save = format::empty_for_test();
        let mut trainer = Trainer::read(&save);
        trainer.name = "David".into();
        trainer.rival = "Serena".into();
        trainer.money = 1234;
        trainer.badges = 0b101;
        trainer.hours = 12;
        OPTIONS[0].set(&mut trainer.options, 2);
        trainer.write(&mut save);
        assert_eq!(Trainer::read(&save), trainer);
        assert_eq!(OPTIONS[0].get(trainer.options), 2);
    }

    #[test]
    fn an_unchanged_value_changes_no_block() {
        let mut save = format::empty_for_test();
        let loaded = save.clone();
        Trainer::read(&loaded).write(&mut save);
        set_party(&mut save, &party(&loaded));
        set_boxed(&mut save, 0, 0, &boxed(&loaded, 0, 0));
        for number in 0..5 {
            set_pocket(&mut save, number, &pocket(&loaded, number));
        }
        assert_eq!(save.changed_blocks(&loaded), Vec::<usize>::new());
    }

    #[test]
    fn the_party_keeps_its_count() {
        let mut save = format::empty_for_test();
        let mut first = Pokemon::empty();
        first.set_species(252);
        first.set_key(99);
        let mut second = first;
        second.set_species(255);
        set_party(&mut save, &[first, second]);
        assert_eq!(party(&save).iter().map(Pokemon::species).collect::<Vec<_>>(), [252, 255]);
        set_party(&mut save, &[second]);
        assert_eq!(party(&save).iter().map(Pokemon::species).collect::<Vec<_>>(), [255]);
    }

    #[test]
    fn the_pockets_fill_the_block_of_the_bag() {
        let mut next = 0;
        for (_, _, first, count) in POCKETS {
            assert_eq!(first, next);
            next += count;
        }
        assert_eq!(next * 4, 0xB90);
        let mut save = format::empty_for_test();
        set_pocket(&mut save, 4, &[(149, 5)]);
        assert_eq!(pocket(&save, 4), [(149, 5)]);
        assert!(pocket(&save, 3).is_empty());
    }

    #[test]
    fn a_move_waits_for_the_next_continue() {
        let mut save = format::empty_for_test();
        let to = Place { map: 224, x: 153.0, y: 2.0, z: 243.0, facing: 1 };
        move_player(&mut save, to);
        assert!(moves_at_continue(&save));
        assert_eq!(place(&save), to);
        assert_eq!(save.changed_blocks(&format::empty_for_test()), vec![format::POSITION]);
    }
}
