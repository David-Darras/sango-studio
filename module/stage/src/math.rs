//! The small vector calculations of the viewer.

use sango_core::posing::{rotation_zyx, Affine};

pub type V3 = [f32; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn mul(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

pub fn scaled(a: V3, factor: f32) -> V3 {
    a.map(|v| v * factor)
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

pub fn normalized(a: V3) -> V3 {
    let size = length(a);
    if size < 1e-9 {
        [0.0; 3]
    } else {
        scaled(a, 1.0 / size)
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn radians(degrees: V3) -> V3 {
    degrees.map(f32::to_radians)
}

/// Turns the point (x, z) by an angle in radians, as the game does it: a quarter of a turn takes +x to +z.
pub fn turn_xz(point: &mut V3, angle: f32) {
    let (sin, cos) = angle.sin_cos();
    let (x, z) = (point[0], point[2]);
    point[0] = x * cos - z * sin;
    point[2] = x * sin + z * cos;
}

/// The matrix of an object: the size first, then the turns around x, y and z (radians), then the place.
pub fn matrix(place: V3, turn: V3, size: V3) -> Affine {
    Affine::new(place, rotation_zyx(turn), size)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_quarter_of_a_turn_takes_x_to_z() {
        let mut point = [1.0, 5.0, 0.0];
        turn_xz(&mut point, std::f32::consts::FRAC_PI_2);
        assert!(point[0].abs() < 1e-6 && (point[2] - 1.0).abs() < 1e-6 && point[1] == 5.0);
    }

    #[test]
    fn an_object_is_sized_then_turned_then_placed() {
        let object = matrix([10.0, 0.0, 0.0], [0.0, std::f32::consts::FRAC_PI_2, 0.0], [2.0, 1.0, 1.0]);
        let point = object.point([1.0, 0.0, 0.0]);
        assert!((point[0] - 10.0).abs() < 1e-5 && (point[2] + 2.0).abs() < 1e-5, "{point:?}");
    }
}
