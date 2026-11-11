//! A small drawing of a model on the processor: no graphics card is needed.
//!
//! The model is drawn in the pose of its file, from a point of view that turns around it. The pictures of the model
//! paint its triangles. Each triangle has a light that comes from the point of view, and a part that is dark when
//! it turns away. The models face the +z axis, so the default point of view is in front of them. The result is a list of pixels (RGBA) that a window can show.

use crate::model::{Model, Texture};
use std::collections::HashMap;

/// Where the model is seen from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The turn around the vertical axis, in radians.
    pub yaw: f32,
    /// The turn up and down, in radians.
    pub pitch: f32,
    /// 1 shows the whole model. More shows it bigger.
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { yaw: std::f32::consts::PI + 0.55, pitch: 0.15, zoom: 1.0 }
    }
}

/// The centre and the radius of the smallest sphere (about) that holds all the vertices of a model.
pub fn bounds(model: &Model) -> ([f32; 3], f32) {
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for mesh in &model.meshes {
        for vertex in &mesh.vertices {
            for axis in 0..3 {
                low[axis] = low[axis].min(vertex.position[axis]);
                high[axis] = high[axis].max(vertex.position[axis]);
            }
        }
    }
    if low[0] > high[0] {
        return ([0.0; 3], 1.0);
    }
    let centre = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, (low[2] + high[2]) / 2.0];
    let radius = ((high[0] - low[0]).powi(2) + (high[1] - low[1]).powi(2) + (high[2] - low[2]).powi(2)).sqrt() / 2.0;
    (centre, radius.max(1e-3))
}

/// A picture of a model with `width` × `height` pixels, on a transparent background.
///
/// `textures` are the pictures that the materials ask for by name. A material whose picture is missing is grey.
pub fn render(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 4];
    let mut depth = vec![f32::MAX; width * height];
    let (centre, radius) = bounds(model);
    let scale = width.min(height) as f32 * 0.46 / radius * camera.zoom;
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
    // The point of view turns around the model: the model turns the other way. First around the vertical axis,
    // then around the horizontal one. The y axis of the screen goes down.
    let turn = |p: [f32; 3]| -> [f32; 3] {
        let (x, y, z) = (p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]);
        let (x, z) = (x * cos_yaw + z * sin_yaw, -x * sin_yaw + z * cos_yaw);
        let (y, z) = (y * cos_pitch - z * sin_pitch, y * sin_pitch + z * cos_pitch);
        [x, y, z]
    };
    let turn_direction = |n: [f32; 3]| -> [f32; 3] {
        let (x, z) = (n[0] * cos_yaw + n[2] * sin_yaw, -n[0] * sin_yaw + n[2] * cos_yaw);
        let (y, z) = (n[1] * cos_pitch - z * sin_pitch, n[1] * sin_pitch + z * cos_pitch);
        [x, y, z]
    };
    let (half_x, half_y) = (width as f32 / 2.0, height as f32 / 2.0);
    for mesh in &model.meshes {
        let material = model.materials.get(mesh.material);
        let texture = material.and_then(|material| textures.get(&material.texture));
        let frame = match (material, texture) {
            (Some(material), Some(texture)) => frame_shift(mesh, material, texture),
            _ => 0.0,
        };
        let moved: Vec<([f32; 3], [f32; 3])> = mesh.vertices.iter().map(|v| (turn(v.position), turn_direction(v.normal))).collect();
        for triangle in &mesh.triangles {
            let corners = triangle.map(|k| k as usize);
            if corners.iter().any(|&k| k >= moved.len()) {
                continue;
            }
            let screen = corners.map(|k| [half_x + moved[k].0[0] * scale, half_y - moved[k].0[1] * scale, moved[k].0[2]]);
            let area = (screen[1][0] - screen[0][0]) * (screen[2][1] - screen[0][1]) - (screen[2][0] - screen[0][0]) * (screen[1][1] - screen[0][1]);
            if area.abs() < 1e-6 {
                continue;
            }
            let min_x = screen.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_x = (screen.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil().min(width as f32 - 1.0)).max(0.0) as usize;
            let min_y = screen.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_y = (screen.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil().min(height as f32 - 1.0)).max(0.0) as usize;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((screen[1][0] - px) * (screen[2][1] - py) - (screen[2][0] - px) * (screen[1][1] - py)) / area;
                    let w1 = ((screen[2][0] - px) * (screen[0][1] - py) - (screen[0][0] - px) * (screen[2][1] - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * screen[0][2] + w1 * screen[1][2] + w2 * screen[2][2];
                    let place = y * width + x;
                    if z >= depth[place] {
                        continue;
                    }
                    let uv = |axis: usize| w0 * mesh.vertices[corners[0]].uv[axis] + w1 * mesh.vertices[corners[1]].uv[axis] + w2 * mesh.vertices[corners[2]].uv[axis];
                    let color = match (texture, material) {
                        (Some(texture), Some(material)) => {
                            let (u, v) = texture_place(material, [uv(0), uv(1)], frame);
                            sample_wrapped(texture, u, v, material.texture_wrap)
                        }
                        (Some(texture), None) => sample_wrapped(texture, uv(0), uv(1), [2, 2]),
                        (None, _) => [170, 170, 170, 255],
                    };
                    // The pictures mark the places that the model does not fill (the shape of a tail, of an ear, of a wing)
                    // with black and no opacity. The fourth channel is not always an opacity, so only that mark cuts.
                    if color[3] < 128 && color[0] < 12 && color[1] < 12 && color[2] < 12 {
                        continue;
                    }
                    // The light comes from the point of view (the z axis of the screen points away from it).
                    let normal: [f32; 3] = std::array::from_fn(|axis| w0 * moved[corners[0]].1[axis] + w1 * moved[corners[1]].1[axis] + w2 * moved[corners[2]].1[axis]);
                    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt().max(1e-6);
                    let light = 0.45 + 0.55 * (-normal[2] / length).abs();
                    depth[place] = z;
                    pixels[place * 4..place * 4 + 4].copy_from_slice(&[(color[0] as f32 * light) as u8, (color[1] as f32 * light) as u8, (color[2] as f32 * light) as u8, 255]);
                }
            }
        }
    }
    pixels
}

/// The number of frames that a picture with several frames (the eyes, the mouth) is cut into, one under the other.
const FRAMES: usize = 4;

/// True for the colour that the pictures of the game use for a place that no frame fills.
fn is_marker(pixel: &[u8]) -> bool {
    pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0
}

/// A place of the picture (0 to 1) from a place that can be outside, for a kind of wrap (see `Material::texture_wrap`).
fn wrap(t: f32, kind: u8) -> f32 {
    match kind {
        0 | 1 => t.clamp(0.0, 1.0),
        3 => {
            let m = t.rem_euclid(2.0);
            if m > 1.0 { 2.0 - m } else { m }
        }
        _ => t.rem_euclid(1.0),
    }
}

/// The place in the picture of a mesh, from the texture position of a vertex: the transform of the material,
/// then, for a picture with frames, a move down by `frame` (the part of the picture that one frame takes).
fn texture_place(material: &crate::model::Material, uv: [f32; 2], frame: f32) -> (f32, f32) {
    (material.texture_scale[0] * (uv[0] - material.texture_translation[0]), material.texture_scale[1] * (uv[1] - material.texture_translation[1]) - frame)
}

/// How far down the picture of a mesh must be moved to show the frame that a still view should show.
///
/// A picture of eyes or of a mouth holds several frames (looks), one under the other, and the file of the model
/// points at one of them that the animation then changes. For a still view the frame with the most dark pixels is
/// taken (the eyes are open, not a closed line), unless a frame has places that no look fills: the picture marks them
/// in pure red. Only the meshes that use a small part of their picture are looked at. Returns the move in the
/// vertical place of the picture (a multiple of one frame), 0 when nothing needs to change.
fn frame_shift(mesh: &crate::model::Mesh, material: &crate::model::Material, texture: &Texture) -> f32 {
    if mesh.vertices.is_empty() {
        return 0.0;
    }
    let spans = |axis: usize| {
        let values = mesh.vertices.iter().map(|v| texture_place(material, v.uv, 0.0)).map(|p| if axis == 0 { p.0 } else { p.1 });
        (values.clone().fold(f32::MAX, f32::min), values.fold(f32::MIN, f32::max))
    };
    let (low_v, high_v) = spans(1);
    let (low_u, high_u) = spans(0);
    // A mesh that covers more than a frame is not one that looks at a frame.
    if high_v - low_v > 1.0 / FRAMES as f32 * 1.2 {
        return 0.0;
    }
    let score = |k: usize| -> Option<usize> {
        let shift = k as f32 / FRAMES as f32;
        let mut dark = 0;
        for row in 0..12 {
            for column in 0..12 {
                let u = low_u + (high_u - low_u) * (column as f32 + 0.5) / 12.0;
                let v = low_v + (high_v - low_v) * (row as f32 + 0.5) / 12.0 - shift;
                let pixel = sample_wrapped(texture, u, v, material.texture_wrap);
                if is_marker(&pixel) {
                    return None;
                }
                if pixel[3] > 128 && (pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) < 240 {
                    dark += 1;
                }
            }
        }
        Some(dark)
    };
    let mut best = (0usize, score(0));
    for k in 1..FRAMES {
        let candidate = score(k);
        if candidate.is_some() && (best.1.is_none() || candidate > best.1) {
            best = (k, candidate);
        }
    }
    best.0 as f32 / FRAMES as f32
}

/// The colour of a picture at a place (u, v) with a kind of wrap for each axis. v goes from the top of the picture.
fn sample_wrapped(texture: &Texture, u: f32, v: f32, kinds: [u8; 2]) -> [u8; 4] {
    let column = (wrap(u, kinds[0]) * texture.width as f32).min(texture.width as f32 - 1.0) as usize;
    // The texture positions of the models count from the top of the picture, like its rows.
    let row = (wrap(v, kinds[1]) * texture.height as f32).clamp(0.0, texture.height as f32 - 1.0) as usize;
    let at = (row * texture.width + column) * 4;
    texture.rgba.get(at..at + 4).map_or([200, 0, 200, 255], |p| [p[0], p[1], p[2], p[3]])
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::{Material, Mesh, Vertex};

    /// A model that is one triangle with the material 0, whose picture is a red pixel row.
    fn triangle() -> (Model, HashMap<String, Texture>) {
        let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv: [u, v], ..Vertex::default() };
        let mesh = Mesh { material: 0, vertices: vec![vertex(-1.0, -1.0, 0.0, 0.0), vertex(1.0, -1.0, 1.0, 0.0), vertex(0.0, 1.0, 0.5, 1.0)], triangles: vec![[0, 1, 2]] };
        let model = Model { bones: Vec::new(), materials: vec![Material { name: "m".into(), texture: "t".into(), texture_scale: [1.0, 1.0], texture_translation: [0.0, 0.0], texture_wrap: [2, 2] }], meshes: vec![mesh] };
        let texture = Texture { name: "t".into(), width: 8, height: 8, rgba: [255u8, 0, 0, 255].repeat(64) };
        (model, HashMap::from([("t".to_string(), texture)]))
    }

    #[test]
    fn a_triangle_is_drawn_with_its_picture_and_the_rest_is_transparent() {
        let (model, textures) = triangle();
        let image = render(&model, &textures, &Camera { yaw: 0.0, pitch: 0.0, zoom: 1.0 }, 64, 64);
        assert_eq!(image.len(), 64 * 64 * 4);
        let centre = &image[(32 * 64 + 32) * 4..][..4];
        assert!(centre[0] > 200 && centre[1] == 0 && centre[3] == 255, "{centre:?}");
        assert_eq!(image[3], 0, "the corner is transparent");
        // Without its picture the triangle is grey.
        let grey = render(&model, &HashMap::new(), &Camera::default(), 64, 64);
        assert!(grey.chunks(4).any(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]));
    }

    #[test]
    fn a_triangle_near_the_camera_hides_one_behind_it() {
        let (mut model, textures) = triangle();
        // A second triangle in front, with no picture (grey): it must hide the red one at the centre.
        let mut front = model.meshes[0].clone();
        front.material = 5;
        for vertex in &mut front.vertices {
            vertex.position[2] = -0.5;
        }
        model.meshes.push(front);
        let image = render(&model, &textures, &Camera { yaw: 0.0, pitch: 0.0, zoom: 1.0 }, 64, 64);
        let centre = &image[(32 * 64 + 32) * 4..][..4];
        assert_eq!(centre[0], centre[1], "the grey triangle is in front: {centre:?}");
    }

    /// With the dump of the game: the model of a species is read and drawn from four sides. When the variable
    /// SANGO_RENDER_OUT is set, the pictures are written there as PNG files, to look at them.
    #[test]
    fn a_model_of_the_dump_is_read_and_drawn() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let garc = crate::Garc::open(&dump.archive(8)).unwrap();
        let parts = |file: usize| -> Vec<Vec<u8>> {
            let bytes = crate::lz::decompress(garc.first_file(file).unwrap()).unwrap();
            let bundle = crate::Bundle::parse(bytes).unwrap();
            (0..bundle.resource_count()).map(|i| bundle.resource(i).unwrap().to_vec()).collect()
        };
        for (name, first) in [("bulbasaur", 3usize), ("charizard", 3 + 8 * 5)] {
            let model = crate::model::parse_model(&parts(first)[0]).unwrap();
            let textures: HashMap<String, Texture> = crate::model::parse_textures(&parts(first + 2)[0]).unwrap().into_iter().map(|t| (t.name.clone(), t)).collect();
            eprintln!("{name}: {} bones, {} materials, {} meshes, {} textures {:?}", model.bones.len(), model.materials.len(), model.meshes.len(), textures.len(), textures.keys().collect::<Vec<_>>());
            eprintln!("materials {:?}", model.materials);
            assert!(model.meshes.iter().map(|m| m.triangles.len()).sum::<usize>() > 100);
            if let Ok(out) = std::env::var("SANGO_RENDER_OUT") {
                for (side, yaw) in [("front", 0.0f32), ("side", 1.57), ("back", 3.14), ("three_quarters", 0.6)] {
                    let image = render(&model, &textures, &Camera { yaw, pitch: 0.15, zoom: 1.0 }, 400, 400);
                    image::save_buffer(format!("{out}/{name}_{side}.png"), &image, 400, 400, image::ColorType::Rgba8).unwrap();
                }
            }
        }
    }

    #[test]
    fn the_bounds_hold_the_vertices() {
        let (model, _) = triangle();
        let (centre, radius) = bounds(&model);
        assert_eq!(centre, [0.0, 0.0, 0.0]);
        assert!((radius - (8.0f32).sqrt() / 2.0).abs() < 1e-4);
        assert_eq!(bounds(&Model::default()), ([0.0; 3], 1.0));
    }
}
