//! The lists of GPU commands (PICA200): each command is a parameter word, a header word (the register in the low 16 bits, the number of
//! extra parameters in bits 20 to 30, bit 31 for a run of registers), then the extra parameters, padded to an even number of words.

/// The index of the parameter word of the first command that writes the register `reg`.
pub fn find_word(words: &[u32], reg: u32) -> Option<usize> {
    let mut i = 0;
    while i + 1 < words.len() {
        let header = words[i + 1];
        let extra = ((header >> 20) & 0xFF) as usize;
        if header & 0xFFFF == reg {
            return Some(i);
        }
        i += 2 + extra + (extra & 1);
    }
    None
}

/// The index of the parameter word of the command that writes the register `reg`, looking at 11 bits for the number of extra words
/// (the size field of a command is 11 bits wide).
pub fn find_word_wide(words: &[u32], reg: u32) -> Option<usize> {
    let mut i = 0;
    while i + 1 < words.len() {
        let header = words[i + 1];
        let extra = ((header >> 20) & 0x7FF) as usize;
        if header & 0xFFFF == reg {
            return Some(i);
        }
        i += 2 + extra + (extra & 1);
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_register_is_found_in_a_list_of_commands() {
        // A command with 2 extra words, then a command of one word.
        let words = [0x10, 0x002F0200, 0x20, 0x30, 0x50, 0x000F0228];
        assert_eq!(find_word(&words, 0x200), Some(0));
        assert_eq!(find_word(&words, 0x228), Some(4));
        assert_eq!(find_word(&words, 0x999), None);
    }
}
