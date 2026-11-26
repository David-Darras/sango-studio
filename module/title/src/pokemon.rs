//! The Pokémon of the title screen. The bottom sequence shows them by groups of steps: one group for the Pokémon of the close views
//! with its own motions, and one for the Pokémon that fills the screen.

use crate::commands::{POKEMON_MOTION, POKEMON_MOTION_FILES, POKEMON_PLACE, POKEMON_SHOW, POKEMON_SHOW_VIDEO, POKEMON_SIZE};
use crate::sequence::{Sequence, Step};

/// The number of the set of motions of the battles, which every Pokémon has.
const BATTLE_MOTIONS: i32 = 0;

fn shows(step: &Step) -> bool {
    step.kind == POKEMON_SHOW || step.kind == POKEMON_SHOW_VIDEO
}

/// The groups of the sequence that show a Pokémon, in the order of their first step.
pub fn groups(sequence: &Sequence) -> Vec<u16> {
    let mut found = Vec::new();
    for step in sequence.steps.iter().filter(|step| shows(step)) {
        if !found.contains(&step.group) {
            found.push(step.group);
        }
    }
    found
}

/// The species, the form and the gender that a group shows.
pub fn pokemon(sequence: &Sequence, group: u16) -> Option<(usize, usize, i32)> {
    sequence.steps.iter().find(|step| step.group == group && shows(step)).map(|step| (step.int(0).max(0) as usize, step.int(1).max(0) as usize, step.int(2)))
}

/// Gives a group another Pokémon. The motions that the game made for its own Pokémon fit only this one: another Pokémon takes the
/// motions of the battles, and the Pokémon of the game takes its motions back (from `game`).
pub fn set_pokemon(sequence: &mut Sequence, game: &Sequence, group: u16, species: usize, form: usize, gender: i32) {
    let original = pokemon(game, group).map(|(s, f, _)| (s, f));
    let own = original == Some((species, form));
    let of_group = |step: &Step| step.group == group && matches!(step.kind, POKEMON_SHOW | POKEMON_SHOW_VIDEO | POKEMON_MOTION | POKEMON_MOTION_FILES);
    if own {
        // The steps of the Pokémon and of its motions of the game, at the frames of the game.
        sequence.steps.retain(|step| !of_group(step));
        for step in game.steps.iter().filter(|step| of_group(step)) {
            sequence.insert(step.clone());
        }
    }
    for step in sequence.steps.iter_mut().filter(|step| step.group == group) {
        match step.kind {
            POKEMON_SHOW | POKEMON_SHOW_VIDEO => {
                step.set_int(0, species as i32);
                step.set_int(1, form as i32);
                step.set_int(2, gender);
                if step.kind == POKEMON_SHOW_VIDEO && !own {
                    step.set_int(3, BATTLE_MOTIONS);
                    step.set_int(4, -1);
                }
            }
            POKEMON_MOTION_FILES if !own => {
                let mut motion = Step::new(POKEMON_MOTION, step.start, step.end, group).expect("a type of step of the table");
                motion.condition = step.condition;
                // Waiting, again and again.
                motion.set_int(1, 1);
                *step = motion;
            }
            _ => {}
        }
    }
}

/// The values of the first step of a type of a group, as three decimal numbers.
fn vector(sequence: &Sequence, group: u16, kind: u16) -> Option<[f32; 3]> {
    sequence.steps.iter().find(|step| step.group == group && step.kind == kind).map(|step| [step.float(0), step.float(1), step.float(2)])
}

/// Gives each time of a group the same values with a step of a type. The normal values remove the steps, when the game has none.
fn set_vector(sequence: &mut Sequence, game: &Sequence, group: u16, kind: u16, values: [f32; 3], normal: [f32; 3]) {
    sequence.steps.retain(|step| !(step.group == group && step.kind == kind));
    if values == normal && vector(game, group, kind).is_none() {
        return;
    }
    let shown: Vec<(i32, u16)> = sequence.steps.iter().filter(|step| step.group == group && shows(step)).map(|step| (step.start, step.condition)).collect();
    for (start, condition) in shown {
        let mut step = Step::new(kind, start, start, group).expect("a type of step of the table");
        step.condition = condition;
        for (place, value) in values.into_iter().enumerate() {
            step.set_float(place, value);
        }
        sequence.insert(step);
    }
}

/// The size of the Pokémon of a group (1: as the game makes it).
pub fn size(sequence: &Sequence, group: u16) -> f32 {
    vector(sequence, group, POKEMON_SIZE).map_or(1.0, |v| v[0])
}

pub fn set_size(sequence: &mut Sequence, game: &Sequence, group: u16, size: f32) {
    set_vector(sequence, game, group, POKEMON_SIZE, [size; 3], [1.0; 3]);
}

/// The place of the Pokémon of a group.
pub fn place(sequence: &Sequence, group: u16) -> [f32; 3] {
    vector(sequence, group, POKEMON_PLACE).unwrap_or_default()
}

pub fn set_place(sequence: &mut Sequence, game: &Sequence, group: u16, place: [f32; 3]) {
    set_vector(sequence, game, group, POKEMON_PLACE, place, [0.0; 3]);
}
