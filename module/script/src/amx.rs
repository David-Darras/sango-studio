//! The container of a compiled script: a header, the tables of the public functions and of the native functions, then the code and the data
//! (written in a compact form that uses fewer bytes for small numbers).
//!
//! The file is read and written again with the same bytes: what the Studio does not change (the other tables, the bytes after the data) is kept.

/// The size of the header of the file.
pub const HEADER_SIZE: usize = 60;
/// The signature of a file with cells of 32 bits.
pub const MAGIC: u16 = 0xF1E0;
/// The flag of a file whose code and data are written in the compact form.
pub const FLAG_COMPACT: u16 = 0x0004;
/// The size of a record of a table (an address and a name).
const RECORD: usize = 8;

/// A record of the table of the public functions or of the native functions: the address (of the code, for a public function) and the name.
/// The game writes the name as a number made from the text (see `names.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub address: u32,
    pub name: u32,
}

/// A compiled script.
#[derive(Clone, Debug, PartialEq)]
pub struct Amx {
    pub file_version: u8,
    pub amx_version: u8,
    pub flags: u16,
    pub definition_size: u16,
    /// The top of the stack.
    pub stack_top: i32,
    /// The address of the code where the script starts, or -1.
    pub entry: i32,
    pub publics: Vec<Entry>,
    pub natives: Vec<Entry>,
    /// The places of the tables after the natives (the libraries, the public variables, the tags, the names, the overlays), counted from the
    /// start of the libraries, and the bytes that they and the space between them hold.
    pub table_offsets: [i32; 5],
    pub other_tables: Vec<u8>,
    pub code: Vec<i32>,
    pub data: Vec<i32>,
    /// The bytes after the compact code and data (none in the files of the game).
    pub tail: Vec<u8>,
}

fn int(bytes: &[u8], at: usize) -> Result<i32, String> {
    bytes.get(at..at + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "The script is cut.".to_string())
}

/// Writes a cell in the compact form: the groups of 7 bits, the high ones first, the shortest form that keeps the sign.
fn encode_cell(value: i32, out: &mut Vec<u8>) {
    let p = value as u32;
    let mut t = [0u8; 5];
    for (index, byte) in t.iter_mut().enumerate() {
        *byte = ((p >> (7 * index)) & 0x7f) as u8;
    }
    let mut index = 5usize;
    while index > 1 && t[index - 1] == 0 && (t[index - 2] & 0x40) == 0 {
        index -= 1;
    }
    if index == 5 && t[index - 1] == 0x0f && (t[index - 2] & 0x40) != 0 {
        index -= 1;
    }
    while index > 1 && t[index - 1] == 0x7f && (t[index - 2] & 0x40) != 0 {
        index -= 1;
    }
    while index > 0 {
        index -= 1;
        out.push(if index == 0 { t[index] } else { t[index] | 0x80 });
    }
}

/// Reads a cell of the compact form. Gives the value and the number of bytes used.
fn decode_cell(bytes: &[u8]) -> Result<(i32, usize), String> {
    let mut value: u32 = 0;
    let mut used = 0;
    loop {
        let byte = *bytes.get(used).ok_or("The compact code is cut.")?;
        if used >= 5 {
            return Err("A cell of the compact code is too long.".to_string());
        }
        value = (value << 7) | u32::from(byte & 0x7f);
        used += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    let bits = 7 * used as u32;
    if bits < 32 && bytes[0] & 0x40 != 0 {
        value |= u32::MAX << bits;
    }
    Ok((value as i32, used))
}

impl Amx {
    /// Reads a file.
    pub fn parse(bytes: &[u8]) -> Result<Amx, String> {
        if bytes.len() < HEADER_SIZE {
            return Err("The script is too small.".to_string());
        }
        let size = int(bytes, 0)? as usize;
        let magic = u16::from_le_bytes([bytes[4], bytes[5]]);
        if magic != MAGIC {
            return Err("This is not a script with cells of 32 bits.".to_string());
        }
        let flags = u16::from_le_bytes([bytes[8], bytes[9]]);
        let definition_size = u16::from_le_bytes([bytes[10], bytes[11]]);
        if usize::from(definition_size) != RECORD {
            return Err(format!("A record of the tables has {definition_size} bytes: only {RECORD} are known."));
        }
        let [cod, dat, hea, stp, cip, publics, natives, libraries, pubvars, tags, nametable, overlays] = {
            let mut fields = [0i32; 12];
            for (k, field) in fields.iter_mut().enumerate() {
                *field = int(bytes, 12 + 4 * k)?;
            }
            fields
        };
        if flags & FLAG_COMPACT == 0 {
            return Err("Only the scripts in the compact form are known.".to_string());
        }
        if publics as usize != HEADER_SIZE || natives < publics || libraries < natives || cod < libraries || size > bytes.len() || size < cod as usize {
            return Err("The tables of the script are not in the usual order.".to_string());
        }
        let table = |from: i32, to: i32| -> Result<Vec<Entry>, String> {
            let (from, to) = (from as usize, to as usize);
            if (to - from) % RECORD != 0 {
                return Err("A table of the script has a cut record.".to_string());
            }
            (from..to).step_by(RECORD).map(|at| Ok(Entry { address: int(bytes, at)? as u32, name: int(bytes, at + 4)? as u32 })).collect()
        };
        let (publics_list, natives_list) = (table(publics, natives)?, table(natives, libraries)?);
        let other_tables = bytes[libraries as usize..cod as usize].to_vec();
        if dat < cod || hea < dat || (dat - cod) % 4 != 0 || (hea - dat) % 4 != 0 {
            return Err("The code and the data of the script have a size that is not a number of cells.".to_string());
        }
        let cells = ((hea - cod) / 4) as usize;
        let mut all = Vec::with_capacity(cells);
        let mut at = cod as usize;
        for _ in 0..cells {
            let (value, used) = decode_cell(&bytes[at..size])?;
            all.push(value);
            at += used;
        }
        if at != size {
            return Err("The compact code does not end where the file ends.".to_string());
        }
        let code_cells = ((dat - cod) / 4) as usize;
        Ok(Amx {
            file_version: bytes[6],
            amx_version: bytes[7],
            flags,
            definition_size,
            stack_top: stp,
            entry: cip,
            publics: publics_list,
            natives: natives_list,
            table_offsets: [0, pubvars - libraries, tags - libraries, nametable - libraries, overlays - libraries],
            other_tables,
            code: all[..code_cells].to_vec(),
            data: all[code_cells..].to_vec(),
            tail: bytes[size..].to_vec(),
        })
    }

    /// The bytes of the file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let libraries = HEADER_SIZE + RECORD * (self.publics.len() + self.natives.len());
        let cod = libraries + self.other_tables.len();
        let dat = cod + 4 * self.code.len();
        let hea = dat + 4 * self.data.len();
        let mut stream = Vec::new();
        for &cell in self.code.iter().chain(&self.data) {
            encode_cell(cell, &mut stream);
        }
        let size = cod + stream.len();
        let mut out = Vec::with_capacity(size + self.tail.len());
        out.extend_from_slice(&(size as i32).to_le_bytes());
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.push(self.file_version);
        out.push(self.amx_version);
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&self.definition_size.to_le_bytes());
        let natives_at = HEADER_SIZE + RECORD * self.publics.len();
        let t = self.table_offsets;
        for value in [cod as i32, dat as i32, hea as i32, self.stack_top, self.entry, HEADER_SIZE as i32, natives_at as i32, libraries as i32, libraries as i32 + t[1], libraries as i32 + t[2], libraries as i32 + t[3], libraries as i32 + t[4]] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for entry in self.publics.iter().chain(&self.natives) {
            out.extend_from_slice(&entry.address.to_le_bytes());
            out.extend_from_slice(&entry.name.to_le_bytes());
        }
        out.extend_from_slice(&self.other_tables);
        out.extend_from_slice(&stream);
        out.extend_from_slice(&self.tail);
        out
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_cell_goes_through_the_compact_form() {
        for value in [0, 1, -1, 63, 64, -64, -65, 8191, 8192, 1_000_000, -1_000_000, i32::MAX, i32::MIN, 0x7fff_ffff, 0x1234_5678] {
            let mut bytes = Vec::new();
            encode_cell(value, &mut bytes);
            assert_eq!(decode_cell(&bytes).unwrap(), (value, bytes.len()), "{value}");
        }
        let mut one = Vec::new();
        encode_cell(0, &mut one);
        assert_eq!(one, [0]);
    }

    /// With the dump: every script of the three archives is read and written again with the same bytes.
    #[test]
    fn the_scripts_of_the_game_keep_their_bytes() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let mut checked = 0;
        for archive in [7usize, 29, 89] {
            let garc = sango_core::Garc::open(&dump.archive(archive)).unwrap();
            for id in 0..garc.file_count() {
                let Some(bytes) = garc.first_file_decompressed(id) else { continue };
                if bytes.len() < 8 || u16::from_le_bytes([bytes[4], bytes[5]]) != MAGIC {
                    continue;
                }
                let amx = Amx::parse(&bytes).unwrap_or_else(|e| panic!("archive {archive} file {id}: {e}"));
                assert!(amx.to_bytes() == bytes, "archive {archive} file {id}: the file is not the same");
                checked += 1;
            }
        }
        assert!(checked >= 90, "{checked}");
    }
}
