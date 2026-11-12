//! Export of a model to a glTF binary file (.glb) and its pictures to PNG files, and import of changed pictures.
//!
//! The .glb file has the skeleton (one node for each bone), the meshes with their weights, and the pictures. The texture
//! positions are written with the transform of the material already applied (the scale and the translation of the
//! first picture), so any viewer shows the pictures where the game does.

use image::ImageEncoder;
use sango_core::model::{Material, Model, Texture};
use sango_core::posing::{rest_worlds, Affine, BoneValues};
use std::collections::HashMap;

/// The bytes of a picture as a PNG file.
pub fn png(texture: &Texture) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&texture.rgba, texture.width as u32, texture.height as u32, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("{}: {e}", texture.name))?;
    Ok(out)
}

/// A picture read from PNG bytes, with a name.
pub fn texture_from_png(name: &str, bytes: &[u8]) -> Result<Texture, String> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
    Ok(Texture { name: name.to_string(), width: image.width() as usize, height: image.height() as usize, rgba: image.into_raw() })
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

/// The animations of the file: for each clip and each bone that it moves, three channels (translation, rotation, scale).
fn animations(model: &Model, clips: &[Clip], buffer: &mut Buffer) -> Vec<String> {
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
            for (path, data, kind) in [("translation", translations, "VEC3"), ("rotation", rotations, "VEC4"), ("scale", scales, "VEC3")] {
                let view = buffer.view(&data, None);
                let output = buffer.accessor(view, 5126, clip.frames.len(), kind, "");
                samplers.push(format!("{{\"input\":{input},\"output\":{output},\"interpolation\":\"LINEAR\"}}"));
                channels.push(format!("{{\"sampler\":{},\"target\":{{\"node\":{bone},\"path\":\"{path}\"}}}}", samplers.len() - 1));
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
    // The skeleton: one node for each bone, then one node for the meshes.
    let rest = rest_worlds(&model.bones);
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); model.bones.len()];
    let mut roots = Vec::new();
    for (i, bone) in model.bones.iter().enumerate() {
        match bone.parent.filter(|&p| p < model.bones.len() && p != i) {
            Some(parent) => children[parent].push(i),
            None => roots.push(i),
        }
    }
    let mut nodes = Vec::new();
    for (i, bone) in model.bones.iter().enumerate() {
        let local = Affine::new(bone.translation, sango_core::posing::rotation_zyx(bone.rotation), [1.0; 3]);
        let q = quaternion(&local.0);
        let child_list = if children[i].is_empty() { String::new() } else { format!(",\"children\":[{}]", children[i].iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")) };
        nodes.push(format!(
            "{{\"name\":{},\"translation\":[{}],\"rotation\":[{}],\"scale\":[{}]{child_list}}}",
            json_text(&bone.name),
            floats(&bone.translation),
            floats(&q),
            floats(&bone.scale)
        ));
    }
    let mesh_node = nodes.len();
    nodes.push("{\"name\":\"model\",\"mesh\":0,\"skin\":0}".to_string());

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
                joints.extend((bone.min(model.bones.len().saturating_sub(1) as u16)).to_le_bytes());
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
    let animation_list = animations(model, clips, &mut buffer);
    buffer.align();

    let join = |list: &[String]| list.join(",");
    let skin_joints = (0..model.bones.len()).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let scene_nodes = roots.iter().map(|r| r.to_string()).chain(std::iter::once(mesh_node.to_string())).collect::<Vec<_>>().join(",");
    let mut json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"Sango Studio\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[{scene_nodes}]}}],\"nodes\":[{}],\"skins\":[{{\"inverseBindMatrices\":{inverse_accessor},\"joints\":[{skin_joints}]}}],\"meshes\":[{{\"primitives\":[{}]}}],\"materials\":[{}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]",
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
        // The text is valid JSON: the quotes and the brackets are balanced.
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        assert_eq!(json.matches('[').count(), json.matches(']').count());
    }

    #[test]
    fn a_picture_goes_to_png_and_comes_back() {
        let texture = Texture { name: "t".into(), width: 2, height: 1, rgba: vec![1, 2, 3, 255, 4, 5, 6, 128] };
        let back = texture_from_png("t", &png(&texture).unwrap()).unwrap();
        assert_eq!(back, texture);
    }

    #[test]
    fn a_rotation_matrix_gives_its_quaternion() {
        let m = Affine::new([0.0; 3], sango_core::posing::rotation_zyx([0.0, 0.0, std::f32::consts::FRAC_PI_2]), [1.0; 3]).0;
        let q = quaternion(&m);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((q[2] - half).abs() < 1e-4 && (q[3] - half).abs() < 1e-4, "{q:?}");
    }
}
