//! The Choreography page: the steps of the choreography of a move, on a timeline.
//!
//! The choreographies are the files of the archive 34 of the romfs. The page reads one, shows its steps as bars on a timeline (one row for each
//! step, the frames from left to right), and edits the parameters of a step with the right widget for each one: a number, a switch, a
//! menu of names. A file that the user changes goes on the SD card (`choreography/<number>.bin`, not compressed, as in the archive: the game freezes on a compressed one) and the plugin
//! reads it instead of the file of the game. See `move_choreography.rs` for the format and `schema.rs` for the meaning of the bytes.

mod move_choreography;
mod placement;
mod schema;

pub use placement::{screen_placements, ScreenPlacement};

/// The names of the effects (the particle files): the number of the file and its name.
pub use schema::EFFECT_NAMES;

use move_choreography::{MoveChoreography, Value};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use schema::{ParamKind, ENUMS};

/// The archive of the choreographies of the moves.
const ARCHIVE: usize = 34;
/// The folder of the SD card.
const FOLDER: &str = "choreography";
/// The width of the names of the steps, at the left of the timeline.
const LABEL_WIDTH: f32 = 250.0;
/// The height of a row of the timeline.
const ROW: f32 = 22.0;

/// The page.
pub struct Choreography {
    /// The window of the button Reset.
    reset_open: bool,
    /// The file that the user chose while the open choreography had changes that were not saved: the window asks first.
    pending_file: Option<usize>,
    garc: Option<Garc>,
    root: Option<std::path::PathBuf>,
    error: Option<String>,
    /// The name of each file of the archive.
    names: Vec<String>,
    file: usize,
    choreography: Option<MoveChoreography>,
    /// The bytes of the file as they were loaded or saved: to know what changed.
    baseline: Vec<u8>,
    selected: Option<usize>,
    /// The width of a frame on the timeline, in points.
    zoom: f32,
    status: Option<(bool, String)>,
    /// The kind of the step that "Add a step" makes.
    new_kind: u16,
    /// Undo and redo of the open choreography.
    history: sango_ui::history::History<Option<MoveChoreography>>,
}

impl Default for Choreography {
    fn default() -> Self {
        Choreography { reset_open: false, garc: None, root: None, error: None, names: Vec::new(), file: 119 + 1, choreography: None, baseline: Vec::new(), selected: None, zoom: 6.0, status: None, new_kind: 1, pending_file: None, history: Default::default() }
    }
}

fn path(file: usize) -> String {
    format!("{FOLDER}/{file}.bin")
}

/// The colour of a step on the timeline, by the family of its type (the first word of the name).
fn family_color(name: &str) -> egui::Color32 {
    match name.split(' ').next().unwrap_or("") {
        "Pokemon" => color::ACCENT,
        "Camera" => egui::Color32::from_rgb(14, 165, 233),
        "Effect" => color::AMBER,
        "Model" | "Debris" => egui::Color32::from_rgb(168, 85, 247),
        "Sound" | "Sound3d" => color::TEAL,
        "Overlay" | "Screen" | "Backdrop" => egui::Color32::from_rgb(236, 72, 153),
        _ => color::MUTED,
    }
}

/// The name of a resource.
fn asset_name(list: &[(i32, &'static str)], value: i32) -> String {
    list.iter().find(|(number, _)| *number == value).map_or_else(|| format!("Unknown ({value})"), |(_, name)| name.to_string())
}

fn asset_list(kind: &str) -> &'static [(i32, &'static str)] {
    match kind {
        "effect" => schema::EFFECT_NAMES,
        "model" => schema::MODEL_NAMES,
        _ => schema::LAYOUT_NAMES,
    }
}

/// A menu of the names of a list of numbers.
fn named_menu(ui: &mut egui::Ui, id: &str, value: &mut i32, entries: &[(i32, String)], width: f32) {
    let shown = entries.iter().find(|(number, _)| number == value).map_or_else(|| format!("Unknown ({value})"), |(_, name)| name.clone());
    // The menu works with the places of the list: the numbers can be big.
    let mut place = entries.iter().position(|(number, _)| number == value).map_or(u16::MAX, |at| at as u16);
    let changed = picker(ui, id, &shown, width, &mut place, None, || vec![Group { title: String::new(), entries: entries.iter().enumerate().take(u16::MAX as usize).map(|(at, (_, name))| (at as u16, name.clone())).collect() }]);
    if changed {
        if let Some((number, _)) = entries.get(place as usize) {
            *value = *number;
        }
    }
}

impl Choreography {
    fn open(&mut self, dump: &Dump) {
        self.error = None;
        match Garc::open(&dump.archive(ARCHIVE)) {
            Ok(garc) => {
                // The menu counts from 1: the name at the place n is the one of the file n + 1.
                self.names = (1..garc.file_count()).map(|id| schema::FILE_LABELS.get(id).copied().unwrap_or("Choreography").to_string()).collect();
                self.garc = Some(garc);
            }
            Err(message) => self.error = Some(message),
        }
        self.choreography = None;
        self.history.clear();
    }

    /// Reads the open choreography: the SD card first, then the game.
    fn load(&mut self, sd: Option<&SdCard>) {
        let Some(garc) = self.garc.as_ref() else { return };
        let bytes = sd.and_then(|sd| sd.read(&path(self.file))).map(|b| if sango_core::lz::is_compressed(&b) { sango_core::lz::decompress(&b).unwrap_or_default() } else { b }).or_else(|| garc.first_file_decompressed(self.file));
        match bytes.map(|b| (MoveChoreography::parse(&b), b)) {
            Some((Ok(choreography), bytes)) => {
                self.choreography = Some(choreography);
                self.baseline = bytes;
                self.selected = None;
                self.status = None;
            }
            Some((Err(message), _)) => {
                self.choreography = None;
                self.status = Some((false, message));
            }
            None => {
                self.choreography = None;
                self.status = Some((false, "This choreography is not in the archive.".to_string()));
            }
        }
    }

    fn dirty(&self) -> bool {
        self.choreography.as_ref().is_some_and(|s| s.to_bytes() != self.baseline)
    }

    fn save(&mut self, sd: &SdCard) {
        let Some(choreography) = self.choreography.as_mut() else { return };
        choreography.fit_header();
        let bytes = choreography.to_bytes();
        // The files of the game are not compressed, and the plugin gives the file of the card to the game as it is.
        let result = sd.write(&path(self.file), &bytes);
        match result {
            Ok(_) => {
                self.baseline = bytes;
                self.status = Some((true, "Saved. Restart the game to see the choreography.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn reset(&mut self, sd: &SdCard) {
        match sd.remove(&path(self.file)) {
            Ok(_) => {
                self.load(Some(sd));
                self.status = Some((true, "The choreography of the game is back. Restart the game to see it.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        if let Some(file) = self.pending_file {
            let what = format!("the choreography {}", self.file);
            match widget::unsaved_window(ui.ctx(), &what) {
                Some(widget::Unsaved::Save) => {
                    if let Some(sd) = session.sd.as_ref() {
                        self.save(sd);
                    }
                    if !self.dirty() {
                        self.file = file;
                        self.load(session.sd.as_ref());
                    }
                    self.pending_file = None;
                }
                Some(widget::Unsaved::Discard) => {
                    self.file = file;
                    self.load(session.sd.as_ref());
                    self.pending_file = None;
                }
                Some(widget::Unsaved::Stay) => self.pending_file = None,
                None => {}
            }
        }
        let dirty = self.dirty();
        let from_card = session.sd.as_ref().is_some_and(|sd| sd.read(&path(self.file)).is_some());
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut file = self.file;
                let names: Vec<&str> = self.names.iter().map(String::as_str).collect();
                sango_species_base::searchable_picker(ui, "choreography_file", &mut file, &names, 320.0);
                if file != self.file && file >= 1 && file <= names.len() {
                    if dirty {
                        // The changes of the open file would be lost: the window asks first.
                        self.pending_file = Some(file);
                    } else {
                        self.file = file;
                        self.load(session.sd.as_ref());
                    }
                }
                let has_card = session.sd.is_some();
                let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(self.choreography.is_some()), import: Some(true), reset: Some(has_card && from_card), save: Some(has_card && dirty), ..Default::default() });
                self.history.buttons(ui, self.choreography.is_some());
                if clicks.export {
                    if let Some(choreography) = self.choreography.as_ref() {
                        self.status = export_bytes("Export the choreography", &format!("choreography_{}.bin", self.file), ("Choreography", &["bin"]), &choreography.to_bytes()).or(self.status.take());
                    }
                }
                if clicks.import {
                    match import_bytes("Import a choreography", ("Choreography", &["bin"])) {
                        Some(Ok(bytes)) => match MoveChoreography::parse(&bytes) {
                            Ok(choreography) => {
                                self.choreography = Some(choreography);
                                self.selected = None;
                                self.status = Some((true, "Imported. Save to keep it.".to_string()));
                            }
                            Err(message) => self.status = Some((false, format!("This file is not a choreography: {message}"))),
                        },
                        Some(Err(message)) => self.status = Some((false, message)),
                        None => {}
                    }
                }
                self.reset_open |= clicks.reset;
                if let (true, Some(sd)) = (clicks.save, session.sd.as_ref()) {
                    self.save(sd);
                }
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if from_card {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if session.sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can look at the choreographies, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &format!("The choreography {}", self.file), &mut []) {
            if let Some(sd) = session.sd.as_ref() {
                self.reset(sd);
            }
        }
    }

    /// The timeline: a row for each step, with a bar from its first to its last frame.
    fn timeline(&mut self, ui: &mut egui::Ui) {
        let Some(choreography) = self.choreography.as_ref() else { return };
        let length = choreography.length().max(choreography.last_frame()).max(30);
        let width = length as f32 * self.zoom + 40.0;
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title("Timeline"));
                ui.label(widget::hint(&format!("{} steps, {} frames ({:.1} s). Click a step to edit it.", choreography.steps.len(), length, length as f32 / 30.0)));
                ui.add(egui::Slider::new(&mut self.zoom, 2.0..=16.0).text("Zoom"));
            });
            ui.add_space(4.0);
            let steps = choreography.steps.clone();
            let height = (steps.len() as f32 * ROW + 24.0).min(420.0);
            egui::ScrollArea::both().id_salt("choreography_timeline").max_height(height + 8.0).auto_shrink([false, true]).show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH + width, steps.len() as f32 * ROW + 20.0), egui::Sense::hover());
                let painter = ui.painter_at(rect);
                let origin = rect.min + egui::vec2(LABEL_WIDTH, 0.0);
                // The ruler: a mark each ten frames, a number each thirty (one second).
                for frame in (0..=length).step_by(10) {
                    let x = origin.x + frame as f32 * self.zoom;
                    painter.line_segment([egui::pos2(x, rect.top() + 16.0), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0_f32, if frame % 30 == 0 { color::BORDER } else { color::SURFACE_STRONG }));
                    if frame % 30 == 0 {
                        painter.text(egui::pos2(x + 2.0, rect.top()), egui::Align2::LEFT_TOP, frame.to_string(), egui::FontId::proportional(11.0), color::MUTED);
                    }
                }
                for (place, step) in steps.iter().enumerate() {
                    let top = rect.top() + 20.0 + place as f32 * ROW;
                    let row = egui::Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(LABEL_WIDTH + width, ROW));
                    let response = ui.interact(row, ui.id().with(("choreography_step", place)), egui::Sense::click());
                    let chosen = self.selected == Some(place);
                    if chosen {
                        painter.rect_filled(row, 3.0, color::ACCENT_SOFT);
                    } else if response.hovered() {
                        painter.rect_filled(row, 3.0, color::SURFACE);
                    }
                    let name = move_choreography::kind_of(step.kind).map_or(format!("Step {}", step.kind), |k| k.name.to_string());
                    let label = if step.group != 0 { format!("{name}  (object {})", step.group) } else { name.clone() };
                    painter.text(egui::pos2(rect.left() + 4.0, top + ROW / 2.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if chosen { color::ACCENT } else { color::TEXT });
                    let (x0, x1) = (origin.x + step.start as f32 * self.zoom, origin.x + step.end.max(step.start) as f32 * self.zoom);
                    let bar = egui::Rect::from_min_max(egui::pos2(x0, top + 4.0), egui::pos2(x1.max(x0 + 4.0), top + ROW - 4.0));
                    painter.rect_filled(bar, 3.0, family_color(&name).gamma_multiply(if step.condition != 0 { 0.55 } else { 1.0 }));
                    if response.clicked() {
                        self.selected = Some(place);
                    }
                }
            });
        });
    }

    /// The parameters of the selected step, and the buttons to add, copy and remove steps.
    fn details(&mut self, ui: &mut egui::Ui) {
        let Some(choreography) = self.choreography.as_mut() else { return };
        let mut remove = None;
        let mut duplicate = None;
        let mut up = None;
        let mut down = None;
        widget::card(ui, |ui| {
            let Some(place) = self.selected.filter(|&p| p < choreography.steps.len()) else {
                ui.label(widget::hint("Choose a step on the timeline to edit it, or add a new one."));
                return;
            };
            let kind = move_choreography::kind_of(choreography.steps[place].kind);
            let step = &mut choreography.steps[place];
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::title(kind.map_or("Unknown step", |k| k.name)));
                if widget::button(ui, "▲") {
                    up = Some(place);
                }
                if widget::button(ui, "▼") {
                    down = Some(place);
                }
                if widget::button(ui, "Duplicate") {
                    duplicate = Some(place);
                }
                if widget::confirm_button(ui, "Remove", "Click again to remove") {
                    remove = Some(place);
                }
            });
            ui.add_space(4.0);
            egui::Grid::new("choreography_step_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("First frame");
                widget::spin(ui, &mut step.start, 0.0, 100_000.0);
                ui.end_row();
                ui.label("Last frame");
                widget::spin(ui, &mut step.end, 0.0, 100_000.0);
                ui.end_row();
                ui.label("Object");
                ui.horizontal(|ui| {
                    widget::spin(ui, &mut step.group, 0.0, 65_535.0);
                    ui.label(widget::hint("The steps with the same number act on the same effect, model or sound."));
                });
                ui.end_row();
                ui.label("Plays when");
                let conditions = ENUMS.iter().find(|(name, _)| *name == "StepCondition").map_or(&[][..], |(_, list)| *list);
                let entries: Vec<(i32, String)> = conditions.iter().map(|(name, value)| (*value, name.to_string())).collect();
                let mut condition = i32::from(step.condition);
                named_menu(ui, "choreography_condition", &mut condition, &entries, 280.0);
                step.condition = condition as u16;
                ui.end_row();
                if let Some(kind) = kind {
                    for param in kind.params {
                        ui.label(param.name.replace('_', " "));
                        let mut value = step.value(param);
                        let changed = param_widget(ui, &format!("choreography_param_{}_{}", place, param.name), param.kind, &mut value);
                        if changed {
                            step.set_value(param, &value);
                        }
                        ui.end_row();
                    }
                }
            });
        });
        if let Some(place) = remove {
            choreography.steps.remove(place);
            self.selected = None;
        }
        if let Some(place) = duplicate {
            let copy = choreography.steps[place].clone();
            choreography.steps.insert(place + 1, copy);
            self.selected = Some(place + 1);
        }
        if let Some(place) = up.filter(|&p| p > 0) {
            choreography.steps.swap(place, place - 1);
            self.selected = Some(place - 1);
        }
        if let Some(place) = down.filter(|&p| p + 1 < choreography.steps.len()) {
            choreography.steps.swap(place, place + 1);
            self.selected = Some(place + 1);
        }
    }

    /// "Add a step": the kind of the step, in a menu of names grouped by family.
    fn add_bar(&mut self, ui: &mut egui::Ui) {
        let Some(choreography) = self.choreography.as_mut() else { return };
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let shown = move_choreography::kind_of(self.new_kind).map_or("?".to_string(), |k| k.name.to_string());
            let mut value = self.new_kind;
            picker(ui, "choreography_new_kind", &shown, 300.0, &mut value, None, || {
                let mut families: Vec<(String, Vec<(u16, String)>)> = Vec::new();
                for kind in schema::KINDS.iter().filter(|k| k.size > 0 || !k.params.is_empty() || k.number > 0) {
                    let family = kind.name.split(' ').next().unwrap_or("").to_string();
                    match families.iter_mut().find(|(name, _)| *name == family) {
                        Some((_, list)) => list.push((kind.number, kind.name.to_string())),
                        None => families.push((family, vec![(kind.number, kind.name.to_string())])),
                    }
                }
                families.into_iter().map(|(title, entries)| Group { title, entries }).collect()
            });
            self.new_kind = value;
            if widget::button_styled(ui, egui::RichText::new("+ Add a step").color(color::WHITE), Some(color::ACCENT)) {
                let start = self.selected.and_then(|p| choreography.steps.get(p)).map_or(0, |s| s.start);
                if let Some(step) = MoveChoreography::new_step(self.new_kind, start, start) {
                    choreography.steps.push(step);
                    self.selected = Some(choreography.steps.len() - 1);
                }
            }
            ui.label(widget::hint("The new step starts at the first frame of the selected step."));
        });
    }
}

/// The widget of a parameter. Gives true when the value changed.
fn param_widget(ui: &mut egui::Ui, id: &str, kind: ParamKind, value: &mut Value) -> bool {
    match (kind, value) {
        (ParamKind::Int, Value::Int(v)) => widget::spin(ui, v, -1_000_000.0, 1_000_000.0),
        (ParamKind::Bool, Value::Bool(v)) => ui.checkbox(v, "").changed(),
        (ParamKind::Float, Value::Float(v)) => ui.add(egui::DragValue::new(v).speed(0.05)).changed(),
        (ParamKind::Vec3, Value::Vec3(v)) => {
            let mut changed = false;
            ui.horizontal(|ui| {
                for (axis, part) in ["X", "Y", "Z"].iter().zip(v.iter_mut()) {
                    ui.label(widget::hint(axis));
                    changed |= ui.add(egui::DragValue::new(part).speed(0.5)).changed();
                }
            });
            changed
        }
        (ParamKind::Text(max), Value::Text(text)) => ui.add(egui::TextEdit::singleline(text).desired_width(200.0).char_limit(max.saturating_sub(1))).changed(),
        (ParamKind::Choice(name), Value::Int(v)) => {
            let list = ENUMS.iter().find(|(n, _)| *n == name).map_or(&[][..], |(_, list)| *list);
            let entries: Vec<(i32, String)> = list.iter().map(|(label, number)| (*number, label.to_string())).collect();
            let before = *v;
            named_menu(ui, id, v, &entries, 240.0);
            *v != before
        }
        (ParamKind::Asset(kind), Value::Int(v)) => {
            let entries: Vec<(i32, String)> = asset_list(kind).iter().map(|(number, name)| (*number, name.to_string())).collect();
            let before = *v;
            named_menu(ui, id, v, &entries, 300.0);
            let _ = asset_name;
            *v != before
        }
        (_, Value::Bytes(_)) => {
            ui.label(widget::hint("A curve: not edited here yet."));
            false
        }
        _ => false,
    }
}

impl Module for Choreography {
    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn title(&self) -> &str {
        "Choreography"
    }

    fn icon(&self) -> &str {
        "🎬"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Choreography");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let root = dump.root().to_path_buf();
        if self.root.as_deref() != Some(root.as_path()) {
            self.root = Some(root);
            self.open(dump);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        if self.choreography.is_none() && self.status.is_none() {
            self.load(session.sd.as_ref());
        }
        self.tool_bar(ui, session);
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("choreography_page").auto_shrink([false, false]).show(ui, |ui| {
            self.timeline(ui);
            ui.add_space(8.0);
            self.details(ui);
            self.add_bar(ui);
        });
        // Ctrl+Z, Ctrl+Y and the buttons on the open choreography. A drag is one step: nothing while the mouse button is down.
        if !ui.input(|i| i.pointer.any_down()) && self.history.track(ui, self.file as u64, &mut self.choreography) {
            let count = self.choreography.as_ref().map_or(0, |choreography| choreography.steps.len());
            self.selected = self.selected.filter(|&step| step < count);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: the page opens the choreography of a move, edits a parameter, saves it, reads it back and resets it.
    #[test]
    fn a_choreography_is_edited_saved_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_choreography_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = Choreography::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        assert!(page.choreography.as_ref().is_some_and(|s| !s.steps.is_empty()), "the choreography of the first move is open");
        assert!(!page.dirty());
        let place = page.choreography.as_ref().unwrap().steps.iter().position(|s| move_choreography::kind_of(s.kind).is_some_and(|k| k.name == "Effect Resize")).expect("a step that resizes an effect");
        let kind = move_choreography::kind_of(page.choreography.as_ref().unwrap().steps[place].kind).unwrap();
        page.choreography.as_mut().unwrap().steps[place].set_value(&kind.params[0], &Value::Vec3([9.0, 9.0, 9.0]));
        assert!(page.dirty());
        // Ctrl+Z and Ctrl+Y on the change.
        for (input, undone) in [(Default::default(), false), (sango_ui::history::test_keys(egui::Key::Z), true), (sango_ui::history::test_keys(egui::Key::Y), false)] {
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
            assert_eq!(!page.dirty(), undone);
        }
        page.save(session.sd.as_ref().unwrap());
        assert!(!page.dirty());
        assert!(session.sd.as_ref().unwrap().read("choreography/index.bin").is_some(), "the list of the files is written");
        // The file of the card is stored as the one of the game: a compressed file freezes the game.
        let saved = session.sd.as_ref().unwrap().read(&path(page.file)).expect("the file is on the SD card");
        let game = page.garc.as_ref().unwrap().first_file(page.file).unwrap();
        assert_eq!((&saved[..4], saved == page.baseline), (&game[..4], true));
        page.choreography = None;
        page.status = None;
        page.load(session.sd.as_ref());
        assert_eq!(page.choreography.as_ref().unwrap().steps[place].value(&kind.params[0]), Value::Vec3([9.0, 9.0, 9.0]));
        page.reset(session.sd.as_ref().unwrap());
        assert!(!page.dirty());
        assert_ne!(page.choreography.as_ref().unwrap().steps[place].value(&kind.params[0]), Value::Vec3([9.0, 9.0, 9.0]));
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/// Writes bytes in a file that the user chooses. Gives what happened, or `None` when the user closed the window.
fn export_bytes(title: &str, name: &str, filter: (&str, &[&str]), bytes: &[u8]) -> Option<(bool, String)> {
    let path = rfd::FileDialog::new().set_title(title).set_file_name(name).add_filter(filter.0, filter.1).save_file()?;
    Some(match std::fs::write(&path, bytes) {
        Ok(()) => (true, format!("Exported to {}.", path.display())),
        Err(e) => (false, format!("{}: {e}", path.display())),
    })
}

/// Reads a file that the user chooses (decompressed when it is compressed like the files of the game).
fn import_bytes(title: &str, filter: (&str, &[&str])) -> Option<Result<Vec<u8>, String>> {
    let path = rfd::FileDialog::new().set_title(title).add_filter(filter.0, filter.1).pick_file()?;
    Some(std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display())).map(|b| if sango_core::lz::is_compressed(&b) { sango_core::lz::decompress(&b).unwrap_or(b) } else { b }))
}
