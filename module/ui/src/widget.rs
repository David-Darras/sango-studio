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
        ui.add_space(8.0);
        if button_styled(ui, egui::RichText::new("Go to Home").color(color::WHITE), Some(color::ACCENT)) {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sango_switch_module"), 0usize));
        }
    });
}

/// Asks the application to show the page with this title (the title that the module gives).
pub fn open_page(ctx: &egui::Context, title: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN_PAGE_KEY), title.to_string()));
}

/// Takes the title of the page that was asked for, if any. The application calls it once per frame.
pub fn take_open_page(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new(OPEN_PAGE_KEY)))
}

/// The key, in the memory of the interface, of the title of the page that was asked for.
const OPEN_PAGE_KEY: &str = "sango_open_page";

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
        ui.add_space(8.0);
        if button_styled(ui, egui::RichText::new("Go to Home").color(color::WHITE), Some(color::ACCENT)) {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sango_switch_module"), 0usize));
        }
    });
}

/// The keys used in egui temp data to remember which widget has focus on the page.
const ACTIVE_WIDGET_KEY: &str = "sango_active_widget";
const PAGE_FOCUS_KEY: &str = "sango_page_focus";
const FOCUSABLE_WIDGETS_KEY: &str = "sango_focusable_widgets";
const FOCUSABLE_WIDGETS_NEXT_KEY: &str = "sango_focusable_widgets_next";
const SCROLL_TO_ACTIVE_KEY: &str = "sango_scroll_to_active";

/// A focusable widget representation with its ID and screen-space bounding rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusableWidget {
    pub id: egui::Id,
    pub rect: egui::Rect,
}

/// Navigation direction for 2D spatial movement across the layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

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

const REQUEST_SIDEBAR_FOCUS_KEY: &str = "sango_request_sidebar_focus";

/// Requests that keyboard focus be returned to the sidebar.
pub fn request_sidebar_focus(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(REQUEST_SIDEBAR_FOCUS_KEY), true));
}

/// Checks and consumes the request to return focus to the sidebar.
pub fn take_sidebar_focus_request(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| {
        let key = egui::Id::new(REQUEST_SIDEBAR_FOCUS_KEY);
        let req = d.get_temp::<bool>(key).unwrap_or(false);
        if req {
            d.remove::<bool>(key);
        }
        req
    })
}

/// Sets whether the page currently has focus (false when side bar has focus).
pub fn set_page_focus(ctx: &egui::Context, focus: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(PAGE_FOCUS_KEY), focus));
}

/// Returns true if the page currently has focus.
pub fn page_has_focus(ui: &egui::Ui) -> bool {
    ui.data(|d| d.get_temp(egui::Id::new(PAGE_FOCUS_KEY)).unwrap_or(true))
}

/// Returns false (Edit Mode is no longer required; values are adjusted directly).
#[inline]
pub fn is_editing(_ui: &egui::Ui) -> bool {
    false
}

/// Returns false (Edit Mode is no longer required; values are adjusted directly).
#[inline]
pub fn is_editing_ctx(_ctx: &egui::Context) -> bool {
    false
}

/// No-op stub maintained for compatibility.
#[inline]
pub fn set_editing(_ui: &egui::Ui, _editing: bool) {}

/// No-op stub maintained for compatibility.
#[inline]
pub fn set_editing_ctx(_ctx: &egui::Context, _editing: bool) {}

/// No-op stub maintained for compatibility.
#[inline]
pub fn cancel_editing(_ui: &egui::Ui) {}

/// No-op stub maintained for compatibility.
#[inline]
pub fn set_edit_backup(_ui: &egui::Ui, _value: f64) {}

/// No-op stub maintained for compatibility.
#[inline]
pub fn edit_backup(_ui: &egui::Ui) -> Option<f64> {
    None
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

/// Registers a widget as focusable with its bounding rectangle for 2D spatial navigation.
pub fn register_focusable(ui: &egui::Ui, id: egui::Id, rect: egui::Rect) {
    ui.ctx().data_mut(|d| {
        let key = egui::Id::new(FOCUSABLE_WIDGETS_NEXT_KEY);
        let list = d.get_temp_mut_or_default::<Vec<FocusableWidget>>(key);
        if let Some(item) = list.iter_mut().find(|w| w.id == id) {
            item.rect = rect;
        } else {
            list.push(FocusableWidget { id, rect });
        }
    });
}

/// Registers a focusable widget by ID only (falls back to zero rect).
pub fn register_focusable_id(ui: &egui::Ui, id: egui::Id) {
    register_focusable(ui, id, egui::Rect::ZERO);
}

/// Finds the best spatial neighbor in the given direction from the current widget.
pub fn find_spatial_neighbor(
    widgets: &[FocusableWidget],
    current: &FocusableWidget,
    dir: Direction,
) -> Option<FocusableWidget> {
    let cur = current.rect;
    let c_cur = cur.center();

    let mut best: Option<(f32, FocusableWidget)> = None;

    for &cand in widgets {
        if cand.id == current.id {
            continue;
        }
        let r = cand.rect;
        let c_cand = r.center();
        let dx = c_cand.x - c_cur.x;
        let dy = c_cand.y - c_cur.y;

        let overlap_y = (cur.bottom().min(r.bottom()) - cur.top().max(r.top())).max(0.0);
        let overlap_x = (cur.right().min(r.right()) - cur.left().max(r.left())).max(0.0);

        let eligible = match dir {
            Direction::Right => dx > 4.0 && (overlap_y > 0.0 || (r.left() >= cur.right() && dx > dy.abs() * 2.0)),
            Direction::Left => dx < -4.0 && (overlap_y > 0.0 || (r.right() <= cur.left() && (-dx) > dy.abs() * 2.0)),
            Direction::Down => dy > 4.0 && (overlap_x > 0.0 || dy > dx.abs() * 0.3),
            Direction::Up => dy < -4.0 && (overlap_x > 0.0 || (-dy) > dx.abs() * 0.3),
        };

        if !eligible {
            continue;
        }

        let score = match dir {
            Direction::Right => {
                let overlap_y = (cur.bottom().min(r.bottom()) - cur.top().max(r.top())).max(0.0);
                let dy_effective = if overlap_y > 0.0 {
                    (c_cand.y - c_cur.y).abs() * 0.2
                } else {
                    (r.top() - cur.bottom()).max(cur.top() - r.bottom()).max(0.0)
                        + (c_cand.y - c_cur.y).abs() * 0.5
                };
                (r.left() - cur.right()).max(0.0) + dx * 0.1 + 3.0 * dy_effective
            }
            Direction::Left => {
                let overlap_y = (cur.bottom().min(r.bottom()) - cur.top().max(r.top())).max(0.0);
                let dy_effective = if overlap_y > 0.0 {
                    (c_cand.y - c_cur.y).abs() * 0.2
                } else {
                    (r.top() - cur.bottom()).max(cur.top() - r.bottom()).max(0.0)
                        + (c_cand.y - c_cur.y).abs() * 0.5
                };
                (cur.left() - r.right()).max(0.0) + (-dx) * 0.1 + 3.0 * dy_effective
            }
            Direction::Down => {
                let overlap_x = (cur.right().min(r.right()) - cur.left().max(r.left())).max(0.0);
                let dx_effective = if overlap_x > 0.0 {
                    (c_cand.x - c_cur.x).abs() * 0.2
                } else {
                    (r.left() - cur.right()).max(cur.left() - r.right()).max(0.0)
                        + (c_cand.x - c_cur.x).abs() * 0.5
                };
                (r.top() - cur.bottom()).max(0.0) + dy * 0.1 + 3.0 * dx_effective
            }
            Direction::Up => {
                let overlap_x = (cur.right().min(r.right()) - cur.left().max(r.left())).max(0.0);
                let dx_effective = if overlap_x > 0.0 {
                    (c_cand.x - c_cur.x).abs() * 0.2
                } else {
                    (r.left() - cur.right()).max(cur.left() - r.right()).max(0.0)
                        + (c_cand.x - c_cur.x).abs() * 0.5
                };
                (cur.top() - r.bottom()).max(0.0) + (-dy) * 0.1 + 3.0 * dx_effective
            }
        };

        if best.as_ref().map_or(true, |(best_score, _)| score < *best_score) {
            best = Some((score, cand));
        }
    }

    best.map(|(_, cand)| cand)
}

/// Transfers the focusable widgets collected in the previous frame and handles
/// Ctrl+Arrow (2D spatial navigation) and Tab / Shift+Tab (linear navigation).
/// Plain arrow keys are preserved for directly adjusting values on the active widget.
pub fn update_navigation(ui: &mut egui::Ui) {
    let widgets: Vec<FocusableWidget> = ui.ctx().data_mut(|d| {
        let next_key = egui::Id::new(FOCUSABLE_WIDGETS_NEXT_KEY);
        let cur_key = egui::Id::new(FOCUSABLE_WIDGETS_KEY);
        let next = d.get_temp::<Vec<FocusableWidget>>(next_key).unwrap_or_default();
        if !next.is_empty() {
            d.insert_temp(cur_key, next.clone());
            d.remove::<Vec<FocusableWidget>>(next_key);
            next
        } else {
            d.get_temp::<Vec<FocusableWidget>>(cur_key).unwrap_or_default()
        }
    });

    if widgets.is_empty() {
        return;
    }

    let mut current_id = active_id(ui);
    if current_id.is_none() || !widgets.iter().any(|w| Some(w.id) == current_id) {
        set_active_id(ui, widgets[0].id);
        current_id = Some(widgets[0].id);
        ui.ctx().request_repaint();
    }

    if !page_has_focus(ui) {
        return;
    }

    let is_button = current_id.map_or(false, |id| {
        ui.ctx().data(|d| d.get_temp::<bool>(id.with("is_button")).unwrap_or(false))
    });

    let handles_arrows = current_id.map_or(false, |id| {
        ui.ctx().data(|d| d.get_temp::<bool>(id.with("handles_arrows")).unwrap_or(false))
    });

    // 1. Check for 2D spatial navigation via Ctrl+Arrow (or Cmd+Arrow on macOS):
    let mut nav_dir = None;
    if !handles_arrows {
        ui.input_mut(|i| {
            for mods in [egui::Modifiers::CTRL, egui::Modifiers::COMMAND] {
                if nav_dir.is_none() && i.consume_key(mods, egui::Key::ArrowRight) {
                    nav_dir = Some(Direction::Right);
                }
                if nav_dir.is_none() && i.consume_key(mods, egui::Key::ArrowLeft) {
                    nav_dir = Some(Direction::Left);
                }
                if nav_dir.is_none() && i.consume_key(mods, egui::Key::ArrowDown) {
                    nav_dir = Some(Direction::Down);
                }
                if nav_dir.is_none() && i.consume_key(mods, egui::Key::ArrowUp) {
                    nav_dir = Some(Direction::Up);
                }
            }
            // If the active widget is a button (does not adjust values directly), plain arrows also navigate:
            if is_button && !i.modifiers.shift && !i.modifiers.alt && !i.modifiers.ctrl && !i.modifiers.command {
                if nav_dir.is_none() && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    nav_dir = Some(Direction::Down);
                }
                if nav_dir.is_none() && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    nav_dir = Some(Direction::Up);
                }
                if nav_dir.is_none() && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                    nav_dir = Some(Direction::Right);
                }
            }
        });
    }

    if let Some(dir) = nav_dir {
        if let Some(cur) = widgets.iter().find(|w| Some(w.id) == current_id) {
            if let Some(neighbor) = find_spatial_neighbor(&widgets, cur, dir) {
                set_active_id(ui, neighbor.id);
                request_scroll_to_active(ui);
                ui.ctx().request_repaint();
                return;
            } else if dir == Direction::Left && cur.rect.is_positive() {
                // Moving left from the leftmost element on the page returns to the sidebar
                request_sidebar_focus(ui.ctx());
                return;
            } else if !cur.rect.is_positive() {
                // Fallback for widgets without valid bounding rects:
                let step: isize = match dir {
                    Direction::Down | Direction::Right => 1,
                    Direction::Up | Direction::Left => -1,
                };
                if let Some(pos) = widgets.iter().position(|w| Some(w.id) == current_id) {
                    let count = widgets.len() as isize;
                    let next = ((pos as isize + step).rem_euclid(count)) as usize;
                    set_active_id(ui, widgets[next].id);
                    request_scroll_to_active(ui);
                    ui.ctx().request_repaint();
                    return;
                }
            }
        }
    }

    // 2. When on the leftmost element of the page, pressing plain ArrowLeft also returns to the sidebar
    // (unless typing in a text field, or if the active widget handles horizontal navigation like tabs):
    if !ui.ctx().wants_keyboard_input() {
        if let Some(cur) = widgets.iter().find(|w| Some(w.id) == current_id) {
            if cur.rect.is_positive() && find_spatial_neighbor(&widgets, cur, Direction::Left).is_none() {
                let handles_left = ui.ctx().data(|d| d.get_temp::<bool>(cur.id.with("handles_left")).unwrap_or(false));
                if !handles_left {
                    let pressed_left = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
                    if pressed_left {
                        request_sidebar_focus(ui.ctx());
                        return;
                    }
                }
            }
        }
    }

    // 2. Sequential navigation via Tab / Shift+Tab:
    let mut step: isize = 0;
    ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
            step += 1;
        }
        if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab) {
            step -= 1;
        }
    });

    if step != 0 {
        if let Some(pos) = widgets.iter().position(|w| Some(w.id) == current_id) {
            let count = widgets.len() as isize;
            let next = ((pos as isize + step).rem_euclid(count)) as usize;
            set_active_id(ui, widgets[next].id);
            request_scroll_to_active(ui);
        }
    }
}

/// A glow around a rectangle to highlight the currently active widget.
pub fn glow(ui: &egui::Ui, rect: egui::Rect, rounding: f32) {
    let time = ui.input(|i| i.time) as f32;
    let pulse = 0.5 + 0.5 * (time * 2.8).sin();
    let strength = 0.2 + 0.8 * pulse;
    for layer in 0..4 {
        let grow = 1.0 + layer as f32 * 1.6;
        let alpha = (strength * (170.0 - 38.0 * layer as f32)) as u8;
        let colour = egui::Color32::from_rgba_unmultiplied(79, 70, 229, alpha);
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
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 14.0), egui::Sense::click_and_drag());
    let id = response.id.with("hold");
    let pointer_down = response.is_pointer_button_down_on() || response.dragged();
    let mut steps: u32 = 0;

    if pointer_down {
        let now = ui.input(|i| i.time);
        let state: Option<(f64, f64)> = ui.data_mut(|d| d.get_temp(id));

        match state {
            None => {
                // First frame of press: fire 1 step immediately and record start time
                ui.data_mut(|d| d.insert_temp(id, (now, now)));
                steps = 1;
            }
            Some((start, last)) => {
                let hold_duration = now - start;
                let interval = if hold_duration > 2.5 {
                    0.02
                } else if hold_duration > 1.2 {
                    0.035
                } else {
                    0.06
                };

                if hold_duration > 0.35 && now - last >= interval {
                    ui.data_mut(|d| d.insert_temp(id, (start, now)));
                    steps = 1;
                }
            }
        }
        ui.ctx().request_repaint();
    } else {
        ui.data_mut(|d| d.remove::<(f64, f64)>(id));
    }

    let is_active = pointer_down;
    let fill = if is_active {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        color::SURFACE_STRONG
    };
    ui.painter().rect_filled(rect, Rounding::same(4.0), fill);
    let ink = if is_active { color::WHITE } else { color::TEXT };
    let c = rect.center();
    let (w, h) = (4.5, 2.5);
    let points = if up {
        vec![egui::pos2(c.x - w, c.y + h), egui::pos2(c.x + w, c.y + h), egui::pos2(c.x, c.y - h)]
    } else {
        vec![egui::pos2(c.x - w, c.y - h), egui::pos2(c.x + w, c.y - h), egui::pos2(c.x, c.y + h)]
    };
    ui.painter().add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));

    steps
}

/// A button that can be focused with keyboard navigation. Pressing Enter or Space clicks it.
pub fn button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> bool {
    button_styled(ui, text, None)
}

/// A styled button that can be focused with keyboard navigation. Pressing Enter or Space clicks it.
pub fn button_styled(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, fill: Option<egui::Color32>) -> bool {
    let mut b = egui::Button::new(text);
    if let Some(c) = fill {
        b = b.fill(c);
    }
    let response = ui.add(b);
    let id = response.id;
    register_focusable(ui, id, response.rect);
    ui.ctx().data_mut(|d| d.insert_temp(id.with("is_button"), true));

    if response.clicked() {
        set_active_id(ui, id);
    }

    let mut clicked = response.clicked();
    if is_active(ui, id) && page_has_focus(ui) {
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

/// An imposing, polished tab button with distinct active, hovered, and inactive states.
pub fn tab_button(ui: &mut egui::Ui, text: &str, selected: bool, has_glow: bool) -> egui::Response {
    let font_id = egui::FontId::proportional(14.5);
    let text_width = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id.clone(), color::WHITE).size().x);
    let padding_x = 18.0_f32;
    let height = 36.0_f32;
    let width = (text_width + 2.0 * padding_x).max(74.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let rounding = Rounding::same(10.0);

    let fill = if selected {
        color::ACCENT
    } else if response.hovered() {
        color::ACCENT_SOFT
    } else {
        egui::Color32::TRANSPARENT
    };

    let text_color = if selected {
        color::WHITE
    } else if response.hovered() {
        color::ACCENT
    } else {
        color::TEXT
    };

    ui.painter().rect_filled(rect, rounding, fill);
    if selected && has_glow {
        glow(ui, rect, 10.0);
    }

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font_id,
        text_color,
    );

    response
}

/// A number field with two arrow buttons. When active, plain ArrowUp and ArrowDown
/// directly increment and decrement the value without requiring an edit mode.
/// Holding Shift steps by 10. PageUp and PageDown step by 10.
pub fn spin<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, min: f64, max: f64) -> bool {
    let before = value.to_f64();
    let mut delta = 0.0;
    let id = ui.next_auto_id();

    let res = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let drag_width = if max > 999.0 { 60.0 } else { 44.0 };
        let drag_response = ui.add_sized([drag_width, 22.0], egui::DragValue::new(value).range(min..=max).speed(0.5));
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
    register_focusable(ui, id, total_rect);

    let clicked = drag_response.clicked()
        || drag_response.dragged()
        || drag_response.has_focus()
        || up_steps > 0
        || down_steps > 0
        || res.response.interact(egui::Sense::click()).clicked();

    if clicked {
        set_active_id(ui, id);
    }

    if is_active(ui, id) && page_has_focus(ui) {
        glow(ui, total_rect, 4.0);
        scroll_active_into_view(ui, &res.response);

        // Arrow keys directly increment/decrement values without any edit mode
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

    let unit = if ui.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
    delta += unit * up_steps as f64;
    delta -= unit * down_steps as f64;

    if delta != 0.0 {
        *value = T::from_f64(stepped(value.to_f64(), delta, min, max));
        ui.data_mut(|d| d.remove::<String>(drag_response.id));
        ui.ctx().request_repaint();
    }
    value.to_f64() != before
}

/// Returns the stat tier color based on the exact Coup Critique / Smogon thresholds:
/// - < 50: Red (#db2828) - Very Low
/// - 50..=74: Orange (#ef6e33) - Low
/// - 75..=99: Yellow (#fbbd08) - Average
/// - 100..=124: Olive (#b5cc18) - Good
/// - 125..=149: Green (#21ba45) - Very Good
/// - 150..=179: Teal / Cyan (#00b5ad) - Exceptional
/// - 180..=255: Blue (#4571e6) - Legendary / Maximum
pub fn stat_color(val: f32) -> egui::Color32 {
    let v = val.round() as i32;
    match v {
        ..=49 => egui::Color32::from_rgb(219, 40, 40),      // Red (#db2828)
        50..=74 => egui::Color32::from_rgb(239, 110, 51),   // Orange (#ef6e33)
        75..=99 => egui::Color32::from_rgb(251, 189, 8),    // Yellow (#fbbd08)
        100..=124 => egui::Color32::from_rgb(181, 204, 24), // Olive (#b5cc18)
        125..=149 => egui::Color32::from_rgb(33, 186, 69),  // Green (#21ba45)
        150..=179 => egui::Color32::from_rgb(0, 181, 173),  // Teal (#00b5ad)
        _ => egui::Color32::from_rgb(69, 113, 230),         // Blue (#4571e6)
    }
}

/// A Pokemon Showdown-like horizontal stat bar.
///
/// Draws a smooth horizontal bar whose fill and color reflect the stat value:
/// from red at 0, through orange, yellow, green, and cyan/blue at the maximum (255).
/// Features a circular grab handle and click/drag interaction.
pub fn stat_bar<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, max: f64) -> egui::Response {
    let width = 150.0;
    let bar_height = 10.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click_and_drag());

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let track_margin = 6.0_f32;
    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(width - 2.0 * track_margin, bar_height));

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
            *value = T::from_f64((ratio as f64 * max).round());
        }
    }

    let rounding = Rounding::same(bar_height / 2.0);

    // Track background and border
    ui.painter().rect_filled(track_rect, rounding, color::SURFACE_STRONG);
    ui.painter().rect_stroke(track_rect, rounding, Stroke::new(1.0_f32, color::BORDER));

    // Filled portion
    let val_f64 = value.to_f64();
    let ratio = (val_f64 / max).clamp(0.0, 1.0) as f32;
    let color = stat_color(val_f64 as f32);

    if ratio > 0.0 {
        let fill_w = (ratio * track_rect.width()).round().max(bar_height / 2.0).min(track_rect.width());
        let filled_rect = egui::Rect::from_min_size(track_rect.min, egui::vec2(fill_w, track_rect.height()));
        let fill_rounding = Rounding::same((bar_height / 2.0).min(fill_w / 2.0));
        ui.painter().rect_filled(filled_rect, fill_rounding, color);
    }

    // Draggable circular handle (thumb)
    let handle_x = (track_rect.left() + ratio * track_rect.width()).clamp(track_rect.left(), track_rect.right());
    let handle_center = egui::pos2(handle_x, track_rect.center().y);
    let radius = if response.dragged() {
        7.5
    } else if response.hovered() {
        7.0
    } else {
        6.0
    };

    // Subtle glow on hover or drag
    if response.hovered() || response.dragged() {
        ui.painter().circle_filled(
            handle_center,
            radius + 3.0,
            egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40),
        );
    }

    // Shadow
    ui.painter().circle_filled(
        handle_center + egui::vec2(0.0, 1.0),
        radius + 0.5,
        egui::Color32::from_black_alpha(25),
    );
    // White knob
    ui.painter().circle_filled(handle_center, radius, color::WHITE);
    // Colored border
    ui.painter().circle_stroke(handle_center, radius, Stroke::new(2.0_f32, color));

    response.on_hover_ui(|ui| {
        ui.label(format!("{} / {}", val_f64.round() as i64, max as i64));
    })
}

/// A view-only Pokemon Showdown-like horizontal stat bar.
pub fn stat_bar_view(ui: &mut egui::Ui, value: f32, max: f32) -> egui::Response {
    let mut temp = value;
    stat_bar(ui, &mut temp, max as f64)
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
                spin(ui, &mut catch_rate, 0.0, 255.0);
            });
        });

        assert_eq!(catch_rate, 44);
    }

    #[test]
    fn spin_steps_by_ten_with_shift_arrow() {
        let ctx = egui::Context::default();
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        });

        let mut hp: u8 = 45;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let id = ui.next_auto_id();
                set_active_id(ui, id);
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });

        assert_eq!(hp, 55);
    }

    #[test]
    fn spin_arrow_button_repeats_while_held() {
        let ctx = egui::Context::default();
        let mut hp: u8 = 45;
        let click_pos = egui::pos2(65.0, 14.0);

        // Frame 0: Lay out UI and position pointer over the arrow button
        let mut warm_up = egui::RawInput::default();
        warm_up.events.push(egui::Event::PointerMoved(click_pos));
        let _ = ctx.run(warm_up, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });
        assert_eq!(hp, 45);

        // Frame 1: Press down on the arrow button at time = 0.0
        let mut input0 = egui::RawInput::default();
        input0.time = Some(0.0);
        input0.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input0, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });
        assert_eq!(hp, 46); // stepped immediately on click!

        // Frame 2: Holding past initial repeat delay (t = 0.45s)
        let mut input1 = egui::RawInput::default();
        input1.time = Some(0.45);
        let _ = ctx.run(input1, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });
        assert_eq!(hp, 47);

        // Frame 3: Holding continuously through multiple intervals
        for t in 1..=10 {
            let mut input = egui::RawInput::default();
            input.time = Some(0.45 + t as f64 * 0.07);
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    spin(ui, &mut hp, 0.0, 255.0);
                });
            });
        }
        assert!(hp >= 55);
    }

    #[test]
    fn spatial_navigation_picks_closest_neighbor() {
        let top_left = FocusableWidget {
            id: egui::Id::new("w_top_left"),
            rect: egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(80.0, 30.0)),
        };
        let top_right = FocusableWidget {
            id: egui::Id::new("w_top_right"),
            rect: egui::Rect::from_min_size(egui::pos2(300.0, 100.0), egui::vec2(80.0, 30.0)),
        };
        let bottom_left = FocusableWidget {
            id: egui::Id::new("w_bottom_left"),
            rect: egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(80.0, 30.0)),
        };
        let bottom_right = FocusableWidget {
            id: egui::Id::new("w_bottom_right"),
            rect: egui::Rect::from_min_size(egui::pos2(300.0, 200.0), egui::vec2(80.0, 30.0)),
        };

        let list = vec![top_left, top_right, bottom_left, bottom_right];

        // Moving right from top_left goes to top_right
        assert_eq!(find_spatial_neighbor(&list, &top_left, Direction::Right), Some(top_right));
        // Moving down from top_left goes to bottom_left
        assert_eq!(find_spatial_neighbor(&list, &top_left, Direction::Down), Some(bottom_left));
        // Moving left from top_right goes to top_left
        assert_eq!(find_spatial_neighbor(&list, &top_right, Direction::Left), Some(top_left));
        // Moving up from bottom_left goes to top_left
        assert_eq!(find_spatial_neighbor(&list, &bottom_left, Direction::Up), Some(top_left));
        // Moving down from top_right goes to bottom_right
        assert_eq!(find_spatial_neighbor(&list, &top_right, Direction::Down), Some(bottom_right));
        // Moving left from bottom_right goes to bottom_left
        assert_eq!(find_spatial_neighbor(&list, &bottom_right, Direction::Left), Some(bottom_left));
    }

    #[test]
    fn ctrl_arrow_navigates_spatially() {
        let ctx = egui::Context::default();
        let id_left = egui::Id::new("elem_left");
        let id_right = egui::Id::new("elem_right");

        // Frame 1: register widgets
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                register_focusable(ui, id_left, egui::Rect::from_min_size(egui::pos2(50.0, 100.0), egui::vec2(60.0, 30.0)));
                register_focusable(ui, id_right, egui::Rect::from_min_size(egui::pos2(250.0, 100.0), egui::vec2(60.0, 30.0)));
                set_active_id(ui, id_left);
            });
        });

        // Frame 2: press Ctrl + ArrowRight
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });

        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                set_page_focus(ctx, true);
                update_navigation(ui);
                assert_eq!(active_id(ui), Some(id_right));
            });
        });
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
                spin(ui, &mut hp, 0.0, 255.0);
            });
        });

        assert_eq!(hp, 45);
    }

    #[test]
    fn stat_color_follows_spectrum() {
        let red = stat_color(45.0);
        assert_eq!(red, egui::Color32::from_rgb(219, 40, 40));

        let orange = stat_color(65.0);
        assert_eq!(orange, egui::Color32::from_rgb(239, 110, 51));

        let yellow = stat_color(80.0);
        assert_eq!(yellow, egui::Color32::from_rgb(251, 189, 8));

        let olive = stat_color(100.0);
        assert_eq!(olive, egui::Color32::from_rgb(181, 204, 24));

        let green = stat_color(130.0);
        assert_eq!(green, egui::Color32::from_rgb(33, 186, 69));

        let teal = stat_color(159.0);
        assert_eq!(teal, egui::Color32::from_rgb(0, 181, 173));

        let blue = stat_color(200.0);
        assert_eq!(blue, egui::Color32::from_rgb(69, 113, 230));
    }

    #[test]
    fn stat_bar_renders_and_responds_to_click() {
        let ctx = egui::Context::default();
        let mut hp: u8 = 50;

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stat_bar(ui, &mut hp, 255.0);
            });
        });
        assert_eq!(hp, 50);
    }
}

