//! Makes the three first packs (`gen7`, `gen8`, `forms`) from the data files that the plugin had before the packs.
//!
//! Usage: `inc_to_packs <folder of the plugin repository> <output folder>`

use sango_pack::inc;
use std::path::Path;

fn run(plugin: &Path, out: &Path) -> Result<(), String> {
    let read = |name: &str| std::fs::read_to_string(plugin.join("lib/src/pokemon/data").join(name)).map_err(|e| format!("{name}: {e}"));
    let gen7 = inc::read_species(&read("gen7_species.inc")?, "kGen7Species")?;
    let gen8 = inc::read_species(&read("gen8_species.inc")?, "kGen8Species")?;
    let forms = inc::read_forms(&read("alolan_form.inc")?)?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for pack in inc::packs_from_inc(&gen7, &gen8, &forms)? {
        let path = out.join(format!("{}.sangopack", pack.id));
        pack.save(&path)?;
        println!("{}: {} species, {} forms", path.display(), pack.species.len(), pack.forms.len());
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: inc_to_packs <folder of the plugin repository> <output folder>");
        std::process::exit(2);
    }
    if let Err(message) = run(Path::new(&args[1]), Path::new(&args[2])) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
