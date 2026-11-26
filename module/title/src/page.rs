//! The Title Screen page: its pictures, its Pokémon, and the timeline of the sequence of each screen.

use crate::commands::{self, Kind, COMMANDS, CONDITIONS};
use crate::pokemon;
use crate::sequence::{Sequence, Step};
use crate::store::{self, Holder, Picture, Store, VERSIONS};
use sango_core::{Dump, SdCard};
use sango_ui::history::{History, Steps};
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module, Session};
use std::path::PathBuf;

const TABS: [&str; 4] = ["Pictures", "Pokémon", "Top screen", "Bottom screen"];
const PICTURES: usize = 0;
const POKEMON: usize = 1;
/// The frame where each sequence goes when the player presses a button on the title screen: the game has these numbers.
const BUTTON_FRAMES: [i32; 2] = [3030, 3450];
const LABEL_WIDTH: f32 = 330.0;
const ROW: f32 = 22.0;
/// How near to an end of a bar the mouse takes this end.
const HANDLE: f32 = 7.0;

/// The files that an action on the pictures replaced: their places on the card and their bytes (`None`: no file).
type Replaced = Vec<(String, Option<Vec<u8>>)>;

/// What a drag on the timeline moves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Start,
    End,
    Both,
}

#[derive(Clone, Copy)]
struct Drag {
    place: usize,
    part: Part,
    from: f32,
    start: i32,
    end: i32,
}

pub struct TitlePage {
    opened_for: Option<(PathBuf, PathBuf)>,
    store: Option<Store>,
    error: Option<String>,
    tab: usize,
    /// The title screen that the page shows (a place of `VERSIONS`).
    version: usize,
    /// The title screen that the user asked for while the open one has changes: the window asks first.
    pending: Option<usize>,
    /// The sequences of the two screens as the page has them, and as they are on the card.
    doc: Option<[Sequence; 2]>,
    saved: Option<[Sequence; 2]>,
    language: usize,
    /// The place of the chosen picture in the list of the store.
    picture: usize,
    /// The picture on the screen: its place, the revision of the pictures, its texture.
    shown: Option<(usize, u64, egui::TextureHandle)>,
    revision: u64,
    /// The pictures that the SD card changes, for a revision of the pictures.
    changed: (u64, Vec<bool>),
    /// The number of the last frame that drew the page: after another page, the card is read again.
    last_frame: u64,
    selected: [Option<usize>; 2],
    zoom: f32,
    new_kind: u16,
    drag: Option<Drag>,
    status: Option<(bool, String)>,
    reset_open: bool,
    /// What the button Reset puts back: the chosen picture, all the pictures, the top screen, the bottom screen.
    reset_parts: [bool; 4],
    history: History<Option<[Sequence; 2]>>,
    steps: Steps<Replaced>,
}

impl Default for TitlePage {
    fn default() -> Self {
        TitlePage { opened_for: None, store: None, error: None, tab: PICTURES, version: 0, pending: None, doc: None, saved: None, language: 2, picture: 0, shown: None, revision: 0, changed: (u64::MAX, Vec::new()), last_frame: 0, selected: [None; 2], zoom: 0.3, new_kind: 28, drag: None, status: None, reset_open: false, reset_parts: [false; 4], history: History::default(), steps: Steps::default() }
    }
}

/// The condition of the logo for a language of the menu (both kinds of Japanese have the same logo).
fn condition_of(language: usize) -> u16 {
    [4, 4, 5, 6, 7, 8, 9, 10][language.min(7)]
}

fn pokemon_name(species: usize, form: usize) -> String {
    sango_species_base::species_form_name(species, form)
}

/// The name of a step on the timeline: its type, and the thing that it acts on.
fn step_label(step: &Step) -> String {
    let name = commands::command(step.kind).map_or_else(|| format!("Step of type {}", step.kind), |command| command.name.to_string());
    let detail = match step.kind {
        commands::POKEMON_SHOW | commands::POKEMON_SHOW_VIDEO => pokemon_name(step.int(0).max(0) as usize, step.int(1).max(0) as usize),
        commands::MODEL_SHOW => format!("file {}", step.int(0)),
        commands::LAYOUT_SHOW => CONDITIONS.iter().find(|(number, _)| *number == step.condition).map_or_else(String::new, |(_, language)| language.to_string()),
        _ => String::new(),
    };
    let name = if detail.is_empty() { name } else { format!("{name} ({detail})") };
    if step.group == 0 {
        name
    } else {
        format!("{name}  · {}", step.group)
    }
}

/// The colour of a step, by the first word of its name.
fn step_color(kind: u16) -> egui::Color32 {
    let name = commands::command(kind).map_or("", |command| command.name);
    match name.split(':').next().unwrap_or("") {
        "Pokémon" => color::ACCENT,
        "Camera" | "Light" | "3D" => egui::Color32::from_rgb(14, 165, 233),
        "Particles" => color::AMBER,
        "Model" => egui::Color32::from_rgb(168, 85, 247),
        "Music" | "Sound effect" => color::TEAL,
        "Logo and menus" | "Screen" => egui::Color32::from_rgb(236, 72, 153),
        _ => color::MUTED,
    }
}

/// A menu of numbers with their names, which the arrows change.
fn number_menu(ui: &mut egui::Ui, id: impl std::hash::Hash, value: &mut i32, entries: &[(i32, &str)], width: f32) {
    let shown = entries.iter().find(|(number, _)| number == value).map_or_else(|| format!("Value {value}"), |(_, name)| name.to_string());
    let combo = egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).height(360.0).show_ui(ui, |ui| {
        for (number, name) in entries {
            ui.selectable_value(value, *number, *name);
        }
    });
    let values: Vec<i32> = entries.iter().map(|(number, _)| *number).collect();
    focus::combo_arrows(ui, &combo.response, value, &values);
}

/// The widgets of one value of a step, at its place (in values of 4 bytes).
fn value_widget(ui: &mut egui::Ui, step: &mut Step, place: usize, kind: Kind, id: (usize, usize)) {
    match kind {
        Kind::Int => {
            let mut value = step.int(place);
            widget::spin(ui, &mut value, -1.0, 1_000_000.0);
            step.set_int(place, value);
        }
        Kind::Bool => {
            let mut on = step.int(place) != 0;
            if widget::checkbox(ui, &mut on, "").changed() {
                step.set_int(place, i32::from(on));
            }
        }
        Kind::Float => {
            let mut value = step.float(place);
            if widget::decimal(ui, &mut value, 0.05, f32::MIN..=f32::MAX).changed() {
                step.set_float(place, value);
            }
        }
        Kind::Vec3 => {
            ui.horizontal(|ui| {
                for (axis, name) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(widget::hint(name));
                    let mut value = step.float(place + axis);
                    if widget::decimal(ui, &mut value, 0.1, f32::MIN..=f32::MAX).changed() {
                        step.set_float(place + axis, value);
                    }
                }
            });
        }
        Kind::Choice(entries) => {
            let mut value = step.int(place);
            number_menu(ui, ("title_value", id), &mut value, entries, 240.0);
            step.set_int(place, value);
        }
        Kind::Locked(note) => {
            let value = step.int(place);
            let music = (value as u32 >> 16 == sango_music::ids::GROUP_MUSIC).then(|| sango_music::names::MUSIC_NAMES.get(value as usize & 0xFFFF)).flatten();
            ui.label(music.map_or_else(|| value.to_string(), |name| format!("{name} ({value})"))).on_hover_text(note);
        }
    }
}

impl TitlePage {
    fn open(&mut self, dump: &Dump, sd: Option<&SdCard>) {
        self.shown = None;
        self.status = None;
        self.pending = None;
        self.picture = 0;
        self.revision += 1;
        self.steps.clear();
        match Store::open(dump) {
            Ok(store) => {
                self.store = Some(store);
                self.error = None;
                self.load(sd);
            }
            Err(message) => (self.store, self.error, self.doc, self.saved) = (None, Some(message), None, None),
        }
    }

    fn load(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        self.history.clear();
        self.selected = [None; 2];
        let version = self.version;
        match store.sequence(sd, version, 0).and_then(|top| Ok([top, store.sequence(sd, version, 1)?])) {
            Ok(doc) => (self.doc, self.saved) = (Some(doc.clone()), Some(doc)),
            Err(message) => {
                // A file of the card that cannot be read: the sequences of the game show, and the Reset button removes the file.
                self.status = Some((false, message));
                (self.doc, self.saved) = (Some(store.game[version].clone()), Some(store.game[version].clone()));
            }
        }
    }

    fn dirty(&self) -> bool {
        self.doc != self.saved
    }

    fn save(&mut self, sd: &SdCard) {
        let (Some(store), Some(doc)) = (&self.store, &self.doc) else { return };
        let result = (0..2).try_for_each(|screen| store.save_sequence(sd, self.version, screen, &doc[screen]));
        match result {
            Ok(()) => {
                self.saved = self.doc.clone();
                self.status = Some((true, "Saved. Restart the game to see the change.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The places of the pictures that the list shows: the logo of the language of the menu, then the pictures of the 3D scene.
    fn listed(&self) -> Vec<usize> {
        let Some(store) = &self.store else { return Vec::new() };
        let condition = condition_of(self.language);
        let shown = |p: &Picture| p.version == self.version && (p.holder == Holder::Model || p.holder == Holder::Logo(condition));
        store.pictures.iter().enumerate().filter(|(_, p)| shown(p)).map(|(place, _)| place).collect()
    }

    /// Does a change of picture files and keeps what it replaced, for the undo.
    fn change_pictures(&mut self, sd: &SdCard, paths: Vec<String>, change: impl FnOnce(&Store) -> Result<(), String>) -> Result<(), String> {
        let Some(store) = &self.store else { return Ok(()) };
        let before: Replaced = paths.into_iter().map(|path| (path.clone(), sd.read(&path))).collect();
        let result = change(store);
        if before.iter().any(|(path, bytes)| sd.read(path) != *bytes) {
            self.steps.done(before);
            self.revision += 1;
        }
        result
    }

    fn export(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        let result = if self.tab == PICTURES {
            let Some(picture) = store.pictures.get(self.picture) else { return };
            let Some(path) = rfd::FileDialog::new().set_title("Export the picture").set_file_name(format!("{}.png", picture.label.replace([':', '/'], ""))).add_filter("PNG", &["png"]).save_file() else { return };
            store.picture(sd, picture).and_then(|(width, height, rgba)| {
                let image = image::RgbaImage::from_raw(width as u32, height as u32, rgba).ok_or("The picture is not complete.")?;
                image.save(&path).map_err(|e| e.to_string())?;
                Ok(path)
            })
        } else {
            let screen = self.tab - 2;
            let Some(doc) = &self.doc else { return };
            let Some(path) = rfd::FileDialog::new().set_title("Export the sequence").set_file_name(format!("title_{}.bin", ["top", "bottom"][screen])).add_filter("Sequence", &["bin"]).save_file() else { return };
            std::fs::write(&path, doc[screen].to_bytes()).map(|()| path).map_err(|e| e.to_string())
        };
        self.status = Some(match result {
            Ok(path) => (true, format!("Exported to {}.", path.display())),
            Err(message) => (false, message),
        });
    }

    fn import(&mut self, sd: &SdCard) {
        if self.tab == PICTURES {
            let Some(picture) = self.store.as_ref().and_then(|s| s.pictures.get(self.picture)).cloned() else { return };
            let Some(path) = rfd::FileDialog::new().set_title("Import a picture").add_filter("PNG", &["png"]).pick_file() else { return };
            let result = image::open(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|image| {
                let image = image.to_rgba8();
                self.change_pictures(sd, vec![picture.path()], |store| store.set_picture(sd, &picture, image.width() as usize, image.height() as usize, image.as_raw()))
            });
            self.status = Some(match result {
                Ok(()) => (true, "The picture is replaced. Restart the game to see it.".to_string()),
                Err(message) => (false, message),
            });
        } else {
            let screen = self.tab - 2;
            let Some(path) = rfd::FileDialog::new().set_title("Import a sequence").add_filter("Sequence", &["bin"]).pick_file() else { return };
            match std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|bytes| Sequence::parse(&bytes)) {
                Ok(sequence) => {
                    if let Some(doc) = &mut self.doc {
                        doc[screen] = sequence;
                        self.selected[screen] = None;
                        self.status = Some((true, "Imported. Save to put it on the SD card.".to_string()));
                    }
                }
                Err(message) => self.status = Some((false, message)),
            }
        }
    }

    /// Writes every picture of the open title screen in a folder that the user chooses.
    fn export_all(&mut self, sd: Option<&SdCard>) {
        let Some(store) = &self.store else { return };
        let Some(folder) = rfd::FileDialog::new().set_title("Folder for the pictures").pick_folder() else { return };
        self.status = Some(match store.export_pictures(sd, self.version, &folder) {
            Ok(count) => (true, format!("{count} pictures written in {}.", folder.display())),
            Err(message) => (false, message),
        });
    }

    /// Reads the pictures of a folder that the user chooses. One step of the undo takes all of them back.
    fn import_all(&mut self, sd: &SdCard) {
        let Some(folder) = rfd::FileDialog::new().set_title("Folder of the pictures").pick_folder() else { return };
        let version = self.version;
        let Some(store) = &self.store else { return };
        let mut paths: Vec<String> = store.pictures.iter().filter(|p| p.version == version).map(Picture::path).collect();
        paths.dedup();
        let mut count = 0;
        let result = self.change_pictures(sd, paths, |store| {
            count = store.import_pictures(sd, version, &folder)?;
            Ok(())
        });
        self.status = Some(match result {
            Ok(()) if count == 0 => (true, "The pictures of the folder are the ones that the title screen has already.".to_string()),
            Ok(()) => (true, format!("{count} pictures replaced. Restart the game to see them.")),
            Err(message) => (false, message),
        });
    }

    /// Puts the checked parts back to the game.
    fn reset(&mut self, sd: &SdCard) {
        let [chosen, pictures, top, bottom] = self.reset_parts;
        if chosen && !pictures {
            let Some(picture) = self.store.as_ref().and_then(|s| s.pictures.get(self.picture)).cloned() else { return };
            if let Err(message) = self.change_pictures(sd, vec![picture.path()], |store| store.reset_picture(sd, &picture)) {
                self.status = Some((false, message));
                return;
            }
        }
        if pictures {
            let version = self.version;
            let Some(store) = &self.store else { return };
            let mut paths: Vec<String> = store.pictures.iter().filter(|p| p.version == version).map(Picture::path).collect();
            paths.dedup();
            let result = self.change_pictures(sd, paths, |store| store.reset_pictures(sd, version));
            if let Err(message) = result {
                self.status = Some((false, message));
                return;
            }
        }
        if let (Some(store), Some(doc)) = (&self.store, &mut self.doc) {
            for (screen, checked) in [top, bottom].into_iter().enumerate() {
                if checked {
                    doc[screen] = store.game[self.version][screen].clone();
                    self.selected[screen] = None;
                }
            }
        }
        if top || bottom {
            self.save(sd);
        }
        self.status = Some((true, "The title screen of the game is back for these parts. Restart the game to see it.".to_string()));
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let dirty = self.dirty();
        let mut clicks = widget::ActionClicks::default();
        if self.changed.0 != self.revision {
            if let Some(store) = &self.store {
                self.changed = (self.revision, store.changed_pictures(sd));
            }
        }
        let changed = self.store.as_ref().is_some_and(|store| match self.tab {
            PICTURES => self.changed.1.get(self.picture) == Some(&true),
            POKEMON => store.sequence_changed(sd, self.version, 1),
            tab => store.sequence_changed(sd, self.version, tab - 2),
        });
        let mut version = self.version;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Title screen of")).on_hover_text("Features page, Title screen of the plugin. On: the menu of the plugin chooses the one that plays (Omega Ruby at first). Off: Alpha Sapphire.");
                let mut chosen = version as i32;
                number_menu(ui, "title_version", &mut chosen, &[(0, VERSIONS[0]), (1, VERSIONS[1])], 180.0);
                version = chosen as usize;
                let files = self.tab != POKEMON;
                let card = sd.is_some() && self.store.is_some();
                let (export, import) = match self.tab {
                    PICTURES => ("Writes the chosen picture in a PNG file.", "Replaces the chosen picture by a PNG file of the same size."),
                    _ => ("Writes the sequence of this screen in a file.", "Replaces the sequence of this screen by a file."),
                };
                clicks = widget::action_bar(
                    ui,
                    widget::ActionBar {
                        export: files.then_some(self.store.is_some()),
                        import: files.then_some(card),
                        reset: Some(card),
                        save: Some(card && dirty),
                        notes: widget::ActionNotes { export, import, reset: "Puts the title screen back to the game: choose the parts in the window.", ..Default::default() },
                        ..Default::default()
                    },
                );
                if self.tab == PICTURES {
                    // The pictures go on the card at once: their undo writes the files again.
                    if let (Some(step), Some(sd)) = (self.steps.asked(ui, sd.is_some()), sd) {
                        let done = self.steps.apply(step, |kept| {
                            let replaced: Replaced = kept.iter().map(|(path, _)| (path.clone(), sd.read(path))).collect();
                            kept.iter().try_for_each(|(path, bytes)| store::put(sd, path, bytes.as_deref()))?;
                            Ok(replaced)
                        });
                        if let Some(result) = done {
                            self.revision += 1;
                            self.status = result.err().map(|message| (false, message));
                        }
                    }
                } else {
                    self.history.buttons(ui, self.doc.is_some());
                }
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if changed {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can look and export, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        if version != self.version {
            if dirty {
                self.pending = Some(version);
            } else {
                self.show_version(sd, version);
            }
        }
        if let Some(wanted) = self.pending {
            match widget::unsaved_window(ui.ctx(), &format!("the title screen of {}", VERSIONS[self.version])) {
                Some(widget::Unsaved::Save) => {
                    if let Some(sd) = sd {
                        self.save(sd);
                    }
                    if !self.dirty() {
                        self.show_version(sd, wanted);
                    }
                    self.pending = None;
                }
                Some(widget::Unsaved::Discard) => {
                    self.show_version(sd, wanted);
                    self.pending = None;
                }
                Some(widget::Unsaved::Stay) => self.pending = None,
                None => {}
            }
        }
        if clicks.export {
            self.export(sd);
        }
        if let (true, Some(sd)) = (clicks.import, sd) {
            self.import(sd);
        }
        if clicks.reset {
            // What the open tab shows is checked first.
            self.reset_parts = [self.tab == PICTURES, false, self.tab == 2, self.tab == POKEMON || self.tab == 3];
            self.reset_open = true;
        }
        if let (true, Some(sd)) = (clicks.save, sd) {
            self.save(sd);
        }
        let [chosen, pictures, top, bottom] = &mut self.reset_parts;
        if widget::reset_window(ui.ctx(), &mut self.reset_open, "The title screen", &mut [("Chosen picture", chosen), ("All the pictures", pictures), ("Top screen", top), ("Bottom screen and its Pokémon", bottom)]) {
            if let Some(sd) = sd {
                self.reset(sd);
            }
        }
    }

    /// Shows the other title screen: its sequences are read again.
    fn show_version(&mut self, sd: Option<&SdCard>, version: usize) {
        self.version = version;
        self.status = None;
        self.drag = None;
        // The pictures of the undo are the ones of both title screens: they stay.
        self.load(sd);
    }

    fn pictures_tab(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let (mut export_all, mut import_all) = (false, false);
        ui.horizontal_wrapped(|ui| {
            ui.label(widget::hint("Logo of the language"));
            sango_ui::language::menu(ui, "title_language", &mut self.language, None);
            ui.add_space(18.0);
            let (button, clicked) = widget::button_response(ui, "Export all the pictures", None);
            button.on_hover_text("Writes every picture of this title screen, in every language, as PNG files in a folder.");
            export_all = clicked;
            ui.add_enabled_ui(sd.is_some(), |ui| {
                let (button, clicked) = widget::button_response(ui, "Import the pictures of a folder", None);
                button.on_hover_text("Reads the PNG files that have the names of the export. Only the pictures that changed are replaced.");
                import_all = clicked;
            });
        });
        if export_all {
            self.export_all(sd);
        }
        if let (true, Some(sd)) = (import_all, sd) {
            self.import_all(sd);
        }
        let listed = self.listed();
        if !listed.contains(&self.picture) {
            self.picture = listed.first().copied().unwrap_or(0);
        }
        ui.add_space(6.0);
        let Some(store) = &self.store else { return };
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(330.0);
                egui::ScrollArea::vertical().id_salt("title_pictures").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                    let mut chosen = self.picture;
                    let mut logo = None;
                    let top = ui.cursor().min;
                    for &place in &listed {
                        let picture = &store.pictures[place];
                        let is_logo = picture.holder != Holder::Model;
                        if logo != Some(is_logo) {
                            logo = Some(is_logo);
                            ui.label(widget::title(if is_logo { "Logo" } else { "3D scene" }));
                        }
                        let mark = if self.changed.1.get(place) == Some(&true) { "  ●" } else { "" };
                        let row = ui.selectable_label(self.picture == place, format!("{}{mark}", picture.label));
                        widget::scroll_to_chosen(ui, &row, self.picture == place);
                        if row.clicked() {
                            chosen = place;
                        }
                    }
                    // A click in the list gives it the focus: then the arrows choose the picture.
                    let area = egui::Rect::from_min_max(top, egui::pos2(top.x + 330.0, ui.cursor().min.y));
                    let clicked = ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(area);
                    if focus::focus(ui, egui::Id::new("title_picture_list"), area.intersect(ui.clip_rect()), clicked, true, None) {
                        let at = listed.iter().position(|&place| place == chosen).unwrap_or(0);
                        if let Some(to) = focus::stepped(at, focus::steps(ui, false), listed.len()) {
                            chosen = listed[to];
                            ui.ctx().request_repaint();
                        }
                    }
                    self.picture = chosen;
                });
            });
            ui.vertical(|ui| {
                let Some(picture) = store.pictures.get(self.picture) else { return };
                if !self.shown.as_ref().is_some_and(|(place, revision, _)| (*place, *revision) == (self.picture, self.revision)) {
                    match store.picture(sd, picture) {
                        Ok((width, height, rgba)) => {
                            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
                            let texture = ui.ctx().load_texture("title_picture", image, egui::TextureOptions::NEAREST);
                            self.shown = Some((self.picture, self.revision, texture));
                        }
                        Err(message) => {
                            self.shown = None;
                            ui.label(egui::RichText::new(message).color(color::ROSE));
                        }
                    }
                }
                let Some((_, _, texture)) = &self.shown else { return };
                widget::card(ui, |ui| {
                    let [width, height] = texture.size();
                    ui.label(widget::title(&picture.label));
                    ui.label(widget::hint(&format!("{width} × {height} pixels")));
                    ui.add_space(6.0);
                    let size = egui::vec2(width as f32, height as f32);
                    let room = egui::vec2(ui.available_width().clamp(100.0, 640.0), 460.0);
                    // A very small picture is made large enough to see it.
                    let most = if size.max_elem() <= 32.0 { 192.0 / size.max_elem() } else { 3.0 };
                    let scale = (room.x / size.x).min(room.y / size.y).min(most);
                    let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
                    // Dark squares behind: the clear parts of a picture show.
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(70, 74, 90));
                    let side = 12.0;
                    for row in 0..(rect.height() / side).ceil() as usize {
                        for column in (row % 2..(rect.width() / side).ceil() as usize).step_by(2) {
                            let corner = rect.min + egui::vec2(column as f32 * side, row as f32 * side);
                            painter.rect_filled(egui::Rect::from_min_size(corner, egui::vec2(side, side)), 0.0, egui::Color32::from_rgb(86, 90, 108));
                        }
                    }
                    painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    // A white picture shows its edges.
                    ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0_f32, color::FIELD_BORDER));
                });
            });
        });
    }

    fn pokemon_tab(&mut self, ui: &mut egui::Ui) {
        let (Some(store), Some(doc)) = (&self.store, &mut self.doc) else { return };
        let game = &store.game[self.version][1];
        let notes = ["It opens the bottom screen, and shows when the player presses a button.", "It shows between the views of the first one."];
        for (at, group) in pokemon::groups(game).into_iter().enumerate() {
            let sequence = &mut doc[1];
            let (Some(now), Some(original)) = (pokemon::pokemon(sequence, group), pokemon::pokemon(game, group)) else { continue };
            widget::card(ui, |ui| {
                ui.set_width(680.0_f32.min(ui.available_width()));
                ui.label(widget::title(["First Pokémon", "Second Pokémon", "Other Pokémon"][at.min(2)]));
                ui.label(widget::hint(notes.get(at).copied().unwrap_or("")));
                ui.add_space(6.0);
                egui::Grid::new(("title_pokemon", group)).num_columns(2).spacing(egui::vec2(18.0, 8.0)).min_col_width(110.0).show(ui, |ui| {
                    let mut wanted = (now.0, now.1);
                    let mut gender = now.2;
                    let back = ui.horizontal(|ui| widget::field_name(ui, "Pokémon", (now.0, now.1) != (original.0, original.1)).1).inner;
                    sango_species::menus::pokemon_menu(ui, &format!("title_species_{group}"), &mut wanted, 320.0, None, None);
                    ui.end_row();
                    let gender_back = ui.horizontal(|ui| widget::field_name(ui, "Look", gender != original.2).1).inner;
                    number_menu(ui, ("title_gender", group), &mut gender, &[(0, "Male or no gender"), (1, "Female")], 180.0);
                    ui.end_row();
                    if back {
                        wanted = (original.0, original.1);
                    }
                    if gender_back {
                        gender = original.2;
                    }
                    if wanted.0 != 0 && (wanted.0, wanted.1, gender) != now {
                        let (size, place) = (pokemon::size(sequence, group), pokemon::place(sequence, group));
                        pokemon::set_pokemon(sequence, game, group, wanted.0, wanted.1, gender);
                        pokemon::set_size(sequence, game, group, size);
                        pokemon::set_place(sequence, game, group, place);
                        self.selected[1] = None;
                    }

                    let mut size = pokemon::size(sequence, group);
                    let size_before = size;
                    let back = ui.horizontal(|ui| widget::field_name(ui, "Size", size != pokemon::size(game, group)).1).inner;
                    widget::decimal(ui, &mut size, 0.05, 0.05..=20.0).on_hover_text("1: as the game makes it.");
                    ui.end_row();
                    if back {
                        size = pokemon::size(game, group);
                    }
                    if size != size_before {
                        pokemon::set_size(sequence, game, group, size);
                        self.selected[1] = None;
                    }

                    let mut place = pokemon::place(sequence, group);
                    let place_before = place;
                    let back = ui.horizontal(|ui| widget::field_name(ui, "Place", place != pokemon::place(game, group)).1).inner;
                    ui.horizontal(|ui| {
                        for (axis, value) in ["Left and right", "Height", "Depth"].iter().zip(place.iter_mut()) {
                            ui.label(widget::hint(axis));
                            widget::decimal(ui, value, 0.5, f32::MIN..=f32::MAX);
                        }
                    });
                    ui.end_row();
                    if back {
                        place = pokemon::place(game, group);
                    }
                    if place != place_before {
                        pokemon::set_place(sequence, game, group, place);
                        self.selected[1] = None;
                    }
                });
            });
            ui.add_space(10.0);
        }
        ui.label(widget::hint("The camera was made for the Pokémon of the game: use Size and Place for another one. The cry at the start is set in the menu of the plugin."));
    }

    fn add_bar(&mut self, ui: &mut egui::Ui, screen: usize) {
        ui.horizontal_wrapped(|ui| {
            let shown = commands::command(self.new_kind).map_or("?", |c| c.name);
            picker(ui, "title_new_kind", shown, 280.0, &mut self.new_kind, None, || vec![Group { title: String::new(), entries: COMMANDS.iter().map(|c| (c.number, c.name.to_string())).collect() }]);
            if widget::button_styled(ui, egui::RichText::new("+ Add a step").color(color::WHITE), Some(color::ACCENT)) {
                let Some(doc) = &mut self.doc else { return };
                let sequence = &mut doc[screen];
                let near = self.selected[screen].and_then(|p| sequence.steps.get(p)).map_or((0, 0), |s| (s.start, s.group));
                if let Some(step) = Step::new(self.new_kind, near.0, near.0, near.1) {
                    self.selected[screen] = Some(sequence.insert(step));
                }
            }
            ui.label(widget::hint("The new step takes the first frame and the object of the chosen step."));
        });
    }

    fn timeline(&mut self, ui: &mut egui::Ui, screen: usize, height: f32) {
        let Some(doc) = &mut self.doc else { return };
        let sequence = &mut doc[screen];
        let length = sequence.length().max(30);
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title("Timeline"));
                ui.label(widget::hint(&format!("{} steps, {length} frames ({:.0} s). Drag a bar or its ends.", sequence.steps.len(), length as f32 / 30.0)));
                ui.add(egui::Slider::new(&mut self.zoom, 0.08..=6.0).logarithmic(true).text("Zoom"));
            });
            ui.add_space(4.0);
            let zoom = self.zoom;
            let count = sequence.steps.len();
            egui::ScrollArea::both().id_salt(("title_timeline", screen)).max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH + length as f32 * zoom + 40.0, count as f32 * ROW + 20.0), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                let origin = rect.left() + LABEL_WIDTH;
                // A number each 10 seconds when the frames are close together, else each second.
                let mark = if zoom < 0.6 { 300 } else if zoom < 2.0 { 90 } else { 30 };
                for frame in (0..=length).step_by(mark) {
                    let x = origin + frame as f32 * zoom;
                    painter.line_segment([egui::pos2(x, rect.top() + 16.0), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0_f32, color::BORDER));
                    painter.text(egui::pos2(x + 2.0, rect.top()), egui::Align2::LEFT_TOP, frame.to_string(), egui::FontId::proportional(11.0), color::MUTED);
                }
                let button = origin + BUTTON_FRAMES[screen] as f32 * zoom;
                painter.line_segment([egui::pos2(button, rect.top() + 16.0), egui::pos2(button, rect.bottom())], egui::Stroke::new(2.0_f32, color::ROSE));
                let mut chosen = self.selected[screen];
                for place in 0..count {
                    let step = &sequence.steps[place];
                    let top = rect.top() + 20.0 + place as f32 * ROW;
                    let row = egui::Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(rect.width(), ROW));
                    let response = ui.interact(row, ui.id().with(("title_step", screen, place)), egui::Sense::click_and_drag());
                    let is_chosen = chosen == Some(place);
                    if is_chosen {
                        painter.rect_filled(row, 3.0, color::ACCENT_SOFT);
                    } else if response.hovered() {
                        painter.rect_filled(row, 3.0, color::SURFACE);
                    }
                    let label_zone = egui::Rect::from_min_size(row.min, egui::vec2(LABEL_WIDTH - 6.0, ROW));
                    painter.with_clip_rect(label_zone.intersect(painter.clip_rect())).text(egui::pos2(rect.left() + 4.0, top + ROW / 2.0), egui::Align2::LEFT_CENTER, step_label(step), egui::FontId::proportional(12.5), if is_chosen { color::ACCENT } else { color::TEXT });
                    let (x0, x1) = (origin + step.start as f32 * zoom, origin + step.end.max(step.start) as f32 * zoom);
                    let bar = egui::Rect::from_min_max(egui::pos2(x0, top + 4.0), egui::pos2(x1.max(x0 + 4.0), top + ROW - 4.0));
                    painter.rect_filled(bar, 3.0, step_color(step.kind).gamma_multiply(if step.condition != 0 { 0.6 } else { 1.0 }));
                    if let Some(pointer) = response.hover_pos().filter(|p| p.x >= bar.left() - HANDLE && p.x <= bar.right() + HANDLE) {
                        let end_of_bar = (pointer.x - bar.left()).abs() < HANDLE || (pointer.x - bar.right()).abs() < HANDLE;
                        ui.ctx().set_cursor_icon(if end_of_bar { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::Grab });
                    }
                    widget::scroll_to_chosen(ui, &response, is_chosen);
                    if response.clicked() {
                        chosen = Some(place);
                    }
                    // A drag that starts on the bar moves it; on one of its ends, this end.
                    if response.drag_started() {
                        if let Some(origin_of_drag) = ui.input(|i| i.pointer.press_origin()) {
                            let part = if (origin_of_drag.x - bar.right()).abs() < HANDLE {
                                Some(Part::End)
                            } else if (origin_of_drag.x - bar.left()).abs() < HANDLE {
                                Some(Part::Start)
                            } else {
                                bar.x_range().contains(origin_of_drag.x).then_some(Part::Both)
                            };
                            if let Some(part) = part {
                                self.drag = Some(Drag { place, part, from: origin_of_drag.x, start: step.start, end: step.end });
                                chosen = Some(place);
                            }
                        }
                    }
                }
                // A click in the timeline gives it the focus: then the arrows choose the step.
                let clicked = ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(rect);
                if focus::focus(ui, egui::Id::new(("title_step_list", screen)), rect.intersect(ui.clip_rect()), clicked, true, None) {
                    if let Some(to) = focus::stepped(chosen.unwrap_or(0), focus::steps(ui, false), count) {
                        chosen = Some(to);
                        ui.ctx().request_repaint();
                    }
                }
                self.selected[screen] = chosen;
            });
        });
        if let Some(drag) = self.drag {
            if let (Some(pointer), Some(step)) = (ui.input(|i| i.pointer.interact_pos()), sequence.steps.get_mut(drag.place)) {
                let moved = ((pointer.x - drag.from) / self.zoom).round() as i32;
                match drag.part {
                    Part::Start => step.start = (drag.start + moved).clamp(0, drag.end.max(0)),
                    Part::End => step.end = (drag.end + moved).max(drag.start),
                    Part::Both => {
                        let moved = moved.max(-drag.start);
                        (step.start, step.end) = (drag.start + moved, drag.end + moved);
                    }
                }
            }
            if !ui.input(|i| i.pointer.any_down()) {
                self.drag = None;
            }
        }
    }

    /// The values of the chosen step, and the buttons that copy and remove it.
    fn details(&mut self, ui: &mut egui::Ui, screen: usize) {
        let Some(doc) = &mut self.doc else { return };
        let sequence = &mut doc[screen];
        let (mut remove, mut duplicate) = (None, None);
        widget::card(ui, |ui| {
            let Some(place) = self.selected[screen].filter(|&p| p < sequence.steps.len()) else {
                ui.label(widget::hint("Choose a step on the timeline to change it, or add a new one."));
                return;
            };
            let step = &mut sequence.steps[place];
            let command = commands::command(step.kind);
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::title(command.map_or("Unknown step", |c| c.name)));
                if widget::button(ui, "Duplicate") {
                    duplicate = Some(place);
                }
                if widget::confirm_button(ui, "Remove", "Click again to remove") {
                    remove = Some(place);
                }
            });
            ui.add_space(4.0);
            egui::Grid::new("title_step_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("First frame");
                widget::spin(ui, &mut step.start, 0.0, 100_000.0);
                ui.end_row();
                ui.label("Last frame");
                widget::spin(ui, &mut step.end, -1.0, 100_000.0);
                ui.end_row();
                ui.label("Object").on_hover_text("The steps with the same number act on the same model, Pokémon or effect.");
                widget::spin(ui, &mut step.group, 0.0, 65_535.0);
                ui.end_row();
                ui.label("Plays when");
                let conditions: Vec<(i32, &str)> = CONDITIONS.iter().map(|&(number, name)| (i32::from(number), name)).collect();
                let mut condition = i32::from(step.condition);
                number_menu(ui, "title_condition", &mut condition, &conditions, 200.0);
                step.condition = condition as u16;
                ui.end_row();
                match command {
                    Some(command) => {
                        let mut at = 0;
                        for (index, &(name, kind)) in command.values.iter().enumerate() {
                            ui.label(name);
                            value_widget(ui, step, at, kind, (place, index));
                            ui.end_row();
                            at += kind.size() / 4;
                        }
                    }
                    None => {
                        for at in 0..step.data.len() / 4 {
                            ui.label(format!("Value {}", at + 1));
                            value_widget(ui, step, at, Kind::Int, (place, at));
                            ui.end_row();
                        }
                    }
                }
            });
        });
        if let Some(place) = remove {
            sequence.steps.remove(place);
            self.selected[screen] = None;
        }
        if let Some(place) = duplicate {
            let copy = sequence.steps[place].clone();
            sequence.steps.insert(place + 1, copy);
            self.selected[screen] = Some(place + 1);
        }
    }
}

impl Module for TitlePage {
    fn title(&self) -> &str {
        "Title Screen"
    }

    fn icon(&self) -> &str {
        "📺"
    }

    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn discard_changes(&mut self) -> bool {
        self.doc = self.saved.clone();
        self.selected = [None; 2];
        self.history.clear();
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Title Screen");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let sd = session.sd.clone();
        let now = (dump.root().to_path_buf(), sd.as_ref().map(|sd| sd.root().to_path_buf()).unwrap_or_default());
        if self.opened_for.as_ref() != Some(&now) {
            self.opened_for = Some(now);
            self.open(dump, sd.as_ref());
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        // Another page (Layout) can change the files of the logo.
        let frame = ui.ctx().cumulative_pass_nr();
        if frame > self.last_frame + 1 {
            self.revision += 1;
        }
        self.last_frame = frame;
        self.top_bar(ui, sd.as_ref());
        ui.add_space(8.0);
        if widget::tabs(ui, "title", &TABS, &mut self.tab) {
            self.drag = None;
        }
        ui.add_space(6.0);
        match self.tab {
            PICTURES => self.pictures_tab(ui, sd.as_ref()),
            POKEMON => {
                egui::ScrollArea::vertical().id_salt("title_pokemon_scroll").auto_shrink([false, false]).show(ui, |ui| self.pokemon_tab(ui));
            }
            tab => {
                // The timeline leaves room under it for the values of the chosen step.
                let height = (ui.available_height() - 420.0).clamp(200.0, 560.0);
                egui::ScrollArea::vertical().id_salt(("title_sequence_scroll", tab)).auto_shrink([false, false]).show(ui, |ui| {
                    self.add_bar(ui, tab - 2);
                    ui.add_space(6.0);
                    self.timeline(ui, tab - 2, height);
                    ui.add_space(8.0);
                    self.details(ui, tab - 2);
                });
            }
        }
        // A drag is one step of the undo, and the steps take their order again at its end.
        if self.tab != PICTURES && !ui.input(|i| i.pointer.any_down()) {
            if let Some(doc) = &mut self.doc {
                for (screen, sequence) in doc.iter_mut().enumerate() {
                    let followed = sequence.settle(self.selected[screen]);
                    if followed != self.selected[screen] {
                        self.selected[screen] = followed;
                        ui.ctx().request_repaint();
                    }
                }
            }
            self.history.track(ui, self.version as u64, &mut self.doc);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: every tab draws without a change, a change of Pokémon is made and undone with the keys.
    #[test]
    fn the_page_draws_every_tab_and_undoes_a_change() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_title_page_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        sango_species::menus::set_source(&session);
        let mut page = TitlePage::default();
        let ctx = egui::Context::default();
        let mut frame = |page: &mut TitlePage, input: egui::RawInput| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), ..input };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        };
        for tab in 0..TABS.len() {
            page.tab = tab;
            if tab >= 2 {
                for place in [0, 5] {
                    page.selected[tab - 2] = Some(place);
                    frame(&mut page, Default::default());
                    frame(&mut page, Default::default());
                }
            }
            frame(&mut page, Default::default());
            frame(&mut page, Default::default());
            assert!(page.doc == page.saved, "tab {tab}: the page changed the sequences by itself");
        }
        page.tab = POKEMON;
        frame(&mut page, Default::default());
        {
            let game = page.store.as_ref().unwrap().game[0][1].clone();
            let group = pokemon::groups(&game)[0];
            pokemon::set_pokemon(&mut page.doc.as_mut().unwrap()[1], &game, group, 25, 0, 0);
        }
        frame(&mut page, Default::default());
        assert!(page.unsaved());
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert!(!page.unsaved(), "undone");
        // The title screen of Omega Ruby shows every tab too.
        page.show_version(None, 1);
        for tab in 0..TABS.len() {
            page.tab = tab;
            frame(&mut page, Default::default());
            frame(&mut page, Default::default());
            assert!(page.doc == page.saved, "Omega Ruby, tab {tab}: the page changed the sequences by itself");
        }
        assert!(page.listed().iter().all(|&place| page.store.as_ref().unwrap().pictures[place].version == 1));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: with the keys alone, the tabs open, and the lists of the pictures and of the steps change their choice.
    #[test]
    fn the_keys_reach_the_tabs_and_the_lists() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_title_keys_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        sango_species::menus::set_source(&session);
        let mut page = TitlePage::default();
        let ctx = egui::Context::default();
        let mut frame = |page: &mut TitlePage, key: Option<(egui::Key, egui::Modifiers)>| {
            let events = key.map(|(key, modifiers)| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
            let modifiers = key.map_or_else(Default::default, |(_, modifiers)| modifiers);
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))), events: events.into_iter().collect(), modifiers, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        };
        let active = || ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        let down = (egui::Key::ArrowDown, egui::Modifiers::NONE);
        let right = (egui::Key::ArrowRight, egui::Modifiers::NONE);
        let jump = (egui::Key::ArrowDown, egui::Modifiers::CTRL);
        for _ in 0..3 {
            frame(&mut page, None);
        }
        // From the menu at the top (the arrows alone change it), Ctrl+Down goes to the tabs, and Right opens the next one.
        frame(&mut page, Some(jump));
        frame(&mut page, None);
        assert_eq!(active(), Some(widget::tab_id("title", PICTURES)), "Ctrl+Down reaches the tabs");
        assert_eq!(page.version, 0);
        frame(&mut page, Some(right));
        frame(&mut page, None);
        assert_eq!(page.tab, POKEMON);
        frame(&mut page, Some((egui::Key::ArrowLeft, egui::Modifiers::NONE)));
        frame(&mut page, None);
        assert_eq!(page.tab, PICTURES);
        // The list of the pictures: reached from the tabs, then Down chooses the next picture.
        let list = egui::Id::new("title_picture_list");
        for _ in 0..4 {
            if active() == Some(list) {
                break;
            }
            frame(&mut page, Some(jump));
            frame(&mut page, None);
        }
        assert_eq!(active(), Some(list), "the list of the pictures is reached");
        let before = page.picture;
        frame(&mut page, Some(down));
        frame(&mut page, None);
        assert_ne!(page.picture, before);
        // The timeline: reached from the tabs, then Down chooses the next step.
        page.tab = 3;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), widget::tab_id("title", 3)));
        frame(&mut page, None);
        frame(&mut page, None);
        let steps = egui::Id::new(("title_step_list", 1usize));
        for _ in 0..5 {
            if active() == Some(steps) {
                break;
            }
            frame(&mut page, Some(jump));
            frame(&mut page, None);
        }
        assert_eq!(active(), Some(steps), "the timeline is reached");
        page.selected[1] = Some(2);
        frame(&mut page, Some(down));
        frame(&mut page, None);
        assert_eq!(page.selected[1], Some(3));
        assert!(!page.unsaved(), "the keys that choose change nothing");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
