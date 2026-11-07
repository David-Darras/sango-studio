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

/// A page that is not written yet. Without the dump of the game, it asks for it.
pub fn placeholder(ui: &mut egui::Ui, name: &str, text: &str, session: &sango_core::Session) {
    ui.add_space(8.0);
    ui.heading(name);
    ui.label(hint(text));
    ui.add_space(16.0);
    card(ui, |ui| {
        ui.set_width(420.0_f32.min(ui.available_width()));
        if session.dump.is_none() {
            badge(ui, "Dump needed", color::ROSE);
            ui.add_space(4.0);
            ui.label(hint("Choose the romfs dump of the game in Home first."));
        } else {
            badge(ui, "Not started", color::AMBER);
            ui.add_space(4.0);
            ui.label(hint("..."));
        }
    });
}
