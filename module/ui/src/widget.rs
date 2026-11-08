use crate::theme::color;
use eframe::egui::{self, Rounding, Stroke};

/// A white card with a thin border.
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(color::WHITE)
        .stroke(Stroke::new(1.0_f32, color::BORDER))
        .rounding(Rounding::same(14.0))
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, add_contents)
        .inner
}

/// The title of a section.
pub fn title(text: &str) -> egui::RichText {
    egui::RichText::new(text).strong().size(17.0)
}

/// A grey line of explanation.
pub fn hint(text: &str) -> egui::RichText {
    egui::RichText::new(text).color(color::MUTED).size(13.5)
}

/// A small coloured label.
pub fn badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32) {
    let galley = egui::WidgetText::from(egui::RichText::new(text).size(12.5).strong().color(egui::Color32::WHITE))
        .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Small);
    let size = galley.size() + egui::vec2(16.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(size.y / 2.0), fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, egui::Color32::WHITE);
}

/// A button of the side bar: filled with the main colour when it is the current page. The icon is in
/// a column of its own, so that the labels of all the buttons start at the same place.
pub fn side_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let fill = if selected {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        egui::Color32::TRANSPARENT
    };
    let text = if selected { color::WHITE } else { color::TEXT };
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    let centre_y = rect.center().y;
    ui.painter().text(egui::pos2(rect.left() + 28.0, centre_y), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(15.0), if selected { color::WHITE } else { color::ACCENT });
    ui.painter().text(egui::pos2(rect.left() + 54.0, centre_y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(15.0), text);
    response
}

/// A card that says which folder of Home is missing.
pub fn missing_card(ui: &mut egui::Ui, message: &str) {
    card(ui, |ui| {
        ui.set_width(420.0_f32.min(ui.available_width()));
        badge(ui, "Folders needed", color::ROSE);
        ui.add_space(4.0);
        ui.label(hint(message));
    });
}

/// A page that is not written yet. Without the two folders of Home, it asks for them.
pub fn placeholder(ui: &mut egui::Ui, name: &str, text: &str, session: &sango_core::Session) {
    ui.add_space(8.0);
    ui.heading(name);
    ui.label(hint(text));
    ui.add_space(16.0);
    if let Some(message) = session.missing() {
        missing_card(ui, message);
        return;
    }
    card(ui, |ui| {
        ui.set_width(420.0_f32.min(ui.available_width()));
        badge(ui, "Not started", color::AMBER);
        ui.add_space(4.0);
        ui.label(hint("..."));
    });
}

/// The keys used in egui temp data to remember which widget has focus on the page.
const ACTIVE_WIDGET_KEY: &str = "sango_active_widget";
const PAGE_FOCUS_KEY: &str = "sango_page_focus";
const FOCUSABLE_WIDGETS_KEY: &str = "sango_focusable_widgets";
const FOCUSABLE_WIDGETS_NEXT_KEY: &str = "sango_focusable_widgets_next";
const EDITING_KEY: &str = "sango_is_editing";
const EDIT_BACKUP_KEY: &str = "sango_edit_backup";
const SCROLL_TO_ACTIVE_KEY: &str = "sango_scroll_to_active";

/// Requests that the active widget be scrolled into view.
pub fn request_scroll_to_active(ui: &egui::Ui) {
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(SCROLL_TO_ACTIVE_KEY), true));
}

/// Checks and consumes the request to scroll the active widget into view.
pub fn take_scroll_to_active(ui: &egui::Ui) -> bool {
    ui.ctx().data_mut(|d| {
        let key = egui::Id::new(SCROLL_TO_ACTIVE_KEY);
        let val = d.get_temp::<bool>(key).unwrap_or(false);
        if val {
            d.remove::<bool>(key);
        }
        val
    })
}

/// Scrolls the widget into view if a scroll was requested.
pub fn scroll_active_into_view(ui: &egui::Ui, response: &egui::Response) {
    if take_scroll_to_active(ui) {
        response.scroll_to_me(Some(egui::Align::Center));
    }
}

/// Sets whether the page currently has focus (false when side bar has focus).
pub fn set_page_focus(ctx: &egui::Context, focus: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(PAGE_FOCUS_KEY), focus));
}

/// Returns true if the page currently has focus.
pub fn page_has_focus(ui: &egui::Ui) -> bool {
    ui.data(|d| d.get_temp(egui::Id::new(PAGE_FOCUS_KEY)).unwrap_or(true))
}

/// Returns true if any field on the page is currently in Edit Mode.
pub fn is_editing(ui: &egui::Ui) -> bool {
    is_editing_ctx(ui.ctx())
}

/// Returns true if any field is in Edit Mode, from Context directly.
pub fn is_editing_ctx(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(egui::Id::new(EDITING_KEY)).unwrap_or(false))
}

/// Sets whether the active field is in Edit Mode.
pub fn set_editing(ui: &egui::Ui, editing: bool) {
    set_editing_ctx(ui.ctx(), editing);
}

/// Sets whether the active field is in Edit Mode, from Context directly.
pub fn set_editing_ctx(ctx: &egui::Context, editing: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(EDITING_KEY), editing));
}

/// Cancels Edit Mode and clears the backup value.
pub fn cancel_editing(ui: &egui::Ui) {
    set_editing(ui, false);
    ui.data_mut(|d| d.remove::<f64>(egui::Id::new(EDIT_BACKUP_KEY)));
}

/// Stores the value before editing so it can be restored on Escape.
pub fn set_edit_backup(ui: &egui::Ui, value: f64) {
    ui.data_mut(|d| d.insert_temp(egui::Id::new(EDIT_BACKUP_KEY), value));
}

/// Returns the backup value stored when Edit Mode began.
pub fn edit_backup(ui: &egui::Ui) -> Option<f64> {
    ui.data(|d| d.get_temp(egui::Id::new(EDIT_BACKUP_KEY)))
}

/// Returns the ID of the widget that currently has focus on the page.
pub fn active_id(ui: &egui::Ui) -> Option<egui::Id> {
    ui.data(|d| d.get_temp(egui::Id::new(ACTIVE_WIDGET_KEY)))
}

/// Sets the active widget ID on the page.
pub fn set_active_id(ui: &egui::Ui, id: egui::Id) {
    ui.data_mut(|d| d.insert_temp(egui::Id::new(ACTIVE_WIDGET_KEY), id));
}

/// Checks if this widget is the active one on the page.
pub fn is_active(ui: &egui::Ui, id: egui::Id) -> bool {
    active_id(ui) == Some(id)
}

/// Registers a widget as focusable in the current frame.
pub fn register_focusable(ui: &egui::Ui, id: egui::Id) {
    ui.ctx().data_mut(|d| {
        let key = egui::Id::new(FOCUSABLE_WIDGETS_NEXT_KEY);
        let list = d.get_temp_mut_or_default::<Vec<egui::Id>>(key);
        if !list.contains(&id) {
            list.push(id);
        }
    });
}

/// Transfers the focusable widgets collected in the previous frame and handles
/// Tab, Shift+Tab, ArrowUp, and ArrowDown navigation between widgets when not in Edit Mode.
pub fn update_navigation(ui: &mut egui::Ui) {
    if !page_has_focus(ui) {
        return;
    }

    let widgets: Vec<egui::Id> = ui.ctx().data_mut(|d| {
        let next_key = egui::Id::new(FOCUSABLE_WIDGETS_NEXT_KEY);
        let cur_key = egui::Id::new(FOCUSABLE_WIDGETS_KEY);
        let next = d.get_temp::<Vec<egui::Id>>(next_key).unwrap_or_default();
        d.insert_temp(cur_key, next.clone());
        d.remove::<Vec<egui::Id>>(next_key);
        next
    });

    if widgets.is_empty() {
        return;
    }

    let mut current_id = active_id(ui);
    if current_id.is_none() || !widgets.contains(&current_id.unwrap()) {
        set_active_id(ui, widgets[0]);
        current_id = Some(widgets[0]);
    }

    // Only navigate between fields when NOT in Edit Mode:
    if !is_editing(ui) {
        let mut step: isize = 0;
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                step -= 1;
            }
        });

        if step != 0 {
            if let Some(pos) = widgets.iter().position(|&id| Some(id) == current_id) {
                let count = widgets.len() as isize;
                let next = ((pos as isize + step).rem_euclid(count)) as usize;
                set_active_id(ui, widgets[next]);
                request_scroll_to_active(ui);
            }
        }
    }
}

/// A glow around a rectangle.
/// In Navigate Mode: soft pulsing accent indigo.
/// In Edit Mode: faster pulsing emerald green.
pub fn glow(ui: &egui::Ui, rect: egui::Rect, rounding: f32) {
    let editing = is_editing(ui);
    let time = ui.input(|i| i.time) as f32;
    let speed = if editing { 4.5 } else { 2.6 };
    let pulse = 0.5 + 0.5 * (time * speed).sin();
    let strength = 0.2 + 0.8 * pulse;
    for layer in 0..4 {
        let grow = 1.0 + layer as f32 * 1.6;
        let alpha = (strength * (170.0 - 38.0 * layer as f32)) as u8;
        let colour = if editing {
            egui::Color32::from_rgba_unmultiplied(16, 185, 129, alpha)
        } else {
            egui::Color32::from_rgba_unmultiplied(79, 70, 229, alpha)
        };
        ui.painter().rect_stroke(rect.expand(grow), Rounding::same(rounding + grow), Stroke::new(1.6_f32, colour));
    }
    ui.ctx().request_repaint();
}

/// Adds a step to a number and keeps it between the limits.
pub fn stepped(value: f64, delta: f64, min: f64, max: f64) -> f64 {
    (value + delta).clamp(min, max)
}

/// One of the two small arrow buttons of a spin box. Returns how many steps to do now:
/// one at the click, then more while the button stays pressed.
fn arrow_button(ui: &mut egui::Ui, up: bool) -> u32 {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 14.0), egui::Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        color::SURFACE_STRONG
    };
    ui.painter().rect_filled(rect, Rounding::same(4.0), fill);
    let ink = if response.is_pointer_button_down_on() { color::WHITE } else { color::TEXT };
    let c = rect.center();
    let (w, h) = (4.5, 2.5);
    let points = if up { vec![egui::pos2(c.x - w, c.y + h), egui::pos2(c.x + w, c.y + h), egui::pos2(c.x, c.y - h)] } else { vec![egui::pos2(c.x - w, c.y - h), egui::pos2(c.x + w, c.y - h), egui::pos2(c.x, c.y + h)] };
    ui.painter().add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
    // The first step is at the click. After a short wait, the button repeats while it is pressed.
    let id = response.id.with("hold");
    if response.is_pointer_button_down_on() {
        let now = ui.input(|i| i.time);
        let (start, last): (f64, f64) = ui.data_mut(|d| *d.get_temp_mut_or(id, (now, now)));
        if response.clicked() {
            return 1;
        }
        ui.ctx().request_repaint();
        if now - start > 0.45 && now - last > 0.05 {
            ui.data_mut(|d| d.insert_temp(id, (start, now)));
            return 1;
        }
        0
    } else {
        ui.data_mut(|d| d.remove::<(f64, f64)>(id));
        response.clicked() as u32
    }
}

/// A button that can be focused with keyboard navigation. Pressing Enter or Space clicks it.
pub fn button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> bool {
    button_styled(ui, text, None)
}

/// A styled button that can be focused with keyboard navigation. Pressing Enter or Space clicks it.
pub fn button_styled(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, fill: Option<egui::Color32>) -> bool {
    let id = ui.next_auto_id();
    register_focusable(ui, id);

    let mut b = egui::Button::new(text);
    if let Some(c) = fill {
        b = b.fill(c);
    }
    let response = ui.add(b);

    if response.clicked() {
        set_active_id(ui, id);
    }

    let mut clicked = response.clicked();
    if is_active(ui, id) && page_has_focus(ui) && !is_editing(ui) {
        glow(ui, response.rect, 6.0);
        scroll_active_into_view(ui, &response);
        let press = ui.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
        });
        if press {
            clicked = true;
        }
    }
    clicked
}

/// A number field with two arrow buttons. In Navigate Mode, Enter enters Edit Mode.
/// In Edit Mode, arrow keys increment/decrement, direct typing changes the value, Enter confirms,
/// and Escape cancels and restores the previous value.
pub fn spin<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, min: f64, max: f64) -> bool {
    let before = value.to_f64();
    let mut delta = 0.0;
    let id = ui.next_auto_id();
    register_focusable(ui, id);

    let res = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let drag_response = ui.add(egui::DragValue::new(value).range(min..=max).speed(0.5));
        let (up_steps, down_steps) = ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let up = arrow_button(ui, true);
            let down = arrow_button(ui, false);
            (up, down)
        }).inner;
        (drag_response, up_steps, down_steps)
    });

    let (drag_response, up_steps, down_steps) = res.inner;
    let total_rect = res.response.rect;

    let clicked = drag_response.clicked()
        || drag_response.dragged()
        || drag_response.has_focus()
        || up_steps > 0
        || down_steps > 0
        || res.response.interact(egui::Sense::click()).clicked();

    if clicked {
        set_active_id(ui, id);
        if !is_editing(ui) {
            set_editing(ui, true);
            set_edit_backup(ui, value.to_f64());
        }
    }

    if is_active(ui, id) && page_has_focus(ui) {
        glow(ui, total_rect, 4.0);
        scroll_active_into_view(ui, &res.response);

        if !is_editing(ui) {
            // Navigate Mode: Enter enters Edit Mode
            let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if enter {
                set_editing(ui, true);
                set_edit_backup(ui, value.to_f64());
                drag_response.request_focus();
                request_scroll_to_active(ui);
            }
        } else {
            // Edit Mode:
            // 1. Esc cancels and restores backup
            let esc = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if esc {
                if let Some(backup) = edit_backup(ui) {
                    *value = T::from_f64(backup);
                }
                ui.data_mut(|d| d.remove::<String>(drag_response.id));
                cancel_editing(ui);
            }

            // 2. Enter confirms and leaves Edit Mode
            let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if enter {
                set_editing(ui, false);
            }

            // 3. Arrow keys increment/decrement
            ui.input_mut(|i| {
                for (modifiers, unit) in [(egui::Modifiers::SHIFT, 10.0), (egui::Modifiers::NONE, 1.0)] {
                    if i.consume_key(modifiers, egui::Key::ArrowUp) {
                        delta += unit;
                    }
                    if i.consume_key(modifiers, egui::Key::ArrowDown) {
                        delta -= unit;
                    }
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                    delta += 10.0;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                    delta -= 10.0;
                }
            });
        }
    }

    let unit = if ui.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
    delta += unit * up_steps as f64;
    delta -= unit * down_steps as f64;

    if delta != 0.0 {
        *value = T::from_f64(stepped(value.to_f64(), delta, min, max));
        ui.data_mut(|d| d.remove::<String>(drag_response.id));
    }
    value.to_f64() != before
}

#[cfg(test)]
mod test {
    use super::*;
    use eframe::egui;

    #[test]
    fn a_step_stays_between_the_limits() {
        assert_eq!(stepped(10.0, 1.0, 0.0, 255.0), 11.0);
        assert_eq!(stepped(255.0, 1.0, 0.0, 255.0), 255.0);
        assert_eq!(stepped(0.0, -10.0, 0.0, 255.0), 0.0);
        assert_eq!(stepped(250.0, 10.0, 0.0, 255.0), 255.0);
    }

    #[test]
    fn page_focus_can_be_set_and_read() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(page_has_focus(ui));
                set_page_focus(ctx, false);
                assert!(!page_has_focus(ui));
                set_page_focus(ctx, true);
                assert!(page_has_focus(ui));
            });
        });
    }

    #[test]
    fn active_id_can_be_set_and_checked() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id1 = egui::Id::new("widget_1");
                let id2 = egui::Id::new("widget_2");
                assert_eq!(active_id(ui), None);
                assert!(!is_active(ui, id1));

                set_active_id(ui, id1);
                assert_eq!(active_id(ui), Some(id1));
                assert!(is_active(ui, id1));
                assert!(!is_active(ui, id2));

                set_active_id(ui, id2);
                assert_eq!(active_id(ui), Some(id2));
                assert!(!is_active(ui, id1));
                assert!(is_active(ui, id2));
            });
        });
    }

    #[test]
    fn spin_increments_with_arrow_up_when_active() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut hp: u8 = 45;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                set_editing(ui, true);
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });

        assert_eq!(hp, 46);
    }

    #[test]
    fn spin_decrements_with_arrow_down_when_active() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut catch_rate: u8 = 45;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                set_editing(ui, true);
                spin(ui, &mut catch_rate, 0.0, 255.0);
            });
        });

        assert_eq!(catch_rate, 44);
    }

    #[test]
    fn spin_enters_edit_mode_on_enter() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut hp: u8 = 45;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                assert!(!is_editing(ui));
                spin(ui, &mut hp, 0.0, 255.0);
                assert!(is_editing(ui));
            });
        });
    }

    #[test]
    fn spin_cancels_edit_and_restores_backup_on_escape() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut hp: u8 = 99;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                set_editing(ui, true);
                set_edit_backup(ui, 45.0);
                spin(ui, &mut hp, 0.0, 255.0);
                assert!(!is_editing(ui));
            });
        });

        assert_eq!(hp, 45);
    }

    #[test]
    fn spin_does_not_change_when_page_lacks_focus() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        let mut hp: u8 = 45;
        let _ = ctx.run(raw_input, |ctx| {
            set_page_focus(ctx, false);
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                set_editing(ui, true);
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });

        assert_eq!(hp, 45);
    }
}
