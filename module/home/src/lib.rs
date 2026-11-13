//! The start page: the user gives the folders that Sango Studio needs.
//!
//! 1. The dump of the romfs of the game. Sango Studio only reads it.
//! 2. The folder of the SD card. Sango Studio puts here the files that it changed.
//! 3. The folder of devkitPro and 4. the folder of the Sango plugin: the code editor needs them to compile a function.

use sango_core::SD_FOLDER_NAME;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};

/// What one of the two cards says about its folder.
enum State<'a> {
    /// The folder is good: its path.
    Found(&'a str),
    /// The folder is refused: the reason.
    Refused(&'a str),
    /// Nothing was given.
    Empty,
}

/// One text field with what the last check said.
#[derive(Default)]
struct Field {
    /// The text of the field.
    text: String,
    /// What the last check said.
    error: Option<String>,
}

#[derive(Default)]
pub struct Home {
    dump: Field,
    sd: Field,
    devkitpro: Field,
    plugin: Field,
    /// True once the fields took the folders of the session.
    filled: bool,
}

/// Draws a card with a folder: a title, an explanation, the field, and the state.
/// Returns true when the user asks to use the folder.
fn folder_card(ui: &mut egui::Ui, title: &str, text: &str, hint: &str, picker: &str, field: &mut Field, state: State) -> bool {
    let mut apply = false;
    ui.push_id(title, |ui| {
        widget::card(ui, |ui| {
            ui.set_width(680.0_f32.min(ui.available_width()));
            ui.label(widget::title(title));
            ui.label(widget::hint(text));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let edit = ui.add(egui::TextEdit::singleline(&mut field.text).desired_width(ui.available_width() - 120.0).hint_text(hint));
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply = true;
                }
                if widget::button(ui, "Browse...") {
                    if let Some(folder) = rfd::FileDialog::new().set_title(picker).pick_folder() {
                        field.text = folder.to_string_lossy().to_string();
                        apply = true;
                    }
                }
            });
            if widget::button_styled(ui, egui::RichText::new("Use this folder").color(color::WHITE), Some(color::ACCENT)) {
                apply = true;
            }
            ui.add_space(6.0);
            match state {
                State::Refused(message) => {
                    widget::badge(ui, "Not usable", color::ROSE);
                    ui.label(widget::hint(message));
                }
                State::Found(path) => {
                    widget::badge(ui, "Ready", color::GREEN);
                    ui.label(widget::hint(path));
                }
                State::Empty => widget::badge(ui, "Missing", color::AMBER),
            }
        });
    });
    apply
}

/// The state of a folder card from the last error and the folder that the session has.
fn state_of<'a>(error: &'a Option<String>, path: &'a Option<String>) -> State<'a> {
    match (error, path) {
        (Some(message), _) => State::Refused(message),
        (None, Some(path)) => State::Found(path),
        (None, None) => State::Empty,
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
        egui::ScrollArea::vertical().id_salt("home_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.content(ui, session);
        });
    }
}

impl Home {
    /// The cards of the page.
    fn content(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        if !self.filled {
            self.filled = true;
            if let Some(dump) = &session.dump {
                self.dump.text = dump.root().to_string_lossy().to_string();
            }
            if let Some(sd) = &session.sd {
                self.sd.text = sd.root().to_string_lossy().to_string();
            }
            if let Some(devkitpro) = &session.devkitpro {
                self.devkitpro.text = devkitpro.to_string_lossy().to_string();
            }
            if let Some(plugin) = &session.plugin {
                self.plugin.text = plugin.to_string_lossy().to_string();
            }
        }
        ui.add_space(8.0);
        ui.heading(sango_core::NAME);
        ui.label(widget::hint(&format!("For {}, version {}. The first two folders are needed by the editors; the last two are for the code editor.", sango_core::GAME, sango_core::GAME_VERSION)));
        ui.add_space(16.0);

        let dump_path = session.dump.as_ref().map(|d| d.root().to_string_lossy().to_string());
        let dump_error = self.dump.error.clone();
        let apply = folder_card(
            ui,
            "Romfs dump (read only)",
            "The dump of the romfs of your game: the folder that has the folder \"a\". Sango Studio never changes it.",
            "C:\\...\\romfs",
            "Folder of the romfs dump",
            &mut self.dump,
            state_of(&dump_error, &dump_path),
        );
        if apply {
            self.dump.error = session.set_dump(&self.dump.text).err();
        }

        ui.add_space(12.0);
        let sd_path = session.sd.as_ref().map(|s| s.root().to_string_lossy().to_string());
        let sd_error = self.sd.error.clone();
        let apply = folder_card(
            ui,
            "SD card folder",
            &format!("The folder {SD_FOLDER_NAME} in the sdmc folder of the emulator or of the console. The files that you change go here, and the game loads them again while it runs."),
            &format!("C:\\...\\sdmc\\{SD_FOLDER_NAME}"),
            "Folder of the SD card",
            &mut self.sd,
            state_of(&sd_error, &sd_path),
        );
        if apply {
            self.sd.error = session.set_sd(&self.sd.text).err();
        }

        ui.add_space(12.0);
        let devkitpro_path = session.devkitpro.as_ref().map(|p| p.to_string_lossy().to_string());
        let devkitpro_error = self.devkitpro.error.clone();
        let apply = folder_card(
            ui,
            "devkitPro (optional)",
            "The folder of devkitPro, with devkitARM inside: the code editor uses its compiler (arm-none-eabi-g++) to compile a function.",
            "C:/devkitPro",
            "Folder of devkitPro",
            &mut self.devkitpro,
            state_of(&devkitpro_error, &devkitpro_path),
        );
        if apply {
            self.devkitpro.error = session.set_devkitpro(&self.devkitpro.text).err();
        }

        ui.add_space(12.0);
        let plugin_path = session.plugin.as_ref().map(|p| p.to_string_lossy().to_string());
        let plugin_error = self.plugin.error.clone();
        let apply = folder_card(
            ui,
            "Sango plugin (optional)",
            "The folder of the Sango plugin, the one with the Makefile: the code editor completes the names of lib/include and compiles with its headers.",
            "C:/.../sango_plugin",
            "Folder of the Sango plugin",
            &mut self.plugin,
            state_of(&plugin_error, &plugin_path),
        );
        if apply {
            self.plugin.error = session.set_plugin(&self.plugin.text).err();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn home_renders_and_registers_buttons() {
        let mut home = Home::default();
        let mut session = Session::default();
        session.focus_page = true;
        let ctx = egui::Context::default();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                home.ui(ui, &mut session);
            });
        });

        // Next frame: update_navigation will have the Browse and Use buttons
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                widget::set_page_focus(ctx, true);
                widget::update_navigation(ui);
                home.ui(ui, &mut session);
                assert!(widget::active_id(ui).is_some());
            });
        });
    }
}
