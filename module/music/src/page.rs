//! The Music page: the musics and the jingles of the game. The user listens to them, exports them as WAV files, and replaces them by
//! a WAV or an MP3 file.
//!
//! A replaced music is a file `<name>.dspadpcm.bcstm` in the folder `sango/bgm` of the SD card, with the name of the file of the game:
//! the plugin plays it in place of the file of the game. Some sounds share their file: a change shows for all of them. The sound
//! effects are in the tab "Sound effects" (see `effects.rs`).

use crate::bcsar::{Archive, Kind, Place};
use crate::bcstm::{self, Music};
use crate::{ids, names, wav};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The rate of the sound of the 3DS: an imported sound is brought to it.
const RATE: u32 = 32728;

/// A music or a jingle of the list.
#[derive(Clone, Debug)]
struct Entry {
    /// The English name: the name of the music, or "Jingle".
    name: String,
    /// The id of the sound: the group in the high 16 bits, the number in the group in the low 16 bits.
    id: u32,
    /// The file of the folder `sound`.
    file: String,
    /// How many sounds use the same file.
    shared: usize,
    /// True for a jingle.
    jingle: bool,
}

impl Entry {
    /// The text of the list: the name, then the number between parentheses.
    fn title(&self) -> String {
        format!("{} ({})", self.name, self.id & 0xFFFF)
    }

    /// A name for a file: the name in lower case with letters and digits only, then the number.
    fn file_name(&self) -> String {
        let words: String = self.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        format!("{}_{}", words.trim_matches('_'), self.id & 0xFFFF)
    }
}

/// The result of an import.
pub(crate) type Outcome = Result<String, String>;

#[derive(Default)]
pub struct MusicPage {
    /// The folder `sound` of the dump that the list was read from.
    folder: Option<PathBuf>,
    entries: Vec<Entry>,
    search: String,
    jingles: bool,
    /// True when the list shows the sound effects.
    effects_tab: bool,
    effects: crate::effects::Effects,
    selected: Option<usize>,
    /// The sound that is shown: its text and where it comes from.
    shown: Option<(usize, Result<Music, String>, bool)>,
    /// The flag that stops the sound that plays.
    playing: Option<Arc<AtomicBool>>,
    job: Option<Receiver<Outcome>>,
    /// The loop of an imported sound.
    loop_on: bool,
    loop_seconds: f32,
    status: Option<(bool, String)>,
}

/// Reads the list of the musics and the jingles of the dump.
fn read_entries(folder: &Path) -> Result<Vec<Entry>, String> {
    let archive = Archive::parse(&std::fs::read(folder.join("sango_sound.bcsar")).map_err(|e| format!("sango_sound.bcsar: {e}"))?)?;
    let mut entries = Vec::new();
    for sound in archive.sounds.iter().filter(|s| s.kind == Kind::Stream) {
        let Place::External(file) = &sound.place else { continue };
        let Some(&(_, id)) = ids::STREAMS.iter().find(|(index, _)| *index as usize == sound.index) else { continue };
        let jingle = id >> 16 == ids::GROUP_JINGLE;
        let name = if jingle {
            "Jingle".to_string()
        } else {
            match names::MUSIC_NAMES.get((id & 0xFFFF) as usize) {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => "Music".to_string(),
            }
        };
        entries.push(Entry { name, id, file: file.clone(), shared: 0, jingle });
    }
    // One row for each name: the sounds that share a file keep their own name, and the count tells it.
    let counts: Vec<usize> = entries.iter().map(|e| entries.iter().filter(|o| o.file == e.file).count()).collect();
    for (entry, count) in entries.iter_mut().zip(counts) {
        entry.shared = count;
    }
    Ok(entries)
}

pub(crate) fn play(music: &Music) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let (flag, rate, channels) = (stop.clone(), music.rate, music.channels.len().max(1) as u16);
    let mut samples = Vec::with_capacity(music.frames() * channels as usize);
    for i in 0..music.frames() {
        for channel in &music.channels {
            samples.push(channel[i]);
        }
    }
    std::thread::spawn(move || {
        let Ok((_stream, handle)) = rodio::OutputStream::try_default() else { return };
        let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
        sink.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
        while !sink.empty() && !flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        sink.stop();
        flag.store(true, Ordering::Relaxed);
    });
    stop
}

/// The sound as the game will play it: the file of the SD card, else the file of the game.
fn read_current(folder: &Path, sd: Option<&SdCard>, file: &str) -> (Result<Music, String>, bool) {
    if let Some(bytes) = sd.and_then(|sd| std::fs::read(sd.bgm_folder().join(file)).ok()) {
        if let Ok(music) = bcstm::decode(&bytes) {
            return (Ok(music), true);
        }
    }
    let result = std::fs::read(folder.join(file)).map_err(|e| format!("{file}: {e}")).and_then(|bytes| bcstm::decode(&bytes));
    (result, false)
}

impl MusicPage {
    fn load(&mut self, folder: &Path) {
        self.folder = Some(folder.to_path_buf());
        self.selected = None;
        self.shown = None;
        match read_entries(folder) {
            Ok(entries) => self.entries = entries,
            Err(message) => {
                self.entries.clear();
                self.status = Some((false, message));
            }
        }
    }

    fn stop(&mut self) {
        if let Some(flag) = self.playing.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    fn is_playing(&self) -> bool {
        self.playing.as_ref().is_some_and(|flag| !flag.load(Ordering::Relaxed))
    }

    /// Reads the sound of an entry, and sets the loop of an import from it.
    fn select(&mut self, index: usize, sd: Option<&SdCard>) {
        self.stop();
        self.selected = Some(index);
        let Some(folder) = self.folder.clone() else { return };
        let (music, from_card) = read_current(&folder, sd, &self.entries[index].file);
        if let Ok(music) = &music {
            self.loop_on = music.loop_start.is_some();
            self.loop_seconds = music.loop_start.map_or(0.0, |s| s as f32 / music.rate.max(1) as f32);
        }
        self.shown = Some((index, music, from_card));
        self.status = None;
    }

    /// Starts the import of a file in a thread: reads it, brings it to the layout of the stream, and writes it on the SD card.
    fn start_import(&mut self, path: PathBuf, sd: &SdCard) {
        let Some(index) = self.selected else { return };
        let Some(folder) = self.folder.clone() else { return };
        let file = self.entries[index].file.clone();
        let target = sd.bgm_folder().join(&file);
        let (loop_on, loop_seconds) = (self.loop_on, self.loop_seconds);
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let outcome = (|| -> Outcome {
                let template = std::fs::read(folder.join(&file)).map_err(|e| format!("{file}: {e}"))?;
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let imported = wav::read(&bytes)?;
                let from_file = imported.loop_start;
                let music = wav::resample(&bcstm::with_channels(&imported, bcstm::channel_count(&template)?), RATE);
                // The loop of the file wins when the file has one; else the choice of the page.
                let start = from_file.map(|s| (s as u64 * RATE as u64 / imported.rate.max(1) as u64) as u32).or_else(|| loop_on.then(|| (loop_seconds * RATE as f32) as u32));
                let music = Music { loop_start: start, ..music };
                let encoded = bcstm::encode(&template, &music)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                std::fs::write(&target, &encoded).map_err(|e| format!("{}: {e}", target.display()))?;
                Ok(format!("{file} was replaced ({:.1} s, {} channel(s)). Restart the game to hear it.", music.seconds(), music.channels.len()))
            })();
            let _ = sender.send(outcome);
        });
        self.job = Some(receiver);
        self.status = None;
    }

    fn export(&mut self, index: usize, mp3: bool) {
        let Some((_, Ok(music), _)) = &self.shown else { return };
        let name = self.entries[index].file_name();
        let (extension, label) = if mp3 { ("mp3", "MP3 sound") } else { ("wav", "WAV sound") };
        let Some(path) = rfd::FileDialog::new().add_filter(label, &[extension]).set_file_name(format!("{name}.{extension}")).save_file() else { return };
        let bytes = if mp3 { crate::mp3::write_mp3(music) } else { Ok(wav::write_wav(music)) };
        self.status = Some(match bytes.and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string())) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn reset(&mut self, index: usize, sd: &SdCard) {
        let file = self.entries[index].file.clone();
        self.status = Some(match std::fs::remove_file(sd.bgm_folder().join(&file)) {
            Ok(()) => (true, "The music of the game is back. Restart the game.".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (true, "The music of the game was already there.".to_string()),
            Err(error) => (false, error.to_string()),
        });
        self.select(index, Some(sd));
    }

    fn list(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        ui.horizontal(|ui| {
            if ui.selectable_label(!self.effects_tab && !self.jingles, "Musics").clicked() {
                (self.effects_tab, self.jingles) = (false, false);
            }
            if ui.selectable_label(!self.effects_tab && self.jingles, "Jingles").clicked() {
                (self.effects_tab, self.jingles) = (false, true);
                self.stop();
            }
            if ui.selectable_label(self.effects_tab, "Sound effects").clicked() {
                self.effects_tab = true;
                self.stop();
            }
        });
        ui.add_space(4.0);
        if self.effects_tab {
            if let Some(sd) = sd {
                self.effects.list(ui, sd);
            }
            return;
        }
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search").desired_width(220.0));
        ui.add_space(4.0);
        let needle = self.search.to_lowercase();
        let rows: Vec<usize> = (0..self.entries.len()).filter(|&i| self.entries[i].jingle == self.jingles && (needle.is_empty() || self.entries[i].title().to_lowercase().contains(&needle))).collect();
        let mut chosen = None;
        egui::ScrollArea::vertical().id_salt("music_list").max_height(560.0).auto_shrink([false, true]).show_rows(ui, 24.0, rows.len(), |ui, range| {
            for &i in &rows[range] {
                let on_card = sd.is_some_and(|sd| sd.bgm_folder().join(&self.entries[i].file).exists());
                let text = self.entries[i].title();
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.selected == Some(i), text).clicked() {
                        chosen = Some(i);
                    }
                    if on_card {
                        widget::badge(ui, "Replaced", color::TEAL);
                    }
                });
            }
        });
        if let Some(i) = chosen {
            self.select(i, sd);
        }
    }

    fn detail(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        let Some(index) = self.selected else {
            ui.label(widget::hint("Choose a music in the list."));
            return;
        };
        let entry = self.entries[index].clone();
        let busy = self.job.is_some();
        widget::card(ui, |ui| {
            ui.label(widget::title(&entry.title()));
            ui.label(widget::hint(&format!("File: {}{}", entry.file, if entry.shared > 1 { format!(" (used by {} sounds: a change shows for all of them)", entry.shared) } else { String::new() })));
            match &self.shown {
                Some((i, Ok(music), from_card)) if *i == index => {
                    ui.horizontal(|ui| {
                        ui.label(format!("{:.1} s, {} Hz, {}", music.seconds(), music.rate, if music.channels.len() == 1 { "mono" } else { "stereo" }));
                        match music.loop_start {
                            Some(start) => widget::badge(ui, &format!("Loop from {:.1} s", start as f32 / music.rate.max(1) as f32), color::ACCENT),
                            None => widget::badge(ui, "No loop", color::AMBER),
                        }
                        if *from_card {
                            widget::badge(ui, "From the SD card", color::TEAL);
                        }
                    });
                }
                Some((i, Err(message), _)) if *i == index => {
                    ui.label(egui::RichText::new(message).color(color::ROSE));
                }
                _ => {
                    ui.label(widget::hint("Reading…"));
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if self.is_playing() {
                    if widget::button(ui, "Stop") {
                        self.stop();
                    }
                } else if widget::button_styled(ui, egui::RichText::new("Play").color(color::WHITE), Some(color::ACCENT)) {
                    if let Some((_, Ok(music), _)) = &self.shown {
                        self.playing = Some(play(music));
                    }
                }
                if widget::button(ui, "Export WAV…") {
                    self.export(index, false);
                }
                if widget::button(ui, "Export MP3…") {
                    self.export(index, true);
                }
            });
            ui.add_space(10.0);
            ui.label(widget::title("Replace"));
            ui.label(widget::hint("Choose a WAV or an MP3 file. It is brought to 32728 Hz and to the channels of this music, then written for the game."));
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.loop_on, "Loop");
                ui.add_enabled(self.loop_on, egui::DragValue::new(&mut self.loop_seconds).speed(0.1).range(0.0..=3600.0).suffix(" s"));
                ui.label(widget::hint("start of the loop (a WAV file with its own loop keeps it)"));
            });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if widget::button_styled(ui, egui::RichText::new("Import WAV or MP3…").color(color::WHITE), Some(color::ACCENT)) {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Sound", &["wav", "mp3"]).pick_file() {
                            self.start_import(path, sd);
                        }
                    }
                });
                let replaced = sd.bgm_folder().join(&entry.file).exists();
                ui.add_enabled_ui(replaced && !busy, |ui| {
                    if widget::button(ui, "Put the music of the game back") {
                        self.reset(index, sd);
                    }
                });
            });
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(widget::hint("Making the file. This can take some seconds."));
                });
            }
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
    }
}

impl Module for MusicPage {
    fn title(&self) -> &str {
        "Music"
    }

    fn icon(&self) -> &str {
        "🎵"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Music");
        ui.add_space(8.0);
        if let Some(message) = session.missing() {
            widget::missing_card(ui, message);
            return;
        }
        let session: &Session = session;
        let (Some(sd), Some(dump)) = (session.sd.as_ref(), session.dump.as_ref()) else { return };
        let folder = dump.root().join("sound");
        if self.folder.as_deref() != Some(folder.as_path()) {
            self.load(&folder);
        }
        if self.effects_tab {
            self.effects.load(&folder, sd);
        }
        // The result of an import.
        if let Some(receiver) = &self.job {
            match receiver.try_recv() {
                Ok(outcome) => {
                    self.job = None;
                    let status = match outcome {
                        Ok(message) => (true, message),
                        Err(message) => (false, message),
                    };
                    if let Some(index) = self.selected {
                        // Reading the sound again clears the message: the result is put back after it.
                        self.select(index, Some(sd));
                    }
                    self.status = Some(status);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.job = None,
            }
        }
        if self.entries.is_empty() {
            if let Some((_, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(color::ROSE));
            } else {
                ui.label(widget::hint("The folder sound of the dump has no sound archive."));
            }
            return;
        }
        ui.columns(2, |columns| {
            self.list(&mut columns[0], Some(sd));
            egui::ScrollArea::vertical().id_salt("music_detail").auto_shrink([false, false]).show(&mut columns[1], |ui| {
                if self.effects_tab {
                    self.effects.detail(ui, sd);
                } else {
                    self.detail(ui, sd);
                }
            });
        });
        if self.is_playing() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game (`SANGO_DUMP`): the page lists the musics with their numbers, draws, imports a WAV file in a stream, and
    /// puts the music of the game back.
    #[test]
    fn the_page_lists_the_musics_and_imports_a_wav_file() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_music_page_{}", std::process::id()));
        std::fs::create_dir_all(folder.join("sdmc")).unwrap();
        let sd = SdCard::open(folder.join("sdmc").to_str().unwrap()).unwrap();

        let mut page = MusicPage::default();
        page.load(&Path::new(&root).join("sound"));
        let theme = page.entries.iter().position(|e| e.title() == "Evolution (1)").expect("the evolution music is listed with its name and its number");
        assert!(page.entries.iter().any(|e| e.jingle));

        page.select(theme, Some(&sd));
        assert!(matches!(&page.shown, Some((_, Ok(_), false))));
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0)));
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                page.list(ui, Some(&sd));
                page.detail(ui, &sd);
            });
        });

        // A WAV file of 2 seconds (a tone, one channel) replaces the stream and keeps the layout of the game file.
        let tone = Music { rate: 22050, channels: vec![(0..44100).map(|i| ((i as f32 * 0.05).sin() * 9000.0) as i16).collect()], loop_start: None };
        let wav_path = folder.join("tone.wav");
        std::fs::write(&wav_path, wav::write_wav(&tone)).unwrap();
        page.loop_on = true;
        page.loop_seconds = 0.5;
        page.start_import(wav_path, &sd);
        let receiver = page.job.take().unwrap();
        let message = receiver.recv_timeout(std::time::Duration::from_secs(60)).unwrap().unwrap();
        assert!(message.contains("was replaced"), "{message}");
        let written = std::fs::read(sd.bgm_folder().join(&page.entries[theme].file)).unwrap();
        let music = bcstm::decode(&written).unwrap();
        assert_eq!(music.rate, RATE);
        assert_eq!(music.channels.len(), 2, "the stream keeps the channels of the game file");
        assert!((music.seconds() - 2.0).abs() < 0.05);
        assert_eq!(music.loop_start, Some(14 * (0.5 * RATE as f32 / 14.0) as u32));
        // The file is the same kind of file as the one of the game.
        let original = std::fs::read(Path::new(&root).join("sound").join(&page.entries[theme].file)).unwrap();
        assert_eq!(&written[..4], &original[..4]);
        assert_eq!(bcstm::channel_count(&written).unwrap(), bcstm::channel_count(&original).unwrap());

        page.reset(theme, &sd);
        assert!(!sd.bgm_folder().join(&page.entries[theme].file).exists());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
