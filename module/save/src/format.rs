//! The save file of the game: the file `main` of Alpha Sapphire and Omega Ruby.
//!
//! The file is not encrypted and not compressed. It has 58 blocks, one after the other, and each block starts at a multiple of
//! 512 bytes. At the end, a footer has the signature `BEEF` and, for each block, its size, its number and its checksum
//! (CRC-16/CCITT-FALSE). The game refuses a file whose checksum is wrong.

use std::path::{Path, PathBuf};

/// The size of the file.
pub const FILE_SIZE: usize = 0x76000;
/// The number of blocks.
pub const BLOCK_COUNT: usize = 58;
/// Where the footer starts, where its signature is, and where its list of the blocks starts.
const FOOTER: usize = 0x75E00;
const SIGNATURE_AT: usize = FOOTER + 0x10;
const LIST_AT: usize = FOOTER + 0x14;
const SIGNATURE: &[u8; 4] = b"FEEB";
const ALIGNMENT: usize = 0x200;
/// The number of copies of the file that the studio keeps.
const BACKUPS: usize = 30;

/// The blocks that the page reads and writes.
pub const ITEMS: usize = 1;
pub const POSITION: usize = 4;
pub const PLAY_TIME: usize = 6;
pub const MISC: usize = 11;
pub const BOX_NAMES: usize = 12;
pub const TRAINER: usize = 17;
pub const PARTY: usize = 18;
pub const OPTIONS: usize = 23;
pub const BOXES: usize = 56;

/// The sizes of these blocks: a file with other sizes is not a save of the game.
const SIZES: [(usize, usize); 9] = [
    (ITEMS, 0xB90),
    (POSITION, 0x150),
    (PLAY_TIME, 8),
    (MISC, 0x130),
    (BOX_NAMES, 0x440),
    (TRAINER, 0x170),
    (PARTY, 0x61C),
    (OPTIONS, 4),
    (BOXES, 0x34AD0),
];

/// The name of a block, for the messages.
pub fn block_name(block: usize) -> String {
    match block {
        ITEMS => "the Bag".to_string(),
        POSITION => "the place of the player".to_string(),
        PLAY_TIME => "the play time".to_string(),
        MISC => "the money and the badges".to_string(),
        BOX_NAMES => "the names of the boxes".to_string(),
        TRAINER => "the trainer".to_string(),
        PARTY => "the party".to_string(),
        OPTIONS => "the options".to_string(),
        BOXES => "the boxes".to_string(),
        other => format!("the block {other}"),
    }
}

/// The checksum of a block: CRC-16/CCITT-FALSE.
pub fn checksum(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0xFFFFu16, |mut crc, &byte| {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
        crc
    })
}

/// A save file in memory.
#[derive(Clone, PartialEq, Eq)]
pub struct Save {
    bytes: Vec<u8>,
    /// The offset and the size of each block.
    table: Vec<(usize, usize)>,
}

impl Save {
    /// Reads a file and checks it: the size, the footer and the checksum of every block.
    pub fn parse(bytes: Vec<u8>) -> Result<Save, String> {
        if bytes.len() != FILE_SIZE {
            return Err(format!("The file has {} bytes. The save of Alpha Sapphire and Omega Ruby has {FILE_SIZE} bytes.", bytes.len()));
        }
        if &bytes[SIGNATURE_AT..SIGNATURE_AT + 4] != SIGNATURE {
            return Err("The file has no footer: it is not a save of the game, or the game never saved in it.".into());
        }
        let mut table = Vec::with_capacity(BLOCK_COUNT);
        let mut offset = 0;
        for block in 0..BLOCK_COUNT {
            let entry = &bytes[LIST_AT + 8 * block..LIST_AT + 8 * block + 8];
            let size = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]) as usize;
            let number = u16::from_le_bytes([entry[4], entry[5]]) as usize;
            if number != block || size > FOOTER - offset {
                return Err(format!("The footer is wrong at the block {block}."));
            }
            table.push((offset, size));
            offset += size.div_ceil(ALIGNMENT) * ALIGNMENT;
        }
        for (block, size) in SIZES {
            if table[block].1 != size {
                return Err(format!("The size of {} is not the size of the game.", block_name(block)));
            }
        }
        let save = Save { bytes, table };
        let wrong: Vec<String> = (0..BLOCK_COUNT).filter(|&block| checksum(save.block(block)) != save.stored_checksum(block)).map(block_name).collect();
        if !wrong.is_empty() {
            return Err(format!("The checksum is wrong for {}. The game refuses this file.", wrong.join(", ")));
        }
        Ok(save)
    }

    /// Reads a file of the disk.
    pub fn read(path: &Path) -> Result<Save, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Save::parse(bytes)
    }

    fn stored_checksum(&self, block: usize) -> u16 {
        let at = LIST_AT + 8 * block + 6;
        u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]])
    }

    pub fn block(&self, block: usize) -> &[u8] {
        let (offset, size) = self.table[block];
        &self.bytes[offset..offset + size]
    }

    /// The bytes of a block, to change them. [`Save::to_bytes`] makes the checksums again.
    pub fn block_mut(&mut self, block: usize) -> &mut [u8] {
        let (offset, size) = self.table[block];
        &mut self.bytes[offset..offset + size]
    }

    /// The bytes of the file, with the checksum of every block.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.bytes.clone();
        for block in 0..BLOCK_COUNT {
            let at = LIST_AT + 8 * block + 6;
            bytes[at..at + 2].copy_from_slice(&checksum(self.block(block)).to_le_bytes());
        }
        bytes
    }

    /// The blocks that are not the same in the two saves.
    pub fn changed_blocks(&self, other: &Save) -> Vec<usize> {
        (0..BLOCK_COUNT).filter(|&block| self.block(block) != other.block(block)).collect()
    }
}

/// Writes the changes in the file of the disk, and keeps a copy of the file as it was.
///
/// `loaded` is the file as the studio read it, and `edited` the file with the changes of the user. The studio reads the file of the
/// disk again and changes only the blocks that the user changed, so what the game saved in the other blocks stays. When the game
/// saved one of the changed blocks after the read, nothing is written. Gives the file as it is now, and the changed blocks.
pub fn write_changes(path: &Path, loaded: &Save, edited: &Save, backups: &Path) -> Result<(Save, Vec<usize>), String> {
    let changed = edited.changed_blocks(loaded);
    let mut disk = Save::read(path)?;
    if changed.is_empty() {
        return Ok((disk, changed));
    }
    let conflicts: Vec<String> = changed.iter().filter(|&&block| disk.block(block) != loaded.block(block)).map(|&block| block_name(block)).collect();
    if !conflicts.is_empty() {
        return Err(format!(
            "The game saved {} after the studio read the file. Nothing is written. Press Read again, then make the change again.",
            conflicts.join(", ")
        ));
    }
    keep_a_copy(path, backups)?;
    for &block in &changed {
        let new = edited.block(block).to_vec();
        disk.block_mut(block).copy_from_slice(&new);
    }
    let bytes = disk.to_bytes();
    // The file is complete or it is the old one: it is written next to the file, then it takes its place.
    let part = path.with_extension("studio");
    std::fs::write(&part, &bytes).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((Save::parse(bytes)?, changed))
}

/// The copies of the file, the newest last.
pub fn copies(backups: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(backups)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "bak")).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// Copies the file of the disk in the folder of the copies, and removes the oldest copies.
pub fn keep_a_copy(path: &Path, backups: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(backups).map_err(|e| format!("{}: {e}", backups.display()))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let copy = backups.join(format!("main-{now:016}.bak"));
    std::fs::copy(path, &copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    let all = copies(backups);
    for old in all.iter().take(all.len().saturating_sub(BACKUPS)) {
        let _ = std::fs::remove_file(old);
    }
    Ok(copy)
}

/// A save with no data but a good footer, for the tests.
#[cfg(test)]
pub fn empty_for_test() -> Save {
    const ALL: [usize; BLOCK_COUNT] = [
        0x2C8, 0xB90, 0x2C, 0x38, 0x150, 0x4, 0x8, 0x1C0, 0xBE, 0x24, 0x2100, 0x130, 0x440, 0x574, 0x4E28, 0x4E28, 0x4E28, 0x170, 0x61C, 0x504,
        0x11CC, 0x644, 0x104, 0x4, 0x420, 0x64, 0x3F0, 0x70C, 0x180, 0x4, 0xC, 0x48, 0x54, 0x644, 0x5C8, 0x2F8, 0x1B40, 0x1F4, 0x3E0, 0x216,
        0x640, 0x1A90, 0x400, 0x618, 0x25C, 0x834, 0x318, 0x7D0, 0xC48, 0x78, 0x200, 0xC84, 0x628, 0x400, 0x7AD0, 0x78B0, 0x34AD0, 0xE058,
    ];
    let mut bytes = vec![0u8; FILE_SIZE];
    bytes[SIGNATURE_AT..SIGNATURE_AT + 4].copy_from_slice(SIGNATURE);
    let mut offset = 0;
    for (block, size) in ALL.into_iter().enumerate() {
        let entry = LIST_AT + 8 * block;
        bytes[entry..entry + 4].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[entry + 4..entry + 6].copy_from_slice(&(block as u16).to_le_bytes());
        let crc = checksum(&bytes[offset..offset + size]);
        bytes[entry + 6..entry + 8].copy_from_slice(&crc.to_le_bytes());
        offset += size.div_ceil(ALIGNMENT) * ALIGNMENT;
    }
    Save::parse(bytes).expect("the test save is good")
}

#[cfg(test)]
mod test {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("sango_save_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn the_checksum_is_the_one_of_the_game() {
        assert_eq!(checksum(b"123456789"), 0x29B1);
    }

    #[test]
    fn a_changed_block_gets_its_checksum() {
        let mut save = empty_for_test();
        let loaded = save.clone();
        save.block_mut(MISC)[8] = 100;
        assert_eq!(save.changed_blocks(&loaded), vec![MISC]);
        let again = Save::parse(save.to_bytes()).unwrap();
        assert_eq!(again.block(MISC)[8], 100);
        // A byte that changes without its checksum is found.
        let mut bad = save.to_bytes();
        bad[0] ^= 1;
        assert!(Save::parse(bad).err().unwrap().contains("checksum is wrong for the block 0"));
    }

    #[test]
    fn a_wrong_file_is_refused() {
        assert!(Save::parse(vec![0; 100]).is_err());
        assert!(Save::parse(vec![0; FILE_SIZE]).is_err());
    }

    #[test]
    fn only_the_changed_blocks_are_written() {
        let folder = folder("write");
        let (path, backups) = (folder.join("main"), folder.join("copies"));
        let loaded = empty_for_test();
        std::fs::write(&path, loaded.to_bytes()).unwrap();
        // The game saves its play time after the studio read the file.
        let mut game = loaded.clone();
        game.block_mut(PLAY_TIME)[0] = 7;
        std::fs::write(&path, game.to_bytes()).unwrap();
        let mut edited = loaded.clone();
        edited.block_mut(MISC)[8] = 5;
        let (now, changed) = write_changes(&path, &loaded, &edited, &backups).unwrap();
        assert_eq!(changed, vec![MISC]);
        assert_eq!((now.block(MISC)[8], now.block(PLAY_TIME)[0]), (5, 7));
        assert_eq!(Save::read(&path).unwrap().block(PLAY_TIME)[0], 7);
        // The copy is the file as it was.
        assert_eq!(Save::read(&copies(&backups)[0]).unwrap().block(MISC)[8], 0);
        // The game saved the same block: nothing is written.
        edited.block_mut(PLAY_TIME)[0] = 9;
        assert!(write_changes(&path, &loaded, &edited, &backups).err().unwrap().contains("play time"));
        assert_eq!(Save::read(&path).unwrap().block(PLAY_TIME)[0], 7);
        let _ = std::fs::remove_dir_all(folder);
    }
}
