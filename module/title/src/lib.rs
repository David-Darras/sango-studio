//! The Title Screen page: the screen that the game shows when it starts.
//!
//! A title screen is two sequences that play at the same time, one for each screen. The archive 162 has the title screen of Alpha
//! Sapphire (files 3 and 86) and the one of Omega Ruby (files 2 and 85): the menu of the plugin chooses the one that plays, also on
//! Alpha Sapphire. A sequence is a list of steps with their first and last frame: it makes the camera, the light, the 3D models, the particles, the Pokémon, the logo and
//! the music. The files that the steps name are in the archive 152. See `sequence.rs` for the format and `store.rs` for the files.

mod commands;
mod pokemon;
mod sequence;
mod store;

mod page;

pub use page::TitlePage;
