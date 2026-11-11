//! The evolutions of a species or a form.
//!
//! The archive 192 has one file for each species and form, in the same order as the species data.
//! A file has 48 bytes: 8 slots of 6 bytes, with little-endian `u16` fields.
//!
//! | Offset in the slot | Content |
//! | --- | --- |
//! | 0 | The method: 0 for an empty slot, then the numbers of [`Method`]. |
//! | 2 | The argument. What it means depends on the method (see [`Argument`]). |
//! | 4 | The species that it evolves into. |
//!
//! The used slots come first. An empty slot has only zeros.

/// The number of the archive that has the evolutions.
pub const ARCHIVE: usize = 192;

/// The size of a file, in bytes.
pub const FILE_SIZE: usize = SLOTS * 6;

/// The number of evolutions that a species can have.
pub const SLOTS: usize = 8;

/// The place on the SD card of the evolutions of a species and a form. The file has the layout of the romfs.
pub fn sd_path(species: usize, form: usize) -> String {
    format!("pokemon/evolution/{species}_{form}.bin")
}

/// What the argument of an evolution means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    /// The argument is not used (0).
    None,
    /// A level.
    Level,
    /// An item number.
    Item,
    /// A move number.
    Move,
    /// A species number.
    Species,
    /// A type number.
    Type,
    /// The Beauty that the Pokémon needs.
    Beauty,
}

/// How a Pokémon evolves: the number of the method, what it is called, and what its argument is.
///
/// The methods follow the conditions that Bulbapedia gives for the evolutions of Generation VI.
pub const METHODS: [(u16, &str, Argument); 34] = [
    (1, "Level up with high friendship", Argument::None),
    (2, "Level up with high friendship, during the day", Argument::None),
    (3, "Level up with high friendship, at night", Argument::None),
    (4, "Level up", Argument::Level),
    (5, "Trade", Argument::None),
    (6, "Trade while holding an item", Argument::Item),
    (7, "Trade for a specific Pokémon", Argument::None),
    (8, "Use an item", Argument::Item),
    (9, "Level up with Attack higher than Defense", Argument::Level),
    (10, "Level up with Attack equal to Defense", Argument::Level),
    (11, "Level up with Attack lower than Defense", Argument::Level),
    (12, "Level up with an even personality value", Argument::Level),
    (13, "Level up with an odd personality value", Argument::Level),
    (14, "Level up (Ninjask)", Argument::Level),
    (15, "Level up with a free party slot and a Poké Ball (Shedinja)", Argument::None),
    (16, "Level up with high Beauty", Argument::Beauty),
    (17, "Use an item, male only", Argument::Item),
    (18, "Use an item, female only", Argument::Item),
    (19, "Level up while holding an item, during the day", Argument::Item),
    (20, "Level up while holding an item, at night", Argument::Item),
    (21, "Level up while knowing a move", Argument::Move),
    (22, "Level up with a specific Pokémon in the party", Argument::Species),
    (23, "Level up, male only", Argument::Level),
    (24, "Level up, female only", Argument::Level),
    (25, "Level up in a magnetic field", Argument::None),
    (26, "Level up near a Moss Rock", Argument::None),
    (27, "Level up near an Ice Rock", Argument::None),
    (28, "Level up with the console upside down", Argument::Level),
    (29, "Level up with high affection and a move of a type", Argument::Type),
    (30, "Level up with a Dark-type Pokémon in the party", Argument::Level),
    (31, "Level up in the rain", Argument::Level),
    (32, "Level up during the day", Argument::Level),
    (33, "Level up at night", Argument::Level),
    (34, "Level up, female only, with a different form", Argument::Level),
];

/// What the argument of a method means. A method that is not in the list has no argument.
pub fn argument_of(method: u16) -> Argument {
    METHODS.iter().find(|(number, _, _)| *number == method).map_or(Argument::None, |(_, _, argument)| *argument)
}

/// The name of a method. An empty slot or an unknown method gives "-".
pub fn method_name(method: u16) -> &'static str {
    METHODS.iter().find(|(number, _, _)| *number == method).map_or("-", |(_, name, _)| name)
}

/// One evolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Evolution {
    /// The method, or 0 for an empty slot.
    pub method: u16,
    /// The argument of the method.
    pub argument: u16,
    /// The species that it evolves into.
    pub species: u16,
}

/// The evolutions of a species or a form: the used slots, in the order of the file.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Evolutions {
    pub list: Vec<Evolution>,
}

impl Evolutions {
    /// Reads the bytes of a file. The used slots must come first.
    pub fn from_bytes(bytes: &[u8]) -> Result<Evolutions, String> {
        if bytes.len() != FILE_SIZE {
            return Err(format!("The file of evolutions has {} bytes. It must have {FILE_SIZE}.", bytes.len()));
        }
        let mut list = Vec::new();
        let mut ended = false;
        for (place, slot) in bytes.chunks_exact(6).enumerate() {
            let field = |at: usize| u16::from_le_bytes([slot[at], slot[at + 1]]);
            let evolution = Evolution { method: field(0), argument: field(2), species: field(4) };
            if evolution.method == 0 {
                if evolution != Evolution::default() {
                    return Err(format!("The empty slot {place} has data."));
                }
                ended = true;
            } else if ended {
                return Err(format!("The slot {place} is used after an empty slot."));
            } else {
                list.push(evolution);
            }
        }
        Ok(Evolutions { list })
    }

    /// The bytes of the file. The slots after the last evolution are empty.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FILE_SIZE);
        for place in 0..SLOTS {
            let evolution = self.list.get(place).copied().unwrap_or_default();
            bytes.extend(evolution.method.to_le_bytes());
            bytes.extend(evolution.argument.to_le_bytes());
            bytes.extend(evolution.species.to_le_bytes());
        }
        bytes
    }

    /// True when another evolution can be added.
    pub fn has_room(&self) -> bool {
        self.list.len() < SLOTS
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn slot(method: u16, argument: u16, species: u16) -> Vec<u8> {
        [method, argument, species].iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn evolutions_are_written_back_the_same() {
        let mut bytes = slot(4, 16, 2);
        bytes.resize(FILE_SIZE, 0);
        let evolutions = Evolutions::from_bytes(&bytes).unwrap();
        assert_eq!(evolutions.list, [Evolution { method: 4, argument: 16, species: 2 }]);
        assert_eq!(evolutions.to_bytes(), bytes);
        assert_eq!(Evolutions::from_bytes(&[0; FILE_SIZE]).unwrap(), Evolutions::default());
        assert_eq!(Evolutions::default().to_bytes(), vec![0; FILE_SIZE]);
    }

    #[test]
    fn a_wrong_file_is_refused() {
        assert!(Evolutions::from_bytes(&[0; 47]).is_err());
        // A used slot after an empty one.
        let mut bytes = vec![0; 6];
        bytes.extend(slot(4, 16, 2));
        bytes.resize(FILE_SIZE, 0);
        assert!(Evolutions::from_bytes(&bytes).is_err());
        // An empty slot that has data.
        let mut bytes = slot(0, 5, 0);
        bytes.resize(FILE_SIZE, 0);
        assert!(Evolutions::from_bytes(&bytes).is_err());
    }

    #[test]
    fn the_room_is_counted() {
        let mut evolutions = Evolutions::default();
        evolutions.list = vec![Evolution { method: 4, argument: 5, species: 1 }; SLOTS];
        assert!(!evolutions.has_room());
        evolutions.list.pop();
        assert!(evolutions.has_room());
        assert_eq!(evolutions.to_bytes().len(), FILE_SIZE);
    }

    #[test]
    fn the_methods_have_a_name_and_an_argument() {
        assert_eq!(METHODS.iter().map(|m| m.0).collect::<Vec<_>>(), (1..=34).collect::<Vec<u16>>());
        assert_eq!(method_name(4), "Level up");
        assert_eq!(argument_of(4), Argument::Level);
        assert_eq!(argument_of(8), Argument::Item);
        assert_eq!(argument_of(21), Argument::Move);
        assert_eq!(argument_of(29), Argument::Type);
        assert_eq!((method_name(0), argument_of(0), argument_of(99)), ("-", Argument::None, Argument::None));
    }

    /// With the dump of the game: every file is read and written back the same, and the evolutions that
    /// Bulbapedia gives for Bulbasaur and Eevee (Generation VI) are there.
    #[test]
    fn the_evolutions_of_the_dump_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let garc = crate::Garc::open(&dump.archive(ARCHIVE)).unwrap();
        assert_eq!(garc.file_count(), 826);
        for id in 0..garc.file_count() {
            let bytes = garc.first_file(id).unwrap();
            assert_eq!(Evolutions::from_bytes(bytes).unwrap().to_bytes(), bytes, "file {id}");
        }
        let read = |species: usize| Evolutions::from_bytes(garc.first_file(species).unwrap()).unwrap().list;
        // Bulbasaur evolves into Ivysaur at the level 16, Ivysaur into Venusaur at the level 32.
        assert_eq!(read(1), [Evolution { method: 4, argument: 16, species: 2 }]);
        assert_eq!(read(2), [Evolution { method: 4, argument: 32, species: 3 }]);
        assert!(read(3).is_empty());
        // Eevee: Leafeon (Moss Rock), Glaceon (Ice Rock), Jolteon (Thunder Stone), Vaporeon (Water Stone),
        // Flareon (Fire Stone), Sylveon (Fairy move), Espeon (friendship, day), Umbreon (friendship, night).
        let eevee: Vec<(u16, u16)> = read(133).iter().map(|e| (e.method, e.species)).collect();
        assert_eq!(eevee, [(26, 470), (27, 471), (8, 135), (8, 134), (8, 136), (29, 700), (2, 196), (3, 197)]);
        // The argument of Sylveon is the type Fairy (17).
        assert_eq!(read(133)[5].argument, 17);
    }
}
