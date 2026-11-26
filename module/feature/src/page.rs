//! The page: one check box for each feature.

use crate::list::{self, Choices, GROUPS, PATH};
use sango_core::SdCard;
use sango_ui::history::History;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

pub struct FeaturePage {
    /// The SD card that the page read.
    card: Option<PathBuf>,
    /// The features that the page changes, and the ones of the card.
    choices: Choices,
    saved: Choices,
    status: Option<(bool, String)>,
    reset_open: bool,
    reset_all: bool,
    history: History<Choices>,
}

impl Default for FeaturePage {
    fn default() -> Self {
        FeaturePage { card: None, choices: Choices::new(), saved: Choices::new(), status: None, reset_open: false, reset_all: true, history: History::default() }
    }
}

fn read(sd: &SdCard) -> Choices {
    sd.read(PATH).map_or_else(Choices::new, |bytes| list::parse(&String::from_utf8_lossy(&bytes)))
}

/// Writes the file, or removes it when no feature changes.
fn write(sd: &SdCard, choices: &Choices) -> Result<(), String> {
    if choices.is_empty() {
        if sd.read(PATH).is_none() {
            return Ok(());
        }
        sd.remove(PATH).map(|_| ())?;
    } else {
        sd.write(PATH, list::format(choices).as_bytes()).map(|_| ())?;
    }
    sd.mark_changed(PATH)
}

impl FeaturePage {
    fn load(&mut self, sd: Option<&SdCard>) {
        self.saved = sd.map_or_else(Choices::new, read);
        self.choices = self.saved.clone();
        self.history.clear();
        self.status = None;
    }

    fn save(&mut self, sd: &SdCard) {
        match write(sd, &self.choices) {
            Ok(()) => {
                self.saved = self.choices.clone();
                self.status = Some((true, "Saved. Restart the game to use the change.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn export(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Export the features").set_file_name(PATH).add_filter("Settings", &["ini"]).save_file() else { return };
        self.status = Some(match std::fs::write(&path, list::format(&self.choices)) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Import features").add_filter("Settings", &["ini"]).pick_file() else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut choices = list::parse(&text);
                let unknown = choices.keys().filter(|key| list::find(key).is_none()).count();
                choices.retain(|key, on| list::find(key).is_some_and(|feature| feature.on != *on));
                self.choices = choices;
                let note = if unknown == 0 { String::new() } else { format!(" {unknown} unknown line(s) are left out.") };
                self.status = Some((true, format!("Imported. Save to put it on the SD card.{note}")));
            }
            Err(error) => self.status = Some((false, format!("{}: {error}", path.display()))),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let dirty = self.choices != self.saved;
        let mut clicks = widget::ActionClicks::default();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let card = sd.is_some();
                clicks = widget::action_bar(
                    ui,
                    widget::ActionBar {
                        export: Some(true),
                        import: Some(true),
                        reset: Some(card),
                        save: Some(card && dirty),
                        notes: widget::ActionNotes { export: "Writes the features that you changed in a file.", import: "Reads a file of features.", reset: "Gives every feature the value of the plugin again.", ..Default::default() },
                        ..Default::default()
                    },
                );
                self.history.buttons(ui, true);
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if !self.saved.is_empty() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the plugin", color::GREEN);
                }
            });
            if sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        if clicks.export {
            self.export();
        }
        if clicks.import {
            self.import();
        }
        self.reset_open |= clicks.reset;
        if let (true, Some(sd)) = (clicks.save, sd) {
            self.save(sd);
        }
        if widget::reset_window(ui.ctx(), &mut self.reset_open, "The features", &mut [("All the features", &mut self.reset_all)]) {
            if let (true, Some(sd)) = (self.reset_all, sd) {
                self.choices.clear();
                self.save(sd);
            }
        }
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        for (title, features) in GROUPS {
            widget::card(ui, |ui| {
                ui.set_width(680.0_f32.min(ui.available_width()));
                ui.label(widget::title(title));
                ui.add_space(4.0);
                egui::Grid::new(("features", title)).num_columns(2).spacing(egui::vec2(18.0, 8.0)).min_col_width(320.0).show(ui, |ui| {
                    for feature in features {
                        let changed = self.choices.contains_key(feature.key);
                        let back = ui
                            .horizontal(|ui| {
                                let (name, back) = widget::field_name(ui, feature.name, changed);
                                name.on_hover_text(feature.note);
                                back
                            })
                            .inner;
                        let mut on = self.choices.get(feature.key).copied().unwrap_or(feature.on);
                        let word = if on { "On" } else { "Off" };
                        widget::checkbox(ui, &mut on, word).on_hover_text(feature.note);
                        if back {
                            on = feature.on;
                        }
                        // Only a value that is not the one of the plugin goes in the file.
                        if on == feature.on {
                            self.choices.remove(feature.key);
                        } else {
                            self.choices.insert(feature.key.to_string(), on);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(10.0);
        }
    }
}

impl Module for FeaturePage {
    fn title(&self) -> &str {
        "Features"
    }

    fn icon(&self) -> &str {
        "🔧"
    }

    fn unsaved(&self) -> bool {
        self.choices != self.saved
    }

    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn discard_changes(&mut self) -> bool {
        self.choices = self.saved.clone();
        self.history.clear();
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Features");
        ui.label(widget::hint("What the plugin does when the game starts. Its menu can still change a feature while you play."));
        ui.add_space(8.0);
        let sd = session.sd.clone();
        let now = sd.as_ref().map(|sd| sd.root().to_path_buf());
        // The first frame reads the card, and again when the user chooses another one.
        if self.card != now {
            self.card = now;
            self.load(sd.as_ref());
        }
        self.top_bar(ui, sd.as_ref());
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("features_scroll").auto_shrink([false, false]).show(ui, |ui| self.list(ui));
        if !ui.input(|i| i.pointer.any_down()) {
            self.history.track(ui, 0, &mut self.choices);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn card(name: &str) -> (SdCard, PathBuf) {
        let folder = std::env::temp_dir().join(format!("sango_feature_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        (SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap(), folder)
    }

    #[test]
    fn a_change_is_saved_and_the_values_of_the_plugin_remove_the_file() {
        let (sd, folder) = card("save");
        let mut page = FeaturePage::default();
        page.choices.insert("title_screen".to_string(), false);
        page.save(&sd);
        assert_eq!(read(&sd), page.choices);
        assert!(folder.join(PATH).exists(), "the plugin reads the file of the SD card");
        page.choices.clear();
        page.save(&sd);
        assert!(sd.read(PATH).is_none() && !folder.join(PATH).exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the keys alone: Down goes from the top bar to the features, Space changes one, and Ctrl+Z takes the change back.
    #[test]
    fn the_keys_reach_the_features() {
        let (sd, folder) = card("keys");
        let mut session = Session { sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = FeaturePage::default();
        let ctx = egui::Context::default();
        let mut frame = |page: &mut FeaturePage, input: egui::RawInput| {
            for input in [input, Default::default()] {
                let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..input };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
                });
            }
        };
        let key = |key| egui::RawInput { events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], ..Default::default() };
        frame(&mut page, Default::default());
        assert!(!page.unsaved(), "the page changes nothing by itself");
        frame(&mut page, key(egui::Key::ArrowDown));
        frame(&mut page, key(egui::Key::Space));
        assert_eq!(page.choices.len(), 1, "{:?}", page.choices);
        frame(&mut page, key(egui::Key::ArrowDown));
        frame(&mut page, key(egui::Key::Space));
        assert_eq!(page.choices.len(), 2, "Down goes to the next feature: {:?}", page.choices);
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert!(page.choices.len() < 2, "undone: {:?}", page.choices);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
