//! The colours of C++: the keywords, the types and the words that start a line of the preprocessor.

use crate::index::Index;
use egui_code_editor::Syntax;
use std::collections::BTreeSet;

/// The keywords of C++ (the language of the plugin, `gnu++11`).
const KEYWORDS: [&str; 69] = [
    "alignas", "alignof", "asm", "auto", "break", "case", "catch", "class", "const", "constexpr", "const_cast", "continue", "decltype", "default", "delete", "do", "dynamic_cast", "else", "enum", "explicit", "extern", "final", "for", "friend", "goto", "if", "inline", "mutable", "namespace", "new", "noexcept", "operator", "override", "private", "protected", "public", "register", "reinterpret_cast", "return", "sizeof", "static", "static_assert", "static_cast", "struct", "switch", "template", "this", "thread_local", "throw", "try", "typedef", "typeid", "typename", "union", "using", "virtual", "volatile", "while", "INLINE", "STATIC_INLINE", "int", "char", "short", "long", "signed", "unsigned", "float", "double", "void",
];

/// The types of the language and of the headers of the system.
const TYPES: [&str; 22] = ["bool", "u8", "u16", "u32", "u64", "s8", "s16", "s32", "s64", "f32", "f64", "vu8", "vu16", "vu32", "uptr", "size_t", "ssize_t", "uintptr_t", "intptr_t", "int32_t", "uint32_t", "uint8_t"];

/// The words of the preprocessor and the values of the language.
const SPECIAL: [&str; 14] = ["include", "define", "pragma", "ifdef", "ifndef", "endif", "if", "elif", "undef", "nullptr", "true", "false", "NULL", "error"];

/// The syntax of C++. The names of the types that the headers of the plugin declare are coloured as types too.
pub fn cpp(index: Option<&Index>) -> Syntax {
    let mut types: BTreeSet<&'static str> = TYPES.into_iter().collect();
    if let Some(index) = index {
        // The editor wants names that live as long as the program: the names are few, and an index is made once.
        for name in index.type_names() {
            types.insert(Box::leak(name.to_string().into_boxed_str()));
        }
    }
    Syntax::new("C++")
        .with_case_sensitive(true)
        .with_comment("//")
        .with_comment_multiline(["/*", "*/"])
        .with_keywords(BTreeSet::from(KEYWORDS))
        .with_types(types)
        .with_special(BTreeSet::from(SPECIAL))
}

#[cfg(test)]
mod test {
    use super::*;
    use egui_code_editor::{Token, TokenType};

    #[test]
    fn the_words_of_cpp_have_their_colours() {
        let syntax = cpp(None);
        let text = "// note\n#include \"common.h\"\nextern \"C\" u32 sango_run(u32 argument) {\n  return 42; /* done */\n}\n";
        let tokens = Token::default().tokens(&syntax, text);
        let kind = |word: &str| tokens.iter().find(|t| t.buffer() == word).map(|t| t.ty());
        assert!(matches!(kind("// note"), Some(TokenType::Comment(false))));
        assert_eq!(kind("extern"), Some(TokenType::Keyword));
        assert_eq!(kind("u32"), Some(TokenType::Type));
        assert_eq!(kind("return"), Some(TokenType::Keyword));
        assert_eq!(kind("include"), Some(TokenType::Special));
        assert!(matches!(kind("42"), Some(TokenType::Numeric(_))));
        assert!(matches!(kind("\"common.h\""), Some(TokenType::Str(_))));
        assert!(matches!(kind("/* done */"), Some(TokenType::Comment(true))));
    }
}
