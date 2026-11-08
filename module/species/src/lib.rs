//! The species editor: one form for the data of a species or of a form.
//!
//! The data comes from the archive 195 of the romfs. The file of the SD card
//! `pokemon/data/<species>_<form>.bin` takes the place of the romfs when it exists.

mod data;
mod names;

use data::SpeciesData;
use sango_core::Garc;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of the archive that has the data of the species.
const ARCHIVE: usize = 195;

/// The number of species that the list shows.
const SPECIES_COUNT: usize = names::SPECIES.len();

/// The tabs of the page. Only the first one works for now.
const TABS: [&str; 7] = ["General", "Movepool", "Evolution", "Mega evolution", "Form", "Icon", "Model"];

/// The place in the archive of the file of a species and a form.
///
/// A form after the first one is in the archive after all the species: the file of the species says where.
fn file_index(garc: &Garc, species: usize, form: usize) -> usize {
    if form == 0 {
        return species;
    }
    match garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()) {
        Some(base) if base.form_index != 0 && form < base.form_count as usize => base.form_index as usize + form - 1,
        _ => species,
    }
}

/// The path in the SD card folder of the file of a species and a form.
fn sd_path(species: usize, form: usize) -> String {
    format!("pokemon/data/{species}_{form}.bin")
}

/// The name in a list from a number. The first number of the list is `first`.
fn name_from(list: &[&str], number: usize, first: usize) -> String {
    number.checked_sub(first).and_then(|i| list.get(i)).map_or("-".to_string(), |s| s.to_string())
}

pub struct Species {
    /// The dump that the archive comes from.
    root: Option<PathBuf>,
    garc: Option<Garc>,
    species: usize,
    form: usize,
    /// The data in the form.
    data: Option<SpeciesData>,
    /// The data as it was loaded or saved: to know what changed.
    baseline: Option<SpeciesData>,
    /// True when the data came from the SD card.
    from_sd: bool,
    /// The species and the form that `data` is for.
    loaded: Option<(usize, usize)>,
    tab: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// True while the list of the species is open: it shows the chosen species at the middle once.
    list_shown: bool,
    /// The list must show the chosen species at the middle (after a key).
    scroll_list: bool,
    /// Text typed into the species search box.
    species_search: String,
    /// True once the search box has received focus on opening the dropdown.
    search_focused: bool,
}

impl Default for Species {
    fn default() -> Self {
        Species {
            root: None,
            garc: None,
            species: 1,
            form: 0,
            data: None,
            baseline: None,
            from_sd: false,
            loaded: None,
            tab: 0,
            status: None,
            list_shown: false,
            scroll_list: false,
            species_search: String::new(),
            search_focused: false,
        }
    }
}

/// Matches species by name (prefix or substring) or by number.
fn match_species(raw_query: &str) -> Vec<usize> {
    let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
    let query = clean(raw_query.trim());
    let num_query = raw_query.trim().trim_start_matches('0');

    if query.is_empty() {
        return (1..=SPECIES_COUNT).collect();
    }

    let mut starts_with = Vec::new();
    let mut contains = Vec::new();
    for number in 1..=SPECIES_COUNT {
        let name = clean(names::SPECIES[number - 1]);
        let num_str = number.to_string();
        let is_num_match = !num_query.is_empty() && num_str == num_query;
        if name.starts_with(&query) || is_num_match {
            starts_with.push(number);
        } else if name.contains(&query) || (!num_query.is_empty() && num_str.contains(num_query)) {
            contains.push(number);
        }
    }
    starts_with.extend(contains);
    starts_with
}

impl Species {
    /// Reads the data from the romfs.
    fn read_romfs(&self) -> Result<SpeciesData, String> {
        let garc = self.garc.as_ref().ok_or("The archive is not open.")?;
        let id = file_index(garc, self.species, self.form);
        SpeciesData::from_bytes(garc.first_file(id).ok_or_else(|| format!("The file {id} is missing."))?)
    }

    /// Reads the data of the chosen species and form: the SD card first, then the romfs.
    fn load(&mut self, session: &Session) {
        self.loaded = Some((self.species, self.form));
        let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(self.species, self.form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
        self.from_sd = on_card.is_some();
        match on_card.map(Ok).unwrap_or_else(|| self.read_romfs()) {
            Ok(data) => {
                self.baseline = Some(data.clone());
                self.data = Some(data);
                self.status = None;
            }
            Err(message) => {
                self.data = None;
                self.baseline = None;
                self.status = Some((false, message));
            }
        }
    }

    /// The number of forms of the chosen species (at least one).
    fn form_count(&self) -> usize {
        let base = self.garc.as_ref().and_then(|g| g.first_file(self.species)).and_then(|b| SpeciesData::from_bytes(b).ok());
        base.map_or(1, |b| (b.form_count as usize).max(1))
    }

    /// The menus, the two buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut reset = false;
        let mut save = false;
        let before = (self.species, self.form);
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let species_text = format!("{:>3}  {}", self.species, name_from(&names::SPECIES, self.species, 1));
                let species_id = ui.make_persistent_id("species_menu");
                let popup_id = species_id.with("popup");
                let is_popup_open = ui.memory(|m| m.is_popup_open(popup_id));

                let button_width = 220.0;
                let button_height = ui.spacing().interact_size.y;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(button_width, button_height), egui::Sense::hover());
                let button_response = ui.interact(rect, species_id, egui::Sense::click());

                if ui.is_rect_visible(rect) {
                    let visuals = if is_popup_open {
                        &ui.visuals().widgets.open
                    } else {
                        ui.style().interact(&button_response)
                    };
                    ui.painter().rect(
                        rect.expand(visuals.expansion),
                        visuals.rounding,
                        visuals.weak_bg_fill,
                        visuals.bg_stroke,
                    );
                    let galley = egui::WidgetText::from(&species_text).into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        button_width - 28.0,
                        egui::TextStyle::Button,
                    );
                    let text_rect = rect.shrink2(ui.spacing().button_padding);
                    let text_pos = egui::Align2::LEFT_CENTER.align_size_within_rect(galley.size(), text_rect).min;
                    ui.painter().galley(text_pos, galley, visuals.text_color());

                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.right() - 14.0, rect.center().y),
                        egui::vec2(8.0, 5.0),
                    );
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![icon_rect.left_top(), icon_rect.right_top(), icon_rect.center_bottom()],
                        visuals.fg_stroke.color,
                        egui::Stroke::NONE,
                    ));
                }

                if button_response.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                    widget::set_active_id(ui, species_id);
                }

                widget::register_focusable(ui, species_id);
                let popup_res = egui::popup::popup_below_widget(
                    ui,
                    popup_id,
                    &button_response,
                    egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_width(260.0);
                        ui.set_max_width(260.0);
                        ui.spacing_mut().item_spacing.y = 4.0;

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(widget::hint("🔍"));
                            let search_input = ui.add(
                                egui::TextEdit::singleline(&mut self.species_search)
                                    .hint_text("Search name or number...")
                                    .desired_width(185.0)
                                    .clip_text(true),
                            );
                            if !self.search_focused {
                                search_input.request_focus();
                                self.search_focused = true;
                            }
                            if !self.species_search.is_empty() && ui.small_button("✕").clicked() {
                                self.species_search.clear();
                            }
                        });
                        ui.separator();

                        let query = self.species_search.trim();
                        let matches = match_species(query);

                        if matches.is_empty() {
                            ui.add_space(4.0);
                            ui.label(widget::hint("No matching Pokémon."));
                            ui.add_space(4.0);
                        } else {
                            let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                            let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

                            if let Some(pos) = matches.iter().position(|&n| n == self.species) {
                                if down && pos + 1 < matches.len() {
                                    self.species = matches[pos + 1];
                                }
                                if up && pos > 0 {
                                    self.species = matches[pos - 1];
                                }
                            } else if (up || down) && !matches.is_empty() {
                                self.species = matches[0];
                            }

                            if enter && !matches.is_empty() {
                                if !matches.contains(&self.species) {
                                    self.species = matches[0];
                                }
                                ui.memory_mut(|m| m.close_popup());
                            }

                            let nav_key = up || down;
                            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                                for &number in &matches {
                                    let chosen = self.species == number
                                        || (!matches.contains(&self.species) && number == matches[0]);
                                    let row = ui.selectable_label(chosen, format!("{number:>3}  {}", names::SPECIES[number - 1]));
                                    if chosen {
                                        widget::glow(ui, row.rect, 5.0);
                                        if nav_key || (query.is_empty() && (self.scroll_list || !self.list_shown)) {
                                            row.scroll_to_me(Some(egui::Align::Center));
                                        }
                                    }
                                    if row.clicked() {
                                        self.species = number;
                                        ui.memory_mut(|m| m.close_popup());
                                    }
                                }
                            });
                        }
                        self.list_shown = true;
                        self.scroll_list = false;
                    },
                );

                if popup_res.is_none() {
                    self.list_shown = false;
                    self.search_focused = false;
                    if !self.species_search.is_empty() {
                        self.species_search.clear();
                    }
                }

                if widget::active_id(ui).is_none() {
                    widget::set_active_id(ui, species_id);
                }
                if widget::is_active(ui, species_id) && widget::page_has_focus(ui) && popup_res.is_none() {
                    widget::glow(ui, button_response.rect, 6.0);
                    widget::scroll_active_into_view(ui, &button_response);
                    let open_key = ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            || i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                    });
                    if open_key {
                        ui.memory_mut(|m| m.open_popup(popup_id));
                    }
                }
                let forms = self.form_count();
                self.form = self.form.min(forms.saturating_sub(1));
                let form_text = |form: usize| if form == 0 { "Form 0 (normal)".to_string() } else { format!("Form {form}") };
                let form_combo = egui::ComboBox::from_id_salt("form_menu").selected_text(form_text(self.form)).width(150.0).show_ui(ui, |ui| {
                    for form in 0..forms {
                        let chosen = self.form == form;
                        let row = ui.selectable_label(chosen, form_text(form));
                        if chosen {
                            widget::glow(ui, row.rect, 5.0);
                        }
                        if row.clicked() {
                            self.form = form;
                        }
                    }
                });
                let form_id = form_combo.response.id;
                widget::register_focusable(ui, form_id);
                if form_combo.response.clicked() {
                    widget::set_active_id(ui, form_id);
                }
                if widget::is_active(ui, form_id) && widget::page_has_focus(ui) {
                    widget::glow(ui, form_combo.response.rect, 6.0);
                    widget::scroll_active_into_view(ui, &form_combo.response);
                    ui.input_mut(|i| {
                        let step = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize
                            - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize;
                        if step != 0 && forms > 0 {
                            self.form = (self.form as isize + step).clamp(0, forms as isize - 1) as usize;
                        }
                    });
                }
                reset = widget::button(ui, "Reset");
                save = self.data.is_some() && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
                if self.data.is_some() && self.data != self.baseline {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else if self.data.is_some() {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some((ok, message)) = &self.status {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
            }
        });
        if (self.species, self.form) != before {
            self.loaded = None;
        }
        if reset {
            match self.read_romfs() {
                Ok(data) => {
                    self.data = Some(data);
                    self.status = Some((true, "The data comes from the romfs again. Save to write it on the SD card.".into()));
                }
                Err(message) => self.status = Some((false, message)),
            }
        }
        if save {
            self.save(session);
        }
    }

    /// Writes the file on the SD card and tells the plugin.
    fn save(&mut self, session: &Session) {
        let (Some(data), Some(sd)) = (&self.data, &session.sd) else { return };
        let path = sd_path(self.species, self.form);
        let result = sd.write(&path, &data.to_bytes()).and_then(|_| sd.mark_changed(&path));
        match result {
            Ok(()) => {
                self.baseline = self.data.clone();
                self.from_sd = true;
                self.status = Some((true, format!("Saved in {path}. The game loads it again by itself.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }
}

/// A rate from 0 to 255 as a percentage of the maximum.
fn percent(value: u8) -> String {
    let text = format!("{:.1}", value as f32 * 100.0 / 255.0);
    format!("{} %", text.strip_suffix(".0").unwrap_or(&text))
}

/// What a gender number means: 0 is only male, 254 only female, 255 no gender, and in between
/// the chance to be female is `(number + 1) / 256`.
fn gender_text(value: u8) -> String {
    match value {
        0 => "male only".to_string(),
        254 => "female only".to_string(),
        255 => "no gender".to_string(),
        _ => {
            let female = (value as f32 + 1.0) * 100.0 / 256.0;
            format!("{} % female, {} % male", trim(female), trim(100.0 - female))
        }
    }
}

/// A number with one digit, without a useless ".0".
fn trim(value: f32) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

/// A number field with arrow buttons: the up and down keys work too.
fn number<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, max: f64) {
    widget::spin(ui, value, 0.0, max);
}

/// A line of the grid: the name of the field, then its widgets.
fn line(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(widget::hint(label));
    ui.horizontal(add);
    ui.end_row();
}

/// A field with the number of an ability and its name.
fn ability(ui: &mut egui::Ui, value: &mut u8) {
    number(ui, value, 255.0);
    ui.label(name_from(&names::ABILITY, *value as usize, 1));
}

/// A field with the number of an item and its name.
fn item(ui: &mut egui::Ui, value: &mut u16) {
    number(ui, value, 65535.0);
    ui.label(name_from(&names::ITEM, *value as usize, 1));
}

/// A menu of names: the value is the place in the list.
fn menu(ui: &mut egui::Ui, id: &str, value: &mut u8, list: &[&str], width: f32) {
    let combo = egui::ComboBox::from_id_salt(id).selected_text(name_from(list, *value as usize, 0)).width(width).show_ui(ui, |ui| {
        for (place, name) in list.iter().enumerate() {
            let chosen = *value == place as u8;
            let row = ui.selectable_label(chosen, *name);
            if chosen {
                widget::glow(ui, row.rect, 5.0);
            }
            if row.clicked() {
                *value = place as u8;
            }
        }
    });
    let menu_id = combo.response.id;
    widget::register_focusable(ui, menu_id);
    if combo.response.clicked() {
        widget::set_active_id(ui, menu_id);
    }
    if widget::is_active(ui, menu_id) && widget::page_has_focus(ui) {
        widget::glow(ui, combo.response.rect, 6.0);
        widget::scroll_active_into_view(ui, &combo.response);
        ui.input_mut(|i| {
            let mut step: isize = 0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                step -= 1;
            }
            if step != 0 && !list.is_empty() {
                *value = (*value as isize + step).clamp(0, list.len() as isize - 1) as u8;
            }
        });
    }
}

/// The width of one column of groups, in points.
const COLUMN_WIDTH: f32 = 430.0;

/// A grid for the fields of a group.
fn grid(name: &'static str) -> egui::Grid {
    egui::Grid::new(name).num_columns(2).spacing([24.0, 8.0])
}

/// A group of fields: a card with a title.
fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    widget::card(ui, |ui| {
        ui.set_min_width(COLUMN_WIDTH - 36.0);
        ui.label(widget::title(title));
        add(ui);
    });
    ui.add_space(10.0);
}

fn group_statistics(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, &format!("Base statistics (total {})", d.base_total()), |ui| {
        grid("stats").show(ui, |ui| {
            line(ui, "HP", |ui| number(ui, &mut d.base_hp, 255.0));
            line(ui, "Attack", |ui| number(ui, &mut d.base_attack, 255.0));
            line(ui, "Defense", |ui| number(ui, &mut d.base_defense, 255.0));
            line(ui, "Special Attack", |ui| number(ui, &mut d.base_special_attack, 255.0));
            line(ui, "Special Defense", |ui| number(ui, &mut d.base_special_defense, 255.0));
            line(ui, "Speed", |ui| number(ui, &mut d.base_speed, 255.0));
        });
    });
}

fn group_types(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, "Types and abilities", |ui| {
        grid("types").show(ui, |ui| {
            line(ui, "Type 1", |ui| menu(ui, "type_1", &mut d.types[0], &names::TYPE, 120.0));
            line(ui, "Type 2", |ui| menu(ui, "type_2", &mut d.types[1], &names::TYPE, 120.0));
            line(ui, "Ability 1", |ui| ability(ui, &mut d.ability[0]));
            line(ui, "Ability 2", |ui| ability(ui, &mut d.ability[1]));
            line(ui, "Hidden ability", |ui| ability(ui, &mut d.ability[2]));
        });
    });
}

fn group_catch(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, "Catch, rewards and breeding", |ui| {
        grid("catch").show(ui, |ui| {
            line(ui, "Catch rate", |ui| {
                number(ui, &mut d.capture_rate, 255.0);
                ui.label(widget::hint(&percent(d.capture_rate)));
            });
            line(ui, "Escape rate", |ui| {
                number(ui, &mut d.escape_rate, 255.0);
                ui.label(widget::hint(&percent(d.escape_rate)));
            });
            line(ui, "Base experience", |ui| number(ui, &mut d.give_experience, 65535.0));
            line(ui, "Effort values given", |ui| {
                number(ui, &mut d.give_effort_values, 65535.0);
                ui.label(widget::hint("2 bits per stat"));
            });
            line(ui, "Item 1 (wild)", |ui| item(ui, &mut d.give_item[0]));
            line(ui, "Item 2 (wild)", |ui| item(ui, &mut d.give_item[1]));
            line(ui, "Item 3 (wild)", |ui| item(ui, &mut d.give_item[2]));
            line(ui, "Gender", |ui| {
                number(ui, &mut d.gender, 255.0);
                ui.label(widget::hint(&gender_text(d.gender)));
            });
            line(ui, "Egg hatch cycles", |ui| number(ui, &mut d.egg_hatch_steps, 255.0));
            line(ui, "Base friendship", |ui| number(ui, &mut d.base_friendship, 255.0));
            line(ui, "Egg Group 1", |ui| menu(ui, "egg_1", &mut d.egg_group[0], &names::EGG_GROUP, 140.0));
            line(ui, "Egg Group 2", |ui| menu(ui, "egg_2", &mut d.egg_group[1], &names::EGG_GROUP, 140.0));
        });
    });
}

fn group_size(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, "Size and forms", |ui| {
        grid("size").show(ui, |ui| {
            line(ui, "Height", |ui| number(ui, &mut d.height, 65535.0));
            line(ui, "Fake height", |ui| number(ui, &mut d.fake_height, 65535.0));
            line(ui, "Weight (hectograms)", |ui| number(ui, &mut d.weight, 65535.0));
            line(ui, "Form count", |ui| number(ui, &mut d.form_count, 255.0));
            line(ui, "Form index", |ui| number(ui, &mut d.form_index, 65535.0));
            line(ui, "Form index 2", |ui| number(ui, &mut d.form_index_2, 65535.0));
        });
    });
}

fn group_machines(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, "Machines (one bit each)", |ui| {
        grid("machines").show(ui, |ui| {
            for (i, word) in d.technical_moves.iter_mut().enumerate() {
                line(ui, &format!("Bits {} to {}", i * 32, i * 32 + 31), |ui| {
                    let r = ui.add(egui::DragValue::new(word).hexadecimal(8, false, true).speed(1.0));
                    widget::register_focusable(ui, r.id);
                    if r.clicked() || r.dragged() || r.has_focus() {
                        widget::set_active_id(ui, r.id);
                    }
                    if widget::is_active(ui, r.id) && widget::page_has_focus(ui) {
                        widget::glow(ui, r.rect, 4.0);
                        widget::scroll_active_into_view(ui, &r);
                        ui.input_mut(|inp| {
                            let mut delta: i64 = 0;
                            if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                                delta += 1;
                            }
                            if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                                delta -= 1;
                            }
                            if delta != 0 {
                                *word = word.wrapping_add_signed(delta as i32);
                            }
                        });
                    }
                });
            }
        });
    });
}

fn group_unknown(ui: &mut egui::Ui, d: &mut SpeciesData) {
    group(ui, "Not described yet", |ui| {
        grid("unknown").show(ui, |ui| {
            line(ui, "Unknown 0", |ui| number(ui, &mut d.unknown_0, 255.0));
            line(ui, "Unknown 1", |ui| number(ui, &mut d.unknown_1, 255.0));
            line(ui, "Unknown 2", |ui| number(ui, &mut d.unknown_2, 255.0));
            line(ui, "Unknown 4", |ui| number(ui, &mut d.unknown_4, 65535.0));
            line(ui, "Last 16 bytes", |ui| {
                // Two lines of eight bytes, to keep the group narrow.
                let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
                ui.label(egui::RichText::new(format!("{}
{}", hex(&d.rest[..8]), hex(&d.rest[8..]))).monospace());
            });
        });
    });
}

/// The groups of the General tab, side by side: as many columns as the window has room for.
fn general_tab(ui: &mut egui::Ui, d: &mut SpeciesData) {
    const GROUPS: [fn(&mut egui::Ui, &mut SpeciesData); 6] = [group_statistics, group_types, group_catch, group_size, group_machines, group_unknown];
    let columns = (((ui.available_width() + 10.0) / (COLUMN_WIDTH + 10.0)).floor() as usize).clamp(1, 3);
    let layout: Vec<Vec<usize>> = match columns {
        3 => vec![vec![0, 1], vec![2], vec![3, 4, 5]],
        2 => vec![vec![0, 1, 3], vec![2, 4, 5]],
        _ => vec![(0..GROUPS.len()).collect()],
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for column in layout {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN_WIDTH);
                    for index in column {
                        GROUPS[index](ui, d);
                    }
                });
                ui.add_space(10.0);
            }
        });
    });
}

impl Module for Species {
    fn title(&self) -> &str {
        "Species"
    }

    fn icon(&self) -> &str {
        "S"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Species");
        ui.add_space(8.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        if self.root.as_deref() != Some(dump.root()) {
            self.root = Some(dump.root().to_path_buf());
            match Garc::open(&dump.archive(ARCHIVE)) {
                Ok(garc) => self.garc = Some(garc),
                Err(message) => {
                    self.garc = None;
                    self.status = Some((false, message));
                }
            }
            self.loaded = None;
        }
        if self.garc.is_some() && self.loaded != Some((self.species, self.form)) {
            self.load(session);
        }
        self.top_bar(ui, session);
        ui.add_space(8.0);
        let tabs_id = ui.make_persistent_id("species_tabs");
        widget::register_focusable(ui, tabs_id);
        ui.horizontal_wrapped(|ui| {
            for (i, name) in TABS.iter().enumerate() {
                let chosen = self.tab == i;
                let r = ui.selectable_label(chosen, *name);
                if r.clicked() {
                    self.tab = i;
                    widget::set_active_id(ui, tabs_id);
                }
                if chosen && widget::is_active(ui, tabs_id) && widget::page_has_focus(ui) {
                    widget::glow(ui, r.rect, 6.0);
                }
            }
        });
        if widget::is_active(ui, tabs_id) && widget::page_has_focus(ui) {
            ui.input_mut(|i| {
                let mut step: isize = 0;
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                {
                    step += 1;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                {
                    step -= 1;
                }
                if step != 0 {
                    self.tab = (self.tab as isize + step).clamp(0, TABS.len() as isize - 1) as usize;
                }
            });
        }
        ui.add_space(6.0);
        match (self.tab, self.data.as_mut()) {
            (0, Some(data)) => general_tab(ui, data),
            (0, None) => {}
            _ => {
                ui.label(widget::hint("..."));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_rate_is_a_percentage_of_255() {
        assert_eq!(percent(255), "100 %");
        assert_eq!(percent(0), "0 %");
        assert_eq!(percent(45), "17.6 %");
        assert_eq!(percent(3), "1.2 %");
    }

    #[test]
    fn the_gender_number_gives_the_chance_to_be_female() {
        assert_eq!(gender_text(0), "male only");
        assert_eq!(gender_text(254), "female only");
        assert_eq!(gender_text(255), "no gender");
        assert_eq!(gender_text(31), "12.5 % female, 87.5 % male");
        assert_eq!(gender_text(127), "50 % female, 50 % male");
    }

    #[test]
    fn species_can_be_searched_by_name_or_number() {
        assert_eq!(match_species("").len(), 721);
        assert_eq!(match_species("pika")[0], 25);
        assert_eq!(match_species("char")[0], 4);
        assert_eq!(match_species("25")[0], 25);
        assert_eq!(match_species("025")[0], 25);
        assert_eq!(match_species("flabebe")[0], 669);
    }

    #[test]
    fn clicking_top_bar_and_general_tab_does_not_crash() {
        let mut species = Species::default();
        let mut data = data::SpeciesData::from_bytes(&[0; 80]).unwrap();
        species.data = Some(data.clone());
        species.baseline = Some(data.clone());

        let ctx = egui::Context::default();
        let session = Session::default();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                widget::set_page_focus(ctx, true);
                widget::update_navigation(ui);
                species.top_bar(ui, &session);
                general_tab(ui, &mut data);
            });
        });

        for &(x, y) in &[(10.0, 10.0), (100.0, 50.0), (250.0, 50.0), (350.0, 50.0), (50.0, 200.0), (200.0, 300.0)] {
            let mut click = egui::RawInput::default();
            click.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            let _ = ctx.run(click, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::set_page_focus(ctx, true);
                    widget::update_navigation(ui);
                    species.top_bar(ui, &session);
                    general_tab(ui, &mut data);
                });
            });
        }
    }
}
