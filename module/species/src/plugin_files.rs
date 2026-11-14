//! The copies of the changed files in the form that the plugin reads (see `sango_core::archive_files`), for the files that were written
//! before the copies existed: they are made again when the studio opens the game files.

use sango_core::archive_files::{archive_of, folder, path, write_index};
use sango_core::{evolution, learnset, Garc, SdCard};
use std::collections::BTreeSet;

/// Makes the copies that are missing or old. Gives the number of files that were written.
pub fn sync(sd: &SdCard, species: &Garc) -> Result<usize, String> {
    let mut written = 0;
    let mut touched: BTreeSet<usize> = BTreeSet::new();
    let mut put = |archive: usize, file: usize, bytes: Vec<u8>| -> Result<(), String> {
        let target = path(archive, file);
        if sd.read(&target).as_deref() != Some(bytes.as_slice()) {
            sd.write(&target, &bytes)?;
            touched.insert(archive);
            written += 1;
        }
        Ok(())
    };
    // The files that have their number in their name.
    for folder_name in ["text", "pokemon/egg_moves", "pokemon/experience"] {
        for found in sd.files_in(folder_name) {
            if let (Some((archive, file)), Some(bytes)) = (archive_of(&found), sd.read(&found)) {
                put(archive, file, bytes)?;
            }
        }
    }
    // The files by species and form: the number of the file is the place of the form in the archive of the species.
    let by_form = |name: &str| -> Option<(usize, usize)> {
        let (species_number, form) = name.rsplit('/').next()?.strip_suffix(".bin")?.split_once('_')?;
        Some((species_number.parse().ok()?, form.parse().ok()?))
    };
    for (folder_name, archive) in [("pokemon/learnset", learnset::LEVEL_UP_ARCHIVE), ("pokemon/evolution", evolution::ARCHIVE)] {
        for found in sd.files_in(folder_name) {
            let (Some((number, form)), Some(bytes)) = (by_form(&found), sd.read(&found)) else { continue };
            if number > crate::SPECIES.len() {
                continue;
            }
            put(archive, crate::file_index(species, number, form), bytes)?;
        }
    }
    for archive in touched {
        write_index(sd, archive)?;
    }
    let _ = folder;
    Ok(written)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_files_written_before_the_copies_existed_get_their_copy() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let species = Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let outer = std::env::temp_dir().join(format!("sango_plugin_files_{}", std::process::id()));
        std::fs::create_dir_all(outer.join("sango-studio")).unwrap();
        let sd = SdCard::open(outer.to_str().unwrap()).unwrap();
        // Written straight on the disk, as the older versions did.
        let learn = outer.join("pokemon/learnset");
        std::fs::create_dir_all(&learn).unwrap();
        std::fs::write(learn.join("1_0.bin"), [1, 0, 1, 0, 255, 255, 255, 255]).unwrap();
        assert_eq!(sync(&sd, &species).unwrap(), 1);
        let file = crate::file_index(&species, 1, 0);
        assert_eq!(sd.read(&path(learnset::LEVEL_UP_ARCHIVE, file)).unwrap(), vec![1, 0, 1, 0, 255, 255, 255, 255]);
        assert_eq!(sync(&sd, &species).unwrap(), 0, "nothing to do the second time");
        let _ = std::fs::remove_dir_all(&outer);
    }
}
