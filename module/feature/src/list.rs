//! The settings of the plugin and the file `features.ini`.
//!
//! The values here are the ones of the game: without a file, the plugin changes nothing. The plugin has the same keys in
//! `lib/src/core/studio_features.cc` (a test compares them).

use std::collections::BTreeMap;

/// The kind of a value, and the value that the plugin has without a line in the file.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Switch(bool),
    Whole { default: i32, min: i32, max: i32 },
    Decimal { default: f32, min: f32, max: f32, step: f32 },
    /// A number of a list: the number is the place of the name.
    Choice { default: i32, names: &'static [&'static str] },
    /// RRGGBB.
    Color(u32),
    /// The number of a species.
    Species(u16),
}

/// A value of the file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Switch(bool),
    Whole(i32),
    Decimal(f32),
    Color(u32),
}

impl Kind {
    pub fn default_value(self) -> Value {
        match self {
            Kind::Switch(on) => Value::Switch(on),
            Kind::Whole { default, .. } | Kind::Choice { default, .. } => Value::Whole(default),
            Kind::Species(species) => Value::Whole(i32::from(species)),
            Kind::Decimal { default, .. } => Value::Decimal(default),
            Kind::Color(color) => Value::Color(color),
        }
    }

    /// Reads the text of a line. `None` when it is not a value of this kind.
    pub fn parse(self, text: &str) -> Option<Value> {
        let text = text.trim();
        match self {
            Kind::Switch(_) => Some(Value::Switch(["1", "on", "true", "yes"].iter().any(|word| text.eq_ignore_ascii_case(word)))),
            Kind::Whole { min, max, .. } => text.parse().ok().filter(|v| (min..=max).contains(v)).map(Value::Whole),
            Kind::Choice { names, .. } => text.parse().ok().filter(|&v: &i32| v >= 0 && (v as usize) < names.len()).map(Value::Whole),
            Kind::Species(_) => text.parse().ok().filter(|v| (1..=2000).contains(v)).map(Value::Whole),
            Kind::Decimal { min, max, .. } => text.parse().ok().filter(|v: &f32| (min..=max).contains(v)).map(Value::Decimal),
            Kind::Color(_) => (text.len() == 6).then(|| u32::from_str_radix(text, 16).ok()).flatten().map(Value::Color),
        }
    }
}

impl Value {
    /// The text of the line, as the plugin reads it.
    pub fn text(self) -> String {
        match self {
            Value::Switch(on) => u8::from(on).to_string(),
            Value::Whole(value) => value.to_string(),
            Value::Decimal(value) => format!("{value}"),
            Value::Color(color) => format!("{color:06X}"),
        }
    }
}

/// One setting of the plugin.
pub struct Setting {
    /// The place of its tab in [`TABS`].
    pub tab: usize,
    pub group: &'static str,
    /// Its name in the file: the plugin knows the same names.
    pub key: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub kind: Kind,
}

const fn setting(tab: usize, group: &'static str, key: &'static str, name: &'static str, note: &'static str, kind: Kind) -> Setting {
    Setting { tab, group, key, name, note, kind }
}

/// The videos of the title screen, by their number.
pub const VIDEOS: [&str; 87] = ["Video 0", "Video 1", "Title Omega Ruby", "Title Alpha Sapphire", "Truck Look At Pokedex", "Truck Show Pokemon", "Truck Open Doors", "Truck Jump Outside", "Video 8", "Boat", "Video 10", "Sky Up", "Sky Down", "Flying With Latios And Steven", "Flying With Latias And Steven", "Green Light", "Video 16", "Video 17", "Video 18", "Video 19", "Groudon Walking", "Kyogre Diving", "Video 22", "Video 23", "Video 24", "Video 25", "Video 26", "Video 27", "Brown Pixels", "Rayquaza Fly Omega Ruby", "Rayquaza Fly Alpha Sapphire", "Sky Pillar", "Diving Go Down", "Diving Go Up", "Egg Hatching", "Waterfall Down", "Waterfall Up", "Execute Field Move", "Execute Field Move 2", "Bulbasaur And Window", "Video 40", "Video 41", "Video 42", "Bulbasaur And Window 2", "Video 44", "Video 45", "Window", "Fly", "Badge 1", "Badge 2", "Badge 3", "Badge 4", "Badge 5", "Badge 6", "Badge 7", "Badge 8", "Use Hm Bugged", "Mega Latios Fly", "Flash 1", "Mega Latios Land", "Flash 2", "Evolution", "Evolution 2", "Evolution 3", "Receive Pokemon", "Receive Pokemon 2", "Send Pokemon", "Pokemon Exchange Red", "Pokemon Exchange Blue", "Black Lines", "Black Lines 2", "Video 71", "Video 72", "Video 73", "Video 74", "Video 75", "Video 76", "Video 77", "Video 78", "Video 79", "Video 80", "Video 81", "Video 82", "Video 83", "Small Window", "Primo Groudon", "Primo Kyogre"];

/// The colour filters of the Pokémon.
pub const POKEMON_FILTERS: [&str; 20] = ["None", "GRB", "BGR", "RBG", "GBR", "BRG", "Inverted RGB", "Inverted GRB", "Inverted BGR", "Darker", "Brighter", "All Black", "All White", "Gray", "Sepia", "Light Only", "Specular Only", "Rim Light", "Liquid Metal", "Ghost"];

/// The models that the player can take in battle.
pub const PLAYER_MODELS: [&str; 29] = ["Game Model", "Serena", "Calem", "Malva", "Holocaster", "May", "Brendan", "Wally", "Wally Awake", "Steven", "Maxie", "Archie", "Zinnia", "Shauna", "Tierno", "Trevor", "Sycamore", "Lysandre", "Flare Admin Male", "Flare Admin Female", "Aliana", "Xerosic", "Flare Grunt Male", "Flare Grunt Female", "Celosia", "Bryony", "Mable", "AZ", "Lysandre Final"];

/// The chances of a shiny Pokémon.
pub const SHINY_RATES: [&str; 22] = ["Chance of the game", "1/1", "1/2", "1/4", "1/8", "1/16", "1/32", "1/64", "1/128", "1/256", "1/512", "1/1024", "1/2048", "1/4096", "1/8192", "1/16384", "1/32768", "1/65536", "1/131072", "1/262144", "1/524288", "1/1048576"];

/// The two players.
pub const GENDERS: [&str; 2] = ["Brendan", "May"];

/// The length of an animation.
pub const LENGTHS: [&str; 3] = ["The game chooses", "Long", "Short"];

/// The kinds of battle.
pub const FORMATS: [&str; 5] = ["Single", "Double", "Triple", "Rotation", "Horde"];

/// The frames for a tile, by their place in the list of the plugin.
pub const FRAMES: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "12"];

/// The colours of the rain.
pub const RAINS: [&str; 3] = ["Normal", "Toxic (purple)", "Radioactive (green)"];

/// The kinds of tall grass.
pub const GRASSES: [&str; 4] = ["All", "Green", "Ferns", "Ash"];

/// The frames each second.
pub const FRAME_RATES: [&str; 3] = ["The game chooses", "30", "60"];

/// The tabs of the page.
pub const TABS: [&str; 8] = ["Graphics", "Start", "Battle", "Overworld", "Weather and time", "Maps", "Pokémon", "System"];

/// Every setting, in the order of the page.
pub const SETTINGS: [Setting; 111] = [
    setting(0, "Outlines", "graphics.outlines", "Outlines", "The dark line around the 3D models. The game has it.", Kind::Switch(true)),
    setting(0, "Outlines", "graphics.outline_width", "Width of the outlines", "0: the width of the game.", Kind::Decimal { default: 0.0, min: 0.0, max: 10.0, step: 0.1 }),
    setting(0, "Outlines", "graphics.use_outline_color", "Another colour for the outlines", "", Kind::Switch(false)),
    setting(0, "Outlines", "graphics.outline_color", "Colour of the outlines", "It shows with Another colour for the outlines.", Kind::Color(0x000000)),
    setting(0, "Lights", "graphics.use_ambient_light", "Another ambient light", "The light that comes from everywhere.", Kind::Switch(false)),
    setting(0, "Lights", "graphics.ambient_color", "Colour of the ambient light", "It shows with Another ambient light.", Kind::Color(0xFFFFFF)),
    setting(0, "Lights", "graphics.use_diffuse_light", "Another diffuse light", "The light that comes from one side.", Kind::Switch(false)),
    setting(0, "Lights", "graphics.diffuse_color", "Colour of the diffuse light", "It shows with Another diffuse light.", Kind::Color(0xFFFFFF)),
    setting(0, "Pokémon", "graphics.pokemon_filter", "Colour filter of the Pokémon", "In battle and in the menus.", Kind::Choice { default: 0, names: &POKEMON_FILTERS }),
    setting(1, "Title screen", "title.top_video", "Video of the top screen", "Title Alpha Sapphire is the one of the game. The page Title Screen changes the videos of both title screens.", Kind::Choice { default: 3, names: &VIDEOS }),
    setting(1, "Title screen", "title.bottom_video", "Video of the bottom screen", "Primo Kyogre is the one of the game; Primo Groudon is the one of Omega Ruby.", Kind::Choice { default: 86, names: &VIDEOS }),
    setting(1, "Title screen", "title.cry_species", "Cry when the player presses a button", "Kyogre in the game.", Kind::Species(382)),
    setting(1, "Title screen", "title.cry_volume", "Volume of the cry", "", Kind::Decimal { default: 0.7, min: 0.0, max: 1.0, step: 0.1 }),
    setting(1, "Title screen", "title.no_delay", "No wait before the title screen", "", Kind::Switch(false)),
    setting(1, "Title screen", "title.no_shadow", "No shadow on the title screen", "", Kind::Switch(false)),
    setting(1, "Title screen", "title.skip_to_frame", "Start at the frame", "0: the start.", Kind::Whole { default: 0, min: 0, max: 3000 }),
    setting(1, "New game", "new_game.skip_intro", "Skip the introduction", "", Kind::Switch(false)),
    setting(1, "New game", "new_game.player_gender", "Player of a new game", "With Skip the introduction.", Kind::Choice { default: 0, names: &GENDERS }),
    setting(2, "Rewards", "battle.exp_multiplier", "Experience", "1: the experience of the game.", Kind::Decimal { default: 1.0, min: 0.0, max: 20.0, step: 0.5 }),
    setting(2, "Rewards", "battle.catch_multiplier", "Catch rate", "", Kind::Decimal { default: 1.0, min: 0.0, max: 20.0, step: 0.5 }),
    setting(2, "Rewards", "battle.money_multiplier", "Money", "", Kind::Decimal { default: 1.0, min: 0.0, max: 20.0, step: 0.5 }),
    setting(2, "Rules", "battle.can_use_items", "The player can use items", "", Kind::Switch(true)),
    setting(2, "Rules", "battle.one_mega_evolution", "One Mega Evolution in each battle", "", Kind::Switch(true)),
    setting(2, "Rules", "battle.unlimited_mega_evolution", "Every Mega Evolution is accepted", "Only when One Mega Evolution in each battle is off.", Kind::Switch(true)),
    setting(2, "Rules", "battle.same_catch_rate", "Same catch rate for every Ball", "", Kind::Switch(false)),
    setting(2, "Rules", "battle.inverse_stats", "Physical and special stats swapped", "", Kind::Switch(false)),
    setting(2, "Rules", "battle.metronome_only", "Every move is Metronome", "", Kind::Switch(false)),
    setting(2, "Rules", "battle.shared_team_hp", "The team faints together", "", Kind::Switch(false)),
    setting(2, "Screen", "battle.type_helper", "Effectiveness of the moves", "", Kind::Switch(false)),
    setting(2, "Screen", "battle.hp_gradient", "Colour gradient of the HP bar", "", Kind::Switch(false)),
    setting(2, "Screen", "battle.real_sizes", "Pokémon at their real sizes", "", Kind::Switch(false)),
    setting(2, "Screen", "battle.no_shader", "No shader in battle", "", Kind::Switch(false)),
    setting(2, "Screen", "battle.fixed_camera", "Camera that does not move", "No split view and no camera animation.", Kind::Switch(false)),
    setting(2, "Animations", "battle.encounter_animation", "Start of a battle", "", Kind::Choice { default: 0, names: &LENGTHS }),
    setting(2, "Animations", "battle.mega_evolve_animation", "Mega Evolution", "", Kind::Choice { default: 0, names: &LENGTHS }),
    setting(2, "Animations", "battle.enemy_view", "Start seen from the opponent", "", Kind::Switch(false)),
    setting(2, "Animations", "battle.trainer_animation", "Trainers move at the start", "", Kind::Switch(true)),
    setting(2, "Animations", "battle.ball_animation", "Poké Ball animation", "", Kind::Switch(true)),
    setting(2, "Animations", "battle.fade_in", "Fade at the start", "", Kind::Switch(false)),
    setting(2, "Animations", "battle.shiny_animation", "Stars of a shiny Pokémon", "", Kind::Switch(true)),
    setting(2, "Animations", "battle.shiny_entry", "Bigger shiny effect", "", Kind::Switch(false)),
    setting(2, "Trainers", "battle.trainer_models", "Trainer models of X and Y", "It needs the files of the models on the SD card.", Kind::Switch(false)),
    setting(2, "Trainers", "battle.player_model", "Model of the player", "", Kind::Choice { default: 0, names: &PLAYER_MODELS }),
    setting(2, "Wild battles", "wild.enabled", "Choose the wild Pokémon", "Every wild battle uses the values below.", Kind::Switch(false)),
    setting(2, "Wild battles", "wild.species", "Wild Pokémon", "", Kind::Species(382)),
    setting(2, "Wild battles", "wild.form", "Its form", "", Kind::Whole { default: 1, min: 0, max: 31 }),
    setting(2, "Wild battles", "wild.format", "Kind of battle", "", Kind::Choice { default: 0, names: &FORMATS }),
    setting(2, "Wild battles", "wild.inverse_teams", "Swap the two teams", "", Kind::Switch(false)),
    setting(3, "Walk", "overworld.run_animation", "Run animation for every character", "A character that runs and has no run animation walks faster.", Kind::Switch(false)),
    setting(3, "Walk", "overworld.auto_surf", "Surf by walking into water", "", Kind::Switch(false)),
    setting(3, "Walk", "overworld.fly_anywhere", "Fly from everywhere", "Inside and in the caves too.", Kind::Switch(false)),
    setting(3, "Walk", "overworld.no_collision", "Walk through walls", "", Kind::Switch(false)),
    setting(3, "Walk", "overworld.noclip", "Noclip", "Walk through walls; L and R go down and up.", Kind::Switch(false)),
    setting(3, "Walk", "overworld.walk_frames", "Frames for a tile when walking", "Fewer frames is faster. The game uses 7.", Kind::Choice { default: 6, names: &FRAMES }),
    setting(3, "Walk", "overworld.run_frames", "Frames for a tile when running", "The game uses 4.", Kind::Choice { default: 3, names: &FRAMES }),
    setting(3, "People", "overworld.following_pokemon", "Pokémon that follows the player", "", Kind::Switch(false)),
    setting(3, "People", "overworld.nurse_follower", "Nurse that follows the player", "", Kind::Switch(false)),
    setting(3, "People", "overworld.swarm", "People that circle the player", "", Kind::Switch(false)),
    setting(3, "People", "overworld.other_players", "Other players on the map", "", Kind::Switch(false)),
    setting(3, "People", "overworld.example_characters", "Example characters of the plugin", "", Kind::Switch(false)),
    setting(3, "Fly on a Pokémon", "flight.enabled", "Fly on a Pokémon", "", Kind::Switch(false)),
    setting(3, "Fly on a Pokémon", "flight.species", "Flying Pokémon", "", Kind::Species(398)),
    setting(3, "Fly on a Pokémon", "flight.size", "Its size", "", Kind::Decimal { default: 0.5, min: 0.1, max: 3.0, step: 0.1 }),
    setting(3, "Fly on a Pokémon", "flight.speed", "Speed", "", Kind::Decimal { default: 1.0, min: 0.25, max: 5.0, step: 0.25 }),
    setting(3, "Fly on a Pokémon", "flight.climb_speed", "Speed up and down", "", Kind::Decimal { default: 2.5, min: 0.5, max: 10.0, step: 0.5 }),
    setting(3, "Ride a Pokémon", "ride.enabled", "Ride a Pokémon", "", Kind::Switch(false)),
    setting(3, "Ride a Pokémon", "ride.species", "Riding Pokémon", "", Kind::Species(243)),
    setting(3, "Ride a Pokémon", "ride.size", "Its size", "", Kind::Decimal { default: 0.25, min: 0.05, max: 2.0, step: 0.05 }),
    setting(3, "Ride a Pokémon", "ride.speed", "Speed", "", Kind::Decimal { default: 1.5, min: 0.25, max: 5.0, step: 0.25 }),
    setting(3, "Ride a Pokémon", "ride.height", "Height of the seat", "", Kind::Decimal { default: -16.0, min: -100.0, max: 100.0, step: 2.0 }),
    setting(3, "Ride a Pokémon", "ride.spread", "Legs to the sides", "", Kind::Decimal { default: 2.0, min: 0.0, max: 10.0, step: 0.1 }),
    setting(3, "Ride a Pokémon", "ride.back", "Seat to the back", "", Kind::Decimal { default: 46.0, min: -100.0, max: 200.0, step: 2.0 }),
    setting(4, "Weather", "weather.keep_indoors", "Weather inside the buildings", "", Kind::Switch(false)),
    setting(4, "Weather", "weather.ignore_zone", "Maps do not change the weather", "", Kind::Switch(false)),
    setting(4, "Weather", "weather.rain_color", "Colour of the rain", "", Kind::Choice { default: 0, names: &RAINS }),
    setting(4, "Time", "time.enabled", "Choose the time of the day", "", Kind::Switch(false)),
    setting(4, "Time", "time.hour", "Hour", "", Kind::Whole { default: 12, min: 0, max: 23 }),
    setting(4, "Time", "time.minute", "Minute", "", Kind::Whole { default: 0, min: 0, max: 59 }),
    setting(4, "Tall grass", "grass.enabled", "3D tall grass", "It needs the grass models on the SD card.", Kind::Switch(false)),
    setting(4, "Tall grass", "grass.types", "Kinds of grass", "", Kind::Choice { default: 0, names: &GRASSES }),
    setting(4, "Tall grass", "grass.radius", "Distance around the player", "", Kind::Whole { default: 5, min: 1, max: 100 }),
    setting(4, "Tall grass", "grass.density", "Density (%)", "", Kind::Whole { default: 60, min: 0, max: 100 }),
    setting(5, "Your changes", "maps.editor_changes", "Changes of the map editor", "The changes that you made. Without a change, the game stays the same.", Kind::Switch(true)),
    setting(5, "Your changes", "maps.tile_changes", "Changes of single tiles", "The tiles that the menu of the plugin saved.", Kind::Switch(true)),
    setting(5, "Your changes", "maps.joined_maps", "Joined maps", "The maps that you joined, and their warps.", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.enabled", "Marks on the map", "They show on the top screen when the menu is closed.", Kind::Switch(false)),
    setting(5, "Marks on the map", "marks.tiles", "Walls, water and wild Pokémon", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.signs", "Signs", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.triggers", "Triggers", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.warps", "Warps", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.hidden_items", "Hidden items", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.characters", "Characters", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.trainer_sight", "Sight of the trainers", "", Kind::Switch(true)),
    setting(5, "Marks on the map", "marks.legend", "Legend of the colours", "", Kind::Switch(false)),
    setting(5, "Marks on the map", "marks.radius", "Tiles around the player", "", Kind::Whole { default: 5, min: 1, max: 8 }),
    setting(5, "Marks on the map", "marks.opacity", "Opacity (%)", "", Kind::Whole { default: 80, min: 10, max: 100 }),
    setting(6, "Pokémon", "pokemon.shiny_rate", "Shiny chance", "", Kind::Choice { default: 0, names: &SHINY_RATES }),
    setting(6, "Pokémon", "pokemon.summary_editor", "Pokémon editor in the Summary", "Press Y in the Summary screen.", Kind::Switch(false)),
    setting(6, "Pokémon", "pokemon.no_ev_limit", "No limit of the base points", "", Kind::Switch(false)),
    setting(6, "Pokémon", "pokemon.mega_mime_jr", "Mega Evolution of Mime Jr.", "With the Life Orb. It needs its model.", Kind::Switch(false)),
    setting(6, "Day Care", "pokemon.instant_egg_hatch", "Eggs hatch at once", "", Kind::Switch(false)),
    setting(6, "Day Care", "pokemon.instant_max_exp", "Day Care gives the highest level", "", Kind::Switch(false)),
    setting(6, "Random", "pokemon.random_gifts", "Random gift Pokémon", "", Kind::Switch(false)),
    setting(6, "Random", "pokemon.random_trades", "Random trades", "", Kind::Switch(false)),
    setting(6, "Random", "pokemon.random_encounters", "Random special encounters", "", Kind::Switch(false)),
    setting(7, "Speed", "system.game_speed", "Speed of the game", "1: the speed of the game. 2 to 5 faster, -2 to -5 slower.", Kind::Whole { default: 1, min: -5, max: 5 }),
    setting(7, "Speed", "system.frame_rate", "Frames each second", "At 60, some animations and scripts run too fast.", Kind::Choice { default: 0, names: &FRAME_RATES }),
    setting(7, "Sound", "system.music_files", "Music from the SD card", "The music files that you put on the SD card. Without a file, the game stays the same.", Kind::Switch(true)),
    setting(7, "Sound", "system.music_lock", "Music lock", "The music does not change in battles and between maps.", Kind::Switch(false)),
    setting(7, "System", "system.free_keyboard", "Free keyboard", "L and R turn the pages of all the characters; the keyboard takes every word.", Kind::Switch(false)),
    setting(7, "System", "system.performance", "Speed and memory on the screen", "", Kind::Switch(false)),
];

/// The file on the SD card.
pub const PATH: &str = "features.ini";
const SECTION: &str = "[features]";

/// The settings that the file changes, by their key.
pub type Choices = BTreeMap<String, Value>;

pub fn find(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|setting| setting.key == key)
}

/// Reads the lines `key = value` of the section `[features]`. A line of an unknown key, with a wrong value or with the value of the
/// plugin is left out. Gives also the number of lines that are unknown or wrong.
pub fn parse(text: &str) -> (Choices, usize) {
    let mut choices = Choices::new();
    let (mut inside, mut wrong) = (false, 0);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(SECTION);
        } else if inside && !line.is_empty() && !line.starts_with(';') && !line.starts_with('#') {
            let read = line.split_once('=').and_then(|(key, value)| {
                let setting = find(&key.trim().to_ascii_lowercase())?;
                Some((setting, setting.kind.parse(value)?))
            });
            match read {
                Some((setting, value)) if value != setting.kind.default_value() => {
                    choices.insert(setting.key.to_string(), value);
                }
                Some(_) => {}
                None => wrong += 1,
            }
        }
    }
    (choices, wrong)
}

/// The text of the file.
pub fn format(choices: &Choices) -> String {
    let mut text = format!("; The settings of the plugin that Sango Studio changes. 1 = on, 0 = off.\n{SECTION}\n");
    for (key, value) in choices {
        text.push_str(&std::format!("{key} = {}\n", value.text()));
    }
    text
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_file_is_read_back() {
        let choices: Choices = [
            ("battle.hp_gradient".to_string(), Value::Switch(true)),
            ("flight.speed".to_string(), Value::Decimal(1.5)),
            ("graphics.outline_color".to_string(), Value::Color(0x00FF80)),
            ("title.top_video".to_string(), Value::Whole(2)),
        ]
        .into();
        assert_eq!(parse(&format(&choices)), (choices, 0));
        // Unknown keys and wrong values are counted; the values of the plugin are left out.
        let (read, wrong) = parse("[other]\na = 1\n[Features]\n; x = 1\nunknown = 1\ntime.hour = 99\ntime.minute = 0\n Time.Enabled = On \n");
        assert_eq!((read.len(), wrong), (1, 2));
        assert_eq!(read["time.enabled"], Value::Switch(true));
    }

    #[test]
    fn each_setting_has_its_own_key_and_a_right_value() {
        let mut keys: Vec<&str> = SETTINGS.iter().map(|setting| setting.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count);
        // The file of the plugin has 6 KB and 160 values at most.
        assert!(count < 160);
        for setting in &SETTINGS {
            assert!(setting.tab < TABS.len(), "{}", setting.key);
            let value = setting.kind.default_value();
            assert_eq!(setting.kind.parse(&value.text()), Some(value), "{}", setting.key);
        }
        // A file with every setting changed still fits in the buffer of the plugin.
        let longest: usize = SETTINGS.iter().map(|setting| setting.key.len() + 12).sum();
        assert!(longest < 6000, "{longest} bytes");
        // Every feature is off, or the value of the game.
        assert!(matches!(find("battle.hp_gradient").unwrap().kind, Kind::Switch(false)));
        assert!(matches!(find("graphics.outlines").unwrap().kind, Kind::Switch(true)));
    }

    /// With `SANGO_PLUGIN` (the folder of the plugin): the plugin knows every key of the page, and the page every key of the plugin.
    #[test]
    fn the_plugin_knows_the_same_settings() {
        let Ok(plugin) = std::env::var("SANGO_PLUGIN") else { return };
        let source = std::fs::read_to_string(std::path::Path::new(&plugin).join("lib/src/core/studio_features.cc")).unwrap();
        let keys_after = |mark: &str| source.split(mark).skip(1).filter_map(|part| part.split('"').next()).map(str::to_string).collect::<Vec<_>>();
        let mut theirs = keys_after("SANGO_FEATURE(\"");
        theirs.extend(keys_after("ApplyCheatCode(ini, \""));
        let mut ours: Vec<String> = SETTINGS.iter().map(|setting| setting.key.to_string()).collect();
        theirs.sort_unstable();
        ours.sort_unstable();
        assert_eq!(ours, theirs);
    }
}
