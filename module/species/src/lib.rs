//! The species editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Species;

impl Module for Species {
    fn title(&self) -> &str {
        "Species"
    }

    fn icon(&self) -> &str {
        "S"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Species", "...", session);
    }
}
