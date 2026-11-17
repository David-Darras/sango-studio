//! The Mega tab: the forms of a species that a Pokémon changes into in a battle, like a Mega Evolution, and what the Pokémon needs for it.
//!
//! The condition is kept in the form (`mega` of the form in its pack, see `docs/species-packs.md`). It is one of four: no condition, a held
//! item, a known move or an ability. The tab shows the forms that the packs add to the species, with the condition of each one, and makes new
//! Mega forms. The Mega Evolutions of the game are in the data of the game and stay as they are. The game does not look at the form that the
//! Pokémon has before.

use crate::{extras, species_name, Species, SPECIES_COUNT};
use sango_pack::table::MegaCondition;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The most forms that a species changes into: the game keeps three places in the table of a species.
const MAX_ROUTES: usize = 3;

/// The items that the first Mega forms get: the Life Orb, the Choice Band, the Choice Specs, the Choice Scarf.
const FIRST_ITEMS: [u16; 4] = sango_pack::table::LEGACY_MEGA_ITEMS;

/// The first item that no form of the species uses yet (the Life Orb when all are used).
pub(crate) fn free_item(used: &[u16]) -> u16 {
    FIRST_ITEMS.iter().copied().find(|item| !used.contains(item)).unwrap_or(FIRST_ITEMS[0])
}

/// The kinds of condition that the menu offers, with their names.
const KINDS: [(u8, &str); 5] = [(1, "No condition"), (2, "A held item"), (3, "A known move"), (4, "An ability"), (0, "Not a Mega Evolution")];

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

impl Species {
    /// The forms of the open species that a pack adds, with their conditions: the form number and the condition.
    fn mega_routes(&self) -> Vec<(u16, MegaCondition)> {
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

    /// Changes the condition of a form, and writes the table on the SD card.
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
        let routes = self.mega_routes();
        let mut change: Option<(u16, MegaCondition)> = None;
        let mut add = false;
        egui::ScrollArea::vertical().id_salt("species_mega_tab").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(780.0_f32.min(ui.available_width()));
                ui.label(widget::title("Mega Evolution"));
                ui.label(widget::hint("A Pokémon of this species changes into a form in a battle, like a Mega Evolution, when it meets the condition of that form: no condition, a held item, a known move or an ability. The Mega Evolutions of the game stay as they are. The game does not look at the form that the Pokémon has before."));
                ui.add_space(8.0);
                if routes.is_empty() {
                    ui.label(widget::hint("No form of a pack changes this species yet. Make a new Mega form, or a new form in the Species menu."));
                }
                for &(form, condition) in &routes {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(crate::form_names::form_label(self.species, form as usize, condition.is_mega()));
                        let mut kind = kind_of(condition);
                        let shown = KINDS.iter().find(|(k, _)| *k == kind).map_or("?", |(_, name)| *name);
                        egui::ComboBox::from_id_salt(format!("mega_kind_{form}")).selected_text(shown).width(190.0).show_ui(ui, |ui| {
                            for (value, name) in KINDS {
                                ui.selectable_value(&mut kind, value, name);
                            }
                        });
                        let mut new = if kind != kind_of(condition) { with_kind(kind, condition) } else { condition };
                        let salt = format!("mega_number_{form}");
                        match &mut new {
                            MegaCondition::Item(number) => crate::item_picker(ui, &salt, number),
                            MegaCondition::Move(number) => crate::searchable_picker(ui, &salt, number, &crate::move_names::MOVE, 200.0),
                            MegaCondition::Ability(number) => {
                                let mut ability = (*number).min(255) as u8;
                                crate::ability_picker(ui, &salt, &mut ability);
                                *number = u16::from(ability);
                            }
                            MegaCondition::Always | MegaCondition::Never => {}
                        }
                        if new != condition {
                            change = Some((form, new));
                        }
                        if ui.small_button("Open").on_hover_text("Open this form to change its data, its model and its texts.").clicked() {
                            self.form = form as usize;
                        }
                    });
                }
                ui.add_space(8.0);
                let used = routes.iter().filter(|(_, condition)| condition.is_mega()).count();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(session.sd.is_some() && self.data.is_some() && used < MAX_ROUTES, |ui| {
                        add = widget::button_styled(ui, egui::RichText::new("+ New Mega form").color(color::WHITE), Some(color::ACCENT));
                    })
                    .response
                    .on_hover_text("Makes a new form that starts with the data that are open now, and gives it a held item. You can change the condition after.");
                    ui.label(widget::hint(&format!("{used} of {MAX_ROUTES} forms with a condition. Restart the game after a change.")));
                });
            });
        });
        if let Some((form, condition)) = change {
            self.set_mega(session, form, condition);
        }
        if add {
            let used: Vec<u16> = routes.iter().filter_map(|(_, condition)| if let MegaCondition::Item(item) = condition { Some(*item) } else { None }).collect();
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
}
