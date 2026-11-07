//! The move editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Move;

impl Module for Move {
    fn title(&self) -> &str {
        "Move"
    }

    fn icon(&self) -> &str {
        "M"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Move", "...", session);
    }
}
