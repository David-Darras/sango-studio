//! A menu with a search box and titled groups: the things have names, and the user never sees the numbers.
//!
//! The arrows move the choice (with the list closed or open), a click gives the keyboard focus to the menu, and Enter closes the list.

use crate::focus;
use crate::theme::color;
use crate::{egui, widget};

/// The name without capitals, accents and signs: "Mr. Mime" and "mrmime" are the same name.
pub fn simple(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// A group of entries of a menu: a title and the things with their names.
pub struct Group {
    pub title: String,
    pub entries: Vec<(u16, String)>,
}

/// A menu with a search box and titled groups. `build` makes the groups when the list opens. Gives true when the value changed.
pub fn picker(ui: &mut egui::Ui, id: &str, shown: &str, width: f32, value: &mut u16, none: Option<&str>, build: impl Fn() -> Vec<Group>) -> bool {
    let picker_id = ui.make_persistent_id(id);
    let popup_id = picker_id.with("popup");
    let open = ui.memory(|m| m.is_popup_open(popup_id));
    let height = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let button = ui.interact(rect, picker_id, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = if open { ui.visuals().widgets.open } else { *ui.style().interact(&button) };
        ui.painter().rect(rect.expand(visuals.expansion), visuals.rounding, visuals.weak_bg_fill, visuals.bg_stroke);
        let galley = egui::WidgetText::from(shown).into_galley(ui, Some(egui::TextWrapMode::Truncate), width - 26.0, egui::TextStyle::Button);
        let text_rect = rect.shrink2(ui.spacing().button_padding);
        let at = egui::Align2::LEFT_CENTER.align_size_within_rect(galley.size(), text_rect).min;
        ui.painter().galley(at, galley, visuals.text_color());
        let arrow = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(7.0, 4.0));
        ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], visuals.fg_stroke.color, egui::Stroke::NONE));
    }
    if button.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let mut changed = false;
    // The menu is the active one after a click: with the list closed, the arrows go to the next or the previous entry.
    if focus::focus(ui, picker_id, rect, button.clicked(), true, Some(&button)) && !open {
        let step = focus::steps(ui, false);
        if step != 0 {
            let flat = flat_entries(&build(), none.is_some());
            if let Some(to) = focus::stepped(flat.iter().position(|&n| n == *value).unwrap_or(0), step, flat.len()) {
                *value = flat[to];
                changed = true;
            }
        }
    }
    let search_id = picker_id.with("search");
    let focus_id = picker_id.with("focus");
    let mut query: String = ui.data_mut(|d| d.get_temp(search_id).unwrap_or_default());
    let was_open = open;
    egui::popup::popup_below_widget(ui, popup_id, &button, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        let list_width = width.max(300.0);
        ui.set_width(list_width);
        ui.horizontal(|ui| {
            ui.label(widget::hint("🔍"));
            let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text("Search").desired_width(list_width - 50.0));
            if !ui.data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false)) {
                edit.request_focus();
                ui.data_mut(|d| d.insert_temp(focus_id, true));
            }
        });
        ui.separator();
        let needle = simple(&query);
        let groups = build();
        // With the list open, the arrows change the choice and Enter closes the list.
        let visible: Vec<Group> = groups.iter().map(|g| Group { title: String::new(), entries: g.entries.iter().filter(|(_, label)| needle.is_empty() || simple(label).contains(&needle)).cloned().collect() }).collect();
        let flat = flat_entries(&visible, none.is_some() && needle.is_empty());
        if let Some(to) = focus::stepped(flat.iter().position(|&n| n == *value).unwrap_or(0), focus::steps(ui, false), flat.len()) {
            *value = flat[to];
            changed = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            ui.memory_mut(|m| m.close_popup());
        }
        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
            if let Some(none) = none {
                if needle.is_empty() {
                    let row = ui.selectable_label(*value == 0, none);
                    widget::scroll_to_chosen(ui, &row, *value == 0);
                    if row.clicked() {
                        *value = 0;
                        changed = true;
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            }
            for group in groups {
                let shown: Vec<&(u16, String)> = group.entries.iter().filter(|(_, label)| needle.is_empty() || simple(label).contains(&needle)).collect();
                if shown.is_empty() {
                    continue;
                }
                if !group.title.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&group.title).strong().color(color::ACCENT).size(12.5));
                }
                for (number, label) in shown {
                    let row = ui.selectable_label(*value == *number, label);
                    widget::scroll_to_chosen(ui, &row, *value == *number);
                    if row.clicked() {
                        *value = *number;
                        changed = true;
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            }
        });
    });
    let open_now = ui.memory(|m| m.is_popup_open(popup_id));
    if was_open && !open_now || changed {
        ui.data_mut(|d| {
            d.remove::<bool>(focus_id);
            d.remove::<String>(search_id);
        });
    } else {
        ui.data_mut(|d| d.insert_temp(search_id, query));
    }
    changed
}

/// The numbers of the entries of some groups, in the order of the menu (0 first when the menu has "none").
fn flat_entries(groups: &[Group], with_none: bool) -> Vec<u16> {
    let mut flat: Vec<u16> = if with_none { vec![0] } else { Vec::new() };
    flat.extend(groups.iter().flat_map(|g| g.entries.iter().map(|(n, _)| *n)));
    flat
}

