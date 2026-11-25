//! The menu of the languages of the game, the same on every page: English first, and before each name the tag that the game shows
//! for a Pokémon of this language (`ENG`, `FRA`...).

use eframe::egui;
use sango_core::language_tags::{self, Picture};
use sango_core::text::LANGUAGES;
use sango_core::Dump;
use std::path::PathBuf;
use std::sync::Mutex;

/// The place of English in [`LANGUAGES`]: the language that every menu starts with.
pub const ENGLISH: usize = 2;

/// The tags of the languages, read from the dump of the session.
static TAGS: Mutex<Option<(PathBuf, Vec<Option<Picture>>)>> = Mutex::new(None);

/// Reads the tags when the dump changes. `game_text::set_dump` calls it.
pub fn set_dump(dump: Option<&Dump>) {
    let Ok(mut held) = TAGS.lock() else { return };
    match dump {
        None => *held = None,
        Some(dump) if held.as_ref().is_some_and(|(root, _)| root == dump.root()) => {}
        Some(dump) => *held = Some((dump.root().to_path_buf(), language_tags::load(dump))),
    }
}

/// The texture of the tag of a language, made the first time it is asked.
fn tag_texture(ctx: &egui::Context, language: usize) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("language_tag", language));
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let (width, height, pixels) = TAGS.lock().ok()?.as_ref()?.1.get(language)?.clone()?;
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
    let texture = ctx.load_texture(format!("language_tag_{language}"), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

/// The size of a tag on the screen.
const TAG_SIZE: egui::Vec2 = egui::vec2(30.0, 15.0);

/// Paints the tag of a language in a place (nothing when the dump is not open).
fn paint_tag(ui: &egui::Ui, place: egui::Rect, language: usize) {
    if let Some(texture) = tag_texture(ui.ctx(), language) {
        ui.painter().image(texture.id(), place, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
}

/// Draws the tag of a language (30 × 15 points), or a space of the same size when the dump is not open.
pub fn tag(ui: &mut egui::Ui, language: usize) {
    let (rect, _) = ui.allocate_exact_size(TAG_SIZE, egui::Sense::hover());
    paint_tag(ui, rect, language);
}

/// The order of the languages in the menus: English, then the others in the order of the game.
pub fn order() -> impl Iterator<Item = usize> {
    std::iter::once(ENGLISH).chain((0..LANGUAGES.len()).filter(|i| *i != ENGLISH))
}

/// The menu of the languages: a button with the tag and the name of the chosen language, and a list of the languages with their tags. `width` is the width of
/// the list button (`None`: 190 points). The response says when the choice changed.
pub fn menu(ui: &mut egui::Ui, id: impl std::hash::Hash, language: &mut usize, width: Option<f32>) -> egui::Response {
    let before = *language;
    let mut response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // The tag is inside the button, before the name: the name starts after a space of the width of the tag.
            let chosen = LANGUAGES.get(*language).map_or("?", |l| l.name);
            let mut text = egui::text::LayoutJob::default();
            text.append(chosen, TAG_SIZE.x + 6.0, egui::TextFormat { font_id: egui::TextStyle::Button.resolve(ui.style()), color: ui.visuals().text_color(), ..Default::default() });
            let mut combo = egui::ComboBox::from_id_salt(id).selected_text(text);
            // The longest name (Japanese (kanji)) and the tag fit in the default width.
            combo = combo.width(width.unwrap_or(190.0));
            let combo = combo.show_ui(ui, |ui| {
                for place in order() {
                    ui.horizontal(|ui| {
                        tag(ui, place);
                        ui.selectable_value(language, place, LANGUAGES[place].name);
                    });
                }
            });
            let button = combo.response.rect;
            let place = egui::Rect::from_min_size(egui::pos2(button.left() + 5.0, button.center().y - TAG_SIZE.y / 2.0), TAG_SIZE);
            paint_tag(ui, place, *language);
            // Up and Down change the language when the menu has the focus, in the order of the list.
            let places: Vec<usize> = order().collect();
            let at = places.iter().position(|p| p == language).unwrap_or(0);
            if let Some(to) = crate::focus::nav(ui, &combo.response, at, places.len(), false) {
                *language = places[to];
            }
        })
        .response;
    if *language != before {
        response.mark_changed();
    }
    response
}
