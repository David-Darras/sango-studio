//! Puts icons in a pack: PNG pictures of 40 × 30 pixels with a `manifest.json` (the folder that `home-icons` made).
//!
//! Studio writes the icons of the active packs on the SD card when it installs the packs (see `Library::write_icons`).
//! This module only fills the packs:
//!
//! - For a pack of species, the icon of each species of the pack (found by its name) goes in the pack.
//! - For the icons of the species of the game, a pack of its own holds the pictures that replace the files of the game: the file
//!   number comes from `icons::ICON_FILES` and `icon_files::FORMS` (the form 0 and the Mega Evolutions).

use crate::icon_files::{FEMALE, FORMS};
use crate::icons::ICON_FILES;
use sango_pack::{pack::PackIcon, Pack, Ref};
use serde::Deserialize;
use std::path::Path;

/// The id of the pack that holds the icons of the species of the game.
pub const GAME_ICON_PACK: &str = "home-icons";

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    credit: String,
    icons: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    dex: u16,
    species: String,
    variant: String,
    file: String,
}

/// What was put in a pack.
#[derive(Debug, Default)]
pub struct Report {
    /// The icons that were added.
    pub added: usize,
    /// The icons that were not used, with the reason.
    pub skipped: Vec<String>,
}

/// The name that Studio shows for an icon: `Venusaur` or `Venusaur (Mega)`.
fn label_of(entry: &Entry) -> String {
    if entry.variant.is_empty() {
        entry.species.clone()
    } else {
        format!("{} ({})", entry.species, entry.variant)
    }
}

fn key(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// The file of the game for a species of the game and a variant: `None` when the pack cannot replace it.
fn game_file(dex: u16, variant: &str) -> Option<u16> {
    // The females of Unfezant, Frillish, Jellicent and Pyroar have a file of their own (a table of the game).
    if variant == "Female" {
        if let Some(&(_, file)) = FEMALE.iter().find(|(number, _)| *number == dex) {
            return Some(file);
        }
        // Meowstic: the female is its form 1.
        return FORMS.iter().find(|(number, _)| *number == dex && dex == 678).and_then(|(_, files)| files.get(1)).copied();
    }
    let form = match variant {
        "" => 0,
        "Mega" | "Mega X" => 1,
        "Mega Y" => 2,
        _ => return None,
    };
    if form == 0 {
        return ICON_FILES.get(dex as usize).copied().filter(|&file| file != 0);
    }
    FORMS.iter().find(|(number, _)| *number == dex).and_then(|(_, files)| files.get(form)).copied()
}

/// Removes all the icons of a pack file (the models are kept).
pub fn clear_icons(pack_file: &Path) -> Result<usize, String> {
    let source = Pack::open(pack_file)?;
    let mut pack = source.clone();
    let count = pack.icons.len();
    pack.icons.clear();
    pack.icon_files.clear();
    let temporary = pack_file.with_extension("sangopack.tmp");
    pack.save_with_models(&temporary, &mut |name| source.read_model(name))?;
    std::fs::rename(&temporary, pack_file).map_err(|e| format!("{}: {e}", pack_file.display()))?;
    Ok(count)
}

/// Puts the icons of a folder in a pack file. With `game`, all the icons go in a pack that is made if the file does not exist
/// (the species of the game by file number, the species of the packs by name); else the icons of the species of the pack go in
/// the pack. The models of the pack are kept.
pub fn add_icons(pack_file: &Path, folder: &Path, game: bool) -> Result<Report, String> {
    let text = std::fs::read_to_string(folder.join("manifest.json")).map_err(|e| format!("manifest.json: {e}"))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| format!("manifest.json: {e}"))?;
    let source = if pack_file.exists() {
        Some(Pack::open(pack_file)?)
    } else if game {
        None
    } else {
        return Err(format!("{}: no such pack", pack_file.display()));
    };
    let mut pack = source.clone().unwrap_or_else(|| Pack {
        id: GAME_ICON_PACK.into(),
        name: "Pokémon HOME icons".into(),
        version: "1.0".into(),
        credit: manifest.credit.clone(),
        ..Pack::default()
    });
    let mut report = Report::default();
    for entry in &manifest.icons {
        let label = format!("{} {} {}", entry.dex, entry.species, entry.variant).trim().to_string();
        let icon = if game {
            if entry.dex > 721 {
                // A species of a pack: the icon is for the species with this name, in whatever pack has it.
                if !entry.variant.is_empty() {
                    continue;
                }
                PackIcon { species: Ref::key(&entry.species), form: 0, file: entry.file.clone(), game_file: None, label: label_of(entry) }
            } else {
            match game_file(entry.dex, &entry.variant) {
                Some(file) => PackIcon { species: Ref::Game(entry.dex), form: 0, file: entry.file.clone(), game_file: Some(file), label: label_of(entry) },
                None => {
                    report.skipped.push(format!("{label}: no file of the game for this form"));
                    continue;
                }
            }
            }
        } else {
            if entry.dex <= 721 || !entry.variant.is_empty() {
                continue;
            }
            match pack.species.iter().find(|s| key(&s.name) == key(&entry.species)) {
                Some(species) => PackIcon { species: Ref::key(&species.name), form: 0, file: entry.file.clone(), game_file: None, label: label_of(entry) },
                None => continue,
            }
        };
        let name = icon.file.trim_start_matches("icons/").trim_end_matches(".png").to_string();
        let path = folder.join(&entry.file);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        sango_pack::library::icon_file_bytes(&bytes)?;
        pack.icons.retain(|i| !(i.species == icon.species && i.form == icon.form && i.game_file == icon.game_file));
        pack.icon_files.insert(name.clone(), bytes);
        pack.icons.push(PackIcon { file: name, ..icon });
        report.added += 1;
    }
    pack.check()?;
    let temporary = pack_file.with_extension("sangopack.tmp");
    match &source {
        Some(old) => pack.save_with_models(&temporary, &mut |name| old.read_model(name))?,
        None => pack.save(&temporary)?,
    }
    std::fs::rename(&temporary, pack_file).map_err(|e| format!("{}: {e}", pack_file.display()))?;
    Ok(report)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn game_files_follow_the_tables() {
        assert_eq!(game_file(1, ""), Some(ICON_FILES[1]));
        assert_eq!(game_file(3, "Mega"), Some(3));
        assert_eq!(game_file(6, "Mega Y"), Some(8));
        assert_eq!(game_file(3, "Gigantamax"), None);
        assert_eq!(game_file(1, "Mega"), None);
        assert_eq!(game_file(521, "Female"), Some(675));
        assert_eq!(game_file(668, "Female"), Some(860));
        assert_eq!(game_file(678, "Female"), Some(923));
        assert_eq!(game_file(25, "Female"), None);
    }

    #[test]
    fn names_match_without_punctuation() {
        assert_eq!(key("Mr. Rime"), key("mr-rime"));
        assert_eq!(key("Type: Null"), key("type-null"));
    }
}
