//! The rules of the new abilities, moves and items: "when this happens, if this is true, do this".
//!
//! A rule is data, not code. The plugin reads the file of the ability, of the move or of the item and runs the rules with its own
//! reaction function, so the user needs no compiler and a rule cannot crash the game. Every number of a rule is a value of an enum
//! of the battle engine (the moment, the statistic, the status condition, the weather, the terrain) or the number of a type or of a
//! move.
//!
//! The layout of the rules in a file (all the numbers are little-endian): a **rule** is a 16-bit moment, a condition count, an
//! action count, then the conditions (3 bytes each) and the actions (7 bytes each). A **condition** is the byte of its kind and a
//! 16-bit signed argument. An **action** is the byte of its kind, the byte of its target, two 16-bit signed arguments `a` and `b`,
//! and the byte of the flags (bit 0: show the banner of the ability).

pub mod editor;

use sango_core::text::LANGUAGES;

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
pub const MOMENTS: [Moment; 19] = [
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
    Moment { value: 61, label: "The power of a move is calculated", help: "Before the damage. Use \"Multiply the value\" to raise or lower the power of the move (the Muscle Band, the type plates)." },
    Moment { value: 64, label: "The attack stat is calculated", help: "The Attack or the Sp. Atk of the attacker, before the damage (the Choice Band, the Light Ball). Use the condition \"The move is\" to choose physical or special moves." },
    Moment { value: 65, label: "The defense stat is calculated", help: "The Defense or the Sp. Def of the target, before the damage (the Assault Vest, the Eviolite). Use the condition \"This Pokémon is the target of the move\"." },
    Moment { value: 79, label: "The damage is calculated", help: "After the type effectiveness: multiplies the final damage (the Life Orb, the Expert Belt with \"The move is super effective\")." },
    Moment { value: 19, label: "The speed is calculated", help: "The speed that decides who moves first (the Choice Scarf, the Iron Ball). Use the condition \"It is this Pokémon\"." },
    Moment { value: 57, label: "The accuracy is calculated", help: "Multiplies the chance to hit (the Wide Lens, the Zoom Lens)." },
    Moment { value: 59, label: "A critical hit is checked", help: "Use \"Raise the critical-hit stage\" (the Scope Lens, the Razor Claw)." },
    Moment { value: 83, label: "A Pokémon took damage from a move", help: "Runs for the Pokémon that was hit, after the damage (the Rocky Helmet, the Sticky Barb). Use the condition \"This Pokémon is the target of the move\"." },
    Moment { value: 128, label: "The end of the turn ends", help: "At the end of the end-of-turn checks (the Leftovers, the Black Sludge)." },
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
    /// The HP of the owner are under a part of its maximum HP (argument: the percent).
    HpBelow = 7,
    /// The Pokémon that receives the move is the owner of the ability.
    DefenderIsOwner = 8,
    /// The move is physical, special or a status move (argument: 1, 2 or 0).
    MoveCategoryIs = 9,
    /// The move is super effective against its target.
    SuperEffective = 10,
    /// The target of the move has no substitute.
    NoSubstitute = 11,
    /// The HP of the owner are not full.
    HpNotFull = 12,
    /// The move makes contact.
    MoveMakesContact = 13,
}

impl ConditionKind {
    pub const ALL: [ConditionKind; 13] = [
        ConditionKind::PokemonIsOwner,
        ConditionKind::MoveUserIsOwner,
        ConditionKind::DefenderIsOwner,
        ConditionKind::MoveTypeIs,
        ConditionKind::MoveCategoryIs,
        ConditionKind::MoveMakesContact,
        ConditionKind::SuperEffective,
        ConditionKind::NoSubstitute,
        ConditionKind::WeatherIs,
        ConditionKind::TargetFainted,
        ConditionKind::OnlyOnce,
        ConditionKind::HpBelow,
        ConditionKind::HpNotFull,
    ];

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
            ConditionKind::HpBelow => "The HP are under",
            ConditionKind::DefenderIsOwner => "This Pokémon is the target of the move",
            ConditionKind::MoveCategoryIs => "The move is",
            ConditionKind::SuperEffective => "The move is super effective",
            ConditionKind::NoSubstitute => "The target has no substitute",
            ConditionKind::HpNotFull => "The HP are not full",
            ConditionKind::MoveMakesContact => "The move makes contact",
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
            ConditionKind::HpBelow => "The HP of the owner are under a part of its maximum HP.",
            ConditionKind::DefenderIsOwner => "The Pokémon that receives the move is the one that has the ability or holds the item.",
            ConditionKind::MoveCategoryIs => "Physical, special or status: the kind of the move.",
            ConditionKind::SuperEffective => "The type of the move is strong against the target (x2 or more).",
            ConditionKind::NoSubstitute => "The move did not hit a substitute.",
            ConditionKind::HpNotFull => "The owner has lost some HP.",
            ConditionKind::MoveMakesContact => "The move touches its target (the property Contact of the move).",
        }
    }

    /// What the argument of the condition is.
    pub fn argument(self) -> Argument {
        match self {
            ConditionKind::MoveTypeIs => Argument::Type,
            ConditionKind::WeatherIs => Argument::Weather,
            ConditionKind::HpBelow => Argument::Percent,
            ConditionKind::MoveCategoryIs => Argument::DamageKind,
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
    /// Multiplies the value that the moment calculates (b: the multiplier in percent, 150 is x1.5).
    MultiplyValue = 8,
    /// Removes a part of the maximum HP (a: the percent).
    DamageHp = 9,
    /// The item of the owner is used up.
    ConsumeItem = 10,
    /// Adds stages to the critical hit stage (a: the number of stages).
    RaiseCritical = 11,
}

impl ActionKind {
    pub const ALL: [ActionKind; 11] = [
        ActionKind::MultiplyValue,
        ActionKind::RaiseCritical,
        ActionKind::Heal,
        ActionKind::DamageHp,
        ActionKind::ChangeStat,
        ActionKind::GiveStatus,
        ActionKind::ConsumeItem,
        ActionKind::SetWeather,
        ActionKind::SetTerrain,
        ActionKind::ChangeType,
        ActionKind::UseMove,
    ];

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
            ActionKind::MultiplyValue => "Multiply the value",
            ActionKind::DamageHp => "Remove HP",
            ActionKind::ConsumeItem => "Use up the item",
            ActionKind::RaiseCritical => "Raise the critical-hit stage",
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
            ActionKind::MultiplyValue => "Multiplies the value that the moment calculates: the power, the attack, the defense, the damage, the speed or the accuracy (150 is x1.5, 50 is x0.5). Choose one of the moments that calculate a value.",
            ActionKind::DamageHp => "Removes a part of the maximum HP (the recoil of the Life Orb, the Rocky Helmet).",
            ActionKind::ConsumeItem => "The held item is used up, like a berry.",
            ActionKind::RaiseCritical => "Adds stages to the critical-hit stage of the move. Choose the moment \"A critical hit is checked\".",
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
            ActionKind::DamageHp => Argument::Percent,
            ActionKind::RaiseCritical => Argument::CritStages,
            ActionKind::MultiplyValue | ActionKind::ConsumeItem => Argument::None,
        }
    }

    /// What the second argument of the action is.
    pub fn second(self) -> Argument {
        match self {
            ActionKind::ChangeStat => Argument::Stages,
            ActionKind::SetWeather => Argument::Forever,
            ActionKind::MultiplyValue => Argument::Multiplier,
            _ => Argument::None,
        }
    }

    /// True when the action has a target that the user can choose.
    pub fn has_target(self) -> bool {
        matches!(self, ActionKind::ChangeStat | ActionKind::GiveStatus | ActionKind::Heal | ActionKind::ChangeType | ActionKind::DamageHp)
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
    /// A multiplier in percent.
    Multiplier,
    /// Physical, special or status.
    DamageKind,
    /// The stages of the critical-hit stage.
    CritStages,
}

/// The kinds of damage of a move. The value is the number of the engine.
pub const DAMAGE_KINDS: [(i16, &str); 3] = [(1, "physical"), (2, "special"), (0, "a status move")];

/// The moments where the engine calculates a value that a rule can multiply: the power, the attack, the defense, the damage, the speed and the
/// accuracy.
pub fn is_value_moment(moment: u16) -> bool {
    matches!(moment, 61 | 64 | 65 | 79 | 19 | 57)
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
            ActionKind::MultiplyValue => (0, 150),
            ActionKind::DamageHp => (10, 0),
            ActionKind::ConsumeItem => (0, 0),
            ActionKind::RaiseCritical => (1, 0),
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

/// Reads the bytes of a file one part after the other.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    /// The place of the next byte.
    pub fn position(&self) -> usize {
        self.at
    }

    pub fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(count).filter(|end| *end <= self.bytes.len()).ok_or("The file ends too early.")?;
        let part = &self.bytes[self.at..end];
        self.at = end;
        Ok(part)
    }

    pub fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    pub fn word(&mut self) -> Result<u16, String> {
        let raw = self.take(2)?;
        Ok(u16::from_le_bytes([raw[0], raw[1]]))
    }

    pub fn text(&mut self) -> Result<String, String> {
        let length = self.word()? as usize;
        let raw = self.take(length * 2)?;
        let units: Vec<u16> = raw.chunks(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        String::from_utf16(&units).map_err(|_| "A text of the file is not valid.".to_string())
    }
}

/// Writes a text as a 16-bit length (in UTF-16 units) and the units.
pub fn write_text(out: &mut Vec<u8>, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    out.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
}

/// Writes the rules: the layout in the header of this file.
pub fn write_rules(out: &mut Vec<u8>, rules: &[Rule]) {
    for rule in rules {
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
}

/// Reads `count` rules.
pub fn read_rules(reader: &mut Reader, count: usize) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    for _ in 0..count {
        let head = reader.take(4)?;
        let moment = u16::from_le_bytes([head[0], head[1]]);
        let (conditions, actions) = (head[2] as usize, head[3] as usize);
        let mut rule = Rule { moment, conditions: Vec::new(), actions: Vec::new() };
        for _ in 0..conditions {
            let raw = reader.take(3)?;
            let kind = ConditionKind::from_byte(raw[0]).ok_or_else(|| format!("The condition {} is not known.", raw[0]))?;
            rule.conditions.push(Condition { kind, arg: i16::from_le_bytes([raw[1], raw[2]]) });
        }
        for _ in 0..actions {
            let raw = reader.take(7)?;
            let kind = ActionKind::from_byte(raw[0]).ok_or_else(|| format!("The action {} is not known.", raw[0]))?;
            rule.actions.push(Action { kind, target: raw[1], a: i16::from_le_bytes([raw[2], raw[3]]), b: i16::from_le_bytes([raw[4], raw[5]]), banner: raw[6] & 1 != 0 });
        }
        rules.push(rule);
    }
    Ok(rules)
}

/// Where a rule runs: the reactions of an ability, of a move that is used, or of an item that a Pokémon holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Ability,
    Move,
    Item,
}

/// The moments of a move, as the engine numbers them.
pub const MOVE_MOMENTS: [Moment; 5] = [
    Moment { value: 36, label: "The move starts", help: "Before the move does anything." },
    Moment { value: 85, label: "The move hit all its targets", help: "After the damage to all the targets, one time." },
    Moment { value: 140, label: "The move did damage", help: "At the end of a damaging move, once a Pokémon took damage." },
    Moment { value: 146, label: "A move without damage ended", help: "At the end of a move that deals no damage." },
    Moment { value: 176, label: "After the move", help: "After the action of the move." },
];

/// The moments of an item that a Pokémon holds.
pub const ITEM_MOMENTS: [Moment; 18] = [
    Moment { value: 94, label: "A Pokémon enters the battle", help: "Runs for every Pokémon that enters, yours or not. Use the condition \"It is this Pokémon\" to act only for the holder." },
    Moment { value: 36, label: "A move starts", help: "Before a move does anything." },
    Moment { value: 140, label: "A move did damage", help: "At the end of a damaging move, once a Pokémon took damage." },
    Moment { value: 176, label: "After a move", help: "After the action of a move." },
    Moment { value: 102, label: "After a stat change", help: "After a stat stage went up or down." },
    Moment { value: 135, label: "After the weather changes", help: "When the weather has just changed." },
    Moment { value: 127, label: "The end of the turn begins", help: "At the start of the end-of-turn checks." },
    Moment { value: 2, label: "After each action", help: "After the action of a Pokémon (a move, a switch, an item)." },
    Moment { value: 175, label: "Right before fainting", help: "A Pokémon is about to faint." },
    Moment { value: 61, label: "The power of a move is calculated", help: "Before the damage. Use \"Multiply the value\" to raise or lower the power of the move (the Muscle Band, the type plates)." },
    Moment { value: 64, label: "The attack stat is calculated", help: "The Attack or the Sp. Atk of the attacker, before the damage (the Choice Band, the Light Ball). Use the condition \"The move is\" to choose physical or special moves." },
    Moment { value: 65, label: "The defense stat is calculated", help: "The Defense or the Sp. Def of the target, before the damage (the Assault Vest, the Eviolite). Use the condition \"This Pokémon is the target of the move\"." },
    Moment { value: 79, label: "The damage is calculated", help: "After the type effectiveness: multiplies the final damage (the Life Orb, the Expert Belt with \"The move is super effective\")." },
    Moment { value: 19, label: "The speed is calculated", help: "The speed that decides who moves first (the Choice Scarf, the Iron Ball). Use the condition \"It is this Pokémon\"." },
    Moment { value: 57, label: "The accuracy is calculated", help: "Multiplies the chance to hit (the Wide Lens, the Zoom Lens)." },
    Moment { value: 59, label: "A critical hit is checked", help: "Use \"Raise the critical-hit stage\" (the Scope Lens, the Razor Claw)." },
    Moment { value: 83, label: "A Pokémon took damage from a move", help: "Runs for the Pokémon that was hit, after the damage (the Rocky Helmet, the Sticky Barb). Use the condition \"This Pokémon is the target of the move\"." },
    Moment { value: 128, label: "The end of the turn ends", help: "At the end of the end-of-turn checks (the Leftovers, the Black Sludge)." },
];

/// The moments that the editor offers in a place.
pub fn moments(context: Context) -> &'static [Moment] {
    match context {
        Context::Ability => &MOMENTS,
        Context::Move => &MOVE_MOMENTS,
        Context::Item => &ITEM_MOMENTS,
    }
}

/// True for a moment of a move: the values of the situation that tell the move and its targets exist.
pub fn is_move_moment(moment: u16) -> bool {
    matches!(moment, 36 | 85 | 140 | 146 | 176 | 61 | 64 | 65 | 79 | 57 | 59 | 83)
}

/// What is wrong with the rules, in words that the user understands. An empty list means the file can be written.
pub fn problems(rules: &[Rule], context: Context) -> Vec<String> {
    let mut problems = Vec::new();
    if rules.len() > MAX_RULES {
        problems.push(format!("There are {MAX_RULES} reactions at most."));
    }
    for (number, rule) in rules.iter().enumerate() {
        let at = format!("Reaction {}", number + 1);
        if rule.actions.is_empty() {
            problems.push(format!("{at} does nothing: add an action."));
        }
        if rule.conditions.len() > MAX_CONDITIONS || rule.actions.len() > MAX_ACTIONS {
            problems.push(format!("{at} has too many conditions or actions."));
        }
        let move_moment = context == Context::Move || is_move_moment(rule.moment);
        for condition in &rule.conditions {
            let needs_move = matches!(
                condition.kind,
                ConditionKind::MoveUserIsOwner
                    | ConditionKind::MoveTypeIs
                    | ConditionKind::TargetFainted
                    | ConditionKind::DefenderIsOwner
                    | ConditionKind::MoveCategoryIs
                    | ConditionKind::SuperEffective
                    | ConditionKind::NoSubstitute
                    | ConditionKind::MoveMakesContact
            );
            if !move_moment && needs_move {
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
            if action.kind == ActionKind::MultiplyValue && !is_value_moment(rule.moment) {
                problems.push(format!("{at}: \"{}\" needs a moment that calculates a value (the power, the attack, the defense, the damage, the speed or the accuracy).", action.kind.label()));
            }
            if action.kind == ActionKind::MultiplyValue && action.b <= 0 {
                problems.push(format!("{at}: the multiplier must be more than 0."));
            }
            if action.kind == ActionKind::RaiseCritical && rule.moment != 59 {
                problems.push(format!("{at}: \"{}\" needs the moment \"A critical hit is checked\".", action.kind.label()));
            }
        }
    }
    problems
}

fn find(list: &[(i16, &'static str)], value: i16) -> &'static str {
    list.iter().find(|(v, _)| *v == value).map(|(_, name)| *name).unwrap_or("?")
}

/// The name of a type from its number.
pub fn type_name(value: i16) -> &'static str {
    usize::try_from(value).ok().and_then(|n| sango_species_base::TYPE_NAMES.get(n).copied()).unwrap_or("?")
}

/// The name of a target.
pub fn target_label(target: u8) -> &'static str {
    TARGETS.iter().find(|(v, _, _)| *v == target).map(|(_, name, _)| *name).unwrap_or("?")
}

/// The name of a moment.
pub fn moment_label(moment: u16) -> &'static str {
    MOMENTS.iter().chain(MOVE_MOMENTS.iter()).chain(ITEM_MOMENTS.iter()).find(|m| m.value == moment).map(|m| m.label).unwrap_or("Another moment")
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
        ConditionKind::HpBelow => format!("its HP are under {}% of the maximum", condition.arg),
        ConditionKind::DefenderIsOwner => "this Pokémon is the target of the move".into(),
        ConditionKind::MoveCategoryIs => format!("the move is {}", find(&DAMAGE_KINDS, condition.arg)),
        ConditionKind::SuperEffective => "the move is super effective".into(),
        ConditionKind::NoSubstitute => "the target has no substitute".into(),
        ConditionKind::HpNotFull => "its HP are not full".into(),
        ConditionKind::MoveMakesContact => "the move makes contact".into(),
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
        ActionKind::MultiplyValue => format!("multiply the value by {}", action.b as f32 / 100.0),
        ActionKind::DamageHp => format!("{who} loses {}% of its maximum HP", action.a),
        ActionKind::ConsumeItem => "use up the item".into(),
        ActionKind::RaiseCritical => format!("raise the critical-hit stage by {}", action.a),
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

    fn life_orb() -> Vec<Rule> {
        vec![
            Rule {
                moment: 79,
                conditions: vec![Condition { kind: ConditionKind::MoveUserIsOwner, arg: 0 }],
                actions: vec![Action { kind: ActionKind::MultiplyValue, target: 0, a: 0, b: 130, banner: false }],
            },
            Rule {
                moment: 140,
                conditions: vec![Condition { kind: ConditionKind::MoveUserIsOwner, arg: 0 }, Condition { kind: ConditionKind::MoveCategoryIs, arg: 1 }],
                actions: vec![Action { kind: ActionKind::DamageHp, target: 0, a: 10, b: 0, banner: false }],
            },
        ]
    }

    #[test]
    fn the_new_reactions_go_through_a_file_and_read_back() {
        let rules = life_orb();
        let mut bytes = Vec::new();
        write_rules(&mut bytes, &rules);
        let read = read_rules(&mut Reader::new(&bytes), rules.len()).unwrap();
        assert_eq!(read, rules);
        assert!(problems(&rules, Context::Item).is_empty(), "{:?}", problems(&rules, Context::Item));
        assert_eq!(rule_sentence(&rules[0]), "When the damage is calculated, and this Pokémon used the move: multiply the value by 1.3.");
    }

    #[test]
    fn a_multiplier_needs_a_moment_that_calculates_a_value() {
        let mut rules = life_orb();
        rules[0].moment = 94;
        let found = problems(&rules, Context::Item);
        assert_eq!(found.len(), 2, "{found:?}");
        rules[0].moment = 79;
        rules[0].actions[0].kind = ActionKind::RaiseCritical;
        assert_eq!(problems(&rules, Context::Item).len(), 1, "the critical stage needs its own moment");
    }

    #[test]
    fn every_moment_has_a_label_and_a_help() {
        for moment in MOMENTS.iter().chain(ITEM_MOMENTS.iter()) {
            assert!(!moment.label.is_empty() && !moment.help.is_empty());
            assert_eq!(moment_label(moment.value), moment.label);
        }
        for value in [61, 64, 65, 79, 19, 57] {
            assert!(is_value_moment(value) && ITEM_MOMENTS.iter().any(|m| m.value == value));
        }
    }
}
