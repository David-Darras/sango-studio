//! The font of the game: the characters that it has, and the picture of each one.
//!
//! The font is the file 0 of the archive [`ARCHIVE`] (the font of the Japanese, English and European games; the file 1 is the
//! font of the Korean game, the file 2 the font of the battles). The game draws its texts with this font, and with the font of
//! the console for the characters that it does not have.
//!
//! **The file** starts with `CFNT`, then blocks. Each block starts with 4 letters and its size (u32, with its header):
//!
//! | Block | Content |
//! | --- | --- |
//! | `FINF` | The heights of a line (byte 9 after the start of the block). |
//! | `TGLP` | The pictures. Byte 8: width of a cell, 9: height of a cell, 10: base line, 12: size of a sheet (u32), 16: number of sheets (u16), 18: format (u16, 11 = 4 bits of alpha for each pixel), 20: cells in a row of a sheet, 22: rows of cells, 24: width of a sheet, 26: height of a sheet, 28: place of the first sheet in the file (u32). |
//! | `CWDH` | The widths: first and last glyph (u16, u16), next block (u32), then 3 bytes for each glyph: left space (i8), width of the picture, width of the character. |
//! | `CMAP` | The characters: first and last character (u16, u16), kind (u16), next block (u32), then the table. Kind 0: the glyphs follow each other from the u16 of the table. Kind 1: one u16 for each character (0xFFFF = none). Kind 2: a count (u16), then pairs (character, glyph). |
//!
//! A sheet is cut in tiles of 8 × 8 pixels, from left to right and top to bottom; inside a tile the pixels are in the Morton
//! order. A cell has 1 pixel of space on its left and on its top.

use std::collections::BTreeMap;

/// The archive of the fonts.
pub const ARCHIVE: usize = 167;

/// The format of the sheets that this reader knows: 4 bits of alpha for each pixel.
const FORMAT_A4: u16 = 11;

/// The widths of a glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Widths {
    /// The space before the picture.
    pub left: i8,
    /// The width of the picture.
    pub glyph: u8,
    /// The space that the character takes in the line.
    pub advance: u8,
}

/// The picture of one character: alpha values (0 to 255), one byte for each pixel, row after row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub width: usize,
    pub height: usize,
    pub alpha: Vec<u8>,
    pub widths: Widths,
}

/// A font of the game, read from its file.
#[derive(Clone, Debug)]
pub struct GameFont {
    bytes: Vec<u8>,
    cell_width: usize,
    cell_height: usize,
    /// The distance from the top of a cell to the base line.
    pub baseline: usize,
    /// The height of a line.
    pub line_height: usize,
    sheet_size: usize,
    sheet_count: usize,
    cells_per_row: usize,
    rows: usize,
    sheet_width: usize,
    first_sheet: usize,
    /// The glyph of each character.
    map: BTreeMap<u16, u16>,
    widths: Vec<Widths>,
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "The font file is too short.".to_string())
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "The font file is too short.".to_string())
}

impl GameFont {
    /// Reads a font file.
    pub fn parse(bytes: &[u8]) -> Result<GameFont, String> {
        if bytes.get(0..4) != Some(b"CFNT") {
            return Err("The file is not a font (no CFNT).".into());
        }
        let mut font = GameFont {
            bytes: bytes.to_vec(),
            cell_width: 0,
            cell_height: 0,
            baseline: 0,
            line_height: 0,
            sheet_size: 0,
            sheet_count: 0,
            cells_per_row: 0,
            rows: 0,
            sheet_width: 0,
            first_sheet: 0,
            map: BTreeMap::new(),
            widths: Vec::new(),
        };
        let mut at = u16_at(bytes, 6)? as usize;
        let mut format = 0;
        while at + 8 <= bytes.len() {
            let magic = &bytes[at..at + 4];
            let size = u32_at(bytes, at + 4)? as usize;
            if size < 8 {
                break;
            }
            match magic {
                b"FINF" => font.line_height = *bytes.get(at + 9).ok_or("The font file is too short.")? as usize,
                b"TGLP" => {
                    font.cell_width = bytes[at + 8] as usize;
                    font.cell_height = bytes[at + 9] as usize;
                    font.baseline = bytes[at + 10] as usize;
                    font.sheet_size = u32_at(bytes, at + 12)? as usize;
                    font.sheet_count = u16_at(bytes, at + 16)? as usize;
                    format = u16_at(bytes, at + 18)?;
                    font.cells_per_row = u16_at(bytes, at + 20)? as usize;
                    font.rows = u16_at(bytes, at + 22)? as usize;
                    font.sheet_width = u16_at(bytes, at + 24)? as usize;
                    font.first_sheet = u32_at(bytes, at + 28)? as usize;
                }
                b"CWDH" => {
                    let first = u16_at(bytes, at + 8)? as usize;
                    let last = u16_at(bytes, at + 10)? as usize;
                    if font.widths.len() < last + 1 {
                        font.widths.resize(last + 1, Widths::default());
                    }
                    for glyph in first..=last {
                        let entry = at + 16 + 3 * (glyph - first);
                        let raw = bytes.get(entry..entry + 3).ok_or("The font file is too short.")?;
                        font.widths[glyph] = Widths { left: raw[0] as i8, glyph: raw[1], advance: raw[2] };
                    }
                }
                b"CMAP" => {
                    let first = u16_at(bytes, at + 8)?;
                    let last = u16_at(bytes, at + 10)?;
                    let kind = u16_at(bytes, at + 12)?;
                    let table = at + 20;
                    match kind {
                        0 => {
                            let start = u16_at(bytes, table)?;
                            for code in first..=last {
                                font.map.insert(code, start + (code - first));
                            }
                        }
                        1 => {
                            for (place, code) in (first..=last).enumerate() {
                                let glyph = u16_at(bytes, table + 2 * place)?;
                                if glyph != 0xFFFF {
                                    font.map.insert(code, glyph);
                                }
                            }
                        }
                        _ => {
                            let count = u16_at(bytes, table)? as usize;
                            for place in 0..count {
                                font.map.insert(u16_at(bytes, table + 2 + 4 * place)?, u16_at(bytes, table + 4 + 4 * place)?);
                            }
                        }
                    }
                }
                _ => {}
            }
            at += size;
        }
        if format != FORMAT_A4 || font.cells_per_row == 0 || font.sheet_width % 8 != 0 {
            return Err(format!("The font has sheets of the format {format}: only the format {FORMAT_A4} is known."));
        }
        Ok(font)
    }

    /// Reads the font of the game from a dump.
    pub fn open(dump: &crate::Dump) -> Result<GameFont, String> {
        let garc = crate::Garc::open(&dump.archive(ARCHIVE))?;
        let bytes = garc.first_file_decompressed(0).ok_or("The archive of the fonts has no file 0.")?;
        GameFont::parse(&bytes)
    }

    /// The characters that the font has, in order.
    pub fn characters(&self) -> Vec<u16> {
        self.map.keys().copied().collect()
    }

    /// True when the font has the character.
    pub fn has(&self, code: u16) -> bool {
        self.map.contains_key(&code)
    }

    /// The widths of a character.
    pub fn widths(&self, code: u16) -> Option<Widths> {
        let glyph = *self.map.get(&code)? as usize;
        self.widths.get(glyph).copied()
    }

    /// The picture of a character: the whole cell (the widths say which part is the character).
    pub fn glyph(&self, code: u16) -> Option<Glyph> {
        let index = *self.map.get(&code)? as usize;
        let per_sheet = self.cells_per_row * self.rows;
        let sheet = index / per_sheet;
        if sheet >= self.sheet_count {
            return None;
        }
        let cell = index % per_sheet;
        let (column, row) = (cell % self.cells_per_row, cell / self.cells_per_row);
        let data = self.bytes.get(self.first_sheet + sheet * self.sheet_size..self.first_sheet + (sheet + 1) * self.sheet_size)?;
        let (x0, y0) = (column * (self.cell_width + 1) + 1, row * (self.cell_height + 1) + 1);
        let mut alpha = vec![0u8; self.cell_width * self.cell_height];
        for y in 0..self.cell_height {
            for x in 0..self.cell_width {
                alpha[y * self.cell_width + x] = self.pixel(data, x0 + x, y0 + y);
            }
        }
        Some(Glyph { width: self.cell_width, height: self.cell_height, alpha, widths: self.widths.get(index).copied().unwrap_or_default() })
    }

    /// The alpha of a pixel of a sheet (0 to 255).
    fn pixel(&self, sheet: &[u8], x: usize, y: usize) -> u8 {
        let tiles_per_row = self.sheet_width / 8;
        let tile = (y / 8) * tiles_per_row + x / 8;
        let (dx, dy) = (x % 8, y % 8);
        // The Morton number of the place inside the tile: the bits of x and y alternate, x first.
        let mut inside = 0;
        for bit in 0..3 {
            inside |= ((dx >> bit) & 1) << (2 * bit);
            inside |= ((dy >> bit) & 1) << (2 * bit + 1);
        }
        let number = tile * 64 + inside;
        let byte = sheet.get(number / 2).copied().unwrap_or(0);
        let value = if number % 2 == 0 { byte & 0x0F } else { byte >> 4 };
        value * 17
    }
}

/// The groups of characters of the keyboard of the editor: a name and a range of characters. The keyboard shows the characters of
/// the font that are in each range.
pub const GROUPS: [(&str, u16, u16); 12] = [
    ("Signs of the game", 0xE000, 0xF8FF),
    ("Latin", 0x0020, 0x007E),
    ("Latin accents", 0x00A0, 0x024F),
    ("Greek", 0x0370, 0x03FF),
    ("Cyrillic", 0x0400, 0x04FF),
    ("Punctuation", 0x2000, 0x206F),
    ("Arrows and letters", 0x2100, 0x21FF),
    ("Mathematics", 0x2200, 0x23FF),
    ("Shapes and symbols", 0x2460, 0x27BF),
    ("Japanese", 0x3000, 0x30FF),
    ("Full width", 0xFF00, 0xFFEF),
    ("Korean", 0xAC00, 0xD7A3),
];

/// The names of the signs of the game (characters 0xE07F to 0xE0A8), from their pictures in the font.
pub fn sign_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0xE080..=0xE083 | 0xE09F..=0xE0A2 => "face",
        0xE084 | 0xE0A3 => "arrow up",
        0xE085 | 0xE0A4 => "arrow down",
        0xE086 | 0xE0A5 => "sleep",
        0xE087 => "times",
        0xE088 => "divided by",
        0xE08D => "ellipsis",
        0xE08E => "male",
        0xE08F => "female",
        0xE090 => "spade",
        0xE091 => "club",
        0xE092 => "heart",
        0xE093 => "diamond",
        0xE094 => "star",
        0xE095 => "double circle",
        0xE096 => "circle",
        0xE097 => "square",
        0xE098 => "triangle",
        0xE099 => "rhombus",
        0xE09A => "note",
        0xE09B => "sun",
        0xE09C => "cloud",
        0xE09D => "umbrella",
        0xE09E => "snowman",
        0xE0A7 => "PK",
        0xE0A8 => "MN",
        _ => return None,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump of the game: the font has the letters, the accents and the signs of the game, and the letter "A" has ink in
    /// its picture.
    #[test]
    fn the_font_of_the_dump_is_read() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let font = GameFont::open(&dump).unwrap();
        let characters = font.characters();
        assert_eq!(characters.len(), 3312);
        assert!(font.has('A' as u16) && font.has('é' as u16) && font.has(0xE08E));
        let a = font.glyph('A' as u16).unwrap();
        assert_eq!((a.width, a.height), (24, 30));
        assert!(a.alpha.iter().filter(|&&v| v > 128).count() > 20);
        assert!(a.widths.advance > 0);
        // A picture to look at: the letters "A", "g" and the sign 0xE08E side by side, in the folder of the tests.
        if let Ok(folder) = std::env::var("SANGO_FONT_PNG") {
            let codes: Vec<u16> = std::env::var("SANGO_FONT_CODES").ok().map(|list| list.split(',').filter_map(|c| u16::from_str_radix(c, 16).ok()).collect()).unwrap_or_else(|| vec!['A' as u16, 'g' as u16, 0xE08E, 0xE08F]);
            let mut rgba = Vec::new();
            for y in 0..a.height {
                for &code in &codes {
                    let glyph = font.glyph(code).unwrap();
                    for x in 0..glyph.width {
                        let value = 255 - glyph.alpha[y * glyph.width + x];
                        rgba.extend([value, value, value, 255]);
                    }
                }
            }
            let picture = image::RgbaImage::from_raw((a.width * codes.len()) as u32, a.height as u32, rgba).unwrap();
            picture.save(std::path::Path::new(&folder).join("font.png")).unwrap();
        }
    }
}
