//! Undo and redo for a page: the page gives the state of its entry at each frame, and Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z), or the two
//! buttons of the top bar, put back an older or a newer state.
//!
//! The changes that come quickly one after the other (a drag of a number) are one step. A text box that has the keys keeps Ctrl+Z for
//! itself.

use crate::egui;
use crate::theme::color;

/// How many steps are kept.
const LIMIT: usize = 200;

/// The time in seconds after which a change starts a new step.
const STEP_GAP: f64 = 0.8;

/// The history of the states of one entry of a page.
#[derive(Clone, Debug)]
pub struct History<T> {
    /// The entry that the history is for: a new entry clears the history.
    key: Option<u64>,
    undo: Vec<T>,
    redo: Vec<T>,
    /// The state of the last frame.
    last: Option<T>,
    /// The time of the last change.
    last_change: f64,
    /// The button that was clicked: the next [`History::track`] does it.
    asked: Option<Step>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History { key: None, undo: Vec::new(), redo: Vec::new(), last: None, last_change: f64::NEG_INFINITY, asked: None }
    }
}

/// What the user asks with a key or with a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Undo,
    Redo,
}

/// The input of a frame where the user presses Ctrl and `key`: for the tests of the pages.
#[doc(hidden)]
pub fn test_keys(key: egui::Key) -> egui::RawInput {
    let modifiers = egui::Modifiers::COMMAND;
    let event = egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    egui::RawInput { modifiers, events: vec![event], ..Default::default() }
}

/// Reads Ctrl+Z (undo), Ctrl+Y and Ctrl+Shift+Z (redo). The key is used. Nothing while a text box has the keys.
pub fn pressed(ui: &egui::Ui) -> Option<Step> {
    if ui.ctx().wants_keyboard_input() {
        return None;
    }
    ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) {
            Some(Step::Redo)
        } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
            Some(Step::Undo)
        } else {
            None
        }
    })
}

/// The buttons of the top bar: undo and redo, each with a turned arrow. One that has nothing to do is grey. Gives the button that
/// was clicked.
pub fn buttons(ui: &mut egui::Ui, can_undo: bool, can_redo: bool) -> Option<Step> {
    let mut clicked = None;
    for (step, enabled, hover) in [(Step::Undo, can_undo, "Undo (Ctrl+Z)"), (Step::Redo, can_redo, "Redo (Ctrl+Y)")] {
        let button = egui::Button::new("").min_size(egui::vec2(30.0, 0.0));
        let response = ui.add_enabled(enabled, button).on_hover_text(hover).on_disabled_hover_text(hover);
        paint_arrow(ui, response.rect, step, if enabled { color::TEXT } else { color::MUTED.gamma_multiply(0.5) });
        if response.clicked() {
            clicked = Some(step);
        }
    }
    clicked
}

/// A half circle with a head at one end: at the left for undo, at the right for redo.
fn paint_arrow(ui: &egui::Ui, rect: egui::Rect, step: Step, ink: egui::Color32) {
    const RADIUS: f32 = 5.0;
    let side = if step == Step::Undo { 1.0 } else { -1.0 };
    let center = rect.center() + egui::vec2(0.0, 0.5);
    let at = |x: f32, y: f32| center + egui::vec2(x * side, y);
    // From the head, over the top, to a short tail on the other side.
    let mut line: Vec<egui::Pos2> = (0..=12).map(|n| std::f32::consts::PI * (1.0 + n as f32 / 12.0)).map(|angle| at(RADIUS * angle.cos(), RADIUS * angle.sin())).collect();
    line.push(at(RADIUS, 3.5));
    ui.painter().add(egui::Shape::line(line, egui::Stroke::new(1.6_f32, ink)));
    let head = vec![at(-RADIUS - 3.5, -0.5), at(-RADIUS + 3.5, -0.5), at(-RADIUS, 4.5)];
    ui.painter().add(egui::Shape::convex_polygon(head, ink, egui::Stroke::NONE));
}

impl<T: Clone + PartialEq> History<T> {
    /// Forgets all the steps.
    pub fn clear(&mut self) {
        *self = History::default();
    }

    /// The two buttons of the top bar ([`buttons`]). Call it before [`History::track`] in the same frame. `usable` is false when the
    /// page does not call `track` now: the buttons are then grey.
    pub fn buttons(&mut self, ui: &mut egui::Ui, usable: bool) {
        if let Some(step) = buttons(ui, usable && self.can_undo(), usable && self.can_redo()) {
            self.asked = Some(step);
            // For a page that calls `track` before its buttons: the next frame does the step.
            ui.ctx().request_repaint();
        }
    }

    /// Call once a frame, after the page drew `value`. `key` names the entry (a new key clears the history). Handles Ctrl+Z, Ctrl+Y
    /// and the buttons, and gives true when it changed `value`.
    pub fn track(&mut self, ui: &egui::Ui, key: u64, value: &mut T) -> bool {
        let step = self.asked.take().or_else(|| pressed(ui));
        let now = ui.input(|i| i.time);
        let before = (self.can_undo(), self.can_redo());
        let changed = self.track_at(key, value, step, now);
        // The buttons are drawn before this call: one more frame shows their new state.
        if changed || before != (self.can_undo(), self.can_redo()) {
            ui.ctx().request_repaint();
        }
        changed
    }

    /// [`History::track`] without the interface: `step` is what the user asked, `now` the time in seconds.
    pub fn track_at(&mut self, key: u64, value: &mut T, step: Option<Step>, now: f64) -> bool {
        if self.key != Some(key) {
            self.clear();
            self.key = Some(key);
            self.last = Some(value.clone());
            return false;
        }
        // A change of this frame goes in the history before the key is read.
        if self.last.as_ref() != Some(value) {
            if let Some(last) = self.last.take() {
                if now - self.last_change > STEP_GAP || self.undo.is_empty() {
                    self.undo.push(last);
                    if self.undo.len() > LIMIT {
                        self.undo.remove(0);
                    }
                }
            }
            self.redo.clear();
            self.last = Some(value.clone());
            self.last_change = now;
        }
        let (from, to) = match step {
            Some(Step::Undo) => (&mut self.undo, &mut self.redo),
            Some(Step::Redo) => (&mut self.redo, &mut self.undo),
            None => return false,
        };
        let Some(state) = from.pop() else { return false };
        to.push(std::mem::replace(value, state));
        self.last = Some(value.clone());
        // The next change starts a new step.
        self.last_change = f64::NEG_INFINITY;
        true
    }

    /// True when there is a step to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// True when there is a step to do again.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// How many steps are kept for the actions that write files: a step can hold a large file.
const FILE_LIMIT: usize = 20;

/// Undo and redo of the actions that write their files at once (an import, a reset). There is no state of the page to follow:
/// each step keeps what the action replaced.
#[derive(Debug)]
pub struct Steps<T> {
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> Default for Steps<T> {
    fn default() -> Self {
        Steps { undo: Vec::new(), redo: Vec::new() }
    }
}

impl<T> Steps<T> {
    /// Forgets all the steps.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// An action replaced `before`: a new step.
    pub fn done(&mut self, before: T) {
        self.undo.push(before);
        if self.undo.len() > FILE_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The two buttons of the top bar ([`buttons`]) and the keys. Gives what the user asks. When `usable` is false, the buttons
    /// are grey and the keys do nothing.
    pub fn asked(&self, ui: &mut egui::Ui, usable: bool) -> Option<Step> {
        let clicked = buttons(ui, usable && !self.undo.is_empty(), usable && !self.redo.is_empty());
        clicked.or_else(|| if usable { pressed(ui) } else { None })
    }

    /// Does a step: `swap` puts back what the step keeps, and gives what it replaced. Gives `None` when there is nothing to do.
    /// When `swap` fails, the step stays.
    pub fn apply(&mut self, step: Step, swap: impl FnOnce(&T) -> Result<T, String>) -> Option<Result<(), String>> {
        let (from, to) = match step {
            Step::Undo => (&mut self.undo, &mut self.redo),
            Step::Redo => (&mut self.redo, &mut self.undo),
        };
        let kept = from.pop()?;
        Some(match swap(&kept) {
            Ok(replaced) => {
                to.push(replaced);
                Ok(())
            }
            Err(message) => {
                from.push(kept);
                Err(message)
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn quick_changes_are_one_step_and_redo_comes_back() {
        let mut h = History::default();
        let mut v = 1;
        h.track_at(7, &mut v, None, 0.0);
        // Three quick changes: one step.
        for (x, t) in [(2, 1.0), (3, 1.2), (4, 1.4)] {
            v = x;
            h.track_at(7, &mut v, None, t);
        }
        // A change later: another step.
        v = 10;
        h.track_at(7, &mut v, None, 5.0);
        assert!(h.track_at(7, &mut v, Some(Step::Undo), 6.0));
        assert_eq!(v, 4);
        assert!(h.track_at(7, &mut v, Some(Step::Undo), 6.1));
        assert_eq!(v, 1);
        assert!(!h.track_at(7, &mut v, Some(Step::Undo), 6.2), "nothing more to undo");
        assert!(h.track_at(7, &mut v, Some(Step::Redo), 6.3));
        assert_eq!(v, 4);
        // A new change forgets the redo.
        v = 20;
        h.track_at(7, &mut v, None, 9.0);
        assert!(!h.track_at(7, &mut v, Some(Step::Redo), 9.1));
        // Another entry: a new history.
        assert!(!h.track_at(8, &mut v, Some(Step::Undo), 9.2));
        assert!(!h.can_undo() && !h.can_redo());
    }

    #[test]
    fn the_steps_of_the_files_go_back_and_forth_and_stay_after_a_failure() {
        let mut steps = Steps::default();
        let mut file = 1;
        for next in [2, 3] {
            steps.done(file);
            file = next;
        }
        let mut swap = |kept: &i32| Ok(std::mem::replace(&mut file, *kept));
        assert_eq!(steps.apply(Step::Undo, &mut swap), Some(Ok(())));
        assert_eq!(steps.apply(Step::Undo, |_| Err("no".to_string())), Some(Err("no".to_string())));
        assert_eq!(steps.apply(Step::Undo, &mut swap), Some(Ok(())));
        assert_eq!(steps.apply(Step::Undo, &mut swap), None);
        assert_eq!(steps.apply(Step::Redo, &mut swap), Some(Ok(())));
        assert_eq!(file, 2);
    }

    #[test]
    fn a_click_on_a_button_is_done_by_the_next_track() {
        let ctx = egui::Context::default();
        let mut history = History::default();
        let mut value = 1;
        // One frame: the buttons, then the track, like a page.
        let frame = |history: &mut History<i32>, value: &mut i32, events: Vec<egui::Event>| {
            let mut undo = egui::Rect::NOTHING;
            let input = egui::RawInput { events, ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let start = ui.cursor().min;
                    ui.horizontal(|ui| history.buttons(ui, true));
                    undo = egui::Rect::from_min_size(start, egui::vec2(30.0, 18.0));
                    history.track(ui, 0, value);
                });
            });
            undo
        };
        let undo = frame(&mut history, &mut value, Vec::new()).center();
        value = 2;
        frame(&mut history, &mut value, Vec::new());
        assert!(history.can_undo());
        // The frame that the history asks for: the button is not grey any more.
        frame(&mut history, &mut value, Vec::new());
        let press = |pressed| egui::Event::PointerButton { pos: undo, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        frame(&mut history, &mut value, vec![egui::Event::PointerMoved(undo), press(true)]);
        frame(&mut history, &mut value, vec![press(false)]);
        assert_eq!(value, 1);
        assert!(history.can_redo() && !history.can_undo());
    }
}
