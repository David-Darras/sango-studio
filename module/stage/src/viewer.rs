//! The viewer on the Choreography page: the picture of the stage, the controls of the movement, and the choice of the
//! two Pokémon, of the side of the attacker and of the place.
//!
//! With the keys: Space plays and stops, Left and Right move one frame, Home and End go to the first and the last one,
//! on the picture and on the bar under it.

use crate::assets::{Choice, Library};
use crate::scene::{picture, Surroundings, PLACES};
use crate::sim::{self, Run, Setup};
use sango_choreography::{kind_of, MoveChoreography, Shown, Viewer, FILE_OF_NO_MOVE};
use sango_core::{Garc, Session};
use sango_ui::theme::color;
use sango_ui::{egui, focus, retired::Retired, widget};

/// The frames of one second of the game.
const FRAMES_PER_SECOND: f32 = 30.0;
/// The size of the top screen, in pixels.
const SCREEN: (usize, usize) = (400, 240);
/// The widest that the picture is shown, in points.
const WIDEST: f32 = 800.0;
/// The archive of the data of the moves, the place of the target in the data of one move, and the targets that are the
/// user or its side.
const MOVES: usize = 189;
const TARGET_BYTE: usize = 20;
const ON_THE_USER: [u8; 3] = [1, 7, 12];

/// What the run was made from: a change makes a new run.
#[derive(PartialEq)]
struct Made {
    file: usize,
    bytes: Vec<u8>,
    setup: Setup,
    cast: [Choice; 2],
}

/// What the picture was drawn from.
#[derive(PartialEq, Clone, Copy)]
struct Drawn {
    run: u32,
    frame: i32,
    place: usize,
    smooth: bool,
}

pub struct StageViewer {
    library: Library,
    cast: [Choice; 2],
    setup: Setup,
    place: usize,
    smooth: bool,
    run: Option<Run>,
    made: Option<Made>,
    /// A number that grows with each new run.
    runs: u32,
    frame: i32,
    playing: bool,
    repeat: bool,
    /// The part of a frame that has passed while playing.
    rest: f32,
    /// The time of the last picture of the movement.
    before: Option<f64>,
    texture: Option<egui::TextureHandle>,
    drawn: Option<Drawn>,
    retired: Retired,
    error: Option<String>,
    /// The names of the species in the menus: the one of the species 1 first.
    names: Vec<String>,
}

impl Default for StageViewer {
    fn default() -> Self {
        // A Blaziken attacks a Sceptile.
        StageViewer { library: Library::default(), cast: [Choice::new(257), Choice::new(254)], setup: Setup::default(), place: 0, smooth: false, run: None, made: None, runs: 0, frame: 0, playing: false, repeat: true, rest: 0.0, before: None, texture: None, drawn: None, retired: Default::default(), error: None, names: Vec::new() }
    }
}

/// True when the move of a choreography file has its user or the side of its user as its target.
fn aims_at_user(session: &Session, file: usize) -> bool {
    let Some(move_id) = file.checked_sub(FILE_OF_NO_MOVE).filter(|&id| id > 0) else { return false };
    let Some(garc) = session.dump.as_ref().and_then(|dump| Garc::open(&dump.archive(MOVES)).ok()) else { return false };
    let Some(Ok(moves)) = garc.first_file(0).map(|bytes| sango_core::Bundle::parse(bytes.to_vec())) else { return false };
    moves.resource(move_id).and_then(|data| data.get(TARGET_BYTE)).is_some_and(|target| ON_THE_USER.contains(target))
}

/// The family of the steps that the viewer does not show, for the note under the picture.
fn missing_part(name: &str) -> &'static str {
    const PARTS: [(&str, &str); 13] = [("Sound", "sounds"), ("Health Bar", "health bars"), ("Break Objects", "broken objects"), ("Pokemon Wild Cry", "sounds"), ("Trainer", "trainers"), ("Overlay", "2D layouts"), ("Message", "messages"), ("Model Spawn Ball", "Poké Balls"), ("Weather", "weather"), ("Field Effect", "field effects"), ("Wait For Animation", "waits"), ("Pokemon Outline", "outlines"), ("Camera Flat View", "the flat camera")];
    PARTS.iter().find(|(start, _)| name.starts_with(start)).map_or("a few other steps", |(_, part)| part)
}

/// The parts of a choreography that the viewer does not show.
fn missing(choreography: &MoveChoreography) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for step in &choreography.steps {
        let Some(kind) = kind_of(step.kind) else { continue };
        if !sim::is_shown(kind.name) && !sim::is_mark(kind.name) {
            let part = missing_part(kind.name);
            if !found.contains(&part) {
                found.push(part);
            }
        }
    }
    found.sort_unstable();
    found
}

/// The first step that the game never reaches: it starts before a step above it.
fn never_played(choreography: &MoveChoreography) -> Option<usize> {
    let mut latest = 0;
    choreography.steps.iter().position(|step| {
        let late = step.start < latest;
        latest = latest.max(step.start);
        late
    })
}

impl StageViewer {
    /// Makes the run again when the file, the steps, the Pokémon or the sides changed.
    fn prepare(&mut self, session: &Session, shown: &Shown<'_>) {
        if self.library.use_session(session) {
            self.made = None;
        }
        let new_file = self.made.as_ref().is_none_or(|made| made.file != shown.file);
        if new_file {
            // The files that the user changed since are read again.
            self.library.forget();
            self.setup.targets_user = aims_at_user(session, shown.file);
            sango_species_base::extras::refresh(session.sd.as_ref());
            self.names = (1..=sango_species_base::max_species()).map(sango_species_base::species_name).map(|name| if name == "-" { String::new() } else { name }).collect();
        }
        let made = Made { file: shown.file, bytes: shown.choreography.to_bytes(), setup: self.setup, cast: self.cast };
        if self.made.as_ref() == Some(&made) {
            return;
        }
        let cast = self.cast.map(|choice| self.library.pokemon(session, choice));
        match cast {
            [Ok(attacker), Ok(target)] => {
                let run = Run::new(&mut self.library, shown.file, shown.choreography.clone(), self.setup, [attacker, target]);
                self.frame = if new_file { run.first } else { self.frame.clamp(run.first, run.last) };
                self.run = Some(run);
                self.error = None;
            }
            [Err(message), _] | [_, Err(message)] => {
                self.run = None;
                self.error = Some(message);
            }
        }
        if new_file {
            self.playing = false;
        }
        self.made = Some(made);
        self.runs += 1;
    }

    /// Moves the frame with the time that passed.
    fn play(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let passed = self.before.map_or(0.0, |before| (now - before).clamp(0.0, 0.25) as f32);
        let Some(run) = self.run.as_ref().filter(|_| self.playing) else {
            self.before = None;
            return;
        };
        self.before = Some(now);
        self.rest += passed * FRAMES_PER_SECOND;
        let whole = self.rest.floor();
        self.rest -= whole;
        self.frame += whole as i32;
        if self.frame > run.last {
            if self.repeat {
                self.frame = run.first;
            } else {
                self.frame = run.last;
                self.playing = false;
            }
        }
        ctx.request_repaint();
    }

    fn toggle(&mut self) {
        self.playing = !self.playing;
        self.rest = 0.0;
        self.before = None;
        // Playing from the end starts again.
        if self.playing && self.run.as_ref().is_some_and(|run| self.frame >= run.last) {
            self.frame = self.run.as_ref().map_or(0, |run| run.first);
        }
    }

    /// The keys of the picture and of the bar, when one of them has the focus.
    fn keys(&mut self, ui: &mut egui::Ui, id: egui::Id) {
        let Some((first, last)) = self.run.as_ref().map(|run| (run.first, run.last)) else { return };
        ui.ctx().data_mut(|d| d.insert_temp(id.with("handles_left"), true));
        let step = focus::steps(ui, true) as i32;
        let (play, start, end) = ui.input_mut(|i| {
            let mut key = |key| i.consume_key(egui::Modifiers::NONE, key);
            (key(egui::Key::Space) | key(egui::Key::Enter), key(egui::Key::Home), key(egui::Key::End))
        });
        if play {
            self.toggle();
        }
        if step != 0 || start || end {
            let to = if start { first } else if end { last } else { self.frame + step };
            self.go_to(to);
        }
    }

    fn draw(&mut self, ctx: &egui::Context, session: &Session) {
        let Some(run) = self.run.as_mut() else { return };
        let wanted = Drawn { run: self.runs, frame: self.frame, place: self.place, smooth: self.smooth };
        if self.drawn == Some(wanted) && self.texture.is_some() {
            return;
        }
        let factor = if self.smooth { 2 } else { 1 };
        run.seek(&mut self.library, session.sd.as_ref(), self.frame);
        let surroundings: Surroundings = PLACES[self.place.min(PLACES.len() - 1)];
        let canvas = picture(run, &mut self.library, session.sd.as_ref(), &surroundings, SCREEN.0 * factor, SCREEN.1 * factor);
        let image = egui::ColorImage::from_rgba_unmultiplied([canvas.width, canvas.height], &canvas.rgba());
        let texture = ctx.load_texture("choreography/stage", image, egui::TextureOptions::LINEAR);
        if let Some(old) = self.texture.replace(texture) {
            self.retired.keep(old);
        }
        self.drawn = Some(wanted);
        self.retired.tick(ctx);
    }

    fn screen(&mut self, ui: &mut egui::Ui) {
        // The timeline under the viewer stays in sight: the picture takes less than half the height of the window.
        let tall = ui.ctx().screen_rect().height() * 0.42;
        let wide = ui.available_width().min(WIDEST).min((tall * SCREEN.0 as f32 / SCREEN.1 as f32).max(SCREEN.0 as f32));
        let size = egui::vec2(wide, wide * SCREEN.1 as f32 / SCREEN.0 as f32);
        ui.vertical_centered(|ui| {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            match &self.texture {
                Some(texture) => {
                    ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
                None => {
                    ui.painter().rect_filled(rect, 6.0, color::SURFACE_STRONG);
                }
            }
            let id = egui::Id::new("choreography_stage");
            if response.clicked() {
                self.toggle();
            }
            if focus::focus(ui, id, rect, response.clicked(), true, Some(&response)) {
                self.keys(ui, id);
            }
            response.on_hover_text("Click to play or stop. With the keys: Space plays, Left and Right move one frame.");
            self.bar(ui, wide);
        });
    }

    /// The bar of the frames: a click or a drag shows a frame.
    fn bar(&mut self, ui: &mut egui::Ui, wide: f32) {
        let Some((first, last)) = self.run.as_ref().map(|run| (run.first, run.last)) else { return };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(wide, 18.0), egui::Sense::click_and_drag());
        let span = (last - first).max(1) as f32;
        let place = |frame: i32| rect.left() + (frame - first) as f32 / span * rect.width();
        let painter = ui.painter();
        let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 6.0));
        painter.rect_filled(track, 3.0, color::SURFACE_STRONG);
        if first < 0 {
            // The frames before the first step: the attack starts.
            painter.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(place(0), track.max.y)), 3.0, color::BORDER);
        }
        painter.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(place(self.frame), track.max.y)), 3.0, color::ACCENT);
        painter.circle_filled(egui::pos2(place(self.frame), rect.center().y), 7.0, color::ACCENT);
        if let Some(at) = response.interact_pointer_pos().filter(|_| response.clicked() || response.dragged()) {
            let frame = first + ((at.x - rect.left()) / rect.width() * span).round() as i32;
            self.go_to(frame);
        }
        let id = egui::Id::new("choreography_stage_bar");
        if focus::focus(ui, id, rect, response.clicked() || response.dragged(), true, Some(&response)) {
            self.keys(ui, id);
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let Some((first, last)) = self.run.as_ref().map(|run| (run.first, run.last)) else { return };
        ui.horizontal_wrapped(|ui| {
            let label = if self.playing { "⏸ Pause" } else { "▶ Play" };
            if widget::button_styled(ui, egui::RichText::new(label).color(color::WHITE), Some(color::ACCENT)) {
                self.toggle();
            }
            if widget::button(ui, "⏮ Start") {
                self.go_to(first);
            }
            if widget::button(ui, "◀ Frame") {
                self.go_to(self.frame - 1);
            }
            if widget::button(ui, "Frame ▶") {
                self.go_to(self.frame + 1);
            }
            widget::checkbox(ui, &mut self.repeat, "Repeat");
            ui.label("Frame");
            let mut frame = self.frame;
            if widget::spin(ui, &mut frame, f64::from(first), f64::from(last)) {
                self.go_to(frame);
            }
            ui.label(widget::hint(&format!("of {last}  ·  {:.2} s", self.frame as f32 / FRAMES_PER_SECOND)));
            if self.frame < 0 {
                widget::badge(ui, "The attack starts", color::AMBER);
            }
        });
    }

    fn pokemon_row(ui: &mut egui::Ui, title: &str, names: &[&str], choice: &mut Choice) {
        ui.label(title);
        let before = choice.species;
        let mut species = choice.species;
        sango_species_base::searchable_picker(ui, &format!("stage_{title}"), &mut species, names, 220.0);
        if species != before && species > 0 {
            *choice = Choice { species, form: 0, female: false, ..*choice };
        }
        let forms = sango_species_base::form_choices(choice.species);
        if forms.len() > 1 {
            let response = egui::ComboBox::from_id_salt(format!("stage_form_{title}"))
                .selected_text(forms.get(choice.form).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (form, name) in forms.iter().enumerate() {
                        ui.selectable_value(&mut choice.form, form, name);
                    }
                })
                .response;
            let values: Vec<usize> = (0..forms.len()).collect();
            focus::combo_arrows(ui, &response, &mut choice.form, &values);
        } else {
            ui.label("");
        }
        widget::checkbox(ui, &mut choice.shiny, "Shiny");
        widget::checkbox(ui, &mut choice.female, "Female");
        ui.end_row();
    }

    fn choices(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("stage_pokemon").num_columns(5).spacing([10.0, 6.0]).show(ui, |ui| {
            let names: Vec<&str> = self.names.iter().map(String::as_str).collect();
            let [attacker, target] = &mut self.cast;
            Self::pokemon_row(ui, "Attacker", &names, attacker);
            Self::pokemon_row(ui, "Target", &names, target);
        });
        ui.horizontal_wrapped(|ui| {
            if widget::button(ui, "Swap the Pokémon") {
                self.cast.swap(0, 1);
            }
            ui.label("The attacker is");
            let mut side = usize::from(self.setup.attacker_is_opponent);
            widget::tabs(ui, "stage_side", &["Yours", "The opponent"], &mut side);
            self.setup.attacker_is_opponent = side == 1;
            widget::checkbox(ui, &mut self.setup.targets_user, "The move aims at its user");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Place");
            let response = egui::ComboBox::from_id_salt("stage_place")
                .selected_text(PLACES[self.place].name)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for (number, place) in PLACES.iter().enumerate() {
                        ui.selectable_value(&mut self.place, number, place.name);
                    }
                })
                .response;
            let values: Vec<usize> = (0..PLACES.len()).collect();
            focus::combo_arrows(ui, &response, &mut self.place, &values);
            widget::checkbox(ui, &mut self.smooth, "Smooth edges (slower)");
            if widget::button(ui, "Read the files again") {
                self.library.forget();
                self.made = None;
            }
        });
    }

    fn notes(&mut self, ui: &mut egui::Ui, shown: &Shown<'_>) {
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
        }
        if let Some(step) = never_played(shown.choreography) {
            let name = shown.choreography.steps.get(step).and_then(|s| kind_of(s.kind)).map_or("?", |k| k.name);
            ui.label(egui::RichText::new(format!("The game stops reading at the step {} ({name}): it starts before a step above it. Move it up, or the steps from it on never play.", step + 1)).color(color::ROSE));
        }
        let missing = missing(shown.choreography);
        if !missing.is_empty() {
            ui.label(widget::hint(&format!("In this file, the preview does not show: {}.", missing.join(", "))));
        }
        ui.label(widget::hint("The preview follows the rules of the game. The light, the sky, the random numbers of the particles and the shadows are not the ones of the game."));
    }
}

impl Viewer for StageViewer {
    fn ui(&mut self, ui: &mut egui::Ui, session: &Session, shown: Shown<'_>) {
        self.retired.tick(ui.ctx());
        if session.dump.is_none() {
            return;
        }
        widget::card(ui, |ui| {
            ui.label(widget::title("Preview"));
            self.prepare(session, &shown);
            self.play(ui.ctx());
            // The picture is drawn before it is shown, so it is the one of this frame.
            self.draw(ui.ctx(), session);
            let before = (self.frame, self.place, self.smooth, self.setup, self.cast);
            self.screen(ui);
            ui.add_space(4.0);
            self.controls(ui);
            ui.add_space(4.0);
            self.choices(ui);
            self.notes(ui, &shown);
            if (self.frame, self.place, self.smooth, self.setup, self.cast) != before {
                ui.ctx().request_repaint();
            }
        });
    }

    fn frame(&self) -> i32 {
        self.frame
    }

    fn go_to(&mut self, frame: i32) {
        self.playing = false;
        self.frame = self.run.as_ref().map_or(frame, |run| frame.clamp(run.first, run.last));
    }
}

#[cfg(test)]
mod test;
