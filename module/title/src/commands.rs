//! The types of steps of a sequence: the size of each one, and the name and the values of the ones that the title screen uses.

/// The size in bytes of the values of each type of step, by its number.
pub const SIZES: [usize; 154] = [
    0, 28, 24, 16, 16, 16, 12, 12, 12, 12, 12, 12, 4, 12, 12, 4, 20, 0, 20, 0, 20, 28, 28, 24, 12, 12, 4, 12, 12, 0, 8, 12, 32, 12, 20, 12,
    28, 28, 32, 48, 20, 20, 36, 0, 0, 16, 12, 0, 8, 8, 8, 12, 12, 4, 12, 4, 20, 0, 24, 8, 8, 4, 8, 12, 12, 4, 12, 4, 20, 32, 0, 20, 12, 12,
    4, 28, 12, 4, 24, 4, 0, 12, 24, 24, 0, 12, 68, 80, 0, 20, 20, 20, 20, 16, 12, 8, 12, 12, 12, 20, 0, 8, 4, 8, 4, 0, 0, 0, 4, 16, 24, 24,
    16, 20, 8, 8, 8, 24, 8, 8, 8, 8, 4, 8, 24, 24, 4, 12, 12, 20, 20, 16, 12, 16, 0, 8, 16, 24, 4, 48, 24, 4, 16, 8, 16, 0, 28, 0, 8, 0, 0,
    0, 0, 4,
];

/// How a value shows on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Bool,
    Float,
    Vec3,
    /// A number of a list: the number and its name.
    Choice(&'static [(i32, &'static str)]),
    /// A value that the page shows and does not change.
    Locked(&'static str),
}

impl Kind {
    /// The number of bytes of the value.
    pub fn size(self) -> usize {
        if self == Kind::Vec3 {
            12
        } else {
            4
        }
    }
}

/// One type of step.
pub struct Command {
    pub number: u16,
    pub name: &'static str,
    /// The values, one after the other from the first byte.
    pub values: &'static [(&'static str, Kind)],
}

const COLORS: &[(i32, &str)] = &[(0, "White"), (1, "Black")];
const SCREENS: &[(i32, &str)] = &[(0, "Both screens"), (1, "Top screen"), (2, "Bottom screen")];
const MOTION_SETS: &[(i32, &str)] = &[(0, "Battle"), (1, "Pokémon-Amie"), (2, "Menus"), (3, "File of the sequence")];
const MOTIONS: &[(i32, &str)] = &[(0, "Waiting"), (1, "Contact attack"), (2, "Distance attack"), (3, "Showing off"), (4, "Hit"), (5, "Special"), (6, "Fainting")];

use Kind::{Bool, Choice, Float, Int, Locked, Vec3};

/// The types of steps that have names. The other ones show their bytes as numbers.
pub const COMMANDS: &[Command] = &[
    Command { number: 1, name: "Model: show", values: &[("File", Int), ("Load before", Bool), ("Draw group", Int), ("Outline", Bool), ("Stays", Bool), ("Map value A", Int), ("Map value B", Int)] },
    Command { number: 7, name: "Model: material animation", values: &[("File", Int), ("Load before", Bool), ("Repeat", Bool)] },
    Command { number: 8, name: "Model: joint animation", values: &[("File", Int), ("Load before", Bool), ("Repeat", Bool)] },
    Command { number: 10, name: "Model: place", values: &[("Position", Vec3)] },
    Command { number: 13, name: "Model: turn", values: &[("Degrees", Vec3)] },
    Command { number: 14, name: "Model: size", values: &[("Size", Vec3)] },
    Command { number: 15, name: "Model: visible", values: &[("Visible", Bool)] },
    Command { number: 21, name: "Pokémon: show", values: &[("Species", Int), ("Form", Int), ("Gender", Int), ("Load before", Bool), ("Motions", Choice(MOTION_SETS)), ("Draw group", Int), ("Shader", Bool)] },
    Command { number: 23, name: "Pokémon: show (video memory)", values: &[("Species", Int), ("Form", Int), ("Gender", Int), ("Motions", Choice(MOTION_SETS)), ("Motion file", Int), ("Draw group", Int)] },
    Command { number: 24, name: "Pokémon: place", values: &[("Position", Vec3)] },
    Command { number: 27, name: "Pokémon: turn", values: &[("Degrees", Vec3)] },
    Command { number: 28, name: "Pokémon: size", values: &[("Size", Vec3)] },
    Command { number: 29, name: "Pokémon: stand at its height", values: &[] },
    Command { number: 30, name: "Pokémon: visible", values: &[("Visible", Bool), ("Remove from the scene", Bool)] },
    Command { number: 32, name: "Pokémon: colour change", values: &[("Start red", Int), ("Start green", Int), ("Start blue", Int), ("Start strength", Int), ("End red", Int), ("End green", Int), ("End blue", Int), ("End strength", Int)] },
    Command { number: 33, name: "Pokémon: motion", values: &[("Motion", Choice(MOTIONS)), ("Repeat", Bool), ("Fixed", Bool)] },
    Command { number: 34, name: "Pokémon: motion from files", values: &[("Motion", Int), ("File 1", Int), ("File 2", Int), ("File 3", Int), ("Repeat", Bool)] },
    Command { number: 46, name: "Pokémon: light direction", values: &[("Direction", Vec3)] },
    Command { number: 71, name: "Particles: start", values: &[("File", Int), ("Screen", Int), ("Load before", Bool), ("Draw group", Int), ("Stays", Bool)] },
    Command { number: 72, name: "Particles: place", values: &[("Position", Vec3)] },
    Command { number: 76, name: "Particles: size", values: &[("Size", Vec3)] },
    Command { number: 77, name: "Particles: visible", values: &[("Visible", Bool)] },
    Command { number: 79, name: "Particles: stop", values: &[("Speed", Float)] },
    Command { number: 81, name: "Camera: animation", values: &[("File", Int), ("Load before", Bool), ("Repeat", Bool)] },
    Command { number: 85, name: "Light: make", values: &[("Light file", Int), ("Scene file", Int), ("Load before", Bool)] },
    Command { number: 89, name: "Logo and menus: show", values: &[("File", Int), ("Layout", Int), ("Part", Int), ("Animations", Int), ("In front of the 3D", Bool)] },
    Command { number: 95, name: "Logo and menus: animation", values: &[("Layout", Int), ("Animation", Int)] },
    Command { number: 101, name: "Screen: fade out", values: &[("Colour", Choice(COLORS)), ("Screens", Choice(SCREENS))] },
    Command { number: 102, name: "Screen: fade in", values: &[("Screens", Choice(SCREENS))] },
    Command { number: 103, name: "Screen: fade in from a colour", values: &[("Colour", Choice(COLORS)), ("Screens", Choice(SCREENS))] },
    Command { number: 112, name: "Music: start", values: &[("Music", Locked("The game waits for this music before it takes a button. Change the sound of this music on the Music page.")), ("Fade in", Int), ("Fade out", Int), ("Does not stop", Bool)] },
    Command { number: 116, name: "Sound effect", values: &[("Sound", Int), ("Stop with the sequence", Bool)] },
    Command { number: 135, name: "3D: depth", values: &[("Depth level", Float), ("Depth range", Float)] },
];

pub const MODEL_SHOW: u16 = 1;
pub const POKEMON_SHOW: u16 = 21;
pub const POKEMON_SHOW_VIDEO: u16 = 23;
pub const POKEMON_PLACE: u16 = 24;
pub const POKEMON_SIZE: u16 = 28;
pub const POKEMON_MOTION: u16 = 33;
pub const POKEMON_MOTION_FILES: u16 = 34;
pub const LAYOUT_SHOW: u16 = 89;

pub fn command(number: u16) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.number == number)
}

/// The condition of a step: when it plays.
pub const CONDITIONS: [(u16, &str); 8] =
    [(0, "Always"), (4, "Japanese"), (5, "English"), (6, "French"), (7, "Italian"), (8, "German"), (9, "Spanish"), (10, "Korean")];

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_values_fit_in_their_step() {
        for command in COMMANDS {
            let used: usize = command.values.iter().map(|(_, kind)| kind.size()).sum();
            assert!(used <= SIZES[command.number as usize], "{}: {used} bytes", command.name);
        }
    }
}
