//! The files that replace the files of the small archives of the game, in the form that the plugin reads.
//!
//! Studio keeps its own files by species and form (`pokemon/learnset/3_0.bin`, `text/en/98.bin`...). The plugin replaces a file of an
//! archive by its number, so every changed file is also written as `archive/<archive>/<file>.bin`, with the bytes of the file of the
//! archive, and the list `archive/<archive>/index.bin` (the number of files, then the number and the size of each, sorted by number).
//!
//! The text files, the experience tables and the egg moves have their number in their name: [`SdCard::write`] and
//! [`SdCard::remove`] copy them here by themselves (see [`mirror`]). The level-up moves and the evolutions are named by species and
//! form: their pages call [`write`] and [`remove`] with the number of the file.

use crate::{evolution, experience, learnset, text, SdCard};

/// The folder of an archive, on the SD card of the studio.
pub fn folder(archive: usize) -> String {
    format!("archive/{archive}")
}

/// The path of a file of an archive.
pub fn path(archive: usize, file: usize) -> String {
    format!("{}/{file}.bin", folder(archive))
}

/// Writes the list of the files of an archive.
pub fn write_index(sd: &SdCard, archive: usize) -> Result<(), String> {
    let mut files: Vec<(u32, u32)> = Vec::new();
    for found in sd.files_in(&folder(archive)) {
        let Some(number) = found.rsplit('/').next().and_then(|name| name.strip_suffix(".bin")).and_then(|n| n.parse::<u32>().ok()) else { continue };
        let size = sd.read(&found).map_or(0, |bytes| bytes.len() as u32);
        files.push((number, size));
    }
    files.sort_unstable();
    let mut bytes = (files.len() as u32).to_le_bytes().to_vec();
    for (number, size) in &files {
        bytes.extend_from_slice(&number.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
    }
    sd.write(&format!("{}/index.bin", folder(archive)), &bytes).map(|_| ())
}

/// Writes a file of an archive, and the list.
pub fn write(sd: &SdCard, archive: usize, file: usize, bytes: &[u8]) -> Result<(), String> {
    sd.write(&path(archive, file), bytes)?;
    write_index(sd, archive)
}

/// Removes a file of an archive, and writes the list again.
pub fn remove(sd: &SdCard, archive: usize, file: usize) -> Result<(), String> {
    if sd.remove(&path(archive, file))? {
        write_index(sd, archive)?;
    }
    Ok(())
}

/// Removes all the files of an archive.
pub fn clear(sd: &SdCard, archive: usize) -> Result<(), String> {
    for found in sd.files_in(&folder(archive)) {
        sd.remove(&found)?;
    }
    Ok(())
}

/// The archive and the file of a path of the studio whose file has its number in its name: a file of texts, an experience table, the egg
/// moves of a species.
pub fn archive_of(relative: &str) -> Option<(usize, usize)> {
    let number = |name: &str| name.strip_suffix(".bin")?.parse::<usize>().ok();
    let parts: Vec<&str> = relative.split('/').collect();
    match parts.as_slice() {
        ["text", language, name] => Some((text::LANGUAGES.iter().find(|l| l.folder == *language)?.archive, number(name)?)),
        ["pokemon", "experience", name] => Some((experience::ARCHIVE, number(name)?)),
        ["pokemon", "egg_moves", name] => Some((learnset::EGG_ARCHIVE, number(name)?)),
        _ => None,
    }
}

/// Copies a change of a file of the studio that has its number in its name: `bytes` is the new file, None for a file that was removed.
pub fn mirror(sd: &SdCard, relative: &str, bytes: Option<&[u8]>) -> Result<(), String> {
    let Some((archive, file)) = archive_of(relative) else { return Ok(()) };
    match bytes {
        Some(bytes) => write(sd, archive, file, bytes),
        None => remove(sd, archive, file),
    }
}

/// The number of the archive of the evolutions (for the pages that write them).
pub const EVOLUTION_ARCHIVE: usize = evolution::ARCHIVE;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_files_of_the_studio_that_have_their_number_in_their_name_are_found() {
        assert_eq!(archive_of("text/en/98.bin"), Some((73, 98)));
        assert_eq!(archive_of("text/fr/5.bin"), Some((74, 5)));
        assert_eq!(archive_of("pokemon/experience/3.bin"), Some((194, 3)));
        assert_eq!(archive_of("pokemon/egg_moves/25.bin"), Some((190, 25)));
        assert_eq!(archive_of("pokemon/learnset/25_0.bin"), None);
        assert_eq!(archive_of("pokemon/data/25_0.bin"), None);
    }

    #[test]
    fn a_change_of_a_text_file_is_copied_with_its_list() {
        let outer = std::env::temp_dir().join(format!("sango_archive_files_{}", std::process::id()));
        std::fs::create_dir_all(outer.join("sango-studio")).unwrap();
        let sd = SdCard::open(outer.to_str().unwrap()).unwrap();
        sd.write("text/en/98.bin", &[1, 2, 3]).unwrap();
        sd.write("text/en/7.bin", &[9; 8]).unwrap();
        assert_eq!(sd.read("archive/73/98.bin").unwrap(), vec![1, 2, 3]);
        let list = sd.read("archive/73/index.bin").unwrap();
        let numbers: Vec<u32> = list.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        assert_eq!(numbers, vec![2, 7, 8, 98, 3], "two files, sorted, with their sizes");
        sd.remove("text/en/7.bin").unwrap();
        let list = sd.read("archive/73/index.bin").unwrap();
        assert_eq!(list.len(), 4 + 8, "one file left in the list");
        assert!(sd.read("archive/73/7.bin").is_none());
        let _ = std::fs::remove_dir_all(&outer);
    }
}
