//! The team builder of the Team tab: the same ideas as the team builder of Pokémon Showdown, with the data of Smogon for Gen 6.
//!
//! One slot for each Pokémon, an editor for the open Pokémon, the base stats with the EVs and the IVs, and menus that put the useful
//! things first: the sets of the players, then what the Pokémon can learn, then (with the Legal switch off) all the rest.
//! The tiers (Uber, OU, UU...), the learnable moves and the sets come from `gen6.rs` (see `tools/gen_gen6.js`).

use crate::gen6::{self, Set};
use crate::showdown::{self, NATURES};
use sango_ui::focus;
use sango_ui::picker::{picker, Group};
use sango_core::trainer::{self as data, BattleExtras, PokemonExtras, Team, TrainerPokemon};
use sango_species::{battle_icons, SpeciesIcons, Summary, ABILITY_NAMES, ITEM_NAMES, MOVE_NAMES, SPECIES, TYPE_NAMES};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};

/// The link of the team builder of Pokémon Showdown.
pub const SHOWDOWN_LINK: &str = "https://play.pokemonshowdown.com/teambuilder";

/// The stats, in the order of the files.
const STATS: [&str; 6] = ["HP", "Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];
/// The stat that a nature raises or lowers, from the number of the nature: Attack, Defense, Speed, Sp. Atk, Sp. Def.
const NATURE_STATS: [usize; 5] = [1, 2, 5, 3, 4];
/// The most EVs of a stat and in all, in the games.
const EV_STAT_MAX: u32 = 252;
const EV_TOTAL_MAX: u32 = 510;

/// The state of the builder.
pub struct Builder {
    summaries: Vec<Option<Summary>>,
    icons: SpeciesIcons,
    /// The open Pokémon.
    selected: usize,
    /// With it the menus show only what the Pokémon can have in the games, and the EVs follow the rules.
    legal: bool,
    /// The text of the import box.
    text: String,
    show_text: bool,
    status: Option<(bool, String)>,
}

impl Default for Builder {
    fn default() -> Self {
        Builder { summaries: Vec::new(), icons: SpeciesIcons::default(), selected: 0, legal: true, text: String::new(), show_text: false, status: None }
    }
}

/// The tier of a species of the game (an index of `gen6::TIER_NAMES`).
fn tier_of(species: usize) -> usize {
    gen6::TIERS.get(species).copied().map_or(gen6::TIER_NAMES.len() - 1, usize::from)
}

/// True, false, or `None` when the data of Gen 6 has nothing about the species (a species of a pack).
fn learns(species: usize, mv: usize) -> Option<bool> {
    if species == 0 || species >= gen6::TIERS.len() || mv == 0 || mv > MOVE_NAMES.len() {
        return None;
    }
    let bit = mv - 1;
    Some(gen6::LEARN[species * gen6::LEARN_BYTES + bit / 8] & (1 << (bit % 8)) != 0)
}

/// The sets of the players for a species.
fn sets_of(species: usize) -> &'static [Set] {
    gen6::SETS.binary_search_by_key(&(species as u16), |(number, _)| *number).map(|at| gen6::SETS[at].1).unwrap_or(&[])
}

/// The things of the sets of a species, the most common first.
fn used_by_sets(species: usize, pick: impl Fn(&Set) -> Vec<u16>) -> Vec<u16> {
    let mut counts: Vec<(u16, usize)> = Vec::new();
    for set in sets_of(species) {
        for thing in pick(set).into_iter().filter(|&thing| thing != 0) {
            match counts.iter_mut().find(|(known, _)| *known == thing) {
                Some(entry) => entry.1 += 1,
                None => counts.push((thing, 1)),
            }
        }
    }
    counts.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    counts.into_iter().map(|(thing, _)| thing).collect()
}

fn species_name(species: usize) -> &'static str {
    SPECIES.get(species.wrapping_sub(1)).copied().unwrap_or("?")
}

fn entry(list: &[&str], number: u16) -> (u16, String) {
    (number, list.get((number as usize).wrapping_sub(1)).copied().unwrap_or("?").to_string())
}

/// The stat of a Pokémon at a level, with the formula of the games.
fn final_stat(stat: usize, base: u32, level: u32, iv: u32, ev: u32, nature: u8) -> u32 {
    let core = (2 * base + iv + ev / 4) * level / 100;
    if stat == 0 {
        return core + level + 10;
    }
    let plain = core + 5;
    match nature_change(nature, stat) {
        1 => plain * 110 / 100,
        -1 => plain * 90 / 100,
        _ => plain,
    }
}

/// 1 when the nature raises the stat, -1 when it lowers it, else 0.
fn nature_change(nature: u8, stat: usize) -> i32 {
    if nature >= 25 {
        return 0;
    }
    let (up, down) = (NATURE_STATS[nature as usize / 5], NATURE_STATS[nature as usize % 5]);
    if up == down {
        0
    } else if stat == up {
        1
    } else if stat == down {
        -1
    } else {
        0
    }
}

/// The text of a nature with the stats that it changes: "Adamant (+Atk, -Sp. Atk)".
fn nature_text(nature: u8) -> String {
    let Some(name) = NATURES.get(nature as usize) else { return "Of the game".to_string() };
    let find = |sign: i32| (1..6).find(|&stat| nature_change(nature, stat) == sign).map(|stat| STATS[stat]);
    match (find(1), find(-1)) {
        (Some(up), Some(down)) => format!("{name} (+{up}, -{down})"),
        _ => format!("{name} (neutral)"),
    }
}

impl Builder {
    /// Reads what the builder needs of the species. Call it when the dump changes.
    pub fn open(&mut self, session: &Session) {
        self.summaries = sango_species::summaries(session).unwrap_or_default();
        if let Some(dump) = session.dump.as_ref() {
            self.icons.open(dump);
            battle_icons::load(dump);
        }
        self.selected = 0;
    }

    fn summary(&self, species: usize) -> Option<Summary> {
        self.summaries.get(species).copied().flatten()
    }

    /// The ability of a species for a place of the data (1, 2 or 3).
    pub fn ability_of(&self, species: usize, place: u8) -> u16 {
        self.summary(species).and_then(|s| s.abilities.get(place as usize - 1).copied()).map_or(0, u16::from)
    }

    /// The exact ability of a Pokémon: the one chosen, else the one of its place.
    fn ability_shown(&self, pokemon: &TrainerPokemon, more: &PokemonExtras) -> u16 {
        if more.ability != 0 {
            more.ability
        } else {
            self.ability_of(pokemon.species as usize, pokemon.ability.max(1))
        }
    }

    /// What is not right for the games: a move the Pokémon cannot learn, an ability it does not have, EVs over the limits.
    fn problems(&self, pokemon: &TrainerPokemon, more: &PokemonExtras, team: &Team) -> Vec<String> {
        let species = pokemon.species as usize;
        let mut list = Vec::new();
        if team.moves {
            for &mv in pokemon.moves.iter().filter(|&&mv| mv != 0) {
                if learns(species, mv as usize) == Some(false) {
                    list.push(format!("{} cannot learn {} in Gen 6.", species_name(species), MOVE_NAMES.get((mv as usize).wrapping_sub(1)).unwrap_or(&"?")));
                }
            }
            for (at, &mv) in pokemon.moves.iter().enumerate() {
                if mv != 0 && pokemon.moves[..at].contains(&mv) {
                    list.push(format!("{} is twice in the moves.", MOVE_NAMES.get((mv as usize).wrapping_sub(1)).unwrap_or(&"?")));
                }
            }
        }
        if let (Some(summary), ability) = (self.summary(species), self.ability_shown(pokemon, more)) {
            if ability != 0 && !summary.abilities.iter().any(|&a| u16::from(a) == ability) {
                list.push(format!("{} does not have the ability {}.", species_name(species), ABILITY_NAMES.get((ability as usize).wrapping_sub(1)).unwrap_or(&"?")));
            }
        }
        let total: u32 = more.evs.iter().map(|&v| u32::from(v)).sum();
        if more.evs.iter().any(|&v| u32::from(v) > EV_STAT_MAX) {
            list.push(format!("A stat has more than {EV_STAT_MAX} EVs."));
        }
        if total > EV_TOTAL_MAX {
            list.push(format!("The EVs add up to {total}: the most is {EV_TOTAL_MAX}."));
        }
        list
    }

    /// The whole tab: the bar, the slots and the editor of the open Pokémon.
    pub fn ui(&mut self, ui: &mut egui::Ui, team: &mut Team, extras: &mut BattleExtras) {
        extras.pokemon.resize(team.pokemon.len(), PokemonExtras::none());
        self.selected = self.selected.min(team.pokemon.len().saturating_sub(1));
        self.bar(ui, team, extras);
        ui.add_space(8.0);
        self.slots(ui, team, extras);
        ui.add_space(8.0);
        if !team.pokemon.is_empty() {
            self.editor(ui, team, extras);
        }
    }

    /// Replaces the team with the Pokémon of a text of the format of Pokémon Showdown. The result goes in the status line.
    pub fn import_text(&mut self, text: &str, team: &mut Team, extras: &mut BattleExtras) {
        let (list, mut problems) = showdown::import(text);
        if list.is_empty() {
            problems.push("No Pokémon in the text.".to_string());
            self.status = Some((false, problems.join(" ")));
            return;
        }
        let count = list.len();
        team.moves |= list.iter().any(|one| one.pokemon.moves[0] != 0);
        team.items |= list.iter().any(|one| one.pokemon.item != 0);
        team.pokemon = list.iter().map(|one| one.pokemon).collect();
        extras.pokemon = list.into_iter().map(|one| one.more).collect();
        for place in 0..team.pokemon.len() {
            let (pokemon, more) = (&mut team.pokemon[place], &extras.pokemon[place]);
            pokemon.ability = self.summary(pokemon.species as usize).map_or(0, |s| s.abilities.iter().position(|&a| u16::from(a) == more.ability).map_or(0, |at| at as u8 + 1));
        }
        self.selected = 0;
        self.show_text = false;
        problems.insert(0, format!("{count} Pokémon imported."));
        self.status = Some((true, problems.join(" ")));
    }

    /// The same, for the tests.
    #[cfg(test)]
    pub fn import_for_test(&mut self, text: &str, team: &mut Team, extras: &mut BattleExtras) {
        self.import_text(text, team, extras);
    }

    fn bar(&mut self, ui: &mut egui::Ui, team: &mut Team, extras: &mut BattleExtras) {
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::title("Team builder"));
                ui.add_space(8.0);
                widget::chip(ui, "Legal", &mut self.legal).on_hover_text("On: the menus show what the Pokémon can have in Gen 6, and the EVs follow the rules (252 for a stat, 510 in all).\nOff: all the moves, abilities and items, the legal ones first.");
                ui.add_space(8.0);
                widget::chip(ui, "Choose the moves", &mut team.moves);
                widget::chip(ui, "Choose the items", &mut team.items);
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Copy the team (Showdown text)").clicked() {
                    let text = showdown::export(team, extras, &|species, place| self.ability_of(species, place));
                    ui.ctx().copy_text(text);
                    self.status = Some((true, "The team is copied: paste it in the team builder of Pokémon Showdown.".to_string()));
                }
                if ui.button("Import a team…").clicked() {
                    self.show_text = !self.show_text;
                }
                ui.hyperlink_to("Open the team builder of Pokémon Showdown", SHOWDOWN_LINK);
            });
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
            if self.show_text {
                ui.add_space(4.0);
                ui.label(widget::hint("Paste a team in the format of Pokémon Showdown. The Pokémon replace the team."));
                ui.add(egui::TextEdit::multiline(&mut self.text).desired_rows(8).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                ui.horizontal(|ui| {
                    if widget::confirm_button(ui, "Import", "Click again: the team is replaced") {
                        let text = std::mem::take(&mut self.text);
                        self.import_text(&text, team, extras);
                        self.text = text;
                    }
                    if ui.button("Close").clicked() {
                        self.show_text = false;
                    }
                });
            }
        });
    }

    fn slots(&mut self, ui: &mut egui::Ui, team: &mut Team, extras: &mut BattleExtras) {
        let size = egui::vec2(168.0, 48.0);
        let strip_id = egui::Id::new("builder_slots");
        let strip = ui.horizontal_wrapped(|ui| {
            for place in 0..team.pokemon.len() {
                let pokemon = team.pokemon[place];
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                let chosen = place == self.selected;
                let fill = if chosen { color::ACCENT_SOFT } else { color::WHITE };
                ui.painter().rect(rect, 10.0, fill, egui::Stroke::new(if chosen { 2.0_f32 } else { 1.0_f32 }, if chosen { color::ACCENT } else { color::BORDER }));
                if let Some(texture) = self.icons.get(ui.ctx(), pokemon.species as usize) {
                    let icon = egui::Rect::from_min_size(egui::pos2(rect.left() + 6.0, rect.center().y - 15.0), egui::vec2(40.0, 30.0));
                    ui.painter().image(texture.id(), icon, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
                let name = extras.pokemon.get(place).map(|m| m.nickname.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| SPECIES.get((pokemon.species as usize).wrapping_sub(1)).unwrap_or(&"?").to_string());
                ui.painter().text(egui::pos2(rect.left() + 52.0, rect.center().y - 8.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(14.0), color::TEXT);
                ui.painter().text(egui::pos2(rect.left() + 52.0, rect.center().y + 9.0), egui::Align2::LEFT_CENTER, format!("Lv. {} · {}", pokemon.level, gen6::TIER_NAMES[tier_of(pokemon.species as usize)]), egui::FontId::proportional(12.0), color::MUTED);
                if response.clicked() {
                    self.selected = place;
                    widget::set_active_id(ui, strip_id);
                }
            }
            if team.pokemon.len() < data::MAX_TEAM {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(size.x * 0.6, size.y), egui::Sense::click());
                ui.painter().rect(rect, 10.0, color::SURFACE, egui::Stroke::new(1.0_f32, color::BORDER));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "+ Add", egui::FontId::proportional(14.0), color::ACCENT);
                if response.clicked() {
                    let level = team.pokemon.last().map_or(50, |last| last.level);
                    team.pokemon.push(TrainerPokemon::new(1, level));
                    extras.pokemon.push(PokemonExtras::none());
                    self.selected = team.pokemon.len() - 1;
                    widget::set_active_id(ui, strip_id);
                }
            }
        });
        // The strip of the slots has the focus after a click: the arrows left and right change the Pokémon.
        if focus::focus(ui, strip_id, strip.response.rect, false, false, None) {
            if let Some(to) = focus::stepped(self.selected, focus::steps(ui, true), team.pokemon.len()) {
                self.selected = to;
            }
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, team: &mut Team, extras: &mut BattleExtras) {
        let place = self.selected;
        let count = team.pokemon.len();
        let legal = self.legal;
        let summary = self.summary(team.pokemon[place].species as usize);
        let problems = self.problems(&team.pokemon[place], &extras.pokemon[place], team);
        let (moves_on, items_on) = (team.moves, team.items);
        let mut action: Option<u8> = None;
        widget::card(ui, |ui| {
            let pokemon = &mut team.pokemon[place];
            let more = &mut extras.pokemon[place];
            ui.horizontal_wrapped(|ui| {
                if let Some(texture) = self.icons.get(ui.ctx(), pokemon.species as usize) {
                    ui.image((texture.id(), egui::vec2(40.0, 30.0)));
                }
                ui.label(egui::RichText::new(SPECIES.get((pokemon.species as usize).wrapping_sub(1)).copied().unwrap_or("?")).strong().size(18.0));
                widget::badge(ui, gen6::TIER_NAMES[tier_of(pokemon.species as usize)], color::ACCENT);
                if let Some(summary) = summary {
                    for kind in if summary.types[0] == summary.types[1] { vec![summary.types[0]] } else { summary.types.to_vec() } {
                        match battle_icons::type_texture(ui.ctx(), kind) {
                            Some(texture) => {
                                ui.image((texture.id(), battle_icons::ICON_SIZE));
                            }
                            None => {
                                ui.label(TYPE_NAMES.get(kind as usize).copied().unwrap_or("?"));
                            }
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Remove").clicked() {
                        action = Some(0);
                    }
                    if ui.small_button("Copy").clicked() {
                        action = Some(1);
                    }
                    if ui.small_button("▶").clicked() {
                        action = Some(2);
                    }
                    if ui.small_button("◀").clicked() {
                        action = Some(3);
                    }
                });
            });
            ui.add_space(6.0);
            ui.columns(2, |columns| {
                Self::details(&mut columns[0], &self.summaries, place, pokemon, more, summary, legal, moves_on, items_on);
                Self::stats(&mut columns[1], place, pokemon, more, summary, legal);
            });
            ui.add_space(6.0);
            if problems.is_empty() {
                ui.label(egui::RichText::new("✔ Legal in Gen 6").color(color::GREEN));
            } else {
                for problem in &problems {
                    ui.label(egui::RichText::new(format!("✖ {problem}")).color(color::ROSE));
                }
            }
        });
        match action {
            Some(0) if count > 1 => {
                team.pokemon.remove(place);
                extras.pokemon.remove(place);
            }
            Some(1) if count < data::MAX_TEAM => {
                let (one, more) = (team.pokemon[place], extras.pokemon[place].clone());
                team.pokemon.insert(place + 1, one);
                extras.pokemon.insert(place + 1, more);
                self.selected = place + 1;
            }
            Some(2) if place + 1 < count => {
                team.pokemon.swap(place, place + 1);
                extras.pokemon.swap(place, place + 1);
                self.selected = place + 1;
            }
            Some(3) if place > 0 => {
                team.pokemon.swap(place, place - 1);
                extras.pokemon.swap(place, place - 1);
                self.selected = place - 1;
            }
            _ => {}
        }
    }

    /// The left column: nickname, species, level, gender, item, ability, nature and moves.
    #[allow(clippy::too_many_arguments)]
    fn details(ui: &mut egui::Ui, summaries: &[Option<Summary>], place: usize, pokemon: &mut TrainerPokemon, more: &mut PokemonExtras, summary: Option<Summary>, legal: bool, moves_on: bool, items_on: bool) {
        let width = (ui.available_width() - 90.0).clamp(180.0, 320.0);
        let species = pokemon.species as usize;
        let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
            ui.horizontal(|ui| {
                ui.add_sized([78.0, 22.0], egui::Label::new(widget::hint(label)));
                add(ui);
            });
        };
        row(ui, "Nickname", &mut |ui| {
            let mut nickname = more.nickname.clone();
            if ui.add(egui::TextEdit::singleline(&mut nickname).desired_width(width).char_limit(data::NICKNAME_LETTERS).hint_text("Same as the species")).changed() {
                more.nickname = nickname;
            }
        });
        // One menu for the species and the form: each form is a Pokémon of its own ("Rotom-Heat").
        row(ui, "Species", &mut |ui| {
            let current = sango_species::form_key(pokemon.species as usize, pokemon.form as usize);
            let mut value = current;
            let shown = format!("#{:04} {}", pokemon.species, sango_species::species_form_name(pokemon.species as usize, pokemon.form as usize));
            if picker(ui, &format!("builder_species_{place}"), &shown, width, &mut value, None, || {
                let mut groups: Vec<Group> = Vec::new();
                for (tier, name) in gen6::TIER_NAMES.iter().enumerate() {
                    let entries: Vec<(u16, String)> = sango_species::species_form_menu((1..=SPECIES.len()).filter(|&s| tier_of(s) == tier));
                    let title = match *name {
                        "Uber" => "Uber (Ubers)".to_string(),
                        "NFE" => "NFE (not fully evolved)".to_string(),
                        "LC" => "LC (little cup)".to_string(),
                        "Other" => "Other".to_string(),
                        other => other.to_string(),
                    };
                    groups.push(Group { title, entries });
                }
                groups
            }) && value != current
            {
                let (species, form) = sango_species::from_form_key(value);
                if species != pokemon.species as usize {
                    // The first ability of the new species, then the moves stay: the list below tells which ones it cannot learn.
                    more.ability = summary_ability(&summaries.get(species).copied().flatten(), 0);
                    pokemon.ability = 1;
                }
                pokemon.species = species as u16;
                pokemon.form = form as _;
            }
        });
        row(ui, "Level", &mut |ui| {
            widget::spin(ui, &mut pokemon.level, 1.0, 100.0);
        });
        row(ui, "Gender", &mut |ui| match summary.map(|s| s.gender_ratio) {
            Some(0) => {
                ui.label("Male only");
                pokemon.gender = 0;
            }
            Some(254) => {
                ui.label("Female only");
                pokemon.gender = 0;
            }
            Some(255) => {
                ui.label("No gender");
                pokemon.gender = 0;
            }
            _ => {
                for (number, name) in ["Random", "Male", "Female"].iter().enumerate() {
                    ui.selectable_value(&mut pokemon.gender, number as u8, *name);
                }
                ui.checkbox(&mut more.shiny, "Shiny");
            }
        });
        if matches!(summary.map(|s| s.gender_ratio), Some(0 | 254 | 255)) {
            row(ui, "Shiny", &mut |ui| {
                ui.checkbox(&mut more.shiny, "Shiny");
            });
        }
        ui.add_space(4.0);
        if items_on {
            row(ui, "Item", &mut |ui| {
                let mut value = pokemon.item;
                let shown = if value == 0 { "(no item)".to_string() } else { entry(&ITEM_NAMES, value).1 };
                if picker(ui, &format!("builder_item_{place}"), &shown, width, &mut value, Some("(no item)"), || {
                    let own = used_by_sets(species, |set| vec![set.item]);
                    let mut groups = vec![Group { title: "Used with this Pokémon".to_string(), entries: own.iter().map(|&n| entry(&ITEM_NAMES, n)).collect() }];
                    let strategic: Vec<u16> = gen6::STRATEGIC_ITEMS.iter().copied().filter(|n| !own.contains(n)).collect();
                    groups.push(Group { title: "Strategic items".to_string(), entries: strategic.iter().map(|&n| entry(&ITEM_NAMES, n)).collect() });
                    groups.push(Group { title: "All the other items".to_string(), entries: (1..=ITEM_NAMES.len() as u16).filter(|n| !own.contains(n) && !strategic.contains(n) && !ITEM_NAMES[*n as usize - 1].is_empty()).map(|n| entry(&ITEM_NAMES, n)).collect() });
                    groups
                }) {
                    pokemon.item = value;
                }
            });
        }
        row(ui, "Ability", &mut |ui| {
            let mut value = if more.ability != 0 { more.ability } else { summary_ability(&summary, pokemon.ability.max(1) as usize - 1) };
            let shown = if value == 0 { "(random of the first two)".to_string() } else { entry(&ABILITY_NAMES, value).1 };
            let own: Vec<u16> = summary.map(|s| s.abilities.iter().map(|&a| u16::from(a)).collect()).unwrap_or_default();
            if picker(ui, &format!("builder_ability_{place}"), &shown, width, &mut value, None, || {
                let used = used_by_sets(species, |set| vec![set.ability]);
                let mut legal_list: Vec<u16> = used.iter().copied().filter(|a| own.contains(a)).collect();
                for &a in &own {
                    if a != 0 && !legal_list.contains(&a) {
                        legal_list.push(a);
                    }
                }
                let hidden = own.get(2).copied().unwrap_or(0);
                let name = |a: u16| {
                    let (number, text) = entry(&ABILITY_NAMES, a);
                    (number, if a == hidden && own.get(..2).is_some_and(|first| !first.contains(&a)) { format!("{text} (hidden)") } else { text })
                };
                let mut groups = vec![Group { title: "Abilities of the species".to_string(), entries: legal_list.iter().map(|&a| name(a)).collect() }];
                if !legal {
                    groups.push(Group { title: "Other abilities (not legal)".to_string(), entries: (1..=ABILITY_NAMES.len() as u16).filter(|a| !legal_list.contains(a) && !ABILITY_NAMES[*a as usize - 1].is_empty()).map(|a| entry(&ABILITY_NAMES, a)).collect() });
                }
                groups
            }) {
                more.ability = value;
                pokemon.ability = own.iter().position(|&a| a == value).map_or(0, |at| at as u8 + 1);
            }
        });
        row(ui, "Nature", &mut |ui| {
            let mut value = u16::from(more.nature);
            let shown = nature_text(more.nature);
            if picker(ui, &format!("builder_nature_{place}"), &shown, width, &mut value, None, || {
                vec![Group { title: "Natures".to_string(), entries: std::iter::once((255u16, "Of the game".to_string())).chain((0..25u8).map(|n| (u16::from(n), nature_text(n)))).collect() }]
            }) {
                more.nature = value as u8;
            }
        });
        if moves_on {
            ui.add_space(4.0);
            for slot in 0..4 {
                row(ui, if slot == 0 { "Moves" } else { "" }, &mut |ui| {
                    let mut value = pokemon.moves[slot];
                    let current = pokemon.moves;
                    let shown = if value == 0 { "(no move)".to_string() } else { entry(&MOVE_NAMES, value).1 };
                    let marked = if value != 0 && learns(species, value as usize) == Some(false) { format!("✖ {shown}") } else { shown };
                    if picker(ui, &format!("builder_move_{place}_{slot}"), &marked, width, &mut value, Some("(no move)"), || {
                        let used = used_by_sets(species, |set| set.moves.to_vec());
                        let can = |m: u16| learns(species, m as usize).unwrap_or(true);
                        let mut groups = vec![Group { title: "Used with this Pokémon".to_string(), entries: used.iter().filter(|&&m| can(m)).map(|&m| entry(&MOVE_NAMES, m)).collect() }];
                        groups.push(Group { title: "Learned by this Pokémon".to_string(), entries: (1..=MOVE_NAMES.len() as u16).filter(|m| can(*m) && !used.contains(m) && !current.contains(m)).map(|m| entry(&MOVE_NAMES, m)).collect() });
                        if !legal {
                            groups.push(Group { title: "Not learned in Gen 6 (not legal)".to_string(), entries: (1..=MOVE_NAMES.len() as u16).filter(|m| !can(*m) && !current.contains(m)).map(|m| entry(&MOVE_NAMES, m)).collect() });
                        }
                        groups
                    }) {
                        pokemon.moves[slot] = value;
                    }
                });
            }
        }
        let sets = sets_of(species);
        if !sets.is_empty() {
            ui.add_space(6.0);
            row(ui, "Popular set", &mut |ui| {
                let mut chosen = 0u16;
                if picker(ui, &format!("builder_set_{place}"), "Load a set of the players…", width, &mut chosen, None, || {
                    vec![Group { title: "Sets of Smogon".to_string(), entries: sets.iter().enumerate().map(|(at, set)| (at as u16 + 1, format!("{} — {}", set.name, entry(&ITEM_NAMES, set.item).1))).collect() }]
                }) && chosen > 0 {
                    let set = &sets[chosen as usize - 1];
                    pokemon.item = set.item;
                    pokemon.moves = set.moves;
                    more.ability = set.ability;
                    pokemon.ability = summary.and_then(|s| s.abilities.iter().position(|&a| u16::from(a) == set.ability)).map_or(0, |at| at as u8 + 1);
                    more.nature = set.nature;
                    more.evs = set.evs;
                    more.ivs = set.ivs;
                }
            });
            ui.label(widget::hint("A set sets the item, the ability, the nature, the moves, the EVs and the IVs. Turn on \"Choose the items\" and \"Choose the moves\" to use them."));
        }
    }

    /// The right column: the base stats, the EVs and the IVs, and the stats at the level of the Pokémon.
    fn stats(ui: &mut egui::Ui, place: usize, pokemon: &mut TrainerPokemon, more: &mut PokemonExtras, summary: Option<Summary>, legal: bool) {
        ui.label(egui::RichText::new("Stats").strong());
        let base = summary.map(|s| s.base).unwrap_or([0; 6]);
        let own_ivs = more.ivs[0] != 255;
        let team_iv = u32::from(pokemon.iv.min(31));
        egui::Grid::new(format!("builder_stats_{place}")).num_columns(6).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(widget::hint(""));
            ui.label(widget::hint("Base"));
            ui.label(widget::hint(""));
            ui.label(widget::hint("EVs"));
            ui.label(widget::hint("IVs"));
            ui.label(widget::hint("Stat"));
            ui.end_row();
            for stat in 0..6 {
                let change = nature_change(more.nature, stat);
                let name = match change {
                    1 => egui::RichText::new(format!("{} +", STATS[stat])).color(color::ROSE).strong(),
                    -1 => egui::RichText::new(format!("{} −", STATS[stat])).color(color::ACCENT).strong(),
                    _ => egui::RichText::new(STATS[stat]),
                };
                ui.label(name);
                ui.label(base[stat].to_string());
                let (bar, _) = ui.allocate_exact_size(egui::vec2(80.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(bar, 4.0, color::SURFACE_STRONG);
                let filled = egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * (f32::from(base[stat]) / 200.0).min(1.0), bar.height()));
                ui.painter().rect_filled(filled, 4.0, widget::stat_color(f32::from(base[stat])));
                let ev_max = if legal { EV_STAT_MAX as u8 } else { 255 };
                ui.horizontal(|ui| {
                    let mut ev = more.evs[stat];
                    ui.spacing_mut().slider_width = 70.0;
                    ui.add(egui::Slider::new(&mut ev, 0..=ev_max).show_value(false).trailing_fill(true));
                    widget::spin(ui, &mut ev, 0.0, f64::from(ev_max));
                    if ev != more.evs[stat] {
                        more.evs[stat] = ev;
                        if legal {
                            let total: u32 = more.evs.iter().map(|&v| u32::from(v)).sum();
                            if total > EV_TOTAL_MAX {
                                more.evs[stat] = (u32::from(ev) - (total - EV_TOTAL_MAX)) as u8;
                            }
                        }
                    }
                });
                let mut iv = if own_ivs { more.ivs[stat] } else { team_iv as u8 };
                if widget::spin(ui, &mut iv, 0.0, 31.0) {
                    if !own_ivs {
                        more.ivs = [team_iv as u8; 6];
                    }
                    more.ivs[stat] = iv;
                }
                let iv_now = if own_ivs { u32::from(more.ivs[stat]) } else { team_iv };
                ui.label(egui::RichText::new(final_stat(stat, u32::from(base[stat]), u32::from(pokemon.level), iv_now, u32::from(more.evs[stat]), more.nature).to_string()).strong());
                ui.end_row();
            }
        });
        let total: u32 = more.evs.iter().map(|&v| u32::from(v)).sum();
        let left = EV_TOTAL_MAX as i64 - i64::from(total);
        let good = left >= 0 && more.evs.iter().all(|&v| u32::from(v) <= EV_STAT_MAX);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("EVs: {total} of {EV_TOTAL_MAX} ({left} left)")).color(if good { color::GREEN } else { color::ROSE }));
            if ui.small_button("Clear the EVs").clicked() {
                more.evs = [0; 6];
            }
            if ui.small_button("IVs 31").clicked() {
                more.ivs = [31; 6];
            }
            if ui.small_button("IVs 0").clicked() {
                more.ivs = [0; 6];
            }
            if own_ivs && ui.small_button("IVs of the team").clicked() {
                more.ivs = [255; 6];
            }
        });
        ui.label(widget::hint(&format!("Base total {}. A Pokémon without IVs of its own has the IVs of the team ({}).", base.iter().map(|&v| u32::from(v)).sum::<u32>(), pokemon.iv)));
    }

    }

/// The ability at a place of a summary (0 when there is none).
fn summary_ability(summary: &Option<Summary>, at: usize) -> u16 {
    summary.and_then(|s| s.abilities.get(at).copied()).map_or(0, u16::from)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_stats_follow_the_formula_of_the_games() {
        // Garchomp, level 100, 252 Atk EVs, Jolly: Attack 359 with 31 IVs.
        assert_eq!(final_stat(1, 130, 100, 31, 252, 13), 359);
        // 108 base HP, 31 IVs, 0 EVs: 357 HP.
        assert_eq!(final_stat(0, 108, 100, 31, 0, 255), 357);
        assert_eq!(nature_change(3, 1), 1);
        assert_eq!(nature_change(3, 3), -1);
        assert_eq!(nature_change(0, 1), 0);
    }

    #[test]
    fn the_data_of_gen_6_knows_the_moves_and_the_tiers() {
        // Garchomp (445) learns Earthquake (89) and is OU in Gen 6; Pikachu (25) cannot learn Earthquake.
        assert_eq!(learns(445, 89), Some(true));
        assert_eq!(learns(25, 89), Some(false));
        assert_eq!(gen6::TIER_NAMES[tier_of(445)], "OU");
        assert_eq!(gen6::TIER_NAMES[tier_of(150)], "Uber");
        assert!(!sets_of(445).is_empty());
        assert!(!gen6::STRATEGIC_ITEMS.is_empty());
    }
}
