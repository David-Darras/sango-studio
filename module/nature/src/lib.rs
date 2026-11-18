//! The Nature page: what each nature does to the statistics, and new natures.
//!
//! The game raises one statistic of a nature by 10 % and lowers another one by 10 % (`value * 110 / 100`). On this page the user writes
//! the change in percent for each of the five statistics: 10 raises by 10 %, -10 lowers by 10 %, -200 lowers by 200 % (the statistic is
//! then 1), 0 changes nothing. The user can also add natures (the numbers 25 to 31).
//!
//! The table goes on the SD card as `nature/natures.csv` (see `sango_core::natures`). The plugin reads it in the game.
//! A new nature works for the statistics only: the game does not know it for the flavors (Pokéblocks, berries that confuse) and for the
//! names, so the name of a new nature shows only in Sango Studio.

use sango_core::natures::{Nature as Row, NatureTable, GAME_COUNT, MAX_CHANGE, MAX_COUNT, NAME_LANGUAGES, STATS};
use sango_core::text::{TextFile, LANGUAGES};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;

const FILE: &str = "nature/natures.csv";

/// The file of the texts of the game with the names of the natures.
const NAME_FILE: usize = 51;

/// The language of each name column (the order of `NAME_LANGUAGES`) in the list of the languages of the game.
const TEXT_LANGUAGE: [usize; 8] = [2, 0, 1, 3, 4, 5, 6, 7];

/// The longest name of a new nature.
const NAME_SIZE: usize = 16;

/// The Nature page.
pub struct Nature {
    opened_for: Option<PathBuf>,
    table: NatureTable,
    saved: NatureTable,
    from_card: bool,
    status: Option<(bool, String)>,
    /// The language of the names that the table shows (the place in `NAME_LANGUAGES`).
    language: usize,
    /// The names of the natures of the game, for each language that was shown.
    game_names: HashMap<usize, Vec<String>>,
}

impl Default for Nature {
    fn default() -> Self {
        Nature { opened_for: None, table: NatureTable::game(), saved: NatureTable::game(), from_card: false, status: None, language: 0, game_names: HashMap::new() }
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

    fn tool_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let changed = self.table != self.saved;
                ui.add_enabled_ui(changed && sd.is_some(), |ui| {
                    if widget::button(ui, "Save") {
                        if let Some(sd) = sd {
                            self.save(sd);
                        }
                    }
                });
                ui.add_enabled_ui(changed, |ui| {
                    if widget::button(ui, "Undo") {
                        self.table = self.saved.clone();
                        self.status = None;
                    }
                });
                if !self.table.is_game() && widget::confirm_button(ui, "Reset to the game", "Click again to reset") {
                    self.table = NatureTable::game();
                    self.status = None;
                }
                let full = self.table.natures.len() >= MAX_COUNT;
                ui.add_enabled_ui(!full, |ui| {
                    if widget::button(ui, "Add a nature") {
                        let number = self.table.natures.len() + 1;
                        self.table.natures.push(Row { name: format!("Nature {number}"), change: [0; 5], other_names: Default::default() });
                    }
                });
                if full {
                    ui.label(widget::hint("The table has the most natures that the game can use."));
                }
                if changed {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_card {
                    widget::badge(ui, "From the SD card", color::ACCENT);
                } else {
                    widget::badge(ui, "From the game", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
    }

    /// The names of the natures of the game in a language: the texts of the game, else the English names.
    fn game_names_for(&mut self, dump: Option<&Dump>, language: usize) -> Vec<String> {
        if let Some(found) = self.game_names.get(&language) {
            return found.clone();
        }
        let from_game: Vec<String> = dump
            .and_then(|d| Garc::open(&d.archive(LANGUAGES[TEXT_LANGUAGE[language]].archive)).ok())
            .and_then(|garc| garc.first_file(NAME_FILE).and_then(|bytes| TextFile::from_bytes(bytes).ok()))
            .map(|file| (0..GAME_COUNT).map(|n| file.text(n).unwrap_or_default()).collect())
            .unwrap_or_default();
        let names = if from_game.len() == GAME_COUNT { from_game } else { (0..GAME_COUNT).map(|n| Row::game(n).name).collect() };
        self.game_names.insert(language, names.clone());
        names
    }

    fn table_card(&mut self, ui: &mut egui::Ui, dump: Option<&Dump>) {
        widget::card(ui, |ui| {
            ui.label(widget::hint(
                "Write the change of each statistic in percent. +10 raises it by 10 % and -10 lowers it by 10 % (like the natures of the game), +50 raises it \
                 by 50 %, -200 lowers it by 200 %, 0 changes nothing. The new value is the value times (100 + the change), divided by 100, and a statistic \
                 is never under 1. The change can be from -1000 to 1000, on any nature.",
            ));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Names in"));
                for (place, name) in NAME_LANGUAGES.iter().enumerate() {
                    if widget::tab_button(ui, name, self.language == place, false).clicked() {
                        self.language = place;
                    }
                }
            });
            ui.label(widget::hint(
                "The game shows these names in its own language. You can rename the natures of the game too: a name that you leave empty keeps the name of the \
                 game (grey). For a new nature (number 25 and more), an empty name is the English name.",
            ));
            ui.add_space(6.0);
            let game_names = self.game_names_for(dump, self.language);
            let language = self.language;
            let mut remove = None;
            let mut reset = None;
            let mut restore_name = None;
            let count = self.table.natures.len();
            egui::Grid::new("nature_table").num_columns(9).spacing([12.0, 6.0]).striped(true).show(ui, |ui| {
                ui.label(widget::hint("Nature"));
                for name in STATS {
                    ui.label(widget::hint(&format!("{name} (%)")));
                }
                ui.label(widget::hint("What it does"));
                ui.label("");
                ui.label("");
                ui.end_row();
                for number in 0..count {
                    let is_new = number >= GAME_COUNT;
                    let game = if is_new { Row { name: String::new(), change: [0; 5], other_names: Default::default() } } else { Row::game(number) };
                    if !is_new {
                        // A nature of the game: its name can be changed, an empty name is the name of the game.
                        let shown = game_names.get(number).cloned().unwrap_or_else(|| game.name.clone());
                        let text = if language == 0 { &mut self.table.natures[number].name } else { &mut self.table.natures[number].other_names[language - 1] };
                        let response = ui.add(egui::TextEdit::singleline(text).hint_text(shown).desired_width(140.0).char_limit(NAME_SIZE));
                        if language == 0 && response.lost_focus() && self.table.natures[number].name.is_empty() {
                            restore_name = Some(number);
                        }
                    } else if is_new {
                        if language == 0 {
                            ui.add(egui::TextEdit::singleline(&mut self.table.natures[number].name).desired_width(140.0).char_limit(NAME_SIZE));
                        } else {
                            let english = self.table.natures[number].name.clone();
                            ui.add(
                                egui::TextEdit::singleline(&mut self.table.natures[number].other_names[language - 1])
                                    .hint_text(english)
                                    .desired_width(140.0)
                                    .char_limit(NAME_SIZE),
                            );
                        }
                    }
                    for stat in 0..5 {
                        widget::spin(ui, &mut self.table.natures[number].change[stat], -f64::from(MAX_CHANGE), f64::from(MAX_CHANGE));
                    }
                    ui.label(widget::hint(&self.table.natures[number].summary()));
                    let changed = self.table.natures[number].change != game.change || (!is_new && (self.table.natures[number].name != game.name || self.table.natures[number].other_names != game.other_names));
                    if changed {
                        if widget::button(ui, if is_new { "Clear" } else { "Game" }) {
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
                self.table.natures[number].name = Row::game(number).name;
            }
            if let Some(number) = reset {
                let game = if number >= GAME_COUNT { None } else { Some(Row::game(number)) };
                self.table.natures[number].change = game.as_ref().map_or([0; 5], |g| g.change);
                if let Some(game) = game {
                    self.table.natures[number].name = game.name;
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
        "Na"
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
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt("nature_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.table_card(ui, session.dump.as_ref());
        });
    }
}
