//! The code editor.
//!
//! The user writes a function in C++, compiles it with the compiler of devkitARM, and the plugin can load the result in
//! memory and run it. The functions are files on the SD card:
//!
//! | File | Content |
//! | --- | --- |
//! | `code/src/<name>.cc` | The source of the function. |
//! | `code/bin/<name>.bin` | The compiled function (see [`compile`] for the format). |
//!
//! The editor has the colours of C++, the numbers of the lines, an indentation that follows the braces, and a completion of
//! the names of `lib/include` of the plugin. The compiler needs the folder of devkitPro (the page Home asks for it).

pub mod compile;
pub mod index;
mod syntax;

use compile::{Output, Severity, Toolchain};
use egui_code_editor::{CodeEditor, ColorTheme};
use index::{Index, Kind, Symbol};
use sango_core::{Backup, SdCard};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

/// The folder of the sources on the SD card, and the folder of the compiled functions.
pub const SOURCE_FOLDER: &str = "code/src";
pub const BINARY_FOLDER: &str = "code/bin";

/// The most names that the completion shows at once.
const COMPLETION_ROWS: usize = 9;

/// The width of one step of indentation, in spaces.
const INDENT: &str = "  ";

/// How many undo steps are kept.
const HISTORY: usize = 20;

/// The path of the source of a function.
pub fn source_path(name: &str) -> String {
    format!("{SOURCE_FOLDER}/{name}.cc")
}

/// The path of the compiled function.
pub fn binary_path(name: &str) -> String {
    format!("{BINARY_FOLDER}/{name}.bin")
}

/// Why a name is refused as the name of a function, or nothing when it is good.
pub fn name_problem(name: &str) -> Option<String> {
    let mut chars = name.chars();
    match chars.next() {
        None => return Some("Type a name.".into()),
        Some(first) if !first.is_ascii_alphabetic() => return Some("The name starts with a letter.".into()),
        _ => {}
    }
    if name.len() > 32 {
        return Some("The name has 32 characters at most.".into());
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("The name has letters, digits and _ only.".into());
    }
    None
}

/// The word that ends at a place of the text: its first character, and the word. A word is letters, digits, `_`, and `::`.
fn word_before(text: &str, at: usize) -> (usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let at = at.min(chars.len());
    let mut start = at;
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '_' || c == ':' {
            start -= 1;
        } else {
            break;
        }
    }
    // A word cannot start with a single colon: `a ? b : c`.
    while start < at && chars[start] == ':' && !(start + 1 < at && chars[start + 1] == ':') {
        start += 1;
    }
    (start, chars[start..at].iter().collect())
}

/// The place of a character in the bytes of a text.
fn byte_of(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map_or(text.len(), |(byte, _)| byte)
}

/// The spaces and the tabs that start the line where a place of the text is.
fn indent_of_line(text: &str, at: usize) -> String {
    let before = &text[..byte_of(text, at)];
    let line = before.rsplit('\n').next().unwrap_or("");
    line.chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

/// The text that a completion puts in place of what the user typed.
pub fn insertion(symbol: &Symbol, typed: &str) -> String {
    if typed.contains("::") {
        // The user wrote the start of the path: complete with the path from there.
        let first = typed.split("::").next().unwrap_or("");
        let parts: Vec<&str> = symbol.qualified.split("::").collect();
        let from = parts.iter().position(|part| part.eq_ignore_ascii_case(first)).unwrap_or(0);
        return parts[from..].join("::");
    }
    if symbol.kind == Kind::EnumValue {
        // A value of an `enum class` is written with the name of the enumeration.
        return symbol.detail.clone();
    }
    symbol.name.clone()
}

/// The list of names that the completion shows.
struct Popup {
    items: Vec<Symbol>,
    selected: usize,
    /// The first character of the word that the completion replaces (a place in characters), and the word.
    start: usize,
    typed: String,
    /// Where the list is drawn: under the cursor.
    anchor: egui::Pos2,
}

pub struct Code {
    /// The names of the functions of the SD card.
    names: Vec<String>,
    /// The chosen function.
    chosen: Option<String>,
    /// The text of the editor, and the text as it is on the SD card.
    text: String,
    saved: String,
    /// The name field of a new function.
    new_name: String,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// What the last compilation said, and for which function.
    output: Option<(String, Output)>,
    /// The names of the headers of the plugin, and the folder that they were read from.
    index: Option<Index>,
    index_for: Option<PathBuf>,
    popup: Option<Popup>,
    /// The completion that the user closed with Escape: it stays closed until the word changes.
    dismissed: Option<(usize, String)>,
    /// The identifier of the text box of the editor, and whether it had the focus.
    editor_id: Option<egui::Id>,
    editor_focus: bool,
    /// A line to show (from 1), after a click on a message of the compiler.
    goto: Option<usize>,
    /// The copies for Undo, the last one at the end.
    history: Vec<Backup>,
    /// The button that the keyboard goes to when the editor lets it go.
    anchor: Option<egui::Id>,
    /// True once the functions of the SD card were listed.
    listed: bool,
}

impl Default for Code {
    fn default() -> Self {
        Code {
            names: Vec::new(),
            chosen: None,
            text: String::new(),
            saved: String::new(),
            new_name: String::new(),
            status: None,
            output: None,
            index: None,
            index_for: None,
            popup: None,
            dismissed: None,
            editor_id: None,
            editor_focus: false,
            goto: None,
            history: Vec::new(),
            anchor: None,
            listed: false,
        }
    }
}

impl Code {
    /// Lists the sources of the SD card again.
    fn list(&mut self, sd: &SdCard) {
        self.names = sd.files_in(SOURCE_FOLDER).into_iter().filter_map(|path| path.rsplit('/').next().and_then(|file| file.strip_suffix(".cc")).map(|name| name.to_string())).collect();
        self.names.sort();
        self.listed = true;
        if self.chosen.as_ref().is_some_and(|name| !self.names.contains(name)) {
            self.chosen = None;
            self.text.clear();
            self.saved.clear();
        }
    }

    /// True when the text of the editor is not on the SD card.
    fn dirty(&self) -> bool {
        self.chosen.is_some() && self.text != self.saved
    }

    /// Writes the text of the function on the SD card.
    fn save(&mut self, sd: &SdCard) -> bool {
        let Some(name) = self.chosen.clone() else { return false };
        match sd.write(&source_path(&name), self.text.as_bytes()) {
            Ok(_) => {
                self.saved = self.text.clone();
                true
            }
            Err(message) => {
                self.status = Some((false, message));
                false
            }
        }
    }

    /// Shows a function in the editor (the one that was open is saved first).
    fn choose(&mut self, sd: &SdCard, name: &str) {
        if self.dirty() {
            self.save(sd);
        }
        match sd.read(&source_path(name)).map(|bytes| String::from_utf8_lossy(&bytes).to_string()) {
            Some(text) => {
                self.chosen = Some(name.to_string());
                self.saved = text.clone();
                self.text = text;
                self.popup = None;
                self.status = None;
            }
            None => self.status = Some((false, format!("The file of {name} is not on the SD card."))),
        }
    }

    /// Makes a new function from the template and opens it.
    fn create(&mut self, sd: &SdCard) {
        let name = self.new_name.trim().to_string();
        if let Some(problem) = name_problem(&name) {
            self.status = Some((false, problem));
            return;
        }
        if self.names.contains(&name) {
            self.status = Some((false, format!("There is already a function {name}.")));
            return;
        }
        if let Err(message) = sd.write(&source_path(&name), compile::template(&name).as_bytes()) {
            self.status = Some((false, message));
            return;
        }
        self.new_name.clear();
        self.list(sd);
        self.choose(sd, &name);
    }

    /// Removes the function that is open. Undo gives it back.
    fn delete(&mut self, sd: &SdCard) {
        let Some(name) = self.chosen.clone() else { return };
        let backup = Backup::snapshot(sd, &["code"], &[], format!("Delete the function {name}"));
        let result = sd.remove(&source_path(&name)).and_then(|_| sd.remove(&binary_path(&name)));
        match result {
            Ok(_) => {
                self.history.push(backup);
                if self.history.len() > HISTORY {
                    self.history.remove(0);
                }
                self.chosen = None;
                self.text.clear();
                self.saved.clear();
                self.output = None;
                self.list(sd);
                self.status = Some((true, format!("Deleted {name}. Undo gives it back.")));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Gives back the last deleted function.
    fn undo(&mut self, sd: &SdCard) {
        let Some(backup) = self.history.pop() else { return };
        self.status = Some(match backup.restore(sd) {
            Ok(count) => (true, format!("Undone: {}. {count} file(s) put back.", backup.label)),
            Err(message) => (false, message),
        });
        self.list(sd);
        if let Some(name) = self.chosen.clone() {
            self.choose(sd, &name);
        }
    }

    /// Saves and compiles the function that is open, and puts the result on the SD card.
    fn compile(&mut self, session: &Session) {
        let (Some(sd), Some(name)) = (session.sd.as_ref(), self.chosen.clone()) else { return };
        let Some(devkitpro) = session.devkitpro.as_ref() else {
            self.status = Some((false, "Give the folder of devkitPro in Home first.".into()));
            return;
        };
        if !self.save(sd) {
            return;
        }
        let tool = match Toolchain::new(devkitpro, session.plugin.as_deref()) {
            Ok(tool) => tool,
            Err(message) => {
                self.status = Some((false, message));
                return;
            }
        };
        let output = tool.compile(&name, &self.text);
        self.status = Some(match &output.file {
            Some(file) => match sd.write(&binary_path(&name), file).and_then(|_| sd.mark_changed(&binary_path(&name))) {
                Ok(()) => (true, format!("Compiled {name}: {} bytes in {}.", file.len(), binary_path(&name))),
                Err(message) => (false, message),
            },
            None => (false, format!("{name} did not compile: {} error(s).", output.count(Severity::Error))),
        });
        self.output = Some((name, output));
    }

    /// Reads the headers of the plugin when the folder is new.
    fn read_headers(&mut self, session: &Session, force: bool) {
        let wanted = session.plugin.as_ref().map(|plugin| plugin.join("lib").join("include"));
        if !force && self.index_for == wanted {
            return;
        }
        self.index = wanted.as_deref().map(Index::build);
        self.index_for = wanted;
    }

    /// The keys that the editor must not give to the text box: the completion list, the indentation of a new line, the Tab.
    /// They change the text and the cursor by hand.
    fn intercept_keys(&mut self, ctx: &egui::Context) {
        if !self.editor_focus {
            return;
        }
        let Some(id) = self.editor_id else { return };
        let Some(mut state) = egui::TextEdit::load_state(ctx, id) else { return };
        let Some(range) = state.cursor.char_range() else { return };
        let (low, high) = (range.primary.index.min(range.secondary.index), range.primary.index.max(range.secondary.index));

        // The list of names.
        if let Some(popup) = self.popup.as_mut() {
            let (down, up, accept, escape) = ctx.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
            if down {
                popup.selected = (popup.selected + 1) % popup.items.len();
            }
            if up {
                popup.selected = (popup.selected + popup.items.len() - 1) % popup.items.len();
            }
            if escape {
                self.dismissed = Some((popup.start, popup.typed.clone()));
                self.popup = None;
            } else if accept {
                let place = popup.selected;
                self.accept(ctx, place);
            }
            return;
        }

        // Enter: a new line with the indentation of the line, one step more after `{`.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            let before: String = self.text.chars().take(low).collect();
            let mut indent = indent_of_line(&self.text, low);
            let opens = before.trim_end_matches([' ', '\t']).ends_with('{');
            let after_char = self.text.chars().nth(high);
            if opens {
                indent.push_str(INDENT);
            }
            let mut insert = format!("\n{indent}");
            let cursor = low + insert.chars().count();
            // `{|}` becomes the braces around an empty line.
            if opens && after_char == Some('}') {
                let outer = indent_of_line(&self.text, low);
                insert.push('\n');
                insert.push_str(&outer);
            }
            self.replace(ctx, id, &mut state, low, high, &insert, cursor);
            return;
        }
        // Tab: a step of indentation.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            self.replace(ctx, id, &mut state, low, high, INDENT, low + INDENT.chars().count());
            return;
        }
        // `}` at the start of a line takes the line back one step.
        let typed_brace = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "}")));
        if typed_brace && low == high {
            let before = &self.text[..byte_of(&self.text, low)];
            let line = before.rsplit('\n').next().unwrap_or("");
            if line.len() >= INDENT.len() && line.chars().all(|c| c == ' ') {
                let from = low - INDENT.len();
                self.replace(ctx, id, &mut state, from, low, "", from);
            }
        }
    }

    /// Replaces the characters from `from` to `to` by a text, and puts the cursor at `cursor` (places in characters).
    #[allow(clippy::too_many_arguments)]
    fn replace(&mut self, ctx: &egui::Context, id: egui::Id, state: &mut egui::text_edit::TextEditState, from: usize, to: usize, insert: &str, cursor: usize) {
        let (start, end) = (byte_of(&self.text, from), byte_of(&self.text, to));
        self.text.replace_range(start..end, insert);
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(cursor))));
        state.clone().store(ctx, id);
    }

    /// Puts a name of the list in the text, in place of the word that the user typed.
    fn accept(&mut self, ctx: &egui::Context, place: usize) {
        let (Some(popup), Some(id)) = (self.popup.take(), self.editor_id) else { return };
        let Some(symbol) = popup.items.get(place) else { return };
        let Some(mut state) = egui::TextEdit::load_state(ctx, id) else { return };
        let end = state.cursor.char_range().map_or(popup.start + popup.typed.chars().count(), |r| r.primary.index.max(r.secondary.index));
        let insert = insertion(symbol, &popup.typed);
        let cursor = popup.start + insert.chars().count();
        self.replace(ctx, id, &mut state, popup.start, end, &insert, cursor);
        self.dismissed = Some((popup.start, insert));
    }

    /// Looks at the word before the cursor and makes the list of the names that go with it.
    fn update_completion(&mut self, out: &egui::text_edit::TextEditOutput) {
        let cursor_range = out.cursor_range;
        let (Some(index), true, Some(range)) = (self.index.as_ref(), out.response.has_focus(), cursor_range) else {
            self.popup = None;
            return;
        };
        if range.primary.ccursor.index != range.secondary.ccursor.index {
            self.popup = None;
            return;
        }
        let at = range.primary.ccursor.index;
        let (start, word) = word_before(&self.text, at);
        // Not in a comment or a string, and not for one letter.
        let line_start = self.text[..byte_of(&self.text, at)].rfind('\n').map_or(0, |p| p + 1);
        let line = &self.text[line_start..byte_of(&self.text, at)];
        let in_comment = line.contains("//");
        let in_string = line.matches('"').count() % 2 == 1;
        if word.chars().count() < 2 && !word.ends_with("::") || in_comment || in_string || self.dismissed.as_ref().is_some_and(|(s, w)| *s == start && w == &word) {
            self.popup = None;
            return;
        }
        let items: Vec<Symbol> = index.complete(&word, COMPLETION_ROWS).into_iter().cloned().collect();
        // Nothing to add: the word is already the only name that goes with it.
        if items.is_empty() || items.len() == 1 && insertion(&items[0], &word) == word {
            self.popup = None;
            return;
        }
        let anchor = out.galley_pos + out.galley.pos_from_ccursor(range.primary.ccursor).left_bottom().to_vec2() + egui::vec2(0.0, 4.0);
        let selected = match &self.popup {
            Some(popup) if popup.typed == word && popup.items == items => popup.selected.min(items.len() - 1),
            _ => 0,
        };
        self.popup = Some(Popup { items, selected, start, typed: word, anchor });
    }

    /// Draws the list of names under the cursor. A click on a name puts it in the text.
    fn draw_popup(&mut self, ctx: &egui::Context) {
        let Some(popup) = self.popup.as_ref() else { return };
        let mut clicked = None;
        egui::Area::new(egui::Id::new("code_completion")).order(egui::Order::Foreground).fixed_pos(popup.anchor).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(520.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                for (place, symbol) in popup.items.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(500.0, 20.0), egui::Sense::click());
                    if place == popup.selected {
                        ui.painter().rect_filled(rect, 4.0, color::ACCENT_SOFT);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 4.0, color::SURFACE_STRONG);
                    }
                    ui.painter().text(egui::pos2(rect.left() + 6.0, rect.center().y), egui::Align2::LEFT_CENTER, &symbol.name, egui::FontId::monospace(13.0), color::TEXT);
                    ui.painter().text(egui::pos2(rect.left() + 230.0, rect.center().y), egui::Align2::LEFT_CENTER, symbol.kind.label(), egui::FontId::proportional(11.5), color::ACCENT);
                    ui.painter().text(egui::pos2(rect.right() - 6.0, rect.center().y), egui::Align2::RIGHT_CENTER, &symbol.header, egui::FontId::proportional(11.5), color::MUTED);
                    if response.clicked() {
                        clicked = Some(place);
                    }
                }
                if let Some(symbol) = popup.items.get(popup.selected) {
                    ui.separator();
                    let detail: String = symbol.detail.chars().take(160).collect();
                    ui.label(egui::RichText::new(detail).monospace().size(12.0));
                    if !symbol.doc.is_empty() {
                        ui.label(widget::hint(&symbol.doc));
                    }
                }
            });
        });
        if let Some(place) = clicked {
            self.accept(ctx, place);
        }
    }

    /// Puts the cursor at the start of a line of the text.
    fn go_to_line(&mut self, ctx: &egui::Context, line: usize) {
        let Some(id) = self.editor_id else { return };
        let mut chars = 0;
        for (number, text) in self.text.split('\n').enumerate() {
            if number + 1 >= line {
                break;
            }
            chars += text.chars().count() + 1;
        }
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(chars))));
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// The list of the functions, the name field of a new function.
    fn side(&mut self, ui: &mut egui::Ui, sd: &SdCard) {
        widget::card(ui, |ui| {
            ui.set_width(210.0);
            ui.label(widget::title("Functions"));
            ui.add_space(4.0);
            if self.names.is_empty() {
                ui.label(widget::hint("No function yet."));
            }
            for name in self.names.clone() {
                let selected = self.chosen.as_deref() == Some(name.as_str());
                let mark = if selected && self.dirty() { format!("{name} •") } else { name.clone() };
                if ui.selectable_label(selected, mark).clicked() && !selected {
                    self.choose(sd, &name);
                }
            }
            ui.add_space(10.0);
            ui.label(widget::hint("New function"));
            let edit = ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(180.0).hint_text("Boost"));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let problem = name_problem(self.new_name.trim());
            ui.add_enabled_ui(problem.is_none(), |ui| {
                if widget::button(ui, "Create") || (enter && problem.is_none()) {
                    self.create(sd);
                }
            });
            if let Some(problem) = problem.filter(|_| !self.new_name.is_empty()) {
                ui.label(widget::hint(&problem));
            }
        });
    }

    /// The buttons above the editor.
    fn toolbar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let Some(sd) = session.sd.as_ref() else { return };
        let can_compile = session.devkitpro.is_some();
        let mut save = false;
        let mut compile = false;
        let mut delete = false;
        let mut undo = false;
        ui.horizontal_wrapped(|ui| {
            if let Some(name) = &self.chosen {
                ui.label(widget::title(name));
            }
            let response = ui.add_enabled(self.chosen.is_some(), egui::Button::new("Save"));
            self.anchor = Some(response.id);
            widget::register_focusable(ui, response.id, response.rect);
            save = response.clicked();
            ui.add_enabled_ui(self.chosen.is_some() && can_compile, |ui| {
                compile = widget::button_styled(ui, egui::RichText::new("Compile").color(color::WHITE), Some(color::ACCENT));
            })
            .response
            .on_hover_text(if can_compile { "Saves the function and compiles it with devkitARM (F5)." } else { "Give the folder of devkitPro in Home first." });
            ui.add_enabled_ui(self.chosen.is_some(), |ui| {
                delete = widget::button(ui, "Delete");
            });
            let undo_label = self.history.last().map(|b| format!("Undo: {}", b.label));
            ui.add_enabled_ui(undo_label.is_some(), |ui| {
                undo = widget::button(ui, "Undo");
            })
            .response
            .on_hover_text(undo_label.unwrap_or_else(|| "Nothing to undo.".into()));
            if self.dirty() {
                widget::badge(ui, "Not saved", color::AMBER);
            }
            if widget::button(ui, "Read the headers again") {
                self.read_headers(session, true);
            }
            let count = self.index.as_ref().map_or(0, |i| i.symbols.len());
            ui.label(widget::hint(&if count > 0 { format!("{count} names for the completion") } else { "No completion: give the folder of the plugin in Home".to_string() }));
        });
        if save {
            self.save(sd);
            self.status = Some((true, "Saved.".into()));
        }
        if compile {
            self.compile(session);
        }
        if delete {
            self.delete(sd);
        }
        if undo {
            self.undo(sd);
        }
    }

    /// The editor, and the completion.
    fn editor(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.intercept_keys(&ctx);
        let syntax = syntax::cpp(self.index.as_ref());
        let rows = ((ui.available_height() - 210.0) / 18.0).clamp(10.0, 60.0) as usize;
        let mut editor = CodeEditor::default().id_source("code_editor").with_fontsize(14.0).with_theme(ColorTheme::GITHUB_LIGHT).with_syntax(syntax).with_numlines(true).with_rows(rows).vscroll(true);
        let out = editor.show(ui, &mut self.text);
        let id = out.response.id;
        self.editor_id = Some(id);
        self.editor_focus = out.response.has_focus();

        // The editor is a stop of the keyboard of the page, and it keeps the arrows for the text.
        widget::register_focusable(ui, id, out.response.rect);
        ctx.data_mut(|d| d.insert_temp(id.with("handles_arrows"), true));
        if out.response.clicked() || out.response.gained_focus() {
            widget::set_active_id(ui, id);
        }
        if widget::is_active(ui, id) && widget::page_has_focus(ui) {
            if !out.response.has_focus() {
                out.response.request_focus();
            }
            // Escape lets the keyboard go to the buttons.
            if out.response.has_focus() && self.popup.is_none() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                out.response.surrender_focus();
                if let Some(anchor) = self.anchor {
                    widget::set_active_id(ui, anchor);
                }
            }
        } else if out.response.has_focus() {
            out.response.surrender_focus();
        }
        self.update_completion(&out);
        self.draw_popup(&ctx);
        if let Some(line) = self.goto.take() {
            self.go_to_line(&ctx, line);
        }
    }

    /// What the last compilation said: the messages, with their line, and the whole text of the compiler.
    fn output_panel(&mut self, ui: &mut egui::Ui) {
        widget::card(ui, |ui| {
            ui.set_min_width(ui.available_width() - 8.0);
            let Some((name, output)) = &self.output else {
                ui.label(widget::hint("The messages of the compiler show here."));
                return;
            };
            let (errors, warnings) = (output.count(Severity::Error), output.count(Severity::Warning));
            ui.horizontal(|ui| {
                ui.label(widget::title(&format!("Compilation of {name}")));
                if output.is_ok() {
                    widget::badge(ui, "Compiled", color::GREEN);
                } else {
                    widget::badge(ui, "Failed", color::ROSE);
                }
                ui.label(widget::hint(&format!("{errors} error(s), {warnings} warning(s)")));
            });
            let mut goto = None;
            egui::ScrollArea::vertical().id_salt("code_messages").max_height(150.0).auto_shrink([false, true]).show(ui, |ui| {
                for diagnostic in &output.diagnostics {
                    ui.horizontal_wrapped(|ui| {
                        let (text, ink) = match diagnostic.severity {
                            Severity::Error => ("error", color::ROSE),
                            Severity::Warning => ("warning", color::AMBER),
                            Severity::Note => ("note", color::MUTED),
                        };
                        ui.label(egui::RichText::new(text).color(ink).strong());
                        if diagnostic.line > 0 && ui.link(format!("line {}:{}", diagnostic.line, diagnostic.column)).clicked() {
                            goto = Some(diagnostic.line);
                        }
                        ui.label(&diagnostic.message);
                    });
                }
                if output.diagnostics.is_empty() && output.is_ok() {
                    ui.label(widget::hint("No message."));
                }
            });
            if goto.is_some() {
                self.goto = goto;
            }
        });
    }
}

impl Module for Code {
    fn title(&self) -> &str {
        "Code"
    }

    fn icon(&self) -> &str {
        "C"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        ui.add_space(8.0);
        ui.heading("Code");
        ui.label(widget::hint("Write a function in C++, compile it, and the plugin can load it and run it. The functions are on the SD card, in the folder code."));
        ui.add_space(8.0);
        let Some(sd) = session.sd.clone() else {
            widget::missing_card(ui, "Choose the SD card folder in Home first.");
            return;
        };
        let session: &Session = session;
        if !self.listed {
            self.list(&sd);
        }
        self.read_headers(session, false);
        // Ctrl+S saves and F5 compiles, when the page has the keyboard.
        if widget::page_has_focus(ui) {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) && self.chosen.is_some() {
                self.save(&sd);
                self.status = Some((true, "Saved.".into()));
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) && self.chosen.is_some() && session.devkitpro.is_some() {
                self.compile(session);
            }
        }
        if session.devkitpro.is_none() {
            widget::card(ui, |ui| {
                widget::badge(ui, "devkitPro missing", color::AMBER);
                ui.label(widget::hint("The editor works, but Compile needs the folder of devkitPro: give it in Home."));
            });
            ui.add_space(8.0);
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| self.side(ui, &sd));
            ui.vertical(|ui| {
                self.toolbar(ui, session);
                if let Some((ok, message)) = &self.status {
                    ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
                }
                ui.add_space(4.0);
                if self.chosen.is_some() {
                    self.editor(ui);
                    ui.add_space(6.0);
                    self.output_panel(ui);
                } else {
                    widget::card(ui, |ui| {
                        ui.label(widget::hint("Choose a function on the left, or give a name to a new one."));
                    });
                }
            });
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_word_is_cut_before_the_cursor() {
        assert_eq!(word_before("int x = Move", 12), (8, "Move".to_string()));
        assert_eq!(word_before("pokemon::MoveData::Get", 22), (0, "pokemon::MoveData::Get".to_string()));
        assert_eq!(word_before("a ? b : c", 7), (7, String::new()));
        assert_eq!(word_before("x = ItemId::", 12), (4, "ItemId::".to_string()));
        assert_eq!(word_before("héllo wörld", 11), (6, "wörld".to_string()));
        assert_eq!(indent_of_line("void f() {\n    int a;", 19), "    ");
        assert_eq!(indent_of_line("void f() {\n    int a;", 3), "");
    }

    #[test]
    fn the_names_of_functions_are_checked() {
        assert!(name_problem("Boost").is_none() && name_problem("a_1").is_none());
        for bad in ["", "1a", "a b", "a-b", "é", &"x".repeat(33)] {
            assert!(name_problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn a_completion_puts_the_right_text() {
        let value = Symbol { name: "kDamage".into(), qualified: "pokemon::MoveKind::kDamage".into(), kind: Kind::EnumValue, detail: "MoveKind::kDamage".into(), header: "x.h".into(), doc: String::new() };
        assert_eq!(insertion(&value, "kDam"), "MoveKind::kDamage");
        assert_eq!(insertion(&value, "MoveKind::kD"), "MoveKind::kDamage");
        assert_eq!(insertion(&value, "pokemon::MoveKind::"), "pokemon::MoveKind::kDamage");
        let function = Symbol { name: "GetInstance".into(), qualified: "pokemon::MoveData::GetInstance".into(), kind: Kind::Function, detail: String::new(), header: "x.h".into(), doc: String::new() };
        assert_eq!(insertion(&function, "GetIn"), "GetInstance");
        assert_eq!(insertion(&function, "MoveData::Get"), "MoveData::GetInstance");
    }

    /// With the SD card: a function is made, edited, saved, and deleted with its undo.
    #[test]
    fn a_function_is_made_saved_deleted_and_given_back() {
        let folder = std::env::temp_dir().join(format!("sango_code_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut page = Code::default();
        page.list(&sd);
        page.new_name = "Boost".into();
        page.create(&sd);
        assert_eq!(page.names, ["Boost"]);
        assert!(page.text.contains("sango_run") && !page.dirty());
        page.text.push_str("// changed\n");
        assert!(page.dirty());
        assert!(page.save(&sd));
        assert!(sd.read(&source_path("Boost")).unwrap().ends_with(b"// changed\n"));
        page.new_name = "Boost".into();
        page.create(&sd);
        assert!(page.status.as_ref().is_some_and(|(ok, m)| !ok && m.contains("already")));
        page.delete(&sd);
        assert!(page.names.is_empty() && sd.read(&source_path("Boost")).is_none());
        page.undo(&sd);
        assert_eq!(page.names, ["Boost"]);
        assert!(sd.read(&source_path("Boost")).is_some());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With devkitPro and the plugin (`SANGO_DEVKITPRO`, `SANGO_PLUGIN`): the page draws, a function with a header of the
    /// plugin is compiled and written, and the completion finds names of the headers.
    #[test]
    fn the_page_draws_and_compiles_a_function() {
        let (Ok(devkitpro), Ok(plugin)) = (std::env::var("SANGO_DEVKITPRO"), std::env::var("SANGO_PLUGIN")) else { return };
        let folder = std::env::temp_dir().join(format!("sango_code_compile_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        // The fields are set by hand: the `set_` functions would write the settings of the user.
        let mut session = Session { sd: Some(SdCard::open(folder.to_str().unwrap()).unwrap()), devkitpro: Some(PathBuf::from(&devkitpro)), plugin: Some(PathBuf::from(&plugin)), ..Session::default() };
        let sd = session.sd.clone().unwrap();
        let mut page = Code::default();
        let ctx = egui::Context::default();
        let frame = |page: &mut Code, session: &mut Session| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        frame(&mut page, &mut session);
        page.new_name = "Power".into();
        page.create(&sd);
        page.text = "#include \"common.h\"\n#include \"pokemon/native/move_data.h\"\n\nextern \"C\" u32 sango_run(u32 argument) {\n  pokemon::MoveData& data = pokemon::MoveData::GetInstance(static_cast<MoveId>(argument));\n  data.power = 90;\n  return data.power;\n}\n".into();
        frame(&mut page, &mut session);
        page.compile(&session);
        assert!(page.output.as_ref().is_some_and(|(_, o)| o.is_ok()), "{:?}", page.output.as_ref().map(|(_, o)| &o.diagnostics));
        let file = sd.read(&binary_path("Power")).expect("the compiled function is on the SD card");
        let (info, image) = compile::read_container(&file).unwrap();
        assert_eq!(info.name, "Power");
        assert!(!image.is_empty());

        // A mistake is reported with its line, and the old binary stays.
        page.text = "extern \"C\" unsigned sango_run(unsigned a) {\n  return nothing;\n}\n".into();
        page.compile(&session);
        let (_, output) = page.output.as_ref().unwrap();
        assert!(!output.is_ok() && output.diagnostics.iter().any(|d| d.line == 2 && d.severity == Severity::Error));
        assert!(sd.read(&binary_path("Power")).is_some());
        frame(&mut page, &mut session);
        assert!(page.index.as_ref().is_some_and(|i| i.complete("GetInst", 3).iter().any(|s| s.name == "GetInstance")));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
