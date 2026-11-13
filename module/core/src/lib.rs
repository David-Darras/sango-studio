//! Shared base of Sango Studio.
//!
//! The formats of the game, the checks and the export to the SD card will live here.

mod dump;
mod sd;
mod setting;

pub mod backup;
pub mod bundle;
pub mod clim;
pub mod csv_text;
pub mod custom_ability;
pub mod custom_item;
pub mod custom_type;
pub mod evolution;
pub mod experience;
pub mod garc;
pub mod item_icons;
pub mod learnset;
pub mod lz;
pub mod model;
pub mod motion;
pub mod modified;
pub mod moves;
pub mod posing;
pub mod render;
pub mod text;

pub use backup::Backup;
pub use bundle::Bundle;
pub use clim::Clim;
pub use dump::Dump;
pub use garc::Garc;
pub use sd::{SdCard, SD_FOLDER_NAME};

/// The name of the application.
pub const NAME: &str = "Sango Studio";

/// The version of the application.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The only game that Sango Studio supports.
pub const GAME: &str = "Pokemon Alpha Sapphire";

/// The only version of that game that Sango Studio supports.
pub const GAME_VERSION: &str = "1.4";

/// What all the pages share while the application runs.
#[derive(Default)]
pub struct Session {
    /// The dump of the romfs of the game. Sango Studio only reads it.
    pub dump: Option<Dump>,
    /// The folder of the SD card. Sango Studio puts here the files that it changed.
    pub sd: Option<SdCard>,
    /// The folder of devkitPro, which has the compiler of the code editor.
    pub devkitpro: Option<std::path::PathBuf>,
    /// The folder of the Sango plugin: the headers of the code editor are in `lib/include`.
    pub plugin: Option<std::path::PathBuf>,
    /// True when the arrow keys belong to the page. False when they change the page of the side bar.
    pub focus_page: bool,
}

/// The compiler of devkitARM in a folder of devkitPro (the program has `.exe` on Windows).
pub fn compiler_in(devkitpro: &std::path::Path) -> Option<std::path::PathBuf> {
    ["arm-none-eabi-g++.exe", "arm-none-eabi-g++"].iter().map(|name| devkitpro.join("devkitARM").join("bin").join(name)).find(|path| path.is_file())
}

impl Session {
    /// A session that opens the two folders of the last run, when they are still there.
    pub fn restore() -> Session {
        let settings = setting::read();
        Session {
            dump: settings.dump.and_then(|path| Dump::open(&path).ok()),
            sd: settings.sd.and_then(|path| SdCard::open(&path).ok()),
            devkitpro: settings.devkitpro.map(std::path::PathBuf::from).filter(|path| compiler_in(path).is_some()),
            plugin: settings.plugin.map(std::path::PathBuf::from).filter(|path| path.join("lib").join("include").is_dir()),
            focus_page: false,
        }
    }

    /// Takes a folder as the dump, and keeps it for the next run.
    pub fn set_dump(&mut self, path: &str) -> Result<(), String> {
        let dump = Dump::open(path)?;
        self.dump = Some(dump);
        self.save();
        Ok(())
    }

    /// Takes a folder as the SD card, and keeps it for the next run.
    pub fn set_sd(&mut self, path: &str) -> Result<(), String> {
        let sd = SdCard::open(path)?;
        self.sd = Some(sd);
        self.save();
        Ok(())
    }

    /// Takes a folder as devkitPro, and keeps it for the next run. The folder is the one of devkitPro (with `devkitARM`
    /// inside) or the folder `devkitARM` itself.
    pub fn set_devkitpro(&mut self, path: &str) -> Result<(), String> {
        let given = std::path::PathBuf::from(path.trim().trim_matches('"'));
        let root = if given.join("bin").join("arm-none-eabi-g++.exe").is_file() || given.join("bin").join("arm-none-eabi-g++").is_file() {
            given.parent().map(|p| p.to_path_buf()).unwrap_or(given.clone())
        } else {
            given.clone()
        };
        if compiler_in(&root).is_none() {
            return Err("This folder has no devkitARM/bin/arm-none-eabi-g++. Give the folder of devkitPro (for example C:/devkitPro).".into());
        }
        self.devkitpro = Some(root);
        self.save();
        Ok(())
    }

    /// Takes a folder as the Sango plugin, and keeps it for the next run.
    pub fn set_plugin(&mut self, path: &str) -> Result<(), String> {
        let folder = std::path::PathBuf::from(path.trim().trim_matches('"'));
        if !folder.join("lib").join("include").is_dir() {
            return Err("This folder has no lib/include. Give the folder of the Sango plugin (the one with the Makefile).".into());
        }
        self.plugin = Some(folder);
        self.save();
        Ok(())
    }

    /// Says what is missing before the editors can work: the two folders are needed.
    pub fn missing(&self) -> Option<&'static str> {
        match (&self.dump, &self.sd) {
            (None, None) => Some("Choose the romfs dump and the SD card folder in Home first."),
            (None, Some(_)) => Some("Choose the romfs dump in Home first."),
            (Some(_), None) => Some("Choose the SD card folder in Home first."),
            (Some(_), Some(_)) => None,
        }
    }

    fn save(&self) {
        setting::write(&setting::Settings {
            dump: self.dump.as_ref().map(|d| d.root().to_string_lossy().to_string()),
            sd: self.sd.as_ref().map(|s| s.root().to_string_lossy().to_string()),
            devkitpro: self.devkitpro.as_ref().map(|p| p.to_string_lossy().to_string()),
            plugin: self.plugin.as_ref().map(|p| p.to_string_lossy().to_string()),
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_editors_need_the_two_folders() {
        let mut session = Session::default();
        assert!(session.missing().unwrap().contains("dump and the SD card"));
        session.sd = SdCard::open(&std::env::temp_dir().to_string_lossy()).ok();
        assert_eq!(session.missing(), Some("Choose the romfs dump in Home first."));
    }
}
