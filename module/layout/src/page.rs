//! The Layout page: the list of the layouts, the screen, the tree of the panes and their properties.

use crate::bclan::{self, Bclan};
use crate::bclyt::{Bclyt, Kind, Node};
use crate::bundle::Bundle;
use crate::tex;
use sango_core::{Clim, Garc, Session};
use sango_ui::theme::color;
use sango_ui::{egui, focus, widget, Module};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// The folder of the layouts on the SD card.
const FOLDER: &str = "layout";

/// The biggest archive that is searched for layouts (the archives of the models and of the maps have none).
const BIGGEST_ARCHIVE: u64 = 60_000_000;

/// The number of changes that can be undone.
const HISTORY: usize = 60;

/// A layout of an archive.
#[derive(Clone, Debug)]
struct Entry {
    archive: usize,
    file: usize,
    language: usize,
    /// The number of languages of the file (names of the SD file depend on it).
    languages: usize,
    layout: String,
}

impl Entry {
    /// The file on the SD card, which the plugin reads in place of the file of the archive. A file that has a version for each language
    /// has none: the plugin replaces a file by its number only.
    fn sd_path(&self) -> Option<String> {
        (self.languages <= 1).then(|| format!("{FOLDER}/{}/{}.bin", self.archive, self.file))
    }

    fn path(&self) -> String {
        format!("a/{}/{}/{}", self.archive / 100, self.archive / 10 % 10, self.archive % 10)
    }
}

/// The menu that an archive belongs to (by the number of the archive).
fn group(archive: usize) -> &'static str {
    match archive {
        0 | 30 | 148 => "Summary",
        2 => "Party",
        61 | 147 => "Bag",
        103 | 142 => "Box",
        25 | 43..=57 | 65 | 161 => "PSS",
        24 | 110 | 67 | 68 | 243 | 246..=249 => "PokéNav and field",
        4 | 5 | 33 | 226 | 233 => "Battle",
        69 | 15 | 70 | 201 | 202 | 19 => "Title, options, save, input",
        109 | 200 | 188 => "Trainer card, Pokédex, HOME",
        107 | 90 | 149 => "Shops and boutique",
        94 | 153 | 159 | 134 | 11 | 12 => "Messages and dialogs",
        16..=18 | 26 | 41 | 42 | 144 | 27 | 28 | 114 | 113 => "Amie and Super Training",
        138 | 155 | 157 | 206 | 112 | 168 | 179 | 204 | 173 | 205 | 140 | 141 => "Online and gifts",
        152 | 170 | 211..=213 | 223 | 237 | 241 | 202..=203 => "Cutscenes and credits",
        _ => "Other",
    }
}

/// A snapshot for the undo.
#[derive(Clone)]
struct Snapshot {
    bundle: Bundle,
    layout: Bclyt,
}

/// A layout that is open.
struct Open {
    entry: Entry,
    bundle: Bundle,
    layout: Bclyt,
    /// The block of the selected pane.
    selected: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The images, by name, in the version of the bundle that is shown.
    textures: HashMap<String, Option<egui::TextureHandle>>,
    dirty: bool,
    /// True when the layout comes from the SD card.
    replaced: bool,
    /// True when the selected pane must come in sight in the tree (after a key or a click on the screen).
    scroll_tree: bool,
    /// True while one edit goes on (a drag of a value): the history already has its first state.
    edit_open: bool,
    /// The animation file that the Animations tab shows.
    animation: Option<String>,
    /// The textures that were removed during this frame. The screen still draws them at the end of the frame: they are freed at the
    /// start of the next one (a texture that is freed too soon makes the graphics card refuse the frame).
    retired: Vec<egui::TextureHandle>,
}

/// What the mouse does on the screen.
#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Move,
    /// The handle: -1, 0 or 1 for the left/top, none, right/bottom sides.
    Handle(i8, i8),
}

/// The Layout page.
pub struct LayoutPage {
    /// The window of the button Reset….
    reset_open: bool,
    scanned_for: Option<PathBuf>,
    scan: Option<Receiver<Vec<Entry>>>,
    entries: Vec<Entry>,
    filter: String,
    open: Option<Open>,
    zoom: f32,
    show_hidden: bool,
    show_text: bool,
    drag: Option<Drag>,
    status: Option<(bool, String)>,
    /// The pane that was asked to be deleted, until the user confirms.
    confirm_delete: bool,
    /// True when the selected layout must come in sight in the list (after a key).
    scroll_list: bool,
    /// The tab of the right panel.
    animations_tab: bool,
}

impl Default for LayoutPage {
    fn default() -> Self {
        LayoutPage {
            reset_open: false,
            scanned_for: None,
            scan: None,
            entries: Vec::new(),
            filter: String::new(),
            open: None,
            zoom: 1.6,
            show_hidden: false,
            show_text: true,
            drag: None,
            status: None,
            confirm_delete: false,
            scroll_list: false,
            animations_tab: false,
        }
    }
}

/// A 2D transform: `x' = a x + c y + tx`, `y' = b x + d y + ty`. The layout space has y going up.
#[derive(Clone, Copy)]
struct Affine {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

impl Affine {
    const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    fn then(self, o: Affine) -> Affine {
        // self ∘ o: apply `o` first, then `self`.
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.tx, self.b * x + self.d * y + self.ty)
    }

    fn inverse(self) -> Affine {
        let det = self.a * self.d - self.b * self.c;
        let det = if det.abs() < 1e-9 { 1e-9 } else { det };
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Affine { a, b, c, d, tx: -(a * self.tx + c * self.ty), ty: -(b * self.tx + d * self.ty) }
    }

    fn translate(x: f32, y: f32) -> Affine {
        Affine { tx: x, ty: y, ..Affine::IDENTITY }
    }

    fn rotate_scale(degrees: f32, sx: f32, sy: f32) -> Affine {
        let (s, c) = degrees.to_radians().sin_cos();
        Affine { a: c * sx, b: s * sx, c: -s * sy, d: c * sy, tx: 0.0, ty: 0.0 }
    }
}

/// What the screen knows of a pane after the calculation.
#[derive(Clone, Copy)]
struct Placed {
    /// From the local space of the pane (its origin point is (0, 0)) to the space of the layout.
    world: Affine,
    /// The rectangle in the local space: left, top, right, bottom (y up).
    rect: (f32, f32, f32, f32),
    visible: bool,
    alpha: f32,
}

fn place_all(layout: &Bclyt, nodes: &[Node]) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let b = node.block;
        let parent = node.parent.map(|p| placed[p]);
        let (pw, palpha, pvis) = match parent {
            Some(p) => (p.world, p.alpha, p.visible),
            None => (Affine::IDENTITY, 1.0, true),
        };
        let pos = layout.position(b);
        let rot = layout.rotation(b);
        let sc = layout.scale(b);
        let world = pw.then(Affine::translate(pos[0], pos[1])).then(Affine::rotate_scale(rot[2], sc[0], sc[1]));
        let [w, h] = layout.size(b);
        let (c, r) = layout.origin(b);
        let left = -(c as f32) * w / 2.0;
        let top = r as f32 * h / 2.0;
        let own = layout.alpha(b) as f32 / 255.0;
        let alpha = if layout.influenced_alpha(b) { own * palpha } else { own };
        placed.push(Placed { world, rect: (left, top, left + w, top - h), visible: pvis && layout.visible(b), alpha });
    }
    placed
}

fn rgba(c: [u8; 4], alpha: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], (c[3] as f32 * alpha).round().clamp(0.0, 255.0) as u8)
}

impl LayoutPage {
    fn scan(&mut self, session: &Session) {
        let Some(dump) = session.dump.as_ref() else { return };
        let root = dump.archive(0);
        if self.scanned_for.as_ref() == Some(&root) {
            return;
        }
        self.scanned_for = Some(root);
        self.entries.clear();
        self.open = None;
        let (sender, receiver) = channel();
        self.scan = Some(receiver);
        let dump = dump.clone();
        std::thread::spawn(move || {
            let mut found = Vec::new();
            for number in 0..300 {
                let path = dump.archive(number);
                if !path.is_file() || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > BIGGEST_ARCHIVE {
                    continue;
                }
                let Ok(garc) = Garc::open(&path) else { continue };
                for file in 0..garc.file_count() {
                    let languages = garc.languages(file);
                    for &language in &languages {
                        let Some(bytes) = garc.file(file, language) else { continue };
                        let Ok(bundle) = Bundle::parse(bytes) else { continue };
                        for layout in bundle.layouts() {
                            found.push(Entry { archive: number, file, language, languages: languages.len(), layout });
                        }
                    }
                }
            }
            let _ = sender.send(found);
        });
    }

    fn load(&mut self, ctx: &egui::Context, session: &Session, entry: Entry) {
        let result = (|| -> Result<Open, String> {
            let dump = session.dump.as_ref().ok_or("No dump.")?;
            let sd_file = session.sd.as_ref().and_then(|sd| entry.sd_path().and_then(|path| sd.read(&path)));
            let replaced = sd_file.is_some();
            let bundle = match sd_file {
                Some(bytes) => Bundle::parse(&bytes)?,
                None => {
                    let garc = Garc::open(&dump.archive(entry.archive))?;
                    Bundle::parse(garc.file(entry.file, entry.language).ok_or("This file is not in the archive.")?)?
                }
            };
            let file = bundle.darc.find(&entry.layout).ok_or("This layout is not in the file.")?;
            let layout = Bclyt::parse(&file.data)?;
            Ok(Open { entry: entry.clone(), bundle, layout, selected: None, undo: Vec::new(), redo: Vec::new(), textures: HashMap::new(), dirty: false, replaced, scroll_tree: true, edit_open: false, animation: None, retired: Vec::new() })
        })();
        let _ = ctx;
        match result {
            Ok(open) => {
                self.open = Some(open);
                self.status = None;
            }
            Err(e) => self.status = Some((false, e)),
        }
    }
}

impl Open {
    /// Forgets the textures (all of them, or the ones of an image), so that they are made again from the bundle.
    fn forget_textures(&mut self, image: Option<&str>) {
        let keys: Vec<String> = self.textures.keys().filter(|k| image.map(|name| k.starts_with(&format!("{name}#"))).unwrap_or(true)).cloned().collect();
        for key in keys {
            if let Some(Some(handle)) = self.textures.remove(&key) {
                self.retired.push(handle);
            }
        }
    }

    /// Writes the layout that is shown into the bundle.
    fn commit(&mut self) {
        let bytes = self.layout.to_bytes();
        self.bundle.darc.replace(&self.entry.layout, bytes);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { bundle: self.bundle.clone(), layout: self.layout.clone() }
    }

    /// Call before a change, so that it can be undone.
    fn before_change(&mut self) {
        let s = self.snapshot();
        self.undo.push(s);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            let now = self.snapshot();
            self.redo.push(now);
            self.restore(s);
        }
    }

    fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            let now = self.snapshot();
            self.undo.push(now);
            self.restore(s);
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.bundle = s.bundle;
        self.layout = s.layout;
        self.forget_textures(None);
        if self.selected.map(|b| b >= self.layout.blocks.len()).unwrap_or(false) {
            self.selected = None;
        }
        self.dirty = true;
    }

    /// The image of a name, as a texture of the screen.
    fn texture(&mut self, ctx: &egui::Context, name: &str, wrap: (u8, u8)) -> Option<egui::TextureHandle> {
        // One texture of the screen for each way to go outside of the image (the screen has one way for both directions).
        let mode = wrap.0.max(wrap.1);
        let key = format!("{name}#{mode}");
        if !self.textures.contains_key(&key) {
            let handle = self.bundle.darc.find(name).and_then(|f| tex::decode(&f.data).ok()).map(|p| {
                let image = egui::ColorImage::from_rgba_unmultiplied([p.width, p.height], &p.rgba);
                let wrap_mode = match mode {
                    1 => egui::TextureWrapMode::Repeat,
                    2 => egui::TextureWrapMode::MirroredRepeat,
                    _ => egui::TextureWrapMode::ClampToEdge,
                };
                ctx.load_texture(format!("layout/{key}"), image, egui::TextureOptions { wrap_mode, ..egui::TextureOptions::LINEAR })
            });
            self.textures.insert(key.clone(), handle);
        }
        self.textures[&key].clone()
    }
}

impl Module for LayoutPage {
    fn title(&self) -> &str {
        "Layout"
    }

    fn icon(&self) -> &str {
        "🖼"
    }

    fn save_changes(&mut self, ctx: &egui::Context, session: &mut Session) {
        if let Some(open) = self.open.as_mut() {
            apply_action(ctx, open, Action::Save, session, &mut self.status, &mut self.confirm_delete);
        }
    }

    fn unsaved(&self) -> bool {
        self.open.as_ref().map(|o| o.dirty).unwrap_or(false)
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        if session.dump.is_none() {
            widget::missing_card(ui, "Open the dump of the game on the Home page to edit the layouts.");
            return;
        }
        self.scan(session);
        if let Some(receiver) = &self.scan {
            if let Ok(found) = receiver.try_recv() {
                self.entries = found;
                self.scan = None;
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let ctx = ui.ctx().clone();
        let mut to_open: Option<Entry> = None;

        let scroll_list = std::mem::take(&mut self.scroll_list);
        let mut displayed: Vec<Entry> = Vec::new();
        egui::SidePanel::left("layout_list").resizable(true).default_width(250.0).show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search a layout or an archive").desired_width(f32::INFINITY));
            ui.add_space(4.0);
            if self.scan.is_some() {
                ui.label(widget::hint("Looking for the layouts in the archives…"));
            }
            let filter = self.filter.to_lowercase();
            let current = self.open.as_ref().map(|o| o.entry.clone());
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut groups: Vec<&'static str> = self.entries.iter().map(|e| group(e.archive)).collect();
                groups.sort_by_key(|g| (*g == "Other", *g));
                groups.dedup();
                for g in groups {
                    let items: Vec<&Entry> = self
                        .entries
                        .iter()
                        .filter(|e| group(e.archive) == g)
                        .filter(|e| filter.is_empty() || e.layout.to_lowercase().contains(&filter) || e.path().contains(&filter))
                        .collect();
                    if items.is_empty() {
                        continue;
                    }
                    let id = ui.make_persistent_id(("layout_group", g));
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, !filter.is_empty() || g == "Bag" || g == "Party" || g == "Summary")
                        .show_header(ui, |ui| {
                            ui.label(egui::RichText::new(format!("{g} ({})", items.len())).strong());
                        })
                        .body(|ui| {
                            for e in items {
                                displayed.push(e.clone());
                                let selected = current.as_ref().map(|c| c.archive == e.archive && c.file == e.file && c.language == e.language && c.layout == e.layout).unwrap_or(false);
                                let label = if e.languages > 1 { format!("{}  ·  {} (lang {})", e.layout, e.path(), e.language) } else { format!("{}  ·  {}", e.layout, e.path()) };
                                let response = ui.selectable_label(selected, label);
                                if selected && scroll_list {
                                    response.scroll_to_me(None);
                                }
                                if response.clicked() {
                                    to_open = Some(e.clone());
                                }
                            }
                        });
                }
            });
            // The arrows up and down choose the previous or the next layout of the list.
            if zone(ui, "list", ui.max_rect()) {
                let step = focus::steps(ui, false);
                match &current {
                    Some(c) => {
                        let at = displayed.iter().position(|e| e.archive == c.archive && e.file == c.file && e.language == c.language && e.layout == c.layout);
                        if let Some(to) = at.and_then(|at| focus::stepped(at, step, displayed.len())) {
                            to_open = Some(displayed[to].clone());
                        }
                    }
                    None if step != 0 && !displayed.is_empty() => to_open = Some(displayed[0].clone()),
                    None => {}
                }
            }
        });
        if to_open.is_some() {
            self.scroll_list = true;
        }
        if let Some(entry) = to_open {
            if let Some(open) = self.open.as_mut() {
                open.commit();
            }
            self.load(&ctx, session, entry);
        }

        let Some(open) = self.open.as_mut() else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(widget::title("Layout"));
                ui.label(widget::hint("Choose a layout on the left: a menu of the game."));
            });
            return;
        };

        // ---- The history: Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z does again (not while a text is being typed) ----
        open.retired.clear();
        if !ui.input(|i| i.pointer.any_down()) {
            open.edit_open = false;
        }
        if !ui.ctx().wants_keyboard_input() {
            let (redo, undo) = ui.input_mut(|i| {
                let redo = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
                (redo, i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z))
            });
            if undo {
                open.undo();
            }
            if redo {
                open.redo();
            }
        }

        // ---- The panel of the properties (right) ----
        let mut action: Option<Action> = None;
        egui::SidePanel::right("layout_props").resizable(true).default_width(300.0).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.animations_tab, false, "Panes");
                ui.selectable_value(&mut self.animations_tab, true, "Animations");
            });
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if self.animations_tab {
                    animations(ui, open);
                } else {
                    tree_and_properties(ui, open, &mut action, &mut self.confirm_delete);
                }
            });
        });

        // ---- The toolbar and the screen (center) ----
        let mut reset_asked = false;
        ui.horizontal_wrapped(|ui| {
            let clicks = widget::action_bar(ui, widget::ActionBar { export: Some(true), import: Some(true), reset: Some(open.replaced), save: Some(open.dirty), ..Default::default() });
            if clicks.export {
                open.commit();
                let name = format!("layout_{}_{}.bin", open.entry.archive, open.entry.file);
                self.status = export_bytes("Export the layout file", &name, ("Layout file", &["bin"]), &open.bundle.to_bytes()).or(self.status.take());
            }
            if clicks.import {
                match import_bytes("Import a layout file", ("Layout file", &["bin"])).map(|r| r.and_then(|bytes| Bundle::parse(&bytes))) {
                    Some(Ok(bundle)) => match bundle.darc.find(&open.entry.layout).ok_or_else(|| format!("This file has no layout {}.", open.entry.layout)).and_then(|f| Bclyt::parse(&f.data)) {
                        Ok(layout) => {
                            open.bundle = bundle;
                            open.layout = layout;
                            open.selected = None;
                            open.undo.clear();
                            open.redo.clear();
                            open.forget_textures(None);
                            open.dirty = true;
                            self.status = Some((true, "Imported. Save to keep it.".to_string()));
                        }
                        Err(message) => self.status = Some((false, message)),
                    },
                    Some(Err(message)) => self.status = Some((false, message)),
                    None => {}
                }
            }
            if clicks.save {
                action = Some(Action::Save);
            }
            reset_asked = clicks.reset;
            if open.dirty {
                widget::badge(ui, "Not saved", color::AMBER);
            } else if open.replaced {
                widget::badge(ui, "From the SD card", color::TEAL);
            } else {
                widget::badge(ui, "From the romfs", color::GREEN);
            }
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 0.5..=4.0).show_value(false));
            ui.checkbox(&mut self.show_hidden, "Show hidden panes");
            ui.checkbox(&mut self.show_text, "Show texts");
        });
        self.reset_open |= reset_asked;
        if widget::reset_window(&ctx, &mut self.reset_open, &format!("The layout {}", open.entry.layout), &mut []) {
            action = Some(Action::Reset);
        }
        if let Some((ok, text)) = &self.status {
            ui.colored_label(if *ok { color::GREEN } else { color::ROSE }, text);
        }
        ui.label(widget::hint(&format!("{} · {}", open.entry.path(), open.entry.layout)))
            .on_hover_text("Drag a pane to move it, drag a handle to resize. Arrow keys move by 1 (Shift: 8). Delete removes. Ctrl+Z goes back, Ctrl+Y goes forward.");

        canvas(ui, open, self.zoom, self.show_hidden, self.show_text, &mut self.drag, &mut action);

        if let Some(action) = action {
            apply_action(&ctx, open, action, session, &mut self.status, &mut self.confirm_delete);
        }
    }
}

/// The color of the outlines of the panes (the hit boxes): it must stand out from the white of the menus.
const HITBOX: egui::Color32 = egui::Color32::from_rgb(255, 40, 110);

/// The coordinates of the image of a corner of a picture, after the matrix of the image of its material (translation, rotation in
/// degrees, scale). The rotation and the scale turn around the center of the image: a scale of -1 flips the image, and with a wrap that
/// repeats or mirrors, a pane can show the image around its edge (for example the two halves of a symmetric background).
fn texture_matrix(srt: [f32; 5], uv: [f32; 2]) -> [f32; 2] {
    let [tx, ty, rotation, sx, sy] = srt;
    let (sin, cos) = rotation.to_radians().sin_cos();
    let (dx, dy) = (uv[0] - 0.5, uv[1] - 0.5);
    [cos * sx * dx - sin * sy * dy + 0.5 + tx, sin * sx * dx + cos * sy * dy + 0.5 + ty]
}

/// A zone of the page that the arrow keys can work in: a click in the zone gives it the focus, and the arrows then change the choice.
/// Gives true when the keys are for the zone.
fn zone(ui: &egui::Ui, name: &str, rect: egui::Rect) -> bool {
    let clicked = ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(rect);
    focus::focus(ui, egui::Id::new(("layout_zone", name)), rect, clicked, false, None)
}

enum Action {
    Save,
    Reset,
    Select(Option<usize>),
    Delete,
    Duplicate,
    AddPane,
    ReplaceImage(String),
}

fn apply_action(ctx: &egui::Context, open: &mut Open, action: Action, session: &mut Session, status: &mut Option<(bool, String)>, confirm: &mut bool) {
    match action {
        Action::Select(block) => {
            open.selected = block;
            *confirm = false;
        }
        Action::Delete => {
            if let Some(b) = open.selected {
                open.before_change();
                open.layout.delete(b);
                open.selected = None;
                *confirm = false;
            }
        }
        Action::Duplicate => {
            if let Some(b) = open.selected {
                open.before_change();
                let (at, names) = open.layout.duplicate(b);
                for n in names {
                    open.bundle.add_pane_name(&n);
                }
                open.selected = Some(at);
            }
        }
        Action::AddPane => {
            let after = open.selected.or_else(|| open.layout.nodes().first().map(|n| n.block));
            if let Some(after) = after {
                open.before_change();
                let (at, name) = open.layout.add_empty(after);
                open.bundle.add_pane_name(&name);
                open.selected = Some(at);
            }
        }
        Action::ReplaceImage(name) => {
            let Some(path) = rfd::FileDialog::new().add_filter("PNG picture", &["png"]).pick_file() else { return };
            let result = image::open(&path).map_err(|e| e.to_string()).and_then(|img| {
                let img = img.to_rgba8();
                Clim::from_rgba8(img.width() as usize, img.height() as usize, img.as_raw())
            });
            match result {
                Ok(clim) => {
                    open.before_change();
                    open.bundle.darc.replace(&name, clim.to_bytes());
                    open.forget_textures(Some(&name));
                    *status = Some((true, format!("{name} is replaced. Its size is now taken from the picture; set the size of the pane if needed.")));
                }
                Err(e) => *status = Some((false, e)),
            }
        }
        Action::Save => {
            open.commit();
            let Some(sd) = session.sd.as_ref() else {
                *status = Some((false, "Open the SD card folder on the Home page first.".into()));
                return;
            };
            let Some(sd_path) = open.entry.sd_path() else {
                *status = Some((false, "This file has a version for each language: the plugin replaces a file by its number only, so it cannot be saved.".into()));
                return;
            };
            match sd.write(&sd_path, &open.bundle.to_bytes()) {
                Ok(path) => {
                    open.dirty = false;
                    open.replaced = true;
                    *status = Some((true, format!("Saved: {}", path.display())));
                }
                Err(e) => *status = Some((false, e)),
            }
        }
        Action::Reset => {
            if let Some(sd) = session.sd.as_ref() {
                if let Some(path) = open.entry.sd_path() {
                    let _ = sd.remove(&path);
                }
            }
            let entry = open.entry.clone();
            let _ = ctx;
            if let (Some(dump), true) = (session.dump.as_ref(), true) {
                if let Ok(garc) = Garc::open(&dump.archive(entry.archive)) {
                    if let Some(bytes) = garc.file(entry.file, entry.language) {
                        if let Ok(bundle) = Bundle::parse(bytes) {
                            if let Some(file) = bundle.darc.find(&entry.layout) {
                                if let Ok(layout) = Bclyt::parse(&file.data) {
                                    open.bundle = bundle;
                                    open.layout = layout;
                                    open.forget_textures(None);
                                    open.undo.clear();
                                    open.redo.clear();
                                    open.selected = None;
                                    open.dirty = false;
                                    open.replaced = false;
                                    *status = Some((true, "The layout of the game is back.".into()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn tree_and_properties(ui: &mut egui::Ui, open: &mut Open, action: &mut Option<Action>, confirm: &mut bool) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(widget::title("Panes"));
        if ui.button("＋ Empty pane").clicked() {
            *action = Some(Action::AddPane);
        }
    });
    let nodes = open.layout.nodes();
    let tree_top = ui.cursor().top();
    for node in &nodes {
        let b = node.block;
        let name = open.layout.name(b);
        let selected = open.selected == Some(b);
        ui.horizontal(|ui| {
            ui.add_space(node.depth as f32 * 12.0);
            let hidden = !open.layout.visible(b);
            let label = egui::RichText::new(format!("{}  {}", icon(node.kind), name));
            let label = if hidden { label.weak() } else { label };
            let response = ui.selectable_label(selected, label);
            if selected && open.scroll_tree {
                response.scroll_to_me(None);
            }
            if response.clicked() {
                *action = Some(Action::Select(Some(b)));
            }
        });
    }
    open.scroll_tree = false;
    // The arrows up and down choose the previous or the next pane of the tree.
    let tree_rect = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), tree_top), egui::pos2(ui.max_rect().right(), ui.cursor().top().max(tree_top)));
    if zone(ui, "tree", tree_rect) {
        let at = open.selected.and_then(|b| nodes.iter().position(|n| n.block == b));
        let step = focus::steps(ui, false);
        let to = match at {
            Some(at) => focus::stepped(at, step, nodes.len()),
            None if step != 0 && !nodes.is_empty() => Some(0),
            None => None,
        };
        if let Some(to) = to {
            *action = Some(Action::Select(Some(nodes[to].block)));
            open.scroll_tree = true;
        }
    }
    ui.separator();
    let Some(b) = open.selected.filter(|&b| b < open.layout.blocks.len()) else {
        ui.label(widget::hint("Click a pane in the tree or on the screen."));
        return;
    };
    let Some(node) = nodes.iter().find(|n| n.block == b).copied() else { return };
    ui.label(widget::title(&format!("{} “{}”", node.kind.label(), open.layout.name(b))));
    let mut changed = false;
    let before = open.layout.clone();

    let mut visible = open.layout.visible(b);
    if ui.checkbox(&mut visible, "Visible").changed() {
        open.layout.set_visible(b, visible);
        changed = true;
    }
    let mut pos = open.layout.position(b);
    ui.horizontal(|ui| {
        ui.label("Position");
        changed |= ui.add(egui::DragValue::new(&mut pos[0]).speed(0.5).prefix("x ")).changed();
        changed |= ui.add(egui::DragValue::new(&mut pos[1]).speed(0.5).prefix("y ")).changed();
    });
    open.layout.set_position(b, pos);
    let mut size = open.layout.size(b);
    ui.horizontal(|ui| {
        ui.label("Size");
        changed |= ui.add(egui::DragValue::new(&mut size[0]).speed(0.5).prefix("w ")).changed();
        changed |= ui.add(egui::DragValue::new(&mut size[1]).speed(0.5).prefix("h ")).changed();
    });
    open.layout.set_size(b, size);
    let mut scale = open.layout.scale(b);
    ui.horizontal(|ui| {
        ui.label("Scale");
        changed |= ui.add(egui::DragValue::new(&mut scale[0]).speed(0.01)).changed();
        changed |= ui.add(egui::DragValue::new(&mut scale[1]).speed(0.01)).changed();
    });
    open.layout.set_scale(b, scale);
    let mut rot = open.layout.rotation(b);
    ui.horizontal(|ui| {
        ui.label("Rotation");
        changed |= ui.add(egui::DragValue::new(&mut rot[2]).speed(0.5).suffix("°")).changed();
    });
    open.layout.set_rotation(b, rot);
    let mut alpha = open.layout.alpha(b) as u32;
    ui.horizontal(|ui| {
        ui.label("Opacity");
        changed |= ui.add(egui::Slider::new(&mut alpha, 0..=255)).changed();
    });
    open.layout.set_alpha(b, alpha as u8);
    let (c, r) = open.layout.origin(b);
    ui.horizontal(|ui| {
        ui.label("Origin");
        egui::Grid::new("origin_grid").spacing([2.0, 2.0]).show(ui, |ui| {
            for row in 0..3u8 {
                for column in 0..3u8 {
                    if ui.selectable_label(c == column && r == row, "•").clicked() && !(c == column && r == row) {
                        open.layout.set_origin(b, column, row);
                        changed = true;
                    }
                }
                ui.end_row();
            }
        });
    });

    match node.kind {
        Kind::Text => {
            ui.label(widget::hint(&format!("Text in the layout: “{}”. The game usually writes its own text here.", open.layout.text(b))));
            let mut fs = open.layout.font_size(b);
            ui.horizontal(|ui| {
                ui.label("Character size");
                changed |= ui.add(egui::DragValue::new(&mut fs[0]).speed(0.1)).changed();
                changed |= ui.add(egui::DragValue::new(&mut fs[1]).speed(0.1)).changed();
            });
            open.layout.set_font_size(b, fs);
        }
        Kind::Picture => {
            let textures = open.layout.textures();
            let materials = open.layout.materials();
            let name = open.layout.material_index(b).and_then(|m| materials.get(m)).and_then(|m| m.texture).and_then(|t| textures.get(t)).cloned();
            match name {
                Some(name) => {
                    ui.label(format!("Image: {name}"));
                    if ui.button("Replace the image with a PNG…").clicked() {
                        *action = Some(Action::ReplaceImage(name));
                    }
                    ui.label(widget::hint("Every pane that uses this image changes."));
                }
                None => {
                    ui.label(widget::hint("This picture has no image."));
                }
            }
        }
        _ => {}
    }

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Duplicate").clicked() {
            *action = Some(Action::Duplicate);
        }
        if *confirm {
            if ui.button(egui::RichText::new("Delete for sure").color(color::ROSE)).clicked() {
                *action = Some(Action::Delete);
            }
            if ui.button("Cancel").clicked() {
                *confirm = false;
            }
        } else if ui.button("Delete").clicked() {
            *confirm = true;
        }
    });
    if *confirm {
        ui.label(widget::hint("The game finds panes by their place in a table. If the game writes in this pane (a name, a number), deleting it can stop the game. Hiding is safer."));
    }

    if changed && open.layout != before {
        // A drag of a value changes it at each frame: the history keeps the state before the first change only.
        if !open.edit_open {
            let snapshot = Snapshot { bundle: open.bundle.clone(), layout: before };
            open.undo.push(snapshot);
            if open.undo.len() > HISTORY {
                open.undo.remove(0);
            }
            open.redo.clear();
            open.edit_open = true;
        }
        open.dirty = true;
    }
}

/// The Animations tab: the animation files of the bundle, and the numbers of the one that is chosen.
fn animations(ui: &mut egui::Ui, open: &mut Open) {
    ui.add_space(4.0);
    ui.label(widget::title("Animations"));
    ui.label(widget::hint("The animations that the game plays on this layout: a color that changes, a pane that moves. The game chooses which one to play."));
    let mut names: Vec<String> = open.bundle.darc.files.iter().filter_map(|f| f.path.rsplit('/').next()).filter(|n| n.ends_with(".bclan")).map(String::from).collect();
    names.sort();
    if names.is_empty() {
        ui.label(widget::hint("This layout has no animation."));
        return;
    }
    egui::ScrollArea::vertical().id_salt("animation_list").max_height(150.0).show(ui, |ui| {
        for name in &names {
            let selected = open.animation.as_deref() == Some(name.as_str());
            if ui.selectable_label(selected, name.trim_end_matches(".bclan")).clicked() {
                open.animation = Some(name.clone());
            }
        }
    });
    ui.separator();
    let Some(name) = open.animation.clone().filter(|n| names.contains(n)) else {
        ui.label(widget::hint("Choose an animation."));
        return;
    };
    let Some(data) = open.bundle.darc.find(&name).map(|f| f.data.clone()) else { return };
    let mut anim = match Bclan::parse(&data) {
        Ok(a) => a,
        Err(e) => {
            ui.colored_label(color::ROSE, e);
            return;
        }
    };
    let mut changed = false;

    let mut frames = anim.frames() as u32;
    ui.horizontal(|ui| {
        ui.label("Length");
        if ui.add(egui::DragValue::new(&mut frames).range(0..=65535).suffix(" frames")).changed() {
            anim.set_frames(frames as u16);
            changed = true;
        }
        let mut looped = anim.looped();
        if ui.checkbox(&mut looped, "Loops").changed() {
            anim.set_looped(looped);
            changed = true;
        }
    });

    let tracks = anim.tracks.clone();
    for track in &tracks {
        let title = format!("{}  ({})", track.name, if track.kind == 1 { "material" } else { "pane" });
        egui::CollapsingHeader::new(title).id_salt(("anim_content", &name, &track.name)).default_open(tracks.len() <= 3).show(ui, |ui| {
            let mut kinds: Vec<[u8; 4]> = Vec::new();
            for t in &track.channels {
                if !kinds.contains(&t.kind) {
                    kinds.push(t.kind);
                }
            }
            for kind in kinds {
                ui.label(egui::RichText::new(bclan::kind_name(&kind)).strong());
                let channels: Vec<&bclan::Channel> = track.channels.iter().filter(|t| t.kind == kind).collect();
                // The colors: one color box for each key, for the channels that have a red, a green and a blue.
                let mut shown_as_color: Vec<u8> = Vec::new();
                if bclan::is_color(&kind) {
                    for slot in 0..8u8 {
                        let channel = |c: u8| channels.iter().find(|t| t.component == slot * 4 + c && t.curve == 2);
                        let (Some(r), Some(g), Some(b)) = (channel(0), channel(1), channel(2)) else { continue };
                        if r.keys.len() != g.keys.len() || r.keys.len() != b.keys.len() {
                            continue;
                        }
                        shown_as_color.extend([slot * 4, slot * 4 + 1, slot * 4 + 2]);
                        for k in 0..r.keys.len() {
                            let frame = anim.frame(&r.keys[k]);
                            let to_u8 = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                            let mut rgb = [to_u8(anim.value(&r.keys[k], 2)), to_u8(anim.value(&g.keys[k], 2)), to_u8(anim.value(&b.keys[k], 2))];
                            ui.horizontal(|ui| {
                                ui.label(format!("{}  ·  frame {}", bclan::component_name(&kind, slot * 4).trim_end_matches(" R"), frame));
                                if ui.color_edit_button_srgb(&mut rgb).changed() {
                                    anim.set_value(&r.keys[k], 2, rgb[0] as f32);
                                    anim.set_value(&g.keys[k], 2, rgb[1] as f32);
                                    anim.set_value(&b.keys[k], 2, rgb[2] as f32);
                                    changed = true;
                                }
                                ui.label(widget::hint(&format!("{}, {}, {}", rgb[0], rgb[1], rgb[2])));
                            });
                        }
                    }
                }
                for t in channels.iter().filter(|t| !shown_as_color.contains(&t.component)) {
                    ui.label(format!("{} ({})", bclan::component_name(&kind, t.component), ["constant", "step", "smooth"].get(t.curve as usize).copied().unwrap_or("?")));
                    for key in &t.keys {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            let mut frame = anim.frame(key);
                            if ui.add(egui::DragValue::new(&mut frame).speed(0.1).prefix("frame ")).changed() {
                                anim.set_frame(key, frame);
                                changed = true;
                            }
                            let mut value = anim.value(key, t.curve);
                            if ui.add(egui::DragValue::new(&mut value).speed(if t.curve == 2 { 0.1 } else { 1.0 }).prefix("value ")).changed() {
                                anim.set_value(key, t.curve, value);
                                changed = true;
                            }
                            if let Some(mut slope) = anim.slope(key) {
                                if ui.add(egui::DragValue::new(&mut slope).speed(0.05).prefix("slope ")).changed() {
                                    anim.set_slope(key, slope);
                                    changed = true;
                                }
                            }
                        });
                    }
                }
            }
        });
    }

    if changed {
        // One edit (a drag of a value) is one step of the history.
        if !open.edit_open {
            let snapshot = open.snapshot();
            open.undo.push(snapshot);
            if open.undo.len() > HISTORY {
                open.undo.remove(0);
            }
            open.redo.clear();
            open.edit_open = true;
        }
        open.bundle.darc.replace(&name, anim.to_bytes());
        open.dirty = true;
    }
}

fn icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Empty => "▫",
        Kind::Picture => "🖼",
        Kind::Text => "T",
        Kind::Window => "▭",
        Kind::Bounding => "⬚",
    }
}

/// Draws the layout and handles the mouse.
fn canvas(ui: &mut egui::Ui, open: &mut Open, zoom: f32, show_hidden: bool, show_text: bool, drag: &mut Option<Drag>, action: &mut Option<Action>) {
    let (sw, sh) = open.layout.screen();
    let avail = ui.available_size();
    let size = egui::vec2((sw * zoom + 60.0).max(avail.x), (sh * zoom + 60.0).max(avail.y));
    let ctx = ui.ctx().clone();
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, egui::Color32::from_gray(38));
        let center = egui::pos2(response.rect.min.x + 30.0 + sw * zoom / 2.0, response.rect.min.y + 30.0 + sh * zoom / 2.0);
        let screen_rect = egui::Rect::from_center_size(center, egui::vec2(sw * zoom, sh * zoom));
        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_gray(70));
        let to_screen = |x: f32, y: f32| egui::pos2(center.x + x * zoom, center.y - y * zoom);
        let from_screen = |p: egui::Pos2| ((p.x - center.x) / zoom, -(p.y - center.y) / zoom);

        let nodes = open.layout.nodes();
        let placed = place_all(&open.layout, &nodes);
        let textures = open.layout.textures();
        let materials = open.layout.materials();

        for (i, node) in nodes.iter().enumerate() {
            let p = placed[i];
            if !p.visible && !show_hidden {
                continue;
            }
            let b = node.block;
            let (l, t, r, bt) = p.rect;
            let corners = [(l, t), (r, t), (l, bt), (r, bt)].map(|(x, y)| {
                let (wx, wy) = p.world.apply(x, y);
                to_screen(wx, wy)
            });
            let ghost = if p.visible { 1.0 } else { 0.35 };
            match node.kind {
                Kind::Picture => {
                    let material = open.layout.material_index(b).and_then(|m| materials.get(m)).copied();
                    let tex_name = material.and_then(|m| m.texture).and_then(|t| textures.get(t)).cloned();
                    let handle = tex_name.and_then(|n| open.texture(&ctx, &n, material.map(|m| m.wrap).unwrap_or((0, 0))));
                    let colors = open.layout.corner_colors(b);
                    let mut uvs = open.layout.corner_uvs(b);
                    if let Some(srt) = material.and_then(|m| m.srt) {
                        for uv in uvs.iter_mut() {
                            *uv = texture_matrix(srt, *uv);
                        }
                    }
                    let mut mesh = match &handle {
                        Some(h) => egui::Mesh::with_texture(h.id()),
                        None => egui::Mesh::default(),
                    };
                    for k in 0..4 {
                        let color = if handle.is_some() { rgba(colors[k], p.alpha * ghost) } else { egui::Color32::from_rgba_unmultiplied(200, 200, 255, (18.0 * p.alpha * ghost) as u8) };
                        // Without an image, the mesh has the texture of the letters of egui: the pixel of white must be used, or its letters show.
                        let uv = if handle.is_some() { egui::pos2(uvs[k][0], uvs[k][1]) } else { egui::epaint::WHITE_UV };
                        mesh.vertices.push(egui::epaint::Vertex { pos: corners[k], uv, color });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);
                    painter.add(egui::Shape::mesh(mesh));
                    if handle.is_none() {
                        // The image is not in the file: the game gives it while it runs (the icon of a Pokémon, of an item...).
                        outline(&painter, &corners, HITBOX.gamma_multiply(0.6));
                    }
                }
                Kind::Text => {
                    if show_text {
                        let text = open.layout.text(b);
                        let text = if text.trim().is_empty() { "…".to_string() } else { text };
                        let fs = open.layout.font_size(b);
                        let colors = open.layout.text_colors(b);
                        let center_pos = egui::pos2((corners[0].x + corners[3].x) / 2.0, (corners[0].y + corners[3].y) / 2.0);
                        let (align, anchor_x) = match open.layout.text_align(b) {
                            1 => (egui::Align2::CENTER_CENTER, center_pos.x),
                            2 => (egui::Align2::RIGHT_CENTER, corners[1].x.max(corners[0].x)),
                            _ => (egui::Align2::LEFT_CENTER, corners[0].x.min(corners[1].x)),
                        };
                        painter.text(egui::pos2(anchor_x, center_pos.y), align, text, egui::FontId::proportional((fs[1].max(8.0) * zoom * 0.8).clamp(6.0, 60.0)), rgba(colors[0], p.alpha * ghost));
                    }
                    outline(&painter, &corners, HITBOX.gamma_multiply(0.75));
                }
                Kind::Window => {
                    let mut mesh = egui::Mesh::default();
                    for c in corners {
                        mesh.vertices.push(egui::epaint::Vertex { pos: c, uv: egui::epaint::WHITE_UV, color: egui::Color32::from_rgba_unmultiplied(255, 255, 255, (35.0 * p.alpha * ghost) as u8) });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);
                    painter.add(egui::Shape::mesh(mesh));
                    outline(&painter, &corners, HITBOX.gamma_multiply(0.9));
                }
                Kind::Bounding => outline(&painter, &corners, HITBOX.gamma_multiply(0.45)),
                Kind::Empty => {
                    if i != 0 {
                        outline(&painter, &corners, HITBOX.gamma_multiply(0.35));
                    }
                }
            }
        }
        painter.rect_stroke(screen_rect, 0.0, egui::Stroke::new(1.0_f32, egui::Color32::from_gray(150)));

        // The selected pane: its outline and its handles.
        let selected_index = open.selected.and_then(|b| nodes.iter().position(|n| n.block == b));
        let mut handles: Vec<(i8, i8, egui::Pos2)> = Vec::new();
        if let Some(i) = selected_index {
            let p = placed[i];
            let (l, t, r, bt) = p.rect;
            let at = |x: f32, y: f32| {
                let (wx, wy) = p.world.apply(x, y);
                to_screen(wx, wy)
            };
            let c = [at(l, t), at(r, t), at(r, bt), at(l, bt)];
            for k in 0..4 {
                painter.line_segment([c[k], c[(k + 1) % 4]], egui::Stroke::new(1.5_f32, color::ACCENT));
            }
            let (mx, my) = ((l + r) / 2.0, (t + bt) / 2.0);
            for (sx, sy, x, y) in [(-1, 1, l, t), (0, 1, mx, t), (1, 1, r, t), (1, 0, r, my), (1, -1, r, bt), (0, -1, mx, bt), (-1, -1, l, bt), (-1, 0, l, my)] {
                handles.push((sx, sy, at(x, y)));
            }
            for (_, _, pos) in &handles {
                painter.rect_filled(egui::Rect::from_center_size(*pos, egui::vec2(8.0, 8.0)), 1.0, egui::Color32::WHITE);
                painter.rect_stroke(egui::Rect::from_center_size(*pos, egui::vec2(8.0, 8.0)), 1.0, egui::Stroke::new(1.0_f32, color::ACCENT));
            }
        }

        // ---- The mouse ----
        let pointer = response.interact_pointer_pos().or_else(|| response.hover_pos());
        if response.drag_started() {
            if let Some(pos) = pointer {
                let handle = handles.iter().find(|(_, _, h)| h.distance(pos) < 8.0).map(|(sx, sy, _)| Drag::Handle(*sx, *sy));
                if let Some(h) = handle {
                    open.before_change();
                    *drag = Some(h);
                } else {
                    // Select what is under the mouse, then move it.
                    let (x, y) = from_screen(pos);
                    if let Some(i) = hit(&nodes, &placed, x, y, show_hidden, &open.layout) {
                        *action = None;
                        open.selected = Some(nodes[i].block);
                        open.before_change();
                        *drag = Some(Drag::Move);
                    } else {
                        *drag = None;
                    }
                }
            }
        }
        if response.dragged() {
            if let (Some(mode), Some(i)) = (*drag, open.selected.and_then(|b| nodes.iter().position(|n| n.block == b))) {
                let delta = response.drag_delta();
                let (dx, dy) = (delta.x / zoom, -delta.y / zoom);
                let b = nodes[i].block;
                let parent_world = nodes[i].parent.map(|p| placed[p].world).unwrap_or(Affine::IDENTITY);
                let inv = Affine { tx: 0.0, ty: 0.0, ..parent_world.inverse() };
                match mode {
                    Drag::Move => {
                        let (lx, ly) = inv.apply(dx, dy);
                        let mut pos = open.layout.position(b);
                        pos[0] += lx;
                        pos[1] += ly;
                        open.layout.set_position(b, pos);
                    }
                    Drag::Handle(sx, sy) => {
                        // The movement in the local space of the pane.
                        let own = Affine { tx: 0.0, ty: 0.0, ..placed[i].world.inverse() };
                        let (lx, ly) = own.apply(dx, dy);
                        let [w, h] = open.layout.size(b);
                        let (c, r) = open.layout.origin(b);
                        let (mut left, mut top) = (-(c as f32) * w / 2.0, r as f32 * h / 2.0);
                        let (mut right, mut bottom) = (left + w, top - h);
                        match sx {
                            -1 => left += lx,
                            1 => right += lx,
                            _ => {}
                        }
                        match sy {
                            1 => top += ly,
                            -1 => bottom += ly,
                            _ => {}
                        }
                        let (nw, nh) = ((right - left).max(1.0), (top - bottom).max(1.0));
                        let anchor = (left + c as f32 * nw / 2.0, top - r as f32 * nh / 2.0);
                        let sc = open.layout.scale(b);
                        let rot = open.layout.rotation(b)[2];
                        let (sin, cos) = rot.to_radians().sin_cos();
                        let (ax, ay) = (anchor.0 * sc[0], anchor.1 * sc[1]);
                        let (px, py) = (ax * cos - ay * sin, ax * sin + ay * cos);
                        let (pxl, pyl) = inv.apply(parent_world.a * px + parent_world.c * py, parent_world.b * px + parent_world.d * py);
                        let mut pos = open.layout.position(b);
                        pos[0] += pxl;
                        pos[1] += pyl;
                        open.layout.set_position(b, pos);
                        open.layout.set_size(b, [nw, nh]);
                    }
                }
                open.dirty = true;
            }
        }
        if response.drag_stopped() {
            if drag.is_some() {
                // Round to whole pixels at the end of a move.
                if let Some(b) = open.selected.filter(|&b| b < open.layout.blocks.len()) {
                    let mut pos = open.layout.position(b);
                    pos[0] = pos[0].round();
                    pos[1] = pos[1].round();
                    open.layout.set_position(b, pos);
                    let mut size = open.layout.size(b);
                    size[0] = size[0].round().max(1.0);
                    size[1] = size[1].round().max(1.0);
                    open.layout.set_size(b, size);
                }
            }
            *drag = None;
        }
        if response.clicked() && drag.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (x, y) = from_screen(pos);
                *action = Some(Action::Select(hit(&nodes, &placed, x, y, show_hidden, &open.layout).map(|i| nodes[i].block)));
                open.scroll_tree = true;
            }
        }

        // ---- The keyboard ----
        if zone(ui, "screen", response.rect) || response.dragged() {
            if let Some(b) = open.selected.filter(|&b| b < open.layout.blocks.len()) {
                let step = if ui.input(|i| i.modifiers.shift) { 8.0 } else { 1.0 };
                let (dx, dy) = ui.input(|i| {
                    (
                        (i.key_pressed(egui::Key::ArrowRight) as i32 - i.key_pressed(egui::Key::ArrowLeft) as i32) as f32,
                        (i.key_pressed(egui::Key::ArrowUp) as i32 - i.key_pressed(egui::Key::ArrowDown) as i32) as f32,
                    )
                });
                if dx != 0.0 || dy != 0.0 {
                    open.before_change();
                    let mut pos = open.layout.position(b);
                    pos[0] += dx * step;
                    pos[1] += dy * step;
                    open.layout.set_position(b, pos);
                }
                if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
                    *action = Some(Action::Delete);
                }
            }
        }
    });
}

fn outline(painter: &egui::Painter, c: &[egui::Pos2; 4], color: egui::Color32) {
    let order = [0, 1, 3, 2];
    for k in 0..4 {
        painter.line_segment([c[order[k]], c[order[(k + 1) % 4]]], egui::Stroke::new(1.0_f32, color));
    }
}

/// The pane under a point of the layout (the one that is drawn on top).
fn hit(nodes: &[Node], placed: &[Placed], x: f32, y: f32, show_hidden: bool, layout: &Bclyt) -> Option<usize> {
    (0..nodes.len()).rev().find(|&i| {
        let p = placed[i];
        if (!p.visible && !show_hidden) || (nodes[i].kind == Kind::Empty && i == 0) || (nodes[i].kind == Kind::Bounding && !show_hidden) {
            return false;
        }
        let (lx, ly) = p.world.inverse().apply(x, y);
        let (l, t, r, b) = p.rect;
        let _ = layout;
        lx >= l.min(r) && lx <= l.max(r) && ly >= b.min(t) && ly <= b.max(t)
    })
}

#[cfg(test)]
mod preview {
    use super::*;
    use sango_core::Dump;

    /// Draws a layout in a PNG file with the same calculation as the page (a check of the geometry and of the images).
    fn render(bundle: &Bundle, layout_name: &str, out: &str) {
        let layout = Bclyt::parse(&bundle.darc.find(layout_name).unwrap().data).unwrap();
        let (sw, sh) = layout.screen();
        let (w, h) = (sw as u32, sh as u32);
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([70, 70, 70, 255]));
        let nodes = layout.nodes();
        let placed = place_all(&layout, &nodes);
        let textures = layout.textures();
        let materials = layout.materials();
        for (i, node) in nodes.iter().enumerate() {
            let p = placed[i];
            if !p.visible || node.kind != Kind::Picture {
                continue;
            }
            let name = layout.material_index(node.block).and_then(|m| materials.get(m)).and_then(|m| m.texture).and_then(|t| textures.get(t)).cloned();
            let pic = name.and_then(|n| bundle.darc.find(&n)).and_then(|f| tex::decode(&f.data).ok());
            let colors = layout.corner_colors(node.block);
            let uvs = layout.corner_uvs(node.block);
            let inv = p.world.inverse();
            let (l, t, r, b) = p.rect;
            for py in 0..h {
                for px in 0..w {
                    let (x, y) = (px as f32 + 0.5 - sw / 2.0, -(py as f32 + 0.5 - sh / 2.0));
                    let (lx, ly) = inv.apply(x, y);
                    if lx < l.min(r) || lx > l.max(r) || ly < b.min(t) || ly > b.max(t) {
                        continue;
                    }
                    let (fu, fv) = ((lx - l) / (r - l), (t - ly) / (t - b));
                    let mut c = [255.0f32; 4];
                    // Bilinear vertex colors (top-left, top-right, bottom-left, bottom-right).
                    for k in 0..4 {
                        c[k] = (colors[0][k] as f32 * (1.0 - fu) + colors[1][k] as f32 * fu) * (1.0 - fv) + (colors[2][k] as f32 * (1.0 - fu) + colors[3][k] as f32 * fu) * fv;
                    }
                    if let Some(pic) = &pic {
                        let u = uvs[0][0] + (uvs[1][0] - uvs[0][0]) * fu;
                        let v = uvs[0][1] + (uvs[2][1] - uvs[0][1]) * fv;
                        let (tx, ty) = (((u * pic.width as f32) as usize).min(pic.width - 1), ((v * pic.height as f32) as usize).min(pic.height - 1));
                        let s = &pic.rgba[(ty * pic.width + tx) * 4..][..4];
                        for k in 0..4 {
                            c[k] = c[k] * s[k] as f32 / 255.0;
                        }
                    }
                    let a = c[3] / 255.0 * p.alpha;
                    let dst = img.get_pixel_mut(px, py);
                    for k in 0..3 {
                        dst.0[k] = (dst.0[k] as f32 * (1.0 - a) + c[k] * a) as u8;
                    }
                }
            }
        }
        img.save(out).unwrap();
    }

    #[test]
    fn preview_layouts() {
        let (Ok(path), Ok(out)) = (std::env::var("SANGO_DUMP"), std::env::var("LAYOUT_PREVIEW")) else { return };
        let dump = Dump::open(&path).unwrap();
        for (archive, file, name) in [(0, 6, "memo.bclyt"), (0, 1, "params.bclyt"), (61, 0, "bag_lower.bclyt"), (61, 0, "bag_upper.bclyt"), (2, 0, "pokelist_lower.bclyt")] {
            let garc = Garc::open(&dump.archive(archive)).unwrap();
            let bundle = Bundle::parse(garc.file(file, 0).unwrap()).unwrap();
            let name = if bundle.darc.find(name).is_some() { name.to_string() } else { bundle.layouts()[0].clone() + ".bclyt" };
            let name = name.trim_end_matches(".bclyt.bclyt").to_string();
            let name = if name.ends_with(".bclyt") { name } else { format!("{name}.bclyt") };
            render(&bundle, &name, &format!("{out}/{archive}_{file}_{}.png", name.trim_end_matches(".bclyt")));
        }
    }
}

/// Writes bytes in a file that the user chooses. Gives what happened, or `None` when the user closed the window.
fn export_bytes(title: &str, name: &str, filter: (&str, &[&str]), bytes: &[u8]) -> Option<(bool, String)> {
    let path = rfd::FileDialog::new().set_title(title).set_file_name(name).add_filter(filter.0, filter.1).save_file()?;
    Some(match std::fs::write(&path, bytes) {
        Ok(()) => (true, format!("Exported to {}.", path.display())),
        Err(e) => (false, format!("{}: {e}", path.display())),
    })
}

/// Reads a file that the user chooses (a layout file stays as it is: the bundle knows when it is compressed).
fn import_bytes(title: &str, filter: (&str, &[&str])) -> Option<Result<Vec<u8>, String>> {
    let path = rfd::FileDialog::new().set_title(title).add_filter(filter.0, filter.1).pick_file()?;
    Some(std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display())))
}
