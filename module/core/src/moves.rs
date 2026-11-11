//! The data of the moves: type, category, power, accuracy and PP.
//!
//! The file 0 of the archive 189 is a bundle with one resource for each move, from the move 0.
//! A resource has 36 bytes. The table lists only the bytes that the studio shows; the other bytes are kept as they are.
//!
//! | Byte | Content |
//! | --- | --- |
//! | 0 | The type, as a number from 0 (Normal) to 17 (Fairy). |
//! | 2 | The category: 0 Status, 1 Physical, 2 Special. |
//! | 3 | The power. 0 for a Status move, 1 for a move whose power changes (Return, Grass Knot...). |
//! | 4 | The accuracy in percent. 101 for a move that never misses. |
//! | 5 | The PP. |

use crate::{Bundle, Garc};

/// The number of the archive that has the data of the moves.
pub const ARCHIVE: usize = 189;

/// The size of the data of one move, in bytes.
pub const ENTRY_SIZE: usize = 36;

/// The category of a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Status,
    Physical,
    Special,
}

impl Category {
    /// The name of the category, as in the games.
    pub fn name(self) -> &'static str {
        match self {
            Category::Status => "Status",
            Category::Physical => "Physical",
            Category::Special => "Special",
        }
    }
}

/// What the studio shows of one move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    /// The type number (0 to 17).
    pub kind: u8,
    pub category: Category,
    /// The power. 0 for a Status move, 1 for a move whose power changes.
    pub power: u8,
    /// The accuracy in percent. 101 for a move that never misses.
    pub accuracy: u8,
    pub pp: u8,
}

impl Move {
    /// The power as text: a number, or a dash when the move has no fixed power.
    pub fn power_text(&self) -> String {
        if self.power <= 1 { "—".to_string() } else { self.power.to_string() }
    }

    /// The accuracy as text: a percentage, or a dash when the move never misses or has no accuracy.
    pub fn accuracy_text(&self) -> String {
        if self.accuracy == 0 || self.accuracy > 100 { "—".to_string() } else { format!("{}%", self.accuracy) }
    }
}

/// The data of all the moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveTable {
    moves: Vec<Move>,
}

impl MoveTable {
    /// Reads the file of the data of the moves.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<MoveTable, String> {
        let bundle = Bundle::parse(bytes)?;
        let mut moves = Vec::with_capacity(bundle.resource_count());
        for id in 0..bundle.resource_count() {
            let entry = bundle.resource(id).ok_or_else(|| format!("The move {id} is missing."))?;
            if entry.len() != ENTRY_SIZE {
                return Err(format!("The move {id} has {} bytes. It must have {ENTRY_SIZE}.", entry.len()));
            }
            let category = match entry[2] {
                0 => Category::Status,
                1 => Category::Physical,
                2 => Category::Special,
                other => return Err(format!("The move {id} has the category {other}.")),
            };
            moves.push(Move { kind: entry[0], category, power: entry[3], accuracy: entry[4], pp: entry[5] });
        }
        Ok(MoveTable { moves })
    }

    /// Reads the data from the archive 189.
    pub fn from_garc(garc: &Garc) -> Result<MoveTable, String> {
        MoveTable::from_bytes(garc.first_file(0).ok_or("The file of the moves is missing.")?.to_vec())
    }

    /// The data of a move.
    pub fn get(&self, id: usize) -> Option<&Move> {
        self.moves.get(id)
    }

    /// The number of moves, with the move 0.
    pub fn len(&self) -> usize {
        self.moves.len()
    }

    /// True when the table has no move.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A file with 3 moves: 0, then a physical move and a status move.
    fn sample() -> Vec<u8> {
        let mut b = vec![b'W', b'D', 3, 0];
        for offset in [20u32, 56, 92, 128] {
            b.extend(offset.to_le_bytes());
        }
        for (kind, category, power, accuracy, pp) in [(0u8, 0u8, 0u8, 0u8, 0u8), (0, 1, 50, 100, 35), (11, 0, 0, 101, 20)] {
            let mut entry = [0u8; ENTRY_SIZE];
            (entry[0], entry[2], entry[3], entry[4], entry[5]) = (kind, category, power, accuracy, pp);
            b.extend(entry);
        }
        b
    }

    #[test]
    fn the_fields_are_read_from_their_bytes() {
        let table = MoveTable::from_bytes(sample()).unwrap();
        assert_eq!(table.len(), 3);
        let tackle = table.get(1).unwrap();
        assert_eq!((tackle.kind, tackle.category, tackle.power, tackle.accuracy, tackle.pp), (0, Category::Physical, 50, 100, 35));
        assert_eq!(tackle.power_text(), "50");
        assert_eq!(tackle.accuracy_text(), "100%");
        let status = table.get(2).unwrap();
        assert_eq!((status.power_text().as_str(), status.accuracy_text().as_str()), ("—", "—"));
        assert!(table.get(3).is_none());
    }

    #[test]
    fn a_wrong_file_is_refused() {
        let mut bytes = sample();
        bytes[4 + 4 * 2] = 90; // the end of the move 1 moves: the move 1 has the wrong size
        assert!(MoveTable::from_bytes(bytes).is_err());
        let mut bytes = sample();
        let place = 20 + 36 + 2;
        bytes[place] = 9; // an unknown category
        assert!(MoveTable::from_bytes(bytes).is_err());
    }

    /// With the dump of the game: the moves of Bulbasaur have the values that Bulbapedia gives (Generation VI).
    #[test]
    fn the_moves_of_the_dump_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let table = MoveTable::from_garc(&Garc::open(&dump.archive(ARCHIVE)).unwrap()).unwrap();
        assert_eq!(table.len(), 622);
        // (move, type, category, power, accuracy, PP)
        let expected = [
            (33, 0, Category::Physical, "50", "100%", 35),   // Tackle
            (45, 0, Category::Status, "—", "100%", 40),      // Growl
            (73, 11, Category::Status, "—", "90%", 10),      // Leech Seed
            (22, 11, Category::Physical, "45", "100%", 25),  // Vine Whip
            (38, 0, Category::Physical, "120", "100%", 15),  // Double-Edge
            (74, 0, Category::Status, "—", "—", 20),         // Growth
            (76, 11, Category::Special, "120", "100%", 10),  // Solar Beam
            (216, 0, Category::Physical, "—", "100%", 20),   // Return: the power changes
            (36, 0, Category::Physical, "90", "85%", 20),    // Take Down
        ];
        for (id, kind, category, power, accuracy, pp) in expected {
            let m = table.get(id).unwrap();
            assert_eq!((m.kind, m.category, m.power_text().as_str(), m.accuracy_text().as_str(), m.pp), (kind, category, power, accuracy, pp), "move {id}");
        }
    }
}
