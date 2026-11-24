//! The small parts of the species that the other pages use: the names, the data of a species, the icons, the menus of
//! types and the search list. They are in their own crate so that a change of the Species page does not compile the
//! other pages again.

pub mod battle_icons;
pub mod data;
pub mod extras;
pub mod form_names;
pub mod icons;
pub mod move_names;
pub mod names;

pub use extras::{is_species, max_species, species_ids, species_name};
pub use move_names::MOVE as MOVE_NAMES;
pub use names::ABILITY as ABILITY_NAMES;
pub use names::ITEM as ITEM_NAMES;
pub use names::TYPE as TYPE_NAMES;
pub use names::{LEVELING_RATE, SPECIES};

use data::SpeciesData;
use sango_core::{Clim, Garc};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Session};
use std::collections::HashMap;

/// The number of the archive that has the data of the species.
pub const ARCHIVE: usize = 195;

/// The number of species of the game. The packs add species with the numbers 724 and more (see the module `extras`).
pub const SPECIES_COUNT: usize = names::SPECIES.len();

/// The key, in the memory of the interface, of the species that another page asks this page to open.
pub const OPEN_SPECIES: &str = "sango_open_species";

/// Asks the Species page to show a species (its form 0) and switches to that page.
pub fn open_species(ctx: &egui::Context, species: usize) {
    open_species_form(ctx, species, 0);
}

/// Asks the Species page to show a species and one of its forms, and switches to that page.
pub fn open_species_form(ctx: &egui::Context, species: usize, form: usize) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN_SPECIES), (species, form)));
    widget::open_page(ctx, "Species");
}

/// The name of a species and a form, as Pokémon Showdown writes it: the name of the species for the first form, else the species,
/// "-" and the form ("Rotom-Heat", "Charizard-Mega-X"). A form is a Pokémon of its own in the lists: it has its own types and stats.
pub fn species_form_name(species: usize, form: usize) -> String {
    if form == 0 {
        return species_name(species);
    }
    // A form of the game has a name of the list ("Mega X" gives "Mega-X"); another form keeps its label ("Form 3").
    let label = match form_names::game_form_name(species, form) {
        Some(name) => name.replace(' ', "-"),
        None => form_names::form_label(species, form, extras::pack_forms(species).iter().any(|f| f.form as usize == form && f.mega.is_mega())),
    };
    format!("{}-{label}", species_name(species))
}

/// The number of forms that a menu offers for a species: the named forms of the game and the forms of the packs (at least one).
pub fn menu_form_count(species: usize) -> usize {
    extras::form_count(species, form_names::names_of(species).len())
}

/// The most forms that [`form_key`] holds.
pub const KEY_FORMS: usize = 32;

/// One number for a species and a form, for the menus that keep a number: `species * 32 + form`.
pub fn form_key(species: usize, form: usize) -> u16 {
    (species * KEY_FORMS + form.min(KEY_FORMS - 1)) as u16
}

/// The species and the form of a number of [`form_key`].
pub fn from_form_key(key: u16) -> (usize, usize) {
    (key as usize / KEY_FORMS, key as usize % KEY_FORMS)
}

/// The entries of a menu of Pokémon: each species and each of its forms, as `(form_key, "#0479 Rotom-Heat")`.
pub fn species_form_menu(species: impl IntoIterator<Item = usize>) -> Vec<(u16, String)> {
    species
        .into_iter()
        .flat_map(|number| (0..menu_form_count(number)).map(move |form| (form_key(number, form), format!("#{number:04} {}", species_form_name(number, form)))))
        .collect()
}

/// The abilities of a species or of a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeciesAbilities {
    pub species: usize,
    pub form: usize,
    /// Ability 1, ability 2 and the hidden ability (0 is none).
    pub abilities: [u8; 3],
}

/// The abilities of every species and form: the SD card first, then the romfs.
pub fn species_abilities(session: &Session) -> Result<Vec<SpeciesAbilities>, String> {
    let dump = session.dump.as_ref().ok_or("The dump is not open.")?;
    let garc = Garc::open(&dump.archive(ARCHIVE))?;
    extras::refresh(session.sd.as_ref());
    let mut all = Vec::new();
    for species in extras::species_ids() {
        let game_forms = if species <= SPECIES_COUNT { form_count(&garc, species) } else { 1 };
        for form in 0..extras::form_count(species, game_forms) {
            let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(species, form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
            let data = on_card.or_else(|| game_data(&garc, species, form));
            if let Some(data) = data {
                all.push(SpeciesAbilities { species, form, abilities: [data.ability_1, data.ability_2, data.ability_hidden] });
            }
        }
    }
    Ok(all)
}

/// What a team builder needs of a species (form 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// The base stats: HP, Attack, Defense, Sp. Atk, Sp. Def, Speed.
    pub base: [u8; 6],
    /// The two types (the same number twice for a species with one type).
    pub types: [u8; 2],
    /// Ability 1, ability 2 and the hidden ability (0 is none).
    pub abilities: [u8; 3],
    /// The gender ratio: 0 male only, 254 female only, 255 no gender.
    pub gender_ratio: u8,
}

/// The summary of every species, form 0: the SD card first, then the romfs. The index is the number of the species (0 is not used).
pub fn summaries(session: &Session) -> Result<Vec<Option<Summary>>, String> {
    let dump = session.dump.as_ref().ok_or("The dump is not open.")?;
    let garc = Garc::open(&dump.archive(ARCHIVE))?;
    extras::refresh(session.sd.as_ref());
    let mut all = vec![None; extras::max_species() + 1];
    for species in extras::species_ids() {
        let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(species, 0))).and_then(|b| SpeciesData::from_bytes(&b).ok());
        if let Some(d) = on_card.or_else(|| game_data(&garc, species, 0)) {
            all[species] = Some(Summary {
                base: [d.base_hp, d.base_attack, d.base_defense, d.base_special_attack, d.base_special_defense, d.base_speed],
                types: [d.type_1, d.type_2],
                abilities: [d.ability_1, d.ability_2, d.ability_hidden],
                gender_ratio: d.gender_ratio,
            });
        }
    }
    Ok(all)
}

/// The icons of the species, as textures: decoded when a species is shown, then kept.
#[derive(Default)]
pub struct SpeciesIcons {
    garc: Option<Garc>,
    textures: HashMap<usize, Option<egui::TextureHandle>>,
}

impl SpeciesIcons {
    /// Opens the archive of the icons of a dump.
    pub fn open(&mut self, dump: &sango_core::Dump) {
        self.garc = Garc::open(&dump.archive(icons::ICON_ARCHIVE)).ok();
        self.textures.clear();
    }

    /// The icon (40 by 30 pixels) of a species.
    pub fn get(&mut self, ctx: &egui::Context, species: usize) -> Option<&egui::TextureHandle> {
        icon_texture(&mut self.textures, self.garc.as_ref(), ctx, species)
    }
}

/// The names of the forms of a species, from the form 0, for the menus of the other pages.
pub fn form_choices(species: usize) -> Vec<String> {
    form_names::names_of(species)
}

/// The Leveling rate of each species, from the species 1 on (form 0).
///
/// The SD card comes first, then the romfs. A species whose file cannot be read gives `None`.
pub fn leveling_rates(session: &Session) -> Result<Vec<Option<u8>>, String> {
    let dump = session.dump.as_ref().ok_or("The dump is not open.")?;
    let garc = Garc::open(&dump.archive(ARCHIVE))?;
    extras::refresh(session.sd.as_ref());
    Ok((1..=extras::max_species())
        .map(|species| {
            if !extras::is_species(species) {
                return None;
            }
            let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(species, 0))).and_then(|b| SpeciesData::from_bytes(&b).ok());
            on_card.or_else(|| game_data(&garc, species, 0)).map(|d| d.leveling_rate)
        })
        .collect())
}

/// The Leveling rate of each species and of each of its forms: `(species, form, rate)`. The SD card comes first, then the romfs.
pub fn form_leveling_rates(session: &Session) -> Result<Vec<(usize, usize, u8)>, String> {
    let dump = session.dump.as_ref().ok_or("The dump is not open.")?;
    let garc = Garc::open(&dump.archive(ARCHIVE))?;
    extras::refresh(session.sd.as_ref());
    let mut rates = Vec::new();
    for species in extras::species_ids() {
        let game_forms = garc.first_file(species).filter(|_| species <= SPECIES_COUNT).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |b| (b.total_form_count as usize).max(1));
        for form in 0..extras::form_count(species, game_forms) {
            let on_card = session.sd.as_ref().and_then(|sd| sd.read(&sd_path(species, form))).and_then(|b| SpeciesData::from_bytes(&b).ok());
            if let Some(data) = on_card.or_else(|| game_data(&garc, species, form)) {
                rates.push((species, form, data.leveling_rate));
            }
        }
    }
    Ok(rates)
}

/// The place in the archive of the file of a species and a form.
///
/// A form after the first one is in the archive after all the species: the file of the species says where.
pub fn file_index(garc: &Garc, species: usize, form: usize) -> usize {
    // A species of a pack has no file in the archive: the numbers after the species of the game are files of forms.
    if species > SPECIES_COUNT {
        return usize::MAX;
    }
    if form == 0 {
        return species;
    }
    match garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()) {
        Some(base) if base.alternate_form_file != 0 && form < base.total_form_count as usize => base.alternate_form_file as usize + form - 1,
        _ => species,
    }
}

/// The data of a species or a form in the archive of the game, or from a pack when the game has none: a species of a pack, or a
/// form that a pack adds.
pub fn game_data(garc: &Garc, species: usize, form: usize) -> Option<SpeciesData> {
    if species <= SPECIES_COUNT {
        let game_forms = garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |b| (b.total_form_count as usize).max(1));
        if form < game_forms {
            return garc.first_file(file_index(garc, species, form)).and_then(|b| SpeciesData::from_bytes(b).ok());
        }
    }
    extras::data(species, form)
}

/// The path in the SD card folder of the file of a species and a form.
pub fn sd_path(species: usize, form: usize) -> String {
    format!("species/data/{species}_{form}.bin")
}

/// The name in a list from a number. The first number of the list is `first`.
pub fn name_from(list: &[&str], number: usize, first: usize) -> String {
    number.checked_sub(first).and_then(|i| list.get(i)).map_or("-".to_string(), |s| s.to_string())
}

/// Where the icons of the species are read: the table of the packs and the folder of the icon files of the SD card.
struct IconSource {
    /// The folder `species/icon` on the computer.
    folder: Option<std::path::PathBuf>,
    /// The icon file of the species of the packs (form 0), from `species/icon_map.bin`.
    files: std::collections::BTreeMap<usize, u16>,
    /// The bytes of the table and the time of the folder: when they change, the textures are made again.
    stamp: (Vec<u8>, Option<std::time::SystemTime>),
    checked: Option<std::time::Instant>,
    /// Grows each time that the table or the folder changed.
    generation: u32,
}

static ICON_SOURCE: std::sync::Mutex<Option<IconSource>> = std::sync::Mutex::new(None);

/// The file of the icon that the game shows for a species that it does not know: the black Poké Ball.
const UNKNOWN_ICON_FILE: usize = 0;

/// Reads the table of the icons of the packs (`species/icon_map.bin`) when it changed, and finds the folder of the icon files of the SD
/// card. It looks once each second and a half at most. The textures of the icons are made again when something changed.
pub fn refresh_icons(sd: Option<&sango_core::SdCard>) {
    let Ok(mut guard) = ICON_SOURCE.lock() else { return };
    let source = guard.get_or_insert_with(|| IconSource { folder: None, files: Default::default(), stamp: (Vec::new(), None), checked: None, generation: 0 });
    if source.checked.is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(1500)) {
        return;
    }
    source.checked = Some(std::time::Instant::now());
    let folder = sd.and_then(|sd| sd.path("species/icon").ok());
    let bytes = sd.and_then(|sd| sd.read(sango_pack::library::ICON_MAP_PATH)).unwrap_or_default();
    let time = folder.as_ref().and_then(|f| std::fs::metadata(f).ok()).and_then(|m| m.modified().ok());
    if source.stamp == (bytes.clone(), time) && source.folder == folder {
        return;
    }
    source.files.clear();
    if bytes.len() >= 8 && &bytes[..4] == b"SICM" {
        let count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        for entry in bytes[8..].chunks_exact(6).take(count) {
            let (species, form, file) = (u16::from_le_bytes([entry[0], entry[1]]), u16::from_le_bytes([entry[2], entry[3]]), u16::from_le_bytes([entry[4], entry[5]]));
            if form == 0 {
                source.files.insert(species as usize, file);
            }
        }
    }
    source.stamp = (bytes, time);
    source.folder = folder;
    source.generation += 1;
}

/// The icon file of a species and the bytes of the file: the SD card first (the icons of the packs and the icons that the user changed),
/// then the archive of the game. A species without an icon gets the black Poké Ball, as in the game.
fn icon_bytes(garc: Option<&Garc>, species: usize) -> Option<Vec<u8>> {
    let (folder, mapped) = match ICON_SOURCE.lock() {
        Ok(guard) => guard.as_ref().map_or((None, None), |s| (s.folder.clone(), s.files.get(&species).copied())),
        Err(_) => (None, None),
    };
    let file = mapped.map(usize::from).or_else(|| icons::ICON_FILES.get(species).map(|&f| f as usize)).filter(|_| species != 0).unwrap_or(UNKNOWN_ICON_FILE);
    let on_card = folder.and_then(|f| std::fs::read(f.join(format!("{file}.bin"))).ok());
    let bytes = on_card.or_else(|| garc?.first_file(file).map(|b| b.to_vec()))?;
    Some(if sango_core::lz::is_compressed(&bytes) { sango_core::lz::decompress(&bytes).ok()? } else { bytes })
}

/// The texture of the icon of a species: decoded at the first display, then kept. It is made again when the icons of the SD card changed
/// (see [`refresh_icons`]).
pub fn icon_texture<'a>(
    textures: &'a mut HashMap<usize, Option<egui::TextureHandle>>,
    garc: Option<&Garc>,
    ctx: &egui::Context,
    species: usize,
) -> Option<&'a egui::TextureHandle> {
    let generation = ICON_SOURCE.lock().ok().and_then(|g| g.as_ref().map(|s| s.generation)).unwrap_or(0);
    // The key `usize::MAX - generation` marks the generation that the textures of this list belong to.
    let marker = usize::MAX - generation as usize;
    if !textures.contains_key(&marker) {
        textures.clear();
        textures.insert(marker, None);
    }
    textures
        .entry(species)
        .or_insert_with(|| {
            let bytes = icon_bytes(garc, species)?;
            let image = Clim::parse(&bytes).ok()?;
            let rgba = image.to_rgba().ok()?;
            let pixels = egui::ColorImage::from_rgba_unmultiplied([image.width(), image.height()], &rgba);
            Some(sango_ui::textures::load(ctx, format!("species_icon_{species}_{generation}"), pixels, egui::TextureOptions::NEAREST))
        })
        .as_ref()
}

/// A drop-down list with a search box of the things of a list of names. The value is the number of the thing: 0 is "None" and the number
/// `n` is the name at the place `n - 1`.
pub fn searchable_picker<T>(
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
    // The menu is the active one after a click: with the list closed, the arrows go to the previous or the next entry.
    if widget::is_active(ui, picker_id) && widget::page_has_focus(ui) && !ui.memory(|m| m.is_popup_open(popup_id)) {
        widget::glow(ui, rect, 6.0);
        widget::scroll_active_into_view(ui, &button_response);
        let (up, down) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)));
        let named = |n: usize| n == 0 || names.get(n - 1).is_some_and(|name| !name.is_empty());
        let target = if down {
            (current_id + 1..=names.len()).find(|&n| named(n))
        } else if up {
            (0..current_id).rev().find(|&n| named(n))
        } else {
            None
        };
        if let Some(new_val) = target.and_then(|n| T::try_from(n).ok()) {
            *value = new_val;
        }
    }

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
                if name.is_empty() {
                    // An id without a name (the free ids between the abilities) is not in the list.
                    continue;
                }
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

/// The numbers that a menu of types lists: the 18 types of the game, then the new types of the user.
pub fn type_entries(with_custom: bool) -> Vec<u8> {
    let custom = if with_custom { battle_icons::custom_type_ids() } else { Vec::new() };
    (0..names::TYPE.len() as u8).chain(custom).collect()
}

/// Draws the icon of a type at a place: the picture, or the name of a new type that has no picture.
pub fn paint_type_or_name(ui: &egui::Ui, kind: u8, left_center: egui::Pos2) {
    if battle_icons::paint_type(ui.painter(), kind, left_center).is_some() {
        return;
    }
    if let Some(name) = battle_icons::custom_type_name(kind) {
        let rect = egui::Rect::from_min_size(egui::pos2(left_center.x, left_center.y - battle_icons::ICON_SIZE.y / 2.0), battle_icons::ICON_SIZE);
        ui.painter().rect_filled(rect, 6.0, color::MUTED.gamma_multiply(0.6));
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(11.0), color::WHITE);
    }
}

/// A menu of the types: the button and the list show the icons of the game. Without the icons it is a menu of names. The new
/// types of the user (see [`battle_icons::refresh_custom_types`]) come after the types of the game.
pub fn type_menu(ui: &mut egui::Ui, id: &str, value: &mut u8, first: Option<u8>) {
    type_menu_of(ui, id, value, first, true);
}

/// The same menu with the 18 types of the game only: for a choice that must be a type of the game.
pub fn game_type_menu(ui: &mut egui::Ui, id: &str, value: &mut u8) {
    type_menu_of(ui, id, value, None, false);
}

pub fn type_menu_of(ui: &mut egui::Ui, id: &str, value: &mut u8, first: Option<u8>, with_custom: bool) {
    if battle_icons::type_texture(ui.ctx(), 0).is_none() {
        return menu(ui, id, value, &names::TYPE, 130.0);
    }
    let entries = type_entries(with_custom);
    let combo = egui::ComboBox::from_id_salt(id).selected_text("").width(battle_icons::ICON_SIZE.x + 12.0).show_ui(ui, |ui| {
        let size = battle_icons::ICON_SIZE;
        for row in entries.chunks(3) {
            ui.horizontal(|ui| {
                for &place in row {
                    let (rect, response) = ui.allocate_exact_size(size + egui::vec2(6.0, 4.0), egui::Sense::click());
                    if *value == place {
                        widget::glow(ui, rect, 5.0);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 4.0, color::MUTED.gamma_multiply(0.25));
                    }
                    paint_type_or_name(ui, place, rect.left_center() + egui::vec2(3.0, 0.0));
                    if response.clicked() {
                        *value = place;
                        ui.close_menu();
                    }
                }
            });
        }
    });
    let rect = combo.response.rect;
    // The second type that is the same as the first one is not shown (as the lists do).
    if first != Some(*value) {
        paint_type_or_name(ui, *value, egui::pos2(rect.left() + 6.0, rect.center().y));
    }
    let menu_id = combo.response.id;
    widget::register_focusable(ui, menu_id, rect);
    if combo.response.clicked() {
        widget::set_active_id(ui, menu_id);
    }
    if widget::is_active(ui, menu_id) && widget::page_has_focus(ui) {
        widget::glow(ui, rect, 6.0);
        widget::scroll_active_into_view(ui, &combo.response);
        ui.input_mut(|i| {
            let mut step: isize = 0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                step += 1;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                step -= 1;
            }
            if step != 0 {
                let at = entries.iter().position(|e| e == value).unwrap_or(0) as isize;
                *value = entries[(at + step).clamp(0, entries.len() as isize - 1) as usize];
            }
        });
    }
}

/// A menu of names: the value is the place in the list.
pub fn menu(ui: &mut egui::Ui, id: &str, value: &mut u8, list: &[&str], width: f32) {
    let combo = egui::ComboBox::from_id_salt(id).selected_text(name_from(list, *value as usize, 0)).width(width).show_ui(ui, |ui| {
        for (place, name) in list.iter().enumerate() {
            let chosen = *value == place as u8;
            let row = ui.selectable_label(chosen, *name);
            widget::scroll_to_chosen(ui, &row, chosen);
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

/// The number of forms of a species (at least one).
pub fn form_count(garc: &Garc, species: usize) -> usize {
    garc.first_file(species).and_then(|b| SpeciesData::from_bytes(b).ok()).map_or(1, |d| (d.total_form_count as usize).max(1))
}
