//! The CSV export and import of the experience tables.
//!
//! One file holds one row per level and one column per Leveling rate. The value is the experience that a
//! Pokémon has when it reaches the level: the same numbers as the tables of the game.
//! The layout follows the model of the species file (see the species module):
//!
//! - The first line is the header: `level`, then the rate columns that the file has.
//! - Each other line is one level. The fields are plain decimal numbers with commas between them, without quotes.
//! - The import reads and checks the whole file first. It changes nothing when a line is wrong.
//!
//! | Column | Content | Limits |
//! | --- | --- | --- |
//! | `level` | The level. Each level from 1 to 100 appears once, in any order. | 1 to 100 |
//! | `medium_fast`, `erratic`, `fluctuating`, `medium_slow`, `fast`, `slow` | The experience at the level, for this Leveling rate. | 0 to 9,999,999 |
//!
//! The level 1 must have 0 experience, and the numbers of a column must not go down from one level to the next.
//! A file can hold only some of the rate columns: the other rates do not change. The two unused rates of the
//! game (6 and 7) are not in the file.

use sango_core::experience::{Curve, ExperienceTable, MAX_LEVEL};
use std::fmt::Write as _;

/// The most experience that a table can have at one level.
pub const MAX_EXPERIENCE: u32 = 9_999_999;

/// The names of the rate columns, in the order of the Leveling rate numbers 0 to 5.
pub const COLUMNS: [&str; 6] = ["medium_fast", "erratic", "fluctuating", "medium_slow", "fast", "slow"];

/// The header line of a file that has all the rates.
pub const HEADER: &str = "level,medium_fast,erratic,fluctuating,medium_slow,fast,slow";

/// Writes the tables of the six real Leveling rates as the text of a file.
pub fn export(table: &ExperienceTable) -> String {
    let mut text = String::from(HEADER);
    text.push('\n');
    for level in 1..=MAX_LEVEL {
        let _ = write!(text, "{level}");
        for rate in 0..COLUMNS.len() {
            let value = table.curve(rate).and_then(|curve| curve.experience(level)).unwrap_or(0);
            let _ = write!(text, ",{value}");
        }
        text.push('\n');
    }
    text
}

/// Reads the text of a file. Gives the new table of each rate that the file has, as (rate number, table).
///
/// Every line is checked. The error names the line, or the column.
pub fn parse(text: &str) -> Result<Vec<(usize, Curve)>, String> {
    let mut lines = text.lines().map(str::trim).enumerate().filter(|(_, line)| !line.is_empty());
    let (_, header) = lines.next().ok_or("The file is empty.")?;
    let names: Vec<&str> = header.split(',').map(str::trim).collect();
    if names.first() != Some(&"level") {
        return Err("Line 1: the first column must be `level`.".into());
    }
    // The rate of each column after `level`.
    let mut rates = Vec::new();
    for name in &names[1..] {
        let rate = COLUMNS.iter().position(|column| column == name).ok_or_else(|| format!("Line 1: unknown column `{name}`."))?;
        if rates.contains(&rate) {
            return Err(format!("Line 1: the column `{name}` appears twice."));
        }
        rates.push(rate);
    }
    if rates.is_empty() {
        return Err("Line 1: the file has no rate column.".into());
    }

    // values[column][level]
    let mut values = vec![[None::<u32>; MAX_LEVEL + 1]; rates.len()];
    for (index, line) in lines {
        let number = index + 1;
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != names.len() {
            return Err(format!("Line {number}: {} fields, the header has {}.", fields.len(), names.len()));
        }
        let level: usize = fields[0].parse().map_err(|_| format!("Line {number}: `{}` is not a level.", fields[0]))?;
        if !(1..=MAX_LEVEL).contains(&level) {
            return Err(format!("Line {number}: the level {level} is not between 1 and {MAX_LEVEL}."));
        }
        for (column, field) in fields[1..].iter().enumerate() {
            let value: u32 = field.parse().map_err(|_| format!("Line {number}: `{field}` is not a whole number."))?;
            if value > MAX_EXPERIENCE {
                return Err(format!("Line {number}: {value} is above {MAX_EXPERIENCE}."));
            }
            if values[column][level].replace(value).is_some() {
                return Err(format!("Line {number}: the level {level} appears twice."));
            }
        }
    }

    let mut tables = Vec::new();
    for (column, &rate) in rates.iter().enumerate() {
        let name = COLUMNS[rate];
        let mut levels = [0u32; MAX_LEVEL + 1];
        for level in 1..=MAX_LEVEL {
            levels[level] = values[column][level].ok_or_else(|| format!("Column `{name}`: the level {level} is missing."))?;
        }
        if levels[1] != 0 {
            return Err(format!("Column `{name}`: the level 1 must have 0 experience, not {}.", levels[1]));
        }
        if let Some(level) = (2..=MAX_LEVEL).find(|&level| levels[level] < levels[level - 1]) {
            return Err(format!("Column `{name}`: the experience goes down at the level {level} ({} after {}).", levels[level], levels[level - 1]));
        }
        let bytes: Vec<u8> = levels.iter().flat_map(|value| value.to_le_bytes()).collect();
        tables.push((rate, Curve::from_bytes(&bytes)?));
    }
    Ok(tables)
}

#[cfg(test)]
mod test {
    use super::*;

    /// A table where each rate is a copy of the same curve: the level n has n * n minus 1 (and 0 at the level 1).
    fn sample() -> ExperienceTable {
        let bytes: Vec<u8> = (0..=MAX_LEVEL as u32).flat_map(|n| (n.saturating_sub(1) * n.saturating_sub(1)).to_le_bytes()).collect();
        let curve = Curve::from_bytes(&bytes).unwrap();
        let mut table = ExperienceTable::from_curves(vec![curve; 8]).unwrap();
        // Make the rates different: the rate 4 is twice the rate 0.
        let doubled: Vec<u8> = (0..=MAX_LEVEL as u32).flat_map(|n| (2 * n.saturating_sub(1) * n.saturating_sub(1)).to_le_bytes()).collect();
        table.set_curve(4, Curve::from_bytes(&doubled).unwrap());
        table
    }

    #[test]
    fn the_export_and_the_import_give_the_same_tables() {
        let table = sample();
        let text = export(&table);
        assert_eq!(text.lines().count(), 101);
        assert_eq!(text.lines().next(), Some(HEADER));
        assert_eq!(text.lines().nth(1), Some("1,0,0,0,0,0,0"));
        assert_eq!(text.lines().nth(3), Some("3,4,4,4,4,8,4"));
        let tables = parse(&text).unwrap();
        assert_eq!(tables.len(), 6);
        for (rate, curve) in tables {
            assert_eq!(Some(&curve), table.curve(rate), "rate {rate}");
        }
    }

    #[test]
    fn a_file_can_hold_some_rates_in_any_order() {
        let text = "level,slow,erratic\n2,10,5\n1,0,0\n3,20,15\n".to_string() + &(4..=100).map(|n| format!("{n},{},{}\n", 100 + n, 50 + n)).collect::<String>();
        let tables = parse(&text).unwrap();
        assert_eq!(tables.iter().map(|(rate, _)| *rate).collect::<Vec<_>>(), [5, 1]);
        assert_eq!(tables[0].1.experience(2), Some(10));
        assert_eq!(tables[1].1.experience(3), Some(15));
    }

    #[test]
    fn a_wrong_file_is_refused_with_its_line() {
        let good = export(&sample());
        let change = |from: &str, to: &str| good.replacen(from, to, 1);
        let error = |text: String| parse(&text).unwrap_err();
        assert!(error(String::new()).contains("empty"));
        assert!(error(change("level,", "lvl,")).contains("first column"));
        assert!(error(change("erratic", "other")).contains("unknown column `other`"));
        assert!(error(change("erratic", "fast")).contains("twice"));
        assert!(error("level\n1\n".into()).contains("no rate column"));
        assert!(error(change("3,4,4,4,4,8,4", "3,4,4,4,4,8")).contains("Line 4"));
        assert!(error(change("3,4,4,4,4,8,4", "3,4,x,4,4,8,4")).contains("`x` is not a whole number"));
        assert!(error(change("3,4,4,4,4,8,4", "300,4,4,4,4,8,4")).contains("level 300"));
        assert!(error(change("3,4,4,4,4,8,4", "2,4,4,4,4,8,4")).contains("level 2 appears twice"));
        assert!(error(change("3,4,4,4,4,8,4", "3,4,4,4,4,8,99999999")).contains("above"));
        // The level 1 has experience: refused. The experience goes down: refused.
        assert!(error(change("1,0,0,0,0,0,0", "1,5,0,0,0,0,0")).contains("`medium_fast`: the level 1 must have 0"));
        assert!(error(change("3,4,4,4,4,8,4", "3,0,4,4,4,8,4")).contains("`medium_fast`: the experience goes down at the level 3"));
        // A level that is missing.
        let without_level_7: String = good.lines().filter(|line| !line.starts_with("7,")).map(|line| format!("{line}\n")).collect();
        assert!(error(without_level_7).contains("the level 7 is missing"));
    }

    #[test]
    fn windows_line_ends_and_empty_lines_are_accepted() {
        let text = export(&sample()).replace('\n', "\r\n") + "\r\n\r\n";
        assert_eq!(parse(&text).unwrap().len(), 6);
    }

    /// With the dump of the game: the six tables of the game go out and come back the same.
    #[test]
    fn the_tables_of_the_dump_go_out_and_come_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(sango_core::experience::ARCHIVE)).unwrap();
        let table = ExperienceTable::from_garc(&garc).unwrap();
        let text = export(&table);
        // Bulbapedia: at the level 100, Medium Slow has 1,059,860 and Erratic 600,000.
        assert!(text.lines().last().unwrap().starts_with("100,1000000,600000,1640000,1059860,800000,1250000"));
        for (rate, curve) in parse(&text).unwrap() {
            assert_eq!(Some(&curve), table.curve(rate), "rate {rate}");
        }
    }
}
