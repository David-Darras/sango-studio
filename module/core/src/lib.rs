//! Shared base of Sango Studio.
//!
//! The formats of the game, the checks and the export to the SD card will live here.

mod dump;
mod sd;
mod setting;

pub mod bundle;
pub mod clim;
pub mod evolution;
pub mod experience;
pub mod garc;
pub mod learnset;
pub mod lz;
pub mod model;
pub mod motion;
pub mod moves;
pub mod posing;
pub mod render;
pub mod text;

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
    /// True when the arrow keys belong to the page. False when they change the page of the side bar.
    pub focus_page: bool,
}

impl Session {
    /// A session that opens the two folders of the last run, when they are still there.
    pub fn restore() -> Session {
        let settings = setting::read();
        Session {
            dump: settings.dump.and_then(|path| Dump::open(&path).ok()),
            sd: settings.sd.and_then(|path| SdCard::open(&path).ok()),
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
