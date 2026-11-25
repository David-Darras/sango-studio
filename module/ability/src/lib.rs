//! The ability editor.
//!
//! An ability has no data file: its effect is in the code of the game. The page edits what the game keeps in files: the name
//! and the description, in the eight languages. It also shows which species and forms have the ability (from the data of the
//! species), and a click on a species opens it in the Species page, where the abilities are chosen.
//!
//! The files that the user changes go on the SD card, in the folder `text`: the romfs is never changed.

mod custom;
mod picker;
pub mod rules;
mod texts;
mod tools;

use sango_core::Dump;
#[cfg(test)]
use sango_core::SdCard;
use sango_species::SpeciesTable;
use sango_species_base::SpeciesAbilities;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of abilities (the ability 0 is no ability).
pub const COUNT: usize = 191;

/// The tabs of the page.


/// The names of the three ability slots of a species.
const SLOTS: [&str; 3] = ["Ability 1", "Ability 2", "Hidden ability"];

/// A species or a form that has an ability.
type User = (usize, usize);

pub struct Ability {
    /// The dump that the archives come from.
    root: Option<PathBuf>,
    id: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The names of the abilities in the list, as the user has them.
    list_names: Option<Vec<String>>,
    /// The entries that are not the same as in the game, found again from time to time.
    modified: sango_core::modified::Cache,
    /// For each ability, the species and forms that have it in each slot. Read from the data of the species.
    usage: Vec<[Vec<User>; 3]>,
    usage_ready: bool,
    /// The list of the species that have the ability, in each slot: the same list as the other pages.
    species_table: SpeciesTable,
    texts: texts::AbilityTexts,
    tools: tools::Tools,
    store: custom::Store,
    /// The language of the texts of a new ability.
    custom_language: usize,
    /// The reaction of the game as rules, for the open ability of the game that has no file yet. It goes in the store at the first
    /// change.
    draft: Option<rules::CustomAbility>,
}

impl Default for Ability {
    fn default() -> Self {
        Ability { root: None, id: 1, status: None, list_names: None, modified: Default::default(), usage: Vec::new(), usage_ready: false, species_table: SpeciesTable::default(), texts: texts::AbilityTexts::default(), tools: tools::Tools::default(), store: custom::Store::default(), custom_language: 2, draft: None }
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
        self.usage = (0..=rules::LAST_ID as usize).map(|_| [Vec::new(), Vec::new(), Vec::new()]).collect();
        match sango_species_base::species_abilities(session) {
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
        self.species_table.load(session);
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

    /// The abilities whose texts are not the same as in the game (the files of the SD card compared with the romfs).
    fn modified_abilities(&mut self, session: &Session) -> std::collections::HashSet<usize> {
        let Some(sd) = session.sd.as_ref() else { return Default::default() };
        let texts = &self.texts;
        self.modified
            .get(|| {
                let files: Vec<usize> = texts::FIELDS.iter().map(|field| field.file).collect();
                sango_core::modified::changed_text_lines(sd, &files, &|language, file| texts.romfs(language, file))
            })
            .clone()
    }

    /// The rows of the ability list as the user has them: the names, and the number of species.
    fn menu_rows(&mut self, session: &Session) -> Vec<picker::Row> {
        let sd = session.sd.as_ref();
        if self.list_names.is_none() {
            let names = (0..=COUNT).map(|n| self.texts.name(sd, n)).collect();
            self.list_names = Some(names);
        }
        let names = self.list_names.clone().unwrap_or_default();
        let mut rows: Vec<picker::Row> = (0..=COUNT).map(|n| picker::Row { name: names.get(n).cloned().unwrap_or_default(), species: self.user_count(n), modified: false }).collect();
        let changed = self.modified_abilities(session);
        let game_rules = self.store.game_ids();
        for (id, row) in rows.iter_mut().enumerate() {
            row.modified = changed.contains(&id) || game_rules.contains(&id);
        }
        // The new abilities come after the abilities of the game, at their numbers: a number between is a row without a name that the list skips.
        if let Some(last) = self.store.ids().into_iter().max() {
            rows.resize(last + 1, picker::Row::default());
            for id in self.store.ids() {
                if let Some(entry) = self.store.find(id) {
                    rows[id] = picker::Row { name: entry.names[rules::ENGLISH].clone(), species: self.user_count(id), modified: false };
                }
            }
        }
        rows
    }

    /// True when the open ability is a new ability of the user (not an ability of the game).
    fn is_custom(&self) -> bool {
        self.id > COUNT
    }

    /// The highest number that the list shows: the last ability of the game, or the last new ability.
    fn last_ability(&self) -> usize {
        self.store.ids().into_iter().max().unwrap_or(COUNT).max(COUNT)
    }

    /// Makes a new ability. When a new ability is open, the new one starts like it (the same texts and reactions).
    fn add_ability(&mut self) {
        let template = self.is_custom().then_some(self.id);
        match self.store.add(template) {
            Ok(id) => {
                self.id = id;
                self.status = Some((true, format!("The new ability {id} is made. Change its name in the Text tab and its reactions in the Reactions tab.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Deletes the open ability when it is a new ability.
    fn delete_ability(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        if !self.is_custom() {
            return;
        }
        let id = self.id;
        match self.store.delete(sd, id) {
            Ok(()) => {
                self.id = 1;
                self.list_names = None;
                self.usage_ready = false;
                self.status = Some((true, format!("The new ability {id} was deleted.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The Reactions tab of an ability of the game. Without a file, the game runs its own code, and the tab shows the reaction of the
    /// game as rules when the rules can do it. The first change puts the rules in the store: "Save" writes the file, and then the
    /// plugin runs the rules in place of the code of the game.
    fn game_reactions_tab(&mut self, ui: &mut egui::Ui, session: &Session) {
        let id = self.id;
        let mut back_to_game = false;
        let mut own_rules = false;
        if let Some(entry) = self.store.find_mut(id) {
            // The rules of the file replace the code of the game.
            ui.horizontal(|ui| {
                ui.label(widget::hint("These rules replace the code of the game."));
                back_to_game = session.sd.is_some() && ui.small_button("Back to the code of the game").clicked();
            });
            custom::reactions_ui(ui, entry);
        } else {
            let preset = rules::presets::game_rules(id as u8);
            if self.draft.as_ref().map(|d| d.id as usize) != Some(id) {
                self.draft = preset.clone().map(|rules| rules::CustomAbility::game(id as u8, rules));
            }
            match &mut self.draft {
                // The reaction of the game as rules: a change makes a file at the next "Save".
                Some(draft) => custom::reactions_ui(ui, draft),
                None => {
                    ui.label(egui::RichText::new(rules::reasons::ability_reason(id as u8)).color(color::AMBER));
                    own_rules = session.sd.is_some() && ui.small_button("Replace with my own rules").on_hover_text("The ability then loses its effect of the game.").clicked();
                }
            }
        }
        if let (Some(draft), Some(preset)) = (&self.draft, rules::presets::game_rules(id as u8)) {
            if draft.rules != preset {
                self.store.insert(draft.clone());
                self.draft = None;
            }
        }
        if own_rules {
            self.store.insert(rules::CustomAbility::game(id as u8, vec![rules::Rule::new()]));
        }
        if back_to_game {
            if let Some(sd) = session.sd.as_ref() {
                match self.store.delete(sd, id) {
                    Ok(()) => self.status = Some((true, format!("The ability {id} runs the code of the game again."))),
                    Err(message) => self.status = Some((false, message)),
                }
            }
            self.draft = None;
        }
    }

    /// Writes the texts on the SD card, and tells the plugin.
    fn save(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let result = self.store.save(sd).map(|_| ()).and_then(|_| self.texts.save(sd).map(|_| ()));
        match result {
            Ok(()) => {
                self.list_names = None;
                self.status = Some((true, "Saved. The files are on the SD card.".into()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The picker, the buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut save = false;
        let mut reset = false;
        let mut add = false;
        let mut delete = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let rows = self.menu_rows(session);
                let mut chosen = self.id;
                picker::ability_menu(ui, "ability_menu", &mut chosen, 300.0, &rows);
                self.id = chosen.clamp(1, self.last_ability());
                let has_card = session.sd.is_some();
                ui.add_enabled_ui(has_card, |ui| {
                    add = widget::button_styled(ui, egui::RichText::new("+ New ability").color(color::WHITE), Some(color::ACCENT));
                })
                .response
                .on_hover_text("Adds an ability: then write its name, its description and its reactions.");
                ui.add_enabled_ui(has_card && self.is_custom(), |ui| {
                    if widget::confirm_button(ui, "Delete", "Click again to delete") {
                        delete = true;
                    }
                })
                .response
                .on_hover_text(if self.is_custom() { "Deletes this new ability." } else { "Only a new ability can be deleted, not one of the game." });
                if widget::button(ui, "Export…") {
                    self.tools.export_open = true;
                }
                ui.add_enabled_ui(has_card, |ui| {
                    if widget::button(ui, "Import…") {
                        self.tools.import_open = true;
                    }
                });
                ui.add_enabled_ui(has_card && !self.is_custom(), |ui| {
                    reset = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Puts the texts or the reactions of one ability or of all the abilities back to the game: choose in the window.");
                save = has_card && widget::save_button(ui, self.unsaved());
                if self.texts.changed() || self.store.changed() {
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
        if add {
            self.add_ability();
        }
        if delete {
            self.delete_ability(session);
        }
        if reset {
            self.tools.reset_open = true;
        }
        if save {
            self.save(session);
        }
    }

    /// The page: the texts and the reactions on the left, the Pokémon that have the ability on the right.
    fn page_view(&mut self, ui: &mut egui::Ui, session: &Session) {
        egui::ScrollArea::vertical().id_salt("ability_page").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(720.0);
                    // The card of the texts, then the card of the reactions.
                    let custom = self.store.find(self.id).is_some_and(|entry| !entry.is_game());
                    widget::card(ui, |ui| {
                        ui.set_min_width(690.0);
                        match self.store.find_mut(self.id).filter(|_| custom) {
                            Some(entry) => {
                                let mut language = self.custom_language;
                                custom::texts_ui(ui, entry, &mut language);
                                self.custom_language = language;
                            }
                            None => self.texts.compact_ui(ui, session.sd.as_ref(), self.id),
                        }
                    });
                    ui.add_space(10.0);
                    widget::card(ui, |ui| {
                        ui.set_min_width(690.0);
                        match self.store.find_mut(self.id).filter(|_| custom) {
                            Some(entry) => custom::reactions_ui(ui, entry),
                            None => self.game_reactions_tab(ui, session),
                        }
                    });
                });
                ui.add_space(12.0);
                ui.vertical(|ui| self.users_card(ui, session));
            });
        });
    }

    /// The card of the species that have the ability, for each slot.
    fn users_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let users = self.usage.get(self.id).cloned().unwrap_or_default();
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title(&format!("Pokémon with this ability ({})", self.user_count(self.id))));
                if widget::button(ui, "Read again") {
                    self.read_usage(session);
                }
            });
            ui.label(widget::hint("A click on a species opens it in the Species page, where its abilities are chosen. The data of the SD card counts."));
            ui.add_space(6.0);
            for (slot, title) in SLOTS.iter().enumerate() {
                // Each form is a Pokémon of its own: Rotom-Heat is not Rotom.
                let mut numbers: Vec<(usize, usize)> = users.get(slot).cloned().unwrap_or_default();
                numbers.sort_unstable();
                numbers.dedup();
                ui.label(egui::RichText::new(format!("{title} ({})", numbers.len())).strong());
                if numbers.is_empty() {
                    ui.label(widget::hint("No species."));
                } else {
                    // Each slot has its own list: its widgets (the header, the scroll area) need their own ids.
                    let clicked = ui.push_id(slot, |ui| self.species_table.show(ui, "ability_users", &numbers, 4.0 * SpeciesTable::ROW_HEIGHT)).inner;
                    if let Some((species, form)) = clicked {
                        sango_species_base::open_species_form(ui.ctx(), species, form);
                    }
                }
                ui.add_space(8.0);
            }
        });
    }
}

impl Module for Ability {
    fn unsaved(&self) -> bool {
        self.texts.changed() || self.store.changed()
    }

    fn title(&self) -> &str {
        "Ability"
    }

    fn icon(&self) -> &str {
        "✨"
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
        self.store.ensure(session.sd.as_ref());
        if !self.usage_ready {
            self.read_usage(session);
        }
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
        for id in [1, 65, 191, 34] {
            page.id = id;
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

#[cfg(test)]
mod add_test {
    use super::*;

    /// With the dump of the game: a new ability is added in the page, edited in the same tabs, saved, and found by another page; it
    /// is deleted.
    #[test]
    fn a_new_ability_is_made_and_edited_like_the_others() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_ability_add_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Ability::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Ability, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        frame(&mut page, &mut session);
        page.add_ability();
        assert_eq!(page.id, 192);
        for _ in 0..3 {
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        page.store.find_mut(192).unwrap().names[rules::ENGLISH] = "Quick Start".into();
        page.save(&session);
        assert!(matches!(page.status, Some((true, _))), "{:?}", page.status);
        let mut other = Ability::default();
        other.id = 192;
        frame(&mut other, &mut session);
        frame(&mut other, &mut session);
        let rows = other.menu_rows(&session);
        assert_eq!(rows[192].name, "Quick Start");
        assert_eq!(rows.len(), 193);
        other.delete_ability(&session);
        assert!(other.store.find(192).is_none() && session.sd.as_ref().unwrap().read("ability/192.bin").is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }
}

#[cfg(test)]
mod game_rules_test {
    use super::*;

    fn draw(page: &mut Ability, session: &Session) {
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.game_reactions_tab(ui, session));
            });
        }
    }

    /// The reaction of the game shows as rules and changes nothing; the first change puts the rules in the list to save.
    #[test]
    fn a_game_reaction_goes_to_the_store_at_the_first_change() {
        let session = Session::default();
        let mut page = Ability { id: 22, ..Ability::default() };
        draw(&mut page, &session);
        assert!(!page.store.changed(), "drawing the reaction of the game changes nothing");
        assert_eq!(page.draft.as_ref().map(|d| d.rules.clone()), rules::presets::game_rules(22));
        page.draft.as_mut().unwrap().rules[0].actions[0].b = -2;
        draw(&mut page, &session);
        assert!(page.store.changed() && page.draft.is_none());
        assert_eq!(page.store.game_ids(), vec![22]);
        assert!(page.store.ids().is_empty(), "an ability of the game is not a new ability");
        // An ability without rules of the game (Sturdy) shows no rules.
        page.id = 5;
        draw(&mut page, &session);
        assert!(page.draft.is_none());
    }
}
