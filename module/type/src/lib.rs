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
use sango_species::battle_icons;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;
use types::TypeList;

/// The tabs of the page.
const TABS: [&str; 3] = ["General", "Text", "CSV"];

/// The place of the tab "CSV" in [`TABS`]: the import and the export of all the types.
const CSV_TAB: usize = 2;

/// The size of a cell of the chart, and of the headers.
const CELL_W: f32 = 30.0;
const CELL_H: f32 = 24.0;
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
    tab: usize,
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
            tab: 0,
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

    /// The name of a type: the English name of a type of the game, or the name of a new type.
    fn type_name(&self, id: usize) -> String {
        match self.list.find(id) {
            Some(custom) => custom.name.clone(),
            None => self.names.get(id).cloned().unwrap_or_default(),
        }
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
                self.status = Some((true, format!("The type {id} was added. Change its name and its icons in the Text tab, its effects in the General tab, then Save.")));
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
        let mut reset_tab = false;
        let mut reset_more = false;
        let mut undo = false;
        let mut add = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let has_card = session.sd.is_some();
                ui.add_enabled_ui(has_card && self.list.custom.len() < MAX_CUSTOM, |ui| {
                    add = widget::button_styled(ui, egui::RichText::new("+ New type").color(color::WHITE), Some(color::ACCENT));
                })
                .response
                .on_hover_text("Adds a type with the effects ×1 against every type. Then you change it like the other types.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_tab = widget::button(ui, "Reset tab");
                })
                .response
                .on_hover_text("Puts the data of this tab back to the romfs and to the chart of the game. Undo gives it back.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Reset the chart, the new types or the names, for the parts that you choose.");
                let undo_label = self.tools.history.last().map(|backup| format!("Undo: {}", backup.label));
                ui.add_enabled_ui(undo_label.is_some() && has_card, |ui| {
                    undo = widget::button(ui, "Undo");
                })
                .response
                .on_hover_text(undo_label.unwrap_or_else(|| "Nothing to undo.".into()));
                save = has_card && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
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
        if reset_tab {
            self.run_reset(session, &tools::tab_parts(self.tab));
        }
        if reset_more {
            self.tools.reset_open = true;
            self.tools.parts = [self.tab == 0, self.tab == 1, self.tab == 1];
        }
        if undo {
            self.undo(session);
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

    /// The General tab: the grid of the types.
    fn chart_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().id_salt("type_chart_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let ids = self.list.ids();
            let n = ids.len();
            let names: Vec<String> = ids.iter().map(|&id| self.type_name(id)).collect();
            widget::card(ui, |ui| {
                ui.label(widget::title("Type chart"));
                ui.label(widget::hint("The row is the type of the move, the column is the type of the target. A click changes a cell (×1, ×2, ×½, ×0); a right click goes back."));
                ui.label(widget::hint("With the keyboard: the arrows move, Space changes the cell, and 0, 5, 1 and 2 give ×0, ×½, ×1 and ×2."));
                ui.add_space(8.0);
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
                    self.icons.paint_turned(ui, id, icons::ENGLISH, column, &names[place], colour);
                    let row = egui::Rect::from_min_size(egui::pos2(rect.left() + 2.0, rect.top() + HEAD_H + place as f32 * CELL_H + 2.0), egui::vec2(HEAD_W - 6.0, CELL_H - 4.0));
                    self.icons.paint(ui, id, icons::ENGLISH, row, &names[place], colour);
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
                ui.add_space(8.0);
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
            });
            ui.add_space(10.0);
        });
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

/// What a button of an icon asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IconAction {
    Export,
    Import,
    Reset,
}

impl Module for Type {
    fn title(&self) -> &str {
        "Type"
    }

    fn icon(&self) -> &str {
        "T"
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
        ui.add_space(8.0);
        let mut clicked = None;
        egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(12.0)).inner_margin(egui::Margin::same(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, name) in TABS.iter().enumerate() {
                    let tab_id = egui::Id::new(format!("type_tab_{i}"));
                    let active = widget::is_active(ui, tab_id);
                    if active && self.tab != i {
                        self.tab = i;
                    }
                    let response = widget::tab_button(ui, name, self.tab == i, active && widget::page_has_focus(ui));
                    widget::register_focusable(ui, tab_id, response.rect);
                    ui.ctx().data_mut(|d| d.insert_temp(tab_id.with("is_button"), true));
                    if response.clicked() {
                        clicked = Some((i, tab_id));
                    }
                }
            });
        });
        if let Some((i, tab_id)) = clicked {
            self.tab = i;
            widget::set_active_id(ui, tab_id);
        }
        ui.add_space(6.0);
        match self.tab {
            0 => self.chart_tab(ui),
            CSV_TAB => {
                egui::ScrollArea::vertical().id_salt("type_csv_tab").auto_shrink([false, false]).show(ui, |ui| self.csv_card(ui, session));
            }
            _ => {
                let sd: Option<&SdCard> = session.sd.as_ref();
                let shown: Vec<(String, u8)> = (0..chart::MAX_TYPES).map(|id| (self.type_name(id), self.colour_of(id))).collect();
                let icons = &mut self.icons;
                let mut asked: Option<(IconAction, usize, usize)> = None;
                // The icon of the type in the language of the tab before the box of the name, its buttons after it.
                let removed = self.texts.ui(ui, sd, &mut self.list, &mut |ui, kind, language, buttons| {
                    if !buttons {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(icons::WIDTH as f32, icons::HEIGHT as f32), egui::Sense::hover());
                        let (name, colour) = shown.get(kind).cloned().unwrap_or_default();
                        icons.paint(ui, kind, language, rect, if kind < GAME_TYPES { "" } else { &name }, colour);
                        return;
                    }
                    if widget::button(ui, "Export PNG…") {
                        asked = Some((IconAction::Export, kind, language));
                    }
                    if widget::button(ui, "Import PNG…") {
                        asked = Some((IconAction::Import, kind, language));
                    }
                    ui.add_enabled_ui(icons.has_override(kind, language), |ui| {
                        if widget::button(ui, "Reset icon") {
                            asked = Some((IconAction::Reset, kind, language));
                        }
                    });
                });
                if let Some((action, id, language)) = asked {
                    self.icon_action(action, id, language);
                }
                if let Some(id) = removed {
                    self.list.remove(id);
                    self.chart.clear_type(id);
                    self.icons.reset_all(id);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the page draws in the three tabs, a type is added and a cell is changed, and everything
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
        for tab in [0, 1, 2, 0] {
            page.tab = tab;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        assert_eq!(page.names[0], "Normal");
        assert_eq!(page.chart.changed_count(), 0);

        // A new type and a changed cell are saved.
        let id = page.list.add("Cosmic", 15).unwrap();
        page.chart.set(id, 0, chart::DOUBLE);
        page.chart.set(9, 11, chart::HALF);
        page.tab = 0;
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
