//! A preview of an effect: the particles of all the layers at a frame, drawn on the processor.
//!
//! The particles follow the rules of the particle system of the game, which the effect files hold:
//!
//! - The emitter makes particles from its start frame, with the number, the interval and the randoms of the layer.
//! - A new particle gets its place from the shape of the emitter, then each initializer gives a first value to a stream (the speed, the size,
//!   the turn, the colour, the transparency, the life), in the order of the file. The scale, the turn and the position of the emitter
//!   then move the place and the speed.
//! - At each frame, the first one too, each updater changes a stream, in the order of the file: the air resistance, the gravity, the turn of
//!   a speed, the move by the speed, the collision with the ground, the child particles, the keys of the colour, of the size and of the
//!   transparency.
//! - A particle is a rectangle that faces the screen, stays upright or lies in the space of the emitter. The steps of its material mix
//!   its picture with its colour, and the blend of the material puts the result over what is behind, set after set.
//!
//! A particle is computed from its birth to the frame by going through the frames one after the other, so the user can go to any frame
//! and the same frame always gives the same particles. The random numbers are not the ones of the game. The preview does not draw the
//! models of the places, so nothing hides a particle.

use crate::effect_file::{Animation, AnimationKind, Builder, ChildTiming, Effect, FormKind, InitRule, Initializer, UpdRule, Updater};
use sango_core::model::{RenderState, Stage};

/// The most particles that the preview draws at one frame.
const MAX_PARTICLES: usize = 3000;
/// The frames that an emitter without end is shown.
const ENDLESS_FRAMES: f32 = 120.0;
/// The most sets in a chain of child particles.
const MAX_CHAIN: usize = 3;

/// A particle at a frame.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    /// The set of particles (see `Effect::group_count`).
    pub group: usize,
    /// The frame of its birth.
    pub born: u32,
    pub position: [f32; 3],
    /// The width and the height of the picture.
    pub size: [f32; 2],
    pub color: [f32; 3],
    pub alpha: f32,
    /// The turn around x, y and z (radians). A picture that faces the screen uses the one around z.
    pub rotate: [f32; 3],
    /// The coordinates of the picture on the particle: the translation, the scale and the turn.
    pub uv_translate: [f32; 2],
    pub uv_scale: [f32; 2],
    pub uv_turn: f32,
}

/// A picture of the particles, as RGBA pixels.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// How the particles of a set look: their picture, the steps and the blend of their material, and the way that they are
/// drawn.
#[derive(Clone, Debug)]
pub struct Look {
    pub picture: Option<Picture>,
    pub stages: Vec<Stage>,
    pub buffer_color: [u8; 4],
    pub state: RenderState,
    /// The matrix of the coordinates of the picture (see `Material::matrix`).
    pub matrix: [[f32; 3]; 2],
    pub wrap: [u32; 2],
    pub builder: Builder,
    /// The place of the particle in its picture, in halves of its sides.
    pub offset: [f32; 3],
    /// The order of the set when the sets are drawn, and the order of its particles.
    pub order: (u32, u32),
    pub oldest_first: bool,
}

/// The point of view: a turn around the vertical axis, a turn up and down, and a zoom (1 shows the whole effect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        // Seen from the side a little, so that a movement in the depth shows.
        Camera { yaw: 0.6, pitch: 0.25, zoom: 1.0 }
    }
}

/// A sequence of random numbers (the same seed gives the same numbers).
struct Rng(u64);

impl Rng {
    fn new(seed: [u64; 4]) -> Rng {
        Rng(seed.iter().fold(0x9E37_79B9_7F4A_7C15u64, |acc, &v| (acc ^ v.wrapping_add(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9).rotate_left(27)))
    }

    /// A number from 0 to 1 (1 is not reached).
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number from -1 to 1.
    fn signed_one(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }

    /// A number from -0.5 to 0.5.
    fn signed_half(&mut self) -> f32 {
        self.next() - 0.5
    }
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length < 1e-8 {
        [0.0; 3]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}

/// The numbers of the shape of an emitter.
struct Shape {
    kind: Option<FormKind>,
    inner: f32,
    arc_width: f32,
    arc_start: f32,
    arc_swing: f32,
    fixed_start: bool,
    even: bool,
    scale: [f32; 3],
}

impl Shape {
    fn of(effect: &Effect, layer: usize) -> Shape {
        Shape::from_values(effect.form_kind(layer), effect.form_values(layer))
    }

    fn from_values(kind: Option<FormKind>, fields: Vec<(crate::effect_file::FormField, [f32; 3])>) -> Shape {
        let value = |name: &str| fields.iter().find(|(field, _)| field.name == name).map_or([0.0; 3], |(_, v)| *v);
        let mut scale = value("Size");
        // A flat shape has two numbers: the width and the depth.
        if matches!(kind, Some(FormKind::Disc) | Some(FormKind::Rectangle)) {
            scale = [scale[0], 0.0, scale[1]];
        }
        // A shape without a side keeps a very small one: a speed away from the centre still has a direction.
        let scale = scale.map(|v| if v.abs() < 1e-5 { 1e-5 } else { v });
        Shape { kind, inner: value("Empty part")[0], arc_width: value("Arc")[0], arc_start: value("Arc start")[0], arc_swing: value("Arc swing")[0], fixed_start: value("Fixed start")[0] != 0.0, even: value("Even spread")[0] != 0.0, scale }
    }

    /// The turn of the arc for the particles that appear together.
    fn start_angle(&self, rng: &mut Rng) -> f32 {
        if self.fixed_start { self.arc_start } else { rng.next() * std::f32::consts::TAU }
    }

    /// The place of the particle `n` of `count` that appear together in the shape (the size of a shape is its radius, or
    /// half of its width). `start` comes from `start_angle`.
    fn position(&self, rng: &mut Rng, start: f32, n: u32, count: u32) -> [f32; 3] {
        let tau = std::f32::consts::TAU;
        let s = self.scale;
        // A number from -1 to 1, pushed away from the middle: the part `empty` in the middle stays free.
        let pushed = |value: f32, empty: f32| if value >= 0.0 { 1.0 - value * (1.0 - empty) } else { -1.0 - value * (1.0 - empty) };
        match self.kind {
            None => [0.0; 3],
            Some(FormKind::Point) => [rng.signed_half() * 1e-3, rng.signed_half() * 1e-3, rng.signed_half() * 1e-3],
            Some(FormKind::Cube) => {
                let mut at = [rng.signed_one(), rng.signed_one(), rng.signed_one()];
                if self.inner >= 1.0 {
                    // On the faces: the larger a pair of faces is, the more particles it gets.
                    let areas = [s[0] * s[1], s[1] * s[2], s[2] * s[0]].map(f32::abs);
                    let pick = rng.next() * areas.iter().sum::<f32>();
                    let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
                    let axis = if pick < areas[0] { 2 } else if pick < areas[0] + areas[1] { 0 } else { 1 };
                    at[axis] = side;
                } else if self.inner > 0.0 {
                    // Between the faces and the empty box in the middle: one axis is pushed out.
                    let weights = [1.0 - self.inner, (1.0 - self.inner) * self.inner, (1.0 - self.inner) * self.inner * self.inner];
                    let pick = rng.next() * weights.iter().sum::<f32>();
                    if pick < weights[0] {
                        at[1] = pushed(at[1], self.inner);
                    } else if pick < weights[0] + weights[1] {
                        at[1] *= self.inner;
                        at[2] = pushed(at[2], self.inner);
                    } else {
                        at[0] = pushed(at[0], self.inner);
                    }
                }
                [at[0] * s[0], at[1] * s[1], at[2] * s[2]]
            }
            Some(FormKind::Rectangle) => {
                let (mut x, mut z) = (rng.signed_one(), rng.signed_one());
                if self.inner >= 1.0 {
                    // On the border: a place along the way around it.
                    let (w, d) = (s[0].abs(), s[2].abs());
                    let along = rng.signed_one() * 2.0 * (w + d);
                    let (x, z) = if along >= 0.0 {
                        if along < w * 2.0 { (along - w, d) } else { (w, along - w * 2.0 - d) }
                    } else if along > -w * 2.0 {
                        (along + w, -d)
                    } else {
                        (-w, along + w * 2.0 + d)
                    };
                    return [x, 0.0, z];
                }
                if self.inner > 0.0 {
                    if rng.next() * (1.0 + self.inner) < 1.0 {
                        z = pushed(z, self.inner);
                    } else {
                        z *= self.inner;
                        x = pushed(x, self.inner);
                    }
                }
                [x * s[0], 0.0, z * s[2]]
            }
            Some(FormKind::Disc) | Some(FormKind::Cylinder) => {
                let d = rng.next();
                let distance = if self.inner == 0.0 {
                    d.sqrt()
                } else if self.inner >= 1.0 {
                    1.0
                } else {
                    (d + self.inner * self.inner * (1.0 - d)).sqrt()
                };
                let disc = self.kind == Some(FormKind::Disc);
                let mut angle = if disc && self.even && count > 0 {
                    // The particles that appear together are at equal angles: a whole circle has no particle twice.
                    let rest = self.arc_width.rem_euclid(tau);
                    let whole = !(1e-3..=tau - 1e-3).contains(&rest) || count == 1;
                    self.arc_width / (if whole { count } else { count - 1 }) as f32 * n as f32 - self.arc_width / 2.0
                } else if disc {
                    rng.signed_half() * self.arc_width
                } else {
                    rng.next() * self.arc_width
                };
                if self.arc_swing != 0.0 {
                    angle += rng.signed_half() * self.arc_swing;
                }
                angle += start;
                let height = if disc { 0.0 } else { rng.signed_one() * s[1] };
                [angle.sin() * distance * s[0], height, -angle.cos() * distance * s[2]]
            }
            Some(FormKind::Sphere) => {
                let d = rng.next();
                let distance = if self.inner == 0.0 {
                    1.0 - d * d * d
                } else if self.inner >= 1.0 {
                    1.0
                } else {
                    let far = 1.0 - d * d * d;
                    far + self.inner * (1.0 - far)
                };
                let mut yaw = rng.next() * self.arc_width + start;
                let mut pitch = std::f32::consts::PI * rng.signed_half();
                if self.arc_swing != 0.0 {
                    yaw += rng.signed_half() * self.arc_swing;
                    pitch += rng.signed_half() * self.arc_swing;
                }
                [yaw.sin() * -pitch.cos() * distance * s[0], -pitch.sin() * distance * s[1], yaw.cos() * pitch.cos() * distance * s[2]]
            }
        }
    }
}

/// The turn of a vector around x, then y, then z.
fn turned(v: [f32; 3], angles: [f32; 3]) -> [f32; 3] {
    let v = rotate_around(v, [1.0, 0.0, 0.0], angles[0]);
    let v = rotate_around(v, [0.0, 1.0, 0.0], angles[1]);
    rotate_around(v, [0.0, 0.0, 1.0], angles[2])
}

/// The streams of a particle.
struct Streams {
    translate: [f32; 3],
    velocity: [f32; 3],
    scale: [f32; 3],
    /// The second size: the game multiplies it with the first one.
    scale_ext: [f32; 3],
    rotate: [f32; 3],
    color: [f32; 3],
    alpha: f32,
    life: f32,
    uv_translate: [f32; 2],
    uv_scale: [f32; 2],
    uv_turn: f32,
}

/// What a set of particles gives to each of its particles.
struct Group {
    parameters: Vec<(i32, Vec<f32>)>,
    initializers: Vec<InitRule>,
    updaters: Vec<UpdRule>,
    inherits_speed: bool,
    /// True when its particles make other particles.
    has_children: bool,
}

impl Group {
    fn all(effect: &Effect) -> Vec<Group> {
        (0..effect.group_count())
            .map(|group| {
                let updaters = effect.group_updaters(group);
                let has_children = updaters.iter().any(|r| matches!(r.rule, Updater::Child { .. }));
                Group { parameters: effect.group_parameters(group), initializers: effect.group_initializers(group), updaters, inherits_speed: effect.group_inherits_speed(group), has_children }
            })
            .collect()
    }
}

impl Streams {
    /// The stream of 3 numbers that a target names.
    fn vector(&mut self, target: i32) -> Option<&mut [f32; 3]> {
        match target {
            0 => Some(&mut self.translate),
            1 => Some(&mut self.scale),
            2 => Some(&mut self.rotate),
            8 => Some(&mut self.scale_ext),
            3 => Some(&mut self.color),
            12 => Some(&mut self.velocity),
            _ => None,
        }
    }
}

fn add(a: &mut [f32; 3], b: [f32; 3], factor: f32) {
    for k in 0..3 {
        a[k] += b[k] * factor;
    }
}

/// Turns a vector around an axis (a unit vector) by an angle.
fn rotate_around(v: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    let dot = v[0] * axis[0] + v[1] * axis[1] + v[2] * axis[2];
    let cross = [axis[1] * v[2] - axis[2] * v[1], axis[2] * v[0] - axis[0] * v[2], axis[0] * v[1] - axis[1] * v[0]];
    [0, 1, 2].map(|k| v[k] * cos + cross[k] * sin + axis[k] * dot * (1.0 - cos))
}

/// A particle at the start: the place from the shape, then the first values from the initializers.
fn new_particle(group: &Group, place: [f32; 3], rng: &mut Rng) -> Streams {
    let mut p = Streams { translate: place, velocity: [0.0; 3], scale: [1.0; 3], scale_ext: [1.0; 3], rotate: [0.0; 3], color: [1.0; 3], alpha: 1.0, life: 1.0, uv_translate: [0.0; 2], uv_scale: [1.0; 2], uv_turn: 0.0 };
    // The constant values of the set come first: an initializer or an updater of the same stream replaces them.
    for (usage, numbers) in &group.parameters {
        let three = |numbers: &[f32]| [numbers.first().copied().unwrap_or(0.0), numbers.get(1).copied().unwrap_or(0.0), numbers.get(2).copied().unwrap_or(0.0)];
        match *usage {
            1 => p.scale = three(&numbers),
            8 => p.scale_ext = three(&numbers),
            2 => p.rotate = three(&numbers),
            3 => p.color = three(&numbers),
            4 => p.alpha = numbers.first().copied().unwrap_or(1.0),
            5 => p.uv_translate = [numbers.first().copied().unwrap_or(0.0), numbers.get(1).copied().unwrap_or(0.0)],
            6 => p.uv_scale = [numbers.first().copied().unwrap_or(1.0), numbers.get(1).copied().unwrap_or(1.0)],
            7 => p.uv_turn = numbers.first().copied().unwrap_or(0.0),
            11 => p.life = numbers.first().copied().unwrap_or(1.0),
            _ => {}
        }
    }
    for rule in &group.initializers {
        let target = rule.target;
        match rule.rule.clone() {
            Initializer::Directional { power, direction } => add(&mut p.velocity, direction, power),
            Initializer::RandomDirectional { power, direction, angle } => {
                if direction != [0.0; 3] {
                    // A cone around the direction: a ring of random width around it.
                    let d = normalize(direction);
                    let tilt = std::f32::consts::FRAC_PI_2 - angle * rng.signed_one();
                    let turn = std::f32::consts::TAU * rng.signed_one();
                    let (ring, along) = (tilt.cos(), tilt.sin());
                    // Two axes at right angles to the direction.
                    let helper = if d[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                    let a = normalize([helper[1] * d[2] - helper[2] * d[1], helper[2] * d[0] - helper[0] * d[2], helper[0] * d[1] - helper[1] * d[0]]);
                    let b = [d[1] * a[2] - d[2] * a[1], d[2] * a[0] - d[0] * a[2], d[0] * a[1] - d[1] * a[0]];
                    for k in 0..3 {
                        p.velocity[k] += (d[k] * along + a[k] * ring * turn.cos() + b[k] * ring * turn.sin()) * power;
                    }
                }
            }
            Initializer::Origin { power } => add(&mut p.velocity, normalize(p.translate), power),
            Initializer::Random { power } => {
                let yaw = rng.next() * std::f32::consts::TAU;
                let pitch = rng.signed_half() * std::f32::consts::PI;
                add(&mut p.velocity, [yaw.sin() * -pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()], power);
            }
            Initializer::YAxis { power } => add(&mut p.velocity, normalize([p.translate[0], 0.0, p.translate[2]]), power),
            Initializer::SpeedRandom { random } => {
                let factor = 1.0 + rng.signed_one() * random;
                for v in p.velocity.iter_mut() {
                    *v *= factor;
                }
            }
            Initializer::Vector3 { value } => {
                if let Some(stream) = p.vector(target) {
                    *stream = value;
                }
            }
            Initializer::Vector3Random1 { base, random } => {
                let factor = if random != 0.0 { rng.signed_one() * random } else { 0.0 };
                if let Some(stream) = p.vector(target) {
                    *stream = base.map(|v| v + v * factor);
                }
            }
            Initializer::Vector3Random3 { base, random } => {
                let mut value = base;
                for k in 0..3 {
                    if random[k] != 0.0 {
                        value[k] += value[k] * rng.signed_one() * random[k];
                    }
                }
                if let Some(stream) = p.vector(target) {
                    *stream = value;
                }
            }
            Initializer::Float { base, random } => {
                let value = if random != 0.0 { base + base * rng.signed_one() * random } else { base };
                match target {
                    4 => p.alpha = value,
                    7 => p.uv_turn = value,
                    11 => p.life = value,
                    _ => {}
                }
            }
            Initializer::FloatRange { max, min } => {
                let value = min + (rng.next() * ((max - min) + 1.0).floor().max(1.0)).floor();
                if target == 11 {
                    p.life = value;
                }
            }
            Initializer::FloatImmediate { value } => match target {
                4 => p.alpha = value,
                7 => p.uv_turn = value,
                11 => p.life = value,
                _ => {}
            },
            Initializer::Vector2 { value } => match target {
                5 => p.uv_translate = value,
                6 => p.uv_scale = value,
                _ => {}
            },
        }
    }
    p.life = p.life.max(1.0);
    p
}


/// The value of 4 keys at a moment of the life: a rising line, a flat part, a falling line.
fn keys_at<T: Copy + Into<f64>>(in_time: u32, out_time: u32, values: [T; 5], life_part: f32) -> f64 {
    let u = life_part as f64;
    let time = (life_part * 65536.0) as u32;
    if time < in_time {
        values[2].into() * u + values[0].into()
    } else if time < out_time {
        values[1].into()
    } else {
        values[3].into() * u + values[4].into()
    }
}

/// The numbers of an animation after `age` frames of a particle with this life. `seed` tells the random frame of a random animation.
fn animation_values(animation: &Animation, base: &[f32], age: f32, life: f32, seed: [u64; 4]) -> Vec<f32> {
    animation_values_in(animation, base, age, life, seed, true)
}

/// The same, with or without a mix of the two frames that the time is between (a number of a cell is not mixed).
fn animation_values_in(animation: &Animation, base: &[f32], age: f32, life: f32, seed: [u64; 4], smooth: bool) -> Vec<f32> {
    let frame = match animation.kind {
        AnimationKind::Frames => age,
        AnimationKind::Fitting => age / life.max(1.0) * animation.duration,
        AnimationKind::Loop { duration, random_offset } => {
            if duration == 0 || animation.duration == 0.0 {
                0.0
            } else {
                let offset = if random_offset { (Rng::new(seed).next() * duration as f32).floor() } else { 0.0 };
                ((age + offset) % duration as f32) / duration as f32 * animation.duration
            }
        }
        AnimationKind::Random => {
            if animation.duration == 0.0 {
                0.0
            } else {
                (Rng::new(seed).next() * (animation.duration + 1.0)).floor()
            }
        }
    }
    .max(0.0);
    let frame = frame.min(animation.length as f32 - 1.0);
    let (index, fraction) = (frame.floor() as usize, frame.fract());
    let mut out = base.to_vec();
    let mut slot = 0;
    for (axis, value) in out.iter_mut().enumerate() {
        if animation.enabled.get(axis).copied().unwrap_or(false) {
            let here = animation.data.get(index * animation.stride + slot).copied().unwrap_or(*value);
            let next = animation.data.get((index + 1) * animation.stride + slot).copied().unwrap_or(here);
            *value = if smooth { here + (next - here) * fraction } else { here };
            slot += 1;
        }
    }
    out
}

/// True when a particle makes child particles at the frame `age` of its life.
fn is_time(timing: ChildTiming, age: u32, life: f32) -> bool {
    // Between two frames, every `interval` frames from `start`, until `end`.
    let every = |start: i32, end: i32, interval: i32| {
        let before = age as i32 - 1;
        let interval = interval.max(1);
        if before >= end {
            return false;
        }
        let now = (age as i32 - start).div_euclid(interval);
        now >= 0 && (before - start).div_euclid(interval) < now
    };
    match timing {
        ChildTiming::Birth => age == 0,
        ChildTiming::Death => age as f32 >= life,
        ChildTiming::LifeParts { start, end, interval } => every((start * life) as i32, (end * life) as i32, interval),
        ChildTiming::Frames { start, end, interval } => every(start, end, interval),
    }
}

/// The work that makes the particles of a frame.
struct Run<'a> {
    effect: &'a Effect,
    groups: Vec<Group>,
    /// The frame of the picture.
    now: u32,
    out: Vec<Particle>,
}

impl Run<'_> {
    /// Takes a particle from its birth to the frame of the picture: its streams change at each frame, it makes its child
    /// particles, and it is drawn when it still lives. `id` makes its random numbers.
    fn follow(&mut self, group: usize, mut p: Streams, born: u32, id: u64, chain: usize, rng: &mut Rng) {
        let age = self.now - born;
        let has_children = self.groups[group].has_children;
        if age as f32 >= p.life && !has_children {
            return;
        }
        // The frame of its death is the last one where its rules run.
        let last = age.min(p.life.ceil() as u32);
        let mut alive = true;
        let seed = |k: usize| [group as u64, born as u64, id, 0x4321 + k as u64];
        for step in 0..=last {
            // The place at the frame before.
            let previous = p.translate;
            for k in 0..self.groups[group].updaters.len() {
                let rule = self.groups[group].updaters[k].clone();
                let target = rule.target;
                match &rule.rule {
                    Updater::Acceleration { factor } => {
                        if let Some(stream) = p.vector(target) {
                            stream.iter_mut().for_each(|v| *v *= factor);
                        }
                    }
                    Updater::General => {
                        let velocity = p.velocity;
                        add(&mut p.translate, velocity, 1.0);
                    }
                    Updater::Gravity { power, direction } => {
                        if let Some(stream) = p.vector(target) {
                            add(stream, *direction, *power);
                        }
                    }
                    Updater::Spin { power, axis } => {
                        if *power != 0.0 && *axis != [0.0; 3] {
                            let axis = normalize(*axis);
                            if let Some(stream) = p.vector(target) {
                                *stream = rotate_around(*stream, axis, *power);
                            }
                        }
                    }
                    Updater::Random { power, interval } => {
                        let interval = (*interval).max(1) as u32;
                        if step % interval == 0 {
                            let noise = [rng.signed_one() * power[0], rng.signed_one() * power[1], rng.signed_one() * power[2]];
                            if let Some(stream) = p.vector(target) {
                                add(stream, noise, 1.0);
                            }
                        }
                    }
                    Updater::Magnet { power, position, flags } => {
                        let translate = p.translate;
                        if let Some(stream) = p.vector(target) {
                            for axis in 0..3 {
                                if flags >> axis & 1 != 0 {
                                    stream[axis] += (position[axis] - translate[axis] - stream[axis]) * power;
                                }
                            }
                        }
                    }
                    // The ground is a height: the particle bounces on it (its fall keeps the part `power`), or dies.
                    Updater::Collision { power, position, flags } => {
                        if p.translate[1] < *position {
                            p.translate[1] = position + 1e-4;
                            if flags & 2 == 0 {
                                p.velocity[1] *= -power;
                            } else {
                                alive = false;
                            }
                        }
                    }
                    Updater::Add { value, animation } => {
                        let value = match animation {
                            Some(animation) => {
                                let v = animation_values(animation, value, step as f32, p.life, seed(k));
                                [v[0], v[1], v[2]]
                            }
                            None => *value,
                        };
                        if let Some(stream) = p.vector(target) {
                            add(stream, value, 1.0);
                        }
                    }
                    Updater::AddRandom { value } => {
                        let noise = [rng.signed_one() * value[0], rng.signed_one() * value[1], rng.signed_one() * value[2]];
                        if let Some(stream) = p.vector(target) {
                            add(stream, noise, 1.0);
                        }
                    }
                    Updater::TurnToMove { along_speed } => {
                        let way = normalize(if *along_speed { p.velocity } else { [0, 1, 2].map(|k| p.translate[k] - previous[k]) });
                        if way != [0.0; 3] {
                            p.rotate = [way[1].clamp(-1.0, 1.0).acos(), way[0].atan2(way[2]), 0.0];
                        }
                    }
                    Updater::Child { group: Some(child), shape: Some(shape), timing, count } => {
                        if chain < MAX_CHAIN && is_time(*timing, step, p.life) {
                            self.children(&p, *child, *shape, *count, born + step, id, chain, rng);
                        }
                    }
                    _ => {}
                }
            }
            if !alive {
                return;
            }
        }
        if (age as f32) < p.life && self.out.len() < MAX_PARTICLES {
            self.draw(group, p, born, age, id);
        }
    }

    /// The child particles that a particle makes at the frame `born`.
    #[allow(clippy::too_many_arguments)]
    fn children(&mut self, parent: &Streams, group: usize, shape: usize, count: i32, born: u32, id: u64, chain: usize, rng: &mut Rng) {
        let shape = Shape::from_values(self.effect.shape_kind(shape), self.effect.shape_values(shape));
        let count = count.clamp(0, 256) as u32;
        let start = shape.start_angle(rng);
        for n in 0..count {
            if self.out.len() >= MAX_PARTICLES {
                return;
            }
            let child_id = id.wrapping_mul(0x100_0003).wrapping_add(u64::from(born) << 16 | u64::from(n));
            let mut random = Rng::new([group as u64, u64::from(born), child_id, 0x71D]);
            let place = shape.position(&mut random, start, n, count);
            let mut child = new_particle(&self.groups[group], place, &mut random);
            add(&mut child.translate, parent.translate, 1.0);
            if self.groups[group].inherits_speed {
                add(&mut child.velocity, parent.velocity, 1.0);
            }
            self.follow(group, child, born, child_id, chain + 1, &mut random);
        }
    }

    /// Adds a living particle to the picture: the rules that set a stream from the part of the life that has passed run
    /// first.
    fn draw(&mut self, group: usize, mut p: Streams, born: u32, age: u32, id: u64) {
        let life_part = (age as f32 / p.life).clamp(0.0, 1.0);
        let seed = |k: usize| [group as u64, born as u64, id, 0x4321 + k as u64];
        for (k, rule) in self.groups[group].updaters.iter().enumerate() {
            let target = rule.target;
            match &rule.rule {
                Updater::Set { value, animation } => {
                    let value = match animation {
                        Some(animation) => {
                            let v = animation_values(animation, value, age as f32, p.life, seed(k));
                            [v[0], v[1], v[2]]
                        }
                        None => *value,
                    };
                    if let Some(stream) = p.vector(target) {
                        *stream = value;
                    }
                }
                Updater::SetVector2 { value, animation } => {
                    let value = match animation {
                        Some(animation) => {
                            let v = animation_values(animation, value, age as f32, p.life, seed(k));
                            [v[0], v[1]]
                        }
                        None => *value,
                    };
                    match target {
                        5 => p.uv_translate = value,
                        6 => p.uv_scale = value,
                        _ => {}
                    }
                }
                Updater::TexturePattern { divisions, index, animation } => {
                    let cell = match animation {
                        Some(animation) if animation.enabled.first().copied().unwrap_or(false) => animation_values_in(animation, &[*index as f32], age as f32, p.life, seed(k), false)[0] as i32,
                        _ => *index,
                    };
                    let (dx, dy) = (divisions[0] as i32, divisions[1] as i32);
                    let cell = cell.clamp(0, dx * dy - 1);
                    // The translation of the picture that shows the cell (the cells go from the left to the right, from the top down).
                    if target == 5 {
                        p.uv_translate = [-0.5 + dx as f32 * 0.5 - (cell % dx) as f32, 0.5 - dy as f32 * 0.5 + (dy - cell / dx - 1) as f32];
                    }
                }
                Updater::SetNumber { value, animation } => {
                    let value = match animation {
                        Some(animation) => animation_values(animation, &[*value], age as f32, p.life, seed(k))[0],
                        None => *value,
                    };
                    match target {
                        4 => p.alpha = value,
                        7 => p.uv_turn = value,
                        _ => {}
                    }
                }
                Updater::NumberKeys { in_time, out_time, values } => {
                    if target == 4 {
                        p.alpha = (keys_at(*in_time, *out_time, *values, life_part) / 65535.0) as f32;
                    }
                }
                Updater::VectorKeys { in_time, out_time, values } => {
                    let value = [0, 1, 2].map(|axis| keys_at(*in_time, *out_time, [values[0][axis], values[1][axis], values[2][axis], values[3][axis], values[4][axis]], life_part) as f32);
                    if let Some(stream) = p.vector(target) {
                        *stream = value;
                    }
                }
                _ => {}
            }
        }
        self.out.push(Particle {
            group,
            born,
            position: p.translate,
            // The size of a particle is its half size: the game puts the corners at plus and minus the size.
            size: [2.0 * p.scale[0] * p.scale_ext[0], 2.0 * p.scale[1] * p.scale_ext[1]],
            color: p.color,
            alpha: p.alpha,
            rotate: p.rotate,
            uv_translate: p.uv_translate,
            uv_scale: p.uv_scale,
            uv_turn: p.uv_turn,
        });
    }
}

/// The longest life of the particles of a set, with the one of the child particles that they make, in frames.
fn longest_life(groups: &[Group], group: usize, chain: usize) -> f32 {
    let own = groups[group].initializers.iter().filter(|r| r.target == 11).map(|r| match r.rule {
        Initializer::FloatRange { max, min } => max.max(min),
        Initializer::FloatImmediate { value } => value,
        Initializer::Float { base, random } => base * (1.0 + random),
        _ => 0.0,
    });
    let constant = groups[group].parameters.iter().filter(|p| p.0 == 11).filter_map(|p| p.1.first().copied());
    let own = own.chain(constant).fold(1.0, f32::max);
    let children = groups[group].updaters.iter().filter_map(|r| match r.rule {
        Updater::Child { group: Some(child), .. } if chain < MAX_CHAIN => Some(longest_life(groups, child, chain + 1)),
        _ => None,
    });
    own + children.fold(0.0, f32::max)
}

/// The frames that an effect lasts: the end of the last particle.
pub fn length(effect: &Effect) -> f32 {
    let groups = Group::all(effect);
    (0..effect.layer_count())
        .filter_map(|layer| {
            let (em, group) = (effect.emission(layer), effect.group_of(layer)?);
            Some(em.start as f32 + if em.infinite { ENDLESS_FRAMES } else { em.span.max(0) as f32 } + longest_life(&groups, group, 0) + 1.0)
        })
        .fold(10.0, f32::max)
}

/// The frames where a layer makes particles, and how many (the rules of the emitter of the game: the counts add up, and the number of
/// whole particles is made at each frame).
fn births(effect: &Effect, layer: usize, last_frame: u32) -> Vec<(u32, u32)> {
    let em = effect.emission(layer);
    let mut rng = Rng::new([layer as u64, 0xE17, 0, 0]);
    let (mut next, mut pending, mut first) = (0.0f32, 0.0f32, true);
    let mut out = Vec::new();
    for frame in 0..=last_frame {
        let cooked = frame as f32 - em.start as f32 - 1.0;
        if cooked < 0.0 || (!em.infinite && cooked >= em.span as f32) {
            continue;
        }
        while next <= cooked && (em.infinite || next <= em.span as f32 - 1.0) {
            pending += (em.ratio * (1.0 + em.ratio_random * rng.signed_one())).max(0.0);
            next += ((em.interval as f32 * (1.0 + em.interval_random * rng.signed_one())) as i32).max(1) as f32;
        }
        if first && em.ratio != 0.0 && pending > 0.0 {
            first = false;
            pending = pending.max(1.0);
        }
        let count = pending as u32;
        pending -= count as f32;
        if count > 0 {
            out.push((frame, count));
        }
    }
    out
}

/// All the particles at a frame.
pub fn particles_at(effect: &Effect, frame: f32) -> Vec<Particle> {
    let now = frame.max(0.0).floor() as u32;
    let mut run = Run { effect, groups: Group::all(effect), now, out: Vec::new() };
    for layer in 0..effect.layer_count() {
        let Some(group) = effect.group_of(layer) else { continue };
        let shape = Shape::of(effect, layer);
        let place = effect.emitter_position(layer);
        let (scale, turn) = effect.emitter_scale_and_turn(layer);
        // The place and the speed of a particle in the space of the emitter, then in the one of the effect.
        let direction = |v: [f32; 3]| turned([v[0] * scale[0], v[1] * scale[1], v[2] * scale[2]], turn);
        for (born, count) in births(effect, layer, now) {
            let start = shape.start_angle(&mut Rng::new([layer as u64, born as u64, 0x57A7, 0]));
            for n in 0..count {
                let mut rng = Rng::new([layer as u64, born as u64, n as u64, 0x9A5]);
                let at = shape.position(&mut rng, start, n, count);
                let mut p = new_particle(&run.groups[group], at, &mut rng);
                p.translate = direction(p.translate);
                add(&mut p.translate, place, 1.0);
                p.velocity = direction(p.velocity);
                run.follow(group, p, born, (layer as u64) << 32 | n as u64, 0, &mut rng);
                if run.out.len() >= MAX_PARTICLES {
                    return run.out;
                }
            }
        }
    }
    run.out
}

/// How the particles of each set of an effect look.
pub fn looks(effect: &Effect) -> Vec<Look> {
    (0..effect.group_count())
        .map(|group| {
            let material = effect.group_material(group);
            let picture = material.as_ref().and_then(|m| m.picture).and_then(|index| {
                let info = effect.pictures().get(index)?;
                Some(Picture { width: info.width, height: info.height, rgba: effect.picture_rgba(index)? })
            });
            let mut stages = material.as_ref().map_or_else(Vec::new, |m| m.stages.clone());
            // The steps at the end that keep what they receive are not run: the drawing is faster.
            let keeps = |s: &Stage| s.color_source[0] == 15 && s.alpha_source[0] == 15 && s.color_mode == 0 && s.alpha_mode == 0 && s.color_operand[0] == 0 && s.alpha_operand[0] == 0 && s.color_scale == 0 && s.alpha_scale == 0 && !s.save_color && !s.save_alpha;
            while stages.last().is_some_and(keeps) {
                stages.pop();
            }
            Look {
                picture,
                stages,
                buffer_color: material.as_ref().map_or([255; 4], |m| m.buffer_color),
                // A set without material (it should not happen) is added, like most of the light effects.
                state: material.as_ref().map_or(RenderState { known: true, blend: true, color_source: 6, color_destination: 1, ..Default::default() }, |m| m.state),
                matrix: material.as_ref().map_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], |m| m.matrix),
                wrap: material.as_ref().map_or([2, 2], |m| m.wrap),
                builder: effect.group_builder(group),
                offset: effect.group_offset(group),
                order: effect.group_order(group),
                oldest_first: effect.group_oldest_first(group),
            }
        })
        .collect()
}

/// The radius of the area that the preview shows with the default point of view: it holds nearly all the particles that can
/// be seen at some frames, with their size. The particles that fly far away do not make the others tiny.
pub fn extent(effect: &Effect) -> f32 {
    let end = length(effect);
    let camera = Camera::default();
    let (sin_y, cos_y, sin_p, cos_p) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    let mut reaches = Vec::new();
    for step in 0..12 {
        for p in particles_at(effect, end * step as f32 / 12.0).into_iter().filter(|p| p.alpha > 0.02) {
            let (x, y, z) = (p.position[0], p.position[1], p.position[2]);
            let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
            let y2 = y * cos_p - z1 * sin_p;
            reaches.push((x1 * x1 + y2 * y2).sqrt() + p.size[0].abs().max(p.size[1].abs()) * 0.5);
        }
    }
    reaches.sort_by(f32::total_cmp);
    // The farthest particle, unless it is much farther than 3 particles out of 4.
    let most = reaches.get(reaches.len().saturating_sub(1) * 3 / 4).copied().unwrap_or(1.0);
    let farthest = reaches.last().copied().unwrap_or(1.0);
    farthest.min(most * 1.8).max(1.0) * 1.1
}

/// An effect in a scene: its frame and the place of its origin.
pub struct Item<'a> {
    pub effect: &'a Effect,
    pub looks: &'a [Look],
    pub frame: f32,
    pub origin: [f32; 3],
}

/// A mark in a scene (a Pokémon): a vertical bar and a disc on the ground.
pub struct Marker {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub radius: f32,
}

/// The two half sides of the rectangle of a particle in the world: to its right and to its top.
fn quad(p: &Particle, look: &Look, right: [f32; 3], up: [f32; 3], viewer: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let (width, height) = (p.size[0] * 0.5, p.size[1] * 0.5);
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // A turn of the two sides in the plane that faces the viewer.
    let spun = |x: [f32; 3], y: [f32; 3]| {
        let angle = p.rotate[2];
        (rotate_around(x, viewer, angle), rotate_around(y, viewer, angle))
    };
    match look.builder {
        Builder::Billboard => spun(right.map(|v| v * width), up.map(|v| v * height)),
        Builder::WorldBillboard => {
            let world_up = if viewer[1].abs() < 0.99999 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
            let y = normalize(cross(viewer, cross(world_up, viewer)));
            let x = normalize(cross(y, viewer));
            spun(x.map(|v| v * width), y.map(|v| v * height))
        }
        // The top of the picture is turned by the particle; the picture then turns around it to face the viewer.
        Builder::UpBillboard => {
            let y = turned([0.0, 1.0, 0.0], p.rotate);
            if dot(viewer, y).abs() >= 0.99999 {
                return ([0.0; 3], [0.0; 3]);
            }
            let facing = cross(cross(y, viewer), y);
            let x = normalize(cross(y, facing));
            (x.map(|v| v * width), y.map(|v| v * height))
        }
        Builder::Plane => (turned([width, 0.0, 0.0], p.rotate), turned([0.0, height, 0.0], p.rotate)),
    }
}

/// The coordinates of the picture of a particle: the ones of the bottom left corner, and the changes along the width and
/// along the height (the V axis goes up).
fn coordinates(p: &Particle) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let (sin, cos) = p.uv_turn.sin_cos();
    let [sx, sy] = p.uv_scale;
    let (x, y) = (-p.uv_translate[0] - 0.5, p.uv_translate[1] - 0.5);
    ([-sx * sin * y + sx * cos * x + 0.5, sy * cos * y + sy * sin * x + 0.5], [sx * cos, sy * sin], [-sx * sin, sy * cos])
}

/// The colour of the picture of a set at a place (from 0 to 1, from its first row), or `None` outside of a picture
/// that does not go on there. A set without a picture shows a soft round spot, at the place `spot` of its rectangle.
fn sample(look: &Look, u: f32, v: f32, spot: [f32; 2]) -> Option<[f32; 4]> {
    let Some(picture) = &look.picture else {
        let soft = (1.0 - (spot[0].powi(2) + spot[1].powi(2)).sqrt() * 2.0).clamp(0.0, 1.0);
        return Some([soft; 4]);
    };
    let wrap = |x: f32, way: u32| -> Option<f32> {
        match way {
            0 => Some(x.clamp(0.0, 0.9999)),
            1 => (0.0..1.0).contains(&x).then_some(x),
            3 => {
                let t = x.rem_euclid(2.0);
                Some(if t < 1.0 { t } else { 1.9999 - t })
            }
            _ => Some(x - x.floor()),
        }
    };
    let (u, v) = (wrap(u, look.wrap[0])?, wrap(v, look.wrap[1])?);
    // The game filters the picture linearly: the four texels around the point, mixed.
    let (fx, fy) = (u * picture.width as f32 - 0.5, v * picture.height as f32 - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (ax, ay) = (fx - x0, fy - y0);
    let texel_at = |x: f32, y: f32| -> [f32; 4] {
        let wrap_index = |i: i32, size: usize, way: u32| -> usize {
            let n = size as i32;
            (match way {
                2 => i.rem_euclid(n),
                3 => {
                    let m = i.rem_euclid(2 * n);
                    if m < n { m } else { 2 * n - 1 - m }
                }
                _ => i.clamp(0, n - 1),
            }) as usize
        };
        let (ix, iy) = (wrap_index(x as i32, picture.width, look.wrap[0]), wrap_index(y as i32, picture.height, look.wrap[1]));
        let at = (iy * picture.width + ix) * 4;
        [picture.rgba[at], picture.rgba[at + 1], picture.rgba[at + 2], picture.rgba[at + 3]].map(|c| f32::from(c) / 255.0)
    };
    let (p00, p10, p01, p11) = (texel_at(x0, y0), texel_at(x0 + 1.0, y0), texel_at(x0, y0 + 1.0), texel_at(x0 + 1.0, y0 + 1.0));
    Some(std::array::from_fn(|k| (p00[k] * (1.0 - ax) + p10[k] * ax) * (1.0 - ay) + (p01[k] * (1.0 - ax) + p11[k] * ax) * ay))
}

/// The comparisons of the test of the opacity.
fn passes(function: u8, value: f32, reference: f32) -> bool {
    match function {
        0 => false,
        1 => true,
        2 => value == reference,
        3 => value != reference,
        4 => value < reference,
        5 => value <= reference,
        6 => value > reference,
        _ => value >= reference,
    }
}

/// Draws the effect at a frame on a dark background. Returns `side` × `side` pixels of RGBA.
pub fn render(effect: &Effect, looks: &[Look], frame: f32, camera: Camera, extent: f32, side: usize) -> Vec<u8> {
    render_scene(&[Item { effect, looks, frame, origin: [0.0; 3] }], &[], false, camera, extent, side)
}

/// Draws several effects and marks together, with a grid on the ground when asked.
pub fn render_scene(items: &[Item], markers: &[Marker], grid: bool, camera: Camera, extent: f32, side: usize) -> Vec<u8> {
    let mut canvas = vec![[24.0f32, 26.0, 34.0]; side * side];
    let scale = side as f32 / (2.0 * extent / camera.zoom.max(0.05));
    let centre = side as f32 / 2.0;
    let (sin_y, cos_y, sin_p, cos_p) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    // A vector of the world on the screen: right and up (in pixels), and the depth.
    let project = |v: [f32; 3]| -> (f32, f32, f32) {
        let (x1, z1) = (v[0] * cos_y + v[2] * sin_y, -v[0] * sin_y + v[2] * cos_y);
        (x1 * scale, (v[1] * cos_p - z1 * sin_p) * scale, v[1] * sin_p + z1 * cos_p)
    };
    let dot = |canvas: &mut Vec<[f32; 3]>, v: [f32; 3], color: [f32; 3], radius: f32| {
        let (x, y, _) = project(v);
        let (cx, cy) = (centre + x, centre - y);
        let reach = radius.ceil() as i32;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if (dx * dx + dy * dy) as f32 <= radius * radius {
                    let (px, py) = ((cx as i32 + dx), (cy as i32 + dy));
                    if px >= 0 && py >= 0 && (px as usize) < side && (py as usize) < side {
                        canvas[py as usize * side + px as usize] = color.map(|c| c * 255.0);
                    }
                }
            }
        }
    };
    if grid {
        let step = (extent / 6.0).max(0.5);
        let lines = (extent / step).ceil() as i32;
        for line in -lines..=lines {
            let at = line as f32 * step;
            let color = if line == 0 { [90.0 / 255.0, 100.0 / 255.0, 130.0 / 255.0] } else { [48.0 / 255.0, 52.0 / 255.0, 66.0 / 255.0] };
            for k in 0..=(side as i32 * 2) {
                let along = -extent * 1.5 + k as f32 * extent * 3.0 / (side as f32 * 2.0);
                dot(&mut canvas, [at, 0.0, along], color, 0.5);
                dot(&mut canvas, [along, 0.0, at], color, 0.5);
            }
        }
    }
    for marker in markers {
        for k in 0..=24 {
            dot(&mut canvas, [marker.position[0], marker.position[1] + marker.radius * 2.0 * k as f32 / 24.0, marker.position[2]], marker.color, 1.5);
        }
        for k in 0..48 {
            let angle = k as f32 / 48.0 * std::f32::consts::TAU;
            dot(&mut canvas, [marker.position[0] + angle.cos() * marker.radius, marker.position[1], marker.position[2] + angle.sin() * marker.radius], marker.color, 1.5);
        }
    }
    let mut list: Vec<(usize, Particle, &Look)> = Vec::new();
    for (number, item) in items.iter().enumerate() {
        for mut p in particles_at(item.effect, item.frame) {
            if let Some(look) = item.looks.get(p.group) {
                for k in 0..3 {
                    p.position[k] += item.origin[k];
                }
                list.push((number, p, look));
            }
        }
    }
    // The game draws one set after the other, and the particles of a set in the order of their birth.
    list.sort_by_key(|(number, p, look)| (look.order, *number, p.group, if look.oldest_first { i64::from(p.born) } else { -i64::from(p.born) }));
    // The directions of the screen in the world: to the right, up, and to the viewer.
    let right = [cos_y, 0.0, sin_y];
    let up = [sin_y * sin_p, cos_p, -cos_y * sin_p];
    let viewer = [-sin_y * cos_p, sin_p, cos_y * cos_p];
    for (_, p, look) in list {
        let (x, y) = quad(&p, look, right, up, viewer);
        // The place of the picture around the particle.
        let centre_at: [f32; 3] = std::array::from_fn(|k| p.position[k] - x[k] * look.offset[0] - y[k] * look.offset[1]);
        let (cx, cy, _) = project(centre_at);
        let (cx, cy) = (centre + cx, centre - cy);
        let ((rx, ry), (ux, uy)) = {
            let (r, u) = (project(x.map(|v| v * 2.0)), project(y.map(|v| v * 2.0)));
            ((r.0, r.1), (u.0, u.1))
        };
        // Screen: x to the right, y down. The picture point (a, b), from -0.5 to 0.5, is at c + a × right + b × up (the up goes toward -y).
        let (ex, ey, fx, fy) = (rx, -ry, ux, -uy);
        let det = ex * fy - ey * fx;
        if det.abs() < 1e-6 {
            continue;
        }
        let reach = ((ex.abs() + fx.abs()) * 0.5).max(1.0);
        let reach_y = ((ey.abs() + fy.abs()) * 0.5).max(1.0);
        let (x_from, x_to) = (((cx - reach).floor().max(0.0)) as usize, ((cx + reach).ceil().min(side as f32)) as usize);
        let (y_from, y_to) = (((cy - reach_y).floor().max(0.0)) as usize, ((cy + reach_y).ceil().min(side as f32)) as usize);
        // The coordinates of the picture: the one of the corner, and the way along each side of the rectangle.
        let (base, along_a, along_b) = coordinates(&p);
        let primary = [p.color[0], p.color[1], p.color[2], p.alpha].map(|c| c.clamp(0.0, 1.0));
        for py in y_from..y_to {
            for px in x_from..x_to {
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                // The point of the picture under the pixel.
                let (a, b) = ((dx * fy - dy * fx) / det, (ex * dy - ey * dx) / det);
                if !(-0.5..0.5).contains(&a) || !(-0.5..0.5).contains(&b) {
                    continue;
                }
                let own: [f32; 2] = std::array::from_fn(|k| base[k] + (a + 0.5) * along_a[k] + (b + 0.5) * along_b[k]);
                let [u, v] = look.matrix.map(|row| row[0] * own[0] + row[1] * own[1] + row[2]);
                // The rows of a decoded picture count like the V axis of the graphics chip (as for the models).
                let Some(texel) = sample(look, u, v, [a, b]) else { continue };
                let mixed = sango_core::render::combine_values(&look.stages, look.buffer_color, &[texel, [1.0; 4], [1.0; 4]], 1.0, primary, false);
                if let Some((function, reference)) = look.state.alpha_test {
                    if !passes(function, (mixed[3] * 255.0).round(), f32::from(reference)) {
                        continue;
                    }
                }
                let pixel = &mut canvas[py * side + px];
                let behind = pixel.map(|c| c / 255.0);
                let (color, _) = sango_core::render::blend(&look.state, [mixed[0], mixed[1], mixed[2]], mixed[3], behind, 1.0);
                *pixel = color.map(|c| c.clamp(0.0, 1.0) * 255.0);
            }
        }
    }
    canvas.into_iter().flat_map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, 255]).collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    /// With the dump of the game: the particles of some effects are computed, and the preview draws something that is not the background.
    #[test]
    fn the_preview_draws_the_particles_of_the_effects() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        let (mut drawn, mut tried) = (0, 0);
        for id in (0..garc.file_count()).step_by(23) {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let Ok(effect) = Effect::parse(&bytes) else { continue };
            tried += 1;
            let pictures = looks(&effect);
            let extent = extent(&effect);
            assert!(extent.is_finite() && extent > 0.0);
            let end = length(&effect);
            let background = [24u8, 26, 34, 255];
            let any = (1..8).any(|step| render(&effect, &pictures, end * step as f32 / 9.0, Camera::default(), extent, 96).chunks(4).any(|p| p != background));
            drawn += usize::from(any);
        }
        assert!(tried > 50);
        assert!(drawn * 10 > tried * 6, "{drawn} of {tried} effects show particles");
        // The same frame always gives the same particles.
        let bytes = garc.first_file_decompressed(0).unwrap();
        let effect = Effect::parse(&bytes).unwrap();
        assert_eq!(particles_at(&effect, 10.0).len(), particles_at(&effect, 10.0).len());
        // The fire beam (the effect 0) shoots its particles along the axis of the beam: the first initializer of speed gives 3 along -Z.
        let speed = effect.initializers(0).iter().any(|r| matches!(r.rule, Initializer::Directional { power, direction } if power == 3.0 && direction == [0.0, 0.0, -1.0]));
        assert!(speed, "the beam has its speed");
        let far = particles_at(&effect, 30.0).iter().map(|p| p.position[2].abs()).fold(0.0f32, f32::max);
        assert!(far > 20.0, "the particles of the beam travel ({far})");
    }

    /// With the dump of the game: each set of particles of an effect has its picture, the particles make their child
    /// particles, and the steps and the colours of the materials are the ones of the file.
    #[test]
    fn the_particles_follow_the_file() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        let read = |id: usize| Effect::parse(&garc.first_file_decompressed(id).unwrap()).unwrap();
        // Each material of the thunder 337 names its own picture.
        let thunder = read(337);
        let names: Vec<String> = looks(&thunder).iter().zip(0..).map(|(_, group)| thunder.pictures()[thunder.group_material(group).unwrap().picture.unwrap()].name.clone()).collect();
        assert_eq!(names, ["etxp_thunder01m", "etxp_circle05m", "etxp_ring04m", "etxp_dot01m"]);
        // Its first colour is a yellow of the material, not the black of the commands.
        assert_eq!(thunder.group_material(0).unwrap().stages[0].constant, [255, 255, 0, 255]);
        // The particles of the flash make the rings, from a set that has no layer.
        assert!(thunder.group_updaters(1).iter().any(|r| matches!(r.rule, Updater::Child { group: Some(2), .. })));
        assert!(thunder.layer_count() < thunder.group_count());
        // The effect 160 shows only child particles: its first ones have no opacity.
        let hearts = read(160);
        assert!(hearts.group_count() > hearts.layer_count());
        let shown = particles_at(&hearts, 5.0);
        assert!(shown.iter().any(|p| p.group == 1 && p.alpha > 0.5), "{} particles", shown.len());
        assert!(length(&hearts) > 30.0, "the child particles live longer than the first ones");
        let farthest = (0..40).flat_map(|f| particles_at(&hearts, f as f32)).map(|p| p.position[1]).fold(0.0f32, f32::max);
        assert!(extent(&hearts) < farthest * 0.7, "{} for particles up to {farthest}", extent(&hearts));
    }

    #[test]
    fn a_child_particle_is_made_at_its_moment() {
        let frames = |timing: ChildTiming| (0..=10).filter(|&age| is_time(timing, age, 10.0)).collect::<Vec<_>>();
        assert_eq!(frames(ChildTiming::Birth), [0]);
        assert_eq!(frames(ChildTiming::Death), [10]);
        assert_eq!(frames(ChildTiming::Frames { start: 2, end: 8, interval: 3 }), [2, 5, 8]);
        assert_eq!(frames(ChildTiming::LifeParts { start: 0.5, end: 1.0, interval: 2 }), [5, 7, 9]);
    }

    /// The rectangle of a particle: it faces the screen, or lies where its turn puts it.
    #[test]
    fn a_rectangle_turns_like_its_kind() {
        let p = Particle { group: 0, born: 0, position: [0.0; 3], size: [4.0, 2.0], color: [1.0; 3], alpha: 1.0, rotate: [std::f32::consts::FRAC_PI_2, 0.0, 0.0], uv_translate: [0.0; 2], uv_scale: [1.0; 2], uv_turn: 0.0 };
        let mut look = Look { picture: None, stages: Vec::new(), buffer_color: [255; 4], state: RenderState::default(), matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], wrap: [2, 2], builder: Builder::Plane, offset: [0.0; 3], order: (0, 0), oldest_first: true };
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5);
        let (right, up, viewer) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        // A quarter turn around x lays the plane on the ground.
        let (x, y) = quad(&p, &look, right, up, viewer);
        assert!(near(x, [2.0, 0.0, 0.0]) && near(y, [0.0, 0.0, 1.0]), "{x:?} {y:?}");
        // A picture that faces the screen only takes the turn around z.
        look.builder = Builder::Billboard;
        let (x, y) = quad(&p, &look, right, up, viewer);
        assert!(near(x, [2.0, 0.0, 0.0]) && near(y, [0.0, 1.0, 0.0]), "{x:?} {y:?}");
        // An upright picture keeps the top that its turn gives, and faces the viewer.
        look.builder = Builder::UpBillboard;
        let p = Particle { rotate: [0.0; 3], ..p };
        let (x, y) = quad(&p, &look, right, up, viewer);
        assert!(near(x, [2.0, 0.0, 0.0]) && near(y, [0.0, 1.0, 0.0]), "{x:?} {y:?}");
        // The coordinates of a picture shown whole go from 0 to 1.
        assert_eq!(coordinates(&p), ([0.0, 0.0], [1.0, 0.0], [-0.0, 1.0]));
    }

    /// Writes a strip of pictures of some effects in the folder named by `SANGO_PREVIEW_OUT`, to look at them.
    #[test]
    fn write_a_sheet_of_frames() {
        let (Ok(root), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("SANGO_PREVIEW_OUT")) else { return };
        let Some(id) = std::env::var("SANGO_SHEET_ID").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        let effect = Effect::parse(&garc.first_file_decompressed(id).unwrap()).unwrap();
        let pictures = looks(&effect);
        let (side, columns, rows) = (160usize, 8usize, 4usize);
        let mut sheet = image::RgbaImage::new((side * columns) as u32, (side * rows) as u32);
        for n in 0..columns * rows {
            let pixels = render(&effect, &pictures, n as f32, Camera { yaw: 0.0, pitch: 0.1, zoom: 1.0 }, 30.0, side);
            let frame = image::RgbaImage::from_raw(side as u32, side as u32, pixels).unwrap();
            image::imageops::replace(&mut sheet, &frame, ((n % columns) * side) as i64, ((n / columns) * side) as i64);
        }
        sheet.save(std::path::Path::new(&out).join(format!("sheet_{id}.png"))).unwrap();
    }

    #[test]
    fn write_some_pictures_to_look_at() {
        let (Ok(root), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("SANGO_PREVIEW_OUT")) else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        let ids: Vec<usize> = std::env::var("SANGO_PREVIEW_IDS").ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_else(|| vec![0, 5, 100, 250, 723]);
        for id in ids {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let Ok(effect) = Effect::parse(&bytes) else { continue };
            let (pictures, extent, end) = (looks(&effect), extent(&effect), length(&effect));
            let side = 200;
            let mut strip = image::RgbaImage::new(side as u32 * 6, side as u32);
            for step in 0..6 {
                let pixels = render(&effect, &pictures, end * (step as f32 + 0.5) / 6.0, Camera::default(), extent, side);
                let frame = image::RgbaImage::from_raw(side as u32, side as u32, pixels).unwrap();
                image::imageops::replace(&mut strip, &frame, side as i64 * step, 0);
            }
            strip.save(std::path::Path::new(&out).join(format!("effect_{id}.png"))).unwrap();
        }
    }
}
