//! Makes the pack of the musics of Pokémon X and Y (see `sango_music::xy`).
//!
//! `make_xy_music_pack <files of Omega Ruby or Alpha Sapphire> <files of X or Y> <pack file>`
//!
//! The two folders are the files of the games (the folders that have `sound`). The musics are converted in parallel: some minutes.

use sango_pack::pack::{FileKind, PackFile};
use sango_pack::Pack;
use std::path::Path;
use std::sync::Mutex;

/// A readable name from the name of a sound: `STRM_BGM_C_01` gives `C 01`.
fn readable(name: &str) -> String {
    let short = name.trim_start_matches("STRM_").trim_start_matches("BGM_").trim_start_matches("ME_");
    let mut words: Vec<String> = short.split('_').filter(|w| !w.is_empty()).map(|w| {
        let lower = w.to_lowercase();
        let mut letters = lower.chars();
        letters.next().map(|first| first.to_uppercase().collect::<String>() + letters.as_str()).unwrap_or_default()
    }).collect();
    if words.is_empty() {
        words.push(name.to_string());
    }
    words.join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("make_xy_music_pack <files of ORAS> <files of X or Y> <pack file>");
        std::process::exit(2);
    }
    let (oras, xy, out) = (Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]));
    let pairings = sango_music::xy::pairings(oras, xy).unwrap_or_else(|e| panic!("{e}"));
    println!("{} musics of X and Y have a place in the game", pairings.len());
    let results: Mutex<Vec<(usize, Result<Vec<u8>, String>)>> = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(12);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let index = {
                    let mut next = next.lock().unwrap();
                    let index = *next;
                    *next += 1;
                    index
                };
                let Some(pairing) = pairings.get(index) else { break };
                let result = sango_music::xy::convert(oras, xy, pairing);
                println!("{} -> {}: {}", pairing.source, pairing.target, if result.is_ok() { "ok" } else { "failed" });
                results.lock().unwrap().push((index, result));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(index, _)| *index);
    let mut pack = Pack {
        id: "xy-music".into(),
        name: "Musics of X and Y".into(),
        version: "1".into(),
        credit: "Musics of Pokémon X and Y (Game Freak), from your own dump of the game.".into(),
        ..Pack::default()
    };
    for (index, result) in results {
        let pairing = &pairings[index];
        match result {
            Ok(bytes) => {
                let file = pairing.source.trim_end_matches(".aac").to_string() + ".bcstm";
                let replaced: Vec<String> = pairing.target_names.iter().map(|n| readable(n)).collect();
                pack.files.push(PackFile { kind: FileKind::Music, target: pairing.target.clone(), file: file.clone(), label: format!("{} (X and Y), in place of {}", readable(&pairing.source_name), replaced.join(", ")) });
                pack.file_data.insert(file, bytes);
            }
            Err(message) => eprintln!("{}: {message}", pairing.source),
        }
    }
    pack.check().unwrap_or_else(|e| panic!("{e}"));
    pack.save(out).unwrap_or_else(|e| panic!("{e}"));
    println!("{} musics in {}", pack.files.len(), out.display());
}
