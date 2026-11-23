//! The natures: what each nature does to the five statistics, as a change in percent.
//!
//! The game raises one statistic by 10 % and lowers another one by 10 % (`value * 110 / 100` and `value * 90 / 100`). Sango Studio lets
//! each nature change each statistic by any percent (10, -10, 50, -200...), and lets the user add natures. The new value is
//! `value * (100 + change) / 100`; a result under 1 is 1. The table goes on the SD card as `nature/natures.csv`
//! (the plugin reads the numbers; the names are for the studio):
//!
//! ```text
//! number,attack,defense,special_attack,special_defense,speed,name,ja,ja_kanji,fr,it,de,es,ko
//! 0,0,0,0,0,0,Hardy,,,,,,,
//! 1,10,-10,0,0,0,Lonely,,,,,,,
//! 25,0,-200,20,20,-30,Cosmic,,,Cosmique,,,,
//! ```
//!
//! The eight names are the name in English and in the other languages. An empty name of a new nature (25 to 31) is the English name; an
//! empty name of a nature of the game is the name of the game. The game shows the names in its language.
//!
//! The numbers 0 to 24 are the natures of the game. The numbers 25 to 31 are new natures.

/// The languages of the names, in the order of the columns: English first (the column `name`), then `ja`, `ja_kanji`, `fr`, `it`, `de`, `es`, `ko`.
pub const NAME_LANGUAGES: [&str; 8] = ["English", "Japanese", "Japanese (kanji)", "French", "Italian", "German", "Spanish", "Korean"];

/// The statistics that a nature changes, in the order of the columns of the file.
pub const STATS: [&str; 5] = ["Attack", "Defense", "Sp. Atk", "Sp. Def", "Speed"];

/// The natures of the game.
pub const GAME_COUNT: usize = 25;

/// The most natures that a table can have.
pub const MAX_COUNT: usize = 32;

/// The most that a nature can change a statistic, up or down, in percent.
pub const MAX_CHANGE: i16 = 1000;

/// The names of the natures of the game, with the effect (+1 for 110 %, -1 for 90 %) on Attack, Defense, Sp. Atk, Sp. Def and Speed.
pub const GAME: [(&str, [i8; 5]); GAME_COUNT] = [
    ("Hardy", [0, 0, 0, 0, 0]),
    ("Lonely", [1, -1, 0, 0, 0]),
    ("Brave", [1, 0, 0, 0, -1]),
    ("Adamant", [1, 0, -1, 0, 0]),
    ("Naughty", [1, 0, 0, -1, 0]),
    ("Bold", [-1, 1, 0, 0, 0]),
    ("Docile", [0, 0, 0, 0, 0]),
    ("Relaxed", [0, 1, 0, 0, -1]),
    ("Impish", [0, 1, -1, 0, 0]),
    ("Lax", [0, 1, 0, -1, 0]),
    ("Timid", [-1, 0, 0, 0, 1]),
    ("Hasty", [0, -1, 0, 0, 1]),
    ("Serious", [0, 0, 0, 0, 0]),
    ("Jolly", [0, 0, -1, 0, 1]),
    ("Naive", [0, 0, 0, -1, 1]),
    ("Modest", [-1, 0, 1, 0, 0]),
    ("Mild", [0, -1, 1, 0, 0]),
    ("Quiet", [0, 0, 1, 0, -1]),
    ("Bashful", [0, 0, 0, 0, 0]),
    ("Rash", [0, 0, 1, -1, 0]),
    ("Calm", [-1, 0, 0, 1, 0]),
    ("Gentle", [0, -1, 0, 1, 0]),
    ("Sassy", [0, 0, 0, 1, -1]),
    ("Careful", [0, 0, -1, 1, 0]),
    ("Quirky", [0, 0, 0, 0, 0]),
];

/// Built-in names of the 25 vanilla natures in all eight languages.
pub const GAME_NATURE_NAMES: [&[&str; GAME_COUNT]; 8] = [
    // 0: English
    &[
        "Hardy", "Lonely", "Brave", "Adamant", "Naughty", "Bold", "Docile", "Relaxed", "Impish",
        "Lax", "Timid", "Hasty", "Serious", "Jolly", "Naive", "Modest", "Mild", "Quiet", "Bashful",
        "Rash", "Calm", "Gentle", "Sassy", "Careful", "Quirky",
    ],
    // 1: Japanese
    &[
        "がんばりや", "さみしがり", "ゆうかん", "いじっぱり", "やんちゃ", "ずぶとい", "すなお", "のんき",
        "わんぱく", "のうてんき", "おくびょう", "せっかち", "まじめ", "ようき", "むじゃき", "ひかえめ",
        "おっとり", "れいせい", "てれや", "うっかりや", "おだやか", "おとなしい", "なまいき", "しんちょう",
        "きまぐれ",
    ],
    // 2: Japanese (kanji)
    &[
        "がんばりや", "さみしがり", "ゆうかん", "いじっぱり", "やんちゃ", "ずぶとい", "すなお", "のんき",
        "わんぱく", "のうてんき", "おくびょう", "せっかち", "まじめ", "ようき", "むじゃき", "ひかえめ",
        "おっとり", "れいせい", "てれや", "うっかりや", "おだやか", "おとなしい", "なまいき", "しんちょう",
        "きまぐれ",
    ],
    // 3: French
    &[
        "Hardi", "Solo", "Brave", "Rigide", "Mauvais", "Assuré", "Docile", "Relax", "Malin",
        "Lâche", "Timide", "Pressé", "Sérieux", "Jovial", "Naïf", "Modeste", "Doux", "Discret",
        "Pudique", "Foufou", "Calme", "Gentil", "Malpoli", "Prudent", "Bizarre",
    ],
    // 4: Italian
    &[
        "Ardita", "Schiva", "Audace", "Decisa", "Birbona", "Sicura", "Docile", "Placida", "Scaltra",
        "Fiacca", "Timida", "Lesta", "Seria", "Allegra", "Ingenua", "Modesta", "Mite", "Quieta",
        "Ritrosa", "Ardente", "Calma", "Gentile", "Vivace", "Cauta", "Furba",
    ],
    // 5: German
    &[
        "Robust", "Solo", "Mutig", "Hart", "Frech", "Kühn", "Sanft", "Locker", "Pfiffig", "Lasch",
        "Scheu", "Hastig", "Ernst", "Froh", "Naiv", "Mäßig", "Mild", "Ruhig", "Zaghaft", "Hitzig",
        "Still", "Zart", "Forsch", "Sacht", "Kauzig",
    ],
    // 6: Spanish
    &[
        "fuerte", "huraña", "audaz", "firme", "pícara", "osada", "dócil", "plácida", "agitada",
        "floja", "miedosa", "activa", "seria", "alegre", "ingenua", "modesta", "afable", "mansa",
        "tímida", "alocada", "serena", "amable", "grosera", "cauta", "rara",
    ],
    // 7: Korean
    &[
        "노력", "외로움", "용감", "고집", "개구쟁이", "대담", "온순", "무사태평", "장난꾸러기", "촐랑",
        "겁쟁이", "성급", "성실", "명랑", "천진난만", "조심", "의젓", "냉정", "수줍음", "덜렁", "차분",
        "얌전", "건방", "신중", "변덕",
    ],
];

/// The display color of a nature in the Trainer Memo summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NatureColor {
    #[default]
    Blue,
    Red,
    Green,
    Default,
}

impl NatureColor {
    pub const ALL: [NatureColor; 4] = [NatureColor::Blue, NatureColor::Red, NatureColor::Green, NatureColor::Default];

    pub fn name(&self) -> &'static str {
        match self {
            NatureColor::Blue => "Blue",
            NatureColor::Red => "Red",
            NatureColor::Green => "Green",
            NatureColor::Default => "Default",
        }
    }

    pub fn csv_str(&self) -> &'static str {
        match self {
            NatureColor::Blue => "blue",
            NatureColor::Red => "red",
            NatureColor::Green => "green",
            NatureColor::Default => "none",
        }
    }

    pub fn from_str(s: &str) -> NatureColor {
        match s.trim().to_ascii_lowercase().as_str() {
            "red" | "1" | "rouge" => NatureColor::Red,
            "green" | "3" | "vert" => NatureColor::Green,
            "none" | "default" | "black" | "0" | "noir" => NatureColor::Default,
            _ => NatureColor::Blue,
        }
    }
}

/// One nature: its name and its change for each statistic, in percent (0 changes nothing, 10 raises by 10 %, -10 lowers by 10 %).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nature {
    pub name: String,
    pub change: [i16; 5],
    /// The name in the other languages, in the order of `NAME_LANGUAGES` without English.
    pub other_names: [String; 7],
    pub color: NatureColor,
}

impl Nature {
    /// The nature of the game with this number.
    pub fn game(number: usize) -> Nature {
        let (name, signs) = GAME[number];
        let mut change = [0i16; 5];
        for (c, s) in change.iter_mut().zip(signs) {
            *c = 10 * i16::from(s);
        }
        let mut other_names: [String; 7] = Default::default();
        for (k, other) in other_names.iter_mut().enumerate() {
            *other = GAME_NATURE_NAMES[k + 1][number].to_string();
        }
        Nature { name: name.to_string(), change, other_names, color: NatureColor::Blue }
    }

    /// The statistics that the nature changes, in words: `+10 % Attack, -10 % Defense`.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .change
            .iter()
            .zip(STATS)
            .filter(|(c, _)| **c != 0)
            .map(|(c, name)| format!("{}{} % {}", if *c > 0 { "+" } else { "" }, c, name))
            .collect();
        if parts.is_empty() {
            "Nothing changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// The table of the natures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NatureTable {
    pub natures: Vec<Nature>,
}

impl NatureTable {
    /// The natures of the game.
    pub fn game() -> NatureTable {
        NatureTable { natures: (0..GAME_COUNT).map(Nature::game).collect() }
    }

    /// True when the table is the table of the game: nothing to write on the SD card.
    pub fn is_game(&self) -> bool {
        self.natures.len() == GAME_COUNT
            && self.natures.iter().enumerate().all(|(i, n)| {
                n.change == Nature::game(i).change
                    && (n.name.is_empty() || n.name == GAME[i].0)
                    && n.other_names.iter().enumerate().all(|(k, o)| o.is_empty() || o == GAME_NATURE_NAMES[k + 1][i])
                    && n.color == NatureColor::Blue
            })
    }

    /// Reads the file `natures.csv`. A line that does not start with a digit is skipped. A nature that the file does not have is the nature of
    /// the game (or a new nature without effect).
    pub fn parse(text: &str) -> Result<NatureTable, String> {
        let mut table = NatureTable::game();
        for (at, line) in text.lines().enumerate() {
            let line = line.trim();
            if !line.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            if fields.len() < 6 {
                return Err(format!("Line {}: a nature needs a number and five percents.", at + 1));
            }
            let number: usize = fields[0].parse().map_err(|_| format!("Line {}: the number of the nature is wrong.", at + 1))?;
            if number >= MAX_COUNT {
                return Err(format!("Line {}: the number of a nature is at most {}.", at + 1, MAX_COUNT - 1));
            }
            let mut change = [0i16; 5];
            for (k, c) in change.iter_mut().enumerate() {
                let value: i16 = fields[k + 1].trim_start_matches('+').parse().map_err(|_| format!("Line {}: a percent is wrong.", at + 1))?;
                if !(-MAX_CHANGE..=MAX_CHANGE).contains(&value) {
                    return Err(format!("Line {}: a percent is from -{MAX_CHANGE} to {MAX_CHANGE}.", at + 1));
                }
                *c = value;
            }
            while table.natures.len() <= number {
                let n = table.natures.len();
                table.natures.push(Nature { name: format!("Nature {}", n + 1), change: [0; 5], other_names: Default::default(), color: NatureColor::Blue });
            }
            let name = fields.get(6).filter(|n| !n.is_empty()).map_or_else(
                || if number < GAME_COUNT { GAME[number].0.to_string() } else { table.natures[number].name.clone() },
                |n| (*n).to_string(),
            );
            let mut other_names: [String; 7] = Default::default();
            for (k, other) in other_names.iter_mut().enumerate() {
                let from_file = fields.get(7 + k).map(|n| (*n).to_string()).unwrap_or_default();
                *other = if !from_file.is_empty() {
                    from_file
                } else if number < GAME_COUNT {
                    GAME_NATURE_NAMES[k + 1][number].to_string()
                } else {
                    String::new()
                };
            }
            let color = fields.get(14).filter(|c| !c.is_empty()).map_or(NatureColor::Blue, |c| NatureColor::from_str(c));
            table.natures[number] = Nature { name, change, other_names, color };
        }
        Ok(table)
    }

    /// The text of the file `natures.csv`.
    pub fn to_csv(&self) -> String {
        let has_custom_color = self.natures.iter().any(|n| n.color != NatureColor::Blue);
        let mut text = String::from(if has_custom_color {
            "number,attack,defense,special_attack,special_defense,speed,name,ja,ja_kanji,fr,it,de,es,ko,color\n"
        } else {
            "number,attack,defense,special_attack,special_defense,speed,name,ja,ja_kanji,fr,it,de,es,ko\n"
        });
        for (number, nature) in self.natures.iter().enumerate() {
            let c = nature.change;
            let others: Vec<String> = nature.other_names.iter().map(|o| o.replace(',', " ")).collect();
            let name = nature.name.replace(',', " ");
            if has_custom_color {
                let color_str = if nature.color == NatureColor::Blue { String::new() } else { nature.color.csv_str().to_string() };
                text += &format!("{number},{},{},{},{},{},{},{},{}\n", c[0], c[1], c[2], c[3], c[4], name, others.join(","), color_str);
            } else {
                text += &format!("{number},{},{},{},{},{},{},{}\n", c[0], c[1], c[2], c[3], c[4], name, others.join(","));
            }
        }
        text
    }

    /// The value of a statistic after the nature, like the plugin: `value * (100 + change) / 100`, from 1 to 65535.
    pub fn apply(&self, nature: usize, stat: usize, value: u32) -> u32 {
        let change = self.natures.get(nature).map_or(0, |n| n.change[stat]);
        (i64::from(value) * (100 + i64::from(change)) / 100).clamp(1, 0xFFFF) as u32
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_game_table_has_the_game_natures() {
        let table = NatureTable::game();
        assert_eq!(table.natures.len(), 25);
        assert_eq!(table.natures[3].name, "Adamant");
        assert_eq!(table.natures[3].change, [10, 0, -10, 0, 0]);
        assert_eq!(table.natures[0].change, [0; 5]);
        assert!(table.is_game());
        // Each nature of the game raises at most one statistic and lowers at most one.
        for nature in &table.natures {
            assert!(nature.change.iter().filter(|c| **c == 10).count() <= 1);
            assert!(nature.change.iter().filter(|c| **c == -10).count() <= 1);
        }
    }

    #[test]
    fn a_table_goes_through_its_file() {
        let mut table = NatureTable::game();
        table.natures[1].change = [25, -20, 0, 0, 0];
        let mut cosmic = Nature { name: "Cosmic".into(), change: [0, -200, 20, 20, -30], other_names: Default::default(), color: NatureColor::Blue };
        cosmic.other_names[2] = "Cosmique".to_string();
        cosmic.other_names[6] = "코스믹".to_string();
        table.natures.push(cosmic);
        let again = NatureTable::parse(&table.to_csv()).unwrap();
        assert_eq!(again, table);
        assert!(!again.is_game());
        assert_eq!(again.natures[25].name, "Cosmic");
        assert_eq!(again.natures[25].other_names[2], "Cosmique");
        assert_eq!(again.natures[25].other_names[6], "코스믹");
        // A file without the name columns is still read.
        assert_eq!(NatureTable::parse("26,0,0,0,0,0,Alone").unwrap().natures[26].name, "Alone");
        // A nature of the game with a new name: the name goes through the file, and all names are written.
        let mut renamed = NatureTable::game();
        renamed.natures[3].name = "Fierce".to_string();
        renamed.natures[3].other_names[2] = "Fougueux".to_string();
        let again = NatureTable::parse(&renamed.to_csv()).unwrap();
        assert_eq!(again, renamed);
        assert_eq!(NatureTable::game().to_csv().lines().skip(1).next().and_then(|l| l.split(',').nth(6)), Some("Hardy"));
    }

    #[test]
    fn a_file_with_a_mistake_is_refused() {
        assert!(NatureTable::parse("1,110,90").is_err());
        assert!(NatureTable::parse("40,100,100,100,100,100").is_err());
        assert!(NatureTable::parse("1,2000,0,0,0,0").is_err());
        assert!(NatureTable::parse("1,-1001,0,0,0,0").is_err());
        assert_eq!(NatureTable::parse("26,+5,-200,0,0,0,X").unwrap().natures[26].change, [5, -200, 0, 0, 0]);
        assert!(NatureTable::parse("number,attack\n").unwrap().is_game());
    }

    #[test]
    fn the_nature_multiplies_like_the_game() {
        let table = NatureTable::game();
        assert_eq!(table.apply(1, 0, 100), 110);
        assert_eq!(table.apply(1, 1, 100), 90);
        assert_eq!(table.apply(1, 2, 100), 100);
        assert_eq!(table.apply(1, 0, 123), 135);
        assert_eq!(Nature::game(3).summary(), "+10 % Attack, -10 % Sp. Atk");
        // A change under -100 % gives a negative value: the statistic is 1.
        let mut strange = NatureTable::game();
        strange.natures[0].change = [0, -200, 0, 0, 0];
        assert_eq!(strange.apply(0, 1, 100), 1);
        strange.natures[0].change = [100, 0, 0, 0, 0];
        assert_eq!(strange.apply(0, 0, 100), 200);
    }
}

#[cfg(test)]
mod scan {
    #[test]
    fn the_names_of_the_natures_are_in_the_file_51_of_every_language() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = crate::Dump::open(&root).unwrap();
        for language in crate::text::LANGUAGES {
            let garc = crate::Garc::open(&dump.archive(language.archive)).unwrap();
            let file = crate::text::TextFile::from_bytes(garc.first_file(51).unwrap()).unwrap();
            let names: Vec<String> = (0..25).map(|n| file.text(n).unwrap_or_default()).collect();
            assert!(names.iter().all(|n| !n.is_empty()), "{}", language.name);
            eprintln!("{}: {} {} {}", language.name, names[0], names[1], names[24]);
        }
        // The English names are the names of the table.
        let english = crate::Garc::open(&dump.archive(crate::text::LANGUAGES[2].archive)).unwrap();
        let file = crate::text::TextFile::from_bytes(english.first_file(51).unwrap()).unwrap();
        for (number, nature) in super::NatureTable::game().natures.iter().enumerate() {
            assert_eq!(file.text(number).unwrap(), nature.name);
        }
    }
}
