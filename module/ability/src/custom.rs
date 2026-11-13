//! The tab "New abilities": the user makes an ability, writes its texts in the eight languages and builds its reactions with
//! menus. The page writes one file for each ability on the SD card (see [`crate::rules`]).

use crate::rules::{self, Action, ActionKind, Argument, Condition, ConditionKind, CustomAbility, Rule};
use sango_core::text::LANGUAGES;
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The folder of the files on the SD card.
const FOLDER: &str = "ability";

/// The state of the tab.
#[derive(Default)]
pub struct Custom {
    /// The abilities, as the user has them now.
    list: Vec<CustomAbility>,
    /// The abilities as they are on the SD card: to know what changed.
    saved: Vec<CustomAbility>,
    loaded: bool,
    /// The SD card folder that `list` was read from.
    root: Option<std::path::PathBuf>,
    /// The place in `list` of the ability that is open.
    selected: Option<usize>,
    /// The language of the text boxes.
    language: usize,
    new_name: String,
    new_description: String,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// True after a first click on Delete: the second click deletes.
    confirm_delete: bool,
}

impl Custom {
    /// Reads the files of the SD card again.
    pub fn reload(&mut self, sd: Option<&SdCard>) {
        self.list.clear();
        self.selected = None;
        self.confirm_delete = false;
        if let Some(sd) = sd {
            for path in sd.files_in(FOLDER) {
                let Some(bytes) = sd.read(&path) else { continue };
                match CustomAbility::from_bytes(&bytes) {
                    Ok(ability) => self.list.push(ability),
                    Err(message) => self.status = Some((false, format!("{path}: {message}"))),
                }
            }
        }
        self.list.sort_by_key(|ability| ability.id);
        self.saved = self.list.clone();
        self.selected = if self.list.is_empty() { None } else { Some(0) };
        self.loaded = true;
    }

    /// True when an ability changed since it was read or saved.
    pub fn changed(&self) -> bool {
        self.list != self.saved
    }

    /// Makes an ability with the texts of the boxes. Gives an error text when it is not possible.
    fn add(&mut self) -> Result<(), String> {
        if self.list.len() >= rules::MAX_ABILITIES {
            return Err(format!("The plugin keeps {} new abilities at most: delete one first.", rules::MAX_ABILITIES));
        }
        let used: Vec<u8> = self.list.iter().map(|a| a.id).collect();
        let id = CustomAbility::free_id(&used).ok_or("All the ids from 192 to 255 are used.")?;
        if self.new_name.trim().is_empty() {
            return Err("Give a name.".into());
        }
        self.list.push(CustomAbility::new(id, self.new_name.trim(), &self.new_description));
        self.list.sort_by_key(|ability| ability.id);
        self.selected = self.list.iter().position(|a| a.id == id);
        self.new_name.clear();
        self.new_description.clear();
        Ok(())
    }

    /// Writes the abilities that changed, and tells the plugin.
    fn save(&mut self, sd: &SdCard) {
        let mut written = 0;
        for ability in &self.list {
            if self.saved.iter().any(|saved| saved == ability) {
                continue;
            }
            if let Some(first) = ability.problems().first() {
                self.status = Some((false, format!("#{} {}: {first}", ability.id, ability.names[rules::ENGLISH])));
                return;
            }
            let path = CustomAbility::path(ability.id);
            if let Err(message) = sd.write(&path, &ability.to_bytes()).and_then(|_| sd.mark_changed(&path)) {
                self.status = Some((false, message));
                return;
            }
            written += 1;
        }
        self.saved = self.list.clone();
        self.status = Some((true, format!("Saved {written} file(s) on the SD card.")));
    }

    /// Deletes the open ability: its file too.
    fn delete(&mut self, sd: &SdCard) {
        let Some(index) = self.selected.filter(|i| *i < self.list.len()) else { return };
        let id = self.list[index].id;
        let path = CustomAbility::path(id);
        match sd.remove(&path).and_then(|_| sd.mark_changed(&path)) {
            Ok(_) => {
                self.list.remove(index);
                self.saved.retain(|a| a.id != id);
                self.selected = if self.list.is_empty() { None } else { Some(index.min(self.list.len() - 1)) };
                self.status = Some((true, format!("The ability {id} was deleted.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
        self.confirm_delete = false;
    }

    /// Draws the tab.
    pub fn ui(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let Some(sd) = sd else {
            widget::missing_card(ui, "Choose the SD card folder in Home: the new abilities are files on the SD card.");
            return;
        };
        if !self.loaded || self.root.as_deref() != Some(sd.root()) {
            self.root = Some(sd.root().to_path_buf());
            self.reload(Some(sd));
        }
        egui::ScrollArea::vertical().id_salt("ability_custom").auto_shrink([false, false]).show(ui, |ui| {
            self.list_card(ui, sd);
            ui.add_space(10.0);
            if let Some(index) = self.selected.filter(|i| *i < self.list.len()) {
                self.texts_card(ui, index);
                ui.add_space(10.0);
                self.rules_card(ui, index);
                ui.add_space(10.0);
                self.preview_card(ui, index);
                ui.add_space(10.0);
            }
        });
    }

    /// The list of the abilities, the form of a new ability, and the buttons that save and delete.
    fn list_card(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            ui.label(widget::title("New abilities"));
            ui.label(widget::hint("An ability here has an id from 192 to 255. Give a name and a description in English: you change them in every language below, then you build what the ability does."));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, ability) in self.list.iter().enumerate() {
                    let label = format!("#{} {}", ability.id, ability.names[rules::ENGLISH]);
                    if widget::tab_button(ui, &label, self.selected == Some(i), false).clicked() {
                        self.selected = Some(i);
                        self.confirm_delete = false;
                    }
                }
                if self.list.is_empty() {
                    ui.label(widget::hint("No new ability yet."));
                }
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Make a new ability").strong());
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(240.0).hint_text("Quick Start"));
            });
            ui.horizontal(|ui| {
                ui.label("Description");
                ui.add(egui::TextEdit::multiline(&mut self.new_description).desired_rows(2).desired_width(420.0).hint_text("Raises the Speed of the Pokémon\nwhen it enters a battle."));
            });
            let mut add = false;
            let mut save = false;
            let mut delete = false;
            ui.horizontal_wrapped(|ui| {
                add = widget::button_styled(ui, egui::RichText::new("Make the ability").color(color::WHITE), Some(color::ACCENT));
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    let label = if self.confirm_delete { "Really delete?" } else { "Delete" };
                    delete = widget::button(ui, label);
                });
                save = widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
                if self.changed() {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else {
                    widget::badge(ui, "Saved", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
            if add {
                self.status = self.add().err().map(|message| (false, message));
            }
            if delete {
                if self.confirm_delete {
                    self.delete(sd);
                } else {
                    self.confirm_delete = true;
                }
            }
            if save {
                self.save(sd);
            }
        });
    }

    /// The name and the description, in the eight languages.
    fn texts_card(&mut self, ui: &mut egui::Ui, index: usize) {
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            ui.label(widget::title("Name and description"));
            ui.label(widget::hint("A new ability starts with the English text in every language. The game shows the text of the language of the console."));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, language) in LANGUAGES.iter().enumerate() {
                    if widget::tab_button(ui, language.name, self.language == i, false).clicked() {
                        self.language = i;
                    }
                }
            });
            ui.add_space(6.0);
            let ability = &mut self.list[index];
            let language = self.language.min(7);
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut ability.names[language]).desired_width(300.0));
            });
            ui.label("Description");
            ui.add(egui::TextEdit::multiline(&mut ability.descriptions[language]).desired_rows(3).desired_width(520.0));
        });
    }

    /// The reactions: the rules, with menus.
    fn rules_card(&mut self, ui: &mut egui::Ui, index: usize) {
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            ui.label(widget::title("What the ability does"));
            ui.label(widget::hint("Each reaction says: when this happens, if all these are true, do these things. An ability can have several reactions, for different moments."));
            let ability = &mut self.list[index];
            let mut remove_rule = None;
            for (number, rule) in ability.rules.iter_mut().enumerate() {
                ui.add_space(8.0);
                egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(10.0)).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("Reaction {}", number + 1)).strong());
                        if widget::button(ui, "Remove the reaction") {
                            remove_rule = Some(number);
                        }
                    });
                    rule_ui(ui, index, number, rule);
                });
            }
            if let Some(number) = remove_rule {
                ability.rules.remove(number);
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(ability.rules.len() < rules::MAX_RULES, |ui| {
                if widget::button(ui, "Add a reaction") {
                    ability.rules.push(Rule::new());
                }
            });
        });
    }

    /// The rules as sentences, and what is wrong.
    fn preview_card(&mut self, ui: &mut egui::Ui, index: usize) {
        let ability = &self.list[index];
        widget::card(ui, |ui| {
            ui.set_min_width(700.0);
            ui.label(widget::title("Summary"));
            for sentence in ability.preview() {
                ui.label(sentence);
            }
            let problems = ability.problems();
            if !problems.is_empty() {
                ui.add_space(6.0);
                for problem in problems {
                    ui.label(egui::RichText::new(problem).color(color::ROSE).size(13.5));
                }
            }
        });
    }
}

/// A menu of a list of values. Gives true when the value changed.
fn pick(ui: &mut egui::Ui, salt: impl std::hash::Hash, value: &mut i16, list: &[(i16, &str)]) -> bool {
    let before = *value;
    let shown = list.iter().find(|(v, _)| v == value).map(|(_, name)| *name).unwrap_or("?");
    egui::ComboBox::from_id_salt(salt).selected_text(shown).show_ui(ui, |ui| {
        for (v, name) in list {
            ui.selectable_value(value, *v, *name);
        }
    });
    *value != before
}

/// The editor of an argument: a menu or a number box.
fn argument_ui(ui: &mut egui::Ui, salt: String, argument: Argument, value: &mut i16) {
    match argument {
        Argument::None => {}
        Argument::Stat => {
            pick(ui, salt, value, &rules::STATS);
        }
        Argument::Weather => {
            pick(ui, salt, value, &rules::WEATHERS);
        }
        Argument::Terrain => {
            pick(ui, salt, value, &rules::TERRAINS);
        }
        Argument::Status => {
            pick(ui, salt, value, &rules::STATUSES);
        }
        Argument::Type => {
            let types: Vec<(i16, &str)> = sango_species::TYPE_NAMES.iter().enumerate().map(|(i, name)| (i as i16, *name)).collect();
            pick(ui, salt, value, &types);
        }
        Argument::Percent => {
            ui.add(egui::DragValue::new(value).range(1..=100).suffix("% of the maximum HP"));
        }
        Argument::Move => {
            ui.add(egui::DragValue::new(value).range(1..=700).prefix("move number "));
        }
        Argument::Stages => {
            ui.add(egui::DragValue::new(value).range(-6..=6).suffix(" stages"));
        }
        Argument::Forever => {
            let mut forever = *value != 0;
            ui.checkbox(&mut forever, "never stops");
            *value = forever as i16;
        }
    }
}

/// The moment, the conditions and the actions of a rule.
fn rule_ui(ui: &mut egui::Ui, ability: usize, number: usize, rule: &mut Rule) {
    let salt = |part: &str, i: usize| format!("ability_{ability}_{number}_{part}_{i}");
    ui.horizontal(|ui| {
        ui.label("When");
        egui::ComboBox::from_id_salt(salt("moment", 0)).width(280.0).selected_text(rules::moment_label(rule.moment)).show_ui(ui, |ui| {
            for moment in &rules::MOMENTS {
                ui.selectable_value(&mut rule.moment, moment.value, moment.label).on_hover_text(moment.help);
            }
        });
    });
    if let Some(moment) = rules::MOMENTS.iter().find(|m| m.value == rule.moment) {
        ui.label(widget::hint(moment.help));
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("If all of this is true").strong());
    let mut remove = None;
    for (i, condition) in rule.conditions.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let mut kind = condition.kind;
            egui::ComboBox::from_id_salt(salt("condition", i)).width(260.0).selected_text(kind.label()).show_ui(ui, |ui| {
                for option in ConditionKind::ALL {
                    ui.selectable_value(&mut kind, option, option.label()).on_hover_text(option.help());
                }
            });
            if kind != condition.kind {
                *condition = Condition { kind, arg: if kind == ConditionKind::MoveTypeIs { 0 } else if kind == ConditionKind::WeatherIs { 2 } else { 0 } };
            }
            argument_ui(ui, salt("condition_arg", i), condition.kind.argument(), &mut condition.arg);
            if widget::button(ui, "Remove") {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        rule.conditions.remove(i);
    }
    if rule.conditions.is_empty() {
        ui.label(widget::hint("No condition: the reaction runs every time."));
    }
    ui.add_enabled_ui(rule.conditions.len() < rules::MAX_CONDITIONS, |ui| {
        if widget::button(ui, "Add a condition") {
            rule.conditions.push(Condition { kind: ConditionKind::PokemonIsOwner, arg: 0 });
        }
    });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Then").strong());
    let mut remove = None;
    for (i, action) in rule.actions.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            let mut kind = action.kind;
            egui::ComboBox::from_id_salt(salt("action", i)).width(200.0).selected_text(kind.label()).show_ui(ui, |ui| {
                for option in ActionKind::ALL {
                    ui.selectable_value(&mut kind, option, option.label()).on_hover_text(option.help());
                }
            });
            if kind != action.kind {
                *action = Action { target: action.target, banner: action.banner, ..Action::new(kind) };
            }
            if action.kind.has_target() {
                ui.label("for");
                egui::ComboBox::from_id_salt(salt("target", i)).width(230.0).selected_text(rules::target_label(action.target)).show_ui(ui, |ui| {
                    for (value, name, help) in rules::TARGETS {
                        ui.selectable_value(&mut action.target, value, name).on_hover_text(help);
                    }
                });
            }
            argument_ui(ui, salt("first", i), action.kind.first(), &mut action.a);
            argument_ui(ui, salt("second", i), action.kind.second(), &mut action.b);
            ui.checkbox(&mut action.banner, "show the banner");
            if widget::button(ui, "Remove") {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        rule.actions.remove(i);
    }
    ui.add_enabled_ui(rule.actions.len() < rules::MAX_ACTIONS, |ui| {
        if widget::button(ui, "Add an action") {
            rule.actions.push(Action::new(ActionKind::ChangeStat));
        }
    });
    ui.add_space(4.0);
    ui.label(widget::hint(&rules::rule_sentence(rule)));
}

#[cfg(test)]
mod test {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sango_custom_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn making_saving_and_reading_back_an_ability() {
        let path = folder("save");
        let sd = SdCard::open(path.to_str().unwrap()).unwrap();
        let mut page = Custom::default();
        page.reload(Some(&sd));
        assert!(page.list.is_empty());
        assert!(page.add().is_err(), "an empty name is refused");
        page.new_name = "Quick Start".into();
        page.new_description = "Raises the Speed.".into();
        page.add().unwrap();
        assert_eq!(page.list[0].id, 192);
        assert!(page.changed());
        page.save(&sd);
        assert!(matches!(page.status, Some((true, _))), "{:?}", page.status);
        assert!(!page.changed());
        assert!(sd.read("ability/192.bin").is_some());

        let mut other = Custom::default();
        other.reload(Some(&sd));
        assert_eq!(other.list, page.list);

        other.delete(&sd);
        assert!(other.list.is_empty());
        assert!(sd.read("ability/192.bin").is_none());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn a_second_ability_gets_the_next_id() {
        let mut page = Custom::default();
        for name in ["One", "Two"] {
            page.new_name = name.into();
            page.add().unwrap();
        }
        assert_eq!(page.list.iter().map(|a| a.id).collect::<Vec<_>>(), vec![192, 193]);
        assert_eq!(page.selected, Some(1));
    }

    #[test]
    fn the_tab_draws_with_a_rule_of_every_kind() {
        let path = folder("draw");
        let sd = SdCard::open(path.to_str().unwrap()).unwrap();
        let mut page = Custom::default();
        page.reload(Some(&sd));
        page.root = Some(sd.root().to_path_buf());
        page.new_name = "Test".into();
        page.add().unwrap();
        for moment in rules::MOMENTS.iter() {
            page.list[0].rules.push(Rule { moment: moment.value, conditions: ConditionKind::ALL.iter().map(|&kind| Condition { kind, arg: 0 }).collect(), actions: ActionKind::ALL.iter().map(|&kind| Action::new(kind)).collect() });
        }
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, Some(&sd)));
            });
        }
        let _ = std::fs::remove_dir_all(&path);
    }
}
