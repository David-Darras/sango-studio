use std::path::PathBuf;

/// The file of the settings: in the folder of the application data of the user.
fn file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("SangoStudio").join("dump_path.txt"))
}

/// The file of the first name of the application (Sango Editor): read when the new one is not there.
fn old_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("SangoEditor").join("dump_path.txt"))
}

/// The dump of the last run.
pub fn read_dump_path() -> Option<String> {
    let text = [file(), old_file()].into_iter().flatten().find_map(|path| std::fs::read_to_string(path).ok())?;
    let line = text.lines().next()?.trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// Keeps the dump for the next run. A failure is not an error: Sango Studio works without it.
pub fn write_dump_path(path: &str) {
    if let Some(file) = file() {
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = std::fs::write(file, path);
    }
}
