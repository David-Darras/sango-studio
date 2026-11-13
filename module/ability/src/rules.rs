//! The reactions of a new ability, as rules: "when this happens, if this is true, do this".
//!
//! A rule is data, not code. The plugin reads the file of the ability and runs the rules with its own reaction function, so
//! the user needs no compiler and a rule cannot crash the game. Every number of a rule is a value of an enum of the battle
//! engine (the moment, the statistic, the status condition, the weather, the terrain) or the number of a type or of a move.
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
//!
//! A **rule** is a 16-bit moment, a condition count, an action count, then the conditions (3 bytes each) and the actions
//! (7 bytes each).
//!
//! A **condition** is the byte of its kind and a 16-bit signed argument. An **action** is the byte of its kind, the byte of its
//! target, two 16-bit signed arguments `a` and `b`, and the byte of the flags (bit 0: show the banner of the ability).

use sango_core::text::LANGUAGES;

/// The first id of a new ability: the game has the abilities 1 to 191.
pub const FIRST_ID: u8 = 192;
/// The last id of an ability: an ability id is one byte.
pub const LAST_ID: u8 = 255;
/// The magic text of the file.
pub const MAGIC: &[u8; 4] = b"SABR";
/// The version of the layout of the file.
pub const VERSION: u8 = 1;
/// The most rules in one ability, the most conditions in one rule and the most actions in one rule.
pub const MAX_RULES: usize = 8;
pub const MAX_CONDITIONS: usize = 6;
pub const MAX_ACTIONS: usize = 6;

/// A moment of the battle where a rule can run. The numbers are the values of the moments of the engine.
pub struct Moment {
    pub value: u16,
    pub label: &'static str,
    pub help: &'static str,
}

/// The moments that the editor offers.
pub const MOMENTS: [Moment; 10] = [
    Moment { value: 94, label: "A Pokémon enters the battle", help: "Runs for every Pokémon that enters, yours or not. Use the condition \"It is this Pokémon\" to act only for the owner." },
    Moment { value: 148, label: "A Pokémon gets a new ability", help: "For example after Skill Swap. Add it next to \"enters the battle\" for an ability that acts on entry." },
    Moment { value: 36, label: "A move starts", help: "Before the move does anything." },
    Moment { value: 140, label: "A move did damage", help: "At the end of a damaging move, once a Pokémon took damage." },
    Moment { value: 176, label: "After a move", help: "After the action of a move." },
    Moment { value: 102, label: "After a stat change", help: "After a stat stage went up or down." },
    Moment { value: 135, label: "After the weather changes", help: "When the weather has just changed." },
    Moment { value: 127, label: "The end of the turn begins", help: "At the start of the end-of-turn checks." },
    Moment { value: 92, label: "After a Pokémon leaves", help: "After a Pokémon is withdrawn from the battle." },
    Moment { value: 175, label: "Right before fainting", help: "A Pokémon is about to faint." },
];

/// The kinds of condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConditionKind {
    /// The Pokémon of the moment is the owner of the ability.
    PokemonIsOwner = 1,
    /// The Pokémon that uses the move is the owner of the ability.
    MoveUserIsOwner = 2,
    /// The move has a type (argument: the number of the type).
    MoveTypeIs = 3,
    /// The weather is a weather (argument: the weather).
    WeatherIs = 4,
    /// At least one target of the move has fainted.
    TargetFainted = 5,
    /// The rule runs one time only in a battle.
    OnlyOnce = 6,
}

impl ConditionKind {
    pub const ALL: [ConditionKind; 6] = [ConditionKind::PokemonIsOwner, ConditionKind::MoveUserIsOwner, ConditionKind::MoveTypeIs, ConditionKind::WeatherIs, ConditionKind::TargetFainted, ConditionKind::OnlyOnce];

    pub fn from_byte(byte: u8) -> Option<ConditionKind> {
        ConditionKind::ALL.into_iter().find(|kind| *kind as u8 == byte)
    }

    pub fn label(self) -> &'static str {
        match self {
            ConditionKind::PokemonIsOwner => "It is this Pokémon",
            ConditionKind::MoveUserIsOwner => "This Pokémon used the move",
            ConditionKind::MoveTypeIs => "The move has the type",
            ConditionKind::WeatherIs => "The weather is",
            ConditionKind::TargetFainted => "A target of the move fainted",
            ConditionKind::OnlyOnce => "Only the first time in a battle",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            ConditionKind::PokemonIsOwner => "The moment is for every Pokémon in the battle. This keeps the rule for the Pokémon that has the ability.",
            ConditionKind::MoveUserIsOwner => "The Pokémon that has the ability is the one that used the move.",
            ConditionKind::MoveTypeIs => "Tests the type of the move that is used.",
            ConditionKind::WeatherIs => "Tests the weather of the battle now.",
            ConditionKind::TargetFainted => "At least one target of the move has no HP left.",
            ConditionKind::OnlyOnce => "The plugin counts in the memory of the listener: the rule is skipped after its first run.",
        }
    }

    /// What the argument of the condition is.
    pub fn argument(self) -> Argument {
        match self {
            ConditionKind::MoveTypeIs => Argument::Type,
            ConditionKind::WeatherIs => Argument::Weather,
            _ => Argument::None,
        }
    }
}

/// The kinds of action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ActionKind {
    /// Raises or lowers a statistic (a: the statistic, b: the number of stages).
    ChangeStat = 1,
    /// Sets the weather (a: the weather, b: 1 for a weather that never stops, 0 for 5 turns).
    SetWeather = 2,
    /// Sets a terrain (a: the terrain).
    SetTerrain = 3,
    /// Gives a status condition (a: the status condition).
    GiveStatus = 4,
    /// Heals a part of the maximum HP (a: the percent).
    Heal = 5,
    /// Changes a type (a: the number of the type).
    ChangeType = 6,
    /// The owner uses a move (a: the number of the move).
    UseMove = 7,
}

impl ActionKind {
    pub const ALL: [ActionKind; 7] = [ActionKind::ChangeStat, ActionKind::SetWeather, ActionKind::SetTerrain, ActionKind::GiveStatus, ActionKind::Heal, ActionKind::ChangeType, ActionKind::UseMove];

    pub fn from_byte(byte: u8) -> Option<ActionKind> {
        ActionKind::ALL.into_iter().find(|kind| *kind as u8 == byte)
    }

    pub fn label(self) -> &'static str {
        match self {
            ActionKind::ChangeStat => "Change a stat",
            ActionKind::SetWeather => "Set the weather",
            ActionKind::SetTerrain => "Set a terrain",
            ActionKind::GiveStatus => "Give a status condition",
            ActionKind::Heal => "Heal HP",
            ActionKind::ChangeType => "Change the type",
            ActionKind::UseMove => "Use a move",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            ActionKind::ChangeStat => "Raises or lowers a stat by some stages. A negative number lowers it.",
            ActionKind::SetWeather => "Changes the weather. It lasts 5 turns, or never stops.",
            ActionKind::SetTerrain => "Generation VI has the Grassy, Misty and Electric terrains only.",
            ActionKind::GiveStatus => "Burn, poison, sleep, confusion...",
            ActionKind::Heal => "Gives back a part of the maximum HP.",
            ActionKind::ChangeType => "The Pokémon gets this type.",
            ActionKind::UseMove => "The Pokémon uses a move, without spending PP.",
        }
    }

    /// What the first argument of the action is.
    pub fn first(self) -> Argument {
        match self {
            ActionKind::ChangeStat => Argument::Stat,
            ActionKind::SetWeather => Argument::Weather,
            ActionKind::SetTerrain => Argument::Terrain,
            ActionKind::GiveStatus => Argument::Status,
            ActionKind::Heal => Argument::Percent,
            ActionKind::ChangeType => Argument::Type,
            ActionKind::UseMove => Argument::Move,
        }
    }

    /// What the second argument of the action is.
    pub fn second(self) -> Argument {
        match self {
            ActionKind::ChangeStat => Argument::Stages,
            ActionKind::SetWeather => Argument::Forever,
            _ => Argument::None,
        }
    }

    /// True when the action has a target that the user can choose.
    pub fn has_target(self) -> bool {
        matches!(self, ActionKind::ChangeStat | ActionKind::GiveStatus | ActionKind::Heal | ActionKind::ChangeType)
    }
}

/// The meaning of an argument: the editor shows a menu or a number box for each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    None,
    Stat,
    Weather,
    Terrain,
    Status,
    Type,
    Percent,
    Move,
    Stages,
    Forever,
}

/// The statistics of a stat change. The value is the number of the engine (`StatStageEffectKind`).
pub const STATS: [(i16, &str); 8] = [(1, "Attack"), (2, "Defense"), (3, "Sp. Atk"), (4, "Sp. Def"), (5, "Speed"), (6, "Accuracy"), (7, "Evasion"), (9, "All the stats")];

/// The weathers. The value is the number of the engine (`Weather`).
pub const WEATHERS: [(i16, &str); 7] = [(1, "Harsh sunlight"), (2, "Rain"), (3, "Hail"), (4, "Sandstorm"), (5, "Heavy rain"), (6, "Extremely harsh sunlight"), (7, "Strong winds")];

/// The terrains. The value is the number of the engine (`TerrainKind`).
pub const TERRAINS: [(i16, &str); 3] = [(1, "Grassy Terrain"), (2, "Misty Terrain"), (3, "Electric Terrain")];

/// The status conditions that make sense for a rule. The value is the number of the engine (`StatusCondition`).
pub const STATUSES: [(i16, &str); 12] = [
    (1, "Paralysis"),
    (2, "Sleep"),
    (3, "Freeze"),
    (4, "Burn"),
    (5, "Poison"),
    (6, "Confusion"),
    (7, "Infatuation"),
    (11, "Taunt"),
    (12, "Torment"),
    (15, "Heal Block"),
    (18, "Leech Seed"),
    (19, "Embargo"),
];

/// Who an action is for. The value is stored in the file.
pub const TARGETS: [(u8, &str, &str); 4] = [
    (0, "this Pokémon", "The Pokémon that has the ability."),
    (1, "the Pokémon of the moment", "The Pokémon that the moment is about: the one that enters, or the one that is hit."),
    (2, "the Pokémon that used the move", "Only for the moments of a move."),
    (3, "the first target of the move", "Only for the moments of a move."),
];

/// A test of a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub kind: ConditionKind,
    pub arg: i16,
}

/// A thing that a rule does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub target: u8,
    pub a: i16,
    pub b: i16,
    /// Shows the banner of the ability.
    pub banner: bool,
}

impl Action {
    /// A new action with values that make sense.
    pub fn new(kind: ActionKind) -> Action {
        let (a, b) = match kind {
            ActionKind::ChangeStat => (5, 1),
            ActionKind::SetWeather => (2, 0),
            ActionKind::SetTerrain => (3, 0),
            ActionKind::GiveStatus => (4, 0),
            ActionKind::Heal => (25, 0),
            ActionKind::ChangeType => (0, 0),
            ActionKind::UseMove => (1, 0),
        };
        Action { kind, target: 0, a, b, banner: true }
    }
}

/// One rule: a moment, the conditions that must all be true, and the actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub moment: u16,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
}

impl Rule {
    /// A new rule that runs when the Pokémon enters the battle.
    pub fn new() -> Rule {
        Rule { moment: MOMENTS[0].value, conditions: vec![Condition { kind: ConditionKind::PokemonIsOwner, arg: 0 }], actions: vec![Action::new(ActionKind::ChangeStat)] }
    }
}

impl Default for Rule {
    fn default() -> Self {
        Rule::new()
    }
}

/// A new ability: the id, the texts in the eight languages and the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomAbility {
    pub id: u8,
    /// The name in each language, in the order of [`LANGUAGES`].
    pub names: [String; 8],
    pub descriptions: [String; 8],
    pub rules: Vec<Rule>,
}

/// The English language: its position in [`LANGUAGES`].
pub const ENGLISH: usize = 2;

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
        if self.rules.len() > MAX_RULES {
            problems.push(format!("An ability has {MAX_RULES} reactions at most."));
        }
        for (number, rule) in self.rules.iter().enumerate() {
            let at = format!("Reaction {}", number + 1);
            if rule.actions.is_empty() {
                problems.push(format!("{at} does nothing: add an action."));
            }
            if rule.conditions.len() > MAX_CONDITIONS || rule.actions.len() > MAX_ACTIONS {
                problems.push(format!("{at} has too many conditions or actions."));
            }
            let move_moment = matches!(rule.moment, 36 | 140 | 176);
            for condition in &rule.conditions {
                if !move_moment && matches!(condition.kind, ConditionKind::MoveUserIsOwner | ConditionKind::MoveTypeIs | ConditionKind::TargetFainted) {
                    problems.push(format!("{at}: \"{}\" needs a moment of a move.", condition.kind.label()));
                }
            }
            for action in &rule.actions {
                if !move_moment && matches!(action.target, 2 | 3) {
                    problems.push(format!("{at}: \"{}\" needs a moment of a move.", target_label(action.target)));
                }
                if action.kind == ActionKind::ChangeStat && action.b == 0 {
                    problems.push(format!("{at}: a stat change of 0 stages does nothing."));
                }
            }
        }
        problems
    }

    /// The file. The ability must have no problem.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&[VERSION, self.id, self.rules.len() as u8, 0]);
        for language in 0..8 {
            for text in [&self.names[language], &self.descriptions[language]] {
                let units: Vec<u16> = text.encode_utf16().collect();
                out.extend_from_slice(&(units.len() as u16).to_le_bytes());
                for unit in units {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
            }
        }
        for rule in &self.rules {
            out.extend_from_slice(&rule.moment.to_le_bytes());
            out.push(rule.conditions.len() as u8);
            out.push(rule.actions.len() as u8);
            for condition in &rule.conditions {
                out.push(condition.kind as u8);
                out.extend_from_slice(&condition.arg.to_le_bytes());
            }
            for action in &rule.actions {
                out.push(action.kind as u8);
                out.push(action.target);
                out.extend_from_slice(&action.a.to_le_bytes());
                out.extend_from_slice(&action.b.to_le_bytes());
                out.push(action.banner as u8);
            }
        }
        out
    }

    /// Reads a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<CustomAbility, String> {
        let mut at = Reader { bytes, at: 0 };
        if at.take(4)? != MAGIC {
            return Err("This is not a file of rules: the text SABR is missing.".into());
        }
        let head = at.take(4)?;
        if head[0] != VERSION {
            return Err(format!("The version {} of the file is not known.", head[0]));
        }
        let (id, count) = (head[1], head[2] as usize);
        let mut names: [String; 8] = Default::default();
        let mut descriptions: [String; 8] = Default::default();
        for language in 0..8 {
            names[language] = at.text()?;
            descriptions[language] = at.text()?;
        }
        let mut rules = Vec::new();
        for _ in 0..count {
            let head = at.take(4)?;
            let moment = u16::from_le_bytes([head[0], head[1]]);
            let (conditions, actions) = (head[2] as usize, head[3] as usize);
            let mut rule = Rule { moment, conditions: Vec::new(), actions: Vec::new() };
            for _ in 0..conditions {
                let raw = at.take(3)?;
                let kind = ConditionKind::from_byte(raw[0]).ok_or_else(|| format!("The condition {} is not known.", raw[0]))?;
                rule.conditions.push(Condition { kind, arg: i16::from_le_bytes([raw[1], raw[2]]) });
            }
            for _ in 0..actions {
                let raw = at.take(7)?;
                let kind = ActionKind::from_byte(raw[0]).ok_or_else(|| format!("The action {} is not known.", raw[0]))?;
                rule.actions.push(Action { kind, target: raw[1], a: i16::from_le_bytes([raw[2], raw[3]]), b: i16::from_le_bytes([raw[4], raw[5]]), banner: raw[6] & 1 != 0 });
            }
            rules.push(rule);
        }
        Ok(CustomAbility { id, names, descriptions, rules })
    }

    /// The rules as sentences in English, one block for each rule.
    pub fn preview(&self) -> Vec<String> {
        self.rules.iter().map(rule_sentence).collect()
    }
}

/// A reader of the bytes of a file.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(count).filter(|end| *end <= self.bytes.len()).ok_or("The file ends too early.")?;
        let part = &self.bytes[self.at..end];
        self.at = end;
        Ok(part)
    }

    fn text(&mut self) -> Result<String, String> {
        let length = self.take(2)?;
        let length = u16::from_le_bytes([length[0], length[1]]) as usize;
        let raw = self.take(length * 2)?;
        let units: Vec<u16> = raw.chunks(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        String::from_utf16(&units).map_err(|_| "A text of the file is not valid.".to_string())
    }
}

fn find(list: &[(i16, &'static str)], value: i16) -> &'static str {
    list.iter().find(|(v, _)| *v == value).map(|(_, name)| *name).unwrap_or("?")
}

/// The name of a type from its number.
pub fn type_name(value: i16) -> &'static str {
    usize::try_from(value).ok().and_then(|n| sango_species::TYPE_NAMES.get(n).copied()).unwrap_or("?")
}

/// The name of a target.
pub fn target_label(target: u8) -> &'static str {
    TARGETS.iter().find(|(v, _, _)| *v == target).map(|(_, name, _)| *name).unwrap_or("?")
}

/// The name of a moment.
pub fn moment_label(moment: u16) -> &'static str {
    MOMENTS.iter().find(|m| m.value == moment).map(|m| m.label).unwrap_or("Another moment")
}

/// The text with its first letter in lower case.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_lowercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

/// A condition as an English phrase.
pub fn condition_sentence(condition: &Condition) -> String {
    match condition.kind {
        ConditionKind::PokemonIsOwner => "it is this Pokémon".into(),
        ConditionKind::MoveUserIsOwner => "this Pokémon used the move".into(),
        ConditionKind::MoveTypeIs => format!("the move is a {} move", type_name(condition.arg)),
        ConditionKind::WeatherIs => format!("the weather is {}", find(&WEATHERS, condition.arg).to_lowercase()),
        ConditionKind::TargetFainted => "a target of the move fainted".into(),
        ConditionKind::OnlyOnce => "it is the first time in the battle".into(),
    }
}

/// An action as an English phrase.
pub fn action_sentence(action: &Action) -> String {
    let who = target_label(action.target);
    let text = match action.kind {
        ActionKind::ChangeStat => {
            let stat = find(&STATS, action.a);
            if action.b >= 0 {
                format!("raise the {stat} of {who} by {} {}", action.b, if action.b == 1 { "stage" } else { "stages" })
            } else {
                format!("lower the {stat} of {who} by {} {}", -action.b, if action.b == -1 { "stage" } else { "stages" })
            }
        }
        ActionKind::SetWeather => format!("set the weather to {}{}", find(&WEATHERS, action.a).to_lowercase(), if action.b != 0 { ", for ever" } else { " for 5 turns" }),
        ActionKind::SetTerrain => format!("set {}", find(&TERRAINS, action.a)),
        ActionKind::GiveStatus => format!("give {} to {who}", find(&STATUSES, action.a).to_lowercase()),
        ActionKind::Heal => format!("heal {who} by {}% of its maximum HP", action.a),
        ActionKind::ChangeType => format!("change the type of {who} to {}", type_name(action.a)),
        ActionKind::UseMove => format!("this Pokémon uses move {}", action.a),
    };
    if action.banner {
        format!("{text} (the banner shows)")
    } else {
        text
    }
}

/// A rule as a sentence: "When …, if …, then …".
pub fn rule_sentence(rule: &Rule) -> String {
    let mut text = format!("When {}", lower_first(moment_label(rule.moment)));
    if !rule.conditions.is_empty() {
        let list: Vec<String> = rule.conditions.iter().map(condition_sentence).collect();
        text.push_str(&format!(", and {}", list.join(", and ")));
    }
    let list: Vec<String> = rule.actions.iter().map(action_sentence).collect();
    text.push_str(&format!(": {}.", list.join(", then ")));
    text
}

/// The name of a language, for the labels of the editor.
pub fn language_name(index: usize) -> &'static str {
    LANGUAGES.get(index).map(|l| l.name).unwrap_or("?")
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
            conditions: vec![Condition { kind: ConditionKind::OnlyOnce, arg: 0 }],
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
    }
}
