//! The sound archive of the game (`sound/sango_sound.bcsar`): the list of all the sounds with their names.
//!
//! All the numbers are little-endian. The file has a header and three blocks: the strings (`STRG`), the information (`INFO`) and
//! the data of the sounds (`FILE`).
//!
//! | Place | Content |
//! | --- | --- |
//! | 0x14 | three entries of 12 bytes: the type of the block, 2 unused bytes, the offset, the size |
//! | STRG + 8 | two references: the table of the strings, the search tree. The table has a count, then for each string a type, 2 unused bytes, an offset and a size. The offsets start at the table. |
//! | INFO + 8 | eight references (type, 2 unused bytes, offset from INFO + 8) to the tables of the sounds, the groups of sounds, the banks, the wave archives, the groups, the players, the files and the player settings |
//!
//! A table has a count, then one reference for each item. The offset of a reference of a table starts at the table. The sound
//! has the number of its file, a reference to its details (a stream or a sequence), and some options: bit 0 gives the number of its
//! name in the table of the strings. A file is inside the archive (an offset in the block `FILE` and a size) or outside it
//! (the path of the file, for the streams).

/// What a sound is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A long sound in its own file (a music or a jingle).
    Stream,
    /// A sequence: notes that a bank of waves plays (the sound effects and the cries).
    Sequence,
    /// A wave.
    Wave,
    Other,
}

/// Where the data of a sound is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A file outside the archive: the name of a `.bcstm` file of the folder `sound`.
    External(String),
    /// A file inside the archive: the offset from the start of the block `FILE` and the size.
    Internal { offset: u32, size: u32 },
    None,
}

/// A bank of instruments (`CBNK`). The file of the bank lists the waves that it plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    pub name: String,
    pub place: Place,
}

/// A wave archive: a file with waves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaveArchive {
    pub name: String,
    pub place: Place,
}

/// A sound of the archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    /// The place of the sound in the list of the archive.
    pub index: usize,
    /// The label that the archive gives to the sound (the program does not show it: the page uses English names).
    pub name: String,
    pub kind: Kind,
    /// The number of the file of the sound.
    pub file_id: u32,
    pub place: Place,
    /// For a sequence: the places of its banks in the list of the banks.
    pub banks: Vec<usize>,
}

/// The sounds of an archive.
#[derive(Clone, Debug, Default)]
pub struct Archive {
    pub sounds: Vec<Sound>,
    pub banks: Vec<Bank>,
    pub wave_archives: Vec<WaveArchive>,
    /// The offset of the block `FILE` in the archive file.
    pub file_block: u32,
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn u16(&self, at: usize) -> Result<u16, String> {
        self.0.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| format!("The archive stops at {at:#x}."))
    }

    fn u32(&self, at: usize) -> Result<u32, String> {
        self.0.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("The archive stops at {at:#x}."))
    }

    /// The place that a reference at `at` points to: `base` plus its offset.
    fn target(&self, at: usize, base: usize) -> Result<usize, String> {
        let offset = self.u32(at + 4)? as i32;
        if offset < 0 {
            return Err(format!("The reference at {at:#x} is empty."));
        }
        Ok(base + offset as usize)
    }

    fn text(&self, at: usize, size: usize) -> Result<String, String> {
        let bytes = self.0.get(at..at + size).ok_or_else(|| format!("The archive stops at {at:#x}."))?;
        Ok(String::from_utf8_lossy(bytes.split(|b| *b == 0).next().unwrap_or(&[])).into_owned())
    }

    /// The value of a bit of options (a flag word, then one value for each bit that is set).
    fn option(&self, at: usize, bit: u32) -> Result<Option<u32>, String> {
        let flags = self.u32(at)?;
        if flags >> bit & 1 == 0 {
            return Ok(None);
        }
        let rank = (flags & (u32::MAX >> (31 - bit))).count_ones() as usize;
        self.u32(at + rank * 4).map(Some)
    }
}

impl Archive {
    /// Reads the list of the sounds of a `.bcsar` file.
    pub fn parse(bytes: &[u8]) -> Result<Archive, String> {
        if bytes.get(..4) != Some(b"CSAR") {
            return Err("This is not a sound archive.".into());
        }
        let r = Reader(bytes);
        let mut blocks = [(0usize, 0usize); 3];
        for (i, block) in blocks.iter_mut().enumerate() {
            let kind = r.u16(0x14 + i * 12)?;
            if kind != 0x2000 + i as u16 {
                return Err(format!("The block {i} of the archive has the type {kind:#x}."));
            }
            *block = (r.u32(0x14 + i * 12 + 4)? as usize, r.u32(0x14 + i * 12 + 8)? as usize);
        }
        let (strings, info, file) = (blocks[0].0 + 8, blocks[1].0 + 8, blocks[2].0);

        // The strings.
        let table = r.target(strings, strings)?;
        let count = r.u32(table)? as usize;
        let mut names = Vec::with_capacity(count);
        for i in 0..count {
            let at = table + 4 + i * 12;
            names.push(r.text(table + r.u32(at + 4)? as usize, r.u32(at + 8)? as usize)?);
        }

        // The tables of the information.
        let find = |kind: u16| -> Result<usize, String> {
            for i in 0..8 {
                if r.u16(info + i * 8)? == kind {
                    return r.target(info + i * 8, info);
                }
            }
            Err(format!("The archive has no table of the type {kind:#x}."))
        };
        let sounds = find(0x2100)?;
        let files = find(0x2106)?;
        let item = |table: usize, index: usize| r.target(table + 4 + index * 8, table);
        // Where the file of a file number is.
        let place_of = |file_id: u32| -> Result<Place, String> {
            if (file_id as usize) >= r.u32(files)? as usize {
                return Ok(Place::None);
            }
            let info_at = item(files, file_id as usize)?;
            let location = info_at + r.u32(info_at + 4)? as usize;
            Ok(match r.u16(info_at)? {
                0x220D => Place::External(r.text(location, 200)?),
                0x220C => Place::Internal { offset: r.u32(location + 4)?, size: r.u32(location + 8)? },
                _ => Place::None,
            })
        };
        let name_of = |at: usize| -> Result<String, String> {
            Ok(match r.option(at, 0)? {
                Some(id) if (id as usize) < names.len() => names[id as usize].clone(),
                _ => String::new(),
            })
        };
        // The wave archives, then the banks.
        let archives_table = find(0x2103)?;
        let mut wave_archives = Vec::new();
        for i in 0..r.u32(archives_table)? as usize {
            let at = item(archives_table, i)?;
            wave_archives.push(WaveArchive { name: name_of(at + 8).unwrap_or_default(), place: place_of(r.u32(at)?)? });
        }
        let banks_table = find(0x2101)?;
        let mut banks = Vec::new();
        for i in 0..r.u32(banks_table)? as usize {
            let at = item(banks_table, i)?;
            banks.push(Bank { name: name_of(at + 12).unwrap_or_default(), place: place_of(r.u32(at)?)? });
        }

        let mut out = Vec::new();
        for index in 0..r.u32(sounds)? as usize {
            let at = item(sounds, index)?;
            let file_id = r.u32(at)?;
            let kind = match r.u16(at + 12)? {
                0x2201 => Kind::Stream,
                0x2202 => Kind::Wave,
                0x2203 => Kind::Sequence,
                _ => Kind::Other,
            };
            let name = name_of(at + 20)?;
            let mut used = Vec::new();
            if kind == Kind::Sequence {
                let detail = r.target(at + 12, at)?;
                let list = r.target(detail, detail)?;
                for k in 0..r.u32(list)? as usize {
                    used.push((r.u32(list + 4 + k * 4)? & 0x00FF_FFFF) as usize);
                }
            }
            out.push(Sound { index, name, kind, file_id, place: place_of(file_id)?, banks: used });
        }
        Ok(Archive { sounds: out, banks, wave_archives, file_block: file as u32 })
    }
}
