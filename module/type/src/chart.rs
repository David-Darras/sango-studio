//! The type chart: how much damage each attacking type does to each defending type.
//!
//! The game keeps the type chart in its code, not in a file of the romfs, so this table is the chart of Omega Ruby and
//! Alpha Sapphire (the chart of the sixth generation) and the changes of the user are the differences from it.
//!
//! The types have the numbers 0 (Normal) to 17 (Fairy). The number 18 is "no type". A new type has a number from 19 to 31.
//! A cell holds the number that the game keeps in its chart: 0 for no effect, 2 for not very effective (×½), 4 for normal
//! (×1) and 8 for super effective (×2).
//!
//! The changed chart goes on the SD card as one file:
//!
//! | Bytes | Content |
//! | --- | --- |
//! | 0-3 | The signature `TCHT`. |
//! | 4-7 | The version, 1 (little-endian). |
//! | 8-11 | The number of types of a side, 32. |
//! | 12-1035 | The 32 × 32 cells, one row for each attacking type. |

/// The number of types of the game: Normal to Fairy.
pub const GAME_TYPES: usize = 18;

/// The number that means "no type".
pub const NO_TYPE: usize = 18;

/// The number of the first new type.
pub const FIRST_CUSTOM: usize = 19;

/// The number of the last new type, and the number of types that a side of the chart has.
pub const MAX_TYPES: usize = 32;

/// The most new types.
pub const MAX_CUSTOM: usize = MAX_TYPES - FIRST_CUSTOM;

/// No effect.
pub const NONE: u8 = 0;
/// Not very effective (×½).
pub const HALF: u8 = 2;
/// Normal (×1).
pub const NORMAL: u8 = 4;
/// Super effective (×2).
pub const DOUBLE: u8 = 8;

/// The values of a cell, in the order of a click.
pub const CYCLE: [u8; 4] = [NORMAL, DOUBLE, HALF, NONE];

/// The text of a value, as the games write it.
pub fn text(value: u8) -> &'static str {
    match value {
        NONE => "×0",
        HALF => "×½",
        DOUBLE => "×2",
        _ => "×1",
    }
}

/// The value that follows (or precedes) another in the order of a click.
pub fn next(value: u8, backwards: bool) -> u8 {
    let place = CYCLE.iter().position(|&v| v == value).unwrap_or(0);
    let place = if backwards { (place + CYCLE.len() - 1) % CYCLE.len() } else { (place + 1) % CYCLE.len() };
    CYCLE[place]
}

const O: u8 = NORMAL;
const H: u8 = HALF;
const D: u8 = DOUBLE;
const Z: u8 = NONE;

/// The chart of the game: one row for each attacking type, one column for each defending type.
pub const ORIGINAL: [[u8; GAME_TYPES]; GAME_TYPES] = [
    [O, O, O, O, O, H, O, Z, H, O, O, O, O, O, O, O, O, O],
    [D, O, H, H, O, D, H, Z, D, O, O, O, O, H, D, O, D, H],
    [O, D, O, O, O, H, D, O, H, O, O, D, H, O, O, O, O, O],
    [O, O, O, H, H, H, O, H, Z, O, O, D, O, O, O, O, O, D],
    [O, O, Z, D, O, D, H, O, D, D, O, H, D, O, O, O, O, O],
    [O, H, D, O, H, O, D, O, H, D, O, O, O, O, D, O, O, O],
    [O, H, H, H, O, O, O, H, H, H, O, D, O, D, O, O, D, H],
    [Z, O, O, O, O, O, O, D, O, O, O, O, O, D, O, O, H, O],
    [O, O, O, O, O, D, O, O, H, H, H, O, H, O, D, O, O, D],
    [O, O, O, O, O, H, D, O, D, H, H, D, O, O, D, H, O, O],
    [O, O, O, O, D, D, O, O, O, D, H, H, O, O, O, H, O, O],
    [O, O, H, H, D, D, H, O, H, H, D, H, O, O, O, H, O, O],
    [O, O, D, O, Z, O, O, O, O, O, D, H, H, O, O, H, O, O],
    [O, D, O, D, O, O, O, O, H, O, O, O, O, H, O, O, Z, O],
    [O, O, D, O, D, O, O, O, H, H, H, D, O, O, H, D, O, O],
    [O, O, O, O, O, O, O, O, H, O, O, O, O, O, O, D, O, Z],
    [O, H, O, O, O, O, O, D, O, O, O, O, O, D, O, O, H, H],
    [O, D, O, H, O, O, O, O, H, H, O, O, O, O, O, D, D, O],
];

/// The type chart: the cells of the 32 types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chart {
    cells: Vec<u8>,
}

impl Chart {
    /// The chart of the game. The cells of the new types are ×1.
    pub fn original() -> Chart {
        let mut cells = vec![NORMAL; MAX_TYPES * MAX_TYPES];
        for (row, line) in ORIGINAL.iter().enumerate() {
            cells[row * MAX_TYPES..row * MAX_TYPES + GAME_TYPES].copy_from_slice(line);
        }
        Chart { cells }
    }

    /// The cell of an attacking type against a defending type (×1 for a number out of the chart).
    pub fn get(&self, attacking: usize, defending: usize) -> u8 {
        if attacking >= MAX_TYPES || defending >= MAX_TYPES {
            return NORMAL;
        }
        self.cells[attacking * MAX_TYPES + defending]
    }

    /// Gives a value to a cell. A value that is not 0, 2, 4 or 8 is refused.
    pub fn set(&mut self, attacking: usize, defending: usize, value: u8) -> bool {
        if attacking >= MAX_TYPES || defending >= MAX_TYPES || !CYCLE.contains(&value) {
            return false;
        }
        self.cells[attacking * MAX_TYPES + defending] = value;
        true
    }

    /// Puts the row and the column of a type back to ×1 (a type that is removed).
    pub fn clear_type(&mut self, kind: usize) {
        for other in 0..MAX_TYPES {
            self.set(kind, other, NORMAL);
            self.set(other, kind, NORMAL);
        }
    }

    /// True when the cell is not what the game has (a cell of a new type is compared with ×1).
    pub fn is_changed(&self, attacking: usize, defending: usize) -> bool {
        let original = if attacking < GAME_TYPES && defending < GAME_TYPES { ORIGINAL[attacking][defending] } else { NORMAL };
        self.get(attacking, defending) != original
    }

    /// The number of cells that are not what the game has.
    pub fn changed_count(&self) -> usize {
        (0..MAX_TYPES).flat_map(|a| (0..MAX_TYPES).map(move |d| (a, d))).filter(|&(a, d)| self.is_changed(a, d)).count()
    }

    /// The chart as CSV text for some types (their numbers): the first line is `attacking` and the numbers of the
    /// defending types; each other line is the number of an attacking type and its cells (`0`, `0.5`, `1` or `2`).
    pub fn to_csv(&self, ids: &[usize]) -> String {
        let mut out = String::from("attacking");
        for id in ids {
            out.push_str(&format!(",{id}"));
        }
        out.push('\n');
        for &attacking in ids {
            out.push_str(&attacking.to_string());
            for &defending in ids {
                out.push_str(match self.get(attacking, defending) {
                    NONE => ",0",
                    HALF => ",0.5",
                    DOUBLE => ",2",
                    _ => ",1",
                });
            }
            out.push('\n');
        }
        out
    }

    /// Reads a CSV text of the chart and gives the changed chart. `ids` are the numbers of the types that exist. Every line
    /// is checked: the chart does not change when one is wrong. A file can have some rows and some columns only.
    pub fn with_csv(&self, text: &str, ids: &[usize]) -> Result<Chart, String> {
        let records = sango_core::csv_text::records(text.strip_prefix('\u{feff}').unwrap_or(text))?;
        let (_, header) = records.first().ok_or("The file is empty.")?;
        if header.first().map(|h| h.trim()) != Some("attacking") {
            return Err("The first line is not the header of a type chart file: it starts with \"attacking\".".into());
        }
        let mut columns = Vec::new();
        for cell in &header[1..] {
            let id = cell.trim().parse::<usize>().map_err(|_| format!("Line 1: \"{cell}\" is not the number of a type."))?;
            if !ids.contains(&id) {
                return Err(format!("Line 1: the type {id} does not exist. Import the types file first."));
            }
            columns.push(id);
        }
        let mut next = self.clone();
        for (line, fields) in records.iter().skip(1) {
            if fields.len() == 1 && fields[0].is_empty() {
                continue;
            }
            if fields.len() != columns.len() + 1 {
                return Err(format!("Line {line}: {} columns instead of {}.", fields.len(), columns.len() + 1));
            }
            let attacking = fields[0].trim().parse::<usize>().map_err(|_| format!("Line {line}: \"{}\" is not the number of a type.", fields[0]))?;
            if !ids.contains(&attacking) {
                return Err(format!("Line {line}: the type {attacking} does not exist. Import the types file first."));
            }
            for (&defending, cell) in columns.iter().zip(&fields[1..]) {
                let value = match cell.trim() {
                    "0" => NONE,
                    "0.5" => HALF,
                    "1" => NORMAL,
                    "2" => DOUBLE,
                    other => return Err(format!("Line {line}, column {defending}: \"{other}\" is not 0, 0.5, 1 or 2.")),
                };
                next.set(attacking, defending, value);
            }
        }
        Ok(next)
    }

    /// The file of the SD card.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = b"TCHT".to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend((MAX_TYPES as u32).to_le_bytes());
        bytes.extend(&self.cells);
        bytes
    }

    /// Reads the file of the SD card.
    pub fn from_bytes(bytes: &[u8]) -> Result<Chart, String> {
        if bytes.get(..4) != Some(b"TCHT") {
            return Err("This is not a type chart file.".into());
        }
        let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if word(4) != Some(1) {
            return Err("The version of the type chart file is not 1.".into());
        }
        if word(8) != Some(MAX_TYPES as u32) {
            return Err(format!("The type chart must have {MAX_TYPES} types on a side."));
        }
        let cells = bytes.get(12..12 + MAX_TYPES * MAX_TYPES).ok_or("The type chart file is too short.")?.to_vec();
        if let Some(bad) = cells.iter().find(|v| !CYCLE.contains(v)) {
            return Err(format!("The type chart has the value {bad}: the values are 0, 2, 4 and 8."));
        }
        Ok(Chart { cells })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The facts of the chart of the sixth generation that every player knows.
    #[test]
    fn the_chart_has_the_effects_of_the_game() {
        let chart = Chart::original();
        // The numbers: 0 Normal, 1 Fighting, 2 Flying, 3 Poison, 4 Ground, 5 Rock, 6 Bug, 7 Ghost, 8 Steel, 9 Fire,
        // 10 Water, 11 Grass, 12 Electric, 13 Psychic, 14 Ice, 15 Dragon, 16 Dark, 17 Fairy.
        for (attacking, defending, value) in [
            (9, 11, DOUBLE),   // Fire hits Grass
            (10, 9, DOUBLE),   // Water hits Fire
            (11, 10, DOUBLE),  // Grass hits Water
            (12, 4, NONE),     // Electric does nothing to Ground
            (0, 7, NONE),      // Normal does nothing to Ghost
            (7, 0, NONE),      // Ghost does nothing to Normal
            (4, 2, NONE),      // Ground does nothing to Flying
            (15, 17, NONE),    // Dragon does nothing to Fairy
            (17, 15, DOUBLE),  // Fairy hits Dragon
            (8, 17, DOUBLE),   // Steel hits Fairy
            (17, 8, HALF),     // Fairy is resisted by Steel
            (7, 8, NORMAL),    // Ghost hits Steel normally (not so before the sixth generation)
            (16, 8, NORMAL),   // Dark hits Steel normally
            (1, 0, DOUBLE),    // Fighting hits Normal
            (13, 16, NONE),    // Psychic does nothing to Dark
            (3, 8, NONE),      // Poison does nothing to Steel
            (14, 15, DOUBLE),  // Ice hits Dragon
            (5, 2, DOUBLE),    // Rock hits Flying
            (6, 17, HALF),     // Bug is resisted by Fairy
        ] {
            assert_eq!(chart.get(attacking, defending), value, "{attacking} against {defending}");
        }
        // Each type has an effect on the chart, the same number of super effective cells as the games have (Normal has none).
        let doubles = |row: usize| ORIGINAL[row].iter().filter(|&&v| v == DOUBLE).count();
        assert_eq!(doubles(0), 0);
        assert_eq!((0..GAME_TYPES).map(doubles).sum::<usize>(), 51, "the sixth generation has 51 super effective cells");
        assert_eq!(ORIGINAL.iter().flatten().filter(|&&v| v == NONE).count(), 8, "and 8 cells with no effect");
        assert_eq!(chart.changed_count(), 0);
    }

    #[test]
    fn the_chart_goes_out_and_comes_back_as_csv() {
        let mut chart = Chart::original();
        let ids: Vec<usize> = (0..GAME_TYPES).chain([19, 20]).collect();
        chart.set(19, 9, DOUBLE);
        chart.set(0, 20, NONE);
        let text = chart.to_csv(&ids);
        assert!(text.starts_with("attacking,0,1,2,"));
        assert_eq!(text.lines().count(), 1 + ids.len());
        assert_eq!(Chart::original().with_csv(&text, &ids).unwrap(), chart);
        // A partial file changes only its cells.
        let partial = Chart::original().with_csv("attacking,9,11\n19,0.5,2\n", &ids).unwrap();
        assert_eq!((partial.get(19, 9), partial.get(19, 11), partial.get(19, 0), partial.changed_count()), (HALF, DOUBLE, NORMAL, 2));
        for (text, start) in [
            ("nothing\n", "The first line"),
            ("attacking,40\n", "Line 1: the type 40"),
            ("attacking,0\n21,1\n", "Line 2: the type 21"),
            ("attacking,0\n0,3\n", "Line 2, column 0"),
            ("attacking,0,1\n0,1\n", "Line 2: 2 columns"),
        ] {
            assert!(Chart::original().with_csv(text, &ids).unwrap_err().starts_with(start), "{text}");
        }
    }

    #[test]
    fn a_cell_is_changed_cycled_and_saved() {
        let mut chart = Chart::original();
        assert!(chart.set(9, 11, HALF));
        assert!(chart.set(19, 0, DOUBLE));
        assert!(!chart.set(9, 11, 3), "only 0, 2, 4 and 8 are values");
        assert!(!chart.set(32, 0, NORMAL));
        assert_eq!(chart.changed_count(), 2);
        assert!(chart.is_changed(19, 0) && chart.is_changed(9, 11) && !chart.is_changed(9, 10));
        let back = Chart::from_bytes(&chart.to_bytes()).unwrap();
        assert_eq!(back, chart);
        chart.clear_type(19);
        assert_eq!(chart.get(19, 0), NORMAL);
        // The click goes ×1, ×2, ×½, ×0, ×1 and back.
        assert_eq!(next(NORMAL, false), DOUBLE);
        assert_eq!(next(DOUBLE, false), HALF);
        assert_eq!(next(HALF, false), NONE);
        assert_eq!(next(NONE, false), NORMAL);
        assert_eq!(next(NORMAL, true), NONE);
        assert!(Chart::from_bytes(b"nope").is_err());
        let mut bad = Chart::original().to_bytes();
        bad[20] = 3;
        assert!(Chart::from_bytes(&bad).unwrap_err().contains("value 3"));
        assert!(Chart::from_bytes(&bad[..100]).is_err());
    }
}
