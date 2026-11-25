//! Puts overworld character models (or the battle files of the trainers of X and Y) in a pack file: they replace the files of the
//! game with the same numbers.
//!
//! Usage: `add_characters <pack file> <folder of the models> [labels.tsv] [kind]`. The kind is `character` (the default),
//! `trainer_model`, `trainer_intro`, `trainer_layout`, `background` (numbered files), `background_choices` (the files
//! `choices_<G|F|B|O>.bin`) or `name_list` (the files `bg*.bin` of a folder `names`) (see `FileKind`); the other files than the
//! characters are put in the pack as they are. The folder has `<number>.bin` files: packs of the
//! archive 21 (a model, its motions), compressed or not. `labels.tsv` has lines `<number><TAB><name>`; a number can be written
//! `0x1b`. The pack is made (id `xy-characters`) if the file does not exist. The models are compressed (LZ11) like the files of
//! the archive: the plugin gives them to the game as they are.

use sango_pack::pack::{FileKind, PackFile};
use sango_pack::Pack;
use std::path::Path;

fn number(text: &str) -> Option<u16> {
    let text = text.trim();
    match text.strip_prefix("0x") {
        Some(hex) => u16::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        eprintln!("Usage: add_characters <pack file> <folder of the models> [labels.tsv] [kind]");
        std::process::exit(2);
    }
    let (pack_path, folder) = (Path::new(&args[1]), Path::new(&args[2]));
    let kind = match args.get(4).map(String::as_str).unwrap_or("character") {
        "character" => FileKind::Character,
        "trainer_model" => FileKind::TrainerModel,
        "trainer_intro" => FileKind::TrainerIntro,
        "trainer_layout" => FileKind::TrainerLayout,
        "background" => FileKind::Background,
        "background_choices" => FileKind::BackgroundChoices,
        "name_list" => FileKind::NameList,
        other => panic!("{other} is not a kind of file"),
    };
    let mut labels = std::collections::BTreeMap::new();
    if let Some(file) = args.get(3).filter(|f| !f.is_empty() && *f != "-") {
        for line in std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}")).lines() {
            if let Some((id, name)) = line.split_once('\t').or_else(|| line.split_once(char::is_whitespace)) {
                if let Some(id) = number(id) {
                    labels.insert(id, name.trim().to_string());
                }
            }
        }
    }
    let background = matches!(kind, FileKind::Background | FileKind::BackgroundChoices | FileKind::NameList);
    let mut pack = if pack_path.is_file() {
        let pack = Pack::open(pack_path).unwrap_or_else(|e| panic!("{e}"));
        // The bytes of the files that the pack has stay: they are written again with the new ones.
        let mut with_bytes = pack.clone();
        for file in &pack.files {
            with_bytes.file_data.insert(file.file.clone(), pack.read_file(&file.file).unwrap_or_else(|e| panic!("{e}")));
        }
        with_bytes
    } else if background {
        Pack { id: "xy-backgrounds".into(), name: "Battle backgrounds of X and Y".into(), version: "1".into(), credit: "Battle backgrounds of Pokémon X and Y (Game Freak), from your own dump of the game.".into(), ..Pack::default() }
    } else {
        Pack { id: "xy-characters".into(), name: "Characters of X and Y".into(), version: "1".into(), credit: "Overworld models of Pokémon X and Y (Game Freak), from your own dump of the game.".into(), ..Pack::default() }
    };
    let mut added = 0;
    // The files of the folder and the target of each: the number of the file, or a part of its name for the lists.
    let mut entries: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "bin"))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_string();
            let target = match kind {
                FileKind::BackgroundChoices => stem.strip_prefix("choices_")?.to_string(),
                FileKind::NameList => stem.starts_with("bg").then(|| stem.clone())?,
                _ => stem.parse::<u16>().ok()?.to_string(),
            };
            Some((target, path))
        })
        .collect();
    entries.sort_by_key(|(target, _)| (target.len(), target.clone()));
    for (target, path) in entries {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // A character is compressed again with the real compression (the tools write it "stored", which is bigger). The battle
        // files stay as the tools wrote them.
        let data = if kind == FileKind::Character {
            let raw = if sango_core::lz::is_compressed(&bytes) { sango_core::lz::decompress(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display())) } else { bytes };
            sango_core::lz::compress_lz11(&raw)
        } else {
            bytes
        };
        let prefix = match kind {
            FileKind::Character => "character",
            FileKind::TrainerModel => "trainer_model",
            FileKind::TrainerIntro => "trainer_intro",
            FileKind::TrainerLayout => "trainer_layout",
            FileKind::Background => "background",
            FileKind::BackgroundChoices => "background_choices",
            FileKind::NameList => "names",
            FileKind::Music => "music",
        };
        let file = format!("{prefix}_{target}.bin");
        let label = match target.parse::<u16>() {
            Ok(id) => labels.get(&id).cloned(),
            Err(_) => None,
        }
        .unwrap_or_else(|| format!("{} {target}", kind.label()));
        pack.files.retain(|f| f.file != file);
        pack.files.push(PackFile { kind, target: target.clone(), file: file.clone(), label });
        pack.file_data.insert(file, data);
        added += 1;
    }
    if !background && kind != FileKind::Character {
        pack.name = "Characters and trainers of X and Y".into();
        pack.credit = "Overworld and battle models of Pokémon X and Y (Game Freak), from your own dump of the game.".into();
    }
    pack.check().unwrap_or_else(|e| panic!("{e}"));
    pack.save(pack_path).unwrap_or_else(|e| panic!("{e}"));
    println!("{added} files of the kind {} in {}", kind.label(), pack_path.display());
}
