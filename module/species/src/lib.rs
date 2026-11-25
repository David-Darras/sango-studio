//! The species editor: one form for the data of a species or of a form.
//!
//! The data comes from the archive 195 of the romfs. The file of the SD card
//! `species/data/<species>_<form>.bin` takes the place of the romfs when it exists.

mod csv;
mod data_tools;
mod evolution;
mod evolution_csv;
mod examples;
mod icon_files;
pub mod icon_pack;
mod cry_tab;
mod icon_tab;
mod learnset;
mod learnset_csv;
mod machines;
pub mod custom_moves;
pub mod model_export;
mod model_import;
mod old_layout;
mod model_tab;
pub use model_tab::{read_game_model, read_pack_model};
mod mega_tab;
mod pokedex_data;
mod table;
mod text_csv;
mod text_tab;
pub mod menus;

/// The names of the 18 types, in the order of the game: Normal to Fairy.
pub use move_names::MOVE as MOVE_NAMES;
pub use names::ABILITY as ABILITY_NAMES;
pub use names::ITEM as ITEM_NAMES;
pub use names::TYPE as TYPE_NAMES;

pub use sango_species_base::{battle_icons, data, extras, form_names, icons, move_names, names};
pub use sango_species_base::{form_choices, form_key, form_leveling_rates, from_form_key, menu_form_count, species_form_menu, species_form_name, game_type_menu, leveling_rates, open_species, open_species_form, searchable_picker, species_abilities, summaries, type_menu, SpeciesAbilities, SpeciesIcons, Summary};
use sango_species_base::{file_index, game_data, menu, name_from, sd_path, OPEN_SPECIES, SPECIES_COUNT};
use data::SpeciesData;
use examples::{Examples, Field};
use pokedex_data::PokedexData;
use sango_core::Garc;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// The number of the archive that has the data of the species.
const ARCHIVE: usize = 195;

pub use extras::{is_species, max_species, species_ids, species_name};
pub use names::{is_unused_rate, LEVELING_RATE, SPECIES};
pub use table::SpeciesTable;

/// A GLB file of a model of the game that is a plain BCH with its textures (the models of the trainers): the skeleton, the meshes and
/// the pictures, in the rest pose.
pub fn glb_of_bch(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let model = sango_core::model::parse_model(bytes)?;
    let textures = sango_core::model::parse_textures(bytes)?;
    let by_name: HashMap<String, sango_core::model::Texture> = textures.into_iter().map(|t| (t.name.clone(), t)).collect();
    model_export::glb(&model, &by_name, &[])
}

/// The tabs of the page. Only the first one works for now.
const TABS: [&str; 4] = ["General", "Model", "Learnset", "Evolution"];

/// The data and the shape of all the species of the romfs, form 0: the source of the examples in the help.
fn build_index(garc: &Garc, pokedex: &PokedexData) -> examples::Index {
    let rows = (1..=SPECIES_COUNT)
        .filter_map(|species| {
            let data = SpeciesData::from_bytes(garc.first_file(species)?).ok()?;
            Some((species, data, pokedex.shape(species, 0).unwrap_or(0)))
        })
        .collect();
    examples::Index::new(rows)
}

pub struct Species {
    /// The dump that the archive comes from.
    root: Option<PathBuf>,
    garc: Option<Garc>,
    species: usize,
    form: usize,
    /// The data in the form.
    data: Option<SpeciesData>,
    /// The data of the game (or of the pack) of the open species and form: the fields that differ from it show a mark and a button
    /// that puts the value of the game back.
    game: Option<SpeciesData>,
    /// The window of the button "+ New species", when it is open.
    new_pokemon: Option<NewPokemon>,
    /// The species and the form that the user chose while the open one had changes that were not saved: the window asks first.
    pending_open: Option<(usize, usize)>,
    /// The data as it was loaded or saved: to know what changed.
    baseline: Option<SpeciesData>,
    /// True when the data came from the SD card.
    from_sd: bool,
    /// The Pokédex data (the shapes). It comes from the SD card when the file is there.
    pokedex: Option<PokedexData>,
    /// The archive of the Pokédex data.
    pokedex_archive: Option<Garc>,
    /// The shape of the chosen species and form.
    shape: u8,
    /// The shape as it was loaded or saved.
    shape_baseline: u8,
    /// True when the Pokédex data came from the SD card.
    pokedex_from_sd: bool,
    /// The species and the form that `data` is for.
    loaded: Option<(usize, usize)>,
    tab: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// True while the list of the species is open: it shows the chosen species at the middle once.
    list_shown: bool,
    /// The list must show the chosen species at the middle (after a key).
    scroll_list: bool,
    /// Text typed into the species search box.
    species_search: String,
    /// True once the search box has received focus on opening the dropdown.
    search_focused: bool,
    /// The archive of the Pokémon icons.
    icon_garc: Option<Garc>,
    /// The archive of the Mega Evolution tables (archive 193).
    mega_garc: Option<Garc>,
    /// The Mega Evolution table of the open species (archive 193).
    pub(crate) mega_table: Option<sango_core::mega_evolution::MegaEvolutionTable>,
    /// True when the Mega Evolution table came from the SD card.
    mega_from_sd: bool,
    /// The icon textures, one per species: decoded at the first display, then kept.
    icon_textures: HashMap<usize, Option<egui::TextureHandle>>,
    /// The form icon textures: decoded at the first display, then kept.
    form_icon_textures: HashMap<(usize, usize), Option<egui::TextureHandle>>,
    /// The types and the base stats of all the species, for the species list.
    list_rows: Option<Vec<ListEntry>>,
    /// The column that sorts the species list.
    sort_column: SortColumn,
    /// True when the species list goes from the largest value to the smallest one.
    sort_descending: bool,
    /// The examples of the help of the fields: built once for each dump.
    examples: Option<Arc<examples::Index>>,
    /// The state of the Learnset tab.
    learnset: learnset::Learnset,
    /// The state of the Evolution tab.
    evolution: evolution::EvolutionTab,
    /// The state of the Text tab.
    text: text_tab::TextTab,
    /// The state of the icon card of the General tab.
    icon_tab: icon_tab::IconTab,
    cry_tab: cry_tab::CryTab,
    /// The state of the Model tab.
    model_tab: model_tab::ModelTab,
    /// The list of items (the rows and the pictures) that the fields of held items use.
    item_list: std::sync::Arc<std::sync::Mutex<sango_ui::item_menu::ItemList>>,
    /// The CSV files, the resets and the undo.
    tools: data_tools::Tools,
}

impl Default for Species {
    fn default() -> Self {
        Species {
            root: None,
            garc: None,
            species: 1,
            form: 0,
            data: None,
            baseline: None,
            game: None,
            new_pokemon: None,
            pending_open: None,
            from_sd: false,
            pokedex: None,
            pokedex_archive: None,
            shape: 0,
            shape_baseline: 0,
            pokedex_from_sd: false,
            loaded: None,
            tab: 0,
            status: None,
            list_shown: false,
            scroll_list: false,
            species_search: String::new(),
            search_focused: false,
            icon_garc: None,
            mega_garc: None,
            mega_table: None,
            mega_from_sd: false,
            icon_textures: HashMap::new(),
            form_icon_textures: HashMap::new(),
            list_rows: None,
            sort_column: SortColumn::Number,
            sort_descending: false,
            examples: None,
            learnset: learnset::Learnset::default(),
            evolution: evolution::EvolutionTab::default(),
            text: text_tab::TextTab::default(),
            icon_tab: icon_tab::IconTab::default(),
            cry_tab: cry_tab::CryTab::default(),
            model_tab: model_tab::ModelTab::default(),
            item_list: Default::default(),
            tools: data_tools::Tools::default(),
        }
    }
}

/// What the species list shows and sorts for one species: the types and the base stats.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ListRow {
    /// The two types. The second one is the first one again when the species has one type.
    types: [u8; 2],
    /// The base stats: HP, Attack, Defense, Special Attack, Special Defense, Speed.
    stats: [u8; 6],
    /// The sum of the six base stats.
    total: u16,
    /// True when a file of the SD card (data, moves or evolutions) is not the same as in the game.
    modified: bool,
}

impl ListRow {
    fn with_modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }
}

/// One line of the list of the page: a species, or one of its forms. A form is a Pokémon of its own in the list ("Rotom-Heat"), as in
/// Pokémon Showdown: it has its own types and stats.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ListEntry {
    species: usize,
    form: usize,
    /// The name in the list: see [`entry_name`].
    name: String,
    row: ListRow,
}

/// The texture of the icon of a species and a form (the SD card first, then the romfs): decoded at the first display, then kept in
/// `textures`.
pub(crate) fn form_icon<'a>(
    textures: &'a mut HashMap<(usize, usize), Option<egui::TextureHandle>>,
    icon_garc: Option<&Garc>,
    sd: Option<&sango_core::SdCard>,
    ctx: &egui::Context,
    species: usize,
    form: usize,
) -> Option<&'a egui::TextureHandle> {
    let file = crate::icon_tab::icon_file(species, form, false).or_else(|| crate::icon_tab::icon_file(species, 0, false))?;
    textures
        .entry((species, form))
        .or_insert_with(|| {
            let on_card = sd.and_then(|sd| sd.read(&crate::icon_tab::sd_path(file)));
            let bytes = on_card.or_else(|| icon_garc?.first_file(file).map(|b| b.to_vec()))?;
            let rgba = sango_core::clim::decode_sized(&bytes, 40, 30).ok()?;
            let pixels = egui::ColorImage::from_rgba_unmultiplied([40, 30], &rgba);
            Some(sango_ui::textures::load(ctx, format!("form_icon_{species}_{form}"), pixels, egui::TextureOptions::NEAREST))
        })
        .as_ref()
}

/// The state of the window of the button "+ New species".
struct NewPokemon {
    /// True: a species of its own (a number of its own, for example a creature of another game). False: a new form of a species.
    own: bool,
    /// The species that gets the new form.
    species: usize,
    /// The name of the form in the menus ("Ice" gives "Rotom-Ice"), or the name of the new species.
    name: String,
    /// True: a copy of the open Pokémon. False: a template.
    copy: bool,
}

/// The data of a new species-form made from a template: the data of the open Pokémon with stats of 50, the Normal type, no EV yield and
/// no held item. The fields of the forms stay: the game needs them.
fn template_data(open: &SpeciesData) -> SpeciesData {
    let mut data = open.clone();
    data.base_hp = 50;
    data.base_attack = 50;
    data.base_defense = 50;
    data.base_special_attack = 50;
    data.base_special_defense = 50;
    data.base_speed = 50;
    data.type_1 = 0;
    data.type_2 = 0;
    for set in [SpeciesData::set_ev_hp, SpeciesData::set_ev_attack, SpeciesData::set_ev_defense, SpeciesData::set_ev_special_attack, SpeciesData::set_ev_special_defense, SpeciesData::set_ev_speed] {
        set(&mut data, 0);
    }
    data.item_50 = 0;
    data.item_5 = 0;
    data
}

/// The keys in the memory of the interface: the group of the fields that are drawn, the fields that differ from the game, and the field
/// whose value of the game the user asked back.
const GROUP_ID: &str = "species_field_group";
const MODIFIED_ID: &str = "species_modified_fields";
const REVERT_ID: &str = "species_revert_field";

/// The fields of the General tab that can show a mark and go back to the value of the game: "group/label".
const FIELD_KEYS: [&str; 31] = [
    "Base stats/HP", "Base stats/Attack", "Base stats/Defense", "Base stats/Special Attack", "Base stats/Special Defense", "Base stats/Speed",
    "EV yield/HP", "EV yield/Attack", "EV yield/Defense", "EV yield/Special Attack", "EV yield/Special Defense", "EV yield/Speed",
    "Characteristics/Type 1", "Characteristics/Type 2", "Characteristics/Ability 1", "Characteristics/Ability 2", "Characteristics/Hidden ability",
    "Characteristics/Egg Group 1", "Characteristics/Egg Group 2", "Characteristics/Held Item 50%", "Characteristics/Held Item 5%",
    "Characteristics/Leveling rate", "Characteristics/Color", "Characteristics/Gender ratio", "Characteristics/Hatch time",
    "Characteristics/Height", "Characteristics/Weight", "Characteristics/Catch rate", "Characteristics/Base friendship",
    "Characteristics/Base experience", "Characteristics/Burrowed entry",
];

/// The value of a field of [`FIELD_KEYS`].
fn field_value(key: &str, d: &SpeciesData) -> Option<u32> {
    Some(match key {
        "Base stats/HP" => d.base_hp as u32,
        "Base stats/Attack" => d.base_attack as u32,
        "Base stats/Defense" => d.base_defense as u32,
        "Base stats/Special Attack" => d.base_special_attack as u32,
        "Base stats/Special Defense" => d.base_special_defense as u32,
        "Base stats/Speed" => d.base_speed as u32,
        "EV yield/HP" => d.ev_hp() as u32,
        "EV yield/Attack" => d.ev_attack() as u32,
        "EV yield/Defense" => d.ev_defense() as u32,
        "EV yield/Special Attack" => d.ev_special_attack() as u32,
        "EV yield/Special Defense" => d.ev_special_defense() as u32,
        "EV yield/Speed" => d.ev_speed() as u32,
        "Characteristics/Type 1" => d.type_1 as u32,
        "Characteristics/Type 2" => d.type_2 as u32,
        "Characteristics/Ability 1" => d.ability_1 as u32,
        "Characteristics/Ability 2" => d.ability_2 as u32,
        "Characteristics/Hidden ability" => d.ability_hidden as u32,
        "Characteristics/Egg Group 1" => d.egg_group_1 as u32,
        "Characteristics/Egg Group 2" => d.egg_group_2 as u32,
        "Characteristics/Held Item 50%" => d.item_50 as u32,
        "Characteristics/Held Item 5%" => d.item_5 as u32,
        "Characteristics/Leveling rate" => d.leveling_rate as u32,
        "Characteristics/Color" => d.color as u32,
        "Characteristics/Gender ratio" => d.gender_ratio as u32,
        "Characteristics/Hatch time" => d.hatch_time as u32,
        "Characteristics/Height" => d.height as u32,
        "Characteristics/Weight" => d.weight as u32,
        "Characteristics/Catch rate" => d.catch_rate as u32,
        "Characteristics/Base friendship" => d.base_friendship as u32,
        "Characteristics/Base experience" => d.base_experience as u32,
        "Characteristics/Burrowed entry" => d.is_burrowed_entry() as u32,
        _ => return None,
    })
}

/// Gives a value to a field of [`FIELD_KEYS`].
fn set_field(key: &str, d: &mut SpeciesData, value: u32) {
    let (byte, word) = (value as u8, value as u16);
    match key {
        "Base stats/HP" => d.base_hp = byte,
        "Base stats/Attack" => d.base_attack = byte,
        "Base stats/Defense" => d.base_defense = byte,
        "Base stats/Special Attack" => d.base_special_attack = byte,
        "Base stats/Special Defense" => d.base_special_defense = byte,
        "Base stats/Speed" => d.base_speed = byte,
        "EV yield/HP" => d.set_ev_hp(byte),
        "EV yield/Attack" => d.set_ev_attack(byte),
        "EV yield/Defense" => d.set_ev_defense(byte),
        "EV yield/Special Attack" => d.set_ev_special_attack(byte),
        "EV yield/Special Defense" => d.set_ev_special_defense(byte),
        "EV yield/Speed" => d.set_ev_speed(byte),
        "Characteristics/Type 1" => d.type_1 = byte,
        "Characteristics/Type 2" => d.type_2 = byte,
        "Characteristics/Ability 1" => d.ability_1 = byte,
        "Characteristics/Ability 2" => d.ability_2 = byte,
        "Characteristics/Hidden ability" => d.ability_hidden = byte,
        "Characteristics/Egg Group 1" => d.egg_group_1 = byte,
        "Characteristics/Egg Group 2" => d.egg_group_2 = byte,
        "Characteristics/Held Item 50%" => d.item_50 = word,
        "Characteristics/Held Item 5%" => d.item_5 = word,
        "Characteristics/Leveling rate" => d.leveling_rate = byte,
        "Characteristics/Color" => d.color = byte,
        "Characteristics/Gender ratio" => d.gender_ratio = byte,
        "Characteristics/Hatch time" => d.hatch_time = byte,
        "Characteristics/Height" => d.height = word,
        "Characteristics/Weight" => d.weight = word,
        "Characteristics/Catch rate" => d.catch_rate = byte,
        "Characteristics/Base friendship" => d.base_friendship = byte,
        "Characteristics/Base experience" => d.base_experience = word,
        "Characteristics/Burrowed entry" => d.set_burrowed_entry(value != 0),
        _ => {}
    }
}

/// The name of a species and a form in the list ("Rotom-Heat").
fn entry_name(species: usize, form: usize) -> String {
    sango_species_base::species_form_name(species, form)
}

/// The number of forms of a species (at least one): the forms of the game and the forms of the packs.
fn form_count_of(garc: Option<&Garc>, species: usize) -> usize {
    let base = garc.filter(|_| species <= SPECIES_COUNT).and_then(|g| g.first_file(species)).and_then(|b| SpeciesData::from_bytes(b).ok());
    extras::form_count(species, base.map_or(1, |b| (b.total_form_count as usize).max(1)))
}

/// The species and the form before or after another one in the order of the numbers: the forms of a species, then the next species.
fn step_entry(garc: Option<&Garc>, species: usize, form: usize, step: isize) -> (usize, usize) {
    let ids = extras::species_ids();
    let place = ids.iter().position(|&n| n == species).unwrap_or(0);
    if step > 0 {
        if form + 1 < form_count_of(garc, species) {
            return (species, form + 1);
        }
        return ids.get(place + 1).map_or((species, form), |&next| (next, 0));
    }
    if form > 0 {
        return (species, form - 1);
    }
    match place.checked_sub(1).and_then(|p| ids.get(p)) {
        Some(&previous) => (previous, form_count_of(garc, previous) - 1),
        None => (species, form),
    }
}

/// Reads the list of the page: each species and each of its forms, with its data (the SD card first, then the romfs).
fn read_list_entries(garc: Option<&Garc>, session: &Session) -> Vec<ListEntry> {
    let changed = modified_species(garc, session);
    let mut entries = Vec::new();
    for species in extras::species_ids() {
        for form in 0..form_count_of(garc, species) {
            let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(species, form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
            let data = on_card.or_else(|| garc.and_then(|g| game_data(g, species, form))).or_else(|| extras::data(species, form));
            let row = match data {
                Some(d) => ListRow {
                    types: [d.type_1, d.type_2],
                    stats: [d.base_hp, d.base_attack, d.base_defense, d.base_special_attack, d.base_special_defense, d.base_speed],
                    total: d.base_total() as u16,
                    modified: false,
                },
                None => ListRow { types: [0, 0], stats: [0; 6], total: 0, modified: false },
            };
            entries.push(ListEntry { species, form, name: entry_name(species, form), row: row.with_modified(changed.contains(&species)) });
        }
    }
    entries
}

/// The places in `entries` that match a search: the species that [`match_species`] finds, with all their forms, then the forms whose
/// name has the search ("heat" finds Rotom-Heat, "mega" finds the Mega forms).
fn match_entries(entries: &[ListEntry], query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..entries.len()).collect();
    }
    let species = match_species(query);
    let mut found: Vec<usize> = Vec::new();
    for number in &species {
        found.extend(entries.iter().enumerate().filter(|(_, e)| e.species == *number).map(|(place, _)| place));
    }
    let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
    let query = clean(query.trim());
    found.extend(entries.iter().enumerate().filter(|(_, e)| e.form > 0 && !species.contains(&e.species) && clean(&e.name).contains(&query)).map(|(place, _)| place));
    found
}

/// Sorts the places of the list on one column. Equal lines stay in the order of the numbers and of the forms.
fn sort_entries(list: &mut [usize], entries: &[ListEntry], column: SortColumn, descending: bool) {
    let type_name = |t: u8| names::TYPE.get(t as usize).copied().unwrap_or("-");
    list.sort_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        let order = match column {
            SortColumn::Number => (x.species, x.form).cmp(&(y.species, y.form)),
            SortColumn::Name => x.name.cmp(&y.name),
            SortColumn::Type1 => type_name(x.row.types[0]).cmp(type_name(y.row.types[0])),
            SortColumn::Type2 => second_type(&x.row).cmp(second_type(&y.row)),
            SortColumn::Stat(place) => x.row.stats[place].cmp(&y.row.stats[place]),
            SortColumn::Total => x.row.total.cmp(&y.row.total),
        };
        let order = if descending { order.reverse() } else { order };
        order.then(a.cmp(&b))
    });
}

/// A column of the species list. A click on its header sorts the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SortColumn {
    #[default]
    Number,
    Name,
    Type1,
    Type2,
    /// The place of the stat: HP, Attack, Defense, Special Attack, Special Defense, Speed.
    Stat(usize),
    Total,
}

/// The width of the species list, in points.
const LIST_WIDTH: f32 = 624.0;

/// The height of one row of the species list, in points.
const LIST_ROW_HEIGHT: f32 = 34.0;

/// The columns of the species list: x position, width, name, and the sort of a click on the header.
const LIST_COLUMNS: [(f32, f32, &str, SortColumn); 11] = [
    (0.0, 96.0, "#", SortColumn::Number),
    (96.0, 144.0, "Name", SortColumn::Name),
    (240.0, 62.0, "Type 1", SortColumn::Type1),
    (302.0, 62.0, "Type 2", SortColumn::Type2),
    (364.0, 36.0, "HP", SortColumn::Stat(0)),
    (400.0, 36.0, "Atk", SortColumn::Stat(1)),
    (436.0, 36.0, "Def", SortColumn::Stat(2)),
    (472.0, 36.0, "SpA", SortColumn::Stat(3)),
    (508.0, 36.0, "SpD", SortColumn::Stat(4)),
    (544.0, 36.0, "Spe", SortColumn::Stat(5)),
    (580.0, 44.0, "BST", SortColumn::Total),
];

/// True for a column of stats: its text is on the right, and a first click sorts it
/// from the largest value to the smallest one.
fn stat_column(column: SortColumn) -> bool {
    matches!(column, SortColumn::Stat(_) | SortColumn::Total)
}

/// The name of the second type, or "-" when it is the first one again.
fn second_type(row: &ListRow) -> &'static str {
    if row.types[1] == row.types[0] {
        "-"
    } else {
        names::TYPE.get(row.types[1] as usize).copied().unwrap_or("-")
    }
}

/// The species that have a file on the SD card (data, moves or evolutions) that is not the same as the file of the game.
fn modified_species(garc: Option<&Garc>, session: &Session) -> std::collections::HashSet<usize> {
    let (Some(sd), Some(dump), Some(garc)) = (session.sd.as_ref(), session.dump.as_ref(), garc) else { return Default::default() };
    // A name like "25_0" is a species and a form: the file of the game is the place of that form in the archive.
    let form_file = |name: &str| -> Option<usize> {
        let (species, form) = name.split_once('_')?;
        Some(file_index(garc, species.parse().ok()?, form.parse().ok()?))
    };
    let mut found = std::collections::HashSet::new();
    let mut note = |names: Vec<String>| found.extend(names.iter().filter_map(|name| name.split('_').next()?.parse::<usize>().ok()));
    note(sango_core::modified::changed_files(sd, "species/data", &|name| garc.first_file(form_file(name)?).map(|bytes| bytes.to_vec())));
    // The files of the moves and of the evolutions are named by the place of the form in the archive: this finds the species of a place.
    let species_of_file: std::collections::HashMap<usize, usize> = (1..=SPECIES_COUNT).flat_map(|species| (0..csv::form_count(garc, species)).map(move |form| (file_index(garc, species, form), species))).collect();
    for (folder, archive) in [("species/level_up_moves", sango_core::learnset::LEVEL_UP_ARCHIVE), ("species/evolution", sango_core::evolution::ARCHIVE)] {
        if let Ok(other) = Garc::open(&dump.archive(archive)) {
            let names = sango_core::modified::changed_files(sd, folder, &|name| other.first_file(name.parse().ok()?).map(|bytes| bytes.to_vec()));
            note(names.iter().filter_map(|name| species_of_file.get(&name.parse::<usize>().ok()?).map(|species| species.to_string())).collect());
        }
    }
    if let Ok(eggs) = Garc::open(&dump.archive(sango_core::learnset::EGG_ARCHIVE)) {
        note(sango_core::modified::changed_files(sd, "species/egg_moves", &|name| eggs.first_file(name.parse().ok()?).map(|bytes| bytes.to_vec())));
    }
    if let Ok(mega) = Garc::open(&dump.archive(sango_core::mega_evolution::ARCHIVE)) {
        let names = sango_core::modified::changed_files(sd, "species/mega_evolution", &|name| mega.first_file(name.parse().ok()?).map(|bytes| bytes.to_vec()));
        note(names.iter().filter_map(|name| name.parse::<usize>().ok().map(|species| species.to_string())).collect());
    }
    found
}

/// The header of a species list: a click on a column name sorts the list on that column.
///
/// The width is `LIST_WIDTH`. A second click on the same column reverses the order.
fn list_header(ui: &mut egui::Ui, sort_column: &mut SortColumn, sort_descending: &mut bool) {
    let (header_rect, _) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, 20.0), egui::Sense::hover());
    for (x, width, label, column) in LIST_COLUMNS {
        let cell = egui::Rect::from_min_size(egui::pos2(header_rect.left() + x, header_rect.top()), egui::vec2(width, 20.0));
        let response = ui.interact(cell, ui.make_persistent_id(("species_list_header", label)), egui::Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let active = *sort_column == column;
        let (position, anchor) = if stat_column(column) {
            (egui::pos2(cell.right() - 4.0, cell.center().y), egui::Align2::RIGHT_CENTER)
        } else {
            (egui::pos2(cell.left(), cell.center().y), egui::Align2::LEFT_CENTER)
        };
        let ink = if active { color::ACCENT } else { color::MUTED };
        let text_rect = ui.painter().text(position, anchor, label, egui::FontId::proportional(12.0), ink);
        if active {
            // A small triangle drawn by hand: the font has no arrow glyph.
            let centre_x = if stat_column(column) { text_rect.left() - 8.0 } else { text_rect.right() + 8.0 };
            let centre_y = cell.center().y;
            let (w, h) = (3.5, 2.5);
            let points = if *sort_descending {
                vec![egui::pos2(centre_x - w, centre_y - h), egui::pos2(centre_x + w, centre_y - h), egui::pos2(centre_x, centre_y + h)]
            } else {
                vec![egui::pos2(centre_x - w, centre_y + h), egui::pos2(centre_x + w, centre_y + h), egui::pos2(centre_x, centre_y - h)]
            };
            ui.painter().add(egui::Shape::convex_polygon(points, color::ACCENT, egui::Stroke::NONE));
        }
        if response.clicked() {
            if active {
                *sort_descending = !*sort_descending;
            } else {
                *sort_column = column;
                *sort_descending = stat_column(column);
            }
        }
    }
}

/// Paints one row of a list with a name of its own (a species and a form, "Rotom-Heat").
fn paint_named_row(ui: &egui::Ui, rect: egui::Rect, number: usize, name: &str, row: Option<&ListRow>, icon: Option<&egui::TextureHandle>) {
    if let Some(texture) = icon {
        let target = egui::Rect::from_min_size(egui::pos2(rect.left() + 2.0, rect.center().y - 15.0), egui::vec2(40.0, 30.0));
        let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), target, full, egui::Color32::WHITE);
    }
    let y = rect.center().y;
    let left = rect.left();
    ui.painter().text(egui::pos2(left + 46.0, y), egui::Align2::LEFT_CENTER, format!("#{:04}", number), egui::FontId::proportional(12.0), color::MUTED);
    let modified = row.is_some_and(|row| row.modified);
    let name_rect = ui.painter().text(egui::pos2(left + 96.0, y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(13.5), if modified { color::ACCENT } else { color::TEXT });
    if modified {
        ui.painter().circle_filled(egui::pos2(name_rect.right() + 8.0, y), 3.0, color::ACCENT);
    }
    if let Some(row) = row {
        // The icons of the types; the text when the dump has no icons.
        if battle_icons::paint_type(ui.painter(), row.types[0], egui::pos2(left + 236.0, y)).is_none() {
            let type_1 = names::TYPE.get(row.types[0] as usize).copied().unwrap_or("-");
            ui.painter().text(egui::pos2(left + 240.0, y), egui::Align2::LEFT_CENTER, type_1, egui::FontId::proportional(12.5), color::TEXT);
        }
        let type_2 = second_type(row);
        if type_2 != "-" && battle_icons::paint_type(ui.painter(), row.types[1], egui::pos2(left + 292.0, y)).is_some() {
            // Drawn.
        } else {
            let ink = if type_2 == "-" { color::MUTED } else { color::TEXT };
            ui.painter().text(egui::pos2(left + 302.0, y), egui::Align2::LEFT_CENTER, type_2, egui::FontId::proportional(12.5), ink);
        }
        for (stat, &value) in row.stats.iter().enumerate() {
            let x = left + 364.0 + 36.0 * stat as f32;
            ui.painter().text(egui::pos2(x + 32.0, y), egui::Align2::RIGHT_CENTER, value.to_string(), egui::FontId::proportional(12.5), widget::stat_color(value as f32));
        }
        ui.painter().text(egui::pos2(left + 620.0, y), egui::Align2::RIGHT_CENTER, row.total.to_string(), egui::FontId::proportional(12.5), color::TEXT);
    }
}

/// Matches species by name (prefix or substring) or by number.
fn match_species(raw_query: &str) -> Vec<usize> {
    let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
    let query = clean(raw_query.trim());
    let num_query = raw_query.trim().trim_start_matches('#').trim_start_matches('0');

    if query.is_empty() {
        return extras::species_ids();
    }

    let mut starts_with = Vec::new();
    let mut contains = Vec::new();
    for number in extras::species_ids() {
        let name = clean(&species_name(number));
        let num_str = number.to_string();
        let formatted = format!("#{:04}", number);
        let is_num_match = !num_query.is_empty() && num_str == num_query;
        if name.starts_with(&query) || formatted.starts_with(&query) || is_num_match {
            starts_with.push(number);
        } else if name.contains(&query) || formatted.contains(&query) || (!num_query.is_empty() && num_str.contains(num_query)) {
            contains.push(number);
        }
    }
    starts_with.extend(contains);
    starts_with
}

impl Species {
    /// Reads the data from the romfs.
    fn read_romfs(&self) -> Result<SpeciesData, String> {
        let garc = self.garc.as_ref().ok_or("The archive is not open.")?;
        game_data(garc, self.species, self.form).ok_or_else(|| format!("The species {} form {} has no data.", self.species, self.form))
    }

    /// Reads the data of the chosen species and form: the SD card first, then the romfs.
    fn load(&mut self, session: &Session) {
        self.loaded = Some((self.species, self.form));
        let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(self.species, self.form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
        self.from_sd = on_card.is_some();
        self.load_shape(session);
        self.load_mega_table(session);
        let file = self.garc.as_ref().map_or(self.species, |garc| file_index(garc, self.species, self.form));
        self.learnset.load(session.sd.as_ref(), self.species, self.form, file);
        self.evolution.load(session.sd.as_ref(), self.species, self.form, file);
        // A species of a pack has its moves and its evolutions in the pack, until the user changes them.
        if self.form == 0 && self.species > SPECIES_COUNT {
            if let Some(record) = extras::pack_species(self.species) {
                if !self.learnset.from_card() {
                    self.learnset.load_pack(&record.moves);
                }
                if !self.evolution.from_card() {
                    self.evolution.load_pack(&record.evolutions);
                }
            }
        }
        self.game = self.read_romfs().ok();
        match on_card.map(Ok).unwrap_or_else(|| self.read_romfs()) {
            Ok(data) => {
                self.baseline = Some(data.clone());
                self.data = Some(data);
                self.status = None;
            }
            Err(message) => {
                self.data = None;
                self.baseline = None;
                self.status = Some((false, message));
            }
        }
    }

    /// Reads the Pokédex data from the romfs.
    fn read_pokedex_romfs(&self) -> Result<PokedexData, String> {
        let garc = self.pokedex_archive.as_ref().ok_or("The archive of the Pokédex data is not open.")?;
        let bytes = garc.first_file(pokedex_data::FILE).ok_or("The file of the Pokédex data is missing.")?;
        PokedexData::from_bytes(bytes.to_vec())
    }

    /// Reads the Pokédex data (the SD card first, then the romfs) and takes the shape of the chosen form.
    fn load_shape(&mut self, session: &Session) {
        let on_card = session.sd.as_ref().and_then(|sd| sd.read(pokedex_data::SD_PATH)).and_then(|b| PokedexData::from_bytes(b).ok());
        self.pokedex_from_sd = on_card.is_some();
        self.pokedex = on_card.or_else(|| self.read_pokedex_romfs().ok());
        self.shape = self.pokedex.as_ref().and_then(|e| e.shape(self.species, self.form)).unwrap_or(0);
        self.shape_baseline = self.shape;
    }

    /// Reads the Mega Evolution table of the open species: the SD card first, then the romfs.
    fn load_mega_table(&mut self, session: &Session) {
        let on_card = session.sd.as_ref()
            .and_then(|sd| sd.read(&sango_core::mega_evolution::sd_path(self.species)))
            .and_then(|b| sango_core::mega_evolution::MegaEvolutionTable::from_bytes(&b).ok());
        self.mega_from_sd = on_card.is_some();
        self.mega_table = on_card.or_else(|| {
            self.mega_garc.as_ref()
                .and_then(|g| g.first_file(self.species))
                .and_then(|b| sango_core::mega_evolution::MegaEvolutionTable::from_bytes(b).ok())
        });
    }

    /// Reads the list of the page: each species and each of its forms.
    fn build_list_rows(&self, session: &Session) -> Vec<ListEntry> {
        read_list_entries(self.garc.as_ref(), session)
    }

    /// The number of forms of the chosen species (at least one).
    fn total_form_count(&self) -> usize {
        form_count_of(self.garc.as_ref(), self.species)
    }

    /// The menus, the two buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut reset_more = false;
        let mut save = false;
        let mut add_form = false;
        let mut delete_open = false;
        let before = (self.species, self.form);
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let species_text = format!("#{:04} {}", self.species, entry_name(self.species, self.form));
                let species_id = ui.make_persistent_id("species_menu");
                let popup_id = species_id.with("popup");
                let is_popup_open = ui.memory(|m| m.is_popup_open(popup_id));

                let button_width = 220.0;
                let button_height = 36.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(button_width, button_height), egui::Sense::hover());
                let button_response = ui.interact(rect, species_id, egui::Sense::click());

                if ui.is_rect_visible(rect) {
                    let visuals = if is_popup_open {
                        &ui.visuals().widgets.open
                    } else {
                        ui.style().interact(&button_response)
                    };
                    ui.painter().rect(
                        rect.expand(visuals.expansion),
                        visuals.rounding,
                        visuals.weak_bg_fill,
                        visuals.bg_stroke,
                    );
                    // The icon of the chosen species, before its name.
                    let mut icon_width = 0.0;
                    let (species, form) = (self.species, self.form);
                    if let Some(texture) = self.form_icon_texture(ui.ctx(), session, species, form) {
                        let icon = egui::Rect::from_min_size(egui::pos2(rect.left() + 6.0, rect.center().y - 15.0), egui::vec2(40.0, 30.0));
                        let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter().image(texture.id(), icon, full, egui::Color32::WHITE);
                        icon_width = 44.0;
                    }
                    let galley = egui::WidgetText::from(&species_text).into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        button_width - 28.0 - icon_width,
                        egui::TextStyle::Button,
                    );
                    let text_rect = rect.shrink2(ui.spacing().button_padding);
                    let text_pos = egui::Align2::LEFT_CENTER.align_size_within_rect(galley.size(), text_rect).min + egui::vec2(icon_width, 0.0);
                    ui.painter().galley(text_pos, galley, visuals.text_color());

                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.right() - 14.0, rect.center().y),
                        egui::vec2(8.0, 5.0),
                    );
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![icon_rect.left_top(), icon_rect.right_top(), icon_rect.center_bottom()],
                        visuals.fg_stroke.color,
                        egui::Stroke::NONE,
                    ));
                }

                if button_response.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                    widget::set_active_id(ui, species_id);
                }

                widget::register_focusable(ui, species_id, button_response.rect);
                if ui.memory(|m| m.is_popup_open(popup_id)) {
                    if self.list_rows.is_none() {
                        let rows = self.build_list_rows(session);
                        self.list_rows = Some(rows);
                    }
                } else {
                    // The list is read again at each opening: a reset, an import or an undo may have changed the files.
                    self.list_rows = None;
                }
                let popup_res = egui::popup::popup_below_widget(
                    ui,
                    popup_id,
                    &button_response,
                    egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_width(LIST_WIDTH);
                        ui.set_max_width(LIST_WIDTH);
                        ui.spacing_mut().item_spacing.y = 4.0;

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(widget::hint("🔍"));
                            let search_input = ui.add(
                                egui::TextEdit::singleline(&mut self.species_search)
                                    .hint_text("Search name or number...")
                                    .desired_width(185.0)
                                    .clip_text(true),
                            );
                            if !self.search_focused {
                                search_input.request_focus();
                                self.search_focused = true;
                            }
                            if !self.species_search.is_empty() && ui.small_button("✕").clicked() {
                                self.species_search.clear();
                            }
                        });
                        ui.separator();

                        // The list leaves the page while it is drawn: the icons of the forms need the page.
                        let entries = self.list_rows.take().unwrap_or_default();
                        let query = self.species_search.trim().to_string();
                        let mut matches = match_entries(&entries, &query);

                        // The header: a click sorts the list on that column.
                        list_header(ui, &mut self.sort_column, &mut self.sort_descending);
                        ui.separator();
                        sort_entries(&mut matches, &entries, self.sort_column, self.sort_descending);

                        if matches.is_empty() {
                            ui.add_space(4.0);
                            ui.label(widget::hint("No matching Pokémon."));
                            ui.add_space(4.0);
                        } else {
                            let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                            let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let key_of = |place: usize| (entries[place].species, entries[place].form);
                            let current = matches.iter().position(|&place| key_of(place) == (self.species, self.form));

                            if let Some(pos) = current {
                                if down && pos + 1 < matches.len() {
                                    (self.species, self.form) = key_of(matches[pos + 1]);
                                }
                                if up && pos > 0 {
                                    (self.species, self.form) = key_of(matches[pos - 1]);
                                }
                            } else if up || down {
                                (self.species, self.form) = key_of(matches[0]);
                            }

                            if enter {
                                if !matches.iter().any(|&place| key_of(place) == (self.species, self.form)) {
                                    (self.species, self.form) = key_of(matches[0]);
                                }
                                ui.memory_mut(|m| m.close_popup());
                            }

                            let nav_key = up || down;
                            let highlighted_place = matches.iter().position(|&place| key_of(place) == (self.species, self.form)).unwrap_or(0);
                            // The list shows the highlighted line at the middle after a key, and at the opening.
                            let centre = Some(highlighted_place).filter(|_| nav_key || (query.is_empty() && (self.scroll_list || !self.list_shown)));

                            ui.spacing_mut().item_spacing.y = 0.0;
                            let mut area = egui::ScrollArea::vertical().max_height(360.0);
                            if let Some(place) = centre {
                                area = area.vertical_scroll_offset(((place as f32 + 0.5) * LIST_ROW_HEIGHT - 180.0).max(0.0));
                            }
                            area.show_rows(ui, LIST_ROW_HEIGHT, matches.len(), |ui, range| {
                                for row_place in range {
                                    let entry = &entries[matches[row_place]];
                                    let (rect, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, LIST_ROW_HEIGHT), egui::Sense::click());
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }
                                    if row_place == highlighted_place {
                                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::ACCENT_SOFT);
                                        widget::glow(ui, rect, 6.0);
                                    } else if response.hovered() {
                                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                                    }
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    let icon = self.form_icon_texture(ui.ctx(), session, entry.species, entry.form);
                                    paint_named_row(ui, rect, entry.species, &entry.name, Some(&entry.row), icon);
                                    if response.clicked() {
                                        (self.species, self.form) = (entry.species, entry.form);
                                        ui.memory_mut(|m| m.close_popup());
                                    }
                                }
                            });
                        }
                        self.list_rows = Some(entries);
                        self.list_shown = true;
                        self.scroll_list = false;
                    },
                );

                if popup_res.is_none() {
                    self.list_shown = false;
                    self.search_focused = false;
                    if !self.species_search.is_empty() {
                        self.species_search.clear();
                    }
                }

                if widget::active_id(ui).is_none() {
                    widget::set_active_id(ui, species_id);
                }
                if widget::is_active(ui, species_id) && widget::page_has_focus(ui) && popup_res.is_none() {
                    widget::glow(ui, button_response.rect, 6.0);
                    widget::scroll_active_into_view(ui, &button_response);
                    let open_key = ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                    });
                    if open_key {
                        ui.memory_mut(|m| m.open_popup(popup_id));
                    }
                    ui.input_mut(|i| {
                        let step = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize
                            - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize;
                        if step != 0 {
                            // The next form, or the next species: the numbers between the species of the game and of the packs are skipped.
                            (self.species, self.form) = step_entry(self.garc.as_ref(), self.species, self.form, step);
                        }
                    });
                }
                let forms = self.total_form_count();
                self.form = self.form.min(forms.saturating_sub(1));
                let has_card = session.sd.is_some() && self.garc.is_some();
                ui.add_enabled_ui(has_card && self.data.is_some(), |ui| {
                    add_form = widget::button_styled(ui, egui::RichText::new("+ New species").color(color::WHITE), Some(color::ACCENT));
                })
                .response
                .on_hover_text("Makes a new species-form (for example \"Rotom-Ice\"), with its own data, icon and model. It starts as a copy of the open one or as a template.");
                // Only a species-form that was added (by a pack or by "+ New species") can be deleted.
                let added = (self.form > 0 && extras::pack_forms(self.species).iter().any(|f| f.form as usize == self.form)) || (self.form == 0 && self.species > SPECIES_COUNT);
                ui.add_enabled_ui(session.sd.is_some() && added, |ui| {
                    if widget::confirm_button(ui, "Delete", "Click again to delete") {
                        delete_open = true;
                    }
                })
                .response
                .on_hover_text(if added { "Deletes this added species-form: its data, and its place in the lists." } else { "Only an added species-form can be deleted, not one of the game." });
                ui.add_enabled_ui(self.garc.is_some(), |ui| {
                    if widget::button(ui, "Export…") {
                        self.tools.export_open = true;
                    }
                })
                .response
                .on_hover_text("Writes CSV files of all the species: choose the parts in the window.");
                ui.add_enabled_ui(has_card, |ui| {
                    if widget::button(ui, "Import…") {
                        self.tools.import_open = true;
                    }
                })
                .response
                .on_hover_text("Reads CSV files of all the species: choose the parts in the window.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Puts parts of one form, one species or all the species back to the romfs: choose in the window.");
                save = self.data.is_some() && widget::save_button(ui, self.unsaved());
                if self.data.is_some() && (self.data != self.baseline || self.shape != self.shape_baseline || self.learnset.changed() || self.evolution.changed() || self.text.changed() || self.icon_tab.changed() || self.cry_tab.changed()) {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd || self.pokedex_from_sd || self.mega_from_sd || self.learnset.from_card() || self.evolution.from_card() || self.text.from_card() || self.icon_tab.from_card() || self.cry_tab.from_card() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else if self.data.is_some() {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
        if (self.species, self.form) != before {
            if self.loses_changes() {
                // The changes of the open Pokémon would be lost: the window asks first.
                self.pending_open = Some((self.species, self.form));
                (self.species, self.form) = before;
            } else {
                self.loaded = None;
            }
        }
        if delete_open {
            if self.form == 0 && self.species > SPECIES_COUNT {
                self.delete_species(session);
            } else {
                let form = self.form as u16;
                self.delete_form(session, form);
            }
        }
        if add_form {
            self.new_pokemon = Some(NewPokemon { own: false, species: self.species, name: String::new(), copy: true });
        }
        if reset_more {
            self.tools.reset_open = true;
            self.tools.parts = [false; 7];
            if let Some(part) = data_tools::tab_part(self.tab) {
                self.tools.parts[data_tools::Part::ALL.iter().position(|p| *p == part).unwrap_or(0)] = true;
            }
        }
        if save {
            self.save(session);
        }
    }

    /// True when opening another species or form would lose changes: the data, the shape, the moves and the evolutions belong to the
    /// open one. (The texts, the icons and the cries keep their changes for every species.)
    fn loses_changes(&self) -> bool {
        self.data.is_some() && (self.data != self.baseline || self.shape != self.shape_baseline || self.learnset.changed() || self.evolution.changed())
    }

    /// The window of the button "+ New species": the species, the name of the form, and what the new species-form starts with.
    fn new_pokemon_window(&mut self, ctx: &egui::Context, session: &Session) {
        let Some(mut state) = self.new_pokemon.take() else { return };
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        let open_name = format!("#{:04} {}", self.species, entry_name(self.species, self.form));
        egui::Window::new("New species").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.label(egui::RichText::new("What is it?").strong());
            ui.radio_value(&mut state.own, false, "A new form of a species that exists (Rotom-Ice, a regional form...)");
            ui.radio_value(&mut state.own, true, "A species of its own, linked to no other (a creature of another game, a fakemon...)");
            ui.add_space(6.0);
            egui::Grid::new("new_pokemon").num_columns(2).spacing(egui::vec2(10.0, 8.0)).show(ui, |ui| {
                if state.own {
                    ui.label("Name of the species");
                    ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("Jibanyan").desired_width(260.0));
                    ui.end_row();
                } else {
                    ui.label("Species");
                    // A form belongs to a species: the menu offers the species (their first form).
                    let mut chosen = (state.species, 0);
                    if menus::pokemon_menu(ui, "new_pokemon_species", &mut chosen, 300.0, Some(&|_, form| form == 0), None) {
                        state.species = chosen.0;
                    }
                    ui.end_row();
                    ui.label("Name of the form");
                    ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("Ice").desired_width(260.0));
                    ui.end_row();
                }
                ui.label("Starts with");
                ui.vertical(|ui| {
                    ui.radio_value(&mut state.copy, true, format!("A copy of {open_name}"));
                    ui.radio_value(&mut state.copy, false, "A template: stats of 50, Normal type, no held item");
                });
                ui.end_row();
            });
            ui.add_space(6.0);
            if state.own {
                let shown = if state.name.trim().is_empty() { "(name)".to_string() } else { state.name.trim().to_string() };
                ui.label(widget::hint(&format!("In the lists: {shown}, with the next free number (724 and more). It has no evolution and no move at first. It shows the model of {} until you import its own (Model tab); give it its icon in the General tab.", species_name(self.species))));
            } else {
                let shown_name = if state.name.trim().is_empty() { "Form".to_string() } else { state.name.trim().replace(' ', "-") };
                ui.label(widget::hint(&format!("In the lists: {}-{shown_name}. Then give it its icon (General tab) and its model (Model tab).", species_name(state.species))));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let named = !state.own || !state.name.trim().is_empty();
                ui.add_enabled_ui(session.sd.is_some() && self.data.is_some() && named, |ui| {
                    create = widget::button_styled(ui, egui::RichText::new("Create").color(color::WHITE), Some(color::ACCENT));
                });
                cancel = widget::button(ui, "Cancel");
            });
        });
        if create {
            self.create_pokemon(session, &state);
        } else if open && !cancel {
            self.new_pokemon = Some(state);
        }
    }

    /// Makes the new species-form of the window: a form of its species in the pack of the user, open at once.
    fn create_pokemon(&mut self, session: &Session, state: &NewPokemon) {
        let (Some(sd), Some(open)) = (session.sd.as_ref(), self.data.as_ref()) else { return };
        let mut data = if state.copy { open.clone() } else { template_data(open) };
        if state.own {
            // A species of its own has one form, and no link to the forms of the open species.
            data.total_form_count = 1;
            data.alternate_form_file = 0;
            data.alternate_form_model = 0;
            let native_model = if self.species <= SPECIES_COUNT { self.species as u16 } else { 1 };
            let result = sango_pack::Library::open(sd).and_then(|mut library| {
                let number = library.add_species(sd, &state.name, &data.to_bytes(), native_model)?;
                library.save(sd)?;
                Ok(number)
            });
            match result {
                Ok(number) => {
                    extras::invalidate();
                    extras::refresh(Some(sd));
                    self.species = number as usize;
                    self.form = 0;
                    self.loaded = None;
                    self.list_rows = None;
                    self.status = Some((true, format!("#{number:04} {} is made. Change its data with the tabs and press Save. Restart the game to see it.", state.name.trim())));
                }
                Err(message) => self.status = Some((false, message)),
            }
            return;
        }
        let species = state.species;
        let reference = if species <= SPECIES_COUNT { sango_pack::Ref::Game(species as u16) } else { sango_pack::Ref::key(&species_name(species)) };
        let form = form_count_of(self.garc.as_ref(), species);
        let result = u16::try_from(form).map_err(|_| "The species has too many forms.".to_string()).and_then(|number| {
            let mut library = sango_pack::Library::open(sd)?;
            library.add_form(sd, reference, number, &state.name, &data.to_bytes(), sango_pack::table::MegaCondition::Never)?;
            library.save(sd)
        });
        match result {
            Ok(()) => {
                extras::invalidate();
                extras::refresh(Some(sd));
                self.species = species;
                self.form = form;
                self.loaded = None;
                self.list_rows = None;
                self.status = Some((true, format!("{} is made. Change its data with the tabs and press Save. Restart the game to see it.", entry_name(species, form))));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Deletes the open species when the user made it with "+ New species", with its forms.
    fn delete_species(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let name = species_name(self.species);
        let result = sango_pack::Library::open(sd).and_then(|mut library| {
            library.remove_species(sd, &name)?;
            library.save(sd)
        });
        match result {
            Ok(()) => {
                for form in 0..self.total_form_count() {
                    let _ = sd.remove(&sd_path(self.species, form));
                }
                extras::invalidate();
                extras::refresh(Some(sd));
                self.species = 1;
                self.form = 0;
                self.loaded = None;
                self.list_rows = None;
                self.status = Some((true, format!("{name} was deleted. Restart the game to see it.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    pub(crate) fn delete_form(&mut self, session: &Session, form: u16) {
        let Some(sd) = session.sd.as_ref() else { return };
        let species = self.pack_ref();
        let result = sango_pack::Library::open(sd).and_then(|mut library| {
            library.remove_form(sd, &species, form)?;
            library.save(sd)
        });
        match result {
            Ok(()) => {
                let _ = sd.remove(&sd_path(self.species, form as usize));
                extras::invalidate();
                extras::refresh(Some(sd));
                let name = entry_name(self.species, form as usize);
                if self.form == form as usize {
                    self.form = 0;
                    self.loaded = None;
                }
                self.list_rows = None;
                self.status = Some((true, format!("{name} was deleted. Restart the game to see it.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Changes an entry in the game mega evolution table for the open species and writes it to SD card.
    pub(crate) fn set_game_mega(&mut self, session: &Session, entry_index: usize, entry: sango_core::mega_evolution::MegaEntry) {
        let Some(sd) = session.sd.as_ref() else { return };
        let mut table = self.mega_table.clone().unwrap_or_else(sango_core::mega_evolution::MegaEvolutionTable::empty);
        if entry_index < table.entries.len() {
            table.entries[entry_index] = entry;
        }
        let bytes = table.to_bytes();
        let path = sango_core::mega_evolution::sd_path(self.species);
        let result = sd.write(&path, &bytes).and_then(|_| sd.mark_changed(&path));
        match result {
            Ok(()) => {
                self.mega_from_sd = true;
                self.mega_table = Some(table);
                self.status = Some((true, format!("The Mega Evolution of {} was saved. Restart the game to see it.", species_name(self.species))));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The texture of the icon of a species and a form: decoded at the first display, then kept.
    pub(crate) fn form_icon_texture<'a>(
        &'a mut self,
        ctx: &egui::Context,
        session: &sango_core::Session,
        species: usize,
        form: usize,
    ) -> Option<&'a egui::TextureHandle> {
        form_icon(&mut self.form_icon_textures, self.icon_garc.as_ref(), session.sd.as_ref(), ctx, species, form)
    }

    /// Writes the file on the SD card and tells the plugin.
    fn save(&mut self, session: &Session) {
        let (Some(data), Some(sd)) = (&self.data, &session.sd) else { return };
        let path = sd_path(self.species, self.form);
        let mut result = sd.write(&path, &data.to_bytes()).and_then(|_| sd.mark_changed(&path));
        // The shape is in the Pokédex data: the file is written again when the shape changed.
        let mut shape_saved = false;
        if result.is_ok() && self.shape != self.shape_baseline {
            let placed = self.pokedex.as_mut().is_some_and(|pokedex| pokedex.set_shape(self.species, self.form, self.shape));
            match self.pokedex.as_ref() {
                Some(pokedex) if placed => {
                    result = sd.write(pokedex_data::SD_PATH, &pokedex.to_bytes()).and_then(|_| sd.mark_changed(pokedex_data::SD_PATH));
                    shape_saved = result.is_ok();
                }
                _ => result = Err("The shape has no place in the Pokédex data.".into()),
            }
        }
        // The moves of the Learnset tab have their own files.
        if result.is_ok() {
            if let Err(message) = self.learnset.save(sd, self.species, self.form) {
                result = Err(message);
            } else if let Err(message) = self.evolution.save(sd, self.species, self.form) {
                result = Err(message);
            } else if let Err(message) = self.text.save(sd) {
                result = Err(message);
            } else if let Err(message) = self.icon_tab.save(sd) {
                result = Err(message);
            } else if let Err(message) = self.cry_tab.save(sd) {
                result = Err(message);
            }
        }
        match result {
            Ok(()) => {
                if shape_saved {
                    self.shape_baseline = self.shape;
                    self.pokedex_from_sd = true;
                }
                self.baseline = self.data.clone();
                self.from_sd = true;
                // The stats or the types may have changed: the species list reads them again.
                self.list_rows = None;
                self.status = Some((true, format!("Saved in {path}. The game loads it again by itself.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }
}

/// A rate from 0 to 255 as a percentage of the maximum.
#[cfg(test)]
fn percent(value: u8) -> String {
    let text = format!("{:.1}", value as f32 * 100.0 / 255.0);
    format!("{} %", text.strip_suffix(".0").unwrap_or(&text))
}

/// What a gender number means: 0 is only male, 254 only female, 255 no gender, and in between
/// the chance to be female is `(number + 1) / 256`.
#[cfg(test)]
fn gender_text(value: u8) -> String {
    match value {
        0 => "male only".to_string(),
        254 => "female only".to_string(),
        255 => "no gender".to_string(),
        _ => {
            let female = (value as f32 + 1.0) * 100.0 / 256.0;
            format!("{} % female, {} % male", trim(female), trim(100.0 - female))
        }
    }
}

/// A number with one digit, without a useless ".0".
fn trim(value: f32) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

const MALE_BLUE: egui::Color32 = egui::Color32::from_rgb(59, 130, 246);
const FEMALE_PINK: egui::Color32 = egui::Color32::from_rgb(236, 72, 153);

#[cfg(test)]
fn height_text(cm: u16) -> String {
    let m = cm as f32 / 100.0;
    let total_inches = cm as f32 / 2.54;
    let mut feet = (total_inches / 12.0).floor() as i32;
    let mut inches = (total_inches % 12.0).round() as i32;
    if inches == 12 {
        feet += 1;
        inches = 0;
    }
    format!("{m:.2} m / {cm} cm ({feet}'{inches:02}\")")
}

#[cfg(test)]
fn weight_text(hectograms: u16) -> String {
    let kg = hectograms as f32 / 10.0;
    let lbs = kg * 2.2046226;
    format!("{kg:.1} kg ({lbs:.1} lbs)")
}

fn gender_widget(ui: &mut egui::Ui, value: &mut u8) {
    ui.spacing_mut().item_spacing.x = 6.0;

    let bar_width = 110.0;
    let bar_height = 12.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(bar_width, 24.0), egui::Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(bar_width, bar_height));
    let track_rounding = egui::Rounding::same(bar_height / 2.0);

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
            let snap_values: [(f32, u8); 7] = [
                (0.0, 254),           // 100% ♀, 0% ♂
                (32.0 / 256.0, 223),  // 87.5% ♀, 12.5% ♂
                (64.0 / 256.0, 191),  // 75% ♀, 25% ♂
                (128.0 / 256.0, 127), // 50% ♀, 50% ♂
                (192.0 / 256.0, 63),  // 25% ♀, 75% ♂
                (224.0 / 256.0, 31),  // 12.5% ♀, 87.5% ♂
                (1.0, 0),             // 0% ♀, 100% ♂
            ];
            let mut snapped = None;
            for (snap_r, snap_val) in snap_values {
                if (ratio - snap_r).abs() < 0.04 {
                    snapped = Some(snap_val);
                    break;
                }
            }
            *value = snapped.unwrap_or_else(|| {
                let female_ratio = 1.0 - ratio;
                (female_ratio * 256.0 - 1.0).round().clamp(0.0, 254.0) as u8
            });
        }
    }

    ui.painter().rect_filled(track_rect, track_rounding, color::SURFACE_STRONG);

    let (male_pct, female_pct) = match *value {
        0 => (100.0, 0.0),
        254 => (0.0, 100.0),
        255 => (0.0, 0.0),
        _ => {
            let f = (*value as f32 + 1.0) * 100.0 / 256.0;
            (100.0 - f, f)
        }
    };

    if *value == 255 {
        ui.painter().rect_filled(track_rect, track_rounding, color::MUTED.linear_multiply(0.35));
    } else {
        let male_w = (male_pct / 100.0 * track_rect.width()).round();
        let female_w = track_rect.width() - male_w;

        if male_w > 0.0 {
            let male_rect = egui::Rect::from_min_size(track_rect.min, egui::vec2(male_w, track_rect.height()));
            let r = if female_w <= 0.0 {
                track_rounding
            } else {
                egui::Rounding { nw: bar_height / 2.0, sw: bar_height / 2.0, ne: 0.0, se: 0.0 }
            };
            ui.painter().rect_filled(male_rect, r, MALE_BLUE);
        }

        if female_w > 0.0 {
            let female_rect = egui::Rect::from_min_size(
                egui::pos2(track_rect.min.x + male_w, track_rect.min.y),
                egui::vec2(female_w, track_rect.height()),
            );
            let r = if male_w <= 0.0 {
                track_rounding
            } else {
                egui::Rounding { nw: 0.0, sw: 0.0, ne: bar_height / 2.0, se: bar_height / 2.0 }
            };
            ui.painter().rect_filled(female_rect, r, FEMALE_PINK);
        }

        let handle_x = (track_rect.left() + male_w).clamp(track_rect.left(), track_rect.right());
        let handle_center = egui::pos2(handle_x, track_rect.center().y);
        let radius = 6.0;

        if response.hovered() || response.dragged() {
            ui.painter().circle_filled(handle_center, radius + 3.0, egui::Color32::from_black_alpha(35));
        }
        ui.painter().circle_filled(handle_center + egui::vec2(0.0, 1.0), radius + 0.5, egui::Color32::from_black_alpha(25));
        ui.painter().circle_filled(handle_center, radius, color::WHITE);
        let border_color = if male_pct > 50.0 {
            MALE_BLUE
        } else if female_pct > 50.0 {
            FEMALE_PINK
        } else {
            color::ACCENT
        };
        ui.painter().circle_stroke(handle_center, radius, egui::Stroke::new(2.0_f32, border_color));
    }

    if *value == 255 {
        ui.label(egui::RichText::new("Genderless").color(color::MUTED).strong());
        if ui.button("Binary (50/50)").clicked() {
            *value = 127;
        }
    } else {
        ui.label(egui::RichText::new(format!("♂ {}%", trim(male_pct))).color(MALE_BLUE).strong().size(12.5));
        ui.label(egui::RichText::new(format!("♀ {}%", trim(female_pct))).color(FEMALE_PINK).strong().size(12.5));

    }

}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HeightUnit {
    #[default]
    Meters,
    Centimeters,
    Inches,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum WeightUnit {
    #[default]
    Kg,
    Lbs,
}

fn height_field(ui: &mut egui::Ui, id_salt: &str, cm_value: &mut u16) {
    let unit_id = ui.make_persistent_id(id_salt).with("unit");
    let mut unit: HeightUnit = ui.data_mut(|d| d.get_temp(unit_id).unwrap_or_default());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match unit {
            HeightUnit::Meters => {
                let mut val_m = *cm_value as f64 / 100.0;
                let r = ui.add(
                    egui::DragValue::new(&mut val_m)
                        .speed(0.01)
                        .range(0.0..=655.35)
                        .fixed_decimals(2),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = (val_m * 100.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
            HeightUnit::Centimeters => {
                let mut val_cm = *cm_value as f64;
                let r = ui.add(
                    egui::DragValue::new(&mut val_cm)
                        .speed(1.0)
                        .range(0.0..=65535.0)
                        .fixed_decimals(0),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = val_cm.round().clamp(0.0, 65535.0) as u16;
                }
            }
            HeightUnit::Inches => {
                let mut val_in = *cm_value as f64 / 2.54;
                let r = ui.add(
                    egui::DragValue::new(&mut val_in)
                        .speed(0.5)
                        .range(0.0..=25800.0)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = (val_in * 2.54).round().clamp(0.0, 65535.0) as u16;
                }
            }
        }

        let unit_label = match unit {
            HeightUnit::Meters => "m",
            HeightUnit::Centimeters => "cm",
            HeightUnit::Inches => "in",
        };
        let mut new_unit = unit;
        egui::ComboBox::from_id_salt(unit_id)
            .selected_text(unit_label)
            .width(42.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_unit, HeightUnit::Meters, "m");
                ui.selectable_value(&mut new_unit, HeightUnit::Centimeters, "cm");
                ui.selectable_value(&mut new_unit, HeightUnit::Inches, "in");
            });
        if new_unit != unit {
            unit = new_unit;
            ui.data_mut(|d| d.insert_temp(unit_id, unit));
        }

    });
}

fn weight_field(ui: &mut egui::Ui, id_salt: &str, hg_value: &mut u16) {
    let unit_id = ui.make_persistent_id(id_salt).with("unit");
    let mut unit: WeightUnit = ui.data_mut(|d| d.get_temp(unit_id).unwrap_or_default());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match unit {
            WeightUnit::Kg => {
                let mut val_kg = *hg_value as f64 / 10.0;
                let r = ui.add(
                    egui::DragValue::new(&mut val_kg)
                        .speed(0.1)
                        .range(0.0..=6553.5)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *hg_value = (val_kg * 10.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
            WeightUnit::Lbs => {
                let mut val_lbs = (*hg_value as f64 / 10.0) * 2.2046226;
                let r = ui.add(
                    egui::DragValue::new(&mut val_lbs)
                        .speed(0.5)
                        .range(0.0..=14448.0)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *hg_value = (val_lbs / 2.2046226 * 10.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
        }

        let unit_label = match unit {
            WeightUnit::Kg => "kg",
            WeightUnit::Lbs => "lbs",
        };
        let mut new_unit = unit;
        egui::ComboBox::from_id_salt(unit_id)
            .selected_text(unit_label)
            .width(46.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_unit, WeightUnit::Kg, "kg");
                ui.selectable_value(&mut new_unit, WeightUnit::Lbs, "lbs");
            });
        if new_unit != unit {
            unit = new_unit;
            ui.data_mut(|d| d.insert_temp(unit_id, unit));
        }

    });
}

/// A number field with arrow buttons: the up and down keys work too.
fn number<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, max: f64) {
    widget::spin(ui, value, 0.0, max);
}

/// The help of a field: a short note, the Bulbapedia page, and the field and value for the examples.
struct FieldNote {
    summary: &'static str,
    url: &'static str,
    /// The field and its current value: the help lists the species that have the same value.
    example: Option<(Field, u32)>,
}

/// The key of the list of items in the memory of the interface.
const ITEM_LIST_ID: &str = "species_item_list";

/// The key of the names of the abilities in the memory of the interface.
const ABILITY_LIST_ID: &str = "species_ability_list";

/// The key of the names of the leveling rates in the memory of the interface.
const LEVELING_RATE_LIST_ID: &str = "species_leveling_rate_list";

/// The names of the leveling rates: the rates of the game, then the custom rates of the SD card.
fn leveling_rate_names(ctx: &egui::Context, sd: Option<&sango_core::SdCard>) -> std::sync::Arc<Vec<String>> {
    let id = egui::Id::new(LEVELING_RATE_LIST_ID);
    let cached: Option<(std::time::Instant, std::sync::Arc<Vec<String>>)> = ctx.data(|d| d.get_temp(id));
    if let Some((when, names)) = cached {
        if when.elapsed() < std::time::Duration::from_secs(1) {
            return names;
        }
    }
    // The two rates that no Pokémon uses have no name: the menu does not offer them.
    let mut names: Vec<String> = names::LEVELING_RATE.iter().enumerate().map(|(rate, &s)| if names::is_unused_rate(rate) { String::new() } else { s.to_string() }).collect();
    if let Some(sd) = sd {
        let mut max_rate = names.len();
        for path in sd.files_in("leveling_rate") {
            let Some(file_name) = path.strip_prefix("leveling_rate/").and_then(|p| p.strip_suffix(".bin")) else { continue };
            let Ok(rate) = file_name.parse::<usize>() else { continue };
            if rate >= sango_core::experience::MAX_RATES {
                continue;
            }
            if rate + 1 > max_rate {
                max_rate = rate + 1;
            }
        }
        for rate in names.len()..max_rate {
            names.push(format!("Rate {rate}"));
        }
    }
    let names = std::sync::Arc::new(names);
    ctx.data_mut(|d| d.insert_temp(id, (std::time::Instant::now(), names.clone())));
    names
}

/// The names of the abilities: the abilities of the game, then the new ones of the SD card. The card is read again once each
/// second at most, so a new ability shows in the lists soon after it is saved.
fn ability_names(ctx: &egui::Context, sd: Option<&sango_core::SdCard>) -> std::sync::Arc<Vec<String>> {
    let id = egui::Id::new(ABILITY_LIST_ID);
    let cached: Option<(std::time::Instant, std::sync::Arc<Vec<String>>)> = ctx.data(|d| d.get_temp(id));
    if let Some((when, names)) = cached {
        if when.elapsed() < std::time::Duration::from_secs(1) {
            return names;
        }
    }
    let names = std::sync::Arc::new(sango_core::custom_ability::list(&names::ABILITY, sd));
    ctx.data_mut(|d| d.insert_temp(id, (std::time::Instant::now(), names.clone())));
    names
}

/// A drop-down list of abilities, with the new abilities of the user after the ones of the game.
fn ability_picker(ui: &mut egui::Ui, id_salt: &str, value: &mut u8) {
    let shared: Option<std::sync::Arc<Vec<String>>> = ui.ctx().data(|d| d.get_temp::<(std::time::Instant, std::sync::Arc<Vec<String>>)>(egui::Id::new(ABILITY_LIST_ID)).map(|(_, names)| names));
    match shared {
        Some(list) => {
            let names: Vec<&str> = list.iter().map(String::as_str).collect();
            searchable_picker(ui, id_salt, value, &names, 180.0);
        }
        None => searchable_picker(ui, id_salt, value, &names::ABILITY, 180.0),
    }
}

/// A drop-down list of items, with their pictures (see [`menus::item_menu`]).
pub(crate) fn item_picker(ui: &mut egui::Ui, id_salt: &str, value: &mut u16) {
    menus::item_menu(ui, id_salt, value, 240.0, true);
}

/// The key of the examples in the memory of the interface.
const EXAMPLES_ID: &str = "species_examples";

/// The examples of the help of a field, as lines of text. Empty when the data of the game is not open.
fn example_lines(ui: &egui::Ui, field: Field, value: u32) -> Vec<String> {
    let shared: Option<(Arc<examples::Index>, usize)> = ui.ctx().data(|d| d.get_temp(egui::Id::new(EXAMPLES_ID)));
    let Some((index, current)) = shared else { return Vec::new() };
    let found = index.examples(field, value, current);
    let mut lines = Vec::new();
    if !found.species.is_empty() {
        let list = found.species.iter().map(|&n| Examples::name(n)).collect::<Vec<_>>().join(", ");
        let more = if found.more > 0 { format!(" and {} more", found.more) } else { String::new() };
        lines.push(format!("Same value: {list}{more}."));
    }
    if let (Some((low, low_value)), Some((high, high_value))) = (found.lowest, found.highest) {
        lines.push(format!(
            "Lowest: {} ({}). Highest: {} ({}).",
            Examples::name(low),
            field.format(low_value),
            Examples::name(high),
            field.format(high_value)
        ));
    }
    lines
}

/// A line of the grid: field label with an optional Bulbapedia info note, then its widgets.
fn line_with_note(ui: &mut egui::Ui, label: &str, note: Option<FieldNote>, add: impl FnOnce(&mut egui::Ui)) {
    let group: String = ui.ctx().data(|d| d.get_temp(egui::Id::new(GROUP_ID))).unwrap_or_default();
    let key = format!("{group}/{label}");
    let modified: std::collections::HashSet<String> = ui.ctx().data(|d| d.get_temp(egui::Id::new(MODIFIED_ID))).unwrap_or_default();
    let is_modified = modified.contains(&key);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(if is_modified { egui::RichText::new(label).color(color::ACCENT).size(13.0) } else { widget::hint(label) });
        if is_modified {
            // The value is not the one of the game: a button puts it back.
            let back = ui.add(egui::Button::new(egui::RichText::new("↺").color(color::ACCENT)).small().frame(false)).on_hover_text("Changed. Click to put back the value of the game.");
            if back.clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(REVERT_ID), key.clone()));
            }
        }
        if let Some(n) = note {
            let info = ui.hyperlink_to("ℹ", n.url);
            info.on_hover_ui(|ui| {
                ui.set_max_width(280.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(egui::RichText::new(label).strong());
                ui.label(n.summary);
                if let Some((field, value)) = n.example {
                    for text in example_lines(ui, field, value) {
                        ui.label(widget::hint(&text));
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.label(widget::hint("Learn more on"));
                    ui.hyperlink_to("Bulbapedia ↗", n.url);
                });
            });
        }
    });
    ui.horizontal(add);
    ui.end_row();
}

/// A line of the grid: the name of the field, then its widgets.
#[allow(dead_code)]
fn line(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    line_with_note(ui, label, None, add);
}

/// The colour of each Pokédex colour, in the order of the game (the palette of Pokémon HOME).
const POKEDEX_COLORS: [(u8, u8, u8); 10] = [
    (250, 72, 72),   // Red
    (125, 146, 250), // Blue
    (255, 235, 50),  // Yellow
    (96, 200, 96),   // Green
    (34, 34, 34),    // Black
    (205, 150, 74),  // Brown
    (186, 120, 250), // Purple
    (186, 186, 186), // Gray
    (255, 255, 255), // White
    (255, 160, 200), // Pink
];

/// A menu of the Pokédex colours: the button and the list show rectangles of colour to click.
fn color_menu(ui: &mut egui::Ui, id: &str, value: &mut u8) {
    let swatch = |place: usize| {
        let (r, g, b) = POKEDEX_COLORS[place.min(POKEDEX_COLORS.len() - 1)];
        egui::Color32::from_rgb(r, g, b)
    };
    let size = egui::vec2(110.0, 22.0);
    let combo = egui::ComboBox::from_id_salt(id).selected_text("").width(size.x + 12.0).show_ui(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        for place in 0..POKEDEX_COLORS.len() {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            ui.painter().rect_filled(rect, 4.0, swatch(place));
            if *value == place as u8 {
                widget::glow(ui, rect, 5.0);
            } else if response.hovered() {
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0_f32, color::MUTED));
            }
            response.clone().on_hover_text(names::COLOR[place]);
            if response.clicked() {
                *value = place as u8;
                ui.close_menu();
            }
        }
    });
    let rect = combo.response.rect;
    let inner = egui::Rect::from_min_max(rect.min + egui::vec2(4.0, 4.0), egui::pos2(rect.right() - 22.0, rect.bottom() - 4.0));
    ui.painter().rect_filled(inner, 3.0, swatch(*value as usize));
    let menu_id = combo.response.id;
    widget::register_focusable(ui, menu_id, rect);
    if combo.response.clicked() {
        widget::set_active_id(ui, menu_id);
    }
    if widget::is_active(ui, menu_id) && widget::page_has_focus(ui) {
        widget::glow(ui, rect, 6.0);
        widget::scroll_active_into_view(ui, &combo.response);
        ui.input_mut(|i| {
            let mut step: isize = 0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                step -= 1;
            }
            if step != 0 {
                *value = (*value as isize + step).clamp(0, POKEDEX_COLORS.len() as isize - 1) as u8;
            }
        });
    }
}

/// The width of one column of groups, in points.
const COLUMN_WIDTH: f32 = 430.0;

/// A grid for the fields of a group.
fn grid(name: &'static str) -> egui::Grid {
    egui::Grid::new(name).num_columns(2).spacing([16.0, 8.0])
}

/// A group of fields: a card with a title.
fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    // The fields of the group know their group: "HP" is a base stat and an EV yield.
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(GROUP_ID), title.to_string()));
    widget::card(ui, |ui| {
        ui.set_min_width(COLUMN_WIDTH - 36.0);
        ui.label(widget::title(title));
        add(ui);
    });
    ui.add_space(10.0);
}

fn stat_line(ui: &mut egui::Ui, label: &str, field: Field, summary: &'static str, value: &mut u8) {
    line_with_note(
        ui,
        label,
        Some(FieldNote {
            summary,
            url: "https://bulbapedia.bulbagarden.net/wiki/Stat#Base_stat_values",
            example: Some((field, *value as u32)),
        }),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            widget::stat_bar(ui, value, 255.0);
            number(ui, value, 255.0);
        },
    );
}

fn group_statistics(ui: &mut egui::Ui, d: &mut SpeciesData, _shape: &mut u8) {
    group(ui, "Base stats", |ui| {
        grid("stats").show(ui, |ui| {
            stat_line(ui, "HP", Field::Hp, "Hit Points (HP). Amount of damage this Pokémon can withstand before fainting.", &mut d.base_hp);
            stat_line(ui, "Attack", Field::Attack, "Physical Attack. Governs direct damage dealt (e.g. Slash, Earthquake) against enemy Defense.", &mut d.base_attack);
            stat_line(ui, "Defense", Field::Defense, "Physical Defense. Reduces damage taken from enemy physical attacks.", &mut d.base_defense);
            stat_line(ui, "Special Attack", Field::SpecialAttack, "Special Attack. Governs special move damage dealt (e.g. Surf, Flamethrower) against enemy Special Defense.", &mut d.base_special_attack);
            stat_line(ui, "Special Defense", Field::SpecialDefense, "Special Defense. Reduces damage taken from enemy special attacks.", &mut d.base_special_defense);
            stat_line(ui, "Speed", Field::Speed, "Speed. Determines turn order: the Pokémon with higher Speed acts first.", &mut d.base_speed);
        });

        ui.add_space(10.0);
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(3, |columns| {
                    columns[0].vertical_centered(|ui| {
                        ui.label(widget::hint("Total"));
                        ui.label(egui::RichText::new(format!("{}", d.base_total())).strong().size(15.0));
                    });
                    columns[1].vertical_centered(|ui| {
                        ui.label(widget::hint("Average"));
                        ui.label(egui::RichText::new(format!("{:.1}", d.base_average())).strong().size(15.0));
                    });
                    columns[2].vertical_centered(|ui| {
                        ui.label(widget::hint("Std Dev"));
                        ui.label(egui::RichText::new(format!("{:.1}", d.base_standard_deviation())).strong().size(15.0));
                    });
                });
            });
    });
}

fn group_characteristics(ui: &mut egui::Ui, d: &mut SpeciesData, shape: &mut u8) {
    group(ui, "Characteristics", |ui| {
        grid("characteristics").show(ui, |ui| {
            line_with_note(
                ui,
                "Type 1",
                Some(FieldNote {
                    summary: "Primary elemental type defining offensive and defensive effectiveness, plus STAB bonus (+50%).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Type",
                    example: Some((Field::Type1, d.type_1 as u32)),
                }),
                |ui| {
                    type_menu(ui, "type_1", &mut d.type_1, None);
                },
            );
            line_with_note(
                ui,
                "Type 2",
                Some(FieldNote {
                    summary: "Secondary elemental type (optional for dual-type Pokémon).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Type",
                    example: Some((Field::Type2, d.type_2 as u32)),
                }),
                |ui| {
                    let first = d.type_1;
                    type_menu(ui, "type_2", &mut d.type_2, Some(first));
                },
            );
            line_with_note(
                ui,
                "Ability 1",
                Some(FieldNote {
                    summary: "First standard passive ability providing a unique battle or field effect.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                    example: Some((Field::Ability1, d.ability_1 as u32)),
                }),
                |ui| {
                    ability_picker(ui, "ability_1", &mut d.ability_1);
                },
            );
            line_with_note(
                ui,
                "Ability 2",
                Some(FieldNote {
                    summary: "Second standard passive ability possible for this species (if any).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                    example: Some((Field::Ability2, d.ability_2 as u32)),
                }),
                |ui| {
                    ability_picker(ui, "ability_2", &mut d.ability_2);
                },
            );
            line_with_note(
                ui,
                "Hidden ability",
                Some(FieldNote {
                    summary: "Rare hidden ability, typically obtainable through special encounters, Friend Safari, or hordes.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                    example: Some((Field::HiddenAbility, d.ability_hidden as u32)),
                }),
                |ui| {
                    ability_picker(ui, "ability_hidden", &mut d.ability_hidden);
                },
            );
            line_with_note(
                ui,
                "Egg Group 1",
                Some(FieldNote {
                    summary: "Primary egg group determining breeding compatibility at the Pokémon Day Care.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_Group",
                    example: Some((Field::EggGroup1, d.egg_group_1 as u32)),
                }),
                |ui| {
                    menu(ui, "egg_1", &mut d.egg_group_1, &names::EGG_GROUP, 140.0);
                },
            );
            line_with_note(
                ui,
                "Egg Group 2",
                Some(FieldNote {
                    summary: "Secondary egg group for Pokémon belonging to multiple breeding categories.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_Group",
                    example: Some((Field::EggGroup2, d.egg_group_2 as u32)),
                }),
                |ui| {
                    menu(ui, "egg_2", &mut d.egg_group_2, &names::EGG_GROUP, 140.0);
                },
            );
            line_with_note(
                ui,
                "Held Item 50%",
                Some(FieldNote {
                    summary: "Common held item present on 50% of wild Pokémon of this species upon encounter.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_wild_held_item",
                    example: Some((Field::HeldItem50, d.item_50 as u32)),
                }),
                |ui| {
                    item_picker(ui, "item_50", &mut d.item_50);
                },
            );
            line_with_note(
                ui,
                "Held Item 5%",
                Some(FieldNote {
                    summary: "Rare held item present on 5% of wild Pokémon of this species upon encounter.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_wild_held_item",
                    example: Some((Field::HeldItem5, d.item_5 as u32)),
                }),
                |ui| {
                    item_picker(ui, "item_5", &mut d.item_5);
                },
            );
            let rate_list = leveling_rate_names(ui.ctx(), None);
            let mut rate_refs: Vec<String> = (*rate_list).clone();
            if (d.leveling_rate as usize) >= rate_refs.len() && (d.leveling_rate as usize) < sango_core::experience::MAX_RATES {
                for r in rate_refs.len()..=(d.leveling_rate as usize) {
                    rate_refs.push(format!("Rate {r}"));
                }
            }
            let rate_strs: Vec<&str> = rate_refs.iter().map(|s| s.as_str()).collect();
            line_with_note(
                ui,
                "Leveling rate",
                Some(FieldNote {
                    summary: "Growth rate determining the total experience points required to reach level 100 (e.g. 1,000,000 for Medium Fast).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Experience#Experience_total_by_growth_rate",
                    example: Some((Field::LevelingRate, d.leveling_rate as u32)),
                }),
                |ui| {
                    menu(ui, "leveling_rate", &mut d.leveling_rate, &rate_strs, 165.0);
                },
            );
            line_with_note(
                ui,
                "Shape",
                Some(FieldNote {
                    summary: "Physical body shape (quadruped, winged, serpentine...) used for form sorting in the Pokédex.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_shape",
                    example: Some((Field::Shape, *shape as u32)),
                }),
                |ui| {
                    menu(ui, "shape", shape, &names::SHAPE, 190.0);
                },
            );
            line_with_note(
                ui,
                "Color",
                Some(FieldNote {
                    summary: "Official dominant color used for visual search in the Pokédex.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_color",
                    example: Some((Field::Color, d.color as u32)),
                }),
                |ui| {
                    color_menu(ui, "pokedex_color", &mut d.color);
                },
            );
            line_with_note(
                ui,
                "Gender ratio",
                Some(FieldNote {
                    summary: "Probability of being male or female. 0 = 100% ♂, 254 = 100% ♀, 255 = Genderless.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Gender",
                    example: Some((Field::GenderRatio, d.gender_ratio as u32)),
                }),
                |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| gender_widget(ui, &mut d.gender_ratio));
                        if d.gender_ratio != 255 && ui.small_button("Genderless").on_hover_text("Genderless / gender-unknown (255)").clicked() {
                            d.gender_ratio = 255;
                        }
                    });
                },
            );
            line_with_note(
                ui,
                "Hatch time",
                Some(FieldNote {
                    summary: "Number of egg cycles required to hatch. In Gen VI, one cycle equals 256 steps.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_cycle",
                    example: Some((Field::HatchTime, d.hatch_time as u32)),
                }),
                |ui| {
                    // The unit: cycles (the field of the game) or steps (256 steps per cycle).
                    let unit_id = egui::Id::new("hatch_unit_steps");
                    let mut steps_unit = ui.data(|data| data.get_temp::<bool>(unit_id).unwrap_or(false));
                    if steps_unit {
                        let mut steps = d.hatch_time as u32 * 256;
                        number(ui, &mut steps, 65280.0);
                        d.hatch_time = (steps / 256).min(255) as u8;
                    } else {
                        number(ui, &mut d.hatch_time, 255.0);
                    }
                    egui::ComboBox::from_id_salt("hatch_unit_menu").width(72.0).selected_text(if steps_unit { "steps" } else { "cycles" }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut steps_unit, false, "cycles");
                        ui.selectable_value(&mut steps_unit, true, "steps");
                    });
                    ui.data_mut(|data| data.insert_temp(unit_id, steps_unit));
                },
            );
            line_with_note(
                ui,
                "Height",
                Some(FieldNote {
                    summary: "Pokémon height from head to toe.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_height",
                    example: Some((Field::Height, d.height as u32)),
                }),
                |ui| {
                    height_field(ui, "height", &mut d.height);
                },
            );
            line_with_note(
                ui,
                "Weight",
                Some(FieldNote {
                    summary: "Pokémon body weight. Determines damage dealt or taken by Heavy Slam, Heat Crash, Grass Knot, and Low Kick.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_weight",
                    example: Some((Field::Weight, d.weight as u32)),
                }),
                |ui| {
                    weight_field(ui, "weight", &mut d.weight);
                },
            );
            line_with_note(
                ui,
                "Catch rate",
                Some(FieldNote {
                    summary: "Catch rate (3 to 255). Higher values make the Pokémon easier to catch with a Poké Ball.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Catch_rate",
                    example: Some((Field::CatchRate, d.catch_rate as u32)),
                }),
                |ui| {
                    number(ui, &mut d.catch_rate, 255.0);
                },
            );
            line_with_note(
                ui,
                "Base friendship",
                Some(FieldNote {
                    summary: "Initial friendship value (usually 70) upon catch or hatch. Requires 220 to evolve by friendship.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Friendship",
                    example: Some((Field::BaseFriendship, d.base_friendship as u32)),
                }),
                |ui| {
                    number(ui, &mut d.base_friendship, 255.0);
                },
            );
            line_with_note(
                ui,
                "Base experience",
                Some(FieldNote {
                    summary: "Base experience yield determining XP awarded to the party upon defeating this Pokémon.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Experience",
                    example: Some((Field::BaseExperience, d.base_experience as u32)),
                }),
                |ui| {
                    number(ui, &mut d.base_experience, 65535.0);
                },
            );
            line_with_note(
                ui,
                "Burrowed entry",
                Some(FieldNote {
                    summary: "Specifies whether the Pokémon emerges directly burrowed in the ground upon entering battle (Diglett and Dugtrio), skipping the standard mid-air Poké Ball drop animation.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Diglett_(Pok%C3%A9mon)#Trivia",
                    example: Some((Field::BurrowedEntry, d.is_burrowed_entry() as u32)),
                }),
                |ui| {
                    let is_burrowed = d.is_burrowed_entry();
                    if ui.selectable_label(is_burrowed, "Yes").clicked() {
                        d.set_burrowed_entry(true);
                    }
                    if ui.selectable_label(!is_burrowed, "No").clicked() {
                        d.set_burrowed_entry(false);
                    }
                },
            );
        });
    });
}

fn ev_graduated_bar(ui: &mut egui::Ui, value: &mut u8) {
    let width = 110.0;
    let height = 24.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let track_rect = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 4.0), egui::vec2(width - 16.0, 6.0));
    let track_rounding = egui::Rounding::same(3.0);

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
            *value = (ratio * 3.0).round().clamp(0.0, 3.0) as u8;
        }
    }

    ui.painter().rect_filled(track_rect, track_rounding, color::SURFACE_STRONG);

    if *value > 0 {
        let fill_w = (*value as f32 / 3.0) * track_rect.width();
        let fill_rect = egui::Rect::from_min_size(track_rect.min, egui::vec2(fill_w, track_rect.height()));
        ui.painter().rect_filled(fill_rect, track_rounding, color::ACCENT);
    }

    let step_x = |s: u8| track_rect.left() + (s as f32 / 3.0) * track_rect.width();
    for s in 0..=3 {
        let x = step_x(s);
        let dot_color = if s <= *value { color::WHITE } else { color::MUTED.linear_multiply(0.6) };
        ui.painter().circle_filled(egui::pos2(x, track_rect.center().y), 2.0, dot_color);

        let is_selected = s == *value;
        let num_color = if is_selected { color::ACCENT } else { color::MUTED };
        ui.painter().text(
            egui::pos2(x, track_rect.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{s}"),
            egui::FontId::proportional(11.0),
            num_color,
        );
    }

    let handle_center = egui::pos2(step_x(*value), track_rect.center().y);
    let radius = 5.5;
    if response.hovered() || response.dragged() {
        ui.painter().circle_filled(handle_center, radius + 3.0, color::ACCENT.linear_multiply(0.25));
    }
    ui.painter().circle_filled(handle_center + egui::vec2(0.0, 1.0), radius + 0.5, egui::Color32::from_black_alpha(20));
    ui.painter().circle_filled(handle_center, radius, color::WHITE);
    ui.painter().circle_stroke(handle_center, radius, egui::Stroke::new(2.0_f32, color::ACCENT));

    ui.label(
        egui::RichText::new(format!("+{value}"))
            .color(if *value > 0 { color::ACCENT } else { color::MUTED })
            .strong()
            .size(13.5),
    );
}

fn ev_line(ui: &mut egui::Ui, label: &str, field: Field, summary: &'static str, mut value: u8, mut set: impl FnMut(u8)) {
    line_with_note(
        ui,
        label,
        Some(FieldNote {
            summary,
            url: "https://bulbapedia.bulbagarden.net/wiki/Effort_values",
            example: Some((field, value as u32)),
        }),
        |ui| {
            let before = value;
            ev_graduated_bar(ui, &mut value);
            if value != before {
                set(value);
            }
        },
    );
}

fn group_ev_yield(ui: &mut egui::Ui, d: &mut SpeciesData, _shape: &mut u8) {
    group(ui, "EV yield", |ui| {
        grid("ev_yield").show(ui, |ui| {
            ev_line(ui, "HP", Field::EvHp, "Effort Values (EVs) in HP awarded to the victor (0 to 3).", d.ev_hp(), |v| d.set_ev_hp(v));
            ev_line(ui, "Attack", Field::EvAttack, "Effort Values (EVs) in Attack awarded to the victor (0 to 3).", d.ev_attack(), |v| d.set_ev_attack(v));
            ev_line(ui, "Defense", Field::EvDefense, "Effort Values (EVs) in Defense awarded to the victor (0 to 3).", d.ev_defense(), |v| d.set_ev_defense(v));
            ev_line(ui, "Special Attack", Field::EvSpecialAttack, "Effort Values (EVs) in Special Attack awarded to the victor (0 to 3).", d.ev_special_attack(), |v| d.set_ev_special_attack(v));
            ev_line(ui, "Special Defense", Field::EvSpecialDefense, "Effort Values (EVs) in Special Defense awarded to the victor (0 to 3).", d.ev_special_defense(), |v| d.set_ev_special_defense(v));
            ev_line(ui, "Speed", Field::EvSpeed, "Effort Values (EVs) in Speed awarded to the victor (0 to 3).", d.ev_speed(), |v| d.set_ev_speed(v));
        });

        ui.add_space(8.0);
        let total = d.ev_total();
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(widget::hint("Total"));
                    ui.label(egui::RichText::new(format!("{total} EV{}", if total > 1 { "s" } else { "" })).strong());
                });
            });
    });
}

/// The groups of the General tab, side by side: as many columns as the window has room for.
fn general_tab(ui: &mut egui::Ui, d: &mut SpeciesData, shape: &mut u8, side: impl FnOnce(&mut egui::Ui)) {
    const GROUPS: [fn(&mut egui::Ui, &mut SpeciesData, &mut u8); 3] = [
        group_statistics,
        group_characteristics,
        group_ev_yield,
    ];
    let columns = (((ui.available_width() + 10.0) / (COLUMN_WIDTH + 10.0)).floor() as usize).clamp(1, 2);
    let layout: Vec<Vec<usize>> = match columns {
        2 => vec![
            vec![0, 2], // Column 0: Base stats, EV yield
            vec![1], // Column 1: Characteristics
        ],
        _ => vec![(0..GROUPS.len()).collect()],
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for column in layout {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN_WIDTH);
                    for index in column {
                        GROUPS[index](ui, d, shape);
                    }
                });
                ui.add_space(10.0);
            }
            // The texts, the icon and the cry, under each other, next to the characteristics.
            ui.vertical(|ui| side(ui));
        });
    });
}

impl Module for Species {
    fn unsaved(&self) -> bool {
        self.data.is_some() && (self.data != self.baseline || self.shape != self.shape_baseline || self.learnset.changed() || self.evolution.changed() || self.text.changed() || self.icon_tab.changed() || self.cry_tab.changed())
    }

    fn title(&self) -> &str {
        "Species"
    }

    fn icon(&self) -> &str {
        "🐾"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        // The packs may have added or removed species: the lists, the data and the models are read again.
        let packs_changed = extras::refresh(session.sd.as_ref());
        if packs_changed {
            self.list_rows = None;
            self.loaded = None;
        }
        let models_folder = session.sd.as_ref().map(|sd| sd.models_folder());
        self.model_tab.set_sd_folder(models_folder, packs_changed);
        self.model_tab.set_sd_root(session.sd.as_ref().map(|sd| sd.root().to_string_lossy().to_string()));
        // Another page asked for a species.
        if let Some((species, form)) = ui.ctx().data_mut(|d| d.remove_temp::<(usize, usize)>(egui::Id::new(OPEN_SPECIES))) {
            self.species = if extras::is_species(species) { species } else { species.clamp(1, SPECIES_COUNT) };
            self.form = form;
            self.tab = 0;
            self.loaded = None;
        }
        ui.add_space(8.0);
        ui.heading("Species");
        ui.add_space(8.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        if self.root.as_deref() != Some(dump.root()) {
            self.root = Some(dump.root().to_path_buf());
            match Garc::open(&dump.archive(ARCHIVE)) {
                Ok(garc) => self.garc = Some(garc),
                Err(message) => {
                    self.garc = None;
                    self.status = Some((false, message));
                }
            }
            self.pokedex_archive = Garc::open(&dump.archive(pokedex_data::ARCHIVE)).ok();
            self.mega_garc = Garc::open(&dump.archive(sango_core::mega_evolution::ARCHIVE)).ok();
            self.icon_garc = Garc::open(&dump.archive(icons::ICON_ARCHIVE)).ok();
            self.learnset.open(&dump);
            self.evolution.open(&dump);
            self.text.open(&dump);
            self.icon_tab.open(&dump);
            self.cry_tab.open(&dump);
            self.model_tab.open(&dump);
            battle_icons::load(&dump);
            if let Ok(mut list) = self.item_list.lock() {
                list.open(&dump);
            }
            self.icon_textures.clear();
            self.form_icon_textures.clear();
            // The files that older versions wrote by species and form get the number of their file.
            if let (Some(sd), Some(garc)) = (session.sd.as_ref(), self.garc.as_ref()) {
                if let Err(message) = old_layout::migrate(sd, garc) {
                    self.status = Some((false, message));
                }
            }
            self.list_rows = None;
            self.loaded = None;
            self.examples = match (&self.garc, self.read_pokedex_romfs()) {
                (Some(garc), Ok(pokedex)) => Some(Arc::new(build_index(garc, &pokedex))),
                _ => None,
            };
        }
        if self.garc.is_some() && self.loaded != Some((self.species, self.form)) {
            self.load(session);
        }
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(ITEM_LIST_ID), self.item_list.clone()));
        ability_names(ui.ctx(), session.sd.as_ref());
        leveling_rate_names(ui.ctx(), session.sd.as_ref());
        custom_moves::refresh(session.sd.as_ref());
        if let Ok(mut list) = self.item_list.lock() {
            list.refresh_custom(session.sd.as_ref());
        }
        battle_icons::refresh_custom_types(session.sd.as_ref());
        self.top_bar(ui, session);
        if let Some(target) = self.pending_open {
            let what = format!("#{:04} {}", self.species, entry_name(self.species, self.form));
            match widget::unsaved_window(ui.ctx(), &what) {
                Some(widget::Unsaved::Save) => {
                    self.save(session);
                    if !self.loses_changes() {
                        (self.species, self.form) = target;
                        self.loaded = None;
                    }
                    self.pending_open = None;
                }
                Some(widget::Unsaved::Discard) => {
                    (self.species, self.form) = target;
                    self.loaded = None;
                    self.pending_open = None;
                }
                Some(widget::Unsaved::Stay) => self.pending_open = None,
                None => {}
            }
        }
        self.reset_window(ui.ctx(), session);
        self.csv_windows(ui.ctx(), session);
        self.new_pokemon_window(ui.ctx(), session);
        ui.add_space(8.0);
        let mut clicked_tab = None;
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(12.0))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for (i, name) in TABS.iter().enumerate() {
                        let tab_id = egui::Id::new(format!("species_tab_{i}"));
                        let is_active_tab = widget::is_active(ui, tab_id);
                        if is_active_tab && self.tab != i {
                            self.tab = i;
                        }
                        let chosen = self.tab == i;
                        let r = widget::tab_button(ui, name, chosen, is_active_tab && widget::page_has_focus(ui));
                        widget::register_focusable(ui, tab_id, r.rect);
                        ui.ctx().data_mut(|d| d.insert_temp(tab_id.with("is_button"), true));
                        if r.clicked() {
                            clicked_tab = Some((i, tab_id));
                        }
                    }
                });
            });
        if let Some((i, tab_id)) = clicked_tab {
            self.tab = i;
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        }

        // Global tab-cycle shortcuts: Ctrl+PageDown / Ctrl+PageUp
        let (next_tab, prev_tab) = ui.input_mut(|i| {
            let next = (i.modifiers.ctrl || i.modifiers.command)
                && (i.consume_key(egui::Modifiers::CTRL, egui::Key::PageDown)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::PageDown));
            let prev = (i.modifiers.ctrl || i.modifiers.command)
                && (i.consume_key(egui::Modifiers::CTRL, egui::Key::PageUp)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::PageUp));
            (next, prev)
        });
        if next_tab && self.tab + 1 < TABS.len() {
            self.tab += 1;
            let tab_id = egui::Id::new(format!("species_tab_{}", self.tab));
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        } else if prev_tab && self.tab > 0 {
            self.tab -= 1;
            let tab_id = egui::Id::new(format!("species_tab_{}", self.tab));
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        }
        ui.add_space(6.0);
        if let Some(index) = &self.examples {
            let shared = (index.clone(), self.species);
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(EXAMPLES_ID), shared));
        }
        match (self.tab, self.data.as_mut()) {
            (0, Some(_)) => {
                // The data leaves the page while the tab is drawn: the icon card can read the files again.
                if let Some(mut data) = self.data.take() {
                    let modified: std::collections::HashSet<String> = match &self.game {
                        Some(game) => FIELD_KEYS.iter().filter(|key| field_value(key, &data) != field_value(key, game)).map(|key| key.to_string()).collect(),
                        None => Default::default(),
                    };
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(MODIFIED_ID), modified));
                    let mut shape = self.shape;
                    let place = self.pokedex.as_ref().map_or(self.species, |pokedex| pokedex.place(self.species, self.form));
                    general_tab(ui, &mut data, &mut shape, |ui| {
                        self.text.compact_ui(ui, session, self.species, self.form, place);
                        ui.add_space(8.0);
                        self.icon_tab.ui(ui, session, self.species, self.form);
                        ui.add_space(8.0);
                        self.cry_tab.ui(ui, session, self.species, self.form);
                    });
                    if let (Some(key), Some(game)) = (ui.ctx().data_mut(|d| d.remove_temp::<String>(egui::Id::new(REVERT_ID))), &self.game) {
                        if let Some(value) = field_value(&key, game) {
                            set_field(&key, &mut data, value);
                        }
                    }
                    if self.loaded.is_some() {
                        self.data = Some(data);
                        self.shape = shape;
                    }
                }
            }
            (2, Some(data)) => self.learnset.ui(ui, data),
            (1, Some(_)) => self.model_tab.ui(ui, self.species, self.form),
            // The evolutions, then the Mega Evolutions: both change what the species becomes.
            (3, Some(_)) => {
                egui::ScrollArea::vertical().id_salt("evolution_tab").auto_shrink([false, false]).show(ui, |ui| {
                    self.evolution.ui(ui);
                    self.mega_tab(ui, session);
                });
            }
            (0, None) => {}
            _ => {
                ui.label(widget::hint("..."));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_help_of_a_field_lists_examples_from_the_index() {
        let row = |n: usize, hp: u8| {
            let mut d = SpeciesData::from_bytes(&[0; 80]).unwrap();
            d.base_hp = hp;
            (n, d, 0u8)
        };
        let index = Arc::new(examples::Index::new(vec![row(1, 45), row(2, 60), row(3, 45)]));
        let ctx = egui::Context::default();
        let mut lines = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(example_lines(ui, Field::Hp, 45).is_empty(), "no index: no lines");
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(EXAMPLES_ID), (index.clone(), 1usize)));
                lines = example_lines(ui, Field::Hp, 45);
            });
        });
        assert_eq!(lines, ["Same value: Venusaur.", "Lowest: Bulbasaur (45). Highest: Ivysaur (60)."]);
    }

    #[test]
    fn a_rate_is_a_percentage_of_255() {
        assert_eq!(percent(255), "100 %");
        assert_eq!(percent(0), "0 %");
        assert_eq!(percent(45), "17.6 %");
        assert_eq!(percent(3), "1.2 %");
    }

    #[test]
    fn the_gender_number_gives_the_chance_to_be_female() {
        assert_eq!(gender_text(0), "male only");
        assert_eq!(gender_text(254), "female only");
        assert_eq!(gender_text(255), "no gender");
        assert_eq!(gender_text(31), "12.5 % female, 87.5 % male");
        assert_eq!(gender_text(127), "50 % female, 50 % male");
    }

    #[test]
    fn every_field_goes_back_to_the_value_of_the_game() {
        let game = SpeciesData::from_bytes(&[3; 0x50]).unwrap();
        for key in FIELD_KEYS {
            let mut data = game.clone();
            let value = field_value(key, &game).unwrap_or_else(|| panic!("{key} has no value"));
            set_field(key, &mut data, if value == 0 { 1 } else { 0 });
            assert_ne!(field_value(key, &data), field_value(key, &game), "{key} changes");
            let changed: Vec<&str> = FIELD_KEYS.iter().copied().filter(|k| field_value(k, &data) != field_value(k, &game)).collect();
            assert_eq!(changed, [key], "only {key} shows a mark");
            set_field(key, &mut data, value);
            assert_eq!(data, game, "{key} goes back");
        }
        let template = template_data(&game);
        assert_eq!((template.base_hp, template.type_1, template.item_50, template.ev_hp()), (50, 0, 0, 0));
        assert_eq!(template.total_form_count, game.total_form_count, "the fields of the forms stay");
    }

    #[test]
    fn a_form_is_a_pokemon_of_its_own_in_the_list() {
        assert_eq!(entry_name(479, 0), "Rotom");
        assert_eq!(entry_name(479, 1), "Rotom-Heat");
        assert_eq!(entry_name(6, 1), "Charizard-Mega-X");
        let row = ListRow { types: [0, 0], stats: [0; 6], total: 0, modified: false };
        let entry = |species, form| ListEntry { species, form, name: entry_name(species, form), row: row.clone() };
        let entries = vec![entry(478, 0), entry(479, 0), entry(479, 1), entry(479, 2), entry(480, 0)];
        assert_eq!(match_entries(&entries, "rotom"), vec![1, 2, 3], "a species comes with all its forms");
        assert_eq!(match_entries(&entries, "heat"), vec![2], "a form is found by the name of the form");
        assert_eq!(match_entries(&entries, "").len(), 5);
        // Without the archive, a species of the game has one form: the steps go from species to species.
        assert_eq!(step_entry(None, 1, 0, 1), (2, 0));
        assert_eq!(step_entry(None, 2, 0, -1), (1, 0));
        assert_eq!(step_entry(None, 1, 0, -1), (1, 0));
    }

    #[test]
    fn species_can_be_searched_by_name_or_number() {
        // The packs may have added species (the tests share them): the species of the game are always there.
        assert!(match_species("").len() >= 721);
        assert_eq!(match_species("pika")[0], 25);
        assert_eq!(match_species("char")[0], 4);
        assert_eq!(match_species("25")[0], 25);
        assert_eq!(match_species("025")[0], 25);
        assert_eq!(match_species("#25")[0], 25);
        assert_eq!(match_species("#0025")[0], 25);
        assert_eq!(match_species("flabebe")[0], 669);
    }

    #[test]
    fn height_and_weight_formatting() {
        assert_eq!(height_text(100), "1.00 m / 100 cm (3'03\")");
        assert_eq!(height_text(170), "1.70 m / 170 cm (5'07\")");
        assert_eq!(weight_text(130), "13.0 kg (28.7 lbs)");
    }

    #[test]
    fn picker_labels_are_formatted() {
        let label = |id: usize, list: &[&str]| {
            if id == 0 {
                "#000 None".to_string()
            } else if let Some(&name) = list.get(id - 1) {
                format!("#{:03} {}", id, name)
            } else {
                format!("#{:03} Unknown", id)
            }
        };
        assert_eq!(label(0, &names::ABILITY), "#000 None");
        assert_eq!(label(1, &names::ABILITY), "#001 Stench");
        assert_eq!(label(65, &names::ABILITY), "#065 Overgrow");
        assert_eq!(label(0, &names::ITEM), "#000 None");
        assert_eq!(label(1, &names::ITEM), "#001 Master Ball");
    }

    #[test]
    fn clicking_top_bar_and_general_tab_does_not_crash() {
        let mut species = Species::default();
        let mut data = data::SpeciesData::from_bytes(&[0; 80]).unwrap();
        species.data = Some(data.clone());
        species.baseline = Some(data.clone());

        let ctx = egui::Context::default();
        let session = Session::default();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                widget::set_page_focus(ctx, true);
                widget::update_navigation(ui);
                species.top_bar(ui, &session);
                general_tab(ui, &mut data, &mut 0u8, |_| {});
            });
        });

        for &(x, y) in &[(10.0, 10.0), (100.0, 50.0), (250.0, 50.0), (350.0, 50.0), (50.0, 200.0), (200.0, 300.0)] {
            let mut click = egui::RawInput::default();
            click.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            let _ = ctx.run(click, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::set_page_focus(ctx, true);
                    widget::update_navigation(ui);
                    species.top_bar(ui, &session);
                    general_tab(ui, &mut data, &mut 0u8, |_| {});
                });
            });
        }
    }

    /// With the dump: the species and the forms that a pack adds are in the list, in the data, in the forms of the page and in the
    /// tabs, like the ones of the game.
    #[test]
    fn the_species_and_the_forms_of_the_packs_are_shown_like_the_ones_of_the_game() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        use sango_pack::table::{FormRecord, SpeciesRecord};
        let mut rowlet = vec![0u8; 0x50];
        rowlet[0] = 68;
        rowlet[1] = 55;
        let mut mega = vec![0u8; 0x50];
        mega[0] = 99;
        let _table = extras::set_for_test(sango_pack::Table {
            species: vec![SpeciesRecord { id: 724, model_form: 1, name: "Rowlet".into(), data: rowlet, moves: vec![(33, 1), (45, 4)], evolutions: vec![(4, 17, 725)] }, SpeciesRecord { id: 725, model_form: 2, name: "Dartrix".into(), data: vec![1; 0x50], moves: Vec::new(), evolutions: Vec::new() }],
            forms: vec![FormRecord { species: 3, form: 2, model_form: 0, data: mega, mega: sango_pack::table::MegaCondition::Never }],
        });
        let folder = std::env::temp_dir().join(format!("sango_species_packs_{}", std::process::id()));
        std::fs::create_dir_all(folder.join("sdmc").join("sango-studio")).unwrap();
        let mut session = Session { dump: Some(sango_core::Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.join("sdmc").to_str().unwrap()).unwrap()), focus_page: true, ..Session::default() };
        let mut page = Species::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Species, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for _ in 0..3 {
            frame(&mut page, &mut session);
        }
        // The list: the numbers 722 and 723 are not species.
        assert_eq!(species_name(724), "Rowlet");
        let ids = species_ids();
        assert!(ids.contains(&724) && ids.contains(&725) && !ids.contains(&722));
        let entries = read_list_entries(page.garc.as_ref(), &session);
        let rowlet = entries.iter().find(|e| (e.species, e.form) == (724, 0)).expect("the species of the pack is in the list");
        assert_eq!(rowlet.row.stats[0], 68);
        assert!(!entries.iter().any(|e| e.species == 722), "the numbers between are not species");
        assert!(entries.iter().any(|e| (e.species, e.form) == (3, 2)), "the form of the pack is a line of its own");
        assert_eq!(match_species("row")[0], 724, "a species of a pack is found by its name, first when its name starts like that");
        // The data and the forms of the page.
        page.species = 724;
        page.form = 0;
        page.loaded = None;
        for _ in 0..3 {
            frame(&mut page, &mut session);
        }
        assert_eq!(page.data.as_ref().map(|d| d.base_hp), Some(68));
        assert_eq!(page.learnset.level_up_moves().len(), 2, "the moves of the pack");
        assert_eq!(page.evolution.count(), 1);
        page.species = 3;
        page.form = 0;
        assert_eq!(page.total_form_count(), 3, "Venusaur has the forms of the game and the form of the pack");
        page.form = 2;
        page.loaded = None;
        for _ in 0..3 {
            frame(&mut page, &mut session);
        }
        assert_eq!(page.data.as_ref().map(|d| d.base_hp), Some(99), "the data of the form of the pack");
        // The other tabs draw for a species of a pack too.
        page.species = 724;
        page.form = 0;
        for tab in 0..TABS.len() {
            page.tab = tab;
            page.loaded = None;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Opens the species list with a click on its button, then moves the choice with the keys.
    #[test]
    fn the_open_species_list_draws_and_moves_with_the_keys() {
        let mut species = Species::default();
        let ctx = egui::Context::default();
        let session = Session::default();

        let run = |species: &mut Species, events: Vec<egui::Event>| {
            let mut input = egui::RawInput::default();
            input.events = events;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::set_page_focus(ctx, true);
                    widget::update_navigation(ui);
                    species.top_bar(ui, &session);
                });
            });
        };
        let click = |x: f32, y: f32| {
            let press = |pressed| egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            vec![press(true), press(false)]
        };

        // Search for the species button in the top bar, until the list opens.
        run(&mut species, vec![]);
        'outer: for y in [30.0, 40.0, 50.0, 60.0, 70.0, 80.0] {
            for x in [40.0, 100.0, 160.0, 220.0] {
                run(&mut species, click(x, y));
                run(&mut species, vec![]);
                if species.list_shown {
                    break 'outer;
                }
            }
        }
        assert!(species.list_shown, "the species list did not open");
        assert!(species.list_rows.is_some(), "the list data was not read");

        // The down key moves the choice to the next species of the sorted list.
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        assert_eq!(species.species, 1);
        run(&mut species, vec![key(egui::Key::ArrowDown)]);
        assert_eq!(species.species, 2);
        run(&mut species, vec![key(egui::Key::ArrowUp)]);
        assert_eq!(species.species, 1);

        // The enter key closes the list and keeps the choice.
        run(&mut species, vec![key(egui::Key::Enter)]);
        run(&mut species, vec![]);
        assert!(!species.list_shown);
        assert_eq!(species.species, 1);
    }

    #[test]
    fn the_species_list_sorts_on_each_column() {
        let row = |types: [u8; 2], hp: u8, attack: u8| ListRow { types, stats: [hp, attack, 0, 0, 0, 0], total: hp as u16 + attack as u16, modified: false };
        let entry = |species: usize, row: ListRow| ListEntry { species, form: 0, name: entry_name(species, 0), row };
        let entries = vec![entry(1, row([0, 1], 10, 0)), entry(2, row([2, 2], 30, 0)), entry(3, row([1, 0], 30, 5))];

        let mut list = vec![0, 1, 2];
        // Equal HP: the order of the list decides.
        sort_entries(&mut list, &entries, SortColumn::Stat(0), true);
        assert_eq!(list, [1, 2, 0]);
        sort_entries(&mut list, &entries, SortColumn::Total, false);
        assert_eq!(list, [0, 1, 2]);
        sort_entries(&mut list, &entries, SortColumn::Name, true);
        assert_eq!(list, [2, 1, 0]);
        sort_entries(&mut list, &entries, SortColumn::Number, false);
        assert_eq!(list, [0, 1, 2]);
        // The names of the types: "Fighting" comes before "Flying", which comes before "Normal".
        sort_entries(&mut list, &entries, SortColumn::Type1, false);
        assert_eq!(list, [2, 1, 0]);
        // A second type that is the first one again shows "-", which comes before the names.
        sort_entries(&mut list, &entries, SortColumn::Type2, false);
        assert_eq!(list, [1, 0, 2]);
        assert_eq!(second_type(&entries[1].row), "-");
        assert_eq!(second_type(&entries[0].row), "Fighting");
    }

    /// With the dump of the game: each species and form points to a file of the archive
    /// of the species data. Two species never share a file; the forms of one species can,
    /// because a form without data of its own uses the file of the form 0.
    #[test]
    fn each_species_and_form_has_its_own_file() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(ARCHIVE)).unwrap();
        let mut seen = HashMap::new();
        for species in 1..=SPECIES_COUNT {
            let forms = garc
                .first_file(species)
                .and_then(|b| SpeciesData::from_bytes(b).ok())
                .map_or(1, |d| (d.total_form_count as usize).max(1));
            for form in 0..forms {
                let id = file_index(&garc, species, form);
                // The last file of the archive is the whole table, not a species.
                assert!(id < garc.file_count() - 1, "species {species} form {form}: file {id}");
                if let Some((other_species, other_form)) = seen.insert(id, (species, form)) {
                    assert_eq!(other_species, species, "species {species} form {form} and species {other_species} form {other_form} share the file {id}");
                }
            }
        }
        assert!(seen.len() > SPECIES_COUNT);
    }

    /// With the dump of the game: the icon of a species is decoded into a texture once,
    /// then the texture is kept.
    #[test]
    fn the_icon_of_a_species_becomes_a_texture() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(icons::ICON_ARCHIVE)).ok();
        let ctx = egui::Context::default();
        let mut textures = HashMap::new();
        let texture = sango_species_base::icon_texture(&mut textures, garc.as_ref(), &ctx, 25).expect("no texture");
        assert_eq!(texture.size(), [40, 30]);
        let _ = sango_species_base::icon_texture(&mut textures, garc.as_ref(), &ctx, 25);
        assert_eq!(textures.len(), 1);
    }

    /// With the dump of the game: every icon file of the table is in the archive of
    /// the Pokemon icons and decodes to a 40 x 30 image.
    #[test]
    fn every_species_icon_of_the_table_is_in_the_dump() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(icons::ICON_ARCHIVE)).unwrap();
        for species in 1..icons::ICON_FILES.len() {
            let file = icons::ICON_FILES[species] as usize;
            let bytes = garc.first_file(file).unwrap_or_else(|| panic!("species {species}: no file {file}"));
            let bytes = if sango_core::lz::is_compressed(bytes) {
                sango_core::lz::decompress(bytes).unwrap_or_else(|e| panic!("species {species}: {e}"))
            } else {
                bytes.to_vec()
            };
            let image = sango_core::Clim::parse(&bytes).unwrap_or_else(|e| panic!("species {species}: {e}"));
            assert_eq!((image.width(), image.height()), (40, 30), "species {species}");
        }
    }

    #[test]
    fn gender_boundary_and_handle_position_align() {
        let compute_boundary = |value: u8, width: f32| -> f32 {
            let (male_pct, _) = match value {
                0 => (100.0, 0.0),
                254 => (0.0, 100.0),
                255 => (0.0, 0.0),
                _ => {
                    let f = (value as f32 + 1.0) * 100.0 / 256.0;
                    (100.0 - f, f)
                }
            };
            (male_pct / 100.0 * width).round()
        };

        let width = 110.0;
        // 100% male -> blue spans full width, handle at right boundary (110)
        assert_eq!(compute_boundary(0, width), 110.0);
        // 100% female -> blue width is 0, handle at left boundary (0)
        assert_eq!(compute_boundary(254, width), 0.0);
        // 50% / 50% -> handle at exactly half width (55)
        assert_eq!(compute_boundary(127, width), 55.0);
        // 87.5% male (starter like Bulbasaur, value 31) -> blue width is 87.5% of 110 = 96.0
        assert_eq!(compute_boundary(31, width), 96.0);
    }
}
