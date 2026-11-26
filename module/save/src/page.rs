//! The page: the file that is open, the top bar, and the changes that go to the disk and to the running game.

use crate::context::GameData;
use crate::editor::Lists;
use crate::format::{self, Save};
use crate::pokemon::Pokemon;
use crate::{data, live};
use sango_species::SpeciesIcons;
use sango_ui::history::History;
use sango_ui::item_menu::ItemList;
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

pub const TABS: [&str; 5] = ["Trainer", "Party", "Boxes", "Bag", "Map"];
/// The blocks that each tab changes, to mark the tabs that have changes.
const TAB_BLOCKS: [&[usize]; 5] = [
    &[format::TRAINER, format::MISC, format::PLAY_TIME, format::OPTIONS],
    &[format::PARTY],
    &[format::BOXES, format::BOX_NAMES],
    &[format::ITEMS],
    &[format::POSITION],
];

/// The changed blocks of the save: what the undo and the redo keep.
type Overrides = Vec<(usize, Vec<u8>)>;

/// What the page read: the file of the save, the dump and the SD card.
type Source = (PathBuf, PathBuf, Option<PathBuf>);

#[derive(Default)]
pub struct SavePage {
    source: Option<Source>,
    /// The save as the file has it, and with the changes of the page.
    pub(crate) loaded: Option<Save>,
    pub(crate) edited: Option<Save>,
    /// The date of the file when it was read.
    modified: Option<SystemTime>,
    checked: Option<Instant>,
    /// True when the game saved while the page has changes.
    outdated: bool,
    pub(crate) data: Option<GameData>,
    pub(crate) lists: Lists,
    pub(crate) icons: SpeciesIcons,
    pub(crate) items: Arc<Mutex<ItemList>>,
    error: Option<String>,
    pub(crate) status: Option<(bool, String)>,
    pub(crate) tab: usize,
    pub(crate) party_slot: usize,
    pub(crate) box_number: usize,
    pub(crate) box_slot: usize,
    pub(crate) pocket: usize,
    /// The Pokémon that the button Copy keeps.
    pub(crate) copied: Option<Pokemon>,
    copies_open: bool,
    history: History<Overrides>,
    /// Grows at each read of the file: the history starts again.
    reads: u64,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn overrides(loaded: &Save, edited: &Save) -> Overrides {
    edited.changed_blocks(loaded).into_iter().map(|block| (block, edited.block(block).to_vec())).collect()
}

fn with_overrides(loaded: &Save, changes: &Overrides) -> Save {
    let mut save = loaded.clone();
    for (block, bytes) in changes {
        save.block_mut(*block).copy_from_slice(bytes);
    }
    save
}

/// The name of a list of blocks for a message.
fn names(blocks: &[usize]) -> String {
    blocks.iter().map(|&block| format::block_name(block)).collect::<Vec<_>>().join(", ")
}

/// The day and the hour of a copy of the file, from its name (UTC).
fn copy_name(path: &Path) -> String {
    let millis: u64 = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.strip_prefix("main-")).and_then(|s| s.parse().ok()).unwrap_or(0);
    let seconds = millis / 1000;
    let (hours, minutes, rest) = (seconds / 3600 % 24, seconds / 60 % 60, seconds % 60);
    let ago = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |now| now.as_secs().saturating_sub(seconds));
    let ago = match ago {
        0..=119 => format!("{ago} s ago"),
        120..=7199 => format!("{} min ago", ago / 60),
        7200..=172_799 => format!("{} h ago", ago / 3600),
        _ => format!("{} days ago", ago / 86_400),
    };
    format!("{ago}  ({hours:02}:{minutes:02}:{rest:02} UTC)")
}

impl SavePage {
    /// Reads the file and what the menus need, when something changed since the last frame.
    fn open(&mut self, session: &Session) {
        let (Some(file), Some(dump)) = (&session.save_file, &session.dump) else {
            self.source = None;
            self.loaded = None;
            self.edited = None;
            return;
        };
        let source: Source = (file.clone(), dump.root().to_path_buf(), session.sd.as_ref().map(|sd| sd.root().to_path_buf()));
        if self.source.as_ref() == Some(&source) {
            return;
        }
        self.source = Some(source);
        self.status = None;
        self.icons.open(dump);
        if let Ok(mut items) = self.items.lock() {
            items.open(dump);
            items.refresh_custom(session.sd.as_ref());
        }
        self.lists.abilities = sango_core::custom_ability::list(&sango_species::ABILITY_NAMES, session.sd.as_ref());
        match GameData::open(dump, session.sd.as_ref()) {
            Ok(data) => self.data = Some(data),
            Err(message) => {
                self.data = None;
                self.error = Some(message);
                return;
            }
        }
        self.read(file.clone().as_path());
    }

    /// Reads the file again. The changes of the page are lost.
    pub(crate) fn read(&mut self, file: &Path) {
        self.modified = modified(file);
        self.outdated = false;
        self.reads += 1;
        match Save::read(file) {
            Ok(save) => {
                self.edited = Some(save.clone());
                self.loaded = Some(save);
                self.error = None;
            }
            Err(message) => {
                self.edited = None;
                self.loaded = None;
                self.error = Some(message);
            }
        }
    }

    /// Looks one time each second if the game saved.
    fn watch(&mut self, ctx: &egui::Context, session: &Session) {
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.checked.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.checked = Some(Instant::now());
        let Some(file) = session.save_file.clone() else { return };
        let now = modified(&file);
        if now.is_none() || now == self.modified {
            return;
        }
        if self.unsaved() {
            self.outdated = true;
        } else {
            self.read(&file);
            self.status = Some((true, "The file changed: the page shows it.".to_string()));
        }
    }

    /// Writes the changes in the file and sends them to the running game.
    fn save(&mut self, session: &Session) {
        let (Some(file), Some(loaded), Some(edited)) = (&session.save_file, &self.loaded, &self.edited) else { return };
        let (now, changed) = match format::write_changes(file, loaded, edited, &sango_core::setting::save_copies_folder()) {
            Ok(done) => done,
            Err(message) => {
                self.status = Some((false, message));
                return;
            }
        };
        let place = changed.contains(&format::POSITION).then(|| data::place(&now));
        let mut message = format!("Written in the file: {}.", names(&changed));
        let mut good = true;
        match &session.sd {
            Some(sd) if !changed.is_empty() => match live::send(sd, &now, &changed, place) {
                Ok(()) => message += " A running game takes the change when your player walks freely.",
                Err(error) => {
                    good = false;
                    message += &format!(" The running game does not get it: {error}");
                }
            },
            Some(_) => {}
            None => message += " To send the change to a running game, choose the SD card folder in Home.",
        }
        self.modified = modified(file);
        self.outdated = false;
        self.reads += 1;
        self.edited = Some(now.clone());
        self.loaded = Some(now);
        self.status = Some((good, message));
    }

    /// Puts the content of another file in the page. It is written at the next Save.
    fn take(&mut self, file: &Path) {
        match Save::read(file) {
            Ok(save) => {
                self.edited = Some(save);
                self.status = Some((true, "The page shows this file. Press Save to write it in the save of the game.".to_string()));
            }
            Err(message) => self.status = Some((false, message)),
        }
    }

    fn export(&mut self) {
        let Some(edited) = &self.edited else { return };
        let Some(path) = rfd::FileDialog::new().set_title("Export the save").set_file_name("main").save_file() else { return };
        self.status = Some(match std::fs::write(&path, edited.to_bytes()) {
            Ok(()) => (true, format!("Exported to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    fn import(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Import a save (a file main)").pick_file() {
            self.take(&path);
        }
    }

    fn copies_window(&mut self, ctx: &egui::Context) {
        if !self.copies_open {
            return;
        }
        let (mut open, mut close, mut chosen) = (true, false, None);
        egui::Window::new("Put back a copy of the save").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(widget::hint("The file as it was before a Save. Choose one, then Save."));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().id_salt("save_copies").max_height(320.0).show(ui, |ui| {
                for path in format::copies(&sango_core::setting::save_copies_folder()).iter().rev() {
                    if widget::button(ui, copy_name(path)) {
                        chosen = Some(path.clone());
                    }
                }
            });
            ui.add_space(4.0);
            close = widget::button(ui, "Cancel");
        });
        if let Some(path) = chosen {
            self.take(&path);
            close = true;
        }
        if close || !open {
            self.copies_open = false;
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, session: &Session) {
        let ready = self.edited.is_some() && self.data.is_some();
        let changed = self.unsaved();
        let (can_new, can_delete) = if ready { (self.can_add(), self.can_delete()) } else { (false, false) };
        let has_copies = !format::copies(&sango_core::setting::save_copies_folder()).is_empty();
        let mut clicks = Default::default();
        let mut again = false;
        ui.horizontal_wrapped(|ui| {
            clicks = widget::action_bar(
                ui,
                widget::ActionBar {
                    new: Some(("+ New Pokémon", can_new)),
                    delete: Some(can_delete),
                    export: Some(ready),
                    import: Some(ready),
                    reset: Some(ready && has_copies),
                    save: Some(changed),
                    notes: widget::ActionNotes {
                        new: "A Pokémon of the player in the chosen place (tabs Party and Boxes)",
                        delete: "Removes the chosen Pokémon",
                        export: "Writes a copy of the save in a file that you choose",
                        import: "Shows another save: Save then writes it",
                        reset: "Puts back a copy of the save from before a Save",
                    },
                },
            );
            self.history.buttons(ui, ready);
            again = ui.add_enabled_ui(session.save_file.is_some(), |ui| widget::button_response(ui, "Read again", None)).inner.0.on_hover_text("Reads the file of the save again: the changes of the page are lost.").clicked();
            if changed {
                widget::badge(ui, "Not saved", color::AMBER);
            }
            if self.outdated {
                widget::badge(ui, "The game saved: press Read again", color::ROSE);
            }
            if let Some(sd) = session.sd.as_ref().filter(|sd| live::is_waiting(sd)) {
                widget::badge(ui, "Waiting for the game", color::TEAL);
                let button = widget::button_response(ui, "Do not send", None).0;
                if button.on_hover_text("The game takes the changes when your player walks freely in the world. This keeps them in the file only.").clicked() {
                    live::cancel(sd);
                }
            }
        });
        if clicks.new {
            self.add();
        }
        if clicks.delete {
            self.delete();
        }
        if clicks.export {
            self.export();
        }
        if clicks.import {
            self.import();
        }
        if clicks.reset {
            self.copies_open = true;
        }
        if clicks.save {
            self.save(session);
        }
        if again {
            if let Some(file) = session.save_file.clone() {
                self.read(&file);
                self.status = None;
            }
        }
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let (Some(loaded), Some(mut work)) = (self.loaded.take(), self.edited.take()) else { return };
        let changed = work.changed_blocks(&loaded);
        let mut tab = self.tab;
        widget::marked_tabs(ui, "save", &TABS, |at| TAB_BLOCKS[at].iter().any(|block| changed.contains(block)), &mut tab);
        self.tab = tab;
        ui.add_space(6.0);
        egui::ScrollArea::both().id_salt(("save_scroll", tab)).auto_shrink([false, false]).show(ui, |ui| match tab {
            0 => self.trainer_tab(ui, &loaded, &mut work),
            1 => self.party_tab(ui, &loaded, &mut work),
            2 => self.boxes_tab(ui, &loaded, &mut work),
            3 => self.bag_tab(ui, &mut work),
            _ => self.map_tab(ui, &loaded, &mut work),
        });
        let mut changes = overrides(&loaded, &work);
        if self.history.track(ui, self.reads, &mut changes) {
            work = with_overrides(&loaded, &changes);
        }
        self.loaded = Some(loaded);
        self.edited = Some(work);
    }
}

impl Module for SavePage {
    fn title(&self) -> &str {
        "Save"
    }

    fn icon(&self) -> &str {
        "💾"
    }

    fn unsaved(&self) -> bool {
        self.loaded.is_some() && self.loaded != self.edited
    }

    fn save_changes(&mut self, _ctx: &egui::Context, session: &mut Session) {
        self.save(session);
    }

    fn discard_changes(&mut self) -> bool {
        self.edited = self.loaded.clone();
        self.history.clear();
        true
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        if session.dump.is_none() {
            widget::missing_card(ui, "Choose the romfs dump in Home first.");
            return;
        }
        if session.save_file.is_none() {
            widget::missing_card(ui, "Choose the save file of the game (main) in Home first.");
            return;
        }
        self.open(session);
        self.watch(ui.ctx(), session);
        self.top_bar(ui, session);
        if let Some((ok, message)) = &self.status {
            ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }));
        }
        if let Some(error) = &self.error {
            ui.label(egui::RichText::new(error).color(color::ROSE));
        }
        ui.add_space(4.0);
        if self.data.is_some() {
            self.content(ui);
        }
        self.copies_window(ui.ctx());
    }
}
