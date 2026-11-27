//! A reader of glTF files (`.glb` and `.gltf`): the skeleton, the meshes with their weights, the materials, the pictures and the
//! animations. It reads what the Model tab exports (one node for each bone, one skin, triangles with 4 weights) and the files that other
//! programs write in the same way.

use serde_json::Value;
use std::path::Path;

/// The end of the name of a node that the export of Studio puts above a bone whose parent has a scale. In the game a bone
/// does not take the scale of its parent, and glTF has no such rule: the node holds the translation of the bone and takes the
/// scale of the parent out. The reader merges such a node with its bone.
pub const SCALE_NODE_SUFFIX: &str = "~scale";

/// The name of the maker in the files that the export of Studio writes.
pub const GENERATOR: &str = "Sango Studio";

/// The name of the node of the meshes in the files that the export of Studio writes. The sizes of these files are in the units
/// of the game. A program that writes the file again keeps the names of the nodes (it can add a number after them), so the
/// reader still knows the units.
pub const GAME_UNITS_NODE: &str = "sango_model";

/// A node of the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub translation: [f32; 3],
    /// The rotation as a quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

/// A mesh: triangles with their weights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// The joints (places in the list of the joints of the skin) and their weights.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub material: usize,
}

/// A material: the name, the colour, the picture and how it blends.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    /// The place of the picture of the base colour in `Source::images`.
    pub image: Option<usize>,
    /// True for the modes BLEND and MASK.
    pub blend: bool,
    /// True for the mode MASK: a pixel is drawn or not.
    pub mask: bool,
    /// True when the file says that the light does not touch the material (`KHR_materials_unlit`).
    pub unlit: bool,
    /// How the picture repeats outside of the square of the texture coordinates (horizontally, vertically), as the GPU numbers it: 0 clamp
    /// to the edge, 1 clamp to the border, 2 repeat, 3 mirrored repeat.
    pub wrap: [u8; 2],
}

/// A picture, as 8-bit RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// What a channel of an animation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path3 {
    Translation,
    Rotation,
    Scale,
}

/// The values of a node over time: the times in seconds, and 3 values (translation, scale) or 4 (rotation) for each time.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub node: usize,
    pub path: Path3,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// A glTF file, read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    pub nodes: Vec<Node>,
    /// The nodes that are the joints of the skin, in the order of the skin.
    pub joints: Vec<usize>,
    /// The inverse bind matrix of each joint, 16 floats in the order of the file (column by column).
    pub inverse_bind: Vec<[f32; 16]>,
    pub primitives: Vec<Primitive>,
    /// The meshes of the file: the name and the places of their primitives in `primitives`.
    pub meshes: Vec<(String, Vec<usize>)>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
    pub animations: Vec<Animation>,
    /// True when the sizes of the file are in the units of the game (a file that comes from the export of Studio), not in metres.
    pub game_units: bool,
    /// The nodes of the bones that do not take the scale of their parent: they had a node above them (see `SCALE_NODE_SUFFIX`).
    /// The other nodes take it, as glTF says.
    pub own_scale: Vec<usize>,
}

/// The binary parts of a file.
struct Binary {
    json: Value,
    buffers: Vec<Vec<u8>>,
}

fn number(value: &Value, key: &str) -> Option<usize> {
    value.get(key)?.as_u64().map(|n| n as usize)
}

fn floats<const N: usize>(value: Option<&Value>, default: [f32; N]) -> [f32; N] {
    let Some(list) = value.and_then(|v| v.as_array()) else { return default };
    let mut out = default;
    for (place, item) in list.iter().take(N).enumerate() {
        out[place] = item.as_f64().unwrap_or(default[place] as f64) as f32;
    }
    out
}

/// Reads a file. `path` is the file; a `.gltf` file may have its buffers and its pictures beside it.
pub fn read(path: &Path) -> Result<Source, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_bytes(&bytes, path.parent())
}

/// Reads the bytes of a `.glb` file, or the text of a `.gltf` file with the folder of the files that it names.
pub fn read_bytes(bytes: &[u8], folder: Option<&Path>) -> Result<Source, String> {
    let binary = if bytes.starts_with(b"glTF") { split_glb(bytes)? } else { split_gltf(bytes, folder)? };
    build(&binary, folder)
}

fn split_glb(bytes: &[u8]) -> Result<Binary, String> {
    let u32_at = |at: usize| -> Result<usize, String> { bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| "The file is cut.".to_string()) };
    if u32_at(4)? != 2 {
        return Err("This is not a glTF 2 file.".into());
    }
    let (mut at, mut json, mut binary) = (12usize, None, None);
    while at + 8 <= bytes.len() {
        let (length, kind) = (u32_at(at)?, &bytes[at + 4..at + 8]);
        let chunk = bytes.get(at + 8..at + 8 + length).ok_or("A chunk of the file is cut.")?;
        match kind {
            b"JSON" => json = Some(chunk),
            b"BIN\0" if binary.is_none() => binary = Some(chunk.to_vec()),
            _ => {}
        }
        at += 8 + length;
    }
    let json: Value = serde_json::from_slice(json.ok_or("The file has no JSON part.")?).map_err(|e| format!("The JSON of the file is wrong: {e}"))?;
    Ok(Binary { json, buffers: vec![binary.unwrap_or_default()] })
}

fn split_gltf(bytes: &[u8], folder: Option<&Path>) -> Result<Binary, String> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| format!("This is not a glTF file: {e}"))?;
    let mut buffers = Vec::new();
    for buffer in json.get("buffers").and_then(|b| b.as_array()).into_iter().flatten() {
        let uri = buffer.get("uri").and_then(|u| u.as_str()).ok_or("A buffer has no file.")?;
        buffers.push(load_uri(uri, folder)?);
    }
    Ok(Binary { json, buffers })
}

/// The bytes that a link names: a `data:` link, or a file beside the glTF file.
fn load_uri(uri: &str, folder: Option<&Path>) -> Result<Vec<u8>, String> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (_, data) = rest.split_once("base64,").ok_or("A data link is not in base64.")?;
        return base64(data);
    }
    let folder = folder.ok_or("A file is named but the file of the glTF is not known.")?;
    let name = uri.replace("%20", " ");
    std::fs::read(folder.join(&name)).map_err(|e| format!("{name}: {e}"))
}

fn base64(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    };
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=' && !c.is_ascii_whitespace()) {
        acc = (acc << 6) | value(c).ok_or("A data link has a wrong character.")?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

impl Binary {
    /// The bytes of a buffer view.
    fn view(&self, index: usize) -> Result<&[u8], String> {
        let view = self.json["bufferViews"].get(index).ok_or("A buffer view is missing.")?;
        let buffer = self.buffers.get(number(view, "buffer").unwrap_or(0)).ok_or("A buffer is missing.")?;
        let start = number(view, "byteOffset").unwrap_or(0);
        let length = number(view, "byteLength").ok_or("A buffer view has no length.")?;
        buffer.get(start..start + length).ok_or_else(|| "A buffer view is out of the buffer.".to_string())
    }

    /// The values of an accessor as floats, `count` lines of `components` values. Integers that are normalised become 0 to 1 (or -1 to 1).
    fn accessor(&self, index: usize) -> Result<(Vec<f64>, usize), String> {
        let accessor = self.json["accessors"].get(index).ok_or("An accessor is missing.")?;
        let components = match accessor["type"].as_str().unwrap_or("") {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            other => return Err(format!("The accessor type {other} is not known.")),
        };
        let count = number(accessor, "count").ok_or("An accessor has no count.")?;
        let kind = number(accessor, "componentType").ok_or("An accessor has no component type.")?;
        let normalized = accessor.get("normalized").and_then(|n| n.as_bool()).unwrap_or(false);
        let size = match kind {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return Err(format!("The component type {kind} is not known.")),
        };
        let Some(view_index) = number(accessor, "bufferView") else { return Ok((vec![0.0; count * components], components)) };
        let view = self.view(view_index)?;
        let stride = number(&self.json["bufferViews"][view_index], "byteStride").filter(|&s| s != 0).unwrap_or(size * components);
        let base = number(accessor, "byteOffset").unwrap_or(0);
        let mut out = Vec::with_capacity(count * components);
        for line in 0..count {
            for component in 0..components {
                let at = base + line * stride + component * size;
                let bytes = view.get(at..at + size).ok_or("An accessor is out of its buffer view.")?;
                let value = match kind {
                    5120 => i8::from_le_bytes([bytes[0]]) as f64 / if normalized { 127.0 } else { 1.0 },
                    5121 => bytes[0] as f64 / if normalized { 255.0 } else { 1.0 },
                    5122 => i16::from_le_bytes([bytes[0], bytes[1]]) as f64 / if normalized { 32767.0 } else { 1.0 },
                    5123 => u16::from_le_bytes([bytes[0], bytes[1]]) as f64 / if normalized { 65535.0 } else { 1.0 },
                    5125 => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
                    _ => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64,
                };
                out.push(value);
            }
        }
        Ok((out, components))
    }
}

fn chunks<const N: usize>(values: &[f64]) -> Vec<[f32; N]> {
    values.chunks_exact(N).map(|c| std::array::from_fn(|k| c[k] as f32)).collect()
}

fn build(binary: &Binary, folder: Option<&Path>) -> Result<Source, String> {
    let json = &binary.json;
    let mut source = Source::default();

    // The nodes and their parents.
    let nodes = json["nodes"].as_array().ok_or("The file has no nodes.")?;
    for (place, node) in nodes.iter().enumerate() {
        let (mut translation, mut rotation, mut scale) = (floats(node.get("translation"), [0.0; 3]), floats(node.get("rotation"), [0.0, 0.0, 0.0, 1.0]), floats(node.get("scale"), [1.0; 3]));
        if let Some(matrix) = node.get("matrix").and_then(|m| m.as_array()).filter(|m| m.len() == 16) {
            let m: Vec<f32> = matrix.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect();
            (translation, rotation, scale) = decompose(&m);
        }
        let children: Vec<usize> = node.get("children").and_then(|c| c.as_array()).map(|c| c.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect()).unwrap_or_default();
        source.nodes.push(Node { name: node.get("name").and_then(|n| n.as_str()).map_or_else(|| format!("node{place}"), |n| n.to_string()), parent: None, children, translation, rotation, scale });
    }
    for parent in 0..source.nodes.len() {
        for child in source.nodes[parent].children.clone() {
            if let Some(node) = source.nodes.get_mut(child) {
                node.parent = Some(parent);
            }
        }
    }

    // The skin.
    if let Some(skin) = json["skins"].get(0) {
        source.joints = skin["joints"].as_array().ok_or("The skin has no joints.")?.iter().filter_map(|j| j.as_u64().map(|n| n as usize)).collect();
        source.inverse_bind = match number(skin, "inverseBindMatrices") {
            Some(accessor) => binary.accessor(accessor)?.0.chunks_exact(16).map(|c| std::array::from_fn(|k| c[k] as f32)).collect(),
            None => vec![[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]; source.joints.len()],
        };
    }

    // The pictures.
    for (place, image) in json["images"].as_array().into_iter().flatten().enumerate() {
        let bytes = match (number(image, "bufferView"), image.get("uri").and_then(|u| u.as_str())) {
            (Some(view), _) => binary.view(view)?.to_vec(),
            (None, Some(uri)) => load_uri(uri, folder)?,
            _ => return Err(format!("The picture {place} has no data.")),
        };
        let decoded = image::load_from_memory(&bytes).map_err(|e| format!("The picture {place}: {e}"))?.to_rgba8();
        let name = image.get("name").and_then(|n| n.as_str()).map_or_else(|| format!("image{place}"), |n| n.to_string());
        source.images.push(Image { name, width: decoded.width() as usize, height: decoded.height() as usize, rgba: decoded.into_raw() });
    }

    // The materials.
    for (place, material) in json["materials"].as_array().into_iter().flatten().enumerate() {
        let pbr = &material["pbrMetallicRoughness"];
        let image = number(&pbr["baseColorTexture"], "index").and_then(|t| json["textures"].get(t)).and_then(|t| number(t, "source"));
        let mode = material.get("alphaMode").and_then(|m| m.as_str()).unwrap_or("OPAQUE");
        // The sampler of the picture says how it repeats (the default of glTF is to repeat).
        let sampler = number(&pbr["baseColorTexture"], "index").and_then(|t| json["textures"].get(t)).and_then(|t| number(t, "sampler")).and_then(|s| json["samplers"].get(s));
        let wrap_of = |key: &str| match sampler.and_then(|s| s.get(key)).and_then(|w| w.as_u64()) {
            Some(33071) => 0,
            Some(33648) => 3,
            _ => 2,
        };
        source.materials.push(Material {
            wrap: [wrap_of("wrapS"), wrap_of("wrapT")],
            name: material.get("name").and_then(|n| n.as_str()).map_or_else(|| format!("material{place}"), |n| n.to_string()),
            base_color: floats(pbr.get("baseColorFactor"), [0.8, 0.8, 0.8, 1.0]),
            image,
            blend: mode == "BLEND" || mode == "MASK",
            mask: mode == "MASK",
            unlit: material["extensions"].get("KHR_materials_unlit").is_some(),
        });
    }

    // The meshes of the nodes that have a mesh (every primitive is a list of triangles).
    for (mesh_place, mesh) in json["meshes"].as_array().into_iter().flatten().enumerate() {
        let first = source.primitives.len();
        for primitive in mesh["primitives"].as_array().into_iter().flatten() {
            if number(primitive, "mode").is_some_and(|m| m != 4) {
                return Err("Only triangles are supported.".into());
            }
            let attributes = &primitive["attributes"];
            let attribute = |name: &str| number(attributes, name).map(|a| binary.accessor(a)).transpose();
            let positions = chunks::<3>(&attribute("POSITION")?.ok_or("A mesh has no positions.")?.0);
            let count = positions.len();
            let normals = attribute("NORMAL")?.map(|a| chunks::<3>(&a.0)).unwrap_or_else(|| vec![[0.0, 0.0, 1.0]; count]);
            let uvs = attribute("TEXCOORD_0")?.map(|a| chunks::<2>(&a.0)).unwrap_or_else(|| vec![[0.0, 0.0]; count]);
            let joints = attribute("JOINTS_0")?.map(|a| a.0.chunks_exact(4).map(|c| std::array::from_fn(|k| c[k] as u16)).collect()).unwrap_or_else(|| vec![[0u16; 4]; count]);
            let weights = attribute("WEIGHTS_0")?.map(|a| chunks::<4>(&a.0)).unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; count]);
            let indices: Vec<u32> = match number(primitive, "indices") {
                Some(accessor) => binary.accessor(accessor)?.0.into_iter().map(|v| v as u32).collect(),
                None => (0..count as u32).collect(),
            };
            if indices.len() % 3 != 0 || indices.iter().any(|&i| i as usize >= count) {
                return Err("A mesh has wrong indices.".into());
            }
            source.primitives.push(Primitive { positions, normals, uvs, joints, weights, indices, material: number(primitive, "material").unwrap_or(0) });
        }
        let name = mesh.get("name").and_then(|n| n.as_str()).map_or_else(|| format!("mesh{mesh_place}"), |n| n.to_string());
        source.meshes.push((name, (first..source.primitives.len()).collect()));
    }
    if source.primitives.is_empty() {
        return Err("The file has no mesh.".into());
    }

    // The animations.
    for (place, animation) in json["animations"].as_array().into_iter().flatten().enumerate() {
        let mut channels = Vec::new();
        for channel in animation["channels"].as_array().into_iter().flatten() {
            let path = match channel["target"]["path"].as_str() {
                Some("translation") => Path3::Translation,
                Some("rotation") => Path3::Rotation,
                Some("scale") => Path3::Scale,
                _ => continue,
            };
            let (Some(node), Some(sampler)) = (number(&channel["target"], "node"), number(channel, "sampler").and_then(|s| animation["samplers"].get(s))) else { continue };
            let (times, _) = binary.accessor(number(sampler, "input").ok_or("A sampler has no input.")?)?;
            let (values, _) = binary.accessor(number(sampler, "output").ok_or("A sampler has no output.")?)?;
            channels.push(Channel { node, path, times: times.into_iter().map(|t| t as f32).collect(), values: values.into_iter().map(|v| v as f32).collect() });
        }
        source.animations.push(Animation { name: animation.get("name").and_then(|n| n.as_str()).map_or_else(|| format!("animation{place}"), |n| n.to_string()), channels });
    }
    source.game_units = json["asset"]["generator"] == GENERATOR || source.nodes.iter().any(|n| n.name.starts_with(GAME_UNITS_NODE));
    merge_scale_nodes(&mut source);
    Ok(source)
}

/// The rotation `a` after the rotation `b` (quaternions x, y, z, w).
fn turn_after(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// A point turned by a rotation, then moved by `by`.
fn turned_and_moved(rotation: [f32; 4], point: [f32; 3], by: [f32; 3]) -> [f32; 3] {
    let back = [-rotation[0], -rotation[1], -rotation[2], rotation[3]];
    let turned = turn_after(turn_after(rotation, [point[0], point[1], point[2], 0.0]), back);
    std::array::from_fn(|k| turned[k] + by[k])
}

/// Merges each node that takes the scale of a parent out (see `SCALE_NODE_SUFFIX`) with the bone below it: the bone takes the
/// place and the translation of the node, so the skeleton is the one of the bones again. The scale of the node is dropped, as
/// the game does not give the scale of a parent to a bone. A program that wrote the file again may have made a joint of the
/// node: its weights go to the bone.
fn merge_scale_nodes(source: &mut Source) {
    let found: Vec<(usize, usize)> = (0..source.nodes.len())
        .filter_map(|upper| {
            let name = source.nodes[upper].name.strip_suffix(SCALE_NODE_SUFFIX)?;
            match source.nodes[upper].children[..] {
                [bone] if source.nodes.get(bone).is_some_and(|n| n.name == name) => Some((upper, bone)),
                _ => None,
            }
        })
        .collect();
    for &(upper, bone) in &found {
        let above = source.nodes[upper].clone();
        let own = source.nodes[bone].clone();
        for animation in &mut source.animations {
            let take = |channels: &mut Vec<Channel>, node: usize, path: Path3| channels.iter().position(|c| c.node == node && c.path == path).map(|at| channels.remove(at));
            let moved_above = take(&mut animation.channels, upper, Path3::Translation);
            let moved_own = take(&mut animation.channels, bone, Path3::Translation);
            // The translation of the bone: the one of the node, plus the one of the bone turned by the node.
            let times = moved_above.as_ref().or(moved_own.as_ref()).map(|c| c.times.clone());
            if let Some(times) = times {
                let sample = |channel: &Option<Channel>, at: usize, rest: [f32; 3]| channel.as_ref().and_then(|c| c.values.get(at * 3..at * 3 + 3)).map_or(rest, |v| [v[0], v[1], v[2]]);
                let values = (0..times.len()).flat_map(|at| turned_and_moved(above.rotation, sample(&moved_own, at, own.translation), sample(&moved_above, at, above.translation))).collect();
                animation.channels.push(Channel { node: bone, path: Path3::Translation, times, values });
            }
            for channel in animation.channels.iter_mut().filter(|c| c.node == bone && c.path == Path3::Rotation) {
                for value in channel.values.chunks_exact_mut(4) {
                    let turned = turn_after(above.rotation, [value[0], value[1], value[2], value[3]]);
                    value.copy_from_slice(&turned);
                }
            }
            animation.channels.retain(|c| c.node != upper);
        }
        source.nodes[bone].translation = turned_and_moved(above.rotation, own.translation, above.translation);
        source.nodes[bone].rotation = turn_after(above.rotation, own.rotation);
        source.nodes[bone].parent = above.parent;
        if let Some(parent) = above.parent {
            for child in source.nodes[parent].children.iter_mut().filter(|c| **c == upper) {
                *child = bone;
            }
        }
        source.nodes[upper].parent = None;
        source.nodes[upper].children.clear();
        source.own_scale.push(bone);
    }
    // The joints without the nodes that are gone, and the new place of each old joint.
    let gone = |node: usize| found.iter().find(|&&(upper, _)| upper == node).map(|&(_, bone)| bone);
    if !source.joints.iter().any(|&node| gone(node).is_some()) {
        return;
    }
    let kept: Vec<usize> = (0..source.joints.len()).filter(|&k| gone(source.joints[k]).is_none()).collect();
    let places: Vec<u16> = (0..source.joints.len())
        .map(|k| {
            let node = gone(source.joints[k]).unwrap_or(source.joints[k]);
            kept.iter().position(|&other| source.joints[other] == node).unwrap_or(0) as u16
        })
        .collect();
    for primitive in &mut source.primitives {
        for joints in &mut primitive.joints {
            *joints = joints.map(|k| places.get(k as usize).copied().unwrap_or(0));
        }
    }
    source.inverse_bind = kept.iter().filter_map(|&k| source.inverse_bind.get(k).copied()).collect();
    source.joints = kept.iter().map(|&k| source.joints[k]).collect();
}

/// The translation, the rotation (quaternion) and the scale of a matrix of 16 floats (column by column).
fn decompose(m: &[f32]) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let column = |c: usize| [m[4 * c], m[4 * c + 1], m[4 * c + 2]];
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let scale = [length(column(0)), length(column(1)), length(column(2))];
    let r = |row: usize, c: usize| if scale[c] > 1e-12 { m[4 * c + row] / scale[c] } else { 0.0 };
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s, 0.25 * s]
    } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt() * 2.0;
        [0.25 * s, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s, (r(2, 1) - r(1, 2)) / s]
    } else if r(1, 1) > r(2, 2) {
        let s = (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt() * 2.0;
        [(r(0, 1) + r(1, 0)) / s, 0.25 * s, (r(1, 2) + r(2, 1)) / s, (r(0, 2) - r(2, 0)) / s]
    } else {
        let s = (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt() * 2.0;
        [(r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, 0.25 * s, (r(1, 0) - r(0, 1)) / s]
    };
    ([m[12], m[13], m[14]], q, scale)
}

#[cfg(test)]
mod test {
    use super::*;

    /// A file with a skeleton of two bones, a triangle weighted to them, a picture and an animation.
    fn small_glb() -> Vec<u8> {
        let mut bin: Vec<u8> = Vec::new();
        let mut push = |bytes: Vec<u8>| -> (usize, usize) {
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let start = bin.len();
            bin.extend_from_slice(&bytes);
            (start, bytes.len())
        };
        let f = |values: &[f32]| values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
        let positions = push(f(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
        let normals = push(f(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
        let uvs = push(f(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
        let joints = push(vec![0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        let weights = push(f(&[1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.25, 0.75, 0.0, 0.0]));
        let indices = push(vec![0, 1, 2]);
        let inverse = push(f(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0]));
        let times = push(f(&[0.0, 1.0]));
        let moves = push(f(&[0.0, 0.0, 0.0, 0.0, 2.0, 0.0]));
        let mut png = Vec::new();
        image::ImageEncoder::write_image(image::codecs::png::PngEncoder::new(&mut png), &[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8).unwrap();
        let picture = push(png);
        // The indices are 8-bit values, which glTF forbids for a mesh but the reader accepts: they are small numbers.
        let views = [positions, normals, uvs, joints, weights, indices, inverse, times, moves, picture];
        let views_json: Vec<String> = views.iter().map(|(start, length)| format!("{{\"buffer\":0,\"byteOffset\":{start},\"byteLength\":{length}}}")).collect();
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0,2]}}],
            "nodes":[{{"name":"root","children":[1],"translation":[0,1,0]}},{{"name":"child","translation":[1,0,0],"rotation":[0,0,0.7071,0.7071]}},{{"name":"model","mesh":0,"skin":0}}],
            "skins":[{{"inverseBindMatrices":6,"joints":[0,1]}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2,"JOINTS_0":3,"WEIGHTS_0":4}},"indices":5,"material":0}}]}}],
            "materials":[{{"name":"Body","pbrMetallicRoughness":{{"baseColorTexture":{{"index":0}}}},"alphaMode":"BLEND"}}],
            "textures":[{{"source":0}}],"images":[{{"name":"picture","bufferView":9,"mimeType":"image/png"}}],
            "animations":[{{"name":"move","samplers":[{{"input":7,"output":8}}],"channels":[{{"sampler":0,"target":{{"node":1,"path":"translation"}}}}]}}],
            "accessors":[
              {{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}},
              {{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}},
              {{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}},
              {{"bufferView":3,"componentType":5121,"count":3,"type":"VEC4"}},
              {{"bufferView":4,"componentType":5126,"count":3,"type":"VEC4"}},
              {{"bufferView":5,"componentType":5121,"count":3,"type":"SCALAR"}},
              {{"bufferView":6,"componentType":5126,"count":2,"type":"MAT4"}},
              {{"bufferView":7,"componentType":5126,"count":2,"type":"SCALAR"}},
              {{"bufferView":8,"componentType":5126,"count":2,"type":"VEC3"}}],
            "bufferViews":[{}],"buffers":[{{"byteLength":{}}}]}}"#,
            views_json.join(","),
            bin.len()
        );
        let mut json_bytes = json.into_bytes();
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn a_glb_file_is_read_with_its_skeleton_its_mesh_its_picture_and_its_animation() {
        let source = read_bytes(&small_glb(), None).unwrap();
        assert_eq!(source.nodes.len(), 3);
        assert_eq!((source.nodes[1].parent, source.nodes[0].children.clone()), (Some(0), vec![1]));
        assert_eq!(source.nodes[0].translation, [0.0, 1.0, 0.0]);
        assert!((source.nodes[1].rotation[2] - 0.7071).abs() < 1e-4);
        assert_eq!((source.joints.clone(), source.inverse_bind.len()), (vec![0, 1], 2));
        assert_eq!(source.inverse_bind[1][12], -1.0);
        let mesh = &source.primitives[0];
        assert_eq!((mesh.positions.len(), mesh.indices.clone()), (3, vec![0, 1, 2]));
        assert_eq!((mesh.joints[1], mesh.weights[2]), ([1, 0, 0, 0], [0.25, 0.75, 0.0, 0.0]));
        assert_eq!(source.materials[0], Material { name: "Body".into(), base_color: [0.8, 0.8, 0.8, 1.0], image: Some(0), blend: true, mask: false, unlit: false, wrap: [2, 2] });
        assert_eq!((source.images[0].width, source.images[0].rgba.clone()), (1, vec![255, 0, 0, 255]));
        let animation = &source.animations[0];
        assert_eq!((animation.name.as_str(), animation.channels[0].path, animation.channels[0].times.clone()), ("move", Path3::Translation, vec![0.0, 1.0]));
        assert_eq!(animation.channels[0].values, vec![0.0, 0.0, 0.0, 0.0, 2.0, 0.0]);
    }

    /// The maker of the file, or the name of the node of the meshes, says that the sizes are in the units of the game.
    #[test]
    fn a_file_of_studio_is_in_the_units_of_the_game() {
        let file = small_glb();
        assert!(!read_bytes(&file, None).unwrap().game_units);
        let length = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        let text = std::str::from_utf8(&file[20..20 + length]).unwrap();
        for (old, new) in [("\"name\":\"model\"", "\"name\":\"sango_model.001\""), ("\"version\":\"2.0\"", "\"version\":\"2.0\",\"generator\":\"Sango Studio\"")] {
            let changed = text.replace(old, new);
            assert_ne!(changed, text);
            let mut json = changed.into_bytes();
            while json.len() % 4 != 0 {
                json.push(b' ');
            }
            // The header with the new lengths, then the rest of the file.
            let rest = &file[20 + length..];
            let mut again = file[..8].to_vec();
            again.extend_from_slice(&((20 + json.len() + rest.len()) as u32).to_le_bytes());
            again.extend_from_slice(&(json.len() as u32).to_le_bytes());
            again.extend_from_slice(b"JSON");
            again.extend_from_slice(&json);
            again.extend_from_slice(rest);
            assert!(read_bytes(&again, None).unwrap().game_units, "{new}");
        }
    }

    #[test]
    fn a_bad_file_is_refused_with_a_reason() {
        assert!(read_bytes(b"nope", None).is_err());
        let mut cut = small_glb();
        cut.truncate(60);
        assert!(read_bytes(&cut, None).is_err());
    }

    /// A file that another program wrote again: the node above the bone is a joint too, and it has weights.
    #[test]
    fn a_node_that_takes_a_scale_out_is_merged_with_its_bone() {
        let node = |name: &str, parent: Option<usize>, children: Vec<usize>, translation: [f32; 3]| Node { name: name.into(), parent, children, translation, rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] };
        let channel = |node: usize, path: Path3, values: Vec<f32>| Channel { node, path, times: vec![0.0, 1.0], values };
        let mut source = Source {
            nodes: vec![node("root", None, vec![1], [0.0; 3]), node("arm~scale", Some(0), vec![2], [1.0, 0.0, 0.0]), node("arm", Some(1), vec![], [0.0; 3])],
            joints: vec![0, 1, 2],
            inverse_bind: vec![[0.0; 16], [1.0; 16], [2.0; 16]],
            primitives: vec![Primitive { joints: vec![[2, 1, 0, 0]], ..Primitive::default() }],
            animations: vec![Animation { name: "move".into(), channels: vec![channel(1, Path3::Translation, vec![1.0, 0.0, 0.0, 3.0, 0.0, 0.0]), channel(1, Path3::Scale, vec![0.5; 6]), channel(2, Path3::Translation, vec![0.0; 6])] }],
            ..Source::default()
        };
        merge_scale_nodes(&mut source);
        assert_eq!((source.nodes[2].parent, source.nodes[0].children.clone(), source.nodes[2].translation), (Some(0), vec![2], [1.0, 0.0, 0.0]));
        assert_eq!(source.animations[0].channels, vec![channel(2, Path3::Translation, vec![1.0, 0.0, 0.0, 3.0, 0.0, 0.0])]);
        assert_eq!((source.joints.clone(), source.inverse_bind[1]), (vec![0, 2], [2.0; 16]));
        assert_eq!(source.primitives[0].joints[0], [1, 1, 0, 0]);
        assert_eq!(source.own_scale, vec![2]);
    }

    #[test]
    fn a_matrix_gives_its_translation_rotation_and_scale() {
        // Scale 2 on x, translation (1, 2, 3), no rotation.
        let (translation, rotation, scale) = decompose(&[2.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
        assert_eq!((translation, scale), ([1.0, 2.0, 3.0], [2.0, 1.0, 1.0]));
        assert!((rotation[3] - 1.0).abs() < 1e-6);
    }
}
