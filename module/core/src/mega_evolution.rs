//! The Mega Evolutions of a species.
//!
//! Archive 193 has one file for each species, in the same order as the species data.
//! A file has 24 bytes: 3 entries of 8 bytes, with little-endian `u16` fields.
//!
//! | Offset in entry | Content |
//! | --- | --- |
//! | 0 | The target form: 0 for an unused entry, otherwise the form number (e.g. 1 for Mega, 2 for Mega Y). |
//! | 2 | The method: 0 for None / inactive, 1 for held item, 2 for known move (Rayquaza). |
//! | 4 | The argument: item number for item method, move number for move method. |
//! | 6 | Additional parameter (unused in base game, 0). |
//!
//! All fields are little-endian.

use crate::bytes::{put16, u16_at};

/// The archive of the Mega Evolution tables.
pub const ARCHIVE: usize = 193;

/// The maximum number of Mega Evolution entries a species can have in the game table.
pub const MAX_ENTRIES: usize = 3;

/// The size of one entry, in bytes.
pub const ENTRY_SIZE: usize = 8;

/// The total size of a Mega Evolution table file, in bytes.
pub const FILE_SIZE: usize = MAX_ENTRIES * ENTRY_SIZE;

/// The path of the Mega Evolution file of a species on the SD card.
pub fn sd_path(species: usize) -> String {
    format!("species/mega_evolution/{species}.bin")
}

/// How a Pokémon Mega Evolves in battle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MegaMethod {
    /// Does not Mega Evolve through this entry.
    #[default]
    None = 0,
    /// Mega Evolves when holding a specific item (Mega Stone).
    Item = 1,
    /// Mega Evolves when knowing a specific move (Rayquaza / Dragon Ascent).
    Move = 2,
}

impl MegaMethod {
    /// Converts a raw 16-bit integer from the file into a method.
    pub fn from_u16(value: u16) -> Self {
        match value {
            1 => Self::Item,
            2 => Self::Move,
            _ => Self::None,
        }
    }

    /// Returns the integer value of the method.
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// One Mega Evolution entry for a species.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MegaEntry {
    /// The form number that the Pokémon transforms into.
    pub evolved_form: u16,
    /// The trigger method for this Mega Evolution.
    pub method: MegaMethod,
    /// The required item or move number.
    pub argument: u16,
    /// Unused secondary parameter (reserved, 0).
    pub argument2: u16,
}

impl MegaEntry {
    /// Creates an empty entry.
    pub const fn empty() -> Self {
        Self {
            evolved_form: 0,
            method: MegaMethod::None,
            argument: 0,
            argument2: 0,
        }
    }

    /// True when this entry is unused or inactive.
    pub fn is_empty(&self) -> bool {
        self.method == MegaMethod::None || self.evolved_form == 0
    }

    /// Reads an entry from an 8-byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < ENTRY_SIZE {
            return Err(format!("A Mega Evolution entry needs {ENTRY_SIZE} bytes, got {}.", bytes.len()));
        }
        let evolved_form = u16_at(bytes, 0)?;
        let method = MegaMethod::from_u16(u16_at(bytes, 2)?);
        let argument = u16_at(bytes, 4)?;
        let argument2 = u16_at(bytes, 6)?;
        Ok(Self { evolved_form, method, argument, argument2 })
    }

    /// Writes this entry into an 8-byte buffer.
    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let mut buffer = [0u8; ENTRY_SIZE];
        put16(&mut buffer, 0, self.evolved_form);
        put16(&mut buffer, 2, self.method.as_u16());
        put16(&mut buffer, 4, self.argument);
        put16(&mut buffer, 6, self.argument2);
        buffer
    }
}

/// The Mega Evolution table for one species (up to 3 entries).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MegaEvolutionTable {
    pub entries: [MegaEntry; MAX_ENTRIES],
}

impl MegaEvolutionTable {
    /// Creates an empty Mega Evolution table.
    pub const fn empty() -> Self {
        Self {
            entries: [MegaEntry::empty(); MAX_ENTRIES],
        }
    }

    /// True when none of the entries are active.
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(MegaEntry::is_empty)
    }

    /// The number of active Mega Evolution entries in this table.
    pub fn entry_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_empty()).count()
    }

    /// Reads the table from raw bytes (typically 24 bytes).
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < FILE_SIZE {
            return Err(format!("A Mega Evolution table needs {FILE_SIZE} bytes, got {}.", bytes.len()));
        }
        let mut entries = [MegaEntry::empty(); MAX_ENTRIES];
        for (i, entry) in entries.iter_mut().enumerate() {
            let offset = i * ENTRY_SIZE;
            *entry = MegaEntry::from_bytes(&bytes[offset..offset + ENTRY_SIZE])?;
        }
        Ok(Self { entries })
    }

    /// Reads the table from raw bytes (alias for parse).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        Self::parse(bytes)
    }

    /// Serializes this table into 24 bytes.
    pub fn to_bytes(&self) -> [u8; FILE_SIZE] {
        let mut output = [0u8; FILE_SIZE];
        for (i, entry) in self.entries.iter().enumerate() {
            let offset = i * ENTRY_SIZE;
            output[offset..offset + ENTRY_SIZE].copy_from_slice(&entry.to_bytes());
        }
        output
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn an_empty_table_serializes_and_parses_back() {
        let table = MegaEvolutionTable::empty();
        assert!(table.is_empty());
        assert_eq!(table.entry_count(), 0);
        let bytes = table.to_bytes();
        assert_eq!(bytes, [0u8; FILE_SIZE]);
        let parsed = MegaEvolutionTable::parse(&bytes).unwrap();
        assert_eq!(parsed, table);
    }

    #[test]
    fn roundtrip_with_custom_entries() {
        let mut table = MegaEvolutionTable::empty();
        table.entries[0] = MegaEntry { evolved_form: 1, method: MegaMethod::Item, argument: 660, argument2: 0 };
        table.entries[1] = MegaEntry { evolved_form: 2, method: MegaMethod::Item, argument: 678, argument2: 0 };
        assert_eq!(table.entry_count(), 2);
        let bytes = table.to_bytes();
        let parsed = MegaEvolutionTable::parse(&bytes).unwrap();
        assert_eq!(parsed, table);
    }

    #[test]
    fn the_mega_evolution_tables_of_the_dump_match_known_data() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let garc = crate::Garc::open(&dump.archive(ARCHIVE)).unwrap();
        assert_eq!(garc.file_count(), 826);

        // Venusaur (3): Mega Venusaur with Venusaurite (659)
        let venusaur_bytes = garc.first_file(3).unwrap();
        let venusaur = MegaEvolutionTable::parse(venusaur_bytes).unwrap();
        assert_eq!(venusaur.entry_count(), 1);
        assert_eq!(venusaur.entries[0].evolved_form, 1);
        assert_eq!(venusaur.entries[0].method, MegaMethod::Item);
        assert_eq!(venusaur.entries[0].argument, 659);

        // Charizard (6): Mega X (660) and Mega Y (678)
        let charizard_bytes = garc.first_file(6).unwrap();
        let charizard = MegaEvolutionTable::parse(charizard_bytes).unwrap();
        assert_eq!(charizard.entry_count(), 2);
        assert_eq!(charizard.entries[0].evolved_form, 1);
        assert_eq!(charizard.entries[0].method, MegaMethod::Item);
        assert_eq!(charizard.entries[0].argument, 660);
        assert_eq!(charizard.entries[1].evolved_form, 2);
        assert_eq!(charizard.entries[1].method, MegaMethod::Item);
        assert_eq!(charizard.entries[1].argument, 678);

        // Rayquaza (384): Delta with known move
        let rayquaza_bytes = garc.first_file(384).unwrap();
        let rayquaza = MegaEvolutionTable::parse(rayquaza_bytes).unwrap();
        assert_eq!(rayquaza.entry_count(), 1);
        assert_eq!(rayquaza.entries[0].evolved_form, 1);
        assert_eq!(rayquaza.entries[0].method, MegaMethod::Move);
    }
}
