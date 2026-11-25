//! The type editor.
//!
//! The page shows the type chart of the game (the effect of each attacking type against each defending type), and lets the
//! user change any cell, add new types (numbers 19 to 31), give a new type a name and an image, and give any type another
//! icon. The files that the user changes go on the SD card, in the folders `type` and `text`: the romfs is never changed.
//!
//! The game keeps its type chart in its code, not in a file of the romfs: the page starts from the chart of Omega Ruby and
//! Alpha Sapphire, and the file of the SD card holds the whole chart as the user edited it.

pub mod chart;
mod icons;
mod texts;
mod tools;
pub mod types;

use chart::{Chart, GAME_TYPES, MAX_CUSTOM};
use sango_core::{Dump, SdCard};
use sango_species_base::battle_icons;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;
use types::TypeList;

/// The size of a cell of the chart, and of the headers.
const CELL_W: f32 = 30.0;
const CELL_H: f32 = 26.0;
/// The width of the column of the names, on the left of the chart: the menu, the box of the name and the reset button.
const NAME_W: f32 = 200.0;
const HEAD_W: f32 = 58.0;
const HEAD_H: f32 = 64.0;

pub struct Type {
    /// The dump that the archives come from.
    root: Option<PathBuf>,
    /// False until the files of the SD card are read (and after a reset, an import or an undo).
    loaded: bool,
    /// The chart as the user has it, and as it was loaded or saved.
    chart: Chart,
    chart_baseline: Chart,
    /// The new types, and as they were loaded or saved.
    list: TypeList,
    list_baseline: TypeList,
    icons: icons::TypeIcons,
    texts: texts::TypeTexts,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    tools: tools::Tools,
    /// The selected cell of the chart: the number of the attacking type and of the defending type.
    selected: (usize, usize),
    /// The English names of the types of the game, as the user has them.
    names: Vec<String>,
}

impl Default for Type {
    fn default() -> Self {
        Type {
            root: None,
            loaded: false,
            chart: Chart::original(),
            chart_baseline: Chart::original(),
            list: TypeList::default(),
            list_baseline: TypeList::default(),
            icons: icons::TypeIcons::default(),
            texts: texts::TypeTexts::default(),
            status: None,
            tools: tools::Tools::default(),
            selected: (9, 11),
            names: Vec::new(),
        }
    }
}

impl Type {
    /// Opens the archives of a dump.
    fn open(&mut self, dump: &Dump) {
        self.root = Some(dump.root().to_path_buf());
        self.texts.open(dump);
        battle_icons::load(dump);
        self.loaded = false;
    }

    /// Reads the files of the SD card: the chart, the new types and the icons.
    fn load(&mut self, session: &Session) {
        let sd = session.sd.as_ref();
        let mut problem = None;
        self.chart = match sd.and_then(|sd| sd.read(tools::CHART_PATH)) {
            Some(bytes) => Chart::from_bytes(&bytes).unwrap_or_else(|message| {
                problem = Some(format!("The chart file of the SD card is not used: {message}"));
                Chart::original()
            }),
            None => Chart::original(),
        };
        self.list = match sd.and_then(|sd| sd.read(types::SD_PATH)).map(|bytes| String::from_utf8_lossy(&bytes).to_string()) {
            Some(text) => TypeList::from_csv(&text).unwrap_or_else(|message| {
                problem = Some(format!("The types file of the SD card is not used: {message}"));
                TypeList::default()
            }),
            None => TypeList::default(),
        };
        self.chart_baseline = self.chart.clone();
        self.list_baseline = self.list.clone();
        self.icons.load_card(sd);
        self.names = (0..GAME_TYPES).map(|kind| self.texts.name(sd, kind)).collect();
        self.loaded = true;
        self.status = problem.map(|message| (false, message));
    }

    /// The colour type of a type: the type itself for a type of the game.
    fn colour_of(&self, id: usize) -> u8 {
        self.list.find(id).map_or(id.min(17) as u8, |c| c.colour)
    }

    /// Adds a type with the next free number. It starts with the effects ×1 against every type, the name "New type" in every
    /// language, and the colour of the buttons of the type that is selected in the chart. The type is then edited like the others.
    fn add_type(&mut self) {
        let colour = self.colour_of(self.selected.0).min(17);
        match self.list.add("New type", colour) {
            Some(id) => {
                self.chart.clear_type(id);
                self.status = Some((true, format!("The type {id} was added. Change its name on the left of its row, its effects in its row and its column, then Save.")));
            }
            None => self.status = Some((false, "The list of the new types is full.".into())),
        }
    }

    /// True when something changed and was not saved.
    fn dirty(&self) -> bool {
        self.chart != self.chart_baseline || self.list != self.list_baseline || self.icons.changed() || self.texts.changed()
    }

    /// Writes the chart, the new types, the icons and the names on the SD card, and tells the plugin.
    fn save(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        if let Some(custom) = self.list.custom.iter().find(|c| types::name_problem(&c.name).is_some()) {
            let problem = types::name_problem(&custom.name).unwrap_or_default();
            self.status = Some((false, format!("The type {}: {problem}", custom.id)));
            return;
        }
        let result = (|| -> Result<(), String> {
            if self.chart != self.chart_baseline {
                if self.chart == Chart::original() {
                    sd.remove(tools::CHART_PATH)?;
                } else {
                    sd.write(tools::CHART_PATH, &self.chart.to_bytes()).and_then(|_| sd.mark_changed(tools::CHART_PATH))?;
                }
            }
            if self.list != self.list_baseline {
                if self.list.custom.is_empty() {
                    sd.remove(types::SD_PATH)?;
                } else {
                    sd.write(types::SD_PATH, self.list.to_csv().as_bytes()).and_then(|_| sd.mark_changed(types::SD_PATH))?;
                }
            }
            self.icons.save(sd)?;
            self.texts.save(sd)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.chart_baseline = self.chart.clone();
                self.list_baseline = self.list.clone();
                self.names = (0..GAME_TYPES).map(|kind| self.texts.name(Some(sd), kind)).collect();
                self.status = Some((true, "Saved. The files are on the SD card.".into()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut save = false;
        let mut reset_more = false;
        let mut export = false;
        let mut import = false;
        let mut add = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let has_card = session.sd.is_some();
                ui.add_enabled_ui(has_card && self.list.custom.len() < MAX_CUSTOM, |ui| {
                    add = widget::button_styled(ui, egui::RichText::new("+ New type").color(color::WHITE), Some(color::ACCENT));
                })
                .response
                .on_hover_text("Adds a type with the effects ×1 against every type. Then you change it like the other types.");
                export = widget::button(ui, "Export…");
                ui.add_enabled_ui(has_card, |ui| {
                    import = widget::button(ui, "Import…");
                })
                .response
                .on_hover_text("Reads the CSV files of the types.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Reset the chart, the new types or the names, for the parts that you choose.");
                save = has_card && widget::save_button(ui, self.unsaved());
                if self.dirty() {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.chart_baseline != Chart::original() || !self.list_baseline.custom.is_empty() || self.icons.from_card() || self.texts.from_card() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "Chart of the game", color::GREEN);
                }
                ui.label(widget::hint(&format!("{} types, {} cells changed", GAME_TYPES + self.list.custom.len(), self.chart.changed_count())));
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
        if add {
            self.add_type();
        }
        if reset_more {
            self.tools.reset_open = true;
        }
        if export {
            self.tools.export_open = true;
        }
        if import {
            self.tools.import_open = true;
        }
        if save {
            self.save(session);
        }
    }

    /// The fill and the text colour of a cell.
    fn cell_colours(value: u8) -> (egui::Color32, egui::Color32) {
        match value {
            chart::DOUBLE => (egui::Color32::from_rgb(126, 211, 130), egui::Color32::from_rgb(20, 70, 30)),
            chart::HALF => (egui::Color32::from_rgb(245, 150, 150), egui::Color32::from_rgb(100, 20, 20)),
            chart::NONE => (egui::Color32::from_rgb(70, 70, 82), egui::Color32::WHITE),
            _ => (egui::Color32::TRANSPARENT, color::MUTED),
        }
    }

    /// The name of a type in the chosen language, for the icons and the bubbles.
    fn name_in_language(&mut self, sd: Option<&SdCard>, id: usize) -> String {
        match self.list.find(id) {
            Some(custom) => custom.name_in(self.texts.language()).to_string(),
            None => self.texts.text(sd, id),
        }
    }

    /// The row of the names of a type, on the left of its row of the chart: the menu of the type, the box of the name in the chosen
    /// language, and the button that puts the name of the game back.
    fn name_row(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, id: usize, asked: &mut Option<RowAction>) {
        let language = self.texts.language();
        let has_override = self.icons.has_override(id, language);
        ui.menu_button("⋯", |ui| {
            if ui.button("Export the icon (PNG)…").clicked() {
                *asked = Some(RowAction::Icon(IconAction::Export, id, language));
                ui.close_menu();
            }
            if ui.button("Import an icon (PNG)…").clicked() {
                *asked = Some(RowAction::Icon(IconAction::Import, id, language));
                ui.close_menu();
            }
            if ui.add_enabled(has_override, egui::Button::new("Put the icon of the game back")).clicked() {
                *asked = Some(RowAction::Icon(IconAction::Reset, id, language));
                ui.close_menu();
            }
            if let Some(entry) = self.list.custom.iter_mut().find(|c| c.id == id) {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Colour of the buttons in battle");
                    let mut colour = entry.colour;
                    sango_species_base::game_type_menu(ui, &format!("type_colour_{id}"), &mut colour);
                    entry.colour = colour;
                });
                if ui.button("Remove this type").clicked() {
                    *asked = Some(RowAction::Remove(id));
                    ui.close_menu();
                }
            }
        })
        .response
        .on_hover_text("The icon of the type in this language, and more");
        let changed = if let Some(entry) = self.list.custom.iter_mut().find(|c| c.id == id) {
            // A new type: its names are in the list of the types. Without a name of its own, a language shows the English name.
            let mut text = entry.name_in(language).to_string();
            let response = sango_ui::game_text::edit_line(ui, ("type_new_name", language, id), &mut text, NAME_W - 52.0);
            if response.changed() {
                if language == texts::ENGLISH {
                    entry.rename(&text);
                } else {
                    entry.translations[language] = text;
                }
            }
            if let Some(problem) = types::name_problem(entry.name_in(language)) {
                ui.painter().rect_stroke(response.rect, 3.0, egui::Stroke::new(1.5_f32, color::ROSE));
                response.on_hover_text(egui::RichText::new(problem).color(color::ROSE));
            }
            language != texts::ENGLISH && !entry.translations[language].is_empty() && entry.translations[language] != entry.name
        } else {
            self.texts.name_field(ui, sd, id, NAME_W - 52.0);
            self.texts.is_changed(sd, id)
        };
        let hint = if self.list.find(id).is_some() { "Shows the English name in this language again" } else { "Puts the name of the game back, in this language" };
        if ui.add_enabled(changed, egui::Button::new("↺").small()).on_hover_text(hint).clicked() {
            *asked = Some(RowAction::ResetName(id));
        }
    }

    /// The page: the menu of the language, then the chart with the name of each type on the left of its row.
    fn page_view(&mut self, ui: &mut egui::Ui, session: &Session) {
        let sd = session.sd.as_ref();
        let mut asked = None;
        egui::ScrollArea::both().id_salt("type_chart_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let ids = self.list.ids();
            let names: Vec<String> = ids.iter().map(|&id| self.name_in_language(sd, id)).collect();
            let language = self.texts.language();
            widget::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Language");
                    self.texts.language_menu(ui);
                    ui.add_space(12.0);
                    ui.label(widget::hint("Row: the type of the move. Column: the type of the target. Click a cell: ×1, ×2, ×½, ×0."))
                        .on_hover_text("A right click goes back. With the keyboard: the arrows move, Space changes the cell, and 0, 5, 1 and 2 give ×0, ×½, ×1 and ×2.");
                });
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    // The names, on the left of the rows.
                    ui.vertical(|ui| {
                        ui.set_width(NAME_W);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add_space(HEAD_H);
                        for &id in &ids {
                            ui.horizontal(|ui| {
                                ui.set_height(CELL_H);
                                self.name_row(ui, sd, id, &mut asked);
                            });
                        }
                    });
                    ui.vertical(|ui| self.chart_grid(ui, &ids, &names, language));
                });
                ui.add_space(8.0);
                self.legend(ui);
            });
            ui.add_space(10.0);
        });
        match asked {
            Some(RowAction::Icon(action, id, language)) => self.icon_action(action, id, language),
            Some(RowAction::Remove(id)) => {
                self.list.remove(id);
                self.chart.clear_type(id);
                self.icons.reset_all(id);
            }
            Some(RowAction::ResetName(id)) => {
                let language = self.texts.language();
                match self.list.custom.iter_mut().find(|c| c.id == id) {
                    Some(entry) => entry.translations[language].clear(),
                    None => self.texts.reset_name(sd, id),
                }
            }
            None => {}
        }
    }

    /// The meaning of the colours of the cells, and the button that puts the chart of the game back.
    fn legend(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [(chart::DOUBLE, "×2 super effective"), (chart::HALF, "×½ not very effective"), (chart::NONE, "×0 no effect")] {
                let (fill, ink) = Self::cell_colours(value);
                let (swatch, _) = ui.allocate_exact_size(egui::vec2(18.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 3.0, fill);
                ui.painter().rect_stroke(swatch, 3.0, egui::Stroke::new(1.0_f32, ink));
                ui.label(widget::hint(label));
            }
            ui.label(egui::RichText::new("•").color(color::AMBER));
            ui.label(widget::hint("different from the game"));
            if widget::button(ui, "Put the chart of the game back") {
                self.chart = Chart::original();
            }
        });
    }

    /// The grid of the chart: the icons of the types in the chosen language on the top and on the left, and the cells.
    fn chart_grid(&mut self, ui: &mut egui::Ui, ids: &[usize], names: &[String], language: usize) {
        let n = ids.len();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(HEAD_W + n as f32 * CELL_W, HEAD_H + n as f32 * CELL_H), egui::Sense::hover());
        let grid_id = ui.make_persistent_id("type_chart_grid");
        widget::register_focusable(ui, grid_id, rect);
        let hovered = ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
            let (x, y) = (pos.x - rect.left() - HEAD_W, pos.y - rect.top() - HEAD_H);
            (x >= 0.0 && y >= 0.0 && ((x / CELL_W) as usize) < n && ((y / CELL_H) as usize) < n).then(|| ((y / CELL_H) as usize, (x / CELL_W) as usize))
        });
        // The icons of the headers.
        for (place, &id) in ids.iter().enumerate() {
            let colour = self.colour_of(id);
            // The icons of the columns are turned: they are as long as the icons of the rows.
            let column = egui::Rect::from_min_size(egui::pos2(rect.left() + HEAD_W + place as f32 * CELL_W + 4.0, rect.top() + 4.0), egui::vec2(CELL_W - 8.0, HEAD_H - 8.0));
            self.icons.paint_turned(ui, id, language, column, &names[place], colour);
            let row = egui::Rect::from_min_size(egui::pos2(rect.left() + 2.0, rect.top() + HEAD_H + place as f32 * CELL_H + 2.0), egui::vec2(HEAD_W - 6.0, CELL_H - 4.0));
            self.icons.paint(ui, id, language, row, &names[place], colour);
        }
        // The cells.
        let mut selected = ids.iter().position(|&id| id == self.selected.0).zip(ids.iter().position(|&id| id == self.selected.1)).unwrap_or((0, 0));
        for row in 0..n {
            for column in 0..n {
                let cell = egui::Rect::from_min_size(egui::pos2(rect.left() + HEAD_W + column as f32 * CELL_W, rect.top() + HEAD_H + row as f32 * CELL_H), egui::vec2(CELL_W, CELL_H)).shrink(1.0);
                let response = ui.interact(cell, grid_id.with((row, column)), egui::Sense::click());
                let (attacking, defending) = (ids[row], ids[column]);
                let value = self.chart.get(attacking, defending);
                if response.clicked() || response.secondary_clicked() {
                    self.chart.set(attacking, defending, chart::next(value, response.secondary_clicked()));
                    selected = (row, column);
                    widget::set_active_id(ui, grid_id);
                }
                let value = self.chart.get(attacking, defending);
                let (fill, ink) = Self::cell_colours(value);
                let on_line = hovered.is_some_and(|(r, c)| r == row || c == column);
                if on_line {
                    ui.painter().rect_filled(cell, 3.0, color::ACCENT_SOFT);
                }
                ui.painter().rect_filled(cell, 3.0, fill);
                if value != chart::NORMAL || on_line {
                    ui.painter().text(cell.center(), egui::Align2::CENTER_CENTER, chart::text(value).trim_start_matches('×'), egui::FontId::proportional(13.0), ink);
                }
                if self.chart.is_changed(attacking, defending) {
                    ui.painter().circle_filled(cell.right_top() + egui::vec2(-4.0, 4.0), 2.5, color::AMBER);
                }
                if hovered == Some((row, column)) {
                    ui.painter().rect_stroke(cell, 3.0, egui::Stroke::new(1.5_f32, color::ACCENT));
                }
                if response.hovered() {
                    let game = if attacking < GAME_TYPES && defending < GAME_TYPES { chart::text(chart::ORIGINAL[attacking][defending]) } else { chart::text(chart::NORMAL) };
                    response.on_hover_text(format!("{} attacking {}: {} (the game: {})", names[row], names[column], chart::text(value), game));
                }
            }
        }
        // The keyboard.
        self.selected = (ids[selected.0], ids[selected.1]);
        if widget::is_active(ui, grid_id) && widget::page_has_focus(ui) {
            let selected_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + HEAD_W + selected.1 as f32 * CELL_W, rect.top() + HEAD_H + selected.0 as f32 * CELL_H), egui::vec2(CELL_W, CELL_H));
            widget::glow(ui, selected_rect, 4.0);
            let (attacking, defending) = (ids[selected.0], ids[selected.1]);
            let mut moved = (selected.0 as isize, selected.1 as isize);
            let mut set = None;
            let current = self.chart.get(attacking, defending);
            ui.input_mut(|i| {
                for (key, dx, dy) in [(egui::Key::ArrowUp, 0, -1), (egui::Key::ArrowDown, 0, 1), (egui::Key::ArrowLeft, -1, 0), (egui::Key::ArrowRight, 1, 0)] {
                    if i.consume_key(egui::Modifiers::NONE, key) {
                        moved = (moved.0 + dy, moved.1 + dx);
                    }
                }
                for (key, value) in [(egui::Key::Num0, chart::NONE), (egui::Key::Num5, chart::HALF), (egui::Key::Num1, chart::NORMAL), (egui::Key::Num2, chart::DOUBLE)] {
                    if i.consume_key(egui::Modifiers::NONE, key) {
                        set = Some(value);
                    }
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Space) || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                    set = Some(chart::next(current, false));
                }
            });
            if let Some(value) = set {
                self.chart.set(attacking, defending, value);
            }
            let (row, column) = (moved.0.clamp(0, n as isize - 1) as usize, moved.1.clamp(0, n as isize - 1) as usize);
            self.selected = (ids[row], ids[column]);
        }
    }

    /// Does what a click on a button of an icon asked: a PNG file goes in or out, or the icon of the game comes back.
    fn icon_action(&mut self, action: IconAction, id: usize, language: usize) {
        match action {
            IconAction::Reset => self.icons.reset(id, language),
            IconAction::Import => {
                if let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() {
                    self.status = Some(match self.icons.import(id, language, &path) {
                        Ok(()) => (true, "Icon imported. Save writes it on the SD card.".to_string()),
                        Err(message) => (false, message),
                    });
                }
            }
            IconAction::Export => {
                if let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name(format!("type_{id}_{}.png", sango_core::text::LANGUAGES[language].folder)).save_file() {
                    self.status = Some(match self.icons.export(id, language, &path) {
                        Ok(()) => (true, format!("Exported the icon to {}.", path.display())),
                        Err(message) => (false, message),
                    });
                }
            }
        }
    }
}

/// What the menu or the reset button of a row asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowAction {
    /// A button of the icon: the action, the type and the language.
    Icon(IconAction, usize, usize),
    /// A new type is removed.
    Remove(usize),
    /// The name of the type in the chosen language goes back to the name of the game.
    ResetName(usize),
}

/// What a button of an icon asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IconAction {
    Export,
    Import,
    Reset,
}

impl Module for Type {
    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn title(&self) -> &str {
        "Type"
    }

    fn icon(&self) -> &str {
        "🔥"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Type");
        ui.add_space(8.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        if self.root.as_deref() != Some(dump.root()) {
            self.open(&dump);
        }
        if !self.loaded {
            self.load(session);
        }
        let session: &Session = session;
        self.top_bar(ui, session);
        self.reset_window(ui.ctx(), session);
        self.csv_windows(ui.ctx(), session);
        ui.add_space(8.0);
        self.page_view(ui, session);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the page draws, a type is added and a cell is changed, and everything
    /// is saved and read again.
    #[test]
    fn the_page_draws_edits_and_saves() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_type_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Type::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Type, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for _ in 0..2 {
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        assert_eq!(page.names[0], "Normal");
        assert_eq!(page.chart.changed_count(), 0);

        // A name changed in a language comes back with the reset of the row.
        let sd = session.sd.clone();
        page.texts.reset_name(sd.as_ref(), 9);
        assert!(!page.texts.is_changed(sd.as_ref(), 9));
        assert_eq!(page.texts.text(sd.as_ref(), 9), "Fire");

        // A new type and a changed cell are saved.
        let id = page.list.add("Cosmic", 15).unwrap();
        page.chart.set(id, 0, chart::DOUBLE);
        page.chart.set(9, 11, chart::HALF);
        frame(&mut page, &mut session);
        assert!(page.dirty());
        page.save(&session);
        assert!(!page.dirty(), "{:?}", page.status);
        let sd = session.sd.as_ref().unwrap();
        assert!(sd.read(tools::CHART_PATH).is_some() && sd.read(types::SD_PATH).is_some());
        let mut again = Type::default();
        frame(&mut again, &mut session);
        assert_eq!(again.chart.get(id, 0), chart::DOUBLE);
        assert_eq!(again.chart.get(9, 11), chart::HALF);
        assert_eq!(again.list.find(id).map(|c| c.name.as_str()), Some("Cosmic"));

        // The chart of the game back removes the file.
        page.chart = Chart::original();
        page.list.remove(id);
        page.save(&session);
        let sd = session.sd.as_ref().unwrap();
        assert!(sd.read(tools::CHART_PATH).is_none() && sd.read(types::SD_PATH).is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}

#[cfg(test)]
mod add_test {
    use super::*;

    /// A new type is added with one click: the effects are ×1, the name is "New type" in every language, and it is edited like the
    /// others (its name, its picture).
    #[test]
    fn a_type_is_added_with_one_click_and_edited_like_the_others() {
        let folder = std::env::temp_dir().join(format!("sango_type_add_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let png = folder.join("cosmic.png");
        let mut picture = image::RgbaImage::new(icons::WIDTH as u32, icons::HEIGHT as u32);
        for pixel in picture.pixels_mut() {
            *pixel = image::Rgba([40, 30, 140, 255]);
        }
        picture.save(&png).unwrap();

        let mut page = Type::default();
        page.chart.set(19, 3, chart::DOUBLE);
        page.selected = (9, 11);
        page.add_type();
        let added = page.list.find(19).expect("the type has the first new number");
        assert_eq!(added.name, "New type");
        assert!((0..8).all(|language| added.name_in(language) == "New type"));
        assert!((0..chart::MAX_TYPES).all(|other| page.chart.get(19, other) == chart::NORMAL && page.chart.get(other, 19) == chart::NORMAL));
        assert!(matches!(page.status, Some((true, _))));
        // The type is edited like the others: its name, and its picture.
        page.list.custom[0].rename("Cosmic");
        page.icons.import(19, icons::ENGLISH, &png).unwrap();
        assert!(page.icons.has_override(19, icons::ENGLISH));
        assert!((0..8).all(|language| page.list.find(19).unwrap().name_in(language) == "Cosmic"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
