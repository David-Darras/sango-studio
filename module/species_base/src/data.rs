//! The data of a species or of a form: one file of the archive 195.
//!
//! A file has 80 bytes. The first 64 bytes are the fields of `SpeciesData`, one after the other,
//! without padding. The last 16 bytes are not described yet: they are kept as they are.

/// The size of a file of the archive.
pub const FILE_SIZE: usize = 80;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The data of a species or of a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct SpeciesData {
    /// The base stats.
    pub base_hp: u8,
    pub base_attack: u8,
    pub base_defense: u8,
    pub base_speed: u8,
    pub base_special_attack: u8,
    pub base_special_defense: u8,
    /// The primary type.
    pub type_1: u8,
    /// The secondary type.
    pub type_2: u8,
    /// The catch rate.
    pub catch_rate: u8,
    pub unused_0: u8,
    /// The effort values and entry flag:
    /// bits 0..11: EV yield (2 bits each for HP, Atk, Def, Spe, SpA, SpD);
    /// bit 12: burrowed entry flag (1 = emerges directly in the ground like Diglett/Dugtrio, 0 = standard drop from Poké Ball);
    /// bits 13..15: unused.
    pub ev_yield: u16,
    /// Common held item present on 50% of wild encounters.
    pub item_50: u16,
    /// Rare held item present on 5% of wild encounters.
    pub item_5: u16,
    pub unused_1: u16,
    /// The gender ratio: 0 male only, 254 female only, 255 no gender.
    pub gender_ratio: u8,
    /// The number of egg cycles to hatch an Egg.
    pub hatch_time: u8,
    pub base_friendship: u8,
    /// Growth curve identifier / Leveling rate (0 to 7).
    pub leveling_rate: u8,
    /// The primary Egg Group.
    pub egg_group_1: u8,
    /// The secondary Egg Group.
    pub egg_group_2: u8,
    /// The primary ability.
    pub ability_1: u8,
    /// The secondary ability.
    pub ability_2: u8,
    /// The hidden ability.
    pub ability_hidden: u8,
    pub unused_2: u8,
    /// The number of the file, in the archive 195, of the first alternate form. 0 when the species has none.
    /// The alternate forms are one after the other: with the value 1000, the form 1 is in the file 1000,
    /// the form 2 is in the file 1001, and so on. The form 0 is the file of the species.
    pub alternate_form_file: u16,
    /// The number of the first model of the alternate forms. 0 when the species has none. The plugin keeps it next to the
    /// number of the first alternate form, so a species file of the SD card does not change it.
    pub alternate_form_model: u16,
    /// The number of total forms.
    pub total_form_count: u8,
    /// The Pokédex color.
    pub color: u8,
    /// The base experience yield.
    pub base_experience: u16,
    /// The height in centimeters.
    pub height: u16,
    /// The weight in hectograms.
    pub weight: u16,
    /// The TMs and HMs that the species can learn: one bit each, 128 bits in all.
    /// The bits 0 to 99 are TM01 to TM100, the bits 100 to 105 are HM01 to HM06, the bit 106 is HM07.
    pub tm_hm_moves: [u32; 4],
    /// The moves of the special Move Tutors that the species can learn: one bit each.
    /// Bit 0: Grass Pledge. Bit 1: Fire Pledge. Bit 2: Water Pledge. Bit 3: Frenzy Plant.
    /// Bit 4: Blast Burn. Bit 5: Hydro Cannon. Bit 6: Draco Meteor.
    pub special_tutor_moves: u32,
    /// Not used. It looks like a fake height.
    pub unused_4: u16,
    /// Not used. It looks like a size class for the online trade.
    pub unused_5: u16,
    /// The moves of the four Move Tutors that charge Battle Points: one `u32` of bits for each tutor.
    /// A table of the game gives the move of each bit.
    pub bp_tutor_moves: [u32; 4],
}

impl SpeciesData {
    /// Reads the data from the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<SpeciesData, String> {
        if bytes.len() != FILE_SIZE {
            return Err(format!("The file has {} bytes. It must have {FILE_SIZE}.", bytes.len()));
        }
        SpeciesData::read_from_bytes(bytes)
            .map_err(|e| format!("The file could not be read: {e}"))
    }

    /// Writes the data as the bytes of a file.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    /// An array of the six base stats.
    pub fn base_stats(&self) -> [u8; 6] {
        [self.base_hp, self.base_attack, self.base_defense, self.base_speed, self.base_special_attack, self.base_special_defense]
    }

    /// The sum of the six base stats.
    pub fn base_total(&self) -> u32 {
        self.base_stats().iter().map(|&v| v as u32).sum()
    }

    /// The arithmetic mean of the six base stats.
    pub fn base_average(&self) -> f32 {
        self.base_total() as f32 / 6.0
    }

    /// The standard deviation of the six base stats (population standard deviation).
    pub fn base_standard_deviation(&self) -> f32 {
        let mean = self.base_average();
        let variance = self.base_stats().iter().map(|&v| {
            let diff = v as f32 - mean;
            diff * diff
        }).sum::<f32>() / 6.0;
        variance.sqrt()
    }

    /// The effort values that the Pokémon gives when it faints: 2 bits per stat (0 to 3).
    pub fn ev_hp(&self) -> u8 {
        (self.ev_yield & 0x3) as u8
    }

    pub fn ev_attack(&self) -> u8 {
        ((self.ev_yield >> 2) & 0x3) as u8
    }

    pub fn ev_defense(&self) -> u8 {
        ((self.ev_yield >> 4) & 0x3) as u8
    }

    pub fn ev_speed(&self) -> u8 {
        ((self.ev_yield >> 6) & 0x3) as u8
    }

    pub fn ev_special_attack(&self) -> u8 {
        ((self.ev_yield >> 8) & 0x3) as u8
    }

    pub fn ev_special_defense(&self) -> u8 {
        ((self.ev_yield >> 10) & 0x3) as u8
    }

    pub fn set_ev_hp(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !0x3) | (val as u16 & 0x3);
    }

    pub fn set_ev_attack(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !(0x3 << 2)) | ((val as u16 & 0x3) << 2);
    }

    pub fn set_ev_defense(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !(0x3 << 4)) | ((val as u16 & 0x3) << 4);
    }

    pub fn set_ev_speed(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !(0x3 << 6)) | ((val as u16 & 0x3) << 6);
    }

    pub fn set_ev_special_attack(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !(0x3 << 8)) | ((val as u16 & 0x3) << 8);
    }

    pub fn set_ev_special_defense(&mut self, val: u8) {
        self.ev_yield = (self.ev_yield & !(0x3 << 10)) | ((val as u16 & 0x3) << 10);
    }

    /// The total effort values given.
    pub fn ev_total(&self) -> u8 {
        self.ev_hp() + self.ev_attack() + self.ev_defense() + self.ev_speed() + self.ev_special_attack() + self.ev_special_defense()
    }

    /// Mask for the burrowed entry bit in `ev_yield` (bit 12: 1 = burrowed entry / Diglett & Dugtrio, 0 = standard drop).
    const BURROWED_ENTRY_MASK: u16 = 1 << 12;

    /// Returns true if the Pokémon emerges directly burrowed in the ground upon battle entry (e.g. Diglett, Dugtrio).
    pub fn is_burrowed_entry(&self) -> bool {
        (self.ev_yield & Self::BURROWED_ENTRY_MASK) != 0
    }

    /// Sets whether the Pokémon emerges directly burrowed in the ground upon battle entry.
    pub fn set_burrowed_entry(&mut self, burrowed: bool) {
        if burrowed {
            self.ev_yield |= Self::BURROWED_ENTRY_MASK;
        } else {
            self.ev_yield &= !Self::BURROWED_ENTRY_MASK;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The file 2 of the archive 195 (Ivysaur), as the dump of the game has it.
    const IVYSAUR: [u8; FILE_SIZE] = [
        0x3c, 0x3e, 0x3f, 0x3c, 0x50, 0x50, 0x0b, 0x03, 0x2d, 0x02, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x1f, 0x14, 0x46, 0x03, 0x01, 0x07, 0x41, 0x41, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x01, 0x03, 0x8e, 0x00, 0x64, 0x00, 0x82, 0x00, 0x20, 0x87, 0x39, 0x84, 0x08, 0x9a, 0x11, 0x00, //
        0x20, 0x04, 0xe0, 0xa2, 0x98, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x64, 0x00, 0x01, 0x00, //
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x04, 0x00, 0x00, 0x02, 0x20, 0x00, 0x00,
    ];

    #[test]
    fn the_fields_are_at_the_right_place() {
        let d = SpeciesData::from_bytes(&IVYSAUR).unwrap();
        assert_eq!((d.base_hp, d.base_attack, d.base_defense, d.base_speed, d.base_special_attack, d.base_special_defense), (60, 62, 63, 60, 80, 80));
        assert_eq!(d.base_total(), 405);
        assert!((d.base_average() - 67.5).abs() < 1e-4);
        assert!((d.base_standard_deviation() - 8.9022).abs() < 1e-3);
        assert_eq!((d.type_1, d.type_2), (11, 3)); // Grass and Poison
        assert_eq!(d.catch_rate, 45);
        assert_eq!((d.item_50, d.item_5), (0, 0));
        assert_eq!(d.gender_ratio, 31);
        assert_eq!(d.hatch_time, 20);
        assert_eq!(d.base_friendship, 70);
        assert_eq!(d.leveling_rate, 3); // Medium Slow
        assert_eq!((d.egg_group_1, d.egg_group_2), (1, 7));
        assert_eq!((d.ability_1, d.ability_2, d.ability_hidden), (65, 65, 34)); // Overgrow, Overgrow, Chlorophyll
        assert_eq!(d.total_form_count, 1);
        assert_eq!(d.color, 3); // Green
        assert_eq!(d.base_experience, 142);
        assert_eq!(d.weight, 130);
        assert_eq!(d.tm_hm_moves[0], 0x8439_8720);
        assert_eq!((d.ev_hp(), d.ev_attack(), d.ev_defense(), d.ev_speed(), d.ev_special_attack(), d.ev_special_defense()), (0, 0, 0, 0, 1, 1));
        assert_eq!(d.ev_total(), 2);
        assert!(!d.is_burrowed_entry());
    }

    #[test]
    fn burrowed_entry_toggles_bit_12_without_touching_evs() {
        let mut d = SpeciesData::from_bytes(&IVYSAUR).unwrap();
        assert!(!d.is_burrowed_entry());
        assert_eq!(d.ev_special_attack(), 1);
        assert_eq!(d.ev_special_defense(), 1);

        d.set_burrowed_entry(true);
        assert!(d.is_burrowed_entry());
        assert_eq!(d.ev_special_attack(), 1);
        assert_eq!(d.ev_special_defense(), 1);
        assert_eq!(d.ev_yield, 0x0500 | (1 << 12));

        d.set_burrowed_entry(false);
        assert!(!d.is_burrowed_entry());
        assert_eq!(d.ev_yield, 0x0500);
    }

    #[test]
    fn writing_gives_the_same_bytes() {
        let d = SpeciesData::from_bytes(&IVYSAUR).unwrap();
        assert_eq!(d.to_bytes(), IVYSAUR.to_vec());
    }

    #[test]
    fn a_wrong_size_is_refused() {
        assert!(SpeciesData::from_bytes(&[0; 79]).is_err());
        assert!(SpeciesData::from_bytes(&[0; 81]).is_err());
    }

    /// With the dump of the game: every file of the archive 195 is read and written back.
    #[test]
    fn every_file_of_the_dump_is_written_back_the_same() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let garc = sango_core::Garc::open(&std::path::Path::new(&root).join("a/1/9/5")).unwrap();
        assert_eq!(garc.file_count(), 827);
        // The last file is the whole table in one piece: the 826 files before it, one after the other.
        let last = garc.file_count() - 1;
        let table = garc.first_file(last).unwrap();
        assert_eq!(table.len(), last * FILE_SIZE);
        for id in 0..last {
            let bytes = garc.first_file(id).unwrap();
            assert_eq!(SpeciesData::from_bytes(bytes).unwrap().to_bytes(), bytes, "file {id}");
            assert_eq!(&table[id * FILE_SIZE..(id + 1) * FILE_SIZE], bytes, "file {id} in the table");
        }
    }
}
