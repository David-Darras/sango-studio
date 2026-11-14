//! The "PC" packs of the model archive (and the packs of the motions, whose two letters differ): a list of files in one file.
//!
//! ```text
//! "PC", the number of entries (u16), the start of each entry (u32, one more for the end), zeros up to a multiple of 128,
//! then the entries, each one padded to a multiple of 128 bytes
//! ```
//!
//! The model of a Pokémon is 8 packs of this kind (see the module `model`). The plugin serves a pack from the SD card as the size
//! of the pack (4 bytes) and the pack, not compressed.

/// The files of a pack.
pub fn entries(pack: &[u8]) -> Result<Vec<&[u8]>, String> {
    if pack.len() < 4 || pack[0] != b'P' {
        return Err("This is not a pack of files.".into());
    }
    let count = u16::from_le_bytes([pack[2], pack[3]]) as usize;
    let offset = |place: usize| -> Result<usize, String> {
        let at = 4 + 4 * place;
        let bytes = pack.get(at..at + 4).ok_or("The pack is cut.")?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    };
    (0..count)
        .map(|place| {
            let (start, end) = (offset(place)?, offset(place + 1)?);
            pack.get(start..end).ok_or_else(|| "An entry of the pack is out of the file.".to_string())
        })
        .collect()
}

/// A pack made of files. An empty entry stays empty.
pub fn pack(files: &[Vec<u8>]) -> Vec<u8> {
    let header = 4 + 4 * (files.len() + 1);
    let start = (header + 0x7F) & !0x7F;
    let mut body: Vec<u8> = Vec::new();
    let mut offsets = Vec::with_capacity(files.len() + 1);
    for file in files {
        offsets.push((start + body.len()) as u32);
        body.extend_from_slice(file);
        while body.len() % 0x80 != 0 {
            body.push(0);
        }
    }
    offsets.push((start + body.len()) as u32);
    let mut out = Vec::with_capacity(start + body.len());
    out.extend_from_slice(b"PC");
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for offset in offsets {
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.resize(start, 0);
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_pack_keeps_its_files_in_order_on_boundaries_of_128_bytes() {
        let files = vec![vec![1u8; 10], Vec::new(), vec![2u8; 300]];
        let made = pack(&files);
        assert_eq!(&made[..2], b"PC");
        assert_eq!(made.len() % 128, 0);
        let read = entries(&made).unwrap();
        assert_eq!(read.len(), 3);
        // Each entry is padded: it starts with the file.
        assert_eq!(&read[0][..10], &[1u8; 10]);
        assert_eq!(read[1].len(), 0);
        assert_eq!(&read[2][..300], &[2u8; 300][..]);
        assert!(entries(b"nope").is_err());
    }

    /// With the dump: the packs of the models and of the textures of the game (the files 0, 2 and 3 of each model) are made again
    /// byte for byte from their files. The small packs of settings have another alignment.
    #[test]
    fn the_packs_of_the_models_of_the_game_are_made_again_byte_for_byte() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let archive = sango_core::Garc::open(&dump.archive(8)).unwrap();
        let mut checked = 0;
        for id in (3..3 + 8 * 200).filter(|id| matches!((id - 3) % 8, 0 | 2 | 3)) {
            let Some(bytes) = archive.first_file(id) else { continue };
            let bytes = if sango_core::lz::is_compressed(bytes) { sango_core::lz::decompress(bytes).unwrap() } else { bytes.to_vec() };
            if bytes.get(..2) != Some(b"PC") {
                continue;
            }
            let files: Vec<Vec<u8>> = entries(&bytes).unwrap().iter().map(|e| e.to_vec()).collect();
            assert_eq!(pack(&files), bytes, "file {id}");
            checked += 1;
        }
        assert!(checked > 100, "only {checked} packs");
    }
}
