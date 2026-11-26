//! The names of the values of the settings of a map, in the order of their numbers.

pub const WEATHERS: [&str; 10] = ["Sunny", "Rain", "Thunderstorm", "Fog", "Volcanic ash", "Sandstorm", "Clouds", "Downpour", "Harsh sunlight", "None"];

/// The window that shows the name of the place when the player comes in.
pub const NAME_WINDOWS: [&str; 3] = ["No window", "Usual window", "Secret Base window"];

/// How the screen fades when the player goes through a door or an exit: the value, its name and what it does. The game looks at
/// the value of the two maps. It has no fade for the value 0, so the list starts at 1.
pub const MAP_CHANGES: [(i64, &str, &str); 6] = [
    (1, "Outdoors", "For towns and routes. Between two of these maps, the screen swipes the way that the player looks."),
    (2, "Cave or closed place", "For caves and closed places (Mauville City too). From outside, a circle closes. Back outside, a circle opens."),
    (3, "Building", "For Pokémon Centers, shops and houses. A Poké Ball shape shows on the way in from outside and on the way back."),
    (4, "Building (diamond shape)", "No map of the game uses it. Like a building, with a diamond in place of the Poké Ball."),
    (5, "Gate", "No map of the game uses it. From outside and back, the screen swipes the way that the player looks."),
    (6, "Dungeon", "No map of the game uses it. From outside, a circle closes. On the way out, the screen goes through white."),
];

pub const BATTLE_BACKGROUNDS: [&str; 93] = [
    "None", "Grassland", "Dark room (factory)", "Cave", "Forest", "Beach", "Mountain", "Room", "City", "Town", "Palace (X and Y)", "Link Battle",
    "Pond or river", "Sea", "Snow cave (X and Y)", "Reflection Cave (X and Y)", "Gym: Rock", "Gym: Fighting", "Gym: Electric", "Gym: Fire",
    "Gym: Normal", "Gym: Flying", "Gym: Psychic", "Gym: Water", "Elite Four: Dark", "Elite Four: Ice", "Elite Four: Ghost", "Elite Four: Dragon",
    "Champion", "Dark room", "World Championships", "World Championships final", "Gym Leader: Rock", "Gym Leader: Fighting",
    "Gym Leader: Electric", "Gym Leader: Fire", "Gym Leader: Normal", "Gym Leader: Flying", "Gym Leader: Psychic", "Gym Leader: Water",
    "Sky Battle", "Water in a cave", "Grey", "Desert", "Volcanic ground", "City beach", "Shallows", "Shallows with mangroves", "Seabed",
    "Rocky cave", "Waterfall in a cave", "Seabed cave", "Waterfall", "Sea cave on the seabed", "Cycling Road", "Ship", "Cemetery", "Sky Pillar",
    "Legend: Groudon", "Legend: Kyogre", "Legend: Rayquaza", "Legendary Pokémon", "Contest hall A: Cool", "Contest hall A: Beauty",
    "Contest hall A: Cute", "Contest hall A: Clever", "Contest hall A: Tough", "Contest hall B: Cool", "Contest hall B: Beauty",
    "Contest hall B: Cute", "Contest hall B: Clever", "Contest hall B: Tough", "Contest hall C: Cool", "Contest hall C: Beauty",
    "Contest hall C: Cute", "Contest hall C: Clever", "Contest hall C: Tough", "Contest hall D: Cool", "Contest hall D: Beauty",
    "Contest hall D: Cute", "Contest hall D: Clever", "Contest hall D: Tough", "Contest opening", "Contest ending", "Team Aqua leader",
    "Team Magma leader", "Top of Sky Pillar", "Legend: Deoxys", "Volcano", "Wally", "Last battle", "Sea cave", "Abandoned ship",
];

/// The musics of the game: the value in the settings of a map, and its name.
pub fn musics() -> Vec<(i64, String)> {
    let mut found: Vec<(i64, String)> = sango_music::ids::STREAMS
        .iter()
        .filter(|(_, id)| id >> 16 == sango_music::ids::GROUP_MUSIC)
        .map(|&(_, id)| {
            let number = (id & 0xFFFF) as usize;
            let name = sango_music::names::MUSIC_NAMES.get(number).filter(|name| !name.is_empty()).copied().unwrap_or("Music");
            (i64::from(id), format!("{name} ({number})"))
        })
        .collect();
    found.sort_unstable();
    found
}
