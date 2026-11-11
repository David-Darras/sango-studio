//! The BCLIM format: the images of the 3DS layouts. The icons of the Pokémon use it.
//!
//! A file is the pixel data, then a footer of 40 bytes. All the numbers are little-endian.
//!
//! | Offset of the footer | Size | Name | Meaning |
//! | --- | --- | --- | --- |
//! | 0x00 | 4 | signature | `CLIM` |
//! | 0x0C | 4 | file size | The size of the whole file. |
//! | 0x1C | 2 | width | The width in pixels. |
//! | 0x1E | 2 | height | The height in pixels. |
//! | 0x20 | 4 | format | See `Format`. |
//!
//! The pixels are in tiles of 8 × 8 pixels. The tiles go from left to right, then from top to bottom,
//! over a texture whose sides are the next power of 2 (8 at least) of the width and of the height.
//! Inside a tile, the bits of the x and of the y of the pixel alternate (the Morton order).
//!
//! # Formats
//!
//! | Format | Name | Bytes |
//! | --- | --- | --- |
//! | 7 | RGBA5551 | 2 for each pixel. Bits 15..11 red, 10..6 green, 5..1 blue, bit 0 alpha. |
//! | 8 | RGBA4 | 2 for each pixel. Four bits for each of red, green, blue and alpha. |
//! | 10 | ETC1 | 8 for each block of 4 × 4 pixels. |
//! | 11 | ETC1A4 | 16 for each block of 4 × 4 pixels: 8 bytes of alpha, then 8 bytes of ETC1. |
//!
//! In a tile, the four blocks of 4 × 4 pixels go top-left, top-right, bottom-left, bottom-right.
//!
//! # Images with a palette
//!
//! The icons of the Pokémon use format 7 or 8 with a palette. The pixel data starts with two `u16`:
//! the size of one color (2) and the number of colors. Then come the colors (one `u16` each, in the
//! format of the image), then one index for each pixel of the texture. An index has 4 bits when the
//! palette has 16 colors or less, else 8 bits. With 4 bits, the first pixel is in the high bits of the byte.

/// The size of the footer.
const FOOTER_SIZE: usize = 0x28;

const SIGNATURE: &[u8; 4] = b"CLIM";
const WIDTH_AT: usize = 0x1C;
const HEIGHT_AT: usize = 0x1E;
const FORMAT_AT: usize = 0x20;

/// The pixel formats that this code reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Rgba5551,
    Rgba4,
    Etc1,
    Etc1A4,
}

impl Format {
    fn from_number(number: u32) -> Option<Format> {
        match number {
            7 => Some(Format::Rgba5551),
            8 => Some(Format::Rgba4),
            10 => Some(Format::Etc1),
            11 => Some(Format::Etc1A4),
            _ => None,
        }
    }
}

/// An image in memory. The bytes are kept as they are in the file, so `to_bytes` gives the file back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clim {
    width: usize,
    height: usize,
    format: Format,
    /// The pixel data: all the file but the footer.
    body: Vec<u8>,
    footer: [u8; FOOTER_SIZE],
}

/// The smallest side of a texture (a power of 2, 8 at least) that holds `value` pixels.
fn texture_side(value: usize) -> usize {
    value.max(8).next_power_of_two()
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize
}

impl Clim {
    /// Reads an image from the bytes of a file (already decompressed).
    pub fn parse(bytes: &[u8]) -> Result<Clim, String> {
        if bytes.len() <= FOOTER_SIZE {
            return Err("The image is too short.".into());
        }
        let body_size = bytes.len() - FOOTER_SIZE;
        let footer: [u8; FOOTER_SIZE] = bytes[body_size..].try_into().unwrap();
        if &footer[..4] != SIGNATURE {
            return Err("This is not a BCLIM image.".into());
        }
        let (width, height) = (u16_at(&footer, WIDTH_AT), u16_at(&footer, HEIGHT_AT));
        if width == 0 || height == 0 {
            return Err("The image has no pixel.".into());
        }
        let number = u32::from_le_bytes(footer[FORMAT_AT..FORMAT_AT + 4].try_into().unwrap());
        let format = Format::from_number(number).ok_or_else(|| format!("The image format {number} is not supported."))?;
        Ok(Clim { width, height, format, body: bytes[..body_size].to_vec(), footer })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.body.clone();
        bytes.extend_from_slice(&self.footer);
        bytes
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// The number of colors of the palette, or `None` when the image has no palette.
    ///
    /// A 16-bit image has a palette when its data starts with the size of a color (2) and a number of
    /// colors (1 to 256), and the colors and the indexes fit in the data. The files of the icons are
    /// padded to the size of the full texture, so the size of the data does not tell.
    pub fn palette_size(&self) -> Option<usize> {
        if !matches!(self.format, Format::Rgba5551 | Format::Rgba4) || self.body.len() < 4 {
            return None;
        }
        let (color_size, count) = (u16_at(&self.body, 0), u16_at(&self.body, 2));
        let pixels = texture_side(self.width) * texture_side(self.height);
        let index_bytes = if count <= 16 { pixels.div_ceil(2) } else { pixels };
        (color_size == 2 && (1..=256).contains(&count) && 4 + 2 * count + index_bytes <= self.body.len()).then_some(count)
    }

    /// Decodes the image to RGBA, 4 bytes for each pixel, row by row from the top.
    pub fn to_rgba(&self) -> Result<Vec<u8>, String> {
        let (side_x, side_y) = (texture_side(self.width), texture_side(self.height));
        let mut out = vec![0u8; self.width * self.height * 4];
        let mut put = |x: usize, y: usize, rgba: [u8; 4]| {
            if x < self.width && y < self.height {
                out[(y * self.width + x) * 4..][..4].copy_from_slice(&rgba);
            }
        };
        let tiles_per_row = side_x / 8;
        match self.format {
            Format::Rgba5551 | Format::Rgba4 => {
                let palette = self.palette_size();
                let (colors, indexes) = match palette {
                    Some(count) => (&self.body[4..(4 + 2 * count).min(self.body.len())], &self.body[(4 + 2 * count).min(self.body.len())..]),
                    None => (&self.body[..0], &self.body[..]),
                };
                let pixel_count = side_x * side_y;
                let needed = match palette {
                    Some(count) if count <= 16 => pixel_count.div_ceil(2),
                    Some(_) => pixel_count,
                    None => pixel_count * 2,
                };
                if indexes.len() < needed || colors.len() < palette.unwrap_or(0) * 2 {
                    return Err("The pixel data of the image is too short.".into());
                }
                for i in 0..pixel_count {
                    let raw = match palette {
                        None => u16_at(indexes, i * 2),
                        Some(count) => {
                            let mut index = if count <= 16 { (indexes[i / 2] >> if i & 1 == 1 { 0 } else { 4 }) & 0xF } else { indexes[i] } as usize;
                            if index >= count {
                                index = 0;
                            }
                            u16_at(colors, index * 2)
                        }
                    };
                    let (dx, dy) = morton_position(i % 64);
                    let tile = i / 64;
                    put((tile % tiles_per_row) * 8 + dx, (tile / tiles_per_row) * 8 + dy, self.rgba_16(raw));
                }
            }
            Format::Etc1 | Format::Etc1A4 => {
                let block_size = if self.format == Format::Etc1A4 { 16 } else { 8 };
                if self.body.len() < side_x * side_y / 16 * block_size {
                    return Err("The pixel data of the image is too short.".into());
                }
                let mut at = 0;
                for tile_y in (0..side_y).step_by(8) {
                    for tile_x in (0..side_x).step_by(8) {
                        for block in 0..4 {
                            let (left, top) = (tile_x + (block & 1) * 4, tile_y + (block >> 1) * 4);
                            let (alpha, color) = if block_size == 16 {
                                (u64::from_le_bytes(self.body[at..at + 8].try_into().unwrap()), u64::from_le_bytes(self.body[at + 8..at + 16].try_into().unwrap()))
                            } else {
                                (u64::MAX, u64::from_le_bytes(self.body[at..at + 8].try_into().unwrap()))
                            };
                            at += block_size;
                            etc1_block(alpha, color, |x, y, rgba| put(left + x, top + y, rgba));
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// One 16-bit pixel as RGBA.
    fn rgba_16(&self, raw: usize) -> [u8; 4] {
        if self.format == Format::Rgba4 {
            let nibble = |shift: u32| ((raw >> shift) & 0xF) as u8 * 17;
            [nibble(12), nibble(8), nibble(4), nibble(0)]
        } else {
            let wide = |shift: u32| {
                let value = ((raw >> shift) & 0x1F) as u8;
                value << 3 | value >> 2
            };
            [wide(11), wide(6), wide(1), if raw & 1 == 1 { 255 } else { 0 }]
        }
    }
}

/// The place of a pixel inside a tile of 8 × 8, from its number: the bits of x and y alternate.
fn morton_position(offset: usize) -> (usize, usize) {
    let x = (offset & 1) | ((offset >> 1) & 2) | ((offset >> 2) & 4);
    let y = ((offset >> 1) & 1) | ((offset >> 2) & 2) | ((offset >> 3) & 4);
    (x, y)
}

/// The modifiers of the ETC1 tables: the two small values (the other two have the opposite sign).
const ETC1_MODIFIERS: [[i32; 2]; 8] = [[2, 8], [5, 17], [9, 29], [13, 42], [18, 60], [24, 80], [33, 106], [47, 183]];

/// Decodes a block of 4 × 4 pixels of ETC1 and gives each pixel to `put`.
///
/// `alpha` has 4 bits for each pixel: the pixel (x, y) is at the bits (x × 4 + y) × 4.
/// `color` is the block: two base colors, two tables, and two bits for each pixel (bit x × 4 + y
/// for the low bit of the index, bit 16 + x × 4 + y for the sign).
fn etc1_block(alpha: u64, color: u64, mut put: impl FnMut(usize, usize, [u8; 4])) {
    let flipped = (color >> 32) & 1 == 1;
    let differential = (color >> 33) & 1 == 1;
    let mut base = [[0i32; 3]; 2];
    for channel in 0..3 {
        if differential {
            let shift = 59 - channel * 8;
            let first = ((color >> shift) & 0x1F) as i32;
            let mut delta = ((color >> (shift - 3)) & 0x7) as i32;
            if delta >= 4 {
                delta -= 8;
            }
            let second = (first + delta) & 0x1F;
            base[0][channel] = first << 3 | first >> 2;
            base[1][channel] = second << 3 | second >> 2;
        } else {
            let shift = 60 - channel * 8;
            base[0][channel] = ((color >> shift) & 0xF) as i32 * 17;
            base[1][channel] = ((color >> (shift - 4)) & 0xF) as i32 * 17;
        }
    }
    let tables = [((color >> 37) & 7) as usize, ((color >> 34) & 7) as usize];
    for x in 0..4 {
        for y in 0..4 {
            let bit = x * 4 + y;
            let half = if flipped { y >= 2 } else { x >= 2 } as usize;
            let modifiers = ETC1_MODIFIERS[tables[half]];
            let mut modifier = modifiers[((color >> bit) & 1) as usize];
            if (color >> (16 + bit)) & 1 == 1 {
                modifier = -modifier;
            }
            let channel = |c: usize| (base[half][c] + modifier).clamp(0, 255) as u8;
            let a = ((alpha >> (bit * 4)) & 0xF) as u8 * 17;
            put(x, y, [channel(0), channel(1), channel(2), a]);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A 16 × 8 image with a palette of two colors: the first tile is color 0, the second tile color 1.
    fn palette_image() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&0xF801u16.to_le_bytes()); // Red, opaque.
        bytes.extend_from_slice(&0x003Fu16.to_le_bytes()); // Blue, opaque.
        bytes.extend(std::iter::repeat(0x00).take(32)); // 64 indexes of 4 bits: color 0.
        bytes.extend(std::iter::repeat(0x11).take(32)); // Color 1.
        let mut footer = [0u8; FOOTER_SIZE];
        footer[..4].copy_from_slice(SIGNATURE);
        footer[WIDTH_AT..WIDTH_AT + 2].copy_from_slice(&16u16.to_le_bytes());
        footer[HEIGHT_AT..HEIGHT_AT + 2].copy_from_slice(&8u16.to_le_bytes());
        footer[FORMAT_AT] = 7;
        bytes.extend_from_slice(&footer);
        bytes
    }

    #[test]
    fn an_image_with_a_palette_is_decoded() {
        let bytes = palette_image();
        let image = Clim::parse(&bytes).unwrap();
        assert_eq!((image.width(), image.height(), image.format(), image.palette_size()), (16, 8, Format::Rgba5551, Some(2)));
        let rgba = image.to_rgba().unwrap();
        assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[(7 * 16 + 15) * 4..][..4], &[0, 0, 255, 255]);
        assert_eq!(image.to_bytes(), bytes);
    }

    #[test]
    fn the_morton_order_alternates_the_bits() {
        assert_eq!(morton_position(0), (0, 0));
        assert_eq!(morton_position(1), (1, 0));
        assert_eq!(morton_position(2), (0, 1));
        assert_eq!(morton_position(4), (2, 0));
        assert_eq!(morton_position(63), (7, 7));
    }

    /// With the dump of the game: set `SANGO_DUMP`. Every file of the archive 91 (the Pokémon icons)
    /// is decompressed, read, decoded and written back to the same bytes.
    #[test]
    fn every_pokemon_icon_of_the_dump_is_read_and_written_back() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let garc = crate::Garc::open(&dump.archive(91)).unwrap();
        let mut checked = 0;
        let mut palettes = 0;
        for id in 0..garc.file_count() {
            let Some(file) = garc.first_file(id) else { continue };
            let bytes = if crate::lz::is_compressed(file) { crate::lz::decompress(file).unwrap_or_else(|e| panic!("file {id}: {e}")) } else { file.to_vec() };
            let image = match Clim::parse(&bytes) {
                Ok(image) => image,
                Err(_) => continue, // The first files are not images.
            };
            assert_eq!(image.to_bytes(), bytes, "file {id}");
            let rgba = image.to_rgba().unwrap_or_else(|e| panic!("file {id}: {e}"));
            assert_eq!(rgba.len(), image.width() * image.height() * 4);
            assert!(rgba.chunks(4).any(|p| p[3] != 0), "file {id} is fully transparent");
            palettes += image.palette_size().is_some() as usize;            checked += 1;
        }
        assert!(checked > 900, "only {checked} images checked");
        assert!(palettes > 0);
    }

    #[test]
    fn a_bad_image_is_refused() {
        assert!(Clim::parse(&[0; 10]).is_err());
        let mut bytes = palette_image();
        let at = bytes.len() - FOOTER_SIZE;
        bytes[at] = b'X';
        assert!(Clim::parse(&bytes).is_err());
        let mut bytes = palette_image();
        let at = bytes.len() - FOOTER_SIZE + FORMAT_AT;
        bytes[at] = 99;
        assert!(Clim::parse(&bytes).unwrap_err().contains("99"));
    }
}
