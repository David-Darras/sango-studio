//! The cry card of the General tab: the cry of a species or a form, that the user can hear, export as a WAV file and replace by a sound
//! file (WAV or MP3).
//!
//! A cry is two files of the archive 100 (see `sango_cry`). The studio replaces the wave archive: it writes it on the SD card as
//! `species/cry/<file>.bin`, where the plugin reads it in place of the file of the game. Some cries share their file: a change shows for all
//! of them.

use sango_core::{Dump, Garc, SdCard};
use sango_cry::table::{self, Voice, KINDS};
use sango_cry::{archive, sound, Sound, ARCHIVE};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The place of the wave archive of a cry on the SD card: the plugin reads it in place of the file of the game.
pub fn cry_path(file: u32) -> String {
    format!("species/cry/{file}.bin")
}

/// The rate of the sounds of the game: an imported sound is brought to it.
const RATE: u32 = 32728;
/// The longest sound that the studio accepts, in seconds: the files of the game are 2 seconds long at most, and the game keeps them in a
/// small memory.
const MAX_SECONDS: f32 = 3.0;

/// The state of the cry card.
#[derive(Default)]
pub struct CryTab {
    garc: Option<Garc>,
    /// The wave archives that the user changed and did not save, by the number of their file.
    pending: HashMap<u32, Vec<u8>>,
    /// The files that were read from the SD card, or written on it.
    from_card: HashSet<u32>,
    /// The kind of cry that the card shows (an index of `KINDS`).
    kind: usize,
    /// The sound that is shown: the file, the bytes that it was made from, and the sound.
    shown: Option<(u32, Vec<u8>, Sound)>,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The sound that plays: the flag stops it.
    playing: Option<Arc<AtomicBool>>,
    /// The format of the export: false for an MP3 file (the default), true for a WAV file.
    wav: bool,
}

/// Plays a sound in a thread of its own (the audio stream cannot move between threads). The flag stops it.
fn play(sound: &Sound) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let (flag, samples, rate) = (stop.clone(), sound.samples.clone(), sound.rate);
    std::thread::spawn(move || {
        let Ok((_stream, handle)) = rodio::OutputStream::try_default() else { return };
        let Ok(sink) = rodio::Sink::try_new(&handle) else { return };
        sink.append(rodio::buffer::SamplesBuffer::new(1, rate, samples));
        while !sink.empty() && !flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        sink.stop();
        flag.store(true, Ordering::Relaxed);
    });
    stop
}

impl CryTab {
    /// Opens the archive of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.garc = Garc::open(&dump.archive(ARCHIVE)).ok();
        self.shown = None;
    }

    /// The wave archive as the user sees it: the changes first, then the SD card, then the file of the game.
    fn current(&mut self, sd: Option<&SdCard>, file: u32) -> Option<Vec<u8>> {
        if let Some(bytes) = self.pending.get(&file) {
            return Some(bytes.clone());
        }
        if let Some(bytes) = sd.and_then(|sd| sd.read(&cry_path(file))).filter(|bytes| archive::read_archive(bytes).is_ok()) {
            self.from_card.insert(file);
            return Some(bytes);
        }
        self.garc.as_ref()?.first_file(file as usize).map(|bytes| bytes.to_vec())
    }

    /// True when a cry changed and was not saved.
    pub fn changed(&self) -> bool {
        !self.pending.is_empty()
    }

    /// True when a cry that was read comes from the SD card.
    pub fn from_card(&self) -> bool {
        !self.from_card.is_empty()
    }

    /// Writes the cries that changed on the SD card, where the plugin reads them.
    pub fn save(&mut self, sd: &SdCard) -> Result<(), String> {
        let mut files: Vec<u32> = self.pending.keys().copied().collect();
        files.sort_unstable();
        for file in files {
            sd.write(&cry_path(file), &self.pending[&file])?;
            self.pending.remove(&file);
            self.from_card.insert(file);
        }
        Ok(())
    }

    /// The kinds of cries that the game has for a species and a form: the first of each different file.
    fn kinds(species: usize, form: usize) -> Vec<(usize, Voice)> {
        let mut found: Vec<(usize, Voice)> = Vec::new();
        for kind in 0..KINDS.len() {
            if let Some(voice) = table::voice(species, form, kind) {
                if !found.iter().any(|(_, other)| other.wave == voice.wave) {
                    found.push((kind, voice));
                }
            }
        }
        found
    }

    /// Draws the card.
    pub fn ui(&mut self, ui: &mut egui::Ui, session: &Session, species: usize, form: usize) {
        let sd = session.sd.as_ref();
        let kinds = Self::kinds(species, form);
        widget::card(ui, |ui| {
            let Some((_, first)) = kinds.first().copied() else {
                ui.label(widget::title("Cry"));
                ui.label(widget::hint("The game has no cry for this species or this form."));
                return;
            };
            if !kinds.iter().any(|(kind, _)| *kind == self.kind) {
                self.kind = kinds[0].0;
            }
            let voice = kinds.iter().find(|(kind, _)| *kind == self.kind).map_or(first, |(_, voice)| *voice);
            let file = voice.wave;
            let bytes = self.current(sd, file);
            // The sound is decoded again when the file or its bytes change.
            let same = matches!((&self.shown, &bytes), (Some((shown_file, shown_bytes, _)), Some(now)) if *shown_file == file && shown_bytes == now);
            if !same {
                self.shown = bytes.as_ref().and_then(|bytes| archive::read_archive(bytes).ok().map(|sound| (file, bytes.clone(), sound)));
            }
            ui.horizontal(|ui| {
                ui.label(widget::title("Cry"));
                if self.pending.contains_key(&file) {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_card.contains(&file) {
                    widget::badge(ui, "From the SD card", color::TEAL);
                }
            });
            if kinds.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label(widget::hint("Kind"));
                    for (kind, _) in &kinds {
                        if ui.selectable_label(self.kind == *kind, KINDS[*kind]).clicked() {
                            self.kind = *kind;
                        }
                    }
                });
            }
            if self.shown.is_none() {
                ui.label(widget::hint("No sound."));
            }
            ui.add_space(6.0);
            let busy = self.playing.as_ref().is_some_and(|flag| !flag.load(Ordering::Relaxed));
            ui.vertical(|ui| {
                // The big button: Play, or Stop while the sound plays.
                let seconds = self.shown.as_ref().map_or(0.0, |(_, _, sound)| sound.seconds());
                let label = if busy { "Stop".to_string() } else { format!("Play {seconds:.2} s") };
                let clicked = ui.add_enabled_ui(self.shown.is_some(), |ui| widget::button_styled(ui, egui::RichText::new(label).color(color::WHITE), Some(color::ACCENT))).inner;
                if clicked {
                    if busy {
                        if let Some(flag) = &self.playing {
                            flag.store(true, Ordering::Relaxed);
                        }
                    } else if let Some((_, _, sound)) = &self.shown {
                        self.playing = Some(play(sound));
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if widget::button(ui, "Export") {
                        self.status = Some(self.export(species, form));
                    }
                    egui::ComboBox::from_id_salt("cry_export_format").width(96.0).selected_text(if !self.wav { "MP3" } else { "WAV" }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.wav, true, "WAV");
                        ui.selectable_value(&mut self.wav, false, "MP3");
                    });
                });
                if widget::button(ui, "Import") {
                    if let Some(path) = rfd::FileDialog::new().set_title("Sound for the cry").add_filter("Sound", &["wav", "mp3"]).pick_file() {
                        self.status = Some(match self.import(&path, file) {
                            Ok(message) => (true, message),
                            Err(message) => (false, message),
                        });
                    }
                }
                let changed = self.pending.contains_key(&file) || self.from_card.contains(&file);
                ui.add_enabled_ui(changed, |ui| {
                    if widget::button(ui, "Reset") {
                        self.status = Some(self.reset(sd, file));
                    }
                });
            });
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
        if self.playing.as_ref().is_some_and(|flag| !flag.load(Ordering::Relaxed)) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Writes the cry that is shown as a WAV or an MP3 file that the user chooses.
    fn export(&self, species: usize, form: usize) -> (bool, String) {
        let Some((_, _, sound)) = &self.shown else { return (false, "There is no sound to export.".into()) };
        let (extension, label) = if !self.wav { ("mp3", "MP3 sound") } else { ("wav", "WAV sound") };
        let Some(path) = rfd::FileDialog::new().set_title("Export the cry").set_file_name(format!("cry_{species:04}_{form}.{extension}")).add_filter(label, &[extension]).save_file() else { return (true, String::new()) };
        let bytes = if !self.wav {
            let music = sango_music::bcstm::Music { rate: sound.rate, channels: vec![sound.samples.clone()], loop_start: None };
            sango_music::mp3::write_mp3(&music)
        } else {
            Ok(sound::write_wav(sound))
        };
        let written = bytes.and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()));
        match written {
            Ok(()) => (true, format!("Exported {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        }
    }

    /// Reads a sound file and makes it the cry of a file of the archive (not saved yet).
    fn import(&mut self, path: &std::path::Path, file: u32) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let read = sound::read(&bytes)?;
        let made = sound::resample(&read, RATE);
        if made.seconds() > MAX_SECONDS {
            return Err(format!("This sound lasts {:.1} seconds: a cry must last {MAX_SECONDS} seconds at most.", made.seconds()));
        }
        if made.samples.len() < 100 {
            return Err("This sound is too short.".into());
        }
        self.pending.insert(file, archive::build_archive(&made));
        self.shown = None;
        Ok(format!("{} is the cry now ({:.2} seconds). Save to keep it.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), made.seconds()))
    }

    /// Puts the cry of the game back: forgets the change and removes the file of the SD card.
    fn reset(&mut self, sd: Option<&SdCard>, file: u32) -> (bool, String) {
        self.pending.remove(&file);
        self.from_card.remove(&file);
        self.shown = None;
        if let Some(sd) = sd {
            if let Err(message) = sd.remove(&cry_path(file)).map(|_| ()) {
                return (false, message);
            }
        }
        (true, "The cry of the game is back.".to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_kinds_are_the_different_files_of_a_species() {
        // Bulbasaur: the four kinds play the same wave archive with other sequences, so the card shows one cry.
        let kinds = CryTab::kinds(1, 0);
        assert_eq!(kinds.len(), 1);
        assert_eq!(kinds[0].0, 0);
        assert!(CryTab::kinds(0, 0).is_empty());
        // A species of a pack has no cry of its own yet.
        assert!(CryTab::kinds(900, 0).is_empty());
    }

    #[test]
    fn an_imported_sound_becomes_a_wave_archive_and_goes_to_the_sd_card() {
        let outer = std::env::temp_dir().join(format!("sango_cry_tab_{}", std::process::id()));
        std::fs::create_dir_all(&outer).unwrap();
        let sd = SdCard::open(outer.to_str().unwrap()).unwrap();
        let wav = sound::write_wav(&Sound { rate: 22050, samples: (0..11_025).map(|i| ((i as f64 / 22050.0 * 500.0 * std::f64::consts::TAU).sin() * 8000.0) as i16).collect() });
        let file = outer.join("tone.wav");
        std::fs::write(&file, wav).unwrap();
        let mut tab = CryTab::default();
        tab.import(&file, 3776).unwrap();
        assert!(tab.changed());
        tab.save(&sd).unwrap();
        assert!(!tab.changed() && tab.from_card());
        let written = sd.read(&cry_path(3776)).unwrap();
        let back = archive::read_archive(&written).unwrap();
        assert_eq!(back.rate, RATE);
        assert!((back.seconds() - 0.5).abs() < 0.01);
        let list = sd.read("species/cry/index.bin").unwrap();
        assert_eq!(u32::from_le_bytes(list[..4].try_into().unwrap()), 1);
        // A sound that is too long is refused.
        let long = sound::write_wav(&Sound { rate: 8000, samples: vec![0; 8000 * 4] });
        std::fs::write(&file, long).unwrap();
        assert!(tab.import(&file, 3776).unwrap_err().contains("seconds"));
        // The reset removes the file and the entry of the list.
        assert!(tab.reset(Some(&sd), 3776).0);
        assert!(sd.read(&cry_path(3776)).is_none());
        let _ = std::fs::remove_dir_all(&outer);
    }
}
