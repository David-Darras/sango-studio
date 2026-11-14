//! A BCH file of the game that the importer takes materials and metadata from (a *template*): a reader of the structures that the writer
//! clones.

use std::collections::HashMap;

/// A BCH file, read.
pub struct Template {
    pub d: Vec<u8>,
    /// The start of the contents, of the strings and of the commands.
    pub c: usize,
    pub s: usize,
    pub cm: usize,
    /// The places of the contents that are pointers: the offset of the pointer and its kind (1 = a string).
    pub relocs: HashMap<usize, u32>,
}

impl Template {
    pub fn open(bytes: &[u8]) -> Result<Template, String> {
        if bytes.len() < 0x44 || !bytes.starts_with(b"BCH") {
            return Err("This is not a BCH file.".into());
        }
        let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
        let (c, s, cm, r, r_len) = (u32_at(8), u32_at(12), u32_at(16), u32_at(28), u32_at(52));
        let mut relocs = HashMap::new();
        for i in 0..r_len / 4 {
            let v = u32::from_le_bytes(bytes.get(r + 4 * i..r + 4 * i + 4).ok_or("The relocation table is cut.")?.try_into().unwrap());
            let (kind, location) = (v >> 25, (v & 0x1FF_FFFF) as usize);
            if kind < 0x20 {
                relocs.insert(if kind == 1 { location } else { location * 4 }, kind);
            }
        }
        Ok(Template { d: bytes.to_vec(), c, s, cm, relocs })
    }

    /// The word at an offset of the contents.
    pub fn c32(&self, offset: usize) -> usize {
        let at = self.c + offset;
        u32::from_le_bytes([self.d[at], self.d[at + 1], self.d[at + 2], self.d[at + 3]]) as usize
    }

    /// The text at an offset of the strings.
    pub fn string(&self, offset: usize) -> String {
        self.d[self.s + offset..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect()
    }

    /// The `count` words of the list of commands at an offset.
    pub fn cmd(&self, offset: usize, count: usize) -> Vec<u32> {
        (0..count)
            .map(|i| {
                let at = self.cm + offset + 4 * i;
                u32::from_le_bytes([self.d[at], self.d[at + 1], self.d[at + 2], self.d[at + 3]])
            })
            .collect()
    }

    /// The metadata of a map at an offset of the contents: for each entry, the name, the type and the words.
    pub fn metadata(&self, map_off: usize) -> Vec<(String, u32, Vec<u32>)> {
        let c = |offset: usize| self.c32(offset);
        let (p, count) = (c(map_off), c(map_off + 4));
        (0..count)
            .map(|i| {
                let e = p + 12 * i;
                let name = self.string(c(e));
                let kind = (c(e + 4) & 0xFF) as u32;
                let word_count = c(e + 4) >> 16;
                let data = c(e + 8);
                (name, kind, (0..word_count).map(|k| c(data + 4 * k) as u32).collect())
            })
            .collect()
    }
}
