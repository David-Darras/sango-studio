//! The start page: the user gives the folders that Sango Studio needs, and runs the game.
//!
//! The two folders of the studio and the save file are in one card, and the card "Run" under it has the two folders of the build. A
//! path is used when the user presses Enter in its field, or picks it with Browse. The card "Start from zero" is at the top.

mod run;

use sango_core::SD_FOLDER_NAME;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};

/// What one of the folders says about itself.
enum State<'a> {
    /// The folder is good.
    Found,
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
    save: Field,
    run: run::Run,
    /// True once the fields took the folders of the session.
    filled: bool,
    /// The card "Start from zero": the user asked and did not answer yet, the box of the answer, and what happened.
    asking: bool,
    understood: bool,
    erased: Option<(bool, String)>,
}

/// What the button Browse picks.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    Folder,
    File,
}

/// Draws one folder of the card, see [`path_row`].
fn folder_row(ui: &mut egui::Ui, title: &str, hint: &str, picker: &str, field: &mut Field, state: State) -> bool {
    path_row(ui, title, hint, picker, Pick::Folder, field, state)
}

/// Draws one folder or file of the card: its title, the field, Browse, and the state. Returns true when it
/// must be used: the user pressed Enter in the field, or picked with Browse.
fn path_row(ui: &mut egui::Ui, title: &str, hint: &str, picker: &str, pick: Pick, field: &mut Field, state: State) -> bool {
    let mut apply = false;
    ui.push_id(title, |ui| {
        ui.label(widget::title(title));
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut field.text).desired_width(ui.available_width() - 110.0).hint_text(hint));
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
            if widget::button(ui, "Browse...") {
                let dialog = rfd::FileDialog::new().set_title(picker);
                let chosen = if pick == Pick::File { dialog.pick_file() } else { dialog.pick_folder() };
                if let Some(chosen) = chosen {
                    field.text = chosen.to_string_lossy().to_string();
                    apply = true;
                }
            }
        });
        match state {
            State::Refused(message) => {
                ui.horizontal(|ui| {
                    widget::badge(ui, "Not usable", color::ROSE);
                    ui.label(widget::hint(message));
                });
            }
            State::Found => widget::badge(ui, "Ready", color::GREEN),
            State::Empty => widget::badge(ui, "Missing", color::AMBER),
        }
    });
    apply
}

/// The state of a folder from the last error and the folder that the session has.
fn state_of<'a>(error: &'a Option<String>, path: &Option<String>) -> State<'a> {
    match (error, path) {
        (Some(message), _) => State::Refused(message),
        (None, Some(_)) => State::Found,
        (None, None) => State::Empty,
    }
}

impl Module for Home {
    fn title(&self) -> &str {
        "Home"
    }

    fn icon(&self) -> &str {
        "🏠"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        egui::ScrollArea::vertical().id_salt("home_scroll").auto_shrink([false, false]).show(ui, |ui| {
            self.content(ui, session);
        });
    }
}

impl Home {
    /// The page: the reset at the top, then the folders.
    fn content(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        if !self.filled {
            self.filled = true;
            if let Some(dump) = &session.dump {
                self.dump.text = dump.root().to_string_lossy().to_string();
            }
            if let Some(sd) = &session.sd {
                self.sd.text = sd.root().to_string_lossy().to_string();
            }
            if let Some(save) = &session.save_file {
                self.save.text = save.to_string_lossy().to_string();
            }
        }
        ui.add_space(8.0);
        if let Some(sd) = session.sd.clone() {
            self.zero_card(ui, &sd);
            ui.add_space(12.0);
        }
        ui.heading(sango_core::NAME);
        ui.label(widget::hint(&format!("{}, version {}", sango_core::GAME, sango_core::GAME_VERSION)));
        ui.add_space(8.0);

        widget::card(ui, |ui| {
            ui.set_width(680.0_f32.min(ui.available_width()));

            let dump_path = session.dump.as_ref().map(|d| d.root().to_string_lossy().to_string());
            let dump_error = self.dump.error.clone();
            if folder_row(ui, "Romfs dump (read only)", "C:\\...\\romfs", "Folder of the romfs dump", &mut self.dump, state_of(&dump_error, &dump_path)) {
                self.dump.error = session.set_dump(&self.dump.text).err();
            }
            ui.add_space(10.0);

            let sd_path = session.sd.as_ref().map(|s| s.root().to_string_lossy().to_string());
            let sd_error = self.sd.error.clone();
            if folder_row(ui, "SD card folder", &format!("C:\\...\\sdmc\\{SD_FOLDER_NAME}"), "Folder of the SD card", &mut self.sd, state_of(&sd_error, &sd_path)) {
                self.sd.error = session.set_sd(&self.sd.text).err();
            }
            ui.add_space(10.0);

            let save_path = session.save_file.as_ref().map(|s| s.to_string_lossy().to_string());
            let save_error = self.save.error.clone();
            let hint = "C:\\...\\title\\00040000\\0011c500\\data\\00000001\\main";
            if path_row(ui, "Save file of the game (main), for the page Save", hint, "The save file of the game: main", Pick::File, &mut self.save, state_of(&save_error, &save_path)) {
                self.save.error = session.set_save_file(&self.save.text).err();
            }
        });
        ui.add_space(12.0);
        self.run.card(ui);
    }
}

impl Home {
    /// The card that removes all the files of the studio, to start again from zero.
    fn zero_card(&mut self, ui: &mut egui::Ui, sd: &sango_core::SdCard) {
        widget::card(ui, |ui| {
            ui.set_width(680.0_f32.min(ui.available_width()));
            ui.label(widget::title("Start from zero"));
            ui.label(widget::hint("Removes all the files of the studio: the changes that you made, the installed packs, the 3D models, the cries, the texts, the music files and the sound effects. The game goes back to its own data. The dump of the game, the settings of the program, the maps of the 3D Maps page and the other files of the plugin (the names, the scripts) stay."));
            ui.add_space(6.0);
            if !self.asking {
                if widget::button(ui, "Start from zero") {
                    self.asking = true;
                    self.understood = false;
                    self.erased = None;
                }
            } else {
                let count = sd.count_everything();
                let line = |what: &str, (files, bytes): (usize, u64)| format!("{what}: {files} files, {:.1} MB", bytes as f64 / 1_048_576.0);
                if sd.is_split() {
                    ui.label(line("On this computer", count.computer));
                    ui.label(widget::hint(&format!("({})", sd.local().display())));
                }
                ui.label(line("On the SD card", count.card));
                ui.label(widget::hint(&format!("({})", sd.root().display())));
                ui.label(line("3D models on the SD card", count.models));
                ui.label(widget::hint(&format!("({})", sd.models_folder().display())));
                ui.add_space(6.0);
                ui.checkbox(&mut self.understood, "I understand that this cannot be undone: the packs have to be installed again.");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(self.understood, |ui| {
                        if widget::button_styled(ui, egui::RichText::new("Erase everything").color(color::WHITE), Some(color::ROSE)) {
                            self.erased = Some(match sd.erase_everything() {
                                Ok(done) => (true, format!("Done: {} files removed on the computer, {} on the SD card, {} models. Restart the game.", done.computer.0, done.card.0, done.models.0)),
                                Err(message) => (false, message),
                            });
                            self.asking = false;
                        }
                    });
                    if widget::button(ui, "Cancel") {
                        self.asking = false;
                    }
                });
            }
            if let Some((ok, message)) = &self.erased {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
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

        // Next frame: update_navigation will have the Browse buttons
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
