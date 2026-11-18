//! The data of the trainers: the three archives of the game and their plain forms.
//!
//! | Archive | File | Content |
//! | --- | --- | --- |
//! | 36 | one for each trainer (1 to 949) | The data of the trainer, 24 bytes: [`TrainerData`]. |
//! | 38 | one for each trainer | The team: the Pokémon one after the other, in the form that [`TrainerData::data_type`] says. |
//! | 37 | one for each class (0 to 279) | The data of the class, 16 bytes: [`TrainerClass`]. |
//!
//! The names of the trainers are the lines of the text file 22 and the names of the classes the lines of the file 21 (see
//! [`crate::text`]). The Studio keeps the files that the user changed in `trainer/data`, `trainer/pokemon` and `trainer/class`.

/// The archive of the data of the trainers.
pub const DATA_ARCHIVE: usize = 36;
/// The archive of the classes.
pub const CLASS_ARCHIVE: usize = 37;
/// The archive of the teams.
pub const TEAM_ARCHIVE: usize = 38;
/// The text file of the names of the classes.
pub const CLASS_NAMES: usize = 21;
/// The text file of the names of the trainers.
pub const TRAINER_NAMES: usize = 22;
/// The number of trainers (the number 0 is not a trainer) and of classes.
pub const TRAINER_COUNT: usize = 950;
pub const CLASS_COUNT: usize = 280;
/// The most Pokémon in a team.
pub const MAX_TEAM: usize = 6;

/// The size of the data of a trainer and of a class.
pub const DATA_SIZE: usize = 24;
pub const CLASS_SIZE: usize = 16;

/// The kinds of battle that a trainer fights.
pub const FIGHT_TYPES: [&str; 5] = ["Single", "Double", "Triple", "Rotation", "Horde"];

/// The ways of thinking of the AI that have a script in the game: the bit, the name and what it does.
///
/// The AI gives each move of the Pokémon 100 points, then each script that is on adds or removes points (many of them with a chance: "+3
/// in 9 cases of 10"), and the Pokémon plays one of the moves that have the most points. The other bits of [`TrainerData::ai`] have no
/// script in the game and change nothing.
pub const AI_BITS: [(u8, &str, &str); 6] = [
    (0, "Basic", "Removes the moves that would fail or do nothing: a type that has no effect, a target that is immune by its ability (a Water move on Water Absorb), a sleep move on a Pokémon that sleeps already, a stat that is at its limit."),
    (1, "Attack", "Gives points to the move that does the most damage, and more when it knocks the target out or is super effective (x4); a weaker move loses a point."),
    (2, "Expert", "The rules of a player for each kind of move: it uses an explosion when its HP are low, does not raise a stat that is already +2, keeps a status move for the right moment, does not waste a move on Protect or Magic Coat."),
    (7, "Double battle", "The rules of the double battles: it does not hit its partner (except for a few moves), uses the moves that help the partner and the moves that hit several Pokémon."),
    (8, "Go easy", "Spares you: when your Pokémon has few HP (under 20 or 40 percent) it often gives less points to the moves that deal damage. The game uses it for the first fights that help the player."),
    (14, "Horde", "The way of the Pokémon of a horde battle: they do not attack each other, except a few pairs that fight in the wild (Zangoose and Seviper, Durant and Heatmor)."),
];

/// The groups of classes. The game uses them for the Pokémon of the bosses.
pub const GROUPS: [&str; 6] = ["Normal", "Rival", "Support", "Gym Leader", "Elite Four", "Champion"];

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// The data of a trainer (the file of the archive 36).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainerData {
    /// How the Pokémon of the team are written (0 to 3, see [`Team`]). The Studio chooses it when it writes the team.
    pub data_type: u8,
    /// The class of the trainer.
    pub class: u16,
    /// The kind of battle (0 single, 1 double, 2 triple, 3 rotation, 4 horde).
    pub fight_type: u8,
    pub pokemon_count: u8,
    /// The items that the trainer uses in the battle.
    pub items: [u16; 4],
    /// The AI: one bit for each way of thinking ([`AI_BITS`]).
    pub ai: u32,
    /// The Pokémon get their HP back after the battle.
    pub heal_after: bool,
    /// The prize money: the game multiplies it by the level of the last Pokémon.
    pub money: u8,
    /// The item that the trainer gives after the battle.
    pub gift_item: u16,
    /// The bytes that the game does not use, kept as they are (the pads and the unused bits).
    padding: [u8; 5],
}

impl TrainerData {
    /// Reads the 24 bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<TrainerData, String> {
        if bytes.len() < DATA_SIZE {
            return Err(format!("The data of a trainer has {} bytes, not {DATA_SIZE}.", bytes.len()));
        }
        Ok(TrainerData {
            data_type: bytes[0],
            class: word(bytes, 2),
            fight_type: bytes[6],
            pokemon_count: bytes[7],
            items: [word(bytes, 8), word(bytes, 10), word(bytes, 12), word(bytes, 14)],
            ai: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            heal_after: bytes[20] & 1 != 0,
            money: bytes[21],
            gift_item: word(bytes, 22),
            // The byte 1, the bytes 4 and 5, and the 7 other bits of the byte 20.
            padding: [bytes[1], bytes[4], bytes[5], bytes[20] & 0xFE, 0],
        })
    }

    /// The 24 bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; DATA_SIZE];
        out[0] = self.data_type;
        out[1] = self.padding[0];
        out[2..4].copy_from_slice(&self.class.to_le_bytes());
        out[4] = self.padding[1];
        out[5] = self.padding[2];
        out[6] = self.fight_type;
        out[7] = self.pokemon_count;
        for (i, item) in self.items.iter().enumerate() {
            out[8 + 2 * i..10 + 2 * i].copy_from_slice(&item.to_le_bytes());
        }
        out[16..20].copy_from_slice(&self.ai.to_le_bytes());
        out[20] = self.padding[3] | u8::from(self.heal_after);
        out[21] = self.money;
        out[22..24].copy_from_slice(&self.gift_item.to_le_bytes());
        out
    }

    /// The data of a new trainer: a single battle, no item, the basic AI.
    pub fn new(class: u16) -> TrainerData {
        TrainerData { data_type: 3, class, fight_type: 0, pokemon_count: 0, items: [0; 4], ai: 1, heal_after: false, money: 20, gift_item: 0, padding: [0; 5] }
    }
}

/// A Pokémon of a team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainerPokemon {
    /// The IVs: the same value for the six stats. The file allows 0 to 255; the game counts more than 31 as 31 (the bosses have 255).
    pub iv: u8,
    /// The ability: 0 chooses one of the first two at random, 1 to 3 the ability of that place in the data of the species.
    pub ability: u8,
    /// The gender: 0 chooses the one of the species (or of the class), 1 male, 2 female.
    pub gender: u8,
    pub level: u16,
    pub species: u16,
    pub form: u16,
    pub item: u16,
    /// The four moves. A team that does not choose its moves gives them 0 and the Pokémon knows the moves of its level.
    pub moves: [u16; 4],
}

impl TrainerPokemon {
    /// A new Pokémon of a species at a level, with the IVs 0, a random ability and the gender of the species.
    pub fn new(species: u16, level: u16) -> TrainerPokemon {
        TrainerPokemon { iv: 0, ability: 0, gender: 0, level, species, form: 0, item: 0, moves: [0; 4] }
    }
}

/// The team of a trainer (the file of the archive 38).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub pokemon: Vec<TrainerPokemon>,
    /// The Pokémon have the moves that the file gives. Without it, they know the moves of their level.
    pub moves: bool,
    /// The Pokémon hold the items that the file gives.
    pub items: bool,
}

/// The size of a Pokémon in the file for each `data_type`: normal, moves, item, item and moves.
const POKEMON_SIZE: [usize; 4] = [8, 16, 10, 18];

impl Team {
    /// Reads the file of a team. `data_type` and `count` come from the data of the trainer.
    pub fn from_bytes(data_type: u8, count: usize, bytes: &[u8]) -> Result<Team, String> {
        let size = *POKEMON_SIZE.get(data_type as usize).ok_or_else(|| format!("The data type {data_type} of the team is not known."))?;
        if count > MAX_TEAM || bytes.len() < count * size {
            return Err(format!("The team has {} bytes for {count} Pokémon of {size} bytes.", bytes.len()));
        }
        let (moves, items) = (data_type & 1 != 0, data_type >= 2);
        let pokemon = (0..count)
            .map(|i| {
                let at = i * size;
                let mut pokemon = TrainerPokemon {
                    iv: bytes[at],
                    ability: bytes[at + 1] >> 4,
                    gender: bytes[at + 1] & 0xF,
                    level: word(bytes, at + 2),
                    species: word(bytes, at + 4),
                    form: word(bytes, at + 6),
                    item: 0,
                    moves: [0; 4],
                };
                let mut next = at + 8;
                if items {
                    pokemon.item = word(bytes, next);
                    next += 2;
                }
                if moves {
                    pokemon.moves = [word(bytes, next), word(bytes, next + 2), word(bytes, next + 4), word(bytes, next + 6)];
                }
                pokemon
            })
            .collect();
        Ok(Team { pokemon, moves, items })
    }

    /// The `data_type` that this team needs.
    pub fn data_type(&self) -> u8 {
        match (self.items, self.moves) {
            (false, false) => 0,
            (false, true) => 1,
            (true, false) => 2,
            (true, true) => 3,
        }
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for pokemon in &self.pokemon {
            out.push(pokemon.iv);
            out.push(pokemon.ability << 4 | (pokemon.gender & 0xF));
            out.extend_from_slice(&pokemon.level.to_le_bytes());
            out.extend_from_slice(&pokemon.species.to_le_bytes());
            out.extend_from_slice(&pokemon.form.to_le_bytes());
            if self.items {
                out.extend_from_slice(&pokemon.item.to_le_bytes());
            }
            if self.moves {
                for mv in pokemon.moves {
                    out.extend_from_slice(&mv.to_le_bytes());
                }
            }
        }
        out
    }
}

/// The data of a class of trainers (the file of the archive 37).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainerClass {
    /// The sequence of the 3D model in the battle (0 the usual one, 1 the added one).
    pub sequence: u8,
    /// The gender of the trainers (0 male, 1 female, 2 none).
    pub gender: u8,
    /// 1 when the class is a group (Twins, Team...).
    pub plural: u8,
    /// The group (see [`GROUPS`]).
    pub group: u8,
    /// The animation of the start of the battle.
    pub battle_effect: u8,
    /// The 3D model of the battle (a number of the list of the models; 255 for none).
    pub model_3d: u8,
    /// The picture of the 2D trainer in the battle (a file of the archive of the pictures).
    pub picture_2d: u32,
    /// The character on the map (a number of the list of the models of the characters).
    pub overworld_model: u32,
    /// The music of the trainer that sees the player.
    pub eye_music: u32,
}

impl TrainerClass {
    /// Reads the 16 bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<TrainerClass, String> {
        if bytes.len() < CLASS_SIZE {
            return Err(format!("The data of a class has {} bytes, not {CLASS_SIZE}.", bytes.len()));
        }
        let long = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        Ok(TrainerClass {
            sequence: bytes[0] & 0xF,
            gender: (bytes[0] >> 4) & 3,
            plural: bytes[0] >> 6,
            group: bytes[1],
            battle_effect: bytes[2],
            model_3d: bytes[3],
            picture_2d: long(4),
            overworld_model: long(8),
            eye_music: long(12),
        })
    }

    /// The 16 bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; CLASS_SIZE];
        out[0] = (self.sequence & 0xF) | (self.gender & 3) << 4 | (self.plural & 3) << 6;
        out[1] = self.group;
        out[2] = self.battle_effect;
        out[3] = self.model_3d;
        out[4..8].copy_from_slice(&self.picture_2d.to_le_bytes());
        out[8..12].copy_from_slice(&self.overworld_model.to_le_bytes());
        out[12..16].copy_from_slice(&self.eye_music.to_le_bytes());
        out
    }
}

/// The archive of the messages of the trainers: the file 0 gives the place of the messages of each trainer, the file 1 is the list of the
/// messages (see [`message_lines`]). The texts are the lines of the text file 23.
pub const MESSAGE_ARCHIVE: usize = 135;
pub const MESSAGE_TEXT: usize = 23;

/// The kinds of messages of a trainer. A trainer has only some of them.
pub const MESSAGE_KINDS: [&str; 21] = [
    "Before the battle",
    "When the trainer loses",
    "After the battle",
    "Before the battle (first trainer of a pair)",
    "Before the battle (second trainer of a pair)",
    "After the battle (first trainer of a pair)",
    "After the battle (second trainer of a pair)",
    "When the trainer loses (second trainer of a pair)",
    "Only one Pokémon (first trainer of a pair)",
    "Only one Pokémon (second trainer of a pair)",
    "Fewer than three Pokémon",
    "After the battle, before the gift",
    "After the battle, after the gift",
    "First damage to the Pokémon of the trainer",
    "Last Pokémon of the trainer",
    "Last Pokémon of the trainer, HP under half",
    "After the battle, before the healing",
    "After the battle, after the healing",
    "Gift of a rematch",
    "Gift of a rematch (first trainer of a pair)",
    "Gift of a rematch (second trainer of a pair)",
];

/// The messages of a trainer: the kind and the line of the text file 23, in the order of the kinds. `offsets` is the file 0 and `table` the
/// file 1 of the archive 135. The trainer has a place in the list (a number of 16 bits, 0 for an entry that is not used, else the kind plus
/// one); the game reads at most 21 entries from there and takes the first entry of each kind. The line of a message is its place in the list.
pub fn message_lines(offsets: &[u8], table: &[u8], trainer: usize) -> Vec<(usize, usize)> {
    let entry = |bytes: &[u8], at: usize| bytes.get(at * 2..at * 2 + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let Some(start) = entry(offsets, trainer).map(|bytes| bytes / 2) else { return Vec::new() };
    let mut found: Vec<(usize, usize)> = Vec::new();
    for place in start..start + MESSAGE_KINDS.len() {
        let Some(value) = entry(table, place) else { break };
        if value != 0 && !found.iter().any(|(kind, _)| *kind == value - 1) {
            found.push((value - 1, place));
        }
    }
    found.sort_unstable();
    found
}

/// The archive of the 3D models of the battles (five files for each model).
pub const MODEL_ARCHIVE: usize = 133;
/// The archive of the pictures of the trainers and their number of files. Each class has three pictures (the whole body and two upper
/// bodies): the files `3 * picture` to `3 * picture + 2`, where `picture` is [`TrainerClass::picture_2d`].
pub const PICTURE_ARCHIVE: usize = 160;
pub const PICTURE_COUNT: usize = 684;
/// The kinds of pictures of a class.
pub const PICTURE_KINDS: [&str; 3] = ["Whole body", "Upper body", "Upper body (second)"];

/// The file of a picture of a class, when the class has it.
pub fn picture_file(class: &TrainerClass, kind: usize) -> Option<usize> {
    let file = class.picture_2d as usize * 3 + kind;
    (kind < 3 && file < PICTURE_COUNT).then_some(file)
}

/// The weathers that a team can start with: the value of the file and the name. 255 keeps the weather of the place.
pub const WEATHERS: [(u8, &str); 8] = [(255, "As the place"), (0, "None"), (1, "Harsh sunlight"), (2, "Rain"), (3, "Hail"), (4, "Sandstorm"), (5, "Heavy rain"), (6, "Extremely harsh sunlight")];
/// The terrains that a team can start with.
pub const TERRAINS: [&str; 4] = ["None", "Grassy Terrain", "Misty Terrain", "Electric Terrain"];
/// The effects on the whole field that a team can start with, one bit each in [`BattleExtras::field`].
pub const FIELD_EFFECTS: [(&str, &str); 4] = [
    ("Trick Room", "The slowest Pokémon move first."),
    ("Gravity", "The Pokémon on the ground: no more Flying type, no Levitate; the moves of the air work."),
    ("Magic Room", "The held items have no effect."),
    ("Wonder Room", "The Defense and the Sp. Def are exchanged."),
];

/// The most letters in a nickname.
pub const NICKNAME_LETTERS: usize = 12;

/// What a Pokémon of a team has more than the file of the game can give it: the nature, the shiny state, the exact ability, the EVs, the IVs
/// of each stat and a nickname. The plugin applies it when the battle starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PokemonExtras {
    /// The nature (0 to 24). 255 keeps the one that the game draws.
    pub nature: u8,
    pub shiny: bool,
    /// The number of the ability. 0 keeps the one of the team.
    pub ability: u16,
    /// The EVs: HP, Attack, Defense, Sp. Atk, Sp. Def, Speed.
    pub evs: [u8; 6],
    /// The IVs in the same order. 255 keeps the IVs of the team.
    pub ivs: [u8; 6],
    pub nickname: String,
}

impl PokemonExtras {
    /// The values that change nothing.
    pub fn none() -> PokemonExtras {
        PokemonExtras { nature: 255, shiny: false, ability: 0, evs: [0; 6], ivs: [255; 6], nickname: String::new() }
    }
}

/// What a team has more than the files of the game: the start of the battle and the extras of each Pokémon. The file `trainer/extra/<n>.bin`
/// (the plugin reads it when the battle with the trainer starts):
///
/// | Offset | Size | Content |
/// | --- | --- | --- |
/// | 0 | 4 | The text `STRX`. |
/// | 4 | 1 | The version: 1. |
/// | 5 | 1 | The number of Pokémon (up to 6). |
/// | 6 | 1 | The weather (see [`WEATHERS`]). |
/// | 7 | 1 | The terrain (see [`TERRAINS`]). |
/// | 8 | 2 | The effects on the field, one bit each ([`FIELD_EFFECTS`]). |
/// | 10 | 2 | 0. |
/// | 12 | 56 each | The Pokémon: the nature, the shiny state, the ability (16 bits), 6 EVs, 6 IVs, 14 bytes of 0 kept for the next versions, and the nickname (13 letters of 16 bits, ending with 0). |
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleExtras {
    pub weather: u8,
    pub terrain: u8,
    pub field: u16,
    pub pokemon: Vec<PokemonExtras>,
}

const EXTRAS_SIGNATURE: &[u8; 4] = b"STRX";
const EXTRAS_HEADER: usize = 12;
const EXTRAS_POKEMON: usize = 56;
/// Where the nickname starts in the record of a Pokémon.
const EXTRAS_NICKNAME: usize = 30;

impl BattleExtras {
    /// Extras that change nothing, for a team of `count` Pokémon.
    pub fn none(count: usize) -> BattleExtras {
        BattleExtras { weather: 255, terrain: 0, field: 0, pokemon: vec![PokemonExtras::none(); count] }
    }

    /// True when the extras change nothing: no file is needed.
    pub fn is_empty(&self) -> bool {
        self.weather == 255 && self.terrain == 0 && self.field == 0 && self.pokemon.iter().all(|pokemon| *pokemon == PokemonExtras::none())
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; EXTRAS_HEADER + EXTRAS_POKEMON * self.pokemon.len()];
        out[..4].copy_from_slice(EXTRAS_SIGNATURE);
        out[4] = 1;
        out[5] = self.pokemon.len() as u8;
        out[6] = self.weather;
        out[7] = self.terrain;
        out[8..10].copy_from_slice(&self.field.to_le_bytes());
        for (place, pokemon) in self.pokemon.iter().enumerate() {
            let at = EXTRAS_HEADER + EXTRAS_POKEMON * place;
            out[at] = pokemon.nature;
            out[at + 1] = u8::from(pokemon.shiny);
            out[at + 2..at + 4].copy_from_slice(&pokemon.ability.to_le_bytes());
            out[at + 4..at + 10].copy_from_slice(&pokemon.evs);
            out[at + 10..at + 16].copy_from_slice(&pokemon.ivs);
            for (letter, unit) in pokemon.nickname.encode_utf16().take(NICKNAME_LETTERS).enumerate() {
                out[at + EXTRAS_NICKNAME + 2 * letter..at + EXTRAS_NICKNAME + 2 + 2 * letter].copy_from_slice(&unit.to_le_bytes());
            }
        }
        out
    }

    /// Reads the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<BattleExtras, String> {
        if bytes.len() < EXTRAS_HEADER || &bytes[..4] != EXTRAS_SIGNATURE || bytes[4] != 1 {
            return Err("This is not a file of extras of a team (version 1).".into());
        }
        let count = bytes[5] as usize;
        if count > MAX_TEAM || bytes.len() < EXTRAS_HEADER + EXTRAS_POKEMON * count {
            return Err("The file of extras is cut.".into());
        }
        let pokemon = (0..count)
            .map(|place| {
                let at = EXTRAS_HEADER + EXTRAS_POKEMON * place;
                let units: Vec<u16> = (0..=NICKNAME_LETTERS).map(|letter| word(bytes, at + EXTRAS_NICKNAME + 2 * letter)).take_while(|unit| *unit != 0).collect();
                PokemonExtras {
                    nature: bytes[at],
                    shiny: bytes[at + 1] != 0,
                    ability: word(bytes, at + 2),
                    evs: bytes[at + 4..at + 10].try_into().unwrap(),
                    ivs: bytes[at + 10..at + 16].try_into().unwrap(),
                    nickname: String::from_utf16_lossy(&units),
                }
            })
            .collect();
        Ok(BattleExtras { weather: bytes[6], terrain: bytes[7], field: word(bytes, 8), pokemon })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_data_of_a_trainer_comes_back_as_it_was() {
        // The first rival of the game: the class 127, one Pokémon, no prize.
        let bytes: Vec<u8> = vec![1, 0, 0x7F, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0x0A, 0, 0, 0];
        let data = TrainerData::from_bytes(&bytes).unwrap();
        assert_eq!((data.class, data.pokemon_count, data.ai, data.heal_after), (127, 1, 0x100, false));
        assert_eq!(data.to_bytes(), bytes, "the unused bits stay");
        let mut changed = data;
        changed.heal_after = true;
        changed.money = 9;
        assert_eq!(TrainerData::from_bytes(&changed.to_bytes()).unwrap(), changed);
    }

    #[test]
    fn a_team_goes_through_its_four_forms() {
        let team = Team {
            pokemon: vec![TrainerPokemon { iv: 31, ability: 2, gender: 1, level: 50, species: 25, form: 0, item: 0x10E, moves: [85, 98, 0, 0] }],
            moves: true,
            items: true,
        };
        assert_eq!(team.data_type(), 3);
        let bytes = team.to_bytes();
        assert_eq!(bytes.len(), 18);
        assert_eq!(Team::from_bytes(3, 1, &bytes).unwrap(), team);
        let plain = Team { pokemon: vec![TrainerPokemon::new(1, 5)], moves: false, items: false };
        assert_eq!((plain.data_type(), plain.to_bytes().len()), (0, 8));
        assert!(Team::from_bytes(0, 2, &plain.to_bytes()).is_err(), "the file is too short");
        assert!(Team::from_bytes(9, 1, &bytes).is_err());
    }

    /// With the dump: every trainer, team and class of the game is read and written again byte for byte.
    #[test]
    fn the_trainers_of_the_game_come_back_byte_for_byte() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let data = crate::Garc::open(&dump.archive(DATA_ARCHIVE)).unwrap();
        let teams = crate::Garc::open(&dump.archive(TEAM_ARCHIVE)).unwrap();
        let classes = crate::Garc::open(&dump.archive(CLASS_ARCHIVE)).unwrap();
        assert_eq!((data.file_count(), teams.file_count(), classes.file_count()), (TRAINER_COUNT, TRAINER_COUNT, CLASS_COUNT));
        for id in 1..TRAINER_COUNT {
            let bytes = data.first_file(id).unwrap();
            let trainer = TrainerData::from_bytes(bytes).unwrap();
            assert_eq!(trainer.to_bytes(), &bytes[..DATA_SIZE], "trainer {id}");
            assert!((trainer.class as usize) < CLASS_COUNT, "trainer {id}");
            let team_bytes = teams.first_file(id).unwrap();
            let team = Team::from_bytes(trainer.data_type, trainer.pokemon_count as usize, team_bytes).unwrap();
            assert_eq!(team.data_type(), trainer.data_type, "trainer {id}");
            assert_eq!(team.to_bytes(), &team_bytes[..team.pokemon.len() * POKEMON_SIZE[trainer.data_type as usize]], "team {id}");
        }
        for id in 0..CLASS_COUNT {
            let bytes = classes.first_file(id).unwrap();
            assert_eq!(TrainerClass::from_bytes(bytes).unwrap().to_bytes(), bytes, "class {id}");
        }
    }

    #[test]
    fn a_class_comes_back_as_it_was() {
        let bytes: Vec<u8> = vec![0x10, 0, 0x24, 0x71, 0, 0, 0, 1, 0, 0, 0, 0x15, 0, 1, 0, 0];
        let class = TrainerClass::from_bytes(&bytes).unwrap();
        assert_eq!(class.to_bytes(), bytes);
        assert!(TrainerClass::from_bytes(&bytes[..8]).is_err());
    }

    #[test]
    fn the_extras_of_a_team_come_back_as_they_were() {
        let mut extras = BattleExtras::none(2);
        assert!(extras.is_empty());
        extras.weather = 2;
        extras.terrain = 3;
        extras.field = 0b101;
        extras.pokemon[1] = PokemonExtras { nature: 3, shiny: true, ability: 70, evs: [0, 252, 0, 0, 4, 252], ivs: [31, 31, 31, 0, 31, 31], nickname: "Ash".into() };
        assert!(!extras.is_empty());
        let bytes = extras.to_bytes();
        assert_eq!(bytes.len(), 12 + 56 * 2);
        assert_eq!(BattleExtras::from_bytes(&bytes).unwrap(), extras);
        assert!(BattleExtras::from_bytes(&bytes[..20]).is_err());
        assert!(BattleExtras::from_bytes(b"NOPE00000000").is_err());
    }

    /// With the dump: the messages and the pictures of the game are found like the game finds them.
    #[test]
    fn the_messages_and_the_pictures_of_the_game_are_found() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let messages = crate::Garc::open(&dump.archive(MESSAGE_ARCHIVE)).unwrap();
        let (offsets, table) = (messages.first_file(0).unwrap(), messages.first_file(1).unwrap());
        // The first trainer has a message before the battle, one when it loses and one after the battle.
        assert_eq!(message_lines(offsets, table, 1), vec![(0, 0), (1, 1), (2, 2)]);
        let en = crate::Garc::open(&dump.archive(crate::text::LANGUAGES[2].archive)).unwrap();
        let texts = crate::text::TextFile::from_bytes(en.first_file(MESSAGE_TEXT).unwrap()).unwrap();
        assert!(texts.len() + 1 >= table.len() / 2, "one line for each place of the list (the last place has none)");
        let pictures = crate::Garc::open(&dump.archive(PICTURE_ARCHIVE)).unwrap();
        assert_eq!(pictures.file_count(), PICTURE_COUNT);
        let classes = crate::Garc::open(&dump.archive(CLASS_ARCHIVE)).unwrap();
        for id in 0..CLASS_COUNT {
            let class = TrainerClass::from_bytes(classes.first_file(id).unwrap()).unwrap();
            for kind in 0..3 {
                assert!(picture_file(&class, kind).is_some(), "class {id} has no picture {kind}");
            }
        }
    }
}
