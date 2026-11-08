use std::path::{Path, PathBuf};

/// The files that a dump of the romfs must have (archives of the game).
const REQUIRED: [&str; 3] = ["a/0/1/3", "a/0/3/9", "a/2/7/7"];

/// The identifier of Pokemon Omega Ruby, which Sango Studio does not support.
const OMEGA_RUBY_ID: &str = "000400000011c400";

/// A dump of the romfs of the game: a folder that has the folder `a`.
#[derive(Clone, Debug)]
pub struct Dump {
    root: PathBuf,
}

impl Dump {
    /// Checks a folder. The folder can be the romfs itself, or hold a folder `romfs`.
    pub fn open(path: &str) -> Result<Dump, String> {
        let path = path.trim().trim_matches('"');
        if path.is_empty() {
            return Err("Give the folder of the romfs dump.".into());
        }
        if path.to_lowercase().contains(OMEGA_RUBY_ID) {
            return Err("This looks like Pokemon Omega Ruby: Sango Studio works with Pokemon Alpha Sapphire only.".into());
        }
        let base = Path::new(path);
        if !base.is_dir() {
            return Err("This folder does not exist.".into());
        }
        let root = if base.join("a").is_dir() {
            base.to_path_buf()
        } else if base.join("romfs").join("a").is_dir() {
            base.join("romfs")
        } else {
            return Err("There is no folder \"a\" here: choose the folder of the romfs dump.".into());
        };
        let missing: Vec<&str> = REQUIRED.iter().copied().filter(|file| !root.join(file).is_file()).collect();
        if !missing.is_empty() {
            return Err(format!("The dump is not complete: {} is missing.", missing.join(", ")));
        }
        Ok(Dump { root })
    }

    /// The folder that holds the folder `a`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The numbers of the archives that the dump has: the files `a/x/y/z` give the number `xyz`.
    pub fn archive_numbers(&self) -> Vec<usize> {
        fn digits(folder: &Path) -> Vec<(usize, PathBuf)> {
            let mut list: Vec<(usize, PathBuf)> = std::fs::read_dir(folder)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    (name.len() == 1).then(|| name.parse::<usize>().ok()).flatten().map(|d| (d, entry.path()))
                })
                .collect();
            list.sort();
            list
        }
        let mut numbers = Vec::new();
        for (a, a_path) in digits(&self.root.join("a")) {
            for (b, b_path) in digits(&a_path) {
                for (c, c_path) in digits(&b_path) {
                    if c_path.is_file() {
                        numbers.push(a * 100 + b * 10 + c);
                    }
                }
            }
        }
        numbers.sort_unstable();
        numbers
    }

    /// The path of an archive: `archive(277)` is `a/2/7/7`.
    pub fn archive(&self, number: usize) -> PathBuf {
        self.root.join("a").join((number / 100).to_string()).join((number / 10 % 10).to_string()).join((number % 10).to_string())
    }
}

#[cfg(test)]
mod test {
    use super::Dump;
    use std::fs;

    fn folder(name: &str, files: &[&str]) -> String {
        let root = std::env::temp_dir().join(format!("sango_dump_test_{name}"));
        let _ = fs::remove_dir_all(&root);
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().to_string()
    }

    #[test]
    fn a_complete_dump_opens() {
        let path = folder("complete", &["a/0/1/3", "a/0/3/9", "a/2/7/7"]);
        let dump = Dump::open(&path).unwrap();
        assert_eq!(dump.archive(277), dump.root().join("a").join("2").join("7").join("7"));
    }

    #[test]
    fn a_folder_that_holds_the_romfs_opens() {
        let path = folder("holder", &["romfs/a/0/1/3", "romfs/a/0/3/9", "romfs/a/2/7/7"]);
        assert!(Dump::open(&path).is_ok());
    }

    #[test]
    fn a_missing_archive_is_said() {
        let path = folder("missing", &["a/0/1/3"]);
        let message = Dump::open(&path).unwrap_err();
        assert!(message.contains("a/0/3/9") && message.contains("a/2/7/7"), "{message}");
    }

    #[test]
    fn a_folder_without_a_is_refused() {
        let path = folder("empty", &["other/file"]);
        assert!(Dump::open(&path).is_err());
        assert!(Dump::open("").is_err());
    }

    #[test]
    fn omega_ruby_is_refused() {
        assert!(Dump::open("C:/dump/000400000011C400/romfs").unwrap_err().contains("Omega Ruby"));
    }
}
