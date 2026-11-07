use eframe::egui;
use sango_core::Session;

/// A page of the application: an editor or the start page.
///
/// The application lists its modules in the side bar and shows the one that is chosen.
pub trait Module {
    /// A short name for the side bar.
    fn title(&self) -> &str;

    /// One or two characters for the button of the side bar.
    fn icon(&self) -> &str;

    /// Draws the page in the central area.
    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session);
}
