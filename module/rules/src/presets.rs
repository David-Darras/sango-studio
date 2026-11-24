//! The reactions of the abilities of the game, written as rules.
//!
//! The game runs its own code for its abilities. When an ability of the game has no file `ability/<id>.bin`, the plugin keeps that
//! code. The Studio shows the reaction of the game as a rule here, so that the user can change one value of it. When the user saves a
//! change, the Studio writes the file and the plugin runs the rules in place of the code of the game.
//!
//! Only the abilities whose rules do the same thing as the code of the game are here. Some values are rounded: the rules use
//! percents, and the game uses fractions (1/8 of the HP is 12%, 1/16 is 6%, 1/3 of the HP is "under 34%"). An ability that is not here
//! has no rules: its effect needs a part of the code of the game that the rules do not have (see the document
//! `docs/ability-reactions.md`).

use crate::{Action, ActionKind as A, Condition, ConditionKind as C, Rule, TARGET_FOES};

/// The numbers of the types (the order of the game).
mod ty {
    pub const NORMAL: i16 = 0;
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
