//! The formulas of the user: the text file that the page reads and the program file that the plugin reads.
//!
//! The text file `formula/formulas.txt` is the source: one line `name = formula` for each statistic that the user changed, and
//! `shiny = percent` when the chance is not the one of the game. The program file `formula/formulas.bin` is made from it (the layout is in
//! `lib/include/pokemon/patch/studio_formulas.h` of the plugin). When nothing is changed, both files are removed.

use crate::defaults::{shiny_game_percent, KEYS, SHINY_GAME, SHINY_OUT_OF, STATS};
use crate::expr;
use sango_core::SdCard;

pub const TEXT_FILE: &str = "formula/formulas.txt";
pub const BIN_FILE: &str = "formula/formulas.bin";

/// The formulas that the page edits.
#[derive(Clone, Debug, PartialEq)]
pub struct Formulas {
    /// The text of the formula of each statistic (the one of the game when the user did not change it).
    pub texts: [String; 6],
    /// The chance of a shiny Pokémon, in percent.
    pub shiny_percent: f64,
}

impl Formulas {
    /// The formulas of the game.
    pub fn game() -> Formulas {
        Formulas { texts: STATS.map(|(_, text)| text.to_string()), shiny_percent: shiny_game_percent() }
    }

    /// True when the text is the formula of the game for this statistic (the spaces do not count).
    pub fn is_game_text(stat: usize, text: &str) -> bool {
        let strip = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
        strip(text) == strip(STATS[stat].1)
    }

    /// True when nothing is different from the game.
    pub fn is_game(&self) -> bool {
        (0..6).all(|s| Formulas::is_game_text(s, &self.texts[s])) && self.threshold() == SHINY_GAME
    }

    /// The shiny chance in 65536ths: the limit of the test of the game.
    pub fn threshold(&self) -> u32 {
        (self.shiny_percent / 100.0 * f64::from(SHINY_OUT_OF)).round().clamp(0.0, f64::from(SHINY_OUT_OF)) as u32
    }

    /// Reads the text or CSV file. A line that is not understood is an error (the page shows it).
    pub fn parse(text: &str) -> Result<Formulas, String> {
        let mut formulas = Formulas::game();
        if text.contains(',') {
            if let Ok(records) = sango_core::csv_text::records(text) {
                let mut found_any = false;
                for (line_no, fields) in records {
                    if fields.is_empty() {
                        continue;
                    }
                    let key = fields[0].trim().to_lowercase();
                    if key == "stat" || key == "name" || key.starts_with('#') {
                        continue;
                    }
                    if fields.len() < 2 {
                        continue;
                    }
                    let value = fields[1].trim();
                    if key == "shiny" {
                        formulas.shiny_percent = value.parse().map_err(|_| format!("Line {line_no}: the shiny chance is not a number."))?;
                        found_any = true;
                    } else if let Some(stat) = KEYS.iter().position(|k| *k == key) {
                        formulas.texts[stat] = value.to_string();
                        found_any = true;
                    } else {
                        return Err(format!("Line {line_no}: {key} is not a statistic."));
                    }
                }
                if found_any {
                    return Ok(formulas);
                }
            }
        }
        for (at, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or_else(|| format!("Line {}: a line is `name = formula`.", at + 1))?;
            let key = key.trim().to_lowercase();
            if key == "shiny" {
                formulas.shiny_percent = value.trim().parse().map_err(|_| format!("Line {}: the shiny chance is not a number.", at + 1))?;
            } else if let Some(stat) = KEYS.iter().position(|k| *k == key) {
                formulas.texts[stat] = value.trim().to_string();
            } else {
                return Err(format!("Line {}: {key} is not a statistic.", at + 1));
            }
        }
        Ok(formulas)
    }

    /// The CSV file with all statistics and the shiny chance.
    pub fn to_csv(&self) -> String {
        let mut text = String::from("stat,formula\n");
        for stat in 0..6 {
            text += &format!("{},{}\n", KEYS[stat], sango_core::csv_text::quote(self.texts[stat].trim()));
        }
        text += &format!("shiny,{}\n", self.shiny_percent);
        text
    }

    /// The text file.
    pub fn to_text(&self) -> String {
        let mut text = String::from("# The formulas of Sango Studio. The page Formula writes this file.\n");
        for stat in 0..6 {
            if !Formulas::is_game_text(stat, &self.texts[stat]) {
                text += &format!("{} = {}\n", KEYS[stat], self.texts[stat].trim());
            }
        }
        if self.threshold() != SHINY_GAME {
            text += &format!("shiny = {}\n", self.shiny_percent);
        }
        text
    }

    /// The program file for the plugin. The error tells which formula is wrong.
    pub fn to_bin(&self) -> Result<Vec<u8>, String> {
        let mut programs = Vec::new();
        for stat in 0..6 {
            if Formulas::is_game_text(stat, &self.texts[stat]) {
                continue;
            }
            let program = expr::compile(&self.texts[stat]).map_err(|e| format!("{}: {e}", STATS[stat].0))?;
            programs.push((stat as u8, program));
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"SFRM");
        bytes.push(1);
        bytes.push(programs.len() as u8);
        bytes.extend_from_slice(&[0, 0]);
        let threshold = if self.threshold() == SHINY_GAME { u32::MAX } else { self.threshold() };
        bytes.extend_from_slice(&threshold.to_le_bytes());
        for (stat, program) in programs {
            bytes.push(stat);
            bytes.push(program.len() as u8);
            bytes.extend_from_slice(&program);
        }
        Ok(bytes)
    }

    /// Reads the formulas of the SD card, or gives the formulas of the game.
    pub fn read(sd: &SdCard) -> Result<Formulas, String> {
        match sd.read(TEXT_FILE) {
            Some(bytes) => Formulas::parse(&String::from_utf8_lossy(&bytes)),
            None => Ok(Formulas::game()),
        }
    }

    /// Writes the files on the SD card (or removes them when nothing is changed) and tells the plugin to read them again.
    pub fn save(&self, sd: &SdCard) -> Result<(), String> {
        if self.is_game() {
            sd.remove(TEXT_FILE)?;
            sd.remove(BIN_FILE)?;
        } else {
            let bin = self.to_bin()?;
            sd.write(TEXT_FILE, self.to_text().as_bytes())?;
            sd.write(BIN_FILE, &bin)?;
        }
        sd.mark_changed(BIN_FILE)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::defaults::HP;

    #[test]
    fn the_game_formulas_write_nothing() {
        let formulas = Formulas::game();
        assert!(formulas.is_game());
        assert_eq!(formulas.threshold(), 16);
        assert_eq!(formulas.to_text().lines().count(), 1);
        // The spaces and the capitals do not make a change.
        let mut other = Formulas::game();
        other.texts[1] = "(2*BASE+iv+ev/4)*level/100+5".to_string();
        assert!(other.is_game());
    }

    #[test]
    fn the_formulas_go_through_their_file() {
        let mut formulas = Formulas::game();
        formulas.texts[1] = "(3 * base + iv + ev / 4) * level / 100 + 5".to_string();
        formulas.shiny_percent = 3.5;
        let again = Formulas::parse(&formulas.to_text()).unwrap();
        assert_eq!(again, formulas);
        assert!(!again.is_game());
        assert_eq!(again.threshold(), 2294);

        let csv = formulas.to_csv();
        assert!(csv.contains("attack,(3 * base + iv + ev / 4) * level / 100 + 5"));
        assert!(csv.contains(&format!("hp,\"{HP}\"")));
        let from_csv = Formulas::parse(&csv).unwrap();
        assert_eq!(from_csv, formulas);
    }

    #[test]
    fn the_program_file_has_the_layout_of_the_plugin() {
        let mut formulas = Formulas::game();
        formulas.texts[5] = "base + level".to_string();
        let bin = formulas.to_bin().unwrap();
        assert_eq!(&bin[..4], b"SFRM");
        assert_eq!((bin[4], bin[5]), (1, 1));
        // The chance of the game.
        assert_eq!(&bin[8..12], &[0xFF, 0xFF, 0xFF, 0xFF]);
        // The statistic 5, then a program of 5 bytes: base, level, add.
        assert_eq!(&bin[12..], &[5, 5, 0x02, 0, 0x02, 3, 0x10]);
        formulas.shiny_percent = 100.0;
        assert_eq!(&formulas.to_bin().unwrap()[8..12], &65536u32.to_le_bytes());
        formulas.shiny_percent = 0.0;
        assert_eq!(&formulas.to_bin().unwrap()[8..12], &0u32.to_le_bytes());
    }

    #[test]
    fn a_wrong_formula_is_not_written() {
        let mut formulas = Formulas::game();
        formulas.texts[0] = "base +".to_string();
        assert!(formulas.to_bin().unwrap_err().starts_with("HP:"));
        assert!(Formulas::parse("speed").is_err());
        assert!(Formulas::parse("luck = 3").is_err());
        assert!(Formulas::parse("shiny = a lot").is_err());
    }
}
