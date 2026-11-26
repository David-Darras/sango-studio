//! The five tabs of the page: the trainer, the party, the boxes, the Bag and the map.

use crate::controls::{self, menu, text_field, Cell};
use crate::data::{self, Place, Trainer, BOX_COUNT, BOX_SLOTS, PARTY_SLOTS, POCKETS, TILE};
use crate::editor::{self, Origin};
use crate::format::{self, Save};
use crate::page::SavePage;
use crate::pokemon::Pokemon;
use sango_species::menus;
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, widget};

const BADGES: [&str; 8] = ["Stone", "Knuckle", "Dynamo", "Heat", "Balance", "Feather", "Mind", "Rain"];
const GENDERS: [&str; 2] = ["Boy", "Girl"];
const FACINGS: [&str; 4] = ["Up", "Down", "Left", "Right"];
/// The species that the button "+ New Pokémon" gives: the user then chooses another one.
const FIRST_SPECIES: u16 = 25;
/// The tabs of the party and of the boxes.
const PARTY_TAB: usize = 1;
const BOX_TAB: usize = 2;

/// The name of a line, blue with ↺ when the value is not the one of the file. Gives true at the click on ↺.
fn name(ui: &mut egui::Ui, text: &str, changed: bool) -> bool {
    ui.horizontal(|ui| widget::field_name(ui, text, changed).1).inner
}

/// A line of the trainer: the name, then the control, for one value.
macro_rules! value {
    ($ui:expr, $label:expr, $now:ident, $file:ident, $field:ident, $control:expr) => {{
        if name($ui, $label, $now.$field != $file.$field) {
            $now.$field = $file.$field.clone();
        }
        $control;
        $ui.end_row();
    }};
}

fn first_free_box_place(save: &Save, from: usize) -> Option<(usize, usize)> {
    (0..BOX_COUNT).map(|n| (from + n) % BOX_COUNT).flat_map(|number| (0..BOX_SLOTS).map(move |slot| (number, slot))).find(|&(number, slot)| data::boxed(save, number, slot).is_empty())
}

impl SavePage {
    /// The name of a Pokémon in a place of a grid.
    fn cell(&mut self, ctx: &egui::Context, pokemon: &Pokemon, level: bool, changed: bool) -> Cell {
        if pokemon.is_empty() {
            return Cell { icon: None, text: String::new(), changed };
        }
        let icon = self.icons.get(ctx, usize::from(pokemon.species())).cloned();
        let mut text = if pokemon.is_egg() { "Egg".to_string() } else { pokemon.nickname() };
        // A name can be made of spaces only: the grid then shows the species.
        if text.chars().all(|c| c.is_whitespace() || c.is_control()) {
            let species = pokemon.species();
            text = self.data.as_mut().map_or_else(String::new, |data| data.species_name(species, pokemon.language()));
        }
        if level && !pokemon.is_egg() {
            text += &format!("  Lv.{}", pokemon.level());
        }
        Cell { icon, text, changed }
    }

    /// What a new Pokémon says about the game of the player.
    fn origin_of(&self, save: &Save) -> Origin {
        let who = save.block(format::TRAINER);
        let trainer = Trainer::read(save);
        // The country and the region of the console are in the Pokémon that the player caught.
        let mine = data::party(save).into_iter().find(|p| p.trainer_id() == trainer.trainer_id && p.secret_id() == trainer.secret_id);
        let (country, console) = mine.map_or((0, who[0x2C]), |p| (p.data[0xE0], p.data[0xE2]));
        let map = data::place(save).map;
        let place = self.data.as_ref().and_then(|d| d.map(map)).map_or(0, |m| m.place);
        Origin { version: who[4], country, region: who[0x2C], console, language: Trainer::language(save), place }
    }

    /// True when the button "+ New Pokémon" has a free place.
    pub(crate) fn can_add(&self) -> bool {
        let Some(save) = &self.edited else { return false };
        match self.tab {
            PARTY_TAB => data::party_count(save) < PARTY_SLOTS,
            BOX_TAB => data::boxed(save, self.box_number, self.box_slot).is_empty(),
            _ => false,
        }
    }

    pub(crate) fn can_delete(&self) -> bool {
        let Some(save) = &self.edited else { return false };
        match self.tab {
            // The game needs one Pokémon in the party.
            PARTY_TAB => self.party_slot < data::party_count(save) && data::party_count(save) > 1,
            BOX_TAB => !data::boxed(save, self.box_number, self.box_slot).is_empty(),
            _ => false,
        }
    }

    pub(crate) fn add(&mut self) {
        let Some(save) = self.edited.clone() else { return };
        if !self.can_add() {
            return;
        }
        let origin = self.origin_of(&save);
        let trainer = Trainer::read(&save);
        let Some(game) = self.data.as_mut() else { return };
        let new = editor::new_pokemon(game, &trainer, origin, FIRST_SPECIES);
        self.put(new);
    }

    /// Puts a Pokémon in the chosen place of the open tab: at the end of the party, or in the place of the box.
    fn put(&mut self, mut pokemon: Pokemon) {
        let Some(save) = self.edited.as_mut() else { return };
        if self.tab == PARTY_TAB {
            let mut party = data::party(save);
            if let Some(game) = &self.data {
                game.calculate(&mut pokemon);
            }
            if self.party_slot < party.len() {
                party[self.party_slot] = pokemon;
            } else if party.len() < PARTY_SLOTS {
                self.party_slot = party.len();
                party.push(pokemon);
            }
            data::set_party(save, &party);
        } else {
            data::set_boxed(save, self.box_number, self.box_slot, &pokemon);
        }
    }

    pub(crate) fn delete(&mut self) {
        if !self.can_delete() {
            return;
        }
        let Some(save) = self.edited.as_mut() else { return };
        if self.tab == PARTY_TAB {
            let mut party = data::party(save);
            party.remove(self.party_slot);
            self.party_slot = self.party_slot.min(party.len() - 1);
            data::set_party(save, &party);
        } else {
            data::set_boxed(save, self.box_number, self.box_slot, &Pokemon::empty());
        }
    }

    pub(crate) fn trainer_tab(&mut self, ui: &mut egui::Ui, loaded: &Save, work: &mut Save) {
        let file = Trainer::read(loaded);
        let mut now = Trainer::read(work);
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                widget::card(ui, |ui| {
                    ui.label(widget::title("Trainer")).on_hover_text("The Pokémon of the player stay theirs only with the same name and the same two numbers.");
                    egui::Grid::new("save_trainer").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        value!(ui, "Name", now, file, name, text_field(ui, "player_name", &mut now.name, data::NAME_LENGTH, 180.0));
                        value!(ui, "Rival", now, file, rival, text_field(ui, "rival_name", &mut now.rival, data::NAME_LENGTH, 180.0));
                        value!(ui, "Character", now, file, gender, {
                            let names: Vec<String> = GENDERS.iter().map(|g| g.to_string()).collect();
                            let mut at = usize::from(now.gender);
                            if menu(ui, "player_gender", &mut at, &names, 180.0) {
                                now.gender = at as u8;
                            }
                        });
                        value!(ui, "Trainer ID", now, file, trainer_id, widget::spin(ui, &mut now.trainer_id, 0.0, 65_535.0));
                        value!(ui, "Secret ID", now, file, secret_id, widget::spin(ui, &mut now.secret_id, 0.0, 65_535.0));
                        value!(ui, "Money", now, file, money, widget::spin(ui, &mut now.money, 0.0, f64::from(data::MAX_MONEY.max(file.money))));
                        value!(ui, "Battle Points", now, file, battle_points, widget::spin(ui, &mut now.battle_points, 0.0, f64::from(data::MAX_POINTS.max(file.battle_points))));
                    });
                });
                ui.add_space(8.0);
                widget::card(ui, |ui| {
                    ui.label(widget::title("Play time"));
                    egui::Grid::new("save_time").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        value!(ui, "Hours", now, file, hours, widget::spin(ui, &mut now.hours, 0.0, f64::from(data::MAX_HOURS.max(u32::from(file.hours)) as u16)));
                        value!(ui, "Minutes", now, file, minutes, widget::spin(ui, &mut now.minutes, 0.0, f64::from(file.minutes.max(59))));
                        value!(ui, "Seconds", now, file, seconds, widget::spin(ui, &mut now.seconds, 0.0, f64::from(file.seconds.max(59))));
                    });
                });
            });
            ui.vertical(|ui| {
                widget::card(ui, |ui| {
                    ui.label(widget::title("Badges"));
                    if name(ui, "Badges won", now.badges != file.badges) {
                        now.badges = file.badges;
                    }
                    egui::Grid::new("save_badges").num_columns(4).spacing([14.0, 6.0]).show(ui, |ui| {
                        for (at, badge) in BADGES.iter().enumerate() {
                            let mut on = now.badges & (1 << at) != 0;
                            if widget::checkbox(ui, &mut on, badge).changed() {
                                now.badges ^= 1 << at;
                            }
                            if at % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                widget::card(ui, |ui| {
                    ui.label(widget::title("Mega Evolution"));
                    if name(ui, "What the player can do", now.mega != file.mega) {
                        now.mega = file.mega;
                    }
                    for (bit, text) in ["The player has the Mega Ring", "Rayquaza can Mega Evolve"].iter().enumerate() {
                        let mut on = now.mega & (1 << bit) != 0;
                        if widget::checkbox(ui, &mut on, text).changed() {
                            now.mega ^= 1 << bit;
                        }
                    }
                });
                ui.add_space(8.0);
                widget::card(ui, |ui| {
                    ui.label(widget::title("Options of the game"));
                    egui::Grid::new("save_options").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        for option in data::OPTIONS {
                            let old = option.get(file.options);
                            if name(ui, option.name, option.get(now.options) != old) {
                                option.set(&mut now.options, old);
                            }
                            let names: Vec<String> = option.values.iter().map(|v| v.to_string()).collect();
                            let mut at = option.get(now.options) as usize;
                            if menu(ui, option.name, &mut at, &names, 150.0) {
                                option.set(&mut now.options, at as u32);
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        });
        now.write(work);
    }

    pub(crate) fn party_tab(&mut self, ui: &mut egui::Ui, loaded: &Save, work: &mut Save) {
        let mut party = data::party(work);
        let before = party.clone();
        let old = data::party(loaded);
        self.party_slot = self.party_slot.min(PARTY_SLOTS - 1);
        ui.horizontal_wrapped(|ui| {
            let slot = self.party_slot;
            let chosen = slot < party.len();
            if widget::button(ui, "Heal the party") {
                if let Some(game) = self.data.as_mut() {
                    party.iter_mut().for_each(|p| editor::heal(game, p));
                }
            }
            ui.add_enabled_ui(chosen && slot > 0, |ui| {
                if widget::button(ui, "Move left") {
                    party.swap(slot, slot - 1);
                    self.party_slot -= 1;
                }
            });
            ui.add_enabled_ui(slot + 1 < party.len(), |ui| {
                if widget::button(ui, "Move right") {
                    party.swap(slot, slot + 1);
                    self.party_slot += 1;
                }
            });
            let free = first_free_box_place(work, self.box_number);
            ui.add_enabled_ui(chosen && party.len() > 1 && free.is_some(), |ui| {
                if widget::button(ui, "Send to a box") {
                    if let Some((number, place)) = free {
                        data::set_boxed(work, number, place, &party.remove(slot));
                        self.status = Some((true, format!("Sent to {}, place {}.", data::box_name(work, number), place + 1)));
                    }
                }
            });
            ui.add_enabled_ui(chosen, |ui| {
                if widget::button(ui, "Copy") {
                    self.copied = party.get(slot).copied();
                }
            });
            ui.add_enabled_ui(self.copied.is_some(), |ui| {
                if widget::button(ui, "Paste") {
                    if let (Some(mut copy), Some(game)) = (self.copied, self.data.as_ref()) {
                        game.calculate(&mut copy);
                        if slot < party.len() {
                            party[slot] = copy;
                        } else {
                            self.party_slot = party.len();
                            party.push(copy);
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);
        let cells: Vec<Cell> = (0..PARTY_SLOTS)
            .map(|at| {
                let pokemon = party.get(at).copied().unwrap_or_else(Pokemon::empty);
                let changed = party.get(at).map(|p| p.data) != old.get(at).map(|p| p.data);
                self.cell(ui.ctx(), &pokemon, true, changed)
            })
            .collect();
        controls::grid(ui, "party", &cells, PARTY_SLOTS, &mut self.party_slot);
        ui.add_space(8.0);
        let slot = self.party_slot;
        if let (Some(&pokemon), Some(game)) = (party.get(slot), self.data.as_mut()) {
            let mut edited = pokemon;
            let file = old.iter().find(|p| p.key() == pokemon.key() && !p.is_empty());
            let trainer = Trainer::read(work);
            ui.push_id(("party_editor", slot), |ui| editor::show(ui, game, &self.lists, &trainer, &mut edited, file, true));
            if edited.data != pokemon.data {
                game.calculate(&mut edited);
            }
            party[slot] = edited;
        } else {
            ui.label(widget::hint("This place is empty. Press + New Pokémon, or Paste."));
        }
        if party != before {
            data::set_party(work, &party);
        }
    }

    pub(crate) fn boxes_tab(&mut self, ui: &mut egui::Ui, loaded: &Save, work: &mut Save) {
        ui.horizontal_wrapped(|ui| {
            if widget::button(ui, "<") {
                self.box_number = (self.box_number + BOX_COUNT - 1) % BOX_COUNT;
            }
            let names: Vec<String> = (0..BOX_COUNT).map(|n| format!("{}.  {}", n + 1, data::box_name(work, n))).collect();
            menu(ui, "box", &mut self.box_number, &names, 220.0);
            if widget::button(ui, ">") {
                self.box_number = (self.box_number + 1) % BOX_COUNT;
            }
            let number = self.box_number;
            if name(ui, "Name", data::box_name(work, number) != data::box_name(loaded, number)) {
                data::set_box_name(work, number, &data::box_name(loaded, number));
            }
            let mut text = data::box_name(work, number);
            if text_field(ui, "box_name", &mut text, data::BOX_NAME_LENGTH, 160.0) {
                data::set_box_name(work, number, &text);
            }
            if name(ui, "Boxes of the player", data::unlocked_boxes(work) != data::unlocked_boxes(loaded)) {
                data::set_unlocked_boxes(work, data::unlocked_boxes(loaded));
            }
            let mut unlocked = data::unlocked_boxes(work);
            let most = f64::from(unlocked.max(BOX_COUNT as u8));
            let least = f64::from(unlocked.min(1));
            if widget::spin(ui, &mut unlocked, least, most) {
                data::set_unlocked_boxes(work, unlocked);
            }
        });
        let number = self.box_number;
        self.box_slot = self.box_slot.min(BOX_SLOTS - 1);
        ui.horizontal_wrapped(|ui| {
            let pokemon = data::boxed(work, number, self.box_slot);
            let filled = !pokemon.is_empty();
            // A Pokémon of a species that the studio does not know has no statistics: it stays in its box.
            let known = self.data.as_ref().is_some_and(|d| d.species(pokemon.species(), pokemon.form()).is_some());
            ui.add_enabled_ui(filled && known && data::party_count(work) < PARTY_SLOTS, |ui| {
                if widget::button(ui, "Send to the party") {
                    let mut party = data::party(work);
                    let mut moved = pokemon;
                    if let Some(game) = &self.data {
                        game.calculate(&mut moved);
                    }
                    party.push(moved);
                    data::set_party(work, &party);
                    data::set_boxed(work, number, self.box_slot, &Pokemon::empty());
                    self.status = Some((true, format!("Sent to the party, place {}.", party.len())));
                }
            });
            ui.add_enabled_ui(filled, |ui| {
                if widget::button(ui, "Copy") {
                    self.copied = Some(pokemon);
                }
            });
            ui.add_enabled_ui(self.copied.is_some(), |ui| {
                if widget::button(ui, "Paste") {
                    if let Some(copy) = self.copied {
                        data::set_boxed(work, number, self.box_slot, &copy);
                    }
                }
            });
        });
        ui.add_space(4.0);
        let cells: Vec<Cell> = (0..BOX_SLOTS)
            .map(|slot| {
                let pokemon = data::boxed(work, number, slot);
                let changed = pokemon.data != data::boxed(loaded, number, slot).data;
                self.cell(ui.ctx(), &pokemon, false, changed)
            })
            .collect();
        controls::grid(ui, "box", &cells, 6, &mut self.box_slot);
        ui.add_space(8.0);
        let slot = self.box_slot;
        let pokemon = data::boxed(work, number, slot);
        let Some(game) = self.data.as_mut() else { return };
        if pokemon.is_empty() {
            ui.label(widget::hint("This place is empty. Press + New Pokémon, or Paste."));
            return;
        }
        let mut edited = pokemon;
        let file = Some(data::boxed(loaded, number, slot)).filter(|p| p.key() == pokemon.key() && !p.is_empty());
        let trainer = Trainer::read(work);
        ui.push_id(("box_editor", number, slot), |ui| editor::show(ui, game, &self.lists, &trainer, &mut edited, file.as_ref(), false));
        if edited.data != pokemon.data {
            data::set_boxed(work, number, slot, &edited);
        }
    }

    pub(crate) fn bag_tab(&mut self, ui: &mut egui::Ui, work: &mut Save) {
        let items = self.items.clone();
        let rows: Vec<(String, u8)> = items.lock().map(|list| list.rows.iter().map(|row| (row.name.clone(), row.pocket)).collect()).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Add an item").on_hover_text("The item goes in its pocket. An item that is already there gets one more.");
            let mut item = 0u16;
            if menus::item_menu(ui, "save_add_item", &mut item, 260.0, true) && item != 0 {
                // An item goes in its own pocket, where the game looks for it.
                let number = rows.get(usize::from(item)).map_or(0, |row| row.1);
                self.pocket = POCKETS.iter().position(|p| p.1 == number).unwrap_or(0);
                let mut entries = data::pocket(work, self.pocket);
                if let Some(entry) = entries.iter_mut().find(|e| e.0 == item) {
                    entry.1 = (entry.1 + 1).min(data::MAX_ITEM_COUNT);
                } else if entries.len() < POCKETS[self.pocket].3 {
                    entries.push((item, 1));
                } else {
                    self.status = Some((false, "This pocket is full.".to_string()));
                }
                data::set_pocket(work, self.pocket, &entries);
            }
        });
        ui.add_space(4.0);
        let names: Vec<String> = (0..POCKETS.len()).map(|at| format!("{} ({})", POCKETS[at].0, data::pocket(work, at).len())).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        widget::tabs(ui, "save_pockets", &names, &mut self.pocket);
        ui.add_space(4.0);
        let pocket = self.pocket.min(POCKETS.len() - 1);
        let mut entries = data::pocket(work, pocket);
        let before = entries.clone();
        let mut removed = None;
        if entries.is_empty() {
            ui.label(widget::hint("This pocket is empty."));
        }
        egui::Grid::new(("save_bag", pocket)).num_columns(3).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
            for (at, (item, quantity)) in entries.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let icon = items.lock().ok().and_then(|mut list| list.icons.texture(ui.ctx(), usize::from(*item)));
                    match icon {
                        Some(icon) => ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(28.0, 28.0))),
                        None => ui.allocate_response(egui::vec2(28.0, 28.0), egui::Sense::hover()),
                    };
                    let text = rows.get(usize::from(*item)).map_or_else(|| format!("Item {item}"), |row| row.0.clone());
                    ui.allocate_ui_with_layout(egui::vec2(240.0, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(240.0);
                        ui.label(text);
                    });
                });
                ui.push_id(at, |ui| {
                    let (least, most) = (f64::from((*quantity).min(1)), f64::from((*quantity).max(data::MAX_ITEM_COUNT)));
                    widget::spin(ui, quantity, least, most);
                });
                if widget::button(ui, "Remove") {
                    removed = Some(at);
                }
                ui.end_row();
            }
        });
        if let Some(at) = removed {
            entries.remove(at);
        }
        if entries != before {
            data::set_pocket(work, pocket, &entries);
        }
    }

    pub(crate) fn map_tab(&mut self, ui: &mut egui::Ui, loaded: &Save, work: &mut Save) {
        let Some(game) = self.data.as_ref() else { return };
        let describe = |place: Place| {
            let map = game.map(place.map).map_or_else(|| format!("Map {}", place.map), |m| format!("{} (map {})", m.name, m.id));
            format!("{map}, tile {:.1} × {:.1}, height {:.0}", place.x / TILE, place.z / TILE, place.y)
        };
        let place = data::place(work);
        let in_file = data::place(loaded);
        let pending = data::moves_at_continue(work);
        widget::card(ui, |ui| {
            ui.label(widget::title(if pending { "Where the player goes" } else { "Where the player is" }));
            ui.label(describe(place));
            if pending {
                widget::badge(ui, "Moves at the next \"Continue\", or at once in a running game", color::TEAL);
            }
            if data::place(work) != in_file || data::moves_at_continue(work) != data::moves_at_continue(loaded) {
                ui.label(widget::hint(&format!("In the file: {}", describe(in_file))));
                if widget::button(ui, "Do not move") {
                    let old = loaded.block(format::POSITION).to_vec();
                    work.block_mut(format::POSITION).copy_from_slice(&old);
                }
            }
        });
        // A click on "Do not move" already put the place of the file back.
        let place = data::place(work);
        ui.add_space(8.0);
        let mut to = place;
        widget::card(ui, |ui| {
            ui.label(widget::title("Move the player")).on_hover_text("A choice of a map puts the player at the arrival point of that map. Change the tiles only when you know the map: a player in a wall cannot move.");
            egui::Grid::new("save_map").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("Map");
                let mut map = place.map;
                let shown = game.map(map).map_or_else(|| format!("Map {map}"), |m| format!("{}  ·  map {}", m.name, m.id));
                picker(ui, "save_map_choice", &shown, 340.0, &mut map, None, || {
                    vec![Group { title: String::new(), entries: game.maps.iter().map(|m| (m.id, format!("{}  ·  map {}", m.name, m.id))).collect() }]
                });
                // A new map starts at the point that the game uses to arrive on it, so the player is never in a wall.
                if map != place.map {
                    if let Some(found) = game.map(map) {
                        to = found.start;
                    }
                }
                ui.end_row();

                ui.label("Known points of the map");
                ui.horizontal(|ui| {
                    let found = game.map(place.map);
                    if widget::button(ui, "Arrival point") {
                        if let Some(found) = found {
                            to = found.start;
                        }
                    }
                    if widget::button(ui, "Where Fly lands") {
                        if let Some(found) = found {
                            to = found.fly;
                        }
                    }
                });
                ui.end_row();

                ui.label("Tile from the left");
                let mut x = to.x / TILE;
                if widget::decimal(ui, &mut x, 1.0, -100_000.0..=100_000.0).changed() {
                    to.x = x * TILE;
                }
                ui.end_row();

                ui.label("Tile from the top");
                let mut z = to.z / TILE;
                if widget::decimal(ui, &mut z, 1.0, -100_000.0..=100_000.0).changed() {
                    to.z = z * TILE;
                }
                ui.end_row();

                ui.label("Height");
                widget::decimal(ui, &mut to.y, 1.0, -100_000.0..=100_000.0);
                ui.end_row();

                ui.label("The player looks");
                let names: Vec<String> = FACINGS.iter().map(|f| f.to_string()).collect();
                let mut at = usize::from(to.facing.min(3));
                if menu(ui, "facing", &mut at, &names, 150.0) {
                    to.facing = at as u8;
                }
                ui.end_row();
            });
        });
        if to != place {
            data::move_player(work, to);
        }
    }
}
