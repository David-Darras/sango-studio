//! The sheet of glyphs: a picture of a font that the user can draw over, and the file of the widths that goes with it.
//!
//! The sheet looks like the pictures of the font converter of the 3DS tools: a grid of blocks, 16 on a line, one block for each glyph in the
//! order of the font. A block has:
//!
//! - a border of 1 pixel (black) and a margin of 1 pixel (grey-green), on the four sides,
//! - the **cell**, `cell_width × cell_height` pixels, that holds the picture of the glyph,
//! - under the cell a margin, then a **bar** of 1 pixel (red) and a border. The bar starts at the space on the left of the glyph and is as
//!   long as the width of the character: it shows the spacing.
//!
//! So a block is `cell_width + 4` pixels wide and `cell_height + 5` pixels high. A block that has no glyph (the end of the last line) has a
//! blue cell.
//!
//! In a font whose pixels have only an alpha, the cell is white and the ink is dark (a darker pixel is more opaque). In a font whose pixels
//! have a luminance and an alpha (an outlined font), the picture has the real alpha: the pixels that you do not draw are transparent.
//!
//! The widths of the glyphs are in a text file with the same name and the extension `csv`: one line for each glyph, `index,code,left,glyph,advance`.
//! `left` is the space on the left of the picture, `glyph` the width of the picture, `advance` the width of the character. When the file is
//! missing, the import reads the bar and the picture.

use crate::bcfnt::{Font, Width};

/// The number of blocks on a line of the sheet.
pub const COLUMNS: usize = 16;

const GRID: [u8; 4] = [0, 0, 0, 255];
const MARGIN: [u8; 4] = [153, 170, 153, 255];
const BAR: [u8; 4] = [255, 0, 0, 255];
const NULL_CELL: [u8; 4] = [0, 128, 255, 255];

/// A picture of 8 bits for each of red, green, blue and alpha.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// The size of a block of the sheet.
pub fn block_size(font: &Font) -> (usize, usize) {
    (font.cell_width as usize + 4, font.cell_height as usize + 5)
}

/// The size of the sheet for a font.
pub fn sheet_size(font: &Font) -> (usize, usize) {
    let (u, v) = block_size(font);
    (COLUMNS * u, font.glyph_count().div_ceil(COLUMNS) * v)
}

/// Draws the sheet of a font.
pub fn draw(font: &Font) -> Picture {
    let (u, v) = block_size(font);
    let (width, height) = sheet_size(font);
    let mut rgba = vec![0u8; width * height * 4];
    let mut put = |x: usize, y: usize, color: [u8; 4]| rgba[(y * width + x) * 4..(y * width + x) * 4 + 4].copy_from_slice(&color);
    let (w, h) = (font.cell_width as usize, font.cell_height as usize);
    let lines = font.glyph_count().div_ceil(COLUMNS);
    for line in 0..lines {
        for column in 0..COLUMNS {
            let index = line * COLUMNS + column;
            let (x0, y0) = (column * u, line * v);
            let exists = index < font.glyph_count();
            for y in 0..v {
                for x in 0..u {
                    let edge = x == 0 || y == 0 || x == u - 1 || y == v - 1;
                    let margin = x == 1 || y == 1 || x == u - 2 || y == v - 3;
                    let color = if edge { GRID } else if margin { MARGIN } else if exists { [255, 255, 255, 255] } else { NULL_CELL };
                    put(x0 + x, y0 + y, color);
                }
            }
            if !exists {
                continue;
            }
            // The cell.
            let pixels = font.glyph_pixels(index);
            for y in 0..h {
                for x in 0..w {
                    let (luminance, alpha) = pixels[y * w + x];
                    let color = if font.has_luminance() {
                        [luminance, luminance, luminance, alpha]
                    } else {
                        let gray = 255 - alpha;
                        [gray, gray, gray, 255]
                    };
                    put(x0 + 2 + x, y0 + 2 + y, color);
                }
            }
            // The bar: the row under the margin below the cell.
            let width_of = font.width_of(index as u16);
            let start = i32::from(width_of.left).max(0) as usize;
            let length = (width_of.advance as usize).min(w.saturating_sub(start));
            for x in 0..w {
                put(x0 + 2 + x, y0 + 3 + h, if x >= start && x < start + length { BAR } else { MARGIN });
            }
        }
    }
    Picture { width, height, rgba }
}

/// The file of the widths for a font.
pub fn metrics_csv(font: &Font) -> String {
    let codes = font.code_of_each_glyph();
    let mut text = String::from("index,code,left,glyph,advance\n");
    for index in 0..font.glyph_count() {
        let w = font.width_of(index as u16);
        let code = codes[index].map_or(String::new(), |c| format!("{c:04X}"));
        text.push_str(&format!("{index},{code},{},{},{}\n", w.left, w.glyph, w.advance));
    }
    text
}

/// Reads the file of the widths: the width of each glyph that it names.
pub fn parse_metrics(text: &str) -> Result<Vec<(usize, Width)>, String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() < 5 {
            return Err(format!("Line {}: five values are needed (index, code, left, glyph, advance).", number + 1));
        }
        let bad = |what: &str| format!("Line {}: the {what} is not a number in the right range.", number + 1);
        let index: usize = parts[0].parse().map_err(|_| bad("index"))?;
        let left: i8 = parts[2].parse().map_err(|_| bad("left space"))?;
        let glyph: u8 = parts[3].parse().map_err(|_| bad("glyph width"))?;
        let advance: u8 = parts[4].parse().map_err(|_| bad("character width"))?;
        found.push((index, Width { left, glyph, advance }));
    }
    Ok(found)
}

/// What a sheet read gave.
pub struct Imported {
    pub font: Font,
    /// How many glyphs the sheet changed.
    pub changed: usize,
    /// True when the widths come from the file of the widths, false when they come from the bars and the pictures.
    pub widths_from_file: bool,
}

/// Makes a font from the font that the sheet was drawn from, and the sheet that the user drew over. The font keeps its layout, its size and
/// its map of characters; only the pictures and the widths change.
pub fn import(base: &Font, picture: &Picture, metrics: Option<&str>) -> Result<Imported, String> {
    let (width, height) = sheet_size(base);
    if (picture.width, picture.height) != (width, height) {
        return Err(format!(
            "The sheet is {}×{} pixels and this font needs {width}×{height}. Draw over the sheet that the export made, and keep its size.",
            picture.width, picture.height
        ));
    }
    let table = match metrics {
        Some(text) => Some(parse_metrics(text)?),
        None => None,
    };
    let (u, v) = block_size(base);
    let (w, h) = (base.cell_width as usize, base.cell_height as usize);
    let pixel = |x: usize, y: usize| -> [u8; 4] {
        let at = (y * width + x) * 4;
        [picture.rgba[at], picture.rgba[at + 1], picture.rgba[at + 2], picture.rgba[at + 3]]
    };
    let mut font = base.clone();
    let mut changed = 0;
    for index in 0..base.glyph_count() {
        let (x0, y0) = ((index % COLUMNS) * u, (index / COLUMNS) * v);
        let mut cell = Vec::with_capacity(w * h);
        let mut right_ink = 0;
        for y in 0..h {
            for x in 0..w {
                let [r, g, b, a] = pixel(x0 + 2 + x, y0 + 2 + y);
                let luminance = ((u32::from(r) + u32::from(g) + u32::from(b)) / 3) as u8;
                let value = if base.has_luminance() {
                    (luminance, a)
                } else {
                    // Dark ink on white: the picture is composed over white, and what is not white is ink.
                    let over_white = (u32::from(luminance) * u32::from(a) + 255 * (255 - u32::from(a))) / 255;
                    (0, 255 - over_white as u8)
                };
                if value.1 > 0 {
                    right_ink = right_ink.max(x + 1);
                }
                cell.push(value);
            }
        }
        if cell != base.glyph_pixels(index) {
            changed += 1;
        }
        font.set_glyph(index, &cell);

        // The widths: the file, else the bar and the picture.
        let old = base.width_of(index as u16);
        let new = match &table {
            Some(rows) => rows.iter().find(|(i, _)| *i == index).map(|(_, w)| *w).unwrap_or(old),
            None => {
                let is_bar = |x: usize| {
                    let [r, g, b, a] = pixel(x0 + 2 + x, y0 + 3 + h);
                    a > 0 && r > 200 && g < 80 && b < 80
                };
                let red: Vec<usize> = (0..w).filter(|x| is_bar(*x)).collect();
                match red.first() {
                    Some(&start) => Width { left: start as i8, glyph: right_ink.min(255) as u8, advance: red.len() as u8 },
                    None => Width { left: 0, glyph: right_ink.min(255) as u8, advance: old.advance },
                }
            }
        };
        if new != old {
            font.set_width(index, new);
        }
    }
    Ok(Imported { font, changed, widths_from_file: table.is_some() })
}

/// Reads a PNG file.
pub fn read_png(bytes: &[u8]) -> Result<Picture, String> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| format!("This is not a PNG picture: {e}"))?.to_rgba8();
    Ok(Picture { width: image.width() as usize, height: image.height() as usize, rgba: image.into_raw() })
}

/// Writes a PNG file.
pub fn write_png(picture: &Picture) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let image = image::RgbaImage::from_raw(picture.width as u32, picture.height as u32, picture.rgba.clone()).ok_or("The picture has a wrong size.")?;
    image.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    fn game_fonts() -> Option<(Garc, Vec<(usize, usize)>)> {
        let root = std::env::var("SANGO_DUMP").ok()?;
        let garc = Garc::open(&Dump::open(&root).ok()?.archive(crate::ARCHIVE)).ok()?;
        let mut fonts = Vec::new();
        for id in 0..garc.file_count() {
            for language in garc.languages(id) {
                if garc.file(id, language).is_some_and(|b| b.get(..4) == Some(b"CFNT")) {
                    fonts.push((id, language));
                }
            }
        }
        Some((garc, fonts))
    }

    /// With the dump: the sheet of every font, read again with its file of widths, gives the same font byte for byte; read again without
    /// the file of widths, it gives the same pictures.
    #[test]
    fn the_sheet_of_every_font_comes_back_as_it_was() {
        let Some((garc, fonts)) = game_fonts() else { return };
        let out = std::env::var("SANGO_FONT_OUT").ok();
        for (id, language) in fonts {
            let original = Font::parse(garc.file(id, language).unwrap().to_vec()).unwrap();
            let picture = draw(&original);
            let png = write_png(&picture).unwrap();
            if let Some(folder) = &out {
                std::fs::write(format!("{folder}/font_{id}_{language}.png"), &png).unwrap();
            }
            let again = read_png(&png).unwrap();
            let csv = metrics_csv(&original);
            let with = import(&original, &again, Some(&csv)).unwrap();
            assert_eq!(with.changed, 0, "font {id}/{language}: the pictures are the same");
            assert!(with.font.bytes() == original.bytes(), "font {id}/{language}: the bytes are the same");
            let without = import(&original, &again, None).unwrap();
            assert_eq!(without.changed, 0, "font {id}/{language}");
            for index in 0..original.glyph_count() {
                assert_eq!(without.font.glyph_pixels(index), original.glyph_pixels(index));
            }
        }
    }

    /// A drawn pixel and a changed width are written in the font, and a sheet of the wrong size is refused.
    #[test]
    fn a_change_on_the_sheet_changes_the_font() {
        let Some((garc, fonts)) = game_fonts() else { return };
        let (id, language) = fonts[0];
        let original = Font::parse(garc.file(id, language).unwrap().to_vec()).unwrap();
        let mut picture = draw(&original);
        let (u, v) = block_size(&original);
        // The first pixel of the cell of glyph 1 becomes black.
        let at = ((v + 2) * picture.width + u + 2) * 4;
        picture.rgba[at..at + 4].copy_from_slice(&[0, 0, 0, 255]);
        let result = import(&original, &picture, None).unwrap();
        assert_eq!(result.changed, 1);
        assert_eq!(result.font.glyph_pixels(COLUMNS + 1)[0].1, 255);
        let mut csv = metrics_csv(&original);
        csv = csv.replacen("
0,", "
0,", 1);
        let edited = csv.lines().map(|l| if l.starts_with("5,") { "5,,3,7,9".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("
");
        let result = import(&original, &draw(&original), Some(&edited)).unwrap();
        assert_eq!(result.font.width_of(5), Width { left: 3, glyph: 7, advance: 9 });
        let small = Picture { width: 10, height: 10, rgba: vec![0; 400] };
        assert!(import(&original, &small, None).is_err());
        assert!(parse_metrics("index,code,left,glyph,advance
1,,x,2,3
").is_err());
    }
}
