//! The item editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Item;

impl Module for Item {
    fn title(&self) -> &str {
        "Item"
    }

    fn icon(&self) -> &str {
        "I"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Item", "...", session);
    }
}
