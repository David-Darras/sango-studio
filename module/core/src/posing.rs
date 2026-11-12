//! Puts a model in a pose: the bones move, and the vertices follow the bones that move them.
//!
//! The model file holds the model in its rest pose. A pose (see `motion`) gives the translation, the rotation and
//! the scale of some bones. A bone that the pose does not name keeps the values of the file. The world matrix of a bone
//! is the world matrix of its parent times its own matrix (translation, then rotation z y x, then scale). A vertex goes
//! to the sum, over its bones, of the weight times the world matrix of the bone times the inverse of the world matrix of
//! the same bone in the rest pose.

use crate::model::{Bone, Model};
use crate::motion::{MotionPack, Pose};
use std::collections::HashMap;

/// A matrix of 3 rows and 4 columns: a rotation with a scale, and a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [[f32; 4]; 3]);

impl Affine {
    pub const IDENTITY: Affine = Affine([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]);

    /// The matrix of a bone: scale first, then the rotation, then the translation.
    pub fn new(translation: [f32; 3], rotation: [[f32; 3]; 3], scale: [f32; 3]) -> Affine {
        Affine::with_parent_scale(translation, rotation, scale, [1.0; 3])
    }

    /// The same, for a bone that does not take the scale of its parent: the translation is in the space of the parent
    /// (so it is scaled), the rotation and the scale of the bone are not.
    pub fn with_parent_scale(translation: [f32; 3], rotation: [[f32; 3]; 3], scale: [f32; 3], parent_scale: [f32; 3]) -> Affine {
        let mut m = [[0.0; 4]; 3];
        for row in 0..3 {
            for column in 0..3 {
                m[row][column] = rotation[row][column] / parent_scale[row] * scale[column];
            }
            m[row][3] = translation[row];
        }
        Affine(m)
    }

    /// `self` after `other`: the matrix that does `other` first.
    pub fn then(&self, other: &Affine) -> Affine {
        let (a, b) = (&self.0, &other.0);
        let mut m = [[0.0; 4]; 3];
        for row in 0..3 {
            for column in 0..4 {
                m[row][column] = a[row][0] * b[0][column] + a[row][1] * b[1][column] + a[row][2] * b[2][column] + if column == 3 { a[row][3] } else { 0.0 };
            }
        }
        Affine(m)
    }

    /// The matrix that undoes this one, or `None` when it flattens space (a scale of 0).
    pub fn inverse(&self) -> Option<Affine> {
        let m = &self.0;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        if det.abs() < 1e-9 {
            return None;
        }
        let d = 1.0 / det;
        let mut inv = [[0.0; 4]; 3];
        inv[0][0] = (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d;
        inv[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d;
        inv[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d;
        inv[1][0] = (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d;
        inv[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d;
        inv[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d;
        inv[2][0] = (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d;
        inv[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d;
        inv[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d;
        for row in 0..3 {
            inv[row][3] = -(inv[row][0] * m[0][3] + inv[row][1] * m[1][3] + inv[row][2] * m[2][3]);
        }
        Some(Affine(inv))
    }

    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|r| self.0[r][0] * p[0] + self.0[r][1] * p[1] + self.0[r][2] * p[2] + self.0[r][3])
    }

    pub fn direction(&self, d: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|r| self.0[r][0] * d[0] + self.0[r][1] * d[1] + self.0[r][2] * d[2])
    }
}

/// The rotation of three angles: around z, then y, then x (the matrix is `Rz · Ry · Rx`).
pub fn rotation_zyx(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let ((sx, cx), (sy, cy), (sz, cz)) = (angles[0].sin_cos(), angles[1].sin_cos(), angles[2].sin_cos());
    [[cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx], [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx], [-sy, cy * sx, cy * cx]]
}

/// The rotation of a quaternion (x, y, z, w).
pub fn rotation_quaternion(q: [f32; 4]) -> [[f32; 3]; 3] {
    let length = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt().max(1e-12);
    let [x, y, z, w] = q.map(|v| v / length);
    [[1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)], [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)], [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)]]
}

fn transpose(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]))
}

fn multiply(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    std::array::from_fn(|r| std::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c]))
}

/// The world matrix of every bone, from the matrix of each bone itself. The parent of a bone can come after it.
fn worlds(bones: &[Bone], locals: &[Affine]) -> Vec<Affine> {
    let mut done: Vec<Option<Affine>> = vec![None; bones.len()];
    for start in 0..bones.len() {
        let mut chain = vec![start];
        while let Some(&last) = chain.last() {
            match bones[last].parent.filter(|&p| p < bones.len() && p != last) {
                Some(parent) if done[parent].is_none() && chain.len() <= bones.len() => chain.push(parent),
                _ => break,
            }
        }
        for &bone in chain.iter().rev() {
            if done[bone].is_some() {
                continue;
            }
            let parent = bones[bone].parent.filter(|&p| p < bones.len() && p != bone).and_then(|p| done[p]);
            done[bone] = Some(match parent {
                Some(parent) => parent.then(&locals[bone]),
                None => locals[bone],
            });
        }
    }
    done.into_iter().map(|w| w.unwrap_or(Affine::IDENTITY)).collect()
}

/// The matrix of each bone in the rest pose of the file.
fn rest_locals(bones: &[Bone]) -> Vec<Affine> {
    // A bone does not take the scale of its parent (see `posed`): the rest pose follows the same rule.
    bones.iter().map(|b| Affine::with_parent_scale(b.translation, rotation_zyx(b.rotation), b.scale, b.parent.and_then(|p| bones.get(p)).map_or([1.0; 3], |parent| parent.scale))).collect()
}

/// The model in a pose of a motion pack. The bones are matched by their names, and the first bone with the first joint.
pub fn posed(model: &Model, pack: &MotionPack, pose: &Pose) -> Model {
    let bones = &model.bones;
    let by_name: HashMap<&str, usize> = pack.joints.iter().enumerate().filter(|(_, j)| !j.name.is_empty()).map(|(i, j)| (j.name.as_str(), i)).collect();
    // The first joint is the parent of the first named joint (the bone "Origin"), and has no name.
    let origin = pack.joints.get(1).and_then(|j| bones.iter().find(|b| b.name == j.name)).and_then(|b| b.parent);
    let joint_of: Vec<Option<usize>> = bones.iter().enumerate().map(|(i, b)| if Some(i) == origin { Some(0) } else { by_name.get(b.name.as_str()).copied() }).collect();
    let rest = rest_locals(bones);
    let locals: Vec<Affine> = bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            let Some(joint) = joint_of[i].filter(|&j| j < pack.joints.len() && j < pose.joints.len()) else { return rest[i] };
            let (rest_joint, state) = (&pack.joints[joint], &pose.joints[joint]);
            // Some joints that the game moves with a simulation (hair, vines) have values that are not meant to be used: a
            // translation far bigger than the joint itself keeps the translation of the rest pose.
            let reach = rest_joint.translation.iter().fold(1.0f32, |m, c| m.max(c.abs())) * 100.0;
            let translation = state.translation.filter(|t| t.iter().all(|c| c.is_finite() && c.abs() <= reach)).unwrap_or(rest_joint.translation);
            let rotation = match state.rotation {
                Some(angles) => rotation_zyx(angles),
                None => {
                    // The rest rotation is in the model: from the parent it is the parent turned back, then this one.
                    let own = rotation_quaternion(rest_joint.rotation);
                    match rest_joint.parent.and_then(|p| pack.joints.get(p)) {
                        Some(parent) => multiply(transpose(rotation_quaternion(parent.rotation)), own),
                        None => own,
                    }
                }
            };
            // The scale of the parent is not inherited by the bone.
            let parent_scale = bone.parent.map_or([1.0; 3], |p| bone.parent.and_then(|_| joint_of[p]).and_then(|j| pose.joints.get(j)).and_then(|s| s.scale).unwrap_or(bones[p].scale));
            Affine::with_parent_scale(translation, rotation, state.scale.unwrap_or(bone.scale), parent_scale)
        })
        .collect();
    let (rest_worlds, now) = (worlds(bones, &rest), worlds(bones, &locals));
    // A bone that is flat in the rest pose (a scale of 0) cannot move vertices: they stay where they are.
    let skin: Vec<Affine> = now.iter().zip(&rest_worlds).map(|(now, rest)| rest.inverse().map_or(Affine::IDENTITY, |inverse| now.then(&inverse))).collect();
    let mut out = model.clone();
    for mesh in &mut out.meshes {
        for vertex in &mut mesh.vertices {
            let mut position = [0.0f32; 3];
            let mut normal = [0.0f32; 3];
            let mut total = 0.0f32;
            for k in 0..4 {
                let weight = vertex.weights[k];
                let Some(matrix) = (weight > 0.0).then(|| skin.get(vertex.bones[k] as usize)).flatten() else { continue };
                let (p, n) = (matrix.point(vertex.position), matrix.direction(vertex.normal));
                for axis in 0..3 {
                    position[axis] += weight * p[axis];
                    normal[axis] += weight * n[axis];
                }
                total += weight;
            }
            if total > 0.0 {
                vertex.position = position.map(|v| v / total);
                let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt().max(1e-12);
                vertex.normal = normal.map(|v| v / length);
            }
        }
    }
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_matrix_and_its_inverse_give_the_identity() {
        let a = Affine::new([1.0, 2.0, 3.0], rotation_zyx([0.3, -0.5, 1.1]), [1.0, 2.0, 0.5]);
        let product = a.then(&a.inverse().unwrap());
        for row in 0..3 {
            for column in 0..4 {
                let expected = Affine::IDENTITY.0[row][column];
                assert!((product.0[row][column] - expected).abs() < 1e-4, "{product:?}");
            }
        }
    }

    #[test]
    fn a_quaternion_of_a_quarter_turn_around_z_turns_x_into_y() {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let matrix = Affine::new([0.0; 3], rotation_quaternion([0.0, 0.0, half, half]), [1.0; 3]);
        let turned = matrix.point([1.0, 0.0, 0.0]);
        assert!((turned[0]).abs() < 1e-5 && (turned[1] - 1.0).abs() < 1e-5);
        // The angles z y x give the same rotation.
        let same = Affine::new([0.0; 3], rotation_zyx([0.0, 0.0, std::f32::consts::FRAC_PI_2]), [1.0; 3]).point([1.0, 0.0, 0.0]);
        assert!((same[1] - 1.0).abs() < 1e-5);
    }

    /// With the dump of the game: the rest pose of the motions is the rest pose of the model (so a model with no
    /// bone named by the pose does not move).
    #[test]
    fn the_rest_pose_of_the_motions_is_the_one_of_the_model() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let archive = crate::Garc::open(&dump.archive(8)).unwrap();
        let part = |file: usize| -> Vec<u8> {
            let bytes = archive.first_file(file).unwrap();
            let bytes = if crate::lz::is_compressed(bytes) { crate::lz::decompress(bytes).unwrap() } else { bytes.to_vec() };
            crate::Bundle::parse(bytes).unwrap().resource(0).unwrap().to_vec()
        };
        for first in [3usize, 59, 723, 4243] {
            let model = crate::model::parse_model(&part(first)).unwrap();
            let pack = MotionPack::parse(&part(first + 5)).unwrap();
            let size = model.meshes.iter().flat_map(|m| &m.vertices).fold(0.0f32, |a, v| a.max(v.position.iter().fold(0.0f32, |b, c| b.max(c.abs()))));
            let drift = |moved: &Model| -> f32 {
                model.meshes.iter().zip(&moved.meshes).flat_map(|(a, b)| a.vertices.iter().zip(&b.vertices)).fold(0.0f32, |worst, (a, b)| worst.max((0..3).map(|k| (a.position[k] - b.position[k]).abs()).fold(0.0, f32::max)))
            };
            let rest = Pose { joints: vec![Default::default(); pack.joints.len()], channels: Vec::new() };
            let still = drift(&posed(&model, &pack, &rest));
            let first_motion = (0..pack.len()).find(|&i| pack.has(i)).unwrap();
            let motion = pack.motion(first_motion).unwrap();
            let moved = drift(&posed(&model, &pack, &motion.pose(pack.joints.len(), 0.0)));
            let middle = drift(&posed(&model, &pack, &motion.pose(pack.joints.len(), motion.frames as f32 / 2.0)));
            eprintln!("file {first}: size {size:.1}, rest drift {still:.3}, motion {first_motion} frame 0 drift {moved:.2}, middle {middle:.2}");
            assert!(still < size * 0.02, "the rest pose of file {first} is not the rest pose of the model");
        }
    }

    #[test]
    fn a_vertex_follows_its_bone() {
        use crate::model::{Mesh, Vertex};
        use crate::motion::{Joint, JointPose};
        let bone = |name: &str, parent: Option<usize>| Bone { name: name.into(), parent, scale: [1.0; 3], rotation: [0.0; 3], translation: [0.0; 3] };
        let model = Model {
            bones: vec![bone("root", None), bone("arm", Some(0))],
            materials: Vec::new(),
            meshes: vec![Mesh { material: 0, vertices: vec![Vertex { position: [1.0, 0.0, 0.0], normal: [1.0, 0.0, 0.0], weights: [1.0, 0.0, 0.0, 0.0], bones: [1, 0, 0, 0], ..Vertex::default() }], triangles: Vec::new() }],
        };
        let joint = |name: &str, parent: Option<usize>| Joint { name: name.into(), parent, translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0] };
        let pack = MotionPack::parse_for_test(vec![joint("", None), joint("arm", Some(0))]);
        let pose = Pose { joints: vec![JointPose::default(), JointPose { rotation: Some([0.0, 0.0, std::f32::consts::FRAC_PI_2]), ..JointPose::default() }], channels: Vec::new() };
        let moved = posed(&model, &pack, &pose);
        let p = moved.meshes[0].vertices[0].position;
        assert!(p[0].abs() < 1e-5 && (p[1] - 1.0).abs() < 1e-5, "{p:?}");
    }
}
