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
const TABS: [&str; 6] = ["Assembly", "Pawn (read only)", "Scripts", "Natives used", "Native functions", "Create a script"];

/// The place of the tab that makes scripts from blocks.
const CREATE_TAB: usize = 5;

/// A script file that the user can open.
struct Item {
    kind: usize,
    index: usize,
    label: String,
    group: &'static str,
}

/// The page.
pub struct ScriptPage {
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
    /// The text in the Pawn tab, and the text of the assembly that it was made from.
    pawn: Vec<String>,
    pawn_from: String,
    pawn_error: Option<String>,
    /// The tab "Create a script".
    creator: crate::create::Creator,
}

impl Default for ScriptPage {
    fn default() -> Self {
        ScriptPage { root: None, error: None, garcs: Vec::new(), items: Vec::new(), chosen: 0, tab: 0, text: String::new(), baseline: String::new(), built: None, status: None, search: String::new(), names: HashMap::new(), pawn: Vec::new(), pawn_from: String::new(), pawn_error: None, creator: Default::default() }
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
            Some(Ok(amx)) => match assembly::to_text(&amx, &|h| names.get(&h).cloned()) {
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

    fn tool_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let dirty = self.dirty();
        let from_card = self.items.get(self.chosen).is_some_and(|i| session.sd.as_ref().is_some_and(|sd| sd.read(&path(i.kind, i.index)).is_some()));
        widget::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("Script file"));
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
                if widget::button(ui, "Check the text") {
                    self.check();
                }
                if let Some(sd) = session.sd.as_ref() {
                    ui.add_enabled_ui(dirty, |ui| {
                        if widget::button_styled(ui, egui::RichText::new("Build and save").color(color::WHITE), Some(color::ACCENT)) {
                            self.save(sd);
                        }
                    })
                    .response
                    .on_disabled_hover_text("Nothing to save: the text is the one that was loaded or saved.");
                    ui.add_enabled_ui(dirty, |ui| {
                        if widget::confirm_button(ui, "Discard the changes", "Click again to discard") {
                            self.load(Some(sd));
                        }
                    });
                    if from_card && widget::confirm_button(ui, "Reset to the game", "Click again to reset") {
                        self.reset(sd);
                    }
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
    }

    fn assembly_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(widget::hint("One instruction on each line. A name followed by \":\" is an address that the jumps use. \".native Name\" adds a native function to the table of the file; \"sysreq.c Name\" calls it."));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("script_text").auto_shrink([false, false]).show(ui, |ui| {
            let dark = ui.visuals().dark_mode;
            let plain = ui.visuals().text_color();
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let mut job = highlight::job(text, Mode::Assembly, dark, plain, &font);
                job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(job))
            };
            ui.add(egui::TextEdit::multiline(&mut self.text).font(egui::TextStyle::Monospace).code_editor().layouter(&mut layouter).desired_width(f32::INFINITY).desired_rows(40).lock_focus(true));
        });
    }

    /// The Pawn tab: the code as text to read, made again when the assembly changed.
    fn pawn_tab(&mut self, ui: &mut egui::Ui) {
        if self.pawn_from != self.text {
            self.pawn_from = self.text.clone();
            let names = &self.names;
            // The assembly of the text, as it is now (the text may have a mistake).
            match assembly::from_text(&self.text).and_then(|amx| decompile::to_pawn(&amx, &|h| names.get(&h).cloned())) {
                Ok(text) => {
                    self.pawn = text.lines().map(str::to_string).collect();
                    self.pawn_error = None;
                }
                Err(message) => {
                    self.pawn.clear();
                    self.pawn_error = Some(message);
                }
            }
        }
        if let Some(message) = &self.pawn_error {
            ui.label(egui::RichText::new(format!("The assembly has a mistake, so the Pawn text cannot be made: {message}")).color(color::ROSE));
            return;
        }
        ui.label(widget::hint("The code as text that looks like Pawn, to help to read the assembly. It cannot be changed here, and it is a guess: the names of the variables are made up."));
        ui.add_space(4.0);
        let height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let dark = ui.visuals().dark_mode;
        let plain = ui.visuals().text_color();
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        egui::ScrollArea::both().id_salt("script_pawn").auto_shrink([false, false]).show_rows(ui, height, self.pawn.len(), |ui, range| {
            for line in &self.pawn[range] {
                ui.label(highlight::job(line, Mode::Pawn, dark, plain, &font));
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
    fn unsaved(&self) -> bool {
        self.dirty() || self.creator.unsaved()
    }

    fn title(&self) -> &str {
        "Script"
    }

    fn icon(&self) -> &str {
        "Sc"
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
            0 => self.assembly_tab(ui),
            1 => self.pawn_tab(ui),
            2 => {
                egui::ScrollArea::vertical().id_salt("script_publics").auto_shrink([false, false]).show(ui, |ui| self.scripts_tab(ui));
            }
            3 => {
                egui::ScrollArea::vertical().id_salt("script_used").auto_shrink([false, false]).show(ui, |ui| self.used_tab(ui));
            }
            _ => self.api_tab(ui),
        }
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
