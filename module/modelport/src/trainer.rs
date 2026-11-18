//! The model of a trainer: a glTF file becomes the file of the battle model (a BCH with its textures in the same file).
//!
//! The materials, the layout of the meshes and the metadata come from the original model of the trainer, so the new model draws with the
//! same shaders as the old one. The motions are not made: the files of the motions of the model stay as they are, and they move the joints
//! that have the same names as in the original model.

use crate::convert;
use crate::files::box_of;
use crate::gltf::Source;
use crate::model;
use crate::skeleton::Joint;
use crate::template::Template;
use crate::texture;

/// A model taller than this (in the units of the file) is in the units of the game, not in metres.
const MAX_HEIGHT_IN_METRES: f32 = 30.0;

/// The file of the model of a trainer, not compressed, from a glTF file and the original file of the model.
/// `joints` is the skeleton of the original model (the first joint has the name of the model): the new model keeps it.
pub fn trainer_model(source: &Source, original: &[u8], joints: &[Joint], scale: f64) -> Result<Vec<u8>, String> {
    let model_name = joints.first().map(|j| j.name.clone()).ok_or("The original model has no joints.")?;
    let model_name = model_name.as_str();
    let (_, height) = box_of(source);
    let scale = if height as f32 / 100.0 > MAX_HEIGHT_IN_METRES { scale * 0.01 } else { scale };
    let prepared = convert::prepare(source, model_name, scale, true, true, Some(joints))?;
    let mut textures = prepared.textures;
    texture::fit_budget(&mut textures);
    let template = Template::open(original)?;
    model::write_model_file_with(&prepared.spec, std::slice::from_ref(&template), model_name, &prepared.texture_names, &textures)
}
