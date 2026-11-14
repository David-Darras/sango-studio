//! The 8 files of a model of the game, made from the result of the conversion and from the packs of a template species.

use crate::convert::Converted;
use crate::gltf::Source;
use crate::pc;
use crate::texture::{self, Texture};

/// The face animations that every species has: the other ones of a template belong to meshes that only the template has.
const FACE_KEEP: [usize; 5] = [0, 3, 9, 10, 11];

/// The packs of the template species that the new model follows (the files 1, 5, 6 and 7 of its model).
pub struct TemplatePacks {
    pub face: Vec<u8>,
    pub battle: Vec<u8>,
    pub amie: Vec<u8>,
    pub setting: Vec<u8>,
}

impl TemplatePacks {
    /// From the folder of extracted models of the tools of the repository.
    pub fn from_tools(folder: &std::path::Path, species: u32) -> Result<TemplatePacks, String> {
        let read = |file: &str| std::fs::read(folder.join(format!("pm{species:04}_00_oras")).join(file)).map_err(|e| format!("{file}: {e}"));
        Ok(TemplatePacks { face: read("1_face.pc")?, battle: read("5_battle.pc")?, amie: read("6_amie.pc")?, setting: read("7_setting.pc")? })
    }
}

/// The settings of a model (64 bytes): the height in centimetres, the size class, the collision box and the camera of the Box.
///
/// `box_cm` is the box of the model in centimetres: minimum then maximum.
pub fn poke_setting(height_cm: i32, box_cm: [f32; 6]) -> Vec<u8> {
    let size: u32 = if height_cm < 80 { 0 } else if height_cm < 180 { 1 } else { 2 };
    let width = (box_cm[3] - box_cm[0]).abs() as f64;
    let depth = (box_cm[4] - box_cm[1]).abs() as f64;
    let camera = (0.0025 * (width * depth).sqrt() + 0.6).clamp(0.45, 3.0) as f32;
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&height_cm.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    for value in box_cm {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in [camera, 0.0, -20.0, 0.0, 0.0] {
        out.extend_from_slice(&f32::to_le_bytes(value));
    }
    out.extend_from_slice(&50i16.to_le_bytes());
    out.extend_from_slice(&50i16.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out
}

/// The box of the meshes of a glTF file in centimetres, and the height (the Y side).
pub fn box_of(source: &Source) -> ([f32; 6], i32) {
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    for primitive in &source.primitives {
        for p in &primitive.positions {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    let cm = |v: f32| (v as f64 * 100.0) as f32;
    let box_cm = [cm(low[0]), cm(low[1]), cm(low[2]), cm(high[0]), cm(high[1]), cm(high[2])];
    (box_cm, (box_cm[4] - box_cm[1]).round() as i32)
}

/// The list of the materials for the outline pass: 33 bytes for each one (the name padded with spaces to 32 bytes, an ID byte).
fn id_setting(names: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for name in names {
        let bytes = name.as_bytes();
        let mut record = bytes[..bytes.len().min(31)].to_vec();
        record.push(0);
        record.resize(32, b' ');
        out.extend_from_slice(&record);
        out.push(1);
    }
    let size = out.len().div_ceil(128).max(1) * 128;
    out.resize(size, 0);
    out
}

/// The files 1 to 7 hold the template's packs, with the entries that depend on the old model removed.
fn entries_of(pack: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    Ok(pc::entries(pack)?.into_iter().map(|e| e.to_vec()).collect())
}

/// The 8 files of a model.
pub fn assemble(converted: &Converted, templates: &TemplatePacks, setting: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    // The face animations that every species has.
    let face: Vec<Vec<u8>> = entries_of(&templates.face)?.into_iter().enumerate().map(|(i, e)| if FACE_KEEP.contains(&i) { e } else { Vec::new() }).collect();

    // The textures of the outline pass: one 8 x 8 texture of ID 0 for each texture.
    let ids: Vec<Texture> = converted
        .texture_names
        .iter()
        .filter(|n| *n != "DummyTex")
        .map(|n| format!("{n}Id"))
        .chain(["DummyTexId".to_string()])
        .map(|name| Texture { name, width: 8, height: 8, format: 7, mips: 1, pixels: vec![0; 64] })
        .collect();

    let mut settings = entries_of(&templates.setting)?;
    let first = settings.first_mut().ok_or("The settings pack of the template is empty.")?;
    let size = first.len();
    let mut replaced = setting.to_vec();
    replaced.resize(size, 0);
    replaced.truncate(size);
    *first = replaced;
    if settings.len() > 1 && settings[1].len() != 8 {
        settings[1] = vec![0; 8];
    }

    let mut files = vec![
        pc::pack(&[converted.model.clone(), converted.shadow.clone()]),
        pc::pack(&face),
        pc::pack(&[converted.textures.clone()]),
        pc::pack(&[converted.shiny_textures.clone()]),
        pc::pack(&[id_setting(&converted.material_names), Vec::new(), texture::write_texture_bch(ids)]),
    ];
    let box_bytes: Vec<u8> = setting.get(12..36).ok_or("The settings are cut.")?.to_vec();
    for (template, motions) in [(&templates.battle, &converted.battle_motions), (&templates.amie, &converted.amie_motions)] {
        let mut entries = entries_of(template)?;
        entries[0] = motions.clone();
        if entries.len() > 1 && entries[1].len() >= 24 {
            // The camera of the Box fits the collision box of the first motion: every motion that has a box gets the box of the model.
            let whole = entries[1].len() / 24 * 24;
            let mut boxes = Vec::with_capacity(entries[1].len());
            for chunk in entries[1][..whole].chunks(24) {
                boxes.extend_from_slice(if chunk.iter().all(|&b| b == 0) { chunk } else { &box_bytes });
            }
            boxes.extend_from_slice(&entries[1][whole..]);
            entries[1] = boxes;
        }
        // The animations of the meshes of the template have nothing to bind to in the new model.
        for (i, entry) in entries.iter_mut().enumerate() {
            if i >= 2 && entry.starts_with(b"BCH") {
                entry.clear();
            }
        }
        files.push(pc::pack(&entries));
    }
    files.push(pc::pack(&settings));
    // The order of the files: model, face, textures, shiny textures, ID, battle, Amie, settings.
    Ok(files)
}

/// A model taller than this (in the units of the file) is in the units of the game, not in metres: the biggest Pokémon is some metres tall.
const MAX_HEIGHT_IN_METRES: f32 = 30.0;

/// The 8 files of the model in a glTF file: the conversion and the packs around it.
///
/// `model_name` is the name inside the files (for example `pm0724_00`: it must differ from the names of the other models of the game),
/// `scale` makes the model bigger or smaller. A glTF file is in metres, and the game's unit is a centimetre; the files that Studio exports
/// are in the units of the game, so a model that is far too tall to be in metres is taken as already in the units of the game.
pub fn model_files(source: &Source, templates: &crate::convert::Templates, packs: &TemplatePacks, model_name: &str, scale: f64) -> Result<Vec<Vec<u8>>, String> {
    let (box_cm, height) = box_of(source);
    let in_game_units = height as f32 / 100.0 > MAX_HEIGHT_IN_METRES;
    let scale = if in_game_units { scale * 0.01 } else { scale };
    let converted = crate::convert::convert_with(source, templates, model_name, scale, true)?;
    let factor = scale as f32;
    let scaled = box_cm.map(|v| v * factor);
    assemble(&converted, packs, &poke_setting((height as f32 * factor).round() as i32, scaled))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::convert::Templates;
    use std::path::Path;

    const TOOLS: &str = r"C:\Users\David\CLionProjects\ctrpluginframework\sango_plugin\private\tools";

    #[test]
    fn the_packs_are_the_ones_of_the_python_tool() {
        let tools = Path::new(TOOLS);
        if !tools.exists() {
            return;
        }
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("model");
        let templates = Templates::from_tools(tools, 25).unwrap();
        let source = crate::gltf::read(&dir.join("body.gltf")).unwrap();
        let converted = crate::convert::convert(&source, &templates, "pm0025_01", 1.0).unwrap();
        let (box_cm, _) = box_of(&source);
        let setting = poke_setting(150, box_cm);
        let expected_setting = std::fs::read(dir.join("work").join("files").join("setting.bin")).unwrap();
        assert_eq!(setting, expected_setting, "the settings");
        let packs = TemplatePacks::from_tools(tools, 25).unwrap();
        let files = assemble(&converted, &packs, &setting).unwrap();
        let mut wrong = Vec::new();
        for (place, file) in files.iter().enumerate() {
            let expected = std::fs::read(dir.join("work").join("files").join(format!("{place}.pc"))).unwrap();
            if &expected != file {
                let at = expected.iter().zip(file).position(|(a, b)| a != b);
                wrong.push(format!("{place}.pc: sizes {} and {}, first difference {at:?}", expected.len(), file.len()));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
