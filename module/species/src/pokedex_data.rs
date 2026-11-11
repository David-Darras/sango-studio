//! The data of the Pokédex that the species data does not hold: the file 1 of the archive 132.
//!
//! The file is a bundle of three resources:
//!
//! | Resource | Content |
//! | --- | --- |
//! | 0 | For each place of the tables, the place of the next form of the same species, or 0. |
//! | 1 | For each place of the tables, the shape: one byte. |
//! | 2 | For each species, its number in the regional Pokédex, or 0. |
//!
//! The place of a species is its number. The place of a form after the first one comes from
//! the table of the next forms: the form 1 of a species is at `next[species]`, the form 2 at
//! `next[next[species]]`, and so on.

use sango_core::Bundle;

/// The number of the archive.
pub const ARCHIVE: usize = 132;

/// The place of the file in the archive.
pub const FILE: usize = 1;

/// The place of the file on the SD card.
pub const SD_PATH: &str = "pokemon/pokedex_data.bin";

const NEXT_FORM: usize = 0;
const SHAPE: usize = 1;

/// The Pokédex data of all the species and forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PokedexData {
    bundle: Bundle,
}

impl PokedexData {
    /// Reads the bytes of the file.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<PokedexData, String> {
        let bundle = Bundle::parse(bytes)?;
        let next = bundle.resource(NEXT_FORM).ok_or("The table of the next forms is missing.")?;
        if bundle.resource(SHAPE).is_none() {
            return Err("The table of the shapes is missing.".into());
        }
        if next.len() % 2 != 0 {
            return Err("The table of the next forms has an odd size.".into());
        }
        Ok(PokedexData { bundle })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bundle.bytes().to_vec()
    }

    fn next_form(&self, place: usize) -> usize {
        let table = self.bundle.resource(NEXT_FORM).unwrap_or(&[]);
        table.get(place * 2..place * 2 + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize)
    }

    /// The place of a species and a form in the tables.
    ///
    /// A form that does not exist gives the place of the species.
    pub fn place(&self, species: usize, form: usize) -> usize {
        let mut place = species;
        for _ in 0..form {
            place = self.next_form(place);
            if place == 0 {
                return species;
            }
        }
        place
    }

    /// The shape of a species and a form: a number from 0 to 13.
    pub fn shape(&self, species: usize, form: usize) -> Option<u8> {
        self.bundle.resource(SHAPE)?.get(self.place(species, form)).copied()
    }

    /// Gives a shape to a species and a form. False when the form has no place in the table.
    pub fn set_shape(&mut self, species: usize, form: usize, shape: u8) -> bool {
        let place = self.place(species, form);
        match self.bundle.resource_mut(SHAPE).and_then(|t| t.get_mut(place)) {
            Some(byte) => {
                *byte = shape;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A file with 4 places: the species 1 has a second form at the place 3, the species 2 has one form.
    fn sample() -> PokedexData {
        let mut b = vec![b'T', b'M', 3, 0];
        for offset in [20u32, 28, 32, 40] {
            b.extend(offset.to_le_bytes());
        }
        b.extend([0, 0, 3, 0, 0, 0, 0, 0]); // next forms: the place 1 goes to the place 3
        b.extend([0, 4, 9, 7]); // shapes
        b.extend([0; 8]); // regional numbers
        PokedexData::from_bytes(b).unwrap()
    }

    #[test]
    fn a_form_is_found_through_the_next_forms() {
        let d = sample();
        assert_eq!(d.place(1, 0), 1);
        assert_eq!(d.place(1, 1), 3);
        assert_eq!(d.shape(1, 0), Some(4));
        assert_eq!(d.shape(1, 1), Some(7));
        assert_eq!(d.shape(2, 0), Some(9));
    }

    #[test]
    fn a_form_that_does_not_exist_gives_the_species() {
        assert_eq!(sample().place(2, 1), 2);
    }

    #[test]
    fn a_shape_changes_only_its_own_byte() {
        let mut d = sample();
        let before = d.to_bytes();
        assert!(d.set_shape(1, 1, 12));
        let after = d.to_bytes();
        assert_eq!(after.len(), before.len());
        assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 1);
        assert_eq!(d.shape(1, 1), Some(12));
        assert!(!d.set_shape(99, 0, 1));
    }

    /// With the dump of the game: Bulbasaur is a quadruped, Charizard is bipedal with a tail.
    #[test]
    fn the_shapes_of_the_dump_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let garc = sango_core::Garc::open(&std::path::Path::new(&root).join("a/1/3/2")).unwrap();
        let d = PokedexData::from_bytes(garc.first_file(FILE).unwrap().to_vec()).unwrap();
        assert_eq!(d.to_bytes(), garc.first_file(FILE).unwrap());
        assert_eq!(d.shape(1, 0), Some(4)); // Bulbasaur: four-legged
        assert_eq!(d.shape(6, 0), Some(9)); // Charizard: two legs and tail
        assert_eq!(d.shape(129, 0), Some(2)); // Magikarp: fish
    }
}
