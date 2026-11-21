//! The events as the user writes them (blocks inside blocks), and the file that the plugin reads (a flat list of steps with jumps).
//!
//! The plugin side is `script::StudioEvents` (see `lib/include/script/patch/studio_events.h` in the plugin): the numbers of the actions
//! and the layout of the file are the same on both sides.

use serde::{Deserialize, Serialize};

/// The number of events, and the script number of the first one.
pub const COUNT: usize = 256;
pub const FIRST_SCRIPT: u32 = 32100;
/// The most steps of an event, and the longest text of a step (in characters).
pub const MAX_STEPS: usize = 128;
pub const MAX_TEXT: usize = 200;
/// The most bytes of the file of an event.
const MAX_FILE: usize = 16 * 1024;

/// The numbers of the actions: the same as `StudioEvents::Action` in the plugin.
mod action {
    pub const STOP: u16 = 0;
    pub const SAY: u16 = 1;
    pub const GIVE_ITEM: u16 = 2;
    pub const TAKE_ITEM: u16 = 3;
    pub const MONEY: u16 = 4;
    pub const SET_FLAG: u16 = 5;
    pub const RESET_FLAG: u16 = 6;
    pub const SET_VARIABLE: u16 = 7;
    pub const SOUND: u16 = 8;
    pub const JINGLE: u16 = 9;
    pub const WAIT: u16 = 10;
    pub const RUN_SCRIPT: u16 = 11;
    pub const GIVE_POKEMON: u16 = 12;
    pub const WILD_BATTLE: u16 = 13;
    pub const TRAINER_BATTLE: u16 = 14;
    pub const WARP: u16 = 15;
    pub const HEAL: u16 = 16;
    pub const FADE: u16 = 17;
    pub const JUMP: u16 = 18;
    pub const IF_FLAG: u16 = 19;
    pub const IF_ITEM: u16 = 20;
    pub const IF_MONEY: u16 = 21;
    pub const IF_VARIABLE: u16 = 22;
    pub const ASK: u16 = 23;
    pub const CHOICE: u16 = 24;
}

/// How a variable is compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compare {
    Equal,
    Different,
    More,
    Less,
}

impl Compare {
    pub const ALL: [Compare; 4] = [Compare::Equal, Compare::Different, Compare::More, Compare::Less];

    pub fn label(self) -> &'static str {
        match self {
            Compare::Equal => "is",
            Compare::Different => "is not",
            Compare::More => "is more than",
            Compare::Less => "is less than",
        }
    }
}

/// What an `If` block tests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// A flag is set. `save` is a flag of Sango (4096 of them), else a flag of the game.
    Flag { flag: u32, save: bool },
    /// The player has the item, `count` times.
    Item { item: u32, count: u32 },
    /// The player has this money or more.
    Money { amount: u32 },
    /// A variable of Sango against a value.
    Variable { variable: u32, compare: Compare, value: u32 },
    /// The player answers yes to a question: the question is the text of the block before.
    Ask,
}

impl Condition {
    pub const KINDS: usize = 5;

    pub fn label(&self) -> &'static str {
        match self {
            Condition::Flag { .. } => "a flag is set",
            Condition::Item { .. } => "the player has an item",
            Condition::Money { .. } => "the player has money",
            Condition::Variable { .. } => "a variable",
            Condition::Ask => "the player answers yes",
        }
    }

    /// A new condition of the kind with this number (the order of `kind`).
    pub fn of_kind(kind: usize) -> Condition {
        match kind {
            0 => Condition::Flag { flag: 0, save: true },
            1 => Condition::Item { item: 1, count: 1 },
            2 => Condition::Money { amount: 100 },
            3 => Condition::Variable { variable: 0, compare: Compare::Equal, value: 0 },
            _ => Condition::Ask,
        }
    }

    pub fn kind(&self) -> usize {
        match self {
            Condition::Flag { .. } => 0,
            Condition::Item { .. } => 1,
            Condition::Money { .. } => 2,
            Condition::Variable { .. } => 3,
            Condition::Ask => 4,
        }
    }
}

/// One block of an event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Block {
    /// Shows a text. `talk` is the window of a conversation, else the window of a sign.
    Say { text: String, talk: bool },
    GiveItem { item: u32, count: u32 },
    TakeItem { item: u32, count: u32 },
    Money { amount: u32, take: bool },
    SetFlag { flag: u32, save: bool, on: bool },
    SetVariable { variable: u32, value: u32 },
    Sound { sound: u32 },
    Jingle { sound: u32 },
    Wait { frames: u32 },
    RunScript { script: u32 },
    GivePokemon { species: u32, level: u32, form: u32 },
    WildBattle { species: u32, level: u32, form: u32 },
    TrainerBattle { trainer: u32 },
    Warp { map: u32, x: u32, z: u32 },
    Heal,
    Fade { out: bool },
    Stop,
    /// Tests something. The first list runs when it is true, the second one when it is not.
    If { condition: Condition, then: Vec<Block>, otherwise: Vec<Block> },
    /// Asks a question with two or three choices: the player picks one, and its blocks run.
    Choice { question: String, options: Vec<(String, Vec<Block>)> },
}

impl Block {
    /// The kinds that the "Add a block" menu has: the name and a new block of that kind.
    pub fn kinds() -> Vec<(&'static str, Block)> {
        vec![
            ("Say a text", Block::Say { text: String::new(), talk: true }),
            ("Ask yes or no, then…", Block::If { condition: Condition::Ask, then: Vec::new(), otherwise: Vec::new() }),
            ("Ask a question with choices", Block::Choice { question: String::new(), options: vec![("Yes".into(), Vec::new()), ("No".into(), Vec::new())] }),
            ("If…", Block::If { condition: Condition::Flag { flag: 0, save: true }, then: Vec::new(), otherwise: Vec::new() }),
            ("Give an item", Block::GiveItem { item: 1, count: 1 }),
            ("Take an item", Block::TakeItem { item: 1, count: 1 }),
            ("Give or take money", Block::Money { amount: 100, take: false }),
            ("Set or clear a flag", Block::SetFlag { flag: 0, save: true, on: true }),
            ("Set a variable", Block::SetVariable { variable: 0, value: 0 }),
            ("Give a Pokémon", Block::GivePokemon { species: 1, level: 5, form: 0 }),
            ("Wild battle", Block::WildBattle { species: 1, level: 5, form: 0 }),
            ("Trainer battle", Block::TrainerBattle { trainer: 1 }),
            ("Go to a map", Block::Warp { map: 1, x: 0, z: 0 }),
            ("Heal the party", Block::Heal),
            ("Fade the screens", Block::Fade { out: true }),
            ("Play a sound effect", Block::Sound { sound: 0 }),
            ("Play a jingle", Block::Jingle { sound: 0 }),
            ("Wait", Block::Wait { frames: 30 }),
            ("Start a script of the game", Block::RunScript { script: 0 }),
            ("End the event", Block::Stop),
        ]
    }

    /// A short title for the header of the block.
    pub fn title(&self) -> String {
        match self {
            Block::Say { .. } => "Say a text".into(),
            Block::GiveItem { .. } => "Give an item".into(),
            Block::TakeItem { .. } => "Take an item".into(),
            Block::Money { take, .. } => if *take { "Take money" } else { "Give money" }.into(),
            Block::SetFlag { on, .. } => if *on { "Set a flag" } else { "Clear a flag" }.into(),
            Block::SetVariable { .. } => "Set a variable".into(),
            Block::Sound { .. } => "Play a sound effect".into(),
            Block::Jingle { .. } => "Play a jingle".into(),
            Block::Wait { .. } => "Wait".into(),
            Block::RunScript { .. } => "Start a script of the game".into(),
            Block::GivePokemon { .. } => "Give a Pokémon".into(),
            Block::WildBattle { .. } => "Wild battle".into(),
            Block::TrainerBattle { .. } => "Trainer battle".into(),
            Block::Warp { .. } => "Go to a map".into(),
            Block::Heal => "Heal the party".into(),
            Block::Fade { out } => if *out { "Fade out" } else { "Fade in" }.into(),
            Block::Stop => "End the event".into(),
            Block::If { condition: Condition::Ask, .. } => "Ask yes or no".into(),
            Block::If { .. } => "If".into(),
            Block::Choice { .. } => "Ask a question with choices".into(),
        }
    }
}

/// An event: a name and its blocks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub blocks: Vec<Block>,
}

impl Event {
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// What the list shows for this event: the name, else the beginning of the first text.
    pub fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        fn first_text(blocks: &[Block]) -> Option<String> {
            blocks.iter().find_map(|block| match block {
                Block::Say { text, .. } if !text.trim().is_empty() => Some(text.trim().chars().take(32).collect()),
                Block::If { then, otherwise, .. } => first_text(then).or_else(|| first_text(otherwise)),
                _ => None,
            })
        }
        first_text(&self.blocks).map(|t| format!("\"{t}\"")).unwrap_or_else(|| "(empty)".to_string())
    }
}

/// A step of the file.
#[derive(Clone, Debug, Default, PartialEq)]
struct Step {
    action: u16,
    target: u16,
    a: u32,
    b: u32,
    c: u32,
    text: String,
}

/// Makes the steps of a list of blocks.
fn emit(blocks: &[Block], steps: &mut Vec<Step>) -> Result<(), String> {
    for block in blocks {
        if steps.len() >= MAX_STEPS {
            return Err(format!("The event has more than {MAX_STEPS} steps."));
        }
        let step = |action: u16, a: u32, b: u32, c: u32| Step { action, target: 0, a, b, c, text: String::new() };
        match block {
            Block::Say { text, talk } => steps.push(Step { text: text.clone(), ..step(action::SAY, u32::from(*talk), 0, 0) }),
            Block::GiveItem { item, count } => steps.push(step(action::GIVE_ITEM, *item, *count, 0)),
            Block::TakeItem { item, count } => steps.push(step(action::TAKE_ITEM, *item, *count, 0)),
            Block::Money { amount, take } => steps.push(step(action::MONEY, *amount, u32::from(*take), 0)),
            Block::SetFlag { flag, save, on } => steps.push(step(if *on { action::SET_FLAG } else { action::RESET_FLAG }, *flag, u32::from(*save), 0)),
            Block::SetVariable { variable, value } => steps.push(step(action::SET_VARIABLE, *variable, *value, 0)),
            Block::Sound { sound } => steps.push(step(action::SOUND, *sound, 0, 0)),
            Block::Jingle { sound } => steps.push(step(action::JINGLE, *sound, 0, 0)),
            Block::Wait { frames } => steps.push(step(action::WAIT, *frames, 0, 0)),
            Block::RunScript { script } => steps.push(step(action::RUN_SCRIPT, *script, 0, 0)),
            Block::GivePokemon { species, level, form } => steps.push(step(action::GIVE_POKEMON, *species, *level, *form)),
            Block::WildBattle { species, level, form } => steps.push(step(action::WILD_BATTLE, *species, *level, *form)),
            Block::TrainerBattle { trainer } => steps.push(step(action::TRAINER_BATTLE, *trainer, 0, 0)),
            Block::Warp { map, x, z } => steps.push(step(action::WARP, *map, *x, *z)),
            Block::Heal => steps.push(step(action::HEAL, 0, 0, 0)),
            Block::Fade { out } => steps.push(step(action::FADE, u32::from(!*out), 0, 0)),
            Block::Stop => steps.push(step(action::STOP, 0, 0, 0)),
            Block::If { condition, then, otherwise } => {
                let test = match condition {
                    Condition::Flag { flag, save } => step(action::IF_FLAG, *flag, u32::from(*save), 0),
                    Condition::Item { item, count } => step(action::IF_ITEM, *item, *count, 0),
                    Condition::Money { amount } => step(action::IF_MONEY, *amount, 0, 0),
                    Condition::Variable { variable, compare, value } => step(action::IF_VARIABLE, *variable, *value, *compare as u32),
                    Condition::Ask => step(action::ASK, 0, 0, 0),
                };
                let test_at = steps.len();
                steps.push(test);
                emit(then, steps)?;
                if otherwise.is_empty() {
                    steps[test_at].target = steps.len() as u16;
                } else {
                    let jump_at = steps.len();
                    steps.push(step(action::JUMP, 0, 0, 0));
                    steps[test_at].target = steps.len() as u16;
                    emit(otherwise, steps)?;
                    steps[jump_at].target = steps.len() as u16;
                }
            }
            Block::Choice { question, options } => {
                if !(2..=3).contains(&options.len()) {
                    return Err("A question has two or three choices.".to_string());
                }
                let clean = |text: &str| text.replace('|', "/");
                let text = std::iter::once(clean(question)).chain(options.iter().map(|(name, _)| clean(name))).collect::<Vec<_>>().join("|");
                let choice_at = steps.len();
                steps.push(Step { action: action::CHOICE, text, ..Step::default() });
                let mut jumps = Vec::new();
                let mut targets = [0u32; 3];
                for (place, (_, blocks)) in options.iter().enumerate() {
                    targets[place] = steps.len() as u32;
                    emit(blocks, steps)?;
                    if place + 1 < options.len() {
                        jumps.push(steps.len());
                        steps.push(step(action::JUMP, 0, 0, 0));
                    }
                }
                let end = steps.len() as u16;
                for at in jumps {
                    steps[at].target = end;
                }
                let choice = &mut steps[choice_at];
                choice.a = targets[0];
                choice.b = targets[1];
                choice.c = targets[2];
                choice.target = end;
            }
        }
    }
    Ok(())
}

/// The file that the plugin reads for an event.
pub fn compile(event: &Event) -> Result<Vec<u8>, String> {
    let mut steps = Vec::new();
    emit(&event.blocks, &mut steps)?;
    if steps.len() > MAX_STEPS {
        return Err(format!("The event has more than {MAX_STEPS} steps."));
    }
    let mut bytes = b"SEVT".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(steps.len() as u16).to_le_bytes());
    for step in &steps {
        if step.text.split('|').any(|part| part.chars().count() > MAX_TEXT) {
            return Err(format!("A text has more than {MAX_TEXT} characters."));
        }
        let units: Vec<u16> = step.text.encode_utf16().collect();
        bytes.extend_from_slice(&step.action.to_le_bytes());
        bytes.extend_from_slice(&step.target.to_le_bytes());
        for value in [step.a, step.b, step.c] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for unit in units {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
    if bytes.len() > MAX_FILE {
        return Err(format!("The event is too big for the game ({} bytes, the most is {MAX_FILE}).", bytes.len()));
    }
    Ok(bytes)
}

/// The number of steps that the event makes in the file.
pub fn step_count(event: &Event) -> Result<usize, String> {
    let mut steps = Vec::new();
    emit(&event.blocks, &mut steps)?;
    Ok(steps.len())
}

#[cfg(test)]
mod test {
    use super::*;

    fn steps_of(blocks: Vec<Block>) -> Vec<Step> {
        let mut steps = Vec::new();
        emit(&blocks, &mut steps).unwrap();
        steps
    }

    #[test]
    fn a_question_jumps_over_its_blocks() {
        let steps = steps_of(vec![
            Block::If { condition: Condition::Ask, then: vec![Block::Heal, Block::Heal], otherwise: vec![Block::Stop] },
            Block::Wait { frames: 1 },
        ]);
        // ask, heal, heal, jump, stop, wait
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0].action, action::ASK);
        assert_eq!(steps[0].target, 4, "no goes to the else blocks");
        assert_eq!(steps[3].action, action::JUMP);
        assert_eq!(steps[3].target, 5, "the then blocks go over the else blocks");
    }

    #[test]
    fn an_if_without_else_goes_to_the_next_block() {
        let steps = steps_of(vec![Block::If { condition: Condition::Flag { flag: 7, save: false }, then: vec![Block::Heal], otherwise: vec![] }, Block::Heal]);
        assert_eq!(steps[0].target, 2);
        assert_eq!((steps[0].a, steps[0].b), (7, 0));
    }

    #[test]
    fn a_choice_gives_the_step_of_each_option() {
        let steps = steps_of(vec![Block::Choice {
            question: "Which?".into(),
            options: vec![("A|x".into(), vec![Block::Heal]), ("B".into(), vec![Block::Heal, Block::Heal]), ("C".into(), vec![])],
        }]);
        // choice, heal, jump, heal, heal, jump, (end)
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0].text, "Which?|A/x|B|C");
        assert_eq!((steps[0].a, steps[0].b, steps[0].c), (1, 3, 6));
        assert_eq!(steps[2].target, 6);
        assert_eq!(steps[5].target, 6);
    }

    #[test]
    fn the_file_has_the_header_and_the_steps() {
        let event = Event { name: String::new(), blocks: vec![Block::Say { text: "Hi".into(), talk: true }, Block::GiveItem { item: 4, count: 2 }] };
        let bytes = compile(&event).unwrap();
        assert_eq!(&bytes[..4], b"SEVT");
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 2);
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), action::SAY);
        assert_eq!(u16::from_le_bytes([bytes[8 + 16], bytes[8 + 17]]), 2);
        assert_eq!(bytes.len(), 8 + 18 + 4 + 18);
    }

    #[test]
    fn too_many_steps_are_refused() {
        let event = Event { name: String::new(), blocks: vec![Block::Heal; MAX_STEPS + 1] };
        assert!(compile(&event).is_err());
    }

    #[test]
    fn an_event_goes_through_json() {
        let event = Event { name: "Gift".into(), blocks: vec![Block::If { condition: Condition::Item { item: 3, count: 2 }, then: vec![Block::Heal], otherwise: vec![] }] };
        let text = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&text).unwrap(), event);
    }
}
