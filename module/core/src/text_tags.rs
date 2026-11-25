//! The readable names of the tags of the texts, the colours that they give, and the parts of a line that each colour reaches.
//!
//! A tag of the game is the character 0x10, the number of values that follow, then the values: the code first (group × 256 +
//! index), then the parameters. The plain form of a line (see [`crate::text`]) writes the tags below with a name, and the
//! other tags as `[tag CODE P1 P2]`.
//!
//! | Plain form | Code | Parameters | What the game does |
//! | --- | --- | --- | --- |
//! | `[color red]` | BD00 | the colour (0 to 8, see [`GAME_COLORS`]) | The next characters have this colour. |
//! | `[/color]` | BD01 | | The next characters have the colour of the text box again. |
//! | `[center]` | BD02 | 0 | The line is in the middle of the text box. |
//! | `[right]` | BD03 | 0 | The line ends at the right side of the text box. |
//! | `[scroll]` | BE00 | | A dialogue: the text goes up by one line. |
//! | `[page]` | BE01 | | A dialogue: the player presses A, then the text box is cleared. |
//! | `[wait 30]` | BE02 | the time, in frames (decimal in the plain form) | A dialogue: the text stops for this time. |
//! | `[color #FF8000]` | BD10 | red, green, blue (0 to 255) | Plugin: the next characters have this colour. |
//! | `[gradient #FF0000 #0000FF]` | BD11 | red, green, blue of the top, then of the bottom | Plugin: the next characters go from the first colour at the top to the second at the bottom. |
//! | `[size 150]` | BD12 | the size, in percent of the normal size | Plugin: the next characters have this size. |
//! | `[/size]` | BD13 | | Plugin: the next characters have the normal size. |
//!
//! `[/color]` also ends `[color #RRGGBB]` and `[gradient ...]`. Without the plugin, the game does nothing for the codes BD10 to
//! BD13 (the code of the game skips an unknown tag of the group BD): the text shows in its normal colour and size.

/// The character that starts a tag.
pub const TAG: u16 = 0x10;

/// The colours of the tag `[color NAME]` (code BD00), in the order of the parameter: the name in the plain form, and the colour
/// that the game gives (red, green, blue).
pub const GAME_COLORS: [(&str, [u8; 3]); 9] = [
    ("red", [255, 0, 0]),
    ("green", [0, 192, 0]),
    ("blue", [0, 140, 255]),
    ("orange", [255, 128, 0]),
    ("pink", [255, 128, 255]),
    ("purple", [140, 0, 255]),
    ("sky", [80, 200, 228]),
    ("yellow", [255, 230, 80]),
    ("lightblue", [100, 180, 255]),
];

/// The colours of the tag of the editor of the game texts (code FF00): 0 ends the colour.
pub const EDITOR_COLORS: [[u8; 3]; 4] = [[0, 0, 0], [255, 0, 0], [0, 140, 255], [0, 192, 0]];

pub const COLOR: u16 = 0xBD00;
pub const COLOR_END: u16 = 0xBD01;
pub const CENTER: u16 = 0xBD02;
pub const RIGHT: u16 = 0xBD03;
pub const SCROLL: u16 = 0xBE00;
pub const PAGE: u16 = 0xBE01;
pub const WAIT: u16 = 0xBE02;
pub const RGB: u16 = 0xBD10;
pub const GRADIENT: u16 = 0xBD11;
pub const SIZE: u16 = 0xBD12;
pub const SIZE_END: u16 = 0xBD13;
pub const EDITOR_COLOR: u16 = 0xFF00;

/// The smallest and the largest size of the tag `[size N]`, in percent.
pub const SIZE_RANGE: (u16, u16) = (25, 400);

/// The readable name of the values of a tag (the code, then the parameters), when the tag has one.
pub fn name(values: &[u16]) -> Option<String> {
    let (&code, params) = values.split_first()?;
    let hex = |p: &[u16]| format!("#{:02X}{:02X}{:02X}", p[0], p[1], p[2]);
    let bytes = |p: &[u16]| p.iter().all(|&v| v <= 255);
    Some(match (code, params) {
        (COLOR, [color]) if (*color as usize) < GAME_COLORS.len() => format!("[color {}]", GAME_COLORS[*color as usize].0),
        (COLOR_END, []) => "[/color]".into(),
        (CENTER, [0]) => "[center]".into(),
        (RIGHT, [0]) => "[right]".into(),
        (SCROLL, []) => "[scroll]".into(),
        (PAGE, []) => "[page]".into(),
        (WAIT, [time]) => format!("[wait {time}]"),
        (RGB, p) if p.len() == 3 && bytes(p) => format!("[color {}]", hex(p)),
        (GRADIENT, p) if p.len() == 6 && bytes(p) => format!("[gradient {} {}]", hex(&p[..3]), hex(&p[3..])),
        (SIZE, [size]) => format!("[size {size}]"),
        (SIZE_END, []) => "[/size]".into(),
        _ => return None,
    })
}

/// The names of the word tags (group 0x01) by their index: what the game puts in the text.
fn word_name(index: u16) -> Option<&'static str> {
    Some(match index {
        0 => "the name of a trainer",
        1 => "the name of a Pokémon species",
        2 => "the nickname of a Pokémon",
        3 => "a type",
        6 => "an ability",
        7 => "a move",
        9 => "an item",
        12 => "the nickname of a Pokémon (even when it is hidden)",
        14 => "a trainer class",
        15 => "the name of a type",
        16 => "an O-Power",
        141 => "the name of the user of an O-Power",
        158 => "a trainer class and a name",
        _ => return None,
    })
}

/// What the word of a choice tag is: 0xFE the word that the game puts in the text now (a name with its number), 0xFF the player.
fn reference(word: u16) -> String {
    match word {
        0xFE => "this word (the name and the number that the game gives to it)".into(),
        0xFF => "the player".into(),
        place => format!("the word {place} of the message"),
    }
}

/// An explanation of a tag in plain words, from its values (the code, then the parameters).
pub fn describe(values: &[u16]) -> String {
    let Some((&code, params)) = values.split_first() else { return "An empty tag.".into() };
    let (group, index) = (code >> 8, code & 0xFF);
    let first = params.first().copied();
    match (code, group) {
        (COLOR, _) => match first.and_then(|c| GAME_COLORS.get(c as usize)) {
            Some((name, _)) => format!("The next letters have the colour {name} of the game. [/color] ends it."),
            None => "A colour of the game that does not exist.".into(),
        },
        (COLOR_END, _) => "The next letters have the colour of the text box again.".into(),
        (CENTER, _) => "The line is in the middle of the text box.".into(),
        (RIGHT, _) => "The line ends at the right side of the text box.".into(),
        (0xBD04, _) => format!("The next letter moves {} points to the right.", first.unwrap_or(0)),
        (0xBD05, _) => format!("The next letter starts at {} points from the left of the text box.", first.unwrap_or(0)),
        (0xBD06 | 0xBD07, _) => "Start of a word that the game squeezes to fit its place.".into(),
        (0xBD08, _) => "End of the squeezed word.".into(),
        (0xBDFF, _) => "A mark of the tools of the game: the text is not used. The game does nothing with it.".into(),
        (RGB, _) => "Plugin: the next letters have this colour. [/color] ends it. Without the plugin, nothing changes.".into(),
        (GRADIENT, _) => "Plugin: the next letters go from the first colour at the top to the second at the bottom. [/color] ends it.".into(),
        (SIZE, _) => format!("Plugin: the next letters have {}% of the normal size. [/size] ends it.", first.unwrap_or(100)),
        (SIZE_END, _) => "Plugin: the next letters have the normal size again.".into(),
        (SCROLL, _) => "Dialogue: the player presses A, then the text goes up by one line.".into(),
        (PAGE, _) => "Dialogue: the player presses A, then the text box is cleared.".into(),
        (WAIT, _) => {
            let frames = first.unwrap_or(0);
            format!("Dialogue: the text stops for {frames} frames ({:.2} s).", frames as f32 / 60.0)
        }
        (0xBE03, _) => format!("Dialogue: each next letter waits {} frames.", first.unwrap_or(0)),
        (0xBE04, _) => "Dialogue: the letters show at the normal speed again.".into(),
        (0xBE05, _) => format!("Dialogue: a value ({}) for the code of the scene (a sound, an animation...).", first.unwrap_or(0)),
        (EDITOR_COLOR, _) => match first {
            Some(0) => "The next letters have the colour of the text box again.".into(),
            Some(c) if (c as usize) < EDITOR_COLORS.len() => format!("The next letters have the colour {} (second colour list of the game).", ["", "red", "blue", "green"][c as usize]),
            _ => "A colour of the second list that does not exist.".into(),
        },
        (_, 0x01) => {
            let what = word_name(index).map_or_else(|| format!("a word of the kind {index}"), str::to_string);
            format!("The game puts {what} here: the word {} that the code gives to the message.", first.unwrap_or(0))
        }
        (_, 0x02) => format!("The game puts a number here: the value {} that the code gives to the message.", first.unwrap_or(0)),
        (0x1000, _) => "The next word counts as one (singular).".into(),
        (0x1001, _) => "The next word counts as many (plural).".into(),
        (0x1002, _) => "The next word counts as masculine.".into(),
        (_, 0x11) => {
            let by = match index {
                0 => "the gender",
                1 => "the number (one or many)",
                2 => "the number and the gender",
                3 => "the number and the gender (German)",
                _ => "something",
            };
            let lengths: Vec<String> = params.iter().skip(1).flat_map(|p| [p & 0xFF, p >> 8]).map(|n| n.to_string()).collect();
            format!(
                "The game chooses a text by {by} of {}. The texts follow the tag one after the other; their lengths in letters are {}.",
                reference(first.unwrap_or(0)),
                lengths.join(", ")
            )
        }
        (_, 0x12..=0x18) => format!("The game puts a small word here (an article such as \"the\" or \"a\"), in the form that fits {}.", reference(first.unwrap_or(0))),
        (_, 0x19) => "Korean: the particle that fits the last word.".into(),
        _ => format!("A tag of the group {group:02X}, index {index:02X}, that the Studio does not know."),
    }
}

/// Reads a colour `#RRGGBB`.
fn parse_hex(word: &str) -> Option<[u16; 3]> {
    let digits = word.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some([(value >> 16) as u16 & 255, (value >> 8) as u16 & 255, value as u16 & 255])
}

/// The values of a tag (the code, then the parameters) from the words of a readable name: `None` when the first word is not a
/// readable name, an error when the words after it are wrong.
pub fn parse(words: &[&str]) -> Option<Result<Vec<u16>, String>> {
    let (&kind, rest) = words.split_first()?;
    let wrong = |what: &str| Some(Err(format!("`[{}]`: {what}", words.join(" "))));
    let alone = |code: u16| if rest.is_empty() { Some(Ok(vec![code])) } else { wrong("this tag has no value.") };
    match kind {
        "color" => match rest {
            [word] => {
                if let Some(place) = GAME_COLORS.iter().position(|(name, _)| name == word) {
                    Some(Ok(vec![COLOR, place as u16]))
                } else if let Some(rgb) = parse_hex(word) {
                    Some(Ok(vec![RGB, rgb[0], rgb[1], rgb[2]]))
                } else {
                    let names: Vec<&str> = GAME_COLORS.iter().map(|(name, _)| *name).collect();
                    wrong(&format!("the colour is a name ({}) or #RRGGBB.", names.join(", ")))
                }
            }
            _ => wrong("write one colour: a name or #RRGGBB."),
        },
        "/color" => alone(COLOR_END),
        "center" => if rest.is_empty() { Some(Ok(vec![CENTER, 0])) } else { wrong("this tag has no value.") },
        "right" => if rest.is_empty() { Some(Ok(vec![RIGHT, 0])) } else { wrong("this tag has no value.") },
        "scroll" => alone(SCROLL),
        "page" => alone(PAGE),
        "/size" => alone(SIZE_END),
        "wait" => match rest {
            [word] => match word.parse::<u16>() {
                Ok(time) => Some(Ok(vec![WAIT, time])),
                Err(_) => wrong("the time is a number of frames (60 frames = 1 second)."),
            },
            _ => wrong("write the time in frames: [wait 30]."),
        },
        "gradient" => match rest {
            [top, bottom] => match (parse_hex(top), parse_hex(bottom)) {
                (Some(a), Some(b)) => Some(Ok(vec![GRADIENT, a[0], a[1], a[2], b[0], b[1], b[2]])),
                _ => wrong("the two colours are #RRGGBB."),
            },
            _ => wrong("write two colours: [gradient #RRGGBB #RRGGBB]."),
        },
        "size" => match rest {
            [word] => match word.parse::<u16>() {
                Ok(size) if (SIZE_RANGE.0..=SIZE_RANGE.1).contains(&size) => Some(Ok(vec![SIZE, size])),
                _ => wrong(&format!("the size is a percent from {} to {}.", SIZE_RANGE.0, SIZE_RANGE.1)),
            },
            _ => wrong("write the size in percent: [size 150]."),
        },
        _ => None,
    }
}

/// The look of a part of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// The colour of the text box.
    Normal,
    /// One colour (red, green, blue).
    Color([u8; 3]),
    /// A colour at the top and a colour at the bottom.
    Gradient([u8; 3], [u8; 3]),
}

/// A part of the plain form of a line: its place (in bytes of the text), whether it is a mark in square brackets, and the look
/// of the characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// True for a mark in square brackets (`[color red]`, `[char E08E]`...): the editor shows it small.
    pub mark: bool,
    pub paint: Paint,
    /// The size in percent (100 = normal).
    pub size: u16,
}

/// The parts of the plain form of a line, with the colour and the size that the tags give to each part. A mark that is wrong is a
/// mark too: the editor shows the error.
pub fn spans(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut paint = Paint::Normal;
    let mut size = 100;
    let mut start = 0;
    let mut at = 0;
    let bytes = text.as_bytes();
    while at < bytes.len() {
        if bytes[at] != b'[' {
            at += 1;
            continue;
        }
        if bytes.get(at + 1) == Some(&b'[') {
            at += 2;
            continue;
        }
        if start < at {
            spans.push(Span { start, end: at, mark: false, paint, size });
        }
        let end = text[at..].find(']').map_or(text.len(), |close| at + close + 1);
        spans.push(Span { start: at, end, mark: true, paint, size });
        let inside = &text[at + 1..end.saturating_sub(1).max(at + 1)];
        let words: Vec<&str> = inside.split_whitespace().collect();
        let values = match words.first() {
            Some(&"tag") => words[1..].iter().map(|w| u16::from_str_radix(w, 16).ok()).collect::<Option<Vec<u16>>>(),
            _ => parse(&words).and_then(|r| r.ok()),
        };
        if let Some(values) = values {
            match (values.first().copied(), &values[1..]) {
                (Some(COLOR), [c]) => paint = GAME_COLORS.get(*c as usize).map_or(paint, |(_, rgb)| Paint::Color(*rgb)),
                (Some(COLOR_END), _) => paint = Paint::Normal,
                (Some(RGB), p) if p.len() == 3 => paint = Paint::Color([p[0] as u8, p[1] as u8, p[2] as u8]),
                (Some(GRADIENT), p) if p.len() == 6 => paint = Paint::Gradient([p[0] as u8, p[1] as u8, p[2] as u8], [p[3] as u8, p[4] as u8, p[5] as u8]),
                (Some(EDITOR_COLOR), [0]) => paint = Paint::Normal,
                (Some(EDITOR_COLOR), [c]) => paint = EDITOR_COLORS.get(*c as usize).map_or(paint, |rgb| Paint::Color(*rgb)),
                (Some(SIZE), [s]) => size = *s,
                (Some(SIZE_END), _) => size = 100,
                _ => {}
            }
        }
        at = end;
        start = end;
    }
    if start < text.len() {
        spans.push(Span { start, end: text.len(), mark: false, paint, size });
    }
    spans
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_readable_names_go_both_ways() {
        for values in [vec![COLOR, 6], vec![COLOR_END], vec![CENTER, 0], vec![RIGHT, 0], vec![SCROLL], vec![PAGE], vec![WAIT, 30], vec![RGB, 255, 128, 0], vec![GRADIENT, 255, 0, 0, 0, 0, 255], vec![SIZE, 150], vec![SIZE_END]] {
            let text = name(&values).unwrap();
            let inside = &text[1..text.len() - 1];
            let words: Vec<&str> = inside.split_whitespace().collect();
            assert_eq!(parse(&words).unwrap().unwrap(), values, "{text}");
        }
        // A value that the name cannot say keeps the form [tag ...].
        assert!(name(&[COLOR, 9]).is_none());
        assert!(name(&[CENTER, 1]).is_none());
        assert!(name(&[RGB, 256, 0, 0]).is_none());
        assert!(parse(&["color", "black"]).unwrap().is_err());
        assert!(parse(&["size", "1000"]).unwrap().is_err());
        assert!(parse(&["name"]).is_none());
    }

    #[test]
    fn the_spans_follow_the_colours_and_the_sizes() {
        let text = "A [color sky]B[/color] [size 200]C[/size][tag BD00 0000]D";
        let parts: Vec<(&str, bool, Paint, u16)> = spans(text).iter().map(|s| (&text[s.start..s.end], s.mark, s.paint, s.size)).collect();
        let sky = Paint::Color([80, 200, 228]);
        assert_eq!(
            parts,
            vec![
                ("A ", false, Paint::Normal, 100),
                ("[color sky]", true, Paint::Normal, 100),
                ("B", false, sky, 100),
                ("[/color]", true, sky, 100),
                (" ", false, Paint::Normal, 100),
                ("[size 200]", true, Paint::Normal, 100),
                ("C", false, Paint::Normal, 200),
                ("[/size]", true, Paint::Normal, 200),
                ("[tag BD00 0000]", true, Paint::Normal, 100),
                ("D", false, Paint::Color([255, 0, 0]), 100),
            ]
        );
        // `[[` is the character `[`, not a mark.
        assert_eq!(spans("a[[b").len(), 1);
    }
}
