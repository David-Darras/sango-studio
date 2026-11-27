//! Export of a model to a glTF binary file (.glb) and its pictures to PNG files, and import of changed pictures.
//!
//! The .glb file has the skeleton (one node for each bone), the meshes with their weights, and the pictures. The texture
//! positions are written with the transform of the material already applied (the scale and the translation of the
//! first picture), so any viewer shows the pictures where the game does.

use image::ImageEncoder;
use sango_core::model::{Material, Model, Texture};
use sango_core::motion::{Motion, MotionPack};
use sango_core::posing::{bone_values, rest_worlds, Affine, BoneValues};
use sango_core::render::{is_cut, is_mixed, Surface};
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

/// How some meshes look in the file: one material of glTF.
struct Look {
    name: String,
    /// The picture, with the key that tells two pictures apart.
    picture: Option<(String, Texture)>,
    /// The place in the picture of a texture position of a mesh: `scale * (position - translation)`.
    scale: [f32; 2],
    translation: [f32; 2],
    wrap: [u8; 2],
    /// The way glTF treats the opacity: "OPAQUE", "MASK" (a pixel is drawn or not) or "BLEND".
    mode: &'static str,
    unlit: bool,
}

impl Look {
    /// True when the file says that the light does not touch the material. This is only said of a material that lets what is
    /// behind it show: Blender does not draw an opaque material of this kind well behind a transparent one.
    fn shows_as_it_is(&self) -> bool {
        self.unlit && self.mode != "OPAQUE"
    }
}

/// The largest side of a picture that the export makes.
const LARGEST_SIDE: f32 = 512.0;

/// The looks of the meshes at the places `drawn`, and the look of each of these meshes.
///
/// A material that shows one picture keeps it, with the opacity that the viewer gives it. A material that mixes several
/// pictures (or an iris, that the eye under it cuts) gets a new picture that holds the result, over the texture positions that
/// its meshes use.
fn looks(model: &Model, textures: &HashMap<String, Texture>, drawn: &[usize]) -> Result<(Vec<Look>, Vec<usize>), String> {
    // The meshes that share a look: the same material, over the same eye.
    let mut groups: Vec<((usize, Option<String>), Vec<usize>)> = Vec::new();
    let mut group_of = Vec::with_capacity(drawn.len());
    for &place in drawn {
        let mesh = &model.meshes[place];
        let surface = Surface::new(model, textures, mesh).ok_or("A mesh has no material.")?;
        let key = (mesh.material, surface.eye_picture().map(str::to_string));
        let group = groups.iter().position(|(k, _)| *k == key).unwrap_or_else(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        groups[group].1.push(place);
        group_of.push(group);
    }
    let mut out: Vec<Look> = Vec::with_capacity(groups.len());
    for ((material, eye), places) in &groups {
        let material = &model.materials[*material];
        let surface = Surface::new(model, textures, &model.meshes[places[0]]).ok_or("A mesh has no material.")?;
        let combined = surface.is_combined();
        // A name that no other look has.
        let mut name = material.name.clone();
        let mut number = 1;
        while out.iter().any(|look| look.name == name) {
            number += 1;
            name = format!("{} {number}", material.name);
        }
        let mut look = Look { name, picture: None, scale: material.texture_scale, translation: material.texture_translation, wrap: material.texture_wrap, mode: "OPAQUE", unlit: material.unlit };
        if !combined && eye.is_none() {
            if let Some(texture) = textures.get(&material.texture) {
                let mut picture = texture.clone();
                for pixel in picture.rgba.chunks_exact_mut(4) {
                    let cut = is_cut(Some(material), [pixel[0], pixel[1], pixel[2], pixel[3]]);
                    pixel[3] = if cut { 0 } else { 255 };
                    if cut {
                        look.mode = "MASK";
                    }
                }
                look.picture = Some((format!("{} {}", texture.name, material.transparent), picture));
            }
        } else {
            bake(model, textures, material, &surface, places, &mut look);
        }
        out.push(look);
    }
    Ok((out, group_of))
}

/// Makes the picture of a look: what the surface shows over the texture positions that the meshes use.
fn bake(model: &Model, textures: &HashMap<String, Texture>, material: &Material, surface: &Surface, places: &[usize], look: &mut Look) {
    let vertices = || places.iter().flat_map(|&place| &model.meshes[place].vertices);
    let low: [f32; 2] = std::array::from_fn(|k| vertices().map(|v| v.uv[k]).fold(f32::MAX, f32::min));
    let high: [f32; 2] = std::array::from_fn(|k| vertices().map(|v| v.uv[k]).fold(f32::MIN, f32::max));
    let extent: [f32; 2] = std::array::from_fn(|k| (high[k] - low[k]).max(1.0 / 256.0));
    // As many pixels as the finest of the pictures has over the same positions, in a side that is a power of two.
    let used = || std::iter::once((&material.texture, material.texture_scale)).chain(material.units.iter().map(|u| (&u.texture, u.scale)));
    let sides: [usize; 2] = std::array::from_fn(|k| {
        let finest = used().filter_map(|(name, scale)| textures.get(name).map(|t| [t.width, t.height][k] as f32 * scale[k].abs() * extent[k])).fold(8.0f32, f32::max);
        (finest.min(LARGEST_SIDE).round() as usize).next_power_of_two()
    });
    // The way the colour goes over what is behind, when glTF has no such way: the colour adds itself, or it multiplies.
    let state = &material.state;
    let factors = (state.color_source, state.color_destination);
    let blends = surface.is_combined() && state.blend;
    let additive = blends && factors.1 == 1;
    let multiply = blends && (factors == (4, 0) || factors == (0, 2));
    let shown = |uv: [f32; 2], primary: [f32; 4]| -> [u8; 4] {
        let mut color = surface.at(uv, primary);
        if additive {
            // Black adds nothing, so the light of a colour is its opacity.
            let light = color[0].max(color[1]).max(color[2]);
            color = if light > 0.0 { [color[0] / light, color[1] / light, color[2] / light, color[3] * light] } else { [0.0; 4] };
        } else if multiply {
            // White changes nothing, so the darkness of a colour is the opacity of a black over what is behind.
            let light = 0.299 * color[0] + 0.587 * color[1] + 0.114 * color[2];
            color = [0.0, 0.0, 0.0, if color[3] > 0.0 { 1.0 - light } else { 0.0 }];
        }
        let mut color = color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        // A place that is not drawn keeps the colour of the picture: a program that does not read the opacity then shows the
        // skin around an eye, not the colour that the mix gives there.
        if color[3] == 0 && !additive && !multiply {
            if let Some(plain) = surface.picture_color(uv) {
                color[..3].copy_from_slice(&plain[..3]);
            }
        }
        color
    };
    // The colour of the vertices, that the steps of the mix may use: the mean one everywhere, then the colour of each place
    // inside the triangles when the vertices do not all have the same one.
    let count = vertices().count().max(1) as f32;
    let mean: [f32; 4] = std::array::from_fn(|k| vertices().map(|v| f32::from(v.color[k])).sum::<f32>() / count / 255.0);
    let first = vertices().next().map_or([255; 4], |v| v.color);
    let uses_vertices = surface.is_combined() && material.stages.iter().any(|s| s.color_source.contains(&0) || s.alpha_source.contains(&0));
    let [width, height] = sides;
    let place = |x: usize, y: usize| [low[0] + (x as f32 + 0.5) / width as f32 * extent[0], low[1] + (y as f32 + 0.5) / height as f32 * extent[1]];
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend(shown(place(x, y), mean));
        }
    }
    if uses_vertices && vertices().any(|v| v.color != first) {
        for mesh in places.iter().map(|&at| &model.meshes[at]) {
            for triangle in &mesh.triangles {
                let Some(corners) = triangle.iter().map(|&k| mesh.vertices.get(k as usize)).collect::<Option<Vec<_>>>() else { continue };
                // The corners in the pixels of the picture.
                let at: [[f32; 2]; 3] = std::array::from_fn(|c| [(corners[c].uv[0] - low[0]) / extent[0] * width as f32, (corners[c].uv[1] - low[1]) / extent[1] * height as f32]);
                let area = (at[1][0] - at[0][0]) * (at[2][1] - at[0][1]) - (at[2][0] - at[0][0]) * (at[1][1] - at[0][1]);
                if area.abs() < 1e-6 {
                    continue;
                }
                let range = |axis: usize, limit: usize| {
                    let from = at.iter().map(|p| p[axis]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
                    let to = at.iter().map(|p| p[axis]).fold(f32::MIN, f32::max).ceil().max(0.0) as usize;
                    from..to.min(limit)
                };
                for y in range(1, height) {
                    for x in range(0, width) {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let first = ((at[1][0] - px) * (at[2][1] - py) - (at[2][0] - px) * (at[1][1] - py)) / area;
                        let second = ((at[2][0] - px) * (at[0][1] - py) - (at[0][0] - px) * (at[2][1] - py)) / area;
                        let weights = [first, second, 1.0 - first - second];
                        if weights.iter().any(|&w| w < -0.01) {
                            continue;
                        }
                        let primary = std::array::from_fn(|k| (0..3).map(|c| weights[c] * f32::from(corners[c].color[k])).sum::<f32>() / 255.0);
                        rgba[(y * width + x) * 4..][..4].copy_from_slice(&shown(place(x, y), primary));
                    }
                }
            }
        }
    }
    let see_through = rgba.chunks_exact(4).any(|p| p[3] < 255);
    look.mode = match (see_through, additive || multiply || is_mixed(material)) {
        (false, _) => "OPAQUE",
        (true, true) => "BLEND",
        (true, false) => "MASK",
    };
    // The new picture covers the texture positions from `low` to `low + extent`.
    look.scale = [1.0 / extent[0], 1.0 / extent[1]];
    look.translation = low;
    look.wrap = [0, 0];
    look.picture = Some((format!("baked {}", look.name), Texture { name: look.name.clone(), width, height, rgba }));
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

    // The materials: what the viewer shows on each mesh.
    let drawn: Vec<usize> = (0..model.meshes.len()).filter(|&i| !model.meshes[i].triangles.is_empty() && !model.meshes[i].vertices.is_empty()).collect();
    let (looks, look_of) = looks(model, textures, &drawn)?;
    let mut image_of: HashMap<String, usize> = HashMap::new();
    let (mut images, mut gltf_textures, mut samplers) = (Vec::new(), Vec::new(), Vec::new());
    let mut materials = Vec::new();
    for look in &looks {
        let mut pbr = String::from("\"pbrMetallicRoughness\":{\"metallicFactor\":0,\"roughnessFactor\":1");
        if let Some((key, picture)) = &look.picture {
            let image = match image_of.get(key) {
                Some(&i) => i,
                None => {
                    let view = buffer.view(&png(picture)?, None);
                    images.push(format!("{{\"name\":{},\"bufferView\":{view},\"mimeType\":\"image/png\"}}", json_text(&picture.name)));
                    image_of.insert(key.clone(), images.len() - 1);
                    images.len() - 1
                }
            };
            let wrap = |kind: u8| match kind {
                0 | 1 => 33071,
                3 => 33648,
                _ => 10497,
            };
            samplers.push(format!("{{\"wrapS\":{},\"wrapT\":{},\"magFilter\":9729,\"minFilter\":9729}}", wrap(look.wrap[0]), wrap(look.wrap[1])));
            gltf_textures.push(format!("{{\"source\":{image},\"sampler\":{}}}", samplers.len() - 1));
            pbr.push_str(&format!(",\"baseColorTexture\":{{\"index\":{}}}", gltf_textures.len() - 1));
        }
        pbr.push('}');
        let cutoff = if look.mode == "MASK" { ",\"alphaCutoff\":0.5" } else { "" };
        // A material that the light of the game does not touch shows its colours as they are.
        let unlit = if look.shows_as_it_is() { ",\"extensions\":{\"KHR_materials_unlit\":{}}" } else { "" };
        materials.push(format!("{{\"name\":{},{pbr},\"alphaMode\":\"{}\"{cutoff},\"doubleSided\":true{unlit}}}", json_text(&look.name), look.mode));
    }

    // The meshes.
    let mut primitives = Vec::new();
    for (&place, &material) in drawn.iter().zip(&look_of) {
        let (mesh, look) = (&model.meshes[place], &looks[material]);
        let positions = bytes_of_f32(mesh.vertices.iter().flat_map(|v| v.position));
        let normals = bytes_of_f32(mesh.vertices.iter().flat_map(|v| v.normal));
        let uvs = bytes_of_f32(mesh.vertices.iter().flat_map(|v| [look.scale[0] * (v.uv[0] - look.translation[0]), look.scale[1] * (v.uv[1] - look.translation[1])]));
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
            "{{\"attributes\":{{\"POSITION\":{position},\"NORMAL\":{normal},\"TEXCOORD_0\":{uv},\"JOINTS_0\":{joint},\"WEIGHTS_0\":{weight}}},\"indices\":{index},\"material\":{material}}}"
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
    if looks.iter().any(Look::shows_as_it_is) {
        json.push_str(",\"extensionsUsed\":[\"KHR_materials_unlit\"]");
    }
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
        let material = Material { name: "m".into(), texture: "t".into(), texture_scale: [1.0, 1.0], texture_wrap: [2, 2], units: [Unit { texture: "t".into(), scale: [1.0, 1.0], translation: [0.0, 0.0], wrap: [2, 2] }, Unit::default(), Unit::default()], ..Material::default() };
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
    fn a_plain_picture_keeps_its_name_and_shows_what_the_viewer_cuts() {
        let (model, mut textures) = small_model();
        // Black with no opacity is cut by the viewer. A colour with no opacity is drawn.
        textures.get_mut("t").unwrap().rgba = vec![0, 0, 0, 0, 200, 10, 10, 0, 9, 9, 9, 255, 255, 255, 255, 255];
        let source = sango_modelport::gltf::read_bytes(&glb(&model, &textures, &[]).unwrap(), None).unwrap();
        assert_eq!(source.images[0].name, "t");
        let opacity: Vec<u8> = source.images[0].rgba.chunks(4).map(|p| p[3]).collect();
        assert_eq!(opacity, [0, 255, 255, 255]);
        assert!(source.materials[0].blend, "the material cuts pixels");
        // Without a pixel to cut, the material is opaque.
        textures.get_mut("t").unwrap().rgba[..4].copy_from_slice(&[50, 0, 0, 0]);
        let source = sango_modelport::gltf::read_bytes(&glb(&model, &textures, &[]).unwrap(), None).unwrap();
        assert!(!source.materials[0].blend && source.images[0].rgba.chunks(4).all(|p| p[3] == 255));
    }

    #[test]
    fn a_material_that_mixes_gets_the_picture_of_the_result() {
        use sango_core::model::{RenderState, Stage};
        let (mut model, textures) = small_model();
        // The white picture times a red of the step, blended, on a material that the light does not touch.
        let step = Stage { color_source: [3, 14, 15], color_mode: 1, alpha_source: [3, 14, 15], alpha_mode: 1, constant: [200, 0, 0, 128], ..Stage::default() };
        let material = &mut model.materials[0];
        material.stages = vec![step];
        material.unlit = true;
        material.state = RenderState { known: true, blend: true, color_source: 6, color_destination: 7, ..RenderState::default() };
        // The texture positions go from 1 to 3: the new picture covers them, and the export gives positions from 0 to 1.
        for vertex in &mut model.meshes[0].vertices {
            vertex.uv = [1.0 + 2.0 * vertex.uv[0], 1.0 + 2.0 * vertex.uv[1]];
        }
        let file = glb(&model, &textures, &[]).unwrap();
        let source = sango_modelport::gltf::read_bytes(&file, None).unwrap();
        let picture = &source.images[0];
        assert_eq!((picture.name.as_str(), &picture.rgba[..4]), ("m", [200, 0, 0, 128].as_slice()));
        assert!(picture.width.is_power_of_two() && picture.width >= 4);
        assert_eq!(source.primitives[0].uvs, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        let length = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&file[20..20 + length]).unwrap();
        assert!(json.contains("\"alphaMode\":\"BLEND\"") && json.contains("\"extensionsUsed\":[\"KHR_materials_unlit\"]"), "{json}");
    }

    #[test]
    fn a_material_that_multiplies_darkens_with_a_black_picture() {
        use sango_core::model::{RenderState, Stage};
        let (mut model, textures) = small_model();
        // A grey of a quarter, multiplied with what is behind.
        let step = Stage { color_source: [14, 15, 15], alpha_source: [3, 15, 15], constant: [64, 64, 64, 255], ..Stage::default() };
        let material = &mut model.materials[0];
        material.stages = vec![step];
        material.state = RenderState { known: true, blend: true, color_source: 4, color_destination: 0, ..RenderState::default() };
        let source = sango_modelport::gltf::read_bytes(&glb(&model, &textures, &[]).unwrap(), None).unwrap();
        assert_eq!(&source.images[0].rgba[..4], [0, 0, 0, 191].as_slice());
        // The new colour takes the place of the old one: every drawn pixel is opaque.
        model.materials[0].state.color_source = 1;
        model.materials[0].unlit = true;
        model.materials[0].stages[0].alpha_source = [14, 15, 15];
        model.materials[0].stages[0].constant = [64, 64, 64, 40];
        let source = sango_modelport::gltf::read_bytes(&glb(&model, &textures, &[]).unwrap(), None).unwrap();
        assert_eq!(&source.images[0].rgba[..4], [64, 64, 64, 255].as_slice());
        assert!(!source.materials[0].blend);
    }

    #[test]
    fn a_rotation_matrix_gives_its_quaternion() {
        let m = Affine::new([0.0; 3], sango_core::posing::rotation_zyx([0.0, 0.0, std::f32::consts::FRAC_PI_2]), [1.0; 3]).0;
        let q = quaternion(&m);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((q[2] - half).abs() < 1e-4 && (q[3] - half).abs() < 1e-4, "{q:?}");
    }
}
