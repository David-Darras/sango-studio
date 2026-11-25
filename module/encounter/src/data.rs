//! The wild Pokémon of a map: the table of the game, and how it goes in the file of the map.
//!
//! Each map of the archive 13 is a bundle of resources. The resource 3 is the table of the wild Pokémon, when the map has some (150 maps have
//! one, the others have an empty resource). All the numbers are little-endian.
//!
//! | Offset | Size | Name | Meaning |
//! | --- | --- | --- | --- |
//! | 0 | 14 | rates | One byte for each kind of place (see [`METHODS`]). 0 means no wild Pokémon there. Five bytes are not used. |
//! | 14 | 4 × 61 | slots | The 61 places, one after the other: species, form, minimum level, maximum level. |
//! | 258 | 2 | padding | Always 0. |
//!
//! A place is `u16` (the species in the 11 low bits, the form in the 5 high bits), then the two levels. The game picks the place by a
//! percentage of its own (see [`Method::chances`]), and then the species and a level between the two levels of the place.

use sango_core::{bundle, lz, Garc};
use std::path::{Path, PathBuf};

/// The number of places in the table.
pub const SLOT_COUNT: usize = 61;
/// The number of rate bytes.
pub const RATE_COUNT: usize = 14;
/// The size of the resource of a map.
pub const PART_SIZE: usize = 260;
/// The number of the resource of the table in a map.
pub const PART_INDEX: usize = 3;
/// The archive of the maps.
pub const ZONE_ARCHIVE: usize = 13;

/// One place of the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    /// The species (0 for no Pokémon).
    pub species: u16,
    /// The form of the species (0 to 31).
    pub form: u8,
    pub min_level: u8,
    pub max_level: u8,
}

impl Slot {
    fn from_bytes(bytes: &[u8]) -> Slot {
        let packed = u16::from_le_bytes([bytes[0], bytes[1]]);
        Slot { species: packed & 0x7FF, form: (packed >> 11) as u8, min_level: bytes[2], max_level: bytes[3] }
    }

    fn put(&self, out: &mut Vec<u8>) {
        let packed = (self.species & 0x7FF) | (u16::from(self.form & 0x1F) << 11);
        out.extend_from_slice(&packed.to_le_bytes());
        out.push(self.min_level);
        out.push(self.max_level);
    }
}

/// A kind of place where wild Pokémon appear.
#[derive(Clone, Copy, Debug)]
pub struct Method {
    pub name: &'static str,
    /// What the player does to meet the Pokémon.
    pub help: &'static str,
    /// The first place of the kind, in the 61 places.
    pub first: usize,
    /// The number of places.
    pub count: usize,
    /// The rate byte of the kind.
    pub rate_at: usize,
    /// The chance of each place, in percent. The list is repeated for each group of places (the hordes have three groups of five).
    pub chances: &'static [u8],
    /// The number of places in a group (a horde has 5 Pokémon).
    pub group: usize,
}

/// What the rate byte of a kind means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateKind {
    /// The chance of a meeting at each step, in percent (0 for none).
    Chance,
    /// A choice of a table: 0 none, then the names.
    Table(&'static [&'static str]),
}

impl Method {
    /// What the rate byte of the kind means.
    pub fn rate_kind(&self) -> RateKind {
        match self.rate_at {
            2 => RateKind::Table(&["Ground", "Water"]),
            4 => RateKind::Table(&["Normal", "Fossils"]),
            _ => RateKind::Chance,
        }
    }

    /// The chance of the place number `place` of the kind, in percent.
    pub fn chance(&self, place: usize) -> u8 {
        self.chances.get(place % self.chances.len().max(1)).copied().unwrap_or(0)
    }
}

const GROUND: [u8; 12] = [10, 10, 10, 10, 10, 10, 10, 10, 10, 5, 4, 1];
const WATER: [u8; 5] = [50, 30, 15, 4, 1];
const THREE: [u8; 3] = [60, 35, 5];

/// The nine kinds of places, in the order of the table.
pub const METHODS: [Method; 9] = [
    Method { name: "Grass", help: "The tall grass, the caves and the other ground where the Pokémon appear at each step.", first: 0, count: 12, rate_at: 0, chances: &GROUND, group: 12 },
    Method { name: "Special grass", help: "A second table of the ground (the flowers, the rough ground): the game uses it where the map says so.", first: 12, count: 12, rate_at: 1, chances: &GROUND, group: 12 },
    Method { name: "Extra", help: "Three more places that the game adds to the ground or to the water.", first: 24, count: 3, rate_at: 2, chances: &THREE, group: 3 },
    Method { name: "Surf", help: "The water, when the player surfs.", first: 27, count: 5, rate_at: 3, chances: &WATER, group: 5 },
    Method { name: "Rock Smash", help: "The rocks that Rock Smash breaks.", first: 32, count: 5, rate_at: 4, chances: &WATER, group: 5 },
    Method { name: "Old Rod", help: "Fishing with the Old Rod.", first: 37, count: 3, rate_at: 5, chances: &THREE, group: 3 },
    Method { name: "Good Rod", help: "Fishing with the Good Rod.", first: 40, count: 3, rate_at: 6, chances: &THREE, group: 3 },
    Method { name: "Super Rod", help: "Fishing with the Super Rod.", first: 43, count: 3, rate_at: 7, chances: &THREE, group: 3 },
    Method { name: "Horde", help: "The hordes: five Pokémon that appear together. The table has three hordes.", first: 46, count: 15, rate_at: 8, chances: &THREE, group: 5 },
];

/// The wild Pokémon of a map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encounter {
    pub rates: [u8; RATE_COUNT],
    pub slots: [Slot; SLOT_COUNT],
}

impl Default for Encounter {
    fn default() -> Encounter {
        Encounter { rates: [0; RATE_COUNT], slots: [Slot::default(); SLOT_COUNT] }
    }
}

impl Encounter {
    /// Reads the resource of a map.
    pub fn parse(bytes: &[u8]) -> Result<Encounter, String> {
        if bytes.len() < RATE_COUNT + 4 * SLOT_COUNT {
            return Err("The table of the wild Pokémon is too short.".into());
        }
        let mut table = Encounter::default();
        table.rates.copy_from_slice(&bytes[..RATE_COUNT]);
        for (i, slot) in table.slots.iter_mut().enumerate() {
            *slot = Slot::from_bytes(&bytes[RATE_COUNT + 4 * i..]);
        }
        Ok(table)
    }

    /// The resource of a map.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PART_SIZE);
        out.extend_from_slice(&self.rates);
        for slot in &self.slots {
            slot.put(&mut out);
        }
        out.resize(PART_SIZE, 0);
        out
    }

    /// The places of a kind.
    pub fn places(&self, method: &Method) -> &[Slot] {
        &self.slots[method.first..method.first + method.count]
    }

    /// The places of a kind, to change.
    pub fn places_mut(&mut self, method: &Method) -> &mut [Slot] {
        &mut self.slots[method.first..method.first + method.count]
    }

    /// The species that a kind of place can give, with the total chance of each one in percent. The hordes count by horde: a species is as
    /// likely as the hordes that have it.
    pub fn species_of(&self, method: &Method) -> Vec<(u16, u8, u32)> {
        let mut found: Vec<(u16, u8, u32)> = Vec::new();
        let mut counted: Vec<(usize, u16, u8)> = Vec::new();
        for (place, slot) in self.places(method).iter().enumerate() {
            if slot.species == 0 {
                continue;
            }
            let per_place = method.group == method.count;
            let (group, chance) = if per_place { (place, method.chance(place) as u32) } else { (place / method.group, method.chance(place / method.group) as u32) };
            if counted.contains(&(group, slot.species, slot.form)) {
                continue;
            }
            counted.push((group, slot.species, slot.form));
            match found.iter_mut().find(|(species, form, _)| *species == slot.species && *form == slot.form) {
                Some(entry) => entry.2 += chance,
                None => found.push((slot.species, slot.form, chance)),
            }
        }
        found
    }
}

/// Where the files of the maps are.
pub struct Zones {
    /// The archive 13 of the dump.
    garc: Garc,
    /// The folder `overworld/zone` of the SD card.
    folder: PathBuf,
}

/// The bytes of a map and where they come from.
pub struct Pack {
    /// The bundle, not compressed.
    pub bytes: Vec<u8>,
    /// True when the file was compressed (the file that Sango Studio writes is compressed the same way).
    pub compressed: bool,
    /// True when the file is the one of the SD card, false for the one of the game.
    pub from_sd: bool,
}

impl Zones {
    pub fn open(dump: &sango_core::Dump, overworld_folder: &Path) -> Result<Zones, String> {
        let garc = Garc::open(&dump.archive(ZONE_ARCHIVE))?;
        Ok(Zones { garc, folder: overworld_folder.join("zone") })
    }

    /// The number of maps of the game.
    pub fn count(&self) -> usize {
        self.garc.file_count()
    }

    fn file_of(&self, zone: usize) -> PathBuf {
        self.folder.join(format!("{zone}.bin"))
    }

    /// The file of a map: the one of the SD card, else the one of the game.
    pub fn pack(&self, zone: usize) -> Result<Pack, String> {
        let (raw, from_sd) = match std::fs::read(self.file_of(zone)) {
            Ok(bytes) => (bytes, true),
            Err(_) => (self.garc.file_or_first(zone, 0).ok_or("This map is not in the archive.")?.to_vec(), false),
        };
        let compressed = lz::is_compressed(&raw);
        let bytes = if compressed { lz::decompress(&raw)? } else { raw };
        Ok(Pack { bytes, compressed, from_sd })
    }

    /// The wild Pokémon of a map as the game has them. `None` when the map has no table.
    pub fn game_table(&self, zone: usize) -> Result<Option<Encounter>, String> {
        let raw = self.garc.file_or_first(zone, 0).ok_or("This map is not in the archive.")?;
        let bytes = if lz::is_compressed(raw) { lz::decompress(raw)? } else { raw.to_vec() };
        table_of(&bytes)
    }

    /// The wild Pokémon of a map, with the change of the SD card. `None` when the map has no table.
    pub fn table(&self, zone: usize) -> Result<Option<Encounter>, String> {
        table_of(&self.pack(zone)?.bytes)
    }

    /// Writes the wild Pokémon of a map in the file of the SD card, with the rest of the map as it is there (the events that the
    /// Overworld page wrote stay).
    pub fn write(&self, zone: usize, table: &Encounter) -> Result<usize, String> {
        let pack = self.pack(zone)?;
        let ranges = bundle::ranges(&pack.bytes)?;
        if ranges.len() <= PART_INDEX {
            return Err("This map has no place for a table of wild Pokémon.".into());
        }
        let new_pack = bundle::replace_part_with(&pack.bytes, PART_INDEX, &table.to_bytes(), 4)?;
        let bytes = if pack.compressed { lz::compress_lz11(&new_pack) } else { new_pack };
        std::fs::create_dir_all(&self.folder).map_err(|e| format!("{}: {e}", self.folder.display()))?;
        let path = self.file_of(zone);
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        write_index(&self.folder, zone, bytes.len())?;
        Ok(bytes.len())
    }

    /// Gives the map its wild Pokémon of the game again. When the rest of the file is the one of the game, the file leaves the SD card.
    pub fn reset(&self, zone: usize) -> Result<(), String> {
        let Some(original) = self.game_table(zone)? else {
            return Ok(());
        };
        if !self.file_of(zone).exists() {
            return Ok(());
        }
        let game = self.garc.file_or_first(zone, 0).ok_or("This map is not in the archive.")?;
        let game_bytes = if lz::is_compressed(game) { lz::decompress(game)? } else { game.to_vec() };
        let pack = self.pack(zone)?;
        let restored = bundle::replace_part_with(&pack.bytes, PART_INDEX, &original.to_bytes(), 4)?;
        if restored == game_bytes {
            let path = self.file_of(zone);
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            remove_index(&self.folder, zone)
        } else {
            self.write(zone, &original).map(|_| ())
        }
    }

    /// The maps that have a table in the game, and the maps whose table the user changed.
    pub fn with_tables(&self) -> (Vec<usize>, Vec<usize>) {
        let mut game = Vec::new();
        let mut changed = Vec::new();
        for zone in 0..self.count() {
            let Ok(Some(original)) = self.game_table(zone) else { continue };
            game.push(zone);
            if self.file_of(zone).exists() && self.table(zone).ok().flatten().is_some_and(|now| now != original) {
                changed.push(zone);
            }
        }
        (game, changed)
    }
}

fn table_of(pack: &[u8]) -> Result<Option<Encounter>, String> {
    let (_, parts) = bundle::split(pack)?;
    match parts.get(PART_INDEX) {
        Some(part) if part.len() >= RATE_COUNT + 4 * SLOT_COUNT => Ok(Some(Encounter::parse(part)?)),
        _ => Ok(None),
    }
}

fn write_index(folder: &Path, id: usize, size: usize) -> Result<(), String> {
    let path = folder.join("index.bin");
    let mut entries: std::collections::BTreeMap<u32, u32> =
        std::fs::read(&path).ok().map(|old| sango_core::file_lists::parse_index(&old)).transpose()?.unwrap_or_default().into_iter().collect();
    entries.insert(id as u32, size as u32);
    let files: Vec<(u32, u32)> = entries.into_iter().collect();
    std::fs::write(&path, sango_core::file_lists::index_bytes(&files)).map_err(|e| format!("{}: {e}", path.display()))
}

fn remove_index(folder: &Path, id: usize) -> Result<(), String> {
    let path = folder.join("index.bin");
    let Ok(old) = std::fs::read(&path) else { return Ok(()) };
    let mut entries: std::collections::BTreeMap<u32, u32> = sango_core::file_lists::parse_index(&old)?.into_iter().collect();
    entries.remove(&(id as u32));
    if entries.is_empty() {
        return std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()));
    }
    let files: Vec<(u32, u32)> = entries.into_iter().collect();
    std::fs::write(&path, sango_core::file_lists::index_bytes(&files)).map_err(|e| format!("{}: {e}", path.display()))
}

/// The header line of the CSV file of the wild Pokémon of a map. A line `rate` gives a rate of the table (its place from 0, and the value); a
/// line `place` gives a place of a kind (its number from 1, the species, the form and the levels).
pub const CSV_HEADER: &str = "line,kind,number,species_or_rate,form,min_level,max_level";

impl Encounter {
    /// The table as a CSV file.
    pub fn to_csv(&self) -> String {
        let mut out = format!("{CSV_HEADER}\n");
        for (place, rate) in self.rates.iter().enumerate() {
            let kind = METHODS.iter().find(|m| m.rate_at == place).map_or("", |m| m.name);
            out.push_str(&format!("rate,{kind},{place},{rate},,,\n"));
        }
        for method in &METHODS {
            for (number, slot) in self.places(method).iter().enumerate() {
                out.push_str(&format!("place,{},{},{},{},{},{}\n", method.name, number + 1, slot.species, slot.form, slot.min_level, slot.max_level));
            }
        }
        out
    }

    /// Reads a CSV file of [`Encounter::to_csv`]. The lines that are not in the file keep the values of `base`.
    pub fn from_csv(text: &str, base: &Encounter) -> Result<Encounter, String> {
        let mut table = base.clone();
        for (number, line) in text.lines().enumerate().skip(1) {
            let at = |message: String| format!("Line {}: {message}", number + 1);
            if line.trim().is_empty() {
                continue;
            }
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            if cells.len() < 4 {
                return Err(at("a line has at least 4 values.".into()));
            }
            let value = |i: usize, max: u32| -> Result<u32, String> {
                let v: u32 = cells.get(i).copied().unwrap_or("").parse().map_err(|_| at(format!("\"{}\" is not a number.", cells.get(i).copied().unwrap_or(""))))?;
                if v > max {
                    return Err(at(format!("{v} is bigger than {max}.")));
                }
                Ok(v)
            };
            match cells[0] {
                "rate" => {
                    let place = value(2, RATE_COUNT as u32 - 1)? as usize;
                    table.rates[place] = value(3, 255)? as u8;
                }
                "place" => {
                    let method = METHODS.iter().find(|m| m.name == cells[1]).ok_or_else(|| at(format!("\"{}\" is not a kind of place.", cells[1])))?;
                    let place = value(2, method.count as u32)? as usize;
                    if place == 0 {
                        return Err(at("the places start at 1.".into()));
                    }
                    let slot = &mut table.places_mut(method)[place - 1];
                    *slot = Slot { species: value(3, 0x7FF)? as u16, form: value(4, 31)? as u8, min_level: value(5, 100)? as u8, max_level: value(6, 100)? as u8 };
                }
                other => return Err(at(format!("\"{other}\" is not rate or place."))),
            }
        }
        Ok(table)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_table_goes_through_its_bytes() {
        let mut table = Encounter::default();
        table.rates[0] = 6;
        table.slots[0] = Slot { species: 263, form: 0, min_level: 2, max_level: 3 };
        table.slots[60] = Slot { species: 2047, form: 31, min_level: 100, max_level: 100 };
        let bytes = table.to_bytes();
        assert_eq!(bytes.len(), PART_SIZE);
        assert_eq!(Encounter::parse(&bytes).unwrap(), table);
    }

    #[test]
    fn the_places_of_the_kinds_cover_the_table_once() {
        let mut next = 0;
        for method in &METHODS {
            assert_eq!(method.first, next);
            assert_eq!(method.count % method.group, 0);
            next += method.count;
        }
        assert_eq!(next, SLOT_COUNT);
    }

    #[test]
    fn the_chances_of_each_group_make_a_hundred() {
        for method in &METHODS {
            assert_eq!(method.chances.iter().map(|&c| u32::from(c)).sum::<u32>(), 100, "{}", method.name);
        }
    }

    #[test]
    fn a_table_is_written_and_given_back() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&path).unwrap();
        let folder = std::env::temp_dir().join("sango_encounter_write_test");
        let _ = std::fs::remove_dir_all(&folder);
        let zones = Zones::open(&dump, &folder).unwrap();
        let zone = zones.with_tables().0[0];
        let original = zones.game_table(zone).unwrap().unwrap();
        let mut changed = original.clone();
        changed.slots[0] = Slot { species: 25, form: 0, min_level: 7, max_level: 9 };
        zones.write(zone, &changed).unwrap();
        assert_eq!(zones.table(zone).unwrap().unwrap(), changed);
        assert_eq!(zones.with_tables().1, vec![zone]);
        // The rest of the map is the one of the game.
        let now = zones.pack(zone).unwrap().bytes;
        let (_, parts) = bundle::split(&now).unwrap();
        let game = zones.garc.file_or_first(zone, 0).unwrap();
        let game = if lz::is_compressed(game) { lz::decompress(game).unwrap() } else { game.to_vec() };
        let (_, game_parts) = bundle::split(&game).unwrap();
        assert_eq!(parts[1], game_parts[1]);
        zones.reset(zone).unwrap();
        assert!(!folder.join("zone").join(format!("{zone}.bin")).exists());
        assert_eq!(zones.table(zone).unwrap().unwrap(), original);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn the_real_maps_keep_their_bytes() {
        let Ok(path) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&path).unwrap();
        let folder = std::env::temp_dir().join("sango_encounter_test");
        let zones = Zones::open(&dump, &folder).unwrap();
        let (with_table, _) = zones.with_tables();
        assert!(with_table.len() > 100, "{}", with_table.len());
        for zone in with_table {
            let table = zones.game_table(zone).unwrap().unwrap();
            let raw = zones.pack(zone).unwrap().bytes;
            let (_, parts) = bundle::split(&raw).unwrap();
            assert_eq!(parts[PART_INDEX], &table.to_bytes()[..], "map {zone}");
        }
    }

    #[test]
    fn a_table_goes_to_csv_and_comes_back() {
        let mut table = Encounter::default();
        table.rates[0] = 10;
        table.slots[3] = Slot { species: 25, form: 1, min_level: 3, max_level: 5 };
        table.slots[50] = Slot { species: 399, form: 0, min_level: 10, max_level: 12 };
        let text = table.to_csv();
        assert_eq!(Encounter::from_csv(&text, &Encounter::default()).unwrap(), table);
        assert!(Encounter::from_csv("h\nplace,Grass,13,1,0,1,1", &table).is_err(), "Grass has 12 places");
        assert!(Encounter::from_csv("h\nplace,Lava,1,1,0,1,1", &table).is_err());
    }
}
