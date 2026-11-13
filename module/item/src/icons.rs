//! The pictures of the items: 30 × 30 pixels, in the archive 92 (see `sango_core::item_icons` for the table).
//! This module adds the pictures that the user changed: they are on the SD card, or not saved yet.

use sango_core::{Clim, Dump, Garc, SdCard};
use sango_ui::egui;
use std::collections::{HashMap, HashSet};

pub use sango_core::item_icons::SIZE;

/// The folder of the pictures that the user changed, on the SD card.
pub const SD_FOLDER: &str = "item/icon";

/// Where the picture file of the archive is on the SD card (a compressed file, as in the archive).
pub fn sd_path(file: usize) -> String {
    format!("{SD_FOLDER}/{file}.bin")
}

/// The pictures of the items: the file of the archive, or the one of the SD card, or the one that the user changed and
/// did not save. The textures are decoded at the first display and then kept.
#[derive(Default)]
pub struct ItemIcons {
    garc: Option<Garc>,
    /// The files of the SD card, and the changes that were not saved: the compressed bytes of each file.
    overrides: HashMap<usize, Vec<u8>>,
    /// The files that changed and were not saved.
    pending: HashSet<usize>,
    textures: HashMap<usize, Option<egui::TextureHandle>>,
}

impl ItemIcons {
    /// Opens the archive of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.garc = Garc::open(&dump.archive(sango_core::item_icons::ARCHIVE)).ok();
        self.overrides.clear();
        self.pending.clear();
        self.textures.clear();
    }

    /// Reads the pictures of the SD card again, and forgets the changes that were not saved.
    pub fn load_card(&mut self, sd: Option<&SdCard>) {
        self.overrides.clear();
        self.pending.clear();
        self.textures.clear();
        let Some(sd) = sd else { return };
        for path in sd.files_in(SD_FOLDER) {
            let file = path.rsplit('/').next().and_then(|name| name.strip_suffix(".bin")).and_then(|n| n.parse::<usize>().ok());
            if let (Some(file), Some(bytes)) = (file, sd.read(&path)) {
                if decode(&bytes).is_ok() {
                    self.overrides.insert(file, bytes);
                }
            }
        }
    }

    /// True when a picture changed and was not saved.
    pub fn changed(&self) -> bool {
        !self.pending.is_empty()
    }

    /// True when a picture comes from the SD card.
    pub fn from_card(&self) -> bool {
        self.overrides.keys().any(|file| !self.pending.contains(file))
    }

    /// The file of the picture of an item, when it has one of its own.
    pub fn file_of(item: usize) -> Option<usize> {
        sango_core::item_icons::file_of(item)
    }

    /// The compressed file as the user sees it: the changes first, then the SD card, then the archive.
    fn compressed(&self, file: usize) -> Option<Vec<u8>> {
        self.overrides.get(&file).cloned().or_else(|| self.garc.as_ref()?.first_file(file).map(|bytes| bytes.to_vec()))
    }

    /// The decoded picture of an item (30 × 30 pixels of RGBA), when it has one.
    pub fn rgba(&self, item: usize) -> Option<Vec<u8>> {
        decode(&self.compressed(Self::file_of(item)?)?).ok()
    }

    /// The texture of an item picture.
    pub fn texture(&mut self, ctx: &egui::Context, item: usize) -> Option<egui::TextureHandle> {
        if let Some(found) = self.textures.get(&item) {
            return found.clone();
        }
        let made = self.rgba(item).map(|rgba| ctx.load_texture(format!("item_icon_{item}"), egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &rgba), egui::TextureOptions::NEAREST));
        self.textures.insert(item, made.clone());
        made
    }

    /// Gives the picture of an item a new look: a PNG file of 30 × 30 pixels. The change is kept until it is saved.
    pub fn import(&mut self, item: usize, path: &std::path::Path) -> Result<(), String> {
        let file = Self::file_of(item).ok_or("This item has no picture of its own.")?;
        let image = image::open(path).map_err(|error| format!("{}: {error}", path.display()))?.to_rgba8();
        if image.width() as usize != SIZE || image.height() as usize != SIZE {
            return Err(format!("The picture must have {SIZE} × {SIZE} pixels. This image has {} × {}.", image.width(), image.height()));
        }
        let clim = Clim::from_rgba(SIZE, SIZE, image.as_raw())?;
        self.overrides.insert(file, sango_core::lz::compress_lz11(&clim.to_bytes()));
        self.pending.insert(file);
        self.textures.clear();
        Ok(())
    }

    /// Writes the picture of an item as a PNG file.
    pub fn export(&self, item: usize, path: &std::path::Path) -> Result<(), String> {
        let rgba = self.rgba(item).ok_or("This item has no picture of its own.")?;
        image::save_buffer(path, &rgba, SIZE as u32, SIZE as u32, image::ColorType::Rgba8).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Writes the pictures that changed on the SD card, and tells the game.
    pub fn save(&mut self, sd: &SdCard) -> Result<(), String> {
        let mut files: Vec<usize> = self.pending.iter().copied().collect();
        files.sort_unstable();
        for file in files {
            let path = sd_path(file);
            sd.write(&path, &self.overrides[&file]).and_then(|_| sd.mark_changed(&path))?;
            self.pending.remove(&file);
        }
        Ok(())
    }
}

use sango_core::item_icons::decode;

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::item_icons::{ICON_FILES, NO_ICON};

    #[test]
    fn the_pictures_of_the_items_are_found_changed_and_saved() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let mut icons = ItemIcons::default();
        icons.open(&Dump::open(&root).unwrap());
        // The Master Ball and the Potion have a picture of 30 × 30 pixels with some pixels that show.
        for item in [1, 17, 50, 775] {
            let rgba = icons.rgba(item).unwrap_or_else(|| panic!("the item {item} has a picture"));
            assert_eq!(rgba.len(), SIZE * SIZE * 4);
            assert!(rgba.chunks(4).filter(|p| p[3] > 200).count() > 100, "the picture of the item {item} is not empty");
        }
        assert!(icons.rgba(0).is_none(), "no item, no picture");
        let missing = (1..=775).filter(|&i| ICON_FILES[i] != NO_ICON && icons.rgba(i).is_none()).count();
        assert_eq!(missing, 0, "every picture of the table is in the archive");

        // Export, change a pixel, import, save, read the SD card again.
        let folder = std::env::temp_dir().join(format!("sango_item_icons_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let png = folder.join("potion.png");
        icons.export(17, &png).unwrap();
        let mut picture = image::open(&png).unwrap().to_rgba8();
        assert_eq!(picture.as_raw(), &icons.rgba(17).unwrap());
        picture.put_pixel(15, 15, image::Rgba([248, 0, 0, 255]));
        picture.save(&png).unwrap();
        icons.import(17, &png).unwrap();
        assert!(icons.changed());
        assert_eq!(icons.rgba(17).unwrap()[(15 * SIZE + 15) * 4..][..4], [255, 0, 0, 255]);
        icons.save(&sd).unwrap();
        assert!(!icons.changed());
        let mut again = ItemIcons::default();
        again.open(&Dump::open(&root).unwrap());
        again.load_card(Some(&sd));
        assert!(again.from_card());
        assert_eq!(again.rgba(17), icons.rgba(17));
        // A picture of another size, and an item without a picture, are refused.
        let wrong = folder.join("wrong.png");
        image::RgbaImage::new(10, 10).save(&wrong).unwrap();
        assert!(again.import(17, &wrong).unwrap_err().contains("30 × 30"));
        assert!(again.import(0, &png).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
