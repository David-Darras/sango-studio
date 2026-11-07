use eframe::egui::{self, Color32, FontId, Rounding, Stroke, TextStyle};

/// The colours of the application. The background is white.
pub mod color {
    use super::Color32;

    pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    /// Side bars and panels.
    pub const SURFACE: Color32 = Color32::from_rgb(246, 247, 252);
    /// A button at rest, a field.
    pub const SURFACE_STRONG: Color32 = Color32::from_rgb(237, 240, 248);
    pub const BORDER: Color32 = Color32::from_rgb(225, 229, 240);
    pub const TEXT: Color32 = Color32::from_rgb(28, 32, 48);
    pub const MUTED: Color32 = Color32::from_rgb(110, 118, 140);
    /// The main colour: indigo.
    pub const ACCENT: Color32 = Color32::from_rgb(79, 70, 229);
    pub const ACCENT_SOFT: Color32 = Color32::from_rgb(238, 236, 254);
    pub const TEAL: Color32 = Color32::from_rgb(20, 184, 166);
    pub const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
    pub const ROSE: Color32 = Color32::from_rgb(244, 63, 94);
    pub const GREEN: Color32 = Color32::from_rgb(34, 197, 94);
}

/// Gives the window the white theme.
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = color::WHITE;
    v.window_fill = color::WHITE;
    v.extreme_bg_color = color::WHITE;
    v.faint_bg_color = color::SURFACE;
    v.override_text_color = Some(color::TEXT);
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(10.0);
    v.window_stroke = Stroke::new(1.0_f32, color::BORDER);
    v.window_shadow = egui::epaint::Shadow { offset: egui::vec2(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(28) };
    v.popup_shadow = v.window_shadow;
    v.selection.bg_fill = color::ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0_f32, color::ACCENT);
    v.hyperlink_color = color::ACCENT;

    let rounding = Rounding::same(8.0);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.rounding = rounding;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, color::BORDER);
    v.widgets.inactive.weak_bg_fill = color::SURFACE_STRONG;
    v.widgets.inactive.bg_fill = color::SURFACE_STRONG;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = color::ACCENT_SOFT;
    v.widgets.hovered.bg_fill = color::ACCENT_SOFT;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, color::ACCENT);
    v.widgets.active.weak_bg_fill = color::ACCENT;
    v.widgets.active.bg_fill = color::ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, color::WHITE);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(16.0, 7.0);
    style.spacing.interact_size.y = 34.0;
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style.spacing.scroll.bar_width = 8.0;
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(24.0));
    style.text_styles.insert(TextStyle::Body, FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(15.0));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(12.5));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.5));
    ctx.set_style(style);
}
