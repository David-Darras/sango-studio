use std::path::{Component, Path, PathBuf};

/// The name of the folder that Sango Studio uses inside the folder `sdmc` of the emulator.
pub const SD_FOLDER_NAME: &str = "sango-studio";

/// The file that tells the plugin that something changed.
pub const RELOAD_FILE: &str = "reload.bin";
const RELOAD_SIGNATURE: &[u8; 4] = b"SREL";
/// The size of one path in the file `reload.bin`.
const RELOAD_PATH_SIZE: usize = 64;
/// The most paths in the file `reload.bin`. The oldest one goes first.
const RELOAD_MAX_PATHS: usize = 256;

/// The folder of the SD card where Sango Studio puts the files that it changed.
///
/// The romfs dump is read only. A file that the user changes goes here, and the plugin loads it
/// from here, also while the game runs.
#[derive(Clone, Debug)]
pub struct SdCard {
    root: PathBuf,
}

impl SdCard {
    /// Checks a folder. It can be the folder of Sango Studio itself, or the folder `sdmc`:
    /// then the folder of Sango Studio is made inside it.
    pub fn open(path: &str) -> Result<SdCard, String> {
        let path = path.trim().trim_matches('"');
        if path.is_empty() {
            return Err("Give the folder of the SD card.".into());
        }
        let given = Path::new(path);
        if given.is_file() {
            return Err("This is a file, not a folder.".into());
        }
        if !given.exists() {
            return Err("This folder does not exist.".into());
        }
        let root = if given.file_name().is_some_and(|name| name.eq_ignore_ascii_case("sdmc")) {
            let inner = given.join(SD_FOLDER_NAME);
            std::fs::create_dir_all(&inner).map_err(|e| format!("Cannot make {}: {e}", inner.display()))?;
            inner
        } else {
            given.to_path_buf()
        };
        // The folder must accept new files.
        let probe = root.join(".sango_write_test");
        std::fs::write(&probe, b"x").map_err(|e| format!("Cannot write in this folder: {e}"))?;
        let _ = std::fs::remove_file(&probe);
        Ok(SdCard { root })
    }

    /// The folder of the files.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The full path of a file, from its path in the folder (like `land/42.bin`).
    ///
    /// A path that is absolute or that goes up with `..` is refused.
    pub fn path(&self, relative: &str) -> Result<PathBuf, String> {
        let relative = Path::new(relative);
        let clean = relative.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if relative.as_os_str().is_empty() || !clean {
            return Err(format!("{} is not a path inside the folder.", relative.display()));
        }
        Ok(self.root.join(relative))
    }

    /// Writes a file. The folders on the way are made.
    pub fn write(&self, relative: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let path = self.path(relative)?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }

    /// Tells the plugin that a file changed, so that it loads the file again while the game runs.
    ///
    /// The plugin looks at the file `reload.bin` of the folder a few times each second. The file has:
    ///
    /// | Size | Meaning |
    /// | --- | --- |
    /// | 4 | The signature `SREL`. |
    /// | 4 | The counter. It grows by one each time that a file changes. |
    /// | 4 | The number of paths. |
    /// | 64 each | A path in the folder, like `pokemon/data/2_0.bin`, filled with zeros. |
    ///
    /// The plugin loads all the listed files again when the counter changes. Write the data file
    /// first, then call this function: the counter is the last thing that changes.
    pub fn mark_changed(&self, relative: &str) -> Result<(), String> {
        if relative.len() >= RELOAD_PATH_SIZE {
            return Err(format!("The path {relative} is too long."));
        }
        let (mut counter, mut paths) = self.reload_state();
        if !paths.iter().any(|p| p == relative) {
            if paths.len() >= RELOAD_MAX_PATHS {
                paths.remove(0);
            }
            paths.push(relative.to_string());
        }
        counter = counter.wrapping_add(1);
        let mut bytes = Vec::with_capacity(12 + paths.len() * RELOAD_PATH_SIZE);
        bytes.extend_from_slice(RELOAD_SIGNATURE);
        bytes.extend_from_slice(&counter.to_le_bytes());
        bytes.extend_from_slice(&(paths.len() as u32).to_le_bytes());
        for path in &paths {
            let mut field = [0u8; RELOAD_PATH_SIZE];
            field[..path.len()].copy_from_slice(path.as_bytes());
            bytes.extend_from_slice(&field);
        }
        self.write(RELOAD_FILE, &bytes).map(|_| ())
    }

    /// Reads `reload.bin`: the counter and the paths. A missing or bad file is an empty state.
    pub fn reload_state(&self) -> (u32, Vec<String>) {
        let Some(bytes) = self.read(RELOAD_FILE) else { return (0, Vec::new()) };
        let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if bytes.get(..4) != Some(RELOAD_SIGNATURE.as_slice()) {
            return (0, Vec::new());
        }
        let (Some(counter), Some(count)) = (word(4), word(8)) else { return (0, Vec::new()) };
        let mut paths = Vec::new();
        for i in 0..count as usize {
            let from = 12 + i * RELOAD_PATH_SIZE;
            let Some(field) = bytes.get(from..from + RELOAD_PATH_SIZE) else { break };
            let end = field.iter().position(|&b| b == 0).unwrap_or(RELOAD_PATH_SIZE);
            paths.push(String::from_utf8_lossy(&field[..end]).to_string());
        }
        (counter, paths)
    }

    /// Reads a file. The result is `None` when the file is not there.
    pub fn read(&self, relative: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(relative).ok()?).ok()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("sango_sd_test_{name}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_folder_opens() {
        let path = folder("plain");
        let sd = SdCard::open(&path.to_string_lossy()).unwrap();
        assert_eq!(sd.root(), path);
        assert!(!path.join(".sango_write_test").exists());
    }

    #[test]
    fn the_folder_sdmc_gets_the_folder_of_the_studio() {
        let path = folder("card").join("sdmc");
        fs::create_dir_all(&path).unwrap();
        let sd = SdCard::open(&path.to_string_lossy()).unwrap();
        assert_eq!(sd.root(), path.join(SD_FOLDER_NAME));
        assert!(sd.root().is_dir());
    }

    #[test]
    fn a_bad_folder_is_refused_with_a_reason() {
        assert!(SdCard::open("").is_err());
        assert!(SdCard::open("C:/this/folder/does/not/exist").unwrap_err().contains("does not exist"));
        let path = folder("file");
        let file = path.join("a.txt");
        fs::write(&file, b"x").unwrap();
        assert!(SdCard::open(&file.to_string_lossy()).unwrap_err().contains("file"));
    }

    #[test]
    fn files_are_written_and_read_inside_the_folder() {
        let sd = SdCard::open(&folder("rw").to_string_lossy()).unwrap();
        let path = sd.write("land/42.bin", b"abc").unwrap();
        assert!(path.ends_with("42.bin"));
        assert_eq!(sd.read("land/42.bin"), Some(b"abc".to_vec()));
        assert_eq!(sd.read("land/43.bin"), None);
    }

    #[test]
    fn a_change_is_announced_with_a_counter_and_the_paths() {
        let sd = SdCard::open(&folder("reload").to_string_lossy()).unwrap();
        assert_eq!(sd.reload_state(), (0, Vec::new()));
        sd.mark_changed("pokemon/data/2_0.bin").unwrap();
        sd.mark_changed("pokemon/data/3_1.bin").unwrap();
        sd.mark_changed("pokemon/data/2_0.bin").unwrap();
        let (counter, paths) = sd.reload_state();
        assert_eq!(counter, 3);
        assert_eq!(paths, vec!["pokemon/data/2_0.bin", "pokemon/data/3_1.bin"]);
        assert_eq!(sd.read("reload.bin").unwrap().len(), 12 + 2 * 64);
        assert!(sd.mark_changed(&"x".repeat(70)).is_err());
    }

    #[test]
    fn a_path_outside_the_folder_is_refused() {
        let sd = SdCard::open(&folder("escape").to_string_lossy()).unwrap();
        assert!(sd.write("../outside.bin", b"x").is_err());
        assert!(sd.write("a/../../outside.bin", b"x").is_err());
        assert!(sd.path("C:/absolute.bin").is_err());
        assert!(sd.path("").is_err());
    }
}
