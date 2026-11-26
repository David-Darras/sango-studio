//! The sound effects of the page Music: a list of the sounds of the archive, and the waves that each one plays.
//!
//! A sound effect is a sequence that plays waves of 4 bits. The page plays a wave, exports it as a WAV or an MP3 file, and replaces it
//! by a WAV or an MP3 file. A replaced wave is written in place in a copy of the sound archive on the SD card
//! (`sango/sound/sango_sound.bcsar`): the plugin reads that copy in place of the archive of the game. The copy has the size of the
//! archive of the game, so a new wave must fit in the room of the old one: a longer sound is cut.

use crate::bcsar::{Archive, Kind};
use crate::bcstm::Music;
use crate::page::{play, Outcome};
use crate::waves::{self, WaveSlot};
use crate::{ids, wav};
use sango_core::SdCard;
use sango_cry::Sound;
use sango_ui::history::{Step, Steps};
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The rate above which an imported wave is brought down (the rate of the sound of the 3DS).
const MAX_RATE: u32 = 32728;

/// A sound effect of the list.
#[derive(Clone, Debug)]
struct Entry {
    /// The id of the sound: the group in the high 16 bits, the number in the group in the low 16 bits.
    id: u32,
    /// The place of the sound in the archive.
    index: usize,
}

impl Entry {
    /// The text of the list: the name, then the number between parentheses (the number that the sound functions of the plugin take
    /// for the first bank of the sound effects, the whole id for the other ones).
    fn title(&self) -> String {
        if self.id >> 16 == ids::GROUP_EFFECT {
            format!("Sound effect ({})", self.id & 0xFFFF)
        } else {
            format!("Sound effect ({})", self.id)
        }
    }
}

/// A step of the history: a part of the sound archive that the game reads, as it was.
struct Replaced {
    /// The sound effect and its wave that the page shows.
    entry: usize,
    wave: usize,
    /// The place of the part in the archive, and its bytes.
    offset: usize,
    bytes: Vec<u8>,
}

/// The part of `before` that is not the same in `after` (two archives of the same size): its place and its old bytes.
fn replaced_part(before: &[u8], after: &[u8]) -> Option<(usize, Vec<u8>)> {
    let first = before.iter().zip(after).position(|(a, b)| a != b)?;
    let last = before.iter().zip(after).rposition(|(a, b)| a != b)?;
    Some((first, before[first..=last].to_vec()))
}

#[derive(Default)]
pub struct Effects {
    /// Undo and redo of the imports and the resets of the waves.
    steps: Steps<Replaced>,
    /// The sound effect, the wave and the archive of the running import, as they were when it started.
    before: Option<(usize, usize, Vec<u8>)>,
    /// The windows of the buttons Export and Reset.
    export_open: bool,
    reset_open: bool,
    folder: Option<PathBuf>,
    archive: Option<Archive>,
    /// The archive of the game.
    original: Vec<u8>,
    /// The archive that the game will read: the copy of the SD card, else the archive of the game.
    current: Vec<u8>,
    entries: Vec<Entry>,
    search: String,
    selected: Option<usize>,
    slots: Vec<WaveSlot>,
    waves: Vec<Result<Sound, String>>,
    /// True for a wave that is not the one of the game.
    changed: Vec<bool>,
    wave: usize,
    playing: Option<Arc<AtomicBool>>,
    job: Option<Receiver<Outcome>>,
    status: Option<(bool, String)>,
}

/// The copy of the archive on the SD card.
fn card_path(sd: &SdCard) -> PathBuf {
    sd.sound_archive()
}

fn as_music(sound: &Sound) -> Music {
    Music { rate: sound.rate, channels: vec![sound.samples.clone()], loop_start: None }
}

impl Effects {
    /// Reads the archive of the game and the list of the sound effects. Does nothing when the folder is already read.
    pub fn load(&mut self, folder: &Path, sd: &SdCard) {
        if self.folder.as_deref() == Some(folder) {
            return;
        }
        self.folder = Some(folder.to_path_buf());
        self.steps.clear();
        self.entries.clear();
        self.selected = None;
        self.archive = None;
        match std::fs::read(folder.join("sango_sound.bcsar")).map_err(|e| e.to_string()).and_then(|bytes| Archive::parse(&bytes).map(|archive| (bytes, archive))) {
            Ok((bytes, archive)) => {
                self.entries = ids::EFFECTS
                    .iter()
                    .filter(|(index, _)| archive.sounds.get(*index as usize).is_some_and(|s| s.kind == Kind::Sequence))
                    .map(|&(index, id)| Entry { id, index: index as usize })
                    .collect();
                self.current = std::fs::read(card_path(sd)).ok().filter(|c| c.len() == bytes.len()).unwrap_or_else(|| bytes.clone());
                self.original = bytes;
                self.archive = Some(archive);
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Forgets the steps of the history (another SD card).
    pub(crate) fn forget_steps(&mut self) {
        self.steps.clear();
    }

    /// An import or a reset of a wave changed the archive that was `before`: a new step.
    fn remember(&mut self, entry: usize, wave: usize, before: &[u8]) {
        if let Some((offset, bytes)) = replaced_part(before, &self.current) {
            self.steps.done(Replaced { entry, wave, offset, bytes });
        }
    }

    /// Writes the copy of the archive on the SD card. A copy that is the same as the archive of the game is removed.
    fn write_copy(&self, sd: &SdCard, copy: &[u8]) -> Result<(), String> {
        crate::page::put_file(&card_path(sd), (copy != self.original).then_some(copy))
    }

    /// Undo or redo of an import or of a reset: the part of the archive goes back, and its wave is shown.
    fn step(&mut self, step: Step, sd: &SdCard) {
        self.stop();
        let mut steps = std::mem::take(&mut self.steps);
        let mut shown = None;
        let done = steps.apply(step, |kept| {
            let end = kept.offset + kept.bytes.len();
            let now = self.current.get(kept.offset..end).ok_or("The sound archive changed: this step cannot be done.")?.to_vec();
            let mut copy = self.current.clone();
            copy[kept.offset..end].copy_from_slice(&kept.bytes);
            self.write_copy(sd, &copy)?;
            shown = Some((kept.entry, kept.wave));
            Ok(Replaced { entry: kept.entry, wave: kept.wave, offset: kept.offset, bytes: now })
        });
        self.steps = steps;
        match done {
            Some(Ok(())) => {
                if let Some((entry, wave)) = shown {
                    (self.selected, self.wave) = (Some(entry), wave);
                    self.refresh(sd);
                    let way = if step == Step::Undo { "Undone" } else { "Done again" };
                    self.status = Some((true, format!("{way}. Restart the game.")));
                }
            }
            Some(Err(message)) => self.status = Some((false, message)),
            None => {}
        }
    }

    fn stop(&mut self) {
        if let Some(flag) = self.playing.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    fn is_playing(&self) -> bool {
        self.playing.as_ref().is_some_and(|flag: &Arc<AtomicBool>| !flag.load(Ordering::Relaxed))
    }

    /// Reads the waves of a sound effect from the archive that the game will read.
    fn select(&mut self, entry: usize) {
        self.stop();
        self.selected = Some(entry);
        self.wave = 0;
        self.slots.clear();
        self.waves.clear();
        self.changed.clear();
        let (Some(archive), Some(sound)) = (&self.archive, self.archive.as_ref().and_then(|a| a.sounds.get(self.entries[entry].index))) else { return };
        match waves::waves_of(&self.current, archive, sound) {
            Ok(slots) => {
                self.waves = slots.iter().map(|slot| waves::read(&self.current, slot)).collect();
                self.changed = slots.iter().map(|slot| self.current[slot.offset..slot.offset + slot.size] != self.original[slot.offset..slot.offset + slot.size]).collect();
                self.slots = slots;
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Reads the archive of the SD card again after a change.
    fn refresh(&mut self, sd: &SdCard) {
        self.current = std::fs::read(card_path(sd)).ok().filter(|c| c.len() == self.original.len()).unwrap_or_else(|| self.original.clone());
        if let Some(entry) = self.selected {
            let wave = self.wave;
            let status = self.status.take();
            self.select(entry);
            self.wave = wave.min(self.slots.len().saturating_sub(1));
            self.status = status;
        }
    }

    fn start_import(&mut self, path: PathBuf, sd: &SdCard) {
        let Some(slot) = self.slots.get(self.wave).cloned() else { return };
        let (Some(folder), target) = (self.folder.clone(), card_path(sd)) else { return };
        self.before = self.selected.map(|entry| (entry, self.wave, self.current.clone()));
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let outcome = (|| -> Outcome {
                let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let imported = wav::read(&bytes)?;
                let mono = crate::bcstm::with_channels(&imported, 1);
                let music = if mono.rate > MAX_RATE { wav::resample(&mono, MAX_RATE) } else { mono };
                let sound = Sound { rate: music.rate, samples: music.channels[0].clone() };
                let mut archive = std::fs::read(&target).ok().filter(|c| c.len() == slot_archive_size(&folder)).map_or_else(|| std::fs::read(folder.join("sango_sound.bcsar")).map_err(|e| e.to_string()), Ok)?;
                let written = waves::replace(&mut archive, &slot, &sound)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                std::fs::write(&target, &archive).map_err(|e| format!("{}: {e}", target.display()))?;
                let cut = if written < sound.samples.len() { format!(" It was cut to {:.2} s to fit the room of the old wave.", written as f32 / sound.rate as f32) } else { String::new() };
                Ok(format!("The wave was replaced ({:.2} s).{cut} Restart the game to hear it.", written as f32 / sound.rate as f32))
            })();
            let _ = sender.send(outcome);
        });
        self.job = Some(receiver);
        self.status = None;
    }

    /// Puts the wave of the game back in the copy of the archive. The copy is removed when nothing else is changed.
    fn reset(&mut self, sd: &SdCard) {
        let Some(slot) = self.slots.get(self.wave).cloned() else { return };
        let before = self.current.clone();
        let mut copy = self.current.clone();
        copy[slot.offset..slot.offset + slot.size].copy_from_slice(&self.original[slot.offset..slot.offset + slot.size]);
        self.status = Some(match self.write_copy(sd, &copy) {
            Ok(()) => (true, "The wave of the game is back. Restart the game.".to_string()),
            Err(message) => (false, message),
        });
        self.refresh(sd);
        if let Some(entry) = self.selected {
            self.remember(entry, self.wave, &before);
        }
    }

    fn export(&mut self, mp3: bool) {
        let Some(Ok(sound)) = self.waves.get(self.wave) else { return };
        let Some(entry) = self.selected.and_then(|i| self.entries.get(i)) else { return };
        let name = format!("sound_effect_{}_{}", entry.id & 0xFFFF, self.wave + 1);
        let (extension, label) = if mp3 { ("mp3", "MP3 sound") } else { ("wav", "WAV sound") };
        let Some(path) = rfd::FileDialog::new().add_filter(label, &[extension]).set_file_name(format!("{name}.{extension}")).save_file() else { return };
        let music = as_music(sound);
        let bytes = if mp3 { crate::mp3::write_mp3(&music) } else { Ok(wav::write_wav(&music)) };
        self.status = Some(match bytes.and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string())) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    pub fn list(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search").desired_width(220.0));
        ui.add_space(4.0);
        let needle = self.search.to_lowercase();
        let rows: Vec<usize> = (0..self.entries.len()).filter(|&i| needle.is_empty() || self.entries[i].title().to_lowercase().contains(&needle)).collect();
        let mut chosen = None;
        egui::ScrollArea::vertical().id_salt("effects_list").max_height(560.0).auto_shrink([false, true]).show_rows(ui, 24.0, rows.len(), |ui, range| {
            for &i in &rows[range] {
                if ui.selectable_label(self.selected == Some(i), self.entries[i].title()).clicked() {
                    chosen = Some(i);
                }
            }
        });
        if let Some(i) = chosen {
            self.select(i);
        }
        let _ = sd;
    }

    pub fn detail(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        // The result of an import.
        if let Some(receiver) = &self.job {
            match receiver.try_recv() {
                Ok(outcome) => {
                    self.job = None;
                    let status = match outcome {
                        Ok(message) => (true, message),
                        Err(message) => (false, message),
                    };
                    self.refresh(sd);
                    if let Some((entry, wave, before)) = self.before.take().filter(|_| status.0) {
                        self.remember(entry, wave, &before);
                    }
                    self.status = Some(status);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.job = None,
            }
        }
        let Some(entry) = self.selected.and_then(|i| self.entries.get(i)).cloned() else {
            ui.label(widget::hint("Choose a sound effect in the list."));
            return;
        };
        let busy = self.job.is_some();
        widget::card(ui, |ui| {
            ui.label(widget::title(&entry.title()));
            if self.slots.is_empty() {
                ui.label(widget::hint("This sound plays no wave."));
                return;
            }
            ui.label(widget::hint(&format!("{} wave(s). A wave can be used by other sounds: a change shows for all of them.", self.slots.len())));
            ui.add_space(4.0);
            for i in 0..self.slots.len() {
                ui.horizontal(|ui| {
                    let text = match &self.waves[i] {
                        Ok(sound) => format!("Wave {}: {:.2} s, {} Hz", i + 1, sound.samples.len() as f32 / sound.rate.max(1) as f32, sound.rate),
                        Err(message) => format!("Wave {}: {message}", i + 1),
                    };
                    if ui.selectable_label(self.wave == i, text).clicked() {
                        self.wave = i;
                    }
                    if self.changed[i] {
                        widget::badge(ui, "Replaced", color::TEAL);
                    }
                });
            }
            ui.add_space(8.0);
            let selected = self.wave.min(self.slots.len() - 1);
            self.wave = selected;
            ui.horizontal(|ui| {
                if self.is_playing() {
                    if widget::button(ui, "Stop") {
                        self.stop();
                    }
                } else if widget::button_styled(ui, egui::RichText::new("Play").color(color::WHITE), Some(color::ACCENT)) {
                    if let Some(Ok(sound)) = self.waves.get(selected) {
                        self.playing = Some(play(&as_music(sound)));
                    }
                }
                ui.separator();
                let changed = self.changed.get(selected).copied().unwrap_or(false);
                // The archive is written at once by an import: there is no Save.
                let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(true), import: Some(!busy), reset: Some(changed && !busy), ..Default::default() });
                self.export_open |= clicks.export;
                if clicks.import {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Sound", &["wav", "mp3"]).pick_file() {
                        self.start_import(path, sd);
                    }
                }
                self.reset_open |= clicks.reset;
                if let Some(step) = self.steps.asked(ui, !busy) {
                    self.step(step, sd);
                }
                if changed {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            let room = waves::room_samples(self.slots[selected].size) as f32 / self.waves.get(selected).and_then(|w| w.as_ref().ok()).map_or(32000.0, |w| w.rate as f32);
            ui.label(widget::hint(&format!("An imported sound is made mono and cut at {room:.2} s.")));
            let ctx = ui.ctx().clone();
            if self.export_open {
                let (mut open, mut wav, mut mp3, mut close) = (true, false, false, false);
                egui::Window::new("Export").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        wav = widget::button(ui, "WAV file");
                        mp3 = widget::button(ui, "MP3 file");
                        close = widget::button(ui, "Cancel");
                    });
                });
                if wav || mp3 {
                    self.export(mp3);
                }
                if wav || mp3 || close || !open {
                    self.export_open = false;
                }
            }
            if widget::reset_window(&ctx, &mut self.reset_open, "The wave of the sound", &mut []) {
                self.reset(sd);
            }
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(widget::hint("Writing the sound archive…"));
                });
            }
        });
        if let Some((ok, message)) = &self.status {
            ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
        }
        if self.is_playing() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

/// The size of the archive of the game: a copy of the SD card with another size is not used.
fn slot_archive_size(folder: &Path) -> usize {
    std::fs::metadata(folder.join("sango_sound.bcsar")).map_or(0, |m| m.len() as usize)
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game (`SANGO_DUMP`): a wave of a sound effect is replaced in a copy of the archive on the SD card, the copy
    /// has the size of the archive of the game, and putting the wave back removes the copy.
    #[test]
    fn a_wave_is_replaced_in_a_copy_of_the_archive_and_comes_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_effects_{}", std::process::id()));
        std::fs::create_dir_all(folder.join("sdmc")).unwrap();
        let sd = SdCard::open(folder.join("sdmc").to_str().unwrap()).unwrap();
        let sound_folder = Path::new(&root).join("sound");
        let mut effects = Effects::default();
        effects.load(&sound_folder, &sd);
        let message = effects.entries.iter().position(|e| e.title() == "Sound effect (0)").expect("the sound effect is listed with its number");
        effects.select(message);
        assert!(!effects.slots.is_empty() && matches!(effects.waves[0], Ok(_)));
        assert!(!effects.changed[0]);

        // The page draws.
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0)));
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                effects.list(ui, &sd);
                effects.detail(ui, &sd);
            });
        });

        // A WAV file replaces the wave.
        let tone = Music { rate: 22050, channels: vec![(0..8000).map(|i| ((i as f32 * 0.08).sin() * 9000.0) as i16).collect()], loop_start: None };
        let wav_path = folder.join("tone.wav");
        std::fs::write(&wav_path, wav::write_wav(&tone)).unwrap();
        effects.start_import(wav_path, &sd);
        let outcome = effects.job.take().unwrap().recv_timeout(std::time::Duration::from_secs(60)).unwrap().unwrap();
        assert!(outcome.contains("was replaced"), "{outcome}");
        effects.refresh(&sd);
        assert!(effects.changed[0]);
        let copy = std::fs::read(card_path(&sd)).unwrap();
        assert_eq!(copy.len(), effects.original.len());
        assert_eq!(waves::read(&copy, &effects.slots[0]).unwrap().rate, 22050);

        let (entry, wave, before) = effects.before.take().unwrap();
        effects.remember(entry, wave, &before);
        effects.step(Step::Undo, &sd);
        assert!(!effects.changed[0] && !card_path(&sd).exists(), "the undo of the import");
        effects.step(Step::Redo, &sd);
        assert!(effects.changed[0]);
        assert_eq!(std::fs::read(card_path(&sd)).unwrap(), copy, "the redo of the import");

        effects.reset(&sd);
        assert!(!effects.changed[0]);
        assert!(!card_path(&sd).exists(), "nothing else is changed: the copy goes away");
        effects.step(Step::Undo, &sd);
        assert!(effects.changed[0], "the undo of the reset");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
