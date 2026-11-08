use std::path::PathBuf;

/// What Sango Studio keeps between two runs: the two folders that the user gave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The dump of the romfs.
    pub dump: Option<String>,
    /// The folder of the SD card.
    pub sd: Option<String>,
}

fn base() -> Option<PathBuf> {
    // SANGO_CONFIG_DIR gives another folder (for the tests).
    if let Some(folder) = std::env::var_os("SANGO_CONFIG_DIR") {
        return Some(PathBuf::from(folder));
    }
    std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

/// The file of the settings: in the folder of the application data of the user.
fn file() -> Option<PathBuf> {
    Some(base()?.join("SangoStudio").join("settings.txt"))
}

/// The files of the first versions, which kept only the dump: read when the new file is not there.
fn old_files() -> Vec<PathBuf> {
    let Some(base) = base() else { return Vec::new() };
    vec![base.join("SangoStudio").join("dump_path.txt"), base.join("SangoEditor").join("dump_path.txt")]
}

/// Reads the settings from their text: one `key=value` on each line.
pub fn parse(text: &str) -> Settings {
    let mut settings = Settings::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "dump" => settings.dump = Some(value.to_string()),
            "sd" => settings.sd = Some(value.to_string()),
            _ => {}
        }
    }
    settings
}

/// Writes the settings as text.
pub fn format(settings: &Settings) -> String {
    let mut text = String::new();
    if let Some(dump) = &settings.dump {
        text.push_str(&format!("dump={dump}\n"));
    }
    if let Some(sd) = &settings.sd {
        text.push_str(&format!("sd={sd}\n"));
    }
    text
}

/// The settings of the last run.
pub fn read() -> Settings {
    if let Some(text) = file().and_then(|path| std::fs::read_to_string(path).ok()) {
        return parse(&text);
    }
    // A first version kept only the path of the dump, alone on the first line.
    let dump = old_files()
        .into_iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| text.lines().next().map(|line| line.trim().to_string()))
        .filter(|line| !line.is_empty());
    Settings { dump, sd: None }
}

/// Keeps the settings for the next run. A failure is not an error: Sango Studio works without it.
pub fn write(settings: &Settings) {
    if let Some(file) = file() {
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = std::fs::write(file, format(settings));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_settings_are_read_back() {
        let settings = Settings { dump: Some("C:/game/romfs".into()), sd: Some("C:/emu/sdmc/sango-studio".into()) };
        assert_eq!(parse(&format(&settings)), settings);
    }

    #[test]
    fn a_missing_or_empty_value_is_none() {
        assert_eq!(parse("dump=\nsd=  \nother=1\nno equal sign"), Settings::default());
        assert_eq!(parse("sd=D:/card").sd.as_deref(), Some("D:/card"));
        assert_eq!(format(&Settings::default()), "");
    }

    #[test]
    fn a_path_can_have_an_equal_sign() {
        assert_eq!(parse("dump=C:/a=b/romfs").dump.as_deref(), Some("C:/a=b/romfs"));
    }
}
