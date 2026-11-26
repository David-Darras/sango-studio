//! The Save page: the save file of the game (`main`).
//!
//! The page reads the file, checks the checksum of each block, and edits the trainer, the party, the boxes, the Bag and the place
//! of the player. Save writes the changed blocks only, after a copy of the file, and asks a running game to take them (see
//! [`live`]), so the change shows without a restart.

mod context;
mod controls;
mod data;
mod editor;
mod format;
mod live;
mod page;
mod pokemon;
mod tabs;
#[cfg(test)]
mod tests;

pub use page::SavePage;
