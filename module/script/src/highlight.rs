//! The colours of the assembly text and of the Pawn text.
//!
//! One scan of a line gives the parts of the line with a kind (a comment, a number, a call...), and each kind has a colour that is readable on the
//! light theme and on the dark theme.

use sango_ui::egui::{text::LayoutJob, Color32, FontId, TextFormat};

/// The language that is coloured.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Assembly,
    Pawn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Plain,
    Comment,
    /// A directive of the assembly (`.code`) or a keyword of Pawn (`if`).
    Keyword,
    /// An instruction of the assembly.
    Instruction,
    Number,
    Text,
    /// The name of a native function that is called.
    Call,
    /// A label or the target of a jump.
    Label,
    /// A parameter, a local variable or a result of the Pawn text.
    Variable,
}

fn colour(kind: Kind, dark: bool) -> Color32 {
    let (light, night) = match kind {
        Kind::Plain => return Color32::PLACEHOLDER,
        Kind::Comment => ((110, 118, 140), (125, 135, 160)),
        Kind::Keyword => ((150, 50, 190), (200, 130, 240)),
        Kind::Instruction => ((30, 90, 200), (110, 170, 255)),
        Kind::Number => ((185, 95, 0), (235, 165, 70)),
        Kind::Text => ((30, 135, 60), (120, 210, 130)),
        Kind::Call => ((0, 130, 140), (80, 205, 215)),
        Kind::Label => ((190, 50, 90), (245, 120, 150)),
        Kind::Variable => ((70, 80, 150), (170, 180, 245)),
    };
    let (r, g, b) = if dark { night } else { light };
    Color32::from_rgb(r, g, b)
}

const KEYWORDS: &[&str] = &[
    "if", "else", "while", "do", "for", "switch", "case", "default", "return", "goto", "break", "continue", "new", "static", "const", "stock", "public",
    "native", "forward", "true", "false", "sizeof", "assert", "exit", "sleep",
];

fn is_word_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '.'
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// The parts of one line, with their kind.
fn parts(line: &str, mode: Mode) -> Vec<(Kind, &str)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut i = 0;
    // How many words came before on this line, and the first one.
    let mut words = 0;
    let mut first = "";
    while i < chars.len() {
        let (start, c) = chars[i];
        let rest = &line[start..];
        if c.is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_whitespace() {
                j += 1;
            }
            let end = chars.get(j).map_or(line.len(), |x| x.0);
            out.push((Kind::Plain, &line[start..end]));
            i = j;
        } else if (mode == Mode::Assembly && c == ';') || (mode == Mode::Pawn && rest.starts_with("//")) {
            out.push((Kind::Comment, rest));
            break;
        } else if c == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != '"' {
                j += 1;
            }
            let end = chars.get(j + 1).map_or(line.len(), |x| x.0);
            out.push((Kind::Text, &line[start..end]));
            i = j + 1;
        } else if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|x| x.1.is_ascii_digit())) {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '.') {
                j += 1;
            }
            let end = chars.get(j).map_or(line.len(), |x| x.0);
            out.push((Kind::Number, &line[start..end]));
            i = j;
        } else if is_word_start(c) {
            let mut j = i + 1;
            while j < chars.len() && is_word(chars[j].1) {
                j += 1;
            }
            let end = chars.get(j).map_or(line.len(), |x| x.0);
            let word = &line[start..end];
            let after = line[end..].trim_start();
            let kind = match mode {
                Mode::Assembly => {
                    if words == 0 && after.starts_with(':') {
                        Kind::Label
                    } else if words == 0 && word.starts_with('.') {
                        Kind::Keyword
                    } else if words == 0 {
                        Kind::Instruction
                    } else if words == 1 && (first == ".native" || first.starts_with("sysreq")) {
                        Kind::Call
                    } else {
                        Kind::Label
                    }
                }
                Mode::Pawn => {
                    let numbered = |prefix: &str| word.strip_prefix(prefix).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
                    if KEYWORDS.contains(&word) {
                        Kind::Keyword
                    } else if after.starts_with('(') {
                        Kind::Call
                    } else if numbered("arg") || numbered("local") || numbered("data") || numbered("result") {
                        Kind::Variable
                    } else if numbered("L") || numbered("public_") {
                        Kind::Label
                    } else {
                        Kind::Plain
                    }
                }
            };
            if words == 0 {
                first = word;
            }
            words += 1;
            out.push((kind, word));
            i = j;
        } else {
            let end = chars.get(i + 1).map_or(line.len(), |x| x.0);
            out.push((Kind::Plain, &line[start..end]));
            i = i + 1;
        }
    }
    out
}

/// The coloured text. `plain` is the colour of the parts that have no kind.
pub fn job(text: &str, mode: Mode, dark: bool, plain: Color32, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    for line in text.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        for (kind, part) in parts(body, mode) {
            let color = if kind == Kind::Plain { plain } else { colour(kind, dark) };
            job.append(part, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
        }
        if !newline.is_empty() {
            job.append(newline, 0.0, TextFormat { font_id: font.clone(), color: plain, ..Default::default() });
        }
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_of_a_line_make_the_line_again() {
        for (line, mode) in [("    sysreq.c SEPlay ; play", Mode::Assembly), ("L0042:", Mode::Assembly), ("  if (arg1 == 0x10) { // test", Mode::Pawn), ("é \"a\" -5", Mode::Pawn)] {
            let joined: String = parts(line, mode).iter().map(|p| p.1).collect();
            assert_eq!(joined, line);
        }
        let kinds: Vec<Kind> = parts("    sysreq.c SEPlay", Mode::Assembly).iter().map(|p| p.0).collect();
        assert_eq!(kinds, [Kind::Plain, Kind::Instruction, Kind::Plain, Kind::Call]);
    }
}
