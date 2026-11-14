//! A species pack: the species and the forms that a user can install, in one `.sangopack` file (a ZIP file).
//!
//! The file has one entry, `manifest.json`. It holds the description of the pack and all the data of its species and forms.
//! The name of a species is its key: two species with the same name cannot be installed together.
//!
//! The 3D models are in the pack too: a model is the 8 files of a model of the game (`models/<name>/0.pc` to `7.pc`). A species
//! or a form names its model, or says that it uses the model of a species of the game. Studio gives the model a place in the
//! model files of the plugin when it installs the pack.

use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

/// The number of files of a model.
pub const MODEL_FILES: usize = 8;

/// The size of the data of a species, in bytes (the file `pokemon/data/<n>.bin` of the game).
pub const DATA_SIZE: usize = 0x50;

/// The most moves that a species of a pack learns by level. The plugin keeps this number of places.
pub const MAX_MOVES: usize = 24;

/// The most evolutions of a species.
pub const MAX_EVOLUTIONS: usize = 4;

/// The most characters in a name (the plugin keeps 24 places of 16 bits, with the zero at the end).
pub const MAX_NAME: usize = 23;

/// The file of the pack that holds everything.
pub const MANIFEST: &str = "manifest.json";

/// A species: a species of the game (its number), or a species of a pack (its name).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Ref {
    /// A species of the game, by its number.
    Game(u16),
    /// A species of a pack, by its name.
    Key(String),
}

impl Ref {
    /// A reference to a species of a pack.
    pub fn key(name: &str) -> Ref {
        Ref::Key(name.to_string())
    }
}

/// Where the model of a species or a form comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelRef {
    /// The model of a species of the game, by its number.
    Native { native: u16 },
    /// A model of the pack: the folder `models/<file>` of the pack file.
    File { file: String },
}

/// A move that a species learns by level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub id: u16,
    pub level: u8,
}

/// An evolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evolution {
    /// The method (the numbers of the game: 4 is "Level Up").
    pub method: u8,
    /// The argument of the method (a level, an item...).
    pub arg: u16,
    /// What the species becomes.
    pub species: Ref,
}

/// A species that the game does not have.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackSpecies {
    /// The name in English. It is also the key of the species.
    pub name: String,
    /// The 3D model of the species.
    pub model: ModelRef,
    /// The 80 bytes of data of the species, as hexadecimal text.
    pub data: String,
    pub moves: Vec<Move>,
    pub evolutions: Vec<Evolution>,
}

/// A form of a species, with its own data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackForm {
    /// The species that has the form.
    pub species: Ref,
    /// The number of the form among the forms of the species (the game counts the normal form as 0).
    pub form: u16,
    /// The name of the form, for the menus. Can be empty.
    #[serde(default)]
    pub name: String,
    /// The 3D model of the form. Without one, the form shows the model that the game has for it.
    ///
    /// For a form of a species of the game, a model of the pack (`file`) is a new slot at the end of the list of the models of
    /// that species. For a form of a species of a pack, it is a slot of a carrier species.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// The second model of a form of a species of the game that has two models for the form (for example a female model):
    /// the slot after `model`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub female_model: Option<ModelRef>,
    /// The 80 bytes of data of the form, as hexadecimal text.
    pub data: String,
}

/// The description of a pack and its content.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pack {
    /// A short name without spaces that tells the pack apart, for example `gen7`.
    pub id: String,
    /// The name that Studio shows.
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    /// The credit of the models and of the data: Studio shows it before the installation.
    #[serde(default)]
    pub credit: String,
    #[serde(default)]
    pub species: Vec<PackSpecies>,
    #[serde(default)]
    pub forms: Vec<PackForm>,
    /// The file that the pack was read from: the models are read from it when they are needed.
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

/// The bytes of a text of hexadecimal digits.
pub fn from_hex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 {
        return Err("The hexadecimal text has an odd number of digits.".into());
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|_| format!("'{}' is not a hexadecimal number.", &text[at..at + 2]))).collect()
}

/// The hexadecimal text of some bytes.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl PackSpecies {
    /// The 80 bytes of data.
    pub fn data_bytes(&self) -> Result<Vec<u8>, String> {
        let bytes = from_hex(&self.data).map_err(|e| format!("{}: {e}", self.name))?;
        if bytes.len() != DATA_SIZE {
            return Err(format!("{}: the data has {} bytes, not {DATA_SIZE}.", self.name, bytes.len()));
        }
        Ok(bytes)
    }
}

impl PackForm {
    /// The 80 bytes of data.
    pub fn data_bytes(&self) -> Result<Vec<u8>, String> {
        let bytes = from_hex(&self.data).map_err(|e| format!("form {} of {:?}: {e}", self.form, self.species))?;
        if bytes.len() != DATA_SIZE {
            return Err(format!("form {} of {:?}: the data has {} bytes, not {DATA_SIZE}.", self.form, self.species, bytes.len()));
        }
        Ok(bytes)
    }
}

impl Pack {
    /// All the models that the species and the forms name, in the order of use.
    pub fn model_refs(&self) -> Vec<&ModelRef> {
        self.species.iter().map(|s| &s.model).chain(self.forms.iter().flat_map(|f| f.model.iter().chain(f.female_model.iter()))).collect()
    }

    /// True for a form that adds models to the list of a species of the game (not to a carrier species).
    pub fn extends_game_species(form: &PackForm) -> bool {
        matches!((&form.species, &form.model), (Ref::Game(_), Some(ModelRef::File { .. })))
    }

    /// The models that use the slots of the carrier species: the models of the species and of the forms of the species of
    /// packs, each one once, in the order of use.
    pub fn carrier_model_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let refs = self.species.iter().map(|s| &s.model).chain(self.forms.iter().filter(|f| !Pack::extends_game_species(f)).flat_map(|f| f.model.iter()));
        for model in refs {
            if let ModelRef::File { file } = model {
                if !names.contains(file) {
                    names.push(file.clone());
                }
            }
        }
        names
    }

    /// The names of the models of the pack (the folders of `models/`), each one once, in the order of use.
    pub fn model_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for model in self.model_refs() {
            if let ModelRef::File { file } = model {
                if !names.contains(file) {
                    names.push(file.clone());
                }
            }
        }
        names
    }

    /// The names of the species of other packs that this pack uses (in evolutions and forms).
    pub fn needed_names(&self) -> Vec<String> {
        let own: std::collections::HashSet<&str> = self.species.iter().map(|s| s.name.as_str()).collect();
        let mut found = Vec::new();
        let refs = self.species.iter().flat_map(|s| s.evolutions.iter().map(|e| &e.species)).chain(self.forms.iter().map(|f| &f.species));
        for reference in refs {
            if let Ref::Key(name) = reference {
                if !own.contains(name.as_str()) && !found.contains(name) {
                    found.push(name.clone());
                }
            }
        }
        found
    }

    /// Checks everything that can be checked inside the pack: the sizes, the names, the references.
    pub fn check(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
            return Err("The id of the pack must have letters, digits, '-' or '_'.".into());
        }
        let mut names = std::collections::HashSet::new();
        for species in &self.species {
            if species.name.is_empty() || species.name.chars().count() > MAX_NAME {
                return Err(format!("The name '{}' must have 1 to {MAX_NAME} characters.", species.name));
            }
            if species.name.parse::<u16>().is_ok() {
                return Err(format!("The name '{}' is a number: a name cannot be a number.", species.name));
            }
            if !names.insert(species.name.as_str()) {
                return Err(format!("The species '{}' is two times in the pack.", species.name));
            }
            species.data_bytes()?;
            if species.moves.len() > MAX_MOVES {
                return Err(format!("{}: {} moves, {MAX_MOVES} at most.", species.name, species.moves.len()));
            }
            if species.evolutions.len() > MAX_EVOLUTIONS {
                return Err(format!("{}: {} evolutions, {MAX_EVOLUTIONS} at most.", species.name, species.evolutions.len()));
            }
        }
        for model in self.model_refs() {
            match model {
                ModelRef::Native { native } if !(1..=721).contains(native) => return Err(format!("The model of the species {native} is not a model of the game.")),
                ModelRef::File { file } if file.is_empty() || file.chars().count() > 60 || file.chars().any(|c| c == '/' || c == '\\' || c.is_control()) => {
                    return Err(format!("The name of the model '{file}' must have 1 to 60 characters, without slashes."));
                }
                _ => {}
            }
        }
        // A name may belong to a species of another pack: the installation looks for it. A number must be a species of the game.
        let known = |reference: &Ref| match reference {
            Ref::Game(number) => (1..=721).contains(number),
            Ref::Key(_) => true,
        };
        for species in &self.species {
            for evolution in &species.evolutions {
                if !known(&evolution.species) {
                    return Err(format!("{}: the evolution goes to {:?}, which is not a species of the game.", species.name, evolution.species));
                }
            }
        }
        for form in &self.forms {
            if !known(&form.species) {
                return Err(format!("A form belongs to {:?}, which is not a species of the game.", form.species));
            }
            form.data_bytes()?;
            if form.female_model.is_some() && !Pack::extends_game_species(form) {
                return Err(format!("The form {} of {:?} has a second model: only a form of a species of the game with a model of the pack can have one.", form.form, form.species));
            }
            if matches!(form.female_model, Some(ModelRef::Native { .. })) {
                return Err(format!("The second model of the form {} of {:?} must be a model of the pack.", form.form, form.species));
            }
        }
        Ok(())
    }

    /// The text of `manifest.json`.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Reads the text of `manifest.json` and checks the pack.
    pub fn from_json(text: &str) -> Result<Pack, String> {
        let pack: Pack = serde_json::from_str(text).map_err(|e| format!("manifest.json: {e}"))?;
        pack.check()?;
        Ok(pack)
    }

    /// The bytes of a `.sangopack` file without models. Only a pack that has no model of its own can be written like this.
    pub fn to_zip(&self) -> Result<Vec<u8>, String> {
        let mut out = std::io::Cursor::new(Vec::new());
        self.write_zip(&mut out, &mut |name| Err(format!("The model '{name}' has no files.")))?;
        Ok(out.into_inner())
    }

    /// Writes the pack as a ZIP file. `model` gives the 8 files of a model of the pack by its name.
    pub fn write_zip<W: Write + Seek>(&self, writer: W, model: &mut dyn FnMut(&str) -> Result<Vec<Vec<u8>>, String>) -> Result<(), String> {
        let mut zip = zip::ZipWriter::new(writer);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
        zip.start_file(MANIFEST, options).map_err(|e| e.to_string())?;
        zip.write_all(self.to_json()?.as_bytes()).map_err(|e| e.to_string())?;
        for name in self.model_names() {
            let files = model(&name)?;
            if files.len() != MODEL_FILES {
                return Err(format!("The model '{name}' has {} files, not {MODEL_FILES}.", files.len()));
            }
            for (place, bytes) in files.iter().enumerate() {
                // The files of a model are packs that do not shrink: they are stored.
                let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
                zip.start_file(format!("models/{name}/{place}.pc"), stored).map_err(|e| e.to_string())?;
                zip.write_all(bytes).map_err(|e| e.to_string())?;
            }
        }
        zip.finish().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Reads the bytes of a `.sangopack` file (the manifest only).
    pub fn from_zip(bytes: &[u8]) -> Result<Pack, String> {
        Pack::read_manifest(std::io::Cursor::new(bytes))
    }

    fn read_manifest<R: Read + Seek>(reader: R) -> Result<Pack, String> {
        let mut zip = zip::ZipArchive::new(reader).map_err(|e| format!("This is not a pack: {e}"))?;
        let mut text = String::new();
        zip.by_name(MANIFEST).map_err(|_| format!("This pack has no {MANIFEST}."))?.read_to_string(&mut text).map_err(|e| e.to_string())?;
        Pack::from_json(&text)
    }

    /// Writes a pack without models in a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_zip()?).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Writes the pack in a file, with its models. `model` gives the 8 files of a model by its name.
    pub fn save_with_models(&self, path: &Path, model: &mut dyn FnMut(&str) -> Result<Vec<Vec<u8>>, String>) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.write_zip(std::io::BufWriter::new(file), model)
    }

    /// Opens a pack file: reads the manifest and remembers the file, for the models.
    pub fn open(path: &Path) -> Result<Pack, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut pack = Pack::read_manifest(std::io::BufReader::new(file))?;
        pack.file = Some(path.to_path_buf());
        Ok(pack)
    }

    /// Reads the 8 files of a model of the pack from the pack file.
    pub fn read_model(&self, name: &str) -> Result<Vec<Vec<u8>>, String> {
        let path = self.file.as_ref().ok_or_else(|| format!("The pack '{}' was not read from a file.", self.id))?;
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
        (0..MODEL_FILES)
            .map(|place| {
                let mut bytes = Vec::new();
                zip.by_name(&format!("models/{name}/{place}.pc"))
                    .map_err(|_| format!("The pack '{}' has no file {place} for the model '{name}'.", self.id))?
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                Ok(bytes)
            })
            .collect()
    }
}
