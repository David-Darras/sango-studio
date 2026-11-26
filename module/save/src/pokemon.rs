//! One Pokémon of the save: 232 bytes in a box, and 20 more bytes of calculated data (28 with the padding) in the party.
//!
//! The game encrypts both parts with a sequence of numbers that starts at the encryption key, and it changes the order of the four
//! parts of the record. [`Pokemon`] keeps the plain data: [`Pokemon::read`] decrypts it and [`Pokemon::to_stored`] encrypts it.

/// The bytes of a Pokémon in a box.
pub const STORED: usize = 232;
/// The bytes of a Pokémon in the party.
pub const PARTY: usize = 260;
const STATS: usize = PARTY - STORED;
const BLOCK: usize = 56;

/// For each order, the place of the four parts of the record in the file.
const ORDERS: [[usize; 4]; 24] = [
    [0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3], [0, 3, 1, 2], [0, 2, 3, 1], [0, 3, 2, 1], [1, 0, 2, 3], [1, 0, 3, 2], [2, 0, 1, 3],
    [3, 0, 1, 2], [2, 0, 3, 1], [3, 0, 2, 1], [1, 2, 0, 3], [1, 3, 0, 2], [2, 1, 0, 3], [3, 1, 0, 2], [2, 3, 0, 1], [3, 2, 0, 1],
    [1, 2, 3, 0], [1, 3, 2, 0], [2, 1, 3, 0], [3, 1, 2, 0], [2, 3, 1, 0], [3, 2, 1, 0],
];

/// The six statistics, in the order that the studio shows them.
pub const STAT_NAMES: [&str; 6] = ["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];
/// The place of each of them in the record: the game has Speed before the two special statistics.
const STORED_ORDER: [usize; 6] = [0, 1, 2, 4, 5, 3];

/// The highest effort value of a statistic that the game gives, and the highest total.
pub const MAX_EV: u8 = 252;
pub const MAX_EV_TOTAL: u32 = 510;
/// The characters of a name, without the end.
pub const NAME_LENGTH: usize = 12;
/// The species that always has 1 HP.
const SHEDINJA: u16 = 292;

/// Reads a name of `length` characters of two bytes, which ends at the first zero.
pub fn read_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&units)
}

/// Writes a name: as many characters as there is room, with the end, then zeros.
pub fn write_text(bytes: &mut [u8], text: &str) {
    let room = bytes.len() / 2 - 1;
    bytes.fill(0);
    let mut at = 0;
    for character in text.chars() {
        let mut units = [0u16; 2];
        let units = character.encode_utf16(&mut units);
        if at + units.len() > room {
            break;
        }
        for unit in units {
            bytes[2 * at..2 * at + 2].copy_from_slice(&unit.to_le_bytes());
            at += 1;
        }
    }
}

/// Encrypts or decrypts: the same operation.
fn crypt(bytes: &mut [u8], key: u32) {
    let mut seed = key;
    for pair in bytes.chunks_exact_mut(2) {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        let mask = ((seed >> 16) as u16).to_le_bytes();
        pair[0] ^= mask[0];
        pair[1] ^= mask[1];
    }
}

fn order(key: u32) -> &'static [usize; 4] {
    &ORDERS[(((key >> 13) & 31) % 24) as usize]
}

/// One Pokémon, in plain data.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Pokemon {
    /// The record, decrypted, with its four parts in their normal order.
    pub data: [u8; STORED],
    /// The calculated data of the party: the status, the level, the HP and the statistics.
    pub stats: [u8; STATS],
    /// False when the checksum of the file was wrong.
    pub sound: bool,
}

macro_rules! number {
    ($get:ident, $set:ident, $kind:ty, $at:expr) => {
        pub fn $get(&self) -> $kind {
            <$kind>::from_le_bytes(self.data[$at..$at + std::mem::size_of::<$kind>()].try_into().unwrap())
        }
        pub fn $set(&mut self, value: $kind) {
            self.data[$at..$at + std::mem::size_of::<$kind>()].copy_from_slice(&value.to_le_bytes());
        }
    };
}

impl Pokemon {
    /// An empty place.
    pub fn empty() -> Pokemon {
        Pokemon { data: [0; STORED], stats: [0; STATS], sound: true }
    }

    /// Reads the bytes of a box (232) or of the party (260).
    pub fn read(bytes: &[u8]) -> Pokemon {
        let key = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let mut stored: [u8; STORED] = bytes[..STORED].try_into().unwrap();
        crypt(&mut stored[8..], key);
        let mut data = stored;
        for (part, &place) in order(key).iter().enumerate() {
            data[8 + part * BLOCK..8 + (part + 1) * BLOCK].copy_from_slice(&stored[8 + place * BLOCK..8 + (place + 1) * BLOCK]);
        }
        let mut stats = [0; STATS];
        if bytes.len() >= PARTY {
            stats.copy_from_slice(&bytes[STORED..PARTY]);
            crypt(&mut stats, key);
        }
        let mut pokemon = Pokemon { data, stats, sound: true };
        pokemon.sound = pokemon.checksum() == pokemon.sum();
        pokemon
    }

    fn sum(&self) -> u16 {
        self.data[8..].chunks_exact(2).fold(0u16, |sum, c| sum.wrapping_add(u16::from_le_bytes([c[0], c[1]])))
    }

    /// The 232 bytes of a box, encrypted, with the checksum of the data.
    pub fn to_stored(&self) -> [u8; STORED] {
        let key = self.key();
        let mut plain = *self;
        let sum = plain.sum();
        plain.set_checksum(sum);
        let mut stored = plain.data;
        for (part, &place) in order(key).iter().enumerate() {
            stored[8 + place * BLOCK..8 + (place + 1) * BLOCK].copy_from_slice(&plain.data[8 + part * BLOCK..8 + (part + 1) * BLOCK]);
        }
        crypt(&mut stored[8..], key);
        stored
    }

    /// The 260 bytes of the party, encrypted.
    pub fn to_party(&self) -> [u8; PARTY] {
        let mut bytes = [0; PARTY];
        bytes[..STORED].copy_from_slice(&self.to_stored());
        bytes[STORED..].copy_from_slice(&self.stats);
        crypt(&mut bytes[STORED..], self.key());
        bytes
    }

    number!(key, set_key, u32, 0x00);
    number!(checksum, set_checksum, u16, 0x06);
    number!(species, set_species, u16, 0x08);
    number!(item, set_item, u16, 0x0A);
    number!(trainer_id, set_trainer_id, u16, 0x0C);
    number!(secret_id, set_secret_id, u16, 0x0E);
    number!(experience, set_experience, u32, 0x10);
    number!(personality, set_personality, u32, 0x18);
    /// The number of the name of the place where the Pokémon was met.
    pub fn set_met_location(&mut self, place: u16) {
        self.data[0xDA..0xDC].copy_from_slice(&place.to_le_bytes());
    }

    pub fn is_empty(&self) -> bool {
        self.species() == 0
    }

    pub fn ability(&self) -> u8 {
        self.data[0x14]
    }

    pub fn set_ability(&mut self, ability: u8) {
        self.data[0x14] = ability;
    }

    /// Which ability of the species it is: 1, 2, or 4 for the hidden one.
    pub fn ability_slot(&self) -> u8 {
        self.data[0x15] & 7
    }

    pub fn set_ability_slot(&mut self, slot: u8) {
        self.data[0x15] = (self.data[0x15] & !7) | (slot & 7);
    }

    pub fn nature(&self) -> u8 {
        self.data[0x1C]
    }

    pub fn set_nature(&mut self, nature: u8) {
        self.data[0x1C] = nature;
    }

    /// True for a Pokémon that an event gave ("fateful encounter").
    pub fn from_event(&self) -> bool {
        self.data[0x1D] & 1 != 0
    }

    pub fn set_from_event(&mut self, on: bool) {
        self.data[0x1D] = (self.data[0x1D] & !1) | u8::from(on);
    }

    /// 0 male, 1 female, 2 no gender.
    pub fn gender(&self) -> u8 {
        (self.data[0x1D] >> 1) & 3
    }

    pub fn set_gender(&mut self, gender: u8) {
        self.data[0x1D] = (self.data[0x1D] & !6) | ((gender & 3) << 1);
    }

    pub fn form(&self) -> u8 {
        self.data[0x1D] >> 3
    }

    pub fn set_form(&mut self, form: u8) {
        self.data[0x1D] = (self.data[0x1D] & 7) | (form << 3);
    }

    /// The effort value of a statistic, in the order of [`STAT_NAMES`].
    pub fn ev(&self, stat: usize) -> u8 {
        self.data[0x1E + STORED_ORDER[stat]]
    }

    pub fn set_ev(&mut self, stat: usize, value: u8) {
        self.data[0x1E + STORED_ORDER[stat]] = value;
    }

    pub fn ev_total(&self) -> u32 {
        (0..6).map(|stat| u32::from(self.ev(stat))).sum()
    }

    pub fn nickname(&self) -> String {
        read_text(&self.data[0x40..0x5A])
    }

    pub fn set_nickname(&mut self, name: &str) {
        write_text(&mut self.data[0x40..0x5A], name);
    }

    pub fn move_id(&self, slot: usize) -> u16 {
        u16::from_le_bytes([self.data[0x5A + 2 * slot], self.data[0x5B + 2 * slot]])
    }

    pub fn set_move_id(&mut self, slot: usize, id: u16) {
        self.data[0x5A + 2 * slot..0x5C + 2 * slot].copy_from_slice(&id.to_le_bytes());
    }

    pub fn pp(&self, slot: usize) -> u8 {
        self.data[0x62 + slot]
    }

    pub fn set_pp(&mut self, slot: usize, value: u8) {
        self.data[0x62 + slot] = value;
    }

    pub fn pp_ups(&self, slot: usize) -> u8 {
        self.data[0x66 + slot]
    }

    pub fn set_pp_ups(&mut self, slot: usize, value: u8) {
        self.data[0x66 + slot] = value;
    }

    fn individual(&self) -> u32 {
        u32::from_le_bytes(self.data[0x74..0x78].try_into().unwrap())
    }

    fn set_individual(&mut self, value: u32) {
        self.data[0x74..0x78].copy_from_slice(&value.to_le_bytes());
    }

    /// The individual value of a statistic (0 to 31), in the order of [`STAT_NAMES`].
    pub fn iv(&self, stat: usize) -> u8 {
        ((self.individual() >> (5 * STORED_ORDER[stat])) & 31) as u8
    }

    pub fn set_iv(&mut self, stat: usize, value: u8) {
        let shift = 5 * STORED_ORDER[stat];
        self.set_individual((self.individual() & !(31 << shift)) | (u32::from(value.min(31)) << shift));
    }

    pub fn is_egg(&self) -> bool {
        self.individual() & (1 << 30) != 0
    }

    pub fn set_egg(&mut self, on: bool) {
        self.set_individual((self.individual() & !(1 << 30)) | (u32::from(on) << 30));
    }

    /// True when the name is one that the player gave.
    pub fn has_nickname(&self) -> bool {
        self.individual() & (1 << 31) != 0
    }

    pub fn set_has_nickname(&mut self, on: bool) {
        self.set_individual((self.individual() & !(1 << 31)) | (u32::from(on) << 31));
    }

    /// True when a trainer other than the first one has the Pokémon now.
    fn is_traded(&self) -> bool {
        self.data[0x93] != 0
    }

    /// The friendship with the trainer who has the Pokémon now. For an Egg, the steps before it hatches.
    pub fn friendship(&self) -> u8 {
        self.data[if self.is_traded() { 0xA2 } else { 0xCA }]
    }

    pub fn set_friendship(&mut self, value: u8) {
        let at = if self.is_traded() { 0xA2 } else { 0xCA };
        self.data[at] = value;
    }

    pub fn trainer_name(&self) -> String {
        read_text(&self.data[0xB0..0xCA])
    }

    pub fn set_trainer_name(&mut self, name: &str) {
        write_text(&mut self.data[0xB0..0xCA], name);
    }

    /// The year (from 2000), the month and the day of the meeting.
    pub fn met_date(&self) -> (u8, u8, u8) {
        (self.data[0xD4], self.data[0xD5], self.data[0xD6])
    }

    pub fn set_met_date(&mut self, date: (u8, u8, u8)) {
        self.data[0xD4..0xD7].copy_from_slice(&[date.0, date.1, date.2]);
    }

    pub fn ball(&self) -> u8 {
        self.data[0xDC]
    }

    pub fn set_ball(&mut self, ball: u8) {
        self.data[0xDC] = ball;
    }

    pub fn met_level(&self) -> u8 {
        self.data[0xDD] & 0x7F
    }

    pub fn set_met_level(&mut self, level: u8) {
        self.data[0xDD] = (self.data[0xDD] & 0x80) | level.min(127);
    }

    /// The gender of the first trainer: 0 male, 1 female.
    pub fn set_trainer_gender(&mut self, gender: u8) {
        self.data[0xDD] = (self.data[0xDD] & 0x7F) | ((gender & 1) << 7);
    }

    /// The version of the game, the country, the region, the region of the console and the language of the Pokémon.
    pub fn set_origin(&mut self, version: u8, country: u8, region: u8, console: u8, language: u8) {
        self.data[0xDF..0xE4].copy_from_slice(&[version, country, region, console, language]);
    }

    pub fn language(&self) -> u8 {
        self.data[0xE3]
    }

    pub fn set_language(&mut self, language: u8) {
        self.data[0xE3] = language;
    }

    // The calculated data of the party.

    fn stat_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.stats[at], self.stats[at + 1]])
    }

    fn set_stat_at(&mut self, at: usize, value: u16) {
        self.stats[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// The status condition (0 when the Pokémon is well).
    pub fn status(&self) -> u32 {
        u32::from_le_bytes(self.stats[0..4].try_into().unwrap())
    }

    pub fn level(&self) -> u8 {
        self.stats[4]
    }

    pub fn hp(&self) -> u16 {
        self.stat_at(8)
    }

    pub fn set_hp(&mut self, value: u16) {
        let max = self.stat(0);
        self.set_stat_at(8, value.min(max));
    }

    /// A calculated statistic, in the order of [`STAT_NAMES`] (0 is the maximum HP).
    pub fn stat(&self, stat: usize) -> u16 {
        self.stat_at(10 + 2 * STORED_ORDER[stat])
    }

    /// Writes the level and the six statistics. The damage that the Pokémon has stays the same, and a fainted Pokémon stays
    /// fainted.
    pub fn set_calculated(&mut self, level: u8, values: [u16; 6]) {
        let (hp, max) = (self.hp(), self.stat(0));
        let new = if max == 0 {
            values[0]
        } else if hp == 0 {
            0
        } else {
            values[0].saturating_sub(max.saturating_sub(hp)).max(1)
        };
        self.stats[4] = level;
        for (stat, value) in values.into_iter().enumerate() {
            self.set_stat_at(10 + 2 * STORED_ORDER[stat], value);
        }
        self.set_stat_at(8, new);
    }

    /// Gives back the HP and removes the status condition.
    pub fn heal(&mut self) {
        let max = self.stat(0);
        self.set_stat_at(8, max);
        self.stats[0..4].fill(0);
    }

    /// True when the trainer sees the Pokémon as shiny.
    pub fn is_shiny(&self) -> bool {
        let id = self.personality();
        (self.trainer_id() ^ self.secret_id() ^ (id >> 16) as u16 ^ id as u16) < 16
    }

    /// Changes the personality value so that the Pokémon is shiny, or not.
    pub fn set_shiny(&mut self, shiny: bool) {
        if shiny == self.is_shiny() {
            return;
        }
        let id = self.personality();
        let high = (id >> 16) as u16;
        let low = if shiny { self.trainer_id() ^ self.secret_id() ^ high } else { id as u16 ^ 0x1000 };
        self.set_personality(u32::from(high) << 16 | u32::from(low));
    }
}

/// The six statistics of a Pokémon without the change of its nature, in the order of [`STAT_NAMES`].
pub fn plain_stats(species: u16, base: [u8; 6], pokemon: &Pokemon, level: u8) -> [u32; 6] {
    let mut values = [0; 6];
    for (stat, value) in values.iter_mut().enumerate() {
        let sum = 2 * u32::from(base[stat]) + u32::from(pokemon.iv(stat)) + u32::from(pokemon.ev(stat)) / 4;
        let part = sum * u32::from(level) / 100;
        *value = if stat > 0 { part + 5 } else if species == SHEDINJA { 1 } else { part + u32::from(level) + 10 };
    }
    values
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_record_comes_back_for_every_order() {
        for order in 0..32u32 {
            let mut pokemon = Pokemon::empty();
            pokemon.set_key(order << 13 | 5);
            pokemon.set_species(25);
            pokemon.set_nickname("Pika");
            pokemon.set_met_location(170);
            pokemon.set_language(3);
            pokemon.set_calculated(50, [100, 60, 40, 55, 50, 90]);
            let back = Pokemon::read(&pokemon.to_party());
            assert!(back.sound);
            assert_eq!((back.species(), back.nickname().as_str(), back.data[0xDA], back.language()), (25, "Pika", 170, 3));
            assert_eq!((back.level(), back.hp(), back.stat(5)), (50, 100, 90));
        }
    }

    #[test]
    fn the_fields_do_not_overlap() {
        let mut pokemon = Pokemon::empty();
        for stat in 0..6 {
            pokemon.set_iv(stat, 10 + stat as u8);
            pokemon.set_ev(stat, 20 + stat as u8);
        }
        pokemon.set_egg(true);
        pokemon.set_has_nickname(true);
        pokemon.set_form(7);
        pokemon.set_gender(1);
        pokemon.set_from_event(true);
        for stat in 0..6 {
            assert_eq!((pokemon.iv(stat), pokemon.ev(stat)), (10 + stat as u8, 20 + stat as u8));
        }
        assert!(pokemon.is_egg() && pokemon.has_nickname() && pokemon.from_event());
        assert_eq!((pokemon.form(), pokemon.gender()), (7, 1));
    }

    #[test]
    fn shiny_goes_on_and_off() {
        let mut pokemon = Pokemon::empty();
        pokemon.set_trainer_id(1234);
        pokemon.set_secret_id(5678);
        pokemon.set_personality(0x89AB_CDEF);
        pokemon.set_shiny(true);
        assert!(pokemon.is_shiny());
        pokemon.set_shiny(false);
        assert!(!pokemon.is_shiny());
    }

    #[test]
    fn the_statistics_follow_the_rules_of_the_game() {
        // Garchomp, level 78, from the example of the games.
        let mut pokemon = Pokemon::empty();
        for (stat, (iv, ev)) in [(24, 74), (12, 190), (30, 91), (16, 48), (23, 84), (5, 23)].into_iter().enumerate() {
            pokemon.set_iv(stat, iv);
            pokemon.set_ev(stat, ev);
        }
        assert_eq!(plain_stats(445, [108, 130, 95, 80, 85, 102], &pokemon, 78), [289, 253, 193, 151, 171, 171]);
        assert_eq!(plain_stats(SHEDINJA, [1, 90, 45, 30, 30, 40], &pokemon, 78)[0], 1);
    }

    #[test]
    fn the_damage_stays_when_the_statistics_change() {
        let mut pokemon = Pokemon::empty();
        pokemon.set_calculated(5, [20, 1, 1, 1, 1, 1]);
        pokemon.set_hp(15);
        pokemon.set_calculated(6, [24, 1, 1, 1, 1, 1]);
        assert_eq!(pokemon.hp(), 19);
        pokemon.set_hp(0);
        pokemon.set_calculated(7, [30, 1, 1, 1, 1, 1]);
        assert_eq!(pokemon.hp(), 0);
        pokemon.heal();
        assert_eq!(pokemon.hp(), 30);
    }

    #[test]
    fn a_name_is_cut_at_its_room() {
        let mut bytes = [0xFF; 26];
        write_text(&mut bytes, "ABCDEFGHIJKLMNOP");
        assert_eq!(read_text(&bytes), "ABCDEFGHIJKL");
    }
}
