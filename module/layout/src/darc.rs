//! The DARC archive: the container of the files of a layout (the layout itself, its animations and its images).
//!
//! All the numbers are little-endian in the layouts of the game.
//!
//! | Offset | Size | Name | Meaning |
//! | --- | --- | --- | --- |
//! | 0x00 | 4 | signature | `darc` |
//! | 0x04 | 2 | byte order | `FF FE` |
//! | 0x06 | 2 | header size | |
//! | 0x08 | 4 | version | |
//! | 0x0C | 4 | file size | |
//! | 0x10 | 4 | table offset | The table of the nodes. |
//! | 0x14 | 4 | table size | The nodes, then the names. |
//! | 0x18 | 4 | data offset | |
//!
//! A node has 12 bytes: the type (1 for a folder) in the high byte and the offset of the name in the low 3 bytes, then for a file the
//! offset and the size of its data, for a folder the index of its parent and the index of the node after its last child. The names are
//! UTF-16. Node 0 is the root.
//!
//! The data of the files go in the same order as the nodes, each one aligned. [`Darc::to_bytes`] writes the same bytes as the file that
//! was read when no file changed.

/// A file of the archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// The index of the node of the file.
    pub node: usize,
    /// The path, for example `./timg/back.bclim`.
    pub path: String,
    pub data: Vec<u8>,
    /// The alignment of the data in the archive (the file had it, so it keeps it).
    align: usize,
}

/// An archive in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Darc {
    /// The bytes before the data (the header, the nodes and the names).
    head: Vec<u8>,
    /// The number of bytes of the file.
    total: usize,
    pub files: Vec<File>,
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| "The archive is too short.".to_string())
}

fn name_at(b: &[u8], mut at: usize) -> String {
    let mut units = Vec::new();
    while at + 2 <= b.len() {
        let unit = u16::from_le_bytes([b[at], b[at + 1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
        at += 2;
    }
    String::from_utf16_lossy(&units)
}

/// The alignment of an offset: the biggest power of 2, from 4 to 256, that divides it.
fn alignment_of(offset: usize) -> usize {
    let mut align = 256;
    while align > 4 && offset % align != 0 {
        align /= 2;
    }
    align
}

impl Darc {
    /// Reads an archive.
    pub fn parse(bytes: &[u8]) -> Result<Darc, String> {
        if bytes.len() < 0x1C || &bytes[..4] != b"darc" {
            return Err("This is not a DARC archive.".into());
        }
        if bytes[4..6] != [0xFF, 0xFE] {
            return Err("This DARC archive is big-endian, which the layouts of the game never are.".into());
        }
        let table = u32_at(bytes, 0x10)? as usize;
        let data_offset = u32_at(bytes, 0x18)? as usize;
        let total = (u32_at(bytes, 0x0C)? as usize).min(bytes.len());
        if data_offset > bytes.len() || table + 12 > data_offset {
            return Err("The table of the archive is wrong.".into());
        }
        let count = u32_at(bytes, table + 8)? as usize;
        let names = table + count * 12;
        if count == 0 || names > data_offset {
            return Err("The table of the archive is wrong.".into());
        }
        // Walk the tree to give a path to each file.
        let mut files = Vec::new();
        let mut stack: Vec<(usize, String)> = vec![(count, String::new())];
        for node in 1..count {
            while stack.len() > 1 && node >= stack.last().unwrap().0 {
                stack.pop();
            }
            let a = u32_at(bytes, table + node * 12)?;
            let b = u32_at(bytes, table + node * 12 + 4)? as usize;
            let c = u32_at(bytes, table + node * 12 + 8)? as usize;
            let name = name_at(bytes, names + (a & 0xFF_FFFF) as usize);
            if a >> 24 == 1 {
                let prefix = format!("{}{}/", stack.last().unwrap().1, name);
                stack.push((c, prefix));
            } else {
                let data = bytes.get(b..b + c).ok_or("A file of the archive goes outside of the archive.")?.to_vec();
                files.push(File { node, path: format!("{}{}", stack.last().unwrap().1, name), data, align: alignment_of(b) });
            }
        }
        Ok(Darc { head: bytes[..data_offset].to_vec(), total, files })
    }

    /// The first file that has this name (the end of the path).
    pub fn find(&self, name: &str) -> Option<&File> {
        self.files.iter().find(|f| f.path.rsplit('/').next() == Some(name))
    }

    /// Changes the data of a file by its name. Returns false when there is no such file.
    pub fn replace(&mut self, name: &str, data: Vec<u8>) -> bool {
        match self.files.iter_mut().find(|f| f.path.rsplit('/').next() == Some(name)) {
            Some(file) => {
                file.data = data;
                true
            }
            None => false,
        }
    }

    /// Writes the archive.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.head.clone();
        let table = u32::from_le_bytes(out[0x10..0x14].try_into().unwrap()) as usize;
        let mut order: Vec<usize> = (0..self.files.len()).collect();
        // The data keep the order that the file had.
        order.sort_by_key(|&i| u32::from_le_bytes(self.head[table + self.files[i].node * 12 + 4..][..4].try_into().unwrap()));
        for i in order {
            let file = &self.files[i];
            while out.len() % file.align != 0 {
                out.push(0);
            }
            let at = out.len() as u32;
            out[table + file.node * 12 + 4..][..4].copy_from_slice(&at.to_le_bytes());
            out[table + file.node * 12 + 8..][..4].copy_from_slice(&(file.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&file.data);
        }
        while out.len() < self.total {
            out.push(0);
        }
        let size = out.len() as u32;
        out[0x0C..0x10].copy_from_slice(&size.to_le_bytes());
        out
    }
}
