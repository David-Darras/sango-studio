//! The motions of the Pokémon: how the bones of a model move (idle, attacks, fainting, Pokémon-Amie).
//!
//! The motions of a species are in one part of two files of the archive 8 (the battle motions and the Pokémon-Amie
//! motions). A part starts with a list of words:
//!
//! | Word | Content |
//! | --- | --- |
//! | 0 | The number of motions. |
//! | 1 | The place of the rest pose. |
//! | 2 and after | The place of each motion (0 when the motion does not exist), then the size of the part. |
//!
//! **The rest pose** has: the number of joints (u8), the number of effectors (u8), then for each joint after the first
//! one its parent (u8), its kind (u8) and its number of children (u8). Then the names of the joints after the first one
//! (each ends with a 0), the number of picture channels (u8) with the kind of each, and, from the next multiple of 4,
//! for each joint a translation (3 floats) and the rotation of the rest pose as a quaternion (4 floats, x y z w) that is
//! the rotation in the model, not the rotation from the parent.
//!
//! **A motion** is a set of curves. It starts with the number of curves (u16) and the number of frames (u16). Each curve
//! has a kind of 3 bits, 8 kinds in 3 bytes: 0 zero, 1 the value of the rest pose, 2 a quarter of a turn, 3 a half turn,
//! 4 three quarters of a turn, 5 a constant that the file gives, 6 a smooth change between key frames, 7 the same with
//! slopes. Then, if the motion has frames over 255 the numbers of frame take 2 bytes, else 1 byte: for each picture
//! channel its keys, then for each curve of kind 6 or 7 its number of key frames and their frames. From the next multiple
//! of 4 come the floats: for each curve of kind 5 one value, for each curve of kind 6 or 7 two more values than key
//! frames (the first one and the last one of the motion are not in the list of frames), and twice as many for kind 7.
//!
//! The values of all the curves, one after the other, give first two numbers for the whole model, then for each joint
//! its translation, its rotation (three angles in radians, turned in the order z, y, x) and its scale. Each of the three
//! is one value that is the largest float when the joint keeps the value of the rest pose, else three values.

use std::f32::consts::{FRAC_PI_2, PI};

/// A joint of the rest pose.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    /// The name. The first joint has none: it is the root of the model.
    pub name: String,
    /// The number of the parent. The first joint has none.
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    /// The rotation in the model (quaternion x, y, z, w).
    pub rotation: [f32; 4],
}

/// What a joint does at one frame. `None` means that the joint keeps the value of the rest pose.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct JointPose {
    pub translation: Option<[f32; 3]>,
    /// Three angles in radians: the rotation is turned around z, then y, then x.
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
}

/// The state of the whole model at one frame.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pose {
    pub joints: Vec<JointPose>,
    /// The value of each picture channel (the state of the eyes or of the mouth), or -1 when the channel does nothing.
    pub channels: Vec<f32>,
}

/// One curve of a motion.
#[derive(Clone, Debug, PartialEq)]
struct Curve {
    kind: u8,
    /// The frames of the keys (kinds 6 and 7).
    frames: Vec<u16>,
    /// The values: for kind 6 one for each key plus two, for kind 7 two for each (the value and the slope).
    values: Vec<f32>,
}

/// The keys of a picture channel.
#[derive(Clone, Debug, PartialEq, Default)]
struct Channel {
    frames: Vec<u16>,
    values: Vec<u8>,
}

/// A motion, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// The number of frames. The game plays 30 frames each second.
    pub frames: u16,
    curves: Vec<Curve>,
    channels: Vec<Channel>,
}

/// All the motions of a part, with the rest pose.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionPack {
    pub joints: Vec<Joint>,
    /// The kind of each picture channel.
    pub channel_kinds: Vec<u8>,
    places: Vec<usize>,
    data: Vec<u8>,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The motion data stops at {at:#x}."))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The motion data stops at {at:#x}."))
}

fn f32_at(data: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(u32_at(data, at)?))
}

impl MotionPack {
    /// Reads the rest pose and the places of the motions.
    pub fn parse(data: &[u8]) -> Result<MotionPack, String> {
        let count = u32_at(data, 0)? as usize;
        if count > 200 {
            return Err("This is not a motion file.".into());
        }
        let base = u32_at(data, 4)? as usize;
        // The last place of the list is the end of the part, not a motion.
        let places = (0..count.saturating_sub(1)).map(|i| u32_at(data, 8 + 4 * i).map(|p| p as usize)).collect::<Result<Vec<_>, _>>()?;
        let joint_count = *data.get(base).ok_or("The rest pose is missing.")? as usize;
        if joint_count == 0 {
            return Err("The rest pose has no joint.".into());
        }
        let mut at = base + 2;
        let mut parents = vec![None];
        for _ in 1..joint_count {
            parents.push(Some(*data.get(at).ok_or("The rest pose stops early.")? as usize));
            at += 3;
        }
        let mut names = vec![String::new()];
        for _ in 1..joint_count {
            let tail = data.get(at..).ok_or("The names stop early.")?;
            let end = tail.iter().position(|&b| b == 0).ok_or("A name has no end.")?;
            names.push(String::from_utf8_lossy(&tail[..end]).into_owned());
            at += end + 1;
        }
        let channel_count = *data.get(at).ok_or("The channels are missing.")? as usize;
        let channel_kinds = data.get(at + 1..at + 1 + channel_count).ok_or("The channels stop early.")?.to_vec();
        at = (at + 1 + channel_count + 3) & !3;
        let mut joints = Vec::new();
        for (i, (name, parent)) in names.into_iter().zip(parents).enumerate() {
            let place = at + 28 * i;
            joints.push(Joint {
                name,
                parent,
                translation: [f32_at(data, place)?, f32_at(data, place + 4)?, f32_at(data, place + 8)?],
                rotation: [f32_at(data, place + 12)?, f32_at(data, place + 16)?, f32_at(data, place + 20)?, f32_at(data, place + 24)?],
            });
        }
        Ok(MotionPack { joints, channel_kinds, places, data: data.to_vec() })
    }

    /// The number of motions that the part can hold (some are empty).
    pub fn len(&self) -> usize {
        self.places.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True when the motion `index` exists.
    pub fn has(&self, index: usize) -> bool {
        self.places.get(index).is_some_and(|&place| place != 0)
    }

    /// Reads the motion `index`.
    pub fn motion(&self, index: usize) -> Result<Motion, String> {
        let place = *self.places.get(index).filter(|&&place| place != 0).ok_or("This motion does not exist.")?;
        parse_motion(&self.data, place, self.channel_kinds.len())
    }
}

#[cfg(test)]
impl MotionPack {
    /// A pack with only a rest pose, for the tests of other modules.
    pub(crate) fn parse_for_test(joints: Vec<Joint>) -> MotionPack {
        MotionPack { joints, channel_kinds: Vec::new(), places: Vec::new(), data: Vec::new() }
    }
}

fn parse_motion(data: &[u8], start: usize, channel_count: usize) -> Result<Motion, String> {
    let curve_count = u16_at(data, start)? as usize;
    let frames = u16_at(data, start + 2)?;
    let mut at = start + 4;
    let mut kinds = Vec::with_capacity(curve_count);
    for _ in 0..curve_count.div_ceil(8) {
        let bytes = data.get(at..at + 3).ok_or("The kinds of the curves stop early.")?;
        let word = bytes[0] as u32 | (bytes[1] as u32) << 8 | (bytes[2] as u32) << 16;
        for k in 0..8 {
            if kinds.len() < curve_count {
                kinds.push(((word >> (3 * k)) & 7) as u8);
            }
        }
        at += 3;
    }
    let wide = frames > 255;
    let number = |at: &mut usize| -> Result<u16, String> {
        if wide {
            *at = (*at + 1) & !1;
            let value = u16_at(data, *at)?;
            *at += 2;
            Ok(value)
        } else {
            let value = *data.get(*at).ok_or("The keys stop early.")? as u16;
            *at += 1;
            Ok(value)
        }
    };
    if wide {
        at = (at + 1) & !1;
    }
    let mut channels = Vec::new();
    for _ in 0..channel_count {
        let sum = number(&mut at)? as usize;
        let mut channel = Channel::default();
        if sum > 0 {
            for _ in 0..sum - 1 {
                channel.frames.push(number(&mut at)?);
            }
            channel.values = data.get(at..at + sum).ok_or("The keys of a channel stop early.")?.to_vec();
            at += sum;
        }
        channels.push(channel);
    }
    let mut curves: Vec<Curve> = kinds.iter().map(|&kind| Curve { kind, frames: Vec::new(), values: Vec::new() }).collect();
    for curve in curves.iter_mut().filter(|c| c.kind >= 6) {
        let sum = number(&mut at)? as usize;
        for _ in 0..sum {
            curve.frames.push(number(&mut at)?);
        }
    }
    at = (at + 3) & !3;
    for curve in curves.iter_mut() {
        let count = match curve.kind {
            5 => 1,
            6 => curve.frames.len() + 2,
            7 => (curve.frames.len() + 2) * 2,
            _ => 0,
        };
        for _ in 0..count {
            curve.values.push(f32_at(data, at)?);
            at += 4;
        }
    }
    Ok(Motion { frames, curves, channels })
}

impl Curve {
    /// The value at a frame. `count` is the number of frames of the motion.
    fn value(&self, frame: f32, count: f32) -> f32 {
        match self.kind {
            0 => 0.0,
            1 => f32::MAX,
            2 => FRAC_PI_2,
            3 => PI,
            4 => PI + FRAC_PI_2,
            5 => self.values.first().copied().unwrap_or(0.0),
            _ => self.spline(frame, count),
        }
    }

    fn spline(&self, frame: f32, count: f32) -> f32 {
        let slopes = self.kind == 7;
        let at = |j: usize| -> f32 { self.values.get(if slopes { j * 2 } else { j }).copied().unwrap_or(0.0) };
        let sum = self.frames.len();
        if frame >= count {
            return at(sum + 1);
        }
        // The key before and after the frame.
        let (mut j, mut before, mut after) = (1usize, 0.0f32, count);
        let mut found = false;
        for (k, &key) in self.frames.iter().enumerate() {
            let key = key as f32;
            if key == frame {
                return at(k + 1);
            }
            if frame < key {
                (j, after, found) = (k + 1, key, true);
                break;
            }
            before = key;
        }
        if !found {
            j = sum + 1;
        }
        let span = after - before;
        if span <= 0.0 {
            return at(j);
        }
        let t = (frame - before) / span;
        if !slopes {
            let (v0, v1) = (at(j - 1), at(j));
            return t * t * (2.0 * t - 3.0) * (v0 - v1) + v0;
        }
        // Hermite, with the slopes in the units of a second.
        let seconds = (frame - before) / 30.0;
        let t3 = t - 1.0;
        let (v0, v1) = (at(j - 1), at(j));
        let (out_slope, in_slope) = (self.values.get((j - 1) * 2 + 1).copied().unwrap_or(0.0), self.values.get(j * 2 + 1).copied().unwrap_or(0.0));
        (t * in_slope + t3 * out_slope) * t3 * seconds + t * t * (2.0 * t3 - 1.0) * (v0 - v1) + v0
    }
}

impl Channel {
    fn value(&self, frame: f32, count: f32) -> f32 {
        let sum = self.values.len();
        if sum == 0 {
            return -1.0;
        }
        if sum == 1 || frame >= count {
            return *self.values.get(if frame >= count { sum - 1 } else { 0 }).unwrap_or(&0) as f32;
        }
        let mut index = 0;
        for (k, &key) in self.frames.iter().enumerate() {
            if frame < key as f32 {
                break;
            }
            index = k + 1;
        }
        self.values[index.min(sum - 1)] as f32
    }
}

impl Motion {
    /// The state of the model at a frame (from 0 to `frames`).
    pub fn pose(&self, joints: usize, frame: f32) -> Pose {
        let count = self.frames as f32;
        let values: Vec<f32> = self.curves.iter().map(|c| c.value(frame, count)).collect();
        // Two numbers for the whole model, then the joints.
        let mut at = 2;
        let mut out = Vec::with_capacity(joints);
        let take = |at: &mut usize, scale: bool| -> Option<[f32; 3]> {
            let first = *values.get(*at)?;
            if first == f32::MAX {
                *at += 1;
                return None;
            }
            let triple = [first, *values.get(*at + 1)?, *values.get(*at + 2)?];
            *at += 3;
            Some(if scale { triple.map(|v| if v == 0.0 { 0.01 } else { v }) } else { triple })
        };
        for _ in 0..joints {
            let translation = take(&mut at, false);
            let rotation = take(&mut at, false);
            let scale = take(&mut at, true);
            out.push(JointPose { translation, rotation, scale });
        }
        Pose { joints: out, channels: self.channels.iter().map(|c| c.value(frame, count)).collect() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_curve_of_kind_6_goes_smoothly_from_key_to_key() {
        // Two key frames, so four values: the first of the motion, the keys, the last of the motion.
        let curve = Curve { kind: 6, frames: vec![10, 20], values: vec![0.0, 1.0, 3.0, 3.0] };
        assert_eq!(curve.value(0.0, 31.0), 0.0);
        assert_eq!(curve.value(10.0, 31.0), 1.0);
        assert_eq!(curve.value(20.0, 31.0), 3.0);
        assert!((curve.value(5.0, 31.0) - 0.5).abs() < 1e-6, "the middle of the first part is the middle of the values");
        assert_eq!(curve.value(40.0, 31.0), 3.0, "after the end: the last value");
    }

    #[test]
    fn the_kinds_without_keys_give_fixed_values() {
        let kind = |kind| Curve { kind, frames: Vec::new(), values: Vec::new() };
        assert_eq!(kind(0).value(3.0, 10.0), 0.0);
        assert_eq!(kind(1).value(3.0, 10.0), f32::MAX);
        assert_eq!(kind(3).value(3.0, 10.0), PI);
    }

    #[test]
    fn the_keys_of_a_channel_are_steps() {
        let channel = Channel { frames: vec![10, 20], values: vec![1, 2, 3] };
        assert_eq!(channel.value(0.0, 30.0), 1.0);
        assert_eq!(channel.value(10.0, 30.0), 2.0);
        assert_eq!(channel.value(19.0, 30.0), 2.0);
        assert_eq!(channel.value(25.0, 30.0), 3.0);
        assert_eq!(Channel::default().value(5.0, 30.0), -1.0);
    }

    #[test]
    fn a_motion_is_read_from_its_bytes() {
        // Three curves of kinds 1, 5 and 6, 20 frames, one key frame.
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_le_bytes());
        data.extend_from_slice(&20u16.to_le_bytes());
        let word: u32 = 1 | 5 << 3 | 6 << 6;
        data.extend_from_slice(&word.to_le_bytes()[..3]);
        data.push(1); // the kind 6 curve has one key frame
        data.push(10); // at frame 10
        while data.len() % 4 != 0 {
            data.push(0);
        }
        for value in [2.5f32, 0.0, 4.0, 4.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let motion = parse_motion(&data, 0, 0).unwrap();
        assert_eq!(motion.frames, 20);
        assert_eq!(motion.curves[1].values, [2.5]);
        assert_eq!(motion.curves[2].frames, [10]);
        assert_eq!(motion.curves[2].value(10.0, 20.0), 4.0);
    }

    /// With the dump of the game: the motions of every species of the test are read, and their first frame is finite.
    #[test]
    fn the_motions_of_the_dump_are_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let archive = crate::Garc::open(&dump.archive(8)).unwrap();
        for first in [3usize, 59, 723, 4243] {
            for offset in [5usize, 6] {
                let bytes = archive.first_file(first + offset).unwrap();
                let bytes = if crate::lz::is_compressed(bytes) { crate::lz::decompress(bytes).unwrap() } else { bytes.to_vec() };
                let bundle = crate::Bundle::parse(bytes).unwrap();
                let part = bundle.resource(0).unwrap();
                let pack = MotionPack::parse(part).unwrap();
                let mut read = 0;
                for index in 0..pack.len() {
                    if !pack.has(index) {
                        continue;
                    }
                    let motion = pack.motion(index).unwrap_or_else(|e| panic!("file {} motion {index}: {e}", first + offset));
                    for frame in [0.0, motion.frames as f32 / 2.0, motion.frames as f32] {
                        let pose = motion.pose(pack.joints.len(), frame);
                        assert_eq!(pose.joints.len(), pack.joints.len());
                        for joint in &pose.joints {
                            for value in joint.translation.iter().chain(&joint.rotation).chain(&joint.scale).flatten() {
                                assert!(value.is_finite(), "file {} motion {index} frame {frame}", first + offset);
                            }
                        }
                    }
                    read += 1;
                }
                eprintln!("file {}: {} joints, {} motions read, {} channels", first + offset, pack.joints.len(), read, pack.channel_kinds.len());
                assert!(read > 0);
            }
        }
    }
}
