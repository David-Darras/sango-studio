//! The files that older versions of the studio wrote by species and form (`pokemon/learnset/3_0.bin`, `pokemon/evolution/3_0.bin`) get
//! the name that the plugin reads: the place of the form in the archive (`species/level_up_moves/5.bin`). The other files change place
//! without the archive (see `SdCard`).

use sango_core::{evolution, file_lists, learnset, Garc, SdCard};

/// Moves the old files. Gives the number of files that were moved.
pub fn migrate(sd: &SdCard, species: &Garc) -> Result<usize, String> {
    let mut moved = 0;
    let by_form = |path: &str| -> Option<(usize, usize)> {
        let (number, form) = path.rsplit('/').next()?.strip_suffix(".bin")?.split_once('_')?;
        Some((number.parse().ok()?, form.parse().ok()?))
    };
    let folders: [(&str, fn(usize) -> String); 2] = [("pokemon/learnset", learnset::level_up_sd_path), ("pokemon/evolution", evolution::sd_path)];
    for (folder, new_path) in folders {
        for found in sd.files_in(folder) {
            let (Some((number, form)), Some(bytes)) = (by_form(&found), sd.read(&found)) else { continue };
            // A species of a pack has no file in the archive.
            if number <= crate::SPECIES.len() {
                let target = new_path(crate::file_index(species, number, form));
                if sd.read(&target).is_none() {
                    sd.write(&target, &bytes)?;
                }
            }
            sd.remove(&found)?;
            moved += 1;
        }
    }
    // The folders that the plugin reads have their list.
    for (folder, _) in file_lists::folders() {
        if !sd.files_in(&folder).is_empty() && sd.read(&format!("{folder}/index.bin")).is_none() {
            file_lists::write_index(sd, &folder)?;
        }
    }
    Ok(moved)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_files_by_species_and_form_get_the_number_of_their_file() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let species = Garc::open(&dump.archive(crate::ARCHIVE)).unwrap();
        let outer = std::env::temp_dir().join(format!("sango_old_layout_{}", std::process::id()));
        std::fs::create_dir_all(&outer).unwrap();
        let sd = SdCard::open(outer.to_str().unwrap()).unwrap();
        sd.write("pokemon/learnset/1_0.bin", &[1, 0, 1, 0, 255, 255, 255, 255]).unwrap();
        sd.write("pokemon/evolution/1_0.bin", &[7; 48]).unwrap();
        assert_eq!(migrate(&sd, &species).unwrap(), 2);
        let file = crate::file_index(&species, 1, 0);
        assert_eq!(sd.read(&learnset::level_up_sd_path(file)).unwrap(), vec![1, 0, 1, 0, 255, 255, 255, 255]);
        assert_eq!(sd.read(&evolution::sd_path(file)).unwrap(), vec![7; 48]);
        assert!(sd.files_in("pokemon").is_empty(), "the old files are gone");
        assert!(sd.read("species/level_up_moves/index.bin").is_some(), "the list of the folder is written");
        assert_eq!(migrate(&sd, &species).unwrap(), 0, "nothing to do the second time");
        let _ = std::fs::remove_dir_all(&outer);
    }
}
