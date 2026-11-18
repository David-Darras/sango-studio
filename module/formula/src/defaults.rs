//! The formulas of the game, written in the language of the page.

/// The formula of Attack, Defense, Sp. Atk, Sp. Def and Speed before the nature.
pub const STAT: &str = "(2 * base + iv + ev / 4) * level / 100 + 5";

/// The formula of the HP: Shedinja (species 292) always has 1 HP.
pub const HP: &str = "if(species == 292, 1, (2 * base + iv + ev / 4) * level / 100 + level + 10)";

/// The statistics, in the order of the file: the name and the formula of the game.
pub const STATS: [(&str, &str); 6] = [
    ("HP", HP),
    ("Attack", STAT),
    ("Defense", STAT),
    ("Sp. Atk", STAT),
    ("Sp. Def", STAT),
    ("Speed", STAT),
];

/// The names that the text file uses for the statistics.
pub const KEYS: [&str; 6] = ["hp", "attack", "defense", "special_attack", "special_defense", "speed"];

/// A shiny Pokémon is one where the test of the game gives a number under this limit, out of 65536.
pub const SHINY_GAME: u32 = 16;

/// The number of values of the test of the shiny chance.
pub const SHINY_OUT_OF: u32 = 65536;

/// The chance of the game, in percent.
pub fn shiny_game_percent() -> f64 {
    f64::from(SHINY_GAME) / f64::from(SHINY_OUT_OF) * 100.0
}
