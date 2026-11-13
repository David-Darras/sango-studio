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

pub mod inc;
pub mod library;
pub mod pack;
pub mod table;

pub use library::Library;
pub use pack::{Pack, PackForm, PackSpecies, Ref};
pub use table::Table;

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::SdCard;

    fn species(name: &str) -> PackSpecies {
        PackSpecies { name: name.into(), model_form: 3, data: pack::to_hex(&[7u8; pack::DATA_SIZE]), moves: vec![pack::Move { id: 33, level: 1 }], evolutions: Vec::new() }
    }

    fn pack(id: &str, names: &[&str]) -> Pack {
        let mut species: Vec<PackSpecies> = names.iter().map(|n| species(n)).collect();
        if names.len() > 1 {
            species[0].evolutions = vec![pack::Evolution { method: 4, arg: 16, species: Ref::key(names[1]) }];
        }
        Pack { id: id.into(), name: id.into(), version: "1".into(), species, ..Pack::default() }
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
        assert!(library.install(pack("one", &["Alpha"])).unwrap_err().contains("already installed"));
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
    fn the_table_is_written_and_read_back_from_the_sd_card() {
        let folder = std::env::temp_dir().join(format!("sango_pack_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut library = Library::default();
        library.install(pack("one", &["Alpha", "Beta"])).unwrap();
        let mut forms = pack("forms", &[]);
        forms.forms.push(PackForm { species: Ref::key("Beta"), form: 1, name: "Big".into(), model_form: 9, data: pack::to_hex(&[5u8; pack::DATA_SIZE]) });
        forms.forms.push(PackForm { species: Ref::Game(25), form: 2, name: String::new(), model_form: 0x8000 | 25, data: pack::to_hex(&[6u8; pack::DATA_SIZE]) });
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

    /// With the data files of the plugin: the table of the three packs made from them is the table that the plugin builds
    /// from the files, byte for byte. `SANGO_PLUGIN` is the folder of the plugin repository.
    #[test]
    fn the_packs_made_from_the_plugin_files_give_the_table_of_the_plugin() {
        let Ok(root) = std::env::var("SANGO_PLUGIN") else { return };
        let read = |name: &str| std::fs::read_to_string(format!("{root}/lib/src/pokemon/data/{name}")).unwrap();
        let gen7 = inc::read_species(&read("gen7_species.inc"), "kGen7Species").unwrap();
        let gen8 = inc::read_species(&read("gen8_species.inc"), "kGen8Species").unwrap();
        let forms = inc::read_forms(&read("alolan_form.inc")).unwrap();
        assert_eq!((gen7.len(), gen8.len()), (81, 216));
        let all: Vec<_> = gen7.iter().chain(&gen8).cloned().collect();
        let expected = inc::table_from_inc(&all, &forms).unwrap();

        // Through the files, the registry and the SD card.
        let folder = std::env::temp_dir().join(format!("sango_pack_inc_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        let mut library = Library::default();
        for pack in inc::packs_from_inc(&gen7, &gen8, &forms).unwrap() {
            let pack = Pack::from_zip(&pack.to_zip().unwrap()).unwrap();
            library.install(pack).unwrap();
        }
        library.save(&sd).unwrap();
        let written = sd.read(library::TABLE_PATH).unwrap();
        assert_eq!(written, expected.to_bytes().unwrap(), "the table of the packs is the table of the plugin files");
        assert_eq!(Library::open(&sd).unwrap().table().unwrap(), expected);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
