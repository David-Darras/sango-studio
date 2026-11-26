//! The card "Run": builds the plugin and starts the game in the emulator.
//!
//! The build is the one of the plugin, in the shell of devkitPro: `make -j8 run-overlay GAME=ORAS`. It builds the overlay and copies it
//! to the SD card of the emulator. The studio starts the emulator itself, with the paths of the file `config.mk` of the plugin, so that
//! it knows when the build ends: it gives `EMULATOR=true` to make, which then starts nothing.

use crate::{folder_row, Field, State};
use sango_core::setting;
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

/// The command of the build, as the user would type it.
const MAKE: &str = "make -j8 run-overlay GAME=ORAS";
/// The folder where the installer of devkitPro puts it.
const USUAL_DEVKITPRO: &str = "C:/devkitPro";
/// The shell of devkitPro, from its folder.
const SHELL: &str = "msys2/usr/bin/bash.exe";
/// The number of lines of the output that stay.
const KEPT_LINES: usize = 600;

/// The paths of the file `config.mk` that the run needs.
#[derive(Debug, PartialEq, Eq)]
struct Config {
    emulator: String,
    game: String,
}

/// Reads the file `config.mk`: lines `NAME := value`, a value can be between quotes.
fn parse_config(text: &str) -> Result<Config, String> {
    let mut values: HashMap<&str, &str> = HashMap::new();
    for line in text.lines().map(str::trim).filter(|line| !line.starts_with('#')) {
        if let Some((name, value)) = line.split_once('=') {
            values.insert(name.trim_end_matches([':', '?']).trim(), value.trim().trim_matches('"'));
        }
    }
    let value = |name: &str| values.get(name).copied().filter(|value| !value.is_empty());
    let missing = |name: &str| format!("The file config.mk of the plugin has no {name}.");
    if value("SDMC").or_else(|| value("DEST")).is_none() {
        return Err(missing("SDMC (the SD card folder of the emulator)"));
    }
    Ok(Config {
        emulator: value("EMULATOR").ok_or_else(|| missing("EMULATOR"))?.to_string(),
        game: value("GAME_PATH").or_else(|| value("ORAS_GAME_PATH")).ok_or_else(|| missing("ORAS_GAME_PATH"))?.to_string(),
    })
}

fn read_config(plugin: &Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(plugin.join("config.mk")).map_err(|_| "The plugin has no file config.mk: copy config.mk.example to config.mk and give your paths in it.".to_string())?;
    parse_config(&text)
}

/// What one run does.
struct Job {
    shell: PathBuf,
    plugin: PathBuf,
    /// Words after the command of make (the tests give another folder for the plugin file).
    more: String,
    emulator: String,
    game: String,
}

/// Where the last run is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Building,
    Started,
    Failed(String),
}

#[derive(Default)]
struct Progress {
    phase: Phase,
    lines: Vec<String>,
    /// The emulator that the last run started.
    game: Option<Child>,
}

type Shared = Arc<Mutex<Progress>>;

fn lock(shared: &Shared) -> MutexGuard<'_, Progress> {
    shared.lock().unwrap_or_else(|broken| broken.into_inner())
}

/// A line of the output without the codes of the colors of a terminal.
fn plain_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut characters = text.trim_end().chars();
    while let Some(c) = characters.next() {
        if c == '\u{1b}' {
            // The code ends with its first letter.
            characters.by_ref().find(char::is_ascii_alphabetic);
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// The program, without a window of its own on Windows.
fn quiet(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    command
}

/// Runs make; each line of its output goes to the progress.
fn build(shared: &Shared, job: &Job, repaint: &dyn Fn()) -> Result<(), String> {
    let folder = job.plugin.to_string_lossy().replace('\\', "/").replace('\'', "'\\''");
    let script = format!("exec 2>&1; cd '{folder}' && {MAKE} EMULATOR=true{}", job.more);
    let mut make = quiet(&job.shell)
        .args(["-lc", &script])
        .env("MSYSTEM", "MSYS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", job.shell.display()))?;
    if let Some(output) = make.stdout.take() {
        let mut output = BufReader::new(output);
        let mut bytes = Vec::new();
        while output.read_until(b'\n', &mut bytes).is_ok_and(|count| count > 0) {
            let mut progress = lock(shared);
            progress.lines.push(plain_line(&bytes));
            let extra = progress.lines.len().saturating_sub(KEPT_LINES);
            progress.lines.drain(..extra);
            drop(progress);
            bytes.clear();
            repaint();
        }
    }
    let status = make.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("The build failed: the output says why.".to_string())
    }
}

/// Starts the emulator with the game. The one of the run before closes first.
fn play(shared: &Shared, job: &Job) -> Result<(), String> {
    let mut progress = lock(shared);
    if let Some(mut before) = progress.game.take() {
        if matches!(before.try_wait(), Ok(None)) {
            let _ = before.kill();
            let _ = before.wait();
        }
    }
    let game = Command::new(&job.emulator).arg(&job.game).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    progress.game = Some(game.map_err(|e| format!("The emulator did not start ({}): {e}", job.emulator))?);
    Ok(())
}

/// Starts a run on another thread. Gives nothing when a build already runs.
fn start(shared: &Shared, job: Job, repaint: impl Fn() + Send + 'static) -> Option<std::thread::JoinHandle<()>> {
    {
        let mut progress = lock(shared);
        if progress.phase == Phase::Building {
            return None;
        }
        progress.phase = Phase::Building;
        progress.lines.clear();
    }
    let shared = shared.clone();
    Some(std::thread::spawn(move || {
        let result = build(&shared, &job, &repaint).and_then(|()| play(&shared, &job));
        lock(&shared).phase = match result {
            Ok(()) => Phase::Started,
            Err(message) => Phase::Failed(message),
        };
        repaint();
    }))
}

#[derive(Default)]
pub struct Run {
    plugin: Field,
    devkitpro: Field,
    /// True once the fields took the folders of the settings.
    filled: bool,
    shared: Shared,
}

/// Why a folder cannot be the one of the plugin.
fn check_plugin(folder: &Path) -> Result<(), String> {
    if !folder.join("Makefile").is_file() || !folder.join("overlay").is_dir() {
        return Err("This folder is not the Sango plugin (the folder with the Makefile and the folder overlay).".into());
    }
    read_config(folder).map(|_| ())
}

/// Why a folder cannot be the one of devkitPro.
fn check_devkitpro(folder: &Path) -> Result<(), String> {
    if folder.join(SHELL).is_file() {
        Ok(())
    } else {
        Err(format!("This folder has no {SHELL}. Give the folder of devkitPro."))
    }
}

/// Draws one folder of the card and checks it. A folder that the user gives is kept for the next run.
fn checked_row(ui: &mut egui::Ui, title: &str, hint: &str, picker: &str, field: &mut Field, check: fn(&Path) -> Result<(), String>, keep: fn(&mut setting::Settings, String)) -> Option<PathBuf> {
    let text = field.text.trim().trim_matches('"').to_string();
    let folder = (!text.is_empty()).then(|| PathBuf::from(&text));
    let error = folder.as_deref().and_then(|folder| check(folder).err());
    let state = match (&error, &folder) {
        (Some(message), _) => State::Refused(message),
        (None, Some(_)) => State::Found,
        (None, None) => State::Empty,
    };
    if folder_row(ui, title, hint, picker, field, state) {
        let given = field.text.trim().trim_matches('"').to_string();
        if check(Path::new(&given)).is_ok() {
            let mut settings = setting::read();
            keep(&mut settings, given);
            setting::write(&settings);
        }
    }
    folder.filter(|_| error.is_none())
}

impl Run {
    pub fn card(&mut self, ui: &mut egui::Ui) {
        if !self.filled {
            self.filled = true;
            let settings = setting::read();
            self.plugin.text = settings.plugin.unwrap_or_default();
            let usual = Path::new(USUAL_DEVKITPRO).join(SHELL).is_file().then(|| USUAL_DEVKITPRO.to_string());
            self.devkitpro.text = settings.devkitpro.or(usual).unwrap_or_default();
        }
        widget::card(ui, |ui| {
            ui.set_width(680.0_f32.min(ui.available_width()));
            ui.label(widget::title("Run"));
            ui.label(widget::hint("Builds the plugin for Alpha Sapphire and starts the game in the emulator."));
            ui.add_space(6.0);
            let plugin = checked_row(ui, "Plugin folder", "C:\\...\\sango_plugin", "Folder of the Sango plugin", &mut self.plugin, check_plugin, |s, folder| s.plugin = Some(folder));
            ui.add_space(6.0);
            let devkitpro = checked_row(ui, "devkitPro folder", USUAL_DEVKITPRO, "Folder of devkitPro", &mut self.devkitpro, check_devkitpro, |s, folder| s.devkitpro = Some(folder));
            ui.add_space(10.0);

            let (phase, game_runs) = {
                let mut progress = lock(&self.shared);
                let runs = progress.game.as_mut().is_some_and(|game| matches!(game.try_wait(), Ok(None)));
                (progress.phase.clone(), runs)
            };
            let building = phase == Phase::Building;
            let ready = plugin.zip(devkitpro);
            ui.horizontal(|ui| {
                let clicked = ui
                    .add_enabled_ui(ready.is_some() && !building, |ui| {
                        let text = egui::RichText::new("▶ Run").color(color::WHITE).strong();
                        let (button, clicked) = widget::button_response(ui, text, Some(color::ACCENT));
                        button.on_hover_text(format!("{MAKE}, then the emulator. The paths are in config.mk of the plugin. The emulator that this button started closes first."));
                        clicked
                    })
                    .inner;
                match &phase {
                    Phase::Idle => {}
                    Phase::Building => {
                        ui.spinner();
                        ui.label("Building...");
                    }
                    Phase::Started if game_runs => widget::badge(ui, "Game running", color::GREEN),
                    Phase::Started => widget::badge(ui, "Built", color::GREEN),
                    Phase::Failed(_) => widget::badge(ui, "Failed", color::ROSE),
                }
                if let (true, Some((plugin, devkitpro))) = (clicked, ready) {
                    match read_config(&plugin) {
                        Ok(config) => {
                            let job = Job { shell: devkitpro.join(SHELL), plugin, more: String::new(), emulator: config.emulator, game: config.game };
                            let ctx = ui.ctx().clone();
                            start(&self.shared, job, move || ctx.request_repaint());
                        }
                        Err(message) => lock(&self.shared).phase = Phase::Failed(message),
                    }
                }
            });
            if let Phase::Failed(message) = &phase {
                ui.label(egui::RichText::new(message).color(color::ROSE));
            }
            self.output(ui, matches!(phase, Phase::Building | Phase::Failed(_)));
        });
    }

    /// The output of the build: open while it runs and after a failure.
    fn output(&self, ui: &mut egui::Ui, open: bool) {
        let progress = lock(&self.shared);
        if progress.lines.is_empty() {
            return;
        }
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Output").id_salt(("run_output", open)).default_open(open).show(ui, |ui| {
            egui::ScrollArea::both().id_salt("run_lines").max_height(280.0).auto_shrink([false, true]).stick_to_bottom(true).show(ui, |ui| {
                for line in &progress.lines {
                    let wrong = line.contains("error") || line.contains("Error") || line.contains("***");
                    let text = egui::RichText::new(line).monospace().size(12.0);
                    ui.add(egui::Label::new(if wrong { text.color(color::ROSE) } else { text }).extend());
                }
            });
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_paths_of_the_config_file_are_read() {
        let text = "# EMULATOR := no\n\n# GAME := XY\nSDMC     := C:/Azahar/sdmc\nEMULATOR := \"C:/Program Files/Azahar/azahar.exe\"\nORAS_GAME_PATH := \"C:/cia/sango.3ds\"\nXY_GAME_PATH := \"C:/cia/kujira.cci\"\n";
        let config = parse_config(text).unwrap();
        assert_eq!(config, Config { emulator: "C:/Program Files/Azahar/azahar.exe".into(), game: "C:/cia/sango.3ds".into() });
        assert!(parse_config("SDMC := a\nEMULATOR := b\n").unwrap_err().contains("ORAS_GAME_PATH"));
        assert!(parse_config("EMULATOR := b\nORAS_GAME_PATH := c\n").unwrap_err().contains("SDMC"));
    }

    #[test]
    fn the_codes_of_the_colors_leave_a_line() {
        assert_eq!(plain_line(b"\x1b[1;31merror:\x1b[0m no\r\n"), "error: no");
    }

    /// With `SANGO_PLUGIN` (the folder of the plugin): the build of the plugin runs for real. The plugin file goes to a temporary
    /// folder, and the shell takes the place of the emulator.
    #[test]
    fn the_plugin_is_built() {
        let Ok(plugin) = std::env::var("SANGO_PLUGIN") else { return };
        let folder = std::env::temp_dir().join(format!("sango_run_{}", std::process::id()));
        let shell = Path::new(USUAL_DEVKITPRO).join(SHELL);
        let more = format!(" DEST='{}'", folder.to_string_lossy().replace('\\', "/"));
        let job = Job { shell: shell.clone(), plugin: plugin.into(), more, emulator: shell.to_string_lossy().to_string(), game: "--version".into() };
        let shared = Shared::default();
        start(&shared, job, || {}).unwrap().join().unwrap();
        let progress = lock(&shared);
        assert_eq!(progress.phase, Phase::Started, "{}", progress.lines.join("\n"));
        assert!(folder.join("sango_plugin.3gx").is_file(), "{}", progress.lines.join("\n"));
        drop(progress);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
