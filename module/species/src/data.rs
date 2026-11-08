//! The data of a species or of a form: one file of the archive 195.
//!
//! A file has 80 bytes. The first 64 bytes are the fields of `SpeciesData`, one after the other,
//! without padding. The last 16 bytes are not described yet: they are kept as they are.

/// The size of a file of the archive.
pub const FILE_SIZE: usize = 80;

/// The size of the part that has fields.
const FIELDS_SIZE: usize = 64;

/// The data of a species or of a form.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// A reader of little-endian numbers.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.bytes[self.at];
        self.at += 1;
        v
    }

    fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes([self.bytes[self.at], self.bytes[self.at + 1]]);
        self.at += 2;
        v
    }

    fn u32(&mut self) -> u32 {
        let b = &self.bytes[self.at..self.at + 4];
        self.at += 4;
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
}

impl SpeciesData {
    /// Reads the data from the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<SpeciesData, String> {
        if bytes.len() != FILE_SIZE {
            return Err(format!("The file has {} bytes. It must have {FILE_SIZE}.", bytes.len()));
        }
        let mut r = Reader { bytes, at: 0 };
        let data = SpeciesData {
            base_hp: r.u8(),
            base_attack: r.u8(),
            base_defense: r.u8(),
            base_speed: r.u8(),
            base_special_attack: r.u8(),
            base_special_defense: r.u8(),
            types: [r.u8(), r.u8()],
            capture_rate: r.u8(),
            unknown_0: r.u8(),
            give_effort_values: r.u16(),
            give_item: [r.u16(), r.u16(), r.u16()],
            gender: r.u8(),
            egg_hatch_steps: r.u8(),
            base_friendship: r.u8(),
            unknown_1: r.u8(),
            egg_group: [r.u8(), r.u8()],
            ability: [r.u8(), r.u8(), r.u8()],
            escape_rate: r.u8(),
            form_index: r.u16(),
            form_index_2: r.u16(),
            form_count: r.u8(),
            unknown_2: r.u8(),
            give_experience: r.u16(),
            height: r.u16(),
            weight: r.u16(),
            technical_moves: [r.u32(), r.u32(), r.u32(), r.u32(), r.u32()],
            fake_height: r.u16(),
            unknown_4: r.u16(),
            rest: bytes[FIELDS_SIZE..].try_into().map_err(|_| "The end of the file is cut.".to_string())?,
        };
        debug_assert_eq!(r.at, FIELDS_SIZE);
        Ok(data)
    }

    /// Writes the data as the bytes of a file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(FILE_SIZE);
        out.extend_from_slice(&[self.base_hp, self.base_attack, self.base_defense, self.base_speed, self.base_special_attack, self.base_special_defense]);
        out.extend_from_slice(&self.types);
        out.extend_from_slice(&[self.capture_rate, self.unknown_0]);
        out.extend_from_slice(&self.give_effort_values.to_le_bytes());
        for item in self.give_item {
            out.extend_from_slice(&item.to_le_bytes());
        }
        out.extend_from_slice(&[self.gender, self.egg_hatch_steps, self.base_friendship, self.unknown_1]);
        out.extend_from_slice(&self.egg_group);
        out.extend_from_slice(&self.ability);
        out.push(self.escape_rate);
        out.extend_from_slice(&self.form_index.to_le_bytes());
        out.extend_from_slice(&self.form_index_2.to_le_bytes());
        out.extend_from_slice(&[self.form_count, self.unknown_2]);
        out.extend_from_slice(&self.give_experience.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.weight.to_le_bytes());
        for word in self.technical_moves {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&self.fake_height.to_le_bytes());
        out.extend_from_slice(&self.unknown_4.to_le_bytes());
        out.extend_from_slice(&self.rest);
        out
    }

    /// The sum of the six base statistics.
    pub fn base_total(&self) -> u32 {
        [self.base_hp, self.base_attack, self.base_defense, self.base_speed, self.base_special_attack, self.base_special_defense].iter().map(|&v| v as u32).sum()
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
