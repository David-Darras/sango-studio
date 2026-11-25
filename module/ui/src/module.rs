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

    /// True when the page has changes that are not saved: the window asks before it closes, and before another page opens.
    fn unsaved(&self) -> bool {
        false
    }

    /// Writes the changes that are not saved, as the button Save of the page does. After the call, [`Module::unsaved`] tells if all
    /// is saved; a problem shows in the page.
    fn save_changes(&mut self, _ctx: &egui::Context, _session: &mut Session) {}

    /// Forgets the changes that are not saved: the page reads its files again and keeps the entry that is open. Gives false when the
    /// page cannot do it: the application then makes the page again from zero.
    fn discard_changes(&mut self) -> bool {
        false
    }

    /// Draws the page in the central area.
    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session);
}
