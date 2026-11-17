//! What the tool remembers between two runs: the folder of the archives and the
//! block that was open.

use std::path::PathBuf;

pub struct Config {
    pub arc_dir: Option<String>,
    pub block: Option<usize>,
    /// The program that starts the game, and the file of the game.
    pub emulator: Option<String>,
    pub game: Option<String>,
}

static FOLDER: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Says where the editor keeps its own files (the last block, the stars of the catalogue, the catalogues): a folder of the workspace of the
/// studio.
pub fn set_folder(folder: &std::path::Path) {
    if let Ok(mut slot) = FOLDER.lock() {
        *slot = Some(folder.to_path_buf());
    }
    copy_old_files(folder);
}

/// The first time: the files that the tool kept alone (in the data of the user, in `sango_editor`) come to the folder of the studio.
fn copy_old_files(folder: &std::path::Path) {
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else { return };
    let old = base.join("sango_editor");
    if folder.exists() || !old.is_dir() || std::fs::create_dir_all(folder).is_err() {
        return;
    }
    if let Ok(list) = std::fs::read_dir(&old) {
        for entry in list.flatten() {
            let _ = std::fs::copy(entry.path(), folder.join(entry.file_name()));
        }
    }
}

/// The folder where the editor keeps its own files.
pub fn folder() -> PathBuf {
    FOLDER.lock().ok().and_then(|slot| slot.clone()).unwrap_or_else(|| std::env::temp_dir().join("sango_overworld"))
}

fn path() -> PathBuf {
    folder().join("config.txt")
}

pub fn load() -> Config {
    let mut config = Config { arc_dir: None, block: None, emulator: None, game: None };
    if let Ok(text) = std::fs::read_to_string(path()) {
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("archives=") {
                config.arc_dir = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("block=") {
                config.block = v.trim().parse().ok();
            } else if let Some(v) = line.strip_prefix("emulator=") {
                config.emulator = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            } else if let Some(v) = line.strip_prefix("game=") {
                config.game = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
        }
    }
    config
}

pub fn save(arc_dir: &str, block: usize) {
    let old = load();
    write(&Config { arc_dir: Some(arc_dir.to_string()), block: Some(block), ..old });
}

/// Keeps the program and the file that start the game.
pub fn save_launch(emulator: &str, game: &str) {
    let old = load();
    write(&Config { emulator: Some(emulator.to_string()), game: Some(game.to_string()), ..old });
}

fn write(config: &Config) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut text = String::new();
    for (key, value) in [("archives", config.arc_dir.clone()), ("block", config.block.map(|b| b.to_string())), ("emulator", config.emulator.clone()), ("game", config.game.clone())] {
        if let Some(value) = value {
            text.push_str(&format!("{key}={value}\n"));
        }
    }
    let _ = std::fs::write(p, text);
}

/// Says whether a folder is a dump of the RomFS (it has the file `a/0/3/9`, the blocks).
fn is_dump(dir: &str) -> bool {
    std::path::Path::new(dir).join("a").join("0").join("3").join("9").is_file()
}

/// The folder of the data of the game: the dump of the RomFS. A folder that the user gives is used when it is a dump, else the folder that
/// was kept.
pub fn archives(given: Option<&str>) -> Result<String, String> {
    if let Some(g) = given.filter(|g| is_dump(g)) {
        return Ok(g.to_string());
    }
    load().arc_dir.filter(|d| is_dump(d)).ok_or_else(|| "cannot find the dump of the RomFS: choose it on the Home page".to_string())
}
