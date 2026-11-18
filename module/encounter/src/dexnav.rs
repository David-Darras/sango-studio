//! The resources of the DexNav screen (the archive 248): the animations, the layout and the pictures of the bottom screen.
//!
//! The archive has one file, compressed. Once decompressed, it is a pack:
//!
//! | Offset | Size | Meaning |
//! | --- | --- | --- |
//! | 0 | 4 | The number of resources (65). |
//! | 4 | 64 × count | The name of each resource, in a field of 64 bytes (`.bclan` animations, `.bclyt` the layout, `.bclim` the pictures). |
//! | … | … | A table that the studio does not need: it keeps it as it is. |
//! | first resource | … | The resources, one after the other, each one starting at a multiple of 8. |
//!
//! The studio changes the pictures and the places of the parts of the layout without changing a size, so the table stays right. A change
//! goes in the file `dexnav/0.bin` of the SD card (compressed like the file of the archive).
//!
//! # The layout
//!
//! A `.bclyt` file is a list of sections. Each pane of the screen is a section (`pan1`, `pic1`, `bnd1`, `wnd1` or `txt1`) of 76 bytes or more:
//! 4 bytes of flags, 24 bytes of name, then the place (`x`, `y`, `z`), the rotation, the scale and the size. The sections `pas1` and `pae1`
//! open and close the children of the pane before them. The name is the number of the part in base 36 (see `navi_parts.rs`).

use sango_core::{lz, Clim, Dump, Garc};
use std::path::{Path, PathBuf};

/// The archive of the DexNav.
pub const ARCHIVE: usize = 248;
const NAME_SIZE: usize = 64;
const ALIGNMENT: usize = 8;
/// The offset of the place of a pane in its section.
const PLACE_AT: usize = 0x24;
/// The offset of the size of a pane in its section.
const SIZE_AT: usize = 0x44;

/// The kind of a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Animation,
    Layout,
    Picture,
}

/// One resource of the pack.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub start: usize,
    pub size: usize,
    pub kind: Kind,
}

/// A part of the screen that the user can move.
#[derive(Clone, Copy, Debug)]
pub struct Pane {
    /// The number of the part (see `navi_parts.rs`).
    pub number: u32,
    /// Where the place of the pane is in the pack.
    pub at: usize,
    /// The place of the pane, relative to its parent.
    pub x: f32,
    pub y: f32,
    /// The place of the parent in the layout: the pane is at `parent + (x, y)`.
    pub parent: (f32, f32),
    pub width: f32,
    pub height: f32,
}

/// The pack of the DexNav, open for changes.
pub struct Navi {
    bytes: Vec<u8>,
    original: Vec<u8>,
    /// The pack as it was saved (or opened).
    saved: Vec<u8>,
    pub entries: Vec<Entry>,
    /// True when the file is the one of the SD card.
    pub from_sd: bool,
    file: PathBuf,
}

impl Navi {
    /// Opens the pack: the file of the SD card, else the one of the game.
    pub fn open(dump: &Dump, sd_root: &Path) -> Result<Navi, String> {
        let garc = Garc::open(&dump.archive(ARCHIVE))?;
        let game = garc.file_or_first(0, 0).ok_or("The archive of the DexNav has no file.")?;
        let original = if lz::is_compressed(game) { lz::decompress(game)? } else { game.to_vec() };
        let file = sd_root.join("dexnav").join("0.bin");
        let (bytes, from_sd) = match std::fs::read(&file) {
            Ok(raw) => (if lz::is_compressed(&raw) { lz::decompress(&raw)? } else { raw }, true),
            Err(_) => (original.clone(), false),
        };
        let entries = parse(&bytes)?;
        if bytes.len() != original.len() {
            return Err("The file of the SD card does not have the size of the one of the game: remove it from the folder dexnav.".into());
        }
        Ok(Navi { saved: bytes.clone(), bytes, original, entries, from_sd, file })
    }

    /// True when the pack is not the one of the game.
    pub fn changed(&self) -> bool {
        self.bytes != self.original
    }

    /// True when the pack has changes that are not saved.
    pub fn unsaved(&self) -> bool {
        self.bytes != self.saved
    }

    /// Takes the changes back: the pack as it was saved.
    pub fn discard(&mut self) {
        self.bytes = self.saved.clone();
    }

    /// The bytes of a resource.
    pub fn data(&self, entry: &Entry) -> &[u8] {
        &self.bytes[entry.start..entry.start + entry.size]
    }

    /// The picture of a resource.
    pub fn picture(&self, entry: &Entry) -> Result<Clim, String> {
        Clim::parse(self.data(entry))
    }

    /// True when a picture is the same as the one of the game.
    pub fn picture_changed(&self, entry: &Entry) -> bool {
        self.data(entry) != &self.original[entry.start..entry.start + entry.size]
    }

    /// Gives a picture other pixels (the size of the picture and its format stay).
    pub fn set_picture(&mut self, entry: &Entry, rgba: &[u8]) -> Result<(), String> {
        let new = self.picture(entry)?.with_rgba(rgba)?.to_bytes();
        if new.len() != entry.size {
            return Err("The new picture does not have the size of the old one.".into());
        }
        self.bytes[entry.start..entry.start + entry.size].copy_from_slice(&new);
        Ok(())
    }

    /// Gives a picture back as it is in the game.
    pub fn reset_picture(&mut self, entry: &Entry) {
        let old = self.original[entry.start..entry.start + entry.size].to_vec();
        self.bytes[entry.start..entry.start + entry.size].copy_from_slice(&old);
    }

    /// The parts of the layout that the page moves, with their places.
    pub fn panes(&self) -> Result<Vec<Pane>, String> {
        let layout = self.entries.iter().find(|e| e.kind == Kind::Layout).ok_or("The pack has no layout.")?;
        let data = self.data(layout);
        let sections = u16::from_le_bytes([data[0x10], data[0x11]]) as usize;
        let mut at = u16::from_le_bytes([data[6], data[7]]) as usize;
        let mut stack: Vec<(f32, f32)> = vec![(0.0, 0.0)];
        let mut last = (0.0f32, 0.0f32);
        let mut found = Vec::new();
        for _ in 0..sections {
            let magic = data.get(at..at + 4).ok_or("The layout is cut.")?;
            let size = u32::from_le_bytes(data[at + 4..at + 8].try_into().unwrap()) as usize;
            match magic {
                b"pan1" | b"pic1" | b"bnd1" | b"wnd1" | b"txt1" => {
                    let float = |offset: usize| f32::from_le_bytes(data[at + offset..at + offset + 4].try_into().unwrap());
                    let name: Vec<u8> = data[at + 12..at + 36].iter().copied().take_while(|&b| b != 0).collect();
                    let (x, y) = (float(PLACE_AT), float(PLACE_AT + 4));
                    let parent = *stack.last().unwrap();
                    last = (parent.0 + x, parent.1 + y);
                    if let Ok(number) = u32::from_str_radix(&String::from_utf8_lossy(&name), 36) {
                        found.push(Pane { number, at: layout.start + at + PLACE_AT, x, y, parent, width: float(SIZE_AT), height: float(SIZE_AT + 4) });
                    }
                }
                b"pas1" => stack.push(last),
                b"pae1" => {
                    if stack.len() > 1 {
                        stack.pop();
                    }
                }
                _ => {}
            }
            at += size.max(8);
        }
        Ok(found)
    }

    /// Moves a pane.
    pub fn set_place(&mut self, pane: &Pane, x: f32, y: f32) {
        self.bytes[pane.at..pane.at + 4].copy_from_slice(&x.to_le_bytes());
        self.bytes[pane.at + 4..pane.at + 8].copy_from_slice(&y.to_le_bytes());
    }

    /// The animation of a type of screen.
    fn type_entry(&self, kind: usize) -> Result<&Entry, String> {
        let name = format!("hokaku_navi_base_type_{kind}.bclan");
        self.entries.iter().find(|e| e.name == name).ok_or_else(|| format!("The pack has no screen of type {kind}."))
    }

    /// The picture that a type of screen shows as the background.
    pub fn type_background(&self, kind: usize) -> Result<String, String> {
        let data = self.data(self.type_entry(kind)?);
        let pai = animation_section(data)?;
        let table = pai + 20;
        let first = read_u32(data, table)? as usize;
        let start = table + first;
        let end = data[start..].iter().position(|&b| b == 0).ok_or("The animation is cut.")?;
        Ok(String::from_utf8_lossy(&data[start..start + end]).to_string())
    }

    /// The places where a type of screen puts the parts (the Pokémon of the map go to the places of the ground, the water and the fishing).
    pub fn type_places(&self, kind: usize) -> Result<Vec<TypePlace>, String> {
        let entry = self.type_entry(kind)?;
        let data = self.data(entry);
        let pai = animation_section(data)?;
        let count = read_u16(data, pai + 14)?;
        let list = pai + read_u32(data, pai + 16)? as usize;
        let mut found = Vec::new();
        for i in 0..count {
            let e = pai + read_u32(data, list + 4 * i)? as usize;
            let name: Vec<u8> = data.get(e..e + 20).ok_or("The animation is cut.")?.iter().copied().take_while(|&b| b != 0).collect();
            let (tags, is_material) = (*data.get(e + 20).ok_or("The animation is cut.")?, *data.get(e + 21).ok_or("The animation is cut.")?);
            if tags == 0 || is_material != 0 {
                continue;
            }
            let Ok(number) = u32::from_str_radix(&String::from_utf8_lossy(&name), 36) else { continue };
            let tag = e + read_u32(data, e + 24)? as usize;
            if data.get(tag..tag + 4) != Some(b"CLPA".as_slice()) {
                continue;
            }
            let (mut x, mut y) = (None, None);
            for t in 0..*data.get(tag + 4).ok_or("The animation is cut.")? as usize {
                let target = tag + read_u32(data, tag + 8 + 4 * t)? as usize;
                let (component, curve, keys) = (*data.get(target + 1).ok_or("The animation is cut.")?, read_u16(data, target + 2)?, read_u16(data, target + 4)?);
                if curve != 2 || keys == 0 {
                    continue;
                }
                // The first key: the frame, the value and the slope.
                let at = target + read_u32(data, target + 8)? as usize + 4;
                let value = f32::from_le_bytes(data.get(at..at + 4).ok_or("The animation is cut.")?.try_into().unwrap());
                match component {
                    0 => x = Some((value, entry.start + at)),
                    1 => y = Some((value, entry.start + at)),
                    _ => {}
                }
            }
            if let (Some((x, at_x)), Some((y, at_y))) = (x, y) {
                found.push(TypePlace { number, x, y, at_x, at_y });
            }
        }
        Ok(found)
    }

    /// Moves a part in a type of screen.
    pub fn set_type_place(&mut self, place: &TypePlace, x: f32, y: f32) {
        self.bytes[place.at_x..place.at_x + 4].copy_from_slice(&x.to_le_bytes());
        self.bytes[place.at_y..place.at_y + 4].copy_from_slice(&y.to_le_bytes());
    }

    /// True when a type of screen is not the one of the game.
    pub fn type_changed(&self, kind: usize) -> bool {
        self.type_entry(kind).is_ok_and(|e| self.data(e) != &self.original[e.start..e.start + e.size])
    }

    /// Gives a type of screen back as it is in the game.
    pub fn reset_type(&mut self, kind: usize) {
        if let Ok(entry) = self.type_entry(kind).cloned() {
            let old = self.original[entry.start..entry.start + entry.size].to_vec();
            self.bytes[entry.start..entry.start + entry.size].copy_from_slice(&old);
        }
    }

    /// Writes the pack in the file of the SD card, or removes the file when the pack is the one of the game.
    pub fn save(&mut self) -> Result<usize, String> {
        if !self.changed() {
            self.saved = self.bytes.clone();
            return self.remove().map(|_| 0);
        }
        let bytes = lz::compress_lz11(&self.bytes);
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::write(&self.file, &bytes).map_err(|e| format!("{}: {e}", self.file.display()))?;
        let index = self.file.with_file_name("index.bin");
        std::fs::write(&index, sango_core::file_lists::index_bytes(&[(0, bytes.len() as u32)])).map_err(|e| format!("{}: {e}", index.display()))?;
        self.from_sd = true;
        self.saved = self.bytes.clone();
        Ok(bytes.len())
    }

    /// Gives all the DexNav back as it is in the game.
    pub fn reset(&mut self) -> Result<(), String> {
        self.bytes = self.original.clone();
        self.saved = self.bytes.clone();
        self.remove()
    }

    fn remove(&mut self) -> Result<(), String> {
        for path in [self.file.clone(), self.file.with_file_name("index.bin")] {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        self.from_sd = false;
        Ok(())
    }
}

/// A part in the animation of a type of screen: the place that it has when the screen is shown.
#[derive(Clone, Copy, Debug)]
pub struct TypePlace {
    /// The number of the part (see `navi_parts.rs`).
    pub number: u32,
    pub x: f32,
    pub y: f32,
    at_x: usize,
    at_y: usize,
}

/// The number of types of screen.
pub const TYPE_COUNT: usize = 21;

/// The archive that tells which type of screen each map uses: pairs of 16 bits, the map then the type.
const TYPES_ARCHIVE: usize = 262;

/// The type of screen of each map that has one (a map that is not in the list uses the type 0).
pub fn map_types(dump: &Dump) -> Result<Vec<(usize, usize)>, String> {
    let garc = Garc::open(&dump.archive(TYPES_ARCHIVE))?;
    let file = garc.file_or_first(0, 0).ok_or("The archive of the types of screen has no file.")?;
    Ok(file.chunks_exact(4).map(|c| (u16::from_le_bytes([c[0], c[1]]) as usize, u16::from_le_bytes([c[2], c[3]]) as usize)).collect())
}

fn read_u16(data: &[u8], at: usize) -> Result<usize, String> {
    Ok(u16::from_le_bytes(data.get(at..at + 2).ok_or("The animation is cut.")?.try_into().unwrap()) as usize)
}

fn read_u32(data: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(data.get(at..at + 4).ok_or("The animation is cut.")?.try_into().unwrap()))
}

/// Where the section of the animations is in a `.bclan` file.
fn animation_section(data: &[u8]) -> Result<usize, String> {
    let mut at = read_u16(data, 6)?;
    for _ in 0..read_u16(data, 16)? {
        let size = read_u32(data, at + 4)? as usize;
        if data.get(at..at + 4) == Some(b"pai1".as_slice()) {
            return Ok(at);
        }
        at += size.max(8);
    }
    Err("The animation has no list of changes.".into())
}

/// The resources of a pack.
fn parse(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let count = u32::from_le_bytes(bytes.get(..4).ok_or("The pack is too short.")?.try_into().unwrap()) as usize;
    if count == 0 || count > 1000 || bytes.len() < 4 + NAME_SIZE * count {
        return Err("This is not the pack of the DexNav.".into());
    }
    let names: Vec<String> = (0..count).map(|i| String::from_utf8_lossy(&bytes[4 + NAME_SIZE * i..4 + NAME_SIZE * (i + 1)]).trim_end_matches('\0').to_string()).collect();
    // The first resource: the first signature after the names.
    let from = 4 + NAME_SIZE * count;
    let mut at = (from..bytes.len().saturating_sub(4)).find(|&i| matches!(&bytes[i..i + 4], b"CLAN" | b"CLYT")).ok_or("The pack has no resource.")?;
    let mut entries = Vec::new();
    for name in names {
        let magic = bytes.get(at..at + 4).ok_or("The pack is cut.")?;
        let (kind, size) = match magic {
            b"CLAN" => (Kind::Animation, read_size(bytes, at)?),
            b"CLYT" => (Kind::Layout, read_size(bytes, at)?),
            _ => (Kind::Picture, picture_size(bytes, at)?),
        };
        if at + size > bytes.len() {
            return Err("A resource is outside the pack.".into());
        }
        entries.push(Entry { name, start: at, size, kind });
        at = (at + size).div_ceil(ALIGNMENT) * ALIGNMENT;
    }
    Ok(entries)
}

fn read_size(bytes: &[u8], at: usize) -> Result<usize, String> {
    Ok(u32::from_le_bytes(bytes.get(at + 12..at + 16).ok_or("The pack is cut.")?.try_into().unwrap()) as usize)
}

/// The size of a picture: its footer (`CLIM`, the byte order mark and the size of the header, 40 bytes in all) ends the file.
fn picture_size(bytes: &[u8], at: usize) -> Result<usize, String> {
    let mut k = at;
    while k + 40 <= bytes.len() {
        if &bytes[k..k + 4] == b"CLIM" && bytes[k + 4..k + 8] == [0xFF, 0xFE, 0x14, 0x00] {
            return Ok(k + 40 - at);
        }
        k += 4;
    }
    Err("A picture of the pack has no footer.".into())
}

#[cfg(test)]
mod test {
    use super::*;

    fn dump() -> Option<Dump> {
        std::env::var("SANGO_DUMP").ok().map(|path| Dump::open(&path).unwrap())
    }

    #[test]
    fn the_pack_has_all_its_resources() {
        let Some(dump) = dump() else { return };
        let navi = Navi::open(&dump, &std::env::temp_dir().join("sango_navi_none")).unwrap();
        assert_eq!(navi.entries.len(), 65);
        assert_eq!(navi.entries.iter().filter(|e| e.kind == Kind::Picture).count(), 33);
        let last = navi.entries.last().unwrap();
        assert_eq!(last.start + last.size, navi.bytes.len());
        for entry in navi.entries.iter().filter(|e| e.kind == Kind::Picture) {
            if entry.name == "kirakira.bclim" {
                continue;
            }
            let picture = navi.picture(entry).unwrap();
            picture.to_rgba().unwrap_or_else(|e| panic!("{}: {e}", entry.name));
        }
    }

    #[test]
    fn a_picture_keeps_its_size_when_it_is_replaced() {
        let Some(dump) = dump() else { return };
        let mut navi = Navi::open(&dump, &std::env::temp_dir().join("sango_navi_none")).unwrap();
        for entry in navi.entries.clone().iter().filter(|e| e.kind == Kind::Picture && e.name != "kirakira.bclim") {
            let before = navi.picture(entry).unwrap();
            let rgba = before.to_rgba().unwrap();
            navi.set_picture(entry, &rgba).unwrap_or_else(|e| panic!("{}: {e}", entry.name));
            let after = navi.picture(entry).unwrap().to_rgba().unwrap();
            // The pixels come back almost the same (the formats lose a little).
            let worst = rgba.chunks_exact(4).zip(after.chunks_exact(4)).filter(|(a, _)| a[3] > 200).map(|(a, b)| (0..3).map(|i| (i32::from(a[i]) - i32::from(b[i])).abs()).max().unwrap()).max().unwrap_or(0);
            assert!(worst < 90, "{}: {worst}", entry.name);
            assert_eq!(navi.data(entry).len(), entry.size);
        }
    }

    #[test]
    fn the_places_of_a_type_of_screen_are_read_and_moved() {
        let Some(dump) = dump() else { return };
        let mut navi = Navi::open(&dump, &std::env::temp_dir().join("sango_navi_none")).unwrap();
        assert_eq!(navi.type_background(3).unwrap(), "screen_03.bclim");
        let places = navi.type_places(3).unwrap();
        let first = places.iter().find(|p| p.number == 112).unwrap();
        assert_eq!((first.x, first.y), (-25.0, 25.0));
        assert!(places.iter().filter(|p| crate::navi_parts::PARTS.iter().any(|q| q.0 == p.number && q.2.starts_with("Places"))).count() == 30);
        navi.set_type_place(first, 7.0, 8.0);
        assert!(navi.type_changed(3) && !navi.type_changed(4));
        let moved = navi.type_places(3).unwrap().into_iter().find(|p| p.number == 112).unwrap();
        assert_eq!((moved.x, moved.y), (7.0, 8.0));
        navi.reset_type(3);
        assert!(!navi.changed());
        let types = map_types(&dump).unwrap();
        assert!(types.contains(&(24, 3)));
    }

    #[test]
    fn the_places_of_the_parts_are_found_and_moved() {
        let Some(dump) = dump() else { return };
        let mut navi = Navi::open(&dump, &std::env::temp_dir().join("sango_navi_none")).unwrap();
        let panes = navi.panes().unwrap();
        for (number, _, _) in crate::navi_parts::PARTS {
            assert!(panes.iter().any(|p| p.number == number), "part {number}");
        }
        let first = panes.iter().find(|p| p.number == crate::navi_parts::PARTS[20].0).unwrap();
        assert_eq!((first.x, first.y), (-100.0, -165.0));
        navi.set_place(first, 5.0, 6.0);
        assert!(navi.changed());
        let moved = navi.panes().unwrap().into_iter().find(|p| p.number == first.number).unwrap();
        assert_eq!((moved.x, moved.y), (5.0, 6.0));
        assert_eq!(navi.bytes.len(), navi.original.len());
    }
}
