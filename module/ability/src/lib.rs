//! The ability editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Ability;

impl Module for Ability {
    fn title(&self) -> &str {
        "Ability"
    }

    fn icon(&self) -> &str {
        "Ab"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Ability", "...", session);
    }
}
