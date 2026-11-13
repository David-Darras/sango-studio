//! The installed packs: the registry of numbers and the files that the plugin reads.
//!
//! On the SD card, in `sango-studio/species/`:
//!
//! | File | Content |
//! | --- | --- |
//! | `registry.json` | the number of each species, in the order of installation, and the forms |
//! | `packs/<id>.sangopack` | a copy of each installed pack |
//! | `table.bin` | the table that the plugin reads (see [`crate::table`]) |
//!
//! A number never changes. A removed pack keeps its places in the registry: the species and the forms of the pack stay in the
//! table as empty entries, so the numbers of the other species and the save files stay valid. Installing the same pack
//! again gives its numbers back.

use crate::pack::{Pack, Ref};
use crate::table::{FormRecord, SpeciesRecord, Table, FIRST_SPECIES};
use sango_core::SdCard;
use serde::{Deserialize, Serialize};

/// The folder of the species files, in the folder of the studio on the SD card.
pub const FOLDER: &str = "species";
pub const REGISTRY_PATH: &str = "species/registry.json";
pub const TABLE_PATH: &str = "species/table.bin";

/// The most species and forms that the plugin can hold. A pack that goes over is refused.
pub const MAX_SPECIES: usize = 384;
pub const MAX_FORMS: usize = 256;

fn pack_path(id: &str) -> String {
    format!("{FOLDER}/packs/{id}.sangopack")
}

/// A pack that is installed (or was installed).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPack {
    pub id: String,
    pub name: String,
    pub version: String,
    /// False for a pack that was removed: its places stay reserved.
    pub active: bool,
}

/// The place of a species: its number is `FIRST_SPECIES` and its position in the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredSpecies {
    pub name: String,
    pub pack: String,
}

/// The place of a form, in the order that the plugin adds the forms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredForm {
    pub species: Ref,
    pub form: u16,
    pub pack: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub packs: Vec<InstalledPack>,
    pub species: Vec<RegisteredSpecies>,
    pub forms: Vec<RegisteredForm>,
}

/// The registry and the packs that it names.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub registry: Registry,
    /// The packs, whether active or not (the data of a removed pack is kept until the user deletes it for good).
    pub packs: Vec<Pack>,
}

impl Library {
    /// Reads the registry and the packs from the SD card. An empty library when nothing is installed.
    pub fn open(sd: &SdCard) -> Result<Library, String> {
        let Some(bytes) = sd.read(REGISTRY_PATH) else { return Ok(Library::default()) };
        let registry: Registry = serde_json::from_slice(&bytes).map_err(|e| format!("{REGISTRY_PATH}: {e}"))?;
        let mut packs = Vec::new();
        for installed in &registry.packs {
            let bytes = sd.read(&pack_path(&installed.id)).ok_or_else(|| format!("The pack '{}' is in the registry but its file is missing.", installed.id))?;
            packs.push(Pack::from_zip(&bytes).map_err(|e| format!("{}: {e}", installed.id))?);
        }
        Ok(Library { registry, packs })
    }

    /// The number of a species of a pack, by its name.
    pub fn number_of(&self, name: &str) -> Option<u16> {
        self.registry.species.iter().position(|s| s.name == name).map(|place| FIRST_SPECIES + place as u16)
    }

    fn resolve(&self, reference: &Ref) -> Option<u16> {
        match reference {
            Ref::Game(number) => Some(*number),
            Ref::Key(name) => self.number_of(name),
        }
    }

    fn is_active(&self, pack_id: &str) -> bool {
        self.registry.packs.iter().any(|p| p.id == pack_id && p.active)
    }

    /// Installs a pack: gives numbers to its species and its forms. The numbers of a pack that was installed before come back.
    /// Nothing changes when the pack is refused.
    pub fn install(&mut self, pack: Pack) -> Result<(), String> {
        pack.check()?;
        if self.is_active(&pack.id) {
            return Err(format!("The pack '{}' is already installed.", pack.id));
        }
        for name in pack.needed_names() {
            if self.registry.species.iter().all(|s| s.name != name || !self.is_active(&s.pack)) {
                return Err(format!("The pack '{}' needs the species '{name}': install the pack that has it first.", pack.id));
            }
        }
        let mut registry = self.registry.clone();
        for species in &pack.species {
            match registry.species.iter().find(|s| s.name == species.name) {
                Some(found) if found.pack == pack.id => {}
                Some(found) => return Err(format!("The species '{}' is already installed by the pack '{}'.", species.name, found.pack)),
                None => registry.species.push(RegisteredSpecies { name: species.name.clone(), pack: pack.id.clone() }),
            }
        }
        for form in &pack.forms {
            let known = registry.forms.iter().any(|f| f.pack == pack.id && f.species == form.species && f.form == form.form);
            if !known {
                registry.forms.push(RegisteredForm { species: form.species.clone(), form: form.form, pack: pack.id.clone() });
            }
        }
        if registry.species.len() > MAX_SPECIES || registry.forms.len() > MAX_FORMS {
            return Err(format!("The plugin holds {MAX_SPECIES} species and {MAX_FORMS} forms at most: this pack does not fit."));
        }
        match registry.packs.iter_mut().find(|p| p.id == pack.id) {
            Some(found) => {
                found.active = true;
                found.name = pack.name.clone();
                found.version = pack.version.clone();
            }
            None => registry.packs.push(InstalledPack { id: pack.id.clone(), name: pack.name.clone(), version: pack.version.clone(), active: true }),
        }
        self.packs.retain(|p| p.id != pack.id);
        self.packs.push(pack);
        self.registry = registry;
        Ok(())
    }

    /// Removes a pack. Its places in the registry stay reserved.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        match self.registry.packs.iter_mut().find(|p| p.id == id && p.active) {
            Some(found) => {
                found.active = false;
                Ok(())
            }
            None => Err(format!("The pack '{id}' is not installed.")),
        }
    }

    /// The table for the plugin. A species or a form of a pack that is not active is an empty entry.
    pub fn table(&self) -> Result<Table, String> {
        let mut table = Table::default();
        for (place, registered) in self.registry.species.iter().enumerate() {
            let id = FIRST_SPECIES + place as u16;
            let found = self.is_active(&registered.pack).then(|| self.packs.iter().find(|p| p.id == registered.pack)).flatten().and_then(|pack| pack.species.iter().find(|s| s.name == registered.name));
            table.species.push(match found {
                Some(species) => SpeciesRecord {
                    id,
                    model_form: species.model_form,
                    name: species.name.clone(),
                    data: species.data_bytes()?,
                    moves: species.moves.iter().map(|m| (m.id, m.level)).collect(),
                    // An evolution to a species that is not there any more is dropped.
                    evolutions: species.evolutions.iter().filter_map(|e| Some((e.method, e.arg, self.resolve(&e.species)?))).collect(),
                },
                None => SpeciesRecord { id, model_form: 0, name: String::new(), data: vec![0; crate::pack::DATA_SIZE], moves: Vec::new(), evolutions: Vec::new() },
            });
        }
        for registered in &self.registry.forms {
            let found = self.is_active(&registered.pack).then(|| self.packs.iter().find(|p| p.id == registered.pack)).flatten().and_then(|pack| pack.forms.iter().find(|f| f.species == registered.species && f.form == registered.form));
            let species = self.resolve(&registered.species).unwrap_or(0);
            table.forms.push(match found {
                Some(form) => FormRecord { species, form: form.form, model_form: form.model_form, data: form.data_bytes()? },
                None => FormRecord { species, form: registered.form, model_form: 0, data: vec![0; crate::pack::DATA_SIZE] },
            });
        }
        Ok(table)
    }

    /// Writes the registry, the packs and the table on the SD card, and tells the plugin.
    pub fn save(&self, sd: &SdCard) -> Result<(), String> {
        let table = self.table()?.to_bytes()?;
        for pack in &self.packs {
            sd.write(&pack_path(&pack.id), &pack.to_zip()?)?;
        }
        sd.write(REGISTRY_PATH, serde_json::to_string_pretty(&self.registry).map_err(|e| e.to_string())?.as_bytes())?;
        sd.write(TABLE_PATH, &table)?;
        sd.mark_changed(TABLE_PATH)
    }
}
