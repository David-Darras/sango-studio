//! The LZ10 and LZ11 compression of Nintendo.
//!
//! Many files of the game are compressed with it, for example the images of the Pokémon icons.
//!
//! A compressed file starts with a header:
//!
//! | Offset | Size | Meaning |
//! | --- | --- | --- |
//! | 0 | 1 | The kind: `0x10` for LZ10, `0x11` for LZ11. |
//! | 1 | 3 | The size of the data after decompression. When it is 0, the real size is the next `u32`. |
//!
//! After the header, the data is a list of groups. A group starts with one flag byte, then has 8 items.
//! The bits of the flag byte are read from the high bit to the low bit. A clear bit is one byte to copy as it is.
//! A set bit is a copy of bytes that are already written: a length and a distance.
//!
//! | Kind | Length | Distance |
//! | --- | --- | --- |
//! | LZ10 | 3 to 18, in 4 bits | 1 to 4096, in 12 bits |
//! | LZ11 | 3 to 65808, in 4, 12 or 20 bits | 1 to 4096, in 12 bits |
//!
//! In LZ11, the high 4 bits of the first byte of a copy say how the length is written.
//! A value of 0 gives an 8-bit length plus 17. A value of 1 gives a 16-bit length plus 273.
//! A value of 2 or more is the length minus 1.

/// The kind of compression of a file, from its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Lz10,
    Lz11,
}

/// Tells if the bytes start with the header of a compressed file.
pub fn is_compressed(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && matches!(bytes[0], 0x10 | 0x11)
}

/// Decompresses a file. The result has the size that the header gives.
pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let kind = match bytes.first() {
        Some(0x10) => Kind::Lz10,
        Some(0x11) => Kind::Lz11,
        _ => return Err("This data is not compressed with LZ10 or LZ11.".into()),
    };
    if bytes.len() < 4 {
        return Err("The compressed data is too short.".into());
    }
    let mut length = bytes[1] as usize | (bytes[2] as usize) << 8 | (bytes[3] as usize) << 16;
    let mut at = 4;
    if length == 0 && bytes.len() >= 8 {
        length = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        at = 8;
    }
    let mut out: Vec<u8> = Vec::with_capacity(length);
    let cut = || "The compressed data stops too early.".to_string();
    while out.len() < length {
        let flags = *bytes.get(at).ok_or_else(cut)?;
        at += 1;
        for bit in 0..8 {
            if out.len() >= length {
                break;
            }
            if flags & (0x80 >> bit) == 0 {
                out.push(*bytes.get(at).ok_or_else(cut)?);
                at += 1;
                continue;
            }
            let first = *bytes.get(at).ok_or_else(cut)? as usize;
            let high = first >> 4;
            let (count, distance, size) = match kind {
                Kind::Lz10 => {
                    let second = *bytes.get(at + 1).ok_or_else(cut)? as usize;
                    (high + 3, ((first & 0xF) << 8 | second) + 1, 2)
                }
                Kind::Lz11 if high == 0 => {
                    let part = bytes.get(at + 1..at + 3).ok_or_else(cut)?;
                    let (b1, b2) = (part[0] as usize, part[1] as usize);
                    (((first & 0xF) << 4 | b1 >> 4) + 0x11, ((b1 & 0xF) << 8 | b2) + 1, 3)
                }
                Kind::Lz11 if high == 1 => {
                    let part = bytes.get(at + 1..at + 4).ok_or_else(cut)?;
                    let (b1, b2, b3) = (part[0] as usize, part[1] as usize, part[2] as usize);
                    (((first & 0xF) << 12 | b1 << 4 | b2 >> 4) + 0x111, ((b2 & 0xF) << 8 | b3) + 1, 4)
                }
                Kind::Lz11 => {
                    let second = *bytes.get(at + 1).ok_or_else(cut)? as usize;
                    (high + 1, ((first & 0xF) << 8 | second) + 1, 2)
                }
            };
            at += size;
            if distance > out.len() {
                return Err("A copy in the compressed data points before the start.".into());
            }
            for _ in 0..count.min(length - out.len()) {
                out.push(out[out.len() - distance]);
            }
        }
    }
    Ok(out)
}

/// The most bytes that one copy of the compressor takes: the longest copy that it writes.
const LZ11_MAX_COPY: usize = 0x110;

/// The farthest place that a copy can read from.
const LZ11_WINDOW: usize = 4096;

/// Compresses data with LZ11. The result decompresses to the same data, but it is not the same bytes that the
/// tools of Nintendo give: the compressor takes the longest copy that it finds in the last 4096 bytes.
pub fn compress_lz11(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x11];
    if data.len() < 1 << 24 {
        out.extend(&(data.len() as u32).to_le_bytes()[..3]);
    } else {
        out.extend([0, 0, 0]);
        out.extend((data.len() as u32).to_le_bytes());
    }
    let mut at = 0;
    while at < data.len() {
        let flag_place = out.len();
        out.push(0);
        let mut flags = 0u8;
        for bit in 0..8 {
            if at >= data.len() {
                break;
            }
            // The longest copy of what comes next, from the bytes that are already written.
            let (mut best_length, mut best_distance) = (0, 0);
            let longest = LZ11_MAX_COPY.min(data.len() - at);
            if longest >= 3 {
                for distance in 1..=at.min(LZ11_WINDOW) {
                    let start = at - distance;
                    let mut length = 0;
                    // A copy can read bytes that it writes itself, so it can repeat a short pattern.
                    while length < longest && data[start + length] == data[at + length] {
                        length += 1;
                    }
                    if length > best_length {
                        (best_length, best_distance) = (length, distance);
                        if length == longest {
                            break;
                        }
                    }
                }
            }
            if best_length >= 3 {
                flags |= 0x80 >> bit;
                let distance = best_distance - 1;
                if best_length <= 16 {
                    out.push(((best_length - 1) << 4 | distance >> 8) as u8);
                    out.push(distance as u8);
                } else {
                    let length = best_length - 0x11;
                    out.push((length >> 4) as u8);
                    out.push((length << 4 | distance >> 8) as u8);
                    out.push(distance as u8);
                }
                at += best_length;
            } else {
                out.push(data[at]);
                at += 1;
            }
        }
        out[flag_place] = flags;
    }
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn compressed_data_decompresses_to_the_same_data() {
        let samples: Vec<Vec<u8>> = vec![
            vec![],
            vec![7],
            b"abcabcabcabcabcabc".to_vec(),
            (0..=255u8).collect(),
            vec![0; 5000],
            (0..3000u32).map(|n| (n * n % 251) as u8).collect(),
            [vec![1u8; 300], (0..100u8).collect(), vec![1u8; 300]].concat(),
        ];
        for data in samples {
            let packed = compress_lz11(&data);
            assert_eq!(decompress(&packed).unwrap_or_default(), data, "{} bytes", data.len());
        }
        // A run is much smaller than the data.
        assert!(compress_lz11(&[0; 5000]).len() < 100);
    }

    #[test]
    fn lz10_copies_bytes_and_repeats() {
        // "abc", then a copy of 6 bytes at distance 3: "abcabcabc".
        let data = [0x10, 9, 0, 0, 0b0001_0000, b'a', b'b', b'c', 0x30, 0x02];
        assert_eq!(decompress(&data).unwrap(), b"abcabcabc");
    }

    #[test]
    fn lz11_short_copy_uses_length_minus_one() {
        // "ab", then a copy of 4 bytes (high = 3) at distance 2: "ababab".
        let data = [0x11, 6, 0, 0, 0b0010_0000, b'a', b'b', 0x30, 0x01];
        assert_eq!(decompress(&data).unwrap(), b"ababab");
    }

    #[test]
    fn lz11_long_copy_forms() {
        // "a", then a copy with high = 0: length = ((0 << 4) | 0) + 17 = 17 at distance 1.
        let data = [0x11, 18, 0, 0, 0b0100_0000, b'a', 0x00, 0x00, 0x00];
        assert_eq!(decompress(&data).unwrap(), vec![b'a'; 18]);
        // With high = 1: length = 0x111 = 273 at distance 1.
        let data = [0x11, 0x12, 1, 0, 0b0100_0000, b'a', 0x10, 0x00, 0x00, 0x00];
        assert_eq!(decompress(&data).unwrap(), vec![b'a'; 274]);
    }

    #[test]
    fn a_bad_stream_is_refused() {
        assert!(decompress(b"abcd").is_err());
        assert!(decompress(&[0x10, 4, 0, 0, 0b1000_0000, 0x00, 0x05]).is_err());
        assert!(decompress(&[0x10, 4, 0, 0, 0, b'a']).is_err());
        assert!(!is_compressed(&[0x10]));
    }
}
