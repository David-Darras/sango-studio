//! The Patricia dictionaries of the H3D files (BCH): the tree that finds a resource by its name.
//!
//! The node 0 is the root and the node `i + 1` is the name `i`. A node has a reference bit and two links. This is the construction that
//! the game's own files follow (checked against the models of the archive in the tests).

/// A node of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub ref_bit: u32,
    pub left: usize,
    pub right: usize,
    /// The name, for the nodes after the root.
    pub name: Option<Vec<u8>>,
}

fn bit(name: &[u8], at: i64) -> u32 {
    if at < 0 {
        return 0;
    }
    let (byte, shift) = ((at >> 3) as usize, (at & 7) as u32);
    name.get(byte).map_or(0, |b| ((b >> shift) & 1) as u32)
}

/// The tree of some names, in the order of the names.
pub fn build(names: &[&[u8]]) -> Vec<Node> {
    let mut nodes = vec![Node { ref_bit: 0xFFFF_FFFF, left: 0, right: 0, name: None }];
    for (place, name) in names.iter().enumerate() {
        let index = place + 1;
        // The closest leaf that exists.
        let mut x = 0usize;
        let mut y = nodes[0].left;
        while nodes[x].ref_bit > nodes[y].ref_bit {
            x = y;
            y = if bit(name, nodes[y].ref_bit as i64) == 1 { nodes[y].right } else { nodes[y].left };
        }
        let other = nodes[y].name.clone().unwrap_or_default();
        let mut at = (name.len().max(other.len()) * 8) as i64 - 1;
        while at >= 0 && bit(name, at) == bit(&other, at) {
            at -= 1;
        }
        // The place where the new node goes in.
        x = 0;
        y = nodes[0].left;
        while nodes[x].ref_bit > nodes[y].ref_bit && (nodes[y].ref_bit as i64) > at {
            x = y;
            y = if bit(name, nodes[y].ref_bit as i64) == 1 { nodes[y].right } else { nodes[y].left };
        }
        let ref_bit = if at < 0 { 0xFFFF_FFFF } else { at as u32 };
        if bit(name, at) == 1 {
            nodes.push(Node { ref_bit, left: y, right: index, name: Some(name.to_vec()) });
        } else {
            nodes.push(Node { ref_bit, left: index, right: y, name: Some(name.to_vec()) });
        }
        if bit(name, nodes[x].ref_bit as i64) == 1 {
            nodes[x].right = index;
        } else {
            nodes[x].left = index;
        }
    }
    nodes
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_dictionary_finds_each_name_by_walking_the_tree() {
        let names: Vec<&[u8]> = vec![b"BodyA", b"Eye", b"Mouth", b"EyeL", b"Hair", b"BodyB"];
        let nodes = build(&names);
        assert_eq!(nodes.len(), names.len() + 1);
        for (place, name) in names.iter().enumerate() {
            // The search of the game: follow the bits until the reference bit does not go down.
            let mut x = 0usize;
            let mut y = nodes[0].left;
            while nodes[x].ref_bit > nodes[y].ref_bit {
                x = y;
                y = if bit(name, nodes[y].ref_bit as i64) == 1 { nodes[y].right } else { nodes[y].left };
            }
            assert_eq!(y, place + 1, "{}", String::from_utf8_lossy(name));
        }
    }
}
