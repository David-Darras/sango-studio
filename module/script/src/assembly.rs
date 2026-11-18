//! The code of a script as text, and the text as code.
//!
//! The text names every address that a jump or a call goes to (`L0042`), so the user can add or remove instructions: the numbers of the jumps are
//! made again when the text is built. The text has the tables (the public functions and the natives), the code and the data, and it gives back the
//! same file when nothing is changed.

use crate::amx::{Amx, Entry};
use crate::opcodes::{Form, OPCODES};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The form and the name of an instruction by its number.
pub(crate) fn opcode(number: u32) -> Option<(&'static str, Form)> {
    OPCODES.iter().find(|(n, _, _)| *n == number).map(|(_, name, form)| (*name, *form))
}

/// The number and the form of an instruction by its name.
fn opcode_named(name: &str) -> Option<(u32, Form)> {
    OPCODES.iter().find(|(_, n, _)| *n == name).map(|(number, _, form)| (*number, *form))
}

/// The label of an address of the code.
fn label(address: i64) -> String {
    format!("L{address:04X}")
}

/// An instruction: its address, its number and the cells after the number.
pub(crate) struct Instruction {
    pub(crate) at: usize,
    pub(crate) number: u32,
    pub(crate) cells: Vec<i32>,
}

pub(crate) fn decode(code: &[i32]) -> Result<Vec<Instruction>, String> {
    let mut list = Vec::new();
    let mut cell = 0;
    while cell < code.len() {
        let word = code[cell] as u32;
        let number = word & 0xFFFF;
        let (_, form) = opcode(number).ok_or_else(|| format!("The instruction {number} at the address {:04X} is not known.", cell * 4))?;
        let wanted = match form {
            Form::Cells(n) => n,
            Form::Packed => 0,
            Form::Jump => 1,
            Form::CaseTable => {
                let count = *code.get(cell + 1).ok_or("A table of cases is cut.")?;
                if !(0..=100_000).contains(&count) {
                    return Err(format!("A table of cases at the address {:04X} has {count} records.", cell * 4));
                }
                1 + 1 + 2 * count as usize
            }
        };
        let end = cell + 1 + wanted;
        if end > code.len() {
            return Err(format!("The instruction at the address {:04X} is cut.", cell * 4));
        }
        let mut cells = code[cell + 1..end].to_vec();
        if form == Form::Packed {
            cells = vec![(word >> 16) as i32];
        }
        list.push(Instruction { at: cell * 4, number, cells });
        cell = end;
    }
    Ok(list)
}

/// The address that a cell of the code gives: the value, from the cell before the cell.
pub(crate) fn target_of(cell_at: usize, value: i32) -> i64 {
    cell_at as i64 - 4 + i64::from(value)
}

fn parse_number(text: &str) -> Result<i64, String> {
    let text = text.trim().trim_end_matches(',');
    let (negative, digits) = text.strip_prefix('-').map_or((false, text), |rest| (true, rest));
    let value = match digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16),
        None => digits.parse::<i64>(),
    }
    .map_err(|_| format!("\"{text}\" is not a number."))?;
    Ok(if negative { -value } else { value })
}

/// The script as text. `native_name` gives the name that the game uses for a native (from its number), for the comments.
pub fn to_text(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<String, String> {
    let code = decode(&amx.code)?;
    let starts: BTreeSet<usize> = code.iter().map(|i| i.at).collect();
    let mut labels: BTreeMap<i64, String> = BTreeMap::new();
    for entry in &amx.publics {
        labels.insert(i64::from(entry.address), format!("public_{:08X}", entry.name));
    }
    if amx.entry >= 0 {
        labels.entry(i64::from(amx.entry)).or_insert_with(|| label(i64::from(amx.entry)));
    }
    let mut targets = |at: usize, cell_index: usize, value: i32| {
        let target = target_of(at + 4 * cell_index, value);
        if target >= 0 && starts.contains(&(target as usize)) {
            labels.entry(target).or_insert_with(|| label(target));
        }
    };
    for instruction in &code {
        match opcode(instruction.number).map(|(_, f)| f) {
            Some(Form::Jump) => targets(instruction.at, 1, instruction.cells[0]),
            Some(Form::CaseTable) => {
                targets(instruction.at, 2, instruction.cells[1]);
                for record in 0..instruction.cells[0] as usize {
                    targets(instruction.at, 4 + 2 * record, instruction.cells[3 + 2 * record]);
                }
            }
            _ => {}
        }
    }
    let name_of = |target: i64| labels.get(&target).cloned();
    let mut out = String::new();
    out.push_str("; The script, as assembly. The addresses of the jumps are made again when the script is built.\n");
    out.push_str(&format!(".amx {} {} 0x{:04X} {} {}\n", amx.file_version, amx.amx_version, amx.flags, amx.definition_size, amx.stack_top));
    out.push_str(&format!(".entry {}\n", if amx.entry < 0 { "none".to_string() } else { name_of(i64::from(amx.entry)).unwrap_or_else(|| label(i64::from(amx.entry))) }));
    let t = amx.table_offsets;
    out.push_str(&format!(".tables {} {} {} {}\n", t[1], t[2], t[3], t[4]));
    if !amx.other_tables.is_empty() {
        for chunk in amx.other_tables.chunks(32) {
            out.push_str(&format!(".other {}\n", chunk.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")));
        }
    }
    for entry in &amx.publics {
        out.push_str(&format!(".public 0x{:08X} {}\n", entry.name, name_of(i64::from(entry.address)).unwrap_or_else(|| label(i64::from(entry.address)))));
    }
    for entry in &amx.natives {
        let name = native_name(entry.name).unwrap_or_else(|| format!("0x{:08X}", entry.name));
        out.push_str(&format!(".native {name} {}\n", entry.address));
    }
    out.push_str(".code\n");
    for instruction in &code {
        if let Some(name) = labels.get(&(instruction.at as i64)) {
            out.push_str(&format!("{name}:\n"));
        }
        let (name, form) = opcode(instruction.number).ok_or("An instruction is not known.")?;
        let target_text = |cell_index: usize, value: i32| -> String {
            let target = target_of(instruction.at + 4 * cell_index, value);
            labels.get(&target).cloned().filter(|_| target >= 0 && starts.contains(&(target as usize))).unwrap_or_else(|| format!("@{value}"))
        };
        match form {
            Form::Cells(0) => out.push_str(&format!("    {name}\n")),
            Form::Cells(_) | Form::Packed => {
                let mut operands: Vec<String> = instruction.cells.iter().map(|c| c.to_string()).collect();
                if name.starts_with("sysreq") {
                    if let Some(text) = native_operand(amx, instruction.cells[0], native_name) {
                        operands[0] = text;
                    }
                }
                out.push_str(&format!("    {name} {}\n", operands.join(", ")));
            }
            Form::Jump => out.push_str(&format!("    {name} {}\n", target_text(1, instruction.cells[0]))),
            Form::CaseTable => {
                out.push_str(&format!("    {name} {}\n", target_text(2, instruction.cells[1])));
                for record in 0..instruction.cells[0] as usize {
                    out.push_str(&format!("    case {}, {}\n", instruction.cells[2 + 2 * record], target_text(4 + 2 * record, instruction.cells[3 + 2 * record])));
                }
            }
        }
    }
    out.push_str(".data\n");
    for chunk in amx.data.chunks(8) {
        out.push_str(&format!("    cell {}\n", chunk.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")));
    }
    if !amx.tail.is_empty() {
        out.push_str(&format!(".tail {}\n", amx.tail.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")));
    }
    Ok(out)
}

/// The name of a native in an instruction that calls it, when the table has the name once (else the number is written).
fn native_operand(amx: &Amx, index: i32, native_name: &dyn Fn(u32) -> Option<String>) -> Option<String> {
    let entry = amx.natives.get(usize::try_from(index).ok()?)?;
    let name = native_name(entry.name)?;
    let same = amx.natives.iter().filter(|n| native_name(n.name).as_deref() == Some(name.as_str())).count();
    (same == 1).then_some(name)
}

fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    text.split_whitespace().map(|b| u8::from_str_radix(b, 16).map_err(|_| format!("\"{b}\" is not a byte."))).collect()
}

/// An instruction that is being read, with its labels not resolved yet.
enum Pending {
    Cells(u32, Vec<i32>),
    Packed(u32, i32),
    Jump(u32, String),
    Table(u32, String, Vec<(i32, String)>),
}

/// Builds a script from the text. The jumps are made again from the labels; the sizes of the file are made again.
pub fn from_text(text: &str) -> Result<Amx, String> {
    let mut amx = Amx { file_version: 10, amx_version: 10, flags: 0, definition_size: 8, stack_top: 0, entry: -1, publics: Vec::new(), natives: Vec::new(), table_offsets: [0; 5], other_tables: Vec::new(), code: Vec::new(), data: Vec::new(), tail: Vec::new() };
    let mut entry_label: Option<String> = None;
    let mut public_labels: Vec<(u32, String)> = Vec::new();
    let mut pending: Vec<(usize, Pending)> = Vec::new();
    let mut label_at: HashMap<String, usize> = HashMap::new();
    let mut section = "";
    let mut at = 0usize;
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let fail = |message: String| format!("Line {}: {message}", number + 1);
        if let Some(rest) = line.strip_prefix('.') {
            let (word, args) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(w, a)| (w, a.trim()));
            match word {
                "amx" => {
                    let parts: Vec<i64> = args.split_whitespace().map(parse_number).collect::<Result<_, _>>().map_err(&fail)?;
                    if parts.len() != 5 {
                        return Err(fail(".amx needs five numbers.".to_string()));
                    }
                    (amx.file_version, amx.amx_version, amx.flags, amx.definition_size, amx.stack_top) = (parts[0] as u8, parts[1] as u8, parts[2] as u16, parts[3] as u16, parts[4] as i32);
                }
                "entry" => entry_label = (args != "none").then(|| args.to_string()),
                "tables" => {
                    let parts: Vec<i64> = args.split_whitespace().map(parse_number).collect::<Result<_, _>>().map_err(&fail)?;
                    if parts.len() != 4 {
                        return Err(fail(".tables needs four numbers.".to_string()));
                    }
                    amx.table_offsets = [0, parts[0] as i32, parts[1] as i32, parts[2] as i32, parts[3] as i32];
                }
                "other" => amx.other_tables.extend(parse_hex_bytes(args).map_err(&fail)?),
                "public" => {
                    let (hash, label) = args.split_once(char::is_whitespace).ok_or_else(|| fail(".public needs a name and a label.".to_string()))?;
                    public_labels.push((parse_number(hash).map_err(&fail)? as u32, label.trim().to_string()));
                }
                "native" => {
                    let mut parts = args.split_whitespace();
                    let word = parts.next().ok_or_else(|| fail(".native needs a name.".to_string()))?;
                    let hash = if word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') { crate::natives::word_hash(word) } else { parse_number(word).map_err(&fail)? as u32 };
                    let address = parts.next().map_or(Ok(0), parse_number).map_err(&fail)? as u32;
                    amx.natives.push(Entry { address, name: hash });
                }
                "code" | "data" => section = if word == "code" { "code" } else { "data" },
                "tail" => amx.tail = parse_hex_bytes(args).map_err(&fail)?,
                _ => return Err(fail(format!("The directive .{word} is not known."))),
            }
            continue;
        }
        if let Some(name) = line.strip_suffix(':') {
            if section != "code" {
                return Err(fail("A label must be in the code.".to_string()));
            }
            if label_at.insert(name.trim().to_string(), at).is_some() {
                return Err(fail(format!("The label {} is twice in the code.", name.trim())));
            }
            continue;
        }
        match section {
            "data" => {
                let rest = line.strip_prefix("cell").ok_or_else(|| fail("A line of the data starts with \"cell\".".to_string()))?;
                for value in rest.split(',') {
                    amx.data.push(parse_number(value).map_err(&fail)? as i32);
                }
            }
            "code" => {
                let (name, args) = line.split_once(char::is_whitespace).map_or((line, ""), |(n, a)| (n, a.trim()));
                if name == "case" {
                    let Some((_, Pending::Table(_, _, records))) = pending.last_mut() else { return Err(fail("\"case\" must follow a table of cases.".to_string())) };
                    let (value, label) = args.split_once(',').ok_or_else(|| fail("\"case\" needs a value and a label.".to_string()))?;
                    records.push((parse_number(value).map_err(&fail)? as i32, label.trim().to_string()));
                    at += 8;
                    continue;
                }
                let (number_of, form) = opcode_named(name).ok_or_else(|| fail(format!("The instruction \"{name}\" is not known.")))?;
                let operands: Vec<&str> = if args.is_empty() { Vec::new() } else { args.split(',').map(str::trim).collect() };
                match form {
                    Form::Cells(n) => {
                        if operands.len() != n {
                            return Err(fail(format!("\"{name}\" needs {n} number(s).")));
                        }
                        let mut cells = Vec::new();
                        for (place, operand) in operands.iter().enumerate() {
                            // A native can be called by its name: it must be in the table (the lines .native).
                            if place == 0 && name.starts_with("sysreq") && operand.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
                                let wanted = crate::natives::word_hash(operand);
                                let at = amx.natives.iter().position(|n| n.name == wanted).ok_or_else(|| fail(format!("The native {operand} is not in the table: add a line \".native {operand}\" at the start.")))?;
                                cells.push(at as i32);
                            } else {
                                cells.push(parse_number(operand).map_err(&fail)? as i32);
                            }
                        }
                        pending.push((at, Pending::Cells(number_of, cells)));
                        at += 4 * (1 + n);
                    }
                    Form::Packed => {
                        if operands.len() != 1 {
                            return Err(fail(format!("\"{name}\" needs one number.")));
                        }
                        pending.push((at, Pending::Packed(number_of, parse_number(operands[0]).map_err(&fail)? as i32)));
                        at += 4;
                    }
                    Form::Jump => {
                        if operands.len() != 1 {
                            return Err(fail(format!("\"{name}\" needs one label.")));
                        }
                        pending.push((at, Pending::Jump(number_of, operands[0].to_string())));
                        at += 8;
                    }
                    Form::CaseTable => {
                        if operands.len() != 1 {
                            return Err(fail(format!("\"{name}\" needs the label of the other cases.")));
                        }
                        pending.push((at, Pending::Table(number_of, operands[0].to_string(), Vec::new())));
                        at += 12;
                    }
                }
            }
            _ => return Err(fail("A line is out of the code and the data.".to_string())),
        }
    }
    let resolve = |name: &str, line_at: usize, cell_at: usize| -> Result<i32, String> {
        let _ = line_at;
        if let Some(raw) = name.strip_prefix('@') {
            return Ok(parse_number(raw)? as i32);
        }
        let target = *label_at.get(name).ok_or_else(|| format!("The label {name} is not in the code."))?;
        Ok((target as i64 - (cell_at as i64 - 4)) as i32)
    };
    for (start, item) in &pending {
        match item {
            Pending::Cells(number, cells) => {
                amx.code.push(*number as i32);
                amx.code.extend_from_slice(cells);
            }
            Pending::Packed(number, value) => amx.code.push(((*value as u32) << 16 | *number) as i32),
            Pending::Jump(number, name) => {
                amx.code.push(*number as i32);
                amx.code.push(resolve(name, *start, start + 4)?);
            }
            Pending::Table(number, default, records) => {
                amx.code.push(*number as i32);
                amx.code.push(records.len() as i32);
                amx.code.push(resolve(default, *start, start + 8)?);
                for (k, (value, name)) in records.iter().enumerate() {
                    amx.code.push(*value);
                    amx.code.push(resolve(name, *start, start + 16 + 8 * k)?);
                }
            }
        }
    }
    amx.publics = public_labels.iter().map(|(name, label)| label_at.get(label).map(|&a| Entry { address: a as u32, name: *name }).ok_or_else(|| format!("The label {label} of a public function is not in the code."))).collect::<Result<_, _>>()?;
    amx.entry = match entry_label {
        Some(label) => *label_at.get(&label).ok_or_else(|| format!("The label {label} of the start is not in the code."))? as i32,
        None => -1,
    };
    Ok(amx)
}

#[cfg(test)]
mod test {
    use super::*;

    /// With the dump: every script of the game goes to text and comes back as the same file.
    #[test]
    fn the_scripts_of_the_game_come_back_from_their_text() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let (mut checked, mut lines) = (0, 0);
        for archive in [7usize, 29, 89] {
            let garc = sango_core::Garc::open(&dump.archive(archive)).unwrap();
            for id in 0..garc.file_count() {
                let Some(bytes) = garc.first_file_decompressed(id) else { continue };
                if bytes.len() < 8 || u16::from_le_bytes([bytes[4], bytes[5]]) != crate::amx::MAGIC {
                    continue;
                }
                let amx = Amx::parse(&bytes).unwrap();
                let names = crate::natives::display_names();
                let text = to_text(&amx, &|h| names.get(&h).cloned()).unwrap_or_else(|e| panic!("archive {archive} file {id}: {e}"));
                lines += text.lines().count();
                let again = from_text(&text).unwrap_or_else(|e| panic!("archive {archive} file {id}: {e}"));
                assert!(again.to_bytes() == bytes, "archive {archive} file {id}: the file is not the same");
                checked += 1;
            }
        }
        assert!(checked >= 90, "{checked}");
        eprintln!("{checked} scripts, {lines} lines of assembly");
    }

    #[test]
    fn an_instruction_added_to_the_text_moves_the_jumps() {
        let text = ".amx 10 10 0x0004 8 4096\n.entry L0000\n.tables 0 0 0 0\n.code\nL0000:\n    proc\n    jump Lend\n    nop\nLend:\n    retn\n.data\n";
        let first = from_text(text).unwrap();
        let more = from_text(&text.replace("    nop\n", "    nop\n    nop\n")).unwrap();
        // proc, jump + 1 cell, nop, retn: the jump goes over the nop (3 cells from the jump).
        assert_eq!(first.code[2], 12);
        assert_eq!(more.code[2], 16, "one more instruction: the jump is longer");
        assert!(from_text(&text.replace("Lend\n", "Lnone\n")).is_err());
    }
}
