//! Sango Studio: one window, one page for each module.

use eframe::egui;
use sango_ui::theme::color;
use sango_ui::{widget, Module, Session};

struct App {
    modules: Vec<Box<dyn Module>>,
    current: usize,
    session: Session,
    /// The frames since the start: the window is maximized during the first ones.
    frames: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        sango_ui::theme::apply(&cc.egui_ctx);
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(sango_home::Home::default()),
            Box::new(sango_pokemon::Pokemon),
            Box::new(sango_item::Item),
            Box::new(sango_move::Move),
            Box::new(sango_ability::Ability),
            Box::new(sango_type::Type),
        ];
        // SANGO_PAGE=2 opens the third page at once (to test a page without clicking).
        let current = std::env::var("SANGO_PAGE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&i| i < modules.len()).unwrap_or(0);
        App { modules, current, session: Session::restore(), frames: 0 }
    }

    fn side_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none().fill(color::SURFACE).inner_margin(egui::Margin::same(14.0)).stroke(egui::Stroke::new(1.0_f32, color::BORDER));
        egui::SidePanel::left("side_bar").exact_width(210.0).resizable(false).frame(frame).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Sango Studio").strong().size(22.0).color(color::ACCENT));
            ui.label(widget::hint("Created By ZettaD"));
            ui.add_space(14.0);
            for (i, module) in self.modules.iter().enumerate() {
                if widget::side_button(ui, module.icon(), module.title(), i == self.current).clicked() {
                    self.current = i;
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(widget::hint(&format!("v{}", sango_core::VERSION)));
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The option of the window alone leaves it at its first width: the window is made normal, then
        // maximized again, once it is on the screen.
        self.frames += 1;
        match self.frames {
            3 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false)),
            6 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
            _ => {}
        }
        if self.frames < 10 {
            ctx.request_repaint();
        }
        self.side_bar(ctx);
        let frame = egui::Frame::none().fill(color::WHITE).inner_margin(egui::Margin::same(18.0));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if let Some(module) = self.modules.get_mut(self.current) {
                module.ui(ui, &mut self.session);
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_min_inner_size([720.0, 480.0]).with_title(sango_core::NAME),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(sango_core::NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
