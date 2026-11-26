//! The Script page: the scripts of the game, as assembly that can be changed and built again.
//!
//! The game runs compiled scripts (the Pawn language). The page opens a script file of the game, shows its code as assembly (every jump has a
//! name, so the user can add and remove instructions), checks the text, builds the file again and writes it on the SD card (`script/<kind>/<n>.bin`,
//! compressed as in the archive). The plugin reads that file instead of the file of the game. The page also lists the scripts that the file holds, the
//! native functions that it calls, and the whole list of the native functions with what each one does and what it takes.

use crate::amx::{self, Amx};
use crate::highlight::{self, Mode};
use crate::{assembly, decompile, files, natives};
use sango_core::{Dump, Garc, SdCard};
use sango_ui::picker::{picker, Group};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashMap;

/// The kinds of script files: the archive, the folder of the SD card and the name.
const KINDS: [(usize, &str, &str); 3] = [(29, "field", "Field scripts"), (7, "ai", "Trainer battle AI"), (89, "amie", "Pokémon-Amie")];

/// The tabs of the page.
const TABS: [&str; 5] = ["Code", "Scripts", "Natives used", "Native functions", "Create a script"];

/// The place of the tab that makes scripts from blocks.
const CREATE_TAB: usize = 4;

/// A script file that the user can open.
struct Item {
    kind: usize,
    index: usize,
    label: String,
    group: &'static str,
}

/// The page.
pub struct ScriptPage {
    /// Undo and redo of the text of the open script.
    history: sango_ui::history::History<String>,
    /// The window of the button Reset.
    reset_open: bool,
    root: Option<std::path::PathBuf>,
    error: Option<String>,
    garcs: Vec<Option<Garc>>,
    items: Vec<Item>,
    chosen: usize,
    tab: usize,
    /// The text of the open script, and the text as it was loaded or saved.
    text: String,
    baseline: String,
    /// The script read from the text the last time that it was checked.
    built: Option<Amx>,
    status: Option<(bool, String)>,
    search: String,
    names: HashMap<u32, String>,
    /// The lines of the Pawn-like text with the address of each one, the address of each line of the assembly, and the text of the assembly
    /// that they were made from.
    pawn: Vec<(String, Option<usize>)>,
    assembly_lines: Vec<Option<usize>>,
    pawn_from: String,
    pawn_error: Option<String>,
    /// The address of the instruction that the user chose (a click in the assembly or in the Pawn text): the two sides show it.
    selected: Option<usize>,
    /// The side that must scroll to the choice at the next frame.
    scroll_pawn: bool,
    scroll_assembly: bool,
    /// The line of the cursor of the assembly at the last frame: a new line is a new choice.
    cursor_line: Option<usize>,
    /// The tab "Create a script".
    creator: crate::create::Creator,
}

impl Default for ScriptPage {
    fn default() -> Self {
        ScriptPage { history: Default::default(), reset_open: false, root: None, error: None, garcs: Vec::new(), items: Vec::new(), chosen: 0, tab: 0, text: String::new(), baseline: String::new(), built: None, status: None, search: String::new(), names: HashMap::new(), pawn: Vec::new(), assembly_lines: Vec::new(), pawn_from: String::new(), pawn_error: None, selected: None, scroll_pawn: false, scroll_assembly: false, cursor_line: None, creator: Default::default() }
    }
}

fn path(kind: usize, index: usize) -> String {
    format!("script/{}/{index}.bin", KINDS[kind].1)
}

/// The name that a group of native functions has in the page.
fn group_name(group: &str) -> &str {
    match group {
        "Common" => "General",
        "Init" => "Data, flags and time",
        "ProgField" => "Secret Bases and Berries",
        "BattleInst" => "Battle facilities",
        "Fashion" => "Fashion",
        "Gimmick" => "Map devices",
        "Interactive" => "Interactive characters",
        "AI" => "Characters moved by the AI",
        "Pokecen" => "Pokémon Center",
        "Prog" => "Engine",
        "Item" => "Items",
        other => other,
    }
}

/// The name of a native with its parameters: `WordSetNumber(buffer, number, digits, display)`.
fn signature(native: &natives::Native) -> String {
    if !native.known {
        return format!("{}(...)", native.alias);
    }
    format!("{}({})", native.alias, native.params.iter().map(|p| p.name).collect::<Vec<_>>().join(", "))
}

/// A list of values: the number, the name of the enumerator of the plugin and what it means.
fn values_list(ui: &mut egui::Ui, what: &str, list: &natives::ValueList, id: (&str, &str)) {
    let title = if list.enums.is_empty() { format!("      {what} ({})", list.items.len()) } else { format!("      {what} ({}, {})", list.items.len(), list.enums) };
    egui::CollapsingHeader::new(egui::RichText::new(title).size(13.0).color(color::MUTED)).id_salt(id).show(ui, |ui| {
        for (value, name, text) in list.items {
            let line = if name.is_empty() { format!("{value} — {text}") } else { format!("{value} — {name} — {text}") };
            ui.label(egui::RichText::new(line).monospace().size(12.5));
        }
    });
}

/// A native: its name with the parameters, what it does, and what each parameter takes.
fn native_card(ui: &mut egui::Ui, native: &natives::Native, calls: Option<usize>) {
    ui.label(egui::RichText::new(signature(native)).monospace().strong());
    if native.alias != native.name {
        ui.label(widget::hint(&format!("Name in the game: {}. Both names work in the assembler.", native.name)));
    }
    ui.label(widget::hint(if native.help.is_empty() { "No description yet." } else { native.help }));
    if !native.known {
        ui.label(widget::hint("The parameters of this function are not documented."));
    } else if native.params.is_empty() {
        ui.label(widget::hint("Takes no parameter."));
    }
    for param in native.params {
        let line = if param.help.is_empty() { format!("   {} — {}", param.name, param.kind) } else { format!("   {} — {}. {}", param.name, param.kind, param.help) };
        ui.label(egui::RichText::new(line).size(13.0));
        if let Some(list) = natives::list(param.list) {
            values_list(ui, &format!("values of {}", param.name), list, (native.name, param.name));
        }
    }
    if !native.returns.is_empty() {
        ui.label(egui::RichText::new(format!("   returns — {}", native.returns)).size(13.0));
    }
    if let Some(list) = natives::list(native.returns_list) {
        values_list(ui, "values that it gives", list, (native.name, "@return"));
    }
    if let Some(calls) = calls {
        ui.label(widget::hint(&format!("The scripts of the game call it {calls} time(s).")));
    }
    ui.add_space(6.0);
}

impl ScriptPage {
    fn open(&mut self, dump: &Dump) {
        self.error = None;
        self.garcs = KINDS.iter().map(|(archive, _, _)| Garc::open(&dump.archive(*archive)).ok()).collect();
        self.names = natives::display_names();
        self.items.clear();
        // The field scripts, in the order of their numbers.
        for file in files::FILES {
            let label = if file.help.is_empty() { format!("Scripts {} to {}", file.first, file.last) } else { format!("{} (scripts {} to {})", file.help, file.first, file.last) };
            self.items.push(Item { kind: 0, index: file.index, label, group: KINDS[0].2 });
        }
        for (kind, (_, _, group)) in KINDS.iter().enumerate().skip(1) {
            let Some(garc) = self.garcs[kind].as_ref() else { continue };
            for index in 0..garc.file_count() {
                let is_script = garc.first_file_decompressed(index).is_some_and(|b| b.len() > 8 && u16::from_le_bytes([b[4], b[5]]) == amx::MAGIC);
                if is_script {
                    self.items.push(Item { kind, index, label: format!("{group} {}", index + 1), group });
                }
            }
        }
        self.chosen = 0;
        self.history.clear();
        self.text.clear();
        self.built = None;
    }

    /// Reads the open script: the SD card first, then the game.
    fn load(&mut self, sd: Option<&SdCard>) {
        let Some(item) = self.items.get(self.chosen) else { return };
        let (kind, index) = (item.kind, item.index);
        let bytes = sd.and_then(|sd| sd.read(&path(kind, index))).map(|b| if sango_core::lz::is_compressed(&b) { sango_core::lz::decompress(&b).unwrap_or_default() } else { b }).or_else(|| self.garcs[kind].as_ref().and_then(|g| g.first_file_decompressed(index)));
        let names = &self.names;
        match bytes.map(|b| Amx::parse(&b)) {
            // The assembly has the names of the Pawn-like text: the functions and the variables.
            Some(Ok(amx)) => match assembly::to_text_named(&amx, &|h| names.get(&h).cloned(), crate::decompile::names(&amx, &|h| names.get(&h).cloned()).ok().as_ref()) {
                Ok(text) => {
                    self.baseline = text.clone();
                    self.text = text;
                    self.built = Some(amx);
                    self.status = None;
                }
                Err(message) => self.fail(message),
            },
            Some(Err(message)) => self.fail(message),
            None => self.fail("This script is not in the archive.".to_string()),
        }
    }

    fn fail(&mut self, message: String) {
        self.text.clear();
        self.baseline.clear();
        self.built = None;
        self.status = Some((false, message));
    }

    fn dirty(&self) -> bool {
        self.text != self.baseline
    }

    /// Reads the text: tells if it is right, and keeps the script for the lists.
    fn check(&mut self) -> Option<Amx> {
        match assembly::from_text(&self.text) {
            Ok(amx) => {
                self.status = Some((true, format!("The text is right: {} cells of code, {} cells of data, {} natives.", amx.code.len(), amx.data.len(), amx.natives.len())));
                self.built = Some(amx.clone());
                Some(amx)
            }
            Err(message) => {
                self.status = Some((false, message));
                None
            }
        }
    }

    fn save(&mut self, sd: &SdCard) {
        let Some(item) = self.items.get(self.chosen) else { return };
        let (kind, index) = (item.kind, item.index);
        let Some(amx) = self.check() else { return };
        let bytes = amx.to_bytes();
        match sd.write(&path(kind, index), &sango_core::lz::compress_lz11(&bytes)) {
            Ok(_) => {
                self.baseline = self.text.clone();
                self.status = Some((true, format!("Saved ({} bytes). Restart the game to run the script.", bytes.len())));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn reset(&mut self, sd: &SdCard) {
        let Some(item) = self.items.get(self.chosen) else { return };
        match sd.remove(&path(item.kind, item.index)) {
            Ok(_) => {
                self.load(Some(sd));
                self.status = Some((true, "The script of the game is back. Restart the game to run it.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    /// Writes the assembly of the open script in a text file that the user chooses.
    fn export_text(&mut self) {
        let name = self.items.get(self.chosen).map_or("script".to_string(), |i| format!("script_{}_{}", KINDS[i.kind].1, i.index));
        let Some(path) = rfd::FileDialog::new().set_title("Export the assembly").set_file_name(format!("{name}.txt")).add_filter("Text", &["txt"]).save_file() else { return };
        self.status = Some(match std::fs::write(&path, &self.text) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(e) => (false, format!("{}: {e}", path.display())),
        });
    }

    /// Reads a text file of assembly in place of the text of the open script. Save builds it.
    fn import_text(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Import an assembly").add_filter("Text", &["txt"]).pick_file() else { return };
        self.status = Some(match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.text = text;
                (true, "Imported. Check it, then save.".to_string())
            }
            Err(e) => (false, format!("{}: {e}", path.display())),
        });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let dirty = self.dirty();
        let from_card = self.items.get(self.chosen).is_some_and(|i| session.sd.as_ref().is_some_and(|sd| sd.read(&path(i.kind, i.index)).is_some()));
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let shown = self.items.get(self.chosen).map_or("(none)".to_string(), |i| i.label.clone());
                let mut value = self.chosen as u16;
                let changed = picker(ui, "script_file", &shown, 420.0, &mut value, None, || {
                    let mut groups: Vec<Group> = Vec::new();
                    for (place, item) in self.items.iter().enumerate() {
                        match groups.iter_mut().find(|g| g.title == item.group) {
                            Some(group) => group.entries.push((place as u16, item.label.clone())),
                            None => groups.push(Group { title: item.group.to_string(), entries: vec![(place as u16, item.label.clone())] }),
                        }
                    }
                    groups
                });
                if changed && (value as usize) < self.items.len() {
                    self.chosen = value as usize;
                    self.load(session.sd.as_ref());
                }
                let has_card = session.sd.is_some();
                let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(true), import: Some(true), reset: Some(has_card && from_card), save: Some(has_card && dirty), ..Default::default() });
                self.history.buttons(ui, !self.text.is_empty());
                if widget::button(ui, "Check") {
                    self.check();
                }
                if clicks.export {
                    self.export_text();
                }
                if clicks.import {
                    self.import_text();
                }
                self.reset_open |= clicks.reset;
                if let (true, Some(sd)) = (clicks.save, session.sd.as_ref()) {
                    self.save(sd);
                }
                if dirty {
                    widget::badge(ui, "Not saved", color::AMBER);
                } else if from_card {
                    widget::badge(ui, "From the SD card", color::TEAL);
                } else {
                    widget::badge(ui, "From the romfs", color::GREEN);
                }
            });
            if let Some(item) = self.items.get(self.chosen) {
                if item.kind == 0 {
                    if let Some(file) = files::FILES.iter().find(|f| f.index == item.index) {
                        ui.label(widget::hint(&format!("Holds the scripts {} to {}. {}", file.first, file.last, file.help)));
                    }
                }
            }
            if session.sd.is_none() {
                ui.label(egui::RichText::new("The SD card is not chosen: you can read the scripts, but nothing can be saved.").color(color::AMBER));
            }
            if let Some((good, message)) = &self.status {
                ui.label(egui::RichText::new(message).color(if *good { color::GREEN } else { color::ROSE }));
            }
        });
        let what = self.items.get(self.chosen).map_or(String::new(), |i| format!("The script file {}", i.label));
        if widget::reset_window(ui.ctx(), &mut self.reset_open, &what, &mut []) {
            if let Some(sd) = session.sd.as_ref() {
                self.reset(sd);
            }
        }
    }

    /// Makes the Pawn-like text and the addresses of the lines again when the assembly changed.
    fn refresh_pawn(&mut self) {
        if self.pawn_from == self.text {
            return;
        }
        self.pawn_from = self.text.clone();
        let names = &self.names;
        // The assembly of the text, as it is now (the text may have a mistake).
        match assembly::from_text_with_lines(&self.text).and_then(|(amx, lines)| decompile::to_pawn_lines(&amx, &|h| names.get(&h).cloned()).map(|pawn| (pawn, lines))) {
            Ok((pawn, lines)) => {
                self.pawn = pawn;
                self.assembly_lines = lines;
                self.pawn_error = None;
            }
            Err(message) => {
                self.pawn.clear();
                self.assembly_lines.clear();
                self.pawn_error = Some(message);
            }
        }
    }

    /// The addresses where a line of the Pawn-like text starts, in order.
    fn pawn_starts(&self) -> Vec<usize> {
        let mut starts: Vec<usize> = self.pawn.iter().filter_map(|(_, at)| *at).collect();
        starts.sort_unstable();
        starts.dedup();
        starts
    }

    /// The part of the code that the choice covers: from the start of its line of the Pawn-like text to the start of the next one.
    fn selected_range(&self) -> Option<(usize, usize)> {
        let at = self.selected?;
        let starts = self.pawn_starts();
        let first = starts.iter().rev().find(|&&s| s <= at).copied().unwrap_or(at);
        let next = starts.iter().find(|&&s| s > first).copied().unwrap_or(usize::MAX);
        Some((first, next))
    }

    /// The Code tab: the assembly on the left (it can be changed) and the Pawn-like text on the right (to read). A click on a line of one side
    /// chooses its instruction: the other side shows it, and both mark it.
    fn code_tab(&mut self, ui: &mut egui::Ui) {
        self.refresh_pawn();
        let range = self.selected_range();
        let mark = ui.visuals().selection.bg_fill.gamma_multiply(0.45);
        ui.columns(2, |columns| {
            // The assembly.
            let ui = &mut columns[0];
            ui.label(widget::hint("Assembly: one instruction on each line. \"name:\" is an address that the jumps use; \".native Name\" adds a native, \"sysreq.c Name\" calls it."));
            egui::ScrollArea::both().id_salt("script_text").auto_shrink([false, false]).show(ui, |ui| {
                let dark = ui.visuals().dark_mode;
                let plain = ui.visuals().text_color();
                let font = egui::TextStyle::Monospace.resolve(ui.style());
                // No wrap: each line of the text is one row of the picture, so the rows of the choice are found by their number.
                let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                    let mut job = highlight::job(text, Mode::Assembly, dark, plain, &font);
                    job.wrap.max_width = f32::INFINITY;
                    ui.fonts(|f| f.layout_job(job))
                };
                let editor_id = egui::Id::new("script_assembly_editor");
                let output = egui::TextEdit::multiline(&mut self.text).id(editor_id).font(egui::TextStyle::Monospace).code_editor().layouter(&mut layouter).desired_width(f32::INFINITY).desired_rows(40).lock_focus(true).show(ui);
                // The line of the cursor: a new line chooses its instruction.
                if let Some(cursor) = output.cursor_range {
                    let line = cursor.primary.pcursor.paragraph;
                    if self.cursor_line != Some(line) && output.response.has_focus() {
                        self.cursor_line = Some(line);
                        if let Some(at) = self.assembly_lines.get(..=line.min(self.assembly_lines.len().saturating_sub(1))).and_then(|lines| lines.iter().rev().find_map(|a| *a)) {
                            self.selected = Some(at);
                            self.scroll_pawn = true;
                        }
                    }
                }
                // The lines of the choice: a colour behind them.
                if let Some((first, next)) = range {
                    let mut target = None;
                    for (row, at) in self.assembly_lines.iter().enumerate() {
                        let Some(at) = at else { continue };
                        if *at < first || *at >= next {
                            continue;
                        }
                        if let Some(galley_row) = output.galley.rows.get(row) {
                            let rect = galley_row.rect.translate(output.galley_pos.to_vec2());
                            let rect = egui::Rect::from_min_max(egui::pos2(output.response.rect.left(), rect.top()), egui::pos2(output.response.rect.right(), rect.bottom()));
                            ui.painter().rect_filled(rect, 2.0, mark);
                            target.get_or_insert(rect);
                        }
                    }
                    if self.scroll_assembly {
                        if let Some(rect) = target {
                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        }
                        self.scroll_assembly = false;
                    }
                }
            });

            // The Pawn-like text.
            let ui = &mut columns[1];
            if let Some(message) = &self.pawn_error {
                ui.label(egui::RichText::new(format!("The assembly has a mistake, so the Pawn text cannot be made: {message}")).color(color::ROSE));
                return;
            }
            ui.label(widget::hint("Pawn (read only): the code as text, to read. A click on a line shows its instructions on the left."));
            let height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
            let dark = ui.visuals().dark_mode;
            let plain = ui.visuals().text_color();
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let mut area = egui::ScrollArea::both().id_salt("script_pawn").auto_shrink([false, false]);
            if self.scroll_pawn {
                self.scroll_pawn = false;
                if let Some((first, _)) = range {
                    if let Some(row) = self.pawn.iter().position(|(_, at)| *at == Some(first)) {
                        area = area.vertical_scroll_offset((row as f32 * height - 120.0).max(0.0));
                    }
                }
            }
            let mut clicked = None;
            area.show_rows(ui, height, self.pawn.len(), |ui, rows| {
                for row in rows {
                    let (line, at) = &self.pawn[row];
                    let chosen = range.is_some_and(|(first, _)| *at == Some(first));
                    let response = ui.add(egui::Label::new(highlight::job(line, Mode::Pawn, dark, plain, &font)).sense(egui::Sense::click()));
                    if chosen {
                        let rect = egui::Rect::from_min_max(egui::pos2(ui.min_rect().left(), response.rect.top()), egui::pos2(ui.max_rect().right(), response.rect.bottom()));
                        ui.painter().rect_filled(rect, 2.0, mark);
                    }
                    if response.clicked() {
                        // A line without instruction (a brace) chooses the line above it that has one.
                        clicked = self.pawn[..=row].iter().rev().find_map(|(_, a)| *a);
                    }
                }
            });
            if let Some(at) = clicked {
                self.selected = Some(at);
                self.scroll_assembly = true;
                // The cursor of the assembly goes to the first line of the instruction.
                if let Some(line) = self.assembly_lines.iter().position(|a| *a == Some(at)) {
                    let place: usize = self.text.lines().take(line).map(|l| l.chars().count() + 1).sum();
                    let id = egui::Id::new("script_assembly_editor");
                    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(place))));
                    state.store(ui.ctx(), id);
                    self.cursor_line = Some(line);
                }
            }
        });
    }

    fn scripts_tab(&mut self, ui: &mut egui::Ui) {
        let Some(amx) = self.built.as_ref() else { return };
        widget::card(ui, |ui| {
            ui.label(widget::title("Scripts of the file"));
            ui.label(widget::hint("The public functions: the entries that the game starts. The game gives each one a number made from its name."));
            ui.add_space(4.0);
            if amx.publics.is_empty() {
                ui.label(widget::hint("This file has no public function."));
            }
            for entry in &amx.publics {
                ui.label(egui::RichText::new(format!("0x{:08X}   at the address {:04X}", entry.name, entry.address)).monospace());
            }
        });
    }

    fn used_tab(&mut self, ui: &mut egui::Ui) {
        let Some(amx) = self.built.as_ref() else { return };
        widget::card(ui, |ui| {
            ui.label(widget::title("Natives used by the file"));
            ui.label(widget::hint("The native functions of the table of the file. A function that the game does not know by this name shows its number."));
            ui.add_space(4.0);
            for entry in &amx.natives {
                match self.names.get(&entry.name).and_then(|name| natives::find(name)) {
                    Some(native) => native_card(ui, native, None),
                    None => {
                        ui.label(egui::RichText::new(format!("0x{:08X}", entry.name)).monospace());
                    }
                }
            }
        });
    }

    fn api_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(widget::hint("🔍"));
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search a function or what it does…").desired_width(320.0));
        });
        let needle = sango_ui::picker::simple(&self.search);
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("script_api").auto_shrink([false, false]).show(ui, |ui| {
            let mut groups: Vec<&str> = Vec::new();
            for native in natives::NATIVES {
                if !groups.contains(&native.group) {
                    groups.push(native.group);
                }
            }
            for group in groups {
                let shown: Vec<&natives::Native> = natives::NATIVES.iter().filter(|n| n.group == group && (needle.is_empty() || sango_ui::picker::simple(&format!("{} {}", n.alias.replace('_', " "), n.name)).contains(&needle) || sango_ui::picker::simple(n.help).contains(&needle))).collect();
                if shown.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(format!("{} ({})", group_name(group), shown.len())).id_salt(("script_group", group)).default_open(!needle.is_empty()).show(ui, |ui| {
                    for native in shown {
                        native_card(ui, native, None);
                    }
                });
            }
        });
    }
}

impl Module for ScriptPage {
    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        if let Some(sd) = session.sd.as_ref() {
            if self.dirty() {
                self.save(sd);
            }
            self.creator.save_changes(sd);
        }
    }

    fn unsaved(&self) -> bool {
        self.dirty() || self.creator.unsaved()
    }

    fn title(&self) -> &str {
        "Script"
    }

    fn icon(&self) -> &str {
        "📜"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Script");
        ui.add_space(8.0);
        let Some(dump) = session.dump.as_ref() else {
            widget::missing_card(ui, session.missing().unwrap_or("Choose the dump of the game on the Home page."));
            return;
        };
        let root = dump.root().to_path_buf();
        if self.root.as_deref() != Some(root.as_path()) {
            self.root = Some(root);
            self.open(dump);
        }
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
            return;
        }
        if self.text.is_empty() && self.status.is_none() {
            self.load(session.sd.as_ref());
        }
        if self.tab != CREATE_TAB {
            self.tool_bar(ui, session);
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            for (place, name) in TABS.iter().enumerate() {
                if widget::tab_button(ui, name, self.tab == place, false).clicked() {
                    self.tab = place;
                }
            }
        });
        ui.add_space(6.0);
        if self.tab == CREATE_TAB {
            self.creator.ui(ui, session.sd.as_ref());
            return;
        }
        match self.tab {
            0 => self.code_tab(ui),
            1 => {
                egui::ScrollArea::vertical().id_salt("script_publics").auto_shrink([false, false]).show(ui, |ui| self.scripts_tab(ui));
            }
            2 => {
                egui::ScrollArea::vertical().id_salt("script_used").auto_shrink([false, false]).show(ui, |ui| self.used_tab(ui));
            }
            _ => self.api_tab(ui),
        }
        // Ctrl+Z, Ctrl+Y and the buttons on the text (the editor keeps the keys while it has them).
        self.history.track(ui, self.chosen as u64, &mut self.text);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: a script file opens, a line is added to its text, the file is built and saved, read back, and reset.
    #[test]
    fn a_script_is_changed_built_saved_and_reset() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_script_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open_with(&folder.to_string_lossy(), Some(folder.join("work"))).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sd), focus_page: true, ..Session::default() };
        let mut page = ScriptPage::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
        }
        assert!(page.text.contains(".code"), "a script is open");
        assert!(!page.dirty());
        // A new instruction at the start of the first function: the file is longer.
        let original = page.built.as_ref().unwrap().to_bytes().len();
        let at = page.text.find("    proc").expect("a function starts with proc");
        page.text.insert_str(at, "    nop\n");
        assert!(page.dirty());
        // Ctrl+Z and Ctrl+Y on the change.
        for (input, undone) in [(Default::default(), false), (sango_ui::history::test_keys(egui::Key::Z), true), (sango_ui::history::test_keys(egui::Key::Y), false)] {
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, &mut session));
            });
            assert_eq!(!page.dirty(), undone);
        }
        // The same on the scripts made of blocks.
        page.tab = CREATE_TAB;
        let frame = |page: &mut ScriptPage, session: &mut Session, input: egui::RawInput| {
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        frame(&mut page, &mut session, Default::default());
        assert!(!page.creator.unsaved());
        page.creator.rename(0, "A test");
        frame(&mut page, &mut session, Default::default());
        assert!(page.creator.unsaved());
        frame(&mut page, &mut session, sango_ui::history::test_keys(egui::Key::Z));
        assert!(!page.creator.unsaved());
        page.tab = 0;
        page.save(session.sd.as_ref().unwrap());
        assert!(!page.dirty(), "{:?}", page.status);
        let index = page.items[page.chosen].index;
        let saved = session.sd.as_ref().unwrap().read(&path(0, index)).expect("the file is on the SD card");
        assert!(sango_core::lz::decompress(&saved).unwrap().len() > original);
        assert!(session.sd.as_ref().unwrap().read("script/field/index.bin").is_some(), "the list of the files is written");
        page.reset(session.sd.as_ref().unwrap());
        assert!(!page.dirty());
        assert_eq!(page.built.as_ref().unwrap().to_bytes().len(), original);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
