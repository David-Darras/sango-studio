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

/// The centre and the radius of a sphere that holds all the vertices of several models (the same model in several poses).
pub fn bounds_of(models: &[Model]) -> ([f32; 3], f32) {
    let mut all = Model::default();
    for model in models {
        all.meshes.extend(model.meshes.iter().map(|mesh| crate::model::Mesh { material: 0, vertices: mesh.vertices.clone(), triangles: Vec::new() }));
    }
    bounds(&all)
}

/// A picture of a model with `width` × `height` pixels, on a transparent background.
///
/// `textures` are the pictures that the materials ask for by name. A material whose picture is missing is grey.
/// The model is drawn two times bigger, then each pixel takes the mean of four: the edges are smooth.
pub fn render(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, width: usize, height: usize) -> Vec<u8> {
    render_framed(model, textures, camera, bounds(model), width, height)
}

/// The same, with the sphere that the picture shows given (`bounds`): a model that moves keeps the same place and size.
pub fn render_framed(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, frame: ([f32; 3], f32), width: usize, height: usize) -> Vec<u8> {
    let big = draw(model, textures, camera, frame, width * 2, height * 2);
    let mut pixels = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            // The mean of the four pixels. The colours count with their opacity: a clear pixel
            // does not darken the edge of the model.
            let mut color = [0u32; 3];
            let mut alpha = 0u32;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let at = ((y * 2 + dy) * width * 2 + x * 2 + dx) * 4;
                let a = big[at + 3] as u32;
                alpha += a;
                for channel in 0..3 {
                    color[channel] += big[at + channel] as u32 * a;
                }
            }
            let at = (y * width + x) * 4;
            if alpha > 0 {
                for channel in 0..3 {
                    pixels[at + channel] = (color[channel] / alpha) as u8;
                }
            }
            pixels[at + 3] = (alpha / 4) as u8;
        }
    }
    pixels
}

/// The drawing itself, one ray of light, no smoothing.
fn draw(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, frame: ([f32; 3], f32), width: usize, height: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 4];
    let mut depth = vec![f32::MAX; width * height];
    let (centre, radius) = frame;
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
    // The meshes that mix their pictures with several steps (the iris of the eyes, the flames, the gas) are drawn after
    // the others, and are blended with what is behind them.
    for pass in 0..2 {
    for mesh in &model.meshes {
        let material = model.materials.get(mesh.material);
        let mixed = material.is_some_and(is_mixed);
        if mixed != (pass == 1) {
            continue;
        }
        let texture = material.and_then(|material| textures.get(&material.texture));
        // The iris: the eye that lies under it. The iris is not seen on the skin around the eye.
        let eye_under = match material {
            Some(material) if material.transparent => eye_under(model, textures, mesh),
            _ => None,
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
                    // The light comes from the point of view (the z axis of the screen points away from it).
                    let normal: [f32; 3] = std::array::from_fn(|axis| w0 * moved[corners[0]].1[axis] + w1 * moved[corners[1]].1[axis] + w2 * moved[corners[2]].1[axis]);
                    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt().max(1e-6);
                    let light = 0.45 + 0.55 * (-normal[2] / length).abs();
                    if mixed {
                        let Some(material) = material else { continue };
                        let mut color = combine(material, textures, [uv(0), uv(1)], light);
                        if let Some((eye_material, eye_texture, opening)) = &eye_under {
                            // The mesh of an iris is bigger than the opening of the eye: the picture of the iris is
                            // kept only where the picture of the eye is not the colour of the skin.
                            let (eye_u, eye_v) = texture_place(eye_material, [uv(0), uv(1)]);
                            let column = (wrap(eye_u, eye_material.texture_wrap[0]) * eye_texture.width as f32).min(eye_texture.width as f32 - 1.0) as usize;
                            let row = (wrap(eye_v, eye_material.texture_wrap[1]) * eye_texture.height as f32).min(eye_texture.height as f32 - 1.0) as usize;
                            if !opening[row * eye_texture.width + column] {
                                color[3] = 0.0;
                            }
                        }
                        if color[3] < 0.02 {
                            continue;
                        }
                        // Blend with what is behind: the colour of the pixel and its opacity.
                        let at = place * 4;
                        let behind_alpha = pixels[at + 3] as f32 / 255.0;
                        let behind = &mut pixels[at..at + 4];
                        let alpha = color[3] + behind_alpha * (1.0 - color[3]);
                        for channel in 0..3 {
                            let mixed_value = (color[channel] * color[3] + behind[channel] as f32 / 255.0 * behind_alpha * (1.0 - color[3])) / alpha.max(1e-6);
                            behind[channel] = (mixed_value.clamp(0.0, 1.0) * 255.0) as u8;
                        }
                        behind[3] = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
                        continue;
                    }
                    let color = match (texture, material) {
                        (Some(texture), Some(material)) => {
                            let (u, v) = texture_place(material, [uv(0), uv(1)]);
                            let mut color = sample_wrapped(texture, u, v, material.texture_wrap);
                            if let Some((eye_material, eye_texture, opening)) = &eye_under {
                                // The mesh of an iris is bigger than the opening of the eye: the picture of the iris is
                                // kept only where the picture of the eye is not the colour of the skin.
                                let (eye_u, eye_v) = texture_place(eye_material, [uv(0), uv(1)]);
                                let column = (wrap(eye_u, eye_material.texture_wrap[0]) * eye_texture.width as f32).min(eye_texture.width as f32 - 1.0) as usize;
                                let row = (wrap(eye_v, eye_material.texture_wrap[1]) * eye_texture.height as f32).min(eye_texture.height as f32 - 1.0) as usize;
                                if !opening[row * eye_texture.width + column] {
                                    color[3] = 0;
                                }
                            }
                            color
                        }
                        (Some(texture), None) => sample_wrapped(texture, uv(0), uv(1), [2, 2]),
                        (None, _) => [170, 170, 170, 255],
                    };
                    // A transparent material (the iris of the eyes) is cut where its picture has no
                    // opacity: the mesh behind it (the white of the eye) shows there. For the other
                    // materials the fourth channel is not always an opacity: the pictures mark the
                    // places that the model does not fill (the shape of a tail, of an ear, of a wing)
                    // with black and no opacity, and only that mark cuts.
                    let cut = match material {
                        Some(material) if material.transparent => color[3] < 128,
                        _ => color[3] < 128 && color[0] < 12 && color[1] < 12 && color[2] < 12,
                    };
                    if cut {
                        continue;
                    }
                    depth[place] = z;
                    pixels[place * 4..place * 4 + 4].copy_from_slice(&[(color[0] as f32 * light) as u8, (color[1] as f32 * light) as u8, (color[2] as f32 * light) as u8, 255]);
                }
            }
        }
    }
    }
    pixels
}

/// True for a material that the game draws with several steps and the opacity of its pictures: the iris of the eyes,
/// the flames, the gas. The ones that the light does not touch (the file gives them no reflection) and the ones that
/// the file marks as transparent.
fn is_mixed(material: &crate::model::Material) -> bool {
    material.transparent || (material.unlit && !material.stages.is_empty())
}

/// The colour and the opacity of a pixel (from 0 to 1) that a material gives at a texture position, with the steps of
/// the mix that the file gives. `light` is the light of the surface, from 0 to 1.
fn combine(material: &crate::model::Material, textures: &HashMap<String, Texture>, uv: [f32; 2], light: f32) -> [f32; 4] {
    use crate::model::Stage;
    // The three pictures, at this place. A unit with no picture gives white.
    let mut pictures = [[1.0f32; 4]; 3];
    for (unit, picture) in material.units.iter().zip(pictures.iter_mut()) {
        if let Some(texture) = textures.get(&unit.texture) {
            let (u, v) = (unit.scale[0] * (uv[0] - unit.translation[0]), unit.scale[1] * (uv[1] - unit.translation[1]));
            *picture = sample_wrapped(texture, u, v, unit.wrap).map(|c| c as f32 / 255.0);
        }
    }
    let to_float = |bytes: [u8; 4]| bytes.map(|c| c as f32 / 255.0);
    let mut previous = [1.0f32; 4];
    let mut buffer = to_float(material.buffer_color);
    for stage in material.stages.iter() {
        let constant = to_float(stage.constant);
        let source = |code: u8, previous: [f32; 4], buffer: [f32; 4]| -> [f32; 4] {
            match code {
                0 => [1.0; 4],
                1 => [light, light, light, 1.0],
                2 => [0.0; 4],
                3..=5 => pictures[(code - 3) as usize],
                13 => buffer,
                14 => constant,
                _ => previous,
            }
        };
        let color_arg = |k: usize| -> [f32; 3] {
            let value = source(stage.color_source[k], previous, buffer);
            let operand = stage.color_operand[k];
            let picked = match operand & !1 {
                2 => [value[3]; 3],
                4 => [value[0]; 3],
                8 => [value[1]; 3],
                12 => [value[2]; 3],
                _ => [value[0], value[1], value[2]],
            };
            if operand & 1 != 0 { picked.map(|c| 1.0 - c) } else { picked }
        };
        let alpha_arg = |k: usize| -> f32 {
            let value = source(stage.alpha_source[k], previous, buffer);
            let operand = stage.alpha_operand[k];
            let picked = match operand & !1 {
                2 => value[0],
                4 => value[1],
                6 => value[2],
                _ => value[3],
            };
            if operand & 1 != 0 { 1.0 - picked } else { picked }
        };
        let (a, b, c) = (color_arg(0), color_arg(1), color_arg(2));
        let mut rgb: [f32; 3] = std::array::from_fn(|i| match stage.color_mode {
            0 => a[i],
            1 => a[i] * b[i],
            2 => (a[i] + b[i]).min(1.0),
            3 => (a[i] + b[i] - 0.5).clamp(0.0, 1.0),
            4 => b[i] + (a[i] - b[i]) * c[i],
            5 => (a[i] - b[i]).max(0.0),
            8 => (a[i] * b[i] + c[i]).min(1.0),
            9 => (a[i] + b[i]).min(1.0) * c[i],
            _ => a[i],
        });
        if stage.color_mode == 6 || stage.color_mode == 7 {
            let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).min(1.0);
            rgb = [dot; 3];
        }
        let (x, y, z) = (alpha_arg(0), alpha_arg(1), alpha_arg(2));
        let mut alpha = match stage.alpha_mode {
            1 => x * y,
            2 => (x + y).min(1.0),
            3 => (x + y - 0.5).clamp(0.0, 1.0),
            4 => y + (x - y) * z,
            5 => (x - y).max(0.0),
            8 => (x * y + z).min(1.0),
            9 => (x + y).min(1.0) * z,
            _ => x,
        };
        let (rgb_scale, alpha_scale) = ((1 << stage.color_scale) as f32, (1 << stage.alpha_scale) as f32);
        rgb = rgb.map(|v| (v * rgb_scale).min(1.0));
        alpha = (alpha * alpha_scale).min(1.0);
        if stage.save_color {
            buffer[..3].copy_from_slice(&previous[..3]);
        }
        if stage.save_alpha {
            buffer[3] = previous[3];
        }
        previous = [rgb[0], rgb[1], rgb[2], alpha];
        let _: &Stage = stage;
    }
    previous
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

/// The place in the picture of a mesh, from the texture position of a vertex: the transform of the material.
fn texture_place(material: &crate::model::Material, uv: [f32; 2]) -> (f32, f32) {
    (material.texture_scale[0] * (uv[0] - material.texture_translation[0]), material.texture_scale[1] * (uv[1] - material.texture_translation[1]))
}

// The pictures of the eyes and of the mouth hold several looks (a sheet), one under the other. The texture positions
// of an eye mesh already point at the look that the model shows when nothing animates it (the open eye): the transform
// of the material (scale, translation) and the wrap of the picture (mirror for the right eye) are all that is needed.
// Turning the band of the mesh over, or moving it to another look, gives closed eyes.

/// The box of the texture positions of a mesh: low and high of u, low and high of v.
fn uv_box(mesh: &crate::model::Mesh) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
    for vertex in &mesh.vertices {
        b = [b[0].min(vertex.uv[0]), b[1].max(vertex.uv[0]), b[2].min(vertex.uv[1]), b[3].max(vertex.uv[1])];
    }
    b
}

/// The material, the picture and the opening of the eye that an iris mesh lies on. The eye is the mesh that is not
/// transparent and that covers all the texture positions of the iris. `None` when there is no such mesh.
fn eye_under<'a>(model: &'a Model, textures: &'a HashMap<String, Texture>, iris: &crate::model::Mesh) -> Option<(&'a crate::model::Material, &'a Texture, Vec<bool>)> {
    if iris.vertices.is_empty() {
        return None;
    }
    let area = uv_box(iris);
    let margin = 0.01;
    // Of the meshes that cover the iris, the eye is the one with the smallest box of texture positions.
    model
        .meshes
        .iter()
        .filter_map(|mesh| {
            let material = model.materials.get(mesh.material).filter(|material| !material.transparent)?;
            let texture = textures.get(&material.texture)?;
            let b = uv_box(mesh);
            (!mesh.vertices.is_empty() && b[0] <= area[0] + margin && b[1] >= area[1] - margin && b[2] <= area[2] + margin && b[3] >= area[3] - margin).then_some(((b[1] - b[0]) * (b[3] - b[2]), material, texture))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, material, texture)| (material, texture, opening(texture)))
}

/// The places of the picture of an eye that are not skin. The picture of an eye is the skin around the eyes with the eyes
/// drawn on it, so the colour of the skin is the colour that most of the pixels have.
fn opening(texture: &Texture) -> Vec<bool> {
    let pixels = || texture.rgba.chunks_exact(4).filter(|p| p[3] >= 128);
    let bucket = |p: &[u8]| (p[0] as usize >> 4) << 8 | (p[1] as usize >> 4) << 4 | (p[2] as usize >> 4);
    let mut counts = vec![0u32; 4096];
    for p in pixels() {
        counts[bucket(p)] += 1;
    }
    let Some(best) = counts.iter().enumerate().max_by_key(|(_, count)| **count).map(|(bucket, _)| bucket) else { return vec![true; texture.width * texture.height] };
    let (mut sum, mut n) = ([0u32; 3], 0u32);
    for p in pixels().filter(|p| bucket(p) == best) {
        for k in 0..3 {
            sum[k] += p[k] as u32;
        }
        n += 1;
    }
    let skin = sum.map(|v| v / n.max(1));
    // Around a dark eye (a black skin) the iris cannot be told from the skin: nothing is cut.
    if (skin[0] * 3 + skin[1] * 6 + skin[2]) / 10 < 80 {
        return vec![true; texture.width * texture.height];
    }
    texture.rgba.chunks_exact(4).map(|p| (0..3).map(|k| (p[k] as i32 - skin[k] as i32).abs()).sum::<i32>() > 40).collect()
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
        let model = Model { bones: Vec::new(), materials: vec![Material { name: "m".into(), texture: "t".into(), texture_scale: [1.0, 1.0], texture_translation: [0.0, 0.0], texture_wrap: [2, 2], ..Material::default() }], meshes: vec![mesh] };
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
