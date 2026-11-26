//! The editor of one Pokémon, the same for the party and for the boxes, and the changes that go together: a new species also
//! changes the name, the ability, the gender and the experience.

use crate::context::GameData;
use crate::controls::{menu, text_field};
use crate::data::Trainer;
use crate::pokemon::{self, Pokemon, MAX_EV, MAX_EV_TOTAL, STAT_NAMES};
use sango_species::menus;
use sango_ui::theme::color;
use sango_ui::{egui, widget};

/// The balls of the game, from the number 1.
pub const BALLS: [&str; 25] = [
    "Master Ball", "Ultra Ball", "Great Ball", "Poké Ball", "Safari Ball", "Net Ball", "Dive Ball", "Nest Ball", "Repeat Ball",
    "Timer Ball", "Luxury Ball", "Premier Ball", "Dusk Ball", "Heal Ball", "Quick Ball", "Cherish Ball", "Fast Ball", "Level Ball",
    "Lure Ball", "Heavy Ball", "Love Ball", "Friend Ball", "Moon Ball", "Sport Ball", "Dream Ball",
];
const GENDERS: [&str; 3] = ["Male", "Female", "No gender"];
/// The most steps of an Egg and the most friendship.
const MOST: f64 = 255.0;

/// The names that the menus of the editor show.
#[derive(Default)]
pub struct Lists {
    /// The abilities, from the number 1.
    pub abilities: Vec<String>,
}

/// A number that changes each time: for the personality values of a new Pokémon.
pub fn random() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    // Each new state of the library has other keys.
    std::collections::hash_map::RandomState::new().build_hasher().finish() as u32
}

/// The year (from 2000), the month and the day of today (UTC).
pub fn today() -> (u8, u8, u8) {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400);
    date_of(days as i64)
}

/// The date of a number of days after the 1st of January 1970.
fn date_of(days: i64) -> (u8, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    ((year - 2000).clamp(0, 255) as u8, month as u8, day as u8)
}

/// The genders that the species can have.
fn genders(ratio: u8) -> &'static [u8] {
    match ratio {
        0 => &[0],
        254 => &[1],
        255 => &[2],
        _ => &[0, 1],
    }
}

/// Gives a Pokémon another species or form: its level stays, and what depends on the species follows.
pub fn change_species(data: &mut GameData, pokemon: &mut Pokemon, species: u16, form: u8) {
    let level = data.level(pokemon.species(), pokemon.form(), pokemon.experience()).unwrap_or(5);
    let new_species = species != pokemon.species();
    pokemon.set_species(species);
    pokemon.set_form(form);
    let Some(found) = data.species(species, form) else { return };
    if let Some(points) = data.experience(species, form, level) {
        pokemon.set_experience(points);
    }
    if new_species && !pokemon.has_nickname() {
        let name = data.species_name(species, pokemon.language());
        pokemon.set_nickname(&name);
    }
    let slot = match pokemon.ability_slot() {
        2 => 1,
        4 => 2,
        _ => 0,
    };
    pokemon.set_ability([found.ability_1, found.ability_2, found.ability_hidden][slot]);
    let allowed = genders(found.gender_ratio);
    if !allowed.contains(&pokemon.gender()) {
        pokemon.set_gender(allowed[0]);
    }
}

/// A new Pokémon of the player: level 5, the best individual values, and the moves of its level.
pub fn new_pokemon(data: &mut GameData, trainer: &Trainer, origin: Origin, species: u16) -> Pokemon {
    const LEVEL: u8 = 5;
    let mut new = Pokemon::empty();
    new.set_key(random());
    new.set_personality(random());
    new.set_trainer_id(trainer.trainer_id);
    new.set_secret_id(trainer.secret_id);
    new.set_shiny(false);
    new.set_language(origin.language);
    new.set_origin(origin.version, origin.country, origin.region, origin.console, origin.language);
    new.set_species(species);
    new.set_ability_slot(1);
    let name = data.species_name(species, origin.language);
    new.set_nickname(&name);
    if let Some(found) = data.species(species, 0) {
        new.set_ability(found.ability_1);
        new.set_friendship(found.base_friendship);
        let allowed = genders(found.gender_ratio);
        let female = allowed.len() > 1 && (random() & 0xFF) < u32::from(found.gender_ratio);
        new.set_gender(if female { 1 } else { allowed[0] });
    }
    if let Some(points) = data.experience(species, 0, LEVEL) {
        new.set_experience(points);
    }
    new.set_nature((random() % 25) as u8);
    for stat in 0..6 {
        new.set_iv(stat, 31);
    }
    for (slot, id) in data.moves_at(species, 0, LEVEL).into_iter().enumerate() {
        new.set_move_id(slot, id);
        let points = data.move_points(id);
        new.set_pp(slot, points);
    }
    new.set_trainer_name(&trainer.name);
    new.set_trainer_gender(trainer.gender);
    new.set_ball(4);
    new.set_met_level(LEVEL);
    new.set_met_date(today());
    new.set_met_location(origin.place);
    data.calculate(&mut new);
    new
}

/// What a new Pokémon says about the game where it comes from.
#[derive(Clone, Copy, Debug, Default)]
pub struct Origin {
    pub version: u8,
    pub country: u8,
    pub region: u8,
    pub console: u8,
    pub language: u8,
    /// The number of the name of the place where it is met.
    pub place: u16,
}

/// The name of a line, blue with ↺ when the value is not the one of the file. Gives true at the click on ↺.
fn name(ui: &mut egui::Ui, text: &str, changed: bool) -> bool {
    ui.horizontal(|ui| widget::field_name(ui, text, changed).1).inner
}

/// A line of the grid: the name with its mark, then the control. A click on ↺ puts back the value of the file.
macro_rules! line {
    ($ui:expr, $label:expr, $pokemon:ident, $file:expr, $get:ident, $set:ident, $control:expr) => {{
        let old = $file.map(|f| f.$get());
        if name($ui, $label, old.as_ref().is_some_and(|old| *old != $pokemon.$get())) {
            if let Some(old) = old {
                $pokemon.$set(old);
            }
        }
        $control;
        $ui.end_row();
    }};
}

fn section(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        widget::card(ui, |ui| {
            ui.label(widget::title(title));
            ui.add_space(4.0);
            content(ui);
        });
    });
}

/// The editor of a Pokémon that is not empty. `file` is the same Pokémon as the file has it.
pub fn show(ui: &mut egui::Ui, data: &mut GameData, lists: &Lists, trainer: &Trainer, pokemon: &mut Pokemon, file: Option<&Pokemon>, in_party: bool) {
    if !pokemon.sound {
        widget::badge(ui, "The checksum of this Pokémon is wrong: the game shows a bad Egg. A change writes a good checksum.", color::ROSE);
    }
    if data.species(pokemon.species(), pokemon.form()).is_none() {
        widget::badge(ui, "Species not known: its pack is not on the SD card", color::AMBER);
    }
    ui.horizontal_wrapped(|ui| {
        section(ui, "Pokémon", |ui| identity(ui, data, lists, pokemon, file));
        ui.vertical(|ui| {
            section(ui, "Moves", |ui| moves(ui, data, pokemon, file));
            ui.add_space(8.0);
            section(ui, "Statistics", |ui| statistics(ui, data, pokemon, file, in_party));
            ui.add_space(8.0);
            section(ui, "Where it comes from", |ui| origin(ui, trainer, pokemon, file));
        });
    });
}

fn identity(ui: &mut egui::Ui, data: &mut GameData, lists: &Lists, pokemon: &mut Pokemon, file: Option<&Pokemon>) {
    egui::Grid::new("save_identity").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
        let old = file.map(|f| (f.species(), f.form()));
        if name(ui, "Species", old.is_some_and(|old| old != (pokemon.species(), pokemon.form()))) {
            if let Some((species, form)) = old {
                change_species(data, pokemon, species, form);
            }
        }
        let mut value = (usize::from(pokemon.species()), usize::from(pokemon.form()));
        if menus::pokemon_menu(ui, "save_species", &mut value, 260.0, None, None) && value.0 != 0 {
            change_species(data, pokemon, value.0 as u16, value.1 as u8);
        }
        ui.end_row();

        line!(ui, "Nickname", pokemon, file, nickname_value, set_nickname_value, ui.horizontal(|ui| {
            let mut given = pokemon.has_nickname();
            if widget::checkbox(ui, &mut given, "Given").changed() {
                pokemon.set_has_nickname(given);
                if !given {
                    let name = data.species_name(pokemon.species(), pokemon.language());
                    pokemon.set_nickname(&name);
                }
            }
            let mut text = pokemon.nickname();
            if text_field(ui, "nickname", &mut text, pokemon::NAME_LENGTH, 150.0) {
                pokemon.set_nickname(&text);
                pokemon.set_has_nickname(true);
            }
        }));

        let level = data.level(pokemon.species(), pokemon.form(), pokemon.experience());
        let old = file.and_then(|f| data.level(f.species(), f.form(), f.experience()));
        if name(ui, "Level", old.is_some() && old != level) {
            if let Some(file) = file {
                pokemon.set_experience(file.experience());
            }
        }
        match level {
            Some(level) => {
                let mut chosen = level;
                if widget::spin(ui, &mut chosen, 1.0, 100.0) && chosen != level {
                    if let Some(points) = data.experience(pokemon.species(), pokemon.form(), chosen) {
                        pokemon.set_experience(points);
                    }
                }
            }
            None => {
                ui.label("—");
            }
        }
        ui.end_row();

        line!(ui, "Experience", pokemon, file, experience, set_experience, {
            let most = data.experience(pokemon.species(), pokemon.form(), 100).unwrap_or(u32::MAX).max(pokemon.experience());
            let mut points = pokemon.experience();
            if widget::spin(ui, &mut points, 0.0, f64::from(most)) {
                pokemon.set_experience(points);
            }
        });

        line!(ui, "Shiny", pokemon, file, is_shiny, set_shiny, {
            let mut shiny = pokemon.is_shiny();
            if widget::checkbox(ui, &mut shiny, "Shiny").changed() {
                pokemon.set_shiny(shiny);
            }
        });

        line!(ui, "Egg", pokemon, file, is_egg, set_egg, {
            let mut egg = pokemon.is_egg();
            if widget::checkbox(ui, &mut egg, "Still in its Egg").changed() {
                pokemon.set_egg(egg);
            }
        });

        line!(ui, "Gender", pokemon, file, gender, set_gender, {
            let allowed = data.species(pokemon.species(), pokemon.form()).map_or(&[0u8, 1, 2][..], |s| genders(s.gender_ratio));
            let names: Vec<String> = allowed.iter().map(|&g| GENDERS[usize::from(g)].to_string()).collect();
            let mut at = allowed.iter().position(|&g| g == pokemon.gender()).unwrap_or(names.len());
            if menu(ui, "gender", &mut at, &names, 200.0) {
                pokemon.set_gender(allowed[at]);
            }
        });

        line!(ui, "Nature", pokemon, file, nature, set_nature, {
            let names: Vec<String> = data.natures.natures.iter().map(|n| format!("{}  ({})", n.name, n.summary())).collect();
            let mut at = usize::from(pokemon.nature());
            if menu(ui, "nature", &mut at, &names, 260.0) {
                pokemon.set_nature(at as u8);
            }
        });

        line!(ui, "Ability", pokemon, file, ability, set_ability, {
            let slots = data.species(pokemon.species(), pokemon.form()).map_or([pokemon.ability(); 3], |s| [s.ability_1, s.ability_2, s.ability_hidden]);
            let name = |id: u8| lists.abilities.get(usize::from(id).wrapping_sub(1)).cloned().unwrap_or_else(|| format!("Ability {id}"));
            let mut names: Vec<String> = ["", "", " (hidden)"].iter().zip(slots).map(|(note, id)| format!("{}{note}", name(id))).collect();
            let current = match pokemon.ability_slot() {
                2 => 1,
                4 => 2,
                _ => 0,
            };
            // An ability that the species does not have stays in the menu.
            let mut at = if slots[current] == pokemon.ability() {
                current
            } else {
                names.push(format!("{} (not of this species)", name(pokemon.ability())));
                3
            };
            if menu(ui, "ability", &mut at, &names, 260.0) && at < 3 {
                pokemon.set_ability(slots[at]);
                pokemon.set_ability_slot(1 << at);
            }
        });

        line!(ui, "Held item", pokemon, file, item, set_item, {
            let mut item = pokemon.item();
            if menus::item_menu(ui, "save_held", &mut item, 260.0, true) {
                pokemon.set_item(item);
            }
        });

        let egg = pokemon.is_egg();
        line!(ui, if egg { "Egg steps left" } else { "Friendship" }, pokemon, file, friendship, set_friendship, {
            let mut value = pokemon.friendship();
            if widget::spin(ui, &mut value, 0.0, MOST) {
                pokemon.set_friendship(value);
            }
        });

        line!(ui, "Ball", pokemon, file, ball, set_ball, {
            let names: Vec<String> = BALLS.iter().map(|name| name.to_string()).collect();
            let mut at = usize::from(pokemon.ball()).wrapping_sub(1).min(names.len());
            if menu(ui, "ball", &mut at, &names, 200.0) {
                pokemon.set_ball(at as u8 + 1);
            }
        });
    });
}

impl Pokemon {
    /// The name and the switch "a name of the player" go together.
    fn nickname_value(&self) -> (String, bool) {
        (self.nickname(), self.has_nickname())
    }

    fn set_nickname_value(&mut self, (name, given): (String, bool)) {
        self.set_nickname(&name);
        self.set_has_nickname(given);
    }
}

fn moves(ui: &mut egui::Ui, data: &mut GameData, pokemon: &mut Pokemon, file: Option<&Pokemon>) {
    egui::Grid::new("save_moves").num_columns(4).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(widget::hint("Move"));
        ui.label("");
        ui.label(widget::hint("PP left"));
        ui.label(widget::hint("PP Ups"));
        ui.end_row();
        for slot in 0..4 {
            let read = |p: &Pokemon| (p.move_id(slot), p.pp(slot), p.pp_ups(slot));
            let old = file.map(read);
            if name(ui, &format!("{}", slot + 1), old.is_some_and(|old| old != read(pokemon))) {
                if let Some((id, pp, ups)) = old {
                    pokemon.set_move_id(slot, id);
                    pokemon.set_pp(slot, pp);
                    pokemon.set_pp_ups(slot, ups);
                }
            }
            let mut id = pokemon.move_id(slot);
            ui.horizontal(|ui| {
                if menus::move_menu(ui, &format!("save_move_{slot}"), &mut id, 220.0) {
                    pokemon.set_move_id(slot, id);
                    pokemon.set_pp_ups(slot, 0);
                    let points = data.move_points(id);
                    pokemon.set_pp(slot, points);
                }
                if id != 0 && widget::button(ui, "Remove") {
                    // The moves after it move up: the game does not leave a hole.
                    for at in slot..4 {
                        let next = (at + 1 < 4).then(|| (pokemon.move_id(at + 1), pokemon.pp(at + 1), pokemon.pp_ups(at + 1))).unwrap_or_default();
                        pokemon.set_move_id(at, next.0);
                        pokemon.set_pp(at, next.1);
                        pokemon.set_pp_ups(at, next.2);
                    }
                }
            });
            let most = data.full_points(pokemon.move_id(slot), pokemon.pp_ups(slot)).max(pokemon.pp(slot));
            let mut pp = pokemon.pp(slot);
            if widget::spin(ui, &mut pp, 0.0, f64::from(most)) {
                pokemon.set_pp(slot, pp);
            }
            let mut ups = pokemon.pp_ups(slot);
            let most_ups = f64::from(ups.max(3));
            if widget::spin(ui, &mut ups, 0.0, most_ups) && ups != pokemon.pp_ups(slot) {
                pokemon.set_pp_ups(slot, ups);
                let full = data.full_points(pokemon.move_id(slot), ups);
                pokemon.set_pp(slot, full);
            }
            ui.end_row();
        }
    });
}

fn statistics(ui: &mut egui::Ui, data: &mut GameData, pokemon: &mut Pokemon, file: Option<&Pokemon>, in_party: bool) {
    // A box does not keep the statistics: they are calculated for the page.
    let mut shown = *pokemon;
    data.calculate(&mut shown);
    egui::Grid::new("save_stats").num_columns(4).spacing([14.0, 6.0]).show(ui, |ui| {
        ui.label(widget::hint("Statistic"));
        ui.label(widget::hint("Individual (IV)"));
        ui.label(widget::hint("Effort (EV)"));
        ui.label(widget::hint("Value"));
        ui.end_row();
        for (stat, stat_name) in STAT_NAMES.iter().enumerate() {
            let old = file.map(|f| (f.iv(stat), f.ev(stat)));
            if name(ui, stat_name, old.is_some_and(|old| old != (pokemon.iv(stat), pokemon.ev(stat)))) {
                if let Some((iv, ev)) = old {
                    pokemon.set_iv(stat, iv);
                    pokemon.set_ev(stat, ev);
                }
            }
            let mut iv = pokemon.iv(stat);
            if widget::spin(ui, &mut iv, 0.0, 31.0) {
                pokemon.set_iv(stat, iv);
            }
            let mut ev = pokemon.ev(stat);
            if widget::spin(ui, &mut ev, 0.0, f64::from(MAX_EV.max(pokemon.ev(stat)))) {
                pokemon.set_ev(stat, ev);
            }
            ui.label(egui::RichText::new(shown.stat(stat).to_string()).strong());
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        let total = pokemon.ev_total();
        let text = format!("Total of the effort values: {total} / {MAX_EV_TOTAL}");
        if total > MAX_EV_TOTAL {
            widget::badge(ui, &text, color::AMBER);
        } else {
            ui.label(widget::hint(&text));
        }
    });
    ui.horizontal(|ui| {
        if widget::button(ui, "Best IVs") {
            (0..6).for_each(|stat| pokemon.set_iv(stat, 31));
        }
        if widget::button(ui, "No EVs") {
            (0..6).for_each(|stat| pokemon.set_ev(stat, 0));
        }
    });
    if in_party {
        ui.horizontal(|ui| {
            ui.label(widget::hint("HP now"));
            let mut hp = pokemon.hp();
            if widget::spin(ui, &mut hp, 0.0, f64::from(pokemon.stat(0).max(pokemon.hp()))) {
                pokemon.set_hp(hp);
            }
            ui.label(widget::hint(&format!("/ {}", pokemon.stat(0))));
            if pokemon.status() != 0 {
                widget::badge(ui, "Status condition", color::AMBER);
            }
            if widget::button(ui, "Heal") {
                heal(data, pokemon);
            }
        });
    }
}

/// Gives back the HP and the PP, and removes the status condition.
pub fn heal(data: &mut GameData, pokemon: &mut Pokemon) {
    pokemon.heal();
    for slot in 0..4 {
        let full = data.full_points(pokemon.move_id(slot), pokemon.pp_ups(slot));
        pokemon.set_pp(slot, full);
    }
}

fn origin(ui: &mut egui::Ui, trainer: &Trainer, pokemon: &mut Pokemon, file: Option<&Pokemon>) {
    egui::Grid::new("save_origin").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
        let read = |p: &Pokemon| (p.trainer_name(), p.trainer_id(), p.secret_id());
        let old = file.map(read);
        if name(ui, "First trainer", old.as_ref().is_some_and(|old| *old != read(pokemon))) {
            if let Some((trainer_name, id, secret)) = old {
                pokemon.set_trainer_name(&trainer_name);
                pokemon.set_trainer_id(id);
                pokemon.set_secret_id(secret);
            }
        }
        ui.horizontal(|ui| {
            let mut text = pokemon.trainer_name();
            if text_field(ui, "first_trainer", &mut text, pokemon::NAME_LENGTH, 130.0) {
                pokemon.set_trainer_name(&text);
            }
            ui.label(widget::hint(&format!("ID {:05}", pokemon.trainer_id())));
            let mine = read(pokemon) == (trainer.name.clone(), trainer.trainer_id, trainer.secret_id);
            if !mine && widget::button(ui, "Make it mine") {
                // A shiny Pokémon stays shiny with the numbers of the player.
                let shiny = pokemon.is_shiny();
                pokemon.set_trainer_name(&trainer.name);
                pokemon.set_trainer_id(trainer.trainer_id);
                pokemon.set_secret_id(trainer.secret_id);
                pokemon.set_trainer_gender(trainer.gender);
                pokemon.set_shiny(shiny);
            }
        });
        ui.end_row();

        line!(ui, "Met at level", pokemon, file, met_level, set_met_level, {
            let mut level = pokemon.met_level();
            let most = f64::from(level.max(100));
            if widget::spin(ui, &mut level, 0.0, most) {
                pokemon.set_met_level(level);
            }
        });

        line!(ui, "Met on (year, month, day)", pokemon, file, met_date, set_met_date, ui.horizontal(|ui| {
            let (mut year, mut month, mut day) = pokemon.met_date();
            let mut full = 2000 + u16::from(year);
            let (months, days) = (f64::from(month.max(12)), f64::from(day.max(31)));
            let changed = widget::spin(ui, &mut full, 2000.0, 2255.0) | widget::spin(ui, &mut month, 0.0, months) | widget::spin(ui, &mut day, 0.0, days);
            if changed {
                year = (full - 2000) as u8;
                pokemon.set_met_date((year, month, day));
            }
        }));

        line!(ui, "Fateful encounter", pokemon, file, from_event, set_from_event, {
            let mut event = pokemon.from_event();
            if widget::checkbox(ui, &mut event, "From an event").changed() {
                pokemon.set_from_event(event);
            }
        });
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_day_gives_its_date() {
        assert_eq!(date_of(0), (0, 1, 1));
        assert_eq!(date_of(11_016), (0, 2, 29));
        assert_eq!(date_of(20_783), (26, 11, 26));
    }
}
