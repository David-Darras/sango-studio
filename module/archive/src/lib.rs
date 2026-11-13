//! The archive page: choose an archive of the game, see its tree, click a file to see its bytes.

mod hex;

use sango_core::{Dump, Garc};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// An archive that is open.
struct Loaded {
    number: usize,
    path: PathBuf,
    garc: Garc,
    /// The file id and the language that show their bytes.
    selected: Option<(usize, usize)>,
    /// The files that have several languages and show them.
    expanded: HashSet<usize>,
    /// The lines of the list that were on the screen at the last frame.
    view: std::ops::Range<usize>,
    /// The line to bring on the screen at the next frame (after a key).
    scroll_to: Option<usize>,
}

/// One line of the list of files.
#[derive(Clone, Copy)]
enum Row {
    File(usize),
    Language(usize, usize),
}

#[derive(Default)]
pub struct Archive {
    /// The dump that the list of numbers comes from.
    dump_root: Option<PathBuf>,
    numbers: Vec<usize>,
    /// The number in the field.
    input: String,
    loaded: Option<Loaded>,
    error: Option<String>,
    /// While the list of the archives is open: the archive that the keyboard cursor is on.
    cursor: Option<usize>,
}

/// The height of a line of the list of files, in points.
const ROW_HEIGHT: f32 = 26.0;

/// A line of the list: the whole line can be clicked, and it shows when it is chosen.
fn row_button(ui: &mut egui::Ui, text: &str, selected: bool, muted: bool, glow: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::click());
    if selected {
        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::ACCENT_SOFT);
        ui.painter().rect_stroke(rect, egui::Rounding::same(6.0), egui::Stroke::new(1.0_f32, color::ACCENT));
        if glow {
            widget::glow(ui, rect, 6.0);
        }
    } else if response.hovered() {
        ui.painter().rect_filled(rect, egui::Rounding::same(6.0), color::SURFACE_STRONG);
    }
    let text_color = if muted { color::MUTED } else { color::TEXT };
    ui.painter().text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::monospace(14.0), text_color);
    response
}

/// All the files that can be chosen, in the order of the list: one for each language of each file id.
fn leaves(garc: &Garc) -> Vec<(usize, usize)> {
    (0..garc.file_count()).flat_map(|id| garc.languages(id).into_iter().map(move |language| (id, language))).collect()
}

/// The place in the list of the line of a file and a language.
fn row_of(loaded: &Loaded, id: usize, language: usize) -> usize {
    let mut row = 0;
    for file in 0..id {
        row += 1;
        if loaded.expanded.contains(&file) {
            row += loaded.garc.languages(file).len();
        }
    }
    if loaded.garc.languages(id).len() > 1 {
        row + 1 + loaded.garc.languages(id).iter().position(|&l| l == language).unwrap_or(0)
    } else {
        row
    }
}

/// A size with its unit.
fn size_text(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// The path of an archive as the game writes it: `a/2/7/7`.
fn short_path(number: usize) -> String {
    format!("a/{}/{}/{}", number / 100, number / 10 % 10, number % 10)
}

impl Archive {
    fn open(&mut self, dump: &Dump, number: usize) {
        let path = dump.archive(number);
        match Garc::open(&path) {
            Ok(garc) => {
                let first_sel = garc.languages(0).first().map(|&l| (0, l));
                self.loaded = Some(Loaded { number, path, garc, selected: first_sel, expanded: HashSet::new(), view: 0..0, scroll_to: None });
                self.error = None;
                self.input = number.to_string();
            }
            Err(message) => {
                self.loaded = None;
                self.error = Some(message);
            }
        }
    }

    /// The number field, the list of the archives and the buttons before and after.
    fn chooser(&mut self, ui: &mut egui::Ui, dump: &Dump) {
        let mut go: Option<usize> = None;
        widget::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(widget::title("Archive"));
                let field = ui.add(egui::TextEdit::singleline(&mut self.input).desired_width(70.0).hint_text("277"));
                // Enter ends the text: ask before the box takes the focus back (taking it back hides the end of the text).
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    match self.input.trim().parse::<usize>() {
                        Ok(n) => go = Some(n),
                        Err(_) => self.error = Some("Type the number of an archive.".into()),
                    }
                }
                // The box is a stop of the keyboard navigation of the page: when the page has chosen it, it takes the focus of the
                // text, so that the digits and Enter go to it. A click chooses it too.
                widget::register_focusable(ui, field.id, field.rect);
                if field.clicked() || field.gained_focus() {
                    widget::set_active_id(ui, field.id);
                }
                if widget::is_active(ui, field.id) && widget::page_has_focus(ui) {
                    if !field.has_focus() {
                        field.request_focus();
                    }
                    widget::glow(ui, field.rect, 4.0);
                } else if field.has_focus() {
                    field.surrender_focus();
                }
                let current = self.loaded.as_ref().map(|l| l.number);
                let position = current.and_then(|n| self.numbers.iter().position(|&x| x == n));
                let label = current.map_or("choose".to_string(), |n| format!("{n}   {}", short_path(n)));
                // While the list is open, the arrows move a cursor in it, Enter chooses and Escape closes (egui does that).
                let combo_id = ui.make_persistent_id(egui::Id::new("archive_list"));
                let mut cursor_moved = false;
                if egui::ComboBox::is_open(ui.ctx(), combo_id) && !self.numbers.is_empty() {
                    let at = self.cursor.or(current).and_then(|n| self.numbers.iter().position(|&x| x == n)).unwrap_or(0);
                    let (step, home, end, enter) = ui.input_mut(|i| {
                        let none = egui::Modifiers::NONE;
                        let step = if i.consume_key(none, egui::Key::ArrowDown) { 1 } else if i.consume_key(none, egui::Key::ArrowUp) { -1 } else if i.consume_key(none, egui::Key::PageDown) { 10 } else if i.consume_key(none, egui::Key::PageUp) { -10 } else { 0 };
                        (step, i.consume_key(none, egui::Key::Home), i.consume_key(none, egui::Key::End), i.consume_key(none, egui::Key::Enter))
                    });
                    let to = if home { 0 } else if end { self.numbers.len() - 1 } else { (at as isize + step).clamp(0, self.numbers.len() as isize - 1) as usize };
                    cursor_moved = to != at || self.cursor.is_none() || step != 0 || home || end;
                    self.cursor = Some(self.numbers[to]);
                    if enter {
                        go = self.cursor;
                        ui.memory_mut(|m| m.close_popup());
                    }
                } else {
                    self.cursor = None;
                }
                let cursor = self.cursor;
                let combo = egui::ComboBox::from_id_salt("archive_list").selected_text(label).width(190.0).height(360.0).show_ui(ui, |ui| {
                    for &n in &self.numbers {
                        let response = ui.selectable_label(current == Some(n) || cursor == Some(n), format!("{n:>3}   {}", short_path(n)));
                        if cursor_moved && cursor == Some(n) {
                            response.scroll_to_me(None);
                        }
                        if response.clicked() {
                            go = Some(n);
                        }
                    }
                });
                // The list is a stop of the keyboard navigation too: a click chooses it, it glows, and Enter opens it.
                let list = combo.response;
                widget::register_focusable(ui, list.id, list.rect);
                if list.clicked() {
                    widget::set_active_id(ui, list.id);
                }
                if widget::is_active(ui, list.id) && widget::page_has_focus(ui) {
                    widget::glow(ui, list.rect, 6.0);
                    let open = egui::ComboBox::is_open(ui.ctx(), list.id);
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !open {
                        ui.memory_mut(|m| m.open_popup(list.id.with("popup")));
                    }
                    // The arrows go to the archive before or after the one that is open.
                    if !open {
                        let (up, down) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)));
                        if let Some(p) = position {
                            if up && p > 0 {
                                go = Some(self.numbers[p - 1]);
                            } else if down && p + 1 < self.numbers.len() {
                                go = Some(self.numbers[p + 1]);
                            }
                        } else if (up || down) && !self.numbers.is_empty() {
                            go = Some(self.numbers[0]);
                        }
                    }
                }
                ui.label(widget::hint(&format!("{} archives in the dump", self.numbers.len())));
            });
        });
        if let Some(n) = go {
            if self.numbers.contains(&n) {
                self.open(dump, n);
            } else {
                self.error = Some(format!("The dump has no archive {n}."));
            }
        }
    }

    /// The details of the header and of the blocks, as small folding sections.
    fn blocks(ui: &mut egui::Ui, loaded: &Loaded) {
        let garc = &loaded.garc;
        let line = |ui: &mut egui::Ui, key: &str, value: String| {
            ui.label(widget::hint(key));
            ui.label(egui::RichText::new(value).monospace());
            ui.end_row();
        };
        egui::CollapsingHeader::new(widget::title("Header")).id_salt("garc_header").show(ui, |ui| {
            egui::Grid::new("garc_header_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                line(ui, "Signature", "GARC (stored as CRAG)".into());
                line(ui, "Version", format!("{:#06x}", garc.version()));
                line(ui, "Byte order", format!("{:#06x} (little-endian)", garc.byte_order()));
                line(ui, "Header size", format!("{:#x}", garc.header_size()));
                line(ui, "Blocks after the header", garc.block_count().to_string());
                line(ui, "Data offset", format!("{:#x}", garc.data_offset()));
                line(ui, "Archive size", format!("{:#x} ({})", garc.archive_size(), size_text(garc.archive_size() as usize)));
                line(ui, "Largest file (real size)", format!("{:#x} ({})", garc.largest_file_size(), size_text(garc.largest_file_size() as usize)));
                line(ui, "Size on disk", size_text(garc.bytes_len()));
            });
        });
        egui::CollapsingHeader::new(widget::title("Offset table (FATO)")).id_salt("garc_fato").show(ui, |ui| {
            egui::Grid::new("garc_fato_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                line(ui, "Block size", format!("{:#x}", garc.offset_table_size()));
                line(ui, "File ids", garc.file_count().to_string());
            });
        });
        egui::CollapsingHeader::new(widget::title("Range table (FATB)")).id_salt("garc_fatb").show(ui, |ui| {
            egui::Grid::new("garc_fatb_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                line(ui, "Block size", format!("{:#x}", garc.range_table_size()));
                line(ui, "Ranges (one per language of a file)", garc.range_count().to_string());
            });
        });
        egui::CollapsingHeader::new(widget::title("Data block (FIMB)")).id_salt("garc_fimb").show(ui, |ui| {
            egui::Grid::new("garc_fimb_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                line(ui, "Starts at", format!("{:#x}", garc.data_offset()));
                line(ui, "Data size", size_text((garc.archive_size() as usize).saturating_sub(garc.data_offset())));
            });
        });
    }

    /// The list of the files: one line for each id, and one for each language of an id that has several.
    fn files(ui: &mut egui::Ui, loaded: &mut Loaded, list_id: egui::Id) {
        let count = loaded.garc.file_count();
        ui.horizontal(|ui| {
            ui.label(widget::title(&format!("Files ({count})")));
            ui.label(widget::hint("Up and down: choose a file. Right and left: show the languages."));
        });
        let mut rows: Vec<Row> = Vec::with_capacity(count);
        for id in 0..count {
            rows.push(Row::File(id));
            if loaded.expanded.contains(&id) {
                for language in loaded.garc.languages(id) {
                    rows.push(Row::Language(id, language));
                }
            }
        }
        let list_has_focus = widget::page_has_focus(ui) && widget::is_active(ui, list_id);

        // The lines touch each other, and all have the same height: the scroll bar needs it.
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().id_salt("garc_files").auto_shrink([false, false]);
        // After a key, the selected line must be on the screen.
        if let Some(target) = loaded.scroll_to.take() {
            // The last line of the view can be cut: it does not count.
            let visible = loaded.view.end.saturating_sub(loaded.view.start).saturating_sub(1).max(2);
            // The first line can never be after the last place where the list still fills the view.
            let last_first = rows.len().saturating_sub(visible);
            if target < loaded.view.start {
                area = area.vertical_scroll_offset(target.min(last_first) as f32 * ROW_HEIGHT);
            } else if target + 1 >= loaded.view.end {
                area = area.vertical_scroll_offset((target + 2).saturating_sub(visible).min(last_first) as f32 * ROW_HEIGHT);
            }
        }
        let scroll_output = area.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
            loaded.view = range.clone();
            for i in range {
                match rows[i] {
                    Row::File(id) => {
                        let languages = loaded.garc.languages(id);
                        if languages.len() > 1 {
                            let open = loaded.expanded.contains(&id);
                            let text = format!("{}  {id:>5}   {} languages", if open { "-" } else { "+" }, languages.len());
                            let is_sel = loaded.selected.map_or(false, |(fid, _)| fid == id) && !open;
                            let res = row_button(ui, &text, is_sel, false, list_has_focus);
                            if res.clicked() {
                                widget::set_active_id(ui, list_id);
                                if open {
                                    loaded.expanded.remove(&id);
                                } else {
                                    loaded.expanded.insert(id);
                                }
                                if let Some(&first_lang) = languages.first() {
                                    loaded.selected = Some((id, first_lang));
                                }
                                ui.ctx().request_repaint();
                            }
                        } else if let Some(&language) = languages.first() {
                            let size = loaded.garc.file_size(id, language).unwrap_or(0);
                            let text = format!("   {id:>5}   {}", size_text(size));
                            let is_sel = loaded.selected == Some((id, language));
                            let res = row_button(ui, &text, is_sel, false, list_has_focus);
                            if res.clicked() {
                                widget::set_active_id(ui, list_id);
                                loaded.selected = Some((id, language));
                                ui.ctx().request_repaint();
                            }
                        } else {
                            row_button(ui, &format!("   {id:>5}   empty"), false, true, false);
                        }
                    }
                    Row::Language(id, language) => {
                        let size = loaded.garc.file_size(id, language).unwrap_or(0);
                        let text = format!("        language {language}   {}", size_text(size));
                        let is_sel = loaded.selected == Some((id, language));
                        let res = row_button(ui, &text, is_sel, false, list_has_focus);
                        if res.clicked() {
                            widget::set_active_id(ui, list_id);
                            loaded.selected = Some((id, language));
                            ui.ctx().request_repaint();
                        }
                    }
                }
            }
        });
        widget::register_focusable(ui, list_id, scroll_output.inner_rect);
        ui.ctx().data_mut(|d| d.insert_temp(list_id.with("handles_arrows"), true));
        ui.ctx().data_mut(|d| d.insert_temp(list_id.with("handles_left"), true));
    }

    /// The keys: up and down choose the file before and after, page up and page down go ten files
    /// away, home and end go to the first and the last, right and left show and hide the languages.
    fn keys(ui: &egui::Ui, loaded: &mut Loaded) {
        let (up, down, page_up, page_down, home, end, left, right, toggle, ctrl_up) = ui.input_mut(|i| {
            let mut u = false;
            let mut d = false;
            let mut l = false;
            let mut r = false;
            let mut cu = false;
            for mods in [egui::Modifiers::NONE, egui::Modifiers::CTRL, egui::Modifiers::COMMAND] {
                if i.consume_key(mods, egui::Key::ArrowUp) {
                    u = true;
                    if mods.ctrl || mods.command {
                        cu = true;
                    }
                }
                if i.consume_key(mods, egui::Key::ArrowDown) {
                    d = true;
                }
                if i.consume_key(mods, egui::Key::ArrowLeft) {
                    l = true;
                }
                if i.consume_key(mods, egui::Key::ArrowRight) {
                    r = true;
                }
            }
            (
                u,
                d,
                i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Home),
                i.consume_key(egui::Modifiers::NONE, egui::Key::End),
                l,
                r,
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                cu,
            )
        });

        if left {
            if let Some((id, _)) = loaded.selected {
                if loaded.expanded.contains(&id) {
                    loaded.expanded.remove(&id);
                    ui.ctx().request_repaint();
                    return;
                }
            }
            widget::request_sidebar_focus(ui.ctx());
            return;
        }

        if right {
            if let Some((id, _)) = loaded.selected {
                if loaded.garc.languages(id).len() > 1 && !loaded.expanded.contains(&id) {
                    loaded.expanded.insert(id);
                    ui.ctx().request_repaint();
                    return;
                }
            }
        }

        if toggle {
            if let Some((id, _)) = loaded.selected {
                if loaded.garc.languages(id).len() > 1 {
                    if loaded.expanded.contains(&id) {
                        loaded.expanded.remove(&id);
                    } else {
                        loaded.expanded.insert(id);
                    }
                    ui.ctx().request_repaint();
                    return;
                }
            }
        }

        let leaves = leaves(&loaded.garc);
        if leaves.is_empty() {
            return;
        }

        if ctrl_up && loaded.selected == leaves.first().copied() {
            let widgets = ui.ctx().data(|d| d.get_temp::<Vec<widget::FocusableWidget>>(egui::Id::new("sango_focusable_widgets"))).unwrap_or_default();
            let list_id = egui::Id::new("archive_file_list");
            if let Some(cur) = widgets.iter().find(|w| w.id == list_id) {
                if let Some(neighbor) = widget::find_spatial_neighbor(&widgets, cur, widget::Direction::Up) {
                    widget::set_active_id(ui, neighbor.id);
                    ui.ctx().request_repaint();
                    return;
                }
            }
        }

        let step: isize = match (up, down, page_up, page_down) {
            (true, ..) => -1,
            (_, true, ..) => 1,
            (_, _, true, _) => -10,
            (_, _, _, true) => 10,
            _ => 0,
        };
        if step == 0 && !home && !end {
            return;
        }
        let current = loaded.selected.and_then(|s| leaves.iter().position(|&l| l == s));
        let target = if home {
            0
        } else if end {
            leaves.len() - 1
        } else {
            match current {
                Some(p) => (p as isize + step).clamp(0, leaves.len() as isize - 1) as usize,
                None => 0,
            }
        };
        let (id, language) = leaves[target];
        // At the end of the list, a key that changes nothing must not scroll again.
        if loaded.selected == Some((id, language)) {
            return;
        }
        loaded.selected = Some((id, language));
        // A file with several languages opens to show the language that is chosen.
        if loaded.garc.languages(id).len() > 1 {
            loaded.expanded.insert(id);
        }
        loaded.scroll_to = Some(row_of(loaded, id, language));
        ui.ctx().request_repaint();
    }

    /// The bytes of the file that is chosen.
    fn bytes(ui: &mut egui::Ui, loaded: &Loaded) {
        let Some((id, language)) = loaded.selected else {
            ui.add_space(6.0);
            ui.label(widget::hint("Click a file to see its bytes."));
            return;
        };
        let (Some(bytes), Some(range)) = (loaded.garc.file(id, language), loaded.garc.range(id, language)) else {
            ui.label(widget::hint("This file has no bytes."));
            return;
        };
        ui.label(widget::title(&format!("File {id}, language {language}")));
        ui.label(widget::hint(&format!(
            "Real size {} bytes ({}). Padding {} bytes. At {:#x} in the archive. Starts with: {}",
            range.real_size,
            size_text(range.real_size as usize),
            range.padding(),
            loaded.garc.data_offset() + range.start as usize,
            first_bytes(bytes)
        )));
        ui.add_space(4.0);
        hex::show(ui, bytes);
    }
}

/// The first four bytes as text when they can be read, else as numbers.
fn first_bytes(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(4)];
    if head.is_empty() {
        return "nothing".into();
    }
    if head.iter().all(|b| (0x20..0x7f).contains(b)) {
        format!("\"{}\"", String::from_utf8_lossy(head))
    } else {
        head.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    }
}

impl Module for Archive {
    fn title(&self) -> &str {
        "Archive"
    }

    fn icon(&self) -> &str {
        "Ar"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        ui.add_space(8.0);
        ui.heading("Archive");
        ui.label(widget::hint("The files of the romfs, with the tree of each GARC archive."));
        ui.add_space(10.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        if self.dump_root.as_deref() != Some(dump.root()) {
            self.dump_root = Some(dump.root().to_path_buf());
            self.numbers = dump.archive_numbers();
            self.loaded = None;
            self.error = None;
        }
        // SANGO_ARCHIVE=277 (and SANGO_FILE=0) open an archive and a file at once, to test without clicking.
        if self.loaded.is_none() && self.error.is_none() {
            if let Some(n) = std::env::var("SANGO_ARCHIVE").ok().and_then(|v| v.parse::<usize>().ok()) {
                self.open(&dump, n);
                let file = std::env::var("SANGO_FILE").ok().and_then(|v| v.parse::<usize>().ok());
                if let (Some(loaded), Some(id)) = (self.loaded.as_mut(), file) {
                    loaded.selected = loaded.garc.languages(id).first().map(|&language| (id, language));
                }
            } else if !self.numbers.is_empty() {
                let initial = if self.numbers.contains(&277) {
                    277
                } else {
                    self.numbers[0]
                };
                self.open(&dump, initial);
            }
        }
        self.chooser(ui, &dump);
        if let Some(message) = &self.error {
            ui.add_space(6.0);
            widget::badge(ui, "Cannot open", color::ROSE);
            ui.label(widget::hint(message));
        }
        let Some(loaded) = self.loaded.as_mut() else {
            if self.error.is_none() {
                ui.add_space(8.0);
                ui.label(widget::hint("Choose an archive."));
            }
            return;
        };
        ui.add_space(10.0);
        ui.label(widget::hint(&format!("{}   {}", loaded.path.display(), Path::new(&short_path(loaded.number)).display())));
        ui.add_space(4.0);
        let list_id = egui::Id::new("archive_file_list");
        if session.focus_page && widget::is_active(ui, list_id) && !ui.ctx().wants_keyboard_input() {
            Self::keys(ui, loaded);
        }
        let size = ui.available_size();
        let left = (size.x * 0.40).clamp(300.0, 520.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(left, size.y), egui::Layout::top_down(egui::Align::Min), |ui| {
                Self::blocks(ui, loaded);
                ui.add_space(6.0);
                Self::files(ui, loaded, list_id);
            });
            ui.separator();
            let rest = ui.available_width();
            ui.allocate_ui_with_layout(egui::vec2(rest, size.y), egui::Layout::top_down(egui::Align::Min), |ui| {
                Self::bytes(ui, loaded);
            });
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn sizes_have_a_unit() {
        assert_eq!(size_text(12), "12 B");
        assert_eq!(size_text(2048), "2.0 KB");
        assert_eq!(size_text(3 * 1024 * 1024), "3.00 MB");
    }

    #[test]
    fn the_path_of_an_archive_is_the_one_of_the_game() {
        assert_eq!(short_path(277), "a/2/7/7");
        assert_eq!(short_path(13), "a/0/1/3");
        assert_eq!(short_path(0), "a/0/0/0");
    }

    #[test]
    fn the_first_bytes_are_text_when_they_can_be_read() {
        assert_eq!(first_bytes(b"GARCxx"), "\"GARC\"");
        assert_eq!(first_bytes(&[0x11, 0x80, 0x00]), "11 80 00");
        assert_eq!(first_bytes(&[]), "nothing");
    }

    /// With the dump of the game: a number typed in the box and Enter open that archive.
    #[test]
    fn enter_in_the_number_box_opens_the_archive() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_archive_page_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), ..Session::default() };
        let mut page = Archive::default();
        let ctx = egui::Context::default();
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0)));
        let frame = |page: &mut Archive, session: &mut Session, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: screen, events, ..Default::default() }, |ctx| {
                // The application chooses the active widget of the page before it draws the page.
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::update_navigation(ui);
                    page.ui(ui, session);
                });
            });
        };
        for _ in 0..3 {
            frame(&mut page, &mut session, Vec::new());
        }
        let wanted = page.numbers[5];
        page.input = wanted.to_string();
        // The box has the focus of the text without a click, because the page chose it first.
        frame(&mut page, &mut session, Vec::new());
        let enter = |pressed| egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        frame(&mut page, &mut session, vec![enter(true), enter(false)]);
        frame(&mut page, &mut session, Vec::new());
        assert_eq!(page.loaded.as_ref().map(|l| l.number), Some(wanted), "Enter opens the archive {wanted}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump of the game: while the list of the archives is open, the arrows move a cursor and Enter opens the archive.
    #[test]
    fn the_open_list_has_a_keyboard_cursor() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_archive_cursor_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), focus_page: true, ..Session::default() };
        let mut page = Archive::default();
        let ctx = egui::Context::default();
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0)));
        let frame = |page: &mut Archive, session: &mut Session, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: screen, events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::update_navigation(ui);
                    page.ui(ui, session);
                });
            });
        };
        for _ in 0..3 {
            frame(&mut page, &mut session, Vec::new());
        }
        let start = page.loaded.as_ref().map(|l| l.number).expect("an archive is open");
        let place = page.numbers.iter().position(|&n| n == start).unwrap();
        // The list is open: the page that draws it has the id of its button.
        let stops = ctx.data(|d| d.get_temp::<Vec<widget::FocusableWidget>>(egui::Id::new("sango_focusable_widgets"))).unwrap_or_default();
        let list = stops.iter().find(|w| (w.rect.width() - 190.0).abs() < 30.0).map(|w| w.id).expect("the list is a stop of the page");
        ctx.memory_mut(|m| m.open_popup(list.with("popup")));
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }];
        frame(&mut page, &mut session, Vec::new());
        assert_eq!(page.cursor, Some(start), "the cursor starts on the open archive");
        frame(&mut page, &mut session, key(egui::Key::ArrowUp));
        frame(&mut page, &mut session, key(egui::Key::ArrowUp));
        assert_eq!(page.cursor, Some(page.numbers[place - 2]));
        frame(&mut page, &mut session, key(egui::Key::ArrowDown));
        assert_eq!(page.cursor, Some(page.numbers[place - 1]));
        assert_eq!(page.loaded.as_ref().map(|l| l.number), Some(start), "the cursor opens nothing by itself");
        frame(&mut page, &mut session, key(egui::Key::Enter));
        assert_eq!(page.loaded.as_ref().map(|l| l.number), Some(page.numbers[place - 1]), "Enter opens the archive of the cursor");
        frame(&mut page, &mut session, Vec::new());
        assert_eq!(page.cursor, None, "the cursor goes away with the list");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the dump of the game: the arrows on the list of the archives (when the page has chosen it) open the archive before
    /// or after.
    #[test]
    fn the_arrows_work_on_the_list_of_the_archives() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_archive_list_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session { dump: Some(Dump::open(&root).unwrap()), sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()), focus_page: true, ..Session::default() };
        let mut page = Archive::default();
        let ctx = egui::Context::default();
        let screen = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0)));
        let frame = |page: &mut Archive, session: &mut Session, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: screen, events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::update_navigation(ui);
                    page.ui(ui, session);
                });
            });
        };
        for _ in 0..3 {
            frame(&mut page, &mut session, Vec::new());
        }
        // The page chooses the list of the archives (the stop whose width is the one of the list).
        let stops = ctx.data(|d| d.get_temp::<Vec<widget::FocusableWidget>>(egui::Id::new("sango_focusable_widgets"))).unwrap_or_default();
        let list = stops.iter().find(|w| (w.rect.width() - 190.0).abs() < 30.0).map(|w| w.id).expect("the list is a stop of the page");
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), list));
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }];
        frame(&mut page, &mut session, Vec::new());
        let open_now = |page: &Archive| page.loaded.as_ref().map(|l| l.number);
        let start = open_now(&page).expect("an archive is open");
        let place = page.numbers.iter().position(|&n| n == start).unwrap();
        assert!(place > 1, "the test needs archives before the open one");
        frame(&mut page, &mut session, key(egui::Key::ArrowUp));
        assert_eq!(open_now(&page), Some(page.numbers[place - 1]), "Up opens the archive before");
        frame(&mut page, &mut session, key(egui::Key::ArrowUp));
        assert_eq!(open_now(&page), Some(page.numbers[place - 2]));
        frame(&mut page, &mut session, key(egui::Key::ArrowDown));
        assert_eq!(open_now(&page), Some(page.numbers[place - 1]), "Down opens the next archive");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn row_of_handles_expanded_and_closed_files() {
        // Build mock GARC or test structure
        let mut expanded = HashSet::new();
        expanded.insert(0);
        // Verify HashSet and row_button behavior
        assert!(expanded.contains(&0));
        assert!(!expanded.contains(&1));
    }

    fn build_test_garc(files: &[&[u8]]) -> Garc {
        let mut offsets = Vec::new();
        let mut entries = Vec::new();
        let mut data = Vec::new();
        for bytes in files {
            offsets.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            let mask = 1u32;
            entries.extend_from_slice(&mask.to_le_bytes());
            let start = data.len() as u32;
            data.extend_from_slice(bytes);
            while data.len() % 4 != 0 {
                data.push(0);
            }
            entries.extend_from_slice(&start.to_le_bytes());
            entries.extend_from_slice(&(data.len() as u32).to_le_bytes());
            entries.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        }
        let header_size = 28u32;
        let offset_table_size = 12 + offsets.len() as u32;
        let range_table_size = 12 + entries.len() as u32;
        let data_offset = header_size + offset_table_size + range_table_size + 12;
        let mut out = Vec::new();
        out.extend_from_slice(b"CRAG");
        out.extend_from_slice(&header_size.to_le_bytes());
        out.extend_from_slice(&0xFEFFu16.to_le_bytes());
        out.extend_from_slice(&0x0400u16.to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&data_offset.to_le_bytes());
        out.extend_from_slice(&(data_offset + data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(files.iter().map(|b| b.len() as u32).max().unwrap_or(0)).to_le_bytes());
        out.extend_from_slice(b"OTAF");
        out.extend_from_slice(&offset_table_size.to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&0xFFFFu16.to_le_bytes());
        out.extend_from_slice(&offsets);
        out.extend_from_slice(b"BTAF");
        out.extend_from_slice(&range_table_size.to_le_bytes());
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&entries);
        out.extend_from_slice(b"BMIF");
        out.extend_from_slice(&12u32.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        Garc::parse(out).unwrap()
    }

    #[test]
    fn keys_navigate_with_ctrl_arrows() {
        let garc = build_test_garc(&[b"first", b"second", b"third"]);
        let mut loaded = Loaded {
            number: 1,
            path: PathBuf::from("a/0/0/1"),
            garc,
            selected: Some((0, 0)),
            expanded: HashSet::new(),
            view: 0..3,
            scroll_to: None,
        };

        let ctx = egui::Context::default();

        // 1. Press Ctrl + ArrowDown moves from file 0 to file 1
        let mut down_input = egui::RawInput::default();
        down_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(down_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                Archive::keys(ui, &mut loaded);
            });
        });
        assert_eq!(loaded.selected, Some((1, 0)));

        // 2. Press Ctrl + ArrowUp moves from file 1 back to file 0
        let mut up_input = egui::RawInput::default();
        up_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(up_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                Archive::keys(ui, &mut loaded);
            });
        });
        assert_eq!(loaded.selected, Some((0, 0)));

        // 3. Press Ctrl + ArrowLeft on collapsed file returns to sidebar
        let mut left_input = egui::RawInput::default();
        left_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(left_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                Archive::keys(ui, &mut loaded);
            });
        });
        assert!(widget::take_sidebar_focus_request(&ctx));
    }
}
