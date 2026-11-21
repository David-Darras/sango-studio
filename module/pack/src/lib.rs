//! Species packs: the species, the forms and the models that the game does not have, in files that users install with the
//! studio and share with each other.
//!
//! The studio owns the data. The plugin only reads `species/table.bin` (see [`table`]). The design is in
//! `docs/species-packs.md`.
//!
//! - [`pack`]: the format of a pack (a `.sangopack` ZIP file) and its checks.
//! - [`library`]: the installed packs, the registry of numbers, and the files written on the SD card.
//! - [`table`]: the binary file of the plugin.
//! - [`inc`]: reads the data files that the plugin had before the packs, and makes the first packs from them.
//! - [`item_icon_pack`]: puts the item icons of a folder in a pack.

pub mod inc;
pub mod item_icon_pack;
pub mod library;
pub mod pack;
pub mod table;

pub use library::Library;
pub use pack::{ModelRef, Pack, PackForm, PackSpecies, Ref};
pub use table::Table;

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::SdCard;
    use crate::table::MegaCondition;

    fn species(name: &str) -> PackSpecies {
        PackSpecies { name: name.into(), model: ModelRef::Native { native: 3 }, data: pack::to_hex(&[7u8; pack::DATA_SIZE]), moves: vec![pack::Move { id: 33, level: 1 }], evolutions: Vec::new() }
    }

    fn pack(id: &str, names: &[&str]) -> Pack {
        let mut species: Vec<PackSpecies> = names.iter().map(|n| species(n)).collect();
        if names.len() > 1 {
            species[0].evolutions = vec![pack::Evolution { method: 4, arg: 16, species: Ref::key(names[1]) }];
        }
        Pack { id: id.into(), name: id.into(), version: "1".into(), species, ..Pack::default() }
    }

    /// The icons of the packs go on the SD card with the table for the plugin, through a zip file, and leave with the pack.
    #[test]
    fn the_icons_of_the_packs_are_written_and_removed() {
        let mut picture = image::RgbaImage::new(40, 30);
        for pixel in picture.pixels_mut().take(50) {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        let mut png = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        let mut a = pack("alpha", &["Alpha", "Beta"]);
        a.icons = vec![pack::PackIcon { species: Ref::key("Beta"), form: 0, file: "beta".into(), game_file: None, label: String::new() }];
        a.icon_files.insert("beta".into(), png.clone());
        let mut g = Pack { id: "game".into(), name: "game".into(), version: "1".into(), ..Pack::default() };
        g.icons = vec![pack::PackIcon { species: Ref::Game(1), form: 0, file: "one".into(), game_file: Some(1), label: String::new() }];
        g.icon_files.insert("one".into(), png);
        let folder = std::env::temp_dir().join(format!("sango_pack_icons_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let (fa, fg) = (folder.join("a.sangopack"), folder.join("g.sangopack"));
        a.save(&fa).unwrap();
        g.save(&fg).unwrap();
        // The pictures come back from the file.
        assert_eq!(Pack::open(&fa).unwrap().icon_files.len(), 1);
        std::fs::create_dir_all(folder.join("sd")).unwrap();
        let sd = SdCard::open(folder.join("sd").to_str().unwrap()).unwrap();
        let mut library = Library::default();
        library.install(Pack::open(&fa).unwrap()).unwrap();
        library.install(Pack::open(&fg).unwrap()).unwrap();
        assert_eq!(library.write_icons(&sd).unwrap(), 2);
        let beta = library::FIRST_PACK_ICON + 1;
        assert!(sd.read(&format!("species/icon/{beta}.bin")).is_some());
        assert!(sd.read("species/icon/1.bin").is_some());
        let map = sd.read(library::ICON_MAP_PATH).unwrap();
        assert_eq!(&map[..4], b"SICM");
        assert_eq!(u32::from_le_bytes(map[4..8].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes([map[8], map[9]]), table::FIRST_SPECIES + 1);
        assert_eq!(u16::from_le_bytes([map[12], map[13]]), beta);
        // A thing that the user turns off is not written, and comes back when it is turned on.
        library.set_enabled("alpha", "icon:beta", false);
        assert_eq!(library.write_icons(&sd).unwrap(), 1);
        assert!(!library.is_enabled("alpha", "icon:beta"));
        library.set_enabled("alpha", "icon:beta", true);
        assert_eq!(library.write_icons(&sd).unwrap(), 2);
        // Removing a pack removes its files at the next write.
        library.remove("game").unwrap();
        assert_eq!(library.write_icons(&sd).unwrap(), 1);
        assert!(sd.read("species/icon/1.bin").is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The item icons of a pack replace files of the game in `item/icon`, with the list that the plugin reads, and leave with
    /// the pack. A file that the user changed in the Item page and that no pack replaces stays.
    #[test]
    fn the_item_icons_of_the_packs_are_written_and_removed() {
        let picture = image::RgbaImage::from_pixel(30, 30, image::Rgba([0, 132, 255, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut p = Pack { id: "items".into(), name: "items".into(), version: "1".into(), ..Pack::default() };
        p.item_icons = vec![pack::PackItemIcon { game_file: 4, file: "4".into(), label: "Potion".into() }];
        p.item_icon_files.insert("4".into(), png.into_inner());
        let folder = std::env::temp_dir().join(format!("sango_pack_item_icons_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("sd")).unwrap();
        let file = folder.join("items.sangopack");
        p.save(&file).unwrap();
        assert_eq!(Pack::open(&file).unwrap().item_icon_files.len(), 1, "the pictures come back from the file");
        let sd = SdCard::open(folder.join("sd").to_str().unwrap()).unwrap();
        sd.write("item/icon/9.bin", b"own picture").unwrap();
        let mut library = Library::default();
        library.install(Pack::open(&file).unwrap()).unwrap();
        library.write_icons(&sd).unwrap();
        let written = sd.read("item/icon/4.bin").unwrap();
        let clim = sango_core::Clim::parse(&sango_core::lz::decompress(&written).unwrap()).unwrap();
        assert_eq!(&clim.to_rgba().unwrap()[..4], &[0, 132, 255, 255], "the picture is in the file (colours of 5 bits)");
        let index = file_lists_numbers(&sd.read("item/icon/index.bin").unwrap());
        assert_eq!(index, vec![4, 9], "the list of the plugin has both files");
        library.set_enabled("items", "item_icon:4", false);
        assert_eq!(library.write_item_icons(&sd).unwrap(), 0);
        assert!(sd.read("item/icon/4.bin").is_none(), "a picture turned off leaves the SD card");
        library.set_enabled("items", "item_icon:4", true);
        assert_eq!(library.write_item_icons(&sd).unwrap(), 1);
        library.remove("items").unwrap();
        assert_eq!(library.write_item_icons(&sd).unwrap(), 0);
        assert!(sd.read("item/icon/4.bin").is_none());
        assert!(sd.read("item/icon/9.bin").is_some(), "the picture of the Item page stays");
        let _ = std::fs::remove_dir_all(&folder);
    }

    fn file_lists_numbers(index: &[u8]) -> Vec<u32> {
        sango_core::file_lists::parse_index(index).unwrap().into_iter().map(|(number, _)| number).collect()
    }

    /// A pack that is cut in two keeps the numbers: the new pack takes the species over when both are installed together.
    #[test]
    fn species_move_to_a_new_pack_and_keep_their_numbers() {
        let mut library = Library::default();
        library.install(pack("whole", &["Alpha", "Beta", "Gamma"])).unwrap();
        let numbers: Vec<u16> = ["Alpha", "Beta", "Gamma"].iter().map(|n| library.number_of(n).unwrap()).collect();
        // The new pack alone is refused while the old one still has the species.
        let mut refused = library.clone();
        let error = refused.install(pack("part", &["Gamma"])).unwrap_err();
        assert!(error.contains("already installed by the pack 'whole'"), "{error}");
        // The two new packs together: the order of the list does not matter.
        let results = library.install_many(vec![pack("part", &["Gamma"]), pack("whole", &["Alpha", "Beta"])]);
        // And a species that goes from one installed pack to another one, in the wrong order.
        let mut crossed = library.clone();
        let results_crossed = crossed.install_many(vec![pack("part", &["Gamma", "Beta"]), pack("whole", &["Alpha"])]);
        assert!(results_crossed.iter().all(|(_, r)| r.is_ok()), "{results_crossed:?}");
        assert_eq!(crossed.registry.species.iter().find(|s| s.name == "Beta").unwrap().pack, "part");
        assert!(results.iter().all(|(_, r)| r.is_ok()), "{results:?}");
        assert_eq!(["Alpha", "Beta", "Gamma"].iter().map(|n| library.number_of(n).unwrap()).collect::<Vec<_>>(), numbers);
        assert_eq!(library.registry.species.iter().find(|s| s.name == "Gamma").unwrap().pack, "part");
        assert_eq!(library.registry.species.len(), 3);
    }

    #[test]
    fn a_pack_goes_through_a_zip_file_and_is_checked() {
        let original = pack("test", &["Alpha", "Beta"]);
        assert_eq!(Pack::from_zip(&original.to_zip().unwrap()).unwrap(), original);
        assert!(Pack::from_zip(b"not a zip").is_err());
        // The checks.
        let mut bad = original.clone();
        bad.species[1].name = "Alpha".into();
        assert!(bad.check().unwrap_err().contains("two times"));
        let mut bad = original.clone();
        bad.species[0].data = "00".into();
        assert!(bad.check().unwrap_err().contains("80"));
        let mut bad = original.clone();
        bad.species[0].evolutions[0].species = Ref::Game(900);
        assert!(bad.check().unwrap_err().contains("species of the game"));
        let mut bad = original.clone();
        bad.species[0].name = "123".into();
        assert!(bad.check().is_err());
        let mut bad = original;
        bad.id = "has space".into();
        assert!(bad.check().is_err());
    }

    #[test]
    fn numbers_follow_the_order_of_installation_and_stay_after_a_removal() {
        let mut library = Library::default();
        library.install(pack("one", &["Alpha", "Beta"])).unwrap();
        library.install(pack("two", &["Gamma"])).unwrap();
        assert_eq!((library.number_of("Alpha"), library.number_of("Beta"), library.number_of("Gamma")), (Some(724), Some(725), Some(726)));
        // The same name in another pack, and the same pack two times, are refused and change nothing.
        let before = library.registry.clone();
        assert!(library.install(pack("three", &["Beta"])).unwrap_err().contains("already installed by"));
        assert_eq!(library.registry, before);
        // A species that no installed pack has is refused.
        let mut lonely = pack("lonely", &["Omega"]);
        lonely.species[0].evolutions = vec![pack::Evolution { method: 4, arg: 1, species: Ref::key("Nowhere") }];
        assert!(library.install(lonely).unwrap_err().contains("Nowhere"));
        assert_eq!(library.registry, before);
        // The evolution of Alpha goes to the number of Beta.
        let table = library.table().unwrap();
        assert_eq!(table.species[0].evolutions, [(4, 16, 725)]);
        // A removed pack leaves empty entries, and the other numbers stay.
        library.remove("one").unwrap();
        let table = library.table().unwrap();
        assert_eq!(table.species.len(), 3);
        assert!(table.species[0].name.is_empty() && table.species[1].name.is_empty());
        assert_eq!(table.species[2].name, "Gamma");
        library.install(pack("four", &["Delta"])).unwrap();
        assert_eq!(library.number_of("Delta"), Some(727), "a removed place is not given again");
        // The same pack again gets its numbers back.
        library.install(pack("one", &["Alpha", "Beta"])).unwrap();
        assert_eq!(library.table().unwrap().species[0].name, "Alpha");
    }

    #[test]
    fn the_models_fill_the_first_carrier_and_then_the_second() {
        // A carrier has 255 slots at most (the count is one byte): its own model and 254 models of the packs.
        assert_eq!((library::carrier_form(0), library::carrier_form(253)), (1, 254));
        assert_eq!((library::carrier_form(254), library::carrier_form(507)), (257, 510));
        assert_eq!(library::MAX_MODELS, 508);
    }

    #[test]
    fn packs_that_need_other_packs_wait_for_them() {
        let mut forms = pack("forms", &[]);
        forms.forms.push(PackForm { species: Ref::key("Beta"), form: 1, name: String::new(), model: None, female_model: None, data: pack::to_hex(&[5u8; pack::DATA_SIZE]), mega: None });
        let mut library = Library::default();
        // The forms come first in the list but need Beta.
        let results = library.install_many(vec![forms, pack("one", &["Alpha", "Beta"]), pack("one", &["Alpha"])]);
        assert!(results.iter().filter(|(id, r)| id == "forms" && r.is_ok()).count() == 1);
        assert_eq!(library.number_of("Beta"), Some(725));
        assert_eq!(library.registry.forms.len(), 1);
        // A pack that is given two times is installed again, and keeps its numbers.
        assert_eq!(results.iter().filter(|(id, r)| id == "one" && r.is_ok()).count(), 2);
        assert_eq!(library.number_of("Alpha"), Some(724));
        // A pack that needs a species that nobody has is refused.
        let mut lonely = pack("lonely", &["Omega"]);
        lonely.species[0].evolutions = vec![pack::Evolution { method: 4, arg: 1, species: Ref::key("Nowhere") }];
        let results = library.install_many(vec![lonely]);
        assert!(results[0].1.as_ref().unwrap_err().contains("Nowhere"));
    }

    #[test]
    fn the_table_is_written_and_read_back_from_the_sd_card() {
        let folder = std::env::temp_dir().join(format!("sango_pack_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut library = Library::default();
        library.install(pack("one", &["Alpha", "Beta"])).unwrap();
        let mut forms = pack("forms", &[]);
        forms.forms.push(PackForm { species: Ref::key("Beta"), form: 1, name: "Big".into(), model: None, female_model: None, data: pack::to_hex(&[5u8; pack::DATA_SIZE]), mega: None });
        forms.forms.push(PackForm { species: Ref::Game(25), form: 2, name: String::new(), model: Some(ModelRef::Native { native: 25 }), female_model: None, data: pack::to_hex(&[6u8; pack::DATA_SIZE]), mega: None });
        library.install(forms).unwrap();
        library.save(&sd).unwrap();
        let again = Library::open(&sd).unwrap();
        assert_eq!(again.registry, library.registry);
        let table = Table::from_bytes(&sd.read(library::TABLE_PATH).unwrap()).unwrap();
        assert_eq!(table, library.table().unwrap());
        assert_eq!(table.forms[0].species, 725, "a form of a pack species gets the number of that species");
        assert_eq!(table.forms[1].species, 25);
        assert_eq!(table.forms[1].model_form, 0x8000 | 25);
        let bytes = table.to_bytes().unwrap();
        assert_eq!(bytes.len(), table::HEADER_SIZE + 2 * table::SPECIES_SIZE + 2 * table::FORM_SIZE);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// With the data files of the plugin: the table of the three packs made from them is the table that the plugin builds from
    /// the files. Only the model forms differ: Studio gives the places of the models itself, so the test checks that the models
    /// that were shared are still shared. `SANGO_PLUGIN` is the folder of the plugin repository, `SANGO_OLD_MODELS` the folder of
    /// the old model files (`pokemodel`) and `SANGO_DUMP` the romfs dump.
    #[test]
    fn the_packs_made_from_the_plugin_files_give_the_table_of_the_plugin() {
        let (Ok(root), Ok(old_models), Ok(dump)) = (std::env::var("SANGO_PLUGIN"), std::env::var("SANGO_OLD_MODELS"), std::env::var("SANGO_DUMP")) else { return };
        let dump = sango_core::Dump::open(&dump).unwrap();
        let archive = sango_core::Garc::open(&dump.archive(8)).unwrap();
        let old = inc::OldModels::open(std::path::Path::new(&old_models), archive.first_file(0).unwrap()).unwrap();
        let read = |name: &str| std::fs::read_to_string(format!("{root}/lib/src/pokemon/data/{name}")).unwrap();
        let gen7 = inc::read_species(&read("gen7_species.inc"), "kGen7Species").unwrap();
        let gen8 = inc::read_species(&read("gen8_species.inc"), "kGen8Species").unwrap();
        let forms = inc::read_forms(&read("alolan_form.inc")).unwrap();
        assert_eq!((gen7.len(), gen8.len()), (81, 216));
        let all: Vec<_> = gen7.iter().chain(&gen8).cloned().collect();
        let expected = inc::table_from_inc(&all, &forms).unwrap();

        let mut library = Library::default();
        for migrated in inc::packs_from_inc(&gen7, &gen8, &forms, &old).unwrap() {
            library.install(migrated.pack).unwrap();
        }
        // The forms of the species of the game that had models at the end of the list: 88 forms with one slot each, and two more slots
        // for the two species whose form needs a second model.
        let slots: usize = library.registry.forms.iter().map(|f| f.slots as usize).sum();
        assert_eq!(slots, 90);
        let mut got = library.table().unwrap();
        // The models: the old number and the new number of each species and form.
        let old: Vec<u16> = expected.species.iter().map(|s| s.model_form).chain(expected.forms.iter().map(|f| f.model_form)).collect();
        let new: Vec<u16> = got.species.iter().map(|s| s.model_form).chain(got.forms.iter().map(|f| f.model_form)).collect();
        let mut seen: std::collections::HashMap<u16, u16> = std::collections::HashMap::new();
        for (old, new) in old.iter().zip(&new) {
            if old & 0x8000 != 0 || *old == 0 {
                assert_eq!(old, new, "a model of the game, or no model, stays as it is");
            } else {
                assert_eq!(*seen.entry(*old).or_insert(*new), *new, "the entries that shared a model still share it");
            }
        }
        let distinct_new: std::collections::HashSet<u16> = seen.values().copied().collect();
        assert_eq!(distinct_new.len(), seen.len(), "two different models never get the same place");
        let mut compare = expected.clone();
        for species in compare.species.iter_mut().chain(got.species.iter_mut()) {
            species.model_form = 0;
        }
        for form in compare.forms.iter_mut().chain(got.forms.iter_mut()) {
            form.model_form = 0;
        }
        assert_eq!(got, compare, "the data of the packs is the data of the plugin files");
    }

    /// A model that the user imports replaces the one of the pack, and the original comes back with the reset.
    #[test]
    fn an_imported_model_replaces_the_model_of_a_species_and_the_original_comes_back() {
        let outer = std::env::temp_dir().join(format!("sango_pack_import_{}", std::process::id()));
        let sdmc = outer.join("sdmc");
        std::fs::create_dir_all(sdmc.join("sango-studio")).unwrap();
        let sd = SdCard::open(sdmc.to_str().unwrap()).unwrap();
        let mut pack = pack("modeled", &["Alpha", "Beta"]);
        pack.species[0].model = ModelRef::File { file: "Alpha".into() };
        pack.species[1].model = ModelRef::File { file: "Beta".into() };
        let files_of = |name: &str| -> Vec<Vec<u8>> { (0..8u8).map(|n| vec![name.as_bytes()[0], n, 9, 9]).collect() };
        let file = sdmc.join("modeled.sangopack");
        pack.save_with_models(&file, &mut |name| Ok(files_of(name))).unwrap();
        let mut library = Library::default();
        library.install(Pack::open(&file).unwrap()).unwrap();
        library.save(&sd).unwrap();

        // Import: the species uses the new model, and the file of the pack still has the original.
        library.set_model(&sd, &Ref::key("Alpha"), 0, Some(files_of("Imported"))).unwrap();
        let pack = &library.packs[0];
        assert_eq!(pack.species[0].model, ModelRef::File { file: "Alpha+import".into() });
        assert_eq!(pack.read_model("Alpha+import").unwrap(), files_of("Imported"));
        assert_eq!(pack.read_model("Alpha").unwrap(), files_of("Alpha"), "the original is kept");
        assert_eq!(pack.read_model("Beta").unwrap(), files_of("Beta"));
        // The file of the SD card is the one that was written, and it opens again.
        let on_card = Pack::open(&sd.path("species/packs/modeled.sangopack").unwrap()).unwrap();
        assert_eq!(on_card.species[0].model, pack.species[0].model);
        // Importing again replaces the imported model.
        library.set_model(&sd, &Ref::key("Alpha"), 0, Some(files_of("Second"))).unwrap();
        assert_eq!(library.packs[0].read_model("Alpha+import").unwrap(), files_of("Second"));
        // Reset: the original model is back, and the imported one leaves the file.
        library.set_model(&sd, &Ref::key("Alpha"), 0, None).unwrap();
        assert_eq!(library.packs[0].species[0].model, ModelRef::File { file: "Alpha".into() });
        assert!(library.packs[0].read_model("Alpha+import").is_err());
        assert_eq!(library.packs[0].read_model("Alpha").unwrap(), files_of("Alpha"));
        assert!(library.set_model(&sd, &Ref::key("Alpha"), 0, None).is_err(), "nothing to reset");
        // The model of a game species cannot be replaced, and a species that no pack has is refused.
        assert!(library.set_model(&sd, &Ref::key("Nobody"), 0, Some(files_of("X"))).is_err());
        let _ = std::fs::remove_dir_all(&outer);
    }

    /// With the dump of the game: the models of a pack go to `species/model`, in the form that the plugin reads.
    #[test]
    fn the_models_of_a_pack_are_written_where_the_plugin_reads_them() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let outer = std::env::temp_dir().join(format!("sango_pack_models_{}", std::process::id()));
        let sdmc = outer.join("sdmc");
        std::fs::create_dir_all(sdmc.join("sango-studio")).unwrap();
        let sd = SdCard::open(sdmc.to_str().unwrap()).unwrap();
        // A pack with two models and three species and forms that use them.
        let mut pack = pack("modeled", &["Alpha", "Beta"]);
        pack.species[0].model = ModelRef::File { file: "Alpha".into() };
        pack.species[1].model = ModelRef::File { file: "Beta".into() };
        pack.forms.push(PackForm { species: Ref::key("Alpha"), form: 1, name: String::new(), model: Some(ModelRef::File { file: "Alpha".into() }), female_model: None, data: pack::to_hex(&[5u8; pack::DATA_SIZE]), mega: None });
        // A form of a species of the game with a model of its own: Venusaur gets one more slot at the end of its list.
        pack.forms.push(PackForm { species: Ref::Game(3), form: 2, name: String::new(), model: Some(ModelRef::File { file: "Gigantamax".into() }), female_model: None, data: pack::to_hex(&[4u8; pack::DATA_SIZE]), mega: None });
        let files_of = |name: &str| -> Vec<Vec<u8>> { (0..8u8).map(|n| vec![name.as_bytes()[0], n, 9, 9]).collect() };
        let file = sdmc.join("modeled.sangopack");
        pack.save_with_models(&file, &mut |name| Ok(files_of(name))).unwrap();
        let opened = Pack::open(&file).unwrap();
        assert_eq!(opened.model_names(), ["Alpha", "Beta", "Gigantamax"]);
        assert_eq!(opened.read_model("Beta").unwrap(), files_of("Beta"));
        assert!(opened.read_model("Gamma").is_err());

        let mut library = Library::default();
        library.install(opened).unwrap();
        assert_eq!(library.apply(&sd, &dump).unwrap(), 3);
        // The table: the models are the forms 1 and 2 of the first carrier.
        let table = Table::from_bytes(&sd.read(library::TABLE_PATH).unwrap()).unwrap();
        assert_eq!((table.species[0].model_form, table.species[1].model_form, table.forms[0].model_form), (1, 2, 1));
        // The files: the list, the index and the files of the slots.
        let folder = sd.models_folder();
        let raw = std::fs::read(folder.join("0.raw")).unwrap();
        assert_eq!(u32::from_le_bytes(raw[..4].try_into().unwrap()) as usize, raw.len() - 4);
        // The list has an entry for each slot, the new ones too: 722 species of 4 bytes, then 2 bytes for each slot.
        let slots = (raw.len() - 4 - 722 * 4) / 2;
        let at_first = (494 - 1) * 4 + 4;
        let first_slot = u16::from_le_bytes([raw[at_first], raw[at_first + 1]]) as usize;
        assert_eq!(slots, first_slot + 3 + 4, "the carrier block, then the list of Venusaur with its 3 models and the model of the form");
        let at = (494 - 1) * 4 + 4;
        let (first, count) = (u16::from_le_bytes([raw[at], raw[at + 1]]) as usize, raw[at + 2]);
        assert_eq!(count, 3, "the own model of the carrier and the two models of the pack");
        let slot_file = |slot: usize, file: usize| std::fs::read(folder.join(format!("{}.pc", (first + slot) * 8 + 3 + file))).unwrap();
        assert_eq!(&slot_file(1, 0)[4..], files_of("Alpha")[0].as_slice());
        assert_eq!(&slot_file(2, 7)[4..], files_of("Beta")[7].as_slice());
        assert_eq!(u32::from_le_bytes(slot_file(2, 7)[..4].try_into().unwrap()) as usize, 4);
        // The own model of the carrier is the file of the game.
        let archive = sango_core::Garc::open(&dump.archive(library::MODEL_ARCHIVE)).unwrap();
        let victini_first = u16::from_le_bytes([archive.first_file(0).unwrap()[(494 - 1) * 4], archive.first_file(0).unwrap()[(494 - 1) * 4 + 1]]) as usize;
        // The plugin compresses the files of the SD card for the game, so the copies of the files of the game are not compressed.
        let plain = |id: usize| {
            let bytes = archive.first_file(id).unwrap();
            if sango_core::lz::is_compressed(bytes) {
                sango_core::lz::decompress(bytes).unwrap()
            } else {
                bytes.to_vec()
            }
        };
        assert_eq!(&slot_file(0, 0)[4..], plain(victini_first * 8 + 3).as_slice());
        let index = std::fs::read(folder.join("index.bin")).unwrap();
        let entries = u32::from_le_bytes(index[..4].try_into().unwrap()) as usize;
        assert_eq!(index.len(), 4 + 8 * entries);
        assert_eq!(entries, 7 * 8, "seven slots of eight files: the carrier block (3) and the list of Venusaur (4)");
        // Venusaur: its list is at the end, with its own models copied and the model of the form after them.
        let venusaur = |raw: &[u8]| (u16::from_le_bytes([raw[4 + 2 * 4], raw[4 + 2 * 4 + 1]]) as usize, raw[4 + 2 * 4 + 2] as usize, raw[4 + 2 * 4 + 3]);
        let (v_first, v_count, v_flags) = venusaur(&raw);
        assert_eq!(v_count, 4, "the three models of the game and the model of the form");
        assert!(v_first >= first_slot + 3, "the list of Venusaur is after the carrier");
        assert_eq!(v_flags & 4, 4);
        let v_file = |slot: usize, file: usize| std::fs::read(folder.join(format!("{}.pc", (v_first + slot) * 8 + 3 + file))).unwrap();
        assert_eq!(&v_file(3, 2)[4..], files_of("Gigantamax")[2].as_slice());
        let game_first = u16::from_le_bytes([archive.first_file(0).unwrap()[2 * 4], archive.first_file(0).unwrap()[2 * 4 + 1]]) as usize;
        assert_eq!(&v_file(1, 0)[4..], plain((game_first + 1) * 8 + 3).as_slice(), "the models of the game are copied, not compressed");
        // A model of a removed pack has its place but no files.
        library.remove("modeled").unwrap();
        assert_eq!(library.write_models(&sd.models_folder(), &dump).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&outer);
    }

    #[test]
    fn a_new_form_is_kept_in_the_pack_of_the_user_and_written_in_the_table() {
        let folder = std::env::temp_dir().join(format!("sango_pack_new_form_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut library = Library::open(&sd).unwrap();
        library.add_form(&sd, Ref::Game(25), 3, &[7u8; pack::DATA_SIZE], MegaCondition::Item(0x10E)).unwrap();
        library.add_form(&sd, Ref::Game(25), 4, &[8u8; pack::DATA_SIZE], MegaCondition::Never).unwrap();
        assert!(library.add_form(&sd, Ref::Game(25), 3, &[1u8; pack::DATA_SIZE], MegaCondition::Never).is_err(), "a form number is used once");
        library.save(&sd).unwrap();
        let again = Library::open(&sd).unwrap();
        assert_eq!(again.registry, library.registry);
        let table = Table::from_bytes(&sd.read(library::TABLE_PATH).unwrap()).unwrap();
        assert_eq!((table.forms.len(), table.forms[0].species, table.forms[0].form, table.forms[1].form), (2, 25, 3, 4));
        assert_eq!(table.forms[0].data, vec![7u8; pack::DATA_SIZE]);
        assert_eq!((table.forms[0].mega, table.forms[1].mega), (MegaCondition::Item(0x10E), MegaCondition::Never), "a new form says its condition");
        for condition in [MegaCondition::Always, MegaCondition::Move(620), MegaCondition::Ability(70), MegaCondition::Item(0xDC)] {
            library.set_mega(&sd, &Ref::Game(25), 4, condition).unwrap();
            let table = library.table().unwrap();
            assert_eq!(table.forms[1].mega, condition);
            assert_eq!(Table::from_bytes(&table.to_bytes().unwrap()).unwrap().forms[1].mega, condition, "the file keeps the condition");
        }
        assert!(library.set_mega(&sd, &Ref::Game(25), 4, MegaCondition::Move(9000)).is_err(), "a number that does not fit is refused");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_new_form_gets_a_model_of_its_own_and_gives_it_back() {
        let folder = std::env::temp_dir().join(format!("sango_pack_form_model_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let files = |name: &str| -> Vec<Vec<u8>> { (0..8u8).map(|n| vec![name.as_bytes()[0], n, 9, 9]).collect() };
        let mut library = Library::open(&sd).unwrap();
        library.add_form(&sd, Ref::Game(197), 1, &[7u8; pack::DATA_SIZE], MegaCondition::Item(0x10E)).unwrap();
        assert_eq!(library.registry.forms[0].slots, 0, "a form without a model has no slot");
        // The import gives only this form a model: it takes a slot after the models of the species.
        library.set_form_model(&sd, &Ref::Game(197), 1, Some(files("Mega")), "pm0197_01").unwrap();
        assert_eq!(library.registry.forms[0].slots, 1);
        let form = &library.packs[0].forms[0];
        assert_eq!(form.model, Some(ModelRef::File { file: format!("pm0197_01{}", library::USER_SUFFIX) }));
        // A second import replaces the files and keeps the name.
        library.set_form_model(&sd, &Ref::Game(197), 1, Some(files("Other")), "pm0197_01").unwrap();
        assert_eq!(library.packs[0].read_model(&format!("pm0197_01{}", library::USER_SUFFIX)).unwrap()[0][0], b'O');
        // Taking the model away gives the form back the model of the game.
        library.set_form_model(&sd, &Ref::Game(197), 1, None, "").unwrap();
        assert_eq!((library.registry.forms[0].slots, library.packs[0].forms[0].model.clone()), (0, None));
        assert!(library.set_form_model(&sd, &Ref::Game(197), 1, None, "").is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
