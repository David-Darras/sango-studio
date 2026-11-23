//! The Mega tab: the forms of a species that a Pokémon changes into in a battle, like a Mega Evolution, and what the Pokémon needs for it.
//!
//! The condition is kept in the form (`mega` of the form in its pack, see `docs/species-packs.md`). It is one of four: no condition, a held
//! item, a known move or an ability. The tab shows the forms that the packs add to the species, with the condition of each one, and makes new
//! Mega forms. The base-game Mega Evolutions (archive 193 of the romfs, or the SD card override) can also be viewed and modified.

use crate::{extras, species_name, Species, SPECIES_COUNT};
use sango_core::mega_evolution::{MegaMethod, MegaEntry, MegaEvolutionTable};
use sango_pack::table::MegaCondition;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The most forms that a species changes into: the game keeps three places in the table of a species.
const MAX_ENTRIES: usize = sango_core::mega_evolution::MAX_ENTRIES;

/// The items that the first Mega forms get: the Life Orb, the Choice Band, the Choice Specs, the Choice Scarf.
const FIRST_ITEMS: [u16; 4] = sango_pack::table::LEGACY_MEGA_ITEMS;

/// The width of the table, in points.
const TABLE_WIDTH: f32 = 860.0;

/// The height of a row, in points.
const ROW_HEIGHT: f32 = 48.0;

/// The places of the columns of a row: x and width, in points.
/// Column 0: Icon + "Mega evolves into"
/// Column 1: "Method"
/// Column 2: "Needs"
/// Column 3: Actions ("Open" + delete/disable)
const COLUMNS: [(f32, f32); 4] = [(0.0, 250.0), (260.0, 190.0), (460.0, 250.0), (720.0, 100.0)];

/// The rectangle of a column inside a row.
fn column(row: egui::Rect, place: usize) -> egui::Rect {
    let (x, width) = COLUMNS[place];
    egui::Rect::from_min_size(egui::pos2(row.left() + x, row.top()), egui::vec2(width, row.height()))
}

/// The first item that no form of the species uses yet (the Life Orb when all are used).
pub(crate) fn free_item(used: &[u16]) -> u16 {
    FIRST_ITEMS.iter().copied().find(|item| !used.contains(item)).unwrap_or(FIRST_ITEMS[0])
}

/// The kinds of condition that the menu offers, with their names.
const KINDS: [(u8, &str); 5] = [(1, "No condition"), (2, "A held item"), (3, "A known move"), (4, "An ability"), (0, "Not a Mega Evolution")];

/// The kinds of condition that the game engine supports for base-game Mega Evolutions (archive 193).
const GAME_KINDS: [(u8, &str); 3] = [(2, "A held item"), (3, "A known move"), (0, "Not a Mega Evolution")];

/// The number of the kind of a condition, as `KINDS` has it.
fn kind_of(condition: MegaCondition) -> u8 {
    match condition {
        MegaCondition::Never => 0,
        MegaCondition::Always => 1,
        MegaCondition::Item(_) => 2,
        MegaCondition::Move(_) => 3,
        MegaCondition::Ability(_) => 4,
    }
}

/// A condition of a kind. The number of the old condition stays when it is the same kind of number, so a change of kind and back does
/// not lose it.
fn with_kind(kind: u8, old: MegaCondition) -> MegaCondition {
    let number = old.number();
    match kind {
        1 => MegaCondition::Always,
        2 => MegaCondition::Item(if number == 0 { FIRST_ITEMS[0] } else { number }),
        3 => MegaCondition::Move(number.max(1)),
        4 => MegaCondition::Ability(number.max(1)),
        _ => MegaCondition::Never,
    }
}

/// The sentence of a condition, for the hint under the list.
fn sentence(condition: MegaCondition) -> &'static str {
    match condition {
        MegaCondition::Never => "never changes",
        MegaCondition::Always => "changes without any condition",
        MegaCondition::Item(_) => "changes when the Pokémon holds the item",
        MegaCondition::Move(_) => "changes when the Pokémon knows the move",
        MegaCondition::Ability(_) => "changes when the Pokémon has the ability",
    }
}

/// The default item for a game mega evolution form when switching method to Held Item:
/// restores vanilla archive item if present, otherwise falls back to 659 (Venusaurite).
fn default_game_item(garc: Option<&sango_core::Garc>, species: usize, entry_index: usize) -> u16 {
    garc.and_then(|g| g.first_file(species))
        .and_then(|b| MegaEvolutionTable::from_bytes(b).ok())
        .and_then(|t| t.entries.get(entry_index).map(|e| e.argument))
        .filter(|&arg| arg != 0)
        .unwrap_or(659) // Venusaurite
}

/// A row in the Mega Evolution table.
enum MegaRow {
    Game {
        entry_index: usize,
        form: u16,
        name: String,
        entry: MegaEntry,
    },
    Pack {
        form: u16,
        condition: MegaCondition,
    },
}

/// Trash button for removing or disabling a mega condition.
fn trash_button(ui: &mut egui::Ui, cell: egui::Rect, hover: &str) -> bool {
    let rect = egui::Rect::from_center_size(cell.center(), egui::vec2(26.0, 26.0));
    let response = ui.interact(rect, ui.make_persistent_id(("mega_trash", rect.min.x as i32, rect.min.y as i32)), egui::Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(rect, egui::Rounding::same(7.0), color::ROSE.linear_multiply(0.15));
    }
    let ink = if response.hovered() { color::ROSE } else { color::MUTED };
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let c = rect.center();
    let painter = ui.painter();
    painter.line_segment([egui::pos2(c.x - 6.0, c.y - 4.5), egui::pos2(c.x + 6.0, c.y - 4.5)], stroke);
    painter.line_segment([egui::pos2(c.x - 2.0, c.y - 6.5), egui::pos2(c.x + 2.0, c.y - 6.5)], stroke);
    painter.rect_stroke(egui::Rect::from_min_max(egui::pos2(c.x - 4.5, c.y - 4.5), egui::pos2(c.x + 4.5, c.y + 6.5)), egui::Rounding::same(1.5), stroke);
    painter.line_segment([egui::pos2(c.x - 1.5, c.y - 1.5), egui::pos2(c.x - 1.5, c.y + 3.5)], stroke);
    painter.line_segment([egui::pos2(c.x + 1.5, c.y - 1.5), egui::pos2(c.x + 1.5, c.y + 3.5)], stroke);
    response.on_hover_text(hover).clicked()
}

impl Species {
    /// The forms of the open species that a pack adds, with their conditions: the form number and the condition.
    fn pack_mega_forms(&self) -> Vec<(u16, MegaCondition)> {
        extras::pack_forms(self.species).iter().map(|record| (record.form, record.mega)).collect()
    }

    /// The reference of the open species in a pack.
    pub(crate) fn pack_ref(&self) -> sango_pack::Ref {
        if self.species <= SPECIES_COUNT {
            sango_pack::Ref::Game(self.species as u16)
        } else {
            sango_pack::Ref::key(&species_name(self.species))
        }
    }

    /// Changes the condition of a pack form, and writes the table on the SD card.
    fn set_mega(&mut self, session: &sango_core::Session, form: u16, condition: MegaCondition) {
        let Some(sd) = session.sd.as_ref() else { return };
        let species = self.pack_ref();
        let result = sango_pack::Library::open(sd).and_then(|mut library| {
            library.set_mega(sd, &species, form, condition)?;
            library.save(sd)
        });
        self.status = Some(match result {
            Ok(()) => {
                extras::invalidate();
                extras::refresh(Some(sd));
                (true, format!("The form {form} of {} {}. Restart the game to see it.", species_name(self.species), sentence(condition)))
            }
            Err(message) => (false, message),
        });
    }

    /// The tab.
    pub(crate) fn mega_tab(&mut self, ui: &mut egui::Ui, session: &sango_core::Session) {
        if self.mega_table.is_none() {
            self.load_mega_table(session);
        }
        let pack_forms = self.pack_mega_forms();
        let mut change_pack: Option<(u16, MegaCondition)> = None;
        let mut delete_pack: Option<u16> = None;
        let mut change_game: Option<(usize, MegaEntry)> = None;
        let mut open_form: Option<usize> = None;
        let mut add = false;

        let game_active = self.mega_table.as_ref().map_or(0, |t| t.entry_count());
        let pack_active = pack_forms.iter().filter(|(_, condition)| condition.is_mega()).count();
        let total_active = game_active + pack_active;

        egui::ScrollArea::vertical().id_salt("species_mega_tab").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(TABLE_WIDTH);
                ui.horizontal(|ui| {
                    let add_btn = ui.add_enabled(
                        session.sd.is_some() && self.data.is_some() && total_active < MAX_ENTRIES,
                        egui::Button::new(egui::RichText::new("+ Add a Mega evolution").color(color::WHITE)).fill(color::ACCENT),
                    );
                    if add_btn.clicked() {
                        add = true;
                    }
                    add_btn.on_hover_text("Makes a new form that starts with the data that are open now, and gives it a held item. You can change the condition after.");
                    ui.label(widget::hint(&format!("{total_active} of {MAX_ENTRIES} Mega forms with a condition. Restart the game after a change.")));
                });
                ui.add_space(8.0);

                // Table header
                let (header_rect, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, 24.0), egui::Sense::hover());
                for (place, name) in ["Mega evolves into", "Method", "Needs"].iter().enumerate() {
                    let cell = column(header_rect, place);
                    ui.painter().text(egui::pos2(cell.left() + 4.0, cell.center().y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.0), color::MUTED);
                }
                ui.separator();

                // Build rows: game megas first (from SD card or ROMFS archive 193), then pack forms
                let mut rows: Vec<MegaRow> = Vec::new();
                if let Some(table) = &self.mega_table {
                    for (idx, entry) in table.entries.iter().enumerate() {
                        if entry.evolved_form != 0 {
                            let name = crate::form_names::form_label(self.species, entry.evolved_form as usize, true);
                            rows.push(MegaRow::Game {
                                entry_index: idx,
                                form: entry.evolved_form,
                                name,
                                entry: *entry,
                            });
                        }
                    }
                }
                for &(form, condition) in &pack_forms {
                    rows.push(MegaRow::Pack { form, condition });
                }

                if rows.is_empty() {
                    ui.add_space(6.0);
                    ui.label(widget::hint("This Pokémon has no Mega Evolution."));
                }

                for row_item in rows {
                    let (form, form_label) = match &row_item {
                        MegaRow::Game { form, name, .. } => (*form, name.clone()),
                        MegaRow::Pack { form, condition } => (*form, crate::form_names::form_label(self.species, *form as usize, condition.is_mega())),
                    };

                    let (row, _) = ui.allocate_exact_size(egui::vec2(TABLE_WIDTH - 36.0, ROW_HEIGHT), egui::Sense::hover());
                    if ui.rect_contains_pointer(row) {
                        ui.painter().rect_filled(row, egui::Rounding::same(8.0), color::SURFACE_STRONG.linear_multiply(0.6));
                    }

                    // Column 0: Icon + Name
                    let cell0 = column(row, 0);
                    let icon_rect = egui::Rect::from_min_size(egui::pos2(cell0.left() + 4.0, cell0.center().y - 15.0), egui::vec2(40.0, 30.0));
                    if let Some(texture) = self.form_icon_texture(ui.ctx(), session, self.species, form as usize) {
                        let full = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter().image(texture.id(), icon_rect, full, egui::Color32::WHITE);
                    }
                    let full_name = format!("{} ({form_label})", species_name(self.species));
                    ui.painter().text(
                        egui::pos2(cell0.left() + 52.0, cell0.center().y),
                        egui::Align2::LEFT_CENTER,
                        &full_name,
                        egui::FontId::proportional(14.0),
                        color::TEXT,
                    );

                    let cell1 = column(row, 1);
                    let method_rect = egui::Rect::from_center_size(cell1.center(), egui::vec2(180.0, 28.0));
                    let cell2 = column(row, 2);
                    let need_rect = egui::Rect::from_center_size(cell2.center(), egui::vec2(240.0, 28.0));
                    let cell3 = column(row, 3);
                    let actions_rect = egui::Rect::from_center_size(cell3.center(), egui::vec2(90.0, 28.0));

                    match row_item {
                        MegaRow::Game { entry_index, entry, .. } => {
                            let mut game_entry = entry;
                            let current_kind = match entry.method {
                                MegaMethod::Item => 2u8,
                                MegaMethod::Move => 3u8,
                                MegaMethod::None => 0u8,
                            };
                            let shown = GAME_KINDS.iter().find(|(k, _)| *k == current_kind).map_or("?", |(_, name)| *name);

                            // Column 1: Method
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(method_rect), |ui| {
                                let mut kind = current_kind;
                                egui::ComboBox::from_id_salt(format!("game_mega_kind_{form}_{entry_index}"))
                                    .selected_text(shown)
                                    .width(170.0)
                                    .show_ui(ui, |ui| {
                                        for (value, name) in GAME_KINDS {
                                            ui.selectable_value(&mut kind, value, name);
                                        }
                                    });
                                if kind != current_kind {
                                    match kind {
                                        2 => {
                                            game_entry.method = MegaMethod::Item;
                                            if game_entry.argument == 0 {
                                                game_entry.argument = default_game_item(self.mega_garc.as_ref(), self.species, entry_index);
                                            }
                                        }
                                        3 => {
                                            game_entry.method = MegaMethod::Move;
                                            if game_entry.argument == 0 {
                                                game_entry.argument = 620; // Dragon Ascent
                                            }
                                        }
                                        _ => {
                                            game_entry.method = MegaMethod::None;
                                            game_entry.argument = 0;
                                        }
                                    }
                                }
                            });

                            // Column 2: Needs
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(need_rect), |ui| {
                                let salt = format!("game_mega_need_{form}_{entry_index}");
                                match game_entry.method {
                                    MegaMethod::Item => {
                                        let mut item = game_entry.argument;
                                        crate::item_picker(ui, &salt, &mut item);
                                        game_entry.argument = item;
                                    }
                                    MegaMethod::Move => {
                                        let mut mv = if game_entry.argument == 0 { 620 } else { game_entry.argument };
                                        crate::searchable_picker(ui, &salt, &mut mv, &crate::move_names::MOVE, 200.0);
                                        game_entry.argument = mv;
                                    }
                                    MegaMethod::None => {
                                        ui.label(egui::RichText::new("—").color(color::MUTED));
                                    }
                                }
                            });

                            if game_entry != entry {
                                change_game = Some((entry_index, game_entry));
                            }

                            // Column 3: Actions
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    if ui.button("Open").on_hover_text("Open this form to change its data, its model and its texts.").clicked() {
                                        open_form = Some(form as usize);
                                    }
                                    if entry.method != MegaMethod::None {
                                        ui.add_space(4.0);
                                        let (trash_rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
                                        if trash_button(ui, trash_rect, "Disable this Mega Evolution condition") {
                                            let mut disabled = entry;
                                            disabled.method = MegaMethod::None;
                                            disabled.argument = 0;
                                            change_game = Some((entry_index, disabled));
                                        }
                                    }
                                });
                            });
                        }
                        MegaRow::Pack { condition, .. } => {
                            let mut new_condition = condition;

                            // Column 1: Method
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(method_rect), |ui| {
                                let mut kind = kind_of(condition);
                                let shown = KINDS.iter().find(|(k, _)| *k == kind).map_or("?", |(_, name)| *name);
                                egui::ComboBox::from_id_salt(format!("pack_mega_kind_{form}"))
                                    .selected_text(shown)
                                    .width(170.0)
                                    .show_ui(ui, |ui| {
                                        for (value, name) in KINDS {
                                            ui.selectable_value(&mut kind, value, name);
                                        }
                                    });
                                if kind != kind_of(condition) {
                                    new_condition = with_kind(kind, condition);
                                }
                            });

                            // Column 2: Needs
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(need_rect), |ui| {
                                let salt = format!("pack_mega_number_{form}");
                                match &mut new_condition {
                                    MegaCondition::Item(number) => crate::item_picker(ui, &salt, number),
                                    MegaCondition::Move(number) => crate::searchable_picker(ui, &salt, number, &crate::move_names::MOVE, 200.0),
                                    MegaCondition::Ability(number) => {
                                        let mut ability = (*number).min(255) as u8;
                                        crate::ability_picker(ui, &salt, &mut ability);
                                        *number = u16::from(ability);
                                    }
                                    MegaCondition::Always | MegaCondition::Never => {
                                        ui.label(egui::RichText::new("—").color(color::MUTED));
                                    }
                                }
                            });

                            if new_condition != condition {
                                change_pack = Some((form, new_condition));
                            }

                            // Column 3: Actions
                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
                                ui.horizontal_centered(|ui| {
                                    if ui.button("Open").on_hover_text("Open this form to change its data, its model and its texts.").clicked() {
                                        open_form = Some(form as usize);
                                    }
                                    ui.add_space(4.0);
                                    let (trash_rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
                                    if trash_button(ui, trash_rect, "Delete this form") {
                                        delete_pack = Some(form);
                                    }
                                });
                            });
                        }
                    }
                }
            });
        });

        if let Some((entry_index, entry)) = change_game {
            self.set_game_mega(session, entry_index, entry);
        }
        if let Some((form, condition)) = change_pack {
            self.set_mega(session, form, condition);
        }
        if let Some(form) = delete_pack {
            self.delete_form(session, form);
        }
        if let Some(form) = open_form {
            self.form = form;
        }
        if add {
            let used: Vec<u16> = pack_forms.iter().filter_map(|(_, condition)| if let MegaCondition::Item(item) = condition { Some(*item) } else { None }).collect();
            self.new_form(session, MegaCondition::Item(free_item(&used)));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_new_mega_form_gets_the_first_item_that_is_free() {
        assert_eq!(free_item(&[]), 0x010E);
        assert_eq!(free_item(&[0x010E]), 0x00DC);
        assert_eq!(free_item(&[0x010E, 0x00DC, 0x0129, 0x011F]), 0x010E, "all used: the first one again");
    }

    #[test]
    fn a_change_of_kind_gives_a_condition_that_makes_sense() {
        assert_eq!(with_kind(1, MegaCondition::Item(5)), MegaCondition::Always);
        assert_eq!(with_kind(2, MegaCondition::Never), MegaCondition::Item(0x010E));
        assert_eq!(with_kind(3, MegaCondition::Item(5)), MegaCondition::Move(5), "the number stays");
        assert_eq!(with_kind(4, MegaCondition::Always), MegaCondition::Ability(1));
        assert_eq!(with_kind(0, MegaCondition::Move(5)), MegaCondition::Never);
        for (kind, _) in KINDS {
            assert_eq!(kind_of(with_kind(kind, MegaCondition::Never)), kind);
        }
    }

    #[test]
    fn game_mega_entries_can_be_built_and_edited() {
        let mut table = MegaEvolutionTable::empty();
        table.entries[0] = MegaEntry {
            evolved_form: 1,
            method: MegaMethod::Item,
            argument: 659,
            argument2: 0,
        };
        table.entries[1] = MegaEntry {
            evolved_form: 2,
            method: MegaMethod::Item,
            argument: 678,
            argument2: 0,
        };
        assert_eq!(table.entry_count(), 2);
        assert_eq!(table.entries[0].evolved_form, 1);
        assert_eq!(table.entries[0].method, MegaMethod::Item);
        assert_eq!(table.entries[0].argument, 659);
        assert_eq!(table.entries[1].evolved_form, 2);
        assert_eq!(table.entries[1].method, MegaMethod::Item);
        assert_eq!(table.entries[1].argument, 678);

        // Disabling an entry
        table.entries[0].method = MegaMethod::None;
        table.entries[0].argument = 0;
        assert_eq!(table.entry_count(), 1);
        assert_eq!(table.entries[0].method, MegaMethod::None);
    }
}
