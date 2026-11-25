//! The Formula page: the user changes the formulas of the game.
//!
//! Two parts:
//! - **Statistics**: the formula of each of the six statistics (HP, Attack, Defense, Sp. Atk, Sp. Def, Speed), before the nature. Each
//!   formula is a text; the page checks it and tells what is wrong.
//! - **Shiny**: the chance of a shiny Pokémon, in percent.
//!
//! The files go on the SD card, in the folder `formula` (see `storage.rs`). The plugin reads them in the game. The natures have their own
//! page (`sango-nature`).

mod defaults;
mod expr;
mod storage;

use defaults::{shiny_game_percent, SHINY_OUT_OF, STATS};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;
use storage::Formulas;

const TABS: [&str; 2] = ["Statistics", "Shiny"];

/// The Formula page.
pub struct Formula {
    opened_for: Option<PathBuf>,
    formulas: Formulas,
    saved: Formulas,
    from_card: bool,
    status: Option<(bool, String)>,
    tab: usize,
}

impl Default for Formula {
    fn default() -> Self {
        Formula {
            opened_for: None,
            formulas: Formulas::game(),
            saved: Formulas::game(),
            from_card: false,
            status: None,
            tab: 0,
        }
    }
}

impl Formula {
    fn open(&mut self, sd: &SdCard) {
        self.opened_for = Some(sd.root().to_path_buf());
        self.from_card = sd.read(storage::TEXT_FILE).is_some();
        self.status = None;
        match Formulas::read(sd) {
            Ok(formulas) => {
                self.saved = formulas.clone();
                self.formulas = formulas;
            }
            Err(message) => {
                self.saved = Formulas::game();
                self.formulas = Formulas::game();
                self.status = Some((false, message));
            }
        }
    }

    fn save(&mut self, sd: &SdCard) {
        self.status = Some(match self.formulas.save(sd) {
            Ok(()) => {
                self.saved = self.formulas.clone();
                self.from_card = !self.formulas.is_game();
                (true, "Saved. The game reads the formulas again by itself.".to_string())
            }
            Err(message) => (false, message),
        });
    }

    fn export_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV file", &["csv"])
            .add_filter("Text file", &["txt"])
            .set_file_name("formulas.csv")
            .save_file()
        else {
            return;
        };
        let content = if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("txt")) {
            self.formulas.to_text()
        } else {
            self.formulas.to_csv()
        };
        self.status = Some(match std::fs::write(&path, content) {
            Ok(()) => (true, format!("Exported formulas to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn import_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Formulas (CSV or text)", &["csv", "txt"])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))
            .and_then(|text| Formulas::parse(&text));
        self.status = Some(match result {
            Ok(formulas) => {
                self.formulas = formulas;
                (true, format!("Imported formulas from {}.", path.display()))
            }
            Err(message) => (false, message),
        });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if widget::button(ui, "Export") {
                    self.export_file();
                }
                if widget::button(ui, "Import") {
                    self.import_file();
                }
                ui.add_enabled_ui(!self.formulas.is_game(), |ui| {
                    if widget::confirm_button(ui, "Reset", "Click again to reset") {
                        self.formulas = Formulas::game();
                        self.status = Some((true, "Reset all formulas to default.".to_string()));
                    }
                });
                let changed = self.formulas != self.saved;
                ui.add_enabled_ui(changed && sd.is_some(), |ui| {
                    if widget::save_button(ui, self.unsaved()) {
                        if let Some(sd) = sd {
                            self.save(sd);
                        }
                    }
                });
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

    fn statistics_tab(&mut self, ui: &mut egui::Ui) {
        widget::card(ui, |ui| {
            ui.label(widget::hint(
                "Write the formula that gives the statistic before the nature (the Nature page then multiplies it). The variables are base, iv, ev, level \
                 and species. You can use + - * / %, brackets, the comparisons == < >, and the functions min(a, b), max(a, b) and if(condition, then, else). \
                 The numbers are whole numbers: 7 / 2 is 3.",
            ));
            ui.add_space(4.0);
            for (name, help) in expr::VARIABLES {
                ui.label(egui::RichText::new(format!("{name} — {help}")).monospace().size(12.5));
            }
        });
        ui.add_space(6.0);
        for stat in 0..6 {
            widget::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(widget::title(STATS[stat].0));
                    let is_game = Formulas::is_game_text(stat, &self.formulas.texts[stat]);
                    if is_game {
                        widget::badge(ui, "Like the game", color::GREEN);
                    } else {
                        widget::badge(ui, "Changed", color::AMBER);
                    }
                    ui.add_enabled_ui(!is_game, |ui| {
                        if widget::button(ui, "Reset") {
                            self.formulas.texts[stat] = STATS[stat].1.to_string();
                        }
                    });
                });
                ui.add(egui::TextEdit::singleline(&mut self.formulas.texts[stat]).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).id_salt(("formula", stat)));
                match expr::compile(&self.formulas.texts[stat]) {
                    Ok(program) => {
                        ui.label(widget::hint(&format!("The formula is correct ({} of {} bytes).", program.len(), expr::MAX_SIZE)));
                    }
                    Err(message) => {
                        ui.label(egui::RichText::new(message).color(color::ROSE));
                    }
                }
            });
            ui.add_space(4.0);
        }
    }

    fn shiny_tab(&mut self, ui: &mut egui::Ui) {
        widget::card(ui, |ui| {
            ui.label(widget::title("Chance of a shiny Pokémon"));
            ui.label(widget::hint(
                "The game gives each Pokémon a number and compares it with the numbers of the trainer. It makes a shiny Pokémon 16 times out of 65536, which is \
                 1 out of 4096. Here you give the chance in percent: 100 % makes every Pokémon shiny, 0 % makes none. The setting of the menu of the plugin \
                 (Shiny rate), when it is not off, has priority over this chance.",
            ));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Chance"));
                ui.add(egui::DragValue::new(&mut self.formulas.shiny_percent).speed(0.01).range(0.0..=100.0).max_decimals(6).suffix(" %"));
                let threshold = self.formulas.threshold();
                if threshold == 0 {
                    ui.label("No Pokémon is shiny.");
                } else if threshold >= SHINY_OUT_OF {
                    ui.label("Every Pokémon is shiny.");
                } else {
                    let odds = f64::from(SHINY_OUT_OF) / f64::from(threshold);
                    ui.label(format!("1 out of {:.1} ({} out of {})", odds, threshold, SHINY_OUT_OF));
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Quick choice"));
                if widget::button(ui, "Reset") {
                    self.formulas.shiny_percent = shiny_game_percent();
                }
                for percent in [0.1, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0] {
                    if widget::button(ui, format!("{percent} %")) {
                        self.formulas.shiny_percent = percent;
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(widget::hint(
                "The chance has a step of 1 out of 65536 (about 0.0015 %): the page rounds the number that you give.",
            ));
        });
    }
}

impl Module for Formula {
    fn title(&self) -> &str {
        "Formula"
    }

    fn icon(&self) -> &str {
        "🧮"
    }

    fn unsaved(&self) -> bool {
        self.formulas != self.saved
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Formula");
        ui.add_space(8.0);
        let Some(sd) = session.sd.as_ref() else {
            widget::missing_card(ui, "Choose the SD card on the Home page to change the formulas.");
            return;
        };
        if self.opened_for.as_deref() != Some(sd.root()) {
            self.open(sd);
        }
        let sd = session.sd.clone();
        self.tool_bar(ui, sd.as_ref());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (place, name) in TABS.iter().enumerate() {
                if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                    self.tab = place;
                }
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt("formula_scroll").auto_shrink([false, false]).show(ui, |ui| {
            if self.tab == 0 {
                self.statistics_tab(ui);
            } else {
                self.shiny_tab(ui);
            }
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_page_starts_with_the_formulas_of_the_game() {
        let page = Formula::default();
        assert!(page.formulas.is_game());
        assert!(!page.unsaved());
    }
}
