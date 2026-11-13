//! The icons of the types: pictures of 50 × 20 pixels.
//!
//! A type of the game has its icon in the romfs. The user can give any type (of the game or new) another icon: a PNG file
//! of 50 × 20 pixels. It goes on the SD card as a BCLIM file of the format RGBA4, the format that the layouts of the game
//! read, in `type/icon/<number>.bclim`. A new type without a file is drawn as a pill with its name, in the colour of its
//! colour type.

use sango_core::{Clim, SdCard};
use sango_species::battle_icons;
use sango_ui::theme::color;
use sango_ui::egui;
use std::collections::{HashMap, HashSet};

/// The size of an icon, in pixels.
pub const WIDTH: usize = 50;
pub const HEIGHT: usize = 20;

/// The folder of the icons on the SD card.
pub const SD_FOLDER: &str = "type/icon";

/// Where the icon of a type is on the SD card.
pub fn sd_path(id: usize) -> String {
    format!("{SD_FOLDER}/{id}.bclim")
}

/// The icons that the user changed, and the textures of all the icons.
#[derive(Default)]
pub struct TypeIcons {
    /// The files of the icons: the SD card, and the changes that were not saved.
    overrides: HashMap<usize, Vec<u8>>,
    /// The icons that changed and were not saved.
    pending: HashSet<usize>,
    /// The icons that were reset and whose file must go.
    removed: HashSet<usize>,
    textures: HashMap<usize, Option<egui::TextureHandle>>,
    /// The textures that were forgotten: they live on a few frames, because the frame may still draw them.
    retired: sango_ui::retired::Retired,
}

impl TypeIcons {
    /// Reads the icons of the SD card again, and forgets the changes that were not saved.
    pub fn load_card(&mut self, sd: Option<&SdCard>) {
        self.overrides.clear();
        self.pending.clear();
        self.removed.clear();
        self.retired.forget_all(&mut self.textures);
        let Some(sd) = sd else { return };
        for path in sd.files_in(SD_FOLDER) {
            let id = path.rsplit('/').next().and_then(|name| name.strip_suffix(".bclim")).and_then(|n| n.parse::<usize>().ok());
            if let (Some(id), Some(bytes)) = (id, sd.read(&path)) {
                if decode(&bytes).is_ok() {
                    self.overrides.insert(id, bytes);
                }
            }
        }
    }

    /// True when an icon changed and was not saved.
    pub fn changed(&self) -> bool {
        !self.pending.is_empty() || !self.removed.is_empty()
    }

    /// True when an icon comes from the SD card.
    pub fn from_card(&self) -> bool {
        self.overrides.keys().any(|id| !self.pending.contains(id))
    }

    /// True when the type has an icon of the user.
    pub fn has_override(&self, id: usize) -> bool {
        self.overrides.contains_key(&id)
    }

    /// The pixels of the icon of a type, when it has one (the icon of the user, or the icon of the game).
    pub fn rgba(&self, id: usize) -> Option<Vec<u8>> {
        if let Some(bytes) = self.overrides.get(&id) {
            return decode(bytes).ok();
        }
        battle_icons::type_rgba(id.min(255) as u8).filter(|_| id < crate::chart::GAME_TYPES).map(|(_, _, rgba)| rgba)
    }

    /// The texture of the icon of a type, when it has one.
    pub fn texture(&mut self, ctx: &egui::Context, id: usize) -> Option<egui::TextureHandle> {
        self.retired.tick(ctx);
        if let Some(found) = self.textures.get(&id) {
            return found.clone();
        }
        let made = self.rgba(id).map(|rgba| ctx.load_texture(format!("type_icon_{id}"), egui::ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], &rgba), egui::TextureOptions::NEAREST));
        self.textures.insert(id, made.clone());
        made
    }

    /// Gives a type an icon: a PNG file of 50 × 20 pixels. The change is kept until it is saved.
    pub fn import(&mut self, id: usize, path: &std::path::Path) -> Result<(), String> {
        let pixels = read_png(path)?;
        let clim = Clim::from_rgba4(WIDTH, HEIGHT, &pixels)?;
        self.overrides.insert(id, clim.to_bytes());
        self.pending.insert(id);
        self.removed.remove(&id);
        self.retired.forget(&mut self.textures, &id);
        Ok(())
    }

    /// Writes the icon of a type as a PNG file.
    pub fn export(&self, id: usize, path: &std::path::Path) -> Result<(), String> {
        let rgba = self.rgba(id).ok_or("This type has no icon yet: import one.")?;
        image::save_buffer(path, &rgba, WIDTH as u32, HEIGHT as u32, image::ColorType::Rgba8).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Takes the icon of the user away: the type has the icon of the game again (or the pill of a new type).
    pub fn reset(&mut self, id: usize) {
        if self.overrides.remove(&id).is_some() {
            self.pending.remove(&id);
            self.removed.insert(id);
            self.retired.forget(&mut self.textures, &id);
        }
    }

    /// Writes the icons that changed on the SD card (and removes the ones that were reset), and tells the game.
    pub fn save(&mut self, sd: &SdCard) -> Result<(), String> {
        let mut changed: Vec<usize> = self.pending.iter().copied().collect();
        changed.sort_unstable();
        for id in changed {
            let path = sd_path(id);
            sd.write(&path, &self.overrides[&id]).and_then(|_| sd.mark_changed(&path))?;
            self.pending.remove(&id);
        }
        let mut removed: Vec<usize> = self.removed.iter().copied().collect();
        removed.sort_unstable();
        for id in removed {
            sd.remove(&sd_path(id))?;
            self.removed.remove(&id);
        }
        Ok(())
    }

    /// The colour of the pill of a new type: the colour of the left of the icon of its colour type.
    fn pill_colour(colour_type: u8) -> egui::Color32 {
        match battle_icons::type_rgba(colour_type) {
            Some((width, _, rgba)) => {
                let at = (10 * width + 8) * 4;
                egui::Color32::from_rgb(rgba[at], rgba[at + 1], rgba[at + 2])
            }
            None => color::MUTED,
        }
    }

    /// Draws the icon of a type in a rectangle. A new type without an icon is a pill with its `name` in the colour of the
    /// type `colour_type`.
    pub fn paint(&mut self, ui: &egui::Ui, id: usize, rect: egui::Rect, name: &str, colour_type: u8) {
        if let Some(texture) = self.texture(ui.ctx(), id) {
            let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(texture.id(), rect, full, egui::Color32::WHITE);
            return;
        }
        let fill = Self::pill_colour(colour_type);
        ui.painter().rect_filled(rect, egui::Rounding::same(rect.height() / 2.0), fill);
        ui.painter().rect_stroke(rect, egui::Rounding::same(rect.height() / 2.0), egui::Stroke::new(1.0_f32, color::BORDER));
        let size = (rect.height() * 0.6).clamp(8.0, 13.0);
        let font = egui::FontId::proportional(size);
        let dark = egui::Color32::from_rgba_unmultiplied(20, 20, 30, 220);
        if rect.width() >= 40.0 {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                ui.painter().text(rect.center() + egui::vec2(dx, dy), egui::Align2::CENTER_CENTER, name, font.clone(), dark);
            }
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, name, font, egui::Color32::WHITE);
        }
    }
}

impl TypeIcons {
    /// Draws the icon of a type turned a quarter of a turn to the left (the name reads from the bottom to the top), in a
    /// rectangle of `width` × `height` points: the icon of 50 × 20 pixels is drawn `height` points long. It is the header of
    /// the columns of the chart, where the columns are narrower than the icons.
    pub fn paint_turned(&mut self, ui: &egui::Ui, id: usize, rect: egui::Rect, name: &str, colour_type: u8) {
        let centre = rect.center();
        // The icon before the turn is `long` points wide and `short` points high.
        let (short, long) = (rect.width(), rect.height());
        let turn = |x: f32, y: f32| centre + egui::vec2(y, -x);
        let (half_long, half_short) = (long / 2.0, short / 2.0);
        if let Some(texture) = self.texture(ui.ctx(), id) {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            for (x, y, u, v) in [(-half_long, -half_short, 0.0, 0.0), (half_long, -half_short, 1.0, 0.0), (half_long, half_short, 1.0, 1.0), (-half_long, half_short, 0.0, 1.0)] {
                mesh.vertices.push(egui::epaint::Vertex { pos: turn(x, y), uv: egui::pos2(u, v), color: egui::Color32::WHITE });
            }
            mesh.indices.extend([0, 1, 2, 0, 2, 3]);
            ui.painter().add(egui::Shape::mesh(mesh));
            return;
        }
        let fill = Self::pill_colour(colour_type);
        ui.painter().rect_filled(rect, egui::Rounding::same(short / 2.0), fill);
        ui.painter().rect_stroke(rect, egui::Rounding::same(short / 2.0), egui::Stroke::new(1.0_f32, color::BORDER));
        if !name.is_empty() {
            let galley = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional((short * 0.55).clamp(8.0, 12.0)), egui::Color32::WHITE);
            let at = centre + egui::vec2(-galley.size().y / 2.0, galley.size().x / 2.0);
            ui.painter().add(egui::epaint::TextShape::new(at, galley, egui::Color32::WHITE).with_angle(-std::f32::consts::FRAC_PI_2));
        }
    }
}

/// The RGBA pixels of a PNG file of an icon. The picture must have the size of an icon.
fn read_png(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let image = image::open(path).map_err(|error| format!("{}: {error}", path.display()))?.to_rgba8();
    if image.width() as usize != WIDTH || image.height() as usize != HEIGHT {
        return Err(format!("The icon must have {WIDTH} × {HEIGHT} pixels. This image has {} × {}.", image.width(), image.height()));
    }
    Ok(image.into_raw())
}

/// The RGBA pixels of an icon file.
fn decode(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = Clim::parse(bytes)?;
    if (image.width(), image.height()) != (WIDTH, HEIGHT) {
        return Err(format!("The icon has {} × {} pixels.", image.width(), image.height()));
    }
    image.to_rgba()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn an_icon_is_imported_saved_and_reset() {
        let folder = std::env::temp_dir().join(format!("sango_type_icons_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut icons = TypeIcons::default();
        let png = folder.join("icon.png");
        let mut picture = image::RgbaImage::new(WIDTH as u32, HEIGHT as u32);
        for pixel in picture.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        picture.save(&png).unwrap();
        icons.import(19, &png).unwrap();
        assert!(icons.changed() && icons.has_override(19));
        assert_eq!(icons.rgba(19).unwrap()[..4], [255, 0, 0, 255]);
        icons.save(&sd).unwrap();
        assert!(!icons.changed());
        assert!(sd.read(&sd_path(19)).is_some());
        let mut again = TypeIcons::default();
        again.load_card(Some(&sd));
        assert!(again.from_card() && again.has_override(19));
        again.reset(19);
        assert!(again.changed() && !again.has_override(19));
        again.save(&sd).unwrap();
        assert!(sd.read(&sd_path(19)).is_none());
        // A picture of another size is refused, and an export needs an icon.
        let wrong = folder.join("wrong.png");
        image::RgbaImage::new(10, 10).save(&wrong).unwrap();
        assert!(again.import(19, &wrong).unwrap_err().contains("50 × 20"));
        assert!(again.export(19, &png).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
