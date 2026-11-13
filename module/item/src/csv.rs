//! The CSV export and import of the data of the items.
//!
//! One file holds one row per item. The first line is the header: `item`, `name`, then one column for each field of
//! the page (the `key` of [`crate::data::DEFS`]). The `name` is there for the reader, and the import ignores it.
//! The values are plain numbers, as the page shows them: the price is in Poké Dollars (a multiple of 10).
//! The last column is `battle_pocket`: the pockets of the battle Bag as a number from 0 to 15 (one bit for each pocket, see
//! [`crate::pockets`]). The fields that no code reads are not in the file: the import keeps them as they are in the item.
//!
//! The import reads the rows, checks all of them, and then writes the items that changed, or nothing. A file can hold
//! a part of the items.

use crate::data::{ItemData, Kind, COUNT, DEFS};
use sango_core::csv_text::{quote, records};
use sango_core::SdCard;

/// The scale of a field: the CSV has the value times this number.
fn scale(kind: Kind) -> i32 {
    match kind {
        Kind::Number { scale } => scale as i32,
        _ => 1,
    }
}

/// The header line of the file.
pub fn header() -> String {
    let mut line = String::from("item,name");
    for def in &DEFS {
        line.push(',');
        line.push_str(def.key);
    }
    line.push_str(",battle_pocket");
    line
}

/// One row of the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub item: usize,
    /// The value of each field of [`DEFS`], as the CSV shows it.
    pub values: Vec<i32>,
    /// The pockets of the battle Bag (4 bits).
    pub battle_pocket: u8,
}

/// Writes one row per item. `data` gives the data of an item as the user has it, `name` its name.
pub fn export(data: &dyn Fn(usize) -> Option<ItemData>, pockets: &[u8], name: &mut dyn FnMut(usize) -> String) -> String {
    let mut out = header();
    out.push('\n');
    for item in 1..=COUNT {
        let Some(data) = data(item) else { continue };
        out.push_str(&format!("{item},{}", quote(&name(item))));
        for def in &DEFS {
            out.push_str(&format!(",{}", def.get(&data.bytes) * scale(def.kind)));
        }
        out.push_str(&format!(",{}", pockets.get(item).copied().unwrap_or(0) & 15));
        out.push('\n');
    }
    out
}

/// Reads and checks the CSV text. Nothing is applied yet.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let records = records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    match records.first() {
        Some((_, first)) if first.join(",") == header() => {}
        _ => return Err("The first line is not the header of an items CSV file.".into()),
    }
    let mut rows = Vec::new();
    for (line, fields) in records.iter().skip(1) {
        if fields.len() == 1 && fields[0].is_empty() {
            continue;
        }
        if fields.len() != DEFS.len() + 3 {
            return Err(format!("Line {line}: {} columns instead of {}.", fields.len(), DEFS.len() + 3));
        }
        let item = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}, column item: \"{}\" is not a number.", fields[0]))?;
        if !(1..=COUNT).contains(&item) {
            return Err(format!("Line {line}, column item: {item} is not between 1 and {COUNT}."));
        }
        let mut values = Vec::new();
        for (place, def) in DEFS.iter().enumerate() {
            let text = fields[place + 2].trim();
            let value = text.parse::<i32>().map_err(|_| format!("Line {line}, column {}: \"{text}\" is not a number.", def.key))?;
            let factor = scale(def.kind);
            if value % factor != 0 {
                return Err(format!("Line {line}, column {}: {value} must be a multiple of {factor}.", def.key));
            }
            let raw = value / factor;
            if raw < def.min() || raw > def.max() {
                return Err(format!("Line {line}, column {}: {value} is not between {} and {}.", def.key, def.min() * factor, def.max() * factor));
            }
            values.push(value);
        }
        let text = fields[DEFS.len() + 2].trim();
        let battle_pocket = text.parse::<u8>().ok().filter(|v| *v <= 15).ok_or_else(|| format!("Line {line}, column battle_pocket: \"{text}\" is not between 0 and 15."))?;
        rows.push(Row { item, values, battle_pocket });
    }
    Ok(rows)
}

/// Writes the files of the items that the rows change, on the SD card, and the table of the battle pockets when it
/// changes. `pockets` is the table now. Gives the number of files that were written.
pub fn apply(rows: &[Row], data: &dyn Fn(usize) -> Option<ItemData>, pockets: &[u8], sd: &SdCard) -> Result<usize, String> {
    let mut written = 0;
    let mut table = pockets.to_vec();
    for row in rows {
        let current = data(row.item).ok_or_else(|| format!("The file of the item {} is missing.", row.item))?;
        if let Some(byte) = table.get_mut(row.item) {
            *byte = row.battle_pocket;
        }
        let mut item = current.clone();
        for (def, value) in DEFS.iter().zip(&row.values) {
            def.set(&mut item.bytes, value / scale(def.kind));
        }
        if item == current {
            continue;
        }
        let path = crate::data::sd_path(row.item);
        sd.write(&path, &item.to_bytes()).and_then(|_| sd.mark_changed(&path))?;
        written += 1;
    }
    if table != pockets {
        crate::pockets::write(sd, &table)?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod test {
    use super::*;

    fn row(item: usize) -> String {
        let mut line = format!("{item},Test");
        for _ in &DEFS {
            line.push_str(",0");
        }
        line.push_str(",0");
        line
    }

    #[test]
    fn a_good_file_is_read_and_a_bad_file_is_refused_with_its_place() {
        let text = format!("{}\n{}\n", header(), row(3));
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].values.len(), DEFS.len());
        assert!(parse("item,name\n").is_err());
        let bad = format!("{}\n{}\n", header(), row(776));
        assert!(parse(&bad).unwrap_err().starts_with("Line 2, column item"));
        // The price is a multiple of 10, and the stat stages stay in 4 bits.
        let bad = format!("{}\n{}\n", header(), row(3).replacen(",0", ",15", 1));
        assert!(parse(&bad).unwrap_err().contains("column price"));
        let place = DEFS.iter().position(|d| d.key == "raise_attack").unwrap();
        let mut cells: Vec<String> = row(3).split(',').map(|c| c.to_string()).collect();
        cells[place + 2] = "16".into();
        let bad = format!("{}\n{}\n", header(), cells.join(","));
        assert!(parse(&bad).unwrap_err().contains("column raise_attack"));
    }

    /// With the dump of the game: the items go out and come back, one change is written, and the fields that the page
    /// does not show are kept.
    #[test]
    fn the_items_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(crate::data::ARCHIVE)).unwrap();
        let folder = std::env::temp_dir().join(format!("sango_item_csv_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let data = |item: usize| {
            let on_card = sd.read(&crate::data::sd_path(item)).and_then(|b| ItemData::from_bytes(&b).ok());
            on_card.or_else(|| garc.first_file(item).and_then(|b| ItemData::from_bytes(b).ok()))
        };
        let mut name = |item: usize| format!("Item {item}");
        let pockets = crate::pockets::current(Some(&sango_core::Garc::open(&dump.archive(crate::pockets::ARCHIVE)).unwrap()), Some(&sd));
        let text = export(&data, &pockets, &mut name);
        assert_eq!(text.lines().count(), COUNT + 1);
        let rows = parse(&text).unwrap();
        assert_eq!(rows.len(), COUNT);
        assert_eq!(apply(&rows, &data, &pockets, &sd), Ok(0), "the items as they are change nothing");
        let potion_pocket = rows.iter().find(|r| r.item == 17).unwrap().battle_pocket;
        assert_eq!(potion_pocket, 4, "the Potion is in the HP/PP recovery pocket");

        // The Potion costs 300, restores 20 HP, and is in the Medicine pocket.
        let potion = rows.iter().find(|r| r.item == 17).unwrap();
        let value = |key: &str| potion.values[DEFS.iter().position(|d| d.key == key).unwrap()];
        assert_eq!((value("price"), value("hp_restore"), value("pocket"), value("restore_hp")), (300, 20, 1, 1));

        // One price is changed: one file is written, and the bytes that the CSV does not hold stay.
        let mut changed = rows.clone();
        let at = DEFS.iter().position(|d| d.key == "price").unwrap();
        changed[16].values[at] = 150;
        assert_eq!(apply(&changed, &data, &pockets, &sd), Ok(1));
        // A pocket changes the table of the battle pockets only: the item file is already written.
        changed[16].battle_pocket = 5;
        assert_eq!(apply(&changed, &data, &pockets, &sd), Ok(1));
        assert_eq!(sd.read(crate::pockets::SD_PATH).unwrap()[17], 5);
        let written = ItemData::from_bytes(&sd.read(&crate::data::sd_path(17)).unwrap()).unwrap();
        let original = ItemData::from_bytes(garc.first_file(17).unwrap()).unwrap();
        assert_eq!(written.get("price"), Some(15));
        let mut same = written.clone();
        same.set("price", original.get("price").unwrap());
        assert_eq!(same, original);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
