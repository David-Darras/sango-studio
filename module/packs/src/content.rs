//! The content of a pack: the things that the user can turn on and off, one by one.
//!
//! For now the things are the icons of the species and of the items. Each row shows the icon that the game has now (before) and
//! the icon of the pack (after), with a check box. A thing that is turned off is not written on the SD card: the game keeps what it had. The choice is kept in the
//! registry (`InstalledPack::disabled`), so it stays when the pack is installed again.

use crate::model_view::ModelPair;
use sango_core::{Dump, Garc, SdCard};
use sango_pack::pack::ModelRef;
use sango_pack::Library;
use sango_ui::theme::color;
use sango_ui::{egui, retired::Retired, textures, widget};
use std::collections::HashMap;

/// The size of a species icon, in pixels.
const WIDTH: usize = 40;
const HEIGHT: usize = 30;
/// The number of the archive of the species icons of the game.
const ICON_ARCHIVE: usize = 91;
/// The size of an item icon, in pixels.
const ITEM_SIDE: usize = sango_core::item_icons::SIZE;
/// The drawn pictures have bigger pixels.
const ZOOM: f32 = 1.6;

/// One row of the list of icons.
struct Row {
    key: String,
    label: String,
    file: String,
    game_file: Option<u16>,
    /// True for an item icon, false for a species icon.
    item: bool,
}

impl Row {
    fn size(&self) -> (usize, usize) {
        if self.item {
            (ITEM_SIDE, ITEM_SIDE)
        } else {
            (WIDTH, HEIGHT)
        }
    }

    fn archive(&self) -> usize {
        if self.item {
            sango_core::item_icons::ARCHIVE
        } else {
            ICON_ARCHIVE
        }
    }
}

#[derive(Default)]
pub struct PackContent {
    /// The id of the pack that is shown.
    pub open: Option<String>,
    search: String,
    /// The archives of the icons of the game, for the "before" pictures, by number.
    garcs: HashMap<usize, Garc>,
    garc_root: Option<std::path::PathBuf>,
    textures: HashMap<String, Option<egui::TextureHandle>>,
    retired: Retired,
    models: ModelPair,
    /// True when a check box changed and the SD card was not written yet.
    changed: bool,
    status: Option<(bool, String)>,
}

impl PackContent {
    /// Closes the view and forgets the pictures.
    pub fn close(&mut self) {
        self.open = None;
        self.models.close();
        self.forget();
    }

    fn forget(&mut self) {
        for (_, texture) in self.textures.drain() {
            if let Some(texture) = texture {
                self.retired.keep(texture);
            }
        }
        self.changed = false;
        self.status = None;
    }

    /// The picture of a file of an archive of icons of the game.
    fn game_icon(&mut self, dump: Option<&Dump>, archive: usize, file: u16, (width, height): (usize, usize)) -> Option<Vec<u8>> {
        let dump = dump?;
        if self.garc_root.as_deref() != Some(dump.root()) {
            self.garcs.clear();
            self.garc_root = Some(dump.root().to_path_buf());
        }
        if !self.garcs.contains_key(&archive) {
            self.garcs.insert(archive, Garc::open(&dump.archive(archive)).ok()?);
        }
        let bytes = self.garcs.get(&archive)?.first_file(file as usize)?;
        sango_core::clim::decode_sized(bytes, width, height).ok()
    }

    /// The texture of a picture, made the first time it is asked.
    fn texture(&mut self, ctx: &egui::Context, key: String, size: (usize, usize), make: impl FnOnce(&mut Self) -> Option<Vec<u8>>) -> Option<egui::TextureHandle> {
        if !self.textures.contains_key(&key) {
            let texture = make(self).map(|rgba| {
                let picture = egui::ColorImage::from_rgba_unmultiplied([size.0, size.1], &rgba);
                textures::load(ctx, format!("pack_icon_{key}"), picture, egui::TextureOptions::NEAREST)
            });
            self.textures.insert(key.clone(), texture);
        }
        self.textures.get(&key).cloned().flatten()
    }

    /// Draws a picture in a box of the size of a species icon, so that all the rows have the same size.
    fn show_picture(ui: &mut egui::Ui, texture: Option<&egui::TextureHandle>, (width, height): (usize, usize)) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(WIDTH as f32 * ZOOM + 8.0, HEIGHT as f32 * ZOOM + 8.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
        if let Some(texture) = texture {
            let image = egui::Rect::from_center_size(rect.center(), egui::vec2(width as f32 * ZOOM, height as f32 * ZOOM));
            let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(texture.id(), image, full, egui::Color32::WHITE);
        } else {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "none", egui::FontId::proportional(11.0), color::MUTED);
        }
    }

    /// Draws the content of the open pack. Returns a message when the SD card was written.
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &mut Library, sd: &SdCard, dump: Option<&Dump>) {
        self.retired.tick(ui.ctx());
        let Some(id) = self.open.clone() else { return };
        let Some(pack) = library.packs.iter().find(|p| p.id == id) else {
            self.close();
            return;
        };
        let label = |label: &str, file: &str| if label.is_empty() { file.to_string() } else { label.to_string() };
        let mut rows: Vec<Row> = pack.icons.iter().map(|i| Row { key: i.key(), label: label(&i.label, &i.file), file: i.file.clone(), game_file: i.game_file, item: false }).collect();
        rows.extend(pack.item_icons.iter().map(|i| Row { key: i.key(), label: label(&i.label, &format!("Item icon {}", i.game_file)), file: i.file.clone(), game_file: Some(i.game_file), item: true }));
        let name = pack.name.clone();
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title(&format!("Content of {name}")));
                if widget::button(ui, "Close") {
                    self.close();
                }
            });
            if rows.is_empty() {
                ui.label(widget::hint("This pack has no icons."));
                self.models_section(ui, library, &id, dump);
                return;
            }
            let enabled = rows.iter().filter(|r| library.is_enabled(&id, &r.key)).count();
            ui.label(widget::hint(&format!("Icons: {enabled} of {} turned on. Check the icons that you want. \"Before\" is the icon that the game has, \"After\" is the icon of the pack.", rows.len())));
            ui.add_space(4.0);
            let needle = self.search.to_lowercase();
            let shown: Vec<&Row> = rows.iter().filter(|r| needle.is_empty() || r.label.to_lowercase().contains(&needle)).collect();
            let mut toggles: Vec<(String, bool)> = Vec::new();
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(180.0));
                if widget::button(ui, "All on") {
                    toggles.extend(shown.iter().map(|r| (r.key.clone(), true)));
                }
                if widget::button(ui, "All off") {
                    toggles.extend(shown.iter().map(|r| (r.key.clone(), false)));
                }
            });
            ui.add_space(4.0);
            let row_height = HEIGHT as f32 * ZOOM + 16.0;
            egui::ScrollArea::vertical().id_salt("pack_content").max_height(460.0).auto_shrink([false, true]).show_rows(ui, row_height, shown.len(), |ui, range| {
                for row in &shown[range] {
                    ui.horizontal(|ui| {
                        let mut on = library.is_enabled(&id, &row.key);
                        if ui.checkbox(&mut on, "").changed() {
                            toggles.push((row.key.clone(), on));
                        }
                        let ctx = ui.ctx().clone();
                        let size = row.size();
                        let kind = if row.item { "item" } else { "species" };
                        let before = match row.game_file {
                            Some(file) => self.texture(&ctx, format!("game:{kind}:{file}"), size, |me| me.game_icon(dump, row.archive(), file, size)),
                            None => None,
                        };
                        let after = {
                            let pack = library.packs.iter().find(|p| p.id == id);
                            let files = pack.map(|p| if row.item { &p.item_icon_files } else { &p.icon_files });
                            let png = files.and_then(|f| f.get(&row.file)).cloned();
                            self.texture(&ctx, format!("pack:{id}:{kind}:{}", row.file), size, |_| {
                                let image = image::load_from_memory(&png?).ok()?.to_rgba8();
                                (image.width() as usize == size.0 && image.height() as usize == size.1).then(|| image.into_raw())
                            })
                        };
                        ui.vertical(|ui| {
                            ui.label(widget::hint("Before"));
                            Self::show_picture(ui, before.as_ref(), size);
                        });
                        ui.vertical(|ui| {
                            ui.label(widget::hint("After"));
                            Self::show_picture(ui, after.as_ref(), size);
                        });
                        ui.label(egui::RichText::new(&row.label).strong());
                        if row.game_file.is_none() {
                            widget::badge(ui, "New", color::TEAL);
                        }
                    });
                }
            });
            for (key, on) in toggles {
                library.set_enabled(&id, &key, on);
                self.changed = true;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.changed, |ui| {
                    if widget::button_styled(ui, egui::RichText::new("Apply to the SD card").color(color::WHITE), Some(color::ACCENT)) {
                        self.status = Some(match library.save(sd).and_then(|_| library.write_icons(sd)) {
                            Ok(_) => {
                                self.changed = false;
                                (true, "The icons that are turned on are on the SD card. Restart the game.".to_string())
                            }
                            Err(message) => (false, message),
                        });
                    }
                });
                if self.changed {
                    widget::badge(ui, "Not applied", color::AMBER);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
            self.models_section(ui, library, &id, dump);
        });
    }

    /// The models of the pack: a list, and the view of the model that the user chooses.
    fn models_section(&mut self, ui: &mut egui::Ui, library: &Library, id: &str, dump: Option<&Dump>) {
        let Some(pack) = library.packs.iter().find(|p| p.id == id) else { return };
        let names = pack.model_names();
        if names.is_empty() {
            return;
        }
        ui.add_space(10.0);
        ui.separator();
        ui.label(widget::title("3D models"));
        ui.label(widget::hint("Choose a model to see it. When it replaces a model of the game, the old one is shown beside it."));
        let mut chosen = None;
        egui::ScrollArea::vertical().id_salt("pack_models").max_height(220.0).auto_shrink([false, true]).show_rows(ui, 24.0, names.len(), |ui, range| {
            for name in &names[range] {
                ui.horizontal(|ui| {
                    let replaces = pack.replacements.iter().find(|r| &r.model == name).map(|r| (r.species, r.slot));
                    let open = self.models.open.as_deref() == Some(name.as_str());
                    if ui.selectable_label(open, name.as_str()).clicked() {
                        chosen = Some((name.clone(), replaces));
                    }
                    match replaces {
                        Some((species, slot)) => widget::badge(ui, &format!("Replaces species {species}, slot {slot}"), color::AMBER),
                        None => widget::badge(ui, "New", color::TEAL),
                    }
                    ui.label(widget::hint(&usage(pack, name)));
                });
            }
        });
        if let Some((name, replaces)) = chosen {
            self.models.select(pack, &name, replaces, dump);
        }
        ui.add_space(6.0);
        self.models.ui(ui);
    }
}

/// What uses a model of the pack: the species and the forms that name it.
fn usage(pack: &sango_pack::Pack, name: &str) -> String {
    let mut users: Vec<String> = pack.species.iter().filter(|s| matches!(&s.model, ModelRef::File { file } if file == name)).map(|s| s.name.clone()).collect();
    for form in &pack.forms {
        let uses = [&form.model, &form.female_model].into_iter().flatten().any(|m| matches!(m, ModelRef::File { file } if file == name));
        if uses {
            let owner = match &form.species {
                sango_pack::Ref::Game(n) => format!("species {n}"),
                sango_pack::Ref::Key(k) => k.clone(),
            };
            users.push(format!("form {} of {owner}", form.form));
        }
    }
    users.join(", ")
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_pack::pack::{to_hex, ModelRef, Move, PackIcon, PackSpecies, DATA_SIZE};
    use sango_pack::{Pack, Ref};

    #[test]
    fn the_view_draws_the_icons_and_the_choices_reach_the_sd_card() {
        let mut picture = image::RgbaImage::new(40, 30);
        for pixel in picture.pixels_mut().take(80) {
            *pixel = image::Rgba([0, 200, 0, 255]);
        }
        let mut png = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut pack = Pack {
            id: "icons".into(),
            name: "icons".into(),
            version: "1".into(),
            species: vec![PackSpecies { name: "Zap".into(), model: ModelRef::Native { native: 25 }, data: to_hex(&[1u8; DATA_SIZE]), moves: vec![Move { id: 1, level: 1 }], evolutions: Vec::new() }],
            ..Pack::default()
        };
        pack.icons = vec![PackIcon { species: Ref::key("Zap"), form: 0, file: "zap".into(), game_file: None, label: "Zap".into() }];
        pack.icon_files.insert("zap".into(), png.into_inner());
        let mut item_png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(30, 30, image::Rgba([255, 0, 0, 255])).write_to(&mut item_png, image::ImageFormat::Png).unwrap();
        pack.item_icons = vec![sango_pack::pack::PackItemIcon { game_file: 4, file: "4".into(), label: "Potion".into() }];
        pack.item_icon_files.insert("4".into(), item_png.into_inner());
        let folder = std::env::temp_dir().join(format!("sango_pack_content_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("icons.sangopack");
        pack.save(&file).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut library = Library::default();
        library.install(Pack::open(&file).unwrap()).unwrap();

        let mut view = PackContent::default();
        view.open = Some("icons".into());
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0)));
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| view.ui(ui, &mut library, &sd, None));
        });
        assert!(view.textures.get("pack:icons:species:zap").map_or(false, |t| t.is_some()), "the picture of the pack is drawn");
        assert!(view.textures.get("pack:icons:item:4").map_or(false, |t| t.is_some()), "the item icon of the pack is drawn");
        assert_eq!(library.write_item_icons(&sd).unwrap(), 1);
        library.set_enabled("icons", "item_icon:4", false);
        assert_eq!(library.write_item_icons(&sd).unwrap(), 0);
        // Turning an icon off and applying leaves it off the SD card.
        assert_eq!(library.write_icons(&sd).unwrap(), 1);
        library.set_enabled("icons", "icon:zap", false);
        assert_eq!(library.write_icons(&sd).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
