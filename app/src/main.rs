//! Sango Studio: one window, one page for each module.

use eframe::egui;
use sango_ui::theme::color;
use sango_ui::{widget, Module, Session};

/// The width of the side bar, in points.
const SIDE_BAR_WIDTH: f32 = 210.0;

struct App {
    modules: Vec<Box<dyn Module>>,
    current: usize,
    session: Session,
    /// The frames since the start: the window is maximized during the first ones.
    frames: u32,
    /// True when the last click was in the side bar: then the up and down arrows change the page.
    side_bar_focus: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        sango_ui::theme::apply(&cc.egui_ctx);
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(sango_home::Home::default()),
            Box::new(sango_archive::Archive::default()),
            Box::new(sango_species::Species::default()),
            Box::new(sango_item::Item),
            Box::new(sango_move::Move),
            Box::new(sango_ability::Ability),
            Box::new(sango_type::Type),
        ];
        // SANGO_PAGE=2 opens the third page at once (to test a page without clicking).
        let current = std::env::var("SANGO_PAGE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&i| i < modules.len()).unwrap_or(0);
        App { modules, current, session: Session::restore(), frames: 0, side_bar_focus: true }
    }

    fn side_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none().fill(color::SURFACE).inner_margin(egui::Margin::same(14.0)).stroke(egui::Stroke::new(1.0_f32, color::BORDER));
        egui::SidePanel::left("side_bar").exact_width(SIDE_BAR_WIDTH).resizable(false).frame(frame).show(ctx, |ui| {
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

    fn update_ctx(&mut self, ctx: &egui::Context) {
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
        // Focus owner: mouse click or keyboard switching.
        if let Some(position) = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None }) {
            self.side_bar_focus = position.x < SIDE_BAR_WIDTH;
        }

        if self.side_bar_focus {
            if !ctx.wants_keyboard_input() {
                let step: isize = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) as isize - i.key_pressed(egui::Key::ArrowUp) as isize);
                if step != 0 {
                    self.current = (self.current as isize + step).clamp(0, self.modules.len() as isize - 1) as usize;
                }
                let enter_page = ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                });
                if enter_page {
                    self.side_bar_focus = false;
                }
            }
        } else {
            let is_editing = widget::is_editing_ctx(ctx);
            let return_sidebar = ctx.input_mut(|i| {
                (!is_editing && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::F6)
            });
            if return_sidebar {
                self.side_bar_focus = true;
            }
        }

        self.session.focus_page = !self.side_bar_focus;
        widget::set_page_focus(ctx, self.session.focus_page);
        self.side_bar(ctx);
        let frame = egui::Frame::none().fill(color::WHITE).inner_margin(egui::Margin::same(18.0));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if let Some(module) = self.modules.get_mut(self.current) {
                module.ui(ui, &mut self.session);
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ctx(ctx);
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn clicking_in_species_page() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_item::Item),
                Box::new(sango_move::Move),
                Box::new(sango_ability::Ability),
                Box::new(sango_type::Type),
            ],
            current: 2,
            session: Session::restore(),
            frames: 10,
            side_bar_focus: false,
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });

        for x in [250.0, 300.0, 400.0, 500.0, 600.0] {
            for y in [50.0, 100.0, 150.0, 200.0, 300.0, 400.0] {
                let mut input = egui::RawInput::default();
                input.events.push(egui::Event::PointerButton {
                    pos: egui::pos2(x, y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
                let _ = ctx.run(input, |ctx| {
                    app.update_ctx(ctx);
                });
            }
        }
    }
}
