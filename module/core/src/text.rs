//! The texts of the game: the files of the eight languages, and a plain-text form of their lines.
//!
//! The game has one archive of texts for each language (see [`LANGUAGES`]). All the archives have the same
//! files, and a file has the same lines in every language: the line 1 of the file of the species names is
//! Bulbasaur in all of them.
//!
//! **A file** is laid out like this, with little-endian numbers:
//!
//! | Offset | Size | Content |
//! | --- | --- | --- |
//! | 0 | 2 | The number of sections: always 1. |
//! | 2 | 2 | The number of lines. |
//! | 4 | 4 | The length of the section, as in the next field of the section. |
//! | 8 | 4 | Always 0. |
//! | 12 | 4 | The place of the section: always 16. |
//! | 16 | 4 | The length of the section. |
//! | 20 | 8 × lines | For each line: its place in the section (u32), its number of characters (u16), a flag (u16). |
//! | after | | The text of the lines, one after the other. A line takes a multiple of 4 bytes: a line with an odd number of characters has 2 bytes of 0 after it. |
//!
//! **A line** is a list of 16-bit characters that ends with a 0 (a few lines have more 0 after it). It is hidden: every character is combined
//! (xor) with a key. The key of the first character of the line number `n` (from 0) is `0x7C89 + 0x2983 * n`,
//! and the key turns 3 bits to the left after each character.
//!
//! **The plain form of a line** (see [`chars_to_text`]) is the text itself, with a few marks in square brackets
//! for what is not a letter:
//!
//! - `[tag CODE P1 P2]`: a tag. The game replaces it by a name, a number or a colour. `CODE` and the parameters are
//!   in hexadecimal. The tag `[tag 0102 0000]` is the name of a Pokémon, for example.
//! - `[char XXXX]`: a character that has no usual form, in hexadecimal.
//! - `[[`: the character `[` itself.
//!
//! A line break is the line break of the text.

use std::fmt::Write as _;

/// A language of the game.
#[derive(Clone, Copy, Debug)]
pub struct Language {
    /// The name, as in the games.
    pub name: &'static str,
    /// The name of its folder on the SD card.
    pub folder: &'static str,
    /// The number of its archive of texts.
    pub archive: usize,
}

/// The eight languages, in the order of their archives.
pub const LANGUAGES: [Language; 8] = [
    Language { name: "Japanese", folder: "ja", archive: 71 },
    Language { name: "Japanese (kanji)", folder: "ja_kanji", archive: 72 },
    Language { name: "English", folder: "en", archive: 73 },
    Language { name: "French", folder: "fr", archive: 74 },
    Language { name: "Italian", folder: "it", archive: 75 },
    Language { name: "German", folder: "de", archive: 76 },
    Language { name: "Spanish", folder: "es", archive: 77 },
    Language { name: "Korean", folder: "ko", archive: 78 },
];

/// The place on the SD card of a file of texts, for a language (0 to 7) and the number of the file in the
/// archive. The file has the layout of the romfs.
pub fn sd_path(language: usize, file: usize) -> String {
    format!("text/{}/{file}.bin", LANGUAGES[language].folder)
}

/// The line that starts a tag.
const TAG: u16 = 0x10;

/// A file of texts: its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextFile {
    /// The characters of each line, with the 0 that ends it.
    lines: Vec<Vec<u16>>,
    /// The flag of each line, kept as it is.
    flags: Vec<u16>,
}

/// The key of the first character of a line.
fn line_key(line: usize) -> u16 {
    0x7C89u16.wrapping_add(0x2983u16.wrapping_mul(line as u16))
}

impl TextFile {
    /// Reads the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<TextFile, String> {
        let short = || "The file of texts is too short.".to_string();
        let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(short);
        let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(short);
        if u16_at(0)? != 1 || u32_at(8)? != 0 || u32_at(12)? != 16 {
            return Err("The file of texts does not have the usual header.".into());
        }
        let count = u16_at(2)? as usize;
        let section_length = u32_at(16)? as usize;
        if section_length + 16 != bytes.len() {
            return Err(format!("The file has {} bytes but its section ends at {}.", bytes.len(), section_length + 16));
        }
        let mut lines = Vec::with_capacity(count);
        let mut flags = Vec::with_capacity(count);
        for line in 0..count {
            let entry = 20 + 8 * line;
            let (start, length, flag) = (u32_at(entry)? as usize, u16_at(entry + 4)? as usize, u16_at(entry + 6)?);
            let data = bytes.get(16 + start..16 + start + 2 * length).ok_or_else(short)?;
            let mut key = line_key(line);
            let chars: Vec<u16> = data
                .chunks_exact(2)
                .map(|pair| {
                    let character = u16::from_le_bytes([pair[0], pair[1]]) ^ key;
                    key = key.rotate_left(3);
                    character
                })
                .collect();
            if !ends_correctly(&chars) {
                return Err(format!("The line {line} does not end with a 0 outside its tags."));
            }
            lines.push(chars);
            flags.push(flag);
        }
        let file = TextFile { lines, flags };
        if file.to_bytes() != bytes {
            return Err("The file of texts has a layout that the studio would not write back the same.".into());
        }
        Ok(file)
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let count = self.lines.len();
        let mut table = Vec::with_capacity(8 * count);
        let mut body = Vec::new();
        for (line, chars) in self.lines.iter().enumerate() {
            let place = 4 + 8 * count + body.len();
            table.extend((place as u32).to_le_bytes());
            table.extend((chars.len() as u16).to_le_bytes());
            table.extend(self.flags[line].to_le_bytes());
            let mut key = line_key(line);
            for &character in chars {
                body.extend((character ^ key).to_le_bytes());
                key = key.rotate_left(3);
            }
            if body.len() % 4 != 0 {
                body.extend([0, 0]);
            }
        }
        let section_length = (4 + table.len() + body.len()) as u32;
        let mut bytes = Vec::with_capacity(16 + section_length as usize);
        bytes.extend(1u16.to_le_bytes());
        bytes.extend((count as u16).to_le_bytes());
        bytes.extend(section_length.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(section_length.to_le_bytes());
        bytes.extend(table);
        bytes.extend(body);
        bytes
    }

    /// The number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// True when the file has no line.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The plain form of a line (see [`chars_to_text`]).
    pub fn text(&self, line: usize) -> Option<String> {
        self.lines.get(line).map(|chars| chars_to_text(&chars[..text_end(chars).unwrap_or(chars.len())]))
    }

    /// Gives a line a new text, in the plain form. The line must exist.
    pub fn set_text(&mut self, line: usize, text: &str) -> Result<(), String> {
        let mut chars = text_to_chars(text)?;
        chars.push(0);
        *self.lines.get_mut(line).ok_or_else(|| format!("The file has no line {line}."))? = chars;
        Ok(())
    }
}

/// The end of the tag that starts at the place "at", if the characters have a whole tag there: a tag is the
/// character 0x10, the number of values that follow, then the values (the first one is the code).
fn tag_end(chars: &[u16], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&TAG) {
        return None;
    }
    chars.get(at + 1).map(|&n| at + 2 + n as usize).filter(|&end| end <= chars.len() && end > at + 2)
}

/// The place of the first 0 outside the tags, which ends the text of a line. A tag can have the value 0.
fn text_end(chars: &[u16]) -> Option<usize> {
    let mut at = 0;
    while at < chars.len() {
        if let Some(end) = tag_end(chars, at) {
            at = end;
        } else if chars[at] == 0 {
            return Some(at);
        } else {
            at += 1;
        }
    }
    None
}

/// True when a line ends with a 0 outside the tags, and only 0 follow it (some lines have a few more).
fn ends_correctly(chars: &[u16]) -> bool {
    text_end(chars).is_some_and(|end| chars[end..].iter().all(|&c| c == 0))
}

/// The plain form of the characters of a line, without the 0 that ends it.
pub fn chars_to_text(chars: &[u16]) -> String {
    let mut text = String::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if let Some(end) = tag_end(chars, at) {
            text.push_str("[tag");
            for &value in &chars[at + 2..end] {
                let _ = write!(text, " {value:04X}");
            }
            text.push(']');
            at = end;
            continue;
        }
        match char::from_u32(c as u32) {
            Some('\n') => text.push('\n'),
            Some('[') => text.push_str("[["),
            Some(letter) if c >= 0x20 && !(0xD800..=0xF8FF).contains(&c) && !letter.is_control() => text.push(letter),
            _ => {
                let _ = write!(text, "[char {c:04X}]");
            }
        }
        at += 1;
    }
    text
}

/// The characters of a line from its plain form, without the 0 that ends it.
pub fn text_to_chars(text: &str) -> Result<Vec<u16>, String> {
    let mut chars = Vec::new();
    let mut rest = text.chars().peekable();
    while let Some(letter) = rest.next() {
        if letter != '[' {
            let mut buffer = [0u16; 2];
            let units = letter.encode_utf16(&mut buffer);
            if units.len() != 1 {
                return Err(format!("The character `{letter}` is not in the range of the game."));
            }
            chars.push(units[0]);
            continue;
        }
        if rest.peek() == Some(&'[') {
            rest.next();
            chars.push('[' as u16);
            continue;
        }
        let mut mark = String::new();
        loop {
            match rest.next() {
                Some(']') => break,
                Some(inside) => mark.push(inside),
                None => return Err("A `[` has no `]`. Write `[[` for the character `[`.".into()),
            }
        }
        let mut words = mark.split_whitespace();
        let kind = words.next().unwrap_or("");
        let numbers: Result<Vec<u16>, String> = words.map(|word| u16::from_str_radix(word, 16).map_err(|_| format!("`{word}` is not a hexadecimal number in `[{mark}]`."))).collect();
        let numbers = numbers?;
        match kind {
            "tag" if !numbers.is_empty() => {
                chars.push(TAG);
                chars.push(numbers.len() as u16);
                chars.extend(numbers);
            }
            "char" if numbers.len() == 1 => chars.push(numbers[0]),
            _ => return Err(format!("`[{mark}]` is not a tag (`[tag CODE ...]`) or a character (`[char XXXX]`).")),
        }
    }
    Ok(chars)
}

#[cfg(test)]
mod test {
    use super::*;

    /// A file of 2 lines made by hand: "Hi" and "A".
    fn sample() -> Vec<u8> {
        TextFile { lines: vec![vec![b'H' as u16, b'i' as u16, 0], vec![b'A' as u16, 0]], flags: vec![0, 4] }.to_bytes()
    }

    #[test]
    fn a_file_is_written_and_read_back_the_same() {
        let bytes = sample();
        let file = TextFile::from_bytes(&bytes).unwrap();
        assert_eq!(file.len(), 2);
        assert_eq!(file.text(0).as_deref(), Some("Hi"));
        assert_eq!(file.text(1).as_deref(), Some("A"));
        assert_eq!(file.to_bytes(), bytes);
        // The first line has 3 characters: 6 bytes and 2 bytes of padding.
        assert_eq!(bytes.len(), 16 + 4 + 16 + 8 + 4);
        assert_eq!(bytes[0..4], [1, 0, 2, 0]);
    }

    #[test]
    fn a_wrong_file_is_refused() {
        assert!(TextFile::from_bytes(&[]).is_err());
        assert!(TextFile::from_bytes(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0]).is_err());
        let mut bytes = sample();
        bytes.push(0);
        assert!(TextFile::from_bytes(&bytes).is_err(), "the section does not end at the end of the file");
    }

    #[test]
    fn a_line_is_changed_in_its_plain_form() {
        let mut file = TextFile::from_bytes(&sample()).unwrap();
        file.set_text(0, "Bulbasaur\nSeed [tag 0102 0000] [[ [char E08E]").unwrap();
        assert_eq!(file.text(0).as_deref(), Some("Bulbasaur\nSeed [tag 0102 0000] [[ [char E08E]"));
        let again = TextFile::from_bytes(&file.to_bytes()).unwrap();
        assert_eq!(again, file);
        assert!(file.set_text(9, "x").is_err());
        assert!(file.set_text(0, "[tag").is_err());
        assert!(file.set_text(0, "[tag zz]").is_err());
        assert!(file.set_text(0, "[colour 1]").is_err());
        assert!(file.set_text(0, "[tag]").is_err());
        assert!(file.set_text(0, "😀").is_err());
    }

    #[test]
    fn tags_and_special_characters_have_a_plain_form() {
        let chars = [b'A' as u16, TAG, 2, 0xBD00, 5, b'!' as u16, 0x0007, 0xE08E, b'[' as u16, 0xAC00];
        let text = chars_to_text(&chars);
        assert_eq!(text, "A[tag BD00 0005]![char 0007][char E08E][[\u{AC00}");
        assert_eq!(text_to_chars(&text).unwrap(), chars);
        // A tag that is cut stays a character, and is written back the same.
        let cut = [TAG, 9, 1];
        assert_eq!(text_to_chars(&chars_to_text(&cut)).unwrap(), cut);
    }

    /// With the dump of the game: every file of the eight languages is written back the same. The files of the
    /// species names, categories and Pokédex entries have the same lines in all the languages, and the names are
    /// the ones of Bulbapedia.
    #[test]
    fn the_texts_of_the_dump_are_written_back_the_same() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        let archives: Vec<crate::Garc> = LANGUAGES.iter().map(|l| crate::Garc::open(&dump.archive(l.archive)).unwrap()).collect();
        for (language, garc) in archives.iter().enumerate() {
            assert_eq!(garc.file_count(), 175);
            for file in 0..garc.file_count() {
                let bytes = garc.first_file(file).unwrap();
                let parsed = TextFile::from_bytes(bytes).unwrap_or_else(|e| panic!("{} file {file}: {e}", LANGUAGES[language].name));
                assert_eq!(parsed.to_bytes(), bytes);
                // The plain form of every line gives the same characters back.
                for line in 0..parsed.len() {
                    let end = text_end(&parsed.lines[line]).unwrap();
                    let chars = text_to_chars(&parsed.text(line).unwrap()).unwrap();
                    assert_eq!(chars, parsed.lines[line][..end], "{} file {file} line {line}", LANGUAGES[language].name);
                }
            }
        }
        let line = |language: usize, file: usize, line: usize| TextFile::from_bytes(archives[language].first_file(file).unwrap()).unwrap().text(line).unwrap();
        // The file 98 has the names of the species, the file 2 the categories (Bulbapedia: Bulbasaur is the Seed Pokémon).
        assert_eq!(line(2, 98, 1), "Bulbasaur");
        assert_eq!(line(3, 98, 1), "Bulbizarre");
        assert_eq!(line(5, 98, 1), "Bisasam");
        assert_eq!(line(2, 2, 1), "Seed Pokémon");
        assert_eq!(line(3, 2, 1), "Pokémon Graine");
        assert_eq!(line(2, 98, 25), "Pikachu");
        assert_eq!(line(2, 5, 912), "Hoopa Unbound");
    }
}
