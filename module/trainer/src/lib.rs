//! The Trainer page: the data, the teams, the classes and the names of all the trainers of the game.
//!
//! The page changes the files of the game archives 36 (the data), 38 (the teams) and 37 (the classes) and the texts 21 (the names of the
//! classes) and 22 (the names of the trainers). It writes only what the user changed, in `trainer/` and `game_text/` of the SD card; the plugin
//! reads these files instead of the files of the game (restart the game after a change). See [`sango_core::trainer`] for the formats.

mod csv;
pub mod store;

use sango_core::text::LANGUAGES;
use sango_core::trainer::{self as data, BattleExtras, PokemonExtras, Team, TrainerClass, TrainerData};
use sango_core::SdCard;
use sango_species::searchable_picker;
use sango_ui::theme::color;
use sango_ui::retired::Retired;
use sango_ui::{egui, widget, Module, Session};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store::Store;

mod builder;
mod lists;
mod model_view;
// Generated data: some lists (the strategic items) are kept for the tests and the next uses.
#[allow(dead_code)]
mod gen6;
mod showdown;

const TABS: [&str; 4] = ["General", "Team", "Class", "Messages"];


/// The name of a line of a grid: blue when the value at `$path` of the open trainer is not the one of the game, with ↺ that puts the
/// value of the game back (see `widget::field_name`).
macro_rules! name {
    ($ui:expr, $label:expr, $open:expr, $game:expr, $($path:tt)+) => {{
        let game_value = $game.as_ref().map(|g: &Open| g.$($path)+.clone());
        let changed = game_value.as_ref().is_some_and(|v| *v != $open.$($path)+);
        let back = $ui.horizontal(|ui| widget::field_name(ui, $label, changed).1).inner;
        if let (true, Some(v)) = (back, game_value) {
            $open.$($path)+ = v;
        }
    }};
}

/// A line of the list of the trainers.
struct Row {
    id: usize,
    name: String,
    class: String,
}

/// A message of the open trainer: its kind, its line of the text file and its text in each language.
#[derive(Clone, PartialEq, Eq)]
struct Message {
    kind: usize,
    line: usize,
    texts: Vec<String>,
}

/// What the user edits for the open trainer.
struct Open {
    id: usize,
    trainer: TrainerData,
    team: Team,
    /// The start of the battle and what each Pokémon has more than the files of the game.
    extras: BattleExtras,
    class: TrainerClass,
    /// The name of the trainer and the name of its class in each language.
    names: Vec<String>,
    class_names: Vec<String>,
    messages: Vec<Message>,
}

/// The page.
pub struct Trainer {
    store: Option<Store>,
    root: Option<PathBuf>,
    error: Option<String>,
    rows: Vec<Row>,
    rows_dirty: bool,
    /// The names of the classes in English, for the menus (the class `n` is at the place `n - 1`).
    class_menu: Vec<String>,
    changed: HashSet<usize>,
    changed_classes: HashSet<usize>,
    trainer: usize,
    tab: usize,
    language: usize,
    open: Option<Open>,
    /// The trainer, the class and the names as they were loaded or saved: to know what changed.
    saved: Option<Open>,
    /// The open trainer as the game has it (the romfs), to show the fields that are not as in the game.
    game: Option<Open>,
    /// The class of the open trainer as the game has it: its number, its data and its names.
    game_class: Option<(usize, TrainerClass, Vec<String>)>,
    /// Undo and redo of the open trainer.
    history: sango_ui::history::History<Open>,
    status: Option<(bool, String)>,
    item_list: Arc<Mutex<sango_ui::item_menu::ItemList>>,
    /// The windows of the buttons Export, Import and Reset, and the parts of a reset: the data and the team, the class.
    export_open: bool,
    import_open: bool,
    reset_open: bool,
    reset_trainer: bool,
    reset_class: bool,
    /// The pictures of the classes, as textures.
    pictures: HashMap<usize, Option<egui::TextureHandle>>,
    retired: Retired,
    /// For each picture of the classes, the names of the classes that use it.
    picture_users: Vec<String>,
    /// The trainer that the user chose while the open one has changes that are not saved: the page asks what to do.
    pending: Option<usize>,
    model_view: model_view::ModelView,
    builder: builder::Builder,
}

impl Default for Trainer {
    fn default() -> Self {
        Trainer {
            store: None,
            root: None,
            error: None,
            rows: Vec::new(),
            rows_dirty: true,
            class_menu: Vec::new(),
            changed: HashSet::new(),
            changed_classes: HashSet::new(),
            trainer: 1,
            tab: 0,
            language: 2,
            open: None,
            saved: None,
            game: None,
            game_class: None,
            history: Default::default(),
            status: None,
            item_list: Default::default(),
            export_open: false,
            import_open: false,
            reset_open: false,
            reset_trainer: true,
            reset_class: false,
            pictures: HashMap::new(),
            retired: Retired::default(),
            picture_users: Vec::new(),
            pending: None,
            model_view: model_view::ModelView::default(),
            builder: builder::Builder::default(),
        }
    }
}

impl Clone for Open {
    fn clone(&self) -> Open {
        Open { id: self.id, trainer: self.trainer, team: self.team.clone(), extras: self.extras.clone(), class: self.class, names: self.names.clone(), class_names: self.class_names.clone(), messages: self.messages.clone() }
    }
}

impl PartialEq for Open {
    fn eq(&self, other: &Open) -> bool {
        self.id == other.id && self.same(other)
    }
}

impl Open {
    fn same(&self, other: &Open) -> bool {
        self.trainer == other.trainer && self.team == other.team && self.extras == other.extras && self.class == other.class && self.names == other.names && self.class_names == other.class_names && self.messages == other.messages
    }
}

/// The AI as a list of switches (only the bits that have a script in the game; the other bits stay as they are).
fn ai_chips(ui: &mut egui::Ui, ai: &mut u32) {
    ui.horizontal_wrapped(|ui| {
        for (bit, name, help) in data::AI_BITS {
            let mut on = *ai & (1 << bit) != 0;
            widget::chip(ui, name, &mut on).on_hover_text(help);
            if on {
                *ai |= 1 << bit;
            } else {
                *ai &= !(1 << bit);
            }
        }
    });
}

/// A number box with the limits of a value.
fn spin<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, min: f64, max: f64) {
    widget::spin(ui, value, min, max);
}

/// A menu of a short list of choices.
fn choice(ui: &mut egui::Ui, id: impl std::hash::Hash, value: &mut u8, list: &[&str], width: f32) {
    let shown = list.get(*value as usize).copied().unwrap_or("?");
    let combo = egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).show_ui(ui, |ui| {
        for (number, name) in list.iter().enumerate() {
            ui.selectable_value(value, number as u8, *name);
        }
    });
    if let Some(to) = sango_ui::focus::nav(ui, &combo.response, *value as usize, list.len(), false) {
        *value = to as u8;
    }
}

/// A menu of named things for a number of the data: the user sees names, never the numbers. A number that is not in the list shows as
/// "Unknown" (it stays as it is until the user chooses another thing).
fn named_menu(ui: &mut egui::Ui, id: &str, value: &mut u32, entries: &[(u32, String)], width: f32) {
    let shown = entries.iter().find(|(number, _)| number == value).map_or_else(|| format!("Unknown ({value})"), |(_, name)| name.clone());
    let mut small = (*value).min(u32::from(u16::MAX)) as u16;
    if sango_ui::picker::picker(ui, id, &shown, width, &mut small, None, || {
        vec![sango_ui::picker::Group { title: String::new(), entries: entries.iter().filter(|(number, _)| *number <= u32::from(u16::MAX)).map(|(number, name)| (*number as u16, name.clone())).collect() }]
    })
    {
        *value = u32::from(small);
    }
}

/// The menu of the languages.
fn language_menu(ui: &mut egui::Ui, id: &str, language: &mut usize, width: f32) {
    sango_ui::language::menu(ui, id, language, Some(width));
}

impl Trainer {
    /// Opens the archives of the dump and reads the names for the list.
    fn open_store(&mut self, session: &Session) {
        self.error = None;
        self.store = None;
        let Some(dump) = session.dump.as_ref() else { return };
        match Store::open(dump) {
            Ok(store) => self.store = Some(store),
            Err(message) => self.error = Some(message),
        }
        if let Ok(mut list) = self.item_list.lock() {
            list.open(dump);
        }
        self.builder.open(session);
        self.rows_dirty = true;
        self.open = None;
        self.saved = None;
    }

    /// Reads the names of the trainers and of the classes (the English ones) and what the SD card changes.
    fn build_rows(&mut self, sd: Option<&SdCard>) {
        let Some(store) = self.store.as_mut() else { return };
        const ENGLISH: usize = 2;
        let class_names: Vec<String> = (0..data::CLASS_COUNT).map(|class| store.line(sd, ENGLISH, data::CLASS_NAMES, class)).collect();
        self.class_menu = class_names.iter().skip(1).cloned().collect();
        self.rows.clear();
        for id in 1..data::TRAINER_COUNT {
            let name = store.line(sd, ENGLISH, data::TRAINER_NAMES, id);
            let class = store.trainer(sd, id).ok().and_then(|(trainer, _)| class_names.get(trainer.class as usize).cloned()).unwrap_or_default();
            self.rows.push(Row { id, name, class });
        }
        // The pictures of the classes, for the menu of the pictures: the classes that use each one.
        let mut users: Vec<Vec<String>> = vec![Vec::new(); data::PICTURE_COUNT / 3];
        for class in 1..data::CLASS_COUNT {
            if let (Ok(record), Some(name)) = (store.class(sd, class), class_names.get(class)) {
                if let Some(list) = users.get_mut(record.picture_2d as usize) {
                    if !name.is_empty() && !list.contains(name) {
                        list.push(name.clone());
                    }
                }
            }
        }
        self.picture_users = users.into_iter().map(|names| names.join(", ")).collect();
        self.changed = Store::changed_trainers(sd);
        self.changed_classes = Store::changed_classes(sd);
        self.rows_dirty = false;
    }

    /// Reads the trainer that the user chose: its data, its team, its class and its names.
    fn load(&mut self, sd: Option<&SdCard>) {
        let id = self.trainer;
        let Some(store) = self.store.as_mut() else { return };
        let loaded = store.trainer(sd, id).and_then(|(trainer, team)| {
            let class = store.class(sd, trainer.class as usize)?;
            Ok((trainer, team, class))
        });
        match loaded {
            Ok((trainer, team, class)) => {
                let names = (0..LANGUAGES.len()).map(|language| store.line(sd, language, data::TRAINER_NAMES, id)).collect();
                let class_names = (0..LANGUAGES.len()).map(|language| store.line(sd, language, data::CLASS_NAMES, trainer.class as usize)).collect();
                let extras = store.extras(sd, id, team.pokemon.len());
                let messages = store
                    .messages(id)
                    .into_iter()
                    .map(|(kind, line)| Message { kind, line, texts: (0..LANGUAGES.len()).map(|language| store.line(sd, language, data::MESSAGE_TEXT, line)).collect() })
                    .collect();
                let open = Open { id, trainer, team, extras, class, names, class_names, messages };
                // The same trainer as the game has it.
                self.game = store.trainer(None, id).ok().map(|(trainer, team)| {
                    let names = (0..LANGUAGES.len()).map(|language| store.game_line(language, data::TRAINER_NAMES, id)).collect();
                    let extras = store.extras(None, id, team.pokemon.len());
                    let messages = open.messages.iter().map(|m| Message { texts: (0..LANGUAGES.len()).map(|language| store.game_line(language, data::MESSAGE_TEXT, m.line)).collect(), ..m.clone() }).collect();
                    Open { id, trainer, team, extras, class: open.class, names, class_names: open.class_names.clone(), messages }
                });
                self.game_class = None;
                self.saved = Some(open.clone());
                self.open = Some(open);
                self.status = None;
            }
            Err(message) => {
                self.open = None;
                self.saved = None;
                self.status = Some((false, message));
            }
        }
    }

    /// The open trainer as the game has it, with the class of the game for the class that is chosen now.
    fn game_view(&mut self) -> Option<Open> {
        let class = self.open.as_ref()?.trainer.class as usize;
        if self.game_class.as_ref().map(|g| g.0) != Some(class) {
            let store = self.store.as_mut()?;
            self.game_class = store.class(None, class).ok().map(|data| (class, data, (0..LANGUAGES.len()).map(|language| store.game_line(language, data::CLASS_NAMES, class)).collect()));
        }
        let mut game = self.game.clone()?;
        if let Some((_, data, names)) = &self.game_class {
            game.class = *data;
            game.class_names = names.clone();
        } else {
            // A class that the game does not have: nothing to compare.
            game.class = self.open.as_ref()?.class;
            game.class_names = self.open.as_ref()?.class_names.clone();
        }
        Some(game)
    }

    /// True when the open trainer has something that was not saved.
    fn dirty(&self) -> bool {
        match (&self.open, &self.saved) {
            (Some(open), Some(saved)) => !open.same(saved),
            _ => false,
        }
    }

    /// The window that asks what to do with the changes of the open trainer when the user chooses another trainer.
    fn pending_dialog(&mut self, ctx: &egui::Context, session: &Session) {
        let Some(next) = self.pending else { return };
        let current = self.open.as_ref().map_or(0, |open| open.id);
        let mut answer = None;
        egui::Window::new("Changes not saved").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(format!("The trainer #{current:03} has changes that are not saved."));
            ui.label(widget::hint(&format!("What do you want to do before you open the trainer #{next:03}?")));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if session.sd.is_some() && widget::button_styled(ui, egui::RichText::new("Save and open").color(color::WHITE), Some(color::ACCENT)) {
                    answer = Some(0);
                }
                if widget::button(ui, "Discard the changes") {
                    answer = Some(1);
                }
                if widget::button(ui, "Stay here") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(2);
                }
            });
        });
        match answer {
            Some(0) => {
                if let Some(sd) = session.sd.as_ref() {
                    self.save(sd);
                }
                self.trainer = next;
                self.pending = None;
            }
            Some(1) => {
                self.trainer = next;
                self.saved = self.open.clone();
                self.pending = None;
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }

    /// Writes what changed on the SD card.
    fn save(&mut self, sd: &SdCard) {
        if let Some(open) = self.open.as_mut() {
            open.extras.pokemon.resize(open.team.pokemon.len(), PokemonExtras::none());
        }
        let (Some(open), Some(saved), Some(store)) = (self.open.as_ref(), self.saved.as_ref(), self.store.as_mut()) else { return };
        let result = (|| -> Result<(), String> {
            if open.trainer != saved.trainer || open.team != saved.team {
                store.save_trainer(sd, open.id, &open.trainer, &open.team)?;
            }
            if open.extras != saved.extras || open.team.pokemon.len() != saved.team.pokemon.len() {
                store.save_extras(sd, open.id, &open.extras)?;
            }
            for (message, before) in open.messages.iter().zip(saved.messages.iter()) {
                for language in 0..LANGUAGES.len() {
                    if message.texts[language] != before.texts[language] {
                        store.set_line(sd, language, data::MESSAGE_TEXT, message.line, &message.texts[language])?;
                    }
                }
            }
            if open.class != saved.class {
                store.save_class(sd, open.trainer.class as usize, &open.class)?;
            }
            for language in 0..LANGUAGES.len() {
                if open.names[language] != saved.names[language] {
                    store.set_line(sd, language, data::TRAINER_NAMES, open.id, &open.names[language])?;
                }
                if open.class_names[language] != saved.class_names[language] && open.trainer.class == saved.trainer.class {
                    store.set_line(sd, language, data::CLASS_NAMES, open.trainer.class as usize, &open.class_names[language])?;
                }
            }
            sd.mark_changed("trainer/data/index.bin")
        })();
        self.status = Some(match result {
            Ok(()) => {
                self.saved = self.open.clone();
                self.rows_dirty = true;
                (true, "Saved. The files are on the SD card: restart the game to see the change.".to_string())
            }
            Err(message) => (false, message),
        });
    }

    /// Gives back the data of the game for the open trainer: its data and its team, and its class, as the window of the reset chose.
    fn reset(&mut self, sd: &SdCard) {
        let (id, class) = match self.open.as_ref() {
            Some(open) => (open.id, open.trainer.class as usize),
            None => return,
        };
        let Some(store) = self.store.as_mut() else { return };
        let result = (|| -> Result<(), String> {
            if self.reset_trainer {
                store.reset_trainer(sd, id)?;
            }
            if self.reset_class {
                store.reset_class(sd, class)?;
            }
            Ok(())
        })();
        if let Err(message) = result {
            self.status = Some((false, message));
            return;
        }
        store.forget_texts();
        self.rows_dirty = true;
        self.load(Some(sd));
        self.status = Some((true, "The data of the game is back. The names stay as they are.".to_string()));
    }

    /// Makes a new trainer in a place that the game does not use (a trainer without name), with the data and the team that are open.
    fn new_trainer(&mut self, sd: &SdCard) {
        let Some(open) = self.open.clone() else { return };
        let free = self.rows.iter().find(|row| row.name.is_empty() && !self.changed.contains(&row.id)).map(|row| row.id);
        let Some(id) = free else {
            self.status = Some((false, "All the places of the game have a trainer: the game has 949 and the new ones take the places without name.".into()));
            return;
        };
        let Some(store) = self.store.as_mut() else { return };
        let mut trainer = open.trainer;
        trainer.pokemon_count = open.team.pokemon.len() as u8;
        let result = store.save_trainer(sd, id, &trainer, &open.team).and_then(|_| store.save_extras(sd, id, &open.extras)).and_then(|_| store.set_line(sd, 2, data::TRAINER_NAMES, id, "New trainer"));
        match result {
            Ok(()) => {
                for language in 0..LANGUAGES.len() {
                    if language != 2 {
                        let _ = store.set_line(sd, language, data::TRAINER_NAMES, id, "New trainer");
                    }
                }
                self.trainer = id;
                self.rows_dirty = true;
                self.open = None;
                self.status = Some((true, format!("The trainer {id} starts like the one that was open. Change its name and its team, then save.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// The menu of the trainers: the name, the class and the number, with a search box (as the menu of the species).
    fn trainer_picker(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.rows.iter().map(|row| format!("{}{}  ·  {}", if self.changed.contains(&row.id) { "● " } else { "" }, if row.name.is_empty() { "(no name)" } else { row.name.as_str() }, row.class)).collect();
        let names: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut id = self.trainer;
        searchable_picker(ui, "trainer_choice", &mut id, &names, 420.0);
        if id != self.trainer && id >= 1 && id <= names.len() {
            self.trainer = id;
        }
    }

    /// The General tab.
    fn general_tab(&mut self, ui: &mut egui::Ui) {
        let game = self.game_view();
        let (Some(open), Some(sd_item_list)) = (self.open.as_mut(), Some(self.item_list.clone())) else { return };
        let language = self.language;
        widget::card(ui, |ui| {
            ui.label(widget::title("The trainer"));
            ui.add_space(4.0);
            egui::Grid::new("trainer_general").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                name!(ui, "Name", open, game, names[language]);
                ui.horizontal(|ui| {
                    language_menu(ui, "trainer_language", &mut self.language, 150.0);
                    ui.add(egui::TextEdit::singleline(&mut open.names[language]).desired_width(240.0));
                });
                ui.end_row();

                name!(ui, "Class", open, game, trainer.class);
                ui.horizontal(|ui| {
                    let names: Vec<&str> = self.class_menu.iter().map(String::as_str).collect();
                    let mut class = open.trainer.class;
                    searchable_picker(ui, "trainer_class", &mut class, &names, 260.0);
                    open.trainer.class = class;
                });
                ui.end_row();

                name!(ui, "Kind of battle", open, game, trainer.fight_type);
                choice(ui, "trainer_fight", &mut open.trainer.fight_type, &data::FIGHT_TYPES, 160.0);
                ui.end_row();

                name!(ui, "Prize money", open, game, trainer.money);
                ui.horizontal(|ui| {
                    spin(ui, &mut open.trainer.money, 0.0, 255.0);
                    ui.label(widget::hint("The game multiplies it by the level of the last Pokémon."));
                });
                ui.end_row();

                name!(ui, "Heal after the battle", open, game, trainer.heal_after);
                ui.checkbox(&mut open.trainer.heal_after, "The Pokémon of the player get their HP back");
                ui.end_row();

                name!(ui, "Item given after the battle", open, game, trainer.gift_item);
                let mut gift = open.trainer.gift_item as usize;
                if sango_ui::item_menu::shared_menu(ui, "trainer_gift", &mut gift, 240.0, &sd_item_list, true) {
                    open.trainer.gift_item = gift as u16;
                }
                ui.end_row();
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Start of the battle"));
            ui.label(widget::hint("What is on the field when the battle with this trainer starts. The plugin adds it: restart the game after a change."));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                name!(ui, "Weather", open, game, extras.weather);
                let extras = &mut open.extras;
                let shown = data::WEATHERS.iter().find(|(value, _)| *value == extras.weather).map_or("?", |(_, name)| *name);
                let combo = egui::ComboBox::from_id_salt("trainer_weather").selected_text(shown).width(190.0).show_ui(ui, |ui| {
                    for (value, name) in data::WEATHERS {
                        ui.selectable_value(&mut extras.weather, value, name);
                    }
                });
                let at = data::WEATHERS.iter().position(|(value, _)| *value == extras.weather).unwrap_or(0);
                if let Some(to) = sango_ui::focus::nav(ui, &combo.response, at, data::WEATHERS.len(), false) {
                    extras.weather = data::WEATHERS[to].0;
                }
                name!(ui, "Terrain", open, game, extras.terrain);
                choice(ui, "trainer_terrain", &mut open.extras.terrain, &data::TERRAINS, 160.0);
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (bit, (name, help)) in data::FIELD_EFFECTS.iter().enumerate() {
                    let extras = &mut open.extras;
                    let mut on = extras.field & (1 << bit) != 0;
                    widget::chip(ui, name, &mut on).on_hover_text(*help);
                    if on {
                        extras.field |= 1 << bit;
                    } else {
                        extras.field &= !(1 << bit);
                    }
                }
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("In the battle"));
            ui.label(widget::hint("How the trainer chooses its moves: each move starts with 100 points, each way of thinking that is on adds or removes points, and the trainer plays one of the moves with the most points. Several can be on. Point at a name to see what it does. Then the items that the trainer uses."));
            ui.add_space(4.0);
            ai_chips(ui, &mut open.trainer.ai);
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (place, item) in open.trainer.items.iter_mut().enumerate() {
                    let mut value = *item as usize;
                    if sango_ui::item_menu::shared_menu(ui, &format!("trainer_item_{place}"), &mut value, 200.0, &sd_item_list, true) {
                        *item = value as u16;
                    }
                }
            });
        });
    }

    /// The Team tab.
    fn team_tab(&mut self, ui: &mut egui::Ui) {
        let Some(open) = self.open.as_mut() else { return };
        self.builder.ui(ui, &mut open.team, &mut open.extras);
    }

    /// The Class tab: the class of the trainer (it changes for all the trainers of this class).
    fn class_tab(&mut self, ui: &mut egui::Ui, users: usize, session: &Session) {
        let model = self.open.as_ref().map_or(model_view::NO_MODEL, |open| open.class.model_3d);
        if model == model_view::NO_MODEL {
            self.pictures_card(ui, session);
        } else {
            // A class with a 3D model has no 2D picture: the battle shows the model.
            let sd = session.sd.as_ref();
            let mut action = None;
            widget::card(ui, |ui| {
                ui.label(widget::title("3D model of the class"));
                ui.label(widget::hint(&format!("The battle shows the 3D model {model} (archive 133) for this class, not a 2D picture.")));
                ui.add_space(4.0);
                let store = self.store.as_ref();
                action = self.model_view.ui(ui, model, Store::model_changed(sd, model), sd.is_some(), || store.and_then(|store| store.model_file(sd, model)));
            });
            if let (Some(action), Some(store), Some(sd)) = (action, self.store.as_ref(), sd) {
                let result = match action {
                    model_view::Action::Import(file, scale) => store.import_model(sd, model, &file, scale).map(|()| "Imported. Restart the game to see it.".to_string()),
                    model_view::Action::Reset => store.reset_model(sd, model).map(|()| "The model of the game is back. Restart the game to see it.".to_string()),
                };
                self.model_view.reload();
                match result {
                    Ok(message) => self.model_view.say(true, message),
                    Err(message) => self.model_view.say(false, message),
                }
            }
        }
        ui.add_space(8.0);
        self.class_data_card(ui, users);
    }

    /// The data of the class.
    fn class_data_card(&mut self, ui: &mut egui::Ui, users: usize) {
        let game = self.game_view();
        let Some(open) = self.open.as_mut() else { return };
        let language = self.language;
        widget::card(ui, |ui| {
            ui.label(widget::title(&format!("Class {}", open.trainer.class)));
            ui.label(widget::hint(&format!("{users} trainer(s) have this class: a change here changes all of them.")));
            ui.add_space(6.0);
            egui::Grid::new("trainer_class_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                name!(ui, "Name", open, game, class_names[language]);
                ui.horizontal(|ui| {
                    language_menu(ui, "class_language", &mut self.language, 150.0);
                    ui.add(egui::TextEdit::singleline(&mut open.class_names[language]).desired_width(240.0));
                });
                ui.end_row();
                name!(ui, "Gender", open, game, class.gender);
                choice(ui, "class_gender", &mut open.class.gender, &["Male", "Female", "None"], 140.0);
                ui.end_row();
                name!(ui, "Group", open, game, class.group);
                choice(ui, "class_group", &mut open.class.group, &data::GROUPS, 160.0);
                ui.end_row();
                name!(ui, "Several trainers", open, game, class.plural);
                let mut plural = open.class.plural != 0;
                ui.checkbox(&mut plural, "The class is a group (Twins, a team...): the texts use the plural");
                open.class.plural = u8::from(plural);
                ui.end_row();
                name!(ui, "Start of the battle", open, game, class.battle_effect);
                let mut effect = u32::from(open.class.battle_effect);
                let effects: Vec<(u32, String)> = lists::BATTLE_EFFECTS.iter().map(|&(value, name)| (u32::from(value), name.to_string())).collect();
                named_menu(ui, "class_effect", &mut effect, &effects, 260.0);
                open.class.battle_effect = effect as u8;
                ui.end_row();
                name!(ui, "3D model of the battle", open, game, class.model_3d);
                let mut model = u32::from(open.class.model_3d);
                let models: Vec<(u32, String)> = lists::MODELS_3D.iter().map(|&(value, name)| (u32::from(value), name.to_string())).chain([(255, "None (a 2D picture)".to_string())]).collect();
                named_menu(ui, "class_model", &mut model, &models, 260.0);
                open.class.model_3d = model as u8;
                ui.end_row();
                name!(ui, "Sequence of the 3D model", open, game, class.sequence);
                choice(ui, "class_sequence", &mut open.class.sequence, &["The usual one", "The added one"], 200.0);
                ui.end_row();
                name!(ui, "2D picture", open, game, class.picture_2d);
                let pictures: Vec<(u32, String)> = self.picture_users.iter().enumerate().map(|(at, users)| (at as u32, if users.is_empty() { format!("Picture {at} (no class)") } else { format!("{users} (picture {at})") })).collect();
                named_menu(ui, "class_picture", &mut open.class.picture_2d, &pictures, 330.0);
                ui.end_row();
                name!(ui, "Character on the map", open, game, class.overworld_model);
                let characters: Vec<(u32, String)> = sango_overworld::names::MODEL_NAMES.iter().enumerate().map(|(at, name)| (at as u32, name.to_string())).collect();
                named_menu(ui, "class_character", &mut open.class.overworld_model, &characters, 260.0);
                ui.end_row();
                name!(ui, "Music of the sight", open, game, class.eye_music);
                let music: Vec<(u32, String)> = lists::MUSIC.iter().map(|&(value, name)| (u32::from(value), name.to_string())).collect();
                // The high half of the number is a flag of the game (1): it stays as it is.
                let mut track = open.class.eye_music & 0xFFFF;
                named_menu(ui, "class_music", &mut track, &music, 260.0);
                open.class.eye_music = (open.class.eye_music & 0xFFFF_0000) | track;
                ui.end_row();
            });
        });
    }

    /// The Messages tab: what the trainer says. A trainer has only some of the kinds of messages, and the list of the game cannot get
    /// more here.
    fn messages_tab(&mut self, ui: &mut egui::Ui) {
        let game = self.game_view();
        let Some(open) = self.open.as_mut() else { return };
        let language = self.language;
        widget::card(ui, |ui| {
            ui.label(widget::title("Messages"));
            ui.label(widget::hint("What the trainer says: before the battle, when it loses, after the battle... Only the messages that the game gives to this trainer are here. A message can be shared with another trainer: it changes for both."));
            ui.add_space(4.0);
            language_menu(ui, "trainer_messages_language", &mut self.language, 180.0);
            ui.add_space(6.0);
            if open.messages.is_empty() {
                ui.label(widget::hint("This trainer has no message."));
            }
            for (at, message) in open.messages.iter_mut().enumerate() {
                let title = data::MESSAGE_KINDS.get(message.kind).copied().unwrap_or("Another message");
                let game_text = game.as_ref().and_then(|g| g.messages.get(at)).map(|m| m.texts[language].clone());
                let changed = game_text.as_ref().is_some_and(|t| *t != message.texts[language]);
                let back = ui.horizontal(|ui| widget::field_name(ui, title, changed).1).inner;
                if let (true, Some(text)) = (back, game_text) {
                    message.texts[language] = text;
                }
                ui.add(egui::TextEdit::multiline(&mut message.texts[language]).desired_rows(3).desired_width(ui.available_width().min(620.0)));
                ui.add_space(6.0);
            }
        });
    }

    /// The pictures of the class: the whole body and the upper bodies. Gives true when a picture changed.
    fn pictures_card(&mut self, ui: &mut egui::Ui, session: &Session) {
        let (Some(open), Some(store)) = (self.open.as_ref(), self.store.as_ref()) else { return };
        let class = open.class;
        let sd = session.sd.as_ref();
        let mut action: Option<(usize, u8)> = None;
        widget::card(ui, |ui| {
            ui.label(widget::title("2D pictures of the class"));
            ui.label(widget::hint("The pictures of the trainer in the battle (the whole body and two upper bodies). Export a picture as a PNG, change it, and import it: it keeps the size of the picture of the game."));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for kind in 0..data::PICTURE_KINDS.len() {
                    let Some(file) = data::picture_file(&class, kind) else { continue };
                    ui.vertical(|ui| {
                        ui.label(widget::hint(data::PICTURE_KINDS[kind]));
                        let texture = self.pictures.entry(file).or_insert_with(|| {
                            let (width, height, rgba) = store.picture(sd, file).ok()?;
                            Some(sango_ui::textures::load(ui.ctx(), format!("trainer_picture_{file}"), egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba), egui::TextureOptions::LINEAR))
                        });
                        if let Some(texture) = texture {
                            let size = texture.size_vec2();
                            let scale = (200.0 / size.x).min(150.0 / size.y);
                            ui.add(egui::Image::new(&*texture).fit_to_exact_size(size * scale));
                        } else {
                            ui.label(widget::hint("No picture."));
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button("Export").clicked() {
                                action = Some((file, 0));
                            }
                            if sd.is_some() && ui.small_button("Import").clicked() {
                                action = Some((file, 1));
                            }
                            if Store::picture_changed(sd, file) && ui.small_button("Reset").clicked() {
                                action = Some((file, 2));
                            }
                        });
                    });
                }
            });
        });
        if let Some((file, what)) = action {
            self.picture_action(file, what, session);
        }
    }

    /// Exports, imports or resets a picture.
    fn picture_action(&mut self, file: usize, what: u8, session: &Session) {
        let Some(store) = self.store.as_ref() else { return };
        let result = (|| -> Result<String, String> {
            match what {
                0 => {
                    let (width, height, rgba) = store.picture(session.sd.as_ref(), file)?;
                    let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).set_file_name(format!("trainer_picture_{file}.png")).save_file() else { return Ok("Nothing was written.".into()) };
                    image::save_buffer(&path, &rgba, width as u32, height as u32, image::ColorType::Rgba8).map_err(|e| format!("{}: {e}", path.display()))?;
                    Ok(format!("Exported {}.", path.display()))
                }
                1 => {
                    let sd = session.sd.as_ref().ok_or("The SD card is not chosen.")?;
                    let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() else { return Ok("Nothing was changed.".into()) };
                    let (width, height) = store.game_picture_size(file).ok_or("The picture of the game is not there.")?;
                    let mut picture = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
                    if picture.width() as usize != width || picture.height() as usize != height {
                        picture = image::imageops::resize(&picture, width as u32, height as u32, image::imageops::FilterType::Lanczos3);
                    }
                    store.save_picture(sd, file, width, height, picture.as_raw())?;
                    Ok(format!("Imported {}. Restart the game to see it.", path.display()))
                }
                _ => {
                    let sd = session.sd.as_ref().ok_or("The SD card is not chosen.")?;
                    store.reset_picture(sd, file)?;
                    Ok("The picture of the game is back.".into())
                }
            }
        })();
        if let Some(old) = self.pictures.remove(&file).flatten() {
            self.retired.keep(old);
        }
        self.status = Some(match result {
            Ok(message) => (true, message),
            Err(message) => (false, message),
        });
    }

    /// The top bar: the menu of the trainers, then the buttons of every page (see `widget::action_bar`) and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let dirty = self.dirty();
        let has_card = session.sd.is_some();
        let mut clicks = widget::ActionClicks::default();
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                self.trainer_picker(ui);
                clicks = widget::action_bar(
                    ui,
                    widget::ActionBar { new: Some(("+ New trainer", has_card && self.open.is_some())), export: Some(true), import: Some(has_card), reset: Some(has_card && self.open.is_some()), save: Some(has_card && dirty), ..Default::default() },
                );
                self.history.buttons(ui, self.open.is_some());
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.changed.contains(&self.trainer) {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if !has_card {
                ui.label(widget::hint("Choose the SD card on the Home page to save."));
            }
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
        let Some(sd) = session.sd.as_ref() else { return };
        if clicks.new {
            self.new_trainer(sd);
        }
        if clicks.save {
            self.save(sd);
        }
        self.export_open |= clicks.export;
        self.import_open |= clicks.import;
        self.reset_open |= clicks.reset;
    }

    /// The windows of the buttons Export, Import and Reset.
    fn windows(&mut self, ctx: &egui::Context, session: &Session) {
        let Some(sd) = session.sd.clone() else {
            self.export_open = false;
            self.import_open = false;
            self.reset_open = false;
            return;
        };
        for import in [false, true] {
            let shown = if import { self.import_open } else { self.export_open };
            if !shown {
                continue;
            }
            let (mut open, mut run, mut close) = (true, false, false);
            egui::Window::new(if import { "Import CSV files" } else { "Export CSV files" }).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(widget::hint("For all the trainers, not only the one that is open."));
                ui.label(format!("{} (the data and the English name of each trainer) and {} (one line for each Pokémon).", csv::TRAINERS_FILE, csv::TEAMS_FILE));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    run = widget::button_styled(ui, egui::RichText::new(if import { "Import" } else { "Export" }).color(color::WHITE), Some(color::ACCENT));
                    close = widget::button(ui, "Cancel");
                });
            });
            if run {
                if let Some(store) = self.store.as_mut() {
                    if import {
                        match csv::import_files(store, &sd) {
                            Ok(Some(count)) => {
                                self.rows_dirty = true;
                                self.open = None;
                                self.status = Some((true, format!("Imported {count} trainers. Restart the game to see the change.")));
                            }
                            Ok(None) => {}
                            Err(message) => self.status = Some((false, message)),
                        }
                    } else {
                        self.status = match csv::export_files(store, &sd) {
                            Ok(Some(path)) => Some((true, format!("Wrote {} and {} in {path}.", csv::TRAINERS_FILE, csv::TEAMS_FILE))),
                            Ok(None) => None,
                            Err(message) => Some((false, message)),
                        };
                    }
                }
            }
            if run || close || !open {
                if import {
                    self.import_open = false;
                } else {
                    self.export_open = false;
                }
            }
        }
        if self.reset_open {
            let (mut open, mut run, mut close) = (true, false, false);
            let name = self.open.as_ref().map(|o| o.names[2].clone()).unwrap_or_default();
            egui::Window::new("Reset to the romfs").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(egui::RichText::new(format!("#{:03} {name}", self.trainer)).strong());
                ui.checkbox(&mut self.reset_trainer, "The data and the team of the trainer");
                ui.checkbox(&mut self.reset_class, "Its class (for every trainer of this class)");
                ui.label(widget::hint("The names stay as they are. The changes that are not saved are lost."));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(self.reset_trainer || self.reset_class, |ui| {
                        run = widget::button_styled(ui, egui::RichText::new("Reset").color(color::WHITE), Some(color::ROSE));
                    });
                    close = widget::button(ui, "Cancel");
                });
            });
            if run {
                self.reset(&sd);
            }
            if run || close || !open {
                self.reset_open = false;
            }
        }
    }
}

impl Module for Trainer {
    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            self.save(sd);
        }
    }

    fn discard_changes(&mut self) -> bool {
        self.open = self.saved.clone();
        true
    }

    fn unsaved(&self) -> bool {
        self.dirty()
    }

    fn title(&self) -> &str {
        "Trainer"
    }

    fn icon(&self) -> &str {
        "🎓"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        self.retired.tick(ui.ctx());
        ui.add_space(8.0);
        ui.heading("Trainer");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let root = dump.root().to_path_buf();
        if self.root.as_deref() != Some(root.as_path()) {
            self.root = Some(root);
            self.open_store(session);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        if self.store.is_none() {
            return;
        }
        if self.rows_dirty {
            self.build_rows(session.sd.as_ref());
        }
        if self.open.as_ref().map(|open| open.id) != Some(self.trainer) {
            match self.open.as_ref().map(|open| open.id).filter(|_| self.dirty()) {
                // The open trainer has changes that are not saved: stay on it and ask.
                Some(current) => {
                    self.pending = Some(self.trainer);
                    self.trainer = current;
                }
                None => self.load(session.sd.as_ref()),
            }
        }
        self.pending_dialog(ui.ctx(), session);
        let users = self.open.as_ref().map_or(0, |open| self.rows.iter().filter(|row| row.class == self.class_menu.get((open.trainer.class as usize).wrapping_sub(1)).cloned().unwrap_or_default()).count());
        {
            ui.vertical(|ui| {
                self.top_bar(ui, session);
                self.windows(ui.ctx(), session);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (place, name) in TABS.iter().enumerate() {
                        if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                            self.tab = place;
                        }
                    }
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical().id_salt("trainer_tab").auto_shrink([false, false]).show(ui, |ui| match self.tab {
                    0 => self.general_tab(ui),
                    1 => self.team_tab(ui),
                    2 => self.class_tab(ui, users, session),
                    _ => self.messages_tab(ui),
                });
            });
        }
        // Ctrl+Z and Ctrl+Y on the open trainer.
        if let Some(open) = self.open.as_mut() {
            self.history.track(ui, open.id as u64, open);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::trainer::TrainerPokemon;

    /// With the dump: the page opens, lists the trainers and opens the first one.
    #[test]
    fn the_page_lists_the_trainers_and_opens_one() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let mut session = Session { dump: Some(sango_core::Dump::open(&root).unwrap()), focus_page: true, ..Session::default() };
        let mut page = Trainer::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        assert_eq!(page.rows.len(), data::TRAINER_COUNT - 1);
        assert_eq!(page.rows[0].name, "Brendan");
        let open = page.open.as_ref().expect("the first trainer is open");
        assert_eq!((open.id, open.trainer.class, open.team.pokemon.len()), (1, 127, 1));
        assert_eq!(open.team.pokemon[0].species, 252);
        assert!(!page.dirty());
    }

    /// With the dump: every value of every class of the game is in a list of names (no raw number is left for the user).
    #[test]
    fn the_values_of_the_classes_have_names() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let store = Store::open(&sango_core::Dump::open(&root).unwrap()).unwrap();
        let mut unknown = Vec::new();
        for class in 1..data::CLASS_COUNT {
            let Ok(c) = store.class(None, class) else { continue };
            if !lists::BATTLE_EFFECTS.iter().any(|(v, _)| *v == c.battle_effect) {
                unknown.push(format!("class {class}: effect {}", c.battle_effect));
            }
            if c.model_3d != 255 && !lists::MODELS_3D.iter().any(|(v, _)| *v == c.model_3d) {
                unknown.push(format!("class {class}: model {}", c.model_3d));
            }
            if !lists::MUSIC.iter().any(|(v, _)| u32::from(*v) == c.eye_music & 0xFFFF) {
                unknown.push(format!("class {class}: music {}", c.eye_music));
            }
            if c.overworld_model as usize >= sango_overworld::names::MODEL_NAMES.len() {
                unknown.push(format!("class {class}: character {}", c.overworld_model));
            }
            if c.picture_2d as usize >= data::PICTURE_COUNT / 3 {
                unknown.push(format!("class {class}: picture {}", c.picture_2d));
            }
        }
        assert!(unknown.is_empty(), "{unknown:?}");
    }

    /// With the dump: the Team tab draws a team, and the import fills the team.
    #[test]
    fn the_team_builder_draws_and_imports() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let mut session = Session { dump: Some(sango_core::Dump::open(&root).unwrap()), focus_page: true, ..Session::default() };
        let mut page = Trainer::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Trainer, session: &mut Session| {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for _ in 0..3 {
            frame(&mut page, &mut session);
        }
        page.tab = 1;
        let text = "Garchomp @ Choice Scarf
Ability: Rough Skin
Level: 50
EVs: 252 Atk / 4 SpD / 252 Spe
Jolly Nature
- Earthquake
- Outrage
- Stone Edge
- Fire Fang

Gengar @ Life Orb
Ability: Levitate
Timid Nature
- Shadow Ball
";
        let open = page.open.as_mut().unwrap();
        page.builder.import_for_test(text, &mut open.team, &mut open.extras);
        for _ in 0..3 {
            frame(&mut page, &mut session);
        }
        let open = page.open.as_ref().unwrap();
        assert_eq!(open.team.pokemon.len(), 2);
        assert_eq!(open.team.pokemon[0].species, 445);
        assert!(open.team.moves && open.team.items);
        assert_eq!(open.extras.pokemon[0].evs, [0, 252, 0, 0, 4, 252]);
    }

    /// With the dump: a change of the team, of the class and of the name is saved on the SD card, read back, and reset.
    #[test]
    fn a_trainer_is_changed_saved_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_trainer_page_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(sango_core::Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = Trainer::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Trainer, session: &mut Session| {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        frame(&mut page, &mut session);
        {
            let open = page.open.as_mut().unwrap();
            open.team.pokemon[0].level = 12;
            open.team.pokemon.push(TrainerPokemon::new(25, 12));
            open.extras.pokemon.push(PokemonExtras::none());
            open.team.items = true;
            open.team.pokemon[1].item = 0x10E;
            open.trainer.money = 99;
            open.names[2] = "Ash".into();
            open.class.group = 1;
            open.extras.weather = 2;
            open.extras.terrain = 3;
            open.extras.field = 1;
            open.extras.pokemon[1].shiny = true;
            open.extras.pokemon[1].nickname = "Pika".into();
            open.extras.pokemon[1].evs = [0, 252, 0, 0, 4, 252];
            open.messages[0].texts[2] = "Let us battle!".into();
        }
        assert!(page.dirty());
        page.save(session.sd.as_ref().unwrap());
        assert!(!page.dirty(), "{:?}", page.status);
        let sd = session.sd.as_ref().unwrap();
        let (trainer, team) = page.store.as_ref().unwrap().trainer(Some(sd), 1).unwrap();
        assert_eq!((trainer.money, trainer.pokemon_count, trainer.data_type, team.pokemon[1].item), (99, 2, 3, 0x10E));
        assert_eq!(page.store.as_mut().unwrap().line(Some(sd), 2, data::TRAINER_NAMES, 1), "Ash");
        assert_eq!(Store::changed_trainers(Some(sd)), HashSet::from([1]));
        // The game files that the plugin reads are the ones of the SD card, with their list.
        assert!(sd.read("trainer/data/index.bin").is_some() && sd.read("trainer/pokemon/index.bin").is_some());
        // The extras and the message were written too: the plugin reads the extras, the game reads the message line.
        let extras = page.store.as_ref().unwrap().extras(Some(sd), 1, 2);
        assert_eq!((extras.weather, extras.terrain, extras.field, extras.pokemon[1].shiny, extras.pokemon[1].nickname.as_str()), (2, 3, 1, true, "Pika"));
        assert_eq!(extras.pokemon[1].evs, [0, 252, 0, 0, 4, 252]);
        assert_eq!(page.store.as_mut().unwrap().line(Some(sd), 2, data::MESSAGE_TEXT, 0), "Let us battle!");
        // The class change was written too.
        assert_eq!(page.store.as_ref().unwrap().class(Some(sd), 127).unwrap().group, 1);
        let sd = session.sd.clone().unwrap();
        page.reset(&sd);
        let (trainer, team) = page.store.as_ref().unwrap().trainer(Some(&sd), 1).unwrap();
        assert_eq!((trainer.pokemon_count, team.pokemon[0].level), (1, 5));
        assert!(Store::changed_trainers(Some(&sd)).is_empty() && Store::changed_classes(Some(&sd)).is_empty(), "the extras go with the trainer");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump: a picture of a class is changed on the SD card, read back, and reset.
    #[test]
    fn a_picture_of_a_trainer_is_changed_and_read_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_trainer_picture_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let store = Store::open(&sango_core::Dump::open(&root).unwrap()).unwrap();
        let (width, height) = store.game_picture_size(6).unwrap();
        assert_eq!((width, height), (512, 256));
        let (w, h, game) = store.picture(Some(&sd), 6).unwrap();
        assert_eq!((w, h, game.len()), (512, 256, 512 * 256 * 4));
        let red: Vec<u8> = (0..width * height).flat_map(|_| [255u8, 0, 0, 255]).collect();
        store.save_picture(&sd, 6, width, height, &red).unwrap();
        assert!(Store::picture_changed(Some(&sd), 6));
        assert_eq!(store.picture(Some(&sd), 6).unwrap().2, red);
        assert!(sd.read("trainer/picture/index.bin").is_some(), "the list of the pictures is written");
        store.reset_picture(&sd, 6).unwrap();
        assert_eq!(store.picture(Some(&sd), 6).unwrap().2, game);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
