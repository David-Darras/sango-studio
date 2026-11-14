//! The geometry that the game draws: the weights of each vertex on the joints of the skeleton, the tangents, the groups of triangles that
//! touch at most 20 joints (a draw of the game skins 20 joints), and the vertex and index buffers of each draw.
//!
//! A vertex has 56 bytes: the position, the normal, the texture coordinates and the tangent (as 32-bit floats), the colour (4 bytes),
//! the 4 joints and the 4 weights (a byte each). The lists of GPU commands of a draw are the ones of a mesh of this layout (see
//! [`ENABLE`], [`DISABLE`] and [`DRAW`]).

use crate::gltf::Primitive;
use crate::pica::find_word;
use std::collections::{BTreeMap, HashMap};

/// The most joints that a draw can skin.
pub const BONE_LIMIT: usize = 20;
/// The size of a vertex in bytes.
pub const VERTEX_SIZE: usize = 56;
/// The number of cells along the longest side of the body for the shadow.
pub const SHADOW_DIVISIONS: f64 = 16.0;

/// The commands of a mesh with a vertex of 56 bytes and four weights (taken from a Pokémon of the generation VII): the commands that
/// set the attributes up, the ones that turn them off, and the ones that draw.
pub const ENABLE: [u32; 32] = [
    0xA0000008, 0x000B02B9, 0x00000008, 0x00010242, 0x68732410, 0x000F02BB, 0x00000005, 0x000F02BC, 0x03000000, 0x805F0200, 0x0DDDB7BB, 0x81800000, 0x99999999, 0x06543210, 0x70380000, 0x00000000, 0x00000007, 0x803F0232, 0x3F000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000008, 0x803F0232, 0x3F000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000001, 0x000F023D,
];
pub const DISABLE: [u32; 40] = [
    0x00000000, 0x802F0203, 0x00000000, 0x00000000, 0x00000000, 0x000F0208, 0x00000000, 0x000F020B, 0x00000000, 0x000F020E, 0x00000000, 0x000F0211, 0x00000000, 0x000F0214, 0x00000005, 0x803F0232, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x000F0217, 0x00000006, 0x803F0232, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x000F021A, 0x00000000, 0x000F021D, 0x00000000, 0x000F0220, 0x00000000, 0x000F0223, 0x00000000, 0x000F0226, 0x00000001, 0x000F023D,
];
pub const DRAW: [u32; 20] = [
    0x00000001, 0x000F025F, 0x81999999, 0x000F0227, 0x00001158, 0x000F0228, 0x00000000, 0x00010245, 0x00000001, 0x000F022F, 0x00000001, 0x00010245, 0x00000001, 0x000F0231, 0x00000000, 0x0008025E, 0x00000000, 0x0008025E, 0x00000001, 0x000F023D,
];

/// The triangles of a mesh with the weights of each vertex on the joints of the skeleton (the places in the list of the joints).
#[derive(Clone, Debug, Default)]
pub struct Prim {
    pub p: Vec<[f64; 3]>,
    pub n: Vec<[f64; 3]>,
    pub uv: Vec<[f64; 2]>,
    /// For each vertex, up to 4 joints and their weights, the first one has the most weight.
    pub jw: Vec<Vec<(usize, f64)>>,
    pub tris: Vec<[usize; 3]>,
}

/// One draw: a group of triangles with its buffers and its commands.
#[derive(Clone, Debug)]
pub struct Face {
    pub material: String,
    pub joints: Vec<usize>,
    pub vertices: usize,
    pub indices: usize,
    pub vb: Vec<u8>,
    pub ib: Vec<u8>,
    /// The commands that set up the attributes, turn them off and draw.
    pub cmds: [Vec<u32>; 3],
}

/// The arrays of a primitive of the file, with the weights on the joints of the writer. `index` maps the joints of the skin to the
/// places in the list of the joints of the writer; a joint that is not there goes to the joint 1.
pub fn read_primitive(primitive: &Primitive, index: &HashMap<usize, usize>, scale: f64) -> Prim {
    let mut jw = Vec::with_capacity(primitive.positions.len());
    for v in 0..primitive.positions.len() {
        let mut found: Vec<(usize, f64)> = Vec::new();
        for (&joint, &weight) in primitive.joints[v].iter().zip(&primitive.weights[v]) {
            if weight as f64 > 0.0 {
                let bone = index.get(&(joint as usize)).copied().unwrap_or(1);
                match found.iter_mut().find(|(b, _)| *b == bone) {
                    Some(entry) => entry.1 += weight as f64,
                    None => found.push((bone, weight as f64)),
                }
            }
        }
        // The heaviest first (a stable sort: the joints with the same weight stay in the order of the file).
        found.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        found.truncate(4);
        if found.is_empty() {
            found.push((1, 1.0));
        }
        let total: f64 = found.iter().map(|(_, w)| w).sum();
        jw.push(found.into_iter().map(|(b, w)| (b, w / total)).collect());
    }
    Prim {
        p: primitive.positions.iter().map(|v| [v[0] as f64 * scale, v[1] as f64 * scale, v[2] as f64 * scale]).collect(),
        n: primitive.normals.iter().map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect(),
        uv: primitive.uvs.iter().map(|v| [v[0] as f64, v[1] as f64]).collect(),
        jw,
        tris: primitive.indices.chunks_exact(3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect(),
    }
}

/// The tangent of each vertex (the direction of the first texture coordinate), as 32-bit floats.
pub fn tangents(prim: &Prim) -> Vec<[f32; 3]> {
    let mut t = vec![[0.0f64; 3]; prim.p.len()];
    let tri_t: Vec<[f64; 3]> = prim
        .tris
        .iter()
        .map(|&[a, b, c]| {
            let e1 = [prim.p[b][0] - prim.p[a][0], prim.p[b][1] - prim.p[a][1], prim.p[b][2] - prim.p[a][2]];
            let e2 = [prim.p[c][0] - prim.p[a][0], prim.p[c][1] - prim.p[a][1], prim.p[c][2] - prim.p[a][2]];
            let d1 = [prim.uv[b][0] - prim.uv[a][0], prim.uv[b][1] - prim.uv[a][1]];
            let d2 = [prim.uv[c][0] - prim.uv[a][0], prim.uv[c][1] - prim.uv[a][1]];
            let mut det = d1[0] * d2[1] - d2[0] * d1[1];
            if det.abs() < 1e-12 {
                det = 1e-12;
            }
            [(e1[0] * d2[1] - e2[0] * d1[1]) / det, (e1[1] * d2[1] - e2[1] * d1[1]) / det, (e1[2] * d2[1] - e2[2] * d1[1]) / det]
        })
        .collect();
    for k in 0..3 {
        for (tri, value) in prim.tris.iter().zip(&tri_t) {
            for c in 0..3 {
                t[tri[k]][c] += value[c];
            }
        }
    }
    t.iter()
        .zip(&prim.n)
        .map(|(t, n)| {
            let dot = t[0] * n[0] + t[1] * n[1] + t[2] * n[2];
            let mut v = [t[0] - n[0] * dot, t[1] - n[1] * dot, t[2] - n[2] * dot];
            let mut length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if length < 1e-9 {
                v = [1.0, 0.0, 0.0];
                length = 1.0;
            }
            [(v[0] / length) as f32, (v[1] / length) as f32, (v[2] / length) as f32]
        })
        .collect()
}

/// Which way the triangles go: 1 when most of them turn the way of the normals, -1 when most turn the other way, 0 when half.
pub fn winding_sign(prim: &Prim) -> f64 {
    let mut positive = 0usize;
    for &[a, b, c] in &prim.tris {
        let u = [prim.p[b][0] - prim.p[a][0], prim.p[b][1] - prim.p[a][1], prim.p[b][2] - prim.p[a][2]];
        let w = [prim.p[c][0] - prim.p[a][0], prim.p[c][1] - prim.p[a][1], prim.p[c][2] - prim.p[a][2]];
        let normal = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
        let vn = [prim.n[a][0] + prim.n[b][0] + prim.n[c][0], prim.n[a][1] + prim.n[b][1] + prim.n[c][1], prim.n[a][2] + prim.n[b][2] + prim.n[c][2]];
        if normal[0] * vn[0] + normal[1] * vn[1] + normal[2] * vn[2] > 0.0 {
            positive += 1;
        }
    }
    let difference = positive as f64 - prim.tris.len() as f64 / 2.0;
    if difference > 0.0 {
        1.0
    } else if difference < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The triangles grouped so that each group touches at most `BONE_LIMIT` joints, with the joints of each group (sorted).
pub fn submeshes(prim: &Prim) -> Vec<(Vec<usize>, Vec<usize>)> {
    let mut order: Vec<usize> = (0..prim.tris.len()).collect();
    order.sort_by(|&x, &y| {
        let key = |t: usize| (prim.jw[prim.tris[t][0]][0].0, prim.p[prim.tris[t][0]][1]);
        let (a, b) = (key(x), key(y));
        a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut groups = Vec::new();
    let (mut current, mut bones): (Vec<usize>, std::collections::BTreeSet<usize>) = (Vec::new(), Default::default());
    for t in order {
        let need: std::collections::BTreeSet<usize> = prim.tris[t].iter().flat_map(|&v| prim.jw[v].iter().map(|(b, _)| *b)).collect();
        if bones.union(&need).count() > BONE_LIMIT && !current.is_empty() {
            groups.push((std::mem::take(&mut current), bones.iter().copied().collect()));
            bones.clear();
        }
        current.push(t);
        bones.extend(need);
    }
    if !current.is_empty() {
        groups.push((current, bones.iter().copied().collect()));
    }
    groups
}

/// The draws of a primitive. `flip` reverses the triangles.
pub fn build_faces(prim: &Prim, material: &str, flip: bool) -> Vec<Face> {
    let tangent = tangents(prim);
    let mut faces = Vec::new();
    for (tri_ids, bones) in submeshes(prim) {
        let local: HashMap<usize, usize> = bones.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        let (mut remap, mut verts, mut idx): (HashMap<usize, usize>, Vec<usize>, Vec<usize>) = (HashMap::new(), Vec::new(), Vec::new());
        for &t in &tri_ids {
            let mut tri = prim.tris[t];
            if flip {
                tri.reverse();
            }
            for v in tri {
                let place = *remap.entry(v).or_insert_with(|| {
                    verts.push(v);
                    verts.len() - 1
                });
                idx.push(place);
            }
        }
        let mut vb = Vec::with_capacity(verts.len() * VERTEX_SIZE);
        for &v in &verts {
            let (mut bi, mut bw) = ([255u8; 4], [0i32; 4]);
            for (k, &(b, w)) in prim.jw[v].iter().enumerate() {
                bi[k] = local[&b] as u8;
                // Python rounds half to even.
                bw[k] = (w * 255.0).round_ties_even() as i32;
            }
            bw[0] += 255 - bw.iter().sum::<i32>();
            for value in prim.p[v].iter().chain(&prim.n[v]).chain(&prim.uv[v]) {
                vb.extend_from_slice(&(*value as f32).to_le_bytes());
            }
            for value in tangent[v] {
                vb.extend_from_slice(&value.to_le_bytes());
            }
            vb.extend_from_slice(&[255, 255, 255, 255]);
            vb.extend_from_slice(&bi);
            vb.extend(bw.iter().map(|&w| w as u8));
        }
        let ib: Vec<u8> = idx.iter().flat_map(|&i| (i as u16).to_le_bytes()).collect();
        let mut draw = DRAW.to_vec();
        let at = find_word(&draw, 0x228).expect("the draw commands set the number of indices");
        draw[at] = idx.len() as u32;
        faces.push(Face { material: material.to_string(), joints: bones, vertices: verts.len(), indices: idx.len(), vb, ib, cmds: [ENABLE.to_vec(), DISABLE.to_vec(), draw] });
    }
    faces
}

/// The smallest and the biggest position of some primitives.
pub fn bounds(prims: &[&Prim]) -> ([f64; 3], [f64; 3]) {
    let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
    for prim in prims {
        for p in &prim.p {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    (low, high)
}

/// A coarse copy of a primitive for the shadow: the vertices that fall in the same cell of a grid become one.
pub fn cluster(prim: &Prim, cell: f64) -> Prim {
    let keys: Vec<[i64; 3]> = prim.p.iter().map(|p| [(p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64, (p[2] / cell).floor() as i64]).collect();
    let mut groups: BTreeMap<[i64; 3], usize> = BTreeMap::new();
    for key in &keys {
        groups.entry(*key).or_default();
    }
    for (place, value) in groups.values_mut().enumerate() {
        *value = place;
    }
    let inverse: Vec<usize> = keys.iter().map(|k| groups[k]).collect();
    let n = groups.len();
    let (mut count, mut p, mut normal, mut uv) = (vec![0.0f64; n], vec![[0.0f64; 3]; n], vec![[0.0f64; 3]; n], vec![[0.0f64; 2]; n]);
    for (v, &c) in inverse.iter().enumerate() {
        count[c] += 1.0;
        for k in 0..3 {
            p[c][k] += prim.p[v][k];
            normal[c][k] += prim.n[v][k];
        }
        for k in 0..2 {
            uv[c][k] += prim.uv[v][k];
        }
    }
    for c in 0..n {
        for k in 0..3 {
            p[c][k] /= count[c];
        }
        for k in 0..2 {
            uv[c][k] /= count[c];
        }
        let length = (normal[c][0] * normal[c][0] + normal[c][1] * normal[c][1] + normal[c][2] * normal[c][2]).sqrt();
        let length = if length == 0.0 { 1.0 } else { length };
        for k in 0..3 {
            normal[c][k] /= length;
        }
    }
    let mut jw: Vec<Option<Vec<(usize, f64)>>> = vec![None; n];
    for (v, &c) in inverse.iter().enumerate() {
        if jw[c].is_none() {
            jw[c] = Some(prim.jw[v].clone());
        }
    }
    let tris = prim.tris.iter().map(|t| [inverse[t[0]], inverse[t[1]], inverse[t[2]]]).filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect();
    Prim { p, n: normal, uv, jw: jw.into_iter().map(|j| j.unwrap_or_default()).collect(), tris }
}
