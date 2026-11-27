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

/// The species of a pack with this number (a species that the game does not have): its name, and the species of the game whose cry it
/// has until it gets its own (the one whose model it shows, else Bulbasaur).
fn new_species(species: usize) -> Option<(String, usize)> {
    if species <= table::SPECIES {
        return None;
    }
    let record = crate::extras::pack_species(species)?;
    let borrowed = usize::from(record.model_form & 0x7FFF);
    let fallback = if record.model_form & 0x8000 != 0 && (1..=table::SPECIES).contains(&borrowed) { borrowed } else { 1 };
    Some((record.name, fallback))
}

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
    /// Asks the user for a sound file: None is the file window of the system (the tests give a file without it).
    pick: Option<fn() -> Option<std::path::PathBuf>>,
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

    /// The kinds of cries that the game has for a species and a form: the first of each different file. A species of a pack has one
    /// cry, for all its forms.
    fn kinds(species: usize, form: usize) -> Vec<(usize, Voice)> {
        if new_species(species).is_some() {
            let wave = sango_cry::FIRST_NEW_SPECIES_FILE + (species - usize::from(sango_pack::table::FIRST_SPECIES)) as u32;
            return vec![(0, Voice { sequence: 0, wave })];
        }
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
            let own = new_species(species);
            if self.shown.is_none() {
                match &own {
                    Some((_, fallback)) => ui.label(widget::hint(&format!("No cry of its own yet: in the game it has the cry of {}.", sango_species_base::species_name(*fallback)))),
                    None => ui.label(widget::hint("No sound.")),
                };
            }
            if own.is_some() {
                ui.label(widget::hint(&format!("One cry for the species and its forms, {} seconds at most.", sango_cry::MAX_SECONDS)));
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
                    let combo = egui::ComboBox::from_id_salt("cry_export_format").width(96.0).selected_text(if !self.wav { "MP3" } else { "WAV" }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.wav, true, "WAV");
                        ui.selectable_value(&mut self.wav, false, "MP3");
                    });
                    sango_ui::focus::combo_arrows(ui, &combo.response, &mut self.wav, &[true, false]);
                });
                if widget::button(ui, "Import") {
                    let path = match self.pick {
                        Some(pick) => pick(),
                        None => rfd::FileDialog::new().set_title("Sound for the cry").add_filter("Sound", &["wav", "mp3"]).pick_file(),
                    };
                    if let Some(path) = path {
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
                        if own.is_some() && self.status.as_ref().is_some_and(|(ok, _)| *ok) {
                            self.status = Some((true, "The cry of this species is gone.".to_string()));
                        }
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
        if made.samples.len() < 100 {
            return Err("This sound is too short.".into());
        }
        let built = archive::build_archive(&made);
        if built.len() > sango_cry::MAX_ARCHIVE_BYTES {
            return Err(format!("This sound lasts {:.1} seconds: a cry must last {} seconds at most.", made.seconds(), sango_cry::MAX_SECONDS));
        }
        self.pending.insert(file, built);
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
        // A number that no species of a pack has: no cry.
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

    /// Draws a card in a new window (the focus is on its first button), then presses the keys one after the other.
    fn press(keys: &[egui::Key], mut card: impl FnMut(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        for key in [None, None].into_iter().chain(keys.iter().flat_map(|&key| [Some(key), None])) {
            let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() };
            input.events.extend(key.map(|key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::set_page_focus(ctx, true);
                    widget::update_navigation(ui);
                    card(ui);
                });
            });
        }
    }

    fn test_folder() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("sango_new_cry_{}", std::process::id()))
    }

    /// The file that the keys-only test imports, in place of the file window.
    fn pick_tone() -> Option<std::path::PathBuf> {
        Some(test_folder().join("tone.wav"))
    }

    /// A species made from nothing gets a cry and loses it, with the keys only. The file is the one that the plugin reads.
    #[test]
    fn a_new_species_gets_a_cry_with_the_keys() {
        use sango_pack::table::SpeciesRecord;
        let record = SpeciesRecord { id: 724, model_form: 0x8000 | 52, name: "Gecqua".into(), data: vec![1; 0x50], moves: Vec::new(), evolutions: Vec::new() };
        let _table = crate::extras::set_for_test(sango_pack::Table { species: vec![record], forms: Vec::new() });
        assert_eq!(new_species(724), Some(("Gecqua".to_string(), 52)), "without a cry, it has the cry of the model that it shows");
        assert_eq!(CryTab::kinds(724, 1), vec![(0, Voice { sequence: 0, wave: 5000 })], "one cry for the species and its forms");
        std::fs::create_dir_all(test_folder().join("sd")).unwrap();
        let sd = SdCard::open(test_folder().join("sd").to_str().unwrap()).unwrap();
        let session = Session { sd: Some(SdCard::open(test_folder().join("sd").to_str().unwrap()).unwrap()), ..Session::default() };
        let tone = |seconds: f32| sound::write_wav(&Sound { rate: RATE, samples: (0..(seconds * RATE as f32) as usize).map(|i| ((i as f64 / RATE as f64 * 500.0 * std::f64::consts::TAU).sin() * 8000.0) as i16).collect() });

        use egui::Key::{ArrowDown, Enter};
        let mut tab = CryTab { pick: Some(pick_tone), ..CryTab::default() };
        // A sound that the game cannot hold is refused.
        std::fs::write(pick_tone().unwrap(), tone(2.5)).unwrap();
        press(&[ArrowDown, ArrowDown, Enter], |ui| tab.ui(ui, &session, 724, 0));
        assert!(!tab.changed() && tab.status.as_ref().is_some_and(|(ok, message)| !ok && message.contains("seconds")), "{:?}", tab.status);
        // The longest sound that the card accepts fits in the memory of the game.
        std::fs::write(pick_tone().unwrap(), tone(sango_cry::MAX_SECONDS)).unwrap();
        press(&[ArrowDown, ArrowDown, Enter], |ui| tab.ui(ui, &session, 724, 0));
        assert!(tab.changed(), "{:?}", tab.status);
        tab.save(&sd).unwrap();
        let written = sd.read(&cry_path(5000)).unwrap();
        assert_eq!(&written[..4], b"CWAR", "not compressed, as the files of the game");
        assert!(written.len() <= sango_cry::MAX_ARCHIVE_BYTES, "{} bytes", written.len());
        archive::read_archive(&written).unwrap();
        // The list that the plugin reads gives the file and its size.
        let list = sd.read("species/cry/index.bin").unwrap();
        let words: Vec<u32> = list.chunks_exact(4).map(|word| u32::from_le_bytes(word.try_into().unwrap())).collect();
        assert_eq!(words, [1, 5000, written.len() as u32]);

        // Reset: the button under Import.
        press(&[ArrowDown, ArrowDown, ArrowDown, Enter], |ui| tab.ui(ui, &session, 724, 0));
        assert!(sd.read(&cry_path(5000)).is_none(), "{:?}", tab.status);
        let _ = std::fs::remove_dir_all(test_folder());
    }
}
