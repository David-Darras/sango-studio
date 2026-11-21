//! Reads the images of a layout (`.bclim` files) as RGBA pixels.
//!
//! `sango_core::Clim` reads the common formats. The simple formats that it does not read (one value for each pixel, no block
//! compression) are read here. The pixels are in tiles of 8 × 8, with the Morton order inside a tile, like in [`sango_core::clim`].

use sango_core::Clim;

/// An image: the size and the RGBA pixels, row by row from the top.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

fn side(value: usize) -> usize {
    value.max(8).next_power_of_two()
}

/// The place of the pixel `i` (0 to 63) in a tile of 8 × 8: the bits of x and y alternate.
fn morton(i: usize) -> (usize, usize) {
    let (mut x, mut y) = (0, 0);
    for bit in 0..3 {
        x |= ((i >> (bit * 2)) & 1) << bit;
        y |= ((i >> (bit * 2 + 1)) & 1) << bit;
    }
    (x, y)
}

/// Decodes the bytes of a `.bclim` file.
pub fn decode(bytes: &[u8]) -> Result<Picture, String> {
    if let Ok(clim) = Clim::parse(bytes) {
        return Ok(Picture { width: clim.width(), height: clim.height(), rgba: clim.to_rgba()? });
    }
    if bytes.len() <= 40 || &bytes[bytes.len() - 40..bytes.len() - 36] != b"CLIM" {
        return Err("This is not a BCLIM image.".into());
    }
    let footer = &bytes[bytes.len() - 40..];
    let width = u16::from_le_bytes([footer[0x1C], footer[0x1D]]) as usize;
    let height = u16::from_le_bytes([footer[0x1E], footer[0x1F]]) as usize;
    let format = u32::from_le_bytes(footer[0x20..0x24].try_into().unwrap());
    let body = &bytes[..bytes.len() - 40];
    let (sx, sy) = (side(width), side(height));
    let bits_per_pixel = match format {
        0 | 1 | 2 => 8,
        3 | 5 => 16,
        6 => 24,
        12 | 13 => 4,
        _ => return Err(format!("The image format {format} is not supported.")),
    };
    if body.len() < sx * sy * bits_per_pixel / 8 {
        return Err("The pixel data of the image is too short.".into());
    }
    let mut rgba = vec![0u8; width * height * 4];
    for i in 0..sx * sy {
        let (dx, dy) = morton(i % 64);
        let tile = i / 64;
        let (x, y) = ((tile % (sx / 8)) * 8 + dx, (tile / (sx / 8)) * 8 + dy);
        if x >= width || y >= height {
            continue;
        }
        let pixel = match format {
            0 => [body[i], body[i], body[i], 255],
            1 => [255, 255, 255, body[i]],
            2 => {
                let (l, a) = (body[i] >> 4, body[i] & 15);
                [l * 17, l * 17, l * 17, a * 17]
            }
            3 => [body[i * 2 + 1], body[i * 2 + 1], body[i * 2 + 1], body[i * 2]],
            5 => {
                let v = u16::from_le_bytes([body[i * 2], body[i * 2 + 1]]) as usize;
                let wide = |bits: u32, shift: u32| (((v >> shift) & ((1 << bits) - 1)) * 255 / ((1 << bits) - 1)) as u8;
                [wide(5, 11), wide(6, 5), wide(5, 0), 255]
            }
            6 => [body[i * 3 + 2], body[i * 3 + 1], body[i * 3], 255],
            12 => {
                let l = (body[i / 2] >> if i % 2 == 0 { 0 } else { 4 }) & 15;
                [l * 17, l * 17, l * 17, 255]
            }
            _ => {
                let a = (body[i / 2] >> if i % 2 == 0 { 0 } else { 4 }) & 15;
                [255, 255, 255, a * 17]
            }
        };
        rgba[(y * width + x) * 4..][..4].copy_from_slice(&pixel);
    }
    Ok(Picture { width, height, rgba })
}
