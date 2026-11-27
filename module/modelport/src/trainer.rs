//! The model of a trainer: a glTF file becomes the file of the battle model (a BCH with its textures in the same file).
//!
//! The materials, the layout of the meshes and the metadata come from the original model of the trainer, so the new model draws with the
//! same shaders as the old one. The motions are not made: the files of the motions of the model stay as they are, and they move the joints
//! that have the same names as in the original model.

use crate::convert;
use crate::files::metres_per_unit;
use crate::gltf::Source;
use crate::model;
use crate::skeleton::Joint;
use crate::template::Template;
use crate::texture;

/// The file of the model of a trainer, not compressed, from a glTF file and the original file of the model.
/// `joints` is the skeleton of the original model (the first joint has the name of the model): the new model keeps it.
pub fn trainer_model(source: &Source, original: &[u8], joints: &[Joint], scale: f64) -> Result<Vec<u8>, String> {
    trainer_model_within(source, original, joints, scale, texture::TEXTURE_BUDGET, false)
}

/// The same, with a limit for the weight of all the pictures, in bytes (a model that is bigger than the old one can freeze the game), and
/// the pictures written in 16 bits for a pixel when `compress` is true.
pub fn trainer_model_within(source: &Source, original: &[u8], joints: &[Joint], scale: f64, budget: usize, compress: bool) -> Result<Vec<u8>, String> {
    let model_name = joints.first().map(|j| j.name.clone()).ok_or("The original model has no joints.")?;
    let model_name = model_name.as_str();
    let scale = scale * metres_per_unit(source);
    let prepared = convert::prepare_with(source, model_name, scale, true, true, Some(joints), compress)?;
    let mut prepared = prepared;
    keep_picture_names(&mut prepared, original);
    let mut textures = std::mem::take(&mut prepared.textures);
    keep_other_pictures(&mut textures, original);
    texture::fit_budget_to(&mut textures, budget);
    let template = Template::open(original)?;
    model::write_model_file_with(&prepared.spec, std::slice::from_ref(&template), model_name, &prepared.texture_names, &textures)
}

/// The game finds some pictures of a model by their names, so the pictures keep the names that the original model gives them: for each
/// material that the original model has too, the picture of the material takes the name of the picture of the original material.
fn keep_picture_names(prepared: &mut convert::Prepared, original: &[u8]) {
    let Ok(old) = sango_core::model::parse_model(original) else { return };
    let mut renames: Vec<(String, String)> = Vec::new();
    for material in &prepared.spec.materials {
        let Some(key) = material.slots[0].as_ref() else { continue };
        let Some(current) = prepared.texture_names.get(key) else { continue };
        let Some(found) = old.materials.iter().find(|m| m.name == material.name && !m.texture.is_empty()) else { continue };
        if *current != found.texture && !renames.iter().any(|(from, to)| from == current || to == &found.texture) {
            renames.push((current.clone(), found.texture.clone()));
        }
    }
    // The units: the original model puts a picture that the game fills in unit 0 and the real picture in unit 1, and the mix of colours
    // reads the unit 1. The new model gets the same layout.
    for material in &mut prepared.spec.materials {
        let Some(found) = old.materials.iter().find(|m| m.name == material.name && !m.texture.is_empty()) else { continue };
        let Some(real) = found.units.iter().position(|u| u.texture == found.texture) else { continue };
        if real == 0 || material.slots[0].is_none() {
            continue;
        }
        let (own, own_uv, own_unit) = (material.slots[0].clone(), material.uv[0], material.layout.units.get(&0).cloned());
        let Some(own_unit) = own_unit else { continue };
        let mut slots: [Option<String>; 3] = [None, None, None];
        let mut uv: [Option<[f64; 5]>; 3] = [None, None, None];
        let mut units = std::collections::BTreeMap::new();
        for (index, unit) in found.units.iter().enumerate() {
            if unit.texture.is_empty() {
                continue;
            }
            let mut layout_unit = own_unit.clone();
            layout_unit.wrap = (unit.wrap[0], unit.wrap[1]);
            if index == real {
                slots[index] = own.clone();
                uv[index] = own_uv;
            } else {
                let key = format!("{}.tga", unit.texture);
                prepared.texture_names.insert(key.clone(), unit.texture.clone());
                slots[index] = Some(key);
                uv[index] = Some([f64::from(unit.scale[0]), f64::from(unit.scale[1]), 0.0, f64::from(unit.translation[0]), f64::from(unit.translation[1])]);
            }
            units.insert(index, layout_unit);
        }
        material.slots = slots;
        material.uv = uv;
        material.layout.units = units;
    }
    for (from, to) in renames {
        for texture in prepared.textures.iter_mut().filter(|t| t.name == from) {
            texture.name = to.clone();
        }
        for name in prepared.texture_names.values_mut().filter(|n| **n == from) {
            *name = to.clone();
        }
    }
}

/// The pictures of the original model that no material uses (the game may look for them by name) stay in the new model, written in 16 bits.
fn keep_other_pictures(textures: &mut Vec<texture::Texture>, original: &[u8]) {
    let Ok(old) = sango_core::model::parse_textures(original) else { return };
    for picture in old {
        if textures.iter().any(|t| t.name == picture.name) {
            continue;
        }
        if let Some(pixels) = sango_core::model::encode_texture(3, picture.width, picture.height, &picture.rgba) {
            textures.push(texture::Texture { name: picture.name, width: picture.width, height: picture.height, format: 3, mips: 1, pixels });
        }
    }
}
