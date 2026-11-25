//! The Font page: the fonts of the game.
//!
//! The fonts are in the archive `a/1/6/7`. Each file is a font of the 3DS (see [`bcfnt`]). The page shows the glyphs of a font and a
//! preview of a text, exports the sheet of glyphs of a font (a PNG picture and a file of widths, see [`sheet`]) and reads it again after the
//! user drew over it, and makes the glyphs from a font of the computer (see [`ttf`]). A replaced font goes on the SD card as `font/<n>.bin` (the bytes of the file, not compressed) and the plugin reads
//! it in place of the font of the game.
//!
//! The game has one font file for each number, except a few numbers that have a file for each language. The plugin replaces a file by its
//! number only, so the page replaces the fonts that have one file.

pub mod bcfnt;
pub mod sheet;
pub mod ttf;

use bcfnt::Font;
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of the archive of the fonts (`a/1/6/7`).
pub const ARCHIVE: usize = 167;

/// The folder of the replaced fonts on the SD card.
const FOLDER: &str = "font";

/// The biggest side of the picture of all the glyphs (the smallest limit of the graphic cards that the program supports).
const ATLAS_SIDE: usize = 2048;

/// The text that the preview shows at first.
const SAMPLE: &str = "Pokémon Alpha Sapphire 0123456789 ABCabc あいう アイウ";

/// A font file of the archive.
struct Entry {
    id: usize,
    language: usize,
    /// The number of files that the number has (one for each language).
    variants: usize,
}

/// A font that is open.
struct Open {
    id: usize,
    language: usize,
    font: Font,
    /// True when the font comes from the SD card.
    replaced: bool,
    /// True when the font on the page was changed and is not on the SD card yet.
    dirty: bool,
    /// All the glyphs in one texture, `columns` on a line.
    texture: egui::TextureHandle,
    stride: (usize, usize),
    /// The number of glyphs on a line of the picture.
    columns: usize,
    atlas_size: (usize, usize),
    codes: Vec<Option<u16>>,
    selected: Option<usize>,
}

/// The Font page.
pub struct FontPage {
    /// The windows of the buttons Export…, Import… and Reset….
    export_open: bool,
    import_open: bool,
    reset_open: bool,
    opened_for: Option<(PathBuf, u32)>,
    entries: Vec<Entry>,
    open: Option<Open>,
    /// The file that is chosen: the number and the language.
    chosen: (usize, usize),
    text: String,
    zoom: usize,
    /// The font files of the computer, read when the page needs them.
    system: Option<Vec<ttf::SystemFont>>,
    pc_filter: String,
    pc_choice: Option<usize>,
    pc_settings: ttf::Settings,
    status: Option<(bool, String)>,
    /// The counter of the changes of this page, so that the list is read again.
    version: u32,
}

impl Default for FontPage {
    fn default() -> Self {
        FontPage { export_open: false, import_open: false, reset_open: false, opened_for: None, entries: Vec::new(), open: None, chosen: (0, 0), text: SAMPLE.to_string(), zoom: 2, system: None, pc_filter: String::new(), pc_choice: None, pc_settings: ttf::Settings { size: 20.0, latin_only: true, outline: true }, status: None, version: 0 }
    }
}

fn file_name(id: usize) -> String {
    format!("{FOLDER}/{id}.bin")
}

/// The ink of a glyph as pixels of the preview: dark on white.
fn ink_pixel(value: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(20, 24, 40, value)
}

/// The color behind the glyphs: white for ink, grey-blue for a font that has white letters.
fn preview_background(has_luminance: bool) -> egui::Color32 {
    if has_luminance {
        egui::Color32::from_rgb(110, 130, 160)
    } else {
        egui::Color32::WHITE
    }
}

/// What a font of the archive is for. The names of the languages follow the order of the language codes of the 3DS (1 Japanese, 2 English, 3
/// French, 4 Italian, 5 German, 7 Spanish, 8 Korean): this order is a guess that was not checked in the game.
pub fn role(id: usize, language: usize, variants: usize) -> String {
    let language_name = match language {
        0 => "default",
        1 => "Japanese",
        2 => "English",
        3 => "French",
        4 => "Italian",
        5 => "German",
        7 => "Spanish",
        8 => "Korean",
        _ => "other",
    };
    match (id, variants > 1) {
        (_, true) => format!("Number font: HP, levels ({language_name}, language {language})"),
        (0, _) => "System font (Japanese, US and Europe)".to_string(),
        (1, _) => "System font (Korean)".to_string(),
        (4, _) => "Braille (not checked in the game)".to_string(),
        (2, _) => "Battle font (nicknames)".to_string(),
        _ => "Font".to_string(),
    }
}

impl FontPage {
    fn read_list(&mut self, dump: &Dump, sd: &SdCard) {
        self.opened_for = Some((sd.root().to_path_buf(), self.version));
        self.entries.clear();
        self.open = None;
        let Ok(garc) = Garc::open(&dump.archive(ARCHIVE)) else {
            self.status = Some((false, "The archive of the fonts is not in the dump.".into()));
            return;
        };
        for id in 0..garc.file_count() {
            let languages = garc.languages(id);
            for &language in &languages {
                if let Some(bytes) = garc.file(id, language) {
                    if bytes.get(..4) == Some(b"CFNT") {
                        self.entries.push(Entry { id, language, variants: languages.len() });
                    }
                }
            }
        }
        if !self.entries.iter().any(|e| (e.id, e.language) == self.chosen) {
            self.chosen = self.entries.first().map_or((0, 0), |e| (e.id, e.language));
        }
    }

    /// Opens the chosen font: the file of the SD card when there is one (and the font has one file only), else the file of the game.
    fn open_chosen(&mut self, ctx: &egui::Context, dump: &Dump, sd: &SdCard) {
        let (id, language) = self.chosen;
        let Ok(garc) = Garc::open(&dump.archive(ARCHIVE)) else { return };
        let single = garc.languages(id).len() == 1;
        let from_card = if single { sd.read(&file_name(id)) } else { None };
        let replaced = from_card.is_some();
        let bytes = match from_card.or_else(|| garc.file(id, language).map(|b| b.to_vec())) {
            Some(bytes) => bytes,
            None => return,
        };
        match Font::parse(bytes) {
            Ok(font) => {
                let (texture, stride, atlas_size, columns) = Self::make_atlas(ctx, &font);
                let codes = font.code_of_each_glyph();
                self.open = Some(Open { id, language, font, replaced, dirty: false, texture, stride, columns, atlas_size, codes, selected: None });
            }
            Err(message) => {
                self.open = None;
                self.status = Some((false, format!("The font {id} cannot be read: {message}")));
            }
        }
    }

    /// One texture with every glyph: a cell of the size of the glyph and a border of 2 pixels, `columns` on a line.
    fn make_atlas(ctx: &egui::Context, font: &Font) -> (egui::TextureHandle, (usize, usize), (usize, usize), usize) {
        let (image, stride, columns) = Self::atlas_image(font);
        let size = (image.size[0], image.size[1]);
        (ctx.load_texture("font_atlas", image, egui::TextureOptions::NEAREST), stride, size, columns)
    }

    /// The picture of all the glyphs. In a font with a luminance, the white fill and the black outline keep their colors.
    fn atlas_image(font: &Font) -> (egui::ColorImage, (usize, usize), usize) {
        let (w, h) = (font.cell_width as usize, font.cell_height as usize);
        let stride = (w + 2, h + 2);
        let count = font.glyph_count().max(1);
        let columns = (ATLAS_SIDE / stride.0).clamp(1, count);
        let size = (columns * stride.0, count.div_ceil(columns) * stride.1);
        let mut pixels = vec![egui::Color32::TRANSPARENT; size.0 * size.1];
        for index in 0..font.glyph_count() {
            let pixels_of = font.glyph_pixels(index);
            let (ox, oy) = ((index % columns) * stride.0 + 1, (index / columns) * stride.1 + 1);
            for y in 0..h {
                for x in 0..w {
                    let (l, a) = pixels_of[y * w + x];
                    pixels[(oy + y) * size.0 + ox + x] = if font.has_luminance() { egui::Color32::from_rgba_unmultiplied(l, l, l, a) } else { ink_pixel(a) };
                }
            }
        }
        (egui::ColorImage { size: [size.0, size.1], pixels }, stride, columns)
    }

    /// The part of the texture for a glyph.
    fn glyph_uv(open: &Open, index: usize) -> egui::Rect {
        let (sw, sh) = (open.stride.0 as f32, open.stride.1 as f32);
        let (w, h) = (open.atlas_size.0 as f32, open.atlas_size.1 as f32);
        let (x, y) = ((index % open.columns) as f32 * sw + 1.0, (index / open.columns) as f32 * sh + 1.0);
        egui::Rect::from_min_max(egui::pos2(x / w, y / h), egui::pos2((x + open.font.cell_width as f32) / w, (y + open.font.cell_height as f32) / h))
    }

    fn list_card(&mut self, ui: &mut egui::Ui) -> bool {
        let mut picked = false;
        widget::card(ui, |ui| {
            ui.label(widget::hint("The fonts of the game. A number that has a file for each language shows each language."));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for entry in &self.entries {
                    let label = format!("{} · {}", entry.id, role(entry.id, entry.language, entry.variants));
                    if widget::tab_button(ui, &label, self.chosen == (entry.id, entry.language), false).clicked() {
                        self.chosen = (entry.id, entry.language);
                        picked = true;
                    }
                }
            });
        });
        picked
    }

    /// Puts a new font on the page. It is not on the SD card until the user saves it.
    fn show(&mut self, ctx: &egui::Context, font: Font) {
        if let Some(open) = self.open.as_mut() {
            let (texture, stride, atlas_size, columns) = Self::make_atlas(ctx, &font);
            open.codes = font.code_of_each_glyph();
            open.font = font;
            open.texture = texture;
            open.stride = stride;
            open.atlas_size = atlas_size;
            open.columns = columns;
            open.dirty = true;
        }
    }

    /// Writes the sheet of the font (a PNG picture) and the file of the widths next to it.
    fn export_sheet(&self, id: usize) -> (bool, String) {
        let Some(open) = self.open.as_ref() else { return (false, String::new()) };
        let Some(path) = rfd::FileDialog::new().set_title("Export the sheet of glyphs").set_file_name(format!("font_{id}.png")).add_filter("PNG picture", &["png"]).save_file() else { return (true, String::new()) };
        let csv_path = path.with_extension("csv");
        let result = sheet::write_png(&sheet::draw(&open.font)).and_then(|png| {
            std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
            std::fs::write(&csv_path, sheet::metrics_csv(&open.font)).map_err(|e| format!("{}: {e}", csv_path.display()))
        });
        match result {
            Ok(()) => (true, format!("Exported {} and {}. Draw over the cells of the PNG, keep its size, then import it again.", path.display(), csv_path.display())),
            Err(e) => (false, e),
        }
    }

    /// Reads a sheet (and the file of widths with the same name, when there is one) and shows the result.
    fn import_sheet(&mut self, ctx: &egui::Context) -> (bool, String) {
        let Some(path) = rfd::FileDialog::new().set_title("Sheet of glyphs").add_filter("PNG picture", &["png"]).pick_file() else { return (true, String::new()) };
        let csv = std::fs::read_to_string(path.with_extension("csv")).ok();
        let Some(open) = self.open.as_ref() else { return (false, String::new()) };
        let result = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|bytes| sheet::read_png(&bytes)).and_then(|picture| sheet::import(&open.font, &picture, csv.as_deref()));
        match result {
            Ok(done) => {
                let message = format!(
                    "Sheet read: {} glyphs changed, widths from {}. Check the preview, then save.",
                    done.changed,
                    if done.widths_from_file { "the .csv file" } else { "the red bars and the pictures (no .csv file next to the PNG)" }
                );
                self.show(ctx, done.font);
                (true, message)
            }
            Err(e) => (false, e),
        }
    }

    fn actions(&mut self, ui: &mut egui::Ui, sd: &SdCard, reload: &mut bool) {
        let Some(open) = self.open.as_ref() else { return };
        let single = self.entries.iter().find(|e| (e.id, e.language) == (open.id, open.language)).is_some_and(|e| e.variants == 1);
        let (id, language, replaced, dirty) = (open.id, open.language, open.replaced, open.dirty);
        let mut clicks = widget::ActionClicks::default();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                clicks = widget::action_bar(ui, widget::ActionBar { export: Some(true), import: Some(true), reset: Some(replaced), save: Some(dirty && single), ..Default::default() });
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if replaced {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if !single {
                ui.label(widget::hint(&format!("This number has a file for each language (this is language {language}): it cannot be saved for the game yet. You can still export it.")));
            }
            if let Some((ok, message)) = &self.status {
                if !message.is_empty() {
                    ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
                }
            }
        });
        if clicks.save {
            self.status = Some(self.save(sd, id));
            *reload = true;
        }
        self.export_open |= clicks.export;
        self.import_open |= clicks.import;
        self.reset_open |= clicks.reset;
        // Export… and Import…: the sheet of the glyphs (a PNG and a CSV file of the widths) or the font file.
        for import in [false, true] {
            let shown = if import { self.import_open } else { self.export_open };
            if !shown {
                continue;
            }
            let (mut open_window, mut sheet, mut file, mut close) = (true, false, false, false);
            egui::Window::new(if import { "Import" } else { "Export" }).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open_window).show(ui.ctx(), |ui| {
                ui.set_max_width(460.0);
                ui.label(widget::hint("The sheet: a PNG picture of the glyphs and a CSV file of their widths. Draw over the PNG and keep its size."));
                ui.horizontal(|ui| {
                    sheet = widget::button(ui, "Sheet (PNG + CSV)");
                    file = widget::button(ui, "Font file (.bcfnt)");
                    close = widget::button(ui, "Cancel");
                });
            });
            if sheet {
                if import {
                    let ctx = ui.ctx().clone();
                    self.status = Some(self.import_sheet(&ctx));
                } else {
                    self.status = Some(self.export_sheet(id));
                }
            }
            if file {
                if import {
                    if let Some(path) = rfd::FileDialog::new().set_title("Font to use").add_filter("Font", &["bcfnt", "bin"]).pick_file() {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(Font::parse) {
                            Ok(font) => {
                                let ctx = ui.ctx().clone();
                                self.show(&ctx, font);
                                self.status = Some((true, "File read. Check the preview, then save.".into()));
                            }
                            Err(e) => self.status = Some((false, format!("This file is not a font that the game can use: {e}"))),
                        }
                    }
                } else if let Some(path) = rfd::FileDialog::new().set_title("Export the font").set_file_name(format!("font_{id}.bcfnt")).add_filter("Font", &["bcfnt"]).save_file() {
                    let bytes = self.open.as_ref().map(|o| o.font.bytes().to_vec()).unwrap_or_default();
                    self.status = Some(match std::fs::write(&path, bytes) {
                        Ok(()) => (true, format!("Exported to {}.", path.display())),
                        Err(e) => (false, format!("{}: {e}", path.display())),
                    });
                }
            }
            if sheet || file || close || !open_window {
                if import {
                    self.import_open = false;
                } else {
                    self.export_open = false;
                }
            }
        }
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &format!("The font {id}"), &mut []) {
            self.status = Some(match sd.remove(&file_name(id)).and_then(|_| sd.mark_changed(&file_name(id))) {
                Ok(_) => (true, "The font of the game is used again. Restart the game.".to_string()),
                Err(e) => (false, e),
            });
            *reload = true;
        }
        ui.add_space(6.0);
        self.generator_card(ui);
    }

    /// Makes the glyphs from a font of the computer.
    fn generator_card(&mut self, ui: &mut egui::Ui) {
        let Some(open) = self.open.as_ref() else { return };
        let (has_luminance, height) = (open.font.has_luminance(), open.font.height);
        if self.system.is_none() {
            self.system = Some(ttf::system_fonts());
        }
        let system = self.system.clone().unwrap_or_default();
        widget::card(ui, |ui| {
            ui.label(egui::RichText::new("Make the glyphs from a font of the computer").strong());
            ui.label(widget::hint(
                "Choose a .ttf or .otf font (the fonts of Windows and of the user are listed). The program draws each character that the game font has, on its \
                 baseline, and sets the widths. The symbols and the characters that the computer font does not have keep the picture of the game. Then export \
                 the sheet to retouch the glyphs by hand.",
            ));
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Search"));
                ui.add(egui::TextEdit::singleline(&mut self.pc_filter).desired_width(160.0));
                let filter = self.pc_filter.to_lowercase();
                let shown = self.pc_choice.and_then(|i| system.get(i)).map_or("Choose a font".to_string(), |f| f.name.clone());
                egui::ComboBox::from_id_salt("font_pc_choice").selected_text(shown).width(260.0).height(360.0).show_ui(ui, |ui| {
                    for (i, font) in system.iter().enumerate().filter(|(_, f)| filter.is_empty() || f.name.to_lowercase().contains(&filter)) {
                        ui.selectable_value(&mut self.pc_choice, Some(i), &font.name);
                    }
                });
                if widget::button(ui, "Other file…") {
                    if let Some(path) = rfd::FileDialog::new().set_title("Font of the computer").add_filter("Font", &["ttf", "otf"]).pick_file() {
                        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        let list = self.system.get_or_insert_with(Vec::new);
                        list.push(ttf::SystemFont { name, path });
                        self.pc_choice = Some(list.len() - 1);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Size in pixels"));
                ui.add(egui::DragValue::new(&mut self.pc_settings.size).range(4.0..=120.0).speed(0.25));
                if widget::button(ui, "Size of the game font") {
                    self.pc_settings.size = f32::from(height) * 0.8;
                }
                ui.checkbox(&mut self.pc_settings.latin_only, "Latin characters only");
                if has_luminance {
                    ui.checkbox(&mut self.pc_settings.outline, "White letters with a black outline");
                }
            });
            let chosen = self.pc_choice.and_then(|i| system.get(i)).cloned();
            ui.add_enabled_ui(chosen.is_some(), |ui| {
                if widget::button(ui, "Make the glyphs") {
                    if let (Some(choice), Some(open)) = (chosen, self.open.as_ref()) {
                        let result = std::fs::read(&choice.path).map_err(|e| format!("{}: {e}", choice.path.display())).and_then(|bytes| ttf::generate(&open.font, &bytes, &self.pc_settings));
                        match result {
                            Ok((font, report)) => {
                                let ctx = ui.ctx().clone();
                                self.show(&ctx, font);
                                self.status = Some((
                                    true,
                                    format!(
                                        "{} glyphs drawn from {}; {} characters are not in that font; {} glyphs are cut by the cell. Check the preview, then save.",
                                        report.drawn, choice.name, report.missing, report.cut
                                    ),
                                ));
                            }
                            Err(e) => self.status = Some((false, e)),
                        }
                    }
                }
            });
        });
    }

    /// Writes the font on the page on the SD card in place of the font `id`.
    fn save(&self, sd: &SdCard, id: usize) -> (bool, String) {
        let Some(open) = self.open.as_ref() else { return (false, String::new()) };
        match sd.write(&file_name(id), open.font.bytes()).and_then(|_| sd.mark_changed(&file_name(id))) {
            Ok(_) => (true, format!("Saved: the font {id} is on the SD card. Restart the game to see it; this is not tested in the game.")),
            Err(e) => (false, e),
        }
    }

    /// Checks a font file and writes it on the SD card in place of the font `id` (used by the tests).
    #[cfg(test)]
    fn import(&self, sd: &SdCard, id: usize, path: &std::path::Path) -> (bool, String) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => return (false, format!("{}: {e}", path.display())),
        };
        if let Err(message) = Font::parse(bytes.clone()) {
            return (false, format!("This file is not a font that the game can use: {message}"));
        }
        match sd.write(&file_name(id), &bytes).and_then(|_| sd.mark_changed(&file_name(id))) {
            Ok(_) => (true, String::new()),
            Err(e) => (false, e),
        }
    }

    fn info_card(&self, ui: &mut egui::Ui) {
        let Some(open) = self.open.as_ref() else { return };
        let f = &open.font;
        widget::card(ui, |ui| {
            ui.label(format!(
                "Cell {}×{} · line feed {} · height {} · ascent {} · baseline {} · format {} · {} sheets of {}×{} ({}×{} glyphs) · {} glyphs, {} characters · {} maps",
                f.cell_width,
                f.cell_height,
                f.line_feed,
                f.height,
                f.ascent,
                f.baseline,
                f.format.name(),
                f.sheet_count,
                f.sheet_width,
                f.sheet_height,
                f.columns,
                f.rows,
                f.glyph_count(),
                f.codes().len(),
                f.maps.len()
            ));
        });
    }

    fn preview_card(&mut self, ui: &mut egui::Ui) {
        let zoom = self.zoom as f32;
        let Some(open) = self.open.as_ref() else { return };
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Text"));
                ui.add(egui::TextEdit::singleline(&mut self.text).desired_width(420.0));
                ui.label(widget::hint("Zoom"));
                for z in 1..=4 {
                    if widget::tab_button(ui, &format!("×{z}"), self.zoom == z, false).clicked() {
                        self.zoom = z;
                    }
                }
            });
            let (glyphs, width) = open.font.layout(&self.text);
            let height = open.font.cell_height as f32 * zoom;
            let (rect, _) = ui.allocate_exact_size(egui::vec2((width.max(1) as f32 + open.font.cell_width as f32) * zoom, height + 6.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, egui::Rounding::same(4.0), preview_background(open.font.has_luminance()));
            for (index, x) in glyphs {
                let at = egui::pos2(rect.left() + x as f32 * zoom, rect.top() + 3.0);
                let target = egui::Rect::from_min_size(at, egui::vec2(open.font.cell_width as f32 * zoom, height));
                ui.painter().image(open.texture.id(), target, Self::glyph_uv(open, index as usize), egui::Color32::WHITE);
            }
        });
    }

    fn grid_card(&mut self, ui: &mut egui::Ui) {
        let zoom = self.zoom as f32;
        let Some(open) = self.open.as_mut() else { return };
        widget::card(ui, |ui| {
            ui.label(widget::hint("All the glyphs, in the order of the sheets. Click a glyph to see its character and its widths."));
            if let Some(index) = open.selected {
                let width = open.font.width_of(index as u16);
                let code = open.codes.get(index).copied().flatten();
                let character = code.and_then(|c| char::from_u32(u32::from(c))).filter(|c| !c.is_control()).map(|c| format!(" “{c}”")).unwrap_or_default();
                let code_text = code.map_or("no character".to_string(), |c| format!("U+{c:04X}"));
                ui.label(format!("Glyph {index} · {code_text}{character} · left {} · picture {} · advance {}", width.left, width.glyph, width.advance));
            }
            let size = egui::vec2(open.atlas_size.0 as f32 * zoom, open.atlas_size.1 as f32 * zoom);
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            ui.painter().rect_filled(rect, egui::Rounding::ZERO, preview_background(open.font.has_luminance()));
            ui.painter().image(open.texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            let cell = |index: usize| {
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + (index % open.columns) as f32 * open.stride.0 as f32 * zoom, rect.top() + (index / open.columns) as f32 * open.stride.1 as f32 * zoom),
                    egui::vec2(open.stride.0 as f32 * zoom, open.stride.1 as f32 * zoom),
                )
            };
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (cx, cy) = (((pos.x - rect.left()) / (open.stride.0 as f32 * zoom)) as usize, ((pos.y - rect.top()) / (open.stride.1 as f32 * zoom)) as usize);
                    let index = cy * open.columns + cx;
                    open.selected = (cx < open.columns && index < open.font.glyph_count()).then_some(index);
                }
            }
            if let Some(index) = open.selected {
                ui.painter().rect_stroke(cell(index), egui::Rounding::ZERO, egui::Stroke::new(1.5_f32, color::ACCENT));
            }
        });
    }
}

impl Module for FontPage {
    fn title(&self) -> &str {
        "Font"
    }

    fn icon(&self) -> &str {
        "🔤"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Font");
        ui.add_space(8.0);
        let (Some(dump), Some(sd)) = (session.dump.as_ref(), session.sd.clone()) else {
            widget::missing_card(ui, "Choose the dump of the game and the SD card on the Home page to see the fonts.");
            return;
        };
        if self.opened_for.as_ref() != Some(&(sd.root().to_path_buf(), self.version)) {
            self.read_list(dump, &sd);
        }
        let picked = self.list_card(ui);
        let stale = self.open.as_ref().map_or(true, |o| (o.id, o.language) != self.chosen);
        if picked || (stale && !self.entries.is_empty()) {
            self.open_chosen(ui.ctx(), dump, &sd);
        }
        ui.add_space(6.0);
        let mut reload = false;
        self.actions(ui, &sd, &mut reload);
        if reload {
            self.open = None;
        }
        ui.add_space(6.0);
        egui::ScrollArea::both().id_salt("font_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.info_card(ui);
            ui.add_space(6.0);
            self.preview_card(ui);
            ui.add_space(6.0);
            self.grid_card(ui);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: the page draws for each font, and replacing a font writes the file and the list on the card.
    #[test]
    fn the_page_draws_every_font_and_replaces_one() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_font_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = FontPage::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut FontPage, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        frame(&mut page, &mut session);
        assert!(page.entries.len() >= 5, "the page lists the fonts");
        let all: Vec<(usize, usize)> = page.entries.iter().map(|e| (e.id, e.language)).collect();
        for chosen in all {
            page.chosen = chosen;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
            assert!(page.open.is_some(), "font {chosen:?} opens");
        }
        // Replace the font 2 by a copy of itself: it comes from the card afterwards.
        let sd = session.sd.clone().unwrap();
        let garc = Garc::open(&session.dump.as_ref().unwrap().archive(ARCHIVE)).unwrap();
        let file = folder.join("font2.bcfnt");
        std::fs::write(&file, garc.first_file(2).unwrap()).unwrap();
        let (ok, message) = page.import(&sd, 2, &file);
        assert!(ok, "{message}");
        assert!(sd.read("font/2.bin").is_some());
        std::fs::write(&file, b"not a font").unwrap();
        assert!(!page.import(&sd, 2, &file).0, "a bad file is refused");
        page.chosen = (2, 0);
        page.version += 1;
        frame(&mut page, &mut session);
        frame(&mut page, &mut session);
        assert!(page.open.as_ref().unwrap().replaced);
        let font = page.open.as_ref().unwrap().font.clone();
        page.show(&ctx, font);
        assert!(page.open.as_ref().unwrap().dirty);
        assert!(page.save(&sd, 2).0);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: in a battle font the picture has a white fill and a black outline (two different colors), and a sheet drawn with
    /// these two colors is read back as the two values of the game.
    #[test]
    fn a_battle_font_shows_its_fill_and_its_outline() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let garc = Garc::open(&Dump::open(&root).unwrap().archive(ARCHIVE)).unwrap();
        let font = Font::parse(garc.file(3, 2).unwrap().to_vec()).unwrap();
        let (image, _, _) = FontPage::atlas_image(&font);
        let white = image.pixels.iter().filter(|p| **p == egui::Color32::WHITE).count();
        let black = image.pixels.iter().filter(|p| **p == egui::Color32::BLACK).count();
        assert!(white > 500 && black > 1000, "white {white}, black {black}");
        let mut picture = sheet::draw(&font);
        let (u, v) = sheet::block_size(&font);
        // The first two pixels of the cell of glyph 0: a white pixel, then a black one.
        let at = (2 * picture.width + 2) * 4;
        picture.rgba[at..at + 8].copy_from_slice(&[255, 255, 255, 255, 0, 0, 0, 255]);
        let _ = (u, v);
        let done = sheet::import(&font, &picture, None).unwrap();
        let pixels = done.font.glyph_pixels(0);
        assert_eq!((pixels[0], pixels[1]), ((255, 255), (0, 255)));
    }
}
