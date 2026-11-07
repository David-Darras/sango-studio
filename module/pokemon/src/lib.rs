//! The pokémon editor.

use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Pokemon;

impl Module for Pokemon {
    fn title(&self) -> &str {
        "Pokémon"
    }

    fn icon(&self) -> &str {
        "P"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::placeholder(ui, "Pokémon", "...", session);
    }
}
