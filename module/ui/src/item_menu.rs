//! A drop-down list of items, with the picture of each item: the same list in the Item page and in the fields of the
//! other pages that hold an item (the held items of a species).
//!
//! The button shows the picture and the name of the chosen item. The list has a search box, a header that sorts the list
//! when it is clicked, the arrow keys, Enter, and it shows the chosen item at the middle when it opens.

use crate::theme::color;
use crate::widget;
use eframe::egui;
use sango_core::item_icons;
use sango_core::{Dump, Garc};
use std::collections::HashMap;

/// The pockets of the Bag, in the order of their number.
pub const POCKETS: [&str; 5] = ["Items", "Medicine", "TMs & HMs", "Berries", "Key Items"];

/// What the list shows for one item. The rows are in the order of the item numbers, the row 0 is for "no item".
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub name: String,
    /// The number of the pocket of the Bag.
    pub pocket: u8,
    /// The price in Poké Dollars.
    pub price: u32,
}

/// The pictures of the items as textures of the window, decoded at the first display and then kept.
#[derive(Default)]
pub struct IconCache {
    garc: Option<Garc>,
    textures: HashMap<usize, Option<egui::TextureHandle>>,
}

impl IconCache {
    /// Opens the archive of the pictures of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.garc = Garc::open(&dump.archive(item_icons::ARCHIVE)).ok();
        self.textures.clear();
    }

    /// The texture of the picture of an item, when it has one.
    pub fn texture(&mut self, ctx: &egui::Context, item: usize) -> Option<egui::TextureHandle> {
        if let Some(found) = self.textures.get(&item) {
            return found.clone();
        }
        let made = item_icons::file_of(item)
            .and_then(|file| self.garc.as_ref()?.first_file(file))
            .and_then(|bytes| item_icons::decode(bytes).ok())
            .map(|rgba| ctx.load_texture(format!("item_icon_{item}"), egui::ColorImage::from_rgba_unmultiplied([item_icons::SIZE, item_icons::SIZE], &rgba), egui::TextureOptions::NEAREST));
        self.textures.insert(item, made.clone());
        made
    }
}

/// Reads the rows of the list from a dump: the English names, and the price and the pocket of each item.
pub fn rows_from_dump(dump: &Dump) -> Vec<Row> {
    let data = Garc::open(&dump.archive(sango_core::item_icons::DATA_ARCHIVE)).ok();
    let names = Garc::open(&dump.archive(sango_core::text::LANGUAGES[2].archive)).ok().and_then(|g| g.first_file(item_icons::NAME_FILE).and_then(|b| sango_core::text::TextFile::from_bytes(b).ok()));
    (0..=item_icons::COUNT)
        .map(|item| {
            let bytes = data.as_ref().and_then(|g| g.first_file(item));
            let (price, pocket) = bytes.map_or((0, 0), item_icons::price_and_pocket);
            Row { name: if item == 0 { "None".to_string() } else { names.as_ref().and_then(|f| f.text(item)).unwrap_or_default() }, pocket, price }
        })
        .collect()
}

/// The rows and the pictures of the list, to share between frames (and between the fields of a page).
#[derive(Default)]
pub struct ItemList {
    pub rows: Vec<Row>,
    pub icons: IconCache,
}

impl ItemList {
    /// Reads the rows and opens the pictures of a dump.
    pub fn open(&mut self, dump: &Dump) {
        self.rows = rows_from_dump(dump);
        self.icons.open(dump);
    }
}

/// The list of items of a shared [`ItemList`]. See [`item_menu`].
pub fn shared_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut usize, width: f32, list: &std::sync::Arc<std::sync::Mutex<ItemList>>, none: bool) -> bool {
    let Ok(mut list) = list.lock() else { return false };
    let ItemList { rows, icons } = &mut *list;
    item_menu(ui, id_salt, value, width, rows, &mut |ctx, n| icons.texture(ctx, n), none)
}

/// A column of the list. A click on its header sorts the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortColumn {
    Number,
    Name,
    Pocket,
    Price,
}

/// The columns of the list: x position, width, name. The picture is before the first column.
const COLUMNS: [(f32, f32, &str, SortColumn); 4] = [(36.0, 48.0, "#", SortColumn::Number), (84.0, 230.0, "Name", SortColumn::Name), (330.0, 100.0, "Pocket", SortColumn::Pocket), (430.0, 90.0, "Price", SortColumn::Price)];

const LIST_WIDTH: f32 = 520.0;
const ROW_HEIGHT: f32 = 32.0;

/// What the list keeps between two frames: the sort, the search and whether the list was already shown.
#[derive(Clone, Default)]
struct State {
    sort: Option<(SortColumn, bool)>,
    search: String,
    searched: bool,
    shown: bool,
}

/// A drop-down list of items. `value` is the item number (0 is no item). The button is `width` points wide.
/// `none` says whether the list has the choice "None". Gives true when the item changed.
pub fn item_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut usize, width: f32, rows: &[Row], icons: &mut dyn FnMut(&egui::Context, usize) -> Option<egui::TextureHandle>, none: bool) -> bool {
    let before = *value;
    let last = rows.len().saturating_sub(1);
    let first = if none { 0 } else { 1 };
    let id = ui.make_persistent_id(id_salt);
    let popup_id = id.with("popup");
    let state_id = id.with("state");
    let mut state: State = ui.data_mut(|d| d.get_temp(state_id).unwrap_or_default());
    let open = ui.memory(|m| m.is_popup_open(popup_id));

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
    let button = ui.interact(rect, id, egui::Sense::click());
    let visuals = if open { ui.visuals().widgets.open } else { *ui.style().interact(&button) };
    ui.painter().rect(rect.expand(visuals.expansion), visuals.rounding, visuals.weak_bg_fill, visuals.bg_stroke);
    let mut text_left = 8.0;
    if let Some(texture) = icons(ui.ctx(), *value) {
        let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), egui::Rect::from_min_size(egui::pos2(rect.left() + 4.0, rect.center().y - 12.0), egui::vec2(24.0, 24.0)), full, egui::Color32::WHITE);
        text_left = 34.0;
    }
    let name = rows.get(*value).map(|r| r.name.as_str()).unwrap_or("Unknown");
    let galley = egui::WidgetText::from(format!("#{:03} {name}", *value)).into_galley(ui, Some(egui::TextWrapMode::Truncate), (width - text_left - 24.0).max(20.0), egui::TextStyle::Button);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(egui::pos2(rect.left() + text_left, top), galley, visuals.text_color());
    let arrow = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), egui::vec2(8.0, 5.0));
    ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], visuals.fg_stroke.color, egui::Stroke::NONE));
    if button.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        widget::set_active_id(ui, id);
    }
    widget::register_focusable(ui, id, button.rect);

    // The button is focused: Enter or Space open the list, Up and Down change the item.
    if widget::is_active(ui, id) && widget::page_has_focus(ui) && !open {
        widget::glow(ui, button.rect, 6.0);
        widget::scroll_active_into_view(ui, &button);
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            ui.memory_mut(|m| m.open_popup(popup_id));
        }
        ui.input_mut(|i| {
            let step = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize;
            if step != 0 {
                *value = (*value as isize + step).clamp(first as isize, last as isize) as usize;
            }
        });
    }

    let shown = egui::popup::popup_below_widget(ui, popup_id, &button, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(LIST_WIDTH);
        ui.set_max_width(LIST_WIDTH);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(widget::hint("🔍"));
            let input = ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search name or number...").desired_width(300.0).clip_text(true));
            if !state.searched {
                input.request_focus();
                state.searched = true;
            }
            if !state.search.is_empty() && ui.small_button("✕").clicked() {
                state.search.clear();
            }
        });
        ui.separator();
        let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
        let query = clean(state.search.trim());
        let number_query = query.trim_start_matches('#').trim_start_matches('0').to_string();
        let mut matches: Vec<usize> = (first..=last).filter(|&n| query.is_empty() || (n == 0 && "none".starts_with(&query)) || clean(&rows[n].name).contains(&query) || (!number_query.is_empty() && n.to_string() == number_query)).collect();

        header(ui, &mut state);
        ui.separator();
        let sort = state.sort;
        matches.sort_by(|&a, &b| {
            let order = match sort {
                None | Some((SortColumn::Number, _)) => a.cmp(&b),
                Some((SortColumn::Name, _)) => rows[a].name.cmp(&rows[b].name),
                Some((SortColumn::Pocket, _)) => rows[a].pocket.cmp(&rows[b].pocket),
                Some((SortColumn::Price, _)) => rows[a].price.cmp(&rows[b].price),
            };
            let order = if sort.is_some_and(|(_, descending)| descending) { order.reverse() } else { order };
            order.then(a.cmp(&b))
        });
        if matches.is_empty() {
            ui.add_space(4.0);
            ui.label(widget::hint("No matching item."));
            return;
        }
        let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if let Some(place) = matches.iter().position(|&n| n == *value) {
            if down && place + 1 < matches.len() {
                *value = matches[place + 1];
            }
            if up && place > 0 {
                *value = matches[place - 1];
            }
        } else if up || down {
            *value = matches[0];
        }
        if enter {
            if !matches.contains(value) {
                *value = matches[0];
            }
            ui.memory_mut(|m| m.close_popup());
        }
        let highlighted = if matches.contains(value) { *value } else { matches[0] };
        let place = matches.iter().position(|&n| n == highlighted);
        // The list shows the highlighted item at the middle after a key, and at the opening.
        let centre = place.filter(|_| up || down || (query.is_empty() && !state.shown));
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().id_salt(id.with("list")).max_height(380.0);
        if let Some(place) = centre {
            area = area.vertical_scroll_offset(((place as f32 + 0.5) * ROW_HEIGHT - 190.0).max(0.0));
        }
        area.show_rows(ui, ROW_HEIGHT, matches.len(), |ui, range| {
            for place in range {
                let number = matches[place];
                let (row, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, ROW_HEIGHT), egui::Sense::click());
                if !ui.is_rect_visible(row) {
                    continue;
                }
                if number == highlighted {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::ACCENT_SOFT);
                    widget::glow(ui, row, 6.0);
                } else if response.hovered() {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if let Some(texture) = icons(ui.ctx(), number) {
                    let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(texture.id(), egui::Rect::from_min_size(egui::pos2(row.left() + 4.0, row.center().y - 12.0), egui::vec2(24.0, 24.0)), full, egui::Color32::WHITE);
                }
                let y = row.center().y;
                ui.painter().text(egui::pos2(row.left() + COLUMNS[0].0, y), egui::Align2::LEFT_CENTER, format!("#{number:03}"), egui::FontId::proportional(12.0), color::MUTED);
                ui.painter().text(egui::pos2(row.left() + COLUMNS[1].0, y), egui::Align2::LEFT_CENTER, &rows[number].name, egui::FontId::proportional(13.5), color::TEXT);
                if number != 0 {
                    let pocket = POCKETS.get(rows[number].pocket as usize).copied().unwrap_or("-");
                    ui.painter().text(egui::pos2(row.left() + COLUMNS[2].0, y), egui::Align2::LEFT_CENTER, pocket, egui::FontId::proportional(12.0), color::MUTED);
                    ui.painter().text(egui::pos2(row.right() - 10.0, y), egui::Align2::RIGHT_CENTER, rows[number].price.to_string(), egui::FontId::proportional(12.5), color::TEXT);
                }
                if response.clicked() {
                    *value = number;
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
        state.shown = true;
    });
    if shown.is_none() {
        state = State::default();
    }
    ui.data_mut(|d| d.insert_temp(state_id, state));
    *value != before
}

/// The header of the list: a click on a name sorts the list on that column; a second click reverses the order.
fn header(ui: &mut egui::Ui, state: &mut State) {
    let (bar, _) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, 20.0), egui::Sense::hover());
    for (x, width, label, column) in COLUMNS {
        let cell = egui::Rect::from_min_size(egui::pos2(bar.left() + x, bar.top()), egui::vec2(width, 20.0));
        let response = ui.interact(cell, ui.make_persistent_id(("item_menu_header", label)), egui::Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let active = state.sort.is_some_and(|(c, _)| c == column);
        let right = column == SortColumn::Price;
        let (position, anchor) = if right { (egui::pos2(cell.right() - 10.0, cell.center().y), egui::Align2::RIGHT_CENTER) } else { (egui::pos2(cell.left(), cell.center().y), egui::Align2::LEFT_CENTER) };
        let ink = if active { color::ACCENT } else { color::MUTED };
        let text = ui.painter().text(position, anchor, label, egui::FontId::proportional(12.0), ink);
        if active {
            let descending = state.sort.is_some_and(|(_, d)| d);
            // A small triangle drawn by hand: the font has no arrow glyph.
            let centre_x = if right { text.left() - 8.0 } else { text.right() + 8.0 };
            let centre_y = cell.center().y;
            let (w, h) = (3.5, 2.5);
            let points = if descending {
                vec![egui::pos2(centre_x - w, centre_y - h), egui::pos2(centre_x + w, centre_y - h), egui::pos2(centre_x, centre_y + h)]
            } else {
                vec![egui::pos2(centre_x - w, centre_y + h), egui::pos2(centre_x + w, centre_y + h), egui::pos2(centre_x, centre_y - h)]
            };
            ui.painter().add(egui::Shape::convex_polygon(points, color::ACCENT, egui::Stroke::NONE));
        }
        if response.clicked() {
            state.sort = match state.sort {
                Some((c, descending)) if c == column => Some((c, !descending)),
                _ => Some((column, column == SortColumn::Price)),
            };
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn rows() -> Vec<Row> {
        let names = ["None", "Master Ball", "Potion", "Antidote", "TM01"];
        names.iter().enumerate().map(|(n, name)| Row { name: name.to_string(), pocket: (n % 5) as u8, price: n as u32 * 100 }).collect()
    }

    /// The list opens, draws its header and rows, sorts, filters, and the keys change the item.
    #[test]
    fn the_list_opens_sorts_filters_and_follows_the_keys() {
        let ctx = egui::Context::default();
        let rows = rows();
        let mut value = 2usize;
        let frame = |value: &mut usize, input: egui::RawInput, open: bool| {
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if open {
                        let popup = ui.make_persistent_id("test_menu").with("popup");
                        ui.memory_mut(|m| m.open_popup(popup));
                    }
                    item_menu(ui, "test_menu", value, 240.0, &rows, &mut |_, _| None, true);
                });
            });
        };
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));
        for _ in 0..3 {
            frame(&mut value, egui::RawInput { screen_rect: screen, ..Default::default() }, true);
        }
        assert_eq!(value, 2, "drawing changes nothing");
        // A key moves the choice to the next item of the list.
        let down = egui::RawInput {
            screen_rect: screen,
            events: vec![egui::Event::Key { key: egui::Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            ..Default::default()
        };
        frame(&mut value, down, true);
        assert_eq!(value, 3);
    }
}
