//! The lists of files that the plugin reads.
//!
//! The plugin replaces a file of the game by its number: a folder of the studio that holds such files (`species/icon`, `species/cry`,
//! `species/level_up_moves`, `species/egg_moves`, `species/evolution`, `leveling_rate`, `game_text/<language>`, `layout/<archive>`) has the files named by
//! their number (`98.bin`) and a list `index.bin` (the number of files, then the number and the size of each, sorted by number). The
//! studio writes the list again each time that it writes or removes a file of such a folder (see [`SdCard::write`]).

use crate::text::LANGUAGES;
use crate::SdCard;

/// The folders that have a list, and the number of the archive of the game that each one replaces.
pub fn folders() -> Vec<(String, usize)> {
    let mut found = vec![
        ("species/icon".to_string(), 91),
        ("species/cry".to_string(), 100),
        ("species/level_up_moves".to_string(), 191),
        ("species/egg_moves".to_string(), 190),
        ("species/evolution".to_string(), 192),
        ("species/mega_evolution".to_string(), 193),
        ("leveling_rate".to_string(), 194),
        ("trainer/data".to_string(), 36),
        ("trainer/class".to_string(), 37),
        ("trainer/pokemon".to_string(), 38),
        ("trainer/picture".to_string(), 160),
        ("trainer/model".to_string(), 133),
        ("choreography".to_string(), 34),
        ("effect".to_string(), 31),
        ("script/field".to_string(), 29),
        ("script/ai".to_string(), 7),
        ("script/amie".to_string(), 89),
        ("dexnav".to_string(), 248),
        ("overworld/model".to_string(), 21),
        ("font".to_string(), 167),
        ("item/icon".to_string(), 92),
        ("battle/background".to_string(), 136),
    ];
    found.extend(LANGUAGES.iter().map(|language| (format!("game_text/{}", language.folder), language.archive)));
    found
}

/// The folder with a list that a path is in, when it is.
pub fn folder_of(relative: &str) -> Option<String> {
    let (folder, file) = relative.rsplit_once('/')?;
    if file == "index.bin" {
        return None;
    }
    // The layouts of the menus: one folder for each archive, named by its number (`layout/61`).
    if let Some(number) = folder.strip_prefix("layout/") {
        return (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then(|| folder.to_string());
    }
    folders().into_iter().map(|(name, _)| name).find(|name| name == folder)
}

/// The bytes of a list: the number of files, then the number and the size of each file. The files must be sorted by number.
pub fn index_bytes(files: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = (files.len() as u32).to_le_bytes().to_vec();
    for (number, size) in files {
        bytes.extend_from_slice(&number.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
    }
    bytes
}

/// The files of a list: the number and the size of each.
pub fn parse_index(bytes: &[u8]) -> Result<Vec<(u32, u32)>, String> {
    let count = crate::bytes::u32_at(bytes, 0)? as usize;
    (0..count).map(|i| Ok((crate::bytes::u32_at(bytes, 4 + 8 * i)?, crate::bytes::u32_at(bytes, 8 + 8 * i)?))).collect()
}

/// Writes the list of the files of a folder.
pub fn write_index(sd: &SdCard, folder: &str) -> Result<(), String> {
    let mut files: Vec<(u32, u32)> = Vec::new();
    for found in sd.files_in(folder) {
        let Some(number) = found.rsplit('/').next().and_then(|name| name.strip_suffix(".bin")).and_then(|n| n.parse::<u32>().ok()) else { continue };
        let size = sd.read(&found).map_or(0, |bytes| bytes.len() as u32);
        files.push((number, size));
    }
    // A folder without a file has no list.
    if files.is_empty() {
        return sd.remove(&format!("{folder}/index.bin")).map(|_| ());
    }
    files.sort_unstable();
    sd.write(&format!("{folder}/index.bin"), &index_bytes(&files)).map(|_| ())
}

/// After a write or a removal: writes the list of the folder of the path, when it has one.
pub fn update(sd: &SdCard, relative: &str) -> Result<(), String> {
    match folder_of(relative) {
        Some(folder) => write_index(sd, &folder),
        None => Ok(()),
    }
}

/// Removes all the files of a folder with a list, and the list.
pub fn clear(sd: &SdCard, folder: &str) -> Result<(), String> {
    for found in sd.files_in(folder) {
        sd.remove(&found)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_folders_with_a_list_are_found() {
        assert_eq!(folder_of("game_text/en/98.bin").as_deref(), Some("game_text/en"));
        assert_eq!(folder_of("species/cry/3776.bin").as_deref(), Some("species/cry"));
        assert_eq!(folder_of("leveling_rate/3.bin").as_deref(), Some("leveling_rate"));
        assert_eq!(folder_of("layout/61/0.bin").as_deref(), Some("layout/61"));
        assert_eq!(folder_of("layout/61/index.bin"), None);
        assert_eq!(folder_of("layout/other/0.bin"), None);
        assert_eq!(folder_of("species/data/25_0.bin"), None);
        assert_eq!(folder_of("species/icon/index.bin"), None);
    }

    #[test]
    fn a_change_of_a_file_writes_the_list_of_its_folder() {
        let outer = std::env::temp_dir().join(format!("sango_file_lists_{}", std::process::id()));
        std::fs::create_dir_all(&outer).unwrap();
        let sd = SdCard::open(outer.to_str().unwrap()).unwrap();
        sd.write("game_text/en/98.bin", &[1, 2, 3]).unwrap();
        sd.write("game_text/en/7.bin", &[9; 8]).unwrap();
        let numbers = |bytes: Vec<u8>| -> Vec<u32> { bytes.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() };
        assert_eq!(numbers(sd.read("game_text/en/index.bin").unwrap()), vec![2, 7, 8, 98, 3], "two files, sorted, with their sizes");
        sd.remove("game_text/en/7.bin").unwrap();
        assert_eq!(sd.read("game_text/en/index.bin").unwrap().len(), 4 + 8, "one file left in the list");
        sd.remove("game_text/en/98.bin").unwrap();
        assert!(sd.read("game_text/en/index.bin").is_none(), "no file, no list");
        let _ = std::fs::remove_dir_all(&outer);
    }
}
