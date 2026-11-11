//! The CSV export and import of the species data.
//!
//! One file holds one row per species and form, with the fields of the General tab.
//! This layout is the model of all the text exports of Sango Studio:
//!
//! - The first line is the header, exactly as in [`HEADER`].
//! - Each other line is one record. The fields are plain decimal numbers with commas
//!   between them, without quotes. The `name` field is the only text: it is there for
//!   the reader, and the import ignores it.
//! - The import reads the rows, checks all of them, and then writes all the changes,
//!   or nothing. A file can hold a part of the rows only.
//!
//! The columns:
//!
//! | Column | Content | Limits |
//! | --- | --- | --- |
//! | `species` | The number of the species. | 1 to 721 |
//! | `form` | The number of the form. 0 is the normal form. | the forms of the species |
//! | `name` | The name of the species. The import ignores it. | text without a comma |
//! | `base_hp` ... `base_speed` | The six base stats. | 0 to 255 |
//! | `type_1`, `type_2` | The two types. Equal values mean one type. | 0 to 17 |
//! | `ability_1`, `ability_2`, `ability_hidden` | The abilities. 0 is none. | 0 to 191 |
//! | `egg_group_1`, `egg_group_2` | The two egg groups. | 0 to 15 |
//! | `item_50`, `item_5` | The wild held items. 0 is none. | 0 to 775 |
//! | `leveling_rate` | The experience curve. | 0 to 7 |
//! | `shape` | The body shape, kept in the Pokédex data. | 0 to 13 |
//! | `color` | The Pokédex color byte, as it is. Some species have more bits set in it. | 0 to 255 |
//! | `gender_ratio` | 0 is male only, 254 female only, 255 no gender. | 0 to 255 |
//! | `hatch_time` | The egg cycles. | 0 to 255 |
//! | `height` | The height in centimeters. | 0 to 65535 |
//! | `weight` | The weight in hectograms. | 0 to 65535 |
//! | `catch_rate` | The catch rate. | 0 to 255 |
//! | `base_friendship` | The friendship at the catch. | 0 to 255 |
//! | `base_experience` | The base experience yield. | 0 to 65535 |
//! | `burrowed_entry` | 1 when the Pokémon enters battle in the ground. | 0 or 1 |
//! | `ev_hp` ... `ev_speed` | The six effort value yields. | 0 to 3 |

use crate::data::SpeciesData;
use crate::pokedex_data::PokedexData;
use crate::{file_index, names, pokedex_data, sd_path, SPECIES_COUNT};
use sango_core::{Garc, SdCard};
use std::fmt::Write as _;

/// The header line of the file.
pub const HEADER: &str = "species,form,name,\
base_hp,base_attack,base_defense,base_special_attack,base_special_defense,base_speed,\
type_1,type_2,ability_1,ability_2,ability_hidden,egg_group_1,egg_group_2,\
item_50,item_5,leveling_rate,shape,color,gender_ratio,hatch_time,height,weight,\
catch_rate,base_friendship,base_experience,burrowed_entry,\
ev_hp,ev_attack,ev_defense,ev_special_attack,ev_special_defense,ev_speed";

/// One row of the file: a species, a form, and the values of the General fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub species: usize,
    pub form: usize,
    pub stats: [u8; 6],
    pub types: [u8; 2],
    pub abilities: [u8; 3],
    pub egg_groups: [u8; 2],
    pub items: [u16; 2],
    pub leveling_rate: u8,
    pub shape: u8,
    pub color: u8,
    pub gender_ratio: u8,
    pub hatch_time: u8,
    pub height: u16,
    pub weight: u16,
    pub catch_rate: u8,
    pub base_friendship: u8,
    pub base_experience: u16,
    pub burrowed_entry: bool,
    pub evs: [u8; 6],
}

/// What an import did: the number of the written species files, and whether the file
/// of the Pokédex data was written for the shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub written: usize,
    pub shape_written: bool,
}

/// The data of one species and form: the SD card first, then the romfs.
fn current(garc: &Garc, sd: Option<&SdCard>, species: usize, form: usize) -> Option<SpeciesData> {
    let on_card = sd.and_then(|sd| sd.read(&sd_path(species, form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
    on_card.or_else(|| garc.first_file(file_index(garc, species, form)).and_then(|b| SpeciesData::from_bytes(b).ok()))
}

/// The number of forms of a species (at least one).
fn form_count(garc: &Garc, species: usize) -> usize {
    garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |d| (d.total_form_count as usize).max(1))
}

/// Writes one row per species and form, from the current data (the SD card first,
/// then the romfs). A missing Pokédex data gives the shape 0.
pub fn export(garc: &Garc, sd: Option<&SdCard>, pokedex: Option<&PokedexData>) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for species in 1..=SPECIES_COUNT {
        for form in 0..form_count(garc, species) {
            let Some(d) = current(garc, sd, species, form) else { continue };
            let shape = pokedex.and_then(|p| p.shape(species, form)).unwrap_or(0);
            let _ = writeln!(
                out,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                species,
                form,
                names::SPECIES[species - 1],
                d.base_hp,
                d.base_attack,
                d.base_defense,
                d.base_special_attack,
                d.base_special_defense,
                d.base_speed,
                d.type_1,
                d.type_2,
                d.ability_1,
                d.ability_2,
                d.ability_hidden,
                d.egg_group_1,
                d.egg_group_2,
                d.item_50,
                d.item_5,
                d.leveling_rate,
                shape,
                d.color,
                d.gender_ratio,
                d.hatch_time,
                d.height,
                d.weight,
                d.catch_rate,
                d.base_friendship,
                d.base_experience,
                d.is_burrowed_entry() as u8,
                d.ev_hp(),
                d.ev_attack(),
                d.ev_defense(),
                d.ev_special_attack(),
                d.ev_special_defense(),
                d.ev_speed(),
            );
        }
    }
    out
}

/// The name of a column, for the error messages.
fn column_name(place: usize) -> &'static str {
    HEADER.split(',').nth(place).unwrap_or("?")
}

/// Reads and checks the CSV text. Nothing is applied yet. The result holds every row,
/// or the list of the problems.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, first)) if first.trim_end() == HEADER => {}
        _ => return Err("The first line is not the header of a species CSV file.".into()),
    }

    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut errors = Vec::new();
    for (place, line) in lines {
        let line_number = place + 1;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        let columns = HEADER.split(',').count();
        if fields.len() != columns {
            errors.push(format!("line {line_number}: {} fields, not {columns}", fields.len()));
            continue;
        }
        // A field as a number, with its limit. A problem gives 0 and one error.
        let mut number = |column: usize, limit: u32| -> u32 {
            match fields[column].trim().parse::<u32>() {
                Ok(value) if value <= limit => value,
                Ok(value) => {
                    errors.push(format!("line {line_number}: {} is {value}, the limit is {limit}", column_name(column)));
                    0
                }
                Err(_) => {
                    errors.push(format!("line {line_number}: {} is not a number", column_name(column)));
                    0
                }
            }
        };
        let species = number(0, SPECIES_COUNT as u32) as usize;
        let form = number(1, 255) as usize;
        // The column 2 is the name: the import ignores it.
        let row = Row {
            species,
            form,
            stats: [
                number(3, 255) as u8,
                number(4, 255) as u8,
                number(5, 255) as u8,
                number(6, 255) as u8,
                number(7, 255) as u8,
                number(8, 255) as u8,
            ],
            types: [number(9, names::TYPE.len() as u32 - 1) as u8, number(10, names::TYPE.len() as u32 - 1) as u8],
            abilities: [
                number(11, names::ABILITY.len() as u32) as u8,
                number(12, names::ABILITY.len() as u32) as u8,
                number(13, names::ABILITY.len() as u32) as u8,
            ],
            egg_groups: [number(14, names::EGG_GROUP.len() as u32 - 1) as u8, number(15, names::EGG_GROUP.len() as u32 - 1) as u8],
            items: [number(16, names::ITEM.len() as u32) as u16, number(17, names::ITEM.len() as u32) as u16],
            leveling_rate: number(18, names::LEVELING_RATE.len() as u32 - 1) as u8,
            shape: number(19, names::SHAPE.len() as u32 - 1) as u8,
            // The color byte of some species has bits above the color: the full byte passes.
            color: number(20, 255) as u8,
            gender_ratio: number(21, 255) as u8,
            hatch_time: number(22, 255) as u8,
            height: number(23, 65535) as u16,
            weight: number(24, 65535) as u16,
            catch_rate: number(25, 255) as u8,
            base_friendship: number(26, 255) as u8,
            base_experience: number(27, 65535) as u16,
            burrowed_entry: number(28, 1) == 1,
            evs: [
                number(29, 3) as u8,
                number(30, 3) as u8,
                number(31, 3) as u8,
                number(32, 3) as u8,
                number(33, 3) as u8,
                number(34, 3) as u8,
            ],
        };
        if species == 0 {
            errors.push(format!("line {line_number}: the species is 0, the first species is 1"));
        }
        if !seen.insert((species, form)) {
            errors.push(format!("line {line_number}: the species {species} form {form} is in the file two times"));
        }
        rows.push(row);
    }
    if !errors.is_empty() {
        let shown = errors.len().min(10);
        let mut message = errors[..shown].join("\n");
        if errors.len() > shown {
            let _ = write!(message, "\n... and {} more problems", errors.len() - shown);
        }
        return Err(message);
    }
    if rows.is_empty() {
        return Err("The file has no rows.".into());
    }
    Ok(rows)
}

/// Puts the values of a row into the data. The other fields of the data do not change.
fn put(row: &Row, data: &mut SpeciesData) {
    [data.base_hp, data.base_attack, data.base_defense, data.base_special_attack, data.base_special_defense, data.base_speed] = row.stats;
    [data.type_1, data.type_2] = row.types;
    [data.ability_1, data.ability_2, data.ability_hidden] = row.abilities;
    [data.egg_group_1, data.egg_group_2] = row.egg_groups;
    [data.item_50, data.item_5] = row.items;
    data.leveling_rate = row.leveling_rate;
    data.color = row.color;
    data.gender_ratio = row.gender_ratio;
    data.hatch_time = row.hatch_time;
    data.height = row.height;
    data.weight = row.weight;
    data.catch_rate = row.catch_rate;
    data.base_friendship = row.base_friendship;
    data.base_experience = row.base_experience;
    data.set_burrowed_entry(row.burrowed_entry);
    data.set_ev_hp(row.evs[0]);
    data.set_ev_attack(row.evs[1]);
    data.set_ev_defense(row.evs[2]);
    data.set_ev_special_attack(row.evs[3]);
    data.set_ev_special_defense(row.evs[4]);
    data.set_ev_speed(row.evs[5]);
}

/// Applies the rows: the changed species files go on the SD card, the plugin is told,
/// and a changed shape goes into the file of the Pokédex data. All the rows are checked
/// first: a bad row stops the import before anything is written. A shape is ignored
/// when the Pokédex data is not open.
pub fn apply(rows: &[Row], garc: &Garc, sd: &SdCard, pokedex: &mut Option<PokedexData>) -> Result<Report, String> {
    // Check all the rows and collect the changes first.
    let mut changed: Vec<(usize, usize, SpeciesData)> = Vec::new();
    let mut shapes: Vec<(usize, usize, u8)> = Vec::new();
    for row in rows {
        if row.form >= form_count(garc, row.species) {
            return Err(format!("The species {} has no form {}.", row.species, row.form));
        }
        let before = current(garc, Some(sd), row.species, row.form)
            .ok_or_else(|| format!("The species {} form {} has no data.", row.species, row.form))?;
        let mut data = before.clone();
        put(row, &mut data);
        if data != before {
            changed.push((row.species, row.form, data));
        }
        if let Some(shape) = pokedex.as_ref().and_then(|p| p.shape(row.species, row.form)) {
            if shape != row.shape {
                shapes.push((row.species, row.form, row.shape));
            }
        }
    }

    // Then write everything.
    for (species, form, data) in &changed {
        let path = sd_path(*species, *form);
        sd.write(&path, &data.to_bytes())?;
        sd.mark_changed(&path)?;
    }
    let mut shape_written = false;
    if let (false, Some(pokedex)) = (shapes.is_empty(), pokedex.as_mut()) {
        for (species, form, shape) in &shapes {
            pokedex.set_shape(*species, *form, *shape);
        }
        sd.write(pokedex_data::SD_PATH, &pokedex.to_bytes())?;
        sd.mark_changed(pokedex_data::SD_PATH)?;
        shape_written = true;
    }
    Ok(Report { written: changed.len(), shape_written })
}

#[cfg(test)]
mod test {
    use super::*;

    /// One valid row as text. The stats start with `hp`; the EV yields are 1 HP and
    /// 1 Special Attack.
    fn line(species: usize, form: usize, hp: u8) -> String {
        format!("{species},{form},Name,{hp},49,49,65,65,45,11,3,65,65,34,0,1,0,0,3,7,4,31,20,70,69,45,70,64,0,1,0,0,1,0,0")
    }

    #[test]
    fn a_good_file_is_read() {
        let text = format!("{HEADER}\n{}\n\n{}", line(1, 0, 45), line(3, 1, 80));
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].species, rows[0].form, rows[0].stats[0]), (1, 0, 45));
        assert_eq!((rows[1].species, rows[1].form, rows[1].stats[0]), (3, 1, 80));
        assert_eq!(rows[0].types, [11, 3]);
        assert_eq!(rows[0].evs, [1, 0, 0, 1, 0, 0]);
        assert!(!rows[0].burrowed_entry);
    }

    #[test]
    fn a_bad_file_is_refused_with_the_line_and_the_column() {
        assert!(parse("x").unwrap_err().contains("header"));
        assert!(parse(HEADER).unwrap_err().contains("no rows"));
        let text = format!("{HEADER}\n{}", line(1, 0, 45).replace(",45,49,", ",abc,49,"));
        assert!(parse(&text).unwrap_err().contains("line 2: base_hp is not a number"));
        let text = format!("{HEADER}\n{}", line(1, 0, 45).replace(",11,3,", ",18,3,"));
        assert!(parse(&text).unwrap_err().contains("type_1 is 18, the limit is 17"));
        let text = format!("{HEADER}\n{},extra", line(1, 0, 45));
        assert!(parse(&text).unwrap_err().contains("36 fields, not 35"));
        let text = format!("{HEADER}\n{}\n{}", line(1, 0, 45), line(1, 0, 50));
        assert!(parse(&text).unwrap_err().contains("two times"));
        let text = format!("{HEADER}\n{}", line(0, 0, 45));
        assert!(parse(&text).unwrap_err().contains("the first species is 1"));
        let text = format!("{HEADER}\n{}", line(900, 0, 45));
        assert!(parse(&text).unwrap_err().contains("species is 900"));
    }

    #[test]
    fn an_import_row_keeps_the_fields_outside_the_general_tab() {
        let mut data = SpeciesData::from_bytes(&[0; 80]).unwrap();
        data.alternate_form_file = 500;
        data.total_form_count = 3;
        data.tm_hm_moves = [1, 2, 3, 4];
        let text = format!("{HEADER}\n{}", line(1, 0, 45));
        let row = &parse(&text).unwrap()[0];
        put(row, &mut data);
        assert_eq!(data.alternate_form_file, 500);
        assert_eq!(data.total_form_count, 3);
        assert_eq!(data.tm_hm_moves, [1, 2, 3, 4]);
        assert_eq!(data.base_hp, 45);
        assert_eq!(data.ev_hp(), 1);
        assert_eq!(data.ev_special_attack(), 1);
    }

    /// With the dump of the game: an export is read back as the same data, an import
    /// writes only the changed files, and a bad row stops the import.
    #[test]
    fn the_export_and_the_import_give_the_same_data() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let pokedex_garc = Garc::open(&dump.archive(pokedex_data::ARCHIVE)).unwrap();
        let mut pokedex = Some(PokedexData::from_bytes(pokedex_garc.first_file(pokedex_data::FILE).unwrap().to_vec()).unwrap());

        let folder = std::env::temp_dir().join("sango_csv_test");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(&folder.to_string_lossy()).unwrap();

        let text = export(&garc, Some(&sd), pokedex.as_ref());
        let rows = parse(&text).unwrap();
        assert!(rows.len() > SPECIES_COUNT, "only {} rows", rows.len());

        // Nothing differs: the import writes nothing.
        let report = apply(&rows, &garc, &sd, &mut pokedex).unwrap();
        assert_eq!(report, Report { written: 0, shape_written: false });

        // One changed value: the import writes one file and tells the plugin.
        let mut changed = rows.clone();
        changed[0].stats[0] = changed[0].stats[0].wrapping_add(1);
        let report = apply(&changed[..1], &garc, &sd, &mut pokedex).unwrap();
        assert_eq!(report, Report { written: 1, shape_written: false });
        let path = sd_path(changed[0].species, changed[0].form);
        assert!(sd.read(&path).is_some());
        let (_, paths) = sd.reload_state();
        assert!(paths.contains(&path));

        // The new export shows the new value.
        let again = parse(&export(&garc, Some(&sd), pokedex.as_ref())).unwrap();
        assert_eq!(again[0], changed[0]);

        // A changed shape goes into the file of the Pokédex data.
        let mut shaped = again[0].clone();
        shaped.shape = (shaped.shape + 1) % names::SHAPE.len() as u8;
        let report = apply(&[shaped.clone()], &garc, &sd, &mut pokedex).unwrap();
        assert_eq!(report, Report { written: 0, shape_written: true });
        assert!(sd.read(pokedex_data::SD_PATH).is_some());
        assert_eq!(pokedex.as_ref().unwrap().shape(shaped.species, shaped.form), Some(shaped.shape));

        // A form that does not exist stops the import.
        let mut bad = again[0].clone();
        bad.form = 60;
        assert!(apply(&[bad], &garc, &sd, &mut pokedex).unwrap_err().contains("no form 60"));
    }
}
