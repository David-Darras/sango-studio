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
const TABS: [&str; 7] = ["Data", "Movepool", "Evolution", "Mega evolution", "Form", "2D view", "3D view"];

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
}

impl Default for Species {
    fn default() -> Self {
        Species { root: None, garc: None, species: 1, form: 0, data: None, baseline: None, from_sd: false, loaded: None, tab: 0, status: None }
    }
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
                egui::ComboBox::from_id_salt("species_menu").selected_text(species_text).width(240.0).height(420.0).show_ui(ui, |ui| {
                    for number in 1..=SPECIES_COUNT {
                        ui.selectable_value(&mut self.species, number, format!("{number:>3}  {}", names::SPECIES[number - 1]));
                    }
                });
                let forms = self.form_count();
                self.form = self.form.min(forms - 1);
                let form_text = |form: usize| if form == 0 { "Form 0 (normal)".to_string() } else { format!("Form {form}") };
                egui::ComboBox::from_id_salt("form_menu").selected_text(form_text(self.form)).width(150.0).show_ui(ui, |ui| {
                    for form in 0..forms {
                        ui.selectable_value(&mut self.form, form, form_text(form));
                    }
                });
                reset = ui.button("Reset").on_hover_text("Read the data again from the romfs").clicked();
                let button = egui::Button::new(egui::RichText::new("Save").color(color::WHITE)).fill(color::ACCENT);
                save = ui.add_enabled(self.data.is_some(), button).on_hover_text("Write the file on the SD card").clicked();
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
    egui::ComboBox::from_id_salt(id).selected_text(name_from(list, *value as usize, 0)).width(width).show_ui(ui, |ui| {
        for (place, name) in list.iter().enumerate() {
            ui.selectable_value(value, place as u8, *name);
        }
    });
}

/// The form of the data: one card for each group of fields.
fn data_tab(ui: &mut egui::Ui, d: &mut SpeciesData) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let grid = |name: &'static str| egui::Grid::new(name).num_columns(2).spacing([24.0, 8.0]);
        widget::card(ui, |ui| {
            ui.label(widget::title(&format!("Base statistics (total {})", d.base_total())));
            grid("stats").show(ui, |ui| {
                line(ui, "HP", |ui| number(ui, &mut d.base_hp, 255.0));
                line(ui, "Attack", |ui| number(ui, &mut d.base_attack, 255.0));
                line(ui, "Defense", |ui| number(ui, &mut d.base_defense, 255.0));
                line(ui, "Special Attack", |ui| number(ui, &mut d.base_special_attack, 255.0));
                line(ui, "Special Defense", |ui| number(ui, &mut d.base_special_defense, 255.0));
                line(ui, "Speed", |ui| number(ui, &mut d.base_speed, 255.0));
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Types and abilities"));
            grid("types").show(ui, |ui| {
                line(ui, "Type 1", |ui| menu(ui, "type_1", &mut d.types[0], &names::TYPE, 120.0));
                line(ui, "Type 2", |ui| menu(ui, "type_2", &mut d.types[1], &names::TYPE, 120.0));
                line(ui, "Ability 1", |ui| ability(ui, &mut d.ability[0]));
                line(ui, "Ability 2", |ui| ability(ui, &mut d.ability[1]));
                line(ui, "Hidden ability", |ui| ability(ui, &mut d.ability[2]));
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Catch, rewards and breeding"));
            grid("catch").show(ui, |ui| {
                line(ui, "Catch rate", |ui| number(ui, &mut d.capture_rate, 255.0));
                line(ui, "Escape rate", |ui| number(ui, &mut d.escape_rate, 255.0));
                line(ui, "Base experience", |ui| number(ui, &mut d.give_experience, 65535.0));
                line(ui, "Effort values given", |ui| {
                    number(ui, &mut d.give_effort_values, 65535.0);
                    ui.label(widget::hint("two bits for each statistic"));
                });
                line(ui, "Item 1 (wild)", |ui| item(ui, &mut d.give_item[0]));
                line(ui, "Item 2 (wild)", |ui| item(ui, &mut d.give_item[1]));
                line(ui, "Item 3 (wild)", |ui| item(ui, &mut d.give_item[2]));
                line(ui, "Gender", |ui| {
                    number(ui, &mut d.gender, 255.0);
                    ui.label(widget::hint("0 male only, 254 female only, 255 no gender"));
                });
                line(ui, "Egg hatch cycles", |ui| number(ui, &mut d.egg_hatch_steps, 255.0));
                line(ui, "Base friendship", |ui| number(ui, &mut d.base_friendship, 255.0));
                line(ui, "Egg Group 1", |ui| menu(ui, "egg_1", &mut d.egg_group[0], &names::EGG_GROUP, 140.0));
                line(ui, "Egg Group 2", |ui| menu(ui, "egg_2", &mut d.egg_group[1], &names::EGG_GROUP, 140.0));
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Size and forms"));
            grid("size").show(ui, |ui| {
                line(ui, "Height", |ui| number(ui, &mut d.height, 65535.0));
                line(ui, "Fake height", |ui| number(ui, &mut d.fake_height, 65535.0));
                line(ui, "Weight (hectograms)", |ui| number(ui, &mut d.weight, 65535.0));
                line(ui, "Form count", |ui| number(ui, &mut d.form_count, 255.0));
                line(ui, "Form index", |ui| number(ui, &mut d.form_index, 65535.0));
                line(ui, "Form index 2", |ui| number(ui, &mut d.form_index_2, 65535.0));
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Technical and hidden machines (one bit each)"));
            grid("machines").show(ui, |ui| {
                for (i, word) in d.technical_moves.iter_mut().enumerate() {
                    line(ui, &format!("Bits {} to {}", i * 32, i * 32 + 31), |ui| {
                        ui.add(egui::DragValue::new(word).hexadecimal(8, false, true).speed(1.0));
                    });
                }
            });
        });
        ui.add_space(8.0);
        widget::card(ui, |ui| {
            ui.label(widget::title("Not described yet"));
            grid("unknown").show(ui, |ui| {
                line(ui, "Unknown 0", |ui| number(ui, &mut d.unknown_0, 255.0));
                line(ui, "Unknown 1", |ui| number(ui, &mut d.unknown_1, 255.0));
                line(ui, "Unknown 2", |ui| number(ui, &mut d.unknown_2, 255.0));
                line(ui, "Unknown 4", |ui| number(ui, &mut d.unknown_4, 65535.0));
                line(ui, "Last 16 bytes", |ui| {
                    ui.label(egui::RichText::new(d.rest.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")).monospace());
                });
            });
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
        ui.horizontal_wrapped(|ui| {
            for (i, name) in TABS.iter().enumerate() {
                ui.selectable_value(&mut self.tab, i, *name);
            }
        });
        ui.add_space(6.0);
        match (self.tab, self.data.as_mut()) {
            (0, Some(data)) => data_tab(ui, data),
            (0, None) => {}
            _ => {
                ui.label(widget::hint("..."));
            }
        }
    }
}
