//! The move editor.
//!
//! The page edits the data of one move (a resource of the file 0 of the archive 189) and its texts in the eight
//! languages. The files that the user changes go on the SD card, in the folders `move/data` and `text`: the romfs is
//! never changed. Every field has a note: what it means, how the game reads it (from the code of the game), and a page of
//! Bulbapedia when one fits. A field that no code reads shows only the word "unused".

mod csv;
mod custom;
pub mod data;
mod picker;
mod texts;
mod tools;

use data::{Def, Group, Kind, MoveData, COUNT, DEFS, UNUSED};
use sango_core::{Bundle, Dump, Garc};
use sango_species::battle_icons;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The tabs of the page.
const TABS: [&str; 4] = ["General", "Reactions", "Text", "CSV"];

/// The place of the tab "CSV" in [`TABS`]: the import and the export of all the entries.
const CSV_TAB: usize = 3;

/// The place of the tab "Reactions" in [`TABS`]: only a new move has reactions.
const REACTIONS_TAB: usize = 1;

/// The place of the Text tab.
const TEXT_TAB: usize = 2;

/// The width of one column of groups, in points.
const COLUMN_WIDTH: f32 = 470.0;

/// The moves of the romfs, to show examples in the notes.
struct Index {
    /// The data of each move (the move 0 is empty).
    moves: Vec<Option<MoveData>>,
    /// The English name of each move.
    names: Vec<String>,
}

impl Index {
    /// The names of some moves that have the same value in a field, not counting the move that is open.
    fn examples(&self, def: &Def, value: i32, current: usize) -> (Vec<&str>, usize) {
        let same: Vec<usize> = (1..=COUNT).filter(|&n| n != current && self.moves.get(n).and_then(|m| m.as_ref()).is_some_and(|m| def.get(&m.bytes) == value)).collect();
        let shown = same.iter().take(4).map(|&n| self.names.get(n).map_or("", |s| s.as_str())).collect();
        (shown, same.len().saturating_sub(4))
    }
}

pub struct Move {
    /// The dump that the archive comes from.
    root: Option<PathBuf>,
    /// The resources of the archive: one for each move.
    bundle: Option<Bundle>,
    id: usize,
    /// The data of the move.
    data: Option<MoveData>,
    /// The data as it was loaded or saved: to know what changed.
    baseline: Option<MoveData>,
    /// True when the data came from the SD card.
    from_sd: bool,
    /// The move that `data` is for.
    loaded: Option<usize>,
    tab: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The names of the moves in the list, as the user has them.
    list_names: Option<Vec<String>>,
    /// The rows of the romfs for the list: the data without the names.
    rows: Vec<picker::Row>,
    index: Option<Index>,
    texts: texts::MoveTexts,
    tools: tools::Tools,
    store: custom::Store,
    /// The language of the texts of a new move.
    custom_language: usize,
}

impl Default for Move {
    fn default() -> Self {
        Move {
            root: None,
            bundle: None,
            id: 1,
            data: None,
            baseline: None,
            from_sd: false,
            loaded: None,
            tab: 0,
            status: None,
            list_names: None,
            rows: Vec::new(),
            index: None,
            texts: texts::MoveTexts::default(),
            tools: tools::Tools::default(),
            store: custom::Store::default(),
            custom_language: 2,
        }
    }
}

impl Move {
    /// Opens the archives of a dump.
    fn open(&mut self, dump: &Dump) {
        self.root = Some(dump.root().to_path_buf());
        self.bundle = Garc::open(&dump.archive(data::ARCHIVE)).ok().and_then(|garc| garc.first_file(0).map(|b| b.to_vec())).and_then(|bytes| Bundle::parse(bytes).ok());
        self.texts.open(dump);
        battle_icons::load(dump);
        self.list_names = None;
        self.loaded = None;
        let moves: Vec<Option<MoveData>> = (0..=COUNT).map(|n| self.bundle.as_ref().and_then(|b| b.resource(n)).and_then(|b| MoveData::from_bytes(b).ok())).collect();
        let rom_names = self.texts.romfs(texts::ENGLISH, texts::FIELDS[0].file);
        let names: Vec<String> = (0..=COUNT).map(|n| rom_names.as_ref().and_then(|f| f.text(n)).unwrap_or_default()).collect();
        self.rows = (0..=COUNT).map(|n| Self::row(moves[n].as_ref(), names[n].clone())).collect();
        self.index = Some(Index { moves, names });
        if self.bundle.is_none() {
            self.status = Some((false, "The archive of the moves is not open.".into()));
        }
    }

    /// The row of the list for the data of a move.
    fn row(data: Option<&MoveData>, name: String) -> picker::Row {
        let get = |key: &str| data.and_then(|d| d.get(key)).unwrap_or(0);
        picker::Row { name, kind: get("type") as u8, damage_kind: get("damage_kind") as u8, power: get("power") as u8, accuracy: get("accuracy") as u8, pp: get("pp") as u8 }
    }

    /// Reads the data of the chosen move: the SD card first, then the romfs.
    fn load(&mut self, session: &Session) {
        self.loaded = Some(self.id);
        let found = self.current(session.sd.as_ref(), self.id);
        self.from_sd = self.is_custom() || session.sd.as_ref().is_some_and(|sd| sd.read(&data::sd_path(self.id)).is_some());
        match found {
            Some(data) => {
                self.baseline = Some(data.clone());
                self.data = Some(data);
                self.status = None;
            }
            None => {
                self.data = None;
                self.baseline = None;
                self.status = Some((false, format!("The data of the move {} is missing.", self.id)));
            }
        }
    }

    /// The data of a move as the user has it: the SD card first, then the romfs.
    fn current(&self, sd: Option<&sango_core::SdCard>, id: usize) -> Option<MoveData> {
        if id >= sango_core::custom_item::FIRST_ID {
            return self.store.find(id).map(|c| c.data.clone());
        }
        let on_card = sd.and_then(|sd| sd.read(&data::sd_path(id))).and_then(|b| MoveData::from_bytes(&b).ok());
        on_card.or_else(|| self.bundle.as_ref().and_then(|b| b.resource(id)).and_then(|b| MoveData::from_bytes(b).ok()))
    }

    /// The rows of the move list as the user has them: the names and the data of the SD card and the romfs.
    fn menu_rows(&mut self, session: &Session) -> Vec<picker::Row> {
        let sd = session.sd.as_ref();
        if self.list_names.is_none() {
            let names = (0..=COUNT).map(|n| self.texts.name(sd, n)).collect();
            self.list_names = Some(names);
        }
        let names = self.list_names.clone().unwrap_or_default();
        let mut rows: Vec<picker::Row> = (0..=COUNT).map(|n| Self::row(self.current(sd, n).as_ref(), names.get(n).cloned().unwrap_or_default())).collect();
        // The new moves come after the moves of the game, at their numbers: a number between is a row without a name that the list skips.
        if let Some(last) = self.store.ids().into_iter().max() {
            rows.resize(last + 1, picker::Row::default());
            for id in self.store.ids() {
                if let Some(entry) = self.store.find(id) {
                    rows[id] = Self::row(Some(&entry.data), entry.names[custom::ENGLISH].clone());
                }
            }
        }
        rows
    }

    /// True when the open move is a new move of the user (not a move of the game).
    fn is_custom(&self) -> bool {
        self.id >= sango_core::custom_item::FIRST_ID
    }

    /// The highest number that the list shows: the last move of the game, or the last new move.
    fn last_move(&self) -> usize {
        self.store.ids().into_iter().max().unwrap_or(COUNT).max(COUNT)
    }

    /// Makes a new move that starts with the data of the open move. The move is then edited like the other moves.
    fn add_move(&mut self) {
        let template = self.data.clone();
        match self.store.add(template.as_ref()) {
            Ok(id) => {
                self.id = id;
                self.loaded = None;
                self.status = Some((true, format!("The new move {id} starts like the move that was open. Change its name in the Text tab.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Deletes the open move when it is a new move.
    fn delete_move(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        if !self.is_custom() {
            return;
        }
        let id = self.id;
        match self.store.delete(sd, id) {
            Ok(()) => {
                self.id = 1;
                self.loaded = None;
                self.list_names = None;
                self.status = Some((true, format!("The new move {id} was deleted.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Writes the data and the texts on the SD card, and tells the plugin.
    fn save(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let mut result: Result<(), String> = Ok(());
        if self.is_custom() {
            // A new move keeps its data in its own file, with its texts and its reactions.
            if let (Some(data), Some(entry)) = (&self.data, self.store.find_mut(self.id)) {
                entry.data = data.clone();
            }
            result = self.store.save(sd).map(|_| ());
        } else if let Some(data) = &self.data {
            if self.baseline.as_ref() != Some(data) {
                let path = data::sd_path(self.id);
                result = sd.write(&path, &data.to_bytes()).and_then(|_| sd.mark_changed(&path));
            }
        }
        if result.is_ok() {
            result = self.texts.save(sd).map(|_| ());
        }
        match result {
            Ok(()) => {
                self.baseline = self.data.clone();
                self.from_sd = true;
                self.list_names = None;
                self.status = Some((true, format!("Saved move {}. The files are on the SD card.", self.id)));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// True when something changed and was not saved.
    fn dirty(&self) -> bool {
        self.data != self.baseline || self.texts.changed() || self.store.changed()
    }

    /// The picker, the buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let before = self.id;
        let mut save = false;
        let mut reset_tab = false;
        let mut reset_more = false;
        let mut undo = false;
        let mut add = false;
        let mut delete = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let rows = self.menu_rows(session);
                let mut chosen = self.id;
                picker::move_menu(ui, "move_menu", &mut chosen, 280.0, &rows);
                self.id = chosen.clamp(1, self.last_move());
                if self.is_custom() {
                    ui.label(widget::hint("new move"));
                }
                let has_card = session.sd.is_some() && self.bundle.is_some();
                ui.add_enabled_ui(has_card, |ui| {
                    add = widget::button_styled(ui, egui::RichText::new("+ New move").color(color::WHITE), Some(color::ACCENT));
                })
                .response
                .on_hover_text("Adds a move that starts like this one (the same data). Then you change it like the other moves.");
                ui.add_enabled_ui(has_card && self.is_custom(), |ui| {
                    delete = widget::button(ui, "Delete");
                })
                .response
                .on_hover_text("Deletes this new move.");
                ui.add_enabled_ui(has_card && !self.is_custom(), |ui| {
                    reset_tab = widget::button(ui, "Reset tab");
                })
                .response
                .on_hover_text("Puts the data of this tab for this move back to the romfs. Undo gives it back.");
                ui.add_enabled_ui(has_card && !self.is_custom(), |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Reset one move or all the moves, for the parts that you choose.");
                let undo_label = self.tools.history.last().map(|backup| format!("Undo: {}", backup.label));
                ui.add_enabled_ui(undo_label.is_some() && session.sd.is_some(), |ui| {
                    undo = widget::button(ui, "Undo");
                })
                .response
                .on_hover_text(undo_label.unwrap_or_else(|| "Nothing to undo.".into()));
                save = self.data.is_some() && session.sd.is_some() && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
                if self.data.is_some() && self.dirty() {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd || self.texts.from_card() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else if self.data.is_some() {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
        if add {
            self.add_move();
        }
        if delete {
            self.delete_move(session);
        }
        if self.id != before {
            self.loaded = None;
        }
        if reset_tab {
            let part = if self.tab == TEXT_TAB { tools::Part::Texts } else { tools::Part::Data };
            self.run_reset(session, tools::Scope::Move, &[part]);
        }
        if reset_more {
            self.tools.reset_open = true;
            self.tools.parts = [self.tab == 0, self.tab == TEXT_TAB];
        }
        if undo {
            self.undo(session);
        }
        if save {
            self.save(session);
        }
    }

    /// The definition of a field.
    fn def(key: &str) -> &'static Def {
        DEFS.iter().find(|d| d.key == key).expect("a field of the table")
    }

    /// The name of a field with its note: what it means, how the game reads it, examples, and Bulbapedia.
    fn label(&self, ui: &mut egui::Ui, data: &MoveData, def: &Def) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(widget::hint(def.label));
            let info = if def.url.is_empty() { ui.label(egui::RichText::new("ℹ").color(color::ACCENT)) } else { ui.hyperlink_to("ℹ", def.url) };
            info.on_hover_ui(|ui| self.note(ui, data, def));
        });
    }

    /// The text of the note of a field.
    fn note(&self, ui: &mut egui::Ui, data: &MoveData, def: &Def) {
        ui.set_max_width(320.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(def.label).strong());
        ui.label(def.summary);
        ui.label(widget::hint(&format!("In the game: {}", def.code)));
        if let Some(index) = &self.index {
            let (shown, more) = index.examples(def, def.get(&data.bytes), self.id);
            if !shown.is_empty() {
                let more = if more > 0 { format!(" and {more} more") } else { String::new() };
                ui.label(widget::hint(&format!("Same value: {}{more}.", shown.join(", "))));
            }
        }
        if !def.url.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(widget::hint("Learn more on"));
                ui.hyperlink_to("Bulbapedia ↗", def.url);
            });
        }
    }

    /// A line of a grid: the name of the field, then what `add` draws for its value. The value is written back when it
    /// changed.
    fn line(&self, ui: &mut egui::Ui, data: &mut MoveData, key: &str, add: impl FnOnce(&mut egui::Ui, &mut i32, &Def)) {
        let def = Self::def(key);
        self.label(ui, data, def);
        ui.horizontal(|ui| {
            let mut value = def.get(&data.bytes);
            let before = value;
            add(ui, &mut value, def);
            if value != before {
                def.set(&mut data.bytes, value);
            }
        });
        ui.end_row();
    }

    /// A line with a bar and the number: for a value that has a usual range. The number can go beyond the bar.
    fn bar_line(&self, ui: &mut egui::Ui, data: &mut MoveData, key: &str, bar_min: i32, bar_max: i32, meaning: &dyn Fn(i32) -> &'static str) {
        self.line(ui, data, key, |ui, value, def| {
            widget::value_bar(ui, value, bar_min, bar_max);
            widget::spin(ui, value, def.min() as f64, def.max() as f64);
            let text = meaning(*value);
            if !text.is_empty() {
                ui.label(widget::hint(text));
            }
        });
    }

    /// A number that is a code of the game, with some moves that have it.
    fn code_line(&self, ui: &mut egui::Ui, data: &mut MoveData, key: &str) {
        self.line(ui, data, key, |ui, value, def| {
            widget::spin(ui, value, def.min() as f64, def.max() as f64);
            if let Some(index) = &self.index {
                let (shown, more) = index.examples(def, *value, self.id);
                if !shown.is_empty() {
                    let more = if more > 0 { format!(" +{more}") } else { String::new() };
                    ui.label(widget::hint(&format!("{}{more}", shown.iter().take(2).copied().collect::<Vec<_>>().join(", "))));
                }
            }
        });
    }

    /// A choice in a list of values that have a name each.
    fn named_line(&self, ui: &mut egui::Ui, data: &mut MoveData, key: &str) {
        self.line(ui, data, key, |ui, value, def| {
            let Kind::Named(list) = def.kind else { return };
            let text = list.iter().find(|(v, _)| v == value).map_or(format!("Value {value}"), |(_, name)| name.to_string());
            let place = list.iter().position(|(v, _)| v == value);
            let combo = egui::ComboBox::from_id_salt(def.key).selected_text(text).width(250.0).height(360.0).show_ui(ui, |ui| {
                for (number, name) in list.iter() {
                    let row = ui.selectable_label(*value == *number, *name);
                    widget::scroll_to_chosen(ui, &row, *value == *number);
                    if row.clicked() {
                        *value = *number;
                    }
                }
            });
            // The arrows move in the list, not in the numbers.
            let mut at = place.unwrap_or(0) as i32;
            let before = at;
            focus_combo(ui, &combo.response, &mut at, list.len() as i32 - 1);
            if at != before {
                *value = list[at as usize].0;
            }
        });
    }

    /// A choice in a list of names.
    fn choice_line(&self, ui: &mut egui::Ui, data: &mut MoveData, key: &str) {
        self.line(ui, data, key, |ui, value, def| {
            let Kind::Choice(list) = def.kind else { return };
            let text = list.get(*value as usize).map_or(format!("Value {value}"), |s| s.to_string());
            let combo = egui::ComboBox::from_id_salt(def.key).selected_text(text).width(220.0).show_ui(ui, |ui| {
                for (place, name) in list.iter().enumerate() {
                    let row = ui.selectable_label(*value == place as i32, *name);
                    widget::scroll_to_chosen(ui, &row, *value == place as i32);
                    if row.clicked() {
                        *value = place as i32;
                    }
                }
            });
            focus_combo(ui, &combo.response, value, list.len() as i32 - 1);
        });
    }

    /// A row of pills for flags.
    fn chips(&self, ui: &mut egui::Ui, data: &mut MoveData, keys: &[&str]) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for key in keys {
                let def = Self::def(key);
                let mut on = def.get(&data.bytes) != 0;
                let response = widget::chip(ui, def.label, &mut on);
                response.on_hover_ui(|ui| self.note(ui, data, def));
                def.set(&mut data.bytes, on as i32);
            }
        });
    }

    /// A card of a group: the title, what `add` draws, and the word "unused" for the fields of the group that no code reads.
    fn card(&self, ui: &mut egui::Ui, group: Group, add: impl FnOnce(&mut egui::Ui)) {
        widget::card(ui, |ui| {
            ui.set_min_width(COLUMN_WIDTH - 36.0);
            ui.label(widget::title(group.title()));
            ui.add_space(4.0);
            add(ui);
            for unused in UNUSED.iter().filter(|u| u.show == Some(group)) {
                ui.horizontal(|ui| {
                    ui.label(widget::hint(unused.label));
                    ui.label(widget::hint("unused"));
                });
            }
        });
        ui.add_space(10.0);
    }

    /// The group "Basics".
    fn basics_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Basics, |ui| {
            egui::Grid::new("move_basics").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.line(ui, data, "type", |ui, value, _| {
                    let mut kind = (*value).clamp(0, 31) as u8;
                    sango_species::type_menu(ui, "move_type", &mut kind, None);
                    *value = kind as i32;
                });
                self.choice_line(ui, data, "category");
                self.line(ui, data, "damage_kind", |ui, value, _| {
                    ui.horizontal(|ui| {
                        let kinds = [("Status", sango_core::moves::Category::Status), ("Physical", sango_core::moves::Category::Physical), ("Special", sango_core::moves::Category::Special)];
                        for (place, (name, category)) in kinds.iter().enumerate() {
                            let mut on = *value == place as i32;
                            let before = on;
                            if let Some(texture) = battle_icons::category_texture(ui.ctx(), *category) {
                                let (rect, response) = ui.allocate_exact_size(battle_icons::ICON_SIZE, egui::Sense::click());
                                let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                ui.painter().image(texture.id(), rect, full, if on { egui::Color32::WHITE } else { egui::Color32::from_white_alpha(110) });
                                if response.clicked() {
                                    on = true;
                                }
                                if on {
                                    widget::glow(ui, rect, 8.0);
                                }
                            } else {
                                widget::chip(ui, name, &mut on);
                            }
                            if on && !before {
                                *value = place as i32;
                            }
                        }
                    });
                });
                self.bar_line(ui, data, "power", 0, 250, &|v| match v {
                    0 => "no damage",
                    1 => "varies",
                    _ => "",
                });
                self.bar_line(ui, data, "accuracy", 0, 101, &|v| if v >= 101 { "never misses" } else { "%" });
                self.bar_line(ui, data, "pp", 0, 40, &|_| "");
                self.bar_line(ui, data, "priority", -7, 5, &|_| "");
            });
        });
    }

    /// The group "Damage and recovery".
    fn damage_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Damage, |ui| {
            egui::Grid::new("move_damage").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.bar_line(ui, data, "max_hits", 0, 6, &|v| if v == 0 { "one hit" } else { "" });
                self.bar_line(ui, data, "crit_stage", 0, 6, &|v| if v >= 6 { "always" } else { "" });
                self.bar_line(ui, data, "flinch", 0, 100, &|_| "%");
                self.bar_line(ui, data, "drain", -100, 100, &|v| match v {
                    1..=127 => "% of the damage given back",
                    -128..=-1 => "% of the damage as recoil",
                    _ => "",
                });
                self.bar_line(ui, data, "hp_change", -100, 100, &|v| match v {
                    1..=127 => "% of the HP recovered",
                    -128..=-1 => "% of the HP lost",
                    _ => "",
                });
            });
        });
    }

    /// The group "Status condition".
    fn status_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Status, |ui| {
            egui::Grid::new("move_status").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.named_line(ui, data, "status");
                self.bar_line(ui, data, "status_chance", 0, 100, &|_| "%");
                self.choice_line(ui, data, "status_duration");
                self.bar_line(ui, data, "status_turns_min", 0, 15, &|_| "");
                self.bar_line(ui, data, "status_turns_max", 0, 15, &|_| "");
            });
        });
    }

    /// The group "Stat changes": three slots.
    fn stats_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Stats, |ui| {
            egui::Grid::new("move_stats").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (stat, stages, chance) in [("stat_1", "stat_1_stages", "stat_1_chance"), ("stat_2", "stat_2_stages", "stat_2_chance"), ("stat_3", "stat_3_stages", "stat_3_chance")] {
                    self.choice_line(ui, data, stat);
                    self.bar_line(ui, data, stages, -6, 6, &|_| "");
                    self.bar_line(ui, data, chance, 0, 100, &|_| "%");
                }
            });
        });
    }

    /// The group "Target and AI".
    fn target_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Target, |ui| {
            egui::Grid::new("move_target").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.choice_line(ui, data, "target");
                self.code_line(ui, data, "ai_script");
            });
        });
    }

    /// The group "Properties": the 16 flags of the move.
    fn properties_group(&self, ui: &mut egui::Ui, data: &mut MoveData) {
        self.card(ui, Group::Properties, |ui| {
            let keys: Vec<&str> = DEFS.iter().filter(|d| d.group == Group::Properties).map(|d| d.key).collect();
            self.chips(ui, data, &keys);
        });
    }

    /// One group, drawn in its card.
    fn group(&self, ui: &mut egui::Ui, data: &mut MoveData, group: Group) {
        match group {
            Group::Basics => self.basics_group(ui, data),
            Group::Damage => self.damage_group(ui, data),
            Group::Status => self.status_group(ui, data),
            Group::Stats => self.stats_group(ui, data),
            Group::Target => self.target_group(ui, data),
            Group::Properties => self.properties_group(ui, data),
        }
    }

    /// The General tab.
    fn general_tab(&mut self, ui: &mut egui::Ui) {
        let Some(mut data) = self.data.take() else { return };
        egui::ScrollArea::vertical().id_salt("move_general").auto_shrink([false, false]).show(ui, |ui| {
            let columns = (((ui.available_width() + 10.0) / (COLUMN_WIDTH + 10.0)).floor() as usize).clamp(1, 2);
            let layout: Vec<Vec<Group>> = if columns == 2 { vec![vec![Group::Basics, Group::Damage, Group::Target], vec![Group::Status, Group::Stats, Group::Properties]] } else { vec![Group::ALL.to_vec()] };
            ui.horizontal_top(|ui| {
                for column in layout {
                    ui.vertical(|ui| {
                        ui.set_width(COLUMN_WIDTH);
                        for group in column {
                            self.group(ui, &mut data, group);
                        }
                    });
                    ui.add_space(10.0);
                }
            });
            ui.add_space(10.0);
        });
        // A reset or an import in the tab reads the files again: the data of this frame is then forgotten.
        if self.loaded.is_some() {
            self.data = Some(data);
        }
    }
}

/// Lets the page focus a menu: a click or the arrows of the page make it active, then Up and Down change the choice.
/// `last` is the largest value.
fn focus_combo(ui: &egui::Ui, response: &egui::Response, value: &mut i32, last: i32) {
    let id = response.id;
    widget::register_focusable(ui, id, response.rect);
    if response.clicked() {
        widget::set_active_id(ui, id);
    }
    if widget::is_active(ui, id) && widget::page_has_focus(ui) {
        widget::glow(ui, response.rect, 6.0);
        widget::scroll_active_into_view(ui, response);
        ui.input_mut(|i| {
            let mut step = 0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                step -= 1;
            }
            if step != 0 {
                *value = (*value + step).clamp(0, last);
            }
        });
    }
}

impl Module for Move {
    fn title(&self) -> &str {
        "Move"
    }

    fn icon(&self) -> &str {
        "M"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Move");
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
        self.store.ensure(session.sd.as_ref());
        if self.bundle.is_some() && self.loaded != Some(self.id) {
            self.load(session);
        }
        let session: &Session = session;
        sango_species::battle_icons::refresh_custom_types(session.sd.as_ref());
        self.top_bar(ui, session);
        self.reset_window(ui.ctx(), session);
        ui.add_space(8.0);
        let mut clicked = None;
        egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(12.0)).inner_margin(egui::Margin::same(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, name) in TABS.iter().enumerate() {
                    let tab_id = egui::Id::new(format!("move_tab_{i}"));
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
        if self.data.is_none() && self.loaded.is_some() {
            return;
        }
        match self.tab {
            0 => self.general_tab(ui),
            CSV_TAB => {
                egui::ScrollArea::vertical().id_salt("move_csv_tab").auto_shrink([false, false]).show(ui, |ui| self.csv_card(ui, session));
            }
            REACTIONS_TAB => {
                let id = self.id;
                match self.store.find_mut(id) {
                    Some(entry) => egui::ScrollArea::vertical().id_salt("move_reactions").auto_shrink([false, false]).show(ui, |ui| custom::reactions_ui(ui, entry)).inner,
                    None => widget::card(ui, |ui| {
                        ui.label(widget::hint("The moves of the game have their reactions in the code of the game. A new move (the button \"+ New move\") has its own reactions here."));
                    }),
                }
            }
            _ => {
                if let Some(entry) = self.store.find_mut(self.id) {
                    let mut language = self.custom_language;
                    egui::ScrollArea::vertical().id_salt("move_custom_texts").auto_shrink([false, false]).show(ui, |ui| custom::texts_ui(ui, entry, &mut language));
                    self.custom_language = language;
                } else {
                    let sd = session.sd.as_ref();
                    self.texts.ui(ui, sd, self.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the page draws for some moves, in both tabs, and the notes find their examples.
    #[test]
    fn the_page_draws_for_some_moves() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_move_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Move::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Move, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for (id, tab) in [(1, 0), (24, 0), (50, 1), (621, 0), (1, 1)] {
            page.id = id;
            page.tab = tab;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        assert!(page.data.is_some());
        let index = page.index.as_ref().unwrap();
        let power = DEFS.iter().find(|d| d.key == "power").unwrap();
        let (shown, more) = index.examples(power, 40, 1);
        assert!(!shown.is_empty() && shown.len() + more > 1, "other moves have the power 40");
        let _ = std::fs::remove_dir_all(&folder);
    }
}

#[cfg(test)]
mod add_test {
    use super::*;

    /// With the dump of the game: a new move is added from the open move, edited in the same tabs, saved, and found by another
    /// page; it is deleted.
    #[test]
    fn a_new_move_is_made_and_edited_like_the_others() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_move_add_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Move::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Move, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        page.id = 85;
        frame(&mut page, &mut session);
        frame(&mut page, &mut session);
        let power = page.data.as_ref().unwrap().get("power").unwrap();
        page.add_move();
        assert_eq!(page.id, 900);
        for tab in [0, 1, 2] {
            page.tab = tab;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        assert_eq!(page.data.as_ref().unwrap().get("power"), Some(power), "the new move starts with the data of the open move");
        page.data.as_mut().unwrap().set("power", 77);
        page.save(&session);
        assert!(matches!(page.status, Some((true, _))), "{:?}", page.status);
        let mut other = Move::default();
        other.id = 900;
        frame(&mut other, &mut session);
        frame(&mut other, &mut session);
        assert_eq!(other.data.as_ref().unwrap().get("power"), Some(77));
        let rows = other.menu_rows(&session);
        assert_eq!(rows[900].name, "New move");
        assert!(rows[800].name.is_empty());
        other.delete_move(&session);
        assert!(other.store.find(900).is_none() && session.sd.as_ref().unwrap().read("move/custom/900.bin").is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
