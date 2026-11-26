//! The Nature page: what each nature does to the statistics, and new natures.
//!
//! The game raises one statistic of a nature by 10 % and lowers another one by 10 % (`value * 110 / 100`). On this page the user writes
//! the change in percent for each of the five statistics: 10 raises by 10 %, -10 lowers by 10 %, -200 lowers by 200 % (the statistic is
//! then 1), 0 changes nothing. The user can also add natures (the numbers 25 to 31).
//!
//! The table goes on the SD card as `nature/natures.csv` (see `sango_core::natures`). The plugin reads it in the game.
//! A new nature works for the statistics only: the game does not know it for the flavors (Pokéblocks, berries that confuse) and for the
//! names, so the name of a new nature shows only in Sango Studio.

use sango_core::natures::{Nature as Row, NatureColor, NatureTable, GAME_COUNT, GAME_NATURE_NAMES, MAX_CHANGE, MAX_COUNT, STATS};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

const FILE: &str = "nature/natures.csv";

/// The longest name of a new nature.
const NAME_SIZE: usize = 16;

/// The Nature page.
pub struct Nature {
    /// The window of the button Reset.
    reset_open: bool,
    opened_for: Option<PathBuf>,
    table: NatureTable,
    /// Undo and redo of the table.
    history: sango_ui::history::History<NatureTable>,
    saved: NatureTable,
    from_card: bool,
    status: Option<(bool, String)>,
    /// The language of the names that the table shows (the place in `NAME_LANGUAGES`).
    language: usize,
}

impl Default for Nature {
    fn default() -> Self {
        Nature { opened_for: None, reset_open: false, history: Default::default(), table: NatureTable::game(), saved: NatureTable::game(), from_card: false, status: None, language: 0 }
    }
}

impl Nature {
    fn open(&mut self, sd: &SdCard) {
        self.opened_for = Some(sd.root().to_path_buf());
        self.status = None;
        self.from_card = false;
        let table = match sd.read(FILE) {
            Some(bytes) => {
                self.from_card = true;
                match NatureTable::parse(&String::from_utf8_lossy(&bytes)) {
                    Ok(table) => table,
                    Err(message) => {
                        self.status = Some((false, message));
                        NatureTable::game()
                    }
                }
            }
            None => NatureTable::game(),
        };
        self.saved = table.clone();
        self.table = table;
    }

    fn save(&mut self, sd: &SdCard) {
        let result = if self.table.is_game() {
            sd.remove(FILE).map(|_| ())
        } else {
            sd.write(FILE, self.table.to_csv().as_bytes()).map(|_| ())
        };
        self.status = Some(match result.and_then(|_| sd.mark_changed(FILE)) {
            Ok(()) => {
                self.saved = self.table.clone();
                self.from_card = !self.table.is_game();
                (true, "Saved. The game reads the natures again by itself.".to_string())
            }
            Err(message) => (false, message),
        });
    }

    fn export_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("natures.csv").save_file() else { return };
        self.status = Some(match std::fs::write(&path, self.table.to_csv()) {
            Ok(()) => (true, format!("Exported natures.csv to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn import_csv(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() else { return };
        let result = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))
            .and_then(|text| NatureTable::parse(&text));
        self.status = Some(match result {
            Ok(table) => {
                self.table = table;
                (true, format!("Imported natures from {}.", path.display()))
            }
            Err(message) => (false, message),
        });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let full = self.table.natures.len() >= MAX_COUNT;
                let changed = self.table != self.saved;
                let clicks = widget::action_bar(ui, widget::ActionBar { new: Some(("+ New nature", !full)), export: Some(true), import: Some(true), reset: Some(!self.table.is_game()), save: Some(changed && sd.is_some()), ..Default::default() });
                {
                    if clicks.new {
                        let number = self.table.natures.len() + 1;
                        self.table.natures.push(Row {
                            name: format!("Nature {number}"),
                            change: [0; 5],
                            other_names: Default::default(),
                            color: NatureColor::Blue,
                        });
                    }
                }
                if clicks.export {
                    self.export_csv();
                }
                if clicks.import {
                    self.import_csv();
                }
                self.reset_open |= clicks.reset;
                if clicks.save {
                    if let Some(sd) = sd {
                        self.save(sd);
                    }
                }
                if full {
                    ui.label(widget::hint("The table has the most natures that the game can use."));
                }
                if changed {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_card {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the game", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
    }

    fn table_card(&mut self, ui: &mut egui::Ui) {
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Language:");
                // The places of NAME_LANGUAGES in the languages of the game: English, then the order of the game.
                const GAME_PLACE: [usize; 8] = [2, 0, 1, 3, 4, 5, 6, 7];
                let mut chosen = GAME_PLACE.get(self.language).copied().unwrap_or(2);
                sango_ui::language::menu(ui, "nature_language_combo", &mut chosen, Some(180.0));
                self.language = GAME_PLACE.iter().position(|&p| p == chosen).unwrap_or(0);
            });
            ui.add_space(6.0);
            let language = self.language;
            let mut remove = None;
            let mut reset = None;
            let mut restore_name = None;
            let count = self.table.natures.len();
            egui::Grid::new("nature_table_v3").num_columns(10).spacing([12.0, 6.0]).striped(true).show(ui, |ui| {
                ui.add_sized([140.0, 0.0], egui::Label::new(widget::hint("Nature")));
                ui.label(widget::hint("Color"));
                for name in STATS {
                    ui.label(widget::hint(&format!("{name} (%)")));
                }
                ui.label(widget::hint("What it does"));
                ui.label("");
                ui.label("");
                ui.end_row();
                for number in 0..count {
                    let is_new = number >= GAME_COUNT;
                    let game = if is_new { Row { name: String::new(), change: [0; 5], other_names: Default::default(), color: NatureColor::Blue } } else { Row::game(number) };
                    let text = if language == 0 {
                        &mut self.table.natures[number].name
                    } else {
                        &mut self.table.natures[number].other_names[language - 1]
                    };
                    let response = ui.add_sized([140.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(text).char_limit(NAME_SIZE));
                    if response.lost_focus() && text.is_empty() {
                        restore_name = Some(number);
                    }

                    // Color dropdown
                    let color = &mut self.table.natures[number].color;
                    let color_text = match color {
                        NatureColor::Blue => egui::RichText::new("Blue").color(egui::Color32::from_rgb(0, 140, 255)),
                        NatureColor::Red => egui::RichText::new("Red").color(egui::Color32::from_rgb(255, 70, 70)),
                        NatureColor::Green => egui::RichText::new("Green").color(egui::Color32::from_rgb(0, 192, 0)),
                        NatureColor::Default => egui::RichText::new("None").color(egui::Color32::from_rgb(180, 180, 180)),
                    };
                    let combo = egui::ComboBox::from_id_salt(("nature_color", number))
                        .selected_text(color_text)
                        .width(75.0)
                        .show_ui(ui, |ui| {
                            for c in NatureColor::ALL {
                                let label = match c {
                                    NatureColor::Blue => egui::RichText::new("Blue").color(egui::Color32::from_rgb(0, 140, 255)),
                                    NatureColor::Red => egui::RichText::new("Red").color(egui::Color32::from_rgb(255, 70, 70)),
                                    NatureColor::Green => egui::RichText::new("Green").color(egui::Color32::from_rgb(0, 192, 0)),
                                    NatureColor::Default => egui::RichText::new("None").color(egui::Color32::from_rgb(180, 180, 180)),
                                };
                                ui.selectable_value(color, c, label);
                            }
                        });
                    sango_ui::focus::combo_arrows(ui, &combo.response, color, &NatureColor::ALL);

                    for stat in 0..5 {
                        widget::spin(ui, &mut self.table.natures[number].change[stat], -f64::from(MAX_CHANGE), f64::from(MAX_CHANGE));
                    }
                    ui.label(widget::hint(&self.table.natures[number].summary()));
                    let changed = self.table.natures[number].change != game.change
                        || (!is_new && (self.table.natures[number].name != game.name
                            || self.table.natures[number].other_names != game.other_names
                            || self.table.natures[number].color != game.color));
                    if changed {
                        if widget::reset_mark(ui, true) {
                            reset = Some(number);
                        }
                    } else {
                        ui.label("");
                    }
                    if is_new && number + 1 == count && widget::confirm_button(ui, "Remove", "Click again") {
                        remove = Some(number);
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
            if let Some(number) = restore_name {
                if language == 0 {
                    self.table.natures[number].name = Row::game(number).name;
                } else if number < GAME_COUNT {
                    self.table.natures[number].other_names[language - 1] = GAME_NATURE_NAMES[language][number].to_string();
                }
            }
            if let Some(number) = reset {
                let game = if number >= GAME_COUNT { None } else { Some(Row::game(number)) };
                self.table.natures[number].change = game.as_ref().map_or([0; 5], |g| g.change);
                self.table.natures[number].color = game.as_ref().map_or(NatureColor::Blue, |g| g.color);
                if let Some(game) = game {
                    self.table.natures[number].name = game.name;
                    self.table.natures[number].other_names = game.other_names;
                } else {
                    self.table.natures[number].name = format!("Nature {}", number + 1);
                    self.table.natures[number].other_names = Default::default();
                }
            }
            if let Some(number) = remove {
                self.table.natures.remove(number);
            }
        });
    }
}

impl Module for Nature {
    fn title(&self) -> &str {
        "Nature"
    }

    fn icon(&self) -> &str {
        "🍃"
    }

    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn discard_changes(&mut self) -> bool {
        self.table = self.saved.clone();
        true
    }

    fn unsaved(&self) -> bool {
        self.table != self.saved
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Nature");
        ui.add_space(8.0);
        let Some(sd) = session.sd.as_ref() else {
            widget::missing_card(ui, "Choose the SD card on the Home page to change the natures.");
            return;
        };
        if self.opened_for.as_deref() != Some(sd.root()) {
            self.open(sd);
        }
        let sd = session.sd.clone();
        self.tool_bar(ui, sd.as_ref());
        if widget::reset_window(ui.ctx(), &mut self.reset_open, "All the natures", &mut []) {
            self.table = NatureTable::game();
            self.status = Some((true, "The natures of the game are back. Save to keep them.".to_string()));
        }
        ui.add_space(6.0);
        egui::ScrollArea::both().id_salt("nature_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.table_card(ui);
        });
        // Ctrl+Z and Ctrl+Y on the table.
        self.history.track(ui, 0, &mut self.table);
    }
}
