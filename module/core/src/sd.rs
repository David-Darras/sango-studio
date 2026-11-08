use std::path::{Component, Path, PathBuf};

/// The name of the folder that Sango Studio uses inside the folder `sdmc` of the emulator.
pub const SD_FOLDER_NAME: &str = "sango-studio";

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
    fn a_path_outside_the_folder_is_refused() {
        let sd = SdCard::open(&folder("escape").to_string_lossy()).unwrap();
        assert!(sd.write("../outside.bin", b"x").is_err());
        assert!(sd.write("a/../../outside.bin", b"x").is_err());
        assert!(sd.path("C:/absolute.bin").is_err());
        assert!(sd.path("").is_err());
    }
}
