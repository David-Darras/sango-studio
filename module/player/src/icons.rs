//! The names of the PSS icons (the files of the archive 108). The files 1 to 76 are the icons that the PSS shows. The files after
//! them are the pieces of the avatars of X and Y (body, face, eyes, hair, hat): the game of this studio keeps them for the players
//! of X and Y.

/// The names of the files 1 to 76.
const NAMES: [&str; 76] = [
    "Player of X and Y, girl (empty)", "Player of X and Y, boy (empty)", "Professor Sycamore", "Diantha", "Wikstrom", "Malva", "Drasna", "Siebold",
    "Viola", "Grant", "Korrina", "Ramos", "Clemont", "Valerie", "Olympia", "Wulfric",
    "Youngster", "Twins (empty)", "Lass", "Lady", "Schoolgirl", "Battle Girl", "Schoolboy", "Rich Boy",
    "Ace Trainer (female)", "Veteran (female, empty)", "Pokémon Ranger (female)", "Ace Trainer (male)", "Pokémon Ranger (male)", "Madame", "Monsieur", "Black Belt",
    "Roughneck", "Fairy Tale Girl", "Shauna", "Tierno", "Trevor", "Brendan", "May", "Icon 40 (empty)",
    "Hiker", "Aroma Lady", "School Kid (male)", "School Kid (female)", "Black Belt (2)", "Battle Girl (2)", "Pokémaniac", "Fairy Tale Girl (2)",
    "Pokéfan (male)", "Pokéfan (female)", "Pokémon Ranger (male, 2)", "Pokémon Ranger (female, 2)", "Swimmer (male)", "Hex Maniac", "Ace Trainer (male, 2)", "Ace Trainer (female, 2)",
    "Street Thug", "Punk Girl", "Expert (male)", "Expert (female)", "Lady (2)", "Rich Boy (2)", "Ninja Boy", "Beauty",
    "Guitarist", "Lass (2)", "Pokémon Breeder (male)", "Pokémon Breeder (female)", "Camper", "Picnicker", "Wally", "Steven",
    "Maxie", "Archie", "Event: Pokémon Center", "Event: gift",
];

/// The first file of the pieces of the avatars of X and Y.
const FIRST_PIECE: usize = NAMES.len() + 1;

/// The name of an icon for the menus: the number of the file, then its name.
pub fn label(file: usize) -> String {
    match file.checked_sub(1).and_then(|place| NAMES.get(place)) {
        Some(name) => format!("{file} · {name}"),
        None => format!("{file} · Avatar piece of X and Y {}", file.saturating_sub(FIRST_PIECE) + 1),
    }
}
