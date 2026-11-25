//! Undo and redo for a page: the page gives the state of its entry at each frame, and Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z) put back an
//! older or a newer state.
//!
//! The changes that come quickly one after the other (a drag of a number) are one step. A text box that has the keys keeps Ctrl+Z for
//! itself.

use crate::egui;

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
}

impl<T> Default for History<T> {
    fn default() -> Self {
        History { key: None, undo: Vec::new(), redo: Vec::new(), last: None, last_change: f64::NEG_INFINITY }
    }
}

/// The key that the user pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Undo,
    Redo,
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

impl<T: Clone + PartialEq> History<T> {
    /// Forgets all the steps.
    pub fn clear(&mut self) {
        *self = History::default();
    }

    /// Call once a frame, after the page drew `value`. `key` names the entry (a new key clears the history). Handles Ctrl+Z and Ctrl+Y
    /// and gives true when it changed `value`.
    pub fn track(&mut self, ui: &egui::Ui, key: u64, value: &mut T) -> bool {
        let step = pressed(ui);
        let now = ui.input(|i| i.time);
        self.track_at(key, value, step, now)
    }

    /// [`History::track`] without the interface: `step` is the key that the user pressed, `now` the time in seconds.
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
        assert!(!h.can_undo());
    }
}
