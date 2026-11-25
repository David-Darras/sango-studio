//! The tab "Create a script": scripts made of blocks, without code.
//!
//! The user builds an event from blocks (show a text, ask a question, give an item, start a battle, go to a map...). Blocks can be inside
//! blocks: "If" has a list for the true case and a list for the other case, and a question with choices has a list for each choice.
//!
//! Studio keeps the blocks in `event_source/<n>.json` (on the computer only) and writes the file that the plugin reads as
//! `event/<n>.bin` (see `create_model::compile`). The event `n` is the script `32100 + n`: a sign, a trigger or a character that has this
//! script number starts it. The plugin reads the file each time that the event starts, so a change is used at once.

use crate::create_model::{self as model, Block, Compare, Condition, Event, COUNT, FIRST_SCRIPT, MAX_STEPS};
use sango_core::SdCard;
use sango_ui::theme::color;
use sango_ui::{egui, widget};
use std::path::PathBuf;

fn source_path(number: usize) -> String {
    format!("event_source/{number}.json")
}

fn file_path(number: usize) -> String {
    format!("event/{number}.bin")
}

/// What a list of blocks asks after it was drawn.
enum Change {
    Up(usize),
    Down(usize),
    Remove(usize),
}

/// The state of the tab.
pub struct Creator {
    opened_for: Option<PathBuf>,
    events: Vec<Event>,
    saved: Vec<Event>,
    chosen: usize,
    status: Option<(bool, String)>,
}

impl Default for Creator {
    fn default() -> Self {
        Creator { opened_for: None, events: vec![Event::default(); COUNT], saved: vec![Event::default(); COUNT], chosen: 0, status: None }
    }
}

impl Creator {
    /// True when an event has changes that are not saved.
    pub fn unsaved(&self) -> bool {
        self.events != self.saved
    }

    fn open(&mut self, sd: &SdCard) {
        self.opened_for = Some(sd.root().to_path_buf());
        self.status = None;
        for number in 0..COUNT {
            let event = sd.read(&source_path(number)).and_then(|bytes| serde_json::from_slice::<Event>(&bytes).ok()).unwrap_or_default();
            self.events[number] = event.clone();
            self.saved[number] = event;
        }
        self.chosen = self.events.iter().position(|e| !e.is_empty()).unwrap_or(0);
    }

    fn save(&mut self, sd: &SdCard, number: usize) {
        let event = self.events[number].clone();
        let result = if event.is_empty() && event.name.trim().is_empty() {
            sd.remove(&source_path(number)).and_then(|_| sd.remove(&file_path(number))).map(|_| ())
        } else {
            model::compile(&event).and_then(|bytes| {
                let json = serde_json::to_vec_pretty(&event).map_err(|e| e.to_string())?;
                sd.write(&source_path(number), &json)?;
                if event.is_empty() {
                    sd.remove(&file_path(number))?;
                } else {
                    sd.write(&file_path(number), &bytes)?;
                }
                Ok(())
            })
        };
        self.status = Some(match result {
            Ok(()) => {
                self.saved[number] = event;
                (true, "Saved. The game reads the event each time that it starts.".to_string())
            }
            Err(message) => (false, message),
        });
    }

    /// The numbers of the events that the list shows.
    fn shown(&self) -> Vec<usize> {
        (0..COUNT).filter(|&n| !self.events[n].is_empty() || !self.saved[n].is_empty() || n == self.chosen).collect()
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        // The arrows go to the next or the previous event, when no text is being edited.
        if widget::page_has_focus(ui) && !ui.ctx().wants_keyboard_input() {
            let step = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize);
            if step != 0 {
                let shown = self.shown();
                if let Some(at) = shown.iter().position(|&n| n == self.chosen) {
                    if let Some(&to) = shown.get((at as isize + step).max(0) as usize) {
                        self.chosen = to;
                        self.status = None;
                    }
                }
            }
        }
        egui::ScrollArea::vertical().id_salt("create_list").auto_shrink([false, false]).show(ui, |ui| {
            for number in self.shown() {
                let mut label = format!("{}  {}", number + 1, self.events[number].label());
                if self.events[number] != self.saved[number] {
                    label.push_str("  *");
                }
                let row = ui.selectable_label(self.chosen == number, label);
                widget::scroll_to_chosen(ui, &row, self.chosen == number);
                if row.clicked() {
                    self.chosen = number;
                    self.status = None;
                }
            }
        });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        widget::card(ui, |ui| {
            let number = self.chosen;
            ui.horizontal_wrapped(|ui| {
                if widget::button(ui, "New script") {
                    if let Some(free) = (0..COUNT).find(|&n| self.events[n].is_empty() && self.saved[n].is_empty() && n != self.chosen) {
                        self.chosen = free;
                        self.status = None;
                    } else {
                        self.status = Some((false, "All the scripts are used.".to_string()));
                    }
                }
                let changed = self.events[number] != self.saved[number];
                ui.add_enabled_ui(changed, |ui| {
                    if widget::save_button(ui, self.unsaved()) {
                        self.save(sd, number);
                    }
                    if widget::button(ui, "Undo") {
                        self.events[number] = self.saved[number].clone();
                        self.status = None;
                    }
                });
                if !self.saved[number].is_empty() && widget::confirm_button(ui, "Delete the script", "Click again to delete") {
                    self.events[number] = Event::default();
                    self.save(sd, number);
                }
                if changed {
                    widget::badge(ui, "Not saved", color::AMBER);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Name"));
                ui.add(egui::TextEdit::singleline(&mut self.events[number].name).desired_width(240.0).hint_text("(optional)"));
                ui.label(widget::hint(&format!("Script number {}: give it to a sign, a trigger or a character on the Overworld page.", FIRST_SCRIPT as usize + number)));
            });
            match model::step_count(&self.events[number]) {
                Ok(count) => {
                    ui.label(widget::hint(&format!("{count} of {MAX_STEPS} steps.")));
                }
                Err(message) => {
                    ui.label(egui::RichText::new(message).color(color::ROSE));
                }
            }
            if let Some((ok, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
            }
        });
    }

    /// Draws the tab.
    pub fn ui(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        ui.label(widget::hint("Scripts made of blocks, without code. A script can show texts, ask questions, test flags and items, give items and Pokémon, start battles and send the player to a map."));
        ui.add_space(6.0);
        let Some(sd) = sd else {
            widget::missing_card(ui, "Choose the SD card on the Home page to create scripts.");
            return;
        };
        if self.opened_for.as_deref() != Some(sd.root()) {
            self.open(sd);
        }
        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(230.0, ui.available_height()), |ui| {
                ui.set_width(230.0);
                self.list(ui);
            });
            ui.vertical(|ui| {
                self.tool_bar(ui, sd);
                ui.add_space(6.0);
                let number = self.chosen;
                egui::ScrollArea::vertical().id_salt("create_blocks").auto_shrink([false, false]).show(ui, |ui| {
                    blocks_ui(ui, &mut self.events[number].blocks);
                });
            });
        });
    }
}

/// A picker of the items of the game.
fn item_field(ui: &mut egui::Ui, value: &mut u32) {
    let mut number = (*value).min(u32::from(u16::MAX)) as u16;
    if sango_species::menus::item_menu(ui, "item", &mut number, 280.0, true) {
        *value = u32::from(number);
    }
}

/// A picker of the species of the game and of the packs, and of their forms: each form is a Pokémon of its own ("Rotom-Heat").
fn species_field(ui: &mut egui::Ui, species: &mut u32, form: &mut u32) {
    let mut value = (*species as usize, *form as usize);
    if sango_species::menus::pokemon_menu(ui, "species", &mut value, 280.0, None, None) {
        *species = value.0 as u32;
        *form = value.1 as u32;
    }
}

fn number_field(ui: &mut egui::Ui, label: &str, value: &mut u32, max: u32) {
    ui.label(widget::hint(label));
    widget::spin(ui, value, 0.0, f64::from(max));
}

fn flag_field(ui: &mut egui::Ui, flag: &mut u32, save: &mut bool) {
    ui.label(widget::hint("Flag"));
    widget::spin(ui, flag, 0.0, 4095.0);
    ui.selectable_value(save, true, "of Sango");
    ui.selectable_value(save, false, "of the game");
}

fn condition_fields(ui: &mut egui::Ui, condition: &mut Condition) {
    let mut kind = condition.kind();
    egui::ComboBox::from_id_salt("condition").selected_text(condition.label()).show_ui(ui, |ui| {
        for place in 0..Condition::KINDS {
            if ui.selectable_label(kind == place, Condition::of_kind(place).label()).clicked() {
                kind = place;
            }
        }
    });
    if kind != condition.kind() {
        *condition = Condition::of_kind(kind);
    }
    match condition {
        Condition::Flag { flag, save } => flag_field(ui, flag, save),
        Condition::Item { item, count } => {
            item_field(ui, item);
            number_field(ui, "times", count, 999);
        }
        Condition::Money { amount } => number_field(ui, "money or more:", amount, 999_999),
        Condition::Variable { variable, compare, value } => {
            number_field(ui, "variable", variable, 511);
            egui::ComboBox::from_id_salt("compare").selected_text(compare.label()).show_ui(ui, |ui| {
                for option in Compare::ALL {
                    ui.selectable_value(compare, option, option.label());
                }
            });
            number_field(ui, "", value, 65535);
        }
        Condition::Ask => {
            ui.label(widget::hint("The text of the block before is the question."));
        }
    }
}

/// The fields of a block that has no blocks inside.
fn fields(ui: &mut egui::Ui, block: &mut Block) {
    match block {
        Block::Say { text, talk } => {
            ui.vertical(|ui| {
                ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(420.0).hint_text("The text. It uses the readable form of the text editor of the plugin."));
                ui.horizontal(|ui| {
                    ui.selectable_value(talk, true, "Conversation window");
                    ui.selectable_value(talk, false, "Sign window");
                });
            });
        }
        Block::GiveItem { item, count } | Block::TakeItem { item, count } => {
            item_field(ui, item);
            number_field(ui, "times", count, 999);
        }
        Block::Money { amount, take } => {
            number_field(ui, "Amount", amount, 999_999);
            ui.selectable_value(take, false, "Give");
            ui.selectable_value(take, true, "Take");
        }
        Block::SetFlag { flag, save, on } => {
            flag_field(ui, flag, save);
            ui.selectable_value(on, true, "Set");
            ui.selectable_value(on, false, "Clear");
        }
        Block::SetVariable { variable, value } => {
            number_field(ui, "Variable of Sango", variable, 511);
            number_field(ui, "gets", value, 65535);
        }
        Block::Sound { sound } | Block::Jingle { sound } => number_field(ui, "Sound number", sound, 65535),
        Block::Wait { frames } => number_field(ui, "Frames (60 is one second)", frames, 3600),
        Block::RunScript { script } => number_field(ui, "Script number", script, 65535),
        Block::GivePokemon { species, level, form } | Block::WildBattle { species, level, form } => {
            species_field(ui, species, form);
            number_field(ui, "level", level, 100);
        }
        Block::TrainerBattle { trainer } => number_field(ui, "Trainer number", trainer, 65535),
        Block::Warp { map, x, z } => {
            number_field(ui, "Map", map, 65535);
            number_field(ui, "tile X", x, 65535);
            number_field(ui, "tile Z", z, 65535);
        }
        Block::Fade { out } => {
            ui.selectable_value(out, true, "Fade out");
            ui.selectable_value(out, false, "Fade in");
        }
        Block::Heal | Block::Stop | Block::If { .. } | Block::Choice { .. } => {}
    }
}

/// The menu that adds a block to a list.
fn add_menu(ui: &mut egui::Ui, blocks: &mut Vec<Block>) {
    let mut added = None;
    ui.menu_button("+ Add a block", |ui| {
        for (name, block) in Block::kinds() {
            if ui.button(name).clicked() {
                added = Some(block);
                ui.close_menu();
            }
        }
    });
    if let Some(block) = added {
        blocks.push(block);
    }
}

/// Draws a list of blocks, with the buttons that move and remove them.
fn blocks_ui(ui: &mut egui::Ui, blocks: &mut Vec<Block>) {
    let mut change = None;
    let count = blocks.len();
    for (place, block) in blocks.iter_mut().enumerate() {
        ui.push_id(place, |ui| {
            egui::Frame::none().fill(color::SURFACE).rounding(6.0).inner_margin(egui::Margin::same(8.0)).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(block.title()).strong().color(color::ACCENT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widget::button(ui, "✕") {
                            change = Some(Change::Remove(place));
                        }
                        ui.add_enabled_ui(place + 1 < count, |ui| {
                            if widget::button(ui, "↓") {
                                change = Some(Change::Down(place));
                            }
                        });
                        ui.add_enabled_ui(place > 0, |ui| {
                            if widget::button(ui, "↑") {
                                change = Some(Change::Up(place));
                            }
                        });
                    });
                });
                match block {
                    Block::If { condition, then, otherwise } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(widget::hint("When"));
                            condition_fields(ui, condition);
                        });
                        let (yes, no) = if matches!(condition, Condition::Ask) { ("Yes", "No") } else { ("Then", "Otherwise") };
                        ui.label(widget::hint(yes));
                        ui.indent("then", |ui| {
                            ui.push_id("then", |ui| blocks_ui(ui, then));
                        });
                        ui.label(widget::hint(no));
                        ui.indent("otherwise", |ui| {
                            ui.push_id("otherwise", |ui| blocks_ui(ui, otherwise));
                        });
                    }
                    Block::Choice { question, options } => {
                        ui.add(egui::TextEdit::multiline(question).desired_rows(2).desired_width(420.0).hint_text("The question"));
                        let mut remove = None;
                        let many = options.len() > 2;
                        for (index, (name, inside)) in options.iter_mut().enumerate() {
                            ui.push_id(("choice", index), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(widget::hint(&format!("Choice {}", index + 1)));
                                    ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
                                    if many && widget::button(ui, "Remove the choice") {
                                        remove = Some(index);
                                    }
                                });
                                ui.indent("inside", |ui| blocks_ui(ui, inside));
                            });
                        }
                        if let Some(index) = remove {
                            options.remove(index);
                        }
                        if options.len() < 3 && widget::button(ui, "Add a choice") {
                            options.push((format!("Choice {}", options.len() + 1), Vec::new()));
                        }
                    }
                    other => {
                        ui.horizontal_wrapped(|ui| fields(ui, other));
                    }
                }
            });
        });
        ui.add_space(4.0);
    }
    match change {
        Some(Change::Up(place)) => blocks.swap(place, place - 1),
        Some(Change::Down(place)) => blocks.swap(place, place + 1),
        Some(Change::Remove(place)) => {
            blocks.remove(place);
        }
        None => {}
    }
    add_menu(ui, blocks);
}
