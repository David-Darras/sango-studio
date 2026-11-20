//! A preview of an effect: the particles of all the layers at a frame, drawn on the processor.
//!
//! The particles follow the rules of the particle system of the game, which the effect files hold:
//!
//! - The emitter makes particles from its start frame, with the number, the interval and the randoms of the layer.
//! - A new particle gets its place from the shape of the emitter, then each initializer gives a first value to a stream (the speed, the size,
//!   the turn, the colour, the transparency, the life), in the order of the file.
//! - At each frame, each updater changes a stream, in the order of the file: the air resistance, the gravity, the turn of a speed, the move by
//!   the speed, the keys of the colour, of the size and of the transparency.
//!
//! A particle is computed from its birth to the frame by going through the frames one after the other, so the user can go to any frame
//! and the same frame always gives the same particles. What the preview does not do: the child particles, the collisions, the pictures with
//! several frames, the mix mode of the material, and the matrix of the emitter. The drawing is flat: a picture facing the viewer, added to
//! the dark background like the light effects of the game.

use crate::effect_file::{Animation, AnimationKind, Builder, Effect, FormKind, Initializer, Updater};

/// The most particles that the preview draws at one frame.
const MAX_PARTICLES: usize = 3000;
/// The frames that an emitter without end is shown.
const ENDLESS_FRAMES: f32 = 120.0;

/// A particle at a frame.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub layer: usize,
    pub position: [f32; 3],
    /// The width and the height of the picture.
    pub size: [f32; 2],
    pub color: [f32; 3],
    pub alpha: f32,
    /// The turn of the picture (radians).
    pub angle: f32,
    /// The coordinates of the picture on the particle: the translation and the scale.
    pub uv_translate: [f32; 2],
    pub uv_scale: [f32; 2],
}

/// A picture of the particles, as RGBA pixels.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// How a layer looks: its picture, how it is mixed, the coordinates of the picture and the way that the particle is drawn.
#[derive(Clone, Debug)]
pub struct Look {
    pub picture: Option<Picture>,
    pub additive: bool,
    pub uv_scale: [f32; 2],
    pub uv_translate: [f32; 2],
    pub wrap: [u32; 2],
    pub mode: u32,
    pub builder: Builder,
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
    scale: [f32; 3],
}

impl Shape {
    fn of(effect: &Effect, layer: usize) -> Shape {
        let fields = effect.form_values(layer);
        let value = |name: &str| fields.iter().find(|(field, _)| field.name == name).map_or([0.0; 3], |(_, v)| *v);
        let kind = effect.form_kind(layer);
        let mut scale = value("Size");
        // A flat shape has two numbers: the width and the depth.
        if matches!(kind, Some(FormKind::Disc) | Some(FormKind::Rectangle)) {
            scale = [scale[0], 0.0, scale[1]];
        }
        let arc_width = value("Arc")[0];
        Shape { kind, inner: value("Empty part")[0], arc_width: if arc_width == 0.0 { std::f32::consts::TAU } else { arc_width }, arc_start: value("Arc start")[0], arc_swing: value("Arc swing")[0], fixed_start: value("Fixed start")[0] != 0.0, scale }
    }

    /// The place of a new particle in the shape (the size of a shape is its radius, or half of its width).
    fn position(&self, rng: &mut Rng) -> [f32; 3] {
        let tau = std::f32::consts::TAU;
        let offset = if self.fixed_start { self.arc_start } else { rng.next() * tau };
        let s = self.scale;
        match self.kind {
            None | Some(FormKind::Point) => [0.0; 3],
            Some(FormKind::Cube) => [rng.signed_one() * s[0], rng.signed_one() * s[1], rng.signed_one() * s[2]],
            Some(FormKind::Rectangle) => [rng.signed_one() * s[0], 0.0, rng.signed_one() * s[2]],
            Some(FormKind::Disc) | Some(FormKind::Cylinder) => {
                let d = rng.next();
                let distance = if self.inner == 0.0 {
                    d.sqrt()
                } else if self.inner >= 1.0 {
                    1.0
                } else {
                    (d + self.inner * self.inner * (1.0 - d)).sqrt()
                };
                let mut angle = if self.kind == Some(FormKind::Disc) { rng.signed_half() * self.arc_width } else { rng.next() * self.arc_width } + offset;
                if self.arc_swing != 0.0 {
                    angle += rng.signed_half() * self.arc_swing;
                }
                let height = if self.kind == Some(FormKind::Cylinder) { rng.signed_one() * s[1] } else { 0.0 };
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
                let mut yaw = rng.next() * self.arc_width + offset;
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
fn new_particle(effect: &Effect, layer: usize, shape: &Shape, rng: &mut Rng) -> Streams {
    let mut p = Streams { translate: shape.position(rng), velocity: [0.0; 3], scale: [1.0; 3], scale_ext: [1.0; 3], rotate: [0.0; 3], color: [1.0; 3], alpha: 1.0, life: 1.0, uv_translate: [0.0; 2], uv_scale: [1.0; 2] };
    // The constant values of the layer come first: an initializer or an updater of the same stream replaces them.
    for (usage, numbers) in effect.parameters(layer) {
        let three = |numbers: &[f32]| [numbers.first().copied().unwrap_or(0.0), numbers.get(1).copied().unwrap_or(0.0), numbers.get(2).copied().unwrap_or(0.0)];
        match usage {
            1 => p.scale = three(&numbers),
            8 => p.scale_ext = three(&numbers),
            2 => p.rotate = three(&numbers),
            3 => p.color = three(&numbers),
            4 => p.alpha = numbers.first().copied().unwrap_or(1.0),
            5 => p.uv_translate = [numbers.first().copied().unwrap_or(0.0), numbers.get(1).copied().unwrap_or(0.0)],
            6 => p.uv_scale = [numbers.first().copied().unwrap_or(1.0), numbers.get(1).copied().unwrap_or(1.0)],
            11 => p.life = numbers.first().copied().unwrap_or(1.0),
            _ => {}
        }
    }
    for rule in effect.initializers(layer) {
        let target = rule.target;
        match rule.rule {
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

/// Goes through the frames of a particle, from its birth to `age` frames after, with the updaters of the layer. `id` makes the random
/// numbers of the particle.
fn age_particle(effect: &Effect, layer: usize, p: &mut Streams, age: u32, rng: &mut Rng, id: [u64; 3]) {
    let updaters = effect.updaters(layer);
    let seed = |k: usize| [id[0], id[1], id[2], 0x4321 + k as u64];
    for step in 1..=age {
        for (k, rule) in updaters.iter().enumerate() {
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
                Updater::Random { power, .. } => {
                    let noise = [rng.signed_one() * power[0], rng.signed_one() * power[1], rng.signed_one() * power[2]];
                    if let Some(stream) = p.vector(target) {
                        add(stream, noise, 1.0);
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
                _ => {}
            }
        }
    }
    // The rules that set a stream from the part of the life that has passed.
    let life_part = (age as f32 / p.life).clamp(0.0, 1.0);
    for (k, rule) in updaters.iter().enumerate() {
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
                if target == 4 {
                    p.alpha = value;
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
}

/// The longest life of a layer, in frames.
fn longest_life(effect: &Effect, layer: usize) -> f32 {
    effect.initializers(layer).iter().filter(|r| r.target == 11).map(|r| match r.rule {
        Initializer::FloatRange { max, min } => max.max(min),
        Initializer::FloatImmediate { value } => value,
        Initializer::Float { base, random } => base * (1.0 + random),
        _ => 0.0,
    }).fold(1.0, f32::max)
}

/// The frames that an effect lasts: the end of the last particle.
pub fn length(effect: &Effect) -> f32 {
    (0..effect.layer_count())
        .map(|layer| {
            let em = effect.emission(layer);
            em.start as f32 + if em.infinite { ENDLESS_FRAMES } else { em.span.max(0) as f32 } + longest_life(effect, layer) + 1.0
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
    let mut out = Vec::new();
    let now = frame.max(0.0).floor() as u32;
    for layer in 0..effect.layer_count() {
        let shape = Shape::of(effect, layer);
        let place = effect.emitter_position(layer);
        for (born, count) in births(effect, layer, now) {
            for n in 0..count {
                let mut rng = Rng::new([layer as u64, born as u64, n as u64, 0x9A5]);
                let mut p = new_particle(effect, layer, &shape, &mut rng);
                let age = now - born;
                if age as f32 >= p.life {
                    continue;
                }
                age_particle(effect, layer, &mut p, age, &mut rng, [layer as u64, born as u64, n as u64]);
                out.push(Particle {
                    layer,
                    position: [p.translate[0] + place[0], p.translate[1] + place[1], p.translate[2] + place[2]],
                    // The size of a particle is its half size: the game puts the corners at plus and minus the size.
                    size: [2.0 * p.scale[0] * p.scale_ext[0], 2.0 * p.scale[1] * p.scale_ext[1]],
                    color: p.color,
                    alpha: p.alpha,
                    angle: p.rotate[2],
                    uv_translate: p.uv_translate,
                    uv_scale: p.uv_scale,
                });
                if out.len() >= MAX_PARTICLES {
                    return out;
                }
            }
        }
    }
    out
}

/// The picture that a layer uses: the one that its material names, else the picture whose name is in the name of its set or of its emitter,
/// else the picture of the same place.
fn picture_of(effect: &Effect, layer: usize, material: Option<&crate::effect_file::Material>) -> Option<usize> {
    if let Some(found) = material.and_then(|m| m.picture) {
        return Some(found);
    }
    let (set, emitter) = (effect.set_name(layer).to_lowercase(), effect.emitter_name(layer).to_lowercase());
    let found = effect.pictures().iter().position(|p| {
        let name = p.name.to_lowercase();
        !name.is_empty() && (set.contains(&name) || emitter.contains(&name) || name.contains(&set) && !set.is_empty())
    });
    found.or_else(|| (!effect.pictures().is_empty()).then(|| layer.min(effect.pictures().len() - 1)))
}

/// How each layer of an effect looks.
pub fn looks(effect: &Effect) -> Vec<Look> {
    (0..effect.layer_count())
        .map(|layer| {
            let material = effect.material(layer);
            let picture = picture_of(effect, layer, material.as_ref()).and_then(|index| {
                let info = effect.pictures().get(index)?;
                Some(Picture { width: info.width, height: info.height, rgba: effect.picture_rgba(index)? })
            });
            Look {
                picture,
                // A layer without material (it should not happen) is added, like most of the light effects.
                additive: material.as_ref().map_or(true, |m| m.additive),
                uv_scale: material.as_ref().map_or([1.0, 1.0], |m| m.scale),
                uv_translate: material.as_ref().map_or([0.0, 0.0], |m| m.translate),
                wrap: material.as_ref().map_or([2, 2], |m| m.wrap),
                mode: material.as_ref().map_or(2, |m| m.mode),
                builder: effect.builder(layer),
            }
        })
        .collect()
}

/// The radius of the area that the preview shows with the default point of view: the farthest particle at some frames, with its size.
pub fn extent(effect: &Effect) -> f32 {
    let end = length(effect);
    let camera = Camera::default();
    let (sin_y, cos_y, sin_p, cos_p) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    let mut radius = 1.0f32;
    for step in 0..12 {
        for p in particles_at(effect, end * step as f32 / 12.0) {
            let (x, y, z) = (p.position[0], p.position[1], p.position[2]);
            let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
            let y2 = y * cos_p - z1 * sin_p;
            radius = radius.max((x1 * x1 + y2 * y2).sqrt() + p.size[0].max(p.size[1]) * 0.5);
        }
    }
    radius * 1.1
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
    let mut list: Vec<(Particle, &Look)> = Vec::new();
    for item in items {
        for mut p in particles_at(item.effect, item.frame) {
            if let Some(look) = item.looks.get(p.layer) {
                for k in 0..3 {
                    p.position[k] += item.origin[k];
                }
                list.push((p, look));
            }
        }
    }
    // The particles that are mixed with their transparency are drawn from the far to the near.
    list.sort_by(|a, b| project(a.0.position).2.partial_cmp(&project(b.0.position).2).unwrap_or(std::cmp::Ordering::Equal).reverse());
    for (p, look) in list {
        let (cx, cy, _) = project(p.position);
        let (cx, cy) = (centre + cx, centre - cy);
        let (width, height) = (p.size[0].abs(), p.size[1].abs());
        let (sin_a, cos_a) = p.angle.sin_cos();
        // The two sides of the picture on the screen (right, up), by the way that the game draws the particle.
        let ((rx, ry), (ux, uy)) = match look.builder {
            Builder::Billboard | Builder::WorldBillboard => ((cos_a * width * scale, sin_a * width * scale), (-sin_a * height * scale, cos_a * height * scale)),
            Builder::UpBillboard => ((width * scale, 0.0), (0.0, height * scale * cos_p)),
            Builder::Plane => {
                let (r, u) = (project([cos_a * width, sin_a * width, 0.0]), project([-sin_a * height, cos_a * height, 0.0]));
                ((r.0, r.1), (u.0, u.1))
            }
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
        for py in y_from..y_to {
            for px in x_from..x_to {
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                // The point of the picture under the pixel.
                let (a, b) = ((dx * fy - dy * fx) / det, (ex * dy - ey * dx) / det);
                if !(-0.5..0.5).contains(&a) || !(-0.5..0.5).contains(&b) {
                    continue;
                }
                let (u, v) = (a + 0.5, 0.5 - b);
                let texel = match &look.picture {
                    Some(picture) => {
                        // The coordinates of the picture: the ones of the material, then the ones of the particle; the picture repeats.
                        // The coordinates turn around the middle of the picture: the translation is counted in widths of the particle, then the scale.
                        let (u, v) = ((u - 0.5 - p.uv_translate[0]) * p.uv_scale[0] + 0.5, (v - 0.5 - p.uv_translate[1]) * p.uv_scale[1] + 0.5);
                        // The coordinates of the material: scale × (coordinate - translation).
                        // The matrix of the material works on the coordinates of the chip (the V axis goes up).
                        let v = 1.0 - v;
                        let (u, v) = if look.mode == 2 {
                            ((u - look.uv_translate[0] - 0.5) * look.uv_scale[0] + 0.5, (v + look.uv_translate[1] - 0.5) * look.uv_scale[1] + 0.5)
                        } else {
                            ((u - look.uv_translate[0]) * look.uv_scale[0], (v - look.uv_translate[1]) * look.uv_scale[1])
                        };
                        let v = 1.0 - v;
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
                        let (Some(u), Some(v)) = (wrap(u, look.wrap[0]), wrap(v, look.wrap[1])) else { continue };
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
                            [picture.rgba[at], picture.rgba[at + 1], picture.rgba[at + 2], picture.rgba[at + 3]].map(|c| c as f32 / 255.0)
                        };
                        let (p00, p10, p01, p11) = (texel_at(x0, y0), texel_at(x0 + 1.0, y0), texel_at(x0, y0 + 1.0), texel_at(x0 + 1.0, y0 + 1.0));
                        let mut t = [0.0f32; 4];
                        for k in 0..4 {
                            t[k] = (p00[k] * (1.0 - ax) + p10[k] * ax) * (1.0 - ay) + (p01[k] * (1.0 - ax) + p11[k] * ax) * ay;
                        }
                        (t[0], t[1], t[2], t[3])
                    }
                    // A particle without a picture is a soft round spot.
                    None => {
                        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0;
                        let soft = (1.0 - d).clamp(0.0, 1.0);
                        (soft, soft, soft, soft)
                    }
                };
                let source = [texel.0 * p.color[0], texel.1 * p.color[1], texel.2 * p.color[2]].map(|c| c.clamp(0.0, 4.0));
                let strength = (texel.3 * p.alpha).clamp(0.0, 1.0);
                let pixel = &mut canvas[py * side + px];
                for k in 0..3 {
                    pixel[k] = if look.additive { (pixel[k] + source[k] * strength * 255.0).min(255.0) } else { pixel[k] * (1.0 - strength) + (source[k] * 255.0).min(255.0) * strength };
                }
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
