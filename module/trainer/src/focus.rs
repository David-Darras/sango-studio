//! The keyboard focus of the page: a click on a list or a menu gives it the focus, and the arrow keys then change the choice.

use sango_ui::{egui, widget};

/// Puts a widget in the navigation of the page: a click makes it the active one, and the active one has a glow. Gives true when the
/// widget is the active one and the page has the focus (the keys are for it).
pub fn focus(ui: &egui::Ui, id: egui::Id, rect: egui::Rect, clicked: bool, glow: bool, response: Option<&egui::Response>) -> bool {
    widget::register_focusable(ui, id, rect);
    if clicked {
        widget::set_active_id(ui, id);
    }
    if !(widget::is_active(ui, id) && widget::page_has_focus(ui)) {
        return false;
    }
    if glow {
        widget::glow(ui, rect, 6.0);
    }
    if let Some(response) = response {
        widget::scroll_active_into_view(ui, response);
    }
    true
}

/// The keys of a step: +1 for the arrow down (right when `horizontal`), -1 for the arrow up (left). The keys are used.
pub fn steps(ui: &egui::Ui, horizontal: bool) -> isize {
    let (previous, next) = if horizontal { (egui::Key::ArrowLeft, egui::Key::ArrowRight) } else { (egui::Key::ArrowUp, egui::Key::ArrowDown) };
    ui.input_mut(|i| {
        let mut step = 0;
        if i.consume_key(egui::Modifiers::NONE, next) {
            step += 1;
        }
        if i.consume_key(egui::Modifiers::NONE, previous) {
            step -= 1;
        }
        step
    })
}

/// The place after a step in a list of `len` places, or `None` when it does not change.
pub fn stepped(at: usize, step: isize, len: usize) -> Option<usize> {
    if step == 0 || len == 0 {
        return None;
    }
    let to = (at as isize + step).clamp(0, len as isize - 1) as usize;
    (to != at).then_some(to)
}

/// A menu or a row: a click gives it the focus, and the arrows give the new place. `at` is the place now, in a list of `len`.
pub fn nav(ui: &egui::Ui, response: &egui::Response, at: usize, len: usize, horizontal: bool) -> Option<usize> {
    if focus(ui, response.id, response.rect, response.clicked(), true, Some(response)) {
        stepped(at, steps(ui, horizontal), len)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_step_stays_in_the_list() {
        assert_eq!(stepped(0, -1, 5), None);
        assert_eq!(stepped(4, 1, 5), None);
        assert_eq!(stepped(2, 1, 5), Some(3));
        assert_eq!(stepped(2, -1, 5), Some(1));
        assert_eq!(stepped(0, 1, 0), None);
    }
}
