//! The ability editor.
//!
//! An ability has no data file: its effect is in the code of the game. The page edits what the game keeps in files: the name
//! and the description, in the eight languages. It also shows which species and forms have the ability (from the data of the
//! species), and a click on a species opens it in the Species page, where the abilities are chosen.
//!
//! The files that the user changes go on the SD card, in the folder `text`: the romfs is never changed.

mod picker;
mod texts;
mod tools;

use sango_core::{Dump, SdCard};
use sango_species::SpeciesAbilities;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of abilities (the ability 0 is no ability).
pub const COUNT: usize = 191;

/// The tabs of the page.
const TABS: [&str; 2] = ["General", "Text"];

/// The names of the three ability slots of a species.
const SLOTS: [&str; 3] = ["Ability 1", "Ability 2", "Hidden ability"];

/// A species or a form that has an ability.
type User = (usize, usize);

pub struct Ability {
    /// The dump that the archives come from.
    root: Option<PathBuf>,
    id: usize,
    tab: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The names of the abilities in the list, as the user has them.
    list_names: Option<Vec<String>>,
    /// For each ability, the species and forms that have it in each slot. Read from the data of the species.
    usage: Vec<[Vec<User>; 3]>,
    usage_ready: bool,
    texts: texts::AbilityTexts,
    tools: tools::Tools,
}

impl Default for Ability {
    fn default() -> Self {
        Ability { root: None, id: 1, tab: 0, status: None, list_names: None, usage: Vec::new(), usage_ready: false, texts: texts::AbilityTexts::default(), tools: tools::Tools::default() }
    }
}

impl Ability {
    /// Opens the archives of a dump.
    fn open(&mut self, dump: &Dump) {
        self.root = Some(dump.root().to_path_buf());
        self.texts.open(dump);
        self.list_names = None;
        self.usage_ready = false;
    }

    /// Reads the abilities of all the species again.
    fn read_usage(&mut self, session: &Session) {
        self.usage = (0..=COUNT).map(|_| [Vec::new(), Vec::new(), Vec::new()]).collect();
        match sango_species::species_abilities(session) {
            Ok(all) => {
                for SpeciesAbilities { species, form, abilities } in all {
                    for (slot, &ability) in abilities.iter().enumerate() {
                        if let Some(users) = self.usage.get_mut(ability as usize).filter(|_| ability != 0) {
                            users[slot].push((species, form));
                        }
                    }
                }
            }
            Err(message) => self.status = Some((false, message)),
        }
        self.usage_ready = true;
    }

    /// The number of species and forms that have an ability, in any slot.
    fn user_count(&self, id: usize) -> usize {
        let Some(slots) = self.usage.get(id) else { return 0 };
        let mut all: Vec<&User> = slots.iter().flatten().collect();
        all.sort_unstable();
        all.dedup();
        all.len()
    }

    /// The rows of the ability list as the user has them: the names, and the number of species.
    fn menu_rows(&mut self, session: &Session) -> Vec<picker::Row> {
        let sd = session.sd.as_ref();
        if self.list_names.is_none() {
            let names = (0..=COUNT).map(|n| self.texts.name(sd, n)).collect();
            self.list_names = Some(names);
        }
        let names = self.list_names.clone().unwrap_or_default();
        (0..=COUNT).map(|n| picker::Row { name: names.get(n).cloned().unwrap_or_default(), species: self.user_count(n) }).collect()
    }

    /// Writes the texts on the SD card, and tells the plugin.
    fn save(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        match self.texts.save(sd) {
            Ok(_) => {
                self.list_names = None;
                self.status = Some((true, "Saved. The files are on the SD card.".into()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The picker, the buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut save = false;
        let mut reset_tab = false;
        let mut reset_more = false;
        let mut undo = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let rows = self.menu_rows(session);
                let mut chosen = self.id;
                picker::ability_menu(ui, "ability_menu", &mut chosen, 300.0, &rows);
                self.id = chosen.clamp(1, COUNT);
                let has_card = session.sd.is_some();
                ui.add_enabled_ui(has_card, |ui| {
                    reset_tab = widget::button(ui, "Reset texts");
                })
                .response
                .on_hover_text("Puts the name and the description of this ability back to the romfs. Undo gives them back.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Reset one ability or all the abilities.");
                let undo_label = self.tools.history.last().map(|backup| format!("Undo: {}", backup.label));
                ui.add_enabled_ui(undo_label.is_some() && has_card, |ui| {
                    undo = widget::button(ui, "Undo");
                })
                .response
                .on_hover_text(undo_label.unwrap_or_else(|| "Nothing to undo.".into()));
                save = has_card && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
                if self.texts.changed() {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.texts.from_card() {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
        if reset_tab {
            self.run_reset(session, tools::Scope::Ability);
        }
        if reset_more {
            self.tools.reset_open = true;
        }
        if undo {
            self.undo(session);
        }
        if save {
            self.save(session);
        }
    }

    /// The General tab: the name and the description, what an ability is, who has it, and the CSV card.
    fn general_tab(&mut self, ui: &mut egui::Ui, session: &Session) {
        let sd = session.sd.as_ref();
        let name = self.texts.text(sd, texts::ENGLISH, 0, self.id);
        let description = self.texts.text(sd, texts::ENGLISH, 1, self.id);
        egui::ScrollArea::vertical().id_salt("ability_general").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_min_width(700.0);
                ui.label(widget::title(&format!("#{:03} {name}", self.id)));
                ui.label(&description);
                ui.add_space(4.0);
                ui.label(widget::hint("The text is the English one: the Text tab changes it, in every language."));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(widget::hint("An ability has no data in a file: the game runs its effect from its code, so this page edits its texts and shows who has it."));
                    ui.hyperlink_to("Bulbapedia ↗", "https://bulbapedia.bulbagarden.net/wiki/Ability");
                });
            });
            ui.add_space(10.0);
            self.users_card(ui, session);
            ui.add_space(10.0);
            self.csv_card(ui, session);
            ui.add_space(10.0);
        });
    }

    /// The card of the species that have the ability, for each slot.
    fn users_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let users = self.usage.get(self.id).cloned().unwrap_or_default();
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            ui.horizontal(|ui| {
                ui.label(widget::title(&format!("Species with this ability ({})", self.user_count(self.id))));
                if widget::button(ui, "Read again") {
                    self.read_usage(session);
                }
            });
            ui.label(widget::hint("A click on a species opens it in the Species page, where its abilities are chosen. The data of the SD card counts."));
            ui.add_space(6.0);
            for (slot, title) in SLOTS.iter().enumerate() {
                let list = users.get(slot).cloned().unwrap_or_default();
                ui.label(egui::RichText::new(format!("{title} ({})", list.len())).strong());
                if list.is_empty() {
                    ui.label(widget::hint("No species."));
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                        for (species, form) in list {
                            let name = sango_species::SPECIES.get(species - 1).copied().unwrap_or("?");
                            let label = if form == 0 { format!("#{species:03} {name}") } else { format!("#{species:03} {name} (form {form})") };
                            if widget::button(ui, label) {
                                sango_species::open_species(ui.ctx(), species);
                            }
                        }
                    });
                }
                ui.add_space(8.0);
            }
        });
    }
}

impl Module for Ability {
    fn title(&self) -> &str {
        "Ability"
    }

    fn icon(&self) -> &str {
        "Ab"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Ability");
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
        let session: &Session = session;
        if !self.usage_ready {
            self.read_usage(session);
        }
        self.top_bar(ui, session);
        self.reset_window(ui.ctx(), session);
        ui.add_space(8.0);
        let mut clicked = None;
        egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(12.0)).inner_margin(egui::Margin::same(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, name) in TABS.iter().enumerate() {
                    let tab_id = egui::Id::new(format!("ability_tab_{i}"));
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
            0 => self.general_tab(ui, session),
            _ => {
                let sd: Option<&SdCard> = session.sd.as_ref();
                self.texts.ui(ui, sd, self.id);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the page draws in both tabs, and the species of an ability are found.
    #[test]
    fn the_page_draws_and_finds_the_species_of_an_ability() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_ability_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Ability::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Ability, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for (id, tab) in [(1, 0), (65, 0), (191, 0), (34, 1), (1, 1)] {
            page.id = id;
            page.tab = tab;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        // Overgrow (65) is the first ability of Bulbasaur, Ivysaur and Venusaur; Chlorophyll (34) is an ability of Bulbasaur.
        let users = &page.usage[65];
        assert!(users[0].contains(&(1, 0)) && users[0].contains(&(2, 0)) && users[0].contains(&(3, 0)), "Overgrow in the slot 1");
        assert!(page.user_count(65) > 10);
        assert!(page.usage[34].iter().any(|slot| slot.iter().any(|&(species, _)| species == 1)), "Chlorophyll is an ability of Bulbasaur");
        assert_eq!(page.texts.name(session.sd.as_ref(), 65), "Overgrow");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
