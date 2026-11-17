//! `species/table.bin`: the file that the plugin reads to add species and forms to the game.
//!
//! ```text
//! Header, 16 bytes:   "SSPT", version u32 (2), species count u16, form count u16, 4 bytes of zero
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
pub const VERSION: u32 = 2;
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
    /// What lets a Pokémon of the species change into this form in a battle, like a Mega Evolution.
    pub mega: MegaCondition,
}

/// What a Pokémon needs to change into a form in a battle, like a Mega Evolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MegaCondition {
    /// The form is not a Mega Evolution: the Pokémon never changes into it.
    Never,
    /// No condition: the Pokémon can always change into it.
    Always,
    /// The Pokémon holds this item (the number of the item).
    Item(u16),
    /// The Pokémon knows this move (the number of the move).
    Move(u16),
    /// The Pokémon has this ability (the number of the ability).
    Ability(u16),
}

/// The largest number that a condition keeps: 13 bits in the file.
pub const MEGA_ARGUMENT_MAX: u16 = 0x1FFF;

impl MegaCondition {
    /// The 16 bits of the file: the kind in the 3 high bits (0 never, 1 always, 2 item, 3 move, 4 ability), the number in the 13 others.
    pub fn to_bits(self) -> u16 {
        let (kind, number) = match self {
            MegaCondition::Never => (0u16, 0),
            MegaCondition::Always => (1, 0),
            MegaCondition::Item(n) => (2, n),
            MegaCondition::Move(n) => (3, n),
            MegaCondition::Ability(n) => (4, n),
        };
        kind << 13 | (number & MEGA_ARGUMENT_MAX)
    }

    /// The condition of 16 bits of the file. A kind that this program does not know gives `Never`.
    pub fn from_bits(bits: u16) -> MegaCondition {
        let number = bits & MEGA_ARGUMENT_MAX;
        match bits >> 13 {
            1 => MegaCondition::Always,
            2 => MegaCondition::Item(number),
            3 => MegaCondition::Move(number),
            4 => MegaCondition::Ability(number),
            _ => MegaCondition::Never,
        }
    }

    /// The number of the item, the move or the ability. 0 for the others.
    pub fn number(self) -> u16 {
        match self {
            MegaCondition::Item(n) | MegaCondition::Move(n) | MegaCondition::Ability(n) => n,
            _ => 0,
        }
    }

    /// True when a Pokémon can change into the form.
    pub fn is_mega(self) -> bool {
        self != MegaCondition::Never
    }

    /// An error when the number does not fit in the file.
    pub fn check(self) -> Result<(), String> {
        if self.number() > MEGA_ARGUMENT_MAX {
            return Err(format!("The number {} is too large for a condition (the largest is {MEGA_ARGUMENT_MAX}).", self.number()));
        }
        Ok(())
    }
}

/// The items that the first four forms of a species of the older packs use to change in a battle (rank 0 to 3): the Life Orb, the Choice
/// Band, the Choice Specs and the Choice Scarf.
pub const LEGACY_MEGA_ITEMS: [u16; 4] = [0x010E, 0x00DC, 0x0129, 0x011F];

/// The condition of a form that does not say its own: the item of its rank among the forms of its species (the rule of the first packs).
pub fn legacy_mega(rank: usize) -> MegaCondition {
    LEGACY_MEGA_ITEMS.get(rank).map_or(MegaCondition::Never, |item| MegaCondition::Item(*item))
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
            put_u16(&mut out, form.mega.to_bits());
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
            table.forms.push(FormRecord { species: u16_at(at), form: u16_at(at + 2), model_form: u16_at(at + 4), data: bytes[at + 6..at + 6 + DATA_SIZE].to_vec(), mega: MegaCondition::from_bits(u16_at(at + 6 + DATA_SIZE)) });
        }
        Ok(table)
    }
}
