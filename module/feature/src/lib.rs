//! The Features page: every setting of the plugin.
//!
//! Without a change, the game is the game: every feature of the plugin is off. The page writes the settings that the user changes in
//! `features.ini`, which the plugin reads when the game starts (`sdmc:/sango-studio/features.ini`). See `list.rs` for the settings
//! and their names in the file.

mod list;
mod page;

pub use page::FeaturePage;
