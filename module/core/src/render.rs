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
    /// True to draw a dark line around the model and where a part of the model passes in front of another one.
    pub outline: bool,
    /// True to look from inside the model, with perspective (a place of a battle): the point of view stands in the middle of the
    /// model, a little above its lowest point, and turns with `yaw` and `pitch`. `zoom` narrows the field of view.
    pub inside: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { yaw: std::f32::consts::PI - 0.55, pitch: 0.15, zoom: 1.0, outline: true, inside: false }
    }
}

/// The centre and the radius of the smallest sphere (about) that holds all the vertices of a model.
pub fn bounds(model: &Model) -> ([f32; 3], f32) {
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for mesh in model.meshes.iter().filter(|mesh| !mesh.hidden) {
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

/// The tilt of the point of view that a frame leaves room for, in radians.
const FRAME_PITCH: f32 = 0.35;

/// The frame of the picture of some models (the same model in several poses): the centre of their box, and the size that the
/// picture shows around it. The size is the tightest one from every side: the models fill the picture, and they stay in it
/// when the point of view turns around them, tilted by up to `FRAME_PITCH`.
pub fn frame_of(models: &[Model]) -> ([f32; 3], f32) {
    let vertices = || models.iter().flat_map(|model| &model.meshes).filter(|mesh| !mesh.hidden).flat_map(|mesh| &mesh.vertices);
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for vertex in vertices() {
        for axis in 0..3 {
            low[axis] = low[axis].min(vertex.position[axis]);
            high[axis] = high[axis].max(vertex.position[axis]);
        }
    }
    if low[0] > high[0] {
        return ([0.0; 3], 1.0);
    }
    let centre: [f32; 3] = std::array::from_fn(|axis| (low[axis] + high[axis]) / 2.0);
    let (sin, cos) = FRAME_PITCH.sin_cos();
    // A vertex goes around the vertical axis at the distance `around`: on the screen it is at most that far to a side, and that
    // far up or down once the tilt is added to its height.
    let size = vertices().fold(0.0f32, |size, vertex| {
        let around = (vertex.position[0] - centre[0]).hypot(vertex.position[2] - centre[2]);
        let height = (vertex.position[1] - centre[1]).abs();
        size.max(around).max(height * cos + around * sin)
    });
    (centre, size.max(1e-3))
}

/// A picture of a model with `width` × `height` pixels, on a transparent background.
///
/// `textures` are the pictures that the materials ask for by name. A material whose picture is missing is grey.
/// The model is drawn two times bigger, then each pixel takes the mean of four: the edges are smooth.
pub fn render(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, width: usize, height: usize) -> Vec<u8> {
    // From inside a place, the point of view comes from the sphere around the place.
    let frame = if camera.inside { bounds(model) } else { frame_of(std::slice::from_ref(model)) };
    render_framed(model, textures, camera, frame, width, height)
}

/// The same, with the frame that the picture shows given (`frame_of`): a model that moves keeps the same place and size.
pub fn render_framed(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, frame: ([f32; 3], f32), width: usize, height: usize) -> Vec<u8> {
    render_framed_at(model, textures, camera, frame, width, height, 2)
}

/// The same, drawn `factor` times bigger before the pixels take the mean: 2 smooths the edges, 1 is four times faster.
pub fn render_framed_at(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, frame: ([f32; 3], f32), width: usize, height: usize, factor: usize) -> Vec<u8> {
    if factor <= 1 {
        return draw(model, textures, camera, frame, width, height, OUTLINE_PIXELS as usize);
    }
    let big = draw(model, textures, camera, frame, width * 2, height * 2, OUTLINE_PIXELS as usize * 2);
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
/// The width of the outline in the final picture, in pixels. The picture that is drawn bigger has a wider outline, so the
/// width does not change with the smoothing.
const OUTLINE_PIXELS: f32 = 3.0;

fn draw(model: &Model, textures: &HashMap<String, Texture>, camera: &Camera, frame: ([f32; 3], f32), width: usize, height: usize, outline_width: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; width * height * 4];
    let mut depth = vec![f32::MAX; width * height];
    let (centre, radius) = frame;
    let scale = width.min(height) as f32 * 0.46 / radius * camera.zoom;
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
    // The point of view turns around the model: the model turns the other way. First around the vertical axis,
    // then around the horizontal one. The z axis of the turned model points away from the point of view, so the x axis of the
    // screen goes against the turned x axis, and the y axis of the screen goes down.
    let turn = |p: [f32; 3]| -> [f32; 3] {
        let (x, y, z) = (p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]);
        let (x, z) = (x * cos_yaw + z * sin_yaw, -x * sin_yaw + z * cos_yaw);
        let (y, z) = (y * cos_pitch - z * sin_pitch, y * sin_pitch + z * cos_pitch);
        [x, y, z]
    };
    // Inside: the point of view is in the middle of the model, at 12% of the radius above its lowest point. The positions are
    // relative to it, and the picture has perspective: a field of view of 70 degrees, narrower with the zoom.
    let lowest = model.meshes.iter().filter(|mesh| !mesh.hidden).flat_map(|mesh| mesh.vertices.iter()).map(|v| v.position[1]).fold(f32::MAX, f32::min);
    let eye = [centre[0], lowest.min(centre[1]) + radius * 0.12, centre[2]];
    let focal = (width.min(height) as f32 / 2.0) / (35f32.to_radians().tan()) * camera.zoom;
    let near = radius * 0.002;
    let turn_inside = |p: [f32; 3]| -> [f32; 3] {
        let (x, y, z) = (p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]);
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
        let combined = mixed || material.is_some_and(|m| m.unlit && !m.stages.is_empty());
        if mixed != (pass == 1) || mesh.hidden {
            continue;
        }
        let texture = material.and_then(|material| textures.get(&material.texture));
        let state = material.map_or(crate::model::RenderState::default(), |m| m.state);
        // The three pictures of the material, found once for the whole mesh.
        let pictures: [Option<&Texture>; 3] = match material {
            Some(material) => std::array::from_fn(|k| textures.get(&material.units[k].texture)),
            None => [None; 3],
        };
        // The iris: the eye that lies under it. The iris is not seen on the skin around the eye.
        let eye_under = match material {
            Some(material) if material.transparent => eye_under(model, textures, mesh),
            _ => None,
        };
        let moved: Vec<([f32; 3], [f32; 3])> = mesh.vertices.iter().map(|v| (if camera.inside { turn_inside(v.position) } else { turn(v.position) }, turn_direction(v.normal))).collect();
        for triangle in &mesh.triangles {
            let corners = triangle.map(|k| k as usize);
            if corners.iter().any(|&k| k >= moved.len()) {
                continue;
            }
            // The pieces of the triangle that are drawn: the corners on the screen, and the place of each corner in the triangle
            // (its weights for the three corners of the mesh). From the outside, the whole triangle. From the inside, the part in
            // front of the point of view, cut in triangles.
            let pieces: Vec<([[f32; 3]; 3], [[f32; 3]; 3])> = if camera.inside {
                let points = corners.map(|k| moved[k].0);
                clip_near(points, near)
                    .into_iter()
                    .map(|(piece, weights)| (piece.map(|q| [half_x - q[0] * focal / q[2], half_y - q[1] * focal / q[2], q[2]]), weights))
                    .collect()
            } else {
                vec![(corners.map(|k| [half_x - moved[k].0[0] * scale, half_y - moved[k].0[1] * scale, moved[k].0[2]]), [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])]
            };
            for (screen, weights) in pieces {
            let area = (screen[1][0] - screen[0][0]) * (screen[2][1] - screen[0][1]) - (screen[2][0] - screen[0][0]) * (screen[1][1] - screen[0][1]);
            if area.abs() < 1e-6 {
                continue;
            }
            // The faces that the material does not draw. A face that turns its front to the point of view has a negative area
            // (the y axis of the screen goes down).
            match state.cull {
                0 if area < 0.0 => continue,
                1 if area > 0.0 => continue,
                _ => {}
            }
            let min_x = screen.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_x = (screen.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil().min(width as f32 - 1.0)).max(0.0) as usize;
            let min_y = screen.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let max_y = (screen.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil().min(height as f32 - 1.0)).max(0.0) as usize;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let s0 = ((screen[1][0] - px) * (screen[2][1] - py) - (screen[2][0] - px) * (screen[1][1] - py)) / area;
                    let s1 = ((screen[2][0] - px) * (screen[0][1] - py) - (screen[0][0] - px) * (screen[2][1] - py)) / area;
                    let s2 = 1.0 - s0 - s1;
                    if s0 < 0.0 || s1 < 0.0 || s2 < 0.0 {
                        continue;
                    }
                    // With perspective, the weights on the screen are not the weights in the triangle: they are divided by the
                    // depth of each corner.
                    let (piece_weights, z) = if camera.inside {
                        let q = [s0 / screen[0][2], s1 / screen[1][2], s2 / screen[2][2]];
                        let sum = q[0] + q[1] + q[2];
                        ([q[0] / sum, q[1] / sum, q[2] / sum], 1.0 / sum)
                    } else {
                        ([s0, s1, s2], s0 * screen[0][2] + s1 * screen[1][2] + s2 * screen[2][2])
                    };
                    let w0 = piece_weights[0] * weights[0][0] + piece_weights[1] * weights[1][0] + piece_weights[2] * weights[2][0];
                    let w1 = piece_weights[0] * weights[0][1] + piece_weights[1] * weights[1][1] + piece_weights[2] * weights[2][1];
                    let w2 = piece_weights[0] * weights[0][2] + piece_weights[1] * weights[1][2] + piece_weights[2] * weights[2][2];
                    let place = y * width + x;
                    if !depth_passes(state.depth_test, z, depth[place]) {
                        continue;
                    }
                    let uv = |axis: usize| w0 * mesh.vertices[corners[0]].uv[axis] + w1 * mesh.vertices[corners[1]].uv[axis] + w2 * mesh.vertices[corners[2]].uv[axis];
                    // The light comes from the point of view (the z axis of the screen points away from it).
                    let normal: [f32; 3] = std::array::from_fn(|axis| w0 * moved[corners[0]].1[axis] + w1 * moved[corners[1]].1[axis] + w2 * moved[corners[2]].1[axis]);
                    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt().max(1e-6);
                    let light = 0.45 + 0.55 * (-normal[2] / length).abs();
                    if combined {
                        let Some(material) = material else { continue };
                        let primary: [f32; 4] = std::array::from_fn(|k| (w0 * mesh.vertices[corners[0]].color[k] as f32 + w1 * mesh.vertices[corners[1]].color[k] as f32 + w2 * mesh.vertices[corners[2]].color[k] as f32) / 255.0);
                        let mut color = combine(material, &pictures, [uv(0), uv(1)], light, primary);
                        if eye_under.as_ref().is_some_and(|eye| outside_eye(eye, [uv(0), uv(1)])) {
                            color[3] = 0.0;
                        }
                        // The test of the opacity of the material, else a pixel that is nearly clear is not drawn.
                        match state.alpha_test {
                            Some((function, reference)) => {
                                if !depth_passes(function, (color[3] * 255.0).round(), reference as f32) {
                                    continue;
                                }
                            }
                            None if color[3] < 0.02 => continue,
                            None => {}
                        }
                        // Mix with what is behind: the colour of the pixel and its opacity.
                        let at = place * 4;
                        let behind_alpha = pixels[at + 3] as f32 / 255.0;
                        let behind_color = [pixels[at] as f32 / 255.0, pixels[at + 1] as f32 / 255.0, pixels[at + 2] as f32 / 255.0];
                        let (mixed_color, mixed_alpha) = blend(&state, [color[0], color[1], color[2]], color[3], behind_color, behind_alpha);
                        for channel in 0..3 {
                            pixels[at + channel] = (mixed_color[channel].clamp(0.0, 1.0) * 255.0) as u8;
                        }
                        pixels[at + 3] = (mixed_alpha.clamp(0.0, 1.0) * 255.0) as u8;
                        if state.depth_write {
                            depth[place] = z;
                        }
                        continue;
                    }
                    let color = match (texture, material) {
                        (Some(texture), Some(material)) => {
                            let (u, v) = texture_place(material, [uv(0), uv(1)]);
                            let mut color = sample_wrapped(texture, u, v, material.texture_wrap);
                            if eye_under.as_ref().is_some_and(|eye| outside_eye(eye, [uv(0), uv(1)])) {
                                color[3] = 0;
                            }
                            color
                        }
                        (Some(texture), None) => sample_wrapped(texture, uv(0), uv(1), [2, 2]),
                        (None, _) => [170, 170, 170, 255],
                    };
                    if is_cut(material, color) {
                        continue;
                    }
                    if state.depth_write {
                        depth[place] = z;
                    }
                    pixels[place * 4..place * 4 + 4].copy_from_slice(&[(color[0] as f32 * light) as u8, (color[1] as f32 * light) as u8, (color[2] as f32 * light) as u8, 255]);
                }
            }
            }
        }
    }
    // The line around the model: after the opaque meshes, before the flames and the gas.
    if pass == 0 && camera.outline {
        outline(&mut pixels, &depth, width, height, radius, outline_width);
    }
    }
    pixels
}

/// The part of a triangle (its corners relative to the point of view) that is in front of the plane at the distance `near`, cut in
/// triangles. Each corner of a piece comes with its weights for the three corners of the triangle.
fn clip_near(points: [[f32; 3]; 3], near: f32) -> Vec<([[f32; 3]; 3], [[f32; 3]; 3])> {
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if points.iter().all(|p| p[2] >= near) {
        return vec![(points, identity)];
    }
    if points.iter().all(|p| p[2] < near) {
        return Vec::new();
    }
    // The polygon in front of the plane: the corners in front, and the points where the edges cross the plane.
    let mut polygon: Vec<([f32; 3], [f32; 3])> = Vec::new();
    for k in 0..3 {
        let (a, b) = (points[k], points[(k + 1) % 3]);
        let (wa, wb) = (identity[k], identity[(k + 1) % 3]);
        if a[2] >= near {
            polygon.push((a, wa));
        }
        if (a[2] >= near) != (b[2] >= near) {
            let t = (near - a[2]) / (b[2] - a[2]);
            let mix = |u: [f32; 3], v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|axis| u[axis] + (v[axis] - u[axis]) * t) };
            polygon.push((mix(a, b), mix(wa, wb)));
        }
    }
    (1..polygon.len().saturating_sub(1)).map(|k| ([polygon[0].0, polygon[k].0, polygon[k + 1].0], [polygon[0].1, polygon[k].1, polygon[k + 1].1])).collect()
}

/// A dark line of `thickness` pixels around the shape of the model and where a part of the model is in front of another one
/// (the outline that the game draws). `depth` holds the depth of each pixel that an opaque mesh covers.
fn outline(pixels: &mut [u8], depth: &[f32], width: usize, height: usize, radius: f32, thickness: usize) {
    let jump = radius * 0.05;
    let covered: Vec<bool> = depth.iter().map(|&d| d < f32::MAX).collect();
    let mut dark = vec![false; width * height];
    // Inside the shape, behind a part that is much nearer: the line is drawn in the pixel that is farther, as wide as the
    // outline of the shape.
    for y in 0..height {
        for x in 0..width {
            let at = y * width + x;
            if !covered[at] {
                continue;
            }
            'near: for step in 1..=thickness {
                for (dx, dy) in [(step as isize, 0isize), (0, step as isize), (-(step as isize), 0), (0, -(step as isize))] {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                        continue;
                    }
                    let other = ny as usize * width + nx as usize;
                    if covered[other] && depth[other] + jump < depth[at] {
                        dark[at] = true;
                        break 'near;
                    }
                }
            }
        }
    }
    // Around the shape: the empty pixels that are at most `thickness` pixels from the shape (a round line).
    let mut reached = covered.clone();
    for _ in 0..thickness {
        let before = reached.clone();
        for y in 0..height {
            for x in 0..width {
                let at = y * width + x;
                if before[at] {
                    continue;
                }
                let near = (x > 0 && before[at - 1]) || (x + 1 < width && before[at + 1]) || (y > 0 && before[at - width]) || (y + 1 < height && before[at + width]);
                if near {
                    reached[at] = true;
                    dark[at] = true;
                }
            }
        }
    }
    for (at, &mark) in dark.iter().enumerate() {
        if mark {
            pixels[at * 4..at * 4 + 4].copy_from_slice(&[20, 14, 24, 255]);
        }
    }
}

/// True for a material that the game draws with several steps and the opacity of its pictures: the iris of the eyes,
/// the flames, the gas. The ones that the light does not touch (the file gives them no reflection) and the ones that
/// the file marks as transparent.
pub fn is_mixed(material: &crate::model::Material) -> bool {
    let state = &material.state;
    if state.known {
        // The game gives the way to draw the material: it is mixed when the new colour is blended with the old one.
        return state.blend && (state.color_source, state.color_destination) != (1, 0);
    }
    material.transparent || (material.unlit && !material.stages.is_empty())
}

/// The eye that an iris lies on: its material, its picture and the places of the picture where the eye is open.
type Eye<'a> = (&'a crate::model::Material, &'a Texture, std::rc::Rc<Vec<bool>>);

/// True where the iris is not seen: the mesh of an iris is bigger than the opening of the eye, and the picture of the iris is
/// kept only where the picture of the eye is not the colour of the skin.
fn outside_eye((material, texture, opening): &Eye, uv: [f32; 2]) -> bool {
    let (u, v) = texture_place(material, uv);
    let column = (wrap(u, material.texture_wrap[0]) * texture.width as f32).min(texture.width as f32 - 1.0) as usize;
    let row = (wrap(v, material.texture_wrap[1]) * texture.height as f32).min(texture.height as f32 - 1.0) as usize;
    !opening[row * texture.width + column]
}

/// True for a pixel of a plain picture that is not drawn. A transparent material (the iris of the eyes) is cut where its
/// picture has no opacity: the mesh behind it (the white of the eye) shows there. For the other materials the fourth channel
/// is not always an opacity: the pictures mark the places that the model does not fill (the shape of a tail, of an ear, of a
/// wing) with black and no opacity, and only that mark cuts.
pub fn is_cut(material: Option<&crate::model::Material>, color: [u8; 4]) -> bool {
    match material {
        Some(material) if material.transparent => color[3] < 128,
        _ => color[3] < 128 && color[0] < 12 && color[1] < 12 && color[2] < 12,
    }
}

/// What the viewer shows on a mesh at a texture position when the light does not dim it. The export of a model makes the
/// pictures of its materials from it.
pub struct Surface<'a> {
    material: &'a crate::model::Material,
    texture: Option<&'a Texture>,
    pictures: [Option<&'a Texture>; 3],
    eye: Option<Eye<'a>>,
}

impl<'a> Surface<'a> {
    /// The surface of a mesh of a model. `None` when the mesh has no material.
    pub fn new(model: &'a Model, textures: &'a HashMap<String, Texture>, mesh: &crate::model::Mesh) -> Option<Surface<'a>> {
        let material = model.materials.get(mesh.material)?;
        let eye = if material.transparent { eye_under(model, textures, mesh) } else { None };
        Some(Surface { material, texture: textures.get(&material.texture), pictures: std::array::from_fn(|k| textures.get(&material.units[k].texture)), eye })
    }

    /// True when the colour comes from the steps that mix the pictures of the material, and not from one picture.
    pub fn is_combined(&self) -> bool {
        is_mixed(self.material) || (self.material.unlit && !self.material.stages.is_empty())
    }

    /// The name of the picture of the eye that cuts this mesh (an iris), when it has one.
    pub fn eye_picture(&self) -> Option<&'a str> {
        self.eye.as_ref().map(|(_, texture, _)| texture.name.as_str())
    }

    /// The colour of the picture of the material alone at a texture position of the mesh, when the material has one.
    pub fn picture_color(&self, uv: [f32; 2]) -> Option<[u8; 4]> {
        let (u, v) = texture_place(self.material, uv);
        self.texture.map(|texture| sample_wrapped(texture, u, v, self.material.texture_wrap))
    }

    /// The colour and the opacity (0 to 1) at a texture position of the mesh, for a vertex of the colour `primary`. An
    /// opacity of 0 means that nothing is drawn.
    pub fn at(&self, uv: [f32; 2], primary: [f32; 4]) -> [f32; 4] {
        let hidden = self.eye.as_ref().is_some_and(|eye| outside_eye(eye, uv));
        if !self.is_combined() {
            let mut color = self.texture.map_or([170, 170, 170, 255], |texture| {
                let (u, v) = texture_place(self.material, uv);
                sample_wrapped(texture, u, v, self.material.texture_wrap)
            });
            if hidden {
                color[3] = 0;
            }
            let opacity = if hidden || is_cut(Some(self.material), color) { 0.0 } else { 1.0 };
            return [f32::from(color[0]) / 255.0, f32::from(color[1]) / 255.0, f32::from(color[2]) / 255.0, opacity];
        }
        let mut color = combine(self.material, &self.pictures, uv, 1.0, primary);
        if hidden {
            color[3] = 0.0;
        }
        let state = &self.material.state;
        let drawn = match state.alpha_test {
            Some((function, reference)) => depth_passes(function, (color[3] * 255.0).round(), f32::from(reference)),
            None => color[3] >= 0.02,
        };
        if !drawn {
            color[3] = 0.0;
        } else if state.known && !is_mixed(self.material) {
            // The new colour takes the place of the old one: the pixel is opaque whatever the opacity.
            color[3] = 1.0;
        }
        color
    }
}

/// The result of a comparison of the graphics chip (0 never, 1 always, 2 equal, 3 not equal, 4 less, 5 less or equal,
/// 6 greater, 7 greater or equal) between a new value and the value that is there.
fn depth_passes(function: u8, new: f32, old: f32) -> bool {
    match function {
        0 => false,
        1 => true,
        2 => new == old,
        3 => new != old,
        4 => new < old,
        5 => new <= old,
        6 => new > old,
        _ => new >= old,
    }
}

/// The colour and the opacity of a pixel when a new colour is drawn over the old one, with the factors of the material.
/// The opacity of a pixel tells how much of it is covered. The colour that is given and returned is not multiplied by it.
pub fn blend(state: &crate::model::RenderState, source: [f32; 3], source_alpha: f32, behind: [f32; 3], behind_alpha: f32) -> ([f32; 3], f32) {
    let factor = |code: u8, channel: usize| -> f32 {
        match code {
            0 => 0.0,
            1 => 1.0,
            2 => source[channel],
            3 => 1.0 - source[channel],
            4 => behind[channel],
            5 => 1.0 - behind[channel],
            6 => source_alpha,
            7 => 1.0 - source_alpha,
            8 => behind_alpha,
            9 => 1.0 - behind_alpha,
            _ => 1.0,
        }
    };
    if !state.blend {
        return (source, 1.0);
    }
    let mut out = [0.0f32; 3];
    for (channel, value) in out.iter_mut().enumerate() {
        // The old colour counts with its coverage: a pixel that nothing covered has no colour.
        *value = source[channel] * factor(state.color_source, channel) + behind[channel] * behind_alpha * factor(state.color_destination, channel);
    }
    let coverage = match (state.color_source, state.color_destination) {
        (_, 7) => source_alpha + behind_alpha * (1.0 - source_alpha),
        (_, 0) => 1.0,
        _ => (source_alpha + behind_alpha).min(1.0),
    };
    if coverage <= 1e-6 {
        return (behind, behind_alpha);
    }
    (out.map(|v| v / coverage), coverage)
}

/// The colour and the opacity of a pixel (from 0 to 1) that a material gives at a texture position, with the steps of
/// the mix that the file gives. `light` is the light of the surface, from 0 to 1.
pub fn combine(material: &crate::model::Material, textures: &[Option<&Texture>; 3], uv: [f32; 2], light: f32, primary: [f32; 4]) -> [f32; 4] {
    // The three pictures, at this place. A unit with no picture gives white.
    let mut pictures = [[1.0f32; 4]; 3];
    for ((unit, picture), texture) in material.units.iter().zip(pictures.iter_mut()).zip(textures) {
        if let Some(texture) = texture {
            let (u, v) = (unit.scale[0] * (uv[0] - unit.translation[0]), unit.scale[1] * (uv[1] - unit.translation[1]));
            *picture = sample_wrapped(texture, u, v, unit.wrap).map(|c| c as f32 / 255.0);
        }
    }
    // The materials that the light does not touch keep an opacity that a step would set to one (see `combine_values`).
    combine_values(&material.stages, material.buffer_color, &pictures, light, primary, material.unlit)
}

/// The steps of a mix on the values of a pixel: the colours of the three pictures at that place, the light of the surface
/// and the colour of the vertex. With `keep_opacity`, a step that only sets the opacity from its own colour keeps the
/// opacity of the steps before.
pub fn combine_values(stages: &[crate::model::Stage], buffer_color: [u8; 4], pictures: &[[f32; 4]; 3], light: f32, primary: [f32; 4], keep_opacity: bool) -> [f32; 4] {
    use crate::model::Stage;
    let to_float = |bytes: [u8; 4]| bytes.map(|c| c as f32 / 255.0);
    let mut previous = [1.0f32; 4];
    let mut buffer = to_float(buffer_color);
    for stage in stages.iter() {
        let constant = to_float(stage.constant);
        let source = |code: u8, previous: [f32; 4], buffer: [f32; 4]| -> [f32; 4] {
            match code {
                0 => primary,
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
        // A step that sets the opacity from its own colour only (mode 0, source 14) gives full opacity: for the materials that
        // the light does not touch (flames, gas) the opacity that the steps before computed is kept.
        if keep_opacity && stage.alpha_mode == 0 && stage.alpha_source[0] == 14 && stage.alpha_operand[0] == 0 {
            alpha = previous[3];
        }
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
/// transparent and that covers all the texture positions of the iris. `None` when there is no such mesh, and when the
/// transparent mesh goes all around it: it does not lie on it (a sphere of gas around its core). An iris or an eye, by the
/// name of its material, lies only on an eye: after an export and an import, the texture positions of a part of the body can
/// cover the ones of the eyes.
fn eye_under<'a>(model: &'a Model, textures: &'a HashMap<String, Texture>, iris: &crate::model::Mesh) -> Option<(&'a crate::model::Material, &'a Texture, std::rc::Rc<Vec<bool>>)> {
    if iris.vertices.is_empty() {
        return None;
    }
    let is_eye = |name: &str| name.to_lowercase().contains("eye");
    let own = model.materials.get(iris.material)?.name.to_lowercase();
    let of_an_eye = is_eye(&own) || own.contains("iris");
    let area = uv_box(iris);
    let margin = 0.01;
    // Of the meshes that cover the iris, the eye is the one with the smallest box of texture positions.
    model
        .meshes
        .iter()
        .filter(|mesh| !mesh.hidden)
        .filter_map(|mesh| {
            let material = model.materials.get(mesh.material).filter(|material| !material.transparent && (!of_an_eye || is_eye(&material.name)))?;
            let texture = textures.get(&material.texture)?;
            let b = uv_box(mesh);
            (!mesh.vertices.is_empty() && b[0] <= area[0] + margin && b[1] >= area[1] - margin && b[2] <= area[2] + margin && b[3] >= area[3] - margin).then_some(((b[1] - b[0]) * (b[3] - b[2]), material, texture, mesh))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|(_, _, _, eye)| !goes_around(iris, eye))
        .map(|(_, material, texture, _)| (material, texture, cached_opening(texture)))
}

/// True when the box of the space of a mesh holds the box of another one and is larger on every side.
fn goes_around(mesh: &crate::model::Mesh, other: &crate::model::Mesh) -> bool {
    let bounds = |mesh: &crate::model::Mesh| {
        mesh.vertices.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(low, high), v| (std::array::from_fn(|k| low[k].min(v.position[k])), std::array::from_fn(|k| high[k].max(v.position[k]))))
    };
    let ((low, high), (from, to)) = (bounds(mesh), bounds(other));
    let margin = 0.1 * (0..3).map(|k| to[k] - from[k]).fold(0.0, f32::max);
    (0..3).all(|k| low[k] < from[k] - margin && high[k] > to[k] + margin)
}

thread_local! {
    /// The openings that were found, by picture: the search reads the whole picture, and the drawing is made many times.
    static OPENINGS: std::cell::RefCell<HashMap<(usize, usize, usize, u64), std::rc::Rc<Vec<bool>>>> = std::cell::RefCell::new(HashMap::new());
}

/// `opening`, but found only once for each picture.
fn cached_opening(texture: &Texture) -> std::rc::Rc<Vec<bool>> {
    // A few bytes of the picture tell two pictures apart when a new one takes the place of an old one in memory.
    let step = (texture.rgba.len() / 64).max(1);
    let checksum = texture.rgba.iter().step_by(step).fold(1469598103934665603u64, |hash, &byte| (hash ^ byte as u64).wrapping_mul(1099511628211));
    let key = (texture.rgba.as_ptr() as usize, texture.width, texture.height, checksum);
    OPENINGS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 64 {
            cache.clear();
        }
        cache.entry(key).or_insert_with(|| std::rc::Rc::new(opening(texture))).clone()
    })
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
mod inside_test {
    use super::*;

    /// A triangle that crosses the plane in front of the point of view is cut: the pieces are in front of it, and their weights
    /// give their corners back.
    #[test]
    fn a_triangle_is_cut_at_the_near_plane() {
        let points = [[0.0, 0.0, -1.0], [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0]];
        let pieces = clip_near(points, 0.1);
        assert_eq!(pieces.len(), 2);
        for (corners, weights) in pieces {
            for (corner, weight) in corners.iter().zip(weights) {
                assert!(corner[2] >= 0.1 - 1e-5);
                let back: [f32; 3] = std::array::from_fn(|axis| weight[0] * points[0][axis] + weight[1] * points[1][axis] + weight[2] * points[2][axis]);
                assert!((0..3).all(|axis| (back[axis] - corner[axis]).abs() < 1e-5));
            }
        }
        assert!(clip_near([[0.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 1.0, -1.0]], 0.1).is_empty());
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::model::{Material, Mesh, Vertex};

    /// A model that is one triangle with the material 0, whose picture is a red pixel row.
    fn triangle() -> (Model, HashMap<String, Texture>) {
        let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv: [u, v], ..Vertex::default() };
        let mesh = Mesh { material: 0, vertices: vec![vertex(-1.0, -1.0, 0.0, 0.0), vertex(1.0, -1.0, 1.0, 0.0), vertex(0.0, 1.0, 0.5, 1.0)], triangles: vec![[0, 1, 2]], ..Mesh::default() };
        let model = Model { bones: Vec::new(), materials: vec![Material { name: "m".into(), texture: "t".into(), texture_scale: [1.0, 1.0], texture_translation: [0.0, 0.0], texture_wrap: [2, 2], ..Material::default() }], meshes: vec![mesh] };
        let texture = Texture { name: "t".into(), width: 8, height: 8, rgba: [255u8, 0, 0, 255].repeat(64) };
        (model, HashMap::from([("t".to_string(), texture)]))
    }

    #[test]
    fn a_triangle_is_drawn_with_its_picture_and_the_rest_is_transparent() {
        let (model, textures) = triangle();
        let image = render(&model, &textures, &Camera { yaw: 0.0, pitch: 0.0, zoom: 1.0, outline: false, inside: false }, 64, 64);
        assert_eq!(image.len(), 64 * 64 * 4);
        let centre = &image[(32 * 64 + 32) * 4..][..4];
        assert!(centre[0] > 200 && centre[1] == 0 && centre[3] == 255, "{centre:?}");
        assert_eq!(image[3], 0, "the corner is transparent");
        // Without its picture the triangle is grey.
        let grey = render(&model, &HashMap::new(), &Camera::default(), 64, 64);
        assert!(grey.chunks(4).any(|p| p[3] == 255 && p[0] == p[1] && p[1] == p[2]));
    }

    /// The camera of the front view: the point of view is on the +z side, where the models look.
    fn front() -> Camera {
        Camera { yaw: std::f32::consts::PI, pitch: 0.0, zoom: 1.0, outline: false, inside: false }
    }

    /// The number of pixels that the picture covers in its left half and in its right half.
    fn halves(image: &[u8], width: usize) -> (usize, usize) {
        let covered = |right: bool| image.chunks(4).enumerate().filter(|(at, p)| (at % width >= width / 2) == right && p[3] > 0).count();
        (covered(false), covered(true))
    }

    #[test]
    fn the_picture_is_not_a_mirror() {
        let (mut model, textures) = triangle();
        // The triangle is on the +x side of the model: seen from the front, it is on the right of the picture.
        for vertex in &mut model.meshes[0].vertices {
            vertex.position[0] = vertex.position[0] * 0.25 + 0.6;
        }
        let image = render_framed(&model, &textures, &front(), ([0.0; 3], 1.5), 64, 64);
        let (left, right) = halves(&image, 64);
        assert!(left == 0 && right > 0, "{left} pixels on the left, {right} on the right");
        // The same from inside a place, looking at +z: what is at +x is on the left.
        for vertex in &mut model.meshes[0].vertices {
            vertex.position[2] = 3.0;
        }
        let mut corner = model.meshes[0].clone();
        corner.material = 5;
        for vertex in &mut corner.vertices {
            // A point at the other end, so that the point of view stands at x = 0.
            vertex.position = [-0.85, -2.0, -3.0];
        }
        model.meshes.push(corner);
        let inside = Camera { yaw: 0.0, inside: true, ..front() };
        let image = render(&model, &textures, &inside, 64, 64);
        let (left, right) = halves(&image, 64);
        assert!(left > 0 && right == 0, "{left} pixels on the left, {right} on the right");
    }

    #[test]
    fn a_mesh_is_cut_only_by_the_eye_that_it_lies_on() {
        let (mut model, mut textures) = triangle();
        // A transparent green triangle in front of a triangle with a light picture of one colour: seen as the picture of an
        // eye, it is skin everywhere.
        textures.get_mut("t").unwrap().rgba = [255u8, 210, 180, 255].repeat(64);
        let green = Texture { name: "green".to_string(), width: 1, height: 1, rgba: vec![0, 255, 0, 255] };
        textures.insert(green.name.clone(), green);
        let mut material = model.materials[0].clone();
        (material.texture, material.transparent, material.state.known) = ("green".to_string(), true, true);
        let mut mesh = model.meshes[0].clone();
        mesh.material = model.materials.len();
        for vertex in &mut mesh.vertices {
            vertex.position[2] += 0.1;
        }
        model.materials.push(material);
        model.meshes.push(mesh);
        let green_seen = |model: &Model| render(model, &textures, &front(), 64, 64).chunks(4).any(|p| p[1] > 100 && p[0] < 50);
        assert!(!green_seen(&model), "a mesh that lies on a skin is not drawn there");
        // An iris lies only on an eye.
        model.materials[1].name = "LIris".to_string();
        assert!(green_seen(&model), "the other mesh is not an eye");
        model.materials[0].name = "Eye".to_string();
        assert!(!green_seen(&model), "the iris is not drawn on the skin around the eye");
        (model.materials[0].name, model.materials[1].name) = ("m".to_string(), "m".to_string());
        // Larger than the mesh under it on every side, like a sphere of gas around its core.
        for (vertex, depth) in model.meshes[1].vertices.iter_mut().zip([-0.5, 0.5, 0.5]) {
            vertex.position = [vertex.position[0] * 1.5, vertex.position[1] * 1.5, depth];
        }
        assert!(green_seen(&model), "the gas is drawn");
    }

    #[test]
    fn the_faces_that_the_material_hides_are_the_right_ones() {
        let (mut model, textures) = triangle();
        let seen = |model: &Model, yaw: f32| render(model, &textures, &Camera { yaw, ..front() }, 64, 64).chunks(4).any(|p| p[3] > 0);
        // The corners of the triangle turn against the clock when it is seen from +z: that side is its front.
        model.materials[0].state.cull = 1;
        assert!(seen(&model, std::f32::consts::PI) && !seen(&model, 0.0), "the back faces are hidden");
        model.materials[0].state.cull = 0;
        assert!(!seen(&model, std::f32::consts::PI) && seen(&model, 0.0), "the front faces are hidden");
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
        let image = render(&model, &textures, &Camera { yaw: 0.0, pitch: 0.0, zoom: 1.0, outline: false, inside: false }, 64, 64);
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
                    let image = render(&model, &textures, &Camera { yaw, pitch: 0.15, zoom: 1.0, outline: false, inside: false }, 400, 400);
                    image::save_buffer(format!("{out}/{name}_{side}.png"), &image, 400, 400, image::ColorType::Rgba8).unwrap();
                }
            }
        }
    }

    #[test]
    fn a_model_fills_the_picture_and_stays_in_it_when_it_turns() {
        // A long flat model and a tall thin one, far from the origin.
        for (wide, tall) in [(20.0f32, 1.0f32), (1.0, 30.0)] {
            let (mut model, textures) = triangle();
            let corner = |x: f32, y: f32| Vertex { position: [100.0 + x * wide, 50.0 + y * tall, 7.0], normal: [0.0, 0.0, 1.0], ..Vertex::default() };
            model.meshes[0].vertices = vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            model.meshes[0].triangles = vec![[0, 1, 2], [0, 2, 3]];
            for yaw in [std::f32::consts::PI, 2.6, 0.4, 5.5] {
                let image = render(&model, &textures, &Camera { yaw, pitch: 0.15, zoom: 1.0, outline: false, inside: false }, 100, 100);
                let drawn: Vec<(usize, usize)> = (0..100 * 100).filter(|at| image[at * 4 + 3] > 0).map(|at| (at % 100, at / 100)).collect();
                assert!(drawn.iter().all(|&(x, y)| x > 0 && x < 99 && y > 0 && y < 99), "{wide} by {tall}, turned {yaw}: it leaves the picture");
                if yaw == std::f32::consts::PI {
                    let size = |values: Vec<usize>| values.iter().max().unwrap() - values.iter().min().unwrap();
                    let largest = size(drawn.iter().map(|p| p.0).collect()).max(size(drawn.iter().map(|p| p.1).collect()));
                    assert!(largest >= 80, "{wide} by {tall}: only {largest} pixels of 100");
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
