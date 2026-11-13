//! The CSV export and import of the data of the moves.
//!
//! One file holds one row per move. The first line is the header: `move`, `name`, then one column for each field of the
//! page (the `key` of [`crate::data::DEFS`]). The `name` is there for the reader, and the import ignores it. The values are
//! plain numbers, as the page shows them (a negative priority, stages or percent is a negative number). The fields that no
//! code reads are not in the file: the import keeps them as they are in the move.
//!
//! The import reads the rows, checks all of them, and then writes the moves that changed, or nothing. A file can hold a
//! part of the moves.

use crate::data::{MoveData, COUNT, DEFS};
use sango_core::csv_text::{quote, records};
use sango_core::SdCard;

/// The header line of the file.
pub fn header() -> String {
    let mut line = String::from("move,name");
    for def in &DEFS {
        line.push(',');
        line.push_str(def.key);
    }
    line
}

/// One row of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: usize,
    /// The value of each field of [`DEFS`].
    pub values: Vec<i32>,
}

/// Writes one row per move. `data` gives the data of a move as the user has it, `name` its name.
pub fn export(data: &dyn Fn(usize) -> Option<MoveData>, name: &mut dyn FnMut(usize) -> String) -> String {
    let mut out = header();
    out.push('\n');
    for id in 1..=COUNT {
        let Some(data) = data(id) else { continue };
        out.push_str(&format!("{id},{}", quote(&name(id))));
        for def in &DEFS {
            out.push_str(&format!(",{}", def.get(&data.bytes)));
        }
        out.push('\n');
    }
    out
}

/// Reads and checks the CSV text. Nothing is applied yet.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    match records.first() {
        Some((_, first)) if first.join(",") == header() => {}
        _ => return Err("The first line is not the header of a moves CSV file.".into()),
    }
    let mut rows = Vec::new();
    for (line, fields) in records.iter().skip(1) {
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        if fields.len() != DEFS.len() + 2 {
            return Err(format!("Line {line}: {} columns instead of {}.", fields.len(), DEFS.len() + 2));
        }
        let id = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}, column move: \"{}\" is not a number.", fields[0]))?;
        if !(1..=COUNT).contains(&id) {
            return Err(format!("Line {line}, column move: {id} is not between 1 and {COUNT}."));
        }
        let mut values = Vec::new();
        for (place, def) in DEFS.iter().enumerate() {
            let text = fields[place + 2].trim();
            let value = text.parse::<i32>().map_err(|_| format!("Line {line}, column {}: \"{text}\" is not a number.", def.key))?;
            if value < def.min() || value > def.max() {
                return Err(format!("Line {line}, column {}: {value} is not between {} and {}.", def.key, def.min(), def.max()));
            }
            values.push(value);
        }
        rows.push(Row { id, values });
    }
    Ok(rows)
}

/// Writes the files of the moves that the rows change, on the SD card. Gives the number of files that were written.
pub fn apply(rows: &[Row], data: &dyn Fn(usize) -> Option<MoveData>, sd: &SdCard) -> Result<usize, String> {
    let mut written = 0;
    for row in rows {
        let current = data(row.id).ok_or_else(|| format!("The data of the move {} is missing.", row.id))?;
        let mut changed = current.clone();
        for (def, value) in DEFS.iter().zip(&row.values) {
            def.set(&mut changed.bytes, *value);
        }
        if changed == current {
            continue;
        }
        let path = crate::data::sd_path(row.id);
        sd.write(&path, &changed.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod test {
    use super::*;

    fn row(id: usize) -> String {
        let mut line = format!("{id},Test");
        for _ in &DEFS {
            line.push_str(",0");
        }
        line
    }

    #[test]
    fn a_good_file_is_read_and_a_bad_file_is_refused_with_its_place() {
        let text = format!("{}\n{}\n", header(), row(3));
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].values.len(), DEFS.len());
        assert!(parse("move,name\n").is_err());
        let bad = format!("{}\n{}\n", header(), row(622));
        assert!(parse(&bad).unwrap_err().starts_with("Line 2, column move"));
        // The stages are signed 8 bits, the max hits 4 bits.
        let place = DEFS.iter().position(|d| d.key == "max_hits").unwrap();
        let mut cells: Vec<String> = row(3).split(',').map(|c| c.to_string()).collect();
        cells[place + 2] = "16".into();
        let bad = format!("{}\n{}\n", header(), cells.join(","));
        assert!(parse(&bad).unwrap_err().contains("column max_hits"));
        cells[place + 2] = "x".into();
        let bad = format!("{}\n{}\n", header(), cells.join(","));
        assert!(parse(&bad).unwrap_err().contains("is not a number"));
    }

    /// With the dump of the game: the moves go out and come back, one change is written, and the fields that the page
    /// does not show are kept.
    #[test]
    fn the_moves_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(crate::data::ARCHIVE)).unwrap();
        let bundle = sango_core::Bundle::parse(garc.first_file(0).unwrap().to_vec()).unwrap();
        let folder = std::env::temp_dir().join(format!("sango_move_csv_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let data = |id: usize| {
            let on_card = sd.read(&crate::data::sd_path(id)).and_then(|b| MoveData::from_bytes(&b).ok());
            on_card.or_else(|| bundle.resource(id).and_then(|b| MoveData::from_bytes(b).ok()))
        };
        let mut name = |id: usize| format!("Move {id}");
        let text = export(&data, &mut name);
        assert_eq!(text.lines().count(), COUNT + 1);
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), COUNT);
        assert_eq!(apply(&rows, &data, &sd), Ok(0), "the moves as they are change nothing");

        // Pound: Normal type, physical, power 40, accuracy 100, 35 PP, with contact.
        let value = |row: &Row, key: &str| row.values[DEFS.iter().position(|d| d.key == key).unwrap()];
        let pound = &rows[0];
        assert_eq!((value(pound, "type"), value(pound, "damage_kind"), value(pound, "power"), value(pound, "accuracy"), value(pound, "pp"), value(pound, "flag_contact")), (0, 1, 40, 100, 35, 1));

        // One power is changed: one file is written, and the bytes that the CSV does not hold stay.
        let mut changed = rows.clone();
        let at = DEFS.iter().position(|d| d.key == "power").unwrap();
        changed[0].values[at] = 90;
        assert_eq!(apply(&changed, &data, &sd), Ok(1));
        let written = MoveData::from_bytes(&sd.read(&crate::data::sd_path(1)).unwrap()).unwrap();
        let original = MoveData::from_bytes(bundle.resource(1).unwrap()).unwrap();
        assert_eq!(written.get("power"), Some(90));
        let mut same = written.clone();
        same.set("power", original.get("power").unwrap());
        assert_eq!(same, original);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
