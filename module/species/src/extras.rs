//! The species and the forms that the packs add (see `docs/species-packs.md`): the pages show them like the species of the game.
//!
//! Studio writes `species/table.bin` when a pack is installed. This module reads it and gives, for a species number, its name,
//! its data, its forms, its moves and its evolutions. The species of the packs have the numbers 724 and more (the numbers 722
//! and 723 are not species). A removed species has an empty name and is not listed.

use crate::data::SpeciesData;
use crate::names;
use sango_core::SdCard;
use sango_pack::table::{FormRecord, SpeciesRecord, FIRST_SPECIES};
use sango_pack::Table;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// The number of species of the game: the numbers 1 to 721.
pub const GAME_SPECIES: usize = names::SPECIES.len();

/// How long the table is kept before the file is looked at again.
const KEEP: Duration = Duration::from_millis(1500);

struct State {
    table: Table,
    /// The bytes that the table was read from, to know when the file changed.
    bytes: Vec<u8>,
    checked: Option<Instant>,
}

static STATE: RwLock<State> = RwLock::new(State { table: Table { species: Vec::new(), forms: Vec::new() }, bytes: Vec::new(), checked: None });

/// Reads `species/table.bin` of the SD card when it changed. It looks at the file once each second and a half at most.
/// Returns true when the species or the forms changed.
pub fn refresh(sd: Option<&SdCard>) -> bool {
    if STATE.read().ok().and_then(|state| state.checked).is_some_and(|at| at.elapsed() < KEEP) {
        return false;
    }
    let bytes = sd.and_then(|sd| sd.read(sango_pack::library::TABLE_PATH)).unwrap_or_default();
    let Ok(mut state) = STATE.write() else { return false };
    state.checked = Some(Instant::now());
    if state.bytes == bytes {
        return false;
    }
    state.table = if bytes.is_empty() { Table::default() } else { Table::from_bytes(&bytes).unwrap_or_default() };
    state.bytes = bytes;
    true
}

/// Gives the table to a test: the species and the forms that the packs would add.
#[cfg(test)]
pub fn set_for_test(table: Table) {
    if let Ok(mut state) = STATE.write() {
        state.bytes = table.to_bytes().unwrap_or_default();
        state.table = table;
        state.checked = Some(Instant::now() + Duration::from_secs(3600));
    }
}

fn with<R>(use_table: impl FnOnce(&Table) -> R) -> R {
    match STATE.read() {
        Ok(state) => use_table(&state.table),
        Err(_) => use_table(&Table::default()),
    }
}

/// The species of a pack with this number, when it is there and not removed.
pub fn pack_species(species: usize) -> Option<SpeciesRecord> {
    let id = u16::try_from(species).ok()?;
    with(|table| table.species.iter().find(|s| s.id == id && !s.name.is_empty()).cloned())
}

/// True for a number that is a species of the game or of a pack.
pub fn is_species(species: usize) -> bool {
    (1..=GAME_SPECIES).contains(&species) || pack_species(species).is_some()
}

/// The name of a species of the game or of a pack, or "-" for a number that is not a species.
pub fn species_name(species: usize) -> String {
    if (1..=GAME_SPECIES).contains(&species) {
        names::SPECIES[species - 1].to_string()
    } else {
        pack_species(species).map_or("-".to_string(), |s| s.name)
    }
}

/// All the numbers of species, in order: the species of the game, then the species of the packs.
pub fn species_ids() -> Vec<usize> {
    let mut ids: Vec<usize> = (1..=GAME_SPECIES).collect();
    with(|table| ids.extend(table.species.iter().filter(|s| !s.name.is_empty()).map(|s| s.id as usize)));
    ids
}

/// The highest number of a species: the rows of the lists have this size.
pub fn max_species() -> usize {
    with(|table| table.species.iter().filter(|s| !s.name.is_empty()).map(|s| s.id as usize).max()).unwrap_or(GAME_SPECIES).max(GAME_SPECIES)
}

/// The forms that the packs add to a species, in the order of the table.
pub fn pack_forms(species: usize) -> Vec<FormRecord> {
    let Ok(id) = u16::try_from(species) else { return Vec::new() };
    with(|table| table.forms.iter().filter(|f| f.species == id && f.data.iter().any(|&b| b != 0)).cloned().collect())
}

/// The number of forms of a species counting the forms of the packs: one more than the highest form number of a pack, or
/// `game` (the number of forms of the game) when that is more.
pub fn form_count(species: usize, game: usize) -> usize {
    let from_packs = pack_forms(species).iter().map(|f| f.form as usize + 1).max().unwrap_or(0);
    game.max(from_packs).max(1)
}

/// The data that a pack gives to a species or a form: the species of a pack (form 0), or a form that a pack adds. None for
/// the species and the forms of the game.
pub fn data(species: usize, form: usize) -> Option<SpeciesData> {
    let bytes = if form == 0 && species >= FIRST_SPECIES as usize {
        pack_species(species)?.data
    } else {
        pack_forms(species).into_iter().find(|f| f.form as usize == form)?.data
    };
    SpeciesData::from_bytes(&bytes).ok()
}

/// The model form that a pack gives to a species or a form (see the pack format): None when the model is the one of the game.
pub fn model_form(species: usize, form: usize) -> Option<u16> {
    if form == 0 && species >= FIRST_SPECIES as usize {
        return pack_species(species).map(|s| s.model_form);
    }
    pack_forms(species).into_iter().find(|f| f.form as usize == form).map(|f| f.model_form).filter(|&m| m != 0)
}

#[cfg(test)]
mod test {
    use super::*;

    fn table() -> Table {
        let record = |id: u16, name: &str| SpeciesRecord { id, model_form: 1, name: name.into(), data: vec![7; 0x50], moves: vec![(33, 1)], evolutions: Vec::new() };
        Table {
            species: vec![record(724, "Rowlet"), record(725, ""), record(726, "Decidueye")],
            forms: vec![FormRecord { species: 3, form: 2, model_form: 0, data: vec![9; 0x50] }, FormRecord { species: 726, form: 1, model_form: 4, data: vec![5; 0x50] }],
        }
    }

    #[test]
    fn the_species_and_the_forms_of_the_packs_are_found_like_the_ones_of_the_game() {
        set_for_test(table());
        assert_eq!((species_name(1), species_name(724), species_name(725), species_name(900)), ("Bulbasaur".to_string(), "Rowlet".to_string(), "-".to_string(), "-".to_string()));
        assert!(is_species(721) && is_species(724) && !is_species(722) && !is_species(725));
        let ids = species_ids();
        assert_eq!((ids.len(), ids[720], ids[721], ids[722]), (723, 721, 724, 726), "a removed species is not listed");
        assert_eq!(max_species(), 726);
        // The forms: Venusaur gets the form 2, Decidueye the form 1.
        assert_eq!((form_count(3, 2), form_count(726, 1), form_count(1, 1)), (3, 2, 1));
        assert_eq!(data(724, 0).map(|d| d.base_hp), Some(7));
        assert_eq!(data(3, 2).map(|d| d.base_hp), Some(9));
        assert!(data(3, 1).is_none() && data(1, 0).is_none(), "the data of the game is not here");
        assert_eq!((model_form(724, 0), model_form(726, 1), model_form(3, 2)), (Some(1), Some(4), None));
    }
}
