//! Reads and writes numbers in the bytes of a file (little-endian, like all the files of the game).

/// The 16-bit number at a place, or an error when the data stops before.
pub fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The data stops too early (at {at:#x})."))
}

/// The 32-bit number at a place, or an error when the data stops before.
pub fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The data stops too early (at {at:#x})."))
}

/// Writes a 16-bit number at a place.
pub fn put16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 32-bit number at a place.
pub fn put32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn numbers_are_read_and_written_little_endian() {
        let mut bytes = vec![0u8; 8];
        put16(&mut bytes, 1, 0x1234);
        put32(&mut bytes, 4, 0xAABBCCDD);
        assert_eq!((u16_at(&bytes, 1), u32_at(&bytes, 4)), (Ok(0x1234), Ok(0xAABBCCDD)));
        assert!(u32_at(&bytes, 6).is_err() && u16_at(&bytes, 7).is_err());
    }
}
