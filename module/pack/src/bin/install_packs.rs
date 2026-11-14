//! Installs packs on the SD card (the page Packs of Studio does the same).
//!
//! Usage: `install_packs <folder of the SD card> <folder of the romfs dump> <pack file>...`
//!
//! The packs are installed in the order of the command line. Packs that are already installed are skipped.

use sango_core::{Dump, SdCard};
use sango_pack::{Library, Pack};
use std::path::Path;

fn run(sd_folder: &str, dump_folder: &str, files: &[String]) -> Result<(), String> {
    let sd = SdCard::open(sd_folder)?;
    let dump = Dump::open(dump_folder)?;
    let mut library = Library::open(&sd)?;
    let mut packs = Vec::new();
    for file in files {
        let pack = Pack::open(Path::new(file))?;
        if library.registry.packs.iter().any(|p| p.id == pack.id && p.active) {
            println!("{}: already installed", pack.id);
        } else {
            packs.push(pack);
        }
    }
    for (id, result) in library.install_many(packs) {
        match result {
            Ok(()) => println!("{id}: installed"),
            Err(message) => println!("{id}: {message}"),
        }
    }
    let models = library.apply(&sd, &dump)?;
    let table = library.table()?;
    println!("species/table.bin: {} species, {} forms; {models} models written", table.species.len(), table.forms.len());
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: install_packs <folder of the SD card> <folder of the romfs dump> <pack file>...");
        std::process::exit(2);
    }
    if let Err(message) = run(&args[1], &args[2], &args[3..]) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
