//! The textures of a model: the PICA texture formats (tiles in Morton order), the size limits, and the file of the textures (BCH).

use crate::bch::{self, Section, Strings};

/// A texture of the file: the name, the size, the PICA format, the number of mip levels and the pixels as the GPU reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub format: u32,
    pub mips: u32,
    pub pixels: Vec<u8>,
}

/// The biggest texture, in pixels: 256 x 256.
pub const TEXTURE_LIMIT: usize = 256 * 256;
/// How much all the textures of a model may weigh, close to what the game ships for one Pokémon. A battle with a bigger pack
/// corrupted the textures of the other Pokémon.
pub const TEXTURE_BUDGET: usize = 600 * 1024;
/// A texture is never made smaller than this: a normal map that is too small turns the lighting into blocks.
pub const MIN_TEXTURE_DIM: usize = 128;

/// The bytes of a pixel for each PICA format (the compressed formats are left out: they cannot be resampled).
pub fn bytes_per_pixel(format: u32) -> Option<usize> {
    match format {
        0 => Some(4),
        1 => Some(3),
        2..=6 => Some(2),
        7..=9 => Some(1),
        _ => None,
    }
}

/// The place of the pixel (x, y) inside a tile of 8 x 8 pixels (the Morton order).
pub fn morton(x: usize, y: usize) -> usize {
    let mut index = 0;
    for bit in 0..3 {
        index |= ((y >> bit) & 1) << (2 * bit);
        index |= ((x >> bit) & 1) << (2 * bit + 1);
    }
    index
}

/// Tiles are stored column by column, each one in the Morton order. This turns tiled pixels into rows of pixels.
pub fn untile(data: &[u8], w: usize, h: usize, bpp: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * bpp];
    for ty in (0..h).step_by(8) {
        for tx in (0..w).step_by(8) {
            let tile = ((tx / 8) * (h / 8) + ty / 8) * 64;
            for y in 0..8 {
                for x in 0..8 {
                    let src = (tile + morton(x, y)) * bpp;
                    let dst = ((ty + y) * w + tx + x) * bpp;
                    out[dst..dst + bpp].copy_from_slice(&data[src..src + bpp]);
                }
            }
        }
    }
    out
}

/// The opposite: rows of pixels become tiles.
pub fn tile(data: &[u8], w: usize, h: usize, bpp: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * bpp];
    for ty in (0..h).step_by(8) {
        for tx in (0..w).step_by(8) {
            let base = ((tx / 8) * (h / 8) + ty / 8) * 64;
            for y in 0..8 {
                for x in 0..8 {
                    let dst = (base + morton(x, y)) * bpp;
                    let src = ((ty + y) * w + tx + x) * bpp;
                    out[dst..dst + bpp].copy_from_slice(&data[src..src + bpp]);
                }
            }
        }
    }
    out
}

/// Halves both sizes with a box filter.
fn halve(linear: &[u8], w: usize, h: usize, bpp: usize) -> (Vec<u8>, usize, usize) {
    let (nw, nh) = (w / 2, h / 2);
    let mut out = vec![0u8; nw * nh * bpp];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..bpp {
                let at = |yy: usize, xx: usize| linear[(yy * w + xx) * bpp + c] as u32;
                out[(y * nw + x) * bpp + c] = ((at(2 * y, 2 * x) + at(2 * y, 2 * x + 1) + at(2 * y + 1, 2 * x) + at(2 * y + 1, 2 * x + 1)) / 4) as u8;
            }
        }
    }
    (out, nw, nh)
}

fn resizable(t: &Texture) -> bool {
    bytes_per_pixel(t.format).is_some() && t.width >= 2 * MIN_TEXTURE_DIM && t.height >= 2 * MIN_TEXTURE_DIM
}

/// Makes the textures smaller, the biggest first, until they fit the limits (the size of one, then the weight of all).
pub fn fit_budget(textures: &mut [Texture]) {
    let mut linear: Vec<Option<Vec<u8>>> = vec![None; textures.len()];
    let mut shrink = |textures: &mut [Texture], i: usize| {
        let t = &textures[i];
        let bpp = bytes_per_pixel(t.format).expect("a format that can be resized");
        // The rows run along the dimension that the header gives second: the untiled image is (h, w) and not (w, h).
        let lin = linear[i].take().unwrap_or_else(|| untile(&t.pixels, t.height, t.width, bpp));
        let (lin, new_h, new_w) = halve(&lin, t.height, t.width, bpp);
        let pixels = tile(&lin, new_h, new_w, bpp);
        linear[i] = Some(lin);
        let t = &mut textures[i];
        t.width = new_w;
        t.height = new_h;
        t.pixels = pixels;
    };
    for i in 0..textures.len() {
        while textures[i].width * textures[i].height > TEXTURE_LIMIT && resizable(&textures[i]) {
            shrink(textures, i);
        }
    }
    while textures.iter().map(|t| t.pixels.len()).sum::<usize>() > TEXTURE_BUDGET {
        let biggest = (0..textures.len()).filter(|&i| resizable(&textures[i])).max_by_key(|&i| textures[i].pixels.len());
        match biggest {
            Some(i) => shrink(textures, i),
            None => break,
        }
    }
}

/// The file of the textures of a model (BCH). The textures are made smaller when they do not fit the limits.
pub fn write_texture_bch(mut textures: Vec<Texture>) -> Vec<u8> {
    fit_budget(&mut textures);
    let (mut sec, mut strs, mut cmds, mut raw) = (Section::new(), Strings::new(), Section::new(), Section::new());
    bch::main_header(&mut sec, &[3]);
    let names: Vec<&str> = textures.iter().map(|t| t.name.as_str()).collect();
    let dic = bch::write_dict(&mut sec, &mut strs, &names);
    let list = sec.tell();
    for _ in &textures {
        sec.u32(0);
    }
    sec.patch_ptr(0x24, 0, list as u32);
    sec.patch(0x28, textures.len() as u32);
    sec.patch_ptr(0x2C, 0, dic as u32);
    for (i, t) in textures.iter().enumerate() {
        raw.align(0x80);
        let address = raw.tell();
        raw.raw(&t.pixels);
        let header = sec.tell();
        sec.patch_ptr(list + 4 * i, 0, header as u32);
        // One list of commands for each of the three texture units.
        for (dim, kind, address_register, format_register) in [(0x82u32, 0x84u32, 0x85u32, 0x8Eu32), (0x92, 0x94, 0x95, 0x96), (0x9A, 0x9C, 0x9D, 0x9E)] {
            let words = [((t.width as u32) << 16) | t.height as u32, 0xF0000 | dim, 0, 0x40000 | kind, 0, 0xF0000 | address_register, t.format, 0xF0000 | format_register, 0, 0, 1, 0xF023D];
            let (offset, count) = bch::emit_commands(&mut cmds, &words, &[(4, 5, address as u32)]);
            sec.ptr(2, offset as u32);
            sec.u32(count as u32);
        }
        sec.u32(t.format | (t.mips << 8));
        let at = strs.get(&t.name);
        sec.ptr(1, at);
    }
    bch::build(&mut sec, &strs, &cmds, &raw)
}

#[cfg(test)]
mod test {
    use super::*;

    fn pixels(w: usize, h: usize, bpp: usize, seed: usize) -> Vec<u8> {
        (0..h).flat_map(|y| (0..w).flat_map(move |x| (0..bpp).map(move |c| ((x * 7 + y * 13 + c * 31 + seed) & 255) as u8))).collect()
    }

    fn texture(name: &str, w: usize, h: usize, format: u32, seed: usize) -> Texture {
        Texture { name: name.into(), width: w, height: h, format, mips: 1, pixels: pixels(w, h, bytes_per_pixel(format).unwrap(), seed) }
    }

    #[test]
    fn tiles_and_rows_are_the_same_pixels() {
        let rows = pixels(32, 16, 4, 3);
        assert_eq!(untile(&tile(&rows, 32, 16, 4), 32, 16, 4), rows);
        assert_eq!((morton(0, 0), morton(1, 0), morton(0, 1), morton(7, 7)), (0, 2, 1, 63));
    }

    /// The file of the textures is the one that the converter in Python writes, byte for byte.
    #[test]
    fn the_file_of_the_textures_is_the_one_of_the_converter() {
        let made = write_texture_bch(vec![texture("Eye", 16, 8, 0, 1), texture("BodyA", 64, 32, 1, 5)]);
        let golden = include_bytes!("../tests/golden/two_textures.bch");
        assert_eq!(made.len(), golden.len());
        assert!(made == golden.as_slice(), "the bytes are not the golden bytes");
    }

    #[test]
    fn a_texture_that_is_too_big_is_made_smaller() {
        let mut textures = vec![texture("Big", 512, 512, 0, 9)];
        fit_budget(&mut textures);
        assert_eq!((textures[0].width, textures[0].height), (256, 256));
        assert_eq!(textures[0].pixels.len(), 256 * 256 * 4);
    }
}
