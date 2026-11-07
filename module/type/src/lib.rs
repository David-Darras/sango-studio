//! The type editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Type;

impl Module for Type {
    fn title(&self) -> &str {
        "Type"
    }

    fn icon(&self) -> &str {
        "T"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Type", "...", session);
    }
}
