//! The animations of a glTF file as motions of the game (GMP): sampling at 30 frames each second, fewer keys, and the slots of a pack.

use crate::gltf::{Path3, Source};
use crate::motion::{base_pose, rot_zyx, skeleton_order, still_motion};
use crate::skeleton::{euler_zyx, Joint};
use std::collections::HashMap;

/// The names of the battle motions, in the order of the file (the motion k is in the slot k + 1 of the pack).
pub const BATTLE_MOTIONS: [&str; 18] = [
    "Idle A",
    "Idle B",
    "Idle C",
    "Entrance 1: jump out",
    "Entrance 1: falling loop",
    "Entrance 1: landing",
    "Entrance 2",
    "Mega Evolution pose",
    "Physical attack 1",
    "Physical attack 2",
    "Physical attack 3",
    "Physical attack 4",
    "Special attack 1",
    "Special attack 2",
    "Special attack 3",
    "Special attack 4",
    "Damage",
    "Faint",
];

/// The names of the Pokémon-Amie motions, in the order of the file.
pub const AMIE_MOTIONS: [&str; 26] = [
    "Idle",
    "React",
    "Turn A",
    "Turn B",
    "Doze off: start",
    "Doze off: loop",
    "Doze off: wake up",
    "Sleep: loop",
    "Sleep: wake up",
    "Dislike",
    "Confused",
    "Happy A",
    "Happy B",
    "Happy C",
    "Habit A",
    "Habit B",
    "Habit C",
    "Habit D",
    "Lonely",
    "Want to play A",
    "Want to play B",
    "Angry",
    "Bite: start",
    "Bite: loop",
    "Bite: end",
    "Touch",
];

/// The name of a motion in the files that Studio exports: the motions of the files hold more motions than the list of the game (they
/// control the loops and the blinks).
pub fn motion_name(battle: bool, index: usize) -> String {
    let names: &[&str] = if battle { &BATTLE_MOTIONS } else { &AMIE_MOTIONS };
    let file = if battle { "Battle" } else { "Pokémon-Amie" };
    match names.get(index) {
        Some(name) => format!("{file} · {name}"),
        None => format!("{file} · control motion {index}"),
    }
}

/// The place of a motion from its name in an exported file (the opposite of `motion_name`).
pub fn motion_index(battle: bool, name: &str) -> Option<usize> {
    let names: &[&str] = if battle { &BATTLE_MOTIONS } else { &AMIE_MOTIONS };
    let file = if battle { "Battle · " } else { "Pokémon-Amie · " };
    let rest = name.strip_prefix(file)?;
    match rest.strip_prefix("control motion ") {
        Some(number) => number.parse().ok(),
        None => names.iter().position(|n| *n == rest),
    }
}

/// The curve kinds that the game knows.
const FC_STATIC_0: u32 = 0;
const FC_STATIC_BASEPOSE: u32 = 1;
const FC_STATIC_DATA: u32 = 5;
/// A smooth curve between key frames, with a slope at each key (the slope is in units for each second).
const FC_SPLINE_SLOPES: u32 = 7;
/// A curve that moves less than this (a position in units of the model, an angle in radians) is held still.
const STATIC_EPS: [f64; 2] = [0.25, 0.01];
/// The frames each second.
pub const FPS: f64 = 30.0;
/// How far (in the units of a curve: a fraction of its range) the curve between the keys may be from the sampled values.
const KEY_TOLERANCE: f64 = 0.002;
/// The smallest error that is kept, whatever the range of the curve (radians or units of the model).
const KEY_FLOOR: f64 = 1e-4;

/// A curve of a bone: a value for all the frames, or a value for each frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    Const(f64),
    Keys(Vec<f64>),
}

impl Curve {
    fn at(&self, frame: usize) -> f64 {
        match self {
            Curve::Const(v) => *v,
            Curve::Keys(values) => values[frame],
        }
    }

    /// A curve from the value of each frame: a constant when the value does not move.
    fn from_column(column: Vec<f64>) -> Curve {
        let (low, high) = column.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        if high - low < 1e-7 {
            Curve::Const(column[0])
        } else {
            Curve::Keys(column)
        }
    }
}

/// The curves of a bone of a motion: the angles of the rotation (X, Y, Z) and the translation.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneCurves {
    pub rotation: [Curve; 3],
    pub translation: [Curve; 3],
}

/// A motion: the number of frames and the curves of each bone, by its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub frames: usize,
    pub bones: HashMap<String, BoneCurves>,
}

/// The value of a channel at a time. The calculation keeps the precision of the tool that wrote the reference files (the times are 32 bits).
fn sample(times: &[f32], values: &[[f64; 4]], width: usize, t: f64, rotation: bool) -> [f64; 4] {
    let t = t as f32;
    if t <= times[0] {
        return values[0];
    }
    if t >= times[times.len() - 1] {
        return values[values.len() - 1];
    }
    let i = times.partition_point(|&x| x <= t) - 1;
    let f = (t - times[i]) / (times[i + 1] - times[i]);
    let (a, mut b) = (values[i], values[i + 1]);
    let mut out = [0.0; 4];
    if !rotation {
        for k in 0..width {
            out[k] = a[k] + (b[k] - a[k]) * f as f64;
        }
        return out;
    }
    let mut d: f64 = (0..4).fold(0.0, |s, k| s + a[k] * b[k]);
    if d < 0.0 {
        b = b.map(|v| -v);
        d = -d;
    }
    let mut r = [0.0; 4];
    if d > 0.9995 {
        for k in 0..4 {
            r[k] = a[k] + (b[k] - a[k]) * f as f64;
        }
    } else {
        let th = d.acos();
        let first = (((1.0f32 - f) * th as f32) as f64).sin();
        let second = ((f * th as f32) as f64).sin();
        let divisor = th.sin();
        for k in 0..4 {
            r[k] = (a[k] * first + b[k] * second) / divisor;
        }
    }
    let norm = r.iter().fold(0.0, |s, v| s + v * v).sqrt();
    r.map(|v| v / norm)
}

fn matrix_from_quat(q: [f64; 4]) -> [[f64; 3]; 3] {
    let [x, y, z, w] = q;
    [[1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)], [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)], [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)]]
}

type Channel<'a> = (&'a [f32], Vec<[f64; 4]>, usize);

/// The motions of a glTF file, for the joints of the skeleton that the game gets.
///
/// `names` gives the name in the game of each glTF joint that was kept (by its place in the skin), `scale` is the factor of the units.
pub fn animations(source: &Source, names: &HashMap<usize, String>, scale: f64) -> Vec<Animation> {
    let node_bone: HashMap<usize, &String> = names.iter().map(|(&k, name)| (source.joints[k], name)).collect();
    let mut result = Vec::new();
    for animation in &source.animations {
        let mut channels: HashMap<&String, (Option<Channel>, Option<Channel>)> = HashMap::new();
        for channel in &animation.channels {
            let Some(&bone) = node_bone.get(&channel.node) else { continue };
            let width = match channel.path {
                Path3::Translation => 3,
                Path3::Rotation => 4,
                Path3::Scale => continue,
            };
            let values: Vec<[f64; 4]> = channel.values.chunks(width).map(|c| std::array::from_fn(|k| if k < width { c[k] as f64 } else { 0.0 })).collect();
            if values.is_empty() || channel.times.is_empty() {
                continue;
            }
            let entry = channels.entry(bone).or_default();
            if width == 3 {
                entry.0 = Some((&channel.times, values, 3));
            } else {
                entry.1 = Some((&channel.times, values, 4));
            }
        }
        let Some(duration) = channels.values().flat_map(|(t, r)| [t, r]).flatten().map(|(times, _, _)| times[times.len() - 1]).reduce(f32::max) else { continue };
        let frames = ((duration as f64 * FPS).round_ties_even() as usize).max(1);
        let mut bones = HashMap::new();
        for (k, name) in names {
            let node = &source.nodes[source.joints[*k]];
            let default_translation: [f64; 3] = node.translation.map(|v| v as f64 * scale);
            let default_rotation: [f64; 4] = node.rotation.map(|v| v as f64);
            let ch = channels.get(name);
            let (mut rotation, mut translation) = ([Vec::new(), Vec::new(), Vec::new()], [Vec::new(), Vec::new(), Vec::new()]);
            for f in 0..=frames {
                let t = f as f64 / FPS;
                let q = match ch.and_then(|c| c.1.as_ref()) {
                    Some((times, values, width)) => sample(times, values, *width, t, true),
                    None => default_rotation,
                };
                let e = euler_zyx(&matrix_from_quat(q));
                let v = match ch.and_then(|c| c.0.as_ref()) {
                    Some((times, values, width)) => {
                        let s = sample(times, values, *width, t, false);
                        [s[0] * scale, s[1] * scale, s[2] * scale]
                    }
                    None => default_translation,
                };
                for axis in 0..3 {
                    rotation[axis].push(e[axis]);
                    translation[axis].push(v[axis]);
                }
            }
            let [r0, r1, r2] = rotation;
            let [t0, t1, t2] = translation;
            bones.insert(name.clone(), BoneCurves { rotation: [Curve::from_column(r0), Curve::from_column(r1), Curve::from_column(r2)], translation: [Curve::from_column(t0), Curve::from_column(t1), Curve::from_column(t2)] });
        }
        let name = animation.name.rsplit('|').next().unwrap_or(&animation.name).to_string();
        result.push(Animation { name, frames, bones });
    }
    result
}

/// The slope of a column at each frame, for each second: the difference between the two neighbours (one side at the ends).
fn slopes_of(column: &[f64]) -> Vec<f64> {
    let n = column.len();
    (0..n)
        .map(|f| {
            let (low, high) = (f.saturating_sub(1), (f + 1).min(n - 1));
            if high == low {
                0.0
            } else {
                (column[high] - column[low]) / (high - low) as f64 * FPS
            }
        })
        .collect()
}

/// The value between two keys of a curve with slopes (the one that the game draws), at the frame `f` between `a` and `b`.
fn hermite(a: usize, b: usize, f: usize, va: f64, vb: f64, sa: f64, sb: f64) -> f64 {
    let span = (b - a) as f64;
    let t = (f - a) as f64 / span;
    let seconds = span / FPS;
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * va + (t3 - 2.0 * t2 + t) * seconds * sa + (-2.0 * t3 + 3.0 * t2) * vb + (t3 - t2) * seconds * sb
}

/// The frames that are worth a key, and the values and slopes of the curve at the first frame, at each key and at the last frame.
///
/// A key is added where the curve between the keys would be farther from the sampled values than the tolerance.
fn reduce_keys(column: &[f64]) -> (Vec<usize>, Vec<(f64, f64)>) {
    let n = column.len() - 1;
    let slopes = slopes_of(column);
    let (low, high) = column.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let tolerance = f64::max(KEY_FLOOR, KEY_TOLERANCE * (high - low));
    let (mut keys, mut a) = (Vec::new(), 0);
    while a < n {
        let mut b = n;
        while b > a + 1 {
            let fits = (a + 1..b).all(|f| (hermite(a, b, f, column[a], column[b], slopes[a], slopes[b]) - column[f]).abs() <= tolerance);
            if fits {
                break;
            }
            b -= 1;
        }
        if b < n {
            keys.push(b);
        }
        a = b;
    }
    if keys.is_empty() && n > 1 {
        keys.push(n / 2);
    }
    let mut values = vec![(column[0], slopes[0])];
    values.extend(keys.iter().map(|&f| (column[f], slopes[f])));
    values.push((column[n], slopes[n]));
    (keys, values)
}

fn unwrap_angle(previous: f64, mut a: f64) -> f64 {
    while a - previous > std::f64::consts::PI {
        a -= 2.0 * std::f64::consts::PI;
    }
    while a - previous < -std::f64::consts::PI {
        a += 2.0 * std::f64::consts::PI;
    }
    a
}

enum Data {
    None,
    Value(f64),
    Spline(Vec<usize>, Vec<(f64, f64)>),
}

/// The bytes of a motion of `frames` frames. A bone without curves keeps its pose of rest.
pub fn write_motion(order: &[(String, Option<usize>)], frames: usize, bones: &HashMap<String, BoneCurves>) -> Vec<u8> {
    let mut curves: Vec<(u32, Data)> = vec![(FC_STATIC_0, Data::None), (FC_STATIC_0, Data::None)];
    for (name, _) in order {
        let Some(bone) = bones.get(name) else {
            curves.extend((0..3).map(|_| (FC_STATIC_BASEPOSE, Data::None)));
            continue;
        };
        // The translation, then the rotation, then the scale (always 1).
        for component in 0..3 {
            if component == 2 {
                curves.extend((0..3).map(|_| (FC_STATIC_DATA, Data::Value(1.0))));
                continue;
            }
            let mut values = vec![[0.0f64; 3]; frames + 1];
            for f in 0..=frames {
                values[f] = if component == 0 {
                    std::array::from_fn(|k| bone.translation[k].at(f))
                } else {
                    let e = euler_zyx(&rot_zyx(bone.rotation[0].at(f), bone.rotation[1].at(f), bone.rotation[2].at(f)));
                    if f > 0 {
                        std::array::from_fn(|k| unwrap_angle(values[f - 1][k], e[k]))
                    } else {
                        e
                    }
                };
            }
            for k in 0..3 {
                let column: Vec<f64> = values.iter().map(|v| v[k]).collect();
                let (low, high) = column.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
                if high - low < STATIC_EPS[component] {
                    curves.push((FC_STATIC_DATA, Data::Value((high + low) / 2.0)));
                } else {
                    let (keys, values) = reduce_keys(&column);
                    curves.push((FC_SPLINE_SLOPES, Data::Spline(keys, values)));
                }
            }
        }
    }
    // The game sizes the curves of a motion for 9 for each bone: the last bone keeps its pose of rest.
    if curves.len() > 9 * order.len() {
        curves.truncate(2 + 9 * (order.len() - 1));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&(curves.len() as u16).to_le_bytes());
    out.extend_from_slice(&(frames as u16).to_le_bytes());
    for group in curves.chunks(8) {
        let mut v = 0u32;
        for (i, (kind, _)) in group.iter().enumerate() {
            v |= kind << (3 * i);
        }
        out.extend_from_slice(&v.to_le_bytes()[..3]);
    }
    let wide = frames > 255;
    if wide && out.len() % 2 != 0 {
        out.push(0);
    }
    for (_, data) in &curves {
        if let Data::Spline(keys, _) = data {
            if wide {
                out.extend_from_slice(&(keys.len() as u16).to_le_bytes());
                keys.iter().for_each(|&k| out.extend_from_slice(&(k as u16).to_le_bytes()));
            } else {
                out.push(keys.len() as u8);
                out.extend(keys.iter().map(|&k| k as u8));
            }
        }
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    for (_, data) in &curves {
        match data {
            Data::Value(v) => out.extend_from_slice(&(*v as f32).to_le_bytes()),
            Data::Spline(_, values) => values.iter().for_each(|(v, slope)| {
                out.extend_from_slice(&(*v as f32).to_le_bytes());
                out.extend_from_slice(&(*slope as f32).to_le_bytes());
            }),
            Data::None => {}
        }
    }
    out
}

/// What the importer does with the motions of a pack: which animation fills which slot of the template.
pub struct Slots<'a> {
    /// The animation for a slot of the template (the slot k is the motion k - 1 of the game), by the name of the animation.
    pub table: &'a HashMap<usize, String>,
    /// The animation for the slots that are not in the table, when it exists.
    pub idle: &'a str,
    /// After the first slot, the slots hold the rest pose (the Pokémon-Amie motions).
    pub still_after_first: bool,
}

/// A motion pack with the slots of the template pack. The first entry is the pose of rest, the other slots that the template uses hold a
/// motion of `animations`, chosen by `slots`; without animation, every slot holds the pose of rest.
pub fn build_pack(template: &[u8], joints: &[Joint], animations: &[Animation], slots: &Slots) -> Result<Vec<u8>, String> {
    let order = skeleton_order(joints);
    let u32_at = |at: usize| -> Result<u32, String> { template.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "The motion pack of the template is cut.".to_string()) };
    let count = u32_at(0)? as usize;
    let offsets: Vec<u32> = (0..=count).map(|i| u32_at(4 + 4 * i)).collect::<Result<_, _>>()?;
    let still = still_motion(&order);
    let by_name = |name: &str| animations.iter().find(|a| a.name == name);
    let pick = |want: Option<&String>| -> Option<&Animation> {
        if animations.is_empty() {
            return None;
        }
        let want = want.map_or(slots.idle, |w| w.as_str());
        by_name(want)
            .or_else(|| animations.iter().find(|a| a.name.contains(want)))
            .or_else(|| by_name(slots.idle))
            .or_else(|| animations.iter().find(|a| a.name.contains("wait")))
            .or_else(|| animations.first())
    };
    let mut entries: Vec<Vec<u8>> = vec![base_pose(joints, &order)];
    let mut written: HashMap<String, Vec<u8>> = HashMap::new();
    let mut first = true;
    for k in 1..count {
        if offsets[k] == 0 {
            entries.push(Vec::new());
            continue;
        }
        if slots.still_after_first && !first {
            entries.push(still.clone());
            continue;
        }
        first = false;
        match pick(slots.table.get(&(k - 1))) {
            None => entries.push(still.clone()),
            Some(animation) => {
                let bytes = written.entry(animation.name.clone()).or_insert_with(|| write_motion(&order, animation.frames, &animation.bones));
                entries.push(bytes.clone());
            }
        }
    }
    let header = 4 + 4 * (count + 1);
    let (mut body, mut starts): (Vec<u8>, Vec<u32>) = (Vec::new(), Vec::new());
    for entry in &entries {
        if entry.is_empty() {
            starts.push(0);
            continue;
        }
        while (header + body.len()) % 4 != 0 {
            body.push(0);
        }
        starts.push((header + body.len()) as u32);
        body.extend_from_slice(entry);
    }
    starts.push((header + body.len()) as u32);
    let mut out = (count as u32).to_le_bytes().to_vec();
    for start in starts {
        out.extend_from_slice(&start.to_le_bytes());
    }
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_curve_between_the_keys_follows_the_samples() {
        // A wave and a hold: the keys must be where the wave bends, and the curve between them must stay near the samples.
        let column: Vec<f64> = (0..=60).map(|f| if f < 40 { (f as f64 / 8.0).sin() } else { (40.0f64 / 8.0).sin() }).collect();
        let (keys, values) = reduce_keys(&column);
        assert!(keys.len() < 30, "the keys are fewer than the frames: {}", keys.len());
        assert_eq!(values.len(), keys.len() + 2);
        let mut places = vec![0];
        places.extend(&keys);
        places.push(60);
        let (low, high) = (-1.0, 1.0);
        let tolerance = f64::max(KEY_FLOOR, KEY_TOLERANCE * (high - low)) * 1.5;
        for pair in 0..places.len() - 1 {
            let (a, b) = (places[pair], places[pair + 1]);
            for f in a + 1..b {
                let value = hermite(a, b, f, values[pair].0, values[pair + 1].0, values[pair].1, values[pair + 1].1);
                assert!((value - column[f]).abs() <= tolerance, "frame {f}: {value} against {}", column[f]);
            }
        }
    }

    #[test]
    fn a_motion_with_slopes_is_read_by_the_reader_of_studio() {
        let order = vec![("Origin".to_string(), None), ("Waist".to_string(), Some(0))];
        let column: Vec<f64> = (0..=30).map(|f| (f as f64 / 5.0).sin()).collect();
        let still = Curve::Const(0.0);
        let bones = HashMap::from([("Waist".to_string(), BoneCurves { rotation: [Curve::Keys(column.clone()), still.clone(), still.clone()], translation: [still.clone(), still.clone(), still.clone()] })]);
        let bytes = write_motion(&order, 30, &bones);
        let frames = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(frames, 30);
        let kinds = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(kinds as usize, 2 + 3 + 9, "two for the locator, three of the pose of rest for the root, nine for the joint");
    }
}
