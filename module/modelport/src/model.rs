//! The model file (BCH) of a Pokémon: the bones, the meshes, the materials and the lookup tables, written from the draws of the geometry and
//! from templates (models of the game).
//!
//! The materials of the game are lists of GPU commands that cannot be made from nothing. The writer clones, for each material of the
//! source, the material of a template that feeds the same texture units, and changes what belongs to the source: the names of the
//! textures, the texture transforms, the samplers. The structures that the writer fills have the layout of the files of the game.

use crate::bch::{self, Section, Strings};
use crate::geometry::Face;
use crate::pica::{find_word, find_word_wide};
use crate::skeleton::{invert, multiply, Joint, Matrix};
use crate::template::Template;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const MODEL_FLAGS: u32 = 0x2;
const MESH_FLAGS: u32 = 8;
/// The base of the vertex shader booleans when the templates have none.
const VS_BOOL_MAP: u32 = 0x7FFF_6F0A;

fn f32bits(v: f64) -> u32 {
    (v as f32).to_bits()
}

/// A texture unit of a material of the source.
#[derive(Clone, Debug)]
pub struct Unit {
    pub wrap: (u8, u8),
    pub mag: u8,
    pub min: u8,
    pub lod: u8,
    pub border: Option<u32>,
    pub mapping: u32,
}

/// What the writer needs to know of the texture units and of the draw order of a material.
#[derive(Clone, Debug)]
pub struct Layout {
    pub units: BTreeMap<usize, Unit>,
    pub sources: [f64; 4],
    pub priority: u32,
    pub layer: u32,
}

/// How a pixel of a material goes over what is behind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// It takes its place.
    Opaque,
    /// It takes its place, or it is not drawn: the opacity of the picture says which.
    Cut,
    /// It is mixed with it, by the opacity of the picture.
    SeeThrough,
}

/// How a material that a file of Studio describes is drawn: the file says it, so the writer sets the material to it and does
/// not take the settings of the template for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finish {
    pub cover: Cover,
    /// The light does not touch the material: it shows its picture.
    pub unlit: bool,
}

/// A material of the source: the name, the texture of each unit, the transform of each unit (scale x and y, rotation, translation x and
/// y), and how it blends: alpha test, alpha blend and, when known, additive.
#[derive(Clone, Debug)]
pub struct MaterialSpec {
    pub name: String,
    pub slots: [Option<String>; 3],
    pub uv: [Option<[f64; 5]>; 3],
    pub blend: Vec<bool>,
    pub layout: Layout,
    /// `None` for a material whose drawing comes from the template (the files that other programs write).
    pub finish: Option<Finish>,
}

/// A mesh of the model (a node): its name, its bounding box and its draws.
#[derive(Clone, Debug)]
pub struct MeshSpec {
    pub name: String,
    pub aabb: ([f64; 3], [f64; 3]),
    pub faces: Vec<Face>,
}

/// A model to write.
#[derive(Clone, Debug)]
pub struct ModelSpec {
    pub aabb: ([f64; 3], [f64; 3]),
    pub joints: Vec<Joint>,
    pub materials: Vec<MaterialSpec>,
    pub meshes: Vec<MeshSpec>,
}

// ----------------------------------------------------------------------------------------------------------- the helpers

fn rot_zyx(rx: f64, ry: f64, rz: f64) -> [[f64; 3]; 3] {
    let (cx, sx, cy, sy, cz, sz) = (rx.cos(), rx.sin(), ry.cos(), ry.sin(), rz.cos(), rz.sin());
    let rxm = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let rym = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rzm = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mul3(&rzm, &mul3(&rym, &rxm))
}

fn mul3(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).fold(0.0, |sum, k| sum + a[i][k] * b[k][j]);
        }
    }
    out
}

type Matrix34 = [[f64; 4]; 3];

fn local_matrix(joint: &Joint) -> Matrix34 {
    let r = rot_zyx(joint.rot[0], joint.rot[1], joint.rot[2]);
    std::array::from_fn(|i| [r[i][0] * joint.scale[0] as f64, r[i][1] * joint.scale[1] as f64, r[i][2] * joint.scale[2] as f64, joint.trans[i] as f64])
}

fn mul34(a: &Matrix34, b: &Matrix34) -> Matrix34 {
    let mut out = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..4 {
            out[i][j] = (0..3).fold(0.0, |sum, k| sum + a[i][k] * b[k][j]) + if j == 3 { a[i][3] } else { 0.0 };
        }
    }
    out
}

fn inv34(m: &Matrix34) -> Matrix34 {
    let a = |i: usize, j: usize| m[i][j];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0)) + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    let mut inv = [[0.0; 3]; 3];
    inv[0][0] = (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) / det;
    inv[0][1] = (a(0, 2) * a(2, 1) - a(0, 1) * a(2, 2)) / det;
    inv[0][2] = (a(0, 1) * a(1, 2) - a(0, 2) * a(1, 1)) / det;
    inv[1][0] = (a(1, 2) * a(2, 0) - a(1, 0) * a(2, 2)) / det;
    inv[1][1] = (a(0, 0) * a(2, 2) - a(0, 2) * a(2, 0)) / det;
    inv[1][2] = (a(0, 2) * a(1, 0) - a(0, 0) * a(1, 2)) / det;
    inv[2][0] = (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0)) / det;
    inv[2][1] = (a(0, 1) * a(2, 0) - a(0, 0) * a(2, 1)) / det;
    inv[2][2] = (a(0, 0) * a(1, 1) - a(0, 1) * a(1, 0)) / det;
    let t: [f64; 3] = std::array::from_fn(|i| -(0..3).fold(0.0, |sum, k| sum + inv[i][k] * m[k][3]));
    std::array::from_fn(|i| [inv[i][0], inv[i][1], inv[i][2], t[i]])
}

/// Writes the metadata of a map: the entries, then the nodes of its dictionary, then the words. Returns the offset of the map.
fn write_metadata(sec: &mut Section, strs: &mut Strings, entries: &[(String, u32, Vec<u32>)]) -> usize {
    let map_off = sec.tell();
    sec.u32(0);
    sec.u32(entries.len() as u32);
    sec.u32(0);
    let names: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
    let dic = bch::write_dict(sec, strs, &names);
    let ent = sec.tell();
    for (name, kind, words) in entries {
        let at = strs.get(name);
        sec.ptr(1, at);
        sec.u32(kind | ((words.len() as u32) << 16));
        sec.u32(0);
    }
    for (i, (_, _, words)) in entries.iter().enumerate() {
        let data = sec.tell();
        for &w in words {
            sec.u32(w);
        }
        sec.patch_ptr(ent + 12 * i + 8, 0, data as u32);
    }
    sec.patch_ptr(map_off, 0, ent as u32);
    sec.patch_ptr(map_off + 8, 0, dic as u32);
    map_off
}

fn words_of(template: &Template, offset: usize, count: usize) -> Vec<u32> {
    template.cmd(offset, count)
}

/// The registers that a list of commands writes, with their values (a run of registers writes the next ones).
fn registers(words: &[u32]) -> HashMap<u32, u32> {
    sango_core::pica::registers(words).into_iter().map(|(register, (value, _))| (u32::from(register), value)).collect()
}

/// (alpha test, alpha blend) of a template material, from its commands.
fn template_blend(tp: &Template, content: usize) -> (bool, bool) {
    let count = tp.c32(content + 0xCC);
    let words = words_of(tp, tp.c32(content + 0xC8), count);
    let regs = registers(&words);
    (regs.get(&0x104).copied().unwrap_or(0) & 1 != 0, regs.get(&0x101).copied().unwrap_or(0) >> 24 != 0x01)
}

/// Does a template material add its colour to the scene (destination factor one)?
fn template_additive(tp: &Template, content: usize) -> bool {
    let count = tp.c32(content + 0xCC);
    let words = words_of(tp, tp.c32(content + 0xC8), count);
    let mut i = 0;
    while i + 1 < words.len() {
        let header = words[i + 1];
        let extra = ((header >> 20) & 0x7FF) as usize;
        if header & 0xFFFF == 0x101 {
            return (words[i] >> 20) & 0xF == 1 && words[i] >> 24 != 1;
        }
        i += 2 + extra + ((extra + 2) % 2);
    }
    false
}

/// The registers of the six steps that mix the colours of a material.
const STAGES: [u32; 6] = [0xC0, 0xC8, 0xD0, 0xD8, 0xF0, 0xF8];

/// The pictures that the steps of a template material read for the colour (the sources 3 to 5), from its commands. The
/// opacity is not looked at: an opaque material does not use it, and the writer sets it for the others.
fn pictures_read(tp: &Template, content: usize) -> [bool; 3] {
    let regs = registers(&words_of(tp, tp.c32(content + 0xC8), tp.c32(content + 0xCC)));
    let mut read = [false; 3];
    for base in STAGES {
        let (Some(&source), mode) = (regs.get(&base), regs.get(&(base + 2)).copied().unwrap_or(0)) else { continue };
        // The number of values that the way of mixing takes.
        let values = match mode & 0xF {
            0 => 1,
            4 | 8 | 9 => 3,
            _ => 2,
        };
        for k in 0..values {
            if let source @ 3..=5 = (source >> (4 * k)) & 0xF {
                read[source as usize - 3] = true;
            }
        }
    }
    read
}

/// How far the colour that the steps of a template material give is from the picture that they receive (0 to 1), in full
/// light, for a red picture and a blue one. A material that darkens or tints its picture is far from it.
fn tint(tp: &Template, content: usize) -> f32 {
    let (stages, buffer_color) = sango_core::model::combiner_of(&words_of(tp, tp.c32(content + 0xC8), tp.c32(content + 0xCC)));
    let unit = sango_core::model::Unit { texture: String::new(), scale: [1.0; 2], translation: [0.0; 2], wrap: [2, 2] };
    let material = sango_core::model::Material { stages, buffer_color, units: std::array::from_fn(|_| unit.clone()), ..Default::default() };
    let picture = |color: [u8; 3]| sango_core::model::Texture { name: String::new(), width: 1, height: 1, rgba: vec![color[0], color[1], color[2], 255] };
    let white = picture([255; 3]);
    [[200u8, 70, 40], [50, 150, 220]]
        .into_iter()
        .flat_map(|color| {
            let own = picture(color);
            let shown = sango_core::render::combine(&material, &[Some(&own), Some(&white), Some(&white)], [0.5; 2], 1.0, [1.0; 4]);
            (0..3).map(move |k| (shown[k] - f32::from(color[k]) / 255.0).abs())
        })
        .fold(0.0, f32::max)
}

/// Changes the value that a list of commands gives to a register. A list that does not write the register stays as it is.
fn set_register(words: &mut [u32], register: u32, value: impl Fn(u32) -> u32) {
    if let Some(&(old, place)) = sango_core::pica::registers(words).get(&(register as u16)) {
        words[place] = value(old);
    }
}

/// Sets the commands of a material to the way that the file says: how its pixels go over what is behind, and where the colour
/// and the opacity come from. Only values change: the list keeps its size.
fn set_finish(words: &mut [u32], finish: Finish) {
    // The mix with the colour behind: the factors of the new colour and of the old one, for the colour and for the opacity.
    let (mix, test, writes_depth) = match finish.cover {
        Cover::Opaque => (0x0101_0000, 0, true),
        // The test of the opacity: on, "greater than" 127.
        Cover::Cut => (0x0101_0000, 1 | 6 << 4 | 127 << 8, true),
        // Greater than 0: a clear pixel is not drawn.
        Cover::SeeThrough => (0x7676_0000, 1 | 6 << 4, false),
    };
    set_register(words, 0x101, |_| mix);
    set_register(words, 0x104, |_| test);
    set_register(words, 0x107, |old| if writes_depth { old | 0x1000 } else { old & !0x1000 });
    if finish.unlit {
        // The first step gives the first picture (colour and opacity), the others keep what they receive (the source 15).
        for (k, base) in STAGES.into_iter().enumerate() {
            set_register(words, base, |_| if k == 0 { 0x0333_0333 } else { 0x0FFF_0FFF });
            for offset in [1, 2, 4] {
                set_register(words, base + offset, |_| 0);
            }
        }
    } else if finish.cover != Cover::Opaque {
        // The last step takes the opacity of the first picture, and keeps its way with the colour.
        let last = STAGES[5];
        set_register(words, last, |old| old & 0x0000_FFFF | 0x0333_0000);
        set_register(words, last + 1, |old| old & 0x0000_0FFF);
        set_register(words, last + 2, |old| old & 0x0000_FFFF);
        set_register(words, last + 4, |old| old & 0x0000_FFFF);
    }
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// The texture of each unit, keeping only the ones that are a layer of their own.
fn normalize_slots(slots: &[Option<String>; 3]) -> [Option<String>; 3] {
    let mut out: [Option<String>; 3] = std::array::from_fn(|k| slots[k].clone().filter(|s| !s.is_empty() && !s.contains("DummyTex")));
    if out[0].is_none() && out[1].is_some() {
        out[0] = out[1].take();
    }
    if out[1].is_some() && out[1] == out[0] {
        out[1] = None;
    }
    out
}

/// The units of a list of textures, with the unit each layer came from.
fn normalize_units(slots: &[Option<String>; 3]) -> ([Option<String>; 3], [Option<usize>; 3]) {
    let mut out: [Option<String>; 3] = std::array::from_fn(|k| slots[k].clone().filter(|s| !s.is_empty() && !s.contains("DummyTex")));
    let mut origin = [Some(0usize), Some(1), Some(2)];
    if out[0].is_none() && out[1].is_some() {
        out[0] = out[1].take();
        origin[0] = Some(1);
        origin[1] = None;
    }
    if out[1].is_some() && out[1] == out[0] {
        out[1] = None;
        origin[1] = None;
    }
    for k in 0..3 {
        if out[k].is_none() {
            origin[k] = None;
        }
    }
    (out, origin)
}

fn used_units(slots: &[Option<String>; 3]) -> [bool; 3] {
    let n = normalize_slots(slots);
    [n[0].is_some(), n[1].is_some(), n[2].is_some()]
}

fn material_class(name: &str) -> &'static str {
    if name.contains("Eye") || name.contains("Iris") {
        "eye"
    } else if name.contains("Mouth") {
        "mouth"
    } else {
        "body"
    }
}

/// The material parameters with the texture transforms of `uv` in them (twelve floats for each unit, uploaded in the order w, z, y, x).
fn texture_transform(words: &[u32], uv: &[Option<[f64; 5]>; 3]) -> Vec<u32> {
    let mut out = words.to_vec();
    let Some(pos) = (0..out.len().saturating_sub(1)).find(|&i| out[i + 1] & 0xFFFF == 0x2C1 && (out[i + 1] >> 20) & 0x7FF == 31) else { return out };
    for (unit, transform) in uv.iter().enumerate() {
        let Some([sx, sy, rot, tx, ty]) = transform else { continue };
        let (ca, sa) = (rot.cos(), rot.sin());
        for (offset, value) in [
            (0, sx * ((0.5 * sa - 0.5 * ca) + 0.5 - tx)),
            (2, sx * -sa),
            (3, sx * ca),
            (4, sy * ((0.5 * -sa - 0.5 * ca) + 0.5 - ty)),
            (6, sy * ca),
            (7, sy * sa),
        ] {
            let k = 12 * unit + offset;
            if k < 32 {
                out[if k == 0 { pos } else { pos + 1 + k }] = f32bits(value);
            }
        }
    }
    out
}

/// Writes into the vertex uniform c10 which UV set feeds each texture unit.
fn texture_sources(words: &[u32], sources: &[f64; 4]) -> Vec<u32> {
    let mut out = words.to_vec();
    for i in 0..out.len().saturating_sub(5) {
        if out[i] == 0x8000_000A && out[i + 1] & 0xFFFF == 0x2C0 && (out[i + 1] >> 20) & 0x7FF == 4 {
            for k in 0..4 {
                out[i + 2 + k] = f32bits(sources[3 - k]);
            }
            break;
        }
    }
    out
}

/// How many components the bone weight of a vertex has in the attribute setup of a mesh (0 when it is not in the buffer).
fn bone_weight_elements(enable: &[u32]) -> u32 {
    let at = find_word(enable, 0x200).expect("the attribute setup");
    let formats = enable[at + 2] as u64 | (enable[at + 3] as u64) << 32;
    let cfg = enable[at + 5] as u64 | ((enable[at + 6] & 0xFFFF) as u64) << 32;
    let perm = enable[find_word(enable, 0x2BB).expect("the attribute order")] as u64 | (enable[find_word(enable, 0x2BC).expect("the attribute order")] as u64) << 32;
    let total = enable[find_word(enable, 0x242).expect("the attribute count")] + 1;
    for slot in 0..total {
        if (formats >> (48 + slot)) & 1 == 1 {
            continue;
        }
        let which = (cfg >> (4 * slot)) & 0xF;
        if (perm >> (4 * which)) & 0xF == 8 {
            return (((formats >> (4 * which)) & 0xF) / 4 + 1) as u32;
        }
    }
    0
}

/// The boolean uniforms of the vertex shader for a mesh drawn with a material.
fn vertex_bools(base: u32, enable: &[u32], layout: &Layout) -> u32 {
    let mut bits = base & !((1 << 8) | (7 << 9) | (3 << 13));
    if bone_weight_elements(enable) == 4 {
        bits |= 1 << 8;
    }
    for k in 0..3usize {
        if !layout.units.get(&k).is_some_and(|u| u.mapping != 0) {
            bits |= 1 << (9 + k);
        }
    }
    for k in 1..3usize {
        if layout.units.contains_key(&k) {
            bits |= 1 << (12 + k);
        }
    }
    bits
}

// ---------------------------------------------------------------------------------------------------------- lookup tables

/// The lookup tables (LUT) of the templates: the sets, cloned once (a set that has the same name and other tables is renamed). Returns
/// the renamed sets and the name of the first set.
type Sampler = (u32, Vec<u32>, String);

struct LutSets {
    sets: Vec<(String, Vec<Sampler>)>,
    renamed: HashMap<(usize, String), String>,
}

fn collect_luts(templates: &[Template], keep: Option<&BTreeSet<String>>) -> LutSets {
    let mut sets: Vec<(String, Vec<Sampler>)> = Vec::new();
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    let mut renamed = HashMap::new();
    for (place, tpl) in templates.iter().enumerate() {
        let (lptr, lcnt) = (tpl.c32(12 * 4), tpl.c32(12 * 4 + 4));
        for i in 0..lcnt {
            let src = tpl.c32(lptr + 4 * i);
            let name = tpl.string(tpl.c32(src + 8));
            let mut samplers: Vec<Sampler> = Vec::new();
            for k in 0..tpl.c32(src + 4) {
                let e = src + 0xC + 0x10 * k;
                let count = tpl.c32(e + 8);
                samplers.push((tpl.c32(e) as u32, tpl.cmd(tpl.c32(e + 4), count), tpl.string(tpl.c32(e + 12))));
            }
            let names: Vec<String> = samplers.iter().map(|s| s.2.clone()).collect();
            let mut final_name = name.clone();
            while seen.get(&final_name).is_some_and(|n| *n != names) {
                final_name.push('_');
            }
            renamed.insert((place, name), final_name.clone());
            if seen.contains_key(&final_name) {
                continue;
            }
            seen.insert(final_name.clone(), names);
            sets.push((final_name, samplers));
        }
    }
    if let Some(keep) = keep {
        let first = sets.first().cloned();
        sets = first.into_iter().chain(sets.iter().skip(1).filter(|s| keep.contains(&s.0)).cloned()).collect();
    }
    LutSets { sets, renamed }
}

/// Writes the lookup table sets into the header entry 4. Returns the name of the first set, when there are some.
fn write_luts(sec: &mut Section, strs: &mut Strings, cmds: &mut Section, luts: &LutSets) -> Option<String> {
    if luts.sets.is_empty() {
        return None;
    }
    let names: Vec<&str> = luts.sets.iter().map(|s| s.0.as_str()).collect();
    let dic = bch::write_dict(sec, strs, &names);
    let list = sec.tell();
    for _ in &luts.sets {
        sec.u32(0);
    }
    sec.patch_ptr(12 * 4, 0, list as u32);
    sec.patch(12 * 4 + 4, luts.sets.len() as u32);
    sec.patch_ptr(12 * 4 + 8, 0, dic as u32);
    for (i, (name, samplers)) in luts.sets.iter().enumerate() {
        let emitted: Vec<(usize, usize)> = samplers.iter().map(|(_, words, _)| bch::emit_commands(cmds, words, &[])).collect();
        let off = sec.align(4);
        sec.patch_ptr(list + 4 * i, 0, off as u32);
        sec.ptr(0, off as u32 + 0xC);
        sec.u32(samplers.len() as u32);
        let at = strs.get(name);
        sec.ptr(1, at);
        for ((flags, _, sampler_name), (cmd_off, cmd_cnt)) in samplers.iter().zip(emitted) {
            sec.u32(*flags);
            sec.ptr(2, cmd_off as u32);
            sec.u32(cmd_cnt as u32);
            let at = strs.get(sampler_name);
            sec.ptr(1, at);
        }
    }
    Some(luts.sets[0].0.clone())
}

// ---------------------------------------------------------------------------------------------------------------- the model

/// The model file. `templates` are the models of the game that the materials are cloned from: the first one also gives the layout of the
/// meshes, of the bones and of the metadata. `model_name` is the name of the model, `texture_names` the textures of the textures file
/// (the name with `.tga` and the name in the file). `keep` is the set of the lookup table sets that the materials read, when known.
pub fn write_model_bch(model: &ModelSpec, templates: &[Template], model_name: &str, texture_names: &HashMap<String, String>, keep: Option<&BTreeSet<String>>, textures: &[crate::texture::Texture]) -> Result<(Vec<u8>, BTreeSet<String>), String> {
    let tpl = templates.first().ok_or("No template.")?;
    let (mut sec, mut strs, mut cmds, mut raw) = (Section::new(), Strings::new(), Section::new(), Section::new());
    let has_luts = templates.iter().any(|t| t.c32(12 * 4 + 4) != 0);
    let present: Vec<usize> = [0].into_iter().chain((!textures.is_empty()).then_some(3)).chain(has_luts.then_some(4)).collect();
    bch::main_header(&mut sec, &present);
    let luts = collect_luts(templates, keep);
    let lut_set = write_luts(&mut sec, &mut strs, &mut cmds, &luts);
    let mut used_luts: BTreeSet<String> = BTreeSet::new();

    // The model dictionary and the pointer list.
    let mdic = bch::write_dict(&mut sec, &mut strs, &[model_name]);
    let mlst = sec.tell();
    sec.u32(0);
    sec.patch_ptr(0, 0, mlst as u32);
    sec.patch(4, 1);
    sec.patch_ptr(8, 0, mdic as u32);

    // The template model, mesh and material data.
    let tmodel = tpl.c32(tpl.c32(0));
    let tmodel_meta = tpl.metadata(tpl.c32(tmodel + 0x94));
    let tmesh_ptr = tpl.c32(tmodel + 0x40);
    let tmesh_meta = tpl.metadata(tpl.c32(tmesh_ptr + 52));
    // The base of the vertex shader booleans: the most common map of the templates, without the bit of the weights.
    let mut counts: Vec<(u32, usize)> = Vec::new();
    for tp in templates {
        let tm = tp.c32(tp.c32(0));
        let tmesh = tp.c32(tm + 0x40);
        for mi in 0..tp.c32(tm + 0x44) {
            let sub = tp.c32(tmesh + 56 * mi + 16);
            let count = tp.c32(sub + 48);
            let words = tp.cmd(tp.c32(sub + 44), count);
            for i in (0..words.len().saturating_sub(1)).step_by(2) {
                if words[i + 1] & 0xFFFF == 0x2B0 {
                    let key = words[i] & !0x100;
                    match counts.iter_mut().find(|c| c.0 == key) {
                        Some(entry) => entry.1 += 1,
                        None => counts.push((key, 1)),
                    }
                    break;
                }
            }
        }
    }
    // The most common one (the first one in the order of the numbers on a tie, as `Counter.most_common` keeps the first met).
    let mut vs_bool_base = VS_BOOL_MAP & !0x100;
    let mut most = 0;
    for &(key, count) in &counts {
        if count > most {
            most = count;
            vs_bool_base = key;
        }
    }
    let uniform_block: Vec<u32> = vec![
        0x80000006, 0x804F02C0, 0, 0, 0, 0, 0x80000007, 0x000F02C0, f32bits(1.0 / 255.0), 0x807F02C1, f32bits(1.0), f32bits(1.0), f32bits(1.0), f32bits(1.0 / 255.0), f32bits(0.0), f32bits(0.0), f32bits(1.0), 0,
    ];

    // The model structure.
    let model_off = sec.tell();
    sec.patch_ptr(mlst, 0, model_off as u32);
    sec.u32(MODEL_FLAGS);
    for v in [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        sec.u32(f32bits(v));
    }
    for _ in 13..38 {
        sec.u32(0);
    }
    // The visibility bits: one for each mesh node, all visible.
    let vis = sec.tell();
    let nbits = model.meshes.len();
    for word in 0..(nbits + 31) / 32 {
        let mut bits = 0u32;
        for bit in 0..32 {
            if word * 32 + bit < nbits {
                bits |= 1 << bit;
            }
        }
        sec.u32(bits);
    }
    sec.patch_ptr(model_off + 31 * 4, 0, vis as u32);
    sec.patch(model_off + 32 * 4, nbits as u32);
    let at = strs.get(model_name);
    sec.patch_ptr(model_off + 33 * 4, 1, at);
    let mesh_names: Vec<&str> = model.meshes.iter().map(|m| m.name.as_str()).collect();
    let node_dic = bch::write_dict(&mut sec, &mut strs, &mesh_names);
    sec.patch(model_off + 34 * 4, nbits as u32);
    sec.patch_ptr(model_off + 35 * 4, 0, node_dic as u32);

    // The materials: the list is filled later.
    let mat_names: Vec<&str> = model.materials.iter().map(|m| m.name.as_str()).collect();
    let mat_dic = bch::write_dict(&mut sec, &mut strs, &mat_names);
    let mat_lst = sec.tell();
    for _ in &model.materials {
        sec.raw(&[0u8; 44]);
    }
    sec.patch_ptr(model_off + 13 * 4, 0, mat_lst as u32);
    sec.patch(model_off + 14 * 4, model.materials.len() as u32);
    sec.patch_ptr(model_off + 15 * 4, 0, mat_dic as u32);

    // The meshes: one for each draw.
    let mut faces: Vec<(usize, &MeshSpec, &Face)> = Vec::new();
    for (mi, mesh) in model.meshes.iter().enumerate() {
        for face in &mesh.faces {
            faces.push((mi, mesh, face));
        }
    }
    let mesh_lst = sec.tell();
    for _ in &faces {
        sec.raw(&[0u8; 56]);
    }
    let mesh_end = sec.tell();
    sec.patch_ptr(model_off + 16 * 4, 0, mesh_lst as u32);
    sec.patch(model_off + 17 * 4, faces.len() as u32);
    sec.patch_ptr(model_off + 18 * 4, 0, mesh_lst as u32);
    sec.patch_ptr(model_off + 19 * 4, 0, mesh_end as u32);
    for k in 20..26 {
        sec.patch_ptr(model_off + k * 4, 0, mesh_end as u32);
    }

    // The bones.
    let bone_names: Vec<&str> = model.joints.iter().map(|j| j.name.as_str()).collect();
    let bone_dic = bch::write_dict(&mut sec, &mut strs, &bone_names);
    let bone_lst = sec.tell();
    sec.patch_ptr(model_off + 28 * 4, 0, bone_lst as u32);
    sec.patch(model_off + 29 * 4, model.joints.len() as u32);
    sec.patch_ptr(model_off + 30 * 4, 0, bone_dic as u32);
    let mut world: HashMap<&str, Matrix34> = HashMap::new();
    let index: HashMap<&str, usize> = bone_names.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    for joint in &model.joints {
        let local = local_matrix(joint);
        let matrix = match world.get(joint.parent.as_str()) {
            Some(parent) => mul34(parent, &local),
            None => local,
        };
        world.insert(joint.name.as_str(), matrix);
        // Bit 22: segment scale compensation (every bone); then what the runtime may skip: uniform scale, unit volume, no rotation, no translation.
        let mut flags = 1u32 << 22;
        if joint.scale[0] == joint.scale[1] && joint.scale[1] == joint.scale[2] {
            flags |= 1 << 23;
        }
        if joint.scale == [1.0, 1.0, 1.0] {
            flags |= 1 << 25;
        }
        if joint.rot.iter().all(|&v| v == 0.0) {
            flags |= 1 << 26;
        }
        if joint.trans.iter().all(|&v| v == 0.0) {
            flags |= 1 << 27;
        }
        sec.u32(flags);
        sec.u32(index.get(joint.parent.as_str()).map_or(0xFFFF, |&i| i as u32));
        for v in joint.scale.iter().map(|&v| v as f64).chain(joint.rot.iter().copied()).chain(joint.trans.iter().map(|&v| v as f64)) {
            sec.u32(f32bits(v));
        }
        let inverse = inv34(&world[joint.name.as_str()]);
        for row in inverse {
            for v in row {
                sec.u32(f32bits(v));
            }
        }
        let at = strs.get(&joint.name);
        sec.ptr(1, at);
        sec.u32(0);
    }

    // The metadata of the model: the bounding box of the source.
    let meta: Vec<(String, u32, Vec<u32>)> = tmodel_meta
        .iter()
        .map(|(name, kind, words)| {
            let words = if name == "$BBoxMinMax" { model.aabb.0.iter().chain(model.aabb.1.iter()).map(|&v| f32bits(v)).collect() } else { words.clone() };
            (name.clone(), *kind, words)
        })
        .collect();
    let meta_off = write_metadata(&mut sec, &mut strs, &meta);
    sec.patch_ptr(model_off + 37 * 4, 0, meta_off as u32);

    // The material contents: the template material that feeds the same texture units.
    struct Candidate {
        place: usize,
        material: usize,
        name: String,
        units: [bool; 3],
        model_name: String,
        blend: (bool, bool),
        additive: bool,
        reads: [bool; 3],
        tint: f32,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    for (place, tp) in templates.iter().enumerate() {
        let tmodel_p = tp.c32(tp.c32(0));
        let mats = tp.c32(tmodel_p + 0x34);
        for i in 0..tp.c32(tmodel_p + 0x38) {
            let md = mats + 44 * i;
            let slots: [Option<String>; 3] = std::array::from_fn(|k| if tp.relocs.get(&(md + 28 + 4 * k)) == Some(&1) { Some(tp.string(tp.c32(md + 28 + 4 * k))) } else { None });
            candidates.push(Candidate {
                place,
                material: md,
                name: tp.string(tp.c32(md + 40)),
                units: used_units(&slots),
                model_name: tp.string(tp.c32(tmodel_p + 33 * 4)),
                blend: template_blend(tp, tp.c32(md)),
                additive: template_additive(tp, tp.c32(md)),
                reads: pictures_read(tp, tp.c32(md)),
                tint: tint(tp, tp.c32(md)),
            });
        }
    }
    let pick = |name: &str, slots: &[Option<String>; 3], blend: &[bool], finish: Option<Finish>| -> usize {
        let (want, class) = (used_units(slots), material_class(name));
        let (mut best, mut score) = (0usize, i64::MIN);
        for (index, cand) in candidates.iter().enumerate() {
            // The first unit carries the colour, so it weighs the most, and the third is only the normal map.
            let mut s: i64 = [8, 4, 2].iter().zip(cand.units.iter().zip(want.iter())).filter(|(_, (a, b))| a == b).map(|(w, _)| *w).sum();
            s += if material_class(&cand.name) == class { 2 } else { 0 };
            s += if cand.name == name { 16 } else { common_prefix(&cand.name, name) as i64 };
            s += if cand.blend.1 == blend[1] { 8 } else { 0 };
            s += if cand.blend.0 == blend[0] { 4 } else { 0 };
            if blend.len() > 2 && cand.additive == blend[2] {
                s += 12;
            }
            // A material whose steps read a picture that the source does not give would show the white picture in its place.
            if finish.is_some_and(|f| !f.unlit) {
                s -= 100 * (1..3).filter(|&k| cand.reads[k] && !want[k]).count() as i64;
                // The steps must show the picture. A template of the game tints it a little for its shadows (less than 0.3);
                // some make it black when they receive a white picture or no constant colour.
                if cand.tint > 0.4 {
                    s -= 1000;
                }
            }
            if s > score {
                best = index;
                score = s;
            }
        }
        best
    };

    // The cache is keyed by template and place: two models can have a material at the same offset.
    let mut cache: HashMap<(usize, usize), (usize, usize, Vec<(String, u32, Vec<u32>)>, (usize, usize), Vec<u32>)> = HashMap::new();
    for (i, spec) in model.materials.iter().enumerate() {
        let (slots, origin) = normalize_units(&spec.slots);
        let uv: [Option<[f64; 5]>; 3] = std::array::from_fn(|k| origin[k].and_then(|o| spec.uv[o]));
        let mappers: [Option<(u8, u8, u8, u8, u8, Option<u32>)>; 3] = std::array::from_fn(|k| origin[k].and_then(|o| spec.layout.units.get(&o)).map(|u| (u.wrap.0, u.wrap.1, u.mag, u.min, u.lod, u.border)));
        let sources: [f64; 4] = std::array::from_fn(|k| if k < 3 { origin[k].map_or(0.0, |o| spec.layout.sources[o]) } else { spec.layout.sources[3] });
        let cand = &candidates[pick(&spec.name, &spec.slots, &spec.blend, spec.finish)];
        let tp = &templates[cand.place];
        let tmat = cand.material;
        let key = (cand.place, tmat);
        if !cache.contains_key(&key) {
            let tcontent = tp.c32(tmat);
            let (texfunc_off, texfunc_cnt) = (tp.c32(tmat + 16), tp.c32(tmat + 20));
            let (param_off, param_cnt) = (tp.c32(tcontent + 0xC8), tp.c32(tcontent + 0xCC));
            let texfunc = bch::emit_commands(&mut cmds, &tp.cmd(texfunc_off, texfunc_cnt), &[]);
            cache.insert(key, (tcontent, tp.c32(tmat + 24), tp.metadata(tp.c32(tcontent + 0x10C)), texfunc, tp.cmd(param_off, param_cnt)));
        }
        let (tcontent, tsamplers, tmeta, texfunc, param_words) = cache[&key].clone();
        let mut param_words = texture_sources(&texture_transform(&param_words, &uv), &sources);
        if let Some(finish) = spec.finish {
            set_finish(&mut param_words, finish);
        }
        let param = bch::emit_commands(&mut cmds, &param_words, &[]);
        let content = sec.tell();
        sec.raw(&tp.d[tp.c + tcontent..tp.c + tcontent + 0x110]);
        // The texture coordinators: one for each unit, 0x18 bytes from 0x0C: four bytes of flags, then scale x and y, rotation, translation x and y.
        for (k, transform) in uv.iter().enumerate() {
            let Some(values) = transform else { continue };
            for (j, v) in values.iter().enumerate() {
                sec.patch(content + 0x0C + 0x18 * k + 4 * (j + 1), f32bits(*v));
            }
        }
        let samplers = sec.tell();
        let mut smp = tp.d[tp.c + tsamplers..tp.c + tsamplers + 0x30].to_vec();
        // The texture mappers, 16 bytes a unit: type, wrap u, wrap v, mag, min, min lod, padding, LOD bias at 8, border colour at 12.
        for (k, mapper) in mappers.iter().enumerate() {
            let Some((wrap_u, wrap_v, mag, min, lod, border)) = mapper else { continue };
            smp[16 * k + 1..16 * k + 6].copy_from_slice(&[*wrap_u, *wrap_v, *mag, *min, *lod]);
            if let Some(border) = border {
                smp[16 * k + 12..16 * k + 16].copy_from_slice(&border.to_le_bytes());
            }
        }
        sec.raw(&smp);
        for off in (0..0x110).step_by(4) {
            match tp.relocs.get(&(tcontent + off)) {
                Some(1) => {
                    let mut s = tp.string(tp.c32(tcontent + off));
                    if (0xE0..=0xE8).contains(&off) {
                        // the paths of the lookup tables 3 to 5 name the set of the tables.
                        s = luts.renamed.get(&(cand.place, s.clone())).cloned().unwrap_or(s);
                        used_luts.insert(s.clone());
                    } else if (0xF8..=0x100).contains(&off) {
                        // names 3 to 5 of the lookup tables: the table read for R, G and B stays the one of the template.
                    } else if s.ends_with(&format!("@{}", cand.model_name)) {
                        s = format!("{}@{}", spec.name, model_name);
                    }
                    let at = strs.get(&s);
                    sec.patch_ptr(content + off, 1, at);
                }
                Some(2) | Some(3) => {
                    let kind = *tp.relocs.get(&(tcontent + off)).unwrap();
                    sec.patch_ptr(content + off, kind, param.0 as u32);
                }
                Some(0) if off != 0x10C => return Err(format!("Unexpected pointer in a material of the template at {off:#x}.")),
                _ => {}
            }
        }
        let meta_ptr = write_metadata(&mut sec, &mut strs, &tmeta);
        sec.patch_ptr(content + 0x10C, 0, meta_ptr as u32);
        let md = mat_lst + 44 * i;
        sec.patch_ptr(md, 0, content as u32);
        sec.patch_ptr(md + 16, 2, texfunc.0 as u32);
        sec.patch(md + 20, texfunc.1 as u32);
        sec.patch_ptr(md + 24, 0, samplers as u32);
        for k in 0..3 {
            let texture = slots[k].clone().unwrap_or_else(|| "DummyTex.tga".to_string());
            // A texture that the model does not carry is a shared one: the game resolves DummyTex, and only that.
            let at = strs.get(texture_names.get(&texture).map_or("DummyTex", |n| n.as_str()));
            sec.patch_ptr(md + 28 + 4 * k, 1, at);
        }
        let at = strs.get(&spec.name);
        sec.patch_ptr(md + 40, 1, at);
    }

    // The mesh data: the buffers, the commands and the structures of the sub-meshes.
    let mat_index: HashMap<&str, usize> = mat_names.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let layouts: HashMap<&str, &Layout> = model.materials.iter().map(|m| (m.name.as_str(), &m.layout)).collect();
    for (fi, (mi, mesh, face)) in faces.iter().enumerate() {
        raw.align(0x80);
        let vb = raw.tell();
        raw.raw(&face.vb);
        raw.align(0x10);
        let ib = raw.tell();
        raw.raw(&face.ib);
        let [enable, disable, draw] = &face.cmds;
        let mut en: Vec<u32> = enable[..enable.len() - 2].to_vec();
        en.extend_from_slice(&uniform_block);
        en.extend_from_slice(&enable[enable.len() - 2..]);
        let base = find_word(&en, 0x200).ok_or("The attribute setup has no buffer.")?;
        let (en_off, en_cnt) = bch::emit_commands(&mut cmds, &en, &[(base, 0xE, 0), (base + 4, 6, vb as u32)]);
        let (dis_off, dis_cnt) = bch::emit_commands(&mut cmds, disable, &[]);
        let layout = layouts.get(face.material.as_str()).ok_or("A draw has an unknown material.")?;
        let mut dr: Vec<u32> = vec![vertex_bools(vs_bool_base, enable, layout), 0xF02B0];
        dr.extend_from_slice(draw);
        let at = find_word_wide(&dr, 0x227).ok_or("The draw has no index buffer.")?;
        let idx16 = (dr[at] >> 31) & 1;
        let (dr_off, dr_cnt) = bch::emit_commands(&mut cmds, &dr, &[(at, if idx16 == 1 { 7 } else { 8 }, ib as u32)]);
        let sub = sec.tell();
        sec.u32(1 | ((face.joints.len() as u32) << 16));
        for k in 0..20 {
            let joint = face.joints.get(k).copied().unwrap_or(0) as u16;
            sec.raw(&joint.to_le_bytes());
        }
        sec.ptr(2, dr_off as u32);
        sec.u32(dr_cnt as u32);
        let center: [f64; 3] = std::array::from_fn(|k| (mesh.aabb.0[k] + mesh.aabb.1[k]) / 2.0);
        let m = mesh_lst + 56 * fi;
        sec.patch(m, mat_index[face.material.as_str()] as u32 | (MESH_FLAGS << 16));
        // The node index, then the draw order: the priority in the low byte, the layer (opaque, translucent, subtractive, additive) in the next two bits.
        sec.patch(m + 4, *mi as u32 | (((layout.priority & 0xFF) | ((layout.layer & 3) << 8)) << 16));
        sec.patch_ptr(m + 8, 2, en_off as u32);
        sec.patch(m + 12, en_cnt as u32);
        sec.patch_ptr(m + 16, 0, sub as u32);
        sec.patch(m + 20, 1);
        sec.patch_ptr(m + 24, 2, dis_off as u32);
        sec.patch(m + 28, dis_cnt as u32);
        for (k, v) in center.iter().enumerate() {
            sec.patch(m + 32 + 4 * k, f32bits(*v));
        }
        sec.patch_ptr(m + 44, 0, model_off as u32);
        let mmeta: Vec<(String, u32, Vec<u32>)> = tmesh_meta
            .iter()
            .map(|(n, t, w)| (n.clone(), *t, if n == "$BBoxMinMax" { mesh.aabb.0.iter().chain(mesh.aabb.1.iter()).map(|&v| f32bits(v)).collect() } else { w.clone() }))
            .collect();
        let at = write_metadata(&mut sec, &mut strs, &mmeta);
        sec.patch_ptr(m + 52, 0, at as u32);
    }

    let _ = lut_set;
    // The unused helpers of the other kinds of source (ported shaders) are not needed here.
    let _ = (multiply as fn(&Matrix, &Matrix) -> Matrix, invert as fn(&Matrix) -> Option<Matrix>);
    if !textures.is_empty() {
        crate::texture::append_textures(&mut sec, &mut strs, &mut cmds, &mut raw, textures);
    }
    Ok((bch::build(&mut sec, &strs, &cmds, &raw), used_luts))
}

/// The model file, written in two passes: the second one keeps only the lookup table sets that the materials read (every set weighs about
/// 1 KB per table, and the heap of the Pokémon models is small). `textures` go in the same file (the models of the trainers hold their
/// textures); a Pokémon model has none, its textures are in another file.
pub fn write_model_file_with(model: &ModelSpec, templates: &[Template], model_name: &str, texture_names: &HashMap<String, String>, textures: &[crate::texture::Texture]) -> Result<Vec<u8>, String> {
    let (first, used) = write_model_bch(model, templates, model_name, texture_names, None, textures)?;
    if !templates.iter().any(|t| t.c32(12 * 4 + 4) != 0) {
        return Ok(first);
    }
    Ok(write_model_bch(model, templates, model_name, texture_names, Some(&used), textures)?.0)
}

/// The model file of a Pokémon (without textures).
pub fn write_model_file(model: &ModelSpec, templates: &[Template], model_name: &str, texture_names: &HashMap<String, String>) -> Result<Vec<u8>, String> {
    write_model_file_with(model, templates, model_name, texture_names, &[])
}
