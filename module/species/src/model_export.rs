//! Export of a model to a glTF binary file (.glb) and its pictures to PNG files, and import of changed pictures.
//!
//! The .glb file has the skeleton (one node for each bone), the meshes with their weights, and the pictures. The texture
//! positions are written with the transform of the material already applied (the scale and the translation of the
//! first picture), so any viewer shows the pictures where the game does.

use image::ImageEncoder;
use sango_core::model::{Material, Model, Texture};
use sango_core::motion::{Motion, MotionPack};
use sango_core::posing::{bone_values, rest_worlds, Affine, BoneValues};
use sango_modelport::gltf::{GAME_UNITS_NODE, GENERATOR, SCALE_NODE_SUFFIX};
use std::collections::HashMap;

/// The bytes of a picture as a PNG file.
pub fn png(texture: &Texture) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&texture.rgba, texture.width as u32, texture.height as u32, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("{}: {e}", texture.name))?;
    Ok(out)
}

/// The rotation of a matrix as a quaternion (x, y, z, w).
fn quaternion(m: &[[f32; 4]; 3]) -> [f32; 4] {
    // The scale is taken out of the columns first.
    let column = |c: usize| (m[0][c] * m[0][c] + m[1][c] * m[1][c] + m[2][c] * m[2][c]).sqrt().max(1e-12);
    let r = |row: usize, c: usize| m[row][c] / column(c);
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
    let length = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt().max(1e-12);
    q.map(|v| v / length)
}

/// The text of a string for a JSON file.
fn json_text(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn floats(values: &[f32]) -> String {
    values.iter().map(|v| if v.is_finite() { format!("{v}") } else { "0".to_string() }).collect::<Vec<_>>().join(",")
}

/// The bytes that the binary part of the file collects, and the descriptions of its views.
#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    views: Vec<String>,
    accessors: Vec<String>,
}

impl Buffer {
    fn align(&mut self) {
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
    }

    /// Adds data and returns the number of its view.
    fn view(&mut self, data: &[u8], target: Option<u32>) -> usize {
        self.align();
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(data);
        let target = target.map_or(String::new(), |t| format!(",\"target\":{t}"));
        self.views.push(format!("{{\"buffer\":0,\"byteOffset\":{offset},\"byteLength\":{}{target}}}", data.len()));
        self.views.len() - 1
    }

    fn accessor(&mut self, view: usize, component: u32, count: usize, kind: &str, extra: &str) -> usize {
        self.accessors.push(format!("{{\"bufferView\":{view},\"componentType\":{component},\"count\":{count},\"type\":\"{kind}\"{extra}}}"));
        self.accessors.len() - 1
    }
}

fn bytes_of_f32(values: impl Iterator<Item = f32>) -> Vec<u8> {
    values.flat_map(|v| v.to_le_bytes()).collect()
}

/// A motion for the export: its name, and the values of each bone at each frame (30 frames each second).
pub struct Clip {
    pub name: String,
    pub frames: Vec<Vec<Option<BoneValues>>>,
}

/// The values of each bone at each frame of a motion, from the frame 0 to the last frame of the motion (both are in).
pub fn motion_frames(model: &Model, pack: &MotionPack, motion: &Motion) -> Vec<Vec<Option<BoneValues>>> {
    (0..=motion.frames).map(|frame| bone_values(model, pack, &motion.pose(pack.joints.len(), f32::from(frame)))).collect()
}

/// True when a scale changes a size.
fn is_scaled(scale: [f32; 3]) -> bool {
    scale.iter().any(|s| (s - 1.0).abs() > 1e-5)
}

/// The scale that takes a scale out.
fn opposite(scale: [f32; 3]) -> [f32; 3] {
    scale.map(|s| if s.abs() < 1e-6 { 1.0 } else { 1.0 / s })
}

/// For each bone, the number of the node that stands above it when its parent has a scale in the rest pose or in a clip (see
/// `SCALE_NODE_SUFFIX`). The numbers start at `first`.
fn scale_nodes(model: &Model, clips: &[Clip], first: usize) -> Vec<Option<usize>> {
    let scaled: Vec<bool> = (0..model.bones.len())
        .map(|bone| is_scaled(model.bones[bone].scale) || clips.iter().flat_map(|c| &c.frames).any(|frame| frame.get(bone).copied().flatten().is_some_and(|v| is_scaled(v.scale))))
        .collect();
    let mut next = first;
    model
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            bone.parent.filter(|&p| p != i && scaled.get(p) == Some(&true))?;
            next += 1;
            Some(next - 1)
        })
        .collect()
}

/// The animations of the file: for each clip and each bone that it moves, a channel for its translation, its rotation and its
/// scale. A bone with a node above it (`above`) gives its translation to that node, which also takes the scale of the parent out.
fn animations(model: &Model, clips: &[Clip], above: &[Option<usize>], buffer: &mut Buffer) -> Vec<String> {
    let mut out = Vec::new();
    for clip in clips.iter().filter(|c| c.frames.len() > 1) {
        let times = bytes_of_f32((0..clip.frames.len()).map(|f| f as f32 / 30.0));
        let view = buffer.view(&times, None);
        let last = (clip.frames.len() - 1) as f32 / 30.0;
        let input = buffer.accessor(view, 5126, clip.frames.len(), "SCALAR", &format!(",\"min\":[0],\"max\":[{last}]"));
        let (mut channels, mut samplers) = (Vec::new(), Vec::new());
        for bone in 0..model.bones.len() {
            if clip.frames.iter().all(|f| f.get(bone).copied().flatten().is_none()) {
                continue;
            }
            let at = |frame: &Vec<Option<BoneValues>>| frame.get(bone).copied().flatten().unwrap_or(BoneValues { translation: model.bones[bone].translation, rotation: sango_core::posing::rotation_zyx(model.bones[bone].rotation), scale: model.bones[bone].scale });
            let translations = bytes_of_f32(clip.frames.iter().flat_map(|f| at(f).translation));
            let scales = bytes_of_f32(clip.frames.iter().flat_map(|f| at(f).scale));
            // The quaternions keep the same sign from one frame to the next (else the viewers turn the long way).
            let mut previous: Option<[f32; 4]> = None;
            let rotations = bytes_of_f32(clip.frames.iter().flat_map(|f| {
                let mut q = quaternion(&Affine::new([0.0; 3], at(f).rotation, [1.0; 3]).0);
                if previous.is_some_and(|p| p.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>() < 0.0) {
                    q = q.map(|v| -v);
                }
                previous = Some(q);
                q
            }));
            let holder = above[bone].unwrap_or(bone);
            let mut outputs = vec![(holder, "translation", translations, "VEC3"), (bone, "rotation", rotations, "VEC4"), (bone, "scale", scales, "VEC3")];
            if let (Some(node), Some(parent)) = (above[bone], model.bones[bone].parent) {
                // The scale of the parent at each frame. A bone that the motion does not move keeps the scale of the rest pose.
                let undone = bytes_of_f32(clip.frames.iter().flat_map(|f| {
                    let moved = f.get(bone).copied().flatten().and(f.get(parent).copied().flatten());
                    opposite(moved.map_or(model.bones[parent].scale, |v| v.scale))
                }));
                outputs.push((node, "scale", undone, "VEC3"));
            }
            for (node, path, data, kind) in outputs {
                let view = buffer.view(&data, None);
                let output = buffer.accessor(view, 5126, clip.frames.len(), kind, "");
                samplers.push(format!("{{\"input\":{input},\"output\":{output},\"interpolation\":\"LINEAR\"}}"));
                channels.push(format!("{{\"sampler\":{},\"target\":{{\"node\":{node},\"path\":\"{path}\"}}}}", samplers.len() - 1));
            }
        }
        if !channels.is_empty() {
            out.push(format!("{{\"name\":{},\"channels\":[{}],\"samplers\":[{}]}}", json_text(&clip.name), channels.join(","), samplers.join(",")));
        }
    }
    out
}

/// A model as a .glb file: `textures` are the pictures that the materials use (by name), `clips` the motions.
pub fn glb(model: &Model, textures: &HashMap<String, Texture>, clips: &[Clip]) -> Result<Vec<u8>, String> {
    let mut buffer = Buffer::default();
    // The skeleton: one node for each bone, then one node for the meshes, then the nodes that stand above the bones whose
    // parent has a scale.
    let rest = rest_worlds(&model.bones);
    let above = scale_nodes(model, clips, model.bones.len() + 1);
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); model.bones.len()];
    let mut roots = Vec::new();
    for (i, bone) in model.bones.iter().enumerate() {
        match bone.parent.filter(|&p| p < model.bones.len() && p != i) {
            Some(parent) => children[parent].push(above[i].unwrap_or(i)),
            None => roots.push(i),
        }
    }
    let mut nodes = Vec::new();
    for (i, bone) in model.bones.iter().enumerate() {
        let local = Affine::new(bone.translation, sango_core::posing::rotation_zyx(bone.rotation), [1.0; 3]);
        let q = quaternion(&local.0);
        let child_list = if children[i].is_empty() { String::new() } else { format!(",\"children\":[{}]", children[i].iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")) };
        let translation = if above[i].is_some() { [0.0; 3] } else { bone.translation };
        nodes.push(format!(
            "{{\"name\":{},\"translation\":[{}],\"rotation\":[{}],\"scale\":[{}]{child_list}}}",
            json_text(&bone.name),
            floats(&translation),
            floats(&q),
            floats(&bone.scale)
        ));
    }
    let mesh_node = nodes.len();
    nodes.push(format!("{{\"name\":\"{GAME_UNITS_NODE}\",\"mesh\":0,\"skin\":0}}"));
    for (i, bone) in model.bones.iter().enumerate() {
        let (Some(_), Some(parent)) = (above[i], bone.parent) else { continue };
        nodes.push(format!(
            "{{\"name\":{},\"translation\":[{}],\"scale\":[{}],\"children\":[{i}]}}",
            json_text(&format!("{}{SCALE_NODE_SUFFIX}", bone.name)),
            floats(&bone.translation),
            floats(&opposite(model.bones[parent].scale))
        ));
    }

    // The skin: the inverse of the world matrix of each bone in the rest pose (column by column).
    let mut inverse = Vec::new();
    for world in &rest {
        let m = world.inverse().unwrap_or(Affine::IDENTITY).0;
        for column in 0..4 {
            inverse.extend([m[0][column], m[1][column], m[2][column], if column == 3 { 1.0 } else { 0.0 }]);
        }
    }
    let inverse_view = buffer.view(&bytes_of_f32(inverse.into_iter()), None);
    let inverse_accessor = buffer.accessor(inverse_view, 5126, model.bones.len(), "MAT4", "");

    // The pictures that the materials use, in the order of the materials.
    let mut image_of: HashMap<String, usize> = HashMap::new();
    let (mut images, mut gltf_textures, mut samplers) = (Vec::new(), Vec::new(), Vec::new());
    let mut materials = Vec::new();
    for material in &model.materials {
        let unit = &material.units[0];
        let name = if textures.contains_key(&material.texture) { material.texture.clone() } else { unit.texture.clone() };
        let mut pbr = String::from("\"pbrMetallicRoughness\":{\"metallicFactor\":0,\"roughnessFactor\":1");
        if let Some(texture) = textures.get(&name) {
            let image = match image_of.get(&name) {
                Some(&i) => i,
                None => {
                    let view = buffer.view(&png(texture)?, None);
                    images.push(format!("{{\"name\":{},\"bufferView\":{view},\"mimeType\":\"image/png\"}}", json_text(&name)));
                    image_of.insert(name.clone(), images.len() - 1);
                    images.len() - 1
                }
            };
            let wrap = |kind: u8| match kind {
                0 | 1 => 33071,
                3 => 33648,
                _ => 10497,
            };
            samplers.push(format!("{{\"wrapS\":{},\"wrapT\":{},\"magFilter\":9729,\"minFilter\":9729}}", wrap(unit.wrap[0]), wrap(unit.wrap[1])));
            gltf_textures.push(format!("{{\"source\":{image},\"sampler\":{}}}", samplers.len() - 1));
            pbr.push_str(&format!(",\"baseColorTexture\":{{\"index\":{}}}", gltf_textures.len() - 1));
        }
        pbr.push('}');
        let blend = material.state.known && material.state.blend && (material.state.color_source, material.state.color_destination) != (1, 0);
        let mode = if blend { "BLEND" } else { "OPAQUE" };
        materials.push(format!("{{\"name\":{},{pbr},\"alphaMode\":\"{mode}\",\"doubleSided\":true}}", json_text(&material.name)));
    }

    // The meshes.
    let mut primitives = Vec::new();
    for mesh in &model.meshes {
        if mesh.triangles.is_empty() || mesh.vertices.is_empty() {
            continue;
        }
        let material: &Material = model.materials.get(mesh.material).ok_or("A mesh has no material.")?;
        let unit = &material.units[0];
        let positions = bytes_of_f32(mesh.vertices.iter().flat_map(|v| v.position));
        let normals = bytes_of_f32(mesh.vertices.iter().flat_map(|v| v.normal));
        let uvs = bytes_of_f32(mesh.vertices.iter().flat_map(|v| [unit.scale[0] * (v.uv[0] - unit.translation[0]), unit.scale[1] * (v.uv[1] - unit.translation[1])]));
        let mut joints = Vec::new();
        let mut weights = Vec::new();
        for v in &mesh.vertices {
            let total: f32 = v.weights.iter().sum();
            for k in 0..4 {
                let (bone, weight) = if total > 0.0 { (v.bones[k], v.weights[k] / total) } else { (0, if k == 0 { 1.0 } else { 0.0 }) };
                // A place with no weight names the joint 0, as the rules of glTF ask.
                let bone = if weight > 0.0 { bone.min(model.bones.len().saturating_sub(1) as u16) } else { 0 };
                joints.extend(bone.to_le_bytes());
                weights.extend(weight.to_le_bytes());
            }
        }
        let indices: Vec<u8> = mesh.triangles.iter().flatten().flat_map(|&i| i.to_le_bytes()).collect();
        let count = mesh.vertices.len();
        let (low, high) = mesh.vertices.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), v| (std::array::from_fn(|k| lo[k].min(v.position[k])), std::array::from_fn(|k| hi[k].max(v.position[k]))));
        let view = buffer.view(&positions, Some(34962));
        let position = buffer.accessor(view, 5126, count, "VEC3", &format!(",\"min\":[{}],\"max\":[{}]", floats(&low), floats(&high)));
        let view = buffer.view(&normals, Some(34962));
        let normal = buffer.accessor(view, 5126, count, "VEC3", "");
        let view = buffer.view(&uvs, Some(34962));
        let uv = buffer.accessor(view, 5126, count, "VEC2", "");
        let view = buffer.view(&joints, Some(34962));
        let joint = buffer.accessor(view, 5123, count, "VEC4", "");
        let view = buffer.view(&weights, Some(34962));
        let weight = buffer.accessor(view, 5126, count, "VEC4", "");
        let view = buffer.view(&indices, Some(34963));
        let index = buffer.accessor(view, 5125, mesh.triangles.len() * 3, "SCALAR", "");
        primitives.push(format!(
            "{{\"attributes\":{{\"POSITION\":{position},\"NORMAL\":{normal},\"TEXCOORD_0\":{uv},\"JOINTS_0\":{joint},\"WEIGHTS_0\":{weight}}},\"indices\":{index},\"material\":{}}}",
            mesh.material
        ));
    }
    if primitives.is_empty() {
        return Err("The model has no triangle.".into());
    }
    let animation_list = animations(model, clips, &above, &mut buffer);
    buffer.align();

    let join = |list: &[String]| list.join(",");
    let skin_joints = (0..model.bones.len()).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let scene_nodes = roots.iter().map(|r| r.to_string()).chain(std::iter::once(mesh_node.to_string())).collect::<Vec<_>>().join(",");
    let mut json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"{GENERATOR}\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[{scene_nodes}]}}],\"nodes\":[{}],\"skins\":[{{\"inverseBindMatrices\":{inverse_accessor},\"joints\":[{skin_joints}]}}],\"meshes\":[{{\"primitives\":[{}]}}],\"materials\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]",
        join(&nodes),
        join(&primitives),
        join(&materials),
        join(&buffer.accessors),
        join(&buffer.views),
        buffer.bytes.len()
    );
    if !animation_list.is_empty() {
        json.push_str(&format!(",\"animations\":[{}]", join(&animation_list)));
    }
    if !images.is_empty() {
        json.push_str(&format!(",\"images\":[{}],\"textures\":[{}],\"samplers\":[{}]", join(&images), join(&gltf_textures), join(&samplers)));
    }
    json.push('}');
    let mut json_bytes = json.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + buffer.bytes.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(buffer.bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&buffer.bytes);
    Ok(out)
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::model::{Mesh, Unit, Vertex};

    fn small_model() -> (Model, HashMap<String, Texture>) {
        let vertex = |x: f32, y: f32| Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv: [x, y], weights: [1.0, 0.0, 0.0, 0.0], ..Vertex::default() };
        let mesh = Mesh { material: 0, vertices: vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(0.0, 1.0)], triangles: vec![[0, 1, 2]] };
        let bone = sango_core::model::Bone { name: "root".into(), parent: None, scale: [1.0; 3], rotation: [0.0; 3], translation: [0.0; 3] };
        let material = Material { name: "m".into(), texture: "t".into(), units: [Unit { texture: "t".into(), scale: [1.0, 1.0], translation: [0.0, 0.0], wrap: [2, 2] }, Unit::default(), Unit::default()], ..Material::default() };
        let texture = Texture { name: "t".into(), width: 2, height: 2, rgba: vec![255; 16] };
        (Model { bones: vec![bone], materials: vec![material], meshes: vec![mesh] }, HashMap::from([("t".to_string(), texture)]))
    }

    #[test]
    fn a_model_is_written_as_a_glb_file() {
        let (model, textures) = small_model();
        let clip = Clip { name: "move".into(), frames: vec![vec![Some(BoneValues { translation: [0.0; 3], rotation: sango_core::posing::rotation_zyx([0.0; 3]), scale: [1.0; 3] })]; 3] };
        let file = glb(&model, &textures, &[clip]).unwrap();
        assert_eq!(&file[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(file[8..12].try_into().unwrap()) as usize, file.len(), "the length in the header");
        let json_length = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&file[20..20 + json_length]).unwrap();
        assert!(json.contains("\"JOINTS_0\"") && json.contains("\"image/png\"") && json.contains("\"skins\"") && json.contains("\"animations\""), "{json}");
        assert!(sango_modelport::gltf::read_bytes(&file, None).unwrap().game_units, "the file says its units");
        // The text is valid JSON: the quotes and the brackets are balanced.
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        assert_eq!(json.matches('[').count(), json.matches(']').count());
    }

    #[test]
    fn a_place_with_no_weight_names_the_first_joint() {
        let (mut model, textures) = small_model();
        let mut second = model.bones[0].clone();
        (second.name, second.parent) = ("second".into(), Some(0));
        model.bones.push(second);
        for vertex in &mut model.meshes[0].vertices {
            vertex.bones = [1; 4];
        }
        let file = glb(&model, &textures, &[]).unwrap();
        let source = sango_modelport::gltf::read_bytes(&file, None).unwrap();
        assert_eq!(source.primitives[0].joints[0], [1, 0, 0, 0]);
    }

    #[test]
    fn a_bone_under_a_scaled_parent_gets_a_node_that_takes_the_scale_out() {
        let (mut model, textures) = small_model();
        let mut second = model.bones[0].clone();
        (second.name, second.parent, second.translation) = ("second".into(), Some(0), [1.0, 0.0, 0.0]);
        model.bones.push(second);
        let values = |translation: [f32; 3], scale: [f32; 3]| Some(BoneValues { translation, rotation: sango_core::posing::rotation_zyx([0.0; 3]), scale });
        let clip = Clip { name: "grow".into(), frames: vec![vec![values([0.0; 3], [2.0, 1.0, 1.0]), values([1.0, 0.0, 0.0], [1.0; 3])]; 2] };
        let file = glb(&model, &textures, &[clip]).unwrap();
        let length = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&file[20..20 + length]).unwrap();
        assert!(json.contains("\"name\":\"second~scale\",\"translation\":[1,0,0],\"scale\":[1,1,1],\"children\":[1]"), "{json}");
        // The reader of Studio gives the skeleton of the bones again, with the translation on the bone.
        let source = sango_modelport::gltf::read_bytes(&file, None).unwrap();
        assert_eq!((source.nodes[1].parent, source.nodes[1].translation), (Some(0), [1.0, 0.0, 0.0]));
        assert!(source.animations[0].channels.iter().all(|c| c.node < 2));
    }

    /// With the dump: the GLB file of a model, played as glTF says, moves like the viewer of Studio.
    #[test]
    fn the_motions_of_the_file_move_like_the_viewer() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let archive = sango_core::Garc::open(&dump.archive(8)).unwrap();
        // Bulbasaur, Pikachu, Muk and Chandelure: their motions scale bones that have children.
        for species in [1usize, 25, 89, 609] {
            let list = archive.first_file(0).unwrap();
            let first = u16::from_le_bytes([list[(species - 1) * 4], list[(species - 1) * 4 + 1]]) as usize * 8 + 3;
            let (model, _) = crate::model_tab::read_model_for_test(&archive, first, false).unwrap();
            let pack = crate::model_tab::battle_motions_for_test(&archive, first).unwrap();
            let motion = pack.motion(0).unwrap();
            let file = crate::model_tab::glb_of_game_model(&archive, species).unwrap();
            let player = Player::read(&file);
            for frame in [0, motion.frames / 3, motion.frames / 2] {
                let expected = sango_core::posing::posed(&model, &pack, &motion.pose(pack.joints.len(), f32::from(frame)));
                let played = player.play(&sango_modelport::animate::motion_name(true, 0), usize::from(frame));
                let mut worst = 0.0f32;
                let drawn = expected.meshes.iter().filter(|m| !m.triangles.is_empty() && !m.vertices.is_empty());
                for (mesh, positions) in drawn.zip(&played) {
                    for (vertex, position) in mesh.vertices.iter().zip(positions) {
                        worst = (0..3).fold(worst, |w, k| w.max((vertex.position[k] - position[k]).abs()));
                    }
                }
                assert!(worst < 0.01, "species {species} frame {frame}: a vertex is {worst} away");
            }
        }
    }

    /// A viewer of the tests: it moves the vertices of a GLB file as the rules of glTF say.
    struct Player {
        json: serde_json::Value,
        data: Vec<u8>,
    }

    impl Player {
        fn read(file: &[u8]) -> Player {
            let length = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
            Player { json: serde_json::from_slice(&file[20..20 + length]).unwrap(), data: file[28 + length..].to_vec() }
        }

        /// The numbers of an accessor.
        fn numbers(&self, accessor: &serde_json::Value) -> Vec<f32> {
            let accessor = &self.json["accessors"][accessor.as_u64().unwrap() as usize];
            let view = &self.json["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
            let width = match accessor["type"].as_str().unwrap() {
                "SCALAR" => 1,
                "VEC3" => 3,
                "VEC4" => 4,
                _ => 16,
            };
            let count = accessor["count"].as_u64().unwrap() as usize * width;
            let start = view["byteOffset"].as_u64().unwrap() as usize;
            let kind = accessor["componentType"].as_u64().unwrap();
            let size = if kind == 5123 { 2 } else { 4 };
            self.data[start..start + count * size]
                .chunks(size)
                .map(|b| match kind {
                    5123 => f32::from(u16::from_le_bytes([b[0], b[1]])),
                    5125 => u32::from_le_bytes(b.try_into().unwrap()) as f32,
                    _ => f32::from_le_bytes(b.try_into().unwrap()),
                })
                .collect()
        }

        /// The positions of the vertices of each part of the mesh at a frame of an animation.
        fn play(&self, name: &str, frame: usize) -> Vec<Vec<[f32; 3]>> {
            let nodes = self.json["nodes"].as_array().unwrap();
            let read = |node: &serde_json::Value, key: &str, rest: [f32; 4]| -> [f32; 4] { std::array::from_fn(|k| node[key][k].as_f64().map_or(rest[k], |v| v as f32)) };
            let mut values: Vec<[[f32; 4]; 3]> = nodes.iter().map(|n| [read(n, "translation", [0.0; 4]), read(n, "rotation", [0.0, 0.0, 0.0, 1.0]), read(n, "scale", [1.0; 4])]).collect();
            let animation = self.json["animations"].as_array().unwrap().iter().find(|a| a["name"] == name).expect("the animation");
            for channel in animation["channels"].as_array().unwrap() {
                let output = self.numbers(&animation["samplers"][channel["sampler"].as_u64().unwrap() as usize]["output"]);
                let (path, width) = match channel["target"]["path"].as_str().unwrap() {
                    "translation" => (0, 3),
                    "rotation" => (1, 4),
                    _ => (2, 3),
                };
                let target = &mut values[channel["target"]["node"].as_u64().unwrap() as usize][path];
                target[..width].copy_from_slice(&output[frame * width..(frame + 1) * width]);
            }
            let mut parents = vec![None; nodes.len()];
            for (parent, node) in nodes.iter().enumerate() {
                for child in node["children"].as_array().into_iter().flatten() {
                    parents[child.as_u64().unwrap() as usize] = Some(parent);
                }
            }
            let local = |i: usize| {
                let [t, r, s] = values[i];
                Affine::new([t[0], t[1], t[2]], sango_core::posing::rotation_quaternion(r), [s[0], s[1], s[2]])
            };
            let world = |i: usize| {
                let (mut matrix, mut at) = (local(i), i);
                while let Some(parent) = parents[at] {
                    (matrix, at) = (local(parent).then(&matrix), parent);
                }
                matrix
            };
            let skin = &self.json["skins"][0];
            let inverse = self.numbers(&skin["inverseBindMatrices"]);
            let joints: Vec<Affine> = skin["joints"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(k, joint)| {
                    let m = &inverse[k * 16..(k + 1) * 16];
                    world(joint.as_u64().unwrap() as usize).then(&Affine(std::array::from_fn(|row| std::array::from_fn(|column| m[column * 4 + row]))))
                })
                .collect();
            self.json["meshes"][0]["primitives"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| {
                    let attributes = &part["attributes"];
                    let (positions, bones, weights) = (self.numbers(&attributes["POSITION"]), self.numbers(&attributes["JOINTS_0"]), self.numbers(&attributes["WEIGHTS_0"]));
                    positions
                        .chunks(3)
                        .enumerate()
                        .map(|(v, p)| {
                            let mut out = [0.0f32; 3];
                            for k in 0..4 {
                                let moved = joints[bones[v * 4 + k] as usize].point([p[0], p[1], p[2]]);
                                for axis in 0..3 {
                                    out[axis] += weights[v * 4 + k] * moved[axis];
                                }
                            }
                            out
                        })
                        .collect()
                })
                .collect()
        }
    }

    #[test]
    fn a_rotation_matrix_gives_its_quaternion() {
        let m = Affine::new([0.0; 3], sango_core::posing::rotation_zyx([0.0, 0.0, std::f32::consts::FRAC_PI_2]), [1.0; 3]).0;
        let q = quaternion(&m);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((q[2] - half).abs() < 1e-4 && (q[3] - half).abs() < 1e-4, "{q:?}");
    }
}
