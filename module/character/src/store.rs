//! The models of the characters of the overworld: the packs of the archive of the character models (the archive 21).
//!
//! Each pack of the archive has the 3D model of a character (part 0: a BCH file with its pictures), usually the motions of the character
//! (part 1: walk, run, jump, wave...) and a small model (part 2). The page changes a model by writing a whole pack on the SD card, in
//! `overworld/model/<pack>.bin`: the plugin gives this file to the game in place of the pack. The files are compressed (LZ11) like the files of the archive: the plugin gives them to the game as they are.

use crate::names::NAMES;
use sango_core::bundle;
use sango_core::model::{parse_model, parse_textures};
use sango_core::motion::MotionPack;
use sango_core::{Dump, Garc, SdCard};
use sango_modelport::skeleton::Joint;
use sango_species::model_export::{glb, Clip};
use std::collections::HashMap;

/// The folder of the packs on the SD card.
pub const FOLDER: &str = "overworld/model";

/// The alignment of the parts in a pack of the archive.
const ALIGNMENT: usize = 128;

/// The path of a pack on the SD card.
pub fn path(pack: usize) -> String {
    format!("{FOLDER}/{pack}.bin")
}

/// The archive of the character models.
pub struct Store {
    garc: Garc,
}

impl Store {
    pub fn open(dump: &Dump) -> Result<Store, String> {
        Ok(Store { garc: Garc::open(&dump.archive(21))? })
    }

    /// The number of packs.
    pub fn count(&self) -> usize {
        self.garc.file_count()
    }

    /// The names of the packs for the lists: the name of the model, and a number when two models have the same name.
    pub fn labels(&self) -> Vec<String> {
        let names: HashMap<u16, &str> = NAMES.iter().copied().collect();
        let base: Vec<String> = (0..self.count()).map(|n| names.get(&(n as u16)).map_or_else(|| format!("Model {}", n + 1), |s| (*s).to_string())).collect();
        let mut total: HashMap<&str, usize> = HashMap::new();
        for name in &base {
            *total.entry(name.as_str()).or_default() += 1;
        }
        let mut seen: HashMap<&str, usize> = HashMap::new();
        base.iter()
            .map(|name| {
                if total[name.as_str()] > 1 {
                    let nth = seen.entry(name.as_str()).or_default();
                    *nth += 1;
                    format!("{name} ({nth})")
                } else {
                    name.clone()
                }
            })
            .collect()
    }

    /// The pack of the game.
    pub fn game_pack(&self, pack: usize) -> Option<Vec<u8>> {
        self.garc.first_file_decompressed(pack)
    }

    /// The pack that the game uses now: the SD card first.
    pub fn pack(&self, sd: Option<&SdCard>, pack: usize) -> Option<Vec<u8>> {
        sd.and_then(|sd| sd.read(&path(pack))).map(|bytes| if sango_core::lz::is_compressed(&bytes) { sango_core::lz::decompress(&bytes).unwrap_or(bytes) } else { bytes }).or_else(|| self.game_pack(pack))
    }

    /// True when the SD card replaces a pack.
    #[cfg(test)]
    pub fn changed(sd: Option<&SdCard>, pack: usize) -> bool {
        sd.is_some_and(|sd| sd.read(&path(pack)).is_some())
    }

    /// The packs that the SD card replaces.
    pub fn changed_packs(sd: Option<&SdCard>) -> Vec<usize> {
        let Some(sd) = sd else { return Vec::new() };
        let mut found: Vec<usize> = sd
            .files_in(FOLDER)
            .iter()
            .filter_map(|file| file.rsplit('/').next().and_then(|name| name.strip_suffix(".bin")).and_then(|n| n.parse::<usize>().ok()))
            .collect();
        found.sort_unstable();
        found
    }

    /// The parts of a pack.
    pub fn parts(pack: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        Ok(bundle::split(pack)?.1.into_iter().map(|p| p.to_vec()).collect())
    }

    /// The model of a pack with its pictures, and its motions when it has some.
    pub fn read(pack: &[u8]) -> Result<Loaded, String> {
        let parts = Store::parts(pack)?;
        let first = parts.first().ok_or("The pack has no model.")?;
        let model = parse_model(first)?;
        let textures = parse_textures(first)?.into_iter().map(|t| (t.name.clone(), t)).collect();
        let motions = parts.get(1).and_then(|part| MotionPack::parse(part).ok());
        Ok(Loaded { model, textures, motions })
    }

    /// The .glb file of a pack: the skeleton, the meshes, the pictures, and one animation for each motion of the character (`motion 12`...).
    pub fn export(&self, sd: Option<&SdCard>, pack: usize) -> Result<Vec<u8>, String> {
        let bytes = self.pack(sd, pack).ok_or("This model is not in the archive.")?;
        let loaded = Store::read(&bytes)?;
        glb(&loaded.model, &loaded.textures, &loaded.clips())
    }

    /// Replaces the model of a pack by the one of a glTF or GLB file. The skeleton and the motions of the pack stay: the motions move the
    /// joints by name, so the new model must have the joints of the old one (the page tells which are missing).
    pub fn import(&self, sd: &SdCard, pack: usize, file: &std::path::Path, scale: f64) -> Result<(), String> {
        let current = self.pack(Some(sd), pack).ok_or("This model is not in the archive.")?;
        let parts = Store::parts(&current)?;
        let original = parts.first().ok_or("The pack has no model.")?;
        let source = sango_modelport::gltf::read(file)?;
        let bones = parse_model(original)?.bones;
        let joints: Vec<Joint> = bones
            .iter()
            .map(|b| Joint { name: b.name.clone(), parent: b.parent.and_then(|p| bones.get(p)).map(|p| p.name.clone()).unwrap_or_default(), scale: b.scale, rot: b.rotation.map(f64::from), trans: b.translation })
            .collect();
        let made = sango_modelport::trainer::trainer_model_within(&source, original, &joints, scale, original.len() / 2, true)?;
        if made.len() > original.len() * 2 {
            return Err(format!("The new model is too big for the game ({} KB, the old one has {} KB). Use a model with fewer polygons or smaller pictures.", made.len() / 1024, original.len() / 1024));
        }
        // The fields that the engine reads, as the models of the game have them.
        let made = sango_core::model::normalize_for_engine(&made)?.unwrap_or(made);
        let new_pack = bundle::replace_part_with(&current, 0, &made, ALIGNMENT)?;
        Store::write(sd, pack, &new_pack)
    }

    /// Writes a pack on the SD card, compressed as the game reads it.
    fn write(sd: &SdCard, pack: usize, bytes: &[u8]) -> Result<(), String> {
        sd.write(&path(pack), &sango_core::lz::compress_lz11(bytes))?;
        sd.mark_changed(&path(pack))
    }

    /// The file of a pack on the SD card, as it is, or `None` when the card does not replace the pack.
    pub fn card_file(sd: &SdCard, pack: usize) -> Option<Vec<u8>> {
        sd.read(&path(pack))
    }

    /// Puts back what [`Store::card_file`] gave.
    pub fn put_card_file(sd: &SdCard, pack: usize, file: Option<&[u8]>) -> Result<(), String> {
        match file {
            Some(bytes) => sd.write(&path(pack), bytes).map(|_| ())?,
            None => sd.remove(&path(pack)).map(|_| ())?,
        }
        sd.mark_changed(&path(pack))
    }

    /// Gives back the model of the game.
    pub fn reset(&self, sd: &SdCard, pack: usize) -> Result<(), String> {
        sd.remove(&path(pack))?;
        sd.mark_changed(&path(pack))
    }
}

/// A model, its pictures and its motions.
pub struct Loaded {
    pub model: sango_core::model::Model,
    pub textures: HashMap<String, sango_core::model::Texture>,
    pub motions: Option<MotionPack>,
}

impl Loaded {
    /// The number of motions that the character has.
    pub fn motion_count(&self) -> usize {
        self.motions.as_ref().map_or(0, |m| (0..m.len()).filter(|&i| m.has(i)).count())
    }

    /// One clip for each motion.
    pub fn clips(&self) -> Vec<Clip> {
        let Some(pack) = &self.motions else { return Vec::new() };
        (0..pack.len())
            .filter(|&i| pack.has(i))
            .filter_map(|index| {
                let motion = pack.motion(index).ok()?;
                let frames = sango_species::model_export::motion_frames(&self.model, pack, &motion);
                Some(Clip { name: format!("motion {index}"), frames })
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn store() -> Option<Store> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        Store::open(&Dump::open(&root).unwrap()).ok()
    }

    /// With the dump: the models have names, and Brendan exports as a GLB file with animations.
    #[test]
    fn a_character_is_exported_with_its_motions() {
        let Some(store) = store() else { return };
        assert_eq!(store.count(), 544);
        let labels = store.labels();
        assert_eq!(labels[172], "Brendan");
        let glb = store.export(None, 172).unwrap();
        assert_eq!(&glb[..4], b"glTF");
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json = String::from_utf8_lossy(&glb[20..20 + json_len]);
        assert!(json.contains("\"animations\""), "the file has animations");
        assert!(json.contains("motion 12"));
        assert!(glb.len() > 200_000);
    }

    /// With the dump: a model is imported from its own GLB file, then it is read again, and the model comes back with a reset.
    #[test]
    fn a_character_is_imported_and_given_back() {
        let Some(store) = store() else { return };
        let folder = std::env::temp_dir().join(format!("sango_character_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let original = Store::read(&store.game_pack(172).unwrap()).unwrap();
        let file = folder.join("brendan.glb");
        std::fs::write(&file, store.export(None, 172).unwrap()).unwrap();
        store.import(&sd, 172, &file, 1.0).unwrap();
        assert!(Store::changed(Some(&sd), 172));
        assert!(sd.read("overworld/model/index.bin").is_some());
        let again = Store::read(&store.pack(Some(&sd), 172).unwrap()).unwrap();
        assert_eq!(again.model.bones.len(), original.model.bones.len(), "the skeleton");
        assert_eq!(again.model.meshes.len(), original.model.meshes.len(), "the meshes");
        assert_eq!(again.motion_count(), original.motion_count(), "the motions stay");
        // The pack has the parts of the game after the new model.
        assert_eq!(Store::parts(&store.pack(Some(&sd), 172).unwrap()).unwrap().len(), Store::parts(&store.game_pack(172).unwrap()).unwrap().len());
        store.reset(&sd, 172).unwrap();
        assert!(!Store::changed(Some(&sd), 172));
        let _ = std::fs::remove_dir_all(&folder);
    }
}

#[cfg(test)]
mod size {
    use super::*;
    /// With the dump: an imported model is not much bigger than the old one (a bigger one froze the game), and its pictures are the right way up.
    #[test]
    fn an_imported_model_is_not_much_bigger_than_the_old_one() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let store = Store::open(&Dump::open(&root).unwrap()).unwrap();
        let orig = store.game_pack(172).unwrap();
        let o = Store::parts(&orig).unwrap();
        let src = std::env::temp_dir().join("scr172.glb");
        std::fs::write(&src, store.export(None, 172).unwrap()).unwrap();
        let source = sango_modelport::gltf::read(&src).unwrap();
        let bones = parse_model(&o[0]).unwrap().bones;
        let joints: Vec<Joint> = bones.iter().map(|b| Joint { name: b.name.clone(), parent: b.parent.and_then(|p| bones.get(p)).map(|p| p.name.clone()).unwrap_or_default(), scale: b.scale, rot: b.rotation.map(f64::from), trans: b.translation }).collect();
        let made = sango_modelport::trainer::trainer_model_within(&source, &o[0], &joints, 1.0, 1000, true).unwrap();
        assert!(made.len() <= o[0].len() * 2, "{} bytes against {}", made.len(), o[0].len());
        let names: Vec<String> = parse_textures(&made).unwrap().into_iter().map(|t| t.name).collect();
        for t in parse_textures(&o[0]).unwrap() {
            assert!(names.contains(&t.name), "the picture {} stays", t.name);
        }
        // The units are the ones of the old model: the game fills the unit 0 and the mix of colours reads the unit 1.
        let (old_model, new_model) = (parse_model(&o[0]).unwrap(), parse_model(&made).unwrap());
        for (x, y) in old_model.materials.iter().zip(&new_model.materials) {
            assert_eq!(x.units[0].texture, y.units[0].texture, "unit 0 of {}", x.name);
            assert_eq!(x.units[1].texture, y.units[1].texture, "unit 1 of {}", x.name);
        }
        let a = parse_textures(&o[0]).unwrap();
        let b = parse_textures(&made).unwrap();
        for suffix in ["body", "face", "head", "sw_parts"] {
            let x = a.iter().find(|t| t.name.ends_with(suffix)).unwrap();
            let y = b.iter().find(|t| t.name.contains(suffix)).unwrap();
            let same: f64 = x.rgba.iter().zip(&y.rgba).map(|(p, q)| (*p as f64 - *q as f64).abs()).sum::<f64>() / x.rgba.len() as f64;
            let flipped: f64 = {
                let row = x.width * 4;
                let rows = x.height;
                (0..rows).map(|r| (0..row).map(|c| (x.rgba[(rows - 1 - r) * row + c] as f64 - y.rgba[r * row + c] as f64).abs()).sum::<f64>()).sum::<f64>() / x.rgba.len() as f64
            };
            assert!(same < 8.0 && same < flipped, "{suffix}: error {same:.1}, flipped {flipped:.1}");
        }
    }
}
