//! The Leveling rate page: graphs of the experience that each level needs, with points that the user moves,
//! and the Pokémon that use each rate.
//!
//! The experience tables come from the archive 194 of the romfs. A table that the user changes goes on the
//! SD card as `pokemon/experience/<rate>.bin` (same layout as the file of the romfs). The Leveling rate of each
//! species comes from its data (the SD card first, then the romfs).
//!
//! The graph has two views: the total experience of each level (like the graph of Bulbapedia), and the
//! experience from a level to the next one. The six Leveling rates are drawn together, each in its own colour.
//! The tables can be exported to a CSV file and imported from one: see the module `csv`.

mod csv;

use sango_core::experience::{self, Curve, ExperienceTable, MAX_LEVEL};
use sango_core::{Garc, SdCard};
use sango_species::{SpeciesTable, LEVELING_RATE};
use sango_ui::theme::color;
use sango_ui::{egui, widget, Module, Session};
use std::ops::RangeInclusive;
use std::path::PathBuf;

/// After this many seconds without a frame, the page was hidden: the species data is read again.
const STALE_SECONDS: f64 = 0.5;

/// The height of the graph, in points.
const GRAPH_HEIGHT: f32 = 460.0;

/// The number of Leveling rates that are real: the others are copies of the first one.
const REAL_RATES: usize = 6;

/// The room, in points, that the graph keeps around the plot: left, top, right, bottom.
const MARGINS: [f32; 4] = [76.0, 34.0, 16.0, 30.0];

/// The most experience that a table can have at the level 100 when the user changes a step.
const TOTAL_CAP: u32 = 9_999_999;

/// The width that the page keeps for the list of species when it is beside the graph.
const LIST_CARD_WIDTH: f32 = SpeciesTable::WIDTH + 40.0;

/// The colour of each of the six Leveling rates, in the order of their numbers.
const RATE_COLORS: [egui::Color32; REAL_RATES] = [
    egui::Color32::from_rgb(59, 130, 246),
    egui::Color32::from_rgb(239, 68, 68),
    egui::Color32::from_rgb(168, 85, 247),
    egui::Color32::from_rgb(34, 160, 80),
    egui::Color32::from_rgb(245, 158, 11),
    egui::Color32::from_rgb(20, 170, 160),
];

/// What the graph shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    /// The total experience of each level.
    Total,
    /// The experience to gain to go from each level to the next one.
    Gain,
}

/// The views, with the name of their button.
const VIEWS: [(&str, View); 2] = [("Experience by level", View::Total), ("Experience per level", View::Gain)];

impl View {
    /// The levels that have a value, on the horizontal axis.
    fn levels(self) -> RangeInclusive<usize> {
        match self {
            View::Total => 1..=MAX_LEVEL,
            View::Gain => 1..=MAX_LEVEL - 1,
        }
    }

    /// The levels whose point the user can move.
    fn editable(self) -> RangeInclusive<usize> {
        match self {
            View::Total => 2..=MAX_LEVEL,
            View::Gain => 1..=MAX_LEVEL - 1,
        }
    }

    /// The value of a level on the vertical axis.
    fn value(self, curve: &Curve, level: usize) -> Option<u32> {
        match self {
            View::Total => curve.experience(level),
            View::Gain => curve.between(level, level + 1),
        }
    }

    /// The text of the vertical axis.
    fn title(self) -> &'static str {
        match self {
            View::Total => "Experience",
            View::Gain => "Experience to the next level",
        }
    }

    /// The name of a level on the graph.
    fn describe(self, level: usize) -> String {
        match self {
            View::Total => format!("Level {level}"),
            View::Gain => format!("Level {level} to {}", level + 1),
        }
    }
}

/// A level inside a range of levels. An empty range gives the level as it is.
fn clamp_level(level: usize, range: &RangeInclusive<usize>) -> usize {
    if range.is_empty() {
        level
    } else {
        level.clamp(*range.start(), *range.end())
    }
}

pub struct LevelingRate {
    /// The dump that the data comes from.
    root: Option<PathBuf>,
    /// The tables that the user edits.
    table: Option<ExperienceTable>,
    /// The tables as they were read or saved: to know what changed.
    saved: Option<ExperienceTable>,
    /// The tables of the romfs: what Reset gives back.
    romfs: Option<ExperienceTable>,
    /// The rates that were read from the SD card.
    from_card: Vec<usize>,
    /// The Leveling rate of each species, from the species 1 on.
    rates: Vec<Option<u8>>,
    /// The list of species of the right side.
    species_table: SpeciesTable,
    /// The time of the last frame, to know when the page comes back after another page.
    last_seen: f64,
    /// What went wrong when the data was read.
    error: Option<String>,
    /// What happened at the last click: true for a success.
    status: Option<(bool, String)>,
    /// The chosen Leveling rate.
    rate: usize,
    /// The chosen view: a place in `VIEWS`.
    view: usize,
    /// The level of the chosen point.
    selected: usize,
    /// The level of the point that the user drags.
    dragging: Option<usize>,
    /// The vertical axis while a point is dragged: it stays still, so the point follows the pointer.
    frozen_axis: Option<(u32, f64)>,
    /// The levels of the box "Between two levels". `from` is also the start of the view "From a level".
    from: u32,
    to: u32,
    /// The plot of the last frame, in screen points: the tests use it to aim at a point.
    plot: egui::Rect,
}

impl Default for LevelingRate {
    fn default() -> Self {
        LevelingRate {
            root: None,
            table: None,
            saved: None,
            romfs: None,
            from_card: Vec::new(),
            rates: Vec::new(),
            species_table: SpeciesTable::default(),
            last_seen: f64::NEG_INFINITY,
            error: None,
            status: None,
            rate: 0,
            view: 0,
            selected: 50,
            dragging: None,
            frozen_axis: None,
            from: 1,
            to: 100,
            plot: egui::Rect::NOTHING,
        }
    }
}

/// The numbers of the species that use a Leveling rate, in the order of their numbers.
fn species_of_rate(rates: &[Option<u8>], rate: usize) -> Vec<usize> {
    rates.iter().enumerate().filter(|(_, r)| **r == Some(rate as u8)).map(|(place, _)| place + 1).collect()
}

/// The number with thousands separators: 1059860 gives "1,059,860".
fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let mut text = String::new();
    for (place, digit) in digits.chars().enumerate() {
        if place > 0 && (digits.len() - place) % 3 == 0 {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

/// The top of the vertical axis and the distance between two lines of the grid, for a curve whose highest
/// value is `top`. The top is 10 % above it, on a line, and the axis has at most 8 lines.
fn nice_axis(top: u32) -> (u32, f64) {
    let wanted = (top as f64 * 1.1).max(10.0);
    let unit = 10f64.powf(wanted.log10().floor()) / 10.0;
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].into_iter().map(|factor| factor * unit).find(|step| wanted / step <= 8.0).unwrap_or(10.0 * unit);
    (((wanted / step).ceil() * step).round() as u32, step)
}

/// A value of the vertical axis as short text: "250k", "2.5k", "1.2M".
fn axis_label(value: u32) -> String {
    let trim = |number: f64, digits: usize| {
        let text = format!("{number:.digits$}");
        let text = if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_string() } else { text };
        text
    };
    if value >= 1_000_000 {
        format!("{}M", trim(value as f64 / 1_000_000.0, 2))
    } else if value >= 1000 {
        format!("{}k", trim(value as f64 / 1000.0, 1))
    } else {
        value.to_string()
    }
}

/// The inside of the graph, where the curves are drawn, in a rectangle of the whole graph.
fn plot_rect(rect: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.left() + MARGINS[0], rect.top() + MARGINS[1]),
        egui::pos2(rect.right() - MARGINS[2], rect.bottom() - MARGINS[3]),
    )
}

/// The place on the screen of a point: the level goes from 1 to 100 on the horizontal axis.
fn point_pos(plot: egui::Rect, axis_max: u32, level: usize, value: u32) -> egui::Pos2 {
    let x = plot.left() + (level as f32 - 1.0) / (MAX_LEVEL as f32 - 1.0) * plot.width();
    let y = plot.bottom() - value as f32 / axis_max as f32 * plot.height();
    egui::pos2(x, y)
}

/// The level at a horizontal place of the screen (1 to 100).
fn level_at(plot: egui::Rect, x: f32) -> usize {
    let ratio = ((x - plot.left()) / plot.width()).clamp(0.0, 1.0);
    (ratio * (MAX_LEVEL as f32 - 1.0)).round() as usize + 1
}

/// The value at a vertical place of the screen. A place out of the plot gives 0 or the top of the axis.
fn value_at(plot: egui::Rect, axis_max: u32, y: f32) -> u32 {
    let ratio = ((plot.bottom() - y) / plot.height()).clamp(0.0, 1.0);
    (ratio * axis_max as f32).round() as u32
}

impl LevelingRate {
    /// Reads the experience tables of the romfs and of the SD card. The user's changes are lost.
    fn load_tables(&mut self, session: &Session) {
        let Some(dump) = &session.dump else { return };
        self.error = None;
        self.status = None;
        match Garc::open(&dump.archive(experience::ARCHIVE)).and_then(|garc| ExperienceTable::from_garc(&garc)) {
            Ok(romfs) => {
                let mut table = romfs.clone();
                self.from_card = session.sd.as_ref().map_or_else(Vec::new, |sd| table.overlay_sd(sd));
                self.saved = Some(table.clone());
                self.table = Some(table);
                self.romfs = Some(romfs);
            }
            Err(message) => {
                self.error = Some(message);
                self.table = None;
                self.saved = None;
                self.romfs = None;
            }
        }
    }

    /// Reads the Leveling rate of each species and the data of the list again.
    fn load_species(&mut self, session: &Session) {
        match sango_species::leveling_rates(session) {
            Ok(rates) => self.rates = rates,
            Err(message) => {
                self.rates.clear();
                self.error = Some(message);
            }
        }
        self.species_table.load(session);
    }

    /// True when a table differs from the one that was read or saved.
    fn changed(&self, rate: usize) -> bool {
        match (&self.table, &self.saved) {
            (Some(table), Some(saved)) => table.curve(rate) != saved.curve(rate),
            _ => false,
        }
    }

    /// The chosen view.
    fn current_view(&self) -> View {
        VIEWS.get(self.view).map_or(View::Total, |(_, view)| *view)
    }

    /// The top of the vertical axis and the distance between two lines: the six curves must fit.
    fn axis(&self) -> (u32, f64) {
        if let Some(frozen) = self.frozen_axis {
            return frozen;
        }
        let view = self.current_view();
        let top = self
            .table
            .as_ref()
            .map(|table| {
                (0..REAL_RATES).filter_map(|rate| table.curve(rate)).flat_map(|curve| view.levels().filter_map(move |level| view.value(curve, level))).max().unwrap_or(0)
            })
            .unwrap_or(0);
        nice_axis(top)
    }

    /// Moves the point of a level to the value of a place of the screen.
    ///
    /// In the view "Experience by level", the points beside it move with it, so a point goes where the pointer goes;
    /// with `keep_neighbours` they stay where they are and the point stops at them.
    fn move_point(&mut self, level: usize, y: f32, keep_neighbours: bool, axis: u32) {
        let value = value_at(self.plot, axis, y);
        let view = self.current_view();
        let rate = self.rate;
        let Some(curve) = self.table.as_mut().and_then(|table| table.curves_mut().get_mut(rate)) else { return };
        let changed = match view {
            View::Total => curve.set_experience(level, value, axis, !keep_neighbours),
            View::Gain => curve.set_gain(level, value, TOTAL_CAP),
        };
        if changed {
            self.status = None;
        }
    }

    /// The buttons that choose the Leveling rate.
    fn rate_tabs(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(color::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, color::BORDER))
            .rounding(egui::Rounding::same(12.0))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for (place, name) in LEVELING_RATE.iter().enumerate() {
                        self.rate = tab(ui, ("leveling_rate_tab", place), name, place, self.rate);
                    }
                });
            });
    }

    /// The buttons that choose the view of the graph.
    fn view_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.label(widget::hint("View"));
            for (place, (name, _)) in VIEWS.iter().enumerate() {
                self.view = tab(ui, ("leveling_rate_view", place), name, place, self.view);
            }
        });
    }

    /// The state, and the buttons. Returns what the user asked: (reset, save, export, import).
    fn tool_bar(&mut self, ui: &mut egui::Ui, can_save: bool) -> (bool, bool, bool, bool) {
        let mut asked = (false, false, false, false);
        ui.horizontal_wrapped(|ui| {
            asked.0 = widget::button(ui, "Reset");
            asked.1 = can_save && widget::button_styled(ui, egui::RichText::new("Save").color(color::WHITE), Some(color::ACCENT));
            asked.2 = widget::button(ui, "Export CSV");
            asked.3 = can_save && widget::button(ui, "Import CSV");
            if (0..LEVELING_RATE.len()).any(|rate| self.changed(rate)) {
                widget::badge(ui, "Not saved", color::AMBER);
            } else if self.from_card.contains(&self.rate) {
                widget::badge(ui, "From the SD card", color::TEAL);
            } else {
                widget::badge(ui, "From the romfs", color::GREEN);
            }
            ui.label(widget::hint("Drag a point up or down. Shift: the points beside it stay. Ctrl: draw the curve."));
        });
        if let Some((ok, message)) = &self.status {
            ui.label(egui::RichText::new(message).color(if *ok { color::GREEN } else { color::ROSE }).size(13.5));
        }
        asked
    }

    /// The graph: the levels on the horizontal axis, a value on the vertical axis, one curve for each rate.
    fn graph(&mut self, ui: &mut egui::Ui, width: f32) {
        let view = self.current_view();
        let editable = view.editable();
        let levels = view.levels();
        let (axis, step) = self.axis();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(360.0), GRAPH_HEIGHT), egui::Sense::click_and_drag());
        self.plot = plot_rect(rect);
        let plot = self.plot;
        let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl));
        let inside = |pos: &egui::Pos2| plot.expand(8.0).contains(*pos);

        // What the pointer does: take the point of the column under it, move it, let it go.
        let hovered = if self.dragging.is_none() { response.hover_pos().filter(inside).map(|pos| clamp_level(level_at(plot, pos.x), &levels)) } else { None };
        if response.drag_started() && !editable.is_empty() {
            if let Some(pos) = response.interact_pointer_pos().filter(inside) {
                let level = clamp_level(level_at(plot, pos.x), &editable);
                self.dragging = Some(level);
                self.selected = level;
                self.frozen_axis = Some((axis, step));
            }
        }
        if let (Some(level), Some(pos)) = (self.dragging, response.interact_pointer_pos()) {
            // With Ctrl the pointer draws: each column that it crosses takes its height.
            let target = if ctrl { clamp_level(level_at(plot, pos.x), &editable) } else { level };
            for crossed in level.min(target)..=level.max(target) {
                self.move_point(crossed, pos.y, shift, axis);
            }
            self.dragging = Some(target);
            self.selected = target;
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
            self.dragging = None;
            self.frozen_axis = None;
        }
        if response.clicked() && !editable.is_empty() {
            if let Some(pos) = response.interact_pointer_pos().filter(inside) {
                self.selected = clamp_level(level_at(plot, pos.x), &editable);
            }
        }
        if self.dragging.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if hovered.is_some_and(|level| editable.contains(&level)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::Rounding::same(10.0), color::SURFACE);
        painter.rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0_f32, color::BORDER));
        let grid = egui::Stroke::new(1.0_f32, color::BORDER);
        let label_font = egui::FontId::proportional(11.5);

        // The grid and the numbers of the two axes.
        let mut tick = 0u32;
        while (tick as f64 * step) <= axis as f64 + 0.5 {
            let value = (tick as f64 * step).round() as u32;
            let y = point_pos(plot, axis, 1, value).y;
            painter.line_segment([egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)], grid);
            painter.text(egui::pos2(plot.left() - 8.0, y), egui::Align2::RIGHT_CENTER, axis_label(value), label_font.clone(), color::MUTED);
            tick += 1;
        }
        for level in [1usize, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            let x = point_pos(plot, axis, level, 0).x;
            painter.line_segment([egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())], grid);
            painter.text(egui::pos2(x, plot.bottom() + 6.0), egui::Align2::CENTER_TOP, level.to_string(), label_font.clone(), color::MUTED);
        }
        painter.text(egui::pos2(plot.center().x, rect.bottom() - 1.0), egui::Align2::CENTER_BOTTOM, "Level", label_font.clone(), color::MUTED);
        painter.text(egui::pos2(rect.left() + 6.0, plot.top() - 8.0), egui::Align2::LEFT_BOTTOM, view.title(), label_font.clone(), color::MUTED);

        let (Some(table), Some(saved)) = (&self.table, &self.saved) else { return };
        let line = |curve: &Curve| -> Vec<egui::Pos2> {
            levels.clone().filter_map(|level| view.value(curve, level).map(|value| point_pos(plot, axis, level, value))).collect()
        };
        let chosen_color = RATE_COLORS[self.rate.min(REAL_RATES - 1) * usize::from(self.rate < REAL_RATES)];

        // The levels of the box "Between two levels".
        if view == View::Total {
            let (low, high) = (self.from.min(self.to) as usize, self.from.max(self.to) as usize);
            let band = egui::Rect::from_min_max(egui::pos2(point_pos(plot, axis, low, 0).x, plot.top()), egui::pos2(point_pos(plot, axis, high, 0).x, plot.bottom()));
            painter.rect_filled(band, egui::Rounding::ZERO, chosen_color.linear_multiply(0.10));
        }
        // The other rates, thin.
        for other in (0..REAL_RATES).filter(|&other| other != self.rate) {
            if let Some(curve) = table.curve(other) {
                painter.add(egui::Shape::line(line(curve), egui::Stroke::new(1.5_f32, RATE_COLORS[other].linear_multiply(0.75))));
            }
        }
        // The saved table of this rate, thin and grey, while the user changes it.
        if self.changed(self.rate) {
            if let Some(curve) = saved.curve(self.rate) {
                painter.add(egui::Shape::line(line(curve), egui::Stroke::new(1.5_f32, color::MUTED)));
            }
        }
        // The table of this rate, with a point for each level that can move.
        let Some(curve) = table.curve(self.rate) else { return };
        painter.add(egui::Shape::line(line(curve), egui::Stroke::new(3.0_f32, chosen_color)));
        for level in editable.clone() {
            let Some(value) = view.value(curve, level) else { continue };
            let pos = point_pos(plot, axis, level, value);
            let marked = level == self.selected || Some(level) == hovered || Some(level) == self.dragging;
            let (radius, fill) = if marked { (6.0, chosen_color) } else { (3.0, color::WHITE) };
            painter.circle_filled(pos, radius, fill);
            painter.circle_stroke(pos, radius, egui::Stroke::new(1.5_f32, chosen_color));
        }
        // The column under the pointer.
        let shown = self.dragging.or(hovered);
        if let Some(level) = shown {
            let x = point_pos(plot, axis, level, 0).x;
            painter.line_segment([egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())], egui::Stroke::new(1.0_f32, color::MUTED));
        }

        // The legend: a click on a name chooses the rate.
        let mut x = plot.left();
        for (rate, name) in LEVELING_RATE.iter().enumerate().take(REAL_RATES) {
            let text_rect = painter.text(egui::pos2(x + 14.0, rect.top() + 12.0), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(12.5), if rate == self.rate { color::TEXT } else { color::MUTED });
            let swatch = egui::Rect::from_center_size(egui::pos2(x + 5.0, text_rect.center().y), egui::vec2(10.0, 10.0));
            painter.rect_filled(swatch, egui::Rounding::same(2.0), RATE_COLORS[rate]);
            let hit = egui::Rect::from_min_max(egui::pos2(x - 2.0, rect.top() + 2.0), egui::pos2(text_rect.right() + 4.0, rect.top() + 24.0));
            let legend = ui.interact(hit, egui::Id::new(("leveling_rate_legend", rate)), egui::Sense::click());
            if legend.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if legend.clicked() {
                self.rate = rate;
                // The tab of the rate becomes the active widget, or it would give the old rate back at the next frame.
                widget::set_active_id(ui, egui::Id::new(("leveling_rate_tab", rate)));
            }
            x = text_rect.right() + 22.0;
        }

        // The numbers of the six rates at the column under the pointer.
        if let Some(level) = shown {
            let rows: Vec<(usize, String)> = (0..REAL_RATES)
                .map(|rate| (rate, table.curve(rate).and_then(|curve| view.value(curve, level)).map_or("-".to_string(), thousands)))
                .collect();
            let title = view.describe(level);
            response.on_hover_ui_at_pointer(|ui| {
                ui.label(egui::RichText::new(title).strong());
                for (rate, value) in rows {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.colored_label(RATE_COLORS[rate], "■");
                        ui.label(LEVELING_RATE[rate]);
                        ui.label(egui::RichText::new(value).strong());
                    });
                }
            });
        }
    }

    /// The numbers of the chosen point, and the box "Between two levels".
    fn numbers(&mut self, ui: &mut egui::Ui) {
        let view = self.current_view();
        let rate = self.rate;
        let Some(curve) = self.table.as_mut().and_then(|table| table.curves_mut().get_mut(rate)) else { return };
        let editable = view.editable();
        if !editable.is_empty() {
            self.selected = clamp_level(self.selected, &editable);
            widget::card(ui, |ui| {
                ui.label(widget::title("Point"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(widget::hint("Level"));
                    let mut level = self.selected as u32;
                    if widget::spin(ui, &mut level, *editable.start() as f64, *editable.end() as f64) {
                        self.selected = level as usize;
                    }
                    let mut changed = false;
                    match view {
                        View::Total => {
                            ui.label(widget::hint("Experience"));
                            let mut value = curve.experience(self.selected).unwrap_or(0);
                            if widget::spin(ui, &mut value, 0.0, TOTAL_CAP as f64) {
                                changed = curve.set_experience(self.selected, value, TOTAL_CAP, true);
                            }
                            let next = if self.selected < MAX_LEVEL { curve.between(self.selected, self.selected + 1) } else { None };
                            ui.label(widget::hint(&format!("To the next level: {}", next.map_or("-".to_string(), thousands))));
                        }
                        _ => {
                            ui.label(widget::hint("Experience to the next level"));
                            let mut value = curve.between(self.selected, self.selected + 1).unwrap_or(0);
                            if widget::spin(ui, &mut value, 0.0, TOTAL_CAP as f64) {
                                changed = curve.set_gain(self.selected, value, TOTAL_CAP);
                            }
                            ui.label(widget::hint(&format!("Total at level {}: {}", self.selected + 1, thousands(curve.experience(self.selected + 1).unwrap_or(0)))));
                        }
                    }
                    if changed {
                        self.status = None;
                    }
                });
            });
            ui.add_space(8.0);
        }
        widget::card(ui, |ui| {
            ui.label(widget::title("Between two levels"));
            ui.horizontal_wrapped(|ui| {
                ui.label(widget::hint("From level"));
                widget::spin(ui, &mut self.from, 1.0, MAX_LEVEL as f64);
                ui.label(widget::hint("to level"));
                widget::spin(ui, &mut self.to, 1.0, MAX_LEVEL as f64);
            });
            match curve.between(self.from as usize, self.to as usize) {
                Some(gain) => ui.label(egui::RichText::new(format!("{} experience", thousands(gain))).strong().size(18.0)),
                None => ui.label(widget::hint("The first level must not be above the second one.")),
            };
        });
    }

    /// The Pokémon that use the chosen rate. A click opens the Species page on that Pokémon.
    fn species_list(&mut self, ui: &mut egui::Ui) {
        let list = species_of_rate(&self.rates, self.rate);
        widget::card(ui, |ui| {
            ui.set_width(SpeciesTable::WIDTH);
            ui.label(widget::title(&format!("Pokémon with this rate ({})", list.len())));
            ui.label(widget::hint("Click a Pokémon to open it in Species."));
            ui.add_space(4.0);
            if list.is_empty() {
                ui.label(widget::hint("No Pokémon uses this rate."));
            } else if let Some(number) = self.species_table.show(ui, "leveling_rate_species", &list, 560.0) {
                sango_species::open_species(ui.ctx(), number);
            }
        });
    }

    /// Takes the table of the romfs back for the chosen rate. Save writes it.
    fn reset(&mut self) {
        let original = self.romfs.as_ref().and_then(|romfs| romfs.curve(self.rate)).cloned();
        if let (Some(table), Some(curve)) = (self.table.as_mut(), original) {
            table.set_curve(self.rate, curve);
            self.status = Some((true, "The table comes from the romfs again. Save to write it on the SD card.".into()));
        }
    }

    /// Writes the six real tables to a CSV file that the user chooses.
    fn export_csv(&mut self) {
        let Some(table) = &self.table else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("leveling_rates.csv").save_file() else { return };
        self.status = Some(match std::fs::write(&path, csv::export(table)) {
            Ok(()) => (true, format!("Exported 100 levels to {}.", path.display())),
            Err(error) => (false, format!("{}: {error}", path.display())),
        });
    }

    /// Reads a CSV file that the user chooses, checks it, and writes the changed tables on the SD card.
    fn import_csv(&mut self, session: &Session) {
        let Some(sd) = &session.sd else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() else { return };
        let result = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display())).and_then(|text| self.import_text(&text, sd));
        self.status = Some(match result {
            Ok(0) => (true, "The file matches the current tables: nothing to write.".to_string()),
            Ok(files) => (true, format!("Imported: {files} file(s) written on the SD card. The game loads them again by itself.")),
            Err(message) => (false, message),
        });
    }

    /// Checks the text of a CSV file, then writes each table that it changes on the SD card.
    /// Nothing is written when the text has a mistake. Returns the number of written files.
    fn import_text(&mut self, text: &str, sd: &SdCard) -> Result<usize, String> {
        let tables = csv::parse(text)?;
        let (Some(table), Some(saved)) = (self.table.as_mut(), self.saved.as_mut()) else { return Err("The tables are not read.".into()) };
        let mut written = 0;
        for (rate, curve) in tables {
            if table.curve(rate) == Some(&curve) && saved.curve(rate) == Some(&curve) {
                continue;
            }
            curve.save(sd, rate)?;
            table.set_curve(rate, curve.clone());
            saved.set_curve(rate, curve);
            if !self.from_card.contains(&rate) {
                self.from_card.push(rate);
            }
            written += 1;
        }
        Ok(written)
    }

    /// Writes the tables that changed on the SD card.
    fn save(&mut self, session: &Session) {
        let (Some(table), Some(sd)) = (&self.table, &session.sd) else { return };
        let mut written = Vec::new();
        for rate in (0..LEVELING_RATE.len()).filter(|&rate| self.changed(rate)) {
            let Some(curve) = table.curve(rate) else { continue };
            if let Err(message) = curve.save(sd, rate) {
                self.status = Some((false, message));
                return;
            }
            written.push(rate);
        }
        if let (Some(saved), Some(table)) = (self.saved.as_mut(), self.table.as_ref()) {
            for &rate in &written {
                if let Some(curve) = table.curve(rate) {
                    saved.set_curve(rate, curve.clone());
                }
                if !self.from_card.contains(&rate) {
                    self.from_card.push(rate);
                }
            }
        }
        self.status = Some((true, format!("Saved {} file(s) in pokemon/experience. The game loads them again by itself.", written.len())));
    }
}

/// A tab button that the arrow keys reach, like the tabs of the Species page. Returns the chosen place.
fn tab(ui: &mut egui::Ui, id: (&str, usize), text: &str, place: usize, chosen: usize) -> usize {
    let tab_id = egui::Id::new(id);
    let mut chosen = chosen;
    let active = widget::is_active(ui, tab_id);
    if active && chosen != place {
        chosen = place;
    }
    let response = widget::tab_button(ui, text, chosen == place, active && widget::page_has_focus(ui));
    widget::register_focusable(ui, tab_id, response.rect);
    if response.clicked() {
        chosen = place;
        widget::set_active_id(ui, tab_id);
    }
    chosen
}

impl Module for LevelingRate {
    fn title(&self) -> &str {
        "Leveling Rate"
    }

    fn icon(&self) -> &str {
        "L"
    }

    fn ui(&mut self, ui: &mut egui::Ui, session: &mut Session) {
        widget::set_page_focus(ui.ctx(), session.focus_page);
        widget::update_navigation(ui);
        ui.add_space(8.0);
        ui.heading("Leveling Rate");
        ui.add_space(8.0);
        let (None, Some(dump)) = (session.missing(), session.dump.clone()) else {
            if let Some(message) = session.missing() {
                widget::missing_card(ui, message);
            }
            return;
        };
        // The tables are read for a new dump. The species are read again when the page comes back after
        // another page: they may have changed there. The user's changes to a table stay.
        let now = ui.ctx().input(|i| i.time);
        let new_dump = self.root.as_deref() != Some(dump.root());
        if new_dump {
            self.root = Some(dump.root().to_path_buf());
            self.load_tables(session);
        }
        if new_dump || now - self.last_seen > STALE_SECONDS {
            self.load_species(session);
        }
        self.last_seen = now;
        if let Some(message) = &self.error {
            ui.label(egui::RichText::new(message).color(color::ROSE));
        }
        if self.table.is_none() {
            return;
        }
        let (reset, save, export, import) = self.tool_bar(ui, session.sd.is_some());
        ui.add_space(8.0);
        self.rate_tabs(ui);
        if reset {
            self.reset();
        }
        if save {
            self.save(session);
        }
        if export {
            self.export_csv();
        }
        if import {
            self.import_csv(session);
        }
        ui.add_space(8.0);
        let wide = ui.available_width() >= 760.0 + LIST_CARD_WIDTH;
        egui::ScrollArea::vertical().id_salt("leveling_rate_page").drag_to_scroll(false).auto_shrink([false, false]).show(ui, |ui| {
            if wide {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        let width = ui.available_width() - LIST_CARD_WIDTH - 10.0;
                        ui.set_width(width);
                        self.view_tabs(ui);
                        ui.add_space(6.0);
                        self.graph(ui, width);
                        ui.add_space(8.0);
                        self.numbers(ui);
                    });
                    ui.add_space(10.0);
                    ui.vertical(|ui| self.species_list(ui));
                });
            } else {
                let width = ui.available_width();
                self.view_tabs(ui);
                ui.add_space(6.0);
                self.graph(ui, width);
                ui.add_space(8.0);
                self.numbers(ui);
                ui.add_space(10.0);
                self.species_list(ui);
            }
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn species_of_a_rate_are_found_by_number() {
        let rates = [Some(3), Some(0), None, Some(3)];
        assert_eq!(species_of_rate(&rates, 3), [1, 4]);
        assert_eq!(species_of_rate(&rates, 0), [2]);
        assert!(species_of_rate(&rates, 5).is_empty());
    }

    #[test]
    fn numbers_have_thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_059_860), "1,059,860");
    }

    #[test]
    fn the_axis_fits_the_curves_on_a_line() {
        assert_eq!(nice_axis(600_000), (700_000, 100_000.0));
        assert_eq!(nice_axis(1_640_000), (2_000_000, 250_000.0));
        assert_eq!(nice_axis(1_000_000).0, 1_200_000);
        // A small axis, for the experience per level.
        assert_eq!(nice_axis(50_000), (60_000, 10_000.0));
        assert_eq!(nice_axis(0).0, 10);
        // Every top is above the data and has at most 8 lines.
        for top in [1u32, 9, 37, 480, 29_701, 1_059_860, 4_000_000] {
            let (max, step) = nice_axis(top);
            assert!(max as f64 >= top as f64 * 1.1 - 1.0, "{top}");
            assert!(max as f64 / step <= 8.0 + 1e-9, "{top}");
        }
        assert_eq!(axis_label(250_000), "250k");
        assert_eq!(axis_label(2_500), "2.5k");
        assert_eq!(axis_label(1_200_000), "1.2M");
        assert_eq!(axis_label(2_000_000), "2M");
        assert_eq!(axis_label(900), "900");
    }

    #[test]
    fn a_place_of_the_screen_gives_back_the_point() {
        let plot = egui::Rect::from_min_size(egui::pos2(100.0, 20.0), egui::vec2(990.0, 400.0));
        for level in [1usize, 2, 50, 99, 100] {
            let pos = point_pos(plot, 1_000_000, level, 250_000);
            assert_eq!(level_at(plot, pos.x), level);
            assert!((value_at(plot, 1_000_000, pos.y) as i64 - 250_000).abs() <= 1);
        }
        // Out of the plot, the values are cut.
        assert_eq!(value_at(plot, 1_000_000, 10_000.0), 0);
        assert_eq!(value_at(plot, 1_000_000, -10_000.0), 1_000_000);
        assert_eq!(level_at(plot, -50.0), 1);
    }

    #[test]
    fn each_view_has_its_levels_and_values() {
        let bytes: Vec<u8> = (0..=MAX_LEVEL as u32).flat_map(|n| (n * n).to_le_bytes()).collect();
        let curve = Curve::from_bytes(&bytes).unwrap();
        assert_eq!(View::Total.levels(), 1..=100);
        assert_eq!(View::Gain.levels(), 1..=99);
        assert_eq!(View::Total.value(&curve, 10), Some(100));
        assert_eq!(View::Gain.value(&curve, 10), Some(21));
        assert_eq!(View::Gain.describe(10), "Level 10 to 11");
        assert_eq!(clamp_level(500, &View::Total.editable()), 100);
        assert_eq!(clamp_level(0, &View::Gain.editable()), 1);
    }

    /// The list of Pokémon, drawn in the layout of the page (a column beside the graph), shows more than 10 rows.
    #[test]
    fn the_list_of_pokemon_shows_many_rows_beside_the_graph() {
        let mut page = LevelingRate::default();
        page.rates = vec![Some(0); 721];
        let ctx = egui::Context::default();
        let mut height = 0.0;
        for _ in 0..3 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1900.0, 1000.0)));
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("test_page").show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.add_space(700.0);
                            height = ui.vertical(|ui| page.species_list(ui)).response.rect.height();
                        });
                    });
                });
            });
            // The first frame is the one that counts, but the later ones must not shrink it.
            assert!(height >= 10.0 * 34.0 + 60.0, "the list is only {height} points high");
        }
    }

    /// With the dump of the game: the page reads its data, points are dragged with the mouse in the two
    /// editable views, the change is saved on the SD card and read again.
    #[test]
    fn the_page_reads_the_dump_and_points_are_dragged() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let folder = std::env::temp_dir().join(format!("sango_leveling_rate_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut session = Session {
            dump: Some(sango_core::Dump::open(&root).unwrap()),
            sd: Some(sango_core::SdCard::open(folder.to_str().unwrap()).unwrap()),
            ..Session::default()
        };
        session.focus_page = true;
        let mut page = LevelingRate::default();
        page.load_tables(&session);
        page.load_species(&session);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.rates.len(), 721);
        let counted: usize = (0..8).map(|rate| species_of_rate(&page.rates, rate).len()).sum();
        assert_eq!(counted, 721);
        // Bulbapedia: Bulbasaur is Medium Slow (3), Pikachu Medium Fast (0), Magikarp Slow (5), Blissey Fast (4).
        let rate_of = |number: usize| page.rates[number - 1];
        assert_eq!((rate_of(1), rate_of(25), rate_of(129), rate_of(242)), (Some(3), Some(0), Some(5), Some(4)));

        let ctx = egui::Context::default();
        let frame = |page: &mut LevelingRate, session: &mut Session, events: Vec<egui::Event>| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1900.0, 1000.0)));
            input.events = events;
            let _ = ctx.run(input, |ctx| {
                // The tab of the chosen rate is the active widget: another active tab would give its rate back at each frame.
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), egui::Id::new(("leveling_rate_tab", page.rate))));
                egui::CentralPanel::default().show(ctx, |ui| page.ui(ui, session));
            });
        };
        // The whole page draws for every rate and every view.
        for rate in 0..8 {
            for view in 0..VIEWS.len() {
                (page.rate, page.view) = (rate, view);
                frame(&mut page, &mut session, vec![]);
            }
        }
        // The mouse takes a point and moves it up by `lift` points, in three frames.
        let drag = |page: &mut LevelingRate, session: &mut Session, level: usize, lift: f32| {
            frame(page, session, vec![]);
            let (axis, _) = page.axis();
            let view = page.current_view();
            let curve = page.table.as_ref().unwrap().curve(page.rate).unwrap();
            let start = point_pos(page.plot, axis, level, view.value(curve, level).unwrap());
            let press = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            let target = egui::pos2(start.x, start.y - lift);
            frame(page, session, vec![egui::Event::PointerMoved(start)]);
            frame(page, session, vec![press(start, true)]);
            frame(page, session, vec![egui::Event::PointerMoved(egui::pos2(start.x, start.y - lift / 2.0))]);
            frame(page, session, vec![egui::Event::PointerMoved(target)]);
            frame(page, session, vec![press(target, false)]);
            assert_eq!(page.dragging, None);
        };

        // View "Experience by level": the point of the level 50 goes up, and the points above it follow if needed.
        (page.rate, page.view) = (0, 0);
        let before = page.table.as_ref().unwrap().curve(0).unwrap().clone();
        drag(&mut page, &mut session, 50, 20.0);
        let after = page.table.as_ref().unwrap().curve(0).unwrap().clone();
        assert!(after.experience(50) > before.experience(50), "the point did not move");
        assert_eq!(after.experience(49), before.experience(49));
        assert!(page.changed(0));
        assert_eq!(page.selected, 50);
        // A point can go a long way up: the points above it follow, the table stays valid.
        drag(&mut page, &mut session, 30, 300.0);
        let high = page.table.as_ref().unwrap().curve(0).unwrap().clone();
        assert!(high.experience(30).unwrap() > 300_000);
        assert!(high.experience(60) >= high.experience(30));
        assert!(Curve::from_bytes(&high.to_bytes()).is_ok());

        // View "Experience per level": the step from the level 40 grows, the levels above move with it.
        (page.rate, page.view) = (1, 1);
        let before = page.table.as_ref().unwrap().curve(1).unwrap().clone();
        drag(&mut page, &mut session, 40, 25.0);
        let after = page.table.as_ref().unwrap().curve(1).unwrap().clone();
        assert!(after.between(40, 41) > before.between(40, 41));
        assert_eq!(after.between(41, 42), before.between(41, 42));
        assert_eq!(after.experience(40), before.experience(40));
        assert!(after.experience(100) > before.experience(100));

        // Save writes the files of the two rates that changed, and a new page reads them.
        page.save(&session);
        assert!(!page.changed(0) && !page.changed(1));
        let written = |rate: usize| folder.join("pokemon").join("experience").join(format!("{rate}.bin"));
        assert!(written(0).exists() && written(1).exists() && !written(2).exists());
        let mut again = LevelingRate::default();
        again.load_tables(&session);
        assert_eq!(again.table.as_ref().unwrap().curve(0), Some(&high));
        assert_eq!(again.from_card, [0, 1]);

        // A CSV file goes out and comes back through the page: a changed column is written on the SD card.
        let text = csv::export(page.table.as_ref().unwrap());
        let sd = session.sd.as_ref().unwrap();
        assert_eq!(page.import_text(&text, sd), Ok(0));
        let edited = text.replacen("100,", "100,0,", 1);
        assert!(page.import_text(&edited, sd).is_err(), "a line with too many fields is refused");
        let lowered: String = text.lines().map(|line| if line.starts_with("100,") { let mut f: Vec<String> = line.split(',').map(String::from).collect(); f[6] = "1250001".into(); format!("{}
", f.join(",")) } else { format!("{line}
") }).collect();
        assert_eq!(page.import_text(&lowered, sd), Ok(1));
        assert_eq!(page.table.as_ref().unwrap().curve(5).unwrap().experience(100), Some(1_250_001));
        assert!(!page.changed(5));
        assert!(written(5).exists());

        // Reset gives back the romfs.
        page.rate = 0;
        page.reset();
        assert_eq!(page.table.as_ref().unwrap().curve(0), Some(&before_of_rate_0(&page)));
        assert!(page.changed(0));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The table of the rate 0 as the romfs has it.
    fn before_of_rate_0(page: &LevelingRate) -> Curve {
        page.romfs.as_ref().unwrap().curve(0).unwrap().clone()
    }
}
