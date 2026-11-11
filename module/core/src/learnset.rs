//! The moves that a Pokémon learns: by leveling up, and as an egg move.
//!
//! **Level-up moves** are in the archive 191, one file for each species and form, in the same order as the
//! species data (the form 1 of a species is where its data says, after all the species). A file is a list of
//! pairs of little-endian `u16`, the move then the level, and ends with the pair `0xFFFF, 0xFFFF`.
//! The game keeps at most [`MAX_LEVEL_UP_MOVES`] pairs.
//!
//! **Egg moves** are in the archive 190, one file for each species (the forms use the file of their species).
//! A file is a little-endian `u16` count followed by that many `u16` move numbers. A species without egg moves
//! can have an empty file of 0 bytes.

/// The number of the archive that has the level-up moves.
pub const LEVEL_UP_ARCHIVE: usize = 191;

/// The number of the archive that has the egg moves.
pub const EGG_ARCHIVE: usize = 190;

/// The most moves that a species can learn by leveling up. This is the only place that says it.
pub const MAX_LEVEL_UP_MOVES: usize = 26;

/// The end of a file of level-up moves.
const END: u16 = 0xFFFF;

/// The place on the SD card of the level-up moves of a species and a form.
pub fn level_up_sd_path(species: usize, form: usize) -> String {
    format!("pokemon/learnset/{species}_{form}.bin")
}

/// The place on the SD card of the egg moves of a species.
pub fn egg_sd_path(species: usize) -> String {
    format!("pokemon/egg_moves/{species}.bin")
}

/// A move that is learned at a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelUpMove {
    /// The move number.
    pub id: u16,
    /// The level (1 to 100).
    pub level: u16,
}

/// The level-up moves of a species or a form, in the order of the file.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LevelUp {
    pub moves: Vec<LevelUpMove>,
}

impl LevelUp {
    /// Reads the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<LevelUp, String> {
        if bytes.len() % 4 != 0 || bytes.len() < 4 {
            return Err(format!("The file of level-up moves has {} bytes. It must be a multiple of 4, with an end.", bytes.len()));
        }
        let mut moves = Vec::new();
        let mut chunks = bytes.chunks_exact(4);
        loop {
            let chunk = chunks.next().ok_or("The file of level-up moves has no end.")?;
            let (id, level) = (u16::from_le_bytes([chunk[0], chunk[1]]), u16::from_le_bytes([chunk[2], chunk[3]]));
            if id == END && level == END {
                break;
            }
            moves.push(LevelUpMove { id, level });
        }
        if chunks.next().is_some() {
            return Err("The file of level-up moves has bytes after its end.".into());
        }
        if moves.len() > MAX_LEVEL_UP_MOVES {
            return Err(format!("The file has {} level-up moves. The most is {MAX_LEVEL_UP_MOVES}.", moves.len()));
        }
        Ok(LevelUp { moves })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = self.moves.iter().flat_map(|m| m.id.to_le_bytes().into_iter().chain(m.level.to_le_bytes())).collect();
        bytes.extend(END.to_le_bytes());
        bytes.extend(END.to_le_bytes());
        bytes
    }

    /// Puts the moves in the order of their levels. Moves of the same level keep their order.
    pub fn sort(&mut self) {
        self.moves.sort_by_key(|m| m.level);
    }

    /// True when another move can be added.
    pub fn has_room(&self) -> bool {
        self.moves.len() < MAX_LEVEL_UP_MOVES
    }
}

/// The egg moves of a species.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EggMoves {
    pub moves: Vec<u16>,
    /// True when the file was empty (0 bytes), not a count of 0. It is written back as it was.
    empty_file: bool,
}

impl EggMoves {
    /// Reads the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<EggMoves, String> {
        if bytes.is_empty() {
            return Ok(EggMoves { moves: Vec::new(), empty_file: true });
        }
        if bytes.len() < 2 {
            return Err("The file of egg moves is too short.".into());
        }
        let count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
        if bytes.len() != 2 + 2 * count {
            return Err(format!("The file of egg moves has {} bytes for {count} moves. It must have {}.", bytes.len(), 2 + 2 * count));
        }
        let moves = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(EggMoves { moves, empty_file: false })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        if self.empty_file && self.moves.is_empty() {
            return Vec::new();
        }
        let mut bytes = (self.moves.len() as u16).to_le_bytes().to_vec();
        bytes.extend(self.moves.iter().flat_map(|m| m.to_le_bytes()));
        bytes
    }

    /// Adds a move at the end. The file is not empty any more.
    pub fn push(&mut self, id: u16) {
        self.moves.push(id);
        self.empty_file = false;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn level_up_moves_are_written_back_the_same() {
        let bytes: Vec<u8> = [(33u16, 1u16), (45, 3), (0xFFFF, 0xFFFF)].iter().flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes())).collect();
        let learnset = LevelUp::from_bytes(&bytes).unwrap();
        assert_eq!(learnset.moves, [LevelUpMove { id: 33, level: 1 }, LevelUpMove { id: 45, level: 3 }]);
        assert_eq!(learnset.to_bytes(), bytes);
        // A species without level-up moves is only the end.
        assert_eq!(LevelUp::from_bytes(&[0xFF; 4]).unwrap(), LevelUp::default());
        assert_eq!(LevelUp::default().to_bytes(), vec![0xFF; 4]);
    }

    #[test]
    fn a_wrong_file_of_level_up_moves_is_refused() {
        assert!(LevelUp::from_bytes(&[]).is_err());
        assert!(LevelUp::from_bytes(&[1, 0, 1]).is_err());
        assert!(LevelUp::from_bytes(&[1, 0, 1, 0]).is_err(), "no end");
        assert!(LevelUp::from_bytes(&[0xFF, 0xFF, 0xFF, 0xFF, 1, 0, 1, 0]).is_err(), "bytes after the end");
        let mut too_many = Vec::new();
        for _ in 0..=MAX_LEVEL_UP_MOVES {
            too_many.extend([1u8, 0, 1, 0]);
        }
        too_many.extend([0xFF; 4]);
        assert!(LevelUp::from_bytes(&too_many).is_err());
    }

    #[test]
    fn level_up_moves_are_sorted_by_level_and_the_room_is_counted() {
        let mut learnset = LevelUp { moves: vec![LevelUpMove { id: 2, level: 9 }, LevelUpMove { id: 1, level: 3 }, LevelUpMove { id: 3, level: 9 }] };
        learnset.sort();
        assert_eq!(learnset.moves.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert!(learnset.has_room());
        learnset.moves = vec![LevelUpMove { id: 1, level: 1 }; MAX_LEVEL_UP_MOVES];
        assert!(!learnset.has_room());
    }

    #[test]
    fn egg_moves_are_written_back_the_same() {
        let bytes = [2, 0, 133, 0, 174, 0];
        let eggs = EggMoves::from_bytes(&bytes).unwrap();
        assert_eq!(eggs.moves, [133, 174]);
        assert_eq!(eggs.to_bytes(), bytes);
        // An empty file stays empty, a count of 0 stays a count of 0.
        assert_eq!(EggMoves::from_bytes(&[]).unwrap().to_bytes(), Vec::<u8>::new());
        assert_eq!(EggMoves::from_bytes(&[0, 0]).unwrap().to_bytes(), vec![0, 0]);
        // A move added to an empty file gives a file with a count.
        let mut eggs = EggMoves::from_bytes(&[]).unwrap();
        eggs.push(5);
        assert_eq!(eggs.to_bytes(), vec![1, 0, 5, 0]);
        assert!(EggMoves::from_bytes(&[3, 0, 1, 0]).is_err());
        assert!(EggMoves::from_bytes(&[1]).is_err());
    }

    /// With the dump of the game: every file is read and written back the same, and Bulbasaur has the moves
    /// that Bulbapedia gives (Generation VI).
    #[test]
    fn the_learnsets_of_the_dump_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let level_up = crate::Garc::open(&dump.archive(LEVEL_UP_ARCHIVE)).unwrap();
        assert_eq!(level_up.file_count(), 826);
        for id in 0..level_up.file_count() {
            let bytes = level_up.first_file(id).unwrap();
            assert_eq!(LevelUp::from_bytes(bytes).unwrap().to_bytes(), bytes, "level-up file {id}");
        }
        let egg = crate::Garc::open(&dump.archive(EGG_ARCHIVE)).unwrap();
        assert_eq!(egg.file_count(), 722);
        for id in 0..egg.file_count() {
            let bytes = egg.first_file(id).unwrap();
            assert_eq!(EggMoves::from_bytes(bytes).unwrap().to_bytes(), bytes, "egg file {id}");
        }
        // Bulbasaur, by leveling up (move number, level). The names are in the Bulbapedia table:
        // Tackle, Growl, Leech Seed, Vine Whip, Poison Powder, Sleep Powder, Take Down, Razor Leaf,
        // Sweet Scent, Growth, Double-Edge, Worry Seed, Synthesis, Seed Bomb.
        let bulbasaur = LevelUp::from_bytes(level_up.first_file(1).unwrap()).unwrap();
        let expected = [(33, 1), (45, 3), (73, 7), (22, 9), (77, 13), (79, 13), (36, 15), (75, 19), (230, 21), (74, 25), (38, 27), (388, 31), (235, 33), (402, 37)];
        assert_eq!(bulbasaur.moves.iter().map(|m| (m.id, m.level)).collect::<Vec<_>>(), expected);
        // Bulbasaur has 15 egg moves (Amnesia ... Sludge) on Bulbapedia.
        assert_eq!(EggMoves::from_bytes(egg.first_file(1).unwrap()).unwrap().moves.len(), 15);
    }
}
