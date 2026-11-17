//! Reads the archives (GARC files) of the game.

use std::path::Path;

/// Returns the files of an archive. Each file is a list of parts (one for each language that has one).
/// Most files have one part, so `files[3][0]` is the data of file 3.
pub fn read(path: &Path) -> Result<Vec<Vec<Vec<u8>>>, String> {
    let archive = sango_core::Garc::open(path)?;
    Ok((0..archive.file_count())
        .map(|id| archive.languages(id).into_iter().filter_map(|language| archive.file(id, language).map(|bytes| bytes.to_vec())).collect())
        .collect())
}

/// The archives of the game that the tool reads. Each one is a file of the
/// RomFS, found by its number: 13 is the file `a/0/1/3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archive {
    /// The maps: settings, events.
    Maps,
    /// The tilesets: pictures, light, models of the buildings.
    Tilesets,
    /// The 3D models of the characters.
    CharacterModels,
    /// For each character model: which model file it uses.
    CharacterModelParams,
    /// The 3D models of the doors, the trees and the other props that a block places.
    Props,
    /// The blocks of the map.
    Blocks,
    /// The matrices: which block is where.
    Matrices,
}

impl Archive {
    /// The number of the archive in the game.
    pub fn number(self) -> usize {
        match self {
            Archive::Maps => 13,
            Archive::Tilesets => 14,
            Archive::CharacterModels => 21,
            Archive::CharacterModelParams => 22,
            Archive::Props => 23,
            Archive::Blocks => 39,
            Archive::Matrices => 40,
        }
    }
}

/// The file of an archive in the dump of the RomFS: `dir` is the folder of the
/// dump, and the files are `a/0/1/3`, `a/0/3/9`...
pub fn locate(dir: &str, archive: Archive) -> std::path::PathBuf {
    sango_core::archive_path(Path::new(dir), archive.number())
}
