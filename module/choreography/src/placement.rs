//! Where the choreographies put the effects that stay in front of the camera (the screen effects of the introductions of
//! the battles, for example): the distance from the camera, then the size, the move and the turn of the effect.

use crate::move_choreography::{kind_of, MoveChoreography, Step, Value};
use crate::{schema, ARCHIVE};
use sango_core::{Dump, Garc};
use std::collections::HashMap;

/// A place of an effect in front of the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenPlacement {
    /// The number of the choreography that uses it, and its name.
    pub choreography: usize,
    pub name: &'static str,
    /// The distance from the camera.
    pub distance: f32,
    pub scale: [f32; 3],
    pub shift: [f32; 3],
    /// The turn around x, y and z, in degrees.
    pub turn: [f32; 3],
}

/// The value of the first parameter of a step.
fn first_value(step: &Step) -> Option<Value> {
    kind_of(step.kind)?.params.first().map(|param| step.value(param))
}

/// The places in front of the camera that a choreography gives to its effects: the number of the effect and the place.
fn placements_of(file: usize, choreography: &MoveChoreography) -> Vec<(i32, ScreenPlacement)> {
    let named = |step: &Step, name: &str| kind_of(step.kind).is_some_and(|kind| kind.name == name);
    let mut found = Vec::new();
    for (at, spawn) in choreography.steps.iter().enumerate().filter(|(_, step)| named(step, "Effect Spawn")) {
        let Some(Value::Int(effect)) = first_value(spawn) else { continue };
        let mut place = ScreenPlacement { choreography: file, name: schema::FILE_LABELS.get(file).copied().unwrap_or("Choreography"), distance: 0.0, scale: [1.0; 3], shift: [0.0; 3], turn: [0.0; 3] };
        let (mut kept, mut seen) = (false, Vec::new());
        // The steps of the same object, until it is made again. The first step of each kind gives the place at the start.
        for step in choreography.steps[at + 1..].iter().filter(|step| step.group == spawn.group) {
            if named(step, "Effect Spawn") {
                break;
            }
            if seen.contains(&step.kind) {
                continue;
            }
            seen.push(step.kind);
            match (first_value(step), kind_of(step.kind).map(|kind| kind.name)) {
                (Some(Value::Float(distance)), Some("Effect Lock Display")) => (place.distance, kept) = (distance, true),
                (Some(Value::Vec3(value)), Some("Effect Resize")) => place.scale = value,
                (Some(Value::Vec3(value)), Some("Effect Shift")) => place.shift = value,
                (Some(Value::Vec3(value)), Some("Effect Turn")) => place.turn = value,
                _ => {}
            }
        }
        if kept {
            found.push((effect, place));
        }
    }
    found
}

/// For each effect that a choreography keeps in front of the camera, its places there, each once, in the order of the
/// choreographies.
pub fn screen_placements(dump: &Dump) -> HashMap<i32, Vec<ScreenPlacement>> {
    let mut all: HashMap<i32, Vec<ScreenPlacement>> = HashMap::new();
    let Ok(garc) = Garc::open(&dump.archive(ARCHIVE)) else { return all };
    for file in 0..garc.file_count() {
        let Some(Ok(choreography)) = garc.first_file_decompressed(file).map(|bytes| MoveChoreography::parse(&bytes)) else { continue };
        for (effect, place) in placements_of(file, &choreography) {
            let places = all.entry(effect).or_default();
            if !places.iter().any(|p| (p.distance, p.scale, p.shift, p.turn) == (place.distance, place.scale, place.shift, place.turn)) {
                places.push(place);
            }
        }
    }
    all
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the effect 464 of an introduction of a battle stays far in front of the camera, made
    /// larger and turned to it.
    #[test]
    fn the_places_of_the_screen_effects_are_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let all = screen_placements(&Dump::open(&root).unwrap());
        let place = &all[&464][0];
        assert_eq!((place.choreography, place.distance, place.scale, place.shift, place.turn), (801, 600.0, [10.0; 3], [0.0, 80.0, 0.0], [0.0, 180.0, 0.0]));
        assert!(all.len() > 20, "{} effects", all.len());
    }
}
