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
    /// The next page that is drawn out of sight to load its data (see `warm_up`).
    warmed: usize,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        sango_ui::theme::apply(&cc.egui_ctx);
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(sango_home::Home::default()),
            Box::new(sango_archive::Archive::default()),
            Box::new(sango_species::Species::default()),
            Box::new(sango_leveling_rate::LevelingRate::default()),
            Box::new(sango_item::Item::default()),
            Box::new(sango_move::Move::default()),
            Box::new(sango_ability::Ability::default()),
            Box::new(sango_type::Type::default()),
            Box::new(sango_code::Code::default()),
        ];
        // SANGO_PAGE=2 opens the third page at once (to test a page without clicking).
        let current = std::env::var("SANGO_PAGE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&i| i < modules.len()).unwrap_or(0);
        App { modules, current, session: Session::restore(), frames: 0, side_bar_focus: true, warmed: 0 }
    }

    fn side_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::none().fill(color::SURFACE).inner_margin(egui::Margin::same(14.0)).stroke(egui::Stroke::new(1.0_f32, color::BORDER));
        egui::SidePanel::left("side_bar").exact_width(SIDE_BAR_WIDTH).resizable(false).frame(frame).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Sango Studio").strong().size(22.0).color(color::ACCENT));
            ui.label(widget::hint("Created By ZettaD"));
            ui.add_space(14.0);
            for (i, module) in self.modules.iter().enumerate() {
                let selected = i == self.current;
                let response = widget::side_button(ui, module.icon(), module.title(), selected);
                if response.clicked() {
                    self.current = i;
                    self.side_bar_focus = true;
                    ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
                    ctx.request_repaint();
                }
                if selected && self.side_bar_focus {
                    widget::glow(ui, response.rect, 10.0);
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(widget::hint(&format!("v{}", sango_core::VERSION)));
            });
        });
    }

    /// Draws one page that is not open, out of sight, at each frame once the window is ready. Each page reads its files and
    /// makes its textures the first time that it is drawn, which takes a few seconds for the big ones: this way the wait
    /// is at the start and the click on a page is quick.
    fn warm_up(&mut self, ctx: &egui::Context) {
        if self.frames < 12 || self.session.missing().is_some() {
            return;
        }
        if self.warmed == self.current {
            self.warmed += 1;
        }
        let place = self.warmed;
        let Some(module) = self.modules.get_mut(place) else { return };
        self.warmed += 1;
        let session = &mut self.session;
        let (was, size) = (session.focus_page, egui::vec2(1300.0, 800.0));
        session.focus_page = false;
        widget::isolated(ctx, || {
            widget::set_page_focus(ctx, false);
            egui::Area::new(egui::Id::new(("warm_up", place))).order(egui::Order::Background).fixed_pos(egui::pos2(-20000.0, -20000.0)).constrain(false).interactable(false).show(ctx, |ui| {
                ui.set_invisible();
                ui.set_min_size(size);
                ui.set_max_size(size);
                module.ui(ui, session);
            });
        });
        session.focus_page = was;
        ctx.request_repaint();
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
                let step: isize = ctx.input_mut(|i| {
                    let mut s = 0isize;
                    for mods in [egui::Modifiers::NONE, egui::Modifiers::CTRL, egui::Modifiers::COMMAND] {
                        if i.consume_key(mods, egui::Key::ArrowDown) {
                            s += 1;
                        }
                        if i.consume_key(mods, egui::Key::ArrowUp) {
                            s -= 1;
                        }
                    }
                    s
                });
                if step != 0 {
                    self.current = ((self.current as isize + step).rem_euclid(self.modules.len() as isize)) as usize;
                    ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
                    ctx.request_repaint();
                }
                let enter_page = ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                        || i.consume_key(egui::Modifiers::CTRL, egui::Key::ArrowRight)
                        || i.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowRight)
                });
                if enter_page {
                    self.side_bar_focus = false;
                    ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
                    ctx.request_repaint();
                }
            }
        } else {
            let key_return = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::F6)
            });
            if key_return || widget::take_sidebar_focus_request(ctx) {
                self.side_bar_focus = true;
                ctx.request_repaint();
            }
        }

        if let Some(target) = ctx.data_mut(|d| d.remove_temp::<usize>(egui::Id::new("sango_switch_module"))) {
            self.current = target;
            self.side_bar_focus = false;
            ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
            ctx.request_repaint();
        }

        self.session.focus_page = !self.side_bar_focus;
        widget::set_page_focus(ctx, self.session.focus_page);
        self.side_bar(ctx);
        let frame = egui::Frame::none().fill(color::WHITE).inner_margin(egui::Margin::same(18.0));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            widget::update_navigation(ui);
            if let Some(module) = self.modules.get_mut(self.current) {
                module.ui(ui, &mut self.session);
            }
        });

        if widget::take_sidebar_focus_request(ctx) {
            self.side_bar_focus = true;
            self.session.focus_page = false;
            widget::set_page_focus(ctx, false);
            ctx.request_repaint();
        }

        // A page asked for another page by its title.
        if let Some(title) = widget::take_open_page(ctx) {
            if let Some(target) = self.modules.iter().position(|module| module.title() == title) {
                self.current = target;
                self.side_bar_focus = false;
                ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
                ctx.request_repaint();
            }
        }

        if let Some(target) = ctx.data_mut(|d| d.remove_temp::<usize>(egui::Id::new("sango_switch_module"))) {
            self.current = target;
            self.side_bar_focus = false;
            ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new("sango_active_widget")));
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ctx(ctx);
        self.warm_up(ctx);
    }
}

/// The logo, as the icon of the window.
fn window_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/logo_256.png")).ok()?.to_rgba8();
    Some(egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]).with_min_inner_size([720.0, 480.0]).with_title(sango_core::NAME);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(sango_core::NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod test {
    use super::*;

    /// The pages that are not open are drawn out of sight one after the other, once, and the keyboard focus of the open page
    /// stays as it was.
    #[test]
    fn the_other_pages_are_loaded_out_of_sight() {
        let mut app = App {
            modules: vec![Box::new(sango_home::Home::default()), Box::new(sango_species::Species::default()), Box::new(sango_item::Item::default())],
            current: 0,
            session: Session::restore(),
            frames: 20,
            warmed: 0,
            side_bar_focus: false,
        };
        let ctx = egui::Context::default();
        for _ in 0..6 {
            let _ = ctx.run(Default::default(), |ctx| {
                app.update_ctx(ctx);
                app.warm_up(ctx);
            });
        }
        if app.session.missing().is_none() {
            assert_eq!(app.warmed, 3, "the two other pages were drawn, one at each frame, and then no more");
        }
        assert_eq!(app.current, 0);
    }

    #[test]
    fn the_logo_is_the_icon_of_the_window() {
        let icon = window_icon().expect("the logo is a PNG picture");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn clicking_in_species_page() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 2,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
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

    #[test]
    fn sidebar_wraps_around_up_and_down() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 0,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: true,
        };

        let ctx = egui::Context::default();

        // 1. ArrowUp on Home (0) wraps to Type (7)
        let mut up_input = egui::RawInput::default();
        up_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(up_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert_eq!(app.current, 7);

        // 2. ArrowDown on Type (7) wraps to Home (0)
        let mut down_input = egui::RawInput::default();
        down_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert_eq!(app.current, 0);
    }

    #[test]
    fn entering_home_page_with_arrow_right_focuses_browse_button() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 0,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: true,
        };

        let ctx = egui::Context::default();

        // Frame 1: render Home so buttons register
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        assert!(app.side_bar_focus);

        // Frame 2: press ArrowRight to enter page
        let mut right_input = egui::RawInput::default();
        right_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(right_input, |ctx| {
            app.update_ctx(ctx);
        });

        assert!(!app.side_bar_focus);
        assert!(app.session.focus_page);
        let active = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert!(active.is_some());

        // Frame 3: normal idle frame
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        assert!(!app.side_bar_focus);
        assert!(app.session.focus_page);
        let active3 = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert_eq!(active, active3);
    }

    #[test]
    fn arrow_down_navigates_between_home_buttons() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 0,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: false,
        };
        let ctx = egui::Context::default();

        // Frame 1: register widgets into next
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        // Frame 2: transfer widgets to cur and set initial active widget
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        let first_active = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert!(first_active.is_some());

        // Frame 3: press ArrowDown to navigate to the "Use this folder" button
        let mut down_input = egui::RawInput::default();
        down_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(down_input, |ctx| {
            app.update_ctx(ctx);
        });
        let second_active = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert!(second_active.is_some());
        assert_ne!(first_active, second_active);
    }

    #[test]
    fn archive_missing_card_has_focusable_go_to_home_button() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 1, // Archive module
            session: Session::default(), // No dump loaded -> missing
            frames: 10,
            warmed: 0,
            side_bar_focus: false,
        };
        let ctx = egui::Context::default();

        // Frame 1: render archive page with missing_card
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });

        // Frame 2: verify active widget is set on the "Go to Home" button
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        let active = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert!(active.is_some());

        // Pressing Enter activates "Go to Home" and switches module to Home (0)
        let mut enter_input = egui::RawInput::default();
        enter_input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(enter_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert_eq!(app.current, 0);
    }

    #[test]
    fn exiting_species_page_with_arrow_left_returns_to_sidebar() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 2,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: false,
        };

        let ctx = egui::Context::default();

        // Frame 1: render species page so widgets are registered
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });
        assert!(!app.side_bar_focus);

        // Frame 2: press ArrowLeft
        let mut left_input = egui::RawInput::default();
        left_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(left_input, |ctx| {
            app.update_ctx(ctx);
        });

        assert!(app.side_bar_focus);
        assert!(!app.session.focus_page);
    }

    #[test]
    fn sidebar_navigates_with_ctrl_arrows_and_enters_page() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 0,
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: true,
        };

        let ctx = egui::Context::default();

        // 1. Ctrl + ArrowDown moves from 0 (Home) to 1 (Archive)
        let mut down_input = egui::RawInput::default();
        down_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(down_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert_eq!(app.current, 1);
        assert!(app.side_bar_focus);

        // 2. Ctrl + ArrowUp moves from 1 (Archive) to 0 (Home)
        let mut up_input = egui::RawInput::default();
        up_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(up_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert_eq!(app.current, 0);
        assert!(app.side_bar_focus);

        // 3. Ctrl + ArrowRight enters the page
        let mut enter_input = egui::RawInput::default();
        enter_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(enter_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert!(!app.side_bar_focus);
        assert!(app.session.focus_page);
    }

    #[test]
    fn ctrl_arrow_navigates_between_species_tabs() {
        let mut app = App {
            modules: vec![
                Box::new(sango_home::Home::default()),
                Box::new(sango_archive::Archive::default()),
                Box::new(sango_species::Species::default()),
                Box::new(sango_leveling_rate::LevelingRate::default()),
                Box::new(sango_item::Item::default()),
                Box::new(sango_move::Move::default()),
                Box::new(sango_ability::Ability::default()),
                Box::new(sango_type::Type::default()),
            ],
            current: 2, // Species module
            session: Session::restore(),
            frames: 10,
            warmed: 0,
            side_bar_focus: false,
        };

        let ctx = egui::Context::default();

        // Frame 1: initial render to register tab widgets
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ctx(ctx);
        });

        // Frame 2: transfer widgets and set active to tab 0 ("General")
        let tab0_id = egui::Id::new("species_tab_0");
        let tab1_id = egui::Id::new("species_tab_1");
        let _ = ctx.run(Default::default(), |ctx| {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), tab0_id));
            app.update_ctx(ctx);
        });

        // Frame 3: press Ctrl + ArrowRight to move to tab 1 ("Learnset")
        let mut right_input = egui::RawInput::default();
        right_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(right_input, |ctx| {
            app.update_ctx(ctx);
        });
        let active = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert_eq!(active, Some(tab1_id));

        // Frame 4: press Ctrl + ArrowLeft to move back to tab 0 ("General")
        let mut left_input = egui::RawInput::default();
        left_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(left_input, |ctx| {
            app.update_ctx(ctx);
        });
        let active_back = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")));
        assert_eq!(active_back, Some(tab0_id));

        // Frame 5: press Ctrl + ArrowLeft from leftmost tab 0 returns to sidebar
        let mut left_sidebar_input = egui::RawInput::default();
        left_sidebar_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(left_sidebar_input, |ctx| {
            app.update_ctx(ctx);
        });
        assert!(app.side_bar_focus);
        assert!(!app.session.focus_page);
    }
}
