//! The list of Pokémon that other pages show: the same rows as the species menu of the Species page.
//!
//! A row is a species or one of its forms ("Rotom-Heat"): the icon, the number, the name, the two types, the six base stats and their
//! total. A click on a column name sorts the list.

use crate::{form_icon, list_header, paint_named_row, read_list_entries, sort_entries, ListEntry, SortColumn, ARCHIVE, LIST_ROW_HEIGHT, LIST_WIDTH};
use sango_core::{Garc, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, Session};
use std::collections::HashMap;
use std::path::PathBuf;

/// A sortable list of species and forms with icons, types and base stats.
pub struct SpeciesTable {
    /// The dump that the data comes from.
    root: Option<PathBuf>,
    icon_garc: Option<Garc>,
    /// The SD card: the icons that the user changed.
    sd: Option<SdCard>,
    /// The icon textures, one per species and form: decoded at the first display, then kept.
    icon_textures: HashMap<(usize, usize), Option<egui::TextureHandle>>,
    /// The types and the base stats of all the species and forms.
    entries: Vec<ListEntry>,
    /// The place in `entries` of each species and form.
    places: HashMap<(usize, usize), usize>,
    sort_column: SortColumn,
    sort_descending: bool,
}

impl Default for SpeciesTable {
    fn default() -> Self {
        SpeciesTable {
            root: None,
            icon_garc: None,
            sd: None,
            icon_textures: HashMap::new(),
            entries: Vec::new(),
            places: HashMap::new(),
            sort_column: SortColumn::Number,
            sort_descending: false,
        }
    }
}

impl SpeciesTable {
    /// The width of the list, in points.
    pub const WIDTH: f32 = LIST_WIDTH;

    /// Reads the types and the base stats again (the SD card first, then the romfs). The icons are opened once for each dump.
    pub fn load(&mut self, session: &Session) {
        let Some(dump) = &session.dump else { return };
        let garc = Garc::open(&dump.archive(ARCHIVE)).ok();
        self.entries = read_list_entries(garc.as_ref(), session);
        self.places = self.entries.iter().enumerate().map(|(place, e)| ((e.species, e.form), place)).collect();
        if self.sd.as_ref().map(|sd| sd.local().to_path_buf()) != session.sd.as_ref().map(|sd| sd.local().to_path_buf()) {
            self.sd = session.sd.clone();
            self.icon_textures.clear();
        }
        if self.root.as_deref() != Some(dump.root()) {
            self.root = Some(dump.root().to_path_buf());
            self.icon_garc = Garc::open(&dump.archive(crate::icons::ICON_ARCHIVE)).ok();
            self.icon_textures.clear();
        }
    }

    /// Draws the list of these species and forms, sorted. Returns the species and the form that the user clicked, if any.
    ///
    /// `id_salt` must differ for two lists in the same parent. `height` is the most that the rows take.
    pub fn show(&mut self, ui: &mut egui::Ui, id_salt: &str, pokemon: &[(usize, usize)], height: f32) -> Option<(usize, usize)> {
        list_header(ui, &mut self.sort_column, &mut self.sort_descending);
        ui.separator();
        // A species and a form without data in the list (the list is not read yet) still gets a line, with its name only.
        let mut extra: Vec<ListEntry> = Vec::new();
        let mut sorted: Vec<usize> = Vec::new();
        for &(species, form) in pokemon {
            match self.places.get(&(species, form)) {
                Some(&place) => sorted.push(place),
                None => {
                    sorted.push(self.entries.len() + extra.len());
                    extra.push(ListEntry {
                        species,
                        form,
                        name: crate::entry_name(species, form),
                        row: crate::ListRow { types: [0, 0], stats: [0; 6], total: 0, modified: false },
                    });
                }
            }
        }
        let all: Vec<ListEntry> = self.entries.iter().cloned().chain(extra).collect();
        sort_entries(&mut sorted, &all, self.sort_column, self.sort_descending);
        let mut clicked = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        // The list gets its height from the number of rows, not from the room that the parent has left:
        // in a row of a layout the room can be very small, and the list would show about two rows.
        let wanted = (sorted.len() as f32 * LIST_ROW_HEIGHT).min(height);
        ui.allocate_ui(egui::vec2(LIST_WIDTH, wanted), |ui| {
            egui::ScrollArea::vertical().id_salt(id_salt).auto_shrink([false, false]).show_rows(ui, LIST_ROW_HEIGHT, sorted.len(), |ui, range| {
                for &place in &sorted[range] {
                    let entry = &all[place];
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, LIST_ROW_HEIGHT), egui::Sense::click());
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    if response.hovered() {
                        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let icon = form_icon(&mut self.icon_textures, self.icon_garc.as_ref(), self.sd.as_ref(), ui.ctx(), entry.species, entry.form);
                    paint_named_row(ui, rect, entry.species, &entry.name, Some(&entry.row), icon);
                    if response.clicked() {
                        clicked = Some((entry.species, entry.form));
                    }
                }
            });
        });
        clicked
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The height that the list takes in its first frame, for a list of `count` species.
    fn first_frame_height(count: usize, max: f32) -> f32 {
        let mut table = SpeciesTable::default();
        let list: Vec<(usize, usize)> = (1..=count).map(|n| (n, 0)).collect();
        let ctx = egui::Context::default();
        let mut height = 0.0;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1900.0, 1000.0)));
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let before = ui.cursor().top();
                table.show(ui, "test_list", &list, max);
                height = ui.cursor().top() - before;
            });
        });
        height
    }

    #[test]
    fn a_long_list_shows_many_rows_at_once() {
        // Even when the parent has almost no room left, the list keeps its height.
        let mut table = SpeciesTable::default();
        let list: Vec<(usize, usize)> = (1..=300).map(|n| (n, 0)).collect();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1900.0, 1000.0)));
        let mut height = 0.0;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_ui(egui::vec2(700.0, 20.0), |ui| {
                    let before = ui.cursor().top();
                    table.show(ui, "narrow_parent", &list, 560.0);
                    height = ui.cursor().top() - before;
                });
            });
        });
        assert!(height >= 10.0 * LIST_ROW_HEIGHT, "the list is only {height} points high in a small parent");
        // The rows alone are more than 10 times the height of a row.
        assert!(first_frame_height(300, 560.0) >= 10.0 * LIST_ROW_HEIGHT + 20.0, "{}", first_frame_height(300, 560.0));
    }

    #[test]
    fn a_short_list_takes_only_its_rows() {
        // A scroll area is never lower than 64 points, so the test compares lists of 3 and 5 rows.
        let three = first_frame_height(3, 560.0);
        let five = first_frame_height(5, 560.0);
        assert!((five - three - 2.0 * LIST_ROW_HEIGHT).abs() < 1.0, "{three} {five}");
    }
}
