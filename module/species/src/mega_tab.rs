//! The Mega tab: the forms of a species that a Pokémon changes into in a battle when it holds an item, like a Mega Evolution.
//!
//! The item is kept in the form (`mega_item` of the form in its pack, see `docs/species-packs.md`). The tab shows the forms that the packs
//! add to the species, with the item of each one, and makes new Mega forms. The Mega Evolutions of the game are in the data of the game and
//! stay as they are. The game changes the Pokémon of a species into the form of the item that it holds: the form that it has before is not
//! looked at.

use crate::{extras, species_name, Species, SPECIES_COUNT};
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

impl Species {
    /// The forms of the open species that a pack adds, with their items: the form number and the item (0: no change).
    fn mega_routes(&self) -> Vec<(u16, u16)> {
        extras::pack_forms(self.species).iter().map(|record| (record.form, record.mega_item)).collect()
    }

    /// The reference of the open species in a pack.
    pub(crate) fn pack_ref(&self) -> sango_pack::Ref {
        if self.species <= SPECIES_COUNT {
            sango_pack::Ref::Game(self.species as u16)
        } else {
            sango_pack::Ref::key(&species_name(self.species))
        }
    }

    /// Changes the item of a form, and writes the table on the SD card.
    fn set_mega_item(&mut self, session: &sango_core::Session, form: u16, item: u16) {
        let Some(sd) = session.sd.as_ref() else { return };
        let species = self.pack_ref();
        let result = sango_pack::Library::open(sd).and_then(|mut library| {
            library.set_mega_item(sd, &species, form, item)?;
            library.save(sd)
        });
        self.status = Some(match result {
            Ok(()) => {
                extras::invalidate();
                extras::refresh(Some(sd));
                (true, format!("The form {form} of {} changes with the item {item}. Restart the game to see it.", species_name(self.species)))
            }
            Err(message) => (false, message),
        });
    }

    /// The tab.
    pub(crate) fn mega_tab(&mut self, ui: &mut egui::Ui, session: &sango_core::Session) {
        let routes = self.mega_routes();
        let mut change: Option<(u16, u16)> = None;
        let mut add = false;
        egui::ScrollArea::vertical().id_salt("species_mega_tab").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            widget::card(ui, |ui| {
                ui.set_width(680.0_f32.min(ui.available_width()));
                ui.label(widget::title("Mega Evolution"));
                ui.label(widget::hint("A Pokémon of this species changes into a form in a battle, like a Mega Evolution, when it holds the item of that form. The Mega Evolutions of the game stay as they are. The game looks at the item only: the form that the Pokémon has before is not looked at."));
                ui.add_space(8.0);
                if routes.is_empty() {
                    ui.label(widget::hint("No form of a pack changes this species yet. Make a new Mega form, or a new form in the Species menu."));
                }
                for &(form, item) in &routes {
                    ui.horizontal(|ui| {
                        ui.label(crate::form_names::form_label(self.species, form as usize, true));
                        ui.label(widget::hint("changes when the Pokémon holds"));
                        let mut value = item;
                        crate::item_picker(ui, &format!("mega_item_{form}"), &mut value);
                        if value != item {
                            change = Some((form, value));
                        }
                        if ui.small_button("Open").on_hover_text("Open this form to change its data, its model and its texts.").clicked() {
                            self.form = form as usize;
                        }
                    });
                }
                ui.add_space(8.0);
                let used = routes.iter().filter(|(_, item)| *item != 0).count();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(session.sd.is_some() && self.data.is_some() && used < MAX_ROUTES, |ui| {
                        add = widget::button_styled(ui, egui::RichText::new("+ New Mega form").color(color::WHITE), Some(color::ACCENT));
                    })
                    .response
                    .on_hover_text("Makes a new form that starts with the data that are open now, and gives it an item.");
                    ui.label(widget::hint(&format!("{used} of {MAX_ROUTES} forms with an item. Restart the game after a change.")));
                });
            });
        });
        if let Some((form, item)) = change {
            self.set_mega_item(session, form, item);
        }
        if add {
            let used: Vec<u16> = routes.iter().map(|(_, item)| *item).collect();
            self.new_form(session, free_item(&used));
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
}
