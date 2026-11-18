//! The text format of Pokémon Showdown for a team: export and import.
//!
//! A Pokémon is a block of lines:
//!
//! ```text
//! Nickname (Species) (F) @ Item
//! Ability: Name
//! Level: 50
//! Shiny: Yes
//! EVs: 252 HP / 252 SpA / 4 Spe
//! Timid Nature
//! IVs: 0 Atk
//! - Move
//! ```
//!
//! The blocks are separated by an empty line. Only what the trainers can use is kept: the happiness has no place in the data.

use sango_core::trainer::{BattleExtras, PokemonExtras, Team, TrainerPokemon, MAX_TEAM, NICKNAME_LETTERS};
use sango_species::{ABILITY_NAMES, ITEM_NAMES, MOVE_NAMES, SPECIES};

/// The natures, in the order of the game.
pub const NATURES: [&str; 25] = [
    "Hardy", "Lonely", "Brave", "Adamant", "Naughty", "Bold", "Docile", "Relaxed", "Impish", "Lax", "Timid", "Hasty", "Serious", "Jolly", "Naive", "Modest", "Mild", "Quiet", "Bashful", "Rash", "Calm", "Gentle", "Sassy", "Careful", "Quirky",
];
/// The short names of the stats in the format: HP, Attack, Defense, Sp. Atk, Sp. Def, Speed.
const STATS: [&str; 6] = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];

/// The name without capitals, accents and signs: "Mr. Mime" and "mrmime" are the same name.
pub fn simple(text: &str) -> String {
    let mut out = String::new();
    for letter in text.chars() {
        match letter {
            '♀' => out.push('f'),
            '♂' => out.push('m'),
            'é' | 'É' | 'è' | 'ê' => out.push('e'),
            letter if letter.is_ascii_alphanumeric() => out.push(letter.to_ascii_lowercase()),
            _ => {}
        }
    }
    out
}

/// The number (from 1) of a name in a list, or 0.
fn find(list: &[&str], text: &str) -> usize {
    let wanted = simple(text);
    if wanted.is_empty() {
        return 0;
    }
    list.iter().position(|name| simple(name) == wanted).map_or(0, |at| at + 1)
}

/// The name of a thing, or an empty text.
fn name_of(list: &[&str], number: usize) -> String {
    number.checked_sub(1).and_then(|at| list.get(at)).map(|name| name.to_string()).unwrap_or_default()
}

/// Writes a team in the format. `ability_of` gives the ability of a species for a place of the data (1, 2 or 3), for the Pokémon that
/// have no exact ability.
pub fn export(team: &Team, extras: &BattleExtras, ability_of: &dyn Fn(usize, u8) -> u16) -> String {
    let mut blocks = Vec::new();
    for (place, pokemon) in team.pokemon.iter().enumerate() {
        let more = extras.pokemon.get(place).cloned().unwrap_or_else(PokemonExtras::none);
        let species = name_of(&SPECIES, pokemon.species as usize);
        let mut header = if more.nickname.is_empty() { species } else { format!("{} ({})", more.nickname, species) };
        match pokemon.gender {
            1 => header.push_str(" (M)"),
            2 => header.push_str(" (F)"),
            _ => {}
        }
        if team.items && pokemon.item != 0 {
            header.push_str(&format!(" @ {}", name_of(&ITEM_NAMES, pokemon.item as usize)));
        }
        let mut lines = vec![header];
        let ability = if more.ability != 0 { more.ability } else { ability_of(pokemon.species as usize, pokemon.ability.max(1)) };
        if ability != 0 {
            lines.push(format!("Ability: {}", name_of(&ABILITY_NAMES, ability as usize)));
        }
        if pokemon.level != 100 {
            lines.push(format!("Level: {}", pokemon.level));
        }
        if more.shiny {
            lines.push("Shiny: Yes".to_string());
        }
        let evs: Vec<String> = more.evs.iter().enumerate().filter(|(_, &value)| value > 0).map(|(stat, value)| format!("{value} {}", STATS[stat])).collect();
        if !evs.is_empty() {
            lines.push(format!("EVs: {}", evs.join(" / ")));
        }
        if let Some(nature) = NATURES.get(more.nature as usize) {
            lines.push(format!("{nature} Nature"));
        }
        let ivs: Vec<String> = more.ivs.iter().enumerate().filter(|(_, &value)| value < 31).map(|(stat, value)| format!("{value} {}", STATS[stat])).collect();
        if !ivs.is_empty() {
            lines.push(format!("IVs: {}", ivs.join(" / ")));
        }
        if team.moves {
            for &mv in pokemon.moves.iter().filter(|&&mv| mv != 0) {
                lines.push(format!("- {}", name_of(&MOVE_NAMES, mv as usize)));
            }
        }
        blocks.push(lines.join("\n"));
    }
    blocks.join("\n\n") + "\n"
}

/// A Pokémon read from the text.
pub struct Imported {
    pub pokemon: TrainerPokemon,
    pub more: PokemonExtras,
}

/// Reads a team from the text. Gives the Pokémon (at most six) and the problems: a name that the game does not know.
pub fn import(text: &str) -> (Vec<Imported>, Vec<String>) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("===") {
            if !block.is_empty() {
                blocks.push(std::mem::take(&mut block));
            }
        } else {
            block.push(line);
        }
    }
    if !block.is_empty() {
        blocks.push(block);
    }
    for block in blocks {
        if found.len() >= MAX_TEAM {
            problems.push(format!("Only {MAX_TEAM} Pokémon fit in a team: the others are ignored."));
            break;
        }
        if let Some(one) = import_block(&block, &mut problems) {
            found.push(one);
        }
    }
    (found, problems)
}

/// The species of a name of the format: "Charizard-Mega-X" falls back to "Charizard" (the form is not kept).
fn species_of(name: &str) -> Option<usize> {
    let mut name = name.trim();
    loop {
        let at = find(&SPECIES, name);
        if at != 0 {
            return Some(at);
        }
        name = &name[..name.rfind('-')?];
    }
}

fn import_block(lines: &[&str], problems: &mut Vec<String>) -> Option<Imported> {
    let mut pokemon = TrainerPokemon::new(1, 100);
    let mut more = PokemonExtras::none();
    let (names, item) = match lines[0].split_once(" @ ") {
        Some((names, item)) => (names.trim(), Some(item.trim())),
        None => (lines[0].trim(), None),
    };
    let mut names = names.to_string();
    for (suffix, gender) in [(" (M)", 1), (" (F)", 2)] {
        if let Some(rest) = names.strip_suffix(suffix) {
            pokemon.gender = gender;
            names = rest.to_string();
        }
    }
    let species = if names.ends_with(')') && names.contains('(') {
        let open = names.rfind('(').unwrap_or(0);
        let inside = names[open + 1..names.len() - 1].to_string();
        let nickname = names[..open].trim().to_string();
        more.nickname = nickname.chars().take(NICKNAME_LETTERS).collect();
        inside
    } else {
        names.clone()
    };
    let Some(number) = species_of(&species) else {
        problems.push(format!("The species \"{species}\" is not in the game."));
        return None;
    };
    pokemon.species = number as u16;
    if let Some(item) = item {
        let at = find(&ITEM_NAMES, item);
        if at == 0 {
            problems.push(format!("The item \"{item}\" is not in the game."));
        }
        pokemon.item = at as u16;
    }
    let mut moves = 0;
    for line in &lines[1..] {
        if let Some(rest) = line.strip_prefix('-') {
            let wanted = rest.trim();
            if moves >= 4 {
                continue;
            }
            // "Hidden Power Fire" is the move Hidden Power: the type comes from the IVs.
            let mut at = find(&MOVE_NAMES, wanted);
            if at == 0 && simple(wanted).starts_with("hiddenpower") {
                at = find(&MOVE_NAMES, "Hidden Power");
            }
            if at == 0 {
                problems.push(format!("The move \"{wanted}\" is not in the game."));
            } else {
                pokemon.moves[moves] = at as u16;
                moves += 1;
            }
        } else if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "ability" => {
                    let at = find(&ABILITY_NAMES, value);
                    if at == 0 {
                        problems.push(format!("The ability \"{value}\" is not in the game."));
                    }
                    more.ability = at as u16;
                }
                "level" => pokemon.level = value.parse::<u16>().unwrap_or(100).clamp(1, 100),
                "shiny" => more.shiny = value.eq_ignore_ascii_case("yes"),
                "evs" => read_stats(value, &mut more.evs, 255),
                "ivs" => {
                    let mut ivs = [31u8; 6];
                    read_stats(value, &mut ivs, 31);
                    more.ivs = ivs;
                }
                _ => {}
            }
        } else if let Some(nature) = line.strip_suffix(" Nature") {
            match NATURES.iter().position(|name| name.eq_ignore_ascii_case(nature.trim())) {
                Some(at) => more.nature = at as u8,
                None => problems.push(format!("The nature \"{nature}\" is not in the game.")),
            }
        }
    }
    Some(Imported { pokemon, more })
}

/// Reads "252 HP / 4 Spe" into the values of the stats; the stats that the text does not name keep their value.
fn read_stats(text: &str, values: &mut [u8; 6], max: u32) {
    for part in text.split('/') {
        let mut words = part.split_whitespace();
        let (Some(number), Some(stat)) = (words.next(), words.next()) else { continue };
        let Ok(number) = number.parse::<u32>() else { continue };
        if let Some(at) = STATS.iter().position(|name| name.eq_ignore_ascii_case(stat)) {
            values[at] = number.min(max) as u8;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const AMOONGUSS: &str = "Amoonguss (F) @ Choice Specs\nAbility: Effect Spore\nLevel: 87\nHappiness: 253\nEVs: 252 HP / 252 SpA\nNaughty Nature\n- Clear Smog\n- Body Slam\n- Facade\n- Energy Ball\n";

    #[test]
    fn a_set_of_the_format_is_read() {
        let (list, problems) = import(AMOONGUSS);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(list.len(), 1);
        let one = &list[0];
        assert_eq!((one.pokemon.species, one.pokemon.gender, one.pokemon.level), (591, 2, 87));
        assert_eq!(one.pokemon.item, 297, "Choice Specs");
        assert_eq!(one.more.evs, [252, 0, 0, 252, 0, 0]);
        assert_eq!(one.more.nature, 4, "Naughty");
        assert_eq!(name_of(&ABILITY_NAMES, one.more.ability as usize), "Effect Spore");
        assert_eq!(one.pokemon.moves.iter().filter(|&&mv| mv != 0).count(), 4);
    }

    #[test]
    fn a_team_goes_out_and_comes_back() {
        let (list, _) = import(&format!("{AMOONGUSS}\nPika (Pikachu) @ Light Ball\nAbility: Static\nShiny: Yes\nIVs: 0 Atk\n- Volt Tackle\n"));
        assert_eq!(list.len(), 2);
        let mut team = Team { pokemon: list.iter().map(|one| one.pokemon).collect(), moves: true, items: true };
        let extras = BattleExtras { pokemon: list.iter().map(|one| PokemonExtras { ..one.more.clone() }).collect(), ..BattleExtras::none(2) };
        team.moves = true;
        let text = export(&team, &extras, &|_, _| 0);
        let (again, problems) = import(&text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(again.len(), 2);
        for (a, b) in list.iter().zip(again.iter()) {
            assert_eq!(a.pokemon, b.pokemon);
            assert_eq!(a.more, b.more);
        }
        assert_eq!(again[1].more.nickname, "Pika");
        assert_eq!(again[1].more.ivs, [31, 0, 31, 31, 31, 31]);
    }

    #[test]
    fn names_with_signs_and_forms_are_found() {
        assert_eq!(species_of("Mr. Mime"), Some(122));
        assert_eq!(species_of("Charizard-Mega-X"), Some(6));
        assert_eq!(species_of("Flabébé"), find(&SPECIES, "Flabebe").into());
        assert_eq!(species_of("Nidoran-F"), Some(29));
        assert_eq!(species_of("Nobody"), None);
    }
}
