//! The motions (GMP): a pack that holds the pose of rest of the skeleton and a motion for each slot of the template pack.
//!
//! A model without an animation stays in its rest pose (its T-pose): the first entry of the pack is the pose of rest, and every other slot of
//! the template is a motion of one frame that keeps the pose of rest.

use crate::skeleton::Joint;
use std::collections::HashMap;

/// The curve kinds that the game knows: a constant, the value of the pose of rest, data, a spline.
const FC_STATIC_0: u32 = 0;
const FC_STATIC_BASEPOSE: u32 = 1;

/// The joints under the origin in depth-first order: the name and the place of the parent.
pub fn skeleton_order(joints: &[Joint]) -> Vec<(String, Option<usize>)> {
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for joint in joints {
        children.entry(joint.parent.as_str()).or_default().push(joint.name.as_str());
    }
    let root = if joints.iter().any(|j| j.name == "Origin") { "Origin" } else { joints[0].name.as_str() };
    let mut order: Vec<(String, Option<usize>)> = Vec::new();
    fn walk<'a>(name: &'a str, parent: Option<usize>, children: &HashMap<&'a str, Vec<&'a str>>, order: &mut Vec<(String, Option<usize>)>) {
        let index = order.len();
        order.push((name.to_string(), parent));
        for child in children.get(name).into_iter().flatten() {
            walk(child, Some(index), children, order);
        }
    }
    walk(root, None, &children, &mut order);
    order
}

type Matrix34 = [[f64; 4]; 3];

pub(crate) fn rot_zyx(rx: f64, ry: f64, rz: f64) -> [[f64; 3]; 3] {
    let (cx, sx, cy, sy, cz, sz) = (rx.cos(), rx.sin(), ry.cos(), ry.sin(), rz.cos(), rz.sin());
    let mul = |a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]| -> [[f64; 3]; 3] {
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = (0..3).fold(0.0, |sum, k| sum + a[i][k] * b[k][j]);
            }
        }
        out
    };
    let rxm = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let rym = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rzm = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mul(&rzm, &mul(&rym, &rxm))
}

fn local_matrix(joint: &Joint) -> Matrix34 {
    let r = rot_zyx(joint.rot[0], joint.rot[1], joint.rot[2]);
    std::array::from_fn(|i| [r[i][0] * joint.scale[0] as f64, r[i][1] * joint.scale[1] as f64, r[i][2] * joint.scale[2] as f64, joint.trans[i] as f64])
}

fn mul34(a: &Matrix34, b: &Matrix34) -> Matrix34 {
    let mut out = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..4 {
            out[i][j] = (0..3).fold(0.0, |sum, k| sum + a[i][k] * b[k][j]) + if j == 3 { a[i][3] } else { 0.0 };
        }
    }
    out
}

fn quat_from_matrix(m: &Matrix34) -> [f64; 4] {
    let trace = m[0][0] + m[1][1] + m[2][2];
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s, 0.25 * s]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s, (m[2][1] - m[1][2]) / s]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [(m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s, (m[0][2] - m[2][0]) / s]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [(m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s, (m[1][0] - m[0][1]) / s]
    }
}

/// The base pose: the skeleton under `Origin` (the number of joints, the parent and the size of the subtree of each joint, the names) then the
/// local translation and the world quaternion of each joint.
pub fn base_pose(joints: &[Joint], order: &[(String, Option<usize>)]) -> Vec<u8> {
    let by_name: HashMap<&str, &Joint> = joints.iter().map(|j| (j.name.as_str(), j)).collect();
    let mut world: HashMap<&str, Matrix34> = HashMap::new();
    for joint in joints {
        let local = local_matrix(joint);
        let matrix = match world.get(joint.parent.as_str()) {
            Some(parent) => mul34(parent, &local),
            None => local,
        };
        world.insert(joint.name.as_str(), matrix);
    }
    let n = order.len();
    let mut out = vec![n as u8, 0];
    for i in 1..n {
        // The size of the subtree, with the joint itself.
        let (mut stack, mut size) = (vec![i], 0u8);
        while let Some(current) = stack.pop() {
            size += 1;
            stack.extend((0..n).filter(|&k| order[k].1 == Some(current)));
        }
        out.extend_from_slice(&[order[i].1.unwrap_or(0) as u8, 0, size]);
    }
    for (name, _) in &order[1..] {
        out.extend_from_slice(name.as_bytes());
        out.push(0);
    }
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    for (name, _) in order {
        let joint = by_name[name.as_str()];
        let q = quat_from_matrix(&world[name.as_str()]);
        for v in joint.trans.iter().map(|&v| v as f64).chain(q) {
            out.extend_from_slice(&(v as f32).to_le_bytes());
        }
    }
    out.extend_from_slice(&[0; 4]);
    out
}

/// A motion of one frame that keeps the pose of rest of every joint.
pub fn still_motion(order: &[(String, Option<usize>)]) -> Vec<u8> {
    // The two curves of the locator, then three curves (translation, rotation, scale) for each joint: all at the base pose.
    let mut kinds = vec![FC_STATIC_0, FC_STATIC_0];
    kinds.extend(std::iter::repeat(FC_STATIC_BASEPOSE).take(3 * order.len()));
    let frames: u16 = 1;
    let mut out = Vec::new();
    out.extend_from_slice(&(kinds.len() as u16).to_le_bytes());
    out.extend_from_slice(&frames.to_le_bytes());
    for group in kinds.chunks(8) {
        let mut v = 0u32;
        for (i, kind) in group.iter().enumerate() {
            v |= kind << (3 * i);
        }
        out.extend_from_slice(&v.to_le_bytes()[..3]);
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

/// A motion pack with the slots of the template pack, every slot of the template that is used holding the pose of rest.
pub fn rest_pose_pack(template: &[u8], joints: &[Joint]) -> Result<Vec<u8>, String> {
    let order = skeleton_order(joints);
    let u32_at = |at: usize| -> Result<u32, String> { template.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "The motion pack of the template is cut.".to_string()) };
    let count = u32_at(0)? as usize;
    let offsets: Vec<u32> = (0..=count).map(|i| u32_at(4 + 4 * i)).collect::<Result<_, _>>()?;
    let still = still_motion(&order);
    let mut entries: Vec<Vec<u8>> = vec![base_pose(joints, &order)];
    for k in 1..count {
        entries.push(if offsets[k] == 0 { Vec::new() } else { still.clone() });
    }
    let header = 4 + 4 * (count + 1);
    let (mut body, mut starts): (Vec<u8>, Vec<u32>) = (Vec::new(), Vec::new());
    for entry in &entries {
        if entry.is_empty() {
            starts.push(0);
            continue;
        }
        while (header + body.len()) % 4 != 0 {
            body.push(0);
        }
        starts.push((header + body.len()) as u32);
        body.extend_from_slice(entry);
    }
    starts.push((header + body.len()) as u32);
    let mut out = (count as u32).to_le_bytes().to_vec();
    for start in starts {
        out.extend_from_slice(&start.to_le_bytes());
    }
    out.extend_from_slice(&body);
    Ok(out)
}
