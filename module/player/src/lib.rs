//! The Player page: the pictures and the 3D models of the two player characters, Brendan and May.
//!
//! The page lists them for one character: the PSS icon, the head on the Town Map, the arrow of the Soar map, and the models of the
//! overworld, of the Soar sequence and of the battles. Each one can be exported, imported and put back as in the game. See `store.rs`
//! for the files.

mod icons;
mod page;
mod store;

pub use page::PlayerPage;
