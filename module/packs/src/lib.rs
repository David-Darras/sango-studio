//! The Packs page: the species packs that are installed, and the buttons to install and remove them.
//!
//! A pack holds species, forms and their 3D models (see `docs/species-packs.md`). The page installs a pack with one click:
//! it gives numbers to the species, writes the table that the plugin reads, keeps a copy of the pack, and writes the models
//! of all the packs where the plugin reads them. Writing the models takes some seconds, so the work runs in a thread.

use sango_core::{Dump, SdCard};
use sango_pack::library::{MAX_FORMS, MAX_MODELS, MAX_SPECIES};
use sango_pack::{Library, Pack};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// What a click asks for.
#[derive(Clone, Debug)]
enum Action {
    Install(Vec<PathBuf>),
    Remove(String),
    Restore(String),
    /// Writes all the files again from the installed packs.
    Rewrite,
}

/// A job that runs in a thread: it gives back the library and what to tell the user.
struct Job {
    receiver: Receiver<(Library, (bool, String))>,
}

#[derive(Default)]
pub struct Packs {
    /// The folder of the SD card that the library was read from.
    root: Option<PathBuf>,
    library: Library,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    job: Option<Job>,
}

/// Does what the user asked, and writes everything on the SD card: the registry, the table, the packs and the models.
fn run_action(mut library: Library, sd: &SdCard, dump: &Dump, action: &Action) -> (Library, (bool, String)) {
    let result = (|| -> Result<String, String> {
        match action {
            Action::Install(files) => {
                let mut packs = Vec::new();
                let mut problems = Vec::new();
                for file in files {
                    match Pack::open(file) {
                        Ok(pack) => packs.push(pack),
                        Err(message) => problems.push(format!("{}: {message}", file.display())),
                    }
                }
                let mut installed = Vec::new();
                for (id, result) in library.install_many(packs) {
                    match result {
                        Ok(()) => installed.push(id),
                        Err(message) => problems.push(format!("{id}: {message}")),
                    }
                }
                if installed.is_empty() {
                    return Err(if problems.is_empty() { "No pack was chosen.".to_string() } else { problems.join(" ") });
                }
                let models = library.apply(sd, dump)?;
                let done = format!("Installed: {} ({models} models written). Restart the game to see the new species.", installed.join(", "));
                if problems.is_empty() {
                    Ok(done)
                } else {
                    Err(format!("{done} Not installed: {}", problems.join(" ")))
                }
            }
            Action::Remove(id) => {
                library.remove(id)?;
                library.apply(sd, dump)?;
                Ok(format!("Removed {id}. Its numbers stay reserved. Restart the game."))
            }
            Action::Rewrite => {
                let models = library.apply(sd, dump)?;
                Ok(format!("The files were written again ({models} models). Restart the game."))
            }
            Action::Restore(id) => {
                let pack = library.packs.iter().find(|p| p.id == *id).cloned().ok_or_else(|| format!("The pack '{id}' is not there."))?;
                library.install(pack)?;
                library.apply(sd, dump)?;
                Ok(format!("Installed {id} again, with its first numbers. Restart the game."))
            }
        }
    })();
    let status = match result {
        Ok(message) => (true, message),
        Err(message) => (false, message),
    };
    (library, status)
}

impl Packs {
    fn load(&mut self, sd: &SdCard) {
        self.root = Some(sd.root().to_path_buf());
        match Library::open(sd) {
            Ok(library) => self.library = library,
            Err(message) => {
                self.library = Library::default();
                self.status = Some((false, message));
            }
        }
    }

    /// Starts an action in a thread.
    fn start(&mut self, session: &Session, action: Action) {
        let (Some(sd), Some(dump)) = (session.sd.as_ref(), session.dump.as_ref()) else { return };
        let (sd_root, dump_root) = (sd.root().to_string_lossy().to_string(), dump.root().to_string_lossy().to_string());
        let library = self.library.clone();
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let outcome = match (SdCard::open(&sd_root), Dump::open(&dump_root)) {
                (Ok(sd), Ok(dump)) => run_action(library, &sd, &dump, &action),
                (Err(message), _) | (_, Err(message)) => (library, (false, message)),
            };
            let _ = sender.send(outcome);
        });
        self.job = Some(Job { receiver });
        self.status = None;
    }

    /// Takes the result of the job when it is done.
    fn poll(&mut self, ctx: &egui::Context, sd: &SdCard) {
        let Some(job) = &self.job else { return };
        match job.receiver.try_recv() {
            Ok((library, status)) => {
                // The thread worked on a copy: the library of the page is the one that it gives back, and what is on the SD
                // card is read again to be sure.
                self.library = library;
                self.job = None;
                if !status.0 {
                    if let Ok(on_card) = Library::open(sd) {
                        self.library = on_card;
                    }
                }
                self.status = Some(status);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(200)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.job = None;
                self.status = Some((false, "The installation stopped without an answer.".to_string()));
            }
        }
    }

    fn install_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let busy = self.job.is_some();
        widget::card(ui, |ui| {
            ui.label(widget::title("Install"));
            ui.label(widget::hint("A pack is a .sangopack file with the species, the forms and the 3D models. Choose one or several: a pack that needs another one waits for it."));
            ui.add_space(6.0);
            ui.add_enabled_ui(!busy, |ui| {
                if widget::button_styled(ui, egui::RichText::new("+ Install packs…").color(color::WHITE), Some(color::ACCENT)) {
                    let mut dialog = rfd::FileDialog::new().add_filter("Species pack", &["sangopack"]).set_title("Choose the packs to install");
                    if Path::new("packs").is_dir() {
                        dialog = dialog.set_directory("packs");
                    }
                    if let Some(files) = dialog.pick_files() {
                        self.start(session, Action::Install(files));
                    }
                }
            });
            ui.add_enabled_ui(!busy && !self.library.registry.packs.is_empty(), |ui| {
                if widget::button(ui, "Write the files again") {
                    self.start(session, Action::Rewrite);
                }
            })
            .response
            .on_hover_text("Writes the table and the models again from the installed packs: use it if the game does not show the species or the models.");
            ui.add_space(6.0);
            ui.label(widget::hint(&format!(
                "{} of {MAX_SPECIES} places for species used, {} of {MAX_FORMS} for forms, {} of {MAX_MODELS} for models.",
                self.library.registry.species.len(),
                self.library.registry.forms.len(),
                self.library.registry.models.len()
            )));
            if busy {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(widget::hint("Writing the files and the models. This can take a minute: do not close Studio."));
                });
            } else if let Some((ok, message)) = &self.status {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
    }

    fn list_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let busy = self.job.is_some();
        let mut asked = None;
        widget::card(ui, |ui| {
            ui.label(widget::title("Installed packs"));
            if self.library.registry.packs.is_empty() {
                ui.label(widget::hint("No pack is installed."));
            }
            for installed in &self.library.registry.packs {
                let pack = self.library.packs.iter().find(|p| p.id == installed.id);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}  {}", installed.name, installed.version)).strong());
                    if installed.active {
                        widget::badge(ui, "Installed", color::GREEN);
                    } else {
                        widget::badge(ui, "Removed", color::AMBER);
                    }
                    if let Some(pack) = pack {
                        ui.label(widget::hint(&format!("{} species, {} forms, {} models", pack.species.len(), pack.forms.len(), pack.model_names().len())));
                    }
                });
                if let Some(pack) = pack {
                    let numbers: Vec<u16> = pack.species.iter().filter_map(|s| self.library.number_of(&s.name)).collect();
                    if let (Some(first), Some(last)) = (numbers.iter().min(), numbers.iter().max()) {
                        ui.label(widget::hint(&format!("Numbers {first} to {last}")));
                    }
                    for (label, text) in [("Author", &pack.author), ("License", &pack.license), ("Credit", &pack.credit)] {
                        if !text.is_empty() {
                            ui.label(widget::hint(&format!("{label}: {text}")));
                        }
                    }
                }
                ui.add_enabled_ui(!busy, |ui| {
                    if installed.active {
                        if widget::button(ui, "Remove") {
                            asked = Some(Action::Remove(installed.id.clone()));
                        }
                    } else if widget::button(ui, "Install again") {
                        asked = Some(Action::Restore(installed.id.clone()));
                    }
                });
                ui.separator();
            }
        });
        if let Some(action) = asked {
            self.start(session, action);
        }
    }
}

impl Module for Packs {
    fn title(&self) -> &str {
        "Packs"
    }

    fn icon(&self) -> &str {
        "P"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Packs");
        ui.add_space(8.0);
        if let Some(message) = session.missing() {
            widget::missing_card(ui, message);
            return;
        }
        let session: &Session = session;
        let Some(sd) = session.sd.as_ref() else { return };
        if self.root.as_deref() != Some(sd.root()) {
            self.load(sd);
        }
        self.poll(ui.ctx(), sd);
        egui::ScrollArea::vertical().id_salt("packs_page").auto_shrink([false, false]).show(ui, |ui| {
            self.install_card(ui, session);
            ui.add_space(8.0);
            self.list_card(ui, session);
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_pack::pack::{to_hex, ModelRef, Move, PackSpecies, DATA_SIZE};

    fn pack(id: &str, name: &str) -> Pack {
        Pack {
            id: id.into(),
            name: id.into(),
            version: "1".into(),
            species: vec![PackSpecies { name: name.into(), model: ModelRef::Native { native: 25 }, data: to_hex(&[1u8; DATA_SIZE]), moves: vec![Move { id: 1, level: 1 }], evolutions: Vec::new() }],
            ..Pack::default()
        }
    }

    /// With a dump: packs are installed, removed and restored, and the files are on the SD card each time.
    #[test]
    fn packs_are_installed_removed_and_restored_with_the_files_written() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let outer = std::env::temp_dir().join(format!("sango_packs_page_{}", std::process::id()));
        let sdmc = outer.join("sdmc");
        std::fs::create_dir_all(sdmc.join("sango-studio")).unwrap();
        let sd = SdCard::open(sdmc.to_str().unwrap()).unwrap();
        let files: Vec<PathBuf> = ["one", "two"]
            .iter()
            .map(|id| {
                let path = sdmc.join(format!("{id}.sangopack"));
                pack(id, &id.to_uppercase()).save(&path).unwrap();
                path
            })
            .collect();
        let (library, status) = run_action(Library::default(), &sd, &dump, &Action::Install(files));
        assert!(status.0, "{status:?}");
        assert!(sd.read("species/table.bin").is_some());
        let (library, status) = run_action(library, &sd, &dump, &Action::Remove("one".into()));
        assert!(status.0, "{status:?}");
        assert_eq!(Library::open(&sd).unwrap().table().unwrap().species[0].name, "");
        let (library, status) = run_action(library, &sd, &dump, &Action::Restore("one".into()));
        assert!(status.0, "{status:?}");
        assert_eq!(Library::open(&sd).unwrap().table().unwrap().species[0].name, "ONE");
        // A wrong file is reported and does not stop the others.
        let broken = sdmc.join("broken.sangopack");
        std::fs::write(&broken, b"nope").unwrap();
        let three = sdmc.join("three.sangopack");
        pack("three", "THREE").save(&three).unwrap();
        let (_, status) = run_action(library, &sd, &dump, &Action::Install(vec![broken, three]));
        assert!(!status.0 && status.1.contains("Installed: three"), "{status:?}");
        let _ = std::fs::remove_dir_all(&outer);
    }

    #[test]
    fn the_page_draws_and_asks_for_the_folders_first() {
        let mut session = Session::default();
        let mut page = Packs::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let outer = std::env::temp_dir().join(format!("sango_packs_draw_{}", std::process::id()));
        std::fs::create_dir_all(outer.join("sdmc/sango-studio")).unwrap();
        session.dump = Some(Dump::open(&root).unwrap());
        session.sd = Some(SdCard::open(outer.join("sdmc").to_str().unwrap()).unwrap());
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        let _ = std::fs::remove_dir_all(&outer);
    }
}
