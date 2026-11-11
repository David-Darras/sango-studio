//! The species editor: one form for the data of a species or of a form.
//!
//! The data comes from the archive 195 of the romfs. The file of the SD card
//! `pokemon/data/<species>_<form>.bin` takes the place of the romfs when it exists.

mod data;
mod pokedex_data;
mod names;

use data::SpeciesData;
use pokedex_data::PokedexData;
use sango_core::Garc;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The number of the archive that has the data of the species.
const ARCHIVE: usize = 195;

/// The number of species that the list shows.
const SPECIES_COUNT: usize = names::SPECIES.len();

/// The tabs of the page. Only the first one works for now.
const TABS: [&str; 6] = ["General", "Learnset", "Evolution", "Text", "Icon", "Model"];

/// The place in the archive of the file of a species and a form.
///
/// A form after the first one is in the archive after all the species: the file of the species says where.
fn file_index(garc: &Garc, species: usize, form: usize) -> usize {
    if form == 0 {
        return species;
    }
    match garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()) {
        Some(base) if base.alternate_form_file != 0 && form < base.total_form_count as usize => base.alternate_form_file as usize + form - 1,
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
    /// The Pokédex data (the shapes). It comes from the SD card when the file is there.
    pokedex: Option<PokedexData>,
    /// The archive of the Pokédex data.
    pokedex_archive: Option<Garc>,
    /// The shape of the chosen species and form.
    shape: u8,
    /// The shape as it was loaded or saved.
    shape_baseline: u8,
    /// True when the Pokédex data came from the SD card.
    pokedex_from_sd: bool,
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
            pokedex: None,
            pokedex_archive: None,
            shape: 0,
            shape_baseline: 0,
            pokedex_from_sd: false,
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
    let num_query = raw_query.trim().trim_start_matches('#').trim_start_matches('0');

    if query.is_empty() {
        return (1..=SPECIES_COUNT).collect();
    }

    let mut starts_with = Vec::new();
    let mut contains = Vec::new();
    for number in 1..=SPECIES_COUNT {
        let name = clean(names::SPECIES[number - 1]);
        let num_str = number.to_string();
        let formatted = format!("#{:04}", number);
        let is_num_match = !num_query.is_empty() && num_str == num_query;
        if name.starts_with(&query) || formatted.starts_with(&query) || is_num_match {
            starts_with.push(number);
        } else if name.contains(&query) || formatted.contains(&query) || (!num_query.is_empty() && num_str.contains(num_query)) {
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
        self.load_shape(session);
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

    /// Reads the Pokédex data from the romfs.
    fn read_pokedex_romfs(&self) -> Result<PokedexData, String> {
        let garc = self.pokedex_archive.as_ref().ok_or("The archive of the Pokédex data is not open.")?;
        let bytes = garc.first_file(pokedex_data::FILE).ok_or("The file of the Pokédex data is missing.")?;
        PokedexData::from_bytes(bytes.to_vec())
    }

    /// Reads the Pokédex data (the SD card first, then the romfs) and takes the shape of the chosen form.
    fn load_shape(&mut self, session: &Session) {
        let on_card = session.sd.as_ref().and_then(|sd| sd.read(pokedex_data::SD_PATH)).and_then(|b| PokedexData::from_bytes(b).ok());
        self.pokedex_from_sd = on_card.is_some();
        self.pokedex = on_card.or_else(|| self.read_pokedex_romfs().ok());
        self.shape = self.pokedex.as_ref().and_then(|e| e.shape(self.species, self.form)).unwrap_or(0);
        self.shape_baseline = self.shape;
    }

    /// The number of forms of the chosen species (at least one).
    fn total_form_count(&self) -> usize {
        let base = self.garc.as_ref().and_then(|g| g.first_file(self.species)).and_then(|b| SpeciesData::from_bytes(b).ok());
        base.map_or(1, |b| (b.total_form_count as usize).max(1))
    }

    /// The menus, the two buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let mut reset = false;
        let mut save = false;
        let before = (self.species, self.form);
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let species_text = format!("#{:04} {}", self.species, name_from(&names::SPECIES, self.species, 1));
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

                widget::register_focusable(ui, species_id, button_response.rect);
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
                                    let row = ui.selectable_label(chosen, format!("#{:04} {}", number, names::SPECIES[number - 1]));
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
                    ui.input_mut(|i| {
                        let step = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as isize
                            - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as isize;
                        if step != 0 {
                            let total = names::SPECIES.len() as isize;
                            self.species = (self.species as isize + step).clamp(1, total) as usize;
                        }
                    });
                }
                let forms = self.total_form_count();
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
                widget::register_focusable(ui, form_id, form_combo.response.rect);
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
                if self.data.is_some() && (self.data != self.baseline || self.shape != self.shape_baseline) {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd || self.pokedex_from_sd {
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
                    if let Some(shape) = self.read_pokedex_romfs().ok().and_then(|e| e.shape(self.species, self.form)) {
                        self.shape = shape;
                    }
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
        let mut result = sd.write(&path, &data.to_bytes()).and_then(|_| sd.mark_changed(&path));
        // The shape is in the Pokédex data: the file is written again when the shape changed.
        let mut shape_saved = false;
        if result.is_ok() && self.shape != self.shape_baseline {
            let placed = self.pokedex.as_mut().is_some_and(|pokedex| pokedex.set_shape(self.species, self.form, self.shape));
            match self.pokedex.as_ref() {
                Some(pokedex) if placed => {
                    result = sd.write(pokedex_data::SD_PATH, &pokedex.to_bytes()).and_then(|_| sd.mark_changed(pokedex_data::SD_PATH));
                    shape_saved = result.is_ok();
                }
                _ => result = Err("The shape has no place in the Pokédex data.".into()),
            }
        }
        match result {
            Ok(()) => {
                if shape_saved {
                    self.shape_baseline = self.shape;
                    self.pokedex_from_sd = true;
                }
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
#[cfg(test)]
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

const MALE_BLUE: egui::Color32 = egui::Color32::from_rgb(59, 130, 246);
const FEMALE_PINK: egui::Color32 = egui::Color32::from_rgb(236, 72, 153);

#[cfg(test)]
fn height_text(cm: u16) -> String {
    let m = cm as f32 / 100.0;
    let total_inches = cm as f32 / 2.54;
    let mut feet = (total_inches / 12.0).floor() as i32;
    let mut inches = (total_inches % 12.0).round() as i32;
    if inches == 12 {
        feet += 1;
        inches = 0;
    }
    format!("{m:.2} m / {cm} cm ({feet}'{inches:02}\")")
}

#[cfg(test)]
fn weight_text(hectograms: u16) -> String {
    let kg = hectograms as f32 / 10.0;
    let lbs = kg * 2.2046226;
    format!("{kg:.1} kg ({lbs:.1} lbs)")
}

fn gender_widget(ui: &mut egui::Ui, value: &mut u8) {
    ui.spacing_mut().item_spacing.x = 6.0;

    let bar_width = 110.0;
    let bar_height = 12.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(bar_width, 24.0), egui::Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(bar_width, bar_height));
    let track_rounding = egui::Rounding::same(bar_height / 2.0);

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
            let snap_values: [(f32, u8); 7] = [
                (0.0, 254),           // 100% ♀, 0% ♂
                (32.0 / 256.0, 223),  // 87.5% ♀, 12.5% ♂
                (64.0 / 256.0, 191),  // 75% ♀, 25% ♂
                (128.0 / 256.0, 127), // 50% ♀, 50% ♂
                (192.0 / 256.0, 63),  // 25% ♀, 75% ♂
                (224.0 / 256.0, 31),  // 12.5% ♀, 87.5% ♂
                (1.0, 0),             // 0% ♀, 100% ♂
            ];
            let mut snapped = None;
            for (snap_r, snap_val) in snap_values {
                if (ratio - snap_r).abs() < 0.04 {
                    snapped = Some(snap_val);
                    break;
                }
            }
            *value = snapped.unwrap_or_else(|| {
                let female_ratio = 1.0 - ratio;
                (female_ratio * 256.0 - 1.0).round().clamp(0.0, 254.0) as u8
            });
        }
    }

    ui.painter().rect_filled(track_rect, track_rounding, color::SURFACE_STRONG);

    let (male_pct, female_pct) = match *value {
        0 => (100.0, 0.0),
        254 => (0.0, 100.0),
        255 => (0.0, 0.0),
        _ => {
            let f = (*value as f32 + 1.0) * 100.0 / 256.0;
            (100.0 - f, f)
        }
    };

    if *value == 255 {
        ui.painter().rect_filled(track_rect, track_rounding, color::MUTED.linear_multiply(0.35));
    } else {
        let male_w = (male_pct / 100.0 * track_rect.width()).round();
        let female_w = track_rect.width() - male_w;

        if male_w > 0.0 {
            let male_rect = egui::Rect::from_min_size(track_rect.min, egui::vec2(male_w, track_rect.height()));
            let r = if female_w <= 0.0 {
                track_rounding
            } else {
                egui::Rounding { nw: bar_height / 2.0, sw: bar_height / 2.0, ne: 0.0, se: 0.0 }
            };
            ui.painter().rect_filled(male_rect, r, MALE_BLUE);
        }

        if female_w > 0.0 {
            let female_rect = egui::Rect::from_min_size(
                egui::pos2(track_rect.min.x + male_w, track_rect.min.y),
                egui::vec2(female_w, track_rect.height()),
            );
            let r = if male_w <= 0.0 {
                track_rounding
            } else {
                egui::Rounding { nw: 0.0, sw: 0.0, ne: bar_height / 2.0, se: bar_height / 2.0 }
            };
            ui.painter().rect_filled(female_rect, r, FEMALE_PINK);
        }

        let handle_x = (track_rect.left() + male_w).clamp(track_rect.left(), track_rect.right());
        let handle_center = egui::pos2(handle_x, track_rect.center().y);
        let radius = 6.0;

        if response.hovered() || response.dragged() {
            ui.painter().circle_filled(handle_center, radius + 3.0, egui::Color32::from_black_alpha(35));
        }
        ui.painter().circle_filled(handle_center + egui::vec2(0.0, 1.0), radius + 0.5, egui::Color32::from_black_alpha(25));
        ui.painter().circle_filled(handle_center, radius, color::WHITE);
        let border_color = if male_pct > 50.0 {
            MALE_BLUE
        } else if female_pct > 50.0 {
            FEMALE_PINK
        } else {
            color::ACCENT
        };
        ui.painter().circle_stroke(handle_center, radius, egui::Stroke::new(2.0_f32, border_color));
    }

    if *value == 255 {
        ui.label(egui::RichText::new("Genderless").color(color::MUTED).strong());
        if ui.button("Binary (50/50)").clicked() {
            *value = 127;
        }
    } else {
        ui.label(egui::RichText::new(format!("♂ {}%", trim(male_pct))).color(MALE_BLUE).strong().size(12.5));
        ui.label(egui::RichText::new(format!("♀ {}%", trim(female_pct))).color(FEMALE_PINK).strong().size(12.5));

        if ui.small_button("Genderless").on_hover_text("Genderless / gender-unknown (255)").clicked() {
            *value = 255;
        }
    }

    ui.label(widget::hint(&format!("0x{:02X}", *value)));
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HeightUnit {
    #[default]
    Meters,
    Centimeters,
    Inches,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum WeightUnit {
    #[default]
    Kg,
    Lbs,
}

fn height_field(ui: &mut egui::Ui, id_salt: &str, cm_value: &mut u16) {
    let unit_id = ui.make_persistent_id(id_salt).with("unit");
    let mut unit: HeightUnit = ui.data_mut(|d| d.get_temp(unit_id).unwrap_or_default());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match unit {
            HeightUnit::Meters => {
                let mut val_m = *cm_value as f64 / 100.0;
                let r = ui.add(
                    egui::DragValue::new(&mut val_m)
                        .speed(0.01)
                        .range(0.0..=655.35)
                        .fixed_decimals(2),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = (val_m * 100.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
            HeightUnit::Centimeters => {
                let mut val_cm = *cm_value as f64;
                let r = ui.add(
                    egui::DragValue::new(&mut val_cm)
                        .speed(1.0)
                        .range(0.0..=65535.0)
                        .fixed_decimals(0),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = val_cm.round().clamp(0.0, 65535.0) as u16;
                }
            }
            HeightUnit::Inches => {
                let mut val_in = *cm_value as f64 / 2.54;
                let r = ui.add(
                    egui::DragValue::new(&mut val_in)
                        .speed(0.5)
                        .range(0.0..=25800.0)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *cm_value = (val_in * 2.54).round().clamp(0.0, 65535.0) as u16;
                }
            }
        }

        let unit_label = match unit {
            HeightUnit::Meters => "m",
            HeightUnit::Centimeters => "cm",
            HeightUnit::Inches => "in",
        };
        let mut new_unit = unit;
        egui::ComboBox::from_id_salt(unit_id)
            .selected_text(unit_label)
            .width(42.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_unit, HeightUnit::Meters, "m");
                ui.selectable_value(&mut new_unit, HeightUnit::Centimeters, "cm");
                ui.selectable_value(&mut new_unit, HeightUnit::Inches, "in");
            });
        if new_unit != unit {
            unit = new_unit;
            ui.data_mut(|d| d.insert_temp(unit_id, unit));
        }

        let total_inches = *cm_value as f32 / 2.54;
        let mut feet = (total_inches / 12.0).floor() as i32;
        let mut inches = (total_inches % 12.0).round() as i32;
        if inches == 12 {
            feet += 1;
            inches = 0;
        }

        let hint = match unit {
            HeightUnit::Meters => format!("({feet}'{inches:02}\") · 0x{:04X}", *cm_value),
            HeightUnit::Centimeters => format!("{:.2} m ({feet}'{inches:02}\") · 0x{:04X}", *cm_value as f32 / 100.0, *cm_value),
            HeightUnit::Inches => format!("{:.2} m / {} cm · 0x{:04X}", *cm_value as f32 / 100.0, *cm_value, *cm_value),
        };
        ui.label(widget::hint(&hint));
    });
}

fn weight_field(ui: &mut egui::Ui, id_salt: &str, hg_value: &mut u16) {
    let unit_id = ui.make_persistent_id(id_salt).with("unit");
    let mut unit: WeightUnit = ui.data_mut(|d| d.get_temp(unit_id).unwrap_or_default());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        match unit {
            WeightUnit::Kg => {
                let mut val_kg = *hg_value as f64 / 10.0;
                let r = ui.add(
                    egui::DragValue::new(&mut val_kg)
                        .speed(0.1)
                        .range(0.0..=6553.5)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *hg_value = (val_kg * 10.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
            WeightUnit::Lbs => {
                let mut val_lbs = (*hg_value as f64 / 10.0) * 2.2046226;
                let r = ui.add(
                    egui::DragValue::new(&mut val_lbs)
                        .speed(0.5)
                        .range(0.0..=14448.0)
                        .fixed_decimals(1),
                );
                widget::register_focusable(ui, r.id, r.rect);
                if r.changed() {
                    *hg_value = (val_lbs / 2.2046226 * 10.0).round().clamp(0.0, 65535.0) as u16;
                }
            }
        }

        let unit_label = match unit {
            WeightUnit::Kg => "kg",
            WeightUnit::Lbs => "lbs",
        };
        let mut new_unit = unit;
        egui::ComboBox::from_id_salt(unit_id)
            .selected_text(unit_label)
            .width(46.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_unit, WeightUnit::Kg, "kg");
                ui.selectable_value(&mut new_unit, WeightUnit::Lbs, "lbs");
            });
        if new_unit != unit {
            unit = new_unit;
            ui.data_mut(|d| d.insert_temp(unit_id, unit));
        }

        let kg = *hg_value as f32 / 10.0;
        let lbs = kg * 2.2046226;
        let hint = match unit {
            WeightUnit::Kg => format!("({lbs:.1} lbs) · raw: {} (0x{:04X})", *hg_value, *hg_value),
            WeightUnit::Lbs => format!("({kg:.1} kg) · raw: {} (0x{:04X})", *hg_value, *hg_value),
        };
        ui.label(widget::hint(&hint));
    });
}

fn searchable_picker<T>(
    ui: &mut egui::Ui,
    id_salt: &str,
    value: &mut T,
    names: &[&str],
    button_width: f32,
) where
    T: Copy + Into<usize> + TryFrom<usize>,
{
    let current_id: usize = (*value).into();
    let label = if current_id == 0 {
        "#000 None".to_string()
    } else if let Some(&name) = names.get(current_id.saturating_sub(1)) {
        format!("#{:03} {}", current_id, name)
    } else {
        format!("#{:03} Unknown", current_id)
    };

    let picker_id = ui.make_persistent_id(id_salt);
    let popup_id = picker_id.with("popup");
    let is_popup_open = ui.memory(|m| m.is_popup_open(popup_id));

    let button_height = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(button_width, button_height), egui::Sense::hover());
    let button_response = ui.interact(rect, picker_id, egui::Sense::click());

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
        let galley = egui::WidgetText::from(&label).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            button_width - 24.0,
            egui::TextStyle::Button,
        );
        let text_rect = rect.shrink2(ui.spacing().button_padding);
        let text_pos = egui::Align2::LEFT_CENTER.align_size_within_rect(galley.size(), text_rect).min;
        ui.painter().galley(text_pos, galley, visuals.text_color());

        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 12.0, rect.center().y),
            egui::vec2(7.0, 4.0),
        );
        ui.painter().add(egui::Shape::convex_polygon(
            vec![icon_rect.left_top(), icon_rect.right_top(), icon_rect.center_bottom()],
            visuals.fg_stroke.color,
            egui::Stroke::NONE,
        ));
    }

    if button_response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        widget::set_active_id(ui, picker_id);
    }
    widget::register_focusable(ui, picker_id, button_response.rect);

    let search_id = picker_id.with("search");
    let mut search_text: String = ui.data_mut(|d| d.get_temp(search_id).unwrap_or_default());

    let popup_res = egui::popup::popup_below_widget(
        ui,
        popup_id,
        &button_response,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_width(button_width.max(220.0));
            ui.set_max_width(button_width.max(240.0));
            ui.spacing_mut().item_spacing.y = 4.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(widget::hint("🔍"));
                let search_input = ui.add(
                    egui::TextEdit::singleline(&mut search_text)
                        .hint_text("Search name or #...")
                        .desired_width(button_width.max(220.0) - 50.0)
                        .clip_text(true),
                );
                let focused_id = picker_id.with("focused");
                let has_focused: bool = ui.data_mut(|d| d.get_temp(focused_id).unwrap_or(false));
                if !has_focused {
                    search_input.request_focus();
                    ui.data_mut(|d| d.insert_temp(focused_id, true));
                }
                if !search_text.is_empty() && ui.small_button("✕").clicked() {
                    search_text.clear();
                }
            });
            ui.separator();

            let query = search_text.trim();
            let clean = |s: &str| s.to_lowercase().replace(['é', 'è'], "e");
            let clean_q = clean(query);
            let num_q = query.trim_start_matches('#').trim_start_matches('0');

            let mut matches: Vec<usize> = Vec::new();
            if clean_q.is_empty()
                || "none".starts_with(&clean_q)
                || query == "0"
                || query == "#0"
                || query == "#00"
                || query == "#000"
                || (num_q.is_empty() && query.starts_with('#'))
            {
                matches.push(0);
            }
            for (idx, &name) in names.iter().enumerate() {
                let id = idx + 1;
                let c_name = clean(name);
                let num_str = id.to_string();
                let formatted = format!("#{:03}", id);
                let is_match = clean_q.is_empty()
                    || c_name.contains(&clean_q)
                    || formatted.contains(&clean_q)
                    || (!num_q.is_empty() && (num_str == num_q || num_str.starts_with(num_q)));
                if is_match {
                    matches.push(id);
                }
            }

            if matches.is_empty() {
                ui.add_space(4.0);
                ui.label(widget::hint("No match found."));
            } else {
                let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

                if let Some(pos) = matches.iter().position(|&n| n == current_id) {
                    if down && pos + 1 < matches.len() {
                        if let Ok(new_val) = T::try_from(matches[pos + 1]) {
                            *value = new_val;
                        }
                    }
                    if up && pos > 0 {
                        if let Ok(new_val) = T::try_from(matches[pos - 1]) {
                            *value = new_val;
                        }
                    }
                } else if (up || down) && !matches.is_empty() {
                    if let Ok(new_val) = T::try_from(matches[0]) {
                        *value = new_val;
                    }
                }

                if enter && !matches.is_empty() {
                    if !matches.contains(&current_id) {
                        if let Ok(new_val) = T::try_from(matches[0]) {
                            *value = new_val;
                        }
                    }
                    ui.memory_mut(|m| m.close_popup());
                }

                let scroll_key = picker_id.with("scrolled");
                let has_scrolled: bool = ui.data_mut(|d| d.get_temp(scroll_key).unwrap_or(false));

                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for &id in &matches {
                        let chosen = current_id == id;
                        let text = if id == 0 {
                            "#000 None".to_string()
                        } else if let Some(&name) = names.get(id - 1) {
                            format!("#{:03} {}", id, name)
                        } else {
                            format!("#{:03} Unknown", id)
                        };
                        let row = ui.selectable_label(chosen, text);
                        if chosen {
                            widget::glow(ui, row.rect, 4.0);
                            if (up || down) || !has_scrolled {
                                row.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                        if row.clicked() {
                            if let Ok(new_val) = T::try_from(id) {
                                *value = new_val;
                            }
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                });
                ui.data_mut(|d| d.insert_temp(scroll_key, true));
            }
        },
    );

    if popup_res.is_some() {
        ui.data_mut(|d| d.insert_temp(search_id, search_text));
    } else {
        ui.data_mut(|d| {
            d.remove::<String>(search_id);
            d.remove::<bool>(picker_id.with("focused"));
            d.remove::<bool>(picker_id.with("scrolled"));
        });
    }
}

/// A number field with arrow buttons: the up and down keys work too.
fn number<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T, max: f64) {
    widget::spin(ui, value, 0.0, max);
}

struct FieldNote {
    summary: &'static str,
    url: &'static str,
}

/// A line of the grid: field label with an optional Bulbapedia info note, then its widgets.
fn line_with_note(ui: &mut egui::Ui, label: &str, note: Option<FieldNote>, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(widget::hint(label));
        if let Some(n) = note {
            let info = ui.hyperlink_to("ℹ", n.url);
            info.on_hover_ui(|ui| {
                ui.set_max_width(280.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(egui::RichText::new(label).strong());
                ui.label(n.summary);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.label(widget::hint("Learn more on"));
                    ui.hyperlink_to("Bulbapedia ↗", n.url);
                });
            });
        }
    });
    ui.horizontal(add);
    ui.end_row();
}

/// A line of the grid: the name of the field, then its widgets.
#[allow(dead_code)]
fn line(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    line_with_note(ui, label, None, add);
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
    widget::register_focusable(ui, menu_id, combo.response.rect);
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
const COLUMN_WIDTH: f32 = 470.0;

/// A grid for the fields of a group.
fn grid(name: &'static str) -> egui::Grid {
    egui::Grid::new(name).num_columns(2).spacing([16.0, 8.0])
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

fn stat_line(ui: &mut egui::Ui, label: &str, summary: &'static str, value: &mut u8) {
    line_with_note(
        ui,
        label,
        Some(FieldNote {
            summary,
            url: "https://bulbapedia.bulbagarden.net/wiki/Stat#Base_stat_values",
        }),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            widget::stat_bar(ui, value, 255.0);
            number(ui, value, 255.0);
            ui.label(widget::hint(&format!("0x{:02X}", *value)));
        },
    );
}

fn group_statistics(ui: &mut egui::Ui, d: &mut SpeciesData, _shape: &mut u8) {
    group(ui, "Base stats", |ui| {
        grid("stats").show(ui, |ui| {
            stat_line(ui, "HP", "Hit Points (HP). Amount of damage this Pokémon can withstand before fainting.", &mut d.base_hp);
            stat_line(ui, "Attack", "Physical Attack. Governs direct damage dealt (e.g. Slash, Earthquake) against enemy Defense.", &mut d.base_attack);
            stat_line(ui, "Defense", "Physical Defense. Reduces damage taken from enemy physical attacks.", &mut d.base_defense);
            stat_line(ui, "Special Attack", "Special Attack. Governs special move damage dealt (e.g. Surf, Flamethrower) against enemy Special Defense.", &mut d.base_special_attack);
            stat_line(ui, "Special Defense", "Special Defense. Reduces damage taken from enemy special attacks.", &mut d.base_special_defense);
            stat_line(ui, "Speed", "Speed. Determines turn order: the Pokémon with higher Speed acts first.", &mut d.base_speed);
        });

        ui.add_space(10.0);
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(3, |columns| {
                    columns[0].vertical_centered(|ui| {
                        ui.label(widget::hint("Total"));
                        ui.label(egui::RichText::new(format!("{}", d.base_total())).strong().size(15.0));
                    });
                    columns[1].vertical_centered(|ui| {
                        ui.label(widget::hint("Average"));
                        ui.label(egui::RichText::new(format!("{:.1}", d.base_average())).strong().size(15.0));
                    });
                    columns[2].vertical_centered(|ui| {
                        ui.label(widget::hint("Std Dev"));
                        ui.label(egui::RichText::new(format!("{:.1}", d.base_standard_deviation())).strong().size(15.0));
                    });
                });
            });
    });
}

fn group_characteristics(ui: &mut egui::Ui, d: &mut SpeciesData, shape: &mut u8) {
    group(ui, "Characteristics", |ui| {
        grid("characteristics").show(ui, |ui| {
            line_with_note(
                ui,
                "Type 1",
                Some(FieldNote {
                    summary: "Primary elemental type defining offensive and defensive effectiveness, plus STAB bonus (+50%).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Type",
                }),
                |ui| {
                    menu(ui, "type_1", &mut d.type_1, &names::TYPE, 130.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.type_1)));
                },
            );
            line_with_note(
                ui,
                "Type 2",
                Some(FieldNote {
                    summary: "Secondary elemental type (optional for dual-type Pokémon).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Type",
                }),
                |ui| {
                    menu(ui, "type_2", &mut d.type_2, &names::TYPE, 130.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.type_2)));
                },
            );
            line_with_note(
                ui,
                "Ability 1",
                Some(FieldNote {
                    summary: "First standard passive ability providing a unique battle or field effect.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                }),
                |ui| {
                    searchable_picker(ui, "ability_1", &mut d.ability_1, &names::ABILITY, 180.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.ability_1)));
                },
            );
            line_with_note(
                ui,
                "Ability 2",
                Some(FieldNote {
                    summary: "Second standard passive ability possible for this species (if any).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                }),
                |ui| {
                    searchable_picker(ui, "ability_2", &mut d.ability_2, &names::ABILITY, 180.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.ability_2)));
                },
            );
            line_with_note(
                ui,
                "Hidden ability",
                Some(FieldNote {
                    summary: "Rare hidden ability, typically obtainable through special encounters, Friend Safari, or hordes.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Ability",
                }),
                |ui| {
                    searchable_picker(ui, "ability_hidden", &mut d.ability_hidden, &names::ABILITY, 180.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.ability_hidden)));
                },
            );
            line_with_note(
                ui,
                "Egg Group 1",
                Some(FieldNote {
                    summary: "Primary egg group determining breeding compatibility at the Pokémon Day Care.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_Group",
                }),
                |ui| {
                    menu(ui, "egg_1", &mut d.egg_group_1, &names::EGG_GROUP, 140.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.egg_group_1)));
                },
            );
            line_with_note(
                ui,
                "Egg Group 2",
                Some(FieldNote {
                    summary: "Secondary egg group for Pokémon belonging to multiple breeding categories.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_Group",
                }),
                |ui| {
                    menu(ui, "egg_2", &mut d.egg_group_2, &names::EGG_GROUP, 140.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.egg_group_2)));
                },
            );
            line_with_note(
                ui,
                "Held Item 50%",
                Some(FieldNote {
                    summary: "Common held item present on 50% of wild Pokémon of this species upon encounter.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_wild_held_item",
                }),
                |ui| {
                    searchable_picker(ui, "item_50", &mut d.item_50, &names::ITEM, 180.0);
                    ui.label(widget::hint(&format!("0x{:04X}", d.item_50)));
                },
            );
            line_with_note(
                ui,
                "Held Item 5%",
                Some(FieldNote {
                    summary: "Rare held item present on 5% of wild Pokémon of this species upon encounter.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_wild_held_item",
                }),
                |ui| {
                    searchable_picker(ui, "item_5", &mut d.item_5, &names::ITEM, 180.0);
                    ui.label(widget::hint(&format!("0x{:04X}", d.item_5)));
                },
            );
            line_with_note(
                ui,
                "Leveling rate",
                Some(FieldNote {
                    summary: "Growth rate determining the total experience points required to reach level 100 (e.g. 1,000,000 for Medium Fast).",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Experience#Experience_total_by_growth_rate",
                }),
                |ui| {
                    menu(ui, "leveling_rate", &mut d.leveling_rate, &names::LEVELING_RATE, 165.0);
                    let exp_at_100 = match d.leveling_rate {
                        0 => "1,000,000 exp",
                        1 => "600,000 exp",
                        2 => "1,640,000 exp",
                        3 => "1,059,860 exp",
                        4 => "800,000 exp",
                        5 => "1,250,000 exp",
                        6 | 7 => "1,000,000 exp",
                        _ => "unknown",
                    };
                    ui.label(widget::hint(&format!("{exp_at_100} · 0x{:02X}", d.leveling_rate)));
                },
            );
            line_with_note(
                ui,
                "Shape",
                Some(FieldNote {
                    summary: "Physical body shape (quadruped, winged, serpentine...) used for form sorting in the Pokédex.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_shape",
                }),
                |ui| {
                    menu(ui, "shape", shape, &names::SHAPE, 190.0);
                    ui.label(widget::hint(&format!("0x{:02X}", *shape)));
                },
            );
            line_with_note(
                ui,
                "Color",
                Some(FieldNote {
                    summary: "Official dominant color used for visual search in the Pokédex.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_color",
                }),
                |ui| {
                    menu(ui, "pokedex_color", &mut d.color, &names::COLOR, 130.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.color)));
                },
            );
            line_with_note(
                ui,
                "Gender ratio",
                Some(FieldNote {
                    summary: "Probability of being male or female. 0 = 100% ♂, 254 = 100% ♀, 255 = Genderless.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Gender",
                }),
                |ui| {
                    gender_widget(ui, &mut d.gender_ratio);
                },
            );
            line_with_note(
                ui,
                "Hatch time",
                Some(FieldNote {
                    summary: "Number of egg cycles required to hatch. In Gen VI, one cycle equals 256 steps.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Egg_cycle",
                }),
                |ui| {
                    number(ui, &mut d.hatch_time, 255.0);
                    let steps = d.hatch_time as u32 * 256;
                    ui.label(widget::hint(&format!("{} cycles ({} steps) · 0x{:02X}", d.hatch_time, steps, d.hatch_time)));
                },
            );
            line_with_note(
                ui,
                "Height",
                Some(FieldNote {
                    summary: "Pokémon height from head to toe.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_height",
                }),
                |ui| {
                    height_field(ui, "height", &mut d.height);
                },
            );
            line_with_note(
                ui,
                "Weight",
                Some(FieldNote {
                    summary: "Pokémon body weight. Determines damage dealt or taken by Heavy Slam, Heat Crash, Grass Knot, and Low Kick.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/List_of_Pok%C3%A9mon_by_weight",
                }),
                |ui| {
                    weight_field(ui, "weight", &mut d.weight);
                },
            );
            line_with_note(
                ui,
                "Catch rate",
                Some(FieldNote {
                    summary: "Catch rate (3 to 255). Higher values make the Pokémon easier to catch with a Poké Ball.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Catch_rate",
                }),
                |ui| {
                    number(ui, &mut d.catch_rate, 255.0);
                    ui.label(widget::hint(&format!("{} · 0x{:02X}", percent(d.catch_rate), d.catch_rate)));
                },
            );
            line_with_note(
                ui,
                "Base friendship",
                Some(FieldNote {
                    summary: "Initial friendship value (usually 70) upon catch or hatch. Requires 220 to evolve by friendship.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Friendship",
                }),
                |ui| {
                    number(ui, &mut d.base_friendship, 255.0);
                    ui.label(widget::hint(&format!("0x{:02X}", d.base_friendship)));
                },
            );
            line_with_note(
                ui,
                "Base experience",
                Some(FieldNote {
                    summary: "Base experience yield determining XP awarded to the party upon defeating this Pokémon.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Experience",
                }),
                |ui| {
                    number(ui, &mut d.base_experience, 65535.0);
                    ui.label(widget::hint(&format!("0x{:04X}", d.base_experience)));
                },
            );
            line_with_note(
                ui,
                "Burrowed entry",
                Some(FieldNote {
                    summary: "Specifies whether the Pokémon emerges directly burrowed in the ground upon entering battle (Diglett and Dugtrio), skipping the standard mid-air Poké Ball drop animation.",
                    url: "https://bulbapedia.bulbagarden.net/wiki/Diglett_(Pok%C3%A9mon)#Trivia",
                }),
                |ui| {
                    let is_burrowed = d.is_burrowed_entry();
                    if ui.selectable_label(is_burrowed, "Yes").clicked() {
                        d.set_burrowed_entry(true);
                    }
                    if ui.selectable_label(!is_burrowed, "No").clicked() {
                        d.set_burrowed_entry(false);
                    }
                    let bit_val = if is_burrowed { 1 } else { 0 };
                    ui.label(widget::hint(&format!("· bit 12: {bit_val}")));
                },
            );
        });
    });
}

fn ev_graduated_bar(ui: &mut egui::Ui, value: &mut u8) {
    let width = 110.0;
    let height = 24.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let track_rect = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, 4.0), egui::vec2(width - 16.0, 6.0));
    let track_rounding = egui::Rounding::same(3.0);

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
            *value = (ratio * 3.0).round().clamp(0.0, 3.0) as u8;
        }
    }

    ui.painter().rect_filled(track_rect, track_rounding, color::SURFACE_STRONG);

    if *value > 0 {
        let fill_w = (*value as f32 / 3.0) * track_rect.width();
        let fill_rect = egui::Rect::from_min_size(track_rect.min, egui::vec2(fill_w, track_rect.height()));
        ui.painter().rect_filled(fill_rect, track_rounding, color::ACCENT);
    }

    let step_x = |s: u8| track_rect.left() + (s as f32 / 3.0) * track_rect.width();
    for s in 0..=3 {
        let x = step_x(s);
        let dot_color = if s <= *value { color::WHITE } else { color::MUTED.linear_multiply(0.6) };
        ui.painter().circle_filled(egui::pos2(x, track_rect.center().y), 2.0, dot_color);

        let is_selected = s == *value;
        let num_color = if is_selected { color::ACCENT } else { color::MUTED };
        ui.painter().text(
            egui::pos2(x, track_rect.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{s}"),
            egui::FontId::proportional(11.0),
            num_color,
        );
    }

    let handle_center = egui::pos2(step_x(*value), track_rect.center().y);
    let radius = 5.5;
    if response.hovered() || response.dragged() {
        ui.painter().circle_filled(handle_center, radius + 3.0, color::ACCENT.linear_multiply(0.25));
    }
    ui.painter().circle_filled(handle_center + egui::vec2(0.0, 1.0), radius + 0.5, egui::Color32::from_black_alpha(20));
    ui.painter().circle_filled(handle_center, radius, color::WHITE);
    ui.painter().circle_stroke(handle_center, radius, egui::Stroke::new(2.0_f32, color::ACCENT));

    ui.label(
        egui::RichText::new(format!("+{value}"))
            .color(if *value > 0 { color::ACCENT } else { color::MUTED })
            .strong()
            .size(13.5),
    );
}

fn ev_line(ui: &mut egui::Ui, label: &str, summary: &'static str, mut value: u8, mut set: impl FnMut(u8)) {
    line_with_note(
        ui,
        label,
        Some(FieldNote {
            summary,
            url: "https://bulbapedia.bulbagarden.net/wiki/Effort_values",
        }),
        |ui| {
            let before = value;
            ev_graduated_bar(ui, &mut value);
            if value != before {
                set(value);
            }
        },
    );
}

fn group_ev_yield(ui: &mut egui::Ui, d: &mut SpeciesData, _shape: &mut u8) {
    group(ui, "EV yield", |ui| {
        grid("ev_yield").show(ui, |ui| {
            ev_line(ui, "HP", "Effort Values (EVs) in HP awarded to the victor (0 to 3).", d.ev_hp(), |v| d.set_ev_hp(v));
            ev_line(ui, "Attack", "Effort Values (EVs) in Attack awarded to the victor (0 to 3).", d.ev_attack(), |v| d.set_ev_attack(v));
            ev_line(ui, "Defense", "Effort Values (EVs) in Defense awarded to the victor (0 to 3).", d.ev_defense(), |v| d.set_ev_defense(v));
            ev_line(ui, "Special Attack", "Effort Values (EVs) in Special Attack awarded to the victor (0 to 3).", d.ev_special_attack(), |v| d.set_ev_special_attack(v));
            ev_line(ui, "Special Defense", "Effort Values (EVs) in Special Defense awarded to the victor (0 to 3).", d.ev_special_defense(), |v| d.set_ev_special_defense(v));
            ev_line(ui, "Speed", "Effort Values (EVs) in Speed awarded to the victor (0 to 3).", d.ev_speed(), |v| d.set_ev_speed(v));
        });

        ui.add_space(8.0);
        let total = d.ev_total();
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(widget::hint("Total"));
                    ui.label(egui::RichText::new(format!("{total} EV{}", if total > 1 { "s" } else { "" })).strong());
                    ui.label(widget::hint(&format!("· raw: 0x{:04X}", d.ev_yield)));
                });
            });
    });
}

/// The groups of the General tab, side by side: as many columns as the window has room for.
fn general_tab(ui: &mut egui::Ui, d: &mut SpeciesData, shape: &mut u8) {
    const GROUPS: [fn(&mut egui::Ui, &mut SpeciesData, &mut u8); 3] = [
        group_statistics,
        group_characteristics,
        group_ev_yield,
    ];
    let columns = (((ui.available_width() + 10.0) / (COLUMN_WIDTH + 10.0)).floor() as usize).clamp(1, 2);
    let layout: Vec<Vec<usize>> = match columns {
        2 => vec![
            vec![0, 2], // Column 0: Base stats, EV yield
            vec![1], // Column 1: Characteristics
        ],
        _ => vec![(0..GROUPS.len()).collect()],
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for column in layout {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN_WIDTH);
                    for index in column {
                        GROUPS[index](ui, d, shape);
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
            self.pokedex_archive = Garc::open(&dump.archive(pokedex_data::ARCHIVE)).ok();
            self.loaded = None;
        }
        if self.garc.is_some() && self.loaded != Some((self.species, self.form)) {
            self.load(session);
        }
        self.top_bar(ui, session);
        ui.add_space(8.0);
        let mut clicked_tab = None;
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(12.0))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for (i, name) in TABS.iter().enumerate() {
                        let tab_id = egui::Id::new(format!("species_tab_{i}"));
                        let is_active_tab = widget::is_active(ui, tab_id);
                        if is_active_tab && self.tab != i {
                            self.tab = i;
                        }
                        let chosen = self.tab == i;
                        let r = widget::tab_button(ui, name, chosen, is_active_tab && widget::page_has_focus(ui));
                        widget::register_focusable(ui, tab_id, r.rect);
                        ui.ctx().data_mut(|d| d.insert_temp(tab_id.with("is_button"), true));
                        if r.clicked() {
                            clicked_tab = Some((i, tab_id));
                        }
                    }
                });
            });
        if let Some((i, tab_id)) = clicked_tab {
            self.tab = i;
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        }

        // Global tab-cycle shortcuts: Ctrl+PageDown / Ctrl+PageUp
        let (next_tab, prev_tab) = ui.input_mut(|i| {
            let next = (i.modifiers.ctrl || i.modifiers.command)
                && (i.consume_key(egui::Modifiers::CTRL, egui::Key::PageDown)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::PageDown));
            let prev = (i.modifiers.ctrl || i.modifiers.command)
                && (i.consume_key(egui::Modifiers::CTRL, egui::Key::PageUp)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::PageUp));
            (next, prev)
        });
        if next_tab && self.tab + 1 < TABS.len() {
            self.tab += 1;
            let tab_id = egui::Id::new(format!("species_tab_{}", self.tab));
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        } else if prev_tab && self.tab > 0 {
            self.tab -= 1;
            let tab_id = egui::Id::new(format!("species_tab_{}", self.tab));
            widget::set_active_id(ui, tab_id);
            ui.ctx().request_repaint();
        }
        ui.add_space(6.0);
        match (self.tab, self.data.as_mut()) {
            (0, Some(data)) => general_tab(ui, data, &mut self.shape),
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
        assert_eq!(match_species("#25")[0], 25);
        assert_eq!(match_species("#0025")[0], 25);
        assert_eq!(match_species("flabebe")[0], 669);
    }

    #[test]
    fn height_and_weight_formatting() {
        assert_eq!(height_text(100), "1.00 m / 100 cm (3'03\")");
        assert_eq!(height_text(170), "1.70 m / 170 cm (5'07\")");
        assert_eq!(weight_text(130), "13.0 kg (28.7 lbs)");
    }

    #[test]
    fn picker_labels_are_formatted() {
        let label = |id: usize, list: &[&str]| {
            if id == 0 {
                "#000 None".to_string()
            } else if let Some(&name) = list.get(id - 1) {
                format!("#{:03} {}", id, name)
            } else {
                format!("#{:03} Unknown", id)
            }
        };
        assert_eq!(label(0, &names::ABILITY), "#000 None");
        assert_eq!(label(1, &names::ABILITY), "#001 Stench");
        assert_eq!(label(65, &names::ABILITY), "#065 Overgrow");
        assert_eq!(label(0, &names::ITEM), "#000 None");
        assert_eq!(label(1, &names::ITEM), "#001 Master Ball");
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
                general_tab(ui, &mut data, &mut 0u8);
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
                    general_tab(ui, &mut data, &mut 0u8);
                });
            });
        }
    }

    #[test]
    fn gender_boundary_and_handle_position_align() {
        let compute_boundary = |value: u8, width: f32| -> f32 {
            let (male_pct, _) = match value {
                0 => (100.0, 0.0),
                254 => (0.0, 100.0),
                255 => (0.0, 0.0),
                _ => {
                    let f = (value as f32 + 1.0) * 100.0 / 256.0;
                    (100.0 - f, f)
                }
            };
            (male_pct / 100.0 * width).round()
        };

        let width = 110.0;
        // 100% male -> blue spans full width, handle at right boundary (110)
        assert_eq!(compute_boundary(0, width), 110.0);
        // 100% female -> blue width is 0, handle at left boundary (0)
        assert_eq!(compute_boundary(254, width), 0.0);
        // 50% / 50% -> handle at exactly half width (55)
        assert_eq!(compute_boundary(127, width), 55.0);
        // 87.5% male (starter like Bulbasaur, value 31) -> blue width is 87.5% of 110 = 96.0
        assert_eq!(compute_boundary(31, width), 96.0);
    }
}
