//! The dictionaries of the model files.
//!
//! A model keeps the names of its materials in a binary tree (a Patricia tree).
//! The game finds a name in it by looking at some bits of the name. A node is
//! 12 bytes: the number of the bit to look at, the numbers of the left and of
//! the right node (16 bits each), and the place of the name in the strings. The
//! first node is the root: its bit number is 0xFFFFFFFF.

/// A node of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub bit: u32,
    pub left: u16,
    pub right: u16,
    pub name: u32,
}

impl Node {
    pub fn to_bytes(self) -> [u8; 12] {
        let mut out = [0u8; 12];
        out[0..4].copy_from_slice(&self.bit.to_le_bytes());
        out[4..6].copy_from_slice(&self.left.to_le_bytes());
        out[6..8].copy_from_slice(&self.right.to_le_bytes());
        out[8..12].copy_from_slice(&self.name.to_le_bytes());
        out
    }

    pub fn from_bytes(b: &[u8]) -> Node {
        Node {
            bit: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            left: u16::from_le_bytes([b[4], b[5]]),
            right: u16::from_le_bytes([b[6], b[7]]),
            name: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
        }
    }
}

/// Builds the tree of some names, in the form of the file. The names are in the order of the table: the node `k + 1` is for the name `k`.
/// The place of the name in the strings comes from `offsets`. The tree itself is built in the core.
pub fn build(names: &[&[u8]], offsets: &[u32]) -> Vec<Node> {
    sango_core::patricia::build(names)
        .into_iter()
        .enumerate()
        .map(|(index, node)| Node { bit: node.ref_bit, left: node.left as u16, right: node.right as u16, name: if index == 0 { 0 } else { offsets[index - 1] } })
        .collect()
}
