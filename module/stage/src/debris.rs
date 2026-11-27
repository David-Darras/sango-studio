//! Debris: many small copies of a model (rocks, seeds, petals) that appear in a shape, fly, fall, bounce and
//! disappear. A set makes new copies at an interval for the length of its step; each copy then moves by itself.

use crate::math::{self, V3};
use sango_core::posing::rotation_zyx;

/// The keys of a curve of a copy: a frame (-1 after the last key) and a value.
pub type Keys = [(i32, V3); 5];

pub const NO_KEYS: Keys = [(-1, [0.0; 3]); 5];

/// The most copies of one set.
const MOST: usize = 128;

/// How the copies start to move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Speed {
    None,
    /// Away from a point (the place of the set plus an offset), along the axis or the plane of the mode.
    Away { mode: i32, offset: V3 },
    /// Along a direction, turned by a random amount (in degrees).
    Along { direction: V3, random: i32 },
}

/// A pull toward a point, along the axis or the plane of the mode.
#[derive(Clone, Copy, Debug)]
pub struct Gravity {
    pub mode: i32,
    pub place: V3,
    /// False when the place counts from the set.
    pub absolute: bool,
    pub power: f32,
}

/// A plane that the copies meet.
#[derive(Clone, Copy, Debug)]
pub struct Bounce {
    pub place: f32,
    pub absolute: bool,
    pub plane: i32,
    /// 1 bounces, 2 stops, 3 disappears.
    pub mode: i32,
    pub brake: f32,
}

/// What the step that makes the set says.
#[derive(Clone, Copy, Debug)]
pub struct Setup {
    /// The frames while new copies appear.
    pub length: i32,
    pub most: usize,
    pub interval: i32,
    pub per_time: i32,
    /// The chance of each copy, in percent.
    pub rate: i32,
    /// The frames that a copy lives.
    pub life: i32,
    /// 0 a point, 1 a sphere, 2 a cylinder from its base, 3 a cylinder around its middle, 4 a box.
    pub shape: i32,
    pub axis: i32,
    pub size: V3,
    pub degrees: (i32, i32),
    /// The part of the radius where the copies appear, in percent.
    pub reach: i32,
    pub turns_with_movement: bool,
}

/// One copy.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub position: V3,
    pub rotation: V3,
    pub scale: V3,
    pub turn: V3,
    pub size: V3,
    direction: V3,
    speed: f32,
    keep: f32,
    pub age: i32,
}

impl Piece {
    /// The place, the turns and the size that are drawn.
    pub fn drawn(&self) -> (V3, V3, V3) {
        (self.position, math::add(self.rotation, self.turn), math::mul(self.scale, self.size))
    }
}

/// Random numbers that are the same each time the choreography plays.
#[derive(Clone, Copy, Debug)]
struct Random(u64);

impl Random {
    /// A whole number from 0 to `below` - 1 (0 when `below` is 0 or less).
    fn below(&mut self, below: i64) -> i64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        if below <= 0 {
            0
        } else {
            ((z ^ (z >> 31)) % below as u64) as i64
        }
    }

    /// A number from `low` to `high`, in steps of a hundredth.
    fn between(&mut self, low: f32, high: f32) -> f32 {
        if high == low {
            low
        } else {
            self.below(((high - low) * 100.0) as i64) as f32 * 0.01 + low
        }
    }
}

/// A set of debris.
#[derive(Clone, Debug)]
pub struct Debris {
    pub setup: Setup,
    pub speed: Speed,
    pub start_speed: f32,
    pub keep_speed: f32,
    pub gravity: Option<Gravity>,
    pub bounce: Option<Bounce>,
    pub sizes: (V3, V3),
    /// The first turns of a copy, in degrees.
    pub turns: (V3, V3),
    pub size_keys: Keys,
    /// The turns of the curve, in radians.
    pub turn_keys: Keys,
    pub pieces: Vec<Piece>,
    count: i32,
    random: Random,
}

/// Keeps the parts of a vector that the mode names: an axis (1 to 3) removes itself, a plane (4 to 6) keeps itself.
fn along(mut v: V3, mode: i32) -> V3 {
    match mode {
        1..=3 => v[(mode - 1) as usize] = 0.0,
        4..=6 => {
            let kept = (mode - 4) as usize;
            v = std::array::from_fn(|k| if k == kept { v[k] } else { 0.0 });
        }
        _ => {}
    }
    v
}

/// The value of a curve at an age, or `None` when the curve says nothing there.
fn value(keys: &Keys, age: i32) -> Option<V3> {
    for pair in keys.windows(2) {
        let ((from, a), (to, b)) = (pair[0], pair[1]);
        if from == -1 {
            return None;
        }
        if to == -1 {
            return Some(a);
        }
        if from <= age && to > age {
            let part = (age - from) as f32 / (to - from) as f32;
            return Some(std::array::from_fn(|k| math::lerp(a[k], b[k], part)));
        }
    }
    (keys[4].0 < age).then_some(keys[4].1)
}

impl Debris {
    pub fn new(setup: Setup, seed: u64) -> Debris {
        Debris { setup, speed: Speed::None, start_speed: 0.0, keep_speed: 1.0, gravity: None, bounce: None, sizes: ([1.0; 3], [1.0; 3]), turns: ([0.0; 3], [0.0; 3]), size_keys: NO_KEYS, turn_keys: NO_KEYS, pieces: Vec::new(), count: 0, random: Random(seed) }
    }

    /// True when no copy is left and no new one will appear.
    pub fn is_over(&self) -> bool {
        self.count > self.setup.length && self.pieces.is_empty()
    }

    /// One frame: new copies appear, then every copy moves. The place, the turns, the size and the factors are the ones
    /// of the set.
    pub fn advance(&mut self, place: V3, turns: V3, size: V3, factors: V3) {
        let setup = self.setup;
        if self.count < setup.length && self.count % setup.interval.max(1) == 0 {
            for _ in 0..setup.per_time.clamp(0, MOST as i32) {
                if (setup.rate == 100 || i64::from(setup.rate) > self.random.below(100)) && self.pieces.len() < setup.most.min(MOST) {
                    let piece = self.spawn(place, turns, size, factors);
                    self.pieces.push(piece);
                }
            }
        }
        self.count = self.count.saturating_add(1);
        let (gravity, bounce, life) = (self.gravity, self.bounce, setup.life);
        let (size_keys, turn_keys) = (self.size_keys, self.turn_keys);
        for piece in &mut self.pieces {
            let before = piece.position;
            let movement = math::scaled(piece.direction, piece.speed);
            piece.position = math::add(piece.position, movement);
            if let Some(gravity) = gravity {
                let toward = if gravity.absolute { gravity.place } else { math::add(gravity.place, place) };
                let pull = math::scaled(math::normalized(along(math::sub(toward, piece.position), gravity.mode)), gravity.power);
                let now = math::add(movement, pull);
                piece.speed = math::length(now);
                piece.direction = math::normalized(now);
            }
            if piece.speed != 0.0 {
                piece.speed *= piece.keep;
            }
            if let Some(bounce) = bounce {
                // The axis that the plane stops: x, y or z.
                let axis = match bounce.plane {
                    4 => 0,
                    6 => 2,
                    _ => 1,
                };
                let level = bounce.place + if bounce.absolute { 0.0 } else { place[axis] };
                let (was, now) = (before[axis], piece.position[axis]);
                if (was < level && now >= level) || (was > level && now <= level) {
                    piece.position[axis] = 2.0 * level - now;
                    piece.direction[axis] = -piece.direction[axis];
                    match bounce.mode {
                        1 => piece.speed -= piece.speed * bounce.brake,
                        2 => piece.speed = 0.0,
                        3 => piece.age = life + 1,
                        _ => {}
                    }
                }
            }
            if let Some(scale) = value(&size_keys, piece.age) {
                piece.scale = scale;
            }
            if let Some(rotation) = value(&turn_keys, piece.age) {
                piece.rotation = rotation;
            }
            if setup.turns_with_movement {
                let way = math::sub(piece.position, before);
                piece.rotation = [way[0].hypot(way[2]).atan2(way[1]), way[0].atan2(way[2]), 0.0];
            }
            piece.age += 1;
        }
        self.pieces.retain(|piece| piece.age <= life);
    }

    fn spawn(&mut self, place: V3, turns: V3, size: V3, factors: V3) -> Piece {
        let setup = self.setup;
        let random = &mut self.random;
        let size = math::mul(size, setup.size);
        let reach = (100 - random.below(i64::from(101 - setup.reach))) as f32 * 0.01;
        let (from, to) = setup.degrees;
        let axis = (setup.axis - 1).clamp(0, 2) as usize;
        // A direction around the axis: it starts along the next axis, then turns.
        let around = |degrees: f32| -> V3 {
            let (sin, cos) = degrees.to_radians().sin_cos();
            let mut v = [0.0; 3];
            // x turns in (z, y), y in (x, z), z in (y, x).
            let (a, b) = [(2, 1), (0, 2), (1, 0)][axis];
            v[a] = cos;
            v[b] = sin;
            v
        };
        let part = |random: &mut Random, length: f32| random.below((length * 100.0) as i64) as f32 * 0.01;
        let mut offset = match setup.shape {
            1 => {
                let first = (random.below(i64::from(to - from)) + i64::from(from)) as f32;
                let second = (random.below(180) - 90) as f32;
                let mut v = around(first);
                // The second turn goes toward the axis.
                let (a, b) = [(0, 2), (1, 0), (2, 1)][axis];
                let (sin, cos) = second.to_radians().sin_cos();
                let (x, z) = (v[a], v[b]);
                (v[a], v[b]) = (x * cos - z * sin, x * sin + z * cos);
                math::mul(math::scaled(v, reach), size)
            }
            2 | 3 => {
                let degrees = (random.below(i64::from(to - from)) + i64::from(from)) as f32;
                let mut v = math::mul(math::scaled(around(degrees), reach), size);
                let height = part(random, size[axis]);
                v[axis] = if setup.shape == 2 { height } else { height - size[axis] / 2.0 };
                v
            }
            4 => {
                let chosen = random.below((size[0] + size[1] + size[2]) as i64) as f32;
                let face = if chosen < size[0] { 0 } else if chosen < size[0] + size[1] { 1 } else { 2 };
                let mut v: V3 = std::array::from_fn(|k| if k == face { size[k] / 2.0 * reach } else { part(random, size[k] / 2.0) });
                for value in &mut v {
                    if random.below(2) == 0 {
                        *value = -*value;
                    }
                }
                v
            }
            _ => [0.0; 3],
        };
        offset = sango_core::posing::Affine::new([0.0; 3], rotation_zyx(turns), [1.0; 3]).point(offset);
        let position = math::add(place, offset);
        let (low, high) = self.sizes;
        let first_size: V3 = std::array::from_fn(|k| random.between(low[k], high[k]));
        let (low, high) = self.turns;
        let first_turn: V3 = std::array::from_fn(|k| random.between(low[k], high[k]).to_radians());
        // The size of a copy follows the factors of the set by half, as an object placed on a Pokémon.
        let size_of_piece = math::mul(first_size, factors).map(|s| (s - 1.0) * 0.5 + 1.0);
        let direction = match self.speed {
            Speed::None => [0.0; 3],
            Speed::Away { mode, offset } => math::normalized(along(math::sub(position, math::add(place, offset)), mode)),
            Speed::Along { direction, random: spread } => {
                let direction = math::normalized(direction);
                if spread == 0 {
                    direction
                } else {
                    let angles: V3 = std::array::from_fn(|_| {
                        let turn = random.below(i64::from(spread) * 20 + 1) - i64::from(spread) * 10;
                        (turn as f32 * 0.1).to_radians().sin()
                    });
                    sango_core::posing::Affine::new([0.0; 3], rotation_zyx(angles), [1.0; 3]).point(direction)
                }
            }
        };
        let moving = self.speed != Speed::None;
        Piece { position, rotation: [0.0; 3], scale: [1.0; 3], turn: math::add(first_turn, turns), size: size_of_piece, direction, speed: if moving { self.start_speed } else { 0.0 }, keep: if moving { self.keep_speed } else { 1.0 }, age: 0 }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn setup() -> Setup {
        Setup { length: 4, most: 10, interval: 2, per_time: 3, rate: 100, life: 5, shape: 0, axis: 2, size: [1.0; 3], degrees: (0, 360), reach: 100, turns_with_movement: false }
    }

    #[test]
    fn copies_appear_at_the_interval_and_leave_after_their_life() {
        let mut debris = Debris::new(setup(), 1);
        (debris.speed, debris.start_speed) = (Speed::Along { direction: [0.0, 1.0, 0.0], random: 0 }, 2.0);
        let advance = |debris: &mut Debris| debris.advance([0.0; 3], [0.0; 3], [1.0; 3], [1.0; 3]);
        advance(&mut debris);
        assert_eq!(debris.pieces.len(), 3);
        assert_eq!(debris.pieces[0].position, [0.0, 2.0, 0.0]);
        advance(&mut debris);
        advance(&mut debris);
        assert_eq!(debris.pieces.len(), 6);
        for _ in 0..10 {
            advance(&mut debris);
        }
        assert!(debris.pieces.is_empty() && debris.is_over());
    }

    #[test]
    fn the_same_seed_gives_the_same_copies() {
        let make = || {
            let mut debris = Debris::new(Setup { shape: 1, size: [50.0; 3], reach: 50, ..setup() }, 7);
            debris.advance([1.0; 3], [0.0; 3], [1.0; 3], [1.0; 3]);
            debris.pieces.iter().map(|p| p.position).collect::<Vec<_>>()
        };
        assert_eq!(make(), make());
        assert!(make().iter().all(|p| math::length(math::sub(*p, [1.0; 3])) <= 50.01));
    }
}
