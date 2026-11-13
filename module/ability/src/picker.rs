//! A drop-down list of abilities: the number, the name and the number of species that have the ability.
//!
//! The list has a search box, a header that sorts the list when it is clicked, the arrow keys, Enter, and it shows the chosen
//! ability at the middle when it opens.

use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// What the list shows for one ability. The rows are in the order of the numbers, the row 0 is for "no ability".
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub name: String,
    /// The number of species and forms that have the ability.
    pub species: usize,
}

/// A column of the list. A click on its header sorts the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortColumn {
    Number,
    Name,
    Species,
}

/// The columns of the list: x position, width, name, and the sort of a click on the header.
const COLUMNS: [(f32, f32, &str, SortColumn); 3] = [(8.0, 50.0, "#", SortColumn::Number), (60.0, 230.0, "Name", SortColumn::Name), (300.0, 100.0, "Species", SortColumn::Species)];

const LIST_WIDTH: f32 = 420.0;
const ROW_HEIGHT: f32 = 28.0;

/// What the list keeps between two frames: the sort, the search and whether the list was already shown.
#[derive(Clone, Default)]
struct State {
    sort: Option<(SortColumn, bool)>,
    search: String,
    searched: bool,
    shown: bool,
}

/// A drop-down list of abilities. `value` is the ability number. The button is `width` points wide. Gives true when the
/// ability changed.
pub fn ability_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut usize, width: f32, rows: &[Row]) -> bool {
    let before = *value;
    let last = rows.len().saturating_sub(1);
    let id = ui.make_persistent_id(id_salt);
    let popup_id = id.with("popup");
    let state_id = id.with("state");
    let mut state: State = ui.data_mut(|d| d.get_temp(state_id).unwrap_or_default());
    let open = ui.memory(|m| m.is_popup_open(popup_id));

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
    let button = ui.interact(rect, id, egui::Sense::click());
    let visuals = if open { ui.visuals().widgets.open } else { *ui.style().interact(&button) };
    ui.painter().rect(rect.expand(visuals.expansion), visuals.rounding, visuals.weak_bg_fill, visuals.bg_stroke);
    let name = rows.get(*value).map(|r| r.name.as_str()).unwrap_or("Unknown");
    let galley = egui::WidgetText::from(format!("#{:03} {name}", *value)).into_galley(ui, Some(egui::TextWrapMode::Truncate), (width - 40.0).max(20.0), egui::TextStyle::Button);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(egui::pos2(rect.left() + 10.0, top), galley, visuals.text_color());
    let arrow = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), egui::vec2(8.0, 5.0));
    ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], visuals.fg_stroke.color, egui::Stroke::NONE));
    if button.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        widget::set_active_id(ui, id);
    }
    widget::register_focusable(ui, id, button.rect);

    // The button is focused: Enter or Space open the list, Up and Down change the ability.
    if widget::is_active(ui, id) && widget::page_has_focus(ui) && !open {
        widget::glow(ui, button.rect, 6.0);
        widget::scroll_active_into_view(ui, &button);
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            ui.memory_mut(|m| m.open_popup(popup_id));
        }
        ui.input_mut(|i| {
            let step = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize;
            if step != 0 {
                *value = (*value as isize + step).clamp(1, last as isize) as usize;
            }
        });
    }

    let shown = egui::popup::popup_below_widget(ui, popup_id, &button, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(LIST_WIDTH);
        ui.set_max_width(LIST_WIDTH);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(widget::hint("🔍"));
            let input = ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search name or number...").desired_width(280.0).clip_text(true));
            if !state.searched {
                input.request_focus();
                state.searched = true;
            }
            if !state.search.is_empty() && ui.small_button("✕").clicked() {
                state.search.clear();
            }
        });
        ui.separator();
        let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
        let query = clean(state.search.trim());
        let number_query = query.trim_start_matches('#').trim_start_matches('0').to_string();
        let mut matches: Vec<usize> = (1..=last).filter(|&n| query.is_empty() || clean(&rows[n].name).contains(&query) || (!number_query.is_empty() && n.to_string() == number_query)).collect();

        header(ui, &mut state);
        ui.separator();
        let sort = state.sort;
        matches.sort_by(|&a, &b| {
            let order = match sort {
                None | Some((SortColumn::Number, _)) => a.cmp(&b),
                Some((SortColumn::Name, _)) => rows[a].name.cmp(&rows[b].name),
                Some((SortColumn::Species, _)) => rows[a].species.cmp(&rows[b].species),
            };
            let order = if sort.is_some_and(|(_, descending)| descending) { order.reverse() } else { order };
            order.then(a.cmp(&b))
        });
        if matches.is_empty() {
            ui.add_space(4.0);
            ui.label(widget::hint("No matching ability."));
            return;
        }
        let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if let Some(place) = matches.iter().position(|&n| n == *value) {
            if down && place + 1 < matches.len() {
                *value = matches[place + 1];
            }
            if up && place > 0 {
                *value = matches[place - 1];
            }
        } else if up || down {
            *value = matches[0];
        }
        if enter {
            if !matches.contains(value) {
                *value = matches[0];
            }
            ui.memory_mut(|m| m.close_popup());
        }
        let highlighted = if matches.contains(value) { *value } else { matches[0] };
        let place = matches.iter().position(|&n| n == highlighted);
        // The list shows the highlighted ability at the middle after a key, and at the opening.
        let centre = place.filter(|_| up || down || (query.is_empty() && !state.shown));
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().id_salt(id.with("list")).max_height(380.0);
        if let Some(place) = centre {
            area = area.vertical_scroll_offset(((place as f32 + 0.5) * ROW_HEIGHT - 190.0).max(0.0));
        }
        area.show_rows(ui, ROW_HEIGHT, matches.len(), |ui, range| {
            for place in range {
                let number = matches[place];
                let (row, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, ROW_HEIGHT), egui::Sense::click());
                if !ui.is_rect_visible(row) {
                    continue;
                }
                if number == highlighted {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::ACCENT_SOFT);
                    widget::glow(ui, row, 6.0);
                } else if response.hovered() {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let y = row.center().y;
                ui.painter().text(egui::pos2(row.left() + COLUMNS[0].0, y), egui::Align2::LEFT_CENTER, format!("#{number:03}"), egui::FontId::proportional(12.0), color::MUTED);
                ui.painter().text(egui::pos2(row.left() + COLUMNS[1].0, y), egui::Align2::LEFT_CENTER, &rows[number].name, egui::FontId::proportional(13.5), color::TEXT);
                let (x, width, _, _) = COLUMNS[2];
                ui.painter().text(egui::pos2(row.left() + x + width - 10.0, y), egui::Align2::RIGHT_CENTER, rows[number].species.to_string(), egui::FontId::proportional(12.5), color::TEXT);
                if response.clicked() {
                    *value = number;
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
        state.shown = true;
    });
    if shown.is_none() {
        state = State::default();
    }
    ui.data_mut(|d| d.insert_temp(state_id, state));
    *value != before
}

/// The header of the list: a click on a name sorts the list on that column; a second click reverses the order.
fn header(ui: &mut egui::Ui, state: &mut State) {
    let (bar, _) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, 20.0), egui::Sense::hover());
    for (x, width, label, column) in COLUMNS {
        let cell = egui::Rect::from_min_size(egui::pos2(bar.left() + x, bar.top()), egui::vec2(width, 20.0));
        let response = ui.interact(cell, ui.make_persistent_id(("ability_menu_header", label)), egui::Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let active = state.sort.is_some_and(|(c, _)| c == column);
        let right = column == SortColumn::Species;
        let (position, anchor) = if right { (egui::pos2(cell.right() - 10.0, cell.center().y), egui::Align2::RIGHT_CENTER) } else { (egui::pos2(cell.left(), cell.center().y), egui::Align2::LEFT_CENTER) };
        let ink = if active { color::ACCENT } else { color::MUTED };
        let text = ui.painter().text(position, anchor, label, egui::FontId::proportional(12.0), ink);
        if active {
            let descending = state.sort.is_some_and(|(_, d)| d);
            // A small triangle drawn by hand: the font has no arrow glyph.
            let centre_x = if right { text.left() - 8.0 } else { text.right() + 8.0 };
            let centre_y = cell.center().y;
            let (w, h) = (3.5, 2.5);
            let points = if descending {
                vec![egui::pos2(centre_x - w, centre_y - h), egui::pos2(centre_x + w, centre_y - h), egui::pos2(centre_x, centre_y + h)]
            } else {
                vec![egui::pos2(centre_x - w, centre_y + h), egui::pos2(centre_x + w, centre_y + h), egui::pos2(centre_x, centre_y - h)]
            };
            ui.painter().add(egui::Shape::convex_polygon(points, color::ACCENT, egui::Stroke::NONE));
        }
        if response.clicked() {
            state.sort = match state.sort {
                Some((c, descending)) if c == column => Some((c, !descending)),
                _ => Some((column, right)),
            };
        }
    }
}
