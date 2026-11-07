//! The start page: the user gives the folder of the romfs dump of the game.

use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};

#[derive(Default)]
pub struct Home {
    /// The text of the field.
    path: String,
    /// What the last check said.
    error: Option<String>,
    /// True once the field took the path of the session.
    filled: bool,
}

impl Home {
    fn check(&mut self, session: &mut Session) {
        match session.set_dump(&self.path) {
            Ok(()) => self.error = None,
            Err(message) => self.error = Some(message),
        }
    }
}

impl Module for Home {
    fn title(&self) -> &str {
        "Home"
    }

    fn icon(&self) -> &str {
        "H"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        if !self.filled {
            self.filled = true;
            if let Some(dump) = &session.dump {
                self.path = dump.root().to_string_lossy().to_string();
            }
        }
        ui.add_space(8.0);
        ui.heading(sango_core::NAME);
        ui.label(widget::hint(&format!("For {}, version {}.", sango_core::GAME, sango_core::GAME_VERSION)));
        ui.add_space(16.0);
        widget::card(ui, |ui| {
            ui.set_width(640.0_f32.min(ui.available_width()));
            ui.label(widget::title("Romfs dump"));
            ui.label(widget::hint("Sango Studio needs the dump of the romfs of your game: the folder that has the folder \"a\"."));
            ui.add_space(6.0);
            let mut apply = false;
            ui.horizontal(|ui| {
                let field = ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(ui.available_width() - 120.0).hint_text("C:\\...\\romfs"));
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply = true;
                }
                if ui.button("Browse...").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().set_title("Folder of the romfs dump").pick_folder() {
                        self.path = folder.to_string_lossy().to_string();
                        apply = true;
                    }
                }
            });
            let open = egui::Button::new(egui::RichText::new("Open the dump").color(color::WHITE)).fill(color::ACCENT);
            if ui.add(open).clicked() {
                apply = true;
            }
            if apply {
                self.check(session);
            }
            ui.add_space(6.0);
            match (&session.dump, &self.error) {
                (_, Some(message)) => {
                    widget::badge(ui, "Not usable", color::ROSE);
                    ui.label(widget::hint(message));
                }
                (Some(dump), None) => {
                    widget::badge(ui, "Dump found", color::GREEN);
                    ui.label(widget::hint(&dump.root().to_string_lossy()));
                }
                (None, None) => {
                    widget::badge(ui, "No dump", color::AMBER);
                }
            }
        });
    }
}
