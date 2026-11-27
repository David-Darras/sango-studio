//! What a motion changes besides the bones: the meshes that it shows and hides, and the places of the pictures and the
//! colours of the materials (the eyes that close, the flames that move).
//!
//! A motion file of a Pokémon is a bundle. Its part 0 holds the motions of the bones (see [`crate::motion`]). Then, after
//! the part 1, come four blocks of `n` parts, one part for each motion of the game (18 in battle, 26 in Pokémon-Amie; the
//! last part of the last block can be missing): the shown meshes, the places of the pictures, the colours, and the
//! sounds. So the part `2 + i` holds the shown meshes of the motion `i`, the part `2 + n + i` the places of its pictures
//! and the part `2 + 2n + i` its colours. A part is empty or it is a model file (BCH) with one animation.
//!
//! The face file of a model (the one after the model) holds the animations that the game runs by itself, each one in
//! three parts like the ones above: the blinks, the mouths and, from the part 18, an animation that goes round and round
//! over every motion (the flames and the gas that move).
//!
//! The content of such a file starts with its tables, 12 bytes each (the place of a list of places, the number of
//! entries, the names): the table 9 holds the animations of materials and the table 10 the ones of shown meshes.
//!
//! **An animation** has its name, flags, its number of frames at 8, then the list of its elements (0x0c) and their
//! number (0x10). **An element** has the name of what it changes, the kind of the change (u16 at 4), the kind of its
//! values (u16 at 6), then the values:
//!
//! - Shown meshes (kind of values 8): at 0x14 the place of the bits, one for each frame, and at 0x18 their number.
//! - Numbers (kind of values 2 for two numbers, 0 for one): flags (u32), then for each number its value or the place of
//!   its curve. The bit `axis` of the flags says that the number is constant, the bit `8 + axis` that there is none.
//!
//! **A curve** has the way between its keys (u8 at 0x0c), the form of its keys (u8 at 0x0d), their number (u16 at 0x0e),
//! the scale and the start of the values (0x10, 0x14), the scale of the frames (0x18), and the place of the keys (0x20).

use crate::model::{File, Model};

/// The table of the animations of materials and the one of the shown meshes.
const MATERIAL_TABLE: usize = 9;
const SHOWN_TABLE: usize = 10;

/// The kinds of change that the reader knows.
const SHOWN_MESH: u16 = 47;
const FIRST_PLACE: u16 = 19;
const LAST_PLACE: u16 = 27;

/// The kinds of values of an element.
const ONE_NUMBER: u16 = 0;
const TWO_NUMBERS: u16 = 2;
const BITS: u16 = 8;

/// The first part of the endless animation in the face file of a model: its places, its shown meshes and its colours.
const LOOP_PARTS: std::ops::Range<usize> = 18..21;

/// A key of a curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub frame: f32,
    pub value: f32,
    /// The change of the value for one frame, before the key and after it.
    pub slope_in: f32,
    pub slope_out: f32,
}

/// The way from a key to the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// The value of the key stays until the next one.
    Step,
    Line,
    /// A smooth curve that follows the slopes of the two keys.
    Smooth,
}

/// A number that changes with the frames.
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    Constant(f32),
    /// The keys, in the order of their frames. There is at least one.
    Keys(Way, Vec<Key>),
}

impl Curve {
    /// The value at a frame. Before the first key and after the last one, their values stay.
    pub fn at(&self, frame: f32) -> f32 {
        match self {
            Curve::Constant(value) => *value,
            Curve::Keys(way, keys) => {
                let next = keys.partition_point(|key| key.frame <= frame);
                let Some(from) = next.checked_sub(1).map(|k| keys[k]) else { return keys.first().map_or(0.0, |key| key.value) };
                let Some(&to) = keys.get(next) else { return from.value };
                let length = to.frame - from.frame;
                let t = (frame - from.frame) / length;
                match way {
                    Way::Step => from.value,
                    Way::Line => from.value + (to.value - from.value) * t,
                    Way::Smooth => {
                        let (t2, t3) = (t * t, t * t * t);
                        from.value * (2.0 * t3 - 3.0 * t2 + 1.0) + length * from.slope_out * (t3 - 2.0 * t2 + t) + to.value * (3.0 * t2 - 2.0 * t3) + length * to.slope_in * (t3 - t2)
                    }
                }
            }
        }
    }
}

/// What a curve of a material changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    /// The scale of the place of the picture of a unit (0 to 2).
    Scale(usize),
    /// The translation of the place of the picture of a unit.
    Translation(usize),
    /// One of the six colours of the material.
    Colour(usize),
}

/// One number of a material that changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub material: String,
    pub property: Property,
    /// The number of the value: x or y, or red, green, blue and alpha.
    pub axis: usize,
    pub curve: Curve,
}

/// The frames where the meshes of a node are shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub node: String,
    /// One value for each frame. After the last one, it stays. There is at least one.
    pub frames: Vec<bool>,
}

/// What a motion changes besides the bones.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Look {
    pub shown: Vec<Shown>,
    pub tracks: Vec<Track>,
    /// The number of frames of the longest animation that was read.
    pub length: f32,
}

impl Look {
    /// The look of the motion `index` of a motion file, from the parts of the file. A part that the reader does not
    /// understand changes nothing.
    pub fn read(parts: &[Vec<u8>], index: usize) -> Look {
        let mut look = Look::default();
        // The number of parts of a block: the parts after the first two, in four blocks.
        let motions = (parts.len() + 1) / 4;
        if index >= motions {
            return look;
        }
        for block in 0..3 {
            if let Some(part) = parts.get(2 + block * motions + index) {
                // A file that is cut gives what was read before the cut.
                let _ = look.add(part);
            }
        }
        look
    }

    /// The endless animation of a model, from the parts of its face file: empty when the model has none.
    pub fn read_loop(parts: &[Vec<u8>]) -> Look {
        let mut look = Look::default();
        for part in LOOP_PARTS.filter_map(|number| parts.get(number)) {
            let _ = look.add(part);
        }
        look
    }

    /// Adds the changes of one model file of animations.
    pub fn add(&mut self, data: &[u8]) -> Result<(), String> {
        let file = File::new(data)?;
        let content = file.content;
        for table in [MATERIAL_TABLE, SHOWN_TABLE] {
            let (list, count) = (file.u32(content + 12 * table)? as usize, file.u32(content + 12 * table + 4)? as usize);
            for number in 0..count.min(64) {
                let animation = content + file.u32(content + list + 4 * number)? as usize;
                let (elements, length) = (file.u32(animation + 0x0c)? as usize, file.u32(animation + 0x10)? as usize);
                if length > 0 {
                    self.length = self.length.max(file.f32(animation + 8)?);
                }
                for element in 0..length.min(512) {
                    let at = content + file.u32(content + elements + 4 * element)? as usize;
                    self.add_element(&file, at)?;
                }
            }
        }
        Ok(())
    }

    fn add_element(&mut self, file: &File, at: usize) -> Result<(), String> {
        let name = file.name(file.u32(at)?);
        let (change, values) = (file.u16(at + 4)?, file.u16(at + 6)?);
        if values == BITS && change == SHOWN_MESH {
            let (bits, count) = (file.content + file.u32(at + 0x14)? as usize, file.u32(at + 0x18)? as usize);
            let frames = (0..count.min(0x4000)).map(|k| Ok(file.u32(bits + 4 * (k / 32))? >> (k % 32) & 1 != 0)).collect::<Result<Vec<_>, String>>()?;
            if !frames.is_empty() {
                self.shown.push(Shown { node: name, frames });
            }
            return Ok(());
        }
        // The material and the property that the element changes.
        let (material, property, first_axis) = match values {
            // The places of the pictures: three kinds for each unit (the scale, the rotation, the translation).
            TWO_NUMBERS if (FIRST_PLACE..=LAST_PLACE).contains(&change) => {
                let (unit, kind) = (usize::from((change - FIRST_PLACE) / 3), (change - FIRST_PLACE) % 3);
                let property = match kind {
                    0 => Property::Scale(unit),
                    2 => Property::Translation(unit),
                    _ => return Ok(()),
                };
                (name, property, 0)
            }
            // A colour: one element for each of its numbers, named "<material>.Constant<colour>.<x, y, z or w>".
            ONE_NUMBER => {
                let mut words = name.rsplitn(3, '.');
                let (Some(axis), Some(colour), Some(material)) = (words.next(), words.next(), words.next()) else { return Ok(()) };
                let (Some(axis), Some(colour)) = (["x", "y", "z", "w"].iter().position(|&a| a == axis), colour.strip_prefix("Constant").and_then(|n| n.parse::<usize>().ok())) else { return Ok(()) };
                (material.to_string(), Property::Colour(colour), axis)
            }
            _ => return Ok(()),
        };
        let flags = file.u32(at + 8)?;
        let count = if values == TWO_NUMBERS { 2 } else { 1 };
        for axis in 0..count {
            if flags >> (8 + axis) & 1 != 0 {
                continue;
            }
            let word = at + 0x0c + 4 * axis;
            let curve = if flags >> axis & 1 != 0 {
                Curve::Constant(file.f32(word)?)
            } else {
                match read_curve(file, file.content + file.u32(word)? as usize)? {
                    Some(curve) => curve,
                    None => continue,
                }
            };
            self.tracks.push(Track { material: material.clone(), property, axis: first_axis + axis, curve });
        }
        Ok(())
    }

    /// Gives a model the look of a frame: the hidden meshes, then the places of the pictures and the colours of the
    /// materials. The model must be the one of the file, not one that has the look of another frame.
    pub fn apply(&self, model: &mut Model, frame: f32) {
        for shown in &self.shown {
            let place = (frame.max(0.0) as usize).min(shown.frames.len() - 1);
            for mesh in model.meshes.iter_mut().filter(|mesh| mesh.node == shown.node) {
                mesh.hidden = !shown.frames[place];
            }
        }
        for track in &self.tracks {
            let value = track.curve.at(frame);
            for material in model.materials.iter_mut().filter(|material| material.name == track.material) {
                match track.property {
                    Property::Scale(unit) | Property::Translation(unit) => {
                        let scales = matches!(track.property, Property::Scale(_));
                        let Some(changed) = material.units.get_mut(unit) else { continue };
                        let numbers = if scales { &mut changed.scale } else { &mut changed.translation };
                        let Some(number) = numbers.get_mut(track.axis) else { continue };
                        *number = value;
                        // The picture of the plain drawing is the one of this unit.
                        if changed.texture == material.texture {
                            let numbers = if scales { &mut material.texture_scale } else { &mut material.texture_translation };
                            numbers[track.axis] = value;
                        }
                    }
                    Property::Colour(colour) => {
                        let slots = material.colour_slots;
                        for (stage, _) in material.stages.iter_mut().zip(slots).filter(|(_, slot)| usize::from(*slot) == colour) {
                            if let Some(number) = stage.constant.get_mut(track.axis) {
                                *number = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Reads a curve. `None` for a form of keys that the reader does not know.
fn read_curve(file: &File, at: usize) -> Result<Option<Curve>, String> {
    let head = file.u32(at + 0x0c)?;
    let (way, form, count) = (head & 0xff, (head >> 8) & 0xff, (head >> 16) as usize);
    let way = match way {
        0 => Way::Step,
        1 => Way::Line,
        _ => Way::Smooth,
    };
    let (scale, start, frame_scale) = (file.f32(at + 0x10)?, file.f32(at + 0x14)?, file.f32(at + 0x18)?);
    let keys_at = file.content + file.u32(at + 0x20)? as usize;
    let byte = |place: usize| file.data.get(place).map(|&b| u32::from(b)).ok_or_else(|| "A curve is outside the file.".to_string());
    let short = |place: usize| Ok::<u32, String>(byte(place)? | byte(place + 1)? << 8);
    let signed = |value: u32, bits: u32| ((value << (32 - bits)) as i32 >> (32 - bits)) as f32;
    let value_of = |raw: u32| raw as f32 * scale + start;
    let mut keys = Vec::with_capacity(count.min(0x4000));
    for k in 0..count.min(0x4000) {
        let (frame, value, slope_in, slope_out) = match form {
            // Four numbers.
            0 => {
                let p = keys_at + 16 * k;
                (file.f32(p)?, file.f32(p + 4)?, file.f32(p + 8)?, file.f32(p + 12)?)
            }
            // The frame (12 bits) and the value (20 bits), then two slopes of 16 bits.
            1 => {
                let p = keys_at + 8 * k;
                let word = file.u32(p)?;
                ((word & 0xfff) as f32, value_of(word >> 12), signed(short(p + 4)?, 16) / 256.0, signed(short(p + 6)?, 16) / 256.0)
            }
            // The frame (8 bits), the value (16 bits), then two slopes of 12 bits.
            2 => {
                let p = keys_at + 6 * k;
                let slopes = short(p + 3)? | byte(p + 5)? << 16;
                (byte(p)? as f32, value_of(short(p + 1)?), signed(slopes & 0xfff, 12) / 32.0, signed(slopes >> 12, 12) / 32.0)
            }
            // Three numbers: one slope for both sides.
            3 => {
                let p = keys_at + 12 * k;
                let slope = file.f32(p + 8)?;
                (file.f32(p)?, file.f32(p + 4)?, slope, slope)
            }
            // The frame (8 bits), the value and one slope (12 bits each).
            5 => {
                let p = keys_at + 4 * k;
                let word = short(p + 1)? | byte(p + 3)? << 16;
                let slope = signed(word >> 12, 12) / 32.0;
                (byte(p)? as f32, value_of(word & 0xfff), slope, slope)
            }
            _ => return Ok(None),
        };
        keys.push(Key { frame: frame * if matches!(form, 0 | 3) { 1.0 } else { frame_scale }, value, slope_in, slope_out });
    }
    Ok((!keys.is_empty()).then_some(Curve::Keys(way, keys)))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::{Material, Mesh, Stage, Unit};

    #[test]
    fn a_curve_goes_from_a_key_to_the_next_one() {
        let key = |frame: f32, value: f32| Key { frame, value, slope_in: 0.0, slope_out: 0.0 };
        let curve = Curve::Keys(Way::Smooth, vec![key(2.0, 1.0), key(6.0, 3.0)]);
        assert_eq!([curve.at(0.0), curve.at(2.0), curve.at(4.0), curve.at(6.0), curve.at(9.0)], [1.0, 1.0, 2.0, 3.0, 3.0]);
        let step = Curve::Keys(Way::Step, vec![key(2.0, 1.0), key(6.0, 3.0)]);
        assert_eq!(step.at(5.9), 1.0);
        let line = Curve::Keys(Way::Line, vec![key(2.0, 1.0), key(6.0, 3.0)]);
        assert_eq!(line.at(3.0), 1.5);
    }

    #[test]
    fn a_look_hides_meshes_and_changes_materials() {
        let unit = |texture: &str| Unit { texture: texture.to_string(), scale: [1.0; 2], translation: [0.0; 2], wrap: [2, 2] };
        let material = Material { name: "Eye".into(), texture: "eye".into(), units: [unit("eye"), unit("mask"), unit("")], stages: vec![Stage::default(); 6], colour_slots: [0, 3, 0, 3, 0, 5], ..Material::default() };
        let mesh = Mesh { node: "tongue".into(), ..Mesh::default() };
        let mut model = Model { materials: vec![material], meshes: vec![mesh], ..Model::default() };
        let track = |property, axis, value| Track { material: "Eye".into(), property, axis, curve: Curve::Constant(value) };
        let look = Look {
            shown: vec![Shown { node: "tongue".into(), frames: vec![true, false] }],
            tracks: vec![track(Property::Translation(0), 1, 0.5), track(Property::Translation(1), 0, 0.25), track(Property::Colour(3), 2, 1.0)],
            length: 2.0,
        };
        look.apply(&mut model, 7.0);
        assert!(model.meshes[0].hidden);
        let material = &model.materials[0];
        assert_eq!((material.units[0].translation, material.texture_translation, material.units[1].translation), ([0.0, 0.5], [0.0, 0.5], [0.25, 0.0]));
        assert_eq!(material.stages.iter().map(|s| s.constant[2]).collect::<Vec<_>>(), [255, 255, 255, 255, 255, 255]);
        assert_eq!(material.stages[1].constant, [255, 255, 255, 255]);
        look.apply(&mut model, 0.0);
        assert!(!model.meshes[0].hidden);
    }

    /// With the dump of the game: Bulbasaur hides two vines while it waits, and its eyes have a place for each frame.
    #[test]
    fn the_looks_of_the_dump_are_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let archive = crate::Garc::open(&crate::Dump::open(&root).unwrap().archive(8)).unwrap();
        let parts = |file: usize| {
            let bytes = archive.first_file(file).unwrap();
            let bytes = if crate::lz::is_compressed(bytes) { crate::lz::decompress(bytes).unwrap() } else { bytes.to_vec() };
            let bundle = crate::Bundle::parse(bytes).unwrap();
            (0..bundle.resource_count()).map(|i| bundle.resource(i).unwrap().to_vec()).collect::<Vec<_>>()
        };
        // The first model of the first species starts at the file 3.
        let mut model = crate::model::parse_model(&parts(3)[0]).unwrap();
        let battle = parts(3 + 5);
        let look = Look::read(&battle, 0);
        look.apply(&mut model, 0.0);
        let hidden: Vec<&str> = model.meshes.iter().filter(|m| m.hidden).map(|m| m.node.as_str()).collect();
        assert_eq!(hidden, ["pm0001_00_LTutaSkin", "pm0001_00_RTutaSkin"]);
        assert!(look.tracks.iter().any(|t| t.material == "Eye" && t.property == Property::Translation(0)));
        assert!(Look::read_loop(&parts(3 + 1)).tracks.is_empty(), "Bulbasaur has no endless animation");
        // The gas of Gastly (its first model starts at the file given by the list) moves for ever.
        let list = archive.first_file(0).unwrap();
        let gastly = usize::from(u16::from_le_bytes([list[91 * 4], list[91 * 4 + 1]])) * 8 + 3;
        let endless = Look::read_loop(&parts(gastly + 1));
        assert_eq!(endless.length, 768.0);
        let gas = endless.tracks.iter().find(|t| t.material == "FireStenA" && t.property == Property::Translation(0)).unwrap();
        assert!((0..768).step_by(32).any(|f| (gas.curve.at(f as f32) - gas.curve.at(0.0)).abs() > 0.05), "{gas:?}");
    }
}
