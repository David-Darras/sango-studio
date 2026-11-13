//! `species/table.bin`: the file that the plugin reads to add species and forms to the game.
//!
//! ```text
//! Header, 16 bytes:   "SSPT", version u32 (1), species count u16, form count u16, 4 bytes of zero
//! Species, 256 bytes each, in the order of their numbers (the first one is the number FIRST_SPECIES):
//!    0  u16  number of the species          2  u16  model form
//!    4  24 x u16  name, UTF-16, zero at the end
//!   52  80 bytes  data of the species
//!  132  24 x {u16 move, u8 level, u8 0}      learnset by level, the list ends with the move 0
//!  228  4 x {u8 method, u8 0, u16 argument, u16 species}   evolutions, the list ends with the species 0
//!  252  4 bytes of zero
//! Forms, 88 bytes each, in the order that the plugin adds them:
//!    0  u16 species   2  u16 form   4  u16 model form   6  80 bytes data   86  2 bytes of zero
//! ```
//!
//! All the numbers are little-endian. A species or a form that was removed keeps its place with an empty name and zero data.

use crate::pack::{DATA_SIZE, MAX_EVOLUTIONS, MAX_MOVES, MAX_NAME};

pub const SIGNATURE: &[u8; 4] = b"SSPT";
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 16;
pub const SPECIES_SIZE: usize = 256;
pub const FORM_SIZE: usize = 88;

/// The number of the first species that a pack adds: after the 721 species of the game and the 3 places that the game keeps.
pub const FIRST_SPECIES: u16 = 724;

/// A species in the file, with the numbers that the plugin uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeciesRecord {
    pub id: u16,
    pub model_form: u16,
    pub name: String,
    pub data: Vec<u8>,
    /// Move and level.
    pub moves: Vec<(u16, u8)>,
    /// Method, argument and species.
    pub evolutions: Vec<(u8, u16, u16)>,
}

/// A form in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormRecord {
    pub species: u16,
    pub form: u16,
    pub model_form: u16,
    pub data: Vec<u8>,
}

/// What the file holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub species: Vec<SpeciesRecord>,
    pub forms: Vec<FormRecord>,
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

impl Table {
    /// The bytes of the file.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(HEADER_SIZE + self.species.len() * SPECIES_SIZE + self.forms.len() * FORM_SIZE);
        out.extend_from_slice(SIGNATURE);
        out.extend_from_slice(&VERSION.to_le_bytes());
        put_u16(&mut out, self.species.len() as u16);
        put_u16(&mut out, self.forms.len() as u16);
        out.extend_from_slice(&[0; 4]);
        for (place, species) in self.species.iter().enumerate() {
            if species.id != FIRST_SPECIES + place as u16 {
                return Err(format!("The species {} is at the place {place}: the numbers must follow each other from {FIRST_SPECIES}.", species.id));
            }
            if species.data.len() != DATA_SIZE || species.moves.len() > MAX_MOVES || species.evolutions.len() > MAX_EVOLUTIONS {
                return Err(format!("The species {} has a wrong size.", species.id));
            }
            let units: Vec<u16> = species.name.encode_utf16().collect();
            if units.len() > MAX_NAME {
                return Err(format!("The name of the species {} is too long.", species.id));
            }
            let start = out.len();
            put_u16(&mut out, species.id);
            put_u16(&mut out, species.model_form);
            for place in 0..24 {
                put_u16(&mut out, units.get(place).copied().unwrap_or(0));
            }
            out.extend_from_slice(&species.data);
            for place in 0..MAX_MOVES {
                let (id, level) = species.moves.get(place).copied().unwrap_or((0, 0));
                put_u16(&mut out, id);
                out.extend_from_slice(&[level, 0]);
            }
            for place in 0..MAX_EVOLUTIONS {
                let (method, argument, target) = species.evolutions.get(place).copied().unwrap_or((0, 0, 0));
                out.extend_from_slice(&[method, 0]);
                put_u16(&mut out, argument);
                put_u16(&mut out, target);
            }
            out.resize(start + SPECIES_SIZE, 0);
        }
        for form in &self.forms {
            if form.data.len() != DATA_SIZE {
                return Err(format!("A form of the species {} has a wrong size.", form.species));
            }
            put_u16(&mut out, form.species);
            put_u16(&mut out, form.form);
            put_u16(&mut out, form.model_form);
            out.extend_from_slice(&form.data);
            out.extend_from_slice(&[0, 0]);
        }
        Ok(out)
    }

    /// Reads the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Table, String> {
        let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        if bytes.len() < HEADER_SIZE || &bytes[..4] != SIGNATURE {
            return Err("This is not a species table.".into());
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != VERSION {
            return Err(format!("The species table has the version {version}: this program reads the version {VERSION}."));
        }
        let (species_count, form_count) = (u16_at(8) as usize, u16_at(10) as usize);
        if bytes.len() != HEADER_SIZE + species_count * SPECIES_SIZE + form_count * FORM_SIZE {
            return Err("The species table has a wrong size.".into());
        }
        let mut table = Table::default();
        for place in 0..species_count {
            let at = HEADER_SIZE + place * SPECIES_SIZE;
            let units: Vec<u16> = (0..24).map(|k| u16_at(at + 4 + k * 2)).take_while(|&unit| unit != 0).collect();
            let moves = (0..MAX_MOVES).map(|k| (u16_at(at + 132 + k * 4), bytes[at + 134 + k * 4])).take_while(|(id, _)| *id != 0).collect();
            let evolutions = (0..MAX_EVOLUTIONS).map(|k| (bytes[at + 228 + k * 6], u16_at(at + 230 + k * 6), u16_at(at + 232 + k * 6))).take_while(|(_, _, target)| *target != 0).collect();
            table.species.push(SpeciesRecord {
                id: u16_at(at),
                model_form: u16_at(at + 2),
                name: String::from_utf16(&units).map_err(|e| e.to_string())?,
                data: bytes[at + 52..at + 52 + DATA_SIZE].to_vec(),
                moves,
                evolutions,
            });
        }
        for place in 0..form_count {
            let at = HEADER_SIZE + species_count * SPECIES_SIZE + place * FORM_SIZE;
            table.forms.push(FormRecord { species: u16_at(at), form: u16_at(at + 2), model_form: u16_at(at + 4), data: bytes[at + 6..at + 6 + DATA_SIZE].to_vec() });
        }
        Ok(table)
    }
}
