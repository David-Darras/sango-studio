//! Puts the icons of a folder in a pack file.
//!
//! Usage: `add_icons <pack file> <folder of the icons> [--game]`, or `add_icons <pack file> --clear` to remove the icons.
//!
//! Without `--game`: the icons of the species of the pack. With `--game`: the icons that replace the icons of the species of the game;
//! the pack is made if the file does not exist.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = args.iter().any(|a| a == "--game");
    if args.len() == 3 && args[2] == "--clear" {
        match sango_species::icon_pack::clear_icons(std::path::Path::new(&args[1])) {
            Ok(count) => println!("{count} icons removed"),
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(1);
            }
        }
        return;
    }
    let paths: Vec<&String> = args.iter().skip(1).filter(|a| *a != "--game").collect();
    if paths.len() != 2 {
        eprintln!("Usage: add_icons <pack file> <folder of the icons> [--game]");
        std::process::exit(2);
    }
    match sango_species::icon_pack::add_icons(std::path::Path::new(paths[0]), std::path::Path::new(paths[1]), game) {
        Ok(report) => {
            println!("{} icons added", report.added);
            for line in &report.skipped {
                println!("skipped: {line}");
            }
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
