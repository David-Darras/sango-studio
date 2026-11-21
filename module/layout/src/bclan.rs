//! The `.bclan` file: the animations of a layout (a pane that moves, a color that changes, an image that is swapped).
//!
//! A file is a header and two blocks: `pat1` (the name of the animation and its groups) and `pai1` (the animation itself).
//! All the offsets below are from the start of the thing that holds them.
//!
//! | `pai1` field | Size | Meaning |
//! | --- | --- | --- |
//! | 0x08 | 2 | length of the animation in frames |
//! | 0x0A | 1 | 1 when it loops |
//! | 0x0C | 2 | number of image names |
//! | 0x0E | 2 | number of tracks |
//! | 0x10 | 4 | offset of the list of tracks (then the image names, one offset each) |
//!
//! A **track** is what one pane or one material does: its name (20 bytes), the number of groups (1 byte), a type (1 byte), 2 bytes of
//! padding and the offset of each group. A **group** is a kind of change (4 letters), the number of channels and the offset of each channel.
//! The kinds: `CLPA` the position, rotation, scale and size of a pane; `CLVI` visible or not; `CLVC` the colors of the corners and the
//! alpha; `CLMC` the colors of a material; `CLTS` the matrix of an image; `CLTP` the image shown.
//!
//! A **channel** is one number that changes: its place in the kind (1 byte), the number (1 byte), the curve (1 byte: 0 constant, 1 step,
//! 2 smooth), 1 byte of padding, the number of keys (2 bytes), 2 bytes of padding and the offset of its keys. A smooth key has 12 bytes:
//! the frame, the value and the slope (3 × f32). A step key has 8 bytes: the frame (f32) and the value (u16, then 2 bytes of padding).
//!
//! This code changes the numbers in place: the bytes of everything else stay as they are.

/// A key of a channel.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    /// The place of the frame (f32) in the file.
    frame_at: usize,
    /// The place of the value: an f32 for a smooth key, a u16 for a step key.
    value_at: usize,
    /// The place of the slope (f32) of a smooth key.
    slope_at: Option<usize>,
}

/// One number that changes.
#[derive(Clone, Debug)]
pub struct Channel {
    /// The kind of the group that holds it (`CLMC`...).
    pub kind: [u8; 4],
    /// The number that changes (see [`component_name`]).
    pub component: u8,
    /// 0 constant, 1 step, 2 smooth.
    pub curve: u8,
    pub keys: Vec<Key>,
}

/// What one pane or one material does.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    /// The type of the track: 0 pane, 1 material.
    pub kind: u8,
    pub channels: Vec<Channel>,
}

/// An animation file in memory.
#[derive(Clone, Debug)]
pub struct Bclan {
    bytes: Vec<u8>,
    frame_size_at: usize,
    pub tracks: Vec<Track>,
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize)
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    b.get(at..at + 4).map(|s| f32::from_le_bytes(s.try_into().unwrap())).unwrap_or(0.0)
}

fn cstr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.split(|&b| b == 0).next().unwrap_or(&[])).into_owned()
}

impl Bclan {
    pub fn parse(bytes: &[u8]) -> Result<Bclan, String> {
        let bad = || "The animation file is not valid.".to_string();
        if bytes.len() < 0x14 || &bytes[..4] != b"CLAN" {
            return Err("This is not a BCLAN animation.".into());
        }
        let mut at = u16_at(bytes, 6).ok_or_else(bad)?;
        let mut pai = None;
        for _ in 0..u16_at(bytes, 16).ok_or_else(bad)? {
            let size = u32_at(bytes, at + 4).ok_or_else(bad)?;
            if size < 8 || at + size > bytes.len() {
                return Err(bad());
            }
            if &bytes[at..at + 4] == b"pai1" {
                pai = Some(at);
            }
            at += size;
        }
        let p = pai.ok_or_else(bad)?;
        let count = u16_at(bytes, p + 14).ok_or_else(bad)?;
        let list = p + u32_at(bytes, p + 16).ok_or_else(bad)?;
        let mut tracks = Vec::new();
        for i in 0..count {
            let c = p + u32_at(bytes, list + 4 * i).ok_or_else(bad)?;
            let name = cstr(bytes.get(c..c + 20).ok_or_else(bad)?);
            let groups = *bytes.get(c + 20).ok_or_else(bad)? as usize;
            let kind = *bytes.get(c + 21).ok_or_else(bad)?;
            let mut channels = Vec::new();
            for j in 0..groups {
                let group = c + u32_at(bytes, c + 24 + 4 * j).ok_or_else(bad)?;
                let group_kind: [u8; 4] = bytes.get(group..group + 4).ok_or_else(bad)?.try_into().unwrap();
                let n = *bytes.get(group + 4).ok_or_else(bad)? as usize;
                for k in 0..n {
                    let t = group + u32_at(bytes, group + 8 + 4 * k).ok_or_else(bad)?;
                    let component = *bytes.get(t + 1).ok_or_else(bad)?;
                    let curve = *bytes.get(t + 2).ok_or_else(bad)?;
                    let key_count = u16_at(bytes, t + 4).ok_or_else(bad)?;
                    let first = t + u32_at(bytes, t + 8).ok_or_else(bad)?;
                    let stride = if curve == 2 { 12 } else { 8 };
                    if first + key_count * stride > bytes.len() {
                        return Err(bad());
                    }
                    let keys = (0..key_count)
                        .map(|m| Key { frame_at: first + m * stride, value_at: first + m * stride + 4, slope_at: (curve == 2).then(|| first + m * stride + 8) })
                        .collect();
                    channels.push(Channel { kind: group_kind, component, curve, keys });
                }
            }
            tracks.push(Track { name, kind, channels });
        }
        Ok(Bclan { bytes: bytes.to_vec(), frame_size_at: p + 8, tracks })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// The length of the animation in frames.
    pub fn frames(&self) -> u16 {
        u16_at(&self.bytes, self.frame_size_at).unwrap_or(0) as u16
    }

    pub fn set_frames(&mut self, frames: u16) {
        self.bytes[self.frame_size_at..self.frame_size_at + 2].copy_from_slice(&frames.to_le_bytes());
    }

    pub fn looped(&self) -> bool {
        self.bytes[self.frame_size_at + 2] != 0
    }

    pub fn set_looped(&mut self, on: bool) {
        self.bytes[self.frame_size_at + 2] = on as u8;
    }

    pub fn frame(&self, key: &Key) -> f32 {
        f32_at(&self.bytes, key.frame_at)
    }

    pub fn set_frame(&mut self, key: &Key, frame: f32) {
        self.bytes[key.frame_at..key.frame_at + 4].copy_from_slice(&frame.to_le_bytes());
    }

    /// The value of a key (a step key has a whole number).
    pub fn value(&self, key: &Key, curve: u8) -> f32 {
        if curve == 2 {
            f32_at(&self.bytes, key.value_at)
        } else {
            u16_at(&self.bytes, key.value_at).unwrap_or(0) as f32
        }
    }

    pub fn set_value(&mut self, key: &Key, curve: u8, value: f32) {
        if curve == 2 {
            self.bytes[key.value_at..key.value_at + 4].copy_from_slice(&value.to_le_bytes());
        } else {
            self.bytes[key.value_at..key.value_at + 2].copy_from_slice(&(value.round().clamp(0.0, 65535.0) as u16).to_le_bytes());
        }
    }

    pub fn slope(&self, key: &Key) -> Option<f32> {
        key.slope_at.map(|at| f32_at(&self.bytes, at))
    }

    pub fn set_slope(&mut self, key: &Key, slope: f32) {
        if let Some(at) = key.slope_at {
            self.bytes[at..at + 4].copy_from_slice(&slope.to_le_bytes());
        }
    }
}

/// The name of a kind of change.
pub fn kind_name(kind: &[u8; 4]) -> &'static str {
    match kind {
        b"CLPA" => "Pane (position, rotation, scale, size)",
        b"CLVI" => "Visibility",
        b"CLVC" => "Colors of the pane",
        b"CLMC" => "Colors of the material",
        b"CLTS" => "Matrix of the image",
        b"CLTP" => "Image shown",
        b"CLIM" => "Indirect image",
        _ => "Other",
    }
}

/// The name of the number that a channel changes.
pub fn component_name(kind: &[u8; 4], component: u8) -> String {
    let channel = |c: u8| ["R", "G", "B", "A"][(c % 4) as usize];
    match kind {
        b"CLPA" => ["Position x", "Position y", "Position z", "Rotation x", "Rotation y", "Rotation z", "Scale x", "Scale y", "Width", "Height"]
            .get(component as usize)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Number {component}")),
        b"CLVI" => "Visible".into(),
        b"CLVC" if component == 16 => "Opacity".into(),
        b"CLVC" => format!("Corner {} {}", ["top-left", "top-right", "bottom-left", "bottom-right"][((component / 4) % 4) as usize], channel(component)),
        b"CLMC" => {
            let slot = component / 4;
            if slot == 0 {
                format!("Color {}", channel(component))
            } else {
                format!("Constant color {} {}", slot - 1, channel(component))
            }
        }
        b"CLTS" => ["Image x", "Image y", "Image rotation", "Image scale x", "Image scale y"].get(component as usize).map(|s| s.to_string()).unwrap_or_else(|| format!("Number {component}")),
        b"CLTP" => "Image".into(),
        _ => format!("Number {component}"),
    }
}

/// True for a kind of change whose numbers are colors from 0 to 255 (the page shows a color box for them).
pub fn is_color(kind: &[u8; 4]) -> bool {
    kind == b"CLMC" || kind == b"CLVC"
}
