//! A camera animation of the archive of the battle models (a `CGFX` file): the camera of the file, then the curves that
//! move its place, the point that it looks at, its upward direction, its turn and its angle.
//!
//! The parts of the file point at each other with offsets that count from the place of the offset. The two lists used
//! here are the cameras (the list 5 of the `DATA` block) and the camera animations (the list 12). Each animation has
//! members with the name of what they move, and a curve for each number that moves. The curves of the camera files of
//! the game all have one form: key frames with a value and two slopes.

use crate::math::V3;

const CAMERAS: usize = 5;
const CAMERA_ANIMATIONS: usize = 12;
/// The place of the first list of the `DATA` block: pairs of a count and an offset.
const LISTS: usize = 0x1C;
/// A view that looks at a point and turns around its look, and one that has an upward direction.
const AIM_VIEW: u32 = 0x8000_0000;
/// The kinds of the members of an animation.
const NUMBER: u32 = 0;
const VECTOR: u32 = 4;
const PLACEMENT: u32 = 5;
/// A curve that is one value, and a segment of keys with a value and two slopes.
const CONSTANT_CURVE: u32 = 2;
const SPLINE_KEYS: u32 = 8;

#[derive(Clone, Debug, PartialEq)]
enum Curve {
    Fixed(f32),
    /// The frame, the value, the slope before and the slope after of each key.
    Keys(Vec<[f32; 4]>),
}

impl Curve {
    fn at(&self, frame: f32) -> f32 {
        let keys = match self {
            Curve::Fixed(value) => return *value,
            Curve::Keys(keys) => keys,
        };
        let next = keys.partition_point(|key| key[0] <= frame);
        if next == 0 {
            return keys.first().map_or(0.0, |key| key[1]);
        }
        let Some(after) = keys.get(next) else { return keys[next - 1][1] };
        let before = keys[next - 1];
        let (span, passed) = (after[0] - before[0], frame - before[0]);
        let part = passed / span;
        let back = part - 1.0;
        before[1] + part * part * (2.0 * part - 3.0) * (before[1] - after[1]) + passed * back * (back * before[3] + part * after[2])
    }
}

/// The camera of a file at a frame.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub position: V3,
    pub target: V3,
    pub up: V3,
    /// The angle from the top to the bottom of the screen, in degrees.
    pub angle: f32,
}

#[derive(Clone, Debug)]
pub struct CameraClip {
    position: V3,
    target: V3,
    up: V3,
    twist: f32,
    angle: f32,
    /// The number of frames.
    pub frames: f32,
    /// True when the animation starts again at its end.
    pub repeat: bool,
    moves: [Option<Curve>; 11],
}

/// The places of the curves in `CameraClip::moves`.
const MOVE_POSITION: usize = 0;
const MOVE_TARGET: usize = 3;
const MOVE_UP: usize = 6;
const MOVE_TWIST: usize = 9;
const MOVE_ANGLE: usize = 10;

struct File<'a>(&'a [u8]);

impl File<'_> {
    fn word(&self, at: usize) -> Option<u32> {
        self.0.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn number(&self, at: usize) -> Option<f32> {
        self.word(at).map(f32::from_bits).filter(|v| v.is_finite())
    }

    fn vector(&self, at: usize) -> Option<V3> {
        Some([self.number(at)?, self.number(at + 4)?, self.number(at + 8)?])
    }

    /// The place that the offset at `at` points at.
    fn follow(&self, at: usize) -> Option<usize> {
        let offset = self.word(at)?;
        (offset != 0).then(|| at.wrapping_add(offset as i32 as usize))
    }

    fn text(&self, at: usize) -> Option<&str> {
        let tail = self.0.get(at..)?;
        std::str::from_utf8(&tail[..tail.iter().position(|&b| b == 0)?]).ok()
    }

    /// The object of the entry `number` of a list: its header (12 bytes), its root (16), then the entries (16 each).
    fn entry(&self, list: usize, number: usize) -> Option<usize> {
        self.follow(list + 28 + 16 * number + 12)
    }

    fn curve(&self, at: usize) -> Option<Curve> {
        let flags = self.word(at + 0x0C)?;
        if flags & CONSTANT_CURVE != 0 {
            return self.number(at + 0x10).map(Curve::Fixed);
        }
        // The cameras of the game have one segment in each curve.
        let segment = (self.word(at + 0x10)? > 0).then(|| self.follow(at + 0x14)).flatten()?;
        let kind = self.word(segment + 8)?;
        if kind & 1 != 0 {
            return self.number(segment + 0x0C).map(Curve::Fixed);
        }
        if kind != SPLINE_KEYS {
            return None;
        }
        let count = (self.word(segment + 0x0C)? & 0xFFFF) as usize;
        let keys = (0..count)
            .map(|k| {
                let key = segment + 0x14 + 16 * k;
                Some([self.number(key)?, self.number(key + 4)?, self.number(key + 8)?, self.number(key + 12)?])
            })
            .collect::<Option<Vec<_>>>()?;
        (!keys.is_empty()).then_some(Curve::Keys(keys))
    }
}

impl CameraClip {
    /// Reads a file. `None` when it is not a camera with an animation.
    pub fn parse(bytes: &[u8]) -> Option<CameraClip> {
        let file = File(bytes);
        if !bytes.starts_with(b"CGFX") {
            return None;
        }
        let list = |number: usize| (file.word(LISTS + 8 * number)? > 0).then(|| file.follow(LISTS + 8 * number + 4)).flatten();
        let camera = file.entry(list(CAMERAS)?, 0)?;
        let animation = file.entry(list(CAMERA_ANIMATIONS)?, 0)?;
        if bytes.get(camera + 4..camera + 8)? != b"CCAM" || bytes.get(animation..animation + 4)? != b"CANM" {
            return None;
        }
        // The camera of the file: its place, then the pointers to its view and to its projection.
        let position = file.vector(camera + 0x48)?;
        let (view, projection) = (file.follow(camera + 0xBC)?, file.follow(camera + 0xC0)?);
        let target = file.vector(view + 8)?;
        let (up, twist) = if file.word(view)? == AIM_VIEW { ([0.0, 1.0, 0.0], file.number(view + 0x14)?) } else { (file.vector(view + 0x14)?, 0.0) };
        let angle = file.number(projection + 0x10)?;
        let mut clip = CameraClip { position, target, up, twist, angle, frames: file.number(animation + 0x14)?, repeat: file.word(animation + 0x10)? == 1, moves: Default::default() };
        let (count, members) = (file.word(animation + 0x18)? as usize, file.follow(animation + 0x1C)?);
        for number in 0..count.min(16) {
            let member = file.entry(members, number)?;
            let (flags, name, kind) = (file.word(member)?, file.text(file.follow(member + 4)?)?, file.word(member + 8)?);
            // The numbers of a member: the place of the first curve, how many they are, the place of the first number in
            // the member, and the first bits that say a fixed value and an absent one.
            let (first, numbers, slot, fixed, absent) = match (name, kind) {
                // Only the three last numbers of a placement move a camera: its place.
                ("Transform", PLACEMENT) => (MOVE_POSITION, 3, 7, 6, 16),
                ("ViewUpdater.TargetPosition", VECTOR) => (MOVE_TARGET, 3, 0, 0, 3),
                ("ViewUpdater.UpwardVector", VECTOR) => (MOVE_UP, 3, 0, 0, 3),
                ("ViewUpdater.Twist", NUMBER) => (MOVE_TWIST, 1, 0, 0, 1),
                ("ProjectionUpdater.Fovy", NUMBER) => (MOVE_ANGLE, 1, 0, 0, 1),
                _ => continue,
            };
            for k in 0..numbers {
                let at = member + 0x0C + 4 * (slot + k);
                clip.moves[first + k] = if flags >> (absent + slot + k) & 1 != 0 {
                    None
                } else if flags >> (fixed + k) & 1 != 0 && kind != PLACEMENT {
                    file.number(at).map(Curve::Fixed)
                } else {
                    file.follow(at).and_then(|curve| file.curve(curve))
                };
            }
        }
        Some(clip)
    }

    /// The camera at a frame.
    pub fn at(&self, frame: f32) -> Shot {
        let moved = |curve: usize, value: f32| self.moves[curve].as_ref().map_or(value, |c| c.at(frame));
        let vector = |curve: usize, value: V3| std::array::from_fn(|k| moved(curve + k, value[k]));
        let position: V3 = vector(MOVE_POSITION, self.position);
        let target: V3 = vector(MOVE_TARGET, self.target);
        let mut up: V3 = vector(MOVE_UP, self.up);
        let twist = moved(MOVE_TWIST, self.twist);
        if twist != 0.0 {
            // The upward direction turns around the look of the camera.
            let look = crate::math::normalized(crate::math::sub(target, position));
            let side = crate::math::cross(look, up);
            let (sin, cos) = twist.sin_cos();
            up = std::array::from_fn(|k| up[k] * cos + side[k] * sin);
        }
        Shot { position, target, up, angle: moved(MOVE_ANGLE, self.angle).to_degrees() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_curve_goes_through_its_keys() {
        let curve = Curve::Keys(vec![[0.0, 1.0, 0.0, 0.0], [10.0, 5.0, 0.0, 0.0]]);
        assert_eq!(curve.at(0.0), 1.0);
        assert!((curve.at(5.0) - 3.0).abs() < 1e-5);
        assert_eq!(curve.at(10.0), 5.0);
        assert_eq!(curve.at(40.0), 5.0);
        assert!(CameraClip::parse(b"BCH\0nothing").is_none());
    }
}
