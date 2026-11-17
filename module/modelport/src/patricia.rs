//! The Patricia dictionaries of the H3D files (BCH): the tree that finds a resource by its name. The construction is in the core (the editor
//! of the maps builds the same trees).

pub use sango_core::patricia::{build, Node};

#[cfg(test)]
mod test {
    use super::*;
    use sango_core::{Dump, Garc};

    /// With the dump: the dictionaries of the models of the game are built again, node for node, from their names.
    #[test]
    fn the_dictionaries_of_the_models_of_the_game_are_built_again() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = Dump::open(&root).unwrap();
        let archive = Garc::open(&dump.archive(8)).unwrap();
        let (mut models, mut dictionaries) = (0, 0);
        for slot in (0..1008).step_by(7) {
            let Some(bytes) = archive.first_file(slot * 8 + 3) else { continue };
            let bytes = if sango_core::lz::is_compressed(bytes) { sango_core::lz::decompress(bytes).unwrap() } else { bytes.to_vec() };
            let Ok(entries) = crate::pc::entries(&bytes) else { continue };
            let Some(bch) = entries.first().filter(|e| e.starts_with(b"BCH")) else { continue };
            let u32_at = |at: usize| u32::from_le_bytes([bch[at], bch[at + 1], bch[at + 2], bch[at + 3]]) as usize;
            let (contents, strings) = (u32_at(8), u32_at(12));
            let name_at = |offset: usize| -> Vec<u8> { bch[strings + offset..].iter().take_while(|&&b| b != 0).copied().collect() };
            for header in 0..15 {
                let base = contents + 12 * header;
                let (count, dic) = (u32_at(base + 4), u32_at(base + 8));
                if count == 0 || dic == 0 || count > 400 {
                    continue;
                }
                let node = |index: usize| (u32_at(contents + dic + 12 * index), u32_at(contents + dic + 12 * index + 4), u32_at(contents + dic + 12 * index + 8));
                // The dictionary of a list of resources has one node more than its entries.
                let names: Vec<Vec<u8>> = (1..=count).map(|i| name_at(node(i).2)).collect();
                let refs: Vec<&[u8]> = names.iter().map(|n| n.as_slice()).collect();
                let built = build(&refs);
                for (index, expected) in built.iter().enumerate() {
                    let (ref_bit, links, _) = node(index);
                    assert_eq!((ref_bit as u32, links & 0xFFFF, links >> 16), (expected.ref_bit, expected.left, expected.right), "slot {slot} dictionary {header} node {index}");
                }
                dictionaries += 1;
            }
            models += 1;
        }
        assert!(models > 50 && dictionaries > 50, "{models} models, {dictionaries} dictionaries");
    }
}
