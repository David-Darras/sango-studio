//! The tests of the whole page. They need the dump (`SANGO_DUMP`). One of them also needs a save of the game (`SANGO_SAVE`); it
//! works on a copy.

use crate::context::GameData;
use crate::data::{self, Place, Trainer};
use crate::format::{self, Save};
use crate::pokemon::{self, Pokemon};
use crate::{controls, live, page, SavePage};
use sango_core::{Dump, SdCard};
use sango_ui::{egui, widget, Module, Session};
use std::path::PathBuf;

fn dump() -> Option<Dump> {
    Dump::open(&std::env::var("SANGO_DUMP").ok()?).ok()
}

/// A folder for one test. The copies of the save and the settings go next to it, never in the folders of the user.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("sango_save_page_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("card")).unwrap();
    std::env::set_var("SANGO_CONFIG_DIR", std::env::temp_dir().join(format!("sango_save_config_{}", std::process::id())));
    folder
}

struct Window {
    ctx: egui::Context,
    session: Session,
    page: SavePage,
}

impl Window {
    fn new(folder: &std::path::Path, save: &[u8], dump: Dump) -> Window {
        let file = folder.join("main");
        std::fs::write(&file, save).unwrap();
        let sd = SdCard::open(&folder.join("card").to_string_lossy()).unwrap();
        let session = Session { dump: Some(dump), sd: Some(sd), save_file: Some(file), focus_page: true, ..Session::default() };
        sango_species::menus::set_source(&session);
        Window { ctx: egui::Context::default(), session, page: SavePage::default() }
    }

    /// One frame with the input, and one more so that the page settles.
    fn frame(&mut self, input: egui::RawInput) {
        for input in [input, Default::default()] {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1600.0))), ..input };
            let (page, session) = (&mut self.page, &mut self.session);
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    widget::set_page_focus(ctx, true);
                    widget::update_navigation(ui);
                    page.ui(ui, session);
                });
            });
        }
    }

    fn key(&mut self, key: egui::Key) {
        self.frame(egui::RawInput { events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], ..Default::default() });
    }

    fn focus(&mut self, id: egui::Id) {
        self.ctx.data_mut(|d| d.insert_temp(egui::Id::new("sango_active_widget"), id));
        self.frame(Default::default());
    }

    fn save(&self) -> &Save {
        self.page.edited.as_ref().unwrap()
    }

    /// Draws every tab, and checks that drawing changes nothing.
    fn look_at_every_tab(&mut self) {
        let before = self.save().clone();
        for tab in 0..page::TABS.len() {
            self.page.tab = tab;
            self.focus(widget::tab_id("save", tab));
            self.frame(Default::default());
            assert_eq!(self.save().changed_blocks(&before), Vec::<usize>::new(), "the tab {} changes the save by itself", page::TABS[tab]);
        }
    }
}

#[test]
fn the_keys_reach_the_tabs_and_the_places() {
    let Some(dump) = dump() else { return };
    let folder = folder("keys");
    let mut window = Window::new(&folder, &format::empty_for_test().to_bytes(), dump);
    window.frame(Default::default());
    assert!(!window.page.unsaved());
    window.focus(widget::tab_id("save", 0));
    window.key(egui::Key::ArrowRight);
    assert_eq!(window.page.tab, 1, "the arrows open the tabs");
    window.focus(controls::grid_id("party"));
    window.key(egui::Key::ArrowRight);
    window.key(egui::Key::ArrowRight);
    assert_eq!(window.page.party_slot, 2);
    window.key(egui::Key::ArrowLeft);
    assert_eq!(window.page.party_slot, 1);
    // A new Pokémon with the keys, then undone.
    window.frame(sango_ui::history::test_keys(egui::Key::N));
    let party = data::party(window.save());
    assert_eq!(party.len(), 1);
    assert_eq!((party[0].species(), party[0].level()), (25, 5));
    assert!(party[0].stat(0) > 0 && party[0].hp() == party[0].stat(0) && party[0].move_id(0) != 0 && !party[0].is_shiny());
    window.frame(sango_ui::history::test_keys(egui::Key::Z));
    assert_eq!(data::party_count(window.save()), 0, "undone");
    // In a box, down goes one row.
    window.page.tab = 2;
    window.focus(widget::tab_id("save", 2));
    window.focus(controls::grid_id("box"));
    window.key(egui::Key::ArrowDown);
    window.key(egui::Key::ArrowRight);
    assert_eq!(window.page.box_slot, 7);
    window.frame(sango_ui::history::test_keys(egui::Key::N));
    assert_eq!(data::boxed(window.save(), 0, 7).species(), 25);
    window.look_at_every_tab();
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn save_writes_the_file_and_asks_the_game() {
    let Some(dump) = dump() else { return };
    let folder = folder("write");
    let mut window = Window::new(&folder, &format::empty_for_test().to_bytes(), dump);
    window.frame(Default::default());
    let edited = window.page.edited.as_mut().unwrap();
    let mut trainer = Trainer::read(edited);
    trainer.money = 5000;
    trainer.write(edited);
    data::move_player(edited, Place { map: 7, x: 9.0, y: 0.0, z: 9.0, facing: 1 });
    window.frame(Default::default());
    assert!(window.page.unsaved());
    window.frame(sango_ui::history::test_keys(egui::Key::S));
    assert!(!window.page.unsaved(), "{:?}", window.page.status);
    let written = Save::read(&folder.join("main")).unwrap();
    assert_eq!(Trainer::read(&written).money, 5000);
    assert!(data::moves_at_continue(&written));
    let changes = live::Changes::parse(&std::fs::read(folder.join("card").join(live::FILE)).unwrap()).unwrap();
    assert_eq!(changes.place.map(|p| p.map), Some(7));
    assert_eq!(changes.blocks.iter().map(|b| b.0).collect::<Vec<_>>(), [format::MISC]);
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn the_maps_have_a_name_and_an_arrival_point() {
    let Some(dump) = dump() else { return };
    let data = GameData::open(&dump, None).unwrap();
    assert!(data.maps.len() > 500);
    let town = data.map(224).unwrap();
    assert_eq!((town.name.as_str(), town.start.x, town.start.z, town.start.map), ("Littleroot Town", 153.0, 243.0, 224));
}

#[test]
fn a_save_of_the_game_is_read_and_written_as_it_is() {
    let (Some(dump), Ok(path)) = (dump(), std::env::var("SANGO_SAVE")) else { return };
    let bytes = std::fs::read(path).unwrap();
    let save = Save::parse(bytes.clone()).unwrap();
    assert_eq!(save.to_bytes(), bytes, "the checksums are the ones of the game");
    let trainer = Trainer::read(&save);
    println!("{trainer:?}\nparty of {}, at {:?}", data::party_count(&save), data::place(&save));
    let folder = folder("real");
    let mut window = Window::new(&folder, &bytes, dump.clone());
    let mut game = GameData::open(&dump, None).unwrap();
    // Each Pokémon of the file is encrypted again as the game did it.
    let block = save.block(format::PARTY);
    for slot in 0..data::party_count(&save) {
        let record = &block[slot * pokemon::PARTY..(slot + 1) * pokemon::PARTY];
        let found = Pokemon::read(record);
        assert!(found.sound, "the checksum of the Pokémon {slot}");
        assert_eq!(found.to_party().as_slice(), record);
        let mut again = found;
        game.calculate(&mut again);
        println!("{} lv {} {:?} {}", found.nickname(), found.level(), (0..6).map(|s| found.stat(s)).collect::<Vec<_>>(), game.species_name(found.species(), found.language()));
        assert_eq!(again.stats, found.stats, "the statistics of the Pokémon {slot}");
    }
    let boxes = save.block(format::BOXES);
    let mut filled = 0;
    for (place, record) in boxes.chunks_exact(pokemon::STORED).enumerate() {
        let found = Pokemon::read(record);
        assert!(found.sound, "the checksum of the place {place}");
        assert_eq!(found.to_stored().as_slice(), record, "the place {place}");
        filled += usize::from(!found.is_empty());
    }
    println!("{filled} Pokémon in the boxes, the first box is {:?}", data::box_name(&save, 0));
    for pocket in 0..data::POCKETS.len() {
        println!("{}: {} items", data::POCKETS[pocket].0, data::pocket(&save, pocket).len());
    }
    window.frame(Default::default());
    window.look_at_every_tab();
    assert!(!window.page.unsaved());
    let _ = std::fs::remove_dir_all(folder);
}
