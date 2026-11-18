//! The files of the trainers: the archives of the game, the files that the user changed on the SD card, and the texts.

use sango_core::text::{self, TextFile, LANGUAGES};
use sango_core::trainer::{self as data, BattleExtras, Team, TrainerClass, TrainerData};
use sango_core::{Clim, Dump, Garc, SdCard};
use std::collections::{HashMap, HashSet};

const DATA_FOLDER: &str = "trainer/data";
const TEAM_FOLDER: &str = "trainer/pokemon";
const CLASS_FOLDER: &str = "trainer/class";
const EXTRA_FOLDER: &str = "trainer/extra";
const PICTURE_FOLDER: &str = "trainer/picture";
const MODEL_FOLDER: &str = "trainer/model";

/// The place of a file of a folder of the SD card.
fn path(folder: &str, id: usize) -> String {
    format!("{folder}/{id}.bin")
}

/// The archives of the trainers and of the texts, and the texts that were read.
pub struct Store {
    data: Garc,
    teams: Garc,
    classes: Garc,
    messages: Garc,
    pictures: Garc,
    models: Garc,
    /// The archives of texts, opened when a language is used.
    languages: HashMap<usize, Option<Garc>>,
    dump: Dump,
    /// The files of texts that were read: the language and the number of the file.
    texts: HashMap<(usize, usize), Option<TextFile>>,
}

impl Store {
    /// Opens the archives of the dump.
    pub fn open(dump: &Dump) -> Result<Store, String> {
        Ok(Store {
            data: Garc::open(&dump.archive(data::DATA_ARCHIVE))?,
            teams: Garc::open(&dump.archive(data::TEAM_ARCHIVE))?,
            classes: Garc::open(&dump.archive(data::CLASS_ARCHIVE))?,
            messages: Garc::open(&dump.archive(data::MESSAGE_ARCHIVE))?,
            pictures: Garc::open(&dump.archive(data::PICTURE_ARCHIVE))?,
            models: Garc::open(&dump.archive(data::MODEL_ARCHIVE))?,
            languages: HashMap::new(),
            dump: dump.clone(),
            texts: HashMap::new(),
        })
    }

    /// The data and the team of a trainer: the SD card first, then the game.
    pub fn trainer(&self, sd: Option<&SdCard>, id: usize) -> Result<(TrainerData, Team), String> {
        let from_card = |folder: &str| sd.and_then(|sd| sd.read(&path(folder, id)));
        let bytes = from_card(DATA_FOLDER).or_else(|| self.data.first_file(id).map(|b| b.to_vec())).ok_or_else(|| format!("The trainer {id} has no data."))?;
        let trainer = TrainerData::from_bytes(&bytes)?;
        let team_bytes = from_card(TEAM_FOLDER).or_else(|| self.teams.first_file(id).map(|b| b.to_vec())).ok_or_else(|| format!("The trainer {id} has no team."))?;
        let team = Team::from_bytes(trainer.data_type, trainer.pokemon_count as usize, &team_bytes)?;
        Ok((trainer, team))
    }

    /// What the team has more than the files of the game (see [`BattleExtras`]): the file of the SD card, or nothing. The extras have one
    /// entry for each Pokémon of the team.
    pub fn extras(&self, sd: Option<&SdCard>, id: usize, count: usize) -> BattleExtras {
        let mut extras = sd.and_then(|sd| sd.read(&path(EXTRA_FOLDER, id))).and_then(|bytes| BattleExtras::from_bytes(&bytes).ok()).unwrap_or_else(|| BattleExtras::none(count));
        extras.pokemon.resize(count, data::PokemonExtras::none());
        extras
    }

    /// The messages of a trainer: the kind and the line of the text file.
    pub fn messages(&self, id: usize) -> Vec<(usize, usize)> {
        match (self.messages.first_file(0), self.messages.first_file(1)) {
            (Some(offsets), Some(table)) => data::message_lines(offsets, table, id),
            _ => Vec::new(),
        }
    }

    /// The file of a 3D model of the battles (the model and its textures), without the compression: the SD card first, then the game.
    pub fn model_file(&self, sd: Option<&SdCard>, number: u8) -> Option<Vec<u8>> {
        let id = number as usize * crate::model_view::FILES_PER_MODEL;
        if let Some(bytes) = sd.and_then(|sd| sd.read(&path(MODEL_FOLDER, id))) {
            return Some(if sango_core::lz::is_compressed(&bytes) { sango_core::lz::decompress(&bytes).ok()? } else { bytes });
        }
        self.models.first_file_decompressed(id)
    }

    /// The model of the game, without the compression.
    pub fn game_model_file(&self, number: u8) -> Option<Vec<u8>> {
        self.models.first_file_decompressed(number as usize * crate::model_view::FILES_PER_MODEL)
    }

    /// True when the SD card has a model for this number.
    pub fn model_changed(sd: Option<&SdCard>, number: u8) -> bool {
        sd.is_some_and(|sd| sd.read(&path(MODEL_FOLDER, number as usize * crate::model_view::FILES_PER_MODEL)).is_some())
    }

    /// Replaces a model by the one in a glTF or GLB file. The materials come from the model of the game; the motions stay as they are.
    pub fn import_model(&self, sd: &SdCard, number: u8, file: &std::path::Path, scale: f64) -> Result<(), String> {
        let original = self.game_model_file(number).ok_or("The model of the game is not in the archive.")?;
        let source = sango_modelport::gltf::read(file)?;
        // The skeleton of the original model stays: the motions of the game move its joints by name.
        let bones = sango_core::model::parse_model(&original)?.bones;
        let joints: Vec<sango_modelport::skeleton::Joint> = bones
            .iter()
            .map(|b| sango_modelport::skeleton::Joint { name: b.name.clone(), parent: b.parent.and_then(|p| bones.get(p)).map(|p| p.name.clone()).unwrap_or_default(), scale: b.scale, rot: b.rotation.map(f64::from), trans: b.translation })
            .collect();
        let made = sango_modelport::trainer::trainer_model(&source, &original, &joints, scale)?;
        // The file has the compression of the files of the archive: the plugin gives it to the game as it is.
        sd.write(&path(MODEL_FOLDER, number as usize * crate::model_view::FILES_PER_MODEL), &sango_core::lz::compress_lz11(&made))?;
        Ok(())
    }

    /// Gives back the model of the game.
    pub fn reset_model(&self, sd: &SdCard, number: u8) -> Result<(), String> {
        sd.remove(&path(MODEL_FOLDER, number as usize * crate::model_view::FILES_PER_MODEL))?;
        Ok(())
    }

    /// A picture of the trainers: the SD card first, then the game. Gives the size and the RGBA pixels.
    pub fn picture(&self, sd: Option<&SdCard>, file: usize) -> Result<(usize, usize, Vec<u8>), String> {
        let bytes = sd.and_then(|sd| sd.read(&path(PICTURE_FOLDER, file))).or_else(|| self.pictures.first_file(file).map(|b| b.to_vec())).ok_or_else(|| format!("The picture {file} is not there."))?;
        let plain = if sango_core::lz::is_compressed(&bytes) { sango_core::lz::decompress(&bytes)? } else { bytes };
        let image = Clim::parse(&plain)?;
        Ok((image.width(), image.height(), image.to_rgba()?))
    }

    /// The size of a picture in the game: an imported picture has the same size.
    pub fn game_picture_size(&self, file: usize) -> Option<(usize, usize)> {
        let bytes = self.pictures.first_file(file)?;
        let plain = if sango_core::lz::is_compressed(bytes) { sango_core::lz::decompress(bytes).ok()? } else { bytes.to_vec() };
        let image = Clim::parse(&plain).ok()?;
        Some((image.width(), image.height()))
    }

    /// Writes a picture on the SD card, in the format of the game (RGBA8, compressed).
    pub fn save_picture(&self, sd: &SdCard, file: usize, width: usize, height: usize, rgba: &[u8]) -> Result<(), String> {
        let image = Clim::from_rgba8(width, height, rgba)?;
        sd.write(&path(PICTURE_FOLDER, file), &sango_core::lz::compress_lz11(&image.to_bytes()))?;
        Ok(())
    }

    /// Gives back a picture of the game.
    pub fn reset_picture(&self, sd: &SdCard, file: usize) -> Result<(), String> {
        sd.remove(&path(PICTURE_FOLDER, file))?;
        Ok(())
    }

    /// True when the SD card has a picture.
    pub fn picture_changed(sd: Option<&SdCard>, file: usize) -> bool {
        sd.is_some_and(|sd| sd.read(&path(PICTURE_FOLDER, file)).is_some())
    }

    /// Writes the extras of a team: the file goes when they change nothing.
    pub fn save_extras(&self, sd: &SdCard, id: usize, extras: &BattleExtras) -> Result<(), String> {
        if extras.is_empty() {
            sd.remove(&path(EXTRA_FOLDER, id))?;
        } else {
            sd.write(&path(EXTRA_FOLDER, id), &extras.to_bytes())?;
        }
        Ok(())
    }

    /// The data of a class: the SD card first, then the game.
    pub fn class(&self, sd: Option<&SdCard>, id: usize) -> Result<TrainerClass, String> {
        let bytes = sd.and_then(|sd| sd.read(&path(CLASS_FOLDER, id))).or_else(|| self.classes.first_file(id).map(|b| b.to_vec())).ok_or_else(|| format!("The class {id} has no data."))?;
        TrainerClass::from_bytes(&bytes)
    }

    /// The data and the team of a trainer in the game.
    #[cfg(test)]
    pub fn game_trainer(&self, id: usize) -> Result<(TrainerData, Team), String> {
        self.trainer(None, id)
    }

    /// Writes the data and the team of a trainer on the SD card. The number of Pokémon and the form of the team are the ones of the team.
    pub fn save_trainer(&self, sd: &SdCard, id: usize, trainer: &TrainerData, team: &Team) -> Result<(), String> {
        if team.pokemon.is_empty() || team.pokemon.len() > data::MAX_TEAM {
            return Err(format!("A team has 1 to {} Pokémon.", data::MAX_TEAM));
        }
        let mut trainer = *trainer;
        trainer.data_type = team.data_type();
        trainer.pokemon_count = team.pokemon.len() as u8;
        sd.write(&path(TEAM_FOLDER, id), &team.to_bytes())?;
        sd.write(&path(DATA_FOLDER, id), &trainer.to_bytes())?;
        Ok(())
    }

    /// Writes the data of a class on the SD card.
    pub fn save_class(&self, sd: &SdCard, id: usize, class: &TrainerClass) -> Result<(), String> {
        sd.write(&path(CLASS_FOLDER, id), &class.to_bytes())?;
        Ok(())
    }

    /// Gives back the data and the team of a trainer of the game.
    pub fn reset_trainer(&self, sd: &SdCard, id: usize) -> Result<(), String> {
        sd.remove(&path(DATA_FOLDER, id))?;
        sd.remove(&path(TEAM_FOLDER, id))?;
        sd.remove(&path(EXTRA_FOLDER, id))?;
        Ok(())
    }

    /// Gives back the data of a class of the game.
    pub fn reset_class(&self, sd: &SdCard, id: usize) -> Result<(), String> {
        sd.remove(&path(CLASS_FOLDER, id))?;
        Ok(())
    }

    /// The numbers of the trainers that the SD card changes.
    pub fn changed_trainers(sd: Option<&SdCard>) -> HashSet<usize> {
        let Some(sd) = sd else { return HashSet::new() };
        [DATA_FOLDER, TEAM_FOLDER, EXTRA_FOLDER].iter().flat_map(|folder| sd.files_in(folder)).filter_map(|found| number_of(&found)).collect()
    }

    /// The numbers of the classes that the SD card changes.
    pub fn changed_classes(sd: Option<&SdCard>) -> HashSet<usize> {
        let Some(sd) = sd else { return HashSet::new() };
        sd.files_in(CLASS_FOLDER).iter().filter_map(|found| number_of(found)).collect()
    }

    /// A file of texts in a language: the SD card first, then the game.
    pub fn text_file(&mut self, sd: Option<&SdCard>, language: usize, file: usize) -> Option<&TextFile> {
        if !self.texts.contains_key(&(language, file)) {
            let from_card = sd.and_then(|sd| sd.read(&text::sd_path(language, file))).and_then(|bytes| TextFile::from_bytes(&bytes).ok());
            let loaded = from_card.or_else(|| {
                let archive = self.languages.entry(language).or_insert_with(|| Garc::open(&self.dump.archive(LANGUAGES[language].archive)).ok());
                archive.as_ref().and_then(|archive| archive.first_file(file)).and_then(|bytes| TextFile::from_bytes(bytes).ok())
            });
            self.texts.insert((language, file), loaded);
        }
        self.texts.get(&(language, file)).and_then(|file| file.as_ref())
    }

    /// A line of a file of texts, or an empty text.
    pub fn line(&mut self, sd: Option<&SdCard>, language: usize, file: usize, line: usize) -> String {
        self.text_file(sd, language, file).and_then(|file| file.text(line)).unwrap_or_default()
    }

    /// Changes a line of a text file in a language and writes the file on the SD card.
    pub fn set_line(&mut self, sd: &SdCard, language: usize, file: usize, line: usize, value: &str) -> Result<(), String> {
        let mut changed = self.text_file(Some(sd), language, file).cloned().ok_or("The file of texts is not there.")?;
        changed.set_text(line, value)?;
        sd.write(&text::sd_path(language, file), &changed.to_bytes())?;
        self.texts.insert((language, file), Some(changed));
        Ok(())
    }

    /// Forgets the texts that were read (after a reset or an import).
    pub fn forget_texts(&mut self) {
        self.texts.clear();
    }
}

/// The number in the name of a file of a folder: `trainer/data/12.bin` gives 12.
fn number_of(found: &str) -> Option<usize> {
    found.rsplit('/').next()?.strip_suffix(".bin")?.parse().ok()
}
