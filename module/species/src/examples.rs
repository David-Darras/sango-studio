//! Examples for the help of the fields: which species use a value, and the lowest and the highest one.
//!
//! The examples come from the data of the game as it is in the romfs, form 0 of each species.
//! They do not change when the user edits a species.

use crate::data::SpeciesData;
use crate::names;

/// The most species that the help lists for one value.
pub const MAX_LISTED: usize = 5;

/// A field of the General tab that has examples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Hp,
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
    Type1,
    Type2,
    Ability1,
    Ability2,
    HiddenAbility,
    EggGroup1,
    EggGroup2,
    HeldItem50,
    HeldItem5,
    LevelingRate,
    Shape,
    Color,
    GenderRatio,
    HatchTime,
    Height,
    Weight,
    CatchRate,
    BaseFriendship,
    BaseExperience,
    BurrowedEntry,
    EvHp,
    EvAttack,
    EvDefense,
    EvSpecialAttack,
    EvSpecialDefense,
    EvSpeed,
}

impl Field {
    /// The value of the field for a species.
    fn value(self, d: &SpeciesData, shape: u8) -> u32 {
        match self {
            Field::Hp => d.base_hp as u32,
            Field::Attack => d.base_attack as u32,
            Field::Defense => d.base_defense as u32,
            Field::SpecialAttack => d.base_special_attack as u32,
            Field::SpecialDefense => d.base_special_defense as u32,
            Field::Speed => d.base_speed as u32,
            Field::Type1 => d.type_1 as u32,
            Field::Type2 => d.type_2 as u32,
            Field::Ability1 => d.ability_1 as u32,
            Field::Ability2 => d.ability_2 as u32,
            Field::HiddenAbility => d.ability_hidden as u32,
            Field::EggGroup1 => d.egg_group_1 as u32,
            Field::EggGroup2 => d.egg_group_2 as u32,
            Field::HeldItem50 => d.item_50 as u32,
            Field::HeldItem5 => d.item_5 as u32,
            Field::LevelingRate => d.leveling_rate as u32,
            Field::Shape => shape as u32,
            Field::Color => d.color as u32,
            Field::GenderRatio => d.gender_ratio as u32,
            Field::HatchTime => d.hatch_time as u32,
            Field::Height => d.height as u32,
            Field::Weight => d.weight as u32,
            Field::CatchRate => d.catch_rate as u32,
            Field::BaseFriendship => d.base_friendship as u32,
            Field::BaseExperience => d.base_experience as u32,
            Field::BurrowedEntry => d.is_burrowed_entry() as u32,
            Field::EvHp => d.ev_hp() as u32,
            Field::EvAttack => d.ev_attack() as u32,
            Field::EvDefense => d.ev_defense() as u32,
            Field::EvSpecialAttack => d.ev_special_attack() as u32,
            Field::EvSpecialDefense => d.ev_special_defense() as u32,
            Field::EvSpeed => d.ev_speed() as u32,
        }
    }

    /// True for a field where the value 0 means "nothing": the help lists no species for it.
    fn zero_is_nothing(self) -> bool {
        matches!(
            self,
            Field::Ability2
                | Field::HiddenAbility
                | Field::EggGroup2
                | Field::HeldItem50
                | Field::HeldItem5
                | Field::BurrowedEntry
                | Field::EvHp
                | Field::EvAttack
                | Field::EvDefense
                | Field::EvSpecialAttack
                | Field::EvSpecialDefense
                | Field::EvSpeed
        )
    }

    /// True for a field that is a quantity: the help gives the lowest and the highest value.
    fn is_quantity(self) -> bool {
        matches!(
            self,
            Field::Hp
                | Field::Attack
                | Field::Defense
                | Field::SpecialAttack
                | Field::SpecialDefense
                | Field::Speed
                | Field::HatchTime
                | Field::Height
                | Field::Weight
                | Field::CatchRate
                | Field::BaseFriendship
                | Field::BaseExperience
        )
    }

    /// A value of the field as text, with its unit.
    pub fn format(self, value: u32) -> String {
        match self {
            Field::Height => format!("{:.2} m", value as f32 / 100.0),
            Field::Weight => format!("{:.1} kg", value as f32 / 10.0),
            _ => value.to_string(),
        }
    }
}

/// What the help shows for a value of a field.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Examples {
    /// The first species that have the value, without the species being edited. At most `MAX_LISTED`.
    pub species: Vec<usize>,
    /// How many more species have the value.
    pub more: usize,
    /// The species with the lowest value, and the value. Only for a quantity.
    pub lowest: Option<(usize, u32)>,
    /// The species with the highest value, and the value. Only for a quantity.
    pub highest: Option<(usize, u32)>,
}

impl Examples {
    /// The name of a species of the list.
    pub fn name(species: usize) -> &'static str {
        species.checked_sub(1).and_then(|i| names::SPECIES.get(i)).copied().unwrap_or("-")
    }

    /// True when there is nothing to show.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.species.is_empty() && self.lowest.is_none() && self.highest.is_none()
    }
}

/// The data of all the species of the romfs, form 0.
#[derive(Debug, Default)]
pub struct Index {
    /// The number of the species, its data and its shape. In the order of the numbers.
    rows: Vec<(usize, SpeciesData, u8)>,
}

impl Index {
    /// Builds the index from the data of the species and the shape of each one.
    pub fn new(rows: Vec<(usize, SpeciesData, u8)>) -> Index {
        Index { rows }
    }

    /// The examples for a value of a field. `current` is the species that the user edits.
    pub fn examples(&self, field: Field, value: u32, current: usize) -> Examples {
        let mut result = Examples::default();
        if !(field.zero_is_nothing() && value == 0) {
            let mut same = self.rows.iter().filter(|(n, d, s)| *n != current && field.value(d, *s) == value).map(|(n, _, _)| *n);
            result.species = same.by_ref().take(MAX_LISTED).collect();
            result.more = same.count();
        }
        if field.is_quantity() {
            let lowest = self.rows.iter().map(|(n, d, s)| (field.value(d, *s), *n)).min_by_key(|&(v, n)| (v, n));
            let highest = self.rows.iter().map(|(n, d, s)| (field.value(d, *s), *n)).min_by_key(|&(v, n)| (std::cmp::Reverse(v), n));
            result.lowest = lowest.map(|(v, n)| (n, v));
            result.highest = highest.map(|(v, n)| (n, v));
        }
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn row(number: usize, hp: u8, shape: u8) -> (usize, SpeciesData, u8) {
        let mut d = SpeciesData::from_bytes(&[0; 80]).unwrap();
        d.base_hp = hp;
        (number, d, shape)
    }

    fn sample() -> Index {
        Index::new((1..=8).map(|n| row(n, if n % 2 == 0 { 50 } else { 90 }, n as u8)).collect())
    }

    #[test]
    fn species_with_the_same_value_are_listed_without_the_current_one() {
        let e = sample().examples(Field::Hp, 90, 1);
        assert_eq!(e.species, [3, 5, 7]);
        assert_eq!(e.more, 0);
    }

    #[test]
    fn the_list_is_cut_and_the_rest_is_counted() {
        let index = Index::new((1..=12).map(|n| row(n, 50, 0)).collect());
        let e = index.examples(Field::Hp, 50, 3);
        assert_eq!(e.species, [1, 2, 4, 5, 6]);
        assert_eq!(e.more, 6);
    }

    #[test]
    fn a_quantity_gives_the_lowest_and_the_highest_species() {
        let e = sample().examples(Field::Hp, 90, 1);
        assert_eq!(e.lowest, Some((2, 50)));
        assert_eq!(e.highest, Some((1, 90)));
        // A field that is not a quantity gives none.
        assert_eq!(sample().examples(Field::Shape, 3, 1).lowest, None);
    }

    #[test]
    fn the_value_zero_of_an_optional_field_lists_nothing() {
        let index = sample();
        assert!(index.examples(Field::HeldItem5, 0, 1).is_empty());
        assert!(index.examples(Field::EvHp, 0, 1).is_empty());
        // The value 0 of a field where it is a real value lists species.
        assert!(!index.examples(Field::Type1, 0, 1).is_empty());
    }

    #[test]
    fn the_shape_comes_from_the_second_part_of_the_row() {
        assert_eq!(sample().examples(Field::Shape, 4, 1).species, [4]);
    }

    #[test]
    fn values_have_their_unit() {
        assert_eq!(Field::Height.format(145), "1.45 m");
        assert_eq!(Field::Weight.format(9500), "950.0 kg");
        assert_eq!(Field::Hp.format(255), "255");
        assert_eq!(Examples::name(1), "Bulbasaur");
        assert_eq!(Examples::name(0), "-");
    }

    /// With the dump of the game: the extreme species are the ones of Bulbapedia.
    #[test]
    fn the_extremes_of_the_dump_are_the_ones_of_bulbapedia() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let garc = sango_core::Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let pokedex = sango_core::Garc::open(&dump.archive(crate::pokedex_data::ARCHIVE)).unwrap();
        let pokedex = crate::pokedex_data::PokedexData::from_bytes(pokedex.first_file(crate::pokedex_data::FILE).unwrap().to_vec()).unwrap();
        let index = crate::build_index(&garc, &pokedex);
        let e = index.examples(Field::Hp, 100, 1);
        assert_eq!(e.highest, Some((242, 255))); // Blissey
        assert_eq!(e.lowest, Some((292, 1))); // Shedinja
        assert_eq!(index.examples(Field::Defense, 100, 1).highest, Some((213, 230))); // Shuckle
        assert_eq!(index.examples(Field::Height, 100, 1).highest, Some((321, 1450))); // Wailord
        assert_eq!(index.examples(Field::Weight, 100, 1).highest, Some((383, 9500))); // Groudon
        // Diglett and Dugtrio come out of the ground.
        assert_eq!(index.examples(Field::BurrowedEntry, 1, 0).species, [50, 51]);
        // Only Blissey has 255 base HP.
        let only = index.examples(Field::Hp, 255, 0);
        assert_eq!((only.species, only.more), (vec![242], 0));
    }
}
