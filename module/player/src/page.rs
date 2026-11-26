//! The Player page: the list of the things of a character on the left, the chosen one on the right.

use crate::icons;
use crate::store::{entries, Entry, Kind, Store, ENTRY_COUNT, ICON_SIDE, PLAYERS};
use sango_core::model::{parse_model, parse_textures, Model, Texture};
use sango_core::render::{render, Camera};
use sango_core::{Dump, SdCard};
use sango_ui::history::{Step, Steps};
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module, Session};
use std::collections::HashMap;
use std::path::PathBuf;

/// The size of the picture of a model, in points.
const VIEW: usize = 400;
/// The width of the list on the left.
const LIST_WIDTH: f32 = 270.0;
/// The place in the list of the line "All the PSS icons", after the things of the character.
const ALL_ICONS: usize = ENTRY_COUNT;

/// A step of the history: a file of the SD card as it was, and the line of the list that changed it.
struct Replaced {
    player: usize,
    line: usize,
    icon: usize,
    path: String,
    bytes: Option<Vec<u8>>,
}

/// The model that the page draws.
struct Shown {
    kind: Kind,
    model: Result<(Model, HashMap<String, Texture>), String>,
    camera: Camera,
    picture: Option<egui::TextureHandle>,
}

pub struct PlayerPage {
    root: Option<PathBuf>,
    /// The SD card that the pictures and the steps are for.
    card: Option<PathBuf>,
    store: Option<Store>,
    error: Option<String>,
    /// 0 Brendan, 1 May.
    player: usize,
    /// The chosen line of the list.
    line: usize,
    /// The icon of the line "All the PSS icons".
    icon: usize,
    /// The icon that the button "Use this icon" copies.
    source: usize,
    /// The models that the button "Use this model" takes: one for the overworld and the Soar sequence, one for the battles.
    other_character: u16,
    other_trainer: u16,
    /// The joints that the model of the menu shares with the one of the player, for the thing that is shown.
    joints: Option<((Kind, u16), Result<(usize, usize), String>)>,
    /// The pictures of the screen, by their file and name. Emptied at each change.
    pictures: HashMap<String, Option<egui::TextureHandle>>,
    shown: Option<Shown>,
    status: Option<(bool, String)>,
    reset_open: bool,
    /// The size of an imported model.
    scale: f32,
    steps: Steps<Replaced>,
}

impl Default for PlayerPage {
    fn default() -> Self {
        PlayerPage { root: None, card: None, store: None, error: None, player: 0, line: 0, icon: 38, source: 38, other_character: 182, other_trainer: 8, joints: None, pictures: HashMap::new(), shown: None, status: None, reset_open: false, scale: 1.0, steps: Steps::default() }
    }
}

impl PlayerPage {
    fn open(&mut self, dump: &Dump) {
        self.root = Some(dump.root().to_path_buf());
        self.forget_pictures();
        self.steps.clear();
        self.status = None;
        match Store::open(dump) {
            Ok(store) => (self.store, self.error) = (Some(store), None),
            Err(message) => (self.store, self.error) = (None, Some(message)),
        }
    }

    /// The pictures and the model are read again at the next frame.
    fn forget_pictures(&mut self) {
        self.pictures.clear();
        self.shown = None;
        self.joints = None;
    }

    /// The chosen thing.
    fn entry(&self) -> Entry {
        match entries(self.player).get(self.line) {
            Some(entry) => *entry,
            None => Entry { name: "PSS icons", note: "Every picture of the PSS: the trainers that a player can choose, and the pieces of the avatars of X and Y.", kind: Kind::Icon(self.icon) },
        }
    }

    /// A picture of the game or of the SD card, as a texture.
    fn texture(&mut self, ctx: &egui::Context, sd: Option<&SdCard>, kind: Kind) -> Option<egui::TextureHandle> {
        let key = match kind {
            Kind::Picture(_, _, name) | Kind::Intro(name) => format!("{}/{name}", kind.path()),
            _ => kind.path(),
        };
        if !self.pictures.contains_key(&key) {
            let made = self.store.as_ref().and_then(|store| store.picture(sd, kind).ok()).map(|(width, height, rgba)| {
                let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
                sango_ui::textures::load(ctx, format!("player/{key}"), image, egui::TextureOptions::NEAREST)
            });
            self.pictures.insert(key.clone(), made);
        }
        self.pictures.get(&key)?.clone()
    }

    /// Does an action that writes the file of `kind` on the SD card, keeps the old file for the undo, and tells the result.
    fn change(&mut self, sd: &SdCard, kind: Kind, done: &str, action: impl FnOnce(&Store) -> Result<(), String>) {
        let Some(store) = &self.store else { return };
        let path = kind.path();
        let before = sd.read(&path);
        match action(store) {
            Ok(()) => {
                self.steps.done(Replaced { player: self.player, line: self.line, icon: self.icon, path, bytes: before });
                self.status = Some((true, format!("{done} Restart the game to see it.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
        self.forget_pictures();
    }

    /// Undo or redo: the file of the SD card goes back, and its line is shown.
    fn step(&mut self, step: Step, sd: &SdCard) {
        let mut shown = None;
        let done = self.steps.apply(step, |kept| {
            let replaced = Replaced { player: kept.player, line: kept.line, icon: kept.icon, path: kept.path.clone(), bytes: sd.read(&kept.path) };
            Store::put(sd, &kept.path, kept.bytes.as_deref())?;
            shown = Some((kept.player, kept.line, kept.icon));
            Ok(replaced)
        });
        match done {
            Some(Ok(())) => {
                if let Some(place) = shown {
                    (self.player, self.line, self.icon) = place;
                }
                let way = if step == Step::Undo { "Undone" } else { "Done again" };
                self.status = Some((true, format!("{way}. Restart the game to see it.")));
            }
            Some(Err(message)) => self.status = Some((false, message)),
            None => {}
        }
        self.forget_pictures();
    }

    fn export(&mut self, sd: Option<&SdCard>, entry: Entry) {
        let Some(store) = &self.store else { return };
        let name = format!("{}_{}", PLAYERS[self.player], entry.name).to_lowercase().replace([' ', ','], "_");
        let (extension, label) = if entry.kind.is_model() { ("glb", "3D model") } else { ("png", "PNG picture") };
        let Some(path) = rfd::FileDialog::new().set_title("Export").set_file_name(format!("{name}.{extension}")).add_filter(label, &[extension]).save_file() else { return };
        let result = if entry.kind.is_model() {
            store.export_model(sd, entry.kind).and_then(|file| std::fs::write(&path, file).map_err(|e| e.to_string()))
        } else {
            store.picture(sd, entry.kind).and_then(|(width, height, rgba)| image::save_buffer_with_format(&path, &rgba, width as u32, height as u32, image::ColorType::Rgba8, image::ImageFormat::Png).map_err(|e| e.to_string()))
        };
        self.status = Some(match result {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(message) => (false, format!("{}: {message}", path.display())),
        });
    }

    fn import(&mut self, sd: &SdCard, entry: Entry) {
        let dialog = rfd::FileDialog::new().set_title("Import");
        if entry.kind.is_model() {
            let Some(file) = dialog.add_filter("3D model", &["glb", "gltf"]).pick_file() else { return };
            let scale = f64::from(self.scale);
            self.change(sd, entry.kind, "The model is changed.", |store| store.import_model(sd, entry.kind, &file, scale));
        } else {
            let Some(file) = dialog.add_filter("PNG picture", &["png"]).pick_file() else { return };
            self.change(sd, entry.kind, "The picture is changed.", |store| {
                let picture = image::open(&file).map_err(|e| format!("{}: {e}", file.display()))?.to_rgba8();
                store.set_picture(sd, entry.kind, picture.width() as usize, picture.height() as usize, picture.as_raw())
            });
        }
    }

    /// The list on the left: the pictures, then the models. The up and down arrows choose the line.
    fn list(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        let things = entries(self.player);
        let mut chosen = None;
        let top = ui.cursor().min;
        for line in 0..=ALL_ICONS {
            if line == 0 {
                ui.label(widget::title("Pictures"));
            }
            if line < ALL_ICONS && things[line].kind.is_model() && !things[line - 1].kind.is_model() {
                ui.add_space(6.0);
                ui.label(widget::title("3D models"));
            }
            if line == ALL_ICONS {
                ui.add_space(6.0);
                ui.label(widget::title("Other trainers"));
            }
            let (name, kind) = things.get(line).map_or(("All the PSS icons", Kind::Icon(self.icon)), |entry| (entry.name, entry.kind));
            let changed = self.store.as_ref().is_some_and(|store| store.changed(sd, kind));
            let picture = if kind.is_model() { None } else { self.texture(ui.ctx(), sd, kind) };
            let row = ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::hover());
                match &picture {
                    Some(texture) => paint_picture(ui, rect, texture),
                    None => {
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "🧍", egui::FontId::proportional(18.0), color::ACCENT);
                    }
                }
                let text = if changed { egui::RichText::new(name).color(color::ACCENT) } else { egui::RichText::new(name) };
                ui.selectable_label(self.line == line, text)
            });
            let response = row.inner;
            widget::scroll_to_chosen(ui, &response, self.line == line);
            if response.clicked() {
                chosen = Some(line);
            }
        }
        // A click in the list gives it the focus: then the arrows choose the line.
        let area = egui::Rect::from_min_max(top, egui::pos2(top.x + LIST_WIDTH, ui.cursor().min.y));
        let clicked = ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(area);
        if focus::focus(ui, egui::Id::new("player_list"), area, clicked, false, None) {
            if let Some(to) = focus::stepped(self.line, focus::steps(ui, false), ALL_ICONS + 1) {
                chosen = Some(to);
            }
        }
        if let Some(line) = chosen {
            if line != self.line {
                self.line = line;
                self.status = None;
            }
        }
    }

    /// A menu of the PSS icons, with their pictures, one on each line.
    fn icon_menu(&mut self, ui: &mut egui::Ui, id: &str, value: &mut usize, sd: Option<&SdCard>) {
        let count = self.store.as_ref().map_or(0, Store::icon_count);
        let combo = egui::ComboBox::from_id_salt(id).selected_text(icons::label(*value)).width(300.0).height(420.0).show_ui(ui, |ui| {
            let height = 34.0;
            let mut list = egui::ScrollArea::vertical().id_salt((id, "rows")).max_height(400.0);
            // The menu opens on the chosen icon, with some icons above it.
            let now = ui.ctx().cumulative_pass_nr();
            let last: Option<u64> = ui.data(|d| d.get_temp(egui::Id::new((id, "open"))));
            ui.data_mut(|d| d.insert_temp(egui::Id::new((id, "open")), now));
            if last.map_or(true, |last| last + 1 < now) {
                list = list.vertical_scroll_offset(value.saturating_sub(4) as f32 * (height + ui.spacing().item_spacing.y));
            }
            list.show_rows(ui, height, count, |ui, range| {
                for file in range.map(|place| place + 1) {
                    let texture = self.texture(ui.ctx(), sd, Kind::Icon(file));
                    ui.horizontal(|ui| {
                        ui.set_height(height);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(height, height), egui::Sense::hover());
                        if let Some(texture) = &texture {
                            paint_picture(ui, rect, texture);
                        }
                        if ui.selectable_label(*value == file, icons::label(file)).clicked() {
                            *value = file;
                            ui.memory_mut(|m| m.close_popup());
                        }
                    });
                }
            });
        });
        let values: Vec<usize> = (1..=count).collect();
        focus::combo_arrows(ui, &combo.response, value, &values);
    }

    /// The card of a picture.
    fn picture_card(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, entry: Entry) {
        match self.texture(ui.ctx(), sd, entry.kind) {
            Some(texture) => {
                let size = texture.size_vec2();
                // Whole pixels, up to 320 points.
                let zoom = (320.0 / size.x.max(size.y)).floor().max(1.0);
                let (rect, _) = ui.allocate_exact_size(size * zoom + egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE_STRONG);
                paint_picture(ui, rect.shrink(8.0), &texture);
                ui.label(widget::hint(&format!("{} × {} pixels. An imported PNG must have this size.", size.x, size.y)));
            }
            None => {
                ui.label(egui::RichText::new("This picture cannot be read.").color(color::ROSE));
            }
        }
        let Kind::Icon(target) = entry.kind else { return };
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(widget::hint("Use the picture of"));
            let mut source = self.source;
            self.icon_menu(ui, "player_icon_source", &mut source, sd);
            self.source = source;
            let use_it = ui.add_enabled(sd.is_some() && source != target, egui::Button::new("Use this icon")).on_hover_text("Gives this icon the picture of the icon of the menu.");
            if let (true, Some(sd)) = (use_it.clicked(), sd) {
                self.change(sd, entry.kind, "The icon has its new picture.", |store| store.copy_icon(sd, source, target));
            }
        });
    }

    /// The card of a model: drag to turn it, the wheel zooms.
    fn model_card(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, entry: Entry) {
        if self.shown.as_ref().map_or(true, |shown| shown.kind != entry.kind) {
            let model = match &self.store {
                Some(store) => store.model(sd, entry.kind).and_then(|bytes| Ok((parse_model(&bytes)?, parse_textures(&bytes)?.into_iter().map(|t| (t.name.clone(), t)).collect()))),
                None => Err("The dump is not open.".to_string()),
            };
            self.shown = Some(Shown { kind: entry.kind, model, camera: Camera::default(), picture: None });
        }
        let Some(shown) = &mut self.shown else { return };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(VIEW as f32, VIEW as f32), egui::Sense::click_and_drag());
        let mut turned = false;
        if response.dragged() {
            let delta = response.drag_delta();
            shown.camera.yaw -= delta.x * 0.012;
            shown.camera.pitch = (shown.camera.pitch + delta.y * 0.012).clamp(-1.4, 1.4);
            turned = true;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            // The wheel zooms the model only: the page does not scroll at the same time.
            ui.input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                i.raw_scroll_delta = egui::Vec2::ZERO;
            });
            if scroll != 0.0 {
                shown.camera.zoom = (shown.camera.zoom * (1.0 + scroll * 0.002)).clamp(0.3, 6.0);
                turned = true;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if turned || shown.picture.is_none() {
            if let Ok((model, textures)) = &shown.model {
                let pixels = render(model, textures, &shown.camera, VIEW, VIEW);
                let image = egui::ColorImage::from_rgba_unmultiplied([VIEW, VIEW], &pixels);
                shown.picture = Some(sango_ui::textures::load(ui.ctx(), "player_model_view", image, egui::TextureOptions::LINEAR));
            }
        }
        ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
        ui.painter().rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0_f32, color::BORDER));
        match (&shown.model, &shown.picture) {
            (Ok((model, textures)), Some(texture)) => {
                ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                ui.label(widget::hint(&format!("{} joints, {} parts, {} pictures. Drag to turn the model.", model.bones.len(), model.meshes.len(), textures.len())));
            }
            (Err(message), _) => {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, message, egui::FontId::proportional(14.0), color::ROSE);
            }
            _ => {}
        }
    }

    /// Under a model: the menu of the other models of the game, and the button that takes the chosen one.
    fn model_menu(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>, entry: Entry) {
        let Some(store) = &self.store else { return };
        let models = store.models_for(entry.kind);
        let battle = matches!(entry.kind, Kind::Battle(_));
        let mut chosen = if battle { self.other_trainer } else { self.other_character };
        let mut use_it = false;
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(widget::hint("Use the model of"));
            let shown = models.iter().find(|(number, _)| *number == chosen).map_or("(none)".to_string(), |(_, name)| name.clone());
            let id = if battle { "player_other_trainer" } else { "player_other_character" };
            picker(ui, id, &shown, 300.0, &mut chosen, None, || {
                if battle {
                    return vec![Group { title: "Trainers of the battles".to_string(), entries: models.clone() }];
                }
                // The characters of the two players first.
                let players: Vec<usize> = (0..PLAYERS.len()).flat_map(|player| entries(player)).filter_map(|entry| if let Kind::Overworld(pack) = entry.kind { Some(pack) } else { None }).collect();
                let (own, others): (Vec<_>, Vec<_>) = models.iter().cloned().partition(|(number, _)| players.contains(&usize::from(*number)));
                vec![Group { title: "Player characters".to_string(), entries: own }, Group { title: "Characters of the game".to_string(), entries: others }]
            });
            let note = "Gives this model the 3D model of the menu. The motions of the player stay. Not tested in the game: a model with other joints or faces can show problems.";
            use_it = ui.add_enabled(sd.is_some(), egui::Button::new("Use this model")).on_hover_text(note).on_disabled_hover_text(note).clicked();
        });
        if battle {
            self.other_trainer = chosen;
        } else {
            self.other_character = chosen;
        }
        let key = (entry.kind, chosen);
        if self.joints.as_ref().map(|(of, _)| *of) != Some(key) {
            self.joints = Some((key, store.shared_joints(entry.kind, chosen)));
        }
        match self.joints.as_ref().map(|(_, joints)| joints) {
            Some(Ok((shared, all))) => {
                ui.label(widget::hint(&format!("{shared} of its {all} joints move with the motions of the player.")));
            }
            Some(Err(message)) => {
                ui.label(egui::RichText::new(message).color(color::AMBER));
            }
            None => {}
        }
        if let (true, Some(sd)) = (use_it, sd) {
            self.change(sd, entry.kind, "The model is changed.", |store| store.use_model(sd, entry.kind, chosen));
        }
    }

    /// The right side: the name of the thing, the buttons, then the thing.
    fn detail(&mut self, ui: &mut egui::Ui, sd: Option<&SdCard>) {
        widget::card(ui, |ui| {
            if self.line == ALL_ICONS {
                ui.horizontal(|ui| {
                    ui.label(widget::hint("Icon"));
                    let mut icon = self.icon;
                    self.icon_menu(ui, "player_icon", &mut icon, sd);
                    self.icon = icon;
                });
                ui.add_space(6.0);
            }
            let entry = self.entry();
            let title = if self.line == ALL_ICONS { icons::label(self.icon) } else { format!("{} · {}", PLAYERS[self.player], entry.name) };
            ui.label(widget::title(&title)).on_hover_text(entry.note);
            ui.label(widget::hint(entry.note));
            ui.add_space(6.0);
            let changed = self.store.as_ref().is_some_and(|store| store.changed(sd, entry.kind));
            let model = entry.kind.is_model();
            ui.horizontal_wrapped(|ui| {
                let clicks = widget::action_bar(
                    ui,
                    widget::ActionBar {
                        export: Some(true),
                        import: Some(sd.is_some()),
                        reset: Some(changed && sd.is_some()),
                        notes: widget::ActionNotes {
                            export: if model { "Writes the model in a GLB file." } else { "Writes the picture in a PNG file." },
                            import: if model { "Replaces the model with a glTF or GLB file. The new model must have the joints of the old one." } else { "Replaces the picture with a PNG file of the same size." },
                            reset: "Puts the file of the game back.",
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                );
                if clicks.export {
                    self.export(sd, entry);
                }
                if let (true, Some(sd)) = (clicks.import, sd) {
                    self.import(sd, entry);
                }
                self.reset_open |= clicks.reset;
                if let (Some(step), Some(sd)) = (self.steps.asked(ui, sd.is_some()), sd) {
                    self.step(step, sd);
                }
                if changed {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
                if model {
                    ui.label(widget::hint("Scale"));
                    ui.add(egui::DragValue::new(&mut self.scale).speed(0.01).range(0.05..=20.0)).on_hover_text("The size of an imported model");
                }
            });
            if sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can look and export, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
            ui.add_space(8.0);
            // A step or an import changed the chosen line: it is read again.
            let entry = self.entry();
            if entry.kind.is_model() {
                self.model_card(ui, sd, entry);
                self.model_menu(ui, sd, entry);
            } else {
                self.picture_card(ui, sd, entry);
            }
        });
        let entry = self.entry();
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &format!("{} of {}", entry.name, PLAYERS[self.player]), &mut []) {
            if let Some(sd) = sd {
                self.change(sd, entry.kind, "The file of the game is back.", |store| store.reset(sd, entry.kind));
            }
        }
    }
}

/// A picture in the middle of a rectangle, as large as it can be with its proportions.
fn paint_picture(ui: &egui::Ui, rect: egui::Rect, texture: &egui::TextureHandle) {
    let size = texture.size_vec2();
    let zoom = (rect.width() / size.x).min(rect.height() / size.y);
    let zoom = if size.x <= ICON_SIDE as f32 && zoom >= 1.0 { zoom.floor() } else { zoom };
    let place = egui::Rect::from_center_size(rect.center(), size * zoom);
    ui.painter().image(texture.id(), place, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
}

impl Module for PlayerPage {
    fn title(&self) -> &str {
        "Player"
    }

    fn icon(&self) -> &str {
        "👤"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Player");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        if self.root.as_deref() != Some(dump.root()) {
            self.open(dump);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        let sd = session.sd.clone();
        let card = sd.as_ref().map(|sd| sd.root().to_path_buf());
        if self.card != card {
            self.card = card;
            self.forget_pictures();
            self.steps.clear();
        }
        if widget::tabs(ui, "player", &PLAYERS, &mut self.player) {
            // The icon of the menu "Use the picture of" follows the character.
            if let Kind::Icon(file) = entries(self.player)[0].kind {
                (self.source, self.icon) = (file, file);
            }
            self.status = None;
        }
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(LIST_WIDTH);
                egui::ScrollArea::vertical().id_salt("player_list_scroll").auto_shrink([false, false]).max_width(LIST_WIDTH).show(ui, |ui| self.list(ui, sd.as_ref()));
            });
            ui.vertical(|ui| {
                egui::ScrollArea::vertical().id_salt("player_detail_scroll").auto_shrink([false, false]).show(ui, |ui| self.detail(ui, sd.as_ref()));
            });
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: every line of both characters draws, and an icon that is copied is undone and done again with the keys.
    #[test]
    fn the_page_draws_every_line_and_undoes_a_change() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_player_page_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd.clone()), focus_page: true, ..Session::default() };
        let mut page = PlayerPage::default();
        let ctx = egui::Context::default();
        let mut frame = |page: &mut PlayerPage, input: egui::RawInput| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..input };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        };
        for player in 0..PLAYERS.len() {
            for line in 0..=ALL_ICONS {
                (page.player, page.line) = (player, line);
                frame(&mut page, Default::default());
                assert!(page.error.is_none(), "{:?}", page.error);
                if let Some(shown) = &page.shown {
                    assert!(shown.model.is_ok(), "line {line}: {:?}", shown.model.as_ref().err());
                }
            }
        }
        (page.player, page.line) = (0, 0);
        let kind = page.entry().kind;
        page.change(&sd, kind, "Changed.", |store| store.copy_icon(&sd, 39, 38));
        assert!(sd.read(&kind.path()).is_some());
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Z));
        assert!(sd.read(&kind.path()).is_none(), "{:?}", page.status);
        frame(&mut page, sango_ui::history::test_keys(egui::Key::Y));
        assert!(sd.read(&kind.path()).is_some());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
