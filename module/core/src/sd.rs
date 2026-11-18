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

/// The paths in the folder of the studio that the plugin reads: the other files stay on the computer. A folder ends with `/`.
const PUBLISHED: [&str; 24] = [
    "reload.bin",
    "species/table.bin",
    "species/data/",
    "species/pokedex_data.bin",
    "species/icon/",
    "species/cry/",
    "species/level_up_moves/",
    "species/egg_moves/",
    "species/evolution/",
    "leveling_rate/",
    "trainer/",
    "choreography/",
    "effect/",
    "script/",
    "dexnav/",
    "game_text/",
    "ability/",
    "move/data/",
    "move/custom/",
    "item/data/",
    "item/custom/",
    "type/",
    "formula/",
    "nature/",
];

/// The files that stay on the SD card only: the 3D models of the packs (some hundreds of megabytes, made again from the packs) and the maps of
/// the Overworld page (the editor of the maps writes them there).
const CARD_ONLY: [&str; 2] = ["species/model/", "overworld/"];

/// The folder of the maps in the folder of the studio of the SD card.
pub const OVERWORLD_FOLDER: &str = "overworld";

/// Where the older versions of the editor of the maps wrote its files, in `sango` (next to the folder of the studio), and where they are
/// now, in `overworld`.
const OLD_MAP_PLACES: [(&str, &str); 5] = [("overworld_model", "model"), ("land", "land"), ("area", "area"), ("matrix", "matrix"), ("zone", "zone")];

/// The places of the files in the older versions of the studio, and where they are now. A folder ends with `/`.
const OLD_PLACES: [(&str, &str); 7] = [
    ("pokemon/data/", "species/data/"),
    ("pokemon/pokedex_data.bin", "species/pokedex_data.bin"),
    ("pokemon/icon/", "species/icon/"),
    ("pokemon/egg_moves/", "species/egg_moves/"),
    ("pokemon/experience/", "leveling_rate/"),
    ("text/", "game_text/"),
    ("archive/100/", "species/cry/"),
];

/// The folder on the computer where the studio keeps all its files. When it is set, the folder of the SD card only gets the files that the
/// plugin reads (see [`SdCard::published`]). Set once, when the program starts.
static WORKSPACE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Tells where the studio keeps its files on the computer.
pub fn set_workspace(folder: PathBuf) {
    let _ = WORKSPACE.set(folder);
}

/// The place of the files of the studio on the computer, when nothing else is chosen.
pub fn default_workspace() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("SangoStudio").join("workspace"))
}

/// How many times the files of the studio were all erased in this session: the pages that keep a copy of them read them again when it changes.
static ERASED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// How many times [`SdCard::erase_everything`] ran.
pub fn erase_count() -> u32 {
    ERASED.load(std::sync::atomic::Ordering::Relaxed)
}

/// What [`SdCard::erase_everything`] removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Erased {
    /// The files and the bytes of the folder of the studio on the computer.
    pub computer: (usize, u64),
    /// The files and the bytes of the folder of the studio on the SD card.
    pub card: (usize, u64),
    /// The files and the bytes of the 3D models that the studio wrote on the SD card (`species/model`, inside the folder of the studio).
    pub models: (usize, u64),
}

/// The files of the studio.
///
/// The studio keeps all its files in a folder of the computer (the workspace, see [`set_workspace`]). The folder of the SD card, in the
/// folder `sdmc` of the emulator, only gets a copy of the files that the plugin reads: the plugin loads them from there, also while the
/// game runs. The romfs dump is read only. Without a workspace, the folder of the SD card holds everything.
#[derive(Clone, Debug)]
pub struct SdCard {
    /// The folder of the SD card.
    root: PathBuf,
    /// The folder of the files of the studio on the computer: the same as `root` when there is no workspace.
    local: PathBuf,
}

impl SdCard {
    /// Checks a folder. It can be the folder of Sango Studio itself, or the folder `sdmc`:
    /// then the folder of Sango Studio is made inside it.
    pub fn open(path: &str) -> Result<SdCard, String> {
        Self::open_with(path, WORKSPACE.get().cloned())
    }

    /// The same, with the folder of the files of the studio on the computer (None: the SD card holds everything).
    pub fn open_with(path: &str, workspace: Option<PathBuf>) -> Result<SdCard, String> {
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
        let local = workspace.unwrap_or_else(|| root.clone());
        let card = SdCard { root, local };
        if card.is_split() {
            std::fs::create_dir_all(&card.local).map_err(|e| format!("Cannot make {}: {e}", card.local.display()))?;
            card.move_files_to_the_computer()?;
        }
        Ok(card)
    }

    /// The folder of the SD card (the plugin reads it).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder of the maps, in the folder of the studio of the SD card: `overworld`. The editor of the maps writes the files of the
    /// land, the areas, the matrices, the zones and the models of the characters there, and the plugin reads them. They are only on the SD card.
    pub fn overworld_folder(&self) -> PathBuf {
        self.root.join(OVERWORLD_FOLDER)
    }

    /// The folder of the 3D models of the packs, in the folder of the studio of the SD card: `species/model`. The plugin reads the models
    /// from there. They are only on the SD card.
    pub fn models_folder(&self) -> PathBuf {
        self.root.join("species").join("model")
    }

    /// What [`SdCard::erase_everything`] would remove.
    pub fn count_everything(&self) -> Erased {
        let count = |folder: &Path| -> (usize, u64) {
            let mut found = Vec::new();
            Self::collect(folder, "", &mut found);
            let bytes = found.iter().map(|file| std::fs::metadata(folder.join(file.trim_start_matches('/'))).map_or(0, |m| m.len())).sum();
            (found.len(), bytes)
        };
        // The maps are the work of the user on the Overworld page: starting from zero does not remove them.
        let (all, models, maps) = (count(&self.root), count(&self.models_folder()), count(&self.overworld_folder()));
        Erased {
            computer: if self.is_split() { count(&self.local) } else { (0, 0) },
            card: (all.0 - models.0 - maps.0, all.1 - models.1 - maps.1),
            models,
        }
    }

    /// Starts from zero: removes all the files of the studio on the computer and on the SD card, and the 3D models that the studio wrote on the
    /// SD card. The folders themselves stay. The settings of the program, the dump of the game and the other files of the plugin (the maps,
    /// the names) are not touched.
    pub fn erase_everything(&self) -> Result<Erased, String> {
        let done = self.count_everything();
        let mut folders = vec![self.root.clone()];
        if self.is_split() {
            folders.push(self.local.clone());
        }
        for folder in folders {
            let Ok(entries) = std::fs::read_dir(&folder) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name() == OVERWORLD_FOLDER && folder == self.root {
                    continue;
                }
                let result = if entry.file_type().is_ok_and(|kind| kind.is_dir()) { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
                result.map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        ERASED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(done)
    }

    /// The folder where the studio keeps its files on the computer.
    pub fn local(&self) -> &Path {
        &self.local
    }

    /// True when the files of the studio are on the computer and only a part of them goes to the SD card.
    pub fn is_split(&self) -> bool {
        self.local != self.root
    }

    /// True for a file that the plugin reads, so that it goes to the SD card: `reload.bin`, the table of the species, the data of the
    /// species, the icons, the abilities, the moves, the items, the types and the files of the archives.
    pub fn published(relative: &str) -> bool {
        PUBLISHED.iter().any(|entry| if entry.ends_with('/') { relative.starts_with(entry) } else { relative == *entry })
    }

    /// At each opening: the files that the studio wrote on the SD card before the workspace existed come to the computer, and the SD card
    /// keeps only the files that the plugin reads (all of them, as the computer has them).
    fn move_files_to_the_computer(&self) -> Result<(), String> {
        self.move_the_old_models()?;
        self.move_the_old_maps()?;
        let mut found = Vec::new();
        Self::collect(&self.root, "", &mut found);
        for relative in found.iter().map(|f| f.trim_start_matches('/').to_string()) {
            if relative == ".sango_write_test" || CARD_ONLY.iter().any(|prefix| relative.starts_with(prefix)) {
                continue;
            }
            let (from, to) = (self.root.join(&relative), self.local.join(&relative));
            if !to.exists() {
                if let Some(folder) = to.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
                }
                std::fs::copy(&from, &to).map_err(|e| format!("{} to {}: {e}", from.display(), to.display()))?;
            }
            if !Self::published(&relative) {
                std::fs::remove_file(&from).map_err(|e| format!("{}: {e}", from.display()))?;
            }
        }
        Self::remove_empty_folders(&self.root);
        self.move_to_the_new_places()?;
        // The files that the plugin reads are on the SD card, as the computer has them.
        let mut local_files = Vec::new();
        Self::collect(&self.local, "", &mut local_files);
        for relative in local_files.iter().map(|f| f.trim_start_matches('/').to_string()).filter(|f| Self::published(f)) {
            let (from, to) = (self.local.join(&relative), self.root.join(&relative));
            let same = match (std::fs::read(&from), std::fs::read(&to)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };
            if !same {
                if let Some(folder) = to.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
                }
                std::fs::copy(&from, &to).map_err(|e| format!("{} to {}: {e}", from.display(), to.display()))?;
            }
        }
        Ok(())
    }

    /// The models that the older versions wrote in `sango/pokemodel` (next to the folder of the studio) come into the folder of the studio.
    fn move_the_old_models(&self) -> Result<(), String> {
        let Some(old) = self.root.parent().map(|sdmc| sdmc.join("sango").join("pokemodel")) else { return Ok(()) };
        let new = self.models_folder();
        if old.is_dir() && !new.exists() {
            if let Some(folder) = new.parent() {
                std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            }
            std::fs::rename(&old, &new).map_err(|e| format!("{} to {}: {e}", old.display(), new.display()))?;
        }
        Ok(())
    }

    /// The files of the maps that the older versions of the editor wrote in `sango` (next to the folder of the studio) come into the folder
    /// `overworld` of the studio.
    fn move_the_old_maps(&self) -> Result<(), String> {
        let Some(sango) = self.root.parent().map(|sdmc| sdmc.join("sango")) else { return Ok(()) };
        for (old, new) in OLD_MAP_PLACES {
            let (from, to) = (sango.join(old), self.overworld_folder().join(new));
            if from.is_dir() && !to.exists() {
                if let Some(folder) = to.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
                }
                std::fs::rename(&from, &to).map_err(|e| format!("{} to {}: {e}", from.display(), to.display()))?;
            }
        }
        Ok(())
    }

    /// The files of the older versions of the studio get their new places (the data of the species in `species`, the experience tables in
    /// `leveling_rate`, the texts in `game_text`...), and the lists of the folders are written. The copies by archive are dropped.
    fn move_to_the_new_places(&self) -> Result<(), String> {
        let mut found = Vec::new();
        Self::collect(&self.local, "", &mut found);
        let mut moved = false;
        for relative in found.iter().map(|f| f.trim_start_matches('/').to_string()) {
            let old = self.local.join(&relative);
            if relative.starts_with("archive/") && !relative.starts_with("archive/100/") {
                // A copy that the studio makes again from its files.
                let _ = std::fs::remove_file(&old);
                continue;
            }
            let Some(new) = OLD_PLACES.iter().find_map(|(from, to)| {
                if from.ends_with('/') {
                    relative.strip_prefix(from).map(|rest| format!("{to}{rest}"))
                } else {
                    (relative == *from).then(|| to.to_string())
                }
            }) else {
                continue;
            };
            let target = self.local.join(&new);
            if !relative.ends_with("/index.bin") && !target.exists() {
                if let Some(folder) = target.parent() {
                    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
                }
                std::fs::rename(&old, &target).map_err(|e| format!("{} to {}: {e}", old.display(), target.display()))?;
            } else {
                let _ = std::fs::remove_file(&old);
            }
            moved = true;
        }
        Self::remove_empty_folders(&self.local);
        if moved {
            for (folder, _) in crate::file_lists::folders() {
                if self.local.join(&folder).is_dir() {
                    crate::file_lists::write_index(self, &folder)?;
                }
            }
        }
        Ok(())
    }

    /// Removes the folders that have nothing in them (the folder itself stays).
    fn remove_empty_folders(folder: &Path) {
        let Ok(entries) = std::fs::read_dir(folder) else { return };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                Self::remove_empty_folders(&entry.path());
                let _ = std::fs::remove_dir(entry.path());
            }
        }
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
        Ok(self.local.join(relative))
    }

    /// Writes a file. The folders on the way are made.
    pub fn write(&self, relative: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let path = self.path(relative)?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        // The plugin reads some files from the SD card.
        if self.is_split() && Self::published(relative) {
            let copy = self.root.join(relative);
            if let Some(folder) = copy.parent() {
                std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            }
            std::fs::write(&copy, bytes).map_err(|e| format!("{}: {e}", copy.display()))?;
        }
        // The files that replace a file of an archive of the game are copied where the plugin reads them.
        crate::file_lists::update(self, relative)?;
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

    /// Removes a file. A file that is not there is not an error. Gives true when a file was removed.
    pub fn remove(&self, relative: &str) -> Result<bool, String> {
        let path = self.path(relative)?;
        if self.is_split() && Self::published(relative) {
            let _ = std::fs::remove_file(self.root.join(relative));
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                crate::file_lists::update(self, relative)?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }

    /// The files of a folder and of the folders inside it, as paths in the folder of the card (with `/`), in
    /// alphabetical order. A folder that is not there has no file.
    pub fn files_in(&self, relative: &str) -> Vec<String> {
        let mut found = Vec::new();
        if let Ok(path) = self.path(relative) {
            Self::collect(&path, relative, &mut found);
        }
        found.sort();
        found
    }

    fn collect(path: &Path, relative: &str, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(path) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let inner = format!("{relative}/{name}");
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => Self::collect(&entry.path(), &inner, found),
                Ok(kind) if kind.is_file() => found.push(inner),
                _ => {}
            }
        }
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
    fn files_are_listed_and_removed() {
        let path = folder("list");
        let sd = SdCard::open(&path.to_string_lossy()).unwrap();
        sd.write("a/1.bin", b"1").unwrap();
        sd.write("a/b/2.bin", b"2").unwrap();
        assert_eq!(sd.files_in("a"), ["a/1.bin", "a/b/2.bin"]);
        assert!(sd.files_in("nothing").is_empty());
        assert_eq!(sd.remove("a/1.bin"), Ok(true));
        assert_eq!(sd.remove("a/1.bin"), Ok(false));
        assert_eq!(sd.files_in("a"), ["a/b/2.bin"]);
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

    #[test]
    fn with_a_workspace_the_sd_card_only_gets_the_files_that_the_plugin_reads() {
        let (card, work) = (folder("split_card"), folder("split_work"));
        let sd = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        assert!(sd.is_split());
        sd.write("species/registry.json", b"{}").unwrap();
        sd.write("species/table.bin", b"table").unwrap();
        sd.write("pokemon/learnset/1_0.bin", b"moves").unwrap();
        sd.write("species/cry/5.bin", b"copy").unwrap();
        // Everything is on the computer.
        assert!(work.join("species/registry.json").is_file() && work.join("pokemon/learnset/1_0.bin").is_file());
        // The SD card has what the plugin reads.
        assert_eq!(fs::read(card.join("species/table.bin")).unwrap(), b"table");
        assert_eq!(fs::read(card.join("species/cry/5.bin")).unwrap(), b"copy");
        assert!(card.join("species/cry/index.bin").is_file(), "the list goes with the files");
        assert!(!card.join("species/registry.json").exists() && !card.join("pokemon/learnset").exists());
        // A removal goes to both.
        assert_eq!(sd.remove("species/cry/5.bin"), Ok(true));
        assert!(!work.join("species/cry/5.bin").exists() && !card.join("species/cry/5.bin").exists());
        // The reads and the lists use the computer.
        assert_eq!(sd.read("species/registry.json").unwrap(), b"{}");
        assert_eq!(sd.files_in("pokemon"), ["pokemon/learnset/1_0.bin"]);
    }

    #[test]
    fn the_files_that_the_sd_card_had_before_come_to_the_computer() {
        let (card, work) = (folder("move_card"), folder("move_work"));
        fs::create_dir_all(card.join("species/packs")).unwrap();
        fs::create_dir_all(card.join("code/src")).unwrap();
        fs::write(card.join("species/packs/a.sangopack"), b"pack").unwrap();
        fs::write(card.join("code/src/x.cc"), b"code").unwrap();
        fs::write(card.join("species/table.bin"), b"table").unwrap();
        let _sd = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        // The files of the studio moved, the table of the plugin stayed (and is on the computer too).
        assert_eq!(fs::read(work.join("species/packs/a.sangopack")).unwrap(), b"pack");
        assert_eq!(fs::read(work.join("code/src/x.cc")).unwrap(), b"code");
        assert_eq!(fs::read(work.join("species/table.bin")).unwrap(), b"table");
        assert!(!card.join("species/packs").exists() && !card.join("code").exists());
        assert_eq!(fs::read(card.join("species/table.bin")).unwrap(), b"table");
        // A file that the plugin reads and that only the computer has goes to the SD card at the next opening.
        fs::write(work.join("pokemon/data_new.bin"), b"x").ok();
        fs::create_dir_all(work.join("ability")).unwrap();
        fs::write(work.join("ability/192.bin"), b"ability").unwrap();
        let _again = SdCard::open_with(&card.to_string_lossy(), Some(work)).unwrap();
        assert_eq!(fs::read(card.join("ability/192.bin")).unwrap(), b"ability");
    }

    #[test]
    fn starting_from_zero_removes_the_files_of_the_studio_and_the_models_only() {
        let (card, work) = (folder("zero").join("sdmc"), folder("zero_work"));
        fs::create_dir_all(&card).unwrap();
        let sd = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        sd.write("species/table.bin", b"table").unwrap();
        sd.write("species/registry.json", b"{}").unwrap();
        fs::create_dir_all(card.join("sango-studio/species/model")).unwrap();
        fs::write(card.join("sango-studio/species/model/0.raw"), b"models").unwrap();
        fs::create_dir_all(card.join("sango/maker")).unwrap();
        fs::write(card.join("sango/maker/names.bin"), b"names").unwrap();
        let before = erase_count();
        let counted = sd.count_everything();
        assert_eq!((counted.computer.0, counted.card.0, counted.models.0), (2, 1, 1));
        assert_eq!(sd.erase_everything().unwrap(), counted);
        assert!(sd.files_in("species").is_empty() && !card.join("sango-studio/species/model").exists());
        assert!(card.join("sango/maker/names.bin").exists(), "the other files of the plugin stay");
        assert!(sd.root().is_dir() && work.is_dir(), "the folders stay");
        assert!(erase_count() > before, "the counter moved (another test may erase at the same time)");
    }

    #[test]
    fn the_models_stay_on_the_sd_card_and_an_older_folder_of_models_comes_in() {
        let card = folder("models").join("sdmc");
        fs::create_dir_all(card.join("sango/pokemodel")).unwrap();
        fs::write(card.join("sango/pokemodel/0.raw"), b"models").unwrap();
        let work = folder("models_work");
        let sd = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        assert_eq!(fs::read(sd.models_folder().join("0.raw")).unwrap(), b"models", "the older folder moved in");
        assert!(!card.join("sango/pokemodel").exists());
        assert!(!work.join("species").exists(), "the models are not copied to the computer");
        // The next opening does not copy them either.
        let _again = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        assert!(!work.join("species/model").exists() && sd.models_folder().join("0.raw").is_file());
    }

    #[test]
    fn the_maps_of_the_older_editor_come_into_the_overworld_folder_and_stay_on_the_card() {
        let card = folder("maps").join("sdmc");
        fs::create_dir_all(card.join("sango/land")).unwrap();
        fs::write(card.join("sango/land/0.bin"), b"land").unwrap();
        fs::create_dir_all(card.join("sango/overworld_model")).unwrap();
        fs::write(card.join("sango/overworld_model/index.bin"), b"models").unwrap();
        let work = folder("maps_work");
        let sd = SdCard::open_with(&card.to_string_lossy(), Some(work.clone())).unwrap();
        assert_eq!(fs::read(sd.overworld_folder().join("land/0.bin")).unwrap(), b"land");
        assert_eq!(fs::read(sd.overworld_folder().join("model/index.bin")).unwrap(), b"models");
        assert!(!card.join("sango/land").exists() && !work.join("overworld").exists(), "the maps are not copied to the computer");
        // Starting from zero keeps them.
        sd.write("species/table.bin", b"table").unwrap();
        sd.erase_everything().unwrap();
        assert!(sd.overworld_folder().join("land/0.bin").is_file());
    }
}
