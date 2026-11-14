//! The importer of 3D models: turns a glTF file into the files of a Pokémon model of the game.
//!
//! The work is split in steps, each one checked against the files of the game:
//!
//! - [`pc`]: the packs of the model archive,
//! - [`patricia`]: the dictionaries of the H3D files,
//! - [`bch`]: the writer of H3D files,
//! - [`texture`]: the textures and the file of the textures,
//! - [`gltf`]: the reader of glTF files,
//! - [`skeleton`]: the joints that the game gets,
//! - [`geometry`]: the weights, the tangents and the draws,
//! - [`pica`]: the lists of GPU commands.
//!
//! The state of the work and the plan are in `docs/model-import.md`.

pub mod animate;
pub mod bch;
pub mod convert;
pub mod files;
pub mod geometry;
pub mod gltf;
pub mod model;
pub mod motion;
pub mod patricia;
pub mod pc;
pub mod pica;
pub mod skeleton;
pub mod template;
pub mod texture;

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::BTreeSet;

    fn golden() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("model")
    }

    /// The small model of the tests, read like the importer reads it.
    pub fn body() -> gltf::Source {
        gltf::read(&golden().join("body.gltf")).unwrap()
    }

    pub fn used_joints(source: &gltf::Source) -> BTreeSet<usize> {
        let mut used = BTreeSet::new();
        for primitive in &source.primitives {
            for (joints, weights) in primitive.joints.iter().zip(&primitive.weights) {
                for (&joint, &weight) in joints.iter().zip(weights) {
                    if weight > 0.0 {
                        used.insert(joint as usize);
                    }
                }
            }
        }
        used
    }

    /// The skeleton and the draws are the ones that the Python tool makes from the same file: the same names, the same numbers, and the
    /// same bytes in the buffers.
    #[test]
    fn the_skeleton_and_the_draws_are_the_ones_of_the_python_tool() {
        let mid: serde_json::Value = serde_json::from_slice(&std::fs::read(golden().join("mid.json")).unwrap()).unwrap();
        let source = body();
        let skeleton = skeleton::build(&source, &used_joints(&source), 100.0, "pm0025_01").unwrap();
        let expected = mid["joints"].as_array().unwrap();
        assert_eq!(skeleton.joints.len(), expected.len());
        for (joint, want) in skeleton.joints.iter().zip(expected) {
            assert_eq!((joint.name.as_str(), joint.parent.as_str()), (want[0].as_str().unwrap(), want[1].as_str().unwrap()));
            let numbers = |value: &serde_json::Value| -> Vec<f64> { value.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect() };
            // The numbers of the JSON file are read to 32 bits: the scale and the translation are 32-bit numbers.
            assert_eq!(joint.scale.to_vec(), numbers(&want[2]).iter().map(|&v| v as f32).collect::<Vec<_>>(), "scale of {}", joint.name);
            assert_eq!(joint.rot.to_vec(), numbers(&want[3]), "rotation of {}", joint.name);
            assert_eq!(joint.trans.to_vec(), numbers(&want[4]).iter().map(|&v| v as f32).collect::<Vec<_>>(), "translation of {}", joint.name);
            // The sign of a zero counts: the file has the bits.
            assert_eq!(joint.rot.iter().map(|v| v.is_sign_negative()).collect::<Vec<_>>(), numbers(&want[3]).iter().map(|v| v.is_sign_negative()).collect::<Vec<_>>(), "zero signs of {}", joint.name);
        }
        let mut faces = Vec::new();
        for primitive in &source.primitives {
            let prim = geometry::read_primitive(primitive, &skeleton.index, 100.0);
            faces.extend(geometry::build_faces(&prim, "Body", false));
        }
        let want_faces = mid["faces"].as_array().unwrap();
        assert_eq!(faces.len(), want_faces.len());
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        for (face, want) in faces.iter().zip(want_faces) {
            assert_eq!(face.joints, want["joints"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect::<Vec<_>>());
            assert_eq!((face.vertices as u64, face.indices as u64), (want["vertices"].as_u64().unwrap(), want["indices"].as_u64().unwrap()));
            assert_eq!(hex(&face.vb), want["vb"].as_str().unwrap(), "the vertex buffer");
            assert_eq!(hex(&face.ib), want["ib"].as_str().unwrap(), "the index buffer");
            assert_eq!(face.cmds[2].iter().map(|&w| w as u64).collect::<Vec<_>>(), want["draw"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect::<Vec<_>>());
        }
    }
}
