//! A new ability: its id, its texts in the eight languages and its rules (see the crate `sango-rules` for the rules).
//!
//! # The file `ability/<id>.bin`
//!
//! All the numbers are little-endian.
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 4 | The text `SABR`. |
//! | 4 | 1 | The version of the layout: 1. |
//! | 5 | 1 | The id of the ability (192 to 255). |
//! | 6 | 1 | The number of rules. |
//! | 7 | 1 | Always 0. |
//! | 8 | … | For each of the eight languages, the name then the description. A text is a 16-bit length (in UTF-16 units) and the UTF-16 units. |
//! | … | … | The rules. |

pub use sango_rules::*;


/// The first id of a new ability: the game has the abilities 1 to 191.
pub const FIRST_ID: u8 = 192;
/// The last id of an ability: an ability id is one byte.
pub const LAST_ID: u8 = 255;
/// The magic text of the file.
pub const MAGIC: &[u8; 4] = b"SABR";
/// The version of the layout of the file.
pub const VERSION: u8 = 1;
/// The most new abilities: the plugin keeps this number of abilities of the studio at the same time.
pub const MAX_ABILITIES: usize = 32;
/// The English language: its position in [`LANGUAGES`].
pub const ENGLISH: usize = 2;

/// A new ability: the id, the texts in the eight languages and the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomAbility {
    pub id: u8,
    /// The name in each language, in the order of [`LANGUAGES`].
    pub names: [String; 8],
    pub descriptions: [String; 8],
    pub rules: Vec<Rule>,
}

impl CustomAbility {
    /// A new ability. The English name and description are the ones that the user gave; the other languages start with the
    /// same text, so that the game never shows an empty line.
    pub fn new(id: u8, name: &str, description: &str) -> CustomAbility {
        CustomAbility { id, names: std::array::from_fn(|_| name.to_string()), descriptions: std::array::from_fn(|_| description.to_string()), rules: vec![Rule::new()] }
    }

    /// The place of the file on the SD card.
    pub fn path(id: u8) -> String {
        format!("ability/{id}.bin")
    }

    /// The first id from 192 that is not in `used`.
    pub fn free_id(used: &[u8]) -> Option<u8> {
        (FIRST_ID..=LAST_ID).find(|id| !used.contains(id))
    }

    /// What is wrong with the ability, in words that the user understands. An empty list means the file can be written.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.id < FIRST_ID {
            problems.push(format!("The id must be from {FIRST_ID} to {LAST_ID}: the game has the abilities 1 to 191."));
        }
        if self.names[ENGLISH].trim().is_empty() {
            problems.push("The English name is empty.".into());
        }
        problems.extend(sango_rules::problems(&self.rules, Context::Ability));
        problems
    }

    /// The file. The ability must have no problem.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&[VERSION, self.id, self.rules.len() as u8, 0]);
        for language in 0..8 {
            write_text(&mut out, &self.names[language]);
            write_text(&mut out, &self.descriptions[language]);
        }
        write_rules(&mut out, &self.rules);
        out
    }

    /// Reads a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<CustomAbility, String> {
        let mut reader = Reader::new(bytes);
        if reader.take(4)? != MAGIC {
            return Err("This is not a file of rules: the text SABR is missing.".into());
        }
        let head = reader.take(4)?;
        if head[0] != VERSION {
            return Err(format!("The version {} of the file is not known.", head[0]));
        }
        let (id, count) = (head[1], head[2] as usize);
        let mut names: [String; 8] = Default::default();
        let mut descriptions: [String; 8] = Default::default();
        for language in 0..8 {
            names[language] = reader.text()?;
            descriptions[language] = reader.text()?;
        }
        let rules = read_rules(&mut reader, count)?;
        Ok(CustomAbility { id, names, descriptions, rules })
    }

    /// The rules as sentences in English, one block for each rule.
    pub fn preview(&self) -> Vec<String> {
        self.rules.iter().map(rule_sentence).collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Quick Start of the tutorial of the plugin: the Speed goes up by one stage when the Pokémon enters the battle.
    fn quick_start() -> CustomAbility {
        CustomAbility::new(230, "Quick Start", "Raises the Speed of the Pokémon\nwhen it enters a battle.")
    }

    #[test]
    fn the_default_rule_is_quick_start() {
        let ability = quick_start();
        assert!(ability.problems().is_empty());
        assert_eq!(ability.preview(), vec!["When a Pokémon enters the battle, and it is this Pokémon: raise the Speed of this Pokémon by 1 stage (the banner shows).".to_string()]);
    }

    #[test]
    fn a_file_reads_back_the_same() {
        let mut ability = quick_start();
        ability.names[3] = "Démarrage rapide".into();
        ability.rules.push(Rule {
            moment: 148,
            conditions: vec![Condition { kind: ConditionKind::OnlyOnce, arg: 0 }, Condition { kind: ConditionKind::HpBelow, arg: 50 }],
            actions: vec![Action::new(ActionKind::SetTerrain), Action { kind: ActionKind::ChangeStat, target: 1, a: 2, b: -2, banner: false }],
        });
        let bytes = ability.to_bytes();
        assert_eq!(&bytes[..4], b"SABR");
        assert_eq!(CustomAbility::from_bytes(&bytes).unwrap(), ability);
        assert!(CustomAbility::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(CustomAbility::from_bytes(b"NOPE0000").is_err());
    }

    #[test]
    fn the_file_layout_is_the_documented_one() {
        let bytes = quick_start().to_bytes();
        assert_eq!(&bytes[4..8], &[1, 230, 1, 0]);
        // The first text is the name in Japanese: 11 units, as in English.
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 11);
        // The rule is at the end: moment 94, one condition, one action of 7 bytes.
        let rule = &bytes[bytes.len() - (4 + 3 + 7)..];
        assert_eq!(rule, &[94, 0, 1, 1, 1, 0, 0, 1, 0, 5, 0, 1, 0, 1]);
    }

    #[test]
    fn the_problems_are_found() {
        let mut ability = quick_start();
        ability.id = 100;
        ability.names[ENGLISH] = " ".into();
        ability.rules[0].moment = 94;
        ability.rules[0].conditions.push(Condition { kind: ConditionKind::MoveTypeIs, arg: 10 });
        ability.rules[0].actions[0].b = 0;
        ability.rules.push(Rule { moment: 94, conditions: vec![], actions: vec![] });
        let problems = ability.problems().join("\n");
        assert!(problems.contains("192 to 255"));
        assert!(problems.contains("English name"));
        assert!(problems.contains("needs a moment of a move"));
        assert!(problems.contains("0 stages"));
        assert!(problems.contains("Reaction 2 does nothing"));
    }

    #[test]
    fn a_free_id_skips_the_used_ones() {
        assert_eq!(CustomAbility::free_id(&[]), Some(192));
        assert_eq!(CustomAbility::free_id(&[192, 193, 195]), Some(194));
        let all: Vec<u8> = (FIRST_ID..=LAST_ID).collect();
        assert_eq!(CustomAbility::free_id(&all), None);
    }

    #[test]
    fn the_sentences_name_the_values() {
        let weather = Action { kind: ActionKind::SetWeather, target: 0, a: 4, b: 1, banner: true };
        assert_eq!(action_sentence(&weather), "set the weather to sandstorm, for ever (the banner shows)");
        let fire = Condition { kind: ConditionKind::MoveTypeIs, arg: 9 };
        assert_eq!(condition_sentence(&fire), "the move is a Fire move");
        assert_eq!(type_name(99), "?");
        let heal = Action { kind: ActionKind::Heal, target: 2, a: 25, b: 0, banner: false };
        assert_eq!(action_sentence(&heal), "heal the Pokémon that used the move by 25% of its maximum HP");
        assert_eq!(condition_sentence(&Condition { kind: ConditionKind::HpBelow, arg: 50 }), "its HP are under 50% of the maximum");
    }

    #[test]
    fn a_move_and_an_item_have_their_own_moments() {
        assert_eq!(moments(Context::Move)[0].value, 36);
        assert_eq!(moments(Context::Item)[0].value, 94);
        assert_eq!(moment_label(85), "The move hit all its targets");
        let rule = Rule { moment: 36, conditions: vec![Condition { kind: ConditionKind::MoveUserIsOwner, arg: 0 }], actions: vec![Action::new(ActionKind::Heal)] };
        assert!(sango_rules::problems(&[rule], Context::Move).is_empty());
    }
}
