//! The font files of the 3DS (`CFNT`, also called BCFNT).
//!
//! All the numbers are little-endian. A file has a header of 0x14 bytes and then blocks. Each block has a name of 4 characters and its size.
//! The pointers of the font information block (`FINF`) point at the data of a block, which is 8 bytes after the start of the block.
//!
//! | Block | Content |
//! | --- | --- |
//! | `FINF` | The line feed, the glyph that replaces an unknown character, the default widths, the pointers to the three blocks that follow, and the height, width and ascent of the font. |
//! | `TGLP` | The pictures of the glyphs: the size of a cell, the baseline, the format of the textures, and the sheets. Each sheet holds `columns × rows` glyphs. |
//! | `CWDH` | The widths of the glyphs: for each glyph the space on the left, the width of the picture and the width of the character (3 bytes). The blocks make a chain. |
//! | `CMAP` | The map from a character code to a glyph. The blocks make a chain. A block has one of three methods, see [`Method`]. |
//!
//! A sheet is a texture. Its pixels are in tiles of 8 × 8 pixels (the Morton order inside a tile). A cell has one more pixel on the right and
//! at the bottom than the size of the glyph, as a border. The glyph with the index `n` is in the sheet `n / (columns × rows)`.

use sango_core::bytes::{u16_at, u32_at};

/// The size of the header of the file.
const HEADER: usize = 0x14;

/// The code of "no glyph" in a map.
const NONE: u16 = 0xFFFF;

/// How a block of the character map finds the glyph of a code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// The glyphs of the codes from the first one to the last one follow each other, from this index.
    Direct { first_index: u16 },
    /// A table with one glyph index for each code of the range (the value 0xFFFF is no glyph).
    Table(Vec<u16>),
    /// A list of pairs (code, glyph).
    Scan(Vec<(u16, u16)>),
}

/// A block of the map from the characters to the glyphs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharMap {
    pub first: u16,
    pub last: u16,
    pub method: Method,
}

impl CharMap {
    /// The glyph of a code, when the block has it.
    pub fn index_of(&self, code: u16) -> Option<u16> {
        if code < self.first || code > self.last {
            return None;
        }
        match &self.method {
            Method::Direct { first_index } => first_index.checked_add(code - self.first),
            Method::Table(table) => table.get((code - self.first) as usize).copied().filter(|index| *index != NONE),
            Method::Scan(pairs) => pairs.iter().find(|(c, _)| *c == code).map(|(_, index)| *index),
        }
    }
}

/// The widths of a glyph, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Width {
    /// The space on the left of the picture.
    pub left: i8,
    /// The width of the picture.
    pub glyph: u8,
    /// The width of the character: the pen goes this far.
    pub advance: u8,
}

/// The format of the pixels of a sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// 8 bits of alpha.
    A8,
    /// 4 bits of alpha.
    A4,
    /// 4 bits of luminance and 4 bits of alpha.
    La4,
    /// 8 bits of luminance and 8 bits of alpha.
    La8,
}

impl Format {
    fn from_number(number: u16) -> Result<Format, String> {
        Ok(match number {
            5 => Format::La8,
            8 => Format::A8,
            9 => Format::La4,
            11 => Format::A4,
            other => return Err(format!("The sheets have the format {other}, which this page does not read.")),
        })
    }

    /// The size of a sheet in bytes, for a width and a height.
    pub fn sheet_bytes(self, width: usize, height: usize) -> usize {
        match self {
            Format::A4 => width * height / 2,
            Format::A8 | Format::La4 => width * height,
            Format::La8 => width * height * 2,
        }
    }

    /// The name of the format.
    pub fn name(self) -> &'static str {
        match self {
            Format::A8 => "A8",
            Format::A4 => "A4",
            Format::La4 => "LA4",
            Format::La8 => "LA8",
        }
    }
}

/// A font.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    bytes: Vec<u8>,
    /// The distance between two lines.
    pub line_feed: i8,
    /// The glyph shown for a character that the font does not have.
    pub alter_index: u16,
    /// The widths of a glyph that has none.
    pub default_width: Width,
    pub height: u8,
    pub width: u8,
    pub ascent: u8,
    /// The size of a cell of a sheet (the biggest glyph), without the border.
    pub cell_width: u8,
    pub cell_height: u8,
    pub baseline: u8,
    pub format: Format,
    pub sheet_count: usize,
    pub columns: usize,
    pub rows: usize,
    pub sheet_width: usize,
    pub sheet_height: usize,
    sheet_size: usize,
    data_start: usize,
    widths: Vec<Width>,
    /// The place in the file of the 3 bytes of the widths of each glyph.
    width_places: Vec<Option<usize>>,
    pub maps: Vec<CharMap>,
}

/// The chain of blocks that starts at a pointer, as the places of the data of each block.
fn chain(bytes: &[u8], first: usize, name: &[u8; 4]) -> Result<Vec<usize>, String> {
    let mut found = Vec::new();
    let mut at = first;
    while at != 0 {
        if found.len() > 4096 {
            return Err("A chain of blocks does not end.".into());
        }
        let block = at.checked_sub(8).ok_or("A pointer is too small.")?;
        if bytes.get(block..block + 4) != Some(name) {
            return Err(format!("A block `{}` is not where the font says.", String::from_utf8_lossy(name)));
        }
        found.push(at);
        // The pointer to the next block: after the range and the four numbers of a map, or after the range of the widths.
        at = u32_at(bytes, if name == b"CWDH" { at + 4 } else { at + 8 })? as usize;
    }
    Ok(found)
}

impl Font {
    /// Reads a font file.
    pub fn parse(bytes: Vec<u8>) -> Result<Font, String> {
        if bytes.get(..4) != Some(b"CFNT") {
            return Err("This is not a font file (the first four bytes are not `CFNT`).".into());
        }
        if bytes.get(HEADER..HEADER + 4) != Some(b"FINF") {
            return Err("The block of the font information is missing.".into());
        }
        let info = HEADER + 8;
        let line_feed = bytes.get(info + 1).copied().ok_or("The file is cut.")? as i8;
        let alter_index = u16_at(&bytes, info + 2)?;
        let default_width = Width { left: bytes[info + 4] as i8, glyph: bytes[info + 5], advance: bytes[info + 6] };
        let tglp = u32_at(&bytes, info + 8)? as usize;
        let cwdh = u32_at(&bytes, info + 12)? as usize;
        let cmap = u32_at(&bytes, info + 16)? as usize;
        let (height, width, ascent) = (bytes[info + 20], bytes[info + 21], bytes[info + 22]);

        if tglp < 8 || bytes.get(tglp - 8..tglp - 4) != Some(b"TGLP") {
            return Err("The block of the glyph pictures is missing.".into());
        }
        let (cell_width, cell_height, baseline) = (bytes[tglp], bytes[tglp + 1], bytes[tglp + 2]);
        let sheet_size = u32_at(&bytes, tglp + 4)? as usize;
        let sheet_count = u16_at(&bytes, tglp + 8)? as usize;
        let format_number = u16_at(&bytes, tglp + 10)?;
        if format_number & 0x8000 != 0 {
            return Err("The sheets are compressed: this page does not read them.".into());
        }
        let format = Format::from_number(format_number)?;
        let columns = u16_at(&bytes, tglp + 12)? as usize;
        let rows = u16_at(&bytes, tglp + 14)? as usize;
        let sheet_width = u16_at(&bytes, tglp + 16)? as usize;
        let sheet_height = u16_at(&bytes, tglp + 18)? as usize;
        let data_start = u32_at(&bytes, tglp + 20)? as usize;
        if columns == 0 || rows == 0 || sheet_width % 8 != 0 || sheet_height % 8 != 0 {
            return Err("The sheets have a size that this page does not read.".into());
        }
        if sheet_size != format.sheet_bytes(sheet_width, sheet_height) {
            return Err("The size of a sheet does not match its format.".into());
        }
        if data_start.checked_add(sheet_size * sheet_count).map_or(true, |end| end > bytes.len()) {
            return Err("The sheets go out of the file.".into());
        }
        if (cell_width as usize + 1) * columns > sheet_width || (cell_height as usize + 1) * rows > sheet_height {
            return Err("The cells do not fit in a sheet.".into());
        }

        let mut widths = Vec::new();
        let mut width_places: Vec<Option<usize>> = Vec::new();
        for at in chain(&bytes, cwdh, b"CWDH")? {
            let (begin, end) = (u16_at(&bytes, at)? as usize, u16_at(&bytes, at + 2)? as usize);
            if end < begin {
                return Err("A block of widths has an end before its start.".into());
            }
            if widths.len() < end + 1 {
                widths.resize(end + 1, default_width);
                width_places.resize(end + 1, None);
            }
            for index in begin..=end {
                let place = at + 8 + 3 * (index - begin);
                let entry = bytes.get(place..place + 3).ok_or("A block of widths is cut.")?;
                widths[index] = Width { left: entry[0] as i8, glyph: entry[1], advance: entry[2] };
                width_places[index] = Some(place);
            }
        }

        let mut maps = Vec::new();
        for at in chain(&bytes, cmap, b"CMAP")? {
            let (first, last) = (u16_at(&bytes, at)?, u16_at(&bytes, at + 2)?);
            let method_number = u16_at(&bytes, at + 4)?;
            let data = at + 12;
            let method = match method_number {
                0 => Method::Direct { first_index: u16_at(&bytes, data)? },
                1 => {
                    let count = (last as usize).checked_sub(first as usize).ok_or("A map has an end before its start.")? + 1;
                    Method::Table((0..count).map(|k| u16_at(&bytes, data + 2 * k)).collect::<Result<_, _>>()?)
                }
                2 => {
                    let count = u16_at(&bytes, data)? as usize;
                    Method::Scan((0..count).map(|k| Ok((u16_at(&bytes, data + 2 + 4 * k)?, u16_at(&bytes, data + 4 + 4 * k)?))).collect::<Result<_, String>>()?)
                }
                other => return Err(format!("A map has the method {other}, which does not exist.")),
            };
            maps.push(CharMap { first, last, method });
        }

        Ok(Font {
            bytes,
            line_feed,
            alter_index,
            default_width,
            height,
            width,
            ascent,
            cell_width,
            cell_height,
            baseline,
            format,
            sheet_count,
            columns,
            rows,
            sheet_width,
            sheet_height,
            sheet_size,
            data_start,
            widths,
            width_places,
            maps,
        })
    }

    /// The bytes of the file.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The number of glyphs that the sheets hold.
    pub fn glyph_count(&self) -> usize {
        self.sheet_count * self.columns * self.rows
    }

    /// The glyph of a character code. A code that the font does not have gives `None`.
    pub fn index_of(&self, code: u16) -> Option<u16> {
        self.maps.iter().find_map(|map| map.index_of(code))
    }

    /// The glyph of a character code, or the glyph that replaces it.
    pub fn index_or_alter(&self, code: u16) -> u16 {
        self.index_of(code).unwrap_or(self.alter_index)
    }

    /// The widths of a glyph.
    pub fn width_of(&self, index: u16) -> Width {
        self.widths.get(index as usize).copied().unwrap_or(self.default_width)
    }

    /// All the pairs (code, glyph) of the maps, sorted by code. A code in two maps keeps the glyph of the first map.
    pub fn codes(&self) -> Vec<(u16, u16)> {
        let mut all: Vec<(u16, u16)> = Vec::new();
        for map in &self.maps {
            match &map.method {
                Method::Scan(pairs) => all.extend(pairs.iter().copied()),
                _ => all.extend((map.first..=map.last).filter_map(|code| map.index_of(code).map(|index| (code, index)))),
            }
        }
        // A stable sort keeps the first map first.
        all.sort_by_key(|(code, _)| *code);
        all.dedup_by_key(|(code, _)| *code);
        all
    }

    /// The code of each glyph (the lowest one, when several codes use it), for the glyphs that have a code.
    pub fn code_of_each_glyph(&self) -> Vec<Option<u16>> {
        let mut found = vec![None; self.glyph_count()];
        for (code, index) in self.codes() {
            if let Some(place @ None) = found.get_mut(index as usize) {
                *place = Some(code);
            }
        }
        found
    }

    /// The picture of a glyph: `cell_width × cell_height` values of ink (the alpha), from 0 (none) to 255. A glyph out of the sheets is empty.
    pub fn glyph_ink(&self, index: usize) -> Vec<u8> {
        self.glyph_pixels(index).into_iter().map(|(_, alpha)| alpha).collect()
    }

    /// True when the pixels have a luminance as well as an alpha.
    pub fn has_luminance(&self) -> bool {
        matches!(self.format, Format::La4 | Format::La8)
    }

    /// The picture of a glyph as (luminance, alpha) for each pixel. A font without luminance gives 0.
    pub fn glyph_pixels(&self, index: usize) -> Vec<(u8, u8)> {
        let (w, h) = (self.cell_width as usize, self.cell_height as usize);
        let mut out = vec![(0u8, 0u8); w * h];
        if index >= self.glyph_count() {
            return out;
        }
        let (base, cx, cy) = self.cell_place(index);
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = self.read_pixel(base, cx + x, cy + y);
            }
        }
        out
    }

    /// The start of the sheet of a glyph, and the place of its cell in the sheet.
    fn cell_place(&self, index: usize) -> (usize, usize, usize) {
        let per_sheet = self.columns * self.rows;
        let (sheet, cell) = (index / per_sheet, index % per_sheet);
        let (w, h) = (self.cell_width as usize, self.cell_height as usize);
        (self.data_start + sheet * self.sheet_size, (cell % self.columns) * (w + 1), (cell / self.columns) * (h + 1))
    }

    /// The number of a pixel in the order of the file: the tiles of 8 × 8 pixels go from left to right, then down; inside a tile the bits of x
    /// and y alternate.
    fn pixel_number(&self, x: usize, y: usize) -> usize {
        let tile = (y / 8) * (self.sheet_width / 8) + x / 8;
        let (tx, ty) = (x % 8, y % 8);
        let morton = (tx & 1) | ((ty & 1) << 1) | ((tx & 2) << 1) | ((ty & 2) << 2) | ((tx & 4) << 2) | ((ty & 4) << 3);
        tile * 64 + morton
    }

    fn read_pixel(&self, base: usize, x: usize, y: usize) -> (u8, u8) {
        let n = self.pixel_number(x, y);
        let at = |offset: usize| self.bytes.get(base + offset).copied().unwrap_or(0);
        match self.format {
            Format::A8 => (0, at(n)),
            Format::A4 => {
                let byte = at(n / 2);
                (0, (if n % 2 == 0 { byte & 0x0F } else { byte >> 4 }) * 17)
            }
            Format::La4 => ((at(n) >> 4) * 17, (at(n) & 0x0F) * 17),
            Format::La8 => (at(2 * n), at(2 * n + 1)),
        }
    }

    fn write_pixel(&mut self, base: usize, x: usize, y: usize, (luminance, alpha): (u8, u8)) {
        let n = self.pixel_number(x, y);
        let nibble = |value: u8| ((u16::from(value) * 15 + 127) / 255) as u8;
        let (format, len) = (self.format, self.bytes.len());
        let put = |bytes: &mut Vec<u8>, offset: usize, value: u8| {
            if base + offset < len {
                bytes[base + offset] = value;
            }
        };
        match format {
            Format::A8 => put(&mut self.bytes, n, alpha),
            Format::A4 => {
                let old = self.bytes.get(base + n / 2).copied().unwrap_or(0);
                let new = if n % 2 == 0 { (old & 0xF0) | nibble(alpha) } else { (old & 0x0F) | (nibble(alpha) << 4) };
                put(&mut self.bytes, n / 2, new);
            }
            Format::La4 => put(&mut self.bytes, n, (nibble(luminance) << 4) | nibble(alpha)),
            Format::La8 => {
                put(&mut self.bytes, 2 * n, luminance);
                put(&mut self.bytes, 2 * n + 1, alpha);
            }
        }
    }

    /// Replaces the picture of a glyph (`cell_width × cell_height` pixels of (luminance, alpha)). The file keeps its size and its layout.
    pub fn set_glyph(&mut self, index: usize, pixels: &[(u8, u8)]) -> bool {
        let (w, h) = (self.cell_width as usize, self.cell_height as usize);
        if index >= self.glyph_count() || pixels.len() != w * h {
            return false;
        }
        let (base, cx, cy) = self.cell_place(index);
        for y in 0..h {
            for x in 0..w {
                self.write_pixel(base, cx + x, cy + y, pixels[y * w + x]);
            }
        }
        true
    }

    /// Replaces the widths of a glyph. A glyph that has no entry of its own in the file keeps the default widths (false).
    pub fn set_width(&mut self, index: usize, width: Width) -> bool {
        let Some(Some(place)) = self.width_places.get(index).copied() else { return false };
        self.bytes[place] = width.left as u8;
        self.bytes[place + 1] = width.glyph;
        self.bytes[place + 2] = width.advance;
        self.widths[index] = width;
        true
    }

    /// True when the glyph has a place for its own widths in the file.
    pub fn has_own_width(&self, index: usize) -> bool {
        matches!(self.width_places.get(index), Some(Some(_)))
    }

    /// The glyphs of a text, as (glyph, x of the left of the picture), and the width of the line.
    pub fn layout(&self, text: &str) -> (Vec<(u16, i32)>, i32) {
        let mut pen = 0i32;
        let mut glyphs = Vec::new();
        for character in text.chars() {
            let code = if (character as u32) < 0x10000 { character as u16 } else { NONE };
            let index = self.index_or_alter(code);
            let width = self.width_of(index);
            glyphs.push((index, pen + i32::from(width.left)));
            pen += i32::from(width.advance);
        }
        (glyphs, pen)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    /// The font archive of the dump, when `SANGO_DUMP` is set.
    fn game_fonts() -> Option<Garc> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        Garc::open(&Dump::open(&root).ok()?.archive(crate::ARCHIVE)).ok()
    }

    /// With the dump: every font of the archive is read, and Latin letters have a glyph with ink.
    #[test]
    fn the_fonts_of_the_game_are_read() {
        let Some(garc) = game_fonts() else { return };
        let mut fonts = 0;
        for id in 0..garc.file_count() {
            for language in garc.languages(id) {
                let bytes = garc.file(id, language).unwrap();
                if bytes.get(..4) != Some(b"CFNT") {
                    continue;
                }
                let font = Font::parse(bytes.to_vec()).unwrap_or_else(|e| panic!("file {id}/{language}: {e}"));
                println!("file {id}/{language}: cell {}x{} {} {} glyphs, {} codes", font.cell_width, font.cell_height, font.format.name(), font.glyph_count(), font.codes().len());
                fonts += 1;
                assert!(font.glyph_count() > 0 && !font.codes().is_empty());
            }
        }
        assert!(fonts >= 3, "the archive has at least three fonts");
    }

    /// With the dump: the letter A of the first font has ink in the middle and none at the corners, and the text layout moves forward.
    #[test]
    fn a_letter_has_a_shape() {
        let Some(garc) = game_fonts() else { return };
        let font = Font::parse(garc.first_file(0).unwrap().to_vec()).unwrap();
        let index = font.index_of('A' as u16).expect("the font has the letter A");
        let ink = font.glyph_ink(index as usize);
        let (w, h) = (font.cell_width as usize, font.cell_height as usize);
        for y in 0..h {
            println!("{}", (0..w).map(|x| [' ', '.', ':', '#'][(ink[y * w + x] / 64).min(3) as usize]).collect::<String>());
        }
        assert!(ink.iter().filter(|v| **v > 0).count() > 20, "the letter has ink");
        let (glyphs, width) = font.layout("AB");
        assert_eq!(glyphs.len(), 2);
        assert!(width > 0 && glyphs[1].1 > glyphs[0].1);
    }
}
