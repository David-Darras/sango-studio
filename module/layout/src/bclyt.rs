//! The `.bclyt` file: the layout of one screen. It is a header and a list of blocks, each one with a 4-letter tag and its size.
//!
//! | Block | Meaning |
//! | --- | --- |
//! | `lyt1` | The size of the screen of the layout. |
//! | `txl1` | The list of the images. |
//! | `fnl1` | The list of the fonts. |
//! | `mat1` | The materials (the image and the colors of a picture). |
//! | `pan1` `pic1` `txt1` `wnd1` `bnd1` | A pane: empty, a picture, a text box, a window, a bounding box. |
//! | `usd1` | The user data of the pane before. |
//! | `pas1` `pae1` | The start and the end of the children of the pane before. |
//! | `grp1` | A group of panes, used by the animations. |
//!
//! A pane block starts with the common part (all the offsets are from the start of the block):
//!
//! | Offset | Size | Meaning |
//! | --- | --- | --- |
//! | 0x08 | 1 | flags: bit 0 visible, bit 1 the alpha of the parent counts, bit 2 adjust to the screen |
//! | 0x09 | 1 | origin: the point of the pane that is at its position, `3 × row + column` with row and column from 0 (top, left) to 2 |
//! | 0x0A | 1 | alpha |
//! | 0x0B | 1 | more flags |
//! | 0x0C | 16 | name |
//! | 0x1C | 8 | user data |
//! | 0x24 | 12 | position x, y, z (3 × f32) |
//! | 0x30 | 12 | rotation x, y, z in degrees |
//! | 0x3C | 8 | scale x, y |
//! | 0x44 | 8 | width, height |
//!
//! The position of a pane is the position of its origin point in the coordinates of its parent, whose own origin point is (0, 0);
//! y goes up. This code keeps the bytes of every block that it does not change, so a file that is read and written again is the same.

use std::ops::Range;

/// The size of the common part of a pane.
pub const PANE_SIZE: usize = 76;

/// The kind of a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Empty,
    Picture,
    Text,
    Window,
    Bounding,
}

impl Kind {
    fn from_tag(tag: &[u8; 4]) -> Option<Kind> {
        Some(match tag {
            b"pan1" => Kind::Empty,
            b"pic1" => Kind::Picture,
            b"txt1" => Kind::Text,
            b"wnd1" => Kind::Window,
            b"bnd1" => Kind::Bounding,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Empty => "Pane",
            Kind::Picture => "Picture",
            Kind::Text => "Text box",
            Kind::Window => "Window",
            Kind::Bounding => "Bounds",
        }
    }
}

/// A block of the file, with its 8-byte header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}

/// A pane of the tree.
#[derive(Clone, Copy, Debug)]
pub struct Node {
    /// The index of its block.
    pub block: usize,
    pub kind: Kind,
    /// The index of the parent in the list of nodes.
    pub parent: Option<usize>,
    pub depth: usize,
}

/// A material: what a picture shows.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    /// The index of the first image in the list of images.
    pub texture: Option<usize>,
    /// How the first image goes outside of the picture, horizontally and vertically: 0 stops at the edge, 1 repeats, 2 repeats mirrored.
    pub wrap: (u8, u8),
    /// The matrix of the texture coordinates of the first image: translation (2), rotation in degrees, scale (2).
    pub srt: Option<[f32; 5]>,
}

/// A layout file in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bclyt {
    head: Vec<u8>,
    pub blocks: Vec<Block>,
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn u16_at(b: &[u8], at: usize) -> usize {
    u16::from_le_bytes([b[at], b[at + 1]]) as usize
}

fn u32_at(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

fn put_f32(b: &mut [u8], at: usize, v: f32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn cstr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.split(|&b| b == 0).next().unwrap_or(&[])).into_owned()
}

impl Bclyt {
    pub fn parse(bytes: &[u8]) -> Result<Bclyt, String> {
        if bytes.len() < 20 || &bytes[..4] != b"CLYT" {
            return Err("This is not a BCLYT layout.".into());
        }
        let head_size = u16_at(bytes, 6);
        let count = u16_at(bytes, 16);
        let mut blocks = Vec::new();
        let mut at = head_size;
        for _ in 0..count {
            if at + 8 > bytes.len() {
                return Err("A block of the layout is cut.".into());
            }
            let size = u32_at(bytes, at + 4);
            if size < 8 || at + size > bytes.len() {
                return Err("A block of the layout has a wrong size.".into());
            }
            blocks.push(Block { tag: bytes[at..at + 4].try_into().unwrap(), data: bytes[at..at + size].to_vec() });
            at += size;
        }
        Ok(Bclyt { head: bytes[..head_size].to_vec(), blocks })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.head.clone();
        for b in &self.blocks {
            out.extend_from_slice(&b.data);
        }
        let size = out.len() as u32;
        out[12..16].copy_from_slice(&size.to_le_bytes());
        out[16..18].copy_from_slice(&(self.blocks.len() as u16).to_le_bytes());
        out
    }

    /// The size of the screen: (400, 240) for the upper screen.
    pub fn screen(&self) -> (f32, f32) {
        self.blocks.iter().find(|b| &b.tag == b"lyt1").map(|b| (f32_at(&b.data, 12), f32_at(&b.data, 16))).unwrap_or((400.0, 240.0))
    }

    /// The tree of the panes, in the order of the file (a parent before its children).
    pub fn nodes(&self) -> Vec<Node> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut last: Option<usize> = None;
        for (i, b) in self.blocks.iter().enumerate() {
            if let Some(kind) = Kind::from_tag(&b.tag) {
                nodes.push(Node { block: i, kind, parent: stack.last().copied(), depth: stack.len() });
                last = Some(nodes.len() - 1);
            } else if &b.tag == b"pas1" {
                if let Some(l) = last {
                    stack.push(l);
                }
            } else if &b.tag == b"pae1" {
                stack.pop();
            }
        }
        nodes
    }

    /// The blocks of a pane and of everything under it.
    pub fn subtree(&self, block: usize) -> Range<usize> {
        let mut end = block + 1;
        while end < self.blocks.len() && &self.blocks[end].tag == b"usd1" {
            end += 1;
        }
        if end < self.blocks.len() && &self.blocks[end].tag == b"pas1" {
            let mut depth = 0;
            while end < self.blocks.len() {
                match &self.blocks[end].tag {
                    b"pas1" => depth += 1,
                    b"pae1" => {
                        depth -= 1;
                        if depth == 0 {
                            end += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                end += 1;
            }
        }
        block..end
    }

    // ---- The common part of a pane ----

    pub fn name(&self, block: usize) -> String {
        cstr(&self.blocks[block].data[12..28])
    }

    pub fn set_name(&mut self, block: usize, name: &str) {
        let d = &mut self.blocks[block].data;
        d[12..28].fill(0);
        let n = name.len().min(15);
        d[12..12 + n].copy_from_slice(&name.as_bytes()[..n]);
    }

    pub fn visible(&self, block: usize) -> bool {
        self.blocks[block].data[8] & 1 != 0
    }

    pub fn set_visible(&mut self, block: usize, on: bool) {
        let d = &mut self.blocks[block].data;
        d[8] = d[8] & !1 | on as u8;
    }

    /// True when the alpha of the parent counts for this pane.
    pub fn influenced_alpha(&self, block: usize) -> bool {
        self.blocks[block].data[8] & 2 != 0
    }

    /// The origin as (column, row), each from 0 to 2.
    pub fn origin(&self, block: usize) -> (u8, u8) {
        let v = self.blocks[block].data[9] % 9;
        (v % 3, v / 3)
    }

    pub fn set_origin(&mut self, block: usize, column: u8, row: u8) {
        self.blocks[block].data[9] = row.min(2) * 3 + column.min(2);
    }

    pub fn alpha(&self, block: usize) -> u8 {
        self.blocks[block].data[10]
    }

    pub fn set_alpha(&mut self, block: usize, alpha: u8) {
        self.blocks[block].data[10] = alpha;
    }

    pub fn position(&self, block: usize) -> [f32; 3] {
        let d = &self.blocks[block].data;
        [f32_at(d, 36), f32_at(d, 40), f32_at(d, 44)]
    }

    pub fn set_position(&mut self, block: usize, p: [f32; 3]) {
        for (i, v) in p.iter().enumerate() {
            put_f32(&mut self.blocks[block].data, 36 + i * 4, *v);
        }
    }

    pub fn rotation(&self, block: usize) -> [f32; 3] {
        let d = &self.blocks[block].data;
        [f32_at(d, 48), f32_at(d, 52), f32_at(d, 56)]
    }

    pub fn set_rotation(&mut self, block: usize, r: [f32; 3]) {
        for (i, v) in r.iter().enumerate() {
            put_f32(&mut self.blocks[block].data, 48 + i * 4, *v);
        }
    }

    pub fn scale(&self, block: usize) -> [f32; 2] {
        let d = &self.blocks[block].data;
        [f32_at(d, 60), f32_at(d, 64)]
    }

    pub fn set_scale(&mut self, block: usize, s: [f32; 2]) {
        put_f32(&mut self.blocks[block].data, 60, s[0]);
        put_f32(&mut self.blocks[block].data, 64, s[1]);
    }

    pub fn size(&self, block: usize) -> [f32; 2] {
        let d = &self.blocks[block].data;
        [f32_at(d, 68), f32_at(d, 72)]
    }

    pub fn set_size(&mut self, block: usize, s: [f32; 2]) {
        put_f32(&mut self.blocks[block].data, 68, s[0]);
        put_f32(&mut self.blocks[block].data, 72, s[1]);
    }

    // ---- Pictures ----

    /// The index of the material of a picture or of a text box.
    pub fn material_index(&self, block: usize) -> Option<usize> {
        let d = &self.blocks[block].data;
        match &self.blocks[block].tag {
            b"pic1" if d.len() >= 94 => Some(u16_at(d, 92)),
            b"txt1" if d.len() >= 82 => Some(u16_at(d, 80)),
            _ => None,
        }
    }

    /// The colors of the 4 corners of a picture (top-left, top-right, bottom-left, bottom-right).
    pub fn corner_colors(&self, block: usize) -> [[u8; 4]; 4] {
        let d = &self.blocks[block].data;
        let mut out = [[255u8; 4]; 4];
        if &self.blocks[block].tag == b"pic1" && d.len() >= 92 {
            for (i, c) in out.iter_mut().enumerate() {
                c.copy_from_slice(&d[76 + i * 4..80 + i * 4]);
            }
        }
        out
    }

    /// The texture coordinates of the 4 corners of a picture, for its first image.
    pub fn corner_uvs(&self, block: usize) -> [[f32; 2]; 4] {
        let d = &self.blocks[block].data;
        let mut out = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        if &self.blocks[block].tag == b"pic1" && d.len() >= 128 && d[94] >= 1 {
            for (i, uv) in out.iter_mut().enumerate() {
                *uv = [f32_at(d, 96 + i * 8), f32_at(d, 100 + i * 8)];
            }
        }
        out
    }

    // ---- Text boxes ----

    /// The text that the layout holds in a text box (the game usually replaces it).
    pub fn text(&self, block: usize) -> String {
        let d = &self.blocks[block].data;
        if &self.blocks[block].tag != b"txt1" || d.len() < 96 {
            return String::new();
        }
        let (at, len) = (u32_at(d, 92), u16_at(d, 78));
        let Some(bytes) = d.get(at..(at + len).min(d.len())) else { return String::new() };
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        String::from_utf16_lossy(&units)
    }

    /// The size of the characters of a text box.
    pub fn font_size(&self, block: usize) -> [f32; 2] {
        let d = &self.blocks[block].data;
        if &self.blocks[block].tag == b"txt1" && d.len() >= 112 {
            [f32_at(d, 104), f32_at(d, 108)]
        } else {
            [0.0, 0.0]
        }
    }

    pub fn set_font_size(&mut self, block: usize, s: [f32; 2]) {
        if &self.blocks[block].tag == b"txt1" && self.blocks[block].data.len() >= 112 {
            put_f32(&mut self.blocks[block].data, 104, s[0]);
            put_f32(&mut self.blocks[block].data, 108, s[1]);
        }
    }

    /// The color of a text box (top, bottom).
    pub fn text_colors(&self, block: usize) -> [[u8; 4]; 2] {
        let d = &self.blocks[block].data;
        let mut out = [[0, 0, 0, 255]; 2];
        if &self.blocks[block].tag == b"txt1" && d.len() >= 104 {
            out[0].copy_from_slice(&d[96..100]);
            out[1].copy_from_slice(&d[100..104]);
        }
        out
    }

    /// The horizontal alignment of a text box: 0 left, 1 center, 2 right, 3 justified (4 = the game decides).
    pub fn text_align(&self, block: usize) -> u8 {
        let d = &self.blocks[block].data;
        if &self.blocks[block].tag == b"txt1" && d.len() > 85 {
            d[85]
        } else {
            0
        }
    }

    // ---- Lists ----

    /// The names of the images (`txl1`).
    pub fn textures(&self) -> Vec<String> {
        let Some(b) = self.blocks.iter().find(|b| &b.tag == b"txl1") else { return Vec::new() };
        let count = u32_at(&b.data, 8);
        (0..count)
            .filter_map(|i| {
                let off = u32_at(&b.data, 12 + i * 4);
                b.data.get(12 + off..).map(cstr)
            })
            .collect()
    }

    /// The materials (`mat1`).
    pub fn materials(&self) -> Vec<Material> {
        let Some(b) = self.blocks.iter().find(|b| &b.tag == b"mat1") else { return Vec::new() };
        let count = u16_at(&b.data, 8);
        (0..count)
            .map(|i| {
                let off = u32_at(&b.data, 12 + i * 4);
                let m = b.data.get(off + 48..).unwrap_or(&[]);
                let bits = if m.len() >= 4 { u32::from_le_bytes(m[..4].try_into().unwrap()) } else { 0 };
                let (maps, srts) = ((bits & 3) as usize, ((bits >> 2) & 3) as usize);
                let texture = (maps >= 1 && m.len() >= 8).then(|| u16::from_le_bytes([m[4], m[5]]) as usize);
                let wrap = if maps >= 1 && m.len() >= 8 { (m[6] & 3, m[7] & 3) } else { (0, 0) };
                let at = 4 + maps * 4;
                let srt = (srts >= 1 && m.len() >= at + 20).then(|| {
                    let mut v = [0f32; 5];
                    for (k, x) in v.iter_mut().enumerate() {
                        *x = f32::from_le_bytes(m[at + k * 4..at + k * 4 + 4].try_into().unwrap());
                    }
                    v
                });
                Material { texture, wrap, srt }
            })
            .collect()
    }

    // ---- Changes ----

    /// The names of all the panes.
    pub fn pane_names(&self) -> Vec<String> {
        self.nodes().iter().map(|n| self.name(n.block)).collect()
    }

    /// A name that no pane has yet.
    pub fn free_name(&self) -> String {
        let taken = self.pane_names();
        (1..).map(|i| format!("n{i}")).find(|n| !taken.contains(n)).unwrap()
    }

    /// Removes a pane and everything under it. The groups forget the panes that are gone. Returns the names that were removed.
    pub fn delete(&mut self, block: usize) -> Vec<String> {
        let range = self.subtree(block);
        let gone: Vec<String> = range.clone().filter(|&i| Kind::from_tag(&self.blocks[i].tag).is_some()).map(|i| self.name(i)).collect();
        self.blocks.drain(range);
        for b in self.blocks.iter_mut().filter(|b| &b.tag == b"grp1") {
            let count = u16_at(&b.data, 24);
            if b.data.len() < 28 + count * 16 {
                continue;
            }
            let kept: Vec<&[u8]> = (0..count).map(|i| &b.data[28 + i * 16..44 + i * 16]).filter(|n| !gone.contains(&cstr(n))).collect();
            let mut data = b.data[..24].to_vec();
            data.extend_from_slice(&(kept.len() as u16).to_le_bytes());
            data.extend_from_slice(&[0, 0]);
            for n in &kept {
                data.extend_from_slice(n);
            }
            let size = data.len() as u32;
            data[4..8].copy_from_slice(&size.to_le_bytes());
            b.data = data;
        }
        gone
    }

    /// Copies a pane and everything under it, right after it. The copies get new names.
    /// Returns the block of the copy and the new names.
    pub fn duplicate(&mut self, block: usize) -> (usize, Vec<String>) {
        let range = self.subtree(block);
        let copy: Vec<Block> = self.blocks[range.clone()].to_vec();
        let at = range.end;
        for (k, b) in copy.into_iter().enumerate() {
            self.blocks.insert(at + k, b);
        }
        let mut names = Vec::new();
        for i in at..at + range.len() {
            if Kind::from_tag(&self.blocks[i].tag).is_some() {
                let name = self.free_name();
                self.set_name(i, &name);
                names.push(name);
            }
        }
        (at, names)
    }

    /// Adds an empty pane after another pane (and its children), at the same level. Returns its block and its name.
    pub fn add_empty(&mut self, after: usize) -> (usize, String) {
        let at = self.subtree(after).end;
        let name = self.free_name();
        let mut data = vec![0u8; PANE_SIZE];
        data[..4].copy_from_slice(b"pan1");
        data[4..8].copy_from_slice(&(PANE_SIZE as u32).to_le_bytes());
        data[8] = 1;
        data[9] = 4;
        data[10] = 255;
        put_f32(&mut data, 60, 1.0);
        put_f32(&mut data, 64, 1.0);
        put_f32(&mut data, 68, 100.0);
        put_f32(&mut data, 72, 50.0);
        self.blocks.insert(at, Block { tag: *b"pan1", data });
        self.set_name(at, &name);
        (at, name)
    }
}
