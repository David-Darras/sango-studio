//! The editor of the rules: the menus that build the reactions of an ability, of a move or of an item. It is used by the pages of the
//! three of them.

use crate::{moments, rule_sentence, ActionKind, Argument, Condition, ConditionKind, Context, Rule, Action};
use crate::{moment_label, target_label, MAX_ACTIONS, MAX_CONDITIONS, MAX_RULES, STATS, STATUSES, TARGETS, TERRAINS, WEATHERS};
use sango_species::battle_icons;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

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

/// The types that a menu lists: the types of the game, then the new types of the user.
fn type_list() -> Vec<(i16, String)> {
    let mut list: Vec<(i16, String)> = sango_species::TYPE_NAMES.iter().enumerate().map(|(i, name)| (i as i16, name.to_string())).collect();
    for id in battle_icons::custom_type_ids() {
        list.push((id as i16, battle_icons::custom_type_name(id).unwrap_or_default()));
    }
    list
}

/// The editor of an argument: a menu or a number box.
fn argument_ui(ui: &mut egui::Ui, salt: String, argument: Argument, value: &mut i16) {
    match argument {
        Argument::None => {}
        Argument::Stat => {
            pick(ui, salt, value, &STATS);
        }
        Argument::Weather => {
            pick(ui, salt, value, &WEATHERS);
        }
        Argument::Terrain => {
            pick(ui, salt, value, &TERRAINS);
        }
        Argument::Status => {
            pick(ui, salt, value, &STATUSES);
        }
        Argument::Type => {
            let types = type_list();
            let list: Vec<(i16, &str)> = types.iter().map(|(id, name)| (*id, name.as_str())).collect();
            pick(ui, salt, value, &list);
        }
        Argument::Percent => {
            ui.add(egui::DragValue::new(value).range(1..=100).suffix("% of the maximum HP"));
        }
        Argument::Move => {
            ui.add(egui::DragValue::new(value).range(1..=1000).prefix("move number "));
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
fn rule_ui(ui: &mut egui::Ui, id: &str, number: usize, rule: &mut Rule, context: Context) {
    let salt = |part: &str, i: usize| format!("{id}_{number}_{part}_{i}");
    ui.horizontal(|ui| {
        ui.label("When");
        egui::ComboBox::from_id_salt(salt("moment", 0)).width(280.0).selected_text(moment_label(rule.moment)).show_ui(ui, |ui| {
            for moment in moments(context) {
                ui.selectable_value(&mut rule.moment, moment.value, moment.label).on_hover_text(moment.help);
            }
        });
    });
    if let Some(moment) = moments(context).iter().find(|m| m.value == rule.moment) {
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
                let arg = match kind {
                    ConditionKind::WeatherIs => 2,
                    ConditionKind::HpBelow => 50,
                    _ => 0,
                };
                *condition = Condition { kind, arg };
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
    ui.add_enabled_ui(rule.conditions.len() < MAX_CONDITIONS, |ui| {
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
                egui::ComboBox::from_id_salt(salt("target", i)).width(230.0).selected_text(target_label(action.target)).show_ui(ui, |ui| {
                    for (value, name, help) in TARGETS {
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
    ui.add_enabled_ui(rule.actions.len() < MAX_ACTIONS, |ui| {
        if widget::button(ui, "Add an action") {
            rule.actions.push(Action::new(ActionKind::ChangeStat));
        }
    });
    ui.add_space(4.0);
    ui.label(widget::hint(&rule_sentence(rule)));
}

/// The reactions: the rules, with menus. `id` tells this editor from the others on the page.
pub fn rules_editor(ui: &mut egui::Ui, id: &str, rules: &mut Vec<Rule>, context: Context) {
    let mut remove_rule = None;
    for (number, rule) in rules.iter_mut().enumerate() {
        ui.add_space(8.0);
        egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(10.0)).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Reaction {}", number + 1)).strong());
                if widget::button(ui, "Remove the reaction") {
                    remove_rule = Some(number);
                }
            });
            rule_ui(ui, id, number, rule, context);
        });
    }
    if let Some(number) = remove_rule {
        rules.remove(number);
    }
    ui.add_space(8.0);
    ui.add_enabled_ui(rules.len() < MAX_RULES, |ui| {
        if widget::button(ui, "Add a reaction") {
            let mut rule = Rule::new();
            rule.moment = moments(context)[0].value;
            if context == Context::Move {
                rule.conditions.clear();
                rule.actions[0].target = 3;
            }
            rules.push(rule);
        }
    });
}

