//! The installed packs: the registry of numbers and the files that the plugin reads.
//!
//! On the SD card, in `sango-studio/species/`:
//!
//! | File | Content |
//! | --- | --- |
//! | `registry.json` | the number of each species, in the order of installation, and the forms |
//! | `packs/<id>.sangopack` | a copy of each installed pack, with its models |
//! | `table.bin` | the table that the plugin reads (see [`crate::table`]) |
//!
//! The models of the packs go to `sango/pokemodel` of the SD card (beside `sango-studio`), in the form that the plugin reads:
//! see [`Library::write_models`].
//!
//! A number never changes. A removed pack keeps its places in the registry: the species and the forms of the pack stay in the
//! table as empty entries, so the numbers of the other species and the save files stay valid. Installing the same pack
//! again gives its numbers back.

use crate::pack::{ModelRef, ModelReplacement, Pack, Ref, MODEL_FILES};
use crate::table::{FormRecord, SpeciesRecord, Table, FIRST_SPECIES};
use sango_core::{Dump, Garc, SdCard};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The folder of the species files, in the folder of the studio on the SD card.
pub const FOLDER: &str = "species";
pub const REGISTRY_PATH: &str = "species/registry.json";
pub const TABLE_PATH: &str = "species/table.bin";

/// The most species and forms that the plugin can hold. A pack that goes over is refused.
pub const MAX_SPECIES: usize = 384;
pub const MAX_FORMS: usize = 256;

/// The species whose model slots hold the models of the packs (the *carriers*): Victini and Volcanion. A carrier has the form
/// 0 (its own model) and the forms 1 to 254: the list of the models of the game keeps the number of slots in one byte, so a
/// carrier has 255 slots at most. The second carrier is the form number 256 and more in `model_form`.
pub const CARRIERS: [u16; 2] = [494, 721];
/// The models that a carrier holds besides its own model.
pub const MODELS_PER_CARRIER: usize = 254;
/// The most models that the packs can hold.
pub const MAX_MODELS: usize = MODELS_PER_CARRIER * CARRIERS.len();
/// The archive of the models of the Pokémon.
pub const MODEL_ARCHIVE: usize = 8;
/// The list of the models (the file 0 of the archive) has 722 entries of 4 bytes for the species (first slot u16, number of
/// slots u8, flags u8), then 2 bytes for each slot (flags u8, `sub_from` u8). A slot that has no entry reads what follows
/// the list in memory, so every new slot needs its entry.
const SPECIES_ENTRIES: usize = 722;

/// The `model_form` of the model that has this place in the list of the models of the registry: the carrier and the form of
/// the carrier, as the plugin reads them (1 to 254 for the first carrier, 257 to 510 for the second).
pub fn carrier_form(index: usize) -> u16 {
    ((index / MODELS_PER_CARRIER) * 256 + index % MODELS_PER_CARRIER + 1) as u16
}

/// The pack that holds the replacements of the models of the species of the game.
pub const REPLACEMENT_PACK: &str = "studio-models";

/// The ending of the name of an imported model: the model `x` that the pack shipped is replaced by `x+import`.
pub const IMPORTED_SUFFIX: &str = "+import";

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
    /// The slots that the form owns at the end of the list of the models of its species of the game: 0 for the other forms, 1 or
    /// 2 for a form that has its own model(s). The slots stay reserved when the pack is removed.
    #[serde(default)]
    pub slots: u8,
}

/// The place of a model of a pack: its position in the list gives its place in the model slots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredModel {
    pub name: String,
    pub pack: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub packs: Vec<InstalledPack>,
    pub species: Vec<RegisteredSpecies>,
    pub forms: Vec<RegisteredForm>,
    #[serde(default)]
    pub models: Vec<RegisteredModel>,
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
            let path = sd.path(&pack_path(&installed.id))?;
            if !path.is_file() {
                return Err(format!("The pack '{}' is in the registry but its file is missing.", installed.id));
            }
            packs.push(Pack::open(&path).map_err(|e| format!("{}: {e}", installed.id))?);
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

    /// Installs a pack: gives numbers to its species and its forms. The numbers of a pack that was installed before come back,
    /// and a pack that is installed again (a new version of it) keeps its numbers and replaces its data.
    /// Nothing changes when the pack is refused.
    pub fn install(&mut self, pack: Pack) -> Result<(), String> {
        pack.check()?;
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
            let slots = if Pack::extends_game_species(form) { 1 + u8::from(form.female_model.is_some()) } else { 0 };
            match registry.forms.iter_mut().find(|f| f.pack == pack.id && f.species == form.species && f.form == form.form) {
                Some(known) => known.slots = slots,
                None => registry.forms.push(RegisteredForm { species: form.species.clone(), form: form.form, pack: pack.id.clone(), slots }),
            }
        }
        for name in pack.carrier_model_names() {
            if !registry.models.iter().any(|m| m.pack == pack.id && m.name == name) {
                registry.models.push(RegisteredModel { name, pack: pack.id.clone() });
            }
        }
        if registry.species.len() > MAX_SPECIES || registry.forms.len() > MAX_FORMS || registry.models.len() > MAX_MODELS {
            return Err(format!("The plugin holds {MAX_SPECIES} species, {MAX_FORMS} forms and {MAX_MODELS} models at most: this pack does not fit."));
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

    /// Installs several packs. A pack that needs a species of another pack waits until that pack is installed, so the order of
    /// the list does not matter for the dependencies (the order of the others is kept: it gives the numbers). Returns the
    /// result for each pack, in the order in which the packs were tried.
    pub fn install_many(&mut self, packs: Vec<Pack>) -> Vec<(String, Result<(), String>)> {
        let mut results = Vec::new();
        let mut waiting = packs;
        while !waiting.is_empty() {
            let before = waiting.len();
            let mut later = Vec::new();
            let mut last_error = String::new();
            for pack in waiting {
                match self.install(pack.clone()) {
                    Ok(()) => results.push((pack.id.clone(), Ok(()))),
                    Err(error) if error.contains("needs the species") => {
                        last_error = error;
                        later.push(pack);
                    }
                    Err(error) => results.push((pack.id.clone(), Err(error))),
                }
            }
            if later.len() == before {
                for pack in later {
                    results.push((pack.id.clone(), Err(last_error.clone())));
                }
                break;
            }
            waiting = later;
        }
        results
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

    /// Replaces the model of a species of a pack (`form` 0) or of a form of a pack by an imported model, or puts the model that the pack
    /// shipped back (`imported` is None). The pack file on the SD card is written again, with the original model kept in it.
    ///
    /// The 8 files of the imported model are the ones that the importer makes. Nothing changes when this returns an error.
    pub fn set_model(&mut self, sd: &SdCard, species: &Ref, form: u16, imported: Option<Vec<Vec<u8>>>) -> Result<(), String> {
        if let Some(files) = &imported {
            if files.len() != MODEL_FILES {
                return Err(format!("A model has {MODEL_FILES} files, not {}.", files.len()));
            }
        }
        let place = self
            .packs
            .iter()
            .position(|pack| {
                self.is_active(&pack.id)
                    && match (species, form) {
                        (Ref::Key(name), 0) => pack.species.iter().any(|s| s.name == *name),
                        _ => pack.forms.iter().any(|f| f.species == *species && f.form == form),
                    }
            })
            .ok_or("No installed pack owns this model.")?;
        let old = self.packs[place].clone();
        let mut pack = old.clone();
        let current = match (species, form) {
            (Ref::Key(name), 0) => pack.species.iter_mut().find(|s| s.name == *name).map(|s| &mut s.model),
            _ => pack.forms.iter_mut().find(|f| f.species == *species && f.form == form).and_then(|f| f.model.as_mut()),
        };
        let Some(ModelRef::File { file }) = current else { return Err("This model is not a model of the pack: only the models of the species and the forms of the packs can be replaced.".into()) };
        let base = file.strip_suffix(IMPORTED_SUFFIX).unwrap_or(file).to_string();
        let replaced = format!("{base}{IMPORTED_SUFFIX}");
        let mut new_files: Option<(String, Vec<Vec<u8>>)> = None;
        match imported {
            Some(files) => {
                if replaced.chars().count() > 60 {
                    return Err(format!("The name of the model '{base}' is too long to be replaced."));
                }
                *file = replaced.clone();
                new_files = Some((replaced.clone(), files));
            }
            None => {
                if *file == base {
                    return Err("This is already the model of the pack.".into());
                }
                *file = base.clone();
            }
        }
        // The original stays in the file of the pack while no species and no form use it.
        pack.kept_models.retain(|name| *name != replaced);
        if !pack.model_refs().iter().any(|m| matches!(m, ModelRef::File { file } if *file == base)) && !pack.kept_models.contains(&base) {
            pack.kept_models.push(base);
        }
        let target = sd.path(&pack_path(&pack.id))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let temporary = target.with_extension("sangopack.new");
        pack.save_with_models(&temporary, &mut |name| match &new_files {
            Some((new_name, files)) if new_name == name => Ok(files.clone()),
            _ => old.read_model(name),
        })?;
        std::fs::rename(&temporary, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        pack.file = Some(target);
        let before = (self.registry.clone(), self.packs.clone());
        if let Err(message) = self.install(pack) {
            (self.registry, self.packs) = before;
            return Err(message);
        }
        Ok(())
    }

    /// Replaces a model of a species of the game (slot 0 is the normal model) by an imported one, or puts the model of the game
    /// back (`imported` is None). The replacements are kept in a pack of their own, `studio-models`, that this creates.
    pub fn replace_game_model(&mut self, sd: &SdCard, species: u16, slot: u8, imported: Option<Vec<Vec<u8>>>) -> Result<(), String> {
        if !(1..=721).contains(&species) || CARRIERS.contains(&species) {
            return Err(format!("The model of the species {species} cannot be replaced: Studio uses its model slots for the models of the packs."));
        }
        if let Some(files) = &imported {
            if files.len() != MODEL_FILES {
                return Err(format!("A model has {MODEL_FILES} files, not {}.", files.len()));
            }
        }
        let old = self.packs.iter().find(|p| p.id == REPLACEMENT_PACK).cloned();
        let mut pack = old.clone().unwrap_or_else(|| Pack { id: REPLACEMENT_PACK.into(), name: "My models".into(), version: "1".into(), ..Pack::default() });
        let name = format!("species-{species:04}-slot-{slot:02}");
        pack.replacements.retain(|r| !(r.species == species && r.slot == slot));
        let mut new_files = None;
        match imported {
            Some(files) => {
                pack.replacements.push(ModelReplacement { species, slot, model: name.clone() });
                new_files = Some(files);
            }
            None if old.as_ref().is_none_or(|p| !p.replacements.iter().any(|r| r.species == species && r.slot == slot)) => {
                return Err("This is already the model of the game.".into());
            }
            None => {}
        }
        let target = sd.path(&pack_path(&pack.id))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let temporary = target.with_extension("sangopack.new");
        pack.save_with_models(&temporary, &mut |model| match &new_files {
            Some(files) if model == name => Ok(files.clone()),
            _ => old.as_ref().ok_or_else(|| format!("The model '{model}' has no files.").to_string())?.read_model(model),
        })?;
        std::fs::rename(&temporary, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        pack.file = Some(target);
        let before = (self.registry.clone(), self.packs.clone());
        if let Err(message) = self.install(pack) {
            (self.registry, self.packs) = before;
            return Err(message);
        }
        Ok(())
    }

    /// The `model_form` of a model of a pack, as the plugin reads it.
    fn model_form(&self, pack: &str, model: Option<&ModelRef>) -> u16 {
        match model {
            None => 0,
            Some(ModelRef::Native { native }) => 0x8000 | native,
            Some(ModelRef::File { file }) => self.registry.models.iter().position(|m| m.pack == pack && m.name == *file).map_or(0, carrier_form),
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
                    model_form: self.model_form(&registered.pack, Some(&species.model)),
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
                // A form of a species of the game has its model in the list of that species, not in a carrier.
                Some(form) => FormRecord { species, form: form.form, model_form: if registered.slots > 0 { 0 } else { self.model_form(&registered.pack, form.model.as_ref()) }, data: form.data_bytes()? },
                None => FormRecord { species, form: registered.form, model_form: 0, data: vec![0; crate::pack::DATA_SIZE] },
            });
        }
        Ok(table)
    }

    /// Writes the registry, the packs and the table on the SD card, and tells the plugin. The file of each pack is copied to the
    /// SD card (with its models); a pack that has no file is written without models.
    pub fn save(&mut self, sd: &SdCard) -> Result<(), String> {
        let table = self.table()?.to_bytes()?;
        for pack in &mut self.packs {
            let target = sd.path(&pack_path(&pack.id))?;
            match pack.file.clone() {
                Some(source) if source == target => {}
                Some(source) => {
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                    }
                    std::fs::copy(&source, &target).map_err(|e| format!("{} to {}: {e}", source.display(), target.display()))?;
                    pack.file = Some(target);
                }
                None => {
                    sd.write(&pack_path(&pack.id), &pack.to_zip()?)?;
                    pack.file = Some(target);
                }
            }
        }
        sd.write(REGISTRY_PATH, serde_json::to_string_pretty(&self.registry).map_err(|e| e.to_string())?.as_bytes())?;
        sd.write(TABLE_PATH, &table)?;
        sd.mark_changed(TABLE_PATH)
    }

    /// Writes the models of the active packs in `sango/pokemodel` (in `sdmc`, the folder of the SD card), where the plugin
    /// reads them. Gives the number of models written.
    ///
    /// The plugin reads a model of a Pokémon as 8 files of the archive 8, and the file 0 of the archive is the list that
    /// tells, for each species, the place of its first model and the number of models. The new models are slots after the
    /// last slot of the game, in a block for each carrier species:
    ///
    /// - the list (`0.raw`) is the list of the game, with the first slot and the count of each carrier changed, and one entry
    ///   for each new slot (the entry of the own model of the carrier for its slot 0, zeros for the models of the packs),
    /// - the slot 0 of a carrier is its own model, copied from the archive, and the next slots are the models of the packs in
    ///   the order of the registry,
    /// - `<file>.pc` has the size of the file (4 bytes) and its data, and `index.bin` lists the files (number and size,
    ///   sorted by number).
    ///
    /// The folder is made again each time: the files that were there are removed. A model of a pack that was removed has
    /// its slot but no files.
    pub fn write_models(&self, sdmc: &Path, dump: &Dump) -> Result<usize, String> {
        let folder = sdmc.join("sango").join("pokemodel");
        let replacements: Vec<(&Pack, &ModelReplacement)> = self.packs.iter().filter(|p| self.is_active(&p.id)).flat_map(|p| p.replacements.iter().map(move |r| (p, r))).collect();
        if self.registry.models.is_empty() && self.registry.forms.iter().all(|f| f.slots == 0) && replacements.is_empty() {
            return Ok(0);
        }
        let archive = Garc::open(&dump.archive(MODEL_ARCHIVE)).map_err(|e| format!("Archive {MODEL_ARCHIVE}: {e}"))?;
        let mut list = archive.first_file(0).ok_or("The list of the models is missing in the archive.")?.to_vec();
        let entry = |list: &[u8], species: usize| -> Option<(usize, usize, u8)> {
            let at = species.checked_sub(1)? * 4;
            let bytes = list.get(at..at + 4)?;
            Some((u16::from_le_bytes([bytes[0], bytes[1]]) as usize, bytes[2] as usize, bytes[3]))
        };
        let per_slot = SPECIES_ENTRIES * 4;
        if list.len() < per_slot || (list.len() - per_slot) % 2 != 0 {
            return Err("The list of the models has a size that this program does not know.".into());
        }
        let slots = (list.len() - per_slot) / 2;
        let last = (1..=SPECIES_ENTRIES).filter_map(|species| entry(&list, species)).filter(|e| e.1 > 0).map(|(first, count, _)| first + count).max().ok_or("The list of the models is empty.")?;
        if last != slots {
            return Err(format!("The list of the models has {slots} slots but its species use {last}."));
        }
        let mut slot_entries = list[per_slot..].to_vec();
        list.truncate(per_slot);

        if folder.exists() {
            std::fs::remove_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let mut index: Vec<(u32, u32)> = Vec::new();
        let mut write = |id: usize, bytes: &[u8]| -> Result<(), String> {
            let mut data = (bytes.len() as u32).to_le_bytes().to_vec();
            data.extend_from_slice(bytes);
            let path = folder.join(format!("{id}.pc"));
            std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
            // A file that is written again (a replaced model over the model of the game) keeps one entry: the list of the plugin
            // looks a file up by its number and must find the size of the file that is on the SD card.
            match index.iter_mut().find(|entry| entry.0 == id as u32) {
                Some(entry) => entry.1 = bytes.len() as u32,
                None => index.push((id as u32, bytes.len() as u32)),
            }
            Ok(())
        };

        // The blocks of the carriers, one after the other after the slots of the game.
        let mut block_start = [0usize; CARRIERS.len()];
        let mut next = slots;
        for (place, species) in CARRIERS.iter().enumerate() {
            let used = self.registry.models.len().saturating_sub(place * MODELS_PER_CARRIER).min(MODELS_PER_CARRIER);
            let (first, count, flags) = entry(&list, *species as usize).ok_or("A carrier is not in the list of the models.")?;
            if count != 1 {
                return Err(format!("The carrier species {species} has {count} models: this program expects 1."));
            }
            if used == 0 {
                continue;
            }
            block_start[place] = next;
            let at = (*species as usize - 1) * 4;
            list[at..at + 2].copy_from_slice(&(next as u16).to_le_bytes());
            list[at + 2] = u8::try_from(1 + used).map_err(|_| "A carrier has too many models.".to_string())?;
            list[at + 3] = flags | 4;
            // The slot 0: the own model of the carrier, with its entry of the list. The models of the packs have zeros.
            slot_entries.resize(2 * (next + 1 + used), 0);
            let own = [slot_entries[2 * first], slot_entries[2 * first + 1]];
            slot_entries[2 * next..2 * next + 2].copy_from_slice(&own);
            for file in 0..MODEL_FILES {
                let bytes = archive.first_file(first * 8 + 3 + file).ok_or_else(|| format!("The model of the species {species} is cut."))?;
                write(next * 8 + 3 + file, bytes)?;
            }
            next += 1 + used;
        }

        // The species of the game that have forms with models of their own: the list of the species is moved to the end, with
        // its own models copied from the archive, and the models of the forms after them (one or two slots for each form, in the
        // order of the registry).
        let mut written = 0;
        let mut extended: Vec<u16> = Vec::new();
        for form in self.registry.forms.iter().filter(|f| f.slots > 0) {
            if let Ref::Game(number) = form.species {
                if !extended.contains(&number) {
                    extended.push(number);
                }
            }
        }
        for (_, replacement) in &replacements {
            if !extended.contains(&replacement.species) {
                extended.push(replacement.species);
            }
        }
        for species in extended {
            let (first, count, flags) = entry(&list, species as usize).ok_or_else(|| format!("The species {species} is not in the list of the models."))?;
            let owned: Vec<&RegisteredForm> = self.registry.forms.iter().filter(|f| f.slots > 0 && f.species == Ref::Game(species)).collect();
            let extra: usize = owned.iter().map(|f| f.slots as usize).sum();
            let total = count + extra;
            let start = next;
            slot_entries.resize(2 * (start + total), 0);
            for slot in 0..count {
                slot_entries.copy_within(2 * (first + slot)..2 * (first + slot) + 2, 2 * (start + slot));
                for file in 0..MODEL_FILES {
                    let bytes = archive.first_file((first + slot) * 8 + 3 + file).ok_or_else(|| format!("The model of the species {species} is cut."))?;
                    write((start + slot) * 8 + 3 + file, bytes)?;
                }
            }
            let at = (species as usize - 1) * 4;
            list[at..at + 2].copy_from_slice(&(start as u16).to_le_bytes());
            list[at + 2] = u8::try_from(total).map_err(|_| format!("The species {species} has too many models."))?;
            list[at + 3] = flags | 4;
            let mut offset = count;
            for registered in owned {
                let form = self.packs.iter().find(|p| p.id == registered.pack && self.is_active(&p.id)).and_then(|pack| pack.forms.iter().find(|f| f.species == registered.species && f.form == registered.form).map(|form| (pack, form)));
                if let Some((pack, form)) = form {
                    for (place, model) in [form.model.as_ref(), form.female_model.as_ref()].into_iter().flatten().enumerate() {
                        let ModelRef::File { file: name } = model else { continue };
                        for (file, bytes) in pack.read_model(name)?.iter().enumerate() {
                            write((start + offset + place) * 8 + 3 + file, bytes)?;
                        }
                        written += 1;
                    }
                }
                offset += registered.slots as usize;
            }
            // The models that replace the models of the game: they come last, so they win over the models of the forms.
            for (pack, replacement) in replacements.iter().filter(|(_, r)| r.species == species && (r.slot as usize) < total) {
                for (file, bytes) in pack.read_model(&replacement.model)?.iter().enumerate() {
                    write((start + replacement.slot as usize) * 8 + 3 + file, bytes)?;
                }
                written += 1;
            }
            next += total;
        }

        for (index_in_registry, model) in self.registry.models.iter().enumerate() {
            let Some(pack) = self.packs.iter().find(|p| p.id == model.pack && self.is_active(&p.id)) else { continue };
            // A model that the pack does not have any more (an imported model that was reset) keeps its slot, empty.
            if !pack.model_names().contains(&model.name) {
                continue;
            }
            let (carrier, form) = (index_in_registry / MODELS_PER_CARRIER, index_in_registry % MODELS_PER_CARRIER + 1);
            let files = pack.read_model(&model.name)?;
            for (file, bytes) in files.iter().enumerate() {
                write((block_start[carrier] + form) * 8 + 3 + file, bytes)?;
            }
            written += 1;
        }

        index.sort_unstable();
        let mut index_bytes = (index.len() as u32).to_le_bytes().to_vec();
        for (id, size) in &index {
            index_bytes.extend_from_slice(&id.to_le_bytes());
            index_bytes.extend_from_slice(&size.to_le_bytes());
        }
        list.extend_from_slice(&slot_entries);
        let mut raw = (list.len() as u32).to_le_bytes().to_vec();
        raw.extend_from_slice(&list);
        for (name, bytes) in [("index.bin", index_bytes), ("0.raw", raw)] {
            let path = folder.join(name);
            std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(written)
    }

    /// Writes everything: the registry, the packs and the table (see [`Library::save`]) and the models (see
    /// [`Library::write_models`]).
    pub fn apply(&mut self, sd: &SdCard, dump: &Dump) -> Result<usize, String> {
        self.save(sd)?;
        let sdmc = sd.root().parent().ok_or("The folder of the SD card has no parent.")?;
        self.write_models(sdmc, dump)
    }
}
