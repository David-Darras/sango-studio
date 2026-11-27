//! The viewer on the page, with the keys only.

use super::*;
use sango_core::Dump;

/// A page that has only the viewer, with the choreography of a move. The keys drive it as in the window.
struct Page {
    viewer: StageViewer,
    session: Session,
    choreography: MoveChoreography,
    file: usize,
    ctx: egui::Context,
    time: f64,
}

impl Page {
    fn new(move_id: usize) -> Option<Page> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        let session = Session { dump: Some(Dump::open(&root).unwrap()), focus_page: true, ..Session::default() };
        let file = FILE_OF_NO_MOVE + move_id;
        let garc = Garc::open(&session.dump.as_ref().unwrap().archive(34)).unwrap();
        let choreography = MoveChoreography::parse(&garc.first_file_decompressed(file).unwrap()).unwrap();
        let mut page = Page { viewer: StageViewer::default(), session, choreography, file, ctx: egui::Context::default(), time: 0.0 };
        for _ in 0..3 {
            page.frame(None);
        }
        Some(page)
    }

    fn frame(&mut self, key: Option<(egui::Key, egui::Modifiers)>) {
        let events = key.map(|(key, modifiers)| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
        let modifiers = key.map_or_else(Default::default, |(_, modifiers)| modifiers);
        self.time += 0.1;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1200.0))), time: Some(self.time), events: events.into_iter().collect(), modifiers, ..Default::default() };
        let Page { viewer, session, choreography, file, ctx, .. } = self;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                widget::set_page_focus(ui.ctx(), true);
                widget::update_navigation(ui);
                viewer.ui(ui, session, Shown { file: *file, choreography, selected: None });
            });
        });
    }

    fn press(&mut self, key: egui::Key) {
        self.frame(Some((key, egui::Modifiers::NONE)));
        self.frame(None);
    }
}

#[test]
fn the_keys_play_stop_and_step_through_the_picture() {
    let Some(mut page) = Page::new(53) else { return };
    assert!(page.viewer.run.is_some(), "{:?}", page.viewer.error);
    assert!(page.viewer.texture.is_some(), "the picture is drawn");
    // The picture is the first widget of the viewer: it has the focus.
    let stage = egui::Id::new("choreography_stage");
    assert_eq!(page.ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget"))), Some(stage));
    let first = page.viewer.frame;
    page.press(egui::Key::ArrowRight);
    page.press(egui::Key::ArrowRight);
    assert_eq!(page.viewer.frame, first + 2);
    page.press(egui::Key::ArrowLeft);
    assert_eq!(page.viewer.frame, first + 1);
    page.press(egui::Key::End);
    assert_eq!(page.viewer.frame, page.viewer.run.as_ref().unwrap().last);
    page.press(egui::Key::Home);
    assert_eq!(page.viewer.frame, first);
    page.press(egui::Key::Space);
    assert!(page.viewer.playing);
    for _ in 0..4 {
        page.frame(None);
    }
    assert!(page.viewer.frame > first + 5, "the frames move on with the time: {}", page.viewer.frame);
    page.press(egui::Key::Space);
    assert!(!page.viewer.playing);
    let stopped = page.viewer.frame;
    page.frame(None);
    assert_eq!(page.viewer.frame, stopped);
}

/// What a key of the viewer can change.
fn state(page: &Page) -> ([Choice; 2], Setup, usize, i32, bool, bool, bool) {
    let viewer = &page.viewer;
    (viewer.cast, viewer.setup, viewer.place, viewer.frame, viewer.playing, viewer.repeat, viewer.smooth)
}

/// The widget that has the focus.
fn active(page: &Page) -> Option<egui::Id> {
    page.ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new("sango_active_widget")))
}

/// A page where the keys of a path were pressed: Ctrl and an arrow move the focus.
fn walked(path: &[egui::Key]) -> Page {
    let mut page = Page::new(53).unwrap();
    // The frame in the middle, so that a number can go down.
    page.viewer.go_to(30);
    for &key in path {
        page.frame(Some((key, egui::Modifiers::CTRL)));
        page.frame(None);
    }
    page
}

/// Ctrl and the arrows reach every control of the viewer, and each choice changes with a key on its control. Only keys are
/// sent.
#[test]
fn the_keys_reach_the_choices() {
    let Some(page) = Page::new(53) else { return };
    let controls: Vec<egui::Id> = page.ctx.data(|d| d.get_temp::<Vec<widget::FocusableWidget>>(egui::Id::new("sango_focusable_widgets"))).unwrap().iter().map(|w| w.id).collect();
    // The shortest path of keys to each control.
    let mut paths: Vec<(egui::Id, Vec<egui::Key>)> = vec![(active(&page).unwrap(), Vec::new())];
    drop(page);
    let mut next = 0;
    while next < paths.len() {
        for key in [egui::Key::ArrowDown, egui::Key::ArrowRight, egui::Key::ArrowLeft, egui::Key::ArrowUp] {
            let mut path = paths[next].1.clone();
            path.push(key);
            if let Some(id) = active(&walked(&path)).filter(|id| controls.contains(id) && !paths.iter().any(|(known, _)| known == id)) {
                paths.push((id, path));
            }
        }
        next += 1;
    }
    // The two tabs of the side count for one control: the row opens the tab that the focus comes to.
    let missing: Vec<&egui::Id> = controls.iter().filter(|id| !paths.iter().any(|(known, _)| known == *id) && **id != widget::tab_id("stage_side", 1)).collect();
    assert!(missing.is_empty(), "not reached: {missing:?} of {} controls", controls.len());
    // The names of the changes that a key made.
    let mut changed = std::collections::BTreeSet::new();
    for (_, path) in &paths {
        for key in [egui::Key::ArrowDown, egui::Key::Space] {
            let mut page = walked(path);
            let before = state(&page);
            page.press(key);
            let after = state(&page);
            let names = [
                (after.0[0].species != before.0[0].species, "attacker"),
                (after.0[1].species != before.0[1].species, "target"),
                (after.0[0].shiny != before.0[0].shiny, "shiny"),
                (after.0[0].female != before.0[0].female, "female"),
                (after.0[0].species == before.0[1].species && after.0[1].species == before.0[0].species, "swap"),
                (after.1.targets_user != before.1.targets_user, "aim"),
                (after.2 != before.2, "place"),
                (after.3 == before.3 - 1, "frame down"),
                (after.3 == before.3 + 1, "next frame"),
                (after.3 == 0 && before.3 != 0, "start"),
                (after.4 != before.4, "play"),
                (after.5 != before.5, "repeat"),
                (after.6 != before.6, "smooth"),
            ];
            changed.extend(names.iter().filter(|(done, _)| *done).map(|(_, name)| *name));
            assert!(page.viewer.run.is_some(), "{:?}", page.viewer.error);
        }
    }
    // The tabs of the side open with the arrows.
    if let Some((_, path)) = paths.iter().find(|(id, _)| *id == widget::tab_id("stage_side", 0)) {
        let mut page = walked(path);
        page.press(egui::Key::ArrowRight);
        if page.viewer.setup.attacker_is_opponent {
            changed.insert("side");
        }
    }
    for name in ["attacker", "target", "shiny", "female", "swap", "side", "aim", "place", "frame down", "next frame", "start", "play", "repeat", "smooth"] {
        assert!(changed.contains(name), "no key changes {name}: {changed:?}");
    }
}

#[test]
fn a_step_out_of_order_is_found() {
    let Some(mut page) = Page::new(53) else { return };
    assert_eq!(never_played(&page.choreography), None);
    let mut late = page.choreography.steps[0].clone();
    (late.start, late.end) = (0, 0);
    page.choreography.steps.push(late);
    assert_eq!(never_played(&page.choreography), Some(page.choreography.steps.len() - 1));
    page.frame(None);
    page.viewer.go_to(10);
    page.frame(None);
    assert!(page.viewer.run.is_some());
}

#[test]
fn a_move_on_its_user_is_known() {
    let Some(page) = Page::new(53) else { return };
    // Swords Dance is a move of its user, Flamethrower is not.
    assert!(aims_at_user(&page.session, FILE_OF_NO_MOVE + 14));
    assert!(!aims_at_user(&page.session, FILE_OF_NO_MOVE + 53));
}
