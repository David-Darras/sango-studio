//! The GARC format: the archives of the game.
//!
//! An archive holds many files. Each file has a number: its file id.
//! A file can have one version for each language.
//!
//! An archive has four blocks, one after the other:
//!
//! | Block | Signature | What it holds |
//! | --- | --- | --- |
//! | Header | `GARC` | The version, the sizes and the offset of the data. |
//! | Offset table | `FATO` | For each file id: where its entry is in the range table. |
//! | Range table | `FATB` | For each file id: its languages, and the range of each one. |
//! | Data block | `FIMB` | The bytes of all the files. |
//!
//! All the numbers are little-endian.
//! The signature of a block is a number. Its bytes read backward: `GARC` is stored as `CRAG`.
//!
//! # The header (28 bytes)
//!
//! | Offset | Size | Name | Meaning |
//! | --- | --- | --- | --- |
//! | 0 | 4 | signature | `CRAG` |
//! | 4 | 4 | header size | The size of this block: 28. |
//! | 8 | 2 | byte order | `0xFEFF` for little-endian. |
//! | 10 | 2 | version | `0x0400` for the archives of the game. |
//! | 12 | 2 | block count | The number of blocks after the header: 4 in the archives of the game. |
//! | 14 | 2 | padding | Not used. |
//! | 16 | 4 | data offset | The offset of the first byte of the data, from the start of the archive. |
//! | 20 | 4 | archive size | The size of the whole archive. |
//! | 24 | 4 | largest file size | The biggest real size of a file, without the padding. |
//!
//! # The offset table
//!
//! A header of 12 bytes (signature, block size, number of file ids, padding), then one `u32` for each
//! file id. The `u32` is an offset. It starts after the header of the range table.
//!
//! # The range table
//!
//! A header of 12 bytes (signature, block size, number of ranges), then the entries.
//! An entry starts with a `u32`: the language mask. One bit is one language.
//! After the mask, there is one range for each bit that is set, in the order of the bits.
//! A range has three `u32`:
//!
//! | Name | Meaning |
//! | --- | --- |
//! | start | The offset of the first byte of the file, from the data offset. |
//! | end | The offset after the last byte of the file. It includes the padding. |
//! | real size | The size of the file, without the padding. |
//!
//! # The data block
//!
//! A header of 12 bytes (signature, block size, size of the data), then the data.
//! The data offset of the archive header points to the first byte of the data.
//!
//! Only version `0x0400` is supported. The game also has older archive versions that do not have
//! the real size in the ranges.

use std::path::Path;

/// The number of languages that a file id can have: one bit each.
pub const LANGUAGE_COUNT: usize = 32;

/// The only version of the format that this code reads.
pub const SUPPORTED_VERSION: u16 = 0x0400;

const SIGNATURE: &[u8; 4] = b"CRAG";
const OFFSET_TABLE_SIGNATURE: &[u8; 4] = b"OTAF";
const RANGE_TABLE_SIGNATURE: &[u8; 4] = b"BTAF";
const DATA_BLOCK_SIGNATURE: &[u8; 4] = b"BMIF";

/// The byte order mark of a little-endian archive.
const BYTE_ORDER: u16 = 0xFEFF;

/// The size of the header of the range table: signature, block size, number of ranges.
const RANGE_TABLE_HEADER_SIZE: usize = 12;

/// The size of the header of the data block: signature, block size, size of the data.
const DATA_BLOCK_HEADER_SIZE: usize = 12;

/// The size of one range: start, end and real size.
const RANGE_SIZE: usize = 12;

/// The place of the bytes of one file.
///
/// The offsets start at the data offset of the archive, not at the start of the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRange {
    /// The offset of the first byte of the file.
    pub start: u32,
    /// The offset after the last byte of the file. It includes the padding.
    pub end: u32,
    /// The size of the file in bytes, without the padding.
    pub real_size: u32,
}

impl FileRange {
    /// Returns the size of the file with its padding.
    pub fn padded_size(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns the size of the padding after the file.
    pub fn padding(&self) -> u32 {
        self.padded_size().saturating_sub(self.real_size)
    }
}

/// What the range table says about one file id.
#[derive(Clone, Debug)]
struct Entry {
    /// One bit for each language that has a file for this id.
    language_mask: u32,
    /// One range for each set bit, in the order of the bits.
    ranges: Vec<FileRange>,
}

impl Entry {
    /// Returns the range of a language.
    ///
    /// The place of the range is the number of set bits below the language.
    fn range(&self, language: usize) -> Option<FileRange> {
        if language >= LANGUAGE_COUNT || self.language_mask & (1 << language) == 0 {
            return None;
        }
        let below = (self.language_mask & ((1u32 << language) - 1)).count_ones() as usize;
        self.ranges.get(below).copied()
    }
}

/// An archive of the game, in memory.
#[derive(Debug)]
pub struct Garc {
    data: Vec<u8>,
    version: u16,
    header_size: usize,
    block_count: u16,
    archive_size: u32,
    data_offset: usize,
    largest_file_size: u32,
    offset_table_size: usize,
    range_table_size: usize,
    entries: Vec<Entry>,
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The archive stops too early (at {at:#x})."))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The archive stops too early (at {at:#x})."))
}

impl Garc {
    /// Reads an archive from its bytes.
    ///
    /// The function checks the signatures, the version and the place of every file.
    pub fn parse(data: Vec<u8>) -> Result<Garc, String> {
        if data.is_empty() {
            return Err("The archive file is empty.".into());
        }
        if data.get(..4) != Some(SIGNATURE) {
            return Err("This is not an archive of the game.".into());
        }
        let header_size = u32_at(&data, 4)? as usize;
        if u16_at(&data, 8)? != BYTE_ORDER {
            return Err("This archive is not little-endian.".into());
        }
        let version = u16_at(&data, 10)?;
        if version != SUPPORTED_VERSION {
            return Err(format!("This archive has version {version:#06x}. Only version {SUPPORTED_VERSION:#06x} is supported."));
        }
        let block_count = u16_at(&data, 12)?;
        let data_offset = u32_at(&data, 16)? as usize;
        let archive_size = u32_at(&data, 20)?;
        let largest_file_size = u32_at(&data, 24)?;

        // The offset table.
        let offset_table = header_size;
        if data.get(offset_table..offset_table + 4) != Some(OFFSET_TABLE_SIGNATURE) {
            return Err("The offset table of the archive is missing.".into());
        }
        let offset_table_size = u32_at(&data, offset_table + 4)? as usize;
        let id_count = u16_at(&data, offset_table + 8)? as usize;
        let mut offsets = Vec::with_capacity(id_count);
        for i in 0..id_count {
            offsets.push(u32_at(&data, offset_table + 12 + 4 * i)? as usize);
        }

        // The range table.
        let range_table = offset_table + offset_table_size;
        if data.get(range_table..range_table + 4) != Some(RANGE_TABLE_SIGNATURE) {
            return Err("The range table of the archive is missing.".into());
        }
        let range_table_size = u32_at(&data, range_table + 4)? as usize;
        let first_entry = range_table + RANGE_TABLE_HEADER_SIZE;
        let mut entries = Vec::with_capacity(id_count);
        for offset in offsets {
            let mut at = first_entry + offset;
            let language_mask = u32_at(&data, at)?;
            at += 4;
            let mut ranges = Vec::with_capacity(language_mask.count_ones() as usize);
            for _ in 0..language_mask.count_ones() {
                ranges.push(FileRange { start: u32_at(&data, at)?, end: u32_at(&data, at + 4)?, real_size: u32_at(&data, at + 8)? });
                at += RANGE_SIZE;
            }
            entries.push(Entry { language_mask, ranges });
        }

        // The data block. Every file must be inside the archive.
        let header_at = data_offset.checked_sub(DATA_BLOCK_HEADER_SIZE);
        if header_at.and_then(|at| data.get(at..at + 4)) != Some(DATA_BLOCK_SIGNATURE) {
            return Err("The data block of the archive is missing.".into());
        }
        for (id, entry) in entries.iter().enumerate() {
            for range in &entry.ranges {
                let from = data_offset + range.start as usize;
                if data.get(from..from + range.real_size as usize).is_none() {
                    return Err(format!("The file {id} is outside the archive."));
                }
            }
        }
        Ok(Garc { data, version, header_size, block_count, archive_size, data_offset, largest_file_size, offset_table_size, range_table_size, entries })
    }

    /// Reads an archive from a file.
    pub fn open(path: &Path) -> Result<Garc, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Garc::parse(data).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Returns the version of the format.
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Returns the byte order mark: `0xFEFF` for little-endian.
    pub fn byte_order(&self) -> u16 {
        BYTE_ORDER
    }

    /// Returns the size of the header block.
    pub fn header_size(&self) -> usize {
        self.header_size
    }

    /// Returns the number of blocks after the header, as the header says.
    pub fn block_count(&self) -> u16 {
        self.block_count
    }

    /// Returns the size of the archive, as the header says.
    pub fn archive_size(&self) -> u32 {
        self.archive_size
    }

    /// Returns the number of bytes that the archive has in memory.
    pub fn bytes_len(&self) -> usize {
        self.data.len()
    }

    /// Returns the offset of the first byte of the data, from the start of the archive.
    pub fn data_offset(&self) -> usize {
        self.data_offset
    }

    /// Returns the biggest real size of a file, without the padding.
    pub fn largest_file_size(&self) -> u32 {
        self.largest_file_size
    }

    /// Returns the size of the offset table block.
    pub fn offset_table_size(&self) -> usize {
        self.offset_table_size
    }

    /// Returns the size of the range table block.
    pub fn range_table_size(&self) -> usize {
        self.range_table_size
    }

    /// Returns the number of file ids.
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the number of ranges: one for each language of each file id.
    pub fn range_count(&self) -> usize {
        self.entries.iter().map(|e| e.ranges.len()).sum()
    }

    /// Returns the language mask of a file id. One bit is one language.
    pub fn language_mask(&self, id: usize) -> Option<u32> {
        self.entries.get(id).map(|e| e.language_mask)
    }

    /// Returns the numbers of the languages that a file id has.
    pub fn languages(&self, id: usize) -> Vec<usize> {
        let mask = self.language_mask(id).unwrap_or(0);
        (0..LANGUAGE_COUNT).filter(|bit| mask & (1 << bit) != 0).collect()
    }

    /// Returns the range of a file for a language.
    ///
    /// The result is `None` when the file id has no file for this language.
    pub fn range(&self, id: usize, language: usize) -> Option<FileRange> {
        self.entries.get(id)?.range(language)
    }

    /// Returns the range of a file for a language, as the game does.
    ///
    /// When the language has no file, the game uses the first file of the id.
    pub fn range_or_first(&self, id: usize, language: usize) -> Option<FileRange> {
        let entry = self.entries.get(id)?;
        entry.range(language).or_else(|| entry.ranges.first().copied())
    }

    /// Returns the real size of a file in bytes.
    pub fn file_size(&self, id: usize, language: usize) -> Option<usize> {
        self.range(id, language).map(|r| r.real_size as usize)
    }

    /// Returns the bytes of a file for a language.
    ///
    /// The file can be compressed.
    pub fn file(&self, id: usize, language: usize) -> Option<&[u8]> {
        self.bytes_of(self.range(id, language)?)
    }

    /// Returns the bytes of a file for a language, as the game does.
    ///
    /// When the language has no file, the function returns the first file of the id.
    pub fn file_or_first(&self, id: usize, language: usize) -> Option<&[u8]> {
        self.bytes_of(self.range_or_first(id, language)?)
    }

    /// Returns the bytes of a file in the first language that it has.
    pub fn first_file(&self, id: usize) -> Option<&[u8]> {
        let mask = self.language_mask(id)?;
        if mask == 0 {
            return None;
        }
        self.file(id, mask.trailing_zeros() as usize)
    }

    fn bytes_of(&self, range: FileRange) -> Option<&[u8]> {
        let from = self.data_offset + range.start as usize;
        self.data.get(from..from + range.real_size as usize)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Dump;

    /// Builds an archive: one list of (language, bytes) for each file id.
    fn build(files: &[Vec<(usize, Vec<u8>)>]) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut entries = Vec::new();
        let mut data = Vec::new();
        for file in files {
            offsets.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            let mut mask = 0u32;
            for (language, _) in file {
                mask |= 1 << language;
            }
            entries.extend_from_slice(&mask.to_le_bytes());
            let mut sorted: Vec<&(usize, Vec<u8>)> = file.iter().collect();
            sorted.sort_by_key(|(language, _)| *language);
            for (_, bytes) in sorted {
                let start = data.len() as u32;
                data.extend_from_slice(bytes);
                while data.len() % 4 != 0 {
                    data.push(0);
                }
                entries.extend_from_slice(&start.to_le_bytes());
                entries.extend_from_slice(&(data.len() as u32).to_le_bytes());
                entries.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
        }
        let header_size = 28u32;
        let offset_table_size = 12 + offsets.len() as u32;
        let range_table_size = 12 + entries.len() as u32;
        let data_offset = header_size + offset_table_size + range_table_size + 12;
        let mut out = Vec::new();
        out.extend_from_slice(b"CRAG");
        out.extend_from_slice(&header_size.to_le_bytes());
        out.extend_from_slice(&0xFEFFu16.to_le_bytes());
        out.extend_from_slice(&0x0400u16.to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&data_offset.to_le_bytes());
        out.extend_from_slice(&(data_offset + data.len() as u32).to_le_bytes());
        out.extend_from_slice(&files.iter().flatten().map(|(_, b)| b.len() as u32).max().unwrap_or(0).to_le_bytes());
        out.extend_from_slice(b"OTAF");
        out.extend_from_slice(&offset_table_size.to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&0xFFFFu16.to_le_bytes());
        out.extend_from_slice(&offsets);
        out.extend_from_slice(b"BTAF");
        out.extend_from_slice(&range_table_size.to_le_bytes());
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        out.extend_from_slice(&entries);
        out.extend_from_slice(b"BMIF");
        out.extend_from_slice(&12u32.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn files_are_read_back() {
        let bytes = build(&[vec![(0, b"first".to_vec())], vec![(0, b"second file".to_vec())], vec![(0, Vec::new())]]);
        let garc = Garc::parse(bytes).unwrap();
        assert_eq!(garc.file_count(), 3);
        assert_eq!(garc.version(), 0x0400);
        assert_eq!(garc.file(0, 0), Some(&b"first"[..]));
        assert_eq!(garc.file(1, 0), Some(&b"second file"[..]));
        assert_eq!(garc.file_size(1, 0), Some(11));
        assert_eq!(garc.file(2, 0), Some(&b""[..]));
        assert_eq!(garc.largest_file_size(), 11);
    }

    #[test]
    fn the_padding_is_not_part_of_the_file() {
        let garc = Garc::parse(build(&[vec![(0, b"abcde".to_vec())]])).unwrap();
        let range = garc.range(0, 0).unwrap();
        assert_eq!(range.real_size, 5);
        assert_eq!(range.padded_size(), 8);
        assert_eq!(range.padding(), 3);
        assert_eq!(garc.file(0, 0).unwrap().len(), 5);
    }

    #[test]
    fn a_file_with_several_languages_has_one_range_for_each() {
        let bytes = build(&[vec![(0, b"en".to_vec()), (2, b"fr".to_vec()), (5, b"de".to_vec())]]);
        let garc = Garc::parse(bytes).unwrap();
        assert_eq!(garc.language_mask(0), Some(0b100101));
        assert_eq!(garc.languages(0), vec![0, 2, 5]);
        assert_eq!(garc.file(0, 0), Some(&b"en"[..]));
        assert_eq!(garc.file(0, 2), Some(&b"fr"[..]));
        assert_eq!(garc.file(0, 5), Some(&b"de"[..]));
        assert_eq!(garc.file(0, 1), None);
        assert_eq!(garc.first_file(0), Some(&b"en"[..]));
        assert_eq!(garc.range_count(), 3);
    }

    #[test]
    fn a_missing_language_uses_the_first_file_like_the_game() {
        let garc = Garc::parse(build(&[vec![(0, b"en".to_vec()), (2, b"fr".to_vec())]])).unwrap();
        assert_eq!(garc.file(0, 1), None);
        assert_eq!(garc.file_or_first(0, 1), Some(&b"en"[..]));
        assert_eq!(garc.file_or_first(0, 2), Some(&b"fr"[..]));
        assert_eq!(garc.file_or_first(5, 0), None);
    }

    #[test]
    fn a_missing_file_gives_none() {
        let garc = Garc::parse(build(&[vec![(0, b"x".to_vec())]])).unwrap();
        assert_eq!(garc.file(1, 0), None);
        assert_eq!(garc.file(0, 40), None);
    }

    #[test]
    fn a_bad_archive_is_refused() {
        assert!(Garc::parse(Vec::new()).unwrap_err().contains("empty"));
        assert!(Garc::parse(b"not an archive".to_vec()).is_err());
        let mut bytes = build(&[vec![(0, b"data".to_vec())]]);
        bytes.truncate(bytes.len() - 6);
        assert!(Garc::parse(bytes).is_err());
        let mut bytes = build(&[vec![(0, b"data".to_vec())]]);
        bytes[0] = b'X';
        assert!(Garc::parse(bytes).is_err());
    }

    #[test]
    fn an_old_version_is_refused() {
        let mut bytes = build(&[vec![(0, b"data".to_vec())]]);
        bytes[10..12].copy_from_slice(&0x0300u16.to_le_bytes());
        assert!(Garc::parse(bytes).unwrap_err().contains("0x0400"));
    }

    /// With the dump of the game: set `SANGO_DUMP` to the folder that has the folder `a`.
    ///
    /// The test checks what the format says on every archive of the dump.
    #[test]
    fn every_archive_of_the_dump_has_the_known_layout() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let mut checked = 0;
        for number in dump.archive_numbers() {
            let bytes = std::fs::read(dump.archive(number)).unwrap();
            if bytes.is_empty() {
                continue; // The dump has one empty archive file.
            }
            let garc = Garc::parse(bytes.clone()).unwrap_or_else(|e| panic!("archive {number}: {e}"));
            assert_eq!(garc.version(), SUPPORTED_VERSION, "archive {number}");
            // The data starts after the header, the two tables and the header of the data block.
            let before_data = garc.header_size() + garc.offset_table_size() + garc.range_table_size() + DATA_BLOCK_HEADER_SIZE;
            assert_eq!(garc.data_offset(), before_data, "archive {number}");
            assert_eq!(garc.archive_size() as usize, bytes.len(), "archive {number}");
            // The largest size is the biggest real size, without the padding.
            let mut biggest = 0;
            for id in 0..garc.file_count() {
                for language in garc.languages(id) {
                    let range = garc.range(id, language).unwrap();
                    assert!(range.end >= range.start + range.real_size, "archive {number} file {id}");
                    biggest = biggest.max(range.real_size);
                }
            }
            assert_eq!(garc.largest_file_size(), biggest, "archive {number}");
            checked += 1;
        }
        assert!(checked > 100, "only {checked} archives checked");
    }
}
