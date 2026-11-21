//! A bundle: one file of a layout archive of the game.
//!
//! The files of these archives are usually compressed (LZ11). Once decompressed, a bundle is:
//!
//! | Part | Size | Meaning |
//! | --- | --- | --- |
//! | resource count `N` | 4 | |
//! | resource names | `N` × 64 | The files of the DARC, by name (`params.bclyt`, `back.bclim`...). |
//! | pane count `P` | 4 | |
//! | pane names | `P` × 32 | The table of the names of the panes. The game finds a pane by its place in this table. |
//! | padding | | Up to a multiple of 128 bytes. |
//! | DARC archive | | See [`crate::darc`]. |

use crate::darc::Darc;
use sango_core::lz;

const NAME: usize = 64;
const PANE: usize = 32;
const ALIGN: usize = 128;

/// A bundle in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    names: Vec<[u8; NAME]>,
    panes: Vec<[u8; PANE]>,
    pub darc: Darc,
    /// True when the file of the archive was compressed (the game needs it to stay so).
    pub compressed: bool,
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.split(|&b| b == 0).next().unwrap_or(&[])).into_owned()
}

impl Bundle {
    /// Reads a file of an archive (compressed or not). Returns an error when the file is not a bundle.
    pub fn parse(file: &[u8]) -> Result<Bundle, String> {
        let compressed = lz::is_compressed(file);
        let raw = if compressed { lz::decompress(file)? } else { file.to_vec() };
        let count = u32::from_le_bytes(raw.get(..4).ok_or("Too short.")?.try_into().unwrap()) as usize;
        if count == 0 || count > 400 || raw.len() < 4 + count * NAME + 4 {
            return Err("This file is not a layout bundle.".into());
        }
        let names: Vec<[u8; NAME]> = (0..count).map(|i| raw[4 + i * NAME..][..NAME].try_into().unwrap()).collect();
        if !names.iter().any(|n| n.starts_with(b".") || text(n).ends_with(".bclyt") || text(n).ends_with(".bclim")) {
            return Err("This file is not a layout bundle.".into());
        }
        let mut at = 4 + count * NAME;
        let pane_count = u32::from_le_bytes(raw[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        if raw.len() < at + pane_count * PANE {
            return Err("The table of the panes is too short.".into());
        }
        let panes: Vec<[u8; PANE]> = (0..pane_count).map(|i| raw[at + i * PANE..][..PANE].try_into().unwrap()).collect();
        at = (at + pane_count * PANE).div_ceil(ALIGN) * ALIGN;
        let darc = Darc::parse(raw.get(at..).ok_or("There is no archive.")?)?;
        Ok(Bundle { names, panes, darc, compressed })
    }

    /// The names of the resources.
    pub fn resource_names(&self) -> Vec<String> {
        self.names.iter().map(|n| text(n)).collect()
    }

    /// The table of the names of the panes.
    pub fn pane_names(&self) -> Vec<String> {
        self.panes.iter().map(|n| text(n)).collect()
    }

    /// The names of the layouts (`.bclyt` files) of the bundle.
    pub fn layouts(&self) -> Vec<String> {
        self.darc.files.iter().filter_map(|f| f.path.rsplit('/').next()).filter(|n| n.ends_with(".bclyt")).map(String::from).collect()
    }

    /// Adds a name at the end of the table of the panes. The names that exist keep their place.
    pub fn add_pane_name(&mut self, name: &str) {
        let mut entry = [0u8; PANE];
        let bytes = name.as_bytes();
        entry[..bytes.len().min(PANE - 1)].copy_from_slice(&bytes[..bytes.len().min(PANE - 1)]);
        self.panes.push(entry);
    }

    /// The bytes that the bundle has when decompressed.
    pub fn to_raw(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.names.len() as u32).to_le_bytes());
        for n in &self.names {
            out.extend_from_slice(n);
        }
        out.extend_from_slice(&(self.panes.len() as u32).to_le_bytes());
        for p in &self.panes {
            out.extend_from_slice(p);
        }
        while out.len() % ALIGN != 0 {
            out.push(0);
        }
        out.extend_from_slice(&self.darc.to_bytes());
        out
    }

    /// The bytes of the file of the archive: compressed again when it was.
    pub fn to_bytes(&self) -> Vec<u8> {
        let raw = self.to_raw();
        if self.compressed {
            lz::compress_lz11(&raw)
        } else {
            raw
        }
    }
}
