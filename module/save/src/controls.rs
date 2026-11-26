//! The controls of the page that the other pages do not have: a text field that the keys open, a menu of names, and the grid of
//! the places of the party and of the boxes.

use sango_ui::theme::color;
use sango_ui::{egui, focus, widget};

/// A text field of one line. With the keys: Enter starts to type, and Enter again stops. Gives true when the text changed.
pub fn text_field(ui: &mut egui::Ui, id_salt: &str, value: &mut String, length: usize, width: f32) -> bool {
    let id = ui.make_persistent_id(("save_text", id_salt));
    let response = ui.add(egui::TextEdit::singleline(value).id(id).char_limit(length).desired_width(width));
    widget::register_focusable(ui, id, response.rect);
    if response.clicked() || response.has_focus() {
        widget::set_active_id(ui, id);
    }
    if widget::is_active(ui, id) && widget::page_has_focus(ui) && !response.has_focus() && !response.lost_focus() && !ui.ctx().wants_keyboard_input() {
        widget::glow(ui, response.rect, 6.0);
        widget::scroll_active_into_view(ui, &response);
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            response.request_focus();
        }
    }
    response.changed()
}

/// A menu of names for a number from 0. Gives true when the value changed.
pub fn menu(ui: &mut egui::Ui, id_salt: &str, value: &mut usize, names: &[String], width: f32) -> bool {
    let before = *value;
    let shown = names.get(*value).cloned().unwrap_or_else(|| format!("Value {value}"));
    let combo = egui::ComboBox::from_id_salt(("save_menu", id_salt)).selected_text(shown).width(width).height(360.0).show_ui(ui, |ui| {
        for (at, name) in names.iter().enumerate() {
            ui.selectable_value(value, at, name);
        }
    });
    let values: Vec<usize> = (0..names.len()).collect();
    focus::combo_arrows(ui, &combo.response, value, &values);
    *value != before
}

/// What a place of a grid shows.
pub struct Cell {
    /// The icon of the Pokémon, or nothing for an empty place.
    pub icon: Option<egui::TextureHandle>,
    pub text: String,
    /// True when the place is not as in the file.
    pub changed: bool,
}

/// The size of a place of a grid.
pub const CELL: egui::Vec2 = egui::vec2(104.0, 62.0);

/// The id of a grid, for the keys and the tests.
pub fn grid_id(id_salt: &str) -> egui::Id {
    egui::Id::new(("save_grid", id_salt))
}

/// A grid of places. A click chooses one, and the four arrows move the choice when the grid has the focus. Gives true when the
/// chosen place changed.
pub fn grid(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], columns: usize, chosen: &mut usize) -> bool {
    let before = *chosen;
    let id = grid_id(id_salt);
    let gap = 4.0;
    let rows = cells.len().div_ceil(columns);
    let size = egui::vec2(columns as f32 * (CELL.x + gap) - gap, rows as f32 * (CELL.y + gap) - gap);
    let (area, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let mut clicked = false;
    for (at, cell) in cells.iter().enumerate() {
        let corner = area.min + egui::vec2((at % columns) as f32 * (CELL.x + gap), (at / columns) as f32 * (CELL.y + gap));
        let rect = egui::Rect::from_min_size(corner, CELL);
        let response = ui.interact(rect, id.with(at), egui::Sense::click());
        if response.clicked() {
            *chosen = at;
            clicked = true;
        }
        let selected = *chosen == at;
        let fill = if selected { color::ACCENT.gamma_multiply(0.18) } else if response.hovered() { color::SURFACE } else { color::WHITE };
        let line = if selected { color::ACCENT } else { color::FIELD_BORDER };
        ui.painter().rect(rect, 6.0, fill, egui::Stroke::new(if selected { 2.0_f32 } else { 1.0 }, line));
        if let Some(icon) = &cell.icon {
            let place = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 9.0), egui::vec2(48.0, 36.0));
            ui.painter().image(icon.id(), place, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        let ink = if cell.changed { color::ACCENT } else { color::TEXT };
        let font = egui::FontId::proportional(11.5);
        let text = ui.painter().layout(cell.text.clone(), font, ink, CELL.x - 6.0);
        // One line only: the end of a long name is cut by the place.
        let at = egui::pos2(rect.center().x - text.size().x.min(CELL.x - 6.0) / 2.0, rect.bottom() - 17.0);
        ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).galley(at, text, ink);
    }
    // From the first column, the arrow left goes out of the grid; from the others, it moves the choice.
    ui.data_mut(|d| d.insert_temp(id.with("handles_left"), *chosen % columns != 0));
    if focus::focus(ui, id, area.intersect(ui.clip_rect()), clicked, true, None) {
        let across = focus::steps(ui, true);
        let down = focus::steps(ui, false) * columns as isize;
        if let Some(to) = focus::stepped(*chosen, across + down, cells.len()) {
            *chosen = to;
        }
    }
    *chosen != before
}
