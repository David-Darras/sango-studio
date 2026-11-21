//! Makes the glyphs of a font of the game from a font of the computer (`.ttf` or `.otf`).
//!
//! For each character that the font of the game has, the generator draws the character of the computer font at a size, on the baseline of
//! the cell, and sets the three widths of the glyph from the shape (the space on the left, the width of the picture) and from the advance of
//! the computer font. The glyphs that have no character in the font of the game (symbols, icons), and the characters that the computer font
//! does not have, keep the picture of the game.

use crate::bcfnt::{Font, Width};
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont};
use std::path::PathBuf;

/// A font file of the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFont {
    pub name: String,
    pub path: PathBuf,
}

/// The font files of the computer: the folder of Windows and the folder of the fonts of the user.
pub fn system_fonts() -> Vec<SystemFont> {
    let mut folders = Vec::new();
    if let Some(windows) = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")) {
        folders.push(PathBuf::from(windows).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        folders.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    let mut found = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let extension = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
            if matches!(extension.as_deref(), Some("ttf") | Some("otf")) {
                let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                found.push(SystemFont { name, path });
            }
        }
    }
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// What the user chooses.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The height of the letters in pixels.
    pub size: f32,
    /// Only the Latin characters (U+0020 to U+024F).
    pub latin_only: bool,
    /// For the fonts with a luminance: a white letter with a black outline of 1 pixel, as the pixel font of the game.
    pub outline: bool,
}

/// What the generator did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The glyphs that were drawn.
    pub drawn: usize,
    /// The characters of the game font that the computer font does not have.
    pub missing: usize,
    /// The glyphs that did not fit in the cell (the part outside is cut).
    pub cut: usize,
}

/// The ink of one character: the pixels of the cell (alpha), the space on the left, and the width of the picture.
fn rasterize(scaled: &ab_glyph::PxScaleFont<&FontRef>, character: char, cell: (usize, usize), baseline: i32) -> Option<(Vec<u8>, i32, usize, bool)> {
    let id = scaled.glyph_id(character);
    if id.0 == 0 {
        return None;
    }
    let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(0.0, baseline as f32));
    let mut alpha = vec![0u8; cell.0 * cell.1];
    let Some(outline) = scaled.outline_glyph(glyph) else {
        // A glyph without a shape, such as a space.
        return Some((alpha, 0, 0, false));
    };
    let bounds = outline.px_bounds();
    let (x0, y0) = (bounds.min.x.floor() as i32, bounds.min.y.floor() as i32);
    let mut cut = false;
    let mut right = 0usize;
    outline.draw(|dx, dy, coverage| {
        let (x, y) = (dx as i32, y0 + dy as i32);
        if x < 0 || y < 0 || x as usize >= cell.0 || y as usize >= cell.1 {
            if coverage > 0.2 {
                cut = true;
            }
            return;
        }
        let value = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        if value > 0 {
            alpha[y as usize * cell.0 + x as usize] = value;
            right = right.max(x as usize + 1);
        }
    });
    Some((alpha, x0, right, cut))
}

/// Draws a white letter with a black outline of 1 pixel from the coverage of the letter.
fn outlined(alpha: &[u8], cell: (usize, usize)) -> Vec<(u8, u8)> {
    let at = |x: i32, y: i32| if x < 0 || y < 0 || x as usize >= cell.0 || y as usize >= cell.1 { 0 } else { alpha[y as usize * cell.0 + x as usize] };
    let mut out = vec![(0u8, 0u8); cell.0 * cell.1];
    for y in 0..cell.1 as i32 {
        for x in 0..cell.0 as i32 {
            let own = at(x, y);
            let around = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| at(x + dx, y + dy)).max().unwrap_or(0);
            out[y as usize * cell.0 + x as usize] = if own >= 128 { (255, 255) } else if around > 0 { (0, around.max(own)) } else { (0, 0) };
        }
    }
    out
}

/// Makes a font from the font of the game and a font file of the computer.
pub fn generate(base: &Font, file: &[u8], settings: &Settings) -> Result<(Font, Report), String> {
    let source = FontRef::try_from_slice(file).map_err(|e| format!("This file is not a font that the program can read: {e}"))?;
    let scaled = source.as_scaled(PxScale::from(settings.size.clamp(4.0, 120.0)));
    let cell = (base.cell_width as usize, base.cell_height as usize);
    let mut font = base.clone();
    let mut report = Report::default();
    for (code, index) in base.codes() {
        let index = index as usize;
        if index >= base.glyph_count() || (settings.latin_only && !(0x20..=0x24F).contains(&code)) {
            continue;
        }
        let Some(character) = char::from_u32(u32::from(code)).filter(|c| !c.is_control()) else { continue };
        let Some((alpha, left, right, cut)) = rasterize(&scaled, character, cell, i32::from(base.baseline)) else {
            report.missing += 1;
            continue;
        };
        let advance = scaled.h_advance(scaled.glyph_id(character)).round().clamp(0.0, 255.0) as u8;
        let pixels: Vec<(u8, u8)> = if base.has_luminance() {
            if settings.outline { outlined(&alpha, cell) } else { alpha.iter().map(|a| (0, *a)).collect() }
        } else {
            alpha.iter().map(|a| (0, *a)).collect()
        };
        font.set_glyph(index, &pixels);
        font.set_width(index, Width { left: left.clamp(-128, 127) as i8, glyph: right.min(255) as u8, advance });
        report.drawn += 1;
        if cut {
            report.cut += 1;
        }
    }
    Ok((font, report))
}

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    /// With the dump and a font of the computer: the letters of the game font are drawn from the computer font, and the font stays valid.
    #[test]
    fn a_computer_font_makes_the_glyphs() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let Some(computer) = system_fonts().into_iter().find(|f| f.name.eq_ignore_ascii_case("arial")) else { return };
        let garc = Garc::open(&Dump::open(&root).unwrap().archive(crate::ARCHIVE)).unwrap();
        for id in [0usize, 2] {
            let base = Font::parse(garc.first_file(id).unwrap().to_vec()).unwrap();
            let settings = Settings { size: f32::from(base.height) * 0.8, latin_only: true, outline: true };
            let (font, report) = generate(&base, &std::fs::read(&computer.path).unwrap(), &settings).unwrap();
            assert!(report.drawn > 80, "{report:?}");
            let reread = Font::parse(font.bytes().to_vec()).expect("the new font is valid");
            assert_eq!(reread.codes(), base.codes(), "the map of characters is not changed");
            assert_eq!(reread.bytes().len(), base.bytes().len());
            let index = reread.index_of('A' as u16).unwrap() as usize;
            assert!(reread.glyph_ink(index).iter().filter(|v| **v > 0).count() > 20);
            assert_ne!(reread.glyph_pixels(index), base.glyph_pixels(index));
            assert!(reread.width_of(index as u16).advance > 0);
            if let Ok(folder) = std::env::var("SANGO_FONT_OUT") {
                std::fs::write(format!("{folder}/generated_{id}.png"), crate::sheet::write_png(&crate::sheet::draw(&reread)).unwrap()).unwrap();
            }
        }
    }
}
