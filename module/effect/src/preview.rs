//! A preview of an effect: the particles of all the layers at a frame, drawn on the processor.
//!
//! The preview shows what the page edits: when the particles appear (the start, the interval, the span, the number at each emission), where
//! (the shape of the emitter), for how long (the life), and how they look (the size, the colour and the transparency during their life, the
//! picture). It does NOT move the particles: their speeds and forces are not read yet, so each particle stays where it appeared. The
//! of the speed that the layer keeps at each frame; the pictures turn. The other forces are not read yet. The drawing is flat: a square
//! picture facing the viewer, added to the dark background like the light effects of the game.
//!
//! The particles of a frame are computed from the frame alone (no memory of the frames before), so the user can go to any frame.

use crate::effect_file::{ColorCurve, Effect, FormKind, NumberCurve};

/// The most particles that the preview draws at one frame.
const MAX_PARTICLES: usize = 3000;
/// The frames that an emitter without end is shown.
const ENDLESS_FRAMES: f32 = 120.0;

/// A particle at a frame.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub layer: usize,
    pub position: [f32; 3],
    /// The width and the height of the picture.
    pub size: [f32; 2],
    pub color: [f32; 3],
    pub alpha: f32,
    /// The turn of the picture (radians).
    pub angle: f32,
}

/// A picture of the particles, as RGBA pixels.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// The point of view: a turn around the vertical axis, a turn up and down, and a zoom (1 shows the whole effect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        // Seen from the side a little, so that a movement in the depth shows.
        Camera { yaw: 0.6, pitch: 0.25, zoom: 1.0 }
    }
}

/// A number between 0 and 1 from some numbers (a hash: the same numbers always give the same result).
fn random(seed: [u64; 4]) -> f32 {
    let mut x = seed.iter().fold(0x9E37_79B9_7F4A_7C15u64, |acc, &v| (acc ^ v.wrapping_add(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9).rotate_left(27));
    x ^= x >> 30;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t.clamp(0.0, 1.0))
}

/// The value of a curve of 3 numbers at a moment of the life (0 to 1).
fn color_at(curve: &ColorCurve, at: f32) -> [f32; 3] {
    if curve.fixed {
        curve.middle
    } else if at < curve.in_time {
        mix3(curve.start, curve.middle, if curve.in_time > 0.0 { at / curve.in_time } else { 1.0 })
    } else if at > curve.out_time {
        mix3(curve.middle, curve.end, if curve.out_time < 1.0 { (at - curve.out_time) / (1.0 - curve.out_time) } else { 1.0 })
    } else {
        curve.middle
    }
}

fn alpha_at(curve: &NumberCurve, at: f32) -> f32 {
    if curve.fixed {
        curve.peak
    } else if at < curve.in_time {
        curve.start + (curve.peak - curve.start) * if curve.in_time > 0.0 { at / curve.in_time } else { 1.0 }
    } else if at > curve.out_time {
        curve.peak + (curve.end - curve.peak) * if curve.out_time < 1.0 { (at - curve.out_time) / (1.0 - curve.out_time) } else { 1.0 }
    } else {
        curve.peak
    }
}

/// Where a particle appears in the shape of the emitter. `r` are 4 random numbers.
fn spawn_position(kind: Option<FormKind>, values: &dyn Fn(&str) -> [f32; 3], r: [f32; 4]) -> [f32; 3] {
    let size = values("Size");
    let inner = values("Empty part")[0].clamp(0.0, 0.99);
    let tau = std::f32::consts::TAU;
    let arc = values("Arc")[0];
    let (arc, arc_start) = if arc > 0.0 && arc < tau { (arc, values("Arc start")[0]) } else { (tau, 0.0) };
    match kind {
        None | Some(FormKind::Point) => [0.0; 3],
        Some(FormKind::Cube) => [(r[0] - 0.5) * size[0], (r[1] - 0.5) * size[1], (r[2] - 0.5) * size[2]],
        Some(FormKind::Rectangle) => [(r[0] - 0.5) * size[0], (r[1] - 0.5) * size[1], 0.0],
        Some(FormKind::Disc) => {
            let (angle, radius) = (arc_start + arc * r[0], (inner * inner + (1.0 - inner * inner) * r[1]).sqrt());
            [angle.cos() * radius * size[0], angle.sin() * radius * size[1], 0.0]
        }
        Some(FormKind::Cylinder) => {
            let (angle, radius) = (arc_start + arc * r[0], (inner * inner + (1.0 - inner * inner) * r[1]).sqrt());
            [angle.cos() * radius * size[0], (r[2] - 0.5) * size[1], angle.sin() * radius * size[2]]
        }
        Some(FormKind::Sphere) => {
            // A direction on the sphere, then a distance (the cube root keeps the particles evenly spread in the volume).
            let (angle, height) = (arc_start + arc * r[0], r[1] * 2.0 - 1.0);
            let ring = (1.0 - height * height).max(0.0).sqrt();
            let radius = (inner.powi(3) + (1.0 - inner.powi(3)) * r[2]).cbrt();
            [angle.cos() * ring * radius * size[0], height * radius * size[1], angle.sin() * ring * radius * size[2]]
        }
    }
}

/// The frames that an effect lasts: the end of the last particle.
pub fn length(effect: &Effect) -> f32 {
    (0..effect.layer_count())
        .map(|layer| {
            let em = effect.emission(layer);
            let (_, longest) = effect.life(layer).unwrap_or((20.0, 20.0));
            em.start as f32 + if em.infinite { ENDLESS_FRAMES } else { em.span.max(0) as f32 } + longest
        })
        .fold(10.0, f32::max)
}

/// All the particles at a frame.
pub fn particles_at(effect: &Effect, frame: f32) -> Vec<Particle> {
    let mut out = Vec::new();
    for layer in 0..effect.layer_count() {
        let em = effect.emission(layer);
        let (shortest, longest) = effect.life(layer).unwrap_or((20.0, 20.0));
        let (shortest, longest) = (shortest.max(1.0), longest.max(shortest.max(1.0)));
        let kind = effect.form_kind(layer);
        let fields = effect.form_values(layer);
        let value = |name: &str| fields.iter().find(|(field, _)| field.name == name).map_or([0.0, 0.0, 0.0], |(_, v)| *v);
        let size = effect.size(layer);
        let color = effect.color(layer);
        let alpha = effect.alpha(layer);
        let motion = effect.motion(layer);
        let interval = em.interval.max(1) as f32;
        let last = ((frame - em.start as f32) / interval).floor();
        let first = (((frame - longest - em.start as f32) / interval).ceil()).max(0.0);
        let mut k = first;
        while k <= last {
            let birth = em.start as f32 + k * interval;
            let emitting = em.infinite || k * interval < em.span.max(1) as f32;
            if emitting && birth <= frame {
                let seed = |n: u64, c: u64| random([layer as u64, k as u64, n, c]);
                let count = (em.ratio * (1.0 + em.ratio_random * (seed(9999, 0) * 2.0 - 1.0)) + seed(9999, 1)).floor().max(0.0) as u64;
                for n in 0..count {
                    let life = shortest + (longest - shortest) * seed(n, 0);
                    let age = (frame - birth) / life;
                    if (0.0..1.0).contains(&age) {
                        let mut position = spawn_position(kind, &value, [seed(n, 1), seed(n, 2), seed(n, 3), seed(n, 4)]);
                        // The place of the emitter, then the way that the particle went since its birth: the sum of the speeds of the frames,
                        // the speed being multiplied by the part that is kept at each frame.
                        let frames = age * life;
                        let travelled = if (motion.keep - 1.0).abs() < 1e-4 { frames } else { (1.0 - motion.keep.powf(frames)) / (1.0 - motion.keep) };
                        for axis in 0..3 {
                            position[axis] += motion.position[axis] + motion.direction[axis] * motion.speed * travelled;
                        }
                        let start = if motion.turn_random { motion.turn_start * seed(n, 5) } else { motion.turn_start };
                        let angle = start + motion.turn_speed * frames;
                        let scale = size.as_ref().map_or([1.0, 1.0, 1.0], |c| color_at(c, age));
                        out.push(Particle {
                            layer,
                            position,
                            size: [scale[0], scale[1]],
                            color: color.as_ref().map_or([1.0, 1.0, 1.0], |c| color_at(c, age)),
                            alpha: alpha.as_ref().map_or(1.0, |c| alpha_at(c, age)),
                            angle,
                        });
                        if out.len() >= MAX_PARTICLES {
                            return out;
                        }
                    }
                }
            }
            k += 1.0;
        }
    }
    out
}

/// The picture that a layer uses: the picture whose name is in the name of its set or of its emitter, else the picture of the same place.
pub fn picture_of(effect: &Effect, layer: usize) -> Option<usize> {
    let (set, emitter) = (effect.set_name(layer).to_lowercase(), effect.emitter_name(layer).to_lowercase());
    let found = effect.pictures().iter().position(|p| {
        let name = p.name.to_lowercase();
        !name.is_empty() && (set.contains(&name) || emitter.contains(&name) || name.contains(&set) && !set.is_empty())
    });
    found.or_else(|| (!effect.pictures().is_empty()).then(|| layer.min(effect.pictures().len() - 1)))
}

/// The pictures of an effect, one for each layer (a picture of several frames side by side gives its first frame).
pub fn pictures(effect: &Effect) -> Vec<Option<Picture>> {
    (0..effect.layer_count())
        .map(|layer| {
            let index = picture_of(effect, layer)?;
            let info = effect.pictures().get(index)?;
            let rgba = effect.picture_rgba(index)?;
            let (width, height) = (info.width, info.height);
            // A strip of pictures (128 × 32 holds 4 pictures of 32 × 32): the first one.
            let cell = if width > height && height > 0 && width % height == 0 {
                (height, height)
            } else if height > width && width > 0 && height % width == 0 {
                (width, width)
            } else {
                (width, height)
            };
            let mut out = Vec::with_capacity(cell.0 * cell.1 * 4);
            for y in 0..cell.1 {
                out.extend_from_slice(&rgba[y * width * 4..(y * width + cell.0) * 4]);
            }
            Some(Picture { width: cell.0, height: cell.1, rgba: out })
        })
        .collect()
}

/// The radius of the area that the preview shows with the default point of view: the farthest particle at some frames, with its size.
pub fn extent(effect: &Effect) -> f32 {
    let end = length(effect);
    let camera = Camera::default();
    let (sin_y, cos_y, sin_p, cos_p) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    let mut radius = 1.0f32;
    for step in 0..12 {
        for p in particles_at(effect, end * step as f32 / 12.0) {
            let (x, y, z) = (p.position[0], p.position[1], p.position[2]);
            let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
            let y2 = y * cos_p - z1 * sin_p;
            radius = radius.max((x1 * x1 + y2 * y2).sqrt() + p.size[0].max(p.size[1]) * 0.5);
        }
    }
    radius * 1.1
}

/// Draws the effect at a frame on a dark background. Returns `side` × `side` pixels of RGBA.
pub fn render(effect: &Effect, pictures: &[Option<Picture>], frame: f32, camera: Camera, extent: f32, side: usize) -> Vec<u8> {
    let mut canvas = vec![[24.0f32, 26.0, 34.0]; side * side];
    let scale = side as f32 / (2.0 * extent / camera.zoom.max(0.05));
    let centre = side as f32 / 2.0;
    let (sin_y, cos_y, sin_p, cos_p) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    for p in particles_at(effect, frame) {
        // Turn the point of view: around the vertical axis, then up and down.
        let (x, y, z) = (p.position[0], p.position[1], p.position[2]);
        let (x1, z1) = (x * cos_y + z * sin_y, -x * sin_y + z * cos_y);
        let (y2, _) = (y * cos_p - z1 * sin_p, y * sin_p + z1 * cos_p);
        let (screen_x, screen_y) = (centre + x1 * scale, centre - y2 * scale);
        let (half_w, half_h) = ((p.size[0].abs() * scale * 0.5).max(1.0), (p.size[1].abs() * scale * 0.5).max(1.0));
        let picture = pictures.get(p.layer).and_then(Option::as_ref);
        // A turned picture covers a bigger area: the corners go out of the box.
        let reach = if p.angle == 0.0 { half_w.max(half_h) } else { (half_w * half_w + half_h * half_h).sqrt() };
        let (x_from, x_to) = (((screen_x - reach).floor().max(0.0)) as usize, ((screen_x + reach).ceil().min(side as f32)) as usize);
        let (y_from, y_to) = (((screen_y - reach).floor().max(0.0)) as usize, ((screen_y + reach).ceil().min(side as f32)) as usize);
        let (sin_a, cos_a) = (p.angle.sin(), p.angle.cos());
        for py in y_from..y_to {
            for px in x_from..x_to {
                // The place of the pixel in the picture: turn the pixel back around the centre of the particle.
                let (dx, dy) = (px as f32 + 0.5 - screen_x, py as f32 + 0.5 - screen_y);
                let (rx, ry) = (dx * cos_a + dy * sin_a, -dx * sin_a + dy * cos_a);
                let (u, v) = (rx / (2.0 * half_w) + 0.5, ry / (2.0 * half_h) + 0.5);
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                // A particle without a picture is a soft round spot.
                let texel = match picture {
                    Some(picture) => {
                        let (tx, ty) = (((u * picture.width as f32) as usize).min(picture.width - 1), ((v * picture.height as f32) as usize).min(picture.height - 1));
                        let at = (ty * picture.width + tx) * 4;
                        // The light effects of the game add the colour of the picture, weighted by its transparency.
                        let a = picture.rgba[at + 3] as f32 / 255.0;
                        [picture.rgba[at], picture.rgba[at + 1], picture.rgba[at + 2]].map(|c| c as f32 / 255.0 * a)
                    }
                    None => {
                        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0;
                        let soft = (1.0 - d).clamp(0.0, 1.0);
                        [soft, soft, soft]
                    }
                };
                let pixel = &mut canvas[py * side + px];
                for k in 0..3 {
                    pixel[k] = (pixel[k] + texel[k] * p.color[k].clamp(0.0, 4.0) * p.alpha.clamp(0.0, 1.0) * 255.0).min(255.0);
                }
            }
        }
    }
    canvas.into_iter().flat_map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, 255]).collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    /// With the dump of the game: the particles of some effects are computed, and the preview draws something that is not the background.
    #[test]
    fn the_preview_draws_the_particles_of_the_effects() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        let (mut drawn, mut tried) = (0, 0);
        for id in (0..garc.file_count()).step_by(23) {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let Ok(effect) = Effect::parse(&bytes) else { continue };
            tried += 1;
            let pictures = pictures(&effect);
            let extent = extent(&effect);
            assert!(extent.is_finite() && extent > 0.0);
            let end = length(&effect);
            let background = [24u8, 26, 34, 255];
            let any = (1..8).any(|step| render(&effect, &pictures, end * step as f32 / 9.0, Camera::default(), extent, 96).chunks(4).any(|p| p != background));
            drawn += usize::from(any);
        }
        assert!(tried > 50);
        assert!(drawn * 10 > tried * 6, "{drawn} of {tried} effects show particles");
        // The same frame always gives the same particles.
        let bytes = garc.first_file_decompressed(0).unwrap();
        let effect = Effect::parse(&bytes).unwrap();
        assert_eq!(particles_at(&effect, 10.0).len(), particles_at(&effect, 10.0).len());
        // The fire beam (the effect 0) shoots its particles along the axis of the beam: they get far from the emitter, and the speed that
        // the file gives is the one that is read.
        let motion = effect.motion(0);
        assert!(motion.speed > 0.0 && motion.direction[2] < 0.0 && motion.keep > 0.0, "{motion:?}");
        let far = particles_at(&effect, 30.0).iter().map(|p| p.position[2].abs()).fold(0.0f32, f32::max);
        assert!(far > 20.0, "the particles of the beam travel ({far})");
    }

    /// Writes a strip of pictures of some effects in the folder named by `SANGO_PREVIEW_OUT`, to look at them.
    #[test]
    fn write_some_pictures_to_look_at() {
        let (Ok(root), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("SANGO_PREVIEW_OUT")) else { return };
        let dump = Dump::open(&root).unwrap();
        let garc = Garc::open(&dump.archive(31)).unwrap();
        for id in [0usize, 5, 100, 250, 723] {
            let Some(bytes) = garc.first_file_decompressed(id) else { continue };
            let Ok(effect) = Effect::parse(&bytes) else { continue };
            let (pictures, extent, end) = (pictures(&effect), extent(&effect), length(&effect));
            let side = 200;
            let mut strip = image::RgbaImage::new(side as u32 * 6, side as u32);
            for step in 0..6 {
                let pixels = render(&effect, &pictures, end * (step as f32 + 0.5) / 6.0, Camera::default(), extent, side);
                let frame = image::RgbaImage::from_raw(side as u32, side as u32, pixels).unwrap();
                image::imageops::replace(&mut strip, &frame, side as i64 * step, 0);
            }
            strip.save(std::path::Path::new(&out).join(format!("effect_{id}.png"))).unwrap();
        }
    }
}
