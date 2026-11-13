//! A copy of files of the SD card, to put them back after an operation that changes many files (an import, a reset).

use crate::SdCard;
use std::collections::BTreeMap;

/// A copy of the files that an operation can change: [`Backup::restore`] puts them back.
pub struct Backup {
    /// What the operation did, for the Undo button.
    pub label: String,
    folders: Vec<String>,
    loose: Vec<String>,
    /// The files as they were: the path and the bytes.
    files: BTreeMap<String, Vec<u8>>,
}

impl Backup {
    /// Copies the files of some folders (with the folders inside them) and some single files.
    pub fn snapshot(sd: &SdCard, folders: &[&str], loose: &[&str], label: String) -> Backup {
        let folders: Vec<String> = folders.iter().map(|f| f.to_string()).collect();
        let loose: Vec<String> = loose.iter().map(|f| f.to_string()).collect();
        let mut files = BTreeMap::new();
        for path in Self::paths(sd, &folders, &loose) {
            if let Some(bytes) = sd.read(&path) {
                files.insert(path, bytes);
            }
        }
        Backup { label, folders, loose, files }
    }

    /// The files that exist now in the folders, and the single files that exist.
    fn paths(sd: &SdCard, folders: &[String], loose: &[String]) -> Vec<String> {
        let mut paths: Vec<String> = folders.iter().flat_map(|folder| sd.files_in(folder)).collect();
        paths.extend(loose.iter().filter(|path| sd.read(path).is_some()).cloned());
        paths
    }

    /// How many files are not as they were: changed, removed or new.
    pub fn changes(&self, sd: &SdCard) -> usize {
        let now = Self::paths(sd, &self.folders, &self.loose);
        let new = now.iter().filter(|path| !self.files.contains_key(*path)).count();
        let different = self.files.iter().filter(|(path, bytes)| sd.read(path).as_deref() != Some(bytes.as_slice())).count();
        new + different
    }

    /// Puts the files back as they were, and tells the plugin about the files that were written. Gives the number of
    /// files that were changed back.
    pub fn restore(&self, sd: &SdCard) -> Result<usize, String> {
        let mut count = 0;
        for path in Self::paths(sd, &self.folders, &self.loose) {
            if !self.files.contains_key(&path) && sd.remove(&path)? {
                count += 1;
            }
        }
        for (path, bytes) in &self.files {
            if sd.read(path).as_deref() != Some(bytes.as_slice()) {
                sd.write(path, bytes)?;
                sd.mark_changed(path)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn card(name: &str) -> SdCard {
        let path = std::env::temp_dir().join(format!("sango_backup_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        SdCard::open(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn a_backup_puts_the_files_back() {
        let sd = card("put_back");
        sd.write("pokemon/data/1_0.bin", b"one").unwrap();
        sd.write("pokemon/data/2_0.bin", b"two").unwrap();
        let backup = Backup::snapshot(&sd, &["pokemon/data"], &[], "test".into());
        assert_eq!(backup.changes(&sd), 0);
        sd.remove("pokemon/data/1_0.bin").unwrap();
        sd.write("pokemon/data/2_0.bin", b"changed").unwrap();
        sd.write("pokemon/data/3_0.bin", b"new").unwrap();
        assert_eq!(backup.changes(&sd), 3);
        assert_eq!(backup.restore(&sd), Ok(3));
        assert_eq!(backup.changes(&sd), 0);
        assert_eq!(sd.read("pokemon/data/1_0.bin").as_deref(), Some(b"one".as_slice()));
        assert_eq!(sd.read("pokemon/data/2_0.bin").as_deref(), Some(b"two".as_slice()));
        assert!(sd.read("pokemon/data/3_0.bin").is_none());
        // The other folders are not touched.
        sd.write("pokemon/evolution/1_0.bin", b"e").unwrap();
        assert_eq!(backup.changes(&sd), 0);
    }
}
