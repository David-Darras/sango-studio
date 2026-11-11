//! The CSV export and import of the evolutions.
//!
//! One file holds one row for each species and form, with its 8 evolution slots. It follows the layout of the
//! species file (see the species CSV module): a header, plain decimal numbers, commas between the fields,
//! no quotes. The import reads and checks the whole file, then writes the changes, or writes nothing.
//!
//! | Column | Content | Limits |
//! | --- | --- | --- |
//! | `species` | The number of the species. | 1 to 721 |
//! | `form` | The number of the form. 0 is the normal form. | the forms of the species |
//! | `name` | The name of the species. The import ignores it. | text without a comma |
//! | `evolutions` | The names of the species that it evolves into, with `;` between them. The import ignores it. | text without a comma |
//! | `method_1` ... `method_8` | How it evolves. 0 is an empty slot. The numbers are those of `sango_core::evolution::METHODS`. | 0 to 34 |
//! | `argument_1` ... `argument_8` | What the method needs: a level, an item, a move, a species, a type or a Beauty. 0 when it needs nothing. | see below |
//! | `evolves_into_1` ... `evolves_into_8` | The species that it evolves into. 0 for an empty slot. | 0 to 721 |
//!
//! The limits of the argument depend on the method: a level 1 to 100, an item 1 to 775, a move 1 to 621,
//! a species 1 to 721, a type 0 to 17, a Beauty 0 to 255, and 0 for a method that needs nothing.
//! The used slots come first, then the empty ones.

use crate::move_names::MOVE;
use crate::{file_index, names, SPECIES_COUNT};
use crate::data::SpeciesData;
use sango_core::evolution::{self, argument_of, Argument, Evolution, Evolutions, METHODS, SLOTS};
use sango_core::{Garc, SdCard};
use std::fmt::Write as _;

/// The header line of the file.
pub fn header() -> String {
    let mut line = String::from("species,form,name,evolutions");
    for slot in 1..=SLOTS {
        let _ = write!(line, ",method_{slot},argument_{slot},evolves_into_{slot}");
    }
    line
}

/// One row of the file: a species, a form, and its evolutions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub species: usize,
    pub form: usize,
    pub evolutions: Evolutions,
}

/// The archives that the evolutions come from, and the SD card that has the changes.
pub struct Library<'a> {
    /// The species data (archive 195).
    pub species: &'a Garc,
    /// The evolutions (archive 192).
    pub evolutions: &'a Garc,
    pub sd: Option<&'a SdCard>,
}

impl Library<'_> {
    fn form_count(&self, species: usize) -> usize {
        self.species.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |d| (d.total_form_count as usize).max(1))
    }

    /// The evolutions of a species and a form: the SD card first, then the romfs.
    fn current(&self, species: usize, form: usize) -> Option<Evolutions> {
        let on_card = self.sd.and_then(|sd| sd.read(&evolution::sd_path(species, form))).and_then(|b| Evolutions::from_bytes(&b).ok());
        on_card.or_else(|| self.evolutions.first_file(file_index(self.species, species, form)).and_then(|b| Evolutions::from_bytes(b).ok()))
    }
}

/// Writes one row for each species and form, from the current data (the SD card first, then the romfs).
pub fn export(library: &Library) -> String {
    let mut out = header();
    out.push('\n');
    for species in 1..=SPECIES_COUNT {
        for form in 0..library.form_count(species) {
            let Some(evolutions) = library.current(species, form) else { continue };
            let into: Vec<&str> = evolutions.list.iter().map(|e| names::SPECIES.get((e.species as usize).wrapping_sub(1)).copied().unwrap_or("-")).collect();
            let _ = write!(out, "{species},{form},{},{}", names::SPECIES[species - 1], into.join(";"));
            for slot in 0..SLOTS {
                let e = evolutions.list.get(slot).copied().unwrap_or_default();
                let _ = write!(out, ",{},{},{}", e.method, e.argument, e.species);
            }
            out.push('\n');
        }
    }
    out
}

/// The limits of the argument of a method.
fn argument_limits(kind: Argument) -> (u32, u32) {
    match kind {
        Argument::None => (0, 0),
        Argument::Level => (1, 100),
        Argument::Item => (1, names::ITEM.len() as u32),
        Argument::Move => (1, MOVE.len() as u32),
        Argument::Species => (1, SPECIES_COUNT as u32),
        Argument::Type => (0, names::TYPE.len() as u32 - 1),
        Argument::Beauty => (0, 255),
    }
}

/// Reads and checks the text of a file. Nothing is applied yet. The result holds every row, or the problems.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let header = header();
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, first)) if first.trim_end() == header => {}
        _ => return Err("The first line is not the header of an evolution CSV file.".into()),
    }
    let columns = header.split(',').count();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (place, line) in lines {
        let number = place + 1;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != columns {
            errors.push(format!("line {number}: {} fields, not {columns}", fields.len()));
            continue;
        }
        let problems_before = errors.len();
        let mut field = |column: usize, limit: u32, what: String| -> u32 {
            match fields[column].parse::<u32>() {
                Ok(value) if value <= limit => value,
                Ok(value) => {
                    errors.push(format!("line {number}: {what} is {value}, the limit is {limit}"));
                    0
                }
                Err(_) => {
                    errors.push(format!("line {number}: {what} is not a number"));
                    0
                }
            }
        };
        let species = field(0, SPECIES_COUNT as u32, "species".into()) as usize;
        let form = field(1, 255, "form".into()) as usize;
        let mut slots = Vec::new();
        for slot in 0..SLOTS {
            let at = 4 + 3 * slot;
            let method = field(at, METHODS.len() as u32, format!("method_{}", slot + 1)) as u16;
            let argument = field(at + 1, 65535, format!("argument_{}", slot + 1)) as u16;
            let into = field(at + 2, SPECIES_COUNT as u32, format!("evolves_into_{}", slot + 1)) as u16;
            slots.push(Evolution { method, argument, species: into });
        }
        if errors.len() == problems_before {
            let mut ended = false;
            for (place, e) in slots.iter().enumerate() {
                let n = place + 1;
                if e.method == 0 {
                    if e.argument != 0 || e.species != 0 {
                        errors.push(format!("line {number}: the slot {n} has no method but it has an argument or a species"));
                    }
                    ended = true;
                    continue;
                }
                if ended {
                    errors.push(format!("line {number}: the slot {n} is used after an empty slot"));
                }
                if e.species == 0 {
                    errors.push(format!("line {number}: the slot {n} has a method but no species"));
                }
                let (low, high) = argument_limits(argument_of(e.method));
                if (e.argument as u32) < low || (e.argument as u32) > high {
                    errors.push(format!("line {number}: argument_{n} is {}, it must be from {low} to {high} for this method", e.argument));
                }
            }
        }
        if species == 0 && errors.len() == problems_before {
            errors.push(format!("line {number}: the species is 0, the first species is 1"));
        }
        if !seen.insert((species, form)) {
            errors.push(format!("line {number}: the species {species} form {form} is in the file two times"));
        }
        let list: Vec<Evolution> = slots.into_iter().filter(|e| e.method != 0).collect();
        rows.push(Row { species, form, evolutions: Evolutions { list } });
    }
    if errors.is_empty() && rows.is_empty() {
        errors.push("The file has no rows.".into());
    }
    if !errors.is_empty() {
        let shown = errors.len().min(10);
        let mut message = errors[..shown].join("\n");
        if errors.len() > shown {
            let _ = write!(message, "\n... and {} more problems", errors.len() - shown);
        }
        return Err(message);
    }
    Ok(rows)
}

/// Checks the rows against the game, then writes each file that changes. Returns the number of written files.
pub fn apply(rows: &[Row], library: &Library, sd: &SdCard) -> Result<usize, String> {
    let errors: Vec<String> = rows.iter().filter(|row| row.form >= library.form_count(row.species)).map(|row| format!("the species {} has no form {}", row.species, row.form)).collect();
    if !errors.is_empty() {
        return Err(errors[..errors.len().min(10)].join("\n"));
    }
    let mut written = 0;
    for row in rows {
        if library.current(row.species, row.form).as_ref() == Some(&row.evolutions) {
            continue;
        }
        let path = evolution::sd_path(row.species, row.form);
        sd.write(&path, &row.evolutions.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod test {
    use super::*;

    fn good_row(parts: &[&str]) -> String {
        let mut fields = vec!["1", "0", "Bulbasaur", "Ivysaur"];
        fields.extend(parts);
        while fields.len() < 28 {
            fields.push("0");
        }
        fields.join(",")
    }

    #[test]
    fn a_good_file_is_read() {
        let text = format!("{}\n{}\n", header(), good_row(&["4", "16", "2"]));
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].evolutions.list, [Evolution { method: 4, argument: 16, species: 2 }]);
        assert!(parse(&text.replace('\n', "\r\n")).is_ok());
        assert_eq!(header().split(',').count(), 28);
    }

    #[test]
    fn a_wrong_file_is_refused_with_its_line() {
        let head = format!("{}\n", header());
        let error = |row: String| parse(&format!("{head}{row}\n")).unwrap_err();
        assert!(parse("species,form\n").unwrap_err().contains("header"));
        assert!(parse(&head).unwrap_err().contains("no rows"));
        assert!(error("1,0,x".into()).contains("line 2: 3 fields"));
        assert!(error(good_row(&["99", "0", "2"])).contains("method_1 is 99"));
        assert!(error(good_row(&["4", "x", "2"])).contains("argument_1 is not a number"));
        assert!(error(good_row(&["4", "16", "999"])).contains("evolves_into_1 is 999"));
        // The argument must fit the method: a level to 100, nothing for a trade, a type to 17.
        assert!(error(good_row(&["4", "0", "2"])).contains("argument_1 is 0, it must be from 1 to 100"));
        assert!(error(good_row(&["4", "101", "2"])).contains("from 1 to 100"));
        assert!(error(good_row(&["5", "3", "2"])).contains("from 0 to 0"));
        assert!(error(good_row(&["29", "18", "2"])).contains("from 0 to 17"));
        // A used slot needs a species, an empty slot has nothing, and the used slots come first.
        assert!(error(good_row(&["4", "16", "0"])).contains("has a method but no species"));
        assert!(error(good_row(&["0", "0", "0", "4", "16", "2"])).contains("slot 2 is used after an empty slot"));
        assert!(error(good_row(&["0", "5", "0"])).contains("has no method but it has an argument"));
        let twice = format!("{head}{}\n{}\n", good_row(&["4", "16", "2"]), good_row(&["4", "16", "2"]));
        assert!(parse(&twice).unwrap_err().contains("two times"));
    }

    /// With the dump of the game: the file goes out and comes back, a change is written, and Eevee has its
    /// eight evolutions.
    #[test]
    fn the_evolutions_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let open = |n: usize| Garc::open(&dump.archive(n)).unwrap();
        let (species, evolutions) = (open(crate::ARCHIVE), open(evolution::ARCHIVE));
        let folder = std::env::temp_dir().join(format!("sango_evolution_csv_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let library = Library { species: &species, evolutions: &evolutions, sd: Some(&sd) };

        let text = export(&library);
        assert!(text.lines().any(|l| l.starts_with("1,0,Bulbasaur,Ivysaur,4,16,2,0,0,0")));
        assert!(text.lines().any(|l| l.starts_with("133,0,Eevee,Leafeon;Glaceon;Jolteon;Vaporeon;Flareon;Sylveon;Espeon;Umbreon,26,0,470")));
        let rows = parse(&text).unwrap();
        assert_eq!(apply(&rows, &library, &sd).unwrap(), 0, "the same file gives no change");

        // Ivysaur evolves at the level 40, and Venusaur gets an evolution.
        let edited = text.replace("\n2,0,Ivysaur,Venusaur,4,32,3", "\n2,0,Ivysaur,Venusaur,4,40,3").replace("\n3,0,Venusaur,,0,0,0", "\n3,0,Venusaur,Bulbasaur,8,82,1");
        assert_ne!(edited, text);
        assert_eq!(apply(&parse(&edited).unwrap(), &library, &sd).unwrap(), 2);
        let changed = export(&library);
        assert!(changed.contains("\n2,0,Ivysaur,Venusaur,4,40,3,"));
        assert!(changed.contains("\n3,0,Venusaur,Bulbasaur,8,82,1,"));
        assert_eq!(text.lines().filter(|l| l.starts_with("133,")).collect::<Vec<_>>(), changed.lines().filter(|l| l.starts_with("133,")).collect::<Vec<_>>());

        // Nothing is written when a row is wrong.
        let bad = format!("{edited}{}\n", good_row(&["4", "16", "2"]).replacen("1,0,", "3,5,", 1));
        let before = export(&library);
        assert!(apply(&parse(&bad).unwrap(), &library, &sd).unwrap_err().contains("no form 5"));
        assert_eq!(export(&library), before);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
