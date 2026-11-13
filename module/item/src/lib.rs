//! The item editor.
//!
//! The page edits the data of one item (the file of the archive 197), its picture, and its texts in the eight
//! languages. The files that the user changes go on the SD card, in the folders `item/data` and `text`: the romfs is
//! never changed. Every field has a note: what it means, how the game reads it (from the code of the game), and a
//! page of Bulbapedia. A field that no code reads is not in the page: it is in the card "Not used by the game".

mod csv;
pub mod data;
mod icons;
mod pockets;
mod texts;
mod tools;

use data::{Def, Group, ItemData, COUNT, DEFS, UNUSED};
use sango_core::{Dump, Garc};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The tabs of the page.
const TABS: [&str; 2] = ["General", "Text"];

/// The width of one column of groups, in points.
const COLUMN_WIDTH: f32 = 470.0;

/// The items of the romfs, to show examples in the notes.
struct Index {
    /// The data of each item (the item 0 is empty).
    items: Vec<Option<ItemData>>,
    /// The English name of each item.
    names: Vec<String>,
}

impl Index {
    /// The names of some items that have the same value in a field, not counting the item that is open.
    fn examples(&self, def: &Def, value: i32, current: usize) -> (Vec<&str>, usize) {
        let same: Vec<usize> = (1..=COUNT).filter(|&n| n != current && self.items.get(n).and_then(|i| i.as_ref()).is_some_and(|i| def.get(&i.bytes) == value)).collect();
        let shown = same.iter().take(4).map(|&n| self.names.get(n).map_or("", |s| s.as_str())).collect();
        (shown, same.len().saturating_sub(4))
    }
}

pub struct Item {
    /// The dump that the archive comes from.
    root: Option<PathBuf>,
    garc: Option<Garc>,
    /// The archive of the battle pockets.
    pocket_garc: Option<Garc>,
    item: usize,
    /// The data of the item.
    data: Option<ItemData>,
    /// The data as it was loaded or saved: to know what changed.
    baseline: Option<ItemData>,
    /// The battle pockets of all the items (one byte each), as the user has them, and as they were loaded or saved.
    pockets: Vec<u8>,
    pockets_baseline: Vec<u8>,
    /// True when the data came from the SD card.
    from_sd: bool,
    /// The item that `data` is for.
    loaded: Option<usize>,
    tab: usize,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The names of the items in the list, as the user has them.
    list_names: Option<Vec<String>>,
    index: Option<Index>,
    /// False until the pictures of the SD card are read.
    icons_ready: bool,
    texts: texts::ItemTexts,
    icons: icons::ItemIcons,
    tools: tools::Tools,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            root: None,
            garc: None,
            pocket_garc: None,
            item: 1,
            data: None,
            baseline: None,
            pockets: Vec::new(),
            pockets_baseline: Vec::new(),
            from_sd: false,
            loaded: None,
            tab: 0,
            status: None,
            list_names: None,
            index: None,
            icons_ready: false,
            texts: texts::ItemTexts::default(),
            icons: icons::ItemIcons::default(),
            tools: tools::Tools::default(),
        }
    }
}

impl Item {
    /// Opens the archives of a dump.
    fn open(&mut self, dump: &Dump) {
        self.root = Some(dump.root().to_path_buf());
        self.garc = Garc::open(&dump.archive(data::ARCHIVE)).ok();
        self.pocket_garc = Garc::open(&dump.archive(pockets::ARCHIVE)).ok();
        self.texts.open(dump);
        self.icons.open(dump);
        self.icons_ready = false;
        sango_species::battle_icons::load(dump);
        self.list_names = None;
        self.loaded = None;
        let items = (0..=COUNT).map(|n| self.garc.as_ref().and_then(|g| g.first_file(n)).and_then(|b| ItemData::from_bytes(b).ok())).collect();
        let rom_names = self.texts.romfs(texts::ENGLISH, texts::FIELDS[0].file);
        let names = (0..=COUNT).map(|n| rom_names.as_ref().and_then(|f| f.text(n)).unwrap_or_default()).collect();
        self.index = Some(Index { items, names });
        if self.garc.is_none() {
            self.status = Some((false, "The archive of the items is not open.".into()));
        }
    }

    /// Reads the data of the chosen item: the SD card first, then the romfs.
    fn load(&mut self, session: &Session) {
        self.loaded = Some(self.item);
        self.pockets = pockets::current(self.pocket_garc.as_ref(), session.sd.as_ref());
        self.pockets_baseline = self.pockets.clone();
        let found = self.current(session.sd.as_ref(), self.item);
        self.from_sd = session.sd.as_ref().is_some_and(|sd| sd.read(&data::sd_path(self.item)).is_some());
        match found {
            Some(item) => {
                self.baseline = Some(item.clone());
                self.data = Some(item);
                self.status = None;
            }
            None => {
                self.data = None;
                self.baseline = None;
                self.status = Some((false, format!("The file {} is missing.", self.item)));
            }
        }
    }

    /// The data of an item as the user has it: the SD card first, then the romfs.
    fn current(&self, sd: Option<&sango_core::SdCard>, item: usize) -> Option<ItemData> {
        let on_card = sd.and_then(|sd| sd.read(&data::sd_path(item))).and_then(|b| ItemData::from_bytes(&b).ok());
        on_card.or_else(|| self.garc.as_ref().and_then(|g| g.first_file(item)).and_then(|b| ItemData::from_bytes(b).ok()))
    }

    /// The names of all the items for the list, read once.
    fn names(&mut self, session: &Session) -> Vec<String> {
        if self.list_names.is_none() {
            let sd = session.sd.as_ref();
            let names = (0..=COUNT).map(|n| self.texts.name(sd, n)).collect();
            self.list_names = Some(names);
        }
        self.list_names.clone().unwrap_or_default()
    }

    /// Writes the data and the texts on the SD card, and tells the plugin.
    fn save(&mut self, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let mut result: Result<(), String> = Ok(());
        if let Some(item) = &self.data {
            if self.baseline.as_ref() != Some(item) {
                let path = data::sd_path(self.item);
                result = sd.write(&path, &item.to_bytes()).and_then(|_| sd.mark_changed(&path));
            }
        }
        if result.is_ok() && self.pockets != self.pockets_baseline {
            result = pockets::write(sd, &self.pockets);
        }
        if result.is_ok() {
            result = self.texts.save(sd).map(|_| ());
        }
        if result.is_ok() {
            result = self.icons.save(sd);
        }
        match result {
            Ok(()) => {
                self.baseline = self.data.clone();
                self.pockets_baseline = self.pockets.clone();
                self.from_sd = true;
                self.list_names = None;
                self.status = Some((true, format!("Saved item {}. The files are on the SD card.", self.item)));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// True when something changed and was not saved.
    fn dirty(&self) -> bool {
        self.data != self.baseline || self.pockets != self.pockets_baseline || self.texts.changed() || self.icons.changed()
    }

    /// The rows of the item list: the names as the user has them, with the price and the pocket of the romfs.
    fn menu_rows(&mut self, session: &Session) -> Vec<sango_ui::item_menu::Row> {
        let names = self.names(session);
        (0..=COUNT)
            .map(|n| {
                let item = self.index.as_ref().and_then(|i| i.items.get(n)).and_then(|i| i.as_ref());
                sango_ui::item_menu::Row { name: if n == 0 { "None".to_string() } else { names.get(n).cloned().unwrap_or_default() }, pocket: item.and_then(|i| i.get("pocket")).unwrap_or(0) as u8, price: item.and_then(|i| i.get("price")).unwrap_or(0) as u32 * 10 }
            })
            .collect()
    }

    /// The picker, the buttons and the state.
    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let before = self.item;
        let mut save = false;
        let mut reset_tab = false;
        let mut reset_more = false;
        let mut undo = false;
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let rows = self.menu_rows(session);
                let icons = &mut self.icons;
                let mut chosen = self.item;
                sango_ui::item_menu::item_menu(ui, "item_menu", &mut chosen, 260.0, &rows, &mut |ctx, n| icons.texture(ctx, n), false);
                self.item = chosen.clamp(1, COUNT);
                if self.garc.is_some() {
                    ui.label(widget::hint(&format!("archive {}, file {}", data::ARCHIVE, self.item)));
                }
                let has_card = session.sd.is_some() && self.garc.is_some();
                ui.add_enabled_ui(has_card, |ui| {
                    reset_tab = widget::button(ui, "Reset tab");
                })
                .response
                .on_hover_text("Puts the data of this tab for this item back to the romfs. Undo gives it back.");
                ui.add_enabled_ui(has_card, |ui| {
                    reset_more = widget::button(ui, "Reset…");
                })
                .response
                .on_hover_text("Reset one item or all the items, for the parts that you choose.");
                let undo_label = self.tools.history.last().map(|backup| format!("Undo: {}", backup.label));
                ui.add_enabled_ui(undo_label.is_some() && session.sd.is_some(), |ui| {
                    undo = widget::button(ui, "Undo");
                })
                .response
                .on_hover_text(undo_label.unwrap_or_else(|| "Nothing to undo.".into()));
                save = self.data.is_some() && session.sd.is_some() && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
                if self.data.is_some() && self.dirty() {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if self.from_sd || self.texts.from_card() || self.icons.from_card() {
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
        if self.item != before {
            self.loaded = None;
        }
        if reset_tab {
            let part = if self.tab == 1 { tools::Part::Texts } else { tools::Part::Data };
            self.run_reset(session, tools::Scope::Item, &[part]);
        }
        if reset_more {
            self.tools.reset_open = true;
            self.tools.parts = [self.tab == 0, self.tab == 1, false];
        }
        if undo {
            self.undo(session);
        }
        if save {
            self.save(session);
        }
    }

    /// The definition of a field.
    fn def(key: &str) -> &'static Def {
        DEFS.iter().find(|d| d.key == key).expect("a field of the table")
    }

    /// The name of a field with its note: what it means, how the game reads it, examples, and Bulbapedia.
    fn label(&self, ui: &mut egui::Ui, item: &ItemData, def: &Def) {
        let current = self.item;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(widget::hint(def.label));
            let info = if def.url.is_empty() { ui.label(egui::RichText::new("ℹ").color(color::ACCENT)) } else { ui.hyperlink_to("ℹ", def.url) };
            info.on_hover_ui(|ui| self.note(ui, item, def, current));
        });
    }

    /// The text of the note of a field.
    fn note(&self, ui: &mut egui::Ui, item: &ItemData, def: &Def, current: usize) {
        ui.set_max_width(320.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(def.label).strong());
        ui.label(def.summary);
        ui.label(widget::hint(&format!("In the game: {}", def.code)));
        if let Some(index) = &self.index {
            let (shown, more) = index.examples(def, def.get(&item.bytes), current);
            if !shown.is_empty() {
                let more = if more > 0 { format!(" and {more} more") } else { String::new() };
                ui.label(widget::hint(&format!("Same value: {}{more}.", shown.join(", "))));
            }
        }
        if !def.url.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(widget::hint("Learn more on"));
                ui.hyperlink_to("Bulbapedia ↗", def.url);
            });
        }
    }

    /// A line of a grid: the name of the field, then what `add` draws for its value. The value is written back when it
    /// changed.
    fn line(&self, ui: &mut egui::Ui, item: &mut ItemData, key: &str, add: impl FnOnce(&mut egui::Ui, &mut i32, &Def)) {
        let def = Self::def(key);
        self.label(ui, item, def);
        ui.horizontal(|ui| {
            let mut value = def.get(&item.bytes);
            let before = value;
            add(ui, &mut value, def);
            if value != before {
                def.set(&mut item.bytes, value);
            }
        });
        ui.end_row();
    }

    /// A line with a bar and the number: for a value that has a usual range. The number can go beyond the bar.
    fn bar_line(&self, ui: &mut egui::Ui, item: &mut ItemData, key: &str, bar_min: i32, bar_max: i32) {
        self.line(ui, item, key, |ui, value, def| {
            widget::value_bar(ui, value, bar_min, bar_max);
            widget::spin(ui, value, def.min() as f64, def.max() as f64);
        });
    }

    /// A line with a bar for an amount that makes its flag: the flag is on when the amount is not 0.
    fn amount_line(&self, ui: &mut egui::Ui, item: &mut ItemData, key: &str, flag: &str, bar_min: i32, bar_max: i32) {
        let before = Self::def(key).get(&item.bytes);
        self.bar_line(ui, item, key, bar_min, bar_max);
        let now = Self::def(key).get(&item.bytes);
        if now != before {
            Self::def(flag).set(&mut item.bytes, (now != 0) as i32);
        }
    }

    /// A number that is a code of the game, with some items that have it.
    fn code_line(&self, ui: &mut egui::Ui, item: &mut ItemData, key: &str) {
        self.line(ui, item, key, |ui, value, def| {
            widget::spin(ui, value, def.min() as f64, def.max() as f64);
            if let Some(index) = &self.index {
                let (shown, more) = index.examples(def, *value, self.item);
                if !shown.is_empty() {
                    let more = if more > 0 { format!(" +{more}") } else { String::new() };
                    ui.label(widget::hint(&format!("{}{more}", shown.iter().take(2).copied().collect::<Vec<_>>().join(", "))));
                }
            }
        });
    }

    /// A choice in a list of names.
    fn choice_line(&self, ui: &mut egui::Ui, item: &mut ItemData, key: &str, list: &[&str]) {
        self.line(ui, item, key, |ui, value, def| {
            let text = list.get(*value as usize).map_or(format!("Value {value}"), |s| s.to_string());
            let combo = egui::ComboBox::from_id_salt(def.key).selected_text(text).width(140.0).show_ui(ui, |ui| {
                for (place, name) in list.iter().enumerate() {
                    if ui.selectable_label(*value == place as i32, *name).clicked() {
                        *value = place as i32;
                    }
                }
            });
            focus_combo(ui, &combo.response, value, list.len() as i32 - 1);
        });
    }

    /// A row of pills for flags.
    fn chips(&self, ui: &mut egui::Ui, item: &mut ItemData, keys: &[&str]) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for key in keys {
                let def = Self::def(key);
                let mut on = def.get(&item.bytes) != 0;
                let response = widget::chip(ui, def.label, &mut on);
                response.on_hover_ui(|ui| self.note(ui, item, def, self.item));
                def.set(&mut item.bytes, on as i32);
            }
        });
    }

    /// A card of a group: the title, what `add` draws, and the word "unused" for the fields of the group that no code reads.
    fn card(&self, ui: &mut egui::Ui, group: Group, add: impl FnOnce(&mut egui::Ui)) {
        widget::card(ui, |ui| {
            ui.set_min_width(COLUMN_WIDTH - 36.0);
            ui.label(widget::title(group.title()));
            ui.add_space(4.0);
            add(ui);
            for unused in UNUSED.iter().filter(|u| u.show == Some(group)) {
                ui.horizontal(|ui| {
                    ui.label(widget::hint(unused.label));
                    ui.label(widget::hint("unused"));
                });
            }
        });
        ui.add_space(10.0);
    }

    /// The group "Shop and Bag".
    fn shop_group(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        self.card(ui, Group::Shop, |ui| {
            egui::Grid::new("item_shop").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.line(ui, item, "price", |ui, value, def| {
                    widget::spin(ui, value, def.min() as f64, def.max() as f64);
                    ui.label(widget::hint(&format!("× 10 = {} Poké Dollars", *value * 10)));
                });
                self.choice_line(ui, item, "pocket", &data::POCKETS);
                self.choice_line(ui, item, "category", &data::CATEGORIES);
                self.line(ui, item, "sort_number", |ui, value, def| {
                    widget::spin(ui, value, def.min() as f64, def.max() as f64);
                });
            });
            ui.add_space(6.0);
            self.chips(ui, item, &["key_item", "registrable"]);
        });
    }

    /// The group "Held item".
    fn held_group(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        self.card(ui, Group::Held, |ui| {
            egui::Grid::new("item_held").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.code_line(ui, item, "hold_effect");
                self.bar_line(ui, item, "hold_power", 0, 100);
                self.code_line(ui, item, "fling_effect");
                self.bar_line(ui, item, "fling_power", 0, 130);
                self.bar_line(ui, item, "natural_gift_power", 0, 100);
                self.line(ui, item, "natural_gift_type", |ui, value, _| {
                    let mut kind = *value as u8;
                    type_picker(ui, "natural_gift_type", &mut kind);
                    *value = kind as i32;
                });
            });
            ui.add_space(6.0);
            self.chips(ui, item, &["consumed"]);
        });
    }

    /// The group "Use".
    fn use_group(&self, ui: &mut egui::Ui, item: &mut ItemData, pockets_table: &mut [u8]) {
        let item_number = self.item;
        self.card(ui, Group::Use, |ui| {
            egui::Grid::new("item_use").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.code_line(ui, item, "field_use");
                self.choice_line(ui, item, "battle_use", &data::BATTLE_USES);
            });
            ui.add_space(6.0);
            self.chips(ui, item, &["use_on_pokemon", "keep_when_used"]);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Pockets of the battle Bag").strong()).on_hover_text("The game reads the battle pockets in their own archive (one byte for each item), not in the file of the item. An item can be in two pockets.");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                let byte = self.pockets.get(item_number).copied().unwrap_or(0);
                let mut now = byte;
                for (bit, (name, text)) in pockets::BITS.iter().enumerate() {
                    let mut on = byte & (1 << bit) != 0;
                    widget::chip(ui, name, &mut on).on_hover_text(*text);
                    if on {
                        now |= 1 << bit;
                    } else {
                        now &= !(1 << bit);
                    }
                }
                if now != byte {
                    if let Some(slot) = pockets_table.get_mut(item_number) {
                        *slot = now;
                    }
                }
            });
        });
    }

    /// The group "Effects on a Pokémon".
    fn effects_group(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        self.card(ui, Group::Effects, |ui| {
            ui.label(egui::RichText::new("Cures").strong());
            self.chips(ui, item, &["cure_sleep", "cure_poison", "cure_burn", "cure_freeze", "cure_paralysis", "cure_confusion", "cure_infatuation"]);
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Also").strong());
            self.chips(ui, item, &["revive", "revive_all", "level_up", "evolve", "pp_up", "pp_max", "guard_stat_stages"]);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Stat stages in battle").strong());
            egui::Grid::new("item_stages").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for key in ["raise_attack", "raise_defense", "raise_sp_atk", "raise_sp_def", "raise_speed", "raise_accuracy"] {
                    self.bar_line(ui, item, key, 0, 6);
                }
                self.bar_line(ui, item, "raise_critical", 0, 3);
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Restores").strong());
            egui::Grid::new("item_restore").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                self.hp_line(ui, item);
                self.pp_line(ui, item);
            });
        });
    }

    /// The line of the HP that the item restores: the pill, the bar, the number and what the number means.
    fn hp_line(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        let hp = Self::def("hp_restore");
        let flag = Self::def("restore_hp");
        self.label(ui, item, hp);
        ui.horizontal(|ui| {
            let mut on = flag.get(&item.bytes) != 0;
            widget::chip(ui, "Restores HP", &mut on);
            flag.set(&mut item.bytes, on as i32);
            let mut value = hp.get(&item.bytes);
            let before = value;
            widget::value_bar(ui, &mut value, 0, 200);
            widget::spin(ui, &mut value, hp.min() as f64, hp.max() as f64);
            ui.label(widget::hint(match value {
                255 => "all the HP",
                254 => "half the HP",
                253 => "a quarter of the HP",
                _ => "HP",
            }));
            if value != before {
                hp.set(&mut item.bytes, value);
            }
        });
        ui.end_row();
    }

    /// The line of the PP that the item restores: one move or all the moves, the bar and the number.
    fn pp_line(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        const MODES: [&str; 3] = ["No PP", "One move", "All moves"];
        let pp = Self::def("pp_restore");
        self.label(ui, item, pp);
        ui.horizontal(|ui| {
            let mode = if Self::def("restore_all_pp").get(&item.bytes) != 0 { 2 } else { Self::def("restore_pp").get(&item.bytes) as usize };
            let mut chosen = mode;
            let combo = egui::ComboBox::from_id_salt("pp_mode").selected_text(MODES[mode]).width(100.0).show_ui(ui, |ui| {
                for (place, name) in MODES.iter().enumerate() {
                    if ui.selectable_label(chosen == place, *name).clicked() {
                        chosen = place;
                    }
                }
            });
            let mut stepped = chosen as i32;
            focus_combo(ui, &combo.response, &mut stepped, MODES.len() as i32 - 1);
            chosen = stepped as usize;
            if chosen != mode {
                Self::def("restore_pp").set(&mut item.bytes, (chosen == 1) as i32);
                Self::def("restore_all_pp").set(&mut item.bytes, (chosen == 2) as i32);
            }
            let mut value = pp.get(&item.bytes);
            let before = value;
            widget::value_bar(ui, &mut value, 0, 20);
            widget::spin(ui, &mut value, pp.min() as f64, pp.max() as f64);
            ui.label(widget::hint(if value == 127 { "all the PP" } else { "PP" }));
            if value != before {
                pp.set(&mut item.bytes, value);
            }
        });
        ui.end_row();
    }

    /// The group "Effort values".
    fn ev_group(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        self.card(ui, Group::Evs, |ui| {
            egui::Grid::new("item_evs").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (value, flag) in [("ev_hp_value", "ev_hp"), ("ev_attack_value", "ev_attack"), ("ev_defense_value", "ev_defense"), ("ev_sp_atk_value", "ev_sp_atk"), ("ev_sp_def_value", "ev_sp_def"), ("ev_speed_value", "ev_speed")] {
                    self.amount_line(ui, item, value, flag, -30, 30);
                }
            });
            ui.add_space(6.0);
            self.chips(ui, item, &["ev_limit_control"]);
        });
    }

    /// The group "Friendship".
    fn friendship_group(&self, ui: &mut egui::Ui, item: &mut ItemData) {
        self.card(ui, Group::Friendship, |ui| {
            egui::Grid::new("item_friendship").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (value, flag) in [("friendship_1_value", "friendship_1"), ("friendship_2_value", "friendship_2"), ("friendship_3_value", "friendship_3")] {
                    self.amount_line(ui, item, value, flag, -20, 20);
                }
            });
        });
    }

    /// One group, drawn in its card.
    fn group(&self, ui: &mut egui::Ui, item: &mut ItemData, pockets_table: &mut [u8], group: Group) {
        match group {
            Group::Shop => self.shop_group(ui, item),
            Group::Held => self.held_group(ui, item),
            Group::Use => self.use_group(ui, item, pockets_table),
            Group::Effects => self.effects_group(ui, item),
            Group::Evs => self.ev_group(ui, item),
            Group::Friendship => self.friendship_group(ui, item),
        }
    }

    /// The General tab.
    fn general_tab(&mut self, ui: &mut egui::Ui, session: &Session) {
        let Some(mut item) = self.data.take() else { return };
        let mut pockets_table = std::mem::take(&mut self.pockets);
        egui::ScrollArea::vertical().id_salt("item_general").auto_shrink([false, false]).show(ui, |ui| {
            self.icon_card(ui, session);
            ui.add_space(10.0);
            self.csv_card(ui, session);
            ui.add_space(10.0);
            let columns = (((ui.available_width() + 10.0) / (COLUMN_WIDTH + 10.0)).floor() as usize).clamp(1, 2);
            let layout: Vec<Vec<Group>> = if columns == 2 {
                vec![vec![Group::Shop, Group::Use, Group::Friendship], vec![Group::Held, Group::Effects, Group::Evs]]
            } else {
                vec![Group::ALL.to_vec()]
            };
            ui.horizontal_top(|ui| {
                for column in layout {
                    ui.vertical(|ui| {
                        ui.set_width(COLUMN_WIDTH);
                        for group in column {
                            self.group(ui, &mut item, &mut pockets_table, group);
                        }
                    });
                    ui.add_space(10.0);
                }
            });
            ui.add_space(10.0);
        });
        // A reset or an import in the tab reads the files again: the data of this frame is then forgotten.
        if self.loaded.is_some() {
            self.data = Some(item);
            self.pockets = pockets_table;
        }
    }
}

/// Lets the page focus a menu: a click or the arrows of the page make it active, then Up and Down change the choice.
/// `last` is the largest value.
fn focus_combo(ui: &egui::Ui, response: &egui::Response, value: &mut i32, last: i32) {
    let id = response.id;
    widget::register_focusable(ui, id, response.rect);
    if response.clicked() {
        widget::set_active_id(ui, id);
    }
    if widget::is_active(ui, id) && widget::page_has_focus(ui) {
        widget::glow(ui, response.rect, 6.0);
        widget::scroll_active_into_view(ui, response);
        ui.input_mut(|i| {
            let mut step = 0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                step -= 1;
            }
            if step != 0 {
                *value = (*value + step).clamp(0, last);
            }
        });
    }
}

/// A picker of the type of Natural Gift: the icons of the 18 types, and "None" for the other values.
fn type_picker(ui: &mut egui::Ui, id: &str, value: &mut u8) {
    use sango_species::battle_icons;
    let names = sango_species::TYPE_NAMES;
    let has_icons = battle_icons::type_texture(ui.ctx(), 0).is_some();
    let text = match (*value < 18, has_icons) {
        (true, true) => String::new(),
        (true, false) => names[*value as usize].to_string(),
        _ => "None".to_string(),
    };
    let combo = egui::ComboBox::from_id_salt(id).selected_text(text).width(battle_icons::ICON_SIZE.x + 12.0).show_ui(ui, |ui| {
        if ui.selectable_label(*value >= 18, "None").clicked() {
            *value = 31;
        }
        for row in 0..names.len().div_ceil(3) {
            ui.horizontal(|ui| {
                for place in row * 3..(row * 3 + 3).min(names.len()) {
                    let size = battle_icons::ICON_SIZE + egui::vec2(6.0, 4.0);
                    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                    if *value == place as u8 {
                        widget::glow(ui, rect, 5.0);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 4.0, color::MUTED.gamma_multiply(0.25));
                    }
                    if battle_icons::paint_type(ui.painter(), place as u8, rect.left_center() + egui::vec2(3.0, 0.0)).is_none() {
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, names[place], egui::FontId::proportional(12.0), color::TEXT);
                    }
                    if response.clicked() {
                        *value = place as u8;
                        ui.close_menu();
                    }
                }
            });
        }
    });
    let mut stepped = (*value).min(17) as i32;
    let before = stepped;
    focus_combo(ui, &combo.response, &mut stepped, 17);
    if stepped != before {
        *value = stepped as u8;
    }
    if *value < 18 && has_icons {
        let rect = combo.response.rect;
        battle_icons::paint_type(ui.painter(), *value, egui::pos2(rect.left() + 6.0, rect.center().y));
    }
}

impl Module for Item {
    fn title(&self) -> &str {
        "Item"
    }

    fn icon(&self) -> &str {
        "I"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Item");
        ui.add_space(8.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        if self.root.as_deref() != Some(dump.root()) {
            self.open(&dump);
        }
        if !self.icons_ready {
            self.icons.load_card(session.sd.as_ref());
            self.icons_ready = true;
        }
        if self.garc.is_some() && self.loaded != Some(self.item) {
            self.load(session);
        }
        let session: &Session = session;
        self.top_bar(ui, session);
        self.reset_window(ui.ctx(), session);
        ui.add_space(8.0);
        let mut clicked = None;
        egui::Frame::none().fill(color::SURFACE).stroke(egui::Stroke::new(1.0_f32, color::BORDER)).rounding(egui::Rounding::same(12.0)).inner_margin(egui::Margin::same(6.0)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for (i, name) in TABS.iter().enumerate() {
                    let tab_id = egui::Id::new(format!("item_tab_{i}"));
                    let active = widget::is_active(ui, tab_id);
                    if active && self.tab != i {
                        self.tab = i;
                    }
                    let response = widget::tab_button(ui, name, self.tab == i, active && widget::page_has_focus(ui));
                    widget::register_focusable(ui, tab_id, response.rect);
                    ui.ctx().data_mut(|d| d.insert_temp(tab_id.with("is_button"), true));
                    if response.clicked() {
                        clicked = Some((i, tab_id));
                    }
                }
            });
        });
        if let Some((i, tab_id)) = clicked {
            self.tab = i;
            widget::set_active_id(ui, tab_id);
        }
        ui.add_space(6.0);
        if self.data.is_none() && self.loaded.is_some() {
            return;
        }
        match self.tab {
            0 => self.general_tab(ui, session),
            _ => {
                let sd = session.sd.as_ref();
                self.texts.ui(ui, sd, self.item);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the page draws for some items, in both tabs, and the notes find their examples.
    #[test]
    fn the_page_draws_for_some_items() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_item_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Item::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Item, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        for (item, tab) in [(1, 0), (17, 0), (50, 1), (775, 0), (1, 1)] {
            page.item = item;
            page.tab = tab;
            frame(&mut page, &mut session);
            frame(&mut page, &mut session);
        }
        assert!(page.data.is_some());
        let index = page.index.as_ref().unwrap();
        let hp = DEFS.iter().find(|d| d.key == "hp_restore").unwrap();
        let (shown, more) = index.examples(hp, 20, 17);
        assert!(!shown.is_empty() && shown.len() + more > 1, "other items restore 20 HP");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
