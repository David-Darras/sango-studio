//! The Map Data page: what the game knows of each map besides its ground and its events.
//!
//! For each map: its name on the screen, the music, the weather, the battle background, what the player can do there (Fly, Bicycle,
//! Escape Rope...), how it looks, where Fly puts the player, the water, and the camera. See `data.rs` for the files.

mod data;
mod lists;
mod page;

pub use page::ZonePage;
