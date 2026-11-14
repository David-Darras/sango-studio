//! A writer of H3D files (BCH): the sections, the strings, the dictionaries and the relocation table.
//!
//! A BCH has four areas: the *contents* (the structures of the model or of the textures, with pointers), the *strings*, the *commands*
//! (lists of GPU commands) and the *raw data* (textures, vertices, indices). The pointers are written as offsets and listed in a
//! relocation table that the game follows when it loads the file. The format is checked against files made by the tool that this
//! code replaces (see the tests).

use crate::patricia;
use std::collections::HashMap;

/// An area of the file: its bytes, and the places that hold a pointer (the offset, the kind of the pointer, the value).
#[derive(Default, Clone)]
pub struct Section {
    pub data: Vec<u8>,
    pub relocs: Vec<(usize, u32, u32)>,
}

impl Section {
    pub fn new() -> Section {
        Section::default()
    }

    /// Pads with zeros to a multiple of `n` and gives the offset.
    pub fn align(&mut self, n: usize) -> usize {
        while self.data.len() % n != 0 {
            self.data.push(0);
        }
        self.data.len()
    }

    pub fn tell(&self) -> usize {
        self.data.len()
    }

    pub fn u32(&mut self, value: u32) {
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    /// A word that is a pointer of a kind (1 = a string, 0 = the contents, 2 = the commands, 5 to 8 = the raw data...).
    pub fn ptr(&mut self, kind: u32, value: u32) {
        self.relocs.push((self.data.len(), kind, value));
        self.u32(value);
    }

    pub fn raw(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn patch(&mut self, offset: usize, value: u32) {
        self.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn patch_ptr(&mut self, offset: usize, kind: u32, value: u32) {
        self.relocs.push((offset, kind, value));
        self.patch(offset, value);
    }
}

/// The strings area: each text once, with its zero.
#[derive(Default)]
pub struct Strings {
    pub data: Vec<u8>,
    offsets: HashMap<String, u32>,
}

impl Strings {
    pub fn new() -> Strings {
        Strings::default()
    }

    /// The offset of a text, added when it is new.
    pub fn get(&mut self, text: &str) -> u32 {
        if let Some(&found) = self.offsets.get(text) {
            return found;
        }
        let offset = self.data.len() as u32;
        self.data.extend_from_slice(text.as_bytes());
        self.data.push(0);
        self.offsets.insert(text.to_string(), offset);
        offset
    }
}

/// The nodes of the dictionary of some names; returns the offset of the first node.
pub fn write_dict(sec: &mut Section, strs: &mut Strings, names: &[&str]) -> usize {
    let bytes: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
    let nodes = patricia::build(&bytes);
    let offset = sec.align(4);
    for node in nodes {
        sec.u32(node.ref_bit);
        sec.u32(((node.right as u32) << 16) | node.left as u32);
        match &node.name {
            None => sec.u32(0),
            Some(name) => {
                let at = strs.get(&String::from_utf8_lossy(name));
                sec.ptr(1, at);
            }
        }
    }
    offset
}

/// The 15 dictionaries of the start of the contents: the ones in `present` are filled by the caller (patched later), the others
/// point at an empty dictionary.
pub fn main_header(sec: &mut Section, present: &[usize]) {
    for _ in 0..15 {
        sec.u32(0);
        sec.u32(0);
        sec.u32(0);
    }
    for i in 0..15 {
        if present.contains(&i) {
            continue;
        }
        let offset = sec.tell();
        sec.u32(0xFFFF_FFFF);
        sec.u32(0);
        sec.u32(0);
        sec.patch_ptr(12 * i + 8, 0, offset as u32);
    }
}

/// Appends a list of GPU commands; `fixups` are the words that are pointers (the index of the word, the kind and the value).
/// Returns the offset and the number of words.
pub fn emit_commands(cmds: &mut Section, words: &[u32], fixups: &[(usize, u32, u32)]) -> (usize, usize) {
    let offset = cmds.align(8);
    for (index, &word) in words.iter().enumerate() {
        match fixups.iter().find(|f| f.0 == index) {
            Some(&(_, kind, value)) => cmds.ptr(kind, value),
            None => cmds.u32(word),
        }
    }
    while (cmds.tell() - offset) % 8 != 0 {
        cmds.u32(0);
    }
    (offset, words.len())
}

/// The bytes of the file: the header, the contents, the strings, the commands, the raw data and the relocation table.
pub fn build(contents: &mut Section, strs: &Strings, cmds: &Section, raw: &Section) -> Vec<u8> {
    contents.align(4);
    let strs_off = 0x44 + contents.data.len();
    let strs_len = strs.data.len();
    let cmds_off = (strs_off + strs_len + 0xF) & !0xF;
    let raw_off = (cmds_off + cmds.data.len() + 0x7F) & !0x7F;
    let reloc_off = (raw_off + raw.data.len() + 0xF) & !0xF;
    let mut out = vec![0u8; 0x44];
    out.extend_from_slice(&contents.data);
    out.resize(strs_off, 0);
    out.extend_from_slice(&strs.data);
    out.resize(cmds_off, 0);
    out.extend_from_slice(&cmds.data);
    out.resize(raw_off, 0);
    out.extend_from_slice(&raw.data);
    out.resize(reloc_off, 0);
    // The pointers inside the commands come first, as in the files of the game.
    let mut relocation: Vec<u8> = Vec::new();
    for &(offset, kind, _) in &cmds.relocs {
        relocation.extend_from_slice(&(((0x20 | kind) << 25) | (offset as u32 / 4)).to_le_bytes());
    }
    for &(offset, kind, _) in &contents.relocs {
        let place = if kind == 1 { offset as u32 } else { offset as u32 / 4 };
        relocation.extend_from_slice(&((kind << 25) | place).to_le_bytes());
    }
    out.extend_from_slice(&relocation);
    while out.len() % 0x80 != 0 {
        out.push(0);
    }
    // The room for the raw-data addresses (textures, vertices, indices) once they are relocated.
    let raw_pointers = cmds.relocs.iter().filter(|r| matches!(r.1, 5..=8)).count() as u32;
    let mut header = Vec::with_capacity(0x44);
    header.extend_from_slice(b"BCH\0");
    header.extend_from_slice(&[0x21, 0x21]);
    header.extend_from_slice(&42607u16.to_le_bytes());
    for value in [0x44, strs_off as u32, cmds_off as u32, raw_off as u32, reloc_off as u32, reloc_off as u32, contents.data.len() as u32, strs_len as u32, cmds.data.len() as u32, raw.data.len() as u32, 0, relocation.len() as u32, 4 * raw_pointers, 0] {
        header.extend_from_slice(&value.to_le_bytes());
    }
    header.extend_from_slice(&[1, 0]);
    header.extend_from_slice(&(raw_pointers as u16).to_le_bytes());
    out[..0x44].copy_from_slice(&header);
    out
}
