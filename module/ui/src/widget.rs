use crate::theme::color;
use eframe::egui::{self, Rounding, Stroke};

/// A white card with a thin border.
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(color::WHITE)
        .stroke(Stroke::new(1.0_f32, color::BORDER))
        .rounding(Rounding::same(14.0))
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, add_contents)
        .inner
}

/// The title of a section.
pub fn title(text: &str) -> egui::RichText {
    egui::RichText::new(text).strong().size(17.0)
}

/// A grey line of explanation.
pub fn hint(text: &str) -> egui::RichText {
    egui::RichText::new(text).color(color::MUTED).size(13.5)
}

/// A small coloured label.
pub fn badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32) {
    let galley = egui::WidgetText::from(egui::RichText::new(text).size(12.5).strong().color(egui::Color32::WHITE))
        .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Small);
    let size = galley.size() + egui::vec2(16.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(size.y / 2.0), fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, egui::Color32::WHITE);
}

/// A button of the side bar: filled with the main colour when it is the current page. The icon is in
/// a column of its own, so that the labels of all the buttons start at the same place.
pub fn side_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let fill = if selected {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        egui::Color32::TRANSPARENT
    };
    let text = if selected { color::WHITE } else { color::TEXT };
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    let centre_y = rect.center().y;
    ui.painter().text(egui::pos2(rect.left() + 28.0, centre_y), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(15.0), if selected { color::WHITE } else { color::ACCENT });
    ui.painter().text(egui::pos2(rect.left() + 54.0, centre_y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(15.0), text);
    response
}

/// A card that says which folder of Home is missing.
pub fn missing_card(ui: &mut egui::Ui, message: &str) {
    card(ui, |ui| {
        ui.set_width(420.0_f32.min(ui.available_width()));
        badge(ui, "Folders needed", color::ROSE);
        ui.add_space(4.0);
        ui.label(hint(message));
    });
}

/// A page that is not written yet. Without the two folders of Home, it asks for them.
pub fn placeholder(ui: &mut egui::Ui, name: &str, text: &str, session: &sango_core::Session) {
    ui.add_space(8.0);
    ui.heading(name);
    ui.label(hint(text));
    ui.add_space(16.0);
    if let Some(message) = session.missing() {
        missing_card(ui, message);
        return;
    }
    card(ui, |ui| {
        ui.set_width(420.0_f32.min(ui.available_width()));
        badge(ui, "Not started", color::AMBER);
        ui.add_space(4.0);
        ui.label(hint("..."));
    });
}

/// Adds a step to a number and keeps it between the limits.
pub fn stepped(value: f64, delta: f64, min: f64, max: f64) -> f64 {
    (value + delta).clamp(min, max)
}

/// One of the two small arrow buttons of a spin box. Returns how many steps to do now:
/// one at the click, then more while the button stays pressed.
fn arrow_button(ui: &mut egui::Ui, up: bool) -> u32 {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 14.0), egui::Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        color::SURFACE_STRONG
    };
    ui.painter().rect_filled(rect, Rounding::same(4.0), fill);
    let ink = if response.is_pointer_button_down_on() { color::WHITE } else { color::TEXT };
    let c = rect.center();
    let (w, h) = (4.5, 2.5);
    let points = if up { vec![egui::pos2(c.x - w, c.y + h), egui::pos2(c.x + w, c.y + h), egui::pos2(c.x, c.y - h)] } else { vec![egui::pos2(c.x - w, c.y - h), egui::pos2(c.x + w, c.y - h), egui::pos2(c.x, c.y + h)] };
    ui.painter().add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
    // The first step is at the click. After a short wait, the button repeats while it is pressed.
    let id = response.id.with("hold");
    if response.is_pointer_button_down_on() {
        let now = ui.input(|i| i.time);
        let (start, last): (f64, f64) = ui.data_mut(|d| *d.get_temp_mut_or(id, (now, now)));
        if response.clicked() {
            return 1;
        }
        ui.ctx().request_repaint();
        if now - start > 0.45 && now - last > 0.05 {
            ui.data_mut(|d| d.insert_temp(id, (start, now)));
            return 1;
        }
        0
    } else {
        ui.data_mut(|d| d.remove::<(f64, f64)>(id));
        response.clicked() as u32
    }
}

/// A number field with two arrow buttons. The up and down keys also change it while it has the focus,
/// and shift makes the step ten times bigger. Returns true when the value changed.
pub fn spin<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, min: f64, max: f64) -> bool {
    let before = value.to_f64();
    let mut delta = 0.0;
    // The keys are taken before the field is drawn: the field would use them to move its cursor.
    let id = ui.next_auto_id();
    if ui.memory(|m| m.has_focus(id)) {
        ui.input_mut(|i| {
            for (modifiers, unit) in [(egui::Modifiers::SHIFT, 10.0), (egui::Modifiers::NONE, 1.0)] {
                if i.consume_key(modifiers, egui::Key::ArrowUp) {
                    delta += unit;
                }
                if i.consume_key(modifiers, egui::Key::ArrowDown) {
                    delta -= unit;
                }
            }
        });
    }
    let unit = if ui.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
    ui.add(egui::DragValue::new(value).range(min..=max).speed(0.5));
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        delta += unit * arrow_button(ui, true) as f64;
        delta -= unit * arrow_button(ui, false) as f64;
    });
    if delta != 0.0 {
        *value = T::from_f64(stepped(value.to_f64(), delta, min, max));
        // While the field is being typed in, it keeps its own text and would write the old number
        // back: the text is dropped, and the field shows the new number at the next frame.
        ui.data_mut(|d| d.remove::<String>(id));
    }
    value.to_f64() != before
}

#[cfg(test)]
mod test {
    use super::stepped;

    #[test]
    fn a_step_stays_between_the_limits() {
        assert_eq!(stepped(10.0, 1.0, 0.0, 255.0), 11.0);
        assert_eq!(stepped(255.0, 1.0, 0.0, 255.0), 255.0);
        assert_eq!(stepped(0.0, -10.0, 0.0, 255.0), 0.0);
        assert_eq!(stepped(250.0, 10.0, 0.0, 255.0), 255.0);
    }
}
