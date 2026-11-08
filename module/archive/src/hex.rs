//! A view of bytes in hexadecimal: the offset, sixteen bytes, and the same bytes as text.

use sango_ui::egui::{self, RichText, TextStyle};
use sango_ui::theme::color;

/// The number of bytes on one line.
const WIDTH: usize = 16;

/// The text of one line: the offset, the bytes in two groups of eight, and the characters.
pub fn line(bytes: &[u8], row: usize) -> String {
    let start = row * WIDTH;
    let chunk = &bytes[start.min(bytes.len())..(start + WIDTH).min(bytes.len())];
    let mut text = format!("{start:08x}  ");
    for i in 0..WIDTH {
        match chunk.get(i) {
            Some(b) => text.push_str(&format!("{b:02x} ")),
            None => text.push_str("   "),
        }
        if i == 7 {
            text.push(' ');
        }
    }
    text.push_str(" ");
    for &b in chunk {
        text.push(if (0x20..0x7f).contains(&b) { b as char } else { '.' });
    }
    text
}

/// The number of lines of a list of bytes.
pub fn line_count(bytes: &[u8]) -> usize {
    bytes.len().div_ceil(WIDTH)
}

/// Draws the bytes. Only the lines that are on the screen are made, so a big file costs nothing.
pub fn show(ui: &mut egui::Ui, bytes: &[u8]) {
    let height = ui.text_style_height(&TextStyle::Monospace) + 2.0;
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, height, line_count(bytes), |ui, rows| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for row in rows {
            ui.label(RichText::new(line(bytes, row)).monospace().color(color::TEXT));
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_full_line_has_the_offset_the_bytes_and_the_text() {
        let bytes: Vec<u8> = (0x41..0x51).collect();
        let text = line(&bytes, 0);
        assert!(text.starts_with("00000000  41 42 43 44 45 46 47 48  49 4a"));
        assert!(text.ends_with("ABCDEFGHIJKLMNOP"));
    }

    #[test]
    fn a_short_last_line_is_padded_and_the_unprintable_bytes_are_dots() {
        let bytes = [0x00, 0x41, 0xff];
        let text = line(&bytes, 0);
        assert!(text.starts_with("00000000  00 41 ff "));
        assert!(text.ends_with(".A."));
        assert_eq!(line_count(&bytes), 1);
        assert_eq!(line_count(&[0u8; 17]), 2);
        assert_eq!(line_count(&[]), 0);
    }

    #[test]
    fn the_offset_follows_the_row() {
        let bytes = [0u8; 40];
        assert!(line(&bytes, 2).starts_with("00000020  "));
    }
}
