//! The names that the headers of the plugin declare: classes, structures, enumerations and their values, functions, constants,
//! types and macros. The code editor completes the word that the user types with them.
//!
//! The reader is simple: it goes through each header line by line, follows the braces (a namespace, a class, an
//! enumeration, or the body of a function that it skips), and keeps the declarations at the level of a namespace or of a
//! class. It does not understand all of C++, but it finds the names that the headers of the plugin use.

use std::path::{Path, PathBuf};

/// What a name is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Class,
    Enum,
    EnumValue,
    Function,
    Constant,
    Type,
    Macro,
    Namespace,
}

impl Kind {
    /// A short word for the list.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Enum => "enum",
            Kind::EnumValue => "value",
            Kind::Function => "function",
            Kind::Constant => "constant",
            Kind::Type => "type",
            Kind::Macro => "macro",
            Kind::Namespace => "namespace",
        }
    }
}

/// A name of a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// The name alone: `GetInstance`.
    pub name: String,
    /// The name with its namespaces and classes: `pokemon::MoveData::GetInstance`.
    pub qualified: String,
    pub kind: Kind,
    /// The declaration, on one line, or the enumeration of a value.
    pub detail: String,
    /// The header, as it is written in an include: `pokemon/native/move_data.h`.
    pub header: String,
    /// The comment before the declaration.
    pub doc: String,
}

/// All the names of a folder of headers.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub symbols: Vec<Symbol>,
}

/// What a `{` opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    Namespace,
    Class,
    Enum { scoped: bool },
    Body,
}

/// The words that start a statement and are not the name of a function.
const NOT_FUNCTIONS: [&str; 16] = ["if", "for", "while", "switch", "return", "sizeof", "static_assert", "alignof", "defined", "catch", "else", "do", "decltype", "case", "throw", "new"];

impl Index {
    /// Reads all the headers (`.h`) of a folder and of the folders inside it.
    pub fn build(include_dir: &Path) -> Index {
        let mut files = Vec::new();
        collect(include_dir, &mut files);
        files.sort();
        let mut symbols = Vec::new();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            let header = file.strip_prefix(include_dir).unwrap_or(&file).to_string_lossy().replace('\\', "/");
            read_header(&text, &header, &mut symbols);
        }
        Index { symbols }
    }

    /// The names that go with what the user typed. A word with `::` matches the start of the qualified name; a word without
    /// matches the start of a name first, then the inside of a name. At most `limit` names.
    pub fn complete(&self, typed: &str, limit: usize) -> Vec<&Symbol> {
        if typed.is_empty() {
            return Vec::new();
        }
        let wanted = typed.to_lowercase();
        let mut starts: Vec<&Symbol> = Vec::new();
        let mut inside: Vec<&Symbol> = Vec::new();
        for symbol in &self.symbols {
            let name = symbol.name.to_lowercase();
            if typed.contains("::") {
                let qualified = symbol.qualified.to_lowercase();
                // `ItemId::` lists the values of `ItemId`, and `pokemon::MoveData::Get` the names that start so.
                if qualified.starts_with(&wanted) || qualified.ends_with(&wanted) || qualified.contains(&format!("::{wanted}")) {
                    starts.push(symbol);
                }
            } else if name.starts_with(&wanted) {
                starts.push(symbol);
            } else if wanted.len() >= 3 && name.contains(&wanted) {
                inside.push(symbol);
            }
        }
        let rank = |s: &&Symbol| (s.name.len(), s.kind, s.qualified.clone());
        starts.sort_by_key(rank);
        inside.sort_by_key(rank);
        starts.dedup_by(|a, b| a.qualified == b.qualified && a.kind == b.kind);
        starts.into_iter().chain(inside).take(limit).collect()
    }

    /// The names of the types, for the colours of the editor.
    pub fn type_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.symbols.iter().filter(|s| matches!(s.kind, Kind::Class | Kind::Enum | Kind::Type)).map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

fn collect(folder: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|e| e == "h" || e == "hpp") {
            files.push(path);
        }
    }
}

/// The word that follows a keyword: `struct Name {` gives `Name`.
fn word_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let at = line.find(keyword)?;
    let rest = line[at + keyword.len()..].trim_start();
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// True when the line starts with the word (and the word ends there).
fn starts_with_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word).is_some_and(|rest| rest.chars().next().map_or(true, |c| !(c.is_alphanumeric() || c == '_')))
}

fn read_header(text: &str, header: &str, out: &mut Vec<Symbol>) {
    let mut stack: Vec<(Block, String)> = Vec::new();
    let mut doc = String::new();
    let mut in_comment = false;
    let mut skip_depth = 0usize;
    let mut enum_name = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        // The comments: `///` lines and `/** ... */` blocks make the doc of the next declaration.
        if in_comment {
            if let Some(end) = line.find("*/") {
                in_comment = false;
                push_doc(&mut doc, &line[..end]);
            } else {
                push_doc(&mut doc, line);
            }
            continue;
        }
        if line.starts_with("/**") || line.starts_with("/*!") {
            doc.clear();
            if let Some(end) = line.find("*/") {
                push_doc(&mut doc, &line[3..end]);
            } else {
                in_comment = true;
                push_doc(&mut doc, &line[3..]);
            }
            continue;
        }
        if line.starts_with("/*") {
            in_comment = !line.contains("*/");
            continue;
        }
        if let Some(comment) = line.strip_prefix("///") {
            push_doc(&mut doc, comment.trim_start_matches(['/', '<', ' ']));
            continue;
        }
        if line.starts_with("//") || line.is_empty() {
            if line.is_empty() {
                doc.clear();
            }
            continue;
        }
        let code = strip_trailing_comment(line);
        let after_comment = line[code.len()..].trim().trim_start_matches(['/', '<', '!', '*', ' ']).to_string();
        let opens = code.matches('{').count();
        let closes = code.matches('}').count();

        // The inside of the body of a function: only the braces count.
        if skip_depth > 0 {
            skip_depth = (skip_depth + opens).saturating_sub(closes);
            if skip_depth == 0 {
                stack.pop();
            }
            continue;
        }
        let top = stack.last().map(|(block, _)| *block);
        let scope = qualifier(&stack);

        if let Some(value) = enum_value(code, top) {
            let scoped = matches!(top, Some(Block::Enum { scoped: true }));
            let qualified = if scoped { format!("{scope}::{value}") } else { format!("{}::{value}", outer_scope(&stack)) };
            let qualified = qualified.trim_start_matches("::").to_string();
            let detail = format!("{enum_name}::{value}");
            out.push(Symbol { name: value.to_string(), qualified, kind: Kind::EnumValue, detail, header: header.to_string(), doc: if after_comment.is_empty() { doc.clone() } else { after_comment.clone() } });
            doc.clear();
        } else if matches!(top, None | Some(Block::Namespace) | Some(Block::Class)) {
            if let Some((kind, name, detail)) = declaration(code) {
                let qualified = if scope.is_empty() { name.clone() } else { format!("{scope}::{name}") };
                out.push(Symbol { name: name.clone(), qualified, kind, detail, header: header.to_string(), doc: doc.clone() });
                if kind == Kind::Enum {
                    enum_name = name;
                }
            }
        }
        if !code.starts_with('#') {
            doc.clear();
        }

        // The braces: what the `{` opens.
        if opens > closes {
            let block = if starts_with_word(code, "namespace") || code.starts_with("extern \"C\"") {
                (Block::Namespace, word_after(code, "namespace").unwrap_or("").to_string())
            } else if code.contains("enum ") || starts_with_word(code, "enum") {
                (Block::Enum { scoped: code.contains("enum class") || code.contains("enum struct") }, word_after(code.trim_start_matches("typedef").trim_start(), "enum").map(|w| if w == "class" || w == "struct" { word_after(code, w).unwrap_or("") } else { w }).unwrap_or("").to_string())
            } else if starts_with_word(code, "class") || starts_with_word(code, "struct") || starts_with_word(code, "union") || code.contains(" class ") || code.contains(" struct ") || code.starts_with("template") && (code.contains("class ") || code.contains("struct ")) {
                let name = word_after(code, "class").or_else(|| word_after(code, "struct")).or_else(|| word_after(code, "union")).unwrap_or("");
                (Block::Class, name.to_string())
            } else {
                (Block::Body, String::new())
            };
            for _ in 0..(opens - closes) {
                if block.0 == Block::Body && top != Some(Block::Body) {
                    // The body of a function: skip it.
                    stack.push(block.clone());
                    skip_depth = opens - closes;
                    break;
                }
                stack.push(block.clone());
            }
        } else if closes > opens {
            for _ in 0..(closes - opens) {
                stack.pop();
            }
        }
    }
}

/// The text of the line without a comment at its end.
fn strip_trailing_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) if !line[..at].contains('"') => line[..at].trim_end(),
        _ => line,
    }
}

fn push_doc(doc: &mut String, text: &str) {
    let text = text.trim().trim_start_matches('*').trim();
    if text.is_empty() || text.starts_with('@') && !text.starts_with("@brief") {
        return;
    }
    let text = text.strip_prefix("@brief").unwrap_or(text).trim();
    if !doc.is_empty() {
        doc.push(' ');
    }
    doc.push_str(text);
}

/// The namespaces and the classes that surround the line, as `a::b`.
fn qualifier(stack: &[(Block, String)]) -> String {
    stack.iter().filter(|(block, name)| matches!(block, Block::Namespace | Block::Class | Block::Enum { scoped: true }) && !name.is_empty()).map(|(_, name)| name.as_str()).collect::<Vec<_>>().join("::")
}

/// The namespaces and the classes around an enumeration (without its own name).
fn outer_scope(stack: &[(Block, String)]) -> String {
    let outer = &stack[..stack.len().saturating_sub(1)];
    qualifier(outer)
}

/// The name of an enumerator in a line of the body of an enumeration, if the line has one.
fn enum_value(code: &str, top: Option<Block>) -> Option<&str> {
    if !matches!(top, Some(Block::Enum { .. })) {
        return None;
    }
    let first = code.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
    let end = first.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(first.len());
    let word = &first[..end];
    (!word.is_empty() && !word.chars().next().is_some_and(|c| c.is_ascii_digit())).then_some(word)
}

/// A declaration in a line at the level of a namespace or of a class: its kind, its name and its text.
fn declaration(code: &str) -> Option<(Kind, String, String)> {
    let line = code.trim_end_matches(['{', ' ']).trim();
    if line.is_empty() {
        return None;
    }
    if let Some(rest) = line.strip_prefix("#define") {
        let name = rest.trim().split(|c: char| !(c.is_alphanumeric() || c == '_')).next()?;
        return (!name.is_empty() && !name.ends_with("_H") && !name.ends_with("_H_")).then(|| (Kind::Macro, name.to_string(), line.to_string()));
    }
    if line.starts_with('#') {
        return None;
    }
    if starts_with_word(line, "namespace") {
        return word_after(line, "namespace").map(|n| (Kind::Namespace, n.to_string(), line.to_string()));
    }
    let plain = line.trim_start_matches("typedef").trim_start().trim_start_matches("template").trim_start();
    if plain.starts_with("enum") || line.starts_with("enum") {
        let name = word_after(line, "enum class").or_else(|| word_after(line, "enum struct")).or_else(|| word_after(line, "enum"))?;
        return (name != "class" && name != "struct").then(|| (Kind::Enum, name.to_string(), line.to_string()));
    }
    if (starts_with_word(line, "class") || starts_with_word(line, "struct") || starts_with_word(line, "union")) && !line.contains('(') {
        let name = word_after(line, "class").or_else(|| word_after(line, "struct")).or_else(|| word_after(line, "union"))?;
        return Some((Kind::Class, name.to_string(), line.to_string()));
    }
    if let Some(rest) = line.strip_prefix("using ") {
        let name = rest.split('=').next()?.trim();
        return (!name.contains(' ') && line.contains('=')).then(|| (Kind::Type, name.to_string(), line.to_string()));
    }
    if line.starts_with("typedef") {
        let before = line.split('(').next().unwrap_or(line);
        let name = before.trim_end_matches(';').split_whitespace().last()?;
        return Some((Kind::Type, name.trim_matches('*').to_string(), line.to_string()));
    }
    if line.contains("constexpr") && line.contains('=') {
        let before = line.split('=').next()?.trim();
        let name = before.split_whitespace().last()?.trim_start_matches('*');
        let name = name.split('[').next().unwrap_or(name);
        return (!name.is_empty()).then(|| (Kind::Constant, name.to_string(), line.to_string()));
    }
    // A function: the last word before the first `(`, when the line looks like a declaration.
    let paren = line.find('(')?;
    let before = line[..paren].trim();
    if before.is_empty() || before.ends_with('=') || line.starts_with("return") || line.starts_with("typedef") || line.starts_with("static_assert") || line.starts_with("friend") {
        return None;
    }
    let name = before.rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '~')).next()?;
    if name.is_empty() || NOT_FUNCTIONS.contains(&name) || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    // Calls in a statement look like declarations: a declaration starts with a type, a macro or a qualifier.
    let ends_ok = line.ends_with(';') || line.ends_with(')') || line.ends_with("const") || line.ends_with("override") || line.ends_with("= 0") || line.contains(") ");
    ends_ok.then(|| (Kind::Function, name.to_string(), line.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    fn read(text: &str) -> Vec<Symbol> {
        let mut out = Vec::new();
        read_header(text, "test/x.h", &mut out);
        out
    }

    #[test]
    fn the_names_of_a_header_are_found_with_their_places() {
        let text = "#pragma once\n#define SANGO_MAX 4\nnamespace pokemon {\n/// The data of one move.\nstruct MoveData {\n  /// Loads the data.\n  static MoveData& GetInstance(const MoveId move) {\n    return table[move];\n  }\n  u8 power; ///< The power.\n  void Set(int a);\n};\nenum class MoveKind : u8 {\n  kDamage = 0, ///< Damage only.\n  kStatus = 1,\n};\nconstexpr u32 kCount = 4;\nusing Row = u8[18];\n}\n";
        let found = read(text);
        let find = |name: &str| found.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} is not found: {found:#?}"));
        assert_eq!((find("SANGO_MAX").kind, find("pokemon").kind), (Kind::Macro, Kind::Namespace));
        assert_eq!((find("MoveData").kind, find("MoveData").qualified.as_str()), (Kind::Class, "pokemon::MoveData"));
        assert_eq!(find("MoveData").doc, "The data of one move.");
        assert_eq!((find("GetInstance").kind, find("GetInstance").qualified.as_str()), (Kind::Function, "pokemon::MoveData::GetInstance"));
        assert_eq!(find("GetInstance").doc, "Loads the data.");
        assert_eq!(find("Set").kind, Kind::Function);
        assert!(!found.iter().any(|s| s.name == "table"), "the body of a function is skipped");
        assert_eq!((find("MoveKind").kind, find("kDamage").kind, find("kDamage").qualified.as_str()), (Kind::Enum, Kind::EnumValue, "pokemon::MoveKind::kDamage"));
        assert_eq!(find("kDamage").doc, "Damage only.");
        assert_eq!(find("kCount").kind, Kind::Constant);
        assert_eq!(find("Row").kind, Kind::Type);
    }

    #[test]
    fn the_typed_word_gives_the_names_that_go_with_it() {
        let text = "namespace pokemon {\nstruct MoveData {\n  static MoveData& GetInstance(int a);\n};\nenum class MoveKind : u8 {\n  kDamage = 0,\n  kStatus = 1,\n};\n}\n";
        let index = Index { symbols: read(text) };
        let names = |typed: &str| index.complete(typed, 10).iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        assert_eq!(names("move")[0], "MoveData");
        assert!(names("move").contains(&"MoveKind".to_string()));
        assert_eq!(names("am"), Vec::<String>::new(), "a short word matches the start of a name only");
        assert_eq!(names("kdam"), ["kDamage"]);
        assert_eq!(names("MoveKind::"), ["kDamage", "kStatus"]);
        assert_eq!(names("pokemon::MoveData::Get"), ["GetInstance"]);
        assert!(names("").is_empty());
    }

    /// With the headers of the plugin (the folder is in `SANGO_PLUGIN`): the names that the plugin documents are found.
    #[test]
    fn the_headers_of_the_plugin_are_read() {
        let Ok(plugin) = std::env::var("SANGO_PLUGIN") else { return };
        let index = Index::build(&Path::new(&plugin).join("lib").join("include"));
        assert!(index.symbols.len() > 2000, "{} names", index.symbols.len());
        let has = |qualified: &str, kind: Kind| index.symbols.iter().any(|s| s.qualified == qualified && s.kind == kind);
        assert!(has("pokemon::MoveData", Kind::Class));
        assert!(has("pokemon::MoveData::GetInstance", Kind::Function));
        assert!(has("battle::TypeChart", Kind::Class));
        assert!(has("battle::TypeChart::Set", Kind::Function));
        assert!(has("battle::TypeMultiplier", Kind::Enum));
        assert!(has("battle::TypeMultiplier::k2", Kind::EnumValue));
        assert!(index.symbols.iter().any(|s| s.name == "kMasterBall" && s.kind == Kind::EnumValue));
        let found = index.complete("GetInst", 5);
        assert!(!found.is_empty() && found[0].header.ends_with(".h"));
        let documented = index.symbols.iter().find(|s| s.qualified == "battle::TypeChart::Set").unwrap();
        assert!(documented.doc.contains("Changes one cell"), "{}", documented.doc);
    }
}
