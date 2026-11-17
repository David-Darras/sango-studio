//! The lists of commands of the graphics chip (PICA200) that the model files hold.
//!
//! A command is a parameter word, a header word (the register in the low 16 bits, the number of extra parameters in bits 20 to 30,
//! bit 31 for a run of registers), then the extra parameters, padded to an even number of words.

use std::collections::HashMap;

/// The values that a list of commands gives to the registers, and the place of each value (the number of the word in the list). A run
/// of registers writes the next registers with the extra parameters.
pub fn registers(words: &[u32]) -> HashMap<u16, (u32, usize)> {
    let mut out = HashMap::new();
    let mut at = 0;
    while at + 1 < words.len() {
        let (value, header) = (words[at], words[at + 1]);
        let register = (header & 0xffff) as u16;
        let extra = ((header >> 20) & 0x7ff) as usize;
        let consecutive = header >> 31 != 0;
        if at + 2 + extra > words.len() {
            break;
        }
        out.insert(register, (value, at));
        for k in 0..extra {
            out.insert(if consecutive { register + 1 + k as u16 } else { register }, (words[at + 2 + k], at + 2 + k));
        }
        at += 2 + extra + (extra & 1);
    }
    out
}

/// The place of the parameter word of the first command that writes the register `reg`.
pub fn find_word(words: &[u32], reg: u32) -> Option<usize> {
    let mut at = 0;
    while at + 1 < words.len() {
        let header = words[at + 1];
        let extra = ((header >> 20) & 0x7FF) as usize;
        if header & 0xFFFF == reg {
            return Some(at);
        }
        at += 2 + extra + (extra & 1);
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

    #[test]
    fn a_run_of_registers_writes_the_next_ones() {
        // The register 0x200 with two extra words in a run: the registers 0x201 and 0x202 get them.
        let words = [0x10, 0x802F0200, 0x20, 0x30];
        let registers = registers(&words);
        assert_eq!(registers[&0x200], (0x10, 0));
        assert_eq!(registers[&0x201], (0x20, 2));
        assert_eq!(registers[&0x202], (0x30, 3));
    }
}
