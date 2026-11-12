//! The 3D models of the Pokémon: the model files (BCH) and their pictures.
//!
//! A model file has five parts. The header at the start gives the place of the others:
//!
//! | Offset | Content |
//! | --- | --- |
//! | 8 | The content: the tables of models, pictures, and so on. |
//! | 12 | The names (text that ends with a 0). |
//! | 16 | The commands of the graphics chip: how to draw each mesh. |
//! | 20 | The raw data: the vertices, the lists of triangles and the pixels of the pictures. |
//! | 28 | The table that says which words of the file hold a place (the game corrects them when it loads the file). |
//!
//! **The first model of the content** (its first 4 bytes are flags) has these fields, from its start:
//!
//! | Offset | Content |
//! | --- | --- |
//! | 0x34 | The materials (an array of 0x2c bytes each), then their number (0x38) and their names (0x3c). |
//! | 0x40 | The meshes: an array of 0x38 bytes each, then their number (0x44). |
//! | 0x70 | The bones: an array of 0x64 bytes each, then their number (0x74). |
//!
//! **A bone** has: flags (u32), the number of its parent (u16, 0xFFFF for the first one), then at 8 its scale,
//! at 20 its rotation (three angles in radians) and at 32 its translation (three floats each), at 0x2c the matrix
//! of its rest pose (12 floats), and at 0x5c the place of its name.
//!
//! **A mesh** has the number of its material (u16), the place of the commands that describe its vertices (0x08, 0x0c),
//! and its parts (0x10, 0x14). A part has at its start a flag, the number of bones that it uses (u16 at 2),
//! then those bones (up to 20 numbers of 16 bits, from 4), and at 0x2c the commands of its triangles.
//! The vertices of a mesh are stored one after the other. Each vertex has these fields, in this order:
//! the position (3 floats), the normal (3 floats), the texture position (2 floats), the tangent (3 floats), then,
//! depending on the mesh: a colour (4 bytes), and the bones that move the vertex with their weights in percent.
//! A bone number of a vertex is a place in the list of bones of its part.

use std::collections::HashMap;

/// A bone of the skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    /// The number of the parent bone. `None` for the first bone.
    pub parent: Option<usize>,
    pub scale: [f32; 3],
    /// The rotation: three angles in radians, around x, y and z.
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
}

/// A vertex.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// The colour of the vertex (red, green, blue, alpha).
    pub color: [u8; 4],
    /// The bones that move the vertex, as places in the skeleton. A weight of 0 means that the slot is not used.
    pub bones: [u16; 4],
    /// The weights of the bones, from 0 to 1.
    pub weights: [f32; 4],
}

/// A material: what a mesh is painted with.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Material {
    pub name: String,
    /// The name of the picture, or an empty text.
    pub texture: String,
    /// How the texture position of the mesh becomes a place in the picture: the scale and the translation.
    /// A picture that holds several frames (the eyes) is cut by this: the place in the picture is
    /// `scale * (position - translation)`.
    pub texture_scale: [f32; 2],
    pub texture_translation: [f32; 2],
    /// What happens to a place outside the picture, horizontally and vertically: 0 the edge colour goes on,
    /// 1 the same with a border colour, 2 the picture repeats, 3 the picture repeats turned over (mirror).
    pub texture_wrap: [u8; 2],
    /// True when the game draws the material with the opacity of its picture: the places of the picture
    /// with no opacity are not drawn, whatever their colour (the iris of the eyes, for example).
    /// The bit comes from the second word of the values of the material.
    pub transparent: bool,
    /// True when the light does not touch the material (the file gives it no reflection): the flames, the gas.
    pub unlit: bool,
    /// The three pictures of the material (a material mixes up to three pictures) and how each one is placed.
    pub units: [Unit; 3],
    /// The steps that mix the pictures, the colour of the vertices and the light into the colour of a pixel.
    /// Empty when the file gives none.
    pub stages: Vec<Stage>,
    /// The colour that the steps can use as a value of their own, from the file (red, green, blue, alpha).
    pub buffer_color: [u8; 4],
}

/// One of the three pictures of a material.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Unit {
    /// The name of the picture, or an empty text when the unit has none.
    pub texture: String,
    pub scale: [f32; 2],
    pub translation: [f32; 2],
    /// See `Material::texture_wrap`.
    pub wrap: [u8; 2],
}

/// One step of the mix that gives the colour of a pixel. The graphics chip has up to six steps, one after the other:
/// each one takes up to three values (a source and an operand for each) and gives a colour and an opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    /// Where each of the three values of the colour comes from: 0 the colour of the vertex, 1 the light of the
    /// surface, 2 the shine of the surface, 3 to 5 the pictures 0 to 2, 13 the value that a step saved, 14 the colour
    /// of the step itself, 15 the result of the step before.
    pub color_source: [u8; 3],
    /// What is taken from a source for the colour: 0 the colour, 1 one minus the colour, 2 the opacity, 3 one minus the
    /// opacity, 4 red, 5 one minus red, 8 green, 9 one minus green, 12 blue, 13 one minus blue.
    pub color_operand: [u8; 3],
    pub alpha_source: [u8; 3],
    /// What is taken from a source for the opacity: 0 the opacity, 1 one minus it, 2 red, 3 one minus red, 4 green,
    /// 5 one minus green, 6 blue, 7 one minus blue.
    pub alpha_operand: [u8; 3],
    /// How the values make the result: 0 the first value, 1 first times second, 2 first plus second, 3 the same minus a
    /// half, 4 a mix of the first and the second by the third, 5 first minus second, 6 and 7 a dot product,
    /// 8 first times second plus third, 9 first plus second, times third.
    pub color_mode: u8,
    pub alpha_mode: u8,
    /// The result is multiplied by 1 (0), 2 (1) or 4 (2).
    pub color_scale: u8,
    pub alpha_scale: u8,
    /// The colour of the step itself, from the file (red, green, blue, alpha).
    pub constant: [u8; 4],
    /// True when the step saves its input colour, or its input opacity, for the later steps.
    pub save_color: bool,
    pub save_alpha: bool,
}

impl Default for Stage {
    /// A step that does nothing: the result is the result of the step before.
    fn default() -> Stage {
        Stage { color_source: [15; 3], color_operand: [0; 3], alpha_source: [15; 3], alpha_operand: [0; 3], color_mode: 0, alpha_mode: 0, color_scale: 0, alpha_scale: 0, constant: [255; 4], save_color: false, save_alpha: false }
    }
}

/// A mesh part: some triangles that use one material.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Mesh {
    /// The place of the material in `Model::materials`.
    pub material: usize,
    pub vertices: Vec<Vertex>,
    /// Three places in `vertices` for each triangle.
    pub triangles: Vec<[u32; 3]>,
}

/// A picture, with the first row at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub name: String,
    pub width: usize,
    pub height: usize,
    /// 4 bytes for each pixel: red, green, blue, alpha.
    pub rgba: Vec<u8>,
}

/// A model: its skeleton, its materials and its meshes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Model {
    pub bones: Vec<Bone>,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
}

/// A model file in memory, with the places of its parts.
struct File<'a> {
    data: &'a [u8],
    content: usize,
    strings: usize,
    commands: usize,
    raw: usize,
    /// The kind of each word of the commands that the table of places marks (0x27 and 0x28 give the width of the
    /// indices), by the place of the word from the start of the commands.
    marked: HashMap<usize, u8>,
}

impl<'a> File<'a> {
    fn new(data: &'a [u8]) -> Result<File<'a>, String> {
        if data.get(..4) != Some(b"BCH\0") {
            return Err("This is not a model file.".into());
        }
        let mut file = File { data, content: 0, strings: 0, commands: 0, raw: 0, marked: HashMap::new() };
        file.content = file.u32(8)? as usize;
        file.strings = file.u32(12)? as usize;
        file.commands = file.u32(16)? as usize;
        file.raw = file.u32(20)? as usize;
        let (table, length) = (file.u32(28)? as usize, file.u32(52)? as usize);
        for entry in 0..length / 4 {
            let value = file.u32(table + 4 * entry)?;
            let kind = (value >> 25) as u8;
            if kind >= 0x20 {
                file.marked.insert((value & 0x01ff_ffff) as usize * 4, kind);
            }
        }
        Ok(file)
    }

    fn u32(&self, at: usize) -> Result<u32, String> {
        self.data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The model file stops at {at:#x}."))
    }

    fn u16(&self, at: usize) -> Result<u16, String> {
        self.data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The model file stops at {at:#x}."))
    }

    fn f32(&self, at: usize) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32(at)?))
    }

    fn vec3(&self, at: usize) -> Result<[f32; 3], String> {
        Ok([self.f32(at)?, self.f32(at + 4)?, self.f32(at + 8)?])
    }

    /// A name: the offset is from the start of the names.
    fn name(&self, offset: u32) -> String {
        let tail = self.data.get(self.strings + offset as usize..).unwrap_or(&[]);
        let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len()).min(80);
        String::from_utf8_lossy(&tail[..end]).into_owned()
    }

    /// The words of a list of commands: the place is from the start of the commands.
    fn words(&self, offset: usize, count: usize) -> Result<Vec<u32>, String> {
        (0..count).map(|i| self.u32(self.commands + offset + 4 * i)).collect()
    }
}

/// The values that a list of commands gives to the registers of the graphics chip, and the place of each value
/// (the number of the word in the list).
fn registers(words: &[u32]) -> HashMap<u16, (u32, usize)> {
    let mut out = HashMap::new();
    let mut at = 0;
    while at + 1 < words.len() {
        let (value, header) = (words[at], words[at + 1]);
        let register = (header & 0xffff) as u16;
        let extra = ((header >> 20) & 0x7ff) as usize;
        let consecutive = header >> 31 != 0;
        if at + 2 + extra > words.len() {
            break;
        }
        out.insert(register, (value, at));
        for k in 0..extra {
            out.insert(if consecutive { register + 1 + k as u16 } else { register }, (words[at + 2 + k], at + 2 + k));
        }
        at += 2 + extra + (extra & 1);
    }
    out
}

/// What an attribute of a vertex is, from its place among the attributes and its format.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Field {
    Position,
    Normal,
    Uv,
    Tangent,
    Color,
    BoneIndex,
    BoneWeight,
}

/// The format of one attribute: the type (0 signed byte, 1 unsigned byte, 2 short, 3 float) and the count.
#[derive(Clone, Copy, Debug)]
struct Format {
    kind: u32,
    count: usize,
}

impl Format {
    fn size(self) -> usize {
        self.count * match self.kind {
            3 => 4,
            2 => 2,
            _ => 1,
        }
    }
}

/// What each attribute of a mesh is: the four first ones are the position, the normal, the texture position and the
/// tangent. The attributes of bytes that follow are a colour, then bone numbers and their weights.
fn fields(formats: &[Format]) -> Vec<Field> {
    let mut out = vec![Field::Position, Field::Normal, Field::Uv, Field::Tangent];
    out.truncate(formats.len());
    match formats.len().saturating_sub(4) {
        1 if formats[4].count == 4 => out.push(Field::Color),
        2 => out.extend([Field::BoneIndex, Field::BoneWeight]),
        3 => out.extend([Field::Color, Field::BoneIndex, Field::BoneWeight]),
        _ => {}
    }
    out.resize(formats.len(), Field::Color);
    out
}

/// Reads the first model of a model file.
pub fn parse_model(data: &[u8]) -> Result<Model, String> {
    let file = File::new(data)?;
    let content = file.content;
    let table = content + file.u32(content)? as usize;
    let model = content + file.u32(table)? as usize;

    // The materials and the name of their picture. A material has 3 places for pictures: a place can hold a picture
    // that the game replaces while it runs, so the first real one is taken.
    let material_count = file.u32(model + 0x38)? as usize;
    let material_table = content + file.u32(model + 0x34)? as usize;
    let name_table = content + file.u32(model + 0x3c)? as usize;
    let mut materials = Vec::with_capacity(material_count);
    for i in 0..material_count {
        let name = file.name(file.u32(name_table + 12 * (i + 1) + 8)?);
        let (unit, texture) = (0..3)
            .filter_map(|unit| file.u32(material_table + 0x2c * i + 0x1c + 4 * unit).ok().map(|offset| (unit, offset)))
            .filter(|&(_, offset)| offset != 0)
            .map(|(unit, offset)| (unit, file.name(offset)))
            .find(|(_, name)| !name.is_empty() && !name.to_lowercase().contains("dummy"))
            .unwrap_or((0, String::new()));
        // The values of the material: at its start the place of a block of 0x110 bytes, where the three
        // texture coordinates start at 0x0c (24 bytes each: 4 of flags, the scale, the rotation, the translation).
        let block = content + file.u32(material_table + 0x2c * i)? as usize + 0x0c + 24 * unit;
        let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
        let (scale, translation) = match (file.vec3(block + 4), file.f32(block + 16), file.f32(block + 20)) {
            (Ok(s), Ok(tx), Ok(ty)) => ([finite(s[0]), finite(s[1])], [finite(tx), finite(ty)]),
            _ => ([1.0, 1.0], [0.0, 0.0]),
        };
        // The way the picture is used is in a block of 16 bytes for each of the 3 pictures: the byte 1 is the wrap
        // horizontally and the byte 2 vertically.
        let sampler = content + file.u32(material_table + 0x2c * i + 0x18)? as usize + 16 * unit;
        let wrap = match (file.data.get(sampler + 1), file.data.get(sampler + 2)) {
            (Some(&s), Some(&t)) => [s, t],
            _ => [2, 2],
        };
        // The second word of the values holds the flags of the material. The bit marks the materials
        // that the game draws with the opacity of their picture (the iris of the eyes).
        let flags = file.u32(content + file.u32(material_table + 0x2c * i)? as usize + 4).unwrap_or(0);
        let transparent = flags & 0x0400_0000 != 0;
        let params = content + file.u32(material_table + 0x2c * i)? as usize;
        let units = std::array::from_fn(|unit| {
            let offset = file.u32(material_table + 0x2c * i + 0x1c + 4 * unit).unwrap_or(0);
            let name = if offset == 0 { String::new() } else { file.name(offset) };
            let transform = params + 0x0c + 24 * unit;
            let (scale, translation) = match (file.vec3(transform + 4), file.f32(transform + 16), file.f32(transform + 20)) {
                (Ok(s), Ok(tx), Ok(ty)) => ([finite(s[0]), finite(s[1])], [finite(tx), finite(ty)]),
                _ => ([1.0, 1.0], [0.0, 0.0]),
            };
            let sampler = content + file.u32(material_table + 0x2c * i + 0x18).unwrap_or(0) as usize + 16 * unit;
            let wrap = match (file.data.get(sampler + 1), file.data.get(sampler + 2)) {
                (Some(&s), Some(&t)) => [s, t],
                _ => [2, 2],
            };
            Unit { texture: name, scale, translation, wrap }
        });
        // The steps of the mix: a list of commands of the graphics chip, at 0xc8 of the values of the material (the
        // place, from the start of the commands, then the length in bytes).
        let (stages, buffer_color) = match (file.u32(params + 0xc8), file.u32(params + 0xcc)) {
            (Ok(place), Ok(length)) => file.words(place as usize, length as usize / 4).map(|words| read_stages(&registers(&words))).unwrap_or_default(),
            _ => Default::default(),
        };
        materials.push(Material { name, texture, texture_scale: scale, texture_translation: translation, texture_wrap: wrap, transparent, unlit: flags & 0x0010_0000 == 0, units, stages, buffer_color });
    }

    // The skeleton.
    let bone_table = content + file.u32(model + 0x70)? as usize;
    let bone_count = file.u32(model + 0x74)? as usize;
    let mut bones = Vec::with_capacity(bone_count);
    for i in 0..bone_count {
        let at = bone_table + 0x64 * i;
        let parent = file.u16(at + 4)?;
        bones.push(Bone {
            name: file.name(file.u32(at + 0x5c)?),
            parent: (parent != 0xFFFF).then_some(parent as usize),
            scale: file.vec3(at + 8)?,
            rotation: file.vec3(at + 20)?,
            translation: file.vec3(at + 32)?,
        });
    }

    // The meshes.
    let mesh_table = content + file.u32(model + 0x40)? as usize;
    let mesh_count = file.u32(model + 0x44)? as usize;
    let mut meshes = Vec::new();
    for i in 0..mesh_count {
        let at = mesh_table + 0x38 * i;
        let material = (file.u32(at)? & 0xffff) as usize;
        let parts = file.u32(at + 20)? as usize;
        let vertex_registers = registers(&file.words(file.u32(at + 8)? as usize, file.u32(at + 12)? as usize)?);
        let get = |register: u16| vertex_registers.get(&register).map(|(value, _)| *value).ok_or(format!("The mesh {i} has no register {register:#x}."));
        let (low, high, buffer, order, info) = (get(0x201)?, get(0x202)?, get(0x203)? as usize, get(0x204)?, get(0x205)?);
        let formats: Vec<Format> = (0..12).map(|k| if k < 8 { (low >> (4 * k)) & 0xf } else { (high >> (4 * (k - 8))) & 0xf }).map(|f| Format { kind: f & 3, count: ((f >> 2) + 1) as usize }).collect();
        let slots: Vec<Format> = (0..(info >> 28) as usize).map(|k| formats[((order >> (4 * k)) & 0xf) as usize]).collect();
        let stride = ((info >> 16) & 0xff) as usize;
        let kinds = fields(&slots);
        for part in 0..parts.clamp(1, 16) {
            let header = content + file.u32(at + 16)? as usize + 0x34 * part;
            let bone_slots = (file.u16(header + 2)? as usize).min(20);
            let palette: Vec<u16> = (0..bone_slots).map(|k| file.u16(header + 4 + 2 * k)).collect::<Result<_, _>>()?;
            let (commands, words) = (file.u32(header + 0x2c)? as usize, file.u32(header + 0x30)? as usize);
            let registers = registers(&file.words(commands, words)?);
            let (Some(&(start, start_word)), Some(&(count, _))) = (registers.get(&0x227), registers.get(&0x228)) else { continue };
            let width = match file.marked.get(&(commands + 4 * start_word)) {
                Some(0x27) => 2,
                Some(0x28) => 1,
                _ => 2,
            };
            let mode = registers.get(&0x25e).map_or(0, |(value, _)| (value >> 8) & 3);
            let (start, count) = ((start & 0x7fff_ffff) as usize, count as usize);
            let index_at = file.raw + start;
            let index = |k: usize| -> Result<u32, String> {
                let bytes = file.data.get(index_at + width * k..index_at + width * (k + 1)).ok_or("An index is outside the file.")?;
                Ok(if width == 2 { u16::from_le_bytes([bytes[0], bytes[1]]) as u32 } else { bytes[0] as u32 })
            };
            let indexes: Vec<u32> = (0..count).map(index).collect::<Result<_, _>>()?;
            // The vertices of this part: each place that the triangles use, in the order of the first use.
            let mut mesh = Mesh { material, ..Mesh::default() };
            let mut place: HashMap<u32, u32> = HashMap::new();
            let mut local = |vertex: u32, mesh: &mut Mesh| -> Result<u32, String> {
                if let Some(&found) = place.get(&vertex) {
                    return Ok(found);
                }
                let read = read_vertex(&file, buffer + vertex as usize * stride, &slots, &kinds, &palette)?;
                mesh.vertices.push(read);
                place.insert(vertex, mesh.vertices.len() as u32 - 1);
                Ok(mesh.vertices.len() as u32 - 1)
            };
            let triangle_places: Vec<[u32; 3]> = match mode {
                1 => (0..indexes.len().saturating_sub(2)).map(|k| if k % 2 == 0 { [indexes[k], indexes[k + 1], indexes[k + 2]] } else { [indexes[k + 1], indexes[k], indexes[k + 2]] }).collect(),
                2 => (1..indexes.len().saturating_sub(1)).map(|k| [indexes[0], indexes[k], indexes[k + 1]]).collect(),
                _ => indexes.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            };
            for corners in triangle_places {
                // A triangle with two equal corners is a gap in a strip.
                if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
                    continue;
                }
                let triangle = [local(corners[0], &mut mesh)?, local(corners[1], &mut mesh)?, local(corners[2], &mut mesh)?];
                mesh.triangles.push(triangle);
            }
            meshes.push(mesh);
        }
    }
    Ok(Model { bones, materials, meshes })
}

/// The steps of the mix of a material, from the registers of the graphics chip that its commands set. The six steps have
/// their registers at 0xc0, 0xc8, 0xd0, 0xd8, 0xf0 and 0xf8: the sources, the operands, the modes, the colour and the
/// scale. The register 0xe0 tells which steps save their input, and 0xfd gives the colour of the saved value.
fn read_stages(registers: &HashMap<u16, (u32, usize)>) -> (Vec<Stage>, [u8; 4]) {
    let value = |register: u16| registers.get(&register).map(|(value, _)| *value);
    let rgba = |word: u32| [word as u8, (word >> 8) as u8, (word >> 16) as u8, (word >> 24) as u8];
    if value(0xc0).is_none() {
        return (Vec::new(), [255; 4]);
    }
    let saves = value(0xe0).unwrap_or(0);
    let mut stages = Vec::new();
    for (number, base) in [0xc0u16, 0xc8, 0xd0, 0xd8, 0xf0, 0xf8].into_iter().enumerate() {
        let Some(source) = value(base) else {
            stages.push(Stage::default());
            continue;
        };
        let (operand, mode, constant, scale) = (value(base + 1).unwrap_or(0), value(base + 2).unwrap_or(0), value(base + 3).unwrap_or(0xffff_ffff), value(base + 4).unwrap_or(0));
        stages.push(Stage {
            color_source: std::array::from_fn(|k| ((source >> (4 * k)) & 0xf) as u8),
            color_operand: std::array::from_fn(|k| ((operand >> (4 * k)) & 0xf) as u8),
            alpha_source: std::array::from_fn(|k| ((source >> (16 + 4 * k)) & 0xf) as u8),
            alpha_operand: std::array::from_fn(|k| ((operand >> (12 + 4 * k)) & 7) as u8),
            color_mode: (mode & 0xf) as u8,
            alpha_mode: ((mode >> 16) & 0xf) as u8,
            color_scale: (scale & 3) as u8,
            alpha_scale: ((scale >> 16) & 3) as u8,
            constant: rgba(constant),
            save_color: (1..=4).contains(&number) && saves & (0x100 << (number - 1)) != 0,
            save_alpha: (1..=4).contains(&number) && saves & (0x1000 << (number - 1)) != 0,
        });
    }
    (stages, rgba(value(0xfd).unwrap_or(0xffff_ffff)))
}

/// Reads one vertex. `palette` gives the skeleton place of each bone number of the part.
fn read_vertex(file: &File, start: usize, slots: &[Format], kinds: &[Field], palette: &[u16]) -> Result<Vertex, String> {
    let mut vertex = Vertex { color: [255; 4], weights: [0.0; 4], ..Vertex::default() };
    let mut at = file.raw + start;
    let mut raw_bones = [0u16; 4];
    for (format, kind) in slots.iter().zip(kinds) {
        let mut values = [0f32; 4];
        for (component, value) in values.iter_mut().enumerate().take(format.count.min(4)) {
            let place = at + component * format.size() / format.count;
            *value = match format.kind {
                3 => file.f32(place)?,
                2 => file.u16(place)? as i16 as f32,
                1 => *file.data.get(place).ok_or("A vertex is outside the file.")? as f32,
                _ => *file.data.get(place).ok_or("A vertex is outside the file.")? as i8 as f32,
            };
        }
        at += format.size();
        match kind {
            Field::Position => vertex.position = [values[0], values[1], values[2]],
            Field::Normal => vertex.normal = [values[0], values[1], values[2]],
            Field::Uv => vertex.uv = [values[0], values[1]],
            Field::Tangent => {}
            Field::Color => vertex.color = [values[0] as u8, values[1] as u8, values[2] as u8, values[3] as u8],
            Field::BoneIndex => raw_bones = [values[0] as u16, values[1] as u16, values[2] as u16, values[3] as u16],
            Field::BoneWeight => vertex.weights = [values[0] / 100.0, values[1] / 100.0, values[2] / 100.0, values[3] / 100.0],
        }
    }
    let has_bones = kinds.contains(&Field::BoneWeight);
    for k in 0..4 {
        vertex.bones[k] = palette.get(raw_bones[k] as usize).copied().unwrap_or(0);
    }
    if !has_bones {
        vertex.weights = [1.0, 0.0, 0.0, 0.0];
        vertex.bones = [palette.first().copied().unwrap_or(0), 0, 0, 0];
    }
    Ok(vertex)
}

/// The size in bytes of a picture of the graphics chip: the format, then its width and height.
fn texture_size(format: u32, width: usize, height: usize) -> usize {
    let pixels = width * height;
    match format {
        0 => pixels * 4,
        1 => pixels * 3,
        2..=6 => pixels * 2,
        7..=9 => pixels,
        0xa..=0xc => pixels / 2,
        _ => pixels,
    }
}

/// The pictures of a model file, decoded. A picture with a format that is not known is left out.
pub fn parse_textures(data: &[u8]) -> Result<Vec<Texture>, String> {
    let file = File::new(data)?;
    let table = file.content + file.u32(file.content + 36)? as usize;
    let count = file.u32(file.content + 40)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let description = file.content + file.u32(table + 4 * i)? as usize;
        let words = file.words(file.u32(description)? as usize, file.u32(description + 4)? as usize)?;
        let registers = registers(&words);
        let (Some(&(dimension, _)), Some(&(address, _)), Some(&(kind, _))) = (registers.get(&0x82), registers.get(&0x85), registers.get(&0x8e)) else { continue };
        let (width, height) = ((dimension >> 16) as usize, (dimension & 0xffff) as usize);
        let format = kind & 0xf;
        let start = file.raw + address as usize;
        let Some(bytes) = file.data.get(start..start + texture_size(format, width, height)) else { continue };
        if let Some(rgba) = decode_texture(format, width, height, bytes) {
            out.push(Texture { name: file.name(file.u32(description + 28)?), width, height, rgba });
        }
    }
    Ok(out)
}

/// The modifiers of the ETC1 tables, with their signs.
const ETC1_TABLE: [[i32; 4]; 8] = [[2, 8, -2, -8], [5, 17, -5, -17], [9, 29, -9, -29], [13, 42, -13, -42], [18, 60, -18, -60], [24, 80, -24, -80], [33, 106, -33, -106], [47, 183, -47, -183]];

fn extend5(value: i32) -> i32 {
    (value << 3) | (value >> 2)
}

/// Decodes a block of 4 × 4 pixels of ETC1, row after row.
fn etc1_block(block: u64) -> [[u8; 3]; 16] {
    let differential = (block >> 33) & 1 == 1;
    let flip = (block >> 32) & 1 == 1;
    let tables = [((block >> 37) & 7) as usize, ((block >> 34) & 7) as usize];
    let signed3 = |value: u64| if value >= 4 { value as i32 - 8 } else { value as i32 };
    let (first, second): ([i32; 3], [i32; 3]) = if differential {
        let (r, g, b) = (((block >> 59) & 31) as i32, ((block >> 51) & 31) as i32, ((block >> 43) & 31) as i32);
        let (r2, g2, b2) = ((r + signed3((block >> 56) & 7)).clamp(0, 31), (g + signed3((block >> 48) & 7)).clamp(0, 31), (b + signed3((block >> 40) & 7)).clamp(0, 31));
        ([extend5(r), extend5(g), extend5(b)], [extend5(r2), extend5(g2), extend5(b2)])
    } else {
        let c = |shift: u32| ((block >> shift) & 15) as i32 * 17;
        ([c(60), c(52), c(44)], [c(56), c(48), c(40)])
    };
    let (low, high) = ((block & 0xffff) as u32, ((block >> 16) & 0xffff) as u32);
    let mut out = [[0u8; 3]; 16];
    for x in 0..4 {
        for y in 0..4 {
            let k = x * 4 + y;
            let index = (((high >> k) & 1) << 1 | ((low >> k) & 1)) as usize;
            let in_second = if flip { y >= 2 } else { x >= 2 };
            let (base, table) = if in_second { (second, tables[1]) } else { (first, tables[0]) };
            let modifier = ETC1_TABLE[table][index];
            out[y * 4 + x] = [(base[0] + modifier).clamp(0, 255) as u8, (base[1] + modifier).clamp(0, 255) as u8, (base[2] + modifier).clamp(0, 255) as u8];
        }
    }
    out
}

/// The place of the pixel number `n` of a tile of 8 × 8 pixels: the bits of x and y alternate.
fn morton(n: usize) -> (usize, usize) {
    ((n & 1) | ((n >> 1) & 2) | ((n >> 2) & 4), ((n >> 1) & 1) | ((n >> 2) & 2) | ((n >> 3) & 4))
}

/// A value of `bits` bits as a byte.
fn expand(value: u32, bits: u32) -> u8 {
    (value * 255 / ((1u32 << bits) - 1)) as u8
}

/// Decodes a picture of the graphics chip to RGBA, with the first row at the top. The chip stores a picture in tiles
/// of 8 × 8 pixels, with the first row of tiles at the bottom. `None` for a format that is not known.
fn decode_texture(format: u32, width: usize, height: usize, data: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width % 8 != 0 || height % 8 != 0 || data.len() < texture_size(format, width, height) {
        return None;
    }
    let mut out = vec![0u8; width * height * 4];
    let mut put = |x: usize, y: usize, rgba: [u8; 4]| {
        let at = ((height - 1 - y) * width + x) * 4;
        out[at..at + 4].copy_from_slice(&rgba);
    };
    let (tiles_x, tiles_y) = (width / 8, height / 8);
    let mut at = 0;
    match format {
        0xc | 0xd => {
            let block_size = if format == 0xc { 8 } else { 16 };
            for ty in 0..tiles_y {
                for tx in 0..tiles_x {
                    for n in 0..4 {
                        let chunk = &data[at..at + block_size];
                        at += block_size;
                        let colors = etc1_block(u64::from_le_bytes(chunk[block_size - 8..].try_into().ok()?));
                        let alpha = if format == 0xd { u64::from_le_bytes(chunk[..8].try_into().ok()?) } else { 0 };
                        for py in 0..4 {
                            for px in 0..4 {
                                let c = colors[py * 4 + px];
                                let a = if format == 0xd { expand(((alpha >> (4 * (px * 4 + py))) & 15) as u32, 4) } else { 255 };
                                put(tx * 8 + (n & 1) * 4 + px, ty * 8 + (n >> 1) * 4 + py, [c[0], c[1], c[2], a]);
                            }
                        }
                    }
                }
            }
        }
        _ => {
            let mut nibble = false;
            for ty in 0..tiles_y {
                for tx in 0..tiles_x {
                    for n in 0..64 {
                        let (x, y) = morton(n);
                        let rgba = match format {
                            0 => {
                                let p = &data[at..at + 4];
                                at += 4;
                                [p[3], p[2], p[1], p[0]]
                            }
                            1 => {
                                let p = &data[at..at + 3];
                                at += 3;
                                [p[2], p[1], p[0], 255]
                            }
                            2..=4 => {
                                let v = u16::from_le_bytes([data[at], data[at + 1]]) as u32;
                                at += 2;
                                match format {
                                    2 => [expand(v >> 11, 5), expand((v >> 6) & 31, 5), expand((v >> 1) & 31, 5), if v & 1 == 1 { 255 } else { 0 }],
                                    3 => [expand(v >> 11, 5), expand((v >> 5) & 63, 6), expand(v & 31, 5), 255],
                                    _ => [expand(v >> 12, 4), expand((v >> 8) & 15, 4), expand((v >> 4) & 15, 4), expand(v & 15, 4)],
                                }
                            }
                            5 => {
                                let (a, l) = (data[at], data[at + 1]);
                                at += 2;
                                [l, l, l, a]
                            }
                            7 => {
                                at += 1;
                                [data[at - 1], data[at - 1], data[at - 1], 255]
                            }
                            8 => {
                                at += 1;
                                [255, 255, 255, data[at - 1]]
                            }
                            9 => {
                                let v = data[at];
                                at += 1;
                                let (l, a) = (expand((v >> 4) as u32, 4), expand((v & 15) as u32, 4));
                                [l, l, l, a]
                            }
                            0xa | 0xb => {
                                let byte = data[at];
                                let v = if nibble { byte >> 4 } else { byte & 15 };
                                if nibble {
                                    at += 1;
                                }
                                nibble = !nibble;
                                let e = expand(v as u32, 4);
                                if format == 0xa { [e, e, e, 255] } else { [255, 255, 255, e] }
                            }
                            _ => return None,
                        };
                        put(tx * 8 + x, ty * 8 + y, rgba);
                    }
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_attributes_of_a_vertex_are_named_from_their_formats() {
        let float3 = Format { kind: 3, count: 3 };
        let bytes4 = Format { kind: 1, count: 4 };
        let bytes2 = Format { kind: 1, count: 2 };
        let base = [float3, float3, Format { kind: 3, count: 2 }, float3];
        assert_eq!(fields(&base), [Field::Position, Field::Normal, Field::Uv, Field::Tangent]);
        assert_eq!(fields(&[base.as_slice(), &[bytes4]].concat())[4], Field::Color);
        assert_eq!(fields(&[base.as_slice(), &[bytes2, bytes2]].concat())[4..], [Field::BoneIndex, Field::BoneWeight]);
        assert_eq!(fields(&[base.as_slice(), &[bytes4, bytes4, bytes4]].concat())[4..], [Field::Color, Field::BoneIndex, Field::BoneWeight]);
        assert_eq!((float3.size(), bytes4.size(), bytes2.size()), (12, 4, 2));
    }

    #[test]
    fn a_register_list_gives_values_and_places() {
        // A value for the register 0x201 and one more for the next register (the header says 1 more and consecutive),
        // a word of padding, then one value for 0x204.
        let words = [10, 0x8010_0201, 11, 0, 20, 0x0000_0204];
        let found = registers(&words);
        assert_eq!(found[&0x201], (10, 0));
        assert_eq!(found[&0x202], (11, 2));
        assert_eq!(found[&0x204], (20, 4));
        assert!(!found.contains_key(&0x203));
    }

    #[test]
    fn a_picture_is_decoded_from_tiles() {
        // A picture of 8 × 8 pixels in format 7 (one byte of light for each pixel): the first byte is the bottom left.
        let mut data = vec![0u8; 64];
        data[0] = 200;
        let rgba = decode_texture(7, 8, 8, &data).unwrap();
        assert_eq!(&rgba[(7 * 8) * 4..][..4], &[200, 200, 200, 255]);
        assert_eq!(&rgba[..4], &[0, 0, 0, 255]);
        assert!(decode_texture(7, 7, 8, &data).is_none());
        assert!(decode_texture(99, 8, 8, &data).is_none());
        assert!(decode_texture(7, 8, 8, &data[..10]).is_none());
    }

    #[test]
    fn a_file_that_is_not_a_model_is_refused() {
        assert!(parse_model(b"nope").is_err());
        assert!(parse_textures(&[0; 40]).is_err());
    }
}
