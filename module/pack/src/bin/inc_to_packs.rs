//! Makes the three first packs (`gen7`, `gen8`, `forms`) from the data files and the model files that the plugin had before
//! the packs. The packs have their models inside.
//!
//! Usage: `inc_to_packs <folder of the plugin repository> <folder of the old models> <folder of the romfs dump> <output folder>`
//!
//! The folder of the old models has `0.raw`, `index.bin` and the files `<number>.pc` (`sango/pokemodel` or
//! `sango_off/pokemodel` on the SD card).

use sango_core::{Dump, Garc};
use sango_pack::inc::{self, OldModels};
use std::path::Path;

fn run(plugin: &Path, models: &Path, dump: &str, out: &Path) -> Result<(), String> {
    let read = |name: &str| std::fs::read_to_string(plugin.join("lib/src/pokemon/data").join(name)).map_err(|e| format!("{name}: {e}"));
    let gen7 = inc::read_species(&read("gen7_species.inc")?, "kGen7Species")?;
    let gen8 = inc::read_species(&read("gen8_species.inc")?, "kGen8Species")?;
    let forms = inc::read_forms(&read("alolan_form.inc")?)?;
    let dump = Dump::open(dump)?;
    let archive = Garc::open(&dump.archive(8))?;
    let old = OldModels::open(models, archive.first_file(0).ok_or("The list of the models is missing in the archive 8.")?)?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for migrated in inc::packs_from_inc(&gen7, &gen8, &forms, &old)? {
        let path = out.join(format!("{}.sangopack", migrated.pack.id));
        migrated.pack.save_with_models(&path, &mut |name| match (migrated.models.get(name), migrated.slots.get(name)) {
            (Some(form), _) => old.files(*form),
            (None, Some(slot)) => old.files_of_slot(*slot),
            _ => Err(format!("The model '{name}' is not known.")),
        })?;
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!("{}: {} species, {} forms, {} models, {} MB", path.display(), migrated.pack.species.len(), migrated.pack.forms.len(), migrated.pack.model_names().len(), size / 1_000_000);
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: inc_to_packs <folder of the plugin repository> <folder of the old models> <folder of the romfs dump> <output folder>");
        std::process::exit(2);
    }
    if let Err(message) = run(Path::new(&args[1]), Path::new(&args[2]), &args[3], Path::new(&args[4])) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
