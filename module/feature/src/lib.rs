//! The Features page: turns features of the plugin on or off.
//!
//! The plugin reads `features.ini` when the game starts (`sdmc:/sango-studio/features.ini`). The file has only the features that the user
//! changed; the other ones keep the value that the plugin gives them. See `list.rs` for the features and their names in the file.

mod list;
mod page;

pub use page::FeaturePage;
