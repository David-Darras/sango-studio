//! The text field of the texts of the game: the text shows in the colours and the sizes that its tags give, a bar of buttons puts
//! the tags around the selected text, and a keyboard gives the characters of the font of the game.
//!
//! The text is in the plain form of `sango_core::text` (`[color sky]Hello[/color]`). The marks in square brackets show small and
//! grey; the user does not need to type them: the buttons of the bar write them.

use crate::theme::color;
use crate::widget;
use eframe::egui;
use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use sango_core::font::{self, GameFont};
use sango_core::text_tags::{self, Paint};
use sango_core::Dump;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// The font of the game, read from the dump of the session.
static FONT: Mutex<Option<(PathBuf, Option<Arc<GameFont>>)>> = Mutex::new(None);

/// Reads the font of the game when the dump changes. The app calls this before it draws the pages.
pub fn set_dump(dump: Option<&Dump>) {
    let Ok(mut held) = FONT.lock() else { return };
    let root = dump.map(|dump| dump.root().to_path_buf());
    match (&*held, &root) {
        (Some((now, _)), Some(root)) if now == root => {}
        (_, None) => *held = None,
        (_, Some(root)) => *held = Some((root.clone(), dump.and_then(|dump| GameFont::open(dump).ok()).map(Arc::new))),
    }
}

fn game_font() -> Option<Arc<GameFont>> {
    FONT.lock().ok()?.as_ref()?.1.clone()
}

/// The pictures of the characters that the keyboard showed, made from the font of the game.
#[derive(Clone, Default)]
struct Glyphs(Arc<Mutex<HashMap<u16, egui::TextureHandle>>>);

fn glyph_texture(ctx: &egui::Context, font: &GameFont, code: u16) -> Option<egui::TextureHandle> {
    let glyphs = ctx.data_mut(|d| d.get_temp_mut_or_default::<Glyphs>(egui::Id::new("game_text_glyphs")).clone());
    let mut map = glyphs.0.lock().ok()?;
    if let Some(texture) = map.get(&code) {
        return Some(texture.clone());
    }
    let glyph = font.glyph(code)?;
    let pixels: Vec<egui::Color32> = glyph.alpha.iter().map(|&a| egui::Color32::from_rgba_unmultiplied(255, 255, 255, a)).collect();
    let image = egui::ColorImage { size: [glyph.width, glyph.height], pixels };
    let texture = ctx.load_texture(format!("game_glyph_{code:04X}"), image, egui::TextureOptions::LINEAR);
    map.insert(code, texture.clone());
    Some(texture)
}

/// The state of the bar and of the keyboard, shared by all the fields.
#[derive(Clone, Default)]
struct Shared {
    /// The field that the bar is for: the last field that had the focus.
    active: Option<egui::Id>,
    /// The field that the keyboard is open for.
    keyboard: Option<egui::Id>,
    /// The chosen group of the keyboard.
    group: usize,
    /// The colours of the buttons "Colour" and "Gradient".
    rgb: [u8; 3],
    top: [u8; 3],
    bottom: [u8; 3],
    /// The size of the button "Size", in percent.
    size: u16,
}

fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| d.get_temp::<Shared>(egui::Id::new("game_text_shared"))).unwrap_or(Shared { rgb: [255, 128, 0], top: [255, 230, 80], bottom: [255, 80, 0], size: 150, ..Shared::default() })
}

fn set_shared(ctx: &egui::Context, value: Shared) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("game_text_shared"), value));
}

fn rgb32(rgb: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// The look of the plain form: the letters in their colour and size, the marks small and grey.
fn layout(ui: &egui::Ui, text: &str, wrap: f32) -> LayoutJob {
    let base = egui::TextStyle::Body.resolve(ui.style());
    let mut job = LayoutJob::default();
    for span in text_tags::spans(text) {
        let piece = &text[span.start..span.end];
        let format = if span.mark {
            let valid = mark_is_valid(piece);
            TextFormat { font_id: egui::FontId::monospace(base.size * 0.72), color: if valid { color::MUTED } else { color::ROSE }, background: if valid { color::SURFACE_STRONG } else { egui::Color32::TRANSPARENT }, ..Default::default() }
        } else {
            let scale = (span.size as f32 / 100.0).clamp(0.6, 1.8);
            let (color, underline) = match span.paint {
                Paint::Normal => (color::TEXT, egui::Stroke::NONE),
                Paint::Color(rgb) => (rgb32(rgb), egui::Stroke::NONE),
                // The field cannot draw a gradient: the letters have the top colour, and a line of the bottom colour.
                Paint::Gradient(top, bottom) => (rgb32(top), egui::Stroke::new(2.0_f32, rgb32(bottom))),
            };
            TextFormat { font_id: egui::FontId::new(base.size * scale, base.family.clone()), color, underline, ..Default::default() }
        };
        job.append(piece, 0.0, format);
    }
    job.wrap.max_width = wrap;
    job
}

/// True when a mark in square brackets is a tag or a character that the game knows.
fn mark_is_valid(mark: &str) -> bool {
    sango_core::text::text_to_chars(mark).is_ok()
}

/// The place in bytes of a place in characters.
fn byte_place(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(at, _)| at)
}

/// The selection of a field, in characters (the start first). Without a saved state, the end of the text.
fn selection(ctx: &egui::Context, id: egui::Id, text: &str) -> (usize, usize) {
    let end = text.chars().count();
    egui::text_edit::TextEditState::load(ctx, id).and_then(|state| state.cursor.char_range()).map_or((end, end), |range| {
        let (a, b) = (range.primary.index.min(end), range.secondary.index.min(end));
        (a.min(b), a.max(b))
    })
}

/// Puts `before` and `after` around the selection of the field (or at the cursor), then selects the text between them again and
/// gives the focus back to the field.
fn wrap_selection(ctx: &egui::Context, id: egui::Id, text: &mut String, before: &str, after: &str) {
    let (start, end) = selection(ctx, id, text);
    let (a, b) = (byte_place(text, start), byte_place(text, end));
    text.insert_str(b, after);
    text.insert_str(a, before);
    let first = start + before.chars().count();
    let last = end + before.chars().count();
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(first), CCursor::new(last))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Puts a text at the cursor of the field, in place of the selection.
fn insert_at_cursor(ctx: &egui::Context, id: egui::Id, text: &mut String, inserted: &str) {
    let (start, end) = selection(ctx, id, text);
    let (a, b) = (byte_place(text, start), byte_place(text, end));
    text.replace_range(a..b, inserted);
    let place = start + inserted.chars().count();
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(place))));
    state.store(ctx, id);
}

/// The plain form of one character of the font: the usual letter, `[[` for `[`, or `[char XXXX]` for a sign of the game.
fn plain_character(code: u16) -> String {
    sango_core::text::chars_to_text(&[code])
}

/// A text field of a text of the game. `rows` is 1 for a name, more for a description; a field of more than one row also has the
/// buttons of the dialogues (new page, wait...). The response says when the text changed.
pub fn edit(ui: &mut egui::Ui, id_salt: impl std::hash::Hash, text: &mut String, rows: usize, width: f32) -> egui::Response {
    let id = ui.make_persistent_id(("game_text", id_salt));
    let ctx = ui.ctx().clone();
    let mut layouter = |ui: &egui::Ui, text: &str, wrap: f32| ui.fonts(|f| f.layout_job(layout(ui, text, wrap)));
    let editor = if rows <= 1 { egui::TextEdit::singleline(text) } else { egui::TextEdit::multiline(text).desired_rows(rows) };
    let output = editor.id(id).desired_width(width).margin(egui::Margin { left: 4.0, right: 26.0, top: 2.0, bottom: 2.0 }).layouter(&mut layouter).show(ui);
    let mut response = output.response;
    // The mark under the pointer: a bubble shows what it is.
    if let Some(pointer) = response.hover_pos() {
        let cursor = output.galley.cursor_from_pos(pointer - output.galley_pos);
        if let Some(mark) = mark_at(text, cursor.ccursor.index) {
            response = response.on_hover_ui_at_pointer(|ui| mark_tooltip(ui, &mark));
        }
    }
    let mut state = shared(&ctx);
    if response.has_focus() && state.active != Some(id) {
        state.active = Some(id);
        set_shared(&ctx, state.clone());
    }
    // The button of the keyboard, inside the field on the right.
    let button = egui::Rect::from_min_size(egui::pos2(response.rect.right() - 24.0, response.rect.top() + 3.0), egui::vec2(20.0, 16.0));
    let keyboard = ui.interact(button, id.with("keyboard_button"), egui::Sense::click()).on_hover_text("Characters of the game");
    paint_keyboard_icon(ui, button, keyboard.hovered() || state.keyboard == Some(id));
    if keyboard.clicked() {
        state.keyboard = if state.keyboard == Some(id) { None } else { Some(id) };
        state.active = Some(id);
        set_shared(&ctx, state.clone());
    }
    let mut changed = false;
    if state.active == Some(id) {
        changed |= format_bar(ui, id, text, rows > 1, &mut state);
    }
    if state.keyboard == Some(id) {
        changed |= keyboard_window(&ctx, id, text, button, &mut state);
    }
    if changed {
        response.mark_changed();
    }
    response
}

/// The mark in square brackets at a place of the text (in characters), when there is one: the letter after the place, or the
/// letter before it.
fn mark_at(text: &str, place: usize) -> Option<String> {
    let after = byte_place(text, place);
    let before = byte_place(text, place.saturating_sub(1));
    text_tags::spans(text).into_iter().find(|span| span.mark && ((span.start <= after && after < span.end) || (span.start <= before && before < span.end))).map(|span| text[span.start..span.end].to_string())
}

/// The bubble of a mark: the picture of a character of the font, or what a tag does.
fn mark_tooltip(ui: &mut egui::Ui, mark: &str) {
    ui.set_max_width(360.0);
    let chars = match sango_core::text::text_to_chars(mark) {
        Ok(chars) => chars,
        Err(message) => {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
    };
    match chars.as_slice() {
        // One character: its picture in the font of the game, large.
        [code] => {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 60.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE);
                match game_font().and_then(|font| glyph_texture(ui.ctx(), &font, *code)) {
                    Some(texture) => {
                        ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), color::TEXT);
                    }
                    None => {
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(28.0), color::MUTED);
                    }
                }
                ui.vertical(|ui| {
                    let name = font::sign_name(*code).unwrap_or("a character of the game");
                    ui.label(egui::RichText::new(name).strong());
                    ui.label(widget::hint(&format!("U+{code:04X}")));
                    if game_font().is_some_and(|font| !font.has(*code)) {
                        ui.label(egui::RichText::new("The font of the game does not have this character.").color(color::ROSE));
                    }
                });
            });
        }
        // A tag: what it does, and its colours.
        [_, _, values @ ..] => {
            ui.label(egui::RichText::new(mark).monospace().strong());
            ui.label(text_tags::describe(values));
            // The colours of a letter after the tag.
            let after = text_tags::spans(&format!("{mark}x"));
            let swatches: Vec<[u8; 3]> = match after.last().map(|span| span.paint) {
                Some(Paint::Color(rgb)) => vec![rgb],
                Some(Paint::Gradient(top, bottom)) => vec![top, bottom],
                _ => Vec::new(),
            };
            if !swatches.is_empty() {
                ui.horizontal(|ui| {
                    for rgb in swatches {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, egui::Rounding::same(4.0), rgb32(rgb));
                        ui.label(widget::hint(&format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])));
                    }
                });
            }
        }
        _ => {}
    }
}

/// A small keyboard: a frame and two rows of keys.
fn paint_keyboard_icon(ui: &egui::Ui, rect: egui::Rect, strong: bool) {
    let painter = ui.painter();
    let ink = if strong { color::ACCENT } else { color::MUTED };
    let body = rect.shrink2(egui::vec2(1.0, 2.0));
    painter.rect_stroke(body, egui::Rounding::same(2.0), egui::Stroke::new(1.2_f32, ink));
    for row in 0..2 {
        for key in 0..4 {
            let at = egui::pos2(body.left() + 3.0 + key as f32 * 3.6, body.top() + 2.6 + row as f32 * 3.0);
            painter.rect_filled(egui::Rect::from_min_size(at, egui::vec2(2.2, 1.8)), 0.0, ink);
        }
    }
    painter.line_segment([egui::pos2(body.left() + 5.0, body.bottom() - 2.4), egui::pos2(body.right() - 5.0, body.bottom() - 2.4)], egui::Stroke::new(1.4_f32, ink));
}

/// The bar of the buttons that put tags around the selection. True when the text changed.
fn format_bar(ui: &mut egui::Ui, id: egui::Id, text: &mut String, dialogue: bool, state: &mut Shared) -> bool {
    let ctx = ui.ctx().clone();
    let before = text.clone();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        // The colours of the game.
        for (name, rgb) in text_tags::GAME_COLORS {
            let (rect, swatch) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
            ui.painter().rect_filled(rect, egui::Rounding::same(4.0), rgb32(rgb));
            if swatch.hovered() {
                ui.painter().rect_stroke(rect.expand(1.5), egui::Rounding::same(5.0), egui::Stroke::new(1.5_f32, color::TEXT));
            }
            if swatch.on_hover_text(format!("Colour of the game: {name}")).clicked() {
                wrap_selection(&ctx, id, text, &format!("[color {name}]"), "[/color]");
            }
        }
        ui.separator();
        // A colour of the plugin.
        egui::color_picker::color_edit_button_srgb(ui, &mut state.rgb).on_hover_text("The colour of the button \"Colour\"");
        if ui.small_button("Colour").on_hover_text("Any colour. Needs the plugin: without it, the text keeps its normal colour.").clicked() {
            let [r, g, b] = state.rgb;
            wrap_selection(&ctx, id, text, &format!("[color #{r:02X}{g:02X}{b:02X}]"), "[/color]");
        }
        ui.separator();
        egui::color_picker::color_edit_button_srgb(ui, &mut state.top).on_hover_text("The colour of the top of the letters");
        egui::color_picker::color_edit_button_srgb(ui, &mut state.bottom).on_hover_text("The colour of the bottom of the letters");
        if ui.small_button("Gradient").on_hover_text("The letters go from the first colour at the top to the second at the bottom. Needs the plugin.").clicked() {
            let ([a, b, c], [d, e, f]) = (state.top, state.bottom);
            wrap_selection(&ctx, id, text, &format!("[gradient #{a:02X}{b:02X}{c:02X} #{d:02X}{e:02X}{f:02X}]"), "[/color]");
        }
        ui.separator();
        ui.add(egui::DragValue::new(&mut state.size).range(text_tags::SIZE_RANGE.0..=text_tags::SIZE_RANGE.1).suffix(" %").speed(1.0));
        if ui.small_button("Size").on_hover_text("The size of the letters. Needs the plugin. A large size can go out of the text box.").clicked() {
            wrap_selection(&ctx, id, text, &format!("[size {}]", state.size), "[/size]");
        }
        ui.separator();
        if ui.small_button("Center").on_hover_text("The line is in the middle of the text box.").clicked() {
            insert_at_cursor(&ctx, id, text, "[center]");
        }
        if ui.small_button("Right").on_hover_text("The line ends at the right side of the text box.").clicked() {
            insert_at_cursor(&ctx, id, text, "[right]");
        }
        if dialogue {
            ui.separator();
            if ui.small_button("New page").on_hover_text("A dialogue: the player presses A, then the text box is cleared.").clicked() {
                insert_at_cursor(&ctx, id, text, "[page]");
            }
            if ui.small_button("Scroll").on_hover_text("A dialogue: the text goes up by one line.").clicked() {
                insert_at_cursor(&ctx, id, text, "[scroll]");
            }
            if ui.small_button("Wait").on_hover_text("A dialogue: the text stops for half a second (30 frames). Change the number in the text.").clicked() {
                insert_at_cursor(&ctx, id, text, "[wait 30]");
            }
        }
        ui.separator();
        if ui.small_button("Clear").on_hover_text("Removes the colours and the sizes of the selected text (of all the text when nothing is selected).").clicked() {
            clear_marks(&ctx, id, text);
        }
    });
    set_shared(&ctx, state.clone());
    *text != before
}

/// Removes the marks of colour and size in the selection (in all the text when nothing is selected).
fn clear_marks(ctx: &egui::Context, id: egui::Id, text: &mut String) {
    let (start, end) = selection(ctx, id, text);
    let (a, b) = if start == end { (0, text.len()) } else { (byte_place(text, start), byte_place(text, end)) };
    let mut kept = String::new();
    for span in text_tags::spans(text) {
        let piece = &text[span.start..span.end];
        let inside = span.start >= a && span.end <= b;
        let styling = ["[color ", "[/color]", "[gradient ", "[size ", "[/size]"].iter().any(|start| piece.starts_with(start));
        if !(span.mark && inside && styling) {
            kept.push_str(piece);
        }
    }
    *text = kept;
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(None);
    state.store(ctx, id);
}

/// The number of characters in a row of the keyboard, and the number of rows that show before the scroll.
const KEYBOARD_COLUMNS: usize = 12;
const KEYBOARD_ROWS: usize = 6;
/// The width of the keyboard: the cells (30 points and the space between them) and the scroll bar.
const KEYBOARD_WIDTH: f32 = KEYBOARD_COLUMNS as f32 * 34.0 + 16.0;

/// The window of the characters of the font of the game. True when the text changed.
fn keyboard_window(ctx: &egui::Context, id: egui::Id, text: &mut String, button: egui::Rect, state: &mut Shared) -> bool {
    let mut open = true;
    let mut changed = false;
    let font = game_font();
    egui::Window::new("Characters of the game").id(id.with("keyboard")).collapsible(false).resizable(false).default_pos(button.left_bottom() + egui::vec2(-KEYBOARD_WIDTH + 20.0, 6.0)).open(&mut open).show(ctx, |ui| {
        ui.set_width(KEYBOARD_WIDTH);
        let Some(font) = font.as_deref() else {
            ui.label(widget::hint("Open the dump of the game: the keyboard shows the characters of its font."));
            return;
        };
        let groups: Vec<(&str, Vec<u16>)> = font::GROUPS.iter().map(|(name, first, last)| (*name, font.characters().into_iter().filter(|c| (*first..=*last).contains(c)).collect::<Vec<u16>>())).filter(|(_, codes)| !codes.is_empty()).collect();
        state.group = state.group.min(groups.len().saturating_sub(1));
        ui.horizontal_wrapped(|ui| {
            for (place, (name, codes)) in groups.iter().enumerate() {
                if ui.selectable_label(state.group == place, *name).on_hover_text(format!("{} characters", codes.len())).clicked() {
                    state.group = place;
                }
            }
        });
        ui.separator();
        let Some((_, codes)) = groups.get(state.group) else { return };
        let columns = KEYBOARD_COLUMNS;
        let rows = codes.len().div_ceil(columns);
        egui::ScrollArea::vertical().id_salt(id.with("keyboard_rows")).max_height(KEYBOARD_ROWS as f32 * 38.0).auto_shrink([false, true]).show_rows(ui, 38.0, rows, |ui, range| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for row in range {
                ui.horizontal(|ui| {
                    for &code in codes.iter().skip(row * columns).take(columns) {
                        let (rect, cell) = ui.allocate_exact_size(egui::vec2(30.0, 34.0), egui::Sense::click());
                        let fill = if cell.hovered() { color::ACCENT_SOFT } else { color::SURFACE };
                        ui.painter().rect_filled(rect, egui::Rounding::same(5.0), fill);
                        if let Some(texture) = glyph_texture(ui.ctx(), font, code) {
                            let picture = egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 30.0));
                            ui.painter().image(texture.id(), picture, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), color::TEXT);
                        }
                        let name = font::sign_name(code).map(|n| format!(" ({n})")).unwrap_or_default();
                        if cell.on_hover_text(format!("U+{code:04X}{name}")).clicked() {
                            insert_at_cursor(ui.ctx(), id, text, &plain_character(code));
                            changed = true;
                        }
                    }
                });
            }
        });
    });
    if !open {
        state.keyboard = None;
    }
    set_shared(ctx, state.clone());
    changed
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_selection_gets_its_tags_and_the_marks_are_removed() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("field");
        let mut text = String::from("Héllo world");
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(5))));
        state.store(&ctx, id);
        wrap_selection(&ctx, id, &mut text, "[color sky]", "[/color]");
        assert_eq!(text, "[color sky]Héllo[/color] world");
        assert_eq!(selection(&ctx, id, &text), (11, 16), "the same letters stay selected");
        insert_at_cursor(&ctx, id, &mut text, &plain_character(0xE08E));
        assert_eq!(text, "[color sky][char E08E][/color] world");
        assert_eq!(mark_at(&text, 12).as_deref(), Some("[char E08E]"));
        assert_eq!(mark_at(&text, 3).as_deref(), Some("[color sky]"));
        assert_eq!(mark_at("Hello", 2), None);
        clear_marks(&ctx, id, &mut text);
        assert_eq!(text, "[char E08E] world");
        assert_eq!(plain_character('[' as u16), "[[");
    }
}
