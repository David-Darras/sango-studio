//! The skeleton of the model that the game gets: the joints that some vertex is weighted to and their ancestors, from the `Origin` joint
//! down, with the names that the game looks up changed.
//!
//! The arithmetic is done in 32 bits, as the Python tool does it (the numbers of the file come from 32-bit matrices), so the files
//! are the same bit for bit.

use crate::gltf::Source;
use std::collections::{BTreeSet, HashMap};

/// A 4 x 4 matrix, row by row.
pub type Matrix = [[f32; 4]; 4];

/// A joint as the writer needs it: the name, the name of the parent (empty for the first one), and the local scale, rotation (Euler
/// angles, Z then Y then X) and translation.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: String,
    pub parent: String,
    pub scale: [f32; 3],
    pub rot: [f64; 3],
    pub trans: [f32; 3],
}

/// The result: the joints (the first one is the model itself), and the place in `joints` of each joint of the glTF skin that was kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    pub index: HashMap<usize, usize>,
}

/// The names that the game looks up (lower case in the glTF file, the name of the game in the model).
const RENAME: [(&str, &str); 6] = [("origin", "Origin"), ("waist", "Waist"), ("head", "Head"), ("lower_jaw", "Jaw"), ("hips", "Hips"), ("spine", "Spine1")];

fn identity() -> Matrix {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            let mut sum = 0.0f32;
            for k in 0..4 {
                sum += a[i][k] * b[k][j];
            }
            out[i][j] = sum;
        }
    }
    out
}

/// The inverse of a matrix (LU decomposition with partial pivoting, in 32 bits).
pub fn invert(m: &Matrix) -> Option<Matrix> {
    let mut a = *m;
    let mut inverse = identity();
    for column in 0..4 {
        let pivot = (column..4).max_by(|&x, &y| a[x][column].abs().partial_cmp(&a[y][column].abs()).unwrap_or(std::cmp::Ordering::Equal))?;
        if a[pivot][column] == 0.0 {
            return None;
        }
        a.swap(column, pivot);
        inverse.swap(column, pivot);
        let divisor = a[column][column];
        for j in 0..4 {
            a[column][j] /= divisor;
            inverse[column][j] /= divisor;
        }
        for row in 0..4 {
            if row != column {
                let factor = a[row][column];
                if factor != 0.0 {
                    for j in 0..4 {
                        a[row][j] -= factor * a[column][j];
                        inverse[row][j] -= factor * inverse[column][j];
                    }
                }
            }
        }
    }
    Some(inverse)
}

/// The angles (x, y, z) of a rotation matrix, with m = Rz * Ry * Rx.
pub fn euler_zyx(m: &[[f64; 3]; 3]) -> [f64; 3] {
    let sy = (-m[2][0]).clamp(-1.0, 1.0);
    let ry = sy.asin();
    if sy.abs() < 0.99999 {
        [m[2][1].atan2(m[2][2]), ry, m[1][0].atan2(m[0][0])]
    } else {
        [(-m[1][2]).atan2(m[1][1]), ry, 0.0]
    }
}

/// The scale, the rotation (Euler angles) and the translation of a local matrix.
fn decompose(m: &Matrix) -> ([f32; 3], [f64; 3], [f32; 3]) {
    let mut scale = [0.0f32; 3];
    for c in 0..3 {
        scale[c] = (m[0][c] * m[0][c] + m[1][c] * m[1][c] + m[2][c] * m[2][c]).sqrt();
        if scale[c] == 0.0 {
            scale[c] = 1.0;
        }
    }
    let mut rot = [[0.0f32; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            rot[r][c] = m[r][c] / scale[c];
        }
    }
    // The determinant: a mirror flips the first axis.
    let det = rot[0][0] * (rot[1][1] * rot[2][2] - rot[1][2] * rot[2][1]) - rot[0][1] * (rot[1][0] * rot[2][2] - rot[1][2] * rot[2][0]) + rot[0][2] * (rot[1][0] * rot[2][1] - rot[1][1] * rot[2][0]);
    if det < 0.0 {
        for row in rot.iter_mut() {
            row[0] = -row[0];
        }
        scale[0] = -scale[0];
    }
    let as_f64: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| rot[r][c] as f64));
    (scale, euler_zyx(&as_f64), [m[0][3], m[1][3], m[2][3]])
}

/// Keeps the joints that some vertex is weighted to (`used`: places in the skin) and their ancestors up to the origin.
pub fn build(source: &Source, used: &BTreeSet<usize>, scale: f32, model_name: &str) -> Result<Skeleton, String> {
    if source.joints.is_empty() {
        return Err("The file has no skin.".into());
    }
    let nodes = &source.nodes;
    let joint_nodes = &source.joints;
    let node_joint: HashMap<usize, usize> = joint_nodes.iter().enumerate().map(|(k, &n)| (n, k)).collect();
    // The world matrix of each joint in the rest pose: the inverse of its inverse bind matrix, with the translation scaled.
    let mut world: Vec<Matrix> = Vec::new();
    for ibm in &source.inverse_bind {
        // The file has the matrices column by column.
        let m: Matrix = std::array::from_fn(|r| std::array::from_fn(|c| ibm[c * 4 + r]));
        let mut w = invert(&m).ok_or("An inverse bind matrix cannot be inverted.")?;
        for row in w.iter_mut().take(3) {
            row[3] *= scale;
        }
        world.push(w);
    }
    if world.len() != joint_nodes.len() {
        return Err("The skin has not one inverse bind matrix for each joint.".into());
    }
    let names: Vec<&str> = joint_nodes.iter().map(|&n| nodes[n].name.as_str()).collect();
    let origin = names.iter().position(|n| n.to_lowercase().starts_with("origin")).ok_or("The skeleton has no joint named Origin.")?;
    let joint_parent = |k: usize| -> Option<usize> {
        let mut current = nodes[joint_nodes[k]].parent;
        while let Some(node) = current {
            if let Some(&joint) = node_joint.get(&node) {
                return Some(joint);
            }
            current = nodes[node].parent;
        }
        None
    };
    let mut keep: BTreeSet<usize> = BTreeSet::from([origin]);
    for &k in used {
        let (mut current, mut chain) = (Some(k), Vec::new());
        while let Some(c) = current.filter(|c| !keep.contains(c)) {
            chain.push(c);
            current = joint_parent(c);
        }
        // A joint that is not under the origin: its weight goes to the origin.
        if current.is_some() {
            keep.extend(chain);
        }
    }
    let mut children: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for &k in &keep {
        if k != origin {
            children.entry(joint_parent(k)).or_default().push(k);
        }
    }
    let (mut order, mut stack) = (Vec::new(), vec![origin]);
    while let Some(k) = stack.pop() {
        order.push(k);
        let mut list = children.get(&Some(k)).cloned().unwrap_or_default();
        list.sort_unstable_by(|a, b| b.cmp(a));
        stack.extend(list);
    }
    let clean = |name: &str| -> String {
        let stem = name.to_lowercase();
        for (base, new) in RENAME {
            let numbered = stem.strip_prefix(&format!("{base}_")).is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
            if stem == base || numbered {
                return new.to_string();
            }
        }
        name.to_string()
    };
    let mut used_names = BTreeSet::new();
    let mut out_names: HashMap<usize, String> = HashMap::new();
    for &k in &order {
        let mut name = clean(names[k]);
        while used_names.contains(&name) {
            name.push('_');
        }
        used_names.insert(name.clone());
        out_names.insert(k, name);
    }
    let mut joints = vec![Joint { name: model_name.to_string(), parent: String::new(), scale: [1.0; 3], rot: [0.0; 3], trans: [0.0; 3] }];
    let mut index = HashMap::new();
    for &k in &order {
        let (local, parent) = if k == origin {
            (world[k], model_name.to_string())
        } else {
            let p = joint_parent(k).ok_or("A joint has no parent.")?;
            (multiply(&invert(&world[p]).ok_or("A world matrix cannot be inverted.")?, &world[k]), out_names[&p].clone())
        };
        let (s, r, t) = decompose(&local);
        joints.push(Joint { name: out_names[&k].clone(), parent, scale: s, rot: r, trans: t });
        index.insert(k, joints.len() - 1);
    }
    Ok(Skeleton { joints, index })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_matrix_is_inverted() {
        let mut m = identity();
        m[0][3] = 2.0;
        m[1][1] = 4.0;
        let inverse = invert(&m).unwrap();
        assert_eq!((inverse[0][3], inverse[1][1]), (-2.0, 0.25));
        assert!(invert(&[[0.0; 4]; 4]).is_none());
        let again = multiply(&m, &inverse);
        assert_eq!(again, identity());
    }

    #[test]
    fn the_angles_of_a_rotation_are_found() {
        // A quarter of a turn around z.
        let m = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let angles = euler_zyx(&m);
        assert!((angles[2] - std::f64::consts::FRAC_PI_2).abs() < 1e-12 && angles[0].abs() < 1e-12 && angles[1].abs() < 1e-12);
    }
}
