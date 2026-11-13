//! Which entries (species, items, moves, abilities) differ from the game.
//!
//! The studio writes a file on the SD card only for an entry that it changes. An entry is *modified* when a file of
//! the SD card has other bytes than the file of the game (a file that is the same as the game is not a change). The
//! lists of the pages use these functions to mark the modified entries.

use crate::text::{self, TextFile, LANGUAGES};
use crate::SdCard;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// How long a result is kept before it is computed again.
const KEEP: Duration = Duration::from_millis(1500);

/// The names (without the folder and without `.bin`) of the files of a folder of the SD card that are not the same as
/// the file of the game. `romfs` gives the bytes of the game for a name. A file that the game does not have is a change.
pub fn changed_files(sd: &SdCard, folder: &str, romfs: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<String> {
    let mut found = Vec::new();
    for path in sd.files_in(folder) {
        let Some(name) = path.rsplit('/').next().and_then(|file| file.strip_suffix(".bin")) else { continue };
        let Some(bytes) = sd.read(&path) else { continue };
        if romfs(name).as_deref() != Some(bytes.as_slice()) {
            found.push(name.to_string());
        }
    }
    found
}

/// Removes the files of a folder of the SD card that are the same as the file of the game: they change nothing, and the
/// entry is not a modified entry. Gives the number of removed files. `romfs` is as for [`changed_files`].
pub fn remove_unchanged(sd: &SdCard, folder: &str, romfs: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<usize, String> {
    let mut removed = 0;
    for path in sd.files_in(folder) {
        let Some(name) = path.rsplit('/').next().and_then(|file| file.strip_suffix(".bin")) else { continue };
        let Some(bytes) = sd.read(&path) else { continue };
        if romfs(name).as_deref() == Some(bytes.as_slice()) && sd.remove(&path)? {
            sd.mark_changed(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// The numbers of the lines of the files of texts that are not the same as the game, in any language. `files` are the
/// numbers of the files of texts that are looked at. `romfs` gives the text file of the game for a language and a file.
pub fn changed_text_lines(sd: &SdCard, files: &[usize], romfs: &dyn Fn(usize, usize) -> Option<TextFile>) -> HashSet<usize> {
    let mut found = HashSet::new();
    for language in 0..LANGUAGES.len() {
        for &file in files {
            let Some(own) = sd.read(&text::sd_path(language, file)).and_then(|bytes| TextFile::from_bytes(&bytes).ok()) else { continue };
            let game = romfs(language, file);
            for line in 0..own.len() {
                if game.as_ref().and_then(|game| game.text(line)) != own.text(line) {
                    found.insert(line);
                }
            }
        }
    }
    found
}

/// A set of modified entries that is computed again from time to time, not at each frame.
#[derive(Default)]
pub struct Cache {
    set: HashSet<usize>,
    at: Option<Instant>,
}

impl Cache {
    /// The set. It is computed with `compute` the first time and when it is older than a short time.
    pub fn get(&mut self, compute: impl FnOnce() -> HashSet<usize>) -> &HashSet<usize> {
        if self.at.map_or(true, |at| at.elapsed() > KEEP) {
            self.set = compute();
            self.at = Some(Instant::now());
        }
        &self.set
    }

    /// Forgets the set: the next call to `get` computes it.
    pub fn forget(&mut self) {
        self.at = None;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn only_the_files_that_differ_from_the_game_are_changes() {
        let folder = std::env::temp_dir().join(format!("sango_modified_test_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let sd = SdCard::open(folder.to_str().unwrap()).unwrap();
        sd.write("item/data/1.bin", &[1, 2, 3]).unwrap();
        sd.write("item/data/2.bin", &[9, 9, 9]).unwrap();
        let game = |name: &str| Some(vec![1u8, 2, 3]).filter(|_| name != "3");
        assert_eq!(changed_files(&sd, "item/data", &game), ["2"]);
        assert_eq!(remove_unchanged(&sd, "item/data", &game), Ok(1));
        assert!(sd.read("item/data/1.bin").is_none() && sd.read("item/data/2.bin").is_some());
        let mut cache = Cache::default();
        assert_eq!(cache.get(|| HashSet::from([4])).len(), 1);
        assert!(cache.get(|| HashSet::new()).contains(&4), "kept for a moment");
        cache.forget();
        assert!(cache.get(|| HashSet::new()).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
