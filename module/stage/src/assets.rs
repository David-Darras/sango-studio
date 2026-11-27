//! The files of the game that a choreography needs: the two Pokémon, the effects, the models and their animations, and
//! the place of the battle. The files that the user changed on the SD card come first.

use crate::clip::CameraClip;
use sango_core::animation::Look;
use sango_core::model::{first_bone_name, parse_model, parse_textures, Material, Model, Stage, Texture};
use sango_core::motion::MotionPack;
use sango_core::{lz, Bundle, Garc, SdCard, Session};
use sango_effect::Effect;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// The archives of the dump.
const POKEMON_MODELS: usize = 8;
const EFFECTS: usize = 31;
const BATTLE_MODELS: usize = 32;
const CHOREOGRAPHIES: usize = 34;
const ATTACK_MOTIONS: usize = 6;
const BACKGROUNDS: usize = 136;

/// The files of a model of a Pokémon, from its first file.
const FACE_FILE: usize = 1;
const TEXTURE_FILE: usize = 2;
const SHINY_TEXTURE_FILE: usize = 3;
const BATTLE_MOTION_FILE: usize = 5;
const SETTING_FILE: usize = 7;

/// The number of records of a motion of the attack table: one for each kind of attack, then the special move.
const ATTACK_KINDS: usize = 14;
const ATTACK_RECORD: usize = ATTACK_KINDS + 4;
/// The number of species of the game.
const SPECIES: usize = 721;

/// The points of the camera that a few big Pokémon change: the species, the form and the factor.
const CAMERA_SIZES: [(usize, usize, f32); 31] = [
    (51, 0, 1.728), (95, 0, 0.775), (110, 0, 1.277), (130, 0, 0.679), (130, 1, 0.677), (142, 0, 0.857), (142, 1, 0.891), (208, 0, 0.874), (250, 0, 0.826), (321, 0, 0.478), (357, 0, 0.842), (382, 0, 0.702),
    (415, 0, 1.375), (450, 0, 0.888), (473, 0, 0.935), (483, 0, 0.915), (487, 0, 0.930), (487, 1, 0.791), (646, 1, 0.856), (646, 2, 0.853), (658, 0, 1.531), (713, 0, 0.605), (716, 1, 0.813), (717, 0, 0.644),
    (718, 0, 0.888), (208, 1, 0.97), (373, 1, 1.5), (382, 1, 0.5), (383, 1, 0.5), (384, 1, 1.15), (720, 1, 0.75),
];

/// Which Pokémon to show: the species, the form and the colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub species: usize,
    pub form: usize,
    pub shiny: bool,
    pub female: bool,
}

impl Choice {
    pub fn new(species: usize) -> Choice {
        Choice { species, form: 0, shiny: false, female: false }
    }
}

/// A Pokémon, ready for a battle.
pub struct Pokemon {
    pub choice: Choice,
    pub model: Model,
    pub textures: HashMap<String, Texture>,
    /// The battle motions.
    pub motions: Option<MotionPack>,
    /// What each battle motion changes besides the bones.
    pub looks: Vec<Look>,
    /// The animation that goes round and round over every motion (flames, gas).
    pub endless: Look,
    /// For each battle motion, the frame where its effect starts, when the file gives one.
    pub keys: Vec<Option<i32>>,
    /// The height that the file gives the Pokémon, in centimeters.
    pub height: u32,
    /// The height that the battle gives it.
    pub shown_height: u32,
    /// 0 small, 1 medium, 2 large: the places of the battle change with it.
    pub size: usize,
    /// True for a Pokémon that does not leave the ground.
    pub no_jump: bool,
    /// The top of the model in its rest pose.
    pub top: f32,
}

impl Pokemon {
    /// The size of the model: the model of the file has the height of the Pokémon, the battle shows it a little bigger when it
    /// is small and a little smaller when it is large.
    pub fn model_scale(&self) -> f32 {
        self.shown_height as f32 / self.height.max(1) as f32
    }

    /// The factor that the places of the effects follow: 1 for a Pokémon of one meter.
    pub fn effect_scale(&self) -> f32 {
        self.shown_height as f32 * 0.01
    }

    /// The factor that the places of the camera follow.
    pub fn camera_scale(&self) -> f32 {
        let rate = self.effect_scale();
        let mut camera = if rate < 1.0 { 1.0 - (1.0 - rate) * 0.5 } else { rate };
        if let Some(&(_, _, factor)) = CAMERA_SIZES.iter().find(|(species, form, _)| (*species, *form) == (self.choice.species, self.choice.form)) {
            camera *= factor;
        }
        camera
    }

    /// The number of frames of a battle motion, when the Pokémon has it.
    pub fn motion_frames(&self, motion: usize) -> Option<u16> {
        let pack = self.motions.as_ref()?;
        pack.has(motion).then(|| pack.motion(motion).ok()).flatten().map(|m| m.frames)
    }
}

/// A model of the archive of the battle models.
pub struct ModelFile {
    pub model: Model,
    pub textures: HashMap<String, Texture>,
}

/// A particle effect with its pictures.
pub struct EffectFile {
    pub effect: Effect,
    pub looks: Vec<sango_effect::Look>,
}

/// A place of a battle: a model, its pictures and the animation of its materials.
pub struct Place {
    pub model: Model,
    pub textures: HashMap<String, Texture>,
    pub look: Look,
}

/// The parts of a file of an archive: it is compressed or not, and it is a bundle.
fn parts(garc: &Garc, file: usize) -> Result<Vec<Vec<u8>>, String> {
    let bytes = garc.first_file_decompressed(file).ok_or_else(|| format!("The file {file} is missing."))?;
    bundle_parts(bytes)
}

fn bundle_parts(bytes: Vec<u8>) -> Result<Vec<Vec<u8>>, String> {
    let bundle = Bundle::parse(bytes)?;
    Ok((0..bundle.resource_count()).map(|i| bundle.resource(i).map_or_else(Vec::new, <[u8]>::to_vec)).collect())
}

fn by_name(textures: Vec<Texture>) -> HashMap<String, Texture> {
    textures.into_iter().map(|t| (t.name.clone(), t)).collect()
}

/// Removes the steps at the end of a material that keep what they receive: the drawing is faster.
fn trim(material: &mut Material) {
    let keeps = |s: &Stage| s.color_source[0] == 15 && s.alpha_source[0] == 15 && s.color_mode == 0 && s.alpha_mode == 0 && s.color_operand[0] == 0 && s.alpha_operand[0] == 0 && s.color_scale == 0 && s.alpha_scale == 0 && !s.save_color && !s.save_alpha;
    while material.stages.len() > 1 && material.stages.last().is_some_and(keeps) {
        material.stages.pop();
    }
}

/// A few materials of the places mix a picture that the file does not give: the battle puts it there while it runs (the
/// light on a plate). Without it, the material shows its own picture, softly joined to the ground by the opacity of the
/// vertices.
fn simplify(material: &mut Material) {
    trim(material);
    let missing = |source: u8| (3..=5).contains(&source) && material.units[usize::from(source - 3)].texture.is_empty();
    if !material.stages.iter().any(|stage| stage.color_source.iter().any(|&source| missing(source))) {
        return;
    }
    let plain = Stage { color_source: [3, 0, 15], color_operand: [0; 3], alpha_source: [0, 14, 15], alpha_operand: [1, 0, 0], color_mode: 1, alpha_mode: 0, color_scale: 0, alpha_scale: 0, constant: [255; 4], save_color: false, save_alpha: false };
    material.stages = vec![plain];
    material.state.blend = true;
    (material.state.color_source, material.state.color_destination) = (6, 7);
    material.state.alpha_test = None;
}

/// The frame of the first key of the kind "effect" of a key file of a motion.
fn effect_key(data: &[u8]) -> Option<i32> {
    let word = |at: usize| data.get(at..at + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let version = word(4)?;
    let header = if version >= 2 { 14 } else { 12 };
    let mut at = 16;
    loop {
        let start = word(at)?;
        if start == -1 {
            return None;
        }
        let kind = data.get(at + header - 2..at + header).map(|b| u16::from_le_bytes([b[0], b[1]]))?;
        // The one command of these files: a kind of key (0 the effect) and a value.
        if kind != 0 {
            return None;
        }
        if word(at + header)? == 0 {
            return Some(start);
        }
        at += header + 8;
    }
}

/// The files of the game that the viewer reads, each one once.
#[derive(Default)]
pub struct Library {
    root: Option<PathBuf>,
    card: Option<PathBuf>,
    archives: HashMap<usize, Rc<Garc>>,
    pokemon: Vec<(Choice, Rc<Pokemon>)>,
    effects: HashMap<i32, Option<Rc<EffectFile>>>,
    models: HashMap<i32, Option<Rc<ModelFile>>>,
    motions: HashMap<i32, Option<Rc<MotionPack>>>,
    animations: HashMap<i32, Option<Rc<Look>>>,
    cameras: HashMap<i32, Option<Rc<CameraClip>>>,
    places: HashMap<usize, Option<Rc<Place>>>,
}

/// The number of Pokémon that stay in memory.
const KEPT_POKEMON: usize = 4;

impl Library {
    /// Takes the folders of a session. Everything that was read is forgotten when they change: gives true then.
    pub fn use_session(&mut self, session: &Session) -> bool {
        let root = session.dump.as_ref().map(|dump| dump.root().to_path_buf());
        let card = session.sd.as_ref().map(|sd| sd.root().to_path_buf());
        let changed = root != self.root || card != self.card;
        if changed {
            *self = Library { root, card, ..Library::default() };
        }
        changed
    }

    /// Forgets the effects, the models and the places: they are read again, with what the user changed since.
    pub fn forget(&mut self) {
        self.effects.clear();
        self.models.clear();
        self.motions.clear();
        self.animations.clear();
        self.cameras.clear();
        self.places.clear();
        self.pokemon.clear();
    }

    /// Frees the files from time to time in a test that plays every choreography: the memory stays small.
    #[cfg(test)]
    pub(crate) fn forget_files_between_runs(&mut self, run: usize) {
        if run.is_multiple_of(20) {
            self.effects.clear();
            self.models.clear();
            self.motions.clear();
            self.animations.clear();
        }
    }

    fn archive(&mut self, number: usize) -> Option<Rc<Garc>> {
        if let Some(garc) = self.archives.get(&number) {
            return Some(garc.clone());
        }
        let garc = Rc::new(Garc::open(&sango_core::archive_path(self.root.as_ref()?, number)).ok()?);
        self.archives.insert(number, garc.clone());
        Some(garc)
    }

    /// The number of choreography files.
    pub fn choreography_count(&mut self) -> usize {
        self.archive(CHOREOGRAPHIES).map_or(0, |garc| garc.file_count())
    }

    /// A Pokémon: its model, its pictures, its motions and its sizes.
    pub fn pokemon(&mut self, session: &Session, choice: Choice) -> Result<Rc<Pokemon>, String> {
        if let Some((_, found)) = self.pokemon.iter().find(|(known, _)| *known == choice) {
            return Ok(found.clone());
        }
        let loaded = Rc::new(self.read_pokemon(session, choice)?);
        if self.pokemon.len() >= KEPT_POKEMON {
            self.pokemon.remove(0);
        }
        self.pokemon.push((choice, loaded.clone()));
        Ok(loaded)
    }

    fn read_pokemon(&mut self, session: &Session, choice: Choice) -> Result<Pokemon, String> {
        let archive = self.archive(POKEMON_MODELS).ok_or("The archive of the Pokémon models cannot be read.")?;
        // The list of the models: for each species, the place of its first model, the number of models and flags.
        let list = archive.first_file(0).ok_or("The list of the models is missing.")?;
        let entry = choice.species.checked_sub(1).and_then(|n| list.get(n * 4..n * 4 + 4));
        let (files, first): (Vec<Vec<u8>>, usize) = match entry.filter(|_| choice.species <= SPECIES) {
            Some(entry) => {
                let (start, count, flags) = (usize::from(u16::from_le_bytes([entry[0], entry[1]])), usize::from(entry[2]), entry[3]);
                if count == 0 {
                    return Err(format!("The species {} has no model.", choice.species));
                }
                let female = usize::from(flags & 2 != 0);
                let slot = match choice.form {
                    0 => usize::from(choice.female) * female,
                    form => (form + female).min(count - 1),
                };
                let mut first = (start + slot) * 8 + 3;
                // A form that shares the model of the species has no model of its own.
                if parts(&archive, first).ok().and_then(|p| first_bone_name(p.first()?)).is_none() {
                    first = start * 8 + 3;
                }
                (Vec::new(), first)
            }
            None => (Self::card_model(session, choice)?, 0),
        };
        let read = |offset: usize| -> Result<Vec<Vec<u8>>, String> {
            if files.is_empty() {
                parts(&archive, first + offset)
            } else {
                let bytes = files.get(offset).ok_or("A file of the model is missing.")?;
                bundle_parts(if lz::is_compressed(bytes) { lz::decompress(bytes)? } else { bytes.clone() })
            }
        };
        let model = parse_model(read(0)?.first().ok_or("The file of the model has no part.")?)?;
        let texture_file = if choice.shiny { SHINY_TEXTURE_FILE } else { TEXTURE_FILE };
        let textures = by_name(read(texture_file)?.first().map(|bytes| parse_textures(bytes)).transpose()?.unwrap_or_default());
        let endless = read(FACE_FILE).map(|p| Look::read_loop(&p)).unwrap_or_default();
        let (motions, looks, keys) = match read(BATTLE_MOTION_FILE) {
            Ok(parts) => {
                let pack = parts.first().and_then(|bytes| MotionPack::parse(bytes).ok());
                let count = pack.as_ref().map_or(0, MotionPack::len);
                let looks = (0..count).map(|index| Look::read(&parts, index)).collect();
                // The parts: the motions, a part, then four blocks of one part for each motion. The keys are the last block.
                let block = (parts.len() + 1) / 4;
                let keys = (0..count).map(|index| (index < block).then(|| parts.get(2 + 3 * block + index)).flatten().and_then(|part| effect_key(part))).collect();
                (pack, looks, keys)
            }
            Err(_) => (None, Vec::new(), Vec::new()),
        };
        // The sizes of the Pokémon: its height, the height that is shown and its size class.
        let setting = read(SETTING_FILE).ok().and_then(|p| p.into_iter().next()).unwrap_or_default();
        let number = |at: usize| setting.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let height = match number(0) {
            0 | 1 => 100,
            cm => cm,
        };
        let shown_height = match number(4) {
            0 => ((height as f32).powf(1.0 - 0.43) * 100f32.powf(0.43)) as u32,
            1 => 100,
            shown => shown,
        };
        let size = match setting.len() {
            0..=11 => 1,
            _ => (number(8) as usize).min(2),
        };
        let no_jump = self.archive(sango_species_base::ARCHIVE).and_then(|garc| sango_species_base::game_data(&garc, choice.species, choice.form)).is_some_and(|data| data.ev_yield & 0x1000 != 0);
        let top = model.meshes.iter().flat_map(|m| &m.vertices).map(|v| v.position[1]).fold(0.0, f32::max);
        Ok(Pokemon { choice, model, textures, motions, looks, endless, keys, height, shown_height, size, no_jump, top })
    }

    /// The files of the model of a species of a pack, from the folder `species/model` of the SD card.
    fn card_model(session: &Session, choice: Choice) -> Result<Vec<Vec<u8>>, String> {
        let sd = session.sd.as_ref().ok_or("This species is on the SD card, which is not chosen.")?;
        let list = sd.read("species/model/0.raw").ok_or("The SD card has no list of models.")?;
        let entry = choice.species.checked_sub(1).and_then(|n| list.get(4 + n * 4..8 + n * 4)).ok_or("This species has no model on the SD card.")?;
        let (start, count) = (usize::from(u16::from_le_bytes([entry[0], entry[1]])), usize::from(entry[2]));
        if count == 0 {
            return Err("This species has no model on the SD card.".to_string());
        }
        let first = (start + choice.form.min(count - 1)) * 8 + 3;
        Ok((0..8).map(|offset| sd.read(&format!("species/model/{}.pc", first + offset)).and_then(|b| b.get(4..).map(<[u8]>::to_vec)).unwrap_or_default()).collect())
    }

    /// The table that says which motion a Pokémon plays for a kind of attack.
    pub fn attack_table(&mut self) -> Vec<u8> {
        self.archive(ATTACK_MOTIONS).and_then(|garc| garc.first_file_decompressed(0)).unwrap_or_default()
    }

    /// A particle effect: the file of the SD card, else the one of the game.
    pub fn effect(&mut self, sd: Option<&SdCard>, number: i32) -> Option<Rc<EffectFile>> {
        if let Some(found) = self.effects.get(&number) {
            return found.clone();
        }
        let loaded = (|| {
            let id = usize::try_from(number).ok()?;
            let bytes = match sd.and_then(|sd| sd.read(&format!("effect/{id}.bin"))) {
                Some(bytes) if lz::is_compressed(&bytes) => lz::decompress(&bytes).ok()?,
                Some(bytes) => bytes,
                None => self.archive(EFFECTS)?.first_file_decompressed(id)?,
            };
            let effect = Effect::parse(&bytes).ok()?;
            let looks = sango_effect::looks(&effect);
            Some(Rc::new(EffectFile { effect, looks }))
        })();
        self.effects.insert(number, loaded.clone());
        loaded
    }

    fn battle_file(&mut self, number: i32) -> Option<Vec<u8>> {
        self.archive(BATTLE_MODELS)?.first_file_decompressed(usize::try_from(number).ok()?)
    }

    /// A model of the archive of the battle models.
    pub fn model(&mut self, number: i32) -> Option<Rc<ModelFile>> {
        if let Some(found) = self.models.get(&number) {
            return found.clone();
        }
        let loaded = self.battle_file(number).and_then(|bytes| {
            let mut model = parse_model(&bytes).ok()?;
            model.materials.iter_mut().for_each(trim);
            (!model.meshes.is_empty()).then(|| Rc::new(ModelFile { model, textures: by_name(parse_textures(&bytes).unwrap_or_default()) }))
        });
        self.models.insert(number, loaded.clone());
        loaded
    }

    /// The motions of a model of the battle models.
    pub fn motions(&mut self, number: i32) -> Option<Rc<MotionPack>> {
        if let Some(found) = self.motions.get(&number) {
            return found.clone();
        }
        let loaded = self.battle_file(number).and_then(|bytes| MotionPack::parse(&bytes).ok()).map(Rc::new);
        self.motions.insert(number, loaded.clone());
        loaded
    }

    /// An animation of the materials or of the visibility of a model of the battle models.
    pub fn animation(&mut self, number: i32) -> Option<Rc<Look>> {
        if let Some(found) = self.animations.get(&number) {
            return found.clone();
        }
        let loaded = self.battle_file(number).and_then(|bytes| {
            let mut look = Look::default();
            look.add(&bytes).ok().map(|()| Rc::new(look))
        });
        self.animations.insert(number, loaded.clone());
        loaded
    }

    /// A camera animation of the battle models.
    pub fn camera(&mut self, number: i32) -> Option<Rc<CameraClip>> {
        if let Some(found) = self.cameras.get(&number) {
            return found.clone();
        }
        let loaded = self.battle_file(number).and_then(|bytes| CameraClip::parse(&bytes)).map(Rc::new);
        self.cameras.insert(number, loaded.clone());
        loaded
    }

    /// A piece of the places of the battles (a ground, a plate, a sky): the file of the SD card, else the one of the game.
    pub fn place(&mut self, sd: Option<&SdCard>, number: usize) -> Option<Rc<Place>> {
        if let Some(found) = self.places.get(&number) {
            return found.clone();
        }
        let loaded = (|| {
            let bytes = match sd.and_then(|sd| sd.read(&format!("battle/background/{number}.bin"))) {
                Some(bytes) if lz::is_compressed(&bytes) => lz::decompress(&bytes).ok()?,
                Some(bytes) => bytes,
                None => self.archive(BACKGROUNDS)?.first_file_decompressed(number)?,
            };
            // A bundle (the model, then the animations of the bones, of the materials and of the visibility), or a model.
            let parts = if bytes.starts_with(b"BCH") { vec![bytes] } else { bundle_parts(bytes).ok()? };
            let mut model = parse_model(parts.first()?).ok()?;
            for material in &mut model.materials {
                simplify(material);
            }
            let textures = by_name(parse_textures(parts.first()?).unwrap_or_default());
            let mut look = Look::default();
            for part in parts.iter().skip(2).take(2) {
                let _ = look.add(part);
            }
            Some(Rc::new(Place { model, textures, look }))
        })();
        self.places.insert(number, loaded.clone());
        loaded
    }
}

/// The motion that a Pokémon plays for a kind of attack (0 physical to 12 guard) of a move: `None` for no motion. The
/// number is the one of a physical attack (0 to 3) or of a special attack (4 to 7).
pub fn attack_motion(table: &[u8], species: usize, form: usize, move_id: usize, kind: usize) -> Option<usize> {
    let word = |at: usize| table.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    let place = species.checked_sub(1).filter(|&n| n < SPECIES)?;
    let (start, end) = (word(place * 2)?, word(place * 2 + 2)?);
    let record = start + if end.saturating_sub(start) > form { form } else { 0 };
    let record = table.get((SPECIES + 1) * 2 + record * ATTACK_RECORD..)?.get(..ATTACK_RECORD)?;
    let special = usize::from(u16::from_le_bytes([record[ATTACK_KINDS], record[ATTACK_KINDS + 1]]));
    let motion = if special != 0 && special == move_id { record[ATTACK_KINDS + 2] } else { *record.get(kind)? };
    (motion < 10).then_some(usize::from(motion))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_key_of_the_effect_is_found_in_a_key_file() {
        let mut data = vec![0u8; 4];
        for word in [2i32, 0, 60] {
            data.extend_from_slice(&word.to_le_bytes());
        }
        for (start, kind) in [(5i32, 2i32), (24, 0)] {
            for word in [start, start] {
                data.extend_from_slice(&word.to_le_bytes());
            }
            data.extend_from_slice(&[0; 6]);
            data.extend_from_slice(&kind.to_le_bytes());
            data.extend_from_slice(&1.0f32.to_le_bytes());
        }
        data.extend_from_slice(&(-1i32).to_le_bytes());
        assert_eq!(effect_key(&data), Some(24));
    }

    #[test]
    fn a_motion_of_an_attack_comes_from_the_table() {
        let mut table = Vec::new();
        for species in 0..=SPECIES {
            table.extend_from_slice(&(species as u16).to_le_bytes());
        }
        let mut record = [10u8; ATTACK_RECORD];
        record[0] = 5;
        record[ATTACK_KINDS..].copy_from_slice(&[85, 0, 2, 0]);
        table.extend_from_slice(&record);
        assert_eq!(attack_motion(&table, 1, 0, 1, 0), Some(5));
        assert_eq!(attack_motion(&table, 1, 0, 1, 1), None);
        assert_eq!(attack_motion(&table, 1, 0, 85, 1), Some(2), "the special move of the species");
    }
}
