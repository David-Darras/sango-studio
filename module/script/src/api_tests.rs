//! Tests of the API of the natives: the names in the words of the plugin and the lists of values.

use crate::natives::{self, NATIVES};
use std::collections::HashSet;

#[test]
fn a_native_has_one_alias_that_finds_it() {
    let mut seen = HashSet::new();
    for native in NATIVES {
        assert!(!native.alias.is_empty(), "{} has no alias", native.name);
        assert!(native.alias.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'), "{}: {}", native.name, native.alias);
        // The same native can be in two tables; two different natives never share an alias.
        if seen.insert(native.alias) {
            assert_eq!(natives::find(native.alias).map(|n| n.name), Some(native.name), "{}", native.alias);
        }
        assert_eq!(natives::word_hash(native.alias), natives::hash(native.name), "{}", native.name);
        assert_eq!(natives::word_hash(native.name), natives::hash(native.name));
    }
}

#[test]
fn the_aliases_use_the_words_of_the_plugin() {
    assert_eq!(natives::find("PlayerGetSex").unwrap().alias, "get_player_gender");
    let words = ["direction", "sex", "obj", "mons", "waza", "zukan", "sodateya", "grid", "mdl", "msg", "wk", "btl"];
    for native in NATIVES {
        for word in native.alias.split('_') {
            assert!(!words.contains(&word), "{}: {}", native.name, native.alias);
        }
        for param in native.params {
            for word in param.name.split('_') {
                assert!(!words.contains(&word), "{}({})", native.name, param.name);
            }
        }
    }
}

#[test]
fn the_lists_of_values_name_their_enumerators() {
    for list in natives::LISTS {
        assert!(!list.enums.is_empty(), "the list {} has no enum class of the plugin", list.name);
        let mut names = HashSet::new();
        for (value, name, text) in list.items {
            assert!(!text.is_empty(), "{} {value}", list.name);
            // The value 65536 and the next one of the move commands are not in an enum (they do not fit in 16 bits).
            if !name.is_empty() {
                assert!(name.starts_with('k'), "{}: {name}", list.name);
                assert!(names.insert(*name), "{}: {name} twice", list.name);
            }
        }
    }
    for native in NATIVES {
        for param in native.params {
            assert!(param.list.is_empty() || natives::list(param.list).is_some(), "{}({}): the list {} does not exist", native.name, param.name, param.list);
        }
    }
}

#[test]
fn the_facing_list_has_the_values_of_the_plugin() {
    let list = natives::list("Facing").unwrap();
    let find = |value: i64| list.items.iter().find(|i| i.0 == value).unwrap().1;
    assert_eq!((find(0), find(1), find(2), find(3), find(9)), ("kUp", "kDown", "kLeft", "kRight", "kInvalid"));
    let female = natives::list("Nature").unwrap().items.iter().find(|i| i.1 == "kHardy").unwrap().0;
    assert_eq!(female, 0);
}
