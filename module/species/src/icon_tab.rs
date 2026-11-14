//! The icon card of the General tab: the small picture of a species or a form that the game shows in the boxes and the lists.
//!
//! The icons are the files of the archive 91: images of 40 × 30 pixels, with at most 256 colours of 15 bits and
//! one bit of transparency, compressed with LZ11. The file of a species, of a form and of a female is given by
//! `icons::ICON_FILES` and `icon_files`. Some forms share a file: a change shows for all of them.
//!
//! The user imports a PNG image of 40 × 30 pixels. The studio reduces its colours to the format of the game, and
//! writes the compressed file on the SD card as `pokemon/icon/<file>.bin`, with the layout of the romfs.

use crate::icon_files::{FEMALE, FORMS};
use crate::icons::{ICON_ARCHIVE, ICON_FILES};
use sango_core::{lz, Clim, Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::{HashMap, HashSet};

/// The size of an icon, in pixels.
const WIDTH: usize = 40;
const HEIGHT: usize = 30;

/// The file of the archive of the icons for a species, a form and a sex.
///
/// A form that has its own icon uses it for both sexes. The other forms use the icon of the species: the
/// female icon for a female when the species has one, the male icon otherwise.
pub fn icon_file(species: usize, form: usize, female: bool) -> Option<usize> {
    if let Some((_, files)) = FORMS.iter().find(|(number, _)| *number as usize == species) {
        if let Some(&file) = files.get(form) {
            return Some(file as usize);
        }
    }
    if female {
        if let Some(&(_, file)) = FEMALE.iter().find(|(number, _)| *number as usize == species) {
            return Some(file as usize);
        }
    }
    ICON_FILES.get(species).map(|&file| file as usize)
}

/// True when the species has an icon of its own for the females.
pub fn has_female_icon(species: usize) -> bool {
    FEMALE.iter().any(|(number, _)| *number as usize == species)
}

/// The place on the SD card of an icon file.
pub fn sd_path(file: usize) -> String {
    format!("pokemon/icon/{file}.bin")
}

/// The state of the icon card.
#[derive(Default)]
pub struct IconTab {
    garc: Option<Garc>,
    /// The files that the user changed and did not save: the compressed bytes of each.
    pending: HashMap<usize, Vec<u8>>,
    /// The files that were read from the SD card, or written on it.
    from_card: HashSet<usize>,
    /// True for the icon of the females, when the species has one.
    female: bool,
    /// What the picture shows: the file, and the compressed bytes that it was made from.
    shown: Option<(usize, Vec<u8>)>,
    texture: Option<egui::TextureHandle>,
    /// The textures that were forgotten: they live on a few frames, because a frame that is being drawn may still use them.
    retired: sango_ui::retired::Retired,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
}

impl IconTab {
    /// Opens the archive of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.garc = Garc::open(&dump.archive(ICON_ARCHIVE)).ok();
        self.pending.clear();
        self.from_card.clear();
        self.shown = None;
        if let Some(old) = self.texture.take() {
            self.retired.keep(old);
        }
    }

    /// The compressed file as the romfs has it.
    fn romfs(&self, file: usize) -> Option<Vec<u8>> {
        self.garc.as_ref()?.first_file(file).map(|bytes| bytes.to_vec())
    }

    /// The compressed file as the user sees it: the changes first, then the SD card, then the romfs.
    fn current(&mut self, sd: Option<&SdCard>, file: usize) -> Option<Vec<u8>> {
        if let Some(bytes) = self.pending.get(&file) {
            return Some(bytes.clone());
        }
        if let Some(bytes) = sd.and_then(|sd| sd.read(&sd_path(file))).filter(|bytes| decode(bytes).is_ok()) {
            self.from_card.insert(file);
            return Some(bytes);
        }
        self.romfs(file)
    }

    /// True when an icon changed and was not saved.
    pub fn changed(&self) -> bool {
        !self.pending.is_empty()
    }

    /// True when an icon that was read comes from the SD card.
    pub fn from_card(&self) -> bool {
        !self.from_card.is_empty()
    }

    /// Writes the icons that changed on the SD card, and tells the game. Returns the paths that were written.
    pub fn save(&mut self, sd: &SdCard) -> Result<Vec<String>, String> {
        let mut files: Vec<usize> = self.pending.keys().copied().collect();
        files.sort_unstable();
        let mut written = Vec::new();
        for file in files {
            let path = sd_path(file);
            sd.write(&path, &self.pending[&file]).and_then(|_| sd.mark_changed(&path))?;
            self.pending.remove(&file);
            self.from_card.insert(file);
            written.push(path);
        }
        Ok(written)
    }

    /// Forgets the changes that were not saved and what was read from the SD card: the next display reads the files again.
    pub fn reload(&mut self) {
        self.pending.clear();
        self.from_card.clear();
        self.shown = None;
        if let Some(old) = self.texture.take() {
            self.retired.keep(old);
        }
    }

    /// Gives an icon a new picture: 40 × 30 pixels of RGBA. Returns the compressed file.
    fn make_file(rgba: &[u8]) -> Result<Vec<u8>, String> {
        let image = Clim::from_rgba(WIDTH, HEIGHT, rgba)?;
        Ok(lz::compress_lz11(&image.to_bytes()))
    }

    /// Reads a PNG file and makes it the icon of a file of the archive.
    fn import(&mut self, path: &std::path::Path, file: usize) -> Result<(), String> {
        let image = image::open(path).map_err(|error| format!("{}: {error}", path.display()))?.to_rgba8();
        if image.width() as usize != WIDTH || image.height() as usize != HEIGHT {
            return Err(format!("The icon must have {WIDTH} × {HEIGHT} pixels. This image has {} × {}.", image.width(), image.height()));
        }
        let bytes = Self::make_file(image.as_raw())?;
        self.pending.insert(file, bytes);
        Ok(())
    }

    /// Draws the tab.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &Session, species: usize, form: usize) {
        self.retired.tick(ui.ctx());
        let sd = session.sd.as_ref();
        if !has_female_icon(species) {
            self.female = false;
        }
        let Some(file) = icon_file(species, form, self.female) else {
            ui.label(widget::hint("This species has no icon in the table."));
            return;
        };
        let bytes = self.current(sd, file);
        // The picture is made again when the file or its bytes change.
        let same = matches!((&self.shown, &bytes), (Some((shown_file, shown_bytes)), Some(now)) if *shown_file == file && shown_bytes == now);
        if !same || self.texture.is_none() {
            if let Some(old) = self.texture.take() {
                self.retired.keep(old);
            }
            self.texture = bytes.as_deref().and_then(|bytes| decode(bytes).ok()).map(|rgba| sango_ui::textures::load(ui.ctx(), "species_icon_editor", egui::ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], &rgba), egui::TextureOptions::NEAREST));
            self.shown = bytes.clone().map(|bytes| (file, bytes));
        }
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE_STRONG);
                if let Some(texture) = &self.texture {
                    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    let image = egui::Rect::from_center_size(rect.center(), egui::vec2(WIDTH as f32 * 2.0, HEIGHT as f32 * 2.0));
                    ui.painter().image(texture.id(), image, full, egui::Color32::WHITE);
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(widget::title("Icon"));
                        if self.pending.contains_key(&file) {
                            widget::badge(ui, "Not saved", color::AMBER);
                        } else if self.from_card.contains(&file) {
                            widget::badge(ui, "From the SD card", color::TEAL);
                        }
                    });
                    ui.label(widget::hint(&format!("{WIDTH} × {HEIGHT} pixels, in the party and the boxes. Some forms share their icon: a change shows for all of them.")));
                    if has_female_icon(species) {
                        ui.horizontal(|ui| {
                            ui.label(widget::hint("Icon of the"));
                            if ui.selectable_label(!self.female, "Males").clicked() {
                                self.female = false;
                            }
                            if ui.selectable_label(self.female, "Females").clicked() {
                                self.female = true;
                            }
                        });
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if widget::button(ui, "Export PNG…") {
                            self.status = Some(self.export(file, bytes.as_deref()));
                        }
                        if widget::button(ui, "Import PNG…") {
                            if let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() {
                                self.status = Some(match self.import(&path, file) {
                                    Ok(()) => (true, "The icon is changed. Save to write it on the SD card.".to_string()),
                                    Err(message) => (false, message),
                                });
                            }
                        }
                        let has_change = self.pending.contains_key(&file) || self.from_card.contains(&file);
                        ui.add_enabled_ui(has_change, |ui| {
                            if widget::button(ui, "Reset icon") {
                                self.status = Some(self.reset(file, sd));
                            }
                        })
                        .response
                        .on_hover_text("Puts the icon of the game back: the change and the file of the SD card are removed.");
                    });
                });
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
    }

    /// Puts the icon of the game back: forgets the change and removes the file of the SD card.
    fn reset(&mut self, file: usize, sd: Option<&SdCard>) -> (bool, String) {
        self.pending.remove(&file);
        self.from_card.remove(&file);
        self.shown = None;
        if let Some(sd) = sd {
            let path = sd_path(file);
            match sd.remove(&path) {
                Ok(true) => {
                    if let Err(message) = sd.mark_changed(&path) {
                        return (false, message);
                    }
                }
                Ok(false) => {}
                Err(message) => return (false, message),
            }
        }
        (true, "The icon of the game is back.".to_string())
    }

    /// Writes the icon as a PNG file that the user chooses.
    fn export(&self, file: usize, bytes: Option<&[u8]>) -> (bool, String) {
        let Some(rgba) = bytes.and_then(|bytes| decode(bytes).ok()) else { return (false, "The icon cannot be read.".into()) };
        let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name(format!("icon_{file}.png")).save_file() else { return (true, String::new()) };
        match image::save_buffer(&path, &rgba, WIDTH as u32, HEIGHT as u32, image::ColorType::Rgba8) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        }
    }
}

/// The RGBA pixels of a compressed icon file.
fn decode(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let raw = if lz::is_compressed(bytes) { lz::decompress(bytes)? } else { bytes.to_vec() };
    let image = Clim::parse(&raw)?;
    if (image.width(), image.height()) != (WIDTH, HEIGHT) {
        return Err(format!("The icon has {} × {} pixels.", image.width(), image.height()));
    }
    image.to_rgba()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn each_form_and_sex_has_its_file() {
        // The first male icon of the species comes from the table of the plugin.
        assert_eq!(icon_file(1, 0, false), Some(ICON_FILES[1] as usize));
        assert_eq!(icon_file(1, 0, true), Some(ICON_FILES[1] as usize));
        // Venusaur has an icon for the Mega Evolution, and a form that does not exist gives the first icon.
        let venusaur = icon_file(3, 0, false).unwrap();
        let mega = icon_file(3, 1, false).unwrap();
        assert_ne!(venusaur, mega);
        assert_eq!(icon_file(3, 9, false), Some(venusaur));
        // Four species have an icon for the females: Unfezant, Frillish, Jellicent, Pyroar.
        for species in [521, 592, 593, 668] {
            assert!(has_female_icon(species));
            assert_ne!(icon_file(species, 0, true), icon_file(species, 0, false), "species {species}");
        }
        assert!(!has_female_icon(1));
        assert_eq!(icon_file(0, 0, false), Some(ICON_FILES[0] as usize));
        assert_eq!(icon_file(9999, 0, false), None);
        assert_eq!(sd_path(7), "pokemon/icon/7.bin");
    }

    #[test]
    fn a_picture_becomes_a_file_and_comes_back() {
        let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
        for pixel in rgba.chunks_exact_mut(4).take(100) {
            pixel.copy_from_slice(&[0, 255, 0, 255]);
        }
        let file = IconTab::make_file(&rgba).unwrap();
        assert!(lz::is_compressed(&file));
        assert_eq!(decode(&file).unwrap(), rgba);
        assert!(IconTab::make_file(&rgba[..40]).is_err());
        assert!(decode(&[1, 2, 3]).is_err());
    }

    /// With the dump of the game: the icons of the forms and of the females of the tables are files of the
    /// archive that decode to 40 × 30 pixels, an icon is imported from a PNG, saved on the SD card and read again.
    #[test]
    fn the_icons_of_the_forms_are_in_the_dump_and_an_icon_is_changed() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let mut tab = IconTab::default();
        tab.open(&dump);
        for (species, files) in FORMS {
            for &file in files {
                let bytes = tab.romfs(file as usize).unwrap_or_else(|| panic!("species {species}: no file {file}"));
                decode(&bytes).unwrap_or_else(|e| panic!("species {species} file {file}: {e}"));
            }
        }
        for (species, file) in FEMALE {
            decode(&tab.romfs(file as usize).unwrap()).unwrap_or_else(|e| panic!("species {species}: {e}"));
        }

        let folder = std::env::temp_dir().join(format!("sango_icon_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        // A PNG of the right size is the new icon of the Bulbasaur file; one of the wrong size is refused.
        let png = folder.join("new.png");
        let mut picture = image::RgbaImage::new(WIDTH as u32, HEIGHT as u32);
        for pixel in picture.pixels_mut().take(200) {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        picture.save(&png).unwrap();
        let file = icon_file(1, 0, false).unwrap();
        tab.import(&png, file).unwrap();
        assert!(tab.changed());
        let wrong = folder.join("wrong.png");
        image::RgbaImage::new(10, 10).save(&wrong).unwrap();
        assert!(tab.import(&wrong, file).unwrap_err().contains("40 × 30"));

        // The tab draws, for a species with a female icon too.
        let ctx = egui::Context::default();
        for (species, form) in [(1, 0), (3, 1), (668, 0)] {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0)));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| tab.ui(ui, &session, species, form));
            });
        }
        assert!(tab.texture.is_some());

        // Save writes the file, and a new tab reads it from the SD card.
        assert_eq!(tab.save(&sd).unwrap(), [sd_path(file)]);
        assert!(!tab.changed());
        let mut again = IconTab::default();
        again.open(&dump);
        let read = again.current(Some(&sd), file).unwrap();
        assert_eq!(decode(&read).unwrap()[..4], [248 + 7, 0, 0, 255]);
        assert!(again.from_card());

        // Reload forgets the read files.
        again.reload();
        assert!(!again.from_card() && !again.changed());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
