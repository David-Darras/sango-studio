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
    pub base_hp: u8,
    pub base_attack: u8,
    pub base_defense: u8,
    pub base_speed: u8,
    pub base_special_attack: u8,
    pub base_special_defense: u8,
    /// The two types.
    pub types: [u8; 2],
    /// The catch rate.
    pub capture_rate: u8,
    pub unknown_0: u8,
    /// The effort values that the Pokémon gives when it faints.
    pub give_effort_values: u16,
    /// The items that the wild Pokémon can hold.
    pub give_item: [u16; 3],
    /// The gender ratio: 0 male only, 254 female only, 255 no gender.
    pub gender: u8,
    /// The steps to hatch an Egg, in cycles.
    pub egg_hatch_steps: u8,
    pub base_friendship: u8,
    pub unknown_1: u8,
    /// The Egg Groups.
    pub egg_group: [u8; 2],
    /// The two abilities and the hidden ability.
    pub ability: [u8; 3],
    pub escape_rate: u8,
    /// The place of the first form in the table, or 0.
    pub form_index: u16,
    pub form_index_2: u16,
    /// The number of forms.
    pub form_count: u8,
    pub unknown_2: u8,
    /// The base experience yield.
    pub give_experience: u16,
    /// The height.
    pub height: u16,
    /// The weight in hectograms.
    pub weight: u16,
    /// The technical machines and hidden machines that the species can learn: one bit each.
    pub technical_moves: [u32; 5],
    pub fake_height: u16,
    pub unknown_4: u16,
    /// The last 16 bytes of the file. They are not described yet.
    pub rest: [u8; 16],
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
        (self.give_effort_values & 0x3) as u8
    }

    pub fn ev_attack(&self) -> u8 {
        ((self.give_effort_values >> 2) & 0x3) as u8
    }

    pub fn ev_defense(&self) -> u8 {
        ((self.give_effort_values >> 4) & 0x3) as u8
    }

    pub fn ev_speed(&self) -> u8 {
        ((self.give_effort_values >> 6) & 0x3) as u8
    }

    pub fn ev_special_attack(&self) -> u8 {
        ((self.give_effort_values >> 8) & 0x3) as u8
    }

    pub fn ev_special_defense(&self) -> u8 {
        ((self.give_effort_values >> 10) & 0x3) as u8
    }

    pub fn set_ev_hp(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !0x3) | (val as u16 & 0x3);
    }

    pub fn set_ev_attack(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !(0x3 << 2)) | ((val as u16 & 0x3) << 2);
    }

    pub fn set_ev_defense(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !(0x3 << 4)) | ((val as u16 & 0x3) << 4);
    }

    pub fn set_ev_speed(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !(0x3 << 6)) | ((val as u16 & 0x3) << 6);
    }

    pub fn set_ev_special_attack(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !(0x3 << 8)) | ((val as u16 & 0x3) << 8);
    }

    pub fn set_ev_special_defense(&mut self, val: u8) {
        self.give_effort_values = (self.give_effort_values & !(0x3 << 10)) | ((val as u16 & 0x3) << 10);
    }

    /// The total effort values given.
    pub fn ev_total(&self) -> u8 {
        self.ev_hp() + self.ev_attack() + self.ev_defense() + self.ev_speed() + self.ev_special_attack() + self.ev_special_defense()
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
        assert_eq!(d.types, [11, 3]); // Grass and Poison
        assert_eq!(d.capture_rate, 45);
        assert_eq!(d.gender, 31);
        assert_eq!(d.egg_hatch_steps, 20);
        assert_eq!(d.base_friendship, 70);
        assert_eq!(d.egg_group, [1, 7]);
        assert_eq!(d.ability, [65, 65, 34]); // Overgrow, Overgrow, Chlorophyll
        assert_eq!(d.form_count, 1);
        assert_eq!(d.give_experience, 142);
        assert_eq!(d.weight, 130);
        assert_eq!(d.technical_moves[0], 0x8439_8720);
        assert_eq!((d.ev_hp(), d.ev_attack(), d.ev_defense(), d.ev_speed(), d.ev_special_attack(), d.ev_special_defense()), (0, 0, 0, 0, 1, 1));
        assert_eq!(d.ev_total(), 2);
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
