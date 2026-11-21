//! Puts item icons in a pack: PNG pictures of 30 × 30 pixels named by the file of the game that they replace (`<file>.png`, a file
//! of the archive 92), with an optional `labels.tsv` (one line for each picture: the file number, a tab, the name that Studio
//! shows).
//!
//! Studio writes the item icons of the active packs on the SD card when it installs the packs (see `Library::write_item_icons`).

use crate::pack::PackItemIcon;
use crate::Pack;
use std::collections::HashMap;
use std::path::Path;

/// The id of the pack that holds the item icons of the recent games.
pub const ITEM_ICON_PACK: &str = "item-icons";

/// What was put in a pack.
#[derive(Debug, Default)]
pub struct Report {
    /// The icons that were added.
    pub added: usize,
    /// The pictures that were not used, with the reason.
    pub skipped: Vec<String>,
}

/// The labels of `labels.tsv`, by file number.
fn read_labels(folder: &Path) -> HashMap<u16, String> {
    let Ok(text) = std::fs::read_to_string(folder.join("labels.tsv")) else { return HashMap::new() };
    text.lines()
        .filter_map(|line| {
            let (file, label) = line.split_once('\t')?;
            Some((file.trim().parse().ok()?, label.trim().to_string()))
        })
        .collect()
}

/// Puts the pictures of a folder in a pack file, and makes the pack if the file does not exist. A picture replaces the icon
/// of the same file that the pack already has. The models and the other icons of the pack are kept.
pub fn add_item_icons(pack_file: &Path, folder: &Path, credit: &str) -> Result<Report, String> {
    let source = if pack_file.exists() { Some(Pack::open(pack_file)?) } else { None };
    let mut pack = source.clone().unwrap_or_else(|| Pack {
        id: ITEM_ICON_PACK.into(),
        name: "Item icons".into(),
        version: "1.0".into(),
        credit: credit.into(),
        ..Pack::default()
    });
    let labels = read_labels(folder);
    let mut report = Report::default();
    let mut entries: Vec<(u16, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        match path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u16>().ok()) {
            Some(file) if (file as usize) < sango_core::item_icons::NO_ICON as usize => entries.push((file, path)),
            _ => report.skipped.push(format!("{}: the name is not a file number of the item icons", path.display())),
        }
    }
    entries.sort();
    for (file, path) in entries {
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Err(message) = crate::library::item_icon_file_bytes(&bytes) {
            report.skipped.push(format!("{}: {message}", path.display()));
            continue;
        }
        let name = file.to_string();
        pack.item_icons.retain(|icon| icon.game_file != file);
        pack.item_icon_files.insert(name.clone(), bytes);
        pack.item_icons.push(PackItemIcon { game_file: file, file: name, label: labels.get(&file).cloned().unwrap_or_default() });
        report.added += 1;
    }
    pack.item_icons.sort_by_key(|icon| icon.game_file);
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

    fn png(side: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let image = image::RgbaImage::from_pixel(side, side, image::Rgba([200, 40, 40, 255]));
        image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        bytes
    }

    #[test]
    fn the_pictures_of_a_folder_go_in_a_new_pack() {
        let folder = std::env::temp_dir().join(format!("sango_item_icon_pack_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("3.png"), png(30)).unwrap();
        std::fs::write(folder.join("4.png"), png(24)).unwrap();
        std::fs::write(folder.join("potion.png"), png(30)).unwrap();
        std::fs::write(folder.join("labels.tsv"), "3\tPotion (SV)\n").unwrap();
        let file = folder.join("items.sangopack");
        let report = add_item_icons(&file, &folder, "credit").unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped.len(), 2, "a picture of the wrong size and a wrong name are left out");
        let pack = Pack::open(&file).unwrap();
        assert_eq!(pack.id, ITEM_ICON_PACK);
        assert_eq!(pack.item_icons, vec![PackItemIcon { game_file: 3, file: "3".into(), label: "Potion (SV)".into() }]);
        assert!(pack.item_icon_files.contains_key("3"));
        // Again: the icon of the file is replaced, not added twice.
        assert_eq!(add_item_icons(&file, &folder, "credit").unwrap().added, 1);
        assert_eq!(Pack::open(&file).unwrap().item_icons.len(), 1);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
