//! The tags of the languages that the game shows in the summary and in the PC boxes when a Pokémon comes from a game of another
//! language: `JPN`, `ENG`, `FRA`, `ITA`, `GER`, `SPA`, `KOR` (40 × 20 pixels).
//!
//! They are pictures of the layout bundle 0 of the archive [`ARCHIVE`] (`timg/<tag>.bclim`). A layout bundle is a list of names, a
//! list of panes, then a DARC archive: a table of nodes of 12 bytes (bits 0-23 of the first word: the place of the name in the
//! names that follow the table, bit 24: a folder; for a file: its offset and its size), then the names in UTF-16.

use crate::{Clim, Dump, Garc};

/// The archive of the layouts of the summary.
pub const ARCHIVE: usize = 212;

/// The name of the picture of each language, in the order of [`crate::text::LANGUAGES`] (both Japanese have the tag `jpn`).
pub const TAGS: [&str; 8] = ["jpn", "jpn", "eng", "fra", "ita", "ger", "spa", "kor"];

/// A picture: its width, its height and its RGBA pixels.
pub type Picture = (usize, usize, Vec<u8>);

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The bytes of a file of a DARC archive, found by the last part of its name.
fn darc_file<'a>(darc: &'a [u8], wanted: &str) -> Option<&'a [u8]> {
    if darc.get(..4) != Some(b"darc") {
        return None;
    }
    let table = u32_at(darc, 0x10)? as usize;
    let count = u32_at(darc, table + 8)? as usize;
    let names = table + count * 12;
    for node in 1..count {
        let a = u32_at(darc, table + node * 12)?;
        if a >> 24 == 1 {
            continue;
        }
        let mut name = String::new();
        let mut at = names + (a & 0xFF_FFFF) as usize;
        while let Some(pair) = darc.get(at..at + 2) {
            let unit = u16::from_le_bytes([pair[0], pair[1]]);
            if unit == 0 {
                break;
            }
            name.push(char::from_u32(unit as u32).unwrap_or('?'));
            at += 2;
        }
        if name == wanted {
            let (offset, size) = (u32_at(darc, table + node * 12 + 4)? as usize, u32_at(darc, table + node * 12 + 8)? as usize);
            return darc.get(offset..offset + size);
        }
    }
    None
}

/// The tags of the eight languages, read from the dump. A tag that is not found is `None`.
pub fn load(dump: &Dump) -> Vec<Option<Picture>> {
    let raw = Garc::open(&dump.archive(ARCHIVE)).ok().and_then(|garc| garc.first_file_decompressed(0));
    let darc = raw.as_deref().and_then(|raw| raw.windows(4).position(|w| w == b"darc").map(|at| &raw[at..]));
    TAGS.iter()
        .map(|tag| {
            let bytes = darc_file(darc?, &format!("{tag}.bclim"))?;
            let image = Clim::parse(bytes).ok()?;
            Some((image.width(), image.height(), image.to_rgba().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod test {
    /// With the dump of the game: the seven tags are found, with their size.
    #[test]
    fn the_tags_of_the_languages_are_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let tags = super::load(&dump);
        assert!(tags.iter().all(|tag| matches!(tag, Some((40, 20, pixels)) if pixels.len() == 40 * 20 * 4)));
    }
}
