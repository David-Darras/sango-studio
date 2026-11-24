//! The menus that every page uses to choose a Pokémon, an item or a move: the same rich lists everywhere.
//!
//! - A Pokémon: the list of the Species page, with the icon, the number, the name ("Rotom-Heat"), the types, the base stats and their
//!   total, a search box and a sort on each column. Each form is a Pokémon of its own.
//! - An item: the list of the items with their pictures, their pocket and their price.
//! - A move: the list of the moves with their type, their category, their power, their accuracy and their PP.
//!
//! The app gives the dump and the SD card with [`set_source`] at each frame; the lists are read when a menu opens.

use crate::learnset::Learnset as LearnsetTab;
use crate::{form_icon, list_header, paint_named_row, read_list_entries, sort_entries, ListEntry, SortColumn, LIST_ROW_HEIGHT, LIST_WIDTH};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The dump and the SD card that the menus read.
#[derive(Clone, Default)]
struct Source {
    dump: Option<Dump>,
    sd: Option<SdCard>,
}

/// What the menus keep between two frames.
#[derive(Default)]
struct Catalog {
    source: Source,
    /// The dump that the archives were opened for.
    opened: Option<std::path::PathBuf>,
    species_garc: Option<Garc>,
    icon_garc: Option<Garc>,
    /// The lines of the Pokémon menu: read again at each opening.
    entries: Vec<ListEntry>,
    icons: HashMap<(usize, usize), Option<egui::TextureHandle>>,
    search: String,
    sort: (SortColumn, bool),
    /// The menu that is open: its lines were read for it.
    open_menu: Option<egui::Id>,
    /// The moves, for the move menu.
    moves: LearnsetTab,
    items: Option<Arc<Mutex<sango_ui::item_menu::ItemList>>>,
}

fn catalog() -> &'static Mutex<Catalog> {
    static CATALOG: std::sync::OnceLock<Mutex<Catalog>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| Mutex::new(Catalog { sort: (SortColumn::Number, false), ..Catalog::default() }))
}

/// Gives the dump and the SD card to the menus. The app calls it at each frame.
pub fn set_source(session: &Session) {
    let Ok(mut catalog) = catalog().lock() else { return };
    catalog.source = Source { dump: session.dump.clone(), sd: session.sd.clone() };
    let root = session.dump.as_ref().map(|dump| dump.root().to_path_buf());
    if catalog.opened != root {
        catalog.opened = root;
        catalog.species_garc = session.dump.as_ref().and_then(|dump| Garc::open(&dump.archive(crate::ARCHIVE)).ok());
        catalog.icon_garc = session.dump.as_ref().and_then(|dump| Garc::open(&dump.archive(crate::icons::ICON_ARCHIVE)).ok());
        catalog.icons.clear();
        catalog.moves = LearnsetTab::default();
        if let Some(dump) = &session.dump {
            catalog.moves.open(dump);
            let mut items = sango_ui::item_menu::ItemList::default();
            items.open(dump);
            catalog.items = Some(Arc::new(Mutex::new(items)));
        } else {
            catalog.items = None;
        }
    }
}

/// A menu of the Pokémon: the button shows the icon and the name of `value` (a species and a form). `allowed` greys out the
/// lines that cannot be chosen; `none` adds the line "No Pokémon" (the species 0). Gives true when the value changed.
pub fn pokemon_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut (usize, usize), width: f32, allowed: Option<&dyn Fn(usize, usize) -> bool>, none: Option<&str>) -> bool {
    let Ok(mut catalog) = catalog().lock() else { return false };
    let catalog = &mut *catalog;
    let id = ui.make_persistent_id(("pokemon_menu", id_salt));
    let popup_id = id.with("popup");
    let before = *value;
    let height = 36.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let button = ui.interact(rect, id, egui::Sense::click());
    let open = ui.memory(|m| m.is_popup_open(popup_id));
    if ui.is_rect_visible(rect) {
        let visuals = if open { ui.visuals().widgets.open } else { *ui.style().interact(&button) };
        ui.painter().rect(rect.expand(visuals.expansion), visuals.rounding, visuals.weak_bg_fill, visuals.bg_stroke);
        let mut text_left = rect.left() + 8.0;
        if value.0 != 0 {
            let sd = catalog.source.sd.clone();
            if let Some(texture) = form_icon(&mut catalog.icons, catalog.icon_garc.as_ref(), sd.as_ref(), ui.ctx(), value.0, value.1) {
                let icon = egui::Rect::from_min_size(egui::pos2(rect.left() + 4.0, rect.center().y - 15.0), egui::vec2(40.0, 30.0));
                ui.painter().image(texture.id(), icon, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                text_left += 40.0;
            }
        }
        let text = if value.0 == 0 { none.unwrap_or("No Pokémon").to_string() } else { format!("#{:04} {}", value.0, crate::entry_name(value.0, value.1)) };
        let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), rect.right() - 24.0 - text_left, egui::TextStyle::Button);
        ui.painter().galley(egui::pos2(text_left, rect.center().y - galley.size().y / 2.0), galley, visuals.text_color());
        let arrow = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(8.0, 5.0));
        ui.painter().add(egui::Shape::convex_polygon(vec![arrow.left_top(), arrow.right_top(), arrow.center_bottom()], visuals.fg_stroke.color, egui::Stroke::NONE));
    }
    widget::register_focusable(ui, id, rect);
    if button.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        widget::set_active_id(ui, id);
    }
    if widget::is_active(ui, id) && widget::page_has_focus(ui) && !open {
        widget::glow(ui, rect, 6.0);
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            ui.memory_mut(|m| m.open_popup(popup_id));
        }
    }
    let now_open = ui.memory(|m| m.is_popup_open(popup_id));
    if now_open && catalog.open_menu != Some(popup_id) {
        // The list is read at each opening: a change on another page shows at once.
        let session = Session { dump: catalog.source.dump.clone(), sd: catalog.source.sd.clone(), ..Session::default() };
        catalog.entries = read_list_entries(catalog.species_garc.as_ref(), &session);
        catalog.search.clear();
        catalog.open_menu = Some(popup_id);
    }
    if !now_open && catalog.open_menu == Some(popup_id) {
        catalog.open_menu = None;
    }
    let mut chosen = None;
    egui::popup::popup_below_widget(ui, popup_id, &button, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(LIST_WIDTH);
        ui.horizontal(|ui| {
            ui.label(widget::hint("🔍"));
            let search = ui.add(egui::TextEdit::singleline(&mut catalog.search).hint_text("Search name or number...").desired_width(220.0));
            if !search.has_focus() && catalog.search.is_empty() {
                search.request_focus();
            }
        });
        ui.separator();
        let (mut column, mut descending) = catalog.sort;
        list_header(ui, &mut column, &mut descending);
        catalog.sort = (column, descending);
        ui.separator();
        let mut places = crate::match_entries(&catalog.entries, &catalog.search);
        sort_entries(&mut places, &catalog.entries, column, descending);
        if none.is_some() {
            let (row, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, 26.0), egui::Sense::click());
            if response.hovered() {
                ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::SURFACE_STRONG);
            }
            ui.painter().text(egui::pos2(row.left() + 46.0, row.center().y), egui::Align2::LEFT_CENTER, none.unwrap_or("No Pokémon"), egui::FontId::proportional(13.5), color::MUTED);
            if response.clicked() {
                chosen = Some((0, 0));
            }
        }
        let can = |entry: &ListEntry| allowed.is_none_or(|test| test(entry.species, entry.form));
        let current = places.iter().position(|&p| (catalog.entries[p].species, catalog.entries[p].form) == *value);
        let (up, down, enter) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
        let mut highlighted = current.unwrap_or(0);
        if down || up {
            // The next line that can be chosen.
            let mut place = highlighted as isize;
            loop {
                place += if down { 1 } else { -1 };
                if place < 0 || place as usize >= places.len() {
                    break;
                }
                if can(&catalog.entries[places[place as usize]]) {
                    highlighted = place as usize;
                    let entry = &catalog.entries[places[highlighted]];
                    *value = (entry.species, entry.form);
                    break;
                }
            }
        }
        if enter {
            if let Some(&place) = places.get(highlighted) {
                let entry = &catalog.entries[place];
                if can(entry) {
                    chosen = Some((entry.species, entry.form));
                }
            }
        }
        let mut area = egui::ScrollArea::vertical().id_salt(id.with("rows")).max_height(360.0);
        if up || down || (current.is_some() && catalog.search.is_empty() && !open) {
            area = area.vertical_scroll_offset(((highlighted as f32 + 0.5) * LIST_ROW_HEIGHT - 180.0).max(0.0));
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        let sd = catalog.source.sd.clone();
        area.show_rows(ui, LIST_ROW_HEIGHT, places.len(), |ui, range| {
            for row_place in range {
                let entry = &catalog.entries[places[row_place]];
                let enabled = can(entry);
                let (row, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, LIST_ROW_HEIGHT), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                if !ui.is_rect_visible(row) {
                    continue;
                }
                if row_place == highlighted && current.is_some() {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::ACCENT_SOFT);
                } else if response.hovered() && enabled {
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let icon = form_icon(&mut catalog.icons, catalog.icon_garc.as_ref(), sd.as_ref(), ui.ctx(), entry.species, entry.form);
                paint_named_row(ui, row, entry.species, &entry.name, Some(&entry.row), icon);
                if !enabled {
                    // A line that cannot be chosen is greyed out.
                    ui.painter().rect_filled(row, egui::Rounding::same(6.0), color::WHITE.gamma_multiply(0.6));
                }
                if response.clicked() && enabled {
                    chosen = Some((entry.species, entry.form));
                }
            }
        });
    });
    if let Some(picked) = chosen {
        *value = picked;
        ui.memory_mut(|m| m.close_popup());
    }
    *value != before
}

/// A menu of the items, with their pictures, their pocket and their price. `none` adds the line "None" (the item 0).
pub fn item_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut u16, width: f32, none: bool) -> bool {
    let list = catalog().lock().ok().and_then(|catalog| catalog.items.clone());
    let Some(list) = list else { return false };
    if let (Ok(mut items), Some(sd)) = (list.lock(), catalog().lock().ok().and_then(|c| c.source.sd.clone())) {
        items.refresh_custom(Some(&sd));
    }
    let mut item = *value as usize;
    let changed = sango_ui::item_menu::shared_menu(ui, id_salt, &mut item, width, &list, none);
    if changed {
        *value = item as u16;
    }
    changed
}

/// A menu of the moves, with their type, their category, their power, their accuracy and their PP.
pub fn move_menu(ui: &mut egui::Ui, id_salt: &str, value: &mut u16, width: f32) -> bool {
    let Ok(catalog) = catalog().lock() else { return false };
    let id = ui.make_persistent_id(("move_menu", id_salt));
    let name = if *value == 0 { "No move".to_string() } else { crate::move_names::MOVE.get(*value as usize - 1).map_or_else(|| format!("Move {value}"), |n| n.to_string()) };
    let button = ui.add_sized([width, 26.0], egui::Button::new(format!("{name}  ⏷")));
    widget::register_focusable(ui, id, button.rect);
    if button.clicked() {
        widget::set_active_id(ui, id);
    }
    match catalog.moves.choose_move(ui, id, &button, Some(*value)) {
        Some(picked) if picked != *value => {
            *value = picked;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The menu draws closed and open without the dump, and a click on the button opens its list.
    #[test]
    fn the_pokemon_menu_opens_and_keeps_its_value() {
        set_source(&Session::default());
        let ctx = egui::Context::default();
        let mut value = (25, 0);
        let frame = |events: Vec<egui::Event>, value: &mut (usize, usize)| {
            let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() };
            input.events = events;
            let mut changed = false;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| changed = pokemon_menu(ui, "test", value, 300.0, Some(&|_, form| form == 0), Some("No Pokémon")));
            });
            changed
        };
        assert!(!frame(Vec::new(), &mut value));
        let click = |pressed| egui::Event::PointerButton { pos: egui::pos2(40.0, 25.0), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        frame(vec![egui::Event::PointerMoved(egui::pos2(40.0, 25.0)), click(true), click(false)], &mut value);
        assert!(!frame(Vec::new(), &mut value), "opening the list does not change the value");
        assert_eq!(value, (25, 0));
    }
}
