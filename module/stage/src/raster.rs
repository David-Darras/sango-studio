//! The drawing of the models of the stage on the processor, with perspective, over a picture that keeps the distance of
//! what it holds: the particles then go behind what is nearer.

use crate::math::{self, V3};
use sango_core::model::{Model, Texture};
use sango_core::posing::Affine;
use sango_core::render::{blend, depth_passes, Surface};
use sango_effect::View;
use std::collections::HashMap;

/// A picture that is being drawn: colours from 0 to 255, and the distance of each pixel along the look of the camera.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub color: Vec<[f32; 3]>,
    pub depth: Vec<f32>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        Canvas { width, height, color: vec![[0.0; 3]; width * height], depth: vec![f32::MAX; width * height] }
    }

    /// Fills the picture with a colour that goes from the one at the top to the one in the middle and below.
    pub fn gradient(&mut self, top: [f32; 3], middle: [f32; 3]) {
        let half = (self.height / 2).max(1) as f32;
        for (y, row) in self.color.chunks_mut(self.width.max(1)).enumerate() {
            let part = (y as f32 / half).min(1.0);
            let color = std::array::from_fn(|k| math::lerp(top[k], middle[k], part));
            row.iter_mut().for_each(|pixel| *pixel = color);
        }
    }

    /// Forgets the distances: what is drawn next covers everything.
    pub fn forget_depth(&mut self) {
        self.depth.iter_mut().for_each(|d| *d = f32::MAX);
    }

    /// Mixes a colour (from 0 to 1) over the whole picture with a strength from 0 to 1.
    pub fn cover(&mut self, color: [f32; 3], strength: f32) {
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        for pixel in &mut self.color {
            for (channel, value) in pixel.iter_mut().enumerate() {
                *value += (color[channel].clamp(0.0, 1.0) * 255.0 - *value) * strength;
            }
        }
    }

    /// The pixels as red, green, blue and opacity.
    pub fn rgba(&self) -> Vec<u8> {
        self.color.iter().flat_map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, 255]).collect()
    }

    /// A soft dark disc on the ground, under a Pokémon.
    pub fn shadow(&mut self, view: &View, centre: V3, radius: f32, strength: f32) {
        let focal = view.focal * self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let (sx, sy) = ((x as f32 + 0.5 - self.width as f32 / 2.0) / focal, (self.height as f32 / 2.0 - y as f32 - 0.5) / focal);
                let ray: V3 = std::array::from_fn(|k| sx * view.right[k] + sy * view.up[k] + view.forward[k]);
                if ray[1] > -1e-6 {
                    continue;
                }
                let distance = (centre[1] - view.eye[1]) / ray[1];
                let place = math::add(view.eye, math::scaled(ray, distance));
                let from = (place[0] - centre[0]).hypot(place[2] - centre[2]) / radius.max(1e-3);
                let at = y * self.width + x;
                if from < 1.0 && distance > view.near && self.depth[at] >= distance * 0.995 {
                    let dark = strength * ((1.0 - from) * 3.0).min(1.0);
                    self.color[at] = self.color[at].map(|c| c * (1.0 - dark));
                }
            }
        }
    }
}

/// How the light and the colours of a model are made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shade {
    /// A Pokémon: lit from the camera, with an optional colour over it and its strength.
    Pokemon { tint: V3, strength: f32 },
    /// A model of an effect: its own colours.
    Effect,
    /// A place of a battle: its light is in the colours of its vertices.
    Place,
}

/// A model in the stage.
pub struct Item<'a> {
    pub model: &'a Model,
    pub textures: &'a HashMap<String, Texture>,
    /// From the space of the model to the stage.
    pub matrix: Affine,
    pub shade: Shade,
}

/// A corner of a triangle on the screen: the place in pixels, one over the distance, and the values of the vertex each
/// divided by the distance.
#[derive(Clone, Copy)]
struct Corner {
    place: [f32; 2],
    inverse: f32,
    values: [f32; 7],
}

/// A triangle ready to be drawn.
struct Triangle {
    surface: usize,
    corners: [Corner; 3],
}

/// What a corner knows before it goes on the screen: its place from the eye, then the texture position, the colour and
/// the light.
type Seen = (V3, [f32; 7]);

/// The part of a triangle that is in front of the nearest distance, as corners of a polygon.
fn cut(points: [Seen; 3], near: f32) -> Vec<Seen> {
    let mut out = Vec::with_capacity(4);
    for k in 0..3 {
        let (a, b) = (points[k], points[(k + 1) % 3]);
        if a.0[2] >= near {
            out.push(a);
        }
        if (a.0[2] >= near) != (b.0[2] >= near) {
            let t = (near - a.0[2]) / (b.0[2] - a.0[2]);
            out.push((std::array::from_fn(|i| math::lerp(a.0[i], b.0[i], t)), std::array::from_fn(|i| math::lerp(a.1[i], b.1[i], t))));
        }
    }
    out
}

/// The determinant of the turn and size of a matrix: below 0 the matrix mirrors.
fn determinant(matrix: &Affine) -> f32 {
    let m = &matrix.0;
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Draws some models: the meshes that replace what is behind them first, then the ones that are mixed with it.
pub fn draw(canvas: &mut Canvas, view: &View, items: &[Item]) {
    let (width, height) = (canvas.width, canvas.height);
    if width == 0 || height == 0 || items.is_empty() {
        return;
    }
    let focal = view.focal * height as f32;
    // The surfaces of the meshes (item, mesh) that have triangles on the screen, and these triangles.
    let mut surfaces: Vec<(usize, usize, bool, bool)> = Vec::new();
    let mut triangles: Vec<Triangle> = Vec::new();
    for (number, item) in items.iter().enumerate() {
        let mirrored = determinant(&item.matrix) < 0.0;
        for (mesh_number, mesh) in item.model.meshes.iter().enumerate().filter(|(_, mesh)| !mesh.hidden) {
            let Some(material) = item.model.materials.get(mesh.material) else { continue };
            let blended = sango_core::render::is_mixed(material);
            let surface = surfaces.len();
            let corners: Vec<Seen> = mesh
                .vertices
                .iter()
                .map(|vertex| {
                    let place = math::sub(item.matrix.point(vertex.position), view.eye);
                    let seen = [math::dot(place, view.right), math::dot(place, view.up), math::dot(place, view.forward)];
                    let light = match item.shade {
                        Shade::Pokemon { .. } => {
                            let normal = math::normalized(item.matrix.direction(vertex.normal));
                            0.45 + 0.55 * math::dot(normal, view.forward).abs()
                        }
                        _ => 1.0,
                    };
                    let color = vertex.color.map(|c| f32::from(c) / 255.0);
                    (seen, [vertex.uv[0], vertex.uv[1], color[0], color[1], color[2], color[3], light])
                })
                .collect();
            let before = triangles.len();
            for triangle in &mesh.triangles {
                let Some(points) = triangle.iter().map(|&k| corners.get(k as usize).copied()).collect::<Option<Vec<_>>>() else { continue };
                let polygon = cut([points[0], points[1], points[2]], view.near);
                if polygon.len() < 3 {
                    continue;
                }
                let screen: Vec<Corner> = polygon
                    .iter()
                    .map(|(place, values)| {
                        let inverse = 1.0 / place[2];
                        Corner { place: [width as f32 / 2.0 + focal * place[0] * inverse, height as f32 / 2.0 - focal * place[1] * inverse], inverse, values: values.map(|v| v * inverse) }
                    })
                    .collect();
                for k in 1..screen.len() - 1 {
                    let corners = [screen[0], screen[k], screen[k + 1]];
                    let [a, b, c] = corners.map(|c| c.place);
                    let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                    // A face that turns its front to the camera has a negative area (the y axis of the screen goes down).
                    let area_seen = if mirrored { -area } else { area };
                    let culled = match material.state.cull {
                        0 => area_seen < 0.0,
                        1 => area_seen > 0.0,
                        _ => false,
                    };
                    let off = corners.iter().all(|c| c.place[0] < 0.0) || corners.iter().all(|c| c.place[0] > width as f32) || corners.iter().all(|c| c.place[1] < 0.0) || corners.iter().all(|c| c.place[1] > height as f32);
                    if !culled && !off && area.abs() > 1e-6 {
                        triangles.push(Triangle { surface, corners });
                    }
                }
            }
            if triangles.len() > before {
                surfaces.push((number, mesh_number, blended, false));
            }
        }
    }
    if triangles.is_empty() {
        return;
    }
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
    let rows = height.div_ceil(cores * 4).max(1);
    let bands = std::sync::Mutex::new(canvas.color.chunks_mut(rows * width).zip(canvas.depth.chunks_mut(rows * width)).enumerate());
    let (surfaces, triangles) = (&surfaces, &triangles);
    std::thread::scope(|scope| {
        for _ in 0..cores {
            scope.spawn(|| {
                // The surfaces belong to the thread that draws with them.
                let made: Vec<Option<Surface>> = surfaces.iter().map(|&(item, mesh, _, _)| Surface::new(items[item].model, items[item].textures, &items[item].model.meshes[mesh])).collect();
                loop {
                    let next = bands.lock().ok().and_then(|mut bands| bands.next());
                    let Some((number, (color, depth))) = next else { break };
                    let first = number * rows;
                    for pass in [false, true] {
                        for triangle in triangles.iter().filter(|t| surfaces[t.surface].2 == pass) {
                            let Some(surface) = &made[triangle.surface] else { continue };
                            fill(color, depth, width, first, triangle, surface, items[surfaces[triangle.surface].0].shade);
                        }
                    }
                }
            });
        }
    });
}

/// Draws one triangle over the rows of a band, which starts at the row `first`.
fn fill(color: &mut [[f32; 3]], depth: &mut [f32], width: usize, first: usize, triangle: &Triangle, surface: &Surface, shade: Shade) {
    let corners = &triangle.corners;
    let [a, b, c] = corners.map(|c| c.place);
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
    let rows = color.len() / width;
    let low = a[1].min(b[1]).min(c[1]).floor().max(first as f32) as usize;
    let high = (a[1].max(b[1]).max(c[1]).ceil().max(0.0) as usize).min(first + rows);
    let left = a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize;
    let right = (a[0].max(b[0]).max(c[0]).ceil().max(0.0) as usize).min(width);
    let state = surface.state();
    let combined = surface.is_combined() || shade == Shade::Place;
    for y in low..high {
        for x in left..right {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let s0 = ((b[0] - px) * (c[1] - py) - (c[0] - px) * (b[1] - py)) / area;
            let s1 = ((c[0] - px) * (a[1] - py) - (a[0] - px) * (c[1] - py)) / area;
            let s2 = 1.0 - s0 - s1;
            if s0 < 0.0 || s1 < 0.0 || s2 < 0.0 {
                continue;
            }
            let inverse = s0 * corners[0].inverse + s1 * corners[1].inverse + s2 * corners[2].inverse;
            let distance = 1.0 / inverse;
            let at = (y - first) * width + x;
            if depth[at] < f32::MAX && !depth_passes(state.depth_test, distance, depth[at]) {
                continue;
            }
            let value = |k: usize| (s0 * corners[0].values[k] + s1 * corners[1].values[k] + s2 * corners[2].values[k]) * distance;
            let primary = [value(2), value(3), value(4), value(5)];
            let Some(mut found) = surface.lit([value(0), value(1)], primary, value(6), shade == Shade::Place) else { continue };
            if let Shade::Pokemon { tint, strength } = shade {
                for k in 0..3 {
                    found[k] += (tint[k] - found[k]) * strength.clamp(0.0, 1.0);
                }
            }
            let rgb = [found[0], found[1], found[2]];
            let mixed = if combined { blend(state, rgb, found[3], color[at].map(|v| v / 255.0), 1.0).0 } else { rgb };
            color[at] = mixed.map(|v| v.clamp(0.0, 1.0) * 255.0);
            if state.depth_write {
                depth[at] = distance;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::model::{Material, Mesh, RenderState, Unit, Vertex};

    fn square(size: f32, depth: f32, cull: u8) -> Model {
        let corner = |x: f32, y: f32| Vertex { position: [x * size, y * size, depth], normal: [0.0, 0.0, -1.0], uv: [0.0; 2], color: [255; 4], bones: [0; 4], weights: [0.0; 4] };
        let material = Material { name: "plain".into(), texture: String::new(), texture_scale: [1.0; 2], texture_translation: [0.0; 2], texture_wrap: [2; 2], transparent: false, unlit: false, units: <[Unit; 3]>::default(), stages: Vec::new(), state: RenderState { cull, ..Default::default() }, buffer_color: [0; 4], colour_slots: [0; 6] };
        let mesh = Mesh { material: 0, vertices: vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)], triangles: vec![[0, 1, 2], [0, 2, 3]], node: String::new(), hidden: false };
        Model { bones: Vec::new(), materials: vec![material], meshes: vec![mesh] }
    }

    /// A camera at the origin that looks toward +z.
    fn view() -> View {
        View { eye: [0.0; 3], right: [-1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], forward: [0.0, 0.0, 1.0], focal: 1.0, near: 1.0 }
    }

    #[test]
    fn the_nearer_square_hides_the_farther_one() {
        let (near, far) = (square(1.0, 10.0, 2), square(4.0, 20.0, 2));
        let textures = HashMap::new();
        let item = |model, strength| Item { model, textures: &textures, matrix: Affine::IDENTITY, shade: Shade::Pokemon { tint: [1.0, 0.0, 0.0], strength } };
        let mut canvas = Canvas::new(40, 40);
        draw(&mut canvas, &view(), &[item(&near, 1.0), item(&far, 0.0)]);
        let centre = 20 * 40 + 20;
        assert_eq!(canvas.color[centre], [255.0, 0.0, 0.0], "the near square, with its colour over it");
        assert!((canvas.depth[centre] - 10.0).abs() < 0.01);
        // Outside the near square, the far one.
        let side = 20 * 40 + 14;
        assert!((canvas.depth[side] - 20.0).abs() < 0.01, "{}", canvas.depth[side]);
        assert_eq!(canvas.color[0], [0.0; 3], "outside both");
    }

    #[test]
    fn a_part_behind_the_camera_is_cut_away() {
        let mut model = square(5.0, 10.0, 2);
        // A floor under the camera: its far side is in front of it, its other side goes behind it.
        for (k, vertex) in model.meshes[0].vertices.iter_mut().enumerate() {
            (vertex.position[1], vertex.position[2]) = (-3.0, if k < 2 { 10.0 } else { -10.0 });
        }
        let textures = HashMap::new();
        let mut canvas = Canvas::new(20, 20);
        draw(&mut canvas, &view(), &[Item { model: &model, textures: &textures, matrix: Affine::IDENTITY, shade: Shade::Effect }]);
        assert!(canvas.depth.iter().any(|&d| d < f32::MAX) && canvas.depth.iter().all(|&d| d >= 0.99));
    }
}
