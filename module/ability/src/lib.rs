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

use sango_core::{Dump, SdCard};
use sango_species::SpeciesTable;
use sango_species_base::SpeciesAbilities;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of abilities (the ability 0 is no ability).
pub const COUNT: usize = 191;

/// The tabs of the page.
const TABS: [&str; 4] = ["General", "Reactions", "Text", "CSV"];

/// The place of the tab "CSV" in [`TABS`]: the import and the export of all the entries.
const CSV_TAB: usize = 3;

/// The place of the tab "Reactions" in [`TABS`]: the rules of a new ability, or of an ability of the game.
const REACTIONS_TAB: usize = 1;

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
        Ability { root: None, id: 1, tab: 0, status: None, list_names: None, modified: Default::default(), usage: Vec::new(), usage_ready: false, species_table: SpeciesTable::default(), texts: texts::AbilityTexts::default(), tools: tools::Tools::default(), store: custom::Store::default(), custom_language: 2, draft: None }
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
        egui::ScrollArea::vertical().id_salt("ability_game_reactions").auto_shrink([false, false]).show(ui, |ui| {
            if let Some(entry) = self.store.find_mut(id) {
                widget::card(ui, |ui| {
                    ui.set_min_width(700.0);
                    ui.label(widget::title("Your rules replace the code of the game"));
                    ui.label(widget::hint("The file ability/<number>.bin is on the SD card (or is written at the next \"Save\"): the plugin runs these rules, and not the code of the game, for this ability. Omega Ruby and Alpha Sapphire only."));
                    ui.add_space(4.0);
                    back_to_game = session.sd.is_some() && widget::button(ui, "Back to the code of the game");
                });
                ui.add_space(10.0);
                custom::reactions_ui(ui, entry);
                return;
            }
            let preset = rules::presets::game_rules(id as u8);
            if self.draft.as_ref().map(|d| d.id as usize) != Some(id) {
                self.draft = preset.clone().map(|rules| rules::CustomAbility::game(id as u8, rules));
            }
            widget::card(ui, |ui| {
                ui.set_min_width(700.0);
                ui.label(widget::title("The game runs its own code"));
                if preset.is_some() {
                    ui.label(widget::hint("Below is what the code of the game does, written as rules. Change a value: at the next \"Save\", the Studio writes the file ability/<number>.bin and the plugin runs the rules in place of the code of the game. Some values are rounded (1/8 of the HP is 12%)."));
                } else {
                    ui.label(widget::hint("The rules cannot do what the code of the game does for this ability (see docs/ability-reactions.md). You can replace the code with your own rules: the ability then loses its effect of the game."));
                    ui.add_space(4.0);
                    own_rules = session.sd.is_some() && widget::button(ui, "Replace with my own rules");
                }
            });
            if let Some(draft) = &mut self.draft {
                ui.add_space(10.0);
                custom::reactions_ui(ui, draft);
            }
        });
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
        let mut reset_tab = false;
        let mut reset_more = false;
        let mut undo = false;
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
                .on_hover_text("Adds an ability. Then you change its name in the Text tab and its reactions in the Reactions tab.");
                ui.add_enabled_ui(has_card && self.is_custom(), |ui| {
                    delete = widget::button(ui, "Delete");
                })
                .response
                .on_hover_text("Deletes this new ability.");
                ui.add_enabled_ui(has_card && !self.is_custom(), |ui| {
                    reset_tab = widget::button(ui, "Reset texts");
                })
                .response
                .on_hover_text("Puts the name and the description of this ability back to the romfs. Undo gives them back.");
                ui.add_enabled_ui(has_card && !self.is_custom(), |ui| {
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

    /// The General tab: the name and the description, what an ability is and who has it.
    fn general_tab(&mut self, ui: &mut egui::Ui, session: &Session) {
        let sd = session.sd.as_ref();
        let (name, description) = match self.store.find(self.id).filter(|entry| !entry.is_game()) {
            Some(entry) => (entry.names[rules::ENGLISH].clone(), entry.descriptions[rules::ENGLISH].clone()),
            None => (self.texts.text(sd, texts::ENGLISH, 0, self.id), self.texts.text(sd, texts::ENGLISH, 1, self.id)),
        };
        egui::ScrollArea::vertical().id_salt("ability_general").auto_shrink([false, false]).show(ui, |ui| {
            widget::card(ui, |ui| {
                ui.set_min_width(700.0);
                ui.label(widget::title(&format!("#{:03} {name}", self.id)));
                ui.label(&description);
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(widget::hint("An ability has no data in a file: the game runs its effect from its code, so this page edits its texts and shows who has it."));
                    ui.hyperlink_to("Bulbapedia ↗", "https://bulbapedia.bulbagarden.net/wiki/Ability");
                });
            });
            ui.add_space(10.0);
            self.users_card(ui, session);
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
                let mut numbers: Vec<usize> = users.get(slot).map(|list| list.iter().map(|&(species, _)| species).collect()).unwrap_or_default();
                numbers.sort_unstable();
                numbers.dedup();
                ui.label(egui::RichText::new(format!("{title} ({})", numbers.len())).strong());
                if numbers.is_empty() {
                    ui.label(widget::hint("No species."));
                } else {
                    // Each slot has its own list: its widgets (the header, the scroll area) need their own ids.
                    let clicked = ui.push_id(slot, |ui| self.species_table.show(ui, "ability_users", &numbers, 300.0)).inner;
                    if let Some(number) = clicked {
                        sango_species_base::open_species(ui.ctx(), number);
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
            CSV_TAB => {
                egui::ScrollArea::vertical().id_salt("ability_csv_tab").auto_shrink([false, false]).show(ui, |ui| self.csv_card(ui, session));
            }
            REACTIONS_TAB => {
                if self.is_custom() {
                    match self.store.find_mut(self.id) {
                        Some(entry) => egui::ScrollArea::vertical().id_salt("ability_reactions").auto_shrink([false, false]).show(ui, |ui| custom::reactions_ui(ui, entry)).inner,
                        None => widget::card(ui, |ui| {
                            ui.label(widget::hint("This ability has no file."));
                        }),
                    }
                } else {
                    self.game_reactions_tab(ui, session);
                }
            }
            _ => {
                if let Some(entry) = self.store.find_mut(self.id).filter(|entry| !entry.is_game()) {
                    let mut language = self.custom_language;
                    egui::ScrollArea::vertical().id_salt("ability_custom_texts").auto_shrink([false, false]).show(ui, |ui| custom::texts_ui(ui, entry, &mut language));
                    self.custom_language = language;
                } else {
                    let sd: Option<&SdCard> = session.sd.as_ref();
                    self.texts.ui(ui, sd, self.id);
                }
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
        for tab in [0, 1, 2] {
            page.tab = tab;
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
