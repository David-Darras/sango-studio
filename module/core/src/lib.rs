//! Shared base of Sango Studio.
//!
//! The formats of the game, the checks and the export to the SD card will live here.

mod dump;
mod setting;

pub mod garc;

pub use dump::Dump;
pub use garc::Garc;

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
    /// The dump of the romfs of the game. Sango Studio does nothing without it.
    pub dump: Option<Dump>,
    /// True when the arrow keys belong to the page. False when they change the page of the side bar.
    pub focus_page: bool,
}

impl Session {
    /// A session that opens the dump of the last run, when it is still there.
    pub fn restore() -> Session {
        let dump = setting::read_dump_path().and_then(|path| Dump::open(&path).ok());
        Session { dump, focus_page: false }
    }

    /// Takes a folder as the dump, and keeps it for the next run.
    pub fn set_dump(&mut self, path: &str) -> Result<(), String> {
        let dump = Dump::open(path)?;
        setting::write_dump_path(&dump.root().to_string_lossy());
        self.dump = Some(dump);
        Ok(())
    }
}
