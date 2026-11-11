//! The experience tables: the experience that a Pokémon needs for each level, one table per Leveling rate.
//!
//! The archive 194 has one file per Leveling rate, in the order of the rate numbers (0 to 7).
//! A file has 404 bytes: 101 little-endian `u32`, one per level from 0 to 100.
//! Each number is the total experience that a Pokémon has when it reaches the level.
//! The levels 0 and 1 are 0.

use crate::{Garc, SdCard};

/// The number of the archive that has the experience tables.
pub const ARCHIVE: usize = 194;

/// The place on the SD card of the table of a Leveling rate. It takes the place of the file of the romfs.
pub fn sd_path(rate: usize) -> String {
    format!("pokemon/experience/{rate}.bin")
}

/// The number of Leveling rates in the archive.
pub const RATE_COUNT: usize = 8;

/// The highest level that a table has.
pub const MAX_LEVEL: usize = 100;

/// The size of the file of one table, in bytes.
pub const FILE_SIZE: usize = (MAX_LEVEL + 1) * 4;

/// The experience table of one Leveling rate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curve {
    /// The total experience at each level, from 0 to 100.
    levels: [u32; MAX_LEVEL + 1],
}

impl Curve {
    /// Reads the bytes of a file. The size must be exact and the numbers must not go down.
    pub fn from_bytes(bytes: &[u8]) -> Result<Curve, String> {
        if bytes.len() != FILE_SIZE {
            return Err(format!("The experience table has {} bytes. It must have {FILE_SIZE}.", bytes.len()));
        }
        let mut levels = [0u32; MAX_LEVEL + 1];
        for (level, chunk) in levels.iter_mut().zip(bytes.chunks_exact(4)) {
            *level = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        if levels.windows(2).any(|pair| pair[1] < pair[0]) {
            return Err("The experience goes down from one level to the next.".into());
        }
        Ok(Curve { levels })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.levels.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// The total experience at a level (0 to 100).
    pub fn experience(&self, level: usize) -> Option<u32> {
        self.levels.get(level).copied()
    }

    /// Gives a level a new experience. The levels 0 and 1 stay at 0. The value is cut at `cap`.
    ///
    /// Without `push`, the value stays between the values of the two next levels: a point cannot cross its neighbours.
    /// With `push`, the neighbours move with it: the levels above are raised to the value if they are below it,
    /// and the levels below are lowered to the value if they are above it. Returns true when the curve changed.
    pub fn set_experience(&mut self, level: usize, value: u32, cap: u32, push: bool) -> bool {
        if !(2..=MAX_LEVEL).contains(&level) {
            return false;
        }
        let before = self.levels;
        let value = value.min(cap);
        if push {
            self.levels[level] = value;
            for above in level + 1..=MAX_LEVEL {
                self.levels[above] = self.levels[above].max(value);
            }
            for below in 2..level {
                self.levels[below] = self.levels[below].min(value);
            }
        } else {
            let low = self.levels[level - 1];
            let high = if level == MAX_LEVEL { cap } else { self.levels[level + 1] };
            self.levels[level] = value.clamp(low, high.max(low));
        }
        self.levels != before
    }

    /// Gives the step from a level to the next one a new size. The levels above keep their own steps:
    /// they move up or down by the same amount, but never below the level before them or above `cap`.
    /// Returns true when the curve changed. The levels 1 to 99 have a step.
    pub fn set_gain(&mut self, level: usize, gain: u32, cap: u32) -> bool {
        if !(1..MAX_LEVEL).contains(&level) {
            return false;
        }
        let before = self.levels;
        let wanted = (self.levels[level] as u64 + gain as u64).min(cap as u64) as i64;
        let shift = wanted - self.levels[level + 1] as i64;
        self.levels[level + 1] = wanted as u32;
        for above in level + 2..=MAX_LEVEL {
            self.levels[above] = (self.levels[above] as i64 + shift).clamp(self.levels[above - 1] as i64, cap.max(self.levels[above - 1]) as i64) as u32;
        }
        self.levels != before
    }

    /// Writes the table on the SD card for a Leveling rate, and tells the game.
    pub fn save(&self, sd: &SdCard, rate: usize) -> Result<(), String> {
        let path = sd_path(rate);
        sd.write(&path, &self.to_bytes())?;
        sd.mark_changed(&path)
    }

    /// The experience to gain to go from one level to a higher one.
    pub fn between(&self, from: usize, to: usize) -> Option<u32> {
        self.experience(to)?.checked_sub(self.experience(from)?)
    }

    /// The level of a Pokémon that has this total experience: the highest level whose table value is not above it.
    pub fn level_for(&self, experience: u32) -> usize {
        self.levels.iter().rposition(|&value| value <= experience).unwrap_or(0).max(1)
    }
}

/// The experience tables of all the Leveling rates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExperienceTable {
    curves: Vec<Curve>,
}

impl ExperienceTable {
    /// Reads the tables from the archive 194.
    pub fn from_garc(garc: &Garc) -> Result<ExperienceTable, String> {
        let curves = (0..RATE_COUNT)
            .map(|rate| {
                let bytes = garc.first_file(rate).ok_or_else(|| format!("The experience table {rate} is missing."))?;
                Curve::from_bytes(bytes).map_err(|message| format!("Table {rate}: {message}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExperienceTable { curves })
    }

    /// The table of a Leveling rate (0 to 7).
    pub fn curve(&self, rate: usize) -> Option<&Curve> {
        self.curves.get(rate)
    }

    /// The tables of all the Leveling rates, to change them.
    pub fn curves_mut(&mut self) -> &mut [Curve] {
        &mut self.curves
    }

    /// Replaces the table of a Leveling rate. False when the rate does not exist.
    pub fn set_curve(&mut self, rate: usize, curve: Curve) -> bool {
        match self.curves.get_mut(rate) {
            Some(place) => {
                *place = curve;
                true
            }
            None => false,
        }
    }

    /// Takes the tables that the SD card has, in place of the ones of the romfs.
    /// A file that is not a valid table is ignored. Returns the rates that now come from the SD card.
    pub fn overlay_sd(&mut self, sd: &SdCard) -> Vec<usize> {
        let mut from_card = Vec::new();
        for rate in 0..self.curves.len() {
            if let Some(curve) = sd.read(&sd_path(rate)).and_then(|bytes| Curve::from_bytes(&bytes).ok()) {
                self.curves[rate] = curve;
                from_card.push(rate);
            }
        }
        from_card
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The tables of the six Leveling rates, from the published formulas of Bulbapedia. Integer divisions round down.
    fn formula(rate: usize, n: u64) -> u64 {
        let cube = n * n * n;
        match rate {
            // Medium Fast
            0 => cube,
            // Erratic
            1 => match n {
                0..=50 => cube * (100 - n) / 50,
                51..=68 => cube * (150 - n) / 100,
                69..=98 => cube * ((1911 - 10 * n) / 3) / 500,
                _ => cube * (160 - n) / 100,
            },
            // Fluctuating
            2 => match n {
                0..=15 => cube * ((n + 1) / 3 + 24) / 50,
                16..=36 => cube * (n + 14) / 50,
                _ => cube * (n / 2 + 32) / 50,
            },
            // Medium Slow: the formula is below zero at the level 1, where the table has 0.
            3 => (6 * cube as i64 / 5 - 15 * (n * n) as i64 + 100 * n as i64 - 140).max(0) as u64,
            // Fast
            4 => 4 * cube / 5,
            // Slow
            5 => 5 * cube / 4,
            _ => unreachable!(),
        }
    }

    fn sample() -> Vec<u8> {
        (0..=MAX_LEVEL as u32).flat_map(|level| (level * level).to_le_bytes()).collect()
    }

    #[test]
    fn a_table_is_written_back_the_same() {
        let bytes = sample();
        let curve = Curve::from_bytes(&bytes).unwrap();
        assert_eq!(curve.to_bytes(), bytes);
        assert_eq!(curve.experience(10), Some(100));
        assert_eq!(curve.experience(101), None);
    }

    #[test]
    fn the_experience_between_two_levels_is_a_difference() {
        let curve = Curve::from_bytes(&sample()).unwrap();
        assert_eq!(curve.between(10, 11), Some(21));
        assert_eq!(curve.between(11, 10), None);
    }

    #[test]
    fn the_level_comes_from_the_experience() {
        let curve = Curve::from_bytes(&sample()).unwrap();
        assert_eq!(curve.level_for(0), 1);
        assert_eq!(curve.level_for(99), 9);
        assert_eq!(curve.level_for(100), 10);
        assert_eq!(curve.level_for(u32::MAX), 100);
    }

    #[test]
    fn a_point_stays_between_its_neighbours() {
        let mut curve = Curve::from_bytes(&sample()).unwrap(); // level n has n * n
        assert!(curve.set_experience(10, 110, 10_000, false));
        assert_eq!(curve.experience(10), Some(110));
        // Cut at the next level (121) and at the previous level (81).
        curve.set_experience(10, 5_000, 10_000, false);
        assert_eq!(curve.experience(10), Some(121));
        curve.set_experience(10, 0, 10_000, false);
        assert_eq!(curve.experience(10), Some(81));
        // The levels 0 and 1 and the levels out of the table do not move.
        assert!(!curve.set_experience(1, 50, 10_000, false));
        assert!(!curve.set_experience(101, 50, 10_000, false));
        // The last level is cut at the cap.
        curve.set_experience(100, 99_999, 10_000, false);
        assert_eq!(curve.experience(100), Some(10_000));
    }

    #[test]
    fn a_pushed_point_moves_its_neighbours() {
        let mut curve = Curve::from_bytes(&sample()).unwrap();
        assert!(curve.set_experience(10, 5_000, 10_000, true));
        assert_eq!(curve.experience(10), Some(5_000));
        assert_eq!(curve.experience(11), Some(5_000));
        assert_eq!(curve.experience(100), Some(10_000));
        assert_eq!(curve.experience(9), Some(81));
        curve.set_experience(10, 20, 10_000, true);
        assert_eq!(curve.experience(9), Some(20));
        assert_eq!(curve.experience(2), Some(4));
        // The levels above keep their values: the push only raises.
        assert_eq!(curve.experience(11), Some(5_000));
        // The table stays valid.
        assert!(Curve::from_bytes(&curve.to_bytes()).is_ok());
    }

    #[test]
    fn a_step_moves_the_levels_above_it() {
        let mut curve = Curve::from_bytes(&sample()).unwrap(); // level n has n * n
        // The step from 10 to 11 is 21. Make it 50: the levels from 11 on go up by 29.
        assert!(curve.set_gain(10, 50, 100_000));
        assert_eq!(curve.between(10, 11), Some(50));
        assert_eq!(curve.experience(11), Some(150));
        assert_eq!(curve.between(11, 12), Some(23));
        assert_eq!(curve.experience(100), Some(10_000 + 29));
        // A smaller step moves them down, and the levels before do not move.
        assert!(curve.set_gain(10, 0, 100_000));
        assert_eq!(curve.experience(11), Some(100));
        assert_eq!(curve.experience(10), Some(100));
        assert_eq!(curve.experience(9), Some(81));
        // The cap cuts the last levels, and the table stays valid.
        curve.set_gain(2, 90_000, 20_000);
        assert!(curve.experience(100).unwrap() <= 20_000);
        assert!(Curve::from_bytes(&curve.to_bytes()).is_ok());
        // The levels 0 and 100 have no step.
        assert!(!curve.set_gain(100, 5, 100_000));
        assert!(!curve.set_gain(0, 5, 100_000));
    }

    #[test]
    fn a_table_goes_on_the_sd_card_and_comes_back() {
        let folder = std::env::temp_dir().join(format!("sango_experience_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut curve = Curve::from_bytes(&sample()).unwrap();
        curve.set_experience(50, 1, 10_000, true);
        curve.save(&sd, 4).unwrap();
        let mut table = ExperienceTable { curves: vec![Curve::from_bytes(&sample()).unwrap(); RATE_COUNT] };
        assert_eq!(table.overlay_sd(&sd), [4]);
        assert_eq!(table.curve(4), Some(&curve));
        assert_eq!(table.curve(3), Some(&Curve::from_bytes(&sample()).unwrap()));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_wrong_table_is_refused() {
        assert!(Curve::from_bytes(&[0; 403]).is_err());
        let mut bytes = sample();
        bytes[8..12].copy_from_slice(&1000u32.to_le_bytes());
        assert!(Curve::from_bytes(&bytes).is_err());
    }

    /// With the dump of the game: each table is written back the same, and it matches the formula of Bulbapedia.
    #[test]
    fn the_tables_of_the_dump_follow_the_formulas() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(ARCHIVE)).unwrap();
        assert_eq!(garc.file_count(), RATE_COUNT);
        let table = ExperienceTable::from_garc(&garc).unwrap();
        for rate in 0..RATE_COUNT {
            assert_eq!(table.curve(rate).unwrap().to_bytes(), garc.first_file(rate).unwrap(), "rate {rate}");
        }
        // Medium Slow reaches 1,059,860 at the level 100, Erratic 600,000, Fluctuating 1,640,000.
        assert_eq!(table.curve(3).unwrap().experience(100), Some(1_059_860));
        assert_eq!(table.curve(1).unwrap().experience(100), Some(600_000));
        assert_eq!(table.curve(2).unwrap().experience(100), Some(1_640_000));
        for rate in 0..6 {
            // A Pokémon starts at the level 1 with 0 experience, whatever the rate: the formulas are for the levels 2 to 100.
            assert_eq!((table.curve(rate).unwrap().experience(0), table.curve(rate).unwrap().experience(1)), (Some(0), Some(0)));
            for level in 2..=MAX_LEVEL {
                assert_eq!(table.curve(rate).unwrap().experience(level), Some(formula(rate, level as u64) as u32), "rate {rate} level {level}");
            }
        }
        // The rates 6 and 7 are the same table as the rate 0.
        assert_eq!(table.curve(6), table.curve(0));
        assert_eq!(table.curve(7), table.curve(0));
    }
}
