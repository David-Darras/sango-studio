//! The viewer of the choreographies: a battle with the Pokémon that attacks and its target, the place of the battle,
//! the camera, the models and the particles, played frame after frame as the game plays the file.

mod assets;
mod clip;
mod debris;
mod math;
mod raster;
mod scene;
mod sim;
mod viewer;

pub use assets::{Choice, Library};
pub use raster::Canvas;
pub use scene::{picture, Surroundings, PLACES};
pub use sim::{Run, Setup, State, ATTACKER, TARGET};
pub use viewer::StageViewer;

#[cfg(test)]
mod test;
