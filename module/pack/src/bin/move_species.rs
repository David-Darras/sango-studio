//! Moves species from one pack file to another one (to cut a pack in two).
//!
//! Usage: `move_species <from pack> <to pack> <file of names> [id] [name]`
//!
//! The file of names has one species name for each line, in any letters: `Mr. Rime` and `mr-rime` are the same name. The pack
//! `to` is made when its file does not exist (then `id` and `name` are needed). The models of the species go with them. The
//! species keep their numbers in the registry: Studio gives them to the new pack when both packs are installed together.

use sango_pack::pack::ModelRef;
use sango_pack::Pack;
use std::path::Path;

fn key(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

fn run(args: &[String]) -> Result<String, String> {
    let (from_file, to_file) = (Path::new(&args[0]), Path::new(&args[1]));
    let names: Vec<String> = std::fs::read_to_string(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?.lines().map(key).filter(|n| !n.is_empty()).collect();
    let from = Pack::open(from_file)?;
    let to_old = if to_file.exists() { Some(Pack::open(to_file)?) } else { None };
    let mut new_from = from.clone();
    let mut new_to = to_old.clone().unwrap_or_else(|| Pack {
        id: args.get(3).cloned().unwrap_or_default(),
        name: args.get(4).cloned().unwrap_or_default(),
        version: "1.0".into(),
        author: from.author.clone(),
        license: from.license.clone(),
        credit: from.credit.clone(),
        ..Pack::default()
    });
    if to_old.is_none() && new_to.id.is_empty() {
        return Err("The pack does not exist: give its id and its name.".into());
    }
    let (moved, kept): (Vec<_>, Vec<_>) = from.species.iter().cloned().partition(|s| names.contains(&key(&s.name)));
    new_from.species = kept;
    let fresh: Vec<_> = moved.iter().filter(|s| !new_to.species.iter().any(|t| t.name == s.name)).cloned().collect();
    new_to.species.extend(fresh);
    new_from.check()?;
    new_to.check()?;
    // The models: each pack writes the models that its species and forms name; the ones of the moved species come from the old pack.
    let model_for = |name: &str, own: &Option<Pack>| -> Result<Vec<Vec<u8>>, String> {
        match own {
            Some(pack) if pack.model_names().iter().any(|n| n == name) && pack.read_model(name).is_ok() => pack.read_model(name),
            _ => from.read_model(name),
        }
    };
    let temp_from = from_file.with_extension("sangopack.tmp");
    let temp_to = to_file.with_extension("sangopack.tmp");
    new_from.save_with_models(&temp_from, &mut |name| from.read_model(name))?;
    new_to.save_with_models(&temp_to, &mut |name| model_for(name, &to_old))?;
    std::fs::rename(&temp_from, from_file).map_err(|e| e.to_string())?;
    std::fs::rename(&temp_to, to_file).map_err(|e| e.to_string())?;
    let models = moved.iter().filter(|s| matches!(s.model, ModelRef::File { .. })).count();
    Ok(format!("{} species moved ({models} with their own model): '{}' has {} left, '{}' has {}", moved.len(), new_from.id, new_from.species.len(), new_to.id, new_to.species.len()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("Usage: move_species <from pack> <to pack> <file of names> [id] [name]");
        std::process::exit(2);
    }
    match run(&args) {
        Ok(message) => println!("{message}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
