//! A species pack: the species and the forms that a user can install, in one `.sangopack` file (a ZIP file).
//!
//! The file has one entry, `manifest.json`. It holds the description of the pack and all the data of its species and forms.
//! The name of a species is its key: two species with the same name cannot be installed together.
//!
//! The models are not in the pack yet. A species has a *model form*: the number that the plugin uses to find the model in the
//! model slots (see [`PackSpecies::model_form`]).

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

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
    /// Where the model is: the form of the first carrier species, or 256 and more for the second carrier, or 0x8000 and the
    /// number of a species of the game for the model of that species.
    pub model_form: u16,
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
    /// Where the model is, as for a species.
    pub model_form: u16,
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

    /// The bytes of a `.sangopack` file.
    pub fn to_zip(&self) -> Result<Vec<u8>, String> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(MANIFEST, options).map_err(|e| e.to_string())?;
            zip.write_all(self.to_json()?.as_bytes()).map_err(|e| e.to_string())?;
            zip.finish().map_err(|e| e.to_string())?;
        }
        Ok(out.into_inner())
    }

    /// Reads the bytes of a `.sangopack` file.
    pub fn from_zip(bytes: &[u8]) -> Result<Pack, String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("This is not a pack: {e}"))?;
        let mut text = String::new();
        zip.by_name(MANIFEST).map_err(|_| format!("This pack has no {MANIFEST}."))?.read_to_string(&mut text).map_err(|e| e.to_string())?;
        Pack::from_json(&text)
    }

    /// Writes the pack in a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_zip()?).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Reads a pack from a file.
    pub fn open(path: &Path) -> Result<Pack, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Pack::from_zip(&bytes)
    }
}
