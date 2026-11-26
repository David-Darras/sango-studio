//! The features and the file `features.ini`.

use std::collections::BTreeMap;

/// A feature of the plugin that can be on or off.
pub struct Feature {
    /// Its name in the file: the plugin knows the same names.
    pub key: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    /// The value of the plugin when the file does not give one.
    pub on: bool,
}

const fn feature(key: &'static str, on: bool, name: &'static str, note: &'static str) -> Feature {
    Feature { key, name, note, on }
}

/// The features by group, in the order of the page.
pub const GROUPS: [(&str, &[Feature]); 5] = [
    ("Start of the game", &[feature("title_screen", true, "Title screen of the plugin", "On: the title screen that the menu of the plugin chooses (Omega Ruby at first). Off: the title screen of the game.")]),
    (
        "Overworld",
        &[
            feature("run_animation", true, "Run animation for every character", "A character that runs and has no run animation walks faster."),
            feature("other_players", true, "Other players on the map", "The other players that are connected show as characters."),
            feature("auto_surf", false, "Surf by walking into water", "The player starts to surf when the player walks into water."),
            feature("nurse_follower", false, "Nurse that follows the player", "Talk to the nurse to heal the party."),
            feature("pokemon_flight", false, "Fly on a Pokémon", "The player flies on the back of a Pokémon. The menu of the plugin chooses the Pokémon."),
            feature("tall_grass", false, "3D tall grass", "Tall grass around the player. It needs the grass models on the SD card."),
            feature("map_marks", false, "Marks on the map", "Walls, water and wild Pokémon show on the map. The menu of the plugin chooses the marks."),
        ],
    ),
    (
        "Maps that you change",
        &[
            feature("map_editor_changes", true, "Changes of the map editor", "Off: the game uses its own tiles."),
            feature("tile_changes", true, "Changes of single tiles", "The tiles that the menu of the plugin saved on the SD card."),
            feature("joined_maps", true, "Joined maps", "The maps that the plugin joins together, and their warps."),
        ],
    ),
    (
        "Battle",
        &[
            feature("trainer_models", true, "Trainer models of X and Y", "The battles show the 3D trainers that the plugin gives to the trainers."),
            feature("shiny_entry", false, "Bigger shiny effect", "More stars and colours when a shiny Pokémon comes out of its Poké Ball."),
        ],
    ),
    (
        "Sound and system",
        &[
            feature("music_files", true, "Music from the SD card", "The music files of the SD card play in place of the ones of the game."),
            feature("music_lock", false, "Music lock", "The music does not change in battles and between maps."),
            feature("summary_editor", false, "Pokémon editor in the Summary", "Press Y in the Summary screen of a Pokémon."),
            feature("performance", false, "Speed and memory on the screen", "The frames each second and the free memory show on the top screen."),
        ],
    ),
];

/// The file on the SD card.
pub const PATH: &str = "features.ini";
const SECTION: &str = "[features]";

/// The features that the file changes, by their key.
pub type Choices = BTreeMap<String, bool>;

pub fn find(key: &str) -> Option<&'static Feature> {
    GROUPS.iter().flat_map(|(_, features)| features.iter()).find(|feature| feature.key == key)
}

/// Reads the file as the plugin does: the lines `key = value` of the section `[features]`, where 1, on, true and yes are on.
pub fn parse(text: &str) -> Choices {
    let mut choices = Choices::new();
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(SECTION);
        } else if inside && !line.starts_with(';') && !line.starts_with('#') {
            if let Some((key, value)) = line.split_once('=') {
                let on = ["1", "on", "true", "yes"].iter().any(|word| value.trim().eq_ignore_ascii_case(word));
                choices.insert(key.trim().to_ascii_lowercase(), on);
            }
        }
    }
    choices
}

/// The text of the file.
pub fn format(choices: &Choices) -> String {
    let mut text = format!("; The features that Sango Studio changes. 1 = on, 0 = off.\n{SECTION}\n");
    for (key, on) in choices {
        text.push_str(&std::format!("{key} = {}\n", u8::from(*on)));
    }
    text
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_file_is_read_back() {
        let choices: Choices = [("title_screen".to_string(), false), ("shiny_entry".to_string(), true)].into();
        assert_eq!(parse(&format(&choices)), choices);
        assert_eq!(parse("[other]\na=1\n[Features]\n; b = 1\n Music_Lock = On \nbad line\n"), [("music_lock".to_string(), true)].into());
        assert!(parse("").is_empty());
    }

    #[test]
    fn each_feature_has_its_own_key() {
        let mut keys: Vec<&str> = GROUPS.iter().flat_map(|(_, features)| features.iter()).map(|feature| feature.key).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count);
        // The values of the plugin that the file is made of: 8 KB at most, and 160 lines.
        assert!(count < 100);
        assert!(find("title_screen").is_some_and(|feature| feature.on));
    }

    /// With `SANGO_PLUGIN` (the folder of the plugin): the plugin knows every key of the page, and the page every key of the plugin.
    #[test]
    fn the_plugin_knows_the_same_features() {
        let Ok(plugin) = std::env::var("SANGO_PLUGIN") else { return };
        let source = std::fs::read_to_string(std::path::Path::new(&plugin).join("lib/src/core/studio_features.cc")).unwrap();
        let mut theirs: Vec<&str> = source.split("SANGO_FEATURE(\"").skip(1).filter_map(|part| part.split('"').next()).collect();
        let mut ours: Vec<&str> = GROUPS.iter().flat_map(|(_, features)| features.iter()).map(|feature| feature.key).collect();
        theirs.sort_unstable();
        ours.sort_unstable();
        assert_eq!(ours, theirs);
    }
}
