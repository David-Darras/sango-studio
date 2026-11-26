//! What the page needs from the game: the data of the species and of the moves, the names, and the places of the maps. The files
//! of the SD card come first, then the dump, so the page follows the changes of the other pages.

use crate::data::Place;
use crate::pokemon::{self, Pokemon};
use sango_core::experience::{self, Curve};
use sango_core::learnset::{self, LevelUp};
use sango_core::natures::NatureTable;
use sango_core::text::{TextFile, LANGUAGES};
use sango_core::{moves, Dump, Garc, SdCard};
use sango_species_base::data::SpeciesData;
use sango_species_base::extras;
use std::collections::HashMap;

/// The files of the texts that have the names of the species and of the items.
const SPECIES_NAMES: usize = 98;

/// A map of the game, as the menu shows it.
pub struct Map {
    pub id: u16,
    pub name: String,
    /// The number of the name of its place: the game keeps it for a Pokémon that is met there.
    pub place: u16,
    /// Where the game puts a player who arrives on the map without a door.
    pub start: Place,
    /// Where Fly lands from this map: it can be on another map.
    pub fly: Place,
}

/// The data of the game that the page reads.
pub struct GameData {
    dump: Dump,
    sd: Option<SdCard>,
    species: Garc,
    experience: Garc,
    learnsets: Garc,
    moves: Option<moves::MoveTable>,
    pub natures: NatureTable,
    /// The names of the species in each language that was read.
    names: HashMap<usize, Option<TextFile>>,
    pub maps: Vec<Map>,
}

/// The place of the list of the languages of the language of a Pokémon or of a player.
fn language_index(language: u8) -> usize {
    match language {
        1 => 0,
        3 => 3,
        4 => 4,
        5 => 5,
        7 => 6,
        8 => 7,
        _ => 2,
    }
}

impl GameData {
    pub fn open(dump: &Dump, sd: Option<&SdCard>) -> Result<GameData, String> {
        let open = |number: usize| Garc::open(&dump.archive(number));
        extras::refresh(sd);
        sango_species::custom_moves::refresh(sd);
        let natures = sd.and_then(|sd| sd.read("nature/natures.csv")).and_then(|b| NatureTable::parse(&String::from_utf8_lossy(&b)).ok()).unwrap_or_else(NatureTable::game);
        Ok(GameData {
            dump: dump.clone(),
            sd: sd.cloned(),
            species: open(sango_species::ARCHIVE)?,
            experience: open(experience::ARCHIVE)?,
            learnsets: open(learnset::LEVEL_UP_ARCHIVE)?,
            moves: moves::MoveTable::from_garc(&open(moves::ARCHIVE)?).ok(),
            natures,
            names: HashMap::new(),
            maps: read_maps(dump, sd),
        })
    }

    fn card(&self, path: &str) -> Option<Vec<u8>> {
        self.sd.as_ref().and_then(|sd| sd.read(path))
    }

    /// The data of a species or of a form.
    pub fn species(&self, species: u16, form: u8) -> Option<SpeciesData> {
        let (species, form) = (usize::from(species), usize::from(form));
        let on_card = self.card(&sango_species_base::sd_path(species, form)).and_then(|b| SpeciesData::from_bytes(&b).ok());
        on_card.or_else(|| sango_species_base::game_data(&self.species, species, form))
    }

    /// The experience table of a species.
    fn curve(&self, data: &SpeciesData) -> Option<Curve> {
        let rate = usize::from(data.leveling_rate);
        let bytes = self.card(&experience::sd_path(rate)).or_else(|| self.experience.first_file(rate).map(<[u8]>::to_vec))?;
        Curve::from_bytes(&bytes).ok()
    }

    /// The experience that a Pokémon has when it reaches a level.
    pub fn experience(&self, species: u16, form: u8, level: u8) -> Option<u32> {
        self.curve(&self.species(species, form)?)?.experience(usize::from(level.clamp(1, 100)))
    }

    /// The level that a number of experience points gives.
    pub fn level(&self, species: u16, form: u8, points: u32) -> Option<u8> {
        let curve = self.curve(&self.species(species, form)?)?;
        Some((1..=100u8).rev().find(|&level| curve.experience(usize::from(level)).is_some_and(|needed| needed <= points)).unwrap_or(1))
    }

    /// The power points of a move without a PP Up.
    pub fn move_points(&self, id: u16) -> u8 {
        if id == 0 {
            return 0;
        }
        if let Some(changed) = self.card(&format!("move/data/{id}.bin")).filter(|b| b.len() == moves::ENTRY_SIZE) {
            return changed[5];
        }
        sango_species::custom_moves::info_of(id).or_else(|| self.moves.as_ref().and_then(|table| table.get(usize::from(id)).copied())).map_or(5, |m| m.pp)
    }

    /// The most power points of a move with a number of PP Ups.
    pub fn full_points(&self, id: u16, ups: u8) -> u8 {
        let points = u32::from(self.move_points(id));
        (points * (5 + u32::from(ups.min(3))) / 5).min(255) as u8
    }

    /// The four last moves that a species learns at a level or before it.
    pub fn moves_at(&self, species: u16, form: u8, level: u8) -> Vec<u16> {
        let file = sango_species_base::file_index(&self.species, usize::from(species), usize::from(form));
        let bytes = self.card(&learnset::level_up_sd_path(file)).or_else(|| self.learnsets.first_file(file).map(<[u8]>::to_vec));
        let Some(table) = bytes.and_then(|b| LevelUp::from_bytes(&b).ok()) else { return Vec::new() };
        let mut known: Vec<u16> = Vec::new();
        for learned in table.moves.iter().filter(|m| m.level <= u16::from(level)) {
            known.retain(|&id| id != learned.id);
            known.push(learned.id);
        }
        known.split_off(known.len().saturating_sub(4))
    }

    /// The name of a species in the language of a Pokémon: the name that it has without a nickname.
    pub fn species_name(&mut self, species: u16, language: u8) -> String {
        let index = language_index(language);
        let (dump, sd) = (&self.dump, &self.sd);
        let file = self.names.entry(index).or_insert_with(|| {
            let on_card = sd.as_ref().and_then(|sd| sd.read(&sango_core::text::sd_path(index, SPECIES_NAMES)));
            let bytes = on_card.or_else(|| Garc::open(&dump.archive(LANGUAGES[index].archive)).ok()?.first_file(SPECIES_NAMES).map(<[u8]>::to_vec))?;
            TextFile::from_bytes(&bytes).ok()
        });
        file.as_ref()
            .and_then(|file| file.text(usize::from(species)))
            .filter(|name| !name.is_empty() && usize::from(species) <= extras::GAME_SPECIES)
            .unwrap_or_else(|| extras::species_name(usize::from(species)))
    }

    /// Calculates the level and the statistics of a Pokémon of the party from its experience.
    pub fn calculate(&self, pokemon: &mut Pokemon) {
        let (species, form) = (pokemon.species(), pokemon.form());
        let Some(data) = self.species(species, form) else { return };
        let level = self.level(species, form, pokemon.experience()).unwrap_or(1);
        let base = [data.base_hp, data.base_attack, data.base_defense, data.base_special_attack, data.base_special_defense, data.base_speed];
        let plain = pokemon::plain_stats(species, base, pokemon, level);
        let mut values = [0u16; 6];
        // The table of the natures has the five statistics after the HP, in the order of the studio.
        for (stat, value) in values.iter_mut().enumerate() {
            let with_nature = if stat == 0 { plain[0] } else { self.natures.apply(usize::from(pokemon.nature()), stat - 1, plain[stat]) };
            *value = with_nature.min(u32::from(u16::MAX)) as u16;
        }
        pokemon.set_calculated(level, values);
    }

    pub fn map(&self, id: u16) -> Option<&Map> {
        self.maps.iter().find(|map| map.id == id)
    }
}

fn read_maps(dump: &Dump, sd: Option<&SdCard>) -> Vec<Map> {
    use sango_zone::data::{self as zone, Store};
    let Ok(store) = Store::open(dump) else { return Vec::new() };
    let names = store.name_files(sd);
    let settings: Vec<Option<zone::Settings>> = (0..store.zone_count()).map(|id| store.settings(sd, id).ok().map(|(settings, _)| settings)).collect();
    let point = |settings: &zone::Settings, map: u16, fields: [zone::Field; 3]| {
        let [x, y, z] = fields.map(|field| settings.get(field) as f32);
        // The game looks down when a player arrives on a map without a door.
        Place { map, x, y, z, facing: 1 }
    };
    settings
        .iter()
        .enumerate()
        .filter_map(|(id, found)| {
            let found = found.as_ref()?;
            let id = id as u16;
            let name = zone::name_of(&names, 2, found.get(zone::PLACE_NAME) as usize).filter(|n| !n.is_empty()).unwrap_or_else(|| "(no name)".to_string());
            let start = point(found, id, [zone::START_X, zone::START_Y, zone::START_Z]);
            let fly = point(found, found.get(zone::FLY_MAP) as u16, [zone::FLY_X, zone::FLY_Y, zone::FLY_Z]);
            Some(Map { id, name, place: found.get(zone::PLACE_NAME) as u16, start, fly })
        })
        .collect()
}
