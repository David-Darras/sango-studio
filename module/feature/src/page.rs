//! The page: a tab for each kind of settings, and a widget for each setting.

use crate::list::{self, Choices, Kind, Setting, Value, PATH, SETTINGS, TABS};
use sango_core::SdCard;
use sango_ui::history::History;
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module, Session};
use std::path::PathBuf;

pub struct FeaturePage {
    /// The SD card that the page read.
    card: Option<PathBuf>,
    /// The settings that the page changes, and the ones of the card.
    choices: Choices,
    saved: Choices,
    tab: usize,
    status: Option<(bool, String)>,
    reset_open: bool,
    /// What the button Reset puts back: the open tab, every setting.
    reset_parts: [bool; 2],
    history: History<Choices>,
}

impl Default for FeaturePage {
    fn default() -> Self {
        FeaturePage { card: None, choices: Choices::new(), saved: Choices::new(), tab: 0, status: None, reset_open: false, reset_parts: [true, false], history: History::default() }
    }
}

fn read(sd: &SdCard) -> Choices {
    sd.read(PATH).map_or_else(Choices::new, |bytes| list::parse(&String::from_utf8_lossy(&bytes)).0)
}

/// Writes the file, or removes it when no setting changes.
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

/// A menu of the names of a choice.
fn choice_menu(ui: &mut egui::Ui, key: &str, value: &mut i32, names: &[&str]) {
    let shown = usize::try_from(*value).ok().and_then(|at| names.get(at)).map_or_else(|| format!("Value {value}"), |name| name.to_string());
    let combo = egui::ComboBox::from_id_salt(("feature", key)).selected_text(shown).width(260.0).height(360.0).show_ui(ui, |ui| {
        for (at, name) in names.iter().enumerate() {
            ui.selectable_value(value, at as i32, *name);
        }
    });
    let values: Vec<i32> = (0..names.len() as i32).collect();
    focus::combo_arrows(ui, &combo.response, value, &values);
}

/// The widget of a setting.
fn value_widget(ui: &mut egui::Ui, setting: &Setting, value: &mut Value) {
    match (setting.kind, value) {
        (Kind::Switch(_), Value::Switch(on)) => {
            let word = if *on { "On" } else { "Off" };
            widget::checkbox(ui, on, word);
        }
        (Kind::Whole { min, max, .. }, Value::Whole(number)) => {
            widget::spin(ui, number, f64::from(min), f64::from(max));
        }
        (Kind::Choice { names, .. }, Value::Whole(number)) => choice_menu(ui, setting.key, number, names),
        (Kind::Species(_), Value::Whole(number)) => {
            let mut chosen = ((*number).max(1) as usize, 0);
            // The plugin takes a species: the first form of each one.
            if sango_species::menus::pokemon_menu(ui, setting.key, &mut chosen, 260.0, Some(&|_, form| form == 0), None) && chosen.0 != 0 {
                *number = chosen.0 as i32;
            }
        }
        (Kind::Decimal { min, max, step, .. }, Value::Decimal(number)) => {
            if widget::decimal(ui, number, step, min..=max).changed() {
                // Steps of 0.1 give long numbers: three decimals are enough.
                *number = (*number * 1000.0).round() / 1000.0;
            }
        }
        (Kind::Color(_), Value::Color(rgb)) => {
            // A row of the height of the number fields: the widgets then stay on one line.
            ui.allocate_ui_with_layout(egui::vec2(380.0, 40.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let mut parts = [(*rgb >> 16) & 0xFF, (*rgb >> 8) & 0xFF, *rgb & 0xFF];
                let (swatch, _) = ui.allocate_exact_size(egui::vec2(28.0, 22.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 4.0, egui::Color32::from_rgb(parts[0] as u8, parts[1] as u8, parts[2] as u8));
                ui.painter().rect_stroke(swatch, 4.0, egui::Stroke::new(1.0_f32, color::FIELD_BORDER));
                for (name, part) in ["R", "G", "B"].iter().zip(parts.iter_mut()) {
                    ui.label(widget::hint(name));
                    widget::spin(ui, part, 0.0, 255.0);
                }
                *rgb = parts[0] << 16 | parts[1] << 8 | parts[2];
            });
        }
        // The value of a line always has the kind of its setting.
        _ => {}
    }
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
        let Some(path) = rfd::FileDialog::new().set_title("Export the settings of the plugin").set_file_name(PATH).add_filter("Settings", &["ini"]).save_file() else { return };
        self.status = Some(match std::fs::write(&path, list::format(&self.choices)) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Import settings of the plugin").add_filter("Settings", &["ini"]).pick_file() else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (choices, wrong) = list::parse(&text);
                self.choices = choices;
                let note = if wrong == 0 { String::new() } else { format!(" {wrong} line(s) that are not understood are left out.") };
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
                        notes: widget::ActionNotes { export: "Writes the settings that you changed in a file.", import: "Reads a file of settings.", reset: "Gives settings the value of the game again: choose them in the window.", ..Default::default() },
                        ..Default::default()
                    },
                );
                self.history.buttons(ui, true);
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if !self.saved.is_empty() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "The game", color::GREEN);
                }
                if !self.choices.is_empty() {
                    ui.label(widget::hint(&format!("{} setting(s) changed", self.choices.len())));
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
        if clicks.reset {
            self.reset_parts = [true, false];
            self.reset_open = true;
        }
        if let (true, Some(sd)) = (clicks.save, sd) {
            self.save(sd);
        }
        let [tab, all] = &mut self.reset_parts;
        let open_tab = format!("The tab {}", TABS[self.tab]);
        if widget::reset_window(ui.ctx(), &mut self.reset_open, "The settings of the plugin", &mut [(&open_tab, tab), ("Every setting", all)]) {
            if let Some(sd) = sd {
                self.reset(sd);
            }
        }
    }

    fn reset(&mut self, sd: &SdCard) {
        let [tab, all] = self.reset_parts;
        let open = self.tab;
        self.choices.retain(|key, _| !(all || tab && list::find(key).is_some_and(|setting| setting.tab == open)));
        self.save(sd);
    }

    /// The cards of the open tab: one for each group of settings.
    fn settings(&mut self, ui: &mut egui::Ui) {
        let of_tab: Vec<&Setting> = SETTINGS.iter().filter(|setting| setting.tab == self.tab).collect();
        for group in of_tab.chunk_by(|a, b| a.group == b.group) {
            widget::card(ui, |ui| {
                ui.set_width(720.0_f32.min(ui.available_width()));
                ui.label(widget::title(group[0].group));
                ui.add_space(4.0);
                egui::Grid::new(("features", self.tab, group[0].group)).num_columns(2).spacing(egui::vec2(18.0, 8.0)).min_col_width(300.0).show(ui, |ui| {
                    for setting in group {
                        let normal = setting.kind.default_value();
                        let changed = self.choices.contains_key(setting.key);
                        let back = ui
                            .horizontal(|ui| {
                                let (name, back) = widget::field_name(ui, setting.name, changed);
                                if !setting.note.is_empty() {
                                    name.on_hover_text(setting.note);
                                }
                                back
                            })
                            .inner;
                        let mut value = self.choices.get(setting.key).copied().unwrap_or(normal);
                        value_widget(ui, setting, &mut value);
                        // Only a value that is not the one of the game goes in the file.
                        if back || value == normal {
                            self.choices.remove(setting.key);
                        } else {
                            self.choices.insert(setting.key.to_string(), value);
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
        ui.label(widget::hint("Without a change, the game is the game. A change applies when the game starts; the menu of the plugin can still change it."));
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
        widget::tabs(ui, "features", &TABS, &mut self.tab);
        ui.add_space(6.0);
        egui::ScrollArea::vertical().id_salt(("features_scroll", self.tab)).auto_shrink([false, false]).show(ui, |ui| self.settings(ui));
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
    fn a_change_is_saved_and_the_values_of_the_game_remove_the_file() {
        let (sd, folder) = card("save");
        let mut page = FeaturePage::default();
        page.choices.insert("battle.hp_gradient".to_string(), Value::Switch(true));
        page.choices.insert("title.top_video".to_string(), Value::Whole(2));
        page.save(&sd);
        assert_eq!(read(&sd), page.choices);
        assert!(folder.join(PATH).exists(), "the plugin reads the file of the SD card");
        // The open tab goes back to the game, the other ones stay.
        page.tab = list::find("battle.hp_gradient").unwrap().tab;
        page.reset_parts = [true, false];
        page.reset(&sd);
        assert_eq!(page.choices.keys().collect::<Vec<_>>(), ["title.top_video"]);
        page.reset_parts = [false, true];
        page.reset(&sd);
        assert!(sd.read(PATH).is_none() && !folder.join(PATH).exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the keys alone: every tab opens without a change, Down goes to a setting, Space changes it, and Ctrl+Z takes it back.
    #[test]
    fn the_keys_reach_the_settings() {
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
        for tab in 0..TABS.len() {
            page.tab = tab;
            frame(&mut page, Default::default());
            assert!(!page.unsaved(), "tab {}: the page changes nothing by itself: {:?}", TABS[tab], page.choices);
        }
        page.tab = 0;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), widget::tab_id("features", 0)));
        frame(&mut page, Default::default());
        frame(&mut page, key(egui::Key::ArrowRight));
        frame(&mut page, key(egui::Key::ArrowLeft));
        assert_eq!(page.tab, 0, "the arrows open the tabs");
        // The first setting of the tab Graphics is a check box.
        frame(&mut page, key(egui::Key::ArrowDown));
        frame(&mut page, key(egui::Key::Space));
        assert_eq!(page.choices.get("graphics.outlines"), Some(&Value::Switch(false)), "{:?}", page.choices);
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert!(page.choices.is_empty(), "undone: {:?}", page.choices);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
