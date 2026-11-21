//! Puts the item icons of a folder in a pack file.
//!
//! Usage: `add_item_icons <pack file> <folder of the icons> [credit]`. The folder has `<file>.png` pictures of 30 × 30 pixels
//! (the file of the item icons of the game that each one replaces) and an optional `labels.tsv`. The pack is made if the file
//! does not exist.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        eprintln!("Usage: add_item_icons <pack file> <folder of the icons> [credit]");
        std::process::exit(2);
    }
    let credit = args.get(3).map(String::as_str).unwrap_or("");
    match sango_pack::item_icon_pack::add_item_icons(std::path::Path::new(&args[1]), std::path::Path::new(&args[2]), credit) {
        Ok(report) => {
            println!("{} item icons added", report.added);
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
