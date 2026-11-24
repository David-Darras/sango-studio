//! The reactions of the abilities, of the moves and of the held items of the game, written as rules.
//!
//! The game runs its own code for its abilities. When an ability of the game has no file `ability/<id>.bin`, the plugin keeps that
//! code. The Studio shows the reaction of the game as a rule here, so that the user can change one value of it. When the user saves a
//! change, the Studio writes the file and the plugin runs the rules in place of the code of the game.
//!
//! Only the abilities whose rules do the same thing as the code of the game are here. Some values are rounded: the rules use
//! percents, and the game uses fractions (1/8 of the HP is 12%, 1/16 is 6%, 1/3 of the HP is "under 34%"). An ability that is not here
//! has no rules: its effect needs a part of the code of the game that the rules do not have (the module
//! [`crate::reasons`] says why).

use crate::{Action, ActionKind as A, Condition, ConditionKind as C, Rule, TARGET_FOES};

/// The numbers of the types (the order of the game).
mod ty {
    pub const NORMAL: i16 = 0;
    pub const FIGHTING: i16 = 1;
    pub const POISON: i16 = 3;
    pub const GROUND: i16 = 4;
    pub const ROCK: i16 = 5;
    pub const STEEL: i16 = 8;
    pub const PSYCHIC: i16 = 13;
    pub const DRAGON: i16 = 15;
    pub const FLYING: i16 = 2;
    pub const BUG: i16 = 6;
    pub const GHOST: i16 = 7;
    pub const FIRE: i16 = 9;
    pub const WATER: i16 = 10;
    pub const GRASS: i16 = 11;
    pub const ELECTRIC: i16 = 12;
    pub const ICE: i16 = 14;
    pub const DARK: i16 = 16;
    pub const FAIRY: i16 = 17;
}

/// The numbers of the weathers.
mod weather {
    pub const SUN: i16 = 1;
    pub const RAIN: i16 = 2;
    pub const HAIL: i16 = 3;
    pub const SAND: i16 = 4;
    pub const HEAVY_RAIN: i16 = 5;
    pub const HARSH_SUN: i16 = 6;
}

/// The numbers of the statistics.
mod stat {
    pub const ATTACK: i16 = 1;
    pub const DEFENSE: i16 = 2;
    pub const SP_ATK: i16 = 3;
    pub const SP_DEF: i16 = 4;
    pub const SPEED: i16 = 5;
}

/// The numbers of the status conditions.
mod status {
    pub const ANY: i16 = 0;
    pub const PARALYSIS: i16 = 1;
    pub const SLEEP: i16 = 2;
    pub const FREEZE: i16 = 3;
    pub const BURN: i16 = 4;
    pub const POISON: i16 = 5;
    pub const CONFUSION: i16 = 6;
    pub const INFATUATION: i16 = 7;
    pub const TAUNT: i16 = 11;
    pub const YAWN: i16 = 14;
}

/// The moments.
mod at {
    pub const ENTER: u16 = 94;
    pub const NEW_ABILITY: u16 = 148;
    pub const PRIORITY: u16 = 17;
    pub const SPEED: u16 = 19;
    pub const MOVE_DATA: u16 = 40;
    pub const MOVE_CHECK: u16 = 49;
    pub const ACCURACY: u16 = 57;
    pub const CRITICAL: u16 = 59;
    pub const POWER: u16 = 61;
    pub const ATTACK: u16 = 64;
    pub const DEFENSE: u16 = 65;
    pub const DAMAGE: u16 = 79;
    pub const HIT: u16 = 83;
    pub const STAT_EFFECT_CHANCE: u16 = 90;
    pub const STAT_CHANGE: u16 = 100;
    pub const STATUS_EFFECT_CHANCE: u16 = 109;
    pub const STATUS: u16 = 110;
    pub const FLINCH_CHANCE: u16 = 117;
    pub const FLINCH: u16 = 118;
    pub const TURN_END: u16 = 128;
    pub const SPECIAL_EFFECT_CHANCE: u16 = 181;
    pub const NO_EFFECT: u16 = 38;
    pub const SKIP_ACCURACY: u16 = 55;
    pub const FIXED_DAMAGE: u16 = 78;
    pub const MOVE_DAMAGE_DONE: u16 = 140;
    pub const ENDURE: u16 = 125;
    pub const TURN_BEGIN: u16 = 127;
}

/// The targets.
const OWNER: u8 = 0;
const MOVE_USER: u8 = 2;

fn when(kind: C) -> Condition {
    Condition { kind, arg: 0 }
}

fn when_arg(kind: C, arg: i16) -> Condition {
    Condition { kind, arg }
}

fn act(kind: A, target: u8, a: i16, b: i16, banner: bool) -> Action {
    Action { kind, target, a, b, banner }
}

fn rule(moment: u16, conditions: Vec<Condition>, actions: Vec<Action>) -> Rule {
    Rule { moment, conditions, actions }
}

/// Multiplies the value of a moment.
fn multiply(percent: i16) -> Action {
    act(A::MultiplyValue, OWNER, 0, percent, false)
}

/// The same rule at the entry of the Pokémon and when it gets the ability.
fn on_entry(actions: Vec<Action>) -> Vec<Rule> {
    [at::ENTER, at::NEW_ABILITY].into_iter().map(|moment| rule(moment, vec![when(C::PokemonIsOwner)], actions.clone())).collect()
}

/// A weather when the Pokémon enters (Drizzle, Drought...).
fn weather_on_entry(value: i16) -> Vec<Rule> {
    on_entry(vec![act(A::SetWeather, OWNER, value, 0, true)])
}

/// A speed x2 in a weather (Swift Swim, Chlorophyll...).
fn speed_in_weather(weathers: &[i16]) -> Vec<Rule> {
    weathers.iter().map(|w| rule(at::SPEED, vec![when(C::PokemonIsOwner), when_arg(C::WeatherIs, *w)], vec![multiply(200)])).collect()
}

/// The power x1.5 of a type at a third of the HP (Overgrow, Blaze...).
fn pinch_boost(move_type: i16) -> Vec<Rule> {
    vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::MoveTypeIs, move_type), when_arg(C::HpBelow, 34)], vec![multiply(150)])]
}

/// Heals 1/16 of the HP at the end of the turn in a weather (Rain Dish, Ice Body).
fn heal_in_weather(weathers: &[i16]) -> Vec<Rule> {
    weathers
        .iter()
        .map(|w| rule(at::TURN_END, vec![when(C::PokemonIsOwner), when(C::HpNotFull), when_arg(C::WeatherIs, *w)], vec![act(A::Heal, OWNER, 6, 0, true)]))
        .collect()
}

/// Damage to the attacker that makes contact (Rough Skin, Iron Barbs).
fn contact_damage() -> Vec<Rule> {
    vec![rule(at::HIT, vec![when(C::DefenderIsOwner), when(C::CausedByOther), when(C::MoveMakesContact)], vec![act(A::DamageHp, MOVE_USER, 12, 0, true)])]
}

/// A status condition to the attacker that makes contact, 30% of the time (Poison Point, Flame Body).
fn contact_status(value: i16) -> Vec<Rule> {
    vec![rule(
        at::HIT,
        vec![when(C::DefenderIsOwner), when(C::CausedByOther), when(C::MoveMakesContact), when(C::NoSubstitute), when_arg(C::Chance, 30)],
        vec![act(A::GiveStatus, MOVE_USER, value, 0, true)],
    )]
}

/// Blocks status conditions (Limber, Insomnia...).
fn block_statuses(values: &[i16]) -> Vec<Rule> {
    vec![rule(at::STATUS, vec![when(C::DefenderIsOwner)], values.iter().map(|v| act(A::BlockStatus, OWNER, *v, 0, false)).collect())]
}

/// Blocks the stat drops that other Pokémon cause (Clear Body, Hyper Cutter...). 0 is every stat.
fn block_drops(value: i16) -> Vec<Rule> {
    vec![rule(at::STAT_CHANGE, vec![when(C::PokemonIsOwner), when(C::CausedByOther)], vec![act(A::BlockStatDrop, OWNER, value, 0, false)])]
}

/// The moves of a type have no effect, then something happens (Volt Absorb, Sap Sipper...).
fn absorb(move_type: i16, then: Action) -> Vec<Rule> {
    vec![rule(
        at::MOVE_CHECK,
        vec![when(C::DefenderIsOwner), when(C::CausedByOther), when_arg(C::MoveTypeIs, move_type)],
        vec![act(A::BlockMove, OWNER, 0, 0, false), then],
    )]
}

/// A move that hits the owner has no effect (Soundproof, Bulletproof).
fn immune(condition: Condition) -> Vec<Rule> {
    vec![rule(at::MOVE_CHECK, vec![when(C::DefenderIsOwner), when(C::CausedByOther), condition], vec![act(A::BlockMove, OWNER, 0, 0, true)])]
}

/// A stat goes up when a move of a type hits the owner (Justified, Rattled).
fn boost_when_hit(types: &[i16], stat_value: i16) -> Vec<Rule> {
    types
        .iter()
        .map(|t| {
            rule(at::HIT, vec![when(C::DefenderIsOwner), when(C::NoSubstitute), when_arg(C::MoveTypeIs, *t)], vec![act(A::ChangeStat, OWNER, stat_value, 1, true)])
        })
        .collect()
}

/// Normal moves get a type and x1.3 power (Refrigerate, Pixilate, Aerilate).
fn skin(new_type: i16) -> Vec<Rule> {
    vec![
        rule(at::MOVE_DATA, vec![when(C::PokemonIsOwner), when_arg(C::MoveTypeIs, ty::NORMAL)], vec![act(A::ChangeMoveType, OWNER, new_type, 0, false)]),
        rule(at::POWER, vec![when(C::MoveUserIsOwner), when(C::MoveTypeWasChanged)], vec![multiply(130)]),
    ]
}

/// The numbers of the abilities of the game that have a reaction as rules, in order.
pub fn game_abilities() -> Vec<u8> {
    (1..=191).filter(|id| game_rules(*id).is_some()).collect()
}

/// The reaction of an ability of the game as rules, or `None` when the rules cannot do what the code of the game does.
pub fn game_rules(ability: u8) -> Option<Vec<Rule>> {
    let rules = match ability {
        // Drizzle, Sand Stream, Drought, Snow Warning.
        2 => weather_on_entry(weather::RAIN),
        45 => weather_on_entry(weather::SAND),
        70 => weather_on_entry(weather::SUN),
        117 => weather_on_entry(weather::HAIL),
        // Battle Armor, Shell Armor.
        4 | 75 => vec![rule(at::CRITICAL, vec![when(C::DefenderIsOwner)], vec![act(A::BlockCritical, OWNER, 0, 0, false)])],
        // Limber, Oblivious, Insomnia, Immunity, Own Tempo, Magma Armor, Water Veil, Vital Spirit.
        7 => block_statuses(&[status::PARALYSIS]),
        12 => block_statuses(&[status::INFATUATION, status::TAUNT]),
        15 | 72 => block_statuses(&[status::SLEEP, status::YAWN]),
        17 => block_statuses(&[status::POISON]),
        20 => block_statuses(&[status::CONFUSION]),
        40 => block_statuses(&[status::FREEZE]),
        41 => block_statuses(&[status::BURN]),
        // Volt Absorb, Water Absorb: the move heals 1/4 of the HP.
        10 => absorb(ty::ELECTRIC, act(A::Heal, OWNER, 25, 0, true)),
        11 => absorb(ty::WATER, act(A::Heal, OWNER, 25, 0, true)),
        // Motor Drive, Sap Sipper: the move raises a stat.
        78 => absorb(ty::ELECTRIC, act(A::ChangeStat, OWNER, stat::SPEED, 1, true)),
        157 => absorb(ty::GRASS, act(A::ChangeStat, OWNER, stat::ATTACK, 1, true)),
        // Compound Eyes.
        14 => vec![rule(at::ACCURACY, vec![when(C::MoveUserIsOwner)], vec![multiply(130)])],
        // Intimidate: the Attack of every foe next to the Pokémon goes down.
        22 => on_entry(vec![act(A::ChangeStat, TARGET_FOES, stat::ATTACK, -1, true)]),
        // Rough Skin, Iron Barbs.
        24 | 160 => contact_damage(),
        // Clear Body, White Smoke, Hyper Cutter, Big Pecks.
        29 | 73 => block_drops(0),
        52 => block_drops(stat::ATTACK),
        145 => block_drops(stat::DEFENSE),
        // Serene Grace: the chances of the secondary effects x2.
        32 => [at::STATUS_EFFECT_CHANCE, at::STAT_EFFECT_CHANCE, at::SPECIAL_EFFECT_CHANCE, at::FLINCH_CHANCE]
            .into_iter()
            .map(|moment| rule(moment, vec![when(C::MoveUserIsOwner)], vec![act(A::MultiplyEffectChance, OWNER, 0, 200, false)]))
            .collect(),
        // Swift Swim, Chlorophyll, Sand Rush.
        33 => speed_in_weather(&[weather::RAIN, weather::HEAVY_RAIN]),
        34 => speed_in_weather(&[weather::SUN, weather::HARSH_SUN]),
        146 => speed_in_weather(&[weather::SAND]),
        // Huge Power, Pure Power.
        37 | 74 => vec![rule(at::ATTACK, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 1)], vec![multiply(200)])],
        // Poison Point, Flame Body.
        38 => contact_status(status::POISON),
        49 => contact_status(status::BURN),
        // Inner Focus.
        39 => vec![rule(at::FLINCH, vec![when(C::DefenderIsOwner)], vec![act(A::BlockFlinch, OWNER, 0, 0, false)])],
        // Soundproof, Bulletproof.
        43 => immune(when_arg(C::MoveHasFlag, 8)),
        171 => immune(when_arg(C::MoveInGroup, 3)),
        // Rain Dish, Ice Body.
        44 => heal_in_weather(&[weather::RAIN, weather::HEAVY_RAIN]),
        115 => heal_in_weather(&[weather::HAIL]),
        // Thick Fat: the Fire and Ice moves that hit the Pokémon do half damage.
        47 => [ty::FIRE, ty::ICE]
            .into_iter()
            .map(|t| rule(at::ATTACK, vec![when(C::DefenderIsOwner), when_arg(C::MoveTypeIs, t)], vec![multiply(50)]))
            .collect(),
        // Hustle.
        55 => vec![
            rule(at::ATTACK, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 1)], vec![multiply(150)]),
            rule(at::ACCURACY, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 1)], vec![multiply(80)]),
        ],
        // Marvel Scale.
        63 => vec![rule(at::DEFENSE, vec![when(C::DefenderIsOwner), when_arg(C::OwnerHasStatus, status::ANY), when_arg(C::MoveCategoryIs, 1)], vec![multiply(150)])],
        // Overgrow, Blaze, Torrent, Swarm.
        65 => pinch_boost(ty::GRASS),
        66 => pinch_boost(ty::FIRE),
        67 => pinch_boost(ty::WATER),
        68 => pinch_boost(ty::BUG),
        // Anger Point: a critical hit raises the Attack to the maximum.
        83 => vec![rule(at::HIT, vec![when(C::DefenderIsOwner), when(C::NoSubstitute), when(C::CriticalHit)], vec![act(A::ChangeStat, OWNER, stat::ATTACK, 12, true)])],
        // Iron Fist.
        89 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::MoveHasFlag, 7)], vec![multiply(120)])],
        // Solar Power.
        94 => [weather::SUN, weather::HARSH_SUN]
            .into_iter()
            .flat_map(|w| {
                [
                    rule(at::ATTACK, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 2), when_arg(C::WeatherIs, w)], vec![multiply(150)]),
                    rule(at::TURN_END, vec![when(C::PokemonIsOwner), when_arg(C::WeatherIs, w)], vec![act(A::DamageHp, OWNER, 12, 0, true)]),
                ]
            })
            .collect(),
        // Sniper.
        97 => vec![rule(at::DAMAGE, vec![when(C::MoveUserIsOwner), when(C::CriticalHit)], vec![multiply(150)])],
        // Technician.
        101 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::PowerAtMost, 60)], vec![multiply(150)])],
        // Super Luck.
        105 => vec![rule(at::CRITICAL, vec![when(C::MoveUserIsOwner)], vec![act(A::RaiseCritical, OWNER, 1, 0, false)])],
        // Tinted Lens.
        110 => vec![rule(at::DAMAGE, vec![when(C::MoveUserIsOwner), when(C::NotVeryEffective)], vec![multiply(200)])],
        // Filter, Solid Rock.
        111 | 116 => vec![rule(at::DAMAGE, vec![when(C::DefenderIsOwner), when(C::SuperEffective)], vec![multiply(75)])],
        // Defeatist.
        129 => vec![rule(at::ATTACK, vec![when(C::MoveUserIsOwner), when_arg(C::HpBelow, 50)], vec![multiply(50)])],
        // Multiscale.
        136 => vec![rule(at::DAMAGE, vec![when(C::DefenderIsOwner), when(C::HpFull)], vec![multiply(50)])],
        // Toxic Boost, Flare Boost.
        137 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::OwnerHasStatus, status::POISON), when_arg(C::MoveCategoryIs, 1)], vec![multiply(150)])],
        138 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::OwnerHasStatus, status::BURN), when_arg(C::MoveCategoryIs, 2)], vec![multiply(150)])],
        // Justified, Rattled.
        154 => boost_when_hit(&[ty::DARK], stat::ATTACK),
        155 => boost_when_hit(&[ty::DARK, ty::GHOST, ty::BUG], stat::SPEED),
        // Prankster, Gale Wings.
        158 => vec![rule(at::PRIORITY, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 0)], vec![act(A::RaisePriority, OWNER, 1, 0, false)])],
        177 => vec![rule(at::PRIORITY, vec![when(C::MoveUserIsOwner), when_arg(C::MoveTypeIs, ty::FLYING)], vec![act(A::RaisePriority, OWNER, 1, 0, false)])],
        // Fur Coat.
        169 => vec![rule(at::DEFENSE, vec![when(C::DefenderIsOwner), when_arg(C::MoveCategoryIs, 1)], vec![multiply(200)])],
        // Refrigerate, Pixilate, Aerilate.
        174 => skin(ty::ICE),
        182 => skin(ty::FAIRY),
        184 => skin(ty::FLYING),
        // Tough Claws.
        181 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when(C::MoveMakesContact)], vec![multiply(130)])],
        _ => return None,
    };
    Some(rules)
}

/// The numbers of the moves of the game that have a reaction as rules, in order.
pub fn game_moves() -> Vec<u16> {
    (1..=621).filter(|id| game_move_rules(*id).is_some()).collect()
}

/// A move power x2 when a condition is true (Hex, Venoshock, Brine, Acrobatics).
fn double_power(condition: Condition) -> Vec<Rule> {
    vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), condition], vec![multiply(200)])]
}

/// A fixed damage (Sonic Boom, Seismic Toss, Super Fang).
fn fixed_damage(value: i16) -> Vec<Rule> {
    vec![rule(at::FIXED_DAMAGE, vec![when(C::MoveUserIsOwner)], vec![act(A::FixedDamage, OWNER, value, 0, false)])]
}

/// The reaction of a move of the game as rules, or `None` when the rules cannot do what the code of the game does. The owner of a
/// rule of a move is the Pokémon that uses the move.
pub fn game_move_rules(id: u16) -> Option<Vec<Rule>> {
    let rules = match id {
        // Jump Kick, High Jump Kick: the user loses half of its HP when the move fails.
        26 | 136 => vec![rule(at::NO_EFFECT, vec![when(C::PokemonIsOwner)], vec![act(A::DamageHp, OWNER, 50, 0, false)])],
        // Sonic Boom, Dragon Rage, Seismic Toss, Night Shade, Super Fang.
        49 => fixed_damage(20),
        82 => fixed_damage(40),
        69 | 101 => fixed_damage(0),
        162 => fixed_damage(-50),
        // Blizzard never misses in hail.
        59 => vec![rule(at::SKIP_ACCURACY, vec![when(C::MoveUserIsOwner), when_arg(C::WeatherIs, weather::HAIL)], vec![act(A::NeverMiss, OWNER, 0, 0, false)])],
        // False Swipe, Hold Back.
        206 | 610 => vec![rule(at::ENDURE, vec![when(C::MoveUserIsOwner)], vec![act(A::LeaveOneHp, OWNER, 0, 0, false)])],
        // Weather Ball: the type of the weather and x2 power.
        311 => {
            let mut rules: Vec<Rule> = [
                (weather::SUN, ty::FIRE),
                (weather::HARSH_SUN, ty::FIRE),
                (weather::RAIN, ty::WATER),
                (weather::HEAVY_RAIN, ty::WATER),
                (weather::HAIL, ty::ICE),
                (weather::SAND, ty::ROCK),
            ]
            .into_iter()
            .map(|(w, t)| rule(at::MOVE_DATA, vec![when(C::PokemonIsOwner), when_arg(C::WeatherIs, w)], vec![act(A::ChangeMoveType, OWNER, t, 0, false)]))
            .collect();
            rules.push(rule(at::POWER, vec![when(C::MoveUserIsOwner), when(C::MoveTypeWasChanged)], vec![multiply(200)]));
            rules
        }
        // Brine, Venoshock, Hex, Acrobatics.
        362 => double_power(when_arg(C::OtherHpBelow, 51)),
        474 => double_power(when_arg(C::OtherHasStatus, status::POISON)),
        506 => double_power(when_arg(C::OtherHasStatus, status::ANY)),
        512 => double_power(when(C::OwnerHoldsNoItem)),
        // Fell Stinger: the Attack +2 when the move makes the target faint.
        565 => vec![rule(at::MOVE_DAMAGE_DONE, vec![when(C::MoveUserIsOwner), when(C::TargetFainted)], vec![act(A::ChangeStat, OWNER, stat::ATTACK, 2, false)])],
        _ => return None,
    };
    Some(rules)
}

/// The numbers of the held items of the game that have a reaction as rules, in order.
pub fn game_items() -> Vec<u16> {
    (1..=775).filter(|id| game_item_rules(*id).is_some()).collect()
}

/// x1.2 power for the moves of a type (Charcoal, the plates, the incenses).
fn type_boost(move_type: i16) -> Vec<Rule> {
    vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::MoveTypeIs, move_type)], vec![multiply(120)])]
}

/// A stat +1 when a move of a type hits the holder, then the item is used up (Absorb Bulb, Cell Battery...).
fn boost_and_use(move_type: i16, stat_value: i16) -> Vec<Rule> {
    vec![rule(
        at::HIT,
        vec![when(C::DefenderIsOwner), when(C::NoSubstitute), when_arg(C::MoveTypeIs, move_type)],
        vec![act(A::ChangeStat, OWNER, stat_value, 1, false), act(A::ConsumeItem, OWNER, 0, 0, false)],
    )]
}

/// The reaction of a held item of the game as rules, or `None` when the rules cannot do what the code of the game does. The owner of
/// a rule of an item is the Pokémon that holds it. The values are the ones of the data of the items of the game.
pub fn game_item_rules(id: u16) -> Option<Vec<Rule>> {
    let rules = match id {
        // The items that raise the power of a type by 20%.
        222 | 308 => type_boost(ty::BUG),
        233 | 313 => type_boost(ty::STEEL),
        237 | 305 => type_boost(ty::GROUND),
        238 | 309 | 315 => type_boost(ty::ROCK),
        239 | 301 | 318 => type_boost(ty::GRASS),
        240 | 312 => type_boost(ty::DARK),
        241 | 303 => type_boost(ty::FIGHTING),
        242 | 300 => type_boost(ty::ELECTRIC),
        243 | 254 | 299 | 317 => type_boost(ty::WATER),
        244 | 306 => type_boost(ty::FLYING),
        246 | 302 => type_boost(ty::ICE),
        247 | 310 => type_boost(ty::GHOST),
        248 | 307 | 314 => type_boost(ty::PSYCHIC),
        249 | 298 => type_boost(ty::FIRE),
        250 | 311 => type_boost(ty::DRAGON),
        251 => type_boost(ty::NORMAL),
        304 => type_boost(ty::POISON),
        644 => type_boost(ty::FAIRY),
        // Bright Powder, Lax Incense: the moves against the holder hit 10% less.
        213 | 255 => vec![rule(at::ACCURACY, vec![when(C::DefenderIsOwner)], vec![multiply(90)])],
        // Macho Brace and the Power items: the Speed is halved.
        215 | 289..=294 => vec![rule(at::SPEED, vec![when(C::PokemonIsOwner)], vec![multiply(50)])],
        // Scope Lens, Razor Claw.
        232 | 326 => vec![rule(at::CRITICAL, vec![when(C::MoveUserIsOwner)], vec![act(A::RaiseCritical, OWNER, 1, 0, false)])],
        // Leftovers: 1/16 of the HP at the start of the end of the turn.
        234 => vec![rule(at::TURN_BEGIN, vec![when(C::PokemonIsOwner), when(C::HpNotFull)], vec![act(A::Heal, OWNER, 6, 0, false)])],
        // Wide Lens.
        265 => vec![rule(at::ACCURACY, vec![when(C::MoveUserIsOwner)], vec![multiply(110)])],
        // Muscle Band, Wise Glasses.
        266 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 1)], vec![multiply(110)])],
        267 => vec![rule(at::POWER, vec![when(C::MoveUserIsOwner), when_arg(C::MoveCategoryIs, 2)], vec![multiply(110)])],
        // Expert Belt.
        268 => vec![rule(at::DAMAGE, vec![when(C::MoveUserIsOwner), when(C::SuperEffective)], vec![multiply(120)])],
        // Life Orb: x1.3 damage, then the holder loses 1/10 of its HP.
        270 => vec![
            rule(at::DAMAGE, vec![when(C::MoveUserIsOwner)], vec![multiply(130)]),
            rule(at::MOVE_DAMAGE_DONE, vec![when(C::MoveUserIsOwner)], vec![act(A::DamageHp, OWNER, 10, 0, false)]),
        ],
        // Rocky Helmet: 1/6 of the HP of the attacker that makes contact.
        540 => vec![rule(at::HIT, vec![when(C::DefenderIsOwner), when(C::NoSubstitute), when(C::MoveMakesContact)], vec![act(A::DamageHp, MOVE_USER, 16, 0, false)])],
        // Absorb Bulb, Cell Battery, Luminous Moss, Snowball.
        545 => boost_and_use(ty::WATER, stat::SP_ATK),
        546 => boost_and_use(ty::ELECTRIC, stat::ATTACK),
        648 => boost_and_use(ty::WATER, stat::SP_DEF),
        649 => boost_and_use(ty::ICE, stat::ATTACK),
        _ => return None,
    };
    Some(rules)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{problems, read_rules, write_rules, Context, Reader, MAX_ACTIONS, MAX_CONDITIONS, MAX_RULES};

    #[test]
    fn the_list_has_every_ability_with_rules() {
        let list = game_abilities();
        assert_eq!(list.len(), 68, "{list:?}");
        assert!(list.contains(&22) && list.contains(&184) && !list.contains(&5));
    }

    #[test]
    fn every_game_reaction_is_a_valid_file() {
        for id in 1..=191u8 {
            let Some(rules) = game_rules(id) else { continue };
            assert!(!rules.is_empty() && rules.len() <= MAX_RULES, "ability {id}");
            for rule in &rules {
                assert!(rule.conditions.len() <= MAX_CONDITIONS && rule.actions.len() <= MAX_ACTIONS, "ability {id}");
            }
            let found = problems(&rules, Context::Ability);
            assert!(found.is_empty(), "ability {id}: {found:?}");
            let mut bytes = Vec::new();
            write_rules(&mut bytes, &rules);
            assert_eq!(read_rules(&mut Reader::new(&bytes), rules.len()).unwrap(), rules, "ability {id}");
        }
    }

    #[test]
    fn every_game_move_reaction_is_a_valid_file() {
        let list = game_moves();
        assert_eq!(list, vec![26, 49, 59, 69, 82, 101, 136, 162, 206, 311, 362, 474, 506, 512, 565, 610]);
        for id in list {
            let rules = game_move_rules(id).unwrap();
            assert!(rules.len() <= MAX_RULES, "move {id}");
            let found = problems(&rules, Context::Move);
            assert!(found.is_empty(), "move {id}: {found:?}");
            for rule in &rules {
                assert!(crate::moments(Context::Move).iter().any(|m| m.value == rule.moment), "move {id}: the editor offers the moment {}", rule.moment);
            }
        }
        assert_eq!(crate::rule_sentence(&game_move_rules(162).unwrap()[0]), "When the fixed damage is set, and this Pokémon used the move: do 50% of the HP that the target has.");
    }

    #[test]
    fn every_game_item_reaction_is_a_valid_file() {
        let list = game_items();
        assert_eq!(list.len(), 60, "{list:?}");
        assert!(game_item_rules(245).is_none(), "Poison Barb also poisons when it is flung: no rules");
        for id in list {
            let rules = game_item_rules(id).unwrap();
            let found = problems(&rules, Context::Item);
            assert!(found.is_empty(), "item {id}: {found:?}");
            for rule in &rules {
                assert!(crate::moments(Context::Item).iter().any(|m| m.value == rule.moment), "item {id}: the editor offers the moment {}", rule.moment);
            }
        }
    }

    #[test]
    fn intimidate_lowers_the_attack_of_every_foe() {
        let rules = game_rules(22).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].moment, 94);
        assert_eq!(rules[0].actions[0].target, TARGET_FOES);
        assert_eq!(crate::rule_sentence(&rules[0]), "When a Pokémon enters the battle, and it is this Pokémon: lower the Attack of every foe next to this Pokémon by 1 stage (the banner shows).");
    }

    #[test]
    fn the_type_numbers_are_the_ones_of_the_game() {
        for (value, name) in [(ty::FLYING, "Flying"), (ty::BUG, "Bug"), (ty::GHOST, "Ghost"), (ty::FIRE, "Fire"), (ty::WATER, "Water"), (ty::GRASS, "Grass"), (ty::ELECTRIC, "Electric"), (ty::ICE, "Ice"), (ty::DARK, "Dark"), (ty::FAIRY, "Fairy")] {
            assert_eq!(crate::type_name(value), name);
        }
    }
}
