//! The lists of commands of the graphics chip (PICA200) that the model files hold.
//!
//! A command is a parameter word, a header word (the register in the low 16 bits, the number of extra parameters in bits 20 to 30,
//! bit 31 for a run of registers), then the extra parameters, padded to an even number of words.

use std::collections::HashMap;

/// The values that a list of commands gives to the registers, and the place of each value (the number of the word in the list). A run
/// of registers writes the next registers with the extra parameters.
pub fn registers(words: &[u32]) -> HashMap<u16, (u32, usize)> {
    let mut out = HashMap::new();
    each_write(words, |register, value, at| {
        out.insert(register, (value, at));
    });
    out
}

/// Gives each write of a list of commands to `write`, in order: the register, the value and the place of the value.
fn each_write(words: &[u32], mut write: impl FnMut(u16, u32, usize)) {
    let mut at = 0;
    while at + 1 < words.len() {
        let (value, header) = (words[at], words[at + 1]);
        let register = (header & 0xffff) as u16;
        let extra = ((header >> 20) & 0x7ff) as usize;
        let consecutive = header >> 31 != 0;
        if at + 2 + extra > words.len() {
            break;
        }
        write(register, value, at);
        for k in 0..extra {
            write(if consecutive { register.wrapping_add(1 + k as u16) } else { register }, words[at + 2 + k], at + 2 + k);
        }
        at += 2 + extra + (extra & 1);
    }
}

/// A number of 24 bits of the graphics chip: the sign, 7 bits of exponent (63 for the power 0) and 16 bits of fraction.
fn float24(value: u32) -> f32 {
    let sign = (value >> 23 & 1) << 31;
    if value & 0x7f_ffff == 0 {
        return f32::from_bits(sign);
    }
    f32::from_bits(sign | (((value >> 16 & 0x7f) + 64) << 23) | ((value & 0xffff) << 7))
}

/// The values that a list of commands gives to the fixed attributes of the vertices (the attributes that are the same for all
/// the vertices of a mesh), by the number of the attribute. The register 0x232 chooses the attribute, and the registers 0x233
/// to 0x235 give its four numbers of 24 bits in three words, the fourth number first.
pub fn fixed_attributes(words: &[u32]) -> [Option<[f32; 4]>; 12] {
    let mut out = [None; 12];
    let (mut attribute, mut data) = (0usize, [0u32; 3]);
    each_write(words, |register, value, _| match register {
        0x232 => attribute = value as usize,
        0x233 => data[0] = value,
        0x234 => data[1] = value,
        0x235 => {
            data[2] = value;
            if let Some(place) = out.get_mut(attribute) {
                let [a, b, c] = data;
                *place = Some([c & 0xff_ffff, (c >> 24) | (b & 0xffff) << 8, (b >> 16) | (a & 0xff) << 16, a >> 8].map(float24));
            }
        }
        _ => {}
    });
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
    fn the_numbers_of_a_fixed_attribute_are_read() {
        // 1.0 is 0x3f0000, 0.5 is 0x3e0000 and -1.0 is 0xbf0000: the numbers (1, 0.5, -1, 0.5) of the attribute 3.
        let (x, y, z, w) = (0x3f_0000u32, 0x3e_0000u32, 0xbf_0000u32, 0x3e_0000u32);
        let words = [3, 0x000f_0232, w << 8 | z >> 16, 0x802f_0233, z << 16 | y >> 8, y << 24 | x];
        let fixed = fixed_attributes(&words);
        assert_eq!(fixed[3], Some([1.0, 0.5, -1.0, 0.5]));
        assert_eq!(fixed.iter().flatten().count(), 1);
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
